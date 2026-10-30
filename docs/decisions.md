# Decisions

The decisions that shape the port and that a reader would otherwise have to rediscover. Each entry
states what is decided now, why, and what pins it. Entries are kept current: a correction is folded
into the entry it corrects, and a reversed entry stays as a one-line pointer so its number still
resolves. The history is in git (the `phase-0` … `phase-9` tags).

Measured differences live in `docs/fidelity/APPROXIMATIONS.md` (A-entries), preserved bugs in
`docs/fidelity/bugs.md`, and the as-built architecture in `docs/design.md` (§-numbers).

---

## D1 — Fidelity target: faithful, with enhancements opt-in

**Decided.** The same 320×200 / 320×400 indexed output, the same fixed-point maths, palette and
timing, verified pixel for pixel by frame-diffing against emulator captures. A small number of
documented approximations are allowed, each in `APPROXIMATIONS.md`. Enhancements are opt-in and
off by default (D43, D48).

**Rejected.** Strictly pixel-perfect (it needs a register-level VGA emulator and hand-decompiled
unrolled assembly); a modern remake (a different visual identity).

## D2 — Scope: the full demo, the hidden part and the dev-parts vault

**Decided.** The 20-part sequence, the hidden DDSTARS part, a modern stand-in for the DOS setup
screen (D44), and a vault menu for the three unreleased effects GRID, HARD and TWIST (D39, D40).

## D3 — Architecture: a software VGA model and one module a part

**Decided.** A small engine owns an indexed surface, a per-scanline palette, a view transform in
place of the CRTC, a fixed 70.086 Hz tick, and a `Part` trait in place of the DIS interrupt server.
Parts are ordinary Rust modules that write indexed pixels.

**Rejected.** A flat framebuffer with per-part special cases (it pushes hardware concepts into a
dozen parts and makes frame-diffs harder); instruction-level emulation (not a port).

## D4 — Reversed by D48

The window's default is 2× square-dot duplication (D48). 4:3 is `--aspect 4:3` and the settings
screen's Aspect row.

## D5 — The music's pattern data is scrambled, and the keystream is recovered

`MAIN/MUSIC0.S3M` and `MUSIC1.S3M` (byte-identical to the copies inside `REALITY.FC`) are standard
S3Ms except that each pattern's packed data is XOR-scrambled, so stock decoders fail. For byte `i`
of a pattern's packed data, counted from 0 at `ptr+2` and restarting each pattern:

```
n      = (i >> 1) + 1
key(i) = (((n ^ (n >> 2)) << 3) | ((5*i + 2) & 7)) & 0xFF
```

XOR for `i` in `0 .. ln-2`. It decodes 77 of 77 patterns in MUSIC0 and 76 of 77 in MUSIC1 with no
invalid command. Two traps: command bytes are ST3 numeric (1–26 = A–Z, 0x30–0x39 = digits), not
ASCII; note bytes are `(octave << 4) | semitone`, valid up to 0x9B.

**Pinned by** `sr_audio::module::descramble`'s tests `the_keystream_is_d5s` and
`descramble_restarts_at_index_zero_for_every_pattern`.

## D6 — Do not copy the C++ ports' shortcuts

`SecondRealityW32` and `SecondRealityPlusPlus` stub `dis_setcopper()` and upload one palette a
frame for the whole screen, which loses TECHNO's mid-redraw interference seam; they also widen the
palette to 8 bits and spin-wait to 70 Hz, and `SecondRealityW32` stubs `dis_muscode()` to 0.

**Decided.** Keep the three copper callbacks, a 6-bit palette widened only at upload, a fixed
timestep decoupled from the display, and beam compose (scanline-banded rendering, so a part's work
lands where the beam would have drawn it). `dis_muscode` is D8's.

## D7 — Preserve the original's bugs

The GMX plasma's palette-leak banding and the particles' overdraw without a z-buffer are part of
the demo's look; a "correct" version looks wrong. They are required behaviour, listed with their
reference frames in `docs/fidelity/bugs.md`.

## D8 — `dis_muscode()` returns `_np_zinfo` as 0

**Decided.** `dis_muscode()` returns STMIK's `_np_zinfo`, and the port returns 0
(`sr_engine::music::muscode_from`), which is the reference's own reading in every sample of every
trace filed under `reference/music/`.

**Why.** `STMIK.300`'s `PUBDEF` records put `_np_zinfo` at segment offset `0x08F7` and `_np_zplus`
at `0x08F9`. Read live, `_np_zinfo` is 0 in all 11,408 samples of the four restart-recipe traces
(MUSIC0 orders 0, 25–28, 70–77; MUSIC1 orders 0, 42–48), while `_np_zplus` takes 1 and 2. The
module data holds no non-standard `S` command that could set it. `_np_zplus` is computed by the
rule in `music-clock.md` §6 (D26). The words GRID, HARD and the vault wait on (`0xf0`, 1/2/3,
`0xff`; HARD's `HARD/B.C:138`, GRID's `GRID/MAIN.C:72` and `:160`) are never produced in any
measured window, so a part that waits on them waits for ever, against the port as against the
reference. BEGLOGO does not depend on it: the shipped part 4 is
`BEG/`'s, which never calls `dis_muscode` (`beglogo.md` §1; `ENDPIC/BEG.C:48` is the unshipped
build). No part the demo runs calls `dis_muscode`, and the vault models its one start key (A40,
A46, D39).

**Pinned by** `cargo test -p sr-audio --test trace zinfo_is_zero_in_every_filed_trace`. The
difference is A56; the word's other values stay unmeasured.

## D9 — Licence: MIT

The new Rust code is MIT (`license = "MIT"` in every manifest). The vendored
`upstream/SecondReality` submodule and the media in `assets/` stay under the Unlicense / public
domain, which puts no conditions on redistribution.

## D10 — The reference oracle is behind a trait, and `xtask` sets up its environment

**Decided.** `xtask reference capture` drives the oracle through a trait (`xtask/src/oracle/`), so
an unusable emulator degrades to an alternative instead of blocking work. The order of preference
is the locally built `dosbox-automation` (0.85.1, 6d100), then DOSBox-X with scripted commands and
screenshots, then DOSBox-staging. The oracle is needed only for verification; engine and part work
never block on it.

**The invocation.** The binary has no `RPATH`/`RUNPATH` (`readelf -d`), so `xtask` sets the
child's environment at spawn time rather than installing anything:

```
LD_LIBRARY_PATH=$SR_DOSBOX/build/release-linux/_deps/{sdl3,sdl3_image,mt32emu,fluidsynth,zlib-ng,iir1}-build
SDL_VIDEODRIVER=dummy
DOSBOX_API_TOKEN=<64 hex chars>
dosbox-automation --noprimaryconf --set webserver_enabled=true
```

Do not `cmake --install`: the fetched dependencies install into `/usr/local` and would still need
the system linker configured. A contained alternative is `cmake --install --prefix <local dir>` or
`-DCMAKE_BUILD_RPATH`.

**The token.** `DOSBOX_API_TOKEN` must be exactly 64 hex characters; any other length is rejected
with a warning and the emulator silently generates its own, which the log masks, so every request
fails with 401. The spawner generates the token, passes it in the environment and sends it as
`Authorization: Bearer` (the alternative, `webserver_token_file`, is not used). The API listens on
127.0.0.1:8386; remote binding would need `webserver_allow_remote=true`.

**The capabilities** capture depends on are in `GET /api/v1/dosbox/info`'s `features`:
`cpu_control` (pinned cycles), `input` (scripted keys), `memory` (indexed reads and the music
traces) and `port_io` (the DAC); `/video/frame` is in the OpenAPI document. `debugger` is false,
and nothing depends on it.

**Pinned by** the spawn tests in `xtask/src/oracle/dosbox.rs` and `info.json` (D14).

## D11 — Folded into D10

The working invocation, the 64-hex-character token and the confirmed capabilities are D10's.

## D12 — The reference of record is the distributed 1993 release

**Decided.** Every frame diff is against the retail release in `reference/original/`, not a build
from the source tree. The source tree is the reference for *how* each effect works.

| File | Size | SHA-256 |
|---|---|---|
| `SECOND.EXE` | 1,451,093 | `a0b637b1c6729aeee8874cb7de701f608fe6ec3028fd72652a962a3a2bd25d8f` |
| `REALITY.FC` | 992,188 | `cffb5839d0660f646c2474ff46452543997c3fc9469fa64401556c2d271aa63b` |

**Why.** The upstream tree has no runnable demo: `MAIN/U2.EXE` (31,612 bytes) and `U2.EXE` (31,548)
are two developer builds of the shell. Fidelity means what people ran. The release's
`README.1ST:34-38` lists five files, of which the demo is `SECOND.EXE` and `REALITY.FC` in one
directory (`part-sequence.md` §3), and `SECOND 1`..`5` start at the beginning, the title screen,
the landscape scroller, the vector part and the end credits. `REALITY.FC` is byte-identical in the
tree and the release, so the music and art the tree ships are the shipped data.

**Where the tree and the release differ**, D24 says what to do and D49 reads the release's own
part images. `MAIN/PACKFINA.INC`'s length words match the staged file's size for 18 of its 22
EXEs; `U2A.EXE` and `U2E.EXE` carry no PKLITE header and are 2.06× and 1.93× the listed length,
`LNS&ZOOM.EXE` is four bytes off and `START.EXE` differs, so for those four the staged file is not
the packed one. Whether the other staged EXEs are the ones `SECOND.EXE` packs is answered for
ENDSCRL (D49) and otherwise left to the frames.

## D13 — The shell's music restarts and block arguments, read from `MAIN/U2.ASM`

`MAIN/U2.ASM` is transcribed into `docs/fidelity/part-sequence.md` with a citation a row. The part
order, the binary names, `partsmask[8] = 0x80` for `SECOND U` and the MUSIC0/MUSIC1 orders are as
`design.md` §2.2 has them.

