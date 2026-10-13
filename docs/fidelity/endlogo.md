# ENDLOGO — the end picture: the white, the 129-step fade, the hold, the 64-step fadeout

Slot 18 of `part-sequence.md` §1 and the entry point of `SECOND 5` (`partsmask[5]` = `10000b`,
`MAIN/U2.ASM:116`), built from `END/`. 320×400 Mode X, 128,000 pixels of output, and **312 lines of
source in three files** — `END/END.C` (130), `END/ASM.ASM` (118) and `END/READP.C` (64). The
reference is `reference/parts/endlogo/` — 430, 500, 900, 1180 (`reference-indices.md` §19).

**The acceptance comparison is exit 0**: `mode: mixed`, four frames at offset 0, 0.0000 % differing,
0 pixels of 128,000, max channel delta 0 (§7).

The port is `bins/secondreality/src/parts/endlogo.rs`, registered as `endlogo`.

## 1. What the original does

```
:50      dis_partstart()                       the version check, not a wait (§5)
:51-60   AC: index 0x11 = 255, then 0x20       unlock the AC; overscan 255
:61      dis_waitb()                           one vertical blank
:62-63   outp(0x3c8,0); 765× outp(0x3c9,63)    the DAC: 765 bytes of 63
:64-66   // mode 13h + inittwk                 commented out: the shell did it (§2)
:67-74   CRTC 0x0c = 0, CRTC 0x0d = 0          display start address 0
:75-80   CRTC 9 &= ~0x80, &= ~0x1f             a 400-line display, one scanline a row
:81-87   AC: index 0x11 = 0                    the overscan colour, and PAS
:89      dis_waitb()                           one vertical blank
:90-91   outp(0x3c8,0); 765× outp(0x3c9,63)    the same 765 bytes again
:92      for(a=0;a<32;a++) dis_waitb()         32 vertical blanks
:94      readp(palette,-1,pic)                  the picture's palette out of the container
:95-99   readp(rowbuf,y,pic); lineblit(vram+y*80,rowbuf)   400 rows
:101-106 for(c=0;c<=128;c++) { pal2 = ramp(c); dis_waitb(); setpalarea(pal2,0,255); }
:107-111 for(a=0;a<5000 && !dis_exit();a++) { dis_waitb(); if(dis_musplus()>-16) break; }
:112-117 for(c=63;c>=0;c--) { pal2 = (palette×c)/64; dis_waitb(); setpalarea(pal2,0,255); }
:118-129 /* the DOS epilogue and the scroller */  commented out
:130     }                                      main() returns
```

The screen never moves after `:99`; everything from 451 to 1210 is the DAC. The code's own helpers:

- **`lineblit`** (`END/ASM.ASM:53-82`, the four-plane loop at `:63-76`) is the routine BEGLOGO links
  too: for each of four planes it writes 80 bytes from `ds:[si+zzz*4+zpl]` to `es:[di+zzz]`, i.e.
  every fourth byte of the row buffer into one plane, at plane offset `y*80`. Four planes × 80 bytes
  = 320 pixels.
- **`setpalarea`** (`:93-115`) is the ranged DAC upload, and it ships with the byte-count bug the port
  preserves (§6).
- **`readp`** (`END/READP.C:10-64`) is the Origin RLE row reader — `row = -1` copies `cols*3` bytes of
  palette (`:17`), a row walks the row-offset table (`:21-28`) and expands one run-length stream
  (`:36-59`). It is `BEG/READP.C` line for line.

## 2. Which parts of the machine the part does *not* own

Three things `END.C` relies on the shell for, and each is a trap for a port that reads `END.C` alone:

- **The video mode.** `:64-66`'s `mov ax,13h+80h` / `int 10h` / `inittwk()` are commented out, because
  `SECOND 5` does it before `exe2021`: `MAIN/U2.ASM:906-908` is `mov ax,13h` / `int 10h` /
  `call inittwk`, and `:905` is the `restartmus` that puts MUSIC0 at order 25 first. `inittwk`
  (`END/ASM.ASM:7-50`) sets CRTC 9 = 0 (`:22-24`) — a 400-line display — takes the address counter to
  byte mode with `CRTC 0x14 = 0` / `CRTC 0x17 = 0xe317` (`:26-30`), sets the sequencer's memory mode
  and map mask (`0x0604`, `0x0f02`, `:31-37`), and clears 64 KB of video memory to index 0 with
  `rep stosw` (`:38-43`). `END.C` then writes CRTC 9 again for itself at `:75-80`, clearing the same
  bits.
