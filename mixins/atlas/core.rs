// Horizon Atlas: a minimap, a world map and waypoints that share one map of
// the world. Every chunk the player loads is scanned into map columns (the
// map item's colours, heights and water depths) and kept on disk per world or
// server and per dimension, so the world map shows everywhere you have been.
// The minimap scrolls smoothly: its image is laid on a grid fixed to the
// world and shifted by the part of a pixel the player has moved. Waypoints
// show on both maps and in the world; the world map makes them where the
// pointer is. Both maps show the same cut of the world: the surface, or a
// cave or layer cross-section, each kept on disk on its own.
//
// This file is both the mod's Cargo library (its tests) and the module the
// bundle adds to the game; it uses only std, and the hooks hand it plain data.
//
// Built on Waypoints by Nirokx (MIT): the waypoint list, the form, the
// storage and the labels in the world. The map columns, their shading and the
// minimap's look follow Horizon's own minimap (`horizon-client-render`'s
// `minimap.rs`, MIT, Ludovic Marie and the Horizon contributors).
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::Instant;

/// The mod's id (its data folder and its keys' lines), name and version.
pub const ID: &str = "atlas";
pub const NAME: &str = "Horizon Atlas";
pub const VERSION: &str = "0.1.0";
pub const DESCRIPTION: &str = "A smooth minimap, a world map of everywhere you have been and waypoints on both. M opens the map, B makes a waypoint, U lists them.";
/// The keys' names and defaults (`key.atlas.map` in `horizon-mods.txt`).
pub const KEY_MAP: &str = "map";
pub const KEY_CREATE: &str = "create";
pub const KEY_LIST: &str = "list";
pub const DEFAULT_MAP: &str = "key.keyboard.m";
pub const DEFAULT_CREATE: &str = "key.keyboard.b";
pub const DEFAULT_LIST: &str = "key.keyboard.u";

pub const OVERWORLD: &str = "minecraft:overworld";
pub const NETHER: &str = "minecraft:the_nether";
pub const END: &str = "minecraft:the_end";

/// The colours a waypoint can have: Minecraft's text colours and a brown.
pub const PALETTE: [u32; 16] = [
    0xFF5555, 0xFFAA00, 0xFFFF55, 0x55FF55, 0x00AA00, 0x55FFFF, 0x00AAAA, 0x5555FF, 0x0000AA,
    0xAA00AA, 0xFF55FF, 0xFFFFFF, 0xAAAAAA, 0x555555, 0xAA5500, 0x000000,
];

const NAME_MAX: usize = 32;
const COORD_MAX: usize = 9;
const LINE: f32 = 10.0;
/// The lines under the minimap, smaller than the GUI's font.
const INFO_SCALE: f32 = 0.7;
const ROW: f32 = 24.0;

const INK: u32 = 0xFFFFFF;
const MUTED: u32 = 0xAAAAAA;
const ERROR: u32 = 0xFF5555;
const SHADE: u32 = 0xA000_0000;
const PANEL: u32 = 0xE010_1018;
const PANEL_EDGE: u32 = 0xFF50_5064;
const BUTTON: u32 = 0xFF34_3442;
const BUTTON_HOVER: u32 = 0xFF4E_4E66;
const BUTTON_ON: u32 = 0xFF3C_6E46;
const FIELD: u32 = 0xFF00_0000;
const FIELD_EDGE: u32 = 0xFF70_7070;
const FIELD_FOCUS: u32 = 0xFFFF_FFFF;
const ROW_BG: u32 = 0x3000_0000;
const LABEL_BG: u32 = 0x9000_0000;
/// The world map where nothing has been seen, and its bars.
const MAP_VOID: u32 = 0xFF00_0000;

/// One waypoint. `pos` is a block position; `color` is `0xRRGGBB`.
#[derive(Clone, Debug, PartialEq)]
pub struct Waypoint {
    pub name: String,
    pub pos: [i32; 3],
    pub dimension: String,
    pub color: u32,
    pub hidden: bool,
}

/// Where the player is, as the hooks read it: feet position, dimension and
/// yaw (0 faces south, 90 west).
#[derive(Clone, Debug, PartialEq)]
pub struct Here {
    pub pos: [f64; 3],
    pub dimension: String,
    pub yaw: f32,
}

/// The camera the last frame drew the world from: its position, the view's
/// yaw and pitch (a third person view facing the player already turned
/// round) and the vertical field of view in degrees.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    pub pos: [f64; 3],
    pub yaw: f32,
    pub pitch: f32,
    pub fov: f32,
}

/// What the hooks know for the lines under the minimap: the biome the
/// player is in (`minecraft:plains`) and the world's time of day in ticks.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MiniInfo {
    pub biome: Option<String>,
    pub day_time: Option<i64>,
}

/// What a dot on the minimap stands for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DotKind {
    Player,
    Mob,
    Item,
    Other,
}

impl DotKind {
    /// White players, yellow mobs, red items, purple the rest.
    pub fn color(self) -> u32 {
        match self {
            DotKind::Player => 0xFFFF_FFFF,
            DotKind::Mob => 0xFFFF_DD33,
            DotKind::Item => 0xFFFF_4444,
            DotKind::Other => 0xFFB0_55FF,
        }
    }
    fn layer(self) -> u8 {
        match self {
            DotKind::Item => 0,
            DotKind::Other => 1,
            DotKind::Mob => 2,
            DotKind::Player => 3,
        }
    }
}

/// An entity near the player, as a dot.
#[derive(Clone, Debug, PartialEq)]
pub struct Dot {
    pub pos: [f64; 3],
    pub kind: DotKind,
}

/// What a hook draws, in GUI pixels: a filled `[x, y, w, h]` in ARGB, a
/// line of text at its top left in RGB with or without a shadow, part `src`
/// (`[x, y, w, h]` in texels) of the image in map texture slot `slot`
/// stretched into `dst`, or a four-cornered shape in ARGB.
#[derive(Clone, Debug, PartialEq)]
pub enum Draw {
    Fill([f32; 4], u32),
    Text(String, [f32; 2], u32, bool),
    Image(usize, [f32; 4], [f32; 4]),
    Quad([[f32; 2]; 4], u32),
    /// Text at its top left, ARGB (it can be see-through), scaled by the
    /// factor (1 is the GUI's font size), with or without a shadow.
    Label(String, [f32; 2], u32, f32, bool),
}

/// An arrow at `at`, `len` pixels from tip to tail, pointing `degrees`
/// clockwise from up: an `argb` head over a dark outline.
pub fn arrow(draws: &mut Vec<Draw>, at: [f32; 2], degrees: f32, len: f32, argb: u32) {
    let (s, c) = degrees.to_radians().sin_cos();
    let turn = |x: f32, y: f32| [at[0] + x * c - y * s, at[1] + x * s + y * c];
    let shape = |k: f32| {
        let (h, w) = (len * 0.5 * k, len * 0.4 * k);
        [turn(0.0, -h), turn(w, h), turn(0.0, h * 0.4), turn(-w, h)]
    };
    draws.push(Draw::Quad(shape(1.35), 0xFF10_1010));
    draws.push(Draw::Quad(shape(1.0), argb));
}

/// How near in height an entity `dy` blocks above (or below) the player
/// is: 1 up to `FADE_FROM` blocks away, falling to 0 at `FADE_TO`.
pub fn height_fade(dy: f64) -> f32 {
    let d = dy.abs() as f32;
    (1.0 - (d - FADE_FROM) / (FADE_TO - FADE_FROM)).clamp(0.0, 1.0)
}
const FADE_FROM: f32 = 8.0;
const FADE_TO: f32 = 40.0;

/// `argb` with its colour times `light` and its alpha faded to 30% at
/// `near` 0.
pub fn fade(argb: u32, light: f32, near: f32) -> u32 {
    let [a, r, g, b] = argb.to_be_bytes();
    let alpha = (f32::from(a) * (0.3 + 0.7 * near)).round() as u32;
    let ch = |c: u8| (f32::from(c) * light).round().clamp(0.0, 255.0) as u32;
    alpha << 24 | ch(r) << 16 | ch(g) << 8 | ch(b)
}

/// A round dot of radius `r` at `at` (an octagon: three quads).
pub fn disc(draws: &mut Vec<Draw>, at: [f32; 2], r: f32, argb: u32) {
    let a = r * 0.414;
    let p = |x: f32, y: f32| [at[0] + x, at[1] + y];
    draws.push(Draw::Quad([p(-a, -r), p(a, -r), p(r, -a), p(-r, -a)], argb));
    draws.push(Draw::Quad([p(-r, -a), p(r, -a), p(r, a), p(-r, a)], argb));
    draws.push(Draw::Quad([p(-r, a), p(r, a), p(a, r), p(-a, r)], argb));
}

/// Text in `rgb` with a black outline, as Xaero's compass letters.
pub fn outlined(draws: &mut Vec<Draw>, text: &str, at: [f32; 2], rgb: u32) {
    for (dx, dy) in [(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0)] {
        draws.push(Draw::Text(text.into(), [at[0] + dx, at[1] + dy], 0x10_1010, false));
    }
    draws.push(Draw::Text(text.into(), at, rgb, false));
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Filter {
    All,
    Overworld,
    Nether,
    End,
}

impl Filter {
    pub const ALL: [Filter; 4] = [Filter::All, Filter::Overworld, Filter::Nether, Filter::End];

    pub fn label(self) -> &'static str {
        match self {
            Filter::All => "All",
            Filter::Overworld => "Overworld",
            Filter::Nether => "Nether",
            Filter::End => "End",
        }
    }

    pub fn matches(self, dimension: &str) -> bool {
        match self {
            Filter::All => true,
            Filter::Overworld => dimension == OVERWORLD,
            Filter::Nether => dimension == NETHER,
            Filter::End => dimension == END,
        }
    }
}

/// The editor's text fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    Name,
    X,
    Y,
    Z,
}

impl Field {
    const ORDER: [Field; 4] = [Field::Name, Field::X, Field::Y, Field::Z];

    fn step(self, back: bool) -> Field {
        let at = Field::ORDER.iter().position(|&f| f == self).unwrap_or(0);
        let n = Field::ORDER.len();
        Field::ORDER[if back { (at + n - 1) % n } else { (at + 1) % n }]
    }
}

/// Where Cancel and Save on the form go: the game, the list or the map.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Back {
    Game,
    List,
    Map,
}

/// The create / edit form. `index` is the waypoint being edited, None for a
/// new one.
#[derive(Clone, Debug, PartialEq)]
pub struct Editor {
    pub index: Option<usize>,
    pub name: String,
    pub coords: [String; 3],
    pub dimension: String,
    pub color: u32,
    pub hidden: bool,
    pub focus: Field,
    pub back: Back,
    pub error: Option<String>,
    /// The name is the suggested one, untouched: typing replaces it.
    pub suggested: bool,
}

impl Editor {
    fn field(&mut self, field: Field) -> &mut String {
        match field {
            Field::Name => &mut self.name,
            Field::X => &mut self.coords[0],
            Field::Y => &mut self.coords[1],
            Field::Z => &mut self.coords[2],
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Mode {
    Closed,
    List,
    Edit(Editor),
    Map,
}

/// What a click on the screen does.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    NewWaypoint,
    Filter(Filter),
    Toggle(usize),
    Edit(usize),
    Delete(usize),
    Focus(Field),
    Color(u32),
    CycleDimension,
    ToggleHidden,
    Here,
    Save,
    Cancel,
    Close,
    /// The world map, from the list.
    OpenMap,
    /// The list, from the map.
    OpenList,
    /// The map follows the player again.
    Centre,
    /// A waypoint's marker on the world map: edit it.
    MapWaypoint(usize),
    /// A row of the list: pick that waypoint.
    Select(usize),
    /// Teleport to a waypoint (operators).
    Teleport(usize),
    /// The next map mode (Auto, Cave, Layer).
    CycleMapMode,
    /// The Layer mode's height up or down a block (16 with Shift).
    LayerStep(i32),
}

/// A clickable rectangle `[x, y, w, h]` of the screen drawn this frame.
#[derive(Clone, Debug, PartialEq)]
pub struct Widget {
    pub rect: [f32; 4],
    pub action: Action,
}

// ---- Map columns -------------------------------------------------------------

/// A column's colour is `kind | 0xRRGGBB`: what it is in the top byte, the
/// colour the player would see from above (the block's top texture, tinted
/// by its biome) in the rest. A kind of 0 is a column with nothing in it.
pub const LAND: u32 = 1 << 24;
pub const WATER: u32 = 2 << 24;
/// Marks a column the map cuts through (solid at the cut's height).
pub const WALL: u32 = 0x80 << 24;
pub const KIND: u32 = 0x7F << 24;
pub const RGB: u32 = 0xFF_FFFF;
/// Columns that have been seen but have no colour show this, unseen ones
/// nothing.
const UNKNOWN: [u8; 4] = [20, 20, 20, 200];

/// What the maps show: the top of every column, or the floor under a
/// height (caves, the Nether, under a roof). Both maps always show the same
/// one, and each is kept in its own folder.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Cut {
    /// The top block, as map items show it.
    Surface,
    /// A cross-section: from this height down. A column that is solid for
    /// `CUT_DEPTH` blocks from it is a wall (`| WALL`); else the first floor
    /// under the air below it shows.
    Below(i32),
}

/// How deep a cut looks for air before the column counts as a wall.
pub const CUT_DEPTH: i32 = 16;

impl Cut {
    /// Its folder in a dimension's map folder.
    pub fn folder(self) -> String {
        match self {
            Cut::Surface => "surface".into(),
            Cut::Below(y) => format!("below{y}"),
        }
    }
    pub fn label(self) -> String {
        match self {
            Cut::Surface => "Surface".into(),
            Cut::Below(y) => format!("Below Y {y}"),
        }
    }
}

/// How the maps choose their cut (Options > Mods > Atlas, `/atlas mode`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapMode {
    /// The surface in the open; the cave around the player under a roof,
    /// underground and in the Nether.
    Auto,
    /// Always the cave around the player.
    Cave,
    /// Always the cut at a height the player chose.
    Layer,
}

impl MapMode {
    pub const ALL: [MapMode; 3] = [MapMode::Auto, MapMode::Cave, MapMode::Layer];
    pub fn name(self) -> &'static str {
        match self {
            MapMode::Auto => "auto",
            MapMode::Cave => "cave",
            MapMode::Layer => "layer",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            MapMode::Auto => "Auto",
            MapMode::Cave => "Cave",
            MapMode::Layer => "Layer",
        }
    }
    pub fn parse(s: &str) -> Option<MapMode> {
        MapMode::ALL.into_iter().find(|m| m.name() == s.trim().to_ascii_lowercase())
    }
}

/// The top of the 16-block band a height is in: caves are cut there, so
/// walking up and down a few blocks keeps the same map.
pub fn band_top(y: i32) -> i32 {
    y.div_euclid(CUT_DEPTH) * CUT_DEPTH + CUT_DEPTH - 1
}

/// The cut `mode` shows for a player whose head is at `head`: `covered`
/// when no sky light reaches it, `ceiling` in a dimension with a roof (the
/// Nether).
pub fn cut_for(mode: MapMode, layer_y: i32, head: i32, covered: bool, ceiling: bool) -> Cut {
    match mode {
        MapMode::Auto if !covered && !ceiling => Cut::Surface,
        MapMode::Auto | MapMode::Cave => Cut::Below(band_top(head)),
        MapMode::Layer => Cut::Below(layer_y),
    }
}

/// One chunk's map columns (`z * 16 + x`): the colour (`LAND`, `WATER`,
/// `| WALL`, with RGB), the height of the block that gave it, and the depth
/// of the water over it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tile {
    pub color: [u32; 256],
    pub height: [i16; 256],
    pub depth: [u8; 256],
}

impl Tile {
    pub fn blank(min_y: i16) -> Self {
        Self {
            color: [0; 256],
            height: [min_y; 256],
            depth: [0; 256],
        }
    }
    #[inline]
    pub fn column(&self, x: i32, z: i32) -> (u32, i16, u8) {
        let i = ((z & 15) * 16 + (x & 15)) as usize;
        (self.color[i], self.height[i], self.depth[i])
    }
}

/// A column as the maps show it: land lit by its slope (lighter where it
/// rises from the north and the west, darker where it falls), water darker
/// the deeper it is, cut walls dark.
pub fn shade(color: u32, height: i16, north: Option<i16>, west: Option<i16>, depth: u8) -> [u8; 4] {
    let rgb = color & RGB;
    let f = if color & WALL != 0 {
        0.38
    } else if color & KIND == WATER {
        1.0 - f32::from(depth.min(14)) * 0.032
    } else {
        let dn = f32::from(height - north.unwrap_or(height));
        let dw = f32::from(height - west.unwrap_or(height));
        1.0 + (dn * 0.11 + dw * 0.05).clamp(-0.32, 0.24)
    };
    let ch = |s: u32| (((rgb >> s) & 255) as f32 * f).round().clamp(0.0, 255.0) as u8;
    [ch(16), ch(8), ch(0), 255]
}

/// The colour of column `(x, z)` from `column`, shaded against the columns
/// `step` blocks north and west of it, or None where nothing was seen.
fn shaded(column: &mut dyn FnMut(i32, i32) -> Option<(u32, i16, u8)>, x: i32, z: i32, step: i32) -> Option<[u8; 4]> {
    let (color, height, depth) = column(x, z)?;
    if color & KIND == 0 {
        return Some(UNKNOWN);
    }
    let land = |c: &(u32, i16, u8)| c.0 & KIND == LAND && c.0 & WALL == 0;
    let north = column(x, z - step).filter(land).map(|c| c.1);
    let west = column(x - step, z).filter(land).map(|c| c.1);
    Some(shade(color, height, north, west, depth))
}

/// Chunk offsets at Chebyshev distance `ring`.
pub fn ring_offsets(ring: i32) -> impl Iterator<Item = (i32, i32)> {
    (-ring..=ring).flat_map(move |dz| {
        (-ring..=ring)
            .filter(move |&dx| dx.abs() == ring || dz.abs() == ring)
            .map(move |dx| (dx, dz))
    })
}

// ---- The store: map columns kept on disk ---------------------------------------

/// A region is 32 x 32 chunks, one file.
pub const REGION: i32 = 32;
const REGION_CHUNKS: usize = (REGION * REGION) as usize;
/// The most regions kept in memory (a megabyte each at most).
const MAX_REGIONS: usize = 160;
const MAGIC: &[u8; 4] = b"ATL2";
/// A chunk's bytes in a region file: colours, heights, depths.
const TILE_BYTES: usize = 256 * 4 + 256 * 2 + 256;

