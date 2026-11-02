//! Pages, held layers and tap/hold. Pure: time and the focused window come from the caller.
use crate::config::{Action, Binding, Config, Display, Visual, APP_LAYER, DEFAULT_PROFILE, FOLDER_BACK_KEY, HOLD_STEPS};
use crate::platform::Window;
use crate::protocol::{Input, Knob, WIDE};
use std::collections::HashMap;

const DEBOUNCE_MS: u64 = 25;
/// A quick tap should not cost a screen update, so the hold border starts after this.
const BORDER_DELAY_MS: u64 = 120;

struct Held {
    control: u8,
    binding: Binding,
    /// Profile the binding belongs to, which is where its page names resolve.
    owner: usize,
    layer: Option<Location>,
    /// Holding runs an action, as opposed to showing a page.
    timed: bool,
    /// That action, until the hold completes.
    run: Option<Action>,
    since: u64,
    /// The hold has been decided; a release no longer counts as a tap.
    active: bool,
}

/// A first click waiting to learn whether a second one follows.
struct PendingClick {
    control: u8,
    action: Option<Action>,
    double: Option<Action>,
    owner: usize,
    at: u64,
}

/// (profile, page) indexes.
type Location = (usize, usize);

#[derive(Default)]
struct Switch {
    accepted: bool,
    raw: bool,
    last_edge: u64,
}

pub struct Engine {
    pub cfg: Config,
    profile: usize,
    /// The page each profile was last on.
    pages: Vec<usize>,
    focus_rule: Option<usize>,
    /// The profile matching the focused app, whether or not it switches by itself.
    focused: Option<usize>,
    /// What a folder's fixed key does.
    back: Binding,
    history: Vec<Location>,
    toggles: HashMap<String, usize>,
    held: Vec<Held>,
    pending: Option<PendingClick>,
    /// Where a one-shot switch returns to, with the history length to restore.
    return_to: Option<(Location, usize)>,
    /// A button went down since then, so its release ends the one shot.
    shot_pressed: bool,
    switches: HashMap<u8, Switch>,
    now: u64,
}

impl Engine {
    pub fn new(cfg: Config) -> Self {
        Self {
            profile: cfg.default_profile(),
            pages: vec![0; cfg.profiles.len()],
            cfg,
            focus_rule: None,
            focused: None,
            back: Binding {
                label: "Back".into(),
                icon: "ph:arrow-left-bold".into(),
                action: Some(Action::Back),
                ..Default::default()
            },
            history: Vec::new(),
            toggles: HashMap::new(),
            held: Vec::new(),
            pending: None,
            return_to: None,
            shot_pressed: false,
            switches: HashMap::new(),
            now: 0,
        }
    }

    /// Swap the config, staying on the same profile and page if they still exist.
    pub fn reload(&mut self, cfg: Config) {
        let (profile, page) = (self.profile_name().to_string(), self.page_name().to_string());
        self.profile = cfg.profiles.iter().position(|p| p.name == profile).unwrap_or(cfg.default_profile());
        self.pages = vec![0; cfg.profiles.len()];
        self.pages[self.profile] = cfg.profiles[self.profile].page_index(&page).unwrap_or(0);
        self.cfg = cfg;
        self.focus_rule = None;
        self.focused = None;
        self.history.clear();
        self.release_all();
    }

    /// Forget physical state: the deck went away, so releases will never arrive.
    pub fn release_all(&mut self) {
        self.held.clear();
        self.pending = None;
        self.cancel_one_shot();
        self.switches.clear();
    }

    fn cancel_one_shot(&mut self) {
        self.return_to = None;
        self.shot_pressed = false;
    }

    fn finish_one_shot(&mut self) {
        if !std::mem::take(&mut self.shot_pressed) {
            return;
        }
        if let Some((place, depth)) = self.return_to.take() {
            self.history.truncate(depth);
            self.move_to(place);
        }
    }

    fn current(&self) -> Location {
        (self.profile, self.pages[self.profile])
    }

    pub fn profile_name(&self) -> &str {
        &self.cfg.profiles[self.profile].name
    }

    pub fn page_name(&self) -> &str {
        let (profile, page) = self.current();
        &self.cfg.profiles[profile].pages[page].name
    }

    pub fn active_layers(&self) -> Vec<String> {
        let layers = self.held.iter().filter(|h| h.active).filter_map(|h| h.layer);
        layers.map(|(pr, pa)| self.cfg.profiles[pr].pages[pa].name.clone()).collect()
    }

    /// Follows `on_focus` profiles. Acts only when the matching profile changes, so a page or
    /// profile opened by hand stays put until the user moves to another app.
    pub fn focus_changed(&mut self, window: &Window) {
        self.focused = self.cfg.profiles.iter().position(|p| p.matches(window));
        let rule = self.cfg.profiles.iter().position(|p| p.on_focus && p.matches(window));
        if rule != self.focus_rule {
            self.cancel_one_shot(); // the deck is following the new app now
            self.focus_rule = rule;
            self.profile = rule.unwrap_or_else(|| self.cfg.default_profile());
        }
    }

    /// The page behind a folder: where it was opened from, or the profile's first page.
    fn under_folder(&self, profile: usize) -> usize {
        let pages = &self.cfg.profiles[profile].pages;
        let opened_from = self.history.iter().rev().find(|(pr, pa)| *pr == profile && !pages[*pa].folder);
        opened_from.map_or(0, |(_, page)| *page)
    }

