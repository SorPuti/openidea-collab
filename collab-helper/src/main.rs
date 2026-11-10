//! Experimental line protocol for an approved OpenIDEA PTY helper.
//! The experimental installer builds it separately and the user must approve it.
use futures_util::StreamExt;
#[cfg(unix)]
use openidea_collab_helper::recovery::{default_directory, Recovery};
use openidea_collab_helper::{Client, SessionApi};
use serde::Deserialize;
use tokio::{
    io::AsyncWriteExt,
    time::{Duration, Instant},
};
use tokio_util::codec::{FramedRead, LinesCodec};
use zeroize::{Zeroize, Zeroizing};

// The PTY host has bounded buffers. Keep this proof conservative until framing
// and backpressure are validated in the real OpenIDEA host.
const MAX_LINE: usize = 64 * 1024;
const MAX_EDITOR_BYTES: usize = 4096;

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Command {
    Attach {
        id: String,
        endpoint: String,
        room: String,
        document: String,
        token: String,
        text: String,
        #[serde(default)]
        recovery: bool,
    },
    Create {
        id: String,
        endpoint: String,
        document: String,
        text: String,
        #[serde(default)]
        recovery: bool,
    },
    Join {
        id: String,
        endpoint: String,
        room: String,
        document: String,
        invite: String,
        text: String,
        #[serde(default)]
        recovery: bool,
    },
    Invite {
        id: String,
        role: String,
        ttl_seconds: u64,
    },
    Edit {
        id: String,
        from: u32,
        to: u32,
        insert: String,
    },
    Disconnect {
        id: String,
    },
}

#[cfg(unix)]
struct TerminalGuard(Option<libc::termios>);
#[cfg(unix)]
impl TerminalGuard {
    fn new() -> std::io::Result<Self> {
        use std::os::fd::AsRawFd;
        let fd = std::io::stdin().as_raw_fd();
        // Only change the terminal when input is a PTY; pipes need no termios.
        if unsafe { libc::isatty(fd) } != 1 {
            return Ok(Self(None));
        }
        let mut original = std::mem::MaybeUninit::<libc::termios>::uninit();
        if unsafe { libc::tcgetattr(fd, original.as_mut_ptr()) } != 0 {
            return Err(std::io::Error::last_os_error());
        }
        let original = unsafe { original.assume_init() };
        let mut raw = original;
        unsafe { libc::cfmakeraw(&mut raw) };
        if unsafe { libc::tcsetattr(fd, libc::TCSANOW, &raw) } != 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(Self(Some(original)))
    }
}
#[cfg(unix)]
impl Drop for TerminalGuard {
    fn drop(&mut self) {
        use std::os::fd::AsRawFd;
        if let Some(original) = &self.0 {
            unsafe {
                libc::tcsetattr(std::io::stdin().as_raw_fd(), libc::TCSANOW, original);
            }
        }
    }
}

async fn emit(output: &mut tokio::io::Stdout, value: serde_json::Value) -> std::io::Result<()> {
    let mut bytes = Zeroizing::new(serde_json::to_vec(&value).map_err(std::io::Error::other)?);
    if bytes.len() > MAX_LINE {
        return Err(std::io::Error::other("output frame too large"));
    }
    bytes.push(b'\n');
    output.write_all(&bytes).await?;
    output.flush().await
}

