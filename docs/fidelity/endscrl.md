# ENDSCRL — the greetings scroller: a 400-line ring, and the wrap that replaces the second copy

The demo's last part: a ring of text, one line written per two vertical blanks, the display
start walking the ring a line at a time. `ENDSCRL/MAIN.C` is 125 lines over
`ENDSCRL/ASMYT.ASM` (75), of which the part uses `setstart` (`ASMYT.ASM:24-44`) and
`setrgbpalette` (`:46-65`).

**The comparison is exit 1**, and this note says why: the reference is the **shipped
build**, and it draws this part differently from the source this tree holds — a 25-row
text pitch where `#define FONAY 30` (`MAIN.C:6`), a raster that differs from
`ENDSCRL/FONA.UH`'s, and a text buffer with no carriage returns. §7 has the numbers and
`docs/fidelity/APPROXIMATIONS.md` A11 is the register entry; §6 measures the chain's own
offset, which the same comparison also shows.

## 1. What the original does

| `MAIN.C` | what it is |
|---|---|
| `:19-21` | `dis_partstart()`, one `dis_waitb()` |
| `:22-26` | mode 0eh (640×350, 16 colours), then CRTC 9 = 0 — the **400-line** display |
| `:27-41` | fifteen `setrgbpalette` calls: entry 1 = 20, 2 = 40, **3-15 = 60** |
| `:43` | `init()`: the text into `char far text[64000]` (`:52`, `:101`) and the strip's metric scan (`:103-124`) |
| `:45-49` | `while(!dis_exit()) { if(dis_waitb()==1) dis_waitb(); do_scroll(); }` |
| `:60-96` | `do_scroll`: one line of the strip into four bit-planes, then the display start |

One line is **80 planar bytes**, and a byte of a 16-colour plane is eight pixels: 640
pixels a line (`:89-92`). `do_scroll` writes the line at `80*yscrl` **and again** at
`80*(yscrl+401)` (`:89-92`), steps `yscrl=(yscrl+1)%401` and sets the display start to
`yscrl*80 + 80` (`:93-95`). The text is therefore a **401-line ring**: one line per
iteration, the display showing the 400 lines below the write pointer.

`:47` makes the step two frames wide. `dis_waitb` returns the frames elapsed and zeroes
the counter (D22), so a part whose work fits a frame gets 1 and waits again: **one line
every two ticks**. A picture is two ticks wide, which is what the reference's own frames
measure — `reference-indices.md` §21.1, "the scroll is quantised in pairs of frames: the
picture on screen at K is usually also the picture at K+1", measured there from the
capture's own triples.

The strip's rows are drawn in turn — `line=(line+1)%FONAY` — so a line of the text takes
30 iterations and the next line of the text starts when `line` comes back to 0 (`:66-74`).

## 2. The ring: 400 lines, and the wrap that stands in for the second copy

