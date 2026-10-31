Second Reality (Rust port) for Windows x64
==========================================

Double-click secondreality.exe, or run it from a terminal in any folder: the demo's files are
built into the program.

  secondreality.exe                    the demo, opening on the settings screen
  secondreality.exe --no-setup         straight into the demo
  secondreality.exe --loop             loop mode
  secondreality.exe --sound-quality enhanced
  secondreality.exe --vault            the three unreleased effects

README.md has the rest: the keys, the blocks, the hidden part and every flag.

When the demo ends it prints the original's end box in the console. A double-clicked run closes
its console when it exits, so start it from Windows Terminal or PowerShell to read the box. The
box is drawn with ANSI colour codes; the old console host may show those as raw characters.

SDL 2 (https://libsdl.org) is built into secondreality.exe, under the zlib licence in
SDL2-LICENSE.txt. The port is MIT (LICENSE); the original demo's source and media are public
domain under the Unlicense.
