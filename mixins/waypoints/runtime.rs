// Waypoints for Horizon: N makes a waypoint where you stand, M lists them.
// Each waypoint has a name, a colour and a dimension, can be hidden, edited
// or deleted, and shows its name and distance in the world while visible.
// Waypoints are kept per world (single player) and per server address.
// This file is both the mod's Cargo library (its tests) and the module the
// bundle adds to the game; it uses only std, and the hooks hand it plain data.
use std::cell::RefCell;
use std::path::PathBuf;

/// The mod's id (its data folder and its keys' lines), name and version.
pub const ID: &str = "waypoints";
pub const NAME: &str = "Waypoints";
pub const VERSION: &str = "1.0.0";
pub const DESCRIPTION: &str = "N creates a waypoint, M lists them: hide, edit, recolour or delete them. Visible waypoints show their name and distance in the world.";
/// The keys' names and defaults (`key.waypoints.create` in `horizon-mods.txt`).
pub const KEY_CREATE: &str = "create";
pub const KEY_LIST: &str = "list";
pub const DEFAULT_CREATE: &str = "key.keyboard.n";
pub const DEFAULT_LIST: &str = "key.keyboard.m";

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

/// One waypoint. `pos` is a block position; `color` is `0xRRGGBB`.
#[derive(Clone, Debug, PartialEq)]
pub struct Waypoint {
    pub name: String,
    pub pos: [i32; 3],
    pub dimension: String,
    pub color: u32,
    pub hidden: bool,
}

