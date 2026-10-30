# RAYSCRL — the raytraced mirror-ball scroll

Sources: `WATER/DEMO.PAS` (190 lines) over `WATER/ROUTINES.ASM` (41 lines); assets `BKG.CLX`,
`MIEKKA.SCI` and `WAT1-3.DAT`. Port: `bins/secondreality/src/parts/rayscrl.rs` (`"rayscrl"`); its
module doc has the acts, the colour-keyed blitter, the font refill and the preserved bugs. Run:
mapping **`boot = 12962 + tick`** (no boot anchor). Music: `MUSIC1` seeked to order 86, pre-roll
268 polls (D31).

## 1. Timeline

One tick is one `waitr` (`dis_waitb`). Tick 0 is the first poll of the init wait, the tick the
reference's published order 88 row 0 first stands at (`reference/music/second3b/trace.csv`, frame
12962).

| tick | boot | source | on screen |
|---|---|---|---|
| 0-6 | 12962-12968 | `DEMO.PAS:98-104` | the init wait's seven confirming polls; nothing drawn |
| 7 | 12969 | `:107-113` | the black DAC and the 64,000-byte `BKG.CLX` blit |
| 8-134 | 12970-13096 | `:117-132` | the 127 fade-in iterations |
| 135-653 | 13097-13615 | `:135-143` | the stage-2 spin; nothing drawn |
| 654 | 13616 | `:155-186` | the main loop's first `scr` |
| 1988 | 14950 | `:155-166` | the cue (`musplus == -11` confirmed); the fade-out's first upload |
| 1988-2052 | 14950-15014 | `:167-178` | the 65-step fade-out; the part retires at boot 15014 |

## 2. The mapping

**`boot = 12962 + tick`, a mapping, not an anchor.** `shell::boot_anchor("rayscrl")` stays
`None`: the part's first drawn frame is the measured window `(12900, 13000]`
(`reference-indices.md` §15), and a `--part rayscrl` run numbers its own frames from zero
(`index_origin: "part"`, `anchor: null`;
`the_run_is_numbered_in_its_own_frames_and_draws_320x200`). The conventions are in part-sequence.md
(block seams / pre-roll / mapping conventions).

Four points pin it:

| boot | tick | what pins it |
|---|---|---|
| 12969 | 7 | the 13000 keyframe: the fade's k=14 hold is 31 ticks after the blit, and the reference's 13000 is exactly the port's tick 38 (rediff's 13000 row) |
| 13616 | 654 | the main-loop font phase: the reference's 14000 is the port's 384th main-loop draw, and the stage-2 confirmation puts the first draw at 13616 |
| 14000 | 1038 | the font phase above, and the reference frame |
| 14500 | 1538 | the same, 500 draws later |

The port's clock is the copper's 70.086 Hz tick, one `waitr` an iteration, so ticks and boot frames
advance 1:1 over the keyframes' span; the music trace's frame labels and the drawing labels agree
within a frame at all four.

## 3. Decisions

- **The gates confirm for seven ticks** (D24, A18): §4. `Rayscrl::confirm_ticks` keeps the tree's
  immediate condition as the measured counterfactual.
- **`MIEKKA.SCI` is the tree's asset**, not the release's patched strip (§6). Patching bytes the
  release alone holds would stop the read being an original-format one.
