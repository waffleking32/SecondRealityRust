# Working in this repository

A faithful Rust port of **Second Reality** (Future Crew, 1993), checked frame by frame against
the original running in an emulator. `README.md` is the user's view (flags, keys);
`docs/design.md` is the architecture (a citation `design §N` means its §N); `docs/decisions.md` has
the rulings (D-numbers). Before touching a verified part, read its note in `docs/fidelity/`,
`APPROXIMATIONS.md` (A-numbers) and `bugs.md`. Project history is in git
(`git show phase-9:PROGRESS.md`).

## Layout

```
crates/sr-engine/     the software VGA: surface, view, palette, copper, tick, compose, present,
                      the Part trait, the Music seam, Machine and Engine, the test card
                      (zero runtime dependencies)
crates/sr-audio/      the S3M player and STMIK's mixer (zero runtime dependencies)
crates/sr-assets/     decoders for the original formats (depends on sr-engine only)
bins/secondreality/   shell/ (the run), cli.rs, window.rs (SDL), headless.rs, audio.rs,
                      menu/ (settings, vault), endscreen.rs, parts.rs (the registry), parts/
xtask/                reference capture/verify, compare, rediff, frames, assets, music
tools/                emulator probes, measurement scripts, the vault capture script, the packages'
                      readmes and the macOS launcher
.github/workflows/    build.yml: Windows, Linux and macOS packages (manual, or a Release on a v* tag);
                      wiki.yml: mirrors wiki/ to the GitHub wiki
wiki/                 the GitHub wiki's pages and pictures
assets/               the curated original media; assets/PROVENANCE.md is parsed by xtask
reference/            captured frames and manifests: input-only
upstream/             the original source (submodule): input-only
out/                  scratch, git-ignored
```

## The upstream source

The original 1993 tree is vendored as a pinned git submodule at `upstream/SecondReality`
(`mtuomi/SecondReality`, public domain / Unlicense). **When porting anything, read the original
first.** Quote the file and line in code comments and commit messages. If the submodule is
missing, run `git submodule update --init`.

One directory per demo part (`ALKU`, `GLENZ`, `TECHNO`, …); `MAIN/` is the shell, loader, copper
and music player; `DIS/` is the Demo Interrupt Server; `VISU/` is the 3D engine; `GRID`, `HARD`
and `TWIST` are unreleased development effects.

## Non-negotiables

- **Port the algorithm, not a resemblance.** Translate the original's fixed-point maths, tables
  and rounding. It is usually less work than reinventing the effect, and it is the only way frame
  diffs pass.
- **Do not stub out the copper.** Registering per-frame callbacks is required; two existing ports
  skipped it and lost visible effects.
- **Preserve the original's bugs.** See `docs/fidelity/bugs.md`. A "correct" implementation of the
  plasma looks wrong.
- **Read assets in their original formats.** No LBM→PNG, no S3M→OGG. Conversion destroys palette precision and the music's sync data.
- **6-bit palette internally**, widened only at presentation.
- **Never spin-wait.** Fixed 70.086 Hz logic tick, decoupled from display refresh.
- **Approximations need justification.** Each one goes in `docs/fidelity/APPROXIMATIONS.md` with
  the measured difference.

## Running it

Run from the repository root: assets are read relative to the working directory. Flags and keys
for players are in `README.md` and `secondreality --help`. The developer's view:

```bash
cargo run --release --bin secondreality -- --part glenz        # one part, by registry name
cargo run --release --bin secondreality -- --part testcard     # also grid, hard, twist
cargo run --release --bin secondreality -- --headless --block 2 \
    --dump-dir out/b2 --dump-frames 430,470,560,900            # BEGLOGO's reference indices
cargo run --release --bin secondreality -- --headless --block 1 --dump-dir out/f \
    --dump-frames 48000 --positions out/f/pos.csv              # index,pass,part,order,row a tick
cargo run --release --bin secondreality -- --audio-dump out/music.wav
```

- A block run's frame indices are the **reference's own numbering**: boot-relative emulator frames,
  the space everything under `reference/` is measured in (D21). A `--part` run of a part with no
  boot anchor numbers its own frames from zero, and the title bar says so.
- ESC, Ctrl and the exit drain: `shell/sequence.rs`, A58, D21, `Part::drains_after_exit` (D35); where a
  part polls the exit flag is in its module doc. The post-PANICEND cue wait is D38, the run's end
  D41, the settings screen D44, loop mode D42 (its seam A61), enhanced audio D43 (it never moves
  `(order, row)`, A60), the 2× display D48.
