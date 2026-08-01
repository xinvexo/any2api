use secrecy::SecretSlice;

/// Secret bytes kept redacted by `Debug` while they are in process memory.
pub type SecretBytes = SecretSlice<u8>;
