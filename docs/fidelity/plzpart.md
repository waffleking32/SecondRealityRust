# PLZPART — the plasma and the cube

Sources: `PLZPART/MAIN.C` runs `init_copper(); initvect(); plz(); vect(); close_copper()`: the
320×400 plasma (`PLZ.C:62-122`) and the textured cube (`VECT.C:110-130`, a 134-row screen shown
on a 320×200 frame), over `PLZFILL.C`, `PLZA.ASM`, `COPPER.ASM`, `ASMYT.ASM`, `SPLINE.ASM` and
`TWEAK.ASM`. Assets: seven `.INC` fragments in `assets/PLZPART/` (§1) and the unpacked
`assets/MAIN/DATA/PLZPART.EXE` image (§5, D49). Port: `bins/secondreality/src/parts/plzpart/`
(`"plzpart"`); `mod.rs`'s doc has the map and the preserved bugs, and `tables.rs`, `release.rs`,
`copper.rs`, `plasma.rs`, `cube.rs` and `raster.rs` have the data, the copper mapping, the two
screens and the rasteriser. Block 3's third part, after LNS&ZOOM. Run: mapping `boot = 5723 + tick` (§2);
music MUSIC1 order 62, entry pre-roll 8 polls (§8). Reference: `reference/parts/plzpart/plasma`
(6000, 7000, 7600), `cube` (8500, 9500) and `band` (8400..8800), all RGB only
(`reference-indices.md` §13).

## 1. The data

The tables are the original's `db`/`dw` text fragments, copied verbatim and parsed in their own
format; `assets/PROVENANCE.md` has their sizes and sha256s. `TILE.INC` and `TILEPAL.INC` are not
copied: `PLZPART.PRJ` names neither and no shipped source includes them. Two of the seven are the
tree's record only: the cube's camera path and sine table are read from the release's image
(§5), so `RATA.INC` and `SINIT.INC` are parsed only by tests. `SPLINE.INC` is the release's
coefficients word for word (`the_release_reads_the_trees_spline_coefficients`).

Parsed sizes and edges (`the_inc_fragments_parse_to_their_original_sizes_and_edges`): `PSINI`
16,384 `db` bytes (first three 64, last four 62/63/63/63); `LSINI4`/`LSINI16` 8,192 `dw` words
each (first two 512/513, last 510); `SINIT` 2,575 bytes; `PTAU` 129 values; `SPLINE` 1,024
words; `RATA` 1,088 words, 136 control points of 8 (36 written lines plus `REPT 100`). The
release's `buu` has the same 1,088 words; its sine table is 10,240 words.

The table bases are the map's (`PLZPART.MAP:368-369,387,391`), not `ASMYT.ASM:46-61`'s
placeholders (`tables.rs`'s `MAP_*`); the plasma expression is `plasma.rs`'s module doc
(`the_plasma_line_is_the_assemblers_index_expression`). Each row
writes 84 bytes for an 80-byte visible line, so the last 4 land in the 96-byte stride's gap
(`ASMYT.ASM:41-69`): kept, and invisible.

## 2. Mapping: `boot = 5723 + tick`

The part's first graphics frame is a measured window, `(5720, 5730]` (`reference-indices.md`
§13). Tick 0 is boot 5723: `_np_zframe` reads zero there, where the part's `setmframe(0)`
(`PLZ.C:69`) lands (`music-clock.md` §2). The keyframes are ticks 277 / 1277 / 1877 (boot
6000 / 7000 / 7600) and 2777 / 3777 (8500 / 9500). Conventions: `part-sequence.md` §10.

The part's `self.tick` is 1-based (the first `frame` call is tick 1), while the dump manifest
numbers the same frame from 0. Ticks in this note are manifest indices.

## 3. Timeline

