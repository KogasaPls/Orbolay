use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use orbolay_core::{
  app_state::{AppHandle, AppState},
  protocol::BridgeCommand,
};
use orbolay_transport::websocket::serve_listener;
use serde_json::json;
use tokio::net::TcpListener;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async, tungstenite::Message};

type Client = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

struct Harness {
  app: AppHandle,
  url: String,
}

async fn start() -> Harness {
  let (redraw_tx, _redraw_rx) = flume::bounded(1);
  let (open_config_tx, _open_config_rx) = flume::unbounded();
  let (app, commands) = AppHandle::new(AppState::default(), redraw_tx, open_config_tx);
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

  eventually(&harness.app, |state| state.voice_users.len() == 1).await;
}

#[tokio::test]
async fn commands_reach_the_connected_bridge() {
  let harness = start().await;
  let mut client = connect(&harness.url).await;
  send(&mut client, joined()).await;
  eventually(&harness.app, |state| !state.voice_users.is_empty()).await;

  harness.app.send(BridgeCommand::ToggleMute);
  assert_eq!(next_text(&mut client).await, json!({"cmd": "TOGGLE_MUTE"}));
}

#[tokio::test]
async fn disconnecting_clears_the_voice_channel() {
  let harness = start().await;
  let mut client = connect(&harness.url).await;
  send(&mut client, joined()).await;
  eventually(&harness.app, |state| !state.voice_users.is_empty()).await;

  client.close(None).await.unwrap();
  eventually(&harness.app, |state| state.voice_users.is_empty()).await;
}

#[tokio::test]
async fn a_new_connection_replaces_the_old_one() {
  let harness = start().await;
  let mut first = connect(&harness.url).await;
  send(&mut first, joined()).await;
  eventually(&harness.app, |state| !state.voice_users.is_empty()).await;

  let mut second = connect(&harness.url).await;
  eventually(&harness.app, |state| state.voice_users.is_empty()).await;
  send(&mut second, joined()).await;
  eventually(&harness.app, |state| !state.voice_users.is_empty()).await;

  harness.app.send(BridgeCommand::ToggleDeaf);
  assert_eq!(next_text(&mut second).await, json!({"cmd": "TOGGLE_DEAF"}));
}

#[tokio::test]
async fn commands_issued_while_disconnected_are_dropped() {
  let harness = start().await;
  let mut first = connect(&harness.url).await;
  send(&mut first, joined()).await;
  eventually(&harness.app, |state| !state.voice_users.is_empty()).await;
  first.close(None).await.unwrap();
  eventually(&harness.app, |state| state.voice_users.is_empty()).await;

  harness.app.send(BridgeCommand::Disconnect);
  tokio::time::sleep(Duration::from_millis(50)).await;

  let mut second = connect(&harness.url).await;
  send(&mut second, joined()).await;
  eventually(&harness.app, |state| !state.voice_users.is_empty()).await;
  harness.app.send(BridgeCommand::ToggleMute);
  assert_eq!(next_text(&mut second).await, json!({"cmd": "TOGGLE_MUTE"}));
}
