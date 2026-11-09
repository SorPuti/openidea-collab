use std::collections::HashMap;
use std::net::SocketAddr;
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

    let app = router(state);

    let port: u16 = std::env::var("COLLAB_PORT")
        .unwrap_or_else(|_| "4444".to_string())
        .parse()
        .unwrap_or(4444);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    tracing::info!("collab-server listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
