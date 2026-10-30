//! Stub parts and run builders for the shell's tests: what they watch is the counter, the
//! music and the shell's stages, not the pictures.

use sr_engine::music::{Music, MusicPosition};
use sr_engine::part::{Part, PartCtx, PartError, PartStatus};
use sr_engine::view::View;
use sr_engine::IndexedFrame;

use super::sequence::PartSource;
use super::{RunSettings, Sequence, SequenceStatus};

/// The tests' surface and view, 8×8.
pub(super) const TEST_SURFACE: usize = 8;

/// Fills the surface with `fill` every frame (or draws nothing, for `None`) and reports
/// `Finished` from frame `retires` on, so `Some(1)` draws two frames.
pub(super) struct Stub {
    pub name: &'static str,
    pub fill: Option<u8>,
    pub retires: Option<u32>,
}

impl Part for Stub {
    fn name(&self) -> &'static str { self.name }
    fn init(&mut self, _ctx: &mut PartCtx) -> Result<(), PartError> { Ok(()) }
    fn frame(&mut self, ctx: &mut PartCtx) -> PartStatus {
        if let Some(fill) = self.fill {
            ctx.surface.clear(fill);
        }
        match self.retires {
            Some(from) if ctx.frame >= from => PartStatus::Finished,
            _ => PartStatus::Continuing,
        }
    }
}

pub(super) fn stub(name: &'static str, fill: Option<u8>, retires: Option<u32>) -> Option<Box<dyn Part>> {
    Some(Box::new(Stub { name, fill, retires }))
}

/// The tests' parts: `a` fills with 7 and draws two frames, `b` draws one frame of nothing,
/// `pam` and `panicend` retire at once (PANICEND filling with 9), and the rest draw nothing
/// and never retire.
pub(super) fn stub_source(name: &str) -> Option<Box<dyn Part>> {
    match name {
        "beglogo" => stub("beglogo", Some(7), None),
        "a" => stub("a", Some(7), Some(1)),
        "b" => stub("b", None, Some(0)),
        "holds" => stub("holds", None, None),
        "pam" => stub("pam", None, Some(0)),
        "panicend" => stub("panicend", Some(9), Some(0)),
        "glenz" => stub("glenz", None, None),
        "alku" => stub("alku", None, None),
        "endlogo" => stub("endlogo", None, None),
        "cred" => stub("cred", None, None),
        "endscrl" => stub("endscrl", None, None),
        _ => None,
    }
}

/// A run of `plan` from `source`, with no music.
pub(super) fn run(plan: Vec<&'static str>, source: PartSource) -> Sequence {
    try_run(plan, source).unwrap()
}

pub(super) fn try_run(
    plan: Vec<&'static str>,
    source: PartSource,
) -> Result<Sequence, super::ShellError> {
    let view = View::new(TEST_SURFACE as u16, TEST_SURFACE as u16);
    Sequence::with_source(TEST_SURFACE, TEST_SURFACE, view, plan, source)
}

/// A run of `plan` from `source` with the real players.
pub(super) fn music_run(
    plan: Vec<&'static str>,
    source: PartSource,
    settings: RunSettings,
) -> Sequence {
    let view = View::new(TEST_SURFACE as u16, TEST_SURFACE as u16);
    Sequence::with_run(
        TEST_SURFACE,
        TEST_SURFACE,
        view,
        plan,
        source,
        Some(crate::audio::test_music),
        settings,
    )
    .unwrap()
}

/// Ticks to the end of the run or `limit` ticks, whichever is first; returns the ticks.
pub(super) fn run_out(run: &mut Sequence, limit: u32) -> u32 {
    let mut ticks = 0;
    while ticks < limit {
        ticks += 1;
        if run.tick() == SequenceStatus::Finished {
            break;
        }
    }
    ticks
}

/// Every pixel of `frame` through the palette on its scanline.
pub(super) fn colours(frame: &IndexedFrame) -> Vec<[u8; 3]> {
    (0..frame.height)
        .flat_map(|y| {
            frame
                .row(y)
                .iter()
                .map(move |&pixel| frame.palette_for(y).entry(pixel))
                .collect::<Vec<_>>()
        })
        .collect()
}

/// A [`Music`] whose `musplus` hands out the next scripted value a call (then -32), for the
/// cue wait.
pub(super) struct CueMusic {
    plus: std::collections::VecDeque<i32>,
}

impl CueMusic {
    pub fn new(plus: impl IntoIterator<Item = i32>) -> CueMusic {
        CueMusic { plus: plus.into_iter().collect() }
    }
}

impl Music for CueMusic {
    fn poll(&mut self) {}
    fn position(&self) -> MusicPosition { MusicPosition::default() }
    fn musplus(&mut self) -> i32 { self.plus.pop_front().unwrap_or(-32) }
    fn musrow(&mut self) -> i32 { 0 }
    fn muscode(&mut self, wanted: i32) -> i32 {
        sr_engine::muscode_from(MusicPosition::default(), wanted)
    }
    fn sync(&mut self) -> i32 { 0 }
    fn getmframe(&mut self) -> i32 { 0 }
    fn setmframe(&mut self, _frame: i32) {}
}