#[derive(Debug)]
pub struct Region {
    pub chunks: Vec<Option<Box<Tile>>>,
    pub dirty: bool,
    /// Changes whenever a chunk of it changes (or it is read again).
    pub revision: u64,
    used: u64,
}

impl Region {
    fn new(revision: u64) -> Self {
        Self {
            chunks: (0..REGION_CHUNKS).map(|_| None).collect(),
            dirty: false,
            revision,
            used: 0,
        }
    }
}

fn region_of(chunk: i32) -> i32 {
    chunk.div_euclid(REGION)
}

fn region_index(cx: i32, cz: i32) -> usize {
    (cz.rem_euclid(REGION) * REGION + cx.rem_euclid(REGION)) as usize
}

/// PackBits: runs of a byte as (-(n - 1), byte), other bytes as (n - 1, bytes).
pub fn pack(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() / 4 + 16);
    let mut i = 0;
    while i < data.len() {
        let mut run = 1;
        while i + run < data.len() && run < 128 && data[i + run] == data[i] {
            run += 1;
        }
        if run >= 3 {
            out.push((1 - run as i32) as i8 as u8);
            out.push(data[i]);
            i += run;
            continue;
        }
        let start = i;
        let mut len = 0;
        while i < data.len() && len < 128 {
            if i + 2 < data.len() && data[i] == data[i + 1] && data[i] == data[i + 2] {
                break;
            }
            i += 1;
            len += 1;
        }
        out.push((len - 1) as u8);
        out.extend_from_slice(&data[start..start + len]);
    }
    out
}

/// What `pack` wrote; None when it is cut short.
pub fn unpack(data: &[u8], expected: usize) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(expected);
    let mut i = 0;
    while i < data.len() {
        let n = data[i] as i8;
        i += 1;
        if n >= 0 {
            let len = n as usize + 1;
            out.extend_from_slice(data.get(i..i + len)?);
            i += len;
        } else if n != -128 {
            let len = (1 - i32::from(n)) as usize;
            let b = *data.get(i)?;
            i += 1;
            out.extend(std::iter::repeat_n(b, len));
        }
    }
    Some(out)
}

/// A region as its file holds it: the magic, then packed bytes of a bitmap
/// of the chunks it has and each chunk's colours, heights (little endian)
/// and depths.
pub fn encode_region(region: &Region) -> Vec<u8> {
    let mut raw = vec![0u8; REGION_CHUNKS / 8];
    for (i, c) in region.chunks.iter().enumerate() {
        if c.is_some() {
            raw[i / 8] |= 1 << (i % 8);
        }
    }
    for tile in region.chunks.iter().flatten() {
        for c in tile.color {
            raw.extend_from_slice(&c.to_le_bytes());
        }
        for h in tile.height {
            raw.extend_from_slice(&h.to_le_bytes());
        }
        raw.extend_from_slice(&tile.depth);
    }
    let mut out = MAGIC.to_vec();
    out.extend_from_slice(&(raw.len() as u32).to_le_bytes());
    out.extend(pack(&raw));
    out
}

pub fn decode_region(bytes: &[u8], revision: u64) -> Option<Region> {
    if bytes.get(..4)? != MAGIC {
        return None;
    }
    let len = u32::from_le_bytes(bytes.get(4..8)?.try_into().ok()?) as usize;
    let raw = unpack(bytes.get(8..)?, len)?;
    if raw.len() != len || len < REGION_CHUNKS / 8 {
        return None;
    }
    let mut region = Region::new(revision);
    let mut at = REGION_CHUNKS / 8;
    for i in 0..REGION_CHUNKS {
        if raw[i / 8] >> (i % 8) & 1 == 0 {
            continue;
        }
        let body = raw.get(at..at + TILE_BYTES)?;
        at += TILE_BYTES;
        let mut tile = Tile::blank(0);
        for (j, c) in tile.color.iter_mut().enumerate() {
            *c = u32::from_le_bytes(body[j * 4..j * 4 + 4].try_into().ok()?);
        }
        for (j, h) in tile.height.iter_mut().enumerate() {
            *h = i16::from_le_bytes([body[1024 + j * 2], body[1025 + j * 2]]);
        }
        tile.depth.copy_from_slice(&body[1536..1792]);
        region.chunks[i] = Some(Box::new(tile));
    }
    Some(region)
}

/// The map columns of one world (or server) and dimension, read from and
/// written to `dir` a region at a time.
#[derive(Debug, Default)]
pub struct Store {
    pub dir: Option<PathBuf>,
    regions: HashMap<(i32, i32), Region>,
    /// Regions with no file, so they aren't looked for again.
    missing: HashSet<(i32, i32)>,
    clock: u64,
    next_revision: u64,
}

impl Store {
    /// Switches to `dir` (None: nothing is kept), writing what changed first.
    #[cfg(test)]
    pub fn open(&mut self, dir: Option<PathBuf>) {
        if self.dir == dir {
            return;
        }
        self.save();
        *self = Store {
            dir,
            next_revision: self.next_revision,
            ..Store::default()
        };
    }

    fn revision(&mut self) -> u64 {
        self.next_revision += 1;
        self.next_revision
    }

    fn path(&self, r: (i32, i32)) -> Option<PathBuf> {
        Some(self.dir.as_ref()?.join(format!("r.{}.{}.atl", r.0, r.1)))
    }

    /// Region `r`, read from its file the first time; None when it has none.
    fn region(&mut self, r: (i32, i32)) -> Option<&mut Region> {
        self.clock += 1;
        if !self.regions.contains_key(&r) {
            if self.missing.contains(&r) {
                return None;
            }
            let revision = self.revision();
            let read = self
                .path(r)
                .and_then(|p| std::fs::read(p).ok())
                .and_then(|b| decode_region(&b, revision));
            let Some(region) = read else {
                self.missing.insert(r);
                return None;
            };
            self.trim();
            self.regions.insert(r, region);
        }
        let clock = self.clock;
        let region = self.regions.get_mut(&r)?;
        region.used = clock;
        Some(region)
    }

    /// Keeps chunk `(cx, cz)`'s tile; true when it changed.
    pub fn put(&mut self, cx: i32, cz: i32, tile: Tile) -> bool {
        let r = (region_of(cx), region_of(cz));
        if self.region(r).is_none() {
            self.missing.remove(&r);
            self.trim();
            let revision = self.revision();
            self.regions.insert(r, Region::new(revision));
        }
        let revision = self.revision();
        let Some(region) = self.region(r) else {
            return false;
        };
        let slot = &mut region.chunks[region_index(cx, cz)];
        if slot.as_deref() == Some(&tile) {
            return false;
        }
        *slot = Some(Box::new(tile));
        region.dirty = true;
        region.revision = revision;
        true
    }

    #[cfg(test)]
    pub fn has(&mut self, cx: i32, cz: i32) -> bool {
        self.region((region_of(cx), region_of(cz)))
            .is_some_and(|r| r.chunks[region_index(cx, cz)].is_some())
    }

    /// Column `(x, z)`, if its chunk was ever seen.
    pub fn column(&mut self, x: i32, z: i32) -> Option<(u32, i16, u8)> {
        let (cx, cz) = (x >> 4, z >> 4);
        let region = self.region((region_of(cx), region_of(cz)))?;
        region.chunks[region_index(cx, cz)]
            .as_ref()
            .map(|t| t.column(x, z))
    }

    /// A number that changes when any of the regions over blocks
    /// `x0..=x1`, `z0..=z1` does.
    pub fn stamp(&mut self, x0: i32, z0: i32, x1: i32, z1: i32) -> u64 {
        let mut stamp = 0u64;
        for rz in region_of(z0 >> 4)..=region_of(z1 >> 4) {
            for rx in region_of(x0 >> 4)..=region_of(x1 >> 4) {
                let rev = self.region((rx, rz)).map_or(0, |r| r.revision);
                stamp = stamp.wrapping_mul(0x100_0000_01B3).wrapping_add(rev);
            }
        }
        stamp
    }

    /// Writes the regions that changed.
    pub fn save(&mut self) {
        let dirty: Vec<(i32, i32)> = self
            .regions
            .iter()
            .filter(|(_, r)| r.dirty)
            .map(|(k, _)| *k)
            .collect();
        for r in dirty {
            self.write(r);
        }
    }

    fn write(&mut self, r: (i32, i32)) {
        let Some(path) = self.path(r) else {
            return;
        };
        let Some(region) = self.regions.get_mut(&r) else {
            return;
        };
        let bytes = encode_region(region);
        region.dirty = false;
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        // Whole files only: a new file, then renamed over the old one.
        let tmp = path.with_extension("atl.tmp");
        if std::fs::write(&tmp, bytes).is_ok() {
            let _ = std::fs::rename(&tmp, &path);
        }
    }

    /// Forgets the least recently used regions past `MAX_REGIONS`.
    fn trim(&mut self) {
        while self.regions.len() >= MAX_REGIONS {
            let Some(oldest) = self.regions.iter().min_by_key(|(_, r)| r.used).map(|(k, _)| *k) else {
                return;
            };
            if self.regions[&oldest].dirty {
                self.write(oldest);
            }
            self.regions.remove(&oldest);
        }
    }

    #[cfg(test)]
    pub fn regions_loaded(&self) -> usize {
        self.regions.len()
    }
}

// ---- The minimap ----------------------------------------------------------------

/// The minimap image's side in texels.
pub const MINI: usize = 128;
/// The texels the minimap shows across: one less than the image, so that a
/// fraction of a texel can be scrolled into view.
pub const MINI_SHOWN: f32 = 127.0;

pub const CORNERS: [(&str, &str); 4] = [
    ("top_left", "Top left"),
    ("top_right", "Top right"),
    ("bottom_left", "Bottom left"),
    ("bottom_right", "Bottom right"),
];
/// The minimap's side as a share of the window's height, in percent: what
/// the settings page steps through, and the most and least `/atlas size`
/// takes. A share, so the minimap keeps its place in the window whatever its
/// size and GUI scale.
pub const SIZE_STEPS: [u16; 6] = [15, 20, 25, 30, 35, 40];
pub const SIZE_MIN: u16 = 10;
pub const SIZE_MAX: u16 = 60;
/// Blocks per minimap pixel: the settings page's steps and the limits.
pub const ZOOM_STEPS: [f32; 5] = [0.5, 1.0, 2.0, 4.0, 8.0];
pub const ZOOM_MIN: f32 = 0.25;
pub const ZOOM_MAX: f32 = 16.0;

/// How the minimap's edge looks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    /// A thin dark rim.
    Thin,
    /// A thicker two-tone frame.
    Frame,
    /// No rim at all.
    Bare,
}

impl Style {
    pub const ALL: [Style; 3] = [Style::Thin, Style::Frame, Style::Bare];
    pub fn name(self) -> &'static str {
        match self {
            Style::Thin => "thin",
            Style::Frame => "frame",
            Style::Bare => "none",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Style::Thin => "Thin rim",
            Style::Frame => "Frame",
            Style::Bare => "No rim",
        }
    }
}

/// Everything Options > Mods > Atlas and `/atlas` set, kept as one small
/// file per key in the mod's folder.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    /// Waypoint names and distances in the world.
    pub labels: bool,
    pub minimap: bool,
    /// Index in `CORNERS`.
    pub corner: usize,
    /// Side in percent of the window's height.
    pub size: u16,
    pub round: bool,
    pub style: Style,
    /// Blocks per minimap pixel.
    pub zoom: f32,
    pub rotate: bool,
    pub mode: MapMode,
    /// The height `MapMode::Layer` cuts at.
    pub layer_y: i32,
    /// Dots for players, mobs, items and other entities.
    pub dots: bool,
    pub coords: bool,
    pub biome: bool,
    pub time: bool,
    /// N, E, S and W on the minimap's edge.
    pub compass: bool,
    pub slime: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            labels: true,
            minimap: true,
            corner: 0,
            size: 25,
            round: false,
            style: Style::Frame,
            zoom: 1.0,
            rotate: true,
            mode: MapMode::Auto,
            layer_y: 64,
            dots: true,
            coords: true,
            biome: true,
            time: true,
            compass: true,
            slime: false,
        }
    }
}

/// The settings page's buttons, in order: section titles and settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Knob {
    Section(&'static str),
    Labels,
    Minimap,
    Corner,
    Size,
    Shape,
    Style,
    Zoom,
    Rotate,
    Mode,
    LayerY,
    Dots,
    Coords,
    Biome,
    Time,
    Compass,
    Slime,
}

const KNOBS: [Knob; 21] = [
    Knob::Section("Minimap"),
    Knob::Minimap,
    Knob::Corner,
    Knob::Size,
    Knob::Zoom,
    Knob::Rotate,
    Knob::Section("Minimap look"),
    Knob::Shape,
    Knob::Style,
    Knob::Compass,
    Knob::Dots,
    Knob::Slime,
    Knob::Section("Under the minimap"),
    Knob::Coords,
    Knob::Biome,
    Knob::Time,
    Knob::Section("Map mode (both maps)"),
    Knob::Mode,
    Knob::LayerY,
    Knob::Section("Waypoints"),
    Knob::Labels,
];

fn on_off(b: bool) -> &'static str {
    if b { "ON" } else { "OFF" }
}

fn parse_flag(v: &str) -> Option<bool> {
    match v.trim().to_ascii_lowercase().as_str() {
        "on" | "true" | "yes" | "1" => Some(true),
        "off" | "false" | "no" | "0" => Some(false),
        _ => None,
    }
}

/// `1 block`, `½ block`, `2 blocks`.
fn blocks_text(n: f32) -> String {
    if n == 0.5 {
        "½ block".into()
    } else if n == 1.0 {
        "1 block".into()
    } else {
        format!("{n} blocks")
    }
}

/// The next of `steps` after `now` (round to the first).
fn next_step<T: PartialOrd + Copy>(steps: &[T], now: T) -> T {
    steps.iter().copied().find(|&s| s > now).unwrap_or(steps[0])
}

impl Settings {
    /// From the stored values; the defaults for the ones missing or broken.
    pub fn read(get: impl Fn(&str) -> Option<String>) -> Self {
        let mut s = Settings::default();
        for (key, _) in s.stored() {
            if let Some(v) = get(key) {
                let _ = s.set(key, v.trim());
            }
        }
        s
    }

    /// Sets `key` from text, as stored or typed after `/atlas`; the error
    /// says what it takes.
    pub fn set(&mut self, key: &str, v: &str) -> Result<(), String> {
        let flag = || parse_flag(v).ok_or_else(|| format!("{key} takes on or off"));
        match key {
            "labels" => self.labels = flag()?,
            "minimap" => self.minimap = flag()?,
            "corner" => {
                self.corner = CORNERS
                    .iter()
                    .position(|c| c.0 == v)
                    .ok_or("corner takes top_left, top_right, bottom_left or bottom_right")?;
            }
            "size" => {
                let n: u16 = v
                    .trim_end_matches('%')
                    .parse()
                    .map_err(|_| "size takes a percent of the window's height, such as 25")?;
                if !(SIZE_MIN..=SIZE_MAX).contains(&n) {
                    return Err(format!("size takes {SIZE_MIN} to {SIZE_MAX} (percent of the window's height)"));
                }
                self.size = n;
            }
            "shape" => {
                self.round = match v {
                    "round" => true,
                    "square" => false,
                    _ => return Err("shape takes round or square".into()),
                }
            }
            "style" => {
                self.style = Style::ALL
                    .into_iter()
                    .find(|s| s.name() == v)
                    .ok_or("style takes thin, frame or none")?;
            }
            "zoom" => {
                let n: f32 = v.parse().map_err(|_| "zoom takes blocks per pixel, such as 2 or 0.5")?;
                if !(ZOOM_MIN..=ZOOM_MAX).contains(&n) {
                    return Err(format!("zoom takes {ZOOM_MIN} to {ZOOM_MAX}"));
                }
                self.zoom = n;
            }
            "rotate" => self.rotate = flag()?,
            "mode" => self.mode = MapMode::parse(v).ok_or("mode takes auto, cave or layer")?,
            "layer" => {
                self.layer_y = v.parse().map_err(|_| "layer takes a height, such as 40")?;
                if !(-2048..=2048).contains(&self.layer_y) {
                    return Err("layer takes -2048 to 2048".into());
                }
            }
            "dots" => self.dots = flag()?,
            "coords" => self.coords = flag()?,
            "biome" => self.biome = flag()?,
            "time" => self.time = flag()?,
            "compass" => self.compass = flag()?,
            "slime" => self.slime = flag()?,
            _ => return Err(format!("no setting {key}")),
        }
        Ok(())
    }

