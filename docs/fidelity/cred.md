# CRED — the 21 credit screens

Sources: `CREDITS/MAIN.C` (318 lines) over `CREDITS/TWEAK.ASM` (357), part 19 of `part-sequence.md`
§1, `SECOND 5`'s second part (`exe22`, `MAIN/U2.ASM:912-913`, `partsmask[5]` = `0x10`). **The data
is the shipped build's** (D49): pictures, captions, font raster and `fonaorder` are read out of the
unpacked `assets/MAIN/DATA/CRED.EXE` (§9).
Port: `bins/secondreality/src/parts/cred.rs`, registered as `cred`. The module doc owns the
mechanism: the tweak-mode geometry, where the picture and caption land, the iris's three
registers (`Display`, `open_ramp`, `close_ramp`), the palette repair (`repair_palette`) and the
font scan.
Run: `--block 5`, chained after ENDLOGO; CRED's tick 0 is reference index **1212** (§2).
Music: none read. MUSIC0 plays on from ENDLOGO.
Reference: `reference/parts/cred/`, keyframes 1440, 3240, 5440, 7640, 8740
(`reference-indices.md` §20.1). **All five are byte-identical.**

## 1. Timeline

`main()` (`MAIN.C:36-159`): `dis_partstart()` (the version check, `DIS/DISC.ASM:51-64`, not a
wait), `init()` (`:246-318`: the metrics scan and the palette repair), `tw_opengraph()`,
`tw_setstart(160*200)`, then `screenin(pic, text)` 21 times (`:41-156`). `tw_closegraph()` is
commented out (`:158-159`); `main()` returns.

One tick is one `dis_waitb`. A screen (`screenin`, `:161-213`) is **363 ticks**:

| tick | source | on screen |
|---|---|---|
| 0 | `:165-167` `tw_setsplit(400)`, `tw_clrscr`, `tw_setstart(160*200)` | parked: the previous screen's band at the parked origin |
| 1 | `:168-174` `dis_waitb`, `tw_setpalette`, caption, picture | the palette, caption and picture land |
| 2-103 | `:176-192` open loop, `y = y*12L/13` from 25,600, 102 steps | the iris opens |
| 104-303 | `:194` `for(a=0;a<200 && !dis_exit();a++) dis_waitb()` | the hold; nothing is written |
| 304-362 | `:196-212` close loop, `y += v, v += 15`, 59 steps | the iris closes |

21 screens are 7,623 ticks (`the_part_runs_twenty_one_screens_of_the_sources_own_length`).

`tw_setsplit(400)` at `:165` is inert: a line compare of 400 sits below a 200-row display (`bugs.md`
index). `tw_opengraph` leaves the line compare at 100 (`TWEAK.ASM:49-52`: CRTC 0x18 = `0x64`, CRTC 7
= `0x0f`, bit 4 clear), and the first `screenin` overwrites it before any wait, so no displayed
frame shows the 100. `tw_opengraph` writes the AC's mode control (`0x30`), never AC 0x11, so the
border is the one CRED inherits.

## 2. Mapping and music

The port's CRED tick 0 is reference index **1212** in the block-5 chain: `--dump-frames 1211,1212`
writes `endlogo_001211.rgb` (384,000 bytes) and `cred_001212.rgb` (192,000), because the dump names
the part that drew each frame. So screen `k`'s hold is `1212 + (k-1)*363 + 103` to `+302` (tick 103,
the open loop's last step, writes the hold's state and shows it too). The run is `--block 5` because
1440 can only be reached if ENDLOGO's duration is right (`part-sequence.md` §10.1, §10.3).

The source tests `dis_exit()` before each `screenin` (`:41-156`) and in the hold (`:194`); the port
does not read the exit flag, so ESC is the shell's.

## 3. The iris, measured

**The split.** `split_scanline` is `(line_compare + 1) / 2`, the emulator's own arithmetic
(`vga_draw.cpp:1586-1593`: `(line_compare + 1) / lines_scaled`, `lines_scaled` 2 because a
double-scanned mode is rendered single-scanned, and the register names the last line *above* the
split). `tw_setsplit(400)` gives row 200, below the display; the hold's `tw_setsplit(200)` gives
row 100.

