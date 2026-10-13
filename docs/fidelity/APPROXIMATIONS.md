# Approximations register

Every place the port knowingly differs from the original. An entry requires a
justification and a measured visual difference; it is not a parking place for "close
enough".

## A1 — Overscan border is padded, not scanned out

**Part:** all; used deliberately by PANIC and the 320×400 parts.
**Original:** the border is the region of the display outside the active area, set by
AC 0x11 and scanned out by the CRTC.
**Port:** `View::border` is a colour; `present()` draws it as a frame of 8 source pixels
per side, scaled with the picture.
**Why:** the engine composes the active display only (spec §4.2, plan decision 8). The
border must still appear, but its exact thickness depends on CRTC timings the port does
not model.
**Measured difference:** 8 source pixels per side at every geometry. To be re-measured
against the original when PANIC is ported (Phase 5).

## A2 — An RLE row that decodes wider than its header's width is clipped

**Part:** START (`START/HZPIC.P`); the case exists in any file of that vintage.

**Original:** `readp()` (`ENDPIC/READP.C:10-41`, `START/READP.C:10-42`) decodes a whole
row into the caller's buffer with no bound on it, and `START/MAIN.C:105-113` hands it
`char rowbuf[640]` (`START/MAIN.C:13`). The 153 of `HZPIC.P`'s 400 rows whose opcode
streams yield more than 640 pixels — 756 for most of them, 761 at the widest, 121 pixels
over — therefore write past the end of that buffer. The overflow lands in whatever the
linker placed after `rowbuf`; it is not part of the picture, and the row the CRTC scans
out is the first 640 pixels either way. The header's width is not a bound on the decoded
stream — the row-length prefix is — and 247 of the 400 rows decode to exactly 640.

**Port:** `sr_assets::origin` takes the first `w` pixels of each row and drops the rest
(`decode_rows`), counting the dropped pixels so that the clip is the same prefix of the
row the original left visible.

**Why:** what the visible output shows is those first 640 pixels. Refusing the file is not
an option — the demo needs it, and the rows that overflow are rows it reads
(`START/MAIN.C:105-113` uses rows 0..255 as the background and 300..331 as the font
strip, and the wide rows include 0..10 and 300..331). Guessing at what the overflow
corrupted would be worse than either.

**Measured difference:** none in the visible 640×400 output; 153 of 400 rows drop
between 3 and 121 pixels each — 16,191 pixels of the decoded stream, 6.32 % of the
file's 256,000 — all of them beyond column 639. Measured by decoding
`assets/START/HZPIC.P` row by row.

## A3 — Three malformed-input paths in PAM's FLI decoder take defined behaviour where the tree's player has none

**Part:** PAM (`PAM/PRAX4.FLI`), Phase 4. PAM's `.ANI` (`PAM/OUT.ANI`) does not need
this entry: it has one malformed-input path, a record run past its 64,000-pixel
canvas, and `PAM/ANIM.C`'s encoder cannot emit one (`cnt+a<64000` bounds every run),
so the refusal there is an internal consistency check rather than a substitution.

**Original:** three constructs in `PAM/VFLI.C` have no defined behaviour, and no file
in either tree reaches any of them:

1. **A run that writes past the canvas.** The pixel offset is a 16-bit `unsigned int`
   (`VFLI.C:177`, `:208`, `:251`) and the only guard is `u<64000` *before* a packet
   (`:184`, `:219`, `:256`), never inside one. A run that has already started
   therefore writes into the 1,536 bytes of the VGA window past 64,000, and a wrap
   through 65,536 lands the remainder in the top-left corner of the screen (the pixel
   buffer is `0xa0000000`, `VFLI.C:6`).
2. **A chunk body longer than the frame that holds it.** `VFLI.C:87` calls `doblock`
   with the file positioned at the body and no bound at all, and only re-seeks to the
   chunk's end afterwards (`:88`) — so a body whose declared size outruns its frame
   reads the next frame's bytes as its own.
3. **A frame record with no chunks.** `blocksleft` reaches zero only inside the chunk
   branch (`VFLI.C:89-90`) — a frame header sets it (`:81`) and a chunk decrements
   it, but a header with an empty chunk list decrements nothing, so the next record's
   header resets it and the frame is never dumped at `:102-113`. The frame is dropped
   rather than shown, and the frame the player counts disagrees with the record chain.

