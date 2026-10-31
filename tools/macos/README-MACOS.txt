Second Reality (Rust port) for macOS (Apple Silicon)
====================================================

Double-click SecondReality.command to play. It starts the demo in this folder, because the demo
reads its files from assets/ relative to the folder it is started in. From a terminal:

  cd SecondReality
  ./secondreality                      the demo, opening on the settings screen
  ./secondreality --no-setup           straight into the demo
  ./secondreality --loop               loop mode
  ./secondreality --sound-quality enhanced
  ./secondreality --vault              the three unreleased effects

README.md has the rest: the keys, the blocks, the hidden part and every flag. When the demo
ends it prints the original's end box in the terminal.

The program is not signed, so macOS refuses it the first time. Either right-click
SecondReality.command and choose Open, or clear the download's quarantine flag once:

  xattr -dr com.apple.quarantine SecondReality

SDL 2 (https://libsdl.org) is built in, under the zlib licence in SDL2-LICENSE.txt. The port is
MIT (LICENSE); the original demo's source and media are public domain under the Unlicense.
