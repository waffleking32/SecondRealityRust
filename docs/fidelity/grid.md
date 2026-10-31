# GRID — the line rider

Sources: `GRID/P.EXE`, which is `MAIN.C` (Microsoft C, large model, 16-bit `int`s) linked with
`ASM.ASM`, the generated `LINEBLIT.INC`, the `_EYE.OBK` blob of `EYE4.UH` and the DIS client stub
(`GRID/MAKEFILE:20-23`, `GRID/LINK.TMP:1-2`). `GRID/DOLINE.EXE` is the line-blit generator
(`DOLINE.C:98`), not the effect; the `*.OK` snapshots are not linked.
Port: `bins/secondreality/src/parts/vault/grid.rs`, registered as `grid`, reached by `--part grid`
and the vault menu. The module and item docs own the mechanism: `sinus_tables`, `load_eye`,
`palettes`, `LineBlitter` and the loop body.
Run: unanchored (the frames number from 0) and silent. The mapping is `boot = 353 + j` (§6).
Reference: `reference/parts/grid`, keyframes 400, 500, 600, 700, 800, all RGB-only. The capture
record, its recipe, the start sweep and the repeats are `reference-indices.md` §24.1, §24.2, §24.5 and
§24.8; `K0` = 353 ±1.

## 1. Timeline

| boot | source | on screen |
|---|---|---|
| 340-351 | `MAIN.C:144-153` | the developer's static 1,024-point dot screen, waiting at `getch()` |
| 352 | `:156` | the one-off transition as the key is consumed: the eye copy overwrites the dots |
| 353 | `:157-177`, iteration 1 | the port's frame 0, the first stable loop frame |
| 353 + `j` | iteration `j + 1` | the curve over the eye; the palette advances one plane a frame |

The moving part is one parametric curve of 1,000 segments through 1,001 points, `x_k =
sinus1[(i + k) & 1023] + 160`, `y_k = sinus2[k] + 100` (`MAIN.C:161-171`). Three successive
iterations' curves are on screen at once in colours 1, 2 and 4, each living three frames. A pixel
visited twice by **one** curve cancels; a pixel visited by two **different** curves stays lit, so the
visible mask is the union of the three self-XOR line masks, not an XOR of masks.

The dot screen is not drawn by the port; it is the measurement that pins `d0` (§2).

## 2. The sine tables, and the measured uninitialised `d`

`sin1` is `(int)(sin(((a * pi) * 2) / 1024) * 256)` (`MAIN.C:44-49`) with `pi` exactly
`3.141592653589`; `sinus1` (`:51-58`) and `sinus2` (`:59-62`) are built with 16-bit wrapping cursors
and truncating division (`sinus_tables`):

```
b += 97; c += 87; d += 67                      (16-bit, wrapping)
sinus1[a] = (sin1[(b>>3)&1023] + 2*sin1[(c>>3)&1023] + sin1[(d>>3)&1023]) / 8
sinus2[a] = sinus1[(a+277)&1023] * 7 / 10
```

**`d` is never initialised** (`:33`; `:51` zeroes only `b` and `c`). The compiled object's frame
predicts `0x116`, but the shipped `P.EXE` does something else: an exhaustive match of every `d0` in
0..65535 against the capture's 982-pixel dot screen (boot 340) finds only the class
**`0x7FE (mod 0x2000)`** matching all 982, while `0x116` gives 1,004 pixels of which 47 are shared
(D24: the binary is the artefact, not the object). Adding `0x2000` adds 1,024 to `d >> 3`, which the
`& 1023` index discards, so the minimal representative is **`d0 = 0x07FE`**. The scan is
`the_measured_d_is_the_only_residue_whose_tables_match`; the 982-pixel set (bbox x 37..280 × y
14..184, `0x27FE`/`0x47FE` identical) and the object's disagreement are
`the_initial_d_is_the_measured_0x7fe`. `bugs.md` §6 is the preserved bug.

With `d0`: `sinus1 ∈ [−123, 120]`, `sinus2 ∈ [−86, 84]`, a `−45 → 37` seam and an 82 jump
(`the_sine_tables_are_the_sources_own`); `sinus1[42] = −3`, `sinus2[34] = 0`
(`the_sinus_tables_use_the_sources_wrapping_arithmetic`).

`sin256` (`MAIN.C:38-43`) and the 64 KB `muldata` (`:130-139`) are built but read by nothing linked
into `P.EXE` (`ASM.ASM` and `LINEBLIT.INC` contain no `muldata`), so they are not ported. `_asmloop2`
(`ASM.ASM:113-134`), the unused second slot words and `_line` (`:247-269`) are dead too.

The keyframes' residual is §8's A41 sampling instant, not the tables.

## 3. The eye shift

`MAIN.C:141`'s `for(u=784;u<64684;u++) eye[u]<<=3` stops **100 bytes short** of the image's end: row
199, x = 220..319 keeps its raw indices 24..28 and shows the eye palette's cyan group 3 under the
line tints instead of the shifted grey ramp (`bugs.md` §24). `the_eye_shift_stops_100_bytes_short`
checks all 64,000 bytes against the file.

