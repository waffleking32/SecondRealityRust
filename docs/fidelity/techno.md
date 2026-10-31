# TECHNO — the interference, the bars and the troll

Sources: `TECHNO/KOE.C` (728 lines) over `KOEA.ASM` (1,065), `KOEB.ASM` (632),
`POLYCLIP.ASM` (273) and `READP.C` (72); assets `assets/TECHNO/CIRCLE2.EGA` (8,000 bytes,
the extern `_circle2`), `assets/DDSTARS/PIC.EGA` (24,000 bytes, `_circle`) and
`assets/TECHNO/TROLL.UP` (70,718 bytes, Origin RLE). Port: `bins/secondreality/src/parts/techno/`
(`"techno"`); the module doc has the three stages, the two screens and the preserved bugs.
Block 2's fourth part, after TUNNELI (`MAIN/U2.ASM`). Run: a mapping in three pieces,
`boot = 4797 / 4799 / 4800 + tick` (§2); music MUSIC1 order 18, entry pre-roll 253 polls
plus 2 (§2). Reference: `reference/parts/techno/{interference,troll}`, seven keyframes, all
RGB only (§8). `PIC.EGA` is the right file for `_circle`: its plane 0 and `CIRCLE2.EGA`'s plane 2
make the 4900 window, which rediff holds at 0.

## 1. Timeline

`the_sections_land_on_the_reference_ticks` runs the real MUSIC1 player through the whole part
and asserts the full transition vector. The frames are the engine's (the dump indices).

| frame | phase | source | what |
|---|---|---|---|
| init | Gate | `KOE.C:213-214` | `musplus < -4` passes; `setmframe(0)` |
| 0, 1 | ResetClear, ResetMode | `KOEB.ASM:175-403` | `resetmode13`, `init_interference` |
| 2 | FirstPass | `KOEB.ASM:405-607` | iteration `n` is frame `n + 2` |
| 258 | SecondPass | `KOEA.ASM:900-905` | until `getmframe >= 925` |
| 1246..1250 | Flash(32)..MusplusMinus3 | `KOE.C:275` | five flashes, `mode16`, gate `musplus < -3` |
| 1295 | RowWait | `KOE.C:277-281` | `musrow & 7 == 7` |
| 1303..1435 | Wipe, WipeRowWait, WipeFlash | `KOE.C:262-316` | four bar wipes |
| 1436 | Doit1 | `KOE.C:317-577` | 420 counts, eight pages |
| 1857 | Doit2 | | 840 counts, eight pages |
| 2698, 2699 | Doit3Prologue, Doit3 | | 980 counts, four pages |
| 3396 | TrollWait1 | `KOE.C:588-596` | after doit3's break; mode 13h, chain-4 off |
| 3398 | TrollBuild | | `TROLL.UP` blitted |
| 3400 | TrollOrderWait | `KOE.C:638-648` | the black hold, `order > 35 \|\| (order == 35 && row > 48)` |
| 3445 | TrollPan | | 300-count pan |
| 3498 | TrollRipple | | 50-frame ripple and fades |
| 3547 | TrollFinalWait | `KOE.C:705-726` | `-6 < musplus < 16` |
| 3967 | Done | | |

The view is 320×200 through frame 3395 and 320×400 (`y_line_offset` 640) from 3396; a dump is
sized at the tick's start, so the first 320×400 dump is 3397
(`the_sections_land_on_the_reference_ticks`,
`the_run_is_numbered_in_its_own_frames_and_declares_the_music_row`). The doit3 shrink gate is
`getmframe > 2520` and `count < 333` (`KOE.C:495-501`).

## 2. Mapping and music

A `--part techno` run seeks MUSIC1 to order 18 (`shell::music_for_part`) and `init` polls
`MUSIC_PRE_ROLL = 253` times on a fresh start. The published position is then the trace's
`18:60` with `musplus` -4, so the stage-1 gate passes on tick 0
(`the_entry_pre_roll_lands_the_reference_position_at_tick_zero`). The source's
`dis_setmframe(0)` is trace frame 4794 (`zframe` 0):

```text
$ awk -F, 'NR==1 || $1==4792 || $1==4794 || $1==4797' reference/music/second2/trace.csv
frame,ord,row,pat,loop,speed,tempo,zinfo,zplus,zframe
4792,18,59,3,0,3,130,0,1,2732
4794,18,60,3,0,3,130,0,1,0
4797,18,60,3,0,3,130,0,1,2
```

The part also polls once more on each of ticks 1 and 2 (`extra_polls`): the emulator's
CPU-bound preamble (`dis_partstart`, the two mode sets, the DAC and video clears) spends six
rendered frames, 4794..4800, over the part's first three ticks, and the player's clock
advances on every rendered frame. The keyframes are the evidence for the two polls, not the
arithmetic.

The keyframes lie on three affine fits, each of slope 1. The intercepts differ because the
reference spends its early frames in that preamble and its troll mode change costs a frame:

