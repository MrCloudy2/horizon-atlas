// Atlas's reads of the game itself (Horizon f319701): the chunks it scans
// into map columns, the map texture slots its images go in, the entities its
// dots stand for, and drawing what `atlas_mod` laid out. Inserted into
// `crate::game`, so it reaches the window's world as the game's own code does.
//
// The column scan follows Horizon's minimap (`horizon-client-render`'s
// `minimap.rs`, MIT, Ludovic Marie and the Horizon contributors), with cuts
// that look for the first air under a height.
use crate::game::Game;
use crate::game::atlas_mod as m;
use horizon_client::blocks::{BlockFacts, Fluid, MAP_COLORS};
use std::cell::RefCell;
use std::time::{Duration, Instant};

/// How a block's colour is tinted, as the game tints its faces (the far
/// terrain's rules).
#[derive(Clone, Copy, PartialEq)]
enum Tint {
    None,
    Grass,
    Foliage,
    Dry,
    Water,
    Constant(u32),
}

/// What the scan knows of a block state: its colour seen from above (its top
/// face's texture averaged, untinted), its tint, and flags.
#[derive(Clone, Copy)]
struct Look {
    rgb: u32,
    /// The map item's colour.
    map_rgb: u32,
    tint: Tint,
    flags: u16,
}
const FLUID: u16 = 3;
const AIR: u16 = 4;
const STURDY_TOP: u16 = 8;
/// It has a map colour: the map item would stop at it.
const MAPPED: u16 = 16;
/// Blocks the settings can see through.
const FLOWER: u16 = 32;
const GLASS: u16 = 64;
const PLANT: u16 = 128;
const LEAVES: u16 = 256;
/// Obsidian: XaeroPlus's see-through roof.
const OBSIDIAN: u16 = 512;

/// The block states' looks, worked out the first time each is met, and
/// every texture's average colour.
struct Palette {
    states: usize,
    texture_layers: u32,
    looks: Vec<Option<Look>>,
    /// Average RGB of each sprite layer, 0 where it is see-through.
    layers: Vec<u32>,
    water: u32,
    lava: u32,
    /// Each state's portal kind (`m::NETHER_PORTAL`...), 0 for the rest.
    portals: Vec<u8>,
}

/// The average colour of each texture layer: alpha-weighted, ignoring the
/// see-through pixels.
fn layer_averages(render: &horizon_client::assets::RenderAssets) -> Vec<u32> {
    let size = render.sprite_size as usize;
    let count = render.texture_layers as usize;
    let pixels = render.pixels.get();
    let per = size * size * 4;
    if per == 0 || pixels.len() < per * count {
        return Vec::new();
    }
    (0..count)
        .map(|layer| {
            let mut sum = [0u64; 3];
            let mut weight = 0u64;
            for px in pixels[layer * per..(layer + 1) * per].chunks_exact(4) {
                let a = u64::from(px[3]);
                if a < 16 {
                    continue;
                }
                for k in 0..3 {
                    sum[k] += u64::from(px[k]) * a;
                }
                weight += a;
            }
            if weight == 0 {
                return 0;
            }
            let c = sum.map(|v| (v / weight) as u32);
            (c[0] << 16 | c[1] << 8 | c[2]).max(1)
        })
        .collect()
}

impl Palette {
    fn new(facts: &BlockFacts, render: &horizon_client::assets::RenderAssets) -> Self {
        let layers = layer_averages(render);
        let sprite = |name: &str| {
            let layer = render
                .sprite_layers
                .get(name)
                .or_else(|| render.sprite_layers.get(&format!("minecraft:{name}")))?;
            layers.get(*layer as usize).copied().filter(|c| *c != 0)
        };
        let water = sprite("block/water_still").unwrap_or(0xB0_B0B0);
        let lava = sprite("block/lava_still").unwrap_or(0xCF_5B13);
        let portals = facts
            .states
            .iter()
            .map(|f| match f.name.strip_prefix("minecraft:").unwrap_or(&f.name) {
                "nether_portal" => m::NETHER_PORTAL,
                "end_portal" => m::END_PORTAL,
                "end_gateway" => m::END_GATEWAY,
                _ => 0,
            })
            .collect();
        Self {
            states: facts.states.len(),
            texture_layers: render.texture_layers,
            looks: vec![None; facts.states.len()],
            layers,
            water,
            lava,
            portals,
        }
    }

    /// State `id`'s look, worked out once.
    fn look(&mut self, id: u32, facts: &BlockFacts, render: &horizon_client::assets::RenderAssets) -> Look {
        let i = id as usize;
        if let Some(Some(look)) = self.looks.get(i) {
            return *look;
        }
        let Some(f) = facts.states.get(i) else {
            return Look { rgb: 0, map_rgb: 0, tint: Tint::None, flags: AIR };
        };
        let flags = state_flags(id, facts);
        let state = horizon_world::registry::BlockStateId(id);
        let name = f.name.strip_prefix("minecraft:").unwrap_or(&f.name);
        let leaves = name.ends_with("_leaves");
        let tinted = match name {
            // The game's own: these don't follow the biome.
            "spruce_leaves" => Tint::Constant(0x61_9961),
            "birch_leaves" => Tint::Constant(0x80_A755),
            "cherry_leaves" | "azalea_leaves" | "flowering_azalea_leaves" | "pale_oak_leaves" => Tint::None,
            "leaf_litter" => Tint::Dry,
            "water" | "bubble_column" => Tint::Water,
            _ if leaves => Tint::Foliage,
            _ => Tint::Grass,
        };
        // Its top face's texture, else the particle texture.
        let models = render
            .model_catalog
            .collect(state, horizon_core::position::BlockPos::new(0, 0, 0))
            .ok();
        let faces = || models.iter().flatten().flat_map(|(_, model)| &model.faces);
        let top = faces()
            .find(|face| face.quad.facing == horizon_assets::model::Direction::Up)
            .or_else(|| faces().next());
        let mut rgb = top
            .and_then(|face| self.layers.get(face.layer as usize).copied())
            .filter(|c| *c != 0)
            .or_else(|| {
                let layer = render.model_catalog.particle(state)?;
                self.layers.get(layer as usize).copied().filter(|c| *c != 0)
            })
            .unwrap_or(0);
        let mut tint = match top {
            Some(face) if face.quad.tint_index >= 0 => tinted,
            _ => Tint::None,
        };
        if name == "water" || name == "bubble_column" {
            rgb = self.water;
            tint = Tint::Water;
        } else if name == "lava" {
            rgb = self.lava;
            tint = Tint::None;
        }
        // Measured from Xaero's map: ice is darker than its texture, and
        // bluer in the dark.
        if matches!(name, "ice" | "frosted_ice") {
            let ch = |s: u32, k: f32| (((rgb >> s & 255) as f32 * k) as u32) << s;
            rgb = ch(16, 0.898) | ch(8, 0.838) | ch(0, 0.761);
        }
        if rgb == 0 {
            // No texture: the map item's colour.
            rgb = MAP_COLORS.get(usize::from(f.map_color)).copied().unwrap_or(0) & 0xFF_FFFF;
        }
        let map_rgb = MAP_COLORS.get(usize::from(f.map_color)).copied().unwrap_or(0) & 0xFF_FFFF;
        let look = Look { rgb, map_rgb, tint, flags };
        if let Some(slot) = self.looks.get_mut(i) {
            *slot = Some(look);
        }
        look
    }
}

