# ALKU — the intro text and the horizon pan

**Sources.** `ALKU/MAIN.C` (542 lines) over `ALKU/ASMYT.ASM` (110, the two blitters),
`ALKU/COPPER.ASM` (149, the three copper routines) and the shared `ALKU/TWEAK.ASM` (333). Assets
`assets/ALKU/HOI.U` (with three palette entries overridden, §7) and `FONA.UH`. **The port runs the
shipped build's credits loop and closing routine**, read from the unpacked `MAIN/DATA/ALKU.EXE`
(D49, §12). **Port.** `bins/secondreality/src/parts/alku.rs` (`"alku"`); its module doc holds the
mechanism (the acts, the 320×400 display from a 640×200 picture, the copper slots, the tick, the
preserved bugs). Integration test `bins/secondreality/tests/alku.rs`. **Run.** Anchor **404**
(`boot_anchor("alku")`); `MUSIC0` at order 0 under block 1's initial `_zplaysong(0)`.

## 1. Timeline

One tick is one copper frame (`COPPER.ASM:85`), so boot = 404 + tick.

| boot | tick | source | what is on screen |
|---|---|---|---|
| 404-1536 | 0-1132 | `MAIN.C:53-59` | black (`fade1`), the picture already in video memory |
| 1537 | 1133 | `:59-64` | code 1: card 1, 64-step fade-in, `wait(300)`, 64-step fade-out (428 ticks) |
| 2097 | 1693 | `:66-70` | code 2: card 2 |
| 2509 | 2105 | `:72-77` | code 3: card 3 (`in` and the two logo pictures) |
| 3069 | 2665 | `:79-85` | code 4: the picture's 128-frame fade-in while the window steps |
| from 3069 | | `:87-146` | the pan, one step every 9 ticks, and five text batches gated on codes 5-9 |
| 5833 | 5429 | release `0x136c` | the fifth (empty) batch ends at `a=308`; the pass then spins (§12) |
| 5949 | 5545 | release `0x1380` | the loop's last step, `a=320→321`; no final fade |
| 5949-5952 | | release `0x13ce` | the closing routine: last window, snap, compacted 320×200 page, linear page; the part finishes on 5952 |

The codes are where `sync_from` first answers each over `second1` (the shell's block-1 test
prints them). A block-1 run ends three ticks later (its codes are 2-4 frames late, §2): ALKU's
320×200 frames are 5954-5955, and U2A starts at **5956** on the linear page's RGB (`0a68ffb9…`).

## 2. Mapping and music

The anchor is the boot's first graphics frame in five runs of five (`reference-indices.md` §1.12,
D17), and `SECOND 1` enters ALKU there (`MAIN/U2.ASM:790-792`). A `--part alku` and a `--block 1`
run are the same run (`a_block_one_run_starts_at_alku_and_is_the_same_run` compares their frames
byte for byte); the manifest says `index_origin: "boot"`, `anchor: 404`, `MUSIC0` at order 0.

The block-1 player starts at `BLOCK_ONE_MUSIC_START_FRAME` (400), so a run drawing from 404
pre-polls four frames; the derivation of 400 is part-sequence.md (block seams / pre-roll / mapping
conventions). `a_block_one_run_starts_under_the_initial_play_song_and_matches_second1` compares a
block-1 run with the real player against `second1` frame by frame: all **638** position changes are
visited in the trace's order, and the port's change frames minus the trace's are

```
{-2: 1, -1: 1, 0: 3, 1: 1, 2: 217, 3: 285, 4: 118, 5: 10, 6: 1, 8: 1}
```

2-4 frames late for 620 of them (the rest are the trace's own missing frames and sample instant,
A12). The code transitions ALKU reads are 2-4 frames late too:
`{1: 2, 2: 3, 3: 2, 4: 3, 5: 2, 6: 4, 7: 3, 8: 4, 9: 3}`.

## 3. Decisions

- **D24, adopt the release's three palette bytes** (§7): the linked `hzpic`'s entries 1-3 over
  `HOI.U`'s. With them every other difference is accounted for.
- **D49, run the release's loop and closing routine** (§12, A20). Where the image differs from
  `MAIN.C`, the image wins; the sections below say which build they describe.
