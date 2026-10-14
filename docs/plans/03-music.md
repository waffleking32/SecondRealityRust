# Phase 3 — Music engine · Implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to
> implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The demo plays its own music. Both modules load, descramble and play end-to-end through a
Scream-Tracker-3-semantics player whose `(order, row)` position is **the same position the original's
player is in at the same reference frame**, so a part that waits on a music cue waits the same number
of frames the original waited — and ENDLOGO, this phase's proof, is still frame-diff-identical with
its measured stand-in (**A7**) replaced by the real cue.

**Architecture:** A new dependency-free crate `crates/sr-audio` holds the module model, the player
and the mixer — the same discipline `sr-engine` is held to, because the player is a deterministic
function of tick counts, never of the audio device or the wall clock. `sr-engine` gains one seam: a
`Music` trait it polls once per tick at the point the original's copper polls `_zpollme`
(`MAIN/COPPER.ASM:449-466`) and exposes to parts through `PartCtx`. The binary owns the only
non-deterministic thing in the phase — the SDL audio device — and drains a ring buffer.

**Tech stack:** Rust 2021, `rstest` for table tests, `serde`/`serde_json` for the trace manifests,
SDL2 (already a dependency of the binary) for audio output. **No new third-party crate**: the S3M
player is written from the sources and the ST3 semantics the original's own player implements.

**Spec:** `docs/spec/2026-10-10-second-reality-rust-design.md` — §2.5, §6.3 (the `dis_getmframe`
rate, this phase's deliverable), §6.4, §7 in full (7.1 descrambling, 7.2 player and mixing, 7.3 the
tempo patch, 7.4 the sync API and `_np_zinfo`), §8.3 (event comparison — the technique this phase
uses as its verdict), §10 (the exit criteria), §13 (open question 1, `zspeedadjust`).
Decisions: **D5** (the keystream, verified), **D8** (the `zinfo` ladder and its escalation
trigger), **D13** (the music restarts are entry points, and the block arguments), **D17/D20** (what
a reference index is: the trace this phase captures is measured in the same space, under the same
recipe), **D25** (every number reproducible), **D24** (tree vs release).

## Global Constraints

- **`sr-engine` keeps zero runtime dependencies**, asserted by a test
  (`crates/sr-engine/src/lib.rs`). **`sr-audio` is held to the same rule**, with the same kind of
  test: an S3M player needs no crate, and a dependency added for convenience is how a player stops
  being reproducible. `sr-audio` may depend on nothing but the standard library.
- **The player is deterministic and has no clock of its own.** It advances because the engine ticked
  — one `poll()` per logic tick, exactly as `MAIN/COPPER.ASM:466` calls `_zpollme` once per copper
  frame. Nothing in `sr-audio` reads `Instant`, sleeps, or looks at how fast the machine is. The
  audio device **drains** a ring buffer; it never drives the music, and a run with no device (all of
  `--headless`, every comparison) produces the same bytes.
- **Port the algorithm, not a resemblance.** Effects, periods, the volume ramp, the order-list
  handling and the `dis_*` sync calls are translations of the sources, and every claim about the
  original cites `file:line` in the commit message and in `docs/fidelity/music.md`, which ships in
  the same commit as the code it describes.
- **Assets are read in their original formats**, from `assets/MAIN/MUSIC{0,1}.S3M`, scrambled
  patterns included: the descrambling is part of the port, not a pre-conversion step. No
  descrambled copy is written into `assets/`.
- **`reference/` and `upstream/` are input-only.** The one exception this phase needs is stated in
  Task 1 and is the same shape as Phase 2's: `xtask music trace` writes a *new* capture directory
  under `reference/music/`, and nothing else writes there.
- **A measurement decides, a reading does not.** Where the sources are ambiguous — what the player
  does at an order-list `254` or `255`, whether `zplus` is 1 or 2 during a given pattern, what
  `zinfo` reports, what the reference's mixer rate is — the answer is the trace (Task 1), not the
  most plausible reading of ST3 semantics. A guess written as a fact is this phase's likeliest
  defect, and `docs/fidelity/music.md` must distinguish the two in every sentence.
- **Preserve the original's bugs** (`docs/fidelity/bugs.md`). The player's own quirks — the
  `ordersync1` table's read past its last entry (`DIS/DISINT.ASM:449-463`), the `-32` sentinel, the
  two phase-dependent rate limits in `muscode_6` — are behaviour, not accidents to tidy.
- **Every number in a note is reproducible** by a named command or a test in the same commit (D25),
  and **when a measurement disagrees with a faithful port, D24's procedure decides** what to do.
  The vendored tree is a development snapshot; the shipped release is the reference of record for
  every comparison, including this phase's trace.

## Review Focus

Five failure modes this phase is most likely to ship, each with the task that pins it:

1. **A guess about the order list, dressed as a measurement.** Both modules' order lists carry
   entries a linear player cannot simply play: MUSIC0 has `255` at orders 16, 17, 64, 69, 78 and 79
   and one `254` at order 27; MUSIC1 has fifteen `254`s and one `255`, at the end. Under stock ST3
   semantics `255` ends the song — which would stop MUSIC0 sixteen orders in, in a demo whose own
   shell restarts it at orders 18, 25 and 70 (`MAIN/U2.ASM:880-885`, `:890-894`, `:898-905`) — so STMIK is
   doing something else with them, and *what* decides the whole cue timeline. The trace shows the
   order counter's actual sequence; the plan requires the port to follow it and to say in
   `music.md` which rule the trace ruled in and which it ruled out. Pinned in Task 1 (the trace) and
   Task 3 (the order advance and its test).
