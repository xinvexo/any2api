use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value};

use crate::{ProtocolError, api::OpenAiChatCompletionsProfile};

use super::{
    ChatToolKind, SourceCallKind, ToolIdentity,
    call::{self, ProjectedSourceCall, RestoredToolCall},
    definition, name,
};

#[derive(Clone, Debug)]
pub(in crate::openai_responses_chat) struct ToolProjection {
    pub(super) profile: OpenAiChatCompletionsProfile,
    pub(super) original_to_chat: BTreeMap<ToolIdentity, String>,
    chat_to_original: BTreeMap<(ChatToolKind, String), ToolIdentity>,
    deferred: BTreeSet<ToolIdentity>,
    active: BTreeSet<(ChatToolKind, String)>,
    chat_tools: Vec<Value>,
}

impl ToolProjection {
    pub(in crate::openai_responses_chat) fn new(
        profile: OpenAiChatCompletionsProfile,
        previous: Option<&Self>,
    ) -> Self {
        let mut projection = previous.cloned().unwrap_or_else(|| Self {
            profile,
            original_to_chat: BTreeMap::new(),
            chat_to_original: BTreeMap::new(),
            deferred: BTreeSet::new(),
            active: BTreeSet::new(),
            chat_tools: Vec::new(),
        });
        projection.profile = profile;
        projection.active.clear();
        projection.chat_tools.clear();
        projection
    }

    pub(in crate::openai_responses_chat) fn configure(
        &mut self,
        value: Option<&Value>,
    ) -> Result<(), ProtocolError> {
        definition::configure(self, value)
    }

    pub(in crate::openai_responses_chat) fn activate_loaded_tools(
        &mut self,
        tools: &[Value],
    ) -> Result<(), ProtocolError> {
        definition::activate_loaded(self, tools)
    }

    pub(in crate::openai_responses_chat) fn chat_tools(&self) -> &[Value] {
        &self.chat_tools
    }

    pub(in crate::openai_responses_chat) fn has_active_tools(&self) -> bool {
        !self.active.is_empty()
    }

    pub(in crate::openai_responses_chat) fn project_tool_choice(
        &self,
        value: &Value,
    ) -> Result<Value, ProtocolError> {
        definition::project_tool_choice(self, value)
    }

    pub(in crate::openai_responses_chat) fn project_source_call(
        &mut self,
        object: &Map<String, Value>,
        kind: SourceCallKind,
    ) -> Result<ProjectedSourceCall, ProtocolError> {
        call::project_source_call(self, object, kind)
    }

    pub(in crate::openai_responses_chat) fn restore_function_call(
        &self,
        name: &str,
        arguments: &str,
    ) -> Result<RestoredToolCall, ProtocolError> {
        call::restore_call(self, ChatToolKind::Function, name, arguments)
    }

    pub(in crate::openai_responses_chat) fn restore_custom_call(
        &self,
        name: &str,
        input: &str,
    ) -> Result<RestoredToolCall, ProtocolError> {
        call::restore_call(self, ChatToolKind::Custom, name, input)
    }

    pub(in crate::openai_responses_chat) fn serialized_bytes(&self) -> usize {
        let mappings = self
            .original_to_chat
            .iter()
            .fold(0_usize, |total, (identity, chat)| {
                total
                    .saturating_add(identity_bytes(identity))
                    .saturating_add(chat.len())
            });
        self.deferred.iter().fold(mappings, |total, identity| {
            total.saturating_add(identity_bytes(identity))
        })
    }

    pub(super) fn register_deferred(&mut self, identity: ToolIdentity) {
        self.deferred.insert(identity);
    }

    pub(super) fn is_registered_deferred(&self, identity: &ToolIdentity) -> bool {
        self.deferred.contains(identity)
    }

    pub(super) fn has_registered_identity(&self, identity: &ToolIdentity) -> bool {
        self.original_to_chat.contains_key(identity)
    }

    pub(super) fn ensure_identity(
        &mut self,
        identity: ToolIdentity,
    ) -> Result<String, ProtocolError> {
        if let Some(name) = self.original_to_chat.get(&identity) {
            return Ok(name.clone());
        }
        let chat_kind = identity.chat_kind(self.profile);
        let name = name::project_name(
            &identity,
            chat_kind,
            self.profile.tool_name_policy.max_chars(),
            |candidate| {
                self.chat_to_original
                    .get(&(chat_kind, candidate.to_owned()))
                    .is_some_and(|existing| existing != &identity)
            },
        )?;
        self.original_to_chat.insert(identity.clone(), name.clone());
        self.chat_to_original
            .insert((chat_kind, name.clone()), identity);
        Ok(name)
    }

    pub(super) fn activate(
        &mut self,
        identity: &ToolIdentity,
        definition: Value,
    ) -> Result<(), ProtocolError> {
        let name = self.ensure_identity(identity.clone())?;
        let key = (identity.chat_kind(self.profile), name);
        if !self.active.insert(key) {
            return Err(ProtocolError::InvalidPayload(format!(
                "tool `{}` is declared more than once",
                identity.diagnostic_name()
            )));
        }
        self.chat_tools.push(definition);
        Ok(())
    }

    pub(super) fn active_name(&self, identity: &ToolIdentity) -> Result<&str, ProtocolError> {
        let name = self.original_to_chat.get(identity).ok_or_else(|| {
            ProtocolError::InvalidPayload(format!(
                "tool_choice references undeclared tool `{}`",
                identity.diagnostic_name()
            ))
        })?;
        let key = (identity.chat_kind(self.profile), name.clone());
        self.active
            .contains(&key)
            .then_some(name.as_str())
            .ok_or_else(|| {
                ProtocolError::InvalidPayload(format!(
                    "tool_choice references inactive tool `{}`",
                    identity.diagnostic_name()
                ))
            })
    }

    pub(super) fn active_identity(
        &self,
        kind: ChatToolKind,
        name: &str,
    ) -> Result<&ToolIdentity, ProtocolError> {
        let key = (kind, name.to_owned());
        if !self.active.contains(&key) {
            return Err(ProtocolError::InvalidPayload(
                "Chat Completions returned a call for an undeclared tool".into(),
            ));
        }
        self.chat_to_original.get(&key).ok_or_else(|| {
            ProtocolError::Internal("active tool projection has no reverse mapping".into())
        })
    }

    pub(in crate::openai_responses_chat) fn known_identity(
        &self,
        kind: ChatToolKind,
        name: &str,
    ) -> Result<ToolIdentity, ProtocolError> {
        self.chat_to_original
            .get(&(kind, name.to_owned()))
            .cloned()
            .ok_or_else(|| {
                ProtocolError::InvalidPayload(
                    "conversation contains an unknown projected tool name".into(),
                )
            })
    }

    pub(in crate::openai_responses_chat) fn active_identity_for_call(
        &self,
        kind: ChatToolKind,
        name: &str,
    ) -> Result<ToolIdentity, ProtocolError> {
        self.active_identity(kind, name).cloned()
    }
}

fn identity_bytes(identity: &ToolIdentity) -> usize {
    match identity {
        ToolIdentity::Function { name } | ToolIdentity::Custom { name } => name.len(),
        ToolIdentity::NamespaceFunction { namespace, name }
        | ToolIdentity::NamespaceCustom { namespace, name } => {
            namespace.len().saturating_add(name.len())
        }
        ToolIdentity::ToolSearch => "tool_search".len(),
    }
}