The port's surface is 640×400 = **256,000** pixels = exactly 400 lines of 640. The
original's 401-line ring needs the second copy because the display's address counter does
not wrap at the ring's end; the port's beam does (`View::source_index` is modulo the
surface's length, `view.rs`). Three consequences, and
`the_four_hundred_line_ring_shows_what_the_four_hundred_and_one_line_ring_shows` checks all
three as bytes over a whole turn:

1. **each line is written once** — the second copy at `80*(yscrl+401)` is at ring line
   401, past this surface (256,640 pixels), and the beam's wrap replaces it;
2. `yscrl` runs `0..400` where the original's runs `0..401`;
3. the display origin is `(yscrl + 1) * 640` — 640 on the first line and **256,000** on
   the last, which is `124,930` past the largest origin the `u16` pair
   `start_addr + x_offset` could hold (`2 × 65,535 = 131,070`). Task 3 widened
   `start_addr` to `u32` for this and for CRED's 128,000 (D16).

The two pictures are not similar but **the same lines in the same rows**:

```text
original, iteration t:  writes line  t mod 401, twice (the copy at +401)
                        display start  ((t+1) mod 401) + 1  (post-increment, `:95`)
                        displayed row j shows the line written at  t - 399 + j
port,     iteration t:  writes line  t mod 400, once
                        display origin (yscrl + 1) * 640,  yscrl = t mod 400
                        displayed row j shows the line written at  t - 399 + j
```

The original's row `j` reads absolute line `((t+1) mod 401) + 1 + j`, which is ring line
`(t + 2 + j) mod 401`, most recently written at iteration `t - 399 + j` — the copy at
`+401` is exactly what makes `j = 399` the newest line rather than one that has not been
written yet. The port's row `j` reads ring line `(t + 1 + j) mod 400`, written at the same
iteration. Both put the line just written at the **bottom** row of the screen and the one
written 399 lines ago at the top.

`the_four_hundred_line_ring_shows_what_the_four_hundred_and_one_line_ring_shows` is that
equality checked as bytes, not argued: a 401-line ring written **twice** at `80*yscrl` and
`80*(yscrl+401)` with the display start at `yscrl*80 + 80`, against the port's 400-line
ring, both driven by the port's own rendered lines, with all 400 displayed rows compared at
every iteration of a 500-line run — past the port's wrap at 400 and the original's at 401.

## 3. The tick map

A tick is one `dis_waitb`, and tick 0 is the code before the first one — CRED's parked tick
is the same convention. So `main`'s acts are ticks 0, 1, 2, 3, 5, 7, …:

- **tick 0** (`:19-21`): nothing drawn; the screen is still CRED's, which is what the
  loader shows while an EXE loads.
- **tick 1** (`:22-43`): the mode set, the palette, `init`.
- **tick 2**: the loop's first wait returning 1, with the second still ahead of it.
- **tick 3** (`:48`): the first `do_scroll`, and every second tick after it.

The part never ends on its own: `while(!dis_exit())` (`:45-49`), and `dis_exit` is the DIS
server's `exitflag`, which only a key sets. `PartCtx::exit_requested` is the port's
equivalent, and the shell is what sees it.

`ctx.surface.clear(0)` at tick 1 is `:22-26`'s mode set: `int 10h`'s mode set clears video
memory (`int10_modes.cpp`'s `finish_set_mode`, `:813` and `:859`), and what it clears is
CRED's last screen — which is why the reference's own frames are black behind the text
(frame 9100 is 252,312 black of 256,000 pixels, and the part writes no background).

## 4. The strip, the metrics, and the renderer

`sr_assets::font::Font` **accepts** this strip and runs the source's own scan: `MAIN.C:103-122`
walks 1,500 columns, skips blank ones and hands each ink run to the next character of
`:13`'s `fonaorder` — `Font::from_image`'s cells *are* those runs (`font.rs`), so the
port's widths are the source's `fonaw` and not a slice of the raster at a stated pitch.
The cell it is built with, `(20, 30)`, is `MAIN.C:123`'s `fonap[32]=1500-20`: the blank is
the strip's last 20 columns, which hold no ink (the last inked column of the whole strip is
**1,008**), and `:124`'s `fonaw[32]=16` is modelled as a 16-column width that advances the
pen and draws nothing. This is the difference from CRED, whose strip `Font` refuses (A8):
that one states a 32-column cell on a 1,500-column raster, `1500/32 = 46.875`.

The measurements the port's tests pin over the raster: **74 ink runs** for the 74
characters of the order, widths **3..25** columns, ink rows **0..17**, and every pixel
value in 0..3 — a 2-bit mask, which is exactly the four plane bits `:80-87` tests (`&1`,
`&2`, `&4`, `&8` into `scanbuf[0..3]`, one plane per bit).

`:77-88` walks the line one pixel at a time, `mtau[x & 7]` (`:55`) selecting the bit of the
byte `x / 8` — most significant bit first, so the leftmost pixel of a byte is its high bit —
and the four plane writes land on the *same* byte address in four planes, so the pixel they
spell is the strip's own value 0..15 (here 0..3) and the port can write it flat.

The shipped text's own arithmetic is inside the arrays the source declares and never
checks: the longest line is **40 characters** (`textline[100]`, `:57`), and its widest line
needs `tstart = (639 - 579) / 2 = 30` with the last drawn column at 607 (`scanbuf[4][80]`,
`:58`). Both are asserted, so the port's bounds checks are never reached — and the scan's
end-of-buffer case, where the original reads past `text[64000]`, is unreachable in the
reference's own longest run: 20,000 is 184 lines in from the reference's own start of 8917
(`(20,000 - 8917 - 3) / 2 / 30`, the arithmetic the test asserts), and the shipped text holds
542.

## 5. Which text the part reads

`MAIN.C:101` opens `"endscrol.txt"` with no path. Two files of that name are in the tree,
and the part reads the shipped one: `assets/MAIN/DATA/ENDSCROL.TXT`, **6,141 bytes**, which
is what `MAIN/PACKFINA.INC:51` packs (`0x17FD`) — not the 1,007-byte
`ENDSCRL/ENDSCROL.TXT` beside the source, whose first line is "Lerto has coded this
hilariously fantastic". `docs/fidelity/asset-inventory.md` §E.9 measures the same pair.

## 6. The comparison

```
$ cargo run --bin secondreality -- --block 5 --headless --dump-frames 9100,10000,12000,14000 \
      --dump-dir out/p2-endscrl
$ cargo xtask compare --reference reference/parts/endscrl --rendered out/p2-endscrl \
      --out out/p2-endscrl-diff
frame 9100: 2.8473% differing (7289/256000 px), max delta 243
frame 10000: 11.0676% differing (28333/256000 px), max delta 243
frame 12000: 9.9617% differing (25502/256000 px), max delta 243
frame 14000: 6.6828% differing (17108/256000 px), max delta 243
verdict: different
exit: 1
```

`mode: rgb`, every frame paired at offset 0 (the reference holds no ±1 neighbours: the tool
cannot take two adjacent frames on this screen, `reference-indices.md` §21.4). Two
residuals are in that number, and they are measured separately.

### 6.1 The chain's offset — 82 frames, measured

The reference's scroller does not begin where the port's chain begins it. Its start is known
only as a window: the sweep's log shows 8840 still CRED's screen and refuses 8940 as
`640x400` (`cred.md` §7), and this part's first 640×400 frame is its tick **2** (the mode set
is tick 1, and the frame follows the view from the tick after) — so the reference's start
lies in **[8841, 8938]**, and the port's CRED ends at 8834, so this part's tick 0 is
index 8835.

