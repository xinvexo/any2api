use super::ProtocolTargetProfile;

#[derive(Clone, Copy, Debug)]
pub struct ProtocolBridgeContext<'a> {
    pub upstream_model: &'a str,
    pub target_profile: ProtocolTargetProfile,
}

impl<'a> ProtocolBridgeContext<'a> {
    #[must_use]
    pub const fn new(upstream_model: &'a str, target_profile: ProtocolTargetProfile) -> Self {
        Self {
            upstream_model,
            target_profile,
        }
    }
}
