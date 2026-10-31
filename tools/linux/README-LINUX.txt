Second Reality (Rust port) for Linux x64
========================================

The demo's files are built into the program, so it runs from any folder. From a terminal in
this folder (or give the program's full path from any other folder):

  ./secondreality                      the demo, opening on the settings screen
  ./secondreality --no-setup           straight into the demo
  ./secondreality --loop               loop mode
  ./secondreality --sound-quality enhanced
  ./secondreality --vault              the three unreleased effects

README.md has the rest: the keys, the blocks, the hidden part and every flag. When the demo
ends it prints the original's end box in the terminal.

The binary needs glibc 2.35 or later (Ubuntu 22.04, Debian 12, Fedora 36 and newer). SDL 2
(https://libsdl.org) is built in, under the zlib licence in SDL2-LICENSE.txt; it uses the
system's X11 or Wayland and ALSA, PulseAudio or PipeWire libraries. The port is MIT (LICENSE);
the original demo's source and media are public domain under the Unlicense.