`the_references_own_header_pins_the_chains_offset` measures the gap from the reference's own
picture. It uses the one thing both builds agree on — **the rows the first line of the text
is drawn on**: for its first 24 ring lines both draw the same strip rows (`FONAY` only enters
when the line counter comes round) and neither has read a control line yet (§6.2), so the
tick whose display puts "Second Reality" on the reference's own rows is the tick that
answers the reference's 9100. That tick is the port's **183**
(index 9018), and it puts the reference's start at **8917** ±1 frame (a picture is two
ticks wide, so nothing finer is observable):

```
reference's start of ENDSCRL = 8917      (tick 183 answers the reference's 9100)
port's chain starts it at      8835
chain's offset                 82 frames = 41 scrolled lines
```

82 is inside CRED's own measured bracket — "6 to 105 frames early" (`cred.md` §7), whose
cause is CRED's 2-to-12-frames-per-screen drift over 21 screens — so the offset is the
chain's, not this part's. `xtask compare --align 82` on a dump of the shifted indices shows
the same shift from the other side, and what an alignment *cannot* absorb:

```
$ cargo run --bin secondreality -- --block 5 --headless \
      --dump-frames 9018,9918,11918,13918 --dump-dir out/p2-endscrl-shift
$ cargo xtask compare --reference reference/parts/endscrl --rendered out/p2-endscrl-shift \
      --align 82 --out out/p2-endscrl-shift-diff
frame 9100: 2.1758% differing (5570/256000 px), max delta 243
frame 10000: 12.5437% differing (32112/256000 px), max delta 243
verdict: different
```

The dump is a *measurement* of where the port's frames sit, not a way to pass: the
acceptance's own command is the one above, run at `--align 0`.

### 6.2 The shipped build, measured — the reason the diff cannot be zero

Five differences, each a measurement of `reference/parts/endscrl`'s own frames against this
tree's source and assets, in the order they matter:

**1. The `[NN` control lines — the dominant term, and it is in the text.** This tree's
`MAIN.C` has no notion of one: `:66-74` reads a line to its newline and `:77-88` draws
whatever is in it, so a `[15` line is drawn as the two digits (the `[` is not in
`fonaorder`, so it is a two-column gap). The reference reads it as a **directive: advance
the ring NN rows and draw nothing** — and every other line advances it 25. The rule
reproduces the reference's own bands *to the row* on the text file this tree ships, with no
free parameter but the start measured above:
`the_references_control_lines_advance_the_ring_and_draw_nothing` predicts every band of
frames 10000 and 14000 from the tree's file and finds each one in the reference. The
readable numbers of the same measurement:

