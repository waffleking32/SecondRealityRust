# TECHNO — the interference, the bars and the troll

`TECHNO/KOE.C` (728 lines) over `KOEA.ASM` (1,065), `KOEB.ASM` (632),
`POLYCLIP.ASM` (273) and `READP.C` (72): **one part, two screens**. The
transcription lives in `bins/secondreality/src/parts/techno.rs`; the unit
tests are beside it and the process-level test is `bins/secondreality/tests/
techno.rs`.

The part is block 2's fourth (`shell.rs`'s `PART_ORDER`; `MAIN/U2.ASM`),
reached after TUNNELI. It is
three stages and a second picture:

1. **The interference, first pass** (`KOE.C:213-216`): a music gate
   (`musplus < -4`), `setmframe(0)`, then `dointerference2` (`KOEB.ASM`):
   `resetmode13`, `init_interference`'s circles and plane, and 256 frames of
   a palette fade that ends white — the reference's 4900 and 5000.
2. **The interference, second pass and the bars** (`KOE.C:218-316`):
   `initinterference` again, `KOEA.ASM`'s `do_interference` until music frame
   925, five flashes, the 16-colour mode set, the white fill, four bar wipes
   gated on `musrow & 7 == 7`, then `doit1/2/3`'s clipped boxes on eight and
   four pages — 6300 and 8100.
3. **The troll** (`KOE.C:588-727`): mode 13h with chain-4 off, an 80,000-byte
   far buffer holding `TROLL.UP`, `READP.C`'s RLE rows blitted at 160-byte
   plane rows, the sixteen fade tables, the pan, the ripple and the final
   music wait — 8200, 8300 and 8400.

A `--part techno` run numbers its own frames from zero (`index_origin:
"part"`, `anchor: null`); the seven reference keyframes are reached by the
piecewise mapping of §8. There is no boot anchor: the first rings are a
content-probed frame, 4797 (`reference-indices.md` §1.7 and §8.5).

The assets are raw reads of the files the original links as its two externs
`_circle2` and `_circle`, plus the Origin RLE picture:

```text
$ wc -c assets/TECHNO/CIRCLE2.EGA assets/TECHNO/TROLL.UP assets/DDSTARS/PIC.EGA
  8000 assets/TECHNO/CIRCLE2.EGA
 70718 assets/TECHNO/TROLL.UP
 24000 assets/DDSTARS/PIC.EGA
```

`_circle2` is the 200×40-byte EGA planar picture (`CIRCLE2.EGA`); `_circle`
is `DDSTARS/PIC.EGA`, three interleaved 40-byte planes a row. That the second
file is the right one is pinned by the picture test below (its plane 0 and the
first file's plane 2 make the 4900 window's 7,888 cyan pixels) and by the
keyframe comparison itself.

## 1. The two screens

### 1.1 The 320×200 screen — EGA 16-colour planar

`resetmode13`/`init_interference` leave the 320×200 screen in the planar
16-colour mode: `init_interference` writes CRTC `13h = 40` (`mov ax,2813h`,
40 words = 80 plane bytes a row), and the port keeps four 64 KiB plane arrays
and composes a 320-pixel window from them:

- a pixel's colour index is the four planes' bits at one plane byte's bit,
  leftmost pixel bit 7 (`compose_ega`);
- a displayed row starts at CRTC start + `y * 80` plane bytes (later,
  `mode16` sets `13h = 20` — 20 words = 40 plane bytes — for the bars, and
  `doit3` sets it back to 40);
- every plane offset wraps at 65,536 bytes, the `& 0xFFFF` the 16-bit
  register arithmetic gives the source;
- the CRTC start address is the plane-byte address (`M_EGA`'s start × 8
  pixels = start plane bytes), and the port's `display_start` field is that
  plane-byte value.

The mode is also why every 320×200 reference frame is **RGB only**: DOSBox
re-decides the mode from the graphics controller and attribute controller,
finds the 16-colour EGA mode, and the oracle's indexed read refuses it
(`reference-indices.md` §1.7, §9). There is no `.idx` for this screen in
`reference/parts/techno/interference/` and none can be.

The bars' mode set is `KOE.C:230-258`: CRTC `13h = 20`, CRTC `0Dh = 0`,
CRTC `0Ch = 0`, an ATC read of `3DAh`, ATC index `13h` with data 0, the
palette-enable bit (`20h`), and the sequencer's map mask `0x0F` (all four
planes). The port's `mode16` performs its state side: stride 40, origin 0,
planes cleared, the first 8,000 plane bytes of each filled `0xFF` (the white
fill), and the dead `ip = flash(-2)` write.

### 1.2 The 320×400 screen — mode 13h with chain-4 off

`doit3` ends with `mov ax,13h; int 10h`, `inittwk()`, and CRTC `13h = 80`
(80 words = 160 plane bytes) (`KOE.C:588-596`). Chain-4 is off, so a CPU
address is four plane bytes and the emulator's `M_VGA` draw reads them as
four consecutive pixels. The port keeps that interleaved image *in the
surface*: 640 bytes a row, four pixels a plane byte. A `View` then reads it:

