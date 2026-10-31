//! The trace on disk: `trace.csv`'s rows and the manifest beside it. `reference verify` reads
//! these types, so every filed `reference/music/*/manifest.json` has to keep deserialising, and
//! `crates/sr-audio/tests/trace.rs` and `bins/secondreality/tests/fullrun.rs` parse [`COLUMNS`]
//! in this order.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::locate::{Needle, SymbolMap};
use super::MusicError;
use crate::reference::BootRecipe;

/// The manifest's `kind` for a trace, which is how `reference verify` tells one from a frame
/// capture.
pub const TRACE_KIND: &str = "music-trace";

/// The trace's own file name, beside `manifest.json`.
pub const TRACE_FILE: &str = "trace.csv";

/// The columns `trace.csv` writes, in order.
pub const COLUMNS: &[&str] = &[
    "frame", "ord", "row", "pat", "loop", "speed", "tempo", "zinfo", "zplus", "zframe",
];

/// One traced row: the player's own state at one frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerState {
    /// The frame counter the guest reported, counted from the emulator's first rendered frame.
    pub frame: u64,
    pub ord: u8,
    pub row: u8,
    pub pat: u8,
    /// `_np_loop` (`MAIN/STMIK.H:125`), the S3M's own loop counter.
    pub loop_: u8,
    pub speed: u8,
    pub tempo: u8,
    pub zinfo: i16,
    pub zplus: i16,
    pub zframe: i16,
}

impl PlayerState {
    /// The CSV line, in [`COLUMNS`]' order.
    pub fn line(&self) -> String {
        format!(
            "{},{},{},{},{},{},{},{},{},{}",
            self.frame,
            self.ord,
            self.row,
            self.pat,
            self.loop_,
            self.speed,
            self.tempo,
            self.zinfo,
            self.zplus,
            self.zframe
        )
    }

    pub fn header() -> String {
        COLUMNS.join(",")
    }

    /// Parse one row, refusing anything that is not exactly the columns.
    pub fn parse(line: &str) -> Result<PlayerState, MusicError> {
        let fields: Vec<&str> = line.split(',').collect();
        if fields.len() != COLUMNS.len() {
            return Err(MusicError::Trace(format!(
                "a trace row has {} field(s) where {} columns are declared: {line:?}",
                fields.len(),
                COLUMNS.len()
            )));
        }
        let number = |at: usize| -> Result<i64, MusicError> {
            fields[at].trim().parse::<i64>().map_err(|e| {
                MusicError::Trace(format!("column {} is not a number: {e}", COLUMNS[at]))
            })
        };
        let byte = |at: usize| -> Result<u8, MusicError> {
            u8::try_from(number(at)?)
                .map_err(|_| MusicError::Trace(format!("column {} is not a byte", COLUMNS[at])))
        };
        Ok(PlayerState {
            frame: u64::try_from(number(0)?)
                .map_err(|_| MusicError::Trace("the frame column is negative".into()))?,
            ord: byte(1)?,
            row: byte(2)?,
            pat: byte(3)?,
            loop_: byte(4)?,
            speed: byte(5)?,
            tempo: byte(6)?,
            zinfo: i16::try_from(number(7)?)
                .map_err(|_| MusicError::Trace("zinfo does not fit 16 bits".into()))?,
            zplus: i16::try_from(number(8)?)
                .map_err(|_| MusicError::Trace("zplus does not fit 16 bits".into()))?,
            zframe: i16::try_from(number(9)?)
                .map_err(|_| MusicError::Trace("zframe does not fit 16 bits".into()))?,
        })
    }

    /// Whether any word of the position moved between two samples: what "the player is
    /// advancing" means for the music's start frame.
    pub(super) fn position_moved_from(&self, earlier: &PlayerState) -> bool {
        self.ord != earlier.ord
            || self.row != earlier.row
            || self.pat != earlier.pat
            || self.loop_ != earlier.loop_
            || self.zframe != earlier.zframe
    }
}

/// The frames of a sample list, for a coverage recomputed from them.
pub(super) fn frames_in(samples: &[PlayerState]) -> Vec<u64> {
    samples.iter().map(|state| state.frame).collect()
}

