# Isolated HTML memory replay

Requires Node.js with WebSocket support, Xvfb (`xvfb-run`), and `pgrep`.
Set `ORBOLAY_TEST_BINARY` to the exact candidate executable. This script copies
configuration into `/tmp`, disables keybinds/notifications, uses a private display
and bridge port, and sends only synthetic participants with no avatar URLs.
It does not change the real config or touch an existing overlay/audio session.
Run tests sequentially; their private port is shared.

```sh
ORBOLAY_TEST_BINARY="$PWD/target/debug/orbolay" node tools/memory-replay.cjs --dry-run
ORBOLAY_TEST_BINARY="$PWD/target/debug/orbolay" node tools/memory-replay.cjs
ORBOLAY_TEST_BINARY="$PWD/target/debug/orbolay" node tools/memory-replay.cjs --always
ORBOLAY_TEST_BINARY="$PWD/target/debug/orbolay" node tools/memory-replay.cjs --simple-template
```

`ORBOLAY_TEST_ROOT` selects an isolated output directory;
`ORBOLAY_TEST_SECONDS` overrides duration (at least 10 seconds).
The default run is 40 seconds; controls are 20 seconds. Results/logs remain in
that directory. More than 30 MiB growth after 10-second warmup fails the replay;
a sample above 700 MiB stops it. The five-second sampling limit is not a hard
memory cap. A passing short software replay does not prove all GPU/image/font
workloads or a causal relationship with voice dropouts.

Optional `ORBOLAY_TEST_SCREENSHOT` captures only the private Orbolay X11 window
using ImageMagick's `import`; it requires ImageMagick.
