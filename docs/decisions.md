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
GRID for 1/2/3, HARD for 1 and 0xff.

**Amended 2026-10-13 — BEGLOGO is not a dependent, and its exposure was a wrong-build artefact.**
This entry (and spec §7.4) named BEGLOGO as waiting for `zinfo == 0xf0`. That line is
`ENDPIC/BEG.C:48`, in the build of part 4 that **did not ship**: the 1993 release's part 4 is
`BEG/`'s, which calls `dis_partstart`, `dis_waitb` and nothing else — no `dis_muscode`, no
`dis_exit`, no copper. `docs/fidelity/beglogo.md` §1 settles which build shipped from three
independent measurements (the reference's held frame 900 probes palette entries that
`BEG/SRTITLE.UP` matches and `ENDPIC/SRTITLE.U` does not; `SRTITLE.UP` decodes to that frame's
128,000 indices exactly; the staged `MAIN/DATA/BEGLOGO.EXE` is 30,874 bytes, which no packer
produces from `ENDPIC/BEG.EXE`'s 13,286), and `docs/fidelity/part-sequence.md` §1 carries the
build note. The exposure is therefore **GRID, HARD and the vault** (Phase 8) — none of them this
phase's, and the phase that has to resolve `_np_zinfo` is Phase 3, whose ladder below is
unchanged. BEGLOGO was ported against no music cue at all, and its comparison is exit 0.

Resolution ladder: (1) parse `STMIK.300`'s OMF symbol records for `_np_zinfo` /
`_np_zplus` offsets, then read them live from the emulated original via
`dosbox-automation`'s memory API and correlate against order/row; (2) read `st3play`'s
source for the semantics of "plus flags"; (3) disassemble the STMIK segment.

Escalation trigger: if route 1 fails within Phase 3, implement affected parts against a
substitute cue and record the difference. Do not let it block the phase.

**Amended 2026-10-14 — route 1 is performed, it does not resolve the word, and the substitute
cue is named.** Phase 3's Task 1 walked `STMIK.300`'s `PUBDEF` records (45 symbols, one
segment) and read the words live: `_np_zinfo` is at segment offset `0x08F7`, `_np_zplus` at
`0x08F9`, and **`_np_zinfo` is 0 in every sample of every trace filed under `reference/music/`** (11,408 of
them across the four) — covering MUSIC0's orders 0 and 25…28 and 70…77 and MUSIC1's orders 0
and 42…48 — while `_np_zplus` takes 1 and 2 in both modules (`docs/fidelity/music-clock.md`
§6; `cargo xtask music analyse --trace reference/music/second5`). So the values GRID, HARD
and the vault wait on (`0xf0`, `1`/`2`/`3`, `0xff`) **are not produced by the reference in
any window this phase measured**, and the escalation trigger's condition is met. What the
phase does instead is in `music-clock.md` §6: **`_np_zplus` is the word the port computes**
— the rule there, verified over every sample of every filed trace — and, for
`dis_muscode`, **the port returns `_np_zinfo` as 0**, which is what the reference's own word
holds for as long as this phase measured it. A part that waits on `dis_muscode(1)` — HARD's
`HARD/B.C:138`, or GRID's `GRID/MAIN.C:72`/`:160` waiting on 2 or 3 — therefore **waits for
ever** against the port's 0, exactly as it does against the reference's 0, and the parts that
wait on it are Phase 8's to write against a cue this phase has measured rather than against
one it inferred. Route 2 (`st3play`'s "plus flags") and route 3 (disassembly) are **not**
taken: route 1 did not fail, it answered — the answer is that the word stays 0 in every
window the phase has, and a mechanism for its other values is not measured by any of them.

**Closed 2026-10-25 (Phase 9 Task 10) — the substitute is adopted and registered.** `dis_muscode()`
returns 0 (`sr_engine::music::muscode_from`), the reference's own reading at every sample of every
trace filed under `reference/music/` (`cargo test -p sr-audio --test trace
zinfo_is_zero_in_every_filed_trace`). That is spec §7.4's escalation clause taken, and its
difference is `APPROXIMATIONS.md` **A56**: 0 in every compared frame, because no part the demo runs
calls `dis_muscode`, and the vault's callers run behind the standalone DIS and model their one start
key (A40, A46, A52). What stays open, and is carried there, is the word's other values, which no
measured window shows.

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

Its `README.1ST:34-38` lists five files in the unpacked directory — itself, the two demo files, and
the two distributor text files (`FCINFO10.TXT`, `FILE_ID.DIZ`) that no part reads — so the demo
itself is exactly `SECOND.EXE` and `REALITY.FC` unpacked into one directory
(`part-sequence.md` §3 makes the same point), and the README documents the command line:
`SECOND 1`..`5` start at the beginning, title
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
decides every frame diff — the ruling above — and the difference is recorded rather than
resolved in favour of either build.

**First data point, 2026-10-13 (Task 5, TUNNELI): the tree's `TUNNELI/TUN10.PAS` and its own
`TUN10.EXE` are *not* the build that put the reference's frames on screen.** `TUN10.PAS:122`
writes a second grey ramp of `(64-x) *3 div 4` at DAC entries 128..192 and `TUN10.EXE` compiles
it as written (`idiv 4`, image offsets `0x1d0`-`0x1e3`), while every reference frame is drawn
with `(64-x) *3 div 5` at those entries — exact at all 42 entries the four frames exercise (38 of
them with a non-black reference pixel), and the unique
best of 81 candidate `(mul, div)` pairs — 70 distinct ramps — rendered against the four frames
(`APPROXIMATIONS.md` A6,
`docs/fidelity/tunneli.md` §5). The tree's `MAIN/DATA/TUNNELI.EXE` is PKLITE-packed, so which
of the two copies of it put that ramp on screen is not resolvable from here; what *is* settled
is that the frames and the tree's source disagree, and D12's ruling above ("the distributed
release is the reference of record for every frame diff") decides which of them the port
follows: the frames. A part whose source and shipped build disagree is therefore not a
contradiction to resolve but a measurement to take, and the difference is recorded — here and
in `APPROXIMATIONS.md` — rather than the newer build being called the winner.

**Closed 2026-10-25 (Phase 9 Task 10) — unresolved by design.** The open question — whether the part
EXEs in `MAIN/DATA/` are byte-identical to those packed inside `SECOND.EXE` — is not answered and
this project will not answer it: the staged EXEs but U2A's and U2E's are PKLITE-packed, `SECOND.EXE` packs them again,
and nothing here unpacks either. The frames arbitrate instead, which is the ruling above and D24's
procedure, and every divergence it found is registered against the frames with its packed EXE as
the lead (A6, A9–A11, A17–A20, A22–A25, A28–A31, A37). The tree does say one thing, and it is
measured: `MAIN/PACKFINA.INC`'s length words are the staged file's size for 18 of its 22 EXEs and
not for `U2A.EXE` and `U2E.EXE` (whose staged copies carry no PKLITE header and are 2.06× and 1.93× the listed
length), `LNS&ZOOM.EXE` (four bytes) or `START.EXE`, so for those four the staged file is certainly
not the packed one (`APPROXIMATIONS.md`'s head has the command).

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

**Amendment, 2026-10-15 (Task 7, D27): "entry-point" is true of the two sites above and not
of the table's other two.** The claim this entry makes — that the restarts "fire *only* when
the blocks leading up to them did not run" — holds for `MUSIC1` at order 42 (`:856-863`, whose
guard is `& 4` and not `& 2`) and for `MUSIC0` at order 25 (`:898-905`, `& 16` and not `& 8`),
and for the hidden part's (`:889-894`, `& 80h`, which only `partsmask[8]` carries). It does not
hold for the other two sites the shell has, and both are the shell's *own* restarts rather
than the ones this entry was written about:

- **`:880-885`** (`MUSIC0` at order 18) is guarded by `whattorun & 8` **alone** — one gating
  test (`:880`/`:881`), and its `jz` lands on `:888`, *past* the restart — so it fires for
  every block whose mask has bit 3: `SECOND 1` (0x1f), `2` (0x1e), `3` (0x1c) and `4` (0x18).
  It is the entry point only of `SECOND 4`, whose plan begins at U2E; for the other three the
  shell reaches it **mid-run**, between JPLOGO (`exe18`) and U2E. **It is not the only
  single-test site**: `:889-894` has one too (`:889`'s `& 80h`, `jz @@skipp5`), and what keeps
  *that* one entry-point-only is the masks — only `partsmask[8]` carries bit 7 — while what
  makes `:880-885` mid-run is that four masks carry bit 3. The two-test shape belongs to
  `:856-863` (`& 4` and not `& 2`), `:898-905` (`& 16` and not `& 8`) and `:813-818` (`& 2` and
  `& 1`).
- **`:813-818`** (`MUSIC1` at order 0) is guarded by `whattorun & 2` — the region `:801-802`
  opens and `:855` closes — **and** `& 1`, and fires for `SECOND 1` mid-run, between BEGLOGO
  (`exe3`) and GLENZ (`exe4`).

So the table this entry prescribes is right about **which module and order a block's run
begins on** — which is what Task 7's `shell::MUSIC_RESTARTS` implements, one row a block — and
the word "entry-point" belongs to the row as a *starting* position, not to every site it cites.
A run that reaches U2E inside blocks 2 or 3 (or GLENZ inside block 1) needs a **mid-run**
restart at that point in the sequence, which the one-row-per-run model has no place for yet;
nothing the port can run today gets that far (`--block 2`'s plan stops at GLENZ, unported;
`--block 3` ran no part when this entry was written and since Phase 5 runs MNTSCRL through JPLOGO
but stops at U2E, unported), and `docs/fidelity/music.md` §12 and D27 carry the
citations.

*(Phase 6 update — superseded by D34/D35: the U2E-boundary mid-run restart is implemented
(`MUSIC_RESTART_AT`, `MusicStart::restart`, waiting 4) and `--block 3` now runs through it;
`--block 1` now reaches PAM and stops at GLENZ. **Phase 7 update:** both mid-run sites are
implemented and reached — block 1 takes `:813-818`'s `MUSIC1@0` and `:880-885`'s `MUSIC0@18`,
blocks 2 and 3 take `:880-885`, and blocks 1/2 run to ENDSCRL — so no block the port runs stops
at an unported part. The paragraph above is D13's record.)*

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

**Closed 2026-10-25 (Phase 9 Task 10).** The "Not resolved" paragraph is answered: `dx` is
`dis_musplus()`'s reading of `_np_row` and `_np_zplus` — `row` in one phase, `row − 64` in the
other, −32 for no cue — whose `_np_zplus` rule D26's traces measured (`music-clock.md` §6) and D8's
2026-10-14 amendment adopted. The wait is implemented on it (D38) and its exit is measured in the
full run at +2 frames against the original (`APPROXIMATIONS.md` A53's cue exit, 16632 against
16630). The restart table this entry prescribes is `shell::MUSIC_RESTARTS` and `MUSIC_RESTART_AT`
(D27, D34, D38).

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

**Closed 2026-10-25 (Phase 9 Task 10) — the left-open probes, one by one.**

- *The `-conf`-anchor mount route*: still untested, and moot — `xtask` writes a scratch primary
  config with `mount_allowed_bases` (`CLAUDE.md`'s capture list, item 2), and no capture needed
  another route.
- *Whether `No sound` lets the reference run*: not tested, and not wanted: the captures run the
  SB16 under the emulator's own silent mode (D17), which keeps the demo's music clock, while the
  shell's no-sound output mode writes `_zspeedadjust2` (`MAIN/U2.ASM:384-389`, D46) and would not
  be the reference's clock.
- *Whether `AUTOTYPE` can drive the setup menu*: yes — `--boot autoexec`'s `AUTOTYPE left enter`
  (D17), the recipe every capture since Phase 1 uses.
- *Capture determinism at fixed indices*: answered by D15 and D17 (a held screen repeats byte for
  byte; a moving one lands in alignment groups, which `compare`'s ±1 search handles, D18).
- *A paletted raw frame's palette payload*: never observed and not needed — the palette is read
  through the DAC ports (item 3 above, `xtask/src/oracle/dosbox.rs`'s `read_palette`).
- *The unexercised endpoints*: `/memory/` (the indexed reads and every music trace), `/io/port`,
  `/script/*`, `/input/*` and `/video/text` are now used; `/api/v1/mixer`, `/capture/*`,
  `/memory/search`, `/memory/freeze`, `/dos/internals`, `/drive/swap` and `/mount/lock` are still
  not, and nothing through Phase 9 needed them (`grep -rhoE '"/api/v1/[a-z/_-]+' xtask/src`).

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
normal and does not shift the demo-relative mapping. And the indexed read costs **1,024
requests for the DAC alone** — 768 port reads plus the 256 index writes that select them —
plus one framebuffer read per plane (four, or one for chain-4) and eight port writes, so it
takes roughly a third of a second: it is only meaningful where the screen holds, and the
capture checks the
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
   `docs/fidelity/APPROXIMATIONS.md`; PANIC landed in Phase 5, and the filed captures are the
   displayed area only (item 8 below), so they carry no border to measure — **A1** records the
   padding as still unmeasured and the Phase 9 fidelity pass as its owner.
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
warning about (`vga_draw.cpp:2245-2293`) — subtracts from it. Neither has been met yet, and the `+ 1`
itself **has now been confirmed against a capture**: PLZPART is the first part that splits, and
`reference-indices.md` §1.9/§13.2 records the measurement — `pompota` (`PLZPART/COPPER.ASM:126-143`)
alternates CRTC 0x18 between `0x3c` (60) and `0x3d` (61) frame by frame and the captured frames' lower
view starts at **62, 61, 62**, with `Split::scanline` the register plus one; and the independent
cross-check is `do_drop`'s own table (`COPPER.ASM:175-182`, `dtau`), whose register values 194/201/208
sit one below the measured first content rows 195/202/209. So the mapping is measured rather than
trusted; the two caveats above remain open for whatever mode meets them.

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

### Open item — `View`'s origin is `u16` and the surface is not — **closed in Phase 2**

**Closed in Phase 2, Task 3 (2026-10-13). The origin widened to `u32`.** `View::start_addr` is
`u32`; `View::set_origin` takes a `u32` pixel and writes it there — the field with room for it —
zeroing `x_offset`; and `source_index` is unchanged in shape, summing the origin in `usize`.
`x_offset` stays `u16`: it is an offset *within* a displayed line, bounded by the widest line rather
than by the buffer. Spec §4.2 is amended in place, below the struct sketch.

**The arithmetic that forced it.** CRED's display start is `tw_setstart(160*200)`
(`CREDITS/MAIN.C:167`): 32,000 planar bytes, which at 80 bytes per 320-pixel logical line is
logical line 400 — **128,000** pixels. Its displayed row stride is 160 planar bytes (CRTC 0x13 =
`0x5013`, `CREDITS/TWEAK.ASM:43`), 640 pixels, so each displayed row steps two logical lines, which
is the geometry the port states as `y_line_offset = Some(640)`. ENDSCRL's display start is
`yscrl*80 + 80` (`ENDSCRL/MAIN.C:93-95`), one 640-pixel line per written line: **256,640** at the
end of the original's 401-line ring, and **256,000** in the port's 400-line one, whose surface is
exactly that wide. The most two `u16`s can sum to is 131,070, so both are out of reach — and out of
reach *silently*, because an origin that wraps paints a live-looking picture several hundred lines
from where it belongs. Neither part can be diffed until the field can name its own display start.

**The `u16` was protecting against nothing.** The two fields were as wide as they were because they
were copied from the CRTC's 16-bit register pair — and that pair counts *bytes within a plane*, not
pixels. A 16-colour planar scanline is 80 bytes, so 16 bits spans a 400-line buffer in the
original's own layout, which is why the original never notices. The port's `View` counts pixels and
sums the two fields into one origin, so the copied width buys it nothing: an origin stops at
131,070, which is 124,930 short of the 256,000 ENDSCRL's ring needs (256,000 − 131,070). The
alternative — modelling the CRTC's per-plane byte counter — would put planar addressing into an
engine that deliberately does not have it (spec §4.2: "The Mode X plane arithmetic disappears"), so
the field widened instead.

**The test card's walk was the other thing the ceiling held, and it stays three rows.** It was
corroborated when the card was written: Task 8's implementer hit the same ceiling and clamped the
card's split walk to 3 rows, with `SPLIT_WALK_ROWS` derived from `u16::MAX` so it would follow if the
fields widened. Following that derivation through after the widening asks for a walk of some 6.7
million rows, one surface row per step — a hang, not a test. The constant is a plain `3` now: the
field no longer bounds the walk, the *surface* does (the window need only stay inside one, and rows
100..=201 of a 400-row surface are all it reads), and three rows are what the card wants to exercise
— a window visibly live, stepping a row per tick. The card's pictures are unchanged, measured rather
than assumed: fifteen dumped frames spanning all five phases and the walk, taken before and after
the widening, are the same bytes.

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
  **Closed.** `acfe436` made that fix (`compare::resolve`/`collapse`, shared by every `--out` and
  `--sheet` guard through `refuse_output_inside`), and Phase 9 Task 5 pins it for both writers
  that land frames next to the reference: `xtask/tests/rediff.rs`'s
  `an_out_through_dot_dot_into_the_reference_is_refused` runs `compare` and `rediff` with
  `--out <tmp>/missing/../ref/nested` and asserts exit 2 with nothing created.
- **`phase_beam_compose_staircases_the_palette_down_the_frame` cannot tell a monotonic ramp from a
  sawtooth.** It passes against both. The property is currently guaranteed by the constant
  (`8 + band * 7`) rather than by a test; a strict-increase walk over all eight band boundaries
  would pin it.

**Closed 2026-10-25 (Phase 9 Task 10).** **Item 6 is decided by derivation:** the border the
parts' CRTC timing gives is 8 / 0 / 3.5 / 3.5 source pixels (left, right, top, bottom) at 320×200,
not whole at every geometry, so the presenter's pad stays 8 per side and is the port's presentation
choice (`APPROXIMATIONS.md` A1, `cargo test -p xtask --test approximations
a1_the_derived_border_is_not_whole_at_every_geometry`); the oracle renders no border to measure.
**The deferred minors:** the `..` guard is fixed (above, Task 5's tests). The staircase test is
**carried**: it is the Phase 0 test card's, which no block plays and no reference compares, and the
constant still guarantees the property.

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
capture that will be compared *against* it holds `K-1, K, K+1` around every keyframe that
moves, and nothing around one that holds.** B1b measured the ends-only form
(`--frames N₁-1 … N_k+1`) and it is sufficient only when the keyframes are **contiguous**;
B2's measurement showed it insufficient on the real parts, which are not — a session's
phase is uniform across a capture, so an interior moving keyframe answered from its own
index pairs at offset 0 and reports the one-step difference instead of aligning, and a held
keyframe's ±1 candidates are the same bytes and need no support
(`docs/fidelity/reference-indices.md` §1.3 rules 1–2, §1.4). Not a style choice — the
symmetric version was measured failing. `compare` pairs *every* frame of the reference, so a
support frame sitting
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
*k* frames spends ~20·*k* rendered frames reading — the 100 frames of deferral below, for five
frames of support, are 20 a frame, and B2 measured three reads costing 57 frames (~19 each,
`reference-indices.md` §1.3) — and a distant index listed too soon behind it is refused.
Measured: `--frames 2448,2449,2450,2451,2452,2540` captures the five and then refuses the
sixth — "the emulator reached frame 2552 before frame 2540 could be captured", 100 frames of
deferral for five frames of support. That is a loud refusal and not a
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

**Amendment (2026-10-19, Phase 6's close-out) — a fit corpus is a third kind of reference.**
Phase 5 committed `reference/parts/plzpart/band/` (six frames, 8400…8800, filed by the
sanctioned `reference capture` path) so that A23's cube projection could be fitted against
evidence a clone can re-run; the phase-5 close-out ruled that acceptable and parked this
amendment. A **fit corpus** is neither a keyframe nor a trace. A keyframe is a screen the
port must reproduce at a run index, paired by `compare` with its ±1 supports; a trace is the
original player's own state, a row a frame (`docs/fidelity/music-clock.md`). A fit corpus is
a run of reference frames used to **determine a constant or a phase** by fitting a model
over them — here the silhouette bands gave `yy*213/zz + 100` (0.46 % at 213/100
against 9.99 % at the tree's 142/66 over the six frames; **retracted 2026-10-26**, D24's
correction below) and the phase state (frames 136, page 4, start 0x0028, music 68:46). It lives under `reference/parts/<part>/` with a manifest whose per-frame `sha256`
`reference verify` checks (D23), and it is named by the test that consumes it. Its indices
are **not** reference keyframes: nothing pairs the port against them with `compare`, they
are not in `reference-indices.md` §2's table, and a difference at one of them is a fit
residual, not a keyframe verdict. Fit corpora are committed rather than left under `out/`
for the same reason a keyframe is: a fit that cannot be re-run from a clone is not evidence
(D25). They are the one kind whose *derivation* — the band mask, the fit range — is part of
the evidence, where a keyframe's is the picture itself.

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
usable is D19's: **the reference holds keyframes only; the capture compared against it holds
`K-1, K, K+1` around every keyframe that moves and nothing around a held one** (§1.3 rules 1–2 —
the ends-only form `N₁-1 … N_k+1` is enough only for a *contiguous* range, which no part in the tree
has).

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
TUNNELI's 8.07–10.18 % on all four keyframes → 0.0000 % on three and 0.0891 % on the fourth,
3DSINFLD's 16500 38.9031 % residual → `verdict: identical`, U2E's 2600 from 0/4 to 3/4). Judge the
re-capture against **independent** sessions, never a partner captured in the
same sitting — §1.8 rule 5 exists because that mistake once made GLENZ's reference *worse*. If the
largest support is below half the sessions, file the best *filable* picture and record the keyframe
as having no settled content. Never: a pixel tolerance, a widened window, a structural-similarity
substitution, or an unrecorded re-roll.

**Recorded as unresolved, because a later phase will meet all three:**

- **The oracle's rare excursion.** One session had TECHNO's rings ~30–40 frames early; it did not
  reproduce in either of two VGA-content probes, and it is not the music clock (the player's counter —
  the 16-bit word at physical `0x105B0E`, advancing 0.7424 a frame, `reference-indices.md` §1.7 —
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

**Closed 2026-10-25 (Phase 9 Task 10) — the three unresolved items, as Phase 9 leaves them.**

- *The oracle's rare excursion*: carried. Phase 9 met it nowhere — `cargo xtask rediff` holds every
  filed TECHNO frame at 0 and the full run's TECHNO seam at +2 (A53) — and it stays unexplained and
  unreproduced.
- *ENDSCRL's deviation*: carried. Its census is still an offset-0 count: Phase 9's full-run
  captures took its opening at three-frame spacing (46280 / 46283 / 46286, `reference-indices.md`
  §26), not a ±1 window, so a one-frame-out diff there is still a difference, not an alignment
  (A11).
- *The CRTC read race*: carried, and met again. `chain4_enabled()` and `crtc_offset_register()`
  still read the index/data pair in two requests (`xtask/src/oracle/dosbox.rs`), and the lead-in
  captures' refusals from 439/444 on are one of the two readings `reference-indices.md` §25.6
  gives for them. The fix — read the pair coherently, or twice — still belongs to whoever next
  changes the oracle.

## D21 — The shell: the run's numbering, the DAC it starts from, and where it ends (2026-10-13)

Phase 2's first task. Two things it settles that a reader of the plan would otherwise have to
re-derive, and one API change it needed.

**A run's index is the reference's own, and it is one number for the whole run.** Every index under
`reference/` is a boot-relative emulator frame counter (§1.12, D20) — the demo's first graphics
frame is 404 and BEGLOGO's keyframes are 430/470/560/900 in that space — while `sr-engine` numbers
each part's own frames from zero. `shell::Sequence` therefore owns one `u32` beside the engine and
advances it by exactly one per tick, whoever drew the frame: `shell::boot_anchor` supplies the start
(404 for `beglogo` and `endlogo`, the phase's two block entries, and `None` for everything else,
whose starts the evidence gives only as windows) and the index is *continuous across a part
boundary*, which is what makes CRED's 1440 reachable only through ENDLOGO's measured length.

**`Sequence::index()` is the run's cursor, not the frame's label.** The plan's interface list calls
it "the reference index of the frame just produced"; its own test asserts that a run anchored at 404
reads 404 before any tick and 405 after one, and the two cannot both be true. The port follows the
test: `index()` is the index the *next* tick draws at, the frame `frame()` holds was drawn at
`index() - 1`, and `--dump-frames` compares against `index() - 1`. `headless.rs` already worked this
way with `Runner::part_frame() - 1`, so this is the existing idiom rather than a new one.

**A run that does not begin at a measured anchor says so.** Its frames are numbered from zero and
`index_is_reference()` is false; the dump's manifest carries `index_origin` (`boot` or `part`) and
`anchor` (`u32` or `null`) so that two captures whose indices look comparable and are not cannot be
diffed as if they were. `compare` ignores both fields.

`boot` keys on **a run that begins at a part with a measured anchor**, and not on a run being a
block's: four of the six blocks begin at parts nothing measures — `SECOND 1` at ALKU, `3` at MNTSCRL,
`4` at U2E, `U` at DDSTARS — so `--block 1` is not `boot`-numbered merely for being a block run, and
`--part beglogo` is. The two coincide for `SECOND 2` and `SECOND 5` today only because those are the
phase's two block entries.

**A run holds where its parts end, rather than stopping.** When a run's plan is done — or its next
part is one the port has not written — the shell keeps ticking through an internal part that draws
nothing, and touches nothing of the surface, the palette or the view, so every tick composes them
exactly as the last real part left them, and the index keeps counting. That is what the original's
loader shows between two EXEs, and it is why `--part beglogo --dump-frames 430,470,560,900` is
answerable: `BEG/BEG.C:87` ends `main()` when the fade has converged, and the reference's 900 is a
*hold* belonging to the loader and the next part's music wait (§3.5), not a frame BEGLOGO draws.
`SequenceStatus::Finished` is therefore reserved for a run that has *ended* — a part the port has that
could not be started — and `--dump-frames` treats an index such a run never reached as a failure
rather than a hole in the dump.

**The one thing a hold does not leave alone is the copper.** Installing the hold part goes through
`Engine::start`, which clears every copper slot (`crates/sr-engine/src/engine.rs:111-113`), the same
way each part start does. A callback the retiring part registered therefore stops cycling during a
hold. It costs nothing here — the hold is a static screen and no part in this phase leaves a callback
running past its own end — but a later part that expects its copper to outlive it will find this, and
the honest description of a hold is "draws nothing, and leaves the surface, the palette and the view
alone", not "touches nothing".

**A held frame is not distinguishable from a drawn one, and the manifest cannot say so.** A
`FrameEntry`'s `part` names the part whose screen the frame is, which during a hold is the last part
that ran, and the frame itself carries no mark saying nothing drew it. So
`--part beglogo --dump-frames 1200` answers GLENZ's index with BEGLOGO's held screen, and `compare`
reports exit 1 (they differ) where exit 2 ("could not compare") would be the truth. The design cannot
do better *yet*: the port has no way to know where the reference's hold ends until GLENZ exists to
end it, and inventing a boundary now would be a fitted constant of exactly the kind Review Focus 5
forbids. Nothing in this phase is affected — 900, the index BEGLOGO's acceptance command asks for, is
inside the hold and correct — and the part that will make the distinction meaningful is the one that
ends it.

**The DAC starts where the hardware starts.** BEGLOGO's keyframe 430 is index 15 over the DAC the
machine already had — `BEG/BEG.C:20-21` fills A-space with 15 and only writes the DAC at `:25-34`,
after the 32 vertical blanks — so a shell that began from a black palette would render 430 black.
The shell installs the table an emulator mode set leaves, read out of the oracle's own source:
`INT10_SetVideoMode`'s `M_VGA` case hands `palette.vga` to `write_palette_dac_data`
(`src/ints/int10_modes.cpp:1977-1987`, `:1152-1159`), entries 0-15 of it are `cga_colors_default`
(`:404-412`, `cga_colors` defaults to `default`, `src/gui/render/render.cpp:1917`), and it is **248
entries, not 256** — the last eight are left at whatever the DAC held, which is black. Note where
the table lives: the task brief said `vga_memory.cpp`, and it is `int10_modes.cpp`. The reference's
own probe agrees with the table at the one entry it constrains: `reference/parts/beglogo/
manifest.json` records frame 430's entry 15 as `(63, 63, 63)`, which is `br_white`.

**`sr-engine` gained `Engine::palette_mut()`** — not a structural change but a file the task's plan
did not list, so it is recorded here. The shell has to install the DAC before it starts the run's
first part, and the engine exposed `surface_mut` and `view_mut` for exactly that purpose and no
palette counterpart. `Engine::start` deliberately does **not** reset the palette: the original's
parts are separate EXEs that inherit the DAC the part before them left, and the port keeps that.

**The keyboard's exit flag needed a door into the engine, and this is it.** ESC is not the window's
business: `checkkeys` sets the DIS's `exitflag` (`DIS/DISINT.ASM:112`, `:151-153`) and `exit_2`
(`:186-190`, "returns 1 if part should exit") hands it to the part's own loop, so what ends the part
is *the part*, and the shell's next act is the next part. The engine had `PartCtx::request_exit` —
the door a part opens for itself — and Task 10 added the same door from outside,
`Engine::request_exit` (`crates/sr-engine/src/engine.rs`), with each band's context now seeded from
the engine's flag rather than a literal `false`. That is also what makes the flag **sticky**, which
it already claimed to be: a request made from outside, or in an earlier band, is visible in every
later context until the next `start` clears it. `Engine::palette_mut` was Task 1's one engine
addition and this is Task 10's.

**Who acts on the flag is the port's one departure here, and it is worth stating.** In the original
the *part* returns — `exit_2` is called by the part's own loop and `while(!dis_exit())` is how
ENDSCRL ends. In the port **no part polls `ctx.exit_requested`**: the engine carries it and
`Sequence::tick` retires the part at the end of the tick it was set in. The two agree on everything
the phase can observe (the part stops, the screen holds what it left, the next part starts at the
next index), and they differ the moment a part writes its own `while(!dis_exit())` loop: such a part
would read the flag and return by itself, and it would be the first place the difference could show.
The engine-level claim is tested (`a_request_from_outside_the_part_reaches_the_parts_own_context`);
the shell-level one is `the_exit_flag_asks_the_running_part_to_quit_and_the_run_goes_on`.

**An ESC press between two parts is kept, not dropped.** One iteration after a part retires there is
no part to ask, and the port drains SDL's events once a tick, so a press that lands there would be
lost — which is exactly where the second press of "get me out" lands when the first one ended a
block's last part. The original does not lose it either: the key waits in the BIOS buffer and the
next part's `checkkeys` consumes it (`DIS/DISINT.ASM:119-123`). `window::EscapeKey` is the port's
buffer: the press waits until a part can be asked, and with nothing left to play it closes the
window.

**A block run was headless-only until the window drove a sequence.** Task 1's window still drove one
`Runner`, so `--block` was refused there rather than silently running the default part instead. Task
10 replaced that runner with the same `Sequence` a headless dump drives — one run, one counter, one
plan — so the refusal is gone, and what is still refused in **either** mode is a block whose first
part the port cannot start at (`--block 3` begins at MNTSCRL, and the command line says so before
the window opens).

**Ctrl skips one part in the port and every later one in the original.** `partexecute` tests
`ctrldown` before running a part and returns without it when the key is down (`MAIN/U2.ASM:205-207`),
and `ctrldown` (`:185-203`) *keeps* its carry from the first time it sees the key down: in the
original a single Ctrl skips every part from then on, and the demo runs to its end in one jump. The
port's window is inside a part rather than in front of one, so Ctrl there drops the part the key
interrupted and the run goes on with the next one, one index later. It is a knowing difference, not a
silence: the original's fast-forward is the window's close button, and the port's Ctrl is "skip this
part" — which is also what the phase's Task 10 asked for.

**Corrected 2026-10-25 (Phase 9 Task 10) — the ESC paragraph above.** "The original does not lose it
either" is not so between two parts: after each part `partexecute` calls `flushkbd`
(`MAIN/U2.ASM:211`, the routine `MAIN/U2A.ASM:537-546`), which reads and discards up to 256 waiting
keystrokes, so a key pressed after one part stopped reading and before the next began is dropped.
The port keeps it; that is a knowing difference, registered with the Ctrl skip as
`APPROXIMATIONS.md` **A58** (0 in every compared frame — no capture presses either key).

## D22 — `dis_waitb` counts the overrun, and spec §6.2 said it discarded it (2026-10-13)

Phase 2 read the source before porting the parts that pace themselves with it, and §6.2 was wrong in
a way that would have made the port *look* faithful and pace wrong.

**What the source does.** `waitb_1` (`DIS/DISINT.ASM:161-170`, the `INDEMO` arm at `:165-169`) is
four instructions: `mov ax,cs:copperframecount`; spin `cmp`/`je` until the counter differs; `mov
ax,cs:copperframecount` again; `mov cs:copperframecount,0`; `ret`. So the function **reads the new
value, returns it, and zeroes the counter after the read**. A part whose work overran *k* frames
while it drew gets *k+1* — the overrun is counted, not discarded.

**Spec §6.2 said the opposite** ("the counter reset discards the overrun... a part that overruns its
frame budget gets '1 frame', not 2"), which is a reset-before-the-read reading of the same four
instructions. It is amended in place, with a pointer here.

**Why it matters to the parts.** Two call sites make the return value load-bearing:

- `TUNNELI/TUN10.PAS:200` is `for sync := 1 to frames`, where `frames` is the value `waitr`
  (`:52`, function 1) got from `waitb_1` — the loop takes *that many* animation steps.
- `ENDSCRL/MAIN.C:47` is `if(dis_waitb()==1) dis_waitb();` — "if exactly one frame passed, wait for
  another", which is its own comment on the semantics: a return of 1 is the normal case, and
  anything else means the part is behind and must not wait again.

**The port's model.** One tick is one frame and a part's `frame()` is called exactly once per tick,
so the value is always 1 and the overrun case cannot arise. Nothing in the port needs to compute it;
what a part *does* need is to take one animation step per `frame()` call, because that is what "the
value is always 1" means. A part that assumes a different step count is not wrong about `dis_waitb`,
it is wrong about how far its own animation moves each tick, and the keyframe diffs are where that
shows.

## D23 — The reference record is machine-readable, and a frame's digest covers both its files (2026-10-13)

Phase 1's close-out left one failure unreported: the reference tree's bytes were pinned by
`docs/fidelity/reference-indices.md`, a prose document, and no test read `reference/`. Comparing the
tree with itself catches a *missing or truncated* file, never **a byte changed in place**. The
document could not be the machine's record — its close-out measured 90 rows in two naming shapes
against 105 `frame_*` files, 24 with no row by name and 10 carrying more than one digest — so the
record moved to where the files are written.

**What changed.** `reference capture` writes the digest into the frame entry of the manifest it writes
beside the frame: `sha256` for the RGB frame, and `idx_sha256` for the index plane. `manifest.json`'s
`version` stays 1 — the fields are additive and `serde` defaults them, so every existing reader
(`compare`'s `Manifest` has no `deny_unknown_fields`) keeps working. `cargo xtask reference verify
--tree <DIR>` walks a tree with `compare`'s all-the-way-down walk and checks each capture against its
own manifest: every named file present at the length its geometry implies, every `frame_*` file named,
every digest matching. Exit 0 clean, 1 any finding, 2 could not check. `reference backfill --tree <DIR>`
is the one-shot that wrote the fields into the 24 manifests filed before `capture` recorded them.

**Why `idx_sha256` and not just the RGB digest.** The plan's interface named one field, `sha256`. The
indexed side is what `compare` measures with whenever both sides have it — it is stricter than widened
RGB — so a byte changed in an `.idx` is a change to what every later diff is measured against, and a
digest of the RGB frame alone would have reported the tree clean. The field is optional and is written
only when the plane is (`write_frame` computes it inside the branch that writes the file), so a frame
whose coherence check refused its indices records no digest of a file it does not have. *Cost if wrong:*
one extra optional field on the capture side, which no reader is required to look at.

**Why a frame with no recorded digest is a finding (exit 1), not a pass.** A manifest that records no
digest is the state this tool exists to report; skipping it would print a green run over exactly the
hole the record was built to close. *Cost if wrong:* a tree that predates the field is red until
`backfill` is run once, which is the intended prompt.

**Why `backfill` refuses a digest that disagrees with its file.** Filling in what is missing and
writing over what disagrees are different acts: the second would let a backfill run *after* a frame
changed record the change as the truth, making the record describe whatever the tree happens to be —
worse than no record, which is the failure the plan's ruling on this exception names. *Cost if wrong:*
a frame whose digest must genuinely change cannot be re-recorded by `backfill`; it needs a re-capture,
which is what changed it in the first place.

**The exception, and its evidence.** Writing inside `reference/` is the plan's single narrow exception
(input-only exists to protect the *frames*; this writes metadata beside them). The evidence is
`PROGRESS.md`'s Phase 2 section: the frames hashed out of band before and after the backfill with
identical lists, the second run changing no byte, and all 105 digests checked against the pre-change
list rather than against the tool's own work.

## D24 — The vendored tree and the distributed release are not the same build: the rule, what the phase found, and the procedure for the next one (2026-10-13)

D12 settled *which* artifact decides (the distributed release) and recorded one data point. Phase 2
produced two more, and they are different kinds of difference, so this entry states the **procedure**
a later phase follows when a reference frame disagrees with a faithful port. Phases 4–8 will meet it:
every one of them diffs a part whose shipped build may not be the tree's.

**The three instances, each measured in the part's own note and in `APPROXIMATIONS.md`.**

1. **TUNNELI — one expression.** `TUNNELI/TUN10.PAS:122` writes a second grey ramp of
   `(64 - x) * 3 div 4` and the tree's own `TUN10.EXE` compiles exactly that (`idiv 4`, image offsets
   `0x1d0`-`0x1e3`), while every reference frame is drawn with `(64 - x) * 3 div 5`: exact at all
   **42** of the ramp's entries the four frames exercise, and **unique** over 81 candidate
   `(mul, div)` pairs (70 distinct ramps) — 1,108 px from the reference's colour field at 3/5 against
   at least 4,200 at every other, the tree's own 3/4 among them at 6,376 (`tunneli.md` §5, A6).
   *Decided:* the port writes the release's ramp, with the derivation as a permanent test.
2. **CRED — data.** The shipped `MAIN/DATA/CRED.EXE` draws captions no source file in the tree
   contains: `grep -rnia sketch upstream/SecondReality` returns exactly **one** hit and it is in the
   shipped data, the packed `CRED.EXE` itself (a `--include` search of the sources returns none in
   any letter case), and its screen 6's picture band has 8 of its 14 colours — 3,524 of 16,000 px —
   in **none** of that screen's 256 palette entries. Nine of the shipped build's 21 screens are off
   the tree's data at one hold sample each (`cred.md` §7, A10). *Decided:* the port keeps
   `CREDITS/MAIN.C`'s own 21 screens and captions.
   *(2026-10-29, under D49: reversed for CRED. The 21 screens are read from the unpacked image —
   each `screenin` call's picture segment and caption offset, in `main`'s own order — and all five
   keyframes match the reference byte for byte. A10, retired.)*
3. **ENDSCRL — a build's behaviour and its art.** The shipped scroller's text pitch is **25 rows a
   line** where `MAIN.C:6` says `FONAY 30`; its `[NN` control lines — **163** in the shipped text —
   advance the ring by the number in the marker and draw nothing, where the tree's renderer draws the
   two digits; its `fonaorder` names **six** characters the tree's 74 do not (the packed EXE's
   appended run is six bytes — `Z \x84 \x94 Y / &` — so its order is 80, not 81); its raster differs
   from `FONA.UH` in 53 px of the header band's 11,520; and its text has **no carriage returns**
   (`endscrl.md` §6.2, A11). *Decided:* the port stays the tree's, as for CRED — including the
   **pitch**, which is measured exactly and deliberately not adopted; step 3 below says why.
   *(2026-10-29, under D49: reversed for ENDSCRL. The packed EXE's image is now read, and every
   one of these is the release's own code or data there; the port reads and translates it, and its
   four keyframes match the reference to the pixel from the release's start. A11.)*

**The procedure.** Three steps, in this order, before anything is changed:

1. **Measure what the release does**, from the reference's own frames — a bit-exact match over the
   whole candidate range where the difference is arithmetic (TUNNELI), a read-off of the picture or
   the geometry where it is data (CRED, ENDSCRL), or a rule that *predicts* frames it was not fitted
   to (ENDSCRL's control lines predict every band of frames 10000 and 14000 from the tree's own text
   file, with no free parameter but the start). A fit to the test is not a measurement — Review Focus
   5 of this phase's plan, and the 441/450/1145 brackets that a bit-exact scan rejected.
2. **Check whether the tree can produce it at all.** A string no source file can emit, a pitch the
   loop bound forbids, a ramp the tree's own binary compiles as a different instruction: if the tree
   *cannot* produce the release's frame, no port of the tree will ever match it and the difference is
   the release's data, not a bug to chase. If the tree *can* produce it, the port has a bug and the
   divergence is not the explanation.
3. **Decide, and the first question is how much differs — not how well the measurement is made.**
   **Adopt the release's value only when the difference is one expression or constant that the
   tree's own algorithm can carry *and every other difference in that part is accounted for*** — in
   TUNNELI, one divisor (1,108 px against ≥4,200 for every other candidate), with the part's residual
   afterwards a few hundred pixels of *ink presence* (179/373/321/433 over the four aligned frames,
   A6) rather than any second value the tree got wrong. Then port the release's value with the
   derivation as a permanent test. **Register, and keep the tree's translation, when the part differs
   in more than that — data, art, text or behaviour the tree does not hold** — the
   alternative is inventing bytes no file in the tree supports, and bending one visible constant
   while the rest of the build still differs leaves the port fitted to a screen it cannot reproduce
   (CRED's captions and pictures; ENDSCRL's pitch, which is a single value measured exactly as
   TUNNELI's is, but its part also differs in the control-line rule, its font order, its raster and
   its carriage returns — A11's own closing paragraph records the non-adoption and the reason, so it
   does not read as a weaker measurement). A *single constant measured to the release's value in a
   part that otherwise matches* is TUNNELI's case and is adopted; the same measurement in a part that
   differs elsewhere is registered. Registering means an `APPROXIMATIONS.md` entry with the per-frame
   numbers, the evidence for the attribution, and **a lead**: name the packed EXE and the pack table
   (`MAIN/PACKFINA.INC`) where the release's own bytes are, so a later phase can turn "the shipped
   one differs" into "here is what it says". A divergence with no entry is the failure this procedure
   exists to prevent.

**What is not established, and must not be implied.** The direction. Nothing in the tree dates the
shipped build; "later revision" is an inference from an extra caption line reading as growth, and the
honest word is **different** (the CRED review's own correction). And which of the two copies of a part
EXE inside `SECOND.EXE` put a frame on screen is not resolvable from here — TUNNELI's `MAIN/DATA/
TUNNELI.EXE` is PKLITE-packed — so the measurement is of the *frames*, never of a file the port can
read.

*Cost if wrong:* two phases resolving the same divergence in opposite directions, or a phase "fixing"
a faithful port towards a build it cannot reconstruct — and losing, in the process, the tree's own
provenance for the algorithm.

**Corrected 2026-10-26 (`fix-user-glitches`) — PLZPART's cube extent and projection were not
release divergences.** A23 adopted a 200-row draw (`CUBE_DRAWN_ROWS = 200`) and a
`yy*213/zz + 100` projection as "single-value release divergences", on the evidence of the
200-line reference captures and a six-frame fit (`the_projection_fit_is_the_committed_release_bands`).
That was a mis-modelled display, and this entry's rule ("the tree and the release are not the same
build") was misapplied: **the release does the same as the tree.** `PLZPART/TWEAK.ASM:31-33`
(`mov ax,0209h` / `out dx,ax ; 134 y`) sets CRTC 0x09 = 2, so each row is shown on three scanlines
of the 400-line display — a 134-row screen, matching `PLZA.ASM:77`'s `cmp yy,134`, `VECT.OBJ`'s
142/66 and the 0x5500-byte (136-row) buffers. A 200-line frame is every other scanline, row
⌊2y/3⌋ of the image, which is what made the cube look 1.5x taller (213/100 = 142/66 × 1.5). The
200-row draw overflowed its page slot, which the user saw as cube pixels where the screen should
be black, and which A23 had filed as an unresolved port-side artifact. The release's registers were
then read live (`tools/crtc_probe.py 3 8500 30`: CRTC 0x09 = 2, 0x12 = 143, 0x13 = 80, display
starts 0x0028/0x5528/0xAA28 in the majority of 30 frames). The port now draws 134 rows at 142/66 and
composes row ⌊2y/3⌋; the 8500 keyframe goes from 26.161 % to 17.577 % and the port-only spill from
6,333 pixels to 304 (`cargo xtask rediff --only plzpart/cube --only plzpart/band`; A23 has the
table). The lesson for the procedure: before adopting a release/tree difference, check that the
reference's *geometry* (scanlines per row, line count, page size) is modelled — a fit that
recovers an exact rational factor (3/2) is a hint that the capture, not the build, differs. The
test-only `FitProbe` and the fit test are removed; `the_cube_is_a_134_row_screen` and
`a_134_row_draw_stays_inside_its_slot` replace them.

**Corrected 2026-10-26 (`fix-user-glitches`) — U2A's pre-gate palette is a race at the 30,000-cycle
reference speed, and the port follows the four speeds that agree.** A32 adopted the release's
"measured bright transform" `(3v + 5) & 63` for U2A's pre-gate hold from one capture, the reference
`u2a/frame_006500.rgb` (`11a66eb0...`, 30,000 cycles). D24's procedure was applied to **one capture
speed**. The same recipe at 15,000, 25,000, 35,000 and 60,000 cycles, at boot 6400 and 6600,
gives `0a68ffb9...` in all eight frames (`reference/parts/u2a-speed`, `reference-indices.md` §27): the
`U2ABG.UH` picture under the merged palette (`U2A.C:152`), the same bytes as the port's own post-gate
7000. At 30,000 cycles 6100-6900 are the neon and 7000 is `0a68ffb9...`. The user saw the neon as a
glitch (~13.5 s of yellow-green) and decided the port shows the normal landscape. The mechanism of the
race is **not traced** (the release's U2A is PKLITE-packed; a guest-memory dump found the gate loop
at linear 0x23a11 but not the source of the neon DAC); an 8.8 fixed-point copper fade running ~3x its
count would give ~3v and wrap at 64, unproven. The port uploads the merged palette in `init`; the
`pad_entry` palette is removed; A32 is rewritten as a registered divergence (the port is 64,000 px
off the 30,000-cycle 6500, 0 off the other eight frames). The lesson for the procedure: a release
behaviour measured at one emulated CPU speed is a hypothesis about timing until a second speed
agrees. `cargo xtask rediff` re-registers `u2a` 6500 and `fullrun/02-u2a-pam` 5961/6500
(60,160 / 64,000).

## D25 — Every number in a fidelity note is reproducible, and an unobservable claim is stated as one (2026-10-13)

Four review rounds across Phase 2's Tasks 4 and 5 were spent not on code but on **records claiming
more than their measurements support**, and the shapes recurred in Tasks 6–10. They are worth naming
because the next phase will write the same kind of note:

| shape | the instance it was found in |
|---|---|
| an **absolute** over a space nobody measured | "never a wrong colour", against 17 pixels the same round recorded |
| a **count that does not reproduce** | "21 candidate ramps" where the test evaluates 81 pairs / 70 distinct ramps |
| a **proof for something unobservable** | the reference's frames "proving" an off-screen out-of-bounds read (the reviewer's model counted 283,689 such reads in 813 of 901 ticks, all landing past the screen, invisible in both builds) |
| an **unknown dressed as a known** | nine frames attributed to the loader, both candidate causes ruleable out of the evidence already in the tree |
| a **number stated once, in prose** | the zeroing variant's 85/142/88 px beside the withdrawn 85/146/90 |

**The rule, for every later phase.** Every number and every absolute in a fidelity note is either
**reproducible by a command the note names** or **produced by a test in the same commit**.
"Never", "every", "all" and "only" are claims about a whole space and need a measurement over that
space, not over the samples at hand. Where the measurement cannot exist — an off-screen write, an
artefact no screen shows, a divergence whose other side is packed and unreadable — **the note says
so**, because that is the finding, and it is worth more than a proof-shaped sentence. A measured
constant is derived by **exhaustive match over its whole range, required to be unique**, never by a
fit to a sample (Phase 2's constants: 446, 451, 1147 and TUNNELI's 5, each unique; the mean-fits gave
441, 450 and 1145). And a constant is **not re-anchored to make a diff pass**: ENDSCRL is reported 82
frames early because that is where the chain puts it, and moving it would be a fit.

**And the record's own arithmetic is part of the claim.** A phase close re-runs its evidence; a number
in the record that no command at the committed state reproduces is a defect of the same class as a
wrong constant. Where the re-run disagrees with what a note or a ledger says, `PROGRESS.md` says which
was measured when, and the note is corrected in the same commit rather than left standing.

## D26 — The music clock, measured from the reference's own player state (2026-10-14)

Phase 3 has no audio reference — the emulator runs silent (D17) — so its oracle is a
**per-frame trace of the original's own player state**. `MAIN/STMIK.300`'s `PUBDEF` records
give the offsets of `MAIN/STMIK.H:125-131`'s position words, the segment is located in guest
memory by two 32-byte runs taken from the object's own `LEDATA`, and the words are read out
of one request per frame, keyed by the guest's own frame counter. The traces are filed
under `reference/music/` and every number below is printed by
`cargo xtask music {symbols,trace,analyse,probe}` — see `docs/fidelity/music-clock.md`, which
names the command for each.

**The measured clock.** The music starts at **frame 408**, measured and not hedged:
back-extrapolating the first row change by the tick counter's own value there (frame 427 with
`_np_zframe` 13, at the tick clock's measured 1.4622 frames a tick) puts the first tick at
frame **408.0**, and the same derivation gives 408.0 in the fresh session of the same recipe
(`second5b`) and 408.1 in each of the other two captures — a spread of **0.1 frames** over
four sessions. The `zframe` column's first non-zero frame is 409/410/411, one whole-frame
sample later: that is the same tick *seen*. (Which index the boot's mode change falls on — the
frame one trace loses and another keeps — is session-dependent, and the note records it as the
sampling it is.) A tick is
`2.5/tempo` seconds and a row `speed` ticks: the longest window, 296 row intervals over 3,455
frames of the hidden part, is predicted from the player's own speed and tempo words to
**−2.6 frames (0.08 %)**, and **every steady run's residual is under one row** — the two that
are not steady are the post-speed-change transients below (second5 −35.6, hiddenu −37.4, one
row 8.76 each). `_np_zframe` counts ticks: 0.6839 a frame (47.93 a second) at tempo 120 in
`second5`, 0.6854 (48.04) at the same tempo in `hiddenu`, and 0.7422 (52.01) at tempo 130 in
`second3` — two tempos, the tick rate each time. It is **reset mid-song** in
`second3` (1516 → 0 at frame 2452, with the same pattern still playing, so the writer is
`_dis_setmframe(0)` — which parts call at their own start — and not a song restart), so the
rate is measured over the longest run that never goes backwards.

**The machine's words are recorded twice, and four of them move** (`second5b`'s manifest is
the one that carries all four: `_np_mastermul`, `_np_mastervol`, `_zmixspeed`, `_zpollmix`).
STMIK's settings are not constant over a run: `_zmixspeed` is 20000 when the segment is
located and **40000** from the music's start
(`cargo xtask music probe --arg 5 --frames 408..1300 --at 0x5AC5`), `_zpollmix`
goes 0 → **1** — the demo switches to the *polled* mode, which is `MAIN/COPPER.ASM:466`
calling `zpollme` once a copper frame, and is exactly the shape the port's `poll()` per logic
tick has — and the volume words walk under `fademusic` (`MAIN/U2.ASM:333-346`). The manifest
records the reading at the locate and the reading at the end, per word
(`machine`/`machine_moved`), rather than one number the machine never held still.

**Three things the files do not say, which the traces do.** (1) The row rate follows the
player's `_np_speed` word, not the module's header byte, **and the word's initial value after
a restart is not the file's byte**: with `--arg 3` the reference plays MUSIC1 (its
`(ord, pat)` pairs match that file's order list) at speed **6** for frames 427…2472 where the
file's byte 0x31 is 3 (row interval 8.09 frames, where speed 3 would be 4.04), and then at
**3** from frame 2488 (interval 4.04) — the same session, both halves matching the trace's own
word. (2) A speed change is followed by a transient: after the module's speed-1 row, the next
speed-6 run advances 35 rows in 271 frames where the rule predicts 306.6. (3) `_np_zplus`
takes 1 and 2 in both modules, at order starts straddling each module's `254`; `_np_zinfo` is
**0 in every sample of every filed trace**, so that question is not resolved by them.

**D8's route 1 is performed and does not close it.** The symbols were read, the words traced
live, and `zinfo` stayed 0 over MUSIC0's orders 0 and 25…28 and 70…77 and MUSIC1's 0 and
42…48. `D8`'s escalation stands and **D8 gains the outcome and the substitute**: the port
computes `_np_zplus` by the rule in `music-clock.md` §6 — verified over every sample of every
filed trace (11,408 across the four) (`music analyse` re-derives it for each, walked from the tree), with the
crossings and the restart case covered and the `2 → 0`
advance named as extrapolation — and `dis_muscode` returns `_np_zinfo`'s measured 0, so a part
that waits on a `1`/`2`/`3`/`0xf0`/`0xff` cue waits for ever against the port exactly as it
does against the reference.

**The order list, from the counter's own sequence.** `254` is **stepped over**: MUSIC0's at
order 27 takes the counter from 26 to 28, and MUSIC1's at order 46 from 45 to 47, each with
both neighbours played. A `255` is **never played at all**: the shell's restarts
(`MAIN/U2.ASM:875-905`, orders 18, 25, 70) jump past the early ones and pre-empt the two at
orders 78/79, so the reference measures nothing about a `255` — Task 3 must say so rather
than read it off stock ST3 semantics.

**`reference-indices.md` §1.7's counter is not `_np_zframe`, and it is not a different
clock.** The word at physical `0x105B0E` is the STMIK segment's *higher view* base (0x1051F0)
+ **0x091E** — an unnamed word two bytes past `_zmixercount`, not `_np_zframe` at 0x08FB.
Measured in one command against `_np_zframe` itself (`cargo xtask music probe --arg 2
--cycles 30000 --frames 899..3000 --at 0x105B0E`), the two advance at the same rate over the
same 2,099 frames — **+0.7418 and +0.7419 per frame, 51.99 and 52.00 a second**, which is
tempo 130's `2.5/tempo`, the tempo the same output reports — and the 0x091E word reads 370 at
frame 900 where §1.7 read 371/373/374. What separates them is a **reset**: `_np_zframe` is
zeroed by `_dis_setmframe(0)` (`DIS/DISC.ASM:138-148`) and 0x091E is not, so the two differ
by exactly the ticks a reset cost (526 in that window). §1.7's address is right, its name is
not; the phase's clock is the published `_np_zframe`, which `dis_getmframe` hands the parts.

**The DOS extender maps the segment twice.** The needles are found at 0x0051F0 and
0x1051F0, exactly 1 MB apart, and the whole 29,607 bytes read through each are byte-identical
at the start and the end of a trace (the emulator's memory read goes through the page tables:
`dosbox-automation`'s `hardware/memory.cpp:199` → `cpu/paging.h:293`). `music trace` accepts
several views only when that equality is proved and refuses a second copy that differs; it
reads through the lowest.

**A trace is a new kind of reference, and the verifier sees it.** The manifest declares
`"kind": "music-trace"`; a manifest with no `kind` is a frame capture (the 24 filed before
this phase still check out), `reference verify` recomputes the trace's coverage from the
file's own frames rather than believing the manifest, and a `kind` it does not know is
**refused by name**, not walked past. A flipped byte of a trace fails the verifier (exit 1,
both digests named).

**What a trace does not hold.** Every trace misses a boundary run and a frame in the middle,
and says so (`per_frame: false`) with what the guest's own counter did around each: `second5`
and `second5b` lose 409 (the counter read 408 then 410, with no reading of 409 between) and
1220 (a read opened on the frame was still open when the guest moved on), `second3` loses
417, `hiddenu` loses 410 — and the count is the manifest's own line rather than this
sentence's (`reference/music/*/manifest.json`'s `coverage`, which `cargo xtask music analyse`
recomputes from the trace file). The manifest names the boundary run as such
(`coverage.before_sampling`: the locate costs frames), and `reference verify` recomputes the
whole coverage from the file **field by field** rather than believing it. Why a middle frame
is lost is a **hypothesis** (a video-mode change publishes no frame for a moment, D17) and is
recorded as one — the measurement is the counter's own readings, and they say only that the
frame was never current.

## D27 — The run's music: the entry point decides the module, and the anchor decides the clock (2026-10-15)

Before this the port had a player nothing played. `--block 5` now installs it, seeks it, feeds
it a sound card or a WAV file, and shows its position in the window's title — and the two
things that had to be decided rather than derived are **which module an entry point starts,
and when its clock starts**. Both are D13's business, which said "`sr-shell` must implement
the restarts as *entry-point* positions — one restart table keyed by which blocks are going to
run, not a single linear timeline"; this is that table and the clock beside it.

**The restart table** is `shell::MUSIC_RESTARTS`, one row an entry point, each carrying the
module (`restartmus`'s `ax`), the order (`bx`), the guard bits (`whattorun`) and the lines of
`MAIN/U2.ASM` it is read from: `SECOND 1`/no argument → MUSIC0 at order 0 (`:766-775`,
`:785-787`), every other block → MUSIC1 at order 0 (`:772-775`), `SECOND 3` → MUSIC1 at 42
(`:856-863`), `SECOND 4` → MUSIC0 at 18 (`:880-885`), `SECOND U` → MUSIC0 at 70 (`:889-894`),
`SECOND 5` → MUSIC0 at 25 (`:898-905`). The test beside it reads the upstream file **as
bytes** — it is not UTF-8, and a decoded read misplaces every line after the first non-ASCII
byte — and checks each row's instructions against its cited lines and its guard against the
block's own `partsmask`. A run takes the row of the entry point it starts at, which is its
plan's first part; a demo part no entry point starts at is played from the demo's own
beginning, and the test card — the port's own part and not the demo's — has no music.

**The clock starts at frame 408, and what a run does about that depends on where it begins.**
408 is Task 1's measurement (D26), and the trace's words around it say what it is: the
modules' own speed/tempo words change at 396 (the boot's initial `restartmus`) and at 408 (the
block's restart), with **zero ticks in between** — the twelve frames `_zsoundon` (`:779`) and
`_asmcopperinit` (`:781`) take before `_zpollmix = 1` (`:783`) hands the player to the
copper's per-frame `zpollme`. The tick clock starts when the polling does, and in every filed
session that is 408. A run anchored **at or before** it *waits*: the module is loaded and
seeked at the run's start, the adapter is installed so a part may ask it anything, and nothing
advances it until the frame the original's own first tick falls on. A run anchored **after**
it *pre-polls* the frames in between at its start, because the music's advance is
deterministic and does not depend on a frame being drawn (`SECOND 3` is that case —
MNTSCRL's first graphics frame is in (415, 420] — and no run the port can make today is,
because both blocks it can start at are anchored at 404).

*(Phase 6 update — superseded by D34/D35: the port now starts blocks 3 and 4 as well. Block 3
has a numbering of its own (MNTSCRL's start is a measured window, not an anchor), and block 4
is anchored at U2E's derived 418 and pre-polls the ten frames from 408. The clause above is
D27's record.)*

**What that buys, and what it costs.** A `--block 5` run's music is the trace's: 778 of
`second5`'s 894 frames from 404 agree exactly on the published `(order, row)` and the worst
displacement is 2 frames — the same shape the trace test measures for a bare player (771 of
887) — and the four ENDLOGO keyframes come out exactly `0:0`, `25:1`, `25:9`, `25:55`,
`26:53`. A four-frame error in the anchor fails the comparison loudly (the run publishes a row
the trace's window never reaches), which is the falsification the note records. The cost is
the boot's own twelve-frame transient: the port does not load MUSIC1 first and replace it, so
the *speed* and *tempo* words at frames 404…407 read MUSIC0's patched 6/120 where the trace
still reads MUSIC1's 3/130. The position words a part can read are identical either way, which
is why this is recorded as a divergence rather than modelled.

**And the seek clears `[0x2000]`.** Task 3 left it open: `_zplaysong`'s routine zeroes the
tick-in-row counter (`+0x56ec`, `movw $0x0,0x2000`) and `play_song` did not mirror it, which
was unobservable while every seek was made on a fresh player. The shell's restarts are seeks
made **mid-song** — `restartmus` reaches `_zplaysong` at `:775`, `:818`, `:863`, `:885`, `:894`
and `:905` with the player anywhere inside a row — and a stale counter would begin the new
order's row 0 on a later tick, so that row's cells would never be read. The object's byte says
what to do, so `play_song` mirrors it, and the test
(`a_seek_mid_song_clears_the_tick_counter_so_the_new_rows_cells_are_read`) fails without it.

**The device is a drain, and it is the binary's.** SDL2 audio opens only in window mode, at the
mixer's own specification with `allowed_changes` at zero — a card that cannot give that rate is
not opened rather than resampled into one that can, and the run prints a line and plays silent
— and its callback copies what the ring holds and fills the rest with zeroes: **silence, never
a stall, never a repeat, never a resample**, because the music's position is the tick's and a
device may not pull it. `--no-audio` asks for the same state, `--audio-dump` writes the tick's
samples as a 16-bit WAV instead (the ring has one consumer, so a dump is silent to watch) at
20000 frames a second stereo, and `--headless` does all of this with no device at all — which
is what makes a dump of a music-gated part the same run a window plays, and why the frames are
byte-identical with and without `--no-audio`.

**The fade is `@@partsdone`'s**, which is what the port's *hold* is: `fademusic`
(`MAIN/U2.ASM:333-346`) walks the module's own global-volume byte (`ds:[30h]`, 0x40 in both
files) down by a step a frame — the loop's `int 0fch` is `dis_waitb()` — so the shell calls one
step a tick, with the demo's own `mov cx,1`, from the moment there is nothing left to play.
Each row of this section has a test; §12 of `docs/fidelity/music.md` is the note.

## D28 — A part's music read is one tick behind the copper's poll, and the one frame that is an alignment (2026-10-15)

Task 8 ran ENDLOGO's gate, `END/END.C:107-111`, against the run's player — the first part in the
port that waits on music — and the cue's answer and A7's measured 567 came out **one frame apart**.
Both halves of that gap are decisions, and both are now written where the code can be read with
them: `bins/secondreality/src/parts/endlogo.rs` carries the two constants and
`docs/fidelity/endlogo.md` §5 the account.

**A part that reads the music at the tick's boundary reads the *previous* tick's words.** The
source's ENDLOGO check is the instruction after `dis_waitb()`, so it stands at a frame boundary:
`waitb_1` (`DIS/DISINT.ASM:161-181`) returns when `cs:copperframecount` changes, and `copper_end`
(`MAIN/COPPER.ASM:283-320`) increments it (`:320`) at the vertical retrace. The music's poll is a
fifth of a frame later — `call _zpollme` (`:466`) inside `copper_doit`, which the copper's list
runs at 20 of the frame's 100 units (`copperlst0`, `:41-43`) — so the check reads the words the
*previous* frame's poll published. The port's engine polls between copper slot 0 and the first band
(`crates/sr-engine/src/engine.rs:206-219`), i.e. before the part's code in the same tick, so its
`musplus()` at tick `T` is the words tick `T`'s poll published: one frame newer than the source's
check at the same instant. **The part keeps the reading for a tick and tests that**, and the tick
it breaks on is the tick the source's break happens on. This is not ENDLOGO's rule: it is the
engine's poll placement showing through, and any later part that reads the music as the first act
of its frame (GLENZ's openers, PLZPART's waits) has the same one-tick relationship.

**The one frame that stayed is a disclosed alignment, not a derivation.** On the reference's boot
the port's own player publishes `26:49` from index **1144**, and with the placement above the gate
would break on **1145** — while the reference's *frames*, derived bit-exactly from 500 and 1180,
put the fadeout's first step at **1147**, i.e. the break at 1146. The port holds one frame past its
own break (`GATE_ALIGNMENT_TICKS`). **Which measurement moved is nameable, and it is the session's,
not the port's:** the port's player is `second5`'s — its direct `_np_zframe` matches the trace's own
column once the sample instant's one-index shift is taken out (753 of 891 frames exact, never more
than one tick apart: `src/shell.rs`'s
`the_runs_music_is_where_the_trace_holds_it_at_every_frame_the_trace_has` asserts it frame by frame,
`ZFRAME_SLOP`), and its published positions are the trace's samples one index on — 100 of the window's
130 compared position changes one frame earlier, 22 exact, 8 two earlier (the window holds 131, and
the one the comparison skips is the window's **first** sample, which has no earlier frame for it to be
a change from — `src/shell.rs`'s `if *frame > start`, whose own comment says so; the last change, at
1295, *is* paired; the same test prints the distribution and fails if any
change is more than two frames out), against the ~20 % the poll geometry
predicts for an unshifted clock, which is what the sample instant (the frame's *start*, D17's own
reading of the emulator) makes of them — and
`second5`'s published row 49 begins one frame earlier, relative to the frame indices, than the
session the reference's four keyframes were captured in. That is D17's per-session counter
alignment, and the cost is stated where the constant is: the port disagrees with `second5`'s own
session by one frame at this crossing, inside the ±3 frames A12 measures between the two filed
sessions of one recipe. **Nothing in the music moved for it** — no anchor, no rate, no position —
and nothing in the part *could* move it: the part carries no hold length, which is the point of the
task — the one lever left is `GATE_ALIGNMENT_TICKS`, the disclosed frame above. A run whose music starts elsewhere has no reference frame to align to, and the constant is a
fact about this boot and `second5`.

## D29 — Editing a record: guard the substitution, name the grep, and check the tree rather than the round's report (2026-10-15)

Phase 3's most-repeated defect was not a wrong value but a **change that was never made**: five
times a fix round's report described an edit as landed while the tree still held the old text, and
each time only a re-review caught it, because the artifact was the only witness. The fifth
instance's author found the mechanism, and it is mechanical rather than careless: the edit was an
**unchecked `str.replace` whose target did not match**, so the substitution silently no-opped while
the summary beside it was written from intent. Three rules follow, and they bind every later phase.

**1. A text substitution is guarded at the point of edit and verified afterwards by search.** The
tool that makes the edit fails when its target is absent or not unique; the round then re-reads the
result rather than the intention. Guarding the substitution does not guard the sentence around it —
the same round's last defect was a dangling fragment where an edit replaced the first half of a
two-line sentence and orphaned the continuation — so the surrounding text is checked too.

**2. A claim that rests on a search says which search.** This shell's `grep` is a **wrapper**, not
the binary: it runs `ugrep` with `--ignore-files` and `-I`, so a recursive search silently skips
every path an ignore file names (`out/`, and any untracked-but-ignored file) and every file it reads
as binary. Measured: `grep -rl needle .` finds **0** where `command grep -rl needle .` finds **1**,
for a file under the git-ignored `out/`. The wrapper is why two "no caller exists" / "no surviving
copy" claims in this phase were false — the second being the discoverable one that stopped a round.
The upstream tree adds the other half: its `.ASM` sources are **not UTF-8**
(`upstream/SecondReality/MAIN/U2.ASM` is ISO-8859 with CRLF), so the wrapper's `-I` skips them
outright and a search of them needs `command grep -a`. The port's evidence therefore names the tool
it used, and the phase's own reviews redo a search with `command grep` when a negative is
load-bearing.

**3. A review reads the tree, and a correction is swept rather than pointed.** Three rounds in this
phase fixed a corrected claim at the site a reviewer named and left copies standing elsewhere — in
a module doc, in a test's *name*, in a plan, in the note 130 lines below — so the dispatch that
follows a correction asks for a `grep` over the retracted vocabulary rather than a list of line
numbers. This is D25's rule applied to the *act* of correcting: the measurement is reproducible by a
command, and the correction is findable by one.

**What this costs if it is not followed**, measured in this phase: nine reviews across seven tasks,
the majority of their findings in records rather than in code, and one task's fix round spent
entirely on the copies of a claim the task had already corrected once.

## D30 — Phase 4's decisions: a second music clock for block 1, anchors only where a first frame is measured, and no inherited cue alignment (2026-10-17)

D27 gave a run one music clock (408) and one wait/pre-poll rule, and D28 gave a part that reads
the music a one-tick relationship to the copper's poll. Phase 4 ported the first parts that
**wait** on the music, and three rules the parts follow were decided rather than measured. Each is
a decision about the port's model, not a divergence from the original; the divergences the phase
did find are **A17–A21**, not this entry.

**1. Block 1's clock starts at 400, not 408, and `MusicStart` takes the recipe.** D27's 408 is the
clock measured for the restart recipes — the filed `second3`, `second5`, `second5b` and `hiddenu`
sessions. `SECOND 1` is a different recipe and never takes a block restart: `MAIN/U2.ASM:766-775`
loads MUSIC0 with `bx = -1`, `:786`'s unconditional `_zplaysong(0)` starts it, and nothing restarts
before ALKU/PAM. The trace of that recipe, `reference/music/second1` (frames 148–5999, Phase 4's
Task 1), fits its first row change (frame 420, `_np_zframe` 14) to a start at **399.6** — the direct
`zframe` reading is 405, 5.4 frames later, the sample instant of D17/A12 — and `second1b` confirms
the row at 7710. The shell now carries `BLOCK_ONE_MUSIC_START_FRAME = 400`, and
`music_start_frame_for(part)` returns it for the three block-1 parts (`alku`, `u2a`, `pam`) and 408
for everything else; `MusicStart::for_part` therefore pre-polls four frames for a block-1 run that
draws from 404 and 7,310 for a PAM run anchored at 7710. The rule is D27's, not a new one: a run
beginning after its recipe's first tick catches up deterministically. The new fact is that there are
**two recipes**, and a run must use the one its entry point takes; a block-1 run under 408 would
wait four frames instead of pre-polling four — four ticks of player position at its first drawn
frame. The check is the block-1 trace comparison
(`a_block_one_run_starts_under_the_initial_play_song_and_matches_second1`: all 638 position
changes visited, the port's change-frame distribution
`{-2: 1, -1: 1, 0: 3, 1: 1, 2: 217, 3: 285, 4: 118, 5: 10, 6: 1, 8: 1}`) and PAM's 7,310-poll
test (the trace's `(13, 1)` at 7710, `the_block_one_pre_roll_puts_the_player_at_the_traces_7710_position`).

**2. A part gets an anchor only where the reference names its first frame; a window stays a
window.** `boot_anchor` returns `Some(404)` for ALKU/BEGLOGO/ENDLOGO (the boot's first graphics
frame, five of five sessions, `reference-indices.md` §1.12) and `Some(7710)` for PAM (the flat
wash, byte-identical in three sessions, §6.1/§6.4). It returns `None` for the other four Phase-4
parts, because the reference bounds their starts without pinning them: MNTSCRL's tick 0 at 419 is a
fade-curve mapping (its fade's frame means, §11.1), RAYSCRL's first drawn frame is in
(12900, 13000], MINVBALL's in (10300, 10400], and DDSTARS's earliest evidence is a probe at 415
with the screen already up and no filed start frame (§22). Those runs number their own frames from
zero, and their manifests say `index_origin: "part"`, `anchor: null`. The rule: **a mapping may be
exact at every reference keyframe without being a claim about the first frame** — each note states
its mapping (`boot = 419 + tick` for MNTSCRL, `12962 + tick` for RAYSCRL, `10368 + tick` for
MINVBALL, `414 + tick` for DDSTARS) and each integration test asserts it — and a guessed anchor
would make every index of the run wrong in a way the keyframes could not catch. Consequence for the
shell: block-3 and block-U dumps are not reference-comparable by index; the part tests compare
bytes at the mapped tick.

**3. No Phase-4 part inherits `GATE_ALIGNMENT_TICKS` or D28's held reading; each cue's timing is
its own measured value.** Phase 3 recorded that constant as a fact about its boot and `second5`,
and D28's held reading as the alignment ENDLOGO's frame comparison needed. The six parts read the
engine's tick-own poll (D28's one-tick relationship) and their timing is carried by their own
measurements: MNTSCRL runs the release's 71-tick wait (**A17**), RAYSCRL the release's seven
confirming reads (**A18**), and MINVBALL explicitly declines the held reading because the only
readings the difference could change are inside its fade (the trace's `-3` at 10,368–10,379) and
the next is boot 12,950, past the frame cap at 12,947 — so the one-tick difference changes no
picture in its run (`docs/fidelity/minvball.md` §5). ALKU's and PAM's gates sit far from their
keyframes; their traces and the four keyframes are the check. What the reference's checks actually
read is D28's one-tick relationship, and where the release's timing differs, A17/A18 register it.

## D31 — Phase 5's decisions: a mid-chain seam on the part context, PLZPART's `.INC` data filed verbatim, and a chain that hands over with no loader gap (2026-10-18)

Phase 5 ported the six mid-tier parts and, with them, the seam Phase 4 deferred, a part whose data is
its original's assembler text, and the first block-3 chain that runs more than one part. Three rules
the parts and the shell follow were decided rather than measured. Each is a decision about the port's
model, not a divergence from the original; the phase's release-versus-tree divergences are
**A22–A27**, not this entry.

**1. The mid-chain seam is a property of the install, not of the part: `PartCtx::music_fresh`.**
Phase 4's final review found the assumption its four parts were built on — MINVBALL's **256**-poll
entry pre-roll, RAYSCRL's **268**, MNTSCRL's **11**, and PAM's open-gate `frame_count = 2` — true only
for a run that begins at the part, and false for a part reached mid-chain with the player already
running. The seam is a flag on the context, not a per-part constant: `Engine::set_music` arms it,
the next `Engine::start` consumes it (clearing it **before** `init`, so a failed start cannot leak
freshness), the init context of that one part reports `music_fresh() == true`, and every later
`start`, every band context and every copper context reports `false`. A second `set_music` re-arms
it. The consumers are the four Phase-4 parts above and the five Phase-5 parts with entry pre-rolls —
`lnszoom` **490**, `plzpart` **8**, `techno` **253** (+ its two extra polls), `coman` **527**,
`jplogo` **483** — each of which polls only when fresh and takes the source's own music gate
mid-chain; PANICEND reads no music at all. The check is the engine test
`the_music_is_fresh_only_for_the_first_part_after_set_music` (true, then false, then true again after
a second install), the four Phase-4 mid-chain cases, and the block-3 chain with the real player:
PLZPART's last frame and MINVBALL's first publish **75:60** one poll apart and the trace's **75:61**
arrives three ticks in; RAYSCRL publishes **86:62** inside its gate and opens on **88:0**. A
256/268-poll jump mid-chain is orders ahead and cannot publish those positions.

**2. PLZPART's data is its original's `.INC` text, filed verbatim under `assets/PLZPART/`.** The part
has no media file; its tables are seven assembler fragments (`PSINI`, `LSINI4`, `LSINI16`, `SINIT`,
`PTAU`, `SPLINE`, `RATA`, 84,992 / 42,496 / 42,496 / 10,616 / 525 / 9,493 / 1,926 bytes). The rule
"read assets in their original formats" makes them assets: they are copied **byte for byte** with
their provenance into `assets/PLZPART/`, hashed by `cargo xtask assets verify --tree`, and parsed at
run time by a part-local `db`/`dw` reader — not converted into Rust constants at build time.
`TILE.INC` and `TILEPAL.INC` sit in the tree and are **not** copied: the shipped `PLZPART.PRJ` names
neither. The assembler's placeholders `0xC100`/`0xC200`/`0xC300`/`0xC400` are not the table bases —
`PLZPART.MAP:368-369,387,391` gives `_psini 027D:0000`, `_lsini4 027D:4000`, `_lsini16 027D:8000`,
`_plzline 027D:C000`, and the port builds the flat `0xC000`-byte segment the map describes. The
parsed shapes are asserted (`16,384` `db`; `8,192`+`8,192` `dw`; `2,575` bytes; `129` values;
`1,024` and `1,088` words — the last with `REPT 100` expanded). The consequence is a deliberate one:
the tree's text is the data even where the release's frames disagree with it, and the disagreement is
A22/A23's, measured against the frames rather than patched into the tables.

**3. The block-3 chain hands over with no loader gap, and the chain test says so rather than
pretending one index fits every part.** The original's parts are separate EXEs the loader brings in;
the port's parts hand over in one tick, with the surface and the player continuous. Chain tick `t`
names boot `419 + t` (MNTSCRL's tick 0), but at a part boundary the reference's boot counter carries
the loader's frames while the port's does not, so one affine translation cannot fit every part. The
chain test therefore asks for the keyframe at **the first chain tick whose trace position is the
reference's at `index + 419`**, and three of the eight keyframe indices (seven parts, PLZPART
twice) move by one to three ticks (PLZPART 8,082 for 8,081; MINVBALL 10,482 for 10,481; JPLOGO
16,884 for 16,881). What the test
asserts at those indices is (part, geometry, manifest `.idx`/`.rgb`, and the trace's published
`(order, row)`) — not a frame-by-frame pixel equality across a seam. A future full-run comparison
against boot indices will need per-seam offsets, the same way the parts' own tests state mappings
(D30), and the loader's own duration is what a Phase 9 full run must model. The stance is measured
by the positions themselves: **75:60 → 75:60** at the PLZPART→MINVBALL handoff and the trace's
**75:61** three ticks in; **86:62** at 12,537 and **88:0** at 12,546 for RAYSCRL. Block 3 still
stops at U2E, so the `MAIN/U2.ASM:880-885` mid-run MUSIC0@18 restart is not reached, and block 2's
stop at GLENZ keeps TECHNO and PANICEND unreachable through a block until Phase 7.

*(Phase 6 update — superseded by D34/D35: the mid-run `MUSIC0@18` restart is implemented
(`MUSIC_RESTART_AT`, `MusicStart::restart`, waiting 4) and `--block 3` runs through it into the
scrollers; block 2 still stops at GLENZ. **Phase 7 update:** block 2 now runs through GLENZ and
on to ENDSCRL, and every block's plan is fully ported. The paragraph above is D31's record.)*

*(Phase 9 Task 6 — the loader question, decided from measurement: **not modelled**.* The full
`SECOND 1` run against the port's `--block 1` gives the seam table of `reference-indices.md` §26.8:
each part's first frame in both, by §26.3's definitions, pinned by
`cargo test --release -p secondreality --test fullrun`. A `LOADER_TICKS` held between
parts would be adopted only if **one** constant explained every seam the music does not decide and
left every filed comparison unchanged (D24). The changes across the seams take both signs — the
port is already later at ALKU→U2A (+47), MNTSCRL→LNS&ZOOM (+19) and RAYSCRL→3DSINFLD (+11), which
no delay at the handover can produce — and the two seams the music pins on both sides need 6 frames
(the cue wait's exit to MNTSCRL) and 59–68 (the U2E reload). The handover stays the one tick this
entry's §3 describes; the measured seams are `APPROXIMATIONS.md` **A53**. The same run found the
post-GLENZ timeline 247–248 frames late, which is A38's lag and whose probable cause — `_zplaysong`
keeps the channel volumes, so MUSIC1's order 0 breaks at row 3 — is recorded there; the port's fix is
Task 6b.)* *(Phase 9 Task 6b: the fix is D45. On the corrected run the decision is re-checked under the
same one-constant rule and stands: the changes still take both signs — every one of the eighteen
seams, positive at eight (ALKU→U2A +47, BEGLOGO→GLENZ +42, TUNNELI→TECHNO +9, PANICEND→MNTSCRL +8, MNTSCRL→LNS&ZOOM +18, MINVBALL→RAYSCRL +9, RAYSCRL→3DSINFLD +10, U2E→ENDLOGO +34) and negative at ten (U2A→PAM −50, PAM→BEGLOGO −34, GLENZ→TUNNELI −12, TECHNO→PANICEND −14, LNS&ZOOM→PLZPART −13, PLZPART→MINVBALL −4, 3DSINFLD→JPLOGO −24, JPLOGO→U2E −37, ENDLOGO→CRED −8, CRED→ENDSCRL −69) — and the two music-pinned seams still
need 6 and 59–68 frames; the figures are A53's and §26.8's.)*

**Closed 2026-10-25 (Phase 9 Task 10).** The loader question is decided — **not modelled** — and its
record is `APPROXIMATIONS.md` A53, whose seam table, filed-frame counts and three carried items
(U2A's opening, PANICEND's first frame, the loader's frames) are re-measured at Task 10's commit by
`cargo test --release -p secondreality --test fullrun -- --nocapture` and `cargo xtask rediff
--only fullrun`. §3's one-tick handover stands as decided.

## D32 — The vector renderer's home: a module in the binary, the packed formats as data in `sr-assets` (2026-10-18)

Decided: the VISU engine lives in the module directory `bins/secondreality/src/parts/visu/` —
`mod.rs` (the skeleton that declares and re-exports the other two, and later the scene
driver), `math.rs` and `draw.rs` — as the plan's Tasks 3 and 5 file lists have it. The packed
scene/object readers live in `crates/sr-assets/src/visu.rs` as plain data. No new crate.

**Why.** Plan 02 states the stance this follows: "No new crate: the shell is a module in the
binary, the parts are modules beside it" (`docs/plans/02-shell.md:20`). Code shared by two parts
has a precedent one directory over: the POS/WAT column decoder is `parts/poslist.rs`
(`bins/secondreality/src/parts/poslist.rs:1-20`, declared at
`bins/secondreality/src/parts.rs:27`), and only the byte layout is shared while the two
blitters stay in their own parts — the same split the two vector parts need (they share the
maths, pipeline and driver; each keeps its own scene driver and gates). D3
(`docs/decisions.md:28`) keeps the software VGA model and the per-part modules in the binary,
and D16 (`docs/decisions.md:752-786`) keeps presentation out of `sr-engine`; demo-specific 3D
maths, tables and hacks are above the engine, not engine material. The readers are a different
kind of code — offsets and lengths checked against a file, no demo semantics — and `sr-assets`
already owns the format readers the binary loads through (`load_path`, the image and font
decoders); Task 2's tests pin every shipped `.00M/.0AA/.0AB/.00N` there.

**Cost if wrong.** Moving one module behind a crate boundary: mechanical, with the `mod`/`use`
paths and the two parts' `use` lines the only consumers. The readers would move if a second
binary ever needed them; nothing in this phase does, and a crate added speculatively would
carry the licence/manifest/test-layout work for one consumer.

**Phase 6 outcome.** The decision held as written. The renderer is
`parts/visu/mod.rs` (the driver and ring), `math.rs` and `draw.rs`; the packed readers are
`crates/sr-assets/src/visu.rs`; `parts/u2a.rs` and `parts/u2e.rs` consume both and no new crate
exists. The readers parse all 45 shipped objects and both scene streams — `U2A.0AB` 522 frames /
1,272 commands, `U2E.0AB` 1,802 frames / 4,047 commands
(`cargo test -p sr-assets --test visu every_shipped_object_resolves_its_lists_and_matches_its_own_vertex_count`,
and `cargo xtask assets verify --tree --decode`'s census: 158 assets, 51 Visu scene files, 44
headerless, 1 refused). The rest of the phase's code is the two parts, their fidelity notes and
the shell rows; nothing moved between the layers.

## D33 — The two parts' tick models, and U2E's first drawn frame (2026-10-18)

Decided: **U2A** advances one animation-stream frame per logic tick and flips one page per
tick (`VISU/C/U2A.C:105-130`'s `copper2`, one `vid_setswitch(-1, clr)` a retrace; the block at
`:330-344` advances `currframe` by `repeat` and parses one stream frame per pass); its drawing
begins at gate 1, `order>10 && row>46` (`:238`), whose first satisfying frame is measured at
**6988** — `second1c`, order 11 row 47, pattern 8, `_np_zframe` 4513. Its gate 2,
`order>11 && row>54` (`:276`), first holds at **7618** — `second1b`, order 12 row 55, pattern
11, `_np_zframe` 4945. That is a **music-only value**: U2A's animation stream ends first
(~7510), so the reference reaches gate 2 in the trace but the part normally retires at the
stream's END before it.

**U2E** advances one animation-stream frame per **two** logic ticks (`currframe += repeat*2`,
`VISU/C/U2E.C:435-444`) and skips its first draw (`firstframe` at `:105`, tested at `:386`);
its drawing begins at gate `order>18` (`:365`), whose first satisfying frame is measured at
**699** in the block-4 boot — `second4`, order 19 row 0, pattern 51. Its intro is **100
`dis_waitb` waits**: 33 fade-in steps, 16 waits, one wait each side of the `fadeset`/CRTC 9
writes, 16 waits, 33 fade-out steps (`VISU/C/U2E.C:285-335`), each wait one frame
(`DIS/DISC.ASM:42-47`'s `int 0fch` fn 1 is `copperwaitborder`, a vertical-retrace wait,
`MAIN/COPPER.ASM:72-81`).

**U2E's run anchor is the intro's start, not the first visible frame.** The measured first
*visible* frame is **467** (`--arg 4` boot): the loader's flat black `ea0787f6…` holds
through 466, and 467 is the first frame with a second colour — a white band at columns 68–252
on black, 185 white and 135 black pixels in every row (37,000 white, 27,000 black overall),
the `fadeset`'s pattern. The part's ticks begin earlier: `U2E.C:285-335` runs **33 fade-in
waits then 16 further waits** before `fadeset` writes the displayed page, and that write
precedes the fadeset's own `dis_waitb()`; the band is therefore drawn after **49** pre-visible
waits, and the run anchor is the derived **467 − 49 = 418**. The alternative **417** counts
the fadeset's own wait before the write instead of after it; the two differ by that
off-by-one. `boot_anchor("u2e")` takes the derived intro anchor, not 467, and Task 7 owns a
tick-map test that calibrates the exact value so the fadeset lands at 467.

**Evidence for 467, the first visible frame: two independent sessions.** The probe
`cargo xtask reference capture --arg 4 --boot autoexec --keys "KBD_left KBD_enter" --cycles
30000 --frames 456,457,458,459,460,461,462,463,464,465,466,467,468,469,470,471,472,473` (run
twice, into `out/u2e-dense-a` and `out/u2e-dense-b`; `reference-indices.md` §18.7 carries the
full command) gives 456…466 black (`ea0787f6…`) and
467…473 the band in both sessions, identical hashes frame for frame. The plan's specified
10-frame grid (`--frames 404,410,420,430,440,450,460,470,480,490,500`) cannot start at 404 on
this host: the capture's demo-start detection returns at frame 419–420 (the emulator has run
past 404 before the first grab; the same command refused twice with "reached frame 419/420
before frame 404"). Run from 420, the grid agrees with the dense scan — 460 is still
`ea0787f6…` and 470 is `6eb4e4cd…` — and both grid sessions are identical at every sampled
index. The manifest's `arguments` is `["4"]` in every probe. The measurement is of the first
visible frame; the anchor above is derived from the intro's structure, not a second
measurement.

**Cost if wrong.** The anchor sets the run's numbering and the intro's phase, not the
keyframes' reachability: U2E's gate polls the music and leaves at order 19 frame 699 whichever
anchor the run uses, so the four filed keyframes (1000/1600/2600/3600) pass even with the
wrong anchor once the gate is reached. What a wrong anchor changes is the intro: with the run
starting at 467 (the visible frame rather than the intro's start), the fadeset would be drawn
at ~517, the reference's 500 would still be the invisible fade-in (black), and its 530 would
be the band rather than the post-fade hold. A wrong **cadence** is a different failure and a
larger one: one stream frame per tick rather than one per two ticks advances the stream by
hundreds of frames by index 3600, so all four moving keyframes are wrong, not by ±1. A
one-frame **phase** offset is the only case that is a uniform ±1. The intro tick map — not
the keyframe comparison — is what pins the anchor.

**Phase 6 outcome.** Both tick models shipped as decided and both parts' comparisons are
recorded. U2E's anchor is the derived **418**, not 417: the unit test
`the_fadeset_lands_on_tick_49_and_paints_the_measured_band` drives 49 black ticks and finds
the band on tick 49 (the candidate anchor 417 would put the band at boot **466**), and
`u2e_runs_are_numbered_from_their_derived_intro_anchor` pins the shell constant. With it, the intro compares at **0 differing** at nine indices — 460,
467, 470, 480, 490, 500, 530, 590 and 660 — against the dense/start/sweep2 sessions
(`docs/fidelity/u2e.md` §3). The gate measurements are session-specific: U2A's **6988** is
`second1c`'s first order-11 row-47 frame; the port's block-1 clock matches **`second1`**
(9:51 / zframe 3771 at boot 5900 against `second1c`'s 9:50 / 3768) and opens at boot 6986,
and the keyframe capture is a third session. U2E's gate is 699 in `second4`; the port's clock
reaches order 19 at 698.

**What "verified" means for these two parts (D24/D17).** No value was fitted to a keyframe.
U2A's 6500 and 7640 compare at **0**; 7300 is registered as **A28** — 426 of 64,000 pixels at
the reference's own stream state 294, the release-versus-tree Gouraud difference, with the
silhouette and the flat object exact — and **A29** — 1098 of 64,000 at the port's boot index,
the music clock's session phase amplified by the part's page holds. U2E's four moving
keyframes are registered as **A30** (189 / 7,118 / 231 / 1,177 at the reference's own state,
all on Gouraud faces) and **A31** (11,006 / 11,805 / 17,278 / 16,234 at K: the port displays
stream state `(t − 698)/2`, the filed session `(t − 702)/2`, a four-tick intercept). Both are
the same D24/D17 classes as U2A's, are measured by committed tests/diagnostics, and are
reproduced by the commands the register entries name. The intro and the held keyframes are
what pin the models; the residuals are the release's Gouraud output and the sessions' phase.

## D34 — The mid-run `MUSIC0@18` restart: four engine ticks of waiting, verified by the two sessions' shift (2026-10-18)

Decided: at the U2E boundary the shell installs a fresh MUSIC0 seeked to order 18
(`MAIN/U2.ASM:880-885`), the row for block '4'; its guard `whattorun & 8` is implied by U2E's
presence in any block plan (D31's block shapes). The new player waits
`MUSIC_START_FRAME - FIRST_GRAPHICS_FRAME = 4` engine ticks before advancing — the restart
recipes' measured 404→408 relation — recorded as `RunMusic.start.waiting == 4`.

**Evidence: two boots, one session.** `second4` is a `SECOND 4` boot, whose first show part is
U2E: the shell reaches the `MUSIC0@18` restart before any other part, at the boot's first
graphics frame, and the whole traced window is that one restarted player's session. Its own
columns derive the restarted clock's first tick at **408.2** (the restart recipes' 408). It
publishes order 19 at frame 699 and the plan's four frames as 19:34 at 1000, 20:38 at 1600,
22:25 at 2600 and 24:11 at 3600. `second3e` is a `SECOND 3` boot, restarted at the U2E
boundary after JPLOGO: it holds the *same* session shifted by **17,441–17,442 frames** (the
frame sampling; 17,442 at the 18000/18100 rows and their `second4` partners 558/658, and at
five of the seven order boundaries) — every order boundary (699↔18140, 1259↔18701,
1820↔19261, 2380↔19822, 2940↔20382, 3501↔20943, 4131↔21573) and the 18000/18100 rows, 18:16
and 18:27, which are `second4`'s 558 and 658. The mid-run restart's clock therefore starts at
408.2 + 17,442 = **17,850.2** (±1 frame), and the boundary's first graphics frame is four
frames earlier at **17,846.2**. At the chain's
mapping (chain tick 0 = boot 419, D31 §3) that is chain **17,427**, and the restarted music's
first tick lands on chain 17,431 = boot 17,850. The trace shows the modelled 4 and no other
offset.

**Recorded with the decision: the restart does not zero `_np_zframe`.** `second3e` reads
**9,099** at 18000, which is the count carried from PLZPART's `_dis_setmframe(0)` at 5,723.
The measured trail from the previous window: `second3d` reads **8,961 at 17,799**; then 47
frames at MUSIC1's tempo-130 rate (~35 ticks), the **4 wait frames** of the `restartmus`/
`_zplaysong` boundary (0 ticks — the player is not polled across them), and 150 frames at
MUSIC0's tempo-120 rate (~103 ticks) give **8,961 + 35 + 0 + 103 = 9,099**, with no reset in
between. So `restartmus`'s
`_zplaysong(18)` does not reset the published tick counter; the port's fresh player starts its
own counter at 0, which differs in that word only. Neither vector part reads it (both gates use
`int 0fch` fn 6's order/row, U2A.C:238, U2E.C:365) and the trace test compares position
and `zplus`, not `zframe`.

**Cost if wrong.** A `waiting` of 0 or 8 moves the whole restarted session by four frames;
U2E's four keyframes then sit one 2-tick stream step out and the block-3 chain test's position
assertions at the mapping fail. The 17,441–17,442-frame shift is independent evidence — it
comes from two different boots' order boundaries, not from the gate measurement it verifies.

**Phase 6 outcome.** The restart shipped as decided (`MUSIC_RESTART_AT`,
`MusicStart::restart` with `waiting = 4`), and the two sessions' shift did verify it. The
block-3 chain measures the port's U2E boundary at chain **17,418** (JPLOGO's last frame is
17,417), **nine ticks before the derived 17,427**. The nine frames are not the restart's
waiting: they are the port's **gapless part handover** accumulating over D31 §3's seams, and
the restart's own behaviour is as derived — the new player waits four ticks, its first row
advance lands at chain 17,431, the ten-deep publish ring primes at `0:0` and first publishes
`18:1` at 17,440, and at the mapping-derived indices the positions agree with `second3e`
within one MUSIC0 row (18:17 against 18:16 at 17,581; 18:19 against 18:18 at 17,600; 19:1
against 19:0 at 17,721). The seam is recorded in `docs/fidelity/u2e.md` §5 and asserted by
`bins/secondreality/tests/shell.rs`'s chain test; nothing was shifted. The manifest records
the last installed player (`MUSIC0@18`, `start_frame` 17,418), which is the value the chain
test pins, and the restart's `_np_zframe` difference stays as recorded and unread. *(Phase 9
Task 6b, D45: the port's restart now carries `_np_zframe` across, as this paragraph measured the
original does.)*

## D35 — The shell's restart seams: an exit drain the part opts into, and a sound card that follows a new player (2026-10-19)

Decided: the shell's mid-run restart needs two contracts the part API did not have, and both
are the narrowest form that keeps existing parts unchanged.

**1. `Part::drains_after_exit() -> bool`, default `false`; U2E overrides it to `true`.**
`Sequence::tick` and `Runner::stopped` keep a draining part live after the exit flag is set
and retire it only when it reports `Finished` (`crates/sr-engine/src/part.rs`,
`crates/sr-engine/src/engine.rs`, `bins/secondreality/src/shell.rs`). The original's
`dis_exit()` ends a part's loop, not the code after it: U2E captures the live DAC and runs a
**sixteen-retrace fade** whichever way its gate or loop exits (`VISU/C/U2E.C:541-555`), so
retiring it on the exit tick would skip every one of the sixteen uploads (Task 7's review
measured the defect: the first upload now lands at T+1, so an exit-at-T retirement loses all
sixteen). The default is `false` because every other ported part either returns on the tick
that sees the flag or finishes synchronously — U2A's teardown runs inside its own exit tick
(`VISU/C/U2A.C:239`, `:432-441`) — and a `true` default would give every part an extra tick
after ESC for no source behaviour. `Engine::start` clears the sticky flag, so a drain cannot
leak into the part that follows. Evidence:
`a_runner_keeps_ticking_a_part_that_drains_after_exit` (engine),
`a_part_that_drains_after_exit_keeps_ticking_until_it_finishes` and
`a_part_that_does_not_drain_retires_on_the_exit_tick` (shell),
`a_sticky_exit_flag_runs_the_whole_sixteen_step_teardown` (U2E, every rotated upload checked).

**2. The sound card follows a restart: a swappable drain player (`Device::follow`).** The
window opens the SDL device once with the entry player's `Arc`
(`bins/secondreality/src/window.rs`), so a mid-run restart's fresh `Player` would leave a
windowed `--block 3` run silent after the U2E boundary while the engine's positions and the
WAV dump (whose dump and scratch the restart carries) stayed correct. `Device` now keeps the
player behind a `Swappable` cell (`Arc<Mutex<Shared>>`) that the audio callback reads per
`drain`; `Device::follow(&mut self, Shared)` swaps it after `run.advance`, and the window
calls it whenever `Arc::ptr_eq` says the run's player has changed. The swap is under the
cell's lock, so a callback in flight finishes against the player it started with and never
takes half a buffer from each. The old ring's tail may be dropped: the original's
`restartmus` (`MAIN/U2.ASM:348-397`) interrupts the module it was playing too, and the
samples of a stopped module are not the run's music. Evidence:
`the_drain_follows_a_restart_to_the_runs_new_player` (the old ring's pending count is
untouched, the next drain is exactly the new ring's, a second is silence). No sound card
exists in CI, so the seam is unit-tested and the window's use of it is the three-line
`ptr_eq`/`follow` the review checked, as ruled.

**Cost if wrong.** The drain flag is one trait method and two call sites; a wrong default
changes *when* a part retires by a tick, which the stub tests pin in both dispositions. The
swappable cell costs one lock and one `Arc` clone per tick in the window loop; a wrong
implementation would split a drain across two players, which the seam test's pending-count
and exact-sample assertions would catch. Neither touches a reference frame: no filed keyframe
covers ESC or a windowed restart.

## D36 — GLENZ's renderer lives in the binary, on linear 320-pixel-row buffers (2026-10-20)

Decided: the part is `bins/secondreality/src/parts/glenz/` — `math.rs` (the fixed-point
matrix/rotation/projection), `tables.rs` (`MATHSIN.INC`'s sin/cos), `poly.rs`
(`ceasypolylist` and the `rolcol`/`rolused` allocator), `span.rs` (NEW.ASM's transparent
compositor), `intro.rs` (the zoomer and the FC bounce), `mod.rs` (the `Glenz` part) — a module
in the binary, D32's precedent, not a crate. It keeps the original's 64,000-byte `vram` and
`bgpic` as private **linear 320-pixel-row** buffers (`GLENZ/MAIN.C:18`, `char far
*vram=(char far *)0xa0000000L`; `bgpic` is `:12`; `NEW.ASM`'s `NE_X` and `yrowad` are row*320)
and copies the visible rows into the engine surface once a tick. The original's plane masks and the shipped build's 92-byte CRTC
plane row are memory-layout facts, not the displayed geometry: the picture is 320 pixels a row
either way.

**Evidence.** `reference-indices.md` §7.3's `frame_001499.idx` reproduces its RGB over all
64,000 pixels (11,720 non-black in rows 154–199, 8 indices) with the 92-byte stride, and §7.6
measures the same 320-pixel-row picture's object boxes (2300's 97–217 × 38–149 is
reproduced exactly by a 320-row model). `reference-indices.md` §1.7 is the stride's own
measurement. The port therefore never models the planes or the CRTC row: the buffers are
linear and the surface copy is `surface[(row*width + x)] = vram[row*320 + x]`.

**The substitution and the colour path.** The source's dirty-span walk (`NEW.ASM:66-172`) is
an optimisation whose pixels are identical (a span whose colour did not change already holds
`colour | bgpic[pixel]`); the port repaints every span and the test module runs the source's
two-list walk beside it over a 13-frame corpus (twelve generated polygon scenes and an empty
frame) — `parts::glenz::span::tests::the_full_repaint_equals_the_source_two_list_walk`, **0
differing bytes** of 64,000 a frame, including the never-painted pixel 63,999 — registered as
**A39**.
The colour path inside this renderer is **A37**'s arbitration: `poly.rs` keeps one
`slots: [u8; 256]` keyed by the colour word's **low byte**, shared by the visible and hidden
paths, because the reference's pictures identify that reading; the tree's full-word visible
key exhausts the pool, writes slot 32's group at DAC entry 0 and is rejected (A37,
`glenz.md` §11.2).

**Phase 7 outcome.** The decision held as written. The renderer is
`bins/secondreality/src/parts/glenz/` (`math.rs`, `tables.rs`, `poly.rs`, `span.rs`,
`intro.rs`, `mod.rs`); the buffers are linear 320-pixel rows and the blit is a row copy, with
no plane mask or CRTC row modelled. The held opening reproduces `frame_001499.idx` at **0
differing indices of 64,000** (`the_held_opening_1499_is_the_reference_index_plane`) and the
reference's 2300 object pixels at boot 2305 exactly — 9,989 px, (156.8, 94.1), every object
pixel byte-identical (`the_moving_keyframes_record_the_registered_alignments_and_residuals`).
The repaint substitution measures **0** (A39) and the colour path is A37's adopted low-byte
key. No shipped constant was adopted and no buffer layout was changed.

**Cost if wrong.** A module move, not a behaviour change: the display model is one copy loop
and the buffers' indexing. No filed frame can distinguish a plane-mask implementation from
this one when both display 320 pixels a row; the 92-byte stride would only matter to a port
that read VGA memory directly, which this one does not.

## D37 — GLENZ's tick model is the source's phase machine, calibrated by the reset and `L_r` (2026-10-20)

Decided: the part is a phase machine, one engine tick per `dis_waitb`/poll:

- **WaitMusic**: poll `musplus`; leave while `< -19`; on the first tick it is not, call
  `dis_setmframe(0)` and enter the zoomer (`GLENZ/MAIN.C:290-291`). Exit-aware.
- **Zoomer**: 47 ticks of `zoomer2` (`GLENZ/ZOOMER.C:38-69`: `zy(n)=n(n+1)/8` capped at 260;
  the top band through `zy`, the bottom through `125*zy/260`; the palette's `c` capped at 32;
  the final clear and entry-0 black).
- **ModeSet**: one tick (`memset(vram,0,65535)`, `mov ax,13h`, `testasm()`'s live path —
  `INIT_320X200` and 80-byte rows, state only).
- **Bounce**: 177 ticks (`MAIN.C:320-351`'s `ya`/`yy` recurrence; the final drawn band is
  rows 153…199, `y1=153 y2=200`); the DAC writes of `:308-318` happen first.
- **Wait300**: ticks until `getmframe() >= 300`; then the tmppal build and two `dis_waitb`
  ticks (`MAIN.C:353-398`), the second taking the `bgpic` snapshot.
- **Wait333**: ticks until `getmframe() >= 333`; then `memcpy(pal,backpal)`, `dis_partstart()`
  and the third `dis_waitb` (`:400-404`).
- **Main**: one frame a tick (`frame++` then the physics of `:419-591`, the drawing of
  `:593-640`); the loop ends on `exit_requested`, `frame >= 7000`, or the exit cue
  `-16 < musplus < 0` (`:411`), holding the screen.

**Measured calibration.** `reference/music/second2b` (`music-clock.md` §2.2) reads the reset at
**1111**, `getmframe()<300` first true at **1515**, `<333` at **1560**, and the main loop's
first `dis_waitb` at 1562. The reference's first object pixels are at **1606** in two capture
sessions, and the source physics' first drawn main-loop frame is **44** — an exact six-frame
silhouette match (reference-indices.md §7.6) — so **`L_r = 1606 − 44 = 1562`**, main-loop frame
f being the picture at `1562 + f`. The session spread is **zero** across the trace and three
capture sessions. Block 1's GLENZ wait exits at **8521** (`second1d`), and the picture's first
change is at **8522** (`out/p7-b1-collapse-dense`) — the sample/publish instant, one frame
after the reset sample. The bounce's 177 is the source's own count (a C
reproduction of `:320-351`; the drawn band converges to 153…199), not a fit.

**The port side, measured (Tasks 5–7).** The block-2 wait exits at **1117**, six ticks after
`second2b`'s 1,111, with the reset and the first zoomer body on the same tick. The intro's own
relative ticks are the source's — 47 zoomer bodies, 177 bounce bodies, the two `dis_waitb`
ticks, the part-start wait and the main loop's first wait (`the_intro_is_47_plus_177_ticks_and_the_measured_waits`)
— but the port's real player reaches the two `getmframe` crossings (300 and 333) one tick
earlier relative to the reset than `second2b`, so the reset-to-frame-44 span is **494** against
the reference's **495**; no transition was changed to move the loop start. Therefore
**`L_p = 1567`** against `L_r = 1562`: two independent measurements agree — the first
above-floor object pixels at boot **1611** (8 px, box 172..179 × 0..0, the reference frame 44's
silhouette; none at 1,610, 21 px at 1,612) and the first faded surface at **2,269** (`pal`
after main frame 701, b = 63) — and the plan's aligned index is **K + 5** (A36,
`glenz.md` §11.1).

The **16-bit int model** is the source's own: every physics term wraps at 16 bits (the jello
trajectory's `xscale` 115 at frame 800 is the wrapped value), and the reference's 2300 object
pixels are reproduced exactly at boot 2305 with no 32-bit widening, so D24 (a) is not
contradicted and no shipped constant is adopted. The **palette pairing** is surface *N* with
`pal` after *N−1*: the source uploads, then does the frame's work (`MAIN.C:414-590`), and the
reference's capture transiently shows surface *N−1* with the DAC of *N* — the 0/1-frame surface
lag and one-frame DAC lead A36 records. The colour path is A37's adopted **low-byte key**: at
2305 all 5,208 differing pixels are palette entries 0..15 and **0** have an index ≥ 16 — the
fade at b = 26 against the port's 27, the pairing and not the colour model.

**The music gates** are the boundary wait (`musplus < -19`, `MAIN.C:290`) and the exit cue
(`−16 < musplus < 0`, `:411`), both read from the published `zplus`/`row`
(`musplus_from`). `musplus` is not `_np_zplus`: at the block-2 boundary (frame 576, `zplus` 0)
it is −32, and the wait opens only when order 2 row 45 with `zplus` 1 gives −19.

**Cost if wrong.** A one-tick error in any phase shifts the moving keyframes: the intro's
absolute start moves, and the loop's frame counter is the palette's and the physics' clock.
Task 7's comparison at 2300/3000/3400 and Task 5's tick-map tests are the arbiters; the reset
and `L_r` above are the calibration against which a drift is visible as a whole number of
frames, not a fitted offset.

## D38 — The two shell seams: the block-1 restart row and the post-PANICEND cue wait (2026-10-20)

Decided: the shell gains two seams, both narrow and both the source's.

**1. The block-1 `MUSIC1@0` restart.** `MAIN/U2.ASM:813-818` fires for a run with `whattorun &
2` and `& 1` (block 1), between BEGLOGO and GLENZ: `restartmus(1, 0, 48700)`. The port's row is
`MUSIC_RESTART_AT: &[(&str, ModuleId, u8)] = &[("u2e", Music0, 18), ("glenz", Music1, 0)]`,
installed only when the run's current player is not already `MUSIC1@0`
(`Sequence::restart_music`'s equality check) — block 2 starts MUSIC1@0 at its boot
(`MAIN/U2.ASM:766-775`) and must not reload it. The engine's restart keeps D34's four-frame
waiting.

**2. The post-PANICEND cue wait.** `MAIN/U2.ASM:826-853`, the wait at `:833-853`
(`@@zh5`/`@@zh4`), polls `musplus`: first while `>= 0`, then until `> 0`, then continues. The
port installs a held `CueWait` part in place of the next plan part whenever the part that just
retired is `panicend` (the pre-flight ruling's `cue_pending` flag, consumed once): it draws
nothing, holds the surface, consumes one tick a call, and is exit-aware. A run whose plan ends
at `panicend` enters the wait and then the hold, as the original does.

**Measured.** The block-1 restart lies in **(8049, 8100)**: the pre-reload window reads MUSIC0
at 8049 (`out/second1d-limit`) and the filed `second1d` opens at 8100 on MUSIC1 order 1 row 4;
the wide `7800..8800` window refuses across the reload (`_np_ord reads -1 … the base 0x0051F0
is not this segment's`, exit 2). The block-1 GLENZ wait exits at **8521** (`music-clock.md`
§2.2), 105 row changes after 8100 (315 ticks at 3 a row against the 312 sampled `zframe`s plus
the reset tick).

**The cue wait's calibration is PANICEND's retirement, not its last drawn frame.** PANICEND's
mapping is `boot = 8782 + tick` and the part itself retires at **tick 121 = boot 8903**
(`docs/fidelity/panicend.md` §7); block 2's `second2` trace shows the wait's phases on the
published words there:

```text
$ awk -F, '($1>=8900 && $1<=8906) || ($1>=8950 && $1<=8956) || ($1>=9215 && $1<=9222)' \
    reference/music/second2/trace.csv
8903,39,19,18,0,3,130,0,2,3047     <- the part has retired; musplus 19 (>=0): phase 1 waits
8953,39,32,18,0,3,130,0,2,3085     <- musplus -32: the first negative; phase 1 ends
9219,42,1,30,0,6,130,0,2,3282      <- musplus 1 (>0): the exit
```

The old 8823/8827 crossing — order 39 row 0/1, `zplus` 2 — is ~80 frames before the part
retires and is PANICEND's own music position, not the wait's exit; it is withdrawn. Task 6 pins
**8953/9219** (the first negative and the first positive after it).

**The block-1 restart's port timing is measured and registered as A38.** Task 6's chain
measured it: the restart installs MUSIC1@0 at the port's boundary **8,057** (inside the
reference's unreconciled (8,049, 8,100)), and the source-faithful seek ends the wait at
**8,770** — **713 ticks after the boundary** and **249 frames after the reference's 8,521**,
whose own exit is also order 2 row 45. The difference is the seek's: the reference's player
had already played a stretch of MUSIC1 when its chain reached GLENZ, and `second1d`'s first
sample (order 1 row 4 at 8,100) cannot separate a seek that left the row from a play that
began during the module load. Nothing was fitted; **A38** carries the measured values and the
commands (`a_block_one_run_is_alku_then_u2a_then_pam_in_the_boots_numbering` pins 8,057,
8,770, 713 and 249 and reads `second1d`'s 8,521 = `2:45`;
`the_block_one_run_restarts_music1_at_glenz` pins the fresh install and `waiting == 4`).

**One limit is carried.** `second1d`'s first sample is 68 rows into MUSIC1, which
at the module's rate puts the restarted play clock ~275 frames before 8100 — earlier than the
part chain's boundary (BEGLOGO's held 900 screen runs through 8520 and its 172 ticks put the
boundary near 8060). The trace cannot tell a seek that left the row from a play that began
during the module load, and the reload hides the restart frame itself. The port implements the
restart as the source says (seek to order 0 at the boundary); the shell's chain tests pin the
module, the order, the waiting and the boundary, and this interval is the trace's record
rather than a fitted row for the port's player.

**Cost if wrong.** The restart row is one table entry and one equality check; the cue wait is
one private part and one install. A wrong guard reloads MUSIC1 in block 2 (audible as a
restart at BEGLOGO→GLENZ), and a wrong cue phase ends PANICEND's screen early or late; the
shell tests pin both (`the_block_one_run_restarts_music1_at_glenz`,
`the_block_two_run_does_not_reload_at_glenz`, the cue wait's two phases).

**Phase 7 outcome.** Both seams shipped as written. The block-1 restart fires in block 1 and
not block 2 (`the_block_one_run_restarts_music1_at_glenz`,
`the_block_two_run_does_not_reload_at_glenz`, the latter asserting the same player `Arc`); the
cue wait runs on the measured phases — 8,903 `musplus` 19 (phase 1 waits), 8,953 −32 (the first
negative), 9,219 1 (the exit) — holds the screen
(`the_post_panicend_wait_ends_on_a_positive_musplus`, `the_cue_wait_holds_the_screen`) and
sits between PANICEND and the next plan part or the hold
(`the_post_panicend_wait_is_between_the_part_and_the_next_one`); an exit flag already set on
PANICEND's last tick skips the wait, the `@@skip5` first-poll path
(`an_exit_on_panicends_last_tick_skips_the_cue_wait`). Blocks 1 and 2 have no stop
(`blocks_one_and_two_have_no_stop`; every block's expected first missing part is `None` in
`every_blocks_plan_stops_at_the_first_part_the_registry_lacks`). The block-1 wait's
8,770/713/249 is **A38**; the reference's restart interval (8,049, 8,100) stays recorded as
the trace's limit. Nothing was fitted.

*(Phase 9 Task 6b: the "one limit" above is resolved and the 8,770/713/249 restated. A restart keeps
the previous module's channel volumes (D45), so MUSIC1's order 0 is 4 rows long and `second1d`'s
first sample is 8 rows into the restarted module, not 68 — the trace's restart is at 8,056, inside
(8,049, 8,100), and the port's wait now ends at **8,527**, 470 ticks after the boundary and **6**
frames after the reference's 8,521, which A38 registers as the restarted clock's residual.)*

## D39 — The vault boots the vendored effects behind the standalone DIS (2026-10-21)

Decided: the vault's references are the three unreleased effects run as their own programs —
`GRID/P.EXE`, `HARD/B.EXE` and `TWIST/T.EXE` — staged with the vendored standalone
`DIS/DIS.EXE` (`270f5fe2c7c7d17b4720c2019c2832f4bd3863f97e750315974692911f0ead08`) by
`tools/vault_capture.sh` and mounted read-only. The capture is route A: the config's
`[autoexec]` writes `RUN.BAT` (`@ECHO OFF`, `DIS.EXE`, the effect) and the tool waits for the
first graphics frame (`--boot autoexec --boot-wait graphics --cycles 30000`). One
`KBD_space` is delivered for GRID and HARD, none for TWIST.

**The standalone DIS is the non-INDEMO server.** `DISINT.ASM`'s service 6 returns 0 until
`checkkeys` consumes a key, then echoes the requested code once (`:242-282`, `:154`); there is
no player, and service 7 (copper) is a bare `ret` (`:294-316`). The captures are therefore
silent. GRID's `getch()` (`MAIN.C:153`), HARD's `while (dis_muscode(1) != 1 …)`
(`B.C:138`) and TWIST's no key (`MAIN.C:49`) are the whole input vocabulary; the later cuts —
GRID's `dis_muscode(3)` (`MAIN.C:160`) and HARD's `muscode(0xff)` (`BP.C:263`) — need a second
key and never fire.

**The port's side.** The three registry rows carry no `boot_anchor`; `music_for_part` returns
`None`, no player is installed and `ctx.music()` is `None`; a silent run refuses
`--audio-dump`. The single start key is modelled at the part's start, not waited on: GRID's
`init` performs the state the key selects (A40), HARD's gate is passed at `init` (A46), TWIST
takes no key (A43). A `--part` run numbers its frames from zero (`index_origin: "part"`,
`anchor: null`, `music: null`). `--vault` and `--part` run the same programs.

**Measured.** The filed captures' `K0`s are GRID **353 ±1**, TWIST **3 ±1** and HARD
**356 ±1** (`reference-indices.md` §24); the twenty filed frames compare at the residuals
recorded in the evidence section (A40/A41, A43–A45, A46–A51), and the silent manifests are
pinned by `a_part_run_of_{grid,hard,twist}_is_silent_and_numbers_itself_from_zero`.

**Cost if wrong.** If the vault ever wants music or the demo's own `muscode`, the silent-run
rows and the start substitutes are the seams to change; the pictures do not move. The
alternative — booting `SECOND.EXE`'s `zinfo` — would be a different program and is not what
the artifacts do.

## D40 — The vault menu is window-only, and `--part` is the headless surface (2026-10-21)

Decided: `--vault` opens a window-only 320×200 menu (`vault_menu.rs`) over the demo's own
`ALKU/FONA.UH` strip: the title `SECOND REALITY` / `VAULT`, the three rows GRID/HARD/TWIST, a
`>` marker on the selection, and the footer `UP DOWN  ENTER  ESC`. Up/Down wrap, Enter starts
the selected part, ESC closes the window. The flag is refused with `--headless`, `--part`,
`--block`, `--dump-dir`, `--dump-frames`, `--frames`, `--run-seconds` and `--audio-dump`
("the vault menu needs a window; name a part with --part"), and accepts the presentation
flags. `--part grid|hard|twist` is the headless/tests surface and starts the same programs.

**The return split is the source's own.** In a running part the first ESC is the part's exit
(the original's `dis_exit`), so the shell retires it and the run holds; a second ESC on the
held run returns to the menu rather than closing the window. A part that finished by itself
(HARD's `default` arm, TWIST's 4,444-frame cap) is already holding, so one ESC returns. ESC in
the menu closes. The normal, non-`--vault` path's ESC/close semantics are unchanged.

**Deferred.** Phase 9's settings screen and loop mode are deliberately not here; the menu is a
new, non-fidelity surface (there is no original frame to compare), and its layout/colours are
the port's choice. The three parts stay silent (D39).

**Measured.** The manual window check (title, three selections, ESC return, ESC exit, close
button) and the tests: `the_selection_wraps_up_and_down`, `enter_chooses_the_selected_part`,
`escape_closes_the_menu`, `an_unhandled_key_redraws_nothing`,
`the_menu_frame_is_three_entries_with_a_selected_row` (the `>` marker and `VAULT` subtitle are
asserted directly), `the_vault_flag_is_parsed`, the three refusal tests,
`a_vault_run_accepts_the_presentation_flags` and
`an_escape_on_a_held_vault_part_returns_to_the_menu`.

**Cost if wrong.** The menu is one small module and one outer loop; the normal path is
untouched. A wrong ESC rule would return from a live part instead of ending it (or close the
window instead of returning); the reducer and window tests pin both.

**Closed 2026-10-25 (Phase 9 Task 10).** The deferral is spent: the settings screen and loop mode
are now D41–D44 (the demo's own run and its end, D41; loop mode, D42; the enhanced sound option the
settings offer, D43; the settings screen, D44), and the restart carry-over the loop's restart rides
on is D45. None of them touched the vault: its parts stay silent and its menu window-only, as above.

## D41 — The demo's own run, and how it ends (2026-10-24)

Decided: a run with no argument is the demo — `SECOND` with no argument is `SECOND 1`
(`MAIN/U2.ASM:108`'s default `whattorun` 7fh and `partsmask[1]`'s 1fh gate the same parts,
`docs/fidelity/part-sequence.md` §4), so `Cli::parse` makes it `--block 1` and the Phase 0 test
card is reached only by `--part testcard`. A run built from a block (`--block`, or no argument)
is `Ending::Exit` and ends the way `SECOND.EXE` does; `--part`, the vault's runs and the shell's
own test constructors are `Ending::Hold` and hold for ever, as before (`RunSettings`, passed to
`Sequence::new` and `Sequence::with_run`; `Cli::run_settings`).

**The exit stage is the hold.** When an `Exit` run's plan runs out, `@@partsdone`
(`MAIN/U2.ASM:935-939`) is `mov cx,1; call fademusic`: `fademusic` (`:333-346`) steps the
module's global-volume byte `module[0x30]` down by `cx` once a `dis_waitb`, and the byte is 0x40,
so the fade is **64** frames (`EXIT_FADE_TICKS`) with the last part's picture still up — which is
what the existing hold already shows while it fades the music a step a tick. `fatalexit` then
sets text mode (`:967-968`) and waits twenty `copperwaitborder`s (`:969-973`, `EXIT_BLANK_TICKS`)
on its blank screen: the port composes those **20** ticks as index 0 through an all-black
palette, the border index left as the part set it (it reads the same black palette, and
dropping it would resize the window). After the 84th tick of the hold the run has
`Sequence::ended()` — distinct from `finished()`, a part that could not start — and the window
closes on it. An ended run keeps ticking and composing the blank, so a headless dump past it is
answered rather than cut short. A hold that began at a part the port has not written is not the
plan running out, and has no exit stage.

**ESC during the stage closes at once.** The original's stage acts on no key — `fademusic`
waits through `dis_waitb` and never tests the exit flag, and the `copperwaitborder` loop runs
after `dis_resetint` (`:966`) and reads only the retrace bit (`MAIN/COPPER.ASM:72-81`) — so it
cannot be skipped.
The port's stage is the hold, where ESC has always closed the window, and it keeps doing so:
a deliberate ESC is not made to wait out up to 84 ticks. That is the port's courtesy and a
recorded difference (`APPROXIMATIONS.md` **A59**, with the end box).
Leaving ENDSCRL is therefore **one** ESC (the part's own `dis_exit`), then
the stage, then the close — where it used to be two.

**The end box goes to the launching terminal.** `printend` (`MAIN/U2.ASM:506-535`) copies
`U2END.BIN` (80×25 cells of CP437 character and attribute) into text-mode memory and puts the
cursor on row 14 (`:530-533`); `fatalexit` calls it on every exit (`:974`). The port has no text
mode, so `main` prints rows `0..14` of the file to stdout once, after the window closes, for
every **windowed demo run** however it closed — the close button and an error path included,
before the error line, as `printerror` follows `printend` — and never for `--part`, `--vault` or
`--headless`. CP437 is mapped to Unicode by `endscreen.rs`'s own table (no new dependency), each
row's trailing blanks are trimmed, and when `std::io::IsTerminal` says stdout is a terminal each
attribute change is an SGR sequence (low nibble foreground, bits 4–6 background, blink ignored)
and every line ends with `ESC[0m`.

**Unchanged.** `--part` runs hold at their end; the vault's ESC semantics (D40); Ctrl skips one
part (D21); headless runs never open SDL and never wait on a key; no part's registered test
count moved.

**Measured.** The tests: `a_demo_run_fades_then_blanks_then_ends` (volume 0x40 → 0 over ticks
1..=64 with the picture held, every pixel `(0, 0, 0)` over 65..=84, `ended()` from 84),
`a_part_run_still_holds_after_its_end`, `a_run_with_no_arguments_is_the_demo`,
`the_test_card_is_still_a_part`, `a_part_run_holds_and_a_block_run_exits`,
`the_end_screen_is_the_shipped_box`, `the_end_screen_colours_are_its_attributes`,
`an_ended_run_closes_the_window`, `escape_during_the_exit_stage_closes_at_once`,
`an_index_past_a_demo_runs_exit_stage_is_its_blank_screen` and
`the_end_screen_is_printed_after_a_windowed_demo_run_only`.

**Evidence (Phase 9 Task 11, at the close-out).** Every test above is in
`cargo test -p secondreality --bin secondreality`, which is 659 passed, 0 failed and 7 ignored
inside `cargo test --workspace`'s 1,459 passed, 0 failed and 11 ignored. The stage begins only once
a plan has run out, and no compared frame moved: `cargo xtask rediff` exits 0 with 231 of 231
frames at their registered counts, its `--block` entries included. The windowed path was run by
hand in Task 8: a windowed `/gravis` run (`NoEndScroller`, the same `Ending::Exit`) closed its own
window and printed the box. The one path no machine check drove is ESC from ENDSCRL ending a
block run. It is left for a hand check (`PROGRESS.md`).

**Cost if wrong.** The stage is one counter on the hold and one predicate in the window; the
parts' frames are untouched, because the stage only begins once the plan has run out. If the
original's stage should be honoured exactly, ESC during it is one branch in
`EscapeKey::apply`; if the box should not print on an error path, it is one condition in `main`.

## D42 — Loop mode: `--loop`, `/gravis`, and the seam between two passes (2026-10-24)

Decided: the original's `looping` byte (`MAIN/U2.ASM:746`, the setup's `ip[3]`, `START.C:103`)
is `shell::Looping` — `Off` (0), `On` (1), `NoEndScroller` (2) — carried in `RunSettings::looping`
and set by the command line: `--loop` is the setup menu's "Demo looping: Enabled (no end scroller
shown)" (`MENU.C:229-238`, 1), and `/gravis` is 2 (`START.C:89-93`: `memicmp(argv[1],"/gravis",7)`,
the first seven characters in any case), which also clears `Cli::setup` — the field Task 8's
settings screen reads, because `/gravis` skips `menu()`. **`/gravis` is honoured in any argument
position**, where `START.C:89` reads only `argv[1]`: the port's flags are not positional
(`--block 5 /gravis` is a natural command line), and the original's `SECOND 5 /gravis` would
have ignored it. Any other `/` switch is ignored (`U2.ASM:605-606`: `checkcmdline` returns at a
`/` with `whattorun` at its 7fh), so `/x` is the demo. Looping is a block's run: `--loop` or
`/gravis` with `--part` or `--vault` is refused.
*(2026-10-25 (Phase 9), the final review — **`/gravis` wins over `--loop`, in either order.**
Until this fix the later flag won, so `--loop /gravis` was `NoEndScroller` and `/gravis --loop`
was `On` with the setup skipped. In the original the two never meet: `/gravis` sets
`m_looping=2` *instead of* calling `menu()` (`START.C:89-94`), and `menu()` is the only place
`looping` 1 is chosen (`MENU.C:229-238`). `--loop` is that menu's choice, so `Cli::parse` now
applies `/gravis` after reading every argument, as 2 and with the setup skipped.
`cli::tests::gravis_wins_over_loop_in_either_order` pins both orders, with and without
`--block`.)*

**The plan.** `shell::run_plan(block, looping)` is `block_plan(block)` without ENDSCRL whenever
`looping` is not 0 (`:914-917`: `cmp cs:looping,0; jne @@skipp6` jumps over `exe23`).

**The loop pass** (`:919-933`). When a `Looping::On` run's plan runs out, the shell starts its loop
seam instead of the hold — the last part's picture (CRED's on every block but U, whose plan is
DDSTARS alone), held by a part that draws nothing:

- ticks 1..=64 (`EXIT_FADE_TICKS`): `mov cx,1; call fademusic` (`:926-928`), the global-volume
  byte stepping down a tick (`:333-346`);
- at the fade's end `LOOP_RESTART` = (MUSIC0, order 0) is installed (`:929-932`, `restartmus` with
  `ax=0, bx=0`; `_zplaysong(0)` at `:394`) under `MusicStart::restart`'s rule (D34: four waiting
  polls), at the next tick's boundary;
- `LOOP_RELOAD_TICKS` = **11** more ticks of that screen, then `jmp @@partsagain` (`:933`): the
  plan is `run_plan('1', On)` from ALKU, and `Sequence::pass()` is 2. `whattorun` is 7fh on every
  pass (`:919`), so pass 2 is block 1's sequence whatever pass 1 began at, and never the hidden
  part (7fh has no bit 7). Pass 2's restarts are `MUSIC_RESTART_AT`'s two — MUSIC1@0 at GLENZ,
  MUSIC0@18 at U2E — and the block-entry restarts (MUSIC1@42, `:858-863`; MUSIC0@25,
  `:900-905`) never fire: they are `with_run`'s entry installs, never `take_next`'s. The
  "already playing" check in `restart_music` cannot suppress pass 2's sites: the run's player is
  then `LOOP_RESTART` (MUSIC0@0) at GLENZ and MUSIC1@0 at U2E, neither the row's own.
- `/gravis` (`NoEndScroller`) goes to `@@partsdone` after CRED (`:923-925`): D41's exit stage, then
  the end.

**`LOOP_RELOAD_TICKS` is the release's measured interval, not a fit (D24).** In the loop capture
the fade is 9029–9092 and ALKU's first 320×400 frame `L` is 9104, so the frames between the fade's
end and `L` are 9093–9103 — **11** — CRED's 320×200 black throughout but for the unpublished mode
set at 9102 (`reference-indices.md` §25.3: the `--frames "$(seq -s, 8951 9050)"` batch for `C`,
`--frames "$(seq -s, 9090 9101)"`, `--frames 9103` and `--frames "$(seq -s, 9104 2 9150)"`). If
404 is ALKU's mode set rather than its first frame (§25.3's Δ = 8698 reading), `L`'s counterpart is
9102 and the same arithmetic gives 9093–9101, **9**.

**The seam is compared from `C`, and it is 2 frames late.** The loop capture's recipe numbers its
frames 115 later than the B1 recipe the port's `--block 5` is in (§25.1), and the port's CRED ends
**81** frames earlier than the original's: its `C` (the first frame that is the filed `C`'s bytes,
read off the dump) is 8833 against the original's 9029 − 115 = 8914 — CRED's own chain drift
(`cred.md` §7; `endscrl.md` §6.1 measures it as 82 from ENDSCRL's start). The port's 8826–8832, the
iris's last seven steps, are byte-identical to the original's 9022–9028 at that offset. So
`tests/loop.rs` pairs the port's `C_port + (n − C)` with the original's `n`. Then the port's CRED
shows its closed iris on **two** ticks before it retires (8833, 8834), so its fade is 8835–8898 —
the original's 9031–9094, 2 frames after 9029–9092 — and with the release's 11 its `L` is **8910**,
the original's 9106: `L − C` is 77 against 75. Measured, with the constant at 11:

| filed frame | port index (`n − 196`) | result |
|---|---|---|
| `cred` 9029, 9055, 9081 | 8833, 8859, 8885 | 0 differing pixels |
| `alku` 9104 (`L`) | 8908 | **a different screen**: the port's CRED 320×200, ALKU starts 2 later |
| `alku` 9600, 10419, 11419 | 9404, 10223, 11223 | 0 differing pixels |

and the port's own `L`, 8910, is the original's 9104 byte for byte; its 8909 is CRED's 320×200. In
the port's numbering `C`, the fade, `L` and Δ are 8833, 8835–8898, 8910 and `L − 404` = 8506,
against the original's 9029, 9029–9092, 9104 and 8700 (8698 if 404 is a mode set). The Δ
difference is 8700 − 8506 = **194 = 196 − 2**: 196 = 115 + 81 is the offset at `C` (the recipe and
CRED's drift), and the port's `L` sits 2 frames later than that offset puts it (CRED's tail), so
`L`'s offset is 9104 − 8910 = 194.

**The race.** ALKU's first pass-2 `dis_sync()` (`ALKU/MAIN.C:59`) races the restarted player in the
original: in six sessions of seven it reads the new song and waits for order 2; in one it reads the
stale (43,43), sync 10, and leaves at once (§25.3). The port installs the restart eleven ticks
before ALKU starts, and a fresh player publishes (0,0) until its clock runs, so ALKU's first read is
the new song's: the port is in **the majority group**, the filed one — its fade-in begins at 10045,
not near the early group's 9112 (the port's 8916).

**Pass 2's music is 5 frames late against `C`.** The seam trace has the restarted song at (0,1) at
9112 (§25.3), 20 frames after the original's fade ends at 9092 — the port's 8916. The port's fade
ends at 8898, and the restart installed there publishes (0,1) 23 frames later, at **8921**: 2
frames of CRED's tail and 3 of the restart's recipe (`MusicStart::restart`'s timing against the
original's `restartmus` here). ALKU's pass-2 screens follow the music: the original's first lit
frame is in (10236, 10238] and its "A Future Crew Production" hold starts in (10296, 10298]
(two-frame sampling), the port's (10040, 10042] and (10100, 10102]; the port's are **10045** and
**10106** — **3 to 5** and **4 to 6** frames late. The held frames sit inside their holds
regardless.

**The seam's residuals, registered by Task 10 as `APPROXIMATIONS.md` A61:** CRED's 2-tick
closed-iris tail at the seam (the fade and `L` 2 frames late, the port's 8908 a CRED frame where
the original shows ALKU); the pass-2 music lag (5 frames at (0,1), 2 of them the tail's and 3 the
restart recipe's); ALKU's pass-2 fade-in and hold lags (3 to 5, 4 to 6); and CRED's 81-frame chain
drift, which shifts every frame of pass 2 by the same amount in absolute numbering. Closing the
tail is CRED's work (its retirement tick), and closing the 3 frames of the recipe is a second
constant D24 does not allow without a measurement of its own.

**The exits.** ESC ends only the running part: the seam is the shell's, drains after exit
(`fademusic` and `restartmus` read no flag), and pass 2's first `Engine::start` clears the flag,
so a press there is spent on nothing. Ctrl (D21's one-part skip) has no part to skip in the seam.
The original's way out of a loop is its sticky Ctrl at `:920`; the port's is the window's close
button, and the end box is printed then (D41). The title bar names `pass N` from pass 2 on.

**Unchanged.** Pass 1 of a `--loop` run is the plain run's, frame for frame and row for row, up to
the seam (`looping_leaves_pass_one_alone`, `looping_leaves_pass_ones_music_alone`); `--part`
holds; the vault (D40); Ctrl (D21); headless never opens SDL; no part's registered test count
moved.

**Measured.** `tests/loop.rs`'s `the_loop_seam_is_the_originals` (the port's `C` read off the dump
and the 81, the seven frames' results above, the port's `L` at 8910 and its bytes, 8921, 10045,
10106) and `looping_leaves_pass_one_alone`; `shell.rs`'s `a_looping_plan_has_no_end_scroller`,
`a_loop_pass_fades_then_restarts_music0_at_order_0_and_starts_alku`,
`the_seam_is_not_a_part_escape_and_ctrl_leave_it_alone`, `a_loop_pass_runs_every_block_whatever_the_entry`,
`a_loop_pass_fires_only_the_full_runs_restarts`, `gravis_runs_once_without_the_end_scroller_and_ends`;
`cli.rs`'s `the_loop_flag_is_looping_on`, `gravis_is_looping_two`, `unknown_slash_switches_are_ignored`;
`window.rs`'s `a_looping_run_is_left_by_closing_the_window`,
`the_status_line_counts_the_pass_from_the_second_on`. The iris's seven-frame agreement is
reproduced by §25.3's `out/p9-loop-c` capture command against
`cargo run --release --bin secondreality -- --block 5 --headless --dump-frames "$(seq -s, 8755 8840)" --dump-dir out/t4/b5-iris`.

**Evidence (Phase 9 Task 11, at the close-out).**
`cargo test --release -p secondreality --test loop -- --nocapture` is 3 passed and 0 failed.
`the_loop_seam_is_the_originals` asserts every figure above: the port's `C` 8833 and the drift 81,
the seven filed frames, `L` 8910 (`L − C` 77 against 75), the fade's end 8898, `(0,1)` at 8921,
10045 and 10106. `cargo xtask rediff`'s `loop/cred` (9029, 9055, 9081 at 8833, 8859, 8885) and
`loop/alku` (9104 at 8910, A61's pairing; 9600, 10419, 11419 at `n − 196`) are 0 differing pixels
at all seven, and every count is the registered one (exit 0).

**Cost if wrong.** If 404 is ALKU's mode set rather than its first frame, the constant is 9 — one
number, and the test's pinned `L`. The 2-tick tail is not a reading of the capture: the fade's
end, 9092, is trace-measured (the player's `zframe` stops there, §25.3), so the original's fade is
9029–9092 and it is the port's CRED that holds its closed iris two ticks longer. Closing it is
CRED's retirement tick, and this constant does not move. If a looping run should be left with
ESC, it is one branch in `EscapeKey::apply`.

## D43 — An enhanced sound option: the port's own renderer at 48 kHz, off by default (2026-10-24)

Decided: the player gains a second renderer, `Quality::Enhanced`, chosen by
`--sound-quality enhanced` (`Cli::quality`, carried in `RunSettings::player`) and **off by
default**. `PlayerOptions` is now `{ mix_rate, stereo, quality }`, and `quality` replaces the
`interpolation` flag nothing read. That flag stood for the object's `[0x919]`, the byte `S1x`
writes, which neither module sets (`music.md` §9.12). `PlayerOptions::output_rate()` is what the
sound card and the WAV dump are opened at: 20,000 for the authentic stereo `Hz40000`, 48,000 for
the enhanced renderer. `MusicSource` becomes `fn(ModuleId, PlayerOptions)`. The `Sequence` keeps
the run's options and passes them at every install, so a restart's new player is built with them
before `Player::restart_from` takes the old one's state over (D45).

**What it is** (`music.md` §13). The tick schedule and every voice's step and advance are the
authentic path's: the mixer gets the same authentic budget and walks the voices exactly as
`Mixer::frame` does. Each authentic frame becomes 2 or 3 output frames by an exact 12/5 integer
accumulator. Output frame `j` of `n` interpolates each voice at its position plus `j/n` of its
step, weights it by `(s − 128) × scale(m_vol)` without the `>> 8`, and converts the side's sum
with the authentic curve's small-signal gain (`Curve::sample` at `0x400 ± 1`), clamped to `i16`.

**Why it is not a fidelity approximation.** An approximation (`APPROXIMATIONS.md`) is the port
falling short of the original on a path it claims to reproduce. This path claims nothing. The
register still lists it, as **A60** (Task 10), because a run with it sounds different from the
original, and the entry says by how much. It is opt-in, it is labelled the port's own in the
help text and in the note, and the authentic path stays the reference. It is the default and
everything compared against the original runs on it.
It is unchanged byte for byte: `tests/mix.rs`'s `the_authentic_stream_is_pinned_byte_for_byte`
asserts the FNV-1a digests of 12,000 polls from four entry orders, and the digests were computed
on a clean extract of 8004905, before the renderer existed. The phase's rule that a setting may not move the clock is held
by tests, not by argument. `enhanced_keeps_the_authentic_tick_schedule` compares every published
word at each of 30,000 polls of both modules, so a run's pictures are the same under either
quality. Pitch and level are measured against the authentic path: within 0.089 % on nine sustained notes
by autocorrelation, and −0.18 dB and −0.57 dB over ten seconds (`tests/enhanced.rs`; printed by
`cargo test --release -p sr-audio --test enhanced -- --nocapture`, re-run in Phase 9 Task 11:
worst note −0.0890 %, MUSIC0 −0.177 dB, MUSIC1 −0.565 dB, 5 passed).

**Cost if wrong.** The authentic path is the default and is untouched, so a wrong enhanced
renderer costs only the runs that ask for it: its sound, never a picture or a published position.
If an edit to the shared `Mixer::mix` moved the authentic stream, the golden fails at the first
of its four digests.
If it did move the clock, the schedule test fails at the first poll that differs. If the
accumulator drifted, `enhanced_emits_twelve_frames_for_every_five` fails, and so would the run's
audio-to-picture sync over a long run. If the options stopped reaching a restart, the run would
switch back to authentic at U2E, and `a_restart_keeps_the_runs_sound_quality` names it. Removing
the option is one enum variant, one `match` in `Mixer::mix`, and the flag.

*(2026-10-25 (Phase 9), the final review — **the flag's scope.** `--sound-quality enhanced` is
refused for a run with no player, in the same words `--audio-dump` uses: `--sound-quality
enhanced needs a run with music, and a run of <part> has none`. That covers the test card and
`--part grid|hard|twist`, whose `shell::music_for_part` is `None`. `--vault` already refused it,
and still does with its own message. Before this fix, a silent `--part` run accepted the setting
and did nothing with it. `authentic`, the default, is accepted everywhere.
`cli::tests::enhanced_sound_is_refused_for_a_run_with_no_music` pins every registry part and
every block.)*

## D44 — The settings screen: the original's setup, asked every time, nothing kept (2026-10-24)

Decided: a **windowed demo run** opens on a five-row settings screen before anything plays.
`START.EXE` ran `menu()` on every start except `/gravis` (`MAIN/START.C:89-95`), and spec §1
replaces `START.EXE`, so the screen is the port's stand-in for it (`settings.rs`, the window
loop `window::setup_run`). **What it shows**, in the original's row order and then the port's:
**Sound** On/Off (the soundcard row, `MENU.C:190-209`; Off is `--no-audio`, and the player still
runs, as the original's no-sound option still syncs to the music, `MENU.C:114-123`; D46 says why
its clock is the reference's and not the original's no-sound mode's), **Sound
quality** Authentic/Enhanced (`--sound-quality`, D43), **Demo looping** Disabled/Enabled
(`--loop`, D42; `MENU.C:229-238`), **Aspect** 4:3/Square and **Border** On/Off
(`PresentOptions`). Up and Down move and **clamp** at the first and last row (`MENU.C:158-159`);
Left and Right change the value and **wrap** (`:161-168`); Enter starts the run, and ESC leaves
without one — and, like `fatalexit` (D41), `main` then prints the end box. The close button
does the same.

**Nothing persists.** The original asked every time and the port does too: no file is written
and none is read. The command-line flags are the initial values (`Settings::from_cli`), and the
choices become the command line the run is built from (`Settings::apply` writes `no_audio`,
`quality`, `looping`, `aspect` and `border`; it leaves `/gravis`'s `NoEndScroller` alone, because
that run never shows the screen). The run is therefore built, and the sound card opened, only
after Enter: a chosen Enhanced reaches the device's 48,000 Hz open rate through
`RunSettings::player` and `PlayerOptions::output_rate`, and a chosen Sound Off decides there is no
card. Aspect and Border re-present the screen under the new `PresentOptions`, which resizes the
window; the screen's frame carries an overscan colour so the Border row has a border to show.

**When it shows.** `Cli::shows_setup`: the run is built from a block (so no argument, `--block`
and `--loop` show it), it has not skipped it, and it is not headless. It does **not** show for
`--part`, `--vault` (its parts are not the demo's), `--headless` (no window, no key), `--no-setup`
(new: the way to start without it, the flags then being the settings) or `/gravis`
(`START.C:89`, which also clears `Cli::setup`). Every other run starts exactly as before, and
the screen waits in bounded `wait_event_timeout`s, never spinning.

**Its look.** The text layer the vault menu had (the FONA table, proportional layout, the `>`
marker, the palette) moved to `menu_text.rs` with its output unchanged — the vault menu's tests
pass as they were — and gained the seven marks `( ) + - * = '` the source's order holds at cells
67..73 (`ALKU/MAIN.C:40`). The frame is 520x325: the font at its own size needs 307 columns for
"Sound quality" and "Authentic" and 475 for the footer `UP DOWN  LEFT RIGHT  ENTER  ESC`, which
the demo's 320 columns cannot hold (both pinned by
`settings::tests::the_frame_is_sized_for_the_widest_row_and_the_footer`), and a 4:3 window of it
is 624x468 plus the border (`window.rs`'s tests), which fits a 600-line desktop. The screen is a
new surface and not a fidelity one: it changes no compared frame, and its reducer, its frame and
its settings are pinned by `settings.rs`'s tests.
`APPROXIMATIONS.md` **A59** registers it beside the end box, at a measured difference of 0 on
every compared frame.

**Cost if wrong.** If a run should remember its choices, it is one file written in `apply` and
read in `from_cli`; the original did not. If the screen should show for `--part` or the vault, it
is one clause of `shows_setup`. If the frame is too big for a small desktop, it is its two
constants and the layout's six; nothing else reads them. If `menu_text.rs`'s move had changed the
vault menu's pixels, `vault_menu`'s six tests would fail.

## D45 — A restart keeps what `_zplaysong`'s initialiser does not write (2026-10-24)

Decided: a mid-run restart is **the same player object taking a new module**, not a fresh player.
The shell's three restarts — block 1's `MUSIC1@0` at GLENZ (`MAIN/U2.ASM:813-818`), `MUSIC0@18`
at U2E (`:880-885`) and the loop's `MUSIC0@0` (`:929-932`) — all go through `install_music`
(`bins/secondreality/src/shell.rs`), which builds the new module's player and, for a restart,
calls `Player::restart_from(&previous, order)`: that takes the previous player's channels, its
loop counter and its `_np_zframe` over and then calls `play_song(order)` itself
(`crates/sr-audio/src/player.rs`). `play_song`'s channel clear is `Channel::initialise`: it
writes exactly the song initialiser's stores and keeps every other field. An entry install has no
old player and is a boot: `install_music` calls `play_song(order)` on the fresh player. The
interface change is two `pub` methods in `sr-audio`, `Player::restart_from(&mut self, previous:
&Player, order: u8)` and `Channel::initialise(&mut self)` (which replaces the crate-private
`Channel::reset`), plus the one call in `install_music`. `MusicSource` is not part of this
change; its signature is D43's `fn(ModuleId, PlayerOptions)`, so a restart's player is built
with the run's options before `restart_from` takes the old one's state over.

**The bytes** (`upstream/SecondReality/MAIN/STMIK.300`'s segment image, rebuilt by
`crates/sr-audio/tests/player.rs`):

- `restartmus` (`:348-397`) runs `_zstopsong` (`+0x25c5`: `[0x939] = 0`), `_zshutup` (`+0x25e3`: the
  initialiser `+0x5614`, a DMA clear, `[0x938] = 0`), the loader's `_zinitmodule` (`+0x2400`:
  `+0x56bc`, which ends in the initialiser) and `_zplaysong` (`+0x2555`: the header watch
  `+0x5424`, `+0x56bc`, the order advance).
- The initialiser's loop (`+0x5625`…`+0x5684`), its silence pass (`+0x56a6`…`+0x56b1`) and
  `+0x5781`'s bit clear store to `achannelused` (ending 0), the periods, `avibcnt`, `aorgvol`,
  `alasteff1/2`, the cell, `lastins`, `lastnote`, `alastnfo` and `m_pos`. They never store to
  `avol`, `a0volcut`, `aequ`, `aadr`'s high word, `ac2spd`, `atreon`, `atrigcnt`, `atremor`,
  `vibtretype`, `a0clearcnt` or `m_poslow`.
- Globally they write neither the loop counter `[0x90b]` nor `_np_zframe`, which the stopped
  tick does not advance. D34 measured the second in the original: `second3e` reads 9,099 at
  18000, carried across the U2E restart from PLZPART's reset. The port now carries it too. No
  test compares `_np_zframe` against a trace, and every part that reads it sets it first, so the
  value is not asserted.
- `the_song_initialiser_writes_exactly_what_initialise_writes` decodes the stores and checks
  `initialise` field by field.
- `a_boot_starts_on_the_objects_zeroed_channels` shows the object's own `_zchn`, `[0x90b]`,
  `_np_zframe` and ring are zero, so a boot is `Player::new` and `second2b` and `second1` do not
  move.

**What it fixes.** Block 1's MUSIC0 hands channel 2 over at volume 63; MUSIC1's order 0 (pattern 73)
has a `C00` on that channel at row 3, which a fresh player's "volume 0" countdown drops. With the
volume kept the break is taken, order 0 is 4 rows long and not 64, and the port publishes `1:0` at
8,087 (`a_block_one_run_is_alku_then_u2a_then_pam_in_the_boots_numbering`; `0:4` there without the
carry-over). Task 6's ≈245-frame lag from GLENZ on is gone:

| measure | before | after |
|---|---|---|
| `second1d` | +247 | +5 |
| `second1e` | +248 → +240 | +5 → −3 |
| `second1f` | +235 → +243 | −8 → 0 |
| start differences, GLENZ to JPLOGO | +230…+259 | −12…+16 |
| GLENZ's wait exit (A38) | 249 frames late | 6 |
| `RESTART_TRACES`' `second1d`/`second1e` anchor | 7,815 | 8,056 |

The block-1 music offsets are from `cargo test --release -p secondreality --test fullrun --
--nocapture`, and the anchor is derived by `block_ones_music1_restart_takes_order_0s_break`.

The U2E restart and the loop restart move by nothing: under the carry-over `second3e`, `second1f`,
`loop.rs`'s (0,1) at 8,921 and ALKU's pass-2 frames are exactly as before, because MUSIC0's orders
18 and 0 have no row whose length depends on a carried volume. Every `fullrun` rediff count is
unchanged at the moved pairings.

**What it does not carry, and why** (A55). The position ring, its pointers and the fed words, and
the tick countdown `[0xbe0]`, are kept by the object too, but what it puts in them happens during
the reload: the stopped tick feeds order `0xffff` (`+0x4408`), and how many polls a reload spends
is the session's (D42's race). The port has no reload (A53). Carrying the old ring would publish the
old song's position at every restart, and that is measured, not argued:
`a_carried_ring_publishes_what_the_stopped_previous_player_would` (sr-audio) and
`tests/loop.rs`'s `a_carried_ring_would_hand_pass_two_the_old_songs_position` show pass 2's ALKU
would read `43:34`, `dis_sync` 10, and pass its four gates at once — D42's minority session, never
the filed one. The countdown's phase is D34's measured wait.

**D31's `LOADER_TICKS`, re-checked on the corrected run: still not adopted.** The seam changes
still take both signs. Listing all eighteen seams, the change is positive at eight (ALKU→U2A +47, BEGLOGO→GLENZ +42, TUNNELI→TECHNO +9, PANICEND→MNTSCRL +8, MNTSCRL→LNS&ZOOM +18, MINVBALL→RAYSCRL +9, RAYSCRL→3DSINFLD +10, U2E→ENDLOGO +34) and
negative at ten (U2A→PAM −50, PAM→BEGLOGO −34, GLENZ→TUNNELI −12, TECHNO→PANICEND −14, LNS&ZOOM→PLZPART −13, PLZPART→MINVBALL −4, 3DSINFLD→JPLOGO −24, JPLOGO→U2E −37, ENDLOGO→CRED −8, CRED→ENDSCRL −69). A delay inserted at a handover can only push a seam later, so it cannot
produce the positive ones. The two music-pinned seams still need 6 frames and 59–68.
One constant explains neither, so D24's rule is unmet as before (A53, `reference-indices.md`
§26.8).

**Cost if wrong.** If a restart should be fresh after all, the lag returns: about 245 frames from
GLENZ on and a 64-row order 0. The change is one call in `install_music`, and `trace.rs`'s and
`fullrun.rs`'s pins fail at once. If the initialiser stores to a field the table keeps, or the other
way round, `the_song_initialiser_writes_exactly_what_initialise_writes` names it. If the ring
should be carried, A55 says what that costs (pass 2's ALKU) and what a faithful model would need: a
signed published order, and the reload's polls.

## D46 — `zspeedadjust`: both words read 0, and the shell writes the second only for Gravis and no sound (2026-10-25)

Decided: spec §13's one open question is closed. Neither `_zspeedadjust` nor `_zspeedadjust2`
moves the reference's timing, and the port models neither.

**The evidence**, four pieces, each with its command:

1. **Both read 0 in the reference's run**, at the segment's locate and while the music plays
   (`docs/fidelity/music-clock.md` §5's table, from `cargo xtask music analyse --trace
   reference/music/second5b`'s section 4).
2. **The object's image holds both as zero** — `LIDATA` runs of `0x00` at `0x8cd` and `0x8d1`
   (`cargo test -p sr-audio --test player the_speed_adjusters_are_zero_in_the_image`).
3. **The row timer adds `_zspeedadjust` and nothing else**: `+0x439f`'s `mov %cs:0x8cd,%dx; add
   %dx,%cs:0x8cf; jae; dec %ax` takes a tick off a row only on the adder's carry, so at 0 it never
   does — and the traces agree: `second5`'s speed-6 row changes are exactly six ticks apart (61 gaps
   in 534 frames where the rule says 534.4, `cargo xtask music analyse --trace
   reference/music/second5`; `music.md` §8). The same test asserts those instruction bytes.
4. **The shell writes `_zspeedadjust2` only for output modes 7 and 2**: `restartmus` passes its
   `cx` (48700, 45000: `part-sequence.md`'s restart table) through `cmp es:_zoutputmode,7; je
   @@isnosound; cmp es:_zoutputmode,2; jne @@nogus2; @@isnosound: mov es:_zspeedadjust2,cx`
   (`MAIN/U2.ASM:384-389`). The shell's own labels make mode 7 no sound and mode 2 the Gravis
   (`:725`, `:734`); the reference's card is mode 5, the Sound Blaster Pro (`:731`,
   `music-clock.md` §5).

So the answer to §13's question — "whether they affect audible timing or are GUS-specific
calibration" — is the second, with the no-sound mode beside the Gravis: the second word is
written on those two paths only, and on the reference's path both are 0. What the player does with
`_zspeedadjust2` on those two paths is not measured, because no reference runs them (the default
Gravis fails on this emulator, `CLAUDE.md`, and the captures are SB16 under the emulator's own
silent mode, D17).

**Consequence for the port.** Nothing to model: its tick arithmetic has no adjuster term, which is
the reference's arithmetic at 0 (`music.md` §8). The port's Sound Off (D44) keeps that clock — the
reference's mode-5 clock, not the original's no-sound mode with its written `_zspeedadjust2` — as
the phase's rule requires (no setting moves the published position).

**Cost if wrong.** If the original's no-sound or Gravis run ticks at another rate, a port run with
Sound Off differs from *that* run's timing and not from the reference's; no capture of either mode
exists to say so. Modelling it would be one adder in the tick, with the restart's `cx` as its
input, and would need a trace of a no-sound session first.

## D47 — Spec §8.3's event stream and §4.8's `audio render` and `event log` are retired in favour of what was built (2026-10-25)

Decided: the event stream spec §8.3 describes — palette writes, view changes and the scanline each
takes effect on, sound-clock advances, logged from both the port and the original — and the two
`xtask` subcommands §4.8 names for it, `audio render` and `event log`, are not built and will not
be. What was built answers the questions they were for:

- **Frame comparison**, `cargo xtask compare` and `cargo xtask rediff`: every filed reference frame
  against the port's run at a registered count, the index plane wherever the oracle can read one
  (§8.2; D18, D19; Phase 9 Task 5). A wrong split, pan, start address or palette write shows as
  pixels on the frame it happens in, and the register attributes each difference with the source
  and the trace (`APPROXIMATIONS.md`).
- **The original's player state as a trace**, `cargo xtask music trace` (with `analyse`, `probe`
  and `symbols`): the object's own position words, one row a frame (D26), against which the
  port's published position is held word for word (`crates/sr-audio/tests/trace.rs`; A12, A54).
  That is §8.3's "sound-clock advances", read from the unmodified original instead of logged.
- **`--audio-dump`**: the port's PCM, for listening and for the mixer's tests. The original's audio
  cannot be captured — the emulator runs silent (D17) — which is why `audio render` had nothing to
  compare against.
- **`--positions`**: a headless run's timeline, the part, frame and published position of every
  frame (Phase 9 Task 6), which the full-run comparison against the traces reads
  (`bins/secondreality/tests/fullrun.rs`).

**Why.** §8.3 needs the same events logged from the original, and the oracle exposes none of them
as events: its frame, port and memory reads are polls between frames with the guest running (D14,
D19, D20's read race), so "the line-compare write at frame 812" cannot be taken from it without
changing the emulator, which this project only reads. The frame comparison is stricter where it
applies — pixel-exact, indexed — and every difference it found was attributed without events.

**Cost if wrong.** A difference the original makes in a register that no filed frame displays — a
palette entry overwritten before the next sample, a split no compared frame reaches — goes unseen;
that is D18's and D19's recorded limit, not a new one. Building the stream later means
instrumenting the oracle's VGA writes and adding a matching log to the engine; nothing built here
would have to change for it.

## D49 — The shipped part EXEs are unpacked and read: the release's own data, not a fit to its frames (2026-10-29)

D12's closing note (2026-10-25) said the packed part EXEs in `MAIN/DATA/` would not be unpacked,
and every release-versus-tree divergence was registered against the frames with "its packed EXE
as the lead" (A6, A9–A11, A17–A20, A22–A25, A28–A31, A37). **That is reversed.** The user, watching
the demo, saw the end scroller print `[15` as text and the credits name the wrong people, both
registered divergences (A11, A10), and asked for them fixed; those fixes need the release's own
text rules, font and captions, which exist only inside the packed builds.

Decided: `sr_assets::exe::unpack` reads the twenty packed part EXEs (copied byte for byte to
`assets/MAIN/DATA/`) through every layer — PKLITE 1.14 small and large, then LZEXE 0.91 (ALKU,
ENDSCRL, PLZPART) or EXEPACK (ten parts); CRED, PAM, PANICEND and the three Pascal parts have one
layer. Each decoder is transliterated from its own stub, with the stub's tables read out of it.
**The images are verified, not assumed**: run alone under DOS, every EXE unpacks itself, exits on
`ERROR: DIS not loaded` and leaves its image in memory, and the port's image with its relocations
applied matches that memory except for the C runtime's startup writes (`tools/release_images.py`;
the counts are in `docs/fidelity/release-images.md`).

What this changes for D24: where a part's shipped build differs from the tree and the difference
is **data or code inside the shipped image**, the port now reads it from the image, which is the
release's own artifact, not a fit to its screen. D24's first branch (adopt a single measured
constant) and its "register, do not bend" branch stay for what the image cannot answer — timing,
CPU cost, the emulator's session phase. Which divergences move from "registered" to "read" is
decided part by part, each with its own measured result, in `APPROXIMATIONS.md`.

The question D12 left open — whether the staged EXEs are the ones `SECOND.EXE` packs — is answered
for ENDSCRL: the release's memory during block 5 (`tools/crtc_probe.py 5 9300 2`) holds the same
80-character font order and the `endscrol.` file name the staged image does. It is still open for
the other parts; their frames decide as before.

*(2026-10-29.)* **ENDSCRL is the first part read from its image**: its raster, `fonaorder`, pitch,
control-line rule, blank and `%` exit are the unpacked image's, and both builds' text-mode `read`
strips the CRs (`docs/fidelity/endscrl.md`, A11). The part now ends by itself, and the shell's end
path did not change: a part's own `Finished` and an ESC already retire it alike.

*(2026-10-29.)* **CRED is the second**: its 21 screens are `main`'s 21 `screenin` calls in the
unpacked image (`0x193a`-`0x1b56`), each picture the `FC FC` blob at its pushed segment and each
caption the lines at its pushed DGROUP offset; the font raster (`0x29f0`) and `fonaorder` are read
from the image too and are the tree's byte for byte (A8 stands). Its code is the tree's in every
respect the port models; the three differences it has (a `printf`, a display-enable wait in place
of a delay loop, a flip-flop reset) are recorded and not modelled (`docs/fidelity/cred.md` §9).
A10 is retired. The image does not settle the CRED→ENDSCRL chain offset: its loops are the tree's,
and the 3 to 4 frames a screen the release spends past the port's are CPU time (A11).
