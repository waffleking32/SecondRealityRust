# PANICEND — the collapse, the line and the dot

Sources: `PANIC/SHUTDOWN.C` (158 lines) over `PANIC/ASMYT.ASM` (`copyline`) and
`PANIC/TWEAK.ASM` (`tw_putpixel`, `tw_getpixel`, `tw_setpalette`, `tw_setrgbpalette`,
`tw_setstart`); no assets. Port: `bins/secondreality/src/parts/panicend.rs` (`"panicend"`);
the module doc has the three acts, the evidence split and the preserved artefacts. Block 2's
fifth part, after TECHNO (`MAIN/U2.ASM:75`, `:826-827`). Run: mapping `boot = 8782 + tick`
(§7); music MUSIC1 order 37, which the part never reads; no pre-roll. Reference:
`reference/parts/panicend`, 8792, 8800 and 9000 (`reference-indices.md` §10).

## 1. The keyframes, and which kind of evidence each is

The part draws nothing of its own. It inherits TECHNO's troll screen (the interleaved
320×400 picture at plane bytes 80..159 of each 160-byte row, the view at `start_addr` 320 with
`y_line_offset` 640) and TECHNO's DAC (entries 0..169 whole and entry 170's red and green,
from the `2*count + start` `setpalarea` bug, `bugs.md` §4). That splits the keyframes:

| index | what it is | evidence |
|---|---|---|
| 8792 | the collapse's band, rows 97..103, each 234 pixels at x = 40..276 | handoff: needs TECHNO's picture and DAC |
| 8800 | the line, row 100, 234 pixels at x = 40..276 | handoff |
| 9000 | the dot: one index-1 pixel at (160, 100), index 0 elsewhere | self-contained |

Both handoff frames have x = 273..275 black. The shapes are read from the reference files
themselves (`the_reference_frames_are_the_collapse_and_the_line_they_describe`).

The handoff is compared by `the_handoff_matches_the_collapse_8792_and_the_line_8800`, which
drives TECHNO's port to `Done` with the real MUSIC1 player and then this part on the same
`Surface`, `View` and `Palette`. The rig starts TECHNO as an entry part. Its end state (the
troll in the surface, the DAC's used entries, the 320×400 view) is the state the chain leaves,
because those last acts do not depend on how the music reached the final wait.

A standalone `--part panicend` run starts on a blank surface and the default DAC, so it cannot
answer 8792 and 8800 (A63).

## 2. Timeline

`SHUTDOWN.C` executes **121 waits**: `:66`, `:83`, `:90`, the collapse's eleven at `:93`, the
line's 47 at `:118` and the pulse's 60 at `:132`. The port gives each segment between waits a
tick, including the empty one between `:90` and `:93`, so ticks 0..120 are the 121 drawn
frames and tick 121 is `sleep(1)`'s retirement:

| tick | source | acts |
|---|---|---|
| init | `:30-55` | `dis_partstart`; `getpal` (§3) |
| 0 | `:61-65` | black row 0; the 64 fade tables; the 4:1 copy; `tw_setstart(100*160)` |
| 1 | `:66-84` | CRTC `09h = 41h`, GC `05h = 41h`, map mask `0x0F`; `fadepals[3]`; entry 0 white; `tw_setstart(0)` |
| 2 | `:84-90` | `fadepals[20]`; CRTC `13h = 0xA0` |
| 3..12 | `:91-102` | the first ten collapse passes |
| 13 | `:91-105` | the last pass (`a = 3`) and the two closing copies of rows 202 and 198 |
| 14 | `:106-127` | GC `05h = 40h`; the line's first iteration (`x = 20`) |
| 15..60 | `:116-127` | the line's other 46 iterations |
| 61 | `:129-135` | `tw_putpixel(160,200,1)`; the pulse's first value |
| 62..120 | `:130-136` | the rest of the 60-frame pulse |
| 121 | `:136` | `sleep(1)`: `Finished` |

The collapse's `a` values are `32, 26, 21, 17, 14, 11, 9, 7, 5, 4, 3` (`a = a*5/6` in C
integers). `the_part_ignores_the_exit_flag_and_completes_its_finite_acts` asserts the last tick
is `FINISHED_STEP` (121).

