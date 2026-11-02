#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use openulanzi::config::{Config, Model, Visual};
use openulanzi::device;
use openulanzi::runtime::{self, Status, VolumePopup};
use openulanzi::platform;
use openulanzi::render::{self, Renderer};
use openulanzi::update::{self, Install, Update};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;
use std::sync::{Arc, Mutex};
use tauri::menu::{CheckMenuItem, Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, State, WebviewWindow, WindowEvent};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};
use tauri_plugin_dialog::DialogExt;

const POPUP_TIME: Duration = Duration::from_millis(1500);
const UPDATE_WAIT: Duration = Duration::from_secs(15);
const DEVICE_SCAN: Duration = Duration::from_secs(5);
/// Height above the bottom of the screen, clear of the taskbar like the Windows flyout.
const POPUP_MARGIN: f64 = 72.0;

struct Shared {
    dir: PathBuf,
    wakes: Mutex<HashMap<String, Arc<AtomicBool>>>,
    renderer: Mutex<Renderer>,
    statuses: Mutex<HashMap<String, Status>>,
    update: Mutex<Option<Update>>,
}

/// What the page needs to offer an update.
#[derive(Serialize)]
struct UpdateOffer {
    version: String,
    install: Install,
}

/// One entry per deck ever seen. The empty name is the starting config, used before any deck has
/// been plugged in and copied for each new one.
#[derive(Serialize)]
struct DeviceInfo {
    id: String,
    label: String,
    connected: bool,
}

/// `device` comes from the page, so it must never be able to point outside the config folder.
fn config_file(dir: &Path, device: &str) -> Result<PathBuf, String> {
    if !device.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return Err(format!("invalid device name {device:?}"));
    }
    Ok(dir.join(if device.is_empty() { "config.toml".to_string() } else { format!("config-{device}.toml") }))
}

fn device_label(dir: &Path, id: &str) -> String {
    let Ok(config) = config_file(dir, id).and_then(|p| Config::load(&p).map_err(|e| e.to_string())) else {
        return id.to_string();
    };
    if !config.name.is_empty() {
        return config.name;
    }
    let model = match config.model {
        Model::D200x => "D200X",
        Model::D200h => "D200H",
        Model::D200 => "D200",
    };
    let tail: String = id.chars().rev().take(4).collect::<Vec<_>>().into_iter().rev().collect();
    if tail.is_empty() { model.to_string() } else { format!("{model} {tail}") }
}

#[tauri::command]
fn list_devices(state: State<Shared>) -> Vec<DeviceInfo> {
    let saved = std::fs::read_dir(&state.dir).into_iter().flatten().flatten();
    let mut ids: Vec<String> = saved
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().to_string();
            Some(name.strip_prefix("config-")?.strip_suffix(".toml")?.to_string())
        })
        .collect();
    ids.sort();
    if ids.is_empty() {
        ids.push(String::new());
    }
    let statuses = state.statuses.lock().unwrap();
    ids.into_iter()
        .map(|id| DeviceInfo {
            label: device_label(&state.dir, &id),
            connected: statuses.get(&id).is_some_and(|s| s.connected),
            id,
        })
        .collect()
}

/// Removes a deck that is not plugged in. If it comes back it starts again from the shared config.
#[tauri::command]
fn forget_device(device: String, state: State<Shared>) -> Result<(), String> {
    if device.is_empty() {
        return Err("the starting config cannot be forgotten".into());
    }
    if state.statuses.lock().unwrap().get(&device).is_some_and(|s| s.connected) {
        return Err("unplug this deck before forgetting it".into());
    }
    let path = config_file(&state.dir, &device)?;
    std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(path.with_extension("toml.bak"));
    Ok(())
}

fn this_exe() -> Result<PathBuf, String> {
    std::env::current_exe().map_err(|e| e.to_string())
}

#[tauri::command]
fn app_version(app: AppHandle) -> String {
    app.package_info().version.to_string()
}

// async: these wait on the network, which must not block the window.
#[tauri::command(async)]
fn check_update(app: AppHandle, state: State<Shared>) -> Result<Option<UpdateOffer>, String> {
    let current = app.package_info().version.to_string();
    let found = update::check(&current).map_err(|e| format!("{e:#}"))?;
    let offer = found.as_ref().map(|u| u.version.clone());
    *state.update.lock().unwrap() = found;
    let install = update::install_kind(&this_exe()?);
    Ok(offer.map(|version| UpdateOffer { version, install }))
}

#[tauri::command(async)]
fn install_update(app: AppHandle, state: State<Shared>) -> Result<(), String> {
    let found = state.update.lock().unwrap().clone().ok_or("no update to install")?;
    update::install(&found, &this_exe()?).map_err(|e| format!("{e:#}"))?;
    app.exit(0); // the installer or new copy is waiting for this one to go
    Ok(())
}

