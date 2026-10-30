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

**Two of the seven are the tree's record only (2026-10-29).** The cube's camera path and sine
table are read from the release's image instead (§5, D49, A23): the shipped build's `buu` is not
`RATA.INC` and its sine table is not `SINIT.INC`. The part parses the other five; the tests parse
`RATA.INC` and `SINIT.INC` to compare the release's tables with them. `SPLINE.INC` is the
release's coefficients word for word (`the_release_reads_the_trees_spline_coefficients`).

`TILE.INC` and `TILEPAL.INC` sit in the tree and are **not** copied: the
shipped part's `PLZPART.PRJ` names neither, and no shipped source includes
them. `assets/PROVENANCE.md` carries the seven rows;
`docs/fidelity/asset-inventory.md` §F's "PLZPART has no media" row now states
the exception.

Parsed sizes and edges (test `the_inc_fragments_parse_to_their_original_sizes_and_edges`):
`PSINI` is 16,384 `db` bytes (first three 64, last four 62/63/63/63);
`LSINI4`/`LSINI16` are 8,192 `dw` words each (first two 512/513; last 510);
`SINIT` is 2,575 bytes; `PTAU` 129 values; `SPLINE`, 1,024 words; `RATA`,
1,088 words = 136 control points of 8 (36 written lines plus `REPT 100`). The release's `buu` has
the same 1,088 words and its sine table is 10,240 words (§5).

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
- **The camera spline is the release's** (`SPLINE.ASM`; the shipped image's `buu` and the
  tree's `SPLINE.INC`): `getspl(4*256 + frames*4)`. The position's high byte selects a control
  point, its low byte the coefficient fraction; the four coefficient groups multiply points
  `seg+3`, `seg+2`, `seg+1`, `seg` at byte offsets `di`, `di+512`, `di+1024`, `di+1536`, and the
  32-bit sum is shifted 15 (`shld cx,bx,1`). The eight outputs are `tx, ty, dis, kx, ky, kz,
  ls_kx, ls_ky`. The control points are read from the unpacked `MAIN/DATA/PLZPART.EXE` at image
  **`0x13894`**: `calculate` calls `getspl` as `call far 1389h:1084h` (`0x12c8a`) and `getspl`'s
  `mov si,0004h` (`0x1491c`) is `mov si, OFFSET buu`. Against `RATA.INC`, every rotation and light
  word is **8×** the tree's, the `REPT 100` tail is `0,0,500,0,0,0,2048,4096`, and `dz` is the
  release's own in rows 14–31 (325, 300, 300, 325, 350 ×11, 325, 300, 325 for a flat 300). The
  coefficients at `cs:0884h` (`0x14114`) are `SPLINE.INC`'s. The unit test pins the outputs at
  frames 0/136/1136/3000: `[0,432,499,3999,5599,3999,0,0]`, `[0,0,499,5676,3914,2314,79,0]`,
  `[0,0,349,5416,4181,2582,2047,0]` (the close-up's camera at `dis` 349, where the tree's put it
  at 299) and `[0,0,499,0,0,0,2047,4095]`
  (`getspl_interpolates_the_release_camera_spline_at_the_keyframes`).
- **The angles are 13 bits** (image `0x12c7d`, `calculate`): `kx, ky, kz, ls_kx, ls_ky` are masked
  with `and ax,1fffh`, where `VECT.C:138-142` has `&1023`, and read through the release's sine
  table at segment `28b1h` (image `0x28b10`): an **8,192-entry turn** with `kosinit` at
  `es:[bx+1000h]`, 2,048 words on (the tree's is 1,024 entries with `kosinit = sinit + 512`
  bytes). The 8× of the table is not shifted away; the release turns at the tree's rate with 8×
  the angular resolution (`the_release_sine_table_is_an_8192_entry_turn`). The light is
  `ls_y = kosinit[ls_kx]>>8`, `ls_x = (sinit[ls_kx]>>8)*(sinit[ls_ky]>>8)>>7`,
  `ls_z = (sinit[ls_kx]>>8)*(kosinit[ls_ky]>>8)>>7` (`VECT.C:144-146`, image `0x12cbd`).
- **The rotation is the release's** (image `0x12d40` `count_const`, `0x1307a` `rotate`).
  `count_const` is the tree's expressions with `>>15` for each `>>15+7` and `cxz = -SY`, a 1.15
  matrix, the `cyx`/`cyy`/`czx`/`czy` lines still subtracting inside the final shift
  (`count_const_is_the_releases_one_point_fifteen_matrix`). `rotate` sums
  `x*cxx + y*cxy + z*cxz` in 32 bits, shifts by 11 and adds `tx << 4`: each coordinate is a word in
  1/16 units, the point keeps `>> 4` of it for `sort_faces`, and the projection divides the 1/16
  words — `xxx = (xx16 << 8)/zz16 + 320` (the tree's `+160+160`, one `add ax,140h`) and
  `yyy = (yy16*142)/zz16 + 66`, the tree's own pair (`VECT.OBJ`: `imul eax,eax,0x8e` /
  `add ax,0x42`), centring the cube on row ~67 of the 134-row screen;
  `the_cube_projection_is_the_trees_142_66` pins it. (An earlier build used 213/100 for a
  "200-row screen"; §7.)
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

