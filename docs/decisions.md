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
