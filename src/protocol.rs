//! Ulanzi D200-family wire format. Pure: no I/O, so it is testable without a deck.
use anyhow::{bail, Result};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io::{Cursor, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};

pub const VID: u16 = 0x2207;
pub const PID: u16 = 0x0019;
pub const PACKET: usize = 1024;
const FIRST_DATA: usize = PACKET - 8;
const HEADER: [u8; 2] = [0x7c, 0x7c];

const CMD_BUTTONS: u16 = 0x0001;
const CMD_PARTIAL_BUTTONS: u16 = 0x000D;
pub const CMD_SMALL_WINDOW: u16 = 0x0006;
pub const CMD_BRIGHTNESS: u16 = 0x000A;
pub const CMD_LOCK: u16 = 0x000F;
pub const CMD_UNLOCK: u16 = 0x0010;
const CMD_INPUT: u16 = 0x0101;
const CMD_INPUT_ALT: u16 = 0x0102;
const CMD_ACK: u16 = 0x010B;

pub const WIDE: u8 = 13;
const COLS: u8 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Knob {
    Click,
    Left,
    Right,
    PressLeft,
    PressRight,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Input {
    Key { index: u8, down: bool },
    Knob { index: u8, gesture: Knob },
}

fn header(cmd: u16, len: usize) -> Vec<u8> {
    let mut p = vec![0u8; PACKET];
    p[..2].copy_from_slice(&HEADER);
    p[2..4].copy_from_slice(&cmd.to_be_bytes());
    p[4..8].copy_from_slice(&(len as u32).to_le_bytes());
    p
}

/// Single-packet command. Payloads here are short ASCII strings, always under 1016 bytes.
pub fn command(cmd: u16, payload: &[u8]) -> Vec<u8> {
    let mut p = header(cmd, payload.len());
    p[8..8 + payload.len()].copy_from_slice(payload);
    p
}

/// A header packet with the first 1016 bytes, then raw zero-padded chunks. `full` replaces the
/// whole deck; otherwise only the cells in the bundle change.
pub fn bundle_packets(zip: &[u8], full: bool) -> Vec<Vec<u8>> {
    let (first, rest) = zip.split_at(zip.len().min(FIRST_DATA));
    let mut head = header(if full { CMD_BUTTONS } else { CMD_PARTIAL_BUTTONS }, zip.len());
    head[8..8 + first.len()].copy_from_slice(first);
    let mut out = vec![head];
    out.extend(rest.chunks(PACKET).map(|c| {
        let mut p = c.to_vec();
        p.resize(PACKET, 0);
        p
    }));
    out
}

fn boundary_safe(zip: &[u8]) -> bool {
    (FIRST_DATA..zip.len())
        .step_by(PACKET)
        .all(|i| zip[i] != 0x00 && zip[i] != 0x7c)
}

fn zip_cells(cells: &[(u8, &[u8])], wide: Wide, bundle: u64, padding: usize) -> Result<Vec<u8>> {
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    // Stored, so each attempt moves everything after it by exactly one byte.
    let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    zip.start_file("dummy.txt", stored)?;
    zip.write_all(&vec![b'x'; padding])?;

    // Every cell of every bundle gets its own file. The firmware keeps icons by name, so a reused
    // name can vanish with the cell that had it: moving an icon would leave both cells empty.
    let add_icon = |zip: &mut ZipWriter<Cursor<Vec<u8>>>, index: u8, png: &[u8]| -> Result<String> {
        let mut h = DefaultHasher::new();
        png.hash(&mut h);
        let name = format!("icons/{:016x}_{bundle}_{index}.png", h.finish());
        zip.start_file(&name, stored)?;
        zip.write_all(png)?;
        Ok(name)
    };

    let mut manifest = Vec::new();
    for (index, png) in cells {
        let name = add_icon(&mut zip, *index, png)?;
        manifest.push(format!(
            r#""{}_{}":{{"State":0,"ViewParam":[{{"Icon":"{name}","Text":""}}]}}"#,
            index % COLS,
            index / COLS
        ));
    }
    // Every bundle names this cell: without its mode the firmware forgets its width and leaves
    // pieces of the old screen behind. An icon here would also sit under a firmware clock.
    let icon = match wide.image {
        Some(png) => format!(r#""Icon":"{}","#, add_icon(&mut zip, WIDE, png)?),
        None => String::new(),
    };
    manifest.push(format!(
        r#""{}_{}":{{"State":0,"SmallViewMode":{},"ViewParam":[{{{icon}"Text":""}}]}}"#,
        WIDE % COLS,
        WIDE / COLS,
        wide.mode
    ));

    zip.start_file(
        "manifest.json",
        SimpleFileOptions::default().compression_method(CompressionMethod::Deflated),
    )?;
    write!(zip, "{{{}}}", manifest.join(","))?;
    Ok(zip.finish()?.into_inner())
}

/// The wide cell's state, which goes in every bundle.
#[derive(Clone, Copy)]
pub struct Wide<'a> {
    pub mode: u16,
    pub image: Option<&'a [u8]>,
}

/// ZIP bundle repainting only `cells`. Re-pads until no packet boundary holds a byte the firmware
/// misparses (0x00 or 0x7c).
pub fn build_bundle(cells: &[(u8, &[u8])], wide: Wide) -> Result<Vec<u8>> {
    static BUNDLES: AtomicU64 = AtomicU64::new(0);
    let bundle = BUNDLES.fetch_add(1, Ordering::Relaxed);
    for padding in 1..=256 {
        let zip = zip_cells(cells, wide, bundle, padding)?;
        if boundary_safe(&zip) {
            return Ok(zip);
        }
    }
    bail!("no safe bundle padding found for {} cells", cells.len())
}

/// Payload for CMD_SMALL_WINDOW. `suffix` holds the date and/or weekday digital modes display.
pub fn small_window(mode: u16, cpu: u8, ram: u8, time: &str, twelve_hour: bool, suffix: &str) -> Vec<u8> {
    let format = if twelve_hour { "12H" } else { "24H" };
    format!("{mode}|{cpu}|{ram}|{time}|0|{format}|{suffix}").into_bytes()
}

/// The firmware finished a bundle.
pub fn is_ack(report: &[u8]) -> bool {
    report.len() >= 4 && report[..2] == HEADER && u16::from_be_bytes([report[2], report[3]]) == CMD_ACK
}

pub fn parse_input(report: &[u8]) -> Option<Input> {
    if report.len() < 12 || report[..2] != HEADER {
        return None;
    }
    let cmd = u16::from_be_bytes([report[2], report[3]]);
    if cmd != CMD_INPUT && cmd != CMD_INPUT_ALT {
        return None;
    }
    let (index, kind, value) = (report[9], report[10], report[11]);
    match (kind, value) {
        (1, v) => Some(Input::Key { index, down: v == 1 }),
        // A knob click arrives as 1 then 0 on release, so only the 1 counts.
        (2, 1) => Some(Input::Knob { index, gesture: Knob::Click }),
        (2, 2) => Some(Input::Knob { index, gesture: Knob::Left }),
        (2, 3) => Some(Input::Knob { index, gesture: Knob::Right }),
        (2, 4) => Some(Input::Knob { index, gesture: Knob::PressLeft }),
        (2, 5) => Some(Input::Knob { index, gesture: Knob::PressRight }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    fn report(cmd: u16, body: [u8; 4]) -> Vec<u8> {
        let mut p = header(cmd, 4);
        p[8..12].copy_from_slice(&body);
        p
    }

    #[test]
    fn command_layout() {
        let p = command(CMD_BRIGHTNESS, b"80");
        assert_eq!(p.len(), PACKET);
        assert_eq!(&p[..10], &[0x7c, 0x7c, 0x00, 0x0a, 2, 0, 0, 0, b'8', b'0']);
    }

    #[test]
    fn packets_reassemble_to_the_zip() {
        let zip: Vec<u8> = (0..3000u32).map(|i| (i % 251) as u8 + 1).collect();
        let packets = bundle_packets(&zip, false);
        assert_eq!(&packets[0][2..4], &[0x00, 0x0d], "partial command");
        assert_eq!(&bundle_packets(&zip, true)[0][2..4], &[0x00, 0x01], "full command");
        assert_eq!(packets.len(), 3);
        assert!(packets.iter().all(|p| p.len() == PACKET));
        assert_eq!(u32::from_le_bytes(packets[0][4..8].try_into().unwrap()), 3000);
        let mut joined = packets[0][8..].to_vec();
        joined.extend(packets[1..].iter().flatten());
        assert_eq!(&joined[..3000], &zip[..]);
    }

    #[test]
    fn bundle_is_safe_and_names_only_given_cells() {
        // Pseudo-random, incompressible payloads of many sizes hit the boundary bug often.
        let mut x = 7u32;
        for size in (500..9000).step_by(431) {
            let png: Vec<u8> = (0..size)
                .map(|_| {
                    x = x.wrapping_mul(1664525).wrapping_add(1013904223);
                    (x >> 24) as u8
                })
                .collect();
            let zip = build_bundle(&[(0, &png), (9, &png)], Wide { mode: 2, image: Some(&png) }).unwrap();
            assert!(boundary_safe(&zip));

            let mut archive = zip::ZipArchive::new(Cursor::new(&zip)).unwrap();
            assert_eq!(archive.len(), 5, "dummy, manifest and one file per cell");
            let mut manifest = String::new();
            archive.by_name("manifest.json").unwrap().read_to_string(&mut manifest).unwrap();
            for cell in [r#""0_0""#, r#""4_1""#, r#""3_2""#] {
                assert!(manifest.contains(cell), "{manifest}");
            }
            assert_eq!(manifest.matches("State").count(), 3);
            assert_eq!(manifest.matches(r#""SmallViewMode":2"#).count(), 1, "only the wide cell has a mode");
            assert!(manifest.contains(r#""3_2":{"State":0,"SmallViewMode":2,"ViewParam":[{"Icon""#));
        }
    }

    #[test]
    fn every_bundle_names_the_wide_cell_and_a_clock_has_no_image() {
        let png = [1u8; 300];
        let zip = build_bundle(&[(0, &png)], Wide { mode: 203, image: None }).unwrap();
        let mut archive = zip::ZipArchive::new(Cursor::new(&zip)).unwrap();
        let mut manifest = String::new();
        archive.by_name("manifest.json").unwrap().read_to_string(&mut manifest).unwrap();
        assert!(manifest.contains(r#""3_2":{"State":0,"SmallViewMode":203,"ViewParam":[{"Text":""}]}"#), "{manifest}");
    }

    #[test]
    fn a_moved_icon_never_reuses_a_file_name() {
        let names = |zip: &[u8]| -> Vec<String> {
            let archive = zip::ZipArchive::new(Cursor::new(zip)).unwrap();
            archive.file_names().flatten().map(|n| n.to_string()).filter(|n| n.starts_with("icons/")).collect()
        };
        let wide = Wide { mode: 203, image: None };
        let png = [1u8; 300];
        let first = names(&build_bundle(&[(0, &png)], wide).unwrap());
        let moved = names(&build_bundle(&[(1, &png)], wide).unwrap());
        let same_cell = names(&build_bundle(&[(0, &png)], wide).unwrap());
        assert_ne!(first, moved);
        assert_ne!(first, same_cell, "a later bundle is new even for the same cell");
        let shared = names(&build_bundle(&[(0, &png), (1, &png)], wide).unwrap());
        assert_eq!(shared.len(), 2, "cells never share a file");
    }

    #[test]
    fn acks_are_recognised() {
        assert!(is_ack(&report(0x010B, [0; 4])));
        assert!(!is_ack(&report(CMD_INPUT, [0; 4])));
        assert!(!is_ack(&[0x7c, 0x7c]));
    }

    #[test]
    fn inputs() {
        let key = |body| parse_input(&report(CMD_INPUT, body));
        assert_eq!(key([1, 6, 1, 1]), Some(Input::Key { index: 6, down: true }));
        assert_eq!(key([0xc8, 13, 1, 0]), Some(Input::Key { index: 13, down: false }));
        assert_eq!(key([1, 18, 2, 3]), Some(Input::Knob { index: 18, gesture: Knob::Right }));
        assert_eq!(key([1, 19, 2, 4]), Some(Input::Knob { index: 19, gesture: Knob::PressLeft }));
        assert_eq!(key([1, 17, 2, 0]), None, "click release is not a second click");
        assert_eq!(parse_input(&report(0x010B, [1, 6, 1, 1])), None, "ack is not input");
    }
}
