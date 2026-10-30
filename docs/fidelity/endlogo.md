# ENDLOGO — the end picture: the white, the 129-step fade, the hold, the 64-step fadeout

Sources: `END/END.C` (130 lines), `END/ASM.ASM` (118: `lineblit`, `setpalarea`, a dead `inittwk`)
and `END/READP.C` (64, `BEG/READP.C` line for line); asset `assets/END/PIC.UH`. Slot 18 of
`part-sequence.md` §1 and the entry point of `SECOND 5` (`partsmask[5]` = `10000b`,
`MAIN/U2.ASM:116`); 320×400, 128,000 pixels. Port: `bins/secondreality/src/parts/endlogo.rs`
(`"endlogo"`); its module doc has the ten acts and the three things the part does not do, and
`Endlogo::gate` and `GATE_ALIGNMENT_TICKS` the gate. Run: anchor **404**
(`shell::FIRST_GRAPHICS_FRAME`). Music: `MUSIC0` restarted at order 25 by the shell
(`MAIN/U2.ASM:905`); the gate is `dis_musplus() > -16`. Reference `reference/parts/endlogo/`: 430,
500, 900, 1180 (`reference-indices.md` §19). BEGLOGO links the same `lineblit`, `setpalarea` and
`readp`; `beglogo.md` explains the shared pieces, and this note gives ENDLOGO's differences.

## 1. Acts

The acts are the module doc's (`END.C:50-117`). The screen never moves after `:99`; everything from
451 to 1210 is the DAC. `:118-129` (the DOS epilogue and the scroller) is commented out and `:130`
returns from `main()`.

## 2. What the shell owns

- **The video mode.** `:64-66` is commented out; `SECOND 5` sets mode 13h and runs `inittwk`
  itself (`MAIN/U2.ASM:906-908`, after `:905`'s `restartmus`). `inittwk` clears 64 KB to index 0.
- **The cleared video memory.** `END.C` has no `memset`, so `reference/parts/endlogo/frame_000430.idx`
  is **index 0 in all 128,000 pixels**, white only because `:62-63` has written 63 into entry 0. A port
  that blitted before 430 would show the same pixels and fail the comparison on the index plane.
- **The row stride.** `init` writes `y_line_offset = None` rather than inheriting it, the convention
  in `beglogo.md` §5: the port's U2E leaves `Some(320)`, which would read every second scanline
  from the picture's right margin. With the write, block 1's ENDLOGO hold is the original's 38300 at 0
  in both planes (`cargo xtask rediff --only fullrun/11-endlogo`; `reference-indices.md` §26.8;
  `the_picture_is_read_at_its_own_row_stride_whatever_the_part_before_left`).

## 3. The two fade constants, derived by matching the reference bit for bit

For each keyframe the ramp is computed from the source's arithmetic (`:103`, `:114`), widened with
the engine's `(c*259+33)>>6`, rendered over all 320×400 pixels of `PIC.UH`, and compared byte for byte
over the whole range of `c`; the answer must be unique
(`the_fade_constants_are_derived_by_matching_the_reference_bit_for_bit`):

| frame | ramp | `c` whose 320×400 RGB is that frame |
|---|---|---|
| 500 | the fade-in, `((128-c)*63+palette[a]*c)/128` | **[49]**, unique over 0…128 |
| 1180 | the fadeout, `(palette[a]*c)/64` | **[30]**, unique over 63…0 |
| 900 | the fade-in, in the hold | [126, 127, 128]: the ramp has converged, one palette |
| 430 | — | one index (0) through a white DAC, before the blit |

- **`FADE_IN_FIRST_INDEX` = 500 − 49 = 451**: the fade-in counts up from `c = 0`, one step a frame.
- **`FADE_OUT_FIRST_INDEX` = 1180 − (63 − 30) = 1147**: the fadeout counts down from `c = 63`.

**A fit gives a different, wrong answer.** `reference-indices.md` §19.1's sweep means (237.44 at
460, 167.31 at 500, 99.54 at 540) put the fade-in's step 0 near 450. The mean of a truncated integer
ramp is a staircase, so three means cannot tell a whole step from the truncation inside one; the
scan separates 450 from 451 on every pixel whose entry the step moves, and rejects 1145 for the
fadeout likewise.

**Two independent facts hold under the constants, unfitted.**

