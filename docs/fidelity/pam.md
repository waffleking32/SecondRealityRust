# PAM — the Praxis explosion

**Sources.** `PAM/OUTTAA.C` (67 lines) over `PAM/ASMYT.ASM` (322, `ulosta_frame` and the dead
`ruudulle`) and `PAM/COPPER.ASM` (87); assets `PAM/OUT.ANI` (the 42-frame delta stream),
`PAM/PAL.PAL` and `PAM/PRAX4.FLI` (the source the `.ANI` was packed from). **Port.**
`bins/secondreality/src/parts/pam.rs` (`"pam"`); its module doc holds the mechanism (the acts, the
two-page planar display, the n−2 delta base, the cadence, the palette washes). Integration test
`bins/secondreality/tests/pam.rs`. **Run.** Anchor **7710** (`shell::PAM_ANCHOR`,
`boot_anchor("pam")`). **Music.** Block 1's initial `_zplaysong(0)`, `MUSIC0`; a `--part pam` run
pre-polls 7,310 (§5).

## 1. Timeline

| boot | tick | source | what is on screen |
|---|---|---|---|
| (before 7710) | — | `OUTTAA.C:39-45` | gate open (code 10); the setup: near-white copy 63, mode set, `frame_count=0` |
| 7710-7711 | 0-1 | `:47` | the flat `(251,251,251)` wash; the counter reads 2, 3 |
| 7712 + 4(f−1) | 2 + 4(f−1) | `:46-64` | step `f` = 1..40: frame `f−1` into page `(f−1)&1`, flip, queue `wfade[f]` |
| 7760 | 50 | | step 13 |
| 7868 | 158 | | step 40, the last drawn frame (0-based 39) under `wfade[40] = 56` |
| 7872-7888 | 162-178 | `:49` | steps 41-45 draw nothing; `wfade[41..45] = 63` up from 7873: the white hold |
| 7888 | 178 | `:65` | step 45's tick: `close_copper`, and the part finishes |

Only 40 of the container's 42 frames are drawn (`:49`'s `if(f<=40)`). `wfade` is declared `[100]`
with 59 initialisers, and only entries 1-45 are indexed; `wfade[1] = 32`, so the opening white is
the pre-loop `tw_setpalette(&pal[768*63])`, not a step.

## 2. The screen

The mechanism (mode set, `ulosta_frame`'s plane-byte addressing, `PAGE_ORIGIN = [0, 0x4000 * 4]`)
is in the module doc. In the emulator's four-byte-group VGA memory a plane byte is four linear
pixels (`vga_memory.cpp`'s unchained handler; `reference-indices.md` §1.6), which is why the
surface index is `4*plane_byte + plane` here and in ALKU, U2A and U2E. CRTC 0x13 is never written,
which is why the reference's 7900 keeps an `.idx` at all. The view's `y_line_offset` is
`Some(320)`: the pages are flat 320-pixel rows inside the 640-wide surface (BEGLOGO, next, resets
it, `beglogo.md` §5). The decoder's n−2 base is cross-checked against the FLI, all 42 frames pixel
for pixel, in `crates/sr-assets/tests/anim.rs`.

## 3. The loop cadence

`copper2` increments `frame_count` once a retrace (`COPPER.ASM:60-81`), so `while(frame_count<4);`
is a four-tick wait. The port keeps the counter, increments it in slot 2 each tick, and checks it
at the top of each `frame()`; `exit_requested` is read before that check on every tick
(`exit_requested_while_waiting_ends_the_part_on_that_tick`). Step `f` lands at tick `2 + 4(f−1)`.

## 4. The anchor, and the cadence the support frames pin

**The anchor is 7710**, the first reference frame with PAM's content (`reference-indices.md` §6.1):
7700 is still U2A's landscape, 7710 is the wash, and §6.4 there records 7710 byte-identical in three
sessions, with 7709 and 7711 the same bytes in the recapture's leftovers.

The reference's support frames (`reference-indices.md` §6.3) force the mapping:

- 7759 and 7760 differ, and 7761 holds 7760. With a four-tick cadence the step boundary must fall
  on **7760**: at 7759, 7759 and 7760 would be one step (same frame, and the palette is copy 0
  there) and identical; at 7761, both would be the previous step. So step 13 starts at 7760.
- `T_f = T_1 + 4(f−1)` gives `T_1 = 7760 − 48 = 7712`. The anchor is two ticks earlier, so the
  counter reads **2** at 7710 (the setup's and the first wait's copper frames have elapsed):
  `FRAME_COUNT_AT_ANCHOR`.
- The rest of the record agrees (`reference-indices.md` §6.1, §6.3): `T_30 = 7828`, so 7829-7831
  are one held frame (the §6.3 triple); `T_40 = 7868`, so 7870 is frame 39 under `wfade[40] = 56`,
  the scan's `644acaf3…` sample; `T_41 = 7872`, so 7880-7920 are the white hold the scan measured.

**The 2 applies only to an anchored entry.** A run whose gate is open at its start because the
chain is continuous (block 1, after U2A) takes the source's ordinary path: the post-gate setup
(`OUTTAA.C:40-45`) zeroes `frame_count` on that tick, and the first step is four frames later.
`PartCtx::music_fresh` (D31) tells the two apart
(`a_mid_chain_start_with_the_gate_open_starts_the_counter_at_zero`); a run whose gate is still
closed holds the predecessor's screen until it opens
(`the_sync_gate_holds_until_code_ten_then_sets_up_and_waits_four_ticks`). In block 1, PAM's part
starts at chain 7,508 and its first frame by `reference-indices.md` §26.3's definition is 7,704,
against the original's 7,707 (`FULLRUN_SEAMS`; the convention is part-sequence.md (block seams /
pre-roll / mapping conventions)).

