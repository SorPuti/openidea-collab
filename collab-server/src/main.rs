use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use collab_server::routes::router;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let state: Arc<Mutex<HashMap<String, collab_server::room::Room>>> =
        Arc::new(Mutex::new(HashMap::new()));

    // Background task: expire inactive members every 30 s.
    let cleanup_state = state.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
            if let Ok(mut guard) = cleanup_state.lock() {
                for room in guard.values_mut() {
                    room.expire_members(30);
                }
            }
        }
    });

    let mode = std::env::var("COLLAB_API").unwrap_or_else(|_| "legacy".into());
    let app = match mode.as_str() {
        "legacy" => router(state),
        "realtime" => {
            if let Some(path) = std::env::var_os("COLLAB_JOURNAL") {
                let path = std::path::PathBuf::from(path);
                if !path.is_absolute() {
                    return Err(
                        "COLLAB_JOURNAL must be an absolute path in a private directory".into(),
                    );
                }
                tracing::warn!(
                    "Experimental journal enabled; secure and back up its private directory"
                );
                collab_server::realtime::persistent_router(&path)?
            } else {
                tracing::warn!(
                    "Experimental real-time API: state is in memory and is lost on restart"
                );
                collab_server::realtime::router()
            }
        }
        _ => return Err("COLLAB_API must be legacy or realtime".into()),
    };

    let port: u16 = std::env::var("COLLAB_PORT")
        .unwrap_or_else(|_| "4444".to_string())
        .parse()
        .unwrap_or(4444);
    let host = std::env::var("COLLAB_BIND").ok();
    let addr = collab_server::bind_address(host.as_deref(), port)?;

    if mode == "realtime" && !addr.ip().is_loopback() {
        return Err(
            "realtime API requires a loopback bind; use an HTTPS reverse proxy for remote access"
                .into(),
        );
    }

    if !addr.ip().is_loopback() {
        tracing::warn!("External bind selected; this prototype does not authenticate clients. Protect access before exposing it.");
    }

    tracing::info!("collab-server listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