- **The cleared video memory.** There is no `memset` in `END.C` at all. The screen the part's first
  frames show is `inittwk`'s zeroed memory through the DAC the mode set left — which is why
  `reference/parts/endlogo/frame_000430.idx` is **index 0 in all 128,000 pixels** and white only
  because `:62-63` has by then written 63 into entry 0. A port that blitted the picture before 430
  would render the same *pixels* (entry 0 is white either way) and fail the comparison, because the
  reference files an `.idx` for 430 and `compare` takes the indexed path whenever both sides have one.
- **The byte-mode CRTC write.** `inittwk`'s `CRTC 0x17 = 0xe317` (`END/ASM.ASM:26-30`) is part of what
  makes mode 13h's one-byte-per-pixel addressing reach this 400-line screen. It has no analogue in a
  chunky model at all — the port's surface is chunky by construction and its rows are its own stride,
  so there is no plane interleave to address and no byte/word mode to select. The port draws through
  `Surface::set` with `y_line_offset` unset (Part B's first convention) and the beam reads back
  exactly the picture (`docs/fidelity/beglogo.md` §5 states the same convention for the same
  `lineblit`).

## 3. The two fade constants, derived by matching the reference bit for bit

Both are read off `reference/`'s own frames by exhaustive match, the method Task 4 established: for
each keyframe, the ramp is computed from the source's arithmetic (`:103` and `:114`), widened with
the engine's `(c*259+33)>>6`, rendered over all 320×400 pixels of `assets/END/PIC.UH`, and compared
**byte for byte**. The search covers the whole range of each ramp, and the answer has to be unique.

| frame | ramp | `c` values whose 320×400 RGB is that frame, byte for byte |
|---|---|---|
| 500 | the fade-in, `((128-c)*63+palette[a]*c)/128` | **[49]** — unique over 0…128 |
| 1180 | the fadeout, `(palette[a]*c)/64` | **[30]** — unique over 63…0 |
| 900 | the fade-in, in the hold | [126, 127, 128] — the ramp has converged; the three are one palette |
| 430 | — | the screen is one index (0) through a white DAC, before the blit |

`the_fade_constants_are_derived_by_matching_the_reference_bit_for_bit`
(`bins/secondreality/src/parts/endlogo.rs`) is that scan. The two constants follow from the middle
two rows:

- **`FADE_IN_FIRST_INDEX` = 500 − 49 = 451.** The fade-in counts *up* from `c = 0`, and each step is
  displayed one frame later, so the step displayed at 500 is the fiftieth and step 0 is 451.
- **`FADE_OUT_FIRST_INDEX` = 1180 − (63 − 30) = 1147.** The fadeout counts *down* from `c = 63`, so
  the step displayed at 1180 is the 34th and `c = 63` is displayed at 1147.

**Why a fit gives a different answer, and why that answer is wrong.** `reference-indices.md` §19.1's
sweep means — 237.44 at 460, 167.31 at 500, 99.54 at 540 — put the fade-in's step 0 "near 450", and
the plan carried that as a bracket. 450 is a different frame from 451: the scan separates them on
every pixel whose entry the step moves, and the mutation check in §8 shows the test failing with
`the fade-in's step 0 is displayed at 451 / left: 451 / right: 450` when 450 is written in. A mean is
not linear in `c` — the mean of a truncated integer ramp is a staircase — and three sampled means
cannot tell a whole step from the truncation inside one. The same applies to the fadeout's bracket,
1145, which the same test rejects.

**The two facts the plan asks the constants to be checked against hold, and neither was fitted.**

1. **The hold is real, and 567 ticks long.** `reference-indices.md` §19.1 records that the sweep's
   samples from **580** to 1120 are one `sha256` (`f2514b29…`, 69 colours, mean 33.23), and its
   table's own words for 430-540 are the fade's states, not the picture's. Under the derived
   constants the fade-in's last step is displayed at 451 + 128 = **579**, so the first frame of the
   settled picture is **580** — the record's first byte-identical sample, named by a sweep that was
   run before this task existed — and the fadeout's first step is 1147, so the picture is displayed
   unchanged for 1147 − 580 = **567** frames. `HOLD_TICKS` is that number, and the two indices it is
   the difference of come from two different keyframes.
2. **The fadeout ends inside (1200, 1240].** The source's 64 steps from 1147 run to 1147 + 63 =
   **1210**, and `SECOND 5`'s change of geometry to CRED's mode 13h screen is in (1200, 1240]
   (`reference-indices.md` §20; §19.1's own sweep: 1200 is "the fadeout's tail, 30 colours, mean
   4.29", and 1240 is refused as `320x200 where the capture started at 320x400`). 1210 is inside it.

**The record's own samples, reproduced.** Every state `reference-indices.md` §19.1's table names
separately is checked against the arithmetic: the colour count and the mean, to two decimals, of 430,
460, 500, 540, 580, 900, 1120, 1160 and 1200 —
`the_sweeps_own_samples_are_the_ramps_frames` asserts them, and that 580, 900 and 1120 are one frame
byte for byte. That is what a sweep run for another purpose is worth here: the means were recorded
before this task, they are a prediction of the two constants rather than a fit to them, and a
constant one frame out moves every sample inside the ramp it belongs to and breaks the hold's
arithmetic besides.

**The residual, stated rather than smoothed over.** 900 does not pin the constant: `c = 126, 127 and
128` are three names for a converged palette, so the held frame can only say the ramp had *ended* by
then, and the scan asserts exactly that. What pins the two constants is 500 and 1180, one keyframe
each. 430 pins nothing about the fade either — it is one index through a DAC that is white at every
`c` — and what it pins is the **surface**: no picture is in video memory yet (§2).

## 4. What the port does, act by act, and the tick map

**`init` does the acts that cost no frame**: `:51-60`'s attribute-controller unlock (which ends in
`outp(0x3c0,0x20)`, the index register set to 0 with bit 5 — the palette address source — and PAS = 1
is the state in which the DAC's palette is displayed at all, which the port's engine always is, so
there is nothing to set); `:67-74`'s display start 0 (`View::set_origin(0)`, which zeroes
`start_addr` **and** `x_offset`, both halves of what that 16-bit register is); `:75-80`'s 400-line
CRTC 9 (`view.height = 400`, `double_scan = false`); `:86-87`'s AC 0x11 = 0, the overscan colour
(`border = Some(0)` — the last of the two writes, 255 at `:55-57` and 0 here, wins); and `:94`'s
`readp(palette,-1,pic)` plus the 400-row decode, in one `sr_assets::load_path` call, refused if the
geometry is not 320×400.

