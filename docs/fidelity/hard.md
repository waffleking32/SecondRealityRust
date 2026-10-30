# HARD — the dots, the grid and the cube

Sources: `HARD/B.EXE`, the only demo part in the directory (`MAKEFILE:1-4,22-26`): `B.C` includes
`BP.C` (`B.C:123`) and links `BA.ASM` (the dots), the c-prefixed `VEC.ASM` engine (`VEC.ASM:1-15`)
and the four generated dot streams plus the grid table (`LINK.TMP:1-3`). `GMAIN.C`/`V.EXE` is an
interactive engine harness, not the effect (it has no DIS, no `muscode`, and a palette the program
zeroes); `ASM.A`, `LINEBLIT.INC` and the working art are on no link line.
Port: `bins/secondreality/src/parts/vault/hard/`, registered as `hard`, reached by `--part hard` and
the vault menu. The module docs own the mechanism: `part.rs` (pre-roll, letterbox, state machine,
palettes), `dots.rs` (streams and dot engine), `math.rs`, `poly.rs` and `span.rs` (the engine fork,
listed in `mod.rs`).
Run: unanchored and silent. The mapping is `boot = 356 + tick` (§1).
Reference: `reference/parts/hard`, keyframes 480, 560, 630, 700, 770, 840, 910, 1050, 1600; 840
carries an `.idx`. The capture record (recipe, the start sweep, the mode-start table and its batches,
the repeats) is `reference-indices.md` §24.1, §24.7 and §24.8; `K0` = 356 ±1. **All nine are
byte-identical**, and 840's index plane too.

## 1. Timeline

**The pre-roll is 119 ticks.** One `waitb` is one tick and one displayed frame, and frame `t` is the
capture's `356 + t` (A47):

| ticks | boot | source | what changes |
|---|---|---|---|
| (`init`) | ≤ 355 | `B.C:131-142` | `dis_partstart`, mode 13h, the white page (index 15), the start gate, four `waitb` |
| 0-19 | 356-375 | `B.C:143-147` | the row-100 wipe, 16 columns a tick from x = 319 down: 356 has 16 black pixels, 375 the whole row |
| 20-47 | 376-403 | `B.C:148-161` | the fill, 28 iterations; the first two clear no new row and the third re-clears row 100 (376-378 repeat 375) |
| 48-117 | 404-473 | `B.C:162` | 70 waits, no change |
| 118 | 474 | `B.C:164-201` | the palette, border and letterbox: the first letterbox frame |
| 119… | 475… | `B.C:203` | the main loop, count 0 of mode 0 |

`the_pre_roll_is_the_sources_tick_counts` pins the 20 and 28.

**The main loop**, one iteration a tick, its result displayed on that tick
(`the_first_half_mode_sequence_is_the_sources`, `the_full_mode_sequence_is_the_sources`):

| mode | iterations | ticks | boot | on screen |
|---|---|---|---|---|
| 0 | 34 | 119-152 | 475-508 | letterbox, no dots |
| 1, `part1init` | its own `waitb` (`BP.C:29`) | 153-154 | 509-510 | letterbox |
| 1, phases 1-4 | 4 × 73 | 155… | 511… | first spawn 511, first drawn dots 512; phase 1's count 50 at 560 (tick 204) |
| 3 | 42 | count 0 at 453 | 809 | FOREVER held, dots dead |
| 4 | 102 | count 0 at 495 | 851 (identical to the mode-3 hold) | count 1 is the first visible band, 852 |
| (boundary) | two inserted displays | 597-598 | 953-954 | the static mode-4 band (§2) |
| 5, part 2 | 501 | count 0 at 599 | 955 | the dot grid; 956 the first visible grid frame; last part-2 frame 1455 |
| 6, part 3 | 801 (counts 0..800) | … | first visible cube 1458 (count 2) | the glenz cube; last drawn cube 2256 |
| 7 + `default` | 2 | to 1902 | 2257-2258 | no draw; the part retires after port frame 1902 |

The run is **1,903 port frames, 0..1902**: the pre-roll, modes 0..4 (478 ticks), the two boundary
displays, 501 + 801 iterations and the two exit arms. The source's count limits alone sum to 1,774
(34 + 4×73 + 4 + 42 + 102 + 500 + 800); the `NEXTMODE` ticks of modes 5 and 6, four `part1init`
waits, the exit arms and the boundary displays make up the difference.
`hard_draws_the_whole_effect_and_holds` (`tests/vault.rs`) pins the last cube, the two no-draw ticks
and the shell's hold.

