# ALKU — the intro text and the horizon pan

`ALKU/MAIN.C` (542 lines) is the part, over `ALKU/ASMYT.ASM` (110 lines: the
two blitters) and `ALKU/COPPER.ASM` (149 lines: the three copper routines), with
`ALKU/TWEAK.ASM` (333 lines) as the shared `tw_*` library PAM and CRED also
carry. (`awk 'END{print NR}'` over each file.) This note records what the source does, what the port does, the one
D24 adoption (three palette bytes), and the measured difference at each of the
four reference keyframes.

The port lives in `bins/secondreality/src/parts/alku.rs`; its integration test
is `bins/secondreality/tests/alku.rs`. Every number below is reproduced by a
command named in §10 or by a test in the same commit (D25).

## 1. The part's acts

1. `MAIN.C:53-56` — `int 10h ax=3`, then `init()`: `dis_partstart`,
   `tw_opengraph` (`TWEAK.ASM:18-54`), `init_copper` (`COPPER.ASM:42-62`),
   `tw_setpalette(fade1)` (a black DAC), `memcpy(palette,hzpic+16,768)`
   (`:161`), the picture blit (`:163-167`), the font's 2-bit→0x40/0x80/0xC0
   conversion (`:169-182`), `palette2`/`fade2` (`:184-210`), the palette alias
   (`:212`), the font's metrics scan (`:214-235`) and the fade tables
   (`:237-242`).
2. `:59-64` — `while(dis_sync()<1 && !dis_exit());` then card 1 (`prtc` ×3),
   `dofade(fade1,fade2); wait(300); dofade(fade2,fade1); fonapois();`.
3. `:66-70` — the same for card 2 after `dis_sync()<2`.
4. `:72-77` — card 3 after `dis_sync()<3`: `in`, and the two 215-column
   pictures the `fonaorder` bytes `\x8f` and `\x99` name.
5. `:79-85` — after `dis_sync()<4`: `memcpy(fadepal,fade1,768)`,
   `cop_fadepal=picin`, `cop_dofade=128`, `a=1,p=1,frame_count=0`, and
   `do_scroll(2)` until the fade ends.
6. `:87-146` — the scroll loop: `do_scroll(1)` at the bottom of every iteration
   and a text batch when `f>50 && cop_dofade==0`; the batches wait
   `dis_sync()<4+tptr` with `a&1` (`:138`) and `fmaketext` lays them on the
   picture.
7. `:147-152` — `if(f>63/SCRLF) dofade(palette2,palette);`, `fonapois()`,
   `close_copper()`.

## 2. The display: 320×400 from a 640×200 picture

`tw_opengraph` sets mode 13h, chain-4 off, CRTC 0x14 = 0, CRTC 0x17 = `0xe3`,
CRTC 9 = 0 and CRTC 0x13 = `0x5813` (`TWEAK.ASM:18-54`). The offset is 88
units = **176 bytes a displayed line** (`reference-indices.md` §1.6: the
oracle's own row stride for this screen is `scan_len * 2`), and one plane byte
holds four pixels: `tw_putpixel` writes plane `x & 3` at byte
`y*176 + x/4` (`TWEAK.ASM:87-113`). A displayed line is therefore 704 pixels.

`outline` (`ASMYT.ASM:60-108`) writes a source dword — four source columns —
into the four planes at byte column `a` of two displayed rows,
`di + ccc*352` and `+176`, and `init` calls it twice for every `a` in 0..87:
once at column `a` and once at `a+88` (`MAIN.C:163-167`). That fills rows
50-349 with the picture's first 352 columns **twice** — a starting state.
`do_scroll`'s refresh (`:405-409`) then rewrites byte column `a/4+86` (and
`+88`) from the source dword at `(a/4+86)*4`: for columns past 87 that is the
picture's *continuation* (columns 352 and up), so the second 352 pixels slide
along behind the first instead of repeating it.

