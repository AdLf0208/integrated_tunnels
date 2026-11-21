# Slice 2 plan — the fluid importer/exporter parts (gap 2)

**Gap line** (`notes.md` § "Gaps (next slices)", item 2): *"the **fluid and energy** parts and the
fluid/energy network channels"*.

**Choice.** Of that item, this slice takes the **fluid** half: `part_importer_fluid` and
`part_exporter_fluid` on a tunnel cable face, a slotless fluid channel in the network store beside the
item channel, and the `/integratedtunnels` readout that shows it. Energy stays a gap because the pin has
no vanilla FE block to prove a transfer against (no in-game assertion = not this slice).

**Why it fits the tier (`deep`).** The jar's fluid parts are the same face-mounted part protocol as the
item parts already ported, plus a fluid capability (`FLUIDS`) and a second network channel. That is
inside the port's existing shape (block entity + per-face parts + end-of-tick network pass); it does not
need an entity, a renderer or a dimension.

**Upstream numbers (read from tag `1.21.1-1.13.0`, commit `dd9ade64`).**
`TunnelAspectWriteBuilders.Fluid.PROPERTIES_RATE` defaults `PROP_RATE` to **1000 mB** (line 1046),
channel 0, round-robin off, passive IO true; `TunnelAspects.Write.Fluid.BOOLEAN_IMPORT/EXPORT` are the
default aspects, each moving through `TunnelHelpers.moveSingleStateOptimized` once per part tick.
The store is upstream's slotless fluid channel; the parts consume 1 energy/tick (recorded gap, as for
the item parts).

## Files

| File | Change |
|---|---|
| `src/runtime.rs` | `Part::ImporterFluid`/`ExporterFluid`; `FLUID_RATE`; `FluidStack`/`Fluids`; `TankView`, `FluidMove`, `plan_fluid_import`, `commit_fluid_import`, `plan_fluid_export`, `commit_fluid_export`, `describe_fluids`; `status_for` takes both channels. Tests first. |
| `src/cable.rs` | `FLUIDS` capability, `import_fluid_once`/`export_fluid_once`, the `fluids` NBT list, the readout line, the part command words, the two part item ids. |
| `src/lib.rs` | doc only (the two new parts). |
| `data/integratedtunnels/horizon/item/part_{importer,exporter}_fluid.json` | the two items. |
| `data/integratedtunnels/horizon/creative_tab/integratedtunnels.json` | the two items. |
| `data/integratedtunnels/horizon/block/tunnel_cable.json` | `importer_fluid`/`exporter_fluid` in every `part_<face>` enum. |
| `assets/.../blockstates/tunnel_cable.json` | 48 fluid multipart entries (6 faces × 2 parts × 2 statuses). |
| `assets/.../models/block/part_{importer,exporter}_fluid_{active,inactive}.json` | the connector box with the jar's texture. |
| `assets/.../models/item/part_{importer,exporter}_fluid.json` | parent = the active block model. |
| `assets/.../textures/part/{importer,exporter}_fluid_{active,inactive}.png` | copied from the pack jar. |
| `assets/.../lang/en_us.json` | the jar's two part names + descriptions. |
| `tools/e2e.sh` | fluid phases: a full water cauldron drains into the fluid channel, an exporter fills a second cauldron, controls (no part / no target), and the store readout. |

## TDD order

1. Red: the fluid rules' unit tests in `runtime.rs` (`fluid_part_names_round_trip`,
   `an_importer_takes_the_first_tank_up_to_its_rate`, `a_committed_fluid_import_lands_in_the_channel`,
   `an_exporter_pushes_the_first_fluid_up_to_its_rate`,
   `a_committed_fluid_export_takes_only_what_the_tank_accepted`,
   `the_status_follows_both_channels`, `the_fluid_line_names_each_stack`).
2. Green: the rules.
3. Glue: `cable.rs` (capability, transaction, NBT, readout, command, placement).
4. Content: assets/data, generated blockstates, textures from the jar.
5. Ladder: `cargo test`, `clippy --all-targets -- -D warnings`, `fmt --check`, two byte-identical
   builds, `check`, two `fits`, `verify-sdk-package.py`.
6. In game: `launcher-game.sh build` (manifest bumped first, `Compiling integratedtunnels (1 of 1)`),
   then the e2e through the slot runner; control (`CONTROL_GAME`) and `SELFTEST=1` at `EXIT:1`.

**Pin.** `[native] horizon = 0a875da…`, mixins `e59da909…`; `.horizon/sdk` deleted after the repin so
the fetch is the new commit (trap §9d-17a).
