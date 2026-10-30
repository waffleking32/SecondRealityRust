# Second Reality → Rust — Design

How the port is built. The upstream source is `mtuomi/SecondReality` (public domain, Unlicense),
commit `071a82e`, vendored at `upstream/SecondReality`. Rulings that shaped the design are in
`docs/decisions.md` (D-numbers); per-part evidence is in `docs/fidelity/`.

## 1. Goal

A faithful, native Rust port of **Second Reality** (Future Crew, 1993), verified frame by frame
against the original running under emulation.

The whole demo plays in order (§2.2), music-synced, with the hidden part and loop mode; the three
unreleased effects play from a vault menu; every part is compared against captured reference frames,
pixel-identical or with each knowing difference registered (§5); the media are read in their
original formats. A settings screen replaces `START.EXE`'s sound-card questions (D44).

Non-goals: running the original x86 binaries (this is a source port, not an emulator), and an
enhanced visual mode. Linux is the verified target; a Windows build is produced by CI
(`.github/workflows/windows.yml`) but nothing verifies it. The one enhancement is opt-in audio:
`--sound-quality enhanced`, the port's own 48 kHz renderer, which never moves the music's
position (D43, A60).

## 2. What the original is

The 2013 public-domain source release: 1319 files, about 35 MB, 126 C files, 98 ASM files,
29 Turbo Pascal projects, the original art and the two music modules.

### 2.1 Runtime architecture

- **One shell plus many part programs.** `MAIN/U2.ASM` (built as `U2.EXE`, shipped renamed
  `SECOND.EXE`) is the only resident program. It statically links the file loader
  (`MAIN/U2A.ASM`), the raster scheduler (`MAIN/COPPER.ASM`), the `int 10h` video hook
  (`MAIN/VMODE.ASM`), the Demo Interrupt Server (`DIS/DISINT.ASM`) and the music player STMIK
  (`MAIN/STMIK.300`, an object with no source).
- **Parts are separate DOS EXEs**, loaded by the shell one at a time. They call back into the
  shell through `int 0xFC` (DIS).
- **The demo is music-counted, not frame-counted.** Parts run until a music-position cue, via
  `dis_sync()`, `dis_musplus()`, `dis_musrow()`, or a frame count started at a cue.

### 2.2 Part sequence

From `MAIN/U2.ASM`. `docs/fidelity/part-sequence.md` is the reference, with the citations, the
entry-point conditions and the block arguments; this table is its summary (D13).

`Order` numbers the show parts only. Slot 17 is the hidden part's, and the main sequence never
runs it.

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

The "only" rows are entry-point restarts: starting mid-demo reproduces the music position a full
run would have reached. Loop mode restarts MUSIC0 at order 0 and runs the whole sequence again
(`@@partsagain`). The block arguments are `1`–`5` (`1` is the same as none) and `U`, which
`README.1ST` never mentions; the port's `--block` takes the same (`part-sequence.md` §4).

### 2.3 Display modes

| Geometry | Where |
|---|---|
| 320×200×256, unchained planar ("Mode X" tweak) | most parts, including PAM, 3DSINFLD (chain-4 off, 80-byte plane rows), U2E (which normalises its own CRTC 9 scan-doubling) and CRED (160-byte rows, CRTC 0x13) |
| 320×400×256, planar | ALKU, BEGLOGO, DDSTARS, ENDLOGO, JPLOGO |
| 640×350×16 → 640×400 via CRTC reg 9 | ENDSCRL |
| BIOS text mode 3 | setup menu, exit screens |

No shipped part shows 640×400×256. The measured geometry of each part is in
`docs/fidelity/reference-indices.md` §2 (and §6 PAM, §16 3DSINFLD, §18 U2E); where it and this
table disagree, the capture wins.

### 2.4 Hardware behaviours that are visually significant

Everything else the original does with the VGA is a performance strategy and is discarded. These
five are not:

1. **Palette DAC written mid-frame**: fades, colour cycling, raster bars. Nearly every part.
2. **CRTC start address**: horizontal panning (ALKU), vertical scrolling (JPLOGO), the 401-line
   circular buffer (ENDSCRL).
