# Atlas for Horizon

**A sharp minimap, a world map of everywhere you've been, and waypoints on both.**
A native mod for [Horizon](https://github.com/horizon-rust/horizon) (Minecraft 26.3), in the style of Xaero's Minimap and
World Map.

![Atlas in the game: the minimap in the corner and waypoints floating over the land](docs/images/hero.jpg)

## Minimap

Drawn at your screen's own resolution, so every block is crisp. It scrolls smoothly, turns with you (or locks north),
and scales with your window. Height steps are drawn as fine contour lines, so cliffs and hills read at a glance.

Entities show as diamonds: yellow mobs, white players, red dropped items, purple everything else, fading the further
they are above or below you. **Hold Tab** for mob faces, player faces and item icons.

<p align="center"><img src="docs/images/minimap-tab.png" width="640" alt="The minimap with dots, and with mob faces while Tab is held"></p>

## World map

Press **M**. It shows everything you have explored, saved per world, server and dimension. Drag to move, scroll to
zoom, and press **B** over any spot to make a waypoint there. The dimension button shows another dimension's map
from wherever you are (the Nether's centred where you would be there), and the pointer's line gives the same spot's
Nether or Overworld coordinates.

![The world map](docs/images/world-map.jpg)

### Map modes

Both maps share three modes, and each keeps its own saved map:

- **Auto**: the surface in the open, the cave around you underground, under a roof and in the Nether
- **Cave**: always a cross-section around you
- **Layer**: a cut at any height you choose

![Cave mode on the world map](docs/images/cave-mode.jpg)

## Waypoints

**B** makes one where you stand, **H** a temporary one. They show on both maps and in the world, as a small square
with their letter and name, and their distance while you look at them.

**U** opens the waypoint screen: sets, a filter, and Add/Edit, Teleport (T, for operators), Share, Enable/Disable and
Delete, and Disable All / Enable All for the whole set. Dying leaves a deathpoint, which goes away when you reach it.
With **Overworld Waypoints in Nether** on, Overworld waypoints show in the Nether at an eighth of their coordinates,
and ones made in the Nether are saved as the Overworld's.

![The waypoint screen](docs/images/waypoints.jpg)

## Settings

**Y** opens the settings: over 90 options on pages, with a search across them all.

| | |
|---|---|
| ![The settings menu](docs/images/settings.jpg) | ![A settings page](docs/images/settings-page.jpg) |

- **View**: size, shape, zoom, opacity, lighting, frame style and colour, enlarged minimap (hold **Z**)
- **Block map**: colour mode, cave mode, slopes, transparency, grass, flowers, leaves
- **Entity radar**: categories, dot size, icons, names, height limit, depth fade, player cross or arrow
- **Overlays**: chunk grid, light overlay, slime chunks, portal highlights (chunks with Nether portals, End portals
  or gateways), the server's view distance, new chunks (red: generated as they came) and old chunks (green: loaded
  from the server's saves), from each chunk's block palettes as the server sent them; on servers only
- **Information**: coordinates, biome, time, facing, dimension, chunk, light and weather lines
- **Waypoints**: scales, opacity, render distance, deathpoints, compass, beacons (a beam over each waypoint)
- **Miscellaneous**: minimap FPS limit, map sharing

`/atlas <setting> <value>` in chat sets any of them exactly, and `/atlas help` lists them:

```
/atlas size 30           minimap side, percent of the window's height
/atlas zoom 2            blocks per minimap pixel
/atlas mode layer 40     auto | cave | layer [height]
/atlas seed <seed>       this world's seed, for slime chunks (a server never sends it)
```

## Map sharing

A Horizon server running Atlas can share its map: every chunk any player has loaded shows on the world map of every
player who runs Atlas, nearest first, and changes follow as the server sees them.

- **On the server**: Share Map with Players (Miscellaneous) is Auto by default: everyone on a dedicated server, nobody
  from a world of your own opened to LAN. Set it to Operators, Everyone or OFF (`share_map=ops` in the server's
  `settings.txt`, or `/atlas share_map all` in your own world). Map Sharing Rate caps what each player is sent, 256 KB/s
  by default. The shared map is kept in the world's folder, `atlas-share/`.
- **On the client**: nothing to do; turn off Use the Server's Map to keep only what you see yourself.
- **How**: the server offers it when you join (players without Atlas never hear of it), your Atlas says what it already
  has, and the rest comes in the background. It sends each column's blocks rather than colours, so your own settings and
  resource pack colour them. Your own view of a chunk wins over the server's.
- Only the surface is shared, not cave layers. Chunks from before Atlas was on the server are added when they next load.

## Keys

| Key | Does |
|---|---|
| **M** | World map |
| **B** | New waypoint (under the pointer on the world map) |
| **U** | Waypoint screen |
| **H** | Temporary waypoint |
| **Y** | Settings |
| **Z** | Enlarged minimap |
| **[** and **]** | Zoom the minimap |
| **Tab** | Faces and icons on the minimap |
| (unbound) | Both maps show the next dimension's map (also `/atlas dimension`) |

All but Tab can be changed in Options > Mods > Atlas.

## Install

Download `atlas-0.2.0.hnmod` from [Releases](https://github.com/MrCloudy2/horizon-atlas/releases), add it to your native
mods in the Horizon launcher, and build. Atlas is pinned to Horizon Nightly 2026-11-05 (`f319701`), because it reads the
game's world directly, so a newer Nightly may need a new release. Turn off Horizon's own Minimap so you don't get two.

Everything Atlas keeps is in `horizon-mods/.horizon-mod-data/atlas/` in the game folder: `settings.txt`, waypoints and
seeds per world and server, and the maps in `maps/<world>/<dimension>/<layer>/`.

## Bugs and ideas

[Open an issue](https://github.com/MrCloudy2/horizon-atlas/issues), or use Report an Issue in Options > Mods > Atlas.

## Credits

- Based on [Waypoints](https://github.com/Nirokx/horizon-waypoints) by Nirokx (MIT)
- The map columns follow Horizon's minimap (MIT)
- Portal highlights, new and old chunks (the palette check, first published by etianl's Trouser-Streak), dimension
  switching, the view distance square, Overworld waypoints in the Nether, Disable All, waypoint beacons, the minimap
  FPS limiter and the minimap's dimension switching come from [XaeroPlus](https://github.com/rfresh2/XaeroPlus) by rfresh2 (MIT), written anew for Atlas
- The look follows Xaero's Minimap and World Map. Atlas is its own code, with no code or art from them, and isn't
  affiliated with them.

Licence: MIT.

## Build from source

Uses the Horizon launcher's own Rust and Horizon source:

```sh
L=~/.local/share/horizon-launcher
export PATH=$L/rust/1.99.1-x86_64-unknown-linux-gnu/bin:$PATH CARGO_HOME=$L/cargo
cargo test --offline                      # the mod's logic
tools/check.sh                            # applies the bundle to Horizon and type-checks the game
python3 tools/package.py --install        # makes target/atlas-0.2.0.hnmod and installs it in the launcher
tools/build-game.sh && tools/play.sh      # an Atlas-only game, offscreen, for testing; drive it with tools/ctl.sh
```

- `mixins/atlas/core.rs`: all the logic, std only, with its tests
- `mixins/atlas/bridge.rs`: the game side: chunk scans, map images, entities, drawing
- `mixins/atlas/*.rs`: one small adapter per hook point
- `mixins/atlas/horizon-mixin.toml`: where each hook goes