    /// Each key and its value as `set` reads it.
    pub fn stored(&self) -> [(&'static str, String); 16] {
        let flag = |b: bool| if b { "on" } else { "off" }.to_owned();
        [
            ("labels", flag(self.labels)),
            ("minimap", flag(self.minimap)),
            ("corner", CORNERS[self.corner].0.to_owned()),
            ("size", self.size.to_string()),
            ("shape", if self.round { "round" } else { "square" }.to_owned()),
            ("style", self.style.name().to_owned()),
            ("zoom", self.zoom.to_string()),
            ("rotate", flag(self.rotate)),
            ("mode", self.mode.name().to_owned()),
            ("layer", self.layer_y.to_string()),
            ("dots", flag(self.dots)),
            ("coords", flag(self.coords)),
            ("biome", flag(self.biome)),
            ("time", flag(self.time)),
            ("compass", flag(self.compass)),
            ("slime", flag(self.slime)),
        ]
    }

    /// The settings page's buttons.
    pub fn labels(&self) -> Vec<String> {
        KNOBS
            .iter()
            .map(|k| match k {
                Knob::Section(title) => format!("------  {title}  ------"),
                Knob::Labels => format!("Waypoint labels in the world: {}", on_off(self.labels)),
                Knob::Minimap => format!("Minimap: {}", on_off(self.minimap)),
                Knob::Corner => format!("Corner: {}", CORNERS[self.corner].1),
                Knob::Size => format!("Size: {}% of the screen height (/atlas size)", self.size),
                Knob::Shape => format!("Shape: {}", if self.round { "Round" } else { "Square" }),
                Knob::Style => format!("Style: {}", self.style.label()),
                Knob::Zoom => format!("Zoom: {} per pixel", blocks_text(self.zoom)),
                Knob::Rotate => format!("Turn with the player: {}", on_off(self.rotate)),
                Knob::Mode => format!("Map mode: {}", self.mode.label()),
                Knob::LayerY => format!("Layer mode height: Y {} (/atlas layer)", self.layer_y),
                Knob::Dots => format!("Entity dots: {}", on_off(self.dots)),
                Knob::Coords => format!("Coordinates: {}", on_off(self.coords)),
                Knob::Biome => format!("Biome: {}", on_off(self.biome)),
                Knob::Time => format!("Time of day: {}", on_off(self.time)),
                Knob::Compass => format!("N E S W letters: {}", on_off(self.compass)),
                Knob::Slime => format!("Slime chunks: {} (needs /atlas seed)", on_off(self.slime)),
            })
            .collect()
    }

    /// Button `i` of `labels` clicked.
    pub fn click(&mut self, i: usize) {
        let Some(k) = KNOBS.get(i) else {
            return;
        };
        match k {
            Knob::Section(_) => {}
            Knob::Labels => self.labels = !self.labels,
            Knob::Minimap => self.minimap = !self.minimap,
            Knob::Corner => self.corner = (self.corner + 1) % CORNERS.len(),
            Knob::Size => self.size = next_step(&SIZE_STEPS, self.size),
            Knob::Shape => self.round = !self.round,
            Knob::Style => {
                let at = Style::ALL.iter().position(|s| *s == self.style).unwrap_or(0);
                self.style = Style::ALL[(at + 1) % Style::ALL.len()];
            }
            Knob::Zoom => self.zoom = next_step(&ZOOM_STEPS, self.zoom),
            Knob::Rotate => self.rotate = !self.rotate,
            Knob::Mode => {
                let at = MapMode::ALL.iter().position(|m| *m == self.mode).unwrap_or(0);
                self.mode = MapMode::ALL[(at + 1) % MapMode::ALL.len()];
            }
            Knob::LayerY => {
                self.layer_y = if self.layer_y >= 304 { -64 } else { self.layer_y.div_euclid(16) * 16 + 16 };
            }
            Knob::Dots => self.dots = !self.dots,
            Knob::Coords => self.coords = !self.coords,
            Knob::Biome => self.biome = !self.biome,
            Knob::Time => self.time = !self.time,
            Knob::Compass => self.compass = !self.compass,
            Knob::Slime => self.slime = !self.slime,
        }
    }

    /// `/atlas <words>`: what to say back, and whether a setting changed.
    pub fn command(&mut self, args: &str) -> (String, bool) {
        let words: Vec<&str> = args.split_whitespace().collect();
        match words.as_slice() {
            [] | ["help"] => (
                "/atlas size <10-60 %> | zoom <0.25-16> | mode auto|cave|layer [y] | layer <y> | \
                 corner top_left|top_right|bottom_left|bottom_right | shape round|square | \
                 style thin|frame|none | rotate|dots|coords|biome|time|compass|slime|labels|minimap on|off | \
                 seed <world seed>|off"
                    .into(),
                false,
            ),
            ["mode", m, y] => {
                let before = *self;
                match self.set("mode", m).and_then(|()| self.set("layer", y)) {
                    Ok(()) => (format!("Atlas: mode {m}, layer Y {y}"), true),
                    Err(e) => {
                        *self = before;
                        (format!("Atlas: {e}"), false)
                    }
                }
            }
            [key, value] => match self.set(key, value) {
                Ok(()) => (format!("Atlas: {key} {value}"), true),
                Err(e) => (format!("Atlas: {e}"), false),
            },
            [key] => match self.stored().iter().find(|s| s.0 == *key) {
                Some((_, v)) => (format!("Atlas: {key} is {v}"), false),
                None => (format!("Atlas: no setting {key} (/atlas help)"), false),
            },
            _ => ("Atlas: /atlas help lists the settings".into(), false),
        }
    }

    /// (side in GUI pixels, blocks per texel)
    pub fn view(&self) -> (f32, f32) {
        (f32::from(self.size), self.zoom)
    }
}

/// The world's directions (x, z) that point right and down on the minimap:
/// east and south north up, else turned so that where the player looks is up.
pub fn mini_axes(rotate: bool, yaw: f32) -> ([f64; 2], [f64; 2]) {
    if !rotate {
        return ([1.0, 0.0], [0.0, 1.0]);
    }
    let (s, c) = f64::from(yaw).to_radians().sin_cos();
    // Forward is (-sin, cos); right of it, seen from above, is (-cos, -sin).
    ([-c, -s], [s, -c])
}

/// Where the minimap's texels lie: the texel grid is fixed to the world in
/// the map's turned frame, `origin` is the grid cell of texel (0, 0) and
/// `shift` how far into texel space the shown area starts, so the player
/// stays in the middle of what shows (`shift + MINI_SHOWN / 2`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MiniGrid {
    pub right: [f64; 2],
    pub down: [f64; 2],
    pub bpp: f64,
    pub origin: [i64; 2],
    pub shift: [f32; 2],
}

pub fn mini_grid(x: f64, z: f64, yaw: f32, rotate: bool, bpp: f32) -> MiniGrid {
    let (right, down) = mini_axes(rotate, yaw);
    let bpp = f64::from(bpp);
    let a = (x * right[0] + z * right[1]) / bpp - 0.5;
    let b = (x * down[0] + z * down[1]) / bpp - 0.5;
    let (fa, fb) = (a.floor(), b.floor());
    MiniGrid {
        right,
        down,
        bpp,
        origin: [fa as i64 - 63, fb as i64 - 63],
        shift: [(a - fa) as f32, (b - fb) as f32],
    }
}

impl MiniGrid {
    /// The block under texel `(i, j)`'s centre.
    pub fn block(&self, i: usize, j: usize) -> (i32, i32) {
        let la = (self.origin[0] + i as i64) as f64 + 0.5;
        let lb = (self.origin[1] + j as i64) as f64 + 0.5;
        let wx = (la * self.right[0] + lb * self.down[0]) * self.bpp;
        let wz = (la * self.right[1] + lb * self.down[1]) * self.bpp;
        (wx.floor() as i32, wz.floor() as i32)
    }
    /// A world offset from the player as GUI pixels from the minimap's
    /// centre, for a minimap `size` pixels wide.
    pub fn to_screen(&self, dx: f64, dz: f64, size: f32) -> [f32; 2] {
        let scale = f64::from(size / MINI_SHOWN) / self.bpp;
        [
            ((dx * self.right[0] + dz * self.right[1]) * scale) as f32,
            ((dx * self.down[0] + dz * self.down[1]) * scale) as f32,
        ]
    }
}

/// The minimap's image for `grid` into `out` (`MINI` x `MINI` RGBA), round
/// or square, from `column`.
pub fn compose_minimap(
    grid: &MiniGrid,
    round: bool,
    column: &mut dyn FnMut(i32, i32) -> Option<(u32, i16, u8)>,
    out: &mut Vec<u8>,
) {
    out.resize(MINI * MINI * 4, 0);
    let half = f64::from(MINI_SHOWN) / 2.0;
    let centre = [
        f64::from(grid.shift[0]) + half,
        f64::from(grid.shift[1]) + half,
    ];
    for j in 0..MINI {
        for i in 0..MINI {
            let o = (j * MINI + i) * 4;
            if round {
                let (u, v) = (i as f64 + 0.5 - centre[0], j as f64 + 0.5 - centre[1]);
                let d = (u * u + v * v).sqrt();
                if d > half {
                    out[o..o + 4].copy_from_slice(&[0; 4]);
                    continue;
                }
            }
            let (x, z) = grid.block(i, j);
            let rgba = shaded(column, x, z, 1).unwrap_or(UNKNOWN);
            out[o..o + 4].copy_from_slice(&rgba);
        }
    }
}

/// Whether chunk `(cx, cz)` of a world with seed `seed` is a slime chunk
/// (`WorldgenRandom.seedSlimeChunk(x, z, seed, 987234911).nextInt(10) == 0`).
pub fn is_slime_chunk(seed: i64, cx: i32, cz: i32) -> bool {
    let mixed = seed
        .wrapping_add(i64::from(cx.wrapping_mul(cx).wrapping_mul(4_987_142)))
        .wrapping_add(i64::from(cx.wrapping_mul(5_947_611)))
        .wrapping_add(i64::from(cz.wrapping_mul(cz)).wrapping_mul(4_392_871))
        .wrapping_add(i64::from(cz.wrapping_mul(389_711)))
        ^ 987_234_911;
    // java.util.Random: setSeed, then nextInt(10).
    const MUL: i64 = 0x5_DEEC_E66D;
    const MASK: i64 = (1 << 48) - 1;
    let mut state = (mixed ^ MUL) & MASK;
    let mut next31 = || {
        state = (state.wrapping_mul(MUL).wrapping_add(0xB)) & MASK;
        (state >> 17) as i32
    };
    let bound = 10;
    let mut u = next31();
    let mut r = u % bound;
    while u.wrapping_sub(r).wrapping_add(bound - 1) < 0 {
        u = next31();
        r = u % bound;
    }
    r == 0
}

/// Greens the slime chunks on a composed minimap image.
pub fn tint_slime(grid: &MiniGrid, round: bool, seed: i64, image: &mut [u8]) {
    let _ = round;
    let mut known: HashMap<(i32, i32), bool> = HashMap::new();
    for j in 0..MINI {
        for i in 0..MINI {
            let o = (j * MINI + i) * 4;
            if image.len() < o + 4 || image[o + 3] == 0 {
                continue;
            }
            let (x, z) = grid.block(i, j);
            let chunk = (x >> 4, z >> 4);
            if !*known.entry(chunk).or_insert_with(|| is_slime_chunk(seed, chunk.0, chunk.1)) {
                continue;
            }
            for (k, green) in [40u16, 200, 60].into_iter().enumerate() {
                image[o + k] = ((u16::from(image[o + k]) * 3 + green * 2) / 5) as u8;
            }
        }
    }
}

/// What the minimap's image was composed for, to skip composing it again.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MiniKey {
    pub origin: [i64; 2],
    pub yaw_bits: u32,
    pub bpp_bits: u32,
    pub round: bool,
    pub rotate: bool,
    pub cut: Cut,
    /// The store's stamp over what the image shows.
    pub stamp: u64,
    /// The seed slime chunks were shown for.
    pub slime: Option<i64>,
}

// ---- The world map -----------------------------------------------------------------

/// A world map image: 128 x 128 texels of `1 << level` blocks each, the
/// `x`-th and `z`-th of its level.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TileKey {
    pub cut: Cut,
    pub level: u8,
    pub x: i32,
    pub z: i32,
}

pub const MAP_TEXELS: i32 = 128;
/// The deepest level: 8 blocks a texel.
pub const MAX_LEVEL: u8 = 3;
/// Blocks per GUI pixel, zoomed in and out the most.
pub const MIN_BPP: f64 = 1.0 / 8.0;
pub const MAX_BPP: f64 = 8.0;

impl TileKey {
    /// The blocks `[x0, z0, x1, z1]` (inclusive) the image covers.
    pub fn blocks(self) -> [i32; 4] {
        let span = MAP_TEXELS << self.level;
        let (x0, z0) = (self.x * span, self.z * span);
        [x0, z0, x0 + span - 1, z0 + span - 1]
    }
}

/// The level whose texels show at a GUI pixel or more at `bpp`.
pub fn map_level(bpp: f64) -> u8 {
    if bpp <= 1.0 {
        return 0;
    }
    (bpp.log2().ceil() as u8).min(MAX_LEVEL)
}

/// The world map image `key` from `store` into `out` (RGBA, transparent where
/// nothing was seen). Each texel shows the block at its centre.
pub fn compose_map_tile(store: &mut Store, key: TileKey, out: &mut Vec<u8>) {
    let n = 1i32 << key.level;
    let [x0, z0, ..] = key.blocks();
    out.clear();
    out.resize((MAP_TEXELS * MAP_TEXELS * 4) as usize, 0);
    let mut column = |x: i32, z: i32| store.column(x, z);
    for j in 0..MAP_TEXELS {
        for i in 0..MAP_TEXELS {
            let (x, z) = (x0 + i * n + n / 2, z0 + j * n + n / 2);
            if let Some(rgba) = shaded(&mut column, x, z, n) {
                let o = ((j * MAP_TEXELS + i) * 4) as usize;
                out[o..o + 4].copy_from_slice(&rgba);
            }
        }
    }
}

/// Where the world map looks: the block at the screen's centre and the
/// blocks per GUI pixel. It follows the player until dragged.
#[derive(Clone, Debug, PartialEq)]
pub struct MapView {
    pub centre: [f64; 2],
    pub bpp: f64,
    pub follow: bool,
    /// A left drag in progress, and whether it moved (so a click on a
    /// waypoint isn't a drag).
    pub dragging: bool,
    pub dragged: f32,
}

impl Default for MapView {
    fn default() -> Self {
        Self {
            centre: [0.0, 0.0],
            bpp: 1.0,
            follow: true,
            dragging: false,
            dragged: 0.0,
        }
    }
}

impl MapView {
    /// The block under GUI pixel `at` of a `size` screen.
    pub fn world(&self, at: [f32; 2], size: [f32; 2]) -> [f64; 2] {
        [
            self.centre[0] + f64::from(at[0] - size[0] / 2.0) * self.bpp,
            self.centre[1] + f64::from(at[1] - size[1] / 2.0) * self.bpp,
        ]
    }
    /// The GUI pixel of world point `(x, z)`.
    pub fn screen(&self, x: f64, z: f64, size: [f32; 2]) -> [f32; 2] {
        [
            size[0] / 2.0 + ((x - self.centre[0]) / self.bpp) as f32,
            size[1] / 2.0 + ((z - self.centre[1]) / self.bpp) as f32,
        ]
    }
    /// Zooms by `steps` wheel notches (up zooms in), keeping the block under
    /// `at` where it is.
    pub fn zoom(&mut self, steps: f64, at: [f32; 2], size: [f32; 2]) {
        let before = self.world(at, size);
        self.bpp = (self.bpp * 0.8f64.powf(steps)).clamp(MIN_BPP, MAX_BPP);
        let after = self.world(at, size);
        self.centre[0] += before[0] - after[0];
        self.centre[1] += before[1] - after[1];
    }
    pub fn pan(&mut self, delta: [f32; 2]) {
        self.centre[0] -= f64::from(delta[0]) * self.bpp;
        self.centre[1] -= f64::from(delta[1]) * self.bpp;
        self.follow = false;
    }
    /// The images a `size` screen shows, with where each goes.
    pub fn tiles(&self, size: [f32; 2], cut: Cut) -> Vec<(TileKey, [f32; 4])> {
        let level = map_level(self.bpp);
        let span = f64::from(MAP_TEXELS << level);
        let [x0, z0] = self.world([0.0, 0.0], size);
        let [x1, z1] = self.world(size, size);
        let mut out = Vec::new();
        for tz in (z0 / span).floor() as i32..=(z1 / span).floor() as i32 {
            for tx in (x0 / span).floor() as i32..=(x1 / span).floor() as i32 {
                let key = TileKey { cut, level, x: tx, z: tz };
                // Edges rounded the same way for both neighbours: no seams.
                let a = self.screen(f64::from(tx) * span, f64::from(tz) * span, size).map(f32::round);
                let b = self
                    .screen(f64::from(tx + 1) * span, f64::from(tz + 1) * span, size)
                    .map(f32::round);
                out.push((key, [a[0], a[1], b[0] - a[0], b[1] - a[1]]));
            }
        }
        out
    }
}

/// The world map's images in the game's map texture slots: which slot key
/// each was given (negative, so they never meet a real map's id), what of
/// the store it showed, and the slot it is in once uploaded.
#[derive(Debug, Default)]
pub struct MapImages {
    ids: HashMap<TileKey, i32>,
    next: i32,
    pub stamps: HashMap<TileKey, u64>,
    pub slots: HashMap<TileKey, usize>,
}

/// The minimap's own slot key: far below the world map's.
pub const MINIMAP_SLOT_KEY: i32 = i32::MIN + 7;

impl MapImages {
    pub fn clear(&mut self) {
        let next = self.next;
        *self = MapImages {
            next,
            ..MapImages::default()
        };
    }
    /// The map slot key image `key` uses.
    pub fn id(&mut self, key: TileKey) -> i32 {
        if let Some(&id) = self.ids.get(&key) {
            return id;
        }
        self.next = self.next.checked_sub(1).filter(|n| *n > i32::MIN + 1024).unwrap_or(-1);
        let id = self.next;
        // A key whose id was handed out again loses it.
        self.forget_id(id);
        self.ids.insert(key, id);
        id
    }
    /// The image with slot key `id` lost its slot (another image took it).
    pub fn forget_id(&mut self, id: i32) {
        let gone: Vec<TileKey> = self.ids.iter().filter(|(_, v)| **v == id).map(|(k, _)| *k).collect();
        for k in gone {
            self.ids.remove(&k);
            self.stamps.remove(&k);
            self.slots.remove(&k);
        }
    }
}

/// The map columns of every cut of one world's dimension, each cut in its
/// own folder under `dir`.
#[derive(Debug, Default)]
pub struct Maps {
    pub dir: Option<PathBuf>,
    stores: HashMap<Cut, Store>,
    next_revision: u64,
}

/// The most cuts kept open at once.
const MAX_CUTS: usize = 4;

impl Maps {
    /// Switches to `dir` (None: nothing is kept), writing what changed first.
    pub fn open(&mut self, dir: Option<PathBuf>) {
        if self.dir == dir {
            return;
        }
        self.save();
        self.stores.clear();
        self.dir = dir;
    }
    /// The store of `cut`, opened the first time.
    pub fn store(&mut self, cut: Cut) -> &mut Store {
        if !self.stores.contains_key(&cut) {
            if self.stores.len() >= MAX_CUTS {
                // Close one other cut (Surface last).
                let other = self
                    .stores
                    .keys()
                    .copied()
                    .filter(|c| *c != cut)
                    .max_by_key(|c| *c != Cut::Surface);
                if let Some(other) = other
                    && let Some(mut st) = self.stores.remove(&other)
                {
                    st.save();
                    self.next_revision = self.next_revision.max(st.next_revision);
                }
            }
            let mut st = Store {
                next_revision: self.next_revision,
                ..Store::default()
            };
            st.dir = self.dir.as_ref().map(|d| d.join(cut.folder()));
            self.stores.insert(cut, st);
        }
        let st = self.stores.get_mut(&cut).expect("just inserted");
        // Revisions keep growing across cuts, so a stamp never repeats.
        self.next_revision = self.next_revision.max(st.next_revision);
        st
    }
    /// Writes what changed of every cut.
    pub fn save(&mut self) {
        for st in self.stores.values_mut() {
            st.save();
        }
    }
}