- The vault's references are captured by `tools/vault_capture.sh` (D39; `grid.md`, `hard.md`,
  `twist.md`).

## Verification

`dosbox-automation` is the oracle, behind a trait (D10 has the fallbacks, D14 the API as it
behaves).

### Capturing reference frames

`cargo xtask reference capture` drives the emulated original and writes frames plus a manifest
under `reference/`. `SECOND.EXE` loads `REALITY.FC` from the current directory, so both sit in
`reference/original/`, the default.

Five things the command needs, each of which cost time to discover:

1. **`LD_LIBRARY_PATH`** must include the emulator's build dependencies, because the binary has no
   RPATH: `$SR_DOSBOX/build/release-linux/_deps/{sdl3,sdl3_image,mt32emu,fluidsynth,zlib-ng,iir1}-build`,
   with `SR_DOSBOX` pointing at the `dosbox-automation` checkout (default: `../dosbox-automation`,
   beside this repository).
2. **A scratch `XDG_CONFIG_HOME`** whose *primary* config carries `mount_allowed_bases`.
   `--noprimaryconf` alone cannot mount the reference directory; `xtask` writes one into a temp
   directory.
3. **The setup screen.** `SECOND 2` does not skip it, and the default Gravis card fails on this
   emulator, so `--keys "KBD_left KBD_enter"` selects an SB16. Under `--boot autoexec` (D17) the
   guest types them (`AUTOTYPE` in `[autoexec]`) and the capture waits for the first graphics
   frame; keys that leave the setup screen up are refused.
4. **Cycles through the config, never the REST PUT.** `cycles=` at startup forces legacy mode and
   the PUT returns 409. A capture without pinned cycles is **refused**: it is not reproducible.
5. **Frame indices are boot-relative.** Frame 0 is the emulator's first rendered frame. Under
   `--boot autoexec` the setup screen is up at 16, the title screen's first graphics frame is 404
   and the first 320×200 part follows around 1160 (D17).

**Support frames.** The reference holds keyframes only. The capture compared against it holds
`K-1, K, K+1` around every keyframe that moves and nothing around one that holds. Indices closer
than about 25 frames are grabbed in one batch and read afterwards, so keep a distant index about
25·*k* frames behind a batch of *k* (D19; `reference-indices.md` §1.3, §1.4).

### Comparing

`cargo xtask compare` reports the differing-pixel percentage, the max channel delta and a diff
image per frame.

- **Exit codes**: `0` every frame identical, `1` frames differ, `2` it could not compare (geometry
  mismatch, a reference index with no rendered frame within one of it, a malformed manifest, a
  missing file). Scripts rely on "they differ" versus "it could not tell".
- **Indexed comparison** is used whenever both sides have `.idx`; it is stricter than RGB and makes
  the DAC widening irrelevant. Otherwise it compares RGB.
- **Pairing is by manifest index**, and a rendered frame may sit one frame either side of the index
  it answers for, because the oracle's animation phase is not reproducible across sessions (D17).
  The offset is reported; anything further is refused (D18).
- **`reference/` is input-only.** Output defaults to `out/compare`; an `--out` inside a reference
  tree is refused.

### Re-diffing everything, and the full run

- **`cargo xtask rediff`** re-renders every filed frame under `reference/parts` with the release
  binary and checks it against the count `xtask/src/rediff/table.rs` registers (`exact`, or the A-
  or D-entry that explains it). Exit **0**: every frame at its count; **1**: a count moved, either
  way; **2**: it could not compare. The report goes to `out/rediff/`. Run it after any change that
  could move a picture, and re-register a count only with the measurement that moved it.
- **The same table is a cargo test.** `bins/secondreality/tests/reference_frames.rs` runs the binary
  once per distinct set of run arguments and holds every frame to its registered count, the way
  rediff compares it. Its block-1 group is `#[cfg_attr(debug_assertions, ignore)]`; the rest run in
  both profiles. Run it all as `cargo test --release -p secondreality --test reference_frames`.
- **The full run is release-only.** `bins/secondreality/tests/fullrun.rs` checks block 1 end to end
  against `reference/parts/fullrun`: part order, seams and music against the traces. Every test in
  it is `#[cfg_attr(debug_assertions, ignore)]`. Run them as
  `cargo test --release -p secondreality --test fullrun`, **without** `-- --ignored`, which in
  release runs only the genuinely ignored tests. `tests/loop.rs` (the loop seam) runs in both
  profiles; `-- --nocapture` prints its figures.
