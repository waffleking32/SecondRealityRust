//! The parts' unit-test rig: an [`sr_engine::Machine`] with an optional player,
//! stepped in [`Engine::tick`](sr_engine::Engine::tick)'s order, plus one
//! scripted [`Music`] and the frame helpers the tests compare with.

use std::ops::{Deref, DerefMut};

use sr_engine::music::{Music, MusicPosition};
use sr_engine::{dac_expand, Machine, Palette, Part, PartCtx, PartStatus, View};

use crate::audio::PlayerMusic;

/// The repository root as seen from a unit test, whose working directory is
/// the crate's.
pub(crate) const ROOT: &str = "../../";

/// `rel` under the repository root.
pub(crate) fn repo(rel: &str) -> String {
    format!("{ROOT}{rel}")
}

/// A filed reference frame, `reference/parts/<part>/frame_<index>.<ext>`.
pub(crate) fn reference(part: &str, index: u32, ext: &str) -> Vec<u8> {
    let path = repo(&format!("reference/parts/{part}/frame_{index:06}.{ext}"));
    std::fs::read(&path).unwrap_or_else(|error| panic!("{path}: {error}"))
}

/// `(order, row)` at boot frame `frame` in `reference/music/<name>/trace.csv`.
pub(crate) fn trace_position(name: &str, frame: u32) -> (u8, u8) {
    let path = repo(&format!("reference/music/{name}/trace.csv"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{path}: {error}"));
    for line in text.lines().skip(1) {
        let fields: Vec<&str> = line.split(',').map(str::trim).collect();
        if fields[0].parse::<u32>().unwrap() == frame {
            return (fields[1].parse().unwrap(), fields[2].parse().unwrap());
        }
    }
    panic!("{name} holds no frame {frame}")
}

/// Indexed pixels through `palette`, widened as the DAC widens.
pub(crate) fn rgb_of(pixels: &[u8], palette: &Palette) -> Vec<u8> {
    let mut out = Vec::with_capacity(pixels.len() * 3);
    for &index in pixels {
        out.extend(palette.entry(index).map(dac_expand));
    }
    out
}

/// How many `unit`-byte pixels differ. The lengths are asserted first: `zip`
/// stops at the shorter side, so a short read would compare a prefix and
/// report a match.
fn differing(rendered: &[u8], expected: &[u8], unit: usize) -> usize {
    assert_eq!(
        rendered.len(),
        expected.len(),
        "the frame is {} bytes where the reference's is {}; a short read is not a comparison",
        rendered.len(),
        expected.len()
    );
    rendered
        .chunks(unit)
        .zip(expected.chunks(unit))
        .filter(|(a, b)| a != b)
        .count()
}

/// Differing RGB pixels (three bytes each).
pub(crate) fn differing_pixels(rendered: &[u8], expected: &[u8]) -> usize {
    differing(rendered, expected, 3)
}

/// Differing bytes: indices of an `.idx`, or channels of an `.rgb`.
pub(crate) fn differing_bytes(rendered: &[u8], expected: &[u8]) -> usize {
    differing(rendered, expected, 1)
}

/// One tick's answers from a [`ScriptedMusic`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Reading {
    pub order: u8,
    pub row: u8,
    pub musplus: i32,
    pub sync: i32,
}

impl Default for Reading {
    /// Order 0 row 0, no plus pending (`-32`), sync code 0.
    fn default() -> Reading {
        Reading {
            order: 0,
            row: 0,
            musplus: -32,
            sync: 0,
        }
    }
}

/// A player scripted one [`Reading`] a tick, indexed by the tick the rig
/// builds each context at. `muscode` is always 0 (D8).
pub(crate) struct ScriptedMusic {
    pub script: Vec<Reading>,
    /// What a tick past the script reads; `None` holds the last entry.
    pub past_end: Option<Reading>,
    /// The tick the rig last built a context at.
    pub at: usize,
    /// The part's own `poll` calls (a pre-roll's): the rig does not poll a script.
    pub polls: u32,
    /// `getmframe`'s clock after a `setmframe`, in frames per tick as
    /// `(numerator, denominator)`, rounded; `None` reads back the frame set.
    pub mframe_rate: Option<(i64, i64)>,
    /// Every `setmframe`, as `(tick, frame)`.
    pub setmframe_calls: Vec<(usize, i32)>,
    /// Set for a part whose source never syncs: `sync` panics with it.
    pub never_syncs: Option<&'static str>,
}

impl ScriptedMusic {
    pub fn readings(script: Vec<Reading>) -> ScriptedMusic {
        ScriptedMusic {
            script,
            past_end: None,
            at: 0,
            polls: 0,
            mframe_rate: None,
            setmframe_calls: Vec::new(),
            never_syncs: None,
        }
    }

    /// `(order, row)` a tick.
    pub fn positions(positions: Vec<(u8, u8)>) -> ScriptedMusic {
        ScriptedMusic::readings(
            positions
                .into_iter()
                .map(|(order, row)| Reading {
                    order,
                    row,
                    ..Reading::default()
                })
                .collect(),
        )
    }

    /// `musplus` a tick.
    pub fn musplus(values: Vec<i32>) -> ScriptedMusic {
        ScriptedMusic::readings(
            values
                .into_iter()
                .map(|musplus| Reading {
                    musplus,
                    ..Reading::default()
                })
                .collect(),
        )
    }

    /// A `sync` code a tick.
    pub fn sync_codes(codes: Vec<i32>) -> ScriptedMusic {
        ScriptedMusic::readings(
            codes
                .into_iter()
                .map(|sync| Reading {
                    sync,
                    ..Reading::default()
                })
                .collect(),
        )
    }

    /// What a tick past the script reads, in place of the last entry.
    pub fn past_end(self, reading: Reading) -> ScriptedMusic {
        ScriptedMusic {
            past_end: Some(reading),
            ..self
        }
    }

    pub fn never_syncs(self, why: &'static str) -> ScriptedMusic {
        ScriptedMusic {
            never_syncs: Some(why),
            ..self
        }
    }

    pub fn mframe_rate(self, numerator: i64, denominator: i64) -> ScriptedMusic {
        ScriptedMusic {
            mframe_rate: Some((numerator, denominator)),
            ..self
        }
    }

    /// The reading at tick `at`.
    pub fn reading_at(&self, at: usize) -> Reading {
        match self.script.get(at) {
            Some(reading) => *reading,
            None => self
                .past_end
                .or(self.script.last().copied())
                .unwrap_or_default(),
        }
    }

    fn reading(&self) -> Reading {
        self.reading_at(self.at)
    }
}

impl Music for ScriptedMusic {
    fn poll(&mut self) {
        self.polls += 1;
    }

    fn position(&self) -> MusicPosition {
        let Reading { order, row, .. } = self.reading();
        MusicPosition {
            order,
            row,
            ..MusicPosition::default()
        }
    }

    fn musplus(&mut self) -> i32 {
        self.reading().musplus
    }

    fn musrow(&mut self) -> i32 {
        i32::from(self.reading().row)
    }

    fn muscode(&mut self, _wanted: i32) -> i32 {
        0
    }

    fn sync(&mut self) -> i32 {
        if let Some(why) = self.never_syncs {
            panic!("{why}");
        }
        self.reading().sync
    }

    fn getmframe(&mut self) -> i32 {
        let Some(&(reset, frame)) = self.setmframe_calls.last() else {
            return 0;
        };
        match self.mframe_rate {
            None => frame,
            Some((numerator, denominator)) => {
                let n = self.at.saturating_sub(reset) as i64;
                frame + ((numerator * n + denominator / 2) / denominator) as i32
            }
        }
    }

    fn setmframe(&mut self, frame: i32) {
        self.setmframe_calls.push((self.at, frame));
    }
}

/// How the rig drives its player: a script is indexed by the tick, the real
/// player is polled once a tick, as the engine polls it.
pub(crate) trait RigMusic: Music + 'static {
    fn seek(&mut self, _tick: u32) {}
    fn poll_for_tick(&mut self) {}
}

