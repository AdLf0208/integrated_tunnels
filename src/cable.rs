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
    CommandArgumentValue, CommandInvocation, Compound, Direction, Hand, Host, HostTransaction,
    ITEMS, Identifier, ItemPlace, ItemResource, ItemStack, LevelRef, ResourceHandler, ServerHost,
    Tag, Text,
};

use crate::runtime::{
    FACE_COUNT, FACE_NAMES, ITEM_RATE, ImportMove, Part, Parts, SlotView, Store, collect_network,
    commit_export, commit_import, describe_store, face_of_normal, opposite, plan_export,
    plan_import, root, status_for,
};

/// The tunnel cable carrier block (and its item).
pub(crate) const CABLE: &str = "integratedtunnels:tunnel_cable";
/// The Item Importer part item.
pub(crate) const IMPORTER: &str = "integratedtunnels:part_importer_item";
/// The Item Exporter part item.
pub(crate) const EXPORTER: &str = "integratedtunnels:part_exporter_item";
/// The network item store, saved in the root cable's block entity: a list of
/// `{item, count}` compounds, in first-arrival order.
const STORE_TAG: &str = "store";
const ITEM_TAG: &str = "item";
const COUNT_TAG: &str = "count";
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
fn parts_at(host: &dyn Host, cable: BlockKey, level: LevelRef, pos: [i32; 3]) -> Option<Parts> {
    if !is_cable(host, cable, level, pos) {
        return None;
    }
    let mut parts = Parts::new();
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

/// Save a network's item store on its root cable, as `/data merge block` does: it
/// loads the merged data and tells clients, and it is part of the caller's
/// transaction.
fn set_store(host: &mut dyn ServerHost, level: LevelRef, pos: [i32; 3], store: &Store) -> bool {
    let Some(entity) = host.block_entity(level, pos) else {
        return false;
    };
    let mut data = Compound::new();
    data.insert(STORE_TAG.into(), store_tag(store));
    // A refused merge aborts the pass's transaction, so no item leaves or enters a
    // container without the store recording it. `Ok(false)` is "no change" here:
    // this compound's depth is 3 and every entry is a string and an int, so the
    // other two causes it names (too deep, cannot load) cannot apply to it.
    entity.merge_data(host, data).is_ok()
}

/// Show the network's part status on a cable (the part models' active/inactive
/// texture); a no-op when it is already right. The status is cosmetic: the
/// command readout derives it from the store.
fn show_status(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: [i32; 3],
    parts: Parts,
    store: &Store,
) {
    let status = status_for(parts, store).name();
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
    let views: Vec<SlotView> = (0..container.size(host))
        .filter_map(|slot| {
            let resource = container.resource(host, slot)?;
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
    let mut done: BTreeSet<[i32; 3]> = BTreeSet::new();
    for start in queued {
        if done.contains(&start) || !is_cable(host, cable, level, start) {
            continue;
        }
        let network = collect_network(start, |pos| is_cable(host, cable, level, pos), MAX_NETWORK);
        done.extend(network.iter().copied());
        let Some(store_pos) = root(&network) else {
            continue;
        };
        // One transaction per network per tick: either every move of this tick
        // happens and the store records it, or none does. A refused store write
        // (an unloaded chunk, a replaced block entity) therefore moves nothing.
        let pass = host.transaction(|host, _tx| {
            let mut store = store_at(host, level, store_pos);
            for &pos in &network {
                let Some(parts) = parts_at(host, cable, level, pos) else {
                    continue;
                };
                for face in parts.faces(Part::ImporterItem) {
                    import_once(host, level, pos, face, &mut store);
                }
            }
            for &pos in &network {
                let Some(parts) = parts_at(host, cable, level, pos) else {
                    continue;
                };
                for face in parts.faces(Part::ExporterItem) {
                    export_once(host, level, pos, face, &mut store);
                }
            }
            match set_store(host, level, store_pos, &store) {
                true => Ok(store),
                false => Err(()),
            }
        });
        // An aborted pass moved nothing, so the store it left on disk is the one the
        // status has to describe.
        let store = match pass {
            Ok(store) => store,
            Err(()) => store_at(host, level, store_pos),
        };
        for &pos in &network {
            let Some(parts) = parts_at(host, cable, level, pos) else {
                continue;
            };
            show_status(host, level, pos, parts, &store);
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
fn parts_line(parts: Parts) -> String {
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
    let status = status_for(parts, &store);
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
            "Usage: /integratedtunnels part <x> <y> <z> <face> <none|importer_item|exporter_item>",
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
                "Integrated Tunnels: unknown part '{wanted}' (none|importer_item|exporter_item)"
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
