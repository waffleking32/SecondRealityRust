# The release's part images

The released demo's parts are the EXEs in `MAIN/DATA/`, packed. `sr_assets::exe::unpack` turns
each into the load image DOS runs (`docs/decisions.md` D49). This note records the layers and how
the images were verified.

## Layers

`cargo test -p sr-assets --test exe` pins every image (length, relocation count, FNV-1a), and
`the_layers_are_pklite_over_lzexe_or_exepack` the four representative layer stacks.

| part | outer | inner | image bytes | relocations |
|---|---|---|---|---|
| 3DSINFLD | PKLITE large | EXEPACK | 172,368 | 54 |
| ALKU | PKLITE small | LZEXE 0.91 | 343,472 | 238 |
| BEGLOGO | PKLITE large | EXEPACK | 52,656 | 46 |
| CRED | PKLITE large | — | 426,384 | 261 |
| DDSTARS | PKLITE small | EXEPACK | 42,048 | 3 |
| ENDLOGO | PKLITE large | EXEPACK | 54,176 | 50 |
| ENDSCRL | PKLITE small | LZEXE 0.91 | 135,424 | 90 |
| GLENZ | PKLITE large | EXEPACK | 398,912 | 211 |
| JPLOGO | PKLITE large | EXEPACK | 209,408 | 48 |
| LNS&ZOOM | PKLITE large | EXEPACK | 170,208 | 167 |
| MINVBALL | PKLITE small | EXEPACK | 31,808 | 113 |
| MNTSCRL | PKLITE large | — | 145,664 | 58 |
| PAM | PKLITE large | — | 381,312 | 59 |
| PANICEND | PKLITE large | — | 141,216 | 68 |
| PLZPART | PKLITE large | LZEXE 0.91 | 223,376 | 298 |
| RAYSCRL | PKLITE large | — | 157,792 | 45 |
| START | PKLITE small | EXEPACK | 21,296 | 244 |
| STARTMUS | PKLITE small | EXEPACK | 15,328 | 185 |
| TECHNO | PKLITE large | EXEPACK | 72,336 | 180 |
| TUNNELI | PKLITE large | — | 52,928 | 29 |

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
`out/release-images/`; `python3 -I tools/release_images.py` then runs each EXE alone in the
emulator (DOS, 30,000 cycles, `assets/MAIN/DATA` mounted read-only), dumps memory after it has
unpacked itself and exited on `ERROR: DIS not loaded`, finds the image, applies its relocations
at the load segment found and counts the bytes that differ:

```text
3DSINFLD   load 001f50 image  172368 compared  172368 differ     54
ALKU       load 001f50 image  343472 compared  343472 differ     87
BEGLOGO    load 001f50 image   52656 compared   52656 differ     51
CRED       load 001f50 image  426384 compared  426384 differ     86
DDSTARS    load 001f50 image   42048 compared   42048 differ     17
ENDLOGO    load 001f50 image   54176 compared   54176 differ     55
ENDSCRL    load 001f50 image  135424 compared  135424 differ     86
GLENZ      load 001f50 image  398912 compared  398912 differ     62
JPLOGO     load 001f50 image  209408 compared  209408 differ     55
LNS&ZOOM   load 001f50 image  170208 compared  170208 differ    122
MINVBALL   load 001f50 image   31808 compared   31808 differ     59
MNTSCRL    load 001f50 image  145664 compared  145664 differ     24
PAM        load 001f50 image  381312 compared  381312 differ     85
PANICEND   load 001f50 image  141216 compared  141216 differ    177
PLZPART    load 001f50 image  223376 compared  223376 differ     86
RAYSCRL    load 001f50 image  157792 compared  157792 differ     20
START      load 001f50 image   21296 compared   21296 differ     49
STARTMUS   load 001f50 image   15328 compared   15328 differ     55
TECHNO     load 001f50 image   72336 compared   72336 differ     57
TUNNELI    load 001f50 image   52928 compared   52928 differ     20
```

(2026-10-29, `--frames 250`.) Every image is found whole, at the same load segment, and the
largest residue is PANICEND's 177 bytes of 141,216.

The residue is the C runtime's startup writes (for example ENDSCRL's `mov cs:[0213h],dx` saving
DGROUP, at image offset `0x213`); MNTSCRL, a Pascal part, also decodes its `RIX3` picture in place
before it checks for DIS, so its count depends on how far that got by the dump frame.
