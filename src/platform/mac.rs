//! macOS integration: input injection, launching, focused window, lock state.
//!
//! Posting keys and reading window titles need the Accessibility permission, which
//! `request_permissions` asks for. Hotkeys are stored with Windows key names and codes, so one
//! config works on both systems: `win` is Command, `alt` is Option and `vkXX` codes are translated
//! to the Mac key in the same place on the keyboard.
use super::{capitalised, split_command, Now, Window};
use anyhow::{anyhow, bail, Context, Result};
use objc2::AllocAnyThread;
use objc2_app_kit::{
    NSBitmapImageFileType, NSBitmapImageRep, NSEvent, NSEventModifierFlags, NSEventType, NSRunningApplication,
    NSWindow, NSWorkspace,
};
use objc2_application_services::{AXError, AXIsProcessTrusted, AXIsProcessTrustedWithOptions, AXUIElement};
use objc2_core_foundation::{CFBoolean, CFDictionary, CFRetained, CFString, CFType};
use objc2_core_graphics::{
    CGDisplayIsAsleep, CGEvent, CGEventFlags, CGEventSource, CGEventSourceStateID, CGEventTapLocation, CGEventType,
    CGMainDisplayID, CGScrollEventUnit, CGSessionCopyCurrentDictionary,
};
use objc2_foundation::{NSDate, NSDateFormatter, NSDateFormatterStyle, NSDictionary, NSLocale, NSString};
use resvg::tiny_skia::{FilterQuality, Pixmap, PixmapPaint, Transform};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::ptr::NonNull;
use std::sync::Mutex;
use std::time::{Duration, Instant};

const ACCESSIBILITY: &str =
    "OpenUlanzi needs Accessibility access to press keys: System Settings > Privacy & Security > Accessibility";

