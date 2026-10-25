# Second Reality → Rust — Design Specification

**Date:** 2026-10-10
**Status:** Approved, pending implementation planning
**Upstream source:** `mtuomi/SecondReality` (public domain, Unlicense), commit `071a82e`

---

## 0. How to use this document

This is the load-bearing artifact for a long, multi-session project. If you are a
future session with no context, read `PROGRESS.md` first to find the active phase,
then the phase's plan in `docs/plans/`, then the sections of this spec that the
plan references.

This spec defines *what* is being built and *why* the architecture is shaped the
way it is. It does not enumerate tasks — those live in per-phase plans.

---

## 1. Goal

Build a faithful, native Rust port of **Second Reality** (Future Crew, 1993) that
runs on a current Linux system, producing output that is visually indistinguishable
from the original and verified against it frame by frame.

### Success criteria

1. The complete demo plays start to finish: the main sequence in order (§2.2),
   music-synced, with the hidden part and loop mode.
2. The three unreleased development effects (GRID, HARD, TWIST) are playable from a
   "vault" menu.
3. Every part is verified against reference frames captured from the original running
   under emulation. Target: pixel-identical. Where a part's effect lives inside
   esoteric generated assembly, a documented approximation is permitted (see §5).
4. The original media assets are consumed in their original formats, unmodified.
5. A future session can resume work with no re-derivation of context.

### Non-goals

- Running the original x86 binaries. This is a source port, not an emulator.
- Windows/macOS/WebAssembly builds. The code is kept portable, but Linux is the only
  verified target. Revisit if wanted.
- An "enhanced" visual mode. The architecture leaves a seam for one (§4.2) but it is
  out of scope.
- Reproducing the DOS-era setup menu (sound card IRQ/DMA selection), which is
  meaningless on modern hardware. Replaced by a modern settings screen.

---

## 2. What the original is

The upstream tree is the 2013 public-domain source release: 1319 files, ~35 MB, 126 C
files, 98 ASM files, 29 Turbo Pascal projects, plus original art and the two music
modules.

### 2.1 Runtime architecture

- **One shell plus many part programs.** `MAIN/U2.ASM` (built as `U2.EXE`, shipped
  renamed `SECOND.EXE`) is the only resident program. It statically links the file
  loader (`MAIN/U2A.ASM`), the raster scheduler (`MAIN/COPPER.ASM`), the `int 10h`
  video hook (`MAIN/VMODE.ASM`), the Demo Interrupt Server (`DIS/DISINT.ASM`) and the
  music player STMIK (`MAIN/STMIK.300`, a binary blob with no source).
- **Parts are separate DOS EXEs**, loaded sequentially by the shell, one at a time.
  They call back into the shell through `int 0xFC` (DIS).
- **The demo is music-counted, not frame-counted.** Parts run until a music-position
  cue is reached, via `dis_sync()`, `dis_musplus()`, `dis_musrow()`, or a frame count
  started at a cue.

### 2.2 Part sequence

From `MAIN/U2.ASM`. **Transcribed and verified in Phase 0** (task 10) — the citations,
the entry-point conditions and the block arguments are in
`docs/fidelity/part-sequence.md`, which is the reference; this table is the summary.
What changed in the correction is recorded as D13.

`Order` numbers the show parts only; the menu and the two helper programs sit outside
that numbering, and the hidden part sits outside the run. Slot **17 is the hidden
part's slot** — it is not executed in the main sequence.

| Order | Binary | Directory | Effect |
|---|---|---|---|
| — | START.EXE | MAIN | setup menu (text mode), not a show part |
| — | STARTMUS.EXE | MAIN | music loader/unloader helper, not a show part |
| — | — | — | MUSIC0 starts at order 0 — only when block 1 runs; `SECOND 2`–`5` load MUSIC1 at order 0 instead |
| 1 | ALKU.EXE | ALKU | intro text I + horizon pan |
| 2 | U2A.EXE | VISU/C | intro text II / vector part 0 |
| 3 | PAM.EXE | PAM | Praxis explosion (FLIC) |
| 4 | BEGLOGO.EXE | BEG | Future Crew logo fades in from white |
| — | — | — | MUSIC1 starts at order 0 |
| 5 | GLENZ.EXE | GLENZ | translucent "glenz" logo |
| 6 | TUNNELI.EXE | TUNNELI | dot tunnel |
| 7 | TECHNO.EXE | TECHNO | techno bars + EGA interference |
| 8 | PANICEND.EXE | PANIC | panic fake-ending (screen shrinks to a dot) |
| — | — | — | music-cue wait; **on `SECOND 3` only**, MUSIC1 then restarts at order 42 — a full run restarts nothing here |
| 9 | MNTSCRL.EXE | FOREST | mountain scroller |
| 10 | LNS&ZOOM.EXE | LENS | lens + rotazoomer |
| 11 | PLZPART.EXE | PLZPART | plasma + plasma cube |
| 12 | MINVBALL.EXE | DOTS | mini vector balls |
| 13 | RAYSCRL.EXE | WATER | raytraced mirror-ball scroll |
| 14 | 3DSINFLD.EXE | COMAN | voxel terrain ("Comanche") |
| 15 | JPLOGO.EXE | JPLOGO | wobbling "jelly" logo |
| — | — | — | MUSIC0 restarts at order 18 |
| 16 | U2E.EXE | VISU/C | vector part II — space battle |
| 17 (hidden) | DDSTARS.EXE | DDSTARS | **hidden part**, MUSIC0 at order 70; run only by `SECOND U`, alone |
| — | — | — | MUSIC0 restarts at order 25 — **on `SECOND 5` only**; a full run restarts nothing here either |
| 18 | ENDLOGO.EXE | END | end picture |
| 19 | CRED.EXE | CREDITS | credits pictures |
| 20 | ENDSCRL.EXE | ENDSCRL | greetings scroller (skipped in loop mode) |

