# ENDSCRL — the greetings scroller, read from the release's own build

The demo's last part: a ring of text, one line written per two vertical blanks, the display
start walking the ring a line at a time, until the text's `%` line ends the part.

**The port is the shipped build.** The tree's `ENDSCRL/MAIN.C` (125 lines over
`ENDSCRL/ASMYT.ASM`) is a development snapshot of this part, and the release's build differs from
it in its constants, its font, its text rules and its end (§1). Since D49 the release's packed
`MAIN/DATA/ENDSCRL.EXE` is unpacked (`sr_assets::exe::unpack`, PKLITE over LZEXE 0.91,
`docs/fidelity/release-images.md`), and the part reads its font raster and `fonaorder` out of that
image and translates that image's code. The four reference keyframes are drawn **to the pixel**
from the release's own start (§6), and the part ends by itself where the release's does (§7).

Everything below cites **image offsets**: offsets into the unpacked load image, which
`cargo test -p sr-assets --test exe -- --ignored dump_the_unpacked_images` writes to
`out/release-images/ENDSCRL.img`, read with
`objdump -D -b binary -mi8086 --start-address=… out/release-images/ENDSCRL.img`. The image is
Borland C++ large model; DGROUP is image `0x1c890` (`mov dx,1c89h` at image 0).
`parts::endscrl::tests::the_image_is_the_build_these_offsets_name` reads back every instruction
cited here.

## 1. What the shipped build does

| image | what it is | the tree's `MAIN.C` |
|---|---|---|
| `0x1220-0x1225` | `dis_partstart()`, one `dis_waitb()` | `:19-21`, the same |
| `0x122a-0x1235` | mode 0eh, then CRTC 9 = 0 (`mov ax,9; mov dx,3d4h; out dx,ax`) — the 400-line display | `:22-26`, the same |
| `0x1236-0x1323` | fifteen `setrgbpalette` calls: entry 1 = 20, 2 = 40, 3-15 = 60 | `:27-41`, the same |
| `0x1326` | `init()` | `:43` |
| `0x132b-0x1348` | `while(!dis_exit()) { if(dis_waitb()==1) dis_waitb(); do_scroll(); }` | `:45-49`, the same |
| `0x1351-0x13c0` | `do_scroll`'s line read: copy to `'\n'`, `tstart += fonaw[c]+2`, `tstart = (639-tstart)/2` | `:66-74`, the same |
| `0x13c3-0x13ca` | **`if (textline[0] == '[') chars = 0;`** — a control line draws nothing | none |
| `0x13e1-0x152a` | the drawing loop, `font[line*1550 + fonap[c] + b]` (`mov dx,060eh; imul dx` at `0x140a`; the far array at segment `0x112e`, `0x1423`) | `:77-88`, stride **1500** |
| `0x152d-0x164b` | the four plane `memcpy`s at `80*yscrl` and `80*(yscrl+401)`, `yscrl = (yscrl+1) % 401` | `:89-93`, the same |
| `0x164f-0x1683` | **`line = (line+1) % 25`**, or, for a control line, `% ((textline[1]-'0')*10 + textline[2]-'0')` | `:94`, `% FONAY` = **30** |
| `0x1687-0x1693` | `setstart(yscrl*80 + 80)` | `:95`, the same |
| `0x169a-0x16a3` | **`if (textline[0] == '%') exit(0);`** — `0x272` is the runtime's `exit` (the `atexit` loop over DGROUP `0x110`, then `_exit`) | none: the part never ends |
| `0x16b2-0x16da` | `open("endscrol.txt", 1)`, `read(fd, text, 60000)` into `018e:0000`, `close` | `:101`, the same |
| `0x16e1-0x17a3` | the scan, over **25** rows (`cmp word [bp-4],19h`, `0x170d`) and **1,550** columns | `:103-122`, `FONAY` 30 rows and 1,500 columns |
| `0x17a8`, `0x17ae` | `fonap[' '] = 0x5fa` (**1530**), `fonaw[' '] = 16` | `:123-124`, 1480 and 16 |

The data, read from the image:

- **`fonaorder`** is the far pointer at DGROUP `0x92`, `1c89:00b2`, and the 80 bytes there are
  `ABCDEFGHIJKLMNOPQRSTUVWXabcdefghijklmnopqrstuvwxyz0123456789!?,.:\x8f\x8f()+-*='Z\x84\x94Y/&` —
  the tree's 74 (`MAIN.C:13`) and `Z ä \x94 Y / &`. `endscrol.txt` follows it at DGROUP `0x103`.
