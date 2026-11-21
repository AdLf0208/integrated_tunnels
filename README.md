# Integrated Tunnels → Horizon (slices 1–2)

A Horizon port of **Integrated Tunnels 1.13.0** (`CyclopsMC/IntegratedTunnels`,
**MIT**), Minecraft 1.21.1 NeoForge, from the exact ATM10 pack jar.

**Slice 1 is the item tunnel:** an **Item Importer** part on a tunnel cable
face pulls items out of the inventory behind that face into the cable network's
item store at the jar's rate (64 items per tick), an **Item Exporter** part on
another cable face pushes them out again, and the cable's
`/integratedtunnels info` readout shows the parts, the store and the status.

**Slice 2 is the fluid tunnel:** a **Fluid Importer** part drains the tank behind
its face (a cauldron answers as one tank) into the network's slotless **fluid
channel** at the jar's 1000 mB per tick, a **Fluid Exporter** fills the tank
behind its face from that channel, and the readout grows a `fluids … mB` line.
The two channels are independent, and a cable's status follows the parts that
have work on their own channel.

What is not here: the energy, world, interface and player-simulator parts,
filters, the variable-card/aspect layer, part screens, network energy, item NBT
fidelity in the store, the multipart block-entity renderer and the part recipes
(their ingredients are Integrated Dynamics content the parent port does not
carry). See `notes.md` for the full slice boundary, the coverage table and the
gaps, `VALIDATION.md` for every measured number, and `FINDINGS.md` for the jar
inventory the slices were chosen from.

## The package (SDK layout, 2026-11-18)

One flat `.hmod` carries **both halves** — content and code — so a single file
installs the carrier block, the four part items and the network behaviour:

```text
integratedtunnels.hmod
  horizon-mod.toml    id, version, api = "0.2", side, capabilities, [native] pins
  Cargo.toml          one crate; the only dependency is horizon-mod-api (.horizon/sdk/)
  src/lib.rs          the one #[mod_api::horizon_mod] impl Hooks
  src/cable.rs        the game glue: block entity part, tick queue, network pass, commands
  src/runtime.rs      the pure logic and its tests (item + fluid rules)
  assets/ data/       the carrier block, the four part items, models, textures, lang, loot
  LICENSE, LICENSE-NOTICE.md
  uses.json           the surface the package uses (written by the build)
```

The retired `.hnmod` did not go away: it is kept under
`ports/_shared/evidence/integratedtunnels/retired-hnmod-0.1.0/`, with the old
content manifest and the built 0.1.0 content `.hmod`.

## Building and checking

The SDK tool runs in the port; the build/test steps go through the campaign's
slot runner and are compiled with the pinned SDK (`.horizon/sdk`, fetched by the
first `cargo horizon-mod` command):

```sh
cd ports/integratedtunnels
cargo test                                    # the pure runtime: 28 passed
cargo clippy --all-targets -- -D warnings      # 0 warnings
cargo fmt --check                              # of this crate only (never --all)
rm -f Cargo.lock                               # see notes.md, trap 5
cargo horizon-mod build                        # -> target/horizon-mod/integratedtunnels.hmod
cp target/horizon-mod/integratedtunnels.hmod ./integratedtunnels.hmod
cargo horizon-mod check integratedtunnels.hmod
cargo horizon-mod fits integratedtunnels.hmod --build 0a875da
cargo horizon-mod fits integratedtunnels.hmod --build 0b14f8acfd97fa2c750e42795249dcc981178ca4
```

The in-game scenario is `tools/e2e.sh`, which needs a **launcher-built** game
(compiled against this port) because native code is compiled into the client:

```sh
ports/tools/launcher-game.sh build integratedtunnels
eval "$(ports/tools/launcher-game.sh env integratedtunnels)"
GAME=$GAME CTL=$CTL tools/e2e.sh
```

The jar is evidence at
`ports/_shared/licence-cache/integratedtunnels-1.21.1-neoforge-1.13.0.jar`.

**Local-only** until the maintainer approves publishing (ARCHITECTURE.md §8).
