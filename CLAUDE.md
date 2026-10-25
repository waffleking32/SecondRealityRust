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

`cargo run --bin secondreality` opens a window and plays **the demo**: no argument is `SECOND 1`,
as it is in the original (`docs/fidelity/part-sequence.md` §4), so it runs block 1 from ALKU to
ENDSCRL and ends the way `SECOND.EXE` does (below). The port has twenty-one parts: the
Phase 0 test card, and the demo's own **twenty** — **ALKU**, **U2A**, **PAM**, **BEGLOGO**,
**GLENZ**, **TUNNELI**, **TECHNO**, **PANIC** (`panicend`), **MNTSCRL**, **LNS&ZOOM**
(`lnszoom`), **PLZPART**, **MINVBALL**, **RAYSCRL**, **3DSINFLD** (`3dsinfld` — not a legal
Rust identifier, so its module is `coman.rs`), **JPLOGO**, **U2E**, **DDSTARS**, **ENDLOGO**,
**CRED** and **ENDSCRL**. Every part a block plays is ported; a name the demo never had is
refused as unknown rather than starting something else.

```bash
cargo run --release --bin secondreality                  # the demo: the same run as --block 1
cargo run --release --bin secondreality -- --block 1     # ALKU -> ... -> ENDSCRL, no stop
cargo run --release --bin secondreality -- --block 2     # BEGLOGO -> ... -> ENDSCRL, no stop
cargo run --release --bin secondreality -- --block 3     # MNTSCRL -> ... -> ENDSCRL, no stop
cargo run --release --bin secondreality -- --block 4     # U2E -> ENDLOGO -> CRED -> ENDSCRL
cargo run --release --bin secondreality -- --block 5     # ENDLOGO -> CRED -> ENDSCRL
cargo run --release --bin secondreality -- --block U     # DDSTARS (the hidden part)
cargo run --release --bin secondreality -- --part glenz
cargo run --release --bin secondreality -- --part testcard   # the Phase 0 test card
```

`--block 1` runs **ALKU → U2A → PAM → BEGLOGO → GLENZ → TUNNELI → TECHNO → PANIC → MNTSCRL →
LNS&ZOOM → PLZPART → MINVBALL → RAYSCRL → 3DSINFLD → JPLOGO → U2E → ENDLOGO → CRED →
ENDSCRL** with no stop. `--block 2` begins at BEGLOGO and runs the same sequence from there;
`--block 3` begins at MNTSCRL, `--block 4` at U2E, and `--block 5` runs ENDLOGO → CRED →
ENDSCRL. Every block's plan is fully ported, so no block stops at a part; a run whose source
cannot build the next name still holds there rather than skipping over it.

