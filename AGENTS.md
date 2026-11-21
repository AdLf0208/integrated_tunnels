# Working on this native Horizon mod

This port uses the **SDK layout** of 2026-11-18 (Horizon's mod-API merge, launcher ≥ 0.2.70): one flat
`.hmod` carries the content (`assets/`, `data/`) and the code (`src/`), and there is no
`mixins/<id>/`, no `horizon-mixin.toml`, no `horizon-native.json`, no plan/injection schema and no
`rust-toolchain.toml`. Read the campaign's `../_shared/AGENT_MOD_STRUCTURE.md` **§0** and
`../_shared/sdk/README.md` before editing.

```text
horizon-mod.toml   the only manifest and the only identity (id/name/version/api/side/capabilities + [native])
Cargo.toml         one crate; its only dependency is horizon-mod-api from .horizon/sdk/ (fetched, never packed)
src/lib.rs         the one #[mod_api::horizon_mod] impl Hooks
src/cable.rs       the game glue (block entity part, tick queue, network pass, /integratedtunnels)
src/runtime.rs     the pure logic and its tests (slice 1 + the fluid rules of slice 2)
assets/ data/      the carried content
tools/e2e.sh       the in-game scenario (needs a launcher-built game)
```

Rules that bite here:

- **Every decision lives in `src/runtime.rs`, with its test.** Slices 0.2.0/0.2.1 shipped it
  byte-identical to the retired `.hnmod`; slice 2 (0.2.2) added the fluid parts, the fluid channel and
  the per-channel status to it, so that byte-identity is history, not a rule — the rule is that game
  access stays in `cable.rs` and the pure rules are unit-tested here. Keep the packaged copy identical
  to the tree (the gate checks it).
- **The SDK is the gate**: `cargo horizon-mod build|check|fits` and `cargo test` run directly in this
  port (`CARGO_INCREMENTAL=0`, no `--target-dir`). `.horizon/` and `Cargo.lock` are gitignored, and
  `Cargo.lock` must not be packed (it changes the artifact).
- **Never edit the fetched SDK** in `.horizon/sdk/`, and never run `cargo fmt --all` (it formats the
  SDK's crates).
- **Only stamped surface**: hooks, host methods and handles that exist in
  `.horizon/sdk/api/crates/horizon-mod-vocab/generated/` — read the generated trait, do not guess.
- Keep the package reproducible (same size and SHA-256 on a rebuild) and record the hash in
  `notes.md`.