**The restart sites.** The shell's `restartmus` calls are of two kinds:

- *Entry-point restarts*, guarded by two tests so they fire only when the blocks before them did
  not run: MUSIC1 at order 42 for `SECOND 3` (`:856-863`, `& 4` and not `& 2`), MUSIC0 at order 25
  for `SECOND 5` (`:898-905`, `& 16` and not `& 8`), and MUSIC0 at 70 for `SECOND U` (`:889-894`,
  `& 80h`, which only `partsmask[8]` carries). The run's first module is chosen at `:767-775`:
  MUSIC0 for block 1, MUSIC1 at order 0 for blocks 2–5.
- *Mid-run restarts*: MUSIC1 at order 0 between BEGLOGO and GLENZ in block 1 (`:813-818`, `& 2`
  and `& 1`), and MUSIC0 at order 18 before U2E (`:880-885`, `& 8` alone), which fires mid-run for
  blocks 1–3 and is the entry point of `SECOND 4`.

The port implements the first kind as one row an entry point (`shell::MUSIC_RESTARTS`, D27) and the
second as `shell::MUSIC_RESTART_AT` (D34, D38). The demo is music-counted, so a part that starts at
the wrong music position drifts against every later one; the capture harness records the argument
it started the original with.

**Block arguments.** `README.1ST:66-79` documents `1`–`5`, and the code agrees. The code also
accepts `U` (`checkcmdline`, `:613-616`, `:626-627`), which the README omits, and its error message
(`:557-559`, "a number between 2 and 5") contradicts both. The port follows the code: `1`–`5` and
`U`/`u` with the code's masks, `1` and no argument the same, `/gravis` as D42 says, and any other
`/` switch ignored so a command line lifted from the original still runs.

**The inter-block wait** keys on `dx` from `dis_musplus()`: `_np_row` in one phase and `row − 64`
in the other, −32 for no cue (`DIS/DISINT.ASM:252-271`, `music-clock.md` §6). The wait after
PANICEND is D38's; its exit is +2 frames against the original in the full run (A53).

**Pinned by** `MUSIC_RESTARTS`' byte-reading test (D27) and the shell's chain tests.

## D14 — The reference oracle's API, as it behaves

Measured against `dosbox-automation 0.85.1 (6d100)` running `reference/original/SECOND.EXE`; the
code is `xtask/src/oracle/dosbox.rs`. The OpenAPI document is at `/openapi.json` (not under
`/api/v1/`).

- **Mounting.** `mount_allowed_bases` is read only from the *primary* config; `-conf` and `--set`
  are ignored for it (`src/webserver/webserver.cpp:666-675`). The spawn points `XDG_CONFIG_HOME` at
  a scratch directory whose `dosbox-automation/dosbox-automation.conf` names the reference
  directory, and mounts it with `MOUNT C -RO`. The demo writes nothing.
- **`GET /api/v1/video/frame?format=raw`** is a 15-byte header, little-endian
  `u32 width, u32 height, i32 pitch, u8 pixel_format, u16 palette_count`, then `palette_count * 3`
  palette bytes, then `height * |pitch|` pixel bytes. In every mode the demo uses,
  `pixel_format = 32` (BGRX: `[blue, green, red, 0]`) and `palette_count = 0`, so this endpoint
  never returns indices. The default format is lossy JPEG; there is no BMP. Geometry follows the
  guest: the title screen is a genuine 320×400 frame, and `mode=rendered` is a scaled image with no
  pixel-exact mapping back.
- **Indices** come from `GET /api/v1/memory/{segment}/{offset}/{len}` (`len` decimal), which
  honours the graphics controller's Read Map Select (index 4 at `0x3CE`/`0x3CF`). Chain-4 is bit 3
  of sequencer register 4 (write 4 to `0x3C4`, read `0x3C5`). Chain-4: `idx(x,y) = mem[y*320 + x]`.
  Mode X: `idx(x,y) = plane[x % 4][y*80 + x/4]`, each plane read with the map selected.
- **The DAC.** `PUT /api/v1/io/port {"port": 967, "value": n}` (JSON integers, not hex strings)
  selects entry `n`; three `GET /api/v1/io/port?port=969` read R, G, B as 6-bit values, and the read
  index advances after each triple, so every entry is selected before its triple. A full palette is
  1,024 requests. Any register poke is meaningful only while the screen holds still.
- **Widening.** The emulator widens with `(c * 259 + 33) >> 6`, which is `round(255c / 63)`
  (`src/utils/rgb.h:22-28`, `vga_dac.cpp:158-160`). `sr_engine::palette::dac_expand` uses the same
  rule. Bit replication, `(c << 2) | (c >> 4)`, agrees on 54 of the 64 values and differs on ten:
  11–15 (one low) and 48–52 (one high); `dac_expand`'s tests pin 15 → 61, 48 → 194 and 52 → 210,
  where replication gives 60, 195 and 211. It affects only the RGB path; indexed comparison does
  not see it.
- **Cycles** are per millisecond (valid 50–2,000,000). Start with `--set cpu_cycles=N`: `cycles=N`
  puts the emulator in legacy mode, where `PUT /api/v1/cpu/cycles` returns 409.
- **Input.** `POST /api/v1/input/type` sends paced ASCII (`\n` is Enter, `\r` is dropped) and
  returns before the keys drain. `POST /api/v1/input/sequence` takes `KBD_KEYS` names (`KBD_esc`,
  not `KBD_escape`). One injection runs at a time; another fails with "Replay already in progress".
- **Scripts.** `POST /api/v1/script/load?name=X` needs `Content-Type: text/plain` or
  `application/lua` (415 otherwise) and is rate-limited to one call per 2,000 ms (429). Lua has
  `dosbox.wait_frames(n)` and `dosbox.frame()`. There is no pause or single-step endpoint.
- **Frame indices.** Frame 0 is the emulator's first rendered frame after process start; the
  counter advances at the DOS rate. It is readable only from a resident script (`frame` in
  `/script/status`), not from `/video/frame` or `/status`. The capture records its origin; the
  boot's landmarks are D17's.
- **The setup screen.** `SECOND n` does not skip it. The default Gravis fails on this emulator
  (which has an SB16 and no GUS), and one `KBD_left` selects SoundBlaster Pro. Under the typed
  recipe the first graphics frame follows Enter by about 0.26 s.

**Fixtures** in `xtask/tests/fixtures/`, recorded from the live emulator: `info.json`, `frame.bin`
(a raw frame) with `frame_info.json` (its `video/frame/info`), `fb.bin` (its 64,000 indices),
`dac.bin` (its 256 triples), `dac_port.json` (the literal port requests behind them), and
`modex_title_frame.bin` with `modex_title_planes.bin` and `modex_title_frame_info.json` (the
320×400 Mode X title screen, its four planes and its info). They check themselves: every pixel of
the frame is the widened DAC entry its index names. **Pinned by** the fixture tests in
`xtask/src/oracle/`.

## D15 — Capture is deterministic in demo time, records only what repeats

**Decided.**

- A capture records its index origin (`"frame_origin": "boot"`) and `skip_boot_frames`, because the
  demo is deterministic in demo-relative time and a boot index is only as exact as the boot (D17).
- One capture holds one geometry. `video/frame` follows the guest, so a frame list that changes
  geometry is refused (`GeometryChanged`) rather than written under a wrong `width`/`height`.
- The palette records only the entries the frame's indices use. A running demo rewrites unused
  entries between frames, so a 256-entry snapshot does not repeat.
- An indexed read takes about a third of a second (the DAC's 1,024 requests dominate, D19), so the
  capture checks the indices against the RGB frame (`indexed_matches_rgb`) and keeps them only if
  they agree.

**Pinned by** the capture tests in `xtask/src/reference.rs` and `xtask/src/oracle/`.

## D16 — The engine's interface conventions

1. **Presentation lives in `bins/secondreality`, not `sr-engine`.** The engine keeps the raster
   maths and zero runtime dependencies, so it is testable without SDL. A test in
   `crates/sr-engine/src/lib.rs` asserts the empty `[dependencies]`.
2. **`View` has a `width`**: 320- and 640-wide displays both exist.
3. **`View::split` is `Option<Box<Split>>`.** The recursion needs the `Box`, so `View` and `Split`
   are `Clone`, not `Copy`.
4. **`Part` has `name()`**, and `init` returns `Result<(), PartError>`, so the shell can name a part
   that failed to start.
5. Music reaches a part through `PartCtx` (D27, D31).
6. **The overscan border is a fixed pad in the presenter**, 8 source pixels a side, scaled with the
   picture. The parts' CRTC timing gives 8 / 0 / 3.5 / 3.5 (left, right, top, bottom) at 320×200,
   not whole at every geometry, so the pad is the port's presentation choice (A1).
7. `xtask` runs as `cargo xtask` through `.cargo/config.toml`.
8. **The composed frame is the displayed area only.** `IndexedFrame` is `view.width × view.height`
   and is resized from the view every tick, so a part can change its line count (double scan).
9. The reference of record is the release (D12).
10. `rstest` is a dev-dependency only; the zero-dependency rule is about runtime.

**The split.** `Split::scanline` is the first scanline the lower view draws, counted within the
enclosing view. The hardware's line-compare register names the last line *above* the split (the
emulator applies `(line_compare + 1) / lines_scaled - vblank_skip`, `vga_draw.cpp:1586-1593`), so a
value copied from assembly is one less than `Split::scanline`. Measured on PLZPART: `pompota`
alternates CRTC 0x18 between 60 and 61 and the lower view starts at 62 / 61 / 62; `do_drop`'s 194 /
201 / 208 sit one below the measured rows 195 / 202 / 209 (`reference-indices.md` §1.9, §13.2). The
`lines_scaled` and `vblank_skip` terms have not been met. The lower view's origin is 0 and its pan
cleared (`VGA_ProcessSplit`, `vga_draw.cpp:1139-1158`), which is what `View::new` gives; a faithful
part leaves them alone. Below a split the scanline count restarts (`View::effective_at`). A lower
view's `width` and `height` are never read; its `border` is.

