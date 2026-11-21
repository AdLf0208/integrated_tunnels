//! Game glue for the tunnel cable: its part slots, the network's item store, the
//! per-tick import/export pass and the `/integratedtunnels` readout.
//!
//! Upstream a tunnel part is a part on an Integrated Dynamics cable: the cable's
//! block entity holds one part per face, an Item Importer moves items from the
//! inventory on its face into the network's slotless item channel, and an Item
//! Exporter moves them from that channel into the inventory on its face
//! (`PartTypeImporterItem`, `TunnelAspectWriteBuilders.Item.PROP_IMPORT/EXPORT`).
//! There is no cross-mod API at this pin and two packs cannot share a namespace,
//! so this crate re-implements the cable it needs: a
//! `integratedtunnels:tunnel_cable` carrier whose six `part_<face>` state
//! properties hold the parts, whose block entity holds the network's item store,
//! and whose level-tick end runs the network's transfers.
//!
//! A cable's own tick only **queues** it: the store is read and written once per
//! network at `level_tick_post`, in one transaction, so a network of many cables
//! moves items once per tick and not once per cable. The item store lives on the
//! network's root cable (the lowest position), so every cable of one network
//! agrees without extra state.

use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, Mutex};

use mod_api::{
    BlockEntityPart, BlockEntityTickEvent, BlockEntityTicksEvent, BlockKey, BlockPart, Capability,
    CommandArgumentValue, CommandInvocation, Compound, Direction, FLUIDS, FluidIdentity,
    FluidResource, FluidStack, Hand, Host, HostTransaction, ITEMS, Identifier, ItemPlace,
    ItemResource, ItemStack, LevelRef, ResourceHandler, ServerHost, Tag, Text,
};

use crate::runtime::{
    FACE_COUNT, FACE_NAMES, FLUID_RATE, FluidImport, Fluids, ITEM_RATE, ImportMove, Part, PartSet,
    SlotView, Store, TankView, collect_network, commit_export, commit_fluid_export,
    commit_fluid_import, commit_import, describe_fluids, describe_store, face_of_normal, opposite,
    plan_export, plan_fluid_export, plan_fluid_import, plan_import, root, status_for,
};

/// The tunnel cable carrier block (and its item).
pub(crate) const CABLE: &str = "integratedtunnels:tunnel_cable";
/// The Item Importer part item.
pub(crate) const IMPORTER: &str = "integratedtunnels:part_importer_item";
/// The Item Exporter part item.
pub(crate) const EXPORTER: &str = "integratedtunnels:part_exporter_item";
/// The Fluid Importer part item.
pub(crate) const IMPORTER_FLUID: &str = "integratedtunnels:part_importer_fluid";
/// The Fluid Exporter part item.
pub(crate) const EXPORTER_FLUID: &str = "integratedtunnels:part_exporter_fluid";
/// The network item store, saved in the root cable's block entity: a list of
/// `{item, count}` compounds, in first-arrival order.
const STORE_TAG: &str = "store";
const ITEM_TAG: &str = "item";
const COUNT_TAG: &str = "count";
/// The network fluid channel, saved beside the item store: a list of
/// `{fluid, amount}` compounds, in first-arrival order.
const FLUIDS_TAG: &str = "fluids";
const FLUID_TAG: &str = "fluid";
const AMOUNT_TAG: &str = "amount";
/// A safety cap on one network's cable count (upstream has no limit; a runaway
/// build must not stall the level tick).
const MAX_NETWORK: usize = 1024;

/// Cables whose block entity ticked this tick, per level, drained at the end of
/// that level's tick.
#[derive(Default)]
pub(crate) struct Queue {
    pending: HashMap<LevelRef, BTreeSet<[i32; 3]>>,
}

impl Queue {
    /// Queue `pos` for the level the cable's tick ran in.
    fn push(&mut self, level: LevelRef, pos: [i32; 3]) {
        self.pending.entry(level).or_default().insert(pos);
    }

