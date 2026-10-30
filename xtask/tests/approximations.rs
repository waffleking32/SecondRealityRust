//! The coupling between `docs/fidelity/APPROXIMATIONS.md` and the code: every live entry carries
//! the five labels, the entries are numbered in increasing order, and every `register` string
//! `cargo xtask rediff` prints names a live entry. A tombstone (`## A<n> — Retired: …` or
//! `## A<n> — Closed: …`) keeps its number so citations resolve, and carries no labels.

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

/// Whether the entry is a tombstone: `## A10 — Retired: …` or `## A9 — Closed: …`.
fn is_tombstone(heading: &str) -> bool {
    heading.contains("— Retired:") || heading.contains("— Closed:")
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

#[test]
fn every_live_entry_carries_the_five_labels() {
    let text = register();
    let mut missing = Vec::new();
    for (heading, body) in entries(&text).iter().filter(|(h, _)| !is_tombstone(h)) {
        for names in LABELS {
            if !names.iter().any(|name| has_label(body, name)) {
                missing.push(format!("{heading}: no **{}", names[0]));
            }
        }
    }
    assert!(missing.is_empty(), "entries without a label:\n{}", missing.join("\n"));
}

#[test]
fn the_entries_are_numbered_in_increasing_order() {
    let text = register();
    let numbers: Vec<u32> = entries(&text).iter().map(|(h, _)| number(h)).collect();
    assert!(!numbers.is_empty(), "the register has no entries");
    for pair in numbers.windows(2) {
        assert!(pair[0] < pair[1], "A{} follows A{}", pair[1], pair[0]);
    }
}

#[test]
fn every_rediff_register_names_a_live_entry() {
    let text = register();
    let entries = entries(&text);
    for entry in table::TABLE {
        for frame in entry.frames {
            let register = frame.register;
            if register == "exact" {
                continue;
            }
            let n: u32 = register
                .strip_prefix('A')
                .and_then(|digits| digits.parse().ok())
                .unwrap_or_else(|| panic!("{}: `{register}` names no entry", entry.capture));
            let (heading, _) = entries
                .iter()
                .find(|(h, _)| number(h) == n)
                .unwrap_or_else(|| panic!("{}: A{n} is not in the register", entry.capture));
            assert!(!is_tombstone(heading), "{}: A{n} is a tombstone: {heading}", entry.capture);
        }
    }
}
