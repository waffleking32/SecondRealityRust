# Phase 2 — Shell + easy tier · Implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to
> implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the port play parts, not just the test card: a shell that starts at the original's
own block entry points and sequences ported parts, with **BEGLOGO frame-diff-identical** to its
reference and **TUNNELI, ENDLOGO, CRED and ENDSCRL** running their own effects to their own ends.

**Architecture:** The shell lives in `bins/secondreality` (`shell.rs`), driving `sr-engine`'s
`Engine` across a list of parts and keeping **one frame counter in the reference's own boot-index
space**, so `xtask compare` needs no translation and a diff needs no offset table. Each part is a
`Part` impl in `bins/secondreality/src/parts/`, translated from its original source, reading its
media through `sr-assets`. `sr-engine` takes exactly one structural change — the view origin
widens to `u32` (D16's open item, which CRED and ENDSCRL both trip) — plus whatever a part needs to
be expressible at all. Music is **not** in this phase: every music gate is replaced by its
**measured duration** and registered in `docs/fidelity/APPROXIMATIONS.md`, for Phase 3 to replace.

**Tech stack:** Rust 2021, `rstest` for table tests, `serde` for manifests — all already
dependencies. No new crate: the shell is a module in the binary, the parts are modules beside it.

**Spec:** `docs/spec/2026-10-10-second-reality-rust-design.md` — §4.2 (amended by Task 3), §5
(the copper, which this phase starts honouring for real), §6.2 (corrected by Task 1), §7.4 and D8
(`_np_zinfo`, whose dependents this phase's parts turn out not to include), §8.2, §9, §10.

## Global Constraints

- **`sr-engine` keeps zero runtime dependencies**, asserted by a test. The shell, the parts and
  SDL live in the binary.
- **Port the algorithm, not a resemblance.** Every part is a translation of its original source;
  every claim about the original cites `file:line`, in the commit message and in the part's
  `docs/fidelity/<part>.md` note, which ships in the same commit as the part.
- **Assets are read in their original formats**, from `assets/`, through `sr-assets`. No
  pre-conversion, no re-encoding, no cleaned-up copy.
- **`reference/` and `upstream/` are input-only.** Nothing a task writes may land inside them —
  `compare`'s guard is the precedent.
- **6-bit palette internally**, widened only at upload by `dac_expand`.
- **One tick is one frame.** The shell counts ticks at the engine's fixed 70.086 Hz; a part's own
  frame loop is its `Part::frame`, called once per tick. Nothing in the port spins or sleeps to
  match a frame.
- **A comparison is exact.** `compare`'s exit code is the verdict; no pixel tolerance, no widened
  window, no structural similarity. A part that cannot be diffed says so and says why.
- **Preserve the original's bugs** (`docs/fidelity/bugs.md`). A faithful port of a part whose
  screen the oracle cannot reproduce **will** differ; the reference row is what says so.
- **Music is deferred, not stubbed.** A gate that a Phase 3 cue will open is replaced by the
  **measured frame** at which the reference shows it opening, and registered in
  `docs/fidelity/APPROXIMATIONS.md` with its measurement. No audio is decoded, mixed or played in
  this phase.
- **Every ported part is verified against the reference tree**, and the result — exit code,
  offsets, percentage — is recorded in `PROGRESS.md`, exit 0 or not.

## Review Focus

Five failure modes this phase is most likely to ship, each with the task that pins it:

1. **A part whose frame index drifts from the reference's.** The port's numbering has to agree
   with a boot-relative emulator frame counter within ±1 (`ALIGNMENT_WINDOW`), and every measured
   constant in this plan — BEGLOGO's fade start 446, ENDLOGO's two fade starts, ENDSCRL's ring
   phase — is an index in that space. A one-frame error in any of them shifts a whole part, and the
   fades are where it shows first: one step of a 129-step ramp is a visible palette difference at
   the keyframes even though it is invisible in the ramp's first few frames. Pinned in Task 1 (the
   numbering), Task 4 and Task 6 (the constants and their derivations), Task 11 (the chain's
   residual).
2. **A palette ramp that is right to the eye and wrong in the last bit.** Both fades are integer
   arithmetic with a truncating divide — `((128-c)*63+palette[a]*c)/128` (`BEG/BEG.C:82`) — over
   6-bit entries, and the 6→8-bit widening at upload is load-bearing for RGB comparisons.
   Reproducing the ramp with floats, or rounding where the original truncates, changes the picture
   in the ramp's last third and nowhere else. Pinned in Task 4 (BEGLOGO) and Task 6 (ENDLOGO).
3. **The white screen's entry 255.** BEGLOGO fills DAC entries 0–254 with white and writes
   **black** into 255 (`BEG/BEG.C:26-34`); ENDLOGO writes 765 bytes, which fills 0–254 and
   **leaves 255 unwritten** (`END/END.C:62-63`, `:90-91`). The two produce the same white — both
   references are the one `sha256` `85789170…` — and a screen that "looks white" with a different
   255 is a diff. Pinned in Task 4 and Task 6.
4. **An origin that wraps.** `View`'s origin sums `start_addr + x_offset` and the beam reads
   `(origin + ly*stride + x) % surface.len()`. CRED's display origin is 128,000 pixels and
   ENDSCRL's walks to 256,000 — both beyond the `u16` the spec declares, and both wrap *silently*
   into a plausible-looking wrong picture. Pinned in Task 3 (the widening and its wrap test).
5. **A measured constant that was fitted to the test rather than derived from the evidence.** The
   lead-in constants are reverse-engineered from the reference's own frames, and a value chosen to
   make a keyframe match is not evidence. The required method is the one Task 4 Step 1 states — a
   **bit-exact search over the ramp's whole range**, required to be *unique*, with the source's own
   arithmetic deciding which `c` is which and the derivation written into the part's note — and a
   fit over mean brightness is explicitly **not** it (it gives 441 where the match gives 446, and
   nothing in the fit's residuals says so). Pinned in Task 4 and Task 6.

---

## Part A — the shell

### Task 1: Block entry, part sequencing, and the reference's frame numbering

The port has no shell: `Engine::start` takes one part, `Runner` owns one part, and the only
non-test `Part` is `TestCard` (`crates/sr-engine/src/engine.rs:249-256`, `testcase` aside). The
reference tree's indices are **boot-relative emulator frames** (`reference-indices.md` §1.12), and
the port's dump index is *ticks since this part started* (`bins/secondreality/src/headless.rs:84`).
Until those two numbering schemes are one, no reference comparison in this phase can be run at all.

**Files:**
- Create: `bins/secondreality/src/shell.rs`
- Modify: `bins/secondreality/src/cli.rs`, `main.rs`, `headless.rs`, `parts.rs`
- Test: `bins/secondreality/tests/shell.rs` (new), `bins/secondreality/src/cli.rs` (unit tests)

**Interfaces:**
- Consumes: `sr_engine::{Engine, Part, PartStatus, IndexedFrame, View}`; `Engine::new(surface_width,
  surface_height, view) -> Engine` (`engine.rs:60`), `Engine::start(&mut self, part: &mut dyn Part)
  -> Result<(), EngineError>` (`:99`), `Engine::tick(&mut self, part: &mut dyn Part, out: &mut
  IndexedFrame) -> PartStatus` (`:140`).
- Produces:
  - `pub const BLOCKS: &[(char, u8)]` — the original's `SECOND n` argument to the `partsmask` byte
    it selects (`MAIN/U2.ASM:110-120`, `:595-630`; `docs/fidelity/part-sequence.md` §4).
  - `pub fn boot_anchor(part: &str) -> Option<u32>` — the reference index of a part's first drawn
    frame, or `None` when the evidence does not give one (see the table below).
  - `pub struct Sequence` with `new(surface_width, surface_height, view, plan) -> Result<Sequence,
    ShellError>`, `fn tick(&mut self) -> SequenceStatus`, `fn frame(&self) -> &IndexedFrame`,
    `fn index(&self) -> u32` (the reference index of the frame just produced), `fn part(&self) ->
    &'static str`, `fn index_is_reference(&self) -> bool`.
  - `pub enum SequenceStatus { Continuing, Finished }` — `Finished` when the run's parts are all
    retired, or when the next part in the block is not ported.
  - `pub enum ShellError { UnknownPart { name: String }, NotPorted { name: &'static str }, Engine(...) }`.

- [ ] **Step 1: The anchor table, with its evidence**

`boot_anchor` returns the reference index at which a part's drawing begins. It is the only place
the port states *where a part begins* (the lead-in constants of Tasks 4 and 6 are different things:
they are offsets inside a part, and they live in the part), so every entry carries its citation:

| part | anchor | why it is this number |
|---|---|---|
| `beglogo` | **404** | The demo's first graphics frame under the D17 recipe — the shell's `mov ax,13h / int 10h` + `inittwk` at `MAIN/U2.ASM:806-808`, measured 404 in five runs of five (`reference-indices.md` §1.12). BEGLOGO's own first act is the white fill, which is what the screen holds from there (§3.3 measures 410 and 429 byte-identical to 430). |
| `endlogo` | **404** | Same boot landmark, different block: `SECOND 5` is `partsmask[5] = 0x10` (`MAIN/U2.ASM:116`), the shell sets mode 13h + `inittwk` at `:906-908`, and ENDLOGO's white is up by 415 and holds (`reference-indices.md` §19). |

Anything not in the table has no anchor the evidence supports. BEGLOGO and ENDLOGO are both *block
entries*; the three mid-block parts (TUNNELI, CRED, ENDSCRL) begin after an unported predecessor or
a music gate, and their start frames are known only as windows (CRED's boundary is in (1200, 1240],
ENDSCRL's in (8840, 8940] — §20, §21). A run that starts at one of those parts is therefore
**not** reference-comparable, and the shell says so rather than inventing a number.

```rust
/// The reference index of `part`'s first drawn frame, or `None` when the
/// evidence does not give one.
pub fn boot_anchor(part: &str) -> Option<u32> { ... }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_block_entry_anchor_is_the_boots_first_graphics_frame() {
        // Both block entries the port can start from are anchored at 404:
        // the demo's first graphics frame under the D17 recipe.
        assert_eq!(boot_anchor("beglogo"), Some(404));
        assert_eq!(boot_anchor("endlogo"), Some(404));
    }

    #[test]
    fn a_mid_block_part_has_no_anchor_and_says_so() {
        // CRED starts in (1200, 1240] and ENDSCRL in (8840, 8940] — windows,
        // not frames. A number here would be a guess wearing a citation.
        assert_eq!(boot_anchor("cred"), None);
        assert_eq!(boot_anchor("endscrl"), None);
    }
}
```

- [ ] **Step 2: `Sequence` — the runner that keeps one index**

`Sequence` owns an `Engine`, an `IndexedFrame`, the list of parts still to run and a `u32` index.
Per `tick()`: if there is no live part, take the next one, `Engine::start` it, and — if the run
began at an anchored part — set the index to that anchor; then `Engine::tick`. The index advances
by one per tick, and the picture is **not** cleared between parts: the original's parts are separate
EXEs that leave the screen as it is, and BEGLOGO's keyframe 900 is that fact (`reference-indices.md`
§3.5 — the hold belongs to the loader and the next part's music wait, not to BEGLOGO's loop).

```rust
#[test]
fn the_index_starts_at_the_anchor_and_advances_one_per_tick() {
    let mut run = Sequence::for_block('2', &["beglogo"]).unwrap();
    assert_eq!(run.index(), 404);
    run.tick();
    assert_eq!(run.index(), 405);
}

#[test]
fn a_part_with_no_anchor_numbers_its_own_frames_and_says_they_are_not_the_references() {
    let run = Sequence::for_part("tunneli").unwrap();
    assert_eq!(run.index(), 0);
    assert!(!run.index_is_reference());
}

#[test]
fn a_retired_part_hands_the_screen_to_the_next_without_clearing_it() { ... }

#[test]
fn a_block_run_stops_at_the_first_part_the_port_cannot_run() {
    // `SECOND 2` runs BEGLOGO then GLENZ; GLENZ is not ported, so the run
    // ends after BEGLOGO rather than silently skipping to TUNNELI and
    // producing frames in a numbering nothing supports.
    let mut run = Sequence::for_block('2', &["beglogo"]).unwrap();
    while run.tick() == SequenceStatus::Continuing {}
    assert_eq!(run.part(), "beglogo");
}
```

- [ ] **Step 3: `--block` and `--dump-frames` on the command line**

`--block <1|2|3|4|5|U>` selects the original's entry point, and the parts it runs are the ported
ones in `part-sequence.md` §1's order from that block's first part. `--dump-frames 430,470,560,900`
dumps **exactly those reference indices** — the alternative, `--frames N`, dumps every tick from
zero, which for BEGLOGO's 900 means 900 frames of 320×400 (345 MB of `.rgb`) to answer four.

```rust
#[test]
fn dump_frames_parses_a_sparse_list_and_refuses_a_bad_one() {
    let cli = Cli::parse(["--headless", "--dump-dir", "d", "--dump-frames", "430,470,560,900"]
        .map(String::from)).unwrap();
    assert_eq!(cli.dump_frames, Some(vec![430, 470, 560, 900]));
    assert!(Cli::parse(["--headless", "--dump-dir", "d", "--dump-frames", "430,,470"]
        .map(String::from)).is_err());
}
```

`--dump-frames` and `--frames` are mutually exclusive, and a requested index below the run's start
is refused with a message naming the anchor — an index the run can never reach is a mistake in the
command, not an empty dump.

- [ ] **Step 4: The dump's manifest carries the numbering it used**

`headless.rs`'s manifest (`:190-208`) gains `"index_origin": "boot" | "part"` and `"anchor": u32 |
null`, so a rendered frame set says *which* numbering its indices are in — the failure this field
prevents is a diff of two captures whose indices look comparable and are not. `compare` ignores
unknown fields (`compare.rs:349-351`), so nothing downstream changes.

```rust
#[test]
fn a_dump_manifest_says_which_numbering_its_indices_are_in() {
    let manifest = run_headless(&["--part", "testcard", "--dump-frames", "0", "--headless"]);
    assert_eq!(manifest["index_origin"], "part");
    assert_eq!(manifest["anchor"], serde_json::Value::Null);
}
```

- [ ] **Step 5: Correct spec §6.2's `dis_waitb` claim, in `docs/decisions.md`**

The source says the opposite of the spec. `waitb_1` (`DIS/DISINT.ASM:161-170`) waits for
`copperframecount` to change, then **returns the new value and zeroes it** — so a part whose work
overran *k* frames gets *k+1*, not 1; the reset happens after the read, not before. Spec §6.2
("the counter reset discards the overrun") is wrong, and the parts' own pacing depends on the real
semantics: `TUNNELI/TUN10.PAS:200` is `for sync := 1 to frames`, `ENDSCRL/MAIN.C:47` is
`if(dis_waitb()==1) dis_waitb();`. Record it as a decision with the two call sites, and state the
port's model: one tick is one frame, a part's `frame()` is called once per tick, so the value is
always 1 and the overrun case cannot arise — but a part that guesses wrong about it is wrong about
how many animation steps to take per tick. Amend §6.2 in place with a pointer to the decision.

- [ ] **Step 6: The DAC starts where the hardware starts**

A run's first frames are drawn through **the palette the machine already had**, and one reference
keyframe is exactly that: BEGLOGO's 430 is index 15 over the *inherited* DAC — BEGLOGO writes its
own white only after the 32 vertical blanks (`BEG/BEG.C:24` then `:25-34`), so frame 430's entry 15
is the value a VGA mode set leaves, `(63,63,63)`. A shell that starts from a black palette renders
430 black and fails a keyframe before the part does anything. The shell therefore initialises the
DAC to the emulator's own mode-13h default table (`vga_memory.cpp`'s default palette — the port's
only oracle for what the hardware powers up with), and says so where it does it.

```rust
#[test]
fn the_shell_starts_from_the_hardwares_default_palette() {
    let run = Sequence::for_part("testcard").unwrap();
    // Entry 15 is white in the VGA's default table, which is what BEGLOGO's
    // frame 430 shows before the part has written a single DAC entry.
    assert_eq!(run.palette().entry(15), [63, 63, 63]);
}
```

- [ ] **Step 7: Commit**

```bash
git add bins/secondreality/src/shell.rs bins/secondreality/src/cli.rs \
        bins/secondreality/src/headless.rs bins/secondreality/src/parts.rs \
        bins/secondreality/src/main.rs bins/secondreality/tests/shell.rs docs/decisions.md
git commit -m "Add the shell: block entry, part sequencing, and the reference's frame numbering"
```

### Task 2: Make the reference record machine-readable, and verify it

Carried into this phase by Phase 1's close-out, in its own words: the reference tree's bytes are
pinned by `docs/fidelity/reference-indices.md` alone, no test reads `reference/`, and
`compare` of the tree against itself can catch a *missing or truncated* file but never **a byte
changed in place** — the one failure nothing in the tree reports. The fix is to write the digests
where they are produced, then walk the tree against them.

**Files:**
- Modify: `xtask/src/reference.rs` (the manifest's frame entries), `xtask/src/main.rs`, `xtask/src/compare.rs` (the tree walk is the rule to reuse)
- Create: `xtask/src/verify.rs`
- Test: `xtask/src/verify.rs` (unit), `xtask/tests/verify.rs` (integration)
- Modify: `docs/fidelity/reference-indices.md` (a line in §1.12 pointing at the command)

**Interfaces:**
- Consumes: `reference.rs`'s `CaptureManifest`/`FrameRef` (`:398`, `:419`), `compare.rs`'s
  `resolve_captures` walk (`:420-460`) and `MANIFEST_VERSION` (`:56`).
- Produces: `xtask reference verify --tree <DIR>` → `ExitCode` 0 when every manifest's frames hash
  as recorded, 1 on any mismatch, 2 when it could not check; `FrameRef` gains
  `#[serde(default, skip_serializing_if = "Option::is_none")] pub sha256: Option<String>`.

- [ ] **Step 1: `sha256` per frame, written at capture time**

`sha256sum` in the shell is how Phase 1 hashed by hand; the tool must do it itself. A small
SHA-256 implementation goes in `xtask` (it is build tooling, not the port, so a crate is allowed —
but the tree has no hashing crate today and `sha2` is the only one worth adding; if the
implementer prefers no new dependency, `xtask` may shell out to `sha256sum` **only** in the
verifier, never in `capture`, which must produce the manifest on machines without coreutils).
Record the choice in the commit message.

```rust
#[test]
fn a_captured_frame_carries_its_digest() {
    let manifest = serde_json::to_value(&sample_manifest()).unwrap();
    assert!(manifest["frames"][0]["sha256"].as_str().unwrap().len() == 64);
}
```

- [ ] **Step 2: Backfill the tree once, mechanically**

Every manifest under `reference/parts/` (22 captures) and `reference/check_a` gets its digests —
including the frames that were filed before this task. `version` stays 1 (the field is additive and
`serde` defaults it), so old readers keep working. A one-shot `cargo xtask reference backfill
--tree reference` subcommand does it, is idempotent, and refuses to write into a tree it cannot
read. **It must not touch a frame file** — a backfill that rewrote a frame would destroy the thing
it is protecting, so the test writes a capture, runs the backfill, and asserts every frame file's
bytes are unchanged.

**This is the one task in the project allowed to write inside `reference/`, and the exception is
narrow and evidenced.** The input-only rule protects the *frames*; this writes metadata beside
them. Three conditions, all of them the task's deliverable: the frames are hashed **out of band**
first (`sha256sum` in the shell, not through the tool — `find reference -name 'frame_*' -exec
sha256sum {} + | sort` before and after, and the two lists must be identical); the backfill is
idempotent (a second run changes no byte of any file); and the digests it writes are checked
against the *pre-change* out-of-band list, not against its own work. Without the first and third,
the field would be self-certifying, which is worse than absent.

- [ ] **Step 3: `xtask reference verify`**

Walks the tree the way `compare` does (any depth, a `manifest.json` marks a capture), and for each
capture checks three things, reporting each failure with the file named:

1. every frame the manifest names exists, with the length its geometry implies;
2. every `frame_*` file in the directory is named by the manifest (**a file the manifest does not
   name is a failure**, not a curiosity — that is what a stray edit looks like);
3. every digest matches the file's bytes.

Exit 0 all good, 1 any finding, 2 could not check. Run it against `reference/`: it must exit 0, and
the transcript goes in `PROGRESS.md`. Then demonstrate it fails: copy a capture to `out/`, flip one
byte in one frame, and show exit 1 naming that file — the negative half is the point of building
it, and a verifier that has never failed is untested.

- [ ] **Step 4: Commit**

```bash
git add xtask/src/reference.rs xtask/src/verify.rs xtask/src/main.rs xtask/tests/verify.rs \
        reference/ docs/fidelity/reference-indices.md
git commit -m "Record every reference frame's digest where it is written, and verify the tree against it"
```

### Task 3: `sr-engine` — the view origin widens to `u32`

D16's open item, deferred from Phase 0 with the words "Phase 2 must settle it when ENDSCRL lands".
It lands here, and CRED trips it too: its display origin is 400 logical lines × 320 px = **128,000**,
and ENDSCRL's walks to **256,000**. Both exceed the 131,070 a `u16` pair can sum to. The port's
`View` is a pixel-addressed model, so the field widens; the alternative — modelling the CRTC's
per-plane byte counter — would put planar addressing into an engine that deliberately does not have
it.

**Files:**
- Modify: `crates/sr-engine/src/view.rs`
- Modify: `docs/spec/2026-10-10-second-reality-rust-design.md` §4.2, `docs/decisions.md` (D16's open item closes)
- Test: `crates/sr-engine/src/view.rs` (unit tests)

**Interfaces:**
- Produces: `View::start_addr: u32`; `View::set_origin(&mut self, pixel: u32)` sets `start_addr`
  and zeroes `x_offset`; `View::source_index` unchanged in shape, now summing in `usize`.
  `x_offset` stays `u16` — it is a pan within a line, bounded by the widest line the engine
  supports.

- [ ] **Step 1: Write the failing tests first**

```rust
#[test]
fn an_origin_beyond_u16_is_not_wrapped_away() {
    // ENDSCRL's ring: the beam starts at the 400th 640-pixel line.
    let mut v = View::new(640, 400);
    v.set_origin(256_000);
    let s = Surface::new(640, 400);
    // 256,000 % 256,000 == 0: the beam wraps to the top of the ring, which is
    // what the surface of 400 ring lines means.
    assert_eq!(v.source_index(&s, 0, 0), 0);
    v.set_origin(255_360);
    assert_eq!(v.source_index(&s, 0, 0), 255_360);
}

#[test]
fn creds_origin_reads_every_other_logical_line() {
    // CRED displays 200 rows from logical line 400 with a 160-byte line
    // offset: row ly starts at line 400 + 2*ly of a 320-px logical line.
    let mut v = View::new(320, 200);
    v.set_origin(400 * 320);
    v.y_line_offset = Some(640);
    let s = Surface::new(640, 400);
    assert_eq!(v.source_index(&s, 0, 1), 400 * 320 + 640);
    assert_eq!(v.source_index(&s, 319, 199), 400 * 320 + 199 * 640 + 319);
}
```

- [ ] **Step 2: Widen the field, amend §4.2, close D16's item**

The spec's §4.2 declares `start_addr: u16`; amend it in place with the reason (one paragraph), and
add a decision entry that closes D16's open item, records the arithmetic that forced it (CRED
128,000, ENDSCRL 256,000), and says what the `u16` was protecting against (nothing — the fields
were copied from the CRTC's 16-bit registers, which count *bytes within a plane*, not pixels).

- [ ] **Step 3: Run everything, including the test card**

`cargo test --workspace`. The test card exercises `set_origin` and the split walk (`testcard.rs`,
`SPLIT_WALK_ROWS` derived from `u16::MAX`); the constant's doc comment must be updated to whatever
it becomes, and the card must still pass unchanged in behaviour.

- [ ] **Step 4: Commit**

```bash
git add crates/sr-engine/src/view.rs docs/spec/2026-10-10-second-reality-rust-design.md docs/decisions.md
git commit -m "Widen the view origin to u32: CRED and ENDSCRL both need more than a CRTC register holds"
```

---

## Part B — the parts

Every part task follows the same shape, and the shape is the reviewer's checklist: **read the
original** (the files are named per task), **translate the algorithm** (fixed-point arithmetic,
tables and rounding included), **write the part's fidelity note** (`docs/fidelity/<part>.md`) in the
same commit, and **run the comparison** against the part's reference with the result recorded in
`PROGRESS.md`.

Two conventions are shared by all five and are stated once here:

- **The part draws through `Surface` in the port's pixel units.** `Surface::set(x, y)` is
  row-major at the surface's own width (640), which is what a part whose picture is the *display*
  wants: a 320-wide view over a 640-wide surface reads the left half of each row, un-sheared, with
  `y_line_offset` left `None`. A part whose original addressed video memory through its own
  arithmetic (CRED packs 320-pixel logical lines; ENDSCRL's ring is 640-pixel lines) writes
  **flat** through `Surface::pixels_mut()` at its own `LINE_LEN`, and sets `y_line_offset` to the
  display's stride in pixels. Both conventions are traps in opposite directions — the first shears
  if the stride is set to the view's width, the second shears if it is left unset — so each part's
  note states which it uses and why.
- **A music gate is a measured frame.** The part's own structure is translated as written, and the
  gate is one constant with its derivation in the note. Phase 3 replaces the constant with a cue.

### Task 4: BEGLOGO — the phase's exit criterion

The Future Crew logo, white, fading in from white over a held picture, 320×400 Mode X. **70 lines
of C** (`BEG/BEG.C`) over `BEG/ASM.ASM` (`inittwk`, `lineblit`, `setpalarea`) and `BEG/READP.C`
(the Origin RLE row reader). It is the whole of BEGLOGO:

```c
dis_partstart();                                  // BEG.C:17
outp(0x3c4,2); outp(0x3c5,15);                    // :18-19  chain-4 off
memset(vram,15,32768); memset(vram+32768,15,32768);  // :20-21  the white field
for(a=0;a<32;a++) dis_waitb();                    // :24     the lead-in
outp(0x3c8,0); for(a=0;a<255;a++) { 63,63,63 }    // :25-31  DAC 0..254 white
outp(0x3c9,0); outp(0x3c9,0); outp(0x3c9,0);      // :32-34  entry 255 black
... CRTC 0x0c/0x0d = 0, CRTC 9 &= ~0x80, &= ~0x1f, AC 0x11 = 0, 0x12 = 0x20 ...  // :40-71
readp(palette,-1,pic); for(y=0;y<400;y++) { readp(rowbuf,y,pic); lineblit(vram+y*80,rowbuf); }  // :73-78
for(c=0;c<=128;c++) { for(a=0;a<768-3;a++) pal2[a]=((128-c)*63+palette[a]*c)/128;
                      dis_waitb(); setpalarea(pal2,0,254); }   // :80-85
setpalarea(palette,0,254);                        // :86  the final palette
```

**Files:**
- Create: `bins/secondreality/src/parts/beglogo.rs`, `docs/fidelity/beglogo.md`
- Modify: `bins/secondreality/src/parts.rs` (the registry)
- Test: `bins/secondreality/tests/beglogo.rs`
- Read: `upstream/SecondReality/BEG/BEG.C`, `BEG/ASM.ASM`, `BEG/READP.C`,
  `reference-indices.md` §3, `docs/fidelity/bugs.md`

**Interfaces:**
- Consumes: `sr_assets::{load_path, Image}` for `assets/BEG/SRTITLE.UP`; `Part`, `PartCtx`
  (`crates/sr-engine/src/part.rs:41-77`); `Palette::set_entry` (`palette.rs:72`) and
  `PartCtx::set_palette` (`part.rs:107`).
- Produces: `pub struct Beglogo` with `pub fn new() -> Beglogo`, registered as `"beglogo"`.

- [ ] **Step 1: Derive the fade's first index by matching the reference bit for bit, and write the
  derivation down**

The constant is the frame the fade's step `c = 0` is *displayed* on, and it is derived by
**exhaustive match, not by fitting**: for each fade keyframe, compute the ramp `c` from the source's
own arithmetic (Step 3), widen it with the DAC rule, and find the `c` whose 320×400 RGB equals that
frame **exactly**. The search over `c = 0…128` finds a **unique** value for each frame — 470 at
`c = 24`, 560 at `c = 114`, and 900 at `c ∈ {126, 127, 128}` (equal to the finished palette), with
430 matching only at `c = 0`. That is what fixes the constant at **446**: `446 + 24 = 470`,
`446 + 114 = 560`.

A fit over `reference-indices.md` §3.3's mean-difference column gives ≈441 instead, and it is
wrong — a mean is not linear in `c` while the palette's entries are, and a five-frame fit cannot
tell a whole step from the truncation in it. **The note records the scan, not a fit**, and the
residual the scan cannot resolve (c ∈ {126,127,128} at 900 — the ramp has converged) is stated as
resolved by the picture, not by arithmetic.

The code's constant is named for what it is:

```rust
/// The reference index the fade's step `c = 0` is displayed on.
///
/// Derived in `docs/fidelity/beglogo.md` by matching each fade keyframe's RGB
/// bit for bit: 470 is the ramp at c = 24 and 560 at c = 114, uniquely, which
/// puts c = 0 at 446.
///
/// The part spends 32 vertical blanks (`BEG/BEG.C:24`) and a 400-row RLE
/// decode and plane blit (`:73-78`) before the fade, and the loader spends
/// frames loading the EXE. The port decodes once, in one call, so all of that
/// is inside this constant rather than a cost the port re-pays:
/// `dis_partstart()` (`:17`) is a cue the port does not model (Global
/// Constraints) and lands here too.
const FADE_FIRST_INDEX: u32 = 446;
```

- [ ] **Step 2: Settle which BEGLOGO build shipped, and correct the two documents that guess wrong**

`part-sequence.md` and `asset-inventory.md` leave it open which of `BEG/` and `ENDPIC/` built the
shipped `BEGLOGO.EXE`, and it matters: the two have different pictures, different palettes, and
only ENDPIC's has a music wait. The evidence settles it, decisively and without unpacking the
binary: at 900 the fade has converged, so the displayed DAC *is* the picture's palette, and
`reference/parts/beglogo/manifest.json`'s probe of 900 lists entries whose values equal
`BEG/SRTITLE.UP`'s palette byte for byte and none of ENDPIC's — decoding `SRTITLE.UP` and widening
reproduces `frame_000900.idx` exactly (0 of 128,000 pixels), while ENDPIC's palette mismatches all
128,000. The staged `MAIN/DATA/BEGLOGO.EXE` is 30,874 bytes, which no packer could have produced
from ENDPIC's 13,286-byte build. **`BEG/` shipped** — record it in `part-sequence.md`'s build note
and in the part's note.

**And correct the spec and D8, which inherited the wrong build's dependency.** Spec §7.4 and D8 say
"BEGLOGO waits for `zinfo == 0xf0`"; that line is `ENDPIC/BEG.C:48`, in the build that did not ship.
`BEG/BEG.C` calls `dis_partstart`, `dis_waitb` and nothing else — no `dis_muscode`, no `dis_exit`,
no copper. So `_np_zinfo`'s exposure narrows to GRID, HARD and the vault (Phase 8), and the phase
that has to resolve it is not this one. Amend §7.4 and D8 in place, with the evidence above.

- [ ] **Step 3: The tests that do not need the oracle**

```rust
#[test]
fn the_opening_white_is_index_15_over_the_palette_the_shell_left() {
    // BEG/BEG.C:20-21 fills A-space with index 15, and `:25-34` writes the DAC
    // only after the 32 waits — so the screen the reference captures at 430 is
    // index 15 through the *inherited* palette, where entry 15 is (63,63,63)
    // (Task 1 Step 6). The part must not clear the DAC at start.
    let mut part = Beglogo::new();
    let mut ctx = ctx_of(320, 400);   // the shell's default DAC
    part.init(&mut ctx).unwrap();
    assert_eq!(ctx.surface.get(0, 0), Some(15));
    assert_eq!(ctx.surface.get(319, 399), Some(15));
    assert_eq!(ctx.palette.entry(15), [63, 63, 63]);
}

#[test]
fn the_fades_white_is_written_into_entries_0_to_254_and_255_is_black() {
    // `:25-34`: 255 iterations of (63,63,63) then one of (0,0,0). The
    // reference's 430 does not constrain entry 255 (nothing on screen uses
    // it); the *border* does, and the ramp does from the first step on.
    ...
    assert_eq!(ctx.palette.entry(254), [63, 63, 63]);
    assert_eq!(ctx.palette.entry(255), [0, 0, 0]);
}

#[test]
fn the_fade_ramp_truncates_where_the_original_truncates() {
    // ((128-c)*63 + pal*c)/128, integer division, over 6-bit entries. A float
    // version agrees on c=0 and c=128 and disagrees in the last third.
    let pal = [63u8, 40, 1];
    assert_eq!(step(&pal, 1)[0], ((127 * 63 + 63 * 1) / 128) as u8);
    assert_eq!(step(&pal, 64)[0], ((64 * 63 + 63 * 64) / 128) as u8);
}

#[test]
fn the_ramp_writes_entries_0_to_254_and_leaves_255_alone() { ... }

#[test]
fn the_picture_is_blitted_before_the_fade_starts() {
    // The screen at the first fade step is still white: the blit lands in
    // surface memory but the DAC shows entry 15 everywhere until c moves.
    ...
}
```

- [ ] **Step 4: The part, and the one bug it ships**

`init` does `:18-78` — chain-4 off, the white fill, the DAC, the CRTC and the AC, the picture
decode through `sr_assets::load_path("assets/BEG/SRTITLE.UP")` and the blit into the surface:
`load_path` already decodes the container `readp` reads row by row, and the decoded picture is
**byte-identical to `reference/parts/beglogo/frame_000900.idx`** (0 of 128,000 pixels), which is
the strongest statement available that the decoder and the art agree. `frame` is the fade: ticks
0..31 wait (the 32 `dis_waitb`s), then one `c` step per tick from `FADE_FIRST_INDEX`, the final
palette one tick after `c = 128`, then `PartStatus::Finished` — the original's `main()` returns
after `:86`, and everything the screen does afterwards belongs to the next part
(`reference-indices.md` §3.5). The `lineblit` is a *plane* routine in the original
(`BEG/ASM.ASM:63-76`): the port writes the picture's 320 pixels per row through `Surface::set` and
leaves `y_line_offset` unset, which is the same picture because the plane interleave is a hardware
detail the chunky model folds away — the note says so, with the citation.

**And `setpalarea` ships with a byte-count bug that must be preserved.** `BEG/ASM.ASM:93-115` sets
the transfer length to `2*count + start` bytes, not `3*count`, so `setpalarea(pal2,0,254)` uploads
**508** bytes — entries 0–168 plus the red byte of 169 — and entries 170–254 keep their old value
for the whole fade. The shipped binary contains exactly this (disassembled at `BEG.EXE` offset
`0xcbd`), and `END/ASM.ASM` and `JPLOGO/ASM.ASM` share it while `LENS`, `START` and `COMAN` do the
correct `3*count`. It is invisible on this art (the picture uses indices 1,2,3,14–58,71–79 and the
background is 15, all inside the written range), so **a port that writes the full 762 bytes matches
every capture** — which is exactly why it has to be a decision and not an accident: write the
508-byte behaviour, cite the disassembly, and add it to `docs/fidelity/bugs.md` as a bug the port
preserves (with the note that its first visible consequence will be in whichever part paints into
entries 170–254).

- [ ] **Step 5: Compare, and record it**

```bash
cargo run --bin secondreality -- --headless --part beglogo --dump-frames 430,470,560,900 \
    --dump-dir out/p2-beglogo
cargo xtask compare --reference reference/parts/beglogo --rendered out/p2-beglogo \
    --out out/p2-beglogo-diff
```

**Exit 0 is required.** Two of the four frames go through the RGB path (470 and 560 are RGB-only:
the indexed read is late by ~20 fade steps, `reference-indices.md` §1.3), so the DAC widening rule
is live here — expect `mode: mixed` and exactly that pair of readings. A diff at 470 or 560 is a
ramp or an index error; a diff at 430 or 900 only is a setup error. Record the exit code, the
offsets and the percentage in `PROGRESS.md`.

- [ ] **Step 6: Commit**

```bash
git add bins/secondreality/src/parts/beglogo.rs bins/secondreality/src/parts.rs \
        bins/secondreality/tests/beglogo.rs docs/fidelity/beglogo.md docs/fidelity/bugs.md \
        docs/spec/2026-10-10-second-reality-rust-design.md docs/decisions.md \
        docs/fidelity/part-sequence.md
git commit -m "Port BEGLOGO: the logo, the white, and the 129-step fade"
```

### Task 5: TUNNELI — the dot tunnel

77 concentric rings of 64 dots, chain-copied outward, one plot per dot per frame with an
`oldpos`-based erase, for `veke = 1060` frames. Turbo Pascal (`TUNNELI/TUN10.PAS`, 227 lines) with
an inline-assembly inner loop; the shipped build is TUN10 (`TUNNELI/X.BAT:1` stages
`tun10.exe` as `tunneli.exe`), and its data is `TUNNELI/SINIT.DAT` (the sine tables) — both
already curated in `assets/TUNNELI/`.

**This part cannot be diffed, and the task says so up front.** Its reference has no `.idx` at all
(41 of 43 frames refused as "the screen moved while it was read", §8.2), all four keyframes move,
five sessions hold **three distinct pictures** at 3800, and 4500 and 4600 are formally recorded as
**no settled content** (§1.8, §2's row). A TUNNELI frame diff measures the oracle's spread, not the
port. What the task pins instead is what the *source* pins.

**Files:**
- Create: `bins/secondreality/src/parts/tunneli.rs`, `docs/fidelity/tunneli.md`
- Modify: `bins/secondreality/src/parts.rs`
- Test: `bins/secondreality/tests/tunneli.rs`
- Read: `upstream/SecondReality/TUNNELI/TUN10.PAS` (and `TUN9.PAS` for what changed),
  `docs/fidelity/asset-inventory.md` (TUNNELI's rows)

**Interfaces:**
- Consumes: `sr_assets::raw_surface`/`load_path` for `assets/TUNNELI/{TUNNEL.DAT,SINIT.DAT}`;
  `PartCtx::{surface, palette, frame}`.
- Produces: `pub struct Tunneli`, registered as `"tunneli"`.

- [ ] **Step 1: Tests pinned to the source, not the oracle**

```rust
#[test]
fn the_tunnel_draws_seventy_seven_rings_of_sixty_four_dots() { ... }   // TUN10.PAS:153

#[test]
fn the_ring_chain_copies_the_last_ring_forward() {
    // :204 `move(putki[1], putki[0], ...)`: every ring but the newest is the
    // previous one's data, six bytes further along.
    ...
}

#[test]
fn the_animation_advances_once_per_tick() {
    // :200 `for sync := 1 to frames` with `frames` from waitr (fn 1). One
    // tick is one frame (Global Constraints), so this is one step.
}

#[test]
fn the_last_hundred_and_two_frames_draw_in_colour_zero() {
    // :210 `if frame >= veke-102 then putki[99].c := 0` — the tunnel fades out
    // by drawing its dots in the black entry, not by touching the palette.
    let part = ...;  // frame set past veke-102
    assert!(dots_are_all(0));
}

#[test]
fn the_part_retires_after_veke_frames() { ... }   // :3 `veke = 1060`, :212
```

- [ ] **Step 2: The part, and the one piece of self-modifying code**

The inner loop patches its own colour immediate (`mov byte ptr cs:[@c+3],al`, `TUN10.PAS:168`); the
port makes it a variable, which is what spec §429 says to do with it. The clip is
`cmp di,319 / ja @yli` on the x sum only, with the y term indexing `rows[bx*2]` unchecked
(`:179-190`) — preserve that shape: a dot whose y is out of range is *not* clipped in the original,
and a port that adds a y bound draws one fewer artefact than the original does. `waitr` rewrites
DAC entry 0 each frame from the BIOS shift flags (`:52`) — the port has no keyboard flags yet, so
entry 0 is written from a value the port owns and the note records what the original keyed it to.

- [ ] **Step 3: Run it, record what it looks like, and compare anyway**

```bash
cargo run --bin secondreality -- --headless --part tunneli --dump-frames 900,1060,2000 \
    --dump-dir out/p2-tunneli
cargo xtask compare --reference reference/parts/tunneli --rendered out/p2-tunneli \
    --out out/p2-tunneli-diff     # expected: exit 2, indices the run cannot answer; and if run
                                  # at the reference's own indices, exit 1 with the oracle's spread
```

The evidence for this part is a **contact sheet a human looks at** (`xtask reference capture
--sheet` renders one from any frame directory; the port's dump is a frame directory) beside the
reference's own sheet, plus the recorded comparison. The note names the reference's §8.2 and §1.8
findings as the reason the diff is not a verdict.

- [ ] **Step 4: Commit**

```bash
git add bins/secondreality/src/parts/tunneli.rs bins/secondreality/src/parts.rs \
        bins/secondreality/tests/tunneli.rs docs/fidelity/tunneli.md
git commit -m "Port TUNNELI: 77 rings, one plot per dot per frame, and the original's clip"
```

### Task 6: ENDLOGO — the end picture

White, a 129-step fade-in of the FUTURE/CREW picture, a music-gated hold, a 64-step fadeout, in
320×400 Mode X whose mode the **shell** sets (`MAIN/U2.ASM:906-908`: `mov ax,13h / int 10h /
inittwk`). `END/END.C` is 130 lines over `END/ASM.ASM` (`lineblit`) and `END/READP.C`. Its picture
is `assets/END/PIC.UH`.

**Files:**
- Create: `bins/secondreality/src/parts/endlogo.rs`, `docs/fidelity/endlogo.md`
- Modify: `bins/secondreality/src/parts.rs`, `docs/fidelity/APPROXIMATIONS.md`
- Test: `bins/secondreality/tests/endlogo.rs`
- Read: `upstream/SecondReality/END/{END.C,ASM.ASM,READP.C}`, `reference-indices.md` §19

**Interfaces:**
- Consumes: `sr_assets::load_path` for `assets/END/PIC.UH`; `Part`, `PartCtx`.
- Produces: `pub struct Endlogo`, registered as `"endlogo"`.

- [ ] **Step 1: Derive the two fade constants, and write the derivations down**

Same method as Task 4, and the same warning: **match the reference bit for bit, do not fit its
means.** Read the two ramps out of `END/END.C` (`:101-106` the fade-in, `:112-117` the fade-out),
then find the step `c` whose widened palette reproduces `reference/parts/endlogo`'s frame at 500
and at 1180 exactly — each search over the ramp's whole range, each required to be unique. §19's
sweep means give the brackets to start from (the fade-in's progression over 460/500/540 puts its
`c = 0` near 450; the fade-out's single sample at 1160 puts its `c = 0` near 1145), and they are
brackets, not the answer.

Two independent facts then check the constants before they are used, and both are already in the
tree: the picture is byte-identical from 580 to 1120 (`reference-indices.md` §19.1), so the hold is
real and its length is the fade-out's index minus 578; and the geometry change to CRED is in
(1200, 1240] (§20), so a 64-step fade-out from the derived index has to end inside that window. If
the two disagree, the ramp has been misread — the same failure Task 4 Step 1's note warns about.

- [ ] **Step 2: The music gate becomes a measured duration, in the register**

`END.C:107-111` is `for(a=0;a<5000 && !dis_exit();a++) { dis_waitb(); if(dis_musplus()>-16) break; }`.
Write the APPROXIMATIONS entry: the gate is replaced by the hold's measured length, the measurement
is §19's sweep and the two constants above, the difference is **zero against the reference** and
unknown against any other music position, and Phase 3 replaces it with the real cue. The entry is
required, not optional — this is the one place in the phase where a faithful-looking port would be
waiting on something that does not exist.

- [ ] **Step 3: Tests, then the part**

```rust
#[test]
fn the_white_leaves_entry_255_unwritten() {
    // END.C:62-63 writes 765 bytes of 63 from index 0: entries 0..254, and
    // 255 keeps whatever the previous part left. That is what makes the two
    // 430s — BEGLOGO's and ENDLOGO's — the same bytes.
}

#[test]
fn the_picture_is_blitted_after_thirty_two_vertical_blanks() { ... }  // END.C:89-99

#[test]
fn the_fade_in_and_fade_out_are_the_same_ramp_reversed() { ... }     // :101-117

#[test]
fn the_hold_is_the_measured_length_and_not_five_thousand() { ... }   // the gate's stand-in
```

Two things the part does *not* do, and both are traps. **It does not set its own mode**: `END.C`'s
mode 13h and `inittwk` call are commented out (`END.C:64-66`) because the shell does it
(`MAIN/U2.ASM:906-908`) — so the port's ENDLOGO is a 320×400 part drawing through `Surface` with
`y_line_offset` unset, and the byte-mode CRTC write (`CRTC 0x17 = 0xE317`) has no analogue in a
chunky model and is recorded as such. **And its `setpalarea` is the same buggy routine BEGLOGO's
is** (`END/ASM.ASM:106-108`, the `2*count + start` length), so the fade uploads 508 bytes per
step, not 762 — the same preserved bug, the same `docs/fidelity/bugs.md` entry, and the same
reason a port that "fixes" it still matches every capture.

- [ ] **Step 4: Compare, record, commit**

```bash
cargo run --bin secondreality -- --headless --part endlogo --dump-frames 430,500,900,1180 \
    --dump-dir out/p2-endlogo
cargo xtask compare --reference reference/parts/endlogo --rendered out/p2-endlogo \
    --out out/p2-endlogo-diff
```

Exit 0 is the target; the reference is settled and reproducible (4/4 sessions at every keyframe,
`reference-indices.md` §19). 900 and 430 are held, 500 and 1180 are the two fades — a diff pair
that separates a ramp error from a hold-length error, and the task records which it got.

### Task 7: CRED — the font, the 21 pictures, and the screens

21 credit screens, each: clear, palette, text centred in proportional type, the picture laid in,
an iris open of ~127 frames, a 200-frame hold, an iris close of ~59 frames. `CREDITS/MAIN.C` is 318
lines over `CREDITS/TWEAK.ASM`, and the pictures are 21 LBMs already curated in
`assets/CREDITS/PICS/`. The font is **not** curated: `CREDITS/FONA.INC` (1,500×40) is missing from
`assets/`, and `sr-assets/src/font.rs` says so in its own doc.

**Files:**
- Create: `bins/secondreality/src/parts/cred.rs`, `docs/fidelity/cred.md`
- Modify: `assets/PROVENANCE.md`, `docs/fidelity/asset-inventory.md` (the font row),
  `bins/secondreality/src/parts.rs`, `docs/fidelity/APPROXIMATIONS.md`
- Test: `bins/secondreality/tests/cred.rs`
- Read: `upstream/SecondReality/CREDITS/{MAIN.C,TWEAK.ASM,FONA.INC}`, `reference-indices.md` §20

**Interfaces:**
- Consumes: `sr_assets::load_path` (the 21 LBMs), `sr_assets::font::Font::from_image(image, cell,
  first) -> Result<Font, AssetError>` (`font.rs:165`), `Font::glyph` (`:243`), `PartCtx`.
- Produces: `pub struct Cred`, registered as `"cred"`.

- [ ] **Step 1: Curate the font, and register what its rows do**

Copy `upstream/SecondReality/CREDITS/FONA.INC` into `assets/CREDITS/FONA.INC`, add its
`PROVENANCE.md` row and its inventory row, and record the divergence: the raster is 1,500×40 and
the part uses **32** rows (`CREDITS/MAIN.C:29` declares `FONAY 32`), which does not divide the
strip. Write **both** register entries now — CRED's and the one Phase 1's A7 left for ALKU
(1,500×30 with 32 rows named; the two extra rows are uninitialised linker padding, which is why the
loader's refusal is plan-mandated and harmless on this art, but the drawn-text difference belongs in
the register). A7's finding is carried here because this is where a font strip is next touched.

- [ ] **Step 2: The screens, without the wipe**

`screenin` (`MAIN.C:161-213`) minus the two wipes: `tw_clrscr`, `tw_setpalette` from the picture's
own palette, the text (the proportional metrics from the font strip's `fonap`/`fonaw` tables), the
picture via `tw_putpixel(400+x, 400+y*2, pic[y][x]+16)` over 160×100 (`:174`), and the 200-frame
hold. The `+16` is the picture's palette offset in the screen's 16-colour-plus ramp, and
`MAIN.C:273-317`'s `init()` — the palette repair and the ten-entry grey ramp — is part of the
screen, not setup: port it.

```rust
#[test]
fn the_fifth_screens_picture_is_the_second_half_of_a_repeated_source() { ... }   // pic5b, :41-156
#[test]
fn the_credit_text_is_centred_by_the_strips_own_metrics() { ... }
#[test]
fn the_hold_is_two_hundred_frames() { ... }                                     // :194
```

- [ ] **Step 3: The logical layout, stated once**

CRED's display is 200 rows of 320 pixels read from logical line `400 + 2*ly` with a 160-byte line
offset (`TWEAK.ASM:18-60`: CRTC 0x13 = `0x5013`, CRTC 9 = `0x0109`). In the port's pixel units that
is a **320-pixel logical line**, the part writing flat through `Surface::pixels_mut()` at
`LINE_LEN = 320`, and a view of `origin = 400*320 = 128_000`, `y_line_offset = Some(640)`,
`width = 320`, `height = 200`. **This is the layout Task 3's widening exists for**, and the note
must show the arithmetic: 160 planar bytes per displayed row = 2 logical lines of 80 bytes = 640
pixels at 4 pixels per byte.

- [ ] **Step 4: Commit**

```bash
git add assets/CREDITS/PROVENANCE.md assets/PROVENANCE.md docs/fidelity/asset-inventory.md \
        bins/secondreality/src/parts/cred.rs bins/secondreality/src/parts.rs \
        bins/secondreality/tests/cred.rs docs/fidelity/cred.md docs/fidelity/APPROXIMATIONS.md
git commit -m "Port CRED's screens: the font, the 21 pictures, and the palette repair"
```

### Task 8: CRED — the wipe

The iris is the part's whole visible motion and the reference says nothing about it: "the reference
says nothing about the 590 frames a screen spends opening and closing" (`reference-indices.md`
§20). Every keyframe is a *held* screen, so the wipes are verified by construction — three moving
registers, stepped once per vblank — and by the held frames they start and end on.

Per wipe step (`CREDITS/MAIN.C:176-192`, close identical at `:196-212`):

```c
dis_waitb();
tw_setsplit(y/128+200);          /* line compare: 400 down to 200 over the open */
yy = 320 - y/80;
for(a=0;a<10000;a++);            /* a delay loop the port does not reproduce */
tw_setstart(160*200 + yy/4);     /* CRTC 0x0C/0x0D */
/* AC 0x13 = (yy & 3) * 2 */     /* the pixel pan */
```

**Files:**
- Modify: `bins/secondreality/src/parts/cred.rs`, `docs/fidelity/cred.md`
- Test: `bins/secondreality/tests/cred.rs`
- Read: `CREDITS/MAIN.C:161-213`, `CREDITS/TWEAK.ASM:203-221` (`tw_setstart`), `:337-353`
  (`tw_setsplit`), `crates/sr-engine/src/view.rs` (`Split`'s two conventions)

**Interfaces:**
- Consumes: `View::{set_origin, split, x_offset}`, `Split { scanline, view }` (`view.rs:55-58`).
- Produces: nothing new — this task completes `Cred`.

- [ ] **Step 1: The three registers, one test each**

```rust
#[test]
fn the_split_register_is_one_more_than_the_origina_ls_line_compare() {
    // `tw_setsplit(200)` names the last line above the split: `Split::scanline`
    // is 201. Confirmed on captured PLZPART frames, reference-indices §13.2.
}

#[test]
fn the_pan_is_the_low_two_bits_of_yy_doubled() { ... }     // AC 0x13 = (yy&3)*2

#[test]
fn the_open_and_close_ramps_are_the_sources_own_sequences() {
    // open: for(y=200*128; y>0; y=y*12L/13)  — ~127 steps
    // close: for(y=0,v=0; y<128*200; y=y+v, v+=15)  — ~59 steps
    // Both are exact integer sequences; a closed form that "matches" them
    // drifts by a frame and the last held frame catches it.
}
```

- [ ] **Step 2: The part, then the comparison**

```bash
cargo run --bin secondreality -- --block 5 --headless --dump-frames 1440,3240,5440,7640,8740 \
    --dump-dir out/p2-cred
cargo xtask compare --reference reference/parts/cred --rendered out/p2-cred --out out/p2-cred-diff
```

Exit 0 is the target: every keyframe is held with `K−1 = K = K+1` in every session (§20.2), so a
correct chain reaches them whatever the wipes do. **The chain is part of the test** — index 1440 is
only reachable if ENDLOGO's duration was right — which is why this run says `--block 5` and not
`--part cred`.

- [ ] **Step 3: Commit**

```bash
git add bins/secondreality/src/parts/cred.rs bins/secondreality/tests/cred.rs docs/fidelity/cred.md
git commit -m "Port CRED's iris: line compare, display start and pan, stepped once a vblank"
```

### Task 9: ENDSCRL — the greetings scroller

A 401-line ring of one-line-at-a-time text, the display start walking it a line per two vblanks,
each line written twice so the wrap is seamless. `ENDSCRL/MAIN.C` is 125 lines over
`ENDSCRL/ASMYT.ASM`; mode 0eh with CRTC 9 = 0 gives a 640×400 screen of 16 colours — the tree's
only 640-wide reference and the only one `compare` cannot take two adjacent frames of

**Files:**
- Create: `bins/secondreality/src/parts/endscrl.rs`, `docs/fidelity/endscrl.md`
- Modify: `bins/secondreality/src/parts.rs`
- Test: `bins/secondreality/tests/endscrl.rs`
- Read: `upstream/SecondReality/ENDSCRL/{MAIN.C,ASMYT.ASM}`, `reference-indices.md` §21,
  `assets/MAIN/DATA/ENDSCROL.TXT` (the shipped text — **not** `ENDSCRL/ENDSCROL.TXT`, which is the
  developer's test text; the original reads `"endscrol.txt"` from disk, and the packed one is the
  shipped one, `MAIN/PACKFINA.INC:51`)

**Interfaces:**
- Consumes: `sr_assets::load_path` for `assets/ENDSCRL/FONA.UH`, the text file, `PartCtx`.
- Produces: `pub struct Endscrl`, registered as `"endscrl"`.

- [ ] **Step 1: The ring, and the wrap that replaces the second copy**

`do_scroll` (`MAIN.C:60-96`) writes the line at `80*yscrl` **and** at `80*(yscrl+401)`, and steps
`yscrl = (yscrl+1) % 401`. The port's surface is 400 lines of 640 pixels — exactly 256,000 — and
its beam wraps modulo the surface, so a 400-line ring reproduces the 401-line ring's *picture*
exactly: the second copy exists because the hardware's address counter does not wrap at the ring's
end, and the port's does. Three things follow, and the note states all three with the arithmetic:

1. the part writes each line **once** (the port's wrap is the second copy);
2. `yscrl` runs `0..400` where the original's runs `0..401`;
3. the display origin is `(yscrl+1) * 640` — up to 256,000, which is `u16::MAX` + 124,462 and the
   reason Task 3 happened.

This is a *modelling* equivalence, not a shortcut, and the diff at four moving keyframes is what
tests it: the four indices are the only place it can be wrong.

- [ ] **Step 2: Two vblanks a line, and the loop that never ends**

`if(dis_waitb()==1) dis_waitb();` (`MAIN.C:47`) — a line every two frames (Task 1's corrected
semantics make this exactly "if one frame passed, wait for another"). The part ends only on
`dis_exit()`, which in the original is ESC or Ctrl; in the port it is `PartCtx::exit_requested`.

```rust
#[test]
fn a_line_is_written_once_and_the_beam_wraps_instead_of_a_second_copy() { ... }
#[test]
fn the_display_advances_one_line_every_two_ticks() { ... }
#[test]
fn the_ring_wraps_at_four_hundred_lines_not_four_hundred_and_one() {
    // and the picture at tick 800 equals the picture at tick 0
}
#[test]
fn the_text_is_the_shipped_one_and_not_the_developers() {
    // assets/MAIN/DATA/ENDSCROL.TXT (6,141 B), not ENDSCRL/ENDSCROL.TXT (1,007 B)
}
```

- [ ] **Step 3: Compare, and record the chain's residual**

```bash
cargo run --bin secondreality -- --block 5 --headless --dump-frames 9100,10000,12000,14000 \
    --dump-dir out/p2-endscrl
cargo xtask compare --reference reference/parts/endscrl --rendered out/p2-endscrl \
    --out out/p2-endscrl-diff
```

**This is the one comparison in the phase whose index cannot be guaranteed.** ENDSCRL's start is
only known as a window — (8840, 8940] — and it is reached through ENDLOGO's measured hold and
CRED's own length; the reference has no ±1 slack here (§21.4: the tool cannot take two adjacent
frames on a 640×400 screen, so a one-frame-out diff is a difference and not an alignment). So the
task's deliverable is the **number and its attribution**: if the diff is zero, say so; if it is not,
measure the residual's *shift* (how many scroll lines the port is off) and attribute it to the chain
(ENDLOGO's gate constant, CRED's wipe lengths) or to the port, and record which. Do **not**
re-anchor the part to make the diff pass: the anchor is evidence and fitting it to the test is the
failure mode Review Focus 5 names.

- [ ] **Step 4: Commit**

```bash
git add bins/secondreality/src/parts/endscrl.rs bins/secondreality/src/parts.rs \
        bins/secondreality/tests/endscrl.rs docs/fidelity/endscrl.md
git commit -m "Port ENDSCRL: the ring, the wrap, and the shipped greetings text"
```

---

## Part C — closing the phase

### Task 10: The runnable binary

The spec's third exit criterion: "a runnable binary sequences them". The shell exists (Task 1) and
the parts exist; this task is what makes the *window* the demonstration rather than a viewer of one
part.

**Files:**
- Modify: `bins/secondreality/src/window.rs`, `main.rs`, `CLAUDE.md` (the run instructions)
- Test: `bins/secondreality/tests/headless.rs` (an end-to-end block run), `window.rs` (unit)

**Interfaces:**
- Consumes: `Sequence` (Task 1), `window.rs`'s existing SDL loop (`:50-108`).
- Produces: `secondreality --block 5` plays ENDLOGO → CRED → ENDSCRL in a window, at the port's
  tick rate, ESC quitting, and a status line naming the part being played.

- [ ] **Step 1: The window drives the sequence**

`window.rs` currently builds a `Runner` for one part. It drives `Sequence` instead, and its event
loop gains the original's own key behaviour: **ESC sets `exit_requested` on the running part**
(`DIS/DISINT.ASM:112-156` — only ESC and Ctrl exit; other keys do not), Ctrl skips the part
(`MAIN/U2.ASM:206-207`). The window never blocks on a key: parts run to their own ends.

```rust
#[test]
fn escape_asks_the_running_part_to_exit_and_ctrl_skips_it() { ... }
#[test]
fn the_status_line_names_the_part_the_reference_index_and_the_block() { ... }
```

- [ ] **Step 2: An end-to-end headless run of the whole ported block**

```bash
cargo run --bin secondreality -- --block 5 --headless --run-seconds 60 --dump-dir out/p2-block5
```

A test runs the equivalent with a frame budget and asserts the parts' order and the indices'
continuity — the shell's own claim, on the port's own clock.

- [ ] **Step 3: The human look — the one step no agent can do**

`cargo run --release --bin secondreality -- --block 5` in a window, watched by the user, and
`--block 2` for BEGLOGO alone. This is Phase 0's lesson repeated deliberately: the phase's only
unverified claim is what the screen looks like, and the last time that was checked by hand it found
a bug no test could see (`PROGRESS.md`, the texture-format bug). The task records what the user
reports, verbatim.

- [ ] **Step 4: Commit**

```bash
git add bins/secondreality/src/window.rs bins/secondreality/src/main.rs \
        bins/secondreality/tests/headless.rs CLAUDE.md
git commit -m "Sequence the ported parts in the window, with the original's own keys"
```

### Task 11: Close out Phase 2

**Files:**
- Modify: `PROGRESS.md`, `docs/decisions.md`, `CLAUDE.md`
- Read: everything the tasks produced

- [ ] **Step 1: Re-run every comparison, and record all of them**

`PROGRESS.md` gains a Phase 2 evidence section with one row per part: the command, the exit code,
the offsets, the percentage, and — where it is not zero — the attribution. A non-zero result is not
a failure of the phase (the spec's criterion for the four easy parts is that they play) but an
**unattributed** non-zero result is: the phase's own standard, from Phase 1's policy, is that the
residue is named.

- [ ] **Step 2: The chain's residual, measured**

The port's part boundaries against the reference's: ENDLOGO's end in (1200, 1240], CRED's in
(8840, 8940]. If the port's own chain lands inside both windows, the chain is consistent with every
measurement the tree holds; if it does not, that is a finding, and it is worth more than the diff
that found it.

- [ ] **Step 3: `grep` the phase for unverified claims**

Every measured constant in this phase is a number derived from a fit or a window. List them in the
close-out — `FADE_FIRST_INDEX`, the two ENDLOGO constants, ENDSCRL's chain — with what would change
if each were wrong by one frame, and mark the two that are checked by an independent fact and the
one that is not.

- [ ] **Step 4: Everything else a phase close needs**

All tests green (`cargo test --workspace`), `cargo clippy --workspace -- -D warnings` clean,
`docs/decisions.md` updated (the numbering, the origin, the `dis_waitb` correction, the deferred
music gates), `CLAUDE.md`'s run instructions and its "what is ported" line made true,
`PROGRESS.md`'s phase table and "where to pick up" updated, and `git tag phase-2`.

- [ ] **Step 5: Commit and tag**

```bash
git add PROGRESS.md docs/decisions.md CLAUDE.md
git commit -m "Close phase 2: the shell sequences, BEGLOGO diffs, four more parts play"
git tag phase-2
```

---

## Task order and why

Task 1 first, because nothing in the phase can be verified without it. Task 2 next, because every
part diff in this phase reads the reference tree and a byte changed in place is currently invisible.
Task 3 before the parts that need it (CRED, ENDSCRL) and after the shell, because it changes an
engine API the shell's tests exercise. Task 4 (BEGLOGO) next: it is the phase's exit criterion and
the project's central technical risk, and its answer — whether bit-exactness is achievable at all —
is worth having before five more parts are ported on the assumption that it is. Tasks 5–9 are the
easy tier in ascending order of difficulty: TUNNELI is one sitting and un-diffable, ENDLOGO adds a
second fade and a music gate, CRED is three mechanisms at once (Task 7's screens and font, Task 8's
three moving registers) and is split because a reviewer could approve the screens and reject the
wipe, ENDSCRL ends with the phase's one irreducible uncertainty. Task 10 then makes the binary the
demonstration the spec asks for, and Task 11 closes.

## Phase completion

A phase is done when its exit criteria are met and: all tests pass; frame diffs are recorded in
`PROGRESS.md`; `docs/decisions.md` is updated with anything that changed the approach; and
`git tag phase-N` is applied. Phase 2's exit criteria, from spec §10: **BEGLOGO frame-diff-identical**
(Task 4's `compare` exits 0 on all four keyframes), **TUNNELI, ENDSCRL, CREDITS, ENDLOGO play**
(Tasks 5–9 each run their part to its own end, with the comparison recorded), and **a runnable
binary sequences them** (Task 10).
