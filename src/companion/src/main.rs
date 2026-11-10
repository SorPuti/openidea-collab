//! WSS transport proof. This executable never reads or writes project files.
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::{io, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    sync::mpsc,
};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{Message, client::IntoClientRequest},
};

const RELAY: &str = "wss://collab.synthosia.com/v1/socket";
const SUBPROTOCOL: &str = "openidea-collab.v1";
const MAX_LINE: usize = 64 * 1024;

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
enum Input {
    Connect {
        websocket_url: String,
        credential: String,
    },
    Send {
        data_hex: String,
    },
    Close,
}
#[derive(Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
enum Output {
    Connected,
    Binary { data_hex: String },
    Closed,
    Error { message: &'static str },
}
enum WsCommand {
    Send(Vec<u8>),
    Close,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut input = BufReader::new(tokio::io::stdin()).lines();
    let mut stdout = tokio::io::stdout();
    let (event_tx, mut events) = mpsc::channel::<Output>(32);
    let mut ws_tx: Option<mpsc::Sender<WsCommand>> = None;
    loop {
        tokio::select! {
            line = input.next_line() => {
                let Some(line) = line? else { break };
                if line.len() > MAX_LINE { event_tx.send(Output::Error{message:"input_limit"}).await?; continue; }
                let cmd: Input = match serde_json::from_str(&line) { Ok(v)=>v, Err(_)=> { event_tx.send(Output::Error{message:"invalid_command"}).await?; continue; }};
                match cmd {
                    Input::Connect { websocket_url, credential } => {
                        if ws_tx.is_some() || websocket_url != RELAY || credential.len() < 32 {
                            event_tx.send(Output::Error{message:"invalid_connection_request"}).await?; continue;
                        }
                        let mut req = RELAY.into_client_request()?;
                        req.headers_mut().insert("authorization", format!("Bearer {credential}").parse()?);
                        req.headers_mut().insert("sec-websocket-protocol", SUBPROTOCOL.parse()?);
                        // Do not retain credentials or include them in error output.
                        drop(credential);
                        match tokio::time::timeout(Duration::from_secs(8), connect_async(req)).await {
                            Ok(Ok((socket,_))) => {
                                let (mut sink, mut stream)=socket.split();
                                let (tx, mut rx)=mpsc::channel::<WsCommand>(16);
                                let events_out=event_tx.clone();
                                tokio::spawn(async move {
                                    events_out.send(Output::Connected).await.ok();
                                    loop { tokio::select! {
                                        cmd=rx.recv()=>match cmd { Some(WsCommand::Send(bytes))=>if sink.send(Message::Binary(bytes.into())).await.is_err(){break}, Some(WsCommand::Close)|None=>{let _=sink.close().await;break} },
                                        msg=stream.next()=>match msg { Some(Ok(Message::Binary(bytes)))=>{if events_out.send(Output::Binary{data_hex:encode_hex(&bytes)}).await.is_err(){break}}, Some(Ok(Message::Ping(p)))=>{if sink.send(Message::Pong(p)).await.is_err(){break}}, Some(Ok(Message::Close(_)))|None|Some(Err(_))=>break, _=>{} }
                                    }}
                                    events_out.send(Output::Closed).await.ok();
                                });
                                ws_tx=Some(tx);
                            }
                            _ => { event_tx.send(Output::Error{message:"connect_failed"}).await?; }
                        }
                    }
                    Input::Send { data_hex } => {
                        let Some(tx)=ws_tx.as_ref() else { event_tx.send(Output::Error{message:"not_connected"}).await?; continue; };
                        if data_hex.len()>MAX_LINE || data_hex.len()%2!=0 { event_tx.send(Output::Error{message:"frame_limit"}).await?; continue; }
                        let Some(bytes)=decode_hex(&data_hex).filter(|b|b.len()<=MAX_LINE) else { event_tx.send(Output::Error{message:"invalid_frame"}).await?; continue; };
                        if tx.try_send(WsCommand::Send(bytes)).is_err() { event_tx.send(Output::Error{message:"send_queue_full"}).await?; ws_tx=None; }
                    }
                    Input::Close => { if let Some(tx)=ws_tx.take(){let _=tx.send(WsCommand::Close).await;} }
                }
            }
            event=events.recv()=>if let Some(event)=event { emit(&mut stdout,&event).await?; }
        }
    }
    if let Some(tx) = ws_tx {
        let _ = tx.send(WsCommand::Close).await;
    }
    Ok(())
}
async fn emit<W: AsyncWriteExt + Unpin, T: Serialize>(out: &mut W, msg: &T) -> io::Result<()> {
    let mut b = serde_json::to_vec(msg).map_err(io::Error::other)?;
    b.push(b'\n');
    out.write_all(&b).await?;
    out.flush().await
}
fn decode_hex(s: &str) -> Option<Vec<u8>> {
    s.as_bytes()
        .chunks_exact(2)
        .map(|p| Some((digit(p[0])? << 4) | digit(p[1])?))
        .collect()
}
fn digit(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}
fn encode_hex(b: &[u8]) -> String {
    const H: &[u8] = b"0123456789abcdef";
    let mut s = String::with_capacity(b.len() * 2);
    for x in b {
        s.push(H[(x >> 4) as usize] as char);
        s.push(H[(x & 15) as usize] as char)
    }
    s
}