- The preserved bugs (the one-band-down refill, the 5,373-byte `move`, the `scp < 390` stop, the
  `pf := 0 to 3` fade loops, the `pal[768]` alias, the fade-out's source, the exact `-11` cue) are
  `bugs.md` §19; the fade's measured consequences are §5.

## 4. The music: the seek, the pre-roll and the gates

**The seek is order 86.** `reference/music/second3b` holds `MUSIC1` order 86 (pattern 60) from frame
12704 through 12961 and order 88 (pattern 35) from 12962. The window `(12900, 13000]` opens inside
order 86, so `shell::music_for_part` returns `(MUSIC1, 86)`. Playing through the `254` marker at
order-list entry 87 is what sets `zplus` bit 1; a seek straight to 88 leaves `zplus` 0 and the
init's `musplus >= 0` gate never passes (module doc).

**The pre-roll is 268 polls.** `play_song(86)` publishes order 88 row 0 on its 269th poll; tick 0 is
that poll (`the_music_pre_roll_puts_the_reference_state_at_tick_zero`). The published position at
each mapped tick is the trace's own
(`the_music_is_the_block_three_position_at_every_keyframe`):

| boot | tick | dump | trace |
|---|---|---|---|
| 13000 | 38 | `88:4` | `88:4` |
| 13500 | 538 | `89:2` | `89:2` |
| 14000 | 1038 | `90:0` | `90:0` |
| 14500 | 1538 | `90:62` | `90:62` |

A mid-chain start skips the pre-roll (`PartCtx::music_fresh`; part-sequence.md (block seams /
pre-roll / mapping conventions)). In the block-3 chain the player stands at 86:62 inside the gate
and the gate opens on 88:0 at chain 12,546, the trace's boot 12,962
(`a_block_three_run_is_the_whole_chain_and_its_mid_chain_entries_do_not_pre_roll`;
`a_mid_chain_start_does_not_pre_roll_the_music` is the unit case).

**The release's gates act seven ticks after the tree's condition** (A18). The tree's three waits
fire on the first tick their condition holds; the release's, measured, leave seven ticks later on
the port's clock (the cue's trace labels are eight, because the trace's player publishes that state
one poll before the port's does):

| gate | tree's condition first holds | release leaves |
|---|---|---|
| init (`musplus >= 0`) | boot 12962 (the published 88:0) | the blit at 12969 |
| stage 2 (order ≠ `co`, row ≥ 16) | boot 13609 (the published 89:16) | the main loop's first draw at 13616 |
| cue (`musplus == -11`) | boot 14942 (the trace; the port's player 14943) | the fade's first upload at 14950 |

One rule reproduces all three: `GATE_CONFIRM_TICKS = 7`, the condition read true on seven
consecutive ticks and the gate acting on the tick after
(`the_releases_confirmation_starts_the_main_loop_seven_ticks_after_the_gate`). With the tree's
immediate condition the main loop starts at boot 13610, six draws early, and 14000 and 14500 differ
by **8,375** and **11,958** pixels (`the_trees_immediate_gate_draws_the_keyframes_early`).

**The cue is exactly -11.** `-10` and `-12` never trigger it
(`minus_ten_and_minus_twelve_never_trigger_the_cue`); a scripted `-11` fades over 65 uploads and ends
the part (`the_cue_is_exactly_minus_eleven_and_fades_sixty_five_ticks`).

## 5. The fade, measured

The fade-in is 127 iterations (`for y := 0 to 63*2`), uploading and stepping on odd `y`, and the
`pf := 0 to 3` loop steps the next entry's red as well (module doc, `bugs.md` §19). Two consequences
show in the reference:

- **At the k=14 upload** the displayed red is `2k+1 = 29` and green/blue are 14. That is the
  reference's 13000 frame byte for byte (rediff's 13000 row; the frame is 35 colours, mean 23.15,
  `reference-indices.md` §15.1).
- **The last upload is one step short on the seven entries whose blue target is 63.** The 63rd
  upload (y=125) displays those blues at 62 and then steps the stored array to 63. The reference's
  13500 palette (`reference/parts/rayscrl/manifest.json`) is MIEKKA's with exactly **seven** blue
  channels at 62 and no other channel short
  (`the_fade_in_is_one_hundred_twenty_seven_iterations_and_pins_tick_zero`).

**The fade-out's first upload** (`:167-178`) uploads `pal`, the array the fade-in left, so it moves
both ways: every entry with red > 0 is one step down on red (entry 39: 53 → 52) and the seven blue-63
entries one step up on blue (entry 39: 62 → 63). No 14950 keyframe is filed; the frame comes from
the survey's partner capture (`reference-indices.md` §15.4's second command, frame list
`14950,15000,15050`). The port's first upload lands on the same boot frame and matches the capture's
palette at j = 1; the frame differs in **1,061 pixels**, in rows 0–4 and a few in 36–45, the same
release-versus-tree `MIEKKA.SCI` difference as §6, at the strip the 390 cap has scrolled in. This is
a one-time measurement; the command above reproduces it.

**The `pal[768]` alias** (`pf = 3` at x=255) reads `pal[0]`, which stays 0 with entry 0 black, so the
font is never touched (`the_fade_loops_step_the_next_entrys_red_and_read_the_byte_past_the_array`).

## 6. Verification

| keyframe | tick | plane | differing | register |
|---|---|---|---|---|
| 13000 | 38 | RGB | 0 | exact |
| 13500 | 538 | RGB and `.idx` | 0 | exact |
| 14000 | 1038 | RGB | 3 px (0.0047 %) | A18 |
| 14500 | 1538 | RGB | 42 px (0.0656 %) | A18 |

The counts are the `rayscrl` rows of `xtask/src/rediff/table.rs`. `tests/rayscrl.rs` drives the
binary over the support frames D19 requires: every neighbour differs from its keyframe except the
13000 upload/hold pair (tick 37 = tick 38, the reference's own 64-pixel capture artefact) and the
held 13500's pair. `cargo xtask compare` is not the tool: it pairs by manifest index, and this run's
tick 38 is not index 13000.

**The residual is an asset difference.** Every differing pixel at 14000 and 14500 is in the
font-overlay region the strip has scrolled into (row 0 columns 2-4; rows 0-5, columns 3-27), and
draws from `MIEKKA.SCI`. At five sampled source positions the reference's byte is the tree's
*previous* source column (`font[15*400+34]` for `+35`, `font[14*400+201]` for `+202`,
`font[14*400+202]` for `+203`, `font[13*400+203]` for `+204`, `font[15*400+201]` for `+202`): the
release ships a patched or re-rendered strip. The five pairs are recovered from the two keyframes'
differing pixels (`the_keyframe_residue_is_the_releases_font_strip`). The tree's `MIEKKA.OBJ` carries the file's bytes exactly
(`the_trees_miekka_object_carries_the_files_bytes`), so the port reads what the tree links.

The assets, as `the_assets_are_read_the_way_the_source_reads_them` checks them:

| asset | bytes | use |
|---|---|---|
| `BKG.CLX` | 64,778 | RIX3 320×200; picture at 778 (its palette is unused) |
| `MIEKKA.SCI` | 14,778 | RIX3 declaring 400×250, carrying 35 rows; palette at 10 (64 entries differ from BKG's), the first `400*34` bytes at 778 |
| `WAT1-3.DAT` | 23,096 / 23,034 / 27,818 | 5,372 columns each; 6,176 / 6,145 / 8,537 positions (20,858); largest offset 63,999 |

## 7. Reproduce

```bash
cargo test -p secondreality --bin secondreality parts::rayscrl
cargo test -p secondreality --test rayscrl
cargo xtask rediff --only rayscrl
```

**Register.** `bugs.md` §19; A18.