- **The first pass, `boot = 4797 + tick`.** The fade is frame-quantised: 4900 is the 101st
  iteration (tick 103), 5000 the 201st (tick 203). 4797 is also the content-probed first
  rings (`reference-indices.md` §1.7, §8.5); the part has no boot anchor.
- **The row-gated sections, `boot = 4799 + tick`.** 6300 is tick 1501, 8100 tick 3301, and
  8200 (the black hold, inside `TrollOrderWait` under either fit) tick 3401. The trace rows:

```text
$ awk -F, '$1==6300 || $1==8100 || $1==8200' reference/music/second2/trace.csv
6300,28,16,10,0,3,130,0,2,1117
8100,35,13,46,0,3,130,0,0,2453
8200,35,38,46,0,3,130,0,0,2526
```

- **The troll's pictures, `boot = 4800 + tick`.** 8300 is tick 3500 and 8400 tick 3600.

The three origins are `FIRST_PASS_ORIGIN`, `MUSIC_ORIGIN` and `TROLL_ORIGIN` in
`tests/parts.rs`; the fresh-start and manifest rules are `part-sequence.md` §10. In a full run
the part's first frame is a `FULLRUN_SEAMS` row (12,214 in the original, 12,216 in the port; A53).

## 3. The first pass (`KOE.C:213-216`, `KOEB.ASM`)

The fade reaches white at its 201st iteration: `palfader` is 202 after 101 iterations (DAC
entry 5 is `(0, 30*202/256, 40*202/256)`) and 402 after 201, where every channel is
`src + 146` saturated at 63, so every entry is white. The loop ends at `framecount == 256`
inside the reference's white hold
(`the_first_pass_reaches_white_at_its_two_hundred_and_first_iteration`). `power1`
(`KOE.C:173-211`) is computed and never read. The KOEB MOVE is dead (`sizefade` 0), so the
display start is the constant `80*100 + 160/8 = 8020` with pan 0.

The sine cursor in `KOEA.ASM`'s `interference_draw` is a byte offset (`:785-792`), not a word
index (the MOVE routines' `shl bx,1`, `:837-838`, `:854-855`). The table is therefore exactly
`SIN1024.INC`'s 1,024 words: the shared `parts::sin1024::SIN1024`, since `TECHNO/SIN1024.INC` is
byte-identical to GLENZ's (asset-inventory §B.7; `sin1024::tests::the_table_is_the_sources_1024_words`). The first pass cannot
show the difference (its power row is zero); 5424, the first second-pass frame with
`sinuspower == 1`, does (§4.1).

## 4. The second pass and the wipes (`KOE.C:218-316`, `KOEA.ASM`)

### 4.1 The second pass's display pipeline

`do_interference` draws the circle window at the start the previous MOVE left, and the MOVE
writes CRTC `0Ch/0Dh` and the pan after the draw; the emulator renders a frame with the
registers from before that write. The port therefore lags the start (and the pan, which the
source writes at the top of the next iteration) by one iteration, except on the first
second-pass frame, which shows the overlay drawn at 0 through the first pass's 8,020.

No keyframe in `reference/parts/techno/` is in the second pass, so the rule rests on four
live capture sessions:

```bash
$ cargo xtask reference capture --arg 2 --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames <list> --out out/techno-secondpass
#   5050,5054,5056,5058,5060,5062,5070
#   5000,5400,5500,5600,5700,5800,5900,6000,6300
#   5410,5420,5430,5440,5450,5460,5470
#   5421,5422,5423,5424,5425,5426,5427,5428,5429,5430,5431,5432
$ target/debug/secondreality --part techno --headless --dump-dir out/techno-secondpass-port \
    --dump-frames 259,261,263,271,601,611,621,622,623,624,625,626,627,628,629,630,631,632,633,641,701,801,1101
```

The second session is calibrated by its own 5000 and 6300, whose RGB `sha256sum`s equal the
tree's (`reference-indices.md` §9.2). At `tick = boot - 4799` all 23 frames compare equal
(`cmp`, no differing byte): 5058, 5060, 5062, 5070, 5400, 5410/5420/5430/5440, 5421..5432,
5500, 5600 and 5900. 5058 is the first second-pass frame; 5424 is the first where the byte
cursor matters. They also confirm `rotate1`'s zero first carry for all eight rotations.

### 4.2 The bar wipes (`KOE.C:262-316`)

Each band runs `for(a=256;a>-400;a-=32)`, 21 iterations, and each calls `flash(a)`, which
computes, **waits one frame**, then uploads. The clear and the upload therefore land in
different frames, and the port applies the pending flash (`wipe_pending`) at the top of the
next tick. Without that lag 6300, the second band's screen, is one block off.

## 5. The bars (`KOE.C:317-577`)

`dis_partstart()` is called a second time (`KOE.C:327`); the version check draws and waits on
nothing. 8100 (tick 3301) is doit3's crosshatch, before `count < 333`.

doit3's pan is `xpos & 7` on the 16-colour screen (`KOE.C:528`), so the offset is
`8*(xpos/8) + (xpos & 7) = xpos`; `(xpos & 3)*2` is the troll's 256-colour form
(`doit3s_pan_is_the_low_three_bits_of_xpos`). The `rot2 >= 32` sine divide is C's
truncating `/4`. The shrink is live for the screen's last ~45 frames, after 8100, so a
capture calibrated by its own 8100 covers it:

```bash
$ cargo xtask reference capture --arg 2 --boot autoexec --keys "KBD_left KBD_enter" \
    --cycles 30000 --frames 8100,8150,8160,8170,8180,8190 --out out/techno-shrink-capture
$ target/debug/secondreality --part techno --headless --dump-dir out/techno-shrink-port \
    --dump-frames 3351,3361,3371,3381,3391
```

Its 8100 is byte-identical to `reference/parts/techno/interference/frame_008100.rgb`, and the
port's ticks 3351..3391 (boot 8150..8190) are byte-identical to the capture. With the troll's
`(xpos & 3)*2` form instead, 8160, 8170 and 8190 differ by 7,993, 13,172 and 4,781 pixels
(8150 and 8180 agree, where `xpos` bit 2 is clear).

## 6. The troll (`KOE.C:588-727`)

The runtime decodes `TROLL.UP` through `sr_assets`' Origin reader, and the palette is the
file's six-bit bytes at +16; the troll keyframes at 0 (§8) pin both. `lineblit`'s planes are
what the emulator draws as one byte a pixel, so the port copies the decoded rows straight into
the surface (`Techno::troll_compose`). 8200 is the black hold, 8300 a ripple
frame and 8400 the held picture.

## 7. Decisions

No D24 divergence and no A-entry. The preserved bugs and dead paths (`bugs.md` §4, §14, index):

- `setpalarea(..., 0, 256)` uploads 512 bytes, `2*count + start` (`KOEA.ASM:1054-1055`).
- `setborder` returns before its assembly (`KOE.C:68-80`): no border, no Scroll Lock test.
- `flash(-2)` returns `pal1` without capturing or uploading (`KOE.C:82-103`); `mode16` writes
  `flash_pal` and uploads nothing (`flash_minus_two_uploads_nothing`).
- `init_interference`'s `mov cx,400` is reloaded before it is read (`KOEB.ASM:356`, `:360`).
- The KOEB plane-8 loop is behind `jmp @@OVER3` and `sizefade` stays 0 (§3).
- `patdir` only moves towards smaller offsets, so the `>= 24` wrap fires only from below 0.
- `rotate1` rotates 31,936 of 32,000 bytes (`KOEA.ASM:605-627`); the last 64 stay
  (`rotate1s_last_sixty_four_bytes_are_untouched`, §4.1).
- `pic = halloc(20000,4)` is 80,000 bytes and the segment poke folds the second 40,000-byte
  read into it (`KOE.C:317`).
- `clipanypoly`'s first edge is the pre-clipped closing edge (`cap3`'s `jmp cap35`). A
  pseudo-point first edge fails 6300.