// ---- State -----------------------------------------------------------------------

/// Everything the mod keeps, on the window's thread.
#[derive(Debug)]
pub struct State {
    /// The world or server the waypoints and maps belong to, and the folder
    /// the waypoints are kept in.
    pub world: Option<String>,
    pub dir: Option<PathBuf>,
    pub waypoints: Vec<Waypoint>,
    /// The world's seed, as the player typed it (`/atlas seed`), for slime
    /// chunks: a server never sends it.
    pub seed: Option<i64>,
    pub mode: Mode,
    pub filter: Filter,
    pub scroll: usize,
    /// The waypoint picked in the list, whose buttons show below it.
    pub selected: Option<usize>,
    /// The row whose Delete was clicked once and asks to be clicked again.
    pub confirm_delete: Option<usize>,
    /// The character of the key that opened the screen, which arrives right
    /// after the screen opened and must not be typed into it.
    pub skip_char: Option<char>,
    pub close_requested: bool,
    /// Options > Mods > Atlas and `/atlas`, read once.
    pub settings: Option<Settings>,
    /// Changed on the mod's own screen, not written yet.
    pub settings_changed: bool,
    /// Whether the player may teleport (an operator), as the game last said.
    pub can_teleport: bool,
    /// A teleport asked for: the dimension and block, sent by the next frame.
    pub teleport: Option<(String, [i32; 3])>,
    /// Map columns of every cut seen, kept on disk.
    pub maps: Maps,
    /// When each loaded chunk was last scanned for a cut, and the world's
    /// revision then.
    pub scans: HashMap<(Cut, (i32, i32)), (u64, Instant)>,
    /// The dimension the maps and the scans are of.
    pub map_dimension: Option<String>,
    /// What both maps show now.
    pub cut: Cut,
    pub mini_key: Option<MiniKey>,
    pub mini_grid: Option<MiniGrid>,
    pub mini_image: Vec<u8>,
    /// The map texture slot the minimap's image is in, once uploaded.
    pub mini_slot: Option<usize>,
    pub map: MapView,
    pub images: MapImages,
    /// The size the mod's screen was last drawn at, and the pointer then.
    pub screen_size: [f32; 2],
    pub mouse: [f32; 2],
    pub last_save: Option<Instant>,
    /// The camera this frame is drawn from: position, yaw, pitch.
    pub frame_camera: Option<([f64; 3], f32, f32)>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            world: None,
            dir: None,
            waypoints: Vec::new(),
            seed: None,
            mode: Mode::Closed,
            filter: Filter::All,
            scroll: 0,
            selected: None,
            confirm_delete: None,
            skip_char: None,
            close_requested: false,
            settings: None,
            settings_changed: false,
            can_teleport: false,
            teleport: None,
            maps: Maps::default(),
            scans: HashMap::new(),
            map_dimension: None,
            cut: Cut::Surface,
            mini_key: None,
            mini_grid: None,
            mini_image: Vec::new(),
            mini_slot: None,
            map: MapView::default(),
            images: MapImages::default(),
            screen_size: [480.0, 270.0],
            mouse: [0.0; 2],
            last_save: None,
            frame_camera: None,
        }
    }
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

/// Runs `f` on the mod's state. Hooks call it without calling back into the
/// game inside, so it is never borrowed twice.
pub fn with<R>(f: impl FnOnce(&mut State) -> R) -> R {
    STATE.with(|s| f(&mut s.borrow_mut()))
}

/// The letter a key name types (`key.keyboard.n` types `n`), if it is one.
pub fn key_char(key: &str) -> Option<char> {
    let last = key.strip_prefix("key.keyboard.")?;
    let mut chars = last.chars();
    let c = chars.next()?;
    chars.next().is_none().then_some(c)
}

// ---- Storage ---------------------------------------------------------------

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('\t', "\\t")
        .replace('\n', "\\n")
}

fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('t') => out.push('\t'),
            Some('n') => out.push('\n'),
            Some(other) => out.push(other),
            None => {}
        }
    }
    out
}

/// One line per waypoint: name, x, y, z, dimension, colour, hidden.
pub fn encode(list: &[Waypoint]) -> String {
    let mut out = String::from("# waypoints 1\n");
    for w in list {
        out.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{:06X}\t{}\n",
            escape(&w.name),
            w.pos[0],
            w.pos[1],
            w.pos[2],
            w.dimension,
            w.color & 0xFF_FFFF,
            u8::from(w.hidden)
        ));
    }
    out
}

/// What `encode` wrote; broken lines are skipped.
pub fn decode(text: &str) -> Vec<Waypoint> {
    text.lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .filter_map(|line| {
            let f: Vec<&str> = line.split('\t').collect();
            if f.len() < 7 {
                return None;
            }
            Some(Waypoint {
                name: unescape(f[0]),
                pos: [f[1].parse().ok()?, f[2].parse().ok()?, f[3].parse().ok()?],
                dimension: f[4].to_owned(),
                color: u32::from_str_radix(f[5], 16).ok()? & 0xFF_FFFF,
                hidden: f[6].trim() == "1",
            })
        })
        .collect()
}

/// A key or id as a plain file name.
pub fn safe_name(key: &str) -> String {
    key.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// The file a world's waypoints are kept in, named after its key.
pub fn world_file(key: &str) -> String {
    format!("{}.txt", safe_name(key))
}

/// The file a world's seed is kept in.
pub fn seed_file(key: &str) -> String {
    format!("{}.seed", safe_name(key))
}

/// A seed as the create-world screen reads it: a number, else the text's
/// `String.hashCode()`.
pub fn parse_seed(text: &str) -> Option<i64> {
    let t = text.trim();
    if t.is_empty() {
        return None;
    }
    if let Ok(n) = t.parse::<i64>() {
        return Some(n);
    }
    let hash = t.encode_utf16().fold(0i32, |h, c| h.wrapping_mul(31).wrapping_add(i32::from(c)));
    Some(i64::from(hash))
}

/// The folder a world's map of `dimension` is kept in, under the mod's.
pub fn map_dir(data: &std::path::Path, key: &str, dimension: &str) -> PathBuf {
    data.join("maps").join(safe_name(key)).join(safe_name(dimension))
}

// ---- Words -----------------------------------------------------------------

pub fn dimension_label(id: &str) -> String {
    match id {
        OVERWORLD => "Overworld".into(),
        NETHER => "Nether".into(),
        END => "The End".into(),
        other => other.rsplit(':').next().unwrap_or(other).replace('_', " "),
    }
}

pub fn dimension_color(id: &str) -> u32 {
    match id {
        OVERWORLD => 0x55FF55,
        NETHER => 0xFF5555,
        END => 0xD27BFF,
        _ => MUTED,
    }
}

fn next_dimension(id: &str) -> &'static str {
    match id {
        OVERWORLD => NETHER,
        NETHER => END,
        _ => OVERWORLD,
    }
}

/// `minecraft:dark_forest` as `Dark Forest`.
pub fn biome_label(id: &str) -> String {
    let name = id.rsplit(':').next().unwrap_or(id);
    name.split('_')
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut c = w.chars();
            c.next().map(|f| f.to_uppercase().chain(c).collect::<String>()).unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The world's time of day `ticks` as `Day 3, 14:30` (tick 0 is 6:00 of day 1).
pub fn time_text(ticks: i64) -> String {
    let day = ticks.div_euclid(24_000) + 1;
    let t = ticks.rem_euclid(24_000);
    let hours = (t / 1000 + 6) % 24;
    let minutes = t % 1000 * 60 / 1000;
    format!("Day {day}, {hours:02}:{minutes:02}")
}

/// `12 m`, then `1.2 km` from a kilometre on.
pub fn distance_text(d: f64) -> String {
    if d < 999.5 {
        format!("{} m", d.round() as i64)
    } else {
        format!("{:.1} km", d / 1000.0)
    }
}

fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    let d = sub(a, b);
    dot(d, d).sqrt()
}

/// The waypoint's block centre.
fn centre(w: &Waypoint) -> [f64; 3] {
    [
        f64::from(w.pos[0]) + 0.5,
        f64::from(w.pos[1]) + 0.5,
        f64::from(w.pos[2]) + 0.5,
    ]
}


// ---- Projection ------------------------------------------------------------

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// Where the player looks: yaw 0 faces south (+Z), 90 west; pitch 90 down.
pub fn forward(yaw: f32, pitch: f32) -> [f64; 3] {
    let (y, p) = (f64::from(yaw).to_radians(), f64::from(pitch).to_radians());
    [-y.sin() * p.cos(), -p.sin(), y.cos() * p.cos()]
}

/// The camera's forward, right and up. Right comes from the yaw alone, so
/// looking straight up or down still has one.
fn basis(yaw: f32, pitch: f32) -> ([f64; 3], [f64; 3], [f64; 3]) {
    let f = forward(yaw, pitch);
    let y = f64::from(yaw).to_radians();
    let r = [-y.cos(), 0.0, -y.sin()];
    (f, r, cross(r, f))
}

/// `point` on a `size` screen, as the world's perspective projection draws
/// it (vertical `fov`, the screen's aspect); None behind the camera.
pub fn project(
    camera: [f64; 3],
    view: ([f64; 3], [f64; 3], [f64; 3]),
    fov: f32,
    point: [f64; 3],
    size: [f32; 2],
) -> Option<[f32; 2]> {
    let (f, r, u) = view;
    let d = sub(point, camera);
    let z = dot(d, f);
    if z < 0.05 || size[1] <= 0.0 {
        return None;
    }
    let t = (f64::from(fov).to_radians() / 2.0).tan();
    let aspect = f64::from(size[0]) / f64::from(size[1]);
    let nx = dot(d, r) / (z * t * aspect);
    let ny = dot(d, u) / (z * t);
    Some([
        ((nx + 1.0) / 2.0 * f64::from(size[0])) as f32,
        ((1.0 - ny) / 2.0 * f64::from(size[1])) as f32,
    ])
}

// ---- Drawing helpers -------------------------------------------------------

struct Out<'a> {
    draws: Vec<Draw>,
    widgets: Vec<Widget>,
    mouse: [f32; 2],
    measure: &'a dyn Fn(&str) -> f32,
}

fn inside(rect: [f32; 4], at: [f32; 2]) -> bool {
    at[0] >= rect[0] && at[0] < rect[0] + rect[2] && at[1] >= rect[1] && at[1] < rect[1] + rect[3]
}

impl Out<'_> {
    fn fill(&mut self, rect: [f32; 4], argb: u32) {
        self.draws.push(Draw::Fill(rect, argb));
    }

    fn text(&mut self, text: &str, at: [f32; 2], rgb: u32) {
        self.draws.push(Draw::Text(text.to_owned(), at, rgb, true));
    }

    fn width(&self, text: &str) -> f32 {
        (self.measure)(text)
    }

    /// A 1 pixel frame round `rect`, filled with `inner`.
    fn boxed(&mut self, rect: [f32; 4], edge: u32, inner: u32) {
        let [x, y, w, h] = rect;
        self.fill(rect, edge);
        self.fill([x + 1.0, y + 1.0, w - 2.0, h - 2.0], inner);
    }

    /// `text` cut to `max` pixels with an ellipsis.
    fn fit(&self, text: &str, max: f32) -> String {
        if self.width(text) <= max {
            return text.to_owned();
        }
        let mut cut: String = text.to_owned();
        while !cut.is_empty() && self.width(&format!("{cut}...")) > max {
            cut.pop();
        }
        format!("{cut}...")
    }

    fn button(&mut self, rect: [f32; 4], label: &str, action: Action, on: bool) {
        let hover = inside(rect, self.mouse);
        let fill = if on {
            BUTTON_ON
        } else if hover {
            BUTTON_HOVER
        } else {
            BUTTON
        };
        self.boxed(rect, if hover { FIELD_FOCUS } else { PANEL_EDGE }, fill);
        let w = self.width(label);
        let at = [rect[0] + (rect[2] - w) / 2.0, rect[1] + (rect[3] - 8.0) / 2.0];
        self.text(label, at, INK);
        self.widgets.push(Widget { rect, action });
    }

    /// A button as wide as its label.
    fn button_auto(&mut self, x: f32, y: f32, label: &str, action: Action, on: bool) -> f32 {
        let w = self.width(label) + 8.0;
        self.button([x, y, w, 12.0], label, action, on);
        w
    }
}

/// `2`, `0.5`, `1.3`: a number with at most one decimal.
fn trim_float(v: f64) -> String {
    let r = (v * 10.0).round() / 10.0;
    if r.fract() == 0.0 { format!("{}", r as i64) } else { format!("{r}") }
}

/// A compass rose centred on `at`: two crossed diagonal lines and N E S W.
fn compass_rose(out: &mut Out<'_>, at: [f32; 2], r: f32) {
    let line = |out: &mut Out<'_>, a: [f32; 2], b: [f32; 2], w: f32, argb: u32| {
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let len = (dx * dx + dy * dy).sqrt().max(1e-6);
        let (nx, ny) = (-dy / len * w / 2.0, dx / len * w / 2.0);
        out.draws.push(Draw::Quad(
            [[a[0] + nx, a[1] + ny], [b[0] + nx, b[1] + ny], [b[0] - nx, b[1] - ny], [a[0] - nx, a[1] - ny]],
            argb,
        ));
    };
    let d = r * 0.7;
    for (a, b) in [([-d, -d], [d, d]), ([d, -d], [-d, d])] {
        let pa = [at[0] + a[0], at[1] + a[1]];
        let pb = [at[0] + b[0], at[1] + b[1]];
        line(out, pa, pb, 2.2, 0xFF10_1010);
        line(out, pa, pb, 1.0, 0xFFFF_FFFF);
    }
    for (letter, [dx, dy]) in [("N", [0.0, -1.0]), ("E", [1.0, 0.0]), ("S", [0.0, 1.0]), ("W", [-1.0, 0.0])] {
        let w = out.width(letter);
        let p = [(at[0] + dx * r - w / 2.0 + 0.5).round(), (at[1] + dy * r - 3.5).round()];
        outlined(&mut out.draws, letter, p, INK);
    }
}

/// A ring round `mid`, `radius` to its middle and `width` thick.
fn ring(draws: &mut Vec<Draw>, mid: [f32; 2], radius: f32, width: f32, argb: u32) {
    let n = 48;
    let (r0, r1) = (radius - width / 2.0, radius + width / 2.0);
    let at = |i: usize, r: f32| {
        let a = i as f32 / n as f32 * std::f32::consts::TAU;
        [mid[0] + a.cos() * r, mid[1] + a.sin() * r]
    };
    for i in 0..n {
        draws.push(Draw::Quad([at(i, r0), at(i, r1), at(i + 1, r1), at(i + 1, r0)], argb));
    }
}

/// The letter a waypoint shows as: the first of its name.
pub fn initial(name: &str) -> String {
    name.trim()
        .chars()
        .next()
        .map(|c| c.to_uppercase().collect())
        .unwrap_or_else(|| "?".into())
}

/// Black or white, whichever reads better on `rgb`.
fn ink_on(rgb: u32) -> u32 {
    let [_, r, g, b] = rgb.to_be_bytes();
    let light = u32::from(r) * 299 + u32::from(g) * 587 + u32::from(b) * 114;
    if light > 140_000 { 0x10_1010 } else { 0xFF_FFFF }
}

/// A waypoint on a map: its initial on a square of its colour, ringed in
/// black (white when `hot`), `half` pixels from the middle to the side.
fn letter_marker(draws: &mut Vec<Draw>, at: [f32; 2], w: &Waypoint, half: f32, hot: bool) {
    let [x, y] = [at[0].round(), at[1].round()];
    let ring = if hot { 0xFFFF_FFFF } else { 0xFF00_0000 };
    let color = if w.hidden { 0x77_7777 } else { w.color & 0xFF_FFFF };
    draws.push(Draw::Fill([x - half - 1.0, y - half - 1.0, half * 2.0 + 2.0, half * 2.0 + 2.0], ring));
    draws.push(Draw::Fill([x - half, y - half, half * 2.0, half * 2.0], 0xFF00_0000 | color));
    if half >= 4.0 {
        let letter = initial(&w.name);
        // The font's capitals are 5 pixels wide and 7 high.
        draws.push(Draw::Text(letter, [x - 2.0, y - 3.5], ink_on(color), false));
    }
}

// ---- The mod's behaviour ---------------------------------------------------

impl State {
    /// Back to nothing open (a screen closed by the game, or refused).
    pub fn reset(&mut self) {
        self.mode = Mode::Closed;
        self.confirm_delete = None;
        self.skip_char = None;
        self.map.dragging = false;
    }

    /// Switches to world `key`'s waypoints, read from `dir`, when it changed.
    pub fn load_world(&mut self, key: Option<String>, dir: PathBuf) {
        if self.world == key {
            return;
        }
        self.waypoints = key
            .as_deref()
            .and_then(|k| std::fs::read_to_string(dir.join(world_file(k))).ok())
            .map(|t| decode(&t))
            .unwrap_or_default();
        self.seed = key
            .as_deref()
            .and_then(|k| std::fs::read_to_string(dir.join(seed_file(k))).ok())
            .and_then(|t| t.trim().parse().ok());
        self.mini_key = None;
        self.world = key;
        self.dir = Some(dir);
        self.scroll = 0;
        self.map = MapView {
            bpp: self.map.bpp,
            ..MapView::default()
        };
        self.reset();
    }

    /// The settings, read with `get` the first time.
    pub fn settings_with(&mut self, get: impl Fn(&str) -> Option<String>) -> Settings {
        *self.settings.get_or_insert_with(|| Settings::read(get))
    }

    /// The settings (the defaults before they were read).
    pub fn settings(&self) -> Settings {
        self.settings.unwrap_or_default()
    }

    /// Switches the maps to `dimension` of the current world, kept in `dir`.
    pub fn load_map(&mut self, dimension: Option<String>, dir: Option<PathBuf>) {
        if self.map_dimension == dimension && self.maps.dir == dir {
            return;
        }
        self.maps.open(dir);
        self.map_dimension = dimension;
        self.scans.clear();
        self.images.clear();
        self.mini_key = None;
        self.map.follow = true;
    }

    /// `/atlas <args>`: the seed is this world's, the rest are settings.
    /// What to say back.
    pub fn command(&mut self, args: &str) -> String {
        let words: Vec<&str> = args.split_whitespace().collect();
        if words.first() == Some(&"seed") {
            let rest = args.trim_start().strip_prefix("seed").unwrap_or("").trim();
            return self.set_seed(rest);
        }
        let mut st = self.settings();
        let (said, changed) = st.command(args);
        if changed {
            self.change_settings(st);
        }
        said
    }