    fn lookup(&self, control: u8) -> Option<(&Binding, usize)> {
        // A held page replaces the deck: its empty keys are empty, not the page underneath.
        let held = self.held.iter().rev().filter(|h| h.active).find_map(|h| h.layer);
        let (profile, mut page) = held.unwrap_or_else(|| self.current());
        if self.cfg.profiles[profile].pages[page].folder {
            if control == FOLDER_BACK_KEY {
                return Some((&self.back, profile));
            }
            if control > WIDE {
                page = self.under_folder(profile); // knobs and side buttons are not the folder's
            }
        }
        self.cfg.profiles[profile].pages[page].keys.get(&control).map(|b| (b, profile))
    }

    fn move_to(&mut self, (profile, page): Location) {
        self.profile = profile;
        self.pages[profile] = page;
    }

    fn step_page(&mut self, forward: bool) {
        let (profile, current) = self.current();
        let pages = &self.cfg.profiles[profile].pages;
        let n = pages.len();
        let offset = |i| if forward { i } else { n - i };
        let candidates = (1..n).map(|i| (current + offset(i)) % n);
        if let Some(p) = candidates.into_iter().find(|p| !pages[*p].hidden && !pages[*p].folder) {
            self.pages[profile] = p;
        }
    }

    fn fire(&mut self, action: Option<Action>, owner: usize, out: &mut Vec<Action>) {
        match action {
            None => {}
            Some(Action::Page(p)) => {
                if let Some(page) = self.cfg.profiles[owner].page_index(&p) {
                    self.history.push(self.current());
                    self.move_to((owner, page));
                }
            }
            Some(Action::Profile(p)) => {
                let target = match p.as_str() {
                    DEFAULT_PROFILE => Some(self.cfg.default_profile()),
                    APP_LAYER => self.focused,
                    name => self.cfg.profiles.iter().position(|q| q.name == name),
                };
                if let Some(profile) = target {
                    self.history.push(self.current());
                    self.profile = profile;
                }
            }
            Some(Action::OneShot(inner)) => {
                let (from, depth) = (self.current(), self.history.len());
                self.fire(Some(*inner), owner, out);
                if self.current() != from {
                    // A one shot opened from a one shot still returns to the very start.
                    self.return_to.get_or_insert((from, depth));
                    self.shot_pressed = false;
                }
            }
            Some(Action::Back) => {
                if let Some(place) = self.history.pop() {
                    self.move_to(place);
                } else {
                    // Reloads clear the history, and a folder must never become a dead end.
                    let (profile, page) = self.current();
                    if self.cfg.profiles[profile].pages[page].folder {
                        self.pages[profile] = self.under_folder(profile);
                    }
                }
            }
            Some(Action::NextPage) => self.step_page(true),
            Some(Action::PrevPage) => self.step_page(false),
            Some(Action::Toggle(list)) if !list.is_empty() => {
                let turn = self.toggles.entry(format!("{list:?}")).or_default();
                let action = list[*turn % list.len()].clone();
                *turn += 1;
                self.fire(Some(action), owner, out);
            }
            Some(Action::Random(list)) if !list.is_empty() => {
                let nanos = std::time::SystemTime::UNIX_EPOCH.elapsed().map_or(0, |d| d.subsec_nanos());
                self.fire(Some(list[nanos as usize % list.len()].clone()), owner, out);
            }
            Some(a) => out.push(a),
        }
    }

    /// Runs a step that arrived from a sequence, returning what the runtime must carry out.
    // shortcut: page steps resolve in the active profile, not the key's; pass the owner along if
    // a sequence ever has to open a page while held from another profile.
    pub fn perform(&mut self, action: Action) -> Vec<Action> {
        let mut out = Vec::new();
        self.fire(Some(action), self.profile, &mut out);
        out
    }

    fn resolve_layer(&self, owner: usize, name: &str, window: &dyn Fn() -> Option<Window>) -> Option<Location> {
        if name != APP_LAYER {
            return self.cfg.profiles[owner].page_index(name).map(|page| (owner, page));
        }
        let w = window()?;
        let profile = self.cfg.profiles.iter().position(|p| p.on_hold && p.matches(&w))?;
        Some((profile, 0))
    }

    /// Settles a waiting click: a second click on the same control makes it a double, returning
    /// true. Any other control means it was a single click after all.
    fn resolve_click(&mut self, control: u8, out: &mut Vec<Action>) -> bool {
        let Some(click) = self.pending.take() else { return false };
        let double = click.control == control;
        self.fire(if double { click.double } else { click.action }, click.owner, out);
        double
    }

    fn press(&mut self, control: u8, now: u64, window: &dyn Fn() -> Option<Window>, out: &mut Vec<Action>) {
        self.shot_pressed |= self.return_to.is_some();
        // Another key during a hold-tap means the user meant hold: the chord uses that page. A
        // hold action keeps counting, because it has to be held on purpose.
        for h in self.held.iter_mut().filter(|h| !h.timed) {
            h.active = true;
        }
        if self.resolve_click(control, out) {
            return;
        }
        let Some((binding, owner)) = self.lookup(control).map(|(b, o)| (b.clone(), o)) else { return };
        let Some(hold) = binding.hold.clone() else {
            return self.click(control, binding, owner, now, out);
        };
        let (layer, run, active) = match hold {
            Action::Show(name) => {
                let layer = self.resolve_layer(owner, &name, window);
                (layer, None, binding.action.is_none() && binding.double.is_none())
            }
            action => (None, Some(action), false),
        };
        let timed = run.is_some();
        self.held.push(Held { control, binding, owner, layer, timed, run, since: now, active });
    }

