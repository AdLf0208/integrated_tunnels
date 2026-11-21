# Integrated Tunnels — jar inventory and upstream behaviour (slice 1)

**Mod:** Integrated Tunnels `integratedtunnels` — class `content`, tier **`deep`**,
add-on (parent: `integrateddynamics`), priority P0.
**Licence:** **MIT**, read from the pack jar's `META-INF/neoforge.mods.toml`
(`license="MIT"`, `modId="integratedtunnels"`, `version="1.13.0"`,
`authors="rubensworks (aka kroeser)"`) → **rank 0**.

**Pack jar (the artefact this file describes):**
`ports/_shared/licence-cache/integratedtunnels-1.21.1-neoforge-1.13.0.jar`
`747,565 B`, SHA-256 `9b5bed5e69d071bf9266c0d9186bfaa952970493e93defda0cce35edb978e19e`,
**808 entries**, kept as the licence evidence.

Every number below is measured from that jar (`unzip -l` / `unzip -p`) or read
from the upstream source at the matching tag, never inferred from the mod's
name.

## 1. Jar inventory (measured)

| Count | Entries |
|---:|---|
| 808 | all entries |
| 22 | `assets/integratedtunnels/blockstates/part_*.json` |
| 23 | `assets/integratedtunnels/models/item/*.json` |
| 58 | `assets/integratedtunnels/textures/part/*.png` |
| 30 | `assets/integratedtunnels/lang/*.json` |
| 22 | `data/integratedtunnels/recipe/crafting/*.json` |
| 9 | `data/integratedtunnels/advancement/**` |
| 71 | `data/integratedtunnels/**` entries in total |

The jar's part families (blockstates + item models):

- `part_importer_item`, `part_importer_fluid`, `part_importer_energy`,
  `part_importer_world_item`, `part_importer_world_fluid`,
  `part_importer_world_energy`, `part_importer_world_block`
- `part_exporter_item`, `part_exporter_fluid`, `part_exporter_energy`,
  `part_exporter_world_item`, `part_exporter_world_fluid`,
  `part_exporter_world_energy`, `part_exporter_world_block`
- `part_interface_item`, `part_interface_fluid`, `part_interface_energy`,
  `part_interface_filter_item`, `part_interface_filter_fluid`,
  `part_interface_filter_energy`
- `part_player_simulator`
- `dummy_pickaxe` (an item model only; the world-block importer's stand-in
  pickaxe)

Shapes the jar carries for every part (`unzip -p`):

- `blockstates/part_importer_item.json`: 18 variants, `facing` ×
  `status=active|inactive|error`, each applying the part model with the rotation
  `north → y:180`, `east → y:270`, `south → none`, `west → y:90`, `up → x:90`,
  `down → x:270`.
- `models/block/connector.json`: one 6×6×4 box `from [5,5,12] to [11,11,16]`,
  all six faces textured `#texture` (uv 4..10 × 4..10), parent
  `integrateddynamics:block/part`.
- `models/block/part_importer_item_<status>.json`: that connector model with
  `texture = integratedtunnels:part/importer_item_<status>`.
- `models/item/part_importer_item.json`: parent
  `integratedtunnels:block/connector`, texture
  `integratedtunnels:part/importer_item_active` (the item is the active model).
- `textures/part/importer_item_{active,inactive,error}.png` (263/264/262 B) and
  the exporter equivalents.

Recipes (`crafting_shapeless`): `part_importer_item` = `part_interface_item` +
`integrateddynamics:variable_transformer_input`; `part_exporter_item` =
`part_interface_item` + `integrateddynamics:variable_transformer_output`. Both
ingredients are Integrated Dynamics content (the parent port does not carry the
variable transformer), so neither recipe is carried in this slice.

`assets/minecraft/atlases/blocks.json` lists every part texture as a
`minecraft:single` sprite (a directory-less atlas in 1.21.1). 26.3 resolves
`textures/part/<name>.png` by path, so the atlas file is not carried.

