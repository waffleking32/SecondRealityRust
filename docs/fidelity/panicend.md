# PANICEND — the collapse, the line and the dot

`PANIC/SHUTDOWN.C` (158 lines) over `PANIC/ASMYT.ASM` (`copyline`) and
`PANIC/TWEAK.ASM` (`tw_putpixel`, `tw_getpixel`, `tw_setpalette`,
`tw_setrgbpalette`, `tw_setstart`). The transcription lives in
`bins/secondreality/src/parts/panicend.rs`; the unit tests are beside it and
the process-level test is `bins/secondreality/tests/panicend.rs`.

The part is block 2's fifth (`shell.rs`'s `PART_ORDER`; `MAIN/U2.ASM:75`,
`:826-827`), reached after TECHNO. It is the demo's shortest part and the
only one that draws nothing of its own: it inherits TECHNO's screen and DAC,
collapses the picture to a line and then to a dot, pulses the dot, and
`sleep(1)`s.

A `--part panicend` run numbers its own frames from zero (`index_origin:
"part"`, `anchor: null`). The reference's indices are reached by a mapping,
not an anchor (§7).

## 1. The inheritance, and which keyframe is which kind of evidence

TECHNO's port ends in `KOE.C`'s troll: the surface is the interleaved
four-plane image of its 320×400 picture — a 160-byte plane row is 640
pixels, the picture starts at plane byte 80 of each row (pixel 320 of the
surface), and the view is 320×400 at `start_addr` 320 with
`y_line_offset = 640`. The DAC is the troll palette in bytes 0..512 —
entries 0-169 whole and entry 170's red and green — from the
`2*count + start` `setpalarea` bug (`bugs.md` §4), with older values above.
`PANICEND` reads that state through `PartCtx` exactly as the original's
separate EXE reads the VGA's memory and DAC.

That is what splits the three keyframes:

| index | what it is | evidence |
|---|---|---|
| 8792 | the collapse's band, seven rows 97-103 of 234 pixels at x = 40-276 | **handoff**: needs TECHNO's picture and DAC |
| 8800 | the line, one row 100 of 234 pixels at x = 40-276 | **handoff**: the same inherited picture |
| 9000 | the dot: one index-1 pixel at (160, 100), index 0 everywhere else | **self-contained** |

The handoff comparison is the unit test
`the_handoff_matches_the_collapse_8792_and_the_line_8800`: it drives
TECHNO's port to `Done` with the real MUSIC1 player and this part on the
**same** `Surface`, `View` and `Palette`, at the ticks §7 derives. Both
frames match byte for byte. The rig starts TECHNO as an entry part; its end
state — the troll's picture in the surface, the DAC's used entries, and the
320×400 view at start 320 — is the state the chain leaves, because those
last acts do not depend on how the music path reached the final wait.

The standalone process test compares 9000 and **records the other two as the
handoff's**: a `--part panicend` run begins on a blank surface and the
machine's default DAC, so its ticks 10 and 18 differ from 8792 and 8800 by
4,914 and 702 of 192,000 bytes
(`cargo test -p secondreality --test panicend -- --nocapture`), which is the
measured form of that limitation.

The four reference files (three frames and the 9000 index plane) are the
tree's:

```text
$ sha256sum reference/parts/panicend/frame_008792.rgb \
            reference/parts/panicend/frame_008800.rgb \
            reference/parts/panicend/frame_009000.rgb \
            reference/parts/panicend/frame_009000.idx
c9bf5114b3e70b841884ae7a075684ee2757983760355a7d8a8aedb914ae9484  frame_008792.rgb
bb38c96bfd2d1da8ae99a5e1569d5c2e99431b3e005a8ca0b5db989a173a3665  frame_008800.rgb
b412a26518f5e0281790d05c16ed0c7644b7382c5debb8f16a9437b7a6aa84e7  frame_009000.rgb
9d7106cabab6980fa3e0f574a5fe2de7a3f8370fd5f336fb2bdaebb48b842c06  frame_009000.idx
```

`the_reference_frames_are_the_collapse_and_the_line_they_describe` pins the
shapes from the files themselves: 8792 is seven rows (97..=103), each 234
pixels from 40 to 276 with x = 273..275 black; 8800 is exactly row 100 with
the same 234 pixels and the same three-pixel gap; 9000's `.idx` holds one
non-zero byte, index 1 at (160, 100). The gap is not a defect in the
picture — it is `copyline`'s (§4).

## 2. What the part does, tick by tick

