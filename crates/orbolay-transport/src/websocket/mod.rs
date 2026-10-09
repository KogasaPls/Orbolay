use orbolay_logging::{error, log, success, warn};
use serde_json::Value;
use std::collections::HashMap;
use std::net::{TcpListener, TcpStream};
use tungstenite::{Message, Utf8Bytes, accept};

use orbolay_core::{
  app_state::AppHandle,
  payloads::{ChannelJoinPayload, NotificationPayload, SoundboardSoundPayload, UpdatePayload},
  util::bridge::BridgeMessage,
};

pub fn create_websocket(
  port: u16,
  app: AppHandle,
  transport_recv: flume::Receiver<BridgeMessage>,
) -> Result<(), Box<dyn std::error::Error>> {
  let server = TcpListener::bind(format!("127.0.0.1:{port}"))?;
  success!("Websocket server started on port {}", port);

  for stream in server.incoming() {
    match stream {
      Ok(stream) => {
        log!("Accepted connection");

        let recv = transport_recv.clone();
        let app_clone = app.clone();
        std::thread::spawn(move || match ws_stream(stream, app_clone, recv) {
          Ok(_) => {
            success!("Websocket stream closed");
          }
          Err(e) => {
            error!("Error in websocket stream: {}", e);
          }
        });
      }
      Err(e) => {
        warn!("Failed to accept connection: {}", e);
      }
    }
  }

  warn!("Server stopped");

  Ok(())
}