- `start_addr = (CRTC start) * 4` pixels — the plane byte is four pixels;
- `y_line_offset = 640`, because a 160-byte plane row is 640 pixels;
- `pan = (ATC 13h & 7) / 2`, because `M_VGA` halves the register value;
- the picture's first plane byte is byte 80 of each row (`lineblit` writes at
  `80 + y*160`), i.e. pixel 320 of the surface's 640.

`the_two_geometries_are_the_parts_two_screens` runs the real player and
asserts the view is 320×200 for every frame from 0 to 3395, and 320×400 with
`y_line_offset: Some(640)` and `start_addr: 0` from frame 3396 on (the
engine sizes a dumped frame at the tick's start, so the first 320×400 dump
is 3397; §8.3). `the_seven_keyframes_at_the_measured_mappings` shows the
interference frames at 320×200 and the troll frames at 320×400 in the
binary's own manifest.

There are no copper callbacks: the source registers none, and the port
registers none.

## 2. The init and the bar palette (`KOE.C:109-211`)

The 768-byte `pal` is sixteen blocks of sixteen colours. A colour's base
triple depends on the popcount of the block index `a` (`KOE.C:119-168`):

| popcount | base (R, G, B) |
|---|---|
| 0 | 0, 0, 0 |
| 1 | 38·64/111, 33·64/111, 44·64/111 |
| 2 | 52·64/111, 45·64/111, 58·64/111 |
| 3 | 67·64/111, 61·64/111, 73·64/111 |
| 4 | 83·64/111, 77·64/111, 89·64/111 |

and the block index `c` scales each channel by `(10 + c*K/9)/10` with
K = 9, 7 and 5 for red, green and blue, clamping at 63 (`KOE.C:169-189`).
`the_bar_palette_is_the_popcount_base_scaled_by_the_sources_factors` rebuilds
all 256 entries from that rule and pins three by hand: entry 1 of block 0 is
`(21, 19, 25)`, entry 1 of block 15 is `(52, 39, 45)` and entry 15 of block 2
is `(56, 48, 56)`.

`power0`/`power1` are each sixteen rows of 256 signed bytes (`KOE.C:173-211`):
row `b < 15` is `(char)(a*b/15)` for `a` the signed byte value, row 15 is the
signed byte itself. `power0` is the second pass's circle mask; `power1` is
computed and never read — the source's own dead table, kept in the struct
with `#[allow(dead_code)]`.

`vbuf = halloc(8192, 1)` is the 8,192-byte, 200×40 one-bit-per-pixel mask;
the port keeps 8,000 used bytes and the same row arithmetic.

The part's only read asset in the original's init is none: the three files
are linked into the binary. The port reads them in `init()` and checks the
raw pictures' sizes (8,000 and 24,000 bytes; `TROLL.UP` 320×400).

## 3. Stage 1 — the first pass (`KOE.C:213-216`, `KOEB.ASM`)

`while(!dis_exit() && dis_musplus()<-4);` then `dis_setmframe(0)`, then
`dointerference2`:

1. `resetmode13` (`KOEB.ASM:175-213`): wait, zero the DAC, `int 10h` mode
   13h, program the ATC's sixteen entries, the border and `32`, zero the DAC
   again.
2. `init_interference` (`KOEB.ASM:304-403`): CRTC `13h = 40` (80 plane bytes);
   `_circle2` blitted into plane 2 at `y*80` and vertically mirrored at
   `(399-y)*80`, each row forward then bit-reversed at +40; eight copies of
   the 32,000-byte plane-2 window rotated right one bit (`rotate1`); then
   `_circle`'s three planes blitted the same way into planes 0..2.
3. `do_interference` (`KOEB.ASM:405-607`): per frame, the palette mixes
   `pal0` into the two 8-triple windows (`palanimc` stepping by `patdir`,
   wrapping 0..24), `palfader += 2` capped at 512; at ≤ 256 each channel is
   `src * palfader >> 8`, above 256 it is `src + (palfader - 256)` clamped to
   63; then the MOVE advances `scrnrot` by 5 and sets the display start.

The MOVE is dead in the shipped build: `sizefade` stays 0 and its
`add ax,256` is commented out, so the sine terms are zero and the display
start is the constant `80*100 + 160/8 = 8020` with pan 0. The plane-8
drawing loop is behind `jmp @@OVER3` and never runs. The port keeps both
(`first_pass_move` multiplies by `p1_sizefade`, which is 0).

The fade is the reference's 4900 and 5000. Calling `first_pass_iteration`
101 times leaves `palfader = 202` and the cyan entry `(0, 30*202/256,
40*202/256)` at DAC entry 5; the 201st call leaves `palfader = 402`, above
256, so every channel is `src + 146` saturated at 63 — **every entry is
white whatever the picture's index**. The 256-iteration loop ends at
`framecount == 256`, which is the reference's white hold (the reference's
5000 is inside it; `the_first_pass_reaches_white_at_its_two_hundred_and_first_iteration`
pins iterations 101, 201 and 256).

### 3.1 The sine cursor is a byte offset