#[cfg(unix)]
async fn handle(
    command: Command,
    client: &mut Option<Client>,
    backup: &mut Option<Recovery>,
    out: &mut tokio::io::Stdout,
) -> std::io::Result<()> {
    match command {
        Command::Create {
            id,
            endpoint,
            document,
            text,
            recovery,
        } => {
            if client.is_some() || text.len() > MAX_EDITOR_BYTES {
                return emit(out, serde_json::json!({"type":"error","id":id,"message":"already attached or document exceeds size limit"})).await;
            }
            let result = async {
                let api = SessionApi::new(&endpoint)?;
                let owner = api.create_room().await?;
                let session =
                    Client::attach(&endpoint, &owner.room_id, &document, owner.token(), &text)
                        .await?;
                let backup = prepare_recovery(recovery, &session)?;
                Ok::<_, String>((owner.room_id.clone(), session, backup))
            }
            .await;
            match result {
                Ok((room_id, session, saved)) => {
                    let path = saved
                        .as_ref()
                        .map(|s| s.path().to_string_lossy().into_owned());
                    emit(out, serde_json::json!({"type":"created","id":id,"room_id":room_id,"text":session.text(),"pending":0,"recovery_path":path})).await?;
                    *backup = saved;
                    *client = Some(session);
                }
                Err(message) => {
                    emit(
                        out,
                        serde_json::json!({"type":"error","id":id,"message":message}),
                    )
                    .await?
                }
            }
        }
        Command::Join {
            id,
            endpoint,
            room,
            document,
            invite,
            text,
            recovery,
        } => {
            let invite = Zeroizing::new(invite);
            if client.is_some() || text.len() > MAX_EDITOR_BYTES {
                return emit(out, serde_json::json!({"type":"error","id":id,"message":"already attached or document exceeds size limit"})).await;
            }
            let result = async {
                let api = SessionApi::new(&endpoint)?;
                let member = api.join_room(&room, invite.as_str()).await?;
                let session =
                    Client::attach(&endpoint, &room, &document, member.token(), &text).await?;
                let backup = prepare_recovery(recovery, &session)?;
                Ok::<_, String>((session, backup))
            }
            .await;
            match result {
                Ok((session, saved)) => {
                    let path = saved
                        .as_ref()
                        .map(|s| s.path().to_string_lossy().into_owned());
                    emit(out, serde_json::json!({"type":"joined","id":id,"room_id":room,"text":session.text(),"pending":0,"recovery_path":path})).await?;
                    *backup = saved;
                    *client = Some(session);
                }
                Err(message) => {
                    emit(
                        out,
                        serde_json::json!({"type":"error","id":id,"message":message}),
                    )
                    .await?
                }
            }
        }
        Command::Invite {
            id,
            role,
            ttl_seconds,
        } => {
            let Some(session) = client.as_ref() else {
                return emit(
                    out,
                    serde_json::json!({"type":"error","id":id,"message":"not attached"}),
                )
                .await;
            };
            match session.create_invite(&role, ttl_seconds).await {
                Ok(invite) => {
                    emit(
                        out,
                        serde_json::json!({"type":"invite","id":id,"token":invite.token()}),
                    )
                    .await?
                }
                Err(message) => {
                    emit(
                        out,
                        serde_json::json!({"type":"error","id":id,"message":message}),
                    )
                    .await?
                }
            }
        }
        Command::Attach {
            id,
            endpoint,
            room,
            document,
            token,
            text,
            recovery,
        } => {
            let token = Zeroizing::new(token);
            let result = if client.is_some() {
                Err("already attached".to_owned())
            } else if text.len() > MAX_EDITOR_BYTES {
                Err("document exceeds experimental size limit".to_owned())
            } else {
                match Client::attach(&endpoint, &room, &document, &token, &text).await {
                    Ok(session) => {
                        prepare_recovery(recovery, &session).map(|saved| (session, saved))
                    }
                    Err(error) => Err(error),
                }
            };
            match result {
                Ok((session, saved)) => {
                    let path = saved
                        .as_ref()
                        .map(|s| s.path().to_string_lossy().into_owned());
                    emit(out, serde_json::json!({"type":"attached","id":id,"room_id":room,"text":session.text(),"pending":0,"recovery_path":path})).await?;
                    *backup = saved;
                    *client = Some(session);
                }
                Err(message) => {
                    emit(
                        out,
                        serde_json::json!({"type":"error","id":id,"message":message}),
                    )
                    .await?
                }
            }
        }
        Command::Edit {
            id,
            from,
            to,
            insert,
        } => {
            let Some(session) = client.as_mut() else {
                return emit(
                    out,
                    serde_json::json!({"type":"error","id":id,"message":"not attached"}),
                )
                .await;
            };
            // Conservative bound: don't accept edits if their worst-case size exceeds the proof limit.
            if session.text().len().saturating_add(insert.len()) > MAX_EDITOR_BYTES {
                return emit(out, serde_json::json!({"type":"error","id":id,"message":"document exceeds experimental size limit"})).await;
            }
            match session.edit(from, to, &insert) {
                Ok(()) => {
                    if let Some(saved) = backup.as_ref() {
                        if saved.save(&session.text()).is_err() {
                            emit(out, serde_json::json!({"type":"error","id":id,"message":"Falha ao salvar recuperação; copie o texto antes de fechar."})).await?;
                            return Err(std::io::Error::other("recovery write failed"));
                        }
                    }
                    emit(out, serde_json::json!({"type":"edited","id":id,"text":session.text(),"pending":session.pending_count()})).await?
                }
                Err(message) => {
                    emit(
                        out,
                        serde_json::json!({"type":"error","id":id,"message":message}),
                    )
                    .await?
                }
            }
        }
        Command::Disconnect { id } => {
            if client.as_ref().is_some_and(|s| s.pending_count() != 0) {
                emit(
                    out,
                    serde_json::json!({"type":"disconnect_blocked","id":id,"pending":client.as_ref().map_or(0, Client::pending_count)}),
                )
                .await?;
            } else {
                *client = None;
                *backup = None;
                emit(out, serde_json::json!({"type":"disconnected","id":id})).await?;
            }
        }
    }
    Ok(())
}