## 3. `getpal`'s `0d` is decimal zero

`SHUTDOWN.C:148` is `mov al, 0d`: in the tree's assembler a trailing `d` is the decimal suffix
(the hex spelling is `TWEAK.ASM:200`'s `0dh`). `getpal` reads the inherited DAC from entry 0,
unrotated. This corrects the survey's "reads the DAC starting at index 13"; it is not a D24
divergence. The reference decides it: tables from the unrotated palette reproduce 8792 and
8800 byte for byte (`the_handoff_matches_the_collapse_8792_and_the_line_8800`), and a 13-entry
rotation does not. A rotation
would need `TROLL.UP`'s palette and pixels shifted by thirteen in opposite directions, and the
vendored `TROLL.UP` is the file the port reads.

## 4. `copyline`'s pair duplication

`copyline` (`ASMYT.ASM:9-24`) moves 80 words in GC write mode 1 (`SHUTDOWN.C:70-79`), and the
module doc has the latch mechanism. In surface terms each 8-pixel group becomes the source's
second 4-pixel group twice:

```text
dest[8k .. 8k+3]   = src[8k+4 .. 8k+7]
dest[8k+4 .. 8k+7] = src[8k+4 .. 8k+7]
```

That is why the line is 234 pixels at x = 40..276 rather than the troll's own 233 at 44..276:
the left edge moves four pixels outward, and the picture's last partial group (byte 149's one
lit pixel of four) is repeated into the hole at 273..275. A straight byte copy fails 8792 and
8800 (`copyline_moves_eighty_words_and_stores_the_second_byte_of_each_pair_twice`). Copies may
overlap their sources (the `a = 32` pass's `b = a/2` copy, plane byte 32,080 to 32,000); the
port runs the word moves in increasing order, as `rep movsw` with DF clear does.

## 5. The fade tables, the collapse and the two handoff frames

The fade tables start at `b = 3` (`SHUTDOWN.C:62`), so entry 0 is black in all 64; `:81`'s
white entry 0 lasts one frame (`the_fade_tables_leave_entry_zero_black_and_start_at_byte_three`).
The collapse's last upload is `fadepals[60]`, and the pulse writes entry 1 only. After the
pulse the DAC is entry 0 = (0,0,0), entry 1 = (1,1,1), which is what makes 9000
self-contained.

Each pass blacks rows `200 ± 2b` for `b = ⌊a/2⌋..a` with row 0, then copies picture row
`400b/a` (from its plane byte 80) to memory row `200 + 2(b − ⌊a/2⌋)`, all even; the display
shows memory row `2r` as row `r` (§6). The tick-0 decimation's rows 150..249 are therefore
visible only before the collapse; its odd rows are read only as copy sources. The two handoff
frames are two states of that walk:

- **8792 is the `a = 7` pass**, with `fadepals[56]`: rows 194..206 written, displayed band
  rows 97..103.
- **8800 is the `a = 3` pass**, with `fadepals[60]`: rows 198, 200 and 202 written, the two
  closing copies (`:104-105`) black 198 and 202, and the line loop up to tick 19 has written
  only x = 20..38 and 282..300, so the 234-pixel line at 40..276 is untouched.

The handoff test asserts 0 differing bytes at ticks 10, 13 and 18. Tick 13 is the
closing-copy state, the same bytes as 8800, which the reference captures at 8795. It also
asserts tick 9 differs from 8792 and ticks 17 and 19 equal 8800, the reference's own held
window (`8799 = 8800 = 8801`, `reference-indices.md` §10.3).

## 6. The display

CRTC `09h = 0x41` makes `M_VGA` double-scanned at 200 logical rows (`vga_draw.cpp:2688-2720`),
and CRTC `13h = 0xA0` is 320 plane bytes = 1280 pixels a displayed row, two memory rows
(`vga_draw.cpp:1785-1792`). The port's view is 320×200, `start_addr` 0,
`y_line_offset = Some(1280)`. The dot at source (160, 200) is displayed at (160, 100)
(`the_line_walks_in_from_both_edges_and_the_dot_is_at_one_sixty_one_hundred`). Tick 0 keeps
the 400-line view (`start_addr = 16000×4`, `y_line_offset = Some(640)`), because
`tw_setstart(100*160)` precedes the mode change.

## 7. Mapping: `boot = 8782 + tick`

The handoff test puts the `a = 7` state at tick 10, the closing-copy state at tick 13 and the
held line at tick 18; the reference numbers them 8792, 8795 and 8800. 8782 lies in the
reference's start window `(8780, 8790]` (`reference-indices.md` §10: the 320×400 screen is gone
after 8780 and the collapse is visible from 8792). The part retires at tick 121, boot 8903,
which calibrates the post-PANICEND cue wait (D38, `part-sequence.md` §10.4).