/// A Windows virtual key code, so configs stay the same on both systems.
fn vk(name: &str) -> Option<u16> {
    let bytes = name.as_bytes();
    if let [c @ (b'a'..=b'z' | b'0'..=b'9')] = bytes {
        return Some(c.to_ascii_uppercase() as u16);
    }
    if let Some(code) = name.strip_prefix("vk") {
        return u8::from_str_radix(code, 16).ok().filter(|c| *c != 0).map(u16::from);
    }
    if let Some(n) = name.strip_prefix('f').and_then(|n| n.parse::<u16>().ok()) {
        return (1..=24).contains(&n).then_some(0x70 + n - 1);
    }
    Some(match name {
        "ctrl" => 0xA2,
        "rctrl" => 0xA3,
        "shift" => 0xA0,
        "rshift" => 0xA1,
        "alt" | "option" | "opt" => 0xA4,
        "altgr" | "ralt" => 0xA5,
        "win" | "cmd" | "command" => 0x5B,
        "rwin" | "rcmd" => 0x5C,
        "enter" => 0x0D,
        "esc" => 0x1B,
        "tab" => 0x09,
        "space" => 0x20,
        "backspace" => 0x08,
        "delete" => 0x2E,
        "insert" => 0x2D,
        "home" => 0x24,
        "end" => 0x23,
        "pageup" => 0x21,
        "pagedown" => 0x22,
        "up" => 0x26,
        "down" => 0x28,
        "left" => 0x25,
        "right" => 0x27,
        "printscreen" => 0x2C,
        "volume_up" => 0xAF,
        "volume_down" => 0xAE,
        "mute" => 0xAD,
        "play_pause" => 0xB3,
        "next_track" => 0xB0,
        "prev_track" => 0xB1,
        "media_stop" => 0xB2,
        _ => return None,
    })
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Key {
    /// A Mac virtual key code (kVK_*).
    Code(u16),
    /// An NX_KEYTYPE_* media key, sent as a system event like the keyboard's own media keys.
    Media(isize),
}

/// The Mac key in the same place as a Windows virtual key.
fn mac_key(vk: u16) -> Option<Key> {
    let letters = b"ASDFHGZXCV\0BQWERYT";
    if let Some(i) = (b'A'..=b'Z').contains(&(vk as u8)).then(|| letters.iter().position(|l| *l as u16 == vk)).flatten() {
        return Some(Key::Code(i as u16));
    }
    let code = match vk {
        0x30 => 0x1D, 0x31 => 0x12, 0x32 => 0x13, 0x33 => 0x14, 0x34 => 0x15,
        0x35 => 0x17, 0x36 => 0x16, 0x37 => 0x1A, 0x38 => 0x1C, 0x39 => 0x19,
        0x4F => 0x1F, 0x55 => 0x20, 0x49 => 0x22, 0x50 => 0x23, 0x4C => 0x25,
        0x4A => 0x26, 0x4B => 0x28, 0x4E => 0x2D, 0x4D => 0x2E,
        0x08 => 0x33, 0x09 => 0x30, 0x0D => 0x24, 0x14 => 0x39, 0x1B => 0x35, 0x20 => 0x31,
        0x21 => 0x74, 0x22 => 0x79, 0x23 => 0x77, 0x24 => 0x73,
        0x25 => 0x7B, 0x26 => 0x7E, 0x27 => 0x7C, 0x28 => 0x7D,
        0x2C => 0x69, // Print Screen sits where F13 is on a Mac keyboard
        0x2D => 0x72, // Insert is Help
        0x2E => 0x75, // forward delete
        0x10 | 0xA0 => 0x38, 0xA1 => 0x3C,
        0x11 | 0xA2 => 0x3B, 0xA3 => 0x3E,
        0x12 | 0xA4 => 0x3A, 0xA5 => 0x3D,
        0x5B => 0x37, 0x5C => 0x36,
        0x60 => 0x52, 0x61 => 0x53, 0x62 => 0x54, 0x63 => 0x55, 0x64 => 0x56,
        0x65 => 0x57, 0x66 => 0x58, 0x67 => 0x59, 0x68 => 0x5B, 0x69 => 0x5C,
        0x6A => 0x43, 0x6B => 0x45, 0x6D => 0x4E, 0x6E => 0x41, 0x6F => 0x4B,
        0x70 => 0x7A, 0x71 => 0x78, 0x72 => 0x63, 0x73 => 0x76, 0x74 => 0x60,
        0x75 => 0x61, 0x76 => 0x62, 0x77 => 0x64, 0x78 => 0x65, 0x79 => 0x6D,
        0x7A => 0x67, 0x7B => 0x6F, 0x7C => 0x69, 0x7D => 0x6B, 0x7E => 0x71,
        0x7F => 0x6A, 0x80 => 0x40, 0x81 => 0x4F, 0x82 => 0x50, 0x83 => 0x5A,
        0xBA => 0x29, 0xBB => 0x18, 0xBC => 0x2B, 0xBD => 0x1B, 0xBE => 0x2F, 0xBF => 0x2C,
        0xC0 => 0x32, 0xDB => 0x21, 0xDC => 0x2A, 0xDD => 0x1E, 0xDE => 0x27,
        0xE2 => 0x0A, // the extra ISO key beside left Shift
        0xC1 => 0x5E, // the ABNT2 slash key
        0xAD => return Some(Key::Media(7)),
        0xAE => return Some(Key::Media(1)),
        0xAF => return Some(Key::Media(0)),
        0xB0 => return Some(Key::Media(17)),
        0xB1 => return Some(Key::Media(18)),
        0xB3 => return Some(Key::Media(16)),
        _ => return None,
    };
    Some(Key::Code(code))
}

/// The modifier flag a Mac key sets while held, or 0.
fn modifier(code: u16) -> u64 {
    match code {
        0x37 | 0x36 => CGEventFlags::MaskCommand.0,
        0x38 | 0x3C => CGEventFlags::MaskShift.0,
        0x3A | 0x3D => CGEventFlags::MaskAlternate.0,
        0x3B | 0x3E => CGEventFlags::MaskControl.0,
        _ => 0,
    }
}

fn mac_keys(hotkey: &str) -> Result<Vec<Key>> {
    let keys = hotkey.split('+').map(|k| {
        let k = k.trim().to_ascii_lowercase();
        let code = vk(&k).ok_or_else(|| anyhow!("unknown key {k:?} in hotkey {hotkey:?}"))?;
        mac_key(code).ok_or_else(|| anyhow!("{k:?} has no key on a Mac"))
    });
    let keys: Vec<Key> = keys.collect::<Result<_>>()?;
    if keys.len() > 1 && keys.iter().any(|k| matches!(k, Key::Media(_))) {
        bail!("media keys can't be combined with other keys on macOS");
    }
    Ok(keys)
}

/// Windows key codes, after checking each one has a Mac key.
pub fn parse_hotkey(hotkey: &str) -> Result<Vec<u16>> {
    mac_keys(hotkey)?;
    Ok(hotkey.split('+').filter_map(|k| vk(&k.trim().to_ascii_lowercase())).collect())
}

fn source() -> Option<CFRetained<CGEventSource>> {
    CGEventSource::new(CGEventSourceStateID::HIDSystemState)
}

fn post(event: Option<CFRetained<CGEvent>>) -> Result<()> {
    let event = event.ok_or_else(|| anyhow!("macOS refused to make the input event"))?;
    CGEvent::post(CGEventTapLocation::HIDEventTap, Some(&event));
    Ok(())
}

fn needs_accessibility() -> Result<()> {
    if !unsafe { AXIsProcessTrusted() } {
        bail!(ACCESSIBILITY);
    }
    Ok(())
}

/// Asks for Accessibility access, showing the system prompt the first time.
pub fn request_permissions() {
    let key = unsafe { objc2_application_services::kAXTrustedCheckOptionPrompt };
    let options = CFDictionary::<CFString, CFBoolean>::from_slices(&[key], &[CFBoolean::new(true)]);
    unsafe { AXIsProcessTrustedWithOptions(Some(options.as_opaque())) };
}

fn media_key(key: isize) -> Result<()> {
    for down in [true, false] {
        let state: isize = if down { 0xA } else { 0xB };
        let event = NSEvent::otherEventWithType_location_modifierFlags_timestamp_windowNumber_context_subtype_data1_data2(
            NSEventType::SystemDefined,
            Default::default(),
            NSEventModifierFlags((state as usize) << 8),
            0.0,
            0,
            None,
            8, // NX_SUBTYPE_AUX_CONTROL_BUTTONS
            (key << 16) | (state << 8),
            -1,
        );
        let event = event.and_then(|e| e.CGEvent()).ok_or_else(|| anyhow!("macOS refused to make the media key"))?;
        CGEvent::post(CGEventTapLocation::HIDEventTap, Some(&event));
    }
    Ok(())
}

/// Press all keys in order, release in reverse, with the held modifiers set on every event.
pub fn send_hotkey(hotkey: &str) -> Result<()> {
    let keys = mac_keys(hotkey)?;
    needs_accessibility()?;
    if let [Key::Media(media)] = keys[..] {
        return media_key(media);
    }
    let codes: Vec<u16> = keys.iter().filter_map(|k| if let Key::Code(c) = k { Some(*c) } else { None }).collect();
    let source = source();
    let key = |code: u16, down: bool, flags: u64| {
        let event = CGEvent::new_keyboard_event(source.as_deref(), code, down);
        if let Some(event) = &event {
            CGEvent::set_flags(Some(event), CGEventFlags(flags));
        }
        post(event)
    };
    let mut flags = 0;
    for code in &codes {
        flags |= modifier(*code);
        key(*code, true, flags)?;
    }
    for code in codes.iter().rev() {
        flags &= !modifier(*code);
        key(*code, false, flags)?;
    }
    Ok(())
}

/// Types text as Unicode, so the keyboard layout doesn't matter.
pub fn send_text(text: &str) -> Result<()> {
    needs_accessibility()?;
    let source = source();
    // Apps read at most 20 UTF-16 units per event; chunks end on whole characters.
    let mut chunks: Vec<Vec<u16>> = vec![Vec::new()];
    for c in text.chars() {
        let mut units = [0u16; 2];
        let units = c.encode_utf16(&mut units);
        if chunks.last().unwrap().len() + units.len() > 20 {
            chunks.push(Vec::new());
        }
        chunks.last_mut().unwrap().extend_from_slice(units);
    }
    for chunk in chunks.iter().filter(|c| !c.is_empty()) {
        for down in [true, false] {
            let event = CGEvent::new_keyboard_event(source.as_deref(), 0, down);
            if let Some(event) = &event {
                CGEvent::set_flags(Some(event), CGEventFlags(0));
                unsafe { CGEvent::keyboard_set_unicode_string(Some(event), chunk.len() as _, chunk.as_ptr()) };
            }
            post(event)?;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    Ok(())
}

/// Mouse wheel: positive scrolls up, or right when `horizontal`.
pub fn scroll(notches: i32, horizontal: bool) -> Result<()> {
    needs_accessibility()?;
    // The second wheel counts to the left, the opposite of Windows.
    let (vertical, sideways) = if horizontal { (0, -notches) } else { (notches, 0) };
    post(CGEvent::new_scroll_wheel_event2(source().as_deref(), CGScrollEventUnit::Line, 2, vertical, sideways, 0))
}

/// The PATH for programs started from here. An app opened from Finder gets only the system
/// folders, which leaves out Homebrew and anything else a shell would find.
pub fn search_path() -> String {
    let current = std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin:/usr/sbin:/sbin".into());
    let mut dirs: Vec<&str> = current.split(':').filter(|d| !d.is_empty()).collect();
    for extra in ["/opt/homebrew/bin", "/usr/local/bin"] {
        if !dirs.contains(&extra) {
            dirs.push(extra);
        }
    }
    dirs.join(":")
}

fn expand(path: &str) -> String {
    match path.strip_prefix("~/") {
        Some(rest) => std::env::var("HOME").map_or_else(|_| path.to_string(), |home| format!("{home}/{rest}")),
        None => path.to_string(),
    }
}

/// Arguments split on spaces, keeping quoted ones whole.
fn split_args(args: &str) -> Vec<String> {
    let (mut out, mut current, mut quoted, mut any) = (Vec::new(), String::new(), false, false);
    for c in args.chars() {
        match c {
            '"' => (quoted, any) = (!quoted, true),
            c if c.is_whitespace() && !quoted => {
                if any || !current.is_empty() {
                    out.push(std::mem::take(&mut current));
                }
                any = false;
            }
            c => current.push(c),
        }
    }
    if any || !current.is_empty() {
        out.push(current);
    }
    out
}

fn is_url(target: &str) -> bool {
    target.split_once(':').is_some_and(|(scheme, _)| {
        scheme.len() > 1 && scheme.chars().all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c))
    })
}

fn run_open(args: &[&str]) -> Result<()> {
    let output = Command::new("/usr/bin/open").args(args).stdin(Stdio::null()).output().context("starting open")?;
    if !output.status.success() {
        bail!("{}", String::from_utf8_lossy(&output.stderr).trim());
    }
    Ok(())
}

/// A file, folder, app or URL, as a double click would open it, or a program followed by its
/// arguments. An app can be named on its own (`Safari`) or by path, and takes arguments too.
pub fn open(target: &str) -> Result<()> {
    let target = target.trim();
    let (file, args) = split_command(target, |p| Path::new(&expand(p)).exists());
    let (file, args) = (expand(file), split_args(args));
    let path = Path::new(&file);
    let app = file.trim_end_matches('/').ends_with(".app");
    if is_url(&file) && !path.exists() || args.is_empty() && path.exists() && !app {
        return run_open(&[&file]).with_context(|| format!("could not open {target:?}"));
    }
    if app || !file.contains('/') {
        let mut open = vec!["-a", &file];
        if !args.is_empty() {
            open.push("--args");
            open.extend(args.iter().map(String::as_str));
        }
        match run_open(&open) {
            Ok(()) => return Ok(()),
            Err(e) if app => return Err(e).with_context(|| format!("could not open {target:?}")),
            Err(_) => {} // not an app name, so maybe a program on the PATH
        }
    }
    let mut command = Command::new(&file);
    command.args(&args).env("PATH", search_path()).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    let child = command.spawn();
    let mut child = child.with_context(|| format!("could not open {target:?}"))?;
    std::thread::spawn(move || child.wait()); // reaped, so it doesn't linger as a zombie
    Ok(())
}

fn app_path(app: &NSRunningApplication) -> Option<String> {
    let url = app.bundleURL().or_else(|| app.executableURL())?;
    Some(url.path()?.to_string())
}

/// The focused window's title through Accessibility, or None without that permission.
fn window_title(pid: i32) -> Option<String> {
    let read = |element: &AXUIElement, name: &'static str| -> Option<CFRetained<CFType>> {
        let mut value: *const CFType = std::ptr::null();
        let error = unsafe { element.copy_attribute_value(&CFString::from_static_str(name), NonNull::from(&mut value)) };
        if error != AXError::Success {
            return None;
        }
        NonNull::new(value as *mut CFType).map(|v| unsafe { CFRetained::from_raw(v) })
    };
    unsafe {
        if !AXIsProcessTrusted() {
            return None;
        }
        let app = AXUIElement::new_application(pid);
        app.set_messaging_timeout(0.25); // a hung app must not stall the deck
        let window = read(&app, "AXFocusedWindow")?.downcast::<AXUIElement>().ok()?;
        Some(read(&window, "AXTitle")?.downcast::<CFString>().ok()?.to_string())
    }
}

