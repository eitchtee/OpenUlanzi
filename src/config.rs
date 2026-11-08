//! TOML profile. See `examples/default.toml`.
use crate::{platform, protocol::WIDE};
use anyhow::{anyhow, bail, Context, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, HashMap};
use std::hash::{Hash, Hasher};
use std::path::Path;

/// Frames of the hold border. Each one is a real screen update, so keep this low.
pub const HOLD_STEPS: u8 = 16;

/// Layer name that means "the page of the app focused at press time".
pub const APP_LAYER: &str = "@app";
/// Profile name that means "whichever profile is the default".
pub const DEFAULT_PROFILE: &str = "@default";
/// A folder's Back button always sits on this key.
pub const FOLDER_BACK_KEY: u8 = 0;
/// Where a plugin button keeps its one binding among a view's bindings.
pub const PLUGIN_BUTTON: u8 = 0;

/// What plugins currently draw: bindings by control, for each instance of a `PluginView`.
pub type Surfaces = HashMap<String, HashMap<u8, Binding>>;

// The derived (de)serializers are wrapped below, so `"next_page"` stays a plain string.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    /// `ctrl+shift+f13`, `volume_up`, `vkea`, ...
    Hotkey(String),
    /// Types the text, whatever the keyboard layout.
    Text(String),
    /// Any file, shortcut, script or URL, launched as if double-clicked.
    Open(String),
    /// Closes a program by exe name, or the focused window when empty.
    Close(String),
    /// Opens a page of the same profile like a folder: `back` returns from it.
    Page(String),
    /// Shows a page for as long as the key is held; `@app` is the focused app's. Only for `hold`.
    Show(String),
    /// Switches to another profile, at the page it was last on. `@app` is the focused app's
    /// profile, if it has one, and `@default` is the default profile.
    Profile(String),
    Back,
    /// Changes how a key looks until the config reloads. Fields left out stay as they are.
    SetLook(Look),
    /// Puts a key changed by `set_look` back to how the config draws it.
    ResetLook(Target),
    /// `set_look` or `reset_look` from a sequence, tied to the key that started it.
    #[serde(skip)]
    Restyle(KeyAt, Box<Action>),
    /// Ignores every button until one bound to `unblock_input` runs it.
    BlockInput,
    UnblockInput,
    /// Without `wrap`, the last page goes on to the first and the other way round. Written
    /// `"next_page"` when it wraps, else `{ next_page = { wrap = false } }`.
    NextPage {
        #[serde(default = "default_true")]
        wrap: bool,
    },
    PrevPage {
        #[serde(default = "default_true")]
        wrap: bool,
    },
    /// Switches to the focused app's profile, and returns to where it started when that app loses
    /// focus. With `others`, moving to another app with a profile follows it instead.
    FollowApp {
        #[serde(default)]
        others: bool,
    },
    /// Runs a page, folder or profile switch, then returns to where it started once the next
    /// button is pressed and released.
    OneShot(Box<Action>),
    /// Mouse wheel notches.
    ScrollUp(u32),
    ScrollDown(u32),
    ScrollLeft(u32),
    ScrollRight(u32),
    /// Relative change, e.g. `{ brightness = -10 }`.
    Brightness(i32),
    /// Changes the focused app's volume by this many percent.
    AppVolume(i32),
    /// Puts the focused app back at 100%, where it follows the system volume.
    AppVolumeReset,
    PlaySound(String),
    StopSound,
    /// Runs every step in order. `delay` steps wait, in milliseconds.
    Sequence(Vec<Action>),
    Delay(u64),
    /// Each press runs the next action in the list.
    Toggle(Vec<Action>),
    /// Each press runs one action picked at random.
    Random(Vec<Action>),
    /// Sent to OBS Studio. Does nothing while OBS is not connected.
    Obs(ObsAction),
    /// Sent to a plugin. One that is not installed does nothing, and the config still loads.
    Plugin {
        id: String,
        action: String,
        #[serde(default)]
        params: BTreeMap<String, String>,
    },
}

impl Serialize for Action {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Action::NextPage { wrap: true } => s.serialize_str("next_page"),
            Action::PrevPage { wrap: true } => s.serialize_str("prev_page"),
            a => Action::serialize(a, s),
        }
    }
}

impl<'de> Deserialize<'de> for Action {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        // JSON's value, not TOML's: the editor sends left-out fields as null, which TOML lacks.
        let value = serde_json::Value::deserialize(d)?;
        match value.as_str() {
            Some("next_page") => Ok(Action::NextPage { wrap: true }),
            Some("prev_page") => Ok(Action::PrevPage { wrap: true }),
            _ => Action::deserialize(value).map_err(serde::de::Error::custom),
        }
    }
}