/// Where the player is, as the hooks read it.
#[derive(Clone, Debug, PartialEq)]
pub struct Here {
    pub pos: [f64; 3],
    pub dimension: String,
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

/// What a hook draws, in GUI pixels: a filled `[x, y, w, h]` in ARGB, or a
/// line of text at its top left in RGB, with or without a shadow.
#[derive(Clone, Debug, PartialEq)]
pub enum Draw {
    Fill([f32; 4], u32),
    Text(String, [f32; 2], u32, bool),
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

/// The create / edit form. `index` is the waypoint being edited, None for a
/// new one; `from_list` says Cancel and Save go back to the list.
#[derive(Clone, Debug, PartialEq)]
pub struct Editor {
    pub index: Option<usize>,
    pub name: String,
    pub coords: [String; 3],
    pub dimension: String,
    pub color: u32,
    pub hidden: bool,
    pub focus: Field,
    pub from_list: bool,
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
}

/// A clickable rectangle `[x, y, w, h]` of the screen drawn this frame.
#[derive(Clone, Debug, PartialEq)]
pub struct Widget {
    pub rect: [f32; 4],
    pub action: Action,
}

/// Everything the mod keeps, on the window's thread.
#[derive(Debug)]
pub struct State {
    /// The world or server the waypoints belong to, and their folder.
    pub world: Option<String>,
    pub dir: Option<PathBuf>,
    pub waypoints: Vec<Waypoint>,
    pub mode: Mode,
    pub filter: Filter,
    pub scroll: usize,
    /// The row whose Delete was clicked once and asks to be clicked again.
    pub confirm_delete: Option<usize>,
    /// The character of the key that opened the screen, which arrives right
    /// after the screen opened and must not be typed into it.
    pub skip_char: Option<char>,
    pub close_requested: bool,
    /// Labels in the world on or off (Options > Mods), read once.
    pub labels: Option<bool>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            world: None,
            dir: None,
            waypoints: Vec::new(),
            mode: Mode::Closed,
            filter: Filter::All,
            scroll: 0,
            confirm_delete: None,
            skip_char: None,
            close_requested: false,
            labels: None,
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

/// The file a world's waypoints are kept in, named after its key.
pub fn world_file(key: &str) -> String {
    let safe: String = key
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect();
    format!("{safe}.txt")
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

// ---- The mod's behaviour ---------------------------------------------------

impl State {
    /// Back to nothing open (a screen closed by the game, or refused).
    pub fn reset(&mut self) {
        self.mode = Mode::Closed;
        self.confirm_delete = None;
        self.skip_char = None;
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
        self.world = key;
        self.dir = Some(dir);
        self.scroll = 0;
        self.reset();
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

    /// The create form, filled in with where the player stands.
    pub fn open_create(&mut self, here: &Here, from_list: bool, skip: Option<char>) {
        let block = here.pos.map(|v| v.floor() as i32);
        self.mode = Mode::Edit(Editor {
            index: None,
            name: self.default_name(),
            coords: block.map(|v| v.to_string()),
            dimension: here.dimension.clone(),
            color: PALETTE[self.waypoints.len() % PALETTE.len()],
            hidden: false,
            focus: Field::Name,
            from_list,
            error: None,
            suggested: true,
        });
        self.skip_char = skip;
    }

    pub fn open_list(&mut self, skip: Option<char>) {
        self.mode = Mode::List;
        self.confirm_delete = None;
        self.skip_char = skip;
    }

    fn open_edit(&mut self, index: usize) {
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
            from_list: true,
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

    /// Leaves the editor: back to the list it came from, or closed.
    fn leave_editor(&mut self, from_list: bool) {
        if from_list {
            self.mode = Mode::List;
        } else {
            self.close();
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
        let (index, from_list) = (editor.index, editor.from_list);
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
        self.leave_editor(from_list);
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
    }

    /// Does what a click on `action` does.
    pub fn act(&mut self, action: Action, here: Option<&Here>) {
        if !matches!(action, Action::Delete(_)) {
            self.confirm_delete = None;
        }
        match action {
            Action::NewWaypoint => {
                if let Some(here) = here {
                    self.open_create(here, true, None);
                }
            }
            Action::Filter(filter) => {
                self.filter = filter;
                self.scroll = 0;
            }
            Action::Toggle(i) => {
                if let Some(w) = self.waypoints.get_mut(i) {
                    w.hidden = !w.hidden;
                    self.persist();
                }
            }
            Action::Edit(i) => self.open_edit(i),
            Action::Delete(i) => self.delete(i),
            Action::Close => self.close(),
            Action::Save => self.save(),
            Action::Cancel => {
                if let Mode::Edit(e) = &self.mode {
                    let from_list = e.from_list;
                    self.leave_editor(from_list);
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
        }
    }

    /// A key pressed over the mod's screen (all of them are the mod's).
    pub fn key(&mut self, key: &str, shift: bool, here: Option<&Here>, list_key: &str, create_key: &str) {
        match &mut self.mode {
            Mode::Closed => {}
            Mode::List => match key {
                "key.keyboard.escape" => self.close(),
                "key.keyboard.up" => self.scroll = self.scroll.saturating_sub(1),
                "key.keyboard.down" => self.scroll += 1,
                k if k == list_key => self.close(),
                k if k == create_key => {
                    if let Some(here) = here {
                        self.open_create(here, true, key_char(k));
                    }
                }
                _ => {}
            },
            Mode::Edit(editor) => match key {
                "key.keyboard.escape" => {
                    let from_list = editor.from_list;
                    self.leave_editor(from_list);
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

    /// The wheel over the list: one row per step.
    pub fn scroll_by(&mut self, steps: f64) {
        if steps > 0.0 {
            self.scroll = self.scroll.saturating_sub(1);
        } else if steps < 0.0 {
            self.scroll += 1;
        }
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
        if !self.labels.unwrap_or(true) {
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
            let dist = distance_text(d);
            let (name_w, dist_w) = (measure(&w.name), measure(&dist));
            let width = name_w + 6.0 + dist_w;
            let left = (x - width / 2.0).round();
            let top = (y - 16.0).round();
            draws.push(Draw::Fill([left - 3.0, top - 2.0, width + 6.0, 12.0], LABEL_BG));
            draws.push(Draw::Text(w.name.clone(), [left, top], w.color, true));
            draws.push(Draw::Text(dist, [left + name_w + 6.0, top], INK, true));
            // The marker: a dot in the waypoint's colour, ringed in black.
            draws.push(Draw::Fill([x - 3.0, y - 3.0, 6.0, 6.0], 0xFF00_0000));
            draws.push(Draw::Fill([x - 2.0, y - 2.0, 4.0, 4.0], 0xFF00_0000 | w.color));
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
        let mut out = Out {
            draws: Vec::new(),
            widgets: Vec::new(),
            mouse,
            measure,
        };
        out.fill([0.0, 0.0, size[0], size[1]], SHADE);
        match self.mode.clone() {
            Mode::Closed => {}
            Mode::List => self.draw_list(&mut out, size, here),
            Mode::Edit(editor) => draw_editor(&mut out, size, &editor),
        }
        (out.draws, out.widgets)
    }

    fn draw_list(&mut self, out: &mut Out<'_>, size: [f32; 2], here: Option<&Here>) {
        let pw = (size[0] - 16.0).min(340.0);
        let ph = (size[1] - 16.0).max(120.0);
        let (px, py) = (((size[0] - pw) / 2.0).round(), 8.0);
        out.boxed([px, py, pw, ph], PANEL_EDGE, PANEL);
        let right = px + pw - 6.0;

        let title = format!("Waypoints ({})", self.waypoints.len());
        out.text(&title, [px + 6.0, py + 6.0], INK);
        let close_w = out.width("Close") + 8.0;
        out.button([right - close_w, py + 4.0, close_w, 12.0], "Close", Action::Close, false);
        let new_w = out.width("+ New") + 8.0;
        out.button(
            [right - close_w - 4.0 - new_w, py + 4.0, new_w, 12.0],
            "+ New",
            Action::NewWaypoint,
            false,
        );

        let mut x = px + 6.0;
        for filter in Filter::ALL {
            let on = self.filter == filter;
            x += out.button_auto(x, py + 20.0, filter.label(), Action::Filter(filter), on) + 3.0;
        }

        let rows: Vec<usize> = self
            .waypoints
            .iter()
            .enumerate()
            .filter(|(_, w)| self.filter.matches(&w.dimension))
            .map(|(i, _)| i)
            .collect();
        let top = py + 38.0;
        let bottom = py + ph - 16.0;
        let fits = (((bottom - top) / ROW).floor() as usize).max(1);
        self.scroll = self.scroll.min(rows.len().saturating_sub(fits));

        if rows.is_empty() {
            let msg = if self.waypoints.is_empty() {
                "No waypoints yet. Press N in game or click + New."
            } else {
                "No waypoints in this dimension."
            };
            out.text(msg, [px + 8.0, top + 6.0], MUTED);
        }
        for (slot, &i) in rows.iter().skip(self.scroll).take(fits).enumerate() {
            let w = &self.waypoints[i];
            let ry = top + slot as f32 * ROW;
            out.fill([px + 4.0, ry, pw - 8.0, ROW - 2.0], ROW_BG);

            // Buttons, from the right.
            let del = if self.confirm_delete == Some(i) { "Sure?" } else { "Delete" };
            let del_w = out.width(del) + 8.0;
            let edit_w = out.width("Edit") + 8.0;
            let vis = if w.hidden { "Show" } else { "Hide" };
            let vis_w = out.width("Show").max(out.width("Hide")) + 8.0;
            let by = ry + (ROW - 2.0 - 12.0) / 2.0;
            let mut bx = right - del_w;
            out.button([bx, by, del_w, 12.0], del, Action::Delete(i), false);
            bx -= edit_w + 3.0;
            out.button([bx, by, edit_w, 12.0], "Edit", Action::Edit(i), false);
            bx -= vis_w + 3.0;
            out.button([bx, by, vis_w, 12.0], vis, Action::Toggle(i), w.hidden);

            // Dimension and distance, right-aligned before the buttons.
            let dim = dimension_label(&w.dimension);
            let dist = match here {
                Some(h) if h.dimension == w.dimension => distance_text(distance(h.pos, centre(w))),
                _ => "other dimension".into(),
            };
            let info_right = bx - 6.0;
            let dim_w = out.width(&dim);
            let dist_w = out.width(&dist);
            out.text(&dim, [info_right - dim_w, ry + 3.0], dimension_color(&w.dimension));
            out.text(&dist, [info_right - dist_w, ry + 3.0 + LINE], MUTED);

            // Colour, name and coordinates.
            out.fill([px + 8.0, ry + 6.0, 10.0, 10.0], 0xFF00_0000);
            out.fill([px + 9.0, ry + 7.0, 8.0, 8.0], 0xFF00_0000 | w.color);
            let text_x = px + 22.0;
            let room = info_right - dim_w.max(dist_w) - 6.0 - text_x;
            let name = out.fit(&w.name, room);
            out.text(&name, [text_x, ry + 3.0], if w.hidden { MUTED } else { INK });
            let coords = format!("{} {} {}", w.pos[0], w.pos[1], w.pos[2]);
            let coords = out.fit(&coords, room);
            out.text(&coords, [text_x, ry + 3.0 + LINE], MUTED);
        }

        let more = rows.len().saturating_sub(self.scroll + fits);
        let hint = if self.scroll > 0 || more > 0 {
            format!("Scroll for more ({} above, {} below)", self.scroll, more)
        } else {
            "N new waypoint  -  M or Esc close".to_owned()
        };
        out.text(&hint, [px + 6.0, py + ph - 12.0], MUTED);
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

    let vis = if e.hidden { "Visible in world: No" } else { "Visible in world: Yes" };
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

/// Labels in the world: on unless the setting says `false`.
pub fn read_labels(stored: Option<String>) -> bool {
    stored.is_none_or(|v| v.trim() != "false")
}

pub fn labels_label(on: bool) -> String {
    format!("Labels in the world: {}", if on { "ON" } else { "OFF" })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn here(dim: &str) -> Here {
        Here {
            pos: [10.7, 64.0, -3.2],
            dimension: dim.into(),
        }
    }

    fn measure(s: &str) -> f32 {
        s.chars().count() as f32 * 6.0
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
    }

    #[test]
    fn words() {
        assert_eq!(dimension_label(NETHER), "Nether");
        assert_eq!(dimension_label("mymod:deep_dark"), "deep dark");
        assert_eq!(distance_text(4.4), "4 m");
        assert_eq!(distance_text(1530.0), "1.5 km");
        assert_eq!(key_char("key.keyboard.n"), Some('n'));
        assert_eq!(key_char("key.keyboard.f1"), None);
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
        s.open_create(&here(OVERWORLD), false, Some('n'));
        // The opening key's character is not typed.
        s.char_typed('n');
        s.char_typed('!');
        // Typing replaces the suggested name.
        let Mode::Edit(e) = &s.mode else { panic!() };
        assert_eq!(e.name, "!");
        assert_eq!(e.coords, ["10".to_string(), "64".into(), "-4".into()]);
        s.key("key.keyboard.backspace", false, None, DEFAULT_LIST, DEFAULT_CREATE);
        for c in "Home".chars() {
            s.char_typed(c);
        }
        s.key("key.keyboard.tab", false, None, DEFAULT_LIST, DEFAULT_CREATE);
        s.char_typed('x'); // not a number: ignored
        s.act(Action::Color(0x5555FF), None);
        s.key("key.keyboard.enter", false, None, DEFAULT_LIST, DEFAULT_CREATE);
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
        s.open_create(&here(OVERWORLD), true, None);
        s.act(Action::Focus(Field::Y), None);
        s.key("key.keyboard.delete", false, None, DEFAULT_LIST, DEFAULT_CREATE);
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
        let edit = widgets.iter().find(|w| w.action == Action::Edit(0)).unwrap();
        let at = [edit.rect[0] + 1.0, edit.rect[1] + 1.0];
        assert_eq!(hit(&widgets, at), Some(Action::Edit(0)));

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
                Draw::Text(t, ..) => Some(t.as_str()),
                Draw::Fill(..) => None,
            })
            .collect();
        assert_eq!(texts, ["A", "20 m"]);
        s.labels = Some(false);
        assert!(s.labels(camera, [0.5, 64.0, 0.5], OVERWORLD, [480.0, 270.0], &measure).is_empty());
    }
}
