# LNS&ZOOM — the face wipe, the lens and the rotazoomer

`LENS/MAIN.C` (394 lines) over `LENS/ASM.ASM` (566 lines), one binary holding
three routines run back to back (`MAIN.C:379-386`):

1. **`part1`** (`:89-131`) — the demon face wipes in with two moving edges for
   at most 300 frames, after a music wait.
2. **`part2`** (`:133-183`) — 715 frames of `drawlens`, a magnifying bubble
   steered along `LENS.EXP`'s first path, then a five-frame hold.
3. **`part3`** (`:185-276`) — the rotazoomer: the face sampled through a
   16.16 walker, rotated and zoomed, for 2000 frames, fading out from frame
   1872 and ending on a flat white palette.

The assets are `LENS.EX0` (215 bytes: the lens's dimensions and three overlay
RGB triples), `LENS.EX1..4` (19,588 / 6,860 / 1,538 / 6,252 bytes: the lens's
row tables), `LENS.EXB` (64,784 bytes: an Origin `Uh` picture, palette at +16
and 64,000 pixels at +784), `LENS.EXP` (18,864 bytes: the two paths) and
`FILLER` (the 4,224-byte pad the linker puts after `LENS.EXB`; see §6):

```text
$ wc -c assets/LENS/LENS.EX0 assets/LENS/LENS.EX1 assets/LENS/LENS.EX2 \
      assets/LENS/LENS.EX3 assets/LENS/LENS.EX4 assets/LENS/LENS.EXB \
      assets/LENS/LENS.EXP assets/LENS/FILLER
   215 assets/LENS/LENS.EX0
 19588 assets/LENS/LENS.EX1
  6860 assets/LENS/LENS.EX2
  1538 assets/LENS/LENS.EX3
  6252 assets/LENS/LENS.EX4
 64784 assets/LENS/LENS.EXB
 18864 assets/LENS/LENS.EXP
  4224 assets/LENS/FILLER
```

The part is block 3's second (`MAIN/U2.ASM:856-874`), reached after MNTSCRL. A
`--part lnszoom` run numbers its own frames from zero (`index_origin: "part"`,
`anchor: null`); the mapping the comparison uses is **boot = 2452 + tick**
(§5).

## 1. The two screens

Parts 1 and 2 are mode 13h: 320×200, one byte a pixel, row stride 320
(`MAIN.C:292`, `setvmode(0x13)`).

Part 3 changes the video mode under itself. `inittwk` (`ASM.ASM:185-238`)
clears the palette, sets CRTC 9 to 0, takes the address counter to byte mode
(`CRTC 0x14 = 0`, `0x17 = 0xe317`), sets the sequencer's memory mode and map
mask (`0x0604`, `0x0f02`), clears 64 KB with `rep stosw`, and then

```asm
;set 4 x vertical
mov dx,3d4h
mov al,9
out dx,al
inc dx
in al,dx
and al,not 31
or al,3
out dx,al
```

— maximum scan line 3. With mode 13h's 400-line vertical total that is 100
character rows of four scanlines; the emulator renders the 320×200 frame as
those hundred rows each twice (`reference/parts/lnszoom/frame_004200.rgb`'s
row pairs, asserted for both the reference and the port by
`the_five_keyframes_at_the_measured_mapping`). The 320×200 capture is
therefore 100 unique rows, not 200.

Because chain-4 is off, the rotazoomer's memory is **Mode X 256-colour
planar**: one CPU address is four plane bytes, one a pixel, and `rotate` writes
each address twice — mask `0x03` (planes 0 and 1) then mask `0x0C` (planes 2
and 3). A four-pixel group is therefore `(a, a, b, b)`: two pixels from the
pass-A sample and two from pass B's. The reference's 4200 and 4900 rows show
exactly that two-pixel repetition, and the port's composition reproduces it
(`the_rotazoomer_composes_160_samples_each_shown_twice_and_100_rows_twice`).

## 2. The init (`MAIN.C:278-372`)