Loop mode restarts MUSIC0 at order 0 and jumps back to part 1. The jump target is the
top of the runner (`@@partsagain`), and `whattorun` is reset to run everything first, so
every pass after the first runs the whole sequence whatever the demo was started with.

The rows marked "on `SECOND 3` only" and "on `SECOND 5` only" are **entry-point**
restarts: they exist so that starting mid-demo reproduces the music position a full run
would have reached by that point, and each is skipped precisely when the blocks leading
up to it did run. A full run therefore reaches the mountain scroller and the ending with
the music already playing and no restart at either boundary.

The hidden part is entered with `SECOND.EXE U` (`checkcmdline` maps `u`/`U` to
`partsmask[8]`, giving `whattorun = 0x80`). The other block arguments are `1`–`5`, and
`1` behaves exactly like no argument. These arguments are how the reference capture
harness starts the original at a chosen point, and the port must accept the same
arguments for comparison. The shipped `reference/original/README.1ST:66-79` documents
`1`–`5` and never mentions `U`; the code is the authority on behaviour, and both are
recorded in `docs/fidelity/part-sequence.md` §4, along with the `/gravis` switch the
shell passes through to `START.EXE`.

### 2.3 Display modes

| Geometry | Where |
|---|---|
| 320×200×256, unchained planar ("Mode X" tweak) | most parts |
| 320×400×256, planar | ALKU, BEG/END/ENDPIC, PAM, COMAN, JPLOGO |
| 640×400×256, planar | vector part II |
| 640×350×16 → 640×400 via CRTC reg 9 | ENDSCRL |
| BIOS text mode 3 | setup menu, exit screens |