#[tauri::command]
fn open_release_notes(state: State<Shared>) -> Result<(), String> {
    let found = state.update.lock().unwrap().clone().ok_or("no update to show")?;
    platform::open(&found.notes_url).map_err(|e| format!("{e:#}"))
}

#[tauri::command]
fn get_config(device: String, state: State<Shared>) -> Result<Config, String> {
    Config::load(&config_file(&state.dir, &device)?).map_err(|e| format!("{e:#}"))
}

#[tauri::command]
fn save_config(device: String, config: Config, state: State<Shared>) -> Result<(), String> {
    let path = config_file(&state.dir, &device)?;
    let config = config.validated().map_err(|e| format!("{e:#}"))?;
    config.save(&path).map_err(|e| format!("{e:#}"))?;
    if let Some(wake) = state.wakes.lock().unwrap().get(&device) {
        wake.store(true, Ordering::Relaxed);
    }
    Ok(())
}

#[tauri::command]
fn get_status(device: String, state: State<Shared>) -> Option<Status> {
    state.statuses.lock().unwrap().get(&device).cloned()
}

#[tauri::command]
fn render(visual: Visual, wide: bool, state: State<Shared>) -> Result<tauri::ipc::Response, String> {
    let mut renderer = state.renderer.lock().unwrap();
    let png = renderer.png(&visual, wide).map_err(|e| format!("{e:#}"))?;
    Ok(tauri::ipc::Response::new(png.to_vec()))
}

#[tauri::command]
fn app_icon(exe: String) -> Result<tauri::ipc::Response, String> {
    platform::app_icon_png(&exe).map(tauri::ipc::Response::new).ok_or_else(|| "no icon".to_string())
}

#[tauri::command]
fn icon_library() -> Vec<(&'static str, &'static str)> {
    let mut icons: Vec<_> = render::library().iter().map(|(n, s)| (*n, *s)).collect();
    icons.sort();
    icons
}

#[tauri::command]
async fn pick_file(app: AppHandle, name: String, extensions: Vec<String>) -> Option<String> {
    let mut dialog = app.dialog().file();
    if !extensions.is_empty() {
        let extensions: Vec<&str> = extensions.iter().map(String::as_str).collect();
        dialog = dialog.add_filter(name, &extensions);
    }
    dialog.blocking_pick_file().map(|p| p.to_string())
}

/// Bottom centre of the primary monitor, ignoring the mouse so it can never take a click.
fn place_popup(popup: &WebviewWindow) -> tauri::Result<()> {
    popup.set_ignore_cursor_events(true)?;
    if let Some(monitor) = popup.primary_monitor()? {
        let (area, size) = (monitor.size(), popup.outer_size()?);
        let x = monitor.position().x + (area.width as i32 - size.width as i32) / 2;
        let margin = (POPUP_MARGIN * monitor.scale_factor()) as i32;
        let y = monitor.position().y + area.height as i32 - size.height as i32 - margin;
        popup.set_position(PhysicalPosition::new(x, y))?;
    }
    Ok(())
}

fn show_popup(app: &AppHandle, volume: &VolumePopup, latest: &Arc<AtomicU64>) {
    let Some(popup) = app.get_webview_window("volume") else { return };
    let _ = app.emit("volume", volume);
    let Ok(hwnd) = popup.hwnd().map(|h| h.0 as isize) else { return };
    // Taking focus would point the next volume press at the wrong window.
    platform::show_no_activate(hwnd);
    latest.store(volume.id, Ordering::Relaxed);
    let (id, latest) = (volume.id, latest.clone());
    std::thread::spawn(move || {
        std::thread::sleep(POPUP_TIME);
        if latest.load(Ordering::Relaxed) == id {
            platform::hide_window(hwnd);
        }
    });
}

fn show_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open OpenUlanzi", true, None::<&str>)?;
    let enabled = app.autolaunch().is_enabled().unwrap_or(false);
    let login = CheckMenuItem::with_id(app, "login", "Start at login", true, enabled, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &login, &quit])?;
    let check = login.clone();
    let mut tray = TrayIconBuilder::new().tooltip("OpenUlanzi").menu(&menu).show_menu_on_left_click(false);
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.on_menu_event(move |app, event| match event.id.as_ref() {
        "open" => show_main(app),
        "quit" => app.exit(0),
        "login" => {
            let auto = app.autolaunch();
            let result = if check.is_checked().unwrap_or(false) { auto.enable() } else { auto.disable() };
            if let Err(e) = result {
                eprintln!("start at login: {e}");
            }
            let _ = check.set_checked(auto.is_enabled().unwrap_or(false));
        }
        _ => {}
    })
    .on_tray_icon_event(|tray, event| {
        let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event else {
            return;
        };
        show_main(tray.app_handle());
    })
    .build(app)?;
    Ok(())
}

