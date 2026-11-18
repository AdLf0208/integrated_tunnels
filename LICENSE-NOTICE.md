# Licence notice

This port is a clean-room Rust rewrite of **Integrated Tunnels 1.13.0** by
rubensworks (CyclopsMC), distributed under the **MIT License**.

- Upstream: <https://github.com/CyclopsMC/IntegratedTunnels>, tag
  `1.21.1-1.13.0` (commit `dd9ade64e31b0f56c2dc672541d0e9583119670f`).
- Licence read from the pack jar's `META-INF/neoforge.mods.toml`
  (`license="MIT"`, `modId="integratedtunnels"`, `version="1.13.0"`) and from the
  upstream `LICENSE.txt` (Copyright (c) 2015 Cyclops), copied verbatim into
  `LICENSE`.
- Pack jar: `integratedtunnels-1.21.1-neoforge-1.13.0.jar`, SHA-256
  `9b5bed5e69d071bf9266c0d9186bfaa952970493e93defda0cce35edb978e19e`,
  kept at `ports/_shared/licence-cache/` as the licence and inventory evidence.
- Carried assets: the two part item models and their four part textures, the
  part geometry and per-face rotations, and the lang lines for the parts, all
  taken from that jar (MIT). The carrier block's core model is authored here; its
  texture is Integrated Dynamics' `block/cable.png` (MIT, CyclopsMC), carried
  because this port's carrier stands in for that cable.
- The block declaration, blockstate, block model, loot table and creative tab
  are written here: upstream draws a cable with a block-entity renderer, and the
  parent port's cable block cannot carry another mod's part values.

The port is **local-only** until the maintainer explicitly approves publishing
(campaign ARCHITECTURE.md §8). A published copy must keep the MIT notice above.

**Relationship to the sibling port.** `ports/integrateddynamics` is this mod's
parent and is also ours. There is no cross-mod API at this pin, so the cable
block entity, the per-face part protocol and the network tick are re-implemented
in this crate (same approach, its own code), and the port ships its own carrier
block. The two ports share no crate, no symbol and no namespace.
