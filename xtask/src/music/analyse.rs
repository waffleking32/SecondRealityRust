//! `music analyse`: read a trace back and print the numbers `docs/fidelity/music-clock.md`
//! quotes: where the music starts, the tick rate against the module's own header, the
//! quantisation of the row changes, the guest's soundcard and mixer words, `zplus`/`zinfo`, and
//! what the player does at the order list's `254`/`255` entries.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use super::format::{frames_in, Coverage, PlayerState, TraceManifest, TRACE_KIND};
use super::{MusicError, FRAME_RATE_HZ};
use crate::reference::sha256_hex;

/// `cargo xtask music analyse --trace …`.
pub fn analyse_command(trace_dir: &Path) -> Result<ExitCode, MusicError> {
    let analysis = Analysis::read(trace_dir)?;
    analysis.print()?;
    Ok(ExitCode::SUCCESS)
}

/// A trace on disk, read back, with the module its clock belongs to.
pub struct Analysis {
    pub dir: PathBuf,
    pub manifest: TraceManifest,
    pub samples: Vec<PlayerState>,
    /// The module whose order list the trace's `(ord, pat)` pairs follow, when one does.
    pub module: Option<Module>,
    /// The coverage recomputed from the trace file's own frames, so the pre-sampling boundary
    /// is named even for a manifest without `before_sampling` (`music-clock.md` §7).
    pub coverage: Coverage,
}

/// One run of constant `(speed, tempo)` in which rows advanced: what the tick
/// rate is measured over.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RateRun {
    pub speed: u8,
    pub tempo: u8,
    /// The run's first and last row change; `None` while no row has advanced.
    pub first_change: Option<u64>,
    pub last_change: Option<u64>,
    /// The row changes inside the run: `rows - 1` row intervals.
    pub rows: usize,
}

impl RateRun {
    pub fn frames(&self) -> u64 {
        match (self.first_change, self.last_change) {
            (Some(first), Some(last)) => last - first,
            _ => 0,
        }
    }

    pub fn intervals(&self) -> usize {
        self.rows.saturating_sub(1)
    }

    /// The frames the module's own rule predicts: `intervals × speed` ticks of `2.5/tempo`
    /// seconds. A prediction, not a fit.
    pub fn predicted_frames(&self) -> f64 {
        self.intervals() as f64 * f64::from(self.speed) * (2.5 / f64::from(self.tempo)) * FRAME_RATE_HZ
    }

    /// One row's worth of frames.
    pub fn row_frames(&self) -> f64 {
        f64::from(self.speed) * (2.5 / f64::from(self.tempo)) * FRAME_RATE_HZ
    }

    pub fn residual(&self) -> f64 {
        self.frames() as f64 - self.predicted_frames()
    }
}

/// An S3M header, as much of it as the tick rate needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Module {
    pub path: String,
    pub orders: Vec<u8>,
    pub patterns: u16,
    pub speed: u8,
    pub tempo: u8,
    /// The tempo the demo patches in before playing (`MAIN/STARTMUS.C:257`).
    pub patched_tempo: Option<u8>,
}

impl Module {
    pub fn playing_tempo(&self) -> u8 {
        self.patched_tempo.unwrap_or(self.tempo)
    }
}

