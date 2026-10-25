# Second Reality, in Rust

A faithful port of **Second Reality**, the demo Future Crew released at Assembly '93. The
original's own source code, vendored at `upstream/SecondReality`, is treated as the
specification. The port translates its fixed-point maths, tables and bugs, plays its two Scream
Tracker modules with a transcription of its own player, and is checked frame by frame against
captures of the original running in an emulator.

## Building and running

You need a Rust toolchain (1.85 or later) and the system **SDL2** library: the `sdl2` crate links
the system library, so it is not built for you. On Debian or Ubuntu, install `libsdl2-dev`.

```bash
cargo run --release --bin secondreality
```

Run it **from the repository root**. The demo's files are read relative to the working directory,
for example `assets/ALKU/FONA.UH`, so the binary started anywhere else cannot find them.

With no argument the demo plays from the beginning, as `SECOND.EXE` does with none.

## The settings screen

A demo run opens on a settings screen first, in place of the original's `START.EXE` setup
questions. It offers:

| row | values | the same as |
|---|---|---|
| Sound | On / Off | `--no-audio` for Off |
| Sound quality | Authentic / Enhanced | `--sound-quality` |
| Demo looping | Disabled / Enabled | `--loop` |
| Aspect | 4:3 / Square | `--aspect 4:3` / `--aspect square` |
| Border | On / Off | `--no-border` for Off |

Up and Down choose a row, Left and Right change it, and Enter starts the demo. ESC leaves without
playing, and the demo's end box is printed. Nothing is saved: the command-line flags are the
starting values, and the screen asks again the next time. `--no-setup` skips the screen and
starts at once with the flags as given.

## Keys

- **ESC** ends the part that is playing, and the next part starts.
- **ESC in the end scroller** (ENDSCRL, the last part) ends the demo the way the original does.
  The music fades for 64 frames (the demo runs at 70.086 frames a second), and the screen goes
  blank for 20 more. The window then closes, and the demo's end box is printed to the terminal
  you started it from. ESC during the fade or the blank closes the window at once.
- **Ctrl** skips the part that is playing, and the next one starts. In the original one Ctrl
  skips every part from then on. The port skips only the one you pressed it in.
- **The window's close button** leaves at any time. A demo run prints the end box however its
  window closes.

Nothing else does anything.

## Starting somewhere else

The original accepts `SECOND 1` to `SECOND 5`, and `SECOND U`. The port accepts the same as
`--block`:

```bash
cargo run --release --bin secondreality -- --block 1   # from the beginning (the same as no argument)
cargo run --release --bin secondreality -- --block 2   # from the title screen
cargo run --release --bin secondreality -- --block 3   # from the landscape scroller
cargo run --release --bin secondreality -- --block 4   # from the vector part
cargo run --release --bin secondreality -- --block 5   # from the end credits
cargo run --release --bin secondreality -- --block U   # the hidden part
```

`--block U` plays **DDSTARS**, a part that is in the demo's code but that `README.1ST` never
mentions (`docs/fidelity/part-sequence.md` §4). Blocks 1 to 5 all end in the end scroller.

One part can be played on its own with `--part <name>`, for example `--part glenz`. Such a run
holds its last screen until ESC closes the window. An unknown name is refused, and the error lists
the names.

## Loop mode

```bash
cargo run --release --bin secondreality -- --loop
```

`--loop` is the setup's "Demo looping: Enabled". The end scroller is left out, and after the
credits the music fades and the demo starts again from the beginning. It loops until you close
the window. ESC still ends only the part that is playing.

`/gravis` is the original's own switch. It plays the demo once without the end scroller and
skips the settings screen. Any other `/` switch is ignored, as `SECOND.EXE` ignores it.

## Sound

The music plays on the default sound card. `--no-audio` runs silent; the music still drives the
pictures. `--audio-dump <FILE>` writes the music to a 16-bit WAV file instead of playing it.

`--sound-quality authentic` is the default. It is the original's own mixer at its 20 kHz.
`--sound-quality enhanced` is the port's own interpolating renderer at 48 kHz. It is off by
default and is not part of the original. The music's timing, and therefore every picture, is the
same either way.

## The vault

Three unreleased development effects from the original source tree, GRID, HARD and TWIST, can be
played without touching the demo:

```bash
cargo run --release --bin secondreality -- --vault       # a menu of the three
cargo run --release --bin secondreality -- --part grid   # or hard, or twist
```

In the menu, Up and Down choose, Enter plays and ESC closes the window. In a running effect, the
first ESC ends it, and the second ESC returns to the menu. An effect that has ended by itself
needs only one ESC. The effects are silent, so `--vault` refuses `--sound-quality enhanced`.

## Display

- `--scale <N>` magnifies the window N times.
- `--aspect 4:3` (the default) shows the picture as a 1993 monitor did; `--aspect square` shows
  square pixels.
- `--no-border` leaves out the overscan border colour.

## How faithful it is

Every part of the demo, and each vault effect, is compared against frames captured from the
original, and every known difference is measured and recorded:

- `docs/fidelity/APPROXIMATIONS.md`: every place the port differs from the original, with the
  measured difference and why.
- `docs/fidelity/bugs.md`: the original's own bugs, which the port keeps.
- `docs/fidelity/`: one note per part, with what the original did and what the port does.
- `PROGRESS.md`: the evidence, phase by phase.
- `docs/decisions.md`: the decisions that shaped the port.

`CLAUDE.md` is the working guide for developing the port: running headless, capturing reference
frames and comparing.

## Licence

The port is MIT licensed (see `LICENSE`). The vendored original source in `upstream/` and the
demo's media in `assets/` are public domain under the Unlicense.