impl RigMusic for ScriptedMusic {
    fn seek(&mut self, tick: u32) {
        self.at = tick as usize;
    }
}

impl RigMusic for PlayerMusic {
    fn poll_for_tick(&mut self) {
        self.poll();
    }
}

/// The engine's machine and an optional player, so a [`PartCtx`] can be built
/// for one call at a time. It derefs to the [`Machine`].
pub(crate) struct Rig<M: RigMusic = ScriptedMusic> {
    pub machine: Machine,
    pub music: Option<M>,
    /// The tick [`step`](Rig::step) runs next.
    pub tick: u32,
}

impl Rig {
    /// A 640×400 surface (the shell's), a `view_width`×`view_height` view, a
    /// black palette and no player.
    pub fn new(view_width: u16, view_height: u16) -> Rig {
        Rig {
            machine: Machine::new(
                sr_engine::MAX_WIDTH,
                sr_engine::MAX_HEIGHT,
                View::new(view_width, view_height),
            ),
            music: None,
            tick: 0,
        }
    }
}

impl<M: RigMusic> Rig<M> {
    /// This rig with `music` as its player.
    pub fn with<N: RigMusic>(self, music: N) -> Rig<N> {
        Rig {
            machine: self.machine,
            music: Some(music),
            tick: self.tick,
        }
    }

    /// This rig with palette entry `index` set to `rgb`.
    pub fn palette_entry(mut self, index: u8, rgb: [u8; 3]) -> Rig<M> {
        self.machine.palette.set_entry(index, rgb);
        self
    }

    /// The player.
    pub fn player(&mut self) -> &mut M {
        self.music.as_mut().expect("the rig has a player")
    }