/// What the scan and the share need to know of block state `id`: whether
/// it is a fluid, air, sturdy on top, on maps, or something a setting can
/// see through. Facts only (no textures), so a server works it out too.
fn state_flags(id: u32, facts: &BlockFacts) -> u16 {
    let Some(f) = facts.states.get(id as usize) else {
        return AIR;
    };
    let mut flags = match f.fluid {
        Fluid::None => 0,
        Fluid::Water => 1,
        Fluid::Lava => 2,
    };
    let state = horizon_world::registry::BlockStateId(id);
    if facts.is_air(state) {
        flags |= AIR;
    }
    // isFaceSturdy(UP, FULL): Direction.UP's ordinal is 1.
    if f.sturdy_full >> 1 & 1 != 0 {
        flags |= STURDY_TOP;
    }
    if f.map_color != 0 {
        flags |= MAPPED;
    }
    let name = f.name.strip_prefix("minecraft:").unwrap_or(&f.name);
    const FLOWERS: [&str; 15] = [
        "dandelion", "poppy", "blue_orchid", "allium", "azure_bluet", "tulip", "oxeye_daisy",
        "cornflower", "lily_of_the_valley", "wither_rose", "sunflower", "lilac", "rose_bush",
        "peony", "torchflower",
    ];
    if FLOWERS.iter().any(|n| name.ends_with(n)) {
        flags |= FLOWER;
    }
    if name.contains("stained_glass") {
        flags |= GLASS;
    }
    const PLANTS: [&str; 16] = [
        "short_grass", "tall_grass", "fern", "large_fern", "short_dry_grass", "tall_dry_grass",
        "bush", "firefly_bush", "dead_bush", "leaf_litter", "wildflowers", "pink_petals",
        "vine", "glow_lichen", "moss_carpet", "pale_moss_carpet",
    ];
    if PLANTS.contains(&name) {
        flags |= PLANT;
    }
    if name.ends_with("_leaves") {
        flags |= LEAVES;
    }
    if name == "obsidian" || name == "crying_obsidian" {
        flags |= OBSIDIAN;
    }
    flags
}

thread_local! {
    static PALETTE: RefCell<Option<Palette>> = const { RefCell::new(None) };
}

/// The blocks a scan reads: a chunk of the world, or one a server shared.
#[derive(Clone, Copy)]
enum Source<'a> {
    Live(&'a horizon_client::chunk::Chunk),
    /// With the world's air.
    Shared(&'a m::SharedChunk, u32),
}

impl Source<'_> {
    fn get(&self, x: i32, y: i32, z: i32) -> u32 {
        match self {
            Source::Live(c) => c.get(x, y, z),
            Source::Shared(c, air) => c.state(x, y, z, *air),
        }
    }
    fn biome(&self, x: i32, y: i32, z: i32) -> u16 {
        match self {
            Source::Live(c) => c.biome(x, y, z),
            Source::Shared(c, _) => c.biome(x, z),
        }
    }
    fn min_y(&self) -> i32 {
        match self {
            Source::Live(c) => c.min_y(),
            Source::Shared(c, _) => c.min_y,
        }
    }
    fn x(&self) -> i32 {
        match self {
            Source::Live(c) => c.x,
            Source::Shared(c, _) => c.x,
        }
    }
    fn z(&self) -> i32 {
        match self {
            Source::Live(c) => c.z,
            Source::Shared(c, _) => c.z,
        }
    }
    /// The highest block worth scanning from.
    fn top(&self, air: u32) -> i32 {
        match self {
            Source::Live(c) => top_of(c, air),
            Source::Shared(c, _) => c.top(),
        }
    }
}

/// What a scan reads: the chunk, the block looks, the biome colours.
struct Scan<'a> {
    chunk: Source<'a>,
    palette: &'a mut Palette,
    facts: &'a BlockFacts,
    render: &'a horizon_client::assets::RenderAssets,
    biomes: &'a horizon_client::biome::Biomes,
    /// The map item's colours, not the textures'.
    vanilla: bool,
    /// Blocks seen through (`FLOWER`, `GLASS`).
    hide: u16,
    cave_depth: i32,
    /// XaeroPlus's roof: obsidian from this height up is seen through.
    roof_y: Option<i32>,
    /// The bottom shows through water.
    transparency: bool,
    /// The block light at a world position, 0 to 15.
    light: &'a dyn Fn(i32, i32, i32) -> u8,
}

impl Scan<'_> {
    fn look(&mut self, x: i32, y: i32, z: i32) -> Look {
        let id = self.chunk.get(x, y, z);
        self.palette.look(id, self.facts, self.render)
    }

    /// `look`'s colour at block `(x, y, z)` of the chunk, tinted by its biome.
    fn color(&self, look: Look, x: i32, y: i32, z: i32) -> u32 {
        if self.vanilla {
            return look.map_rgb;
        }
        let tint = match look.tint {
            Tint::None => return look.rgb,
            Tint::Constant(c) => c,
            other => {
                let id = self.chunk.biome(x, y, z);
                let b = self.biomes.get(id);
                match other {
                    Tint::Grass => {
                        let (wx, wz) = (self.chunk.x() * 16 + x, self.chunk.z() * 16 + z);
                        self.biomes.grass(id, f64::from(wx), f64::from(wz))
                    }
                    Tint::Foliage => b.foliage,
                    Tint::Dry => b.dry_foliage,
                    _ => b.water,
                }
            }
        };
        let ch = |s: u32| ((look.rgb >> s & 255) * (tint >> s & 255) / 255) << s;
        ch(16) | ch(8) | ch(0)
    }

    /// `MapItem.update` for one column from `y` down: the first block with a
    /// map colour, the water over it unless the block is sturdy on top, and
    /// the water's depth.
    fn column_from(&mut self, x: i32, z: i32, mut y: i32) -> (u32, i16, u8) {
        let bottom = self.chunk.min_y();
        while y > bottom {
            let look = self.look(x, y, z);
            let roof = look.flags & OBSIDIAN != 0 && self.roof_y.is_some_and(|r| y >= r);
            if look.flags & MAPPED == 0 || look.flags & self.hide != 0 || roof {
                y -= 1;
                continue;
            }
            let fluid = look.flags & FLUID;
            if fluid == 0 || look.flags & STURDY_TOP != 0 {
                let kind = if look.flags & LEAVES != 0 { m::LEAF } else { m::LAND };
                return (kind | self.color(look, x, y, z), y as i16, 0);
            }
            let mut depth = 1u8;
            let mut below = y - 1;
            while below > bottom && self.look(x, below, z).flags & FLUID != 0 {
                depth = depth.saturating_add(1);
                below -= 1;
            }
            if fluid == 2 {
                return (m::LAND | self.palette.lava, y as i16, 0);
            }
            let water = Look { rgb: self.palette.water, map_rgb: look.map_rgb, tint: Tint::Water, flags: look.flags };
            let mut rgb = self.color(water, x, y, z);
            if self.transparency {
                // See-through at every depth: the bottom keeps showing
                // under the water, less of it the deeper it is, and the
                // water itself darkens. Fitted to Xaero's map at noon over
                // sand and over dirt, 1 to 10 deep; past 15 it stays.
                let under = self.look(x, below, z);
                let bottom = self.color(under, x, below, z);
                let d = f32::from(depth.clamp(1, 15) - 1);
                let a = 0.716 + 0.0145 * d;
                let dark = 1.0 - 0.0185 * d;
                let ch = |s: u32, k: f32| {
                    let (w, g) = ((rgb >> s & 255) as f32, (bottom >> s & 255) as f32);
                    ((w * k * dark * a + g * (1.0 - a)).round().min(255.0) as u32) << s
                };
                rgb = ch(16, 0.895) | ch(8, 0.874) | ch(0, 0.849);
            }
            return (m::WATER | rgb, y as i16, depth);
        }
        (0, bottom as i16, 0)
    }

    /// One column for `cut`. `top` is the highest block of the chunk.
    fn column(&mut self, x: i32, z: i32, top: i32, cut: m::Cut) -> (u32, i16, u8) {
        let m::Cut::Below(cut_y) = cut else {
            return self.column_from(x, z, top);
        };
        if cut_y > top {
            // Open sky over the cut: the column as it is.
            return self.column_from(x, z, top);
        }
        // Down through what is solid at the cut; solid all the way is a wall.
        let floor = (cut_y - self.cave_depth).max(self.chunk.min_y());
        let mut y = cut_y;
        let mut wall = None;
        while y > floor {
            let look = self.look(x, y, z);
            if look.flags & (AIR | FLUID) != 0 || look.flags & MAPPED == 0 {
                return self.column_from(x, z, y);
            }
            if wall.is_none() {
                wall = Some(self.color(look, x, y, z));
            }
            y -= 1;
        }
        (m::WALL | m::LAND | wall.unwrap_or(0x30_3030), cut_y as i16, 0)
    }
}

/// The highest block of a chunk worth scanning from.
fn top_of(chunk: &horizon_client::chunk::Chunk, air: u32) -> i32 {
    (0..chunk.sections.len())
        .rev()
        .find(|&s| chunk.sections.get(s).is_some_and(|sec| !sec.is_uniform(air)))
        .map_or(chunk.min_y(), |s| chunk.min_y() + s as i32 * 16 + 15)
}

