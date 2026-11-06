//! Actions, pages and buttons from other programs. Each folder in `plugins/` has a `plugin.toml`
//! and runs as one process, shared by every deck, that talks in JSON lines. See `docs/plugins.md`.
use crate::config::{control_index, Binding, PluginView, Surfaces, PLUGIN_BUTTON};
use crate::protocol::WIDE;
use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Bump on a change old plugins can't work with.
const PROTOCOL: u32 = 1;
const MANIFEST: &str = "plugin.toml";
const RETRY: Duration = Duration::from_secs(3);
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginField {
    pub key: String,
    pub label: String,
    #[serde(default)]
    pub hint: String,
    #[serde(default)]
    pub default: String,
    /// Which list sent by the plugin to suggest values from.
    #[serde(default)]
    pub options: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginAction {
    pub id: String,
    pub name: String,
    /// Library name like `ph:lightbulb-bold`.
    #[serde(default)]
    pub icon: String,
    #[serde(default)]
    pub fields: Vec<PluginField>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    protocol: u32,
    id: String,
    name: String,
    /// Program and arguments. A program found in the plugin's folder wins over one on the PATH.
    command: Vec<String>,
    #[serde(default, rename = "action")]
    actions: Vec<PluginAction>,
    #[serde(default, rename = "page")]
    pages: Vec<PluginAction>,
    #[serde(default, rename = "button")]
    buttons: Vec<PluginAction>,
}

/// What the editor needs to offer a plugin's actions.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct PluginState {
    pub id: String,
    pub name: String,
    pub running: bool,
    /// Why it is not running.
    pub error: String,
    pub actions: Vec<PluginAction>,
    pub pages: Vec<PluginAction>,
    pub buttons: Vec<PluginAction>,
    /// Lists sent by the plugin, by name.
    pub options: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Message {
    Options { key: String, values: Vec<String> },
    Draw {
        instance: String,
        /// A plugin page's keys by name. Keys left out stay as they are; null empties one.
        #[serde(default)]
        keys: BTreeMap<String, Option<Binding>>,
        /// A plugin button's one binding.
        button: Option<Binding>,
    },
    /// Forget what an instance drew: a button goes back to how the config draws it.
    Clear { instance: String },
}

/// What the plugins draw and which of their views are on a deck, shared by decks and plugins.
#[derive(Default)]
struct Board {
    /// Moves on with every change of `surfaces`, so decks copy them only when they have to.
    generation: u64,
    surfaces: Surfaces,
    shown: HashMap<String, HashSet<PluginView>>,
}

enum Job {
    Run { action: String, params: BTreeMap<String, String> },
    /// The views on the decks changed.
    Sync,
    /// The process with this number ended.
    Exited(u64),
}

struct Launch {
    folder: PathBuf,
    command: Vec<String>,
    inbox: Receiver<Job>,
}

struct Host {
    jobs: Sender<Job>,
    state: Arc<Mutex<PluginState>>,
    /// Taken by `start`. A plugin that can't load has none.
    launch: Mutex<Option<Launch>>,
}

/// Every plugin found when the app started.
#[derive(Default)]
pub struct Plugins {
    hosts: Vec<Host>,
    board: Arc<Mutex<Board>>,
}

fn read_manifest(path: &Path) -> Result<Manifest> {
    let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let manifest: Manifest = toml::from_str(&text).context("plugin.toml")?;
    if manifest.protocol != PROTOCOL {
        bail!("It needs plugin protocol {}, and this OpenUlanzi speaks {PROTOCOL}.", manifest.protocol);
    }
    if manifest.id.is_empty() || manifest.command.is_empty() {
        bail!("plugin.toml needs an id and a command.");
    }
    Ok(manifest)
}

impl Plugins {
    /// Reads the manifests, running nothing yet. A folder that can't be read still lists, with why.
    pub fn load(dir: &Path) -> Self {
        let dir = dir.join("plugins");
        if let Err(e) = std::fs::create_dir_all(&dir) {
            eprintln!("no plugins folder: {e}");
        }
        let mut folders: Vec<PathBuf> = std::fs::read_dir(&dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.join(MANIFEST).is_file())
            .collect();
        folders.sort();
        let hosts = folders
            .into_iter()
            .map(|folder| {
                let (jobs, inbox) = mpsc::channel();
                let (state, launch) = match read_manifest(&folder.join(MANIFEST)) {
                    Ok(m) => {
                        let state = PluginState { id: m.id, name: m.name, actions: m.actions, pages: m.pages, buttons: m.buttons, ..Default::default() };
                        (state, Some(Launch { folder, command: m.command, inbox }))
                    }
                    Err(e) => {
                        let name = folder.file_name().unwrap_or_default().to_string_lossy().to_string();
                        (PluginState { name, error: format!("{e:#}"), ..Default::default() }, None)
                    }
                };
                Host { jobs, state: Arc::new(Mutex::new(state)), launch: Mutex::new(launch) }
            })
            .collect();
        Self { hosts, board: Arc::default() }
    }

    /// Separate from `load` because a second copy of the app must never run a plugin.
    pub fn start(&self) {
        for host in &self.hosts {
            if let Some(launch) = host.launch.lock().unwrap().take() {
                let (jobs, state, board) = (host.jobs.clone(), host.state.clone(), self.board.clone());
                std::thread::spawn(move || worker(launch, jobs, state, board));
            }
        }
    }

    /// Never waits on the plugin, so a stuck one can't stall the deck.
    pub fn run(&self, id: &str, action: &str, params: &BTreeMap<String, String>) {
        let host = self.hosts.iter().find(|h| h.state.lock().unwrap().id == id);
        match host {
            Some(host) => {
                let _ = host.jobs.send(Job::Run { action: action.to_string(), params: params.clone() });
            }
            None => eprintln!("plugin action skipped: {id} is not installed"),
        }
    }

    /// Says which views a deck has on it now; plugins hear about the ones that came and went.
    pub fn show(&self, deck: &str, views: HashSet<PluginView>) {
        self.board.lock().unwrap().shown.insert(deck.to_string(), views);
        for host in &self.hosts {
            let _ = host.jobs.send(Job::Sync);
        }
    }

    /// What plugins draw, if it changed since `seen`.
    pub fn surfaces(&self, seen: &mut u64) -> Option<Surfaces> {
        let board = self.board.lock().unwrap();
        (board.generation != *seen).then(|| {
            *seen = board.generation;
            board.surfaces.clone()
        })
    }

    pub fn state(&self) -> Vec<PluginState> {
        self.hosts.iter().map(|h| h.state.lock().unwrap().clone()).collect()
    }
}

fn spawn(launch: &Launch) -> Result<(Child, ChildStdin)> {
    let (program, args) = launch.command.split_first().context("no command")?;
    let local = launch.folder.join(program);
    let program = if local.is_file() { local.as_os_str() } else { program.as_ref() };
    // A pipe nobody reads would block the plugin, so what it logs goes to a file.
    let log = File::create(launch.folder.join("plugin.log")).map_or(Stdio::null(), Stdio::from);
    let mut child = Command::new(program)
        .args(args)
        .current_dir(&launch.folder)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(log)
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .with_context(|| format!("Could not start {}", launch.command[0]))?;
    let stdin = child.stdin.take().context("no stdin")?;
    Ok((child, stdin))
}

/// All or nothing, so a mistake in one key can't leave a page half drawn.
fn draw(board: &Mutex<Board>, instance: String, keys: BTreeMap<String, Option<Binding>>, button: Option<Binding>) -> Result<()> {
    let mut changes = Vec::new();
    for (name, binding) in keys {
        let index = control_index(&name).filter(|i| *i <= WIDE).ok_or_else(|| anyhow!("no screen named {name}"))?;
        changes.push((index, binding));
    }
    changes.extend(button.map(|b| (PLUGIN_BUTTON, Some(b))));
    for binding in changes.iter().filter_map(|(_, b)| b.as_ref()) {
        binding.check_look()?;
    }
    let mut board = board.lock().unwrap();
    let surface = board.surfaces.entry(instance).or_default();
    for (index, binding) in changes {
        match binding {
            Some(binding) => surface.insert(index, binding),
            None => surface.remove(&index),
        };
    }
    board.generation += 1;
    Ok(())
}

fn apply(line: &str, state: &Mutex<PluginState>, board: &Mutex<Board>) -> Result<()> {
    let message: Message = serde_json::from_str(line)?;
    let mine = format!("{}/", state.lock().unwrap().id);
    match message {
        Message::Options { key, values } => {
            state.lock().unwrap().options.insert(key, values);
        }
        Message::Draw { instance, .. } | Message::Clear { instance } if !instance.starts_with(&mine) => {
            bail!("{instance} is not one of this plugin's");
        }
        Message::Draw { instance, keys, button } => draw(board, instance, keys, button)?,
        Message::Clear { instance } => {
            let mut board = board.lock().unwrap();
            board.surfaces.remove(&instance);
            board.generation += 1;
        }
    }
    Ok(())
}

/// Tells the plugin which of its views came and went since `told`.
fn sync(id: &str, stdin: &mut ChildStdin, board: &Mutex<Board>, told: &mut HashSet<PluginView>) -> std::io::Result<()> {
    let shown = board.lock().unwrap().shown.values().flatten().filter(|v| v.id == id).cloned().collect();
    for view in told.difference(&shown) {
        writeln!(stdin, "{}", json!({ "type": "disappear", "instance": view.instance() }))?;
    }
    for view in shown.difference(told) {
        let line = json!({ "type": "appear", "instance": view.instance(), "view": view.view, "params": view.params });
        writeln!(stdin, "{line}")?;
    }
    *told = shown;
    Ok(())
}

fn forget_for(wait: Duration, inbox: &Receiver<Job>) -> bool {
    let deadline = Instant::now() + wait;
    loop {
        // Running it later would surprise whoever pressed the key long ago.
        match inbox.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            Ok(Job::Run { action, .. }) => eprintln!("plugin action skipped while stopped: {action}"),
            Ok(Job::Exited(_) | Job::Sync) => {} // a new process hears about every view anyway
            Err(RecvTimeoutError::Timeout) => return true,
            Err(RecvTimeoutError::Disconnected) => return false,
        }
    }
}

