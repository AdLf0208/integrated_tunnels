//! Integrated Tunnels for Horizon: an Item Importer and an Item Exporter part on
//! a tunnel cable, and the network that moves items between them.
//!
//! Integrated Tunnels (see `FINDINGS.md`) puts parts on an Integrated Dynamics
//! cable's block entity: an Item Importer moves items from the inventory on its
//! face into the network's slotless item channel, and an Item Exporter moves them
//! from that channel into the inventory on its face. There is no cross-mod API at
//! this pin, so this mod ships its own carrier cable with the same six-face part
//! protocol, and the parts are the two items it declares.
//!
//! The rules are what [`runtime`] decides (parts, network, store, the per-tick
//! move); this crate's glue reads the cable's block state and block entity, calls
//! those rules and writes the result back:
//!
//! - [`cable`] is the game side: the block entity type, the queue a cable's tick
//!   fills, the once-per-network pass at the end of the level's tick, and the
//!   `/integratedtunnels` command handlers;
//! - this file is the one `impl Hooks`: the part registration, the right-click
//!   placement, the tick pass and the command surface.
pub mod runtime;

mod cable;

use std::sync::{Arc, Mutex};

use mod_api::{
    self, Command, Flow, Hand, Hooks, HostSide, LevelTickPostEvent, RegisterCommandsEvent,
    Registrar, RegistrationContext, RightClickBlockEvent, RunModCommandEvent, ServerHost,
};

use crate::cable::{Cable, Placement, Queue};

/// The mod's session state: the cables its block entities ticked this tick.
#[derive(Default)]
struct Mod {
    queue: Arc<Mutex<Queue>>,
}

#[mod_api::horizon_mod]
impl Hooks for Mod {
    /// Declare the cable's block entity type and attach the one part that makes it
    /// tick. A refusal here can only mean the same part attached twice, which is a
    /// broken build rather than a player's world.
    fn register(&self, registrar: &mut dyn Registrar, _context: &RegistrationContext) {
        let kind = registrar
            .block_entity_type("tunnel_cable")
            .blocks(&["tunnel_cable"])
            .build();
        let attached = registrar.block_entity_part(&kind, Cable::new(Arc::clone(&self.queue)));
        debug_assert!(
            attached.is_ok(),
            "the cable part attaches once: {attached:?}"
        );
    }

    /// Right-clicking a cable face with a part item places that part on the face
    /// (upstream's `ItemPart.useOn`). The main hand only: the game fires this hook
    /// once per hand.
    fn right_click_block(&self, host: HostSide, event: &mut RightClickBlockEvent<'_>) -> Flow {
        let HostSide::Server(host) = host else {
            return Flow::Continue;
        };
        let click = event.click;
        if click.hand != Hand::Main {
            return Flow::Continue;
        }
        let Some(held) = click.stack else {
            return Flow::Continue;
        };
        match cable::place_part_from_item(host, click.pos, click.hit.face, held, click.player) {
            Placement::NotMine => Flow::Continue,
            Placement::Placed { face, part } => {
                cable::tell(host, click.player, &cable::placement_line(face, part));
                Flow::Cancel
            }
            Placement::Taken { face } => {
                cable::tell(host, click.player, &cable::taken_face_line(face));
                Flow::Cancel
            }
        }
    }

    /// After the level's tick: run every queued cable network's transfers, once per
    /// network, in one transaction. A client's tick runs nothing.
    fn level_tick_post(&self, host: HostSide, event: &mut LevelTickPostEvent) {
        let HostSide::Server(host) = host else {
            return;
        };
        // The pass reaches containers through this level's `ITEMS` capability, and
        // `LevelRef::capability` answers only for the level the server ticks now.
        // Outside that window the queue is left alone and the next tick retries,
        // rather than dropping a network's work: the queue is bounded by the number
        // of cables, so waiting costs nothing.
        if !event.level.in_place(host) {
            return;
        }
        let queued = {
            // Only this expression runs under the lock: one `remove` on the port's
            // own state, which cannot panic and so cannot poison it.
            self.queue
                .lock()
                .expect("the tunnel queue lock is held for one drain")
                .take(event.level)
        };
        cable::run_networks(host, event.level, queued);
    }

    /// `/integratedtunnels info <pos>` and `/integratedtunnels part <pos> <face>
    /// <part>`: the harness readout and the path a face is plumbed without a click
    /// (`ctl interact` only reaches a visible face).
    fn register_commands(&self, _host: &mut dyn ServerHost, event: &mut RegisterCommandsEvent<'_>) {
        event
            .commands
            .push(Command::new("integratedtunnels info <pos:block_pos>"));
        event.commands.push(Command::new(
            "integratedtunnels part <pos:block_pos> <face:word> <part:word>",
        ));
    }

    /// Run the two usages [`Hooks::register_commands`] declares, after the game has
    /// parsed and permission-checked them.
    fn run_mod_command(
        &self,
        host: &mut dyn ServerHost,
        event: &mut RunModCommandEvent<'_>,
    ) -> Flow {
        let literals: Vec<&str> = event.command.literals.iter().map(String::as_str).collect();
        match literals.as_slice() {
            ["integratedtunnels", "info"] => {
                cable::info(host, event.command);
                Flow::Cancel
            }
            ["integratedtunnels", "part"] => {
                cable::set_part(host, event.command);
                Flow::Cancel
            }
            _ => Flow::Continue,
        }
    }
}