**Superseded for three rows, by capture (Phase 1, 2026-10-12).** PAM is **320×200**, not
320×400; COMAN/3DSINFLD is **320×200** with chain-4 off (80-byte plane rows); and the vector part II
(U2E) is **320×200** and normalises its own display (CRTC 9 scan-doubling), so its screen does not
depend on the boot — only its boot index does. The rows above are the spec's own record and are left
standing; the measured geometries, per part, are in
`docs/fidelity/reference-indices.md` (§6 PAM, §16 3DSINFLD, §18 U2E) and `PROGRESS.md`'s per-part
table, and where the two disagree the capture is the current reading. (The same treatment D17 gives
`PROGRESS.md`'s 1657/1700 landmarks.)

### 2.4 Hardware behaviours that are visually significant

Everything else the original does with the VGA is a performance strategy and is
discarded. These five are not:

1. **Palette DAC written mid-frame** — fades, colour cycling, raster bars. Present in
   nearly every part.
2. **CRTC start address** — horizontal panning (`ALKU`), vertical scrolling
   (`JPLOGO`), the 401-line circular buffer (`ENDSCRL`).
3. **Line compare (CRTC 0x18 + 0x07 bit 4)** — split screens (`PLZPART`'s falling
   screen, `CREDITS`).
4. **Double-scan / 400-line mode** — `PANIC`'s shrink-to-dot, the 320×400 parts.
5. **Overscan border colour (AC 0x11)** — used deliberately as a design element.

### 2.5 Music

Two Scream Tracker 3 modules: MUSIC0 "UnreaL ][ - The 2ND Reality" (80 orders,
inittempo 125, speed 6) and MUSIC1 "UnreaL ][ / PM" (100 orders, inittempo 130,
speed 3). Shipped inside `REALITY.FC` as a 16-byte header plus the two files.

**The pattern data is XOR-scrambled.** Standard S3M decoders fail on it; the rest of
the file is a standard S3M (headers, instrument tables, pointer table, plain PCM
samples). The keystream is recovered and verified — see §7.1. `STARTMUS.C` also
patches MUSIC0's initial tempo from 125 to **120** at load, which affects every
music-derived timing constant.

There are **no separate sound effect files**. The rumble, flybys and explosion are
samples inside the two modules.

### 2.6 Protection and packing — discarded

`MAIN/PROT/` and `FCP/` implement a self-decrypting EXE protector with anti-debugging.
`MAIN/PACK.C` appends every part EXE and data file to `SECOND.EXE` behind a
filename-hash-keyed directory with per-file XOR. None of this is ported. The pack
format is understood but irrelevant; the port reads the loose files.

---

## 3. Prior art and its lessons

Three modern ports exist and none is in Rust.

| Project | Strategy | Relevance |
|---|---|---|
| `ConspiracyHu/SecondRealityW32` | keeps the original x86 asm, 16→32 bit, thin hardware shims | the canonical modern port |
| `XorJS/SecondRealityPlusPlus` | fork of the above, asm→C++, SDL2/OpenGL, WASM | cross-platform version |
| `treylorswift/demoscene-recomp` | instruction-level recompilation to WASM with modelled hardware | best fidelity reference |

**What they got wrong, and we will not:**

- Both C++ ports **stub the copper out** (`dis_setcopper` is an empty function) and
  upload one palette per frame. They consequently **lost the techno part's
  interference seam** — a visible quarter-screen discontinuity where the beam caught
  the screen mid-redraw. `demoscene-recomp` composes per scanline from VRAM and
  palette as they stood when the beam drew them, which is the only reason its techno
  part is correct.
- They **widen the palette to 8-bit**, losing the VGA's 6-bit DAC precision.
- They **spin-wait to 70 Hz**, burning a core and juddering on 60 Hz displays.
- `SecondRealityW32` **stubs `dis_muscode()` to `return 0`**, which means the parts
  that wait on it must have been patched. We will implement it properly (§7.4).

**What they got right, and we will copy:**

- **The original asset formats are read at runtime**, unmodified, by every project.
  No LBM→PNG or S3M→OGG conversion. This preserves palette precision and, for the
  music, the sync data.
- **Preserve the original's bugs.** A port that "fixes" them looks wrong. See §5.1.
- **Verification is differential against an emulator** — frame hashes and, in
  `demoscene-recomp`'s case, event-level comparison.

---

## 4. Architecture

### 4.1 Crate layout

```
SecondRealityRust/
  Cargo.toml                 workspace
  crates/
    sr-engine/               surface, view, palette, tick, copper, Part trait
    sr-assets/               original-format loaders
    sr-audio/                module player + music clock
    sr-parts/                one module per demo part
    sr-shell/                sequencer, part order, transitions, settings screen
    sr-vault/                GRID / HARD / TWIST
  bins/
    secondreality/           the binary
  xtask/                     asset verification, reference capture, comparison
  assets/                    curated original media (see §9)
  reference/                 captured reference frames + manifests (see §8)
  docs/spec/                 this document
  docs/plans/                per-phase implementation plans
  docs/fidelity/             per-part fidelity notes, approximations register
  docs/decisions.md          decisions and findings that changed the approach
  PROGRESS.md                phase status board
  CLAUDE.md                  how to work in this repo
```

Rationale: crate boundaries give clean, independently testable units and let a single
part be compiled and tested without the whole demo. `sr-part` modules are large enough
in aggregate that a single crate with one module per part is the right granularity —
crate-per-part would be ceremony without benefit.

**As built (amended 2026-10-25, Phase 9 Task 10): three library crates, and the parts, the shell
and the vault in the binary.** The workspace is `crates/sr-engine`, `crates/sr-assets`,
`crates/sr-audio`, `bins/secondreality` and `xtask`. There is no `sr-parts`, `sr-shell` or
`sr-vault`: the parts are modules of the binary (`bins/secondreality/src/parts/`, one a part, with
`parts/visu/` and `parts/glenz/` for the two renderers two parts or one large part need), the vault
is `parts/vault/` with `vault_menu.rs`, and the shell is `shell.rs` with `settings.rs` and
`window.rs`. The decision is Phase 2's — "No new crate: the shell is a module in the binary, the
parts are modules beside it" (`docs/plans/02-shell.md:20`) — recorded as **D32** for the vector
renderer, D36 for GLENZ's and Phase 8's plan for the vault (`docs/plans/08-vault.md:21`); the
rationale above is unchanged, one module per part, with the binary as the single crate. The
dependency rule is the one that held throughout: `sr-engine` and `sr-audio` have no runtime
dependencies, and `sr-assets` depends on `sr-engine` only. The tree above is the spec's record.

### 4.2 `sr-engine` — the software VGA model

The core insight: **the visible output of the original is always an indexed 8-bit
image at one of a few geometries, plus a palette that can change mid-frame, plus a
view into a larger canvas.** Everything else is implementation detail of 1993
hardware.

**`Surface`** — a chunky 8-bit indexed canvas, allocated at the largest geometry in
use (640×400). Parts write individual pixels with the same coordinate maths as the
original. The Mode X plane arithmetic disappears because it never changed *which*
pixels were written, only how fast.

**`View`** — describes what of the surface is displayed and how:

```rust
struct View {
    x_offset: u16,        // pixel offset within a line, added to start_addr
    y_line_offset: Option<u16>,  // pixels per displayed scanline (CRTC line offset 0x13);
                                 // None = the surface's own row stride
    start_addr: u32,      // display start, in pixels (CRTC 0x0C/0x0D)
    pan: u8,              // fine pan in *pixels*, 0..=7 (AC 0x13's value is halved
                          // in a 256-colour mode; see view.rs's `pan`)
    double_scan: bool,    // CRTC reg 9 bit 7 / max scanline 0
    split: Option<Split>, // line compare: scanline + second View
    border: Option<u8>,   // overscan colour (AC 0x11)
    height: u16,
}
```

**Amended in Phase 2 (2026-10-13): `start_addr` is `u32`, not `u16`.** The sketch above copied the
CRTC's 16-bit register pair, but that pair counts *bytes within a plane* and this `View` counts
pixels: the widest geometry is 640 × 400 = 256,000 pixels, which 16 bits do not reach, and the two
parts that need to reach it would have wrapped *silently* into a plausible-looking wrong picture.
CRED's display start is 400 logical lines of 320 pixels = **128,000**, and ENDSCRL's display start
walks a 400-line ring up to **256,000**. `x_offset` stays `u16`: it is an offset within a displayed
line, bounded by the widest line rather than by the buffer, and the origin now lives in
`start_addr`, which is what `View::set_origin` writes (zeroing `x_offset`). See `docs/decisions.md`
D16, whose open item this closes.

**And two more sketch lines were stale, and are corrected here (Phase 2's close-out).**
`y_line_offset` is `Option<u16>` counting *pixels* — amended in Phase 0's closing review, which
caught the trap this line set: a default of the view's own width shears a 320-wide picture drawn on
the 640-wide surface, and `None` — read the surface's own rows — is what a part drawing through
`Surface` wants (D16, `CLAUDE.md`'s caveats). And `pan` is a **pixel** count 0..=7: Task 8 measured
that a 256-colour mode shifts `(value & 0x7) / 2` pixels (`vga_attr.cpp:225-227`), so it is AC
0x13's unit only in some modes, and `View::pan`'s own doc carries the measurement.

This single struct replaces the CRTC. It is fully testable in isolation, which is
where most of the hardware-trick risk is retired.

**`Palette`** — a base 256-entry palette with **6-bit per channel precision**,
widened to 8-bit only when uploaded for display, exactly as the DAC did. A frame
carries a list of `(scanline, palette)` change events; parts push new palettes, or
tag a change at a scanline for raster bars.

**`Tick`** — a fixed 70.086 Hz logic tick, independent of display refresh. See §6.

**Copper** — the original's three per-frame callbacks are preserved as named slots,
with the ordering made explicit and deterministic:

```
tick:
  1. copper slot 0   (top of screen — where the original polled the music mixer)
  2. part.frame()    (the part's drawing work for this frame)
  3. copper slot 1   (just before retrace)
  4. copper slot 2   (in retrace — page flips, palette setup)
  5. compose + present
```

A part that used `dis_setcopper` in the original registers here; it is **not** a
no-op. This is the single most important correction over the existing C++ ports.

**Beam compose.** A part may optionally render in top-to-bottom bands. After each
band the engine commits those scanlines to the output using the palette current at
that moment, reproducing mid-frame VRAM updates *and* mid-frame palette changes in
the correct order. Default is whole-frame; parts that need it opt in via a
`Part::band_count()` hint. This exists specifically because the techno part's
interference circles have a visible seam without it, and because `PLZPART`'s split
sweeps during the frame.

**Presentation.** Index → RGB per scanline through that scanline's palette, scaled
nearest-neighbour to the window at an integer factor where possible. Default 4:3
(320×200 stretched to a 4:3 box, matching a VGA monitor's rectangular pixels); a
square-pixel 16:10 option is available in the settings screen. The border is rendered
as a frame around the picture when a part sets an overscan colour, with a setting to
disable it.

### 4.3 The `Part` trait — replacing DIS

```rust
pub trait Part {
    fn init(&mut self, ctx: &mut PartCtx) -> Result<()>;
    fn frame(&mut self, ctx: &mut PartCtx) -> PartStatus;
    fn band_count(&self) -> u16 { 1 }   // >1 enables beam compose
}

pub enum PartStatus { Continuing, Finished }
```

`PartCtx` is the DIS surface, minimised:

| DIS service (original) | Port equivalent |
|---|---|
| `dis_version`, `dis_partstart` | gone — trait construction |
| `dis_waitb()` | engine tick; **return value and counter-reset semantics preserved (§6.2)** |
| `dis_exit()` | `ctx.exit_requested` |
| `dis_indemo()` | `ctx.standalone` — parts run standalone too (this is what the vault uses) |
| `dis_msgarea(n)` | typed inter-part channels in `sr-shell` |
| `dis_sync()` | `ctx.music.sync_code()` — table preserved verbatim, **including its off-by-one** |
| `dis_muscode/musplus/musrow` | `ctx.music.{zinfo, plus, row}()` |
| `dis_setmframe/getmframe` | `ctx.music.{set_frame_origin, frame_count()}` |
| `dis_setcopper` | `ctx.set_copper(slot, f)` |

### 4.4 `sr-assets`

Loaders for the original formats, all discovered by inspecting the tree. Each writes
into a `Surface` and returns its palette.

| Format | Notes |
|---|---|
| IFF ILBM / PBM (`.LBM`, `.BBM`) | RLE (ByteRun1), stencil/EHB, multiple bitplane counts |
| Autodesk FLI/FLC (`.FLI`) | delta-frame decode, 320×200 |
| "Uh1" raw (`.UH`) | 16-byte header + 768-byte palette + chunky pixels |
| Packed screens (`.U`, `.P`, `.UP`, `.16`) | magic `0xFCFC` raw / `0xFCFD` RLE, palette at paragraph offset |
| `.X` | `"UH\1\0"` header variant, no palette |
| RIX3 (`.CLX`, `.SCI`) | 10-byte header + 768-byte palette + raw |
| `.PAL` | 768-byte VGA palettes |
| Font strips | `FONA.LBM` and friends |
| Custom animation (`PAM/OUT.ANI`) | record scheme from `PAM/ANIM.C` |

Also needed, for the vector parts: the packed scene/object format produced by
`VISU/C`'s converters from the `.3DS` meshes (magic `VERS`/`NAME`/`VERT`, one file per
object, plus `.00M` master and `.0AA` scene script). This is deferred to Phase 6.

Not needed: `.OBK`/`.OBJ` (OMF build artifacts), `.INC`/`.BBK`/`.CUT`/`.PRE` (source
data fragments), `.TPU`/`.TP`/`.DSK`/`.PRJ` (Pascal/3D-Studio project files).

Where a needed asset only exists inside a shipped EXE as a compiled data segment, it
is recovered from the OMF `LEDATA` records during Phase 1.

### 4.5 `sr-audio`

A purpose-built S3M player with Scream Tracker 3 semantics, exposing the STMIK clock.
See §7.

### 4.6 `sr-shell`

The `U2.ASM` equivalent: part sequence, music restart points, the inter-block
music-cue wait, transitions, exit handling, loop mode, and the modern settings screen.

*(Amended 2026-10-25: built as `bins/secondreality/src/shell.rs`, `settings.rs` and `window.rs`,
not a crate — §4.1's note. Loop mode is D42, the settings screen D44, the run's end D41.)*

### 4.7 `sr-vault`

GRID, HARD and TWIST, playable standalone. These are development effects that never
shipped; they are real ports, held to the same standard, but are not part of the main
sequence.

*(Amended 2026-10-25: built as `bins/secondreality/src/parts/vault/` and `vault_menu.rs`, not a
crate — §4.1's note; D39, D40.)*

### 4.8 Tooling

`xtask` subcommands: `assets verify`, `reference capture`, `compare`, `audio render`,
`event log`. See §8.

**As built (amended 2026-10-25, Phase 9 Task 10, D47).** `cargo xtask` has `reference capture`,
`reference verify` (and `backfill`), `compare`, `rediff` (every filed frame at its registered
count), `frames sheet`, `assets verify`, and `music symbols`, `trace`, `analyse` and `probe` (the
original's player state, D26). `audio render` and `event log` were not built: D47 retires them,
with §8.3's event stream, in favour of `rediff`'s frame comparison, `music trace`, and the binary's
`--audio-dump` (the port's PCM) and `--positions` (a headless run's part and published position a
frame).

---

## 5. Fidelity policy

**Default: pixel-identical.** The reference is a framing captured from the original
running under emulation (§8). A part is done when its frames match.

### 5.1 Preserve the bugs

The original contains artefacts that are part of its visual identity. These are
**required behaviour**, recorded in `docs/fidelity/bugs.md` with a reference frame
each, so that no future session "fixes" them:

- The GMX plasma's palette-leak banding (`PLZPART`): some colour from the previous
  palette persists, producing banding. A *correct* implementation does not produce
  this and looks wrong.
- Shadow particles rasterised over the cyan particles in the jumping-particle part —
  there is no z-buffer.
- The `dis_sync()` off-by-one (returns the previous entry's code; reads off the table
  start at order 0).

### 5.2 Approximations register

`docs/fidelity/APPROXIMATIONS.md` lists every place the port knowingly differs, with:
the part, the original technique, what the port does instead, why, and the measured
visual difference. An entry requires justification; it is not a parking place for
"close enough".

Known candidates, to be confirmed per part:

- **Generated unrolled assembly** — `GLENZ/ZOOMLOOP.INC` (14.5k lines),
  `JPLOGO/ZOOM.INC` (16.8k), `TWIST/TWSTLOOP.INC` (11.8k), `GRID/LINEBLIT.INC` (8.1k),
  `COMAN/THELOOP.INC` (5.2k), `HARD/LINEBLIT.INC`. These are unrolled specialisations
  of simple loops. Replacing them with the equivalent loop should be bit-identical;
  where it is not, the difference is recorded.
- **Self-modifying code** — rewritten as variables/tables. `TUNNELI`'s patched y-base,
  `GLENZ/VEC.ASM`'s matrix-in-immediates, `LENS/ASM.ASM`'s patched addressing,
  `PLZPART`'s jump table. The original compiles a value into an instruction; the port
  reads a variable. Expected to be identical.

**The candidates' outcomes (amended 2026-10-25, Phase 9 Task 10).** `APPROXIMATIONS.md`'s section
"Generated and self-modifying code" has one row each, with its test: eight are bit-identical where
the reference can see them, and `GLENZ/ZOOMLOOP.INC` and `HARD/LINEBLIT.INC` are not on the shipped
path. Two of the list's words are wrong, and the register's rows say so: what `TUNNELI` patches is
the plot's **colour immediate** (`TUN10.PAS:168`), not a y-base, and `PLZPART` has no jump table —
`setplzparas` patches **four table bases** into `plzline`'s instructions (`ASMYT.ASM:46-61`,
`:74-95`).

### 5.3 Timing fidelity

The port runs at exactly 70.086 Hz. It does not re-time to the display. On a 60 Hz
panel some frames are shown twice, which is the honest behaviour. No interpolation.

Two documented exceptions where the original did *not* run at a nominal 70 Hz:

- `dis_waitb()`'s return value and reset semantics (§6.2).
- Parts calibrated to `dis_getmframe()`, whose clock runs slower than 70 Hz under
  load because the original's timer handler misses ticks (§6.3).

---

## 6. Timing model

### 6.1 The tick

A fixed 70.086 Hz logic tick drives everything. Presentation is paced by display
vsync and shows the most recent completed frame — the newest due frame, dropping
stale ones. The engine never spin-waits.

The engine's tick sequence is the copper ordering in §4.2.

### 6.2 `dis_waitb` semantics

> **Corrected in Phase 2** — the two paragraphs below said the opposite of the source.
> The reading in `docs/decisions.md` **D22** is the one the port follows: the counter is
> read, returned and *then* zeroed, so a part whose work overran *k* frames gets *k+1*.

In-demo, `dis_waitb()` does **not** poll the retrace port. It waits for the copper's
frame counter to change, then **reads the new value, returns it and zeroes the counter**
— the reset happens *after* the read (`DIS/DISINT.ASM:165-169`). Consequence: a part
whose work overran *k* frames gets *k+1*, not 1 — the overrun is **counted**, not
discarded. Parts use the return value to pace animation: `TUNNELI/TUN10.PAS:200` is
`for sync := 1 to frames` and `ENDSCRL/MAIN.C:47` is `if(dis_waitb()==1) dis_waitb();`,
so "how many frames did I miss" is the number they are written against.

One tick is one frame and the port calls a part's `frame()` once per tick, so the value
is always 1 and the overrun case cannot arise. A part that guesses wrong about it takes
the wrong number of animation steps per tick; that is the whole of its pacing (D22).

The `0x3DA` retrace poll only applies when a part runs standalone from DOS, outside
the demo.

### 6.3 `dis_getmframe` caveat

`_np_zframe` is a frame counter that parts reset (`dis_setmframe(0)`) and then wait on
(`while(dis_getmframe() < 300)`). In the original it is advanced by the timer
interrupt, and **the handler misses ticks under heavy per-pixel load**, so it runs
slower than the vertical refresh. Parts keyed to it (`GLENZ`, `PLZPART`, `TECHNO`) are
therefore calibrated to its *real* rate, not to 70 Hz.

The port must determine that real rate empirically — by logging `dis_getmframe()`
against wall-clock frames in the emulated original — and reproduce it, rather than
assuming 70 Hz. This is a Phase 3 deliverable; until then, affected parts will drift.

### 6.4 Music as the clock

Parts synchronise to the *music position*, not to frames. As long as the port exposes
`ord` and `row` correctly and its tick model matches the original's, cue alignment
follows. The critical parameters are therefore the tick model, the tempo patch (§7.3)
and the speed-adjust constants — not the sync API's shape.

---

## 7. Music subsystem

### 7.1 Descrambling

The pattern data is XOR-scrambled. For byte index `i` counted within one pattern's
packed data, starting at 0 at `ptr+2` and restarting for every pattern:

```
n      = (i >> 1) + 1
key(i) = (((n ^ (n >> 2)) << 3) | ((5*i + 2) & 7)) & 0xFF
```

Apply `data[ptr+2+i] ^= key(i)` for `i` in `0 .. ln-2`, where `ptr = patternPtr * 16`,
`ln = u16_le(data[ptr..ptr+2])`, and the pattern pointer table is at
`0x60 + ordNum + insNum*2` — a **byte** offset (`module` is `char far *`, so
`STARTEMS.C:333`'s address arithmetic is in bytes, and `parains`/`parapat` are `int *`,
so `:334`'s `parapat = parains + insNum` adds `insNum × 2` bytes); what is in paragraphs
is each table *entry* (`parapat[a] << 4`, `STARTEMS.C:347`).

**Verified:** MUSIC0 77/77 patterns decode as exactly 64 rows consuming exactly their
data; MUSIC1 76/77 (one cell has command byte 0; treat 0 as no-command). Zero invalid
command bytes. The resulting effect histogram reproduces an independently derived one
from another project exactly.

Two encoding traps that cost time during verification:

- S3M command bytes are **ST3 numeric**: 1–26 = `A`–`Z`, `0x30`–`0x39` = digits,
  255 = none. Not ASCII letters.
- S3M note bytes are `(octave << 4) | semitone`, valid up to 0x9B. Capping at 96
  wrongly rejects MUSIC1's high-octave material.

### 7.2 Player

A Scream Tracker 3-semantics player. The modules use a limited effect set — D (volume
slide), G (tone porta), O (sample offset), H (vibrato), E (porta down), F (porta up),
K, A (set speed), S (subcommand `SDx` note-delay only), Q (retrigger), C — which
bounds the work. Accuracy of *timing* matters more than exotic effect coverage,
because the demo's whole edit is synchronised to row position.

Mixing: reproduce STMIK's polled model. `zpollmix = 1` in the original, and the copper
calls `zpollme()` once per frame, so music advances in **frame-quantized steps**. Each
logic tick pushes exactly one frame of mixed samples and advances the tracker. The
**logic tick is the sync clock, not the audio device**; an audio thread drains a ring
buffer with a small lookahead to avoid underruns.

Sample playback rate and mix rate should default to the authentic choice (the original
mixed at 12/16/20 kHz, doubled for SB Pro) with a higher-quality option available in
settings.

### 7.3 Tempo patch

`STARTMUS.C` patches MUSIC0's inittempo from 125 to **120** at load. The port must do
the same. `zspeedadjust` / `zspeedadjust2` (45000 for MUSIC0, 48700 for MUSIC1) exist
for GUS/polled playback; determine whether they affect audible timing before
reproducing them.

### 7.4 Sync API and the `zinfo` open item

Four of five sync calls are understood from source:

- **`dis_sync()`** — hardcoded `(order<<8)|row → code 1..10` threshold table in
  `DIS/DISINT.ASM`. Reproduce verbatim, including the off-by-one.
- **`dis_musplus()`** — `muscode_6` derives it purely from `_np_row` and `_np_zplus`:
  `row` in one phase, `row-64` in the other, `-32` as the "no cue" sentinel. Every
  `while(dis_musplus() < N)` in the parts is therefore a **row-position wait**.
  `_np_zplus` is the tracker player's "plus flags", which the W32 port maps to
  `st3play_GetPlusFlags()`.
- **`dis_musrow()`** — current row; used for beat effects.
- **`dis_setmframe`/`dis_getmframe`** — frame counter with a settable origin.

**Open item: `_np_zinfo`** (behind `dis_muscode()`). Not found in the module data:
every `S` command in both modules is standard `SDx`, MUSIC0 has no cells outside
channels 0–7, and MUSIC1's only off-channel cells are five near-empty ones on
channels 10–14. It is produced inside STMIK.

Exposure: GRID for 1/2/3; HARD for 1 and 0xff. **BEGLOGO is not a dependent** — the line
that waits for `zinfo == 0xf0` is `ENDPIC/BEG.C:48`, in the build of part 4 that did not
ship; the shipped build is `BEG/`'s, which calls `dis_partstart`, `dis_waitb` and nothing
else (amended 2026-10-13: `docs/fidelity/beglogo.md` §1 settles which build shipped, from
the reference's held frame 900 and the staged EXE's size). The vault is the third
dependent and is Phase 8's. The W32 port stubbed `dis_muscode()` to `return 0` and
presumably patched the parts that call it.

Resolution ladder, cheapest first:

1. **Parse `STMIK.300`'s OMF symbol records** for the offsets of `_np_zinfo` and
   `_np_zplus` within the STMIK segment (reading a symbol table, not disassembling
   code), then read them live from the running original via `dosbox-automation`'s
   memory API and correlate against `order`/`row` frame by frame.
2. Read `st3play`'s source for the semantics of "plus flags"; `zinfo` may be a
   sibling concept.
3. Disassemble the 0x73A7-byte STMIK segment. Feasible, has symbols, last resort.

**Trigger for escalation:** if route 1 does not resolve it within Phase 3, implement
the affected parts against a substitute cue and record the difference in
`APPROXIMATIONS.md`; do not let it block the phase.

**Outcome (amended 2026-10-25, Phase 9 Task 10).** Route 1 was performed in Phase 3 and found the
word — `_np_zinfo` at segment offset `0x08F7` — and its value, **0 in every sample of every filed
trace**, but no mechanism for any other value (D8, amended 2026-10-14). The trigger was therefore
met and the substitute taken: `dis_muscode()` returns 0 (`sr_engine::music::muscode_from`),
registered as `APPROXIMATIONS.md` **A56**. Routes 2 and 3 were not taken. No part the demo runs
calls `dis_muscode`; GRID and HARD run behind the standalone DIS and model their one start key
(A40, A46, A52), so the substitute's measured difference is 0 in every compared frame.

---

## 8. Verification

### 8.1 The oracle

The reference is **an emulator that can run the original deterministically and yield
its frames**. `dosbox-automation` (a DOSBox fork exposing a REST API) is the preferred
implementation: it provides `/api/v1/video/frame` for exact frame grabs,
`/api/v1/cpu/cycles` to pin emulation speed for reproducibility,
`/api/v1/input/sequence` for scripted input, and `/api/v1/memory/*` for live variable
inspection, which is also the primary tool for resolving `_np_zinfo` (§7.4).

`xtask reference capture` therefore sits behind a small trait, not a hard dependency on
one emulator, so that an unavailable or unusable oracle degrades to a documented
alternative rather than blocking a phase. Candidate fallbacks, in order: a locally
built `dosbox-automation`; stock DOSBox-X with its scripted-command and screenshot
facilities; DOSBox-staging.

**The oracle is only needed for verification, not for development.** Engine work —
surface, view, palette, tick, the `Part` trait — is testable against synthetic
expectations and does not block on it. See §12 for the risk entry.

The original's own command line (`SECOND 2..5`, `U`) starts the demo at any block,
which is how per-part references are captured.

### 8.2 Frame comparison

`xtask reference capture` drives the emulator, captures frames at chosen indices, and
writes them under `reference/` with a manifest (commit, cycles, arguments, frame
indices). `xtask compare` runs the port headless, renders the same frame numbers, and
reports differing-pixel percentage, maximum channel delta, and a diff image.

Keyframes only, not full video — a handful per part at structurally significant
moments.

### 8.3 Event comparison

Beyond pixels, the port emits an **event stream**: palette writes, view changes
(start address / pan / split / double-scan), the scanline at which each palette change
takes effect, and sound-clock advances. The same events can be logged from the
original. This turns "the picture is slightly off" into "the line-compare write at
frame 812 was 0x3C, you produced 0x13C", and is the technique `demoscene-recomp` uses
to claim event-level correctness.

**Retired (amended 2026-10-25, Phase 9 Task 10, D47).** The event stream was not built. The oracle
exposes the original's registers only as polls between frames, so the events cannot be logged from
the original without changing the emulator, and the frame comparison (§8.2, `cargo xtask rediff`)
and the trace of the original's player state (`cargo xtask music trace`) answered what it was for.
D47 has the decision and its cost.

### 8.4 Limitations, stated honestly

- The reference is **DOSBox's rendering of the VGA, not real hardware's**. It is the
  only objective oracle available and is what every existing port uses.
- A small number of parts may prove non-deterministic under emulation. Those get
  captured across multiple runs and compared on structural similarity.
- The emulator's own Mode X / CRTC emulation fidelity is an assumption we inherit.

---

## 9. Asset handling

Original formats, unmodified, read at runtime. No conversion.

The subset the port needs — roughly 5–6 MB — is copied into `assets/` with a
provenance file recording origin, licence (Unlicense / public domain) and the
deduplication applied. The upstream tree is vendored as a **pinned git submodule** at
`upstream/SecondReality` so any "what did the original do here" question has a citable
answer, and so the repo is self-contained.

Known duplicates to collapse: `TMP.BBM` ×4, `ENDPIC/PIC.LBM` ≡ `JPLOGO/PIC.LBM`,
`END/PIC.LBM` ≡ `NUTS.LBM`, the `U2A`/`U2E` scene set duplicated between `VISU/C/SCENE`
and `MAIN/DATA`, `TROLL.UP` ×2, `O2.SCI` ×2, `SIN1024.INC` ×9.

Assets that exist only compiled into a shipped EXE are recovered from OMF `LEDATA`
records during Phase 1.

---

## 10. Phases

Ordered to retire risk early and to leave something runnable after each. Phase 3 has
no dependency on Phase 2 and may run in parallel.

| # | Phase | Exit criteria |
|---|---|---|
| 0 | Foundation & harness | `cargo test` green; a synthetic test card runs at correct speed exercising per-scanline palette, split, scroll, double-scan and beam compose; `xtask reference capture` captures frames from the original; `xtask compare` produces a diff report |
| 1 | Assets & references | every needed asset decodes and matches the source art; provenance file complete; references captured for all parts |
| 2 | Shell + easy tier | **BEGLOGO frame-diff-identical**; TUNNELI, ENDSCRL, CREDITS, ENDLOGO play; a runnable binary sequences them |
| 3 | Music engine | both modules play end-to-end; the `(order,row)` cue timeline matches the parts' expectations; `zinfo` resolved or consciously deferred with a documented workaround; `dis_getmframe` rate measured |
| 4 | Music-synced tier A | ALKU, PAM, DDSTARS, DOTS, MNTSCRL, RAYSCRL verified |
| 5 | Mid tier | PLZPART, TECHNO, LENS, PANIC, JPLOGO, COMAN verified |
| 6 | Vector engine | U2A and U2E verified |
| 7 | GLENZ | GLENZ verified |
| 8 | Vault | GRID, HARD, TWIST playable standalone and verified |
| 9 | Assembly & fidelity pass | full sequence with music restarts, cue waits and loop mode; settings screen; `APPROXIMATIONS.md` and `docs/fidelity/bugs.md` complete; every part re-diffed |

Phase 2's exit criterion is the project's central technical risk: **whether
bit-exactness is achievable at all** is answered there, not in Phase 9.

---

## 11. Resumption protocol

The project is designed to be stopped and resumed at any phase boundary.

- **`PROGRESS.md`** is the status board: per phase, its state (not started / in
  progress / done), the evidence for completion, and a "where to pick up" line.
- **`docs/plans/NN-<phase>.md`** holds one plan per phase, written **just in time**,
  before the phase starts. Plans for later phases are not written in advance — they
  would be fiction.
- **Each phase ends with:** all tests green, frame diffs recorded, `PROGRESS.md`
  updated, `docs/decisions.md` updated with anything that changed the approach, and a
  `phase-N` git tag.
- **`CLAUDE.md`** instructs any future session to read `PROGRESS.md` first, then the
  active phase plan, and documents the reference-capture workflow, the fidelity
  policy, and where the original source lives.

---

## 12. Risks

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Bit-exactness not achievable | medium | high | answered in Phase 2 with a single part; approximations register as the fallback |
| `_np_zinfo` unresolved | medium | low | three-route ladder (§7.4); GRID, HARD and the vault depend on it — not BEGLOGO, whose shipped build does not call `dis_muscode` (§7.4); hard escalation trigger |
| `dis_getmframe` real rate not reproducible | medium | medium | measured empirically in Phase 3; affects GLENZ/PLZPART/TECHNO |
| FOREST, WATER and PANIC shipped binaries have only prototype sources | high | medium | shipped EXEs exist in `MAIN/DATA/`; disassemble against reference frames |
| Emulator reference itself wrong for Mode X/CRTC tricks | low | medium | cross-check against `demoscene-recomp`'s modelled-VGA notes |
| Reference emulator unavailable or unusable | medium | medium | oracle is behind a trait (§8.1) with documented fallbacks; engine work and part implementation do not block on it, only verification gates do |
| Vector engine (Phase 6) larger than expected | medium | high | it is the largest single work item and is isolated in its own phase |
| Scope creep from the vault | medium | low | vault is Phase 8, after the main sequence works |

---

## 13. Open questions

1. **`zspeedadjust` / `zspeedadjust2`.** Whether they affect audible timing or are
   GUS-specific calibration. Resolve in Phase 3.

**Closed (amended 2026-10-25, Phase 9 Task 10): D46.** Both read 0 in the reference's run and in the
object's image, the row timer's adder therefore adds nothing, and the shell writes
`_zspeedadjust2` only for output modes 7 (no sound) and 2 (Gravis) — `MAIN/U2.ASM:384-389` — where
the reference's card is mode 5. No question in this section is open.

---

## 14. Licence

The port is **MIT licensed**, in the same permissive spirit as the original, which was
released into the public domain under the Unlicense. See `LICENSE`.

This applies to the new source code. The original media assets in `assets/` and the
vendored `upstream/SecondReality` submodule remain under their own Unlicense terms —
both are public domain, so redistribution alongside MIT code is unencumbered. The
`assets/README.md` provenance file records this per asset.

Phase 0 sets `license = "MIT"` in the workspace `Cargo.toml` and in each crate's
manifest.
