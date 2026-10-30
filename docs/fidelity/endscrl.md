# ENDSCRL — the greetings scroller, read from the release's own build

Sources: the shipped `MAIN/DATA/ENDSCRL.EXE`, unpacked by `sr_assets::exe::unpack` (PKLITE over
LZEXE 0.91, `release-images.md`), a Borland C++ large-model image whose DGROUP is `0x1c890`
(`mov dx,1c89h` at image 0). The tree's `ENDSCRL/MAIN.C` (125 lines over `ENDSCRL/ASMYT.ASM`) is a
development snapshot of it with other constants, font, text rules and end (§1). The port reads the
image's raster and `fonaorder` and translates its code (D49).
Port: `bins/secondreality/src/parts/endscrl.rs`, registered as `endscrl`. The module doc owns the
mechanism: `main`'s acts, the tick map, the 400-line ring and the line renderer.
Run: `--block 5`, chained after CRED; the port's tick 0 is index **8835**, the release's **8917**
(§6.1). Music: none read.
Reference: `reference/parts/endscrl/`, keyframes 9100, 10000, 12000, 14000 (`reference-indices.md`
§21). All four are 0 from the release's start; the block-5 counts are A11's chain offset.

Offsets are into the unpacked image: `cargo test -p sr-assets --test exe -- --ignored
dump_the_unpacked_images` writes `out/release-images/ENDSCRL.img`, read with
`objdump -D -b binary -mi8086 --start-address=…`.
`parts::endscrl::tests::the_image_is_the_build_these_offsets_name` reads back the cited
instructions.

## 1. What the shipped build does