- `setvmode(0x13)`, then the DAC is zeroed: `outp(0x3c8,0)` and 768 zero
  bytes (`:292-294`). The part's mode set clears the screen; the port clears
  its video buffer and uploads the generated palette at the end of `init`.
- `rotpic = halloc(16384,4)`, `fade = halloc(16000,1)`, `fade2 =
  halloc(20000,1)` (`:286-291`).
- The palette at `lensexb+16` and the picture at `back = lensexb + 784`
  (`:300-301`; `(768+16)/16 = 49` paragraphs).
- **The 1,536-byte overrun** (`:302`):
  `memcpy(back+64000, back+64000-1536, 1536)`. The destination is past the
  64,000-byte picture — the `FILLER` pad — and the source is the picture's
  last 1,536 bytes. The port's `back` is a 65,536-byte buffer whose
  `[64000, 65536)` is that copy; the lens's 16-bit source addresses reach it.
  `the_tables_are_bounded_and_the_overrun_is_the_1536_byte_copy` pins it.
- `lenswid`/`lenshig` from `LENS.EX0` and `lensxs = wid/2 = 76`,
  `lensys = hig/2 = 58` (`:303-307`). The file's first four bytes are
  `98 00 74 00` = 152, 116 (`xxd -l 16 assets/LENS/LENS.EX0`); `init`
  refuses any other dimensions.
- The three overlay banks (`:308-325`): for `i = 1..3` and each of the base
  palette's 64 entries, `palette[i*192 + a + c] = min(63, base + delta)`, with
  the triples `(0,5,15)`, `(0,7,26)`, `(0,9,37)` at `LENS.EX0`+4
  (`xxd -l 16 assets/LENS/LENS.EX0` shows `00 05 0f 00 07 1a 00 09 25`). The
  lens's OR bits select the banks.
- `fade` (`:330-347`): 64 blocks of 192 bytes, `(pal[y]*(63-x) + x*63)/63`,
  then 16 blocks of `pal[y] + (15-x)*5` clamped at 63.
- `fade2` (`:348-357`): 32 blocks of 576 bytes, `pal[y] - pal[y%192]*(31-x)/31`
  for `y` in 192..767.
- `rotpic` (`:358-368`): `rotpic[x+y*256] = back[x+32+a*320]` with
  `a = y*10/11 - 36/2` and `a = 0` when it falls outside 0..199.
- `waitb(); setpalarea(palette,0,256);` (`:370-371`).

The arithmetic is tested by `the_palette_and_fades_are_the_sources_arithmetic`
and `rotpic_is_the_squashed_face_and_rotpic90_its_rotation`.

## 3. Part 1 (`MAIN.C:89-131`)

The tables are `firfade1a[b] = (19+b/5+4) & ~7`,
`firfade2a[b] = (-(19+(199-b)/5+4)) & ~7` and
`firfade1[b] = firfade2[b] = 170*64 + (100-b)*50` (`:96-103`). The gate is

```c
if(dis_musplus>-30) while(!dis_exit() && dis_musplus()<-6) ;
dis_waitb();
dis_setmframe(0);
```

The `if` is missing its `()`, so it tests the function pointer — non-zero —
and the wait always runs. **Preserved**: the port waits unconditionally while
`musplus < -6` and does not model the dead guard.

The loop runs while `frame < 300`; while `frame < 80` it makes six copies of
the two-edge walk (`:109-127`). Each copy writes **one** picture byte per row
at each edge — `cp[x] = dp[x]` with `x = firfade1[y]>>6` and `firfade2[y]>>6`,
the arrays advancing by `firfade1a`/`firfade2a` — so the face fills in as the
two comb edges sweep, and after 80 frames the screen is the picture byte for
byte (`the_wipe_runs_three_hundred_frames_and_draws_only_the_first_eighty`
asserts it at frame 79). The source's writes land at the same linear index in
`vram` and `back`; the port writes `video[index] = back[index]` for the
visible 0..63,999 and drops the rest, which are outside the displayed screen.

## 4. Part 2 (`MAIN.C:133-183`)

