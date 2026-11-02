//! The HID handle. Only one program can hold it, so Ulanzi Studio must be closed.
use crate::protocol::{self, Input, PACKET};
use anyhow::{anyhow, bail, Result};
use hidapi::{HidApi, HidDevice};
use std::collections::HashMap;
use std::ffi::CString;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;
use windows::core::{HSTRING, PCWSTR};
use windows::Win32::Devices::DeviceAndDriverInstallation::{
    CM_Get_Device_Interface_ListW, CM_Get_Device_Interface_List_SizeW, CM_GET_DEVICE_INTERFACE_LIST_PRESENT, CR_SUCCESS,
};
use windows::Win32::Devices::HumanInterfaceDevice::{HidD_GetHidGuid, HidD_GetSerialNumberString};
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};

pub struct Deck {
    device: HidDevice,
    pub last_write: Instant,
    /// A bundle ack arrived since the last bundle was sent.
    pub acked: bool,
}

/// Stable names for decks, safe in file names. A serial is the name; decks with none, or with
/// the same one, are told apart by order. Input must be in a fixed order for that to hold.
fn name_decks(serials: Vec<Option<String>>) -> Vec<String> {
    let mut seen: HashMap<String, usize> = HashMap::new();
    let names = serials.into_iter().map(|serial| {
        let clean: String = serial.unwrap_or_default().chars().filter(char::is_ascii_alphanumeric).collect();
        let base = if clean.is_empty() { "deck".to_string() } else { clean };
        let count = seen.entry(base.clone()).or_default();
        *count += 1;
        if *count == 1 { base } else { format!("{base}-{count}") }
    });
    names.collect()
}

/// The control side of a deck. It also appears as a boot keyboard on interface 1.
fn is_deck(path: &str) -> bool {
    let wanted = format!("vid_{:04x}&pid_{:04x}&mi_00", protocol::VID, protocol::PID);
    path.to_ascii_lowercase().contains(&wanted)
}

/// Device paths from Windows' own list. This opens nothing, so it sends nothing to any device:
/// hidapi's scan asks every HID device for its strings, which can restart keyboards when repeated.
fn hid_paths() -> Result<Vec<String>> {
    unsafe {
        let guid = HidD_GetHidGuid();
        let mut size = 0;
        let flags = CM_GET_DEVICE_INTERFACE_LIST_PRESENT;
        if CM_Get_Device_Interface_List_SizeW(&mut size, &guid, PCWSTR::null(), flags) != CR_SUCCESS {
            bail!("cannot size the device list");
        }
        let mut list = vec![0u16; size as usize];
        if CM_Get_Device_Interface_ListW(&guid, PCWSTR::null(), &mut list, flags) != CR_SUCCESS {
            bail!("cannot read the device list");
        }
        Ok(list.split(|c| *c == 0).filter(|p| !p.is_empty()).map(String::from_utf16_lossy).collect())
    }
}

fn read_serial(path: &str) -> Option<String> {
    unsafe {
        let handle = CreateFileW(
            &HSTRING::from(path),
            0, // no access: enough to ask for strings, and it can't disturb the open connection
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_FLAGS_AND_ATTRIBUTES(0),
            None,
        )
        .ok()?;
        let mut text = [0u16; 64];
        let read = HidD_GetSerialNumberString(handle, text.as_mut_ptr() as *mut _, size_of_val(&text) as u32);
        let _ = CloseHandle(handle);
        let len = text.iter().position(|c| *c == 0).unwrap_or(text.len());
        read.then(|| String::from_utf16_lossy(&text[..len]))
    }
}

/// Serials never change, so each deck is asked once rather than on every scan.
fn serial_of(path: &str) -> Option<String> {
    static SERIALS: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();
    let mut known = SERIALS.get_or_init(Mutex::default).lock().unwrap();
    if let Some(serial) = known.get(path) {
        return Some(serial.clone());
    }
    let serial = read_serial(path)?;
    known.insert(path.to_string(), serial.clone());
    Some(serial)
}

