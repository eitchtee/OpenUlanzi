//! The deck loop, shared by the GUI and the headless runner.
use crate::config::{Action, Config, Display, Visual};
use crate::device::Deck;
use crate::engine::Engine;
use crate::protocol::{self, CMD_BRIGHTNESS, CMD_LOCK, CMD_SMALL_WINDOW, CMD_UNLOCK, WIDE};
use crate::{platform, render::Renderer};
use anyhow::Result;
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime};

const KEEP_ALIVE: Duration = Duration::from_secs(5);
const HOUSEKEEPING: Duration = Duration::from_secs(1);
const LOCK_CHECK: Duration = Duration::from_millis(100);
/// Pressing the deck can't turn the screens on, so it stays usable this long without them.
const DECK_ONLY_AWAKE: Duration = Duration::from_secs(30);
const STATS_REFRESH: Duration = Duration::from_secs(2);
/// Longest wait for a bundle ack, so one lost ack cannot freeze the screens.
const ACK_TIMEOUT: Duration = Duration::from_millis(300);

/// What the deck is doing, for anyone watching it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Status {
    /// Which deck this is about; empty for the headless runner.
    pub device: String,
    pub connected: bool,
    pub locked: bool,
    /// Dark because nobody is at the PC, as opposed to locked.
    pub asleep: bool,
    pub profile: String,
    pub page: String,
    /// Layers currently held, oldest first.
    pub layers: Vec<String>,
    pub view: Vec<(u8, Visual)>,
    pub volume: Option<VolumePopup>,
}

/// The latest app volume change, for the on-screen popup.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct VolumePopup {
    /// Grows with every change, so the same level twice still shows.
    pub id: u64,
    pub exe: String,
    /// Percent, or None when the app has no audio session yet.
    pub level: Option<u8>,
}

/// Popup ids are shared by all decks, so the newest change is always the highest.
static POPUP_ID: AtomicU64 = AtomicU64::new(0);

struct App<F> {
    device: Option<String>,
    path: PathBuf,
    modified: Option<SystemTime>,
    engine: Engine,
    renderer: Renderer,
    started: Instant,
    brightness: u8,
    locked: bool,
    asleep: bool,
    last_deck_input: Instant,
    wake: Arc<AtomicBool>,
    on_status: F,
    published: Option<Status>,
    steps: (Sender<Action>, Receiver<Action>),
    volume: Option<VolumePopup>,
    wide: Option<Display>,
    /// A bundle the firmware may still be unpacking.
    in_flight: Option<Instant>,
    /// The wide screen needs its small-window command once that bundle lands.
    wide_due: bool,
    wide_date: String,
    last_stats: Instant,
    stats: platform::Stats,
}