fn scan_tile(scan: &mut Scan<'_>, air: u32, cut: m::Cut) -> m::Tile {
    let top = scan.chunk.top(air);
    let mut tile = m::Tile::blank(scan.chunk.min_y() as i16);
    for z in 0..16 {
        for x in 0..16 {
            let (c, h, d) = scan.column(x, z, top, cut);
            let i = (z * 16 + x) as usize;
            tile.color[i] = c;
            tile.height[i] = h;
            tile.depth[i] = d;
            // What lights the top of it: the block over it (over the water),
            // or its own where it gives light, as lava does.
            if c & m::WALL == 0 && c & m::KIND != 0 {
                let (wx, wz) = (scan.chunk.x() * 16 + x, scan.chunk.z() * 16 + z);
                let at = |y: i32| (scan.light)(wx, y, wz);
                tile.light[i] = at(i32::from(h) + 1).max(at(i32::from(h)));
            }
        }
    }
    tile
}

/// The kinds of portal in a chunk: each section's palette is asked first,
/// so a section without one costs a look at its few palette entries.
fn portals_in(chunk: &horizon_client::chunk::Chunk, kinds: &[u8]) -> u8 {
    let kind = |id: u32| kinds.get(id as usize).copied().unwrap_or(0);
    let mut found = 0;
    for sec in chunk.sections.iter() {
        if !sec.blocks.maybe_has(|id| kind(id) != 0) {
            continue;
        }
        for bit in [m::NETHER_PORTAL, m::END_PORTAL, m::END_GATEWAY] {
            if found & bit == 0 && sec.blocks.has(|id| kind(id) & bit != 0) {
                found |= bit;
            }
        }
    }
    found
}

/// XaeroPlus's new chunks: each chunk that came is checked once, as soon
/// as the budget allows, before block changes reshape its palettes. Only
/// from a server: a world of one's own hands the client its own sections.
fn check_new_chunks(game: &Game<'_>) {
    if game.integrated.is_some() {
        return;
    }
    let started = Instant::now();
    for (key, chunk) in &game.world.chunks {
        if !m::with(|s| s.wants_new_check(*key)) {
            continue;
        }
        let wire: Vec<_> = chunk.sections.iter().map(|sec| sec.blocks.to_wire()).collect();
        let new = m::is_new_chunk(wire.iter().map(|(bits, palette, words)| (*bits, palette.as_slice(), *words)));
        m::with(|s| s.put_new_chunk(*key, new));
        if started.elapsed() >= Duration::from_micros(1000) {
            return;
        }
    }
}

/// Where the player is, for `atlas_mod`.
pub(super) fn here(game: &Game<'_>) -> Option<m::Here> {
    let p = game.native_player()?;
    Some(m::Here {
        pos: [p.x, p.y, p.z],
        dimension: p.dimension.to_owned(),
        yaw: p.yaw,
    })
}

/// The cut the maps show for the player now.
fn current_cut(game: &Game<'_>, settings: &m::Settings) -> Option<m::Cut> {
    let p = game.player.as_ref()?;
    let dimension = game.native_player()?.dimension;
    let head = (p.position.y + f64::from(p.dimensions.eye_height)).floor() as i32;
    let (bx, bz) = (p.position.x.floor() as i32, p.position.z.floor() as i32);
    let sky = |dx: i32, dz: i32| game.world.light(bx + dx, head, bz + dz) >> 4;
    let covered = match settings.auto_cave {
        // All nine columns around the player: one gap doesn't flip the map.
        0 => (-1..=1).all(|dx| (-1..=1).all(|dz| sky(dx, dz) == 0)),
        1 => sky(0, 0) == 0,
        _ => false,
    };
    let ceiling = settings.cave_nether && dimension == m::NETHER;
    Some(m::cut_for(settings.mode, settings.layer_y, head, covered, ceiling))
}

thread_local! {
    /// The server key worked out for (address, folder mode): the server
    /// list is read once a connection, not each frame.
    static SERVER_KEY: RefCell<Option<(String, usize, String)>> = const { RefCell::new(None) };
}

/// A server's key by the folder mode chosen (`m::server_key`), the server
/// list read only when the address or the mode changes.
fn server_key(address: &str, mode: usize) -> String {
    SERVER_KEY.with(|k| {
        let mut k = k.borrow_mut();
        if let Some((a, was, key)) = k.as_ref()
            && a == address
            && *was == mode
        {
            return key.clone();
        }
        let list: Vec<(String, String)> = if mode == m::FOLDER_NAME {
            std::env::current_dir()
                .map(|dir| horizon_protocol::servers::load(&horizon_client::multiplayer::servers_file(&dir.join("saves"))))
                .unwrap_or_default()
                .into_iter()
                .map(|e| (e.name, e.ip))
                .collect()
        } else {
            Vec::new()
        };
        let key = m::server_key(mode, address, &list);
        *k = Some((address.to_owned(), mode, key.clone()));
        key
    })
}

/// The world or server being played, naming its waypoints and maps.
fn world_key(game: &Game<'_>, folder: usize) -> Option<String> {
    let on_server = matches!((&game.integrated, &game.remote), (None, Some(_)));
    if !on_server {
        // Between connections: the list is read again (a server renamed).
        SERVER_KEY.with(|k| *k.borrow_mut() = None);
    }
    game.native_player()?;
    if let (None, Some(remote)) = (&game.integrated, &game.remote) {
        return Some(server_key(&remote.session.address, folder));
    }
    // The single player world's save folder, which the connection keeps
    // (worlds can share a name, never a folder).
    if let Some(folder) = game.remote.as_ref().and_then(|r| r.save_folder.as_deref()) {
        return Some(format!("world-{folder}"));
    }
    if let Some(folder) = game
        .world_icon
        .as_ref()
        .and_then(|icon| icon.path.parent()?.file_name())
    {
        // Else the icon's: the window keeps it from the moment its server
        // starts till the world has one.
        return Some(format!("world-{}", folder.to_string_lossy()));
    }
    // The icon is taken and forgotten: the world stays the one loaded.
    let current = m::with(|s| s.world.clone()).filter(|w| w.starts_with("world-"));
    let named = game.integrated.as_ref().map(|i| format!("world-{}", i.world_name));
    current.or(named).or_else(|| Some("world".into()))
}

/// Once a frame: settings, the world and dimension, the scan of the chunks
/// around the player, the images, and the mod's screen's state.
pub(super) fn frame(game: &mut Game<'_>) {
    let dir = game.native_data_dir(m::ID);
    let settings = {
        let g = &*game;
        m::with(|s| s.settings_with(|k| g.native_setting(m::ID, k)))
    };
    if let Some(changed) = m::with(|s| s.take_changed_settings()) {
        game.native_set_setting(m::ID, m::SETTINGS_FILE, &changed.to_text());
    }
    if let Some(remote) = game.remote.as_mut() {
        chat_tree(&mut remote.chat_tree);
    }
    let key = world_key(game, settings.server_folder);
    let dimension = game.native_player().map(|p| p.dimension.to_owned());
    m::with(|s| {
        s.load_world(key.clone(), dir.clone());
        let maps = match (&key, &dimension) {
            (Some(k), Some(d)) => Some(m::map_dir(&dir, k, d)),
            _ => None,
        };
        s.load_map(dimension.clone(), maps);
    });

    // Teleport (operators): the server checks the permission again.
    let op = game.ui.registries.permission >= 2;
    m::with(|s| s.can_teleport = op);
    if let Some(command) = m::with(|s| s.take_teleport())
        && let Err(e) = game.native_send_chat(&command)
    {
        game.native_chat(&format!("Atlas: can't teleport ({e})"));
    }

    if let Some(line) = m::with(|s| s.take_say())
        && let Err(e) = game.native_send_chat(&line)
    {
        game.native_chat(&format!("Atlas: can't share ({e})"));
    }
    if let Some(here) = here(game) {
        let dead = game.screen == crate::game::Screen::Dead;
        m::with(|s| s.track_death(dead, &here));
    }

    let close = m::with(|s| s.take_close());
    if close {
        game.native_mod_screen_close(m::ID);
    }
    let ours = game.native_mod_screen() == Some(m::ID);
    m::with(|s| s.sync_open(ours));

    if !game.native_on(m::ID) {
        return;
    }
    let Some(cut) = current_cut(game, &settings) else {
        m::with(|s| s.save_now_and_then(10.0));
        return;
    };
    m::with(|s| s.want_cut(cut));
    let cut = m::with(|s| s.cut);
    // Only the overworld's sky turns; elsewhere the maps are lit as by day.
    let overworld = game.native_player().is_some_and(|p| p.dimension == m::OVERWORLD);
    let ticks = game.native_clock().filter(|_| overworld).map(|c| c.day_time);
    m::with(|s| s.set_day_time(ticks));
    // The server's view distance (Login, SetChunkCacheRadius); in a world
    // of one's own, the render distance it is served at.
    let view = game.remote.as_ref().map_or(game.settings.render_distance, |r| r.view_distance);
    m::with(|s| s.view_distance = Some(view));
    scan(game, cut, &settings);
    share_client(game, &settings);
    if settings.new_chunks || settings.old_chunks {
        check_new_chunks(game);
    }
    m::with(|s| s.save_now_and_then(10.0));
    if settings.minimap {
        upload_minimap(game);
        if settings.icons != 0 {
            upload_faces(game);
        }
    }
    if m::with(|s| s.map_visible()) {
        upload_map_tiles(game);
    }
}