`SHUTDOWN.C` is a straight line of acts punctuated by `dis_waitb()`. The
source executes **121 waits**: one at `:66`, one at `:83`, one at `:90`, the
collapse loop's eleven at `:93`, the line loop's 47 at `:118` and the
pulse's 60 at `:132`. The port gives each segment between waits its own
tick, including the empty segment between the consecutive `:90` and `:93`
waits — tick 2 displays the mode-change state and tick 3 is the first
collapse pass — so ticks 0-120 are the part's 121 drawn frames and tick 121
is `sleep(1)`'s retirement:

| tick | source | acts |
|---|---|---|
| *init* | `:30-55` | `dis_partstart`; `getpal` (§3) |
| 0 | `:61-65` | black row 0; build the 64 fade tables; the 100-row 4:1 copy; `tw_setstart(100*160)` |
| 1 | `:66-84` | CRTC `09h = 41h`; GC `05h = 41h`; map mask `0x0F`; `fadepals[3]`; entry 0 white; `tw_setstart(0)` |
| 2 | `:84-90` | `fadepals[20]`; CRTC `13h = 0xA0` (the empty segment before `:93`) |
| 3-12 | `:91-102` | the first ten collapse passes |
| 13 | `:91-105` | the last pass (`a = 3`) **and** the two closing copies of rows 202 and 198 |
| 14 | `:106-127` | GC `05h = 40h`; the line's first iteration (`x = 20`) |
| 15-60 | `:116-127` | the rest of `for(x=20;x<=160;x+=3)`: 47 iterations |
| 61 | `:129-135` | `tw_putpixel(160,200,1)`; the pulse's first value |
| 62-120 | `:130-136` | the rest of the 60-frame cosine pulse |
| 121 | `:136` | `sleep(1)`: the part returns `Finished`; the shell holds its last frame |

The collapse's `a` values are `32, 26, 21, 17, 14, 11, 9, 7, 5, 4, 3` —
`for(a=32;a>2;a=a*5/6)` in C integer arithmetic, which ends at 3. The two
closing copies are in the last pass's inter-wait segment, after the `a = 3`
copies and before the line loop's first `dis_waitb()` (`:118`);
`the_part_ignores_the_exit_flag_and_completes_its_finite_acts` asserts the
last tick is `FINISHED_STEP` (121), built from the same constants the match
uses.

### The display

The inline writes are the whole of the CRTC/GC state:

- `mov ax, 4109h` — AL is the index, AH the data: CRTC `09h = 0x41`, which
  makes the emulator's `M_VGA` a double-scanned mode of **200 logical rows**
  (`vga_draw.cpp:2688-2720`), halving the 400-line screen.
- `mov ax, 4105h` (GC) — write mode 1 for the collapse's `copyline`s.
- `mov ax, 0f02h` (sequencer) — map mask `0x0F`.
- `mov ax, 0a013h` — index `0x13`, data `0xA0`: the line offset is 0xA0
  words = 320 plane bytes = **1280 pixels**, two memory rows.
  `vga_draw.cpp:1785-1792`'s `M_VGA` address advance is `scan_len * 8`.
- `tw_setstart(0)` — display start 0.

A displayed row is therefore memory row `2r` (stride 1280 pixels = 320 plane
bytes = two 160-byte rows), and its first 320 pixels are the row's first 80
plane bytes. The dot at source (160, 200) is memory row 200, plane byte 40,
plane 0 — displayed at **(160, 100)**, which is the reference's single
non-black pixel. `the_line_walks_in_from_both_edges_and_the_dot_is_at_one_sixty_one_hundred`
pins the line's 47 iterations over x = 20..300 of row 200 and the dot's
displayed position.

The port models the displayed result as a `View`: width 320, height 200,
`start_addr = 0`, `y_line_offset = Some(1280)`, `pan = 0`, `border = 0`.
Tick 0 leaves the 400-line view (`start_addr = 16000×4`,
`y_line_offset = Some(640)`), because the source's `tw_setstart(100*160)` is
written before the mode change.

## 3. `getpal`: `0d` is decimal zero, not `0x0D`

This is the one place where the survey's reading and the source's actual
semantics differ, and the reference settles it.

`SHUTDOWN.C:148` is `mov al, 0d`. In the assembler the tree uses, a trailing
`d` is the **decimal** suffix — `cmp [BYTE PTR fs:bx], 0d` and `mov bx, 0d`
are the tree's own zeros — while the hex spelling needs an `h`, as
`TWEAK.ASM:200` writes `mov al, 0dh` for the CRTC's start-address register.
So `getpal` sets the DAC read index to **0** and reads 768 bytes from it:
the inherited DAC, entry for entry, unrotated. There is no rotation by
thirteen; the byte `0d` is not `0x0D`.

