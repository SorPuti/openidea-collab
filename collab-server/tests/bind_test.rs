use collab_server::bind_address;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

#[test]
fn defaults_to_loopback_and_requires_explicit_external_bind() {
    assert_eq!(
        bind_address(None, 4444).unwrap(),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 4444)
    );
    assert_eq!(
        bind_address(Some("0.0.0.0"), 4444).unwrap(),
        "0.0.0.0:4444".parse().unwrap()
    );
    assert!(bind_address(Some("not-an-ip"), 4444).is_err());
}
