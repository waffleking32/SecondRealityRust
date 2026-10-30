# BEGLOGO — the Future Crew logo, the white, and the 129-step fade

**Sources.** `BEG/BEG.C` (87 lines), `BEG/ASM.ASM` (118: `lineblit`, `setpalarea`) and
`BEG/READP.C` (64: the Origin RLE row reader); asset `assets/BEG/SRTITLE.UP` (§1). Slot 4 of
`part-sequence.md` §1, `SECOND 2`'s first part; 320×400, 128,000 pixels. **Port.**
`bins/secondreality/src/parts/beglogo.rs` (`"beglogo"`); its module doc holds the acts and the
build question's summary. Integration test `bins/secondreality/tests/beglogo.rs`. **Run.** Anchor
**404** (`shell::FIRST_GRAPHICS_FRAME`, `boot_anchor("beglogo")`). **Music.** None: `BEG/` has no
music gate (§1). Reference `reference/parts/beglogo/`: 430, 470, 560, 900
(`reference-indices.md` §3).

## 1. Which build shipped: `BEG/`

`ENDPIC/MAKEFILE:22` also stages a `beg.exe` as `beglogo.exe`, so two directories build part 4.
`ENDPIC/BEG.C` sets its own video mode (`:18-20`) and waits on the music (`dis_muscode(0xf0)`,
`:45-49`); `BEG/BEG.C` calls `dis_partstart`, `dis_waitb` and nothing else. Three measurements
settle it without unpacking anything:

1. **The held frame's palette.** 900 is past the fade, so the DAC in force is the picture's own,
   and `reference/parts/beglogo/manifest.json` records the 57 entries that frame's indices probe.
   `BEG/SRTITLE.UP`'s palette (bytes 16..784, copied verbatim by `readp`, `BEG/READP.C:17`) differs
   in **0 of 57**; `ENDPIC/SRTITLE.U`'s in **57 of 57** (its entry 1 is `(1,2,3)` where the
   reference has `(63,63,63)`).
2. **The held frame's picture.** `BEG/SRTITLE.UP` decodes to `frame_000900.idx` with **0 of
   128,000** pixels different; `ENDPIC/SRTITLE.U` is a different picture (126,261 differ).
3. **The staged EXE.** `MAIN/DATA/BEGLOGO.EXE` is 30,874 bytes and PKLITE-packed
   (`part-sequence.md` §1). `BEG/BEG.EXE` is 50,830 bytes and `ENDPIC/BEG.EXE` 13,286; no packer
   grows a binary by 2.3×.

So the picture is `BEG/SRTITLE.UP` with its own palette, the screen opens white
(`BEG/BEG.C:20-21`), and BEGLOGO has no music gate (D8).

## 2. Timeline

| index | tick | source | what the screen is |
|---|---|---|---|
| 404-435 | 0-31 | `:17-24` | the white field (64 KB of index 15) through the shell's DAC; 32 vertical blanks |
| 436 | 32 | `:25-78` | the white DAC (0-254 white, 255 black), the CRTC/AC writes, the picture blitted: still white |
| 437-445 | 33-41 | — | white; nine frames the port does not model (§3, A5) |
| 446-574 | 42-170 | `:80-85` | the ramp, `c = index − 446`, one step a vertical blank |
| 575 | 171 | `:86` | the picture's palette once more; `main()` returns (`:87`) |
| 576-900 | — | | the hold: the loader and GLENZ's music wait (`reference-indices.md` §3.5) |

The screen never moves after `:78`; everything from 446 to 575 is the DAC. In block 2 GLENZ's
first tick is 576 (172 ticks); in block 1 BEGLOGO runs 7,885-8,056 (`FULLRUN_SEAMS`, A53).

## 3. The fade's first index is 446, by matching the reference bit for bit

`FADE_FIRST_INDEX` is the reference index of step `c = 0`.
`the_fade_constants_are_derived_by_matching_the_reference_bit_for_bit` runs the source's arithmetic
over `c = 0…128`, widens with the engine's `(c*259+33)>>6`, renders all 320×400 pixels and asks
which `c` reproduces each keyframe exactly:

| frame | `c` values whose 320×400 RGB is that frame, byte for byte |
|---|---|
| 470 | **[24]**, unique |
| 560 | **[114]**, unique |
| 900 | [126, 127, 128]: the ramp has converged |
| 430 | all 129: the white field, before the ramp |

470 − 24 = 560 − 114 = **446**: two keyframes 90 frames apart, each unique, agree. A line fitted
through `reference-indices.md` §3.3's per-frame mean differences (23.815 at 440 down to 0.156 at
570) puts the white's last frame near 441 instead: the mean of a truncated integer ramp is a
staircase, and a five-frame fit cannot tell a whole step from the truncation inside one. 900 says
only where the ramp ended; 430 pins the surface (the white field, no picture yet), not the
constant, because entry 15 is white at every `c`.