**Port:** the first two are refused — `Unsupported` for a packet that would write
outside the canvas, `Truncated` for a chunk whose body outruns its frame, both with
the byte named — and the third completes with the canvas unchanged, which is what a
frame with no chunks means in the format. `sr_assets::anim`'s module docs carry the
same three, with the refusals listed beside the rest.

**Why:** each original behaviour is a write outside the picture's own bytes or a read
outside the frame's own bytes, and the crate's standing promise is that no input
panics and none is decoded past its input. What the first would corrupt is visible
(it paints real pixels of the top rows) and what the second would read is not the
chunk's data at all, so reproducing either would be inventing output; the third is
not a corruption at all but a dropped frame, and the port prefers the record chain it
walks.

**Measured difference:** **zero on the curated tree — no shipped file fires any of
the three.** Measured over `PRAX4.FLI`'s 42 frame records: every packet of every
`0x000C`/`0x000F` chunk stays inside its own 320-pixel line and inside the canvas
(0 packets cross either bound); every chunk's size fits its frame and every frame's
chunks end exactly on its frame's end, with the record walk landing on the file's
last byte (0 overshoots, 0 bytes left over); and every record carries 1 or 2 chunks
(frame 0 has 2, frames 1-41 have 1), so none is empty. The paths exist for files the
demo never shipped, and a later frame-diff comparison cannot encounter them.

