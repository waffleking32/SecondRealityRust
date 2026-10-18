# JPLOGO — the jelly logo

`JPLOGO/JP.C` (227 lines) over `JPLOGO/ASM.ASM` (156 — `inittwk`,
`lineblit`, `setpalarea`, `linezoom`), the generated `JPLOGO/ZOOM.INC`
(16,837 lines: `@zoom0` and one unrolled routine for each even scale from
140 to 244) and `JPLOGO/READP.C` (64), with the linked `ICEKNGDM.UP`
(58,566 bytes, `_PIC.OBK`'s payload) and **JPLOGO's own** `SIN1024.INC`
(33 lines). The transcription is `bins/secondreality/src/parts/jplogo.rs`;
its unit tests are beside it and the process-level test is
`bins/secondreality/tests/jplogo.rs`.

```text
$ wc -l upstream/SecondReality/JPLOGO/JP.C upstream/SecondReality/JPLOGO/ASM.ASM \
        upstream/SecondReality/JPLOGO/READP.C upstream/SecondReality/JPLOGO/DOL.C \
        upstream/SecondReality/JPLOGO/ZOOM.INC upstream/SecondReality/JPLOGO/SIN1024.INC
   227 upstream/SecondReality/JPLOGO/JP.C
   156 upstream/SecondReality/JPLOGO/ASM.ASM
    64 upstream/SecondReality/JPLOGO/READP.C
   108 upstream/SecondReality/JPLOGO/DOL.C
 16837 upstream/SecondReality/JPLOGO/ZOOM.INC
    33 upstream/SecondReality/JPLOGO/SIN1024.INC
 17425 total
$ wc -c assets/JPLOGO/ICEKNGDM.UP
58566 assets/JPLOGO/ICEKNGDM.UP
$ grep -c '^@zoom' upstream/SecondReality/JPLOGO/ZOOM.INC
54
```

The part is block 3's seventh and last (`shell.rs`'s `PART_ORDER`), reached
after 3DSINFLD, and is registered under the demo's own name, `"jplogo"`.

**There is no boot anchor.** The reference's 320×400 screen enters in the
window `(17000, 17100]` (`reference-indices.md` §17) and the part marks no
start frame, so a `--part jplogo` run numbers its own frames from zero:
`index_origin: "part"`, `anchor: null`.

## 1. The screen: 320×400 Mode X, 80 plane bytes a row

`:173-175` is `mov ax,13h / int 10h` and `inittwk()`, and `ASM.ASM:36-74`
does the mode work: CRTC `0x09 = 0` (the comment says "400 rows": one
scanline a CRTC row), `0x14 = 0`, `0x17 = 0xE3`, the sequencer memory mode
`0x04 = 6` (**chain-4 off**, odd/even off) and the map mask `0x02 = 0x0F`.
It writes no CRTC `0x13`, so the offset stays mode 13h's 40 words = **80
plane bytes a row**. It also writes 768 zero bytes to the DAC data port
(`ASM.ASM:31-38`), blacking every entry before the part uploads its 512
bytes (§2.3). `inittwk` then clears video memory with `mov cx,32768 / rep stosw`:
each word write goes through the emulator's unchained handler, which issues
two dword stores at `((uint32_t*)linear)[addr]` — eight linear bytes a word
write — so the 32,768 words cover **all 256 KB**, not the 64 KB a plain
`rep stosw` would.

In that unchained model a CPU byte address names four plane bytes (one
dword), and the `M_VGA` display reads one linear byte a pixel; with an
80-byte CRTC row the displayed row is 320 linear bytes. The port therefore keeps a
flat 320×400 screen at 320 bytes a row and shows it with a `View` of
`width = 320`, `height = 400`, `double_scan = false` and
`y_line_offset = Some(320)` — the packed stride the surface does not have.

`scrolly` (`:30-45`) writes its argument `a = y*80` to CRTC `0x0D` (low
byte) and then `0x0C` (high byte). The emulator's `M_VGA` setup multiplies
`real_start` by four (`vga_draw.cpp:1655-1656`, `byte_panning_shift = 4`),
so the pixel origin is `y*320`: `View::set_origin(y*320)`. The low-then-high
order matters only to the hardware; the port's origin is one 16-bit display
start, which is what the register pair is.

`JP.C:176-185` then writes AC `0x11 = 0xFF` (the overscan colour, which the
port records as border entry 255) and AC `0x20` (video on).

## 2. Init: the spring, the picture, and the 512-byte upload