/// A plugin's page or button placed in the config. The plugin decides what it shows and does.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginView {
    pub id: String,
    pub view: String,
    #[serde(default)]
    pub params: BTreeMap<String, String>,
}

impl PluginView {
    /// Placements with the same settings are one instance, so they show the same thing everywhere.
    pub fn instance(&self) -> String {
        let mut hasher = DefaultHasher::new();
        (&self.view, &self.params).hash(&mut hasher);
        format!("{}/{:016x}", self.id, hasher.finish())
    }
}

/// A key of a page: ((profile, page), control).
pub type KeyAt = ((usize, usize), u8);

/// The key a look action changes. What is left out is the key that runs it: its profile, and its
/// page while the profile is the same, otherwise the profile's first page.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Target {
    pub profile: Option<String>,
    pub page: Option<String>,
    pub key: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Look {
    pub label: Option<String>,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub background: Option<String>,
    pub image: Option<String>,
    /// Goes with `icon` and `image` from the same look; a new picture without one shows whole.
    pub icon_crop: Option<Crop>,
    pub image_crop: Option<Crop>,
    // Inline rather than a nested table, which serde cannot flatten next to deny_unknown_fields.
    pub profile: Option<String>,
    pub page: Option<String>,
    pub key: Option<String>,
}

impl Look {
    pub fn target(&self) -> Target {
        Target { profile: self.profile.clone(), page: self.page.clone(), key: self.key.clone() }
    }
}

impl Action {
    /// Is `target` or runs it as one of its steps.
    pub fn runs(&self, target: &Action) -> bool {
        match self {
            a if a == target => true,
            Action::OneShot(a) => a.runs(target),
            Action::Sequence(list) | Action::Toggle(list) | Action::Random(list) => list.iter().any(|a| a.runs(target)),
            _ => false,
        }
    }

    pub fn unblocks(&self) -> bool {
        self.runs(&Action::UnblockInput)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaAction {
    Play,
    Pause,
    Stop,
    Restart,
    Next,
    Previous,
}

/// Names are the ones shown in OBS.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum ObsAction {
    ToggleRecord,
    ToggleRecordPause,
    ToggleStream,
    ToggleVirtualCam,
    ToggleReplayBuffer,
    SaveReplay,
    ToggleStudioMode,
    /// Sends the preview to program in studio mode.
    Transition,
    /// Saves what is on air next to the recordings.
    Screenshot,
    /// Marks a chapter in the recording. Empty lets OBS name it.
    Chapter(String),
    Scene(String),
    SceneCollection(String),
    Profile(String),
    /// Which transition scene changes use.
    SceneTransition(String),
    ToggleSource { scene: String, source: String },
    ToggleFilter { source: String, filter: String },
    Media { input: String, action: MediaAction },
    ToggleMute(String),
    /// Relative change in dB.
    Volume { input: String, db: i32 },
}

/// What a screen shows. Everything but `image` and `page_indicator` is drawn by the firmware,
/// which only offers them on the wide screen.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Display {
    #[default]
    Image,
    PageIndicator,
    Dial,
    Time,
    TimeWeekday,
    TimeDate,
    DateTimeWeekday,
    Stats,
}

impl Display {
    /// Value of the protocol's small-window mode byte.
    pub fn mode(self) -> u16 {
        match self {
            Self::Image | Self::PageIndicator => 2,
            Self::Dial => 1,
            Self::Stats => 0,
            Self::DateTimeWeekday => 200,
            Self::TimeWeekday => 201,
            Self::TimeDate => 202,
            Self::Time => 203,
        }
    }
}

/// Part of a picture as `[x, y, width, height]`, each a fraction of the picture's size.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Crop(pub [f32; 4]);

// Visuals key the image cache, and a crop never holds NaN once validated.
impl Eq for Crop {}

impl std::hash::Hash for Crop {
    fn hash<H: std::hash::Hasher>(&self, h: &mut H) {
        self.0.map(f32::to_bits).hash(h);
    }
}

