//! The music a run plays: which module and order it starts at, when the module's clock
//! starts in the run's numbering, and the mid-run `restartmus` sites.
//!
//! `MAIN/U2.ASM:766-786` starts the music for the demo's beginning and `:856-905` restarts it
//! at each block's entry; a run takes the row of the entry point its plan begins at.
//! `docs/fidelity/music.md` §12 and `part-sequence.md` §10 hold the evidence for each row,
//! `music-clock.md` §2 the clock's measured start.

use super::plan::{block_plan, BLOCKS, FIRST_GRAPHICS_FRAME};
use super::ShellError;

/// One of the demo's two modules: `restartmus`'s `ax` (`MAIN/U2.ASM:766-775`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleId {
    /// `ax = 0`, `MUSIC0.S3M`, whose tempo byte the shell patches (`MAIN/STARTMUS.C:254-258`).
    Music0,
    /// `ax = 1`, `MUSIC1.S3M`.
    Music1,
}

impl ModuleId {
    /// The module's row in the shared song table: name, file and patched tempo
    /// (`MAIN/STARTMUS.C:252-265`, `music-clock.md` §3).
    pub fn song(self) -> &'static sr_audio::Song {
        match self {
            ModuleId::Music0 => &sr_audio::SONGS[0],
            ModuleId::Music1 => &sr_audio::SONGS[1],
        }
    }

    /// The module's name, as the demo's files spell it.
    pub fn name(self) -> &'static str {
        self.song().name
    }

    /// `restartmus`'s `ax` for this module (`MAIN/U2.ASM:767`, `:772`).
    #[cfg(test)]
    pub fn number(self) -> u8 {
        match self {
            ModuleId::Music0 => 0,
            ModuleId::Music1 => 1,
        }
    }
}

/// The frame the restart recipes' first tick falls on: 408.0, stable to a tenth of a frame
/// across four sessions (`music-clock.md` §2).
pub const MUSIC_START_FRAME: u32 = 408;

/// The frame the initial `_zplaysong(0)`'s first tick falls on, for a block-1 run
/// (`MAIN/U2.ASM:766-786`): `second1` fits it at 399.6 (`music-clock.md` §2,
/// `cargo xtask music analyse --trace reference/music/second1`), used as 400.
pub const BLOCK_ONE_MUSIC_START_FRAME: u32 = 400;

/// The clock a run beginning at `part` starts under: the block-1 start for the parts before
/// any restart (`MAIN/U2.ASM:790-799`), the restart recipes' for everything else.
pub(super) fn music_start_frame_for(part: &str) -> u32 {
    match part {
        "alku" | "u2a" | "pam" => BLOCK_ONE_MUSIC_START_FRAME,
        _ => MUSIC_START_FRAME,
    }
}

/// How a run reaches the music's clock. A run whose first frame is before the clock's start
/// waits for it; one that begins after it pre-polls the frames it missed (`Player::poll` is
/// the only thing that moves the music, so N polls in a row land where N polls a frame
/// would); a run numbered from zero starts its music with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MusicStart {
    /// The run frame the music is aligned to: the clock's start for a waiting entry, the
    /// run's first frame for a pre-polled one, the boundary for a restart. The manifest's
    /// `start_frame`.
    pub frame: u32,
    /// Engine polls before the music advances.
    pub waiting: u32,
    /// Polls made at the run's start, before its first tick.
    pub pre: u32,
}

impl MusicStart {
    /// The start of a run whose plan begins at `part` and whose first frame is `anchor`
    /// (`None`: a run numbered from zero).
    pub fn for_part(part: &str, anchor: Option<u32>) -> MusicStart {
        let start_frame = music_start_frame_for(part);
        match anchor {
            Some(anchor) if anchor < start_frame => MusicStart {
                frame: start_frame,
                waiting: start_frame - anchor,
                pre: 0,
            },
            Some(anchor) if anchor > start_frame => MusicStart {
                frame: anchor,
                waiting: 0,
                pre: anchor - start_frame,
            },
            Some(_) => MusicStart {
                frame: start_frame,
                waiting: 0,
                pre: 0,
            },
            None => MusicStart {
                frame: 0,
                waiting: 0,
                pre: 0,
            },
        }
    }