`KOEA.ASM`'s `interference_draw` keeps `bp` in **bytes** into the word
table: it adds `9*2` a row (`:791`) and masks with `& 2047` (`:785-786`,
`:791-792`), i.e. 1,024 words. The MOVE routines keep a word index and
write `shl bx,1` before `_sin1024[bx]` (`:837-838`, `:854-855`); the two cursors
are written differently, and that is what tells them apart. The table is
therefore exactly `SIN1024.INC`'s 1,024 words and the code after it in the
linked segment is never read. The brief's "alias tail" was this byte cursor
read as a word index; the first pass's palette made the misreading invisible
(its per-row power row is zero), and the first frame with `sinuspower == 1`
in the second pass exposed it. `the_sin_table_is_the_incs_1024_words`
rebuilds the table from the `.INC` and pins its 256 maximum.

## 4. Stage 2 — the second pass, the flashes and the wipes (`KOE.C:218-316`, `KOEA.ASM`)

`initinterference()` (the extern name; the same routine as above) is called a
second time, then `dointerference()` runs until `getmframe() >= 925`
(`KOEA.ASM:900-905`: `mov ax,0; mov bx,9; int 0fch; cmp ax,925; jae`). Each
frame:

- upload `pal1[0..24]` to DAC 0..7 and `pal2[0..24]` to DAC 8..15, with
  `palanimc` stepping by `patdir` and wrapping 0..24;
- draw the moving circle window: 200 rows of 44 bytes copied from the
  rotated circle selected by `7 - ((power0[b] - pan + overx) & 7)` into
  plane 3 at `scrnpos + row*80`, where `b` is `(sin[bp]/8 & 0xFF) |
  (sinuspower << 8)` and the source row is `80*row + overya + (ax >> 3)`;
- advance `scrnrot` by 5 and `overrot` by 7, set the display start and pan;
- after 350 frames, advance `powercnt` every frame and `sinuspower` every
  16 (a 16-step ramp capped at 15).

The port transcribes all of it (`second_pass_iteration`, `interference_draw`)
including the `& 2047` sine indices.

Then five flashes and the mode set of §1.1:

```c
flash(-1); flash(32); flash(64); flash(192); flash(256);
```

`flash(-1)` reads DAC entries 0..15 into the static `pal1`; `flash(i)` computes
`(pal1[a]*(256-i) + 63*i) >> 8`, **waits one frame**, then uploads 48 bytes to
DAC entry 0. `flash(-2)` is the `if(i==-2) ;` no-op that returns `pal1` and
nothing else — kept as written. The port's flash phases apply one value a
tick, which is the wait's effect at the engine's one-frame tick.

### 4.1 The second pass's display pipeline

`do_interference`'s frame draws the 44-byte circle window at the start
position the previous MOVE left, and the MOVE writes CRTC `0Ch/0Dh` and the
ATC pan **after** the draw. The emulator's rendered frame uses the register
state from before that write: the first second-pass frame (a live capture at
boot 5058) shows the overlay drawn at the static's 0 through the first
pass's 8,020 — the overlay's visible rows are its 100..199, which is what
the reference's plane-3 colouring shows. The port therefore lags the start
by one iteration (and the pan, which the source writes at the top of the
next iteration), with the first frame's 8,020 as the exception. This is
invisible in the first pass because its start is the constant 8,020.

The second pass has no keyframe in `reference/parts/techno/` (the tree's four
320×200 frames are the first pass and the bars), so it was checked against
live captures during the port and those are the evidence for the rule:

The captures are four sessions (all with the same prefix
`cargo xtask reference capture --arg 2 --boot autoexec --keys "KBD_left
KBD_enter" --cycles 30000`):

```text
--frames 5050,5054,5056,5058,5060,5062,5070                  --out out/techno-secondpass-start
--frames 5000,5400,5500,5600,5700,5800,5900,6000,6300         --out out/techno-secondpass-cal
--frames 5410,5420,5430,5440,5450,5460,5470                   --out out/techno-secondpass-mid
--frames 5421,5422,5423,5424,5425,5426,5427,5428,5429,5430,5431,5432
                                                              --out out/techno-secondpass-dense
```

The `cal` session is calibrated by its own 5000 and 6300: `sha256sum` of
those two RGB files equals the tree's (`reference-indices.md` §9.2's
hashes), so its boot indices are the tree's. At `tick = boot - 4799`, a
single dump,

```bash
$ target/debug/secondreality --part techno --headless \
    --dump-frames 259,261,263,271,601,611,621,622,623,624,625,626,627,628,629,630,631,632,633,641,701,801,1101 \
    --dump-dir out/techno-secondpass-port
```

compares equal (`cmp`, no differing byte) for all 23 frames it names: 5058
(tick 259), 5060/5062/5070 (261/263/271), 5400 (601), 5410/5420/5430/5440
(611/621/631/641), 5421..5432 (622..633), 5500 (701), 5600 (801) and 5900
(1101). 5058 is the first second-pass frame (its overlay is drawn at 0 and
shown through 8,020); 5424 is the first frame where `sinuspower` is 1 and
the byte cursor matters. Before the byte-offset fix, 5424 differed by
**11,205 pixels**; the committed port reports 0. The captures and the dump
are in `out/` (git-ignored); the committed acceptance remains the seven
keyframes of §9.

