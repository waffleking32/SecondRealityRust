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
(`src/utils/rgb.h:22-28`, applied in `src/hardware/video/vga_dac.cpp:158-160`).

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

**Superseded by D17, and left standing as the record of what was measured then.** The 1657/1677/1718
landmarks and "the title screen sits around frame 1700" are the **typed** recipe's, where every step
of the boot waits on the host. Under the recipe every reference in the tree was captured with
(`--boot autoexec` plus the silent emulator, D17), the setup screen is at **16** and the demo's
first graphics frame at **404**. Nothing above is wrong about the run it measured; a reader landing
here first should take the landmarks from D17 and this section's *rule* — indices count from the
emulator's own start, so define the origin and record it — and not its numbers. (B6's close-out, the
deferred minor from B1's review.)

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

## D17 — A boot that repeats, and what still does not (2026-10-11)

B1 asked whether the boot can be made to repeat. It can, and it took two changes rather
than one: the guest has to run its own boot recipe (route 1), and the emulator has to be
run silent. With both, the same boot index is the same screen in every run, and captures
of a screen that moves repeat byte for byte *within an alignment group* — a session's
captures are internally consistent, and a re-run lands on the same pictures about half the
time, which the last section here measures. Measured on
`dosbox-automation 0.85.1 (6d100)`, `SECOND 2`, 30000 cycles/ms, on a 320×200 part screen
whose picture changes ~3–7% of its pixels per frame (the vector object turning over its
checkered floor) — a screen where a one-frame difference is visible.

**What repeats is the boot, and it was measured on the emulator's own clock, not through
the API.** A Lua script that yields once per frame (`dosbox.wait_frames(1)`) and reports
`dosbox.output` records the frame the setup menu's text appears on and the first graphics
frame. Five runs of `--boot autoexec` — three before and two after the silent change —
gave **menu frame 16 and first graphics frame 404 every time**. The typed recipe gave
45/46 and 80/81 on an idle host: a frame of jitter at its best, and 43–45 frames under
load (D15). Route 1 shifts the whole demo timeline later by a fixed amount: measured with
one audio device on both recipes, the typed recipe's two video-mode changes at frames 83
and 809 move to 407 and 1133, a pure +324, with the title screen lasting 726 frames in
both. Under the silent recipe the demo runs on the emulated clock instead, so its own
title-to-part change lands at frame 1160 — the parts are the same, the clock they run on
is the emulated one.

**Route 1 is `[autoexec]` + `AUTOTYPE`** (`BootRecipe::Autoexec`, `--boot autoexec`). The
primary config gains an `[autoexec]` section — `MOUNT C -RO <abs cwd>`, `C:`,
`AUTOTYPE -w 5 left enter`, `SECOND.EXE 2` — and the emulator's own shell runs it at boot.
`AUTOTYPE` comes before the program line because the program does not return: it schedules
its press/release pairs through `MAPPER_AutoType` → `PIC_AddEvent` (`gui/mapper.cpp:1220-1244`)
on the emulated clock, then exits. Nothing in the boot waits on the host, so nothing in it
jitters with the host.

