# MNTSCRL — the mountain scroller

`FOREST/READ2.PAS` (224 lines) over `FOREST/ROUTINES.ASM` (39 lines) and the
linked assets `HILLBACK.CLX` (`BGR.PAS:2-6`), `POS1-3.DAT` (`AOS1-3.PAS`) and
`O2.SCI` (read at run time, `:110`). A 320×200 mode 13h landscape with a leaf
overlay scrolling left across it, music-gated at both ends.

The port is `bins/secondreality/src/parts/mntscrl.rs`, registered as
`"mntscrl"`. `--block 3` begins here (`MAIN/U2.ASM:856-866`).

## 1. What the original does

| source | act |
|---|---|
| `:101-108` | mode 13h; `dis_partstart` |
| `:110-117` | 31 rows of 640 bytes from `O2.SCI` at `x*640+778`, into `fbuf` |
| `:119` | every non-zero `fbuf` byte gets `+128`; the loop runs to `sizeof(fbuf)`, one byte past the declared array |
| `:120-123` | palette from `hback+10`; the 64,000-byte picture from `hback+778` rebased to `+0`; the DAC set black; the picture blitted to `$a000` |
| `:125-142` | 64 frames fading the DAC up to `pal`, with entries 0-31 and 128-159 held black |
| `:144-151` | `tmppal = pal`; `fpal = pal` with those bands black, uploaded; the font built from source columns 0-132 into font columns 104-236 of each of 31 rows; `scp := 133` |
| `:153-159` | busy-wait while `dis_musplus() < 0` |
| `:165-182` | 127 frames of `scr(sss)` with `sss` cycling 0,1,2; on odd frames entries 0-176 are re-uploaded and ramped |
| `:184` | `tmppal` zeroed — the fade-out's target |
| `:193-224` | the main loop: `waitb`, poll `dis_musplus`, fade out on **`== -11` exactly**, `scr(sss)`, exit on `dis_exit` or `quit` |

## 2. The port's tick map

One tick is one `waitr` (`dis_waitb`). Tick 0 is the code before the first
wait, the convention CRED and ENDSCRL use too.

```text
tick 0        mode-state view, the black DAC, the 64,000-byte background blit
ticks 1-64    the 64 fade-in uploads; tick 64 also `:144-151`
ticks 65-…    the music wait — one poll a tick, drawing nothing
127 ticks     the pre-roll
then          the main loop, one `scr` a tick
```

The reference's own fade pins tick 0 to boot frame **419**:
`reference-indices.md` §11.1 records the frame means 6.08 (425), 8.91 (430),
10.25 (435) and 10.53 (440); the port's palette curve `min(step, tmppal)`
puts them on steps 5, 10, 15 and 20 with the displayed frame
`tick 0 + step + 1`. All four agree on 419
(`the_fade_in_pins_tick_zero_to_boot_419`). **This is a mapping, not an
anchor**: `shell::boot_anchor("mntscrl")` stays `None`, a `--part mntscrl` or
`--block 3` run numbers its own frames from zero, and its manifest says
`index_origin: "part"`, `anchor: null`
(`the_run_is_numbered_in_its_own_frames_and_draws_320x200`,
`a_block_three_run_starts_here_and_is_numbered_the_same_way`).

## 3. The blitter and the font

`Putrouts` (`ROUTINES.ASM:11-39`) walks one list's 7,347 columns (`237*31`,
`:20`). Each column is a count and that many 16-bit video offsets; a zero count
is a hidden column and writes nothing (`:23-26`). For each offset `di`:

```text
es:[di] = gs:[di] + fs:[bx]
```

— the **background** byte (read from `hback`, not the screen) plus the
column's single font byte, a byte add. A font byte of 0 is transparent in the
additive sense. `scr` (`:56-91`) picks the list; `scr(2)` then shifts the
packed 31×237 font one byte left (`move(font[1],font[0],237*31)`, `:86`),
appends `fbuf[ff*640+scp]` at column 236 of every row (`:87`), and advances
`scp` (`:88`). The packed array is kept as one buffer because that is what the
source's `move` shifts: the shift crosses row boundaries, and the append
immediately overwrites the seam. `the_pre_roll_uploads_on_odd_frames_and_ramps_entries_zero_to_176`
measures the pre-roll's 42 shifts (`scp` 133 → 175) and the ramp's odd-frame
rhythm; `scr_adds_the_font_column_to_the_background_and_a_zero_count_is_transparent`
and `scr_two_shifts_the_font_and_appends_the_next_o2_column` cover the
blitter's units.

## 4. The palette