    /// The start a mid-run restart installs at the boundary frame `index`: nothing
    /// pre-polled, and the clock four frames later, the restart recipes' measured 404→408
    /// (D34).
    pub fn restart(index: u32) -> MusicStart {
        MusicStart {
            frame: index,
            waiting: MUSIC_START_FRAME - FIRST_GRAPHICS_FRAME,
            pre: 0,
        }
    }
}

/// How a [`Sequence`](super::Sequence) builds a player with the run's options, at the entry
/// and at every restart; the binary's is [`audio::load_player`](crate::audio::load_player).
pub(crate) type MusicSource =
    fn(ModuleId, sr_audio::PlayerOptions) -> Result<crate::audio::Shared, String>;

/// One row of the demo's restart table: the module and order a block's run begins at. The
/// evidence fields are what the test below checks against `MAIN/U2.ASM`; nothing at run
/// time reads them.
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub struct MusicRow {
    pub block: char,
    pub module: ModuleId,
    pub order: u8,
    /// The `whattorun` bit the row's guard requires set (0: none).
    pub when: u8,
    /// The `whattorun` bit the guard requires clear.
    pub unless: u8,
    /// The `MAIN/U2.ASM` lines the row is read from, as `file:first-last`.
    pub citations: &'static [&'static str],
    /// What those lines carry, in order.
    pub said: &'static [&'static str],
}

/// The demo's restart table (D13, D27): the initial start (`MAIN/U2.ASM:766-786`) and its two
/// branches, then the four restart sites. `:880-885` is block 4's entry and also a mid-run
/// restart for blocks 1-3 ([`MUSIC_RESTART_AT`]).
pub const MUSIC_RESTARTS: &[MusicRow] = &[
    MusicRow {
        block: '1',
        module: ModuleId::Music0,
        order: 0,
        when: 0x01,
        unless: 0,
        // `mov bx,-1` is "no seek"; the play is `_zplaysong(0)` once the sound is on.
        citations: &["MAIN/U2.ASM:766-775", "MAIN/U2.ASM:785-787"],
        said: &["mov ax,0", "mov bx,-1", "call restartmus", "push 0", "call _zplaysong"],
    },
    MusicRow {
        block: '2',
        module: ModuleId::Music1,
        order: 0,
        when: 0,
        unless: 0x01,
        citations: &["MAIN/U2.ASM:772-775"],
        said: &["mov ax,1", "mov bx,0", "mov cx,48700", "call restartmus"],
    },
    MusicRow {
        block: '3',
        module: ModuleId::Music1,
        order: 42,
        when: 0x04,
        unless: 0x02,
        citations: &["MAIN/U2.ASM:856-863"],
        said: &[
            "test cs:whattorun,4",
            "test cs:whattorun,2",
            "mov ax,1",
            "mov bx,42",
            "call restartmus",
        ],
    },
    MusicRow {
        block: '4',
        module: ModuleId::Music0,
        order: 18,
        when: 0x08,
        unless: 0,
        citations: &["MAIN/U2.ASM:880-885"],
        said: &["test cs:whattorun,8", "mov ax,0", "mov bx,18", "call restartmus"],
    },
    MusicRow {
        block: 'U',
        module: ModuleId::Music0,
        order: 70,
        when: 0x80,
        unless: 0,
        citations: &["MAIN/U2.ASM:889-894"],
        said: &["test cs:whattorun,80h", "mov ax,0", "mov bx,70", "call restartmus"],
    },
    MusicRow {
        block: '5',
        module: ModuleId::Music0,
        order: 25,
        when: 0x10,
        unless: 0x08,
        citations: &["MAIN/U2.ASM:898-905"],
        said: &[
            "test cs:whattorun,16",
            "test cs:whattorun,8",
            "mov ax,0",
            "mov bx,25",
            "call restartmus",
        ],
    },
];

/// The mid-run `restartmus` sites, keyed by the part each precedes.
///
/// `MAIN/U2.ASM:880-885` is guarded by `whattorun & 8` alone, so every block whose plan
/// reaches U2E takes it (blocks 1-3 mid-run; block 4's is its entry install). `:813-818` is
/// guarded by `& 2` (the region `:801-802` opens) and `& 1`, which only block 1's mask has;
/// block 2 skips it and is already on MUSIC1@0 (`:772-775`), which is why
/// [`Sequence::restart_music`](super::Sequence) keeps a player already on the row. D34 and D38
/// have the measurements.
pub const MUSIC_RESTART_AT: &[(&str, ModuleId, u8)] = &[
    ("u2e", ModuleId::Music0, 18),
    ("glenz", ModuleId::Music1, 0),
];