### 4.2 `waitborder` and the four bar wipes (`KOE.C:262-316`)

`waitborder` reads the player's row; when it changes and `row & 7 == 7` it
arms block 15; every call uploads the current block (48 bytes to DAC 0..15)
and decrements the block index to a floor of 0. `the_waitborder_slides_the_
bar_blocks_and_resets_on_row_seven` pins the slide and the floor.

Each of the four bands runs `for(a=256;a>-400;a-=32)` — 21 iterations. Each
iteration clears a growing triangle of ten-byte columns down the band and
calls `flash(a)` (`flash(0)` below zero). Because `flash` waits after
computing, the clear and the upload land in different frames; the port models
that with a pending flash (`wipe_pending`) applied at the top of the next
tick. This is the difference between a matching and a one-block-off 6300 —
the palette upload must lag the clear by the frame the source waits.

After a band, the source waits for `musrow & 7 == 7` again, captures the
palette, and flashes 32/64/192/256. The port's phases are `Wipe`,
`WipeRowWait`, `WipeFlash`; the transition table in §8.1 is the measured
timeline. 6300 is the second band's screen.

The five flashes after `dointerference` use the same pipeline; the section
test records their ticks (1247..1251). `mode16` then clears and fills white.

## 5. The bars — `doit1`, `doit2`, `doit3` (`KOE.C:317-577`)

`dis_partstart()` is called a second time (`KOE.C:327`, preserved), then
`doit1(70*6 = 420)`, `doit2(70*12 = 840)`, `doit3(70*14 = 980)`. Each routine:

- `waitborder()` first (the port's `doit_first` tick uploads the armed block
  and reads the row without drawing);
- per iteration, `count -= waitborder()`; the port decrements one a tick,
  which is `waitb` returning 1 on the emulator's 70 Hz clock;
- clear `vbuf`, draw eleven sinusoidal parallelograms (`c = -10, -8, …, 10`)
  through `asmbox`, advance `rot`, `vm`, `vma` (and `rota` in doit2/3);
- select the plane mask (`pl`), fill the page with `asmdoit` (doit1/2,
  `blit16`, row step 40) or `asmdoit2` (doit3, `blit16b`, row step 80);
- set CRTC `0Ch` to the page, advance `plv`, move `vram`, double `pl` at the
  wrap, `setborder(0)`.

**The page flips.** doit1/2 use eight pages: `plv = 0..7`,
`vram = 0xa0000000 + 0x2000000*plv` (0x200 paragraphs = 8,192 plane bytes),
CRTC `0Ch = plv*0x20` (so the start register is `plv*0x2000` plane bytes),
and the one-bit plane mask doubles 1→2→4→8→1 at every wrap. doit3 uses four:
`plv += 2; plv &= 7` (values 0, 2, 4, 6), CRTC `0Ch` the same expression, and
the pointer `+0x2000000*plv + 40` — the +40 plane bytes is the crosshatch's
x offset, carried through the start register's low byte.

**The geometry.** `drawline` (`KOEA.ASM:170-370`) is the source's y-sorted
DDA with unrolled mask states, XORing `0x80 >> (x & 7)` into `vbuf`; it skips
horizontal lines and drops its last row. `asmbox` (`KOEA.ASM:373-419`) clips
the four corners with `POLYCLIP.ASM`'s `clipanypoly` and draws the closed
polygon. `clipanypoly`'s first edge is the **closing** edge pre-clipped
(`cap3` ends with `jmp cap35`, which skips the first loop iteration's
clipping) — modelling it as a pseudo-point, as one earlier attempt did,
produces a sparse picture and fails 6300 immediately.
`the_box_renderer_fills_the_clipped_polygon` pins the filled rectangle and
the half-open right edge.

**The fill.** `blit16`/`blit16b` read `_blit16t[byte ^ carry]` and write the
two table bytes; the low byte is the prefix XOR, the high byte is the chain
carry (`(low & 1) << 7`). The port rebuilds the table from that rule
(`blit_table`) and applies it to the 8,000-byte `vbuf` row by row. 8100 is
doit3's crosshatch.

