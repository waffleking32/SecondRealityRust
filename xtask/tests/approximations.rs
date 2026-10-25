//! `docs/fidelity/APPROXIMATIONS.md` keeps the conventions Phase 9 Task 10 gave it. This checks
//! the conventions, not completeness: whether every knowing difference is registered, and whether
//! an entry names an open item in words [`OPEN_ITEM`] does not list, is for a reader.
//!
//! - **Every entry carries the five labels** — `**Part`, `**Original`, `**Port`, `**Why` and
//!   `**Measured difference` at the start of a line, each followed by something other than a
//!   letter (`**Original.**`, `**Port (Phase 9 Task 6b).**`, `**Why — D24's procedure.**`).
//! - **Every entry that names an open item has a status line**, `**Status (Phase 9):**`, saying
//!   whether it is closed or carried and why. An open item is named by one of [`OPEN_ITEM`]'s
//!   phrases; an entry may carry a status line without one.
//! - **Every register string `cargo xtask rediff` prints names an entry that exists**, and none
//!   still points at Task 10 — the strings Task 5 left as "(Task 10: restate)".
//!
//! And A1's derivation, which this file holds because the register cites it: the overscan border
//! the parts' CRTC timing gives, in source pixels, is not whole at every geometry, so the
//! presenter's pad stays a presentation choice (`crates/sr-engine/src/present.rs`'s
//! `BORDER_PX`).

use std::path::PathBuf;

#[allow(dead_code)]
#[path = "../src/rediff/table.rs"]
mod table;

fn register() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../docs/fidelity/APPROXIMATIONS.md");
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// The register's entries: each `## A<n>` heading and the text up to the next `## ` heading.
fn entries(text: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let mut current: Option<(String, String)> = None;
    for line in text.lines() {
        if let Some(title) = line.strip_prefix("## ") {
            if let Some(entry) = current.take() {
                out.push(entry);
            }
            let is_entry = title
                .strip_prefix('A')
                .and_then(|rest| rest.chars().next())
                .is_some_and(|c| c.is_ascii_digit());
            if is_entry {
                current = Some((line.to_string(), String::new()));
            }
            continue;
        }
        if let Some((_, body)) = current.as_mut() {
            body.push_str(line);
            body.push('\n');
        }
    }
    out.extend(current);
    out
}

/// The entry's number: `## A14 — …` is 14.
fn number(heading: &str) -> u32 {
    heading[4..]
        .chars()
        .take_while(char::is_ascii_digit)
        .collect::<String>()
        .parse()
        .unwrap()
}

/// Whether `body` has a line beginning `**<label>` followed by a non-letter.
fn has_label(body: &str, label: &str) -> bool {
    body.lines().any(|line| {
        line.strip_prefix("**")
            .and_then(|rest| rest.strip_prefix(label))
            .and_then(|rest| rest.chars().next())
            .is_some_and(|next| !next.is_alphabetic())
    })
}

const LABELS: [&[&str]; 5] = [
    &["Part", "Parts"],
    &["Original"],
    &["Port"],
    &["Why"],
    &["Measured difference"],
];

/// The phrases that name an open item. Lower-case; matched against the lower-cased entry.
const OPEN_ITEM: &[&str] = &[
    "unmeasured",
    "not measured",
    "unresolved",
    "unexplained",
    "not established",
    "unknown",
    "unpacker",
    "would settle",
    "would close",
    "cannot close",
    "should watch",
    "hypothesis",
    "open item",
    "still open",
    "remains open",
    "left open",
    "is open",
    "are open",
    "stays open",
];

const STATUS: &str = "**Status (Phase 9):**";

#[test]
fn every_entry_carries_the_five_labels() {
    let text = register();
    let entries = entries(&text);
    assert!(entries.len() >= 63, "the register has {} entries", entries.len());
    let mut missing = Vec::new();
    for (heading, body) in &entries {
        for names in LABELS {
            if !names.iter().any(|name| has_label(body, name)) {
                missing.push(format!("{heading}: no **{}", names[0]));
            }
        }
    }
    assert!(missing.is_empty(), "entries without a label:\n{}", missing.join("\n"));
}

#[test]
fn every_entry_that_names_an_open_item_has_a_status_line() {
    let text = register();
    let mut missing = Vec::new();
    for (heading, body) in entries(&text) {
        let lower = body.to_lowercase();
        let named: Vec<&str> = OPEN_ITEM.iter().copied().filter(|p| lower.contains(p)).collect();
        if !named.is_empty() && !body.contains(STATUS) {
            missing.push(format!("{heading}: names {named:?}"));
        }
        for line in body.lines().filter(|line| line.contains(STATUS)) {
            let after = line.split(STATUS).nth(1).unwrap().trim_start();
            assert!(
                after.starts_with("Closed") || after.starts_with("Carried"),
                "{heading}: a status line says neither Closed nor Carried: {line}"
            );
        }
    }
    assert!(missing.is_empty(), "open items without a status:\n{}", missing.join("\n"));
}

#[test]
fn the_entries_are_numbered_in_order_without_a_gap() {
    let text = register();
    let numbers: Vec<u32> = entries(&text).iter().map(|(h, _)| number(h)).collect();
    let expected: Vec<u32> = (1..=numbers.len() as u32).collect();
    assert_eq!(numbers, expected);
}