| tick | boot | what |
|---|---|---|
| 0 | 5723 | the plasma gate (`PLZ.C:68`) passes; `setmframe(0)` |
| 975, 2010, 2527 | 6698, 7733, 8250 | the timetable's first three switches (`PLZ.C:46`, `getmframe` thresholds 723 / 1491 / 1875; the boots are the trace's `zframe` crossings) |
| 2592 | 8315 | the cube's mode set: the plasma's exit test (`curpal == 4 && cop_drop > 64`, §10) fires at 2591 and `PLZ.C:119`'s wait ends here |
| 2641 | 8364 | the cube's `musplus() >= 13` gate passes (order 68 row 13) |
| 2642 | 8365 | the cube's first draw |
| 2777 | 8500 | cube keyframe: `frames` 136, page 4, display start 0x0028 |
| 3777 | 9500 | cube keyframe |

The switches and exit are asserted with the real player by
`the_plasma_switches_and_exits_on_the_reference_ticks`, the gate by
`the_music_pre_roll_lands_the_cube_gate_on_the_reference_tick`. The exit lies in the capture's
`(8310, 8340]` window. The tree's fourth switch (2259) would be boot 8769, and
`timetable[4] = 2778` is never reached. The cube's exit,
`-4 <= musplus < 0` (`VECT.C:119-120`), is long after 9500.

## 4. The plasma at the keyframes (A24)

| boot | tick | differing pixels | max channel delta |
|---|---|---|---|
| 6000 | 277 | 14,348 (11.209 %) | 16 |
| 7000 | 1277 | 7,486 (5.848 %) | 21 |
| 7600 | 1877 | 5,186 (4.052 %) | 17 |

The reference's plasma is a tear: `plz()`'s loop has no frame wait (`PLZ.C:74-122`), the
copper's `moveplz`/`pompota` can land between its four plane passes, and `setplzparas` latches
`k`/`l` at each pass's start. A diagnostic that forced the port's walk before each pass gave:

| split before pass | 0 | 1 | 2 | 3 | boundary (the port) |
|---|---|---|---|---|---|
| 6000 | **4.15 %** | 5.23 | **4.38** | 8.42 | 11.21 |
| 7000 | 19.48 | 15.68 | 12.09 | 8.59 | **5.85** |
| 7600 | 26.19 | 19.47 | 14.12 | 8.28 | **4.05** |

6000's tear sits before pass 0 or 2; 7000 and 7600 sit at the boundary. No fixed split matches
all three (split 3 gives 8.42 / 8.59 / 8.28), so the port keeps the boundary, the loop's
phase-locked steady state. Every colour delta is within a sixteenth of the palette, and the
timetable and palette events match, so the residual is the tear. The diagnostic is not in the
tree (the table is its record); A24 carries the argument.

## 5. The cube is the release's

The release's camera path, sine table and rotation are read or translated from the unpacked
image (D49, A23). `release.rs` and `cube.rs` have the mechanism; the facts the reading
rests on:

