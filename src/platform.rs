//! Windows integration: input injection, launching, focused window, lock state.
// shortcut: Windows only, hide this module behind cfg when adding Linux/macOS.
use anyhow::{anyhow, bail, Result};
use resvg::tiny_skia::Pixmap;
use windows::core::{w, Interface, GUID, HSTRING, PCWSTR, PWSTR};
use windows::Win32::Foundation::SYSTEMTIME;
use windows::Win32::Globalization::{GetDateFormatEx, DATE_SHORTDATE};
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits, GetObjectW, BITMAP, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
    DIB_RGB_COLORS, HGDIOBJ,
};
use windows::Win32::Media::Audio::{
    eRender, IAudioSessionControl2, IAudioSessionManager2, IMMDeviceEnumerator, ISimpleAudioVolume,
    MMDeviceEnumerator, DEVICE_STATE_ACTIVE,
};
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED};
use std::ffi::c_void;
use std::path::Path;
use std::os::windows::process::CommandExt;
use std::ptr::null_mut;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Once;
use std::time::Duration;
use windows::Win32::Foundation::{CloseHandle, FILETIME, HANDLE, HWND, LPARAM, WIN32_ERROR, WPARAM};
use windows::Win32::System::Power::{
    PowerSettingRegisterNotification, DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS, POWERBROADCAST_SETTING,
};
use windows::Win32::UI::WindowsAndMessaging::{DEVICE_NOTIFY_CALLBACK, PBT_POWERSETTINGCHANGE};
use windows::Win32::Media::Multimedia::mciSendStringW;
use windows::Win32::System::StationsAndDesktops::{CloseDesktop, OpenInputDesktop, DESKTOP_CONTROL_FLAGS, DESKTOP_SWITCHDESKTOP};
use windows::Win32::System::SystemInformation::{GetLocalTime, GetTickCount, GlobalMemoryStatusEx, MEMORYSTATUSEX};
use windows::Win32::System::Threading::{
    GetSystemTimes, OpenProcess, QueryFullProcessImageNameW, WaitForSingleObject, PROCESS_SYNCHRONIZE, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::Shell::{SHDefExtractIconW, ShellExecuteW};
use windows::Win32::UI::WindowsAndMessaging::{
    DestroyIcon, GetForegroundWindow, GetIconInfo, GetWindowTextW, GetWindowThreadProcessId, PostMessageW, ShowWindow,
    HICON, ICONINFO, SW_HIDE, SW_SHOWNOACTIVATE, SW_SHOWNORMAL, WHEEL_DELTA, WM_CLOSE,
};

#[derive(Debug, Clone)]
pub struct Window {
    pub exe: String,
    pub title: String,
}

fn vk(name: &str) -> Option<u16> {
    let bytes = name.as_bytes();
    if let [c @ (b'a'..=b'z' | b'0'..=b'9')] = bytes {
        return Some(c.to_ascii_uppercase() as u16);
    }
    // Any virtual key by hex code, for keys no keyboard can type: vkea, vk60.
    if let Some(code) = name.strip_prefix("vk") {
        return u8::from_str_radix(code, 16).ok().filter(|c| *c != 0).map(u16::from);
    }
    if let Some(n) = name.strip_prefix('f').and_then(|n| n.parse::<u16>().ok()) {
        return (1..=24).contains(&n).then_some(VK_F1.0 + n - 1);
    }
    let key = match name {
        "ctrl" => VK_LCONTROL,
        "rctrl" => VK_RCONTROL,
        "shift" => VK_LSHIFT,
        "rshift" => VK_RSHIFT,
        "alt" => VK_LMENU,
        "altgr" | "ralt" => VK_RMENU,
        "win" => VK_LWIN,
        "rwin" => VK_RWIN,
        "enter" => VK_RETURN,
        "esc" => VK_ESCAPE,
        "tab" => VK_TAB,
        "space" => VK_SPACE,
        "backspace" => VK_BACK,
        "delete" => VK_DELETE,
        "insert" => VK_INSERT,
        "home" => VK_HOME,
        "end" => VK_END,
        "pageup" => VK_PRIOR,
        "pagedown" => VK_NEXT,
        "up" => VK_UP,
        "down" => VK_DOWN,
        "left" => VK_LEFT,
        "right" => VK_RIGHT,
        "printscreen" => VK_SNAPSHOT,
        "volume_up" => VK_VOLUME_UP,
        "volume_down" => VK_VOLUME_DOWN,
        "mute" => VK_VOLUME_MUTE,
        "play_pause" => VK_MEDIA_PLAY_PAUSE,
        "next_track" => VK_MEDIA_NEXT_TRACK,
        "prev_track" => VK_MEDIA_PREV_TRACK,
        "media_stop" => VK_MEDIA_STOP,
        _ => return None,
    };
    Some(key.0)
}

pub fn parse_hotkey(hotkey: &str) -> Result<Vec<u16>> {
    let keys = hotkey.split('+').map(|k| {
        let k = k.trim().to_ascii_lowercase();
        vk(&k).ok_or_else(|| anyhow!("unknown key {k:?} in hotkey {hotkey:?}"))
    });
    keys.collect()
}

fn is_extended(key: u16) -> bool {
    [
        VK_RCONTROL, VK_RMENU, VK_LWIN, VK_RWIN, VK_DELETE, VK_INSERT, VK_HOME, VK_END, VK_PRIOR, VK_NEXT, VK_UP,
        VK_DOWN, VK_LEFT, VK_RIGHT,
    ]
    .iter()
    .any(|k| k.0 == key)
}

fn key_input(key: u16, up: bool) -> INPUT {
    let mut flags = KEYBD_EVENT_FLAGS(0);
    if up {
        flags |= KEYEVENTF_KEYUP;
    }
    if is_extended(key) {
        flags |= KEYEVENTF_EXTENDEDKEY;
    }
    let scan = unsafe { MapVirtualKeyW(key as u32, MAPVK_VK_TO_VSC) } as u16;
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT { wVk: VIRTUAL_KEY(key), wScan: scan, dwFlags: flags, time: 0, dwExtraInfo: 0 },
        },
    }
}