    /// Sets (or with `off`, forgets) this world's seed and keeps it.
    pub fn set_seed(&mut self, text: &str) -> String {
        let (Some(dir), Some(key)) = (self.dir.clone(), self.world.clone()) else {
            return "Atlas: join a world first".into();
        };
        let path = dir.join(seed_file(&key));
        if text.is_empty() {
            return match self.seed {
                Some(seed) => format!("Atlas: this world's seed is {seed}"),
                None => "Atlas: no seed set (/atlas seed <seed>)".into(),
            };
        }
        self.mini_key = None;
        if text == "off" {
            self.seed = None;
            let _ = std::fs::remove_file(path);
            return "Atlas: seed forgotten".into();
        }
        let Some(seed) = parse_seed(text) else {
            return "Atlas: /atlas seed <seed>".into();
        };
        self.seed = Some(seed);
        let _ = std::fs::create_dir_all(&dir).and_then(|()| std::fs::write(path, seed.to_string()));
        let hint = if self.settings().slime { "" } else { " (turn on with /atlas slime on)" };
        format!("Atlas: seed {seed} for slime chunks{hint}")
    }

    /// New settings, to be written by the next hook that can.
    pub fn change_settings(&mut self, st: Settings) {
        self.settings = Some(st);
        self.settings_changed = true;
        self.mini_key = None;
    }

    /// The settings to write, once, when they changed.
    pub fn take_changed_settings(&mut self) -> Option<Settings> {
        std::mem::take(&mut self.settings_changed).then(|| self.settings())
    }

    /// What both maps show from now on.
    pub fn set_cut(&mut self, cut: Cut) {
        if self.cut != cut {
            self.cut = cut;
            self.mini_key = None;
        }
    }

    /// Whether loaded chunk `chunk` wants scanning for `cut`: never scanned,
    /// or the world changed since and the last scan is `every` seconds old.
    pub fn wants_scan(&self, cut: Cut, chunk: (i32, i32), revision: u64, every: f64) -> bool {
        match self.scans.get(&(cut, chunk)) {
            None => true,
            Some(&(rev, at)) => rev != revision && at.elapsed().as_secs_f64() >= every,
        }
    }

    /// Chunk `chunk` was scanned for `cut` at world revision `revision`.
    pub fn put_scan(&mut self, cut: Cut, chunk: (i32, i32), revision: u64, tile: Tile) {
        self.scans.insert((cut, chunk), (revision, Instant::now()));
        self.maps.store(cut).put(chunk.0, chunk.1, tile);
    }

    /// Forgets the scans of chunks more than `radius` chunks from `centre`
    /// (they are rescanned when they load again).
    pub fn forget_far_scans(&mut self, centre: (i32, i32), radius: i32) {
        if self.scans.len() < 4096 {
            return;
        }
        self.scans
            .retain(|(_, (x, z)), _| (x - centre.0).abs() <= radius && (z - centre.1).abs() <= radius);
    }

    /// Writes what changed of the map, at most every `every` seconds.
    pub fn save_now_and_then(&mut self, every: f64) {
        let due = self
            .last_save
            .is_none_or(|t| t.elapsed().as_secs_f64() >= every);
        if due {
            self.maps.save();
            self.last_save = Some(Instant::now());
        }
    }

    /// Lays the minimap's grid over the player at `(x, z)` facing `yaw` and
    /// recomposes its image when what it shows changed: true when it did
    /// (upload `mini_image` then).
    pub fn update_minimap(&mut self, x: f64, z: f64, yaw: f32) -> bool {
        let seed = self.seed;
        let st = self.settings();
        let (_, bpp) = st.view();
        let grid = mini_grid(x, z, yaw, st.rotate, bpp);
        self.mini_grid = Some(grid);
        let slime = seed.filter(|_| st.slime);
        // The blocks the image can reach.
        let reach = (f64::from(MINI as u32) * grid.bpp * std::f64::consts::SQRT_2 / 2.0).ceil() as i32 + 2;
        let (bx, bz) = (x.floor() as i32, z.floor() as i32);
        let cut = self.cut;
        let stamp = self.maps.store(cut).stamp(bx - reach, bz - reach, bx + reach, bz + reach);
        let key = MiniKey {
            origin: grid.origin,
            yaw_bits: if st.rotate { yaw.to_bits() } else { 0 },
            bpp_bits: bpp.to_bits(),
            round: st.round,
            rotate: st.rotate,
            cut,
            stamp,
            slime,
        };
        if self.mini_key == Some(key) && !self.mini_image.is_empty() {
            return false;
        }
        let store = self.maps.store(cut);
        let mut column = |x: i32, z: i32| store.column(x, z);
        compose_minimap(&grid, st.round, &mut column, &mut self.mini_image);
        if let Some(seed) = slime {
            tint_slime(&grid, st.round, seed, &mut self.mini_image);
        }
        self.mini_key = Some(key);
        true
    }

    /// Writes the current world's waypoints.
    pub fn persist(&self) {
        let (Some(dir), Some(key)) = (&self.dir, &self.world) else {
            return;
        };
        let _ = std::fs::create_dir_all(dir)
            .and_then(|()| std::fs::write(dir.join(world_file(key)), encode(&self.waypoints)));
    }

    fn default_name(&self) -> String {
        let mut n = self.waypoints.len() + 1;
        while self.waypoints.iter().any(|w| w.name == format!("Waypoint {n}")) {
            n += 1;
        }
        format!("Waypoint {n}")
    }

    /// The create form, filled in with block `block` of `dimension`.
    pub fn open_create_at(&mut self, block: [i32; 3], dimension: &str, back: Back, skip: Option<char>) {
        self.mode = Mode::Edit(Editor {
            index: None,
            name: self.default_name(),
            coords: block.map(|v| v.to_string()),
            dimension: dimension.to_owned(),
            color: PALETTE[self.waypoints.len() % PALETTE.len()],
            hidden: false,
            focus: Field::Name,
            back,
            error: None,
            suggested: true,
        });
        self.skip_char = skip;
    }

    /// The create form, filled in with where the player stands.
    pub fn open_create(&mut self, here: &Here, back: Back, skip: Option<char>) {
        let block = here.pos.map(|v| v.floor() as i32);
        self.open_create_at(block, &here.dimension, back, skip);
    }

    pub fn open_list(&mut self, skip: Option<char>) {
        self.mode = Mode::List;
        self.confirm_delete = None;
        self.skip_char = skip;
    }

    /// The world map, on the player when it follows them.
    pub fn open_map(&mut self, here: Option<&Here>, skip: Option<char>) {
        self.mode = Mode::Map;
        self.skip_char = skip;
        self.map.dragging = false;
        if let Some(h) = here
            && self.map.follow
        {
            self.map.centre = [h.pos[0], h.pos[2]];
        }
    }

    fn open_edit(&mut self, index: usize, back: Back) {
        let Some(w) = self.waypoints.get(index) else {
            return;
        };
        self.mode = Mode::Edit(Editor {
            index: Some(index),
            name: w.name.clone(),
            coords: w.pos.map(|v| v.to_string()),
            dimension: w.dimension.clone(),
            color: w.color,
            hidden: w.hidden,
            focus: Field::Name,
            back,
            error: None,
            suggested: false,
        });
    }

    fn close(&mut self) {
        self.reset();
        self.close_requested = true;
    }

    /// Whether the game should close the screen now (once).
    pub fn take_close(&mut self) -> bool {
        std::mem::take(&mut self.close_requested)
    }

    /// The game's screen is (`ours`) or isn't the mod's: forget an open
    /// screen the game closed (Escape, another screen).
    pub fn sync_open(&mut self, ours: bool) {
        self.skip_char = None;
        if !ours && self.mode != Mode::Closed {
            self.reset();
        }
    }

    /// Leaves the editor: back to where it was opened from.
    fn leave_editor(&mut self, back: Back) {
        match back {
            Back::List => self.mode = Mode::List,
            Back::Map => self.mode = Mode::Map,
            Back::Game => self.close(),
        }
    }

    /// Saves the form, or says what is wrong with it.
    fn save(&mut self) {
        let Mode::Edit(editor) = &mut self.mode else {
            return;
        };
        let parsed: Vec<Option<i32>> = editor
            .coords
            .iter()
            .map(|c| c.trim().parse::<i32>().ok())
            .collect();
        let [Some(x), Some(y), Some(z)] = parsed[..] else {
            editor.error = Some("Coordinates must be whole numbers".into());
            return;
        };
        let name = editor.name.trim().to_owned();
        let waypoint = Waypoint {
            name,
            pos: [x, y, z],
            dimension: editor.dimension.clone(),
            color: editor.color,
            hidden: editor.hidden,
        };
        let (index, back) = (editor.index, editor.back);
        let waypoint = if waypoint.name.is_empty() {
            Waypoint {
                name: self.default_name(),
                ..waypoint
            }
        } else {
            waypoint
        };
        match index.filter(|&i| i < self.waypoints.len()) {
            Some(i) => self.waypoints[i] = waypoint,
            None => self.waypoints.push(waypoint),
        }
        self.persist();
        self.leave_editor(back);
    }

    fn delete(&mut self, index: usize) {
        if self.confirm_delete != Some(index) {
            self.confirm_delete = Some(index);
            return;
        }
        self.confirm_delete = None;
        if index < self.waypoints.len() {
            self.waypoints.remove(index);
            self.persist();
        }
        self.selected = match self.selected {
            Some(s) if s == index => None,
            Some(s) if s > index => Some(s - 1),
            other => other,
        };
        // A form open on it from the map goes back there.
        if let Mode::Edit(e) = &self.mode
            && e.index == Some(index)
        {
            let back = e.back;
            self.leave_editor(back);
        }
    }

    /// Does what a click on `action` does.
    pub fn act(&mut self, action: Action, here: Option<&Here>) {
        if !matches!(action, Action::Delete(_)) {
            self.confirm_delete = None;
        }
        match action {
            Action::NewWaypoint => {
                if let Some(here) = here {
                    self.open_create(here, Back::List, None);
                }
            }
            Action::Filter(filter) => {
                self.filter = filter;
                self.scroll = 0;
            }
            Action::Select(i) => {
                self.selected = (self.selected != Some(i)).then_some(i);
            }
            Action::Teleport(i) => {
                if let Some(w) = self.waypoints.get(i)
                    && self.can_teleport
                {
                    self.teleport = Some((w.dimension.clone(), w.pos));
                    self.close();
                }
            }
            Action::Toggle(i) => {
                if let Some(w) = self.waypoints.get_mut(i) {
                    w.hidden = !w.hidden;
                    self.persist();
                }
            }
            Action::Edit(i) => self.open_edit(i, Back::List),
            Action::MapWaypoint(i) => self.open_edit(i, Back::Map),
            Action::Delete(i) => self.delete(i),
            Action::Close => self.close(),
            Action::Save => self.save(),
            Action::Cancel => {
                if let Mode::Edit(e) = &self.mode {
                    let back = e.back;
                    self.leave_editor(back);
                }
            }
            Action::Focus(field) => {
                if let Mode::Edit(e) = &mut self.mode {
                    e.focus = field;
                }
            }
            Action::Color(color) => {
                if let Mode::Edit(e) = &mut self.mode {
                    e.color = color;
                }
            }
            Action::CycleDimension => {
                if let Mode::Edit(e) = &mut self.mode {
                    e.dimension = next_dimension(&e.dimension).to_owned();
                }
            }
            Action::ToggleHidden => {
                if let Mode::Edit(e) = &mut self.mode {
                    e.hidden = !e.hidden;
                }
            }
            Action::Here => {
                if let (Mode::Edit(e), Some(here)) = (&mut self.mode, here) {
                    e.coords = here.pos.map(|v| (v.floor() as i32).to_string());
                    e.dimension = here.dimension.clone();
                    e.error = None;
                }
            }
            Action::OpenMap => self.open_map(here, None),
            Action::OpenList => self.open_list(None),
            Action::CycleMapMode => {
                let mut st = self.settings();
                let at = MapMode::ALL.iter().position(|m| *m == st.mode).unwrap_or(0);
                st.mode = MapMode::ALL[(at + 1) % MapMode::ALL.len()];
                self.change_settings(st);
            }
            Action::LayerStep(n) => {
                let mut st = self.settings();
                st.layer_y = (st.layer_y + n).clamp(-2048, 2048);
                self.change_settings(st);
            }
            Action::Centre => {
                self.map.follow = true;
                if let Some(h) = here {
                    self.map.centre = [h.pos[0], h.pos[2]];
                }
            }
        }
    }

    /// A key pressed over the mod's screen (all of them are the mod's).
    /// `keys` are the player's map, create and list keys.
    pub fn key(&mut self, key: &str, shift: bool, here: Option<&Here>, keys: [&str; 3]) {
        let [map_key, create_key, list_key] = keys;
        match &mut self.mode {
            Mode::Closed => {}
            Mode::List => match key {
                "key.keyboard.escape" => self.close(),
                "key.keyboard.up" => self.scroll = self.scroll.saturating_sub(1),
                "key.keyboard.down" => self.scroll += 1,
                k if k == list_key => self.close(),
                k if k == map_key => self.open_map(here, key_char(k)),
                k if k == create_key => {
                    if let Some(here) = here {
                        self.open_create(here, Back::List, key_char(k));
                    }
                }
                _ => {}
            },
            Mode::Map => {
                let size = self.screen_size;
                let pan = 32.0;
                match key {
                    "key.keyboard.escape" => self.close(),
                    "key.keyboard.up" => self.map.pan([0.0, pan]),
                    "key.keyboard.down" => self.map.pan([0.0, -pan]),
                    "key.keyboard.left" => self.map.pan([pan, 0.0]),
                    "key.keyboard.right" => self.map.pan([-pan, 0.0]),
                    "key.keyboard.equal" | "key.keyboard.keypad.add" => {
                        self.map.zoom(1.0, [size[0] / 2.0, size[1] / 2.0], size);
                    }
                    "key.keyboard.minus" | "key.keyboard.keypad.subtract" => {
                        self.map.zoom(-1.0, [size[0] / 2.0, size[1] / 2.0], size);
                    }
                    "key.keyboard.space" => self.act(Action::Centre, here),
                    k if k == map_key => self.close(),
                    k if k == list_key => self.open_list(key_char(k)),
                    // A new waypoint where the pointer is.
                    k if k == create_key => {
                        let mouse = self.mouse;
                        self.create_at_pointer(mouse, here, key_char(k));
                    }
                    _ => {}
                }
            }
            Mode::Edit(editor) => match key {
                "key.keyboard.escape" => {
                    let back = editor.back;
                    self.leave_editor(back);
                }
                "key.keyboard.enter" | "key.keyboard.keypad.enter" => self.save(),
                "key.keyboard.tab" => editor.focus = editor.focus.step(shift),
                "key.keyboard.up" => editor.focus = editor.focus.step(true),
                "key.keyboard.down" => editor.focus = editor.focus.step(false),
                "key.keyboard.backspace" => {
                    let focus = editor.focus;
                    if focus == Field::Name && editor.suggested {
                        editor.name.clear();
                    } else {
                        editor.field(focus).pop();
                    }
                    editor.suggested = false;
                    editor.error = None;
                }
                "key.keyboard.delete" => {
                    let focus = editor.focus;
                    editor.field(focus).clear();
                    editor.error = None;
                }
                _ => {}
            },
        }
    }

    /// A character typed over the mod's screen.
    pub fn char_typed(&mut self, c: char) {
        if let Some(skip) = self.skip_char.take()
            && skip.eq_ignore_ascii_case(&c)
        {
            return;
        }
        let Mode::Edit(editor) = &mut self.mode else {
            return;
        };
        let focus = editor.focus;
        if focus == Field::Name && std::mem::take(&mut editor.suggested) {
            editor.name.clear();
        }
        let field = editor.field(focus);
        let ok = match focus {
            Field::Name => field.chars().count() < NAME_MAX && !c.is_control(),
            _ => {
                field.len() < COORD_MAX && (c.is_ascii_digit() || (c == '-' && field.is_empty()))
            }
        };
        if ok {
            field.push(c);
            editor.error = None;
        }
    }

    /// The wheel over the mod's screen: the list scrolls a row a notch, the
    /// map zooms round the pointer.
    pub fn scroll_by(&mut self, steps: f64, mouse: [f32; 2]) {
        if self.mode == Mode::Map {
            let size = self.screen_size;
            self.map.zoom(steps, mouse, size);
            return;
        }
        if steps > 0.0 {
            self.scroll = self.scroll.saturating_sub(1);
        } else if steps < 0.0 {
            self.scroll += 1;
        }
    }

    /// A mouse button went down on the mod's screen at `mouse`, over
    /// `widgets` (this frame's).
    pub fn press(&mut self, button: i32, mouse: [f32; 2], widgets: &[Widget], here: Option<&Here>) {
        if let Some(action) = hit(widgets, mouse) {
            if button == 0 {
                self.act(action, here);
            }
            return;
        }
        if self.mode != Mode::Map {
            return;
        }
        match button {
            0 => {
                self.map.dragging = true;
                self.map.dragged = 0.0;
            }
            1 => self.create_at_pointer(mouse, here, None),
            _ => {}
        }
    }

    /// The teleport asked for, once: `/execute in <dimension> run tp @s x y z`.
    pub fn take_teleport(&mut self) -> Option<String> {
        let (dimension, [x, y, z]) = self.teleport.take()?;
        Some(format!("/execute in {dimension} run tp @s {x}.5 {y} {z}.5"))
    }

    /// The dots on the level the maps show: on the surface, entities no
    /// more than a few blocks under the ground the map shows where they
    /// are; in a cut, those inside its band.
    pub fn visible_dots(&mut self, dots: Vec<Dot>, player_y: f64) -> Vec<Dot> {
        let cut = self.cut;
        let store = self.maps.store(cut);
        dots.into_iter()
            .filter(|d| {
                let y = d.pos[1];
                match cut {
                    Cut::Below(top) => {
                        let top = f64::from(top);
                        y <= top + 2.0 && y >= top - f64::from(CUT_DEPTH) - 8.0
                    }
                    Cut::Surface => match store.column(d.pos[0].floor() as i32, d.pos[2].floor() as i32) {
                        Some((c, h, _)) if c & KIND != 0 => y >= f64::from(h) - 3.0,
                        _ => (y - player_y).abs() <= 24.0,
                    },
                }
            })
            .collect()
    }

