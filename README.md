# Second Reality, in Rust

A faithful port of **Second Reality**, the demo Future Crew released at Assembly '93. The
original's own source code, vendored at `upstream/SecondReality`, is treated as the
specification. The port translates its fixed-point maths, tables and bugs, plays its two Scream
Tracker modules with a transcription of its own player, and is checked frame by frame against
captures of the original running in an emulator.

## Building and running

Prebuilt packages for Windows x64, Linux x64 and macOS (Apple Silicon) are on the
[Releases](https://github.com/waffleking32/SecondRealityRust/releases) page; each one's readme
says how to start it.

To build it yourself you need a Rust toolchain (1.85 or later), CMake and a C compiler: SDL 2 is
compiled from source and linked into the binary, so no SDL library is installed or shipped. On
Debian or Ubuntu, SDL's build also wants the headers of the libraries it uses at run time:

```bash
sudo apt install cmake libx11-dev libxext-dev libxrandr-dev libxcursor-dev libxi-dev libxss-dev \
  libxfixes-dev libwayland-dev libxkbcommon-dev libegl-dev libasound2-dev libpulse-dev libudev-dev
```

```bash
cargo run --release --bin secondreality
```

The binary is self-contained: every file under `assets/` is built into it, so it runs from any
directory with nothing beside it.

With no argument the demo plays from the beginning, as `SECOND.EXE` does with none.

## The settings screen

A demo run opens on a settings screen first, in place of the original's `START.EXE` setup
questions. It offers:

| row | values | the same as |
|---|---|---|
| Sound | On / Off | `--no-audio` for Off |
| Sound quality | Authentic / Enhanced | `--sound-quality` |
| Demo looping | Disabled / Enabled | `--loop` |
| Aspect | Square / 4:3 | `--aspect square` / `--aspect 4:3` |
| Border | On / Off | `--no-border` for Off |

Up and Down choose a row, Left and Right change it, and Enter starts the demo. ESC leaves without
playing, and the demo's end box is printed. Nothing is saved: the command-line flags are the
starting values, and the screen asks again the next time. `--no-setup` skips the screen and
starts at once with the flags as given.

## Keys

- **ESC** ends the part that is playing, and the next part starts.
- **The end scroller** (ENDSCRL, the last part) ends by itself when its text runs out, as the
  released demo's does, about five and a half minutes in; ESC in it ends it early. Either way the
  demo then ends the way the original does. The music fades for 64 frames (the demo runs at
  70.086 frames a second), and the screen goes blank for 20 more. The window then closes, and the
  demo's end box is printed to the terminal you started it from. ESC during the fade or the blank
  closes the window at once.
- **Ctrl** skips the part that is playing, and the next one starts. In the original one Ctrl
  skips every part from then on. The port skips only the one you pressed it in.
- **The window's close button** leaves at any time. A demo run prints the end box however its
  window closes.

Nothing else does anything.

## Starting somewhere else

The original accepts `SECOND 1` to `SECOND 5`, and `SECOND U`. The port accepts the same as
`--block`:

```bash
cargo run --release --bin secondreality -- --block 1   # from ALKU: the whole demo (as no argument)
cargo run --release --bin secondreality -- --block 2   # from BEGLOGO
cargo run --release --bin secondreality -- --block 3   # from MNTSCRL, the landscape scroller
cargo run --release --bin secondreality -- --block 4   # from U2E
cargo run --release --bin secondreality -- --block 5   # ENDLOGO, CRED, then the end scroller
cargo run --release --bin secondreality -- --block U   # DDSTARS, the hidden part
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
skips the settings screen. Given with `--loop`, in either order, `/gravis` wins: in the original
it skips the setup menu, which is the only place looping is chosen. Any other `/` switch is
ignored, as `SECOND.EXE` ignores it.

## Sound

The music plays on the default sound card. `--no-audio` runs silent; the music still drives the
pictures. `--audio-dump <FILE>` writes the music to a 16-bit WAV file instead of playing it.

`--sound-quality authentic` is the default. It is the original's own mixer at its 20 kHz.
`--sound-quality enhanced` is the port's own interpolating renderer at 48 kHz. It is off by
default and is not part of the original. The music's timing, and therefore every picture, is the
same either way. A run with no music, such as the test card or one of the vault's effects, refuses
`enhanced`, as it refuses `--audio-dump`.

## The vault

Three unreleased development effects from the original source tree, GRID, HARD and TWIST, can be
played without touching the demo:

```bash
cargo run --release --bin secondreality -- --vault       # a menu of the three
cargo run --release --bin secondreality -- --part grid   # or hard, or twist
```

In the menu, Up and Down choose, Enter plays and ESC closes the window. In a running effect, the
first ESC ends it, and the second ESC returns to the menu. An effect that has ended by itself
needs only one ESC. The effects are silent, so `--vault` and `--part grid|hard|twist` refuse
`--sound-quality enhanced`.

## Display

The window shows the picture at **2×, by duplicating pixels** — no smoothing or interpolation:
each pixel of a 320×200 part is a 2×2 block, so the picture is 640×400, and the 320×400 and
640×400 parts are the same 640×400 on the screen.

- `--scale <N>` magnifies by any whole number instead: each 320×200 pixel becomes N×N.
- `--aspect square` (the default) keeps that exact duplication; `--aspect 4:3` stretches the
  picture into a 4:3 box as a 1993 monitor did, which resamples 200 lines into 288 and so is not
  pure duplication.
- `--no-border` leaves out the overscan border colour, a flat pad around the picture.

## Without a window

`--headless` runs the demo with no window and no sound card, and writes its frames to files
instead. It is what the frame comparisons use.

```bash
cargo run --release --bin secondreality -- --headless --part glenz --dump-dir out/glenz --frames 700
```

- `--dump-dir <DIR>` is where the frames go, and is required.
- `--frames <N>` runs N frames and writes every one; `--dump-frames <A,B,C>` writes only those
  frames and stops after the last; `--run-seconds <S>` runs for S seconds of real time and writes
  the last frame. One of the three is needed.
- `--positions <FILE>` also writes the run's timeline as CSV, one line a frame: the frame's
  index, the loop pass, the part on screen, and the music's order and row. It needs `--frames` or
  `--dump-frames`.

A dump is always one file pixel per picture pixel, whatever `--scale` and `--aspect` say.

## How faithful it is

Every part of the demo, and each vault effect, is compared against frames captured from the
original, and every known difference is measured and recorded:

- `docs/fidelity/APPROXIMATIONS.md`: every place the port differs from the original, with the
  measured difference and why.
- `docs/fidelity/bugs.md`: the original's own bugs, which the port keeps.
- `docs/fidelity/`: one note per part, with what the original did and what the port does.
- `docs/decisions.md`: the decisions that shaped the port.
- `docs/design.md`: how the port is built.

`CLAUDE.md` is the working guide for developing the port: capturing reference frames, comparing
and the checks to run before committing.

## Licence

The port is MIT licensed (see `LICENSE`). The vendored original source in `upstream/` and the
demo's media in `assets/` are public domain under the Unlicense.