impl Crop {
    fn check(&self) -> Result<()> {
        let [x, y, w, h] = self.0;
        let inside = |start: f32, len: f32| start >= 0.0 && len > 0.0 && start + len <= 1.0001;
        if !(inside(x, w) && inside(y, h)) {
            bail!("crop {:?} must lie inside the picture, as fractions from 0 to 1", self.0);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default)]
pub struct Visual {
    pub label: String,
    pub icon: String,
    pub color: String,
    pub background: String,
    /// Picture under the label and icon, filling the key.
    pub image: String,
    pub image_crop: Option<Crop>,
    pub icon_crop: Option<Crop>,
    pub display: Display,
    /// SVG document from a plugin, drawn over the background and scaled to fit.
    pub svg: String,
    /// Large centred text, such as the page indicator's `2/3`.
    pub big: String,
    /// How much of the hold border is drawn, up to `HOLD_STEPS`.
    pub progress: u8,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub label: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub icon: String,
    pub color: Option<String>,
    pub background: Option<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub image: String,
    /// The part of `image` shown; without it the picture is centred and fills the key.
    pub image_crop: Option<Crop>,
    /// The part of a picture icon shown; without it the whole picture is.
    pub icon_crop: Option<Crop>,
    /// SVG document drawn over the background. Plugins send these.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub svg: String,
    pub display: Option<Display>,
    /// Keys: runs on press, or on tap when `hold` is also set. Knobs: runs on click.
    pub action: Option<Action>,
    /// Runs on a double click. A single click then waits `double_ms` to be sure it is one.
    pub double: Option<Action>,
    /// Runs when held for `hold_ms`. `show` keeps a page up until release instead.
    pub hold: Option<Action>,
    /// Older configs: `layer = "page"` is `hold = { show = "page" }`.
    #[serde(default, skip_serializing)]
    pub layer: Option<String>,
    pub hold_ms: Option<u64>,
    pub left: Option<Action>,
    pub right: Option<Action>,
    pub press_left: Option<Action>,
    pub press_right: Option<Action>,
    /// A plugin button: while the plugin draws it, its binding replaces this one.
    pub plugin: Option<PluginView>,
}

impl Binding {
    /// Mistakes that would otherwise only show up when drawing.
    pub fn check_look(&self) -> Result<()> {
        [&self.color, &self.background].into_iter().flatten().try_for_each(|c| check_color(c))?;
        [&self.image_crop, &self.icon_crop].into_iter().flatten().try_for_each(Crop::check)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Page {
    pub name: String,
    /// next_page/prev_page skip it; holding, rules and page actions still reach it.
    #[serde(default)]
    pub hidden: bool,
    /// Opened by an action, with a fixed Back key. Knobs and side buttons stay those of the page
    /// it was opened from, and it is never part of next_page/prev_page.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub folder: bool,
    /// A plugin page: the plugin owns the keys and the wide screen. Knobs and side buttons stay
    /// the ones set here, and a folder's Back key stays too.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin: Option<PluginView>,
    #[serde(default, rename = "keys")]
    raw_keys: BTreeMap<String, Binding>,
    #[serde(skip)]
    pub keys: HashMap<u8, Binding>,
}

/// A set of pages. With conditions it follows an app; the first profile without any is the default.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub name: String,
    /// Where the deck goes when no app claims it.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub default: bool,
    pub exe: Option<String>,
    pub title: Option<String>,
    pub title_regex: Option<String>,
    /// Switch to this profile by itself while a matching app has focus.
    #[serde(default = "default_true")]
    pub on_focus: bool,
    /// Used by keys that hold "the page of the focused app".
    #[serde(default = "default_true")]
    pub on_hold: bool,
    #[serde(default, rename = "page")]
    pub pages: Vec<Page>,
    #[serde(skip)]
    regex: Option<Regex>,
}

impl Profile {
    fn has_conditions(&self) -> bool {
        self.exe.is_some() || self.title.is_some() || self.title_regex.is_some()
    }

    /// All given conditions must hold; a profile without conditions matches nothing.
    pub fn matches(&self, w: &platform::Window) -> bool {
        let exe_name = w.exe.rsplit(['\\', '/']).next().unwrap_or(&w.exe);
        // A Mac app matches with or without its .app, as it shows in Finder.
        let app_name = exe_name.strip_suffix(".app").unwrap_or(exe_name);
        self.has_conditions()
            && self.exe.as_ref().is_none_or(|e| e.eq_ignore_ascii_case(exe_name) || e.eq_ignore_ascii_case(app_name) || e.eq_ignore_ascii_case(&w.exe))
            && self.title.as_ref().is_none_or(|t| w.title.to_lowercase().contains(&t.to_lowercase()))
            && self.regex.as_ref().is_none_or(|r| r.is_match(&w.title))
    }