/// Scans the loaded chunks around the player, nearest first, for `cut` (and
/// the surface too, so the surface map grows underground), within a budget.
fn scan(game: &Game<'_>, cut: m::Cut, settings: &m::Settings) {
    let Some(p) = game.player.as_ref() else {
        return;
    };
    let facts = game.res.facts();
    let started = Instant::now();
    let budget = Duration::from_micros(2500);
    let (cx, cz) = ((p.position.x.floor() as i32) >> 4, (p.position.z.floor() as i32) >> 4);
    let radius = game.settings.render_distance.clamp(2, 32);
    let revision = game.world.revision;
    let air = game.world.air.0;
    let cuts: &[m::Cut] = if cut == m::Cut::Surface { &[m::Cut::Surface] } else { &[cut, m::Cut::Surface] };
    let world = &game.world;
    let light = |x: i32, y: i32, z: i32| world.light(x, y, z) & 15;
    let context = game.worker_context();
    let render = &*context.render;
    PALETTE.with(|cell| {
        let mut slot = cell.borrow_mut();
        let stale = slot
            .as_ref()
            .is_none_or(|p| p.states != facts.states.len() || p.texture_layers != render.texture_layers);
        if stale {
            *slot = Some(Palette::new(facts, render));
        }
        let Some(palette) = slot.as_mut() else {
            return;
        };
        for ring in 0..=radius {
            for (dx, dz) in m::ring_offsets(ring) {
                let key = (cx + dx, cz + dz);
                let Some(chunk) = game.world.chunks.get(&key) else {
                    continue;
                };
                for &c in cuts {
                    // The surface is refreshed less often when it isn't shown.
                    let every = if c == cut { 1.0 } else { 5.0 };
                    if !m::with(|s| s.wants_scan(c, key, revision, every)) {
                        continue;
                    }
                    let mut scan = Scan {
                        chunk: Source::Live(chunk),
                        palette: &mut *palette,
                        facts,
                        render,
                        biomes: &context.biomes,
                        vanilla: settings.color_mode == 1,
                        hide: u16::from(!settings.flowers) * FLOWER
                            | u16::from(!settings.stained_glass) * GLASS
                            | u16::from(!settings.plants) * PLANT
                            | u16::from(!settings.leaves) * LEAVES,
                        cave_depth: settings.cave_depth,
                        roof_y: settings.obsidian_roof.then_some(settings.roof_y),
                        transparency: settings.transparency,
                        light: &light,
                    };
                    let tile = scan_tile(&mut scan, air, c);
                    m::with(|s| s.put_scan(c, key, revision, tile));
                    // The surface is scanned whenever the chunk changes:
                    // its portals with it.
                    if c == m::Cut::Surface {
                        let kinds = portals_in(chunk, &palette.portals);
                        m::with(|s| s.put_portals(key, kinds));
                    }
                    if started.elapsed() >= budget {
                        return;
                    }
                }
            }
        }
    });
    m::with(|s| s.forget_far_scans((cx, cz), radius + 8));
}

/// The map texture slot for slot key `id`, and whether it was just claimed
/// (blank: upload into it).
/// None when every slot is drawn this frame: try again on the next.
fn slot_for(game: &mut Game<'_>, id: i32) -> Option<(usize, bool)> {
    if let Some(slot) = game.maps.slots.show(&id) {
        return Some((slot, false));
    }
    let (slot, old) = game.maps.slots.claim(id)?;
    if let Some(old) = old {
        m::with(|s| {
            s.images.forget_id(old);
        });
    }
    Some((slot, true))
}

/// Uploads a 128 x 128 image into map texture slot `slot`.
fn upload(game: &mut Game<'_>, slot: usize, rgba: Vec<u8>) -> bool {
    let Some(name) = horizon_client::hud::map_slot_sprite(slot) else {
        return false;
    };
    let Some(rect) = game.gui.atlas.sprites.get(name).copied() else {
        return false;
    };
    let (w, h) = (game.gui.atlas.width as f32, game.gui.atlas.height as f32);
    game.map_patches.push(horizon_client::font::Patch {
        x: (rect.u0 * w).round() as u32,
        y: (rect.v0 * h).round() as u32,
        width: m::MINI as u32,
        height: m::MINI as u32,
        rgba,
    });
    true
}

fn upload_minimap(game: &mut Game<'_>) {
    let Some(p) = game.native_player() else {
        return;
    };
    let (x, z, yaw) = (p.x, p.z, p.yaw);
    if let Some([pw, ph]) = game.native_physical_window_size() {
        let (w, h) = game.gui_size();
        m::with(|s| s.size_image([w, h], [pw as f32, ph as f32]));
    }
    let world = &game.world;
    let light = |x: i32, y: i32, z: i32| world.light(x, y, z) & 15;
    let changed = m::with(|s| s.update_minimap_lit(x, z, yaw, &light));
    // Again now and then: the atlas is replaced when resources reload.
    let due = m::with(|s| s.mini_upload_due());
    if changed || due {
        let (n, image) = m::with(|s| (s.mini_n as u32, std::mem::take(&mut s.mini_image)));
        let done = game.native_mod_image(n, n, &image);
        m::with(|s| {
            s.mini_image = image;
            s.mini_ready = done;
        });
    }
}

/// Composes and uploads the world map's tiles that changed, a few a frame.
fn upload_map_tiles(game: &mut Game<'_>) {
    if let Some([pw, ph]) = game.native_physical_window_size() {
        let (w, h) = game.gui_size();
        m::with(|s| s.set_gui_scale([w, h], [pw as f32, ph as f32]));
    }
    let tiles = m::with(|s| {
        s.begin_map_frame();
        s.map.tiles(s.screen_size, s.map_cut(), s.gui_scale)
    });
    let mut budget = 6;
    // Composing far out reads many columns: a few milliseconds a frame at most.
    let started = Instant::now();
    for (key, _) in tiles {
        let id = m::with(|s| s.images.id(key));
        let Some((slot, fresh)) = slot_for(game, id) else {
            // No slot free this frame: not drawn till one is.
            m::with(|s| s.images.slots.remove(&key));
            continue;
        };
        let (stamp, shown) = m::with(|s| (s.tile_stamp(key), s.images.stamps.get(&key).copied()));
        if !fresh && shown == Some(stamp) {
            m::with(|s| s.images.slots.insert(key, slot));
            continue;
        }
        if budget == 0 || started.elapsed() > Duration::from_millis(4) {
            if fresh {
                m::with(|s| s.images.slots.remove(&key));
            }
            continue;
        }
        budget -= 1;
        let (image, whole) = m::with(|s| s.compose_tile(key));
        if upload(game, slot, image) {
            m::with(|s| {
                // Not whole: made again once the rest of it is read.
                if whole {
                    s.images.stamps.insert(key, stamp);
                } else {
                    s.images.stamps.remove(&key);
                }
                s.images.slots.insert(key, slot);
            });
        }
    }
}