/// What a trace covers: the window asked for, and the frames actually sampled.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Coverage {
    /// The window the command sampled, inclusive at both ends.
    pub window: [u64; 2],
    pub requested: u64,
    pub first: u64,
    pub last: u64,
    pub samples: u64,
    pub per_frame: bool,
    /// The runs of frames the window asked for and the trace does not hold, as inclusive
    /// `[first, last]` pairs.
    pub missing: Vec<[u64; 2]>,
    /// The first run of [`Coverage::missing`] when it is the frames gone before sampling began
    /// (the locate's own cost). Absent in older manifests.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before_sampling: Option<[u64; 2]>,
}

impl Coverage {
    /// The coverage a window and a list of sampled frames make. `reference verify` recomputes
    /// a manifest's claim through this, so it must not panic on manifest data: a window whose
    /// last frame is before its first, or whose span does not fit a `u64`, is an empty
    /// coverage (zero requested, nothing missing, not per-frame), which then disagrees with
    /// the claim and is reported as a finding.
    pub(crate) fn of(window: [u64; 2], frames: &[u64]) -> Coverage {
        let requested = window[1]
            .checked_sub(window[0])
            .and_then(|span| span.checked_add(1));
        let Some(requested) = requested else {
            return Coverage {
                window,
                requested: 0,
                first: frames.first().copied().unwrap_or(window[0]),
                last: frames.last().copied().unwrap_or(window[1]),
                samples: frames.len() as u64,
                per_frame: false,
                missing: Vec::new(),
                before_sampling: None,
            };
        };
        let mut missing = Vec::new();
        let mut cursor = window[0];
        for frame in frames {
            if *frame > cursor {
                missing.push([cursor, *frame - 1]);
            }
            cursor = frame.saturating_add(1);
        }
        if cursor <= window[1] {
            missing.push([cursor, window[1]]);
        }
        let before_sampling = match (missing.first(), frames.first()) {
            (Some([from, to]), Some(first)) if *to < *first && *from == window[0] => {
                Some([*from, *to])
            }
            _ => None,
        };
        Coverage {
            window,
            requested,
            first: frames.first().copied().unwrap_or(window[0]),
            last: frames.last().copied().unwrap_or(window[0]),
            samples: frames.len() as u64,
            per_frame: missing.is_empty(),
            missing,
            before_sampling,
        }
    }

    /// How many frames of the window are missing, saturating: the runs may come out of a
    /// manifest.
    fn gaps(&self) -> u64 {
        self.missing
            .iter()
            .map(|[first, last]| last.saturating_sub(*first).saturating_add(1))
            .fold(0u64, u64::saturating_add)
    }

    /// Every field of this coverage that disagrees with `recorded`, naming the field.
    /// `before_sampling` is compared only when the manifest records one (D23's additive rule).
    pub(crate) fn disagreements(&self, recorded: &Coverage) -> Vec<String> {
        let mut out = Vec::new();
        let mut field = |name: &str, mine: String, theirs: String| {
            if mine != theirs {
                out.push(format!("{name}: the file makes it {mine}, the manifest says {theirs}"));
            }
        };
        field(
            "window",
            format!("{}..{}", self.window[0], self.window[1]),
            format!("{}..{}", recorded.window[0], recorded.window[1]),
        );
        field(
            "requested",
            self.requested.to_string(),
            recorded.requested.to_string(),
        );
        field(
            "first/last",
            format!("{}..{}", self.first, self.last),
            format!("{}..{}", recorded.first, recorded.last),
        );
        field(
            "samples",
            self.samples.to_string(),
            recorded.samples.to_string(),
        );
        field(
            "per_frame",
            self.per_frame.to_string(),
            recorded.per_frame.to_string(),
        );
        field(
            "missing",
            format!("{:?}", self.missing),
            format!("{:?}", recorded.missing),
        );
        if let Some(theirs) = recorded.before_sampling {
            field(
                "before_sampling",
                format!("{:?}", self.before_sampling),
                format!("{:?}", Some(theirs)),
            );
        }
        out
    }