The window is `cop_start = a/4 + p*88` bytes in with `cop_scrl = (a&3)*2` of
AC pel panning (`:403`). The port writes `start_addr = 4*cop_start`,
`x_offset = 0` and `pan = cop_scrl/2` — the AC register's unit is half a pixel
(`docs/fidelity/cred.md` §3) — and reads a flat video memory with a 704-pixel
line stride. **Why not a 640×200 surface with `double_scan`?** Because the
source has *two* 176-byte memory rows per picture row: `outline` writes
`di + ccc*352` and `+176`, and the refresh's `+88` write crosses into the next
row when the column passes 87. A folded model shows the same picture per pair
but loses the spill — the row crossing is what makes the `p=1` window the
continuation rather than a wrap — so the port keeps 400 rows of 176 bytes
(350 written, the rest zero) and the view does not double-scan. The first displayed pixel is `a + p*352`; the fresh copies are
equal at init and the refresh's spill carries the continuation into the next
row, so the two `p` values show the same picture. The unit test
`the_pan_is_the_crtc_start_and_pel_panning` pins the start, the pan and the
first pixel for `a` 0, 1, 2, 3, 4, 7, 308, 309, 318 and 319, both `p`;
`the_picture_blit_writes_the_band_and_the_refresh_reads_past_the_row` pins the
copies and the spill.

## 3. The copper: three slots, and the port's mapping

`init_copper` registers the three routines at the DIS's own slots
(`COPPER.ASM:42-62`):

| routine | source | original slot | what it does |
|---|---|---|---|
| `copper1` | `:64-81` | **1** (before retrace) | writes CRTC 0x0C/0x0D = `cop_start` and AC 0x13 = `cop_scrl` |
| `copper2` | `:83-104` | **2** (retrace) | `inc frame_count`; if `do_pal`, uploads 768 bytes of `cop_pal` to the DAC |
| `copper3` | `:115-145` | **0** (after the display start) | the 8.8 fade: decrements `cop_dofade`, adds `cop_fadepal` into `fadepal`, sets `cop_pal=fadepal` and `do_pal=1` |

The engine's slots run in the same order — 0 at the top of the screen, the
part's frame, 1 just before retrace, 2 in retrace — so **the mapping is the
identity**: `copper3` → slot 0, `copper1` → slot 1, `copper2` → slot 2. The
fade applied in slot 0 is uploaded by slot 2 in the same tick, so a frame is
composed with the previous tick's upload — the original's line-20 write and
its retrace upload, seen one frame later. The unit test
`all_three_copper_slots_are_installed_and_each_does_its_job` drives all three
slots through the engine's order, and
`the_fade_applier_uploads_the_post_add_palette` pins slot 0's `add al`/`adc
ah` arithmetic on a two-high accumulator (`+100` and `−240` deltas give
`0x032C` and `0x3B10`, and `cop_pal` is the post-add palette).

## 4. The tick is the copper's frame

`frame_count` is a word `copper2` increments once per vertical blank
(`COPPER.ASM:85`). The engine calls one tick per frame, so **one tick is one
copper frame**: slot 2 increments the same word once a tick, and every
`while(frame_count<9);` in `do_scroll` becomes a 9-tick cadence — `do_scroll(1)`
and `(2)` are blocking waits that consume the tick and retry next tick, never
spins, and `do_scroll(0)` steps only when the counter has reached nine. The
unit test `the_scroll_advances_one_step_every_nine_ticks` pins the cadence:
the first step is the ninth tick after the picture's reset, the view is
`start_addr 4*88, pan 1` at `(a=1,p=1)`, it holds for eight ticks, and the
eighteenth tick is `(a=2,p=0)` — `start_addr 0, pan 2`. The source's
`dofade` and `wait(300)` are paced by the same counter; the port's `dofade`
steps once a tick and its `wait(300)` is a 300-tick `Hold` with its own
counter where the source counts `frame_count` — one frame a tick either way,
and the port resets `frame_count` at the source's own points. The card is therefore **428 ticks** — the
fade-in's 64, the hold's 300 and the fade-out's 64 — and its 428th tick hands
over to the next gate's stage
(`the_stage_gates_follow_the_music_and_a_skipped_code_still_passes_them`
runs to `5 + 428` and sees the code-2 gate).

## 5. The stages and the text's off-by-one

The gates are `dis_sync() < 1`, `< 2`, `< 3` and `< 4` (`MAIN.C:59`, `:66`,
`:72`, `:79`). `dis_sync()` is `sync_10`'s `ordersync1` answer
(`sr_engine::music::sync_from`); the codes the trace reaches are 1 at
boot 1537, 2 at 2097, 3 at 2509 and 4 at 3069, which the shell's block-1 test
prints as `second1: trace code transitions {…}` (the frames at which
`sync_from` first answers each code over `second1`).

The batches wait

```c
while(((a&1) || dis_sync()<4+tptr) && !dis_exit() && a<319)
    do_scroll(0);