**The surface is not cleared, and the DAC's entry 255 is not written.** Both are the source's own
silences (§2, §6), and both are visible in the reference: 430's `.idx` is index 0 everywhere — which
`the_opening_screen_is_the_shells_surface_through_the_shells_dac` and
`the_white_leaves_entry_255_unwritten` pin — and its pixels are white through entry 0.

**`frame` follows the source's vertical blanks one for one.** `dis_waitb()` is one tick in the port,
and the acts that cost no blank fall on the tick after the blank that precedes them:

| tick | index | source | what the screen is |
|---|---|---|---|
| 0 | 404 | `:61` one blank | `inittwk`'s index 0 through the shell's DAC — black, entry 0 being `(0,0,0)` |
| 1 | 405 | `:62-63` | white: 63 written into entries 0-254 |
| 2 | 406 | `:89` one blank | white |
| 3 | 407 | `:90-91` | white |
| 4-35 | 408-439 | `:92`'s 32 blanks | white — **430 (tick 26) is here** |
| 36 | 440 | `:94-99` | the picture is in video memory; the DAC is still white, so the screen is white |
| 37-46 | 441-450 | — | not modelled (§5) |
| 47-175 | 451-579 | `:101-106` | the fade-in, `c = index − 451` |
| 176-742 | 580-1146 | `:107-111` | the hold: the picture through its own palette |
| 743-806 | 1147-1210 | `:112-117` | the fadeout, `c = 63 − (index − 1147)` |
| 807+ | 1211+ | `:130` | `main()` has returned; the loader holds the black screen |

(`:67-88`'s CRTC and AC writes are `init`'s rather than a tick's — they cost no frame, and the port's
view is a description of the display rather than a register write, which is where BEGLOGO's CRTC
block lives too.)

The blit's tick is load-bearing in the same way BEGLOGO's is: 430 is inside the 32 blanks, so video
memory is index 0 there and the *indexed* comparison at 430 sees the shell's surface. The last frame
is the fadeout's `c = 0`, which is black for every index the picture uses (it indexes 0-71, and 0-169
are the entries the 510-byte upload writes), and the tick after it reports
`PartStatus::Finished` — `:118-129`'s DOS epilogue and scroller are commented out and `:130` ends
`main()`.