### 2.1 The spring tables (`JP.C:117-171`)

`frame=0; ysz=400*16; ysza=-460/6; y=0; y1=0; y1a=500; y2=399*16;
y2a=500; mika=1`, then 200 passes of the source's 16-bit `int` arithmetic:

```text
	y1 += y1a;  y2 += y2a;
	y2a += 16;
	if(y2 > 400*16) { y2 -= y2a; y2a = -y2a*mika/8; if(mika < 4) mika += 3; }
	y1a += 16;
	la = a;  a = (y2-y1) - 400*16;
	if((a&0x8000) ^ (la&0x8000)) y1a = y1a*7/8;
	y1a += a/8;  y2a -= a/8;
	/* frame > 90: */
	if(y2 >= 399*16) { y2 = 400*16; halt = 1; } else y2a = 8;
	y1 = y2 - 400*16;
```

`a` is **200** on the first pass: `for(a=0;a<200;a++)` (`:114-115`) left it
there, so `la=a` reads 200, not an uninitialised word. Every assignment is
wrapped to 16 bits (`wrap16`), and the divisions truncate toward zero, as
Turbo C's did.

The first pass and the settle are pinned by
`parts::jplogo::tests::the_spring_tables_are_the_sources_own_first_frames`:
`framey1 = [500, 885, 1172, 1323, 1317, 1153, …]`,
`framey2 = [6368, 6370, 6354, 6338, 6322, 6306, …]`, `framey1[95:] = 0`
and `framey2[95:] = 6400` exactly. The test also compares all 200 entries
of each table against a second transliteration of the same source lines —
a check on transcription, not an independent oracle; the keyframe
comparisons (§7) are the external check.

`:164-171` interpolates fourfold — `b=a/4, c=a&3, d=3-c` and
`(framey1[b]*d + framey1[b+1]*c)/3` — into the 800-entry tables `doit`
reads. At `a = 796..799` the source reads `framey1[200]`/`framey2[200]`,
one past each global. In the declaration order `:23-26` the next word is
`framey2[0]` and `framey1t[0]`; the port writes that model out. The four
values are `t1 = [0, 2122, 4245, 6368]` and `t2 = [6400, 4433, 2466, 500]`
(`the_interpolation_is_the_sources_own_and_its_overrun_is_unreachable`),
and they are **unobservable**: `doit` reads the tables only for
`frame <= 511`, and this commit's `FREEZE_AFTER` is a compile-time
assertion of that bound.

### 2.2 The picture (`READP.C`, `JP.C:187-196`)

`READP.C` reads a 16-bit-int header — magic at 0, `wid` 2, `hig` 4, `cols`
6, `add` 8 — copies the palette from `+16` for `cols*3` bytes, and steps
`row` row records from `add*16`, each a 16-bit byte count then that many
RLE bytes. A byte `>= 0x80` is a run of `b & 0x7F` copies of the next
byte. The count register is 8 bits:

```text
	mov	ah,al
	and	ah,7fh
	mov	al,ds:[si]
	inc	si
l4:	mov	es:[di],al
	inc	di
	dec	ah
	jnz	l4
```

so an opcode of exactly `0x80` writes **256** copies: `dec` wraps 0 to
255 and the loop runs the body 256 times. The shipped writers never emit
it (`sr-assets` refuses the opcode rather than inventing a meaning), and
the test `a_0x80_run_opcode_wraps_to_256_copies` pins the source's
reading.

`ICEKNGDM.UP` is `FD FC` (RLE), 320×400, 256 colours, `add = 49`
(pixels at 784). `readp_is_the_sources_row_rle_and_palette` reads the raw
file through a transliteration of `READP.C` and compares its palette and
all 400 rows with `sr_assets::load_path`'s decode: every row is 320
pixels and equal.

`:191-196` then takes **184 bytes from x=70** of each of the 400 rows and
sets byte 184 to **65** — a synthetic white border the zoom samples as
source index 184. `:188-190` blacks palette entry 64 first, and `:198-201`
remaps every source `0` to `64`: black in the artwork is drawn with entry
64, which the palette has just made black. The border's 65 is untouched.
The rows are 185 bytes, `[0..184)` crop and `row[184] = 65`
(`the_rows_are_the_184_byte_crop_remapped_with_a_65_border`).

### 2.3 The 512-byte palette upload

`setpalarea(palette,0,256)` (`ASM.ASM:104-126`) is