## 4. The palettes

`palettes` (`MAIN.C:95-126`) builds three palettes from eye entries 0..31 only. Entries with
`x & 7 == 0` are identical in all three, so the shifted background (multiples of 8) never changes hue
with the cycle; only the curve colours and the unshifted tail do
(`the_three_palettes_are_the_sources_own`).

## 5. The line blit

`LineBlitter` produces the generated code's pixel set rather than running it.
`the_line_blit_is_the_committed_generated_code` is the oracle: it reads
`upstream/SecondReality/GRID/LINEBLIT.INC`, pairs its 357 routines with the generator's y = −8..=8,
x = −10..=10 order, and checks that each routine's 16-bit `di` stores are exactly the pixels the
port's blit XORs for that delta.

Every out-of-table delta draws the dispatch table's default, `lblt_54b56`, the line to (−10,−8)
(`ASM.ASM:213-217`, `LINEBLIT.INC:2-4`; `bugs.md` §24). With the measured tables two such segments
occur: **segment 746 of every iteration has dy = 56** (the y waveform is static), and from
**iteration 24** the cyclic seam segment `k = 1023 − i` has dx = 82 (`sinus1[0] − sinus1[1023]`). Both
draw the (−10,−8) fill word once per line in the current colour
(`an_out_of_table_delta_draws_the_fill_word`). The y key's dropped bit 7 is never
reached: the static waveform's adjacent deltas are −7..=56
(`the_static_y_waveform_never_reaches_the_dropped_bit`).

## 6. The loop, and the frame mapping

Each iteration (`MAIN.C:157-177`, `ASM.ASM:136-202`) checks `dis_exit()` and `dis_muscode(3)`,
advances `plane = (plane + 1) % 3` from 0, erases the line this plane drew three iterations earlier
(the first three slots hold `@@ret`), draws and stores 1,000 segments, then `setborder(0)`,
`dis_waitb()`, `setborder(1)`, `setpalette(pal[plane])`. `d = sin1[k&1023]/4+256; d = 0;`
(`MAIN.C:162-163`) is a dead store: the y waveform is static.

**The palette phase is measured.** The upload comes after the retrace, so the frame published for
iteration `i` shows under `pal[i mod 3]`, uploaded at the end of iteration `i−1`. The filed 400's line
tints are `pal[0]`'s +40/+20/+10 ranks, 500's `pal[1]`'s, 600's `pal[2]`'s, and 700/800 repeat the
cycle; the comparison matches only under that mapping. The port sets the palette at the **start** of
the tick that draws iteration `i`, because the engine stamps the live palette onto the frame this tick
composes. No constant or anchor was changed (D17/D18).

**The frame mapping.** Reference 353 is the content of source iteration **1**; iteration 0 is the
transition frame 352. So the port performs iteration 0 in `init` without composing it, and its frame
`j = boot − 353` draws iteration `j + 1`. `init` installs the mode-13h default DAC
(`shell::hardware_default_palette`); the first upload comes at frame 0.

## 7. Decisions

Registered as A40:

- `getch()` is not a key wait: the capture's one `KBD_space` selects the state the port starts in.
- `dis_muscode(3) == 3` cannot fire: `muscode` is 0 with no player (D8), and the capture sends no
  second key.
- The dot screen is not drawn (§1). The `#if 0` wipe and the commented `muscode(2)` wait
  (`MAIN.C:70-93`, `:155`) do not ship.

## 8. Comparison

| keyframe | port frame | differing px | register |
|---|---|---|---|
| 400 | 47 | 6 | A41 |
| 500 | 147 | 2 | A41 |
| 600 | 247 | 36 | A41 |
| 700 | 347 | 33 | A41 |
| 800 | 447 | 14 | A41 |

RGB of 64,000 pixels, at offset 0, max channel delta 41 (rediff's `report.json` gives it); the ±1 neighbours differ by about
10,000-12,000. `grid_matches_the_reference_at_its_measured_start` (`tests/parts.rs`) compares
`R−K0−1`, `R−K0`, `R−K0+1` for each keyframe and asserts that every differing pixel is on rows 14-15,
the topmost rows the curve reaches: the capture published its frame while the original was still
drawing the top of the newest line, so those pixels miss it or carry the previous red rank. The eye,
the 100 unshifted bytes, the three line colours, the palettes and the (−10,−8) artefacts match
everywhere else. No constant, anchor or palette phase was fitted to reduce it (D17/D18/D24/D25).

## 9. Reproduce

```bash
cargo xtask rediff --only grid
cargo test -p secondreality --test parts -- --nocapture vault::
cargo test -p secondreality --bin secondreality parts::vault::grid
tools/vault_capture.sh grid --frames 400,500,600,700,800     # the capture, reference-indices.md §24.1
```

**Register.** `bugs.md` §6 (`sinus1`'s `d`) and §24 (the eye shift, the dispatch default); A40, A41.