**The two key vocabularies are not one spelling with a prefix**, and getting that wrong
answered the setup screen with nothing and wrote a capture of the menu at exit 0. `--keys`
takes `input/sequence`'s `KBD_KEYS` names; `AUTOTYPE` wants the mapper's button names; 27
of the 105 keys differ beyond the prefix — `KBD_backspace` is `bspace`, `KBD_leftshift` is
`lshift`, `KBD_leftctrl` is `lctrl`, `KBD_leftbracket` is `lbracket`, and every keypad key
(`KBD_kp8` is `kp_8`, `KBD_kpenter` is `kp_enter`). `AUTOTYPE` does not report an unknown
name: `type_button` only reaches `LOG_ERR` (`gui/mapper.cpp:209-222`) and presses nothing,
and that log is the scratch file the capture deletes. Both halves were measured, not read:
a run with `AUTOTYPE -w 2 kp8 backspace leftshift left` logs six "Couldn't find a button"
lines and a run with `AUTOTYPE -w 2 kp_8 bspace lshift left` logs none, while the API
answers `{"key": "KBD_kp8"}` with 200 and `{"key": "KBD_kp_8"}` with 400 "Unknown key".
`KEY_NAMES` in `xtask/src/reference.rs` is therefore the real pairing, taken from the
mapper's button layout and `KeyBlock` tables (`gui/mapper.cpp`) and the API's list
(`webserver/input.cpp`, `key_name_map`); every name is validated against it before anything
starts (exit 2, naming the key, in **both** recipes), and a capture whose keys do not clear
the setup screen — the menu's default soundcard fails and drops the demo back to the DOS
prompt — is refused by waiting for the demo's first **graphics** frame, because key names
alone cannot judge a recipe (`KBD_kp8` is a real key and a real button, and it answers
nothing there).

**Silent is part of the recipe, and it is not the boot's doing.** With a real audio device
the mixer follows the host's audio clock and the demo's music and animation drift against
the frame counter *continuously*: two route-1 captures at indices 2125/2250/2400 differed
by 14.28/35.26/24.56% of pixels (max channel delta 142/158/162). SDL's dummy audio driver
alone removed most of it but still let two runs land a frame or two apart over twenty
seconds (at index 2000, byte-identical only after shifting one run's counters by −2). The
emulator's own `nosound` mode (`--set mixer_nosound=true`, `mixer.cpp:2943-2946`: sound
still emulated, no host output) leaves the emulated clock in charge, and then the emulator
is deterministic per frame. `spawn` now sets both (`SDL_AUDIODRIVER=dummy` and the setting).

**The captures.** With `--boot autoexec --cycles 30000` and the silent emulator, three
captures at indices 2125/2250/2400 were byte-identical in all three pairwise comparisons
(0.0000%, `verdict: identical`, exit 0), as were three at 1800/2000 after the `nosound`
fix. The frames are not a still screen: within one run those three indices differ from
each other by 20–40% of pixels, and one frame of the animation is worth 5.5–12.7% of bytes
at those indices (2.6–4.2% at frame 2125, where the object turns more slowly), so the
identical pairs could have differed. For contrast, the typed recipe at the
same indices with the same silent emulator differed by 12.66–15.79% — the boot anchor's
cost, now that it is the only difference left.

**Route 2 was skipped, and the ruling allowed skipping it.** One `input/sequence` carrying
the typed command line plus the setup answer needs a character→key map (the sequence takes
only `KBD_*` key events — `webserver/input.cpp:449-465`) and spawn plumbing for the keys,
well over the ~30 lines that made it worth measuring; and it cannot remove the anchor
anyway, because the sequence is re-based to the frame at call time
(`webserver/input.cpp:288-320`). The typed recipe's 12.66–15.79% above is that anchor's
cost, measured directly.

**The capture now takes the frame the counter names, or none.** D15's `FRAME_READ_SLOP`
let a grab land up to three frames after the index and be written under it anyway — the
tool itself was measured doing it (four captures of index 3000 landed on 3000, 3000, 3001
and 3003, differing by 14–22% on that screen). The grab is now identified instead: the
counter is read on both sides of it, and the emulator publishes a frame's pixels and
counts it immediately afterwards, in that order and on one thread (`render.cpp:446`, then
`sdl_gui.cpp:1205`), so a reading that did not move across the grab names the frame that
was published — with one narrow exception, a video-mode stop or reset calls
`GFX_EndUpdate` without publishing (`GFX_Stop`, `sdl_gui.cpp:396-402`; `halt_render`,
`render.cpp:349-353`), so a single counter value per stop can still see the old picture.
Where such a stop changes geometry the capture's `GeometryChanged` refusal catches it, and
the same counter reading also identifies the mode-change frames after it. A grab that straddles a frame boundary is thrown away; a frame that
arrives late is refused, not relabelled; and the loop waits for exactly the index asked
for, first sleeping off all but the last 25 frames (in one-second steps — a single long
sleep was itself worth three frames, which is where the old lateness came from). Decoding
runs only after a frame is identified: it costs several milliseconds in a debug build, and
inside the loop it dropped the sampling rate to one grab per two or three frames, which
made the frame asked for slip past and be refused. Measured after the change: the
identifying probe landed on the requested index in all 18 landings, and eleven captures
with the final code (three at the title screen, eight at moving indices) plus seven more
while the decode's placement was still being settled wrote the frame they were asked for
with no refusal at all.