    /// Everything queued for `level`, in position order (empty afterwards).
    pub(crate) fn take(&mut self, level: LevelRef) -> BTreeSet<[i32; 3]> {
        self.pending.remove(&level).unwrap_or_default()
    }
}

/// The part this mod attaches to every tunnel cable's block entity type: it makes
/// the cable tick, and its tick only queues the cable for the level tick's end.
pub(crate) struct Cable {
    queue: Arc<Mutex<Queue>>,
}

impl Cable {
    pub(crate) fn new(queue: Arc<Mutex<Queue>>) -> Cable {
        Cable { queue }
    }
}

#[mod_api::horizon_part]
impl BlockPart for Cable {}

#[mod_api::horizon_part]
impl BlockEntityPart for Cable {
    /// Every tunnel cable ticks on the server.
    fn block_entity_ticks(&self, _host: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        event.ticks = true;
    }

    /// One cable tick: queue it for the level tick's end. The tick itself writes
    /// no block entity data; the network's pass at the end of the tick does.
    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        let Some(level) = level_in_place(host) else {
            return;
        };
        // Only this expression runs under the lock: one insert on the port's own
        // state, which cannot panic and so cannot poison it.
        self.queue
            .lock()
            .expect("the tunnel queue lock is held for one insert")
            .push(level, event.pos);
    }
}

/// The level the server is ticking now: the one a block-entity tick runs in.
fn level_in_place(host: &dyn ServerHost) -> Option<LevelRef> {
    host.levels()
        .list_levels()
        .into_iter()
        .find(|&level| level.in_place(host))
}

/// The block state property of `face` (`part_down` … `part_east`).
fn face_property(face: usize) -> String {
    format!("part_{}", FACE_NAMES[face])
}

/// The cable's block key, resolved once per pass (`BlockKey` is the engine's own
/// identity: comparing it costs no string).
fn cable_key(host: &dyn Host) -> Option<BlockKey> {
    host.block_key(CABLE)
}

/// Whether `pos` holds the tunnel cable.
fn is_cable(host: &dyn Host, cable: BlockKey, level: LevelRef, pos: [i32; 3]) -> bool {
    level.block(host, pos) == Some(cable)
}

/// The parts a cable carries, or `None` for another block.
fn parts_at(host: &dyn Host, cable: BlockKey, level: LevelRef, pos: [i32; 3]) -> Option<PartSet> {
    if !is_cable(host, cable, level, pos) {
        return None;
    }
    let mut parts = PartSet::new();
    for face in 0..FACE_COUNT {
        let value = level.block_property(host, pos, &face_property(face))?;
        parts = parts.with(face, Part::from_name(&value)?);
    }
    Some(parts)
}

/// The network's item store, as saved in the root cable's block entity.
fn store_at(host: &dyn Host, level: LevelRef, pos: [i32; 3]) -> Store {
    let Some(entity) = host.block_entity(level, pos) else {
        return Store::new();
    };
    let Some(data) = entity.data(host) else {
        return Store::new();
    };
    let Some(Tag::List(entries)) = data.get(&STORE_TAG.into()) else {
        return Store::new();
    };
    let mut store = Store::new();
    for entry in entries {
        let Some(Tag::String(item)) = entry.get(ITEM_TAG) else {
            continue;
        };
        let Some(Tag::Int(count)) = entry.get(COUNT_TAG) else {
            continue;
        };
        if *count > 0 {
            let Ok(count) = u32::try_from(*count) else {
                continue;
            };
            store.add(&item.to_string_lossy(), count);
        }
    }
    store
}

/// The network's fluid channel, as saved in the root cable's block entity.
fn fluid_store_at(host: &dyn Host, level: LevelRef, pos: [i32; 3]) -> Fluids {
    let Some(entity) = host.block_entity(level, pos) else {
        return Fluids::new();
    };
    let Some(data) = entity.data(host) else {
        return Fluids::new();
    };
    let Some(Tag::List(entries)) = data.get(&FLUIDS_TAG.into()) else {
        return Fluids::new();
    };
    let mut fluids = Fluids::new();
    for entry in entries {
        let Some(Tag::String(fluid)) = entry.get(FLUID_TAG) else {
            continue;
        };
        let Some(Tag::Int(amount)) = entry.get(AMOUNT_TAG) else {
            continue;
        };
        if *amount > 0 {
            let Ok(amount) = u32::try_from(*amount) else {
                continue;
            };
            fluids.add(&fluid.to_string_lossy(), amount);
        }
    }
    fluids
}