```text
	mov	ax,[bp+10]      ; start
	mov	dx,3c8h
	out	dx,al
	inc	dx
	mov	cx,[bp+12]      ; count
	shl	cx,1
	add	cx,ax
	rep	outsb
```

— **`2*count + start` bytes**, 512 of the 768, not a 256-entry upload.
Entries 0..169 are written whole, entry 170 receives its red and green and
then the `rep` stops, and entries 171..255 keep whatever the DAC held.
`the_palette_upload_is_the_sources_512_bytes` starts from a patterned live
palette and pins all three regions, entry 64 blacked.

**The DAC is cleared first.** `inittwk` (`ASM.ASM:31-38`) points the DAC's
write index at 0 and writes 768 zeros before `readp`, so the 512-byte upload
builds on black, not on the palette the previous part left. The source's
post-init DAC is therefore the file's entries 0..169 (entry 64 black), entry
170's red and green over a **zero** blue, and entries 171..255 **black**.
`init_clears_the_dac_before_the_512_byte_upload` drives `init` with a
patterned inherited palette and pins all three; in a `--block 3` chain this
is the DAC state that replaces 3DSINFLD's.

The upload is **invisible** on this screen: the picture's highest index is
65 (`the_picture_uses_only_colours_the_512_byte_upload_writes`), so the
partial entry and the untouched tail cannot be reached — and it is
**mandatory** all the same, because it is what the original does and what
a later part's inherited DAC depends on.

### 2.4 The entry's first state

`:203` sets every `lasty`/`lasts` to -1, `:204-206` wait a vblank, set
`scrolly(400)` and wait again, and `:207-210` draw all 400 rows once with
`linezoom(vram+y*80,row[y],184)`. In the port `init` clears the surface
(the 256 KB clear), draws those 400 rows and leaves the view at origin
`400*320` — the black start-400 screen. The initial draw does **not**
touch `lasty`/`lasts`; they stay -1 until `doit`'s first iteration.

## 3. The entry, the tick model, and the mapping

`:211-219` walks the start address in 83 steps:

```text
	a = 64;  y = 400*64;
	while(y > 0) { y -= a; a += 6; if(y < 0) y = 0; scrolly(y/64); dis_waitb(); }
```

The row sequence is `399, 397, 396, …, 6, 0`
(`the_entry_walks_eighty_three_start_rows_from_399_to_zero`). The source's
`dis_waitb` returns the copper frames since the previous call
(`DISINT.ASM:161-183`), so an entry step is one vblank; the port runs one
engine tick a displayed frame:

| tick | what it shows |
|---|---|
| 0 | the `scrolly(400)` screen `init` left |
| 1..83 | entry steps 0..82 (`scrolly(399)` .. `scrolly(0)`) |
| 84 | the `dis_waitb()` at `:221` |
| 85.. | `doit`'s iterations, its counter reading 1 on tick 85 |

The mapping is **derived from the reference pictures**, and one origin fits
all of them: tick `t` is boot frame **17065 + t**.

| reference | content | tick |
|---|---|---|
| 17300 | the filed keyframe | 235 |
| 17500 | the filed keyframe | 435 |
| 17700 | the filed keyframe | 635 |

17300 and 17500 match at ticks 235 and 435 with **0 differing pixels**
(`bins/secondreality/tests/jplogo.rs` prints them). 17700 at tick 635
reproduces the census's settled `ec3de2bb…` state and leaves the recorded
**51,445 px (40.1914 %, max delta 255)** residual against the filed
`91d6ce6b…` picture — the reference caveat of §6, not a match. The same
origin puts the sweep's 17100 at tick 35, the entry's step 34, which is the
record's "band of the picture across the top: the entry has just begun"
(`reference-indices.md` §17.1); that capture is under `out/` and is a
corroboration, not a filed reference.

The `doit` counter advances **one a tick**. The source's `frame += c`
(`:77`) adds whatever `dis_waitb` returns, and a drawn iteration can take
more than one vblank; the reference's own keyframes are reproduced by the
fixed one, which is the approximation registered as A27 in
`docs/fidelity/APPROXIMATIONS.md`. The sessions' spread at 17300/17500/17700
is the oracle's timing draw, not a second algorithm (§6).

## 4. `doit`

`:71` first waits for `dis_musplus() >= 4` — the loop condition is
`while(!dis_exit() && dis_musplus()<4);`, so it spins while the value is
below four. At this point of MUSIC1 the published order is 96, whose
`zplus` is 1 from its row 0 on (the trace: order 95 is `zplus` 0 and the
`254` at order 97 is the next marker), so its `musplus` is `row-64` — below
four; the gate opens only in order 98, whose `zplus` 2 gives `musplus = row`
from **row 4** until row 31.
With the reference's position (tick 84 is order 98 row 5) the gate passes
without a spent iteration. The port checks it once a tick until it does.