**What still does not repeat is a per-session alignment of the counter, not the boot and not
the grab.** A frame counter names one picture within a session and a picture one frame away
in the next session half the time. Measured directly: four sessions, each capturing the
same five moving indices in one run (`--frames 2400,2450,2500,2550,2600`, so five indices
per emulator process), all five indices of a session in the *same* alignment — session 2
and session 3 were one frame of animation off session 1 at every index (5.45–12.73% of
bytes), session 4 matched session 1 at every index (0.003–0.070%), and no session mixed
alignments across its five indices. Across sessions it splits roughly evenly: in that
batch 2 of 4 sessions were off (2 of 6 session pairs agreed at every index); in an earlier
sample of 12 sessions at a single index, 6 runs were byte-identical, 5 matched each other
but sat one frame of animation away from the six (6.62% of bytes), and 1 differed by 0.057%
(145 bytes).

**The flip is in the emulator's own accounting, and it reaches the guest, not just the
picture.** The guest's framebuffer memory, read at counter-identified moments, differs the
same way: two sessions' 64,000 index bytes at counter 2450 were byte-identical while their
counter-2500 reads differed by 2.02%, and a third session differed from both by 26–30% at
both indices (one animation frame). Yet the demo's *discrete* events stay put: a Lua script
watching the BIOS video-mode byte saw 3@2, 19@404 and 13@4796 frame-for-frame in four
sessions. So the counter-to-guest-event map is fixed while the *continuous* drawing at a
counter can be a frame older or newer. The best candidate in the source is DOSBox's
host-measured clock adjustment: `adjust_ticks_after_present_frame`
(`gui/sdl_gui.cpp:2555-2570`) subtracts the *measured host time* spent presenting from the
emulator's tick budget, so the emulated clock itself carries a host-timing term. Nothing in
the REST API or the settings reaches it.