#[cfg(unix)]
fn prepare_recovery(enabled: bool, client: &Client) -> Result<Option<Recovery>, String> {
    if !enabled {
        return Ok(None);
    }
    let directory = default_directory()
        .map_err(|_| "não foi possível abrir diretório privado de recuperação".to_owned())?;
    Recovery::new(&directory, &client.text())
        .map(Some)
        .map_err(|_| "não foi possível salvar cópia de recuperação".to_owned())
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    #[cfg(unix)]
    let _terminal = TerminalGuard::new()?;
    #[cfg(not(unix))]
    return Err(std::io::Error::other(
        "PTY support has not been validated on this platform",
    ));
    #[cfg(unix)]
    run().await
}

#[cfg(unix)]
async fn run() -> std::io::Result<()> {
    let mut lines = FramedRead::new(
        tokio::io::stdin(),
        LinesCodec::new_with_max_length(MAX_LINE),
    );
    let mut out = tokio::io::stdout();
    let mut session: Option<Client> = None;
    let mut backup: Option<Recovery> = None;
    let mut retry_at = Instant::now();
    emit(&mut out, serde_json::json!({"type":"ready","protocol":1})).await?;
    loop {
        enum Next {
            Input(Option<Result<String, tokio_util::codec::LinesCodecError>>),
            Background(Result<(bool, bool), String>),
        }
        let next = if let Some(client) = session.as_mut() {
            let pending = client.pending_count() != 0;
            tokio::select! {
                line = lines.next() => Next::Input(line),
                result = async {
                    tokio::time::sleep_until(retry_at).await;
                    if pending { client.sync().await.map(|_| (true, true)) }
                    else { client.wait_for_change().await.map(|changed| (changed, false)) }
                } => Next::Background(result),
            }
        } else {
            Next::Input(lines.next().await)
        };
        match next {
            Next::Input(Some(Ok(mut line))) => {
                let parsed = serde_json::from_str::<Command>(&line);
                line.zeroize();
                match parsed {
                    Ok(cmd) => handle(cmd, &mut session, &mut backup, &mut out).await?,
                    Err(_) => {
                        emit(
                            &mut out,
                            serde_json::json!({"type":"error","message":"invalid command"}),
                        )
                        .await?
                    }
                }
            }
            Next::Input(Some(Err(_))) => {
                return Err(std::io::Error::other("input frame too large or invalid"))
            }
            Next::Input(None) => {
                if session.as_ref().is_some_and(|s| s.pending_count() != 0) {
                    return Err(std::io::Error::other("connection closed with unsent edits"));
                }
                return Ok(());
            }
            Next::Background(Ok((changed, sent))) => {
                retry_at = Instant::now();
                if changed {
                    let client = session.as_ref().expect("session exists");
                    if let Some(saved) = backup.as_ref() {
                        if saved.save(&client.text()).is_err() {
                            emit(&mut out, serde_json::json!({"type":"error","message":"Falha ao salvar recuperação; copie o texto antes de fechar."})).await?;
                            return Err(std::io::Error::other("recovery write failed"));
                        }
                    }
                    emit(&mut out, serde_json::json!({"type":if sent {"synced"} else {"remote"},"text":client.text(),"pending":client.pending_count()})).await?;
                }
            }
            Next::Background(Err(message)) => {
                retry_at = Instant::now() + Duration::from_secs(1);
                emit(&mut out, serde_json::json!({"type":"connection_error","message":message,"pending":session.as_ref().map_or(0, Client::pending_count)})).await?;
            }
        }
    }
}
