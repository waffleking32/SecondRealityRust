//! The vault: the three unreleased development effects, ported standalone.
//!
//! GRID, HARD and TWIST never shipped inside `SECOND.EXE`; each was its own
//! program (`GRID/P.EXE`, `HARD/B.EXE`, `TWIST/T.EXE`) run behind the
//! standalone `DIS/DIS.EXE`. They are not in the demo's run order
//! (`shell::plan::PART_ORDER`), carry no boot anchor, and run silent: the captures
//! that file them boot the shipped DIS with one start key and no music
//! (`docs/fidelity/reference-indices.md` §24).

pub mod grid;
pub mod hard;
pub mod twist;

/// The vault menu's rows ([`crate::menu::vault`]), in menu order: the registry
/// name, so a row and `--part <name>` start the same program, and its label.
pub const VAULT: [(&str, &str); 3] = [("grid", "GRID"), ("hard", "HARD"), ("twist", "TWIST")];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_vault_row_is_a_registered_part() {
        for (name, _) in VAULT {
            assert_eq!(crate::parts::registered(name), Some(name));
        }
    }
}
