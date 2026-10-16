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
   unchanged for 1147 − 580 = **567** frames. That number, and the two indices it is the difference
   of, now live beside the check that compares the port's cue with it — `HOLD_TICKS` and
   `FADE_OUT_FIRST_INDEX` in `bins/secondreality/src/parts/endlogo.rs`'s tests, where the part used to
   carry them (§5).
2. **The fadeout ends inside (1200, 1240].** The source's 64 steps from 1147 run to 1147 + 63 =
   **1210**, and `SECOND 5`'s change of geometry to CRED's mode 13h screen is in (1200, 1240]
   (`reference-indices.md` §20; §19.1's own sweep: 1200 is "the fadeout's tail, 30 colours, mean
   4.29", and 1240 is refused as `320x200 where the capture started at 320x400`). 1210 is inside it.

**The record's own numbers, reproduced.** The test re-derives nine of the record's own values —
colour count and mean, to two decimals over 384,000 bytes — from the two constants: **430, 460, 500,
540, 1160 and 1200**, which §19.1's table lists separately, and **580, 900 and 1120**, three samples
of its grouped `580 … 1120` row (900 is also §19.2's held keyframe). It also asserts that those three
are one frame byte for byte. That is what a sweep run for another purpose is worth here: the numbers
were recorded before this task, they are a prediction of the two constants rather than a fit to them,
and a constant one frame out moves every sample inside the ramp it belongs to and breaks the hold's
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

**The surface is not cleared, and the DAC's entry 255 is not written** — the source's own two
silences. **The first is visible in a reference frame.** `frame_000430.idx` is index 0 in all 128,000
pixels, which is `inittwk`'s cleared memory and nothing this part writes, and
`the_opening_screen_is_the_shells_surface_through_the_shells_dac` pins it. **The second is not:**
entry 255 is written by no path of this part, and it is *displayed* by none either — the screen at 430
is index 0 and the picture indexes 0-71 — so the artefact that pins it is the marker value
`the_white_leaves_entry_255_unwritten` leaves at 255 in the rig's palette, not a frame. That is
Review Focus 3's contrast with BEGLOGO, which fills entries 0-254 with the same white and then writes
**black** into 255 (`BEG/BEG.C:25-34`, `docs/fidelity/beglogo.md` §6): the two DACs differ in exactly
the entry neither part displays, which is why the two screens are the one `sha256` and why the
difference is a *state* the next part inherits rather than anything either part shows.

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
| 176-742 | 580-1146 | `:107-111` | the hold: the picture through its own palette — 567 ticks, and the music's, not a constant (§5; the map is the reference's boot, and the port's ticks are whatever the cue says) |
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
rectangle rather than scanning one out. The music gate is modelled, and §5 is how.

## 5. The music gate, which is the source's own loop

`:107-111` is `for(a=0;a<5000 && !dis_exit();a++) { dis_waitb(); if(dis_musplus()>-16) break; }`. The
port runs it: one iteration a tick — `dis_waitb()` *is* the tick, one band pass — the 5000-iteration
cap stays, and the condition is the run's player's `musplus()`
(`crates/sr-engine/src/music.rs`'s [`musplus_from`], the DIS's own arithmetic over the published
`_np_zplus` and `_np_row`). **No *hold length* is in the part.** The hold is 567 ticks on the
reference's boot because the music says so, and the same run with any other music would have that
music's hold. The gate's own numbers are its threshold and its cap (the source's `-16` and `5000`) and
the **one lever** the alignment below adds — a frame, not a length.

**This is where the phase's proof lands, and it lands in two pieces.** A7 carried the gate as its
*measured* length, 567 ticks
derived bit-exactly from the reference's **frames** (§3 — the fade-in's last step displayed at 579,
the fadeout's first at 1147). The port's cue is derived from the *player's state* — a different
measurement of a different thing — and the two meet on the reference's boot, with **one frame that
the cue supplies and the derivation does not** (§5.2): the placement below moves the break from the
port's own crossing (1144) to **1145**, which is one of the two frames between the port's words and
the reference's frames, and the second is the alignment. The rows:

| | index | what |
|---|---|---|
| the port's published words cross (`musplus` from −16 to −15) | **1144** | `26:49` at `zplus` 1 — `row − 64` |
| the tick the check breaks on (§5.1) | **1145** | the source's `break`, drawn on that frame |
| the tick the port holds past it (§5.2) | **1146** | the alignment |
| the fadeout's `c = 63` — the first step below the picture | **1147** | 1180 − 33, from the reference's own frame |
| the hold: 1147 − 580 | **567 ticks** | from two keyframes, 500 and 1180 |

and the four keyframes the acceptance compares are still `0.0000 %`, both planes byte-identical
(§7). The test that says so is
`the_gate_opens_on_the_tick_the_reference_frames_derived` (`src/parts/endlogo.rs`), which drives the
gate with `reference/music/second5`'s own words a tick at a time; and
`the_runs_own_player_opens_the_gate_on_the_same_tick` drives the same part through the shell, with
the run's own player, and compares the composed frames with §3's arithmetic.

### 5.1 Why the check reads the *previous* tick's words

The source's check is the instruction after `dis_waitb()`, so it stands at a frame boundary; and
`dis_waitb()` returns on the copper's own boundary. `waitb_1` (`DIS/DISINT.ASM:161-181`) spins on
`cs:copperframecount`, which `copper_end` (`MAIN/COPPER.ASM:283-320`) increments (`:320`) right after
its wait for the vertical retrace (`:305-311`). The music's poll is somewhere else entirely: it is
`call _zpollme` (`MAIN/COPPER.ASM:466`) inside `copper_doit`, and the copper's list runs that at 20 of
a frame's 100 units (`copperlst0`, `:41-43`) — a fifth of a frame *after* the boundary the check
stands on. So the check reads the words the **previous** frame's poll published, and the port's engine
— which polls between copper slot 0 and the first band
(`crates/sr-engine/src/engine.rs:206-219`), i.e. before the part's code in the same tick — hands a
part the words of the tick's *own* poll. The part keeps the reading a tick: the tick it breaks on is
the tick the source's `break` happens on.

### 5.2 The one frame that is an alignment, and what it costs

The port's own player publishes `26:49` from index **1144** — measured, and it is `second5`'s own
player: the trace's frame `N` sample is taken at frame `N`'s *start* (the emulator publishes a frame's
pixels and increments its counter immediately afterwards, D17's own reading), so `second5`'s frame `N`
holds the words the port publishes at index `N − 1`, and the port's row 49 begins at 1144 where the
trace's begins at 1145. With §5.1's placement the check therefore breaks on index **1145**, and the
fadeout would start at **1146** — while the reference's frames put the fadeout's first step at
**1147** (§3, bit-exactly from 500 and 1180).

**The port's player is not the one that moved, and the measurement that says so is quantitative.**
The port's published positions differ from the trace's by one frame at 100 of this window's position
changes, agree at 22 and differ by two at 8 — 130 in all, of the window's **131**: the one the
comparison skips is the window's **first** sample, which has no earlier frame for it to be a change
from (`src/shell.rs`'s `if *frame > start`), and the last change, at 1295, *is* paired. Reproduced by
`cargo test -p secondreality --bin secondreality the_runs_music_is_where_the_trace -- --nocapture`,
which prints `130 position change(s) compared; the port's frame minus the trace's: {-2: 8, -1: 100,
0: 22}` and fails if any change is more than two frames out. That ratio is what the poll geometry
predicts: the port matches a session exactly when a row change falls in the fifth of the frame between
the port's poll (the tick's start) and the original's (a fifth in), one match in five, against
22/130 = **16.9 %** measured. A clock that had *shifted* would match at **none** of them. So the −1 is
a sub-sample phase effect of where in the frame the poll and the sample sit, not a wrong player, and
the one frame the gate is out by is the session's — which is what this section's next paragraph names.

**The two measurements are one frame apart, and the frames are the record** (D12). The port holds one
frame past its own break (`GATE_ALIGNMENT_TICKS`, `src/parts/endlogo.rs`), which is where the 567
comes from, and the cost is named: **the port disagrees with `second5`'s own session by one frame at
this crossing.** That is inside the spread the sessions themselves have — the two filed sessions of
one recipe disagree with each other about which frame eight of their row changes land on, and the port
sits between them (`APPROXIMATIONS.md` A12, `JITTER_FRAMES` 3; the port's own worst displacement from
`second5` is 2 frames, 778 of 894 frames exact) — and it is inside D17's finding that a frame counter
names one picture in a session and a picture one frame away in the next. **Which measurement moved is
therefore nameable too:** not the anchor (Task 1 measured the first tick at 408.0 in four sessions,
spread 0.1) and not the player (the port's direct `_np_zframe` matches `second5`'s own column once the
sample instant's one-index shift is taken out — 753 of 891 frames exact and never more than **1 tick**
apart, asserted frame by frame in `src/shell.rs`'s
`the_runs_music_is_where_the_trace_holds_it_at_every_frame_the_trace_has`, `ZFRAME_SLOP`; the same
test prints the *unshifted* reading beside it, **353 of 894, worst 2 ticks** — the one-tick shift plus
the one tick the shifted reading already allows, and nothing that grows with the window, which is what
says the one-index offset is the sample instant rather than a drifting clock), but the
*session* — `second5`'s published row 49 begins one frame earlier, relative to the
frame indices, than the session the reference's four keyframes were captured in. A run whose music
starts elsewhere has no such alignment to make, and the constant is a fact about this boot and
`second5`, not about ENDLOGO.

The gate's `!dis_exit()` half has no port analogue either, and that is a property of this task rather
than an omission: a part's `PartCtx` is built with `exit_requested: false` and a part can only
*request* an exit (`crates/sr-engine/src/part.rs:79-103`, `:139-147`), so no part observes the run's
exit flag. The port's gate therefore runs the loop's *music* half and not its keyboard half.

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
    --dump-dir out/t8-part
cargo run --bin secondreality -- --headless --block 5 --dump-frames 430,500,900,1180 \
    --dump-dir out/t8-block5
cargo xtask compare --reference reference/parts/endlogo --rendered out/t8-part \
    --out out/t8-part-diff
cargo xtask compare --reference reference/parts/endlogo --rendered out/t8-block5 \
    --out out/t8-block5-diff
```

```
frame 430: 0.0000% differing (0/128000 px), max delta 0
frame 500: 0.0000% differing (0/128000 px), max delta 0
frame 900: 0.0000% differing (0/128000 px), max delta 0
frame 1180: 0.0000% differing (0/128000 px), max delta 0
(…)
verdict: identical
exit: 0
```

(`(…)` marks the tool's `report: <path>` and `captures: 1 measured` lines, dropped here and named
below; `exit: 0` is the shell's own reading of the command's exit code, not a line of its output.)

**Both dumps are the acceptance, and both were re-run with the cue in place** (Task 8): the
`--part endlogo` one and the `--block 5` one, which differ only in their plans (a one-part run and the
block's three) and compare identically — `out/t8-part-diff/report.json` and
`out/t8-block5-diff/report.json`, `mode: mixed`, every offset 0, the same four lines of output under
either. The block printed above is the first; the second's
report is the file named beside it. The frames are the Phase 2 ones, byte for byte, with a real cue
behind them: `--block 5` is the run whose music the phase built, and
1180 is the frame that would move if the gate opened on any tick but §5.2's.

`mode: mixed`, every offset 0 (`out/t8-part-diff/report.json`, offsets `{"0": 4}` — the same file
Phase 2's run wrote as `out/p2-endlogo-diff/report.json`). **430 and 900 go through the indexed
path** — 430 because the reference files an `.idx` for it as well as its
partners, and 900 because §19.3's census shows the reference's `.idx` there is a real one (72
distinct indices, 69 distinct triples, no index carrying two, checked against the manifest's own
palette) — and 500 and 1180 through RGB, because an indexed read is 20+ fade steps late and D15's
coherence rule refuses those pairs. So the DAC's 6→8-bit widening is load-bearing on the middle pair
and bypassed on the outer pair, exactly as §3.1 of BEGLOGO's note predicted for the same mechanism.

The port's own sheet, `out/p2-endlogo-sheet.png` (`cargo xtask frames sheet --dir out/p2-endlogo
--out out/p2-endlogo-sheet.png`: 4 tiles, 2 columns, 650×810), is the visual evidence, and it is
beside the reference's own sheet, `out/endlogo.png`, written by the capture that produced this
reference. **The two are byte-identical** — `sha256sum out/endlogo.png out/p2-endlogo-sheet.png`
returns `4a3afb5c…` for both, 281,156 bytes each, from two different tools run a day apart — which is
what four byte-identical frames of the same geometry and the same sheet layout have to produce.
`out/` is git-ignored scratch, so the durable form of the claim is the keyframe comparison above; the
two PNGs are the readable face of it. **The human look at these pictures is deferred to the phase
boundary**: nothing here claims a person has judged them.

## 8. Tests

`src/parts/endlogo.rs` (unit, fifteen): the 765 bytes leaving entry 255 unwritten, at both `:63` and
`:91` and across the fades; the opening screen as the shell's surface through the shell's DAC; the
blit landing on tick 36 / index 440 and not before 430; the fade-in's ramp and its truncating divide
(including the `c = 1, palette = 0` case where it lands on 62, one below a rounded ramp); the
fadeout's ramp *and* that it is not the fade-in reversed; the 510-byte upload's exact edges; the part
retiring on the tick after the fadeout's last step; the derivation scan of §3; §19.1's own sampled
states reproduced — and **Task 8's five new tests plus one rewritten**, which are the gate's:
`the_gate_opens_on_the_tick_the_reference_frames_derived` (§5's table, driven tick by tick with
`reference/music/second5`'s own words), `the_runs_own_player_opens_the_gate_on_the_same_tick` (the
same, through the shell with the run's own player and the composed frames compared against §3's
arithmetic), `the_gates_check_reads_the_previous_ticks_words` (§5.1's placement),
`the_gate_holds_for_five_thousand_ticks_when_the_cue_never_comes` (`:107`'s own cap) and
`a_run_with_no_player_is_refused_by_name` (`init`) — and
`the_hold_is_the_cues_own_length_and_not_five_thousand`, which is Phase 2's
`the_hold_is_the_measured_length_and_not_five_thousand` renamed and rewritten: it now walks the hold
with the trace's words and asserts its length is the difference of the two indices §3 fixed, where it
used to read the port's own constant.

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

**Mutation checks (the tests' discriminating power, measured).** Each mutation is one edit to a
constant in `bins/secondreality/src/parts/endlogo.rs` followed by
`cargo test -p secondreality --bin secondreality parts::endlogo`, and the table records which tests go
red — the two bracket mutations take more than the derivation test, because a wrong first index moves
the sweep and the hold's arithmetic too:

| mutation | red | message |
|---|---|---|
| `FADE_IN_FIRST_INDEX` 451 → 450 (the plan's bracket) | the derivation test, the hold, the blit and the sweep | `the fade-in's step 0 is displayed at 451 / left: 451 / right: 450` |
| the check's `FADE_OUT_FIRST_INDEX` 1147 → 1145 (the plan's bracket) | the check, the derivation, the hold, the retire, the sweep and the run test | `left: 1147 / right: 1145`, and on the walks `left: 1208 / right: 1210` and `left: 739 / right: 741` |
| **`GATE_ALIGNMENT_TICKS` 1 → 0** (§5.2's alignment dropped) | the check, the placement test, the hold, the retire and the run test | the walks move a tick earlier: `left: [48,45,42] / right: [47,44,41]` on the frame §5's table puts the fadeout on |
| **the check reading the tick's *own* words** (§5.1's placement dropped) | the check, the hold, the retire and the run test | the same shape, one tick the other way |
| the white written over entries 0-255 | `the_white_leaves_entry_255_unwritten` | `:63's 765 bytes stop short / left: [63,63,63] / right: [1,2,3]` |
| `setpalarea` with the intended `3*count` | `the_upload_is_the_originals_two_count_plus_start_bytes` | `the 511th byte is never written / left: [42,5,6] / right: [1,2,3]` |
| the blit moved onto the fade's first tick | `the_picture_is_blitted_after_the_vertical_blanks` | `the blit is the tick after the last blank / left: 47 / right: 36` |
| the fadeout's first step taken at the picture (`c = 64`) | `the_hold_is_the_cues_own_length_and_not_five_thousand` | `left: [48,45,42] / right: [47,44,41]` — entry 1 one step up |
| `BLANKS_TICKS` 32 → 31 (the lead-in one tick short) | `the_picture_is_blitted_after_the_vertical_blanks` | `left: 31 / right: 32` |

**The gate's three rows are the ones Task 8 added, and they are the phase's three ways to fail it**:
its alignment dropped, its placement dropped, and the check's own measurement moved — each takes the
tests that read the frame the fadeout's `c = 63` is displayed on, which is the frame the reference's
keyframe at 1180 counts back from. **And one mutation is now impossible, which is the point of the
task**: there is no *hold length* in the part to edit — the one lever left is `GATE_ALIGNMENT_TICKS`,
the disclosed frame of §5.2, and it moves the gate's frame 1:1 (the row above) — so the two rows that
used to be "the plan's brackets" on the port's constants are the *check's* constant instead, and
moving it makes the check disagree with the cue rather than the cue with the
reference. The two rows that share `the_picture_is_blitted_after_the_vertical_blanks` are still the
two ways the lead-in's map can be wrong: the blit on the wrong tick, and one blank short — which is
why that test also pins the lead-in's counts (34 blanks, 37 ticks the source's code accounts for, ten
not modelled, `BLIT_TICK` 36) rather than leaving them to the note's prose.

The first four are the traps Phase 2's task was written around; the last two are the two ways a port
that "looks right" fails a keyframe on this part; and the gate's three are the ways a port that runs
the source's loop but reads it at the wrong instant — or with no alignment — fails the same keyframe
from the other side.