`x = pathdata1[frame*2]; y = pathdata1[frame*2+1]; drawlens(x,y);` with
`LENS.EXP`'s first path: 715 frames × 2 ints at +4 (`:170-172`, `:296-297`).

`drawlens` (`:49-76`) walks the lens's 58 top rows at
`u1 = (x-76) + (y-58)*320` and its 58 bottom rows at
`u2 = (x-76) + (y+58-1)*320` reversed through table row `115-y`. For each row
four passes run, in this order:

| pass | table | value |
|---|---|---|
| `dorow`   | EX1 | `back[base+offset[i]] OR 0x40`, `count` pixels to `video[base+i]`, nothing below count 4 |
| `dorow2`  | EX2 | `back[base+source] OR 0x80` to `video[base+dest]`, `{dest, source}` pairs |
| `dorow2`  | EX3 | the same OR `0xC0` |
| `dorow3` | EX4 | `back[base+o]` copied to `video[base+o]`, `count` bytes |

`base = u + delta` with the row's delta from the table. Every address is
16-bit (`es:[bx+di]`, `ds:[bx+di]`) and wraps at 64 KB; the port uses
`wrapping_add`. The tables are `CALC.C`'s: 116 `{u16 data_offset, u16 count}`
entries, then the data at 480 (`rowbeg*2 + ymax*4`, `CALC.C:153`), each row
`[delta][offsets...]`. The table test prints the files' geometry:

```text
EX1: 19588 bytes, 103 rows drawn, data ends at 19588
EX2: 6860 bytes, 102 rows drawn, data ends at 6860
EX3: 1538 bytes, 43 rows drawn, data ends at 1538
EX4: 6252 bytes, 116 rows drawn, data ends at 6208
```

(`cargo test -p secondreality --bin secondreality the_tables_are_bounded
-- --nocapture`). Rows with a zero count (EX2/3/4) or under four (EX1) point
at the file's end and are never read; every row the blitter does read is
inside its file. The four passes are what makes the lens a warped magnifier
rather than a copy: `dorow` spreads the picture's pixels with the table's
signed offsets, and the interior copy (`dorow3`) is only the middle.

`uframe < 96` uploads `fade2 + ((uframe-32)/2)*576` to entries 64..255
(`:146-151`), so the overlays fade in over the lens's first 96 frames. The
loop ends at `uframe = 715`; a five-frame hold runs to 720 (`:179-182`). The
`a > 3` clamp (`:176`) is kept: `frame` advances by `min(waitb(), 3)` while
`uframe` advances by the whole count — they agree at one frame a tick.

The lens keyframe's residual (§7) is the only thing this act gets wrong, and
it is the picture asset, not the lens.

## 5. Part 3 (`MAIN.C:185-276`)

```c
rotpic90=back;
for(x=0;x<256;x++)
    for(y=0;y<256;y++)
        rotpic90[x+y*256]=rotpic[y+(255-x)*256];
waitb();
setpalarea(fade+64*64*3,0,64);
inittwk();
```

- `rotpic90 = back`: the rotated texture is written over the picture buffer,
  while the source `rotpic` is the separate heap copy. **Preserved** (the port
  keeps two buffers and builds the rotation from `rotpic`).
- The `setpalarea` at `:200` uploads 64 entries of `fade`'s first white-step
  block and `inittwk`'s DAC clear **erases it immediately**. **Preserved**:
  the port uploads it and then sets the whole palette black.
- `inittwk` also clears video memory and takes the screen planar (§1).

The loop (`:211-272`):

```c
while(!dis_exit() && frame<2000)
{
    if(dis_musplus()>-4) break;
    x=pathdata2[frame*4+0];
    y=pathdata2[frame*4+1];
    xa=pathdata2[frame*4+2];
    ya=pathdata2[frame*4+3];
    rotate(x,y,xa,ya);
    frame+=waitb();
    if(frame>2000-128) { a=frame-(2000-128); a/=2; if(a>63) a=63;
                         setpalarea(fade+a*64*3,0,64); }
    if(frame<16) { setpalarea(fade+(64+frame)*64*3,0,64); }
}
for(a=0;a<768;a++) palette[a]=63;
setpalarea(palette,0,256);
```