**What is not modelled, and why it cannot be.** `:118-129` is commented out in the source, so the
epilogue and the "second best endscroller ever" are not part of this part. The CRTC's timing
registers are not written by `END.C` either and have no analogue in an engine that composes a
rectangle rather than scanning one out. And the music gate is not modelled: §5.

## 5. The music gate, as its measured length — `APPROXIMATIONS.md` A7

`:107-111` is `for(a=0;a<5000 && !dis_exit();a++) { dis_waitb(); if(dis_musplus()>-16) break; }`,
which waits on the music player's position. **The port has no music in this phase**, so the gate is
replaced by the length the reference shows it having: 567 ticks, from the two derived constants
(§3) — the picture is displayed from 580 and the fadeout begins at 1147. It is the *measured* length,
not the loop's `a < 5000` bound, and not a number the port chose; the entry in
`docs/fidelity/APPROXIMATIONS.md` carries the measurement, the difference (zero against this
reference; unknown against any other music position or machine speed) and what Phase 3 replaces it
with.

The gate's `!dis_exit()` half has no port analogue either, and that is a property of this phase
rather than an omission: a part's `PartCtx` is built with `exit_requested: false` and a part can only
*request* an exit (`crates/sr-engine/src/part.rs:79-103`, `:139-147`), so no part observes the run's
exit flag. Phase 3's cue replaces the whole condition.

**And ten of the lead-in's ticks are not modelled** — A7's second half, the same class as A5's nine
for BEGLOGO. The source's own code accounts for **37** of the 47 ticks between the part's first frame
(404) and its fade's first step (451): 34 vertical blanks (`:61`, `:89`, `:92`'s 32) and the three
ticks the code between them runs on (`:62-63` with `:67-88`, `:90-91`, `:94-99`). Ticks 37-46 —
indices 441-450 — are frames the port does not model, and their cause is unidentified; the two
candidates are ruled out with the same evidence BEGLOGO's are (not the EXE load: the part's own white
is up by 415, and `inittwk`'s palette clear is commented out, `END/ASM.ASM:14-20`, so that white can
only be `END.C:62-63`'s; and not `dis_partstart()`, which is a version check costing no frame,
`DIS/DISC.ASM:51-64`). The port draws `inittwk`'s screen — index 0 through the shell's DAC — and then
the white field through them, which is what the reference shows at 415 and 430.

## 6. The bug the part ships: `setpalarea`'s byte count — **510** bytes, not BEGLOGO's 508

`END.C:105`/`:116`'s `setpalarea(pal2,0,255)` is `END/ASM.ASM:93-115`, the same routine as
`BEG/ASM.ASM:93-115`: `:104-105` points the DAC's write index at `start`, `:106` loads `count` into
`cx`, `:107-108` are `shl cx,1` / `add cx,ax` — `2*count + start`, with `ax` still holding `start` —
and `REPOUTSB` (`:84-91`) moves exactly that many bytes. `docs/fidelity/bugs.md` §4 is the entry.

**The length is `2*count + start`, and the two parts' calls differ.** `BEG/BEG.C:84` passes 254 and
moves 508 bytes; `END.C:105` passes **255** and moves **510** — entries 0-169 whole, and entries
170-255 untouched. Both source files mean 255 or 254 *entries* and get two bytes of each, which is
the bug; the numbers differ because the calls do. The brief for this task and the plan's Step 3 both
say "508 bytes a step" for ENDLOGO, which is BEGLOGO's number carried across; the source's third
argument is `255` at both call sites (`END.C:105`, `:116`), and `the_upload_is_the_originals_two_
count_plus_start_bytes` pins 510 with the entry-169/170 boundary.

**It is invisible on this art, and that is why it is written down.** The picture indexes 0-71 (the
decode's histogram: 72 distinct indices, 0 through 71), the screen before the blit is index 0, and
both uploads reach entry 169 — so a port that wrote the intended 765 bytes answers every keyframe
identically. The derivation test asserts precisely that: the same scan run with BEGLOGO's 254 (508
bytes) returns the same `c` for 500. What the bug *does* leave is a DAC whose entries 170-254 have
been white since `:90-91` and never faded, going into CRED.

## 7. The comparison

```bash
cargo run --bin secondreality -- --headless --part endlogo --dump-frames 430,500,900,1180 \
    --dump-dir out/p2-endlogo
cargo xtask compare --reference reference/parts/endlogo --rendered out/p2-endlogo \
    --out out/p2-endlogo-diff
```

```
frame 430: 0.0000% differing (0/128000 px), max delta 0
frame 500: 0.0000% differing (0/128000 px), max delta 0
frame 900: 0.0000% differing (0/128000 px), max delta 0
frame 1180: 0.0000% differing (0/128000 px), max delta 0
verdict: identical
exit: 0
```

