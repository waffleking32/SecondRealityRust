//! The demo's run order, its blocks, and where a run that starts at a part begins.

use super::sequence::Looping;
use super::ShellError;
use crate::parts;

/// The demo's first graphics frame under the capture recipe: the shell's mode 13h and
/// `inittwk`. Measured 404 in five runs of five (`reference-indices.md` §1.12, D17).
pub const FIRST_GRAPHICS_FRAME: u32 = 404;

/// U2E's tick 0: the first visible frame, 467 (`reference-indices.md` §18.7), less the 49
/// pre-visible waits of `U2E.C:285-303`. The unit test
/// `the_fadeset_lands_on_tick_49_and_paints_the_measured_band` calibrates it.
pub const U2E_ANCHOR: u32 = 418;

/// PAM's first drawn frame, the flat `(251,251,251)` wash: measured, byte-identical in three
/// sessions (`reference-indices.md` §6.1, §6.4).
pub const PAM_ANCHOR: u32 = 7710;

/// `SECOND n`'s argument and the `partsmask` byte it selects: the literals at
/// `MAIN/U2.ASM:111-117`, read by `checkcmdline` (`:595-630`). `1` and no argument are the
/// same run (the default `whattorun` is `7fh`, `:108`, and bits 5-6 gate nothing); `U`
/// selects the hidden part (`:613-616`, `:626-627`).
pub const BLOCKS: &[(char, u8)] = &[
    ('1', 0x1f),
    ('2', 0x1e),
    ('3', 0x1c),
    ('4', 0x18),
    ('5', 0x10),
    ('U', 0x80),
];

/// The demo's parts in the order the shell runs them (`MAIN/U2.ASM:790-918`'s
/// `partexecute` calls, `part-sequence.md` §1), each with the `partsmask` bit that gates it.
pub(super) const PART_ORDER: &[(&str, u8)] = &[
    ("alku", 0x01),
    ("u2a", 0x01),
    ("pam", 0x01),
    ("beglogo", 0x02),
    ("glenz", 0x02),
    ("tunneli", 0x02),
    ("techno", 0x02),
    ("panicend", 0x02),
    ("mntscrl", 0x04),
    ("lnszoom", 0x04),
    ("plzpart", 0x04),
    ("minvball", 0x04),
    ("rayscrl", 0x04),
    ("3dsinfld", 0x04),
    ("jplogo", 0x04),
    ("u2e", 0x08),
    ("ddstars", 0x80),
    ("endlogo", 0x10),
    ("cred", 0x10),
    ("endscrl", 0x10),
];

/// The reference index of `part`'s first drawn frame when a run *starts* there, or `None`
/// when the evidence gives only a window (`part-sequence.md` §10.1).
///
/// ALKU, BEGLOGO and ENDLOGO are the block entries at the boot's first graphics frame
/// (`MAIN/U2.ASM:790-792`, `:806-808`, `:906-908`); PAM and U2E are measured. A part reached
/// mid-run begins wherever the parts before it left off.
pub fn boot_anchor(part: &str) -> Option<u32> {
    match part {
        "alku" | "beglogo" | "endlogo" => Some(FIRST_GRAPHICS_FRAME),
        "pam" => Some(PAM_ANCHOR),
        "u2e" => Some(U2E_ANCHOR),
        _ => None,
    }
}

/// The smallest of `wanted` that a run starting at `start` can never reach: a run's indices
/// only go up.
pub fn first_unreachable(start: u32, wanted: &[u32]) -> Option<u32> {
    wanted.iter().copied().filter(|index| *index < start).min()
}

/// The parts `block` runs, in the demo's order.
pub fn block_plan(block: char) -> Result<Vec<&'static str>, ShellError> {
    let mask = block_mask(block).ok_or_else(|| ShellError::UnknownPart {
        name: block.to_string(),
    })?;
    Ok(PART_ORDER
        .iter()
        .filter(|(_, gate)| gate & mask != 0)
        .map(|(name, _)| *name)
        .collect())
}

/// The `partsmask` byte `SECOND block` selects, or `None` for a block the original rejects.
/// `U` and `u` are the same block.
fn block_mask(block: char) -> Option<u8> {
    let block = block.to_ascii_uppercase();
    BLOCKS.iter().find(|(name, _)| *name == block).map(|(_, mask)| *mask)
}

/// The parts a run of `block` plays under `looping`: [`block_plan`], without ENDSCRL for any
/// `looping` but 0 (`MAIN/U2.ASM:914-917`).
pub fn run_plan(block: char, looping: Looping) -> Result<Vec<&'static str>, ShellError> {
    let mut plan = block_plan(block)?;
    if looping != Looping::Off {
        plan.retain(|name| *name != "endscrl");
    }
    Ok(plan)
}