/// The module and order `SECOND block` starts at, or [`ShellError::UnknownPart`] for a block
/// the original rejects.
pub fn music_for(block: char) -> Result<(ModuleId, u8), ShellError> {
    let block = block.to_ascii_uppercase();
    MUSIC_RESTARTS
        .iter()
        .find(|row| row.block == block)
        .map(|row| (row.module, row.order))
        .ok_or(ShellError::UnknownPart {
            name: block.to_string(),
        })
}

/// The music of a run that begins at `part`: the row of the block whose plan begins there,
/// or, for a part the demo only reaches mid-run, the module and order in force when its
/// sequence gets there (`music.md` §12, each part's note). `None` for a part the demo never
/// played (the test card, the vault), whose run is silent.
pub fn music_for_part(part: &str) -> Option<(ModuleId, u8)> {
    let entry_point = BLOCKS
        .iter()
        .map(|(block, _)| *block)
        .find(|block| {
            block_plan(*block)
                .ok()
                .and_then(|plan| plan.first().copied())
                == Some(part)
        });
    if let Some(block) = entry_point {
        return music_for(block).ok();
    }
    match part {
        // Before any restart: the initial start's MUSIC0@0 (`MAIN/U2.ASM:766-775`).
        "alku" | "u2a" | "pam" => Some((ModuleId::Music0, 0)),
        // Block 2's initial start (`:772-775`) and block 1's `:813-818` restart both put
        // MUSIC1@0 under GLENZ; TUNNELI's order is not measured, and 0 is the one the
        // evidence names.
        "glenz" | "tunneli" => Some((ModuleId::Music1, 0)),
        // `second5`/`second5b`: 26:56 at CRED's tick 0 (reference index 1212).
        "cred" => Some((ModuleId::Music0, 26)),
        // Derived, not measured: block 5's MUSIC0@25 advanced by the port's player to the
        // ENDSCRL start window (8841..8938).
        "endscrl" => Some((ModuleId::Music0, 43)),
        // `second3a`: 62:0 at 5723, the part's `_dis_setmframe(0)` (`PLZPART/PLZ.C:69`).
        "plzpart" => Some((ModuleId::Music1, 62)),
        // `second3`: 45:59 at 2452, the part's `_dis_setmframe(0)` (`LENS/MAIN.C:106`).
        "lnszoom" => Some((ModuleId::Music1, 45)),
        // The order when DOTS's start window opens (boot 10,300..10,368).
        "minvball" => Some((ModuleId::Music1, 75)),
        // `second3b`: order 86 across the start window (12900, 13000]. Seeking 86 and
        // playing through the `254` at 87 sets `zplus` bit 1, which RAYSCRL's init wait
        // needs (`WATER/DEMO.PAS:98-104`); a seek to 88 never passes it.
        "rayscrl" => Some((ModuleId::Music1, 86)),
        // `second2`: 18:60 at 4797, the start window.
        "techno" => Some((ModuleId::Music1, 18)),
        // `second2`: 37:53..37:55 across the start window (8780, 8790].
        "panicend" => Some((ModuleId::Music1, 37)),
        // Derived: the order `second3d` holds where the window (15000, 15100] opens, before
        // the 91 -> 93 crossing the part's gate waits for.
        "3dsinfld" => Some((ModuleId::Music1, 91)),
        // `second3d`: order 96 across the whole start window (17000, 17100].
        "jplogo" => Some((ModuleId::Music1, 96)),
        _ => None,
    }
}