**The stride.** `y_line_offset` is `Option<u16>`, and `None` resolves to the surface's row stride
per effective view. Set it only to pack rows more tightly (CRTC 0x13).

**The origin.** `View::start_addr` is a `u32` pixel address and `set_origin` writes it and zeroes
`x_offset` (a `u16` offset within a line). The surface is 256,000 pixels: CRED starts at 128,000
(`tw_setstart(160*200)`, `CREDITS/MAIN.C:167`) and ENDSCRL's ring reaches 256,000, beyond two
`u16`s. The CRTC's 16 bits count bytes within a plane, which the pixel-addressed `View` does not
model. The test card's split walk (`SPLIT_WALK_ROWS`) is a plain 3.

**`compare`'s output guard** collapses `.`/`..` lexically after canonicalising the deepest existing
ancestor (`compare::resolve`, `collapse`, `refuse_output_inside`), so
`--out <missing>/../<reference>/x` is refused. **Pinned by**
`an_out_through_dot_dot_into_the_reference_is_refused` in `xtask/tests/rediff.rs`.

## D17 — A boot that repeats

**Decided.** Every reference is captured with `--boot autoexec --cycles 30000` and the emulator run
silent. With both, the same boot index is the same screen in every run.

**Route 1: `[autoexec]` + `AUTOTYPE`** (`BootRecipe::Autoexec`). The primary config's `[autoexec]`
runs `MOUNT C -RO <dir>`, `C:`, `AUTOTYPE -w 5 left enter` and `SECOND.EXE <block>`. `AUTOTYPE`
comes before the program because it schedules its presses on the emulated clock and returns
(`gui/mapper.cpp:1220-1244`), so nothing in the boot waits on the host. Measured on the emulator's
own clock by a Lua script: the setup menu is at frame **16** and the first graphics frame at **404**
in five runs of five. The typed recipe reaches the same screens 324 frames earlier, with the host's
jitter (D15).

**Two key vocabularies.** `--keys` takes the API's `KBD_KEYS` names, `AUTOTYPE` the mapper's button
names, and 27 of the 105 differ beyond the prefix (`KBD_backspace` is `bspace`, `KBD_kp8` is
`kp_8`). `AUTOTYPE` presses nothing for an unknown name and only logs it (`gui/mapper.cpp:209-222`).
`KEY_NAMES` in `xtask/src/reference.rs` is the pairing, from the mapper's tables and the API's
`key_name_map`. Every key is validated before anything starts (exit 2, naming it), and a capture
waits for the demo's first graphics frame, so keys that leave the setup screen up are refused.

**Silence.** With a real audio device the mixer follows the host's audio clock and the animation
drifts against the frame counter (two captures 14–35 % apart). `emulator_command` sets
`SDL_AUDIODRIVER=dummy` and passes `--set mixer_nosound=true`. **Known bug in the tool:** the
emulator's setting is `nosound` (`mixer.cpp:2943-2946`: sound still emulated, no host output), not
`mixer_nosound`, so the second half of the silent recipe may be
doing nothing. The determinism measured here is that of the command as it stands; fixing the name
needs a re-measurement before any reference is re-captured.

**The grab is identified.** The counter is read on both sides of a raw-frame grab, and the emulator
publishes a frame's pixels before it counts it, on one thread (`render.cpp:446`,
`sdl_gui.cpp:1205`), so an unchanged reading names the frame published. A grab that straddles a
frame is discarded, and a frame that arrives late is refused, not relabelled. A video-mode stop
publishes nothing (`GFX_Stop`), which `GeometryChanged` catches where the geometry changes.

**What still does not repeat** is the per-session alignment of the counter against the continuous
drawing. All indices of one session share one alignment, and a second session lands one animation
frame away about half the time (four sessions: two off at every index, two matching). Discrete guest
events (the BIOS video-mode byte at 2, 404, 4796) do not move. The likely cause is
`adjust_ticks_after_present_frame` (`gui/sdl_gui.cpp:2555-2570`), which takes measured host time off
the emulated tick budget; nothing in the API or the settings reaches it, and `mode=rendered`, the
recorder and Lua do not give a frame-locked framebuffer either. Held screens repeat byte for byte.
`compare`'s ±1 search (D18) and the support frames (D19) handle the rest.

A boot to frame 404 takes about 5.9 s, including the 5 s `AUTOTYPE` wait.

## D18 — `compare` searches a frame either side, and says which offset matched

**Decided.** For each reference index in ascending order, the candidates are the rendered frames at
that index, one below and one above, that no earlier reference frame has taken. The candidate with
the fewest differing pixels wins; ties go to the smaller offset, then the lower index. A rendered
frame answers one reference frame. A reference index with no candidate is refused (exit 2) with the
nearest rendered index named. The window is the constant `ALIGNMENT_WINDOW = 1`, not a flag.

**Why.** A moving screen captured at one boot index lands one animation frame apart between sessions
(D17), in the emulated clock, out of reach. The search widens the *alignment*, never the pixels: a
pair is 0.0000 % or a difference. Pairing is by index, not by count or position, because the port's
dump holds every tick while a capture holds a few keyframes.

**The report** (version 2) gives each frame's signed `offset` (`rendered_index - index`) and the
capture's `offset_frames` and `offsets` distribution, and the console prints both. Offsets scattered
across ±1 are the oracle's ambiguity. A constant non-zero offset is a suspicion about the port, which
a second capture settles: the oracle's ambiguity moves between sessions, a port bug does not.
`reference-indices.md` records each part's offsets.

**Limit.** The search aligns only frames the rendered side holds, so a capture compared against
another capture must carry the neighbours (D19).

**Pinned by** `compare::tests::a_frame_one_index_later_is_found_and_its_offset_is_reported`.

## D19 — A capture carries its own neighbours

**Decided.** `reference capture` takes every frame of a run of indices closer together than
`INDEXED_READ_FRAMES` (25) before it reads any of them; the kept frames wait in a queue. The read
itself is unchanged. Frames far apart behave as before.

**Why.** A grab costs about 1 ms. The indexed read costs about 23 frames (the DAC's 1,024 requests
are 260–320 ms; a Mode X plane read 10 ms; writing a 128 KB index file 20 ms), and when it sat
between two grabs, no index within ~25 frames of the previous one could be taken.

**The convention is asymmetric.** The reference holds keyframes only. The capture compared against
it holds `K-1, K, K+1` around every keyframe that moves and nothing around one that holds
(`reference-indices.md` §1.3 rules 1–2, §1.4). A session's phase is uniform, so an interior moving
keyframe answered from its own index pairs at offset 0 and reports the one-step difference. And
`compare` pairs every reference frame, so support frames in the reference would themselves need
support: two captures both holding `N-1 … N+1` refuse at the outermost index. Support frames go in
scratch captures under `out/`, never in `reference/parts/`.

**Leave room after a support run.** A run of *k* frames spends ~20·*k* frames being read, so a
distant index listed less than ~25·*k* frames after it is refused.

**Residues are recorded, not thresholded.** A re-capture can align and still differ by a few dozen
pixels in one session; the number goes in the part's row.

**Fit corpora.** A run of reference frames committed so that a fit or a whole-frame measurement can
be re-run from a clone (`reference/parts/plzpart/band/`, `reference-indices.md` §1.13) is a third
kind of reference beside keyframes and traces. It lives under `reference/parts/<part>/` with
`sha256`s that `reference verify` checks (D23), is not in `reference-indices.md` §2's table, and is
named by what consumes it (for `band/`, `cargo xtask rediff`'s `plzpart/band` entry, whose counts
A23 registers). A difference at one of its indices is a residual, not a keyframe verdict.

## D20 — What a reference index means, and the ladder for a part that does not match

The full statement is `reference-indices.md` §1.12.

**An index is boot-relative and valid under one recipe.** Every capture under `reference/parts/`
used `--arg <block> --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000` with the emulator
spawned silent (D17). A capture without pinned cycles is refused. Every manifest records `boot`,
`cycles`, `keys` and `arguments`, and a frame set that says otherwise is not comparable.

**Three claims of different strength.** The boot repeats exactly. A held screen repeats byte for
byte. A moving screen's alignment falls into discrete groups (D17), which `compare`'s ±1 search and
D19's support frames handle.

**No structural-similarity fallback.** `design.md` §8.4's fallback is not used: it would give up
exactness. Where ±1 cannot reach a difference, the residue is recorded and named; where no picture
holds a majority, the keyframe is recorded as having no settled content (§1.8 rule 4).

**The ladder.** Census the part over every session holding its window (§1.8) and re-capture it into
the largest alignment group; judge the re-capture against independent sessions, never one from the
same sitting (§1.8 rule 5). If the largest group is under half, file the best picture and record
the keyframe as unsettled. Never: a pixel tolerance, a wider window, a similarity measure, or an
unrecorded re-roll.

**Known limits.**

- *A rare oracle excursion*: one session had TECHNO's rings 30–40 frames early. It has not
  reproduced, and it is not the music clock or a counter stall.
- *ENDSCRL's census is an offset-0 count*: the tool cannot take two frames one index apart on its
  640×400 screen, so a one-frame-out diff there is a difference, not an alignment (A11).
- *A CRTC read race*: `chain4_enabled()` and `crtc_offset_register()` in
  `xtask/src/oracle/dosbox.rs` read the index/data pair in two requests with the guest running, so
  a read can see the other register or a half-written state. The loud case refuses the frame; the
  quiet case can pass on a palette-collapsing screen. The fix is to read the pair twice and require
  agreement.