2. **A music position that is right at the start and wrong by the end.** The demo is music-counted
   (D13): a part begins where the music says it begins, and a player that starts at the right
   `(order, row)` but advances at the wrong *rate* drifts invisibly until a diff at the end of the
   run. The rate is not "one row a frame" and not a chosen rows-per-second: it is tempo and speed
   through the original's mixing rule, quantised by the 70.086 Hz poll the copper performs. A
   one-tick error in the quantisation moves a cue by a frame, which is exactly what ENDLOGO's
   keyframes can see. Pinned in Task 1 (the start index and the measured rate), Task 3 (the whole
   window compared frame by frame) and Task 8 (ENDLOGO's four keyframes re-diffed).
3. **The effect set decided by reading §7.2's list instead of the module's cells.** The spec names
   the effects it expects; the modules' own data is what decides which the port must get right, and
   an effect the demo never plays must be *measured* absent rather than assumed. The consequence of
   getting one wrong is audible and, in `S`'s and `Q`'s case, visible in the row timeline as well —
   a note delay that fires a tick late moves a row's worth of pictures. Pinned in Task 2 (the
   histogram, from the data) and Task 4 (one test per effect, over the module's own cells).
4. **The engine learning about Scream Tracker.** `sr-engine` must not import `sr-audio` and the
   player must not reach into the engine: the seam is a `Music` trait with the six `dis_*` calls and
   a `poll`, and the proof is the manifest test plus a stub implementation the engine's own tests
   use. A leak here is the kind that later phases cannot undo, because every part would then be
   written against it. Pinned in Task 6.
5. **A verifier that skips the new kind of reference.** `reference verify` walks the frame captures
   (`xtask/src/verify.rs`); a *trace* is a new kind of capture, and the natural implementation —
   `if kind != "frames" { continue }` — is the phase's signature defect: a check that reports
   nothing is indistinguishable from one that passed. The verifier must check the trace's digest,
   must refuse a capture whose kind it does not know, and must be seen to fail on a flipped byte.
   Pinned in Task 1, Step 5.

---

## Part A — the instrument

### Task 1: STMIK's symbols, the live trace, and the measured music clock

The phase has no oracle for music: no reference audio exists (the emulator runs silent — D17
requires it, because a real audio device makes the animation drift), and the two modules' *timing*
is invisible from the outside. What is visible is the player's own state: STMIK keeps its position
in `np_ord`, `np_row`, `np_pat`, `np_zplus`, `np_zinfo` and `np_zframe`
(`MAIN/STMIK.H:125-131`), and `dosbox-automation` can read guest memory
(`xtask/src/oracle/dosbox.rs:561`, `GET /api/v1/memory/{segment}/{offset}/{len}`). So the oracle
this phase builds is **a per-frame trace of the original's own player state**, and it is the ground
truth for Tasks 3, 6 and 8. This is spec §8.3's event comparison, applied to the one subsystem
whose events the frame captures cannot show.

**Files:**
- Create: `xtask/src/music.rs` (the symbol map, the locator, the trace command)
- Create: `docs/fidelity/music-clock.md` (the measured clock: every number in it reproducible by a
  command this task adds)
- Modify: `crates/sr-assets/src/omf.rs` (or a sibling module — see Step 1) for the public symbols
- Modify: `xtask/src/main.rs` (the `music` subcommand), `xtask/src/reference.rs` or
  `xtask/src/verify.rs` (the trace's manifest and its verification)
- Modify: `docs/decisions.md` (D26), `docs/fidelity/APPROXIMATIONS.md` (only if the trace leaves
  something unmeasured that a port constant has to stand in for)
- Test: `xtask/tests/music.rs` (new), `crates/sr-assets/tests/omf_symbols.rs` (new)

**Interfaces:**
- Consumes: `xtask::oracle::{Oracle, Dosbox}`; `Oracle::boot/set_cycles/step_frames/wait_for_frame`
  and the dosbox backend's `read_memory(segment, offset, len)`; `sr_assets::omf::extract_ledata`.
- Produces:
  - `pub struct Symbol { pub name: String, pub segment: u16, pub offset: u32 }` and
    `pub fn public_symbols(object: &[u8]) -> Result<Vec<Symbol>, OmfError>` — the OMF **PUBDEF**
    walk, beside the existing LEDATA reader. It must name every symbol `MAIN/STMIK.H` and
    `MAIN/STMIKA.INC` declare — `_zinit`, `_zpollme`, `_zoutputmode`, `_zmixspeed`, `_zspeedadjust`,
    `_zspeedadjust2`, `_zpollmix`, `_zbufsize`, `_np_ord`, `_np_row`, `_np_pat`, `_np_loop`,
    `_np_zinfo`, `_np_zplus`, `_np_zframe`, `_np_speed`, `_np_tempo` — and the test asserts those
    names by name, not "some symbols were found".
  - `pub struct PlayerState { frame: u64, ord: u8, row: u8, pat: u8, loop: u8, speed: u8, tempo: u8,
    zplus: i16, zinfo: i16, zframe: i16 }` — one traced row. `loop` is in it because Task 3's
    comparison asserts the whole position and `np_loop` is one of the seven words the trace reads;
    a column the comparison needs is cheaper here than a second capture later.
  - `pub fn locate(target: u16, needle: &[u8], read: impl FnMut(u16, u32, usize) -> Vec<u8>) ->
    Result<(u16, u32), LocateError>` — find the STMIK segment in guest memory by a byte run taken
    from the object's own LEDATA, requiring a **unique** match and confirming it with a second,
    independent run.
  - A trace directory under `reference/music/<name>/` holding `trace.csv` (one row per reference
    frame, header naming the columns) and `manifest.json` (recipe: block argument, boot, cycles,
    keys, the emulator's description, the physical address each word was read from, the symbols'
    offsets, and a `sha256` per file) — the same shape and the same guarantees as a frame capture
    (D19, D23).
  - `xtask music symbols --object upstream/SecondReality/MAIN/STMIK.300` — prints the map, so the
    note's numbers have a command behind them.

- [ ] **Step 1: The OMF public symbols, and a walk that fails loudly**

`extract_ledata` walks the record framing (`crates/sr-assets/src/omf.rs:73`); a symbol walk needs
**SEGDEF** (`0x98`/`0x99`, with the segment's length in paragraph units) and **PUBDEF**
(`0x90`/`0x91`, name plus a segment index and an offset), and it must walk *past* every other record
type by its length prefix. That last part is where this step can fail silently: a walk that skips
unknown types is correct only if the length prefix is read correctly at every step, and a walk that
mis-steps still "finds symbols" further down the file. So the walk, in this order:

1. Records every type byte and its length it encounters, and returns them alongside the symbols
   (`pub struct Walk { pub symbols: Vec<Symbol>, pub types: BTreeMap<u8, usize> }`).
2. Refuses (`OmfError::Malformed`) on a record whose length runs past the end of the object, and on
   a walk that does not end exactly at the last byte.
3. Has a test that **checks the measured type census** of `STMIK.300` against a number the test
   itself counts from the file, so a walk that silently stopped early cannot pass.
4. Asserts the three structural facts the phase depends on, each independently falsifiable: the
   seven `np_*` offsets are **consecutive two-byte steps in the order `MAIN/STMIK.H:125-131`
   declares** (`np_loop, np_ord, np_pat, np_row, np_zinfo, np_zplus, np_zframe`); every symbol's
   offset lies inside its SEGDEF's declared length; and `_zinit`'s segment is the same as
   `_np_row`'s (STMIK is one segment, which is why `DIS/DISINT.ASM:242-282` can do
   `mov es,cs:stmikseg` once and read all of them).

**Measured while writing this plan, and to be re-derived by the test**: `STMIK.300` is **278
records** — THEADR 1, COMENT 5, MODEND 1, **PUBDEF 45**, LNAMES 2, **SEGDEF 1**, LEDATA 105,
**LIDATA 118** — and a walk that steps by `3 + length` ends exactly at byte 24,849 of 24,849. The
primary route is therefore available (45 public symbols, one segment), and two consequences are
worth stating before the work starts: there is **one segment**, which is what makes a single
`mov es,cs:stmikseg` sufficient (`DIS/DISINT.ASM:242-282`), and the segment's bytes are carried by
**LIDATA as well as LEDATA** — so the locator's needle is taken from a LEDATA run (verbatim data),
and nothing in this phase has to expand an iterated record. If the walk's census disagrees with the
file, the walker is wrong and the symbols it "found" are worthless: the test asserts the census it
counted itself.

Should the object have turned out to carry no PUBDEF records, the ladder would be: the
object's **LINNUM** records (`0x94`/`0x95`, source line → segment offset) against the data
declarations in `MAIN/STMIK.H`, then **D8's route 3** (disassemble the segment) with its cost
recorded. Do not invent a map, and do not stop at the first route that produces *some* symbols.

Whichever route is used, **identification is confirmed by behaviour as well as by name**: the words
the trace reads must behave like the counters they are named after over a whole capture (a row that
runs 0…63, an order that runs 0…79 or 0…99, a speed and a tempo that hold the module's own values),
and the manifest records the addresses, the route that produced them and the behavioural check.
That check is not decoration: it is what makes the four steps of Step 4 measurable at all, and it is
the difference between reading `np_row` and reading the byte next to it.

- [ ] **Step 2: Find the STMIK segment in the running original**

Symbol offsets are relative to a segment base the object cannot know. Locate the base from the
running guest: take a byte run from the object's own LEDATA (a distinctive one — at least 32 bytes,
chosen by a rule the tool prints, e.g. the first run with no byte repeated in the whole object),
scan the guest's linear memory for it, and require **exactly one** match; then confirm with a second
run from a different LEDATA block at a different offset. The confirmation is the point: one match is
a coincidence until a second, independently chosen run lands at the same base.

The scan reads the guest in 64 KiB requests (`read_memory(segment, 0, 0x10000)` walking segments
`0x0000, 0x1000, 0x2000, …`) so the whole conventional-plus-UMB space costs ~20 requests rather than
65,536. It runs **after the demo has reached graphics** (the loader must have executed), and it
re-runs at the end of the trace to prove the segment did not move — a moved segment would make
every later sample a different variable, which is the failure mode that would otherwise look like a
music anomaly.

Both ends of the check are recorded in the trace's manifest: the base, the two needles' offsets and
`sha256`s, and the guest's own `describe()`.

- [ ] **Step 3: `xtask music trace`**

One command, one capture, same discipline as `reference capture`:

```
cargo xtask music trace --arg 5 --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 \
      --frames 0..1300 --out reference/music/second5
```

- It boots the original under the D17 recipe (silent emulator; the same keys, because the soundcard
  the keys select is part of the measurement — see Step 4), steps frame by frame, and reads the
  player's words at **every** requested index.
- **Every frame, not every Nth**: the tick quantisation of Review Focus 2 lives in the *sequence* of
  row transitions, and a sparse sample cannot see a one-frame error. Each sample is **keyed by the
  frame counter the guest itself reports**, never by a hoped-for index, so a sample that arrives late
  is one frame with a different number rather than a wrong value under the right number. Each read
  is **bracketed by two frame-counter reads**, and a sample whose frame moved between them is
  discarded and re-taken; the seven `np_*` words are contiguous (`MAIN/STMIK.H:125-131`, verified in
  Step 1) so one memory request returns all of them and they cannot be torn *against each other*.
  The manifest states the coverage actually achieved — which frame numbers are present and which
  are missing — and the tool **refuses to describe a trace with gaps as per-frame**, because a trace
  that quietly sampled every other frame is the phase's signature defect with a data file around it.
- The window is fixed by what the phase has to answer, and it is stated in the manifest: `0..1300`
  covers the boot (where the music starts), the first graphics frame (404), and ENDLOGO's whole
  gate window (580…1214), which is what Task 8 checks A7 against. A second, longer capture is taken
  once the first shows the rate is steady (Step 4 decides how long).
- `--frames` takes a range as well as a list (`0..1300` or `0,1,…,1299`), because a trace is dense
  by nature and a 1,300-item list is not a command line.
- It writes the trace **and** the manifest, refuses to write into a directory that already holds a
  trace (`--force` to replace), and refuses to write at all if the numbers fail their own
  coherence checks — the `np_*` offsets consecutive, the segment stable, `frame` strictly
  increasing, `speed` and `tempo` inside 1..=255.
- `--out` inside a *frame* capture's directory is refused, `reference/` gets an explicit refusal
  list; the same rule `compare` enforces (`xtask/src/compare.rs`).

- [ ] **Step 4: The measurements, and `docs/fidelity/music-clock.md`**

This is the task's real deliverable: the numbers Tasks 3, 6 and 8 are written against. Each is a
derivation from the trace, and each is written into the note with the command that produces it:

1. **Where the music starts.** The first reference frame at which the player is advancing (the row
   or the order changes, or `zframe` first moves). This is the port's **music boot anchor**, the
   counterpart of Phase 2's 404, and everything in Task 7 hangs off it. Report it as a frame index
   with the evidence (the frames either side, and what each register held).
2. **The tick rate, and the rule behind it.** Rows advance one per `speed` ticks and a tick is
   `2.5/tempo` seconds (`MAIN/STARTMUS.C:257` patches MUSIC0's tempo byte to 120;
   `assets/MAIN/MUSIC0.S3M` carries speed 6, `MUSIC1.S3M` speed 3 and tempo 130). So the row rate
   is derivable from the module's own header — *and the trace must reproduce it*: fit nothing, but
   check that `rows_advanced × speed × 2.5/tempo` equals the elapsed frames per second within the
   measurement's own resolution over the longest window available, and report the residual in
   frames. The rate is a **prediction** of the trace, not a fit to it.
3. **The quantisation.** The sequence of frame indices at which the row changes, for a long run.
   Report the distribution of gaps (in frames) and whether it is periodic. This is what Task 3's
   player must reproduce tick for tick, and the trace's own gaps are the test.
4. **Which soundcard and mixer rate the reference runs at.** Read `_zoutputmode`, `_zmixspeed`,
   `_zpollmix`, `_zbufsize`, `_zspeedadjust`, `_zspeedadjust2` and `_np_mastermul` from the guest
   and report them. This settles spec §13's open question (`zspeedadjust`/`zspeedadjust2`'s effect
   on audible timing) and the spec §7.2 default, from the machine rather than from `MENU.C`'s menu
   order. Note that `MAIN/U2.ASM:389` writes `_zspeedadjust2` **only** for outputmode 7 (no sound)
   or 2 (Gravis) — so under the reference's own card the value the shell passes is not the value in
   memory, and what is in memory is what the port must use.
5. **`zplus` and `zinfo`.** Both are sampled per frame. For `zplus`: report the value against
   `(order, row)` and state whether it is a function of them (the check: every frame with the same
   `(ord, row)` inside one capture has the same `zplus`), and at which rows the transitions happen.
   For `zinfo`: report its distinct values, when each appears, and the correlation with `(ord,
   row)`, `zplus`, the module's order list and its pattern data. **`zinfo` is this task's D8
   milestone**: either it is resolved here (its values reproduce from something in the module data
   or from a rule over the position), or the escalation trigger fires and D8 gains the record of
   which routes were tried and what the substitute cue will be. Do not let it block the task:
   resolve, or defer in writing with D8's evidence.
6. **The two order-list questions.** What the player does at MUSIC0's six `255`s and its one `254`,
   and at MUSIC1's fifteen `254`s — read off the order counter's own sequence in the trace.

- [ ] **Step 5: The trace is verified, and the verifier is seen to fail**

`cargo xtask reference verify --tree reference` must check the traces too: every file the manifest
names is present with the length its row count implies, every digest matches, and a `kind` the tool
does not know is a **failure**, not a skip. The trace's manifest declares itself
(`"kind": "music-trace"`); a manifest with **no** `kind` is a frame capture, which is what the 24
captures already in the tree are (D23's `version` field is the precedent for adding a field to a
manifest that already exists), and any *other* value is refused by name. Prove it both ways, in the task's report: the tree
passes (exit 0), and a copy with one byte of `trace.csv` flipped fails (exit 1) naming the file and
both digests. A verifier that has never been seen to fail is worth nothing (this is Phase 2's
Task 2 precedent, and the phase's Review Focus 5).

- [ ] **Step 6: Commit what is measured so far**

Commit the object-symbol API, the locator, the trace command, the trace itself, the note and D26.
The commit message states the measured start index, the rate, and what the trace says about
`zinfo` — in one sentence each, with the numbers.


- [ ] **Step 7: The anchor's session spread, measured (added after the first review)**

The trace's `zframe` column puts the music's first tick at frame 409 *or* 410 in `second5` — the
trace holds `zframe` 0 at 408 and 1 at 410 and **never published 409** — and the row-change timeline
independently requires 410 (a sample-accumulator simulation reproduces 57 of 61 row intervals at 410
and **none** at 409), which is also what the filed reference frames require from the other side
(ENDLOGO's gate breaks at 1146). One session, however, is not a spread: `hiddenu`, captured in a
different session, holds a 409 with `zframe` 1 and loses 410 instead, so *that* session's first tick
was a frame earlier.

So: **re-take `second5` in a fresh session and report three numbers** — the `zframe` anchor, the
start the row-change timeline implies, and whether they agree with the filed trace's. If the fresh
session agrees with the filed one, the anchor is a property of the boot and the note can drop the
"409 or 410" hedge for the recipe; if it differs by a frame, take a third session and record the
spread as a measured interval, the way §1.7 did for the music clock's rate. Either outcome is a
result; what is not allowed is leaving the port's anchor resting on an argument when one emulator
run would settle it. File the new trace beside the old one (a distinct name), with its manifest, and
let `reference verify` cover it.

---

## Part B — `sr-audio`

### Task 2: The module — descrambling, patterns, instruments

**Files:**
- Create: `crates/sr-audio/` (`Cargo.toml`, `src/lib.rs`, `src/module.rs`)
- Modify: the workspace `Cargo.toml` (member + `workspace.dependencies` entry)
- Create: `docs/fidelity/music.md` (the note's first half: the container, the keystream, the data)
- Test: `crates/sr-audio/tests/module.rs`, unit tests in `src/module.rs`

**Interfaces:**
- Produces:
  - `pub struct Module` with `load(bytes: &[u8]) -> Result<Module, ModuleError>`, `orders() ->
    &[u8]`, `order_count()`, `pattern_count()`, `instrument_count()`, `initial_speed()`,
    `initial_tempo()`, `global_volume()`, `master_volume()`, `stereo() -> bool`,
    `channel_pan() -> &[u8; 32]`, `instrument(i) -> Option<&Instrument>`, `pattern(p) ->
    Option<&Pattern>`, `sample_bytes(i) -> Option<&[u8]>`.
  - `pub struct Pattern { pub rows: Vec<[Cell; 32]>, pub channel_count: u8 }` (32 cells a row, one
    per channel, the S3M maximum) and `pub struct Cell { pub note: u8, pub instrument: u8, pub
    volume: u8, pub command: u8, pub info: u8 }` with `Cell::EMPTY` and `is_empty()`.
  - `pub fn apply_tempo_patch(&mut self, tempo: u8)` — `MAIN/STARTMUS.C:257`'s `module[50]=0x78`,
    which the shell applies to MUSIC0 and *only* to MUSIC0 (`:252-265`: `case 0` is MUSIC0).
  - `pub fn descramble(pattern: &mut [u8])` / a private `key(i) -> u8` — D5's keystream,
    `(((n ^ (n >> 2)) << 3) | ((5*i + 2) & 7)) & 0xFF` with `n = (i >> 1) + 1`, applied to
    `data[ptr+2 .. ptr+ln]`.

- [ ] **Step 1: The loader, and the tests that would catch a wrong one**

D5's verification is the specification and every number in it is re-derived here, not copied:

- MUSIC0: **77 of 77** patterns decode as exactly 64 rows consuming exactly their data; MUSIC1:
  **76 of 77**, the exception being a cell whose command byte is 0 — treat 0 as *no command*
  (`MAIN/STMIK.H`'s player has no command 0; D5 records the cell).
- Zero invalid command bytes over both modules with the **ST3 numeric** alphabet (1–26 = `A`–`Z`,
  `0x30`–`0x39` = digits, 255 = none) — not ASCII.
- Note bytes are `(octave << 4) | semitone` and the valid range runs to `0x9B`; a decoder that caps
  at 96 rejects MUSIC1's high material. The test asserts the **maximum note found in each module**
  and that the cap is above it.
- The effect histogram, counted from the decoded cells, is asserted **per module**: the counts are
  the phase's scope evidence (Review Focus 3) and `music.md` reproduces them. D5 records the
  histogram it verified — `D:7348, F:1051, E:827, G:802, O:755, H:212` — and those six are
  **MUSIC0's own counts**, measured while writing this plan by decoding both modules with the D5
  keystream and the S3M cell flags (bit 5 = note *and* instrument, two bytes; bit 6 = volume; bit 7 =
  command and info): MUSIC0 `A:63, B:3, C:8, D:7348, E:827, F:1051, G:802, H:212, J:137, K:105,
  O:755, Q:84, S:140`, MUSIC1 `A:54, C:4, D:3340, E:217, F:180, G:2607, H:365, K:62, O:2417, Q:38,
  S:48` plus **one cell whose command byte is 0** — which is D5's recorded MUSIC1 exception, and the
  decoder must treat 0 as no command. The test counts them itself; the numbers above are what it
  should find, and a disagreement is a finding in the note either way.
- Each module's header, asserted as measured: `ordNum/insNum/patNum` 80/44/77 and 100/54/77, global
  volume 64, initial speed 6 and 3, initial tempo **125** (before the patch) and 130, master volume
  176 (stereo bit set), and the pan table's 32 bytes. The patch is applied in a test and the tempo
  read back as 120.
- Instruments: the sample header is parsed in full (type, memory segment, length, loop begin/end,
  volume, pack, flags, `c2spd`), sample bytes are sliced by paragraph pointer, and the **type
  census** is asserted: MUSIC0 is 22 PCM and 22 empty, MUSIC1 41 PCM and 13 empty, **no type-2
  (adlib) instrument in either module** — which is why this phase does not model OPL, a fact that
  belongs in the note as a measurement.
- The pattern pointer table's location (`0x60 + ordNum + insNum*2`, paragraph units) and the
  observation D5 relied on — the blocks tile contiguously on 16-byte boundaries — are both asserted,
  because they are the evidence that the pointer table was read correctly at all.
- `SCRS` at the sample header's tail, `SCRM` at the file's offset 44: named checks, not magic
  numbers in prose.

- [ ] **Step 2: The order list, preserved as data**

The order list is read as bytes and **nothing is normalised**: 254 and 255 are kept. What the player
does with them is Task 3's, decided by Task 1's trace. A loader that "fixes" the order list here
would hide the question instead of answering it, and the test asserts the exact byte sequences of
both modules (MUSIC0's 80 orders including `255` at 16, 17, 64, 69, 78, 79 and `254` at 27; MUSIC1's
100 including fifteen `254`s and one final `255`).

- [ ] **Step 3: The crate's dependency test, and the note**

`crates/sr-audio/src/lib.rs` carries `the_player_has_no_runtime_dependencies`, the same shape as
`sr-engine`'s: it reads the manifest's own text and fails on any entry under `[dependencies]` or a
`[dependencies.x]` table. `docs/fidelity/music.md` is started here with what is *measured*: the
container layout, the keystream and its verification, the two modules' headers, the instrument
census, the effect histogram, and the order list as data.

- [ ] **Step 4: Commit**

---

### Task 3: The player — position, and the trace's own timeline

**Files:**
- Create: `crates/sr-audio/src/player.rs`
- Modify: `crates/sr-audio/src/lib.rs`, `docs/fidelity/music.md`
- Test: `crates/sr-audio/tests/player.rs`, `crates/sr-audio/tests/trace.rs` (the comparison against
  `reference/music/`)

**Interfaces:**
- Consumes: `Module` (Task 2).
- Produces:
  - `pub struct Player` with `new(module: Module, options: PlayerOptions) -> Player`,
    `play_song(&mut self, order: u8)` (`_zplaysong`, `MAIN/STMIK.H:43`), `goto_song(&mut self,
    order: u8, row: u8)` (`_zgotosong`, `:46`), `stop(&mut self)`, `shut_up(&mut self)`
    (`_zshutup`, `:39`), `set_global_volume(&mut self, v: u8)` and `global_volume(&self) -> u8`
    (the shell's `fademusic`, `MAIN/U2.ASM:333-346`, which walks the module's own global-volume
    byte down to 0).
  - `pub struct Position { pub order: u8, pub pattern: u8, pub row: u8, pub speed: u8,
    pub tempo: u8, pub loop: u8 }` — the words `DIS/DISINT.ASM` reads out of STMIK
    (`np_ord`, `np_pat`, `np_row`, `np_speed`, `np_tempo`, `np_loop`).
  - `pub fn poll(&mut self)` — **one video frame has passed**: the player mixes this frame's samples
    and advances the tracker by however many whole ticks that covers. This is the whole of the
    port's timing model, and it is where Review Focus 2 lives.
  - `pub fn state(&self) -> Position`, `pub fn zplus(&self) -> i32`, `pub fn zinfo(&self) -> i32`,
    `pub fn zframe(&self) -> i32`, `pub fn set_zframe(&mut self, frame: i32)`.
  - `pub struct PlayerOptions { pub mix_rate: MixRate, pub stereo: bool, pub interpolation: bool }`
    with `pub enum MixRate { Hz12000, Hz16000, Hz20000, Hz40000 }` — the original's four
    (`MAIN/U2.ASM:737-743`, `:759`), with the reference's measured one (Task 1, Step 4.4) as the
    default.

- [ ] **Step 1: The tick advance, decided by the trace and not by arithmetic taste**

The rule to implement is: samples accumulate at the mix rate across polls; the tracker advances one
whole tick whenever the accumulated total reaches one tick's worth (`mix_rate × 2.5 / tempo`
samples), carrying the remainder; the row advances one per `speed` ticks; at a row past the
pattern's last (64), the order advances. The observable consequence — **which frame a row change
lands on** — is in Task 1's trace, and this task's acceptance is a byte-for-byte agreement with it
over the whole capture window, not a plausible-looking rate.

**That rule is a first approximation, and it was measured to be one while this plan was being
written** (the controller's own simulation of exactly the sentence above, against `second5`'s
speed-6 run, frames 427…961): starting the music at frame 409 it reproduces **none** of the 61 row
intervals, starting at **410** it reproduces 57, and **four intervals are a frame late at any
initial phase** — the trace changes rows at 576, 664, 848 and 918 where the model changes them at
575, 663, 847 and 917. Three of those four are inside ENDLOGO's hold window, which is the phase's
spine. So this step is not "implement the sentence above": it is **derive the rule that reproduces
every one of the trace's row changes**, with the sentence above as the starting model and the four
named intervals as the evidence that something else is in it (candidates to rule in or out, by
measurement: the player mixes from its own timer — Task 1 measured `_zpollmix = 0` — so a delayed
interrupt is one; a mixing-chunk quantisation the model does not carry is another). The 410-vs-409
half of the measurement is the session offset Task 8 will meet from the other side. Two implementation details are
therefore load-bearing and must be pinned by tests of their own: the *rounding* of samples per frame
(`mix_rate / 70.086` is not an integer — the accumulator must carry fractional samples rather than
round per frame, or the rate drifts by construction) and the *order* of the row-limit checks at a
tick boundary.

A third is the **row column's phase against effect processing**, and Task 1's trace already shows
it: MUSIC0's pattern 71 sets speed 1 in its row-0 cell and the trace's `speed` word is 1 while its
`row` column still reads the last row of the *previous* pattern, one row earlier than the cell that
sets it. Whether the player applies a row's effects before or after it publishes the row is
therefore visible in the trace and must be reproduced rather than smoothed over — this is a
one-frame-sized detail of exactly the kind a cue lands on.

- [ ] **Step 2: The order advance, from the trace**

Implement `play_song`/`goto_song` and the order advance to match the counter sequence Task 1
measured, including whatever the player does at `254`/`255`.

**This step owns the three effects that move the timeline — `A` (set speed), `B` (pattern jump) and
`C` (pattern break) — because the traces' own windows exercise two of them, and a comparison that
matches the row changes cannot be written without them** (measured while writing this plan, from
the modules' own cells):

- `A`: MUSIC0's pattern 70 row 0 sets speed 6, pattern 71 row 0 sets 1 and row 36 sets 6 again;
  MUSIC1's pattern 30 row 0 sets 6 (which is why the trace's speed word reads 6 where MUSIC1's
  header byte says 3). `reference/music/second5`'s speed-1 window (frames 979…1021) is the first
  effect's doing, and its return to 6 is the second's.
- `C`: MUSIC0's pattern 71 row 61 breaks — and the trace shows it: `second5` and `hiddenu` both go
  from ord 26 row 61 to **ord 28 row 0** (`second5` frame 1251 → 1260), stepping over the `254` at
  order 27 on the way. MUSIC1 has four `C` cells (patterns 13, 21, 41, 73).
- `B`: MUSIC0 has three (patterns 26, 62 and 69, played at orders 63, 66 and 77) and MUSIC1 none.
  No filed window reaches one, so `B` is implemented and unit-tested here but **not** exercised by
  the comparison — and `music.md` says so rather than implying the trace checked it.

The channel effects are Task 4's; the split is by what the timeline needs, not by what the data
contains. The port must be able to play a module
**end to end** (the exit criterion): MUSIC0 through its 80 orders and MUSIC1 through its 100, with
the order counter's stops, skips or repeats matching the trace's. Where the trace and stock ST3
semantics disagree, the trace wins (D12: the release is the reference of record) and the disagreement
is written into `music.md` with both readings.

The end-to-end play is **shown, not asserted**: a test plays each module from its start order to the
point the player reports the song finished (or the order counter stops advancing), asserts where it
stopped and how long it took in frames, and prints both — so `PROGRESS.md`'s evidence section can
quote a command rather than a claim. A second test plays *past* that point for a further window and
asserts the player does not panic, hang or run off the end of the order list: a player that loops
for ever is fine (the demo restarts its music), a player that reads past its orders is not.

- [ ] **Step 3: The comparison against the trace, as a permanent test**

`crates/sr-audio/tests/trace.rs` reads `reference/music/<name>/trace.csv` and drives a `Player`
for the same number of polls, starting at the trace's own music-start frame, and asserts that
**every frame's `(order, row)` equals the trace's** — and `speed`, `tempo`, `pat` and `loop` with
it. This is the phase's exit criterion in executable form. It is *unconditional* (the trace is
committed, not scratch), so a fresh clone runs it.

Three things the test must not do: skip frames where the trace shows no change (a player that never
advances would pass), start from a hand-picked order (`play_song(trace.first_order)` hides the
start-index question — the test starts the player at the boot's own frame and lets it run to the
trace's first sample), or paper over the trace's **gaps**. Task 1's traces are keyed by the guest's
counter and carry a handful of frames the emulator never published (a mode change swallows an index:
147, 409 and 1220 in the three filed traces, each named in its manifest). The comparison runs over
the frames the trace *has*, and the gaps stay gaps — a comparison that quietly renumbers around them
is comparing a different timeline, and the manifest is where the honest statement of coverage lives.

- [ ] **Step 4: `zplus`, and `zframe`**

`zplus` is computed by the port from its own position to reproduce the trace's column for every
frame — the rule comes from Task 1's correlation (Review Focus 1's cousin: if the trace's `zplus` is
a function of `(order, row)`, the port computes it; if it is *not*, that is a finding, and the port
must reproduce whatever it is instead, with `music.md` saying what). `zframe` is the frame counter
parts wait on (`_dis_getmframe`, `DIS/DISC.ASM:150-159`, forwarded by `DIS/DISINT.ASM:412-426`),
advanced at the rate Task 1 measured — spec §6.3's deliverable, and the one number in this phase
that is a *rate* rather than a position. `_np_zframe` is written by the parts themselves
(`_dis_setmframe`, `DIS/DISC.ASM:138-148`) and read back by them (`:150-159`, forwarded by
`DIS/DISINT.ASM:412-425`), so its *phase* matters as much as its rate: a counter that is 1 at frame
*n* in the original and 0 in the port moves a 300-frame wait by a frame.

- [ ] **Step 5: Commit**

---

### Task 4: The effects

**Files:**
- Create: `crates/sr-audio/src/effects.rs` (or one module per family, if that reads better)
- Modify: `docs/fidelity/music.md`
- Test: `crates/sr-audio/tests/effects.rs`

- [ ] **Step 1: Scope from the histogram, then one test per effect**

Task 2's histogram is the list of effects the modules actually use; it decides this task's scope,
and any effect in spec §7.2's list that the histogram shows **zero** of is *recorded as absent with
the count*, not implemented on spec's say-so (and not silently missing either — the note names it).
For each effect that does occur, the test drives the module's own cells through the player and
asserts the state after each tick against the effect's specification:

The set, measured while writing this plan by decoding both modules (Task 2's own count is the
authority), is **A, B, C, D, E, F, G, H, J, K, O, Q, S** — note that **`J` (arpeggio, 137 cells in
MUSIC0) and `B` (pattern jump, 3 cells) are present** and spec §7.2's list omits them, while **`T`
(tempo) occurs nowhere in either module**: the tempo is the header's, patched once, and never moved
by the data. `I`, `M`, `N`, `P`, `R`, `U` and `V` are absent too. The tests, one per effect:

- **A** set speed (and its effect on the row rate — the trace shows MUSIC0's pattern 70 setting 6,
  pattern 71 setting 1 and then 6 at row 36, and MUSIC1's pattern 30 setting 6), **B** pattern jump
  (order and row together, and the row it takes effect on), **C** pattern break (the next order, row
  0).
- **D** volume slide (up/down nibbles, fine variants), **E**/**F** porta down/up (with the fine and
  extra-fine subcommands), **G** tone porta, **H** vibrato (speed/depth nibbles, the waveform and
  the position counter's update), **K** vibrato + volume slide (the shared memory with `H` and `D`
  is the classic trap: `K` and `D` share one "last effect" byte in ST3), **J** arpeggio (the
  tick-rotating base/+4/+7 semitone cycle and the period table it reads), **O** sample offset (in
  *samples*, not bytes, and the 256-multiple form), **Q** retrigger, **S** subcommand `SDx` note
  delay only — and the measured subcommands are **MUSIC0 `D3` 77 cells and `D6` 63, MUSIC1 `D2`
  48**, so the delay is by 2, 3 and 6 ticks and nothing else: the test asserts that set, not the
  general form.
- Each test is written against the *data*: it names the order, row and channel it drives, and the
  expected values come from the effect's own arithmetic (the S3M/ST3 rules), never from the port's
  current output. A test that asks the port what it does and then asserts that is the phase's
  signature defect wearing a green tick.
- **Periods and the note table** are their own step because everything else is measured in them:
  the ST3 period table (the S3M's 12-TET table with `c2spd`), `c2spd` 8363 as the reference, and the
  arithmetic that maps a period to a playback step (`(8363 × 2^octave × 16) / period`, integer
  division, as ST3 does). One test per octave against the table's own values.

- [ ] **Step 2: What Task 3 already owns**

`A`, `B` and `C` move the timeline, so Task 3 implements them (with their tests) to satisfy its own
comparison — the trace's own order change at `second5` frame 1260 is a `C` firing. This task's job
is the **channel** effects; nothing here should re-implement the timeline's. A defect in `C` shows
up as a frame-by-frame mismatch in Task 3's unconditional test, which is why that test is written
before this step and kept.

- [ ] **Step 3: Commit**

---

### Task 5: The mixer

**Files:**
- Create: `crates/sr-audio/src/mix.rs`
- Modify: `crates/sr-audio/src/player.rs` (the poll pushes mixed samples), `docs/fidelity/music.md`
- Test: `crates/sr-audio/tests/mix.rs`

**Interfaces:**
- Produces: `pub fn take_samples(&mut self, out: &mut [i16]) -> usize` on the player — the device's
  drain, no more — plus a bounded ring buffer whose overflow policy is stated and tested (the
  original's DMA buffer plays *behind* the tracker: `MAIN/STMIK.H:100-103` says the `np_*` variables
  describe the mixing engine, which is "a bit ahead of the actual music"). The port's model is the
  same: the position the parts read is the engine's position, and the audio device is behind it by
  whatever it has not drained.

- [ ] **Step 1: The voice, and the arithmetic ST3 actually uses**

S3M samples are **unsigned 8-bit**, mono or stereo per the header flag, and ST3's mixer is
**integer, no interpolation** — the step through the sample is fixed-point, the loop is honoured
from the sample header, and volume is applied per channel before the mix. The port reproduces that
model, with tests that compute expected sample values *by hand from the module's own bytes*: a
single channel, one note, one tick, the first N output samples checked against values derived from
the sample data and the period, so the mixer's arithmetic is pinned independently of the player.

- [ ] **Step 2: Determinism, and the property that matters**

Two runs of the same window are byte-identical (asserted). The output is not silent (asserted — a
mixer that produces zeros passes every "does it run" test). And the samples-per-tick relation holds:
over a window of *k* rows the mixed sample count is `k × speed × mix_rate × 2.5 / tempo` within one
tick's worth, which is the audio-side counterpart of Task 3's position test and the only way to see
a mixer that drops or duplicates a tick's samples.

- [ ] **Step 3: The option, and the authentic default**

`MixRate` defaults to the reference's measured rate (Task 1), with the other three selectable, and
the note records what the original's own menu offered (`MAIN/U2.ASM:737-743`) and which of them the
reference used. Stereo follows the module's own flag *and* the output mode the reference used
(`MAIN/STARTMUS.C:123`'s `nosurround` for a mono card); the default is the reference's, and the
alternatives are the ones the hardware had.

- [ ] **Step 4: Commit**

---

## Part C — the demo plays music

### Task 6: The engine's music seam

**Files:**
- Create: `crates/sr-engine/src/music.rs`
- Modify: `crates/sr-engine/src/lib.rs`, `engine.rs` (the tick's poll point), `part.rs` (`PartCtx`)
- Modify: `bins/secondreality/src/*` where `Engine` is constructed (the seam must compile through)
- Test: unit tests in `crates/sr-engine/src/music.rs` and `engine.rs` (a stub `Music`), plus the
  dependency test that already lives in `lib.rs`

**Interfaces:**
- Produces:
  - `pub trait Music` with `fn poll(&mut self)`, `fn position(&self) -> MusicPosition`,
    `fn musplus(&mut self) -> i32`, `fn musrow(&mut self) -> i32`, `fn muscode(&mut self, code: i32)
    -> i32`, `fn sync(&mut self) -> i32`, `fn getmframe(&mut self) -> i32`, `fn setmframe(&mut self,
    frame: i32)` — the six calls of `DIS/DIS.H:37-59` in the order the parts call them, and nothing
    else. `poll` takes no arguments: the engine says "a frame passed", never "here are samples".
  - `pub struct MusicPosition { pub order: u8, pub pattern: u8, pub row: u8, pub speed: u8,
    pub tempo: u8 }` — what the window's status line shows and what a test can read.
  - `Engine::set_music(&mut self, music: Box<dyn Music>)` and `Engine::music(&self) ->
    Option<&dyn Music>`; `PartCtx::music(&mut self) -> Option<&mut dyn Music>`.
  - `pub struct NoMusic` — the engine's own tests' stub, which is **not** a degraded music: it
    returns the values a part would see with no player at all, so a part that ignores music is
    unaffected and a part that needs it is visibly wrong rather than silently fine.

- [ ] **Step 1: The poll point, cited**

`Engine::tick` currently runs copper slot 0, then the part's bands, then slots 1 and 2
(`crates/sr-engine/src/engine.rs`). The original's copper runs slot 0, then `_zpollme`, then the
rest (`MAIN/COPPER.ASM:452-466`). The poll goes **between slot 0 and the first band**, and a test
pins the order (a stub `Music` that records when it was polled against a part that records when its
bands ran).

- [ ] **Step 2: The six calls, verbatim, with their bugs**

These are *translations*, and three of them have a detail that a "clean" implementation would lose:

- `musplus` (`DIS/DISINT.ASM:242-282`, `DIS/DISC.ASM:102-112`): the two-phase rule — `zplus == 1`
  gives `row - 64` clamped up to `-32`, `zplus == 2` gives `row` while `row < 32` and `-32` after,
  `zplus == 0` gives `-32` always, and any other `zplus` value dispatches on `row > 32`. It is
  written out as a table test over every `(zplus, row)` pair, and the *port's* `zplus` comes from
  Task 3.
- `sync` (`DIS/DISINT.ASM:428-463`): the `ordersync1` table verbatim, the unsigned `jbe` compare,
  the `(order << 8) | row` key, and the **read past the table's last entry** when no row matches
  (the loop runs 16 times over 15 entries). The port cannot reproduce what that read returns — it
  is code, and the code differs between builds — so the port's behaviour there is a **declared
  boundary**: the reachable domain (ord ≤ 0x42) returns the table's codes, and beyond it the call is
  refused loudly rather than returned as a fabricated number, with the reason in `music.md` and an
  entry in `APPROXIMATIONS.md` if the phase cannot show the domain is unreachable for every part in
  the tree (the only caller, `PAM/OUTTAA.C:39`, is block 1's, and Phase 4's).
- `muscode` (`DIS/DISINT.ASM:242-282`, `DIS/DISC.ASM:91-100`): returns `zinfo`, whose value comes
  from Task 1's resolution or from the documented deferral, and the deferral's substitute is
  implemented here as the same *shape* (a function of the position) so that a later phase's
  swap-in is a one-line change.

- [ ] **Step 3: What the engine does when there is no music**

A run with no player — a `--part testcard` run, the engine's own tests — must not acquire a hidden
dependency on one: `PartCtx::music()` returns `None`, and the parts that need a cue are the ones
that say so. The shell refuses to start a part whose cue is required when the run has no music
(Task 7), rather than letting it wait forever.

- [ ] **Step 4: Commit**

---

### Task 7: The run has music

**Files:**
- Modify: `bins/secondreality/src/shell.rs` (the module and start order per block/entry point),
  `main.rs`, `window.rs` (the SDL audio device), `headless.rs`, `cli.rs`
- Create: `bins/secondreality/src/audio.rs` (the device, the thread, the ring buffer, the WAV dump)
- Test: `bins/secondreality/tests/audio.rs` (new), `bins/secondreality/tests/shell.rs` (the
  restart table)

**Interfaces:**
- Produces:
  - `pub fn music_for(block: char) -> (ModuleId, u8)` — **which module, at which order**, per the
    original's own table: block 1 (`partsmask & 1`) starts MUSIC0 at order 0
    (`MAIN/U2.ASM:766-775`), everything else starts MUSIC1 at order 0 (`:772-775`), and the
    *entry-point* restarts are `SECOND 3` → MUSIC1 at order 42 (`:856-863`), `SECOND 4` → MUSIC0 at
    order 18 (`:880-885`), `SECOND U` → MUSIC0 at order 70 (`:890-894`), `SECOND 5` → MUSIC0 at
    order 25 (`:898-905`), each guarded by its block mask exactly as D13 records. The table is a
    constant with a test that asserts every row against those citations.
  - The run's **music start**, in two halves, because the original does two different things. A
    mid-demo entry point *seeks*: `restartmus` ends in `_zplaysong(order)` (`MAIN/U2.ASM:394`), so
    `--block 3`, `--block 4`, `--block U` and `--block 5` start their module at the order the table
    names, at row 0 — a `play_song`, with the player's own state reset the way the original's
    player resets it. A run whose entry point is the *start* of the demo reaches its position by
    **polling** from the frame the music was started on (Task 1, Step 4.1): the port pre-polls that
    many ticks at the run's start, because the music's advance is deterministic and does not depend
    on a frame being drawn. Both halves carry the trace's own frame numbers in a test: the port's
    `(order, row)` at the reference index of its first drawn frame must equal the trace's.
  - `--audio-dump <FILE>` (WAV, 16-bit signed, the mixer's own rate and channel count) and
    `--no-audio` (no device; the player still runs) — the dump is the phase's artefact for the
    human listen and for tests that need the bytes rather than the sound.
  - `pub fn fade_music(player: &mut Player, step: u8)` — `MAIN/U2.ASM:333-346`'s `fademusic`, which
    subtracts from the module's global-volume byte until it reaches 0, called where the demo calls
    it (`:928`, `:939`, at the demo's own end).

- [ ] **Step 1: The device is a drain, and nothing else**

SDL2 audio (`sdl2::audio`, already in the tree), a callback that copies from the player's ring
buffer, and a policy for underrun that is stated and tested: **silence, never a stall or a
resample** — the music's position is the tick's, not the device's. The device opens only in window
mode and only when `--no-audio` was not passed; a machine with no audio device falls back to no
audio with a printed line rather than a failure (the demo must still run).

- [ ] **Step 2: The headless run plays the music, and that is the point**

`--headless` runs the same player with no device, so a headless dump of a music-gated part is the
same frames a windowed run draws. The test: a `--block 5` headless run to ENDLOGO's keyframes is
byte-identical with and without `--no-audio`, and the two runs' `(order, row)` at a fixed frame are
equal. Without this, the phase's comparisons would be measuring a demo that isn't the one that
plays.

- [ ] **Step 3: The status line, and what a person sees**

The window's title already shows the part, the index and the block; it gains the music position
(`order:row`), because that is what a person debugging a sync problem needs and what the phase's
human check is *about*. The CLI's `--help`/usage gains the two new flags.

- [ ] **Step 4: Commit**

---

### Task 8: ENDLOGO's gate becomes the real cue, and the phase's proof lands

ENDLOGO is the only part the port has that waits on music: `END/END.C:107-111`'s
`for(a=0;a<5000 && !dis_exit();a++) { dis_waitb(); if(dis_musplus()>-16) break; }`. Phase 2 replaced
it with its measured length — 567 ticks — and registered the stand-in as **A7**. This task removes
the stand-in and keeps the measurement as its *check*, which is what makes it the phase's proof:
A7 was derived from the reference frames, and the port's cue is derived from the player's state; if
they agree, two independent measurements of the same instant agree, and if they do not, one of them
is wrong and the phase says which.

**Files:**
- Modify: `bins/secondreality/src/parts/endlogo.rs`, `docs/fidelity/endlogo.md` (§5 and the
  gate's section), `docs/fidelity/APPROXIMATIONS.md` (A7 retired — *struck through with what
  replaced it named*, never deleted: `APPROXIMATIONS.md`'s entries are how a later phase knows what
  was once approximate)
- Modify: `PROGRESS.md`, `docs/decisions.md` if the check's outcome needs one
- Test: `bins/secondreality/tests/endlogo.rs` (the gate's new test), and the keyframe comparison

- [ ] **Step 1: The gate, in the part's own terms**

The part loops: each tick, ask `ctx.music()?.musplus()`; break when it is greater than `-16`; give up
at 5000 ticks as the source does. The 5000 is the source's own cap and stays. `dis_waitb()` is the
tick itself in the port (one band pass is one tick), so the loop is one call a tick.

- [ ] **Step 2: The check — A7's 567 against the cue's own answer**

Run ENDLOGO from the run's own start and record the tick at which the gate opens. It must be the
same tick Phase 2 measured from the reference frames, or the difference must be explained by naming
which of the two measurements is wrong (and if it is A7's, this is the place to say so, with the
frames). Two further readings go in the report: the trace's `musplus` values at the frames either
side of the gate's end (Task 1's capture must show it — if it does not, widen the trace rather than
reasoning about it), and the port's own `(order, row)` at both ends of the hold.

- [ ] **Step 3: The comparison, re-run**

`--block 5` and `--part endlogo`, dumped at 430, 500, 900 and 1180 and compared against
`reference/parts/endlogo` — the same command the phase's evidence table names. It must be **exit 0,
0.0000 % on all four, both planes byte-identical**, exactly as in Phase 2. This is the acceptance
test for the whole phase: the port's music clock, its start index and the part's cue all have to be
right for those four frames to survive. If they do not match, the defect is in the music (its
position, its rate or its start), not in the part — the part did not change except in *when* it
advances.

- [ ] **Step 4: The human listen — the step no agent can do**

`cargo run --release --bin secondreality -- --block 5` (and `--block 2`), with audio, watched and
listened to by the user. What is being asked is narrow and should be asked narrowly: does it sound
like the demo's music, and does ENDLOGO hold for about as long as it sounds like it should. The
answer is recorded as the user's own report, not interpreted (Phase 2's precedent). If audio is not
available on the machine, `--audio-dump out/block5.wav` is the fallback and the report says which
was used.

- [ ] **Step 5: Commit**

---

## Part D — closing

### Task 9: Close out Phase 3

- [ ] **Step 1: Re-run every criterion, at the committed state**

In one pass, into fresh output directories, with the numbers recorded verbatim: the two modules
played end-to-end (the end-of-song report), the trace comparison, ENDLOGO's four keyframes
(exit 0), `reference verify --tree reference` (now covering the traces), `assets verify --decode`
(unchanged, but re-run), `cargo test --workspace`, and both clippy commands (the exit-item
precedent).

- [ ] **Step 2: The falsification list**

Every constant and rule this phase introduced, in Phase 2's table shape: the value, how it was
derived, what a one-unit error would change, and the strongest *independent* check that exists for
it. The ones this phase already knows belong in it: the music boot anchor (Task 1), the tick
quantisation rule (Task 3), the `zplus` rule (Task 3/6), `zframe`'s rate (Task 3), the mixer's
samples-per-tick relation (Task 5), and A7's 567 as the independent check on the cue (Task 8).

- [ ] **Step 3: The negatives**

What could not be confirmed, stated as such: no audio reference exists (the oracle runs silent by
D17, so the mixed *sound* is verified by construction, by internal invariants and by a person's
ears — never by a null test against the original); `zinfo`'s status (resolved, or deferred with
D8's record); whatever the trace could not settle; the order-list questions the trace answered and
any it did not; and the `sync` table's unreachable domain.

- [ ] **Step 4: The record**

`PROGRESS.md`'s phase row and its evidence section; `docs/decisions.md` for anything that changed
the approach (D26 and whatever else the phase earned); `docs/fidelity/music.md` finished;
`APPROXIMATIONS.md` showing A7 retired and any new entries.

- [ ] **Step 5: The whole-branch review, then the tag**

The final review runs before `git tag phase-3` (Phase 1's and Phase 2's practice: a tag that has to
be moved is worse than a tag applied a few commits later), and its findings are applied before the
tag. Then merge to `main` and push, as Phase 2 did — at the user's instruction, since the merge and
push are theirs to ask for.
