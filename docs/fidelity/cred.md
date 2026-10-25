# CRED — the 21 credit screens, the font, and the three registers the iris moves

Part 19 of `part-sequence.md` §1 and `SECOND 5`'s second part (`exe22`,
`MAIN/U2.ASM:912-913`, `partsmask[5]` = `0x10`). Built from `CREDITS/`: `MAIN.C` (318 lines) over
`TWEAK.ASM` (357 — the `tw_*` library PAM and ALKU share, whose six entry points the port keeps
together and named after the original) and 21 pictures in `CREDITS/PICS/`. The reference is
`reference/parts/cred/` — 1440, 3240, 5440, 7640, 8740 (`reference-indices.md` §20.1).

The port is `bins/secondreality/src/parts/cred.rs`, registered as `cred`. The font strip is newly
curated — `assets/CREDITS/FONA.INC` — and its divergences are `APPROXIMATIONS.md` A8 (this part's
strip) and A9 (ALKU's, carried from Phase 1's A7 because this is where a font strip is next
touched).

**The acceptance comparison is exit 1, with two of the five keyframes byte-identical and three
differing** — and §7 shows what each difference is: the *shipped* build's credits, whose captions
and pictures two of those three screens do not share with the source tree. Nothing was tuned to
move that number.

## 1. What the original does

```
:37      dis_partstart()                        the version check (DIS/DISC.ASM:51-64), not a wait
:38      init()                                 the font metrics scan, and the palette repair (§4)
:39      tw_opengraph()                         mode 13h; chain-4 off; the tweak-mode CRTC (§2)
:40      tw_setstart(160*200)                   display start 32,000 bytes
:41-156  screenin(pic, text) × 21               screens 1..21, in this file's order
:158-159 //tw_closegraph(); }                   commented out: main() returns

screenin(pic, text)   (:161-213)
:165     tw_setsplit(400)                       line compare 400: below a 200-row display, inert
:166     tw_clrscr()                            clear 1,000h dwords = 65,536 pixels (§5)
:167     tw_setstart(160*200)                   32,000 bytes = 400 logical lines
:168     dis_waitb()                            one vertical blank
:169     tw_setpalette(&pic[0][16])             768 bytes: the whole DAC, from the picture's palette
:170     pic = &pic[0][784]                     16 bytes of header and the palette, then the pixels
:172     y=16; while(*(text=prtc(160,y,text))) y+=FONAY+10;   the caption, centred
:174     for(x..160) for(y..100) tw_putpixel(400+x, 400+y*2, pic[y][x]+16)
:176-192 for(y=200*128;y>0;y=y*12L/13)          the iris opening, one step a vertical blank
:194     for(a=0;a<200 && !dis_exit();a++) dis_waitb()   the hold
:196-212 for(y=0,v=0;y<128*200;y=y+v,v+=15)     the iris closing
```

`init()` (`:246-318`) is two acts and this note gives each its own section: the metrics scan
(`:250-269`, §6) and the palette repair (`:273-317`, §4). The text routines are `prt` (`:215-234`)
and `prtc` (`:236-243`), both translated literally (§6).

## 2. The display geometry, and the arithmetic the brief asks for

`tw_opengraph` (`TWEAK.ASM:18-60`) does **not** leave mode 13h alone:

```
:20-21   mov ax,13h / int 10h        mode 13h: 320x200, 256 colours, GC mode bit 6 set
:23-25   sequencer 4 = 06h           chain-4 OFF: the four planes are addressed separately
:29-34   es=0a000h; rep stosw 8000h  65,536 byte addresses x 4 planes = 262,144 pixels: all 256 KB
:36-41   CRTC 0x14 = 0, 0x17 = e3h   the address counter in bytes ("crtc long off/byte on")
:43-44   CRTC 0x13 = 5013h           the line offset: 80 words = 160 bytes
:46-47   CRTC 9 = 0109h              maximum scan line 1: twofold scan doubling
:49-52   CRTC 0x18 = 64h, CRTC 7 = 0fh   line compare 100: CRTC 7's bit 4 is clear
:54-58   AC 0x30 = 61h               the AC's mode control, PAS
```

