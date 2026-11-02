//! Actions from other programs. Each folder in `plugins/` has a `plugin.toml` and runs as one
//! process, shared by every deck, that reads a JSON line per action on stdin. See `docs/plugins.md`.
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeMap;
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
    /// Lists sent by the plugin, by name.
    pub options: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Message {
    Options { key: String, values: Vec<String> },
}

enum Job {
    Run { action: String, params: BTreeMap<String, String> },
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
                        let state = PluginState { id: m.id, name: m.name, actions: m.actions, ..Default::default() };
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
        Self { hosts }
    }

    /// Separate from `load` because a second copy of the app must never run a plugin.
    pub fn start(&self) {
        for host in &self.hosts {
            if let Some(launch) = host.launch.lock().unwrap().take() {
                let (jobs, state) = (host.jobs.clone(), host.state.clone());
                std::thread::spawn(move || worker(launch, jobs, state));
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

fn apply(line: &str, state: &Mutex<PluginState>) {
    match serde_json::from_str::<Message>(line) {
        Ok(Message::Options { key, values }) => {
            state.lock().unwrap().options.insert(key, values);
        }
        Err(e) => eprintln!("plugin message ignored: {e}"),
    }
}

fn forget_for(wait: Duration, inbox: &Receiver<Job>) -> bool {
    let deadline = Instant::now() + wait;
    loop {
        // Running it later would surprise whoever pressed the key long ago.
        match inbox.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            Ok(Job::Run { action, .. }) => eprintln!("plugin action skipped while stopped: {action}"),
            Ok(Job::Exited(_)) => {}
            Err(RecvTimeoutError::Timeout) => return true,
            Err(RecvTimeoutError::Disconnected) => return false,
        }
    }
}

// shortcut: if the app dies the plugin only learns from stdin closing, so one that ignores that
// lingers; put children in a Windows job object if that shows up in practice.
fn worker(launch: Launch, jobs: Sender<Job>, state: Arc<Mutex<PluginState>>) {
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
                let (shared, exited) = (state.clone(), jobs.clone());
                std::thread::spawn(move || {
                    for line in stdout.map(BufReader::new).into_iter().flat_map(|s| s.lines()) {
                        let Ok(line) = line else { break };
                        apply(&line, &shared);
                    }
                    let _ = exited.send(Job::Exited(number));
                });
                loop {
                    match launch.inbox.recv() {
                        Ok(Job::Run { action, params }) => {
                            let line = json!({ "type": "run", "action": action, "params": params });
                            if writeln!(stdin, "{line}").is_err() {
                                break;
                            }
                        }
                        // An older process ending says nothing about this one.
                        Ok(Job::Exited(n)) if n == number => break,
                        Ok(Job::Exited(_)) => {}
                        Err(_) => return,
                    }
                }
                let _ = child.kill();
                let _ = child.wait();
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