```

(`MAIN.C:138`), and `tptr` is **post-incremented** by `switch(tptr++)`
(`:102`), so the branch taken as case 0 leaves `tptr` at 1 and the wait is for
code **5**, not 4. The unit test
`the_picture_fade_hands_over_to_a_batch_gated_on_code_five` asserts the gate's
code is 5 after the picture fade; the off-by-one is preserved, not fixed.

The `f` chain (`:89-99`) is a counter of *outer iterations*, and an outer
iteration is one 9-tick `do_scroll(1)`: `f==0` starts the `textin` fade and
adds 20; `f==50` starts the `textout` fade; once `f>50` and `cop_dofade==0` a
batch runs. The empty case 4 (`:129-131`) draws no text but calls
`ffonapois`, and `fmaketext` is skipped when `aa >= 320-12 = 308` (`:141`).

## 6. The bugs, preserved

- **The palette write-order alias** (`MAIN.C:212`): after `palette2` and
  `fade2` are built, `palette[192..768] = palette[..576]` repeats entries
  0-63 three times. Every entry 64-255 is entry `v mod 64`; the test
  `the_picture_is_hoi_u_with_the_three_entries_the_release_ships` asserts it
  for all 256 entries. This is what makes the text XOR invisible over the
  picture: `xor 0x40/0x80/0xC0` and `and 0x3f` both keep the low six bits
  (`the_text_operations_keep_the_pictures_low_six_bits`).
- **The refresh reads past the row** (`:407`: `(a/4+86)*4`): the scroll ends
  at `a=319`, and the last refresh is the one at `a=316`; its source dword is
  columns 660-663 of a 640-column picture, so the last byte (663) and its
  neighbours are the next row's columns 20-23. The unit test
  `the_picture_blit_writes_the_band_and_the_refresh_reads_past_the_row` pins
  `a=316`'s column 165 to `picture[660..664] == picture[640+20..640+24]`.
- **The `memset` clears 186×320 of a 186×352 `tbuf`** (`MAIN.C:101`):
  `the_tbuf_clear_keeps_the_arrays_last_sixteen_rows` pins that the last 16
  rows survive.
- **The final fade only when `f>7`** (`:148`): `63/SCRLF` is 7. The port runs
  the 64-step `dofade(palette2,palette)` when `f` is above it and retires
  immediately otherwise, as the source does, and `fonapois()` (`:150`) runs
  after either branch of that `if`; the fade only touches entries 64-255,
  which the picture never indexes.
- **The exit path skips the source's three closing calls.** `:147-152` runs
  after either ending of the `for` loop (`:87`) — the `a=319` completion *and*
  `dis_exit` — so a prompt exit in the source still performs
  `if(f>7) dofade(...)`, `fonapois()` and `close_copper()`. The port's `frame`
  returns `Finished` at the top of the tick that sees `ctx.exit_requested`
  (`parts/alku.rs:1307-1309`) and runs none of the three: the prompt-exit /
  no-spin rule ends the part on that tick rather than fading it. Nothing
  visible is misjudged by this — the engine clears the copper on the next
  start, `fonapois` only clears the text's top two bits (`:250-270`), and no
  reference frame is near an exit — but it is recorded as a deviation, not
  claimed as equivalence.
- **The `while(frame_count<9);` spin** is the 9-tick cadence (§4), never a
  loop.

## 7. The assets

**The picture.** `hzpic` is the linked `HOI.IN0+IN1`, not `HOI.U`
(`ALKU/INCLUDE.ASM:7-12`), and the linked bytes differ from `HOI.U` in exactly
nine bytes — palette entries 1, 2 and 3 (`asset-inventory.md` §E.18):

| entry | the linked hzpic (shipped) | `assets/ALKU/HOI.U` |
|---|---|---|
| 1 | `(10, 10, 10)` | `(2, 2, 3)` |
| 2 | `(30, 30, 30)` | `(5, 5, 7)` |
| 3 | `(63, 63, 63)` | `(6, 6, 9)` |

The picture *bytes* are identical, so the port reads `HOI.U` and overrides
those three entries with the shipped values. **This is D24's adopt-the-release
case**: the difference is one data constant (nine bytes), and with it every
other difference is accounted for. The counterfactual, measured through the
same renderer (`the_curated_palette_entries_leave_the_reference_different`,
ignored; §10):

| keyframe | `HOI.U`'s own entries 1-3 | the shipped entries |
|---|---|---|
| 900 | 0 of 128,000 | 0 |
| 1719 | 1,595 of 128,000 | **0** |
| 2719 | 4,171 of 128,000 | **0** |
| 5859 | 2,706 of 128,000 | 2,358 (this file's script; 2,353 through the real binary, §8) — the text-batch phase, A20; the post-add upload fix leaves the counts unchanged |

The cards are the strong evidence: their text is drawn in entries 64, 128 and
192 of `fade2`, which are palette entries 1, 2 and 3 (`MAIN.C:194-208`), so
the two palettes give visibly different cards and the shipped one is exact.

**The font.** `ALKU/INCLUDE.ASM:17` includes `fona.inc`, which is
`FONA.UH[783:]` — one byte earlier than the decoder's pixel area at 784
(`asset-inventory.md` §8). The port builds `sr_assets::font::Font` from the
decoded 1500×30 strip with a 20×30 cell. The routine reads 32 rows
(`MAIN.C:218`/`:225`) of a 30-row strip; the two rows past the file are the
linker's uninitialised padding and A9's resolution — the port draws the 30 the
file holds. The shift itself is invisible: a test scans both
`FONA.UH[783:]` and `[784:]` and asserts all 76 runs hold the same glyph bytes
(they are the same runs one column over; byte 783 is zero).

## 8. The reference comparison

All four reference frames are `.rgb` only — the oracle refuses this screen's
`.idx` because 176 × 400 = 70,400 is more than one plane's 65,536 bytes
(`reference-indices.md` §1.6) — so every comparison is RGB, and the 6→8-bit
widening is load-bearing.

```
cargo test -p secondreality --test alku -- --nocapture
```

| index | what is on screen | port vs reference |
|---|---|---|
| 900 | the black screen | **0 of 128,000 pixels differ** |
| 1719 | card 1 mid-hold | **0 of 128,000** |
| 2719 | card 3 mid-hold (`in` over the Dolby logos) | **0 of 128,000** |
| 5859 | the scrolled picture | **2,353 of 128,000 (1.8383 %)**, all in rows 150-243 and columns 40-279 |

The three held frames are byte-identical, and their `K-1`/`K+1` support
frames are the same bytes (the test asserts it). 5859 is the moving frame:
its `K-1` and `K+1` are different pans, as D19 says a moving keyframe's
support must be.

**The 5859 residual, measured.** The picture band, the pan and the palette
are the reference's: 350 of the 400 rows are byte-identical, and every
differing pixel is inside the old text's glyph region. The difference is a
*timing phase*, not a pixel: the empty text batch (case 4) calls
`ffonapois`, which clears the text's XOR bits, but its `fmaketext` — which
would rebuild the XOR list from the zeroed `tbuf` and so keep them cleared —
runs only when `aa<308` (`MAIN.C:141`). The port's trace-faithful phase
reaches that batch at `aa=308` and skips it, so the stale XOR list re-adds the
old text on every subsequent scroll, and the next `textin` fade (in progress
at 5859, `f=22`) makes it visible. In the reference's session the batch
reached `aa=306` (or the fade was elsewhere), and the frame is bare. The
phase sensitivity test `the_5859_phase_sensitivity` scans the batches'
transitions relative to the fixed gates:

Three rows of the 37 the scan prints, others elided:

```text
shift  -9:     0 pixels differ; a=311 p=1 f=0 stage=Main(Tail { scrl: 322 })
shift  +0:  2358 pixels differ; a=311 p=1 f=22 stage=Main(Outer)
shift +10:     0 pixels differ; a=311 p=1 f=20 stage=Main(Outer)
```

With the post-gate transitions at a phase at which the frame is
byte-identical (`-9`, and `+10` as well — the scan prints both), the picture,
the palette and the pan are exact and the phase is the only difference
(`the_scrolling_keyframes_picture_is_the_references_under_its_batch_phase`).
The phase is the original's own CPU time in the batch loops (the port's
`fmaketext`/`faddtext` are zero-cost; the original's were not) and the
session's music clock; the port cannot reproduce either, and registers the
difference as **A20**.

## 9. The music: block 1's start, and the anchor

`SECOND 1` runs ALKU first (`MAIN/U2.ASM:790-792`) and never takes a block
restart, so its player's clock starts at the initial `_zplaysong(0)`
(`:766-775`). `cargo xtask music analyse --trace reference/music/second1`
fits the first row change at frame 420 with `_np_zframe` 14 and derives the
start at **399.6**; the `zframe` column's direct reading is **405**, 5.4
frames later (the same tick seen at the next whole-frame sample). The port
uses the fitted value's nearest whole frame, **`BLOCK_ONE_MUSIC_START_FRAME =
400`**; the restart recipes keep 408 (`MUSIC_START_FRAME`). A `SECOND 1` run
draws from 404, four frames after 400, so it **pre-polls four frames**; a
block-5 run waits four frames for its 408 instead.

The measurement is the shell's
`a_block_one_run_starts_under_the_initial_play_song_and_matches_second1`: a
block-1 run with the real player is compared frame by frame with `second1`.
All **638** of the trace's position changes are visited, the positions in
order are the trace's, and the port's change frames minus the trace's are

```
{-2: 1, -1: 1, 0: 3, 1: 1, 2: 217, 3: 285, 4: 118, 5: 10, 6: 1, 8: 1}
```

— the port's change frames are **2-4 frames after** the trace's for 620 of
them (the port's music is behind the trace's direct reading; the rest are the
trace's own missing frames and sample instant, A12). The code transitions
ALKU reads are 2-4 frames behind too:

```
{1: 2, 2: 3, 3: 2, 4: 3, 5: 2, 6: 4, 7: 3, 8: 4, 9: 3}
```

The anchor is **404**: the boot's first graphics frame in five runs of five
(`reference-indices.md` §1.12, D17), and `SECOND 1` enters ALKU there.
`boot_anchor("alku") = Some(404)` makes a `--part alku` and a `--block 1` run
the same run (`a_block_one_run_starts_at_alku_and_is_the_same_run` compares
their frames byte for byte); the manifest says `index_origin: "boot"`,
`anchor: 404`, `MUSIC0` at order 0.

## 10. Reproduction

```bash
# The four keyframes through the real binary, with D19's support frames.
cargo test -p secondreality --test alku -- --nocapture