/// The store as NBT (the shape [`store_at`] reads back).
fn store_tag(store: &Store) -> Tag {
    let entries = store
        .entries()
        .iter()
        .map(|stack| {
            Tag::compound([
                (ITEM_TAG, Tag::String(stack.item.as_str().into())),
                (
                    COUNT_TAG,
                    Tag::Int(i32::try_from(stack.count).unwrap_or(i32::MAX)),
                ),
            ])
        })
        .collect();
    Tag::List(entries)
}

/// The fluid channel as NBT (the shape [`fluid_store_at`] reads back).
fn fluids_tag(fluids: &Fluids) -> Tag {
    let entries = fluids
        .entries()
        .iter()
        .map(|entry| {
            Tag::compound([
                (FLUID_TAG, Tag::String(entry.fluid.as_str().into())),
                (
                    AMOUNT_TAG,
                    Tag::Int(i32::try_from(entry.amount).unwrap_or(i32::MAX)),
                ),
            ])
        })
        .collect();
    Tag::List(entries)
}

/// Save a network's item store and fluid channel on its root cable, as
/// `/data merge block` does: it loads the merged data and tells clients, and it
/// is part of the caller's transaction.
fn set_stores(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: [i32; 3],
    items: &Store,
    fluids: &Fluids,
) -> bool {
    let Some(entity) = host.block_entity(level, pos) else {
        return false;
    };
    let mut data = Compound::new();
    data.insert(STORE_TAG.into(), store_tag(items));
    data.insert(FLUIDS_TAG.into(), fluids_tag(fluids));
    // A refused merge aborts the pass's transaction, so nothing leaves or enters a
    // container without the network recording it. `Ok(false)` is "no change" here:
    // both compounds are depth 3 and every entry is a string and an int, so the
    // other two causes it names (too deep, cannot load) cannot apply to them.
    entity.merge_data(host, data).is_ok()
}

/// Show the network's part status on a cable (the part models' active/inactive
/// texture); a no-op when it is already right. The status is cosmetic: the
/// command readout derives it from the store.
fn show_status(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: [i32; 3],
    parts: PartSet,
    store: &Store,
    fluids: &Fluids,
) {
    let status = status_for(parts, store, fluids).name();
    if level.block_property(host, pos, "status").as_deref() != Some(status) {
        host.set_block_property(pos, "status", status);
    }
}

/// The side of the neighbouring block that touches the part on `face`.
fn touching_side(face: usize) -> Direction {
    Direction::ALL[opposite(face)]
}

/// The container behind `face`, as the engine's side-filtered item handler.
fn container(
    host: &mut dyn ServerHost,
    level: LevelRef,
    cable: [i32; 3],
    face: usize,
) -> Option<Capability<dyn ResourceHandler<ItemResource>>> {
    level.capability(
        host,
        &ITEMS,
        crate::runtime::neighbours(cable)[face],
        Some(touching_side(face)),
    )
}

/// The tank behind `face`, as the engine's side-filtered fluid handler (a
/// cauldron answers as one tank).
fn fluid_container(
    host: &mut dyn ServerHost,
    level: LevelRef,
    cable: [i32; 3],
    face: usize,
) -> Option<Capability<dyn ResourceHandler<FluidResource>>> {
    level.capability(
        host,
        &FLUIDS,
        crate::runtime::neighbours(cable)[face],
        Some(touching_side(face)),
    )
}