/// Each mob's texture and where its face is on it (`[x, y, w, h]`).
const FACES: &[(&str, &str, [u32; 4])] = &[
    ("zombie", "zombie/zombie", [8, 8, 8, 8]),
    ("husk", "zombie/husk", [8, 8, 8, 8]),
    ("drowned", "zombie/drowned", [8, 8, 8, 8]),
    ("skeleton", "skeleton/skeleton", [8, 8, 8, 8]),
    ("stray", "skeleton/stray", [8, 8, 8, 8]),
    ("wither_skeleton", "skeleton/wither_skeleton", [8, 8, 8, 8]),
    ("creeper", "creeper/creeper", [8, 8, 8, 8]),
    ("enderman", "enderman/enderman", [8, 8, 8, 8]),
    ("spider", "spider/spider", [40, 12, 8, 8]),
    ("cave_spider", "spider/cave_spider", [40, 12, 8, 8]),
    ("pig", "pig/pig_temperate", [8, 8, 8, 8]),
    ("sheep", "sheep/sheep", [8, 8, 6, 6]),
    ("cow", "cow/cow_temperate", [6, 6, 8, 8]),
    ("chicken", "chicken/chicken_temperate", [3, 3, 4, 6]),
    ("villager", "villager/villager", [8, 8, 8, 10]),
    ("witch", "witch", [8, 8, 8, 10]),
    ("wolf", "wolf/wolf", [4, 4, 6, 6]),
    ("iron_golem", "iron_golem/iron_golem", [8, 8, 8, 10]),
    ("cat", "cat/cat_tabby", [5, 5, 5, 4]),
    ("ocelot", "cat/ocelot", [5, 5, 5, 4]),
    ("cod", "fish/cod", [0, 7, 7, 4]),
    ("salmon", "fish/salmon", [0, 8, 8, 5]),
    ("pufferfish", "fish/pufferfish", [8, 8, 8, 8]),
    ("tropical_fish", "fish/tropical_a", [0, 6, 6, 3]),
    ("fox", "fox/fox", [7, 11, 8, 6]),
    ("bee", "bee/bee", [10, 10, 7, 7]),
    ("axolotl", "axolotl/axolotl_lucy", [5, 6, 8, 5]),
    ("panda", "panda/panda", [9, 15, 13, 10]),
    ("polar_bear", "bear/polarbear", [7, 7, 7, 7]),
    ("squid", "squid/squid", [12, 12, 12, 16]),
    ("glow_squid", "squid/glow_squid", [12, 12, 12, 16]),
    ("slime", "slime/slime", [8, 8, 8, 8]),
    ("magma_cube", "slime/magmacube", [8, 8, 8, 8]),
    ("ghast", "ghast/ghast", [32, 32, 32, 32]),
    ("piglin", "piglin/piglin", [8, 8, 10, 8]),
    ("zombified_piglin", "piglin/zombified_piglin", [8, 8, 10, 8]),
    ("piglin_brute", "piglin/piglin_brute", [8, 8, 10, 8]),
    ("hoglin", "hoglin/hoglin", [80, 4, 16, 10]),
    ("zoglin", "hoglin/zoglin", [80, 4, 16, 10]),
    ("strider", "strider/strider", [16, 16, 16, 14]),
    ("guardian", "guardian/guardian", [16, 16, 12, 12]),
    ("elder_guardian", "guardian/guardian_elder", [16, 16, 12, 12]),
    ("pillager", "illager/pillager", [8, 8, 8, 10]),
    ("vindicator", "illager/vindicator", [8, 8, 8, 10]),
    ("evoker", "illager/evoker", [8, 8, 8, 10]),
    ("ravager", "illager/ravager", [16, 16, 16, 20]),
    ("zombie_villager", "zombie_villager/zombie_villager", [8, 8, 8, 10]),
    ("wandering_trader", "wandering_trader/wandering_trader", [8, 8, 8, 10]),
    ("horse", "horse/horse_brown", [0, 16, 10, 8]),
    ("donkey", "horse/donkey", [0, 16, 10, 8]),
    ("mule", "horse/mule", [0, 16, 10, 8]),
    ("llama", "llama/llama_creamy", [9, 9, 4, 4]),
    ("trader_llama", "llama/llama_creamy", [9, 9, 4, 4]),
    ("phantom", "phantom/phantom", [5, 5, 7, 3]),
    ("warden", "warden/warden", [10, 42, 16, 16]),
    ("allay", "allay/allay", [5, 5, 5, 5]),
    ("snow_golem", "snow_golem/snow_golem", [8, 8, 8, 8]),
    ("wither", "wither/wither", [8, 8, 8, 8]),
    ("bogged", "skeleton/bogged", [8, 8, 8, 8]),
    ("breeze", "breeze/breeze", [8, 8, 8, 8]),
    ("dolphin", "dolphin/dolphin", [6, 6, 8, 7]),
    ("mooshroom", "cow/mooshroom_red", [6, 6, 8, 8]),
    ("bat", "bat/bat", [0, 7, 9, 6]),
    ("blaze", "blaze/blaze", [8, 8, 8, 8]),
];

// The sheet is 8 x 8 cells.
const _: () = assert!(FACES.len() <= 64);

/// A PNG as (width, RGBA).
fn read_png(path: &std::path::Path) -> Option<(usize, Vec<u8>)> {
    let bytes = horizon_core::prepared_files::read(path).ok()?;
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    let channels = info.color_type.samples();
    let mut rgba = Vec::with_capacity(info.width as usize * info.height as usize * 4);
    for px in buf[..info.buffer_size()].chunks_exact(channels) {
        match channels {
            4 => rgba.extend_from_slice(px),
            3 => rgba.extend_from_slice(&[px[0], px[1], px[2], 255]),
            2 => rgba.extend_from_slice(&[px[0], px[0], px[0], px[1]]),
            _ => rgba.extend_from_slice(&[px[0], px[0], px[0], 255]),
        }
    }
    Some((info.width as usize, rgba))
}

/// Keeps the mob face sheet in a map texture slot: 16 x 16 texel cells.
fn upload_faces(game: &mut Game<'_>) {
    let Some((slot, fresh)) = slot_for(game, m::FACES_SLOT_KEY) else {
        return;
    };
    if !fresh && m::with(|s| s.face_slot == Some(slot)) {
        return;
    }
    let root = game.options.resources.join("assets/minecraft/textures/entity");
    let mut sheet = vec![0u8; m::MINI * m::MINI * 4];
    let mut faces = std::collections::HashMap::new();
    for (i, (mob, file, [fx, fy, fw, fh])) in FACES.iter().enumerate() {
        let Some((width, rgba)) = read_png(&root.join(format!("{file}.png"))) else {
            continue;
        };
        let (cx, cy) = (i % 8 * 16, i / 8 * 16);
        // Faces over 16 texels (a ghast's 32) every k-th texel: a cell's 16.
        let k = (*fw.max(fh)).div_ceil(16) as usize;
        let (w, h) = ((*fw as usize / k).max(1), (*fh as usize / k).max(1));
        for y in 0..h {
            for x in 0..w {
                let from = ((*fy as usize + y * k) * width + *fx as usize + x * k) * 4;
                let to = ((cy + y) * m::MINI + cx + x) * 4;
                if let (Some(px), Some(out)) = (rgba.get(from..from + 4), sheet.get_mut(to..to + 4)) {
                    out.copy_from_slice(px);
                }
            }
        }
        faces.insert(format!("minecraft:{mob}"), [cx as f32, cy as f32, w as f32, h as f32]);
    }
    if upload(game, slot, sheet) {
        m::with(|s| {
            s.faces = faces;
            s.face_slot = Some(slot);
        });
    }
}

