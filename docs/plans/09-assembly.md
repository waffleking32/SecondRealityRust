# Phase 9 — Assembly & Fidelity Pass Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the port play the demo the way `SECOND.EXE` does — a settings screen, then the full sequence with its music restarts, cue wait, loop mode and its own ending — prove the assembled run against the original, re-diff every part with one command, and complete the two fidelity registers.

**Architecture:** The shell (`bins/secondreality/src/shell.rs`) gains the runner's missing tail — `MAIN/U2.ASM:914-994`: the looping rule, the loop pass, and the `@@partsdone` exit — behind a `RunSettings` value that the command line and a new window-only settings screen both produce. `sr-audio` gains an optional enhanced renderer that rides the authentic tick schedule. Verification adds captures of the original (the loop seam, two lead-in windows, the full `SECOND 1` run and its music traces) and `cargo xtask rediff`, a release-mode walker that re-renders every filed reference at its measured mapping and checks every frame against its registered count. The registers are then completed against the audit carried below.

**Tech Stack:** Rust 2021; `crates/sr-engine`, `crates/sr-audio` (zero runtime dependencies), `crates/sr-assets`; SDL2 only in the binary; `xtask` (capture, compare, verify, music trace) driving `dosbox-automation` at 30000 cycles. No new crate and no new dependency.

**Spec:** `docs/spec/2026-10-10-second-reality-rust-design.md` §1 success criteria 1 and 3 (`:29-35`) and the settings-screen non-goal (`:46-47`), §2.2 loop mode (`:111-113`), §4.2 presentation settings (`:333-338`), §5 (`:416-452`), §7.2 the mix-rate option (`:564-566`), §8 (`:618-667`), §10 Phase 9 (`:706`), §13. Binding rules: `docs/decisions.md` D7, D12, D13, D16, D17–D21, D24, D25, D29, D31, D34, D38, D40; `docs/fidelity/part-sequence.md` §4–§7; `docs/fidelity/APPROXIMATIONS.md`; `docs/fidelity/bugs.md`; `CLAUDE.md`.

## Global Constraints