    /// Whether the world map shows (alone, or under a form opened from it).
    pub fn map_visible(&self) -> bool {
        match &self.mode {
            Mode::Map => true,
            Mode::Edit(e) => e.back == Back::Map,
            _ => false,
        }
    }

    /// A number that changes when what world map image `key` shows does.
    pub fn tile_stamp(&mut self, key: TileKey) -> u64 {
        let [x0, z0, x1, z1] = key.blocks();
        self.maps.store(key.cut).stamp(x0, z0, x1, z1)
    }

    /// World map image `key`, 128 x 128 RGBA.
    pub fn compose_tile(&mut self, key: TileKey) -> Vec<u8> {
        let mut out = Vec::new();
        compose_map_tile(self.maps.store(key.cut), key, &mut out);
        out
    }

    /// The block under the world map's GUI pixel `at`: x, the height the
    /// map shows there (a block above its floor), z.
    pub fn map_block(&mut self, at: [f32; 2], here: Option<&Here>) -> [i32; 3] {
        let [x, z] = self.map.world(at, self.screen_size);
        let (x, z) = (x.floor() as i32, z.floor() as i32);
        let cut = self.cut;
        let y = self
            .maps
            .store(cut)
            .column(x, z)
            .filter(|c| c.0 & KIND != 0)
            .map(|c| i32::from(c.1) + 1)
            .or_else(|| here.map(|h| h.pos[1].floor() as i32))
            .unwrap_or(64);
        [x, y, z]
    }

    /// The create form for the block under the world map's pointer.
    fn create_at_pointer(&mut self, at: [f32; 2], here: Option<&Here>, skip: Option<char>) {
        let Some(dimension) = here.map(|h| h.dimension.clone()) else {
            return;
        };
        let block = self.map_block(at, here);
        self.open_create_at(block, &dimension, Back::Map, skip);
    }

    pub fn release(&mut self, button: i32) {
        if button == 0 {
            self.map.dragging = false;
        }
    }

    /// The pointer moved `delta` with a button held.
    pub fn drag(&mut self, button: i32, delta: [f32; 2]) -> bool {
        if self.mode != Mode::Map || button != 0 || !self.map.dragging {
            return false;
        }
        self.map.dragged += delta[0].abs() + delta[1].abs();
        self.map.pan(delta);
        true
    }

    /// The labels of this dimension's visible waypoints, far ones first so
    /// near ones draw over them.
    /// Distances are from the player's feet (`player`), as the list shows them.
    pub fn labels(
        &self,
        camera: Camera,
        player: [f64; 3],
        dimension: &str,
        size: [f32; 2],
        measure: &dyn Fn(&str) -> f32,
    ) -> Vec<Draw> {
        if !self.settings().labels {
            return Vec::new();
        }
        let view = basis(camera.yaw, camera.pitch);
        let mut shown: Vec<(f64, [f32; 2], &Waypoint)> = self
            .waypoints
            .iter()
            .filter(|w| !w.hidden && w.dimension == dimension)
            .filter_map(|w| {
                let at = project(camera.pos, view, camera.fov, centre(w), size)?;
                let on_screen = at[0] >= 0.0 && at[0] <= size[0] && at[1] >= 0.0 && at[1] <= size[1];
                on_screen.then(|| (distance(player, centre(w)), at, w))
            })
            .collect();
        shown.sort_by(|a, b| b.0.total_cmp(&a.0));
        let mut draws = Vec::new();
        for (d, [x, y], w) in shown {
            // As Xaero's: a see-through square of its colour with its
            // letter, and its name and distance small under it, placed to
            // the sub-pixel so they glide as the view turns.
            let half = 4.5;
            let color = w.color & RGB;
            draws.push(Draw::Fill([x - half - 0.5, y - half - 0.5, half * 2.0 + 1.0, half * 2.0 + 1.0], 0x7000_0000));
            draws.push(Draw::Fill([x - half, y - half, half * 2.0, half * 2.0], 0xA000_0000 | color));
            let letter = initial(&w.name);
            let lw = measure(&letter) * 0.9;
            draws.push(Draw::Label(letter, [x - lw / 2.0 + 0.4, y - 3.4], 0xE000_0000 | ink_on(color), 0.9, false));
            let line = format!("{}  {}", w.name, distance_text(d));
            let scale = 0.7;
            let tw = measure(&line) * scale;
            let top = y + half + 2.0;
            draws.push(Draw::Fill([x - tw / 2.0 - 2.0, top - 1.0, tw + 4.0, 8.0 * scale + 2.0], 0x5000_0000));
            draws.push(Draw::Label(line, [x - tw / 2.0, top], 0xE0FF_FFFF, scale, false));
        }
        draws
    }

    /// The lines under the minimap.
    fn info_lines(&self, here: &Here, info: &MiniInfo) -> Vec<String> {
        let st = self.settings();
        let mut lines = Vec::new();
        if st.coords {
            let b = here.pos.map(|v| v.floor() as i64);
            lines.push(format!("{}, {}, {}", b[0], b[1], b[2]));
        }
        if st.biome
            && let Some(biome) = &info.biome
        {
            lines.push(biome_label(biome));
        }
        if st.time
            && let Some(t) = info.day_time
        {
            lines.push(time_text(t));
        }
        if self.cut != Cut::Surface {
            lines.push(self.cut.label());
        }
        lines
    }

    /// The minimap's side on a `gui` sized screen: the share of its height
    /// chosen (never wider than 45% of it, never under 24 pixels).
    pub fn mini_side(&self, gui: [f32; 2]) -> f32 {
        let share = f32::from(self.settings().size) / 100.0;
        (gui[1] * share).min(gui[0] * 0.45).max(24.0).floor()
    }

    /// How far the frame reaches out of the map on each side.
    fn frame_width(&self) -> f32 {
        match self.settings().style {
            Style::Frame => 4.0,
            Style::Thin => 1.0,
            Style::Bare => 0.0,
        }
    }

    /// The minimap's box, `[w, h]` in GUI pixels: the map, its frame and
    /// the lines under it.
    pub fn minimap_box(&self, gui: [f32; 2], here: &Here, info: &MiniInfo, measure: &dyn Fn(&str) -> f32) -> [f32; 2] {
        let side = self.mini_side(gui) + self.frame_width() * 2.0;
        let lines = self.info_lines(here, info);
        let widest = lines.iter().map(|l| measure(l) * INFO_SCALE + 2.0).fold(side, f32::max);
        let below = if lines.is_empty() { 0.0 } else { lines.len() as f32 * LINE * INFO_SCALE + 3.0 };
        [widest, side + below]
    }

    /// The minimap with its box's top left at `at` (as `minimap_box` sized
    /// it), as Xaero's minimap looks: the map in a grey bevelled frame (or a
    /// ring), N E S W on the frame, round dots for `dots`, waypoints as
    /// letters, a red arrow for the player and the lines under it.
    pub fn minimap_hud(
        &self,
        gui: [f32; 2],
        at: [f32; 2],
        here: &Here,
        info: &MiniInfo,
        dots: &[Dot],
        measure: &dyn Fn(&str) -> f32,
    ) -> Vec<Draw> {
        let st = self.settings();
        let mut draws = Vec::new();
        let (Some(grid), Some(slot)) = (self.mini_grid, self.mini_slot) else {
            return draws;
        };
        let [bw, _] = self.minimap_box(gui, here, info, measure);
        let side = self.mini_side(gui);
        let fw = self.frame_width();
        let x = (at[0] + (bw - side) / 2.0).round();
        let y = at[1] + fw;
        let half = side / 2.0;
        let mid = [x + half, y + half];

        // The frame, under the map.
        if st.round {
            match st.style {
                Style::Frame => {
                    ring(&mut draws, mid, half + 2.0, 5.0, 0xFF10_1010);
                    ring(&mut draws, mid, half + 2.0, 3.0, 0xFF9C_9C9C);
                    ring(&mut draws, mid, half + 1.5, 1.0, 0xFFD8_D8D8);
                }
                Style::Thin => ring(&mut draws, mid, half + 0.5, 1.0, 0xFF10_1010),
                Style::Bare => {}
            }
        } else {
            let r = |d: f32| [x - d, y - d, side + d * 2.0, side + d * 2.0];
            match st.style {
                Style::Frame => {
                    draws.push(Draw::Fill(r(4.0), 0xFF10_1010));
                    draws.push(Draw::Fill(r(3.0), 0xFF9C_9C9C));
                    draws.push(Draw::Fill(r(2.0), 0xFFD8_D8D8));
                    draws.push(Draw::Fill(r(1.0), 0xFF56_5656));
                }
                Style::Thin => draws.push(Draw::Fill(r(1.0), 0xFF10_1010)),
                Style::Bare => {}
            }
        }
        draws.push(Draw::Image(
            slot,
            [grid.shift[0], grid.shift[1], MINI_SHOWN, MINI_SHOWN],
            [x, y, side, side],
        ));

        // Where a world point lands, pulled onto the edge when past it.
        let reach = half - 1.0;
        let place = |dx: f64, dz: f64, pull: bool| -> Option<([f32; 2], bool)> {
            let [mut sx, mut sy] = grid.to_screen(dx, dz, side);
            let past = if st.round {
                let r = (sx * sx + sy * sy).sqrt();
                if r > reach {
                    if !pull {
                        return None;
                    }
                    sx *= reach / r;
                    sy *= reach / r;
                }
                r > reach
            } else {
                let past = sx.abs() > reach || sy.abs() > reach;
                if past && !pull {
                    return None;
                }
                sx = sx.clamp(-reach, reach);
                sy = sy.clamp(-reach, reach);
                past
            };
            Some(([mid[0] + sx, mid[1] + sy], past))
        };

        // Dots a little under 1/40 of the map across, as Xaero's.
        let dot = (side / 85.0).clamp(0.8, 2.2);
        if st.dots {
            let mut sorted: Vec<&Dot> = dots.iter().collect();
            sorted.sort_by_key(|d| d.kind.layer());
            for d in sorted {
                let Some((p, _)) = place(d.pos[0] - here.pos[0], d.pos[2] - here.pos[2], false) else {
                    continue;
                };
                let r = if d.kind == DotKind::Player { dot * 1.3 } else { dot };
                // Darker and fainter the further from the player's height.
                let near = height_fade(d.pos[1] - here.pos[1]);
                disc(&mut draws, p, r + 0.6, fade(0xE020_2020, 1.0, near));
                disc(&mut draws, p, r, fade(d.kind.color(), 0.35 + 0.65 * near, near));
            }
        }

        // Waypoints: their letter on their colour, on the edge when far.
        for w in self.waypoints.iter().filter(|w| !w.hidden && w.dimension == here.dimension) {
            let c = centre(w);
            if let Some((p, _)) = place(c[0] - here.pos[0], c[2] - here.pos[2], true) {
                letter_marker(&mut draws, p, w, 4.0, false);
            }
        }

        // The player: a red arrow, as Xaero's.
        let facing = if st.rotate { 0.0 } else { 180.0 + here.yaw };
        arrow(&mut draws, mid, facing, (side / 14.0).clamp(5.0, 9.0), 0xFFE0_3030);

        // N E S W on the frame.
        if st.compass {
            for (letter, dir) in [("N", [0.0, -1.0]), ("E", [1.0, 0.0]), ("S", [0.0, 1.0]), ("W", [-1.0, 0.0])] {
                let [sx, sy] = grid.to_screen(dir[0], dir[1], 1.0);
                let len = (sx * sx + sy * sy).sqrt().max(1e-6);
                let (ux, uy) = (sx / len, sy / len);
                let edge = half + fw / 2.0;
                let t = if st.round { edge } else { edge / ux.abs().max(uy.abs()) };
                let at = [mid[0] + ux * t, mid[1] + uy * t];
                let lw = measure(letter);
                outlined(&mut draws, letter, [(at[0] - lw / 2.0 + 0.5).round(), (at[1] - 3.5).round()], INK);
            }
        }

        // The lines under it.
        let lines = self.info_lines(here, info);
        let top = y + side + fw + 2.0;
        for (i, line) in lines.iter().enumerate() {
            let lw = measure(line) * INFO_SCALE;
            let tx = x + half - lw / 2.0;
            draws.push(Draw::Label(line.clone(), [tx, top + i as f32 * LINE * INFO_SCALE], 0xFFFF_FFFF, INFO_SCALE, true));
        }
        draws
    }

    /// The open screen's drawing and clickable areas, for a `size` GUI.
    pub fn screen(
        &mut self,
        size: [f32; 2],
        mouse: [f32; 2],
        here: Option<&Here>,
        measure: &dyn Fn(&str) -> f32,
    ) -> (Vec<Draw>, Vec<Widget>) {
        self.screen_size = size;
        self.mouse = mouse;
        let mut out = Out {
            draws: Vec::new(),
            widgets: Vec::new(),
            mouse,
            measure,
        };
        match self.mode.clone() {
            Mode::Closed => {}
            Mode::List => {
                out.fill([0.0, 0.0, size[0], size[1]], SHADE);
                self.draw_list(&mut out, size, here);
            }
            Mode::Edit(editor) => {
                if editor.back == Back::Map {
                    self.draw_map(&mut out, size, here, false);
                }
                out.fill([0.0, 0.0, size[0], size[1]], SHADE);
                draw_editor(&mut out, size, &editor);
            }
            Mode::Map => self.draw_map(&mut out, size, here, true),
        }
        (out.draws, out.widgets)
    }

    /// The world map, as Xaero's world map looks: what has been seen of
    /// this dimension on black, edge to edge, the scale top left, the
    /// pointer's block top centre, a compass rose top right, the player as
    /// a red arrow, waypoints as letters with their names, and a few
    /// buttons bottom right.
    fn draw_map(&mut self, out: &mut Out<'_>, size: [f32; 2], here: Option<&Here>, live: bool) {
        if let Some(h) = here
            && self.map.follow
        {
            self.map.centre = [h.pos[0], h.pos[2]];
        }
        out.fill([0.0, 0.0, size[0], size[1]], MAP_VOID);
        for (key, dst) in self.map.tiles(size, self.cut) {
            if let Some(&slot) = self.images.slots.get(&key) {
                let t = MAP_TEXELS as f32;
                out.draws.push(Draw::Image(slot, [0.0, 0.0, t, t], dst));
            }
        }
        let dimension = here.map(|h| h.dimension.as_str()).unwrap_or(OVERWORLD);

        // Waypoints: their letter, and their name over it.
        let mut hot = None;
        for (i, w) in self.waypoints.iter().enumerate() {
            if w.dimension != dimension {
                continue;
            }
            let c = centre(w);
            let at = self.map.screen(c[0], c[2], size);
            if at[0] < -40.0 || at[1] < -20.0 || at[0] > size[0] + 40.0 || at[1] > size[1] + 20.0 {
                continue;
            }
            let rect = [at[0] - 6.0, at[1] - 6.0, 12.0, 12.0];
            let over = live && inside(rect, out.mouse);
            if over {
                hot = Some(i);
            }
            letter_marker(&mut out.draws, at, w, 5.0, over);
            let name_w = out.width(&w.name);
            let top = at[1] - 16.0;
            out.fill([at[0] - name_w / 2.0 - 2.0, top - 1.0, name_w + 4.0, 10.0], LABEL_BG);
            out.text(&w.name, [at[0] - name_w / 2.0, top], if w.hidden { MUTED } else { INK });
            if live {
                out.widgets.push(Widget {
                    rect,
                    action: Action::MapWaypoint(i),
                });
            }
        }

        if let Some(h) = here {
            let at = self.map.screen(h.pos[0], h.pos[2], size);
            arrow(&mut out.draws, at, 180.0 + h.yaw, 10.0, 0xFFE0_3030);
        }

        // Top left: the scale (pixels a block).
        let scale = format!("Scale: {}x", trim_float(1.0 / self.map.bpp));
        out.text(&scale, [4.0, 4.0], INK);

        // Top centre: the block under the pointer.
        if live {
            let [bx, by, bz] = self.map_block(out.mouse, here);
            let at = format!("X: {bx}  Y: {by}  Z: {bz}");
            let w = out.width(&at);
            out.text(&at, [((size[0] - w) / 2.0).round(), 4.0], INK);
        }
        if self.cut != Cut::Surface {
            let cut = self.cut.label();
            let w = out.width(&cut);
            out.text(&cut, [((size[0] - w) / 2.0).round(), 15.0], MUTED);
        }

        // Top right: the compass rose.
        compass_rose(out, [size[0] - 24.0, 22.0], 14.0);

        // Bottom right: the buttons; bottom left: how to use it.
        let st = self.settings();
        let mut buttons: Vec<(String, Action)> = vec![
            ("Waypoints".into(), Action::OpenList),
            ("Centre".into(), Action::Centre),
        ];
        if st.mode == MapMode::Layer {
            buttons.push(("+".into(), Action::LayerStep(1)));
            buttons.push((format!("Y {}", st.layer_y), Action::LayerStep(0)));
            buttons.push(("-".into(), Action::LayerStep(-1)));
        }
        buttons.push((format!("Mode: {}", st.mode.label()), Action::CycleMapMode));
        let mut x = size[0] - 4.0;
        let by = size[1] - 18.0;
        for (label, action) in buttons {
            let w = out.width(&label) + 10.0;
            x -= w;
            if live {
                let on = action == Action::Centre && self.map.follow;
                out.button([x, by, w, 14.0], &label, action, on);
            }
            x -= 3.0;
        }
        let hint = match hot {
            Some(i) => format!("Click to edit {}", self.waypoints[i].name),
            None => "Drag to move, wheel to zoom, B for a waypoint here, Space to centre".into(),
        };
        let hint = out.fit(&hint, x - 8.0);
        out.text(&hint, [4.0, size[1] - 14.0], MUTED);
    }

