pub use openidea_collab_core as collaboration;
pub mod document;
pub mod events;
pub mod journal;
pub mod realtime;
pub mod room;
pub mod routes;

/// Bind locally by default. External exposure must be configured explicitly.
pub fn bind_address(
    host: Option<&str>,
    port: u16,
) -> Result<std::net::SocketAddr, std::net::AddrParseError> {
    let ip = host.unwrap_or("127.0.0.1").parse::<std::net::IpAddr>()?;
    Ok(std::net::SocketAddr::new(ip, port))
}