/// The entities the minimap's dots stand for, within `reach` blocks, the
/// nearest `limit`. `skins` are the players' skin sprites by UUID.
fn dots(
    game: &Game<'_>,
    here: &m::Here,
    reach: f64,
    settings: &m::Settings,
    skins: &std::collections::HashMap<u128, String>,
) -> Vec<m::Dot> {
    let near = |x: f64, z: f64| (x - here.pos[0]).abs() <= reach && (z - here.pos[2]).abs() <= reach;
    let mut out = Vec::new();
    for mob in game.mobs.iter() {
        if mob.removed || !near(mob.position.x, mob.position.z) {
            continue;
        }
        if settings.hide_invisible && (mob.invisible || mob.has_effect("minecraft:invisibility")) {
            continue;
        }
        let uuid = game.remote_player_uuid(mob.id);
        let player = mob.kind.id == "minecraft:player" || uuid.is_some();
        let kind = if player {
            m::DotKind::Player
        } else if mob.kind.category == "misc" {
            m::DotKind::Other
        } else {
            m::DotKind::Mob
        };
        let icon = match uuid.and_then(|u| skins.get(&u)) {
            Some(skin) => m::Icon::Skin(skin.clone()),
            None if player => m::Icon::None,
            None => m::Icon::Mob(mob.kind.id.clone()),
        };
        let name = game
            .remote_player_name(mob.id)
            .map(str::to_owned)
            .or_else(|| Some(m::biome_label(&mob.kind.id)));
        out.push(m::Dot {
            pos: [mob.position.x, mob.position.y, mob.position.z],
            kind,
            icon,
            name,
        });
    }
    for item in game.dropped.iter() {
        if near(item.position.x, item.position.z) {
            out.push(m::Dot {
                pos: [item.position.x, item.position.y, item.position.z],
                kind: m::DotKind::Item,
                icon: m::Icon::Item(item.stack.item.to_string()),
                name: Some(m::biome_label(&item.stack.item)),
            });
        }
    }
    for other in game.others.iter() {
        if near(other.position.x, other.position.z) {
            out.push(m::Dot {
                pos: [other.position.x, other.position.y, other.position.z],
                kind: m::DotKind::Other,
                icon: m::Icon::None,
                name: Some(m::biome_label(&other.kind)),
            });
        }
    }
    let away = |d: &m::Dot| (d.pos[0] - here.pos[0]).powi(2) + (d.pos[2] - here.pos[2]).powi(2);
    out.sort_by(|a, b| away(a).total_cmp(&away(b)));
    out.truncate(settings.entity_limit.max(0) as usize);
    out
}

/// Draws what `atlas_mod` laid out.
pub(super) fn paint(game: &Game<'_>, batch: &mut horizon_client::hud::HudBatch, draws: Vec<m::Draw>) {
    for draw in draws {
        match draw {
            m::Draw::Fill(rect, argb) => game.native_fill(batch, rect, argb),
            m::Draw::Text(text, at, rgb, shadow) => game.native_text(batch, &text, at, rgb, shadow),
            m::Draw::Quad(points, argb) => game.native_quad(batch, points, argb),
            m::Draw::Item(item, at, size) => {
                game.native_item(batch, &item, at, size);
            }
            m::Draw::Skin(skin, dst) => {
                // The face and the hat over it, on a 64 x 64 skin.
                game.native_sprite(batch, &skin, [8.0, 8.0, 8.0, 8.0], dst);
                game.native_sprite(batch, &skin, [40.0, 8.0, 8.0, 8.0], dst);
            }
            m::Draw::Nine(sprite, rect, argb) => game.native_gui_nine_slice(batch, sprite, rect, argb),
            m::Draw::Label(text, at, argb, scale, shadow) => {
                let start = batch.vertices.len();
                let [a, r, g, b] = argb.to_be_bytes();
                let color = horizon_client::hud::rgba(r, g, b, a);
                let text = horizon_client::text::Text::literal(text);
                batch.text_styled(&game.gui.atlas, &text, at[0], at[1], color, shadow);
                batch.scale_since(start, at[0], at[1], scale, scale);
            }
            m::Draw::ModImage(src, dst, tint) => {
                paint_image(game, batch, horizon_client::hud::MOD_IMAGE_SPRITE, src, dst, tint);
            }
            m::Draw::Image(slot, src, dst, tint) => {
                if let Some(name) = horizon_client::hud::map_slot_sprite(slot) {
                    paint_image(game, batch, name, src, dst, tint);
                }
            }
        }
    }
}

/// Part `src` of sprite `name`, in texels, stretched into `dst`, tinted.
fn paint_image(
    game: &Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    name: &str,
    src: [f32; 4],
    dst: [f32; 4],
    tint: u32,
) {
    let Some(mut r) = game.gui.atlas.sprites.get(name).copied() else {
        return;
    };
    let (du, dv) = ((r.u1 - r.u0) / r.w as f32, (r.v1 - r.v0) / r.h as f32);
    r.u0 += src[0] * du;
    r.v0 += src[1] * dv;
    r.u1 = r.u0 + src[2] * du;
    r.v1 = r.v0 + src[3] * dv;
    let [a, red, g, bl] = tint.to_be_bytes();
    batch.rect_uv(dst[0], dst[1], dst[2], dst[3], r, horizon_client::hud::rgba(red, g, bl, a));
}

/// The HUD: the minimap in its corner and the waypoints' labels in the world.
pub(super) fn hud(
    game: &Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    corners: &mut horizon_client::extras_hud::Corners,
    skins: &std::collections::HashMap<u128, String>,
) {
    if !game.native_on(m::ID) {
        return;
    }
    let Some(here) = here(game) else {
        return;
    };
    let settings = m::with(|s| s.settings());
    let gui = [batch.width, batch.height];
    let measure = |t: &str| game.native_text_width(t);

    // Labels in the world, seen from the camera the world was drawn from.
    // This frame's camera, kept by the `camera` injection as the game posed
    // it, with the field of view the world is drawn with; the locator bar's,
    // a frame behind, else.
    let partial = game.clock.camera_fraction() as f32;
    let camera = m::with(|s| s.frame_camera)
        .map(|(pos, yaw, pitch)| m::Camera {
            pos,
            yaw,
            pitch,
            fov: game.fov_degrees(partial, pos),
        })
        .or_else(|| game.locator_camera.map(|(pos, yaw, pitch, fov)| m::Camera { pos, yaw, pitch, fov }));
    if let Some(camera) = camera {
        let draws = m::with(|s| s.labels(camera, here.pos, &here.dimension, gui, &measure));
        paint(game, batch, draws);
    }

    // The minimap, unless the world map covers it.
    if !settings.minimap || m::with(|s| s.map_visible()) {
        return;
    }
    if settings.hide_under_screen && !game.native_playing() {
        return;
    }
    let corner = horizon_client::extras::Corner::ALL[settings.corner.min(3)];
    if settings.hide_under_f3 && !game.native_corner_shown(corner) {
        return;
    }
    let p = horizon_core::geometry::Vec3::new(here.pos[0], here.pos[1], here.pos[2]);
    let info = m::MiniInfo {
        biome: game.biome_name_at(p),
        day_time: game.native_clock().map(|c| c.day_time),
        light: Some(game.native_light(here.pos.map(|v| v.floor() as i32))),
        weather: Some(game.native_weather()),
    };
    let reach = m::with(|s| s.mini_reach());
    let dots = if settings.dots {
        let all = dots(game, &here, reach, &settings, skins);
        m::with(|s| s.visible_dots(all, here.pos[1]))
    } else {
        Vec::new()
    };
    let [w, h] = m::with(|s| s.minimap_box(gui, &here, &info, &measure));
    let centered = settings.center_enlarged && m::with(|s| s.enlarged);
    let (x, y) = if centered {
        ((gui[0] - w) / 2.0, (gui[1] - h) / 2.0)
    } else {
        corners.place(corner, w, h)
    };
    let draws = m::with(|s| s.minimap_hud(gui, [x, y], &here, &info, &dots, &measure));
    paint(game, batch, draws);
}

// ---- Map sharing -----------------------------------------------------------
//
// The server's side runs on the server's thread (a dedicated server's, or a
// world of one's own opened to LAN), with state of its own; the client's in
// the window's frame. See `Map sharing` in core.rs.

/// How long the server's thread spends a level tick on looking at chunks.
const SHARE_SCAN_BUDGET: Duration = Duration::from_micros(600);
/// How often a loaded chunk is looked at again for changes.
const SHARE_RESCAN: Duration = Duration::from_secs(30);
/// How long the window spends a frame colouring shared chunks.
const SHARE_APPLY_BUDGET: Duration = Duration::from_micros(1500);