fn modified(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

fn set_brightness(deck: &mut Deck, value: u8) -> Result<()> {
    deck.command(CMD_BRIGHTNESS, value.to_string().as_bytes())
}

impl<F: FnMut(&Status)> App<F> {
    fn now(&self) -> u64 {
        self.started.elapsed().as_millis() as u64
    }

    fn publish(&mut self, connected: bool) {
        let status = Status {
            device: self.device.clone().unwrap_or_default(),
            connected,
            locked: self.locked,
            asleep: self.asleep,
            profile: self.engine.profile_name().to_string(),
            page: self.engine.page_name().to_string(),
            layers: self.engine.active_layers(),
            view: self.engine.view(),
            volume: self.volume.clone(),
        };
        if self.published.as_ref() != Some(&status) {
            (self.on_status)(&status);
            self.published = Some(status);
        }
    }

    fn change_app_volume(&mut self, change: impl Fn(&str) -> Result<Option<u8>>) -> Result<()> {
        let Some(window) = platform::foreground_window() else { return Ok(()) };
        let level = change(&window.exe)?;
        let id = POPUP_ID.fetch_add(1, Ordering::Relaxed) + 1;
        self.volume = Some(VolumePopup { id, exe: window.exe, level });
        Ok(())
    }

    fn run_action(&mut self, deck: &mut Deck, action: Action) -> Result<()> {
        // An action failing is the user's to read about, not a reason to drop the deck.
        let outcome = match &action {
            Action::Hotkey(h) => platform::send_hotkey(h),
            Action::Open(target) => platform::open(target),
            Action::Brightness(delta) => {
                self.brightness = (self.brightness as i32 + delta).clamp(0, 100) as u8;
                return set_brightness(deck, self.brightness);
            }
            Action::Text(text) => platform::send_text(text),
            Action::ScrollUp(notches) => platform::scroll(*notches as i32, false),
            Action::ScrollDown(notches) => platform::scroll(-(*notches as i32), false),
            Action::ScrollLeft(notches) => platform::scroll(-(*notches as i32), true),
            Action::ScrollRight(notches) => platform::scroll(*notches as i32, true),
            Action::Close(program) => platform::close(program),
            Action::AppVolume(percent) => self.change_app_volume(|exe| platform::step_app_volume(exe, *percent)),
            Action::AppVolumeReset => self.change_app_volume(platform::reset_app_volume),
            Action::PlaySound(path) => platform::play_sound(path),
            Action::StopSound => {
                platform::stop_sound();
                Ok(())
            }
            Action::Sequence(steps) => {
                // Delays must not stall the deck, so only the waiting happens on this thread.
                let (steps, sender) = (steps.clone(), self.steps.0.clone());
                std::thread::spawn(move || {
                    for step in steps {
                        if let Action::Delay(ms) = step {
                            std::thread::sleep(Duration::from_millis(ms));
                        } else if sender.send(step).is_err() {
                            return;
                        }
                    }
                });
                Ok(())
            }
            _ => Ok(()), // pages, toggles and the like end in the engine
        };
        if let Err(e) = outcome {
            eprintln!("action failed: {e:#}");
        }
        Ok(())
    }

    /// Repaint only cells that differ from what the deck shows, one bundle at a time: anything that
    /// changes meanwhile is picked up by the next call, so the latest state wins.
    fn paint(&mut self, deck: &mut Deck, painted: &mut HashMap<u8, Visual>) -> Result<()> {
        if let Some(sent) = self.in_flight {
            if !deck.acked && sent.elapsed() < ACK_TIMEOUT {
                return Ok(());
            }
            self.in_flight = None;
            if std::mem::take(&mut self.wide_due) {
                self.send_wide(deck)?;
            }
        }
        let black = Visual { background: "#000000".into(), ..Default::default() };
        let mut view: HashMap<u8, Visual> = self.engine.view().into_iter().collect();
        let mut wide = view.remove(&WIDE).unwrap_or_default();
        if self.locked {
            view.values_mut().for_each(|v| *v = black.clone());
            wide = painted.get(&WIDE).cloned().unwrap_or(wide);
        }
        let changed: Vec<(u8, Visual)> = view.into_iter().filter(|(i, v)| painted.get(i) != Some(v)).collect();
        if changed.is_empty() && painted.get(&WIDE) == Some(&wide) {
            return Ok(());
        }

        let mut pngs = Vec::with_capacity(changed.len());
        for (index, visual) in &changed {
            pngs.push((*index, self.renderer.png(visual, false)?.to_vec()));
        }
        let cells: Vec<(u8, &[u8])> = pngs.iter().map(|(i, p)| (*i, p.as_slice())).collect();
        let image = match wide.display.mode() {
            2 => Some(self.renderer.png(&wide, true)?.to_vec()),
            _ => None,
        };
        let cell = protocol::Wide { mode: wide.display.mode(), image: image.as_deref() };
        deck.bundle(&protocol::build_bundle(&cells, cell)?, painted.is_empty())?;
        self.in_flight = Some(Instant::now());
        // A firmware face has to be re-sent after any bundle, since the bundle names its cell.
        self.wide_due = image.is_none() || self.wide != Some(wide.display);
        self.wide = Some(wide.display);
        painted.extend(changed);
        painted.insert(WIDE, wide);
        Ok(())
    }

    fn clock(&self) -> platform::Now {
        let cfg = &self.engine.cfg;
        platform::now(&cfg.date_format, &cfg.weekday_format)
    }

    fn send_wide(&mut self, deck: &mut Deck) -> Result<()> {
        let display = self.wide.unwrap_or_default();
        let now = self.clock();
        let suffix = match display {
            Display::DateTimeWeekday => format!("{} {}", now.date, now.weekday),
            Display::TimeWeekday => now.weekday.clone(),
            Display::TimeDate => now.date.clone(),
            _ => String::new(),
        };
        let (cpu, ram) = self.stats.sample();
        let twelve = self.engine.cfg.twelve_hour;
        self.wide_date = now.date;
        self.last_stats = Instant::now();
        deck.command(CMD_SMALL_WINDOW, &protocol::small_window(display.mode(), cpu, ram, &now.time, twelve, &suffix))
    }

    fn reload_if_changed(&mut self) {
        let stamp = modified(&self.path);
        if stamp == self.modified {
            return;
        }
        self.modified = stamp;
        match Config::load(&self.path) {
            Ok(cfg) => {
                self.renderer.forget_images(); // an icon file may have changed behind the same path
                // Only a changed setting applies, so saving anything else keeps what the knobs set.
                if cfg.brightness != self.engine.cfg.brightness {
                    self.brightness = cfg.brightness;
                }
                self.engine.reload(cfg);
                println!("config reloaded");
            }
            Err(e) => eprintln!("config not reloaded, keeping the old one: {e:#}"),
        }
    }

    fn takeover(&mut self, deck: &mut Deck) -> Result<HashMap<u8, Visual>> {
        set_brightness(deck, self.current_brightness())?;
        self.wide = None;
        self.in_flight = None;
        self.wide_due = false;
        Ok(HashMap::new())
    }

    fn housekeeping(&mut self, deck: &mut Deck, painted: &mut HashMap<u8, Visual>) -> Result<()> {
        let clock_style = |cfg: &Config| (cfg.twelve_hour, cfg.date_format.clone(), cfg.weekday_format.clone());
        let style = clock_style(&self.engine.cfg);
        let brightness = self.current_brightness();
        self.reload_if_changed();
        if self.current_brightness() != brightness {
            set_brightness(deck, self.current_brightness())?;
        }
        if clock_style(&self.engine.cfg) != style {
            *painted = self.takeover(deck)?;
        }
        let stale_stats = self.wide == Some(Display::Stats) && self.last_stats.elapsed() >= STATS_REFRESH;
        if stale_stats || (self.wide.is_some() && self.clock().date != self.wide_date) {
            self.send_wide(deck)?;
        }
        if deck.last_write.elapsed() >= KEEP_ALIVE {
            set_brightness(deck, self.current_brightness())?;
        }
        Ok(())
    }

    fn current_brightness(&self) -> u8 {
        if self.locked { self.engine.cfg.locked_brightness } else { self.brightness }
    }

    fn check_lock(&mut self, deck: &mut Deck) -> Result<()> {
        let idle = platform::idle_time().min(self.last_deck_input.elapsed());
        let limit = Duration::from_secs(self.engine.cfg.sleep_minutes as u64 * 60);
        let screens_off = self.engine.cfg.sleep_with_display
            && platform::display_off()
            && self.last_deck_input.elapsed() >= DECK_ONLY_AWAKE;
        self.asleep = screens_off || (!limit.is_zero() && idle >= limit);
        let locked = platform::session_locked() || self.asleep;
        if locked != self.locked {
            self.locked = locked;
            self.engine.release_all();
            deck.command(if locked { CMD_LOCK } else { CMD_UNLOCK }, &[])?;
            set_brightness(deck, self.current_brightness())?;
        }
        Ok(())
    }

    fn follow_focus(&mut self) {
        // Editing the deck in our own window must not change what's on it.
        let window = platform::foreground_window().filter(|w| !w.exe.ends_with("openulanzi-app.exe"));
        if let Some(window) = window {
            self.engine.focus_changed(&window);
        }
    }

    fn run_steps(&mut self, deck: &mut Deck) -> Result<()> {
        while let Ok(step) = self.steps.1.try_recv() {
            for action in self.engine.perform(step) {
                self.run_action(deck, action)?;
            }
        }
        Ok(())
    }

    /// Runs until the deck errors out, which usually means it was unplugged or the PC slept.
    fn session(&mut self, deck: &mut Deck) -> Result<()> {
        let mut painted = self.takeover(deck)?;
        self.stats.sample();
        let mut last_housekeeping = Instant::now();
        let mut last_lock_check = Instant::now();
        loop {
            // Drain everything queued, then paint once: latest state wins, never a backlog.
            let mut next = deck.read(10)?;
            while let Some(input) = next {
                self.last_deck_input = Instant::now();
                if self.locked {
                    // The firmware lights the screen again on any press.
                    set_brightness(deck, self.current_brightness())?;
                } else {
                    let actions = self.engine.input(input, self.now(), &platform::foreground_window);
                    for action in actions {
                        self.run_action(deck, action)?;
                    }
                }
                next = deck.read(0)?;
            }
            if last_lock_check.elapsed() >= LOCK_CHECK {
                last_lock_check = Instant::now();
                self.check_lock(deck)?;
                self.follow_focus();
            }
            self.run_steps(deck)?;
            for action in self.engine.tick(self.now(), &platform::foreground_window) {
                self.run_action(deck, action)?;
            }
            self.paint(deck, &mut painted)?;
            self.publish(true);
            if self.wake.swap(false, Ordering::Relaxed) || last_housekeeping.elapsed() >= HOUSEKEEPING {
                last_housekeeping = Instant::now();
                self.housekeeping(deck, &mut painted)?;
            }
        }
    }
}

/// Owns the deck forever, reconnecting as needed. Set `wake` after saving the config to apply it
/// at once instead of on the next poll. Returns only if the config can't be read at startup.
pub fn run(
    path: PathBuf,
    device: Option<String>,
    wake: Arc<AtomicBool>,
    on_status: impl FnMut(&Status),
) -> Result<()> {
    let cfg = Config::load(&path)?;
    platform::watch_display();
    let mut app = App {
        device,
        modified: modified(&path),
        renderer: Renderer::new(path.parent().unwrap_or(Path::new(".")).to_path_buf()),
        brightness: cfg.brightness,
        engine: Engine::new(cfg),
        started: Instant::now(),
        path,
        locked: false,
        asleep: false,
        last_deck_input: Instant::now(),
        wake,
        on_status,
        published: None,
        steps: mpsc::channel(),
        volume: None,
        wide: None,
        in_flight: None,
        wide_due: false,
        wide_date: String::new(),
        last_stats: Instant::now(),
        stats: platform::Stats::default(),
    };
    let mut reported = false;
    loop {
        match Deck::open(app.device.as_deref()) {
            Ok(mut deck) => {
                println!("deck connected");
                reported = false;
                if let Err(e) = app.session(&mut deck) {
                    eprintln!("deck lost: {e:#}");
                }
                app.engine.release_all();
            }
            Err(e) if !reported => {
                eprintln!("waiting for deck ({e:#}). Is Ulanzi Studio closed?");
                reported = true;
            }
            Err(_) => {}
        }
        app.reload_if_changed();
        app.publish(false);
        std::thread::sleep(Duration::from_secs(2));
    }
}