The reference is the proof, not the disassembly alone: the fade tables built
from the unrotated inherited palette reproduce 8792 and 8800 byte for byte,
and a table built from a 13-entry rotation does not. The unit test
`getpal_reads_from_dac_entry_zero_because_0d_is_decimal` pins both halves —
the source's spelling (and TWEAK's contrasting `0dh`) and the identity read —
and the handoff test pins the frames.

This is a correction to the survey's "`getpal` reads the DAC starting at
index 13", not a D24 divergence: the source, the assembler and the reference
all say entry 0, and the port matches all three keyframes. The alternative
would need the shipped `TROLL.UP`'s palette and pixel values to be shifted by
thirteen in opposite directions — a compensating pair that would leave the
RGB identical at 8400 — and there is no evidence for it: the vendored
`TROLL.UP` is the file the port reads, and with it only entry 0 reproduces
the reference.

## 4. `copyline`: 80 words, and the latch's second byte

`ASMYT.ASM:9-24` is `rep movsw` with the count in words. The call sites pass
80, so a copy moves **160 planar bytes** — one full 160-byte memory row, 640
display pixels in the interleaved model — not the 320 pixels a "one displayed
row" reading would suggest.

The copy runs in GC write mode 1 (`SHUTDOWN.C:70-79`, restored to mode 0 at
`:106-114`). In write mode 1 the destination data is the VGA latch, not the
CPU's; the latch is loaded by the source **read**, and a 16-bit read loads
the four-plane latch twice, leaving the **second** planar byte's value. The
16-bit write then stores that same latch to both destination planar bytes.
In the port's surface terms, each 8-pixel group becomes the source's second
4-pixel group repeated twice:

```text
dest[8k .. 8k+3] = src[8k+4 .. 8k+7]
dest[8k+4 .. 8k+7] = src[8k+4 .. 8k+7]
```

This is why the reference's collapsed line is **234 pixels at x = 40..276
with x = 273..275 black** rather than the troll's own 233 at 44..276: the
pair duplication moves the left edge four pixels **outward** (from 44 to 40)
by repeating the first four-pixel group, and repeats the picture's last
partial group — byte 149's one lit pixel out of four — into the destination
group, which is where the three-pixel hole at 273..275 comes from. A
straight byte copy produces 233 at 44..276 and fails 8792 and 8800; the port
reproduces the reference exactly.
`copyline_moves_eighty_words_and_stores_the_second_byte_of_each_pair_twice`
pins the rule and the 160-plane-byte/640-pixel count on a synthetic row.

Copies may overlap their own sources: the `b = a/2` copy of the `a = 32`
pass (from plane byte 32,080 to 32,000) has a source span that overlaps its
destination. The port runs the word moves in increasing order against the
memory as it stands, which is what `rep movsw` with DF clear does.

## 5. The fade tables, and the palette the part leaves

`SHUTDOWN.C:62` builds 64 tables:

```c
for(a=0;a<64;a++) for(b=3;b<768;b++)
    fadepals[a][b]=(a*63+kuvapal[b]*(64-a))/64;
```

The loop starts at `b = 3` and the tables are zero-initialised, so **entry 0
is black in all 64**. `:81`'s `tw_setrgbpalette(0,63,63,63)` makes entry 0
white for the tick-1 frame only; `:84`'s `tw_setpalette(fadepals[20])`
overwrites it. `the_fade_tables_leave_entry_zero_black_and_start_at_byte_three`
pins all 64 tables from the rule and entry 0.

The collapse's last upload is `fadepals[60]` (the `a = 3` pass). Nothing
after it changes entries 2-255; the pulse writes entry 1 only. After the
last pulse value the DAC is entry 0 = (0,0,0) and entry 1 = (1,1,1) —
`the_part_ignores_the_exit_flag_and_completes_its_finite_acts` asserts both —
which is exactly what makes 9000 self-contained: the one index-1 pixel is
drawn through (1,1,1) and everything else is index 0.

## 6. The collapse and the two keyframes