#[derive(Default)]
struct ServerSide {
    share: m::ShareServer,
    dedicated: bool,
    settings: Option<m::Settings>,
    last_text: Option<String>,
    read_at: Option<Instant>,
    pumped_at: Option<Instant>,
    /// Loaded chunks to look at, per dimension, each when it is due: those
    /// just loaded, and those looked at before (each queue in due order).
    queue: std::collections::HashMap<String, std::collections::VecDeque<((i32, i32), Instant)>>,
    again: std::collections::HashMap<String, std::collections::VecDeque<((i32, i32), Instant)>>,
    queued: std::collections::HashSet<(String, (i32, i32))>,
    /// The dimensions whose loaded chunks were queued when sharing began.
    seeded: std::collections::HashSet<String>,
    /// The flags of each block state, worked out once.
    flags: Vec<Option<u16>>,
    told_full: bool,
    /// A share closed without waiting, still writing.
    closing: Option<std::thread::JoinHandle<()>>,
    /// The longest a level tick's share work took, for the log at the stop.
    slowest: Duration,
}

thread_local! {
    static SERVER: RefCell<ServerSide> = RefCell::new(ServerSide::default());
}

/// Who the server shares with, from the setting: Auto is everyone on a
/// dedicated server, nobody from a world of one's own.
fn share_mode(settings: &m::Settings, dedicated: bool) -> usize {
    match settings.share_map {
        m::SHARE_AUTO if dedicated => m::SHARE_ALL,
        m::SHARE_AUTO => m::SHARE_OFF,
        mode => mode,
    }
}

/// The server is starting: whether it is a dedicated one.
pub(super) fn share_server_start(dedicated: bool) {
    SERVER.with(|cell| cell.borrow_mut().dedicated = dedicated);
}

/// The server is stopping: everything is written, then the disk thread stops.
pub(super) fn share_server_stop() {
    SERVER.with(|cell| {
        let mut sv = cell.borrow_mut();
        if sv.share.dir.is_some() {
            eprintln!("Atlas: map sharing took at most {:.2} ms a tick", sv.slowest.as_secs_f64() * 1000.0);
        }
        sv.share.close();
        if let Some(old) = sv.closing.take() {
            let _ = old.join();
        }
        sv.queue.clear();
        sv.again.clear();
        sv.queued.clear();
        sv.seeded.clear();
    });
}

/// A chunk of `dimension` loaded on the server: looked at soon, once its
/// light is there.
pub(super) fn share_chunk_loaded(dimension: &str, chunk: [i32; 2]) {
    SERVER.with(|cell| {
        let mut sv = cell.borrow_mut();
        if sv.share.dir.is_none() {
            return;
        }
        let key = (chunk[0], chunk[1]);
        if sv.queued.insert((dimension.to_owned(), key)) {
            let due = Instant::now() + Duration::from_secs(2);
            sv.queue.entry(dimension.to_owned()).or_default().push_back((key, due));
        }
    });
}

/// A level closed: have its share written.
pub(super) fn share_level_unload() {
    SERVER.with(|cell| cell.borrow().share.flush());
}

/// A player left.
pub(super) fn share_player_left(player: u128) {
    SERVER.with(|cell| cell.borrow_mut().share.leave(player));
}

/// A message from a player's client.
pub(super) fn share_from_client(game: &Game<'_>, player: u128, channel: &str, data: &[u8]) {
    if channel != m::SHARE_CHANNEL {
        return;
    }
    let Some((dimension, since)) = m::decode_hello(data) else {
        return;
    };
    let Some(p) = game.native_server_players().into_iter().find(|p| p.uuid == player) else {
        return;
    };
    // Where they are, or where they are going: the catch-up waits till the
    // server has them in that dimension.
    let at = ((p.position[0].floor() as i32) >> 4, (p.position[2].floor() as i32) >> 4);
    SERVER.with(|cell| cell.borrow_mut().share.hello(player, &dimension, since, at));
}

/// What the share keeps of chunk `key` of the level in place, or None when
/// it isn't loaded.
fn share_extract(game: &Game<'_>, flags: &mut Vec<Option<u16>>, key: (i32, i32)) -> Option<m::SharedChunk> {
    let chunk = game.world.chunks.get(&key)?;
    let facts = game.res.facts();
    if flags.len() != facts.states.len() {
        *flags = vec![None; facts.states.len()];
    }
    let air = game.world.air.0;
    let top = top_of(chunk, air);
    let bottom = chunk.min_y();
    let mut flag = |id: u32| -> u16 {
        match flags.get_mut(id as usize) {
            Some(Some(f)) => *f,
            Some(slot) => *slot.insert(state_flags(id, facts)),
            None => AIR,
        }
    };
    let mut columns = Vec::with_capacity(256);
    for z in 0..16 {
        for x in 0..16 {
            let mut cache: Vec<(u32, u16)> = Vec::new();
            let mut part_of = |s: u32| -> m::Part {
                let f = match cache.iter().find(|c| c.0 == s) {
                    Some(c) => c.1,
                    None => {
                        let f = flag(s);
                        cache.push((s, f));
                        f
                    }
                };
                if f & MAPPED == 0 {
                    m::Part::Skip
                } else if f & FLUID != 0 && f & STURDY_TOP == 0 {
                    m::Part::Fluid
                } else if f & (FLOWER | GLASS | PLANT | LEAVES | OBSIDIAN) != 0 {
                    m::Part::Through
                } else {
                    m::Part::Ground
                }
            };
            let runs = m::share_column(top, bottom, |y| chunk.get(x, y, z), &mut part_of);
            let surface = runs.first().map_or(bottom, |r| r.top);
            let (wx, wz) = (key.0 * 16 + x, key.1 * 16 + z);
            let light = (game.world.light(wx, surface + 1, wz) & 15).max(game.world.light(wx, surface, wz) & 15);
            columns.push(m::SharedColumn { biome: chunk.biome(x, surface, z), light, runs });
        }
    }
    Some(m::SharedChunk { x: key.0, z: key.1, min_y: bottom, columns })
}

/// After each level's tick on the server: reads the settings now and then,
/// opens or closes the share, looks at due chunks of this level within a
/// budget, and once a tick sends what is due.
pub(super) fn share_level_tick(game: &mut Game<'_>, dimension: &str) {
    if game.remote.is_some() {
        return;
    }
    let tick_started = Instant::now();
    let world_dir = game.native_world_dir();
    let players = game.native_server_players();
    let sends = SERVER.with(|cell| {
        let mut sv = cell.borrow_mut();
        let sv = &mut *sv;
        if sv.read_at.is_none_or(|t| t.elapsed() >= Duration::from_secs(5)) {
            // The window may be writing the file: a read is taken when it is
            // whole, when it is the first, or when it is as it was last time
            // (a file left by hand, or from an older Atlas, short of some
            // lines); a half-written one waits for the next. No file: the
            // defaults.
            let text = game.native_setting(m::ID, m::SETTINGS_FILE);
            match &text {
                None => sv.settings = Some(m::Settings::default()),
                Some(t) => {
                    if let Some(read) = m::Settings::read_whole(t) {
                        sv.settings = Some(read);
                    } else if sv.settings.is_none() || sv.last_text.as_ref() == Some(t) {
                        sv.settings = Some(m::Settings::read(|k| (k == m::SETTINGS_FILE).then(|| t.clone())));
                    }
                }
            }
            sv.last_text = text;
            sv.read_at = Some(Instant::now());
        }
        let settings = sv.settings.unwrap_or_default();
        let mode = share_mode(&settings, sv.dedicated);
        if mode == m::SHARE_OFF || world_dir.is_none() {
            if sv.share.dir.is_some() {
                // Turned off while running: no wait on this thread.
                sv.closing = sv.share.detach();
                sv.queue.clear();
                sv.again.clear();
                sv.queued.clear();
                sv.seeded.clear();
            }
            return Vec::new();
        }
        if let Some(dir) = world_dir {
            if sv.share.dir.is_none() {
                // One closed a moment ago may still be writing the folder.
                if let Some(old) = sv.closing.take() {
                    let _ = old.join();
                }
                eprintln!("Atlas: sharing the map with {}", if mode == m::SHARE_OPS { "operators" } else { "everyone" });
            }
            sv.share.open(dir.join("atlas-share"));
        }
        // The chunks each level had loaded before sharing began, at its
        // first tick since (the level in place is this one).
        if sv.seeded.insert(dimension.to_owned()) {
            let now = Instant::now();
            let q = sv.queue.entry(dimension.to_owned()).or_default();
            for &key in game.world.chunks.keys() {
                if sv.queued.insert((dimension.to_owned(), key)) {
                    q.push_back((key, now));
                }
            }
        }
        // Chunks of this level that are due, within the budget.
        if sv.share.ready() {
            let started = Instant::now();
            let now = Instant::now();
            // The new ones first, then the ones due again; each queue is in
            // due order, so the first not due ends it.
            for again in [false, true] {
                loop {
                    if started.elapsed() >= SHARE_SCAN_BUDGET {
                        break;
                    }
                    let q = if again { sv.again.get_mut(dimension) } else { sv.queue.get_mut(dimension) };
                    let Some(q) = q else {
                        break;
                    };
                    match q.front() {
                        Some(&(_, due)) if due <= now => {}
                        _ => break,
                    }
                    let Some((key, _)) = q.pop_front() else {
                        break;
                    };
                    let Some(chunk) = share_extract(game, &mut sv.flags, key) else {
                        // Unloaded: forgotten till it loads again.
                        sv.queued.remove(&(dimension.to_owned(), key));
                        continue;
                    };
                    sv.share.observe(dimension, &chunk);
                    sv.again.entry(dimension.to_owned()).or_default().push_back((key, now + SHARE_RESCAN));
                }
            }
            if sv.share.full && !sv.told_full {
                sv.told_full = true;
                eprintln!("Atlas: the shared map is full ({} chunks); new chunks aren't shared", m::MAX_SHARED_CHUNKS);
            }
        }
        // Once a tick, whichever level ticks first.
        if sv.pumped_at.is_some_and(|t| t.elapsed() < Duration::from_millis(45)) {
            return Vec::new();
        }
        sv.pumped_at = Some(Instant::now());
        let allowed: Vec<u128> = players
            .iter()
            .filter(|p| mode == m::SHARE_ALL || p.permission >= 2)
            .map(|p| p.uuid)
            .collect();
        let here: std::collections::HashMap<u128, String> =
            players.iter().map(|p| (p.uuid, p.dimension.clone())).collect();
        let mut sends = sv.share.offers(&allowed);
        sends.extend(sv.share.tick(settings.share_rate.max(1) as usize * 1024, &here));
        sends
    });
    for (player, data) in sends {
        // A full tick's worth was already sent: the rest waits (the share
        // sends at most a few a tick).
        let _ = game.native_net_send_to_player(player, m::SHARE_CHANNEL, &data);
    }
    let took = tick_started.elapsed();
    SERVER.with(|cell| {
        let mut sv = cell.borrow_mut();
        sv.slowest = sv.slowest.max(took);
    });
}

