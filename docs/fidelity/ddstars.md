# DDSTARS — the hidden part

Sources: `DDSTARS/KOE.ASM` (99 lines, the program) including `DDSTARS/STARS.ASM` (803, the effect)
at `KOE.ASM:63`; `POLYEGA.ASM` is linked but its routines are never called. Port:
`bins/secondreality/src/parts/ddstars.rs` (`"ddstars"`); its module doc has the acts, the 320×400
two-plane screen, the page/text offset, the star arithmetic, the never-cleared ring and the palette.
Run: mapping **`boot = 414 + tick`** (no boot anchor). Music: none read; the shell's block-U row
restarts `MUSIC0` at order 70.

Slot 17, outside the numbered sequence: `SECOND U` sets `whattorun = 0x80` (`partsmask[8]`,
`MAIN/U2.ASM:119`, accepted at `:626-627`), restarts MUSIC0 at order 70 (`:889-894`) and runs
`exehid` alone. No other boot sets bit 7.

## 1. Timeline

| source | act |
|---|---|
| `KOE.ASM:72-76`, `STARS.ASM:56-70` | `dis_partstart`; mode 13h; AC 0-15 identity; overscan 255; DAC cleared |
| `STARS.ASM:72-98` | CRTC 9 bit 7 cleared; 64 KB of video cleared; EMS 64 pages, `clearsbu` clears all 128 units once |
| `STARS.ASM:100-167` | the row tables, the 1024-star table, the two `muldiv` tables, `starlimit = 512`, the text state |
| `STARS.ASM:169-174` | **100 `starfetch0`/`staradd` passes** before the first displayed frame |
| `STARS.ASM:540-778` | `do_stars`, one iteration a tick: flip, `waitb`, fade, marks, `risetext`, a star pass, the blit |