**doit3's ending.** The loop reads `getmframe()`: above 2520 it forces
`xpos` to 0 on one frame and breaks on the next; from `count < 333` it
shrinks `xpos` by `xposa/4` until 0. `xpos` also drives the display start's
low byte (`xpos/8`) and the ATC pan (`xpos & 7`, `KOE.C:528`; the
`(xpos & 3)*2` form is the troll's 256-colour screen), so the total pixel
offset is `8*(xpos/8) + (xpos & 7) = xpos`. The port's `DOIT3_MFRAME = 2520`
and `DOIT3_COUNTDOWN = 333`; the `rot2 >= 32` sine divide is C's truncating
`/4`, not an arithmetic shift — an arithmetic shift moved the negative-sine
edges by a pixel in the port's first build, a then-measurement of 4,552
differing pixels at 8100 that the committed build does not reproduce (8100
is byte-identical).

The shrink is live for the last ~45 frames of the screen, and no committed
keyframe samples it: 8100 is frame 3301, before `count < 333` begins. A
logged capture covers it and is calibrated by its own 8100:

```bash
$ cargo xtask reference capture --arg 2 --boot autoexec --keys "KBD_left KBD_enter" \
    --cycles 30000 --frames 8100,8150,8160,8170,8180,8190 --out out/techno-shrink-capture
```

Its 8100 is byte-identical to `reference/parts/techno/interference/frame_
008100.rgb` (0 differing bytes, `cmp`). At `tick = boot - 4799` the port's
8150/8160/8170/8180/8190 dumps (ticks 3351/3361/3371/3381/3391) are all
byte-identical to the capture. The troll's `(xpos & 3)*2` form differs by
7,993 pixels at 8160, 13,172 at 8170 and 4,781 at 8190 (and happens to
agree at 8150 and 8180, where `xpos` bit 2 is clear). The differential
command is the same dump plus the same `cmp`;
`doit3s_pan_is_the_low_three_bits_of_xpos` pins the formula itself.

## 6. The troll (`KOE.C:588-727`)

1. `mov ax,13h; int 10h`, `inittwk()`, CRTC `13h = 80` (160 plane bytes),
   `vram = 0xa0000000`.
2. `pic = halloc(20000, 4)` — **80,000 bytes**, then `palfade = halloc(13000,
   1)`. Two 40,000-byte reads of `troll.up` with `add word ptr p[2], 40000/16`
   between them (the far pointer's segment is moved 2,500 paragraphs), i.e.
   the file in one 80,000-byte buffer.
3. `readp(palette, -1, pic)` reads the file's 768-byte palette; for each of
   400 rows `readp(rowbuf, y, pic)` decodes the Origin RLE row and
   `lineblit(vram + 80 + y*160, rowbuf)` writes pixel x to plane x&3 at byte
   x>>2 of the 160-byte row, starting 80 plane bytes in.
4. The sixteen fade tables: `palfade[y*768 + a] = palette[a] + (45 - 3y)`,
   clamped at 63.
5. `dis_waitb(); setpalarea(palette, 0, 256);` — **512 bytes** of the 768,
   the `2*count + start` bug already registered in `bugs.md` §4.
6. The order/row wait: `(order > 35) || (order == 35 && row > 48)`.
7. 300 counts of the pan: `xpos += xposa/4`, capped at 320, CRTC `0Dh` low
   byte = `xpos/4`, ATC `13h` = `(xpos & 3)*2`, one wait a count.
8. 50 frames of the ripple: `ripplep = ripplep*5/4` capped at 1024,
   `xpos = 320 + sin1024[ripple & 1023]/ripplep`, `ripple += ripplep + 100`,
   and the sixteen fade tables uploaded one a frame (also 512 bytes each).
9. The final wait: `musplus > -6 && musplus < 16`, then mode 3 in DOS if not
   in-demo; the port returns `Finished` and lets the shell decide.

The port's `build_troll` writes the decoded picture into the four planes with
the same `lineblit` and keeps the `palfade` tables; `troll_view` sets the
`View`'s start, pan, 640-byte line offset and 320×400 size; `troll_compose`
copies the 320-pixel rows to surface columns 320..639, which is where the
CRTC's plane byte 80 lands in the interleaved image.

**The RLE and the 80,000-byte buffer.** The runtime decodes `TROLL.UP`
through `sr_assets`' Origin reader (`load_path`), which yields 400 rows of
320 bytes and the 256-entry six-bit palette. `readp_rows` in `parts/
techno.rs` is the literal `READP.C` transcription (16-bit fields, the
in-place header hack, the 80,000-byte pad), kept under `#[cfg(test)]`;
`the_troll_rle_is_the_origin_readers_rows` decodes the file both ways and
asserts row-for-row equality, and `the_troll_palette_is_the_files_own_six_bit_
palette` pins the palette to the file's bytes at +16.

## 7. The preserved bugs

Each is required behaviour, not a defect:

- **`setpalarea`'s byte count** (`KOEA.ASM:1054-1055`, `bugs.md` §4): `shl
  cx,1; add cx,ax` where `ax` still holds `start`, so `(..., 0, 256)` uploads
  512 bytes. The port's `set_pal_area` writes `2*count + start` bytes;
  `the_palette_upload_is_two_bytes_a_colour_and_truncates_at_512` pins both
  the 512 and the general rule (start 4, count 2 → 8 bytes at offset 4).
- **`setborder` is dead** (`KOE.C:68-80`): it returns before its assembly.
  The port keeps no border work; the calls are represented by nothing.
- **`flash(-2)`** (`KOE.C:82-103`): returns `pal1` without capturing or
  uploading. `mode16` writes the 48 bytes into `flash_pal` and uploads
  nothing, exactly as the source's dead write.
- **the overwritten `mov cx,400`** in `init_interference`: the register is
  reloaded before it is read; the port has no equivalent loop count.
- **the dead KOEB plane-8 loop and `sizefade`** (§3): the first pass's
  display start is constant.
- **`patdir` only moves towards smaller offsets**: it is set to -3 at rows
  ≡ 0 or 4 (mod 8) and never to +3, so the `>= 24` wrap only fires from
  below 0. The port keeps the negative-only arithmetic.
- **`rotate1`'s last 64 bytes** (`KOEA.ASM:605-627`): `cx = 32000/32 - 2`
  rotates 31,936 bytes; the last 64 stay as the destination held them. The
  port copies the source's 16-bit rotate including its chained carry; the
  first carry comes from the caller's flags and is zero on the shipped path,
  which the second pass's exact captures (§4.1) confirm for all eight
  rotations.
- **`pic = halloc(20000, 4)`** is 80,000 bytes and the segment poke folds the
  second 40,000-byte read into it (§6).
- **`clipanypoly`'s first edge** is the pre-clipped closing edge (§5).

The brief's `_PIC1`/`_PIC2` and `_COMBG` objects **exist in the tree but are
not linked**: `TECHNO/_PIC1.OBK` and `_PIC2.OBK` (symbols `_pic`/`_pic2`) and
`COMAN/_COMBG.OBK` (`_combg`, with `COMBG.LBM/UH/X`) are present, but
`TECHNO/MAKEFILE:6-7` has `data=_circle.obk _circle2.obk` with
`#_pic1.obk _pic2.obk` commented out, and `COMAN/MAKEFILE:3-4`'s link lists
do not name `_combg.obk`. The port reads the two linked externs
`_circle`/`_circle2` (above); the unlinked pictures are not part of the
binary and there is nothing to model.

## 8. The gates, the entry pre-roll and the ticks

### 8.1 The gates

| gate | source | value |
|---|---|---|
| stage 1 start | `KOE.C:213` | `musplus < -4` |
| interference end | `KOEA.ASM:900-905` | `getmframe >= 925` |
| first bars wait | `KOE.C:275-281` | `musrow & 7 == 7` |
| second stage start | `KOE.C:284` | `musplus < -3` |
| doit3 shrink | `KOE.C:495-501` | `getmframe > 2520` (and `count < 333`) |
| troll order wait | `KOE.C:638-648` | `order > 35 || (order == 35 && row > 48)` |
| troll exit | `KOE.C:705-726` | `-6 < musplus < 16` |

The port's constants are the same numbers; `the_sections_land_on_the_
reference_ticks` asserts the whole timeline with the real player (§8.2), which
is what proves the gates are read at the same frames the reference's were.

### 8.2 The entry pre-roll

A `--part techno` run seeks MUSIC1 to order 18 (`shell::music_for_part`) and
`init` pre-polls `MUSIC_PRE_ROLL = 253` polls when `ctx.music_fresh()`. The
engine's tick-0 poll then publishes the trace's part-start position:

```text
$ awk -F, 'NR==1 || $1>=4792 && $1<=4804' reference/music/second2/trace.csv
frame,ord,row,pat,loop,speed,tempo,zinfo,zplus,zframe
4792,18,59,3,0,3,130,0,1,2732
4793,18,59,3,0,3,130,0,1,2732
4794,18,60,3,0,3,130,0,1,0
4795,18,60,3,0,3,130,0,1,1
4797,18,60,3,0,3,130,0,1,2
4798,18,60,3,0,3,130,0,1,2
4799,18,61,3,0,3,130,0,1,3
4800,18,61,3,0,3,130,0,1,4
4801,18,61,3,0,3,130,0,1,5
4802,18,61,3,0,3,130,0,1,5
4803,18,62,3,0,3,130,0,1,6
4804,18,62,3,0,3,130,0,1,7
```

`the_entry_pre_roll_lands_the_reference_position_at_tick_zero` asserts the
pre-roll's outcome: after 253 polls the published position is `18:60` and
`musplus = -4`, so the stage-1 gate (`< -4`) passes on tick 0 without
waiting, and the engine's tick-0 poll leaves both where they were. The
source's `dis_setmframe(0)` frame is 4794 in the trace, `zframe` 0.

The port also polls twice more during its first two ticks (`extra_polls = 2`):
the emulator's CPU-bound preamble — `dis_partstart`, the two mode sets, the
DAC and video clears — spends six rendered frames (4794..4800) over the
part's first three ticks, and the player's clock advances on every rendered
frame. The two extra polls put the player's clock where the reference's is
when the first pass starts, which is what aligns all of the row-gated
sections; the keyframes are the evidence, not the arithmetic.

### 8.3 The ticks

`the_sections_land_on_the_reference_ticks` runs the real player for the whole
part and asserts the complete transition vector (frame, phase). The frames
are the engine's — the same indices the dumps and the reference use; the
part's own `tick` counter, which `frame` increments, is one higher. The
summary:

| frame | phase | what |
|---|---|---|
| init | Gate | `musplus >= -4`; `setmframe(0)` |
| 0, 1 | ResetClear, ResetMode | the DAC, mode and circle init |
| 2 | FirstPass | the phase is set; the first fade iteration is frame 3 |
| 258 | SecondPass | after the 256th iteration, which runs on frame 258 |
| 1246..1250 | Flash32..MusplusMinus3 | the five flashes and the second gate |
| 1295 | RowWait | `musrow & 7 == 7` |
| 1303..1435 | Wipe, WipeRowWait, WipeFlash | four bands |
| 1436 | Doit1 | 420 counts on eight pages |
| 1857 | Doit2 | 840 counts on eight pages |
| 2698, 2699 | Doit3Prologue, Doit3 | mode set, 980 counts on four pages |
| 3396 | TrollWait1 | after doit3's break |
| 3398 | TrollBuild | `TROLL.UP` blitted |
| 3400 | TrollOrderWait | the black hold |
| 3445 | TrollPan | 300-count pan |
| 3498 | TrollRipple | 50-frame ripple and fades |
| 3547 | TrollFinalWait | the picture holds |
| 3967 | Done | the final `musplus` window |

The first pass's iteration `n` is frame `n + 2`: frame 3 is the first
iteration and frame 103 the 101st, which is the reference's 4900.

The dumped frame's geometry lags the view by one tick: the engine sizes a
frame from the view as the tick starts ("the geometry a part sets while a
tick runs applies from the next tick", `Engine::tick`), so the troll's mode
change during frame 3396 first produces a 320×400 dump at 3397.
`the_run_is_numbered_in_its_own_frames_and_declares_the_music_row` asserts
3396's dump is 320×200 and 3397's 320×400.

### 8.4 The mapping

The seven keyframes are reached by three affine fits on the same tick axis.
Each has slope 1 — the port's tick is one displayed frame — and the
intercepts differ because the reference's part spends its early frames in
the CPU-bound preamble and its troll mode change costs it a frame:

- **the first pass's picture: `boot = 4797 + tick`.** The fade is
  frame-quantised: the 101st iteration's palette is 4900 and the 201st's is
  5000's white, so ticks 103 and 203. 4797 is also the content-probed first
  rings (`reference-indices.md` §1.7).
- **the row-gated sections: `boot = 4799 + tick`.** 6300 is tick 1501 and
  8100 tick 3301; both sections advance on the player's clock, and the trace
  rows at those boots are the gate evidence:

```text
$ awk -F, '$1==4900 || $1==5000 || $1==6300 || $1==8100 || $1==8200 || $1==8300 || $1==8400' reference/music/second2/trace.csv
4900,20,22,9,0,3,130,0,2,78
5000,20,46,9,0,3,130,0,2,152
6300,28,16,10,0,3,130,0,2,1117
8100,35,13,46,0,3,130,0,0,2453
8200,35,38,46,0,3,130,0,0,2526
8300,35,62,46,0,3,130,0,0,2601
8400,36,23,15,0,3,130,0,0,2675
```

- **the troll's pictures: `boot = 4800 + tick`.** 8300 is tick 3500 and
  8400 tick 3600; the mode change's frame cost is the one-frame shift from
  the music fit. 8200 is the black hold and lands inside `TrollOrderWait`
  under either fit; the test uses tick 3401.

These are mappings, not anchors. The tests pin them
(`the_seven_keyframes_at_the_measured_mappings`, and the constants
`FIRST_PASS_ORIGIN`/`MUSIC_ORIGIN`/`TROLL_ORIGIN` in `tests/techno.rs`) and
the manifest still says `index_origin: "part"`, `anchor: null`.

## 9. The comparison

```bash
$ cargo test -p secondreality --test techno -- --nocapture
4900 (tick 103): 0 px 0.0000 %, max delta 0
5000 (tick 203): 0 px 0.0000 %, max delta 0
6300 (tick 1501): 0 px 0.0000 %, max delta 0
8100 (tick 3301): 0 px 0.0000 %, max delta 0
8200 (tick 3401): 0 px 0.0000 %, max delta 0
8300 (tick 3500): 0 px 0.0000 %, max delta 0
8400 (tick 3600): 0 px 0.0000 %, max delta 0
```

| keyframe | tick | screen | reference files | differing | % | max Δ |
|---|---|---|---|---|---|---|
| 4900 | 103 | 320×200 | `.rgb` | 0 | 0.0000 | 0 |
| 5000 | 203 | 320×200 | `.rgb` | 0 | 0.0000 | 0 |
| 6300 | 1501 | 320×200 | `.rgb` | 0 | 0.0000 | 0 |
| 8100 | 3301 | 320×200 | `.rgb` | 0 | 0.0000 | 0 |
| 8200 | 3401 | 320×400 | `.rgb` | 0 | 0.0000 | 0 |
| 8300 | 3500 | 320×400 | `.rgb` | 0 | 0.0000 | 0 |
| 8400 | 3600 | 320×400 | `.rgb` | 0 | 0.0000 | 0 |

**Every keyframe is byte-identical; there is no D24 divergence and nothing
new to register in `APPROXIMATIONS.md`.** The comparison asserts the frame
length first, so a truncated dump cannot pass as a matching prefix.

**All seven are RGB only, and that is not a choice.** The 320×200 screen is
16-colour EGA, which the oracle's indexed read refuses (`reference-indices.md`
§1.7, §9); the reference holds no `.idx` there. The 320×400 screen has one
`.idx`, at 8200, and it is **128,000 bytes of index 0** — a flat black frame
whose indices are consistent with the RGB and worthless as evidence about
the 160-byte stride. The test asserts its length and its all-zero contents and
does not count it as a check; the stride is witnessed by the RGB picture at
8400 instead.

**D19's support.** The test dumps `K-1, K, K+1` around every keyframe:
4900, 6300, 8100 and 8300 move (both neighbours differ); 5000 (the white
hold), 8200 (the black hold) and 8400 (the held picture) are the same bytes
on both sides. The reference's own neighbours are not in `reference/` (D19's
rule: keyframes only), so the support measures that each keyframe is a
distinct animation state in the port's own timeline.

**A reference caveat, not a port one.** `reference-indices.md` §9.5 records
that a re-capture session's 8300 was one fade step away from the tree's
committed 8300 (99.9883 % at offset -1). The port is compared against the
committed tree frame and matches it byte for byte; the observation is about
that session's timeline and is already recorded there.

`cargo xtask reference verify --tree reference` exits 0 on the tree the
comparison reads:

```text
$ cargo xtask reference verify --tree reference
...
ok   parts/techno/interference: 4 frame(s), 4 file(s)
ok   parts/techno/troll: 3 frame(s), 4 file(s)
...
verdict: ok — 35 capture(s), 116 file(s), 26963022 bytes
```

## 10. What is not modelled

- **The Scroll Lock border test**: `setborder` is dead (`KOE.C:68-80`) and
  nothing else reads `shiftstatus`. The port draws no border.
- **The intro's DOS mode restore** (`if(!dis_indemo()) { mov ax,3; int 10h }`):
  the shell owns what happens after a part; `Techno` returns `Finished`.
- **`_PIC1`/`_PIC2`/`_COMBG`**: the objects are in the tree but are
  commented out of the link (`TECHNO/MAKEFILE:6-7`) or omitted from it
  (`COMAN/MAKEFILE:3-4`), so nothing is modelled (§7).
- **The copper**: the source registers no callbacks, and the port registers
  none.

## 11. The assets, and the tests

The run reads `assets/TECHNO/CIRCLE2.EGA` and `assets/DDSTARS/PIC.EGA` as raw
bytes (8,000 and 24,000) and `assets/TECHNO/TROLL.UP` through `sr_assets`'
Origin reader. The `readp` transcription and the shared reader agree row for
row.

The unit tests in `src/parts/techno.rs` pin, by name:

- `the_bar_palette_is_the_popcount_base_scaled_by_the_sources_factors` — all
  256 entries from the popcount base and the three scale factors, plus three
  hand-pinned entries.
- `the_waitborder_slides_the_bar_blocks_and_resets_on_row_seven` — the row-7
  arm, the 16-block slide and the floor at 0.
- `the_palette_upload_is_two_bytes_a_colour_and_truncates_at_512` — the
  `2*count + start` byte count and the untouched tail.
- `the_first_pass_reaches_white_at_its_two_hundred_and_first_iteration` —
  `palfader` 202 at 101, white at 201, the 512 cap at 256.
- `the_interference_picture_is_the_circles_in_the_reference_window` — the
  4900 window: indices 0..7 and exactly 7,888 cyan (index 5) pixels.
- `the_sin_table_is_the_incs_1024_words` — the table rebuilt from
  `SIN1024.INC`, 1,024 words, maximum 256.
- `the_troll_rle_is_the_origin_readers_rows` and
  `the_troll_palette_is_the_files_own_six_bit_palette` — the RLE transcription
  against the shared reader, and the six-bit palette against the file.
- `lineblit_writes_pixel_x_to_plane_x_mod_four` — x → plane `x & 3`, byte
  `x >> 2`, at the +80 offset and 160-byte row.
- `the_box_renderer_fills_the_clipped_polygon` — `clipanypoly`/`drawline`/
  `blit16`'s fill and the half-open right edge.
- `the_second_pass_circle_walks_one_eighty_byte_row_a_row` — the 44-byte
  window's source row advances 80 bytes a row.
- `the_troll_fade_tables_add_the_sources_steps` — the sixteen tables,
  `45 - 3y`, clamped at 63.
- `the_sections_land_on_the_reference_ticks` — the complete transition
  vector of §8.3 with the real player.
- `the_two_geometries_are_the_parts_two_screens` — 320×200 on frames
  0..=3395, the view 320×400 from frame 3396 and the 640-byte line offset
  (the first 320×400 dump is 3397; §1.2, §8.3).
- `the_second_pass_reads_the_sine_as_a_byte_offset` — `sinuspower = 1`,
  each row's 44 plane-3 bytes against the word `bp/2`, more than one
  circle selected.
- `doit3s_pan_is_the_low_three_bits_of_xpos` — `xpos & 7`, with bit 2 set
  (4, 5, 13).
- `rotate1s_last_sixty_four_bytes_are_untouched` and
  `flash_minus_two_uploads_nothing` — the two dead paths.

The integration test `tests/techno.rs` drives the real binary from the
repository root for the registry row, the manifest's numbering and music
(`MUSIC1`, order 18), the two geometries, and the seven keyframes with D19's
support.