The path is 2000 frames × 4 ints at `lensexp+4+2*a` (`a` = 1430, the file's
word at +2; the file is exactly `4 + 1430*2 + 2000*8` bytes,
`the_path_data_is_the_expected_size_and_starts_where_the_c_prelude_reads`).
The break never fires in the reference's window: `reference/music/second3a`
holds `musplus = -32` (order 52, `zplus` 0) at boot 3648 and order 60 row 46
(`-18`) at 5650 — `awk -F, '$1>=3640 && $1<=5660' reference/music/second3a/trace.csv`
— so the rotazoomer runs all 2000 frames.

### 5.1 `rotate` (`ASM.ASM:274-417`)

`xpos`/`ypos` are `x`/`y << 16`; `xadd`/`yadd` are `xa`/`ya << 6`. The
direction test takes `rotpic90` and maps `(xa, ya) -> (-ya, xa)` with
`xpos/ypos` swapped and negated when `|xa| > |ya|` — the two branches sample
the same face along the same vector (the 90° rotation in the picture is
cancelled by the swapped address bytes). The displacement table is then
accumulated: 160 steps of `si += yadd`, `di += -xadd`, the high bytes of the
two 24-bit accumulators patched into the unrolled draw code.

**The ordering that matters.** In `ASM.ASM` the aspect-ratio block

```asm
;aspect ratio
mov	eax,307
mul	dword ptr cs:xadd
sar	eax,8
mov	cs:xadd,eax
mov	eax,307
mul	dword ptr cs:yadd
sar	eax,8
mov	cs:yadd,eax
```

comes **after** the displacement table has been patched and **before** the
row loop. The in-row sample steps therefore keep the uncorrected `xa/ya << 6`
and only the row-to-row base advance is scaled by 307/256. Applying the
aspect before the table (the obvious reading) is a different animation: it
misses the reference's 4200 by 40 % of its pixels. The port's order is the
source's; `the_aspect_ratio_scales_the_row_advance_not_the_in_row_steps` pins
the table against the corrected row advance.

The row loop runs `ZOOMYW = 100` rows. Each row adds the corrected vector to
`ypos`/`xpos`, takes the low bytes of their integer parts as `(y, x)`, and
samples `src[(base + disp[n]) & 0xffff]` for `n = 1..=160`. The display's 320
pixels are those 160 samples each shown twice (pixels `2n, 2n+1` from sample
`n+1`), and the 100 rows are composed onto the emulator's 200 scanlines, each
row twice (§1).

### 5.2 The palette's last act

After the loop the part writes 63 into all 768 palette bytes and uploads all
256 entries. The flat white screen — the reference's 5650 — is the palette,
not the picture: the video memory still holds the last rotazoomer frame, and
the reference's `.idx` at 5650 is a planar read of that memory (§7).

## 6. What is not modelled

- **The Scroll Lock border test** (`MAIN.C:36-47`): `waitb` calls `setborder`
  when `shiftstatus` bit 4 is set. The engine's frame has no overscan border
  and the reference was captured headless, so the test is absent. Nothing
  else reads `shiftstatus`.
- **`rotatez`** (`ASM.ASM:419-564`): byte-for-byte `rotate` plus two `add`/
  `sub` of 256 in the row loop. It is never called; the port has no path for
  it (`rotatez_is_dead`).
- **The copper**: the source registers no callbacks, and the port registers
  none.

## 7. The ticks, the entry pre-roll, and the reference

### 7.1 The mapping

A `--part lnszoom` run seeks MUSIC1 to order 45 (`shell::music_for_part`) and
`init` pre-polls `MUSIC_PRE_ROLL = 490` polls when `ctx.music_fresh()`, so the
engine's tick-0 poll lands on the reference's position at boot 2452:

```text
$ awk -F, '$1>=2452 && $1<=2456' reference/music/second3/trace.csv
2452,45,59,33,0,6,130,0,1,0
2453,45,59,33,0,6,130,0,1,0
2454,45,59,33,0,6,130,0,1,1
2455,45,59,33,0,6,130,0,1,2
2456,45,60,33,0,6,130,0,1,2
```