## D21 — The shell: one run index, the DAC it starts from, and the exit flag

**One index for the whole run.** `shell::Sequence` owns one `u32` beside the engine and advances it
by one a tick whoever draws, so the index is continuous across part boundaries. `boot_anchor`
supplies the start where the reference measures a first frame (D30). `Sequence::index()` is the
index the *next* tick draws at; the frame `frame()` holds was drawn at `index() - 1`, and
`--dump-frames` compares against that. A run that does not begin at an anchor numbers its frames
from zero, `index_is_reference()` is false, and the dump manifest says `index_origin: "part"`,
`anchor: null`.

**A run holds where its parts end.** When the plan is done, or its next part cannot be built, the
shell ticks an internal part that draws nothing and leaves the surface, the palette and the view
alone, and the index keeps counting. `Engine::start` clears the copper slots, so the previous part's
callbacks stop during a hold. A held frame carries no mark that nothing drew it, so a dump index
inside a hold is answered with the last part's screen. `Finished` is reserved for a run that cannot
start a part it has.

**The DAC starts where the hardware starts.** BEGLOGO's keyframe 430 is index 15 over the DAC the
machine already had (`BEG/BEG.C:20-34`). The shell installs the table the emulator's mode set
leaves: `INT10_SetVideoMode`'s `M_VGA` case writes `palette.vga` (`src/ints/int10_modes.cpp:
1977-1987`, `:1152-1159`), whose entries 0–15 are `cga_colors_default` (`:404-412`). It is **248**
entries; the last eight stay black. The reference's frame 430 records entry 15 as `(63, 63, 63)`.
`Engine::start` never resets the palette, because the original's parts inherit the DAC.

**The exit flag.** ESC sets the DIS's `exitflag` (`DIS/DISINT.ASM:112`, `:151-153`) and `exit_2`
(`:186-190`) hands it to the part's loop. The port's door is `Engine::request_exit`; the flag is
sticky in every later context until the next `start`. Parts poll `ctx.exit_requested` where their
sources call `dis_exit()`; for a part that does not poll, the shell retires it at the end of the
tick the flag was set in. A part that runs code after its loop opts into
`Part::drains_after_exit` (D35).

**Between parts, and Ctrl.** An ESC pressed when no part is running is kept by `window::EscapeKey`
and asked of the next part, or closes the window when nothing is left; the original's `partexecute`
calls `flushkbd` after each part (`MAIN/U2.ASM:211`, `MAIN/U2A.ASM:537-546`) and drops it. Ctrl
skips the running part and the run goes on with the next; the original's `ctrldown`
(`MAIN/U2.ASM:185-207`) is sticky and skips every later part. Both are knowing differences,
registered as A58.

**Pinned by** `a_request_from_outside_the_part_reaches_the_parts_own_context` (engine) and
`the_exit_flag_asks_the_running_part_to_quit_and_the_run_goes_on` (shell).

## D22 — `dis_waitb` counts the overrun

**Decided.** `waitb_1` (`DIS/DISINT.ASM:161-170`) reads `copperframecount` after it changes,
returns it, and zeroes the counter after the read, so a part that overran *k* frames gets *k+1*.
`design.md` §6.2 is corrected to match. Two call sites depend on it: `TUNNELI/TUN10.PAS:200` takes
that many animation steps, and `ENDSCRL/MAIN.C:47` waits again only when the return is exactly 1.

**The port.** One tick is one frame and `frame()` runs once a tick, so the value is always 1 and a
part takes one animation step a call. A part that assumes another step count is wrong about its own
animation, which the keyframe diffs show.

## D23 — The reference record is machine-readable, and a digest covers both files of a frame

**Decided.** `reference capture` writes `sha256` (the RGB frame) and `idx_sha256` (the index plane,
only when one is written) into each frame entry of its manifest. `manifest.json` stays version 1:
the fields are additive. `cargo xtask reference verify --tree <DIR>` checks every capture against
its own manifest: every named file present at the length its geometry implies, every `frame_*` file
named, every digest matching. Exit 0 clean, 1 any finding, 2 could not check. A frame with no
recorded digest is a finding, not a pass. `reference backfill --tree <DIR>` fills missing digests
and refuses one that disagrees with its file, so a changed frame needs a re-capture.

**Why `idx_sha256`.** `compare` measures with the indices whenever both sides have them, so a byte
changed in an `.idx` changes every later diff, and an RGB digest alone would miss it.

**Writing inside `reference/`** is this tool's narrow exception: input-only protects the frames,
and this writes metadata beside them.

**Pinned by** `xtask/tests/verify.rs` over the `verify-capture/` fixture.

## D24 — The vendored tree and the release are different builds: the procedure

When a reference frame disagrees with a faithful port of the tree, follow these steps in order
before changing anything. The step numbers are cited from `APPROXIMATIONS.md` and the part notes.

1. **Measure what the release does** from its own frames: an exact match over the whole candidate
   range for arithmetic, a read-off for data, or a rule that predicts frames it was not fitted to. A
   fit to the test frames is not a measurement (D25). Before blaming the build, check that the
   reference's geometry and timing are modelled (scanlines per row, line count, page size). A fit
   that recovers an exact rational factor points at the capture, not the build: PLZPART's cube
   looked 1.5× tall until CRTC 0x09 = 2 (`PLZPART/TWEAK.ASM:31-33`, three scanlines a row) was
   modelled, and the release then matched the tree (A23). Behaviour measured at one emulated CPU
   speed is a hypothesis until a second speed agrees: U2A's pre-gate neon appears only at 30,000
   cycles, and 15,000, 25,000, 35,000 and 60,000 agree on the normal landscape, which the port
   shows (A32, `reference-indices.md` §27).
2. **Check whether the tree can produce it at all.** If it cannot (a string no source holds, a
   pitch the loop bound forbids, a ramp the tree's own binary compiles differently), the difference
   is the release's. If it can, the port has a bug.
3. **Decide.** When the difference is data or code inside a shipped part EXE, first read the
   release's image (D49): that is the release's own artifact, not a fit. For what the image cannot
   answer:
   - **(a) Adopt** the release's value when the difference is one expression or constant the
     tree's algorithm can carry and every other difference in the part is accounted for. TUNNELI:
     `TUN10.PAS:122` writes `(64-x)*3 div 4` and the frames use `div 5`, exact at all 42 entries
     the four frames use and unique among 81 candidate pairs (A6, `tunneli.md` §5). Port the value
     with its derivation as a permanent test.
   - **(b) Register** and keep the tree's translation for timing, CPU cost, the emulator's session
     phase, or anything the image does not hold. Registering means an `APPROXIMATIONS.md` entry
     with the per-frame numbers and the evidence for the attribution. A divergence with no entry is
     the failure this procedure exists to prevent.

CRED's captions and ENDSCRL's pitch, control lines, font order and raster are read from the images
(D49), and both parts match their keyframes (A10 retired, A11).

**Not established.** The direction: nothing dates the shipped build, so the word is *different*,
not *later*.

## D25 — Every number in a doc is reproducible, and an unobservable claim says so

**Decided.** Every number and every absolute in a fidelity note or register is reproducible by
`cargo xtask rediff` (the table in `xtask/src/rediff/table.rs`), by a test that stays in the tree
(named), or by a named command. "Never", "every", "all" and "only" are claims about a whole space
and need a measurement over that space. Where no measurement can exist (an off-screen write, a
difference whose other side is unreadable), the note says so. A measured constant is derived by an
exhaustive match over its whole range, required to be unique, never by a fit to a sample (446, 451,
1147 and TUNNELI's 5 are unique; the mean-fits gave 441, 450 and 1145). A constant is not
re-anchored to make a diff pass.

**The shapes this rule catches**: an absolute over an unmeasured space; a count that does not
reproduce; a proof of something unobservable; an unknown presented as a known; a number stated once,
in prose. When a re-run disagrees with a note, the note is corrected in the same commit.

## D26 — The music clock, measured from the reference's own player state

The emulator runs silent (D17), so the oracle for the music is a per-frame trace of the original's
player words. `MAIN/STMIK.300`'s `PUBDEF` records give the offsets of `MAIN/STMIK.H:125-131`'s
position words; the segment is found in guest memory by two 32-byte runs from the object's own
`LEDATA`. Traces are under `reference/music/` and `cargo xtask music {symbols,trace,analyse,probe}`
prints every number below (`music-clock.md` names each command).

- **The clock starts at frame 408** in all four restart-recipe sessions (spread 0.1 frames),
  back-extrapolated from the first row change by `_np_zframe`. A tick is `2.5/tempo` seconds and a
  row `speed` ticks: the longest window (296 rows over 3,455 frames) is predicted to −2.6 frames, and
  every steady run's residual is under one row.
- **`_np_zframe` counts ticks** and is reset mid-song by `_dis_setmframe(0)`, which parts call.
- **The speed word after a restart is not the file's byte**: with `--arg 3` MUSIC1 plays at speed 6
  until frame 2472 where the header says 3, then at 3. A speed change is followed by a transient.
- **The machine words move**: `_zmixspeed` goes 20000 → 40000 at the music's start, `_zpollmix`
  0 → 1 (polled mode, `MAIN/COPPER.ASM:466` calling `zpollme` once a frame, which is the port's
  `poll()` a tick), and the volumes walk under `fademusic`. Manifests record the reading at the
  locate and at the end.
- **The order list**: `254` is stepped over; a `255` is never played, because the shell's restarts
  pre-empt them, so nothing is measured about one.
- **`reference-indices.md` §1.7's counter** at physical `0x105B0E` is an unnamed word at segment
  offset `0x091E`, not `_np_zframe`; it ticks at the same rate but is never reset.
- **The DOS extender maps the segment twice**, at `0x0051F0` and `0x1051F0`; `music trace` reads
  through the lowest only when the copies are proved identical.
- **A trace is its own kind of reference**: `"kind": "music-trace"`; `reference verify` recomputes
  its coverage from the file field by field and refuses an unknown `kind`. Every trace misses a
  boundary run and a frame or two in the middle, and its manifest's `coverage` says which.

## D27 — The run's music: the entry point decides the module, the anchor decides the clock

**The restart table** is `shell::MUSIC_RESTARTS`, one row an entry point with the module, the
order, the `whattorun` guard and the `MAIN/U2.ASM` lines: `SECOND 1` → MUSIC0 at 0
(`:766-775`, `:785-787`), blocks 2–5 → MUSIC1 at 0 (`:772-775`), `SECOND 3` → MUSIC1 at 42
(`:856-863`), `SECOND 4` → MUSIC0 at 18 (`:880-885`), `SECOND U` → MUSIC0 at 70 (`:889-894`),
`SECOND 5` → MUSIC0 at 25 (`:898-905`). Its test reads the upstream file as bytes (it is not UTF-8)
and checks each row against its cited instructions and its block's `partsmask`. A part no entry
point starts at plays from the demo's beginning; the test card has no music.

**The clock.** The tick clock starts when the copper starts polling (`_zpollmix = 1`, `:783`),
twelve frames after the module loads. A run anchored at or before its recipe's first tick *waits*:
the player is loaded and seeked and advances from that frame. A run anchored after it *pre-polls*
the frames in between, because the music's advance does not depend on drawing. The start frames are
D26's 408 and D30's 400.

**What it costs.** At frames 404–407 the port's speed and tempo words read MUSIC0's 6/120 where the
trace still reads MUSIC1's 3/130; the position words a part reads are identical, so this is not
modelled. A `--block 5` run's published `(order, row)` agrees with `second5` to within two frames.

**The seek clears the tick-in-row counter**, as `_zplaysong` does (`movw $0x0,0x2000`), so a
mid-song restart reads the new order's row 0. Pinned by
`a_seek_mid_song_clears_the_tick_counter_so_the_new_rows_cells_are_read`.

**The device is a drain.** SDL audio opens only in a window, at the mixer's own rate with no
allowed changes, and its callback copies what the ring holds and fills the rest with silence: never
a stall, a repeat or a resample, because the music's position is the tick's. `--no-audio` gives the
same state, `--audio-dump` writes the samples as a 16-bit stereo WAV, and `--headless` has no
device; the frames are byte-identical in all three.

**The fade** at the end of a run is `fademusic` (`MAIN/U2.ASM:333-346`): the module's global-volume
byte (0x40) steps down by one a tick (D41).

## D28 — A part's music read at the tick boundary is one tick behind

**Decided.** In the original, a check right after `dis_waitb()` stands at the vertical retrace
(`copper_end`, `MAIN/COPPER.ASM:283-320`), while the music poll runs a fifth of a frame later
(`call _zpollme`, `:466`, at 20 of the frame's 100 copper units), so the check reads the words the
*previous* frame's poll published. The port's engine polls before the part's code in the same tick,
so a part that reads the music as the first act of its frame keeps the reading for one tick and
tests that (ENDLOGO's gate, `END/END.C:107-111`). Any part that reads the music first in its frame
has the same one-tick relationship.

**One frame is a disclosed alignment.** On the reference's boot the gate would break at 1145; the
reference's frames put the break at 1146. ENDLOGO holds one extra tick, `GATE_ALIGNMENT_TICKS`. The
difference belongs to the session (D17): the port's player matches `second5`, and `second5`'s row 49
begins one frame earlier than in the session the keyframes were captured in. It is inside the ±3
frames A12 measures between two sessions of one recipe. The constant is a fact about this boot and
`second5`, and no other part inherits it (D30).

**Pinned by** `the_runs_music_is_where_the_trace_holds_it_at_every_frame_the_trace_has`
(`ZFRAME_SLOP`) and `endlogo.md` §5.

## D29 — Searching and editing the record

Search `upstream/` with `grep -a`: its `.ASM` sources are not UTF-8 (`MAIN/U2.ASM` is ISO-8859 with
CRLF), and a search that skips binary files misses them. A claim that rests on a search names the
search. A text edit is checked by searching the tree afterwards, and a correction is swept over the
retracted wording everywhere, not fixed only where it was reported (D25).

## D30 — Block 1's clock, anchors only where a first frame is measured, and no inherited alignment

1. **Block 1's clock starts at 400.** `SECOND 1` never takes a block restart: `MAIN/U2.ASM:766-775`
   loads MUSIC0 and `:786`'s `_zplaysong(0)` starts it. `reference/music/second1` fits its first
   row change to a start at 399.6, and `second1b` confirms the row at 7710. The shell's
   `BLOCK_ONE_MUSIC_START_FRAME = 400` applies to `alku`, `u2a` and `pam`
   (`music_start_frame_for`), and 408 to everything else, so a block-1 run drawing from 404
   pre-polls four frames. **Pinned by**
   `a_block_one_run_starts_under_the_initial_play_song_and_matches_second1` and
   `the_block_one_pre_roll_puts_the_player_at_the_traces_7710_position`.
2. **An anchor only where the reference names a first frame.** `boot_anchor` gives 404 for ALKU,
   BEGLOGO and ENDLOGO (the boot's first graphics frame, five sessions of five), 7710 for PAM (its
   wash, byte-identical in three sessions, `reference-indices.md` §6.1, §6.4) and 418 for U2E
   (D33). Parts whose start the reference only brackets have `None` and number their own frames;
   their notes state each mapping (`419 + tick` for MNTSCRL, `12962 + tick` for RAYSCRL,
   `10368 + tick` for MINVBALL, `414 + tick` for DDSTARS) and their tests assert it. A guessed
   anchor would make every index wrong in a way the keyframes cannot catch.
3. **No part inherits `GATE_ALIGNMENT_TICKS`** or D28's alignment. Each cue's timing is its own
   measured value, and where the release's timing differs it is registered (MNTSCRL's 71-tick wait,
   A17; RAYSCRL's seven confirming reads, A18).

## D31 — The mid-chain seam, PLZPART's `.INC` data, and a chain with no loader gap

**§1. Freshness is a property of the install: `PartCtx::music_fresh`.** `Engine::set_music` arms
it; the next `Engine::start` clears it before `init`, so only that part's init context reports
`true`. Parts with entry pre-rolls (MINVBALL 256, RAYSCRL 268, MNTSCRL 11, PAM's open gate,
LNS&ZOOM 490, PLZPART 8, TECHNO 253, 3DSINFLD 527, JPLOGO 483) pre-roll only when fresh and
otherwise take the source's own gate, because a part reached mid-chain finds the player running.
**Pinned by** `the_music_is_fresh_only_for_the_first_part_after_set_music` and the block-3 chain
test (PLZPART→MINVBALL publish 75:60 one poll apart; RAYSCRL publishes 86:62 in its gate and opens
on 88:0).

**§2. PLZPART's tables are its original `.INC` text, filed verbatim** under `assets/PLZPART/`
(`PSINI`, `LSINI4`, `LSINI16`, `SINIT`, `PTAU`, `SPLINE`, `RATA`), hashed by `cargo xtask assets
verify --tree` and parsed at run time by a part-local `db`/`dw` reader. `TILE.INC` and `TILEPAL.INC`
are not copied, because `PLZPART.PRJ` names neither. The table bases come from `PLZPART.MAP:368-391`
(`_psini 027D:0000`, `_lsini4 :4000`, `_lsini16 :8000`, `_plzline :C000`), not the assembler's
placeholders.

**§3. The chain hands over with no loader gap.** The port's parts hand over in one tick, with the
surface and the player continuous; the original's loader takes frames between EXEs. So no single
affine mapping fits every part of a chain, and the chain test asks for each keyframe at the first
chain tick whose trace position is the reference's at `index + 419`. A `LOADER_TICKS` constant is
not modelled: the full run's seam changes take both signs, which no delay at a handover can
produce, and the two music-pinned seams need different amounts (6 and 59–68 frames). The seam
table is A53 and `reference-indices.md` §26.8; **pinned by**
`cargo test --release -p secondreality --test fullrun`.

## D32 — The vector renderer is a module in the binary; the packed formats are `sr-assets` data

**Decided.** The VISU engine is `bins/secondreality/src/parts/visu/` (`mod.rs`, `math.rs`,
`draw.rs`), shared by U2A and U2E, which keep their own scene drivers and gates. The scene and
object readers are `crates/sr-assets/src/visu.rs`, plain format code. No new crate.

**Why.** Demo-specific 3D maths sits above the engine (D3, D16), and the parts share it the way
`parts/poslist.rs` shares the POS/WAT column decoder. `sr-assets` owns the format readers. The
readers parse all 45 shipped objects and both scene streams (`U2A.0AB` 522 frames, `U2E.0AB` 1,802).

**Pinned by** `cargo test -p sr-assets --test visu
every_shipped_object_resolves_its_lists_and_matches_its_own_vertex_count`.

## D33 — U2A's and U2E's tick models, and U2E's anchor

**U2A** advances one animation-stream frame a tick and flips one page a tick
(`VISU/C/U2A.C:105-130`, `:330-344`). It draws from gate 1, `order>10 && row>46` (`:238`), first
true at **6988** in `second1c`. Gate 2, `order>11 && row>54` (`:276`), first holds at 7618 in
`second1b`, but the stream ends first (~7510), so the part normally retires at its END.

**U2E** advances one stream frame every **two** ticks (`currframe += repeat*2`,
`VISU/C/U2E.C:435-444`) and skips its first draw (`:105`, `:386`). It draws from gate `order>18`
(`:365`), first true at 699 in `second4`. Its intro is 100 `dis_waitb`s (`:285-335`): 33 fade-in
steps, 16 waits, one each side of the `fadeset`/CRTC 9 writes, 16 waits, 33 fade-out steps.

**U2E's anchor is the intro's start, 418.** The first visible frame on the `--arg 4` boot is
**467** (the loader's black holds through 466; 467 is the fadeset's white band, columns 68–252, in
two independent dense scans, `reference-indices.md` §18.7). The band is drawn after 49 pre-visible
waits, so `U2E_ANCHOR = 467 − 49 = 418`; 417 would put the band at 466. With it the intro compares
at 0 at nine indices from 460 to 660 (`u2e.md` §3).

**Pinned by** `the_fadeset_lands_on_tick_49_and_paints_the_measured_band` and
`u2e_runs_are_numbered_from_their_derived_intro_anchor`. The moving keyframes' residuals are
A28–A31.

## D34 — The mid-run `MUSIC0@18` restart waits four ticks

**Decided.** At the U2E boundary the shell installs MUSIC0 at order 18 (`MAIN/U2.ASM:880-885`;
U2E's presence in a plan implies the guard `& 8`). The new player waits
`MUSIC_START_FRAME - FIRST_GRAPHICS_FRAME = 4` ticks before advancing, the restart recipes'
404 → 408.

**Why.** `second4` (`SECOND 4`, U2E first) restarts at the boot and derives its first tick at
408.2. `second3e` (`SECOND 3`, restarted after JPLOGO) holds the same session shifted by
17,441–17,442 frames at every order boundary, so the mid-run restart's clock starts four frames
after its boundary, as the entry restart's does. The restart does not zero `_np_zframe`: `second3e`
reads 9,099 at 18000, carried from PLZPART's reset; the port carries it too (D45).

**In the port** the block-3 chain reaches the U2E boundary nine ticks before the reference's
mapping predicts, which is the gapless handover (D31 §3), not the restart; after it, positions agree
with `second3e` within one MUSIC0 row. `u2e.md` §5 records the seam.

**Pinned by** `MusicStart::restart`'s `waiting == 4` and the chain test in
`bins/secondreality/tests/shell.rs`.

## D35 — Restart seams: an exit drain a part opts into, and a sound card that follows a new player

1. **`Part::drains_after_exit()`, default `false`; U2E returns `true`.** The shell keeps a draining
   part live after the exit flag and retires it when it reports `Finished`. `dis_exit()` ends a
   part's loop, not the code after it: U2E runs a sixteen-retrace fade from the live DAC however its
   loop ends (`VISU/C/U2E.C:541-555`). Every other part returns on the tick that sees the flag or
   tears down synchronously (U2A, `VISU/C/U2A.C:239`, `:432-441`). `Engine::start` clears the flag,
   so a drain cannot leak into the next part. **Pinned by**
   `a_part_that_drains_after_exit_keeps_ticking_until_it_finishes`,
   `a_part_that_does_not_drain_retires_on_the_exit_tick` and
   `a_sticky_exit_flag_runs_the_whole_sixteen_step_teardown`.
2. **The sound card follows a restart** (`Device::follow`). The device keeps its player behind a
   `Swappable` cell that the audio callback reads per drain; the window swaps it when `Arc::ptr_eq`
   says the run's player changed. The swap is under the cell's lock, so a callback in flight never
   takes half a buffer from each. The old ring's tail may be dropped, as `restartmus`
   (`MAIN/U2.ASM:348-397`) interrupts the old module. **Pinned by**
   `the_drain_follows_a_restart_to_the_runs_new_player`.

## D36 — GLENZ's renderer lives in the binary, on linear 320-pixel rows

**Decided.** The part is `bins/secondreality/src/parts/glenz/` (`math.rs`, `tables.rs`, `poly.rs`,
`span.rs`, `intro.rs`, `mod.rs`). It keeps the original's 64,000-byte `vram` and `bgpic`
(`GLENZ/MAIN.C:12`, `:18`) as private linear 320-pixel-row buffers and copies the visible rows into
the engine surface each tick. The plane masks and the shipped build's 92-byte CRTC row are memory
layout, not displayed geometry (`reference-indices.md` §7.3, §7.6, §1.7).

**The span walk.** `NEW.ASM:66-172`'s dirty-span walk is an optimisation with identical pixels; the
port repaints every span (A39). The colour path keys `poly.rs`'s `slots` by the colour word's low
byte, shared by the visible and hidden paths (A37, `glenz.md` §11.2).

**Pinned by** `the_held_opening_1499_is_the_reference_index_plane` (0 of 64,000) and
`parts::glenz::span::tests::the_full_repaint_equals_the_source_two_list_walk`.

## D37 — GLENZ's tick model is the source's phase machine

One engine tick per `dis_waitb` or poll:

- **WaitMusic**: poll `musplus`; wait while `< -19`; then `dis_setmframe(0)` and the zoomer
  (`GLENZ/MAIN.C:290-291`). Exit-aware.
- **Zoomer**: 47 ticks of `zoomer2` (`GLENZ/ZOOMER.C:38-69`).
- **ModeSet**: one tick (`memset`, mode 13h, `testasm()`'s state).
- **Bounce**: 177 ticks (`MAIN.C:320-351`; the last band is rows 153–199), after `:308-318`'s DAC
  writes.
- **Wait300**: until `getmframe() >= 300`, then the tmppal build and two `dis_waitb` ticks
  (`:353-398`), the second taking the `bgpic` snapshot.
- **Wait333**: until `getmframe() >= 333`, then `memcpy(pal, backpal)`, `dis_partstart()` and a
  third wait (`:400-404`).
- **Main**: one frame a tick (`:419-640`); it ends on the exit flag, `frame >= 7000`, or
  `-16 < musplus < 0` (`:411`), holding the screen.

**Calibration.** `second2b` reads the reset at 1111 and the main loop's first wait at 1562; the
reference's first object pixels are at 1606 and the source physics' first drawn frame is 44, so
`L_r = 1562`. The port's wait exits at 1117 and its player reaches the two `getmframe` crossings one
tick earlier relative to the reset, so `L_p = 1567` and a keyframe is compared at `K + 5` (A36,
`glenz.md` §11.1). The physics uses 16-bit ints that wrap, as the source's do, and the reference's
2300 object pixels are reproduced at boot 2305 with no widening, so D24 (a) does not apply. Surface
*N* pairs with `pal` after *N−1* (A36). `musplus` (`musplus_from`) is not `_np_zplus`.

**Pinned by** `the_intro_is_47_plus_177_ticks_and_the_measured_waits` and
`the_moving_keyframes_record_the_registered_alignments_and_residuals`.

## D38 — The block-1 `MUSIC1@0` restart and the post-PANICEND cue wait

1. **The block-1 restart.** `MAIN/U2.ASM:813-818` (`whattorun & 2` and `& 1`) runs
   `restartmus(1, 0, 48700)` between BEGLOGO and GLENZ. It is `MUSIC_RESTART_AT`'s
   `("glenz", Music1, 0)` row, installed only when the run's player is not already MUSIC1@0
   (block 2 starts on it and must not reload), with D34's four-tick wait. The trace puts the
   reference's restart at 8,056; the port's GLENZ wait ends 6 frames after the reference's 8,521
   (A38, with D45's carry-over).
2. **The cue wait after PANICEND.** `MAIN/U2.ASM:826-853` polls `musplus` while `>= 0`, then while
   `<= 0`, and continues on the first `> 0`. The shell installs a held `CueWait` part after
   `panicend` retires: it draws nothing, holds the surface, and is exit-aware (an exit flag already
   set skips it, `@@skip5`). A plan that ends at PANICEND enters the wait and then the hold. The
   calibration is PANICEND's retirement at tick 121 = boot 8903 (`panicend.md` §7); `second2` gives
   `musplus` 19 at 8903, the first negative (−32) at 8953 and the exit (1) at 9219.

**Pinned by** `the_block_one_run_restarts_music1_at_glenz`,
`the_block_two_run_does_not_reload_at_glenz`, `the_post_panicend_wait_ends_on_a_positive_musplus`,
`the_cue_wait_holds_the_screen` and `an_exit_on_panicends_last_tick_skips_the_cue_wait`.

## D39 — The vault boots the vendored effects behind the standalone DIS

**Decided.** The vault's references are `GRID/P.EXE`, `HARD/B.EXE` and `TWIST/T.EXE` run as their
own programs behind the vendored `DIS/DIS.EXE`
(`270f5fe2c7c7d17b4720c2019c2832f4bd3863f97e750315974692911f0ead08`), staged by
`tools/vault_capture.sh` and captured with `--boot autoexec --boot-wait graphics --cycles 30000`
(the `[autoexec]` writes `RUN.BAT`). One `KBD_space` goes to GRID and HARD, none to TWIST.

**Why.** The standalone DIS is the non-INDEMO server: service 6 returns 0 until `checkkeys` takes a
key, then echoes the requested code once (`DISINT.ASM:242-282`, `:154`); there is no player, and
the copper service is a bare `ret` (`:294-316`). So the captures are silent, one key passes GRID's
`getch()` (`MAIN.C:153`) and HARD's `dis_muscode(1)` wait (`B.C:138`), and the later cues
(GRID's `dis_muscode(3)`, HARD's `0xff`) never fire.

**The port.** The three parts have no `boot_anchor` and no player (`music_for_part` → `None`), and
number their frames from zero. The start key is modelled at `init` (GRID A40, HARD A46; TWIST takes
none, A43). `K0` is 353 / 356 / 3 for GRID / HARD / TWIST (`reference-indices.md` §24). **Pinned
by** the three `a_part_run_of_{grid,hard,twist}_…` tests in `bins/secondreality/tests/vault.rs`.

## D40 — The vault menu is window-only; `--part` is the headless surface

**Decided.** `--vault` opens a 320×200 menu (`vault_menu.rs`) drawn in `ALKU/FONA.UH`: `SECOND
REALITY` / `VAULT`, the rows GRID, HARD, TWIST with a `>` marker, and the footer
`UP DOWN  ENTER  ESC`. Up and Down wrap, Enter starts the part, ESC closes the window. It is refused
with `--headless`, `--part`, `--block`, `--dump-dir`, `--dump-frames`, `--frames`, `--run-seconds`
and `--audio-dump` ("the vault menu needs a window; name a part with --part") and accepts the
presentation flags. `--part grid|hard|twist` runs the same programs headless.

**ESC.** In a running part the first ESC is the part's own exit and the run holds; a second ESC
returns to the menu. A part that finished by itself (HARD's `default` arm, TWIST's 4,444-frame cap)
is already holding, so one ESC returns. The menu is the port's own surface with no original to
compare.

**Pinned by** `vault_menu.rs`'s tests and `an_escape_on_a_held_vault_part_returns_to_the_menu`.

## D41 — The demo's own run, and how it ends

**Decided.** No argument is `SECOND 1` (`MAIN/U2.ASM:108`'s default `whattorun` 7fh,
`part-sequence.md` §4), so `Cli::parse` makes it `--block 1`; the test card is `--part testcard`.
A block's run is `Ending::Exit` and ends as `SECOND.EXE` does; `--part` and the vault are
`Ending::Hold` and hold for ever (`RunSettings`).

**The exit stage.** When an `Exit` run's plan runs out, `@@partsdone` (`:935-939`) calls
`fademusic` with `cx = 1`, which steps the global-volume byte 0x40 down once a `dis_waitb`
(`:333-346`): **64** ticks (`EXIT_FADE_TICKS`) with the last picture up. `fatalexit` then sets text
mode and waits twenty `copperwaitborder`s (`:967-973`): **20** ticks (`EXIT_BLANK_TICKS`) composed
as index 0 through an all-black palette. After tick 84 `Sequence::ended()` is true and the window
closes; an ended run keeps composing the blank, so a dump past it is answered.

**ESC during the stage closes at once.** The original's stage reads no key (`fademusic` never tests
the flag; `copperwaitborder` runs after `dis_resetint`, `:966`). Closing early is the port's
courtesy (A59).

**The end box.** `printend` (`:506-535`) copies `U2END.BIN` (80×25 CP437 cells) into text memory,
and `fatalexit` calls it on every exit (`:974`). `main` prints rows 0–14 to stdout after the window
closes, for every windowed demo run however it closed (the close button and error paths too, before
the error line), and never for `--part`, `--vault` or `--headless`. `endscreen.rs` maps CP437 to
Unicode; on a terminal (`IsTerminal`) each attribute change is an SGR sequence and every line ends
`ESC[0m`.

**Pinned by** `a_demo_run_fades_then_blanks_then_ends`, `a_run_with_no_arguments_is_the_demo`,
`escape_during_the_exit_stage_closes_at_once`, `the_end_screen_is_the_shipped_box` and
`the_end_screen_is_printed_after_a_windowed_demo_run_only`.

## D42 — Loop mode: `--loop`, `/gravis`, and the seam between passes

**Decided.** The original's `looping` byte (`MAIN/U2.ASM:746`; the setup's `ip[3]`, `START.C:103`)
is `shell::Looping`: `Off` (0), `On` (1), `NoEndScroller` (2), in `RunSettings::looping`. `--loop`
is the setup's "Demo looping: Enabled" (`MENU.C:229-238`); `/gravis` is 2 (`START.C:89-93`, the
first seven characters in any case) and skips the settings screen. `/gravis` is honoured in any
argument position (the original reads only `argv[1]`), and it wins over `--loop` in either order,
because the original's `/gravis` skips `menu()`, the only place looping 1 is chosen. Other `/`
switches are ignored (`U2.ASM:605-606`). Looping is refused with `--part` and `--vault`.

**The plan** (`shell::run_plan`) drops ENDSCRL whenever looping is not 0 (`:914-917`).

**The loop seam** (`:919-933`), when a `Looping::On` plan runs out, holds the last picture:

- 64 ticks of `fademusic` (`EXIT_FADE_TICKS`, `:926-928`);
- `LOOP_RESTART` (MUSIC0 at order 0, `:929-932`) installed under D34's four-tick wait;
- `LOOP_RELOAD_TICKS` = **11** more ticks, then pass 2 from ALKU (`jmp @@partsagain`, `:933`).
  `whattorun` is 7fh on every pass, so pass 2 is block 1's sequence whatever pass 1 began at, and
  only `MUSIC_RESTART_AT`'s two restarts fire in it.

`NoEndScroller` goes to D41's exit stage after CRED (`:923-925`).

**11 is the release's measured interval** (D24): in the loop capture the fade is 9029–9092 and
ALKU's first 320×400 frame is 9104, leaving 9093–9103 (`reference-indices.md` §25.3). If 404 is
ALKU's mode set rather than its first frame, the interval is 9.

**The measured seam.** The loop capture's recipe numbers frames 115 later than the port's
`--block 5`, and the port's CRED ends 81 frames earlier than the original's (CRED's chain drift,
`cred.md`, `endscrl.md` §6.1), so `tests/loop.rs` pairs the port's `C_port + (n − C)` with the
original's `n`. The port's CRED shows its closed iris for two extra ticks, so its fade and `L` are 2
frames late (`L − C` is 77 against 75). The original's ALKU races the restarted player (six sessions
of seven read the new song); the port is in the majority group. Pass 2's music is 5 frames late at
`(0,1)` and ALKU's pass-2 fade-in and hold 3–6 frames late. These residuals are A61.

**Exits.** ESC ends only the running part; the seam drains, and pass 2's `Engine::start` clears the
flag. A looping run is left with the window's close button, and the end box is printed. The title
bar names `pass N` from pass 2.

**Pinned by** `tests/loop.rs`'s `the_loop_seam_is_the_originals` and
`looping_leaves_pass_one_alone`, `cli::tests::gravis_wins_over_loop_in_either_order`, and
`cargo xtask rediff`'s `loop/` entries.

## D43 — An enhanced sound option: the port's own 48 kHz renderer, off by default

**Decided.** `--sound-quality enhanced` selects `Quality::Enhanced` (`PlayerOptions { mix_rate,
stereo, quality }`, carried in `RunSettings::player`). `PlayerOptions::output_rate()` opens the
device and the WAV dump at 20,000 Hz for the authentic stereo path and 48,000 Hz for the enhanced
one. Every install, including a restart, builds its player with the run's options (D45).
`--sound-quality enhanced` is refused for a run with no music (`--part testcard|grid|hard|twist`)
in the words `--audio-dump` uses, and by `--vault`.

**What it is** (`music.md` §13). The tick schedule and every voice's step are the authentic path's.
Each authentic frame becomes 2 or 3 output frames by an exact 12/5 accumulator; output frame `j` of
`n` interpolates each voice at `j/n` of its step and converts the sum with the authentic curve's
small-signal gain, clamped to `i16`.

**Why it is opt-in.** It claims no fidelity, so it is listed as A60 for the audible difference only.
The authentic path is the default and the reference, and it is unchanged byte for byte. No setting
moves the published `(order, row)`, so pictures are identical under either quality.

**Pinned by** `tests/mix.rs`'s `the_authentic_stream_is_pinned_byte_for_byte`,
`enhanced_keeps_the_authentic_tick_schedule`, `enhanced_emits_twelve_frames_for_every_five`,
`a_restart_keeps_the_runs_sound_quality` and
`cli::tests::enhanced_sound_is_refused_for_a_run_with_no_music`; pitch and level are printed by
`cargo test --release -p sr-audio --test enhanced -- --nocapture`.

## D44 — The settings screen: the original's setup, asked every time, nothing kept

**Decided.** A windowed demo run opens on a five-row settings screen (`settings.rs`,
`window::setup_run`), the port's stand-in for `START.EXE`'s `menu()` (`MAIN/START.C:89-95`). The
rows are **Sound** On/Off (`MENU.C:190-209`; Off is `--no-audio`, and the player still runs, as the
original's no-sound option still syncs, `MENU.C:114-123`; D46), **Sound quality**
Authentic/Enhanced (D43), **Demo looping** Disabled/Enabled (D42), **Aspect** Square/4:3 (D48) and
**Border** On/Off. Up and Down clamp (`MENU.C:158-159`); Left and Right change and wrap
(`:161-168`); Enter starts; ESC or the close button leaves and prints the end box (D41).

**Nothing persists.** The command-line flags are the initial values (`Settings::from_cli`) and the
choices become the run's settings (`Settings::apply`); no file is read or written. The run, and the
sound card, are built only after Enter.

**When it shows** (`Cli::shows_setup`): a block's run that is not headless and has not skipped it.
Not for `--part`, `--vault`, `--headless`, `--no-setup` or `/gravis`. It waits in bounded
`wait_event_timeout`s.

**Its look.** The text layer is `menu_text.rs`, shared with the vault menu, plus the seven marks
`( ) + - * = '` at cells 67–73 of the source's order (`ALKU/MAIN.C:40`). The frame is 520×325,
because "Sound quality", "Authentic" and the footer `UP DOWN  LEFT RIGHT  ENTER  ESC` do not fit
320 columns. It changes no compared frame (A59). **Pinned by** `settings.rs`'s tests, including
`the_frame_is_sized_for_the_widest_row_and_the_footer`.

## D45 — A restart keeps what `_zplaysong`'s initialiser does not write

**Decided.** A mid-run restart is the same player taking a new module. The shell's three restarts
(block 1's MUSIC1@0 at GLENZ, `MAIN/U2.ASM:813-818`; MUSIC0@18 at U2E, `:880-885`; the loop's
MUSIC0@0, `:929-932`) go through `install_music`, which builds the new player and calls
`Player::restart_from(&previous, order)`: it takes over the previous player's channels, loop
counter and `_np_zframe`, then calls `play_song(order)`. `play_song`'s clear is
`Channel::initialise`, which writes exactly the song initialiser's stores. An entry install is a
boot: a fresh `Player` and `play_song`.

**The bytes** (`MAIN/STMIK.300`'s segment, rebuilt by `crates/sr-audio/tests/player.rs`).
`restartmus` (`:348-397`) runs `_zstopsong` (`+0x25c5`: `[0x939] = 0`), `_zshutup` (`+0x25e3`: the
initialiser `+0x5614`, a DMA clear, `[0x938] = 0`), `_zinitmodule` (`+0x2400`, through `+0x56bc`)
and `_zplaysong` (`+0x2555`: the header watch `+0x5424`, `+0x56bc`, the order advance), so each
ends in the initialiser. The initialiser (`+0x5625`…`+0x5684`, `+0x56a6`…`+0x56b1`, `+0x5781`)
stores `achannelused`, the periods, `avibcnt`, `aorgvol`, `alasteff1/2`, the cell, `lastins`,
`lastnote`, `alastnfo` and `m_pos`, and never `avol`, `a0volcut`, `aequ`, `aadr`'s high word,
`ac2spd`, `atreon`, `atrigcnt`, `atremor`, `vibtretype`, `a0clearcnt` or `m_poslow`, nor the loop
counter `[0x90b]` or `_np_zframe` (D34 measured the latter carried).

**Why it matters.** Block 1's MUSIC0 hands channel 2 over at volume 63, and MUSIC1's order 0 has a
`C00` on that channel at row 3, which a fresh player's zero volume drops. With the volume kept the
break is taken, order 0 is 4 rows long instead of 64, and the port publishes `1:0` at 8,087. A fresh
player puts everything from GLENZ on about 245 frames late. The U2E and loop restarts do not move.

**Not carried** (A55): the position ring, its pointers and the tick countdown `[0xbe0]`. The object
keeps them too, but the reload fills them (the stopped tick feeds order `0xffff`, `+0x4408`) and the
port has no reload.
Carrying the ring would hand pass 2's ALKU the old song's `43:34` and the minority session's race
(D42).

**Pinned by** `the_song_initialiser_writes_exactly_what_initialise_writes`,
`a_boot_starts_on_the_objects_zeroed_channels`,
`a_carried_ring_publishes_what_the_stopped_previous_player_would`,
`block_ones_music1_restart_takes_order_0s_break` and
`a_block_one_run_is_alku_then_u2a_then_pam_in_the_boots_numbering`.

## D46 — `zspeedadjust`: both words read 0, and the port models neither

**Decided.** Neither `_zspeedadjust` nor `_zspeedadjust2` moves the reference's timing.

1. Both read 0 in the reference's run (`music-clock.md` §5; `cargo xtask music analyse --trace
   reference/music/second5b`).
2. The object's image holds both as zero (`LIDATA` at `0x8cd` and `0x8d1`).
3. The row timer adds only `_zspeedadjust` (`+0x439f`: a tick comes off a row only on the adder's
   carry), so at 0 it never does; `second5`'s speed-6 rows are exactly six ticks apart (`music.md`
   §8).
4. The shell writes `_zspeedadjust2` only for output modes 7 (no sound) and 2 (Gravis)
   (`MAIN/U2.ASM:384-389`, `:725`, `:734`); the reference's card is mode 5, the SB Pro (`:731`).

The port's tick has no adjuster term, which is the reference's arithmetic at 0. Its Sound Off (D44)
keeps the reference's mode-5 clock, not the original's no-sound mode. What the player does with
`_zspeedadjust2` on modes 7 and 2 is unmeasured: no reference runs them.

**Pinned by** `cargo test -p sr-audio --test player the_speed_adjusters_are_zero_in_the_image`.

## D47 — No event stream: `design.md` §8.3's events and §4.8's `audio render` and `event log` are not built

**Decided.** These questions are answered by other tools instead:

- **Frame comparison** (`cargo xtask compare`, `cargo xtask rediff`): every filed frame at a
  registered count, indexed wherever the oracle can read indices (D18, D19). A wrong split, pan,
  start address or palette write shows as pixels.
- **The original's player state as a trace** (`cargo xtask music trace`, D26), against which the
  port's published position is held word for word (`crates/sr-audio/tests/trace.rs`; A12, A54).
- **`--audio-dump`** for the port's PCM. The original's audio cannot be captured, because the
  emulator runs silent (D17).
- **`--positions`** for a headless run's timeline, read by `bins/secondreality/tests/fullrun.rs`.

**Why.** The oracle exposes no events, only polls between frames with the guest running (D14, D19,
D20's read race), so an event log of the original would mean changing the emulator. The cost is
D18's and D19's limit: a register change no filed frame displays goes unseen.

## D48 — The window's default is 2× pixel duplication with square dots; 4:3 is an option

**Decided.** The window presents at scale 2 with square dots by default (`PresentOptions::WINDOW`,
read by `Cli::default`). `present` copies one source pixel's colour into each output pixel and SDL
never scales (`window::blit` copies the texture into a rectangle of its own size, centred; the
texture's scale mode is nearest and its blend mode none). Asked for directly: "scaled by 2x, just
duplicate pixels, no interpolation". The 4:3 box needs 200 rows into 288 lines a step, which is a
resample, so it is `--aspect 4:3` and the settings screen's second Aspect value.

**The picture is sized by the display.** A VGA scans 320×200 (each row twice), 320×400 and 640×400
to the same 640×400 dots, so a source pixel's block is `present::pixel_block`: at scale 2, 2×2 for
320×200, 2×1 for 320×400 and 1×1 for 640×400, all 640×400 on screen. At an odd scale a doubled axis
takes `max(1, scale / 2)`. The 4:3 box is 384×288 a step (a 640-column frame at least two steps).
A frame that is not a demo mode (the settings screen) is `scale`×`scale`. The border is `8 × scale`
output pixels of flat colour (A1).

**The dump is untouched.** `--headless` uses `Aspect::SourcePixels` at 1 with no border,
`scale`×`scale` a source pixel whatever the mode; the window's presentation never reaches a dump.

**Pinned by** `present::tests` and `window::tests` (the default frame through SDL's software
renderer, read back pixel by pixel).

## D49 — The shipped part EXEs are unpacked and read

**Decided.** `sr_assets::exe::unpack` reads the twenty packed part EXEs (copied byte for byte to
`assets/MAIN/DATA/`) through every layer: PKLITE 1.14 small and large, then LZEXE 0.91 (ALKU,
ENDSCRL, PLZPART) or EXEPACK (ten parts); CRED, PAM, PANICEND and the three Pascal parts have one
layer. Each decoder is transliterated from its own stub, with the
stub's tables read out of it. Asked for directly, after the end scroller printed `[15` and the
credits named the wrong people, which needed the release's own text rules, font and captions.

**The images are verified.** Each EXE run alone under DOS unpacks itself, exits on
`ERROR: DIS not loaded` and leaves its image in memory, and the port's image with its relocations
applied matches that memory except for the C runtime's startup writes (`tools/release_images.py`;
`docs/fidelity/release-images.md`).

**What it changes.** Where a part's shipped build differs from the tree in data or code inside the
image, the port reads it from the image (D24 step 3). D24's adopt and register branches stay for
what the image cannot answer: timing, CPU cost, the session phase. Each part's result is in its
A-entry and note. The release's memory during block 5 (`tools/crtc_probe.py 5 9300 2`) holds the
staged ENDSCRL image's font order and file name, so for ENDSCRL the staged EXE is the one
`SECOND.EXE` packs (D12).

**The parts read from their images**:

- **ENDSCRL**: its raster, `fonaorder`, pitch, control-line rule, blank and `%` exit; both builds'
  text-mode `read` strips the CRs. The part ends by itself (`endscrl.md`, A11).
- **CRED**: its 21 screens are `main`'s 21 `screenin` calls (`0x193a`–`0x1b56`), each picture the
  `FC FC` blob at its pushed segment and each caption at its pushed DGROUP offset; the font raster
  (`0x29f0`) and `fonaorder` match the tree (A8). Three code differences are recorded and not
  modelled (`cred.md` §9). The CRED→ENDSCRL chain offset is CPU time (A11). A10 is retired.
- **PLZPART's cube**: the camera path (`buu`, `0x13894`) and sine table (segment `28b1h`, 8,192
  entries) are read from the image, and `calculate`, `count_const` and `rotate` follow the image's
  code: 13-bit angles (`and ax,1fffh`), a 1.15 matrix and a 32-bit rotation in 1/16 units. Both cube
  keyframes are exact; A23 holds the palette-timing residue on four band frames, and A22's exit
  index 4 is read at `0x1181c`.
- **ALKU's credits loop**: `main` (`0x0f0d`) runs while `a <= 320` (`0x1380`), `do_scroll` returns
  past 320 (`0x2315`), `fmaketext` runs only when `aa < 304` (`0x134f`), and each pass ends
  `do_scroll(tptr<5 ? 1 : 0)` (`0x136c`). A closing routine the tree lacks (`0x13ce`) rebuilds the
  last window as the 320×200 page U2A continues, and `exit(0)`s. 5859 is exact and U2A starts at
  5956 (A20, A53; A9 closed). The spin's final `f`, which decides the 64-tick final fade, is taken
  from the frames (no fade) and registered in A20.

**Pinned by** the unpacker's tests in `crates/sr-assets` and the rediff entries of the four parts.