pub fn foreground_window() -> Option<Window> {
    let app = NSWorkspace::sharedWorkspace().frontmostApplication()?;
    let exe = app_path(&app)?;
    Some(Window { title: window_title(app.processIdentifier()).unwrap_or_default(), exe })
}

/// macOS has no volume per app; only the system volume, through the media keys.
pub fn step_app_volume(_exe: &str, _percent: i32) -> Result<Option<u8>> {
    bail!("macOS has no volume per app; use the volume up and down keys instead")
}

pub fn reset_app_volume(exe: &str) -> Result<Option<u8>> {
    step_app_volume(exe, 0)
}

/// The app's icon as a 48 px PNG.
pub fn app_icon_png(exe: &str) -> Option<Vec<u8>> {
    const SIZE: u32 = 48;
    if !Path::new(exe).exists() {
        return None;
    }
    let image = NSWorkspace::sharedWorkspace().iconForFile(&NSString::from_str(exe));
    let png = unsafe {
        let mut rect = objc2_foundation::NSRect::new(Default::default(), objc2_foundation::NSSize::new(SIZE as f64, SIZE as f64));
        let picture = image.CGImageForProposedRect_context_hints(&mut rect, None, None)?;
        let bitmap = NSBitmapImageRep::initWithCGImage(NSBitmapImageRep::alloc(), &picture);
        bitmap.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new())?
    };
    let decoded = Pixmap::decode_png(&png.to_vec()).ok()?;
    if decoded.width() == SIZE && decoded.height() == SIZE {
        return Some(png.to_vec());
    }
    // A Retina screen hands back a bigger picture.
    let mut scaled = Pixmap::new(SIZE, SIZE)?;
    let scale = Transform::from_scale(SIZE as f32 / decoded.width() as f32, SIZE as f32 / decoded.height() as f32);
    let paint = PixmapPaint { quality: FilterQuality::Bicubic, ..Default::default() };
    scaled.draw_pixmap(0, 0, decoded.as_ref(), &paint, scale, None);
    scaled.encode_png().ok()
}