`FADE_FIRST_TICK` = 446 − 404 = **42**. **33 of those 42 ticks are the source's**: 32 vertical
blanks (`:24`) and the tick `:25-78` runs on. **The other nine are not modelled, and their cause is
unidentified** (A5). Two causes are ruled out: the EXE load (`reference-indices.md` §3.3 files 410,
429, 430 and 431 as one byte-identical white, and `:20-21`'s memsets are what make it white, so the
part was running by 410), and `dis_partstart()`, a version check that returns at once
(`DIS/DISC.ASM:51-64`). The port draws the white field through all 42 ticks, which the reference
shows at every index it holds in the window; 446 does not depend on the split.

## 4. What the port does, act by act

- **`init`** does `:18-21` (one `clear` to 15: the plane interleave folds away in the chunky
  model), sets the 320×400 view (`:42-53`: display start 0, CRTC 9's double-scan and max-scan
  fields cleared) with `y_line_offset = None` (§5) and `border = Some(255)`, and reads the picture
  with `sr_assets::load_path("assets/BEG/SRTITLE.UP")` (one call for what `readp` reads row by
  row; refused unless 320×400).
- **The blit and the white DAC are on tick 32** (`LEAD_IN_TICKS`, index 436), not in `init`:
  `:24`'s waits come before `:25-78`. The reference makes this load-bearing: `frame_000430.idx` is
  128,000 bytes of 15, and `compare` prefers the indexed path, so a port that blitted in `init`
  would fail 430 on its index plane. The first 32 ticks run over the shell's DAC, whose entry 15 is
  white; a part that cleared the DAC would render 430 black.
- **`frame` is the fade**: one `c` a tick from tick 42, then `:86`'s upload and `Finished`.
- **Folded registers.** CRTC timing is not written by `BEG.C`. Each AC write ends with `0x20` to
  `0x3c0` (`:59-60`, `:69-70`): PAS = 1, the state in which the palette is displayed at all, which
  the engine always is. AC 0x11 is written 255, 0, 255 (`:37`, `:57`, `:67`); the port states the
  last (`border = Some(255)`). The only thing left out is A5's timing.

## 5. The row stride: `y_line_offset` unset

The picture is written through `Surface::set` at the surface's own stride, and the 320-wide view
with no line offset reads exactly that half of the 640-wide surface. `BEG.C` never writes CRTC
0x13: it inherits the tweak's 80-byte row, the stride `lineblit(vram+(unsigned)y*80U,rowbuf)` draws
at. In the port, block 1's PAM leaves `Some(320)`, which would read every second scanline from the
picture's right margin, so `init` writes `None` (what a run starting here already has;
`the_picture_is_read_at_its_own_row_stride_whatever_the_part_before_left`). Block 1's 7930 is the
original's byte for byte (`tests/fullrun.rs`). ENDLOGO takes the same convention (`endlogo.md`).

## 6. `setpalarea`'s byte count, and the DAC it leaves

`BEG/BEG.C:84`'s `setpalarea(pal2,0,254)` uploads `2*count + start` = **508** bytes, not 762:
entries 0-168, entry 169's red, and nothing of 170-254, which stay white for the whole part; entry
255 keeps the black `:32-34` wrote. `bugs.md` §4 holds the bug, its disassembly in `BEG/BEG.EXE`
and the other parts that carry it
(`logo::tests::the_upload_is_two_count_plus_start_bytes`). It is invisible on this
art: every index the screen uses is inside the written 169, and the §3 scan rerun with a 762-byte
upload returns the same `c` for all three moving frames. GLENZ inherits entries 170-254 white.

## 7. Verification

| index | plane | port vs reference | register |
|---|---|---|---|
| 430 | indices and RGB | **0 of 128,000** (all index 15, all white) | exact |
| 470 | RGB | **0** | exact |
| 560 | RGB | **0** | exact |
| 900 | indices | **0** (byte-identical to `SRTITLE.UP` decoded) | exact |
| 430-450 (lead-in, every frame) | RGB | **0** | A5 |
| 436 | indices | **127,998** (the port's plane already holds the picture) | A5 |

430 and 900 compare as indices, being held screens; 470 and 560 as RGB, because the indexed read
is late by about twenty fade steps (`reference-indices.md` §1.3, D15), so the DAC widening is
load-bearing on the middle pair. Pinned by `tests/beglogo.rs` and by `cargo xtask rediff` (entries
`beglogo`, `beglogo-leadin/even`, `/odd`, `/plane-436`; the lead-in window is
`reference-indices.md` §25.6). In block 1 the ramp is frame-locked, so the port's 7930 is the
original's 7930 byte for byte (§5); `fullrun/03-beglogo` pairs it at the seam's −37 frames, where
it is 81,273 px off, and 8300 is 0 (A53).

## 8. Reproduce

```bash
cargo test -p secondreality --bin secondreality parts::beglogo
cargo test -p secondreality --test beglogo
cargo xtask rediff --only beglogo
cargo run --release --bin secondreality -- --headless --part beglogo \
    --dump-frames 430,470,560,900 --dump-dir out/beglogo
cargo xtask compare --reference reference/parts/beglogo --rendered out/beglogo \
    --out out/beglogo-diff        # mode: mixed, four frames at offset 0, exit 0
```

**Register.** `bugs.md` §4 (`setpalarea`'s byte count).
