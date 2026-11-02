use log::{debug, error, info, warn};
use std::net::{TcpListener, TcpStream};
use tungstenite::{Message, Utf8Bytes, accept};

use orbolay_core::{
  app_state::AppHandle,
  protocol::{BridgeCommand, BridgeEvent},
};

pub fn create_websocket(
  port: u16,
  app: &AppHandle,
  transport_recv: &flume::Receiver<BridgeCommand>,
) -> Result<(), Box<dyn std::error::Error>> {
  let server = TcpListener::bind(format!("127.0.0.1:{port}"))?;
  info!("Websocket server started on port {port}");

  for stream in server.incoming() {
    match stream {
      Ok(stream) => {
        info!("Accepted connection");

        let recv = transport_recv.clone();
        let app_clone = app.clone();
        std::thread::spawn(move || match ws_stream(stream, &app_clone, &recv) {
          Ok(()) => {
            info!("Websocket stream closed");
          }
          Err(e) => {
            error!("Error in websocket stream: {e}");
          }
        });
      }
      Err(e) => {
        warn!("Failed to accept connection: {e}");
      }
    }
  }

  warn!("Server stopped");

  Ok(())
}

fn ws_stream(
  stream: TcpStream,
  app: &AppHandle,
  transport_recv: &flume::Receiver<BridgeCommand>,
) -> Result<(), Box<dyn std::error::Error>> {
  let mut websocket = accept(stream)?;

  websocket.get_mut().set_nonblocking(true)?;

  info!("Stream connected");

  'outer: loop {
    loop {
      match websocket.read() {
        Ok(msg) if msg.is_close() => {
          info!("Stream closed");
          app.apply(BridgeEvent::ChannelLeft);
          break 'outer;
        }
        Ok(msg) if msg.is_empty() => {}
        Ok(msg) => match serde_json::from_str::<BridgeEvent>(&msg.to_string()) {
          Ok(event) => app.apply(event),
          Err(e) => warn!("Ignoring bridge message: {e}"),
        },
        Err(tungstenite::Error::Io(ref e)) if e.kind() == std::io::ErrorKind::WouldBlock => break,
        Err(e) => {
          // socket is dead, exit so this thread stops draining messages.
          error!("Websocket read error: {e}");
          app.apply(BridgeEvent::ChannelLeft);
          break 'outer;
        }
      }
    }

    if let Ok(msg) = transport_recv.try_recv() {
      let msg = serde_json::to_string(&msg)?;
      debug!("Sending message to websocket: {msg:?}");
      websocket
        .write(Message::Text(Utf8Bytes::from(msg)))
        .unwrap_or_else(|_| error!("Failed to send message to websocket, socket closed?"));
      websocket
        .flush()
        .unwrap_or_else(|_| error!("Failed to flush message to websocket, socket closed?"));
    }

    std::thread::sleep(std::time::Duration::from_millis(10));
  }
  Ok(())
}