/// The loop's restart: `restartmus` with `ax=0, bx=0` reloads MUSIC0 and seeks order 0
/// (`MAIN/U2.ASM:929-932`).
pub const LOOP_RESTART: (ModuleId, u8) = (ModuleId::Music0, 0);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell::plan::PART_ORDER;
    use crate::shell::PAM_ANCHOR;

    /// `MAIN/U2.ASM` as lines, read as bytes (it is not UTF-8), with each run of spaces and
    /// tabs collapsed to one space.
    fn upstream_lines() -> Vec<String> {
        let path = "../../upstream/SecondReality/MAIN/U2.ASM";
        let bytes = std::fs::read(path).unwrap_or_else(|error| {
            panic!("{path}: {error} — the upstream tree is a submodule; run `git submodule update --init`")
        });
        bytes
            .split(|byte| *byte == b'\n')
            .map(|line| {
                let text: String = line.iter().map(|byte| char::from(*byte)).collect();
                text.split_whitespace().collect::<Vec<_>>().join(" ")
            })
            .collect()
    }

    /// The lines a citation such as `MAIN/U2.ASM:898-905` names, joined.
    fn cited(lines: &[String], citation: &str) -> String {
        let (file, range) = citation.split_once(':').unwrap();
        assert_eq!(file, "MAIN/U2.ASM");
        let (first, last) = range.split_once('-').unwrap();
        let (first, last): (usize, usize) = (first.parse().unwrap(), last.parse().unwrap());
        lines[first - 1..last].join("\n")
    }

    /// Whether `text` holds each of `said`, in order.
    fn says_in_order(text: &str, said: &[&str]) -> bool {
        let mut at = 0;
        said.iter().all(|said| match text[at..].find(said) {
            Some(found) => {
                at += found + said.len();
                true
            }
            None => false,
        })
    }

    fn mask(block: char) -> u8 {
        BLOCKS.iter().find(|(name, _)| *name == block).unwrap().1
    }

    #[test]
    fn every_row_of_the_restart_table_is_the_originals_own() {
        let lines = upstream_lines();
        for row in MUSIC_RESTARTS {
            let text: Vec<String> = row.citations.iter().map(|c| cited(&lines, c)).collect();
            let text = text.join("\n");
            assert!(says_in_order(&text, row.said), "{}: {:?} is not in\n{text}", row.block, row.said);
            assert!(row.said.contains(&"call restartmus"), "{}", row.block);
            let module = format!("mov ax,{}", row.module.number());
            assert!(row.said.contains(&module.as_str()), "{}: {module}", row.block);
            if row.block == '1' {
                assert!(row.order == 0 && row.said.contains(&"mov bx,-1"));
            } else {
                let order = format!("mov bx,{}", row.order);
                assert!(row.said.contains(&order.as_str()), "{}: {order}", row.block);
            }
            assert_eq!(mask(row.block) & row.when, row.when, "{}'s guard bit", row.block);
            assert_eq!(mask(row.block) & row.unless, 0, "{}'s clear bit", row.block);
            assert_eq!(music_for(row.block).ok(), Some((row.module, row.order)));
        }
        assert_eq!(MUSIC_RESTARTS.len(), BLOCKS.len(), "one row a block");
    }

    #[test]
    fn the_mid_run_restarts_are_the_guards_the_masks_reach() {
        // `:880-885` is bit 3 alone, and blocks 1-4 all carry it.
        let row = MUSIC_RESTARTS.iter().find(|row| row.block == '4').unwrap();
        assert_eq!((row.when, row.unless), (0x08, 0));
        for block in ['1', '2', '3', '4'] {
            assert_ne!(mask(block) & 0x08, 0, "{block} reaches :880");
        }
        // `:889-894` is entry-point-only because only `SECOND U` has bit 7.
        for (block, mask) in BLOCKS {
            assert_eq!(mask & 0x80 != 0, *block == 'U', "{block}");
        }
        // `:813-818` sits in the region `:801-802`'s `& 2` opens and `:855` closes, behind
        // its own `& 1`; the only mask with bit 0 is block 1's, and it has bit 1.
        let lines = upstream_lines();
        assert!(says_in_order(&cited(&lines, "MAIN/U2.ASM:801-802"), &["test cs:whattorun,2", "jz @@skipp2"]));
        assert!(cited(&lines, "MAIN/U2.ASM:855-855").contains("@@skipp2:"));
        assert!(says_in_order(
            &cited(&lines, "MAIN/U2.ASM:813-818"),
            &["test cs:whattorun,1", "jz @@skip2b", "mov ax,1", "mov bx,0", "mov cx,48700", "call restartmus"],
        ));
        for (block, mask) in BLOCKS {
            if mask & 0x01 != 0 {
                assert_eq!(*block, '1');
                assert_ne!(mask & 0x02, 0);
            }
        }
        assert_eq!(MUSIC_RESTART_AT, &[("u2e", ModuleId::Music0, 18), ("glenz", ModuleId::Music1, 0)]);
        assert_eq!(music_for('4').unwrap(), (ModuleId::Music0, 18), "the same call as block 4's");
    }

    #[test]
    fn a_blocks_first_part_is_the_entry_point_its_row_is_about() {
        for row in MUSIC_RESTARTS {
            let first = block_plan(row.block).unwrap()[0];
            assert_eq!(music_for_part(first), Some((row.module, row.order)), "{first}");
        }
        // Every registered name, in the demo's order, then the parts it never played.
        let expected: &[(&str, Option<(ModuleId, u8)>)] = &[
            ("alku", Some((ModuleId::Music0, 0))),
            ("u2a", Some((ModuleId::Music0, 0))),
            ("pam", Some((ModuleId::Music0, 0))),
            ("beglogo", Some((ModuleId::Music1, 0))),
            ("glenz", Some((ModuleId::Music1, 0))),
            ("tunneli", Some((ModuleId::Music1, 0))),
            ("techno", Some((ModuleId::Music1, 18))),
            ("panicend", Some((ModuleId::Music1, 37))),
            ("mntscrl", Some((ModuleId::Music1, 42))),
            ("lnszoom", Some((ModuleId::Music1, 45))),
            ("plzpart", Some((ModuleId::Music1, 62))),
            ("minvball", Some((ModuleId::Music1, 75))),
            ("rayscrl", Some((ModuleId::Music1, 86))),
            ("3dsinfld", Some((ModuleId::Music1, 91))),
            ("jplogo", Some((ModuleId::Music1, 96))),
            ("u2e", Some((ModuleId::Music0, 18))),
            ("ddstars", Some((ModuleId::Music0, 70))),
            ("endlogo", Some((ModuleId::Music0, 25))),
            ("cred", Some((ModuleId::Music0, 26))),
            ("endscrl", Some((ModuleId::Music0, 43))),
            ("testcard", None),
            ("grid", None),
            ("hard", None),
            ("twist", None),
        ];
        for (name, music) in expected {
            assert_eq!(music_for_part(name), *music, "{name}");
        }
        assert_eq!(expected.len(), crate::parts::registry().len());
        for (name, _) in crate::parts::registry() {
            assert!(expected.iter().any(|(at, _)| at == name), "{name} has no expected music");
        }
        assert_eq!(expected.len(), PART_ORDER.len() + 4);
    }

    #[test]
    fn the_music_waits_for_its_anchor_and_catches_up_past_it() {
        let start = MusicStart::for_part;
        // The restart recipes' 408: a block entry at 404 waits four frames, a run on the
        // clock waits for nothing, `SECOND 3`'s MNTSCRL (about 417) pre-polls.
        assert_eq!(start("beglogo", Some(404)), MusicStart { frame: 408, waiting: 4, pre: 0 });
        assert_eq!(start("endlogo", Some(404)), MusicStart { frame: 408, waiting: 4, pre: 0 });
        assert_eq!(start("beglogo", Some(408)), MusicStart { frame: 408, waiting: 0, pre: 0 });
        assert_eq!(start("mntscrl", Some(417)), MusicStart { frame: 417, waiting: 0, pre: 9 });
        // A run numbered from zero starts its music with it.
        assert_eq!(start("mntscrl", None), MusicStart { frame: 0, waiting: 0, pre: 0 });
        // Block 1's 400: a run at 404 pre-polls four frames, `--part pam` 7,310.
        for part in ["alku", "u2a", "pam"] {
            assert_eq!(start(part, Some(404)), MusicStart { frame: 404, waiting: 0, pre: 4 });
        }
        assert_eq!(
            start("pam", Some(PAM_ANCHOR)),
            MusicStart { frame: PAM_ANCHOR, waiting: 0, pre: 7310 }
        );
        // A restart waits four frames from its boundary (D34).
        assert_eq!(MusicStart::restart(17_418), MusicStart { frame: 17_418, waiting: 4, pre: 0 });
    }

    // --- the run's music against the filed traces ------------------------------------

    use super::super::rediff_table::JITTER_FRAMES;
    use crate::parts::test_rig::trace;
    use crate::shell::test_support::{music_run, stub_source};
    use crate::shell::{RunSettings, Sequence, SequenceStatus};
    use std::collections::BTreeMap;

    /// How far the port's `_np_zframe` may sit from the trace's, once the sample instant's
    /// one-frame shift is taken out: one tick is the quantisation, two a drift (A12).
    const ZFRAME_SLOP: i32 = 1;

    /// A run walked over every frame `reference/music/<name>` holds from the run's start:
    /// the trace's published positions and `_np_zframe`, and the port's at the same frames.
    struct Walk {
        start: u32,
        wanted: BTreeMap<u32, (u8, u8)>,
        wanted_ticks: BTreeMap<u32, i32>,
        port: BTreeMap<u32, (u8, u8)>,
        port_ticks: BTreeMap<u32, i32>,
    }

    fn walk(run: &mut Sequence, name: &str) -> Walk {
        let trace = trace(name);
        let last = trace.iter().map(|row| row.frame).max().unwrap();
        let start = run.index();
        let held = || trace.iter().filter(|row| row.frame >= start);
        let wanted: BTreeMap<u32, (u8, u8)> = held().map(|row| (row.frame, row.position)).collect();
        let wanted_ticks = held().map(|row| (row.frame, row.zframe)).collect();
        let (mut port, mut port_ticks) = (BTreeMap::new(), BTreeMap::new());
        while run.frame_index().is_none_or(|index| index < last) {
            if run.tick() == SequenceStatus::Finished {
                break;
            }
            let index = run.index() - 1;
            if wanted.contains_key(&index) {
                let position = run.music_position().expect("the run has a player");
                port.insert(index, (position.order, position.row));
                let player = run.audio_player().unwrap();
                port_ticks.insert(index, crate::audio::lock(&player).zframe());
            }
        }
        assert_eq!(port.len(), wanted.len(), "every frame the trace holds was visited");
        Walk { start, wanted, wanted_ticks, port, port_ticks }
    }

    /// The distinct values of a sequence, in order.
    fn distinct<T: PartialEq + Copy>(items: impl Iterator<Item = T>) -> Vec<T> {
        let mut out: Vec<T> = Vec::new();
        for item in items {
            if out.last() != Some(&item) {
                out.push(item);
            }
        }
        out
    }

    #[test]
    fn the_runs_music_is_where_the_trace_holds_it_at_every_frame_the_trace_has() {
        // A `--block 5` run with the real player against `second5`: a music start a few
        // frames off moves every row change, and only a frame-by-frame walk sees it.
        let mut run = music_run(block_plan('5').unwrap(), stub_source, RunSettings::default());
        assert_eq!(run.anchor(), Some(404));
        assert_eq!(run.music_start_frame(), Some(MUSIC_START_FRAME));
        let Walk { start, wanted, wanted_ticks, port, port_ticks } = walk(&mut run, "second5");

        // Placement: every position the port publishes is one the trace publishes within
        // the two filed sessions' jitter (A12). `music.md` §12 quotes the printout.
        let nearest = |frame: u32, position: (u8, u8)| -> Option<u32> {
            wanted.iter().filter(|(_, value)| **value == position).map(|(at, _)| at.abs_diff(frame)).min()
        };
        let (mut exact, mut worst, mut unknown) = (0usize, 0u32, Vec::new());
        for (frame, position) in &port {
            match nearest(*frame, *position) {
                Some(0) => exact += 1,
                Some(distance) => worst = worst.max(distance),
                None => unknown.push((*frame, *position)),
            }
        }
        println!(
            "second5: {exact} of {} frames agree exactly ({:.3} %); worst displacement {worst} frame(s)",
            port.len(),
            100.0 * exact as f64 / port.len() as f64
        );
        assert!(unknown.is_empty(), "the run's music is ahead of the trace's: {unknown:?}");
        assert!(worst <= JITTER_FRAMES, "worst displacement {worst} frames (A12)");

        // The change frames: the port's ticks land at a frame's start and the original's
        // poll a fifth of a frame in, so a change is on time or one frame early, never a
        // shifted clock (`endlogo.md` §5.2, D28).
        let first_frame = |pairs: &BTreeMap<u32, (u8, u8)>| {
            let mut out: Vec<((u8, u8), u32)> = Vec::new();
            let mut last: Option<(u8, u8)> = None;
            for (frame, position) in pairs {
                if last != Some(*position) {
                    out.push((*position, *frame));
                    last = Some(*position);
                }
            }
            out
        };
        let trace_changes: BTreeMap<(u8, u8), u32> = first_frame(&wanted).into_iter().collect();
        let mut by_delta: BTreeMap<i64, usize> = BTreeMap::new();
        for (position, frame) in first_frame(&port) {
            if let Some(at) = trace_changes.get(&position) {
                if frame > start {
                    *by_delta.entry(i64::from(frame) - i64::from(*at)).or_insert(0) += 1;
                }
            }
        }
        println!(
            "second5: {} position change(s) compared; the port's frame minus the trace's: {by_delta:?}",
            by_delta.values().sum::<usize>()
        );
        assert!(by_delta.keys().all(|delta| (-2..=0).contains(delta)), "{by_delta:?}");

        // `_np_zframe`: the trace samples a frame before the copper polls for it (D17), so
        // the port's index N is the trace's N + 1; and, printed beside it, the unshifted
        // reading, whose worst is the shift itself rather than a drift.
        let compare = |shift: u32| {
            let (mut compared, mut off) = (0usize, Vec::new());
            for (frame, ticks) in &port_ticks {
                if let Some(want) = wanted_ticks.get(&(frame + shift)) {
                    compared += 1;
                    if ticks != want {
                        off.push(ticks - want);
                    }
                }
            }
            let worst = off.iter().map(|d: &i32| d.abs()).max().unwrap_or(0);
            println!(
                "second5: {} of {compared} frames agree on `zframe` exactly (port index N vs \
                 trace frame N+{shift}); worst displacement {worst} tick(s)",
                compared - off.len()
            );
            assert!(compared > 800, "the walk covered {compared} frames");
            worst
        };
        assert!(compare(1) <= ZFRAME_SLOP, "a drifting clock (A12)");
        assert!(compare(0) <= ZFRAME_SLOP + 1, "a drifting clock, not the sample instant (A12)");

        // Content: the positions, in order, are the trace's.
        assert_eq!(distinct(port.values().copied()), distinct(wanted.values().copied()));
    }

    #[test]
    fn a_block_one_run_starts_under_the_initial_play_song_and_matches_second1() {
        // The block-1 clock starts at 400, four frames before the run draws, so the run
        // pre-polls them; the port's change frames are a few frames behind `second1`'s
        // direct reading (`alku.md` §2 quotes the printout).
        let mut run = music_run(block_plan('1').unwrap(), stub_source, RunSettings::default());
        assert_eq!(run.anchor(), Some(404));
        assert_eq!(run.music_start_frame(), Some(404), "400-403 pre-polled");
        let Walk { start, wanted, port, .. } = walk(&mut run, "second1");

        let changes = |values: &BTreeMap<u32, (u8, u8)>| {
            let mut out: BTreeMap<(u8, u8), u32> = BTreeMap::new();
            let mut previous = None;
            for (frame, position) in values {
                if previous.is_some() && previous != Some(*position) {
                    out.entry(*position).or_insert(*frame);
                }
                previous = Some(*position);
            }
            out
        };
        let trace_changes = changes(&wanted);
        let mut by_delta: BTreeMap<i64, usize> = BTreeMap::new();
        for (position, frame) in &changes(&port) {
            if let Some(at) = trace_changes.get(position) {
                if *frame > start && *at > start {
                    *by_delta.entry(i64::from(*frame) - i64::from(*at)).or_insert(0) += 1;
                }
            }
        }
        println!(
            "second1: {} position change(s) compared; the port's frame minus the trace's: {by_delta:?}",
            by_delta.values().sum::<usize>()
        );
        // The code transitions ALKU's gates read.
        let codes = |values: &BTreeMap<u32, (u8, u8)>| {
            let mut out: BTreeMap<i32, u32> = BTreeMap::new();
            for (frame, (order, row)) in values {
                out.entry(sr_engine::music::sync_from(*order, *row)).or_insert(*frame);
            }
            out
        };
        let trace_codes = codes(&wanted);
        let mut code_delta: BTreeMap<i64, i64> = BTreeMap::new();
        for (code, frame) in &codes(&port) {
            if let Some(at) = trace_codes.get(code) {
                if *frame > start && *at > start {
                    code_delta.insert(i64::from(*code), i64::from(*frame) - i64::from(*at));
                }
            }
        }
        println!("second1: trace code transitions {trace_codes:?}");
        println!("second1: code transition deltas (port minus trace): {code_delta:?}");
        assert!(by_delta.keys().all(|delta| (-3..=9).contains(delta)), "{by_delta:?}");
        assert!(code_delta.values().all(|delta| (-3..=9).contains(delta)), "{code_delta:?}");
        assert_eq!(distinct(port.values().copied()), distinct(wanted.values().copied()));
    }
}
