# MINVBALL — the mini vector balls

Sources: `DOTS/MAIN.C` (291 lines) over `DOTS/ASM.ASM` (250 lines, `drawdots`) and
`DOTS/SIN1024.INC`; no other assets. `DOTS/MAKEFILE:20` builds `dots.exe` and stages it as
`minvball.exe`. Port: `bins/secondreality/src/parts/minvball.rs` (`"minvball"`); the module doc
has the acts, the RNG, the tick map, the drawing and the preserved bugs. Block 3's fourth part,
after PLZPART. Run: mapping `boot = 10,368 + tick` (§3); music MUSIC1 order 75, entry pre-roll
256 polls (§5). Reference: `reference/parts/minvball`, 10,900, 11,200, 11,800, 12,400, RGB only
(`reference-indices.md` §14).

## 1. Timeline

| tick | frame counter | boot | source | what |
|---|---|---|---|---|
| 0 | | 10,368 | `MAIN.C:79-167` | the setup is up; the fade's first wait |
| 1..129 odd | | 10,369..10,497 | `:171-183` | the 65 fade uploads, `pal - b` for b = 64 down to 0 |
| 130 | 1 | 10,498 | `:185-284` | the main loop's first iteration |
| 532 / 832 / 1,432 / 2,032 | 403 / 703 / 1,303 / 1,903 | 10,900 / 11,200 / 11,800 / 12,400 | | the keyframes |
| 2,579 | 2,450 | 12,947 | `:185` | the frame cap ends the loop; the part retires and holds its last frame |

The position phases end at frame counters 500, 900, 1,700 and 2,360, and the palette phases at
2,400 (blue ramp) and 2,440 (every entry to black) (`MAIN.C:202-269`). The cap tick is asserted
by the integration test (`the_run_reaches_the_frame_cap_and_retires`).

## 2. The RNG is Microsoft C's

`DOTS/MAKEFILE:10-16` compiles with `cl /qc /AL` (Microsoft C 6.0's large model), assembles with
`tasm` and links with Microsoft's `link /E`; the linked `DOTS.EXE` carries the "MS Run-Time
Library - Copyright (c) 1990, Microsoft Corp" string (`strings -a DOTS/DOTS.EXE`). Its generator's
first ten values are 41, 18,467, 6,334, 26,500, 19,169, 15,724, 11,478, 29,358, 26,962, 24,464
(`msrand::tests::the_first_values_are_microsoft_cs_sequence`). Borland's generator (seed
1, multiplier `0x015A4E35`) is a different stream. With Microsoft's, all four keyframes match
(§6), so the frames decide it (D24, A19).

The source never seeds. The setup's values are pinned by
`the_setup_is_deterministic_and_its_permutation_is_fixed`: the first twelve `dottaul` entries are
223, 464, 331, 287, 94, 64, 6, 173, 17, 208, 10, 216, and the 500 swaps are a permutation of
0..511. The second shuffle changes no value (every dot starts identical), but its 1,000 calls put
the scatter's stream where the reference has it, so the port runs it.

## 3. Mapping: `boot = 10,368 + tick`

The part's first frame is in the measured window `(10,300, 10,400]` (`reference-indices.md`
§14.5). The four keyframes are reproduced at ticks 532, 832, 1,432 and 2,032, which is the fit's
own evidence (rediff's `minvball` entry).

**The independent pin is the survey's 12,900 flat-grey frame.** Its sessions record
`(239,239,239)` and `(231,231,231)`, the `c = 59` and `c = 57` uploads (`reference-indices.md`
§14.5). Under the mapping boot 12,900 is frame counter 2,403, whose upload is `pal2(2402)`, the
`c = 59` step; the other session's `c = 57` is the same curve one frame on. A different capture
and a different observable, it pins the mapping to ±1.

**`repeat` is one.** The original advances the physics `repeat` times per iteration and the port
once a tick. The keyframes' frame counters differ by 300, 600 and 600, exactly the boot-index
differences, and the ball rows (one integration a draw) match at every keyframe; a `repeat` above
one between two keyframes would advance the counter without a draw and move them. That covers
10,900..12,400; the 12,947 end is the mapping's prediction from the source's 2,450-frame cap,
not a measured frame.

The manifest and anchor rules are `part-sequence.md` §10 (mapping, pre-roll and block-seam
conventions). In a full run the part's first frame is a `FULLRUN_SEAMS` row (26,673 in the
original, 26,670 in the port; A53).

## 4. The drawing, and the fitted `j`

The z rotation (`ASM.ASM:65-74`) is `sub ax,bx` / `sub dx,cx`, object bytes `2b c3 2b d1`
(`grep -obUaP '\x2b\xc3\x2b\xd1' upstream/SecondReality/DOTS/ASM.OBJ upstream/SecondReality/DOTS/DOTS.EXE`
finds them once each, at offsets 782 and 3926). `sub dx,cx` ignores the carry and `ax` is
overwritten at `:74`, so the stale `bx` never reaches `bp` (`the_stale_bx_never_reaches_bp`,
`bugs.md` §18.1).

