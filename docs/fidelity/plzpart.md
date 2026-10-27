# PLZPART — the plasma and the cube

`PLZPART/MAIN.C` runs `init_copper(); initvect(); plz(); vect(); close_copper()`:
**one part, two screens.** `plz()` (`PLZ.C:62-122`) is the 320×400 plasma,
`vect()` (`VECT.C:110-130`) the textured cube (134 rows, shown on a 320×200 frame); `PLZFILL.C` is the cube's
scene, `PLZA.ASM` its span rasteriser and clear, `COPPER.ASM` the three copper
routines, `ASMYT.ASM` the plasma's index expression, `SPLINE.ASM` the camera
spline and `TWEAK.ASM` the two mode sets.

The port is `bins/secondreality/src/parts/plzpart.rs`, registered as
`"plzpart"`. A `--part plzpart` run seeks `MUSIC1` to order 62
(`shell::music_for_part`), the order the block-3 trace holds when the part's
start window opens.

## 1. The data: seven `.INC` fragments, copied verbatim

The part has no media file. Its tables are the original's `db`/`dw` text
fragments, copied byte for byte to `assets/PLZPART/` and parsed by a part-local
reader in their own format — no conversion. The copies are pinned by
`sha256sum assets/PLZPART/*.INC` and `wc -c assets/PLZPART/*.INC`:

| fragment | bytes | sha256 |
|---|---|---|
| `PSINI.INC` | 84,992 | `05fd03cd4f63fae47ef00414742de0d67936ac69294c10e04053b4cacfa15138` |
| `LSINI4.INC` | 42,496 | `e16892a5bb66ede2484e67e3f66724e92ba1618ee7f7733e01135ca966c2a554` |
| `LSINI16.INC` | 42,496 | `fca96d96edf4a75035672cd17bf7d773ccdc109768e452e0c16ee2de56be04f3` |
| `SINIT.INC` | 10,616 | `eb60f31c61c2f3c5688bec2ac1ba26eebccfe39027c60afa0d56b2a0a4964f8e` |
| `PTAU.INC` | 525 | `01cc922f6686999e5586bb9ebb2ed428294edb3650a1813c9be435a09a6b96f5` |
| `SPLINE.INC` | 9,493 | `a478022fde30c4a0cbb0572b7b4c28dbdf118786b41f7e0b2f8a6a5e1dea0e06` |
| `RATA.INC` | 1,926 | `cf9ed8269618657d0a40dcdd5dcaef8e574096f0f46cd9803a6da78e8295188c` |

`TILE.INC` and `TILEPAL.INC` sit in the tree and are **not** copied: the
shipped part's `PLZPART.PRJ` names neither, and no shipped source includes
them. `assets/PROVENANCE.md` carries the seven rows;
`docs/fidelity/asset-inventory.md` §F's "PLZPART has no media" row now states
the exception.

Parsed sizes and edges (test `the_inc_fragments_parse_to_their_original_sizes_and_edges`):
`PSINI` is 16,384 `db` bytes (first three 64, last four 62/63/63/63);
`LSINI4`/`LSINI16` are 8,192 `dw` words each (first two 512/513; last 510);
`SINIT` is 2,575 bytes; `PTAU` 129 values; `SPLINE`, 1,024 words; `RATA`,
1,088 words = 136 control points of 8 (36 written lines plus `REPT 100`).

**The map, not the assembler's placeholders, fixes the table bases.**
`ASMYT.ASM:46-61`'s `0xC100`/`0xC200`/`0xC300`/`0xC400` sit inside `plzline`'s
own instructions; `setplzparas` (`ASMYT.ASM:74-95`) patches each one to a
table address. `PLZPART.MAP:368-369,387,391` gives `_psini 027D:0000`,
`_lsini4 027D:4000`, `_lsini16 027D:8000`, `_plzline 027D:C000`
(test `the_table_bases_are_the_maps_not_the_assembly_placeholders`). The port
builds the flat 0xC000-byte segment the map describes and evaluates the patched
index expression in it. The plasma's byte `r` of line `y` is
(test `the_plasma_line_is_the_assemblers_index_expression`):