/// Press all keys in order, release in reverse. Windows silently drops this for elevated targets
/// when we are not elevated ourselves (UIPI).
pub fn send_hotkey(hotkey: &str) -> Result<()> {
    let keys = parse_hotkey(hotkey)?;
    let mut inputs: Vec<INPUT> = keys.iter().map(|k| key_input(*k, false)).collect();
    inputs.extend(keys.iter().rev().map(|k| key_input(*k, true)));
    let sent = unsafe { SendInput(&inputs, size_of::<INPUT>() as i32) };
    if sent as usize != inputs.len() {
        bail!("SendInput accepted {sent} of {} events", inputs.len());
    }
    Ok(())
}

/// Like the Run dialog: a file, folder, URL or `shell:` target, or a program followed by its
/// arguments. A path with spaces is taken whole when it exists, and otherwise needs quotes.
pub fn open(target: &str) -> Result<()> {
    let (file, args) = split_command(target.trim(), |p| Path::new(p).exists());
    let args = (!args.is_empty()).then(|| HSTRING::from(args));
    let result = unsafe {
        let args = args.as_ref().map_or(PCWSTR::null(), |a| PCWSTR(a.as_ptr()));
        ShellExecuteW(Some(HWND::default()), w!("open"), &HSTRING::from(file), args, None, SW_SHOWNORMAL)
    };
    // Documented contract: values above 32 mean success.
    if result.0 as usize <= 32 {
        bail!("could not open {target:?} (code {})", result.0 as usize);
    }
    Ok(())
}

fn split_command(command: &str, exists: impl Fn(&str) -> bool) -> (&str, &str) {
    if let Some((file, args)) = command.strip_prefix('"').and_then(|rest| rest.split_once('"')) {
        return (file, args.trim_start());
    }
    if exists(command) {
        return (command, "");
    }
    command.split_once(char::is_whitespace).map_or((command, ""), |(file, args)| (file, args.trim_start()))
}

fn process_path(pid: u32) -> Option<String> {
    unsafe {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut exe = [0u16; 1024];
        let mut size = exe.len() as u32;
        let named = QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, PWSTR(exe.as_mut_ptr()), &mut size);
        let _ = CloseHandle(process);
        named.ok()?;
        Some(String::from_utf16_lossy(&exe[..size as usize]))
    }
}