3. **Line compare (CRTC 0x18 + 0x07 bit 4)**: split screens (PLZPART's falling screen, CRED).
4. **Double-scan / 400-line mode**: PANICEND's shrink-to-dot, the 320×400 parts.
5. **Overscan border colour (AC 0x11)**, used deliberately as a design element.

### 2.5 Music

Two Scream Tracker 3 modules: MUSIC0 "UnreaL ][ - The 2ND Reality" (80 orders, inittempo 125,
speed 6) and MUSIC1 "UnreaL ][ / PM" (100 orders, inittempo 130, speed 3), shipped inside
`REALITY.FC` as a 16-byte header plus the two files.

The pattern data is XOR-scrambled, so standard S3M decoders fail on it; the rest is a standard S3M
(§7.1). `STARTMUS.C` patches MUSIC0's initial tempo from 125 to **120** at load. There are no
separate sound-effect files: the rumble, flybys and explosion are samples inside the modules.

### 2.6 Protection and packing

`MAIN/PROT/` and `FCP/` implement a self-decrypting EXE protector, and `MAIN/PACK.C` appends every
part and data file to `SECOND.EXE`. Neither is ported: the port reads the loose files. The shipped
part EXEs in `MAIN/DATA/` are themselves compressed (PKLITE, LZEXE, EXEPACK); `sr_assets::exe`
unpacks them so a part can read the release's own data from its image (D49).

## 3. Prior art

`ConspiracyHu/SecondRealityW32` and its C++ fork stub out the copper (losing TECHNO's interference
seam), widen the palette to 8 bits and spin-wait to 70 Hz; the port does none of these (D6).
`treylorswift/demoscene-recomp` composes per scanline, which is the model §4.2 follows.

## 4. Architecture

### 4.1 Crate layout

```
crates/sr-engine/     the software VGA: surface, view, palette, copper, tick, compose, present,
                      the Part trait and the Music seam, and the engine that ticks a part
crates/sr-assets/     the original's formats: decoders, OMF reader, EXE unpacker, VISU scenes
crates/sr-audio/      the S3M player: descrambling, tracker, effects, the STMIK mixer
bins/secondreality/   the binary: shell/ (the run), cli.rs, window.rs (SDL), headless.rs,
                      audio.rs (Player as Music, the sound card, the WAV dump), settings.rs,
                      vault_menu.rs, endscreen.rs, parts.rs (the registry) and parts/
xtask/                capture, compare, rediff, asset and music tooling (§4.8)
```

`parts/` has one module per part, named by its registry name (`3dsinfld` is `coman.rs`, since an
identifier cannot start with a digit), plus `parts/visu/` (the vector engine U2A and U2E share,
D32), `parts/glenz/` (GLENZ's renderer, D36) and `parts/vault/` (GRID, HARD, TWIST; D39, D40).

The dependency rule: `sr-engine` and `sr-audio` have no runtime dependencies, and `sr-assets`
depends on `sr-engine` only; `the_engine_has_no_runtime_dependencies` and
`the_player_has_no_runtime_dependencies` read the two manifests. SDL lives only in the binary,
and the adapter between `Music` and the player is the binary's (`audio::PlayerMusic`), because the
binary is the only crate that depends on both.

### 4.2 `sr-engine` — the software VGA model

The visible output of the original is always an indexed 8-bit image at one of a few geometries,
plus a palette that can change mid-frame, plus a view into a larger canvas. Everything else is an
implementation detail of 1993 hardware.

**`Surface`** is a chunky 8-bit canvas at the largest geometry, 640×400 (`MAX_WIDTH`,
`MAX_HEIGHT`). Parts write pixels with the original's coordinate maths. The Mode X plane arithmetic
disappears, because it never changed *which* pixels were written, only how fast.

**`View`** replaces the CRTC:

```rust
pub struct View {
    pub width: u16,                // displayed width in pixels
    pub height: u16,               // displayed height in scanlines
    pub start_addr: u32,           // display start, in pixels (CRTC 0x0C/0x0D)
    pub x_offset: u16,             // pixel offset within a line, on top of start_addr
    pub y_line_offset: Option<u16>,// pixels per scanline (CRTC 0x13); None = the surface's rows
    pub pan: u8,                   // fine pan in pixels, 0..=7
    pub double_scan: bool,         // CRTC reg 9 bit 7
    pub split: Option<Box<Split>>, // line compare: the first scanline below it, and a second View
    pub border: Option<u8>,        // overscan colour (AC 0x11)
}
```

`start_addr` counts pixels, so it is `u32` (the surface is 256,000 pixels; D16). The fields' docs
carry the stride default, `pan`'s pixel unit and the split's rules.

**`Palette`** is 256 entries at **6 bits per channel**, widened to 8 only when presented, as the DAC
did. A frame carries a `PaletteTimeline` of the palette in force at each band.

**`Tick`** is a fixed 70.086 Hz logic tick, independent of display refresh (§6).

**The copper.** The original's three per-frame callbacks are named slots that a part registers
with `ctx.set_copper(slot, f)`; they are not a no-op. `Engine::tick` runs, in this order
(`MAIN/COPPER.ASM:449-468`):

```
1. copper slot 0      top of screen
2. music.poll()       the copper's `call _zpollme` (`:466`)
3. part.frame()       once per band, each band composed as soon as it is drawn
4. copper slot 1      just before retrace
5. copper slot 2      in retrace: page flips, palette setup
```

**Beam compose.** A part may draw in top-to-bottom bands (`Part::band_count`); each band is
composed with the palette current at that moment, so mid-frame VRAM and palette writes land on the
scanline the beam drew them on (TECHNO's interference, PLZPART's split).

**Presentation** (`sr_engine::present`) turns an `IndexedFrame` into RGBA through each scanline's
palette. The window's default is **2× pixel duplication with square dots**: 320×200, 320×400 and
640×400 frames are all 640×400 on the screen, and nothing interpolates (D48). `--aspect 4:3` is a
resample into a 4:3 box; the border is a flat pad (A1). A headless dump is one pixel per source
pixel, and `xtask compare` reads indexed frames, so presentation cannot affect a measurement.

### 4.3 The `Part` trait — replacing DIS

```rust
pub trait Part {
    fn name(&self) -> &'static str;
    fn init(&mut self, ctx: &mut PartCtx) -> Result<(), PartError>;
    fn frame(&mut self, ctx: &mut PartCtx) -> PartStatus; // one band; PartStatus::{Continuing, Finished}
    fn band_count(&self) -> u16 { 1 }                     // > 1 enables beam compose
    fn drains_after_exit(&self) -> bool { false }         // U2E's DAC fade after ESC
}
```

`PartCtx` is the DIS surface, minimised. Its public fields are `surface`, `view`, `palette`,
`frame`, `band`, `bands` and `exit_requested`. `Machine` (surface, view, palette, copper) builds one
with `Machine::ctx`; the engine owns one `Machine`.

| DIS service (original) | Port equivalent |
|---|---|
| `dis_version`, `dis_partstart` | gone: construction and `init` |
| `dis_waitb()` | the engine tick: `frame()` runs once a tick, so the value is always 1 (§6.2) |
| `dis_exit()` | `ctx.exit_requested`; a part asks for its own exit with `ctx.request_exit()` |
| `dis_indemo()` | not modelled: no part reads it |
| `dis_msgarea(n)` | not needed: only `STARTMUS` uses it, and the shell owns the music restarts |
| `dis_sync()` | `music.sync()`, the shipped table verbatim, off-by-one included (bugs.md) |
| `dis_musplus()`, `dis_musrow()` | `music.musplus()`, `music.musrow()`; `ctx.musplus()`, `ctx.musrow()` answer `-32` and 0 with no music |
| `dis_muscode(n)` | `music.muscode(n)`, which returns 0 (§7.4, A56) |
| `dis_setmframe`, `dis_getmframe` | `music.setmframe(n)`, `music.getmframe()` |
| `dis_setcopper` | `ctx.set_copper(slot, f)` |

`ctx.music()` returns `Option<MusicRef>`: `None` in a run with no music, which a part that waits on
a cue refuses with `ctx.require_music`. `ctx.music_fresh()` is true
only in the `init` of the part the music was just installed for, which is when a part catches the
player up to its boot frame (D31). The palette is written through `set_palette` and
`set_palette_entry`.

### 4.4 `sr-assets`

Loaders for the original formats, each writing into a `Surface` and returning its palette. The
magic bytes decide the format, never the extension; palettes stay six bits; a decoder never reads
past its input (the crate doc has the rules).

The modules: `origin` (the Origin PIC family behind `.UH`, `.U`, `.UP`, `.P`, `.16`, `.X`), `iff`
(ILBM and PBM), `rix` (RIX3, `.CLX`/`.SCI`), `pal` (headerless `.PAL`), `anim` (PAM's FLI and
`.ANI`), `font` (the text strips), `visu` (VISU's packed scenes `.00M`/`.0AA`/`.0AB`/`.00N`),
`poslist` (FOREST's and WATER's `.DAT` column lists), `omf`
(the `.OBK`/`.OBJ` container, to recover assets compiled into an object) and `exe` (the shipped
part EXEs, unpacked; D49).

Not needed: `.INC`/`.BBK`/`.CUT`/`.PRE` source fragments (a part that needs one carries its data),
and the Pascal and 3D-Studio project files.

### 4.5 `sr-audio`

A purpose-built S3M player with Scream Tracker 3 semantics that exposes STMIK's clock (§7).

### 4.6 The shell

`shell/` is the `U2.ASM` equivalent. `Sequence` holds the run: the block plans (`BLOCKS`,
`block_plan`), the boot anchors that put every frame in the reference's boot-relative numbering
(D21), the music restarts (`part-sequence.md` §3), the post-PANICEND cue wait (D38), the ESC, Ctrl
and drain semantics (A58, D35), loop mode (D42) and the run's end (D41). `window.rs` feeds it
wall-clock time and presents the newest frame; `headless.rs` ticks it and dumps frames. The settings
screen is `settings.rs` (D44).

### 4.7 The vault

GRID, HARD and TWIST, playable standalone from `--vault` or `--part`. They never shipped; they are
held to the same standard but are not in `PART_ORDER`, any block or any boot anchor. They run
silent, behind the standalone DIS (D39); the menu is window-only (D40).

### 4.8 Tooling

`cargo xtask` has `reference capture|verify|backfill` (file frames from the emulated original
under `reference/`, check the manifests), `compare` (§8.2), `rediff` (every filed frame against its
count in `xtask/src/rediff/table.rs`), `frames sheet`, `assets verify` (against
`assets/PROVENANCE.md`) and `music symbols|trace|probe|analyse` (the original's player state, read
through `STMIK.300`'s OMF symbols, D26). The binary's `--audio-dump` and `--positions` complete the
set (D47).

## 5. Fidelity policy

**Default: pixel-identical.** The reference is frames captured from the original under emulation
(§8). A part is done when its frames match.

### 5.1 Preserve the bugs

The original's artefacts are part of its look. They are required behaviour, each with a reference
frame and a pinning test in `docs/fidelity/bugs.md`, so nobody "fixes" them. Examples: the plasma's
palette-leak banding (PLZPART), shadow particles drawn over the particles in the jumping-particle
part (there is no z-buffer), and the `dis_sync()` off-by-one (it returns the previous entry's code,
and reads off the table start at order 0).

### 5.2 Approximations register

`docs/fidelity/APPROXIMATIONS.md` lists every place the port knowingly differs, with the part, the
original technique, what the port does instead, why, and the measured difference. An entry needs a
justification; it is not a parking place for "close enough".

Generated unrolled assembly is ported as the loop it unrolls, and self-modifying code reads a
variable instead. The register's "Generated and self-modifying code" section has the ten candidates
with their tests: eight are bit-identical where the reference can see them, and two
(`GLENZ/ZOOMLOOP.INC`, `HARD/LINEBLIT.INC`) are not on the shipped path.

### 5.3 Timing fidelity

The port runs at exactly 70.086 Hz and does not re-time to the display. On a 60 Hz panel some frames
show twice. There is no interpolation. The two places the original does not run at a nominal 70 Hz
are `dis_waitb()`'s return value (§6.2) and the parts keyed to `dis_getmframe()` (§6.3).

## 6. Timing model

### 6.1 The tick

A fixed 70.086 Hz tick drives everything, in §4.2's order. The window shows the newest finished
frame and drops the ticks the display fell behind on. Nothing spin-waits.

### 6.2 `dis_waitb` semantics

In the demo, `dis_waitb()` does not poll the retrace port. It waits for the copper's frame counter
to change, reads it, returns it and **then** zeroes it (`DIS/DISINT.ASM:165-169`), so a part whose
work overran *k* frames gets *k+1* (D22). Parts pace animation by it: `TUNNELI/TUN10.PAS:200` is
`for sync := 1 to frames` and `ENDSCRL/MAIN.C:47` is `if(dis_waitb()==1) dis_waitb();`. The port
calls `frame()` once a tick, so the value is always 1 and a part's pacing follows from that. The
`0x3DA` retrace poll applies only to a part run from DOS outside the demo.

### 6.3 `dis_getmframe` caveat

`_np_zframe` is the counter parts reset (`dis_setmframe(0)`) and wait on
(`while(dis_getmframe() < 300)`). It advances once per **player tick**, not per video frame: 48 a
second at tempo 120 and 52 at tempo 130, measured from the original's own player state
(`docs/fidelity/music-clock.md` §8, D26). The port's `getmframe` is the player's tick counter, so
GLENZ, PLZPART, TECHNO and the other parts keyed to it see the original's rate.

### 6.4 Music as the clock

Parts synchronise to the music position, not to frames. If the port exposes order and row correctly
and its tick model matches the original's, cue alignment follows. The critical parameters are the
tick model, the tempo patch (§7.3) and the polled mixer (§7.2), not the sync API's shape.

## 7. Music subsystem

### 7.1 Descrambling

The pattern data is XOR-scrambled. For byte index `i` counted within one pattern's packed data,
starting at 0 at `ptr+2` and restarting for every pattern:

```
n      = (i >> 1) + 1
key(i) = (((n ^ (n >> 2)) << 3) | ((5*i + 2) & 7)) & 0xFF
```

Apply `data[ptr+2+i] ^= key(i)` for `i` in `0 .. ln-2`, where `ptr = patternPtr * 16`,
`ln = u16_le(data[ptr..ptr+2])`, and the pattern pointer table is at `0x60 + ordNum + insNum*2`, a
**byte** offset (`module` is `char far *`, so `STARTEMS.C:333`'s arithmetic is in bytes, and
`parapat = parains + insNum` at `:334` adds `insNum × 2` bytes); each table *entry* is in
paragraphs (`parapat[a] << 4`, `:347`). D5 has the derivation.

Verified: all of MUSIC0's 77 patterns decode as exactly 64 rows consuming exactly their data;
MUSIC1 76/77 (one cell has command byte 0, which means no command). There are no invalid command
bytes. `every_pattern_decodes_to_exactly_sixty_four_rows` (`sr-audio/tests/module.rs`) pins it.

Two encoding traps:

- S3M command bytes are **ST3 numeric**: 1–26 = `A`–`Z`, `0x30`–`0x39` = digits, 255 = none. Not
  ASCII letters.
- S3M note bytes are `(octave << 4) | semitone`, valid up to 0x9B. Capping at 96 wrongly rejects
  MUSIC1's high-octave material.

### 7.2 Player

A Scream Tracker 3 player over the effects the modules use: `A`, `B`, `C` in the tracker
(`player`), and `D`, `E`, `F`, `G`, `H`, `J`, `K`, `O`, `Q` and `SDx` in the channel (`effects`).
Timing matters more than effect coverage, because the demo's edit is synchronised to row position.

Mixing reproduces STMIK's polled model: `_zpollmix = 1`, and the copper calls `zpollme()` once per
frame, so the music advances in frame-quantised steps (`music-clock.md` §5). Each logic tick calls
`Player::poll` once, which advances the tracker and mixes one frame of samples into a ring. **The
logic tick is the sync clock, not the audio device**: the sound card only drains the ring
(`audio::Device`). The default mixer is the original's, at the rate the reference runs: a Sound
Blaster Pro in stereo, 20,000 frames a second with `_zmixspeed` doubled to 40000 (`music-clock.md`
§5, `sr_audio::mix`); the enhanced 48 kHz renderer is opt-in (D43).

### 7.3 Tempo patch

`STARTMUS.C` patches MUSIC0's inittempo from 125 to **120** at load, and the port does the same.
`zspeedadjust` and `zspeedadjust2` both read 0 in the reference's run, so they add nothing (D46).

### 7.4 Sync API and `zinfo`

- **`dis_sync()`**: the shell's `(order<<8)|row → code` threshold table, reproduced verbatim with
  its off-by-one (`sr_engine::music::sync_from`).
- **`dis_musplus()`**: derived from `_np_row` and `_np_zplus`: `row` in one phase, `row-64` in the
  other, `-32` as the "no cue" sentinel (`musplus_from`). Every `while(dis_musplus() < N)` is a
  row-position wait.
- **`dis_musrow()`**: the current row.
- **`dis_setmframe`/`dis_getmframe`**: the tick counter with a settable origin (§6.3).
- **`dis_muscode()`** reads `_np_zinfo`, which is produced inside STMIK. The trace located it at
  segment offset `0x08F7` and read **0 in every sample of every filed trace**, with no mechanism
  for any other value (D8). The port's `muscode_from` returns 0 (A56). No part the demo runs calls
  it; BEGLOGO's `zinfo == 0xf0` wait is in `ENDPIC/BEG.C:48`, the build that did not ship. GRID and
  HARD run behind the standalone DIS and model their one start key (A40, A46, A52), so the
  measured difference is 0 in every compared frame.

## 8. Verification

### 8.1 The oracle

The reference is an emulator that runs the original deterministically and yields its frames:
`dosbox-automation`, a DOSBox fork with a REST API (`/api/v1/video/frame` for frame grabs,
`/api/v1/cpu/cycles`, `/api/v1/input/sequence`, `/api/v1/memory/*` for live variables). The capture
sits behind a trait (`xtask/src/oracle/`), not a hard dependency on one emulator, with documented
fallbacks (D10); D14 records the API as it behaves. The original's own block arguments
(`SECOND 2`..`5`, `U`) start it at a chosen block, which is how per-part references are captured.
The oracle is needed only for verification:
engine work is tested against synthetic expectations. CLAUDE.md has the capture recipe and its
gotchas.

### 8.2 Frame comparison

`reference capture` files frames at chosen indices with a manifest (commit, cycles, arguments,
indices, digests). `compare` pairs rendered frames with a capture by index and reports the
differing-pixel percentage, the maximum channel delta and a diff image; `rediff` holds every filed
frame to its registered count. References are keyframes at significant moments, not full video.

### 8.3 Event comparison

Retired: the event stream was not built; frame comparison and `music trace` answer what it was for
(D47).

### 8.4 Limitations

- The reference is DOSBox's rendering of the VGA, not real hardware's. It is the only objective
  oracle available.
- The emulator's Mode X and CRTC fidelity is an assumption the port inherits.
- Some animation phases differ between emulator sessions (D17), so the comparison searches one
  frame either side and reports the offset (D18). No structural-similarity fallback was needed
  (`reference-indices.md` §1).

## 9. Asset handling

Original formats, unmodified, read at runtime. No conversion. The subset the port needs is copied
into `assets/`, and `assets/PROVENANCE.md` records each file's origin, licence and digest, and the
duplicates collapsed (`TMP.BBM` ×4, `ENDPIC/PIC.LBM` ≡ `JPLOGO/PIC.LBM`, `END/PIC.LBM` ≡ `NUTS.LBM`,
the U2A/U2E scene set in `VISU/C/SCENE` and `MAIN/DATA`, `TROLL.UP` ×2, `O2.SCI` ×2,
`SIN1024.INC` ×9; `docs/fidelity/asset-inventory.md` §B verifies the list). Assets that exist only
compiled into an object are recovered from its OMF `LEDATA` records, and the release's own data is
read from the unpacked part EXEs (D49). `cargo xtask assets verify --tree` checks the tree against
the provenance file.

## 10–14. Retired

The phase table, resumption protocol, risk table, open questions (the last one closed by D46) and
licence section. `git show phase-9:docs/spec/2026-10-10-second-reality-rust-design.md` has them; the
licence is in CLAUDE.md and `LICENSE`.