Not an approximation, recorded here for scope: the `0x0004` and `0x0007` chunk
decodes are ported from `VFLI.C:161-172` and `:203-246` as written, including
`0x0007`'s treatment of a word's two bytes as two consecutive pixels (`:226-230`)
rather than the format's line pairing. No file in either tree carries either chunk
type (`PRAX4.FLI`'s histogram is `0x0B:1 0x0C:41 0x0F:1`), so those two paths are
cited from the authority and unmeasured, which `sr_assets::anim`'s module docs say.

## A4 — Two FLI decoding choices where the port follows the format rather than VFLI.C's own reading

**Part:** PAM (`PAM/PRAX4.FLI`), Phase 4. Both are choices about what a decode *is*, not about
malformed input; A3 covers the malformed-input paths.

**Original:** two places where `PAM/VFLI.C` reads a valid file in a narrower way than the format
allows, and where the tree's one file makes the two readings identical:

1. **Which palette entries a colour chunk lands in.** `VFLI.C:157` sets the DAC index from the
   chunk's `first` field (`outportb(0x3c8,a)`) but `:158` stores the same payload into its own
   `palette[]` array **from index 0**, never offset by `first` — and `0x0004` does the same pair
   at `:169`/`:170`. The array is what `:47-52` dumps as `pal.pal` for `PAM/ANIM.C` to read back
   (`:19-21`), so on a file whose `first` is not 0, the palette `VFLI.C` saves would place the
   chunk's colours at entries 0..count−1 while the screen shows them at first..first+count−1.
2. **The canvas geometry.** `VFLI.C` hardcodes the stride 320 (`:183`, `:218`, `:254`) and the
   canvas end 64,000 (`:184`, `:219`, `:256`), so a FLI of any other shape is decoded as if it
   were 320 wide: each decoded row advances 320 bytes, whatever the header says, and the picture
   shears.

**Port:** a colour chunk's range is applied **at `first`**, which is the DAC's reading and what
the screen shows, and the canvas is the header's width × height, as in every other decoder in
`sr_assets` (with the engine's area rule).

**Why:** with `first` read as the DAC reads it, the palette a frame is returned with is the
palette the part displayed, which is the only thing the port needs it for — `VFLI.C`'s own array
is a dump buffer for a second tool, not a picture. And the geometry is the file's own declaration;
`VFLI.C`'s 320 is a limitation of the tool that converted this one animation, not a property of
the format, so shearing a differently-shaped file onto a 320-wide canvas would be reproducing a
conversion bug rather than the demo.

**Measured difference:** **zero on the curated tree**, for both. `PRAX4.FLI`'s one colour chunk
carries `first` 0 and 256 entries, so the port's range and `VFLI.C`'s from-0 store are the same
entries — measured by reading the chunk's two big-endian words at its body (`01 00 00 00`, offset
150) and by the palette test's equality with `PAM/PAL.PAL` entry for entry. And the file's header
declares 320×200 at offsets 8 and 10, so the header geometry and the hardcoded one are the same
numbers; the tree holds exactly one FLI, so no other shape exists to differ on.

## A5 — BEGLOGO's lead-in: nine frames of the original's 42 are not modelled

**Part:** BEGLOGO (`BEG/BEG.C`), Phase 2 — the part that answers the phase's exit criterion, so this
entry is the one place that says what that criterion cannot see.

**Original:** the fade's first *displayed* step is reference index **446** (`docs/fidelity/
beglogo.md` §3: 470 is the ramp at `c = 24` and 560 at `c = 114`, each uniquely, and both give
`446`), and a run that starts at BEGLOGO begins drawing at **404** (`shell::FIRST_GRAPHICS_FRAME`,
`reference-indices.md` §1.12). Between them are **42 frames, of which the source accounts for 33**:
the 32 vertical blanks of `BEG/BEG.C:24` and the tick `:25-78` runs on. **The remaining nine are
unaccounted for, and the cause is unidentified.** What is ruled out is the two obvious candidates:

- **Not the EXE load.** `reference-indices.md` §3.3 files 410, 429, 430 and 431 as one `sha256` and
  byte-identical white — and `BEG/BEG.C:20-21`'s memsets are what makes that screen white, so the
  loader had finished and the part's own code was running by 410, six frames after 404.
- **Not `dis_partstart()`.** `BEG/BEG.C:17` calls it, and it is a **version check, not a
  synchronisation cue**: `DIS/DISC.ASM:51-64` calls `_dis_version`, prints
  `ERROR: DIS not loaded.` and exits 3 if the server is absent, and otherwise returns. It costs no
  frame, so there is nothing of it to fold into a constant.

What is left is 42 = 33 + 9 with the nine's cause unknown. Recording it as unknown is this entry's
point: an earlier draft of the part's own comments attributed them to "the frames the loader spends
on the EXE", which the 410 sample rules out, and then to `dis_partstart`, which costs nothing. A
measured-looking sentence that outruns its evidence is exactly what the phase's Review Focus 5 is
about, and it is not confined to constants.

**Port:** the port draws the white field for the whole 42 ticks and blits at tick 32 (reference index
436), and it has no model of anything in between. The port's split of the 42 is therefore 33 modelled
+ 9 not, not 42 modelled. The measurement that fixes 446 is unaffected: 446 is where the ramp is
*displayed*, and that is measured from the frames, not inferred from the lead-in's internal split.

**Why:** the port decodes the picture once, in `init` (`sr_assets` reads the whole container in one
call), and cannot re-pay a cost whose shape it does not know. Moving the blit so that the nine
frames "land where the original's must have" would be a fit of exactly the kind Task 4 Step 1
forbids for the fade's constant, and it would put the picture's indices into 436-444 against no
evidence at all.

**Measured difference:** **zero on every frame the reference holds.** 430 is the white field in both
(128,000 bytes of index 15), the four keyframes compare at exit 0, and 436-444 fall between two
keyframes the reference does not hold at all. **What a later phase should watch:** Phase 9 compares
every frame, and these nine are the frames a difference would show in — with the port's map the
*index plane* at 436-444 holds the picture where any reading in which the original's code starts
later holds the white field, while the two RGBs stay white (the DAC `:25-31` leaves every entry the
picture indexes white until `c` moves). A capture of 436-444, or a count of the original's
`dis_waitb()`s read from live memory, would settle it; neither was taken here, because no index in
`reference/` falls inside the window.

## A6 — TUNNELI: the shipped build's second palette ramp, and the residual left over it

**Part:** TUNNELI (`TUNNELI/TUN10.PAS`), Phase 2. Two entries in one, because they are the same
story: a palette ramp the tree's source and the shipped build disagree about, and the small residual
that is left once the shipped one is ported.

**The original — the ramp.** `TUN10.PAS:122` writes `(64-x) *3 div 4` into DAC entries 128..192
(a second grey ramp, three quarters of the first one's). The tree's own build of it, `TUN10.EXE`,
compiles exactly that: `3*(64-x)` then `idiv 4`, three times over for r, g and b, with the entry
pushed as `128+x` (image offsets `0x1d0`-`0x1e3`). **The build that shipped — the one every reference
frame came from — is a later revision in this one expression: its second ramp is
`(64 - x) * 3 div 5` at the same 65 entries.** Two measurements say so and neither is a fit to a
shape: the four reference frames exercise **42** of the ramp's entries, **38** of them with a
non-black reference pixel, over **5,275** informative pixels and **29** distinct greys (a dot's entry
is `128 + round(x/1.3)`), and every one matches `3*(64-k) div 5` exactly — including the **17** where
rounding rather than truncation would differ. And a search over every `(mul, div)` pair with
`mul ≤ 7`, `div ≤ 15` — **81 candidates evaluated, 70 distinct ramps** — each rendered over the four
reference frames, puts the port's colour field 1,108 pixels from them at 3/5 and at least 4,200 at
every other candidate: the source's own 3/4 is 6,376 and the runner-up (7/12) is 4,200. With 3/5,
**1,091 of the 1,108 are pixels the port inks that the reference leaves as background** — the
ink-presence residual below, which no ramp can fix — and the other **17 are dot overlap**: both sides
inked in different colours, **10 of them on first-ramp entries** (72, 75, 76, 83, 89) where no
second-ramp value can reach them.

**Port:** the *shipped* ramp, `3*(64-x) div 5`, with `TUN10.PAS:122` cited as the structure it
reproduces and the derivation test
(`the_second_ramp_is_derived_by_matching_the_reference_bit_for_bit`) as the constant's evidence. This
is D12's ruling applied — "the distributed release is the reference of record for every frame diff…
the source tree remains the reference for *how* every effect works" — and it is the first data point
on D12's own open item (whether the part EXEs in `MAIN/DATA/` are byte-identical to those packed
inside `SECOND.EXE`): for TUNNELI they are not the same build as the tree's `TUN10.PAS`/`TUN10.EXE`.
`docs/fidelity/tunneli.md` §5 carries the derivation and the disassembly.

**Why the ramp is an entry rather than silence:** the port deliberately does not transcribe a line of
the tree's source. That is a decision, and a decision needs its measurement where the next reader
looks — particularly because the tree's `.PAS` is the authority for every other part's porting.

**The residual — what is left, and what may not be claimed about it.** With the shipped ramp the
port differs from the four reference frames by **179 / 373 / 321 / 433 pixels of 64,000 (0.28 /
0.58 / 0.50 / 0.68 %)** at the alignment the reference supplies (offset 3,722; port ticks 78/478/
778/878), and the difference is *presence*, not colour — the largest channel delta at 3800 is 12,
against ~235 at the other three, where a port dot meets the reference's black. **The four keyframes
are not equally trustworthy and must not be averaged:**

| keyframe | census (sessions that can present its picture, ±1) | the re-capture (§8.4) | what the residual can be |
|---|---|---|---|
| 3800 | 7/9 — **settled** | byte-identical | the port's, or a sub-frame difference in it |
| 4200 | 3/6 — exactly half | byte-identical | probably the port's |
| 4500 | 2/5 — **no settled content** | byte-identical | either |
| 4600 | 1/6 — **no settled content** | 57 px away | either; the oracle cannot resolve better |

So at 3800 — and largely at 4200 — the reference is reproducible and *was* reproduced byte for byte;
a 179-pixel residual there is **not** "the oracle's spread", and the entry says so rather than
hiding behind it. The pre-fix partners' 8.07–10.18 % is not a spread either: §8.4 itself reframes
those numbers as "≈1 frame at 4200, 4500 and 4600 and ≈4 frames at 3800" — the history of a reference
that has been replaced. At 4500 and 4600 nothing can attribute the residual, because no session
reproduces those pictures at all.

**What is ruled out, measured rather than argued.** The colour field is right (§5) and the alignment
is whole: at 3800 the port's tick 78 leaves 179 pixels whose colour the reference does not show,
where its neighbours leave 754 and 953 — four times as many — so the comparison is at the right
tick and not a near one. The remaining differences are a few hundred of the 2,400–3,900
inked dots, in both directions (and 17 of the 1,108 at the aligned ticks are overlap rather than
presence) (at 3800 the port inks 179 that the reference does not and the
reference none that the port does not; at 4200, 347 and 26) — what two rings overlapping differently
looks like, which is a *sub-frame* difference in the chain or an artefact of the erases' order.
**What a later phase should watch:** Phase 9 compares every frame, and this residual is the number
that will follow TUNNELI there. A fix would have to come from a session that reproduces one of these
pictures *and* a longer run to see where the two trajectories diverge — the reference's own census
(`reference-indices.md` §1.8) is what would say whether the picture the port draws is a picture no
session drew.

## A7 — ENDLOGO: the music gate is its measured length, and ten lead-in frames are not modelled

**Part:** ENDLOGO (`END/END.C`), Phase 2. Two entries in one, following A5's shape: the one gate in
this phase that a faithful-looking port would have had to *wait on* something that does not exist,
and the small residual either side of it.

**The original — the gate.** `END.C:107-111` is

```c
for(a=0;a<5000 && !dis_exit();a++) { dis_waitb(); if(dis_musplus()>-16) break; }
```

— a loop that waits one vertical blank per iteration until the music player's position passes −16, or
until five thousand blanks have gone by. Nothing in it is a duration: how long the hold lasts is a
property of the music, of where `SECOND 5` restarted it (`MAIN/U2.ASM:905` puts MUSIC0 at order 25
before the part starts) and of the machine's speed. **The port decodes, mixes and plays no audio in
this phase**, so the cue cannot be evaluated at all — this is not a routine that could be stubbed and
called later, it is the loop's exit condition.

**Port:** the gate is replaced by the length the reference shows it having — **567 ticks**, one
`dis_waitb()` each — derived in `docs/fidelity/endlogo.md` §3 from the part's two measured fade
constants, which come from two different keyframes: the fade-in's last step is *displayed* at
reference index 579 (from 500 at the ramp's `c = 49`, uniquely) and the fadeout's first step at 1147
(from 1180 at `c = 30`, uniquely), so the settled picture is displayed for 1147 − 580 = 567 frames.
The record corroborates it from the other side without being asked: `reference-indices.md` §19.1's
sweep has the picture byte-identical at **twelve** consecutive samples, **580**, 620, 660, 700, 740,
780, 840, 900, 960, 1020, 1080 and 1120 — one `sha256` — and 580 is exactly the hold's first frame
under the two constants. The derivation test reproduces all nine of the states that table names
separately — 430, 460, 500, 540, 580, 900, 1120, 1160 and 1200, colour count and mean — from the two
constants (`the_sweeps_own_samples_are_the_ramps_frames`).

**Measured difference: zero against this reference, and unknown against any other.** All four
keyframes compare at exit 0 (`docs/fidelity/endlogo.md` §7), and 900 sits inside the hold, so the
substitution is invisible to the comparison — but only because the reference was captured on this
recipe's music, from this boot. A run whose music started elsewhere, or a machine whose frames are
not the emulator's, would reach `dis_musplus() > -16` at a different frame and the hold would be a
different length; the port's constant would be wrong there and nothing in the port could say so. That
is the sense in which this entry is an approximation rather than a translation, and it is why the
number is written down as a measurement rather than as a port constant.

**What Phase 3 replaces it with.** The real cue: the music player's position, sampled once per tick
by the port's own mixer, with `dis_musplus()`'s −16 threshold (`END.C:110`) as the condition. The
constant disappears entirely when that lands — it is not a threshold to tune, it is a stand-in for a
value that does not exist yet.

**The second half — the lead-in's ten unmodelled ticks.** The port's ticks follow the source's
vertical blanks one for one: tick 0 is `:61`, tick 1 is `:62-63`'s white and `:67-88`'s CRTC and AC,
tick 2 is `:89`, tick 3 is `:90-91`, ticks 4-35 are `:92`'s 32 blanks, and tick 36 is `:94-99`'s
`readp` and 400-row blit. The fade's first step is then displayed at tick 47 — measured, from 451 —
and the source's own code accounts for 37 of those ticks: 34 vertical blanks and the three ticks the
code between them runs on. **The ten ticks between the blit and the fade are frames the port does not
model, and their cause is unidentified**, exactly as A5's nine are between BEGLOGO's `:24` and its fade
— counted the same way, with the fade's own `:104` blank part of the iteration that displays step 0
rather than a tick of its own. The same two candidates are ruled out for both. It is not the EXE
load: the part's own white is up by **415**, fifteen frames before the first sweep sample
(`reference-indices.md` §19) — `inittwk`'s own palette clear is commented out (`END/ASM.ASM:14-20`),
so that white can only be `END.C:62-63`'s — and its picture is in video memory from tick 36, index
440, both before the window. And it is not `dis_partstart()`: `END.C:50` calls it, and it is a
**version check, not a synchronisation cue** (`DIS/DISC.ASM:51-64`: `_dis_version`, a message and
exit 3 if the server is absent, and otherwise a return). It costs no frame.

**Measured difference: zero on every frame the reference holds.** 430 (tick 26) is inside the 32
blanks and is index 0 in the reference and in the port; 500 and 1180 are the two fade frames and 900
is the hold, all three far outside the window. Ticks 441-450 are frames the reference does not hold at
all, and the port draws white through them, which is what the DAC makes of that window either way.
**What a later phase should watch:** the same thing A5 says — Phase 9 compares every frame, and these
ten are where a difference would appear, in the *index plane* rather than the RGB (the picture is
already in video memory from tick 36, and every entry it indexes is white until `c` moves). **A
capture of 441-450 would settle it, as it would A5's 436-444.** A count of the original's
`dis_waitb()`s read from live memory — A5's other remedy — would settle only the half of the question
it can see: it would say whether the part waits more often than the source shows, but not whether
frames are consumed by code that does not wait at all, and the only such code in this part is the 400
`readp` rows and 400 `lineblit`s of `END.C:95-99`, which sit exactly between `:99` and `:101`. Both
remedies are named here because they answer different halves.

**And the two residuals are the same size for the same work, which is worth recording now.** BEGLOGO's
is **nine** frames and this part's is **ten**, and both sit between a 400-row Origin decode-and-blit
and a palette fade — the same two routines, linked into both EXEs from the same two source files
(`BEG/ASM.ASM` and `END/ASM.ASM` are identical apart from one comment character). That is a *common
candidate*, not a cause: neither residual has been measured against a frame that could attribute it,
and this entry does not claim the blit costs ten frames — only that a later phase comparing, say, a
part whose lead-in has no decode-and-blit at all would test the hypothesis in one measurement, where
rediscovering the coincidence would cost it a session.


## A8 — CRED: the font strip the routine reads is not the strip the file is

**Part:** CRED (`CREDITS/MAIN.C` over `CREDITS/FONA.INC`), Phase 2. This is the
first entry of the part's own, and it is a *data* divergence rather than a
substitution: what the routine states and what the file holds do not agree, and
neither can be changed.

**The original.** `#define FONAY 32` (`CREDITS/MAIN.C:29`) with
`extern char far font[FONAY][1500]` (`:31`): the routine reads **32 rows** of a
raster that has **40** (60,000 `db` values, the 1,500×40 raster the inventory's
§E.8 and §E.20 measure). It also states a **32-column cell** —
`fonap[32]=1500-32; fonaw[32]=8;` (`:270-271`), the routine's blank — on a strip
1,500 columns wide: `1500/32 = 46.875`, so the cell does not divide the raster.
The strip's real structure is 47 ink runs, one per character of `fonaorder`
(`:32`), which the routine *scans* (`:250-269`) rather than indexes by cell.

**Port.** The part reads the raster itself, from the file's own values, and
translates `:250-269`'s scan. It does **not** load the strip through
`sr_assets::font::Font`: `Font::from_image` refuses this strip, because the
routine's 32-column cell is not a whole number of cells across the 1,500-column
raster, and passing a cell that *does* divide would state a geometry no line of
the source states — the guess the loader's own refusal exists to prevent
(`crates/sr-assets/src/font.rs:156-187`, and A7's ruling that the glyphs are the
scan's, never the cell's).

**Why.** Both mismatches are properties of the shipped art, not of the port: the
raster is 40 rows tall and the routine's window is 32, and the cell is an
arithmetic convenience for the blank. Reading only the first 32 rows is what the
original draws, and it is what the reference's own frames show.

**Measured difference: the pixels of the eight unread rows, and nothing else.**
Those rows hold **92** ink pixels, and **no column is inked in them that rows
0-31 leave blank** — so all 47 runs, every `fonap` and every `fonaw` are
identical whether the scan reads 32 rows or 40
(`parts::cred::tests::the_eight_rows_the_routine_never_reads_do_not_move_a_run`,
which measures both). The drawn difference is those 92 pixels, and the display
shows every other logical line, so **46** of them fall on rows a caption shows.
Where they do, a build that read 40 rows would differ from this one and this one
agrees with the reference: on the five keyframes the caption region (rows
100-199) is byte-identical to the reference's on 1440, 3240 and 7640, and on
5440 and 8740 it differs for a *different* reason entirely — the shipped build's
caption text, which the source tree does not hold. That is **A10**, not this
entry; it is only mentioned here because it is the other thing a caption region
can differ by (`bins/secondreality/tests/cred.rs`,
`the_three_frames_that_differ_differ_where_the_records_own_text_does`, which
prints both numbers per frame).

## A9 — ALKU: two rows of the font strip the routine reads are not in the file

**Part:** ALKU (`ALKU/MAIN.C` over `ALKU/FONA.UH`), Phase 5 — carried into the
register here because this is where a font strip is next touched: Phase 1's task
A7 found it (`docs/plans/01-assets.md`, A7's Interfaces block) and its ledger
says Phase 2 owns the entry.

**The original.** `ALKU/MAIN.C:169` with `:218` and `:225` scan **32** rows, and
`y2w=y+32` (`:274`, `:429`) draws 32 — of a strip that is **1,500×30**: the
header and the 45,000-byte payload of `ALKU/FONA.UH`. The array ALKU's routine
indexes is however longer than the strip: `ALKU/INCLUDE.ASM:17` includes the
`fona.inc` fragment and `:19` follows it with `dw 1500*5 dup(?)` — 7,500 words,
15,000 bytes, of **uninitialised** padding that the linker filled with whatever
was there. The routine's last two rows (rows 30 and 31, 2 × 1,500 = 3,000 bytes)
are part of that padding, not of the strip.

**Port.** The port reads the strip the file has — 30 rows — exactly as
`sr_assets::font::Font` requires: a cell taller than the strip is refused
(`crates/sr-assets/src/font.rs:170-180`), and A7 recorded that refusal as the
honest reading of a routine that scans rows the data does not contain.

**Why.** No file in either tree records those 3,000 bytes: they are whatever the
linker left, and a port that picked a value — black, white, a repeat of row 29 —
would be drawing pixels no evidence supports. The refusal is therefore
plan-mandated, and the pixels it costs are the finding.

**Measured difference: unmeasured, and where it can be measured is named here.** Half of the
answer is already in the tree: ALKU's own captures hold the held font cards
(`reference/parts/alku/frame_001719.rgb` and `frame_002719.rgb`, `reference-indices.md` §4.3), and
those cards are drawn by `prt`'s 32-row loop — `ALKU/MAIN.C:429`'s `font[y2-y][sx]` — so the cell's
rows 30-31 *are on screen* in them. Read as pixels, each card's three cells are inked only in their
first 14 of 16 displayed rows (the bands at 1719 are rows 120-133, 160-173 and 200-213, and the
cell's last two rows — the strip's 30 and 31 — are blank in those columns), i.e. the shipped build
drew no *visible* ink from the padding there. What no screen can show is the other half: padding
bits a black palette hides. Those are recorded in the linked image — `MAIN/DATA/ALKU.EXE`, PKLITE-
packed like every part EXE in that directory — so the honest state is **"not measured here", not
"unmeasurable"**, and Phase 5 (ALKU) is the task that owns settling it.

## A10 — CRED: three of the five keyframes are the shipped build's credits, not the tree's

**Part:** CRED (`CREDITS/MAIN.C` over `CREDITS/PICS/*.LBM`), Phase 2. A **data** divergence of the
same kind A6 records for TUNNELI, and the one that decides this part's acceptance comparison: the
port below the register *is* this tree's source, and the reference is the **released demo**, whose
CRED was not built from the source this tree holds.

**The original.** What the shipped `CRED.EXE` draws — `MAIN/DATA/CRED.EXE`, 148,711 bytes,
PKLITE-packed, its pack entry at `MAIN/PACKFINA.INC:70` — is recorded screen by screen, read off
its own sheet, in `reference-indices.md` §20.1.

**The tree.** `CREDITS/MAIN.C:41-156` calls `screenin(pic, text)` 21 times with the captions this
port draws, and `CREDITS/INCLUDE.ASM`/`INCLUD2.ASM` link the 21 `PIC*.LBM` files into it.

**Where they differ, measured.** The brief's acceptance command exits 1, and each difference is
localised (`bins/secondreality/tests/cred.rs` asserts both regions' byte counts per frame):

| index (screen) | picture band, rows 0-99 | caption, rows 100-199 | reading |
|---|---|---|---|
| 3240 (6) | 20,122 bytes differ | 0 | the caption is the tree's; the picture is not |
| 5440 (12) | 0 | 18,264 bytes differ | the picture is the tree's; the caption is not |
| 8740 (21) | 5,702 bytes differ | 5,520 bytes differ | both |

At 3240 the picture difference is not a palette the tree could upload: **8 of the band's 14 colours
— 3,524 of its 16,000 pixels — are in none of the screen's 256 palette entries**
(`the_references_sixth_screen_is_not_the_trees_picture_at_all`). And the captions are strings the
tree cannot draw: `grep -rn SKETCH upstream/SecondReality` returns **no hits in any letter case**,
while §20.1's table — written from the shipped build's own sheet — reads screens 11 and 12's
captions as "SKETCH - SKAVEN / GRAPHICS - PIXEL / …" and describes screen 6 as "a dotted vortex".
A port of this tree cannot produce either sentence's screen.

**The divergence is not confined to those three screens, and that is measured too.** The record's
own sweep of this part (`out/b5-s5-cred-map`, the source of §20.1's table) holds a *hold sample* for
every one of the 21 screens, and each sample against the port's hold puts **nine** of the 21 off the
tree's data — the three above and six more:

| screen | sample | picture band | caption | reading |
|---|---|---|---|---|
| 2 | 1740 | 25,286 | 3,297 | both |
| 6 | 3240 | 20,122 | 0 | the picture is not the tree's |
| 10 | 4640 | 8,080 | 0 | the picture is not the tree's |
| 11 | 5040 | 0 | 18,264 | the caption is not the tree's |
| 12 | 5440 | 0 | 18,264 | the caption is not the tree's |
| 17 | 7240 | 21,871 | 0 | the picture is not the tree's |
| 19 | 7940 | 34,668 | 8,832 | both |
| 20 | 8340 | 14,272 | 11,343 | both |
| 21 | 8740 | 5,702 | 5,520 | both |

Measured by `bins/secondreality/src/parts/cred.rs`'s
`the_records_other_holds_measure_which_else_the_release_changed` (an `#[ignore]`d test, because it
reads the scratch capture), over **all 21** of those screens' hold samples — the table above is one
sample per screen, and the samples on any one screen agree. The three keyframes' rows are unchanged
in it; screens 11 and 12's captions differ from the tree's by the same 18,264 bytes — §20.1 reads
both as the one four-line caption — and §20.1's sheet text names the release's own caption lines on
screens 2, 19 and 20 ("OBJECTS – WILDFIRE", "WORLD – TRUG", "MUSIC – SKAVEN") where
`CREDITS/MAIN.C` has none. The remaining twelve screens' holds are the tree's, byte for byte
(asserted at one sample each, 0 of 192,000 bytes).

**Why the port does not follow the reference here.** Copying the shipped captions would be inventing
text that is in no file in the tree, and nothing in the tree says which pictures they were paired
with. The port translates `CREDITS/MAIN.C` as it stands, and this entry is what the register is for
— the same call A6 made for TUNNELI's second ramp.

**Measured difference: 11.2563 % / 9.5125 % / 5.8453 % of one frame at 3240 / 5440 / 8740, and
0.0000 % at 1440 and 7640.** The two frames that agree are byte-identical, 0 of 192,000 bytes each,
which is what says the residue is the *data* and not the port: the same machinery, the same tick
map, the same palette repair and the same geometry reproduce them exactly.

**For whoever unpacks it.** `MAIN/DATA/CRED.EXE` is PKLITE-compressed, so its captions and its 21
pictures are recorded but unread. Unpacking it yields 21 `FC FC` blobs (16,784 bytes each: a 16-byte
header, a 768-byte palette at offset 16, 16,000 pixels at offset 784 — the layout `CREDITS/
INCLUDE.OBJ`'s own blobs have) and the caption strings, which is what would turn "the shipped
window differs" into "here is what it says". Phase 9 compares every frame; these three are where
that phase will first look.