## 7. The cube at the keyframes

`cargo test --release -p secondreality --test plzpart -- --nocapture` prints:

| boot | tick | differing pixels | max channel delta |
|---|---|---|---|
| 8500 | 2777 | **0.000 %** (0) | 0 |
| 9500 | 3777 | **0.000 %** (0) | 0 |

`the_cube_keyframes_match_the_reference` pins both at 0 (`cargo xtask rediff --only plzpart/cube`
reproduces the counts). **Read from the image on 2026-10-29** (D49, A23): until then the cube
used the tree's `RATA.INC`, its 1,024-entry sine table and its 8.8 `count_const`/16-bit `rotate`,
and the port read `ls_y` from `kosinit[ls_ky]` — 17.577 % (11,249 px, max 247) at 8500 and
95.570 % (61,165 px, max 255) at 9500, registered as "the shade/pose residual" and "absent
release camera data". Neither was absent: both were the release's `buu`, angle unit and rotation,
which the image holds (§5), and the port's `ls_y` bug.

**The `ls_y` bug.** `VECT.C:144` and the release read `ls_y = kosinit[ls_kx]>>8`; the port read
`kosinit[ls_ky]`. In the close-up the light has `ls_kx` a quarter turn and `ls_ky` 0, so the port's
`ls_y` was 127 for 0, the light vector up to √2 long, and the face shade
`(ls·n)/250000 + 32` left `0..=64`. `shadepal` then scaled the face's 64 colours past the 6-bit
DAC, which wraps: hard bands of the wrong colour inside a face (green and black on a red/yellow
face, yellow and black on a blue one, navy on a green one). With only that line reverted in the
corrected build, the full-run close-up frames 24990/25620/26111 differ in 37,460/50,183/60,804
pixels against 108/188/0 (`out/glitches/plzcube-lsy-bug.png`, scratch);
`the_light_keeps_every_shade_inside_its_band` asserts every visible face's shade is in `0..=64`
over frames 0–2199 and fails at frames 369 (shade 66) with the bug.

**The screen (corrected 2026-10-26, unchanged).** `TWEAK.ASM:31-33` sets CRTC 0x09 = 2 (three
scanlines a row, "134 y"), `PLZA.ASM:77` clips at 134 rows, and the buffers are 136 rows apart;
the 200-line reference frames show row ⌊2y/3⌋ of that image. An earlier build fitted a 200-row
draw at 213/100 (= 142/66 × 1.5), which overflowed the 136-row slot (the "page-slot spill"). The
shipped release does the same as the tree: `tools/crtc_probe.py 3 8500 30` read CRTC 0x09 = 2,
0x12 = 143 (400 lines), 0x13 = 80 and display starts 0x0028/0x5528/0xAA28 in the great majority
of 30 frames (A23 has the counts).

**The phase.** The music gate is the trace's: with the entry pre-roll of **8 polls** the
`musplus() >= 13` gate passes at manifest 2641 = boot 8364, the frame the published order-68 row
13 first stands on (`the_music_pre_roll_lands_the_cube_gate_on_the_reference_tick` asserts
2641), and the first draw is boot 8365. At the 8500 keyframe the part's state is therefore
**frames 136, page 4, display start 0x0028** — the release's own; the keyframe test asserts the
manifest's music row 68:46 there. (A nine-poll pre-roll put the gate one frame early.)

**The band corpus.** The six band frames, taken by the sanctioned path:

```bash
cargo xtask reference capture --arg 3 --boot autoexec \
    --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames 8400,8450,8500,8600,8700,8800 --out reference/parts/plzpart/band
```

(RGB only: the indexed read refused because the screen moved while it was read, as it does for
the cube keyframes.) Whole-frame differing pixels of the port against it, `cargo xtask rediff
--only plzpart/band` (tick = index − 5723):

| | 8400 | 8450 | 8500 | 8600 | 8700 | 8800 | total |
|---|---|---|---|---|---|---|---|
| now | 340 | 24 | **0** | **0** | 148 | 4 | 516 of 384,000 |
| the tree's camera (2026-10-26) | 6,531 | 6,092 | 11,249 | 21,722 | 11,190 | 33,457 | 90,241 |
| before the 134-row correction | 20,315 | 21,832 | 16,743 | 32,287 | 14,979 | 37,374 | 143,530 |