impl Analysis {
    /// Read a trace directory: the manifest, then the file the manifest names.
    pub fn read(dir: &Path) -> Result<Analysis, MusicError> {
        let manifest_path = dir.join("manifest.json");
        let text = std::fs::read_to_string(&manifest_path).map_err(|e| MusicError::Io {
            path: manifest_path.clone(),
            message: e.to_string(),
        })?;
        let manifest: TraceManifest =
            serde_json::from_str(&text).map_err(|e| MusicError::Trace(format!(
                "{}: {e}",
                manifest_path.display()
            )))?;
        if manifest.kind != TRACE_KIND {
            return Err(MusicError::Trace(format!(
                "{} is a {:?} capture; this command reads a {TRACE_KIND}",
                manifest_path.display(),
                manifest.kind
            )));
        }
        let trace_path = dir.join(&manifest.trace.file);
        let text = std::fs::read_to_string(&trace_path).map_err(|e| MusicError::Io {
            path: trace_path.clone(),
            message: e.to_string(),
        })?;
        let digest = sha256_hex(text.as_bytes());
        if digest != manifest.trace.sha256 {
            return Err(MusicError::Trace(format!(
                "{} records sha256 {} for {} and the file is {digest}: this is not the trace the \
                 manifest describes",
                manifest_path.display(),
                manifest.trace.sha256,
                manifest.trace.file
            )));
        }
        let mut samples = Vec::new();
        for (number, line) in text.lines().enumerate() {
            if number == 0 {
                let header = line.trim_start_matches('\u{feff}');
                if header != PlayerState::header() {
                    return Err(MusicError::Trace(format!(
                        "{}'s first line is {header:?} where the columns are {:?}",
                        trace_path.display(),
                        PlayerState::header()
                    )));
                }
                continue;
            }
            if line.trim().is_empty() {
                continue;
            }
            samples.push(PlayerState::parse(line)?);
        }
        if samples.len() as u64 != manifest.trace.rows {
            return Err(MusicError::Trace(format!(
                "{} holds {} row(s) and the manifest says {}",
                trace_path.display(),
                samples.len(),
                manifest.trace.rows
            )));
        }
        // The module is the one whose order list names every `(ord, pat)` pair the trace
        // holds, not the one whose header speed matches: the block 3 trace plays MUSIC1's
        // patterns with its speed word at 6, where that file says 3.
        let coverage = Coverage::of(manifest.coverage.window, &frames_in(&samples));
        let module = sr_audio::SONGS
            .iter()
            .filter_map(|song| read_module(song.file, song.tempo_patch).ok())
            .find(|module| {
                let mut pairs = 0usize;
                for state in &samples {
                    if state.ord == 0 && state.pat == 0 {
                        continue;
                    }
                    let Some(pattern) = module.orders.get(usize::from(state.ord)) else {
                        return false;
                    };
                    if *pattern != state.pat {
                        return false;
                    }
                    pairs += 1;
                }
                pairs > 0
            });
        Ok(Analysis {
            dir: dir.to_path_buf(),
            manifest,
            samples,
            module,
            coverage,
        })
    }

    /// The frames at which the row advanced, in order.
    fn row_changes(&self) -> Vec<u64> {
        self.samples
            .windows(2)
            .filter(|pair| pair[1].row != pair[0].row || pair[1].ord != pair[0].ord)
            .map(|pair| pair[1].frame)
            .collect()
    }

    /// The runs of constant `(speed, tempo)` in which a row advanced. Per run, because the
    /// module's `Axx`/`Txx` cells change both; between the run's first and last row change,
    /// because a run starts and ends mid-row and a part-row would put up to a row of error
    /// into the residual.
    fn rate_runs(&self) -> Vec<RateRun> {
        let mut runs: Vec<RateRun> = Vec::new();
        let mut previous: Option<&PlayerState> = None;
        for state in &self.samples {
            let moved = previous.map(|earlier| earlier.row != state.row);
            match runs.last_mut() {
                Some(run) if run.speed == state.speed && run.tempo == state.tempo => {
                    if moved == Some(true) {
                        if run.first_change.is_none() {
                            run.first_change = Some(state.frame);
                        }
                        run.last_change = Some(state.frame);
                        run.rows += 1;
                    }
                }
                _ => runs.push(RateRun {
                    speed: state.speed,
                    tempo: state.tempo,
                    first_change: None,
                    last_change: None,
                    rows: 0,
                }),
            }
            previous = Some(state);
        }
        let first = runs
            .iter()
            .position(|run| run.rows > 1)
            .unwrap_or(runs.len());
        runs.drain(..first);
        runs.retain(|run| run.rows > 1);
        runs
    }

    /// A machine word as it is printed: both ways when it reads negative, because
    /// `MAIN/STMIK.H` declares some of these words `unsigned`.
    fn show_word(value: i64) -> String {
        match i16::try_from(value) {
            Ok(signed) if signed < 0 => format!(
                "{signed} ({} unsigned)",
                u16::from_le_bytes(signed.to_le_bytes())
            ),
            _ => value.to_string(),
        }
    }