    /// The sentence the tool prints, and the one the note may quote.
    pub fn describe(&self) -> String {
        if self.per_frame {
            format!(
                "frames {}..{} of the requested {}..{}, every one of them ({} sample(s))",
                self.first, self.last, self.window[0], self.window[1], self.samples
            )
        } else {
            let missing: Vec<String> = self
                .missing
                .iter()
                .map(|[first, last]| {
                    if first == last {
                        format!("{first}")
                    } else {
                        format!("{first}..{last}")
                    }
                })
                .collect();
            let boundary = self
                .before_sampling
                .map(|[from, to]| {
                    format!(
                        " (frames {from}..{to} of them are the boundary: the segment was located \
                         after they had gone)"
                    )
                })
                .unwrap_or_default();
            format!(
                "frames {}..{} of the requested {}..{}: {} sample(s), {} frame(s) missing \
                 ({}){boundary} — this trace is NOT per-frame",
                self.first,
                self.last,
                self.window[0],
                self.window[1],
                self.samples,
                self.gaps(),
                missing.join(", ")
            )
        }
    }
}

/// One run of frames the trace could not sample, and what the guest's counter did around it:
/// the sampler's observation, which the verifier cannot recompute.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Skipped {
    /// The frames that are not in the trace, inclusive.
    pub from: u64,
    pub to: u64,
    pub counter_before: u64,
    pub counter_after: u64,
    pub why: String,
}

/// The trace file, as the manifest records it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceFile {
    pub file: String,
    pub columns: Vec<String>,
    pub rows: u64,
    pub bytes: u64,
    pub sha256: String,
}

/// Where the segment was found, how, and whether it was still there at the end.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocateRecord {
    /// The segment base every sample is read through, in the guest's paged address space.
    pub base: u32,
    /// Every base the needles were found at, sorted; the trace reads through the lowest.
    pub views: Vec<u32>,
    /// Why several views are one memory, when there are several.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aliasing: Option<String>,
    /// The pair the oracle's memory read takes for `base` (`segment * 16 + offset`).
    pub segment: u16,
    pub offset: u32,
    /// The guest's frame when the segment was first found.
    pub frame: u64,
    /// The address of every word the trace reads, `base + offset`.
    pub words: BTreeMap<String, u32>,
    pub needles: Vec<Needle>,
    /// What the re-locate at the end of the trace found; `base` must be among the views.
    pub rerun_base: u32,
    pub rerun_views: Vec<u32>,
    pub rerun_frame: u64,
}

/// What the words did over the whole capture: they behave like the counters they are named
/// after.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Behaviour {
    pub row: [u8; 2],
    pub ord: [u8; 2],
    pub pat: [u8; 2],
    /// Every distinct speed and tempo, in order of first appearance.
    pub speeds: Vec<u8>,
    pub tempos: Vec<u8>,
    #[serde(default)]
    pub zinfo_min: i16,
    #[serde(default)]
    pub zinfo_max: i16,
    /// The first frame at which a row, an order, a pattern, a loop or `zframe` moved.
    pub first_advance: Option<u64>,
    pub zframe_moves: u64,
    pub zframe_total: i64,
}

impl Behaviour {
    pub fn z_info_is_constant(&self) -> bool {
        self.zinfo_max == self.zinfo_min
    }
}