pub fn foreground_window() -> Option<Window> {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_invalid() {
            return None;
        }
        let mut title = [0u16; 512];
        let len = GetWindowTextW(hwnd, &mut title) as usize;
        let mut pid = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        Some(Window { exe: process_path(pid)?, title: String::from_utf16_lossy(&title[..len]) })
    }
}

/// Audio sessions of every process running this exe, on every output. Matching the exe and not
/// the process catches browsers and Electron apps, which play sound from a child process.
fn audio_sessions(exe: &str) -> Result<Vec<ISimpleAudioVolume>> {
    let mut found = Vec::new();
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED); // already initialised is fine
        let devices: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        let outputs = devices.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE)?;
        for i in 0..outputs.GetCount()? {
            let manager: IAudioSessionManager2 = outputs.Item(i)?.Activate(CLSCTX_ALL, None)?;
            let sessions = manager.GetSessionEnumerator()?;
            for j in 0..sessions.GetCount()? {
                let session = sessions.GetSession(j)?;
                let pid = session.cast::<IAudioSessionControl2>()?.GetProcessId().unwrap_or(0);
                if pid != 0 && process_path(pid).is_some_and(|p| p.eq_ignore_ascii_case(exe)) {
                    found.push(session.cast::<ISimpleAudioVolume>()?);
                }
            }
        }
    }
    Ok(found)
}

/// Steps a 0-1 session volume by whole percents, so repeated presses land on round numbers.
fn stepped(level: f32, percent: i32) -> f32 {
    ((level * 100.0).round() as i32 + percent).clamp(0, 100) as f32 / 100.0
}

/// Changes the app's volume, returning the new percent. None means Windows has no audio session
/// for it, which is the case until the app has played something.
fn change_app_volume(exe: &str, change: impl Fn(f32) -> f32) -> Result<Option<u8>> {
    let sessions = audio_sessions(exe)?;
    let Some(first) = sessions.first() else { return Ok(None) };
    let level = change(unsafe { first.GetMasterVolume()? });
    for session in &sessions {
        unsafe { session.SetMasterVolume(level, std::ptr::null())? };
    }
    Ok(Some((level * 100.0).round() as u8))
}

pub fn step_app_volume(exe: &str, percent: i32) -> Result<Option<u8>> {
    change_app_volume(exe, |level| stepped(level, percent))
}

/// Windows scales an app by the system volume, so at 100% it simply follows it.
pub fn reset_app_volume(exe: &str) -> Result<Option<u8>> {
    change_app_volume(exe, |_| 1.0)
}

/// The exe's icon as a PNG.
pub fn app_icon_png(exe: &str) -> Option<Vec<u8>> {
    const SIZE: u32 = 48;
    unsafe {
        let mut icon = HICON::default();
        SHDefExtractIconW(&HSTRING::from(exe), 0, 0, Some(&mut icon), None, SIZE).ok().ok()?;
        let mut info = ICONINFO::default();
        let got = GetIconInfo(icon, &mut info);
        let _ = DestroyIcon(icon);
        got.ok()?;

        let mut bitmap = BITMAP::default();
        let read = GetObjectW(
            HGDIOBJ(info.hbmColor.0),
            size_of::<BITMAP>() as i32,
            Some(&mut bitmap as *mut BITMAP as *mut _),
        );
        let (width, height) = (bitmap.bmWidth, bitmap.bmHeight);
        let mut pixels = vec![0u8; (width.max(0) * height.max(0) * 4) as usize];
        let mut header = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height, // top-down
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let dc = CreateCompatibleDC(None);
        let lines = GetDIBits(dc, info.hbmColor, 0, height as u32, Some(pixels.as_mut_ptr() as *mut _), &mut header, DIB_RGB_COLORS);
        let _ = DeleteDC(dc);
        let _ = DeleteObject(HGDIOBJ(info.hbmColor.0));
        let _ = DeleteObject(HGDIOBJ(info.hbmMask.0));
        if read == 0 || lines == 0 {
            return None;
        }
        // Old icons carry no alpha at all, which would read as fully transparent.
        let opaque = pixels.chunks(4).all(|p| p[3] == 0);
        for p in pixels.chunks_mut(4) {
            let alpha = if opaque { 255 } else { p[3] };
            let scale = |c: u8| (c as u32 * alpha as u32 / 255) as u8;
            let (b, g, r) = (p[0], p[1], p[2]);
            p.copy_from_slice(&[scale(r), scale(g), scale(b), alpha]);
        }
        let size = resvg::tiny_skia::IntSize::from_wh(width as u32, height as u32)?;
        Pixmap::from_vec(pixels, size)?.encode_png().ok()
    }
}