**ESC** ends the part that is playing and the shell starts the next one — the original's exit flag
(`DIS/DISINT.ASM:151-153`, handed to the part's own loop by `exit_2` `:186-190`). The Phase-4
parts, and LNS&ZOOM, TECHNO, 3DSINFLD and JPLOGO, read it (`ctx.exit_requested`) where their
sources consult the exit flag, and in their wait loops, which must not spin; a part that sees it
can end on that tick without drawing. **U2A** and **U2E** poll it in their waits and loops too:
U2A's teardown is synchronous within the exit tick, while U2E runs its sixteen-step DAC fade
after ESC — the one part that opts into `Part::drains_after_exit`, so the shell keeps ticking it
until it reports `Finished`. U2A's gate-2 cut is the original's music cut (`U2A.C:276`), not an
exit; it leaves through the part's own teardown path.
**GLENZ** polls it in its music wait (`MAIN.C:290`), the bounce (`:320`), both `getmframe` waits
(`:353`, `:400`) and the main loop (`:411`), and the main loop also ends on the music cut
`-16 < musplus < 0` or after frame 7000. The music wait leaves on either condition and continues
into the zoomer, whose 47 bodies do not poll; ESC before the bounce's first body skips the
bounce and both waits, as the source orders it, and the part retires on its first main-loop
check — it has no teardown and does not drain.
**PANICEND never polls it** — `SHUTDOWN.C` has no `dis_exit()` call — and **PLZPART**'s port leaves
it to the shell as well (its source polls, but the tick is drawn and the part retired either way);
for a part that does not poll, the engine carries the flag and the *shell* retires the part at the
end of the tick it was set in. **After PANICEND retires** (by its own `Finished`, not by ESC), the
shell runs the original's cue wait (`MAIN/U2.ASM:826-853`, the wait at `:833-853`): it holds the
screen and polls `musplus`, waiting while it is `>= 0` and then while it is `<= 0`, and continues
on the first `> 0`. The wait is exit-aware — an ESC already set when it would be installed skips it
(`@@skip5`) — and a plan that ends at PANICEND enters the wait and then the hold. Once a
`--part` run has nothing left to play it holds its last screen, and ESC closes the window, where
the original's shell returns to DOS (`MAIN/U2.ASM:935`); a block's run ends by itself instead
(below). **Ctrl** skips the part that is playing
(`MAIN/U2.ASM:205-207`) — dropped where it stands, the next one starting one index on; the original's
`ctrldown` is sticky and skips every later part too. Nothing else does anything, and nothing waits on
a key.

`--block 1` through `--block 5` end in ENDSCRL, whose own loop is `while(!dis_exit())`
(`ENDSCRL/MAIN.C:45-49`) — the scroller runs until you exit, as it does in the demo, so leaving it
is **one** ESC. A run built from a block (`--block`, or no argument) then ends the way `SECOND.EXE`
does (D41): the hold fades the music for **64** ticks over the last picture (`@@partsdone`'s
`fademusic`, `MAIN/U2.ASM:937-939`, `:333-346`), shows text mode's blank screen for **20** ticks
(`fatalexit`, `:967-973`), and the window closes by itself; ESC during that stage closes it at once
(the original's stage acts on no key — closing early is the port's courtesy). The end box,
`assets/MAIN/DATA/U2END.BIN` (`printend`, `:506-535`), is then printed to the terminal the run was
launched from — the stand-in for the text mode it is written into — with its colours when stdout is
a terminal and as plain text when it is not. It is printed for every windowed demo run however its
window closed (the close button too, as `fatalexit` prints it on every exit), and never for
`--part`, `--vault` or `--headless`. A `--part` run, and a vault run, still hold at their end.

A block plays the parts of that block, in the demo's own order; every block's plan is now fully
ported, so no block stops. A run whose source cannot build the next part still holds at the first
one it cannot run rather than skipping over it. The window's title bar is the run's
status line: the part whose picture is up, that frame's index in the **reference's own numbering**
(boot-relative emulator frames — the space everything under `reference/` is measured in), and the
block it came from. A run that has stopped says why, and a run whose index is *not* the
reference's — `--part tunneli` numbers its own frames from zero — says that instead.

A windowed demo run opens on **the settings screen** first (D44): Sound, Sound quality, Demo looping,
Aspect and Border, drawn in `ALKU/FONA.UH` — Up/Down choose, Left/Right change, Enter starts and ESC
leaves (printing the end box). It is the port's stand-in for `START.EXE`'s `menu()`; nothing is saved,
the flags are the initial values, and `--no-setup` or `/gravis` skip it. `--part`, `--vault` and
`--headless` never show it, so the only run that waits on a key is a demo run's setup.

`--headless` runs the same sequence on a machine with no display, and is what the comparisons below
use.

### The vault

The three unreleased development effects are playable without touching the demo's sequence:

```bash
cargo run --release --bin secondreality -- --vault      # the window-only menu
cargo run --release --bin secondreality -- --part grid  # GRID, HARD or TWIST directly
```

GRID, HARD and TWIST are registered as silent, unanchored parts (`music_for_part` → `None`,
no player; `--part <name>` numbers its frames from zero) and are deliberately **not** in
`PART_ORDER`, `BLOCKS` or any boot anchor. `--vault` opens a 320×200 menu built from the demo's
own `ALKU/FONA.UH` strip and starts the chosen part; it is window-only and refuses
`--headless`, `--part`, `--block`, the dump/frames flags and `--audio-dump` ("the vault menu
needs a window; name a part with --part"). In the menu, ESC closes the window. In a running
part, the first ESC is the part's own exit (the original's `dis_exit`), and the second ESC —
on the held run — returns to the menu; a part that has finished by itself (HARD's `default`
arm, TWIST's 4,444-frame cap) is already holding, so one ESC returns. The normal,
non-`--vault` path is unchanged.

The references are `reference/parts/{grid,hard,twist}`, captured from the vendored
`GRID/P.EXE`, `HARD/B.EXE` and `TWIST/T.EXE` behind the standalone `DIS/DIS.EXE` by
`tools/vault_capture.sh` (route A: `--boot autoexec --boot-wait graphics --cycles 30000`, one
`KBD_space` for GRID/HARD and none for TWIST). The parts are verified against them at
`K0` = 353 / 3 / 356; the notes are `docs/fidelity/{grid,hard,twist}.md`, the capture record
`docs/fidelity/reference-indices.md` §24, and the decisions **D39/D40**.

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