- **The reference is the authority (D24); the original's code is the specification of behaviour.** Shell behaviour is transcribed from `MAIN/U2.ASM` (ISO-8859, CRLF: use `grep -a` or `sed -n`), `MAIN/START.C` and `MAIN/MENU.C`, and each new mode is checked against a capture of the original where one can be taken.
- **Nothing already verified may move.** Every per-part test's registered count stays exactly as it is unless a task here re-measures it and says why; `--part` runs still hold at their end; the vault's semantics (D40) are unchanged; Ctrl still skips one part (D21's recorded difference); headless runs never open SDL and never wait on a key.
- **The music clock is the picture.** No new mode — a loop pass, enhanced audio, sound off, any setting — may change the `(order, row)` the engine publishes at any tick of any run. Tests pin it.
- **New non-fidelity surfaces are the port's own and are labelled so:** the settings screen (spec §1 replaces `START.EXE`), the end screen printed to the launching terminal, and the enhanced renderer (off by default). None changes a compared frame.
- **Assets in their original formats.** The end screen is `assets/MAIN/DATA/U2END.BIN` (4,000 bytes, SHA-256 `4c2ae297…be99cf4`, identical to `upstream/SecondReality/MAIN/DATA/U2END.BIN`); menu text is drawn from `assets/ALKU/FONA.UH`.
- **`reference/` and `upstream/` are input-only.** Sanctioned writes: capture output under `reference/parts/<new name>` and traces via `cargo xtask music trace --out reference/music/<new name>`. Never overwrite an existing capture. `out/` is scratch. 6-bit palette internally; never spin-wait.
- **`sr-engine` and `sr-audio` keep zero runtime dependencies; `sr-assets` depends on `sr-engine` only.**
- **Every number in a note is reproducible by a command the note names, or is produced by a test in the same commit** (D25). A quoted block is a paste or a marked elision (D29).
- **Gate:** `cargo test --workspace` green with zero warnings; `cargo clippy --workspace --all-targets -- -D warnings` and `cargo clippy --workspace --all-targets --release -- -D warnings` clean. Commits cite the original's file:line and end with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## Review Focus

Five failure modes the spec implies that no single task's own tests would otherwise catch. Each is pinned by the named test in the task that owns the code.

1. **A setting lost at a mid-run restart.** `MusicSource` is a bare `fn(ModuleId)` (`shell.rs:410`) and `audio::load_player` hard-codes `PlayerOptions::default()` (`audio.rs:164-186`), so a quality chosen on the settings screen would silently revert at GLENZ's `MUSIC1@0` or U2E's `MUSIC0@18`. Pinned by Task 7's `a_restart_keeps_the_runs_sound_quality`.
2. **A loop pass that fires the wrong restarts.** Pass 2 must re-fire exactly the full run's sites (`MUSIC1@0` before GLENZ, `MUSIC0@18` before U2E), never the entry-only ones (`MUSIC1@42`, `MUSIC0@25`), never DDSTARS, never ENDSCRL, and must run every block even when the run began at `--block 3` (`whattorun = 7fh`, `U2.ASM:919`). Pinned by Task 4's `a_loop_pass_fires_only_the_full_runs_restarts` and `a_loop_pass_runs_every_block_whatever_the_entry`.
3. **The end of a demo run against the end of a part run.** A demo run (`--block`, or no argument) ends — fade, blank, close — while a `--part` or vault run still holds, and a headless run never blocks. Pinned by Task 3's `a_demo_run_fades_then_blanks_then_ends` and `a_part_run_still_holds_after_its_end`.
4. **Settings that do not reach their consumer before it is built.** Aspect, border and scale size the window; sound off must leave the player running; looping shapes the plan. Pinned by Task 8's `the_chosen_settings_are_the_runs_settings`.
5. **A re-diff that passes by omission.** A capture missing from the walker's table, or a frame the dump lacks, must fail loudly (exit 2), never pass. Pinned by Task 5's `the_table_names_every_filed_capture_once` and `a_frame_the_dump_lacks_is_exit_2`.

---

## The measured ground truth

Carried from this phase's four research reports (shell, settings, registers, re-diff) and the records they cite. **The implementer re-derives each row against the file before relying on it** — the plan is an argument, and the artifacts are the evidence.

| Item | What it is | Where |
|---|---|---|
| Runner tail | `:914-917` ENDSCRL only when `looping == 0`; `:919` `whattorun = 07fh` every pass; `:920-925` re-enter only if no Ctrl and `looping ∉ {0, 2}`; `:926-928` `fademusic` with `cx = 1`; `:929-932` `restartmus(0, 0, 45000)` (MUSIC0 at order 0, played at once); `:933` `jmp @@partsagain` | `MAIN/U2.ASM` |
| `fademusic` | steps the S3M **global-volume byte** at `module[0x30]` down by `cx` once per `dis_waitb` until it is ≤ 0, then stores 0 — **not the tempo** (`part-sequence.md` §7 says tempo; it is wrong). Both modules carry 0x40, so `cx = 1` is **64 frames** with the music playing | `U2.ASM:333-346`; port `audio::fade_music` (`audio.rs:246`), `Player::set_global_volume` |
| `@@partsdone` | `:936` `notextmode = 0`; `:937-939` fade (64 frames, last picture held); `:941-942` copper/sound off; `fatalexit` `:962-974`: real text mode 3, **20 × `copperwaitborder`** (20 frames of a blank screen), `printend` | `U2.ASM:935-994` |
| `printend` | copies `U2END.BIN` (80×25 CP437 cells + attributes) to `B800:0000`, cursor to row 14 col 0 (`dx = 0E00h`). Rows 0–13 are a single-line box: row 1 `S E C O N D   R E A L I T Y` … `Copyright (C) 1993 Future Crew`; rows 3–5 the distribution notice; rows 7–12 BBS/Fax/Mail/Internet. `fatalexit` also runs when the menu is left with ESC (`START.C:96-99` → `U2.ASM:719-720`) | `U2.ASM:506-535` |
| `looping` | `msg[6]` (`U2.ASM:745-746`), written by `START.C:103`; the menu offers 0 Disabled / 1 "Enabled (no end scroller shown)" (`MENU.C:229-238`, clamped `:167-168`); **2** only from `/gravis` (`START.C:89-93`: also card 3, menu skipped) = skip ENDSCRL, do not loop | `START.C`, `MENU.C` |
| The setup menu | three rows: Soundcard (0 none, 1 SB, 2 SB Pro, **3 GUS default**), Sound quality (0 Poor, 1 Standard, **2 High default**), Demo looping (**0 default**); Up/Down move and **clamp** (`:158-159`), Left/Right change and **wrap** (`:161-168`), Enter continues, ESC exits | `MENU.C:27-31, :91-109, :158-238` |
| The reference's sound | SB Pro (`_zoutputmode` 5) at High: `_zmixspeed` 20000 doubled to **40000** (`U2.ASM:737-759`) = 20,000 stereo frames/s — exactly the port's only mode (`MixRate::Hz40000`, stereo). Poor/Standard on SB Pro are uncaptured; GUS is not modelled | `music-clock.md` §5; `player.rs:113-163` |
| The tick schedule | `Player::poll` advances `acc` by `mix_rate × 1000` per 70.086 Hz poll and runs a tracker tick every `samples_per_tick = mix_rate × 5 / (2 × tempo)` (truncated; 833 at 40000/120). The mixer emits one frame per two samples in stereo. A second player at another rate would drift (tempo 120: 416 vs 1000 samples, 0.16 %) | `player.rs:455-500, :1117-1121`; `mix.rs:510-560` |
| The port's shell | `Sequence` (`shell.rs:822`): `plan`, `next` (only increments), `hold`, `music_after_tick` fades the music while holding (`:1227-1239`); restarts `MUSIC_RESTART_AT` = glenz `MUSIC1@0`, u2e `MUSIC0@18` (`:614`), `MusicStart::restart` waits 4 polls (`:367`); `CueWait` (`:959`). **No loop mode, no `looping`, no exit stage, no `/gravis`** (rejected as an unknown argument). With no argument the port runs the **test card** (`cli.rs:18`), not the demo | `shell.rs`, `cli.rs` |
| The port's window | SDL2 0.38; `window::run` (`window.rs:94-164`), the vault loop and `present_run` → `RunEnd {Close, Menu}` (`:168-325`); `vault_menu.rs` draws FONA text (A–X, a–z, 0–9, `! ? , . :`; Y/Z fall back to lowercase; `( ) + - * = '` are in the strip but unmapped); helpers are private; `MenuKey` has no Left/Right. Not resizable, no fullscreen, no persisted settings | `window.rs`, `vault_menu.rs` |
| Oracle cost | ~65 frames/s real time; an indexed read costs 20–25 frames, so indices closer than ~25 are batched and read afterwards (D19); 640×400 cannot take adjacent frames; largest index ever captured 23,000; **no full `SECOND 1` length is recorded** (≈47,000 to ENDSCRL by arithmetic, unmeasured); the oracle renders **no overscan region** (`dosbox-automation/src/hardware/video/vga_draw.cpp:1185`) | `reference-indices.md`, D14, D19 |
| Re-diff today | 26 filed captures (23 dirs; plzpart ×3, techno ×2), 109 frames + 4 in `check_a`, 19 with `.idx`; each part is compared by its own test at a mapping hard-coded in that test (e.g. MNTSCRL `boot = 419 + tick`, TUNNELI 3722, GLENZ K+5); **no single command**; TUNNELI and ENDSCRL have no pixel test; tests run the debug binary | `bins/secondreality/tests/*.rs` |
| Registers | A1–A52 contiguous; ~30 entries carry an open item, five owned by Phase 9 (A1, A5, A6, A7, A10); ~15 lack a labelled field; D24 leads missing on A19, A23, A25, A28–A31, A37. `bugs.md` §1–§9: §1–§4 name no pinning test, §3 has no frame, §1/§4 have stale text; ~80 "preserved" behaviours live only in 17 part notes. Decisions D8, D12, D13, D14, D16, D20, D31, D40 carry stale or open text; `zspeedadjust` (spec §13) is settled in `music-clock.md:700` but closed nowhere. Spec §8.3 (event stream) and §4.8's `audio render`/`event log` were never built and never re-deferred | the audit report's tables |

---

### Task 1: The loop seam and the two lead-in windows

**Files:**
- Create: `reference/parts/loop/`, `reference/parts/beglogo-leadin/`, `reference/parts/endlogo-leadin/` (frames + `manifest.json` each)
- Modify: `docs/fidelity/reference-indices.md` (new §25 "Phase 9 captures: the loop seam and the lead-ins")
- Test: `cargo xtask reference verify --tree reference`; `cargo test -p xtask --test verify`

**Interfaces:**
- Produces, for Tasks 4, 5 and 10: the three capture directories; §25's table of the loop seam — CRED's last frame `C`, the fade's frames, ALKU's first frame `L` on pass 2, and **Δ = L − 404** (pass 1's ALKU starts at the shell's first graphics frame, `FIRST_GRAPHICS_FRAME`) — and the lead-in windows' frame-by-frame index-plane content.

- [ ] **Step 1: Survey the loop seam.** `SECOND 5` with the menu set to SB Pro and looping enabled: `cargo xtask reference capture --arg 5 --boot autoexec --cycles 30000 --keys "KBD_left KBD_down KBD_down KBD_right KBD_enter" --frames <every 50 from 8700 to 10700> --out out/p9-loop-survey --sheet out/p9-loop-survey.png`. The keys are `MENU.C`'s: Left moves the soundcard row from GUS (3) to SB Pro (2), Down twice reaches the looping row, Right sets it to 1. Read the sheet: no 640×400 frame may appear (ENDSCRL skipped, `:914-915`), CRED's picture holds through the fade, then ALKU. If ENDSCRL appears, the keys did not set looping: stop and record what the sheet shows. Narrow `C` and `L` with a second survey at ≤ 10-frame spacing inside the bracketing 50-frame windows (runs closer than 25 frames are batched, D19).
- [ ] **Step 2: File `reference/parts/loop`.** `--frames` holding: `C`, two frames inside the fade, `L`, and ALKU's three held keyframes shifted by Δ (900+Δ, 1719+Δ, 2719+Δ — held per `tests/alku.rs`, so they reproduce byte for byte) — spaced per D19. Repeat the capture once into `out/p9-loop-repeat` and record that the held frames' SHA-256s are equal.
- [ ] **Step 3: File the lead-in windows.** `--arg 2 --frames 430,431,…,450` → `reference/parts/beglogo-leadin` (A5's 436–444) and `--arg 5 --frames 436,437,…,455` → `reference/parts/endlogo-leadin` (A7's 441–450); both 320×400 with `.idx`. If the batch is refused, split it into two runs and record the refusal.
- [ ] **Step 4: Record §25.** The commands verbatim, `C`, the fade, `L`, Δ, the repeat's equality, and for each lead-in index whether its index plane is the white field or the picture (counts of index 15, and the first index whose plane is not all one value). Measure only; port nothing.
- [ ] **Step 5: Re-pin the tree's counts, verify and commit.** Three tests pin the tree's size and must move with it, each to the number the new tree measures: `xtask/src/compare.rs:2127` and `:2143` (26 capture directories), `xtask/tests/verify.rs:213` (`44 capture(s), 148 file(s)`) and `xtask/tests/music.rs:287` (`44 capture(s)`). `cargo xtask reference verify --tree reference` reports three more captures, 0 findings, and `cargo test -p xtask` passes.

**Acceptance:** the three captures verify; §25's numbers are each reproducible by a recorded command; no existing capture is touched.

---

### Task 2: The full run and its music

**Files:**
- Create: `reference/parts/fullrun/` (frames + `manifest.json`), `reference/music/second1e/`, `reference/music/second1f/`
- Modify: `docs/fidelity/reference-indices.md` (new §26 "The full `SECOND 1` run"), `docs/fidelity/music-clock.md` (the two traces' windows)
- Test: `cargo xtask reference verify --tree reference`; `cargo test -p sr-audio --test trace` (`the_port_reproduces_every_filed_trace` walks every trace automatically)

**Interfaces:**
- Produces, for Task 6: §26's **boundary table** — for every part of block 1 in order, its first and last frame in the original's `SECOND 1` boot numbering, the PANICEND cue-wait exit, the U2E `MUSIC0@18` restart window, and ENDSCRL's first frame — each with the survey command that measured it; the `fullrun` capture; and the two traces covering block 1 from 8800 to the end, split at the U2E restart (a trace cannot follow STMIK's reload, `music-clock.md:36-47`).

- [ ] **Step 1: Survey the whole run.** `cargo xtask reference capture --arg 1 --boot autoexec --cycles 30000 --keys "KBD_left KBD_enter" --frames <every 100 from 400 to 50000> --out out/p9-fullrun-survey --sheet out/p9-fullrun-survey.png`. Expect ≈ 13–20 minutes of wall time (≈ 65 frames/s, slower in heavy parts); run it in the background. If ENDSCRL has not begun by 50,000, extend the survey; ENDSCRL never ends, so stop ~2,000 frames into it.
- [ ] **Step 2: Refine every boundary.** For each part change the survey brackets, take a ≤ 10-frame survey of that 100-frame window (batched runs; space the windows per D19), and record each part's first frame. Use the existing per-block references to name pictures (each part's filed keyframes are what its screens look like).
- [ ] **Step 3: File `reference/parts/fullrun`.** At most 60 frames: every part's first frame, at least one **held** frame per part where the part has one (held frames reproduce byte for byte across sessions; for a part with none, its least-moving screen), the frames on either side of the cue-wait exit and of the U2E restart, and three ENDSCRL frames ≥ 3 apart (640×400 cannot take adjacent frames). Repeat the held frames once into `out/p9-fullrun-repeat` and record their equality.
- [ ] **Step 4: Trace the music.** `cargo xtask music trace --arg 1 --cycles 30000 --frames 8800..<restart − 50> --out reference/music/second1e` and `--frames <restart + 50>..<ENDSCRL start + 2000> --out reference/music/second1f`, where *restart* is §26's U2E restart frame. If a trace refuses (the fixed-base read), narrow its end and record the refusal, as `second3d` did.
- [ ] **Step 5: Record and commit.** §26: the boundary table with commands, the run's total length to ENDSCRL, and the repeat's equality. Re-pin the same four count assertions Task 1 moved (one more capture directory, two more traces). `cargo xtask reference verify --tree reference`, `cargo test -p xtask` and `cargo test -p sr-audio --test trace` pass with the two new traces included.

**Acceptance:** the original's full run is measured part by part with reproducible commands; the new traces pass the player's trace test or the test's report names exactly which rows disagree and the task records that as a finding for Task 6.

---

### Task 3: The demo's own run, and how it ends

**Files:**
- Create: `bins/secondreality/src/endscreen.rs`
- Modify: `bins/secondreality/src/cli.rs` (the default run), `bins/secondreality/src/shell.rs` (`RunSettings`, `Ending`, the exit stage, `Sequence::ended`), `bins/secondreality/src/window.rs` (close when ended; ESC during the stage), `bins/secondreality/src/main.rs` (print the end screen; usage text), `bins/secondreality/src/headless.rs` (an ended run keeps composing its last frame), `CLAUDE.md` ("Running it")
- Test: unit tests in `cli.rs`, `shell.rs`, `endscreen.rs`, `window.rs`; `bins/secondreality/tests/shell.rs`

**Interfaces:**
- Produces, used by Tasks 4, 6, 7 and 8:
  - `pub enum Ending { #[default] Hold, Exit }` — `Exit` for a run built from a block (`--block`, or no argument); `Hold` for `--part`, the vault and the shell's own test constructors.
  - `#[derive(Clone, Copy, Debug, Default)] pub struct RunSettings { pub ending: Ending }` passed to `Sequence::with_run`; Task 4 adds `looping: Looping` and Task 7 adds `player: sr_audio::PlayerOptions`. `Cli::run_settings(&self) -> RunSettings`.
  - `pub const EXIT_FADE_TICKS: u32 = 64;` (`U2.ASM:333-346, :937-939`: 0x40 ÷ `cx` 1) and `pub const EXIT_BLANK_TICKS: u32 = 20;` (`:969-973`).
  - `Sequence::ended(&self) -> bool` — true once the exit stage has run. Distinct from `finished()` (a part that could not start), which keeps its meaning.
  - `endscreen::render(screen: &[u8], ansi: bool) -> String` — rows `0..CURSOR_ROW` of an 80×25 cell image, CP437 mapped to Unicode, each row's trailing blanks trimmed; with `ansi`, each attribute change becomes an SGR sequence (low nibble foreground, bits 4–6 background, bit 7 ignored) and every line ends with `ESC[0m`. `pub const CURSOR_ROW: usize = 14;` `pub const END_SCREEN: &str = "assets/MAIN/DATA/U2END.BIN";`

- [ ] **Step 1: Write the failing tests.**
  - `cli.rs`: `a_run_with_no_arguments_is_the_demo` — `Cli::parse([])` gives `block == Some('1')`; `the_test_card_is_still_a_part` — `--part testcard` parses; `a_part_run_holds_and_a_block_run_exits` — `run_settings().ending` is `Hold` for `--part glenz`, `Exit` for `--block 3` and for no argument.
  - `shell.rs` (stub parts): `a_demo_run_fades_then_blanks_then_ends` — after the plan's last part retires: ticks 1..=64 compose the last part's picture and the player's global volume steps 0x40 → 0; ticks 65..=84 compose a frame whose every pixel is (0, 0, 0); `ended()` is false through tick 83 and true from tick 84. `a_part_run_still_holds_after_its_end` — an `Ending::Hold` run 1,000 ticks past its plan has `ended() == false` and composes the held picture.
  - `endscreen.rs`: `the_end_screen_is_the_shipped_box` — `render(U2END.BIN, false)` has 14 lines; line 0 starts `┌` and ends `┐`; line 1 contains `S E C O N D   R E A L I T Y` and `Copyright (C) 1993 Future Crew`; line 13 starts `└`. `the_end_screen_colours_are_its_attributes` — with `ansi`, line 0's first cell carries the SGR for attribute 0x08 (bright black on black).
  - `window.rs` (pure predicates): `an_ended_run_closes_the_window` and `escape_during_the_exit_stage_closes_at_once` (the stage cannot be skipped in the original; closing on a deliberate ESC is the port's courtesy, recorded in Step 5).
- [ ] **Step 2: Run them; they fail.** `cargo test -p secondreality --bin secondreality`.
- [ ] **Step 3: Implement.** Default run = `--block 1` (`SECOND` with no argument is block 1, `part-sequence.md` §4); rewrite `DEFAULT_PART`'s comment and the usage text. The exit stage reuses the existing hold and its fade; the blank is a frame of index 0 with a palette whose every entry is black. The window closes when `ended()` turns true. `main` prints `endscreen::render` to stdout once, after the window closes, for every **windowed demo run** however it closed (the original's `fatalexit` prints it on every exit) — never for `--part`, `--vault` or headless — with `ansi` when `std::io::IsTerminal` says stdout is a terminal.
- [ ] **Step 4: Run the suite.** Fix any test that relied on the no-argument test card or on a block run holding forever (search `tests/` and the unit tests for `Cli::default`, `part_name()` and holds after ENDSCRL); each fix keeps the test's intent and names the new rule.
- [ ] **Step 5: Record and commit.** `CLAUDE.md`'s "Running it": no argument plays the demo; leaving ENDSCRL is **one** ESC (then the fade, the blank, the close); the end screen. Write the decision text for Task 11's D41 into the commit message (the exit stage, ESC-during-stage, the terminal end screen as the text mode's stand-in).

**Acceptance:** a windowed run with no arguments plays block 1 and, after ENDSCRL's ESC, fades for 64 ticks, blanks for 20, closes and prints the box; `--part` and vault runs behave exactly as before.

---

### Task 4: Loop mode

**Files:**
- Modify: `bins/secondreality/src/shell.rs` (`Looping`, the plan rule, the loop pass), `bins/secondreality/src/cli.rs` (`--loop`, `/gravis`, ignored `/` switches), `bins/secondreality/src/window.rs` (the title's pass number), `docs/fidelity/part-sequence.md` (§7's `fademusic` correction and the exit transcription as a new §9)
- Create: `bins/secondreality/tests/loop.rs`
- Test: unit tests in `shell.rs` and `cli.rs`; `cargo test -p secondreality --test loop`

**Interfaces:**
- Consumes: Task 3's `RunSettings`, `Ending`, the exit stage; Task 1's `reference/parts/loop` and §25's Δ.
- Produces: `pub enum Looping { #[default] Off, On, NoEndScroller }` (the original's `looping` 0 / 1 / 2) as `RunSettings::looping` and `Cli::looping: Looping` (`--loop` sets `On`, `/gravis` sets `NoEndScroller` and also `Cli::setup = false`, the field Task 8 reads); `pub const LOOP_RESTART: (ModuleId, u8) = (ModuleId::Music0, 0);` (`U2.ASM:929-932`); `Sequence::pass(&self) -> u32` (1 on the first pass).

- [ ] **Step 1: Write the failing tests.**
  - `a_looping_plan_has_no_end_scroller` — for blocks `1` and `5` with `On` and with `NoEndScroller`, the plan is `block_plan` without `endscrl`; with `Off` it is unchanged.
  - `a_loop_pass_fades_then_restarts_music0_at_order_0_and_starts_alku` — after CRED retires: `EXIT_FADE_TICKS` ticks holding CRED's picture with the global volume stepping down, then `music_module() == Some(LOOP_RESTART)`, `part() == "alku"`, `pass() == 2`.
  - `a_loop_pass_runs_every_block_whatever_the_entry` — a run begun at `--block 3` with `On` has, on pass 2, the plan `block_plan('1')` minus `endscrl`; `ddstars` never appears.
  - `a_loop_pass_fires_only_the_full_runs_restarts` — on pass 2, `MUSIC1@0` installs at `glenz` and `MUSIC0@18` at `u2e`; `MUSIC1@42` and `MUSIC0@25` never install.
  - `gravis_runs_once_without_the_end_scroller_and_ends` — `NoEndScroller`: after CRED, the exit stage, then `ended()`.
  - `cli.rs`: `the_loop_flag_is_looping_on`; `gravis_is_looping_two`; `unknown_slash_switches_are_ignored` (`U2.ASM:605-606`; `/GRAVIS` matches case-insensitively by its first seven characters, `START.C:89`).
  - `tests/loop.rs`: `the_loop_seam_is_the_originals` — `--headless --block 5 --loop --dump-frames <reference/parts/loop's indices>` compared exactly with `reference/parts/loop`; the expected counts are 0 for every held frame.
- [ ] **Step 2: Run them; they fail.**
- [ ] **Step 3: Implement the loop pass.** At plan exhaustion with `On`: run the fade stage, then `install_music(Music0, 0, …, MusicStart::restart(index))`, reset `plan` to `block_plan('1')` minus `endscrl` and `next` to 0, start ALKU. The restart sites then fire through `MUSIC_RESTART_AT` as on any block-1 run; confirm `restart_music`'s "already playing" check cannot suppress pass 2's sites. If Task 1's seam shows frames between the fade's end and ALKU's first frame that the port does not produce, measure them as **one constant** from §25 (`LOOP_RELOAD_TICKS`, the STARTMUS load and ALKU's load) and adopt it only if ALKU's three held keyframes then compare at 0 (D24); otherwise keep the port's seam and register the difference for Task 10.
- [ ] **Step 4: The exits and the title.** ESC still ends only the running part, so a looping run is left by closing the window (the original's way out is its sticky Ctrl, `:920`; the port's Ctrl is D21's one-part skip). The end screen prints when it closes. The title bar shows `pass N` from pass 2 on.
- [ ] **Step 5: Correct `part-sequence.md`.** §7: `fademusic` fades the global-volume byte (`module[0x30]`), 64 frames at `cx = 1`. New §9: `@@partsdone` and `fatalexit` (`U2.ASM:935-994`) transcribed, and what the port does at each line (Task 3's stage, this task's loop).
- [ ] **Step 6: Run the suite and commit.**

**Acceptance:** `--loop` plays the demo forever with the original's seam, compared exactly against the original at the loop seam's held frames; `/gravis` plays once without the end scroller; every existing block test is unchanged.

---

### Task 5: `cargo xtask rediff` — every part, one command

**Files:**
- Create: `xtask/src/rediff.rs`, `xtask/tests/rediff.rs`
- Modify: `xtask/src/main.rs` (the subcommand), `xtask/src/compare.rs` (D16's deferred `..` guard bypass on `--out`)
- Test: `cargo test -p xtask --test rediff`, `cargo test -p xtask --lib compare`

**Interfaces:**
- Consumes: `compare::compare_indexed`, `compare::compare_rgb`, `FrameDiff`; the binary's `--headless --dump-dir D --dump-frames …`; every filed capture under `reference/parts`, including Task 1's three.
- Produces, for Tasks 6 and 11:
  - `cargo xtask rediff [--binary PATH] [--only NAME]… [--out DIR]` — without `--binary` it runs `cargo build --release -p secondreality` and uses that binary; default `--out out/rediff`, refused inside `reference/`.
  - `pub const TABLE: &[Entry]` with `pub struct Entry { pub capture: &'static str /* path under reference/parts */, pub args: &'static [&'static str] /* e.g. ["--part", "mntscrl"] */, pub frames: &'static [Frame] }` and `pub struct Frame { pub reference: u32, pub rendered: u32, pub plane: Plane, pub registered: u32, pub register: &'static str /* "exact", or "A41", … */ }`, `pub enum Plane { Indexed, Rgb }`.
  - The report: one line per frame — capture, reference index, rendered index, plane, differing pixels, registered, `ok`/`DIFFERS` — and `<out>/report.json`. **Exit 0** when every frame's differing count equals its registered count, **1** when any does not, **2** when it cannot compare (a missing frame, a geometry mismatch, a capture with neither an entry nor a `PENDING` line, an entry with no capture, a failed run).
  - `pub const PENDING: &[&str] = &["fullrun"];` — filed captures whose mapping a later task measures. `rediff` names each one in its report as not compared; Task 6 empties it.

- [ ] **Step 1: Write the failing tests.** `the_table_names_every_filed_capture_once` — walk every `manifest.json` under `reference/parts` and assert the set of capture paths equals the union of the `TABLE` captures and `PENDING`, with no duplicates and no overlap, and that every filed frame index appears in its entry. `a_frame_the_dump_lacks_is_exit_2` and `a_count_off_the_register_is_exit_1` — with a fixture capture and a fake binary script under `xtask/tests/fixtures`. `an_out_through_dot_dot_into_the_reference_is_refused` (compare and rediff both).
- [ ] **Step 2: Run them; they fail.**
- [ ] **Step 3: Build the table from the tests, not from memory.** For each capture, copy the run arguments, the reference→rendered mapping and the asserted count from the part's own test (`bins/secondreality/tests/<part>.rs`, or the unit test that holds the number): e.g. MNTSCRL `rendered = reference − 419`, TUNNELI `− 3722`, GLENZ's moving keyframes at the test's K+5/K+4 ticks, U2E at the test's own index rule, the vault parts at `K0` 353/3/356. Where a test asserts bytes or a band rather than pixels (CRED's bands, PANICEND's standalone 8792/8800, ENDSCRL and TUNNELI which have no pixel test), record the pixel count this command measures and cite the A-entry whose figure it reproduces (ENDSCRL's A11 percentages at `--block 5` with the chain's 82, TUNNELI's A6 179/373/321/433). The pairing is explicit — no ±1 search — because every test already states its index.
- [ ] **Step 4: Implement** the runner (one headless run per entry, all its rendered indices in one `--dump-frames`), the comparison (indexed when the frame is `Plane::Indexed`), the report and the exit codes.
- [ ] **Step 5: Run it.** `cargo xtask rediff` exits 0. Record its wall time and its table in the commit message.
- [ ] **Step 6: Commit.**

**Acceptance:** one command re-renders and re-compares every filed reference frame in release mode and agrees, frame for frame, with every registered count; a capture or frame that is not compared fails the run.

---

### Task 6: The full sequence against the original

**Files:**
- Modify: `bins/secondreality/src/headless.rs` and `cli.rs` (`--positions FILE`), `bins/secondreality/src/shell.rs` (`Sequence::starts`), `xtask/src/rediff.rs` (the `fullrun` entry), `docs/fidelity/reference-indices.md` §26 (the port's column), `docs/fidelity/APPROXIMATIONS.md` (new entry for the seams, if Step 4 registers), `docs/fidelity/music-clock.md`
- Create: `bins/secondreality/tests/fullrun.rs`
- Test: `cargo test -p secondreality --test fullrun` (release only: `#[ignore]`d in debug with the reason, run by `cargo test --release -p secondreality --test fullrun -- --ignored`); `cargo xtask rediff --only fullrun`

