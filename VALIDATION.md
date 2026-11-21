# Validation record — Integrated Tunnels

One entry per delivered version, newest first. Every claim here has its log under
`ports/_shared/evidence/integratedtunnels*/`; the log's header prints the package sha256 and size it
ran against (or, for the red run, the tree it compiled).

## 0.2.3 — slice 2, the fluid tunnel

**Pin:** Horizon `0a875da91dbb0fdcb7ba8c42c11341aa229af51a`, horizon-mixins
`e59da9094f0c2fdda6c627fe9eff22dec6a906f0`, game 26.3, `api = "0.2"`.
**Artifact:** `integratedtunnels.hmod`, **49,232 B**, sha256
`1dbd235c13792d22c1478643cc79b1b4f09bf418144f74883125d576dc176fa9`, **46 entries**, no packed
`Cargo.lock`, `src/runtime.rs` identical in the tree and the package (`bf298bd7…`).

| Check | Command | Result | Log |
|---|---|---|---|
| unit tests (red first) | `cargo test --offline` | 51 compile errors before the rules existed | `evidence/integratedtunnels/sdk-test-red-fluid.log` |
| unit tests | `cargo test --offline` | **28 passed**, 0 failed | `sdk-test.log` |
| lints | `cargo clippy --all-targets -- -D warnings` | exit 0, 0 warnings | `sdk-clippy.log` |
| formatting | `cargo fmt --check` | clean | `sdk-fmt.log` |
| reproducibility | `rm -f Cargo.lock; cargo horizon-mod build` twice | `cmp` identical, same size and sha | `sdk-build.log`, `build-a.sha256`, `build-b.sha256` |
| package check | `cargo horizon-mod check integratedtunnels.hmod` | `0.2.3 (native-mod, format native-1/0a875da…/e59da90…, for 26.3)`, EXIT 0 | `sdk-check.log` |
| fits the pin | `cargo horizon-mod fits integratedtunnels.hmod --build 0a875da` | `0a875da  made for` | `sdk-fits.log` |
| fits another build | `… --build 0b14f8acfd97fa2c750e42795249dcc981178ca4` | `0b14f8a  fits` | `sdk-fits-other.log` |
| package gate | `python3 ports/tools/verify-sdk-package.py integratedtunnels` | **1/1** | — |
| in game | `launcher-game.sh build` + `tools/e2e.sh` on the built game | `Compiling integratedtunnels (1 of 1)`, `PASS` + `EXIT:0` | `evidence/integratedtunnels-e2e/e2e-launcher.log`, `native-build.log` |
| control | the same Nightly without the package (`CONTROL_GAME`) | `EXIT:1` | `…/e2e-launcher-control.log` |
| selftest | `SELFTEST=1` (phase B expects 6401) | `FAIL` + `EXIT:1` | `…/e2e-launcher-selftest.log` |
| log audit | `python3 ports/tools/check-e2e-logs.py integratedtunnels-e2e --strict` | `logs=3 ok=3 missing-exit=0 bad-exit=0` | — |
| skills gate | `python3 ports/tools/check-skills.py --notes ports/integratedtunnels` | PASS | — |

The fluid half of the game leg: a full water cauldron behind a Fluid Importer drains into the
network's fluid channel (1000 mB, cauldron empty), a Fluid Exporter fills a second cauldron from it
(channel 0 mB, sink `water_cauldron[level=3]`), and removing either part stops its half (K1/K2).
Both fluid part items are given and read back (L), and the cable's carried loot table rolls the
cable through `/loot give` (M) — which also measures that 26.3 accepts its `bonus_rolls`/`rolls`
spelling, the two shapes `check_261_dialect.py` treats as unverified.

## 0.2.1 — review fixes for registry review #1187

21 tests, clippy `-D warnings` 0, `fmt --check` clean, two builds byte-identical, `check` and `fits`
exit 0; launcher e2e `PASS` + `EXIT:0`, control `EXIT:1`. Artifact **39,418 B**, sha256
`87ab0b4b23892527a1c8ba6550a7b084d3fe058384f35ce860c5427e1639bfaa`.

## 0.2.0 — the SDK migration

17 tests, check and fits at `0b14f8a`, two builds byte-identical, `runtime.rs` byte-identical to the
retired `.hnmod`. Artifact **37,602 B**, sha256
`c72e5302004d32f13772d8ba976c75be208e947624fff312b8d60019d0695e0b`. The in-game leg was not
re-runnable at the time (no launcher); it was restored for 0.2.1.
