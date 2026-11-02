//! The websocket server the `OrbolayBridge` plugin connects to.

use futures_util::{SinkExt, StreamExt};
use log::{debug, error, info, warn};
use orbolay_core::{
  app_state::AppHandle,
  protocol::{BridgeCommand, BridgeEvent, Notification},
};
use tokio::{net::TcpListener, task::JoinHandle};
use tokio_tungstenite::{accept_async, tungstenite::Message};

/// Accepts bridge connections on `127.0.0.1:port`. A new connection replaces the previous one,
/// so commands always reach the most recently connected client.
async fn serve(app: AppHandle, port: u16, commands: flume::Receiver<BridgeCommand>) {
  let listener = match TcpListener::bind(("127.0.0.1", port)).await {
    Ok(listener) => listener,
    Err(e) => {
      error!("Could not listen for the bridge on port {port}: {e}");
      app.notify(Notification {
        title: "Orbolay cannot reach Discord".into(),
        body: format!("Port {port} is unavailable: {e}"),
        ..Notification::default()
      });
      return;
    }
  };
  info!("Websocket server started on port {port}");
  serve_listener(app, listener, commands).await;
}

async fn serve_listener(
  app: AppHandle,
  listener: TcpListener,
  commands: flume::Receiver<BridgeCommand>,
) {
  let mut current: Option<JoinHandle<()>> = None;
  loop {
    let connected = current.as_ref().is_some_and(|task| !task.is_finished());
    tokio::select! {
      accepted = listener.accept() => match accepted {
        Ok((stream, peer)) => {
          info!("Bridge connected from {peer}");
          if let Some(previous) = current.take().filter(|task| !task.is_finished()) {
            previous.abort();
            app.apply(BridgeEvent::ChannelLeft);
          }
          current = Some(tokio::spawn(connection(app.clone(), stream, commands.clone())));
        }
        Err(e) => warn!("Failed to accept a bridge connection: {e}"),
      },
      () = async { if let Some(task) = current.as_mut() { let _ = task.await; } }, if connected => {
        current = None;
      }
      command = commands.recv_async(), if !connected => match command {
        Ok(command) => debug!("No bridge connected; dropping {command:?}"),
        Err(_) => return,
      },
    }
  }
}

async fn connection(
  app: AppHandle,
  stream: tokio::net::TcpStream,
  commands: flume::Receiver<BridgeCommand>,
) {
  let socket = match accept_async(stream).await {
    Ok(socket) => socket,
    Err(e) => {
      warn!("Bridge handshake failed: {e}");
      return;
    }
  };
  let (mut sink, mut source) = socket.split();

  loop {
    tokio::select! {
      frame = source.next() => match frame {
        Some(Ok(Message::Text(text))) => match serde_json::from_str::<BridgeEvent>(&text) {
          Ok(event) => app.apply(event),
          Err(e) => warn!("Ignoring bridge message: {e}"),
        },
        Some(Ok(Message::Close(_))) | None => break,
        Some(Ok(_)) => {}
        Some(Err(e)) => {
          warn!("Bridge connection failed: {e}");
          break;
        }
      },
      command = commands.recv_async() => {
        let Ok(command) = command else { break };
        let json = serde_json::to_string(&command).expect("commands serialize to JSON");
        debug!("Sending to bridge: {json}");
        if let Err(e) = sink.send(Message::text(json)).await {
          warn!("Failed to send to the bridge: {e}");
          break;
        }
      }
    }
  }

  info!("Bridge disconnected");
  app.apply(BridgeEvent::ChannelLeft);
}

/// Runs the bridge server on its own thread with a single-threaded async runtime.
pub fn start(app: AppHandle, port: u16, commands: flume::Receiver<BridgeCommand>) {
  let spawned = std::thread::Builder::new()
    .name("orbolay-bridge".into())
    .spawn(move || {
      match tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .build()
      {
        Ok(runtime) => runtime.block_on(serve(app, port, commands)),
        Err(e) => error!("Could not start the bridge runtime: {e}"),
      }
    });
  if let Err(e) = spawned {
    error!("Could not start the bridge thread: {e}");
  }
}

#[cfg(test)]
mod tests {
  use std::time::Duration;