/// A message from the server, on the window.
pub(super) fn share_from_server(channel: &str, data: &[u8]) {
    if channel != m::SHARE_CHANNEL {
        return;
    }
    m::with(|s| {
        if m::FromServer::decode(data) == Some(m::FromServer::Offer) {
            // Offered (again): say hello for the map in place.
            s.share_link.offered = true;
            s.share_link.from = s.world.clone();
            s.maps.shared.hello_for = None;
            return;
        }
        // Only what the player said yes to.
        if !s.settings().server_map || !s.share_wanted() {
            return;
        }
        if let Some(dimension) = s.map_dimension.clone() {
            s.maps.shared.receive(&dimension, data);
        }
    });
}

/// The window's side, each frame: the hello once the server offered its map
/// (again for each dimension), and the shared chunks coloured within a
/// budget. The player's own scan of a loaded chunk wins over the server's.
fn share_client(game: &mut Game<'_>, settings: &m::Settings) {
    // Not on a server, or on one's own (single player, a world opened to
    // LAN): an offer is of the next one only; one's own map is already here.
    if game.remote.is_none() || game.integrated.is_some() {
        m::with(|s| s.share_link = m::ShareLink::default());
        return;
    }
    if !settings.server_map {
        return;
    }
    if let Some(hint) = m::with(|s| s.take_share_hint()) {
        game.native_chat(&hint);
    }
    // Downloaded only once the player said so (the world map asks).
    let (offered, dir, hello_for, since, dimension) = m::with(|s| {
        let offered = s.share_link.offered && s.share_wanted();
        (offered, s.maps.dir.clone(), s.maps.shared.hello_for.clone(), s.maps.shared.since, s.map_dimension.clone())
    });
    if offered
        && dir.is_some()
        && hello_for != dir
        && let Some(dimension) = dimension
    {
        // Until the player has joined this fails; it is tried next frame.
        if game.native_net_send_to_server(m::SHARE_CHANNEL, &m::encode_hello(&dimension, since)).is_ok() {
            m::with(|s| s.maps.shared.hello_for = dir);
        }
    }
    if m::with(|s| s.maps.shared.inbox.is_empty()) {
        return;
    }
    let facts = game.res.facts();
    let air = game.world.air.0;
    let context = game.worker_context();
    let render = &*context.render;
    let started = Instant::now();
    PALETTE.with(|cell| {
        let mut slot = cell.borrow_mut();
        let stale = slot
            .as_ref()
            .is_none_or(|p| p.states != facts.states.len() || p.texture_layers != render.texture_layers);
        if stale {
            *slot = Some(Palette::new(facts, render));
        }
        let Some(palette) = slot.as_mut() else {
            return;
        };
        while started.elapsed() < SHARE_APPLY_BUDGET {
            let Some(item) = m::with(|s| s.maps.shared.inbox.pop_front()) else {
                return;
            };
            let chunk = match item {
                m::Shared::Done(upto) => {
                    m::with(|s| s.maps.shared.since = s.maps.shared.since.max(upto));
                    continue;
                }
                m::Shared::Chunk(c) => c,
            };
            let key = (chunk.x, chunk.z);
            // The player's own scan of it wins.
            if m::with(|s| s.scans.contains_key(&(m::Cut::Surface, key))) {
                continue;
            }
            let light = |x: i32, _y: i32, z: i32| chunk.light(x - chunk.x * 16, z - chunk.z * 16);
            let mut scan = Scan {
                chunk: Source::Shared(&chunk, air),
                palette: &mut *palette,
                facts,
                render,
                biomes: &context.biomes,
                vanilla: settings.color_mode == 1,
                hide: u16::from(!settings.flowers) * FLOWER
                    | u16::from(!settings.stained_glass) * GLASS
                    | u16::from(!settings.plants) * PLANT
                    | u16::from(!settings.leaves) * LEAVES,
                cave_depth: settings.cave_depth,
                roof_y: settings.obsidian_roof.then_some(settings.roof_y),
                transparency: settings.transparency,
                light: &light,
            };
            let tile = scan_tile(&mut scan, air, m::Cut::Surface);
            m::with(|s| s.put_shared(key, tile));
        }
    });
}

/// `/atlas` in the chat box's tree, once a frame when the tree has none (a
/// new one each connection and when the server sends its own): each
/// subcommand and setting and what each takes, so the chat box lists and
/// completes them. The command still runs as the mod's own (`native_command`).
fn chat_tree(tree: &mut horizon_client::command::Dispatcher) {
    use horizon_client::command::{ArgType, Dispatcher, MOD_COMMAND};
    // A server's own /atlas keeps its tree.
    if tree.find("atlas").is_some() {
        return;
    }
    fn word(d: &mut Dispatcher, parent: usize, name: &str) -> usize {
        let node = d.literal(parent, name);
        d.executes(node, MOD_COMMAND)
    }
    fn arg(d: &mut Dispatcher, parent: usize, name: &str, ty: ArgType) -> usize {
        let node = d.argument(parent, name, ty);
        d.executes(node, MOD_COMMAND)
    }
    let d = tree;
    let root = word(d, 0, "atlas");
    let help = word(d, root, "help");
    for (page, _) in m::HELP_PAGES {
        word(d, help, page);
    }
    word(d, root, "dimension");
    let download = word(d, root, "download");
    word(d, download, "never");
    let seed = word(d, root, "seed");
    word(d, seed, "off");
    arg(d, seed, "seed", ArgType::Greedy);
    for w in m::command_words() {
        let key = word(d, root, w.key);
        for v in &w.values {
            let node = word(d, key, v);
            // `/atlas mode layer <y>`: the Layer mode at a height.
            if w.key == "mode" && v == "layer" {
                arg(d, node, "y", ArgType::Integer { min: -2048, max: 2048 });
            }
        }
        if let Some((min, max, whole)) = w.number {
            let ty = if whole {
                ArgType::Integer { min: min as i32, max: max as i32 }
            } else {
                ArgType::Double { min, max }
            };
            arg(d, key, "value", ty);
        }
    }
}
