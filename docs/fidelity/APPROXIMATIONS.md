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

## A7 — ENDLOGO: ~~the music gate is its measured length~~ **retired, Task 8 — the cue is the source's own loop**, and ten lead-in frames are not modelled

**Part:** ENDLOGO (`END/END.C`), Phase 2. Two entries in one, following A5's shape: the one gate in
this phase that a faithful-looking port would have had to *wait on* something that does not exist,
and the small residual either side of it.

**Retired 2026-10-15 (Task 8), and what replaced it.** The gate is `END.C:107-111`'s own loop, run
against the run's player: one `dis_musplus() > -16` a tick, the source's five-thousand-iteration cap,
and no length anywhere — the hold is 567 ticks on the reference's boot because the music says so.
`docs/fidelity/endlogo.md` §5 is the replacement and the measurement it was checked against, and the
paragraphs below that describe the substitution are struck through with it. **Two things were kept
from the substitution, and both are named where they live:** A7's *measurement* (the 567, and the two
indices it is the difference of) survives as the **check** — the cue's own answer is compared with it
in `bins/secondreality/src/parts/endlogo.rs`'s tests — and the ten unmodelled lead-in frames, this
entry's second half, are untouched. The one **new** approximation Task 8 leaves behind is the gate's
alignment: one frame, because the port's own words and the reference's frames are one frame apart
(`docs/fidelity/endlogo.md` §5.2), and the frames are the record (D12).

**The original — the gate.** `END.C:107-111` is

```c
for(a=0;a<5000 && !dis_exit();a++) { dis_waitb(); if(dis_musplus()>-16) break; }
```

— a loop that waits one vertical blank per iteration until the music player's position passes −16, or
until five thousand blanks have gone by. Nothing in it is a duration: how long the hold lasts is a
property of the music, of where `SECOND 5` restarted it (`MAIN/U2.ASM:905` puts MUSIC0 at order 25
before the part starts) and of the machine's speed. ~~**The port decodes, mixes and plays no audio in
this phase**~~ — *until Phase 3, which is the task this entry's retirement records:* the cue is the
loop's exit condition, and it was the phase's own work to have something to evaluate it against.

