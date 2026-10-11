# Decisions and findings

Anything that changed the approach, or that a future session would otherwise have to
rediscover. Newest last.

---

## D1 — Fidelity target: faithful now, enhancements opt-in later (2026-10-10)

Accepted: same 320×200 / 320×400 indexed output, same fixed-point maths, same palette
and timing. Target pixel-identical to the original, verified by frame-diffing against
emulator captures. A small number of documented approximations are permitted where an
effect lives inside esoteric generated assembly. The renderer leaves a seam for an
optional enhanced mode later.

Rejected: strictly pixel-perfect (would require a VGA register-level emulation layer
and hand-decompiling multi-thousand-line unrolled assembly); modern remake (different
visual identity).

## D2 — Scope: full demo + hidden part + dev-parts vault (2026-10-10)

Accepted: the 20-part sequence, the hidden DDSTARS part, a modern replacement for the
DOS setup screen, and a vault menu exposing the three unreleased development effects
(GRID, HARD, TWIST).

Rejected: main sequence only (leaves GRID/HARD/TWIST unported).

## D3 — Architecture: software VGA model + per-part modules (2026-10-10)

Accepted: a small engine layer owning an indexed surface, a per-scanline palette, a
view transform replacing the CRTC, a fixed 70.086 Hz tick, and a `Part` trait replacing
the DIS interrupt server. Parts are ordinary Rust modules writing indexed pixels.

Rejected: flat framebuffer with per-part special cases (pushes hardware concepts into
a dozen parts, makes frame-diffs harder); instruction-level emulation (not a port).

## D4 — Pixel aspect: 4:3 default, square-pixel toggle (2026-10-10)

Accepted: default to 4:3 with rectangular pixels, matching a VGA monitor. Square-pixel
16:10 available as a setting.

Rejected: square pixels only (what the existing C++ ports do — geometrically wrong);
4:3 only (no flexibility).

## D5 — The music pattern data is scrambled, and the keystream is recovered (2026-10-10)

**Finding.** `MAIN/MUSIC0.S3M` and `MUSIC1.S3M` (byte-identical copies of what is
embedded in `REALITY.FC`) are standard S3Ms in every respect — header, instrument
tables, correct pattern pointer table tiling on 16-byte boundaries, plain PCM samples —
**except that the packed pattern data is XOR-scrambled.** Stock S3M decoders fail.

**Keystream.** For byte index `i` within a pattern's packed data, from 0 at `ptr+2`,
restarting per pattern:

```
n      = (i >> 1) + 1
key(i) = (((n ^ (n >> 2)) << 3) | ((5*i + 2) & 7)) & 0xFF
```

XOR for `i` in `0 .. ln-2`. Verified 77/77 patterns for MUSIC0, 76/77 for MUSIC1, zero
invalid commands, and the resulting effect histogram reproduces an independently
derived one exactly.

**Two traps found while verifying:** S3M command bytes are ST3 numeric (1–26 = A–Z,
0x30–0x39 = digits), not ASCII; and note bytes are `(octave << 4) | semitone`, valid up
to 0x9B.

**Consequence:** the music is not a research spike; it is roughly ten lines of
descrambling plus a normal ST3-semantics player. This materially reduced Phase 3.

## D6 — Do not copy the C++ ports' shortcuts (2026-10-10)

`SecondRealityW32` and `SecondRealityPlusPlus` stub `dis_setcopper()` to nothing and
upload one palette per frame per whole screen. Consequence: they lost the techno part's
interference seam, a visible quarter-screen discontinuity where the beam catches the
screen mid-redraw. They also widen the palette to 8-bit and spin-wait to 70 Hz.

Adopted instead: preserve the three copper callbacks; keep the palette at 6-bit
precision; fixed timestep decoupled from display refresh; and add **beam compose** —
optional scanline-banded rendering so a part's per-frame work is committed to the
output as the beam would have drawn it.

Also noted: `SecondRealityW32` stubs `dis_muscode()` to `return 0`, so the parts that
wait on it must have been patched. We will implement it properly.

## D7 — Preserve the original's bugs (2026-10-10)

The GMX plasma's palette-leak banding and the no-z-buffer particle overdraw are part of
the visual identity. A "correct" implementation looks wrong. These are recorded as
required behaviour in `docs/fidelity/bugs.md` with reference frames.

## D8 — Open: `_np_zinfo` (2026-10-10)