/// One importer's move this tick: up to [`ITEM_RATE`] items from the first slot
/// the face may take from, into the network's store.
fn import_once(
    host: &mut dyn ServerHost,
    level: LevelRef,
    cable: [i32; 3],
    face: usize,
    store: &mut Store,
) {
    let Some(container) = container(host, level, cable, face) else {
        return;
    };
    // Every slot this handler lists is one the side reaches, so it is takeable
    // through that side; the extract is what the engine has the last word on.
    // A stack the store cannot round-trip (components or damage) is left alone:
    // the store keeps an item id and a count, and rebuilding a fresh stack from
    // those would lose a shulker's contents or a tool's wear.
    let views: Vec<SlotView> = (0..container.size(host))
        .filter_map(|slot| {
            let resource = container.resource(host, slot)?;
            let stack = resource.as_stack();
            if !round_trips(stack.components().is_some(), stack.damage()) {
                return None;
            }
            let count = container.amount(host, slot);
            Some(SlotView {
                slot,
                item: resource.item().id(host),
                count,
                can_take: true,
            })
        })
        .collect();
    let Some(plan) = plan_import(&views, ITEM_RATE) else {
        return;
    };
    let Some(resource) = container.resource(host, plan.slot) else {
        return;
    };
    let taken = host
        .transaction(|host, tx| {
            Ok::<_, ()>(container.extract(host, tx, plan.slot, &resource, plan.count))
        })
        .ok()
        .unwrap_or(0);
    if taken == 0 {
        return;
    }
    // The engine is the truth about what actually left the container, and the
    // resource it named is the truth about which item did.
    commit_import(
        store,
        &ImportMove {
            slot: plan.slot,
            item: resource.item().id(host),
            count: taken,
        },
    );
}

/// One exporter's move this tick: up to [`ITEM_RATE`] items from the store into
/// the inventory on its face. What the destination refuses stays in the store.
fn export_once(
    host: &mut dyn ServerHost,
    level: LevelRef,
    cable: [i32; 3],
    face: usize,
    store: &mut Store,
) {
    let Some(plan) = plan_export(store, ITEM_RATE) else {
        return;
    };
    let Ok(item) = Identifier::parse(&plan.item) else {
        return;
    };
    let Some(item) = host.item_key(&item) else {
        return;
    };
    let Some(stack) = ItemStack::new(item, 1) else {
        return;
    };
    let resource = ItemResource::of(&stack);
    let Some(container) = container(host, level, cable, face) else {
        return;
    };
    let inserted = host
        .transaction(|host, tx| Ok::<_, ()>(container.insert_any(host, tx, &resource, plan.count)))
        .ok()
        .unwrap_or(0);
    commit_export(store, &plan, inserted);
}

/// The id of a tank's fluid, or `None` for one this build cannot name.
fn fluid_name(host: &dyn Host, resource: &FluidResource) -> Option<String> {
    match resource.fluid() {
        FluidIdentity::Resolved(key) => Some(key.id(host)),
        FluidIdentity::Unresolved(fluid) => Some(fluid.id(host)),
        // `FluidIdentity` is non-exhaustive: a future identity is not one we can store.
        _ => None,
    }
}

/// One fluid importer's move this tick: up to [`FLUID_RATE`] millibuckets from
/// the first tank holding fluid on its face into the network's fluid channel.
fn import_fluid_once(
    host: &mut dyn ServerHost,
    level: LevelRef,
    cable: [i32; 3],
    face: usize,
    fluids: &mut Fluids,
) {
    let Some(container) = fluid_container(host, level, cable, face) else {
        return;
    };
    let tanks: Vec<TankView> = (0..container.size(host))
        .filter_map(|tank| {
            let resource = container.resource(host, tank)?;
            let fluid = fluid_name(host, &resource)?;
            Some(TankView {
                tank,
                fluid,
                amount: container.amount(host, tank),
            })
        })
        .collect();
    let Some(plan) = plan_fluid_import(&tanks, FLUID_RATE) else {
        return;
    };
    let Some(resource) = container.resource(host, plan.tank) else {
        return;
    };
    let id = fluid_name(host, &resource).unwrap_or_else(|| plan.fluid.clone());
    let taken = host
        .transaction(|host, tx| {
            Ok::<_, ()>(container.extract(host, tx, plan.tank, &resource, plan.amount))
        })
        .ok()
        .unwrap_or(0);
    if taken == 0 {
        return;
    }
    // The engine is the truth about what actually left the tank, and the fluid it
    // named is the truth about which fluid did.
    commit_fluid_import(
        fluids,
        &FluidImport {
            tank: plan.tank,
            fluid: id,
            amount: taken,
        },
    );
}