```text
b  = lsini16[c2 + 320 - 4r + y]
px = psini[c1 + 8r + b]
d  = lsini4[c4 + 16r + y]
py = psini[c3 + 320 - 4r + d + 2y]
value = (px + py) as u8
```

with the lc2 read allowed to point below `lsini16` into `lsini4`'s tail and
every address taken 16 bits wide, as the original's segmented address
arithmetic does. Each row writes 84 bytes for an 80-byte visible line, so the
last 4 land in the 96-byte stride's gap (`ASMYT.ASM:41-69`); the overrun is
kept and invisible.

## 2. The two screens and the tick map

| | plasma | cube |
|---|---|---|
| mode set | `tw_opengraph2` (`TWEAK.ASM:44-67`) | `tw_opengraph` (`TWEAK.ASM:18-37`) |
| screen | 320×400 | 320×200 |
| plane row | 96 bytes (CRTC 0x13 = `0x3013`) | 160 bytes (0x13 = `0x5013`) |
| rows register | 0x09 = 0 (400) | 0x09 = 2 (200) |
| start | `cop_start = 96*(682-400) = 27,072` plane bytes | six pages, `PAGE_VMEM` 0/0x5500/0xAA00 |
| split | line compare 60/61, register+1 → scanline 61/62 | none (line compare 500 at the hand-off) |

The split/pan walk (`COPPER.ASM:126-143` `pompota`, test
`the_split_and_pan_alternate_at_the_reference_parity`): every retrace the
register alternates 60/61 and the attribute pan 4/0. `Split::scanline` is the
register **plus one**, phase 1's measurement (`reference-indices.md` §13.2);
`View.pan` is in pixels and a 256-colour mode shifts `(value & 7) / 2`, so the
pan is 2/0. `pompi` starts at 1: the source's BSS 0 plus the retrace that lands
before the first composed frame. That is what makes the measured sequence line
up with the reference rather than one tick off.

**The mapping is `boot = 5723 + tick`, and it is a mapping, not an anchor.**
The part's first graphics frame is a measured window, `(5720, 5730]`
(`reference-indices.md` §13), so a run numbers its own frames from zero and
its manifest says `index_origin: "part"`, `anchor: null`; `boot_anchor` is not
added. Tick 0 is boot 5723: `_np_zframe` reads zero there and the part's
`setmframe(0)` (`PLZ.C:69`) lands there (`docs/fidelity/music-clock.md` §2).
The keyframes are therefore ticks 277/1277/1877 (boot
6000/7000/7600) and 2777/3777 (boot 8500/9500).

The part's `self.tick` is 1-based — the first `frame` call is tick 1 — while
the dump manifest numbers the same frame from 0 (`run.index() - 1`). The
keyframe ticks above are manifest indices; the music test states its tick in
the part's own 1-based numbering and subtracts one.

## 3. The plasma

`plz()` (`PLZ.C:62-122`): wait for `musplus() >= 0`, `setmframe(0)`,
`init_plz` (`:124-218`), then the frame loop. `init_plz` sets the mode, the
starting display parameters, the palette tables (`pals[6][768]`, five of
them used, built from `PTAU` and the `(v-63)*2` / `*8` scales), and starts
the white screen. The loop:

- reads `getmframe()`; when it passes `timetable[ttptr]` the current colour
  half is `memset` to zero, `cop_drop` starts, the next table is selected and
  `ttptr` advances. `timetable = {723, 1491, 1875, 2259, 2778, 0}`
  (`PLZ.C:46`; test `the_timetable_events_happen_at_the_getmframe_thresholds`).
- draws the four row passes (planes 1/3 on even K, 1/3 on odd L, 0/2 on odd K,
  0/2 on even L), 280 rows of 84 bytes each, into the four-plane surface.
