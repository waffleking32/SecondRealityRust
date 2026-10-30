# The release's part images

The released demo's parts are the EXEs in `MAIN/DATA/`, packed. `sr_assets::exe::unpack` turns
each into the load image DOS runs (`docs/decisions.md` D49). This note records the layers and how
the images were verified.

## Layers

`cargo test -p sr-assets --test exe` pins every image (length, relocation count, FNV-1a), and
`the_layers_are_pklite_over_lzexe_or_exepack` the four representative layer stacks. `residue` is
the verification's count of differing bytes (below).

| part | outer | inner | image bytes | relocations | residue |
|---|---|---|---|---|---|
| 3DSINFLD | PKLITE large | EXEPACK | 172,368 | 54 | 54 |
| ALKU | PKLITE small | LZEXE 0.91 | 343,472 | 238 | 87 |
| BEGLOGO | PKLITE large | EXEPACK | 52,656 | 46 | 51 |
| CRED | PKLITE large | — | 426,384 | 261 | 86 |
| DDSTARS | PKLITE small | EXEPACK | 42,048 | 3 | 17 |
| ENDLOGO | PKLITE large | EXEPACK | 54,176 | 50 | 55 |
| ENDSCRL | PKLITE small | LZEXE 0.91 | 135,424 | 90 | 86 |
| GLENZ | PKLITE large | EXEPACK | 398,912 | 211 | 62 |
| JPLOGO | PKLITE large | EXEPACK | 209,408 | 48 | 55 |
| LNS&ZOOM | PKLITE large | EXEPACK | 170,208 | 167 | 122 |
| MINVBALL | PKLITE small | EXEPACK | 31,808 | 113 | 59 |
| MNTSCRL | PKLITE large | — | 145,664 | 58 | 24 |
| PAM | PKLITE large | — | 381,312 | 59 | 85 |
| PANICEND | PKLITE large | — | 141,216 | 68 | 177 |
| PLZPART | PKLITE large | LZEXE 0.91 | 223,376 | 298 | 86 |
| RAYSCRL | PKLITE large | — | 157,792 | 45 | 20 |
| START | PKLITE small | EXEPACK | 21,296 | 244 | 49 |
| STARTMUS | PKLITE small | EXEPACK | 15,328 | 185 | 55 |
| TECHNO | PKLITE large | EXEPACK | 72,336 | 180 | 57 |
| TUNNELI | PKLITE large | — | 52,928 | 29 | 20 |

U2A's and U2E's staged files are not packed and are not the release's builds (D12).

## How the decoders were found

Each decoder is read off its stub, not a format description, because the published descriptions
of PKLITE did not match these files: a decoder built from one produced an image whose Borland
copyright string had bit-buffer words spliced into it. The PKLITE 1.14 stub sits at the image's
`0x42` (file `header + 0x42`); its `add bx,NN` gives the stream's paragraph, and its Huffman
tables are at its own `cs:0x161`/`0x171` (small) and `0x207`/`0x212`/`0x21d`/`0x22d` (large). A
PKLITE image with no relocations and an entry at `CS:000E` is an LZEXE 0.91 program (`*FAB*` in its
stub); one with `RB` at `CS:000E` is EXEPACK.

## Verification

`cargo test -p sr-assets --test exe -- --ignored dump_the_unpacked_images` writes the images to
`out/release-images/`. `python3 -I tools/release_images.py --frames 250` then runs each EXE alone
in the emulator (DOS, 30,000 cycles, `assets/MAIN/DATA` mounted read-only). It dumps memory after
the EXE has unpacked itself and exited on `ERROR: DIS not loaded`, finds the image, applies its
relocations at the load segment found, and prints one line per EXE with the count of bytes that
differ (the table's `residue`). Every image is found whole, at the same load segment `001f50`,
and the largest residue is PANICEND's 177 bytes of 141,216.

The residue is the C runtime's startup writes (for example ENDSCRL's `mov cs:[0213h],dx` saving
DGROUP, at image offset `0x213`). MNTSCRL, a Pascal part, also decodes its `RIX3` picture in place
before it checks for DIS, so its count depends on how far that got by the dump frame.
