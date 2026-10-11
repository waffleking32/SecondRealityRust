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
   on this emulator, so the capture sends `KBD_left` then `KBD_enter` to select an SB16
   before the demo starts.
4. **Cycles through the config, never the REST PUT.** Setting `cycles=` at startup forces
   legacy mode and the PUT then returns 409. A capture without pinned cycles is **refused**
   rather than produced: an unpinned capture is not reproducible and must never be mistaken
   for a reference.
5. **Frame indices are boot-relative, and the demo does not start at frame 0.** Frame 0 is
   the emulator's first rendered frame; `SECOND.EXE` is not reached until around counter
   1657. The title screen is near frame 1700, and frames below that are DOS shell.

### Comparing

`cargo xtask compare` diffs a rendered frame set against a capture and reports
differing-pixel percentage, max channel delta, and a diff image per frame.

- **Exit codes matter**: `0` every frame identical, `1` frames differ, `2` the tool could
  not compare (geometry or frame-count mismatch, malformed manifest, missing file). Scripts
  rely on the difference between "they differ" and "it could not tell".
- **Indexed comparison is preferred** and used whenever both sides have `.idx`. It is
  stricter than RGB and makes the DAC widening rule irrelevant. The fallback is RGB.
- Frames are paired by their manifest **index**, not by position.
- **`reference/` is input-only.** Output defaults to `out/compare` and an explicit `--out`
  inside a reference tree is refused. Do not write into a capture directory.

A part is done when its frames match. Record the evidence in `PROGRESS.md`.

### Geometry caveats worth knowing before they bite

- **A view's `y_line_offset` is the surface stride, not the view width.** `View::new(w, h)`
  defaults it to `w`, which is correct only when the surface *is* `w` wide. A 320-wide view
  on the 640-wide surface must set it to 640, or it reads a different layout than the part
  wrote.
- **`View`'s origin is `u16` and the surface is 256,000 pixels** — see the open item in
  `docs/decisions.md` D16. It will need settling when ENDSCRL lands.
- **Below a split, the scanline count restarts.** Line compare resets the CRTC's counters,
  so `View::effective_at` returns the scanline *within* the view.

## Fidelity notes

`docs/fidelity/` holds one note per part, written as the part is ported: what the
original did, what the port does, which techniques were translated literally and which
were substituted, and the measured difference. Write the note in the same commit as
the part.

## Phase completion

A phase is done when its exit criteria are met and:

- all tests pass,
- frame diffs are recorded in `PROGRESS.md`,
- `docs/decisions.md` is updated with anything that changed the approach,
- `git tag phase-N` is applied.
