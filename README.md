<div align="center">
  <h1>Orbolay</h1>
  <p>A small native Discord voice overlay for modded clients on Linux</p>
</div>

This is a hard fork of [SpikeHD/Orbolay](https://github.com/SpikeHD/Orbolay). It supports Linux only
and talks to Discord exclusively through the `OrbolayBridge` client plugin over a local websocket;
the official-client IPC mode, Windows and macOS support, and the update checker were removed.

# Features

* Voice channel members and their state (speaking, muted, deafened, streaming)
* Message notifications that jump to the message when clicked
* Mute, deafen, disconnect, and stop-stream controls
* Per-user volume (right-click a member)
* Soundboard
* Fully customizable using HTML and CSS, without a webview ([TEMPLATES.md](./TEMPLATES.md))

# Requirements

* A Discord client running the `OrbolayBridge` plugin from this fork's companion repository
  (`equibop-plugin-orbolay-bridge`), which speaks the protocol in
  [`crates/orbolay-core/src/protocol.rs`](crates/orbolay-core/src/protocol.rs)
* X11 or Wayland through XWayland (the default; see "Run with XWayland" in the settings)
* For the built-in keybind, membership in the `input` group. Orbolay reads keyboards without
  grabbing them and only opens devices that can send the bound keys while keybinds are enabled.

# Usage

1. Start your client with the bridge plugin enabled.
2. Run `orbolay`. The bridge connects to `127.0.0.1:6888` by default.
3. Press <kbd>Ctrl</kbd> + <kbd>`</kbd> to open the overlay and use the voice controls.

On first run the settings window opens. Open it later with the cog in the overlay or
`orbolay --config`. Settings live in `~/.config/orbolay/config.json` and reload when the file
changes.

## Command-line controls

These commands talk to the running instance, so you can bind them in your desktop's shortcut
settings instead of using the built-in keybind (disable "Enable Keybind" if you do):

```sh
orbolay --toggle-overlay
orbolay --mute
orbolay --deafen
```

`orbolay --port <port>` overrides the configured websocket port.

## Wrapping a game

Orbolay can launch a program and exit with it. In Steam launch options:

```sh
orbolay -- %command%
```

## Displays

The Display setting saves the connector name (for example `"displayName": "DP-1"`), so monitor
enumeration changes do not move the overlay. If the named display is missing, Orbolay uses the
primary display. Configs that only have the older `displayIdx` keep working until a display is
selected again.

## Logging

Orbolay logs its own crates at `info` and dependencies at `warn` to stderr. Set `RUST_LOG` to
change that, for example `RUST_LOG=orbolay::bridge=debug` to log every bridge command.

## Hyprland

Hyprland needs window rules for the overlay (from upstream issue
[#28](https://github.com/SpikeHD/Orbolay/issues/28)); with them the clickable controls do not work:

```
hl.window_rule({
    match = {title = "^(orbolay)$"},
    no_initial_focus = true,
    suppress_event = "activatefocus",
    float = true,
    pin = true,
    center = true,
    no_blur = true,
    no_dim = true,
    no_follow_mouse = true,
    no_shadow = true,
    border_size = 0,
    no_focus = true,
    move = {"monitor_w", "monitor_h"},
    size = {"monitor_w - 5", "monitor_h - 5"}
})
```

# Building

The repository pins the stable toolchain in `rust-toolchain.toml`.

```sh
cargo build --release
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

The binary is `target/release/orbolay`.

# Credits

* [SpikeHD](https://github.com/SpikeHD) and the upstream Orbolay contributors
* [Freya](https://github.com/marc2332/freya), the GUI library
* [docs.discord.food](https://docs.discord.food/), a reference for undocumented Discord APIs
* [whynothugo/twemoji.ttf](https://git.sr.ht/~whynothugo/twemoji.ttf), a CBDT Twemoji font
