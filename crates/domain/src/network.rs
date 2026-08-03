use std::net::IpAddr;

#[must_use]
pub fn canonical_ip(address: IpAddr) -> IpAddr {
    address.to_canonical()
}

#[must_use]
pub fn is_loopback_ip(address: IpAddr) -> bool {
    canonical_ip(address).is_loopback()
}

#[cfg(test)]
mod tests {
    use super::{canonical_ip, is_loopback_ip};

    #[test]
    fn ipv4_mapped_addresses_use_their_canonical_ipv4_semantics() {
        let mapped_loopback = "::ffff:127.0.0.1".parse().expect("mapped loopback");
        let mapped_external = "::ffff:203.0.113.8".parse().expect("mapped external");

        assert_eq!(
            canonical_ip(mapped_loopback),
            "127.0.0.1"
                .parse::<std::net::IpAddr>()
                .expect("IPv4 loopback")
        );
        assert!(is_loopback_ip(mapped_loopback));
        assert_eq!(
            canonical_ip(mapped_external),
            "203.0.113.8"
                .parse::<std::net::IpAddr>()
                .expect("IPv4 external")
        );
        assert!(!is_loopback_ip(mapped_external));
    }
}