    pub fn page_index(&self, name: &str) -> Option<usize> {
        self.pages.iter().position(|p| p.name == name)
    }
}

/// Format before profiles: `[[page]]` and `[[app]]` at the top level. Read, never written.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct OldRule {
    page: String,
    exe: Option<String>,
    title: Option<String>,
    title_regex: Option<String>,
    #[serde(default = "default_true")]
    on_hold: bool,
    #[serde(default)]
    on_focus: bool,
}

fn default_brightness() -> u8 {
    80
}
fn default_weekday() -> String {
    "ddd".into()
}
fn default_wide() -> Display {
    Display::Time
}
fn default_hold_action() -> u64 {
    600
}
fn default_double() -> u64 {
    250
}
fn default_hold() -> u64 {
    200
}
fn default_true() -> bool {
    true
}
fn default_color() -> String {
    "#4ea1ff".into()
}
fn default_background() -> String {
    "#17181c".into()
}

/// Where OBS's WebSocket server listens. OBS shows these under Tools, WebSocket Server Settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ObsSettings {
    pub host: String,
    pub port: u16,
    pub password: String,
}

impl Default for ObsSettings {
    fn default() -> Self {
        Self { host: "127.0.0.1".into(), port: 4455, password: String::new() }
    }
}

/// USB cannot tell these apart, so the owner picks one. It only decides what the editor offers.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Model {
    #[default]
    D200x,
    D200h,
    D200,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// What to call this deck when several are connected.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    #[serde(default)]
    pub model: Model,
    #[serde(default = "default_brightness")]
    pub brightness: u8,
    #[serde(default)]
    pub locked_brightness: u8,
    /// How long a hold takes to show a page on keys that also have a click action.
    #[serde(default = "default_hold")]
    pub hold_ms: u64,
    /// How long a hold takes to run an action.
    #[serde(default = "default_hold_action")]
    pub hold_action_ms: u64,
    /// Longest gap between the two clicks of a double click.
    #[serde(default = "default_double")]
    pub double_ms: u64,
    /// Look for new releases on GitHub at startup and once a day.
    #[serde(default = "default_true")]
    pub check_updates: bool,
    /// Turn the deck off after this long without keyboard or mouse input. 0 keeps it on.
    #[serde(default)]
    pub sleep_minutes: u32,
    /// Turn the deck off while Windows has the screens off.
    #[serde(default)]
    pub sleep_with_display: bool,
    /// Wide screen on pages that don't set it themselves.
    #[serde(default = "default_wide")]
    pub wide: Display,
    #[serde(default)]
    pub twelve_hour: bool,
    /// Windows date picture for the clock, like `dd/MM/yyyy`. Empty uses the system short date.
    #[serde(default)]
    pub date_format: String,
    /// Windows picture for the weekday: `ddd` is short, `dddd` is full, in the system language.
    #[serde(default = "default_weekday")]
    pub weekday_format: String,
    /// Older configs: `clock = false` meant a blank wide screen. `overlay` no longer exists.
    #[serde(default, skip_serializing)]
    clock: Option<bool>,
    #[allow(dead_code)]
    #[serde(default, skip_serializing)]
    overlay: Option<bool>,
    #[serde(default = "default_color")]
    pub color: String,
    #[serde(default = "default_background")]
    pub background: String,
    #[serde(default)]
    pub obs: ObsSettings,
    #[serde(default, rename = "profile")]
    pub profiles: Vec<Profile>,
    #[serde(default, rename = "page", skip_serializing)]
    old_pages: Vec<Page>,
    #[serde(default, rename = "app", skip_serializing)]
    old_rules: Vec<OldRule>,
}

/// Controls as named in the config: `0`-`12`, `wide`, `side1-2`, `knob1-3`.
pub fn control_index(name: &str) -> Option<u8> {
    match name {
        "wide" => Some(WIDE),
        "side1" => Some(15),
        "side2" => Some(16),
        "knob1" => Some(17),
        "knob2" => Some(18),
        "knob3" => Some(19),
        n => n.parse().ok().filter(|i| *i < WIDE),
    }
}

fn check_color(c: &str) -> Result<()> {
    if c.len() == 7 && c.starts_with('#') && c[1..].chars().all(|d| d.is_ascii_hexdigit()) {
        Ok(())
    } else {
        bail!("invalid color {c:?}, expected #rrggbb")
    }
}