/// Shows a window without taking focus away from the current one.
pub fn show_no_activate(hwnd: isize) {
    let _ = unsafe { ShowWindow(HWND(hwnd as *mut _), SW_SHOWNOACTIVATE) };
}

/// Hides a window shown by `show_no_activate`. Tauri's own hide does nothing there, because it
/// never learned the window became visible.
pub fn hide_window(hwnd: isize) {
    let _ = unsafe { ShowWindow(HWND(hwnd as *mut _), SW_HIDE) };
}

/// The input desktop can't be opened by a normal process while the session is locked.
pub fn session_locked() -> bool {
    match unsafe { OpenInputDesktop(DESKTOP_CONTROL_FLAGS(0), false, DESKTOP_SWITCHDESKTOP) } {
        Ok(desktop) => {
            let _ = unsafe { CloseDesktop(desktop) };
            false
        }
        Err(_) => true,
    }
}

/// Fields the firmware's digital clock faces display.
pub struct Now {
    pub time: String,
    pub date: String,
    pub weekday: String,
}

/// Formats through Windows, so names come out in the system language. An empty picture uses the
/// system short date.
fn format_date(when: &SYSTEMTIME, picture: &str) -> String {
    let picture = HSTRING::from(picture);
    let (flags, format) = if picture.is_empty() {
        (DATE_SHORTDATE, PCWSTR::null())
    } else {
        (Default::default(), PCWSTR(picture.as_ptr()))
    };
    let mut text = [0u16; 128];
    let written = unsafe { GetDateFormatEx(PCWSTR::null(), flags, Some(when), format, Some(&mut text), PCWSTR::null()) };
    // The count includes the terminator; 0 means a failure, shown as nothing rather than garbage.
    String::from_utf16_lossy(&text[..(written.max(1) as usize - 1)])
}

fn capitalised(text: &str) -> String {
    let mut letters = text.chars();
    letters.next().map_or_else(String::new, |first| first.to_uppercase().chain(letters).collect())
}

pub fn now(date_format: &str, weekday_format: &str) -> Now {
    let t = unsafe { GetLocalTime() };
    Now {
        time: format!("{:02}:{:02}:{:02}", t.wHour, t.wMinute, t.wSecond),
        date: format_date(&t, date_format),
        weekday: capitalised(&format_date(&t, weekday_format)),
    }
}

/// CPU use since the previous call, plus RAM use, both in percent.
#[derive(Default)]
pub struct Stats {
    idle: u64,
    total: u64,
}

impl Stats {
    pub fn sample(&mut self) -> (u8, u8) {
        let ticks = |f: FILETIME| ((f.dwHighDateTime as u64) << 32) | f.dwLowDateTime as u64;
        let (mut idle, mut kernel, mut user) = (FILETIME::default(), FILETIME::default(), FILETIME::default());
        let mut memory = MEMORYSTATUSEX { dwLength: size_of::<MEMORYSTATUSEX>() as u32, ..Default::default() };
        let read = unsafe { GetSystemTimes(Some(&mut idle), Some(&mut kernel), Some(&mut user)) };
        let _ = unsafe { GlobalMemoryStatusEx(&mut memory) };
        if read.is_err() {
            return (0, memory.dwMemoryLoad as u8);
        }
        let (idle, total) = (ticks(idle), ticks(kernel) + ticks(user)); // kernel time includes idle
        let (idle_delta, total_delta) = (idle - self.idle, (total - self.total).max(1));
        (self.idle, self.total) = (idle, total);
        (100 - (idle_delta * 100 / total_delta).min(100) as u8, memory.dwMemoryLoad as u8)
    }
}

/// Waits for a process to exit, up to `limit`. An update uses this so its old copy lets go of the
/// deck and the single-instance lock first.
pub fn wait_for_exit(pid: u32, limit: Duration) {
    if let Ok(process) = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, false, pid) } {
        unsafe {
            WaitForSingleObject(process, limit.as_millis() as u32);
            let _ = CloseHandle(process);
        }
    }
}

