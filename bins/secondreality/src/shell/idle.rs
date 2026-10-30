//! The shell's own stretches between a plan's parts. Each draws nothing and touches nothing,
//! so every tick composes the screen, palette and view the last part left, as the VGA keeps
//! showing video memory while the original's loader works. Installing one goes through
//! `Engine::start`, which clears the copper slots as every part start does.

use sr_engine::part::{Part, PartCtx, PartError, PartStatus};

/// The hold after a plan's last part (`@@partsdone`, `MAIN/U2.ASM:935`), and the loop seam
/// between two passes (`:919-933`). The seam drains after exit: `fademusic` and `restartmus`
/// never read the exit flag, and pass 2's first start clears it.
pub(super) struct Idle {
    /// The part whose screen is up.
    pub of: &'static str,
    pub drains: bool,
}

impl Part for Idle {
    fn name(&self) -> &'static str {
        self.of
    }

    fn init(&mut self, _ctx: &mut PartCtx) -> Result<(), PartError> {
        Ok(())
    }

    fn frame(&mut self, _ctx: &mut PartCtx) -> PartStatus {
        PartStatus::Continuing
    }

    fn drains_after_exit(&self) -> bool {
        self.drains
    }
}

/// The wait between PANICEND and the next part, `@@zh5`/`@@zh4` (`MAIN/U2.ASM:833-853`): one
/// call is one `dis_waitb`. It waits while `musplus` is non-negative, then, once it has seen
/// a negative, while it is non-positive, and leaves on the first positive: the measured 8903
/// (19) → 8953 (−32) → 9219 (1) of `reference/music/second2` (D38). The exit flag skips it
/// (`jnz @@skip5`), and so does a run with no player.
#[derive(Default)]
pub(super) struct CueWait {
    saw_negative: bool,
}

impl Part for CueWait {
    fn name(&self) -> &'static str {
        // The picture is PANICEND's.
        "panicend"
    }

    fn init(&mut self, _ctx: &mut PartCtx) -> Result<(), PartError> {
        Ok(())
    }

    fn frame(&mut self, ctx: &mut PartCtx) -> PartStatus {
        if ctx.exit_requested {
            return PartStatus::Finished;
        }
        let Some(music) = ctx.music() else {
            return PartStatus::Finished;
        };
        let plus = music.musplus();
        if !self.saw_negative {
            if plus >= 0 {
                return PartStatus::Continuing;
            }
            self.saw_negative = true;
        }
        if plus > 0 {
            PartStatus::Finished
        } else {
            PartStatus::Continuing
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parts::test_rig::{Rig, ScriptedMusic};
    use PartStatus::{Continuing, Finished};

    fn statuses(plus: Vec<i32>) -> Vec<PartStatus> {
        let ticks = plus.len();
        let mut rig = Rig::new(8, 8).with(ScriptedMusic::musplus(plus));
        let mut cue = CueWait::default();
        (0..ticks).map(|_| rig.step(&mut cue)).collect()
    }

    #[test]
    fn the_post_panicend_wait_ends_on_a_positive_musplus() {
        // Negatives from the first call, a zero that still waits, then the first positive.
        let ends_on_the_fifth = [Continuing, Continuing, Continuing, Continuing, Finished];
        assert_eq!(statuses(vec![-3, -1, -32, 0, 1]), ends_on_the_fifth);
        // The measured 8903 reading first: a non-negative is the first phase's wait.
        assert_eq!(sr_engine::musplus_from(2, 19), 19);
        assert_eq!(sr_engine::musplus_from(2, 32), -32);
        assert_eq!(statuses(vec![19, 19, -32, 0, 1]), ends_on_the_fifth);

        let mut rig = Rig::new(8, 8).with(ScriptedMusic::musplus(vec![-3]));
        assert_eq!(rig.step_exit(&mut CueWait::default()), Finished, "the exit flag skips it");
        let mut rig = Rig::new(8, 8);
        assert_eq!(rig.step(&mut CueWait::default()), Finished, "no player, no cue");
    }
}
