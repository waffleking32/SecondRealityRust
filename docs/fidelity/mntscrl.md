# MNTSCRL — the mountain scroller

Sources: `FOREST/READ2.PAS` (224 lines) over `FOREST/ROUTINES.ASM` (39 lines); assets
`HILLBACK.CLX` (`BGR.PAS:2-6`), `POS1-3.DAT` (`AOS1-3.PAS`) and `O2.SCI` (read at run time,
`:110`). Port: `bins/secondreality/src/parts/mntscrl.rs` (`"mntscrl"`); the module doc has the
acts, the blitter, the tick map and the preserved bugs. Block 3's first part
(`MAIN/U2.ASM:856-866`) and block 1's after PANICEND. Run: mapping `boot = 419 + tick` (§2);
music MUSIC1 order 42 from the block-3 restart, entry pre-roll 11 polls (§6). Reference:
`reference/parts/mntscrl`, 500 (`.idx` and `.rgb`), 1100, 1700, 2300 (`reference-indices.md`
§11).

## 1. Timeline

One tick is one `waitr` (`dis_waitb`); tick 0 is the code before the first wait.

| tick | boot | source | what |
|---|---|---|---|
| 0 | 419 | `:101-123` | mode 13h, the loads, the `+128` bias, the black DAC, the background blit |
| 1..64 | 420..483 | `:125-142` | the 64 fade-in uploads; tick 64 also `:144-151` (palette swap, font build) |
| 65..135 | 484..554 | `:153-159` | the release's 71-tick first wait, nothing drawn (§6) |
| 136..262 | 555..681 | `:165-182` | the 127-frame pre-roll: `scr(sss)`, the band ramp on odd frames |
| 263.. | 682.. | `:193-224` | the main loop, one `scr` a tick |
| 1,979 | 2,398 | `:196-203` | the `musplus == -11` cue: the fade's full-palette upload |
| 2,043 | 2,462 | | the 65-tick fade ends; the run holds black |

## 2. Mapping: `boot = 419 + tick`

The reference's own fade pins tick 0. `reference-indices.md` §11.1 records the frame means
6.08 (425), 8.91 (430), 10.25 (435) and 10.53 (440); the port's curve `min(step, tmppal)` puts
them on steps 5, 10, 15 and 20, displayed at `tick 0 + step + 1`. All four give 419
(`the_fade_in_pins_tick_zero_to_boot_419`). A `--part mntscrl` or `--block 3` run numbers its
own frames from zero (`a_block_three_run_starts_at_mntscrl_with_its_own_numbering`); the
manifest rule is `part-sequence.md` §10 (mapping, pre-roll and block-seam conventions), and
`--block 3`'s chain tick `t` is boot `419 + t`. In a full run the part's first frame is a
`FULLRUN_SEAMS` row (16,637 in the original, 16,633 in the port; A53).

## 3. The font and the pre-roll

The pre-roll makes 42 `scr(2)` shifts (`scp` 133 → 175) and re-uploads entries 0..176 on odd
frames (`the_pre_roll_uploads_on_odd_frames_and_ramps_entries_zero_to_176`). The packed 31×237
font is one buffer because the source's `move` (`:86`) shifts across row boundaries and the
append (`:87`) overwrites the seam.

## 4. The palette

The pre-roll's last odd iteration uploads step 62 and then increments to 63, so a channel
whose target is 63 is displayed at 62 until the fade-out's first upload. Entry 151 is the one
such channel in the bands; the pre-roll test pins it, and the shortfall is the source's
(`bugs.md` §15). The fade-out uploads before each decrement, so its first tick shows entry 151
at 63 and its 65th uploads black (`the_cue_is_exactly_minus_eleven`,
`the_fade_out_is_sixty_five_ticks_and_ends_the_part`).

## 5. The assets and the substitutions

| asset | bytes | what the part uses |
|---|---|---|
| `O2.SCI` | 21,258 | RIX3 640×32; rows 0..30, 640 bytes each, at 778 |
| `HILLBACK.CLX` | 64,778 | RIX3 320×200; palette at 10, picture at 778 |
| `POS1-3.DAT` | 25,088 / 25,064 / 25,114 | 7,347 columns each, 15,592 positions in all |

`the_assets_are_read_the_way_the_source_reads_them` compares every `O2.SCI` byte after the
bias, the palette against the file's bytes 10..777, the background against bytes 778..64,777,
and the lists' column count and positions. The largest destination offset in any list is
63,870, inside the picture, so no offset reaches the 64 KB wrap.

- `O2.SCI` and `HILLBACK.CLX` are read through the RIX3 decoder, not by raw `seek`s; the
  payloads are the bytes at offsets 10 and 778.
- The `hback` rebase (`:121`) is not performed: `background` is the picture's 64,000 bytes,
  and `pos_list` refuses any offset past them, so the 778-byte tail the rebase leaves standing
  is never read (the largest offset is 63,870).
- The fades, the ramp, the shift and the additive blit are the source's arithmetic.

## 6. The music and the release's first wait