    /// The first row change in the trace: its frame, the pattern's row after it,
    /// and the `zframe` value there. `None` when no row ever advanced.
    fn first_row_change(&self) -> Option<(u64, u8, i16)> {
        self.samples
            .windows(2)
            .find(|pair| pair[1].row != pair[0].row)
            .map(|pair| (pair[1].frame, pair[1].row, pair[1].zframe))
    }

    /// The runs of `_np_zframe` between its resets, as `(first frame, last frame, first value,
    /// last value)`. The player counts ticks into it, and `_dis_setmframe`
    /// (`DIS/DISC.ASM:138-148`) and a song restart reset it.
    pub fn zframe_runs(&self) -> Vec<(u64, u64, i16, i16)> {
        let mut runs: Vec<(u64, u64, i16, i16)> = Vec::new();
        for state in &self.samples {
            // The leading zeros, before the music starts, are not a run.
            if runs.is_empty() && state.zframe == 0 {
                continue;
            }
            match runs.last_mut() {
                Some(run) if state.zframe >= run.3 => {
                    run.1 = state.frame;
                    run.3 = state.zframe;
                }
                Some(_) | None => {
                    runs.push((state.frame, state.frame, state.zframe, state.zframe))
                }
            }
        }
        runs.retain(|run| run.1 > run.0);
        runs
    }

    /// The frames at which the order advanced, in order.
    fn order_changes(&self) -> Vec<(u64, u8, u8)> {
        self.samples
            .windows(2)
            .filter(|pair| pair[1].ord != pair[0].ord)
            .map(|pair| (pair[1].frame, pair[1].ord, pair[1].row))
            .collect()
    }

    fn print(&self) -> Result<(), MusicError> {
        println!("trace: {}", self.dir.display());
        println!(
            "  {} (recomputed from the trace file's own frames)",
            self.coverage.describe()
        );
        println!(
            "  oracle {}, boot {:?}, cycles {}, keys {:?}",
            self.manifest.oracle,
            self.manifest.boot,
            self.manifest.cycles,
            self.manifest.keys.join(" ")
        );
        println!(
            "  segment base 0x{:06X} (found at frame {}, still there at frame {})",
            self.manifest.locate.base,
            self.manifest.locate.frame,
            self.manifest.locate.rerun_frame
        );
        self.print_start();
        self.print_rate()?;
        self.print_quantisation();
        self.print_machine();
        self.print_zplus_zinfo();
        self.print_order_list()?;
        Ok(())
    }

    /// 1. Where the music starts.
    fn print_start(&self) {
        println!("\n1. where the music starts");
        let advance = self.manifest.check.first_advance;
        match advance {
            None => println!("  the player never advanced in this trace"),
            Some(frame) => {
                let at = self
                    .samples
                    .iter()
                    .position(|state| state.frame >= frame)
                    .expect("the advancing frame is a sample");
                println!("  first advancing frame: {frame}");
                let from = at.saturating_sub(2);
                let to = (at + 3).min(self.samples.len());
                for state in &self.samples[from..to] {
                    let earlier = self
                        .samples
                        .iter()
                        .find(|other| other.frame + 1 == state.frame);
                    let moved = earlier
                        .map(|earlier| {
                            let mut moved = Vec::new();
                            if earlier.row != state.row {
                                moved.push(format!("row {} -> {}", earlier.row, state.row));
                            }
                            if earlier.ord != state.ord {
                                moved.push(format!("ord {} -> {}", earlier.ord, state.ord));
                            }
                            if earlier.pat != state.pat {
                                moved.push(format!("pat {} -> {}", earlier.pat, state.pat));
                            }
                            if earlier.loop_ != state.loop_ {
                                moved.push(format!("loop {} -> {}", earlier.loop_, state.loop_));
                            }
                            if earlier.zframe != state.zframe {
                                moved.push(format!(
                                    "zframe {} -> {}",
                                    earlier.zframe, state.zframe
                                ));
                            }
                            if moved.is_empty() {
                                String::new()
                            } else {
                                format!("   <- {}", moved.join(", "))
                            }
                        })
                        .unwrap_or_default();
                    println!(
                        "    frame {:5}  ord {:3} row {:3} pat {:3}  zframe {:6}{moved}",
                        state.frame, state.ord, state.row, state.pat, state.zframe
                    );
                }
            }
        }
    }