- **The binary's other integration tests** check what rediff cannot see: `tests/cli.rs` (exit
  codes, refusals, the audio dump, no sound card), `tests/parts.rs` (one module a `--part` run:
  numbering, music, D19's holds and moves, a part's end) and `tests/blocks.rs` (each `--block`
  chain's hand-overs and music, and the parts only a block reaches). They share
  `tests/common/mod.rs`, which runs a dump once per argument list and reads it into memory.

### Caveats worth knowing before they bite

- **A view's line stride defaults to the surface's row stride** (`y_line_offset: None`). Set it
  only to pack rows tighter (CRTC 0x13). Setting it to the view's width on a wider surface shears
  the picture with no error.
- **A split's lower view reads from origin 0**, with panning cleared, so leave its origin and pan
  alone. `Split::scanline` is the first line *below* the split; the line-compare register names the
  last line above it, so add one (D16).
- **`View::start_addr` is a `u32` pixel address**, not the CRTC's 16-bit register. `set_origin`
  writes it and zeroes `x_offset`, which is an offset within a line.
- **Below a split the scanline count restarts**: `View::effective_at` returns the line within the
  view.
- **SDL names packed formats by bit significance.** For bytes with red first, as `present` writes,
  the enum is `PixelFormatEnum::RGBA32`, not `RGBA8888`; the wrong one turns the screen red and no
  frame test notices. Pixels that reach SDL need a test like
  `window::tests::the_texture_format_reads_the_presenters_bytes_in_order`.

## Who owns which explanation

- **The code** (module `//!` and item docs) owns *how the port mirrors the original*, with
  `FILE.EXT:line` citations. **The part's note** `docs/fidelity/<part>.md` owns *how we know it
  matches*: the boot↔tick mapping, the music cue, the keyframes and their measurements, long
  investigations; it points at the module by item name. **`APPROXIMATIONS.md`** owns each
  approximation's argument, **`bugs.md`** the preserved bugs, **`decisions.md`** the rulings.
- **The wiki** (`wiki/`, mirrored to the GitHub wiki by `.github/workflows/wiki.yml` on a push to
  main) owns the *reader's explanation*: how each effect works, for a curious programmer. It links
  to the module and the part's note rather than restating them. Its pictures are
  `cargo xtask frames stills` (the table in `xtask/src/stills.rs`); a page that changes a picture
  changes that table and regenerates.
- A- and D-numbers, and cited § numbers, never change; a retired entry is a one-line tombstone.
  Write the current fact in the present tense (history is in git), and cite this repo's files by
  item or test name, not line number.
- **Every number a doc states is reproducible** (D25): by `cargo xtask rediff`, by a named test that
  stays in the tree, or by a named command. When a reference frame disagrees with a faithful port,
  **D24**'s procedure decides: the vendored tree is a development snapshot, not the release.

## Before committing

`cargo test --workspace` (0 failed, 0 warnings); `cargo clippy --workspace --all-targets --
-D warnings` in debug and in `--release`; `cargo xtask rediff` exits 0; and the release `fullrun`
when the shell, the music or a part's timing moves. Do not run `cargo fmt` across the workspace (it
is not rustfmt-clean); format only the lines you write.

## Licence

MIT (see `LICENSE`). The vendored original source and the media in `assets/` are public domain
under the Unlicense. Set `license = "MIT"` in any new crate manifest.

## Open items

- **Hand checks at a real window** (no test drives keys in one): the settings screen's row cycling
  and `>` marker; a live Aspect or Border change resizing the window; Enter with Sound Off, with
  Demo looping Enabled and with Sound quality Enhanced; ESC on the settings screen printing the end
  box; ESC from ENDSCRL ending a block run (fade, blank, close, end box); a windowed `--loop` run
  crossing into pass 2, and the close button then printing the end box.
- **The emulator's silence flag.** `emulator_command` (`xtask/src/oracle/dosbox.rs`) passes
  `--set mixer_nosound=true`, but the emulator's setting is `[mixer] nosound`
  (`dosbox-automation`'s `src/audio/mixer.cpp`), so captures are silent only through
  `SDL_AUDIODRIVER=dummy` (D17). Fixing the name needs D17's determinism re-measured before any
  reference is re-captured.
- **The end box's ANSI colour codes** print raw in the old Windows console host
  (`README-WINDOWS.txt`); enabling virtual-terminal processing would fix it.
