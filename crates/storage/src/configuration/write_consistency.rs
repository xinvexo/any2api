use crate::error::{ConfigurationWriteComponent, StorageError};

pub(crate) fn ensure_write_matches<T>(
    actual: T,
    expected: T,
    component: ConfigurationWriteComponent,
) -> Result<(), StorageError>
where
    T: PartialEq,
{
    if actual == expected {
        Ok(())
    } else {
        Err(StorageError::ConfigurationWriteMismatch(component))
    }
}

#[cfg(test)]
mod tests {
    use crate::error::{ConfigurationWriteComponent, StorageError};

    use super::ensure_write_matches;

    #[test]
    fn every_configuration_component_returns_a_typed_mismatch() {
        for component in ConfigurationWriteComponent::ALL {
            ensure_write_matches(7, 7, component).expect("matching values");
            assert!(matches!(
                ensure_write_matches(7, 8, component),
                Err(StorageError::ConfigurationWriteMismatch(actual)) if actual == component
            ));
        }
    }
}