| what | measured |
|---|---|
| the shipped text's control lines | **163**, in four sizes: `[8` ×6, `[10` ×14, `[12` ×135, `[15` ×8 |
| frame 10000, the tree's own lines 16, 17, 19, 20, 22 | ring rows **0, 25, 65, 90, 130** — a `[15` between each pair, so each step is **40 = 25 + 15** |
| frame 14000, the personal greetings | **six** `[12` lines on screen, each a **37 = 25 + 12** step (`Lord Cyrix / Access Denied` sits across one) |

The `[10` and `[8` lines are *not* visible in any of this part's four reference frames —
their first is text line 430 and the deepest line frame 14000 reaches is 108 — so their 35-
and 33-row steps cannot be measured from what is committed, and neither the note nor the
test claims them.

**2. The text pitch is 25 rows a line, not 30.** `MAIN.C:6` is `#define FONAY 30` and `:94`
steps `line=(line+1)%FONAY`, so a line of text is 30 ring lines — 30 displayed rows — apart
in the tree's source. The reference's frame 12000 — whose bands §21.2 identifies as the
"Future Crew greets the following groups" block, twelve whole lines — has gaps of
25, 100, 50, 25, 25, 25, 25, 25, 25 rows between them: every gap a whole multiple of 25,
the smallest 25, which cannot be one line's fragments because a line's ink is 18 rows tall.
`the_references_text_runs_at_twenty_five_rows_a_line_where_the_source_says_thirty` pins that
list, and the same measurement on the port's own frame gives gaps that are multiples of 30
and never 25.

**3. The shipped `fonaorder` names six characters the tree's does not.** The tree's order
(`MAIN.C:13`) is 74 characters; the shipped build's is **80** — the packed
`MAIN/DATA/ENDSCRL.EXE` carries the tree's own tail followed by the six it adds:

```
$ xxd -s 9286 -l 40 upstream/SecondReality/MAIN/DATA/ENDSCRL.EXE
00002446: 3637 3839 213f 2c2e 3a8f 8f28 292b 0000  6789!?,.:..()+..
00002456: ff87 2d2a 3d27 5a84 9459 2f26 0065 2201  ..-*='Z..Y/&.e".
```

`Z`, `Y`, `/`, `&`, and the two high bytes `\x84` (`ä`) and `\x94` — the blob is
PKLITE-packed, so the run is interrupted by four bytes of the stream between `()+` and
`-*='`. The shipped text uses five of them: `/` 83 times, `&` 26, `Z` 12, `Y` once, and `ä`
twice. `fonaw` is 0 for all six here, so the port draws a two-column gap where the reference
draws a glyph. Measured on frame 14000's own lines, where the port's rendered extent meets
the reference's band: `Daryl / Sun Projects` **15** columns wider, `Tran & White Shadow`
**14**, `Dr.Weird / DanTe` **15**, `Jussi Leppänen` **14**, `Emphii, Mace & Shaft` **14** —
and `Martti Palonen`, `Jussi Markula`, `Mikko Vierula` and `Wildfire:`, which name none of
the six, within **1** column, which is the raster below.
The appended run is **six** bytes — `Z \x84 \x94 Y / &`, the `xxd` dump above — so the
shipped order is 74 + 6 = **80**; a count of seven contradicted this note's own dump and its
own "`fonaw` is 0 for all six", and was corrected at the close-out.

**4. The raster is not `ENDSCRL/FONA.UH`'s either.** At the tick whose header sits on the
reference's own rows, the port's header band differs from the reference's in **53 pixels of
11,520** (18 rows × 640), of which 22 are ink and the rest are values. The port's `FONA.UH`
is the tree's — and the tree holds the same art twice: `ENDSCRL/FONA.LBM` decodes to that
raster value for value (`the_strip_is_the_one_the_part_scans`, and `assets/PROVENANCE.md`
rows 164-165 read the two as source and converted form). The shapes, widths and columns
agree — the same 13 column runs in the header, to the pixel — so this is a re-drawn raster
and not a misread one, and it is the same class of difference A10 records for CRED.

