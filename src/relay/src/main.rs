use collab_server::Relay;
use openidea_collab_protocol::DEFAULT_ORIGIN;
use std::{net::SocketAddr, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .max_blocking_threads(2)
        .build()?
        .block_on(run())
}
async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let address: SocketAddr = std::env::var("COLLAB_BIND")
        .unwrap_or_else(|_| "127.0.0.1:4444".into())
        .parse()?;
    if !address.ip().is_loopback() {
        return Err("relay must bind to loopback; use a TLS reverse proxy".into());
    }
    let origin =
        std::env::var("COLLAB_PUBLIC_ORIGIN").unwrap_or_else(|_| DEFAULT_ORIGIN.to_owned());
    let state = PathBuf::from(
        std::env::var("COLLAB_STATE_FILE").unwrap_or_else(|_| "collab-state/metadata.json".into()),
    );
    let state = if state.is_absolute() {
        state
    } else {
        std::env::current_dir()?.join(state)
    };
    let admission = std::env::var("COLLAB_ADMISSION_TOKEN").ok();
    let relay = Relay::with_admission(&origin, Some(state), admission.as_deref())?;
    let listener = tokio::net::TcpListener::bind(address).await?;
    eprintln!(
        "OpenIDEA relay listening on {address}; content is transient, authorization metadata is persisted"
    );
    let app = relay.router();
    let reporter = relay.clone();
    let reporter_task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
        loop {
            interval.tick().await;
            reporter.log_presence_snapshot();
        }
    });
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            #[cfg(unix)]
            {
                let mut terminate =
                    tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                        .expect("signal handler");
                tokio::select! { _ = tokio::signal::ctrl_c() => {}, _ = terminate.recv() => {} }
            }
            #[cfg(not(unix))]
            {
                let _ = tokio::signal::ctrl_c().await;
            }
            relay.shutdown();
        })
        .await?;
    reporter_task.abort();
    Ok(())
}