**The frame-locked alternatives were probed and do not rescue it.**
`GET /video/frame?mode=rendered` really is synchronised — one request blocks for exactly
one vblank, measured at 14.243 ms per request against 0.405 ms for `mode=raw` — but it
returns a rendered image, not the framebuffer: 758×569 (BGR24) by default, and still
640×480 with `render_aspect=square-pixels`, `render_shader=none` and `window_size=640x400`
(the window's minimum height is 480), with no pixel-exact mapping back to the raw frame
(its pixels pass through the render pipeline's adjustments). The video recorder writes every
frame but needs a decoder this machine does not have; Lua runs frame-exact but has no pixel
access. Held screens are unaffected — the same captures repeat byte-for-byte — which is why
Phase 0's still-screen evidence never met any of this. Whether B2–B5 may capture moving
screens at all, and under what policy, is the user's call and not this task's.

**The cost of a boot.** ~5.9 s wall from process start to the first graphics frame (frame
404), the 5 s `AUTOTYPE` wait included; the typed recipe booted in ~1.2 s but at host
timing. A three-frame capture reaching index 2400 costs ~35 s. The setup screen's text is
up at frame 16 (0.23 s), so the 5 s wait carries ~4.8 s of margin and could be shorter if
boot time ever matters.

**`reference/check_a` was re-captured**, because the recipe moved what its indices mean:
frame 400 is now the setup screen, not the title screen. The new captures are the same
two screens at the indices this boot puts them at — `title` at 700/900 (320×400), `part`
at 1350/1450 (320×200) — with `cycles: 30000`, `boot: "autoexec"` and the same key names
in the manifest, and the frames are byte-identical to the ones they replace. The harness
checks were re-run against the new tree: self-comparison `verdict: identical` exit 0, the
`SECOND 5` negative still `verdict: different` (title 99.5609%) exit 1.

**For B2–B5.** Pass `--boot autoexec --cycles 30000`. Boot-relative indices are now one
convention: the autoexec boot's. A part's frames can be found by sweeping as before, but
they no longer move between runs, and a capture taken on a busy host can still land a few
frames late — re-run it if a pair that should be identical is not.

---

## D18 — `compare` searches a frame either side, and says which offset matched (2026-10-11)

B1/D17 left one thing the phase's exit criterion could not survive: even with an exact grab,
a *moving* screen captured at the same boot index lands one frame of animation apart between
emulator sessions about half the time, and the flip is in the emulated clock — nothing the
API or `--set` reaches. The user ruled: **`compare` searches ±1 frame and reports which
offset matched.**

**It is a widening of the alignment, never a tolerance on the pixels.** The comparison is
still exact — a paired frame is 0.0000% or it is a difference, and the DAC widening,
index-vs-RGB and strictness rules are untouched. What moved is only *which* rendered frame
answers for a reference one, and every pair says how far apart the two indices were. The
window is the constant `ALIGNMENT_WINDOW = 1`, not a flag: a knob that widened it would be
the pixel-tolerance mistake wearing another name, and the width is a measurement (D17), not
a setting.

**The rule.** For each reference index in ascending order, the candidates are the rendered
frames actually present at that index, one below and one above, that no earlier reference
frame has taken. The candidate with the fewest differing pixels wins — so the pairing
follows the *picture* — and equal scores go to the smaller offset, then to the lower index.
A still screen therefore reports offset 0 (several candidates match exactly) instead of a
coin flip, and a report that must fail shows the closest picture rather than an arbitrary
neighbour. A rendered frame answers one reference frame only. A reference index with no
candidate is **refused with exit 2** and a message naming the index and the nearest rendered
frame; nothing is ever paired from further away. Exit codes keep their meanings: 0 only when
every reference frame matched at some offset inside the window.

**The pairing engine it replaces was index-blind, and the phase already had the evidence.**
`pair_indices` checked only that the two sides held the same *number* of frames and then
zipped them positionally. `reference/check_a` (now 700/900/1350/1450) against the older
`out/check_neg` (400/600/1100/1150) therefore compared frames 300 apart and printed
`title 99.5609% differing`, exit 1 (PROGRESS.md's Task 13 section). With the window in place
the same command refuses:

```
error: reference frame 1350 has no rendered frame within ±1 of it that no earlier reference
frame has taken: the nearest rendered index is 1150, 200 frames away, and the rendered side
holds 2 frames. Refusing to pair a frame from further away, which would report a number that
means nothing
exit: 2
```

That count check had to go rather than be kept alongside the window: the port's dump holds
*every* tick (`--frames N` writes one frame per tick) while a capture holds a handful of
keyframes, so a count comparison would refuse the very workflow the search exists for. It is
the reference's indices that must all be answered; a rendered side that holds frames the
reference does not is normal, and one that cannot answer a reference index is refused by the
window, per index, with a message that says which index.

**The report says it out loud.** The report is version 2: each frame carries `offset`
(`rendered_index - index`, signed, so the direction is not left to subtraction) and the
summary carries `offset_frames` and the distribution `offsets` (`{-1: 1, 0: 3}`). The
console repeats it, on the frame's own line and once for the capture — as measured below,
`frame 2450: 0.0063% differing (4/64000 px), max delta 121, rendered index 2451 (offset +1)`
and `alignment: 1 of 1 frames paired at a non-zero offset (+1 x1)`. The version moved
because the *meaning* of a paired frame moved: a reader that knows version 1 must not read a
version-2 report as if a frame's counterpart were always the frame at its own index.

**What this could hide, and the evidence that separates it.** A port whose animation is
systematically one frame out would match at a *constant* non-zero offset and pass. **Scattered
non-zero offsets are the oracle's ambiguity; a constant one is a finding about the port** —
with D17's caveat attached, which makes the reading sharper rather than softer: a session's
phase is constant too, so a constant non-zero offset across a capture is a *suspicion*, not a
conviction, and one comparison cannot tell a port bug from the oracle's phase. What settles
it is a second capture — re-capturing the reference in a new session, or comparing the port
against two sessions — because the oracle's ambiguity *moves the offset between sessions*
and a port bug does not. The offset distribution is what makes both readings visible at all,
and it is why every offset is recorded rather than absorbed: offset 0 everywhere, scattered
±1, and one constant non-zero offset are three different findings. Each part's row in
`docs/fidelity/reference-indices.md` is where the offsets and that settling are recorded (B2
writes the first row).

**What the search cannot do by itself.** It aligns frames the rendered side actually holds.
Two *captures* at the same index list hold, between them, only the frames at those indices:
if one session's picture at index 2400 is the other's 2401, neither capture holds the 2401
needed to see that — measured on the real captures (`out/b1-ws-s1` against `out/b1-ws-s2`,
the same five indices 2400–2600 of the moving part screen): every frame paired at offset 0,
the only candidate the rendered side held, and 9.1453–14.6172% of *pixels* differing (D17's
5.45–12.73% was measured in bytes, over index files, not over pixels). The two captures must
therefore also hold the neighbours — one frame beyond each end of the keyframe range — and
`reference capture` was changed so that a run can hold adjacent indices at all: D19.

**Measured on the real captures.** `reference/check_a` compared with itself: `verdict:
identical`, every offset 0, exit 0. `reference/check_a` against the older `out/check_neg`:
refused, exit 2, message above — the same command the old code answered with
`title 99.5609%` (frames 700 against 400 and 900 against 600) and `part 40.7172%` (1350
against 1100), exit 1, which is the mispairing this replaces.

The offset search was then measured against the oracle, on the moving 320×200 part screen:
the reference frame at index 2450, taken from `out/b1-ws-s1` (session 1 of D17's
alignment-batch measurement), and two single frames captured in later sessions, at 2449 and
2451 (`out/b1b-2449`, `out/b1b-2451`, D17 recipe, 30000 cycles, both RGB-only because the
screen moved during the indexed read — D15's coherence check doing its job).

| reference 2450 vs | paired with | offset | differing | verdict |
|---|---|---|---|---|
| a later session's 2449 | 2449 | −1 | 14.8531% (9506 px, max delta 162) | different, exit 1 |
| a later session's 2451 | 2451 | +1 | **0.0063% (4 px, max delta 121)** | different, exit 1 |

The 2451 session was in the *other* alignment: its frame at 2451 is the reference's frame at
2450, to within **four pixels of 64,000** — the small residue D17 measured one session in
five carrying (145 bytes at index 2500). The search found it and reported the offset; the
pixel comparison then reported those four pixels and exited 1. That is the policy working as
ruled: alignment searched, pixels exact, nothing waved through.

A triple in one session was attempted first and **could not be captured**: after writing
frame 2449, the capture loop is ~18 frames late for 2450 and refuses ("the emulator reached
frame 2468 before frame 2450 could be captured"). That turned out to be fixable, and it had
to be: see D19.

The tests are `cargo test -p xtask`. Thirteen of them were run against the pre-B1b pairing
first — the old code with only the new signatures shimmed so they could compile — and all
thirteen failed: the four the plan names, plus the report version, the missing `offset`
fields, the old count-mismatch refusals (`reference: 1, rendered: 3`), and one that shows the
old engine outright lying (`pair_indices(&[700, 900], &[400, 600])` returned
`Ok([(700, 400), (900, 600)])`). The one that carries the ruling is
`compare::tests::a_frame_one_index_later_is_found_and_its_offset_is_reported`: the same
reference frame is measured against the rendered frame at its own index (100% differing,
exit 1) and against a rendered side holding its picture one index later (0.0000%,
`rendered_index: 901`, `offset: 1`, `offset_frames: 1`, exit 0), so the pair is a real pair
of different pictures and the transition is the search's doing, not the fixture's.

---

## D19 — What a grab costs, and a capture that carries its own neighbours (2026-10-11)

B1b's report said a capture could not hold two adjacent indices, so a reference could not
carry the neighbouring frames the ±1 search needs (D18) — and B2 step 4's re-capture check
was therefore unreachable on a moving screen. The measurement that decided it was asked for
rather than assumed: break a grab into its parts and see whether the cost is the indexed
read, and whether the neighbours being RGB-only is enough. It is the read, and RGB-only
neighbours are *not* enough — the fix is that a run's read must not sit between two grabs.

**What a grab costs, measured** (`dosbox-automation 0.85.1` (6d100), `SECOND 2`, 30000
cycles, the silent D17 recipe, timings taken with temporary instrumentation around each
phase of the capture loop):

| phase | 320×200 chain-4 | 320×400 Mode X | requests |
|---|---|---|---|
| identified RGB grab (counter, raw frame, counter) | 0.7–2.2 ms | 0.7–2.2 ms | 3 |
| `video/frame/info` | 0.3–0.4 ms | 0.2–0.3 ms | 1 |
| framebuffer read | 1.0 ms | 10 ms | 1 (chain-4) / 4 planes + 8 port writes |
| **DAC palette: 768 port reads + 256 index writes** | **273.0–323.5 ms** | **260.2–285.0 ms** | 1024 |
| `indexed_matches_rgb` | 0.0 ms | 0.0 ms | 0 |
| `write_frame` (rgb + idx + palette) | 0.1 ms (192 + 64 KB) | 19.8–21.8 ms (384 + 128 KB) | 0 |
| one frame, grab to disk | 325.7 ms ≈ 22.8 frames | 317.8–344.1 ms ≈ 22.3–24.1 frames | |

So the grab itself is ~1 ms — a tenth of one frame — and **the indexed read is 82% of the
cost of a Mode X frame and 99% of a chain-4 one**, the palette dominating it either way (the
title screen's 128 KB index file costs 20 ms to write, which chain-4's 64 KB does not). The capture loop's problem was never
the grab: it was that the read sat *between* two grabs, so any requested index within ~25
frames of the one before was already gone by the time the loop looked for it.

**A capture can carry its own neighbours.** `reference capture` now takes every frame in a
run of indices closer than `INDEXED_READ_FRAMES` (25, the measured read rounded up) before
reading any of them, and the oracle's kept-frame slot became a queue so more than one take
can be held. Two properties matter: the indexed capture itself is untouched — same read, same
`indexed_matches_rgb` coherence rule, same bytes, and only its *start* moves, by at most one
index within a run — and a capture whose frames are far apart behaves exactly as before,
because the rule only fires within a read's reach. Measured after the change:

- `--frames 2449,2450,2451` captures, exit 0, where it was refused before (each frame's read
  is still attempted and still fails coherence on that moving screen, so all three are RGB
  only, as D15 requires).
- A held screen is unchanged: `--frames 700,900` (320×400 Mode X, indices kept) compares
  byte for byte with a capture of the same two frames taken before the change —
  `verdict: identical`, `mode: indexed`, 0.0000%, exit 0.
- Two sessions that disagree about the animation phase now align: `out/b1b-tri-b` (a session
  one frame ahead) against `out/b1b-span-c` paired all three of its frames at **offset −1**
  (0.2141%, 0.5984%, 0.2766% of pixels) with no refusal, where the same pair against a
  capture holding only the same three indices was **refused, exit 2**.

**The convention that follows is asymmetric: the reference holds keyframes only; the
capture that will be compared *against* it holds one support frame beyond each end of the
keyframe range** (`--frames N₁-1 … N_k+1`). Not a style choice — the symmetric version was
measured failing. `compare` pairs *every* frame of the reference, so a support frame sitting
in the reference is itself a reference index, and its own counterpart lies one further out
still: two captures both holding `N₁-1 … N_k+1` still refuse at the outermost index (exit 2,
"reference frame 2451 has no rendered frame within ±1 of it … the nearest rendered index is
2449" — `out/b1b-tri-a` against `out/b1b-tri-b`, 2449–2451 each, phases one frame apart),
while the reference's 2449–2451 against the five-index span 2448–2452 aligns completely (all
three at offset −1). So the two captures are **two different commands**: the reference's
comes from keyframes alone, the partner's adds the support frames. Support frames are
support — they go in the manifest like any other frame, in the scratch directory
(`out/…-recapture`), never in `reference/parts/`, and not in `reference-indices.md`'s table.

**Leave room after a support run.** A batched run is read *after* it is taken, so a batch of
*k* frames spends ~23·*k* rendered frames reading, and a distant index listed too soon behind
it is refused. Measured: `--frames 2448,2449,2450,2451,2452,2540` captures the five and then
refuses the sixth — "the emulator reached frame 2552 before frame 2540 could be captured",
100 frames of deferral for five frames of support. That is a loud refusal and not a
regression (the same list was refused before the batching too), but a capture list must not
put a distant index less than ~25·*k* frames after a run of *k*. Keyframes hundreds of
frames apart are naturally clear of it.

**Two residues, recorded and not thresholded.** A same-phase re-capture of the five-index
span (`out/b1b-span-a` vs `out/b1b-span-b` and vs `out/b1b-span-c`) was byte-identical on
every frame but one: `span-a`'s first captured frame, 2448, differs from both later captures
by the same **47 pixels** of 64,000 (max delta 121) — `span-b` and `span-c` agree with each
other exactly, so that residue is that one session's, the class D17 measured one session in
twelve carrying. The aligned pair above carried 137–383 pixels. So a moving part's re-capture
can be aligned and *still* report `verdict: different`; the number is recorded in the part's
row, and no tolerance is introduced — a pixel tolerance is exactly what the user's ruling
avoided (fix round 1, ruling 3).

---

## D20 — What a reference index means, and the fallback that was not needed (2026-10-12)

Phase 1 closed (B6) with twenty references, a per-part evidence table in `PROGRESS.md` and a policy.
The policy is a **decision**, not a summary: it fixes what a capture index means, what makes a
capture valid, what is done when a part does not reproduce, and — the part a later phase is most
likely to get wrong — that spec §8.4's structural-similarity fallback was **not used**. The full
statement with its measurements is `docs/fidelity/reference-indices.md` **§1.12**; this entry is the
durable version, so a later session does not have to read the method sections to find it.

**An index is boot-relative and valid only under one recipe.** Frame 0 is the emulator's first
rendered frame, and the counter counts emulated frames. Every capture under `reference/parts/` used

```
--arg <block> --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000
```

with the emulator run **silent** (`SDL_AUDIODRIVER=dummy` + `mixer_nosound=true`, D17). Silence is
not decoration: with a real audio device the mixer follows the *host's* clock and the demo's
animation drifts against the frame counter, continuously and differently per run (D17 measured two
otherwise-identical autoexec captures 14.3/35.3/24.6 % of bytes apart until the emulator was also run
silent). The cycles matter less than the recipe — index 900 captured at 25000 and at 30000 is
byte-identical, §1.5 — but a capture that does not pin them is **refused, not produced**, because an
unpinned capture is not reproducible. Every manifest records `boot`, `cycles`, `keys` and
`arguments`; a frame set whose manifest says something else was captured some other way and is not
comparable with the references.

**Three claims of different strength, and a row means one of them.** The boot repeats exactly (menu
frame 16, first graphics frame 404, five runs in five, D17). A **held** screen repeats byte for byte
across sessions. A **moving** screen's alignment does **not**: sessions land in *discrete alignment
groups* and a second capture lands in the same group about half the time (D17), because the emulated
clock carries a host-timing term that neither the API nor `--set` reaches
(`gui/sdl_gui.cpp:2555-2570`). That is why `compare` searches **±1 frame** and reports the signed
offset per frame and the distribution per capture (D18) — and the asymmetry that makes the search
usable is D19's: **the reference holds keyframes only; the capture compared against it holds one
support frame beyond each end of the keyframe range**, and one on each side of every keyframe that
moves (§1.3 rule 1).

**Spec §8.4's structural-similarity fallback was not needed, and the distinction is the point.** ±1
keeps the comparison **pixel-exact** and searches only the *alignment*: a paired frame is 0.0000 %
or it is a difference, and no pixel tolerance exists anywhere in the toolchain. Structural similarity
would give up exactness — "the pictures are similar enough" is a judgement the port cannot audit, and
it is the kind of tolerance this phase refused everywhere else. Where the ±1 window cannot reach a
difference the residue is **recorded and named** (ALKU's 5859 pan, §4.4); where no picture is
presented by a majority the keyframe is recorded as having **no settled content** (§1.8 rule 4).
The fallback stays available in the spec for a screen neither instrument can describe; nothing in
this phase invoked it.

**A part that does not reproduce — the ladder.** Census it by §1.8's rule over every session holding
its window and **re-capture it into the largest alignment group** (this worked and is not marginal:
TUNNELI 8–10 % → 0.0000 %, 3DSINFLD's 16500 43–48 % residual → `verdict: identical`, U2E's 2600 from
0/3 to 2/3). Judge the re-capture against **independent** sessions, never a partner captured in the
same sitting — §1.8 rule 5 exists because that mistake once made GLENZ's reference *worse*. If the
largest support is below half the sessions, file the best *filable* picture and record the keyframe
as having no settled content. Never: a pixel tolerance, a widened window, a structural-similarity
substitution, or an unrecorded re-roll.

**Recorded as unresolved, because a later phase will meet all three:**

- **The oracle's rare excursion.** One session had TECHNO's rings ~30–40 frames early; it did not
  reproduce in either of two VGA-content probes, and it is not the music clock (the player's counter
  agrees to ≈1 music frame across sessions, rate identical to 1e-4), not a fixed offset and not a
  counter stall (0 stalls ≥150 ms at 20 ms resolution to frame 4900). Rare, real, unexplained, not
  reproducible; the ordinary spread is pinned (five mode transitions within 4 frames over four
  sessions).
- **ENDSCRL's deviation.** No session in the tree holds that part's ±1 window: the tool cannot take
  two frames one index apart on its 640×400 screen (five recorded refusals, a sixth attempt killed;
  two remedies tried and both failed *because the grabs are served by the emulator's own process*),
  so its census is an **offset-0 count** — accepted because every session that took the index is
  byte-identical at offset 0, so there is no disagreement for the window rule to inflate. A
  one-frame-out diff there **is a difference, not an alignment**, and the offset-0 agreement proves
  nothing about a ±1 pairing.
- **The CRC-register read race in `xtask`'s oracle.** `crtc_offset_register()` and
  `chain4_enabled()` each read the CRTC's index/data port pair in two requests with the guest running
  in between, so a read can name the other register's value or a mid-write state. The loud half
  refuses the frame (JPLOGO: eight refusals); the **quiet half can be used** and the coherence check
  that would catch a resulting shear is blind on a palette-collapsing screen. Nothing in the tree is
  wrong because of it — every `.idx` from a part that writes CRTC registers was hand-checked before
  it was trusted — but the durable fix (read the pair coherently, or twice and require agreement)
  belongs to whoever next touches the oracle.

**And: the `phase-1` tag is withheld at the close-out.** The plan's B6 step 4 says commit *and* tag;
the final whole-branch review comes after B6 and may require fixes, and a tag that has to be moved is
worse than a tag applied a few commits later. The phase's exit criteria are met and its evidence is
recorded; the tag goes on when that review is clean.