The pen (`tw_putpixel`, `TWEAK.ASM:87-117`) writes plane `x & 3` at byte address `y*80 + x/4`. So

```
one logical line = 80 bytes = 80 x 4 pixels = 320 pixels        (four pixels a byte, one per plane)
one displayed row = 160 planar bytes = 4 planes x 40 bytes = 160 bytes
                  = 2 logical lines of 80 bytes = 640 pixels
```

A displayed row is therefore **every other logical line**, which is what the port's view says:

| field | value | why |
|---|---|---|
| `width` | 320 | `horiz_end x 4`, CRTC 0x13's 80 words |
| `height` | 200 | the vertical display end of a double-scanned mode |
| `y_line_offset` | `Some(640)` | CRTC 0x13's 160 bytes, in pixels: **twice the view's width** |
| `double_scan` | `false` | the doubling and the line offset cancel: the port folds them into the stride |
| `pan` | `0` at a hold | AC 0x13's low three bits (`yy & 3) * 2`, §3) |
| `border` | inherited | `tw_opengraph` writes AC 0x30 (the mode control), never AC 0x11 |

`tw_opengraph` leaves the line compare at `0x64` = 100 — `TWEAK.ASM:49-52` writes CRTC 0x18 = `0x64`
and CRTC 7 = `0x0f`, whose bit 4 (the compare's 9th bit) is **clear** — and the first `screenin` then
overwrites it with `tw_setsplit(400)` (`MAIN.C:165`) before any wait, so no displayed frame of this
part shows the 100.

This is the second of Part B's two conventions, and the trap it names is live in the opposite
direction: a part that set `y_line_offset` to its own width would shear the picture silently, and
this part's *stride is deliberately twice its width*. The part writes **flat** through
`Surface::pixels_mut()` at `LINE_LEN = 320` — one logical line — and the view reads it back at 640.

### Where the picture and the caption land

```
picture pixel (x, y):  tw_putpixel(400+x, 400+2y)
  pen byte address      = (400+2y)*80 + (400+x)/4
  flat pixel            = 4 * that + ((400+x) & 3)
                        = 320*(400+2y) + 400 + x
                        = 320*(401+2y) + 80 + x
```

so the picture's row 0 is in **logical line 401** — an *odd* line — at column 80, and displayed at
row 0, column 80. Its 160 columns are displayed at columns 80-239. The caption is drawn at logical
lines 16, 58, 100, … (all even), displayed from the split down, where the address counter restarts
at 0: the display shows the glyph's *even* rows, so a caption is 16 displayed rows tall and its
first line starts at displayed row 108.

## 3. The three registers the iris moves, and the state a hold shows

The iris is not a shape: it is three register writes a vertical blank (`MAIN.C:176-192`).

```c
tw_setsplit(y/128 + 200);                       /* the line compare: 400 down to 200 */
yy = 320 - y/80;
tw_setstart(160*200 + yy/4);                    /* the display start */
out 0x3c0,0x33; out 0x3c0, (yy & 3) * 2;        /* AC 0x13: the fine pan */
```

**The port's split convention** — [`split_scanline`] — is `(line_compare + 1) / 2`, and both parts
of that are the emulator's own arithmetic (`vga_draw.cpp:1586-1593`:
`(line_compare + 1) / lines_scaled`, with `lines_scaled` 2 because a double-scanned mode is
rendered single-scanned, and the register naming the last line *above* the split). For
`tw_setsplit(400)` that is row 200 — the first row *below* a 200-row display, which is what "parked"
means — and for the hold's `tw_setsplit(200)` it is row 100.