`:72-104` then, while `!dis_exit() && frame < 700`:

- `c = waitb(); frame += c;` — the port adds one (A27). `:74-76` paints the
  border 0 around the wait and 127 after it when the BIOS keyboard flags at
  0x0417 have bit 4 (Scroll Lock) set. The engine has no keyboard state, so
  the branch is carried and never taken; the unit test sets the flag and
  checks the 127.
- `if(frame>511) c=400; else { y1=framey1t[frame]/16; y2=framey2t[frame]/16; }`.
  `c` is **never read again**: the freeze is the skipped assignment, and
  `y1`/`y2` keep their previous values. The port writes that as the
  `frame <= FREEZE_AFTER` condition and a comment.
- `xsc = (400-(y2-y1))/8`.
- For every row `y`:
  - outside `[y1,y2)`: `linezoom(vram+y*80,NULL,0)` — a clear;
  - inside: `b = (long)(y-y1)*400L/(long)(y2-y1)` (signed 32-bit,
    truncating), `a = 184 + (sin1024[b*32/25]*xsc + 32)/64`, `a &= ~1`;
  - if `lasty[y] != b || lasts[y] != a`: `linezoom(vram+y*80,row[b],a)`,
    then record the pair.

`b*32/25` reads JPLOGO's sine at indices 0..510 only. `a` is evened before
the guard and before `linezoom`'s own clamp. The guard is the palimpsest:
a row whose `(b,a)` repeat is **not redrawn**, so it keeps whatever an
earlier state put there — the mechanism behind the session-specific holds
(§6). `the_doit_guard_is_a_palimpsest` paints a sentinel into a repeated
row and checks it survives, then forgets the row and checks it does not.

### 4.1 The zoom routines (`ZOOM.INC` over `ASM.ASM:128-153`)

`linezoom(dest, src, a)` clamps `a` to 318 with an **unsigned** compare
(`cmp bx,318 / jbe` — a negative 16-bit value is above 318) and clears its
low bit, then dispatches through a 160-word table indexed by `a`. `DOL.C`
generates every even scale from 140 to 244; the table maps 0 to `@zoom0`,
2..138 to `@zoom140`, 140..244 to that scale and 246..318 to `@zoom244`.
The routine's geometry is the **label's**, not the index's: a passed 318
draws at 244's `le/ri`, not at 318's.

The part's own scales never leave the generated range: over every `doit`
frame 1..511 the reachable `a` is **[164, 224]**
(`the_scale_dispatch_is_the_tables_and_the_reachable_range_stays_inside`,
which also pins 318/320/-3 → `@zoom244`, 2/141 → `@zoom140` and 0 →
`@zoom0`). The two clamps are therefore unreachable on this screen and are
carried because they are the routine's.

A routine's semantics are one row image:

- `le = 160 - a/2`, `ri = 160 + a/2`, `c = ri-le+1 = a+1`;
- for `x` in `[le, ri]`, the source byte is `((x-le)*185 + c/2)/c`, out of
  the 185-byte row; outside it, black;
- the fixed margins `[0,32)` and `[288,320)` are **never written** — `le2`
  and `ri2` come out 32 and 288 for every scale, and the margins are marked
  `EMPTY` before the plane loops;
- the generated group optimisations (`mov es:[di+x/4],ax` for eight black
  pixels under the all-planes map mask, the four-pixel form for `al`) write
  exactly the zeros the per-pixel model would.

The port's `zoom_row` is that per-pixel model — one byte per displayed
pixel into the packed surface — and
`the_zoom_window_is_the_source_bytes_at_each_scale` compares it against a
direct transliteration of `DOL.C`'s formula at scales 140, 141, 142, 144,
184, 200, 244, 318, 320 and -3, margins included (the helper is a second
transcription of the same formula, not an independent oracle; §7 is the
external check). `scale_184_is_the_identity…`
pins the entry's strip: x=68..251 are source bytes 0..183, x=252 is the 65
border, and x=67/x=253 are black.

The entry's initial draw and every `doit` row go through this one function;
the plane writes and the map-mask trick are the original's way of writing
those bytes, not a different picture.

## 5. The music: the row, the pre-roll and the gate