/// One fluid exporter's move this tick: up to [`FLUID_RATE`] millibuckets from
/// the fluid channel into the tank on its face. What the destination refuses
/// stays in the channel (a cauldron takes whole levels).
fn export_fluid_once(
    host: &mut dyn ServerHost,
    level: LevelRef,
    cable: [i32; 3],
    face: usize,
    fluids: &mut Fluids,
) {
    let Some(plan) = plan_fluid_export(fluids, FLUID_RATE) else {
        return;
    };
    let Some(key) = host.fluid_key(&plan.fluid) else {
        return;
    };
    let stack = FluidStack::new(key, 1);
    let resource = FluidResource::of(&stack);
    let Some(container) = fluid_container(host, level, cable, face) else {
        return;
    };
    let inserted = host
        .transaction(|host, tx| Ok::<_, ()>(container.insert_any(host, tx, &resource, plan.amount)))
        .ok()
        .unwrap_or(0);
    commit_fluid_export(fluids, &plan, inserted);
}

/// Run every queued cable network's transfers at the end of the level's tick.
/// Every importer takes from the inventory on its face into the network's store,
/// then every exporter hands the store on, so items imported this tick can leave
/// the same tick (as upstream's parts, each written once per tick).
pub(crate) fn run_networks(host: &mut dyn ServerHost, level: LevelRef, queued: BTreeSet<[i32; 3]>) {
    if queued.is_empty() {
        return;
    }
    let Some(cable) = cable_key(host) else {
        return;
    };
    // One shared traversal budget for the level tick, and one `seen` set: a cable is
    // walked once even when every cable of its network queued itself, so a part
    // cannot transfer twice in a tick. A component the budget cannot hold is
    // refused whole (nothing of it moves) instead of being cut into overlapping
    // pieces; the remaining queued starts are refused too, consistently.
    let mut seen: BTreeSet<[i32; 3]> = BTreeSet::new();
    let mut budget = MAX_NETWORK;
    for start in queued {
        if seen.contains(&start) || !is_cable(host, cable, level, start) {
            continue;
        }
        let Some(network) = walk_component(
            start,
            |pos| is_cable(host, cable, level, pos),
            &mut seen,
            &mut budget,
        ) else {
            break;
        };
        let Some(store_pos) = root(&network) else {
            continue;
        };
        // One transaction per network per tick: either every move of this tick
        // happens and the store records it, or none does. A refused store write
        // (an unloaded chunk, a replaced block entity) therefore moves nothing.
        let pass = host.transaction(|host, _tx| {
            let mut store = store_at(host, level, store_pos);
            let mut fluids = fluid_store_at(host, level, store_pos);
            for &pos in &network {
                let Some(parts) = parts_at(host, cable, level, pos) else {
                    continue;
                };
                for face in parts.faces(Part::ImporterItem) {
                    import_once(host, level, pos, face, &mut store);
                }
                for face in parts.faces(Part::ImporterFluid) {
                    import_fluid_once(host, level, pos, face, &mut fluids);
                }
            }
            for &pos in &network {
                let Some(parts) = parts_at(host, cable, level, pos) else {
                    continue;
                };
                for face in parts.faces(Part::ExporterItem) {
                    export_once(host, level, pos, face, &mut store);
                }
                for face in parts.faces(Part::ExporterFluid) {
                    export_fluid_once(host, level, pos, face, &mut fluids);
                }
            }
            match set_stores(host, level, store_pos, &store, &fluids) {
                true => Ok((store, fluids)),
                false => Err(()),
            }
        });
        // An aborted pass moved nothing, so the stores it left on disk are the ones
        // the status has to describe.
        let (store, fluids) = match pass {
            Ok(stores) => stores,
            Err(()) => (
                store_at(host, level, store_pos),
                fluid_store_at(host, level, store_pos),
            ),
        };
        for &pos in &network {
            let Some(parts) = parts_at(host, cable, level, pos) else {
                continue;
            };
            show_status(host, level, pos, parts, &store, &fluids);
        }
    }
}