9000 is tick 218, inside the shell's hold of the tick-121 screen; the hold changes nothing,
so the standalone run answers it. The manifest and anchor rules are `part-sequence.md` §10
(mapping, pre-roll and block-seam conventions). In a full run the part's first frame is a
`FULLRUN_SEAMS` row (16,193 in the original, 16,181 in the port; A53).

## 8. Verification

| keyframe | run | tick | plane | differing px | register |
|---|---|---|---|---|---|
| 8792 | handoff (unit test) | 10 | RGB | 0 bytes | exact |
| 8795 (8799..8801 hold) | handoff | 13 | RGB | 0 bytes | exact |
| 8800 | handoff | 18 | RGB | 0 bytes | exact |
| 8792 | standalone | 10 | RGB | 1,638 | A63 |
| 8800 | standalone | 18 | RGB | 234 | A63 |
| 9000 | standalone | 218 | RGB and `.idx` | 0 | exact |

The standalone rows are the `panicend` entry of the rediff table (`xtask/src/rediff/table.rs`):
1,638 and 234 are exactly the reference's non-black pixels (7 × 234 and 234). There is no D24
divergence.

Reproduce:

```bash
cargo xtask rediff --only panicend
cargo test -p secondreality --bin secondreality parts::panicend
```

## 9. Retirement: the shell owns it

`SHUTDOWN.C` has no `dis_exit()` call (`grep -c dis_exit` prints 0); the original's exit
reaches it only through the shell's `partexecute`. `Panicend::frame` never reads
`ctx.exit_requested`: with the flag set on every tick it still returns `Finished` at tick 121
(`the_part_ignores_the_exit_flag_and_completes_its_finite_acts`). `Sequence::tick` drops the
part at the end of the tick the flag was set in (`shell::sequence`'s
`a_part_that_does_not_drain_retires_on_the_exit_tick`). No reference frame
is at an exit.

## 10. Decisions

- `getpal` reads from entry 0 (§3); the rotation by thirteen is not reproduced.
- `copyline`'s pair duplication is reproduced (§4).
- The part never polls the exit flag (§9), and registers no copper callbacks (the source
  registers none).
- The standalone run's 8792 and 8800 are a registered limitation, A63; the handoff is exact.

## 11. Open items

- **The tick-0 view reads past the port's surface.** The surface is 64,000 plane bytes and
  the hardware's plane memory 65,536. Tick 0 starts at plane byte 16,000 with a 400-row,
  640-pixel stride, so its last 100 rows wrap at 65,536 in hardware; the port's `View` wraps
  modulo the surface. No keyframe covers that frame; every compared frame (tick 2 on) reads
  below plane byte 63,760.
- **The pulse's intermediate values are not reference-pinned.** The port computes
  `cos(a/120·6π)·31 + 32` with the platform's `cos`; the DOS libm's rounding at the six exact
  angles (a = 0, 10, …, 50) is not recoverable. The first value (63) and the last (1, in 9000)
  are pinned.

**Register.** `bugs.md` §13 (`getpal`, `copyline`, the fade tables, the collapse).