The display phases are measured, not fitted. Mode 4 uploads its palette at the top of the iteration
(`B.C:208`), before the switch rebuilds the band, so count `n` shows the band built at count `n−1`:
reference 852's first band has `a = 252 - pal1colp` at `pal1colp = 1` (entries 251..253), not 2.

## 2. The mode-5 boundary

Mode 4's last iteration is count 101, so the source's part-2 count 0 would be **953** (851 + 102).
The artifact's is **955**: a contiguous capture of 950-960 shows 950-955 byte-identical and 956 the
first changed frame. The 955 band is 40,860 background `(0,0,81)` pixels plus 100 white trail pixels
at index 189; 956 and 957 replace exactly those 100 with `(4,255,255)`, 6-bit `(1,63,63)`, which is
**entry 65** of `part2init`'s gradient (a = 1, d = 56), not the literal white `pal[1] = (63,63,63)`
that `BP.C:135-138` writes. At `zp = 3500` the first grid frame draws only the lattice's `x = -500`
face, whose 100 points all project to `depthcol[3000] = 65` and are exactly the shipped `_GRID1.TMP`
pair list (`the_first_grid_frame_is_the_shipped_grid1_list`). 955 is the port's model of `part2init`
(count 0): it converts the 100 index-189 pixels to entry 1, white like 189, so the frame is
pixel-identical to the mode-4 hold. 956/957 are counts 1 and 2 (the state advances only after count
1's draw); 958 is count 3. The pair is RGB-only, so the 955 attribution is the port's model.

The overrun is not at mode 4's count 0 (851/852 match at 0), so it is a later mode-4 iteration's
missed retrace (D22). The port inserts two static displays between mode 4's advance (port 596,
reference 952) and part 2's count 0 (port 599, reference 955), and references 950-960 compare at 0
(A48). The mode-5 palette is `BP.C:135-154` literally.

## 3. Measured facts the module docs do not hold

- **The streams.** `BDO.C:12-41` extracts four 40-row bands of `FCRZ.U` (64,000 bytes, SHA-256
  `e2c068fe…`): **859 / 690 / 722 / 1,071** records; the picture's histogram is 60,645 zeros, 164
  ones, 441 twos, 2,737 threes and 13 fours. `B.EXE`'s linked streams at 55,875 / 59,315 / 62,082 /
  64,977 match byte for byte except the last terminator byte of bands 1 and 2 (A50;
  `the_dot_streams_are_the_bdo_extraction`). A stream of `R` records yields `N - N/R` dot calls for
  `N` spawn iterations, because record 0 is never read.
- **`_GRID1.TMP`** (`GRID1: [i16; 200]`, `B.C:112-121`) matches `B.EXE`'s copy at 69,265 in **399 of
  400 bytes**: the last is `0x4F` in the binary, `0x00` in the file, the y of the 100th pair. The port
  uses the file (A50; `the_grid_table_is_the_shipped_grid1`); no keyframe depends on it.
- **There is no `rand()` pre-roll.** `B.C` has no burn loop (the 60-call loop is `EDGE.C:122`, another
  program). With none, the first drawn frame is reference 512 exactly, and the first spawned record's
  six words follow the MS C 6.0 LCG (`MsRand`) from its seed
  (`the_first_spawn_is_the_sources_rand_sequence`).
- **`part1`'s palette** uploads 96 bytes, so its writes land on entries 1..3/5..7, 5..7/9..11,
  9..11/13..15 and 13..15/17..19 for the four phases; the capture's count-50 group 0 is `(39,39,53)`
  (`the_part1_palette_uploads_only_32_entries`). **Mode 4's band** forces entries 189 and 1 to 63
  (`a = 254-65`, `pal[1*3]`); mode 5's rebuild tests `vram[u] == 254-65` (`B.C:299`), and the held
  930-955 band's only trail pixels are the 100 white ones at 189
  (`the_mode_four_band_palette_is_the_sources`). A mode-4 dot spawned at 254 last draws at 189 after 65
  frames (`the_dot_engine_leaves_the_66_frame_trail`); each reaches its `doit2` destination on that
  draw, which is why the trails end on the first grid frame's points.
- **The lattice** is the shell of a 10×10×10 cube: **488 points** (`10³ − 8³`;
  `gridpoints[0] = (a-1)/3`, `BP.C:164`) (`the_grid_is_the_ten_cube_shell`). **`depthcol`'s `b`** is
  uninitialised at the first test but carries 127 at `a = 1000`, so the table is 127 below 1032 and
  `127-(a-1000)/32` down to 62 at 3099 (`part2init_reads_the_uninitialised_b`).