/// Allocates the level a click or a command acts in: the acting player's own.
fn acting_level(host: &dyn ServerHost) -> Option<LevelRef> {
    let player = host.players().acting_player()?;
    player.level(host)
}

/// What a right click with a part item did.
pub(crate) enum Placement {
    /// The click was not this mod's business.
    NotMine,
    /// The part went onto `face`, and one item left the player's hand.
    Placed { face: usize, part: Part },
    /// The face already holds a part; the (unported) screen would open.
    Taken { face: usize },
}

/// Right-clicking a cable face with a part item places that part on the face
/// (upstream's `ItemPart.useOn` + `PartHelpers.addPart`); the item is consumed
/// outside creative.
pub(crate) fn place_part_from_item(
    host: &mut dyn ServerHost,
    pos: [i32; 3],
    normal: [i32; 3],
    held: &ItemStack,
    player: u128,
) -> Placement {
    let Some(level) = acting_level(host) else {
        return Placement::NotMine;
    };
    let Some(cable) = cable_key(host) else {
        return Placement::NotMine;
    };
    if !is_cable(host, cable, level, pos) {
        return Placement::NotMine;
    }
    let Some(face) = face_of_normal(normal) else {
        return Placement::NotMine;
    };
    let part = match held.item().id(host).as_str() {
        IMPORTER => Part::ImporterItem,
        EXPORTER => Part::ExporterItem,
        IMPORTER_FLUID => Part::ImporterFluid,
        EXPORTER_FLUID => Part::ExporterFluid,
        _ => return Placement::NotMine,
    };
    let property = face_property(face);
    if level.block_property(host, pos, &property).as_deref() != Some("none") {
        return Placement::Taken { face };
    }
    if !host.set_block_property(pos, &property, part.name()) {
        return Placement::NotMine;
    }
    // `ItemPlace::extract` on a creative player's hand gives without losing any,
    // which is the rule this replaces (`game_mode != 1`).
    let resource = ItemResource::of(held);
    let place = ItemPlace::PlayerHand {
        player,
        hand: Hand::Main,
    };
    let _ = host.transaction(|host, tx| Ok::<_, ()>(place.extract(host, tx, &resource, 1)));
    Placement::Placed { face, part }
}