The 516 are A23's registered residue: all both-set, at a channel delta of at most 9, in row runs
on one face, and inside those reference frames one colour index shows two colours (24 of the
port's indices at 8400, 18 at 8700) — the palette changed while the frame was read. `vect()` sets
`cop_pal=fpal; do_pal=1` before `calculate` rewrites `fpal` (`VECT.C:123-125`), so which shade the
DAC holds depends on the original's CPU time.

**The full run's close-up.** The dense comparison of the whole demo (`out/dense-orig`, one frame
every 35) pairs with the port's `--block 1 --headless --dump-frames` at **+38** — the dense
capture's own offset from the port's numbering, the same +35 to +40 its plasma and MINVBALL
frames show (the committed `fullrun` capture pairs PLZPART at +1). Over the close-up, 24990–26215
(36 frames), the port differs in **3,614** pixels in all (15 frames at 0, the largest 1,716); the
tree's camera gave **2,120,856**. Every cube frame 24570–26530 is within 1,716. The contact sheets
are `out/glitches/plzcube-sheet-{before,after}.png` (scratch).

**What was found and fixed on the way** (earlier work, still in force):

- **`RATA.INC`'s `REPT 100`.** The spline has 136 control points (36 written lines plus 100
  repeated), not 37 — the tree's and the release's alike. A run past frame 1920 (segment 34, the
  first whose `getspl` fraction reads past the 37 explicit control points) reads the tail;
  `getspl_interpolates_the_release_camera_spline_at_the_keyframes` pins it at frames 3000.
- **The `dist1` warp scale.** `dseg = disttaus[color] + dd*16` adds 16
  *paragraphs* (256 bytes), not 16 bytes: the texture sample is
  `dist1[dd*256 + bx]`, and `the_texture_warp_shifts_dist1_by_dd_paragraphs`
  fails if it reverts.
- **The entry pre-roll** (8, above) and **the 134-row screen** (above), each with a
  test that fails on reversion.
- The ctau union hull, `do_clear`'s page base, the gate-frame `swappage`, and
  the span divisor/edx sign mask (the last four described in the code and
  A23).

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
at 68:46, the cube's keyframes are byte-identical (§7) and the part's remaining differences are
the registered ones — A24's plasma tear and A23's 516-pixel palette-timing residue on four band
frames.

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
- The double `+160` in `xxx = (xx*256)/zz + 160 + 160` (the release's one `add ax,140h`).
- The subtraction inside `count_const`'s final shift, in the tree and the release alike. The
  tree's `>>15+7` (= `>>22`) is **not** the release's: its `count_const` shifts by 15 (§5), so the
  port no longer keeps it (`bugs.md` §17.4).
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
- **The cube is the release's (§5, §7, A23).** Its screen is the tree's 134-row screen at
  142/66 (the release's CRTC 0x09 = 2, `tools/crtc_probe.py`, and `TWEAK.ASM:31-33` agree); its
  camera path, sine table, angle mask, `count_const` and `rotate` are read or translated from the
  shipped image (D49). Both keyframes are byte-identical; four band frames keep A23's 516-pixel
  palette-timing residue, registered.
- **The exit index is read from the image too (A22).** The release's `plz()` compares `curpal`
  (DGROUP `0192h`) with 4 at image `0x1181c`, the value the frames measured;
  `the_release_exit_index_is_the_images` pins it.
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

The 33 unit tests cover the data, the map, the plasma expression, the
timetable, the release exit (measured, and read from the image), `dtau`/the split walk, the
split/pan parity, the release's camera table, coefficients and sine table read from the image
(and an image of another layout refused), the spline (including the `REPT` tail), the light's
shade band, the 134-row screen, the 142/66 projection and the ⌊2y/3⌋ line mapping, a draw
staying inside its 0x5500-byte slot, the release's `count_const`, the mask
underflow, the beam wrap, the clear, the texture warp, the synthetic
rasteriser cases, the wait walks and the music gate. The three integration
tests cover the registry and the two screens, the music row (including the
manifest's 68:46 at boot 8500), the three plasma
keyframes (bounded), and the two cube keyframes (byte-identical). The
five keyframe RGB files are `reference/parts/plzpart/{plasma,cube}/`; the six
band frames are `reference/parts/plzpart/band/`;
`cargo xtask reference verify --tree reference` checks the tree (36 captures,
122 files, 28,115,022 bytes at this record).

**Register.** The behaviours of this part that the original does and the port must keep are in `bugs.md` §1 (the palette-leak banding) and §17 (the mask underflow, the cut-short fade, the double `+160`, the shift precedence).