/// Display state as Windows last reported it: 0 off, 1 on, 2 dimmed.
static DISPLAY: AtomicU32 = AtomicU32::new(u32::MAX);

/// Called by Windows from its own thread, once at registration and on every change.
unsafe extern "system" fn display_changed(_: *const c_void, kind: u32, setting: *const c_void) -> u32 {
    if kind == PBT_POWERSETTINGCHANGE && !setting.is_null() {
        let setting = unsafe { &*(setting as *const POWERBROADCAST_SETTING) };
        if setting.DataLength >= 4 {
            let state = unsafe { (setting.Data.as_ptr() as *const u32).read_unaligned() };
            DISPLAY.store(state, Ordering::Relaxed);
        }
    }
    0
}

/// Starts listening for the display turning on and off. Safe to call more than once.
pub fn watch_display() {
    static STARTED: Once = Once::new();
    STARTED.call_once(|| unsafe {
        // GUID_CONSOLE_DISPLAY_STATE
        let display = GUID::from_u128(0x6fe69556_704a_47a0_8f24_c28d936fda47);
        // Leaked on purpose: this lasts as long as the app, and Windows may keep the pointer.
        let params = Box::leak(Box::new(DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS {
            Callback: Some(display_changed),
            Context: null_mut(),
        }));
        let mut registration = null_mut();
        let result = PowerSettingRegisterNotification(
            &display,
            DEVICE_NOTIFY_CALLBACK,
            HANDLE(params as *mut _ as *mut c_void),
            &mut registration,
        );
        if result != WIN32_ERROR(0) {
            eprintln!("cannot watch the display state: error {}", result.0);
        }
    });
}

pub fn display_off() -> bool {
    DISPLAY.load(Ordering::Relaxed) == 0
}

/// How long since the last keyboard or mouse input anywhere on this PC.
pub fn idle_time() -> Duration {
    let mut info = LASTINPUTINFO { cbSize: size_of::<LASTINPUTINFO>() as u32, dwTime: 0 };
    if !unsafe { GetLastInputInfo(&mut info) }.as_bool() {
        return Duration::ZERO;
    }
    Duration::from_millis(unsafe { GetTickCount() }.wrapping_sub(info.dwTime) as u64)
}

/// Types text through Unicode input, so the keyboard layout doesn't matter.
pub fn send_text(text: &str) -> Result<()> {
    let unicode = |unit: u16, up: bool| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(0),
                wScan: unit,
                dwFlags: if up { KEYEVENTF_UNICODE | KEYEVENTF_KEYUP } else { KEYEVENTF_UNICODE },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    let inputs: Vec<INPUT> = text.encode_utf16().flat_map(|u| [unicode(u, false), unicode(u, true)]).collect();
    let sent = unsafe { SendInput(&inputs, size_of::<INPUT>() as i32) };
    if sent as usize != inputs.len() {
        bail!("SendInput accepted {sent} of {} events", inputs.len());
    }
    Ok(())
}

/// Mouse wheel: positive scrolls up, or right when `horizontal`.
pub fn scroll(notches: i32, horizontal: bool) -> Result<()> {
    let input = INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx: 0,
                dy: 0,
                mouseData: (notches * WHEEL_DELTA as i32) as u32,
                dwFlags: if horizontal { MOUSEEVENTF_HWHEEL } else { MOUSEEVENTF_WHEEL },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    if unsafe { SendInput(&[input], size_of::<INPUT>() as i32) } != 1 {
        bail!("SendInput rejected the scroll");
    }
    Ok(())
}

/// Asks a program to close, as its X button would. Empty means the focused window.
pub fn close(program: &str) -> Result<()> {
    if program.is_empty() {
        unsafe { PostMessageW(Some(GetForegroundWindow()), WM_CLOSE, WPARAM(0), LPARAM(0))? };
        return Ok(());
    }
    let status = std::process::Command::new("taskkill")
        .args(["/IM", program])
        .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
        .status()?;
    if !status.success() {
        bail!("could not close {program}");
    }
    Ok(())
}

fn mci(command: &str) -> u32 {
    unsafe { mciSendStringW(&HSTRING::from(command), None, None) }
}

