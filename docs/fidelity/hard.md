# HARD — the dots, the grid and the cube

HARD is `HARD/B.EXE`, the only demo part in the directory (`MAKEFILE:1-4,22-26`): `B.C`
includes `BP.C` (`B.C:123`) and links `BA.ASM` (the dots), the c-prefixed `VEC.ASM` engine
(`VEC.ASM:1-15`) and the four generated dot streams plus the grid table (`LINK.TMP:1-3`).
`GMAIN.C`/`V.EXE` is an interactive engine harness, not the effect (`HARD/V.EXE` has no DIS,
no `muscode` and a palette the program zeroes), and `ASM.A`, `LINEBLIT.INC` and the working
art are not on any link line. This note covers the whole effect; the sections marked **Task
7** are the second half's and are written when it lands.

The port translates the source literally, including the bugs the pictures depend on: the
16-bit `int` arithmetic, `part2init`'s uninitialised `b`, `do3dots`' 16-bit z load, the dot
engine's `sar 6` and unsigned clips, `dot3d2`'s colour decrement, and the mode-4 palette's
forced entries. The first half (modes 0..4) is exact against every filed keyframe; the
measured corrections to the plan's and the research's summary are recorded below.

## Capture (Phase 8 Task 2)

The reference is `reference/parts/hard`, a version-1 capture of the vendored `HARD/B.EXE`
(`1f8a7bdae29a34af42c3ffde63163661ca82a6884a01cb6eb775e81ac600c81d`) run behind the
standalone `DIS/DIS.EXE`, staged and mounted read-only by `tools/vault_capture.sh`, with one
key (`KBD_space`) to open `while (dis_muscode(1) != 1 …)` (`HARD/B.C:138`):

```bash
tools/vault_capture.sh hard --frames 480,560,630,700,770,840,910,1050,1600
```

The manifest says `exe: "RUN.BAT"`, `arguments: []`, `boot: "autoexec"`,
`boot_wait: "graphics"`, `cycles: 30000`, `frame_origin: "boot"`, `skip_boot_frames: 0`,
320×200, `keys: ["KBD_space"]`, oracle `dosbox-automation 0.85.1 (6d100)`, upstream
`071a82e`. The batch is the script's CRLF `RUN.BAT`: `@ECHO OFF`, `DIS.EXE`, `B.EXE`. The one
key never reaches the `0xff` cut (`BP.C:263` needs a second key), so part 3 runs to its own
`count>=800` end.

### Start and `K0`

