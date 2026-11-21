# Integrated Tunnels → Horizon (slices 1–2)

Port of **Integrated Tunnels 1.13.0** (`CyclopsMC/IntegratedTunnels`, **MIT** per
the jar's `META-INF/neoforge.mods.toml`), Minecraft 1.21.1 NeoForge, from the
exact ATM10 jar: `integratedtunnels-1.21.1-neoforge-1.13.0.jar`, **747,565 B**,
SHA-256 `9b5bed5e69d071bf9266c0d9186bfaa952970493e93defda0cce35edb978e19e`,
**808 entries**, kept at `ports/_shared/licence-cache/`. The jar inventory, the
upstream source read and the behaviour extracted are in [`FINDINGS.md`](FINDINGS.md).

**Class + tier:** `content` / **`deep`** (ledger row; provenance
`ports/_shared/source-map/atm10-sources.csv`). Approach per Horizon's
`docs/modpacks.md` (§ "Tiers"): a deep mod needs new vanilla machinery — here a
block entity, a per-face part protocol and a network that moves items on the
level's tick, none of which a `data`/`logic`/`machines` carry would express.
`precheck-mod.py integratedtunnels` reports `content · deep · addon · P0`, licence
`MIT` rank 0.

**Licence:** MIT → **rank 0** (permissive). Local-only until the maintainer
approves publishing (ARCHITECTURE.md §8); the file keeps the Cyclops copyright
and the MIT text (`LICENSE`, `LICENSE-NOTICE.md`).
**Why now:** licence-first queue (rank 0), and its parent `integrateddynamics` is
**ours and ported-partial**, so the part protocol could be mirrored from a
working implementation instead of guessed.

**Format: the SDK package (2026-11-18), version 0.2.0.** This port was migrated from
the retired `.hnmod` container to the flat SDK package — one `.hmod` carrying the content
(`assets/`, `data/`) **and** the code (`src/`) — see
[`## Migration to the SDK format`](#migration-to-the-sdk-format) for the file map, the
helper→hook table and every trap. `src/runtime.rs` is **byte-identical** to the retired
`mixins/integratedtunnels/runtime.rs` (sha256
`123e45abe9c446638b727c358777d26e3c696b367904085bfb33dac4cdadee1c`), including the copy
inside the built package. The previous submission, **PR #810** (0.1.0, the `.hnmod`), was
closed by the 2026-11-18 format purge; 0.2.0 supersedes it.

## Slice 1: the item tunnel

An **Item Importer** part on a tunnel cable face pulls items out of the inventory
behind that face into the cable network's slotless item store at the jar's rate
(64 items per part per tick), an **Item Exporter** part on another cable face
hands them into the inventory behind *its* face, and the cable's
`/integratedtunnels info` readout shows the parts, the store and the status. It
is upstream's `import_all_items`/`exporter_item` pair without the variable-card
layer.

### What the slice does

- **`integratedtunnels:tunnel_cable`** — a 4 px non-occluding core (hardness
  3.0), six `part_<face>` enum properties (`none|importer_item|exporter_item`),
  a `status` property (`inactive|active`), a block entity holding the network's
  item store, and a self-drop loot table. This carrier is the port's own block:
  see "Deviations" — there is no cross-mod API, two packs cannot share a
  namespace, and the parent port's cable enumerates only `none|reader|writer`.
- **Parts** — `integratedtunnels:part_importer_item` and
  `integratedtunnels:part_exporter_item` **items** (the jar's ids), placed on a clicked cable
  face (`right_click_block`) and consumed outside creative; parts live in the
  cable's state (and its block entity), as upstream's multipart data lives in
  the cable. A face that already holds a part is refused (the unported screen).
- **Importer** — `native_container_slots` (API 48) reads the slots the face
  reaches, `native_extract_item` takes up to **64** items from the first slot the
  side may take from, and the items join the network store
  (`runtime::plan_import` + `commit_import`). The engine's answer, not the plan,
  is what is stored.
- **Exporter** — `runtime::plan_export` picks the store's first stack (up to 64)
  and `native_insert_item` (API 20) hands it into the inventory behind its face;
  what the destination refuses stays in the store (`commit_export`).
- **Network** — the connected cable set (6-way adjacency, bounded to 1,024
  cables); the store lives on the network's **root** cable (lowest position) so
  every cable of one network agrees without extra state; importers run first,
  then exporters, so items imported this tick can leave the same tick.
- **Cadence** — a cable's block-entity tick only **queues** it (a tick's own
  block-entity writes are reverted); the store is read and written at
  `level_tick_post`, once per network.
- **Harness readout** — `/integratedtunnels info <x> <y> <z>` prints the six
  parts, the network's store (`2 stacks, 192 items (…); status active`) and the
  status; `/integratedtunnels part <x> <y> <z> <face> <none|importer_item|
  exporter_item>` places/removes a part (the path `ctl interact` needs when a
  face is not clickable).

### Deliberate deviations

| Upstream | Here | Why |
|---|---|---|
| Parts sit on an Integrated Dynamics cable's block entity | the port ships its own `integratedtunnels:tunnel_cable` carrier with the same six-slot protocol | no cross-mod API at the pin (ARCHITECTURE §3); two packs cannot share a namespace; the parent port's cable property enumerates only `none|reader|writer`, so another pack's part has nowhere to live. A pack with both ports sees two cable families (gap below) |
| The part is one of seven aspects (boolean/integer/slot/itemstack/list/predicate/nbt) selected in a GUI | one importer/exporter part with the **default aspect's** behaviour: `item.boolean.import`/`export`, rate 64, channel 0, slot −1, no round-robin | the aspect/card layer is the variable system, out of slice; the defaults are the jar's (`PROPERTIES_RATESLOT`) |
| The network store holds full `ItemStack`s, NBT included | the store holds an item id and a count per entry | no filter/NBT aspect in the slice; a component-carrying item would be normalised on import. Recorded as a gap |
| A part consumes network energy (`importerItemBaseConsumption` 1/tick) | no energy | the parent port's slice has no energy network; the energy parts and the network's energy buffer are gaps |
| A part sleeps for `inventoryUnchangedTickCount` ticks after moving nothing | every importer/exporter re-checks each tick | an optimisation only; the sleep is per-connection upstream and invisible in game |
| The part asks for a target through `IItemTarget` capabilities (other mods' slotless handlers included) | the engine's container view through the pin's API 48 helpers (chests, barrels, furnaces, mod containers) | the pin's helper is the capability surface for mods; a slotless, non-container source is not reachable |
| `status=error` when the target is invalid | only `inactive`/`active` | the slice has no error state to report; the jar's `*_error` models are not carried |
| Parts drop themselves and keep their state on break | the cable's state properties hold the parts; the carrier drops itself | the part items are carried, the part-removal screen is not |

### Upstream defects fixed

**None found in this slice's path.** The importer/exporter aspects, the rate
default and the two recipes were read line by line against the jar's data and
nothing is internally inconsistent: no id/tag mismatch, no unreachable recipe,
no off-by-one in the movement (the rate is the predicate's `maxQuantity`, used as
an amount), no slot index written where another is read. Checked on
2026-11-11 against tag `1.21.1-1.13.0`; a later slice carrying filters, slots or
the world parts will re-run this check on those paths.

### Gaps (next slices)

1. the **variable/aspect layer**: variable cards, the aspect selection GUI, and
   the other six item aspects (integer count, slot index, itemstack, list,
   predicate, NBT) — the whole point of the mod's "advanced" behaviour;
2. the **energy** parts and the energy network channel — the **fluid** half is
   [slice 2](#slice-2-the-fluid-tunnel-022);
3. the **world parts** (`importer/exporter_world_item|fluid|energy|block`) and
   the block break/place handlers (the `ExtendedFakePlayer`, the
   `dummy_pickaxe`);
4. the **interface** parts (item/fluid/energy and their filtering variants) and
   the **player simulator**;
5. **part screens**, the part-removal/drop path and the part `error` state;
6. **network energy**: `getConsumptionRate`, the energy buffer, the energy parts;
7. **cable interop**: an Integrated Tunnels part cannot sit on the sibling port's
   `integrateddynamics:cable` (properties and namespaces), and this carrier
   cannot be crafted (upstream's cable recipe is Integrated Dynamics content);
8. **NBT fidelity** in the store (components on stored items);
9. the two part **recipes** (they need `integrateddynamics:variable_transformer_
   input|output`, which the parent port does not carry), advancements and the
   info book (`tunnels_info.xml`, `tunnels_tutorials.xml`);
10. the multipart **renderer** (upstream draws parts and cable arms with a block
    entity renderer; this port draws them with multipart block models).

Skills: superpowers/using-superpowers,brainstorming,writing-plans,test-driven-development,systematic-debugging,verification-before-completion; BeautifulRust/rust-best-practices,rust-port

## Migration to the SDK format

*Recipe followed from `ports/imfast/notes.md` → `## Migration to the SDK format` (the first
migration), 2026-11-18. Version **0.1.0 → 0.2.0**; **PR #810** (the `.hnmod`) is superseded.*

**Pins.** SDK nightly **`0b14f8acfd97fa2c750e42795249dcc981178ca4`** (surface + api-source cached
at `ports/_shared/sdk/0b14f8a/`), mixins **`e59da9094f0c2fdda6c627fe9eff22dec6a906f0`** (was
`5a64af93…`), launcher 0.2.78, `package_format` 6, `content_api` 0.2. The old campaign pin
`1d63e0a` does not apply to a package build.

**Why.** Horizon's mod-API merge (#16941, launcher ≥ 0.2.70) retired `.hnmod`; the registry closed
every `.hnmod` submission on 2026-11-18 with one reason ("republish this as a native `.hmod` with the
SDK from the next Nightly"). The new unit is **one flat `.hmod`** — content, native code, or both —
so this port no longer needs the AE2 embedded-content trick at all.

**The tool is run directly in the port.** `ports/tools/build-slot.sh` speaks the retired staging
tool (`tools/horizon.py`, `mixins/`), which this layout no longer has, so every command below ran in
`ports/integratedtunnels` with `CARGO_INCREMENTAL=0` and no `--target-dir`. That is a deliberate
exception to the build-slot rule, forced by the format, and it should end when the runner learns the
SDK layout.

### 1. Files created

| Path | What it is |
|---|---|
| `horizon-mod.toml` | the only manifest and the only identity: `id = "integratedtunnels"`, `version = "0.2.0"`, `api = "0.2"`, `side = "both"`, `capabilities = ["registries"]`, `license = "MIT"`, and `[native]` pinned to `0b14f8a` / `e59da90` |
| `Cargo.toml` | one crate; its only dependency is `horizon-mod-api` from `.horizon/sdk/` |
| `src/lib.rs` | the one `#[mod_api::horizon_mod] impl Hooks`: `register`, `right_click_block`, `level_tick_post`, `register_commands`, `run_mod_command` |
| `src/cable.rs` | the game glue (the old `support.rs`): the block entity **Part**, the tick queue, the once-per-network pass, the store's NBT, the two command handlers |
| `src/runtime.rs` | copied byte-for-byte from `mixins/integratedtunnels/runtime.rs` (`cp`, then `cmp`) |
| `.gitignore` | `/target`, `/.horizon`, `/Cargo.lock` |
| `assets/`, `data/` | the carried content, moved from `content/` (same files, no separate build) |
| `LICENSE`, `LICENSE-NOTICE.md`, `README.md` | inside the package |
| `tools/e2e.sh` | kept, with its launch path rewritten for a launcher-built game |

### 2. Files deleted (and where the evidence went)

| Deleted | Why | Kept as evidence |
|---|---|---|
| `mixins/integratedtunnels/` (plan + 8 helpers + `support.rs` + `runtime.rs` + the embedded `.hmod`) | the container is retired; `runtime.rs` moved to `src/` | `ports/_shared/evidence/integratedtunnels/retired-hnmod-0.1.0/mixins/` |
| `integratedtunnels.hnmod` (21,682 B) | unloadable on launcher ≥ 0.2.70 | same folder |
| `content/horizon-mod.toml`, `content/integratedtunnels.hmod` (9,812 B) | one manifest and one package now | same folder, `content-old/` |
| `compatibility.json` | `[native]` in the manifest is the pin now | same folder |
| `rust-toolchain.toml` | the pinned Horizon commit and the launcher decide the toolchain | same folder |
| `tools/horizon.py`, `tools/package.py`, `tools/package.sh`, `tools/assemble_content.py` | staging and `.hnmod` packaging are gone | same folder, `tools/` |
| `Cargo.lock` | not needed without registry deps, and it changes the artifact (trap 5) | not kept |

**Kept unchanged:** `notes.md`, `FINDINGS.md`, `LICENSE`, `LICENSE-NOTICE.md`, and the scenario in
`tools/e2e.sh` (its assertions are format-independent).

### 3. Manifest field mapping (this port)

| Old (`.hnmod` + `content/`) | New (one `.hmod`) |
|---|---|
| `horizon-mixin.toml` `[mod] id = "integratedtunnels"` | `id` (also the resource namespace) |
| `[mod] version = "0.1.0"` (bundle) and `0.1.0` (content) | `version = "0.2.0"` — one version for the one package |
| `[mod] priority = 100` | gone; ordering is `#[listener(priority = …)]` |
| `[compatibility] horizon_revision = "1d63e0a5…"` | `[native] horizon = "0b14f8ac…"` |
| mixins `5a64af938ec8…` | `[native] mixins = "e59da9094f0c…"` |
| `[[module]] runtime` + `[[module]] state` (`support.rs`) | `src/runtime.rs` and `src/cable.rs` — ordinary Rust modules; no plan section, no support module |
| 8 `[[injection]]` entries | 5 `Hooks` methods + 2 `BlockEntityPart` methods (table below) |
| `horizon-native.json` | gone |
| `content/horizon-mod.toml` | one `horizon-mod.toml` at the package root; `assets/` and `data/` beside it |
| — | `side = "both"` (the cable is placed and ticked by an integrated server, which belongs to the client installation), `capabilities = ["registries"]` (fixed shared value) |

### 4. Old injection → new surface

| Old injection (helper) | New surface | Notes |
|---|---|---|
| `embedded_content.rs` (`content_root` HEAD, `native_data!`) | **gone** | one package carries `assets/` + `data/`; nothing is written to `.horizon-cache` and no dev-list append is needed |
| `block_entity.rs` (`block_entity`, HEAD) | *not needed* | a declared block entity type starts with an empty compound; `BlockEntityPart::block_entity` only adds initial data, and there is none |
| `block_entity_ticks.rs` | `BlockEntityPart::block_entity_ticks` → `event.ticks = true` | a **Part** method, not a `Hooks` method |
| `block_entity_tick.rs` | `BlockEntityPart::block_entity_tick` → queue `event.pos` | the Part gets `host: &mut dyn ServerHost`, `event.pos`, `event.entity` (the saved data, writable) |
| `level_tick.rs` (`level_tick_post`) | `Hooks::level_tick_post(host: HostSide, event: &mut LevelTickPostEvent)` | `event.level: LevelRef`; the whole pass runs in **one transaction** per network |
| `right_click.rs` | `Hooks::right_click_block(host: HostSide, event: &mut RightClickBlockEvent) -> Flow` | `event.click: BlockClick` (pos, hit.face, hand, stack, player); returns `Flow::Cancel` to take the click |
| `register_commands.rs` | `Hooks::register_commands` → `Command::new(usage)` | real usages: `integratedtunnels info <pos:block_pos>`, `integratedtunnels part <pos:block_pos> <face:word> <part:word>` |
| `perform_command.rs` (`perform_command`) | `Hooks::run_mod_command` → `Flow` | `perform_command` no longer exists; the invocation carries parsed `literals` and typed `arguments` |
| `support.rs` (`static STATE: Mutex<…>`) | `Mod { queue: Arc<Mutex<Queue>> }` + `Cable { queue }` | the macro boxes one `Default` `Mod`; the Part gets a clone of the same `Arc` in `register` |
| — | `Hooks::register` | `r.block_entity_type("tunnel_cable").blocks(&["tunnel_cable"]).build()` then `r.block_entity_part(&kind, Cable::new(…))` |

### 5. What changed in the glue's behaviour (not in `runtime.rs`)

- **One transaction per network per tick.** Every import/export and the store write now share the
  network's transaction, and a store write the engine refuses (`merge_data` answering not-`Ok(true)`)
  aborts it, so no item leaves or enters a container without the store recording it. In 0.1.0 the
  store was written straight into the block entity's NBT, which could not fail.
- **The queue is per level.** A block-entity tick event carries no level handle, so the tick
  resolves the level in place (`host.levels().list_levels()` + `level.in_place(host)`) and keys the
  queue by `LevelRef`. 0.1.0 kept one position set for every dimension.
- **The container view is the `ITEMS` capability** (`ResourceHandler::size/resource/amount/extract/
  insert_any`) instead of the retired `native_container_slots`/`native_extract_item`/
  `native_insert_item` helpers. The handler is already side-filtered, so `SlotView::can_take` is
  `true` for every slot it lists; the engine's `extract` is the last word on what may be taken.
- **The pass runs only while its level is in place** (`level.in_place(host)` at `level_tick_post`),
  because `LevelRef::capability` — the container view the movers use — answers only for the level the
  server ticks now. Outside that window the queue is left for the next tick (bounded by the cable
  count), so a network's work is never dropped. The status property is written in the same window,
  since `ServerHost::set_block_property` also acts on the level in place. It is cosmetic: the
  `/integratedtunnels info` readout derives the status from the store, as before.
- **Placement consumes through `ItemPlace::extract`**, which gives without losing any for a creative
  player — the rule 0.1.0 spelled out as `game_mode != 1`.

### 6. Ladder measured for 0.2.0

Run in the port, `CARGO_INCREMENTAL=0`, in the order below (logs in
`ports/_shared/evidence/integratedtunnels/`):

| Step | Command | Result |
|---|---|---|
| Runtime unit tests | `cargo test` | **17 passed**, 0 failed (`sdk-test.log`) |
| Lints | `cargo clippy --all-targets -- -D warnings` | exit 0, **0 warnings** (`sdk-clippy.log`) |
| Formatting | `cargo fmt --check` (never `--all`) | clean (`sdk-fmt.log`) |
| Package | `rm -f Cargo.lock; cargo horizon-mod build` | `target/horizon-mod/integratedtunnels.hmod`, EXIT 0 (`sdk-build.log`) |
| Registry check | `cargo horizon-mod check integratedtunnels.hmod` | `integratedtunnels 0.2.0 (native-mod, format native-1/0b14f8a/e59da90, for 26.3)`, **37,602 B**, sha256 `c72e5302004d32f13772d8ba976c75be208e947624fff312b8d60019d0695e0b`, **EXIT 0** (`sdk-check.log`, `sdk-check-json.log`) |
| What it uses | `cargo horizon-mod check … --uses` | 330 surface entries (`sdk-uses.json`), EXIT 0 |
| Fits the pinned build | `cargo horizon-mod fits integratedtunnels.hmod --build nightly` | `0b14f8a  made for`, EXIT 0 (`sdk-fits.log`) |
| Reproducible | `cargo horizon-mod build` again | byte-identical, same 37,602 B and sha256 |
| Fresh clone | `.horizon/`, `target/`, `Cargo.lock` removed, then `cargo horizon-mod build` | SDK fetched by itself; same size, same sha256 (`sdk-fresh-clone.log`) |
| `runtime.rs` untouched | `cmp src/runtime.rs <retired>` and `unzip -p … \| sha256sum` | `123e45ab…dae1c` in all three places (`sdk-runtime-sha256.txt`) |
| Skills line | `python3 ports/tools/check-skills.py --notes ports/integratedtunnels` | `Skills line present` |

### 7. Traps, all hit in this migration (16–26 are new to this port)

The first fifteen are the reference's (`ports/imfast/notes.md` §6) and were all re-confirmed here:
a leftover old path refuses the package, a 38-char pin reads as missing, `check` with no file checks
wasm, `fmt --all` formats the SDK, a packed `Cargo.lock` changes the artifact, `side` is the
installation side, `capabilities = ["registries"]` is mandatory, `runtime.rs` must be `pub mod`,
session state must be `Mutex`, `ServerTickPhase`/`#[non_exhaustive]` wildcards, context types changed
shape, `commands()` is `WorldOnly`, no `rust-toolchain.toml`, `horizon-mod build` does pack a native
project, and the e2e needs a launcher-built game. New here:

16. **Most `host::levels` / `host::items` / `host::players` names are *backend* methods, not host
    methods.** `level_in_place`, `block_property`, `container_listing`, `player_level` and
    `game_extract` fail with "no method named … found for `&mut dyn ServerHost`"; each is reached
    through a **handle**: `level.in_place(host)`, `level.block_property(host, pos, key)`,
    `player.level(host)`, `ItemPlace::extract(host, tx, …)`. Grep
    `generated/mod_api_generated_host.rs` for `impl <Handle>` before writing a call.
17. **Containers are the `ITEMS` capability, not a host helper.** `level.capability(host, &ITEMS,
    pos, Some(side))` gives `Capability<dyn ResourceHandler<ItemResource>>` (`size`, `resource`,
    `amount`, `extract`, `insert`, `insert_any`). It returns `None` for a level that is **not in
    place**, which is why the pass belongs in `level_tick_post`.
18. **`JavaString` is not re-exported.** Write NBT with `Tag::String("x".into())`,
    `Tag::compound([("k", tag)])` and `Compound::insert("k".into(), tag)`; read an entry's field with
    `Tag::get(&str)` (the `Compound` itself is a `BTreeMap<JavaString, Tag>`, so its `get` needs a
    `JavaString`).
19. **A part is a trait impl, not a hook.** `#[mod_api::horizon_part] impl BlockPart for Cable {}`
    **plus** `#[mod_api::horizon_part] impl BlockEntityPart for Cable { … }`, attached in
    `Hooks::register`; `block_entity_ticks` and `block_entity_tick` live on the Part and never on
    `Hooks`. The Part's methods need `event.ticks`/`event.pos`/`event.entity`, never a level.
20. **The macro boxes one `Default` mod instance.** Shared state must be an `Arc<Mutex<…>>` on the
    mod struct, cloned into the Part in `register`; a `Mutex` field alone is unreachable from the
    Part, and the §0 rule against statics still holds.
21. **A block-entity tick event has no level handle** (`pos` and `entity` only), so the ticking level
    is resolved with `host.levels().list_levels()` + `level.in_place(host)`. That is one backend call
    per open level per cable per tick — measured acceptable here, and it makes the queue per level.
22. **`ServerHost::set_block_property` acts on the level in place** and keeps the block entity. It is
    the right call for a state property; `LevelRef::set_block` is **not** a substitute, because a
    block write drops the block entity (and with it the network's store).
23. **`perform_command` is gone.** A mod command is a registered usage (`Command::new(…)`) handled by
    `run_mod_command`, which receives parsed literals and typed arguments — so the old free-text
    parsing of `/integratedtunnels info x y z` became `<pos:block_pos> <face:word> <part:word>`.
24. **The embedded-content pattern is unnecessary.** `content_root` + `native_data!` +
    `.horizon-cache` existed only because the registry took one file per project; the SDK package
    carries `assets/` and `data/` beside `src/`.
25. **`cargo horizon-mod build` packs every root document.** The first build shipped the old
    `README.md` (which described `content/` and the `.hnmod`) inside the package; a stale root file
    is a packaging bug. Rewrite it before building and read `unzip -l` afterwards.
26. **The e2e could not be re-run here** — a native package is compiled into the client by the
    launcher (`horizon-launcher native-build <port> --instance <i> --trust-native-code`), and
    `horizon-launcher` is not installed in this environment. `tools/e2e.sh` keeps every assertion
    and now takes `GAME=<launcher-built horizon-client>`; record that run before claiming `ported`.

### 8. Changelog / supersede text (for the resubmission that replaces #810)

> **0.2.0 — republished as a native `.hmod` (the SDK format).** Horizon's mod-API merge retired the
> `.hnmod` container, so this release replaces the closed submission #810 with one flat `.hmod` that
> carries the carrier block, the two part items, their assets and the network code together — the
> embedded-content workaround of 0.1.0 (and its `.horizon-cache` write) is gone, and one file
> installs both halves. Built against SDK nightly `0b14f8a` (mixins `e59da90`, api 0.2, format
> native-1). Behaviour is unchanged: an Item Importer pulls up to 64 items a tick from the inventory
> behind its face into the cable network's store, an Item Exporter pushes up to 64 a tick out, and
> `/integratedtunnels info <pos>` reports the parts, the store and the status. Every move and the
> store write now share one transaction per network per tick, so a store the engine cannot record
> moves nothing. `src/runtime.rs` is byte-identical to 0.1.0 and keeps its 17 tests.

### 9. Gaps this migration adds to the slice's list

- **The in-game e2e is not re-run** (trap 26); it stays the scenario of record from 0.1.0 until a
  launcher-built game exists here.
- **`SlotView::can_take` is always `true`** for the `ITEMS` handler, because the handler is already
  side-filtered. A mod container that lists a slot through a side but refuses the extract makes that
  importer idle for the tick (no fallback to the next slot) — a narrower behaviour than 0.1.0's
  per-slot `can_take`; recorded rather than worked around.
- **The store's counts are saved as `i32`** (saturating at `i32::MAX`), the shape 0.1.0 read; a
  network holding more than 2³¹ of one item would clamp.
- **A network whose root cable has no block entity moves nothing.** The store write is part of the
  pass's transaction, so when `merge_data` cannot reach the root (a cable placed while this mod was
  disabled, a chunk that unloaded mid-tick) the tick's moves are rolled back instead of silently
  losing what left the source — a deliberate behaviour change from 0.1.0, which could drop the
  store; re-placing the cable gives the network a home for it again.

Skills: superpowers/using-superpowers,brainstorming,writing-plans,test-driven-development,systematic-debugging,verification-before-completion; BeautifulRust/rust-best-practices,rust-port

## Review fixes (2026-11-18) — registry review #1187

| Finding | Fix | Test / proof |
|---|---|---|
| `src/lib.rs:115` — `/integratedtunnels part` is level 0 and rewrites any loaded cable without reach or a part item | `part` is registered with `Command::op(usage, 2)`; `info` stays a level-0 readout | `cable::tests::the_part_command_is_operator_only_and_the_readout_is_not` |
| `src/cable.rs:287` — an import saved only the item id and count, so a filled shulker (or a damaged tool) was rebuilt as a fresh stack and lost its state | the importer leaves any stack with components or damage in its container (`round_trips`); the store keeps its id+count shape, and nothing is silently flattened | `cable::tests::a_stack_with_state_is_left_out_of_the_store`; the e2e asserts a filled shulker stays in the chest while a plain stack flows |
| `src/cable.rs:345` — a per-start cap let overlapping traversals repeat a large network's transfers | one **shared** per-tick `seen` set and budget (`walk_component`); a component that does not fit is refused whole and the rest of the tick stops, so no cable is walked or transfers twice | `cable::tests::{a_component_that_fits_is_walked_once, a_component_larger_than_the_shared_budget_is_refused_whole}` |

The `Parts` type was renamed `PartSet` (a port-local type whose name collided with the SDK's
`Parts`, which `fits` flagged as a changed item on the newest Nightly); behaviour is unchanged
and `src/runtime.rs` is otherwise untouched.

**Found while proving the leg:** the e2e still passed `--dev-mod <port>` to a game that already
compiles the package in; removed (the flat `.hmod` is its own content). **Ladder:** 21 tests,
clippy `-D warnings` 0, `fmt --check` clean, two builds byte-identical, `check` and `fits` exit 0;
`launcher-game.sh run integratedtunnels` → `PASS` + `EXIT:0`, `GAME=$CONTROL_GAME` →
`EXIT:1` (`../_shared/evidence/integratedtunnels-e2e/`).

Package 0.2.1: **39,418 B**, SHA-256 `87ab0b4b23892527a1c8ba6550a7b084d3fe058384f35ce860c5427e1639bfaa`;
a second build is byte-identical (`cmp`), `check` and `fits` exit 0.

## Slice 2: the fluid tunnel (0.2.3)

**Gap line** (`## Gaps (next slices)` above, item 2): *"the **fluid and energy** parts and the
fluid/energy network channels"*. This slice takes the **fluid** half. Energy is not attemptable as a
faithful in-game leg at this pin: the game has no vanilla FE block to feed or drain, so a transfer
would have no failure-capable assertion (the API has the energy capability; a proof needs a mod
tank, which this port must not add).

**Why it fits the tier (`deep`).** The jar's Fluid Importer/Exporter are the same face-mounted part
protocol as the item parts already ported (a part on a cable face, moved once per part tick), plus a
second network channel (`FLUIDS` instead of `ITEMS`). No new engine machinery, no entity, no
renderer.

**Upstream read** (`CyclopsMC/IntegratedTunnels`, tag `1.21.1-1.13.0`, commit `dd9ade64e31b`,
re-cloned for this slice; the clone is deleted after these findings):
`part/PartTypeImporterFluid.java`, `part/PartTypeExporterFluid.java`,
`part/aspect/TunnelAspects.java` (`Write.Fluid.BOOLEAN_IMPORT`/`BOOLEAN_EXPORT` are the default
aspects), `part/aspect/TunnelAspectWriteBuilders.java` (`Fluid.PROPERTIES_RATE`: `PROP_RATE`
default **1000 mB**, channel 0, round-robin off, passive IO true, `PROP_CHECK_AMOUNT` false),
`core/TunnelFluidHelpers.java` (`matchAll(rate, exactAmount)`), `GeneralConfig.java`
(`importerFluidBaseConsumption`/`exporterFluidBaseConsumption` = 1 energy/tick, a gap as for the
item parts). Each part moves through `TunnelHelpers.moveSingleStateOptimized` once per tick, so the
importer moves **up to 1000 mB per part per tick**.

### What the slice does

- **`part_importer_fluid` / `part_exporter_fluid`** — the jar's two items, models and textures
  (`importer_fluid_active|inactive`, `exporter_fluid_active|inactive`, copied from the jar), added to
  the cable's `part_<face>` vocabulary and to the creative tab, with the jar's lang lines.
- **Fluid channel** — a slotless fluid store beside the item store, saved on the network's root
  cable as a `fluids` list of `{fluid, amount}` compounds. A fluid id is the resolved registry name
  (or the host's retained name for an unresolved fluid), so a network merges by fluid id and keeps
  first-arrival order, as upstream's channel does.
- **Importer** — reads the tanks of the `FLUIDS` capability behind its face, takes the first tank
  holding fluid up to [`runtime::FLUID_RATE`] (1000 mB) with the engine's `extract`, and records what
  the engine actually took.
- **Exporter** — pushes the channel's first fluid up to 1000 mB with `insert_any`, and removes only
  what the destination accepted. A cauldron takes whole levels, so a partial fill stays in the
  channel.
- **Status** — each channel's parts make the cable active on **their own** channel: an item part
  while the item store holds anything, a fluid part while the fluid channel does. This is stricter
  than 0.2.1's "any part and any items" and is what phase I asserts (a fluid-only network with 1000
  mB shows `active`).
- **Readout** — `/integratedtunnels info <pos>` gains a third line, `…: fluids 1 stack, 1000 mB
  (minecraft:water x1000 mB)`; the two existing lines are unchanged, and the part command accepts
  `none|importer_item|exporter_item|importer_fluid|exporter_fluid`.
- **Same transaction** — the item store and the fluid channel are read once per network per tick,
  written together, and a refused write aborts the whole pass as before.

### Deliberate deviations (fluid)

| Upstream | Here | Why |
|---|---|---|
| The fluid part asks for an `IFluidTarget` (NeoForge fluid handlers, including slotless ones) | the engine's `FLUIDS` capability through the pin's API | the pin's capability is the mod-facing fluid surface; a slotless non-tank source is not reachable |
| The channel stores full `FluidStack`s with data components | the channel stores a fluid id and an amount (no components) | the slice has no NBT aspect; a component-carrying fluid (a potion fluid) is stored as its plain fluid — recorded gap |
| A part consumes 1 energy/tick | no energy | no energy network in this port (unchanged gap) |
| `status=error` for an invalid target | `inactive`/`active` only | unchanged from slice 1 |

### Upstream defects fixed (fluid)

**None found in this slice's path.** The boolean import/export aspect, the 1000 mB default and the
two part recipes were read line by line: no id/tag mismatch, no unreachable recipe shape, no
off-by-one (the rate is the predicate's amount, not a multiplier). The **recipes are not carried**:
they need `integratedtunnels:part_interface_fluid` (the interface family, gap 4) and
`integrateddynamics:variable_transformer_input|output`, which the parent port does not carry — the
same reason the two item part recipes are absent.

### Coverage (whole port, ruling 2026-11-20)

| Family (jar) | State | Proof |
|---|---|---|
| Item Importer / Exporter parts | **tested** | e2e A/A2/B/C/D/E/F/G + `cable::tests` + `runtime::tests` |
| Fluid Importer / Exporter parts | **tested** | e2e I (drain a full cauldron), J (fill an empty one), K1/K2 (each part removed stops its half), L (both items read back); `runtime::tests::*fluid*` |
| Carrier cable block + its part items | **tested** | e2e scene, G, L, `runtime::tests::part_names_round_trip` |
| Carrier cable self-drop loot table | **tested** | e2e M rolls `integratedtunnels:blocks/tunnel_cable` through `/loot give` and reads the cable back |
| `/integratedtunnels info` / `part` commands | **tested** | every phase reads the parsed lines; `part` is level 2 (`cable::tests`) |
| Varying/aspect layer (variable cards, 6 other item aspects, fluid aspects) | **not ported** | gap 1 — the parent's variable system |
| Energy parts and channel | **not ported** | gap 2 (energy half) — no vanilla FE block to prove a transfer against |
| World parts (import/export world item/fluid/energy/block) | **not ported** | gap 3 — needs the fake player and drill |
| Interface parts, player simulator | **not ported** | gap 4 |
| Part screens, part removal/drop, `error` state | **not ported** | gap 5 |
| Network energy | **not ported** | gap 6 |
| Sibling cable (`integrateddynamics:cable`) interop | **not ported** | gap 7 — no cross-mod API at the pin |
| Item NBT fidelity in the store | **not ported** | gap 8 — component stacks stay in their container (e2e H asserts that) |
| Part recipes, advancements, info book | **not ported** | gap 9 — ingredients are content the parent port does not carry |
| Multipart block-entity renderer | **not ported** | gap 10 — parts are drawn by multipart block models |

### Ladder measured for 0.2.3 (`ports/_shared/evidence/integratedtunnels/`)

| Step | Result |
|---|---|
| RED first | `sdk-test-red-fluid.log`: `cargo test` failed to compile against the new tests — `error[E0425]: cannot find value `FLUID_RATE` in this scope`, `error[E0433]: cannot find type `Fluids` in this scope`, `error[E0599]: no variant, associated function, or constant named `ImporterFluid` …` (51 errors) |
| Runtime tests | **28 passed**, 0 failed (`sdk-test.log`, sha + size in the log header) |
| Lints | clippy `--all-targets -- -D warnings` exit 0, **0 warnings** (`sdk-clippy.log`) |
| Formatting | `cargo fmt --check` clean (`sdk-fmt.log`) |
| Package | two builds, `rm -f Cargo.lock` before each: **byte-identical**, 49,232 B, sha256 `1dbd235c13792d22c1478643cc79b1b4f09bf418144f74883125d576dc176fa9`, **46 entries**, no `Cargo.lock` (`sdk-build.log`) |
| Registry check | `integratedtunnels 0.2.3 (native-mod, format native-1/0a875da…/e59da90…, for 26.3)`, 49,232 B, same sha, EXIT 0 (`sdk-check.log`) |
| Fits the pin | `0a875da  made for`, EXIT 0 (`sdk-fits.log`) |
| Fits another build | `0b14f8a  fits`, EXIT 0 (`sdk-fits-other.log`) |
| Gate | `verify-sdk-package.py integratedtunnels` → **1/1** |
| In game | `launcher-game.sh build` → `Compiling integratedtunnels (1 of 1)` (manifest bumped, trap §9d-1); `PASS` + `EXIT:0` (`evidence/integratedtunnels-e2e/e2e-launcher.log`), control (same Nightly, no package) `EXIT:1`, `SELFTEST=1` `EXIT:1`; `check-e2e-logs.py integratedtunnels-e2e --strict` → `logs=3 ok=3` |

**Pin.** This slice repins `[native] horizon` from `0b14f8a` to **`0a875da91dbb0fdcb7ba8c42c11341aa229af51a`**
(mixins unchanged at `e59da909…`). `.horizon/sdk` was **deleted** before the first build so the fetch
is the new commit (trap §9d-17a); the build prints `SDK 0a875da` and `check` names the new pin.

**Version.** The first build of this slice was 0.2.2; it shipped without the two
`assets/integratedtunnels/items/part_{importer,exporter}_fluid.json` item definitions (and with the
slice-1 README), so the items would have rendered as nothing even though the ids worked. 0.2.3 is the
corrected package — those two entries (46 in all, 49,232 B) — and `check`, both `fits` runs and all
three game legs were re-run on it. The game build was forced by the version bump, as the launcher
reuses a build whose `horizon-mod.toml` is byte-identical (trap §9d-1), and its log confirms
`Compiling integratedtunnels (1 of 1)`.

**`src/runtime.rs` is no longer byte-identical to the retired 0.1.0 bundle**, and that is deliberate:
slice 2's rules live in it beside slice 1's (the campaign's rule is one pure unit-tested runtime per
port, and splitting the part vocabulary across two modules would have been worse). The tree and the
packed copy still hash the same (`bf298bd7…dd938b`, `sdk-runtime-sha256.txt`); the retired file is
kept at `ports/_shared/evidence/integratedtunnels/retired-hnmod-0.1.0/mixins/integratedtunnels/runtime.rs`.

**Measured on the way (campaign dialect question, §9d-11/13):** the carried loot table keeps
`"bonus_rolls": 0.0` and `"rolls": 1.0`, the two shapes `check_261_dialect.py` flags as *unverified*,
and phase M proves 26.3 accepts them: `/loot give @s loot integratedtunnels:blocks/tunnel_cable`
returned a tunnel cable in game. They are therefore not fatal for this family and were not rewritten
(the checker's recipe-ingredient and loot-`conditions` shapes remain the fatal ones).

**The e2e's chat readout had to become marker-based.** The chat history holds the newest 100 lines;
the fluid phases pushed the old "count matching lines" wait past that cap, so a fresh reply could no
longer be told from an old one (`FAIL: no /integratedtunnels info fluid reply` on the first two legs).
`read_info`/`set_part` now bracket each command with a unique player-chat marker and read the block of
lines immediately older than it (`tools/e2e.sh`), which is cap-independent.

Skills: superpowers/using-superpowers,brainstorming,writing-plans,test-driven-development,systematic-debugging,verification-before-completion; BeautifulRust/rust-best-practices,rust-port

## Verification (0.1.0, the retired `.hnmod`)

Pin **`1d63e0a5a37a7a708eb0b32b49bf0dfeb28ed8a9`**, mixins
**`5a64af938ec86b00d53d16a408aabc0038fb3e57`** (schema 6). Every build, test and
e2e ran through `ports/tools/build-slot.sh` (`CARGO_INCREMENTAL=0`, no bare
cargo, no `--target-dir`); the in-game runs used the pin-matched shared
`horizon-ctl` (`ports/_shared/bin/horizon-1d63e0a5/horizon-ctl`) and the
port-built fast game, built and run in **one** lease.

### Ladder

| Step | Command (through the slot runner) | Result |
|---|---|---|
| runtime RED → GREEN | `cargo test --offline` in the port (18 tests written first) | first run failed to compile the `todo!()` bodies, then **18 passed**; the lint fix below removed one test with its two redundant predicates, and the final sources run **17 passed**, 0 failed |
| fmt / clippy | `cargo fmt --all --check`; `cargo clippy --all-targets --locked --offline -- -D warnings` | pass, **0 warnings** |
| stage / check / test / build | `python3 tools/horizon.py stage`, `check`, `test`, `build` at the pin | stage green (its report has no `cargo_exit`; it only builds the mixin CLI), then **`cargo_exit: 0` for check, test and build**, **0 warnings**, **17 passed** embedded (`game::integratedtunnels::tests`), 1240 filtered — `ladder-check.json`, `ladder-build.json`, `ladder-test.log` |
| content `.hmod` | `tools/package.sh` (`cargo-horizon-mod build` + `check`) | **9,812 B**, SHA-256 `aa337de3ff37f07acf85eb8d955f9c2fd2ff6c448da2222c262f72c8235003e3`, **24 entries**, "no code: content only" |
| native `.hnmod` | `tools/package.sh` (`tools/package.py`) | **21,682 B**, SHA-256 `0c8bcfe277ef4a179a3291ee68812598bb57ced74463594e5545a98fa5f8e99a`, **13 entries**, embeds `mixins/integratedtunnels/data/integratedtunnels-content.hmod` (byte-identical to `content/`) |
| reproducibility | `tools/package.sh` in the final lease, then `tools/package.py` run twice more against the same sources | byte-identical: `.hmod` `aa337de3…03e` (24 entries) every time, and the rebuilt `.hnmod` reproduces the delivered one byte-for-byte (`0c8bcfe2…e99a`, 21,682 B) — `evidence/integratedtunnels/ladder-package.log` |
| bundle check | `cargo horizon-mod check integratedtunnels.hnmod --kind native-mod` | exit 0, `integratedtunnels 0.1.0 (native-mod, format native-1/1d63e0a5…/5a64af93…, for 26.3)` |
| e2e (one `.hnmod`, embedded, **no `--dev-mod`**) | `EMBEDDED=1 CONTROL=1 tools/e2e.sh`, built and run in one lease | **PASS**, `EXIT:0` — `ports/_shared/evidence/integratedtunnels/e2e-embedded.log` |
| e2e (dev-mod leg, same binary, `--dev-mod content/`) | `tools/e2e.sh` | **PASS**, `EXIT:0` — `e2e-devmod.log` |
| control | the same binary with `.horizon-cache` unusable | `block: minecraft:air` — the ids are unknown without the embedded file, so the embedded leg proves something |
| selftest | `SELFTEST=1 tools/e2e.sh` (phase B expects 6401 conserved, which the network cannot produce) | **FAIL + `EXIT:1`** at phase B: `FAIL: B: store + sink = 6400, expected 6401` — the scenario can fail — `e2e-selftest.log` |
| log check | `python3 ports/tools/check-e2e-logs.py integratedtunnels --strict` | exit **0**: `logs=3 ok=3 missing-exit=0 bad-exit=0` (`e2e-devmod.log` PASS+EXIT:0, `e2e-embedded.log` PASS+EXIT:0, `e2e-selftest.log` FAIL+EXIT:1) |
| skills gate | `python3 ports/tools/check-skills.py --notes ports/integratedtunnels` | `SKILLS GATE: PASS` |
| readiness | `python3 ports/tools/upstream_ready.py integratedtunnels` | **exit 1**, two submission-level blockers outside this job's write ownership: (a) the ledger row is still `pending` — the orchestrator sets `ported-partial`; (b) "add-on whose required parent(s) integrateddynamics are not in the registry – the reviewer closes this (see refinedstorage_quartz_arsenal, #498)". With the status flipped in a scratch copy of the tree, (a) clears and only (b) remains: every per-port gate (artifact in the port, `Skills:` line, `PASS + EXIT:0` e2e, pin, MIT) passes. Evidence: `evidence/integratedtunnels/gates.log` |

Ladder and gate logs: `ports/_shared/evidence/integratedtunnels/` (`ladder-check.log`,
`ladder-test.log`, `ladder-build.log`, `ladder-package.log`,
`ladder-check-native.log`, `ladder-check.json`, `ladder-build.json`, `gates.log`).

**The lint fix.** The first game build reported one `dead_code` warning: `runtime`
exposed `Part::imports`/`Part::exports`, which only their own test used — the tick
selects faces with `Parts::faces(Part::ImporterItem)`/`(Part::ExporterItem)`, which
already says which way the items go. Both predicates and that test were removed
(rather than silenced), the whole ladder was re-run on the final sources, and every
staged step is now **0 warnings**.

### Artifacts (0.2.0, the delivered package)

One deliverable, in the port root (never `target/`):

- `ports/integratedtunnels/integratedtunnels.hmod` — **37,602 B**, SHA-256
  `c72e5302004d32f13772d8ba976c75be208e947624fff312b8d60019d0695e0b`, **32 entries**: the manifest,
  `Cargo.toml`, `LICENSE`, `LICENSE-NOTICE.md`, `README.md`, 28 content files under
  `assets/integratedtunnels/` and `data/integratedtunnels/`, `src/lib.rs`, `src/cable.rs`,
  `src/runtime.rs` and `uses.json`. No `Cargo.lock` (trap 5), no `.horizon/`.

What 0.1.0 delivered (kept only as evidence, both files retired to
`_shared/evidence/integratedtunnels/retired-hnmod-0.1.0/`):

- `integratedtunnels.hnmod` — **21,682 B**, SHA-256
  `0c8bcfe277ef4a179a3291ee68812598bb57ced74463594e5545a98fa5f8e99a`,
  **13 entries**: `horizon-native.json`, `horizon-mixin.toml`, 10 `.rs` files and
  `mixins/integratedtunnels/data/integratedtunnels-content.hmod` (**9,812 B**,
  SHA-256 `aa337de3…03e`, byte-identical to `content/integratedtunnels.hmod`).
- `content/integratedtunnels.hmod` — the content half, embedded above.

### In-game scenario (quoted from `evidence/integratedtunnels/e2e-embedded.log`)

```
== A: a source chest drains into the network store, at a bounded rate ==
  Tunnel Cable at 200,101,0: parts down=none,up=none,north=none,south=none,west=importer_item,east=none
  Tunnel Cable at 200,101,0: store 1 stack, 384 items (minecraft:cobblestone x384); status active
  rate: 384 items imported within ~15 ticks (64 per tick in the unit tests)
== A2: the source empties into the store ==
  Tunnel Cable at 200,101,0: store 1 stack, 6400 items (minecraft:cobblestone x6400); status active
== B: an exporter hands the store to a sink chest, and nothing is lost ==
  Tunnel Cable at 201,101,0: parts …east=exporter_item…
  Tunnel Cable at 201,101,0: store 1 stack, 4672 items (minecraft:cobblestone x4672); status active
  has(202,101,0) minecraft:cobblestone x1728: ok
== C: with the exporter removed the store grows and the sink stays put ==
  Tunnel Cable at 200,101,0: store 1 stack, 4736 items (minecraft:cobblestone x4736); status active
== D: with the importer removed a full source chest stays full ==
  Tunnel Cable at 200,101,0: parts …west=none…; store 4736 items; status inactive
== E: both parts back, the source drains into the store again ==
  Tunnel Cable at 200,101,0: store 1 stack, 4800 items (minecraft:cobblestone x4800); status active
== F: a part on a face with no container behind it does nothing ==
  Tunnel Cable at 206,102,0: store 0 items; status inactive
== G: right-clicking a cable face with the Item Importer places the part ==
  Tunnel Cable at 206,102,4: parts …west=importer_item…
  embedded content written: …/integratedtunnels-embedded-content.hmod (9812 B) sha256=aa337de3…03e
  embedded content is byte-identical to the built content/ mod
== control: the same binary with an unusable .horizon-cache ==
  block: minecraft:air
PASS: an Item Importer pulled a chest into the tunnel network's store at a bounded rate, an Item Exporter handed it to a second chest without losing any (6400), and removing either part stopped its half of the loop
EXIT:0
```

What the scenario proves: 6,400 items behind the importer's face move into the
network store **not in one tick** (384 at the first sample, far below 6,400 — the
64-per-tick rate), the source empties and the store holds exactly what the chest
held; an exporter on another cable face hands the store to a second chest that
fills to its own capacity with **nothing lost** (store + sink = 6,400); removing
the exporter stops the sink and lets the store grow; removing the importer leaves
a full chest full; a part with no container behind its face does nothing; and
upstream's right-click placement works — **from the one `.hnmod` alone, with no
`--dev-mod`** — while the control (content cache blocked) registers nothing.