fn connected() -> Result<Vec<(String, String)>> {
    let mut paths: Vec<String> = hid_paths()?.into_iter().filter(|p| is_deck(p)).collect();
    paths.sort();
    let names = name_decks(paths.iter().map(|p| serial_of(p)).collect());
    Ok(names.into_iter().zip(paths).collect())
}

/// hidapi can only open a device through a context, and creating one scans every HID device, with
/// no working way to turn that off on Windows. So there is one context, made once.
fn context() -> Result<&'static Mutex<HidApi>> {
    static CONTEXT: OnceLock<Mutex<HidApi>> = OnceLock::new();
    if let Some(context) = CONTEXT.get() {
        return Ok(context);
    }
    let api = HidApi::new()?;
    Ok(CONTEXT.get_or_init(|| Mutex::new(api)))
}

/// Names of the decks plugged in right now.
pub fn list() -> Result<Vec<String>> {
    Ok(connected()?.into_iter().map(|(name, _)| name).collect())
}

impl Deck {
    /// Opens the named deck, or the first one found.
    pub fn open(name: Option<&str>) -> Result<Self> {
        let decks = connected()?;
        let found = decks.iter().find(|(id, _)| name.is_none_or(|n| n == id));
        let (_, path) = found.ok_or_else(|| anyhow!("deck not found"))?;
        let device = context()?.lock().unwrap().open_path(&CString::new(path.as_str())?)?;
        Ok(Self { device, last_write: Instant::now(), acked: false })
    }

    fn write(&mut self, packet: &[u8]) -> Result<()> {
        // Windows needs a leading report id; reads come back without it.
        let mut report = Vec::with_capacity(PACKET + 1);
        report.push(0);
        report.extend_from_slice(packet);
        self.device.write(&report)?;
        self.last_write = Instant::now();
        Ok(())
    }

    pub fn command(&mut self, cmd: u16, payload: &[u8]) -> Result<()> {
        self.write(&protocol::command(cmd, payload))
    }

    pub fn bundle(&mut self, zip: &[u8], full: bool) -> Result<()> {
        self.acked = false;
        protocol::bundle_packets(zip, full).iter().try_for_each(|p| self.write(p))
    }

    /// Ok(None) on timeout or on a report that isn't input (acks, device info).
    pub fn read(&mut self, timeout_ms: i32) -> Result<Option<Input>> {
        let mut buffer = [0u8; PACKET];
        let len = self.device.read_timeout(&mut buffer, timeout_ms)?;
        self.acked |= protocol::is_ack(&buffer[..len]);
        Ok(protocol::parse_input(&buffer[..len]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(serials: &[Option<&str>]) -> Vec<String> {
        name_decks(serials.iter().map(|s| s.map(str::to_string)).collect())
    }

    #[test]
    fn decks_get_unique_file_safe_names() {
        assert_eq!(names(&[Some("ABC123"), Some("99B")]), ["ABC123", "99B"]);
        assert_eq!(names(&[Some("SAME"), Some("SAME"), Some("SAME")]), ["SAME", "SAME-2", "SAME-3"]);
        assert_eq!(names(&[None, Some(""), Some("OK")]), ["deck", "deck-2", "OK"]);
        assert_eq!(names(&[Some(r"..\evil/name")]), ["evilname"]);
    }

    #[test]
    fn only_the_control_side_of_a_deck_counts() {
        assert!(is_deck(r"\\?\HID#VID_2207&PID_0019&MI_00#b&1475a410&0&0000#{4d1e55b2}"));
        assert!(!is_deck(r"\\?\HID#VID_2207&PID_0019&MI_01#b&2c393055&0&0000#{4d1e55b2}\KBD"));
        assert!(!is_deck(r"\\?\HID#VID_1507&PID_2814&MI_00#b&2d2e23b7&0&0000#{4d1e55b2}"));
    }

    #[test]
    fn windows_lists_hid_devices() {
        assert!(!hid_paths().unwrap().is_empty());
    }

    #[test]
    fn listing_works_while_others_do_the_same() {
        let threads: Vec<_> = (0..4).map(|_| std::thread::spawn(list)).collect();
        for thread in threads {
            assert!(thread.join().unwrap().is_ok());
        }
    }
}