1. **The hold is 567 ticks.** §19.1 records the sweep's samples from **580** to 1120 as one `sha256`
   (`f2514b29…`, 69 colours, mean 33.23). The fade-in's last step is displayed at 451 + 128 = 579,
   so the settled picture begins at **580**, the record's first identical sample, and lasts
   1147 − 580 = **567** frames (`HOLD_TICKS` in the part's tests).
2. **The fadeout ends inside (1200, 1240].** It runs to 1147 + 63 = **1210**; `SECOND 5`'s change
   to CRED's 320×200 screen lies in (1200, 1240] (`reference-indices.md` §20, §19.1).

What pins the constants is 500 and 1180, one each; 900 only says the ramp had ended, and 430 pins
the surface, not the fade.

## 4. The tick map, and what the port does

`init` does the acts that cost no frame: the AC unlock (ending `0x20`, PAS = 1, which the engine
always is), display start 0 (`View::set_origin(0)`), the 400-line CRTC 9, the overscan
(`border = Some(0)`, the last of the 255 and 0 writes), and the picture read in one
`sr_assets::load_path` call (refused unless 320×400). One `dis_waitb()` is one tick; the acts that
cost no blank fall on the tick after the blank before them.

| tick | index | source | screen |
|---|---|---|---|
| 0 | 404 | `:61` | `inittwk`'s index 0 through the shell's DAC: black |
| 1 | 405 | `:62-63` | white: 63 written into entries 0-254 |
| 2 | 406 | `:89` | white |
| 3 | 407 | `:90-91` | white |
| 4-35 | 408-439 | `:92`'s 32 blanks | white; **430 is tick 26** |
| 36 | 440 | `:94-99` | the picture in video memory under the white DAC (`BLIT_TICK`; `the_picture_is_blitted_after_the_vertical_blanks`) |
| 37-46 | 441-450 | — | not modelled (§5, A7) |
| 47-175 | 451-579 | `:101-106` | the fade-in, `c = index − 451` |
| 176-742 | 580-1146 | `:107-111` | the hold: 567 ticks on this boot, as long as the music says (§5) |
| 743-806 | 1147-1210 | `:112-117` | the fadeout, `c = 63 − (index − 1147)` |
| 807 | 1211 | `:130` | `PartStatus::Finished` (`the_part_retires_after_the_fade_out_has_finished`) |

**The surface is not cleared and DAC entry 255 is never written**, the source's two silences
(`bugs.md` index). The first shows at 430
(`the_opening_screen_is_the_shells_surface_through_the_shells_dac`); the second no frame shows (the
screen at 430 is index 0 and the picture indexes 0-71), so `the_white_leaves_entry_255_unwritten`
pins it with a marker. BEGLOGO writes black into 255 (`BEG/BEG.C:25-34`); the two DACs differ in
exactly the entry neither part displays, so the difference is a state the next part inherits.

## 5. The music gate

`:107-111` is `for(a=0;a<5000 && !dis_exit();a++) { dis_waitb(); if(dis_musplus()>-16) break; }`.
The port runs it one iteration a tick, with the 5000 cap
(`the_gate_holds_for_five_thousand_ticks_when_the_cue_never_comes`) and the run's player's
`musplus()`. **No hold length is in the part**: the hold is 567 ticks on this boot because the music
says so. A run with no player is refused at `init` (`a_run_with_no_player_is_refused_by_name`).

| index | what |
|---|---|
| **1144** | the port's published words cross: `26:49` at `zplus` 1, `musplus` −16 → −15 |
| **1145** | the tick the check breaks on (§5.1) |
| **1146** | the tick the port holds past it (§5.2) |
| **1147** | the fadeout's `c = 63` (§3, from 1180) |

`the_gate_opens_on_the_tick_the_reference_frames_derived` drives the gate with
`reference/music/second5`'s words tick by tick; with the run's own player, rediff's 1180 (and
block 1's `fullrun/11-endlogo`) holds the fadeout where §3 puts it.

The gate's `!dis_exit()` half is not ported: the shell retires the part on the ESC tick.

**Ten lead-in ticks are not modelled** (A7's second half, A5's class for BEGLOGO). The source
accounts for **37** of the 47 ticks from 404 to 451: 34 blanks (`:61`, `:89`, `:92`'s 32) and the
three ticks the code between them runs on. Ticks 37-46 (441-450) have no identified cause. The two
candidates are ruled out as for BEGLOGO: not the EXE load (the part's white is up by 415, and
`inittwk`'s palette clear is commented out, `END/ASM.ASM:14-20`, so that white is `END.C:62-63`'s),
and not `dis_partstart()`, a version check costing no frame (`DIS/DISC.ASM:51-64`).

### 5.1 Why the check reads the previous tick's words

The check is the instruction after `dis_waitb()`, which returns on the copper's boundary: `waitb_1`
(`DIS/DISINT.ASM:161-181`) spins on `copperframecount`, which `copper_end`
(`MAIN/COPPER.ASM:283-320`) increments at `:320` after its retrace wait (`:305-311`). The music's
poll is `call _zpollme` (`MAIN/COPPER.ASM:466`) in `copper_doit`, which the list runs at 20 of the
frame's 100 units (`copperlst0`, `:41-43`), a fifth of a frame after the boundary. So the check reads
the **previous** frame's words, while the port's engine polls before the part's code in the same
tick. The part keeps the reading a tick (`the_gates_check_reads_the_previous_ticks_words`).

### 5.2 The one frame that is an alignment

The port's player publishes `26:49` from index **1144**. `second5`'s frame `N` sample is taken at
frame `N`'s start (D17), so it holds what the port publishes at `N − 1`: the trace's row 49 begins at
1145. With §5.1 the check breaks on **1145** and the fadeout would start at 1146, while the
reference's frames put it at **1147**.

**The player is not what moved.** Over this window the port's position changes differ from the
trace's by one frame at 100, agree at 22 and differ by two at 8 (130 compared of 131; the first
sample has no earlier frame to change from):

```bash
cargo test -p secondreality --bin secondreality the_runs_music_is_where_the_trace -- --nocapture
# 130 position change(s) compared; the port's frame minus the trace's: {-2: 8, -1: 100, 0: 22}
```

That is the poll geometry's prediction: the port matches exactly when a row change falls in the fifth
of a frame between the port's poll and the original's, one in five, against 22/130 = **16.9 %**. A
shifted clock would match at none. The port's `_np_zframe` matches `second5`'s once the sample
instant's one-index shift is taken out: 753 of 891 frames exact, never more than **1 tick** apart
(`ZFRAME_SLOP`, `the_runs_music_is_where_the_trace_holds_it_at_every_frame_the_trace_has`); unshifted,
353 of 894, worst 2 ticks. The first tick is at 408.0 in four sessions (spread 0.1).

**So the session moved** (D12: the frames are the record). The port holds one frame past its break
(`GATE_ALIGNMENT_TICKS = 1`), and the cost is named: it disagrees with `second5`'s session by one
frame at this crossing. That is inside the two filed sessions' own spread (A12, `JITTER_FRAMES` 3;
the port's worst displacement from `second5` is 2 frames, 778 of 894 exact) and D17's finding that a
frame counter names pictures one frame apart in different sessions. The constant is a fact about this
boot and `second5`, not about ENDLOGO (A7).

## 6. `setpalarea`: 510 bytes, not BEGLOGO's 508

`END.C:105` and `:116` call `setpalarea(pal2,0,255)`, the routine `beglogo.md` §6 and `bugs.md` §4
describe: `2*count + start` = **510** bytes, entries 0-169 whole and 170-255 untouched
(`logo::tests::the_upload_is_two_count_plus_start_bytes`). BEGLOGO passes 254 and moves 508. It is
invisible on this art: the picture indexes 0-71 (72 distinct), the pre-blit screen is index 0, and
the derivation scan rerun with 508 bytes returns the same `c` for 500. What it leaves is a DAC whose
entries 170-254 have been white since `:90-91`, going into CRED.

## 7. Verification

| frame | plane | differing | register |
|---|---|---|---|
| 430 | indices | 0 of 128,000 (all index 0; RGB pure white) | exact |
| 500 | RGB | 0 | exact |
| 900 | indices | 0 | exact |
| 1180 | RGB | 0 | exact |
| 436-455 (lead-in, every frame) | RGB | 0 | A7 |
| 440 | indices | 55,499 (the port's plane already holds the picture) | A7 |
| 37970 (block 1) | RGB / indices | 0 / 46,997 | A53 |
| 38300 (block 1) | RGB / indices | 0 / 0 | A53 |

The counts are the `endlogo`, `endlogo-leadin/*` and `fullrun/11-endlogo` rows of
`xtask/src/rediff/table.rs` (the lead-in window is `reference-indices.md` §25.6; 37970's index plane
is the video memory U2E left, under ENDLOGO's white DAC). 430 and 900 go through the indexed path
(900's `.idx` is a real one: 72 indices, 69 triples, none carrying two, §19.3); 500 and 1180 through
RGB, because an indexed read is 20+ fade steps late and D15's coherence rule refuses those pairs.
`tests/endlogo.rs` checks that a run is in the reference's own numbering (`index_origin: boot`,
anchor 404); a `--block 5` run gives the same four frames.

## 8. The decode and the derivation's power

The decoded `PIC.UH` is byte-identical to the reference's held frame at 900, 0 of 128,000 pixels
(`the_fade_constants_are_derived_by_matching_the_reference_bit_for_bit`), which is why the derivation can render candidate
ramps over the port's own decode. The scan is discriminating: moving `FADE_IN_FIRST_INDEX` to 450
or `FADE_OUT_FIRST_INDEX` to 1145, dropping `GATE_ALIGNMENT_TICKS` or the previous-tick reading, or
taking `BLANKS_TICKS` to 31 each turns the part's tests red (a one-line edit in `endlogo.rs`,
then §9's first command). The fade-in truncates where the original does
(`c = 1, palette = 0` gives 62; `logo::tests::the_white_to_picture_ramp_truncates_where_the_original_truncates`).

## 9. Reproduce

```bash
cargo test -p secondreality --bin secondreality parts::endlogo
cargo test -p secondreality --test endlogo
cargo xtask rediff --only endlogo
cargo run --release --bin secondreality -- --headless --block 5 \
    --dump-frames 430,500,900,1180 --dump-dir out/endlogo-block5
cargo xtask compare --reference reference/parts/endlogo --rendered out/endlogo-block5
```

**Register.** `bugs.md` §4 and the index; A7, A12.
