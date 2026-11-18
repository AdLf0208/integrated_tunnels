# Integrated Tunnels — a Horizon port

**Integrated Tunnels** is a Minecraft mod. Transfer other energy, items and fluids over Integrated Dynamics networks. This repository is a **port of it to [Horizon](https://github.com/horizon-rust/horizon)**, the Rust engine for Minecraft Java 26.3. It carries the complete source of the port plus the content files it ships, and installs as a single `.hmod` package.

Upstream: [CyclopsMC/IntegratedTunnels](https://github.com/CyclopsMC/IntegratedTunnels) — licence **MIT**.

| | |
|---|---|
| Upstream mod | Integrated Tunnels ([source](https://github.com/CyclopsMC/IntegratedTunnels)) |
| Ported to | Horizon 0b14f8a, Minecraft Java 26.3, mod API `0.2` |
| Package | `integratedtunnels.hmod` (install it with the Horizon launcher) |
| Licence | **MIT** — the upstream mod's licence; see [Licence and credit](#licence-and-credit) |

## What this port implements

- **`integratedtunnels:tunnel_cable`** — a 4 px non-occluding core (hardness
- **Parts** — `integratedtunnels:part_importer_item` and
- **Importer** — `native_container_slots` (API 48) reads the slots the face
- **Exporter** — `runtime::plan_export` picks the store's first stack (up to 64)
- **Network** — the connected cable set (6-way adjacency, bounded to 1,024
- **Cadence** — a cable's block-entity tick only **queues** it (a tick's own
- **Harness readout** — `/integratedtunnels info <x> <y> <z>` prints the six
- **One transaction per network per tick.** Every import/export and the store write now share the

## Not yet ported

- the **variable/aspect layer**: variable cards, the aspect selection GUI, and
- the **fluid and energy** parts and the fluid/energy network channels;
- the **world parts** (`importer/exporter_world_item|fluid|energy|block`) and
- the **interface** parts (item/fluid/energy and their filtering variants) and
- **part screens**, the part-removal/drop path and the part `error` state;
- **network energy**: `getConsumptionRate`, the energy buffer, the energy parts;
- **cable interop**: an Integrated Tunnels part cannot sit on the sibling port's
- **NBT fidelity** in the store (components on stored items);

## Installing

1. Install the [Horizon launcher](https://github.com/horizon-rust/horizon/releases) (0.2.70 or newer).
2. Add `integratedtunnels.hmod` to the instance's mods.
3. The package is pinned to a specific Horizon build; the launcher checks that pin.

## Licence and credit

Integrated Tunnels was created by its upstream authors; the code, models and assets in the original mod
remain theirs and are used here under **MIT**. The Rust in this repository is a clean-room
rewrite for Horizon, not a translation of the original source.

- `LICENSE` — the upstream licence text.
- `LICENSE-NOTICE.md` — what this port carries and where it came from.
- The upstream project: https://github.com/CyclopsMC/IntegratedTunnels

## Technical details

```
horizon-mod.toml   package manifest (id, version, api, [native] pin)
Cargo.toml         the crate; depends on `horizon-mod-api` from the fetched SDK
src/               lib.rs (the one `impl Hooks`), runtime.rs (pure logic + tests), glue
assets/ data/      the content the package ships
tools/             content generators and the e2e scenario
```

- Build: `cargo horizon-mod build` (packages; it does not compile against the game).
- Test: `cargo test`, `cargo clippy --all-targets -- -D warnings`.
- In-game: `ports/tools/launcher-game.sh run integratedtunnels` in the campaign tree, or install the
  `.hmod` and play; the harness scenario is `tools/e2e.sh`.

This repository is generated from the campaign's port at `ports/integratedtunnels/`; the port's own
`notes.md` is the full record (decisions, traps, evidence).