The off-screen shadow store (`ASM.ASM:194-196`) makes the largest byte any write touches
64,642; only the first 64,000 are displayed, and `bgpic`'s bytes past 64,000 are the source's
undefined read and the port's zero fill (D24;
`the_shadow_restore_bug_stores_the_old_balls_third_row_word`). The gravity skip for an
off-screen shadow is `an_off_screen_shadow_holds_the_dots_y_still`.

**The initial `j` is fitted.** `MAIN.C:87` declares `j` and `:200` reads it, unassigned. `j` only
decides which dot draws a coincident position, so the reference's overlaps are the only evidence.
The port starts at `INITIAL_J = 88`:

```bash
cargo test --release -p secondreality --bin secondreality \
    parts::minvball::tests::the_initial_j_is_fitted -- --ignored --nocapture
# best five (pixels, j): [(0, 88), (283, 171), (296, 168), (297, 386), (300, 14)]
```

88 is the minimum of all 512 against the 10,900 frame (`the_initial_j_is_fitted_to_the_reference`)
and the only value at 0; the next best leaves 283. 10,900 is the fit's objective, so its 0 is by
construction; the other three keyframes are out of sample (A19).

## 5. The cue and the music

`MAIN.C:191-195` breaks the loop when `dis_musplus()` is in the open interval `(-4, 0)`
(`the_cue_window_is_the_open_interval_minus_four_to_zero`); a script that walks past the window
leaves the part drawing (`a_skipped_cue_leaves_the_part_continuing_and_drawing`). The player
publishes -3 from boot 10,368 (order 75 row 61), during the fade, which does not check the cue.
The first -3 the main loop would see is boot **12,950**, after the cap at 12,947, so the cap ends
the loop in both the reference and the port, and the cue never fires.

**There is no `dis_indemo`.** The run is always "in demo", so the cue applies to standalone runs
too. The source's non-demo branch (`:285-289`, the mode set back to 3h) has no picture and no
port equivalent.

**D28's held reading is declined here.** The part reads the cue from the engine's own poll for
the tick, not the previous tick's reading. The only reading the difference could change is one in
the window, and the first main-loop -3 is past the cap, so holding the reading would move the
check a tick and change no picture. This is a choice for this part, not a ruling on other cues.

**The seek and the pre-roll.** The shell seeks MUSIC1 to order 75, the order
`reference/music/second3b/trace.csv` holds in the part's start window. The published position at
boot 10,368 is **75:61** (`zplus` 1, `musplus` -3); a player seeked to 75 reaches it on its
257th poll, so `init` pre-polls 256 on a fresh start
(`the_music_pre_roll_puts_the_reference_state_at_tick_zero`). The fade's 130 ticks then carry the
player into order 77, the trace's position at 10,498. With no player the part runs without the
pre-roll. Reached mid-chain it takes none: the block-3 chain test publishes 75:60 on PLZPART's last
frame and MINVBALL's first, one poll apart, and the trace's 75:61 at chain 9,946, which a 256-poll
jump could not (`part-sequence.md` §10.2).

## 6. Verification

| keyframe | tick | frame counter | plane | differing px | register |
|---|---|---|---|---|---|
| 10,900 | 532 | 403 | RGB | 0 | exact (A19's fitted `j`) |
| 11,200 | 832 | 703 | RGB | 0 | exact |
| 11,800 | 1,432 | 1,303 | RGB | 0 | exact |
| 12,400 | 2,032 | 1,903 | RGB | 0 | exact |

The rows are the `minvball` entry of the rediff table (`xtask/src/rediff/table.rs`). The dots
move every frame, so no frame carries `.idx`; `each_keyframes_neighbours_differ_from_it`
(`tests/parts.rs`) holds `K-1, K, K+1` for each (D19), so each keyframe is reproduced at
exactly one frame counter.

The release's `MAIN/DATA/MINVBALL.EXE` (7,395 bytes) is PKLITE-packed; the tree's
`DOTS/DOTS.EXE` (10,372 bytes) is the development build of the source ported here. The tree's
source reproduces all four frames, so no D24 adoption is needed beyond the fitted `j`.

Reproduce:

```bash
cargo xtask rediff --only minvball
cargo test -p secondreality --test parts minvball::
cargo test -p secondreality --bin secondreality parts::minvball
```

## 7. Open items

None. The release's stack value for `j` is not readable; the fit stands in for it (A19).

**Register.** `bugs.md` §2 (the painter's order) and §18 (the inert `mov ax,ax`, the clobbered
offset, the skipped gravity, `j`).
