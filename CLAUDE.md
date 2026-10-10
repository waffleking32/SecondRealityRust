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
dependency. See `docs/decisions.md` D10 for the LD_LIBRARY_PATH workaround on this
machine and the fallback order. The original accepts `SECOND 2..5` and `SECOND U` to
start at a chosen block.

- `xtask reference capture` — capture frames from the emulated original into
  `reference/` with a manifest (commit, CPU cycles, arguments, frame indices).
  Capture is only meaningful with CPU cycles pinned.
- `xtask compare` — run the port headless, render the same frames, report
  differing-pixel percentage, max channel delta, and a diff image.

A part is done when its frames match. Record the evidence in `PROGRESS.md`.

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
