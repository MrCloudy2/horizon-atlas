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
    tint: Tint,
    flags: u8,
}
const FLUID: u8 = 3;
const AIR: u8 = 4;
const STURDY_TOP: u8 = 8;
/// It has a map colour: the map item would stop at it.
const MAPPED: u8 = 16;

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
        Self {
            states: facts.states.len(),
            texture_layers: render.texture_layers,
            looks: vec![None; facts.states.len()],
            layers,
            water,
            lava,
        }
    }

    /// State `id`'s look, worked out once.
    fn look(&mut self, id: u32, facts: &BlockFacts, render: &horizon_client::assets::RenderAssets) -> Look {
        let i = id as usize;
        if let Some(Some(look)) = self.looks.get(i) {
            return *look;
        }
        let Some(f) = facts.states.get(i) else {
            return Look { rgb: 0, tint: Tint::None, flags: AIR };
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
        let leaves = name.ends_with("_leaves");
        let tinted = match name {
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
        if rgb == 0 {
            // No texture: the map item's colour.
            rgb = MAP_COLORS.get(usize::from(f.map_color)).copied().unwrap_or(0) & 0xFF_FFFF;
        }
        let look = Look { rgb, tint, flags };
        if let Some(slot) = self.looks.get_mut(i) {
            *slot = Some(look);
        }
        look
    }
}

thread_local! {
    static PALETTE: RefCell<Option<Palette>> = const { RefCell::new(None) };
}

/// What a scan reads: the chunk, the block looks, the biome colours.
struct Scan<'a> {
    chunk: &'a horizon_client::chunk::Chunk,
    palette: &'a mut Palette,
    facts: &'a BlockFacts,
    render: &'a horizon_client::assets::RenderAssets,
    biomes: &'a horizon_client::biome::Biomes,
}