— `45:59`, `zplus` 1, `zframe` 0 (the part's `_dis_setmframe(0)`, `:106`) at
2452. The pole is the poll count: `play_song(45)` publishes `45:59` on the
491st poll, so 490 precede the engine's tick-0 poll
(`the_entry_pre_roll_lands_the_reference_position_at_tick_zero`, which also
asserts `musplus = -5` — the part 1 gate `<-6` passes without waiting). The
pre-roll is not just the row: at 490 the published `48:44` falls on the
trace's boot 2925

```text
$ awk -F, '$1==2925' reference/music/second3a/trace.csv
2925,48,44,52,0,3,130,0,1,351
```

— `musplus = -20`, the inter-act gate's frame. The port's acts therefore land
where the reference's did:

| tick | boot | what |
|---|---|---|
| 0 | 2452 | part 1's gate; `setmframe(0)` |
| 1 | 2453 | part 1 frame 0 (the first wipe) |
| 300 | 2752 | the wipe's 300th iteration, loop exits |
| 301 | 2753 | the `musplus < -20` gate begins |
| 473 | 2925 | the gate passes (`48:44`, `-20`) |
| 474 | 2926 | part 2 frame 0 (path frame 0) |
| 1188 | 3640 | part 2 frame 714 |
| 1189–1193 | 3641–3645 | the five-frame hold |
| 1194–1195 | 3646–3647 | the rotpic90 rotation and `inittwk` |
| 1196 | 3648 | part 3 frame 0 (path frame 0) |
| 3195 | 5647 | part 3 frame 1999 |
| 3196 | 5648 | the loop exits; the white palette |
| 3198 | 5650 | the reference's white keyframe |

`the_real_player_puts_the_acts_on_the_reference_ticks` asserts the two
load-bearing rows of that table (the lens at 474 and the rotazoomer at 1196)
with the real player.

**The one-frame setup.** The source's explicit waits between the last lens
draw (tick 1188/boot 3640) and the first rotazoomer frame are seven: the
loop's exit wait, five hold waits and `:199`'s `waitb();`. The reference's
pictures need eight — 4200 is path frame 552 and 4900 path frame 1252
(`the_five_keyframes_at_the_measured_mapping`), both exactly `T3 + frame`
with `T3 = 3648`. The rotpic90 rotation and `inittwk`'s 64 KB clear cost the
release a vblank; `ROT_SETUP_FRAMES = 2` charges the port's setup the extra
frame. This is a measured divergence registered as A25, not a reading of the
source's wait count.

### 7.2 The comparison

```bash
$ cargo test -p secondreality --test lnszoom -- --nocapture
2700 (tick 248): 3.2156 % pixels, max delta 53
3400 (tick 948): 3.0234 % pixels, max delta 48
4200 (tick 1748): 0 pixels, max delta 0
4900 (tick 2448): 2.1812 % pixels, max delta 41
5650 (tick 3198): 0 pixels
```

| keyframe | tick | reference files | differing | % | max Δ |
|---|---|---|---|---|---|
| 2700 | 248 | `.rgb` + `.idx` | 2,058 (both planes) | 3.2156 | 53 |
| 3400 | 948 | `.rgb` | 1,935 | 3.0234 | 48 |
| 4200 | 1748 | `.rgb` | 0 | 0.0000 | 0 |
| 4900 | 2448 | `.rgb` | 1,396 | 2.18125 | 41 |
| 5650 | 3198 | `.rgb` (+ an `.idx`, below) | 0 | 0.0000 | 0 |

The 4900 line prints `2.1812` because the test formats to four places; the
exact fraction is 1,396/64,000 = 2.18125 %, pinned to 0.0001 by the test.
(The reference's `reference-indices.md` §12.4 tables the same five files
byte-identical across five sessions.)

**The residual is one asset difference (D24, APPROXIMATIONS A25).** The
tree's `LENS.EXB` picture differs from the release's in 2,058 pixels of the
forehead — rows 33..91, columns 129..211, every one a small palette-index
change. `the_forehead_asset_difference_is_the_whole_residual`
(`src/parts/lnszoom.rs`) measures that set from the reference's 2700 index
plane, patches the tree's picture with those bytes, and then renders the
lens's 3400 and the rotazoomer's 4200 and 4900: **all three become
byte-identical**. The five residuals above are that picture, seen through the
wipe (2,058), the lens (1,935) and the rotazoomer (1,396). The port reads the
tree's asset, as the rules require; the release's picture is not in the tree.

**5650's `.idx` is not a picture.** The reference's plane is the tool's planar
*read of memory*: rows 0..99 are the rotazoomer's 100 character rows and rows
100..199 are the memory past them (`the_five_keyframes_at_the_measured_
mapping` pins 13,692 non-zero bytes in rows 0..99, zero in rows 100..199, 63
distinct indices). The displayed frame — the RGB file — has each character
row on two scanlines, so the port's `.idx` (the displayed, doubled plane) is a
different geometry and is not compared. The white keyframe is RGB-only, and
the index plane is evidence of the memory read and nothing about the stride.

### 7.3 D19's support

The test dumps `K-1, K, K+1` around every keyframe. 2700 and 5650 are held:
both neighbours are the same bytes. 3400, 4200 and 4900 move: both neighbours
differ. The reference's own neighbours are not in `reference/` (D19's rule:
`reference/` holds keyframes only), so the port's support measures that each
keyframe is a distinct animation state, not that the reference's neighbour
exists.