- exits when `curpal == 4` (the release's index, see §10) and `cop_drop > 64`.

The fade (`COPPER.ASM:184, 238-252`) is a 768-entry, 16-bit accumulator table:
the high byte is the colour and the low byte the fraction. `do_drop` adds the
next table's word to the accumulator with `add [di+ccc+768],al / adc
[di+ccc],ah`, so the low half keeps the fraction across the `memset` that
clears only the high half. That is the palette-leak banding the port keeps.

The 60/61 split moves down the screen at the exit (`do_drop`'s `dtau` walk,
`COP_PAL`), the last step resets the line compare, and the part then switches
to the cube.

## 4. The plasma at the keyframes

`cargo test -p secondreality --test plzpart -- --nocapture` prints the three
plasma verdicts:

| boot | tick | differing pixels | max channel delta |
|---|---|---|---|
| 6000 | 277 | **11.209 %** | 16 |
| 7000 | 1277 | **5.848 %** | 21 |
| 7600 | 1877 | **4.052 %** | 17 |

The reference's plasma is a **tear**, and the mechanism is measured. `plz()`'s
loop has no frame wait (`PLZ.C:74-122`); it runs unthrottled, and the copper's
once-per-vblank `moveplz`/`pompota` can land between the loop's four plane
passes. `setplzparas` latches `k`/`l` at each pass's start, so the screen is
torn at whichever pass boundary the retrace lands. Forcing the port's walk to
land before each pass (a temporary diagnostic) gives this residual table:

| split before pass | 0 | 1 | 2 | 3 | boundary (the port) |
|---|---|---|---|---|---|
| 6000 | **4.15 %** | 5.23 | **4.38** | 8.42 | 11.21 |
| 7000 | 19.48 | 15.68 | 12.09 | 8.59 | **5.85** |
| 7600 | 26.19 | 19.47 | 14.12 | 8.28 | **4.05** |

The 6000 frame's tear sits before pass 0 or 2; 7000 and 7600 sit at the
boundary. Which boundary the retrace lands on depends on the original's
per-frame execution time (the fade/drop work changes it), i.e. on the machine,
not the algorithm; the port's fixed 70.086 Hz tick latches once per tick, the
loop's phase-locked steady state. No fixed split matches all three (split 3
gives 8.42/8.59/8.28), so the port keeps the boundary and registers the
timing tear as A24. Every colour delta is within a sixteenth of the palette
(16/21/17 of 255), and the timetable/palette events match, so the residual is
the tear, not palette.

## 5. The cube

`vect()` (`VECT.C:110-130`): `tw_opengraph()` (chain-4 off, CRTC 0x09 = 2 so
each row is three scanlines — a **134-row** screen, `TWEAK.ASM:31-33` "134 y" —
and a 160-byte plane row), wait for `musplus() >= 13`, then per frame `swappage`,
the frame wait (`while(frame_count<1)`), `frames += frame_count`,
`calculate(1)`, `draw(15)`, `clear()`.

- **The six-page flip** (`VECT.C:265-298`): `page = (page+1)%6`; the three
  buffers at 0x0000/0x5500/0xAA00 are drawn in turn, and `cop_start` points the
  display at the buffer drawn the *previous* iteration (`0xAA28`, `0x0028`,
  `0x5528`, …). `PAGE_PAN` alternates 4/0, i.e. 2/0 pixels.
- **The camera spline** (`SPLINE.ASM`, `RATA.INC`, `SPLINE.INC`):
  `getspl(4*256 + frames*4)`. The position's high byte selects a control
  point, its low byte the coefficient fraction; the four coefficient groups
  multiply points `seg+3`, `seg+2`, `seg+1`, `seg` at byte offsets
  `di`, `di+512`, `di+1024`, `di+1536`, and the 32-bit sum is shifted 15
  (`shld cx,bx,1`). The eight outputs are `tx, ty, dis, kx, ky, kz, ls_kx,
  ls_ky`. The unit test pins the values at frames 0/136/1136:
  `[0,432,499,499,699,499,0,0]`, `[0,0,499,709,489,289,9,0]`,
  `[0,0,299,677,522,322,255,0]`
  (`getspl_interpolates_the_camera_spline_at_the_keyframes`).
- **The rotation** (`VECT.C:166-199` `count_const`/`rotate`): the angles are
  masked to 1023 and read through `sinit`/`kosinit`; `kosinit = sinit + 512`
  is 512 **bytes** = 256 words — the map's `_kosinit 2CE4:0200` confirms it.
  The matrix keeps the C's precedence (`>>15+7` is `>>22`, and `cyx`/`cyy`/
  `czx`/`czy` subtract inside the final shift);
  `count_const_keeps_the_c_precedence_shifts` pins both. The projection is
  `xxx = (xx*256)/zz + 160 + 160` and `yyy = (yy*142)/zz + 66` — the tree's
  own pair (`VECT.OBJ`: `imul eax,eax,0x8e` / `add ax,0x42`), centring the cube
  on row ~67 of the 134-row screen; `the_cube_projection_is_the_trees_142_66`
  pins it. (An earlier build used 213/100 for a "200-row screen"; §7.)
- **The cube is a 134-row screen.** `TWEAK.ASM:31-33` sets CRTC 0x09 = 2: every
  row is shown on three scanlines of the 400-line display (400/3 = 133.3), the
  source's own "134 y". The tree's `do_block` clips at `cmp yy,134`
  (`PLZA.ASM:77`) and the draw buffers, 0x5500 bytes apart, are 136 rows of 160
  bytes. `CUBE_DRAWN_ROWS` is 134 and the clear walks `CLEAR_LINES` 135.
  `the_cube_is_a_134_row_screen` pins them and `a_134_row_draw_stays_inside_its_slot`
  checks that no write leaves its 0x5500-byte slot. The 320×200 frame is every
  other scanline, so output line y shows image row ⌊2y/3⌋
  (`the_cube_frame_shows_row_two_y_over_three`).
- **The rasteriser** (`PLZA.ASM:59-259`): the face's four projected points are
  split into two edges by the topmost vertex; per scanline `do_block` fills
  the span, writes the end bytes with the periodic start/end masks and the
  middle bytes through the 16.16 texture interpolator. The texture is the
  fixed rectangle `(64,4)-(190,60)` of `kuva1/2/3` (`PLZFILL.C:88`), warped by
  `dist1[(frames&63)+1]`; the per-sample address update is `add ecx,eax` /
  `adc ebx,edx` / `adc bh,0`, and the span divisor is **bytes−1**, the asm's
  `dec si`, not the span length.
- **The clear** (`PLZFILL.C:178-185`, `PLZA.ASM:291-372`): each row's ctau
  entry is the union hull `[smallest span start, largest span end]`
  (`PLZA.ASM:73-90`), and `do_clear` trims the old hull against the new one
  three draws back — the same buffer in the three-buffer flip. The row base is
  the drawn page's offset (`les dx,[vmem]`) with the fill-latch byte
  overwriting its low byte; the mask loop starts at the same page offset.
  `the_clear_erases_a_moved_quad` pins the erase.
- **The beam** is composed by the part: the four planes are 262,144 linear
  bytes and the engine's surface is 6,144 pixels short, so the cube keeps its
  own 262,144-byte video and copies the visible 320×200 window (row ⌊2y/3⌋ for
  line y) into the surface (`the_cube_beam_wraps_at_sixty_four_kilobytes_of_plane_bytes`).

## 6. The copper mapping

`init_copper` (`COPPER.ASM:44-65`) registers `copper1` on slot 1, `copper2` on
slot 2 and the empty `copper3` on slot 0. The DIS's slots are `0 = after
display start`, `1 = just before retrace`, `2 = in the retrace`
(`DIS/DISINT.ASM:284-291`), and the engine runs `0 → bands → 1 → 2`. The
mapping is the **identity**, stated by what each routine does:

```text
engine slot 0  <- copper3  empty
engine slot 1  <- copper1  the CRTC start/pan writer (CRTC 0x0d/0x0c, AC 0x33)
engine slot 2  <- copper2  frame_count++, the 768-byte palette upload,
                           pompota + moveplz (with cop_plz), do_drop (with cop_drop)
```

One tick is one copper frame. The engine composes a tick's bands before slot 2
runs, so slot 2's state is seen from the next tick — the original's retrace
boundary. The unit test `the_split_and_pan_alternate_at_the_reference_parity`
exercises slots 1 and 2 through the engine's order, and
`the_cube_beam_wraps_at_sixty_four_kilobytes_of_plane_bytes` and
`the_face_palette_is_shaded_at_its_own_block` cover the slot-2 palette path.

## 7. The cube at the keyframes — D24 measurements

`cargo test --release -p secondreality --test plzpart -- --nocapture` prints:

| boot | tick | differing pixels | max channel delta |
|---|---|---|---|
| 8500 | 2777 | **17.577 %** (11,249) | 247 |
| 9500 | 3777 | **95.570 %** (61,165) | 255 |

The tests pin the percentages to 0.001 points and the max deltas exactly: the
run is deterministic (`cargo xtask rediff --only plzpart/cube` reproduces the
counts). **"Verified" for this part means a faithful port with the divergences
below registered, not a matching cube**: the plasma's CPU-timing tear (A24),
the 8500 residual's shade/pose (17.577 %, attributed below), and the 9500
close-up's absent release camera data (95.570 %) are the recorded differences.

**Corrected 2026-10-26: the cube is a 134-row screen.** This section used to
record a 200-row extent, a 213/100 projection and a port-side "page-slot
spill" (26.161 % at 8500, 99.483 % at 9500). Those were one mis-modelled
display. `TWEAK.ASM:31-33` sets CRTC 0x09 = 2 (three scanlines a row, "134 y"),
`PLZA.ASM:77` clips at 134 rows, and the buffers are 136 rows apart; the
200-line reference frames show row ⌊2y/3⌋ of that image, which made the cube
look 1.5x taller (213/100 = 142/66 × 1.5) and "drawn to row 199". A 200-row
draw overflows the 136-row slot into its neighbour — that was the spill. The
shipped release does the same as the tree: `tools/crtc_probe.py 3 8500 30`
read CRTC 0x09 = 2, 0x12 = 143 (400 lines), 0x13 = 80 and display starts
0x0028/0x5528/0xAA28 in the great majority of 30 frames (A23 has the counts).
The port now draws 134 rows at 142/66 and shows row ⌊2y/3⌋.

**The phase.** The music gate is the trace's: with the entry pre-roll of
**8 polls** the `musplus() >= 13` gate passes at manifest 2641 = boot 8364,
the frame the published order-68 row 13 first stands on
(`the_music_pre_roll_lands_the_cube_gate_on_the_reference_tick` asserts
2641), and the first draw is boot 8365. At the 8500 keyframe the part's state
is therefore **frames 136, page 4, display start 0x0028** — the release's
own. (The earlier nine-poll pre-roll put the gate one frame early; the
8500 residual was 32.106 % at the wrong phase, in an earlier build. The phase
state itself — frames 136, page 4, start 0x0028, music 68:46 — is what the
8500 keyframe test's `music_position` assertion and the tick numbering pin.)

**The extent, the projection and the spill: see the correction above.** The
rasteriser clips at 134 rows (`PLZA.ASM:77`), the projection is the tree's
142/66 (`VECT.OBJ`, `SPLINE.OBJ`/`INCLUDE.OBJ` carry the RATA/SPLINE/sinit data
the port parses, compared word for word), and a draw stays inside its slot.
The earlier fit of 213/100 over six committed band frames (0.46 % against 9.99 %
for 142/66, over a 134-row band) compared two different mappings of one image
and is retired with its test-only probe. The evidence that remains is the
committed fit corpus, taken by the sanctioned path:

```bash
cargo xtask reference capture --arg 3 --boot autoexec \
    --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames 8400,8450,8500,8600,8700,8800 --out reference/parts/plzpart/band
```

(RGB only: the indexed read refused because the screen moved while it was
read, as it does for the cube keyframes.) Whole-frame differing pixels of the
port against it, `cargo xtask rediff --only plzpart/band` (tick = index −
5723):

| 8400 | 8450 | 8500 | 8600 | 8700 | 8800 | total |
|---|---|---|---|---|---|---|
| 6,531 | 6,092 | 11,249 | 21,722 | 11,190 | 33,457 | 90,241 of 384,000 (23.500 %) |

(before the correction: 20,315 / 21,832 / 16,743 / 32,287 / 14,979 / 37,374,
143,530, 37.378 %). The cube silhouettes align with the references at offset
(0, 0); what remains is inside the faces.

**The 8500 residual's attribution.** The 17.577 % is 11,249 pixels of 64,000:
304 are port-only (set in the port, unset in the reference: 0.48 points; was
6,333, the spill), 51 reference-only (0.08 points; was 105), and 10,894
both-set and differing (17.02 points; was 10,305). 47 delta-247 pixels remain
in the frame, all port-only (was 3,137); the both-set differences are the
shading/pose family at mostly small deltas — 9,406 of the 10,894 are ≤ 20,
with 4/8/12 the most common (5,699/1,755/807) and none at 247. That both-set
part is A23's registered shade/pose residual, not explained.
`the_cube_keyframes_are_measured_and_recorded` computes and pins this
decomposition.

**The close-ups.** The 9100–9600 frames need frame-dependent camera data the
tree does not carry: the RATA control points those frames interpolate
(15–26; boot 9100 is animation frame 736 and segment 15, boot 9600 frame 1236
and segment 23, and the spline at frame 1136 is segment 21) are all
`dx = dy = 0, dz = 300`, constant across those segments — the tree carries no
frame-dependent translation. That is **absent release camera data**, not a
translation the port can apply.
A global alignment (best shift = minimum mask-XOR fraction over overlaps of at
least 10,000 pixels, shifts within ±100, re-measured at the corrected geometry:
the port by (+53, +9)) confirms that translation cannot explain the frame: over
the 267×191 overlap the silhouette masks XOR in 3,812 of 50,997 pixels (7.47 %;
IoU 0.923), and 99.66 % of the 45,738 common pixels still differ (max delta 255,
mode 61, 8,062 of the common pixels). The 9500 keyframe's 95.570 % is that
registered data divergence (A23, registered), not an unexplained port error.
`the_cube_keyframes_are_measured_and_recorded` pins the aligned counts;
`the_close_up_segments_carry_no_frame_dependent_translation` derives the
control-point values from the committed `RATA.INC`.

**What was found and fixed on the way:**

- **`RATA.INC`'s `REPT 100`.** The tree's spline has 136 control points
  (36 written lines plus 100 repeated), not 37. The parser expands
  `REPT`/`ENDM` (1,088 words); the fix does not change the 8500/9500
  residuals (frames 136/1136 use segments 6/21, inside the 37 explicit
  points), but a run past frame 1920 — segment 34 is the first whose `getspl`
  fraction reads past the 37 explicit control points — would have indexed
  past the table.
  `getspl_interpolates_the_camera_spline_at_the_keyframes` pins the repeated
  tail (`[0,0,499,0,0,0,255,511]` at frames 3000/6000).
- **The `dist1` warp scale.** `dseg = disttaus[color] + dd*16` adds 16
  *paragraphs* (256 bytes), not 16 bytes: the texture sample is
  `dist1[dd*256 + bx]`, and `the_texture_warp_shifts_dist1_by_dd_paragraphs`
  fails if it reverts.
- **The entry pre-roll** (8, above) and **the 134-row screen** (above), each with a
  test that fails on reversion.
- The ctau union hull, `do_clear`'s page base, the gate-frame `swappage`, and
  the span divisor/edx sign mask (the last four described in the code and
  A23).

Ruled out by measurement: the page/display parity (the corrected phase puts
the keyframe on the release's own page 4/start 0x0028), the spline phase
(offset probes did not lower the residual at both keyframes), the texture
rect (transpose/mirror/rotate variants), and the tree's own constants and
data (verified against `VECT.OBJ`/`SPLINE.OBJ`/`INCLUDE.OBJ`). The
diagnostic captures are under `out/`; the projection fit's reference is the
committed `reference/parts/plzpart/band/`, and `reference/` stays
capture-written only.

## 8. The music

- **The seek.** `shell::music_for_part("plzpart") = (Music1, 62)`: order 62 is
  where `reference/music/second3a/trace.csv` holds the sequence when the
  part's start window opens.
- **The entry pre-roll is 8 polls**, guarded on `ctx.music_fresh()`. The
  player's published words lag the reference's when the part starts; with
  eight pre-polls the cube gate (`musplus() >= 13`) passes at the reference's
  boot 8364. The test measures it with the real player, not a constant: eight
  pre-polls, the tick-0 engine poll, then the first poll whose `musplus`
  reaches 13 at **manifest 2641** = boot 8364 (the dump's 0-based numbering;
  the part's 1-based tick is 2642). The corrected-phase manifest then reads
  the trace's published position at boot 8500, **68:46**, matching
  `reference/music/second3a/trace.csv`; the nine-poll phase read 68:47. The
  dump manifest's row is asserted by
  `the_cube_keyframes_are_measured_and_recorded`, and the fit test asserts
  the same position from the engine's player in-process. A
  mid-chain start has already run through those polls and takes none.
- **The plasma's gate** (`PLZ.C:68`) is the same position: the seek has
  already put an entry run where the reference's `musplus >= 0` was waiting,
  and the `254` at MUSIC1 order 61 that gave the reference `zplus == 2` is
  behind the seek. A fresh run's gate is tick 0; a mid-chain run uses the
  source's `musplus() >= 0`.
- **The cube's exit** is `-4 <= musplus < 0` (`VECT.C:119-120`), long after
  the two keyframes.

**What "verified" means for this part.** With the music row matching the trace
at 68:46, the part's remaining differences are the registered ones — A24's
plasma tear, the cube's 17.577 % residual (17.02 points both-set shade/pose,
0.48 points port-only, 0.08 reference-only; the page-slot spill is gone with
the 134-row correction, §7) and the 95.570 % close-up's absent camera data. "Verified" means a faithful port with those divergences
registered, not a matching cube.

## 9. The preserved bugs

- The 84-byte `plzline` overrun into the 96-byte stride gap.
- `do_clear`'s zero-count mask underflow: `end_mask[x-1]` at `x = 0` reads the
  byte before the table, `start_mask`'s last `0x08` (its final `1000b`)
  (`do_clears_mask_underflow_reads_the_byte_before_end_mask`).
- The palette-leak banding: `memset(fadepal,0,768)` of 1,536 clears the colour
  half and leaves the fractions.
- The cut-short last fade: `do_drop`'s last step resets the line compare and
  accumulates nothing.
- The dead `ttptr` row: `timetable[4] = 2778` can never be reached — the exit
  fires 65 ticks after the previous switch.
- The 404 split: `do_drop`'s `dtau` reaches 404, so the split walks off the
  bottom of the 400-line screen; the port writes 405 as `Split::scanline`,
  which is inert.
- The double `+160` in `xxx = (xx*256)/zz + 160 + 160`.
- The C precedence in `count_const` (`>>15+7` = `>>22`; the subtraction inside
  the final shift).
- The stale `PLZPART/MAIN.ASM` is not part of the port; `MAIN.C` is the
  entry point.

## 10. Divergences (D24)

- **The exit palette index: the release's 4, not the tree's 5.** The tree's
  `curpal == 5` (`PLZ.C:94`, `PLZ.OBJ`'s `cmpw $5`) would fire four switches
  in, at boot 8769 — past the reference's plasma screen, which is gone by boot
  8340. The release's plasma-to-cube hand-off is at boot ~8315 (the measured
  exit at tick 2592; `the_release_exit_index_is_the_measured_one` asserts the
  switches at ticks 975/2010/2527 and the exit at 2592 = boot 8315, inside the
  capture's `(8310, 8340]` window). The port adopts 4, D24's single-value
  rule; `APPROXIMATIONS.md` A22 records the measurement.
- **The cube is the tree's 134-row screen at 142/66 (§5, §7, A23).** The
  release's CRTC 0x09 = 2 (`tools/crtc_probe.py`) and the tree's `TWEAK.ASM:31-33`
  agree, so there is no divergence of extent or projection: an earlier build
  adopted 200 rows and 213/100 from the 200-line captures (corrected
  2026-10-26, §7). The remaining 8500 residual — 17.577 %, of which 0.48
  points is port-only and 17.02 points both-set shade/pose — and the
  close-up's camera data, which the tree does not carry (the 9500 keyframe is
  registered at 95.570 %), are recorded, not explained away. "Verified" for
  this part means a faithful port with these differences registered.
- **The plasma's tear timing (§4, A24).** The reference frame's per-pass
  parameter latch depends on the original's execution time; the port's fixed
  tick draws the loop's phase-locked steady state. The split-position
  measurements are in §4.

**One model omission, recorded here rather than as a D24 divergence.** The source polls
`dis_exit()` at every loop boundary — `PLZ.C:68` and `:76` for the plasma, `VECT.C:116` and
`:117` for the cube — and the port's `frame` consults neither. The omission is unobservable in
this phase: the engine's exit flag is sticky and the shell retires any part at the end of the
tick the flag was set in (`Sequence::tick`), so ESC ends the part on the same tick with or
without the poll. It would matter only for a part that could abandon work inside a tick; this
one's `frame` does a phase's bounded work and returns. Registered as `APPROXIMATIONS.md` A62.

## 11. The tests and the run

```bash
cargo test -p secondreality --bin secondreality parts::plzpart
cargo test -p secondreality --test plzpart -- --nocapture
```

The 28 unit tests cover the data, the map, the plasma expression, the
timetable, the release exit, `dtau`/the split walk, the split/pan parity, the
spline (including the `REPT` tail), the close-up segments' control points,
the 134-row screen, the 142/66 projection and the ⌊2y/3⌋ line mapping, a draw
staying inside its 0x5500-byte slot, `count_const`, the mask
underflow, the beam wrap, the clear, the texture warp, the synthetic
rasteriser cases, the wait walks and the music gate. The three integration
tests cover the registry and the two screens, the music row (including the
manifest's 68:46 at boot 8500), the three plasma
keyframes (bounded), and the two cube keyframes (measured and pinned, with the
8500 decomposition and the 9500 aligned measurement pinned beside them). The
five keyframe RGB files are `reference/parts/plzpart/{plasma,cube}/`; the six
band frames are `reference/parts/plzpart/band/`;
`cargo xtask reference verify --tree reference` checks the tree (36 captures,
122 files, 28,115,022 bytes at this record).

**Register.** The behaviours of this part that the original does and the port must keep are in `bugs.md` §1 (the palette-leak banding) and §17 (the mask underflow, the cut-short fade, the double `+160`, the shift precedence).