# The part's arithmetic, the pan, the cadence, the font, the bugs and the
# keyframes in process.
cargo test -p secondreality --bin secondreality parts::alku

# The block-1 music recipe against the trace (prints the change-frame and
# start-code distributions §9 quotes).
cargo test -p secondreality --bin secondreality \
    shell::tests::a_block_one_run_starts_under_the_initial_play_song_and_matches_second1 \
    -- --nocapture

# The D24 adoption's counterfactual: the curated entries' residual.
cargo test -p secondreality --bin secondreality \
    parts::alku::tests::the_curated_palette_entries_leave_the_reference_different \
    -- --ignored --nocapture

# The batch phase's sensitivity at 5859.
cargo test -p secondreality --bin secondreality \
    parts::alku::tests::the_5859_phase_sensitivity -- --ignored --nocapture

# The music start's measurement.
cargo xtask music analyse --trace reference/music/second1
```

## 11. What is not claimed

- The port does not model the original's CPU time inside `fmaketext`,
  `faddtext` and `prt`; that is A20's residual at 5859 and could move the
  text's fade phase in any frame where a fade and a batch overlap.
- The 5859 picture is reproduced at the reference's pan (a=309 displayed);
  the reference's own sessions differ by 1-2 whole pixels of pan
  (`reference-indices.md` §4.4), and a future re-capture may show that offset.
- The two rows of the font the routine reads past the strip are not drawn:
  A9's resolution, cited rather than re-derived.

**Register.** The behaviours of this part that the original does and the port must keep are in `bugs.md` §10 (the palette alias, the row-spill refresh, the `tbuf` clear, the code-5 gate; `dis_sync` itself is §3).