| tick | boot | `starframe` | on screen |
|---|---|---|---|
| 0 | 414 | 1 | stars drawn into the back page; the CRT shows the never-written page (all index 0) |
| 1 | 415 | 2 | the first frame with stars (the reference's probe at 415) |
| 0..32 | 414..446 | | the palette fade (`starpalfade <= 32`) |
| 900..1199 | | 901..1200 | no star is drawn; the ring's stale units repeat |
| 1199 | | 1200 | mark: text block 0 opens |
| 1499 | | 1500 | mark: `starlimit = 1024` |
| 3199 | | 3200 | mark: text block 1 |
| on | | | no cap; exit on DIS function 2 only (`:772-776`) |

`the_run_is_numbered_in_its_own_frames_and_draws_320x400` asserts tick 0's picture is all index 0
and tick 1's is not. `init_stars`'s 100 passes leave `emmpage4 = 36` and `starlimit = 412`.

## 2. The screen

The reference is `.rgb` only: the emulator names the screen 16-colour and refuses the indexed read
(`reference-indices.md` §22.5, all 20 sweep frames). The port composes `index = plane0bit |
plane1bit << 1 | plane2bit << 2`; the text forces plane 2, so its colours are 4-7 and the palette
has seven: black (0 and 4), the stars' 1-3 and the text's 5-7; the keyframes match the reference
byte for byte (rediff). Through the engine's rounding the
full-fade entries are `(65,81,101)`, `(97,125,150)`, `(166,210,251)` for the stars and
`(40,81,142)`, `(81,121,182)`, `(121,162,243)` for the text
(`the_full_fade_palette_is_the_references_seven_colours`).

The model is a 40-byte plane row and a 400-row page (16,000 bytes a plane). CRTC 9's bit 7 is
cleared (`:72-78`), but the doubling that matters is the maximum-scan-line field the mode set leaves
at 1; the reference's 400 rows are what the blits and the text occupy.

## 3. The mapping

**`boot = 414 + tick`, a mapping, not an anchor.** The reference files no frame at the part's
start; its earliest evidence is a probe at 415 with the screen up (`reference-indices.md` §22), and a
probe bounds a start without pinning it. `shell::boot_anchor("ddstars")` stays `None` and the
manifest says `index_origin: "part"`, `anchor: null`
(`the_run_is_numbered_in_its_own_frames_and_draws_320x400`; conventions in part-sequence.md
(block seams / pre-roll / mapping conventions)).

The pictures pin it. The four keyframes reproduce byte for byte at `tick = boot - 414` (rediff's
`ddstars` entry), and so do the survey's frames at 500, 700, 900, 1100, 1300, 1500, 1700 and 2100
against the capture in §8 (§8's sweep comparison; it needs the capture, so it is not part of the gate):

| boot | tick | what pins it |
|---|---|---|
| 500 | 86 | the top half is the page written at tick 85, the bottom its `+32` mirror; no text |
| 1700 | 1286 | reproduced only with `staradd2`'s second unit (§4) |
| 1900 | 1486 | the text band's open height: 28 source rows is `startxtuse = 32`, `starframe` 1487; the stars are the page written at 1485 |
| 3000 | 2586 | `startxtuse` capped at 99, block 0, source rows 1-95 |
| 5000 | 4586 | block 1's mark at `starframe` 3200, source rows 101-195 |
| 8000 | 7586 | the text-free late field |

**The layers.** Star pixels are on even scanlines and text on odd:
`the_text_and_star_layers_are_on_opposite_row_parities` finds 200 star rows, all even, and 28 text
rows at 1900, all odd. The text is written into the *displayed* page and the stars into the back
page (module doc), so the screen at tick `t` carries the text of `t` and the stars of `t-1`; that
is what reconciles the star and text evidence at 1900. Drawing both in one iteration would put one
layer a frame out.

## 4. The stars and the ring, measured

- **The LCG** (`STARS.ASM:33-42`) returns the `mul`'s `dx`, not the new seed's high word; the seed
  starts at 0. The first twelve returns are `0, 126, 25208, 45290, 7958, 4852, 38525, 27318, 39895,
  10061, 34431, 60585`, and the first entries `(z,x,y)` are `(255,-512,-386)`, `(254,120,-278)`,
  `(253,278,244)`, `(252,125,182)`
  (`the_rng_is_the_originals_lcg_and_its_first_values_are_the_sequence`).
- **The tables**: `muldivy = 23592`, `muldivx = 31457` at z = 0, and 3024 and 4032 at z = 255
  (`the_projection_tables_are_the_originals_integer_divisions`). At the bounds `(0,0,-69)` draws at
  `(0,160)`, `(0,0,-70)` is row 100 and skipped, `(255,-512,-512)` draws at `(5,34)`
  (`the_projection_places_the_star_and_clips_at_the_sources_bounds`).
- **The counts.** `staradd` walks the first 512 entries and decrements `starlimit` by 1 a call
  (`:278-285`); `staradd2`, called from `starframe` 1201 on (`:670-676`), walks all 1024 and
  subtracts 4 a call (`:345-351`). An entry draws only when `bp >= starlimit` (`:292`, `:358`).
  The mark at 1500 sets `starlimit = 1024` (`:663-665`), so the field fills in four entries a frame
  again.
- **The ring** never clears after `init`, so each reuse ORs in another generation and the field
  densifies across the keyframes.
  `the_ring_wraps_every_sixty_four_ticks_and_never_clears` plants one star in unit 37, runs a whole
  ring and finds the bit still there; `the_blit_reads_the_fetched_unit_on_top_and_the_plus_thirty_two_reversed`
  finds row 99 of the `+32` unit at page row 100.
- **`staradd2` writes two units**, `u` and `(u + 17) & 63` (`starfetch1`, `:260-267`, `:378-397`;
  `the_second_star_pass_writes_the_same_bits_into_the_plus_seventeen_unit`). The survey's 1700 frame
  is the evidence: with both writes the port's page is byte-identical to the capture (§8's sweep
  comparison); with only the fetched unit it was observed once to differ, which is not part of the
  gate.

## 5. The text wipe

`risetext` (`:413-538`) is a bottom-anchored reveal. With `A = startxtuse = min(startxtopen,
startxtclose)` after both step:

```text
startxtopen  -9999, +1 a call, capped at 99; the marks reset it to -256
startxtclose 10000, -1 a call, floored at 0; the marks reset it to 1500
row range    150-A to 149; the copy begins at 151-A
copies       A-4 source rows, from `startxtp0/80`
```

The band's odd page rows are `2*(151-A)+1` through **293** for every `A`; the two clears are the odd
rows just above, `2*(150-A)-1` and `2*(150-A)+1`, and the closing clear is row 295. At 1900 (`A = 32`)
the band is 28 odd rows at scanlines **239-293** (source rows 1-28). The reference's visible ink is
on **15** of them, the first at **259**: the source's first inked row is 11 (21 pixels; plane 0 has
12) (`the_text_strip_is_read_as_its_two_planes`). The marks are `starframe` 1200 (block 0,
`startxtp0 = 80`) and 3200 (block 1, `startxtp0 = 101*80`). `A = 2`'s 80-byte clear (`:528-537`)
crosses one row pair; the port keeps it. It is a `risetext` path, not the star clip of §6.

## 6. D24: the star clip, and which artifact each decision follows