/// Empty hotkeys pass: an action just dropped on a key has not been filled in yet.
fn check_action(action: &Action, pages: &[String], profiles: &[String]) -> Result<()> {
    match action {
        Action::Page(p) if !pages.contains(p) => bail!("unknown page {p:?}"),
        Action::Show(_) => bail!("showing a page only works as a hold action"),
        Action::Profile(p) if !profiles.contains(p) && p != APP_LAYER && p != DEFAULT_PROFILE => {
            bail!("unknown profile {p:?}")
        }
        Action::Hotkey(h) if !h.is_empty() => platform::parse_hotkey(h).map(|_| ()),
        Action::OneShot(inner) => check_action(inner, pages, profiles),
        Action::Sequence(list) | Action::Toggle(list) | Action::Random(list) => {
            list.iter().try_for_each(|a| check_action(a, pages, profiles))
        }
        Action::SetLook(look) => {
            [&look.color, &look.background].into_iter().flatten().try_for_each(|c| check_color(c))?;
            [&look.icon_crop, &look.image_crop].into_iter().flatten().try_for_each(Crop::check)
        }
        _ => Ok(()),
    }
}

fn check_names<'a>(kind: &str, names: impl Iterator<Item = &'a String>) -> Result<Vec<String>> {
    let mut seen: Vec<String> = Vec::new();
    for name in names {
        if name.trim().is_empty() {
            bail!("a {kind} needs a name");
        }
        if seen.contains(name) {
            bail!("two {kind}s are named {name:?}");
        }
        seen.push(name.clone());
    }
    Ok(seen)
}

fn actions_mut(b: &mut Binding) -> impl Iterator<Item = &mut Action> {
    [&mut b.action, &mut b.double, &mut b.left, &mut b.right, &mut b.press_left, &mut b.press_right]
        .into_iter()
        .flatten()
}

fn mentions(action: &mut Action, page: &str) -> bool {
    match action {
        Action::Page(p) => p == page,
        Action::Sequence(list) | Action::Toggle(list) | Action::Random(list) => {
            list.iter_mut().any(|a| mentions(a, page))
        }
        _ => false,
    }
}

