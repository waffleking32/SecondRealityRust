# Working in this repository

A faithful Rust port of **Second Reality** (Future Crew, 1993). Read
`docs/spec/2026-10-10-second-reality-rust-design.md` for the full design; this file is
the short version and the working protocol.

## Resuming work

1. Read **`PROGRESS.md`** — it names the active phase and where to pick up.
2. Read that phase's plan in `docs/plans/`.
3. Read `docs/decisions.md` for anything that has changed the approach since the spec.
4. Read `docs/fidelity/APPROXIMATIONS.md` and `docs/fidelity/bugs.md` before touching a
   part that is already verified.

Do not start a new phase without writing its plan first. Plans are written just in
time, one phase at a time — plans for later phases written in advance are fiction.

## The upstream source

The original 1993 tree is vendored as a pinned git submodule at `upstream/SecondReality`
(`mtuomi/SecondReality`, public domain / Unlicense). **When porting anything, read the
original first.** Quote the file and line in commit messages and in per-part notes. If
the submodule is missing, run `git submodule update --init`.

Directory map for the original: one directory per demo part (`ALKU`, `GLENZ`,
`TECHNO`, …); `MAIN/` is the shell, loader, copper and music player; `DIS/` is the
Demo Interrupt Server; `VISU/` is the 3D engine; `GRID`/`HARD`/`TWIST` are unreleased
development effects.

## Non-negotiables

- **Port the algorithm, not a resemblance.** Translate the original's fixed-point
  maths, tables and rounding. It is usually less work than reinventing the effect and
  it is the only way frame diffs pass.
- **Do not stub out the copper.** Registering per-frame callbacks is required; two
  existing ports skipped it and lost visible effects.
- **Preserve the original's bugs.** See `docs/fidelity/bugs.md`. A "correct"
  implementation of the plasma looks wrong.
- **Read assets in their original formats.** No LBM→PNG, no S3M→OGG. Conversion
  destroys palette precision and the music's sync data.
- **6-bit palette internally**, widened only at upload.
- **Never spin-wait.** Fixed 70.086 Hz logic tick, decoupled from display refresh.
- **Approximations need justification.** Each one goes in
  `docs/fidelity/APPROXIMATIONS.md` with the measured visual difference.

## Licence

MIT (see `LICENSE`). The vendored original source and the media in `assets/` are
public domain under the Unlicense, which imposes no conditions. Set `license = "MIT"`
in any new crate manifest.

## Running it

`cargo run --bin secondreality` opens a window and plays a run. The port has six parts: the
Phase 0 test card, and the demo's own **BEGLOGO**, **TUNNELI**, **ENDLOGO**, **CRED** and
**ENDSCRL**. Any other part the demo runs is refused by name — `--part glenz` says "glenz is not
ported yet" rather than starting something else.

```bash
cargo run --release --bin secondreality -- --block 5     # ENDLOGO -> CRED -> ENDSCRL
cargo run --release --bin secondreality -- --block 2     # BEGLOGO, then it stops: GLENZ is unported
cargo run --release --bin secondreality -- --part beglogo
```

