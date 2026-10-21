//! The vault: the three unreleased development effects, ported standalone.
//!
//! GRID, HARD and TWIST never shipped inside `SECOND.EXE`; each was its own
//! program (`GRID/P.EXE`, `HARD/B.EXE`, `TWIST/T.EXE`) run behind the
//! standalone `DIS/DIS.EXE`. They are not in the demo's run order
//! (`shell::PART_ORDER`), carry no boot anchor, and run silent — the captures
//! that file them boot the shipped DIS with one start key and no music
//! (`docs/fidelity/reference-indices.md` §24).
//!
//! The menu that selects among them is Task 8's; [`VAULT`] is the table it
//! consumes, in menu order.

pub mod grid;

/// The vault's menu rows, in menu order: the registry name and its label.
///
/// The names are the registry's own (`parts::registry`), so a menu entry and
/// `--part <name>` start the same program. Task 8's menu consumes the table;
/// the `hard` and `twist` rows are listed here from the start because the
/// menu's order is the vault's own, not the order the ports land in.
#[allow(dead_code)] // Task 8's menu reads it; until then only the test does.
pub const VAULT: [(&str, &str); 3] = [("grid", "GRID"), ("hard", "HARD"), ("twist", "TWIST")];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_vault_table_is_the_three_effects_in_menu_order() {
        assert_eq!(
            VAULT,
            [("grid", "GRID"), ("hard", "HARD"), ("twist", "TWIST")]
        );
        // The names are the registry's: a menu row and `--part` agree. Only
        // `grid` is ported so far; Tasks 4 and 5 add the other two rows to
        // the registry, and this table already carries their place.
        assert_eq!(crate::parts::registered("grid"), Some("grid"));
        for (_, label) in VAULT {
            assert!(!label.is_empty());
        }
    }
}