/// Shows a window without making the app active. Takes an `NSWindow` pointer and must run on
/// the main thread.
pub fn show_no_activate(ns_window: isize) {
    if let Some(window) = unsafe { (ns_window as *const NSWindow).as_ref() } {
        window.orderFrontRegardless();
    }
}

/// Hides a window shown by `show_no_activate`, on the main thread.
pub fn hide_window(ns_window: isize) {
    if let Some(window) = unsafe { (ns_window as *const NSWindow).as_ref() } {
        window.orderOut(None);
    }
}

/// Locked, or switched to another user.
pub fn session_locked() -> bool {
    let Some(session) = CGSessionCopyCurrentDictionary() else { return false };
    let flag = |name: &'static str| unsafe {
        let key = CFString::from_static_str(name);
        let value = session.value((&*key as *const CFString).cast());
        (value as *const CFBoolean).as_ref().map(|b| b.as_bool())
    };
    flag("CGSSessionScreenIsLocked") == Some(true) || flag("kCGSSessionOnConsoleKey") == Some(false)
}

/// Turns a Windows date picture into the Unicode one macOS uses, so a config reads the same on
/// both: `ddd` is the short weekday and `dddd` the full one, as on Windows.
fn unicode_picture(windows: &str) -> String {
    let mut out = String::new();
    let mut chars = windows.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\'' {
            out.push(c);
            for q in chars.by_ref() {
                out.push(q);
                if q == '\'' {
                    break;
                }
            }
            continue;
        }
        let mut run = 1;
        while chars.peek() == Some(&c) {
            chars.next();
            run += 1;
        }
        let piece = match (c, run) {
            ('d', 1..=2) | ('M', _) => c.to_string().repeat(run),
            ('d', 3) => "EEE".into(),
            ('d', _) => "EEEE".into(),
            ('y', 1..=2) => "yy".into(),
            ('y', _) => "yyyy".into(),
            ('g', _) => "G".into(),
            // Every other letter is a field in Unicode pictures, and literal on Windows.
            (c, _) if c.is_ascii_alphabetic() => format!("'{}'", c.to_string().repeat(run)),
            (c, _) => c.to_string().repeat(run),
        };
        out.push_str(&piece);
    }
    out
}

