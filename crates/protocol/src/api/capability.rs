use any2api_domain::ProtocolOperation;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProtocolFidelity {
    Direct,
    Translated,
}

impl ProtocolFidelity {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Direct => "direct",
            Self::Translated => "translated",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BridgeRequestFieldBehavior {
    Forwarded,
    Translated,
    ValidatedOnly,
    LocalState,
}

impl BridgeRequestFieldBehavior {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Forwarded => "forwarded",
            Self::Translated => "translated",
            Self::ValidatedOnly => "validated_only",
            Self::LocalState => "local_state",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BridgeRequestFieldCapability {
    pub path: &'static str,
    pub behavior: BridgeRequestFieldBehavior,
}

impl BridgeRequestFieldCapability {
    #[must_use]
    pub const fn new(path: &'static str, behavior: BridgeRequestFieldBehavior) -> Self {
        Self { path, behavior }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BridgeLimitation {
    pub code: &'static str,
    pub description: &'static str,
}

impl BridgeLimitation {
    #[must_use]
    pub const fn new(code: &'static str, description: &'static str) -> Self {
        Self { code, description }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProtocolBridgeCapabilities {
    pub contract_id: &'static str,
    pub operations: &'static [ProtocolOperation],
    pub request_fields: &'static [BridgeRequestFieldCapability],
    pub tool_types: &'static [&'static str],
    pub limitations: &'static [BridgeLimitation],
}

impl ProtocolBridgeCapabilities {
    #[must_use]
    pub fn supports_operation(self, operation: ProtocolOperation) -> bool {
        self.operations.contains(&operation)
    }

    #[must_use]
    pub fn request_field(self, path: &str) -> Option<BridgeRequestFieldCapability> {
        self.request_fields
            .iter()
            .copied()
            .find(|field| field.path == path)
    }

    #[must_use]
    pub fn supports_tool_type(self, tool_type: &str) -> bool {
        self.tool_types.contains(&tool_type)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProtocolPairCapabilities {
    pub fidelity: ProtocolFidelity,
    pub operations: Vec<ProtocolOperation>,
    pub bridge: Option<&'static ProtocolBridgeCapabilities>,
}
