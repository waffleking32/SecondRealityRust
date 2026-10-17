# DDSTARS — the hidden part

`DDSTARS/KOE.ASM` (99 lines) is the program; `DDSTARS/STARS.ASM` (803 lines) is
the effect, `include`d into it at `KOE.ASM:63`; `POLYEGA.ASM` is linked in but
its `poly`/`polyn_disp` routines are never called. Slot 17 of the demo, and not
in the numbered sequence at all: `SECOND U` sets `whattorun = 0x80`
(`partsmask[8]`, `MAIN/U2.ASM:119`, accepted at `:626-627`), restarts MUSIC0 at
order 70 (`:889-894`) and runs `exehid` by itself. No other boot sets bit 7.

The port is `bins/secondreality/src/parts/ddstars.rs`, registered as
`"ddstars"`. A `--part ddstars` run takes the shell's block-U row,
`MUSIC0` at order 70 (`shell::music_for_part`); the part itself never reads the
player (§9).

## 1. What the original does

| source | act |
|---|---|
| `KOE.ASM:72` | `dis_partstart` (a version check, no frame) |
| `KOE.ASM:74-76`, `STARS.ASM:56-70` | mode 13h; AC entries 0-15 identity; overscan 255; DAC cleared |
| `STARS.ASM:72-78` | CRTC 9's bit 7 cleared |
| `STARS.ASM:80-85` | 64 KB of video cleared |
| `STARS.ASM:87-98` | EMS allocate 64 pages, page frame taken, `clearsbu`: all 128 units cleared once |
| `STARS.ASM:100-118` | the 200-entry and 400-entry `rows` tables (40 bytes a row) |
| `STARS.ASM:123-139` | the 1024-entry star table: `z = (cl-1) & 0xFF`, `x`/`y` from the LCG |
| `STARS.ASM:141-161` | `muldivy[z] = ((108<<16)/bp)>>1`, `muldivx[z] = ((144<<16)/bp)>>1`, `bp = 150+4z` |
| `STARS.ASM:163-167` | `starlimit = 512`, `starpalfade = 0`, text state `-9999/10000` |
| `STARS.ASM:169-174` | **100 `starfetch0`/`staradd` passes** before the first displayed frame |
| `STARS.ASM:540-548` | `do_stars` begins: the page flip, then `waitb` |
| `STARS.ASM:559-648` | the palette fade while `starpalfade <= 32` |
| `STARS.ASM:650-666` | `starframe++`; marks at 1200 (text block 0), 3200 (block 1), 1500 (`starlimit = 1024`) |
| `STARS.ASM:667` | `risetext`: the wipe, into the page the CRTC is showing |
| `STARS.ASM:669-678` | `starfetch0`; `staradd` through 900, nothing 901-1200, `staradd2` after 1200 |
| `STARS.ASM:679-770` | the two half-blits into the back page |
| `STARS.ASM:772-776` | exit on DIS function 2 (`exitflag`) and nothing else |

## 2. The screen: 320×400, two planes, two pages

Mode 13h's CRTC offset register is never written, so its row is 40 words; the
port's model — measured against the reference — is a **40-byte plane row** and
a 400-row page, 16,000 bytes a plane. CRTC 9's bit 7 is cleared (`:72-78`), but
the doubling that matters is the maximum-scan-line field the mode set leaves at
1, and the reference's 400 rows are what the star blits and the text occupy.