/// Formats through macOS, so names come out in the system language. An empty picture uses the
/// system short date.
fn format_date(when: &NSDate, picture: &str) -> String {
    let formatter = NSDateFormatter::new();
    formatter.setLocale(Some(&NSLocale::currentLocale()));
    if picture.is_empty() {
        formatter.setDateStyle(NSDateFormatterStyle::ShortStyle);
        formatter.setTimeStyle(NSDateFormatterStyle::NoStyle);
    } else {
        formatter.setDateFormat(Some(&NSString::from_str(&unicode_picture(picture))));
    }
    formatter.stringFromDate(when).to_string()
}

pub fn now(date_format: &str, weekday_format: &str) -> Now {
    let when = NSDate::now();
    let formatter = NSDateFormatter::new();
    formatter.setLocale(Some(&NSLocale::localeWithLocaleIdentifier(&NSString::from_str("en_US_POSIX"))));
    formatter.setDateFormat(Some(&NSString::from_str("HH:mm:ss")));
    Now {
        time: formatter.stringFromDate(&when).to_string(),
        date: format_date(&when, date_format),
        weekday: capitalised(&format_date(&when, weekday_format)),
    }
}

unsafe extern "C" {
    fn mach_host_self() -> u32;
    fn host_statistics(host: u32, flavor: i32, info: *mut i32, count: *mut u32) -> i32;
}