`dis_muscode()` returns STMIK's `_np_zinfo`, which is not present in the module data
(all `S` commands are standard `SDx`; MUSIC0 has no cells outside channels 0–7;
MUSIC1's only off-channel cells are five near-empty ones on channels 10–14). Exposure:
BEGLOGO waits for `0xf0`, GRID for 1/2/3, HARD for 1 and 0xff.

Resolution ladder: (1) parse `STMIK.300`'s OMF symbol records for `_np_zinfo` /
`_np_zplus` offsets, then read them live from the emulated original via
`dosbox-automation`'s memory API and correlate against order/row; (2) read `st3play`'s
source for the semantics of "plus flags"; (3) disassemble the STMIK segment.

Escalation trigger: if route 1 fails within Phase 3, implement affected parts against a
substitute cue and record the difference. Do not let it block the phase.

## D11 — The reference oracle is verified end-to-end (2026-10-10)

Supersedes the "still unverified" caveat in D10. The emulator's REST API was exercised
successfully. **Working invocation:**

```
LD_LIBRARY_PATH=<repo>/build/release-linux/_deps/{sdl3,sdl3_image,mt32emu,fluidsynth,zlib-ng,iir1}-build
SDL_VIDEODRIVER=dummy
DOSBOX_API_TOKEN=<64 hex chars>
dosbox-automation --noprimaryconf --set webserver_enabled=true
```

Three gotchas, all found the hard way:

1. **The API token must be exactly 64 hex characters.** `DOSBOX_API_TOKEN` with any
   other length is rejected with `WARN: DOSBOX_API_TOKEN set but invalid (need 64 hex
   chars)` and the emulator silently generates its own. Requests then fail with 401.
2. **The log masks the token** (`API token: a4b00e48...`), so it cannot be scraped from
   stdout for a full-length token. There is an alternative — `webserver_token_file` —
   but supplying the token ourselves is better: deterministic, and no file to manage.
   Our spawner generates 64 hex chars, passes them via the environment, and uses the
   same value in the `Authorization: Bearer` header.
3. **Port 8386, bound to 127.0.0.1** by default; remote binding requires
   `webserver_allow_remote=true`.

**Headless operation works.** `SDL_VIDEODRIVER=dummy` runs the emulator with no window,
which is what automated capture needs — no GUI stealing focus during a test run.

**Confirmed capabilities** from `GET /api/v1/dosbox/info`:

```json
"features": { "cpu_control": true, "cpu_registers": true, "debugger": false,
              "freeze": true, "input": true, "memory": true, "port_io": true }
```

All four capabilities the design depends on are present:

- `cpu_control` — pin cycles so captures are reproducible (§8.1)
- `input` — scripted input sequences
- `video/frame` — frame grabs (endpoint present in the OpenAPI document)
- `memory` — **live variable inspection, which is the primary route for resolving
  `_np_zinfo` (§7.4 route 1). This is now a confirmed capability, not an assumption.**

`debugger` is false, so there is no interactive debugger — consistent with the design
never having depended on one.

## D12 — The reference of record is the distributed 1993 release (2026-10-10)

The upstream `mtuomi/SecondReality` tree contains **no runnable demo binary**. It has two
different builds of the shell — `MAIN/U2.EXE` (31,612 bytes) and `U2.EXE` (31,548 bytes) —
plus the unpacked part EXEs and data in `MAIN/DATA/`. Neither shell is the shipped demo.

The original retail release has been supplied separately and now lives at
`reference/original/`:

| File | Size | SHA-256 |
|---|---|---|
| `SECOND.EXE` | 1,451,093 | `a0b637b1c6729aeee8874cb7de701f608fe6ec3028fd72652a962a3a2bd25d8f` |
| `REALITY.FC` | 992,188 | `cffb5839d0660f646c2474ff46452543997c3fc9469fa64401556c2d271aa63b` |

Its `README.1ST` confirms the distribution is exactly these two files unpacked into one
directory, and documents the command line: `SECOND 1`..`5` start at the beginning, title
screen, landscape scroller, vector part and end credits respectively.

**Decision: the distributed release is the reference of record for every frame diff.**
Fidelity means "looks like the demo people actually ran", and that is this binary, not a
developer build from the source tree. The source tree remains the reference for *how*
every effect works, which is what the port transcribes.

**`REALITY.FC` is byte-identical in both places** (`cffb5839…`). So the music and art the
source release ships *are* the shipped data, and Phase 1's asset decoding works on
authentic input rather than something re-encoded. This retires a risk the spec did not
name: there is no chance the media in the source tree is a later or earlier revision.

**Open, to settle in Phase 2:** whether the part EXEs in `MAIN/DATA/` are byte-identical to
those packed inside `SECOND.EXE`. If they differ, the port could match one reference and
not the other. This is cheap to answer once BEGLOGO is diffable against real frames, and
it does not block anything before then. If they *do* differ, the distributed release still
wins and the difference gets recorded in `APPROXIMATIONS.md`.

## D9 — Licence: MIT (2026-10-10)

The new Rust code is MIT licensed, in the same permissive spirit as the original, which
is public domain under the Unlicense. Applies to new source only; the vendored
`upstream/SecondReality` submodule and the media copied into `assets/` stay under
Unlicense/public-domain terms, which impose no conditions on redistribution.

Phase 0 sets `license = "MIT"` in the workspace and crate manifests.

## D10 — The reference oracle is behind a trait, not a hard dependency (2026-10-10)

The `dosbox-automation` binaries on this machine did not run. Diagnosed: the build at
`dosbox-automation/build/release-linux/` succeeds and produces the executable, but it
has no rpath to its own vcpkg-style dependencies, so `libSDL3.so.0`,
`libSDL3_image.so.0`, `libmt32emu.so.2` and `libiir.so.1` are not found. All four *are*
built, under `build/release-linux/_deps/*-build/`. Supplying

```
LD_LIBRARY_PATH=<repo>/build/release-linux/_deps/{sdl3,sdl3_image,mt32emu,fluidsynth,zlib-ng,iir1}-build
```

makes it run — confirmed, reports `dosbox-automation version 0.85.1 (6d100)`.

**Do not use `cmake --install` to make this permanent.** Verified: the fetched
dependencies carry install rules (`sdl3` 12, `sdl3_image` 12, `mt32emu` 39, `iir1` 41,
`fluidsynth` 17) and `CMAKE_INSTALL_PREFIX` is `/usr/local`, so a default install would
place SDL3, mt32emu, iir1, fluidsynth and their headers system-wide. It also would not
reliably fix the binary: `readelf -d` shows the executable has **no `RPATH`/`RUNPATH` at
all**, so installing to `/usr/local/lib` still depends on the system linker being
configured to search there.

**Chosen approach: `xtask` sets `LD_LIBRARY_PATH` on the child process.** The emulator is
spawned programmatically, so its environment can be set at spawn time. No wrapper
script, no system modification, no `patchelf`. The workaround lives in our code where
it is discoverable and reviewable, rather than in a shell script somebody must remember
to use. If a standalone runnable binary is ever wanted, the contained options are
`cmake --install --prefix <project-local-dir>` or rebuilding with
`-DCMAKE_BUILD_RPATH=<_deps build dirs>`; neither touches `/usr/local`.

**Design consequence, independent of whether that fix sticks:** `xtask reference
capture` sits behind a small trait rather than depending on one emulator, so an
unusable oracle degrades to a documented alternative instead of blocking a phase.
Fallback order: locally built `dosbox-automation`; DOSBox-X with scripted commands and
screenshots; DOSBox-staging.

Important consequence for sequencing: **the oracle is only needed for verification.**
Engine work and part implementation do not block on it — only the frame-diff gates in a
phase's exit criteria do. Phase 0 must therefore be planned so that its engine and
synthetic-test-card work can complete even if no oracle is available, with the
reference-capture deliverable explicitly separable.

## D13 — Spec §2.2 was wrong about the music restarts (2026-10-10)

`MAIN/U2.ASM` at the pinned `071a82e` has been transcribed into
`docs/fidelity/part-sequence.md`, with a line citation per row. The upstream tree is now
vendored as the submodule `.gitmodules` pins, so these citations are checkable in-tree.

**What held up.** The part order itself, every binary name, all 20 directory
attributions, `partsmask[8] = 0x80` for `SECOND U`, and `MUSIC0`/`MUSIC1` at orders
0/0/18/42/70 are all correct as §2.2 had them. The survey did well on the sequence.

**What was wrong.** §2.2 presented the two mid-run restarts as part of the normal flow.
They are not: both are **entry-point** restarts, guarded so that they fire *only* when
the blocks leading up to them did not run.

- **MUSIC1 at order 42** fires only for `SECOND 3` (`U2.ASM:856-863`, guarded
  `whattorun & 4` and not `whattorun & 2`). A full run does not restart here — it waits
  for a music cue at `U2.ASM:829-853` and then starts the mountain scroller with MUSIC1
  where it already is.
- **MUSIC0 at order 25** was missing from the table entirely. It exists (`U2.ASM:898-905`)
  and fires only for `SECOND 5` (guarded `whattorun & 16` and not `whattorun & 8`). A full
  run has no restart in the ending either.
- **The startup line was unqualified.** `MUSIC0 starts at order 0` is true only when
  block 1 runs. `U2.ASM:767-775` loads MUSIC1 and starts it at order 0 instead for
  `SECOND 2`–`5`. Block 1 is what selects the module the whole run begins with.

**Consequence for the port, and why it matters.** The demo is music-counted: a part that
begins at the wrong music position drifts against every later part, and the drift is
invisible until a frame diff at the end of the run. `sr-shell` must implement the
restarts as *entry-point* positions — one restart table keyed by which blocks are going
to run, not a single linear timeline — and the reference capture harness must record the
argument it started the original with, because the same part is diffed against different
music positions depending on where the run started.

**Block arguments: the README and the code disagree, and the port follows the code.**
`reference/original/README.1ST:66-79` documents `1`–`5` and their meanings, and all five
agree with what the code selects. But the README never mentions `U`, which the code
accepts (`checkcmdline`, `U2.ASM:613-616, 626-627`) to run the hidden part alone; and the
original's own error message (`U2.ASM:557-559`) says "a number between 2 and 5", which
contradicts both the README and the code. `/`-prefixed arguments are neither rejected nor
interpreted by the shell — they are passed through to `START.EXE`, which understands only
`/gravis` (forces Gravis, `looping = 2`, skips the menu). The port accepts `1`–`5` and
`U`/`u` with those exact masks, treats `1` and no argument as identical, and accepts and
ignores `/`-switches it does not implement, so that a capture command line lifted from
the original keeps working.

**Not resolved:** the musical event the inter-block wait keys on. `dis_muscode`'s `dx`
comes from STMIK's `_np_zplus`/`_np_row` (`DIS/DISINT.ASM:252-271`), and `_np_zplus`'s
semantics remain open — the same unknown D8 already tracks under `_np_zinfo`. Phase 3
resolves it from live memory. The wait's *shape* is certain (wait for `dx` to go
negative, then positive, aborting on any key, skipped if Ctrl is held); only what
musical event `dx` encodes is not.

