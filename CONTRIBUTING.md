# Development

Run `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, and
`cargo test --workspace` before committing; CI runs all three. The workspace enables
`clippy::pedantic` in `Cargo.toml`, and an `#[expect]` with a `reason` is the way to accept a
specific lint. Freya's [system libraries](https://docs.rs/freya/latest/freya/_docs/development_setup/)
are required to build.

# Architecture

Orbolay draws a fullscreen, transparent, always-on-top window and makes it click-through while
the overlay is closed (with an X11 input shape, since `set_cursor_hittest` is not enough on X11).

## Crates

| Crate | Owns |
| --- | --- |
| `orbolay-core` | Config file format, the bridge protocol, and `AppState` with its transitions |
| `orbolay-keys` | Key names stored in the config and matching held keys against binds |
| `orbolay-ui` | Freya components, HTML templates, and the settings window |
| `orbolay` (root) | Startup and window setup, plus the threads that feed the state: the bridge websocket server (`bridge.rs`), the config file watcher, the control socket behind `--toggle-overlay`, `--mute`, and `--deafen` (`control.rs`), and the evdev keybind listener (`keys/`) |

## State

`AppHandle` owns the shared `AppState`. Background threads change it in two ways:

* `AppHandle::apply` runs `AppState::apply` on a `BridgeEvent`. The transition is pure and returns
  effects, such as a timer that ends a speaking grace period or expires a notification; one timer
  thread fires them.
* `AppHandle::set_config` changes the config and notifies subscribers, such as the keybind listener.

Every change signals the UI through a one-slot channel, so a burst of updates produces a single
snapshot and render. State that only the UI uses, like whether the overlay or soundboard is open,
lives in the root component rather than in `AppState`.

## Bridge protocol

The `OrbolayBridge` client plugin connects to `ws://127.0.0.1:<port>`. Messages are JSON objects
tagged by `cmd`: `BridgeEvent` for client to Orbolay and `BridgeCommand` for Orbolay to client, both
in `crates/orbolay-core/src/protocol.rs`. Change both sides together; the plugin lives in its own
repository with tests for its half of the protocol.

## Keybinds

The listener reads `/dev/input` devices through evdev without grabbing them. While keybinds are
disabled its thread only waits for config changes; enabling them creates an async runtime that
reads the devices able to send the bound keys and watches for hotplugged ones.