The emulator names the screen **16-colour** and refuses the indexed read
(`reference-indices.md` §22.5: all 20 sweep frames carry *"not one palette
index a byte"*), so the reference is `.rgb` only and the port composes the
index itself:

```text
index = plane0bit | plane1bit << 1 | plane2bit << 2
```

The stars set planes 0 and 1 (`STARS.ASM:315-327`, `:378-397`). The text copies
its two source planes into planes 0 and 1 and, through the graphics
controller's set/reset, forces **plane 2 = 1** and plane 3 = 0 (`:479-483`), so
the text's four colours are indices 4-7. That is the reference's seven distinct
colours: black (entries 0 and 4), the three star entries 1-3 and the three text
entries 5-7. The full-fade entries through the engine's rounding are asserted
by `the_full_fade_palette_is_the_references_seven_colours`: `(65,81,101)`,
`(97,125,150)`, `(166,210,251)` for the stars and `(40,81,142)`,
`(81,121,182)`, `(121,162,243)` for the text.

**The row pairs.** The blit copies 40-byte source rows to page offsets 0, 80,
160, … (`:697-704`), so its rows are the *even* page rows. `risetext` starts
its destination 40 bytes into the 80-byte stride (`:450-460`), so the text
lands on the *odd* ones. The reference shows exactly this: star pixels on even
scanlines, text on odd (`the_text_and_star_layers_are_on_opposite_row_parities`
asserts 200 star rows, all even, and 28 text rows at the 1900 keyframe, all
odd).

**The two pages, and the text's one-tick offset.** `starvram` flips between
segments `0xA000` and `0xA400` while the CRTC start flips between 0x0000 and
0x4000 (`:544-553`), so the page being drawn is never the page being shown.
Two write targets follow from the order of the source:

- `risetext` runs at `:667`, **before** `:683-684` reloads `es` with
  `starvram`. Its `es` is still the page the *previous* iteration selected —
  which is the page currently on the CRT. So the text is written into the
  **displayed** page.
- the star blits run after `es = starvram`, into the **back** page.

The screen during iteration `t` therefore carries the text of `t` and the
stars of `t-1`. This is what reconciles the star and text evidence at boot
1900: the stars are the page written at tick 1485, and the text is tick
1486's, whose `startxtuse` is 32 (§5). A port that drew both in the same
iteration would be one frame out on one layer or the other.

## 3. The tick map, and the mapping

One tick is one `do_stars` iteration — one `waitb`. **The mapping is
`boot = 414 + tick`.** It is a mapping, not an anchor: the reference files no
frame at the part's start, its earliest evidence is a probe at 415 with the
screen already up (`reference-indices.md` §22), and a probe bounds a start
without pinning it. `shell::boot_anchor("ddstars")` stays `None`, a run of
this part numbers its own frames from zero, and its manifest says
`index_origin: "part"`, `anchor: null`
(`the_run_is_numbered_in_its_own_frames_and_draws_320x400`,
`the_mapping_is_boot_equals_fourteen_plus_the_tick`).

The mapping is pinned by the pictures. The survey's `out/b5-su-sweep1`
frames at 500, 700, 900, 1100, 1300, 1500, 1700, 2100, 3000, 5000 and 8000
are all reproduced byte for byte at the ticks below
(`the_plus_seventeen_write_is_what_the_survey_frame_shows`, the ignored
measurement test; `out/` is gitignored and the capture command is §10's):

| boot | tick | what pins it |
|---|---|---|
| 500 | 86 | the top half is the page written at tick 85, the bottom its `+32` mirror, and the text is absent |
| 1700 | 1286 | reproduced only with `staradd2`'s second unit (the counterfactual is §4) |
| 1900 | 1486 | the text band's open height: 28 source rows is `startxtuse = 32`, which is `starframe 1487`, and the stars are the page written at 1485 |
| 3000 | 2586 | `startxtuse` capped at 99, block 0, source rows 1-95 |
| 5000 | 4586 | block 1's mark at `starframe 3200`, source rows 101-195 |
| 8000 | 7586 | the survey's and the reference's text-free late field |

Tick 0 is the part's first `do_stars` iteration: it draws the first field into
the back page and the CRT still shows the never-written page. Tick 1 (boot
415) is the first frame with stars on it, which is what the reference's probe
finds. `the_run_is_numbered_in_its_own_frames_and_draws_320x400` asserts tick
0's picture is all index 0 and tick 1's is not.

## 4. The stars and the ring

**The LCG is not the new seed's high word.** `random` (`STARS.ASM:33-42`)
does `mul` of EAX = `0x343FD` by the seed, adds `0x269EC3` to the low half
only, stores that as the seed, and returns **`dx`** — bits 32-47 of the
*product*, not the new state's high word. The seed starts at 0. The first
twelve returns are `0, 126, 25208, 45290, 7958, 4852, 38525, 27318, 39895,
10061, 34431, 60585`, and the first table entries `(z,x,y)` are
`(255,-512,-386)`, `(254,120,-278)`, `(253,278,244)`, `(252,125,182)`
(`the_rng_is_the_originals_lcg_and_its_first_values_are_the_sequence`).

**The projection is the tables', not a floating-point imitation.** `z` is read
through `movzx` and `z -= 2` with the byte's borrow (`:288-290`). The y product
is shifted right 14 through `shld dx,ax,2`, 100 is added, and rows above 99 are
skipped **unsigned**; the x side uses 160 and 319. At z = 0 the tables are
`muldivy = 23592`, `muldivx = 31457`; at z = 255 they are 3024 and 4032. Rows
0-99 and columns 0-319 are the whole bitmap; the boundary examples are in
`the_projection_tables_are_the_originals_integer_divisions` and
`the_projection_places_the_star_and_clips_at_the_sources_bounds` (e.g.
`(z,x,y) = (0,0,-69)` draws at `(0,160)`, `(0,0,-70)` is row 100 and skipped,
and `(255,-512,-512)` draws at `(5,34)`). The bands are `z >= 180` → plane 0,
`110 <= z < 180` → plane 1, `z < 110` → both.

**The counts.** `staradd` walks the first 512 entries and `staradd2` all 1024.
`starlimit` starts at 512, falls by 1 a call, and from `starframe` 1201 falls
by 4 a call; an entry draws only when `bp >= starlimit`. `starframe` 901
through 1200 draws nothing at all, so the screen holds the ring's stale unit
for 300 frames. The mark at 1500 sets `starlimit = 1024`, which lets the field
fill in four entries a frame again.

**The ring is the picture.** 64 units of 8 KB: plane 0 at bytes 0-3999, a gap,
plane 1 at 4096-8095. `starfetch0` takes `(previous + 1) & 63`; the top half
of the screen reads that unit's rows 0-99, and the bottom reads unit
`(u + 32) & 63` with the rows reversed (`:726-770`) — a 32-frame-old mirror
(`the_blit_reads_the_fetched_unit_on_top_and_the_plus_thirty_two_reversed`
plants marker bytes and finds row 99 of the `+32` unit at page row 100).
Every pass **ORs** into its unit, and no unit is ever cleared after `init`, so
each reuse accumulates another generation of stars and the field densifies
monotonically: the reference's mean byte grows across the keyframes
(7.69 → 24.81 → 42.51 → 57.24; §8).

`staradd2` writes **two** units in one pass. `starfetch1` (`:260-267`) maps the
logical page `(u + 17) & 63` into EMS physical page 1 and returns a segment
there; the routine writes every bit to `es` and `fs` (`:378-397`). The port
keeps both. The survey's 1700 frame is the measurement: with both writes the
port's page is byte-identical to `out/b5-su-sweep1/frame_001700.rgb`; with only
the fetched unit it differs in **729 pixels** (2,187 RGB bytes)
(`the_plus_seventeen_write_is_what_the_survey_frame_shows`, an ignored
measurement test; `out/` is gitignored and §22.4's capture command makes the
frame).

The wrap is exactly 64 ticks and the persistence is exact:
`the_ring_wraps_every_sixty_four_ticks_and_never_clears` draws one star into
unit 37, hides it, runs a whole ring, and finds the bit still there when unit
37 is fetched again; `the_second_star_pass_writes_the_same_bits_into_the_plus_seventeen_unit`
pins the `u`/`u + 17` pair. `init_stars`'s 100 opening passes leave
`emmpage4 = 36` and `starlimit = 412`, so the first displayed field is the
100-pass accumulation and not a bare table.

## 5. The text wipe

`risetext` (`:413-538`) is a bottom-anchored reveal. Let `A` be its
`startxtuse` = `min(startxtopen, startxtclose)` after both counters step:

```text
startxtopen  -9999, +1 a call, capped at 99; the marks reset it to -256
startxtclose 10000, -1 a call, floored at 0; the marks reset it to 1500
row range    150-A to 149; the copy begins at 151-A
copies       A-4 source rows, from `startxtp0/80`
```

The band is bottom-anchored: its odd page rows are `2*(151-A)+1` through
**293** for every `A`, and the two clears are the odd rows just above it,
`2*(150-A)-1` and `2*(150-A)+1`; the closing clear is row 295. So the band's
top moves up as `A` grows while its bottom stands still. Measured, the 1900
keyframe's text occupies 28 odd rows at scanlines **239-293**, which is
`A = 32` and source rows 1-28
(`the_text_and_star_layers_are_on_opposite_row_parities`). The source's first
pixel row with ink is row 11 (21 pixels; plane 0 has 12 of them), which is why
the first *visible* line at 1900 is source row 11 at scanline 259, not the
band's top at 239 (`the_text_strip_is_read_as_its_two_planes`).

The two marks are `starframe` 1200 (block 0: `startxtp0 = 80`, source row 1)
and 3200 (block 1: `startxtp0 = 101*80`, source row 101). `A` opens over 99
frames, holds at 99 while `close` is above it, and closes over 99. The one
special path is `A = 2`'s 80-byte clear (`:528-537`), which crosses one 40-byte
row pair and therefore touches an even (star) row; the port keeps it.

## 6. D24: the text is the divergence, and which artifact each decision follows

`MAIN/DATA/DDSTARS.EXE` is the shipped, **packed** build; `KOE.OBJ` in the tree
is the source build (`DDSTARS/MAKEFILE:2-3` links it and copies it there). The
tree's star-clip tests are assembled out: the `cmp di,_nostar1/_nostar2`
guards are commented out at `STARS.ASM:692-696`, `:711-716`, `:737-742` and
`:756-762`, and `risetext` still computes `_nostar1/_nostar2` (`:438-445`,
`:526`). The reference nevertheless shows the text.

**The port follows the reference, and the tree agrees with it at every
measured frame.** It keeps the text visible and does **not** clip the stars —
the tree's assembled-out state. The evidence is the four keyframes: 0
differing bytes each (§8), so the packed release and the tree's pictured
behavior are the same picture at 1900, 3000, 5000 and 8000. The only frame the
clip could change is the `A = 2` clear, which crosses a star row; no filed
frame holds it.

**There is no `.idx` to follow.** The emulator refuses the indexed read for
this screen (16-colour-named; `reference-indices.md` §22.5), so the reference
is four `.rgb` files and the comparison is RGB. The port's index composition
is still the source's: planes 0-2 and the set/reset text bit.

## 7. The palette

`do_stars` writes DAC entries 0-7 every tick while `starpalfade <= 32`
(`:559-648`), then stops; the live DAC holds the last upload. Entries 1-3 are
scaled by `b = 8*(starpalfade+1)` saturating to 255 on the shift's carry:
`(17,21,26)`, `(25,32,38)`, `(42,53,63)`, each `*b >> 8`. Entry 4 is black and
entries 5-7 are the fixed `(10,20,35)`, `(20,30,45)`, `(30,40,60)`.
`the_palette_fade_is_the_sources_thirty_three_ticks` walks it: tick 0 uploads
`b = 8` (entry 1 black), ticks 31 and 32 both upload 255, and tick 33 stops
with the DAC held. The reference's keyframes are past the fade, and their six
non-black colours are the full-fade entries (§2).

## 8. The reference comparison

The reference's four keyframes are all moving (`reference-indices.md` §22.3),
so D19's support is a frame each side. The port reproduces them **byte for
byte**:

| boot | tick | differing bytes | distinct colours | mean byte |
|---|---|---|---|---|
| 1900 | 1486 | 0 | 7 | 7.6920 |
| 3000 | 2586 | 0 | 7 | 24.8134 |
| 5000 | 4586 | 0 | 7 | 42.5059 |
| 8000 | 7586 | 0 | 4 | 57.2442 |

- `the_four_keyframes_match_the_reference_at_the_mapping`
  (`src/parts/ddstars.rs`) runs the part to each tick without the surface
  composition, renders the displayed page through the live palette, and
  asserts 0 differing bytes; it also computes and asserts the colour counts
  and means above, and asserts the `K-1`/`K+1` neighbours are different
  pictures.
- `the_four_keyframes_and_their_support_frames_match_at_the_mapping`
  (`tests/ddstars.rs`) does the same through the real binary: the dump's
  `ddstars_001486.rgb` and `ddstars_007586.rgb` are compared with
  `reference/parts/ddstars/frame_001900.rgb` and `frame_008000.rgb` (and the
  other two) and are identical.
- The neighbours the port renders are the same pictures the survey recorded:
  §22.3's `K-1`/`K+1` hashes. `sha256sum` on the dump the command block below
  makes gives the prefixes `de6d1676`, `a1697853`, `f768ffbb`,
  `5eb100a6`, `062d91c4`, `0af0a960`, `fb4b902c`, `328201f6` at ticks 1485,
  1487, 2585, 2587, 4585, 4587, 7585 and 7587 — §22.3's table, entry for
  entry.

The reference files' own digests are checked by
`cargo xtask reference verify --tree reference` (its `ok parts/ddstars: 4
frame(s), 4 file(s)` line).

## 9. The music, the sync band, and why this part makes A15 real

The part makes **no music calls**: no `dis_musplus`, no `dis_sync`, no
`dis_muscode`. The block's cue is the shell's — `SECOND U`'s `restartmus` of
MUSIC0 at order 70 (`MAIN/U2.ASM:889-894`) — and `music_for_part("ddstars")`
returns `(MUSIC0, 70)` through block U's plan; the integration test asserts the
manifest's row. Where the shell put the player at the mapped tick is measured,
not read by the part: at tick 7586 the port's published position is `30:51`,
while the filed `hiddenu2` trace's boot-8000 frame stands at `30:54`. The three
rows are the reference's restart landing before the mapping's zero; the part
reads neither number, and the picture is identical either way
(`the_four_keyframes_and_their_support_frames_match_at_the_mapping` asserts
both).

**A15's band.** `dis_sync` answers with the table's last code above the
sixteenth key (`0x4200 < key <= 0x8E2E`) and refuses past it; the `hiddenu`
trace holds `(order << 8) | row > 0x4200` for **5,148 of its 6,351 frames
(81.1 %)** (`docs/fidelity/music.md` §11). DDSTARS never calls it, but it is
the reason that band is not a corner: the hidden part is the demo's longest
single run — the reference was still densifying at frame 20,000
(`reference-indices.md` §22.1) — and its music is the module that spends the
most frames in the band. Any Phase-4 part placed here that waited on
`dis_sync` would meet the answered band immediately, which is what A15
registers.

## 10. Commands

```bash
# the part's arithmetic and the four keyframes, without the process
cargo test -p secondreality --bin secondreality ddstars

# the ignored measurement: the survey's sweep frames at the mapping, and the
# 1700 frame's one-unit counterfactual (needs out/b5-su-sweep1; §22.4's
# capture command reproduces it)
cargo test -p secondreality --bin secondreality the_plus_seventeen_write -- --ignored

# the real binary: numbering, the music row, the four keyframes and support
cargo test -p secondreality --test ddstars

# the survey capture the ignored test reads (reference-indices.md §22.4's
# recipe, with the sweep's frame list)
cargo xtask reference capture --arg U --boot autoexec \
    --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames 500,700,900,1100,1300,1500,1700,2100,3000,5000,8000 \
    --out out/b5-su-sweep1

# §8's support hashes: the keyframes and a frame either side, then sha256sum
cargo run -p secondreality -- --part ddstars --headless \
    --dump-frames 1485,1486,1487,2585,2586,2587,4585,4586,4587,7585,7586,7587 \
    --dump-dir out/ddstars-dump
sha256sum out/ddstars-dump/ddstars_*.rgb

# the reference tree's own digests
cargo xtask reference verify --tree reference
```