## 8. The assets, and the tests

The run reads `assets/LENS/LENS.EX0`, `LENS.EX1..4` (raw table bytes),
`LENS.EXB` (`load_path`'s Origin `Uh` decoder: the 768-byte palette at +16 and
the 64,000 pixels at +784) and `LENS.EXP` (raw path words). `FILLER` itself is
not read: the overrun's destination is past `LENS.EXB`, and the port
reproduces the copy's contents rather than the pad's `0x21` bytes.

The unit tests in `src/parts/lnszoom.rs` pin, by name:

- `the_acts_are_the_trees_lengths_and_gates` — 300/80/6, 715/720, 2000/1872/16,
  -6/-20/-4, and the setup's two frames; and
  `the_gates_wait_below_their_thresholds_and_pass_at_them` — -7 blocks and -6
  passes the first, -21 blocks and -20 passes the second.
- `the_wipe_runs_three_hundred_frames_and_draws_only_the_first_eighty` — the
  300-frame loop, the first 80 drawing, and the convergence on the picture.
- `the_lens_is_seven_hundred_fifteen_frames_and_a_five_frame_hold`.
- `the_rotazoomer_is_two_thousand_frames_and_exits_on_the_white_palette`.
- `the_music_break_ends_the_part_with_the_white_palette` and
  `exit_requested_finishes_the_act_on_that_tick` — the two exits.
- `the_entry_pre_roll_lands_the_reference_position_at_tick_zero` (490) and
  `the_real_player_puts_the_acts_on_the_reference_ticks` (ticks 474/1196).
- `the_tables_are_bounded_and_the_overrun_is_the_1536_byte_copy` — the row
  bounds, the row counts and the overrun.
- `the_palette_and_fades_are_the_sources_arithmetic` and
  `the_path_data_is_the_expected_size_and_starts_where_the_c_prelude_reads`.
- `rotpic_is_the_squashed_face_and_rotpic90_its_rotation`,
  `the_rotazoomer_composes_160_samples_each_shown_twice_and_100_rows_twice`,
  `the_aspect_ratio_scales_the_row_advance_not_the_in_row_steps`,
  `the_axis_swap_selects_rotpic90_and_the_swapped_walk` and `rotatez_is_dead`.
- `the_forehead_asset_difference_is_the_whole_residual` — the D24 proof.

The integration test `tests/lnszoom.rs` drives the real binary for the
registry row, the manifest's numbering and music, and the five keyframes.