- **The raster** is `char far font[30][1550]` at image `0x112e0`, values 0..3, ink in rows
  0..20; rows 25..29 hold none and nothing reads them. Its runs, as the scan finds them, give
  'A' at column 1 and width 20, `Z`/`ä`/`\x94`/`Y`/`/`/`&` at 1012/1029/1043/1055/1073/1088 with
  widths 16/13/11/17/14/13, and the blank's 16 columns from 1530 hold no ink
  (`the_strip_and_its_order_are_read_from_the_image`). The raster also holds one 56-column run at
  1391 that no character of the order reaches.
- **The text array**, `018e:0000`, 64,000 bytes up to the raster's segment, starts with the
  tree's own initialiser (`"Testing Testing\0PIXEL SUCKS\0…"`, `MAIN.C:52`); `read` overwrites its
  front.

The chars are read unsigned throughout (`mov ah,0` before each table index, `0x1380`, `0x176c`), so
`ä` (0x84) indexes entry 132; the tree's signed `char` would have indexed `fonaw[-124]`.

## 2. The carriage returns

`open("endscrol.txt", 1)` passes `O_RDONLY` and neither `O_TEXT` nor `O_BINARY`, so `open` (image
`0xab0`) takes `_fmode`'s mode (`mov ax,[02d8h]; and ax,0c000h` at `0xac1`), and DGROUP `0x2d8`
holds `0x4000`, `O_TEXT`. Borland's `read` (`0xc62`) on a text-mode handle compacts the bytes DOS
returned **in place** — `lodsb`, `cmp al,1ah` (end of file), `cmp al,0dh` (skip), `stosb`, at
`0xcbf-0xcc9`. The tree's `MAIN.C:101` makes the same call, so both builds read the text without
its CRs, and the port's old buffer with them was the port's own deviation.

The port's `read_text_mode` is that loop: the shipped 6,141-byte file has 542 CRs and no `^Z`, so
5,599 bytes are kept, the next 542 are the raw read's own tail, and the rest of the array is its
initialiser (`the_text_is_read_in_text_mode_without_its_carriage_returns`). The one case it does
not model, a CR as the read's last byte (which makes the runtime read one byte more, `0xcd0`),
cannot arise: the file ends in `\r\n`, and that test pins it.

Which text: `MAIN/PACKFINA.INC:51` packs the 6,141-byte `MAIN/DATA/ENDSCROL.TXT`, the file the
part reads, not the tree's 1,007-byte `ENDSCRL/ENDSCROL.TXT` ("Lerto has coded this…");
`docs/fidelity/asset-inventory.md` §E.9 measures the pair.

## 3. The control lines

A line whose first byte is `[` draws nothing (`chars = 0`) and lasts its own count of rows; every
other line lasts 25. On the CR-less text the count is `(t[1]-'0')*10 + t[2]-'0'` on the line's
own bytes and its newline, so the four forms in the text are (`a_control_line_draws_nothing_and_lasts_its_own_count`):

| form | lines | rows |
|---|---|---|
| `[12` | 135 | 12 |
| `[10` | 14 | 10 |
| `[15` | 8 | 15 |
| `[8` | 6 | **42** — the third byte is the newline: `8*10 + 10 - 48` |

`[8`'s 42 is the original's own arithmetic, kept (D7). None of the four reference keyframes shows
a `[8` (the first is text line 430), so the 42 is read from the code, not from a frame.

The same test renders the `[15` under "Canada" (text lines 17 and 18): its fifteen rows are
blank, the next line ("Epic MegaGames") is read on the 41st row, and the same line drawn under the
tree's rule inks — the digits are in the order — so a regression to printing `[15` fails it.

## 4. The ring: 400 lines, and the wrap that stands in for the second copy

The port's surface is 640×400 = **256,000** pixels = exactly 400 lines of 640. The original's
401-line ring needs the second copy at `80*(yscrl+401)` because the display's address counter does
not wrap at the ring's end; the port's beam does (`View::source_index` is modulo the surface's
length). So each line is written once, `yscrl` runs `0..400` where the original's runs `0..401`,
and the display origin is `(yscrl + 1) * 640` — 640 on the first line and **256,000** on the last,
which is why `start_addr` is a `u32` (D16). The two pictures are the same lines in the same rows:

