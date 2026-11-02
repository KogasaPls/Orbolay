//! The websocket server the `OrbolayBridge` plugin connects to.

use futures_util::{SinkExt, StreamExt};
use log::{debug, info, warn};
use tokio::{net::TcpListener, task::JoinHandle};
use tokio_tungstenite::{accept_async, tungstenite::Message};

use orbolay_core::{
  app_state::AppHandle,
  payloads::Notification,
  protocol::{BridgeCommand, BridgeEvent},
};

/// Accepts bridge connections on `127.0.0.1:port`. A new connection replaces the previous one,
/// so commands always reach the most recently connected client.
pub async fn serve(app: AppHandle, port: u16, commands: flume::Receiver<BridgeCommand>) {
  let listener = match TcpListener::bind(("127.0.0.1", port)).await {
    Ok(listener) => listener,
    Err(e) => {
      log::error!("Could not listen for the bridge on port {port}: {e}");
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

pub async fn serve_listener(
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