fn config_dir() -> std::io::Result<PathBuf> {
    let root = std::env::var_os("APPDATA").ok_or(std::io::ErrorKind::NotFound)?;
    let dir = PathBuf::from(root).join("OpenUlanzi");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("config.toml");
    if !path.exists() {
        std::fs::write(&path, include_str!("../../examples/default.toml"))?;
    }
    Ok(dir)
}

/// A deck starts as a copy of the shared config.
fn ensure_config(dir: &Path, id: &str) -> Result<PathBuf, String> {
    let path = config_file(dir, id)?;
    if !path.exists() {
        std::fs::copy(dir.join("config.toml"), &path).map_err(|e| format!("cannot create a config for {id}: {e}"))?;
    }
    Ok(path)
}

/// Gives a deck its own config and its own thread.
fn start_device(handle: &AppHandle, id: String, path: PathBuf, latest: Arc<AtomicU64>) {
    let shared = handle.state::<Shared>();
    let wake = Arc::new(AtomicBool::new(false));
    shared.wakes.lock().unwrap().insert(id.clone(), wake.clone());
    let handle = handle.clone();
    std::thread::spawn(move || {
        let mut seen = 0;
        let on_status = |status: &Status| {
            if let Some(volume) = status.volume.as_ref().filter(|v| v.id > seen) {
                seen = volume.id;
                show_popup(&handle, volume, &latest);
            }
            handle.state::<Shared>().statuses.lock().unwrap().insert(status.device.clone(), status.clone());
            let _ = handle.emit("deck-status", status);
        };
        // An unreadable config is reported by get_config, where the user can see it.
        if let Err(e) = runtime::run(path, Some(id), wake, on_status) {
            eprintln!("deck stopped: {e:#}");
        }
    });
}

/// Starts each deck the first time it appears. Its thread then handles it coming and going.
fn watch_devices(handle: AppHandle) {
    let latest = Arc::new(AtomicU64::new(0));
    let mut started = HashSet::new();
    loop {
        for id in device::list().unwrap_or_default() {
            // Checked on every scan: a forgotten deck may have been plugged back in.
            let existed = config_file(&handle.state::<Shared>().dir, &id).is_ok_and(|p| p.exists());
            let path = match ensure_config(&handle.state::<Shared>().dir, &id) {
                Ok(path) => path,
                Err(e) => {
                    eprintln!("{e}");
                    continue;
                }
            };
            let new = started.insert(id.clone());
            if new {
                start_device(&handle, id, path, latest.clone());
            }
            if new || !existed {
                let _ = handle.emit("devices-changed", ());
            }
        }
        std::thread::sleep(DEVICE_SCAN);
    }
}

/// A portable update starts the new exe while the old one is closing.
fn finish_update() {
    let mut args = std::env::args().skip_while(|a| a != "--after-update").skip(1);
    if let Some(pid) = args.next().and_then(|p| p.parse().ok()) {
        platform::wait_for_exit(pid, UPDATE_WAIT);
    }
    if let Ok(exe) = std::env::current_exe() {
        update::clean_up(&exe);
    }
}

fn main() {
    finish_update();
    let dir = config_dir().expect("cannot create the OpenUlanzi config folder");
    tauri::Builder::default()
        // Here, not in setup: windows exist and start calling commands before setup runs.
        .manage(Shared {
            wakes: Mutex::default(),
            renderer: Mutex::new(Renderer::new(dir.clone())),
            statuses: Mutex::default(),
            update: Mutex::default(),
            dir,
        })
        // Must be first: a second copy would fight over the deck.
        .plugin(tauri_plugin_single_instance::init(|app, _, _| show_main(app)))
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, Some(vec!["--hidden"])))
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![app_version, check_update, install_update, open_release_notes, list_devices, forget_device, get_config, save_config, get_status, render, pick_file, icon_library, app_icon])
        .on_window_event(|window, event| {
            // Closing keeps the deck running; Quit in the tray ends it.
            if let (WindowEvent::CloseRequested { api, .. }, "main") = (event, window.label()) {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .setup(|app| {
            let handle = app.handle().clone();
            build_tray(&handle)?;
            if let Some(popup) = app.get_webview_window("volume") {
                place_popup(&popup)?;
            }
            if !std::env::args().any(|a| a == "--hidden") {
                show_main(&handle);
            }
            std::thread::spawn(move || watch_devices(handle));
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("failed to start OpenUlanzi");
}