## 2. Upstream source read (analysis aid, not a code source)

- Repo: <https://github.com/CyclopsMC/IntegratedTunnels>, tag
  **`1.21.1-1.13.0`**, commit `dd9ade64e31b0f56c2dc672541d0e9583119670f`
  (2026-10-03, "Bump mod version") — the tag that matches the pack jar's
  `version="1.13.0"`.
- Clone: `ports/_shared/upstream/integratedtunnels/` (gitignored; deleted after
  this file was written).
- Files read:
  - `part/PartTypeImporterItem.java`, `part/PartTypeExporterItem.java` — the two
    part types, their write aspects and their consumption rates.
  - `part/aspect/TunnelAspects.java` — the aspect registrations for the item
    importer/exporter.
  - `part/aspect/TunnelAspectWriteBuilders.java` — `PROP_IMPORT` / `PROP_EXPORT`,
    the property defaults (`PROPERTIES_RATESLOT`), and the matcher each builds.
  - `core/TunnelHelpers.java` — `moveSingle` / `moveSingleStateOptimized`, the
    "nothing moved → sleep" counter.
  - `part/aspect/PartStateItem.java`, `PartStateWorld.java` — part state.
  - `GeneralConfig.java` — the configurable numbers.
  - `core/TunnelItemHelpers.java` — `matchAll(amount, exactAmount)`.

### Behaviour extracted

1. **The part is a face-mounted part on an Integrated Dynamics network.** A
   cable's block entity holds one part per face; a part's target is the block
   on its own face (`PartTarget`/`PartPos`). The part item is placed on a cable
   face (`ItemPart.useOn` → `PartHelpers.addPart`).
2. **An Item Importer pulls from its target into the network.** Its default
   aspect is `item.boolean.import`
   (`TunnelAspects.Write.Item.BOOLEAN_IMPORT`), whose propagator is
   `PROP_IMPORT`: `TunnelHelpers.moveSingleStateOptimized(network, …, source =
   the part's storage [the inventory behind its face], destination = the
   network's item channel, matcher, …)`.
3. **An Item Exporter pushes from the network into its target**, the mirror
   image (`PROP_EXPORT`: source = the network channel, destination = the
   inventory behind the face).
4. **The rate is 64 items per write.** `TunnelAspectWriteBuilders.Item.
   PROPERTIES_RATESLOT` defaults `PROP_RATE` to **64**, `PROP_CHANNEL` to
   `IPositionedAddonsNetworkIngredients.DEFAULT_CHANNEL` (0), `PROP_ROUNDROBIN`
   to `false`, `PROP_SLOT` to `-1` (any slot), `PROP_PASSIVE_IO` to `true`.
   `PROP_BOOLEAN_ITEMPREDICATE` turns that into
   `TunnelItemHelpers.matchAll(rate, exact=false)`, and `moveSingle` moves at
   most that many items per call. A part's write aspect is evaluated once per
   part tick, so the importer moves **at most 64 items per tick**.
5. **The network store is slotless.** `IPositionedAddonsNetworkIngredients<
   ItemStack, Integer>` is a channel-keyed ingredient store shared by every part
   on the network: an importer adds to it, an exporter takes from it. It holds
   full `ItemStack`s (NBT included) and is not limited to 64 per type.
6. **A transfer that moves nothing puts the connection to sleep** for
   `inventoryUnchangedTickCount` ticks (`TunnelHelpers.CACHE_INV_CHECKS`), an
   optimisation only; this slice re-checks every tick.
7. **The part's status** (`active|inactive|error`) drives which part model is
   drawn; `PartStateItem` starts inactive and becomes active while it has a
   valid target.
8. **Every part consumes network energy** (`getConsumptionRate` =
   `importerItemBaseConsumption` / `exporterItemBaseConsumption`, default **1**
   per tick). This slice has no energy system (the parent port's slice has none
   either), so the consumption is a recorded gap.