- **The camera path.** `calculate` calls `getspl` as `call far 1389h:1084h` (image `0x12c8a`),
  and `getspl`'s `mov si,0004h` (`0x1491c`) is `mov si, OFFSET buu`, so the control points are at
  image `0x13894`. Against `RATA.INC` every rotation and light word is 8× the tree's, the
  `REPT 100` tail is `0,0,500,0,0,0,2048,4096`, and `dz` is the release's own in rows 14..31
  (325, 300, 300, 325, 350 ×11, 325, 300, 325 for a flat 300). The coefficients at `cs:0884h`
  (`0x14114`) are `SPLINE.INC`'s. `getspl_interpolates_the_release_camera_spline_at_the_keyframes`
  pins the outputs at frames 0 / 136 / 1136 / 3000: `[0,432,499,3999,5599,3999,0,0]`,
  `[0,0,499,5676,3914,2314,79,0]`, `[0,0,349,5416,4181,2582,2047,0]` (the close-up at `dis` 349,
  where the tree's is 299) and `[0,0,499,0,0,0,2047,4095]`; frame 3000 reads the `REPT` tail.
- **The angles are 13 bits** (image `0x12c7d`): `and ax,1fffh` where `VECT.C:138-142` has `&1023`,
  read through an 8,192-entry turn at image `0x28b10` with `kosinit` 2,048 words on
  (`the_release_sine_table_is_an_8192_entry_turn`). The light is `VECT.C:144-146`'s
  (image `0x12cbd`), with `ls_y = kosinit[ls_kx]>>8`.
  `the_light_keeps_every_shade_inside_its_band` asserts every visible face's shade stays in
  `0..=64` over frames 0..2199; reading `kosinit[ls_ky]` instead fails at frame 369 (shade 66).
- **The rotation** (image `0x12d40` `count_const`, `0x1307a` `rotate`): a 1.15 matrix with
  `cxz = -SY` (`count_const_is_the_releases_one_point_fifteen_matrix`). The projection is the
  tree's pair, `xxx = (xx16 << 8)/zz16 + 320` and `yyy = (yy16*142)/zz16 + 66` (`VECT.OBJ`:
  `imul eax,eax,0x8e` / `add ax,0x42`), centring the cube on row ~67
  (`the_cube_projection_is_the_trees_142_66`).
- **The 134-row screen.** `TWEAK.ASM:31-33` sets CRTC 0x09 = 2 (three scanlines a row, "134 y"),
  `PLZA.ASM:77` clips at 134, and the buffers are 136 rows apart. The release does the same: a
  live probe at boot 8500 read CRTC 0x09 = 2 in 26 of 30 frames (A23 has the command). A 320×200
  frame shows row ⌊2y/3⌋ (`a_134_row_draw_stays_inside_its_slot`,
  `the_cube_frame_shows_row_two_y_over_three`).
- **The texture warp** adds `dd*16` paragraphs, so the sample is `dist1[dd*256 + bx]`
  (`the_texture_warp_shifts_dist1_by_dd_paragraphs`).

## 6. The copper