`MAIN/DATA/DDSTARS.EXE` is the shipped, **packed** build; the tree's `KOE.OBJ` is the source build
(`DDSTARS/MAKEFILE:2-3`). The tree's star clip is assembled out: the `cmp di,_nostar1/_nostar2`
guards are commented out at `STARS.ASM:692-696`, `:711-716`, `:737-742` and `:756-762`, while
`risetext` still computes `_nostar1 = 80*(150-A)` (`:436-442`) and `_nostar2 = 11,879`
(`:525-526`). With the guards in, every blit row whose destination offset lies in
`[80*(150-A), 11879]` would be skipped: at 1900 (`A = 32`) the bottom half's rows 18-48, page rows
**236-296**, where the reference holds **1,025 star pixels**; at `A = 99` the window reaches into the
top half too (98 page rows from 102 to 296).

**The port does not clip, and the keyframes show the shipped build does not either.** The four
keyframes reproduce at 0 *with those 1,025 pixels present*
(`the_star_clip_is_assembled_out_and_the_keyframes_hold_its_pixels` counts them in the reference and
in the port's page). There is no `.idx` to follow (§2); the port's index composition is the
source's.

## 7. Verification

The four keyframes all move (`reference-indices.md` §22.3), so D19's support is a frame each side.

| keyframe | tick | plane | differing | register |
|---|---|---|---|---|
| 1900 | 1486 | RGB | 0 | exact |
| 3000 | 2586 | RGB | 0 | exact |
| 5000 | 4586 | RGB | 0 | exact |
| 8000 | 7586 | RGB | 0 | exact |

The counts are the `ddstars` rows of `xtask/src/rediff/table.rs`.
`the_keyframes_neighbours_differ_and_the_music_runs_twenty_two_frames_behind_the_trace`
(`tests/ddstars.rs`) asserts through the binary that `K-1`/`K+1` differ. The rendered neighbours at ticks 1485, 1487,
2585, 2587, 4585, 4587, 7585 and 7587 hash to `de6d1676`, `a1697853`, `f768ffbb`, `5eb100a6`,
`062d91c4`, `0af0a960`, `fb4b902c`, `328201f6`: `reference-indices.md` §22.3's table, entry for entry.

**The palette** (`:559-648`): `the_palette_fade_is_the_sources_thirty_three_ticks` finds tick 0
uploading `b = 8` (entry 1 black), ticks 31 and 32 both 255, and tick 33 stopping with the DAC held.
The keyframes are past the fade.

**The music.** The part makes no music calls. At tick 7586 the port publishes `30:51`, while the
`hiddenu2` trace's boot 8000 stands at `30:54`. D27's clock starts at **408**, the mapping's zero is
**414**, and the `hiddenu2` session's phase is **-16 frames** (`cargo test -p sr-audio --test trace --
--nocapture`; `music-clock.md` §2), so the trace runs `414 - (408 - 16) = 22` frames ahead: its frame 7978
reads **30:51**. The integration test asserts the port's position and both trace frames. The picture does
not depend on either.

**The exit.** `STARS.ASM:772-776` draws the iteration, then checks DIS function 2. The port polls
`exit_requested` at the top of `frame`, so it is one frame short at the exit
(`exit_requested_finishes_on_that_tick`). No filed frame is near an exit.

**`dis_sync`'s upper band (A15).** The `hiddenu` trace holds `(order << 8) | row > 0x4200` for
**5,148 of its 6,351 frames (81.1 %)** (the `awk` in §8), and the reference is still running at
20,000 (`reference-indices.md` §22.1). DDSTARS never calls `dis_sync`, but a part placed here that
waited on it would meet the answered band at once.

## 8. Reproduce

```bash
cargo test -p secondreality --bin secondreality parts::ddstars
cargo test -p secondreality --test ddstars
cargo xtask rediff --only ddstars

# the survey capture behind §3's sweep and §4's 1700, and the sweep comparison (exit 0, identical)
cargo xtask reference capture --arg U --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames 500,700,900,1100,1300,1500,1700,2100,3000,5000,8000 --out out/b5-su-sweep1
cargo run --release --bin secondreality -- --part ddstars --headless \
    --dump-frames 86,286,486,686,886,1086,1286,1686,2586,4586,7586 --dump-dir out/ddstars-sweep
cargo xtask compare --reference out/b5-su-sweep1 --rendered out/ddstars-sweep --align 414 \
    --out out/ddstars-sweep-diff

# §7's support hashes
cargo run --release --bin secondreality -- --part ddstars --headless \
    --dump-frames 1485,1487,2585,2587,4585,4587,7585,7587 --dump-dir out/ddstars
sha256sum out/ddstars/ddstars_*.rgb

# §7's band count, and the still-running capture
awk -F, 'NR>1 && ($2*256+$3)>0x4200 {n++} END {print n, NR-1}' reference/music/hiddenu/trace.csv
cargo xtask reference capture --arg U --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames 10000,12000,14000,16000,20000 --out out/ddstars-late
```

**Register.** `bugs.md` §27; A15.