`shell::music_for_part("jplogo")` seeks **MUSIC1 order 96**. The filed
`second3d` trace holds order 96 across the part's whole start window
(17000..17100 are rows 50..62 of pattern 71), and the counter steps over
the `254` at order 97 into order 98 at frame 17109 — nine frames *after*
the window's end.

```text
$ awk -F, '$1 >= 17000 && $1 <= 17100' reference/music/second3d/trace.csv
17000,96,50,71,0,6,130,0,1,8368
...
17100,96,62,71,0,6,130,0,1,8442
```

`Jplogo::MUSIC_PRE_ROLL` is **483**. The player is seeked to order 96 row
0; the test counts the published row-58 window's first poll at **479**, and
the trace's row 58 starts at boot 17060 — five frames before tick 0's
17065. Tick 0's poll is the (pre-roll + 1)th, so `483 + 1 - 5 = 479`; the
test asserts both the count and that arithmetic. With 483 pre-polls the
published position is

| tick | boot | published | trace |
|---|---|---|---|
| 0 | 17065 | 96:58 | 96:58 |
| 3 | 17068 | 96:59 | 96:59 |
| 44 | 17109 | 98:0 | 98:0 |
| 76 | 17141 | 98:4 | 98:4 |
| 84 | 17149 | 98:5 | 98:5 |
| 235 | 17300 | 98:23 | 98:23 |
| 435 | 17500 | 98:48 | 98:48 |
| 635 | 17700 | 98:49 | 98:49 |

`the_entry_pre_roll_reproduces_the_traces_positions` reads
`reference/music/second3d/trace.csv` and asserts every row; the process
test asserts the manifest's `music_position` at the three keyframes is the
trace's. A mid-chain start polls nothing (`PartCtx::music_fresh`).

The gate needs `musplus >= 4`; at tick 84 it reads 5, so `doit`'s first
iteration is tick 85 and the counter reads 1. `musplus` is 4 or more only
from order 98 row 4 on, so any timing that reaches `doit` before that row
costs a few ticks — which the fixed-origin mapping above does not need.

## 6. The three phases, the holds and the 17700 filing

The part has three visual phases, all measured in `reference-indices.md`
§17:

1. **The entry** (ticks 1..83): the picture reveals top-down into the start
   address. The reference's 17100 is step 34, start 309; 17200/17300/17400
   are further in.
2. **The wobble** (`doit` counter 1..378): `y2-y1` is below 400, so
   `xsc != 0`, `a` varies with the sine per row, and the row scale combs
   the picture's edges.
3. **The hold** (counter 379 on): `framey1t[379]/16 = 0` and
   `framey2t[379]/16 = 400` (asserted by the interpolation test), so
   `xsc = 0`, every row's `a` is 184 and `b = y`. The first such iteration redraws every row that was not
   already `(y,184)`; every later iteration finds `(b,a)` unchanged and
   **draws nothing**. The screen is static from tick 463 (counter 379) to
   the part's end.

**The hold's pixels are the session's.** The palimpsest keeps rows from
earlier states, and how far the counter jumps between drawings depends on
the host's speed: `frame += c` can step past the 379..511 window in one
iteration, freezing `y1`/`y2` before the settle and leaving a different
patchwork (`reference-indices.md` §17.3 records this as the hypothesis the
block could not turn into a finding). The census is:

- **17300**: the filed picture is 4/4 — every session presents it at `N`.
- **17500**: the filed picture is 2/4 — settled, at exactly half.
- **17700**: **the state is settled 2/4 (`ec3de2bb…`) but the filed picture
  (`91d6ce6b…`) is presented by 0/4.** The block's twelve captures hold ten
  distinct byte-images at 17700, and seven re-capture probes did not draw
  the filed one (§17.3, §17.5).

The port's model is deterministic and reproduces the **settled** states:
0 differing pixels at 17300 and 17500, and at 17700 the settled state
`ec3de2bb…` — byte-identical to the census's `-sess2`/`jp-sess`. Its dumped
frame's digest is the census's own:

```text
$ cargo run -q --bin secondreality -- --part jplogo --headless \
      --dump-frames 635 --dump-dir /tmp/jp
$ sha256sum /tmp/jp/jplogo_000635.rgb
ec3de2bb823772bf275387ab122e12e17b1d27a140ec9be8a71faa7d1bd3cb37  /tmp/jp/jplogo_000635.rgb
```