**The pan is the register value halved.** The source writes `(yy & 3) * 2` to AC 0x13. The
emulator's 256-colour arm is `pel_panning = (val & 0x7) / 2` (`vga_attr.cpp:225-227`), so one
register unit is **half a pixel**, in the same unit the display start is counted in
(`vga_draw.cpp:1648-1651` builds a line's address as the CRTC start times four *plus* the pan). A
step's pan is `yy & 3` pixels and its effective start is **128,000 + `yy`** pixels exactly, which is
what makes the two loops meet at the hold's 128,320. `View::pan` and ALKU (`alku.md`) use the same
unit.

**The hold state is checked, not fitted.** The open loop's last step (`y = 1`) writes line compare
200, display start 32,080 bytes (128,320 pixels: logical line 401, one `LINE_LEN` past the parked
128,000) and pan 0. All five keyframes are that state at 0 of 192,000 bytes (rediff's `cred`); the
parked origin would read the even logical lines, which hold no picture, and a split one row out
moves the caption by one displayed row, 320 pixels.

The per-step values of both loops (open 1: `y` 23,630, `yy` 25, line compare 384, start 128,024,
AC 0x13 2; close 58: 24,795, 629, 393, 128,628, 2) are
asserted in `the_three_registers_move_once_a_tick_through_both_wipes` (the start in pixels, the
line compare, and the pan in pixels, half the AC 0x13 value).

**The record's wipe frames.** The five keyframes are all holds, so `reference/parts/cred` cannot see
the iris. A scratch sweep of every 100th frame from 1240 to 8840 (not filed under `reference/`)
can: of its 77 frames, 49 are `reference-indices.md` §20.1's hold samples, each the port's hold of its own screen at 0 of
192,000 bytes, and **28 are wipe states**, each the port's state byte for byte. The lag is the
record's index minus the index at which the port's chain shows that state (`1212 + (screen-1)*363 +
tick`, open step `k` on tick `2+k`, close step `k` on `304+k`):

| index | state | lag | index | state | lag |
|---|---|---|---|---|---|
| 1240 | screen 1, open 12 | 14 | 1540 | screen 1, close 10 | 14 |
| 1640 | screen 2, open 45 | 18 | 1940 | screen 2, close 43 | 18 |
| 2340 | screen 4, open 13 | 24 | 2640 | screen 4, close 11 | 24 |
| 2740 | screen 5, open 46 | 28 | 3040 | screen 5, close 45 | 27 |
| 3440 | screen 7, open 15 | 33 | 3740 | screen 7, close 13 | 33 |
| 3840 | screen 8, open 48 | 37 | 4140 | screen 8, close 47 | 36 |
| 4540 | screen 10, open 16 | 43 | 4840 | screen 10, close 14 | 43 |
| 4940 | screen 11, open 49 | 47 | 5240 | screen 11, close 47 | 47 |
| 5640 | screen 13, open 16 | 54 | 5940 | screen 13, close 14 | 54 |
| 6040 | screen 14, open 50 | 57 | 6340 | screen 14, close 48 | 57 |
| 6740 | screen 16, open 17 | 64 | 7040 | screen 16, close 15 | 64 |
| 7140 | screen 17, open 51 | 67 | 7440 | screen 17, close 49 | 67 |
| 7840 | screen 19, open 18 | 74 | 8140 | screen 19, close 16 | 74 |
| 8240 | screen 20, open 52 | 77 | 8540 | screen 20, close 50 | 77 |

4940 and 7140 are each two steps' frame (the same pixels); the table names the step 300 ticks before
its close partner. Nineteen of the frames **rule out** reading `(yy & 3) * 2` as whole pixels, by 167 to 46,722 bytes; 6340 is
invariant under the shift, and the other eight have a pan of 0. These figures are a one-time
measurement against that sweep (`reference-indices.md` §20 records it); a re-capture cannot land on
the same indices (D17), so no test in the tree repeats it.

**What the lag says.** Within a screen it does not grow: each open frame and its close partner are
300 frames apart in the record and 300 or 301 ticks apart in the port (screens 5 and 8 show the
one-frame phase). Between screens it grows by **3 to 4** frames, `(77 − 14) / 19` = 3.3, all of it
between one screen's last close step and the next one's first open step, where `screenin` parks the
display, uploads the palette and draws 16,000 `tw_putpixel` far calls for the picture plus 32 a
caption column. DIS's `waitb` waits for the *next* frame however many have passed
(`DIS/DISINT.ASM:161-181`), so on the emulated 30,000-cycle CPU the drawing costs whole frames. The
image's loops are the tree's (§9), so this is CPU time, not data; the port draws in zero time, and
the offset it leaves is §7's chain residual (A11, A53).

## 4. Decisions

- **The shipped image is the data** (D49, §9); A10 (the nine changed screens) is retired.
- **The font strip is read raw, 32 of its 40 rows** (A8): a `sr_assets::font::Font` glyph is every
  row of its strip.
- **Not modelled:** the `for(a=0;a<10000;a++)` delays (`MAIN.C:182`, `:202`), which keep the writes
  inside the retrace the port's tick already is, and the image's three differences (§9).

## 5. The font, measured

The image's raster is at `0x29f0`, 1,500×40, and its `fonaorder` is `MAIN.C:32`'s 47 characters
(`the_images_font_and_order_are_where_init_reads_them`). The scan finds
**47 runs**, 447 inked columns, the last ink at column 557; the widest run is 11 columns and the
narrowest 2 (`I`, `!`, `:`, `,`, `.`). The routine's blank, `fonap[32] = 1500-32`, `fonaw[32] = 8`
(`:270-271`), is a column and a width no run provides
(`the_metrics_are_the_runs_the_strip_itself_holds`). Over 32 rows the runs are the same as over 40
(`the_eight_rows_the_routine_never_reads_do_not_move_a_run`; the unread-ink figures are A8's).

## 6. The palette repair and the clears

Mechanism in the module doc and `repair_palette`. `tw_opengraph`'s clear (`TWEAK.ASM:29-34`) covers
every pixel the machine has, so no state ENDLOGO leaves can show through a credit screen.

## 7. Comparison

`cargo xtask rediff --only cred` renders the five keyframes from `--block 5` and checks them at 0
(`mode: rgb`: the oracle refused the `.idx` of all 82 of the reference's CRED frames,
`reference-indices.md` §20.2, so the reference has none).

| screen | keyframe | the port's hold | frames into it | differing px |
|---|---|---|---|---|
| 1 | 1440 | 1316-1515 | 124 | 0 |
| 6 | 3240 | 3131-3330 | 109 | 0 |
| 12 | 5440 | 5309-5508 | 131 | 0 |
| 18 | 7640 | 7487-7686 | 153 | 0 |
| 21 | 8740 | 8576-8775 | 164 | 0 |

Every keyframe is inside its hold, the tightest (screen 21) ending 35 frames after its keyframe, and
`reference-indices.md` §20.2 measured all five as held, so a frame or two of phase cannot move a pixel. `tests/blocks.rs` asserts
the frames around each are the same (`the_five_keyframes_are_inside_the_chains_own_holds`). The
full run's `fullrun/12-cred` frames are 0 too, and so were the sweep's 49 hold samples (§3).

**The chain residual.** The port's screens are 363 ticks; the release's are 3 to 4 frames longer
(§3), so by screen 21's hold the port is about 80 frames early, inside the 200-frame hold. The
sweep's log refuses its 78th index, 8940 (`frame 8940 is 640x400`: ENDSCRL's screen), so that
session's last CRED frame is in **[8840, 8939]**. The port's is **8834** (tick 0 at 1212, last tick
7622), 6 to 105 frames early. ENDSCRL's reading (A11) puts the release's next part at 8917, so the
release's CRED ends at 8916 and the port's is 82 frames early: the lags above (77 at screen 20, plus
screen 21's 3 to 4, plus ENDSCRL's start), inside the bracket.

## 8. Reproduce

```bash
cargo xtask rediff --only cred --only fullrun/12-cred
cargo test --release -p secondreality --test blocks
cargo test -p secondreality cred::                 # the unit tests
cargo run --release --bin secondreality -- --block 5 --headless \
    --dump-frames 1211,1212 --dump-dir out/cred    # the 1212 boundary (§2)
```

## 9. The shipped image

`sr_assets::exe::unpack(assets/MAIN/DATA/CRED.EXE)` (PKLITE 1.14 large, one layer) gives a
426,384-byte Borland C++ large-model image with 261 relocations (`release-images.md`).
`cargo test -p sr-assets --test exe -- --ignored dump_the_unpacked_images` writes it to
`out/release-images/CRED.img`; every offset here is into that image, disassembled with
`objdump -D -b binary -mi8086`.

| what | where | how the code names it |
|---|---|---|
| DGROUP | `0x67520` | `mov dx,6752h` at image 0 |
| `main` | `0x190c-0x1b68` | `lcall 0190:0269` (`screenin`) 21 times, one every 27 bytes from `0x193a` |
| a screen's picture | `SS SS × 16`, segments `0x1145` + `k`·`0x419` | the call's `push SS SS` (a relocated word) |
| a screen's caption | DGROUP + `TT TT` | the call's `push TT TT`, after `push ds` |
| `font[40][1500]` | `0x29f0-0x1144f` | `mov dx,029fh` (`0x1e83`, `0x1ec5`, `0x1db0`), `mov dx,05dch; imul dx` |
| `fonaorder` | far pointer at DGROUP `0x92` → DGROUP `0x96` | `les bx,[0092h]` at `0x1eed` |

Each picture is an `FC FC` blob in the tree's layout (16-byte header, palette at 16, pixels at 784),
decoded by `load(blob, Format::Origin)`; `the_screens_are_mains_twenty_one_calls_read_from_the_image`
pins each call's offsets and caption. A relocation inside any byte read is refused, and
so is a `main` with another number of calls (`an_image_without_this_builds_calls_is_refused`).

**The code is the tree's.** `init` (`0x1e63-0x271c`): the scan over 32 rows and 1,500 columns,
`fonap[32] = 0x5bc` and `fonaw[32] = 8` (`0x1f30`, `0x1f36`), 21 `memmove(pic+0x40, pic+0x10,
0x2d0)` (`0x1f3c-0x211c`) and the `7*a` ramp. `prt` (`0x1d57`) and `prtc` (`0x1e01`) are the tree's.
`screenin` (`0x1b69-0x1d56`) has the tree's order and counts: the open loop from `0x6400` by
`y*12L/13` (Borland's long multiply and divide, `0x1ca4`/`0x1cab`), the 200-wait exit-aware hold
(`0x1cbc-0x1cd9`), the close loop with `v += 15`. Three things differ, and none is modelled:

1. **`main` starts with a `printf`** (`0x190c-0x1915`, `printf` at `0x5fa`) of DGROUP `0xc6`,
   "TÄMÄ ASIA PITÄÄ SELVITTÄÄ: Trugin creditsin" ("this must be sorted out: Trug's credits"), with no
   newline. The runtime's `fputc` flushes a line-buffered stream on a newline (`0x661-0x675`), so
   whether it reaches the screen depends on how `stdout` is buffered under DIS; no capture shows it.
2. **Each wipe step waits for the display, not a count.** In place of `MAIN.C:182`/`:202`'s spin, the
   image computes `yy` before the step's `dis_waitb`, then spins `while(inp(0x3da) & 1);`
   (`0x1c4d-0x1c5a`, `0x1cfb-0x1d08`) and writes `tw_setstart` **before** `tw_setsplit` (`0x1c69`,
   `0x1c7d`; `0x1d17`, `0x1d2b`), then the pan. The wait is shorter than a frame and moves no tick;
   the record's 28 wipe frames match the port's writes (§3).
3. **`tw_opengraph` resets the attribute flip-flop** (`mov dx,3dah; in al,dx` at `0x275d`) before
   writing AC `0x30` = `0x61`; the tree's does not. The port does not model the AC's mode register.

## Open items

- §3's sweep is a scratch capture, not filed under `reference/`, so its figures are a one-time
  measurement. Filing it would let a test hold them.

**Register.** `bugs.md` (the inert `tw_setsplit(400)`); A8; A10 retired; A11 and A53 for the chain
residual.
