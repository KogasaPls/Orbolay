use log::error;
use orbolay_core::{app_state::AppHandle, util::bridge::BridgeMessage};

use crate::websocket;

pub fn create_transport_thread(
  app: AppHandle,
  port: u16,
  transport_recv: flume::Receiver<BridgeMessage>,
) {
  std::thread::spawn(move || {
    if let Err(e) = websocket::create_websocket(port, &app, &transport_recv) {
      error!("Websocket server failed on port {port}: {e}");
    }
  });
}