`mode: mixed`, every offset 0 (`out/p2-endlogo-diff/report.json`, offsets `{"0": 4}`). **430 and 900
go through the indexed path** — 430 because the reference files an `.idx` for it as well as its
partners, and 900 because §19.3's census shows the reference's `.idx` there is a real one (72
distinct indices, 69 distinct triples, no index carrying two, checked against the manifest's own
palette) — and 500 and 1180 through RGB, because an indexed read is 20+ fade steps late and D15's
coherence rule refuses those pairs. So the DAC's 6→8-bit widening is load-bearing on the middle pair
and bypassed on the outer pair, exactly as §3.1 of BEGLOGO's note predicted for the same mechanism.

The port's own sheet, `out/p2-endlogo-sheet.png` (`cargo xtask frames sheet --dir out/p2-endlogo
--out out/p2-endlogo-sheet.png`: 4 tiles, 2 columns, 650×810), is the visual evidence, and it is
beside the reference's own sheet, `out/endlogo.png`, written by the capture that produced this
reference. **The human look at these pictures is deferred to the phase boundary** — this task
recorded that the two sheets' tiles are the same four screens, which is what a byte-exact comparison
already says.

## 8. Tests

`src/parts/endlogo.rs` (unit, ten): the 765 bytes leaving entry 255 unwritten, at both `:63` and
`:91` and across the fades; the opening screen as the shell's surface through the shell's DAC; the
blit landing on tick 36 / index 440 and not before 430; the fade-in's ramp and its truncating divide
(including the `c = 1, palette = 0` case where it lands on 62, one below a rounded ramp); the
fadeout's ramp *and* that it is not the fade-in reversed; the 510-byte upload's exact edges; the hold
as 567 ticks with the DAC not moving through it; the part retiring on the tick after the fadeout's
last step; the derivation scan of §3; and §19.1's own sampled states reproduced.

`tests/endlogo.rs` (five, through the real binary from the repository root): the four keyframes in
the reference's own numbering (`index_origin: boot`, anchor 404, 320×400 each); 430 as 128,000 bytes
of index 0, byte-identical to the reference's `.idx`, and pure white RGB; 500 and 1180 byte-for-byte
against `reference/parts/endlogo`'s own RGB frames; the held 900 equal to `assets/END/PIC.UH` as
decoded *and* to the reference's `.idx`; and the guard that makes those comparisons able to fail at
all.

What the five add over the unit tests is the path a run really takes, end to end: the registry, the
shell's anchor and numbering, the asset read at `assets/END/PIC.UH` from the repository root, the DAC
the shell installed, and the frames the dump wrote to disk and read back. **The reference holds no
`.idx` for either fade frame**, so nothing under `reference/` has to be compared as indices for 500
and 1180 — the two frames who would show a wrong `c` at their own index — and it is the derivation
test that compares those bytes, against the part's own arithmetic rather than against a session.

**Mutation checks (the tests' discriminating power, measured).** Each mutation turns exactly the test
that pins it red and no other, and each is one edit to a constant in
`bins/secondreality/src/parts/endlogo.rs` followed by
`cargo test -p secondreality --bin secondreality parts::endlogo`:

| mutation | red | message |
|---|---|---|
| `FADE_IN_FIRST_INDEX` 451 → 450 (the plan's bracket) | the derivation test, plus the hold and the sweep | `the fade-in's step 0 is displayed at 451 / left: 451 / right: 450` |
| `FADE_OUT_FIRST_INDEX` 1147 → 1145 (the plan's bracket) | the derivation test, the hold, the retire and the sweep | `the fade-out's step 63 is displayed at 1147 / left: 1147 / right: 1145` |
| the white written over entries 0-255 | `the_white_leaves_entry_255_unwritten` | `:63's 765 bytes stop short / left: [63,63,63] / right: [1,2,3]` |
| `setpalarea` with the intended `3*count` | `the_upload_is_the_originals_two_count_plus_start_bytes` | `the 511th byte is never written / left: [42,5,6] / right: [1,2,3]` |
| the blit moved to the fade's first tick | `the_picture_is_blitted_after_the_vertical_blanks` | — |
| the fadeout's first step at the picture (`c = 64`) | `the_hold_is_the_measured_length_and_not_five_thousand` | — |

The first four are the traps this task was written around; the last two are the two ways a port that
"looks right" fails a keyframe on this part.
