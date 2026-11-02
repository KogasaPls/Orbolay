use log::error;
use orbolay_core::{app_state::AppHandle, protocol::BridgeCommand};

use crate::websocket;

/// Runs the bridge server on its own thread with a single-threaded async runtime.
pub fn create_transport_thread(
  app: AppHandle,
  port: u16,
  commands: flume::Receiver<BridgeCommand>,
) {
  let spawned = std::thread::Builder::new()
    .name("orbolay-bridge".into())
    .spawn(move || {
      match tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .build()
      {
        Ok(runtime) => runtime.block_on(websocket::serve(app, port, commands)),
        Err(e) => error!("Could not start the bridge runtime: {e}"),
      }
    });
  if let Err(e) = spawned {
    error!("Could not start the bridge thread: {e}");
  }
}