- **Part 2's z domain.** Over its 501 states the projected z′ is 1139..3964, every drawn point's low
  word is 1139..3000 (the signed `jg` skips the rest), and the high word is zero
  (`the_part2_grid_z_range_and_drawn_words_are_the_measured_domain`).
- **The part-2 fade is the source's bug.** `a=(500-16)-count` (`BP.C:173-176`) is negative for every
  count that reaches it, and `fadepal`'s unsigned `cmp dx,256` clamps to 255, so the palette only dims
  by 255/256: entry 0's blue renders 8-bit 77 from 81 (`fadepal_is_the_high_byte_multiply`). The
  capture agrees: the band background is `(0,0,81)` at 1440 and `(0,0,77)` at 1445
  (`--frames "$(seq -s, 1430 5 1500)"`).
- **Part 3.** `cubepoints` is **-510..410** (`BP.C:85-94`). The palette ramps in over counts 1..19
  and from count 17 is the ramp dimmed by 255/256; `part3init`'s `pal[2..253] = (0,0,20)` equals the
  band's entry 0, so the first cube display is a uniform band and the first *visible* cube is count 2
  (1458). Reference 1600's top colour `(215,215,57)` is entry 88 = `0x58`, the XOR of the shadow's
  `0x40` with faces `0x08|0x10`, at ramp value 54 dimmed to 53. The band is never cleared, so spans
  that are 0 in both frames keep what part 2 left (`the_band_is_not_cleared_between_frames`).
- **The end.** The last drawn cube is **2256** (count 800); 2257 is mode 7's no-draw iteration and
  byte-identical. A contiguous capture of 2245-2257 matches the port's 1889-1901 at 0 at every index
  (A48). A capture of 2245-2258 is refused ("reached frame 2259 before frame 2258"), so the close after
  2258 (the `if(!dis_indemo())` text-mode restore, `B.C:325-329`) is source-derived.

## 4. Decisions

- **The start gate is the capture's one key**, and the `0xff` cut (`BP.C:263`) is unreachable
  without a second, so the port omits it (A46; `the_full_mode_sequence_is_the_sources`).
- **One tick is one `waitb`**, with the display phase measured (A47).
- **The two boundary displays are static inserted ticks** (A48).
- **The argument path, its debug strip and the border** are not ported (A49).
- **The data's two one-byte questions** are recorded, not fitted (A50).
- **Two unreachable branches are modelled, not copied** (D24). `do3dots`' second clear writes two
  **word** stores at `[si+12]`/`[si+12]+320` (`BA.ASM:229-231`), but the code that sets `[si+12]` is
  behind `jmp @@4` (`:235`), so it stays 0; the port clears one byte a row at 0 and 320. And a negative
  z low word would index `cs:_depthcol[bp]` past the array (for 0xFF00, `_depthcol - 0x100`, a code
  byte, 0x07 with the shipped link map); the port returns 0. §3's z domain shows neither is reached
  (`the_shadow_grid_dot_skips_positive_z_and_models_the_negative_word`). The port's `count`,
  record-length, shadow and offset bounds checks are guards for inputs part 2 never produces.

## 5. Comparison

| keyframe | port frame | mode | differing px | register |
|---|---|---|---|---|
| 480 | 124 | 0 | 0 | exact |
| 560 | 204 | 1 / phase 1 | 0 | exact |
| 630 | 274 | 1 / phase 2 | 0 | exact |
| 700 | 344 | 1 / phase 3 | 0 | exact |
| 770 | 414 | 1 / phase 4 | 0 | exact |
| 840 | 484 | 3 | 0 (RGB and `.idx`) | exact |
| 910 | 554 | 4 | 0 | exact |
| 1050 | 694 | 5 | 0 | exact |
| 1600 | 1244 | 6 | 0 | exact |

At offset 0 (`cargo xtask rediff --only hard`). 480 and 840 are held; the filed 480 is RGB-only because its read landed after mode 0 had
ended. The boundary (950-960 against 594-604) and the end (2245-2257 against 1889-1901) are compared
against captures of their own, at 0 everywhere (A48 names the commands).

## 6. Reproduce

```bash
cargo xtask rediff --only hard
cargo test -p secondreality --test vault -- --nocapture hard_
cargo test -p secondreality --bin secondreality parts::vault::hard
tools/vault_capture.sh hard --frames 480,560,630,700,770,840,910,1050,1600   # reference-indices.md §24.1
```

**Register.** `bugs.md` §7-§9 and §25 (the dot engine, the 16-bit z load, record 0); A46-A50.