The mapping is the identity (`copper.rs`'s module doc has the table); one tick is one copper frame, and
`pompi` starts at 1 so the 60/61 alternation lines up with the reference
(`the_split_and_pan_alternate_at_the_reference_parity`; `Split::scanline` is the register plus
one, `reference-indices.md` §13.2).

## 7. The cube at the keyframes, and the band corpus

8500 and 9500 are byte-identical (rediff's `plzpart/cube`).

The band corpus is six frames of the close-up, filed for A23:

```bash
cargo xtask reference capture --arg 3 --boot autoexec --keys "KBD_left KBD_enter" \
    --cycles 30000 --frames 8400,8450,8500,8600,8700,8800 --out reference/parts/plzpart/band
```

| | 8400 | 8450 | 8500 | 8600 | 8700 | 8800 |
|---|---|---|---|---|---|---|
| differing px | 340 | 24 | 0 | 0 | 148 | 4 |

The indexed read refused because the screen moved while it was read. The 516 pixels are A23's
residue: all painted on both sides, at a channel delta of at most 9, in row runs on one face,
and inside those frames the reference shows one colour index in two colours (24 of the port's
indices at 8400, 18 at 8700). `vect()` sets `cop_pal=fpal; do_pal=1` before `calculate`
rewrites `fpal` (`VECT.C:123-125`), so the shade the DAC holds depends on the original's CPU
time.

## 8. The music

- **The seek** is MUSIC1 order 62 (`shell::music_for_part`), where
  `reference/music/second3a/trace.csv` holds the sequence when the part's window opens.
- **The plasma's gate** (`PLZ.C:68`) passes at tick 0 on a fresh run: the seek has put the
  player where the reference's `musplus >= 0` was, and the `254` at order 61 that gave
  `zplus == 2` is behind it. A mid-chain run uses the source's `musplus() >= 0`.
- **The entry pre-roll is 8 polls**, on a fresh start. With them the cube's `musplus() >= 13`
  gate passes at manifest 2641 = boot 8364, the frame the published order 68 row 13 first stands
  on, and the manifest reads the trace's **68:46** at boot 8500
  (`the_music_pre_roll_lands_the_cube_gate_on_the_reference_tick`,
`the_cube_keyframe_reads_the_traces_68_46`). A
  nine-poll pre-roll puts the gate a frame early and reads 68:47.

## 9. The preserved bugs

`bugs.md` §1 and §17 hold them: the 84-byte `plzline` overrun (§1); `do_clear`'s zero-count
mask underflow (`do_clears_mask_underflow_reads_the_byte_before_end_mask`); the palette-leak
banding (`a_table_switch_zeroes_the_colour_half_and_keeps_the_fraction`); the cut-short last
fade (`the_last_drop_step_resets_the_split_and_adds_nothing`); the unreachable
`timetable[4] = 2778`; `do_drop`'s `dtau` reaching 404 (`COPPER.ASM:175-181`), off the 400-line
screen, written as an inert `Split::scanline` 405; the double `+160` (the release's one
`add ax,140h`); and the subtraction inside `count_const`'s final shift. The tree's `>>15+7` is not
the release's 15-bit shift, so the port does not keep it (`bugs.md` §17.4). The stale
`PLZPART/MAIN.ASM` is not linked; `MAIN.C` is the entry point.

## 10. Divergences (D24)

- **The exit palette index is the release's 4, not the tree's 5** (A22, closed). The tree's
  `curpal == 5` (`PLZ.C:94`, `PLZ.OBJ`'s `cmpw $5`) would fire at boot 8769, past the plasma
  screen, which is gone by 8340. The measured exit is tick 2592 = boot 8315
  (`the_plasma_switches_and_exits_on_the_reference_ticks`), and the release's `plz()` compares `curpal`
  (DGROUP `0192h`) with 4 at image `0x1181c` (`the_release_exit_index_is_the_images`).
- **The cube is the release's** (§5, A23). Both keyframes are byte-identical; four band frames
  keep A23's palette-timing residue.
- **The plasma's tear timing** (§4, A24).
- **The exit poll is omitted.** The source polls `dis_exit()` at every loop boundary (`PLZ.C:68`,
  `:76`; `VECT.C:116-117`), and the port's `frame` does not. The engine's exit flag is sticky and
  the shell retires a part at the end of the tick the flag was set in (`Sequence::tick`), so ESC
  ends the part on the same tick either way (A62).

## 11. Verification

| capture | keyframe | tick | differing px | register |
|---|---|---|---|---|
| `plasma` | 6000 | 277 | 14,348 | A24 |
| `plasma` | 7000 | 1277 | 7,486 | A24 |
| `plasma` | 7600 | 1877 | 5,186 | A24 |
| `cube` | 8500 | 2777 | 0 | exact |
| `cube` | 9500 | 3777 | 0 | exact |
| `band` | 8400 / 8450 / 8500 / 8600 / 8700 / 8800 | 2677..3077 | 340 / 24 / 0 / 0 / 148 / 4 | A23 |

All RGB. The rows are the `plzpart/*` entries of the rediff table (`xtask/src/rediff/table.rs`);
rediff's `report.json` gives the plasma's max channel deltas (§4). In a full run the part's first frame is a `FULLRUN_SEAMS` row (21,927 in the original,
21,928 in the port; A53).

Reproduce:

```bash
cargo xtask rediff --only plzpart
cargo test -p secondreality --test parts plzpart::
cargo test -p secondreality --bin secondreality parts::plzpart
```

## 12. Open items

- **A23's band residue** (340 / 24 / 148 / 4 pixels) and **A24's tear** depend on the original's
  per-frame CPU time, which no capture or trace records.

**Register.** `bugs.md` §1 (the palette-leak banding) and §17 (the mask underflow, the cut-short
fade, the double `+160`, the shift precedence).