/// CPU use since the previous call, plus RAM use, both in percent.
#[derive(Default)]
pub struct Stats {
    idle: u64,
    total: u64,
}

fn memory_load() -> u8 {
    unsafe {
        let mut vm: libc::vm_statistics64 = std::mem::zeroed();
        let mut count = libc::HOST_VM_INFO64_COUNT;
        #[allow(deprecated)]
        let host = libc::mach_host_self();
        if libc::host_statistics64(host, libc::HOST_VM_INFO64, &mut vm as *mut _ as *mut i32, &mut count) != 0 {
            return 0;
        }
        let (mut total, mut size) = (0u64, size_of::<u64>());
        if libc::sysctlbyname(c"hw.memsize".as_ptr(), &mut total as *mut _ as *mut _, &mut size, std::ptr::null_mut(), 0) != 0
            || total == 0
        {
            return 0;
        }
        // What Activity Monitor calls memory used: app memory, wired and compressed.
        let pages = (vm.internal_page_count as u64).saturating_sub(vm.purgeable_count as u64)
            + vm.wire_count as u64
            + vm.compressor_page_count as u64;
        let page = libc::sysconf(libc::_SC_PAGESIZE) as u64;
        (pages * page * 100 / total).min(100) as u8
    }
}

impl Stats {
    pub fn sample(&mut self) -> (u8, u8) {
        const HOST_CPU_LOAD_INFO: i32 = 3;
        let mut ticks = [0u32; 4]; // user, system, idle, nice
        let mut count = ticks.len() as u32;
        let read = unsafe { host_statistics(mach_host_self(), HOST_CPU_LOAD_INFO, ticks.as_mut_ptr() as *mut i32, &mut count) };
        let memory = memory_load();
        if read != 0 {
            return (0, memory);
        }
        let idle = ticks[2] as u64;
        let total: u64 = ticks.iter().map(|t| *t as u64).sum();
        let (idle_delta, total_delta) = (idle.wrapping_sub(self.idle), total.wrapping_sub(self.total).max(1));
        (self.idle, self.total) = (idle, total);
        (100 - (idle_delta * 100 / total_delta).min(100) as u8, memory)
    }
}