- **The spin's `f` is the frames' answer.** The release's `f` when the spin ends is CPU time; the
  port takes what the frames show, no final fade (`SPIN_LEAVES_F_ABOVE_THRESHOLD`, A20).
- **The closing page is shown doubled from its first frame** (§12, A20): the hardware's behaviour,
  not the emulator's half-frame-late CRTC 9.
- **The exit path skips the closing calls** (§6, A62).

## 4. The display, the copper and the tick

All three are in the module doc. Facts the comparison rests on: the AC pel-panning unit is half a
pixel, so the port's `pan = cop_scrl/2` (`cred.md` §3); the oracle refuses this screen's `.idx`
because 176 × 400 = 70,400 bytes exceeds one plane's 65,536 (`reference-indices.md` §1.6), so every
ALKU comparison is RGB and the DAC's 6→8-bit widening is load-bearing.

## 5. The gates and the text's off-by-one

The card gates are `dis_sync() < 1`, `< 2`, `< 3`, `< 4` (`MAIN.C:59`, `:66`, `:72`, `:79`), and
`dis_sync()` is `sync_10`'s `ordersync1` answer (`bugs.md` §3). The batches wait
`while(((a&1) || dis_sync()<4+tptr) && !dis_exit() && a<319)` (`:138`); `tptr` is post-incremented
by `switch(tptr++)` (`:102`), so the first batch waits for code **5**, not 4
(`the_picture_fade_hands_over_to_a_batch_gated_on_code_five`; `bugs.md` §10).

The `f` chain (`:89-99`) counts outer iterations, one 9-tick `do_scroll(1)` each: `f==0` starts the
`textin` fade and adds 20, `f==50` starts `textout`, and a batch runs once `f>50` and
`cop_dofade==0`. The empty case 4 (`:129-131`) calls only `ffonapois`, and `fmaketext` runs when
`aa < 304` in the release (`aa < 308` in `MAIN.C:141`).

## 6. The final fade and the exit path

- **The final fade only when `f>7`** (`:148`; `63/SCRLF` is 7). It touches only entries 64-255,
  which the picture never indexes, and `fonapois()` (`:150`) runs after either branch. In the
  release the loop ends on the spin and the port runs no fade (§3); a loop that ends on a blocking
  pass with `f>7` runs the 64 steps
  (`the_final_fade_runs_sixty_four_ticks_when_the_blocking_loop_leaves_f_above_seven`).