    /// Section 2, the tick rate: predicted from the module's header (a tick is `2.5/tempo`
    /// seconds, a row `speed` ticks) and checked per run; then the anchor the rows imply.
    fn print_rate(&self) -> Result<(), MusicError> {
        println!("\n2. the tick rate");
        let Some(first) = self.samples.first() else {
            println!("  no samples");
            return Ok(());
        };
        let last = self.samples.last().expect("checked");
        println!(
            "  trace covers frames {}..{}: {} frames, {:.3} s at {FRAME_RATE_HZ} Hz",
            first.frame,
            last.frame,
            last.frame - first.frame,
            (last.frame - first.frame) as f64 / FRAME_RATE_HZ
        );
        match &self.module {
            Some(module) => println!(
                "  module {}: file speed {} tempo {}{}, {} order(s), {} pattern(s) \n  a tick is \
                 then 2.5/{} s = {:.4} s",
                module.path,
                module.speed,
                module.tempo,
                match module.patched_tempo {
                    Some(tempo) => format!(" (the demo patches it to {tempo})"),
                    None => String::new(),
                },
                module.orders.len(),
                module.patterns,
                module.playing_tempo(),
                2.5 / f64::from(module.playing_tempo()),
            ),
            None => println!(
                "  no module's own speed matches the trace's (speeds seen {:?})",
                self.manifest.check.speeds
            ),
        }
        let runs = self.rate_runs();
        if runs.is_empty() {
            println!("  the player never advanced a row in this trace");
            return Ok(());
        }
        println!(
            "  the rate is measured over each run of constant speed and tempo (the module's own \
             Axx/Txx cells change both):"
        );
        let mut total_frames = 0i64;
        let mut total_predicted = 0.0f64;
        let mut total_rows = 0usize;
        let longest = runs
            .iter()
            .max_by_key(|run| run.frames())
            .expect("checked not empty");
        for run in &runs {
            println!(
                "    frames {}..{} at speed {} tempo {}: {} row interval(s) in {} frame(s) — \
                 predicted {:.1} frame(s), residual {:+.1}, one row is {:.2} frame(s)",
                run.first_change.expect("a run with rows has a change"),
                run.last_change.expect("a run with rows has a change"),
                run.speed,
                run.tempo,
                run.intervals(),
                run.frames(),
                run.predicted_frames(),
                run.residual(),
                run.row_frames()
            );
            total_frames += run.frames() as i64;
            total_predicted += run.predicted_frames();
            total_rows += run.intervals();
        }
        println!(
            "  over all runs: {total_rows} row interval(s) in {total_frames} frame(s), predicted \
             {total_predicted:.1} — residual {:+.1} frame(s), {:.2}% of the measured frames",
            total_frames as f64 - total_predicted,
            100.0 * (total_frames as f64 - total_predicted) / total_frames as f64
        );
        println!(
            "  longest run: frames {}..{} at speed {} tempo {}, residual {:+.1} frame(s) over \
             {:.1} predicted — the prediction is the module's own header, not a fit",
            longest.first_change.expect("rows"),
            longest.last_change.expect("rows"),
            longest.speed,
            longest.tempo,
            longest.residual(),
            longest.predicted_frames(),
        );
        println!(
            "  the player's own words: speed {:?}, tempo {:?} (both straight out of the trace)",
            self.manifest.check.speeds, self.manifest.check.tempos
        );
        let runs = self.zframe_runs();
        let longest_zframe_run = runs
            .iter()
            .map(|run| (run.1 - run.0, i64::from(run.3) - i64::from(run.2)))
            .filter(|(frames, ticks)| *frames > 0 && *ticks > 0)
            .max_by_key(|(frames, _)| *frames);

        // The anchor the rows imply: the first row change, less its `zframe` ticks at the
        // measured frames a tick. No simulation.
        if let (Some(first_change), Some(longest)) = (self.first_row_change(), longest_zframe_run)
        {
            let frames_per_tick = longest.0 as f64 / longest.1 as f64;
            let ticks = f64::from(first_change.2);
            let implied = first_change.0 as f64 - ticks * frames_per_tick;
            let direct = self
                .samples
                .iter()
                .find(|state| state.zframe != 0)
                .map(|state| state.frame);
            println!(
                "  the anchor the rows imply: the first row change is at frame {} with \
                 `_np_zframe` {}, and {} ticks at the measured {frames_per_tick:.4} frames a tick \
                 is {:.1} frames — so that row began at frame {:.1} (± the tick the row \
                 boundary is within and the frame the change is seen on)",
                first_change.0,
                first_change.2,
                first_change.2,
                ticks * frames_per_tick,
                implied
            );
            if let Some(direct) = direct {
                let apart = (direct as f64 - implied).abs();
                println!(
                    "  the anchor the `zframe` column shows: frame {direct} is the first frame \
                     whose `zframe` is not 0 — {apart:.1} frame(s), or {:.1} tick(s), from the \
                     rows' reading: the same tick, seen at the next whole-frame sample. Both \
                     readings are quantised to a whole frame, so the gap is one frame plus the \
                     tick the row change is seen within; it is not a claim that they agree to \
                     within a tick, and the two numbers above say how big it is.",
                    apart / frames_per_tick
                );
            }
        }

        // `_np_zframe`, the word the parts wait on (`DIS/DISINT.ASM:412-425`), over its
        // longest run without a reset.
        if !runs.is_empty() {
            let resets: usize = runs.len() - 1;
            let longest = runs
                .iter()
                .max_by_key(|run| run.1 - run.0)
                .expect("checked not empty");
            let (first_frame, last_frame, first, last) = *longest;
            let ticks = i64::from(last) - i64::from(first);
            let frames = last_frame - first_frame;
            if frames > 0 {
                println!(
                    "  _np_zframe: {} at frame {} to {} at frame {} — {ticks} tick(s) in {frames} \
                     frames = {:.4} per frame = {:.2} per second, over the longest run without a \
                     reset ({resets} reset(s) in the trace)",
                    first,
                    first_frame,
                    last,
                    last_frame,
                    ticks as f64 / frames as f64,
                    ticks as f64 / frames as f64 * FRAME_RATE_HZ
                );
            }
        }
        Ok(())
    }

