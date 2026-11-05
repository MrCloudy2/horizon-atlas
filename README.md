# Atlas for Horizon

A minimap, a world map of everywhere you have been, and waypoints on both, as a native Horizon mod
(Minecraft 26.3, Horizon Nightly 2026-11-05, `f319701`).

Based on [Waypoints](https://github.com/Nirokx/horizon-waypoints) by Nirokx (MIT). The map columns and
their shading follow Horizon's own minimap (MIT).

## In the game

- **Minimap** (top left by default): scrolls smoothly, turns with you if you like, and shrinks with the
  window. It shows:
  - dots: white for players, yellow for mobs, red for dropped items, purple for other entities
  - waypoints as their first letter on their colour (on the edge when far)
  - N, E, S and W arrows
  - your coordinates, biome and the time of day underneath
  - slime chunks, once you give the world's seed
- **M**: the world map. Drag to move, wheel to zoom. **B** (or a right click) makes a waypoint at the spot
  under the pointer, and a click on a waypoint edits it.
- **B** in the game: a new waypoint where you stand.
- **U**: the waypoint list. Click a waypoint, then use the buttons below the list: Teleport (operators
  only), Edit, Hide/Show and Delete.
- **Map modes**, shared by both maps:
  - **Auto**: the surface in the open, the cave around you underground, under a roof and in the Nether
  - **Cave**: always the cave around you
  - **Layer**: a cut at a height you choose

  Each mode is kept on disk, so the world map remembers caves too.
- **Waypoint labels in the world**: name, distance and letter.

## Settings

**Y** opens the settings screen, laid out as Xaero's: pages of buttons and sliders, and a search
across them all. Options > Mods > Atlas has the main switches and the keys.

`/atlas <setting> <value>` in chat sets any setting exactly (`/atlas help` lists them):

```
/atlas size 30           minimap side, percent of the window's height
/atlas zoom 2            blocks per minimap pixel
/atlas mode layer 40     auto | cave | layer [height]
/atlas seed <seed>       this world's seed, for slime chunks (a server never sends it)
```

## Where it keeps things

Everything goes in `horizon-mods/.horizon-mod-data/atlas/` inside the game folder:

- the settings, in `settings.txt`
- waypoints and seeds, one file per single player world folder and per server address
- the maps, in `maps/<world>/<dimension>/<mode layer>/`

## Build and install

Uses the Horizon launcher's own Rust and Horizon source (nothing else to install):

```sh
L=~/.local/share/horizon-launcher
export PATH=$L/rust/1.99.1-x86_64-unknown-linux-gnu/bin:$PATH CARGO_HOME=$L/cargo
cargo test --offline                      # the mod's logic
tools/check.sh                            # applies the bundle to Horizon f319701 and type-checks the game
python3 tools/package.py --install        # target/atlas-0.1.0.hnmod, copied into the launcher's native mods
```

Then build the game with its mods in the launcher. The mod is pinned to Horizon `f319701`. Besides the
stable hook points, it reads the window's world, its map texture slots and its entities, so a newer
Nightly may need the pin in `compatibility.json` and `mixins/atlas/horizon-mixin.toml` moved.

## Layout

- `mixins/atlas/core.rs`: all the logic, std only, with its unit tests (also the module the game gets)
- `mixins/atlas/bridge.rs`: the game side: chunk scans, map images, entities, drawing
- `mixins/atlas/*.rs`: one small hook adapter per hook point
- `mixins/atlas/horizon-mixin.toml`: where each hook goes
- `tools/package.py`: makes the `.hnmod`
- `tools/check.sh`: builds the bundle into Horizon's source to check it compiles