**5. The text the renderer sees has no carriage returns.** `assets/MAIN/DATA/ENDSCROL.TXT`
holds `\r\n`; the centring arithmetic `tstart=(639-tstart)/2` (`:73`) with `fonaw[13] = 0`
and a two-column pen step makes a line with the CR exactly **one column** further left than
without it. The reference's header starts at column **219** and its second line at **41** —
the CR-less width in both cases; the port's are 218 and 40. What put the buffer in that state
is **not established**: the part reads the file through the demo's own service — the packed
`ENDSCROL.TXT` of `MAIN/PACKFINA.INC:51`, which is this same 6,141-byte file — and the tree
holds no source for that service, so a text-mode read that drops CRs and a pack that stored
the text without them are both open.

**Two of these drift, and that is why no alignment reaches zero.** The pitch makes the port
advance the text 5 rows further per line than the reference (60 frames against 50), and the
control lines make the reference skip whole blocks the port renders — so the content moves
apart as the part runs, at a rate a single frame shift cannot follow. `--align 82` shows it
from the other side: 9100 improves (2.8473 % → **2.1758 %**, the chain's offset being the
one thing an alignment *can* absorb), and 10000 worsens (11.0676 % → 12.5437 %).

`the_references_own_header_pins_the_chains_offset` asserts the tick, the start and the
53-pixel remainder in one place — the geometry, the metrics, the palette, the plane/bit
mapping and the display's own walking of the ring are all exercised by that same
comparison, which is what makes the remainder the font's.

## 7. Tests

`bins/secondreality/src/parts/endscrl.rs` (12 unit tests, reading the curated `assets/` and
the committed `reference/`):

| test | what it pins |
|---|---|
| `the_strip_is_the_one_the_part_scans` | 74 runs for the 74 characters of the order, widths 3..25, the blank's 16 columns and no cell, absent characters width 0, every value a 2-bit mask, ink rows 0..17 |
| `the_text_is_the_shipped_one_and_not_the_developers` | 6,141 bytes read of the shipped file, the 1,007-byte one not, the buffer's tail `read`'s, and what each file opens with |
| `the_shipped_text_fits_the_row_and_the_textline_buffer` | 542 lines, ≤ 100 characters each, `tstart ≥ 0` and the width inside the 640-column row — the two bounds the source never checks |
| `the_mode_set_is_the_four_hundred_line_screen_with_the_inherited_dac` | the clear, 640×400, stride 640, CRTC 9 = 0, the start address back at 0, entries 1-15 grey and 0 and 16 untouched |
| `a_line_is_written_once_and_the_beam_wraps_instead_of_a_second_copy` | the write lands in one ring line and nowhere else, `80*(yscrl+401)` is past the surface, and the line just written is the display's **bottom** row |
| `the_display_advances_one_line_every_two_ticks` | ticks 3, 5, 7 … move the origin one line; the ticks between them do not |
| `the_ring_wraps_at_four_hundred_lines_not_four_hundred_and_one` | 400 lines is a whole turn, the 400th origin is 256,000, and the 401st is the first state again |
| `the_four_hundred_line_ring_shows_what_the_four_hundred_and_one_line_ring_shows` | the modelling equivalence, 400 displayed rows compared as bytes at every iteration of a 500-line run against the original's own 401-line arithmetic |
| `the_references_text_runs_at_twenty_five_rows_a_line_where_the_source_says_thirty` | §6.2's pitch, on the reference's frame and on the port's |
| `the_references_own_header_pins_the_chains_offset` | §6.1's tick, start and residual, and §6.2's 53-pixel raster difference |
| `the_references_control_lines_advance_the_ring_and_draw_nothing` | §6.2's `[NN` rule: the 163 control lines and their four sizes, and every band of frames 10000 and 14000 predicted from the tree's own text and found in the reference |
| `the_shipped_order_names_six_characters_the_trees_does_not` | §6.2's six extra characters: `fonaw` 0 for each, the text's own counts, and the 14/15-column width of the lines that name them against ≤ 1 for the rest |

`bins/secondreality/tests/endscrl.rs` (2 tests through the real binary, and **two** runs of
it — a far one is 13,596 ticks, so the checks are grouped by the dump they need): the four
keyframes dump in the reference's own numbering (`index_origin: boot`, anchor 404, 640×400),
each under the part that owns it, and the run leaves `reference/` alone; and the chain hands
this part its tick 0 at **8835** with CRED's screen at 8834 and the geometry change at
8837.

Every number in this note is produced by a test in this commit or by a command named here.