Each pass copies row 0 (black) over rows `200 ± 2b` for `b = ⌊a/2⌋..a`,
then runs, for `b = 0..a-1`, `copyline` from plane byte
`80 + (400b/a)·160` — **picture row `400b/a`, at that row's byte 80** — to
plane byte `200·160 + (b − ⌊a/2⌋)·320`, which is memory row
**`200 + 2(b − ⌊a/2⌋)`**. The destinations are therefore all even, and
`a = 7` writes rows 194, 196, …, 206 while `a = 3` writes 198, 200, 202.

The display reads only even memory rows (§2), so the first copy's rows
150..249 — the 4:1 decimation — are visible only in the pre-collapse
segments; its odd rows are read as *sources* by the third loop, and their
first halves land in the destination's invisible half. The destination's
visible half is built from the source row's picture half (`copyline`'s pair
duplication, §4).

The reference's two handoff frames are two states of that walk:

- **8792 = the `a = 7` pass's state**, with `fadepals[56]` uploaded in the
  same iteration: rows 194..206 are written, and the displayed band is
  rows 97..103.
- **8800 = the `a = 3` pass's state**, with `fadepals[60]`: its third loop
  writes rows 198, 200 and 202, and the two closing copies of rows 202 and
  198 (`SHUTDOWN.C:104-105`, in the same inter-wait segment) black the outer
  two, leaving row 200. The line loop's iterations up to tick 19 have
  written only x = 20..38 and x = 282..300, so the 234-pixel line at
  x = 40..276 is untouched. A straight-line copy of the picture rows would
  leave a different width and fail the comparison (§4).

The handoff test compares the composed 320×200 frames at ticks 10, 13 and
18 and asserts **0 differing bytes** at each: tick 13 is the closing-copy
state, which is the same bytes as 8800, and it is the frame the reference
captures at 8795. The test also asserts tick 9 differs from 8792 and
ticks 17 and 19 equal 8800, which is the reference's own held-window
observation (`8799 = 8800 = 8801`, `reference-indices.md` §10.3).

## 7. The numbering: `boot = 8782 + tick`

The handoff test puts the `a = 7` state at tick 10, the closing-copy state
at tick 13, and the held line at tick 18; the reference numbers those 8792,
8795 and 8800. The mapping is therefore **boot = 8782 + tick**. 8782 lies in
the reference's start window `(8780, 8790]` (`reference-indices.md` §10: the
320×400 screen is gone after 8780, the collapse is visible from 8792, and
8795-8799 are the held line). It is a mapping, not an anchor:
`boot_anchor("panicend")` stays `None` and a `--part panicend` run's
manifest says `index_origin: "part"`, `anchor: null`.

9000 is deep inside the loader's hold after the part's `sleep(1)`; under the
mapping it is tick 218, and the part itself has retired at tick 121. The
standalone test dumps 218 — the mapping's own number — and compares both the
RGB and the `.idx` to 9000 with **0 differing bytes**. The part's own frames
are numbered from zero, so its tick 218 is the shell holding the tick-121
screen; the two agree because the hold changes nothing.

## 8. The comparison

```bash
$ cargo test -p secondreality --bin secondreality parts::panicend
...
the_handoff_matches_the_collapse_8792_and_the_line_8800 ... ok
...
test result: ok. 9 passed; 0 failed

$ cargo test -p secondreality --test panicend -- --nocapture
tick 10 vs 8792: 4914 of 192000 bytes differ
tick 18 vs 8800: 702 of 192000 bytes differ
...
test result: ok. 3 passed; 0 failed
```

| keyframe | evidence | frame | comparing test | differing |
|---|---|---|---|---|
| 8792 | handoff | 320×200 RGB | `the_handoff_matches_the_collapse_8792_and_the_line_8800` (tick 10) | 0 |
| 8795 (8799-8801 hold) | handoff | 320×200 RGB | the same test (tick 13, the closing copies) | 0 |
| 8800 | handoff | 320×200 RGB | the same test (tick 18) | 0 |
| 9000 | standalone | 320×200 RGB + `.idx` | `the_standalone_run_matches_the_self_contained_dot_9000_exactly` | 0 |

**There is no D24 divergence and nothing new for `APPROXIMATIONS.md`.** Two
boundaries are recorded rather than approximated:

- **The tick-0 view reads past the port's surface.** The port's surface is
  400 rows × 160 plane bytes = 64,000 plane bytes; the hardware's plane
  memory is 65,536. The frame the source displays at tick 0 starts at plane
  byte 16,000 with a 400-row, 640-pixel stride, so its last 100 displayed
  rows address plane bytes above 64,000 and wrap in hardware at 65,536. The
  port's `View` wraps modulo the surface instead. No reference keyframe
  covers that one frame; every compared frame (ticks 2+) reads below
  plane byte 64,000 (at most `199×320 + 79 = 63,759`).