    /// 3. The quantisation: the gaps between row changes, per run of constant speed and tempo.
    fn print_quantisation(&self) {
        println!("\n3. the quantisation");
        let changes = self.row_changes();
        if changes.len() < 2 {
            println!("  fewer than two row changes: nothing to measure");
            return;
        }
        println!(
            "  {} row change(s) in the trace; the frames themselves, first 32: {:?}",
            changes.len(),
            &changes[..changes.len().min(32)]
        );
        for run in self.rate_runs() {
            let (Some(first), Some(last)) = (run.first_change, run.last_change) else {
                continue;
            };
            let inside: Vec<u64> = changes
                .iter()
                .copied()
                .filter(|frame| first <= *frame && *frame <= last)
                .collect();
            if inside.len() < 2 {
                continue;
            }
            let mut gaps: BTreeMap<u64, usize> = BTreeMap::new();
            for pair in inside.windows(2) {
                *gaps.entry(pair[1] - pair[0]).or_insert(0) += 1;
            }
            let histogram: Vec<String> = gaps
                .iter()
                .map(|(gap, count)| format!("{gap} frame(s) x{count}"))
                .collect();
            println!(
                "  frames {}..{} at speed {} tempo {}: {} gap(s) — {}",
                first,
                last,
                run.speed,
                run.tempo,
                inside.len() - 1,
                histogram.join(", ")
            );
            println!(
                "    distinct gap sizes: {} (one row is {:.1} frame(s) at this speed/tempo)",
                gaps.len(),
                run.row_frames()
            );
            // Gaps outside `floor`..`ceil` of the row length, and where in the run they are: at
            // an edge a gap is a boundary, in the middle a fact about the player.
            let exact = run.row_frames();
            let (below, above) = (exact.floor() as u64, exact.ceil() as u64);
            let outliers: Vec<String> = inside
                .windows(2)
                .enumerate()
                .filter(|(_, pair)| {
                    let gap = pair[1] - pair[0];
                    gap < below || gap > above
                })
                .map(|(at, pair)| {
                    format!("{} of {} ({} -> {}, {})", at + 1, inside.len() - 1, pair[0], pair[1], pair[1] - pair[0])
                })
                .collect();
            if !outliers.is_empty() {
                let shown: Vec<&String> = outliers.iter().take(8).collect();
                println!(
                    "    the {} gap(s) outside the {below}..{above} the run's own row length \
                     ({exact:.2} frames) brackets, by their position in the run: {}{}",
                    outliers.len(),
                    shown
                        .iter()
                        .map(|entry| entry.as_str())
                        .collect::<Vec<_>>()
                        .join(", "),
                    if outliers.len() > shown.len() {
                        format!(", and {} more", outliers.len() - shown.len())
                    } else {
                        String::new()
                    }
                );
            }
        }
    }