```text
original, iteration t:  writes line  t mod 401, twice (the copy at +401)
                        display start  ((t+1) mod 401) + 1  (post-increment)
                        displayed row j shows the line written at  t - 399 + j
port,     iteration t:  writes line  t mod 400, once
                        display origin (yscrl + 1) * 640,  yscrl = t mod 400
                        displayed row j shows the line written at  t - 399 + j
```

`the_four_hundred_line_ring_shows_what_the_four_hundred_and_one_line_ring_shows` checks that
equality as bytes: a 401-line ring written twice with the display start at `yscrl*80 + 80`,
against the port's ring, all 400 displayed rows at every iteration of a 500-line run.

## 5. The tick map, and the end

A tick is one `dis_waitb`, and tick 0 is the code before the first one:

- **tick 0**: nothing drawn; the screen is still CRED's.
- **tick 1**: the mode set (which clears video memory — `int10_modes.cpp`'s `finish_set_mode` —
  so the ring's unwritten lines are black), the palette, `init`.
- **tick 2**: the loop's first wait returning 1, with the second still ahead of it.
- **tick 3**: the first `do_scroll`, and one every second tick after it — a picture is two ticks
  wide, as `reference-indices.md` §21.1 measures on the reference's own triples.
- **the `%` line**: text line 537. The rows before it sum to **11,482**, so the `do_scroll` that
  reads it is the 11,483rd, on part tick 3 + 2 × 11,482 = **22,967**; that call draws its line,
  sets the start and calls `exit(0)`, and the part reports `Finished` on that tick
  (`the_part_ends_by_itself_on_the_percent_line`). The screen is black by then: tick 22,350 is
  the last with ink in the ring, and it is blank from 22,351.

ESC still ends the part early, as before: the loop is `while(!dis_exit())`, and the shell retires
a part on the tick the flag is set in.

**What the shell does next is the same either way.** A part's own `Finished` and an ESC both
retire it (`Sequence::tick`), and a demo run whose plan has run out goes to the hold, which is
`@@partsdone`: 64 ticks of fade, 20 of text mode's blank, the end (D41). Nothing in the shell
assumed ENDSCRL ends only on ESC, so the shell is unchanged;
`a_demo_run_ends_after_the_scroller_ends_itself` runs the real part under `Sequence` with
`Ending::Exit` and finds the hold on the tick after 22,967 and the end 84 ticks after that. (That
test composes 23,000 640×400 frames, about two minutes in a debug build, so it is
`#[cfg_attr(debug_assertions, ignore)]`; run it with
`cargo test --release -p secondreality --bin secondreality parts::endscrl`.)

**Where that lands.** In a `--block 5` run the port's ENDSCRL begins at 8835 (§6), so it ends at
index **31,802**, the fade runs to 31,866 and the blank to 31,886. The release's ENDSCRL begins at
8917, so its `%` is at **31,884** and its text mode, after the 64-frame fade, near 31,948. The
controller's capture of the original agrees: `SECOND 5`, the standard recipe, 30,000 cycles, a
frame grabbed every 250 frames from 30,500 (scratch, `out/endscrl-end2/`, not filed) has
640×400 frames up to and including **31,750**, black at 31,500 and 31,750 (the text has scrolled
off: the ring is blank from the release's 8917 + 22,351 = 31,268), and 720×400 text mode with
the end box and `C:\>` from **32,000**. 31,948 lies in (31,750, 32,000].

## 6. The comparison

```
$ cargo xtask rediff --only endscrl --only fullrun/13-endscrl
capture             reference  rendered  plane    differing  registered  register  verdict
endscrl                  9100      9100  rgb           7217        7217  A11       ok
endscrl                 10000     10000  rgb          32508       32508  A11       ok
endscrl                 12000     12000  rgb          25118       25118  A11       ok
endscrl                 14000     14000  rgb          17613       17613  A11       ok
fullrun/13-endscrl      46280     46192  rgb              0           0  A53       ok
fullrun/13-endscrl      46283     46195  rgb              0           0  A53       ok
fullrun/13-endscrl      46286     46198  rgb              0           0  A53       ok
```

Before this change the same command gave 7,289 / 28,333 / 25,502 / 17,108 and 42 / 67 / 149.

**What is left is the chain's offset and nothing else.** The reference's scroller begins at
**8917**: the tick whose display puts the text's first line on the reference's own rows of frame
9100 is the port's **183** (and 184, its pair), and at those ticks the whole frame differs in **0**
pixels (`the_references_own_header_pins_the_chains_offset`). Rendered from that start, all four
keyframes differ in **0** pixels of 256,000; rendered from the port's own start, 8835, 82 frames
(41 ring lines) earlier, they differ in 7,217 / 32,508 / 25,118 / 17,613, the `rediff` counts
above (`the_port_draws_the_references_frames_at_the_references_own_ticks` asserts both sets). The
82 frames are CRED's span in block 5, inside CRED's own measured bracket of 6 to 105 frames early
(`cred.md` §7); the full run's is 88 (A53's seam table), and at that pairing the three filed opening
frames are 0. `APPROXIMATIONS.md` A11 registers the block-5 counts.

`tests/endscrl.rs` measures the port's start: the chain hands this part its tick 0 at **8835**, with
CRED's screen at 8834 and the geometry change at 8837. The reference's start lies in
**[8841, 8938]** (the sweep's log shows 8840 still CRED's and refuses 8940 as 640×400, `cred.md`
§7), and 8917 is inside it.

## 7. Tests

`bins/secondreality/src/parts/endscrl.rs` (15 unit tests, reading `assets/MAIN/DATA/ENDSCRL.EXE`,
`assets/MAIN/DATA/ENDSCROL.TXT` and the committed `reference/`):

| test | what it pins |
|---|---|
| `the_image_is_the_build_these_offsets_name` | every instruction this note and the part cite, read back from the image: DGROUP, `main`'s mode set, palette and loop, the control and `%` tests, the stride, segment and 25-row bounds, the blank, `open`'s mode, `read`'s CR and `^Z` tests, `_fmode`, the file name, `fonaorder`'s pointer |
| `the_strip_and_its_order_are_read_from_the_image` | the 80-character order, a run for every character, the six added characters' columns and widths, widths 3..25, the blank at 1530/16 with no ink, ink rows 0..20, rows 25..29 empty |
| `the_text_is_read_in_text_mode_without_its_carriage_returns` | 6,141 bytes, 542 CRs, 5,599 kept, the raw tail and the initialiser after them, and `read_text_mode` on a CR and a `^Z` |
| `the_shipped_text_fits_the_row_and_the_textline_buffer` | 542 lines, the longest 40 characters, the widest 592 columns starting at 23 and ending at 612 |
| `the_mode_set_is_the_four_hundred_line_screen_with_the_inherited_dac` | the clear, 640×400, CRTC 9 = 0, entries 1-15 grey and 0 and 16 untouched |
| `a_line_is_written_once_and_the_beam_wraps_instead_of_a_second_copy` | one ring line written, the bottom row is the line just written |
| `the_display_advances_one_line_every_two_ticks` | ticks 3, 5, 7 … move the origin |
| `the_ring_wraps_at_four_hundred_lines_not_four_hundred_and_one` | the turn, and the 256,000 origin |
| `the_four_hundred_line_ring_shows_what_the_four_hundred_and_one_line_ring_shows` | §4's equivalence, as bytes |
| `the_references_own_header_pins_the_chains_offset` | ticks 183/184 at 0 pixels, the start 8917, the 82 |
| `the_port_draws_the_references_frames_at_the_references_own_ticks` | 0 at all four keyframes from 8917; the `rediff` counts from 8835 |
| `a_control_line_draws_nothing_and_lasts_its_own_count` | §3's four forms, 42 for `[8`, the `[15` under "Canada" blank and lasting 15 rows |
| `the_six_characters_the_trees_order_lacks_are_drawn` | the text's counts (`/` 83, `&` 26, `Z` 12, `Y` 1, `ä` 2, `\x94` 0) and a run for each |
| `the_part_ends_by_itself_on_the_percent_line` | line 537, 11,482 rows, `Finished` on tick 22,967 and not before, the black screen, 31,884 and 31,802 |
| `a_demo_run_ends_after_the_scroller_ends_itself` | release-only: the hold after the part's own end, and the end 84 ticks later |

`bins/secondreality/tests/endscrl.rs` (2 tests through the real binary): the four keyframes dump in
the reference's own numbering, each under the part that owns it; and the chain's boundary at
8834-8838.

Every number in this note is produced by a test in this commit or by a command named here, except
the controller's capture in §5, whose command is named and whose frames are scratch.

**Register.** A11 (the block-5 chain offset). The part's one unreachable behaviour, reading past the
text array if the text had no `%`, is a row of `bugs.md`'s index.