  use futures_util::{SinkExt, StreamExt};
  use orbolay_core::{
    app_state::{AppHandle, AppState},
    protocol::BridgeCommand,
  };
  use serde_json::json;
  use tokio::net::TcpListener;
  use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async, tungstenite::Message};

  use super::serve_listener;

  type Client = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

  struct Harness {
    app: AppHandle,
    url: String,
  }

  async fn start() -> Harness {
    let (redraw_tx, _redraw_rx) = flume::bounded(1);
    let (app, commands) = AppHandle::new(AppState::default(), redraw_tx);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}", listener.local_addr().unwrap());
    tokio::spawn(serve_listener(app.clone(), listener, commands));
    Harness { app, url }
  }

  async fn connect(url: &str) -> Client {
    connect_async(url).await.unwrap().0
  }

  async fn send(client: &mut Client, value: serde_json::Value) {
    client.send(Message::text(value.to_string())).await.unwrap();
  }

  async fn eventually(app: &AppHandle, condition: impl Fn(&AppState) -> bool) {
    for _ in 0..200 {
      if app.read(&condition) {
        return;
      }
      tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("state never satisfied the condition");
  }

  async fn next_text(client: &mut Client) -> serde_json::Value {
    let frame = tokio::time::timeout(Duration::from_secs(2), client.next())
      .await
      .expect("no frame within 2 s")
      .unwrap()
      .unwrap();
    serde_json::from_str(frame.to_text().unwrap()).unwrap()
  }

  fn joined() -> serde_json::Value {
    json!({
      "cmd": "CHANNEL_JOINED",
      "channelId": "voice",
    "states": [{ "userId": "peer", "username": "Peer", "channelId": "voice" }]
    })
  }

  #[tokio::test]
  async fn events_update_state_and_bad_frames_are_skipped() {
    let harness = start().await;
    let mut client = connect(&harness.url).await;

    client.send(Message::text("not json")).await.unwrap();
    send(&mut client, json!({"cmd": "UNKNOWN"})).await;
    send(&mut client, joined()).await;

    eventually(&harness.app, |state| state.members().len() == 1).await;
  }

  #[tokio::test]
  async fn commands_reach_the_connected_bridge() {
    let harness = start().await;
    let mut client = connect(&harness.url).await;
    send(&mut client, joined()).await;
    eventually(&harness.app, |state| !state.members().is_empty()).await;

    harness.app.send(BridgeCommand::ToggleMute);
    assert_eq!(next_text(&mut client).await, json!({"cmd": "TOGGLE_MUTE"}));
  }

  #[tokio::test]
  async fn disconnecting_clears_the_voice_channel() {
    let harness = start().await;
    let mut client = connect(&harness.url).await;
    send(&mut client, joined()).await;
    eventually(&harness.app, |state| !state.members().is_empty()).await;

    client.close(None).await.unwrap();
    eventually(&harness.app, |state| state.members().is_empty()).await;
  }

  #[tokio::test]
  async fn a_new_connection_replaces_the_old_one() {
    let harness = start().await;
    let mut first = connect(&harness.url).await;
    send(&mut first, joined()).await;
    eventually(&harness.app, |state| !state.members().is_empty()).await;

    let mut second = connect(&harness.url).await;
    eventually(&harness.app, |state| state.members().is_empty()).await;
    send(&mut second, joined()).await;
    eventually(&harness.app, |state| !state.members().is_empty()).await;

    harness.app.send(BridgeCommand::ToggleDeaf);
    assert_eq!(next_text(&mut second).await, json!({"cmd": "TOGGLE_DEAF"}));
  }

  #[tokio::test]
  async fn commands_issued_while_disconnected_are_dropped() {
    let harness = start().await;
    let mut first = connect(&harness.url).await;
    send(&mut first, joined()).await;
    eventually(&harness.app, |state| !state.members().is_empty()).await;
    first.close(None).await.unwrap();
    eventually(&harness.app, |state| state.members().is_empty()).await;

    harness.app.send(BridgeCommand::Disconnect);
    tokio::time::sleep(Duration::from_millis(50)).await;

    let mut second = connect(&harness.url).await;
    send(&mut second, joined()).await;
    eventually(&harness.app, |state| !state.members().is_empty()).await;
    harness.app.send(BridgeCommand::ToggleMute);
    assert_eq!(next_text(&mut second).await, json!({"cmd": "TOGGLE_MUTE"}));
  }
}