    /// 4. The machine's card and mixer words.
    fn print_machine(&self) {
        println!("\n4. the soundcard and mixer, from the guest");
        for (name, value) in &self.manifest.machine {
            let meaning = match (name.as_str(), *value) {
                ("_zoutputmode", 0) => " (no sound)",
                ("_zoutputmode", 1) => " (Sound Blaster)",
                ("_zoutputmode", 2) => " (Gravis Ultrasound)",
                ("_zoutputmode", 7) => " (no sound)",
                ("_zpollmix", 0) => {
                    " (0 at the locate: the demo has not switched the player to the polled \
                     mode yet)"
                }
                ("_zpollmix", 1) => " (the caller polls the player: `_zpollme` once a frame)",
                ("_znoborders", 1) => " (no border colours)",
                _ => "",
            };
            println!("  {name:<16} {}{meaning}", Self::show_word(*value));
        }
        for (name, value) in &self.manifest.machine_moved {
            println!(
                "  {name:<16} moved during the trace: {} at the locate, {} at the end — a word \
                 whose two readings differ is recorded as both, not merged",
                Self::show_word(self.manifest.machine.get(name).copied().unwrap_or(0)),
                Self::show_word(*value)
            );
        }
        if self.manifest.machine_moved.is_empty() {
            println!(
                "  (this capture records the locate-time readings only: it has no \
                 `machine_moved`, so what these words became when the music started is not in \
                 the file — `music-clock.md` §9)"
            );
        }
        println!(
            "  addresses: {}",
            self.manifest
                .locate
                .words
                .iter()
                .map(|(name, address)| format!("{name} 0x{address:06X}"))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    /// 5. `zplus` and `zinfo`.
    fn print_zplus_zinfo(&self) {
        println!("\n5. zplus and zinfo");
        let mut zplus_values: Vec<i16> = Vec::new();
        let mut zinfo_values: Vec<i16> = Vec::new();
        for state in &self.samples {
            if !zplus_values.contains(&state.zplus) {
                zplus_values.push(state.zplus);
            }
            if !zinfo_values.contains(&state.zinfo) {
                zinfo_values.push(state.zinfo);
            }
        }
        println!("  zplus values: {zplus_values:?}");
        println!("  zinfo values: {zinfo_values:?}");

        // Is zplus a function of (ord, row)?
        let mut by_position: BTreeMap<(u8, u8), Vec<i16>> = BTreeMap::new();
        for state in &self.samples {
            let values = by_position.entry((state.ord, state.row)).or_default();
            if !values.contains(&state.zplus) {
                values.push(state.zplus);
            }
        }
        let conflicting: Vec<_> = by_position
            .iter()
            .filter(|(_, values)| values.len() > 1)
            .collect();
        println!(
            "  zplus against (ord, row): {} distinct position(s), {} of them carrying more than \
             one zplus value",
            by_position.len(),
            conflicting.len()
        );
        for ((ord, row), values) in conflicting.iter().take(8) {
            println!("    ord {ord} row {row}: zplus {values:?}");
        }
        // The transitions: the rows at which zplus changes within one order.
        let transitions: Vec<(u8, u8, i16, i16)> = self
            .samples
            .windows(2)
            .filter(|pair| pair[1].zplus != pair[0].zplus)
            .map(|pair| {
                (
                    pair[1].ord,
                    pair[1].row,
                    pair[0].zplus,
                    pair[1].zplus,
                )
            })
            .collect();
        println!(
            "  zplus transitions: {} (first 12: {:?})",
            transitions.len(),
            &transitions[..transitions.len().min(12)]
        );

        // zinfo: the frames each value covers, as runs.
        let mut runs: Vec<(i16, u64, u64)> = Vec::new();
        for state in &self.samples {
            match runs.last_mut() {
                Some(run) if run.0 == state.zinfo => run.2 = state.frame,
                _ => runs.push((state.zinfo, state.frame, state.frame)),
            }
        }
        println!("  zinfo runs: {}", runs.len());
        for (value, first, last) in runs.iter().take(12) {
            let at = self.samples.iter().find(|state| state.frame == *last);
            println!(
                "    {value:6}  frames {first}..{last} ({}){}",
                last - first + 1,
                at.map(|state| format!(", ending at ord {} row {}", state.ord, state.row))
                    .unwrap_or_default()
            );
        }
        if runs.len() > 12 {
            println!("    … and {} more run(s)", runs.len() - 12);
        }
        let with_position: Vec<_> = self
            .samples
            .windows(2)
            .filter(|pair| pair[1].zinfo != pair[0].zinfo)
            .collect();
        let same_row: usize = with_position
            .iter()
            .filter(|pair| pair[1].row == pair[0].row)
            .count();
        println!(
            "  zinfo changes on {} frame(s); on {same_row} of them the row did not change, and on \
             {} the row changed too",
            with_position.len(),
            with_position.len() - same_row
        );
        if self.manifest.check.z_info_is_constant() {
            println!(
                "  zinfo holds {} for the whole trace: this capture does NOT measure what its \
                 other values are — the record says so rather than reading a rule off the \
                 modules' cells (D8)",
                zinfo_values.first().copied().unwrap_or(0)
            );
        }
        let zplus_and_zinfo: usize = self
            .samples
            .windows(2)
            .filter(|pair| pair[1].zinfo != pair[0].zinfo && pair[1].zplus != pair[0].zplus)
            .count();
        println!("  zinfo and zplus change together on {zplus_and_zinfo} frame(s)");
    }

    /// Section 6: what the player does at the order list's `254` and `255` entries, read off
    /// the order counter's own sequence; an entry the trace never reached is unmeasured.
    fn print_order_list(&self) -> Result<(), MusicError> {
        println!("\n6. the order list, and the 254/255 entries");
        let changes = self.order_changes();
        let held: BTreeSet<u8> = self.samples.iter().map(|state| state.ord).collect();
        println!(
            "  the order counter held {} distinct value(s); its changes ({}):",
            held.len(),
            changes.len()
        );
        let shown: Vec<String> = changes
            .iter()
            .take(24)
            .map(|(frame, ord, row)| format!("{frame}: ord {ord} row {row}"))
            .collect();
        println!("    {}", shown.join(" | "));
        if changes.len() > 24 {
            println!("    … and {} more", changes.len() - 24);
        }
        match &self.module {
            Some(module) => {
                let entries: Vec<(usize, u8)> = module
                    .orders
                    .iter()
                    .enumerate()
                    .filter(|(_, entry)| **entry == 254 || **entry == 255)
                    .map(|(index, entry)| (index, *entry))
                    .collect();
                println!(
                    "  {}'s order list: {} order(s), {} 254/255 entry/entries at {}",
                    module.path,
                    module.orders.len(),
                    entries.len(),
                    entries
                        .iter()
                        .map(|(index, entry)| format!("{index}={entry}"))
                        .collect::<Vec<_>>()
                        .join(", ")
                );
                for (index, entry) in &entries {
                    let value = *index as u8;
                    match self.samples.iter().find(|state| state.ord == value) {
                        Some(state) => {
                            // The counter rested on the entry: what came next?
                            let after = self
                                .samples
                                .iter()
                                .find(|later| later.frame > state.frame && later.ord != value);
                            println!(
                                "    order {index} ({entry}): the counter held it at frame {} \
                                 (row {}) and then went to {}",
                                state.frame,
                                state.row,
                                after
                                    .map(|later| format!("ord {} row {}", later.ord, later.row))
                                    .unwrap_or_else(|| "the end of the trace".into())
                            );
                        }
                        None => {
                            let before = held.range(..value).next_back();
                            let after = held.range(value + 1..).next();
                            match (before, after) {
                                // Both list neighbours were played: the player steps over it.
                                (Some(before), Some(after))
                                    if *before + 1 == value && value + 1 == *after =>
                                {
                                    println!(
                                        "    order {index} ({entry}): never held — the counter \
                                         went from ord {before} to ord {after} without resting on \
                                         {value}, having played each of them, so the player steps \
                                         over this entry"
                                    )
                                }
                                (Some(before), None) => println!(
                                    "    order {index} ({entry}): never reached — the counter \
                                     stopped at ord {before} in this trace, so what the player \
                                     does at this entry is NOT measured by it"
                                ),
                                // A jump (a restart elsewhere): not measured.
                                _ => println!(
                                    "    order {index} ({entry}): never held — the counter went \
                                     from {} to {}, a jump rather than a step, so this entry was \
                                     not played in this trace and what the player does at it is \
                                     NOT measured by it",
                                    before
                                        .map(|ord| format!("ord {ord}"))
                                        .unwrap_or_else(|| "nothing".into()),
                                    after
                                        .map(|ord| format!("ord {ord}"))
                                        .unwrap_or_else(|| "nothing".into())
                                ),
                            }
                        }
                    }
                }
            }
            None => println!("  no module matched the trace's speed, so its orders cannot be read"),
        }
        Ok(())
    }

}

/// Read an S3M's header, as much of it as the tick rate needs: the order list, the pattern
/// count and the speed and tempo bytes (`0x31`, `0x32`).
fn read_module(path: &str, patched_tempo: Option<u8>) -> Result<Module, MusicError> {
    let bytes = std::fs::read(path).map_err(|e| MusicError::Io {
        path: PathBuf::from(path),
        message: e.to_string(),
    })?;
    if bytes.len() < 0x60 || &bytes[0x2C..0x30] != b"SCRM" {
        return Err(MusicError::Trace(format!("{path} is not an S3M")));
    }
    let orders = usize::from(u16::from_le_bytes([bytes[0x20], bytes[0x21]]));
    let patterns = u16::from_le_bytes([bytes[0x24], bytes[0x25]]);
    let order_list = bytes
        .get(0x60..0x60 + orders)
        .ok_or_else(|| MusicError::Trace(format!("{path} is shorter than its order list")))?
        .to_vec();
    Ok(Module {
        path: path.to_string(),
        orders: order_list,
        patterns,
        speed: bytes[0x31],
        tempo: bytes[0x32],
        patched_tempo,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `MAIN/STMIK.H` declares some machine words `unsigned` (`_zmixspeed`, `_zbufsize`), so a
    /// reading above 32767 prints both ways: 40000 is -25536 as a signed word.
    #[test]
    fn a_machine_word_above_32767_is_printed_both_ways() {
        assert_eq!(Analysis::show_word(40000), "40000");
        assert_eq!(Analysis::show_word(-25536), "-25536 (40000 unsigned)");
        assert_eq!(Analysis::show_word(192), "192");
        assert_eq!(Analysis::show_word(0), "0");
    }
}