/// Every pass after the first: `mov cs:whattorun,07fh` (`MAIN/U2.ASM:919`) gates block 1's
/// parts, so pass 2 is block 1's plan without ENDSCRL, whichever block pass 1 began at.
pub(super) fn loop_plan() -> Vec<&'static str> {
    run_plan('1', Looping::On).expect("block 1 is a block")
}

/// The registered name `name` is, for a run's plan.
pub fn resolve(name: &str) -> Result<&'static str, ShellError> {
    parts::registered(name).ok_or_else(|| ShellError::UnknownPart {
        name: name.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell::MusicStart;

    #[test]
    fn every_name_in_the_demos_order_is_registered_and_in_some_blocks_plan() {
        for (name, _) in PART_ORDER {
            assert_eq!(parts::registered(name), Some(*name), "{name} is registered");
            assert!(
                BLOCKS.iter().any(|(block, _)| block_plan(*block).unwrap().contains(name)),
                "{name} is in no block's plan"
            );
        }
    }

    #[test]
    fn a_mid_block_part_has_no_anchor_and_says_so() {
        // The measured anchors, and `None` for every other part of the demo: their starts
        // are windows (`part-sequence.md` §10.1).
        let anchored =
            [("alku", 404), ("beglogo", 404), ("endlogo", 404), ("pam", 7710), ("u2e", 418)];
        for (name, _) in PART_ORDER {
            let expected = anchored.iter().find(|(at, _)| at == name).map(|(_, index)| *index);
            assert_eq!(boot_anchor(name), expected, "{name}");
        }
    }

    #[test]
    fn u2e_runs_are_numbered_from_their_derived_intro_anchor() {
        assert_eq!(U2E_ANCHOR, 467 - 49);
        // `SECOND 4`'s plan begins at U2E, and its clock (408) is ten frames before it.
        let first = block_plan('4').unwrap()[0];
        assert_eq!(first, "u2e");
        assert_eq!(boot_anchor(first), Some(U2E_ANCHOR));
        assert_eq!(
            MusicStart::for_part("u2e", Some(U2E_ANCHOR)),
            MusicStart { frame: U2E_ANCHOR, waiting: 0, pre: 10 }
        );
    }

    #[test]
    fn a_blocks_parts_are_the_demos_own_order_from_that_blocks_first_part() {
        assert_eq!(block_plan('5').unwrap(), ["endlogo", "cred", "endscrl"]);
        let plan = block_plan('2').unwrap();
        assert_eq!(plan.first(), Some(&"beglogo"));
        assert_eq!(plan.last(), Some(&"endscrl"));
        assert!(!plan.contains(&"alku"));
    }

    #[test]
    fn the_hidden_parts_block_runs_the_hidden_part_alone() {
        assert_eq!(block_plan('U').unwrap(), ["ddstars"]);
        assert_eq!(block_plan('u').unwrap(), ["ddstars"]);
    }

    #[test]
    fn a_block_the_original_rejects_is_not_a_block() {
        // `checkcmdline` sets errorcode 2 for '0' and '6'-'9' and runs nothing.
        for rejected in ['0', '6', '7', '8', '9', 'x'] {
            assert!(block_plan(rejected).is_err(), "{rejected} is not a block");
        }
    }

    #[test]
    fn a_looping_plan_has_no_end_scroller() {
        for block in ['1', '5'] {
            let full = block_plan(block).unwrap();
            let without: Vec<_> = full.iter().copied().filter(|name| *name != "endscrl").collect();
            assert_ne!(full, without, "block {block} ends in ENDSCRL");
            assert_eq!(run_plan(block, Looping::On).unwrap(), without);
            assert_eq!(run_plan(block, Looping::NoEndScroller).unwrap(), without);
            assert_eq!(run_plan(block, Looping::Off).unwrap(), full);
        }
    }

    #[test]
    fn resolve_rejects_a_name_the_demo_never_had() {
        assert_eq!(resolve("testcard").unwrap(), "testcard");
        assert!(matches!(resolve("nope"), Err(ShellError::UnknownPart { name }) if name == "nope"));
        // `--part ""` must say which name was empty.
        assert_eq!(resolve("").unwrap_err().to_string(), "unknown part \"\"");
    }

    #[test]
    fn an_index_before_the_runs_start_can_never_be_reached() {
        assert_eq!(first_unreachable(404, &[430, 100, 900]), Some(100));
        assert_eq!(first_unreachable(404, &[404, 430, 900]), None);
        assert_eq!(first_unreachable(404, &[]), None);
        assert_eq!(first_unreachable(0, &[0, 1]), None);
    }
}
