# LNS&ZOOM — the face wipe, the lens and the rotazoomer

Sources: `LENS/MAIN.C` (394 lines) over `LENS/ASM.ASM` (566 lines): `part1` (`:89-131`),
`part2` (`:133-183`) and `part3` (`:185-276`) run back to back (`:379-386`). Assets
`LENS.EX0` (215 bytes: the lens's dimensions and three overlay triples), `LENS.EX1..4`
(19,588 / 6,860 / 1,538 / 6,252 bytes: the row tables), `LENS.EXB` (64,784 bytes, Origin `Uh`:
palette at +16, pixels at +784) and `LENS.EXP` (18,864 bytes: the two paths); the 4,224-byte
`FILLER` pad is not read (§2). Port: `bins/secondreality/src/parts/lnszoom.rs` (`"lnszoom"`);
the module doc has the screens, `rotate`, the lens, the palettes and the preserved bugs. Block 3's
second part, after MNTSCRL (`MAIN/U2.ASM:856-874`). Run: mapping `boot = 2452 + tick` (§7.1);
music MUSIC1 order 45, entry pre-roll 490 polls. Reference: `reference/parts/lnszoom`, 2700
(`.rgb` and `.idx`), 3400, 4200, 4900, 5650 (`reference-indices.md` §12).

## 1. Timeline

| tick | boot | what |
|---|---|---|
| 0 | 2452 | part 1's gate passes; `setmframe(0)` |
| 1 | 2453 | part 1 frame 0, the first wipe |
| 300 | 2752 | the wipe's 300th iteration; the loop exits |
| 301 | 2753 | the `musplus < -20` gate begins |
| 473 | 2925 | the gate passes (`48:44`, -20) |
| 474 | 2926 | part 2 frame 0 (path frame 0) |
| 1188 | 3640 | part 2 frame 714, the last lens draw |
| 1189..1193 | 3641..3645 | the five-frame hold |
| 1194..1195 | 3646..3647 | the rotpic90 rotation and `inittwk` (`ROT_SETUP_FRAMES = 2`) |
| 1196 | 3648 | part 3 frame 0 |
| 3195 | 5647 | part 3's 2000th iteration; it also sets the white palette |
| 3198 | 5650 | the reference's white keyframe, inside the hold |

`the_real_player_puts_the_acts_on_the_reference_ticks` asserts the two load-bearing rows (474
and 1196) with the real player.

## 2. The screens and the init

Part 3's `inittwk` sets maximum scan line 3, so the 320×200 capture is 100 unique rows, each
shown twice (the row pairs of `reference/parts/lnszoom/frame_004200.rgb`, asserted for the
reference and the port by `the_five_keyframes_at_the_measured_mapping`). `rotate`'s two mask
writes make a four-pixel group `(a, a, b, b)`; the reference's 4200 and 4900 show that
repetition (`the_rotazoomer_composes_160_samples_each_shown_twice_and_100_rows_twice`).

`LENS.EX0`'s first bytes are `98 00 74 00` (152, 116), so `lensxs = 76`, `lensys = 58`; `init`
refuses other dimensions. The overlay triples at +4 are `00 05 0f 00 07 1a 00 09 25`
(`xxd -l 16 assets/LENS/LENS.EX0`). The 1,536-byte overrun (`:302`) copies the picture's last
1,536 bytes into the pad past it; the port's `back` is 65,536 bytes with that copy at
`[64000, 65536)`, which the lens's 16-bit addresses reach
(`the_tables_are_bounded_and_the_overrun_is_the_1536_byte_copy`). The port reproduces the copy's
contents, not `FILLER`'s `0x21` bytes; the copied bytes are all 0 (`bugs.md` index).

## 3. Part 1 (`MAIN.C:89-131`)

`if(dis_musplus>-30)` (`:104`) tests the function pointer, so the wait always runs.
`the_missing_call_in_the_part1_guard_is_preserved` feeds `musplus = -32` (the value a corrected
guard would skip the wait on) for three ticks, and the part waits, then passes at -6. After 80
drawing frames the screen is the picture byte for byte; the loop runs to 300
(`the_wipe_runs_three_hundred_frames_and_draws_only_the_first_eighty`, at frame 79). The
source's writes outside 0..63,999 are outside the displayed screen and the port drops them.

## 4. Part 2 (`MAIN.C:133-183`)

Part 2's frame 0 is tick 474: the entry pre-roll puts the inter-act gate's `48:44` on tick 473
(boot 2925, §7.1). The table test prints the files' geometry:

```text
$ cargo test -p secondreality --bin secondreality the_tables_are_bounded -- --nocapture
EX1: 19588 bytes, 103 rows drawn, data ends at 19588
EX2: 6860 bytes, 102 rows drawn, data ends at 6860
EX3: 1538 bytes, 43 rows drawn, data ends at 1538
EX4: 6252 bytes, 116 rows drawn, data ends at 6208
```

Rows with a zero count (EX2/3/4) or under four (EX1) point at the file's end and are never read;
every row the blitter reads is inside its file. `uframe < 96` uploads `fade2` to entries
64..255 (`:146-151`). The `a > 3` clamp (`:176`) is unreachable: the port runs one frame a tick,
so `frame` and `uframe` advance 1:1 (the original's `frame` advanced by `min(waitb(), 3)`).
The lens keyframe's residual is the picture asset, not the lens (§7.2).

## 5. Part 3 (`MAIN.C:185-276`)

- The `setpalarea` at `:200` uploads `fade`'s first white-step block and `inittwk`'s DAC clear
  erases it at once; the port uploads it and then sets black
  (`the_lens_is_seven_hundred_fifteen_frames_and_a_five_frame_hold`).
- `rotpic90 = back` (`:191`) aliases the picture buffer; the port keeps the buffers separate,
  which is unobservable because nothing reads `back` after part 3 begins.
- The path is 2000 frames × 4 ints at `lensexp+4+2*1430`, and the file is exactly
  `4 + 1430*2 + 2000*8` bytes
  (`the_path_data_is_the_expected_size_and_starts_where_the_c_prelude_reads`).
- The `musplus > -4` break never fires in the reference's window: `reference/music/second3a`
  holds -32 (order 52, `zplus` 0) at boot 3648 and -18 (order 60 row 46) at 5650
  (`awk -F, '$1>=3640 && $1<=5660' reference/music/second3a/trace.csv`), so the rotazoomer runs
  all 2000 frames.
- `rotate` applies the aspect ratio (`* 307 >> 8`) after the displacement table is patched, so
  only the row advance is scaled. Applying it first does not reproduce 4200
  (`the_aspect_ratio_scales_the_row_advance_not_the_in_row_steps`; `bugs.md` §16).
- After the loop the palette is all 63. 5650's white is the palette; video memory still holds
  the last rotazoomer frame (§7.2).

## 6. Decisions

- **The picture asset is the tree's** `LENS.EXB`, which differs from the release's in the
  forehead (A25, §7.2).
- **The hand-over costs eight frames, one more than the source's seven waits** (the loop's exit
  wait, five hold waits and `:199`'s `waitb()`). 4200 is path frame 552 and 4900 path frame
  1252, both `3648 + frame`. The rotpic90 rotation and `inittwk`'s 64 KB clear cost the release a
  vblank; `ROT_SETUP_FRAMES = 2` charges it (A25, adopted under D24).
- Preserved: the part-1 guard, the erased `setpalarea`, the 1,536-byte overrun, the unreachable
  clamp (`bugs.md` §16 and the index).
- Not modelled: the Scroll Lock border test (`MAIN.C:36-47`; the engine's frame has no
  overscan border and the reference was captured headless), `rotatez` (`ASM.ASM:419-564`,
  never called, `rotatez_is_dead`), and the copper (the source registers no callbacks).

## 7. Mapping and verification

### 7.1 The mapping

A `--part lnszoom` run seeks MUSIC1 to order 45 (`shell::music_for_part`) and `init` polls
`MUSIC_PRE_ROLL = 490` times on a fresh start, so the engine's tick-0 poll lands on the
reference's position at boot 2452:

```text
$ awk -F, '$1>=2452 && $1<=2456' reference/music/second3/trace.csv
2452,45,59,33,0,6,130,0,1,0
2453,45,59,33,0,6,130,0,1,0
2454,45,59,33,0,6,130,0,1,1
2455,45,59,33,0,6,130,0,1,2
2456,45,60,33,0,6,130,0,1,2
```

`play_song(45)` publishes `45:59` on the 491st poll, so 490 precede tick 0
(`the_entry_pre_roll_lands_the_reference_position_at_tick_zero`, which also asserts
`musplus = -5`: part 1's `< -6` gate passes without waiting). At 490 the published `48:44` also
falls on the trace's boot 2925, the inter-act gate's frame (`musplus = -20`):

```text
$ awk -F, '$1==2925' reference/music/second3a/trace.csv
2925,48,44,52,0,3,130,0,1,351
```

`zframe` 0 at 2452 is the part's `_dis_setmframe(0)` (`:106`). The manifest and fresh-start rules
are `part-sequence.md` §10 (mapping, pre-roll and block-seam conventions). In a full run the
part's first frame is a `FULLRUN_SEAMS` row (18,656 in the original, 18,670 in the port; A53),
and the full run's 19000 differs by the same 2,058 as 2700 (A25).

### 7.2 The comparison

| keyframe | tick | plane | differing px | % | max Δ | register |
|---|---|---|---|---|---|---|
| 2700 | 248 | RGB and `.idx` | 2,058 | 3.2156 | 53 | A25 |
| 3400 | 948 | RGB | 1,935 | 3.0234 | 48 | A25 |
| 4200 | 1748 | RGB | 0 | 0 | 0 | exact |
| 4900 | 2448 | RGB | 1,396 | 2.18125 | 41 | A25 |
| 5650 | 3198 | RGB | 0 | 0 | 0 | exact |

The rows are the `lnszoom` entry of the rediff table (`xtask/src/rediff/table.rs`), and
`the_five_keyframes_at_the_measured_mapping` asserts them (`-- --nocapture` prints the
percentages and deltas). `reference-indices.md` §12.4 tables the same five files byte-identical
across five sessions.

**The residual is one asset difference (A25).** The tree's `LENS.EXB` differs from the release's
in 2,058 pixels of the forehead, rows 33..91 and columns 129..211, each a small palette-index
change. `the_forehead_asset_difference_is_the_whole_residual` measures that set from the
reference's 2700 index plane, patches the tree's picture with it, and renders 3400, 4200 and
4900: all three become byte-identical. The five residuals are that picture seen through the
wipe, the lens and the rotazoomer.

**5650's `.idx` is not compared.** It is the tool's planar memory read: rows 0..99 are the
rotazoomer's 100 character rows and rows 100..199 the memory past them (the test pins 13,692
non-zero bytes in rows 0..99, zero below, 63 distinct indices). The displayed frame shows each
character row twice, so the port's doubled plane is a different geometry. The white keyframe is
compared in RGB only.

### 7.3 D19's support

The test dumps `K-1, K, K+1` around every keyframe. 2700 and 5650 are held (both neighbours
equal); 3400, 4200 and 4900 move (both differ).

### 7.4 Reproduce

```bash
cargo xtask rediff --only lnszoom
cargo test -p secondreality --test lnszoom -- --nocapture
cargo test -p secondreality --bin secondreality parts::lnszoom
```

## 8. Open items

- **The release's face picture** is in its `LNS&ZOOM.EXE` image (D49), not yet read (A25).

**Register.** `bugs.md` §16 (the aspect block), with the 1,536-byte overrun, the guard, the clamp
and the dead paths in the index.
