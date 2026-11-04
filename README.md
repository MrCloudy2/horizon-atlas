# Waypoints for Horizon

A native Horizon mod (Minecraft 26.3, Horizon Nightly 2026-11-01, `c9d474d`).

- **N**: new waypoint where you stand: name, X/Y/Z, dimension, colour, visible or hidden. Enter saves, Tab moves between fields, Esc cancels.
- **M**: the waypoint list, with filters for All / Overworld / Nether / End. Each row shows its colour, name, coordinates, dimension and distance, with **Hide/Show**, **Edit** and **Delete** (click twice) buttons.
- **In the world**: each visible waypoint of your dimension shows its name, its distance and a marker in its colour.
- **Options > Mods > Waypoints**: change both keys, and turn the in-world labels on or off.

Waypoints are kept per single player world folder and per server address, in
`horizon-mods/.horizon-mod-data/waypoints/` inside the game folder.

## Build and install

Uses the Horizon launcher's own Rust and Horizon source (nothing else to install):

```sh
L=~/.local/share/horizon-launcher
export PATH=$L/rust/1.99.1-x86_64-unknown-linux-gnu/bin:$PATH CARGO_HOME=$L/cargo
cargo test --offline                      # the mod's logic
python3 tools/package.py --install        # target/waypoints-1.0.0.hnmod, copied into the launcher's native mods
```

Then build the game with its mods in the launcher. The mod is pinned to Horizon
`c9d474d` because it reads the world folder, the server address and the last
frame's camera from the game itself, which the stable hook points don't offer. A newer Nightly may need the
pin in `compatibility.json` and `mixins/waypoints/horizon-mixin.toml` moved.

## Layout

- `mixins/waypoints/runtime.rs`: all the logic, with its unit tests (also the module the game gets)
- `mixins/waypoints/*.rs`: one small hook adapter per hook point
- `mixins/waypoints/horizon-mixin.toml`: where each hook goes
- `tools/package.py`: makes the `.hnmod`
