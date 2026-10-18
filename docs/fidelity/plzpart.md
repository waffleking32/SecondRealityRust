# PLZPART — the plasma and the cube

`PLZPART/MAIN.C` runs `init_copper(); initvect(); plz(); vect(); close_copper()`:
**one part, two screens.** `plz()` (`PLZ.C:62-122`) is the 320×400 plasma,
`vect()` (`VECT.C:110-130`) the 320×200 textured cube; `PLZFILL.C` is the cube's
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
`SINIT` is 2,575 bytes; `PTAU` 129 values; `SPLINE` 1,024 words; `RATA` 296
words = 37 control points of 8.

**The map, not the assembler's placeholders, fixes the table bases.**
`ASMYT.ASM:46-61`'s `0xC100`/`0xC200`/`0xC300`/`0xC400` sit inside `plzline`'s
own instructions; `setplzparas` (`ASMYT.ASM:74-95`) patches each one to a
table address. `PLZPART.MAP:368-369,391` gives `_psini 027D:0000`,
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

The reference's plasma is a **tear**: it is presented at a sub-frame retrace,
and adjacent rows can belong to two different parameter phases. The evidence
is in the reference's own frames: at 6000, the even rows and the odd rows'
planes 1/3 follow walk T while the odd rows' planes 0/2 and rows 90+ follow
T+1. No fixed-phase port can be byte-identical to a frame that mixes two
phases; the three numbers above are the residual of that, with every colour
delta within a sixteenth of the palette (16/21/17 of 255). The timetable and
palette events match the reference's, so the residual is phase, not palette.

## 5. The cube

`vect()` (`VECT.C:110-130`): `tw_opengraph()` (chain-4 off, 200 rows, a
160-byte plane row), wait for `musplus() >= 13`, then per frame `swappage`,
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
  `xxx = (xx*256)/zz + 160 + 160`, `yyy = (yy*142)/zz + 66`
  (`the_cube_projection_adds_three_hundred_twenty_and_sixty_six`).
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
  own 262,144-byte video and copies the visible 320×200 window into the
  surface (`the_cube_beam_wraps_at_sixty_four_kilobytes_of_plane_bytes`).

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

## 7. The cube at the keyframes — an open divergence

`cargo test -p secondreality --test plzpart -- --nocapture` prints:

| boot | tick | differing pixels | max channel delta |
|---|---|---|---|
| 8500 | 2777 | **44.352 %** | 243 |
| 9500 | 3777 | **92.866 %** | 255 |

The cube gate is pinned: the published order-68 row 13 first stands at boot
8364 (`reference/music/second3a/trace.csv`), and the part's `musplus() >= 13`
gate with the measured nine-poll entry pre-roll passes on boot 8364
(`the_music_pre_roll_lands_the_cube_gate_on_the_reference_tick`: the 0-based
tick is 2640, the part's 1-based 2641). The first drawn frame is the gate's
successor, boot 8365, and the source advances `frames` by one per draw, so the
animation's frame count is the reference's. The reference's cube frames are
byte-identical across four capture sessions (`reference-indices.md` §13.5),
so the difference is not a capture phase.

What was found and fixed while chasing it:

- the ctau entries were stored as max-start/min-end (the intersection) instead
  of the union hull, which turned `do_clear` inside out and left the sliver's
  trails (`the_clear_erases_a_moved_quad`);
- `do_clear`'s row and mask-loop bases were missing the drawn page's high
  offset byte, so pages 0x5500/0xAA00 cleared into page 0;
- the gate frame now performs the loop's first `swappage` in the same frame
  period (`VECT.C:116-117`), so the displayed page is the one just drawn;
- the span divisor and the edx sign-correction mask were wrong and are fixed.

What was measured and ruled out: the page/display parity (a
`--dump-frames 2775,2776,2777,2778,2779,2780` sweep differs by 38.7–44.6 % at
8500 — no page is the reference's), the spline phase (offset probes during
development did not lower the residual at both keyframes together), the
projection constants (142 and the double +160 are the source's and are
pinned by `the_cube_projection_adds_three_hundred_twenty_and_sixty_six`), and
the extent/clear pair (now exact). The remaining suspects are the tree's
`RATA.INC`/`SPLINE.INC` camera data against the release's (D24), or a
still-unfound rasteriser difference; the divergence is recorded as
`APPROXIMATIONS.md` A23, **registered, not adopted**. The integration test
pins the measured percentages so a future change cannot silently move them.

## 8. The music

- **The seek.** `shell::music_for_part("plzpart") = (Music1, 62)`: order 62 is
  where `reference/music/second3a/trace.csv` holds the sequence when the
  part's start window opens.
- **The entry pre-roll is 9 polls**, guarded on `ctx.music_fresh()`. The
  player's published words lag the reference's when the part starts; with nine
  pre-polls the cube gate (`musplus() >= 13`) passes at the reference's boot
  8364. The test measures it with the real player, not a
  constant: nine pre-polls, the tick-0 engine poll, then the first poll whose
  `musplus` reaches 13 (0-based 2640 = the part's 1-based 2641 = boot 8364).
  A mid-chain start has already run through those polls and takes none.
- **The plasma's gate** (`PLZ.C:68`) is the same position: the seek has
  already put an entry run where the reference's `musplus >= 0` was waiting,
  and the `254` at MUSIC1 order 61 that gave the reference `zplus == 2` is
  behind the seek. A fresh run's gate is tick 0; a mid-chain run uses the
  source's `musplus() >= 0`.
- **The cube's exit** is `-4 <= musplus < 0` (`VECT.C:119-120`), long after
  the two keyframes.
- **A publication-phase note.** At boot 8500 the trace's published position is
  `68:46`; the port's is `68:47` (manifest of the keyframe dump), one row
  early. The gate frame itself is the same (row 13 at 8364), so this is the
  DIS's publication instant, not the seek or the pre-roll; the note records it
  rather than hiding it. No keyframe's pixels depend on it.

## 9. The preserved bugs

- The 84-byte `plzline` overrun into the 96-byte stride gap.
- `do_clear`'s zero-count mask underflow: `end_mask[x-1]` at `x = 0` reads the
  byte before the table, `start_mask`'s last `0x0F`
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
- **The cube's keyframe residual (§7)** is registered as A23, not adopted: the
  difference is measured, the reference is deterministic, and the root cause
  is still open.

## 11. The tests and the run

```bash
cargo test -p secondreality --bin secondreality parts::plzpart
cargo test -p secondreality --test plzpart -- --nocapture
```

The 19 unit tests cover the data, the map, the plasma expression, the
timetable, the release exit, `dtau`/the split walk, the split/pan parity, the
spline, the projection, `count_const`, the mask underflow, the beam wrap, the
clear, the synthetic rasteriser cases and the music gate. The three
integration tests cover the registry and the two screens, the music row, the
three plasma keyframes (bounded) and the two cube keyframes (measured and
pinned). The five RGB reference files are `reference/parts/plzpart/{plasma,cube}/`;
`cargo xtask reference verify --tree reference` checks the tree.