**Interfaces:**
- Consumes: Task 2's boundary table, `fullrun` capture and `second1e`/`second1f` traces; Task 5's `TABLE`.
- Produces: `Sequence::starts(&self) -> &[(u32, &'static str)]` (the index at which each part started, in order, across passes); `--positions FILE` (headless only) writing one CSV line per tick, `index,pass,part,order,row`; §26's per-part **seam table** (the original's first frame, the port's, the difference, and its change across each seam).

- [ ] **Step 1: Write the failing tests.** In `tests/fullrun.rs`: `the_full_run_plays_every_part_in_the_originals_order` — `--headless --block 1 --positions …` to ENDSCRL's start + 2,000: the parts in `starts()` order are block 1's plan; `the_full_runs_music_is_where_the_trace_holds_it` — at every row of `second1e` and `second1f`, the port's `(order, row)` agrees within the existing `JITTER_FRAMES = 3` window (`shell.rs:2322`), with the agreement percentage reported and asserted against the figure Step 3 records.
- [ ] **Step 2: Run them; they fail.**
- [ ] **Step 3: Measure the seams.** Implement `starts()` and `--positions`, run the full run, and fill §26's port column. The music timeline check is the proof of the restarts and the cue wait in context: GLENZ's `MUSIC1@0`, the post-PANICEND wait (A38 already registers its 249-frame zoomer start), and U2E's `MUSIC0@18`.
- [ ] **Step 4: Decide the loader (D31).** The port hands a part over in one tick; the original spends the loader's frames. From the seam table: if **one constant** per seam explains every music-independent seam and modelling it leaves every filed per-part comparison unchanged, adopt it as `LOADER_TICKS` held between parts in demo runs (D24's one-constant rule) and re-run `cargo xtask rediff`; otherwise register the measured per-seam table as a new APPROXIMATIONS entry and leave the handover as it is. Do not fit per-seam constants.
- [ ] **Step 5: Compare the pictures.** Add the `fullrun` entry to `TABLE` and remove it from `PENDING` (which is then empty): each filed frame's rendered index is its reference index plus **that part's start difference** from the seam table (measured at the part's first frame, never chosen to minimise a keyframe's diff). Held frames must compare at 0; a moving frame carries its part's registered residual or a new registered count with its A-entry.
- [ ] **Step 6: Record and commit.** `cargo xtask rediff --only fullrun` exits 0.