    fn click(&mut self, control: u8, binding: Binding, owner: usize, now: u64, out: &mut Vec<Action>) {
        if binding.double.is_some() {
            let (action, double) = (binding.action, binding.double);
            self.pending = Some(PendingClick { control, action, double, owner, at: now });
        } else {
            self.fire(binding.action, owner, out);
        }
    }

    fn release(&mut self, control: u8, now: u64, out: &mut Vec<Action>) {
        if let Some(i) = self.held.iter().position(|h| h.control == control) {
            let h = self.held.remove(i);
            if !h.active {
                self.click(control, h.binding, h.owner, now, out);
            }
        }
    }

    fn edge(&mut self, control: u8, down: bool, now: u64, window: &dyn Fn() -> Option<Window>, out: &mut Vec<Action>) {
        if down {
            self.press(control, now, window, out);
        } else {
            self.release(control, now, out);
            self.finish_one_shot();
        }
    }

    fn hold_time(&self, h: &Held) -> u64 {
        let default = if h.timed { self.cfg.hold_action_ms } else { self.cfg.hold_ms };
        h.binding.hold_ms.unwrap_or(default)
    }

    /// Steps of the border for a key on a hold action. It stays full until release.
    fn progress(&self, control: u8) -> u8 {
        let Some(h) = self.held.iter().find(|h| h.control == control && h.timed) else { return 0 };
        if h.run.is_none() {
            return HOLD_STEPS;
        }
        let elapsed = self.now.saturating_sub(h.since).saturating_sub(BORDER_DELAY_MS);
        let span = self.hold_time(h).saturating_sub(BORDER_DELAY_MS).max(1);
        (elapsed * HOLD_STEPS as u64 / span).min(HOLD_STEPS as u64) as u8
    }

    pub fn input(&mut self, input: Input, now: u64, window: &dyn Fn() -> Option<Window>) -> Vec<Action> {
        self.now = now;
        let mut out = Vec::new();
        match input {
            Input::Key { index, down } => {
                let s = self.switches.entry(index).or_default();
                s.raw = down;
                if down != s.accepted && now.saturating_sub(s.last_edge) >= DEBOUNCE_MS {
                    s.accepted = down;
                    s.last_edge = now;
                    self.edge(index, down, now, window, &mut out);
                }
            }
            Input::Knob { index, gesture } => {
                if gesture == Knob::Click {
                    self.shot_pressed |= self.return_to.is_some();
                    if !self.resolve_click(index, &mut out) {
                        if let Some((b, owner)) = self.lookup(index).map(|(b, o)| (b.clone(), o)) {
                            self.click(index, b, owner, now, &mut out);
                        }
                    }
                    self.finish_one_shot();
                    return out;
                }
                let found = self.lookup(index).map(|(b, owner)| {
                    let action = match gesture {
                        Knob::Click => None, // handled above
                        Knob::Left => b.left.clone(),
                        Knob::Right => b.right.clone(),
                        Knob::PressLeft => b.press_left.clone(),
                        Knob::PressRight => b.press_right.clone(),
                    };
                    (action, owner)
                });
                if let Some((action, owner)) = found {
                    self.fire(action, owner, &mut out);
                }
            }
        }
        out
    }

    /// Call often: settles debounced switches and promotes holds past their threshold.
    pub fn tick(&mut self, now: u64, window: &dyn Fn() -> Option<Window>) -> Vec<Action> {
        self.now = now;
        let mut out = Vec::new();
        let settled: Vec<(u8, bool)> = self
            .switches
            .iter_mut()
            .filter(|(_, s)| s.raw != s.accepted && now.saturating_sub(s.last_edge) >= DEBOUNCE_MS)
            .map(|(i, s)| {
                s.accepted = s.raw;
                s.last_edge = now;
                (*i, s.raw)
            })
            .collect();
        for (control, down) in settled {
            self.edge(control, down, now, window, &mut out);
        }
        let expired = self.pending.as_ref().is_some_and(|c| now.saturating_sub(c.at) >= self.cfg.double_ms);
        if expired {
            // No control has this index, so the click resolves as a single one.
            self.resolve_click(u8::MAX, &mut out);
        }
        let done: Vec<usize> = (0..self.held.len())
            .filter(|i| now.saturating_sub(self.held[*i].since) >= self.hold_time(&self.held[*i]))
            .collect();
        for i in done {
            self.held[i].active = true;
            if let Some(action) = self.held[i].run.take() {
                let owner = self.held[i].owner;
                self.fire(Some(action), owner, &mut out);
            }
        }
        out
    }

    fn position(&self) -> String {
        let (profile, page) = self.current();
        let pages = &self.cfg.profiles[profile].pages;
        let visible: Vec<usize> = (0..pages.len()).filter(|p| !pages[*p].hidden && !pages[*p].folder).collect();
        match visible.iter().position(|p| *p == page) {
            Some(i) => format!("{}/{}", i + 1, visible.len()),
            None => self.page_name().to_string(),
        }
    }