**The head start.** The block-3 restart starts MUSIC1 at order 42 on the music clock's first
tick, frame 408 (`shell::MUSIC_START_FRAME`). With tick 0 at boot 419 the reference's player
had been polled **11 times** before the part's first tick, so `Mntscrl::init` pre-polls 11 on a
fresh start (`tick_zero_is_the_background_through_a_black_dac`). At tick 81 (boot 500) the
published position is `42:10`, as `reference/music/second3/trace.csv` holds at frame 500
(`the_music_is_eleven_polls_in_at_the_parts_first_tick`); without the pre-roll it would read
`42:8`, the trace's frame 490. A mid-chain start (block 1) does not pre-roll
(`a_mid_chain_start_does_not_pre_roll_the_music`; the rule is `part-sequence.md` §10.2).

**The first wait is the release's 71 ticks (A17).** The tree's `:153-159` waits for
`dis_musplus() >= 0`. The release's last held frame is boot **554** and its pre-roll's first
`scr` is boot **555**, ticks 135 and 136. The trace's position is `42:16` on both sides of that
boundary (`zplus` 0, `musplus` -32 at both; the port publishes `42:16` at ticks 134 and 135,
`the_releases_first_wait_is_seventy_one_ticks`), so any `musplus` threshold either ends the
wait at tick 65 or never at 135. The port runs `RELEASE_WAIT_TICKS = 71` (ticks 65..135), D24's
single-constant case.

The tree's `musplus >= 0` would first hold at trace frame **2488** (order 47 row 0, `zplus` 2),
tick 2,068 under the port's one-frame lead, and its main loop would start at tick 2,196. The part
would still hold the opening at 1100, 1700 and 2300, **118,843 / 118,924 / 119,056 bytes** away
(`the_trees_own_wait_would_hold_through_the_scroll_keyframes`).

**The cue.** The tree's `:196-203` cue, `dis_musplus() == -11` exactly, first fires after the
main loop begins at order 45 row 53, tick **1,979**: that frame is the fade's full-palette
upload and 1,980 is one step down. 2,050 and 2,100 are black
(`the_real_players_cue_fades_the_part_out_and_the_run_holds_black`).

## 7. The `scp` bound: the release's 640 (D24, adopted)

The tree's `:88` is `if scp < 639 then inc(scp)`; the release reads one column further. The cap
is first reached at tick **1,656**, the 507th `scr(2)` of the main loop: tick 1,655 reads 639
and 1,656 reads 640. Boot 2300, 225 ticks on, is the first keyframe after it. Under 639 it
differs in **401 bytes / 146 pixels**, and under 640 in 0; 1100 and 1700 are before the bound
(`the_releases_scp_bound_is_sixty_forty_where_the_tree_says_639`, which runs the tree's 639 by
clamping `scp`). The tree's own `fbuf` is `array[0..640*31]`: index 19,840, one past the 31
loaded rows, is the element a stride-640 read at column 640 needs for row 30, and `:119`'s loop
covers it. The extra element exists for this read, so 640 is the array's own bound. This is
D24 step 3's single-constant branch.

## 8. Decisions

- **The `fbuf` overrun** (`:119`): the bias loop covers index 19,841, which in the original is
  `font[0]`, still zero, so the increment is a no-op; the port biases the array alone
  (`apply_bias`).
- **The dead `veke` bound** (`:3`, `:221-224`): `inc(frame)` is commented out, so
  `frame = veke` never fires; the port has no `frame` counter and runs until the cue.
- **The forced-black bands and their ramp** (`:126-127`, `:146-147`, `:165-178`) are kept
  (§4).
- **The exact cue** (`:200`): -10 and -12 keep the part scrolling; only -11 starts the fade
  (`the_cue_is_exactly_minus_eleven`).
- **The first wait honours `dis_exit`** although `:153-159` does not test it: the port does
  not spin, so an exit ends the wait on the tick it is seen
  (`exit_requested_ends_the_wait_on_that_tick`). The source's wait would hold an ESC until the
  tree's `musplus >= 0` gate (trace frame 2488). Registered as A62.
- **The first wait's 71 ticks** (§6, A17) and **the `scp` bound 640** (§7, D24).

## 9. Verification

| keyframe | tick | plane | differing px | register |
|---|---|---|---|---|
| 500 | 81 | `.idx` | 0 | exact |
| 500 | 81 | RGB | 0 | exact |
| 1100 | 681 | RGB | 0 | exact (A17's wait) |
| 1700 | 1281 | RGB | 0 | exact (A17's wait) |
| 2300 | 1881 | RGB | 0 | exact (A17's wait, D24's `scp`) |

The rows are the `mntscrl` entry of the rediff table (`xtask/src/rediff/table.rs`); the
reference files no `.idx` for the three scroll frames. D19's support holds:
`each_scroll_keyframes_neighbours_differ_from_it` shows `K-1` and `K+1` differ from `K` at every
scroll keyframe (the scroller moves 2.4–3.1 % of its pixels a frame), so the mapping is exact.
`cargo xtask compare` does not apply: it pairs by manifest index, and the run is numbered from
zero.

Reproduce:

```bash
cargo xtask rediff --only mntscrl
cargo test -p secondreality --test parts mntscrl::
cargo test -p secondreality --bin secondreality mntscrl::tests
```

## 10. Open items

- **The 71 ticks' mechanism** (A17): one reference run cannot tell a constant from a
  release-side condition the frames do not expose. The condition, if there is one, is in the
  release's `MNTSCRL.EXE` image (D49), not yet read.

**Register.** `bugs.md` §15 (the `fbuf` byte, the bands, the shortfall, the exact cue), with the
dead `veke` bound in the index.
