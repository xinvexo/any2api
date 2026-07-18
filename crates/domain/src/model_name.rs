use thiserror::Error;

pub const MAX_MODEL_NAME_CHARS: usize = 255;

macro_rules! define_model_name {
    ($name:ident) => {
        #[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, ModelNameValidationError> {
                validate(value.into()).map(Self)
            }

            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}

define_model_name!(PublicModelName);
define_model_name!(UpstreamModelName);

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum ModelNameValidationError {
    #[error("model name must not be empty")]
    Empty,
    #[error("model name must be trimmed")]
    NotTrimmed,
    #[error("model name is too long")]
    TooLong,
    #[error("model name must not contain control characters")]
    ControlCharacter,
}

fn validate(value: String) -> Result<String, ModelNameValidationError> {
    if value.trim().is_empty() {
        return Err(ModelNameValidationError::Empty);
    }
    if value.trim() != value {
        return Err(ModelNameValidationError::NotTrimmed);
    }
    if value.chars().count() > MAX_MODEL_NAME_CHARS {
        return Err(ModelNameValidationError::TooLong);
    }
    if value.chars().any(char::is_control) {
        return Err(ModelNameValidationError::ControlCharacter);
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::{ModelNameValidationError, PublicModelName};

    #[test]
    fn model_names_are_exact_and_allow_local_aliases() {
        let name = PublicModelName::new("本地/Codex").expect("model name");

        assert_eq!(name.as_str(), "本地/Codex");
        assert_eq!(
            PublicModelName::new(" model ").expect_err("untrimmed name"),
            ModelNameValidationError::NotTrimmed
        );
    }
}