which is the `ec3de2bb…` state `-sess2` and the reviewer's `jp-sess` both
hold (`reference-indices.md` §17.3), and the port's residual against the
filed picture equals `-sess2`'s exactly. The filed 17700 is outside its own census, so the
residual is the **reference's**, not the port's; this is D24's register,
recorded as A27, and the process test asserts the residual rather than
tolerating it.

```text
$ cargo xtask compare --reference reference/parts/jplogo --rendered out/b4b-jp-sess2
frame 17300: 0.0000% differing (0/128000 px), max delta 0
frame 17500: 0.0000% differing (0/128000 px), max delta 0
frame 17700: 40.1914% differing (51445/128000 px), max delta 255
```

## 7. The reference comparison

`cargo test -p secondreality --test jplogo` runs the real binary headlessly
from the repository root (`--part jplogo --headless --dump-frames …`) and
compares the dumped RGB frames with `reference/parts/jplogo/`.

```text
17300 (tick 235): 0 px 0.0000 %, max delta 0
17300 (tick 236): 0 px 0.0000 %, max delta 0
17500 (tick 435): 0 px 0.0000 %, max delta 0
17500 (tick 436): 0 px 0.0000 %, max delta 0
17700 (tick 635): 51445 px 40.1914 %, max delta 255
```

D19's support is asserted from the rendered side: 17300 and 17500 hold on
the next tick, and the tick before each is a different picture (the census
sessions' 17299 and 17499 differ from the keyframes too, §17.3). 17700 is the
settle: ticks 634, 635 and 636 are byte-identical, as the census's
17699/17700/17701 are in every session that holds the state.

**All three keyframes are RGB only.** The part writes CRTC `0x0c`/`0x0d`
during the entry and the oracle's indexed read of them is racy; no `.idx`
was filed, and the test asserts the reference directory holds none.

The run's own numbering is asserted too: `index_origin: "part"`,
`anchor: null`, width 320, height 400, MUSIC1 order 96 at frame 0.

## 8. The preserved bugs

- **The DAC clear and `setpalarea`'s 512-byte upload** (§2.3): `inittwk`'s
  768 zero bytes (`ASM.ASM:31-38`) and then the file's 512, leaving entry
  170's blue and entries 171..255 black; entry 64 blacked and the source
  0→64 remap follow. Invisible on this picture and mandatory: it is the DAC
  this part hands on.
- **JPLOGO's own sine table.** `SIN1024.INC` differs from the common copy:
  indices 256 and 768 are ±255, not ±256. The port uses this file's table;
  `the_sine_table_is_the_sources_own_1024_words` parses the shipped file
  and compares all 1,024 words.
- **The crop and the 65 border** (§2.2).
- **Source 0→64** (§2.2).
- **The interpolation's out-of-bounds read** (§2.1) — modelled and
  unreachable.
- **The freeze's unused `c=400`** (§4) — the skipped assignment, not a
  clamp.
- **The palimpsest redraw guard** (§4, §6).
- **`scrolly`'s low-then-high byte order** (§1); the port's origin is the
  16-bit display start the pair forms.
- **`doit`'s dead locals**: `halt`, `storea` and `ysb` are declared and
  never used, and `calc` (`:53-65`) is never called. Not carried.
- **`lineblit` is never called** by `JP.C`; it is a build utility in
  `ASM.ASM:76-100` and the port does not transliterate it.
- **The dead `FINAL`/`PXLSUX`-style switches**: `JP.C` has none; the
  `storea = a` and its commented-out `printf` at `:220-225` are dead.

## 9. Reproducing the numbers in this note

```bash
# source and asset counts (§ front)
wc -l upstream/SecondReality/JPLOGO/{JP.C,ASM.ASM,READP.C,DOL.C,ZOOM.INC,SIN1024.INC}
wc -c assets/JPLOGO/ICEKNGDM.UP
grep -c '^@zoom' upstream/SecondReality/JPLOGO/ZOOM.INC

# the unit tests: spring, interpolation, entry, zoom, readp, palette, pre-roll
cargo test -p secondreality --bin secondreality parts::jplogo

# the three keyframes, D19 support, the manifest and the music positions
cargo test -p secondreality --test jplogo -- --nocapture

# the census session against the filing (the 17700 number, 40.1914 %)
cargo xtask compare --reference reference/parts/jplogo --rendered out/b4b-jp-sess2

# the trace's positions at the mapped frames
awk -F, '$1 >= 17000 && $1 <= 17100' reference/music/second3d/trace.csv
awk -F, '$1 == 17300 || $1 == 17500 || $1 == 17700' reference/music/second3d/trace.csv
```