/// Waits for a process to exit, up to `limit`.
pub fn wait_for_exit(pid: u32, limit: Duration) {
    let start = Instant::now();
    while start.elapsed() < limit && unsafe { libc::kill(pid as i32, 0) } == 0 {
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// macOS is asked on every check, so there is nothing to start.
pub fn watch_display() {}

pub fn display_off() -> bool {
    CGDisplayIsAsleep(CGMainDisplayID())
}

/// How long since the last keyboard or mouse input anywhere on this Mac.
pub fn idle_time() -> Duration {
    let any_input = CGEventType(u32::MAX);
    let seconds = CGEventSource::seconds_since_last_event_type(CGEventSourceStateID::CombinedSessionState, any_input);
    Duration::try_from_secs_f64(seconds).unwrap_or_default()
}

fn names_match(app: &NSRunningApplication, wanted: &str) -> bool {
    let wanted = wanted.trim_end_matches(".exe").trim_end_matches(".app").to_lowercase();
    let path = app_path(app).unwrap_or_default();
    let file = path.rsplit('/').next().unwrap_or_default().trim_end_matches(".app").to_lowercase();
    let name = app.localizedName().map(|n| n.to_string().to_lowercase()).unwrap_or_default();
    file == wanted || name == wanted || path.eq_ignore_ascii_case(&expand(&wanted))
}

/// Asks an app to quit, as Quit in its menu would. Empty closes the focused window instead.
pub fn close(program: &str) -> Result<()> {
    if program.trim().is_empty() {
        return send_hotkey("cmd+w");
    }
    let apps = NSWorkspace::sharedWorkspace().runningApplications();
    let mut found = false;
    for app in apps.iter().filter(|app| names_match(app, program.trim())) {
        found |= app.terminate();
    }
    if !found {
        bail!("could not close {program}");
    }
    Ok(())
}

static SOUND: Mutex<Option<Child>> = Mutex::new(None);

/// Plays through afplay, which handles wav, mp3, m4a and aiff. Replaces any current sound.
pub fn play_sound(path: &str) -> Result<()> {
    stop_sound();
    let child = Command::new("/usr/bin/afplay").arg(expand(path)).stdin(Stdio::null()).stderr(Stdio::null()).spawn();
    *SOUND.lock().unwrap() = Some(child.with_context(|| format!("cannot play {path}"))?);
    Ok(())
}

pub fn stop_sound() {
    if let Some(mut child) = SOUND.lock().unwrap().take() {
        let _ = child.kill();
        let _ = child.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hotkeys_parse_with_windows_names() {
        assert_eq!(parse_hotkey("Ctrl + Shift+F13").unwrap(), vec![0xA2, 0xA0, 0x7C]);
        assert_eq!(parse_hotkey("win+v").unwrap(), vec![0x5B, b'V' as u16]);
        assert_eq!(parse_hotkey("cmd+v").unwrap(), parse_hotkey("win+v").unwrap());
        assert_eq!(parse_hotkey("volume_up").unwrap(), vec![0xAF]);
        assert_eq!(parse_hotkey("ctrl+vkC0").unwrap(), vec![0xA2, 0xC0]);
        for bad in ["", "ctrl+", "f25", "f0", "nope", "vk", "vk00", "vkzz", "vk100", "f21", "ctrl+mute", "media_stop"] {
            assert!(parse_hotkey(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn keys_land_where_they_are_on_a_mac_keyboard() {
        assert_eq!(mac_keys("cmd+shift+a").unwrap(), [Key::Code(0x37), Key::Code(0x38), Key::Code(0x00)]);
        assert_eq!(mac_keys("q").unwrap(), [Key::Code(0x0C)]);
        assert_eq!(mac_keys("z").unwrap(), [Key::Code(0x06)]);
        assert_eq!(mac_keys("m").unwrap(), [Key::Code(0x2E)]);
        assert_eq!(mac_keys("0").unwrap(), [Key::Code(0x1D)]);
        assert_eq!(mac_keys("f5").unwrap(), [Key::Code(0x60)]);
        assert_eq!(mac_keys("enter").unwrap(), [Key::Code(0x24)]);
        assert_eq!(mac_keys("play_pause").unwrap(), [Key::Media(16)]);
        for letter in 'a'..='z' {
            assert!(mac_keys(&letter.to_string()).is_ok(), "{letter}");
        }
        assert_eq!(modifier(0x37), CGEventFlags::MaskCommand.0);
    }

    #[test]
    fn date_pictures_translate() {
        assert_eq!(unicode_picture("dd/MM/yyyy"), "dd/MM/yyyy");
        assert_eq!(unicode_picture("ddd"), "EEE");
        assert_eq!(unicode_picture("dddd, d MMM"), "EEEE, d MMM");
        assert_eq!(unicode_picture("d 'de' MMMM"), "d 'de' MMMM");
        assert_eq!(unicode_picture("yy"), "yy");
        assert_eq!(unicode_picture("dd h"), "dd 'h'");
    }

    #[test]
    fn dates_follow_the_requested_picture() {
        // Noon UTC on 1 November 2026, the same day in every time zone but the farthest.
        let day = NSDate::dateWithTimeIntervalSince1970(1_793_534_400.0);
        assert_eq!(format_date(&day, "dd/MM/yyyy"), "01/11/2026");
        assert_eq!(format_date(&day, "yyyy-MM-dd"), "2026-11-01");
        assert!(!format_date(&day, "").is_empty(), "system short date");
        assert!(!format_date(&day, "ddd").is_empty(), "weekday name");
        assert_eq!(now("yyyy", "ddd").date.len(), 4);
        assert_eq!(now("", "").time.len(), 8);
    }

    #[test]
    fn arguments_keep_quotes_together() {
        assert_eq!(split_args(r#"--a "b c" d"#), ["--a", "b c", "d"]);
        assert_eq!(split_args(r#""""#), [""]);
        assert!(split_args("  ").is_empty());
        assert!(is_url("https://example.com") && is_url("x-apple.systempreferences:com.apple"));
        assert!(!is_url("/Applications/Zed.app") && !is_url("C:\\x"));
    }

    #[test]
    fn app_icons_are_read_from_the_bundle() {
        let png = app_icon_png("/System/Applications/Calculator.app").expect("Calculator has an icon");
        let image = Pixmap::decode_png(&png).unwrap();
        assert_eq!(image.width(), 48);
        assert!(image.pixels().iter().any(|p| p.alpha() > 0), "not blank");
        assert!(app_icon_png("/no/such/App.app").is_none());
    }

    #[test]
    fn system_state_is_readable() {
        assert!(idle_time() < Duration::from_secs(60 * 60 * 24 * 365));
        let mut stats = Stats::default();
        stats.sample();
        std::thread::sleep(Duration::from_millis(200));
        let (cpu, memory) = stats.sample();
        assert!(cpu <= 100 && memory > 0 && memory <= 100);
        let _ = session_locked();
        let _ = display_off();
    }

    #[test]
    fn an_app_has_no_volume_of_its_own() {
        assert!(step_app_volume("/Applications/Safari.app", 5).is_err());
    }
}