/// The parts of a cable as `down=none,north=importer_item`.
fn parts_line(parts: PartSet) -> String {
    (0..FACE_COUNT)
        .map(|face| {
            format!(
                "{}={}",
                FACE_NAMES[face],
                parts.get(face).unwrap_or(Part::None).name()
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}

/// `/integratedtunnels info <pos>`: the parts, the network's item store and its
/// status.
pub(crate) fn info(host: &mut dyn ServerHost, command: &CommandInvocation) {
    let Some(level) = acting_level(host) else {
        feedback(host, "Integrated Tunnels: no level to read.");
        return;
    };
    let Some(pos) = command_pos(command) else {
        feedback(host, "Usage: /integratedtunnels info <x> <y> <z>");
        return;
    };
    let Some(cable) = cable_key(host) else {
        return;
    };
    let Some(parts) = parts_at(host, cable, level, pos) else {
        feedback(host, "Integrated Tunnels: no tunnel cable there.");
        return;
    };
    feedback(
        host,
        &format!(
            "Tunnel Cable at {},{},{}: parts {}",
            pos[0],
            pos[1],
            pos[2],
            parts_line(parts)
        ),
    );
    let network = collect_network(
        pos,
        |other| is_cable(host, cable, level, other),
        MAX_NETWORK,
    );
    let store = root(&network)
        .map(|store_pos| store_at(host, level, store_pos))
        .unwrap_or_default();
    let fluids = root(&network)
        .map(|store_pos| fluid_store_at(host, level, store_pos))
        .unwrap_or_default();
    let status = status_for(parts, &store, &fluids);
    feedback(
        host,
        &format!(
            "Tunnel Cable at {},{},{}: store {}; status {}",
            pos[0],
            pos[1],
            pos[2],
            describe_store(&store),
            status.name()
        ),
    );
    feedback(
        host,
        &format!(
            "Tunnel Cable at {},{},{}: fluids {}",
            pos[0],
            pos[1],
            pos[2],
            describe_fluids(&fluids)
        ),
    );
}

/// `/integratedtunnels part <pos> <face> <part>`: the harness path for placing or
/// removing a part, since `ctl interact` only reaches a visible face.
pub(crate) fn set_part(host: &mut dyn ServerHost, command: &CommandInvocation) {
    let Some(level) = acting_level(host) else {
        feedback(host, "Integrated Tunnels: no level to write.");
        return;
    };
    let Some(pos) = command_pos(command) else {
        feedback(
            host,
            "Usage: /integratedtunnels part <x> <y> <z> <face> <none|importer_item|exporter_item|importer_fluid|exporter_fluid>",
        );
        return;
    };
    let Some(name) = command_word(command, "face") else {
        feedback(
            host,
            "Usage: /integratedtunnels part <x> <y> <z> <face> <part>",
        );
        return;
    };
    let Some(face) = FACE_NAMES.iter().position(|known| *known == name) else {
        feedback(
            host,
            &format!("Integrated Tunnels: unknown face '{name}' (down|up|north|south|west|east)"),
        );
        return;
    };
    let Some(wanted) = command_word(command, "part") else {
        feedback(
            host,
            "Usage: /integratedtunnels part <x> <y> <z> <face> <part>",
        );
        return;
    };
    let Some(part) = Part::from_name(wanted) else {
        feedback(
            host,
            &format!(
                "Integrated Tunnels: unknown part '{wanted}' (none|importer_item|exporter_item|importer_fluid|exporter_fluid)"
            ),
        );
        return;
    };
    let Some(cable) = cable_key(host) else {
        return;
    };
    if parts_at(host, cable, level, pos).is_none() {
        feedback(host, "Integrated Tunnels: no tunnel cable there.");
        return;
    }
    if !host.set_block_property(pos, &face_property(face), part.name()) {
        feedback(host, "Integrated Tunnels: the cable refused that part.");
        return;
    }
    feedback(
        host,
        &format!(
            "Tunnel Cable at {},{},{}: {} on the {} side",
            pos[0],
            pos[1],
            pos[2],
            part.name(),
            FACE_NAMES[face]
        ),
    );
}

/// The two usages and the level each is registered at. `part` changes a cable at
/// coordinates and consumes no part item, so it is a gamemaster tool (level 2), as
/// `/data` is; `info` is a readout.
pub(crate) fn usages() -> [(&'static str, u8); 2] {
    [
        ("integratedtunnels info <pos:block_pos>", 0),
        (
            "integratedtunnels part <pos:block_pos> <face:word> <part:word>",
            2,
        ),
    ]
}

/// Walk one connected component of cables from `start`, spending a **shared**
/// per-tick `budget` and recording every visited position in `seen`. `None` when
/// the budget runs out: the component is bigger than a level tick may walk, so it
/// is refused as a whole (the caller moves nothing of it) rather than processed in
/// overlapping pieces that would repeat its transfers.
fn walk_component(
    start: [i32; 3],
    is_cable: impl Fn([i32; 3]) -> bool,
    seen: &mut BTreeSet<[i32; 3]>,
    budget: &mut usize,
) -> Option<Vec<[i32; 3]>> {
    let mut component = Vec::new();
    let mut queue = vec![start];
    while let Some(pos) = queue.pop() {
        if !seen.insert(pos) {
            continue;
        }
        if *budget == 0 {
            return None;
        }
        *budget -= 1;
        component.push(pos);
        for next in crate::runtime::neighbours(pos) {
            if !seen.contains(&next) && is_cable(next) {
                queue.push(next);
            }
        }
    }
    Some(component)
}

/// Whether the network store can round-trip a stack. The store saves an item id
/// and a count (`store_tag`), so a component-bearing stack (a filled shulker, an
/// enchanted book) or a damaged one would come back as a fresh item; the importer
/// leaves those in their container instead of destroying their state.
fn round_trips(has_components: bool, damage: u32) -> bool {
    !has_components && damage == 0
}

/// The `pos` argument of a command as a block position.
fn command_pos(command: &CommandInvocation) -> Option<[i32; 3]> {
    match command.argument("pos")? {
        CommandArgumentValue::BlockPos(pos) => Some(*pos),
        _ => None,
    }
}

/// A `word` argument of a command.
fn command_word<'a>(command: &'a CommandInvocation, name: &str) -> Option<&'a str> {
    match command.argument(name)? {
        CommandArgumentValue::String(word) => Some(word),
        _ => None,
    }
}

/// Answer the command's source (its silence, track-output and swapped-in player
/// rules apply, as for any command).
fn feedback(host: &mut dyn ServerHost, line: &str) {
    let _ = host.commands().feedback(Text::literal(line));
}

/// A reply to the player who right-clicked, outside the command path.
pub(crate) fn tell(host: &mut dyn ServerHost, player: u128, line: &str) {
    let Some(player) = host.player(player) else {
        return;
    };
    let _ = player.tell(host, Text::literal(line));
}

/// What a placement or a refused placement should say.
pub(crate) fn placement_line(face: usize, part: Part) -> String {
    format!(
        "Integrated Tunnels: {} on the {} side",
        part.name(),
        FACE_NAMES[face]
    )
}

/// The line a face that already holds a part answers with.
pub(crate) fn taken_face_line(face: usize) -> String {
    format!(
        "Integrated Tunnels: this cable face already holds a part ({})",
        FACE_NAMES[face]
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A straight line of `len` cables along +x at y=0, z=0.
    fn line(len: i32) -> impl Fn([i32; 3]) -> bool {
        move |pos| pos[1] == 0 && pos[2] == 0 && (0..len).contains(&pos[0])
    }

    #[test]
    fn the_part_command_is_operator_only_and_the_readout_is_not() {
        let usages = usages();
        assert_eq!(usages[0].1, 0, "info is a readout");
        assert_eq!(
            usages[1].1, 2,
            "part rewrites another player's cable at coordinates: it must be level 2"
        );
        assert!(usages[1].0.contains(" part "));
    }

    #[test]
    fn a_component_that_fits_is_walked_once() {
        let mut seen = BTreeSet::new();
        let mut budget = 8;
        let component = walk_component([0, 0, 0], line(5), &mut seen, &mut budget)
            .expect("five cables fit in eight");
        assert_eq!(component.len(), 5);
        assert_eq!(budget, 3);
        // The other cables of the same network were visited by that walk, so a
        // queued start inside it does not walk (or transfer) them again.
        let again = walk_component([3, 0, 0], line(5), &mut seen, &mut budget)
            .expect("an already-seen start costs nothing");
        assert!(again.is_empty(), "got {again:?}");
        assert_eq!(budget, 3);
    }

    #[test]
    fn a_component_larger_than_the_shared_budget_is_refused_whole() {
        let mut seen = BTreeSet::new();
        let mut budget = 4;
        assert_eq!(
            walk_component([0, 0, 0], line(20), &mut seen, &mut budget),
            None,
            "a component over the budget must be refused, not processed in pieces"
        );
        assert_eq!(
            budget, 0,
            "the budget is spent, so the rest of the tick stops"
        );
    }

    #[test]
    fn a_stack_with_state_is_left_out_of_the_store() {
        assert!(round_trips(false, 0), "a plain item round-trips");
        assert!(
            !round_trips(true, 0),
            "a filled shulker must not be flattened"
        );
        assert!(!round_trips(false, 3), "a worn tool must keep its damage");
    }
}