## D14 — The reference oracle's API shape (2026-10-10)

Everything below was measured against `dosbox-automation 0.85.1 (6d100)` running
`reference/original/SECOND.EXE`, with the emulator as the reference of record (D12).
Response shapes are recorded literally; fixtures are committed in
`xtask/tests/fixtures/`.

**The API document is at `/openapi.json`, not `/api/v1/openapi.json`.** The command in
the task brief 404s. The document is OpenAPI 3.1.0, 76,031 bytes, titled
"dosbox-automation control API". 37 paths, 43 operations. Auth is
`Authorization: Bearer <token>` everywhere.

### Mounting the reference directory is blocked by the default invocation

With `--noprimaryconf --set webserver_enabled=true` and no `-conf` file, the mount
allow-list is empty and `MOUNT C -RO <reference/original>` is refused:

```
The mount policy: outside allowed directories.
Put a dosbox-automation.conf next to the game and start with -conf,
or add the path to mount_allowed_bases in the primary config.
```

`mount_allowed_bases` is read **only from the primary config file** — a `-conf` or a
`--set` override is deliberately ignored (`src/webserver/webserver.cpp:666-675`). Working
recipe, verified: point `XDG_CONFIG_HOME` at a scratch directory containing
`dosbox-automation/dosbox-automation.conf` with

```
[webserver]
webserver_enabled = true
mount_allowed_bases = /path/to/reference/original
```

