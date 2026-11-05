# Roadmap: Xaero's Minimap features, one commit each

Items 1 to 9 are done, apart from what is listed under Not done.

From the user's Xaero reference screenshots (2026-11-05). Each item is its own commit.

1. **Settings screen**: Atlas's own screen in Xaero's layout (title with page n/m, search,
   Minecraft-style buttons in two columns, sliders, Prev/Next/Back), opened by a key and from
   Options > Mods. Categories: Minimap (on/off, position), View, Block Map, Entity Radar, Overlay,
   Information, Waypoint settings, Miscellaneous.
2. **View settings**: size (Auto or exact), shape, lock north, lighting, zoom levels and further
   zoom-out, cave-mode zoom-in, opacity, undiscovered-area opacity, frame style and colour,
   enlarged minimap (toggle, centre, keep rotation, zoom), hide under screens and F3.
3. **In-world waypoints as Xaero draws them**: a small square with the letter and the name
   under it; the distance only while looking at it (Distance to WP: Looking At); icon, name and
   distance scales; opacity in world and on map; min/max render distance; km from 10000 m.
4. **Waypoint screen**: World/Server and Sub-World/Dimension selectors, waypoint sets, filter
   field, Add/Edit, Teleport (T), Share (to chat), Enable/Disable, Delete, Done. Deathpoints
   (keep old, delete when reached), temporary waypoints, teleport command settings,
   cross-dimensional teleport.
5. **Entity radar**: categories, dot size and style, icons (faces, item icons) and names,
   height limit (20), depth fade (min Y distance), entity height display, arrow scale, colour
   and opacity, hide invisible entities, tracked players.
6. **Block map**: cave mode settings (auto cave mode 3x3 ceiling, start Y, depth, toggle timer,
   per-dimension), terrain slopes and depth, block transparency, flowers, stained glass,
   short-block Y adjust, anti-aliasing.
7. **Overlays**: chunk grid (line width), light overlay, slime chunks.
8. **Information display manager**: which lines, background opacity, alignment.
9. **Compass**: on map / over everything, scale, shadow and north colours.

## Not done

- Settings with nothing to act on here: profiles, Minimap Item, update notifications, safe mode,
  chunk claims, Waystones, boss health and potion push boxes (the corner layout handles effects).
- Sub-worlds (only the world and dimension are selectable), custom teleport commands, tracked
  players, up-close waypoint scale.
- Faces for mobs not in `FACES` (`bridge.rs`): they stay dots.
