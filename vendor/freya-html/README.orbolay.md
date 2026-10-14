# Orbolay HTML memory fix

This crate is copied from Freya commit `db7503276e93f52932465865afa8a84a9ff1accf`,
`crates/freya-html`, under its original MIT license. Cargo.toml replaces workspace
inheritance with equivalent explicit dependencies; Cargo.lock pins the git dependencies.

Changes to the original source:

- `rustfmt.toml` and source formatting: use stable rustfmt options to match
  Orbolay's CI toolchain.
- `src/anyrender/scene.rs`: reuse native typefaces across fresh font blob identities
  and viewer remounts using exact font bytes and collection index. The thread-local
  LRU holds at most 16 fonts and 64 MiB of font bytes (one oversized font may occupy
  it alone). Variable-font clones share the native backing stream. Working sets
  exceeding the cache capacity can still recreate typefaces after eviction.
- `src/handle.rs`: add `replace_html`, which clears obsolete inline history.
  Orbolay participant and notification rows use it for template updates;
  navigation and `load_html` keep their original behavior.

Keep this override tied to the locked Freya revision. Reassess it when upgrading
Freya and remove it when equivalent upstream fixes are available.

Run `cargo test --locked -p freya-html --lib` for the native reuse and capacity
regressions. Validate RSS using isolated voice-state replay as well; the tests
alone do not prove behavior with the user's complete font/image/GPU workload.
