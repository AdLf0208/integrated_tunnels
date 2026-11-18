# Integrated Tunnels → Horizon (slice 1)

A Horizon port of **Integrated Tunnels 1.13.0** (`CyclopsMC/IntegratedTunnels`,
**MIT**), Minecraft 1.21.1 NeoForge, from the exact ATM10 pack jar.

**Slice 1 is the item tunnel:** an **Item Importer** part on a tunnel cable
face pulls items out of the inventory behind that face into the cable network's
item store at the jar's rate (64 items per tick), an **Item Exporter** part on
another cable face pushes them out again, and the cable's
`/integratedtunnels info` readout shows the parts, the store and the status.

What is not here: the fluid, energy, world, interface and player-simulator
parts, filters, the variable-card/aspect layer, part screens, network energy,
the multipart block-entity renderer and the two part recipes (their ingredients
are Integrated Dynamics content). See `notes.md` for the full slice boundary and
gaps, and `FINDINGS.md` for the jar inventory the slice was chosen from.

## The package (SDK layout, 2026-11-18)

One flat `.hmod` carries **both halves** — content and code — so a single file
installs the carrier block, the two part items and the network behaviour:

```text
integratedtunnels.hmod
  horizon-mod.toml    id, version, api = "0.2", side, capabilities, [native] pins
  Cargo.toml          one crate; the only dependency is horizon-mod-api (.horizon/sdk/)
  src/lib.rs          the one #[mod_api::horizon_mod] impl Hooks
  src/cable.rs        the game glue: block entity part, tick queue, network pass, commands
  src/runtime.rs      the pure logic and its 17 tests (byte-identical to 0.1.0)
  assets/ data/       the carrier block, the two part items, models, textures, lang, loot
  LICENSE, LICENSE-NOTICE.md
  uses.json           the surface the package uses (written by the build)
```

The retired `.hnmod` did not go away: it is kept under
`ports/_shared/evidence/integratedtunnels/retired-hnmod-0.1.0/`, with the old
content manifest and the built 0.1.0 content `.hmod`.

## Building and checking

`build-slot.sh` speaks the old staging tool, which no longer applies to this
layout, so the commands run **directly in the port** with `CARGO_INCREMENTAL=0`
and no `--target-dir`:

```sh
cd ports/integratedtunnels
cargo test                                    # the pure runtime: 17 passed
cargo clippy --all-targets -- -D warnings      # 0 warnings
cargo fmt --check                              # of this crate only (never --all)
rm -f Cargo.lock                               # see notes.md, trap 5
cargo horizon-mod build                        # -> target/horizon-mod/integratedtunnels.hmod
cp target/horizon-mod/integratedtunnels.hmod ./integratedtunnels.hmod
cargo horizon-mod check integratedtunnels.hmod
cargo horizon-mod check integratedtunnels.hmod --uses
cargo horizon-mod fits integratedtunnels.hmod --build nightly
```

The in-game scenario is `tools/e2e.sh`, which needs a **launcher-built** game
(compiled against this port) because native code is compiled into the client:

```sh
horizon-launcher native-build ports/integratedtunnels --instance <i> --trust-native-code
GAME=<instance>/horizon-client tools/e2e.sh
```

The jar is evidence at
`ports/_shared/licence-cache/integratedtunnels-1.21.1-neoforge-1.13.0.jar`.

**Local-only** until the maintainer approves publishing (ARCHITECTURE.md §8).