    /// The waypoint list: every waypoint (or a dimension's), one a row;
    /// a click picks one, and the buttons under the list act on it.
    fn draw_list(&mut self, out: &mut Out<'_>, size: [f32; 2], here: Option<&Here>) {
        let pw = (size[0] - 16.0).min(360.0);
        let ph = (size[1] - 16.0).max(140.0);
        let (px, py) = (((size[0] - pw) / 2.0).round(), 8.0);
        out.boxed([px, py, pw, ph], PANEL_EDGE, PANEL);
        let right = px + pw - 6.0;

        let title = format!("Waypoints ({})", self.waypoints.len());
        out.text(&title, [px + 6.0, py + 6.0], INK);
        let mut x = right;
        for (label, action) in [("Close", Action::Close), ("+ New", Action::NewWaypoint), ("Map", Action::OpenMap)] {
            let w = out.width(label) + 8.0;
            x -= w;
            out.button([x, py + 4.0, w, 12.0], label, action, false);
            x -= 4.0;
        }

        let mut fx = px + 6.0;
        for filter in Filter::ALL {
            let on = self.filter == filter;
            fx += out.button_auto(fx, py + 20.0, filter.label(), Action::Filter(filter), on) + 3.0;
        }

        let rows: Vec<usize> = self
            .waypoints
            .iter()
            .enumerate()
            .filter(|(_, w)| self.filter.matches(&w.dimension))
            .map(|(i, _)| i)
            .collect();
        if self.selected.is_some_and(|i| !rows.contains(&i)) {
            self.selected = None;
        }
        let bar = 24.0;
        let top = py + 38.0;
        let bottom = py + ph - bar - 14.0;
        let fits = (((bottom - top) / ROW).floor() as usize).max(1);
        self.scroll = self.scroll.min(rows.len().saturating_sub(fits));

        if rows.is_empty() {
            let msg = if self.waypoints.is_empty() {
                "No waypoints yet. Press B in game, or on the world map."
            } else {
                "No waypoints in this dimension."
            };
            out.text(msg, [px + 8.0, top + 6.0], MUTED);
        }
        for (slot, &i) in rows.iter().skip(self.scroll).take(fits).enumerate() {
            let w = &self.waypoints[i];
            let ry = top + slot as f32 * ROW;
            let rect = [px + 4.0, ry, pw - 8.0, ROW - 2.0];
            let picked = self.selected == Some(i);
            let hover = inside(rect, out.mouse);
            if picked {
                out.boxed(rect, FIELD_FOCUS, 0x6040_4060);
            } else {
                out.fill(rect, if hover { 0x5040_4050 } else { ROW_BG });
            }

            // Letter on its colour, name and coordinates.
            let mut marks = Vec::new();
            letter_marker(&mut marks, [px + 14.0, ry + 11.0], w, 5.0, false);
            out.draws.extend(marks);
            let dim = dimension_label(&w.dimension);
            let dist = match here {
                Some(h) if h.dimension == w.dimension => distance_text(distance(h.pos, centre(w))),
                _ => "other dimension".into(),
            };
            let (dim_w, dist_w) = (out.width(&dim), out.width(&dist));
            let info_right = right - 4.0;
            out.text(&dim, [info_right - dim_w, ry + 3.0], dimension_color(&w.dimension));
            out.text(&dist, [info_right - dist_w, ry + 3.0 + LINE], MUTED);
            let text_x = px + 24.0;
            let room = info_right - dim_w.max(dist_w) - 6.0 - text_x;
            let name = out.fit(&w.name, room);
            out.text(&name, [text_x, ry + 3.0], if w.hidden { MUTED } else { INK });
            let coords = format!("{} {} {}{}", w.pos[0], w.pos[1], w.pos[2], if w.hidden { "  (hidden)" } else { "" });
            let coords = out.fit(&coords, room);
            out.text(&coords, [text_x, ry + 3.0 + LINE], MUTED);
            out.widgets.push(Widget {
                rect,
                action: Action::Select(i),
            });
        }

        let more = rows.len().saturating_sub(self.scroll + fits);
        let hint = if self.scroll > 0 || more > 0 {
            format!("Scroll for more ({} above, {} below)", self.scroll, more)
        } else if self.selected.is_none() && !rows.is_empty() {
            "Click a waypoint for its buttons".to_owned()
        } else {
            "B new waypoint  -  M map  -  U or Esc close".to_owned()
        };
        out.text(&hint, [px + 6.0, bottom + 2.0], MUTED);

        // The button row, as Minecraft's screens lay theirs out.
        let by = py + ph - bar - 2.0;
        let mut buttons: Vec<(String, Option<Action>)> = Vec::new();
        let picked = self.selected.and_then(|i| self.waypoints.get(i).map(|w| (i, w.hidden)));
        let act = |a: Action| picked.map(|_| a);
        let i = picked.map_or(0, |p| p.0);
        if self.can_teleport {
            buttons.push(("Teleport".into(), act(Action::Teleport(i))));
        }
        buttons.push(("Edit".into(), act(Action::Edit(i))));
        let vis = if picked.is_some_and(|p| p.1) { "Show" } else { "Hide" };
        buttons.push((vis.into(), act(Action::Toggle(i))));
        let del = if picked.is_some() && self.confirm_delete == picked.map(|p| p.0) { "Sure?" } else { "Delete" };
        buttons.push((del.into(), act(Action::Delete(i))));
        let n = buttons.len() as f32;
        let gap = 4.0;
        let bw = ((pw - 12.0 - gap * (n - 1.0)) / n).floor();
        for (k, (label, action)) in buttons.into_iter().enumerate() {
            let rect = [px + 6.0 + k as f32 * (bw + gap), by, bw, 20.0];
            mc_button(out, rect, &label, action);
        }
    }
}

/// A button as Minecraft draws its own: a grey bevelled face, white text,
/// lighter under the pointer, darker and inert when `action` is None.
fn mc_button(out: &mut Out<'_>, rect: [f32; 4], label: &str, action: Option<Action>) {
    let [x, y, w, h] = rect;
    let active = action.is_some();
    let hover = active && inside(rect, out.mouse);
    let (face, light, dark, ink) = match (active, hover) {
        (false, _) => (0xFF2C_2C2C, 0xFF3A_3A3A, 0xFF1E_1E1E, 0xA0A0A0),
        (true, false) => (0xFF6F_6F6F, 0xFFA8_A8A8, 0xFF4A_4A4A, INK),
        (true, true) => (0xFF7E_88B8, 0xFFBE_C8F0, 0xFF56_5E8A, 0xFFFFA0),
    };
    out.fill([x, y, w, h], 0xFF00_0000);
    out.fill([x + 1.0, y + 1.0, w - 2.0, h - 2.0], dark);
    out.fill([x + 1.0, y + 1.0, w - 3.0, h - 3.0], light);
    out.fill([x + 2.0, y + 2.0, w - 4.0, h - 5.0], face);
    let tw = out.width(label);
    out.text(label, [(x + (w - tw) / 2.0).round(), (y + (h - 8.0) / 2.0).round()], ink);
    if let Some(action) = action {
        out.widgets.push(Widget { rect, action });
    }
}

fn draw_editor(out: &mut Out<'_>, size: [f32; 2], e: &Editor) {
    let pw = (size[0] - 16.0).min(250.0);
    let ph = 178.0_f32.min(size[1] - 8.0);
    let px = ((size[0] - pw) / 2.0).round();
    let py = ((size[1] - ph) / 2.0).round().max(4.0);
    out.boxed([px, py, pw, ph], PANEL_EDGE, PANEL);
    let (left, inner) = (px + 6.0, pw - 12.0);

    let title = if e.index.is_some() { "Edit waypoint" } else { "New waypoint" };
    out.text(title, [left, py + 6.0], INK);
    if let Some(i) = e.index {
        let del_w = out.width("Delete") + 8.0;
        out.button([left + inner - del_w, py + 4.0, del_w, 12.0], "Delete", Action::Delete(i), false);
    }

    out.text("Name", [left, py + 20.0], MUTED);
    text_field(out, [left, py + 30.0, inner, 14.0], &e.name, e.focus == Field::Name, Field::Name);

    let third = (inner - 8.0) / 3.0;
    for (i, (field, label)) in [(Field::X, "X"), (Field::Y, "Y"), (Field::Z, "Z")]
        .into_iter()
        .enumerate()
    {
        let x = left + i as f32 * (third + 4.0);
        out.text(label, [x, py + 48.0], MUTED);
        text_field(out, [x, py + 58.0, third, 14.0], &e.coords[i], e.focus == field, field);
    }

    let here_w = out.width("Use my position") + 8.0;
    out.button([left, py + 76.0, here_w, 12.0], "Use my position", Action::Here, false);
    let dim = format!("Dimension: {}", dimension_label(&e.dimension));
    let dim_w = out.width(&dim) + 8.0;
    out.button(
        [left + inner - dim_w, py + 76.0, dim_w, 12.0],
        &dim,
        Action::CycleDimension,
        false,
    );

    out.text("Colour", [left, py + 94.0], MUTED);
    let swatch = ((inner + 2.0) / PALETTE.len() as f32 - 2.0).min(12.0).floor();
    for (i, &color) in PALETTE.iter().enumerate() {
        let x = left + i as f32 * (swatch + 2.0);
        let rect = [x, py + 104.0, swatch, swatch];
        let edge = if color == e.color { FIELD_FOCUS } else { 0xFF00_0000 };
        out.boxed(rect, edge, 0xFF00_0000 | color);
        if color == e.color {
            out.fill([x + 2.0, py + 106.0, swatch - 4.0, swatch - 4.0], 0xFF00_0000 | color);
        }
        out.widgets.push(Widget {
            rect,
            action: Action::Color(color),
        });
    }

    let vis = if e.hidden { "Visible: No" } else { "Visible: Yes" };
    out.button_auto(left, py + 122.0, vis, Action::ToggleHidden, !e.hidden);

    if let Some(error) = &e.error {
        out.text(error, [left, py + 138.0], ERROR);
    } else {
        out.text("Enter save  -  Tab next field  -  Esc cancel", [left, py + 138.0], MUTED);
    }

    let bw = (inner - 4.0) / 2.0;
    out.button([left, py + ph - 20.0, bw, 14.0], "Save", Action::Save, false);
    out.button([left + bw + 4.0, py + ph - 20.0, bw, 14.0], "Cancel", Action::Cancel, false);
}

fn text_field(out: &mut Out<'_>, rect: [f32; 4], text: &str, focused: bool, field: Field) {
    out.boxed(rect, if focused { FIELD_FOCUS } else { FIELD_EDGE }, FIELD);
    let shown = if focused { format!("{text}_") } else { text.to_owned() };
    // Long text shows its end, where the typing is.
    let max = rect[2] - 6.0;
    let mut tail = shown.as_str();
    while out.width(tail) > max && !tail.is_empty() {
        let mut chars = tail.chars();
        chars.next();
        tail = chars.as_str();
    }
    out.text(tail, [rect[0] + 3.0, rect[1] + 3.0], INK);
    out.widgets.push(Widget {
        rect,
        action: Action::Focus(field),
    });
}