    /// A context at `tick`, with the player (if any) seeked to it.
    pub fn ctx_with(&mut self, tick: u32, exit: bool, music_fresh: bool) -> PartCtx<'_> {
        let music = self.music.as_mut().map(|music| {
            music.seek(tick);
            music as &mut (dyn Music + 'static)
        });
        self.machine.ctx(music, tick, 0, 1, exit, music_fresh)
    }

    /// A band's context: never fresh, as the engine's band contexts are.
    pub fn ctx(&mut self, tick: u32) -> PartCtx<'_> {
        self.ctx_with(tick, false, false)
    }

    pub fn ctx_exit(&mut self, tick: u32) -> PartCtx<'_> {
        self.ctx_with(tick, true, false)
    }

    /// The init context of an entry-point run: the part the run's music was
    /// installed for.
    pub fn init_ctx(&mut self, tick: u32) -> PartCtx<'_> {
        self.ctx_with(tick, false, true)
    }

    /// The init context of a part reached mid-chain, the player already running.
    pub fn init_ctx_chain(&mut self, tick: u32) -> PartCtx<'_> {
        self.ctx_with(tick, false, false)
    }

    /// A context for a run with no player at all.
    pub fn ctx_without_music(&mut self, tick: u32) -> PartCtx<'_> {
        self.machine.ctx(None, tick, 0, 1, false, false)
    }

    /// Copper slot `slot` at the rig's tick, out of the registry for the call
    /// as `Engine::run_copper` takes it.
    pub fn run_slot(&mut self, slot: usize) {
        let Some(mut callback) = self.machine.copper.take(slot) else {
            return;
        };
        let tick = self.tick;
        callback(&mut self.ctx(tick));
        self.machine.copper.set(slot, callback);
    }

    /// One tick in the engine's order: slot 0, the poll, the part, the compose,
    /// slots 1 and 2. Returns the part's status and the composed frame.
    fn tick_with(
        &mut self,
        part: &mut dyn Part,
        exit: bool,
        render: bool,
    ) -> (PartStatus, Vec<u8>) {
        self.run_slot(0);
        if let Some(music) = self.music.as_mut() {
            music.poll_for_tick();
        }
        let tick = self.tick;
        let status = part.frame(&mut self.ctx_with(tick, exit, false));
        let frame = if render {
            self.rendered_rgb()
        } else {
            Vec::new()
        };
        self.run_slot(1);
        self.run_slot(2);
        self.tick += 1;
        (status, frame)
    }

    pub fn step(&mut self, part: &mut dyn Part) -> PartStatus {
        self.tick_with(part, false, false).0
    }

    /// [`step`](Rig::step) with the exit flag set.
    pub fn step_exit(&mut self, part: &mut dyn Part) -> PartStatus {
        self.tick_with(part, true, false).0
    }

    /// [`step`](Rig::step), returning the frame composed during the tick: the
    /// palette slots 1 and 2 leave is the next frame's.
    pub fn step_render(&mut self, part: &mut dyn Part) -> (PartStatus, Vec<u8>) {
        self.tick_with(part, false, true)
    }

    /// The view's window as palette indices.
    pub fn rendered_indices(&self) -> Vec<u8> {
        let Machine { surface, view, .. } = &self.machine;
        let mut out = Vec::with_capacity(usize::from(view.width) * usize::from(view.height));
        for y in 0..view.height {
            for x in 0..view.width {
                out.push(surface.pixels()[view.source_index(surface, x, y)]);
            }
        }
        out
    }

    /// The view's window through the live palette, widened as the DAC widens.
    pub fn rendered_rgb(&self) -> Vec<u8> {
        let Machine {
            surface,
            view,
            palette,
            ..
        } = &self.machine;
        let mut out = Vec::with_capacity(usize::from(view.width) * usize::from(view.height) * 3);
        for y in 0..view.height {
            for x in 0..view.width {
                let entry = palette.entry(surface.pixels()[view.source_index(surface, x, y)]);
                out.extend(entry.map(dac_expand));
            }
        }
        out
    }
}

impl<M: RigMusic> Deref for Rig<M> {
    type Target = Machine;

    fn deref(&self) -> &Machine {
        &self.machine
    }
}

impl<M: RigMusic> DerefMut for Rig<M> {
    fn deref_mut(&mut self) -> &mut Machine {
        &mut self.machine
    }
}

mod tests {
    use super::*;

    #[test]
    #[should_panic(expected = "a short read is not a comparison")]
    fn a_truncated_frame_is_refused_rather_than_counted() {
        differing_pixels(&[0; 6], &[0; 3]);
    }

    #[test]
    fn a_script_holds_its_last_reading_unless_told_otherwise() {
        let held = ScriptedMusic::sync_codes(vec![1, 2]);
        assert_eq!(held.reading_at(5).sync, 2);
        let fixed = ScriptedMusic::sync_codes(vec![1, 2]).past_end(Reading::default());
        assert_eq!(fixed.reading_at(5).sync, 0);
    }
}