The gate is followed by four `waitb` calls (`B.C:139-142`) and the pre-roll wipe — one
row-100 pixel cleared per x, a `waitb` every 16 columns, then the accelerating fill
(`B.C:143-161`) — then 70 waits (`B.C:162`). A dense sweep and its repeat
(`out/task2/hard-start`, `out/task2/hard-start-b`, byte-identical) measured: 340–355 are the
same white screen `20f3d62f…`; **356 is the first changed frame**
`e7d2f71620f10ce4cbb92a7e42f402c47635a57deeca300100b31d928d1c632d`, with **16 black pixels
at row 100, x=304..319**; 357 has 32, 358 48, … and 375 the whole row; 376–378 repeat 375 (the
fill's first iterations clear no new row); 379 has rows 100–101 black, 380 rows 100–102. The
port counts the wipe as a tick, so its frame 0 is the first wipe frame: **`K0` = 356,
tolerance ±1** (355 is still white, 357 is the 32-pixel step, so a one-frame session
alignment is visible and is what `compare`'s ±1 answers). The commands:

```bash
tools/vault_capture.sh hard --frames "$(seq -s, 340 380)"  --out out/task2/hard-start
tools/vault_capture.sh hard --frames "$(seq -s, 350 362)"  --out out/task2/hard-start-b
```

### Measured mode starts

| boot index | what changes | picture hash |
|---|---|---|
| 474 | first letterbox frame (473 is the half-black filled screen) | `1930616d436601884fe0f39831ec2ba1309c942d011f238b4c687562756f655b` |
| 474–511 | letterbox holds with no dots (38 frames: the letterbox frame, mode 0's 34 iterations, `part1init`'s two `waitb` frames and the first part1 iteration's `waitb`) | `1930616d…` |
| 512 | first spawned dots (mode 1, logo phase 1) | — |
| ~512–585 | logo phase 1, `dots1`; **FUTURE** traced in | 560 = `318503a6…` |
| ~587–660 | logo phase 2, `dots2`; **CREW** traced over FUTURE | 630 = `788e0a7e…` |
| ~662–735 | logo phase 3, `dots3`; **RULES** over CREW | 700 = `9574495d…` |
| ~737–809 | logo phase 4, `dots4`; **FOREVER** over RULES | 770 = `cbdf6e0c…` |
| 810–851 | mode 3: dots dead, FOREVER held (810, the first sampled identical frame, through 851 are byte-identical) | `f2718de8cb4357740cdc467cc13a511a56e3415bd47c2f6b841b944264c016c7` |
| 852 | first mode-4 band frame (851 is still the mode-3 hold) | `963a0272243acd9488092fddf49912afb511f789f538ce4148b8e6794568208f` |
| 852–955 | mode 4 band; 930–955 hold the last band state | `9cced0af…` |
| 956 | first mode-5 frame (`part2init`; the band is rebuilt) | `0495730ecff1097c279926c72dac44fd180f89b103dc6c09bd888109982e519d` |
| 958 | first part-2 grid frame distinct from the init pair (956/957) | — |
| 1455 | last part-2 frame (fading grid) | `996c4880…` |
| 1456 | part 2's fade leaves the empty band (same bytes as the 474 letterbox) | `1930616d…` |
| 1458 | first part-3 cube frame | `e31eb951ac946b583e1ea7b073662abd2b99baafaa1a808e32a03b6f29a1052f` |

Every row is measured from one of these batches, one `vault_capture.sh hard` run each:

```bash
tools/vault_capture.sh hard --frames "$(seq -s, 470 520)"     --out out/task2/hard-470-520
tools/vault_capture.sh hard --frames "$(seq -s, 500 10 830)"  --out out/task2/hard-500-830
tools/vault_capture.sh hard --frames 840,845,850,855,860,865,870,875,880 \
                              --out out/task2/hard-840-880
tools/vault_capture.sh hard --frames "$(seq -s, 846 860)"     --out out/task2/hard-846-860
tools/vault_capture.sh hard --frames "$(seq -s, 930 5 1000)"  --out out/task2/hard-930-1000
tools/vault_capture.sh hard --frames "$(seq -s, 948 962)"     --out out/task2/hard-948-962
```

The dense batches are contiguous or five apart, so D19's read never sits between two grabs;
the 100-frame map (`--frames "$(seq -s, 475 100 2175)"`) is the coarse cross-check. The
`930-1000` batch is where the 948–955 hold is byte-identical and 956 first differs (by 100
pixels: the band's index-189 trails become mode 5's index 1 under part 2's palette, which is
white in both, so only the non-189 dot pixels change).

### Keyframes

| index | mode / phase | what is on screen | `sha256` |
|---|---|---|---|
| 480 | 0 | letterbox, empty band (no dots yet) | `1930616d436601884fe0f39831ec2ba1309c942d011f238b4c687562756f655b` |
| 560 | 1 / phase 1 | `dots1` tracing FUTURE | `318503a68ad4dd6489124a0b4496dcecc6db1085eda16323eca1fb5c2281063a` |
| 630 | 1 / phase 2 | `dots2` tracing CREW over FUTURE | `788e0a7ee7d0f58e3865443a1edc7b6f6ebfd37ed9f1aca77a5aa59ac43cfc19` |
| 700 | 1 / phase 3 | `dots3` tracing RULES over CREW | `9574495dda2d6aa237bffd29f01ce376a4c7317abe30e16a78f0c99fc572e920` |
| 770 | 1 / phase 4 | `dots4` tracing FOREVER over RULES | `cbdf6e0cb30941281d2bc00ce45b878995f0222ae46b0fe1fad10b7535a331e2` |
| 840 | 3 | FOREVER held, dots dead | `f2718de8cb4357740cdc467cc13a511a56e3415bd47c2f6b841b944264c016c7` |
| 910 | 4 | the grey/blue band | `59f2d5fc1cc9d6839344acbc230e8b9c7c2c20457a4fbf3b7ea9ef8c8985caa2` |
| 1050 | 5 | part 2's 3-D dot grid | `c1743f6af0375ecb76c4f939724b7f2b1897a2665af9c93f44cabb7aa76a9367` |
| 1600 | 6 | part 3's glenz cube | `5894039bea2564ce78bde3c479c80a986f144f59d76af15919e7a238a1b9858d` |

70–550 apart, one per stage. **`840` and `480` are the held keyframes.** `840` is the one
whose indexed read reproduced its RGB (`frame_000840.idx` plus `idx_sha256` in the manifest);
the filed `480` is RGB-only because in the filed run its read landed after mode 0 had ended
(the trial session `out/task2/hard-reference-trial` caught it while it still held and wrote an
`.idx` then; filing that file would change the reference census, so it stays scratch). The
seven moving frames are RGB-only (D15/D19).

### Reproduced

480 and 1600 (the two most different pictures) were captured again in a second session
(`out/task2/hard-repeat-480-1600`) and are byte-identical to the filed `1930616d…` and
`5894039b…`; a separate nine-frame trial session is byte-identical at all nine indices too.
`cargo xtask reference verify --tree reference` passes over the capture.

## The part (Phase 8 Task 6)

### The pre-roll is 119 display ticks

The port counts one `waitb` as one engine tick and one displayed frame, and it is the
**display** that is modelled: frame `t` is the picture the capture shows at reference
`356 + t`. The source's pre-roll then unfolds as:

| ticks | source | what changes |
|---|---|---|
| — | `B.C:131-142` | `dis_partstart`, mode 13h, the white page (index 15), the start gate and four `waitb` calls; these precede the first drawn frame and are `init` |
| 0–19 | `B.C:143-147` | the row-100 wipe: 16 columns a tick (x = 319 down to 0), `waitb` after each 16. Tick 0 is the capture's 356 (16 pixels), tick 19 its 375 (the whole row) |
| 20–47 | `B.C:148-161` | the fill: `b += 16; a += b; y2 = a/64` (clamped 200), clear rows `y1..y2`, one `waitb`. 28 iterations reach y1 = 200; the first two clear no new row and the third re-clears row 100 (capture 376–378 repeat 375) |
| 48–117 | `B.C:162` | 70 waits, no change (capture 404–473) |
| 118 | `B.C:164-201` | the palette, border and letterbox setup; the capture's 474, the first letterbox frame |
| 119… | `B.C:203` | the main loop |

The wipe's 20 steps and the fill's 28 are pinned by `the_pre_roll_is_the_sources_tick_counts`;
the letterbox tick is **`K0` + 118 = 474**, and the main loop's first iteration (count 0) is
port 119.

### The display is the A2D0 band inside a letterbox

The work buffer is `unsigned char far *vram = 0xa2d00000` (`B.C:17`) and mode 13h displays
`A000:0000`, so the 128-row band appears at screen rows 36..163 (`0xA2D0 - 0xA000 = 0x2D00` =
11,520 = 36·320). The five `memset` bands (`B.C:168-172`) are the picture's frame:

| rows | index | palette entry |
|---|---|---|
| 0..34 | 255 | `(0,0,0)` |
| 35 | 254 | `(45,45,45)` |
| 36..163 | the 128-row work band | the band's own indices |
| 164 | 254 | `(45,45,45)` |
| 165..199 | 255 | `(0,0,0)` |

`defpal` is zero except entries 0 = `(0,0,20)`, 254 = `(45,45,45)` and 255 = `(0,0,0)`
(`B.C:173-182`). The whole 320×200 screen is the part's: the port owns a 64,000-byte screen,
the dot engine writes `screen[11520..52480]` with the source's band offsets (`rows[y] =
y*320`), and the letterbox rows are never touched by the engine. The view is 320×200, origin
0, `y_line_offset = Some(320)`, no split (the source registers no copper and no line compare).
`setborder` is inert: the C `border` variable is 0 (`B.C:26,196,201`) and its only other
write is the one-off attribute register pair at `:166-167`, modelled as a black view border.
`the_letterbox_and_defpal_are_the_sources` pins the rows and entries.

### The state machine and the display phase

`count=0, mode=0` (`B.C:26`); every iteration does `count++`, `setborder(255)`, one `waitb`,
the palette uploads, `setborder(0)`, `dodots`/`dodots2`, then `switch(mode)`
(`B.C:203-324`). `NEXTMODE` is `{ count = -1; mode++ }` (`B.C:10`). Modes 0..4:

| mode | iterations | work |
|---|---|---|
| 0 | 34 (counts 0..33) | `dodots` on `dots1`; `count>32` → next |
| 1 | four phases × 73 (counts 0..72) | `part1init` at 0 (`BP.C:6-33`), `part1` after (`BP.C:35-72`); `count>=72` → next |
| 2 | one frame | `NEXTMODE`, then `dots1→dots2→dots3→dots4`; the fourth rotation kills all 2,048 dots |
| 3 | 42 (counts 0..41) | dots only; `count>40` → next |
| 4 | 102 (counts 0..101) | `part1colp = 1`, the background rebuilt from the last stream with colour 254, the moving 66-entry band, `count>100` → next; the `count>400` branch (`B.C:243-246`) is dead |

The display phase is measured, not fitted. A source iteration's palette upload and drawing are
seen at the **next** retrace, so the port's tick performs the iteration and shows its result;
`part1init` calls its own `waitb` (`BP.C:29`), which is one extra display frame for mode 1's
count 0. That puts the portrait at: letterbox 474 (tick 118), mode 0's 34 iterations
475–508 (ticks 119–152), `part1init`'s two frames 509–510 (ticks 153–154), the first `part1`
spawn at 511 (tick 155), the first drawn dots at 512 (tick 156) and phase 1's count 50 — the
palette the capture shows — at 560 (tick 204). Mode 3's count 0 is reference 809 (tick 453);
mode 4's count 0 is reference 851 (tick 495, a picture identical to the mode-3 hold), and its
count 1 is the first visible band frame, 852. `the_first_half_mode_sequence_is_the_sources`
pins the counts and ticks; `hard_matches_the_reference_in_its_early_modes` is the measurement.

Mode 4's palette is deliberately one iteration behind: `B.C:208` uploads at the top of the
iteration, before the switch rebuilds the band, so the frame at count n shows the band built
at count n−1. The capture says so: reference 852's first band has `a = 252 - pal1colp` at
`pal1colp = 1` (entries 251..253), not at 2.

### The palettes are the source's byte writes

`pal` is the pending C array and `dac` the DAC mirror; `loadpal(n)` copies the first `n`
bytes only (`B.C:68-82`). **`part1` uploads 32 entries (96 bytes)** (`BP.C:59`); the pre-roll,
mode 4 and the later parts upload 768. `part1init` builds `pal1` from `b = (64-a)²/64`,
clamped, in three groups scaled by 1, 2/3 and 1/3, with the blue plane `(64-a)·4/6 + 20`
(`BP.C:10-28`). `part1` writes two three-entry groups at the literal byte offsets
`-12 + pal1colp*3 + {3,6,9}` and `pal1colp*3 + {3,6,9}`, from `b = (count-6)` clamped and
`b = 64 - min(count,64)`, both `·3 + 48` (`BP.C:38-58`) — so the four phases write entries
1..3/5..7, 5..7/9..11, 9..11/13..15 and 13..15/17..19. `the_part1_palette_uploads_only_32_entries`
pins the capture's count-50 group 0 = `(39,39,53)` and asserts every write is inside the
96-byte upload.

Mode 4's band (`B.C:269-286`) is `a = 252 - pal1colp`, 66 entries clamped at 254, `c` from 63
down by 4 (`c<0 → 0`), `b = c*c/64`, `(b,b,max(b,20))`, and then **entries 189 and 1 are
forced to 63**. The plan and the research say "`pal[251] = pal[3] = 63`"; the tree says
`a = 254-65 = 189` and `pal[1*3]`, and the capture agrees: mode 5's rebuild tests
`vram[u] == 254-65` (`B.C:299`), and the held 930–955 band's only distinguishable trail
pixels are 100 white ones at index 189 (`out/task2/hard-930-1000`). The band is displayed one
iteration behind, as above. `the_mode_four_band_palette_is_the_sources` pins the entries.

### The dot engine

`adddot` (`B.C:84-98`) stores six 16-bit words per record in `_dotxyz[2048]` (`BA.ASM:7-12`):
`x*64 + 32 - (ax-x)`, the same for y, `ax-x`, `ay-y`, `colour + 66*256`, and `x + y*320`. The
high byte is a 66-frame life; the low byte the colour; a colour of 0 marks a dead record with
`0x100`.

`dodots` = `dot3d` (`BA.ASM:75-122`): for every record in order, a dead one is skipped; a
record whose life reaches 0 writes its colour into the background **and** the screen at `last`
and is marked `0x100`; otherwise the previous screen pixel is restored from the background,
the fixed-point y then x advance by `vy`/`vx`, and the dot draws when `y <= 127` and
`x <= 319` — both **unsigned** compares after `sar 6`, so a negative coordinate is clipped,
and the x advance is skipped when y is out. `dodots2` = `dot3d2` (`:124-171`) is the same walk
without the background or the restore, and with `dec dl` (`:144`): **the colour decrements
every frame**, so a mode-4 dot spawned at 254 walks down the palette and its last drawn
colour is 189 after 65 frames; the 66th call kills it without drawing. The 930–955 hold's
100 white pixels are those last positions. `the_dot_engine_leaves_the_66_frame_trail` pins the
record, the restore, the 66-frame life, the clips and the decrement.

Part 2's `do3dots` (`BA.ASM:224-280`) is ported and pinned now for Task 7: it clears the
previous pixel and a two-row shadow, loads z's **low word** into `bp`, skips it when the
16-bit value is signed `> 3000`, then reads `depthcol[bp]` and draws at `rows[y] + x` when
`y <= 127`. `depthcol` is 8,192 bytes with 0..3,099 written (`BA.ASM:221-222`,
`BP.C:128-133`); a z whose low word is negative passes the signed test and indexes past the
array, where the BSS is zero. `the_shadow_grid_dot_reads_bss_zero_past_3100` pins that rule
and the 16-bit load. (The plan's "z>3099 reads colour 0" is not reachable for a positive z —
the `jg` skips it; the reachable BSS read is the negative low word.)

### The data

`BDO.C:12-41` extracts four 40-row bands of `FCRZ.U` (64,000 bytes; SHA-256 `e2c068fe…`) as
`(value, 0, x-160, y-20)` records for `0 < value < 4`, zero-terminated. The measured counts
are **859 / 690 / 722 / 1,071** and the picture's histogram is 60,645 zeros, 164 ones, 441
twos, 2,737 threes and 13 fours. The linked streams in `B.EXE` at 55,875 / 59,315 / 62,082 /
64,977 match byte for byte except the last terminator byte of bands 1 and 2 (the next linked
object's bytes abut); `the_dot_streams_are_the_bdo_extraction` pins the extraction and the
cross-check. The spawner reads the bytes with the source's `dotsp += 4` before the first
record, so **record 0 is never read**, and a terminator resets `dotsp` to 0, so the next read
is record 1 again (`BP.C:60-69`, `B.C:251-261`). That is also why a 256-iteration spawn frame
is not 256 spawns: a stream of R records yields `N - N/R` dot calls.

The 400-byte `_GRID1.TMP` is `GRID1: [i16; 200]` (`doit2`'s 100-pair cycle, `B.C:112-121`).
`B.EXE`'s linked copy at file offset 69,265 matches it for **399 of 400 bytes**; the last byte
is `0x4F` in the binary against `0x00` in the file, so the binary's last word would be
`0x4F5A` rather than the file's `0x005A`. The port uses the file's table and
`the_grid_table_is_the_shipped_grid1` pins the difference; the affected word is the y of the
100th pair, and no filed keyframe depends on it.

There is **no `rand()` pre-roll**. The plan and the research put a 60-call burn at `B.C:122`;
the vendored `B.C` has no such loop, and the capture agrees: with the burn the port's first
drawn frame (port 156) is 92 pixels where the capture's 512 has 93 and the four logo
keyframes differ by thousands of pixels; without it the first-draw set is the capture's 93/93
and all seven keyframes are exact. `the_first_spawn_is_the_sources_rand_sequence` pins the
first spawned record's six words against the MS C 6.0 LCG (`MsRand`, `minvball.rs:243-265`)
with no burn. The 60-call figure in the research is `EDGE.C:122`'s loop, a different program.

### The mode-5 boundary

Mode 4's last iteration is count 101, so the source's part 2 starts at the port tick 102
iterations after mode 4's count 0: reference **953** (851 + 102). The filed artifact's first
mode-5 frame is **956** (`out/task2/hard-948-962`: 948–955 hold, 956 first changes). Three
display frames are unaccounted for by the source's loop — the lead is a missed-retrace
overrun (D22), which the port's fixed tick cannot model; Task 7 owns the boundary and the
part-2 keyframes. The port retires on the mode-4 advance tick (port 596) and the shell holds;
`hard_ends_at_the_mode_five_boundary` pins the hold and
`the_first_half_mode_sequence_is_the_sources` the tick.

### What is Task 7's

Modes 5 and 6 (`part2init`/`part2`, `part3init`/`part3`), the `part2init` uninitialised `b`,
the `0xff` cut, `fadepal` and the final `default` exit; the compositor band seeding and the
clipper's `Norm`/`Hide` passes (Task 5's deferred minors). The port's `do3dots`, `GRID1`,
`Dot` and the engine interfaces are in place for it.

## Verification

The first-half comparison is exact. `cargo test -p secondreality --test vault -- --nocapture
hard_matches_the_reference_in_its_early_modes` prints, for every filed keyframe before mode 5,
the three ±1 candidates and their differing-pixel counts; all seven minima are **0 pixels at
offset 0**, and reference 840's `.idx` compares byte for byte (the comparison helper compares
the indexed plane only at the offset the RGB search chose, which is this task's carried Task-3
minor). The unit tests above pin the pre-roll, the letterbox and `defpal`, the 96-byte upload,
the mode-4 band, the mode counts, the first spawn and the dot engine.

A reference `R` is port frame `R - 356`; `docs/fidelity/reference-indices.md` §24.7 carries the
capture. The substitutions — the start gate, the fixed tick and the dropped argument path — are
`docs/fidelity/APPROXIMATIONS.md` A46–A50, each with its measured difference.