The 64-frame fade-in ramps `fpal` from black toward `tmppal = pal` with bands
0-31 and 128-159 zeroed; each tick uploads before stepping
(`the_fade_in_is_sixty_four_uploads_one_step_a_tick`). After it, `fpal` is
`pal` with the bands black, uploaded, and the font built. The pre-roll's odd
ticks upload entries 0-176 and step them toward `pal` — the reference's
"background appearing" ramp — while entries 177-255 stay where `:148` left
them. The pre-roll's last odd iteration uploads step 62 and increments to 63,
so a channel whose target is 63 is displayed at 62 until the fade-out's first
upload: entry 151 is the one such channel in the bands, and the test above
pins it. That one-step shortfall is the source's, not the port's. The
fade-out zeroes every entry one step a tick, uploading before each decrement;
the first fade tick is therefore the pre-roll's target — entry 151 at 63 —
and the 65th uploads black (`the_cue_is_exactly_minus_eleven`,
`the_fade_out_is_sixty_five_ticks_and_ends_the_part`).

## 5. The assets

| asset | bytes | what the part uses |
|---|---|---|
| `O2.SCI` | 21,258 | RIX3 640×32; rows 0-30, 640 bytes each, at 778 |
| `HILLBACK.CLX` | 64,778 | RIX3 320×200; palette at 10, picture at 778 |
| `POS1-3.DAT` | 25,088 / 25,064 / 25,114 | 7,347 columns each, 15,592 positions in all |

All three are read in their original formats (`sr_assets::load_path` and the
shared `PosList` decoder); `the_assets_are_read_the_way_the_source_reads_them`
compares every O2 byte after the +128 bias, the palette against the file's
bytes 10-777, the background against bytes 778-64,777, and the lists' column
count and 15,592 positions. The largest destination offset in any list is
**63,870**, inside the picture.

The source's rebase of `hback` (`:121`) is not performed. The port keeps the
rebased segment's layout — the 64,000 picture bytes followed by the asset's
own last 778 bytes — so that a blitter offset in the 64 KB range reads what
the original's data segment held (`BACKGROUND_LEN`).

## 6. The music, and the release's timeline — the D24 measurement

**The head start.** The block-3 restart (`MAIN/U2.ASM:856-863`) starts MUSIC1
at order 42 on the music clock's first tick, frame 408
(`shell::MUSIC_START_FRAME`). The reference's part tick 0 is boot 419, so its
player had been polled **11 times** before the part's first tick. An
unanchored run starts the player with the part, so `Mntscrl::init` pre-polls
those 11 (`tick_zero_is_the_background_through_a_black_dac` asserts the poll
count; the value is the same `pre` `MusicStart` would compute for an anchor at
419). The observable consequence is the dump's music position: at tick 81
(boot 500) it is `42:10`, which is exactly what
`reference/music/second3/trace.csv` holds at frame 500
(`the_music_is_eleven_polls_in_at_the_parts_first_tick`). Without the
pre-roll the position would read `42:9`.

**The release's first wait is not the tree's.** `:153-159` waits for
`dis_musplus() >= 0`, and the trace never reads that until frame 2488 (order
47 row 0): from the part's start to then, `zplus` is 0 through orders 42-44
and 1 from order 45, where `musplus = row - 64` is still negative until row
64. A faithful translation therefore holds through every reference keyframe
but 500.

What the release does instead is measured from its own keyframes. With a
first wait of `W` ticks, the reference's frame at boot `K` is the part's
main-loop tick `K - 611 - W`; the port's unit test runs the same part with a
scripted wait of each candidate `W` and compares the rendered RGB:

- at boot **1100**, exactly one `W` in 0…200 reproduces the frame: **71**
  (0 of 192,000 bytes);
- the same 71, applied without refitting, reproduces **1700** and **2300**
  byte for byte.

So in the release the last held frame is boot **554** (tick 135), the
pre-roll runs 555-681 (ticks 136-262), and the main loop starts at boot
**682** (tick 263). That is `the_reference_holds_seventy_one_frames_and_predicts_every_scroll_keyframe`,
and it is a *prediction*, not a fit: 1100 alone chooses the number, and the
other two keyframes test it.

**What the tree's wait draws at those indices.** In the real run the trace's
first non-negative reading lands on part tick **2068** (the port's published
words lead the trace by one frame; the dump at 2068 reads `47:0` and the
pre-roll's first `scr` changes the frame at 2069 —
`the_tree_waits_until_the_traces_first_non_negative_reading`). The main loop
therefore starts at tick 2196. The real cue — order 48 row 53, `musplus =
-11` — falls inside the integration test's bracket: 2,500 is the full
scroller at `48:42`, and 2,650 is black at `50:16`, the part retired
(`the_real_players_cue_fades_the_part_out_and_the_run_holds_black`).

Consequently the four keyframes through the binary compare as

| boot | part tick | tree's wait | differing bytes | differing pixels | % of 64,000 px |
|---|---|---|---|---|---|
| 500 | 81 | held opening | 0 (both planes) | 0 | 0.0000 |
| 1100 | 681 | still waiting | 118,843 | 39,658 | 61.97 |
| 1700 | 1281 | still waiting | 118,924 | 39,684 | 62.01 |
| 2300 | 1881 | still waiting | 119,056 | 39,738 | 62.09 |