/// `<DIR>/manifest.json` for a trace.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraceManifest {
    pub version: u32,
    /// [`TRACE_KIND`].
    pub kind: String,
    pub oracle: String,
    pub upstream_commit: String,
    pub exe: String,
    pub arguments: Vec<String>,
    pub cycles: u32,
    pub frame_origin: String,
    pub skip_boot_frames: u64,
    pub boot: BootRecipe,
    pub keys: Vec<String>,
    pub coverage: Coverage,
    pub trace: TraceFile,
    pub object: SymbolMap,
    pub locate: LocateRecord,
    /// The machine's words, read when the segment was found.
    pub machine: BTreeMap<String, i64>,
    /// The words whose reading at the end of the trace differs from `machine`'s. Nothing is
    /// assumed constant: `_zmixspeed` goes 20000 → 40000 in `second5b`, `_zpollmix` 0 → 1, and
    /// the volume words move under `fademusic` (`MAIN/U2.ASM:333-346`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub machine_moved: BTreeMap<String, i64>,
    pub check: Behaviour,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skipped: Vec<Skipped>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The CSV is the record: a row survives the round trip, and a row that is not the columns
    /// is refused rather than half-read.
    #[test]
    fn a_row_round_trips_and_a_malformed_row_is_refused() {
        let state = PlayerState {
            frame: 1234,
            ord: 25,
            row: 63,
            pat: 70,
            loop_: 1,
            speed: 6,
            tempo: 120,
            zinfo: 0,
            zplus: -1,
            zframe: 385,
        };
        let line = state.line();
        assert_eq!(PlayerState::parse(&line).unwrap(), state);
        assert_eq!(line, "1234,25,63,70,1,6,120,0,-1,385");
        assert!(PlayerState::parse("1,2,3").is_err());
        assert!(PlayerState::parse("1,2,3,4,5,6,7,8,9,70000").is_err(), "not a 16-bit word");
        assert!(PlayerState::parse("1,300,3,4,5,6,7,8,9,10").is_err(), "not a byte");
        assert!(PlayerState::parse("1,2,3,4,5,6,7,8,9,x").is_err());
    }

    /// The coverage is recomputed from the frames, so a trace that sampled every other frame
    /// cannot describe itself as per-frame, and the run before sampling began is named.
    #[test]
    fn a_trace_with_a_hole_is_not_per_frame() {
        let every = Coverage::of([10, 19], &(10..=19).collect::<Vec<u64>>());
        assert!(every.per_frame, "{}", every.describe());
        assert!(every.missing.is_empty());
        assert_eq!(every.before_sampling, None);

        let holed = Coverage::of([10, 19], &[10, 11, 13, 14, 19]);
        assert!(!holed.per_frame);
        assert_eq!(holed.missing, vec![[12, 12], [15, 18]]);
        assert!(holed.describe().contains("NOT per-frame"), "{}", holed.describe());
        assert_eq!(holed.gaps(), 5);
        assert_eq!(holed.before_sampling, None);

        let late = Coverage::of([0, 19], &[10, 11, 12]);
        assert!(!late.per_frame);
        assert_eq!(late.missing, vec![[0, 9], [13, 19]]);
        assert_eq!(late.before_sampling, Some([0, 9]));
        assert!(late.describe().contains("the boundary"), "{}", late.describe());
    }

    /// A window out of a manifest is data a verifier has to disbelieve: back to front, or a
    /// span that does not fit a `u64`, is the empty coverage and never a panic; an absurd but
    /// representable span is counted, saturating.
    #[test]
    fn a_window_that_has_no_frame_count_is_not_a_panic() {
        for window in [[100, 10], [0, u64::MAX], [u64::MAX, u64::MAX - 1], [u64::MAX, 0], [7, 6]] {
            let coverage = Coverage::of(window, &[10, 11, 12]);
            assert_eq!(coverage.requested, 0, "{window:?}");
            assert!(!coverage.per_frame, "{window:?}");
            assert!(coverage.missing.is_empty(), "{window:?}");
            assert_eq!(coverage.before_sampling, None, "{window:?}");
            assert_eq!((coverage.first, coverage.samples), (10, 3), "{window:?}");
            assert_eq!(coverage.gaps(), 0, "{window:?}");
            let _ = coverage.describe();
        }
        let absurd = Coverage::of([1, u64::MAX], &[10, 11, 12]);
        assert_eq!(absurd.requested, u64::MAX);
        assert_eq!(absurd.gaps(), u64::MAX - 3, "two runs: 1..9 and 13..u64::MAX");
        let _ = absurd.describe();

        let saturating = Coverage {
            missing: vec![[0, u64::MAX]],
            ..absurd
        };
        assert_eq!(saturating.gaps(), u64::MAX);
    }
}