    /// What every screen should show right now.
    pub fn view(&self) -> Vec<(u8, Visual)> {
        (0..=WIDE)
            .map(|i| {
                let binding = self.lookup(i).map(|(b, _)| b);
                let mut visual = binding.map_or_else(|| self.cfg.blank(), |b| self.cfg.visual(b));
                if i == WIDE {
                    visual.display = self.cfg.wide_display(binding);
                }
                if visual.display == Display::PageIndicator {
                    visual.big = self.position();
                }
                // Firmware faces cannot be drawn on, so they just run when the hold completes.
                if visual.display.mode() == Display::Image.mode() {
                    visual.progress = self.progress(i);
                }
                (i, visual)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CFG: &str = r#"
        [[page]]
        name = "main"
        [page.keys.0]
        label = "mod"
        layer = "fn"
        [page.keys.1]
        label = "tap"
        action = { open = "tap" }
        layer = "fn"
        [page.keys.2]
        label = "app"
        layer = "@app"
        [page.keys.3]
        label = "base"
        action = { open = "base" }
        [page.keys.4]
        action = "next_page"
        [page.keys.knob1]
        left = { open = "base-left" }

        [[page]]
        name = "fn"
        [page.keys.3]
        label = "layer"
        action = { open = "layer" }
        [page.keys.knob1]
        left = { open = "layer-left" }

        [[page]]
        name = "zed"
        hidden = true
        [page.keys.3]
        action = { open = "zed" }

        [[app]]
        page = "zed"
        exe = "zed.exe"
    "#;

    const FOLDER: &str = r#"
        [[profile]]
        name = "Default"
        [[profile.page]]
        name = "main"
        [profile.page.keys.0]
        action = { page = "folder" }
        [profile.page.keys.1]
        action = "back"
        [profile.page.keys.2]
        action = { toggle = [{ open = "one" }, { open = "two" }] }
        [profile.page.keys.3]
        action = { profile = "Default" }
        [profile.page.keys.4]
        display = "page_indicator"
        [profile.page.keys.wide]
        label = "W"
        layer = "folder"

        [[profile.page]]
        name = "folder"
        [profile.page.keys.0]
        action = "back"
        [profile.page.keys.1]
        action = "back"
        [profile.page.keys.wide]
        display = "dial"

        [[profile]]
        name = "Zed"
        exe = "zed.exe"
        [[profile.page]]
        name = "edit"
        [profile.page.keys.5]
        action = "next_page"
        [[profile.page]]
        name = "git"
        [profile.page.keys.3]
        action = { profile = "Default" }
        [profile.page.keys.5]
        action = "next_page"

        [[profile]]
        name = "Vlc"
        exe = "vlc.exe"
        on_focus = false
        [[profile.page]]
        name = "player"
        [profile.page.keys.6]
        action = { page = "player" }
    "#;

    const DOUBLE: &str = r#"
        [[profile]]
        name = "Default"
        [[profile.page]]
        name = "main"
        [profile.page.keys.0]
        action = { open = "one" }
        double = { open = "two" }
        [profile.page.keys.1]
        action = { open = "other" }
        [profile.page.keys.2]
        action = { open = "tap" }
        double = { open = "double" }
        layer = "held"
        [profile.page.keys.3]
        double = { open = "only" }
        layer = "held"
        [profile.page.keys.knob1]
        action = { open = "knob one" }
        double = { open = "knob two" }
        [[profile.page]]
        name = "held"
        [profile.page.keys.5]
        label = "held"
    "#;

    const FOLDERS: &str = r#"
        [[profile]]
        name = "Zed"
        exe = "zed.exe"
        [[profile.page]]
        name = "edit"
        [profile.page.keys.1]
        action = { profile = "@default" }
        [[profile]]
        name = "Home"
        default = true
        exe = "never.exe"
        [[profile.page]]
        name = "one"
        [profile.page.keys.0]
        label = "first"
        action = { page = "stuff" }
        [profile.page.keys.1]
        action = { profile = "@app" }
        [profile.page.keys.2]
        action = "next_page"
        [profile.page.keys.knob1]
        left = { open = "one left" }
        [profile.page.keys.side1]
        action = { open = "one side" }
        [[profile.page]]
        name = "two"
        [profile.page.keys.0]
        action = { page = "stuff" }
        [profile.page.keys.2]
        action = "next_page"
        [profile.page.keys.knob1]
        left = { open = "two left" }
        [profile.page.keys.side1]
        action = { open = "two side" }
        [[profile.page]]
        name = "stuff"
        folder = true
        [profile.page.keys.0]
        label = "ignored"
        action = { open = "ignored" }
        [profile.page.keys.1]
        label = "inside"
        action = { open = "inside" }
        [profile.page.keys.knob1]
        left = { open = "folder left" }
        [profile.page.keys.side1]
        action = { open = "folder side" }
    "#;

    fn folders() -> Engine {
        Engine::new(Config::parse(FOLDERS).unwrap())
    }

    #[test]
    fn folder_always_has_back_and_returns_to_the_page_it_came_from() {
        let mut e = folders();
        assert_eq!(e.profile_name(), "Home", "the marked default wins");
        key(&mut e, 2, true, 100);
        key(&mut e, 2, false, 200);
        assert_eq!(e.page_name(), "two");
        key(&mut e, 0, true, 300);
        key(&mut e, 0, false, 400);
        assert_eq!(e.page_name(), "stuff");
        assert_eq!(label(&e, 0), "Back", "the fixed key beats what is stored there");
        assert_eq!(label(&e, 1), "inside");
        assert_eq!(key(&mut e, 0, true, 500), vec![]);
        assert_eq!(e.page_name(), "two");
    }

    #[test]
    fn folder_leaves_knobs_and_side_buttons_alone() {
        let mut e = folders();
        key(&mut e, 2, true, 100);
        key(&mut e, 2, false, 200);
        key(&mut e, 0, true, 300);
        key(&mut e, 0, false, 400);
        let left = Input::Knob { index: 17, gesture: Knob::Left };
        assert_eq!(e.input(left, 450, &none), open("two left"));
        assert_eq!(key(&mut e, 15, true, 500), open("two side"));
    }

    #[test]
    fn next_page_skips_folders_and_back_is_never_a_dead_end() {
        let mut e = folders();
        key(&mut e, 2, true, 100);
        key(&mut e, 2, false, 200);
        key(&mut e, 2, true, 300);
        key(&mut e, 2, false, 400);
        assert_eq!(e.page_name(), "one", "wraps past the folder");
        key(&mut e, 0, true, 500);
        key(&mut e, 0, false, 600);
        e.reload(Config::parse(FOLDERS).unwrap());
        assert_eq!(e.page_name(), "stuff");
        assert_eq!(key(&mut e, 0, true, 700), vec![]);
        assert_eq!(e.page_name(), "one");
    }

    #[test]
    fn profile_actions_reach_the_app_and_default_profiles() {
        let mut e = folders();
        let zed = Window { exe: "zed.exe".into(), title: String::new() };
        key(&mut e, 1, true, 100);
        key(&mut e, 1, false, 200);
        assert_eq!(e.profile_name(), "Home", "no app matches yet, so it stays");
        e.focus_changed(&zed);
        assert_eq!(e.profile_name(), "Zed");
        key(&mut e, 1, true, 300);
        key(&mut e, 1, false, 400);
        assert_eq!(e.profile_name(), "Home", "@default");
        key(&mut e, 1, true, 500);
        assert_eq!(e.profile_name(), "Zed", "@app finds the focused app without a focus change");
    }

    const SHOTS: &str = r#"
        [[profile]]
        name = "Home"
        [[profile.page]]
        name = "start"
        [profile.page.keys.0]
        action = { one_shot = { page = "pick" } }
        [profile.page.keys.1]
        action = { one_shot = { profile = "Tools" } }
        [profile.page.keys.2]
        action = { one_shot = { page = "start" } }
        [profile.page.keys.3]
        action = { page = "pick" }
        [[profile.page]]
        name = "pick"
        [profile.page.keys.5]
        action = { open = "picked" }
        [profile.page.keys.6]
        action = { one_shot = { page = "more" } }
        [profile.page.keys.knob1]
        left = { open = "turned" }
        action = { open = "clicked" }
        [profile.page.keys.7]
        hold = { show = "more" }
        [[profile.page]]
        name = "more"
        [profile.page.keys.5]
        action = { open = "deep" }
        [[profile]]
        name = "Tools"
        exe = "tools.exe"
        on_focus = false
        [[profile.page]]
        name = "tools"
        [profile.page.keys.5]
        action = { open = "tool" }
    "#;

    fn shots() -> Engine {
        Engine::new(Config::parse(SHOTS).unwrap())
    }
    fn tap(e: &mut Engine, index: u8, at: u64) -> Vec<Action> {
        let out = key(e, index, true, at);
        key(e, index, false, at + 50);
        out
    }

    #[test]
    fn one_shot_page_returns_after_the_next_press() {
        let mut e = shots();
        tap(&mut e, 0, 100);
        assert_eq!(e.page_name(), "pick", "releasing the key that opened it changes nothing");
        assert_eq!(key(&mut e, 5, true, 300), open("picked"));
        assert_eq!(e.page_name(), "pick", "still there until the release");
        key(&mut e, 5, false, 350);
        assert_eq!(e.page_name(), "start");
        tap(&mut e, 5, 500);
        assert_eq!(e.page_name(), "start", "a later press changes nothing");
    }

    #[test]
    fn one_shot_profile_returns_to_profile_and_page() {
        let mut e = shots();
        tap(&mut e, 1, 100);
        assert_eq!(e.profile_name(), "Tools");
        assert_eq!(tap(&mut e, 5, 300), open("tool"));
        assert_eq!((e.profile_name(), e.page_name()), ("Home", "start"));
    }

    #[test]
    fn pressing_an_empty_key_also_ends_it() {
        let mut e = shots();
        tap(&mut e, 0, 100);
        tap(&mut e, 12, 300);
        assert_eq!(e.page_name(), "start");
    }

    #[test]
    fn knob_turn_keeps_the_page_and_knob_click_ends_it() {
        let mut e = shots();
        tap(&mut e, 0, 100);
        let turn = Input::Knob { index: 17, gesture: Knob::Left };
        assert_eq!(e.input(turn, 300, &none), open("turned"));
        assert_eq!(e.page_name(), "pick");
        let click = Input::Knob { index: 17, gesture: Knob::Click };
        assert_eq!(e.input(click, 400, &none), open("clicked"));
        assert_eq!(e.page_name(), "start");
    }

    #[test]
    fn chained_one_shots_return_to_the_very_start() {
        let mut e = shots();
        tap(&mut e, 0, 100);
        tap(&mut e, 6, 300);
        assert_eq!(e.page_name(), "more", "the second one shot is not the press that ends the first");
        assert_eq!(tap(&mut e, 5, 500), open("deep"));
        assert_eq!(e.page_name(), "start");
    }

    #[test]
    fn a_hold_on_a_one_shot_page_returns_on_release() {
        let mut e = shots();
        tap(&mut e, 0, 100);
        key(&mut e, 7, true, 300);
        assert_eq!(e.active_layers(), ["more"]);
        key(&mut e, 7, false, 900);
        assert_eq!(e.page_name(), "start");
        assert!(e.active_layers().is_empty());
    }

    #[test]
    fn plain_page_switch_stays_and_one_shot_to_the_same_page_does_nothing() {
        let mut e = shots();
        tap(&mut e, 2, 100);
        tap(&mut e, 3, 300);
        tap(&mut e, 5, 500);
        assert_eq!(e.page_name(), "pick", "off by default, so nothing returns");
    }

    #[test]
    fn switching_apps_cancels_the_return() {
        let cfg = SHOTS.replace("on_focus = false\n", "");
        let mut e = Engine::new(Config::parse(&cfg).unwrap());
        tap(&mut e, 0, 100);
        e.focus_changed(&Window { exe: "tools.exe".into(), title: String::new() });
        assert_eq!(e.profile_name(), "Tools");
        tap(&mut e, 5, 300);
        assert_eq!(e.profile_name(), "Tools", "no jump back from under the new app");
    }

    fn holder() -> Engine {
        let cfg = r#"
            hold_action_ms = 600
            [[profile]]
            name = "Default"
            [[profile.page]]
            name = "main"
            [profile.page.keys.0]
            hold = { open = "held" }
            [profile.page.keys.1]
            action = { open = "tap" }
            hold = { open = "held" }
            hold_ms = 1000
            [profile.page.keys.2]
            action = { open = "other" }
            [profile.page.keys.3]
            hold = { page = "second" }
            [[profile.page]]
            name = "second"
        "#;
        Engine::new(Config::parse(cfg).unwrap())
    }
    fn progress(e: &Engine, i: u8) -> u8 {
        e.view().into_iter().find(|(c, _)| *c == i).unwrap().1.progress
    }

    #[test]
    fn hold_action_runs_once_when_the_time_is_up() {
        let mut e = holder();
        assert_eq!(key(&mut e, 0, true, 100), vec![]);
        assert_eq!(e.tick(699, &none), vec![]);
        assert_eq!(e.tick(700, &none), open("held"));
        assert_eq!(e.tick(900, &none), vec![], "only once per hold");
        assert_eq!(key(&mut e, 0, false, 1000), vec![]);
    }

    #[test]
    fn releasing_early_cancels_and_a_tap_still_works() {
        let mut e = holder();
        key(&mut e, 1, true, 100);
        e.tick(500, &none);
        assert!(progress(&e, 1) > 0);
        assert_eq!(key(&mut e, 1, false, 600), open("tap"));
        assert_eq!(progress(&e, 1), 0, "the border goes away");
        assert_eq!(e.tick(5000, &none), vec![]);
    }

    #[test]
    fn border_waits_then_fills_with_the_hold() {
        let mut e = holder();
        key(&mut e, 0, true, 100);
        e.tick(200, &none);
        assert_eq!(progress(&e, 0), 0, "a quick tap would cost no screen update");
        e.tick(460, &none);
        assert_eq!(progress(&e, 0), 8, "half way");
        e.tick(700, &none);
        assert_eq!(progress(&e, 0), HOLD_STEPS, "finishes complete");
        e.tick(2000, &none);
        assert_eq!(progress(&e, 0), HOLD_STEPS, "and stays that way while held");
        key(&mut e, 0, false, 2100);
        assert_eq!(progress(&e, 0), 0, "release clears it");
    }

    #[test]
    fn another_key_does_not_rush_a_hold_action() {
        let mut e = holder();
        key(&mut e, 0, true, 100);
        assert_eq!(key(&mut e, 2, true, 200), open("other"));
        assert_eq!(e.tick(300, &none), vec![]);
        assert_eq!(e.tick(700, &none), open("held"));
    }

    #[test]
    fn hold_can_open_a_page_for_good() {
        let mut e = holder();
        key(&mut e, 3, true, 100);
        e.tick(700, &none);
        assert_eq!(e.page_name(), "second");
        key(&mut e, 3, false, 800);
        assert_eq!(e.page_name(), "second");
    }

    fn double() -> Engine {
        Engine::new(Config::parse(DOUBLE).unwrap())
    }

    #[test]
    fn double_click_runs_only_the_double_action() {
        let mut e = double();
        assert_eq!(key(&mut e, 0, true, 100), vec![], "the first click waits");
        key(&mut e, 0, false, 150);
        assert_eq!(key(&mut e, 0, true, 250), open("two"));
        key(&mut e, 0, false, 300);
        assert_eq!(e.tick(1000, &none), vec![], "the single action never runs");
    }

    #[test]
    fn single_click_runs_once_the_wait_ends() {
        let mut e = double();
        key(&mut e, 0, true, 100);
        key(&mut e, 0, false, 150);
        assert_eq!(e.tick(349, &none), vec![]);
        assert_eq!(e.tick(350, &none), open("one"));
        assert_eq!(key(&mut e, 0, true, 500), vec![], "and the next click starts over");
    }

    #[test]
    fn another_key_settles_the_click_at_once_and_in_order() {
        let mut e = double();
        key(&mut e, 0, true, 100);
        key(&mut e, 0, false, 150);
        assert_eq!(key(&mut e, 1, true, 200), vec![Action::Open("one".into()), Action::Open("other".into())]);
    }

    #[test]
    fn keys_without_a_double_stay_instant() {
        assert_eq!(key(&mut double(), 1, true, 100), open("other"));
    }

    #[test]
    fn tap_hold_and_double_share_a_key() {
        let mut e = double();
        key(&mut e, 2, true, 100);
        key(&mut e, 2, false, 150);
        assert_eq!(e.tick(160, &none), vec![]);
        assert_eq!(key(&mut e, 2, true, 250), open("double"));
        key(&mut e, 2, false, 300);

        key(&mut e, 2, true, 1000);
        key(&mut e, 2, false, 1050);
        assert_eq!(e.tick(1300, &none), open("tap"));

        key(&mut e, 2, true, 2000);
        e.tick(2201, &none);
        assert_eq!(label(&e, 5), "held");
        assert_eq!(key(&mut e, 2, false, 2300), vec![]);
        assert_eq!(e.tick(3000, &none), vec![], "a hold is not a click");
    }

    #[test]
    fn a_hold_key_with_only_a_double_still_holds() {
        let mut e = double();
        key(&mut e, 3, true, 100);
        assert_eq!(label(&e, 5), "", "it waits to rule out a double click");
        e.tick(301, &none);
        assert_eq!(label(&e, 5), "held");
        key(&mut e, 3, false, 400);
        key(&mut e, 3, true, 1000);
        key(&mut e, 3, false, 1050);
        assert_eq!(key(&mut e, 3, true, 1150), open("only"));
    }

    #[test]
    fn knobs_double_click_too() {
        let mut e = double();
        let click = Input::Knob { index: 17, gesture: Knob::Click };
        assert_eq!(e.input(click, 100, &none), vec![]);
        assert_eq!(e.input(click, 200, &none), open("knob two"));
        assert_eq!(e.input(click, 500, &none), vec![]);
        assert_eq!(e.tick(800, &none), open("knob one"));
    }

    fn engine() -> Engine {
        Engine::new(Config::parse(CFG).unwrap())
    }
    fn none() -> Option<Window> {
        None
    }
    fn zed() -> Option<Window> {
        Some(Window { exe: r"C:\Zed\zed.exe".into(), title: String::new() })
    }
    fn key(e: &mut Engine, index: u8, down: bool, now: u64) -> Vec<Action> {
        e.input(Input::Key { index, down }, now, &none)
    }
    fn open(s: &str) -> Vec<Action> {
        vec![Action::Open(s.into())]
    }
    fn label(e: &Engine, i: u8) -> String {
        e.view().into_iter().find(|(c, _)| *c == i).unwrap().1.label
    }

    #[test]
    fn pure_modifier_shows_layer_at_once_and_restores() {
        let mut e = engine();
        key(&mut e, 0, true, 100);
        assert_eq!(label(&e, 3), "layer");
        assert_eq!(key(&mut e, 3, true, 200), open("layer"));
        key(&mut e, 0, false, 300);
        assert_eq!(label(&e, 3), "base");
        assert_eq!(key(&mut e, 3, false, 310), vec![]);
    }

    #[test]
    fn knob_turns_follow_the_held_layer() {
        let mut e = engine();
        let left = Input::Knob { index: 17, gesture: Knob::Left };
        key(&mut e, 0, true, 100);
        assert_eq!(e.input(left, 110, &none), open("layer-left"));
    }

    #[test]
    fn held_page_replaces_the_deck() {
        let mut e = engine();
        key(&mut e, 0, true, 100);
        assert_eq!(label(&e, 2), "", "a key the held page leaves empty shows nothing");
        assert_eq!(key(&mut e, 4, true, 200), vec![], "and does nothing");
        assert_eq!(e.page_name(), "main");
    }

    #[test]
    fn hold_tap_tap_runs_action_without_flashing_layer() {
        let mut e = engine();
        key(&mut e, 1, true, 100);
        e.tick(150, &none);
        assert_eq!(label(&e, 3), "base");
        assert_eq!(key(&mut e, 1, false, 180), open("tap"));
    }

    #[test]
    fn hold_tap_hold_shows_layer_and_skips_action() {
        let mut e = engine();
        key(&mut e, 1, true, 100);
        e.tick(301, &none);
        assert_eq!(label(&e, 3), "layer");
        assert_eq!(key(&mut e, 1, false, 500), vec![]);
    }

    #[test]
    fn chord_before_threshold_counts_as_hold() {
        let mut e = engine();
        key(&mut e, 1, true, 100);
        assert_eq!(key(&mut e, 3, true, 150), open("layer"));
        assert_eq!(key(&mut e, 1, false, 180), vec![]);
    }

    #[test]
    fn app_layer_resolves_at_press_and_survives_focus_change() {
        let mut e = engine();
        e.input(Input::Key { index: 2, down: true }, 100, &zed);
        // Focus is only read on press; the second key reports no window at all.
        assert_eq!(key(&mut e, 3, true, 200), open("zed"));
        key(&mut e, 2, false, 300);
        assert_eq!(key(&mut e, 3, false, 400), vec![]);
        assert_eq!(key(&mut e, 3, true, 500), open("base"));
    }

    #[test]
    fn app_layer_with_no_match_changes_nothing() {
        let mut e = engine();
        key(&mut e, 2, true, 100);
        assert_eq!(key(&mut e, 3, true, 200), open("base"));
    }

    #[test]
    fn bounce_fires_once_and_short_tap_still_releases() {
        let mut e = engine();
        assert_eq!(key(&mut e, 3, true, 1000), open("base"));
        key(&mut e, 3, false, 1007);
        assert_eq!(key(&mut e, 3, true, 1015), vec![]);
        e.tick(1040, &none);
        key(&mut e, 3, false, 1100);

        // A real 10 ms tap on a modifier must not leave the layer stuck.
        key(&mut e, 0, true, 2000);
        key(&mut e, 0, false, 2010);
        assert_eq!(label(&e, 3), "layer");
        e.tick(2030, &none);
        assert_eq!(label(&e, 3), "base");
    }

    #[test]
    fn folder_page_goes_back_where_it_came_from() {
        let mut e = Engine::new(Config::parse(FOLDER).unwrap());
        assert_eq!(key(&mut e, 0, true, 100), vec![]);
        assert_eq!(e.page_name(), "folder");
        key(&mut e, 0, false, 200);
        key(&mut e, 1, true, 300);
        assert_eq!(e.page_name(), "main");
        key(&mut e, 1, false, 400);
        key(&mut e, 1, true, 500);
        assert_eq!(e.page_name(), "main", "back with no history stays");
    }

    #[test]
    fn focused_app_switches_profile_and_hand_navigation_sticks() {
        let mut e = Engine::new(Config::parse(FOLDER).unwrap());
        let app = |exe: &str| Window { exe: exe.into(), title: String::new() };
        e.focus_changed(&app("zed.exe"));
        assert_eq!((e.profile_name(), e.page_name()), ("Zed", "edit"));
        key(&mut e, 5, true, 100);
        assert_eq!(e.page_name(), "git", "profiles flip through their own pages");
        key(&mut e, 5, false, 200);
        e.focus_changed(&app("other.exe"));
        assert_eq!(e.profile_name(), "Default");
        e.focus_changed(&app("zed.exe"));
        assert_eq!(e.page_name(), "git", "a profile remembers its page");
        key(&mut e, 3, true, 600);
        e.focus_changed(&app("zed.exe"));
        assert_eq!(e.profile_name(), "Default", "profile opened by hand survives the next poll");
        e.focus_changed(&app("vlc.exe"));
        assert_eq!(e.profile_name(), "Default", "on_focus = false leaves the deck alone");
    }

    #[test]
    fn holding_for_the_app_borrows_its_first_page() {
        let cfg = FOLDER.replace(
            "keys.1]\n        action = \"back\"\n        [profile.page.keys.2]",
            "keys.1]\n        layer = \"@app\"\n        [profile.page.keys.2]",
        );
        let mut e = Engine::new(Config::parse(&cfg).unwrap());
        let vlc = || Some(Window { exe: "vlc.exe".into(), title: String::new() });
        e.input(Input::Key { index: 1, down: true }, 100, &vlc);
        assert_eq!(e.active_layers(), ["player"]);
        assert_eq!(e.profile_name(), "Default");
        key(&mut e, 6, true, 200);
        assert_eq!(e.profile_name(), "Vlc", "a page action opens in the profile the key belongs to");
    }

    #[test]
    fn wide_key_holds_and_follows_its_binding() {
        let mut e = Engine::new(Config::parse(FOLDER).unwrap());
        let wide = |e: &Engine| e.view().into_iter().find(|(c, _)| *c == WIDE).unwrap().1;
        assert_eq!(wide(&e).display, Display::Image, "a label makes it an image");
        assert_eq!(wide(&e).label, "W");
        key(&mut e, WIDE, true, 100);
        assert_eq!(e.page_name(), "main");
        assert_eq!(wide(&e).display, Display::Dial, "layer shows its own display");
        key(&mut e, WIDE, false, 200);
        assert_eq!(wide(&e).display, Display::Image);
    }

    #[test]
    fn toggle_alternates_and_indicator_counts_visible_pages() {
        let mut e = Engine::new(Config::parse(FOLDER).unwrap());
        assert_eq!(key(&mut e, 2, true, 100), open("one"));
        key(&mut e, 2, false, 200);
        assert_eq!(key(&mut e, 2, true, 300), open("two"));
        key(&mut e, 2, false, 400);
        assert_eq!(key(&mut e, 2, true, 500), open("one"));
        assert_eq!(e.view()[4].1.big, "1/2");
    }

    #[test]
    fn page_navigation_stays_inside_the_engine() {
        let mut e = engine();
        assert_eq!(key(&mut e, 4, true, 100), vec![]);
        assert_eq!(label(&e, 3), "layer");
        assert_eq!(e.view().len(), 14);
        e.step_page(true);
        assert_eq!(label(&e, 3), "base", "hidden page is skipped when wrapping around");
        e.step_page(false);
        assert_eq!(label(&e, 3), "layer", "and when going backwards");
    }
}