**ESC** ends the part that is playing and the shell starts the next one — the original's exit flag
(`DIS/DISINT.ASM:151-153`, handed to the part's own loop by `exit_2` `:186-190`), except that **no
part the port has yet polls it**: the engine carries the flag and the *shell* retires the part at
the end of the tick it was set in, so a part that reads `ctx.exit_requested` for itself would be the
first to notice the difference. Once the run has nothing left to play, ESC closes the window, where
the original's shell returns to DOS (`MAIN/U2.ASM:935`). **Ctrl** skips the part that is playing
(`MAIN/U2.ASM:205-207`) — dropped where it stands, the next one starting one index on; the original's
`ctrldown` is sticky and skips every later part too. Nothing else does anything, and nothing waits on
a key.

`--block 5` ends in ENDSCRL, whose own loop is `while(!dis_exit())` (`ENDSCRL/MAIN.C:45-49`) — the
scroller runs until you exit, as it does in the demo, so leaving it is ESC twice: once to end the
part, once to close the window (or the window's close button).

A block plays the parts of that block the port has, in the demo's own order, and **stops at the
first one it has not written** rather than skipping over it. The window's title bar is the run's
status line: the part whose picture is up, that frame's index in the **reference's own numbering**
(boot-relative emulator frames — the space everything under `reference/` is measured in), and the
block it came from. A run that has stopped says why, and a run whose index is *not* the
reference's — `--part tunneli` numbers its own frames from zero — says that instead.

`--headless` runs the same sequence on a machine with no display, and is what the comparisons below
use.

## Verification

`dosbox-automation` is the preferred reference oracle, behind a trait — not a hard
dependency. See `docs/decisions.md` D10 for the fallback order and D14 for the API as it
actually behaves. The original accepts `SECOND 2..5` and `SECOND U` to start at a chosen
block.

### Capturing reference frames

`cargo xtask reference capture` drives the emulated original and writes frames plus a
manifest under `reference/`. Two files must sit together in the working directory —
`SECOND.EXE` loads `REALITY.FC` from the current directory — which is why the defaults
are both `reference/original/`.

Five things the command needs, each of which cost time to discover:

1. **`LD_LIBRARY_PATH`** must include the emulator's own build dependencies, because the
   binary has no RPATH: `$SR_DOSBOX/build/release-linux/_deps/{sdl3,sdl3_image,mt32emu,fluidsynth,zlib-ng,iir1}-build`,
   with `SR_DOSBOX` pointing at the `dosbox-automation` checkout.
2. **A scratch `XDG_CONFIG_HOME`** whose *primary* config carries `mount_allowed_bases`.
   `--noprimaryconf` alone cannot mount the reference directory — the mount policy is
   honoured only in the primary config file. `xtask` writes one into a temp directory.
3. **The setup screen.** `SECOND 2` does *not* skip it. The default Gravis soundcard fails
   on this emulator, so `--keys "KBD_left KBD_enter"` selects an SB16 before the demo
   starts. Under `--boot autoexec` (D17) the *guest* presses those keys — the tool sends
   nothing over the API — via `AUTOTYPE left enter` in the config's `[autoexec]` section,
   and the capture then waits for the demo's first graphics frame: keys that leave the
   setup screen up are refused, not captured.
4. **Cycles through the config, never the REST PUT.** Setting `cycles=` at startup forces
   legacy mode and the PUT then returns 409. A capture without pinned cycles is **refused**
   rather than produced: an unpinned capture is not reproducible and must never be mistaken
   for a reference.
5. **Frame indices are boot-relative, and the demo does not start at frame 0.** Frame 0 is
   the emulator's first rendered frame, and `SECOND.EXE` is reached in the first few dozen.
   Under `--boot autoexec` the setup screen is up at frame 16, the title screen's first
   graphics frame is 404 (measured on the emulator's own clock, D17) and the first 320×200
   part follows around 1160; the typed recipe reaches the same screens about 324 frames
   earlier but with the boot's host-timing jitter.

**A capture can hold adjacent indices** — `--frames 2449,2450,2451` — because a run of
indices closer together than one frame's indexed read (~25 rendered frames) is all taken
before any of it is read; the read itself is unchanged (D19). Two things follow.

The **reference holds keyframes only**; the capture that will be compared *against* it holds
**`K-1, K, K+1` around every keyframe that moves**, and **nothing around one that holds**
(`--frames N₁-1 … N_k+1` is the shape only when the keyframes are contiguous, which none in
the tree is). A keyframe that moves has to be answerable from its neighbour on either side:
a session's animation phase is uniform across a capture, so an interior moving keyframe
answered from its own index pairs at offset 0 and reports the one-step difference instead of
aligning. A held keyframe needs no support at all — its ±1 candidates are the same bytes, so
the search finds offset 0 whatever the partner's phase (`reference-indices.md` §1.3, §1.4).
The asymmetry is measured, not stylistic: `compare` pairs *every* frame of the reference, so
a support frame in the reference is itself a reference index whose counterpart lies one
further out — two captures both holding `N₁-1 … N_k+1` refuse at the outermost index (D19).
The two captures are two different commands.

And a batched run is **read after it is taken**, so a run of *k* indices spends ~20·*k*
rendered frames reading afterwards: do not list a distant index less than ~25·*k* frames
behind a support run, or it is refused (D19).

### Comparing

`cargo xtask compare` diffs a rendered frame set against a capture and reports
differing-pixel percentage, max channel delta, and a diff image per frame.

- **Exit codes matter**: `0` every frame identical, `1` frames differ, `2` the tool could
  not compare (a geometry mismatch, a reference index with no rendered frame within one
  frame of it, malformed manifest, missing file). Scripts rely on the difference between
  "they differ" and "it could not tell".
- **Indexed comparison is preferred** and used whenever both sides have `.idx`. It is
  stricter than RGB and makes the DAC widening rule irrelevant. The fallback is RGB.
- Frames are paired by their manifest **index**, not by position, and a rendered frame may
  sit **one frame either side** of the reference index it answers for: the oracle's
  animation phase at one boot index is not reproducible across sessions (D17), so the
  alignment is searched rather than assumed. Which offset each frame was paired at is in
  the report and on the console; a reference index that can only be paired further away is
  refused (exit 2) rather than paired. See D18 for what that could hide.
- **`reference/` is input-only.** Output defaults to `out/compare` and an explicit `--out`
  inside a reference tree is refused. Do not write into a capture directory.

A part is done when its frames match. Record the evidence in `PROGRESS.md`.

### Caveats worth knowing before they bite

- **A view's line stride is unset by default, and resolves to the surface's row stride.**
  `y_line_offset` is `Option<u16>`; `None` reads the surface's own rows, which is what a
  part that draws through `Surface` wants. Set it only to pack rows more tightly than the
  surface's (that is what CRTC 0x13 is for). Setting it to the view's own width on a wider
  surface shears the picture with no error anywhere — that trap was live once.
- **A split's lower view reads from origin 0 in the original.** The hardware sets the
  address counter to 0 and clears panning at the split, so a faithful port leaves the lower
  view's origin and pan alone. And `Split::scanline` is the first line *below* the split,
  while the assembly's line-compare register names the last line above it: add one. See
  `Split`'s doc comment and D16.
- **`View`'s origin is a `u32` pixel address in `start_addr`**, not the CRTC's 16-bit register: the
  surface is 256,000 pixels and CRED reads from 128,000 (D16, closed in Phase 2). `set_origin`
  writes `start_addr` and zeroes `x_offset`, which stays `u16` — an offset *within* a displayed
  line, on top of the origin.
- **Below a split, the scanline count restarts.** Line compare resets the CRTC's counters,
  so `View::effective_at` returns the scanline *within* the view.
- **SDL names packed pixel formats by bit significance, not memory order.** For a byte
  array with red first — which is what `present` writes — the correct enum is
  `PixelFormatEnum::RGBA32`, not `RGBA8888`. They are reverses of each other on a
  little-endian host, and the wrong one reads red as alpha: the screen goes red and every
  test still passes, because nothing else in the project looks at a texture. Anything that
  displays or exports pixels needs a test through SDL's format tables, like
  `window::tests::the_texture_format_reads_the_presenters_bytes_in_order`.

## Fidelity notes

`docs/fidelity/` holds one note per part, written as the part is ported: what the
original did, what the port does, which techniques were translated literally and which
were substituted, and the measured difference. Write the note in the same commit as
the part. **Every number in a note is reproducible by a command the note names, or is
produced by a test in the same commit** (D25), and when a reference frame disagrees with
a faithful port, **D24**'s procedure decides what to do — the vendored tree is a
development snapshot and the shipped release is not the same artifact.

## Phase completion

A phase is done when its exit criteria are met and:

- all tests pass,
- frame diffs are recorded in `PROGRESS.md`,
- `docs/decisions.md` is updated with anything that changed the approach,
- `git tag phase-N` is applied **once the final whole-branch review is clean** — the
  close-out records the evidence and withholds the tag, because a tag that has to be
  moved is worse than a tag applied a few commits later (Phase 1's and Phase 2's
  practice).