// shortcut: if the app dies the plugin only learns from stdin closing, so one that ignores that
// lingers; put children in a Windows job object if that shows up in practice.
fn worker(launch: Launch, jobs: Sender<Job>, state: Arc<Mutex<PluginState>>, board: Arc<Mutex<Board>>) {
    let id = state.lock().unwrap().id.clone();
    let mine = format!("{id}/");
    let mut number = 0;
    loop {
        number += 1;
        match spawn(&launch) {
            Ok((mut child, mut stdin)) => {
                {
                    let mut shared = state.lock().unwrap();
                    shared.running = true;
                    shared.error.clear();
                }
                let stdout = child.stdout.take();
                let (shared, drawn, exited) = (state.clone(), board.clone(), jobs.clone());
                std::thread::spawn(move || {
                    for line in stdout.map(BufReader::new).into_iter().flat_map(|s| s.lines()) {
                        let Ok(line) = line else { break };
                        if let Err(e) = apply(&line, &shared, &drawn) {
                            eprintln!("plugin message ignored: {e:#}");
                        }
                    }
                    let _ = exited.send(Job::Exited(number));
                });
                let mut told = HashSet::new();
                let mut alive = sync(&id, &mut stdin, &board, &mut told).is_ok();
                while alive {
                    match launch.inbox.recv() {
                        Ok(Job::Run { action, params }) => {
                            let line = json!({ "type": "run", "action": action, "params": params });
                            alive = writeln!(stdin, "{line}").is_ok();
                        }
                        Ok(Job::Sync) => alive = sync(&id, &mut stdin, &board, &mut told).is_ok(),
                        // An older process ending says nothing about this one.
                        Ok(Job::Exited(n)) if n == number => alive = false,
                        Ok(Job::Exited(_)) => {}
                        Err(_) => return,
                    }
                }
                let _ = child.kill();
                let _ = child.wait();
                {
                    // What it drew can't react any more, so its keys fall back to the config.
                    let mut board = board.lock().unwrap();
                    board.surfaces.retain(|instance, _| !instance.starts_with(&mine));
                    board.generation += 1;
                }
                let mut shared = state.lock().unwrap();
                shared.running = false;
                shared.error = "The plugin stopped. Restarting it…".into();
            }
            Err(e) => {
                let mut shared = state.lock().unwrap();
                shared.running = false;
                shared.error = format!("{e:#}");
            }
        }
        if !forget_for(RETRY, &launch.inbox) {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("openulanzi-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("plugins")).unwrap();
        dir
    }

    fn wait_until(what: &str, done: impl Fn() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(30);
        while !done() {
            assert!(Instant::now() < deadline, "timed out waiting for {what}");
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    const POWERSHELL: &str = "command = ['powershell', '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', 'main.ps1']";

    #[test]
    fn the_example_runs_and_gets_its_params() {
        let dir = folder("example");
        let plugin = dir.join("plugins/notes");
        std::fs::create_dir_all(&plugin).unwrap();
        for file in ["plugin.toml", "main.ps1"] {
            std::fs::copy(Path::new("examples/plugins/notes").join(file), plugin.join(file)).unwrap();
        }
        let plugins = Plugins::load(&dir);
        plugins.start();
        wait_until("the plugin's list", || plugins.state()[0].options.contains_key("notes"));
        assert_eq!(plugins.state()[0].error, "");
        let params = BTreeMap::from([("text".to_string(), "olá mundo".to_string())]);
        plugins.run("example.notes", "write", &params);
        let text = || std::fs::read_to_string(plugin.join("notes.txt")).unwrap_or_default();
        wait_until("the note", || text().contains("olá mundo"));
    }

    #[test]
    fn the_tally_example_draws_its_page_and_counts_presses() {
        let dir = folder("tally");
        let plugin = dir.join("plugins/tally");
        std::fs::create_dir_all(&plugin).unwrap();
        for file in ["plugin.toml", "main.ps1"] {
            std::fs::copy(Path::new("examples/plugins/tally").join(file), plugin.join(file)).unwrap();
        }
        let plugins = Plugins::load(&dir);
        plugins.start();
        assert_eq!(plugins.state()[0].pages[0].id, "board");
        assert_eq!(plugins.state()[0].buttons[0].id, "clock");
        let params = |pairs: &[(&str, &str)]| pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        let board = PluginView { id: "example.tally".into(), view: "board".into(), params: params(&[("left", "Casa"), ("right", "Fora")]) };
        let clock = PluginView { id: "example.tally".into(), view: "clock".into(), params: params(&[("color", "#34d399")]) };
        let keys = |view: &PluginView| plugins.board.lock().unwrap().surfaces.get(&view.instance()).cloned().unwrap_or_default();
        plugins.show("deck", HashSet::from([board.clone(), clock.clone()]));
        wait_until("the page", || keys(&board).len() == 5);
        wait_until("the button", || keys(&clock).contains_key(&PLUGIN_BUTTON));
        assert_eq!(keys(&board)[&0].label, "Casa");
        assert!(matches!(keys(&board)[&2].hold, Some(crate::config::Action::Plugin { .. })));
        assert!(keys(&board)[&WIDE].svg.contains("0 x 0"));

        let Some(crate::config::Action::Plugin { id, action, params }) = keys(&board)[&0].action.clone() else { panic!() };
        plugins.run(&id, &action, &params);
        wait_until("the new score", || keys(&board)[&WIDE].svg.contains("1 x 0"));
    }

    /// Answers like Codex Minibar's bridge, and keeps what it was asked.
    fn fake_minibar(token: &'static str) -> (u16, Arc<Mutex<Vec<serde_json::Value>>>) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let asked: Arc<Mutex<Vec<serde_json::Value>>> = Arc::default();
        let log = asked.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let mut line = String::new();
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                if reader.read_line(&mut line).is_err() {
                    continue;
                }
                let request: serde_json::Value = serde_json::from_str(&line).unwrap();
                let window = |left: u32| json!({ "used_percent": 100 - left, "remaining_percent": left, "resets_at": "2099-01-01T00:00:00.123456789Z", "duration_minutes": 300 });
                let provider = |id: &str, name: &str, left: u32| json!({ "id": id, "name": name, "brand_rgb": [217, 119, 87], "sampled_at": "2026-01-01T00:00:00.123456789Z", "metrics": [
                    { "id": "x.session", "label": "Sessão de 5h", "window": window(left) },
                    { "id": "x.weekly", "label": "Semanal", "window": window(left - 10) },
                ] });
                let empty = json!({ "id": "off", "name": "Off", "metrics": [] });
                let answer = match request["op"].as_str().unwrap() {
                    _ if request["token"] != token => json!({ "ok": false }),
                    "snapshot" => json!({ "ok": true, "providers": [provider("a", "Claude \u{b7} Pessoal", 64), empty, provider("b", "Codex", 12)] }),
                    "catalog" => json!({ "ok": true, "providers": [{ "id": "a", "metrics": [{ "id": "x.session", "label": "5h session" }, { "id": "x.weekly", "label": "Weekly" }] }] }),
                    _ => json!({ "ok": true }),
                };
                log.lock().unwrap().push(request);
                let _ = writeln!(&stream, "{answer}");
            }
        });
        (port, asked)
    }

    #[test]
    fn the_minibar_plugin_draws_quotas_and_opens_the_popup() {
        let dir = folder("minibar");
        let plugin = dir.join("plugins/minibar");
        let config = plugin.join("Codex Minibar/Codex Minibar/config");
        std::fs::create_dir_all(&config).unwrap();
        let (port, asked) = fake_minibar("secret");
        let bridge = json!({ "protocol": 1, "host": "127.0.0.1", "port": port, "token": "secret" });
        std::fs::write(config.join("streamdeck-bridge.json"), bridge.to_string()).unwrap();
        std::fs::copy("examples/plugins/codex-minibar/main.ps1", plugin.join("main.ps1")).unwrap();
        // Keeps the test off the real Minibar of whoever runs it.
        std::fs::write(plugin.join("start.ps1"), r#"$env:APPDATA = $PSScriptRoot; & "$PSScriptRoot\main.ps1""#).unwrap();
        let manifest = std::fs::read_to_string("examples/plugins/codex-minibar/plugin.toml").unwrap().replace("main.ps1", "start.ps1");
        std::fs::write(plugin.join("plugin.toml"), manifest).unwrap();

        let plugins = Plugins::load(&dir);
        plugins.start();
        wait_until("the suggestions", || plugins.state()[0].options.contains_key("providers"));
        assert_eq!(plugins.state()[0].options["providers"], ["Claude \u{b7} Pessoal", "Codex"]);
        let view = |view: &str, params: &[(&str, &str)]| PluginView {
            id: "openulanzi.codex-minibar".into(),
            view: view.into(),
            params: params.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
        };
        let page = view("dashboard", &[]);
        let button = view("quota", &[("provider", "Codex"), ("metric", "x.weekly")]);
        let keys = |view: &PluginView| plugins.board.lock().unwrap().surfaces.get(&view.instance()).cloned().unwrap_or_default();
        plugins.show("deck", HashSet::from([page.clone(), button.clone()]));
        wait_until("the page", || keys(&page).contains_key(&WIDE));
        wait_until("the button", || keys(&button).contains_key(&PLUGIN_BUTTON));

        let page = keys(&page);
        assert!(page[&0].svg.contains("64%") && page[&0].svg.contains("Pessoal") && page[&0].svg.contains("5h"));
        assert!(page[&5].svg.contains("54%") && page[&5].svg.contains("Weekly"));
        assert!(page[&1].svg.contains("12%") && page[&6].svg.contains("2%"));
        assert!(page[&WIDE].svg.contains("2%") && page[&WIDE].svg.contains("Codex"), "the tightest quota");
        assert!(!page.contains_key(&2), "a provider with no data takes no column");
        assert_eq!(page[&12].action, Some(crate::config::Action::Back));
        assert!(page[&11].label.starts_with("Updated"), "shows when a refresh has worked");
        assert!(keys(&button)[&PLUGIN_BUTTON].svg.contains("2%"));

        let Some(crate::config::Action::Plugin { id, action, params }) = page[&1].action.clone() else { panic!() };
        plugins.run(&id, &action, &params);
        wait_until("the popup", || asked.lock().unwrap().iter().any(|r| r["op"] == "open_popup" && r["provider"] == "b"));
    }

    #[test]
    fn a_crashed_plugin_comes_back() {
        let dir = folder("crash");
        let plugin = dir.join("plugins/crash");
        std::fs::create_dir_all(&plugin).unwrap();
        let manifest = format!("protocol = 1\nid = 'crash'\nname = 'Crash'\n{POWERSHELL}\n");
        std::fs::write(plugin.join("plugin.toml"), manifest).unwrap();
        // Counts its starts, then dies on the first action.
        let script = r#"
            Add-Content starts.txt 'x'
            $starts = (Get-Content starts.txt).Count
            [Console]::Out.WriteLine('not json, which must be ignored')
            [Console]::Out.WriteLine("{""type"":""options"",""key"":""starts"",""values"":[""$starts""]}")
            [Console]::In.ReadLine() | Out-Null
            exit 1
        "#;
        std::fs::write(plugin.join("main.ps1"), script).unwrap();
        let plugins = Plugins::load(&dir);
        plugins.start();
        let starts = || plugins.state()[0].options.get("starts").cloned();
        wait_until("the first start", || starts().is_some());
        plugins.run("crash", "any", &BTreeMap::new());
        wait_until("the restart", || starts() == Some(vec!["2".to_string()]));
    }

    #[test]
    fn plugins_hear_about_views_and_draw_on_them() {
        let dir = folder("views");
        let plugin = dir.join("plugins/views");
        std::fs::create_dir_all(&plugin).unwrap();
        std::fs::write(plugin.join("plugin.toml"), format!("protocol = 1\nid = 'views'\nname = 'Views'\n{POWERSHELL}\n")).unwrap();
        // Draws on what appears and clears what goes; a bad draw must change nothing. The second
        // process only draws, to prove a restarted plugin hears about what is still on the deck.
        let script = r##"
            [Console]::InputEncoding = [Text.UTF8Encoding]::new($false)
            [Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
            Add-Content starts.txt 'x'
            $starts = (Get-Content starts.txt).Count
            while ($null -ne ($line = [Console]::In.ReadLine())) {
                $m = $line | ConvertFrom-Json
                if ($m.type -eq 'run') { exit 1 }
                if ($m.type -eq 'appear') {
                    $good = @{ type = 'draw'; instance = $m.instance; keys = @{ '3' = @{ label = "$($m.params.who) $starts"; action = 'back' } } }
                    $bad = @{ type = 'draw'; instance = $m.instance; keys = @{ '4' = @{ label = 'no' }; knob1 = @{ label = 'no' } } }
                    $other = @{ type = 'draw'; instance = 'someone.else/1'; button = @{ label = 'no' } }
                    foreach ($message in $bad, $other, $good) { [Console]::Out.WriteLine(($message | ConvertTo-Json -Compress -Depth 5)) }
                }
                if ($m.type -eq 'disappear') { [Console]::Out.WriteLine("{""type"":""clear"",""instance"":""$($m.instance)""}") }
            }
        "##;
        std::fs::write(plugin.join("main.ps1"), script).unwrap();
        let plugins = Plugins::load(&dir);
        plugins.start();
        let view = PluginView { id: "views".into(), view: "page".into(), params: [("who".to_string(), "olá".to_string())].into() };
        let instance = view.instance();
        let keys = |plugins: &Plugins| {
            let board = plugins.board.lock().unwrap();
            let surface = board.surfaces.get(&instance).cloned().unwrap_or_default();
            surface.iter().map(|(k, b)| (*k, b.label.clone())).collect::<Vec<_>>()
        };

        plugins.show("deck", HashSet::from([view]));
        wait_until("the draw", || keys(&plugins) == [(3, "olá 1".to_string())]);
        plugins.run("views", "die", &BTreeMap::new());
        wait_until("the redraw after a restart", || keys(&plugins) == [(3, "olá 2".to_string())]);
        let mut seen = 0;
        assert!(plugins.surfaces(&mut seen).is_some());
        assert!(plugins.surfaces(&mut seen).is_none(), "nothing new to copy");
        plugins.show("deck", HashSet::new());
        wait_until("the clear", || keys(&plugins).is_empty());
        assert!(plugins.board.lock().unwrap().surfaces.is_empty());
    }

    #[test]
    fn a_plugin_that_cannot_start_says_why() {
        let dir = folder("missing");
        let plugin = dir.join("plugins/gone");
        std::fs::create_dir_all(&plugin).unwrap();
        std::fs::write(plugin.join("plugin.toml"), "protocol = 1\nid = 'gone'\nname = 'Gone'\ncommand = ['no-such-program']").unwrap();
        let plugins = Plugins::load(&dir);
        plugins.start();
        wait_until("the error", || plugins.state()[0].error.contains("no-such-program"));
        assert!(!plugins.state()[0].running);
    }

    #[test]
    fn a_broken_manifest_is_listed_and_never_runs() {
        let dir = folder("broken");
        for (name, text) in [("bad", "protocol ="), ("newer", "protocol = 99\nid = 'x'\nname = 'X'\ncommand = ['x']")] {
            let plugin = dir.join("plugins").join(name);
            std::fs::create_dir_all(&plugin).unwrap();
            std::fs::write(plugin.join("plugin.toml"), text).unwrap();
        }
        let plugins = Plugins::load(&dir);
        plugins.start();
        let state = plugins.state();
        assert_eq!(state[0].name, "bad");
        assert!(state[1].error.contains("protocol 99"));
        assert!(state.iter().all(|p| !p.running));
    }
}
