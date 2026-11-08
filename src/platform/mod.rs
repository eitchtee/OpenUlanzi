//! The operating system: input injection, launching, focused window, lock state, sound.
//! Each system has its own module with the same functions; the shared pieces live here.
#[cfg(target_os = "macos")]
mod mac;
#[cfg(windows)]
mod win;
#[cfg(target_os = "macos")]
pub use mac::*;
#[cfg(windows)]
pub use win::*;

#[derive(Debug, Clone)]
pub struct Window {
    /// The program's full path. On macOS this is the app bundle when there is one.
    pub exe: String,
    pub title: String,
}

/// Fields the firmware's digital clock faces display.
pub struct Now {
    pub time: String,
    pub date: String,
    pub weekday: String,
}

/// Whether a focused program is this app, which must never pick a profile for itself.
pub fn is_own(exe: &str) -> bool {
    let name = exe.rsplit(['\\', '/']).next().unwrap_or(exe).to_ascii_lowercase();
    ["openulanzi-app.exe", "openulanzi-app", "openulanzi.app"].contains(&name.as_str())
}

/// Like the Run dialog: a quoted program followed by its arguments, a path with spaces taken
/// whole when it exists, and otherwise the first word as the program.
#[cfg_attr(not(any(windows, target_os = "macos")), allow(dead_code))]
fn split_command(command: &str, exists: impl Fn(&str) -> bool) -> (&str, &str) {
    if let Some((file, args)) = command.strip_prefix('"').and_then(|rest| rest.split_once('"')) {
        return (file, args.trim_start());
    }
    if exists(command) {
        return (command, "");
    }
    command.split_once(char::is_whitespace).map_or((command, ""), |(file, args)| (file, args.trim_start()))
}

/// Steps a 0-1 volume by whole percents, so repeated presses land on round numbers.
#[cfg_attr(not(windows), allow(dead_code))]
fn stepped(level: f32, percent: i32) -> f32 {
    ((level * 100.0).round() as i32 + percent).clamp(0, 100) as f32 / 100.0
}

fn capitalised(text: &str) -> String {
    let mut letters = text.chars();
    letters.next().map_or_else(String::new, |first| first.to_uppercase().chain(letters).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_split_like_the_run_dialog() {
        let none = |_: &str| false;
        assert_eq!(split_command("notepad", none), ("notepad", ""));
        assert_eq!(split_command("notepad  a b.txt", none), ("notepad", "a b.txt"));
        assert_eq!(split_command(r#""C:\My App\x.exe" --flag"#, none), (r"C:\My App\x.exe", "--flag"));
        assert_eq!(split_command(r"C:\My Scripts\a b.ahk", |_| true), (r"C:\My Scripts\a b.ahk", ""));
        assert_eq!(split_command(r"shell:AppsFolder\Microsoft.WindowsCalculator_8wekyb3d8bbwe!App", none).1, "");
        assert_eq!(split_command("/Applications/Visual Studio Code.app", |_| true).1, "");
    }

    #[test]
    fn volume_steps_stay_on_round_numbers_and_in_range() {
        assert_eq!(stepped(0.33, 5), 0.38);
        assert_eq!(stepped(0.97, 5), 1.0);
        assert_eq!(stepped(0.02, -5), 0.0);
    }

    #[test]
    fn names_capitalise() {
        assert_eq!(capitalised("dom"), "Dom");
        assert_eq!(capitalised(""), "");
    }

    #[test]
    fn this_app_is_recognised() {
        assert!(is_own(r"C:\Program Files\OpenUlanzi\openulanzi-app.exe"));
        assert!(is_own("/Applications/OpenUlanzi.app"));
        assert!(!is_own("/Applications/Zed.app"));
    }
}