#[test]
fn every_rediff_register_names_an_entry_that_exists() {
    let text = register();
    let numbers: Vec<u32> = entries(&text).iter().map(|(h, _)| number(h)).collect();
    for entry in table::TABLE {
        for frame in entry.frames {
            let register = frame.register;
            assert!(
                !register.contains("Task 10"),
                "{} {}: the register still points at Task 10: {register}",
                entry.capture,
                frame.reference
            );
            if register == "exact" {
                continue;
            }
            let digits: String = register
                .strip_prefix('A')
                .unwrap_or_else(|| panic!("{}: `{register}` names no entry", entry.capture))
                .chars()
                .take_while(char::is_ascii_digit)
                .collect();
            let n: u32 = digits
                .parse()
                .unwrap_or_else(|_| panic!("{}: `{register}` names no entry", entry.capture));
            assert!(numbers.contains(&n), "{}: A{n} is not in the register", entry.capture);
        }
    }
}

/// A1's derivation (`APPROXIMATIONS.md` A1), in the oracle's own arithmetic.
///
/// Every part's screen is the BIOS's mode 13h or 0Eh with only 0x09, 0x0C/0x0D, 0x13, 0x14, 0x17,
/// 0x18 and PLZPART's timing-preserving 0x07 rewritten (the entry cites each), so the timing
/// registers are what the oracle's `int10_modes.cpp` writes for its table row `htotal 100,
/// hdispend 80, vtotal 449, vdispend 400` (`:40`, `:45`, programmed at `:1352-1460`), read back
/// the way its `vga_draw.cpp:2001-2095` reads them. The border is the displayed-but-not-active
/// interval on each side, and a source pixel is 4 (320-wide) or 8 (640-wide) dots of a character
/// across and 2 (200-row) or 1 (400-row) lines down.
#[test]
fn a1_the_derived_border_is_not_whole_at_every_geometry() {
    let (htotal, hdispend, vtotal, vdispend): (u32, u32, u32, u32) = (100, 80, 449, 400);

    // `int10_modes.cpp:1352-1460`: the registers the BIOS mode set writes.
    let cr00 = htotal - 5;
    let cr01 = hdispend - 1;
    let cr02 = hdispend;
    let blank_end = (htotal - 2) & 0x7f;
    let cr03 = 0x80 | (blank_end & 0x1f);
    let cr05_bit7 = (blank_end & 0x20) << 2;
    let cr06 = (vtotal - 2) & 0xff;
    let vblank_trim = 6; // `case 400: vblank_trim=6;`, `:1435`
    let cr12 = (vdispend - 1) & 0xff;
    let cr15 = (vdispend + vblank_trim) & 0xff;
    let cr16 = (vtotal - vblank_trim - 2) & 0xff;
    let overflow = (((vtotal - 2) & 0x100) >> 8)
        | (((vdispend - 1) & 0x100) >> 7)
        | (((vdispend + vblank_trim) & 0x100) >> 5);
    assert_eq!((cr00, cr01, cr02, cr03, cr05_bit7), (95, 79, 80, 0x82, 0x80));
    assert_eq!((cr06, cr12, cr15, cr16), (0xbf, 0x8f, 0x96, 0xb9));

    // `vga_draw.cpp:2001-2095`: how the oracle reads them back.
    let h_total = cr00 + 3 + 2;
    let h_display_end = cr01 + 1;
    let h_blank_start = cr02;
    let h_blank_end_raw = (cr03 & 0x1f) | (cr05_bit7 >> 2);
    let h_blank_end = h_blank_start + (h_blank_end_raw.wrapping_sub(h_blank_start) & 0x3f);
    let v_total = (cr06 | ((overflow & 1) << 8)) + 2;
    let v_display_end = (cr12 | ((overflow & 2) << 7)) + 1;
    let v_blank_start = (cr15 | ((overflow & 8) << 5)) + 1;
    let v_blank_end = v_blank_start + ((cr16 & 0x7f).wrapping_sub(v_blank_start) & 0x7f) + 1;
    assert_eq!((h_total, h_display_end, h_blank_start, h_blank_end), (100, 80, 80, 98));
    assert_eq!((v_total, v_display_end, v_blank_start, v_blank_end), (449, 400, 407, 442));

    // The border, in characters and lines.
    let (left, right) = (h_total - h_blank_end, h_blank_start - h_display_end);
    let (top, bottom) = (v_total - v_blank_end, v_blank_start - v_display_end);
    assert_eq!((left, right, top, bottom), (2, 0, 7, 7));

    // In source pixels at each geometry: (dots a character, lines a row).
    let geometries = [("320x200", 4.0, 2.0), ("320x400", 4.0, 1.0), ("640x400", 8.0, 1.0)];
    let sides: Vec<(&str, [f64; 4])> = geometries
        .iter()
        .map(|&(name, per_char, per_row)| {
            (
                name,
                [
                    f64::from(left) * per_char,
                    f64::from(right) * per_char,
                    f64::from(top) / per_row,
                    f64::from(bottom) / per_row,
                ],
            )
        })
        .collect();
    for (name, [l, r, t, b]) in &sides {
        println!("{name}: left {l}, right {r}, top {t}, bottom {b}");
    }
    assert_eq!(sides[0].1, [8.0, 0.0, 3.5, 3.5]);
    assert_eq!(sides[1].1, [8.0, 0.0, 7.0, 7.0]);
    assert_eq!(sides[2].1, [16.0, 0.0, 7.0, 7.0]);
    let whole_everywhere = sides
        .iter()
        .all(|(_, values)| values.iter().all(|v| v.fract() == 0.0));
    assert!(!whole_everywhere, "the derived border is not whole at 320x200, so BORDER_PX stays 8");
}