- `_PIC1`/`_PIC2` and `_COMBG` are in the tree but not linked (`TECHNO/MAKEFILE:6-7`,
  `COMAN/MAKEFILE:3-4`); the linked externs are `_circle`/`_circle2`.
- Not modelled: the in-DOS mode restore (`if(!dis_indemo())`; `Techno` returns `Finished`).
  The source registers no copper callbacks, and nor does the port.

## 8. Verification

| capture | keyframe (tick) | screen | plane | differing px | register |
|---|---|---|---|---|---|
| `techno/interference` | 4900 (103), 5000 (203), 6300 (1501), 8100 (3301) | 320×200 | RGB | 0 each | exact |
| `techno/troll` | 8200 (3401), 8300 (3500), 8400 (3600) | 320×400 | RGB | 0 each | exact |

The rows are the rediff table's entries (`xtask/src/rediff/table.rs`), and
`the_keyframes_move_and_hold_as_the_references_do` (`tests/parts.rs`) checks D19's support:
4900, 6300, 8100 and 8300 differ from both port neighbours; 5000, 8200 and 8400 (the holds) do not.

All seven are RGB only by necessity. The 320×200 screen is EGA 16-colour, which the oracle's
indexed read refuses (`reference-indices.md` §1.7, §9). The 320×400 screen's one `.idx`, at
8200, is 128,000 bytes of index 0 (`tr -d '\0' < reference/parts/techno/troll/frame_008200.idx | wc -c`
prints 0), which says nothing about the stride; 8400's RGB witnesses the 160-byte stride. A re-capture session's 8300 was one fade step from the
tree's (`reference-indices.md` §9.5); the port matches the tree's. Reproduce:

```bash
cargo xtask rediff --only techno
cargo test -p secondreality --test parts techno:: -- --nocapture
```

**Register.** `bugs.md` §4 (`setpalarea`) and §14 (`patdir`, `rotate1`, `clipanypoly`), with
the dead paths in the index.