| image | what it is | the tree's `MAIN.C` |
|---|---|---|
| `0x1220-0x1225` | `dis_partstart()`, one `dis_waitb()` | `:19-21`, the same |
| `0x122a-0x1235` | mode 0eh, then CRTC 9 = 0 (`mov ax,9; mov dx,3d4h; out dx,ax`): the 400-line display | `:22-26`, the same |
| `0x1236-0x1323` | fifteen `setrgbpalette` calls: entry 1 = 20, 2 = 40, 3-15 = 60 | `:27-41`, the same |
| `0x1326` | `init()` | `:43` |
| `0x132b-0x1348` | `while(!dis_exit()) { if(dis_waitb()==1) dis_waitb(); do_scroll(); }` | `:45-49`, the same |
| `0x1351-0x13c0` | the line read: copy to `'\n'`, `tstart += fonaw[c]+2`, `tstart = (639-tstart)/2` | `:66-74`, the same |
| `0x13c3-0x13ca` | **`if (textline[0] == '[') chars = 0;`** | none |
| `0x13e1-0x152a` | the drawing loop, `font[line*1550 + fonap[c] + b]` (`mov dx,060eh; imul dx` at `0x140a`; the far array at segment `0x112e`, `0x1423`) | `:77-88`, stride **1500** |
| `0x152d-0x164b` | the four plane `memcpy`s at `80*yscrl` and `80*(yscrl+401)`, `yscrl = (yscrl+1) % 401` | `:89-93`, the same |
| `0x164f-0x1683` | **`line = (line+1) % 25`**, or for a control line `% ((textline[1]-'0')*10 + textline[2]-'0')` | `:94`, `% FONAY` = **30** |
| `0x1687-0x1693` | `setstart(yscrl*80 + 80)` | `:95`, the same |
| `0x169a-0x16a3` | **`if (textline[0] == '%') exit(0);`** (`0x272` is the runtime's `exit`: the `atexit` loop over DGROUP `0x110`, then `_exit`) | none: the part never ends |
| `0x16b2-0x16da` | `open("endscrol.txt", 1)`, `read(fd, text, 60000)` into `018e:0000`, `close` | `:101`, the same |
| `0x16e1-0x17a3` | the scan, over **25** rows (`cmp word [bp-4],19h`, `0x170d`) and **1,550** columns | `:103-122`, 30 rows and 1,500 columns |
| `0x17a8`, `0x17ae` | `fonap[' '] = 0x5fa` (**1530**), `fonaw[' '] = 16` | `:123-124`, 1480 and 16 |

The data, read from the image (`the_strip_and_its_order_are_read_from_the_image`):

- **`fonaorder`** is the far pointer at DGROUP `0x92`, `1c89:00b2`: 80 bytes,
  `ABCDEFGHIJKLMNOPQRSTUVWXabcdefghijklmnopqrstuvwxyz0123456789!?,.:\x8f\x8f()+-*='Z\x84\x94Y/&`,
  the tree's 74 (`MAIN.C:13`) and `Z ä \x94 Y / &`. `endscrol.txt` follows it at DGROUP `0x103`.
- **The raster** is `char far font[30][1550]` at image `0x112e0`, values 0..3, ink in rows 0..20;
  rows 25..29 hold none. 'A' is at column 1, width 20; `Z`/`ä`/`\x94`/`Y`/`/`/`&` are at
  1012/1029/1043/1055/1073/1088, widths 16/13/11/17/14/13; the blank's 16 columns from 1530 hold no
  ink. One 56-column run at 1391 is reached by no character of the order. The text uses the six added
  characters `/` 83, `&` 26, `Z` 12, `Y` 1, `ä` 2 and `\x94` 0 times, and each has a run
  (`the_six_characters_the_trees_order_lacks_are_drawn`).
- **The text array**, `018e:0000`, 64,000 bytes up to the raster's segment, starts with the tree's
  initialiser (`"Testing Testing\0PIXEL SUCKS\0…"`, `MAIN.C:52`); `read` overwrites its front.

The chars are read unsigned (`mov ah,0` before each table index, `0x1380`, `0x176c`), so `ä` (0x84)
indexes entry 132; the tree's signed `char` would index `fonaw[-124]`.

## 2. The carriage returns

`open("endscrol.txt", 1)` passes neither `O_TEXT` nor `O_BINARY`, so `open` (`0xab0`) takes `_fmode`'s
mode (`mov ax,[02d8h]; and ax,0c000h` at `0xac1`; DGROUP `0x2d8` holds `0x4000`, `O_TEXT`), and
Borland's `read` (`0xc62`) compacts the bytes in place: `lodsb`, `cmp al,1ah` (end of file),
`cmp al,0dh` (skip), `stosb` (`0xcbf-0xcc9`). The tree's `MAIN.C:101`
makes the same call. `read_text_mode` is that loop: the shipped 6,141-byte file has 542 CRs and no
`^Z`, so 5,599 bytes are kept, the next 542 are the raw read's tail, and the rest is the initialiser
(`the_text_is_read_in_text_mode_without_its_carriage_returns`). The case it does not model, a CR as
the read's last byte (the runtime then reads one more, `0xcd0`), cannot arise: the file ends in
`\r\n`, and the test pins it.

The text is `MAIN/DATA/ENDSCROL.TXT` (packed by `MAIN/PACKFINA.INC:51`), not the tree's 1,007-byte
`ENDSCRL/ENDSCROL.TXT` (`asset-inventory.md` §E.9). It has 542 lines, the longest 40 characters, the
widest 592 columns (from 23 to 612) (`the_shipped_text_fits_the_row_and_the_textline_buffer`).

## 3. The control lines

A line whose first byte is `[` draws nothing and lasts its own count of rows; every other line lasts
25. On the CR-less text the count is `(t[1]-'0')*10 + t[2]-'0'` over the line's own bytes and its
newline, so the four forms are (`a_control_line_draws_nothing_and_lasts_its_own_count`):

| form | lines | rows |
|---|---|---|
| `[12` | 135 | 12 |
| `[10` | 14 | 10 |
| `[15` | 8 | 15 |
| `[8` | 6 | **42**: the third byte is the newline, `8*10 + 10 - 48` |

`[8`'s 42 is the original's arithmetic, kept (D7). No keyframe shows a `[8` (the first is text line
430), so the 42 is read from the code. The same test renders the `[15` under "Canada" (text lines 17
and 18): fifteen blank rows, "Epic MegaGames" read on the 41st, and the tree's rule would ink the
digits.

## 4. The ring

The port's ring is 400 lines where the original's is 401, and the beam's wrap stands in for the
second copy: the module doc gives the equivalence, and
`the_four_hundred_line_ring_shows_what_the_four_hundred_and_one_line_ring_shows` checks it byte for
byte, all 400 rows at every iteration of a 500-line run.

## 5. Timeline and the end

The tick map is the module doc's: tick 0 shows CRED's screen, tick 1 sets the mode (which clears
video memory, `int10_modes.cpp`'s `finish_set_mode`, so unwritten ring lines are black; DAC entries
1-15 become grey and entry 0 and 16 up keep what CRED left,
`the_mode_set_is_the_four_hundred_line_screen_with_the_inherited_dac`), tick 2 is the
loop's first wait, and `do_scroll` runs on ticks 3, 5, 7, … (`reference-indices.md` §21.1 measures the
two-tick picture on the reference's triples).

**The `%` line** is text line 537. The rows before it sum to **11,482**, so the `do_scroll` that reads
it is the 11,483rd, on part tick 3 + 2 × 11,482 = **22,967**; it draws its line, sets the start and
calls `exit(0)`, and the part reports `Finished` on that tick. Tick 22,350 is the last with ink in the
ring; it is blank from 22,351 (`the_part_ends_by_itself_on_the_percent_line`). Reading past
`text[64000]` when the text has no `%` line (`MAIN.C:66-74`; image `0x1351-0x13a3`) is unreachable
with the shipped text (`bugs.md` index).

ESC ends the part early through the shell. Either way the shell retires the part and a demo run goes
to `@@partsdone`'s 64-tick fade, 20 ticks of blank and the end (D41);
`a_demo_run_ends_after_the_scroller_ends_itself` (release-only) finds the hold on the tick after
22,967 and the end 84 ticks later (`part-sequence.md` §9).

**Where that lands.** In `--block 5` the port's ENDSCRL begins at 8835, so it ends at **31,802**, the
fade runs to 31,866 and the blank to 31,886. The release's begins at 8917, so its `%` is at
**31,884** and its text mode, after the fade, near 31,948. A scratch capture of the original
(`SECOND 5`, the standard recipe, 30,000 cycles, every 250th frame from 30,500; not filed) has
640×400 frames up to **31,750**, black at 31,500 and 31,750 (the ring is blank from 8917 + 22,351 =
31,268), and text mode with the end box from **32,000**. 31,948 lies in (31,750, 32,000].

## 6. Comparison

| keyframe | rendered | differing px (port's start, 8835) | from the release's start, 8917 | register |
|---|---|---|---|---|
| 9100 | 9100 | 7,217 | 0 | A11 |
| 10000 | 10000 | 32,508 | 0 | A11 |
| 12000 | 12000 | 25,118 | 0 | A11 |
| 14000 | 14000 | 17,613 | 0 | A11 |

The first count column is `cargo xtask rediff --only endscrl`'s; both columns are asserted by
`the_port_draws_the_references_frames_at_the_references_own_ticks`. Planes are RGB of 256,000 pixels.
The full run's filed opening frames (`fullrun/13-endscrl`: 46280, 46283, 46286, rendered 88 frames
earlier at the A53 seam) are 0.

### 6.1 The chain offset

The tick whose display puts the text's first line on frame 9100's own rows is the port's **183**
(and 184, its pair), and there the whole frame is 0
(`the_references_own_header_pins_the_chains_offset`): the release's scroller begins at **8917**.
The port's chain hands the part its tick 0 at **8835**, with CRED's screen at 8834 and the geometry
change at 8837 (`tests/endscrl.rs`,
`the_chain_hands_the_scroller_its_tick_zero_at_the_index_the_note_records`). The 82 frames (41 ring
lines) are CRED's span in block 5, inside CRED's measured bracket of 6 to 105 frames early
(`cred.md` §7). The reference's start lies in **[8841, 8938]** (the CRED sweep shows 8840 still CRED's
and refuses 8940 as 640×400), and 8917 is inside it. The full run's offset is 88 (A53's seam table).

## 7. Decisions

- **The shipped build is the part** (D49); the tree's constants are not used.
- **`[8`'s 42 rows** are the original's arithmetic, kept (D7).
- **The 82-frame block-5 offset** is CRED's drawing time, not this part's, and stays registered (A11).

## 8. Reproduce

```bash
cargo xtask rediff --only endscrl --only fullrun/13-endscrl
cargo test --release -p secondreality --test endscrl
cargo test --release -p secondreality --bin secondreality parts::endscrl   # includes the release-only run
```

**Register.** A11 (the block-5 chain offset); `bugs.md` index (the read past the text array).