- **The release's last refresh** is `a=320`'s: columns 664-667 (the next row's 24-27), written to
  byte columns 166 and 254, neither inside the last window (bytes 80-159) nor the closing page.
- **The exit path** (A62). The port returns `Finished` on the tick that sees
  `ctx.exit_requested`, without drawing. The release's exit path runs neither the final fade nor
  `fonapois`: it shows one black frame and `exit(1)`s (§12), so the port lacks only that frame.
  (`MAIN.C:147-152` would run the closing calls after either ending of the loop, but its `dofade`
  runs zero steps on an exit, because its loop tests `!dis_exit()`, `:306`.)

## 7. The assets

**The picture.** `hzpic` is the linked `HOI.IN0+IN1` (`ALKU/INCLUDE.ASM:7-12`), which differs from
`HOI.U` in exactly nine bytes, palette entries 1-3 (`asset-inventory.md` §E.18):

| entry | the linked hzpic (shipped) | `assets/ALKU/HOI.U` |
|---|---|---|
| 1 | `(10, 10, 10)` | `(2, 2, 3)` |
| 2 | `(30, 30, 30)` | `(5, 5, 7)` |
| 3 | `(63, 63, 63)` | `(6, 6, 9)` |

The picture bytes are identical, so the port reads `HOI.U` and overrides the three entries
(`the_picture_is_hoi_u_with_the_three_entries_the_release_ships`). The counterfactual, through the
same renderer (`the_curated_palette_entries_leave_the_reference_different`, ignored, §9):

| keyframe | `HOI.U`'s own entries 1-3 | the shipped entries |
|---|---|---|
| 900 | 0 of 128,000 | 0 |
| 1719 | 1,595 | **0** |
| 2719 | 4,171 | **0** |
| 5859 | 966 | **0** |

The cards are the strong evidence: their text is drawn in `fade2`'s entries 64, 128 and 192, which
are palette entries 1-3 (`MAIN.C:194-208`), so the two palettes give visibly different cards.

**The font.** `ALKU/INCLUDE.ASM:17` includes `fona.inc`, which is `FONA.UH[783:]`, one byte before
the decoder's pixel area at 784 (`asset-inventory.md` §E.8). The port builds `sr_assets::font::Font`
from the decoded 1500×30 strip. The shift is invisible: both `[783:]` and
`[784:]` hold the same glyph bytes in all 76 runs (byte 783 is zero). `MAIN.C` reads 32 rows
(`:218`, `:225`) of the 30-row strip; the release reads 29 (§12), and its two padding rows are zero
in the image (A9, closed).

## 8. Verification

All four reference frames are `.rgb` only (§4).

| index | what is on screen | port vs reference |
|---|---|---|
| 900 | the black screen | **0 of 128,000** |
| 1719 | card 1 mid-hold | **0** |
| 2719 | card 3 mid-hold (`in` over the Dolby logos) | **0** |
| 5859 | the scrolled picture | **0** |

The held frames' `K-1`/`K+1` support frames are the same bytes. 5859 is the moving frame, but the
run's 5858 and 5860 are the same step (the window moves every nine frames), as the reference's
session holds 5859-5861 (`reference-indices.md` §4.3); `tests/alku.rs`'s
`the_support_frames_hold_as_the_references_do` asserts every pair. Pinned by `cargo xtask rediff`
(entry `alku`, all four at 0; `fullrun/01-alku`'s 1719 at 0).

5859 is bare whatever the batches' phase: once the fifth batch's `ffonapois` has cleared the text,
the release's `do_scroll(0)` passes do not carry it back (§12). `the_5859_phase_sensitivity`
(ignored) shifts the batches' transitions and prints **0** for every shift from −60 to +45 frames
and **2,065** from +46, where the fourth batch's text is still up. The batches' phase itself is
A20's: a dense block-1 capture, not filed under `reference/`, shows that session's text fading
earlier than the port's from the second batch on, at fade edges only.

## 9. Reproduce

```bash
cargo test -p secondreality --test alku -- --nocapture          # §8's support frames
cargo xtask rediff --only alku                                  # §8
cargo test -p secondreality --bin secondreality \
    shell::music::tests::a_block_one_run_starts_under_the_initial_play_song_and_matches_second1 \
    -- --nocapture                                              # §2's distributions
cargo test -p secondreality --bin secondreality \
    parts::alku::tests::the_curated_palette_entries_leave_the_reference_different \
    -- --ignored --nocapture                                    # §7's counterfactual
cargo test -p secondreality --bin secondreality \
    parts::alku::tests::the_5859_phase_sensitivity -- --ignored --nocapture   # §8
cargo test -p secondreality --bin secondreality parts::alku::tests::the_release   # §12
cargo xtask music analyse --trace reference/music/second1
```

## 10. Open items

- A20's batch phase and the spin's `f` (§11).

## 11. What is not claimed

- The original's CPU time inside `fmaketext`, `faddtext` and `prt`, and the spin's: that is A20's
  batch phase, seen at text-fade edges, and the spin's `f`.
- The 5859 picture is reproduced at the reference's pan (`a=309` displayed); the reference's
  sessions differ by 1-2 whole pixels of pan (`reference-indices.md` §4.4).
- The font rows the tree's routine reads past the strip are not drawn: the release's image holds
  zeros there (A9, closed).

**Register.** `bugs.md` §10 (the palette alias, the row-spill refresh, the `tbuf` clear, the code-5
gate); `dis_sync` is §3.

## 12. The release's build (D49)

`cargo test -p sr-assets --test exe -- --ignored dump_the_unpacked_images` writes
`out/release-images/ALKU.img`, the 343,472-byte load image of `MAIN/DATA/ALKU.EXE` (PKLITE small
over LZEXE 0.91; Borland large; DGROUP `0x50fb`). `main` is at image `0x0f0d`, in code segment
`0f0h` (image `0xf00`), and the credits loop is `0x10ee`-`0x13c3`:

```bash
objdump -D -b binary -mi8086 --start-address=0x10c0 --stop-address=0x13c4 \
    out/release-images/ALKU.img
```

`a`, `p` and `tptr` are DGROUP `9eh`, `0a0h` and `0a2h`, `f` is `[bp-4]`, and the copper's
`frame_count`, `cop_dofade`, `cop_start` and `cop_pal` are `0354h:0004h`, `:00b3h`, `:0010h` and
`:000ah`. What differs from `MAIN.C`, each pinned by
`the_release_loop_and_its_closing_routine_are_the_images`:

| image | the release | `MAIN.C` |
|---|---|---|
| `0x1380` | the loop runs while `a <= 320` | `a<320` (`:87`) |
| `0x2315` | `do_scroll` returns 0 at once when `a > 320` | no guard (`:397`) |
| `0x134f` | `fmaketext` when `aa < 304` | `aa < 308` (`:141`) |
| `0x136c` | each pass ends `do_scroll(tptr<5 ? 1 : 0)` | `do_scroll(1)` (`:145`) |
| `0x13bd` | `fonapois()` then the closing routine (`0x13ce`) | `close_copper()` (`:151`) |
| `0x1d49` | `prt` draws 29 rows | 32 (`:274`) |
| `0x1857`, `0x1acf` | the strip's conversion and its metrics scan cover 29 rows | 32 (`:169`, `:218`) |

Everything else the loop reaches is the tree's: the `f` chain, the five cases and their strings and
positions (jump table `cs:04c4h`), the batch wait, `if(f>7)`, `addtext`'s and `faddtext`'s 32 rows,
`fmaketext`, `ffonapois`, `fonapois`, `dofade`, `wait`, `do_scroll`'s body and the three copper
routines.

**After the fifth batch** (`MainPhase::Spin`), mode 0 neither runs `ascrolltext` nor waits: the
window steps on the tick its nine frames are up, the `textin` and then `textout` fades are set on
their first tick, and `f` holds at 51
(`the_fifth_batch_stops_carrying_the_text_and_the_pass_no_longer_waits`,
`the_loop_ends_past_320_without_the_final_fade_and_hands_over_in_four_ticks`).

**The closing routine** (`0x13ce`-`0x1723`), one step a copper frame:

1. `p=0`, `cop_start=a/4` (80), `cop_scrl=0`, `cop_pal=palette`: the last step's window again;
2. write mode 1; row `y` of a 200-row page at byte `a/4 + 176y + 0x738` gets 80 bytes of memory
   row `2*min(y,180)`; `cop_start` moves there, and CRTC 9 = 1 doubles it (320×200);
3. the page is copied to a linear one at `0xc000` (80 bytes a row), every byte of `0xbff0`-`0xffef`
   becomes `(v&63)+0xc0`, and `cop_start=0xc000` with CRTC 0x13 = 0x28;
4. three 16,000-byte page copies, CRTC 9 = 0x41, `close_copper`, `exit(0)`.

In the unit script the frames are 5949 (`a=319`'s window), 5950 (`a=320`'s, the snap), 5951 (the
compacted page, each row the 400-line frame's even row) and 5952 (the linear page, the same RGB)
(`the_closing_routine_shows_the_last_window_as_a_320x200_page`). The port does not make step 4's
page copies, which nothing shows before the exit and which would land on the wrapped tail of the
`0xc000` page in the port's 256,000-pixel video memory (D16). The emulator applies CRTC 9 half a
frame late, so the original's capture holds one frame of step 2's page that turns double-scanned at
line 185 (5950, `reference-indices.md` §4.5).

**The exit path.** After each card gate, `fonapois`, `ffonapois` and the loop's end, `main` calls
the closing routine when `dis_exit()` is set: `fade1` (black), one frame, `close_copper`, `exit(1)`
(A62, §6).

**The font** (segment `4254h`, read at `es:[bx+1]`, image `0x42541`): rows 30-31 are zero, and rows
0-29 are `FONA.UH[783:]` but for three bytes that merge the tree's two glyphs at 938 and 944 into
one; the release's `fonaorder` (DGROUP `0c4h`) is the 75-character `…:&()+-*=/[]`, and the third card
is `'['`/`']'`, the same two logo runs as the tree's `\x8f`/`\x99`
(`the_release_font_is_the_strip_but_for_one_glyph_and_its_padding_is_zero`, A9).