after which `MOUNT C -RO <dir>` succeeds and reports `Mounted read-only`. The read-only
mount is not a limitation: the demo writes nothing, and the reference directory is
untouched (hashes re-verified below). The source also documents a second route — a
`-conf` file whose directory becomes the anchor root — which was **not tested here**.

### `GET /api/v1/video/frame`

`?format={jpeg|png|raw}` (default jpeg), `?mode={raw|rendered}` (default raw),
`?quality=1..100` (jpeg only, default 98). `Accept` selects the format when `?format` is
absent (`image/png`, `application/octet-stream`; anything else gets jpeg).

| request | Content-Type | size on the probe screen |
|---|---|---|
| no params | `image/jpeg` | 9,367 |
| `?format=png` | `image/png` | 1,393 |
| `?format=raw` | `application/octet-stream` | 256,015 |
| `?format=raw&mode=rendered` | `application/octet-stream` | 1,293,921 (758×569, pitch 2274, `pixel_format` 24 = BGR24) |

**`raw` is a 15-byte header, not 16:**

```
u32 width, u32 height, i32 pitch, u8 pixel_format, u16 palette_count   (little-endian)
```

then `palette_count * 3` palette bytes when paletted, then `height * |pitch|` bytes of
pixels. Getting the header length wrong by one byte shifts every pixel; it cost an hour
during this task, so it is recorded here.

Observed on every graphics mode sampled: `pixel_format = 32` (`BGRX32_ByteArray`),
`pitch = 1280`, `palette_count = 0`, and each pixel is `[blue, green, red, 0]` — the
fourth byte is unused and always zero. **`/video/frame` never returns indices**: the
paletted (`Indexed8`, 8) branch of the schema was not reachable in any mode the demo
uses, so `CapturedFrame::indexed` cannot come from here.

Geometry follows the guest, it is not normalised. For the same 64000-byte source:

- `INT 10h AX=0013h` → frame 320×200, `video_mode.is_double_scanned = true`.
- the demo's own mode 13h/CRTC programming → frame **320×400**,
  `is_double_scanned = false`, and the picture is a genuine 400-line image (353 distinct
  row signatures out of 400; only 24 of 200 adjacent row pairs are equal), so a 320×200
  port frame cannot be compared against it without doubling rows.

Neither is stretched horizontally: the 4:3 correction only appears in `mode=rendered`
(758×569 in this run). `mode=raw` is the emulator framebuffer at native resolution.

### Indexed capture is available, through the memory API (task questions 2 and 6)