**The state a hold shows** is the state the **last** open step leaves. `y = y*12L/13` from 25,600
reaches `y = 1` in **102** steps, and that step writes:

| register | value | arithmetic |
|---|---|---|
| line compare | 200 | `y/128 + 200` = `1/128 + 200` |
| display start | 32,080 bytes | `160*200 + yy/4`, `yy = 320 - y/80 = 320` → `yy/4 = 80` |
| pan | 0 | `(yy & 3) * 2` = `(320 & 3) * 2` |

32,080 bytes is logical line **401** = 400 × 320 + 320 = **128,320 pixels**, i.e. one logical line —
`LINE_LEN` — past the parked 128,000. That single line is the whole reason the odd lines (the
picture's) are the ones displayed.

**It is checked, not fitted.** The port's screen 1 and screen 18 reproduce the reference's frames
1440 and 7640 **byte for byte** — 0 of 192,000 bytes each, `src/parts/cred.rs`'s
`the_holds_display_state_reproduces_the_references_own_frames` — and at the *parked* origin the
same state differs in 29,523 bytes of that frame: it reads the even logical lines, which hold no
picture. A split one row out moves the caption by one displayed
row and is 320 pixels of difference; the same test renders the parked state to show the difference
is the origin, not the split alone.

### The wipes, step by step

Both loops write the same three registers from the step's `y`, and the port writes them from the
loops themselves ([`Display::opening`], [`Display::closing`]) rather than from a curve that
"matches" them: the sequences are the source's own integer arithmetic

```c
for(y=200*128;y>0;y=y*12L/13)      /* open,  102 steps */
for(y=0,v=0;y<128*200;y=y+v,v+=15) /* close,  59 steps */
```

and a closed form a step out moves every hold after it. The values, `(step, y, yy, line compare,
display start in pixels, AC 0x13)`:

| step | `y` | `yy` | line compare | display start | AC 0x13 |
|---|---|---|---|---|---|
| open 0 | 25,600 | 0 | 400 | 128,000 | 0 |
| open 1 | 23,630 | 25 | 384 | 128,024 | 2 |
| open 2 | 21,812 | 48 | 370 | 128,048 | 0 |
| open 5 | 17,155 | 106 | 334 | 128,104 | 4 |
| open 101 (last) | 1 | 320 | 200 | 128,320 | 0 |
| close 0-3 | 0 … 45 | 320 | 200 | 128,320 | 0 |
| close 4 | 90 | 321 | 200 | 128,320 | 2 |
| close 10 | 675 | 328 | 205 | 128,328 | 0 |
| close 58 (last) | 24,795 | 629 | 393 | 128,628 | 2 |

Two things in that table are load-bearing and neither is obvious:

- **The display start is a byte address and the port counts pixels.** `tw_setstart` names a byte of
  a plane (CRTC 0x17 = `0xE3` is byte mode, `TWEAK.ASM:40-41`) and a byte address of this mode is
  four pixels — one per plane, `tw_putpixel`'s own arithmetic — so the start is the source's byte
  address times four. `160*200 + yy/4` bytes is `128,000 + 4·(yy/4)` pixels.
- **The pan is the register value halved.** The source writes `(yy & 3) * 2`; the emulator's
  256-colour arm is `pel_panning = (val & 0x7) / 2` (`vga_attr.cpp:225-227`), i.e. one register
  unit is **half a pixel**, in the same unit the display start is counted in (`vga_draw.cpp:1648-1651`
  builds a line's address as the CRTC start times four *plus* the pan).
  So a step's pan is `yy & 3` pixels and the effective start is **128,000 + `yy`** exactly — one
  pixel of scroll per unit of `yy`, which is what makes the two runs meet at the hold's 128,320.

The two loops meet where the source says: the open loop's **first** step writes the parked state
again (line compare 400, `yy = 0` → 128,000, no pan) and its **last** writes the hold's, which the
close loop's first four steps write too. So the state every hold frame shows is the loop's own last
step rather than a constant set beside it.

**One tick is one `dis_waitb`**, and the tick each step is *displayed* on is the step's own: tick 0
is the parked state (`:165-167`), tick 1 the tick the palette and the picture land on (`:169-174`),
ticks 2-103 the open loop's 102 steps, ticks 104-303 the hold (`:194`, which writes nothing), ticks
304-362 the close loop's 59 steps. The next screen's tick 0 re-parks the display. A screen is
therefore 363 ticks — 362 waits plus the frame already in force when `screenin` is entered — which is
the same count the timing side of this note uses.

**The `for(a=0;a<10000;a++)` delay loops (`MAIN.C:182`, `:202`) are not modelled.** They sit between
each step's `tw_setsplit` and its `tw_setstart` to keep the writes inside the vertical retrace; the
port's tick *is* the frame boundary they protect, and a spin count has no frame-visible effect that
could be measured. Nothing else of the iris is left out.

**The wipes are checked against the record, not only against themselves.** The five keyframes are
all held screens, so `reference/parts/cred` cannot see the iris at all — but `out/b5-s5-cred-map`
(a scratch capture of every 100th frame from 1240 to 8840, in the git-ignored `out/`; the 78th
index its log asks for, 8940, is refused because the screen there is ENDSCRL's) holds frames that
are, and the wipes are in them. The sweep's **77** frames divide four ways, and the test named
below asserts the division frame by frame:

| what the 77 frames are | count |
|---|---|
| the port's own holds — §20.1's samples on the twelve screens whose data is the tree's | 28 |
| the nine changed screens' own samples, which the port's hold reproduces nowhere | 21 |
| **wipe states of the tree's screens** — the table below, one state of one screen each | 16 |
| wipe frames of the changed screens (no screen's hold is any of them) | 12 |

The sixteen are the port's, byte for byte over all 192,000 bytes:

| index | state | index | state |
|---|---|---|---|
| 1240 | screen 1, open 12 | 3840 | screen 8, open 48 |
| 1540 | screen 1, close 10 | 4140 | screen 8, close 47 |
| 2340 | screen 4, open 13 | 5640 | screen 13, open 16 |
| 2640 | screen 4, close 11 | 5940 | screen 13, close 14 |
| 2740 | screen 5, open 46 | 6040 | screen 14, open 50 |
| 3040 | screen 5, close 45 | 6340 | screen 14, close 48 |
| 3440 | screen 7, open 15 | 6740 | screen 16, open 17 |
| 3740 | screen 7, close 13 | 7040 | screen 16, close 15 |

Eleven of those frames also **rule out** the other reading of `(yy & 3) * 2` — the register value
taken as pixels — by 1,534 to 46,722 bytes; the twelfth (6340) is invariant under the shift and
carries no evidence either way. The test is
`bins/secondreality/src/parts/cred.rs`'s
`the_records_transition_frames_are_the_states_this_part_writes`, `#[ignore]`d because the directory
is scratch: it is a *record* of what the release's own frames show, and a re-capture cannot land on
the same indices (a session's phase is not reproducible, D17).

## 4. The palette repair, which is part of the screen

`init()` (`MAIN.C:273-317`) rewrites every picture's 768-byte palette at part start, in two moves,
and the port does both ([`repair_palette`]):

```c
memmove(&pic1[16*3+16], &pic1[16], 768-16*3);       /* entries 0..239 -> 16..255 */
pic1[a*3+0+16] = pic1[a*3+1+16] = pic1[a*3+2+16] = 7*a;   /* for a = 0..9 */
```

The palette begins at **offset 16** of the picture's entry (`:169` uploads from there), so the
destination offset 64 is palette entry 16 and the ramp's `a*3+16` are palette entries **0-9** — the
*low* entries, which is what makes the captions grey: the caption is drawn with the strip's own
values 0-9, so it indexes the ramp. The picture is drawn at `+16`, so picture pixel `v` indexes
`v+16`, which the `memmove` has made the picture's own entry `v` — except for `v` 0-9, where the
ramp overwrote it. Entries 0-15 are left standing by the `memmove`, so the picture's colours 0-15
are reachable twice, at `v` and `v+16`; the port's copy snapshots before writing, which is
`memmove`'s overlapping-copy semantics.

`tw_setpalette` (`TWEAK.ASM:155-176`) then uploads all 768 bytes — the whole DAC — from the
repaired palette, once per screen at `:169`.

## 5. The two clears

- `tw_clrscr` (`TWEAK.ASM:62-75`): `mov cx,1000h` / `rep stosd` with the map mask open is 4,096
  dword writes = **16,384 byte addresses**, and each address carries four pixels (one per plane) =
  **65,536 pixels**. The port clears exactly that prefix of the surface. The picture band — logical
  lines 401-599 — is *past* the clear, so the previous screen's band stands until this screen's
  picture overwrites it, which it does completely (the same 160×100 at the same lines).
- `tw_opengraph` (`:29-34`): 32,768 word writes = 65,536 addresses × 4 planes = 262,144 pixels,
  every pixel the machine has. The port's whole 256,000-pixel surface. Nothing a previous part left
  can show through a credit screen, which is why the port's post-ENDLOGO surface state cannot reach
  these frames.

## 6. The font, and the caption's metrics

**Curation.** `CREDITS/FONA.INC` was not in `assets/` (only `CREDITS/PICS/` was). It is copied
there byte for byte, with its `PROVENANCE.md` row and its inventory row. What the file *is*: 3,751
`db` lines, **60,000 values**, i.e. the 1,500×40 raster the inventory's §E.8 and §E.20 measure — and
byte for byte the raster `CREDITS/INCLUDE.OBJ` links (the `asdf` segment of
`CREDITS/INCLUDE.ASM`). It is assembler source, so the part parses the `db` values itself
(`parse_strip`): the port reads the asset in the format it ships in, and a line that is not a `db`
line is refused rather than skipped.

**The metrics scan** (`MAIN.C:250-269`) walks `x` from 0 to 1,500 asking whether any of the first
`FONAY` rows has ink there, and hands each blank-to-ink-to-blank run to the next character of
`fonaorder` (`:32`, 47 characters). The strip holds **47 runs**, 447 inked columns, the last ink at
column 557; the widest run is 11 columns and the narrowest 2 (`I`, `!`, `:`, `,`, `"`), and the
routine's own blank — `fonap[32] = 1500-32`, `fonaw[32] = 8` (`:270-271`) — is a column and a width
no run provides.

**The two refusals of `sr_assets::font::Font`,** which is why the part reads the raster itself
(`APPROXIMATIONS.md` A8 has the measured difference):

1. The routine states a 32-row cell (`MAIN.C:29`) and the raster has **40** rows. Reading the first
   32 is what the source draws, and the port reads exactly those;
2. the routine's cell is **32 columns** wide and `1500/32 = 46.875`: `Font::from_image` refuses a
   cell that does not divide the strip, and a cell that *does* divide would be a geometry no line of
   the source states. The strip has no cell at all — its glyphs are the scan's runs.

**The caption** (`prt` `:215-234`, `prtc` `:236-243`): each character is `fonaw[ch]` columns from
`fonap[ch]`, `FONAY` rows each, then a two-column gap; a line is centred at `160 - Σ(fonaw[ch]+2)/2`
with the divide at the end; and `:172`'s `y += FONAY+10` puts the lines 42 logical lines apart. The
port translates all three literally, and the reference's caption regions are the check (§7).

## 7. The comparison

```bash
cargo run --bin secondreality -- --block 5 --headless --dump-frames 1440,3240,5440,7640,8740 \
    --dump-dir out/t8-cred
cargo xtask compare --reference reference/parts/cred --rendered out/t8-cred --out out/t8-cred-diff
```

```
frame 1440: 0.0000% differing (0/64000 px), max delta 0
frame 3240: 11.2563% differing (7204/64000 px), max delta 255
frame 5440: 9.5125% differing (6088/64000 px), max delta 255
frame 7640: 0.0000% differing (0/64000 px), max delta 0
frame 8740: 5.8453% differing (3741/64000 px), max delta 255
report: out/t8-cred-diff/report.json
captures: 1 measured
verdict: different
exit: 1
```

Re-run from a deleted output directory in the commit that implemented the wipes, and **the same
five numbers as the run before them** — which is what that run is for: a keyframe is a hold deep
inside its screen's 200 held frames, so the only way a wipe could move one is by leaving the display
in a state the hold does not expect.

`mode: rgb`, as `reference-indices.md` §20.2 predicts: all 82 of the reference's CRED frames were
refused by the indexed read's coherence check, so the **reference has no `.idx`** and the comparison
falls back to RGB. (The port's dumps do write theirs — `cred_001440.idx` and the rest — but indexed
comparison needs both sides.)

**The chain is part of the test, and it lands.** The run is `--block 5` for that reason: 1440 is only
reachable if ENDLOGO's duration was right. The port's CRED tick 0 is reference index **1212** —
`--dump-frames 1211,1212` writes `endlogo_001211.rgb` (384,000 bytes) and `cred_001212.rgb`
(192,000), and the dump names the part that drew each frame — and a screen is **363** ticks, so the
holds are at

| screen | keyframe | the port's hold | frames into it |
|---|---|---|---|
| 1 | 1440 | 1315-1514 | 125 |
| 6 | 3240 | 3130-3329 | 110 |
| 12 | 5440 | 5308-5507 | 132 |
| 18 | 7640 | 7486-7685 | 154 |
| 21 | 8740 | 8575-8774 | 165 |

— every keyframe inside its hold, the tightest (screen 21) with 34 frames to spare. The hold is
ticks 104-303 of a screen (tick 103 is the open loop's last step, which is the tick that *writes*
the hold's display state and so shows it too; the close's first tick is 304), so screen `k`'s hold is
`1212 + (k-1)*363 + 103` to `+302`. That margin is
what makes the comparison a comparison: §20.2 measured all five keyframes as **held**, so the port's
frames there are static screens and a two-frame phase difference cannot move a pixel.

**The residual, attributed.** The three differing frames are the three screens whose *shipped* build
the source tree does not match — **`APPROXIMATIONS.md` A10** is the register entry for it — and each
difference is localised (`bins/secondreality/tests/cred.rs` prints both regions per frame):

| index | picture band (rows 0-99) | caption (rows 100-199) | what it means |
|---|---|---|---|
| 3240 | **20,122 bytes differ** | 0 | the caption is the tree's, the *picture* is not |
| 5440 | 0 | **18,264 bytes differ** | the picture is the tree's `PIC10B.LBM`, the caption is not |
| 8740 | 5,702 bytes differ | 5,520 bytes differ | both |

- **3240 (screen 6).** The reference's band is not the tree's `PIC05B.LBM` at any palette: 8 of the
  band's 14 colours — 3,524 of its 16,000 pixels — are in **none** of the screen's 256 palette
  entries, and they are greys (`cred.rs`'s
  `the_references_sixth_screen_is_not_the_trees_picture_at_all` makes that measurement). The tree's
  pic5b is a 44-index noise; the reference's band is a greyscale starburst, and
  `reference-indices.md` §20.1 — written from the shipped build's own sheet — describes screen 6 as
  "a dotted vortex".
- **5440 (screen 12).** The picture is the tree's, byte for byte; the caption is not. §20.1's table
  reads screens 11 and 12's captions as "SKETCH - SKAVEN / GRAPHICS - PIXEL / …", and
  `grep -rn SKETCH upstream/SecondReality` finds **nothing**: the string is not in the source tree
  at all, so no port of this tree can draw it.
- **8740 (screen 21).** §20.1 records its caption as "MUSIC - SKAVEN" where `MAIN.C:154` draws
  "MUSIC - PURPLE MOTION", and its picture band differs too.

**The divergence is wider than the five keyframes, and that is measured.** The record's sweep holds
a *hold sample* for every one of the 21 screens, and all **21** of the samples of the **nine**
screens whose data is not the tree's are off it — the three above and six more (screens 2, 10, 11,
17, 19 and 20; A10 carries the readings, and
`the_records_other_holds_measure_which_else_the_release_changed` measures all 21 samples). The
remaining twelve screens' holds are the tree's, byte for byte — asserted at one sample each, which
is the other half of §3's evidence: the sweep's own frames of those screens are reproduced exactly,
at holds *and* at wipes.

**The port is faithful to the tree, and the tree is older than the shipped build.** The evidence
that this is a data divergence and not a port's error: the *same* machinery reproduces 1440 and 7640
byte for byte; the caption on 3240 matches the reference's exactly while the picture does not; and
the caption on 5440 is a string the tree never draws. Nothing here was tuned to move the number, and
no constant was fitted to a frame — the residual is what translating `CREDITS/MAIN.C` as it stands
produces.

**For Task 11's chain residual.** The port's screens are **363** ticks. The reference's own
screens are a little longer, and `reference-indices.md` §20.1's table is what says so — its hold
samples are 1340 and 1440 for screen 1 and 8640, 8740 and 8840 for screen 21, and a *hold sample* is
a frame the screen was held on, which the table also records as true of every keyframe. The last
sample of screen 1's hold to the last of screen 21's is 20 screens, so the reference's screen length
is `(8840 - 1440) / 20 = 370`, and each end is a hold *sample* rather than a hold *end*, so the
length is bracketed by ±100/20 = ±10: **360 to 380**, against the port's 363. The port's CRED
therefore runs 2 to 12 frames *ahead* per screen, and by screen 21 its hold sits 40 to 240 frames
before the reference's — well inside the 200-frame hold, which is why every keyframe is still
answered, the tightest with 34 frames to spare (the table above). Task 11's boundary measurement should
expect the port's CRED-ENDSCRL boundary to fall short of the reference's by the same kind of margin,
not at it.

**And there is one measured bracket for that boundary.** The sweep's log refuses its 78th index —
`error: the display changed geometry between frames: frame 8940 is 640x400` — so in that session
the reference's CRED still holds at 8840 and is ENDSCRL's by 8940: its last CRED frame is in
**[8840, 8939]** (8940 is the refused, 640×400 screen). The port's is **8834** (its CRED tick 0 is
reference 1212 and its last tick is 7622), i.e. **6 to 105 frames early** — the same order as the
per-screen drift above, and the number Task 11 can compare its own measurement against.

## 8. Tests

`bins/secondreality/src/parts/cred.rs` (20 unit tests — 18 of them unconditional and 2 `#[ignore]`d
on the scratch capture they read — reading the curated `assets/` and the committed `reference/`):

| test | what it pins |
|---|---|
| `the_strip_parses_to_the_raster_the_source_declares` | 60,000 values, 40 rows, values 0-9; a non-`db` line and a partial row are refused |
| `the_metrics_are_the_runs_the_strip_itself_holds` | 47 runs, 447 inked columns, per-character widths, the routine's blank |
| `the_eight_rows_the_routine_never_reads_do_not_move_a_run` | A8's measurement: the same runs over 32 rows as over 40, 92 unread ink pixels, 46 of them on displayed rows |
| `the_credit_text_is_centred_by_the_strips_own_metrics` | the line's width, its start column, per-pixel glyph placement, the two-column gap, the 42-line step |
| `the_picture_is_laid_in_at_the_coordinates_the_source_gives` | `:174`'s flat coordinates, and that the picture's first pixel is the display's row 0, column 80 |
| `the_sixth_screen_draws_pic5b_and_it_is_its_own_picture` | the screen/picture pairing, and that pic5b is not a crop of pic5 (6 of 16,000 pixels agree) |
| `the_palette_repair_moves_entries_zero_to_239_up_by_sixteen` | the `memmove`, including entries 0-15 standing and reaching `v+16` |
| `the_ten_entry_grey_ramp_lands_on_entries_zero_to_nine` | the ramp's entries, and that the caption indexes them |
| `the_hold_is_two_hundred_frames_and_it_does_not_move` | `:194`; the surface, the view and the DAC across all 200 |
| `the_part_runs_twenty_one_screens_of_the_sources_own_length` | 363 ticks a screen, 7,623 in all, then `main()` returns |
| `the_wipes_ramps_are_the_sources_own_sequences` | the two loops' own sequences, and that `HOLD_ORIGIN`/`HOLD_LINE_COMPARE`/`HOLD_PAN` are what the last step writes |
| `the_parked_split_is_below_the_display_and_the_holds_is_half_way` | the split convention at both ends, and the lower view's origin and stride |
| `the_holds_display_state_reproduces_the_references_own_frames` | screens 1 and 18 against `reference/`'s frames, 0 differing bytes |
| `the_references_sixth_screen_is_not_the_trees_picture_at_all` | §7's palette measurement for 3240: 14 colours in the band, 8 of them not in the screen's 256 entries, 3,524 pixels |
| `the_split_register_is_one_more_than_the_line_compare_odd_or_even` | §3's split mapping at 200 (the hold, measured on 1440/7640) and at 205 (the close's step 10, measured on the record's 1540) |
| `the_pan_register_is_the_low_two_bits_of_yy_doubled` | §3's pan: the register is `(yy & 3) * 2` and the pixels are `yy & 3`, the emulator's 256-colour mapping |
| `the_two_ramps_write_the_three_registers_the_source_writes` | §3's per-step table, both loops, and that the loops' own ends write the parked and the hold states |
| `the_three_registers_move_once_a_tick_through_both_wipes` | which tick each step is displayed on (2-103, 304-362), the hold writing nothing, and the lower view's cleared origin and pan at a panned step |

Run in a commit with the strip's own asset and the reference's five frames; the unit tests read
`assets/` and `reference/`, and nothing here reads `upstream/`.

Two more unit tests are **`#[ignore]`d**, because they read `out/b5-s5-cred-map` — the scratch
capture of §3, under the git-ignored `out/`, which a fresh clone does not have:
`the_records_transition_frames_are_the_states_this_part_writes` (the sixteen frames above, 0
differing bytes each; the pan measurement's eleven exclusions with their byte counts; and §3's
77-frame partition) and `the_records_other_holds_measure_which_else_the_release_changed` (§7's
divergence over all 21 samples of the nine screens, and the twelve screens that are the tree's,
0 of 192,000 bytes each). `cargo test -- --ignored` runs them where the directory is.

`bins/secondreality/tests/cred.rs` (6 tests, the real binary from the repository root): the
manifest's numbering and geometry; **the five keyframes inside the chain's own holds** — three
frames around each keyframe, byte-identical, i.e. §20.2's measurement made of the port, which says
at once that each keyframe is inside a hold and that the screen there is the one the reference
holds; the two frames that agree with the reference byte for byte; the three that differ, with both
regions' byte counts per frame; **the wipes moving the display between the holds** — two blank early
open steps, two that differ, the open loop's last step identical to the hold's first frame, and two
close steps that differ, all through the binary; and the truncated-frame guard.

**Register.** The behaviours of this part that the original does and the port must keep are in `bugs.md` the index (the inert line compare).