500 is compared **indexed** (both sides have `.idx`; the port's tick-81 `.idx`
is the reference's byte for byte — `the_held_opening_is_the_references_frame_500_in_both_planes`).
1100, 1700 and 2300 are RGB-only in the reference, so they are compared RGB
(`the_tree_wait_holds_through_the_reference_scroll_keyframes` for the
percentages, `the_reference_comparison_at_the_tree_timeline_measures_the_wait_divergence`
for the byte counts).

`cargo xtask compare` is not the tool here, and cannot be: it pairs frames by
the manifest's index, and a run numbered from zero at tick 81 is not index
500. The comparison above is the integration test's own byte count at the
mapped tick — stricter than `compare`'s ±1 pairing, because the port has no
session-to-session phase to search. **D19's support is taken anyway**: the
integration test dumps `K-1, K, K+1` around each keyframe and the run's three
are all the held opening — the divergence in its sharpest form — while the
unit test's 71-tick run matches `K` and is *different* at `K±1`, which is what
makes the mapping exact rather than a search's loosest fit.

**The decision, under D24.** The tree cannot produce the release's hold from
any `musplus` reading: through the whole interval `musplus` is the constant
`-32`, so every threshold either exits at tick 65 or waits until tick 2,068.
The release's behaviour is therefore registered, not adopted (A17), and the
port keeps `:153-159`'s wait. The *picture* is not in doubt: with the
release's 71-tick hold the port reproduces all four keyframes exactly, which
is the note's evidence that the transliteration of the blit, the font, the
palette and the pre-roll is right and the difference is the wait alone.

## 7. The `scp` bound: the release's 640 (D24's single constant, adopted)

The tree's `:88` is `if scp < 639 then inc(scp)`, so its last appended column
is 639. The release reads one column further. At boot 2300 — the first
keyframe after the bound is reached (main tick 1,618, shift 506) — the port's
frame under 639 differs in **401 bytes / 146 pixels**, and under **640** in
**0**; 1100 and 1700 are before the bound and are identical under both. The
tree's own `fbuf` is `array[0..640*31]`: index 19,840, one past the 31 loaded
rows, is exactly the element a stride-640 read at column 640 needs for row
30, and `:119`'s loop covers it. The array's extra element exists for this
read, so the release's bound is the array's own. This is D24 step 3's
single-constant branch — one expression, with every other difference in the
part accounted for (the four keyframes are otherwise byte-identical) — so the
port writes 640. The measurement is
`the_releases_scp_bound_is_sixty_forty_where_the_tree_says_639`; the tree's
639 survives in it as the counterfactual the same code is run with.

## 8. The bugs kept

- **The `fbuf` overrun** (`:119`): `for x := 0 to sizeof(fbuf)` covers index
  19,841, one past `array[0..640*31]`. In the original that byte is `font[0]`,
  still zero because the font is built at `:150`; the port keeps an explicit
  zero overrun byte and applies the bias to it too
  (`the_bias_includes_the_loops_one_byte_overrun`).
- **The dead `veke` bound** (`:3`, `:224`): `inc(frame)` is commented out
  (`:221`), so `frame = veke` can never fire. The port has no frame counter;
  `the_dead_veke_bound_never_ends_the_part` runs past 2,800 main-loop ticks
  with no cue and the part is still running.
- **The forced-black bands** (`:126-127`, `:146-147`) and their ramp back
  (`:165-178`) are both kept — see §4.
- **The exact cue** (`:200`): `dis_musplus() == -11`, not `<= -11` and not
  `< -10`. `the_cue_is_exactly_minus_eleven` drives -10 and -12 and the part
  keeps scrolling with the full palette; -11 starts the 65-tick fade, which
  ends the part (`the_fade_out_is_sixty_five_ticks_and_ends_the_part`).
- **The wait honours `dis_exit` in the port** even though `:153-159` does not
  test it: the port does not spin, so an exit request ends the wait on the
  tick it is seen (`exit_requested_ends_the_wait_on_that_tick`, and the
  phase's Review Focus 1). The source's wait would hold until the cue.

## 9. What is substituted

- `O2.SCI` and `HILLBACK.CLX` are read through the RIX3 decoder rather than
  by raw `seek`s. The decoder's palette and pixel payload are the bytes at
  offsets 10 and 778; the part's test compares both against the files.
- The `hback` rebase (`:121`) is not performed; the rebased layout is kept as
  data (§5).
- The blitter writes into the surface's first 64 KB with `y_line_offset =
  Some(320)`, which is the mode 13h picture. The `di` offsets and the 64 KB
  wraparound are the original's; no list offset reaches the wrap.
- No approximation of the algorithm itself: the fades, the ramp, the shift
  and the additive blit are the source's arithmetic.

## 10. Commands

```bash
cargo test -p secondreality --bin secondreality mntscrl::tests   # 17 tests
cargo test -p secondreality --test mntscrl                        # 8 tests
cargo test --workspace
cargo clippy --workspace -- -D warnings
cargo clippy --workspace --all-targets -- -D warnings
cargo xtask reference verify --tree reference                     # exit 0
```

The per-keyframe numbers above are produced by the named tests; the trace's
positions are `reference/music/second3/trace.csv`'s, and the reference's
palette and dimensions are `reference/parts/mntscrl/manifest.json`'s.