`GET /api/v1/memory/{segment}/{offset}/{len}` returns `application/octet-stream` by
default (`Content-Disposition: attachment; filename ="memory.bin"`), no `Accept` needed.
The segment accepts `0xA000` or `40960`; `len` is decimal only (`0x10` → 400 "Invalid
request parameter"). **The read honours the VGA graphics-controller Read Map Select
register** (port `0x3CE` index 4, data `0x3CF`): reads of the same address with the
register set to 0..3 return different bytes (57/54/57/55 distinct values in a live test),
which is what makes the planar case solvable.

Two layouts, both verified against `/video/frame` pixel by pixel:

- **Chain-4 / linear** (sequencer memory mode bit 3 set — e.g. mode 13h as the BIOS sets
  it, and the demo's 320×200 parts): `idx(x,y) = mem[y*320 + x]`. Verified on a
  controlled screen: 0 mismatches over 64,000 pixels. Verified on a demo screen: 0
  index↔RGB conflicts.
- **Planar Mode X** (sequencer memory mode `0x06`, GC mode `0x40`, CRTC offset `0x28` —
  the demo's 320×400 title screen): with the read map set to `p`, plane `p` occupies
  `A000:0 .. +32000` at 80 bytes per row, and
  `idx(x,y) = plane[x % 4][y*80 + x/4]`. Verified on the live title screen: **0 conflicts
  over all 128,000 pixels**, 57 distinct indices, with the frame byte-identical either
  side of the capture.

  A cheap mode test before deciding which read to use: `PUT 0x3C4 = 4`, then
  `GET 0x3C5`; bit 3 is chain-4.

The DAC is readable the same way: write the entry number to `0x3C7`, then read `0x3C9`
three times for R, G, B **as 6-bit values (0..63)**; the read index advances after every
triple. Writing the write index (`0x3C8 = n`) also sets the read index to `n-1`.

The REST shape of that, verbatim from a live capture (recorded in
`xtask/tests/fixtures/dac_port.json`, which also carries the protocol note):

```
PUT /api/v1/io/port   {"port": 967, "value": 1}
  -> {"port": 967, "status": "ok", "value": 1, "width": 1}
GET /api/v1/io/port?port=969
  -> {"port": 969, "value": 11, "width": 1}          # entry 1's red, 6-bit
```

`PUT` takes `port` and `value` as JSON **integers** (the query form's `0x`-prefixed hex is
not accepted in a body: `{"port":"0x3C7",...}` → `{"error":"Malformed request body"}`), and
`width` defaults to 1. `GET` takes `?port=` plus an optional `width=1|2` and answers with
`port`, `width` and a single `value`; there is no bulk read, so a 256-entry DAC costs 768
requests, and each read advances the DAC's internal read index — which is why the entry
has to be selected with a `0x3C7` write before each triple, not just once.

Caveat that applies to every register poke: a running demo rewrites the VGA registers
many times per frame, so a plane selection or a DAC read is only meaningful while the
guest is idle or the screen is static. That is also why a Mode X capture needs the four
plane reads *and* the frame read to happen inside one still moment.

### The widening rule the emulator actually uses (differs from Task 2's choice)

DOSBox widens six bits to eight with **round-to-nearest**,
`rgb6_to_8(c) = (c * 259 + 33) >> 6` — identical to `round(255c/63)`
(`src/utils/rgb.h:31-36`, applied in `src/hardware/video/vga_dac.cpp:158-160`).

Task 2 chose `dac_expand(v) = (v << 2) | (v >> 4)`. The two agree on 54 of the 64 values
and differ on ten: **11–15** (Task 2 is one too low) and **48–52** (Task 2 is one too
high).

```
v=11: (11<<2)|(11>>4) = 44|0 = 44        (11*259+33)>>6 = 2882>>6 = 45
v=48: (48<<2)|(48>>4) = 192|3 = 195      (48*259+33)>>6 = 12465>>6 = 194
v=42: both give 170                      (the standard blue, so it proves nothing)
```

Measured live, not inferred: a probe program set DAC entries to 11/48/63, 12/49/0,
13/50/21, 14/51/42, 15/52/63, 16/46/47, 0/0/0 and 63/63/63 and painted one flat band per
entry. Every pixel of `/video/frame` matched the round-to-nearest rule; the Task 2 rule
mismatched 40,000 of 64,000 pixels (62.5%). The fixture triple `frame.bin` / `fb.bin` /
`dac.bin` re-checks this offline: 0 mismatches for `(c*259+33)>>6`, 40,000 for
`(c<<2)|(c>>4)`.

**Ruled: the port follows the emulator.** `dac_expand` in `crates/sr-engine/src/palette.rs`
is now `((v & 0x3F) as u16 * 259 + 33) >> 6`, with `15 -> 61`, `48 -> 194` and `52 -> 210`
pinned as test cases because bit replication gives 60, 195 and 211 for those. The
emulator's formula is also the better model of the hardware: it is `round(v * 255 / 63)`,
the even mapping of 64 levels onto 256, whereas replicating the top bits is a wiring
shortcut that diverges from it on exactly those ten values.

**The rule cannot affect a fidelity measurement, which is why adopting it is free.**
Task 13 compares indices whenever both sides expose them, and indices are what the capture
path actually records (`fb.bin`, `planes`), so widening only ever enters the RGB path —
the PNG/JPEG output, the diff images, and human inspection. That is also the main reason
to keep the indexed path first-class.

### `GET`/`PUT /api/v1/cpu/cycles` (task question 4)

The unit is **cycles per millisecond**, verbatim from the emulator's own setting help:
"Emulate a fixed number of cycles per millisecond (roughly equivalent to MIPS). Valid
range is from 50 to 2000000", with a ballpark table (386DX-33 = 6000, 486DX/2-66 =
25000, Pentium 90 = 50000). The demo asks for a 33 MHz 486, so 25000 was used.

`PUT {"cycles": N}` → `{"status":"ok","cycles":N}` and sets **both** real and protected
mode. Out of range → 400 `{"error": "Field 'cycles' must be 50..2000000"}`; missing field
→ 400 `{"error":"Field 'cycles' must be an integer"}`; 409 while input recording is
active, or in legacy cycles mode.

**The trap:** starting the emulator with `--set cycles=N` puts it in *legacy* mode
(`"mode":"legacy"`, `real_mode`/`protected_mode` null) and the runtime PUT is then
refused. Use `--set cpu_cycles=N` (modern, verified: `"mode":"modern"`, `real_mode`
25000, PUT still works), or set cycles only through the API.

### Input, scripting, and stepping frames (task question 5)

- `POST /api/v1/input/type {"text": "...", "cps": 250}` — paced keystrokes. Only ASCII
  that maps to a key is sent: `\n` is Enter, and **`\r` is silently dropped**
  (`src/lua/text_input.cpp`). It returns `{"status":"ok","chars":N}` *before* the keys
  have drained: the caller must wait about `N/cps` seconds.
- `POST /api/v1/input/sequence {"events": [...]}` — keys with `t` (ms, absolute on the
  sequence timeline) or `delay_ms` (relative), and mouse/timed forms. Key names are the
  C++ `KBD_KEYS` enumerator names: `KBD_left`, `KBD_right`, `KBD_enter`, `KBD_space`,
  `KBD_esc` — `KBD_escape` returns 400 `"Unknown key: KBD_escape"`.
- One injection at a time: anything sent while a sequence is still draining fails with
  `{"error":"Replay already in progress"}`. `input/type` and `input/sequence` share that
  one queue, so a harness must pace its calls.
- `POST /api/v1/script/load?name=X` with the Lua source as the raw request body and
  **`Content-Type: text/plain` or `application/lua`** — anything else, `application/json`
  included, is refused with 415 `"unsupported Content-Type: …"`, and a missing header with
  415 `"missing Content-Type header"` → `{"status":"loaded","name":"X"}`; then
  `/api/v1/script/start`, `/api/v1/script/status` (`state` idle|loaded|running|yielded|
  completed|error, plus `frame` and the script's `dosbox.output` table), `/script/stop`.
  `/script/load` is rate-limited to one call per 2,000 ms (429 + `Retry-After`).
- Lua exposes `dosbox.wait_frames(n)` and `dosbox.frame()`. Verified: `wait_frames(60)`
  advanced the counter exactly 60 and took 0.92 s of wall clock, so the emulator runs at
  the DOS rate in real time. **There is no pause or step-the-emulator endpoint** and no
  debugger, so "frame N" means "after N rendered frames of wall-clock-paced emulation",
  which is what `wait_frames` can pin exactly.

Recommendation: scripted input is simpler as keyboard events (`input/type` +
`input/sequence`); use Lua only when frame-exact timing or screen-text synchronisation is
needed.

### What happens between boot and the first demo frame (task question 7)

**`SECOND 2` does not skip the setup screen**, and neither does a bare `SECOND`. The
block argument chooses the demo's entry point; the soundcard menu runs first either way.

- Launching `SECOND 2` draws the setup screen (text mode 03h, 720×400 frame) fully
  **0.31 s** after the command.
- Its three fields default to `Soundcard: Gravis Ultrasound, 512K of memory (stereo)`,
  `Sound quality: High`, `Demo looping: Disabled`. Pressing Enter with the Gravis default
  fails: `Failed to initialize the selected soundcard. Try selecting another soundcard or
  no sound.` and the program returns to the DOS prompt. This emulator provides an SB16
  (`BLASTER=A220 I7 D1 H5 T6`, from the startup log) and no GUS (no ROM files), so the
  menu has to be answered.
- The soundcard entries, in order: `No sound`, `SoundBlaster (mono)`,
  `SoundBlaster Pro (stereo)`, `Gravis Ultrasound, 512K of memory (stereo)`. One
  `KBD_left` from the default lands on `SoundBlaster Pro (stereo)`.
- **Recipe, verified end to end:** type `SECOND 2\n`, wait for the setup screen, send
  `KBD_left`, send `KBD_enter`. First graphics frame 0.26 s after Enter; about 1.0 s from
  launch to the first demo frame. The demo then runs from the read-only mount without
  complaint.

Not tested: the `No sound` option (the demo is music-counted, so it is not obviously
safe), `SECOND /gravis` (D13 says `/gravis` skips the menu, but it forces the GUS this
emulator cannot provide), and driving the menu with the DOSBox shell's own `AUTOTYPE`
command from an `[autoexec]` section, which exists with `-w`/`-p` and key names such as
`left enter` and would remove the input round-trips entirely.

### What a frame index counts from

**Frame 0 is the emulator's first rendered frame after it starts** — a boot-relative
counter running at the DOS rate, exposed as `dosbox.frame()` in Lua and as the `frame`
field of `GET /api/v1/script/status` (a resident script is the only way to read it; there
is no frame-number field on `/video/frame` or `/status`).

Measured on one API-driven run at 25000 cycles/ms, typing at 250 cps:

```
counter just before the SECOND 2 command       1657
setup screen visible                            1677   (+20 frames, 0.29 s)
first demo (graphics) frame                     1718   (+41 frames after the setup,
                                                        0.26 s after Enter)
```

So the counter advances at about 70 Hz from process start and **frames 10, 60 and 120 do
not reach the title screen**: from boot they are text-mode DOS-shell frames several
hundred milliseconds in, before `SECOND.EXE` has even been typed. The title screen sits
around frame 1700 on this path, and how many exactly depends on how long the harness
spends mounting and typing — the number is a property of the run, not of the demo.

Consequence for the capture tool: define the origin explicitly, count
`--skip-boot-frames` and every captured index from the emulator's start, and record both
in the manifest, so a frame index means the same thing in two runs. Nothing in the
committed fixtures contradicts this: they are single frames identified by the screen they
show, and `frame_info.json` records geometry, not an index.

### Fixtures

`xtask/tests/fixtures/`, recorded from the live emulator, never hand-written:

| file | bytes | what it is |
|---|---|---|
| `info.json` | 326 | `GET /api/v1/dosbox/info` |
| `frame_info.json` | 411 | `GET /video/frame/info` on the probe screen |
| `frame.bin` | 256,015 | `GET /video/frame?format=raw` on the probe screen |
| `fb.bin` | 64,000 | `GET /memory/0xA000/0/64000` for that same screen |
| `dac.bin` | 768 | 256 RGB triples, 6-bit, read through `0x3C7`/`0x3C9` |
| `dac_port.json` | 2,638 | the literal `PUT`/`GET /io/port` request and response pairs behind that read |
| `modex_title_frame.bin` | 512,015 | the demo's 320×400 Mode X title screen |
| `modex_title_planes.bin` | 128,000 | its four 32,000-byte planes, in plane order |
| `modex_title_frame_info.json` | 413 | that screen's `video/frame/info` |

The first five are a self-checking set: for every pixel,
`frame.bin` must read `(blue, green, red, 0)` where each channel is
`(dac.bin[3i + ch] * 259 + 33) >> 6` and `i` is `fb.bin[y*320 + x]`. Verified 0
mismatches. `modex_title_frame.bin` and `modex_title_planes.bin` are the same check for
the planar case (`idx(x,y) = planes[x%4][y*80 + x/4]`, 0 conflicts) and are the only
recorded bytes of the screen Phase 0 actually captures.

### What this contradicts in Tasks 12 and 13

1. **Mounting is blocked by the brief's spawn invocation.** `--noprimaryconf --set
   webserver_enabled=true` has an empty mount allow-list, and `--set` cannot fill it.
   Task 12's `spawn` must also provide a primary config (`XDG_CONFIG_HOME` scratch dir)
   naming the reference directory in `mount_allowed_bases`.
2. **There is no BMP.** `decode_frame(..., "image/bmp", ...)` has nothing to match:
   the content types are `image/jpeg` (default), `image/png` and
   `application/octet-stream`. The default is lossy JPEG at quality 98, so a capture must
   ask for `raw` (or `png`) explicitly.
3. **The raw header is 15 bytes and the pixels are BGRX.** `palette_count` is 0 for every
   mode sampled, so `/video/frame` never carries indices: `CapturedFrame::indexed` has to
   be filled from `/memory/...` (with the read-map-select dance in Mode X) and
   `CapturedFrame::palette` from the port API, not from the frame endpoint.
4. **`geometry: [320, 200]` is not universal, and a frame index needs an origin.** The
   title screen is 320×400 and is not a doubled 200-line image, so geometry has to be
   recorded per capture. And because the emulator's frame counter starts at process start
   (see "What a frame index counts from"), frames 10/60/120 counted from boot are DOS-shell
   frames: the capture has to name a boot-relative origin and skip to the demo.
5. **`arguments: ["2"]` alone does not reach the demo.** The soundcard menu must be
   answered with `KBD_left`, `KBD_enter` (the default choice fails on this emulator), so
   the manifest's arguments plus the key script are both part of the capture recipe.
6. **`set_cycles` must use the PUT**, and the emulator must not be started with a
   `cycles=` command-line value, which switches it to legacy mode where the PUT fails
   with 409. `cpu_cycles=` is the modern key.

### Left open

- Whether the `-conf`-anchor mount route works (documented, untested).
- Whether `No sound` lets the reference run (would cut the soundcard dependency).
- Whether `AUTOTYPE` can drive the setup menu from `[autoexec]`.
- Capture determinism at fixed frame indices — Task 12's own step 5, not answered here.
- The palette payload of a paletted raw frame was never observed.
- Other endpoints exist and were not exercised: `/api/v1/mixer`, `/capture/*`,
  `/memory/search`, `/memory/freeze`, `/dos/internals`, `/drive/swap`, `/mount/lock`.
  Nothing in Phase 0 needs them.

## D15 — Capture is deterministic, and what that costs (2026-10-10)

Task 12's step-5 check is answered: **two captures of the same boot-relative frame indices
are byte-identical**, frames and manifest both. Three things had to be understood first,
and each is a rule a later phase has to keep.

**The demo is deterministic in demo-relative time, not in boot time.** Two runs with the
same commands produced the same screens at indices 43–45 frames apart — the whole demo was
shifted by exactly the jitter of the boot (how long the host took to answer the setup
screen), with identical pixels per screen. So a fixed boot index lands at a demo position
only as precisely as the boot repeats, and a *still* section is what makes the comparison
safe: `xtask reference capture` therefore records the origin (`"frame_origin": "boot"`) and
the skipped boot (`skip_boot_frames`) in the manifest, and Phase 2+ should capture where the
screen holds still unless it re-anchors on a guest-visible event.

**Captures are one geometry each.** The title screen is 320×400 Mode X and the demo's next
part is 320×200 chain-4, and `video/frame` follows the guest rather than normalising. One
manifest carries one `width`/`height` (the controller's ruling for Task 13's reading of it),
so the capture refuses a frame list that changes geometry mid-way rather than writing a
manifest that misdescribes some of its frames.

**The DAC cannot be recorded whole.** A running demo rewrites palette entries between
frames — including entries the current screen does not use — so a snapshot of all 256
registers does not repeat between runs: the two determinism runs differed in every *unused*
entry while every frame byte was identical. The capture records the entries the frame's
indices actually use (which the coherence check has verified widen back to the RGB frame),
and that is deterministic.

**Small print, all measured.** The frame counter is read through a command the emulator
services once per rendered frame, so a capture lands within a frame or two of the requested
index — the capture refuses (rather than mislabelling) only when the emulator has run more
than three frames past it. The emulator stalls for ~20 frames when the demo loads, which is
normal and does not shift the demo-relative mapping. And the indexed read costs ~770
requests (four planes, or one for chain-4, plus 768 for the DAC), so it takes roughly a
third of a second: it is only meaningful where the screen holds, and the capture checks the
indices against the RGB frame before writing them.

## D16 — Phase 0's interface decisions, and the open question it did not close (2026-10-10)

Ten decisions the plan made where the spec was silent, each because the alternative was worse.
Recorded here so a later phase inherits the reasoning rather than re-deriving it.

1. **Presentation lives in `bins/secondreality`, not `sr-engine`.** Spec §4.2 puts "Presentation"
   under `sr-engine`; putting the SDL2 dependency there would make the engine untestable without
   SDL. No `sr-present` crate was introduced — the engine keeps the pure raster maths, the binary
   owns the window. The engine's `[dependencies]` is still empty, and a test asserts nothing has
   crept in.
2. **`View` gained a `width` field.** §4.2's sketch lists height but not width, and 320- and
   640-wide displays both exist (§2.3).
3. **`View::split` is `Option<Box<Split>>`, and neither `View` nor `Split` is `Copy`.** A `Split`
   containing a `View` containing a `Split` is an infinitely-sized type, hence the `Box`. That
   `Box` is also why they are `Clone` only: `Box<T>` is never `Copy`, and the bound is recursively
   unsatisfiable. The plan originally declared `Copy` on both and would not have compiled.
4. **`Part` gained `name()`**, and `init` returns `Result<(), PartError>` rather than `Result<()>`,
   so the runner can name the part that failed to start.
5. **Music is not in `PartCtx` in Phase 0.** It arrives in Phase 3 (spec §7). Adding fields to
   `PartCtx` is additive and breaks no part.
6. **The overscan border is a fixed pad in the presenter**, not a modelled overscan region — 8
   source pixels per side, scaled with the picture. It is the first entry in
   `docs/fidelity/APPROXIMATIONS.md`, and Phase 5 must re-measure it when PANIC lands.
7. **`xtask` is invoked as `cargo xtask`** through a `.cargo/config.toml` alias.
8. **The composed frame is the displayed area only** — no full-frame overscan synthesis.
   `IndexedFrame` is exactly `view.width × view.height`, and `Engine::tick` resizes it from the
   view every tick, because a part that cannot change the displayed line count cannot express
   double-scan at all.
9. **The reference of record is the 1993 retail release**, vendored at `reference/original/`, not a
   build from the source tree — see D12. The source tree contains no runnable demo binary, only two
   developer builds of the shell.
10. **`rstest` is a dev-dependency.** The zero-*runtime*-dependency constraint is what keeps
    `sr-engine` usable without SDL2; a test-only crate does not violate it. Used for `#[case]`
    tables and `#[fixture]`s where they read better than a hand-rolled loop.

Two further decisions were made mid-phase and are recorded where they belong rather than here: the
split model (**post-split scanlines count from the split**, because line compare resets the CRTC's
counters and counting from the frame top would make the register a no-op) and the DAC widening rule
(**`(c*259+33)>>6`**, adopted from the emulator because it is also the ideal even mapping). Both were
corrections of Phase 0's own plan, not refinements of the spec.

### The split's two conventions, and the stride's default (settled after the phase review)

Phase 0's closing fidelity review found the split model and the DAC rule right, and two of the split's
conventions unwritten. Both are now on `Split`'s doc comment with their source lines, and repeated
here because they are exactly what a later phase gets wrong.

**The line-compare register is not the scanline.** `Split::scanline` is the first displayed scanline
the lower view draws, counted within the enclosing view. The hardware's register names the last line
*above* the split: the emulator applies it at `(line_compare + 1) / lines_scaled - vblank_skip`
(`vga_draw.cpp:1586-1593`) and resets the beam at the start of that scanline (`vga_draw.cpp:1276`). A
value copied from the assembly — `PLZPART/COPPER.ASM:126` alternates `60`/`61` — is therefore **one
less** than the number to write here, and taking it literally puts the split one line high.

Two caveats are recorded but **not** settled: `lines_scaled` divides the sum for a mode forced to
single scan (`vga_draw.cpp:3594`), and `vblank_skip` — a vertical-blanking trick the emulator logs a
warning about (`vga_draw.cpp:2245-2293`) — subtracts from it. Neither has been met yet, so the first
part that splits must confirm the mapping against a capture rather than trust the `+ 1`.

**The lower view's origin is forced to 0.** `VGA_ProcessSplit` (`vga_draw.cpp:1139-1158`) sets the
address counter to 0, clears panning, and resets the sub-row phase: the lower region of a hardware
split reads from the top of video memory, which is the whole trick. The port's `View` is a superset —
it lets the lower view name any origin — while `View::new` already produces the hardware's answer, so
a faithful port leaves the lower view's origin and pan alone. A faithful-*looking* translation of
`set_plzstart` that gave the lower view the scrolled origin instead would show a whole region wrong in
a way that looks like an arithmetic bug. The test card sets a walking origin deliberately, to prove the
switch lands on named scanlines; that is a picture the hardware cannot produce, and the reason the
field was not simply hard-wired to 0.

A lower view carries only its origin, stride, pan, split and border: `width` and `height` on a lower
view are never read, because the frame's geometry is the top-level view's. `border` *is* read from the
effective view, which is what lets a part change the overscan colour partway down the frame.

**The line stride's default was a trap, and is defused.** `View::new` defaulted `y_line_offset` to the
view's own width. It is now `Option<u16>`, and `None` — the default — resolves to the **surface's** row
stride inside `View::source_index`, which is handed the surface. Resolution is per effective view, so a
split's lower view resolves its own rather than inheriting the enclosing view's. The trap had already
sprung in `bins/secondreality/src/main.rs`, which worked only because `TestCard::init` overwrote the
view; no part had been ported, so nothing downstream was wrong. `TestCard` no longer sets a stride at
all, which makes the card a live test of the default.

### Open item — `View`'s origin is `u16` and the surface is not

**Unresolved. Phase 2 must settle it when ENDSCRL lands.**

`start_addr` and `x_offset` are `u16` (spec §4.2), so an origin sums to at most 131,070. The
mandated surface is 640 × 400 = **256,000** pixels.

Every Phase 0 use stays well inside that — the test card's largest origin is 64,000 — so nothing in
Phase 0 is wrong. But ENDSCRL's **401-line circular buffer** at 640 pixels wide needs display
origins up to ~256,000:

- On real hardware this is fine, because the CRTC address counter counts *bytes*, and a 16-colour
  planar scanline is 80 bytes. 401 × 80 = 32,080, comfortably inside 16 bits.
- In the port's chunky-pixel model the same buffer is 256,640 *pixels*, which needs 18 bits.

Phase 0 deliberately did **not** widen the fields: the spec declares `u16`, and widening mid-phase
would be a unilateral deviation from an approved document. Phase 2 either widens the fields or
models the CRTC's byte countering, and either way amends §4.2 and records the choice here.

Independently corroborated: Task 8's implementer hit the same ceiling and had to clamp the test
card's split walk to 3 rows, with `SPLIT_WALK_ROWS` derived from `u16::MAX` so it follows if the
fields widen.

### Deferred minor findings

Phase 0's task reviews left a list of Minor findings that were deliberately not fixed — they do not
block anything and several are documentation polish. The full list with the reasoning is in the
phase ledger (`.superpowers/sdd/00-foundation/progress.md`), which is git-ignored scratch. The two
that a later phase is most likely to trip over:

- **`compare`'s guard has a demonstrated bypass.** `--out <nonexistent>/../<reference>/x` is not
  refused: `canonicalize` fails for every ancestor up to the first that exists, so the `..` survives
  in the appended tail and is compared literally, and the write lands inside the reference. A
  reviewer compiled the guard standalone and reproduced the write. The doc comment claiming a `..`
  cannot slip past is therefore not unconditionally true. It needs a contrived path, not the
  accidental one the guard was added for, which is why it was deferred — but the fix is one line:
  lexically collapse `.`/`..` in the tail after canonicalising the deepest existing ancestor.
- **`phase_beam_compose_staircases_the_palette_down_the_frame` cannot tell a monotonic ramp from a
  sawtooth.** It passes against both. The property is currently guaranteed by the constant
  (`8 + band * 7`) rather than by a test; a strict-increase walk over all eight band boundaries
  would pin it.