**Acceptance:** the port's block-1 run is shown, part by part and row by row, to be the original's full run within the registered differences, and the loader question D31 left to this phase is decided from measurement.

---

### Task 7: The enhanced sound option

**Files:**
- Modify: `crates/sr-audio/src/player.rs` (`Quality`, `PlayerOptions::quality` replacing the unread `interpolation`), `crates/sr-audio/src/mix.rs` (the enhanced frame), `bins/secondreality/src/audio.rs` (`load_player` takes `PlayerOptions`), `bins/secondreality/src/shell.rs` (the run's options reach every install, including restarts), `bins/secondreality/src/cli.rs` (`--sound-quality authentic|enhanced`), `docs/fidelity/music.md` (a labelled "Enhanced quality" section)
- Create: `crates/sr-audio/tests/enhanced.rs`
- Test: `cargo test -p sr-audio`; unit tests in `shell.rs`, `audio.rs`, `cli.rs`

**Interfaces:**
- Produces, for Task 8: `pub enum Quality { #[default] Authentic, Enhanced }`; `PlayerOptions { mix_rate, stereo, quality }`; `PlayerOptions::output_rate(&self) -> u32` — frames per second the device and the WAV dump are opened at: 20,000 for Authentic stereo at `Hz40000`, **48,000** for Enhanced; `RunSettings::player: PlayerOptions`; `Cli::quality: Quality` (`--sound-quality authentic|enhanced`, default `authentic`); `MusicSource` becomes `fn(ModuleId, PlayerOptions) -> Result<Shared, String>`.

- [ ] **Step 1: Write the failing tests.** In `crates/sr-audio/tests/enhanced.rs`, on both shipped modules (MUSIC0 with its tempo patch): `enhanced_keeps_the_authentic_tick_schedule` — two players, one per quality, polled 30,000 times: `state()` and `engine_position()` equal at every poll. `enhanced_emits_twelve_frames_for_every_five` — after any number of polls the enhanced player's total frames are `floor(authentic_frames × 12 / 5)`. `enhanced_pitch_is_the_authentic_pitch` — a sustained note's zero-crossing frequency over one second agrees within 0.5 %. `enhanced_level_is_the_authentic_level` — RMS over ten seconds from order 0 agrees within 1 dB. `the_default_quality_is_authentic` replaces `the_interpolation_setting_is_the_objects_own_and_off`. In the binary: `a_restart_keeps_the_runs_sound_quality` (after the u2e restart the new player's `options().quality` is `Enhanced`) and `the_audio_dump_header_is_the_output_rate` (48,000 Hz, 2 channels for Enhanced).
- [ ] **Step 2: Run them; they fail.**
- [ ] **Step 3: Implement the enhanced frame.** The tick schedule and every voice's position advance stay exactly the authentic ones — `poll`, `samples_per_tick` and `advance` are untouched. For each authentic frame the mixer emits 2 or 3 output frames by an exact 12/5 accumulator; output frame *j* of *n* evaluates each voice at its position plus `j/n` of its step, interpolating linearly between the two sample bytes (the second is the loop start for a looped voice at its end, and the last byte for a one-shot), weights it by `(s − 128) × scale(m_vol)` in `i32` without the `>> 8`, and converts the side's sum with the authentic curve's small-signal gain around its centre (`Curve::sample` at `0x400 ± 1`), clamped to `i16`. The ring grows to hold 12/5 as many samples.
- [ ] **Step 4: Carry the options.** `Sequence` keeps the run's `PlayerOptions` and passes them to every `MusicSource` call; `audio_format()` reads `output_rate()`.
- [ ] **Step 5: Record and commit.** `music.md`'s section says what the enhanced path is, that it is off by default, that the clock is untouched (the first test), and what it does not reproduce (the curve's clamp and A14's ends).

**Acceptance:** the enhanced option plays the same music at the same pitch, level and timing, at 48 kHz without the authentic path's stepping, and changes nothing about any run's clock or pictures.

---

### Task 8: The settings screen

**Files:**
- Create: `bins/secondreality/src/settings.rs` (state, reducer, draw), `bins/secondreality/src/menu_text.rs` (the FONA text layer moved out of `vault_menu.rs`, with the seven unmapped glyphs `( ) + - * = '` added to the table)
- Modify: `bins/secondreality/src/vault_menu.rs` (use `menu_text`), `bins/secondreality/src/window.rs` (the settings loop before a windowed demo run), `bins/secondreality/src/cli.rs` (`--no-setup`), `bins/secondreality/src/main.rs` (usage)
- Test: unit tests in `settings.rs`, `menu_text.rs`, `window.rs`, `cli.rs`; the vault menu's existing tests unchanged

**Interfaces:**
- Consumes: Task 3's `RunSettings`/`Ending`, Task 4's `Looping`, Task 7's `Quality`; `PresentOptions` (`present.rs:30-36`).
- Produces: `pub struct Settings { pub sound: bool, pub quality: Quality, pub looping: bool, pub aspect: Aspect, pub border: bool }`, `Settings::from_cli(&Cli) -> Settings`, `Settings::apply(&self, cli: &mut Cli)` (writes `no_audio`, `quality`, `looping` — `On` or `Off`, leaving a `/gravis` `NoEndScroller` alone since that run never shows the screen — `aspect`, `border`); `Cli::setup: bool` (default true; `--no-setup` and `/gravis` clear it); `pub enum SettingsKey { Up, Down, Left, Right, Enter, Escape, Other }`; `pub enum SettingsAction { Redraw, Start, Exit }`; `pub fn draw(settings: &Settings, selected: usize, frame: &mut IndexedFrame, font: &FontStrip)`.

The rows, in the original's order and then the port's: **Sound** On / Off (the original's soundcard row; Off is `--no-audio` — the player still runs); **Sound quality** Authentic / Enhanced; **Demo looping** Disabled / Enabled; **Aspect** 4:3 / Square; **Border** On / Off. Title `SECOND REALITY`, subtitle `SETUP`, a `>` marker, footer `UP DOWN  LEFT RIGHT  ENTER  ESC`. Up/Down move and **clamp** (`MENU.C:158-159`); Left/Right change the value and **wrap** (`:161-168`); Enter starts the run; ESC exits without playing — and, like `fatalexit`, the end screen is printed. No persistence: the original asked every time; command-line flags set the initial values.

- [ ] **Step 1: Write the failing tests.** `the_selection_clamps_at_both_ends`, `left_and_right_wrap_each_value`, `enter_starts_and_escape_exits`, `the_settings_frame_draws_five_rows_and_the_marker`, `the_chosen_settings_are_the_runs_settings` (each row's value reaches `RunSettings`, `PresentOptions` and `Cli::no_audio`), `the_screen_shows_only_for_a_windowed_demo_run` (not for `--part`, `--vault`, `--no-setup`, `/gravis` or headless), `changing_the_aspect_resizes_the_window` (the pure `output_size` before and after).
- [ ] **Step 2: Run them; they fail.**
- [ ] **Step 3: Implement** the module, the shared text layer, and the window loop (the vault's pattern: draw, wait in bounded `wait_event_timeout`s, re-present on `Exposed`, re-present with new `PresentOptions` when Aspect or Border change).
- [ ] **Step 4: Check by hand.** Open the window, cycle every row, change the aspect and border, start a run with sound off, start one looping, leave with ESC; record what was checked in the commit.
- [ ] **Step 5: Commit.**

**Acceptance:** a windowed demo run opens on a five-row settings screen drawn in the demo's own font whose choices reach the run; every other run starts as before.

---

### Task 9: `bugs.md`, complete

**Files:**
- Modify: `docs/fidelity/bugs.md`; the part notes that gain a back-reference; test files only where a visible bug has no pinning test (e.g. `bins/secondreality/src/parts/minvball.rs`)
- Test: the new pinning tests; `cargo test --workspace`

**Interfaces:** none produced; Task 10 cross-references the new sections.

- [ ] **Step 1: Apply one filter to every preserved behaviour.** Walk every `docs/fidelity/*.md` section that calls a behaviour *preserved*, *kept*, *required* or *D7* (the audit lists ~80 across 17 notes), plus A14's curve builder, A15's band and A19's three. A behaviour gets a **full entry** if a reference frame shows it or "fixing" it would change a drawn pixel, a palette value, a sample or a timing; otherwise it gets a row in a new closing **index table** (part, behaviour, source line, why no frame can show it, the note that holds it).
- [ ] **Step 2: Bring every full entry to the file's own standard.** Original, where (file:line), what the port must do, **a reference frame that shows it** (capture and index) or "no frame can show it" with the reason, and **a pinning test**. Write a test where none exists for a visible bug: at least `shadows_are_drawn_over_balls_without_a_depth_test` for §2 (MINVBALL's painter order).
- [ ] **Step 3: Fix the stale sections.** §1's "What remains for Phase 5" (the mechanism is the accumulator's half, `plzpart.md:119-122`); §3 names `sync_reads_the_table_the_object_reads` and `sync_answers_the_position_before_the_table_with_its_own_word` and points at A15; §4's heading, and its pinning tests `the_fade_upload_is_the_originals_508_bytes_and_stops_inside_entry_169` and `the_palette_upload_is_two_bytes_a_colour_and_truncates_at_512`.
- [ ] **Step 4: Prove completeness.** Commit with the classification of every audited behaviour (full entry or index row) and the `grep` that finds no preserved behaviour in a part note missing from `bugs.md`.

**Acceptance:** spec §5.1's rule holds for every visible preserved bug (an entry, a frame or a stated reason, a test), and every other preserved behaviour is indexed.

---

### Task 10: `APPROXIMATIONS.md`, the decisions and the spec, complete

**Files:**
- Modify: `docs/fidelity/APPROXIMATIONS.md`, `docs/decisions.md`, `docs/spec/2026-10-10-second-reality-rust-design.md` (dated amendments only), the part notes an entry's status line cites; `crates/sr-engine/src/present.rs` only if Step 3 adopts a derived border
- Test: `cargo test --workspace`

**Interfaces:** consumes Tasks 1, 2, 4, 5 and 6's measurements; produces the register Task 11's close-out cites.

- [ ] **Step 1: Repair the fields.** Every `## A<n>` entry carries the labels **Part**, **Original**, **Port**, **Why** and **Measured difference** (the audit's list: A6, A7, A10, A11, A13, A14, A17–A20, A22–A27, A45), moving existing text under the labels without changing its claims; add D24's lead (the packed EXE and its `MAIN/PACKFINA.INC` line) to A19, A23, A25, A28–A31 and A37.
- [ ] **Step 2: Give every open item a status.** Each entry that names an open item gets one line: `**Status (Phase 9):** Closed — <the measurement and its command>` or `Carried — <why it cannot be closed: the packed EXE (D12), what the oracle cannot see, an unreachable path>`. Close A5 and A7 from Task 1's lead-in windows; restate A6, A10 and A11 at the full run's and `rediff`'s numbers.
- [ ] **Step 3: Close A1 by derivation.** The oracle renders no overscan region (`vga_draw.cpp:1185`), so derive the border from the CRTC timing the parts' modes program — mode 13h's horizontal display end, blank start/end and total, and the vertical ones, from the oracle's own `src/ints/int10_modes.cpp` table and the tweaks in `MAIN/VMODE.ASM` and the parts' `inittwk` — as per-side widths in source pixels for each geometry. Adopt them in `present.rs`'s `BORDER_PX` (with a test) only if they are whole source pixels at every geometry; otherwise keep 8 and record the derivation.
- [ ] **Step 4: Add the missing entries.** The `_np_zinfo` substitute (`dis_muscode` returns 0; spec §7.4's escalation clause); DDSTARS' assembled-out star clip (D24, 0 bytes); Ctrl's one-part skip (D21) and ESC's carry across a part boundary in place of `flushkbd` (`U2.ASM:211`); the end screen printed to the terminal instead of text mode; the enhanced option (off by default; the clock untouched); and Task 4's loop seam or Task 6's seams if those tasks registered rather than adopted.
- [ ] **Step 5: Record the spec §5.2 outcomes.** A section "Generated and self-modifying code" with one row per spec candidate — outcome and evidence (bit-identical with its test; not applicable; registered) — noting that the spec's TUNNELI "patched y-base" is a colour immediate and PLZPART's "jump table" is four patched table bases.
- [ ] **Step 6: Close the decisions.** A dated closing paragraph on D8 (substitute adopted; the new entry), D12 (unresolved by design: the shipped EXEs are packed and the frames arbitrate), D13 (its "Not resolved" is answered by D26 and D8's amendment), D14's left-open probes, D16 item 6 (A1's outcome) and its deferred minors (the `..` guard fixed in Task 5), D20's three items, D31 (Task 6's loader decision) and D40 (settings and loop mode now D41–D44); a new decision closing `zspeedadjust` (both read 0, and `U2.ASM:384-389` writes `_zspeedadjust2` only for output modes 7 and 2); and a new decision retiring spec §8.3's event stream and §4.8's `audio render`/`event log` in favour of what was built (frame comparison, `music trace`, `--audio-dump`, `--positions`), with its cost if wrong.
- [ ] **Step 7: Amend the spec** with dated notes in its own style: §13 closed; §7.4's outcome; §4.1's layout as built (parts in the binary) with its decision; §4.8 as built; §5.2's two wording errors.
- [ ] **Step 8: Prove completeness and commit.** Commit with the check that every `## A` heading has the five labels and every open-item phrase has a status line.

**Acceptance:** every knowing difference is registered with a measured difference or a stated reason it cannot be measured, every open item has a status, and no decision or spec section describes a state the project has left.

---

### Task 11: The re-diff of record and the close-out

**Files:**
- Create: `README.md` (user-facing)
- Modify: `PROGRESS.md`, `CLAUDE.md`, `docs/decisions.md` (D41–D44 finalized)
- Test: the full gate

**Interfaces:** consumes everything above.

- [ ] **Step 1: Re-diff every part at the tip.** `cargo xtask rediff` exits 0; its table is the phase's "every part re-diffed" evidence. Then `cargo test --workspace`, both clippy commands, `cargo xtask reference verify --tree reference`, `cargo xtask assets verify --tree assets --decode`, and `cargo test --release -p secondreality --test fullrun -- --ignored`. Record each result line.
- [ ] **Step 2: Write `README.md`.** For a person who wants to watch the demo: build and run, the settings screen, the keys (ESC, Ctrl, the close button), `--block 1`–`5` and `U` (the hidden part `README.1ST` never mentions, `part-sequence.md` §4), `--loop`, `/gravis`, the vault, the enhanced option, the end screen; the licence; where the fidelity record lives.
- [ ] **Step 3: Update `CLAUDE.md`** ("Running it", the vault paragraph's ESC wording if it changed, `cargo xtask rediff` under Verification) **and finalize D41** (the demo's own run and its exit), **D42** (loop mode), **D43** (the enhanced option), **D44** (the settings screen).
- [ ] **Step 4: Update `PROGRESS.md`.** Phase 9's row (done; evidence; tag withheld until the final review is clean), the evidence block (the re-diff table, the full-run seam table, the loop seam, the suite counts), and a "where to pick up" that says the planned work is complete and lists what the registers carry.
- [ ] **Step 5: Commit.** The controller runs the final whole-branch review; the `phase-9` tag is applied once it is clean.

**Acceptance:** spec §10's Phase 9 exit is met and evidenced — the full sequence with music restarts, cue waits and loop mode; the settings screen; both registers complete; every part re-diffed.

---

## Self-review notes (for the controller, not the implementers)

- **Spec coverage.** §10 Phase 9: full sequence → Tasks 3, 4, 6; music restarts and cue waits in context → Task 6's trace check over Task 2's traces; loop mode → Task 4 against Task 1's capture; settings screen → Task 8 (with §4.2's aspect and border, §7.2's quality via Task 7); registers complete → Tasks 9 and 10; every part re-diffed → Task 5's walker run in Task 11. §1 success criterion 1 (start to finish, hidden part, loop) → Tasks 3, 4, 6. §8.3 and §4.8's unbuilt tools are decided in Task 10, not silently dropped.
- **Rulings recorded here for review.** (1) No argument plays the demo through the settings screen, as `SECOND` does; `--part testcard` keeps the card. (2) A demo run ends as the original does — one ESC on ENDSCRL, the 64-tick fade, 20 blank ticks, the window closes and the end box is printed to the terminal; ESC during that stage closes at once. (3) Ctrl stays D21's one-part skip; a looping run is left with the close button. (4) The settings screen offers Sound, Quality, Looping, Aspect and Border, does not persist, and does not offer GUS or the SB rates the reference never captured. (5) The enhanced option is built because §7.2 asks for it, but it is the most optional task here: Task 8 can ship with the Quality row removed if Task 7 is dropped. (6) "Phase 9 compares every frame" in A5/A6/A7/A10 is discharged by the named windows, the full-run capture and the re-diff, not by capturing every frame (§8.2 is keyframes; dense capture costs ~0.3 s a frame and session phase makes it ambiguous).
- **Ordering.** Tasks 1–2 are measurement and gate Tasks 4 and 6. Task 3 precedes Task 4 (the exit stage is the loop's fade). Task 5 needs Tasks 3–4's behaviour settled so its mappings are final, and Task 6 extends it. Tasks 7 → 8 because the settings screen consumes `Quality`. Tasks 9–10 consume every measurement; Task 11 runs the re-diff of record last.
- **Known coupling.** `RunSettings` grows across Tasks 3, 4 and 7 (fields named in Task 3's Interfaces); `MusicSource`'s signature changes in Task 7 and every caller moves with it; `vault_menu.rs`'s text layer moves in Task 8 and its tests must pass unchanged.