fn ws_stream(
  stream: TcpStream,
  app: AppHandle,
  transport_recv: flume::Receiver<BridgeMessage>,
) -> Result<(), Box<dyn std::error::Error>> {
  let mut websocket = accept(stream)?;

  websocket.get_mut().set_nonblocking(true)?;

  log!("Stream connected");

  'outer: loop {
    loop {
      match websocket.read() {
        Ok(msg) if msg.is_close() => {
          log!("Stream closed");
          app.update(|state| state.voice_users = vec![]);
          break 'outer;
        }
        Ok(msg) if msg.is_empty() => continue,
        Ok(msg) => {
          let msg = msg.to_string();
          let msg: BridgeMessage = serde_json::from_str(&msg)?;
          if let Err(e) = handle_ws_message(&msg, app.clone()) {
            error!("Failed to handle websocket message: {}", e);
          }
        }
        Err(_) => break,
      }
    }

    if let Ok(msg) = transport_recv.try_recv() {
      let msg = serde_json::to_string(&msg)?;
      log!("Sending message to websocket: {:?}", msg);
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

pub fn handle_ws_message(
  msg: &BridgeMessage,
  app: AppHandle,
) -> Result<(), Box<dyn std::error::Error>> {
  log!("Received message: {:?}", msg);

  let data = msg.data.clone();

  match msg.cmd.as_str() {
    "REGISTER_CONFIG" => {
      let user_id = msg
        .data
        .get("userId")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
      let port = msg
        .data
        .get("port")
        .and_then(|v| v.as_u64())
        .map(|v| v as u16);
      app.update(|state| {
        if let Some(port) = port {
          state.config.port = Some(port);
        }
        state.user_id = user_id;
      });
    }
    "CHANNEL_JOINED" => {
      let data = serde_json::from_value::<ChannelJoinPayload>(data)?;
      app.update(|state| {
        let user_id = state.user_id.clone();
        let mut users = Vec::with_capacity(data.states.len());

        for voice_state in data.states {
          if voice_state.user_id == user_id {
            state.current_channel = voice_state.channel_id.clone().unwrap_or("0".to_string());
          }
          users.push(voice_state.into());
        }

        state.voice_users = users;
      });
    }
    "VOICE_STATE_UPDATE" => {
      let channel_is_null = msg
        .data
        .get("state")
        .and_then(|v| v.get("channelId"))
        .map(|v| v.is_null())
        .unwrap_or(false);
      let mut data = serde_json::from_value::<UpdatePayload>(msg.data.clone())?;

      app.update(|state| {
        let should_remove = match &data.state.channel_id {
          Some(channel_id) => channel_id != &state.current_channel,
          None => channel_is_null,
        };

        if should_remove {
          state
            .voice_users
            .retain(|user| user.id != data.state.user_id);
        } else {
          let user = state
            .voice_users
            .iter_mut()
            .find(|user| user.id == data.state.user_id);

          if let Some(user) = user {
            if data.state.streaming.is_none() {
              data.state.streaming = Some(user.streaming);
            }
            if let Some(epoch) = user.set_voice_state(data.state.clone().into()) {
              app.end_linger_later(user.id.clone(), epoch);
            }
            user.streaming = data.state.streaming.unwrap_or_default();
          } else {
            state.voice_users.push(data.state.into());
          }
        }
      });
    }
    "CHANNEL_LEFT" => {
      app.update(|state| {
        state.voice_users = vec![];
        state.current_channel = String::new();
      });
    }
    "MESSAGE_NOTIFICATION" => {
      let mut data = serde_json::from_value::<NotificationPayload>(data)?;
      data.message.timestamp = Some(chrono::Utc::now().timestamp());
      data.message.icon = data.message.icon.replace(".webp", ".png");
      app.notify(data.message);
    }
    "STREAMER_MODE" => {
      let is_censor = msg
        .data
        .get("enabled")
        .unwrap_or(&Value::from(false))
        .as_bool()
        .unwrap_or_default();
      app.update(|state| state.is_censor = is_censor);
    }
    "SOUNDBOARD_UPDATE" => {
      let sounds = match data.get("sounds") {
        Some(sounds) => serde_json::from_value::<Vec<SoundboardSoundPayload>>(sounds.clone())?,
        None => serde_json::from_value::<Vec<SoundboardSoundPayload>>(data)?,
      };
      let mut by_guild: HashMap<String, Vec<_>> = HashMap::new();
      for sound in sounds {
        by_guild
          .entry(sound.guild_id.clone().unwrap_or_default())
          .or_default()
          .push(sound);
      }
      app.update(|state| {
        for (guild_id, sounds) in by_guild {
          state.soundboard_cache.insert(guild_id, sounds);
        }
      });
    }
    _ => {
      warn!("Unknown command: {}", msg.cmd);
    }
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use std::{
    sync::{Arc, RwLock},
    time::Duration,
  };

  use orbolay_core::{
    app_state::AppState,
    user::{User, UserVoiceState},
  };
  use serde_json::json;

  use super::*;

  fn app_with_user(linger_secs: f32) -> AppHandle {
    let mut state = AppState::new();
    state.config.voice_linger_secs = linger_secs;
    state.current_channel = "c".into();
    state.voice_users = vec![User {
      name: "user".into(),
      id: "u".into(),
      avatar: String::new(),
      voice_state: UserVoiceState::NotSpeaking,
      streaming: false,
      camera: false,
      volume: 100.,
      speaking_epoch: 0,
      lingering: false,
    }];
    let (redraw_tx, _redraw_rx) = flume::unbounded();
    AppHandle::new(Arc::new(RwLock::new(state)), redraw_tx)
  }

  fn speaking(app: &AppHandle, speaking: bool) {
    let msg = BridgeMessage {
      cmd: "VOICE_STATE_UPDATE".into(),
      data: json!({ "state": { "userId": "u", "channelId": "c", "speaking": speaking } }),
    };
    handle_ws_message(&msg, app.clone()).unwrap();
  }

  fn user_state(app: &AppHandle) -> (UserVoiceState, bool) {
    app.read(|state| {
      let user = &state.voice_users[0];
      (user.voice_state.clone(), user.lingering)
    })
  }

  #[test]
  fn user_lingers_after_speaking() {
    let app = app_with_user(0.1);

    speaking(&app, true);
    assert_eq!(user_state(&app), (UserVoiceState::Speaking, false));

    speaking(&app, false);
    assert_eq!(user_state(&app), (UserVoiceState::NotSpeaking, true));

    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(user_state(&app), (UserVoiceState::NotSpeaking, false));
  }

  #[test]
  fn speaking_again_restarts_the_linger() {
    let app = app_with_user(0.15);

    speaking(&app, true);
    speaking(&app, false);
    std::thread::sleep(Duration::from_millis(75));
    speaking(&app, true);
    speaking(&app, false);

    // The first stop's timer has fired by now, but must not end the second linger
    std::thread::sleep(Duration::from_millis(125));
    assert_eq!(user_state(&app), (UserVoiceState::NotSpeaking, true));

    std::thread::sleep(Duration::from_millis(150));
    assert_eq!(user_state(&app), (UserVoiceState::NotSpeaking, false));
  }
}