The mapping is not a claim that every intermediate frame is four ticks apart in the original:
`reference-indices.md` §6.1 measures the animation's window as (7705, 7920], 215 frames, which
includes the setup, the white hold and each `ulosta_frame`'s time. The filed keyframes and
support frames all match. One unfiled scan sample does not: the scan's 7720 hash `10bb3b95…` is the
port's 7721-7723 (§8's timeline dump), a one-frame phase difference in a sample from another
session.

## 5. The music: the entry pre-roll

`MusicStart::for_part("pam", Some(7710))` is `frame 7710, waiting 0, pre 7310`: the shell pre-polls
the player 7,310 times before the first tick, and the engine's tick polls once more before the part
reads `dis_sync()`. `the_block_one_pre_roll_puts_the_player_at_the_traces_7710_position` polls the
real `MUSIC0` and asserts:

| polls | order | row | zframe |
|---|---|---|---|
| 7,304 | 13 | 0 | 5004 |
| 7,305 | 13 | 1 | 5004 |
| 7,310 | 13 | 1 | 5008 |
| 7,311 (the engine's tick) | 13 | 1 | 5009 |

`(13, 1)` is `sync_from`'s code **10**, so the gate opens on the first tick. The trace agrees on
the row (`awk -F, '$1==7705 || $1==7710' reference/music/second1b/trace.csv`):

```text
7705,13,1,13,0,6,120,0,0,5005
7710,13,1,13,0,6,120,0,0,5008
```

The `zframe` sampling boundary differs (the trace samples at the frame, the port after a tick's
poll), so the two may sit one tick either side; the published row, which the gate reads, agrees.
The catch-up is longer than the player's sample ring, which drops its oldest samples; the position
does not depend on the ring. A block-1 chain applies no catch-up.

## 6. Verification, and 7900's index plane

| index | plane | port vs reference | register |
|---|---|---|---|
| 7710 | RGB | **0 of 64,000** | exact |
| 7760 | RGB | **0** | exact |
| 7830 | RGB | **0** | exact |
| 7900 | RGB | **0** | exact |
| 7900 | indices | **60,375** (max delta 194) | A21 |

Pinned by `tests/pam.rs` (with D19's support frames) and `cargo xtask rediff` (entry `pam`;
`fullrun/02-u2a-pam` for the block-1 run's 7707 and 7880, A53). The record's relations hold:
7711 = 7710 (7709 is unreachable from the anchor); 7759 ≠ 7760 and 7761 = 7760; 7829 = 7830 = 7831;
and the port's own 7899 and 7901 are the same white as 7900. The port's hashes match
`reference-indices.md` §6.3's entry for entry: 7759 `23fa5096…`, 7761 `5d558081…`, 7829/7831
`e1967ed2…`; the keyframes are `1fef46de…`, `5d558081…`, `e1967ed2…`, `1fef46de…`.

**7900's `.idx` is not PAM's screen** (A21, registered). The reference's plane is 64,000 bytes of
index **15**, and its manifest's entry 15 is `[62,62,62]`, PAM's copy 63 widened to 251. Index 15
is BEGLOGO's white field (`BEG/BEG.C:18-21`), and 7900 is inside the window where the indexed read
runs late (D15's read costs about twenty frames): BEGLOGO's field is in video memory while PAM's
copy 63 is still the DAC, before BEGLOGO's 320×400 mode change in (7920, 7930]
(`reference-indices.md` §6.5). The coherence check cannot see this on a collapsed palette
(`reference-indices.md` §1.6). The port's plane is the last frame PAM drew, 0-based 39, which
`the_7900_index_plane_is_the_next_parts_white_field_not_pams_frame` asserts equals `OUT.ANI`'s
frame 39; `cargo xtask compare` reports it as `94.3359 %`, max delta 194 (the frame's highest index
209 less 15).

## 7. Preserved behaviours

- **The FLI header counts 41, the container holds 42, the loop draws 40** (`VFLI.C:66`;
  `asset-inventory.md` §E.5; `the_fli_header_counts_forty_one_and_the_container_forty_two`).
- **`ruudulle` is dead** (`ASMYT.ASM:207-320`): not ported.
- **Steps 41-45 draw nothing** and only re-queue copy 63: the closing white hold (`bugs.md` §11).

`OUT.ANI` and `PAL.PAL` are read in their original formats (`load_frames(Format::Ani)`,
`sr_assets::pal::decode`); the decoder's malformed-input behaviour and two format choices are A3
and A4.

## 8. Reproduce

```bash
cargo test -p secondreality --bin secondreality parts::pam -- --nocapture
cargo test -p secondreality --test pam -- --nocapture
cargo xtask rediff --only pam

# the port's dump against the record (§6's hashes and compare figures)
cargo run --release --bin secondreality -- --part pam --headless \
    --dump-frames 7710,7711,7759,7760,7761,7829,7830,7831,7899,7900,7901 \
    --dump-dir out/pam-port
sha256sum out/pam-port/pam_00*.rgb
cargo xtask compare --reference reference/parts/pam --rendered out/pam-port \
    --out out/pam-port-diff

# the timeline around the anchor (§4's scan samples)
cargo run --release --bin secondreality -- --part pam --headless \
    --dump-frames "$(seq -s, 7710 7930)" --dump-dir out/pam-timeline
sha256sum out/pam-timeline/pam_007720.rgb out/pam-timeline/pam_007721.rgb \
          out/pam-timeline/pam_007870.rgb
```

**Register.** `bugs.md` §11 (the 40-frame loop), with the dead `ruudulle` in the index.