/// The widget under `at`, the last drawn winning.
pub fn hit(widgets: &[Widget], at: [f32; 2]) -> Option<Action> {
    widgets
        .iter()
        .rev()
        .find(|w| inside(w.rect, at))
        .map(|w| w.action.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEYS: [&str; 3] = [DEFAULT_MAP, DEFAULT_CREATE, DEFAULT_LIST];

    fn here(dim: &str) -> Here {
        Here {
            pos: [10.7, 64.0, -3.2],
            dimension: dim.into(),
            yaw: 0.0,
        }
    }

    fn measure(s: &str) -> f32 {
        s.chars().count() as f32 * 6.0
    }

    fn grass(height: i16) -> Tile {
        let mut t = Tile::blank(-64);
        t.color = [LAND | 0x5D_A040; 256];
        t.height = [height; 256];
        t
    }

    #[test]
    fn storage_round_trips_odd_names() {
        let list = vec![
            Waypoint {
                name: "Base\twith\\tab".into(),
                pos: [-12, 70, 300],
                dimension: NETHER.into(),
                color: 0xFF5555,
                hidden: true,
            },
            Waypoint {
                name: "Ünïcode ⛏".into(),
                pos: [0, -64, 0],
                dimension: OVERWORLD.into(),
                color: 0x00AA00,
                hidden: false,
            },
        ];
        assert_eq!(decode(&encode(&list)), list);
        assert!(decode("garbage\n1\t2").is_empty());
    }

    #[test]
    fn world_files_are_plain_names() {
        assert_eq!(world_file("server-mc.hypixel.net:25565"), "server-mc.hypixel.net_25565.txt");
        assert_eq!(world_file("world-New World"), "world-New_World.txt");
        let d = map_dir(std::path::Path::new("/d"), "world-A B", NETHER);
        assert_eq!(d, PathBuf::from("/d/maps/world-A_B/minecraft_the_nether"));
    }

    #[test]
    fn words() {
        assert_eq!(dimension_label(NETHER), "Nether");
        assert_eq!(dimension_label("mymod:deep_dark"), "deep dark");
        assert_eq!(distance_text(4.4), "4 m");
        assert_eq!(distance_text(1530.0), "1.5 km");
        assert_eq!(key_char("key.keyboard.n"), Some('n'));
        assert_eq!(key_char("key.keyboard.f1"), None);
        assert_eq!(trim_float(1.0), "1");
        assert_eq!(trim_float(0.25), "0.3");
        assert_eq!(trim_float(4.0), "4");
    }

    #[test]
    fn projection_puts_what_is_ahead_in_the_middle() {
        let size = [400.0, 200.0];
        let view = basis(0.0, 0.0);
        // Facing south: a point straight ahead is in the centre...
        let c = project([0.0; 3], view, 70.0, [0.0, 0.0, 10.0], size).unwrap();
        assert!((c[0] - 200.0).abs() < 0.01 && (c[1] - 100.0).abs() < 0.01);
        // ...east (+X) is on the left, up is up, behind is nothing.
        let e = project([0.0; 3], view, 70.0, [3.0, 2.0, 10.0], size).unwrap();
        assert!(e[0] < 200.0 && e[1] < 100.0);
        assert!(project([0.0; 3], view, 70.0, [0.0, 0.0, -5.0], size).is_none());
        // Looking straight down still has a right vector.
        let down = basis(0.0, 90.0);
        let below = project([0.0; 3], down, 70.0, [0.0, -10.0, 0.0], size).unwrap();
        assert!((below[0] - 200.0).abs() < 0.01 && (below[1] - 100.0).abs() < 0.01);
        // The edge of the vertical field of view is the screen's edge.
        let t = (35f64).to_radians().tan();
        let top = project([0.0; 3], view, 70.0, [0.0, 10.0 * t, 10.0], size).unwrap();
        assert!(top[1].abs() < 0.01);
    }

    #[test]
    fn create_list_edit_hide_delete() {
        let mut s = State::default();
        s.open_create(&here(OVERWORLD), Back::Game, Some('b'));
        // The opening key's character is not typed.
        s.char_typed('b');
        s.char_typed('!');
        // Typing replaces the suggested name.
        let Mode::Edit(e) = &s.mode else { panic!() };
        assert_eq!(e.name, "!");
        assert_eq!(e.coords, ["10".to_string(), "64".into(), "-4".into()]);
        s.key("key.keyboard.backspace", false, None, KEYS);
        for c in "Home".chars() {
            s.char_typed(c);
        }
        s.key("key.keyboard.tab", false, None, KEYS);
        s.char_typed('x'); // not a number: ignored
        s.act(Action::Color(0x5555FF), None);
        s.key("key.keyboard.enter", false, None, KEYS);
        assert_eq!(s.waypoints.len(), 1);
        assert_eq!(s.waypoints[0].name, "Home");
        assert_eq!(s.waypoints[0].color, 0x5555FF);
        assert!(s.take_close());

        s.open_list(None);
        s.act(Action::Toggle(0), None);
        assert!(s.waypoints[0].hidden);
        s.act(Action::Edit(0), None);
        s.act(Action::CycleDimension, None);
        s.act(Action::Save, None);
        assert_eq!(s.waypoints[0].dimension, NETHER);
        assert_eq!(s.mode, Mode::List);

        s.act(Action::Delete(0), None);
        assert_eq!(s.waypoints.len(), 1, "asks first");
        s.act(Action::Delete(0), None);
        assert!(s.waypoints.is_empty());
    }

    #[test]
    fn bad_coordinates_are_refused() {
        let mut s = State::default();
        s.open_create(&here(OVERWORLD), Back::List, None);
        s.act(Action::Focus(Field::Y), None);
        s.key("key.keyboard.delete", false, None, KEYS);
        s.act(Action::Save, None);
        let Mode::Edit(e) = &s.mode else { panic!() };
        assert!(e.error.is_some());
        assert!(s.waypoints.is_empty());
    }

    #[test]
    fn screens_draw_and_clicks_find_their_buttons() {
        let mut s = State::default();
        s.waypoints.push(Waypoint {
            name: "Far away base with a very long name indeed".into(),
            pos: [100, 64, 100],
            dimension: OVERWORLD.into(),
            color: 0xFFAA00,
            hidden: false,
        });
        s.open_list(None);
        let (draws, widgets) = s.screen([480.0, 270.0], [0.0, 0.0], Some(&here(OVERWORLD)), &measure);
        assert!(draws.iter().any(|d| matches!(d, Draw::Text(t, _, _, _) if t == "Overworld")));
        // Nothing picked: the button row is inert.
        assert!(!widgets.iter().any(|w| w.action == Action::Edit(0)));
        let row = widgets.iter().find(|w| w.action == Action::Select(0)).unwrap();
        let at = [row.rect[0] + 1.0, row.rect[1] + 1.0];
        s.press(0, at, &widgets, None);
        assert_eq!(s.selected, Some(0));
        let (_, widgets) = s.screen([480.0, 270.0], [0.0, 0.0], Some(&here(OVERWORLD)), &measure);
        let edit = widgets.iter().find(|w| w.action == Action::Edit(0)).unwrap();
        let at = [edit.rect[0] + 1.0, edit.rect[1] + 1.0];
        assert_eq!(hit(&widgets, at), Some(Action::Edit(0)));
        assert!(widgets.iter().any(|w| w.action == Action::OpenMap));
        // Teleport only for operators, and it closes the screen.
        assert!(!widgets.iter().any(|w| w.action == Action::Teleport(0)));
        s.can_teleport = true;
        let (_, widgets) = s.screen([480.0, 270.0], [0.0, 0.0], Some(&here(OVERWORLD)), &measure);
        assert!(widgets.iter().any(|w| w.action == Action::Teleport(0)));
        s.act(Action::Teleport(0), None);
        assert_eq!(
            s.take_teleport().as_deref(),
            Some("/execute in minecraft:overworld run tp @s 100.5 64 100.5")
        );
        assert!(s.take_close());
        s.open_list(None);

        s.act(Action::Edit(0), None);
        let (_, widgets) = s.screen([480.0, 270.0], [0.0, 0.0], None, &measure);
        assert!(widgets.iter().any(|w| w.action == Action::Color(0xFF55FF)));
        assert!(widgets.iter().any(|w| w.action == Action::Save));
    }

    #[test]
    fn labels_show_only_visible_waypoints_of_this_dimension() {
        let mut s = State::default();
        let camera = Camera {
            pos: [0.5, 65.62, 0.5],
            yaw: 0.0,
            pitch: 0.0,
            fov: 70.0,
        };
        let mk = |name: &str, dim: &str, hidden| Waypoint {
            name: name.into(),
            pos: [0, 65, 20],
            dimension: dim.into(),
            color: 0x55FFFF,
            hidden,
        };
        s.waypoints = vec![mk("A", OVERWORLD, false), mk("B", OVERWORLD, true), mk("C", NETHER, false)];
        let draws = s.labels(camera, [0.5, 64.0, 0.5], OVERWORLD, [480.0, 270.0], &measure);
        let texts: Vec<&str> = draws
            .iter()
            .filter_map(|d| match d {
                Draw::Text(t, ..) | Draw::Label(t, ..) => Some(t.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(texts, ["A", "A  20 m"]);
        s.settings = Some(Settings {
            labels: false,
            ..Settings::default()
        });
        assert!(s.labels(camera, [0.5, 64.0, 0.5], OVERWORLD, [480.0, 270.0], &measure).is_empty());
    }

    #[test]
    fn packbits_round_trips() {
        let mut data = vec![7u8; 300];
        data.extend(0..=255u8);
        data.extend([1, 1, 2, 2, 2, 3]);
        let packed = pack(&data);
        assert!(packed.len() < data.len());
        assert_eq!(unpack(&packed, data.len()).unwrap(), data);
        assert_eq!(unpack(&pack(&[]), 0).unwrap(), Vec::<u8>::new());
        assert!(unpack(&[5, 1, 2], 6).is_none(), "cut short");
    }

    #[test]
    fn regions_round_trip_and_the_store_keeps_what_it_saw() {
        let dir = std::env::temp_dir().join(format!("atlas-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut store = Store::default();
        store.open(Some(dir.clone()));
        let mut t = grass(70);
        t.color[5] = WATER | 0x3F_76E4;
        t.depth[5] = 4;
        t.height[17] = -12;
        assert!(store.put(-1, 33, t.clone()));
        assert!(!store.put(-1, 33, t.clone()), "the same tile again is no change");
        assert!(store.has(-1, 33));
        assert!(!store.has(0, 33));
        // Chunk (-1, 33) is blocks x -16..-1, z 528..543.
        assert_eq!(store.column(-16 + 5, 528), Some((WATER | 0x3F_76E4, 70, 4)));
        assert_eq!(store.column(-16 + 1, 529), Some((LAND | 0x5D_A040, -12, 0)));
        assert_eq!(store.column(5, 528), None);
        let before = store.stamp(-16, 528, -1, 543);
        store.put(-1, 33, grass(71));
        assert_ne!(store.stamp(-16, 528, -1, 543), before);
        store.save();
        // Read back by a new store.
        let mut again = Store::default();
        again.open(Some(dir.clone()));
        assert_eq!(again.column(-16 + 3, 530), Some((LAND | 0x5D_A040, 71, 0)));
        assert!(!again.has(-2, 33));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_minimap_grid_is_fixed_to_the_world_and_shifts_smoothly() {
        // North up, 1 block a texel: moving a tenth of a block shifts the
        // shown area a tenth of a texel and keeps the grid.
        let a = mini_grid(100.0, 50.0, 0.0, false, 1.0);
        let b = mini_grid(100.1, 50.0, 0.0, false, 1.0);
        assert_eq!(a.origin, b.origin);
        assert!((b.shift[0] - a.shift[0] - 0.1).abs() < 1e-4);
        // The player is in the middle of what shows.
        let mid = |g: &MiniGrid| {
            let i = (g.shift[0] + MINI_SHOWN / 2.0).floor() as usize;
            let j = (g.shift[1] + MINI_SHOWN / 2.0).floor() as usize;
            g.block(i, j)
        };
        assert_eq!(mid(&a), (100, 50));
        // Crossing into the next block moves the grid by one.
        let c = mini_grid(101.0, 50.0, 0.0, false, 1.0);
        assert_eq!(c.origin[0], a.origin[0] + 1);
        assert_eq!(mid(&c), (101, 50));
        // Turning: where the player looks is up.
        let (right, down) = mini_axes(true, 0.0);
        // Facing south (+z): up on the map is +z, so down is -z; right is west (-x).
        assert_eq!(down.map(|v| v.round()), [0.0, -1.0]);
        assert_eq!(right.map(|v| v.round()), [-1.0, 0.0]);
        let g = mini_grid(0.0, 0.0, 0.0, true, 1.0);
        let ahead = g.to_screen(0.0, 10.0, MINI_SHOWN);
        assert!(ahead[1] < -9.0 && ahead[0].abs() < 1e-3);
    }

    #[test]
    fn the_minimap_image_shades_like_a_map_and_rounds() {
        let mut live = Store::default();
        live.put(0, 0, grass(64));
        let grid = mini_grid(8.0, 8.0, 0.0, false, 1.0);
        let mut img = Vec::new();
        let mut column = |x, z| live.column(x, z);
        compose_minimap(&grid, true, &mut column, &mut img);
        assert_eq!(img.len(), MINI * MINI * 4);
        // The corner is outside the circle; the middle is the grass.
        assert_eq!(&img[0..4], &[0, 0, 0, 0]);
        let mid = (64 * MINI + 64) * 4;
        assert_eq!(img[mid + 3], 255);
        assert_ne!(&img[mid..mid + 4], &UNKNOWN);
        // Far off nothing was seen.
        let mut column = |x, z| live.column(x, z);
        let far = mini_grid(5000.0, 5000.0, 0.0, false, 1.0);
        compose_minimap(&far, false, &mut column, &mut img);
        assert_eq!(&img[mid..mid + 4], &UNKNOWN);
    }

    #[test]
    fn the_minimap_draws_its_image_dots_waypoints_compass_and_lines() {
        let mut s = State::default();
        s.settings = Some(Settings {
            rotate: false,
            ..Settings::default()
        });
        s.waypoints.push(Waypoint {
            name: "far".into(),
            pos: [1000, 64, 0],
            dimension: OVERWORLD.into(),
            color: 0xFF0000,
            hidden: false,
        });
        let h = Here {
            pos: [0.5, 64.0, 0.5],
            dimension: OVERWORLD.into(),
            yaw: 0.0,
        };
        let info = MiniInfo {
            biome: Some("minecraft:dark_forest".into()),
            day_time: Some(6000),
        };
        // Nothing before the image was uploaded.
        let gui = [480.0, 270.0];
        assert!(s.minimap_hud(gui, [0.0, 0.0], &h, &info, &[], &measure).is_empty());
        assert!(s.update_minimap(0.5, 0.5, 0.0));
        assert!(!s.update_minimap(0.6, 0.5, 0.0));
        s.mini_slot = Some(3);
        let dots = [
            Dot { pos: [5.0, 64.0, 0.0], kind: DotKind::Mob },
            Dot { pos: [900.0, 64.0, 0.0], kind: DotKind::Item },
        ];
        let draws = s.minimap_hud(gui, [0.0, 0.0], &h, &info, &dots, &measure);
        let mut texts: Vec<&str> = draws
            .iter()
            .filter_map(|d| match d {
                Draw::Text(t, ..) | Draw::Label(t, ..) => Some(t.as_str()),
                _ => None,
            })
            .collect();
        // Each compass letter is drawn five times: four for its outline.
        texts.dedup();
        assert_eq!(texts, ["F", "N", "E", "S", "W", "0, 64, 0", "Dark Forest", "Day 1, 12:00"]);
        assert!(matches!(draws.iter().find(|d| matches!(d, Draw::Image(..))), Some(Draw::Image(3, ..))));
        // The near mob's dot shows; the far item's doesn't.
        // A dot is three quads.
        let yellow = draws.iter().filter(|d| matches!(d, Draw::Quad(_, c) if *c == DotKind::Mob.color())).count();
        let red = draws.iter().filter(|d| matches!(d, Draw::Quad(_, c) if *c == DotKind::Item.color())).count();
        assert_eq!((yellow, red), (3, 0));
        // The far waypoint's letter sits on the right edge.
        let at = draws
            .iter()
            .find_map(|d| match d {
                Draw::Text(t, at, ..) if t == "F" => Some(*at),
                _ => None,
            })
            .unwrap();
        let [bw, _] = s.minimap_box(gui, &h, &info, &measure);
        assert!(at[0] > bw / 2.0 + s.mini_side(gui) / 2.0 - 8.0, "{at:?}");
        // A quarter of the window's height, whatever its size.
        assert_eq!(s.mini_side(gui), 67.0);
        assert_eq!(s.mini_side([200.0, 120.0]), 30.0);
    }

    #[test]
    fn modes_choose_the_cut_and_cuts_keep_their_own_maps() {
        assert_eq!(cut_for(MapMode::Auto, 64, 70, false, false), Cut::Surface);
        assert_eq!(cut_for(MapMode::Auto, 64, 20, true, false), Cut::Below(31));
        assert_eq!(cut_for(MapMode::Auto, 64, 100, false, true), Cut::Below(111));
        assert_eq!(cut_for(MapMode::Cave, 64, -3, false, false), Cut::Below(-1));
        assert_eq!(cut_for(MapMode::Layer, 40, 100, false, false), Cut::Below(40));
        let mut maps = Maps::default();
        maps.store(Cut::Surface).put(0, 0, grass(64));
        assert!(maps.store(Cut::Surface).column(3, 3).is_some());
        assert!(maps.store(Cut::Below(31)).column(3, 3).is_none());
        // More cuts than are kept open: the surface stays.
        for y in 0..6 {
            maps.store(Cut::Below(y * 16 + 15));
        }
        assert!(maps.store(Cut::Surface).column(3, 3).is_some());
    }

    #[test]
    fn seeds_are_typed_per_world() {
        assert_eq!(parse_seed(" 42 "), Some(42));
        assert_eq!(parse_seed("-7"), Some(-7));
        // Java: "hello".hashCode() == 99162322
        assert_eq!(parse_seed("hello"), Some(99_162_322));
        assert_eq!(parse_seed(""), None);
        let dir = std::env::temp_dir().join(format!("atlas-seed-{}", std::process::id()));
        let mut s = State::default();
        assert_eq!(s.command("seed 5"), "Atlas: join a world first");
        s.load_world(Some("world-a".into()), dir.clone());
        assert!(s.command("seed 12345").starts_with("Atlas: seed 12345"));
        s.load_world(Some("world-b".into()), dir.clone());
        assert_eq!(s.seed, None);
        s.load_world(Some("world-a".into()), dir.clone());
        assert_eq!(s.seed, Some(12345));
        assert_eq!(s.command("slime on"), "Atlas: slime on");
        assert!(s.settings().slime && s.take_changed_settings().is_some());
        assert_eq!(s.command("seed off"), "Atlas: seed forgotten");
        assert_eq!(s.seed, None);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn dots_fade_with_height() {
        assert_eq!(height_fade(0.0), 1.0);
        assert_eq!(height_fade(-8.0), 1.0, "near heights don't fade");
        assert!(height_fade(24.0) > 0.4 && height_fade(24.0) < 0.6);
        assert_eq!(height_fade(45.0), 0.0);
        assert_eq!(fade(0xFFFF_DD33, 1.0, 1.0), 0xFFFF_DD33);
        let far = fade(0xFFFF_DD33, 0.35, 0.0);
        assert_eq!(far >> 24, 77);
        assert!((far >> 16 & 255) < 100);
    }

    #[test]
    fn dots_show_on_the_level_the_map_shows() {
        let mut s = State::default();
        s.maps.store(Cut::Surface).put(0, 0, grass(64));
        let at = |y: f64| Dot { pos: [5.0, y, 5.0], kind: DotKind::Mob };
        let kept = |s: &mut State, y: f64| !s.visible_dots(vec![at(y)], 65.0).is_empty();
        assert!(kept(&mut s, 65.0));
        assert!(!kept(&mut s, 20.0), "a cave mob under the grass");
        s.set_cut(Cut::Below(31));
        assert!(kept(&mut s, 20.0));
        assert!(!kept(&mut s, 65.0));
    }

    #[test]
    fn slime_chunks_follow_the_seed() {
        // About one chunk in ten.
        let n = (0..100).flat_map(|x| (0..100).map(move |z| (x, z))).filter(|&(x, z)| is_slime_chunk(12345, x, z)).count();
        assert!((800..1200).contains(&n), "{n}");
        assert_ne!(
            (0..64).map(|x| is_slime_chunk(1, x, 0)).collect::<Vec<_>>(),
            (0..64).map(|x| is_slime_chunk(2, x, 0)).collect::<Vec<_>>()
        );
    }

    #[test]
    fn settings_round_trip_and_commands_set_them() {
        let none = |_: &str| None;
        let s = Settings::read(none);
        assert_eq!(s, Settings::default());
        assert_eq!(s.view(), (25.0, 1.0));
        let mut s = s;
        let labels = s.labels();
        let find = |start: &str| labels.iter().position(|l| l.starts_with(start)).unwrap();
        // Sections first, and clicking one changes nothing.
        assert!(labels[0].contains("Minimap"));
        let before = s;
        s.click(0);
        assert_eq!(s, before);
        s.click(find("Minimap:"));
        s.click(find("Shape:"));
        s.click(find("Zoom:"));
        assert_eq!(s.command("size 30"), ("Atlas: size 30".to_string(), true));
        assert!(!s.command("size 120").1);
        assert!(s.command("mode layer -20").1);
        assert!(!s.command("mode layer high").1);
        assert_eq!(s.mode, MapMode::Layer);
        assert_eq!(s.layer_y, -20);
        let stored = s.stored();
        let back = Settings::read(|k: &str| stored.iter().find(|f| f.0 == k).map(|f| f.1.clone()));
        assert_eq!(back, s);
        assert!(!back.minimap && back.round && back.size == 30);
        assert_eq!(back.labels()[find("Zoom:")], "Zoom: 2 blocks per pixel");
        let mut back = back;
        assert_eq!(back.command("zoom").0, "Atlas: zoom is 2");
    }

    #[test]
    fn the_world_map_pans_zooms_and_makes_waypoints_where_you_click() {
        let mut s = State::default();
        let h = here(OVERWORLD);
        s.open_map(Some(&h), Some('m'));
        assert_eq!(s.map.centre, [10.7, -3.2]);
        let size = [400.0, 200.0];
        s.screen_size = size;
        // Zooming keeps the block under the pointer.
        let at = [300.0, 50.0];
        let before = s.map.world(at, size);
        s.scroll_by(3.0, at);
        let after = s.map.world(at, size);
        assert!((before[0] - after[0]).abs() < 1e-9 && (before[1] - after[1]).abs() < 1e-9);
        assert!(s.map.bpp < 1.0);
        // Dragging moves it and stops following.
        s.press(0, [200.0, 100.0], &[], Some(&h));
        assert!(s.drag(0, [10.0, 0.0]));
        assert!(!s.map.follow);
        s.release(0);
        assert!(!s.drag(0, [10.0, 0.0]));
        // A right click opens the form there, and Save goes back to the map.
        s.map = MapView {
            centre: [0.0, 0.0],
            bpp: 1.0,
            follow: false,
            ..MapView::default()
        };
        // B over the map opens the form there, as a right click does.
        s.screen(size, [210.0, 80.0], Some(&h), &measure);
        s.key(DEFAULT_CREATE, false, Some(&h), KEYS);
        let Mode::Edit(e) = &s.mode else { panic!() };
        assert_eq!(e.coords, ["10".to_string(), "64".into(), "-20".into()]);
        s.act(Action::Cancel, None);
        s.press(1, [210.0, 80.0], &[], Some(&h));
        let Mode::Edit(e) = &s.mode else { panic!() };
        assert_eq!(e.coords, ["10".to_string(), "64".into(), "-20".into()]);
        assert_eq!(e.back, Back::Map);
        s.act(Action::Save, None);
        assert_eq!(s.mode, Mode::Map);
        assert_eq!(s.waypoints[0].pos, [10, 64, -20]);
        // Its marker is drawn on the map and clicking it edits it.
        let (_, widgets) = s.screen(size, [210.0, 80.0], Some(&h), &measure);
        assert_eq!(hit(&widgets, [210.5, 80.5]), Some(Action::MapWaypoint(0)));
        s.press(0, [210.5, 80.5], &widgets, Some(&h));
        assert!(matches!(&s.mode, Mode::Edit(e) if e.index == Some(0) && e.back == Back::Map));
        // Esc in the form goes back to the map; on the map it closes.
        s.key("key.keyboard.escape", false, None, KEYS);
        assert_eq!(s.mode, Mode::Map);
        s.key("key.keyboard.escape", false, None, KEYS);
        assert!(s.take_close());
    }

    #[test]
    fn world_map_tiles_cover_the_screen_at_a_pixel_or_more() {
        assert_eq!(map_level(0.25), 0);
        assert_eq!(map_level(1.0), 0);
        assert_eq!(map_level(1.5), 1);
        assert_eq!(map_level(8.0), 3);
        let view = MapView {
            centre: [0.0, 0.0],
            bpp: 2.0,
            ..MapView::default()
        };
        let tiles = view.tiles([400.0, 200.0], Cut::Surface);
        // 800 x 400 blocks at 256 blocks a tile: x -400..400, z -200..200.
        assert_eq!(tiles.len(), 4 * 2);
        let (key, dst) = tiles[0];
        assert_eq!((key.level, key.x, key.z), (1, -2, -1));
        assert_eq!(dst[2], 128.0);
        // An image shows what the store saw, transparent elsewhere.
        let mut store = Store::default();
        store.put(0, 0, grass(64));
        let mut img = Vec::new();
        compose_map_tile(&mut store, TileKey { cut: Cut::Surface, level: 0, x: 0, z: 0 }, &mut img);
        assert_eq!(img[3], 255);
        let o = (20 * 128 + 20) * 4;
        assert_eq!(img[o + 3], 0);
    }

    #[test]
    fn map_slot_ids_are_negative_and_stable() {
        let mut images = MapImages::default();
        let a = images.id(TileKey { cut: Cut::Surface, level: 0, x: 0, z: 0 });
        let b = images.id(TileKey { cut: Cut::Surface, level: 0, x: 1, z: 0 });
        assert!(a < 0 && b < 0 && a != b);
        assert_eq!(images.id(TileKey { cut: Cut::Surface, level: 0, x: 0, z: 0 }), a);
        assert!(a > MINIMAP_SLOT_KEY && b > MINIMAP_SLOT_KEY);
    }
}