~~**Port:** the gate was replaced by the length the reference shows it having — **567 ticks**, one
`dis_waitb()` each — derived in `docs/fidelity/endlogo.md` §3 from the part's two measured fade
constants, which come from two different keyframes: the fade-in's last step is *displayed* at
reference index 579 (from 500 at the ramp's `c = 49`, uniquely) and the fadeout's first step at 1147
(from 1180 at `c = 30`, uniquely), so the settled picture is displayed for 1147 − 580 = 567
frames.~~ The record corroborates that length from the other side without being asked:
`reference-indices.md` §19.1's sweep has the picture byte-identical at **twelve** consecutive
samples, **580**, 620, 660, 700, 740, 780, 840, 900, 960, 1020, 1080 and 1120 — one `sha256` — and
580 is exactly the hold's first frame. The derivation test reproduces all nine of the states that
table names separately — 430, 460, 500, 540, 580, 900, 1120, 1160 and 1200, colour count and mean —
from the two constants (`the_sweeps_own_samples_are_the_ramps_frames`), and that is what makes them
the check the port's cue is measured against rather than a memory.

~~**Measured difference: zero against this reference, and unknown against any other.** All four
keyframes compare at exit 0 (`docs/fidelity/endlogo.md` §7), and 900 sits inside the hold, so the
substitution is invisible to the comparison — but only because the reference was captured on this
recipe's music, from this boot. A run whose music started elsewhere, or a machine whose frames are
not the emulator's, would reach `dis_musplus() > -16` at a different frame and the hold would be a
different length; the port's constant would be wrong there and nothing in the port could say so.~~
**The warning above is what the cue answers**, and the answer was measured rather than hoped for: the
port's hold is the music's, so a run whose music started elsewhere gets that run's hold; and the
reference's own frames are what say the port's is right on this boot — four keyframes, 0.0000 %, exit
0 (`docs/fidelity/endlogo.md` §7).

**What Phase 3 replaced it with, and where the check landed.** The real cue, as above: the music
player's position, sampled once per tick by the port's own mixer, with `dis_musplus()`'s −16
threshold (`END.C:110`) as the condition — and the constant did disappear from the part, which now
carries no length at all. What the phase's proof asked was whether the cue's own answer would be the
567 the substitution had measured from the reference's frames: it is, and the one frame between the
two readings of it is named in `docs/fidelity/endlogo.md` §5.2. **A later phase reading this entry
should take the alignment with it**: a run whose music starts elsewhere (a `SECOND 3` or `SECOND U`
entry point) has no reference frame to align to, and `GATE_ALIGNMENT_TICKS` is a fact about the
reference's boot and `second5`'s session, not about ENDLOGO.

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
(`crates/sr-assets/src/font.rs:156-187`, and **Phase 1's task A7**'s ruling that
the glyphs are the scan's, never the cell's — a *task* A7, not this register's
A7, which is ENDLOGO's letter above).

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
(`the_references_sixth_screen_is_not_the_trees_picture_at_all`). And the captions are strings no
source file in the tree contains: `grep -rnia sketch upstream/SecondReality` returns exactly **one**
hit, and it is in the shipped *data* — `MAIN/DATA/CRED.EXE` (line 958 of grep's own counting of a
binary file, not a byte offset) — while a search restricted to the sources (`--include='*.C'
--include='*.ASM' --include='*.H' --include='*.INC' --include='*.PAS'`) returns none in any letter
case. §20.1's table — written from the shipped build's own sheet — reads screens 11 and 12's
captions as "SKETCH - SKAVEN / GRAPHICS - PIXEL / …" and describes screen 6 as "a dotted vortex".
A port of this tree cannot produce either sentence's screen.
*(Corrected at the Phase 2 close-out: this entry said "`grep -rn SKETCH upstream/SecondReality`
returns no hits in any letter case", which the `-a` run contradicts — the word is in the packed EXE,
which is where the release's own data lives.)*

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

**For whoever unpacks it.** `MAIN/DATA/CRED.EXE` is PKLITE-compressed, so its captions and its
pictures are recorded but unread. Unpacking it **would** yield the 21 pictures the credits draw as
`FC FC` blobs plus the caption strings, which is what would turn "the shipped window differs" into
"here is what it says". The count and the layout are **inferred, not read**: the 21 is
`CREDITS/INCLUDE.OBJ`'s 12 plus `INCLUD2.OBJ`'s 9 `FC FC` pictures, at a 16,784-byte stride
(`docs/fidelity/asset-inventory.md` §E.20, the row for those two objects) — the tree's own linked
objects, which is what the credits were built from — and the internal layout (a 16-byte header, a
768-byte palette at offset 16, pixels at offset 784) is the one those objects' blobs have, not one
read out of the packed EXE. Nothing here has been unpacked; what the markers inside `CRED.EXE` are
is unmeasured. Phase 9 compares every frame; these three are where that phase will first look.

## A11 — ENDSCRL: the shipped build's control lines, pitch, order and carriage returns are not the tree's

**Part:** ENDSCRL (`ENDSCRL/MAIN.C` over `ENDSCRL/ASMYT.ASM` and `ENDSCRL/FONA.UH`), Phase 2. A6's
and A10's class — this tree is not the released build — and the one that decides this part's
acceptance comparison. The port below the register *is* this tree's source read through this tree's
asset; the reference is the **released demo**, whose ENDSCRL was built from other constants, other
text handling and other art.

**The original.** The shipped part is `MAIN/DATA/ENDSCRL.EXE`, **9,965 bytes**, which is the file
`MAIN/PACKFINA.INC:50` packs (`0x26ED` unpacked, `0x140C` packed) and which PKLITE packed — the tree
holds two builds of this part and the reference ran the other one.

**Where they differ, measured.** Five differences, each measured on `reference/parts/endscrl`'s own
frames by `bins/secondreality/src/parts/endscrl.rs`'s reference tests, in the order they matter:

1. **The `[NN` control lines — the dominant term, and it is in the text.** This tree's `MAIN.C` has
   no notion of one: `:66-74` reads a line to its newline and `:77-88` draws what is in it, so a
   `[15` line is drawn as the two digits. The shipped build reads it as a **directive: advance the
   ring NN rows and draw nothing**, and every other line advances it 25. The shipped text holds
   **163** of them (`[8` ×6, `[10` ×14, `[12` ×135, `[15` ×8), and the rule *predicts* every band of
   frames 10000 and 14000 from this tree's own file, with no free parameter but the start below:
   frame 10000's lines 16, 17, 19, 20 and 22 sit at ring rows **0, 25, 65, 90, 130** (a `[15`
   between each pair: steps of **40 = 25 + 15**), and frame 14000's greetings show **six** `[12`
   lines, each a **37 = 25 + 12** step. The `[10` and `[8` lines are not visible in any of the four
   reference frames (their first is text line 430; the deepest line frame 14000 reaches is 108), so
   their 35- and 33-row steps are **unmeasured** here and the note says so.
2. **The line pitch is 25 rows, not `FONAY 30`.** Frame 12000's twelve whole text lines have gaps of
   25, 100, 50, 25, 25, 25, 25, 25, 25 rows: every gap a multiple of 25, the smallest 25. The port's
   own frame under the same measurement has gaps that are multiples of 30 and never 25.
3. **The shipped `fonaorder` names six characters the tree's does not** — `Z`, `Y`, `/`, `&`, and
   the high bytes `\x84` (`ä`) and `\x94`. The tree's order is 74 characters; the shipped build's is
   **80**, and the packed EXE carries the tree's own tail followed by the six
   (`xxd -s 9286 -l 40 upstream/SecondReality/MAIN/DATA/ENDSCRL.EXE`, whose appended run is the six
   bytes `5a 84 94 59 2f 26`). The text uses five of them:
   `/` 83 times, `&` 26, `Z` 12, `Y` once, `ä` twice. `fonaw` is 0 for all six here, so the port
   draws a two-column gap where the reference draws a glyph: on frame 14000 the lines naming one are
   **14–15 columns wider** than the port's rendering, and the lines naming none are within **1**.
4. **The raster is not `ENDSCRL/FONA.UH`'s.** At the tick whose header sits on the reference's own
   rows, the header band differs in **53 pixels of 11,520** (18 rows × 640; 22 of them ink, the rest
   values), while its glyph geometry — the same 13 ink-column runs — is the tree's to the pixel.
   The port's `FONA.UH` is the tree's: `ENDSCRL/FONA.LBM` decodes to it value for value. So this is
   a re-drawn raster and not a misread one.
5. **The renderer's text has no carriage returns.** `assets/MAIN/DATA/ENDSCROL.TXT` is CRLF; with
   `fonaw[13] = 0` and a two-column pen step, a line's centring `(639 - width) / 2` (`MAIN.C:73`) is
   exactly one column further left with the CR than without. The reference's header starts at column
   **219** and its second line at **41** — the CR-less width in both cases; the port's are 218 and
   40. The demo read the file through its own file service, which serves the packed `ENDSCROL.TXT`
   (`MAIN/PACKFINA.INC:51`); what that service did to the CRs is **not established** — the tree
   holds no source for it, so a text-mode read and a CR-stripped pack entry are both open.

**Why the port does not follow the reference.** These are the *part's build* and the *demo's file
service*; the port's authority for the former is this tree's source (`MAIN.C:6` says 30, and its
renderer has no control-line rule) and for the latter the tree's asset (which holds the CRs).
Bending either to make the comparison pass would be a fit rather than a translation — the shape A10
records for CRED, where the port likewise keeps the tree's captions and pictures and the register
carries the difference.

**And the pitch is measured but deliberately *not* adopted, which is D24's second branch.**
Difference 2 above is a single value — 25 rows where the source says 30 — and it is derived by
exact measurement, so D24's first branch ("a single expression or constant the release changed")
would reach for it. It is not taken, because that branch requires the release's difference to be
*one value the tree's algorithm can carry and nothing else in the part to differ*: this same part
differs in the control-line rule (a behaviour in the text), in its font order and raster (art) and
in its carriage returns (the demo's file service), and bending `FONAY` to 25 would leave every one
of those still off while making the port's own constant a fit to a screen it cannot reproduce. The
pitch is therefore registered here with its measurement, and the port keeps the source's 30. **The
measurement is not weaker than TUNNELI's** — both are exact and both are unique — the *decision*
differs, and this paragraph is the discriminator D24's step 3 now states.

**Measured difference.** `cargo xtask compare` exits **1** on all four keyframes: 9100
**2.8473 %**, 10000 **11.0676 %**, 12000 **9.9617 %**, 14000 **6.6828 %**, max channel delta 243,
every frame paired at offset 0 (`docs/fidelity/endscrl.md` §6). Of that, the **chain's own
offset** is 82 frames — the port's ENDSCRL begins at index 8835 and the reference's at 8917 ±1,
measured from the first line of the text, whose *rows* both builds draw identically — and the
remainder is this entry. **Two of the five drift**, which is why a single frame alignment cannot
reconcile them: the pitch moves the content 5 rows per line apart, and the control lines make the
reference skip blocks the port renders. `--align 82` shows both sides of that: 9100 improves
(2.8473 % → 2.1758 %) and 10000 worsens (11.0676 % → 12.5437 %). Phase 3's music work does not
touch any of it.

**Replaces:** nothing. **Leads for whoever takes it:** the two builds of this part are
`ENDSCRL/ENDSCRL.EXE` (14,113 bytes) and `MAIN/DATA/ENDSCRL.EXE` (9,965, PKLITE-packed) — the
shipped one's control-line rule, its `FONAY`, its 80-character order and its raster are all inside
that packed image. **The mechanism of the control-line rule is unknown, as the CR's is: what is
measured is the behaviour** — a `[NN` line advances the ring NN rows and draws nothing.

## A12 — The music clock: the port's tick clock is exact, the reference's frame counter is not

**Part:** the music, under every part that plays it (Task 3; the traces are
`reference/music/{second5,second5b,second3,hiddenu}`).

**Original:** the player's tick is `2.5/tempo` seconds of *the emulated machine's* time, and the
traces are sampled once per frame of the emulator's own frame counter
(`docs/fidelity/music-clock.md` §1). The ratio between the two is therefore a property of the
emulator's host timing, which D17 measured as a term the API and `--set` do not reach.

**Port:** `Player::poll` adds exactly `mix_rate/70.086` samples per logic tick and advances a whole
tick whenever `mix_rate × 2.5/tempo` samples have accumulated — integer arithmetic, no float, no
drift by construction. The port's ratio is 48 ticks a second at tempo 120; the reference's, measured
from the traces' own `zframe` column, is 47.93 (`second5`), 48.04 (`hiddenu`) and 52.01
(`second3`, tempo 130) — the same clock to 0.15 %, and not the same clock to the frame.

**Why this is registered rather than fixed:** it cannot be fixed. The two sessions of *one recipe*
— `second5` and `second5b`, the same command against the same emulator at the same pinned cycles —
disagree with **each other** about which frame eight of their 131 row changes land on, by one frame
each (`cargo test -p sr-audio --test trace -- --nocapture the_two_sessions`, whose output is pasted
into `music.md` §8). A clock that matched one session would be a fit to that session; the port's
exact clock sits *between* them, and that is the best a fixed clock can do.

**Measured difference.** `cargo test -p sr-audio --test trace -- --nocapture the_port_reproduces`
prints, for every trace in `reference/music/`, the share of the trace's frames whose published
position matches the port's, and the worst displacement:

```
4 filed trace(s)
  hiddenu (MUSIC0): 5687 of 6088 frames agree (93.413 %); worst displacement 3 frame(s)
  second3 (MUSIC1): 2144 of 2488 frames agree (86.174 %); worst displacement 2 frame(s)
  second5 (MUSIC0): 771 of 887 frames agree (86.922 %); worst displacement 2 frame(s)
  second5b (MUSIC0): 777 of 887 frames agree (87.599 %); worst displacement 2 frame(s)
```

The frames that disagree are one per row change — the ±1-frame placement of a row boundary — and
the *content* is not in doubt: the comparison test asserts, word by word, that the port publishes
exactly the trace's sequence of values for each of `order`, `pattern`, `row`, `speed`, `tempo`,
`loop` and `zplus`, and that every published value is one the trace publishes within 3 frames of
that frame.

**What is not an approximation here:** the ten-poll publication delay (`_np_*` is a ring buffer ten
entries deep — `STMIK.300` + `0x2ac1`), the tick and row arithmetic (`+0x3d99`, `+0x4397`), the
`254`/`255` reading and `zplus` (`+0x4452`, `+0x4469`), and the timeline effects are all the
original's own, cited in `music.md` §8 and asserted byte for byte by
`the_objects_own_bytes_say_what_the_player_says`. This entry covers the *frame the row changes on*
and nothing else.

**If it is ever closed:** it would take a trace of the reference's own frame clock beside a trace
of the guest's, which is a measurement `music probe`-style and not a code change.

## A13 — ~~The mixer's output curve is the object's linear case, not the reference's gated one~~ **Withdrawn — the curve is implemented**

**Withdrawn 2026-10-14, on review.** The entry above recorded the port converting the object's
accumulator linearly instead of through the table `+0x551c` builds. The review's ruling was that
the curve is behaviour on the reference's own path, with a parameter the port already loads
(`Module::master_volume`), and that this project's oldest rule — port the algorithm, not a
resemblance — decides it. It is now implemented: `sr_audio::{multiplier, Curve}`, `Curve::build`
is the builder's own loop **as the machine executes it** — `K = 0x8000/mul` ramp entries of
`(k × (65536/K)) >> 8` after `2·⌊(0x800 − K)/4⌋` zero entries and before `2·⌊rem/2⌋` bytes of
`0xFF` (A14's subject) — and `Mixer::frame` indexes it with the frame's value at `0x400 + value`
and widens the byte the card would play.
`docs/fidelity/music.md` §10.3 is the reading; §A13 leaves only one thing approximated in its
place:

**A13 (as it stands) — the curve's index is clamped at the table's ends.** The object's index is a
16-bit `bx` added to the base and read out of its own code segment (`+0x2cf4`'s `mov %cs:(%bx),%ah`,
`+0x2d08`'s `mov %cs:0x0(%bp),%al`), so a signal further from silence than the table is wide reads
whatever the linker put after it — the note tables at `+0x97c` are 0x10E bytes
past the table's end at `cs:0x86e`, and the scale table at `+0x9c4` further on. The port has no such neighbours to read
and clamps to the table's 2048 entries instead of inventing a value: `Curve::sample`. At the base
the port indexes with (`0x400 + value`) the index reaches past the table's end only for a value
at or above **+1024** — past the ±1000 these modules' signal reaches — and below the table's
start only for a value at or below **−1025**, which a sixteen-voice mono mix can reach.
(The marker's `0x501 + value`, which the port does not apply, would move both thresholds 257
lower; A14(a) is that question.) **Measured difference:** none that any capture can show (the
emulator runs silent, D17); the object's value there is not a defined output but its own
segment's bytes, which is the finding rather than a number.

## A14 — The curve the machine's memory holds is not the curve the builder's formula describes: the `rep stosw` clobbers its own count, and the index's base is open

**Part:** the music, under every part that plays it (the re-review's probe, `cargo xtask music
probe --dump`).

**Measured** at `--cycles 30000 --arg 5 --frames 400..1300` (`music probe --at 0x525C --dump
0x6C:2 --dump 0x6E:16 … --dump 0x86C:2`, the whole window, one line per change — the dump printed
**one** line per region, so nothing in these spans moved between frame 401 and frame 1299):

1. **The base word never moves.** `cs:0x6C` reads `6E 00` at every frame of the window, before
   the music starts and after. The port's single-address model is right.
2. **The table's bytes are not `+0x551c`'s output as read.** They are `00` for `cs:0x6E+0x000`
   (indices 0…767), then the **ramp itself** at `+0x300` (`00 00 01 01 … F0 F0 F1 F1 …`, i.e.
   entry `(i − 768)/2`), ending at `+0x500`, and **`FF` for every byte of `+0x500 … +0x7FF`**
   (`+0x56E`, `+0x5EE`, `+0x66E`, `+0x76E`, `+0x7EE` and `+0x86C` are all `FF`). The `mul` 64
   ramp's *shape* (one level per two entries) is there; its *placement* is 768 bytes early and
   the region above it is `FF`.
3. **The reason is in the builder's own bytes.** `+0x559d`'s `rep stosw` leaves `CX` = 0, so
   `+0x559f`'s `add %cx,%cx` doubles zero and `+0x55a1`'s `rep stosb` writes **nothing**. The two
   `shr $1,%cx` at `+0x5592`/`+0x559b` had left `CX` = `⌊(0x800 − K)/4⌋` **words**, so the zero
   region is `2·⌊(0x800 − K)/4⌋` bytes — half of `0x800 − K`, and one byte short of a plain
   halving when `K` is 1 or 2 mod 4 (`mul` 48's 682 is the case the port's own mono runs hit).
   `+0x55b5`'s `sub %di,%cx` then sees a short `di` and `+0x55bd`'s `rep stosw` fills the
   *remainder* with `0xFFFF`, `2·⌊rem/2⌋` bytes of it, which is exactly the 768 bytes the probe
   read. The port's `Curve::build` writes these counts, bug included.

**The audible buffer says which reading the machine actually plays through.** `[0xbe6]` =
`0x0C9B`, `[0xbe8]` = `0xC9B0` (measured, `base + 0x0BE4`), and the reviewer's dump of that
buffer at frame 1299 is a stereo waveform **centred on 0x80** with a swing of ±40–50 and
per-channel DCs ≈0x7C (L) and ≈0xA2 (R). The code's own index — the marker's `0x501 + value` —
contradicts it: through the table the machine holds, `0x501` and everything above it is the
`0xFF` tail the bug's short fill leaves, so silence would read the byte 0xFF. The *unmarked* base
through that same table does not: index `0x400 + 0` = 1024 is the ramp's `(1024 − 768)/2` =
**0x80**, the buffer's own centre, and a value of ±80 moves the byte by ±40 — the measured swing.

**What the port does about it.** `Curve::build` reproduces the builder **with its bug**: the zero
region is `2·⌊(0x800 − K)/4⌋` entries, the ramp follows it, and `+0x55bd`'s shortfall fills the
remainder of the span with `0xFF` for `2·⌊rem/2⌋` bytes. For `mul` 64 that is 768 zeros, the ramp
at 768…1279 and `0xFF` from 1280 — the boundaries the probe read — and `tests/mix.rs`'s
`curve_with` is the same arithmetic written out again, so a test that computed the *intended*
table (1536 zeros, the ramp at 1536…2047) or the linear map fails. The mixer indexes it at
**`0x400 + value`** (the audible base — see (a)).

**One clause is still open, with the measurement behind it.**

**(a) The marker.** The flush writes `0x0101` into every accumulator word it reads (`+0x2ce5`,
`+0x30d0`), so the *code's* index should be `0x501 + value` — 257 entries above the base the port
uses. The measurement says otherwise: with the buggy table the marker would put silence at index
1281, inside the `0xFF` tail, while the DMA buffer the probe read (`[0xbe6]:0xC9B0`) is a
waveform **centred on 0x80** — the ramp's `(1024 − 768)/2` at index 1024 = `0x400 + 0`. The port
follows the measurement (its `Mixer::base` uses `0x400`) and keeps the marker's bookkeeping in the
code without applying it. The code's own side of it is not in doubt — `+0x2cf1` writes `0x0101`
*after* reading and `+0x3482` adds into the plane, so the word a flush leaves behind is marked —
which is exactly why the audible index's behaviour is unexplained and not a code question. What
would settle it is a dump of the DMA buffer beside the accumulator plane it was flushed from:
the plane lives at `[0xbe4]:0x4000`, outside the located segment, so the probe needs a
linear-address mode that reads that segment, which this round does not add.

**(b) Withdrawn 2026-10-15 — there was never a second table, and round 3's refutation of the
formula was a label error in its own report.** The re-review re-measured the MUSIC1 window
(`--cycles 30000 --arg 3 --frames 600..900`) and found **all 2048 bytes of the span identical to
MUSIC0's run**, one change line, `cs:0x6C` = `6E 00` in both: the region is the builder's output,
in two modules' runs, at the base the pointer names. Round 3 quoted `+0x35A`, `+0x3CB` and
`+0x4B0` as if they were *table-relative* when they are absolute segment offsets, and read the
table's own bytes at those addresses as a different shape (`--dump 0x3C8:4` reads `2D 2D 2E 2E`,
where a shifted table would read zeros); its predicted boundaries were mis-derived too
(`0x8000/48` is 682, not 341, and both sessions run `_zoutputmode` 5, so both modules' 0xB0 byte
gives `mul` 64 and `mul` 48 is unreachable in this demo). The wording that asserted the withdrawn
claim was **replaced** rather than left standing — `music.md` §10.3 now says what both runs show
— and this paragraph is the record of why it went, so a reader who meets the old claim in an
earlier note or report finds the error named here. **So the port's table is not "the
MUSIC0-matched one": it is the release of record's own behaviour, which is the stronger claim and
the one D12 wants.**

**Measured difference:** the port's byte is the machine's table applied to the object's
accumulator — the same map **both modules' runs** are consistent with, byte for byte — and what
remains open is whether the audible index carries the marker (a). No capture can quantify the
audible result (the emulator runs silent, D17). `cargo xtask music
probe --dump` reproduces the **segment's** bytes above — the base word and the whole table span,
in either module's run — and does **not** reach the DMA buffer's contents (the waveform and its
0x7C/0xA2 per-channel DCs), which live at `[0xbe6]:0xC9B0` outside the located segment; a dump of
that buffer beside the accumulator plane it came from is what would close (a).

## A15 — `dis_sync()` above the table's last key is refused, not returned

**Part:** the engine's music seam (§11 of `docs/fidelity/music.md`), reachable only by a part that
calls `dis_sync()` — `PAM/OUTTAA.C:39` and `ALKU/MAIN.C:59`, `:66`, `:72`, `:79`, `:138`, all of
them block 1's and all of them Phase 4's.

**Original:** `sync_10` (`DIS/DISINT.ASM:449-463`) turns the published position into a part-start
code with the `ordersync1` table above it (`:428-446`): the loop is `mov cx,16` over fifteen
four-byte entries (`:455`) and the answer is read at `%bx - 2` (`:460`). Above the last key
(`0x4200`) the 16th iteration compares against the bytes that follow the table, which are
`sync_10`'s own first instruction, `mov es, cs:stmikseg` (`2e 8e 06 …`); where that 16th compare
*matches*, the answer is the table's last code, and where it does not, the loop falls through one
entry further and returns the word **after the one it compared** — the `mov`'s own modrm byte and
the low byte of its displacement, not the `2e 8e` it just compared with.

**Port:** `sr_engine::sync_from` reproduces the table, the `%bx - 2` arithmetic and the read
*before* the table exactly (the position `(0, 0)` included — see below), and **panics** for a
position above the last key, naming the position and the reason.

**Why the boundary is there, tier by tier.** The region above `0x4200` is not one thing:

| key | what the object's 16th iteration does | the port |
|---|---|---|
| `<= 0x8E2E` | the compare matches (`2e 8e` read as a word — the `mov`'s prefix and opcode, the same in every build) and the answer read at `%bx - 2` is the table's own last code, **4** | refused |
| `> 0x8E2E` | the compare fails and the loop returns the word after the one it compared — **not** the word the row above compares with, which is `2e 8e`, but the two bytes *after* those: the `mov`'s modrm byte `06` and the *low byte* of a **linker-set** displacement — measured **`0xCD06`** in `MAIN/U2.EXE`, **`0xED06`** in `MAIN/2NDFIX.EXE` and **`0x9D06`** in `U2.EXE` | refused |

Only the second tier *forces* a refusal: a port cannot return a number that differs between builds
of the original. The first tier is deterministic in every build the tree holds, and the port
refuses it too, on the brief's own boundary (`ord <= 0x42`): above the last key the answer stops
being the table's arithmetic and starts being a read of whatever the assembler left after it, and
this phase draws the line at the table rather than modelling that read. The tests say so —
`sync_refuses_the_band_whose_16th_compare_matches` refuses the answerable tier by name, alongside
the two harder ones — so the choice is visible rather than implied.

**The read *before* the table is answered, not refused.** At `(0, 0)` the `%bx - 2` read lands on
the two bytes before `ordersync1`, which are `pop es; ret` (`07 c3`) because `ordersync1` follows
`musframe_9`, whose INDEMO body ends `pop es` (`DIS/DISINT.ASM:420`) then `ret` (`:424`) — the
source's own instruction sequence, identical in all three shell builds the tree holds
(`the_table_is_the_shipped_shells_own_bytes` finds them). `sync_from(0, 0)` returns that word as
the C callers receive it, a 16-bit `int`: **-15609** (`0xC307` read signed; the parts' C is Turbo
C — `PAM/PAM.PRJ` names `Turbo C Project File` — whose `int` is 16 bits), and
`sync_answers_the_position_before_the_table_with_its_own_word` pins it. The position is **reached**
rather than hypothetical: in `second5` every sample from frame 148 to 426 reads `ord 0 row 0 pat 0`
(278 samples; the trace's coverage drops frame 409) and the first published row is 427. What a
caller gets there is a negative number, so every `dis_sync() < N` in the tree (`N` >= 1, all of
them) keeps waiting — exactly what the reference does, and the reason this is an answer rather than
a divergence. (ALKU's first `dis_sync()` is at this position by its own source, `ALKU/MAIN.C:59`,
and `part-sequence.md` §1 runs ALKU first in block 1 — but `second5` is a `SECOND 5` session and
does not run ALKU (`MAIN/U2.ASM:792` guards `exe1` on `whattorun & 1`), so the trace is the
position's evidence, not ALKU's.)

**Measured difference:** none for the answered domain — the word before the table is the same in
every build the tree holds and the port returns it — and the refusal covers only what the phase
cannot answer. For the refused region there is no measured difference either, and that is the
honest statement of the gap: no caller in the tree is *shown* to call `dis_sync()` above the last
key (both callers are block 1's, and how far MUSIC0's order list has got while they run is not
measured until they are ported), while the answered position is measured — reached for 278
consecutive samples of `second5`. An earlier draft of this entry claimed "no caller is shown to
reach the outside of the domain" while its own next paragraph said ALKU's first call meets the
refusal at `(0, 0)`; both cannot stand, and the trace settles which one is true: `(0, 0)` is
reached, above `0x4200` is not shown reachable.

**Revisit when a part is ported that syncs late (Phase 4):** if one of the two callers turns out to
call `dis_sync()` above the last key, the first tier's answer (the deterministic 4) is derivable
from the source's own instruction sequence and could be added here with its own note; the second
tier's cannot be answered by anything the tree holds.