- **The pulse's intermediate values are not reference-pinned.** Only its
  last value is visible in a filed frame (9000's entry 1 = (1,1,1)); the
  port computes `cos(a/120·6π)·31 + 32` with the platform's `cos`, and the
  source's DOS libm's rounding at the six exact-integer angles (a = 0, 10,
  20, 30, 40, 50) is not recoverable from the tree or the captures. The
  first value (63) and the last (1) are pinned.

## 9. Retirement: the shell owns it, and the part never polls

`SHUTDOWN.C` contains no `dis_exit()` call: the part's acts are a fixed line
of waits, and the original's exit reaches it only because the shell's
`partexecute` ends the part's EXE. The port does the same, twice over:

- `Panicend::frame` never reads `ctx.exit_requested`. With the flag set on
  every tick from outside, the part's acts are identical and it still
  returns `Finished` at tick 121 —
  `the_part_ignores_the_exit_flag_and_completes_its_finite_acts`.
- `Sequence::tick` reads `Engine::exit_requested` back after the band and
  drops the part at the end of that tick —
  `the_shell_retires_the_part_at_the_end_of_the_tick_the_exit_was_set_in`
  (the real part, ESC from outside) and
  `the_shell_also_retires_a_part_that_requests_its_own_exit` (a stub that
  polls, for the shell's other path).

This is the same arrangement as the Phase-4 parts that do not poll, and it
is why the part is listed as shell-retired rather than exit-aware.

## 10. The preserved artefacts

- `getpal`'s `0d` is decimal zero (§3): the inherited DAC is read from
  entry 0. The intended-but-never-executed rotation by thirteen is not
  reproduced, because it is not what the binary does.
- The fade tables leave entry 0 black, and `:81`'s white entry 0 is
  overwritten at `:84` (§5).
- `copyline` moves 80 words and, in write mode 1, stores the pair's second
  planar byte to both destination planar bytes (§4).
- The collapse ends at `a = 3`; its third loop writes even rows 198, 200 and
  202, and the two closing copies clear 198 and 202, leaving only row 200
  (§6).
- Nothing beyond memory row 0 is cleared, and the fade tables are computed
  over all 768 bytes with `b` starting at 3.
- No exit poll and no copper callbacks (the source registers none).

## 11. The tests

In `src/parts/panicend.rs`:

- `getpal_reads_from_dac_entry_zero_because_0d_is_decimal` — the source
  spelling, TWEAK's contrasting `0dh`, the identity read and the rejection
  of the 13-entry reading.
- `the_fade_tables_leave_entry_zero_black_and_start_at_byte_three` — all 64
  tables from the rule.
- `copyline_moves_eighty_words_and_stores_the_second_byte_of_each_pair_twice`
  — the word count, the 640-pixel span and the pair duplication.
- `the_line_walks_in_from_both_edges_and_the_dot_is_at_one_sixty_one_hundred`
  — the 47 line iterations, the dot's source and display coordinates, and
  the blank-surface frame's single visible pixel.
- `the_part_ignores_the_exit_flag_and_completes_its_finite_acts` — tick 121
  with and without the flag, and the final (0,0,0)/(1,1,1) entries.
- `the_shell_retires_the_part_at_the_end_of_the_tick_the_exit_was_set_in`
  and `the_shell_also_retires_a_part_that_requests_its_own_exit` — the
  shell-owned retirement, real part and stub.
- `the_reference_frames_are_the_collapse_and_the_line_they_describe` — the
  reference shape numbers this note quotes.
- `the_handoff_matches_the_collapse_8792_and_the_line_8800` — TECHNO's port
  to `Done`, then this part, at ticks 10, 13 and 18, plus the neighbour
  checks.

In `tests/panicend.rs`:

- `the_run_is_numbered_in_its_own_frames_and_declares_the_music_row` — the
  manifest's `part`, `anchor`, `index_origin` and MUSIC1 order 37.
- `the_standalone_run_matches_the_self_contained_dot_9000_exactly` — RGB and
  `.idx`.
- `the_standalone_run_cannot_answer_the_handoffs_8792_and_8800_and_says_so`
  — the evidence split, measured.

The shell's music table test (`a_blocks_first_part_is_the_entry_point_its_row_is_about`)
also covers the new `music_for_part("panicend")` row: every registry name
must have an expected row.