## 3. What slice 1 carries, and why

The smallest honest `deep` behaviour for this jar is **one part type on a
cable's face doing its real work**: `part_importer_item` pulling from the
inventory behind its face into the network store at the jar's rate, plus
`part_exporter_item` because without a sink the store is unobservable in game
(an exporter is the differential that proves the loop is the network's, not a
block-to-block hop).

Carried from the jar, byte-for-byte or re-spelled for 26.3:

- `part_importer_item` and `part_exporter_item` **item models** and their
  **textures** (`importer_item_active|inactive`, `exporter_item_active|
  inactive`; `error` is not carried — the slice has no error state).
- The part **geometry** (the connector's 6×6×4 box) and the jar's per-face
  rotations, in our own model files.
- The **lang** lines for the two parts and the part-type descriptions.
- The two **items** (`data/integratedtunnels/horizon/item/…`, template
  `minecraft:paper` as upstream's item is a plain item).

Written here because the jar does not contain them:

- A **carrier block** `integratedtunnels:tunnel_cable` with the six
  `part_<face>` properties and a `status`, mirroring the parent port's
  `integrateddynamics:cable`. There is no cross-mod API at this pin
  (ARCHITECTURE.md §3), two packs cannot share a namespace, and the parent
  port's cable enumerates only `none|reader|writer`, so an Integrated Tunnels
  part cannot live on that block's state. The carrier is the port's own
  stand-in for the Integrated Dynamics cable; a cable recipe is not carried.

Not carried (gaps, recorded in `notes.md`): energy/world parts, the interface
parts, the player simulator, filters, the variable-card/aspect layer, the part
screens, network energy, item NBT fidelity in the store, the multipart BE
renderer, the part recipes (their ingredients are content the parent port does
not carry), advancements, and the `tunnels_tutorials`/`tunnels_info` info book.

## 4. Slice 2: the fluid parts (measured 2026-11-21)

Read from the same jar and from tag `1.21.1-1.13.0` (`dd9ade64e31b`; the clone
was re-made for this slice and deleted after these facts were recorded):

- **Items and shapes.** `part_importer_fluid`/`part_exporter_fluid` exist as
  item models and six `facing × status=active|inactive|error` blockstate
  variants each, exactly like the item parts: the connector's 6×6×4 box with
  the jar's rotations. The four textures are `part/importer_fluid_active|
  inactive` and `part/exporter_fluid_active|inactive` (260/260/233/226 B).
- **Rate.** `TunnelAspectWriteBuilders.Fluid.PROPERTIES_RATE` defaults
  `PROP_RATE` to **1000** (mB), channel 0, round-robin off, passive IO true,
  `PROP_CHECK_AMOUNT` false; the default aspects are `Write.Fluid.
  BOOLEAN_IMPORT`/`BOOLEAN_EXPORT`, each moving through
  `TunnelHelpers.moveSingleStateOptimized` once per part tick.
- **Channel.** Upstream's fluid channel is the same slotless
  `IPositionedAddonsNetworkIngredients` store as the item channel, keyed by
  fluid; the part's target is the block on its face
  (`TunnelFluidHelpers.matchAll(rate, exact)`).
- **Energy.** `GeneralConfig.importerFluidBaseConsumption` /
  `exporterFluidBaseConsumption` default **1** per tick — a gap, as for the item
  parts.
- **Recipes.** `part_importer_fluid` = `part_interface_fluid` +
  `integrateddynamics:variable_transformer_input`; the exporter uses
  `…_output`. Neither ingredient is content this pack carries, so the recipes
  stay a gap (the same reason as the item parts).
- **The test tank.** At `0a875da` the game's fluid capability answers for a
  cauldron as one tank holding whole levels (333/667/1000 mB for water), which
  is what makes the fluid family provable in game without another mod.