impl Scan<'_> {
    fn look(&mut self, x: i32, y: i32, z: i32) -> Look {
        let id = self.chunk.get(x, y, z);
        self.palette.look(id, self.facts, self.render)
    }

    /// `look`'s colour at block `(x, y, z)` of the chunk, tinted by its biome.
    fn color(&self, look: Look, x: i32, y: i32, z: i32) -> u32 {
        let tint = match look.tint {
            Tint::None => return look.rgb,
            Tint::Constant(c) => c,
            other => {
                let id = self.chunk.biome(x, y, z);
                let b = self.biomes.get(id);
                match other {
                    Tint::Grass => {
                        let (wx, wz) = (self.chunk.x * 16 + x, self.chunk.z * 16 + z);
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
            if look.flags & MAPPED == 0 {
                y -= 1;
                continue;
            }
            let fluid = look.flags & FLUID;
            if fluid == 0 || look.flags & STURDY_TOP != 0 {
                return (m::LAND | self.color(look, x, y, z), y as i16, 0);
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
            let water = Look { rgb: self.palette.water, tint: Tint::Water, flags: look.flags };
            return (m::WATER | self.color(water, x, y, z), y as i16, depth);
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
        let floor = (cut_y - m::CUT_DEPTH).max(self.chunk.min_y());
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
    let top = top_of(scan.chunk, air);
    let mut tile = m::Tile::blank(scan.chunk.min_y() as i16);
    for z in 0..16 {
        for x in 0..16 {
            let (c, h, d) = scan.column(x, z, top, cut);
            let i = (z * 16 + x) as usize;
            tile.color[i] = c;
            tile.height[i] = h;
            tile.depth[i] = d;
        }
    }
    tile
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
    let covered = game.world.light(bx, head, bz) >> 4 == 0;
    Some(m::cut_for(settings.mode, settings.layer_y, head, covered, dimension == m::NETHER))
}

/// The world or server being played, naming its waypoints and maps.
fn world_key(game: &Game<'_>) -> Option<String> {
    game.native_player()?;
    if let (None, Some(remote)) = (&game.integrated, &game.remote) {
        return Some(format!("server-{}", remote.session.address.trim().to_lowercase()));
    }
    if let Some(folder) = game
        .world_icon
        .as_ref()
        .and_then(|icon| icon.path.parent()?.file_name())
    {
        // The single player world's save folder: the window keeps it for the
        // world's icon from the moment its server starts.
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
    let key = world_key(game);
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
    m::with(|s| s.set_cut(cut));
    scan(game, cut);
    m::with(|s| s.save_now_and_then(10.0));
    if settings.minimap {
        upload_minimap(game);
    }
    if m::with(|s| s.map_visible()) {
        upload_map_tiles(game);
    }
}

/// Scans the loaded chunks around the player, nearest first, for `cut` (and
/// the surface too, so the surface map grows underground), within a budget.
fn scan(game: &Game<'_>, cut: m::Cut) {
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
                        chunk,
                        palette: &mut *palette,
                        facts,
                        render,
                        biomes: &context.biomes,
                    };
                    let tile = scan_tile(&mut scan, air, c);
                    m::with(|s| s.put_scan(c, key, revision, tile));
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
fn slot_for(game: &mut Game<'_>, id: i32) -> (usize, bool) {
    if let Some(slot) = game.maps.slots.show(&id) {
        return (slot, false);
    }
    let (slot, old) = game.maps.slots.claim(id);
    if let Some(old) = old {
        m::with(|s| {
            s.images.forget_id(old);
            if old == m::MINIMAP_SLOT_KEY {
                s.mini_slot = None;
            }
        });
    }
    (slot, true)
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
    let changed = m::with(|s| s.update_minimap(x, z, yaw));
    let (slot, fresh) = slot_for(game, m::MINIMAP_SLOT_KEY);
    if changed || fresh || m::with(|s| s.mini_slot != Some(slot)) {
        let image = m::with(|s| s.mini_image.clone());
        if upload(game, slot, image) {
            m::with(|s| s.mini_slot = Some(slot));
        }
    }
}

/// Composes and uploads the world map's tiles that changed, a few a frame.
fn upload_map_tiles(game: &mut Game<'_>) {
    let tiles = m::with(|s| s.map.tiles(s.screen_size, s.cut));
    let mut budget = 6;
    for (key, _) in tiles {
        let id = m::with(|s| s.images.id(key));
        let (slot, fresh) = slot_for(game, id);
        let (stamp, shown) = m::with(|s| (s.tile_stamp(key), s.images.stamps.get(&key).copied()));
        if !fresh && shown == Some(stamp) {
            m::with(|s| s.images.slots.insert(key, slot));
            continue;
        }
        if budget == 0 {
            if fresh {
                m::with(|s| s.images.slots.remove(&key));
            }
            continue;
        }
        budget -= 1;
        let image = m::with(|s| s.compose_tile(key));
        if upload(game, slot, image) {
            m::with(|s| {
                s.images.stamps.insert(key, stamp);
                s.images.slots.insert(key, slot);
            });
        }
    }
}

/// The entities the minimap's dots stand for, within `reach` blocks.
fn dots(game: &Game<'_>, here: &m::Here, reach: f64) -> Vec<m::Dot> {
    let near = |x: f64, z: f64| (x - here.pos[0]).abs() <= reach && (z - here.pos[2]).abs() <= reach;
    let mut out = Vec::new();
    for mob in game.mobs.iter() {
        if mob.removed || !near(mob.position.x, mob.position.z) {
            continue;
        }
        let kind = if mob.kind.id == "minecraft:player" || game.remote_player_uuid(mob.id).is_some() {
            m::DotKind::Player
        } else if mob.kind.category == "misc" {
            m::DotKind::Other
        } else {
            m::DotKind::Mob
        };
        out.push(m::Dot {
            pos: [mob.position.x, mob.position.y, mob.position.z],
            kind,
        });
    }
    for item in game.dropped.iter() {
        if near(item.position.x, item.position.z) {
            out.push(m::Dot {
                pos: [item.position.x, item.position.y, item.position.z],
                kind: m::DotKind::Item,
            });
        }
    }
    for other in game.others.iter() {
        if near(other.position.x, other.position.z) {
            out.push(m::Dot {
                pos: [other.position.x, other.position.y, other.position.z],
                kind: m::DotKind::Other,
            });
        }
    }
    out
}

/// Draws what `atlas_mod` laid out.
pub(super) fn paint(game: &Game<'_>, batch: &mut horizon_client::hud::HudBatch, draws: Vec<m::Draw>) {
    for draw in draws {
        match draw {
            m::Draw::Fill(rect, argb) => game.native_fill(batch, rect, argb),
            m::Draw::Text(text, at, rgb, shadow) => game.native_text(batch, &text, at, rgb, shadow),
            m::Draw::Quad(points, argb) => game.native_quad(batch, points, argb),
            m::Draw::Nine(sprite, rect, argb) => game.native_gui_nine_slice(batch, sprite, rect, argb),
            m::Draw::Label(text, at, argb, scale, shadow) => {
                let start = batch.vertices.len();
                let [a, r, g, b] = argb.to_be_bytes();
                let color = horizon_client::hud::rgba(r, g, b, a);
                let text = horizon_client::text::Text::literal(text);
                batch.text_styled(&game.gui.atlas, &text, at[0], at[1], color, shadow);
                batch.scale_since(start, at[0], at[1], scale, scale);
            }
            m::Draw::Image(slot, src, dst, tint) => {
                let rect = horizon_client::hud::map_slot_sprite(slot)
                    .and_then(|name| game.gui.atlas.sprites.get(name).copied());
                if let Some(mut r) = rect {
                    let (du, dv) = ((r.u1 - r.u0) / r.w as f32, (r.v1 - r.v0) / r.h as f32);
                    r.u0 += src[0] * du;
                    r.v0 += src[1] * dv;
                    r.u1 = r.u0 + src[2] * du;
                    r.v1 = r.v0 + src[3] * dv;
                    let [a, red, g, bl] = tint.to_be_bytes();
                    let color = horizon_client::hud::rgba(red, g, bl, a);
                    batch.rect_uv(dst[0], dst[1], dst[2], dst[3], r, color);
                }
            }
        }
    }
}

/// The HUD: the minimap in its corner and the waypoints' labels in the world.
pub(super) fn hud(
    game: &Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    corners: &mut horizon_client::extras_hud::Corners,
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
    };
    let reach = f64::from(m::MINI as u32) * f64::from(m::with(|s| s.mini_zoom()));
    let dots = if settings.dots {
        let all = dots(game, &here, reach);
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