/// A page moved to its own profile can no longer reach pages left behind.
fn retarget(action: &mut Action, pages: &[String], default: &str) {
    match action {
        Action::Page(p) if !pages.contains(p) => *action = Action::Profile(default.to_string()),
        Action::Sequence(list) | Action::Toggle(list) | Action::Random(list) => {
            list.iter_mut().for_each(|a| retarget(a, pages, default))
        }
        _ => {}
    }
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        Self::parse(&text)
    }

    pub fn parse(text: &str) -> Result<Self> {
        toml::from_str::<Config>(text)?.validated()
    }

    /// Writes atomically, keeping the previous file as `.bak`: saving drops hand-written comments.
    pub fn save(&self, path: &Path) -> Result<()> {
        let text = toml::to_string_pretty(self)?;
        Self::parse(&text).context("generated config does not read back")?;
        let temp = path.with_extension("toml.tmp");
        std::fs::write(&temp, text)?;
        if path.exists() {
            std::fs::copy(path, path.with_extension("toml.bak"))?;
        }
        std::fs::rename(&temp, path)?;
        Ok(())
    }

    /// Checks everything a person could get wrong and builds the lookup tables.
    /// Turns the old format into profiles: all pages in a default one, plus one profile per rule.
    fn migrate(&mut self) {
        const DEFAULT: &str = "Default";
        if self.old_pages.is_empty() {
            return;
        }
        let mut pages = std::mem::take(&mut self.old_pages);
        let mut extra: Vec<Profile> = Vec::new();
        for rule in std::mem::take(&mut self.old_rules) {
            let Some(mut page) = pages.iter().find(|p| p.name == rule.page).cloned() else { continue };
            page.hidden = false;
            let names = [page.name.clone()];
            for b in page.raw_keys.values_mut() {
                actions_mut(b).for_each(|a| retarget(a, &names, DEFAULT));
                b.layer = b.layer.take().filter(|l| l == APP_LAYER || names.contains(l));
            }
            let mut name = rule.page.clone();
            while name == DEFAULT || extra.iter().any(|p| p.name == name) {
                name.push_str(" copy");
            }
            extra.push(Profile {
                name,
                default: false,
                exe: rule.exe,
                title: rule.title,
                title_regex: rule.title_regex,
                on_focus: rule.on_focus,
                on_hold: rule.on_hold,
                pages: vec![page],
                regex: None,
            });
        }
        let moved: Vec<String> = extra.iter().flat_map(|p| p.pages.iter().map(|q| q.name.clone())).collect();
        let needed: Vec<String> = moved
            .iter()
            .filter(|name| {
                pages.clone().iter_mut().filter(|p| p.name != **name).any(|p| {
                    p.raw_keys.values_mut().any(|b| {
                        b.layer.as_deref() == Some(name.as_str()) || actions_mut(b).any(|a| mentions(a, name))
                    })
                })
            })
            .cloned()
            .collect();
        // Rule pages that were skipped in navigation now live only in their profile.
        pages.retain(|p| !p.hidden || !moved.contains(&p.name) || needed.contains(&p.name));
        let mut profiles = vec![Profile {
            name: DEFAULT.to_string(),
            default: false,
            exe: None,
            title: None,
            title_regex: None,
            on_focus: true,
            on_hold: true,
            pages,
            regex: None,
        }];
        profiles.append(&mut extra);
        profiles.append(&mut self.profiles);
        self.profiles = profiles;
    }

    pub fn validated(mut self) -> Result<Self> {
        self.migrate();
        for page in self.profiles.iter_mut().flat_map(|p| &mut p.pages) {
            for b in page.raw_keys.values_mut() {
                if let Some(layer) = b.layer.take() {
                    b.hold = Some(Action::Show(layer));
                }
            }
        }
        if self.clock.take() == Some(false) {
            self.wide = Display::Image;
        }
        let cfg = &mut self;
        if cfg.profiles.is_empty() {
            bail!("config needs at least one [[profile]]");
        }
        check_color(&cfg.color)?;
        check_color(&cfg.background)?;
        let profiles = check_names("profile", cfg.profiles.iter().map(|p| &p.name))?;
        for profile in &mut cfg.profiles {
            if profile.pages.is_empty() {
                bail!("profile {:?} needs at least one page", profile.name);
            }
            if profile.pages[0].folder {
                bail!("profile {:?} needs a normal page first, not a folder", profile.name);
            }
            let names = check_names("page", profile.pages.iter().map(|p| &p.name))?;
            for page in &mut profile.pages {
                page.keys.clear();
                for (name, b) in page.raw_keys.clone() {
                    let ctx = format!("page {:?} key {name}", page.name);
                    let index = control_index(&name).ok_or_else(|| anyhow!("{ctx}: unknown control"))?;
                    b.check_look().context(ctx.clone())?;
                    match &b.hold {
                        Some(Action::Show(l)) if l != APP_LAYER && !names.contains(l) => {
                            bail!("{ctx}: unknown page {l:?}");
                        }
                        Some(Action::Show(_)) | None => {}
                        Some(a) => check_action(a, &names, &profiles).context(ctx.clone())?,
                    }
                    let actions = [&b.action, &b.double, &b.left, &b.right, &b.press_left, &b.press_right];
                    for a in actions.into_iter().flatten() {
                        check_action(a, &names, &profiles).context(ctx.clone())?;
                    }
                    page.keys.insert(index, b);
                }
            }
            profile.regex = profile.title_regex.as_deref().map(Regex::new).transpose()?;
        }
        Ok(self)
    }

    /// The marked profile, or else the first one with no conditions.
    pub fn default_profile(&self) -> usize {
        let marked = self.profiles.iter().position(|p| p.default);
        marked.or_else(|| self.profiles.iter().position(|p| !p.has_conditions())).unwrap_or(0)
    }

    pub fn visual(&self, b: &Binding) -> Visual {
        Visual {
            label: b.label.clone(),
            icon: b.icon.clone(),
            color: b.color.clone().unwrap_or_else(|| self.color.clone()),
            background: b.background.clone().unwrap_or_else(|| self.background.clone()),
            image: b.image.clone(),
            image_crop: b.image_crop,
            icon_crop: b.icon_crop,
            svg: b.svg.clone(),
            display: b.display.unwrap_or_default(),
            big: String::new(),
            progress: 0,
        }
    }

    /// The wide screen follows the global setting until a binding gives it something to draw.
    pub fn wide_display(&self, b: Option<&Binding>) -> Display {
        let Some(b) = b else { return self.wide };
        let drawn = !b.label.is_empty() || !b.icon.is_empty() || !b.image.is_empty() || !b.svg.is_empty() || b.color.is_some() || b.background.is_some();
        b.display.unwrap_or(if drawn { Display::Image } else { self.wide })
    }

    pub fn blank(&self) -> Visual {
        self.visual(&Binding::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window(exe: &str, title: &str) -> platform::Window {
        platform::Window { exe: exe.into(), title: title.into() }
    }

    #[test]
    fn actions_from_the_editor_may_hold_nulls() {
        let json = r#"{ "set_look": { "label": "x", "icon": null, "key": null } }"#;
        let action: Action = serde_json::from_str(json).unwrap();
        assert_eq!(action, Action::SetLook(Look { label: Some("x".into()), ..Default::default() }));
        assert_eq!(serde_json::from_str::<Action>(r#""next_page""#).unwrap(), Action::NextPage { wrap: true });
    }

    #[test]
    fn example_config_is_valid() {
        let cfg = Config::parse(include_str!("../examples/default.toml")).unwrap();
        assert!(cfg.profiles.len() >= 2);
    }

    #[test]
    fn saved_config_reads_back_the_same() {
        let cfg = Config::parse(include_str!("../examples/default.toml")).unwrap();
        let path = std::env::temp_dir().join("openulanzi-save-test.toml");
        cfg.save(&path).unwrap();
        let back = Config::load(&path).unwrap();
        assert_eq!(serde_json::to_string(&cfg).unwrap(), serde_json::to_string(&back).unwrap());
        assert_eq!(back.profiles[0].pages[0].keys[&12].action, Some(Action::NextPage { wrap: true }));
    }

    #[test]
    fn old_format_becomes_profiles() {
        let old = r#"
            clock = false
            overlay = true
            [[page]]
            name = "main"
            [page.keys.0]
            layer = "media"
            [page.keys.1]
            action = { page = "second" }
            [[page]]
            name = "second"
            [[page]]
            name = "media"
            hidden = true
            [[page]]
            name = "zed"
            hidden = true
            [page.keys.0]
            action = { page = "main" }
            [page.keys.1]
            layer = "media"
            [[page]]
            name = "shared"
            hidden = true
            [[app]]
            page = "zed"
            exe = "zed.exe"
            on_focus = true
            [[app]]
            page = "media"
            exe = "vlc.exe"
        "#;
        let cfg = Config::parse(old).unwrap();
        assert_eq!(cfg.wide, Display::Image);
        let names: Vec<&str> = cfg.profiles.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["Default", "zed", "media"]);
        let default: Vec<&str> = cfg.profiles[0].pages.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(default, ["main", "second", "media", "shared"], "media is still held, zed moved out");
        let zed = &cfg.profiles[1];
        assert!(zed.on_focus && zed.exe.is_some() && !zed.pages[0].hidden);
        assert_eq!(zed.pages[0].keys[&0].action, Some(Action::Profile("Default".into())));
        assert_eq!(zed.pages[0].keys[&1].hold, None);
        assert_eq!(cfg.profiles[0].pages[0].keys[&0].hold, Some(Action::Show("media".into())));
        assert_eq!(cfg.profiles[0].pages[0].keys[&0].layer, None);
        assert_eq!(cfg.default_profile(), 0);
        let text = toml::to_string_pretty(&cfg).unwrap();
        assert!(!text.contains("clock") && !text.contains("overlay") && !text.contains("[[app]]"));
        assert!(Config::parse(&text).is_ok());
    }

    #[test]
    fn one_shot_is_checked_and_saved() {
        let good = "[[profile]]\nname='p'\n[[profile.page]]\nname='a'\n[profile.page.keys.0]\naction={one_shot={page='a'}}";
        let cfg = Config::parse(good).unwrap();
        assert!(Config::parse(&toml::to_string_pretty(&cfg).unwrap()).is_ok());
        let bad = "[[profile]]\nname='p'\n[[profile.page]]\nname='a'\n[profile.page.keys.0]\naction={one_shot={page='nope'}}";
        assert!(Config::parse(bad).is_err());
    }

    #[test]
    fn nested_actions_are_checked_and_saved() {
        let mixed = "[[profile]]\nname='p'\n[[profile.page]]\nname='a'\n[profile.page.keys.0]\naction={sequence=[{hotkey='ctrl+c'},{delay=50},'back',{scroll_left=3},{scroll_up=2}]}";
        let cfg = Config::parse(mixed).unwrap();
        assert!(Config::parse(&toml::to_string_pretty(&cfg).unwrap()).is_ok());
        let bad = "[[profile]]\nname='p'\n[[profile.page]]\nname='a'\n[profile.page.keys.0]\naction={toggle=[{profile='nope'}]}";
        assert!(Config::parse(bad).is_err());
    }

    #[test]
    fn obs_actions_and_settings_are_saved() {
        let text = r#"
            [obs]
            port = 4456
            password = "secret"
            [[profile]]
            name = "p"
            [[profile.page]]
            name = "a"
            [profile.page.keys.0]
            action = { obs = "toggle_record" }
            [profile.page.keys.1]
            action = { obs = { scene = "Main" } }
            [profile.page.keys.2]
            action = { obs = { media = { input = "Clip", action = "restart" } } }
            [profile.page.keys.knob1]
            left = { obs = { volume = { input = "Mic", db = -2 } } }
        "#;
        let cfg = Config::parse(text).unwrap();
        assert_eq!(cfg.obs, ObsSettings { host: "127.0.0.1".into(), port: 4456, password: "secret".into() });
        let back = Config::parse(&toml::to_string_pretty(&cfg).unwrap()).unwrap();
        assert_eq!(back.obs, cfg.obs);
        let keys = &back.profiles[0].pages[0].keys;
        assert_eq!(keys[&0].action, Some(Action::Obs(ObsAction::ToggleRecord)));
        assert_eq!(keys[&1].action, Some(Action::Obs(ObsAction::Scene("Main".into()))));
        assert_eq!(keys[&2].action, Some(Action::Obs(ObsAction::Media { input: "Clip".into(), action: MediaAction::Restart })));
        assert_eq!(keys[&17].left, Some(Action::Obs(ObsAction::Volume { input: "Mic".into(), db: -2 })));
    }

    #[test]
    fn rejects_mistakes() {
        let page = "[[profile]]\nname='p'\n[[profile.page]]\nname='a'\n";
        for bad in [
            String::new(),
            "[[profile]]\nname='p'".to_string(),
            "[[profile]]\nname='p'\n[[profile.page]]\nname='a'\n[[profile.page]]\nname='a'".to_string(),
            format!("{page}[[profile]]\nname='p'\n[[profile.page]]\nname='a'"),
            format!("{page}[profile.page.keys.99]\nlabel='x'"),
            format!("{page}[profile.page.keys.0]\nlayer='nope'"),
            format!("{page}[profile.page.keys.0]\nhold={{show='nope'}}"),
            format!("{page}[profile.page.keys.0]\naction={{show='a'}}"),
            format!("{page}[profile.page.keys.0]\nhold={{sequence=[{{show='a'}}]}}"),
            format!("{page}[profile.page.keys.0]\naction={{hotkey='ctrl+nope'}}"),
            format!("{page}[profile.page.keys.0]\ntypo=1"),
            format!("{page}[profile.page.keys.0]\ncolor='red\"/><x'"),
            format!("[[profile]]\nname='p'\ntitle_regex='('\n[[profile.page]]\nname='a'"),
        ] {
            assert!(Config::parse(&bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn profile_conditions_combine() {
        let cfg = Config::parse(
            "[[profile]]\nname='z'\nexe='ZED.exe'\ntitle_regex='^main\\.rs'\ntitle='OPEN'\n[[profile.page]]\nname='a'\n[[profile]]\nname='d'\n[[profile.page]]\nname='a'",
        )
        .unwrap();
        let rule = &cfg.profiles[0];
        assert!(rule.matches(&window(r"C:\Programs\Zed\zed.exe", "main.rs - OpenUlanzi")));
        assert!(!rule.matches(&window(r"C:\Programs\Zed\zed.exe", "lib.rs - OpenUlanzi")));
        assert!(!rule.matches(&window(r"C:\x\vivaldi.exe", "main.rs - OpenUlanzi")));
        assert!(!cfg.profiles[1].matches(&window("any.exe", "any")), "no conditions never match");
        assert_eq!(cfg.default_profile(), 1);
        let marked = "[[profile]]\nname='a'\n[[profile.page]]\nname='p'\n[[profile]]\nname='b'\ndefault=true\nexe='x.exe'\n[[profile.page]]\nname='p'";
        assert_eq!(Config::parse(marked).unwrap().default_profile(), 1, "the mark beats having no conditions");
    }

    #[test]
    fn plugin_actions_survive_a_save_without_the_plugin_installed() {
        let text = "[[profile]]\nname='p'\n[[profile.page]]\nname='a'\n[profile.page.keys.0]\naction={plugin={id='gone',action='go',params={light='Desk'}}}\n[profile.page.keys.1]\naction={sequence=[{plugin={id='gone',action='go'}}]}";
        let cfg = Config::parse(text).unwrap();
        let back = Config::parse(&toml::to_string_pretty(&cfg).unwrap()).unwrap();
        let params = BTreeMap::from([("light".to_string(), "Desk".to_string())]);
        let want = Action::Plugin { id: "gone".into(), action: "go".into(), params };
        assert_eq!(back.profiles[0].pages[0].keys[&0].action, Some(want));
    }

    #[test]
    fn a_profile_cannot_start_with_a_folder() {
        let bad = "[[profile]]\nname='p'\n[[profile.page]]\nname='f'\nfolder=true";
        assert!(Config::parse(bad).is_err());
        let ok = "[[profile]]\nname='p'\n[[profile.page]]\nname='a'\n[profile.page.keys.0]\naction={profile='@app'}\n[profile.page.keys.1]\naction={profile='@default'}";
        assert!(Config::parse(ok).is_ok());
    }
}
