use any2api_domain::ProviderKind;

pub struct OAuthStartResult {
    provider: ProviderKind,
    session_id: String,
    authorization_url: String,
    redirect_uri: &'static str,
    expires_in_seconds: u64,
}

impl OAuthStartResult {
    pub(super) fn new(
        provider: ProviderKind,
        session_id: String,
        authorization_url: String,
        redirect_uri: &'static str,
        expires_in_seconds: u64,
    ) -> Self {
        Self {
            provider,
            session_id,
            authorization_url,
            redirect_uri,
            expires_in_seconds,
        }
    }

    #[must_use]
    pub const fn provider(&self) -> ProviderKind {
        self.provider
    }

    #[must_use]
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    #[must_use]
    pub fn authorization_url(&self) -> &str {
        &self.authorization_url
    }

    #[must_use]
    pub const fn redirect_uri(&self) -> &'static str {
        self.redirect_uri
    }

    #[must_use]
    pub const fn expires_in_seconds(&self) -> u64 {
        self.expires_in_seconds
    }
}

pub struct OAuthDownload {
    provider: ProviderKind,
    filename: &'static str,
    bytes: Vec<u8>,
}

impl OAuthDownload {
    pub(super) fn new(provider: ProviderKind, bytes: Vec<u8>) -> Self {
        let filename = match provider {
            ProviderKind::Codex => "codex-auth.json",
            ProviderKind::Claude => "claude-auth.json",
        };
        Self {
            provider,
            filename,
            bytes,
        }
    }

    #[must_use]
    pub const fn provider(&self) -> ProviderKind {
        self.provider
    }

    #[must_use]
    pub const fn filename(&self) -> &'static str {
        self.filename
    }

    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}