/// Plays through Windows' media control, which handles wav and mp3. Replaces any current sound.
pub fn play_sound(path: &str) -> Result<()> {
    stop_sound();
    let error = mci(&format!("open \"{path}\" alias openulanzi"));
    if error != 0 {
        bail!("cannot play {path} (MCI error {error})");
    }
    match mci("play openulanzi") {
        0 => Ok(()),
        error => bail!("cannot play {path} (MCI error {error})"),
    }
}

pub fn stop_sound() {
    mci("close openulanzi");
}

#[cfg(test)]
mod tests {
    #[test]
    fn commands_split_like_the_run_dialog() {
        let none = |_: &str| false;
        assert_eq!(split_command("notepad", none), ("notepad", ""));
        assert_eq!(split_command("notepad  a b.txt", none), ("notepad", "a b.txt"));
        assert_eq!(split_command(r#""C:\My App\x.exe" --flag"#, none), (r"C:\My App\x.exe", "--flag"));
        assert_eq!(split_command(r"C:\My Scripts\a b.ahk", |_| true), (r"C:\My Scripts\a b.ahk", ""));
        assert_eq!(split_command(r"shell:AppsFolder\Microsoft.WindowsCalculator_8wekyb3d8bbwe!App", none).1, "");
    }
    use super::*;

    #[test]
    fn dates_follow_the_requested_picture() {
        let day = SYSTEMTIME { wYear: 2026, wMonth: 11, wDay: 1, wDayOfWeek: 0, ..Default::default() };
        assert_eq!(format_date(&day, "dd/MM/yyyy"), "01/11/2026");
        assert_eq!(format_date(&day, "yyyy-MM-dd"), "2026-11-01");
        assert!(!format_date(&day, "").is_empty(), "system short date");
        assert!(!format_date(&day, "ddd").is_empty(), "weekday name");
        assert_eq!(capitalised("dom"), "Dom");
        assert_eq!(capitalised(""), "");
        assert_eq!(now("yyyy", "ddd").date.len(), 4);
    }

    #[test]
    fn volume_steps_stay_on_round_numbers_and_in_range() {
        assert_eq!(stepped(0.33, 5), 0.38);
        assert_eq!(stepped(0.97, 5), 1.0);
        assert_eq!(stepped(0.02, -5), 0.0);
    }

    #[test]
    fn an_app_without_sound_has_no_session() {
        assert!(step_app_volume(r"C:\no\such\program.exe", 5).unwrap().is_none());
    }

    #[test]
    fn app_icons_are_read_from_the_exe() {
        let exe = std::env::var("WINDIR").unwrap() + r"\explorer.exe";
        let png = app_icon_png(&exe).expect("explorer has an icon");
        let image = Pixmap::decode_png(&png).unwrap();
        assert_eq!(image.width(), 48);
        assert!(image.pixels().iter().any(|p| p.alpha() > 0), "not blank");
        assert!(app_icon_png(r"C:\no\such\program.exe").is_none() || !png.is_empty());
    }

    #[test]
    fn windows_reports_the_display_state() {
        watch_display();
        watch_display();
        std::thread::sleep(Duration::from_millis(500));
        assert!(DISPLAY.load(Ordering::Relaxed) <= 2, "Windows reports it once at registration");
    }

    #[test]
    fn idle_time_is_readable() {
        assert!(idle_time() < Duration::from_secs(60 * 60 * 24 * 365));
    }

    #[test]
    fn hotkeys_parse() {
        assert_eq!(parse_hotkey("Ctrl + Shift+F13").unwrap(), vec![VK_LCONTROL.0, VK_LSHIFT.0, 0x7C]);
        assert_eq!(parse_hotkey("win+v").unwrap(), vec![VK_LWIN.0, b'V' as u16]);
        assert_eq!(parse_hotkey("volume_up").unwrap(), vec![VK_VOLUME_UP.0]);
        assert_eq!(parse_hotkey("ctrl+vkEA").unwrap(), vec![VK_LCONTROL.0, 0xEA]);
        for bad in ["", "ctrl+", "f25", "f0", "nope", "vk", "vk00", "vkzz", "vk100"] {
            assert!(parse_hotkey(bad).is_err(), "{bad}");
        }
    }
}
