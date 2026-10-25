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
**Measured difference:** 8 source pixels per side at every geometry. The filed reference captures
are the displayed area, with no overscan border in them to measure, so the padding's exact
thickness stays unmeasured; the Phase 9 fidelity pass owns closing it against the original.

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
and no *hold length* anywhere — the hold is 567 ticks on the reference's boot because the music says
so.
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
carries no hold length at all. What the phase's proof asked was whether the cue's own answer would be
the
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

**Part:** ALKU (`ALKU/MAIN.C` over `ALKU/FONA.UH`), Phase 4 — carried into the
register here because this is where a font strip is next touched: Phase 1's task
A7 found it (`docs/plans/01-assets.md`, A7's Interfaces block) and its ledger
says the entry belongs to the phase that ports the part.

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
"unmeasurable"**. Phase 4 ported ALKU and settled the part's drawing: it draws the 30 rows the file
holds and not the two past it (`docs/fidelity/alku.md` §7, §11), and its two held font-card
keyframes (1719, 2719) are byte-identical with that strip (`docs/fidelity/alku.md` §8). The padding
bits a black palette hides remain unmeasured; the lead is the packed `MAIN/DATA/ALKU.EXE`.

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
`reference/music/{second5,second5b,second3,hiddenu}`, Phase 4's
`reference/music/{second1,second1b,second3b,hiddenu2}` and Phase 5's
`reference/music/{second2,second3a,second3d}`).

**Original:** the player's tick is `2.5/tempo` seconds of *the emulated machine's* time, and the
traces are sampled once per frame of the emulator's own frame counter
(`docs/fidelity/music-clock.md` §1). The ratio between the two is therefore a property of the
emulator's host timing, which D17 measured as a term the API and `--set` do not reach.

**Port:** `Player::poll` adds exactly `mix_rate/70.086` samples per logic tick and advances a whole
tick whenever `mix_rate × 2.5/tempo` samples have accumulated — integer arithmetic, no float, no
drift by construction. The port's ratio is 48 ticks a second at tempo 120; the reference's, measured
from the traces' own `zframe` column, is 47.93 (`second5`), 48.04 (`hiddenu`), 48.06
(`second1`) and 52.01 (`second3`, tempo 130) — the same clock to 0.15 %, and not the same clock
to the frame. A window trace's rate measured over a few hundred frames is finer than that
(47.83 in `second1b`'s 359 frames) and is not used as a long-run measurement.

**Why this is registered rather than fixed:** it cannot be fixed. The two sessions of *one recipe*
— `second5` and `second5b`, the same command against the same emulator at the same pinned cycles —
disagree with **each other** about which frame eight of their 131 row changes land on, by one frame
each (`cargo test -p sr-audio --test trace -- --nocapture the_two_sessions`, whose output is pasted
into `music.md` §8). A clock that matched one session would be a fit to that session; the port's
exact clock sits *between* them, and that is the best a fixed clock can do.

**Measured difference.** `cargo test -p sr-audio --test trace -- --nocapture the_port_reproduces`
prints, for every trace in `reference/music/`, the share of the trace's frames whose published
position matches the port's, and the worst displacement. Phase 4's four, Phase 5's three,
Phase 6's three and Phase 7's two are included; the ten window runs are phase-aligned to their
own session first (each prints its offset):

```
16 filed trace(s)
  hiddenu (MUSIC0): 5685 of 6084 frames agree (93.442 %); worst displacement 3 frame(s)
  hiddenu2: the session's phase is -16 frame(s) from the boot's
  hiddenu2 (MUSIC0): 1620 of 1692 frames agree (95.745 %); worst displacement 1 frame(s)
  second1 (MUSIC0): 3738 of 5538 frames agree (67.497 %); worst displacement 7 frame(s)
  second1b: the session's phase is +1 frame(s) from the boot's
  second1b (MUSIC0): 344 of 352 frames agree (97.727 %); worst displacement 1 frame(s)
  second1c: the session's phase is +1 frame(s) from the boot's
  second1c (MUSIC0): 1609 of 1692 frames agree (95.095 %); worst displacement 1 frame(s)
  second1d: the session's phase is -1 frame(s) from the boot's
  second1d (MUSIC1): 475 of 692 frames agree (68.642 %); worst displacement 2 frame(s)
  second2: the session's phase is +2 frame(s) from the boot's
  second2 (MUSIC1): 3207 of 4590 frames agree (69.869 %); worst displacement 4 frame(s)
  second2b (MUSIC1): 2000 of 3390 frames agree (58.997 %); worst displacement 5 frame(s)
  second3 (MUSIC1): 2141 of 2484 frames agree (86.192 %); worst displacement 2 frame(s)
  second3a: the session's phase is +2 frame(s) from the boot's
  second3a (MUSIC1): 6089 of 7291 frames agree (83.514 %); worst displacement 2 frame(s)
  second3b: the session's phase is +1 frame(s) from the boot's
  second3b (MUSIC1): 3179 of 4839 frames agree (65.695 %); worst displacement 6 frame(s)
  second3d: the session's phase is +4 frame(s) from the boot's
  second3d (MUSIC1): 2716 of 2792 frames agree (97.278 %); worst displacement 2 frame(s)
  second3e: the session's phase is +1 frame(s) from the boot's
  second3e (MUSIC0): 3857 of 3992 frames agree (96.618 %); worst displacement 1 frame(s)
  second4: the session's phase is +1 frame(s) from the boot's
  second4 (MUSIC0): 3822 of 3892 frames agree (98.201 %); worst displacement 1 frame(s)
  second5 (MUSIC0): 769 of 883 frames agree (87.089 %); worst displacement 2 frame(s)
  second5b (MUSIC0): 775 of 883 frames agree (87.769 %); worst displacement 2 frame(s)
```

On the Phase 3 windows the frames that disagree are one per row change — the ±1-frame placement
of a row boundary. The larger displacements are `second1`'s **7** over 5,538 compared frames
and `second3b`'s **6** over 4,839 — not the longest windows (the longest, `second3a`'s 7,291
compared frames, reach 2) but the ones whose reference rate is farthest from the exact one:
48.06 and 52.06 a second where the port runs exactly 48.00 and 52.00, so 0.1 % accumulates to
several frames over thousands. The *content* is not in doubt: the comparison test asserts, word by word, that the port
publishes exactly the
trace's sequence of values for each of `order`, `pattern`, `row`, `speed`, `tempo`, `loop` and
`zplus`, and that every published value is one the trace publishes within **7** frames of that
frame — the measured worst (`second1`), still under one 6-tick row (8.76 frames). The windows'
own phase offsets (−16 for `hiddenu2`, −1 for `second1d`, +1 for `second1b`, `second3b`,
`second1c`, `second4` and `second3e`, +2 for `second2` and `second3a`, +4 for `second3d`) are
the boot's session-to-session spread, measured in
`music-clock.md` §2, not clock drift.

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

## A15 — `dis_sync()` above `0x8E2E` is refused, not returned

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
*before* the table exactly (the position `(0, 0)` included — see below). Above the last key it
**returns the table's own last code, 4**, up to and including `0x8E2E`; above that it **panics**,
naming the position and the reason.

**Why the line is at `0x8E2E`, tier by tier.** The region above `0x4200` is not one thing:

| key | what the object's 16th iteration does | the port |
|---|---|---|
| `<= 0x8E2E` | the compare matches (`2e 8e` read as a word — the `mov`'s prefix and opcode, the same in every build) and the answer read at `%bx - 2` is the table's own last code, **4** | **returns 4** — the same number in every build the tree holds |
| `> 0x8E2E` | the compare fails and the loop returns the word after the one it compared — **not** the word the row above compares with, which is `2e 8e`, but the two bytes *after* those: the `mov`'s modrm byte `06` and the *low byte* of a **linker-set** displacement — measured **`0xCD06`** in `MAIN/U2.EXE`, **`0xED06`** in `MAIN/2NDFIX.EXE` and **`0x9D06`** in `U2.EXE` | refused |

Only the second tier *forces* a refusal: a port cannot return a number that differs between builds
of the original. The first tier is deterministic in every build the tree holds — both the compare
and the code it reads are the source's own bytes — so the port returns it. The tests pin both
edges: `sync_answers_the_band_above_the_tables_last_key_with_the_tables_last_code` checks the `4`
at `0x4201`, `0x4300` and `0x8E2E`, `sync_refuses_the_first_key_past_the_sixteenth_compare` the
refusal at `0x8E2F`, and `the_answerable_bands_upper_edge_is_the_word_past_the_table` ties the
boundary to the image's own `2e 8e`.

**The answered band is not a corner.** The `hiddenu` trace (`SECOND U`) holds
`(order << 8) | row > 0x4200` for **5,148 of its 6,351 frames (81.1 %)**, while `second5`,
`second5b` and `second3` hold 0 of their 1,150 / 1,152 / 2,755. The phase's earlier entry refused
that band on a drawn boundary; the final review changed it because the object's answer there is a
table value and a Phase-4 part waiting on `dis_sync()` while MUSIC1 or HIDDENU plays would have
met the refusal within seconds.

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

**Measured difference:** none for the answered domain — the word before the table and the last
code field are the same in every build the tree holds, and the port returns them. For the refused
region (`> 0x8E2E`) there is no measured difference either, and that is the honest statement of the
gap: **Phase 4 ported both callers and measured their windows**, and neither reaches the refused
region — ALKU's gates are crossed at boot 1537/2097/2509/3069/3630 over `second1`, whose highest
published order is **9**, and PAM's gate is crossed at `(13, 1)` on boot 7710 (`second1b`), so the
highest key either caller asks about is `0x0D01` — below even the table's last key `0x4200`, and far
below `0x8E2E`. The answered band, by contrast, is measured — occupied for 81.1 % of `hiddenu`'s
frames.

**What this entry still cannot close:** a part that syncs above `0x8E2E` would receive a refusal
(a panic naming the position) where the release returns a linker-set word. No caller in the tree is
shown to do so — the two Phase-4 callers stay in orders ≤ 13 — and the case remains hypothetical.

## A16 — The boot's twelve-frame music transient: a run installs its own module at its start, not the boot's

**Part:** the music of a run whose entry restart replaces the boot's module — today `--block 5`
and `--part endlogo`, which install MUSIC0 at order 25 where the boot had loaded MUSIC1; a future
block whose entry restart does the same inherits it.
**Original:** the shell's boot loads **MUSIC1 at order 0** at `MAIN/U2.ASM:772-775` (the initial
`restartmus`), and a block-5 run then replaces it with **MUSIC0 at order 25** at `:898-905` before
ENDLOGO starts. The modules' own speed/tempo words therefore change at frame **396** (MUSIC1's
3/130) and again at frame **408** (MUSIC0's patched 6/120), with no tick between them: the twelve
frames 396…407 are the boot's transient, and `_np_zframe` is 0 throughout.
**Port:** the run installs the entry point's module — MUSIC0 at order 25 for block 5 — at the run's
start and never loads-and-replaces MUSIC1. The run's first drawn frame is 404, so on frames
**404…407** the port's part-facing `MusicPosition::speed`/`tempo` read MUSIC0's **6/120** where
the trace reads MUSIC1's **3/130**.
**Why it is not modelled:** reproducing it means loading a module, polling nothing, and replacing
it four frames later — a second player's lifetime for four frames of a boot the port's runs start
inside (`MusicStart`'s whole design is "the run begins at its entry point"). The consequence is
confined to the two words: the published position is `0:0` on both sides (neither module has
ticked), so every cue a part could read is identical, and no part in either runnable block reads
`speed`/`tempo`.
**Measured difference:** the four frames 404–407 of a `--block 5` run differ in `speed` and
`tempo` only (6/120 against 3/130), as `docs/decisions.md` D27 records; from frame 408 on the two
agree. The difference is in the part-facing `MusicPosition` words, not in the run manifest: a
manifest carries the module, the start order and the start frame, and a per-frame `music_position`
that is `0:0` on all four frames either way. `music.md` §12 has the trace's own two readings
(`reference/music/second5` frame 396 = 3/130, frame 408 = 6/120).

## A17 — MNTSCRL: the first wait is the release's 71-tick duration, not the tree's music gate

**Part:** MNTSCRL (`FOREST/READ2.PAS`), Phase 4. The port follows the **release**, under D24 and
the controller's ruling on this task: the reference is the authority, and the real run must match
its four keyframes. This entry records the tree-versus-release difference the port does not
reproduce.

**Original:** `:153-159` busy-waits `dis_musplus() >= 0` after the fade-in and the font build.

**The release, measured.** Its last held frame is boot **554** and its pre-roll's first `scr` is
boot **555**; with tick 0 at boot 419 (measured from the fade-in's frame means,
`docs/fidelity/mntscrl.md` §6) that is ticks 65 through 135 — a first wait of **71 ticks**. It
cannot be a `musplus` threshold: the published position is order 42 row 16 **on both sides of the
boundary** (`reference/music/second3/trace.csv` frames 554 and 555; the port's own published
position at ticks 134 and 135 is `42:16`, `the_releases_first_wait_is_seventy_one_ticks`), and
`zplus` is 0, so `musplus` is **-32 at both**. Every `musplus >= X` or `> X` therefore either
ends the wait at tick 65 (X ≤ -32) or never at tick 135 (X > -32). The port runs the measured
duration as `RELEASE_WAIT_TICKS = 71`, named and cited. A single reference run cannot distinguish
a constant from a release-side condition its frames do not expose; the constant is the fitted
value the evidence supports, and the mechanism is unknown.

**The tree's counterfactual, and its divergence.** The development snapshot's `musplus >= 0` would
first be true at trace frame **2488** (order 47 row 0, `zplus` 2 → row 0), tick **2068** through
the port's one-frame lead. `Mntscrl::tree_wait` keeps that condition for the counterfactual test,
which measures what the tree would have drawn: the part would still be holding at every reference
keyframe after 500, and the held opening differs from the reference's 1100, 1700 and 2300 by
**118,843 / 118,924 / 119,056 bytes**
(`bins/secondreality/src/parts/mntscrl.rs`'s
`the_trees_own_wait_would_hold_through_the_scroll_keyframes`).

**Port (and measured difference, through the real binary).** The release's 71-tick wait: 500,
1100, 1700 and 2300 are all byte-identical to the reference at the mapped ticks — 500 in both
planes, the three scroll keyframes in RGB, with `K±1` distinguishable so the mapping is exact
(`bins/secondreality/tests/mntscrl.rs`'s `the_four_keyframes_match_the_reference`). The tree's
own wait, by contrast, would have left three of the four keyframes ~62 % of pixels away. The
part's other difference from the tree, the `scp` bound, is adopted too (§7 of the note: the
release's 640 over the tree's 639, measured at 401 bytes / 146 pixels on boot 2300).

**Replaces:** the earlier version of this entry, which kept the tree's wait and registered the
divergence. **Leads for whoever takes it:** the tree's shipped copy is
`upstream/SecondReality/MAIN/DATA/MNTSCRL.EXE` (77,047 bytes, PKLITE-packed), and
`MAIN/PACKFINA.INC:56` names `MNTSCRL.EXE` in the demo's pack list. What is measured here is the
wait's duration; the packed image's own condition, if it has one, would need an unpacker.

## A18 — RAYSCRL: the release's gates act seven ticks after the tree's condition

**Part:** RAYSCRL (`WATER/DEMO.PAS`), Phase 4. The port follows the **release**, under D24 and
the controller's ruling on this task: the reference is the authority, and the real run must match
its four keyframes. This entry records the tree-versus-release difference the port does not
reproduce.

**Original:** three waits, each acting on the first tick its condition holds —
`:98-104` `dis_musplus() >= 0`; `:135-143` `order != co` and `row >= 16`, with no `waitb`; and
`:158-166` `dis_musplus() == -11`, the cue.

**The release, measured.** Each of the three leaves **seven ticks after the condition first
holds on the port's clock** — the cue's trace labels are eight, because the trace's player
publishes that state one poll before the port's does; the port's own interval is seven — and the
three measurements are independent:

| gate | condition first holds | release leaves |
|---|---|---|
| init | boot **12962**, the published order 88 row 0 (`reference/music/second3b/trace.csv`) | the blit at boot **12969** (the 13000 keyframe is the fade's k=14 hold, 31 ticks on) |
| stage 2 | boot **13609**, the published order 89 row 16 | the main loop's first `scr` at boot **13616** (the font phase: the 14000 keyframe is main-loop draw index 384) |
| cue | boot **14942** (`musplus == -11`; the port's player publishes the same state one poll on, 14943) | the fade's first upload at boot **14950** (the survey's partner capture, `reference-indices.md` §15.4's second command: `out/b4b-ray-sess1/frame_014950.rgb`, whose recovered palette is the first upload's) |

All three are reproduced by one rule: the condition must be read true on seven consecutive
ticks, and the gate acts on the tick after (`GATE_CONFIRM_TICKS = 7`). The values are not row
thresholds or `musplus` offsets: the published row stays 16 for eight frames (13609–13616), and
`-11` for the trace's nine (14942–14950; the port's player, one poll on, 1981–1988), so the
leaves fall inside those windows, not at their starts.

**The tree's counterfactual, and its divergence.** `Rayscrl::confirm_ticks = 0` keeps the tree's
immediate condition. The main loop then starts at boot **13610**, **six draws early** (13616 −
13610), and by the keyframes the reference has 385 draws (indices 0–384) where the tree's policy
has 391 (0–390):
`the_trees_immediate_gate_draws_the_keyframes_early` measures **8,375** differing pixels at
14000 and **11,958** at 14500 (of 64,000) against the reference.

**Port (and measured difference, through the real binary).** With the seven-tick confirmation
the four keyframes are exact where the tree's own assets are: 13000 and 13500 are byte-identical
(13500 in both planes), and 14000/14500 differ by **3/42 pixels** — not from timing but from the
release's embedded `MIEKKA.SCI` differing from the tree's at the handful of strip bytes those
frames expose (`docs/fidelity/rayscrl.md` §6). The exact cue is kept: only `-11` triggers the
fade, and it runs 65 uploads to black (`the_cue_is_exactly_minus_eleven_and_fades_sixty_five_ticks`).

**Leads for whoever takes it:** the shipped part is `upstream/SecondReality/MAIN/DATA/RAYSCRL.EXE`
(90,351 bytes, PKLITE-packed, like MNTSCRL's). What is measured here is each wait's leave frame;
the packed image's own conditions would need an unpacker.

---

## A19 — MINVBALL: the release's coincident-dot overlay is not the tree's, and the RNG is Microsoft's

**Part:** MINVBALL (`DOTS/MAIN.C` + `DOTS/ASM.ASM`), Phase 4. Under D24: the source is the
evidence, the reference's frames are the authority, and the shipped release is the tiebreaker.

**The RNG, identified from the compile.** The brief names Turbo C; the source says Microsoft C
6.0. `DOTS/MAKEFILE:10-16` uses `cl /qc /AL` — Microsoft's large-model compiler — and the linked
`DOTS.EXE` carries the **“MS Run-Time Library - Copyright (c) 1990, Microsoft Corp”** string. Its
`rand()` is the 32-bit LCG with modulus 2³², multiplier **214,013**, increment **2,531,011**,
seed 1, returning `(seed >> 16) & 0x7FFF`; the first ten values are 41, 18,467, 6,334, 26,500,
19,169, 15,724, 11,478, 29,358, 26,962, 24,464
(`the_rng_is_microsoft_cs_lcg_and_its_first_values_are_the_sequences`). The port implements that
generator, not Borland's; the reference's dense 12,400 scatter frame is reproduced to **25
pixels** (`the_four_keyframes_match_the_reference_at_the_mapping`), which is the evidence the
stream is the release's.

**The residual the port does not reproduce.** The four reference keyframes differ from the port
by **67 / 63 / 37 / 25 pixels** of 64,000 (10,900 / 11,200 / 11,800 / 12,400; 0.1047 / 0.0984 /
0.0578 / 0.0391 %), asserted exactly by `the_four_keyframes_match_the_reference_at_the_mapping`.
The geometry is exact — at least **99 %** of each reference frame's shadow pixels are at the same
pixel in the port's frame (same test) — so the difference is confined to *coincident* dots: two
balls, or a ball and a shadow, land on one pixel and the draw order decides the colour. The
source fixes the draw order (the `dot` array), but which dot draws a given position is the
`dottaul` permutation, and the permutation's cursor `j` is **never initialized** (`MAIN.C:87`,
`:200`): the value is whatever the stack held. The port fits the one value the source leaves free
to the reference, **88** (the value the residues in this entry are measured with;
`the_setup_is_deterministic_and_its_permutation_is_fixed` pins the setup around it). The fit is
committed as the ignored measurement test `the_initial_j_is_fitted_to_the_reference`, which
replays all 512 values against the 10,900 frame:

```bash
cargo test --release -p secondreality --bin secondreality \
    parts::minvball::tests::the_initial_j_is_fitted -- --ignored --nocapture
# best five (pixels, j): [(67, 88), (341, 171), (353, 168), (354, 14), (356, 386)]
```

88 is the minimum; the residual is the release's own stack-`j` difference (the permutation itself
is the RNG's and is reproduced). The compiled
caller's `bx` — the other register the source does not fix — is measured as unobservable:
`the_entry_bx_is_unobservable_at_the_keyframe` renders 10,900 under `0, 0xFFFF, 0x8000, 0x1234,
0x4000, 0x5757` and asserts identical bytes.

**Why it cannot be followed further.** The shipped part is
`upstream/SecondReality/MAIN/DATA/MINVBALL.EXE`, a 7,395-byte PKLITE-packed self-extractor (the
same shape as A17/A18's leads); the tree's `DOTS/DOTS.EXE` is the 10,372-byte development build
of the ported source. The `dottaul` permutation is the RNG's and the port reproduces it — the
25-pixel scatter is the evidence — so the only unreadable input is the release's **initial stack
`j`**, inside the packed image. Everything the source states is implemented literally — the
`mov ax,ax` stale-`bx` rotation, the corrupted-offset store (`mov ds:[si+6],ax` stores the low
word of the dword the three ball restores left in `eax`, `ASM.ASM:186-195`, not the shadow row),
the gravity integration skipped when the shadow is off screen (`ja @@2` jumps past it), the
shadow colour 87 inside the background ramp, the exact cue window and the frame cap. The
corrupted-offset store was corrected in fix round 1 and the four keyframes re-measured; the
numbers in this entry are the post-fix measurement.

**Measured difference, through the real binary.** 67/63/37/25 pixels at the four keyframes, with
the shadow layer at 1856/1860, 2152/2159, 2175/2187 and 2168/2170 pixels shared (99.78 / 99.68 /
99.45 / 99.91 %), and every keyframe reproduced at exactly one frame counter
(403/703/1303/1903; the `K±1` neighbours are different pictures, asserted by the test, so the
mapping is not a loose fit).

## A20 — ALKU: the linked hzpic's three palette entries (adopted), and the text batches' phase (registered)

**Part:** ALKU (`ALKU/MAIN.C` over `ALKU/ASMYT.ASM`), Phase 4. Two entries in
one, because both are about one asset family and one frame: a D24 adoption of
three palette bytes and a residual the port's fixed tick cannot carry.

**The original — the palette.** `hzpic` is not `HOI.U`: `ALKU/INCLUDE.ASM:7-12`
assembles `HOI.IN0` + `HOI.IN1` as the symbol, and the linked payload differs
from `assets/ALKU/HOI.U` in exactly nine bytes — palette entries 1, 2 and 3
(`docs/fidelity/asset-inventory.md` §E.18):

| entry | the linked hzpic (shipped) | `HOI.U` |
|---|---|---|
| 1 | `(10, 10, 10)` | `(2, 2, 3)` |
| 2 | `(30, 30, 30)` | `(5, 5, 7)` |
| 3 | `(63, 63, 63)` | `(6, 6, 9)` |

The picture *bytes* are identical. **Measured:** the three entries are the
text palette's three levels — `fade2`'s entries 1, 2 and 3 become its entries
64, 128 and 192 (`MAIN.C:194-208`), which every glyph pixel uses
(`MAIN.C:173-181` gives it 0x40/0x80/0xC0). With `HOI.U`'s own entries the
reference's 1719 differs in **1,595** of 128,000 pixels and 2719 in **4,171**;
at 5859 in **2,706**. With the shipped entries 1719 and 2719 are
byte-identical, and 5859's residual is the phase below: **2,358** pixels in
the in-process script (`cargo test -p secondreality --bin secondreality
parts::alku` — the unit test
`the_scrolling_keyframes_port_phase_differs_only_in_the_text_xor_pixels`;
the phase scan's shift +0 row is the ignored `the_5859_phase_sensitivity`,
run with `--ignored`) and **2,353** through the real binary
(`cargo test -p secondreality --test alku -- --nocapture`).

**Port:** reads `HOI.U` (the curated picture) and overrides those three
entries with the shipped values, a named constant in `parts/alku.rs`
(`SHIPPED_PALETTE_ENTRIES`). **This is D24's adopt-the-release case**: the
difference is one data constant (nine bytes), the picture is byte-identical,
and every other difference in the part is accounted for — the cards are
*exactly* the reference's at the shipped values and visibly dark at the
curated ones. The derivation test is
`the_curated_palette_entries_leave_the_reference_different` (ignored):

```bash
cargo test -p secondreality --bin secondreality \
    parts::alku::tests::the_curated_palette_entries_leave_the_reference_different \
    -- --ignored --nocapture
```

which prints:

```text
boot 900: 0 of 128000 pixels differ with the curated entries
boot 1719: 1595 of 128000 pixels differ with the curated entries
boot 2719: 4171 of 128000 pixels differ with the curated entries
boot 5859: 2706 of 128000 pixels differ with the curated entries
```

**The original — the text batches' phase.** The fifth text batch is the empty
`case 4` (`MAIN.C:129-131`): it zeroes `tbuf`, calls `ffonapois` (which clears
the old text's top two bits) and then waits. Its `fmaketext` — which would
rebuild the XOR list from the zeroed buffer and keep the text erased — runs
only when `aa<320-12 = 308` (`:141`). The port's trace-faithful phase reaches
that batch at `aa=308` and skips it, so the stale XOR list re-adds the old
text on the following scrolls; the `textin` fade that is in progress at boot
5859 then makes it visible, where the reference's session reached the batch
at `aa=306` (and/or its fade was elsewhere) and its 5859 is bare. **Measured:**
the port's 5859 differs in **2,353 of 128,000 pixels (1.8383 %)** through the
real binary (`cargo test -p secondreality --test alku -- --nocapture`), all
inside rows 150-243 and columns 40-279 — the old text's glyph region — and
**350 of the 400 rows are byte-identical**. The phase sensitivity, with the
four gates fixed and the batches' transitions moved
(`the_5859_phase_sensitivity`, ignored):

```bash
cargo test -p secondreality --bin secondreality \
    parts::alku::tests::the_5859_phase_sensitivity -- --ignored --nocapture
```

whose 37 rows include (others elided):

```text
shift  -9:     0 pixels differ; a=311 p=1 f=0 stage=Main(Tail { scrl: 322 })
shift  +0:  2358 pixels differ; a=311 p=1 f=22 stage=Main(Outer)
shift +10:     0 pixels differ; a=311 p=1 f=20 stage=Main(Outer)
```

At a phase at which the frame is byte-identical — `-9` and `+10` in the
scan both give 0 pixels
(`the_scrolling_keyframes_picture_is_the_references_under_its_batch_phase`) —
the pixels, the palette, the pan and the text are the source's; the phase
is the port's fixed tick giving `fmaketext`/`faddtext` zero CPU cost where the
original's 1993 loops took frames, plus the session's music clock (the trace's
own `second1` fit has a 5.4-frame spread between its two readings). The port
cannot reproduce either, and this entry registers the difference. **Fix round
1** changed the copper fade's upload to the source's post-add step
(`copper3`'s `cop_pal` pointer, `COPPER.ASM:122-124`) and re-measured: the counts above are unchanged —
the fade's step changes the text pixels' colours, not which pixels differ.

**What is not established, and must not be implied.** The direction: the
reference session's phase is one boot, not a property of the release, and the
residual is confined to frames where a text fade and a batch overlap. The
lead to the released bytes is `MAIN/DATA/ALKU.EXE`, PKLITE-packed, its pack
entry `dw 0BE72h,06D53h,00002h,0132Eh,0797Fh,00000h ;ALKU.EXE`
(`MAIN/PACKFINA.INC:47`). The tree holds ALKU's sources and no built
`ALKU/ALKU.EXE`, so the comparison is against the shipped frames only.
`docs/fidelity/alku.md` §8 carries the frame-by-frame reading.

## A21 — PAM: the reference's 7900 index plane is the next part's white field, not PAM's screen (registered, not adopted)

**Part:** PAM (`PAM/OUTTAA.C`), Phase 4. **Original:** the reference's 7900
frame carries both `.rgb` and `.idx`. The RGB is the flat `(251,251,251)`
wash; the index plane is 64,000 bytes of **15**, and the frame's manifest
palette entry 15 is `[62,62,62]`, which widens to 251.

**Why that plane cannot be PAM's frame.** PAM's 7900 screen is its last drawn
animation frame (0-based frame 39, 64 distinct indices) under the near-white
63rd palette copy. The reference's plane is index 15 everywhere, which is
**BEGLOGO's `WHITE_FIELD`** (`BEG/BEG.C:18-21` fills A-space with 15 before its
own DAC writes; `bins/secondreality/src/parts/beglogo.rs`). The capture's
indexed read runs after the RGB frame and costs about twenty rendered frames
(D15); the reference's 7900 is inside the window before BEGLOGO's 320×400
mode change in (7920, 7930] (`reference-indices.md` §6.5), so the read caught
BEGLOGO's field while PAM's 63rd copy was still in the DAC. The coherence
check cannot tell: every index of the white palette widens to one colour, the
blind spot `reference-indices.md` §1.6 records.

**Port:** the port draws the source's screen — animation frame 39 under
palette copy 63 — whose RGB is byte-identical.

**Measured difference.** RGB: **0 of 64,000 pixels** at 7710, 7760, 7830 and
7900 (`cargo test -p secondreality --test pam -- --nocapture`). Index plane at
7900: **60,375 of 64,000** pixels differ, max delta **194** (the frame's
highest index 209 less 15); `cargo xtask compare --reference
reference/parts/pam --rendered out/pam-port` reports it as 94.3359 % in its
mixed mode. The port's plane equals `OUT.ANI`'s frame 39 — 64 distinct
indices, highest 209, both asserted by
`the_7900_index_plane_is_the_next_parts_white_field_not_pams_frame`.

**Decision (D24's procedure, step 2).** The tree cannot produce the reference's
plane at this index because the plane is not this part's screen; the port keeps
its own drawing and the comparison at 7900 is RGB. Registered, not adopted.
The lead is `BEG/BEG.C:18-21` and the reference's own
`reference/parts/pam/manifest.json` palette override; `docs/fidelity/pam.md`
§6 carries the frame-by-frame reading.

## A22 — PLZPART: the plasma exits at the release's fourth palette table, not the tree's fifth (adopted)

**Part:** PLZPART (`PLZPART/PLZ.C`), Phase 5. **Original:** `plz()` advances
`curpal` through `pals[6][768]` and exits when `curpal == 5 && cop_drop > 64`
(`PLZ.C:94`; `PLZ.OBJ` disassembles to `cmpw $5`).

**Why the tree's 5 cannot be the release's.** The tree's exit would fire four
switches in, at boot **8769** — 429 frames past the reference's last plasma
screen. The reference's plasma-to-cube hand-off is at boot ~8315: the cube's
music gate (`musplus() >= 13`) passes at boot 8364 and the 320×400 screen is
gone from (8310, 8340] (`reference-indices.md` §13.7). A vendored tree that
runs four switches is a development snapshot, not the shipped part.

**Port:** `EXIT_PALETTE_RELEASE = 4`; the third switch starts the
`cop_drop` walk and the exit follows 65 ticks later.
`the_release_exit_index_is_the_measured_one` asserts the switch ticks
975/2010/2527 and the exit at 2592 = boot 8315 inside the capture's window.
The tree's 5 stays in the file as `EXIT_PALETTE_TREE` for the test.

**Measured difference.** The port's plasma screen is gone by the reference's
cube keyframes, as the reference's is; the plasma keyframe residuals (11.209 %,
5.848 %, 4.052 % at boots 6000/7000/7600) are §4 of `docs/fidelity/plzpart.md`,
not this divergence.

**Decision (D24's procedure).** The tree's value contradicts the reference's
own screen boundary; the release's value is adopted. `docs/fidelity/plzpart.md`
§10 records the reading.

## A23 — PLZPART: the release's cube projection is centred and 3/2 taller, and it draws the full screen, not the tree's 134 rows (adopted); its close-up camera data is absent from the tree (registered)

**Part:** PLZPART (`PLZPART/VECT.C`, `PLZA.ASM`, `SPLINE.ASM`), Phase 5.
**Original:** a 320×200 textured cube on a six-page flip whose music gate the
reference pins to boot 8364.

**The tree's build is unambiguous.** `VECT.OBJ` compiles `imul eax,eax,0x8e`
(142) and `add ax,0x42` (66) for `yyy`, `sar eax,0x16` for the matrix's
`>>15+7`, and `PLZA.ASM:77`'s `cmp yy,134` clips the rasteriser at 134 rows;
`SPLINE.OBJ` and `INCLUDE.OBJ` carry the RATA/SPLINE/sinit data the port
parses, word for word. A faithful port of the tree draws a 134-row cube with
`yyy = yy*142/zz + 66`.

**The release does not.** The committed six-frame band capture
(`reference/parts/plzpart/band/`, frames 8400–8800, taken by the documented `SECOND 3`
recipe at cycles 30000) and the filed 8500/9500 cube frames
(`reference/parts/plzpart/cube/`) show:

- **The extent.** The discriminating evidence is the lower band's face
  continuation: at 8500 the 134-row render fills the band below row 133 with
  the wrapped top face carried through the page slot, and its content reaches
  row 199 anyway; the band-silhouette fits below are the second line. Cube
  content reaches row 181 at boot 8500 and 199 at 9500, so the release draws
  the full 200-row screen. The port adopts `CUBE_DRAWN_ROWS = 200` and a clear
  mirroring the draw (`CLEAR_LINES = 201`, the tree's 135/134 scaled to the
  clip).
- **The page-slot wrap (registered, not adopted).** The slots are 0x5500 bytes
  (136 rows), so a full-height draw crosses into the next slot. The committed
  full-extent render shows the wrap — the wrapped top face and background
  spill at rows 0–32 and 182–199 and inside the band — where the reference's
  8500 frame is clean (its content spans rows 33–181). **Which side's page
  timing, clear or draw path makes the difference is not measured**: no
  capture of the release's flip frames was taken, so this is recorded as an
  unresolved port-side artifact, not as a release divergence. The spill is
  measured as the 6,333 port-only pixels (9.90 points, 2,805 of the 3,137
  delta-247 pixels) in the 8500 residual, and is registered as such. The
  earlier claim that the next page redraws the slot before display is
  contradicted by the committed render and is not made.
- **The phase.** The entry pre-roll is **8 polls**: the `musplus() >= 13`
  gate passes at manifest 2641 = boot 8364 (the trace's first order-68 row
  13), the first draw is boot 8365, and at the 8500 keyframe the part is at
  **frames 136, page 4, start 0x0028**;
  `the_projection_fit_is_the_committed_release_bands` asserts that state
  (frames 136, drawing page 4, display start 0x0028, music 68:46). Nine polls
  were one frame early (the previous commit records 32.106 % at 8500 there);
  a scratch diagnostic render of the corrected phase at the tree's 134-row
  extent measured 25.467 % (git-ignored, not reproducible from a clone), and
  the release extent's 26.161 % is test-pinned.
- **The projection.** The release's `yyy` is `yy*213/zz + 100`: scale 213 =
  142 × 3/2, origin 100 = the screen's vertical centre (the tree's 66 leaves
  the cube 34 px high). The x pair is unchanged (256; a scratch x-scale
  sweep's minimum — a diagnostic, not reproducible from a clone).

**The comparison rule** (the numbers below): a pixel is set iff any RGB
channel is nonzero; the shift is the integer (dx, dy) in [−20, 20]
minimising the differing-mask fraction, comparing the port at (x, y) with
the reference at (x + dx, y + dy) and treating reference pixels outside the
frame as unset; a frame's percentage is XOR over the band's 42,880 pixels
and a mean is the six-band total. The projection fit is measured on the
**cube band, rows 0–133, at the tree's 134-row extent** — the release
extent's full-height draw crosses the 0x5500-byte page slots and leaves a
background spill the reference's frame does not show, so the full frame
measures that spill, not the projection. Its reference is a committed
capture, taken by the sanctioned path:

```bash
cargo xtask reference capture --arg 3 --boot autoexec \
    --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames 8400,8450,8500,8600,8700,8800 --out reference/parts/plzpart/band
```

`the_projection_fit_is_the_committed_release_bands` renders the port through
the real engine and player with a test-only probe selecting the tree extent
and each pair, and pins every frame's XOR count and the totals: the six
silhouette frames 8400–8800 give 0.30/0.50/0.57/0.35/0.49/0.58 % (mean
**0.46 %**, 1,196 of 257,280) at 213/100 and 5.33/13.64/7.82/13.41/11.37/
8.36 % (mean **9.99 %**, 25,696 of 257,280) at the tree's 142/66. The
y-scale fit is flat over 206–216 at origin 100 (an earlier scratch sweep);
213 is the 3/2 value. `the_cube_projection_is_the_releases_centred_scale`
pins 213/100, `the_cube_draws_the_releases_full_screen` pins 200/201, and
`the_music_pre_roll_lands_the_cube_gate_on_the_reference_tick` pins the gate
at 2641 — each fails if reverted (mutation checks in the task report).

**The 8500 residual's attribution.** 16,743 pixels of 64,000 differ: 6,333
port-only (9.90 points; 2,805 of the frame's 3,137 delta-247 pixels are the
spill), 105 reference-only (0.16 points), and 10,305 both-set and differing
(16.10 points). 8,677 of the both-set differences are ≤ 20 (deltas 4/8/12 the
most common, 5,084/1,802/713); 332 more are at 247. The port-only part is the
page-slot spill registered above; the both-set part is shade/pose.
`the_cube_keyframes_are_measured_and_recorded` pins the counts.

**The residual that remains is registered, not adopted.** The close-up frames
9100–9600 need frame-dependent camera data the tree does not carry: the RATA
control points they interpolate (15–26; boot 9100 is animation frame 736 and
segment 15, boot 9600 frame 1236 and segment 23, and the spline at frame 1136
is segment 21) are all `dx = dy = 0, dz = 300`, constant across those
segments — the tree carries no frame-dependent translation. This is **absent
release camera data**, not a translation the port can apply. A global
alignment (a wider search than the projection fit's [−40, 40]) confirms
translation cannot explain the frame: the recorded best shift moves the
reference by (−32, +83) (the port by (+32, −83)); over the 288×117 overlap
the silhouette masks XOR in 10.33 % of the region (10.47 % with the reviewer's
border convention; IoU 0.897), and 99.95 % of the 30,214 common pixels still
differ (max delta 255, mode 194, 9,226 of the common pixels). The 9500
keyframe therefore stands at **99.483 %**. The keyframe values are pinned to
0.001 points (percentages) and exactly (max deltas) by
`the_cube_keyframes_are_measured_and_recorded`: 8500 **26.161 %** (max delta
247), 9500 **99.483 %** (max 255), with the 8500 decomposition and the 9500
aligned counts pinned beside them. The close-up's control points are derived
from the committed `RATA.INC` by
`the_close_up_segments_carry_no_frame_dependent_translation`. The plasma
keyframes are unaffected (11.209/5.848/4.052 %).

**Decision (D24's procedure).** The extent, the projection and the entry
phase are single-value release divergences and are adopted; the close-up
camera path is a multi-value data difference the tree cannot supply and is
registered. The page-slot spill is **not** a measured release divergence: the
port's own 200-row draw wraps through the 0x5500-byte page slot, the
release's 8500 frame shows no such content, and which of the release's page
timing, clear or draw path differs was not captured — it is an unresolved
port-side artifact, registered as such. The far-keyframe residual is
registered at **26.161 %**: 9.90 points port-only page spill and 16.10 points
both-set shade/pose, with 0.16 points reference-only. "Verified" for PLZPART
means a faithful port with A24's plasma tear, the 9.90-point port-side spill
(not a release divergence) and the close-up's absent camera data registered —
not a matching cube. The projection fit's reference is the committed
`reference/parts/plzpart/band/`; the remaining diagnostic captures are under
`out/`.

## A24 — PLZPART: the plasma's per-pass parameter latch is a CPU-timing tear the port does not reproduce (registered)

**Part:** PLZPART (`PLZPART/PLZ.C` over `PLZPART/COPPER.ASM`), Phase 5.
**Original:** `plz()`'s frame loop has **no frame wait** (`PLZ.C:74-122`); it
runs unthrottled, and the copper's once-per-vblank `moveplz`/`pompota` can
land between — or inside — the loop's four plane passes. `setplzparas` latches
the `k`/`l` parameters into the patched code at the start of each pass, so the
screen is torn at whatever pass boundary the retrace lands on. Which boundary
that is depends on the original's per-frame execution time (the fade/drop work
changes it), i.e. on the machine, not on the algorithm.

**Port:** the engine is a fixed 70.086 Hz tick and draws the four passes with
one parameter generation, the loop's phase-locked steady state (the copper's
walk runs at the tick's end). `PLZ.C`'s own arithmetic, timetable, palette
accumulators and the split/pan walk are transliterated exactly; only the
intra-frame interrupt landing is not modelled.

**Measured difference.** `cargo test -p secondreality --test plzpart`:
11.209 %, 5.848 % and 4.052 % at boots 6000/7000/7600 (max channel delta
16/21/17). The tear was measured directly by forcing the walk to land before
each pass (a temporary diagnostic): the 6000 frame's residual by split
position is 4.15/5.23/4.38/8.42/11.21 % (before passes 0/1/2/3, and at the
frame boundary), i.e. the 6000 reference frame's tear sits before pass 0 or 2;
7000 and 7600 sit at the boundary (5.85/4.05 %). No fixed split matches all
three: split 3 gives 8.42/8.59/8.28 and the boundary 11.21/5.85/4.05. The
reference's own sessions agree within 0.09–3.08 % because a session's timing
repeats; the port cannot choose the split without the original's cycle costs.

**Decision.** The port keeps the steady state (the boundary); the residual is
registered with the split measurements. `docs/fidelity/plzpart.md` §4 carries
the mechanism and the table. The diagnostic renders are under `out/`.

## A25 — LNS&ZOOM: the release's face picture differs from the tree's `LENS.EXB` (registered), and the part 3 hand-over costs one frame more than the source's waits (adopted)

**Part:** LNS&ZOOM (`LENS/MAIN.C` over `LENS/ASM.ASM`), Phase 5.

**The picture (registered).** The port reads the tree's `LENS.EXB` in its
original format — palette at +16, 64,000 pixels at +784 — and the wipe
converges on it byte for byte. The reference's held face (2700's `.idx`)
differs from that picture in **2,058 pixels**, all in the forehead (rows
33..91, columns 129..211), every one a small palette-index change. The
release embedded a later revision of the picture that is not in the tree (the
`.LBM` source art is not the linked asset). D24's procedure: the difference is
measured, and its consequences are pinned. The keyframes' residuals are
exactly this picture seen through each act — 2,058 pixels at 2700 (3.2156 %,
max delta 53), 1,935 behind the lens's bubble at 3400 (3.0234 %, max 48),
0 at 4200 (the sampled region misses the forehead), 1,396 through the
rotazoomer at 4900 (2.18125 %, max 41) and 0 on the white exit at 5650 — and
patching the tree's picture with the reference's 2700 index plane makes 3400,
4200 and 4900 **byte-identical**
(`parts::lnszoom::tests::the_forehead_asset_difference_is_the_whole_residual`
proves it). The numbers are pinned by `tests/lnszoom.rs`. "Verified" for
LNS&ZOOM means a faithful port with this asset divergence registered, not a
matching forehead.

**The hand-over (adopted).** The source's explicit waits between the last
lens draw (`MAIN.C:174`) and the first rotazoomer frame are seven — the lens
loop's exit wait, the five-frame hold (`:179-182`) and the `waitb()` before
`inittwk` (`:199`). The reference's pictures require eight: its 4200 is the
path's frame 552 at boot 4200 and its 4900 frame 1252 at 4900, both exactly
`T3 + frame` with `T3 = 3648`, while the lens's last draw is at 3640
(`T2 = 2926`, pinned by the trace's `48:44` at 2925 and the lens's frame 474
at 3400). The rotpic90 rotation (65,536 byte moves) and `inittwk`'s 64 KB
`rep stosw` cost the release a vblank, so the port charges the setup two
frames (`ROT_SETUP_FRAMES`). The adoption is measured, not read off the
source, and the note's tick table states it.

**5650's index plane (not adopted).** The reference's 5650 `.idx` is the
tool's planar *memory* read — 100 character rows and the zeros past them —
not the displayed (row-doubled) picture, so it is a different geometry from
the port's plane and is not compared. Its palette is 63 in every entry, so it
could not validate a stride in any case. `docs/fidelity/lnszoom.md` §7.2.

## A26 — 3DSINFLD: the render's vblank count is a fixed two, the reference's occasionally returns one (registered)

**Part:** 3DSINFLD (`COMAN/MAIN.C` over `COMAN/ASM.ASM` and the generated
`COMAN/THELOOP.INC`, Phase 5).

The source's `frame += (frepeat = dis_waitb())` (`MAIN.C:105`, `:111`) takes
whatever the DIS's INDEMO wait returns — the number of copper frames since
the previous call (`DIS/DISINT.ASM:161-183`). The port's model is a fixed
**two** (`VBLANKS_PER_ITERATION`), plus the frame-4 body's two waits
(`MAIN.C:69-73`). It reproduces every keyframe byte for byte at measured
ticks, but it drifts against the reference's own clock: the four keyframes
are the port's ticks **464, 966, 1470 and 1772** and the reference's frames
**15500, 16000, 16500 and 16800**, so the implied origin is **15036, 15034,
15030 and 15028** — 2, 4 and 2 frames of drift across the 500-, 500- and
300-frame spans. In iteration terms the reference renders 500 / ((966-464)/2)
= **1.992** frames an iteration across the first span, where the port's two
is about 0.4 % slow.

The pictures do not depend on the count: the camera and wave cursors advance
once an iteration, the palette is static, and `startrise` moves once a
vblank, which the model reproduces at one a tick. The comparison is
byte-identical at all four keyframes
(`tests/coman.rs::the_four_keyframes_at_the_measured_ticks`, 0 differing
pixels each), and `docs/fidelity/3dsinfld.md` §6 states the origin per
keyframe rather than as an anchor.

**Not adopted, and why the other side is not readable.** A variable model
would need the release's render time, and neither the tree nor the reference
records it: the block-3 music traces carry `ord`/`row`/`zframe`, not the
part's waits, and `waitb_1`'s non-INDEMO arm is a literal 1 marked `TEMP!`
(`:178-179`) while the INDEMO arm returns the count. The port keeps the
model the pictures pin.

## A27 — JPLOGO: the `doit` counter's fixed one-vblank step, and the 17700 filing (registered)

**Part:** JPLOGO (`JPLOGO/JP.C` over `ASM.ASM`, the generated `ZOOM.INC` and
`READP.C`), Phase 5.

The source's `frame += c` (`JP.C:77`) adds the number of copper frames the
iteration took: `dis_waitb` returns the frames since the previous call
(`DIS/DISINT.ASM:161-183`). A `doit` iteration redraws up to 400 rows with
the unrolled `ZOOM.INC` routines and can take several; how many depends on
the host's speed, and a jump past 511 freezes `y1`/`y2` at whatever the
previous iteration used (`JP.C:78-83`). The port models **one vblank an
iteration** (`frame += 1`), which is the reference's own cadence at the filed
keyframes: 17300 is tick 235 and 17500 tick 435, 0 differing pixels each.

The row *images* depend only on the counter's current `y1`/`y2`; the
*screen* depends on the path, through the palimpsest guard
(`lasty`/`lasts`, `JP.C:96-99`): how far the counter jumps decides which
pre-settle rows an earlier state leaves behind. The fixed step reproduces
the **filed keyframes and the settled state**, not every intermediate
wobble frame of another session: **scratch observations**, not reference
evidence — `out/b4b-jp-sweep1` is a git-ignored diagnostic capture whose
session phase the tree does not hold. The port was dumped at the mapping's
ticks with

```bash
target/debug/secondreality --part jplogo --headless \
    --dump-frames 135,335,535 --dump-dir out/jp-sweep-repro
```

and those RGB frames compared against the capture's 17200/17400/17600
(320×400; a pixel differs iff any channel differs): 17200 matches at
**0.0000 %** (0/128,000), while 17400 and 17600 are **45.6023 %**
(58,371/128,000) and **39.8672 %** (51,030/128,000) away, max channel delta
255. The captures are not reproducible from a clone and the numbers are
recorded as the diagnostic observations they are; `tests/jplogo.rs` pins the
filed keyframes and the settled state, which are the part's evidence. The
census records the consequence (`reference-indices.md` §17.3, §17.5):

- 17300's filed picture is 4/4; 17500's is 2/4;
- 17700's *state* is settled 2/4 (`ec3de2bb…`) while the filed picture
  (`91d6ce6b…`) is presented by **0/4**; twelve captures hold ten distinct
  byte-images at 17700 and seven re-capture probes did not draw the filed
  one.

The port reproduces the settled state: its 17700 is byte-identical to the
census's `-sess2`/`jp-sess` state — `sha256sum` of
`--dump-frames 635`'s RGB is the census's
`ec3de2bb823772bf275387ab122e12e17b1d27a140ec9be8a71faa7d1bd3cb37` — and
its residual against the filed
picture is exactly that session's — **51,445 px of 128,000, 40.1914 %, max
channel delta 255** (`tests/jplogo.rs` asserts the number; `cargo xtask
compare --reference reference/parts/jplogo --rendered out/b4b-jp-sess2`
prints the same). This is D24's register: the filing is outside its own
census and no faithful deterministic model can be asked to reproduce a
state the reference itself took twelve captures to spread over ten
pictures. The port keeps the fast-session model and records the residual
rather than fitting a jump to the filing.

**Not adopted, and why the other side is not readable.** The reference
records the picture, not the iteration durations; the copper frame counter
is not in any trace, and the part's `waitb` returns a value no filed
capture carries. A cost model built from the `ZOOM.INC` instruction counts
would need the emulator's cycles-per-instruction and VGA wait states, none
of which the tree records.

## A28 — U2A: the release's Gouraud shading differs at 7300 (registered, not adopted)

**Part:** U2A (`VISU/C/U2A.C` over the VISU engine), Phase 6.

**Original:** the reference's 7300 picture is the shipped build's rendering of
stream state 294 — the state search in the port's unit tests finds it as the
unique minimum (426 differing pixels; 892 at state 293 and 858 at 295) — and
the difference is in the Gouraud shading of the three `U2A.001` instances. The
object silhouette and the flat-shaded `U2A.002` object are exact.

**Port:** the tree's algorithm — `ACALC.ASM`'s matrix and normal rotations,
`ADRAW.ASM`/`ADRAWCLP.ASM`'s culling, clipping and span generation, and
`AVIDFILL.ASM`'s three Gouraud tiers — transliterated in
`bins/secondreality/src/parts/visu/`. At state 294 it differs from the
reference in **426 of 64,000 pixels (0.6656 %)**, entirely on the Gouraud
faces: attributing each differing pixel to the topmost object that writes it
gives 75 on instance 1, 0 on instance 2, 115 on 4 and 236 on 5, with
palette-index deltas from −5 to +12.

**Why (D24 step 3).** No tree-carriable constant reproduces the release's
output. The variant scan in `docs/fidelity/u2a.md` §5 covers the face lists,
draw order, matrix order, normal-rotation variants, camera multipliers,
Gouraud slope/tier/buffer variants and lighting variants, and an independent
transliteration of the tree's drawing path agrees with the Rust engine byte
for byte at state 294; the tree's two filler versions (`AVIDFILL.ASM`,
`AVIDFILL.OK`) differ only in copy loops, not in the Gouraud code. The
release's own constants cannot be read without unpacking: `SECOND.EXE`
carries no `U2A`/`AVISTAN` strings and no six-byte light-vector sequence, so
D24 step 2's "can the tree produce it" cannot be answered from the packed
artifact. A one-frame fit to a single Gouraud picture would be unverifiable
against another Gouraud frame.

**Measured difference:** 426/64,000 = 0.6656 %, all on the Gouraud faces; the
flat object and the silhouette are exact. Reproduce with
`cargo test -p secondreality the_reference_7300_is_the_measured_stream_state`.

## A29 — U2A: the 7300 boot-index picture is a session-phase offset (registered)

**Part:** U2A (`VISU/C/U2A.C`), Phase 6.

**Original:** boot 7300 displays stream state 294 in the capture session whose
keyframes are filed at `reference/parts/u2a`.

**Port:** boot 7300 displays stream state 297, three states later; **1098 of
64,000 pixels (1.7156 %)** differ, in both the RGB and the index plane.

**Why (D17).** The offset is the music clock's session phase, amplified by the
part's page holds. The port's block-1 clock matches `second1`: at boot 5900 it
reads **9:51 / zframe 3771**, the trace's own value, while `second1c` — the
trace that supplies the gate-1 measurement the plan carried — reads
**9:50 / zframe 3768** at the same index. The 6988 gate is therefore
`second1c`'s, not the keyframe capture session's, and the port is faithful to
`second1`. Two to three ticks of music phase move the gate, and the ring
displays each page for four to five stream states, so the visible state at
7300 moves with it. Changing the block-1 clock to match `second1c` would move
ALKU's verified frames.

**Measured difference:** 1098/64,000 = 1.7156 %, RGB and indices. Reproduce
with `cargo test -p secondreality --test u2a
the_saucer_frame_records_its_measured_divergence`,
`cargo test -p secondreality --test u2a
the_block_one_clock_matches_second1_at_boot_5900` (the 9:51/zframe 3771 pin)
and `cargo test -p secondreality
the_reference_7300_is_the_measured_stream_state` (1098 belongs to state 297
and to no other of the 522), plus the dump command in
`docs/fidelity/u2a.md` §7.

## A30 — U2E: the release's Gouraud shading differs at the four keyframes (registered, not adopted)

**Part:** U2E (`VISU/C/U2E.C` over the VISU engine), Phase 6.

**Original:** the four filed keyframes render the release's Gouraud shading on
the moving objects. At the reference's own stream state — the port's `K - 4`
frame, the state search's unique minimum for every keyframe — the silhouettes
are exact and the differences are confined to the Gouraud faces of `s01`
(123 of its 124 faces are Gouraud), `Tree01g` (64 of 70) and `Car02` (all 10).

**Port:** the tree's algorithm, the same engine `parts/visu/` carries for
U2A. At the reference's own state the residual is **189 / 7,118 / 231 / 1,177
of 64,000 pixels (0.2953 / 11.1219 / 0.3609 / 1.8391 %)** for 1000 / 1600 /
2600 / 3600. Every flat-shaded object and every silhouette reproduces exactly;
the reference-derived palette-index deltas run from −8 to +12 and are dominated
by **+1** (6,655 of 7,118 at 1600).

**Why (D24 step 3).** This is the same release-versus-tree Gouraud difference
A28 registers for U2A: the same engine reproduces the flat faces and the
silhouettes byte for byte, the tree's lighting and filler arithmetic match
`ADRAW.ASM`/`AVIDFILL.ASM` as transliterated, and no tree-carriable constant
turns a whole object's Gouraud interpolation into the release's. The release's
constants are not readable from `SECOND.EXE` (D24 step 2), and fitting one
keyframe's Gouraud shade would be unverifiable against another.

**Measured difference:** 189, 7,118, 231 and 1,177 of 64,000 pixels
(0.2953 / 11.1219 / 0.3609 / 1.8391 %), all on the named Gouraud faces;
silhouettes and flat objects exact. Reproduce with
`cargo test --release -p secondreality --bin secondreality
search_reference_states -- --ignored --nocapture` and
`cargo test -p secondreality --test u2e
the_keyframes_record_the_measured_session_phase_and_gouraud_residual`.

## A31 — U2E: the block-4 keyframe boot indices are a session-phase intercept (registered)

**Part:** U2E (`VISU/C/U2E.C`), Phase 6.

**Original:** the filed keyframe capture displays stream state `(t - 702)/2` at
boot `t`; at 1000 / 1600 / 2600 / 3600 that is states 149 / 449 / 949 / 1449.

**Port:** the displayed state is `(t - 698)/2` — the same slope to the frame, a
**four-tick (two-state) intercept**. The pictures at the exact keyframes differ
by **11,006 / 11,805 / 17,278 / 16,234 of 64,000 pixels (17.1969 / 18.4453 /
26.9969 / 25.3656 %)**.

**Why (D17).** The port's music clock reaches order 19 at boot 698 where the
filed `second4` trace reads it at 699, and the keyframe capture is another
session; the four-tick offset is the same class of session phase A29 registers
for U2A's 7300. The port's intro is byte-exact against the sessions that agree
with it (boot 460..660 at 0 differing), which pins the 418 anchor and the
70 Hz tick; changing the block-4 start or the gate to chase the keyframes
would move the verified intro. No anchor, cadence or clock was fitted.

**Measured difference:** 11,006 / 11,805 / 17,278 / 16,234 of 64,000 pixels
(17.1969 / 18.4453 / 26.9969 / 25.3656 %) at 1000 / 1600 / 2600 / 3600, with
the aligned-state residual A30's. Reproduce with
`cargo test -p secondreality --test u2e
the_keyframes_record_the_measured_session_phase_and_gouraud_residual` and the
`search_reference_states` diagnostic above.

## A32 — U2A: the pre-gate pad palette is the release's measured bright transform (adopted)

**Part:** U2A (`VISU/C/U2A.C` over the VISU engine), Phase 6.

**Original:** the tree draws nothing before its first music gate (`U2A.C:229-240`), but the
release paints the `U2ABG.UH` picture at the part's start and shows it under a **bright**
palette until the gate. The released palette is not the tree's merged scene palette and not
ALKU's final DAC: both match **0 of 64,000** pixels of the reference's 6500 frame. It is the
background palette passed through `(3 × bg + 5) & 63` on every channel — reconstructed with
zero conflicts from the 6500 RGB and the 7640 index plane (52 entries) and independently from
7300's own RGB by its own index plane — with **entry 195 black**, the entry the background's
white stars use.

**Port:** `init` paints page 0 with the `U2ABG.UH` payload and uploads
`pad_entry(v) = (3v + 5) & 63` over all 256 entries with entry 195 forced black; the merged
scene palette goes up at gate 1, as the source has it. The whole 6500 frame then compares at
**0 of 64,000** pixels.

**Why (D24, adopted).** This is the release behaving where the tree does not, and the measured
formula is one constant expression the tree can carry; it reproduces every used entry but one,
and that one is a single forced black. Adopting the release's picture is the rule D24's
procedure gives when the value is measurable and reproducible; inventing the tree's silence
would be a visible miss at the reference's own keyframe. The 195 entry's 84 star pixels are
the exception the formula cannot express (it predicts `(2,2,2)` there), so the port carries
the measured exception rather than the formula's value.

**Measured difference:** the formula matches **57,516 of the 57,600** pixels that use indices
≥ 192; the remaining **84** are the index-195 pixels, which the adopted black entry renders
black — exactly the reference's pixels — so the full frame is **0/64,000**. Task 1's two
index-plane reconstructions and the candidate comparison are
`.superpowers/sdd/06-vector-engine/task-1-report.md` §4; the counts are now produced in-tree
by `cargo test -p secondreality --bin secondreality
the_pad_palette_is_the_measured_formula_with_entry_195_black`, which reads
`reference/parts/u2a/frame_007640.idx` and `frame_006500.rgb` with the `U2ABG.UH` palette and
asserts **57,600 / 84 / 57,516**, and the port's whole frame by `the_pad_hold_is_the_reference_6500`.

## A33 — U2A: the first draw uses `fov = 0` for the C's uninitialised variable (substitution; no measured difference)

**Part:** U2A (`VISU/C/U2A.C`), Phase 6.

**Original:** `int fov;` is declared inside the outer loop (`U2A.C:261`) and read by
`vid_cameraangle(fov)` (`:287`) before the first parse assigns it (`:348`). In the release's
build the stack slot holds whatever the preceding code left; the value is not defined by the
source.

**Port:** `Driver::new` starts with `fov = 0` (`parts/visu/mod.rs`). The first draw is made
with that value and the stream's `0x22` (`0x2200`) takes over from the first parsed frame.

**Why.** Every object is still off on the first draw — `U2A.0AB` frame 0 has no toggle; object
2 turns on at frame 16 — so the camera angle multiplies nothing and the frame is the cleared
background whatever the angle is. The C's uninitialised read is therefore unobservable in the
shipped data, and 0 is the deterministic choice rather than a guess at the release's stack.

**Measured difference:** **0 of 64,000** pixels. Reproduce with
`cargo test -p secondreality --bin secondreality the_first_displayed_page_is_the_cleared_background`,
which asserts the first rendered frame equals the cleared background under both palettes, and
`the_first_stream_frame_leaves_every_object_off_and_frame_sixteen_turns_object_two_on`, which
pins frame 0's all-off state and the `0x2200` that replaces the initial value.

## A34 — The vector engine fills a linear page, not the original's planar VGA memory (substitution; no measured difference)

**Part:** U2A and U2E (`VISU/AVIDFILL.ASM`, `ADRAW.ASM`, `ADRAWCLP.ASM`), Phase 6.

**Original:** the polygon pipeline emits a `{FILL-DATA}` stream and a VGA-planar filler writes
it in the unchained 320×200 mode: one page row is 80 addresses and one address holds four
pixels, one per plane (`x >> 2`, plane `x & 3`), selected by the map mask. The stream and the
planes are a VGA implementation detail of the tree's code.

**Port:** the span generators emit the same bands and `write_address` expands every planar
byte write to the pixels `4 × address + plane` for each plane the mask selects, into a linear
320-wide page. The tiers' one-, two- and four-pixel colour quantisation, `AFILLDIV.INC`'s
`65536/len` slope, the `0x8000` X bias, the increment-before-use row loop and the
right-exclusive stop are transliterated literally (`parts/visu/draw.rs`'s module comment).

**Why.** The port's surface is a linear indexed image; carrying a planar stream and a VGA map
mask would add a representation the engine does not use anywhere else. What is preserved is
the pixel set and its colours, which is what the reference compares.

**Measured difference:** **0** on every shipped comparison — the U2A 6500 and 7640 keyframes
are 0/64,000, U2E's intro is 0 differing at nine indices, and the registered Gouraud residuals
(A28/A30) have **0**-difference silhouettes, so they are colour deltas on faces the pipeline
drew, not a fill-geometry difference. Reproduce with
`cargo test -p secondreality --bin secondreality parts::visu::draw`
(the span/fill unit tests), `cargo test -p secondreality --test u2a` and
`cargo test -p secondreality --test u2e`.

## A35 — The four VGA pages are packed at 64,000 pixels, not the original's 65,536-pixel slots (substitution; no measured difference)

**Part:** U2A and U2E (`VISU/C/U2A.C:105-166`, `U2E.C:115-183` over `AVID.ASM:341-358`),
Phase 6.

**Original:** `vid_setswitch` gives page `p` the drawing segment `0xA000 + p*0x400` and the
display start `p << 14`, a page slot of 0x4000 bytes. In the emulator's four-plane byte-group
memory a plane byte is four linear pixels, so consecutive pages are **16,384 plane bytes =
65,536 linear pixels** apart, with the picture (16,000 plane bytes = 64,000 pixels) and a
1,536-pixel gap per slot.

**Port:** the surface packs the four pages at `page * 64_000` pixels (`PAGE_PIXELS`), with no
gap. Every page is cleared, drawn and read through the port's own origin, so each page shows
the same picture the original's does. **The reference's U2A keyframes are page 0's picture
at origin 0** — the one origin the two layouts share; the port's live boot index displays a
later page of the same ring, which the reference does not file, and U2E's page origins are
not established by its RGB-only keyframes. U2E's `fadeset` writes the same plane addresses
into each of the four pages; its top block's last write is 172 addresses past the row just
written, lands in rows `y+3..` and is overwritten by the later row passes, and the port's
page bound (16,000 addresses, the original's picture within its 0x4000-byte slot) never fires
for the shipped pattern, whose largest address is the bottom row's byte 15,999
(`docs/fidelity/u2e.md` §4).

**Why.** The port's pages are a linear indexed surface, not VGA memory; a gap buys nothing and
would have to be modelled in every page slice. The observable content — every page's pixels
under the shared palette and the origins the parts set — is identical.

**Measured difference:** **0** on every shipped comparison: U2A 6500/7640 at 0/64,000, U2E's
intro at 0 differing at nine indices, and the registered keyframe residuals are A28/A30's
Gouraud colour deltas with exact silhouettes. Reproduce with
`cargo test -p secondreality --bin secondreality the_fadeset_writes_the_source_s_pattern_within_its_page`
(the fadeset's plane expansion and page bound), `cargo test -p secondreality --test u2a` and
`cargo test -p secondreality --test u2e`.

## A36 — GLENZ: the block-2 keyframes are five ticks late, and the reference capture's DAC leads its surface (registered)

**Part:** GLENZ (`GLENZ/MAIN.C`), Phase 7.

**Original:** block 2's music gate (`MAIN.C:290`, `musplus < -19`) exits at
boot **1111** in `reference/music/second2b`, and the intro from the reset to
main-loop frame 44 is **495** ticks (first object pixels at **1606**, frame
44: `1606 − 44 = 1562 = L_r`). The filed moving keyframes are at
2300 / 3000 / 3400. Their capture carries a surface/DAC phase: with the
reference's linear counter (`1562 + f`), the filed 2300's surface is frame
**738** (lag 0), the filed 3000/3400 surfaces are frames **1437/1837**
against counters 1438/1838 (lag 1), and the DAC runs one frame ahead of the
surface in each (the filed 2300's low palette is `pal` after 738, b = 26;
the 3000/3400 group entries are the next frame's writes) — D17's
sample/publish instant. The filed pictures are byte-identical to the
review's fresh partner at the same indices (`reference-indices.md` §7.4),
and equal to `out/glenz-recapture4`'s 2299/2999/3399 (0 / 6 / 65 px); one
older Task 1 capture, `out/p7-stability`, presents the next frame at 2300.

**Port:** the gate exits at **1117**, six ticks late, but the port's intro
from the reset to frame 44 is **494** ticks (first object pixels at **1611**,
8 px, box 172..179 × 0..0; first faded surface at **2269**, frame 702,
b = 63, after 2268, b = 64), so **`L_p = 1567`** against `L_r = 1562` and
the plan's aligned index is **`K + 5`**. At `K + 5` (2305/3005/3405 against
2300/3000/3400) the port differs by **5,208 / 3,114 / 5,261 of 64,000
pixels (8.1375 / 4.8656 / 8.2203 %)**. At the surface minima (2305, and
3004/3404 for the capture's one-frame lag) the residuals are **5,208 /
2,289 / 5,941**; one frame past the plan's alignment (`K + 6`) they are
9,777 / 10,806 / 12,001.

**Why (D17, A29/A31's class).** The one-tick difference between the
reference's 495 and the port's 494 is the port's real player reaching
zframe 300/333 one tick earlier relative to the reset than `second2b` (the
494-vs-495 span); the phase machine's own relative ticks match the source and
are pinned by its unit test, and no transition was changed to move the loop
start. The remaining keyframe differences are the
capture's surface/DAC phase — a transient the port cannot draw: the port
pairs surface *N* with `pal` after *N−1* (the source's upload-before-work
order), while the capture samples after the upload or group writes and
before the paint. No anchor, tick or palette order was fitted.

**Measured difference:** as above. Reproduce with
`cargo test -p secondreality --test glenz the_loop_start_is_1567` and
`cargo test -p secondreality --test glenz
the_moving_keyframes_record_the_registered_alignments_and_residuals`, and
the dump/compare commands in `docs/fidelity/glenz.md` §11.

## A37 — GLENZ: the reference identifies the low-byte colour key (adopted); the surface minima's residual is registered-unexplained

**Part:** GLENZ (`GLENZ/VEC.ASM:403-506`), Phase 7.

**Original:** the reference's moving screens draw every face and keep the low
palette equal to the frame's `pal`. At boot 2305 — the filed 2300's surface,
main frame 738 — the 5,208 differing pixels against the filed picture are
exactly palette entries 0..15 and **0** have an index ≥ 16: the reference's
colours are the fade at `b = 26` (the value computed at main frame 738), the
port's at `b = 27` (its surface 738 pairs with `pal` after 737). That is the
capture's one-frame DAC lead, not a colour-model difference.

**Port:** the tree's `demo_glz` keys `rolcol` by the **full colour word** for
visible faces (`VEC.ASM:450`) and the **low byte** for hidden ones (`:417`);
with the 256-byte table those maps are independent, so the fifteen even slots
fill with every word that has ever been visible and the sixteenth takes slot
32. The slot-32 group is written at DAC entry 0 (the background reads the
group ramp: boot 2306's sky is `(97,97,97)` where the faded picture is black)
and its stored colour is 0, so every post-exhaustion face vanishes (the 2306
object box is 8,066 px against the reference's 9,927/9,989; the picture
numbers are from the pre-adoption tree, `63b4267`, with §11's dump command,
and the mechanism is pinned on the committed tree by
`glz_pool_exhaustion_uses_slot_32_and_entry_0`). **The reference's picture
identifies the in-bounds low-byte key on both paths** — one
`slots: [u8; 256]`, with a hidden face freeing the slot its visible face took
— and the port adopts that reading (the packed EXE cannot be disassembled to
confirm it). The adoption is exact against the filed 2300: at boot 2305 the
object box is 9,989 px, (156.8, 94.1), and every object pixel is
byte-identical. The surface minima's residuals are **5,208 / 2,289 / 5,941
px (8.1375 / 3.5766 / 9.2828 %)** at 2300 / 3000 / 3400; at 3000 / 3400 the
**711 / 839 px** of the 35,896 / 43,938 object pixels that the index
structure does not explain (710 of 711 at indices < 16 at 3004, all 839 at
3404) are **registered-unexplained**: the second object's interior XOR
boundaries, its outer red silhouette matching within **118 / 180 px** (the
port's red masks hold 27,057 / 33,312 px). The sub-pixel edge/phase cause is
a hypothesis, not a claim.

**Why (D24 step 3).** The key is one expression the tree's own algorithm can
carry, and the picture is the evidence for it. D24 (a) — the shipped
projection/window — is not contradicted: the same `INIT_320X200` constants
and the chunky 320-pixel-row model reproduce the reference's 2300 object
pixels exactly at 2305, so no shipped constant is adopted. The residual is
registered, not fitted: no constant, shade or offset resolves it without
breaking the exact 2305 match. The derivation's permanent test is
`glenz::poly::tests::glz_hidden_face_frees_the_visible_slot` (a hidden face
frees the visible slot and the next colour reuses it).

**Measured difference:** as above. Reproduce with
`cargo test -p secondreality --test glenz
the_moving_keyframes_record_the_registered_alignments_and_residuals` and
`cargo test -p secondreality --bin secondreality
glz_hidden_face_frees_the_visible_slot`.

## A38 — GLENZ: block 1's zoomer starts 249 frames after the reference's, because the restart seeks MUSIC1 from order 0 (registered)

**Part:** GLENZ (`GLENZ/MAIN.C:290`) through the shell's block-1 restart
(`MAIN/U2.ASM:813-818`), Phase 7.

**Original.** Block 1's `restartmus(1, 0, 48700)` fires between BEGLOGO (`exe3`) and
GLENZ (`exe4`). The reference's restart lies in the interval **(8,049, 8,100)**: the
pre-reload window `out/second1d-limit` (frames 7,800..8,050, scratch) reads MUSIC0 at
8,049 with the player's positions intact, and the filed `second1d` (8,100..8,800) opens
on **MUSIC1 order 1 row 4** at 8,100 — already 68 rows into the restarted module. The
reload hides the restart frame itself, and the wide 7,800..8,800 window refuses across
it (exit 2: `_np_ord reads -1, which is not a byte value: the base 0x0051F0 is not this
segment's`). The reference's WaitMusic exit is **8,521**: `second1d` reads order 2
row 45 (`musplus` −19, `zplus` 1, `zframe` 5,588 → 0), and
`out/p7-b1-collapse-dense` (frames 8,516…8,528) holds through 8,521 and first differs
on **8,522** (the sample/publish instant, D17).

**Port.** The restart installs MUSIC1@0 at the port's BEGLOGO→GLENZ boundary, **8,057**
(inside the reference's interval), with D34's four-tick `waiting`. The wait
(`MAIN.C:290`, `musplus < -19`) ends on the first published order 2 row 45; from order 0
row 0 that is 173 rows × 3 ticks = 519 music ticks ≈ 699 frames, and the port's first
zoomer body is at boot **8,770** — **713 ticks after the boundary** and **249 frames
after the reference's 8,521**. The port's 8,770 is the first frame that differs from the
held picture and publishes `2:45`; 8,769 still holds at `2:44`.

**Why (registered, not fitted).** The seek is the source's own (`restartmus(1,0)` seeks
the module to order 0 row 0); the reference's player had already played a stretch of
MUSIC1 when its part chain reached GLENZ, and `second1d`'s first sample cannot tell a
seek that left the row from a play that began during the module load (D38's recorded
limit). No row, order, frame or tick was adjusted to reduce the 249: the interval is
the reference's own record, the port's boundary is the chain's measured one, and the
difference is the consequence of the source-faithful seek. It is not the same quantity
as A36 (block 2's loop start): this is block 1's *wait length*, measured on the shell's
chain.

**Measured difference:** **8,770 − 8,521 = 249 frames** (the wait's exit); the port's
boundary 8,057 lies inside the reference's unreconciled `(8,049, 8,100)`. Reproduce
with

```text
cargo test -p secondreality --test shell \
    a_block_one_run_is_alku_then_u2a_then_pam_in_the_boots_numbering
cargo test -p secondreality --bin secondreality \
    the_block_one_run_restarts_music1_at_glenz
awk -F, '$1 == 8521' reference/music/second1d/trace.csv
cargo run -q -p xtask -- music trace --cycles 30000 --frames 7800..8050 --arg 1 \
    --out out/second1d-limit
awk -F, '$1 == 8049' out/second1d-limit/trace.csv
```

The chain test pins 8,057, 8,770, 713, 249 and reads `second1d`'s 8,521 = `2:45`; the
restart test pins the fresh install and `waiting == 4`. The two `awk` lines print
`8521,2,45,50,0,3,130,0,1,0` and `8049,14,4,31,0,6,120,0,0,5239`.

**Phase 9 Task 6: the lag is the rest of block 1, and its probable cause is the player's
initialiser, not the seek.** The full run (`reference-indices.md` §26.8, `cargo test --release -p secondreality
--test fullrun -- --nocapture`) is **247 frames late** at `second1d`'s first rows and
**248** at `second1e`'s, and every part from GLENZ to JPLOGO inherits it (A53's seam table: start
differences +230…+259). Three committed tests point at why. `_zplaysong`'s per-channel
initialiser (`+0x5614`) writes `achannelused` as `0x80` and `lastins` as 0 and **never `avol` or
`a0volcut`** (`cargo test -p sr-audio --test player
the_song_initialiser_leaves_the_channel_volume_and_its_zero_countdown`), so a mid-song seek keeps
the previous module's volumes, and a note-off with `lastins` 0 returns before it can zero one.
MUSIC1's order 0 is pattern 73, whose row 3 is a `C00` on channel 2 and nothing else; a fresh
player — a boot, and every restart the port installs — has channel 2 at volume 0, the "volume 0"
countdown frees it on row 3 before the command is dispatched, and order 0 plays 64 rows (what block
2's boot trace `second2b` shows). Block 1's MUSIC0 has channel 2 at 63 around 8,057, so the
original probably takes the break and plays order 0 for **4** rows: back from `second1d`'s first `1:5`
(8,102), a 64-row order 0 starts the restart at **7,814** — while `second1b` still publishes MUSIC0
at 7,959 — and a 4-row one at **8,056.6**, the port's own boundary (`cargo test -p sr-audio --test
trace block_ones_music1_restart_takes_order_0s_break -- --nocapture`). The 60 rows between, 242.6
frames, are this entry's 249 but for 6. This is the probable cause, confirmed as far as the
committed evidence goes by those two tests; the paragraph above's reading ("the reference's player
had already played a stretch of MUSIC1") stands as the entry's record until the port's fix — Task 6b —
lands and re-measures it. **Not changed here**: modelling it — `Channel::reset` keeping what the initialiser keeps, and the shell's
restart handing the new player the old one's channels — is a player change that moves this entry's
pinned 8,770/249, the trace test's `RESTART_TRACES` rows for `second1d`/`second1e` (7,815, the
back-extrapolation through a 64-row order 0) and possibly the U2E and loop restarts, so it is Task
6b's.

## A39 — GLENZ: the span compositor repaints every span instead of walking the previous frame's list (substitution; measured difference 0)

**Part:** GLENZ (`GLENZ/NEW.ASM:66-172`, `ng_pass3`), Phase 7.

**Original.** The screen is not cleared between frames. `ng_pass3` walks the previous
frame's boundary list and the new one together, keeping the new and old accumulated
colours, and repaints `[prev, position)` **only where the two colours differ**. Every
span's pixels are `colour | bgpic[pixel]`, so a span whose colour did not change already
holds exactly the bytes a repaint would write: the walk is an optimisation with
identical pixels.

**Port.** `span.rs`'s `Compositor::paint` drops the dirty-span walk and repaints every
span from `bgpic`, never clearing the buffer. The equivalence is not assumed: the test
module keeps the source's two-list walk beside the port and compares whole 64,000-byte
frames over a 13-frame corpus (including the sentinel pixel 63,999, which neither
implementation writes).

**Why.** No pixel differs; the substitution removes a stateful walk the port would
otherwise have to carry between ticks. No constant, upload order or boundary changed.

**Measured difference:** **0** — byte-identical frames over the corpus (twelve generated
frames of moving, rotating and held polygons plus an empty frame, 13 in all; 64,000 bytes a
frame). Reproduce with
`cargo test -p secondreality --bin secondreality
parts::glenz::span::tests::the_full_repaint_equals_the_source_two_list_walk`.

## A40 — GRID: the developer pause is a start event, not a key wait (substitution; measured difference 0)

**Part:** GRID (`GRID/MAIN.C:144-153`), Phase 8 Task 3.

**Original.** After the eye is shifted and copied and 1,024 points are plotted in colour
15, `getch()` (`MAIN.C:153`) blocks until a BIOS key. The reference capture
(`tools/vault_capture.sh grid`, `--keys KBD_space`) delivers exactly one `KBD_space`
there; the dots are overwritten by the eye copy at `MAIN.C:156` and are never part of the
animation. The loop's only other key-sensitive calls are `dis_exit()` (the `while`
condition, `MAIN.C:158`) and `dis_muscode(3)` (`:160`), which the standalone DIS answers
0 until a consumed key, then 3 once.

**Port.** `Grid::init` performs the source's **loop iteration 0** — the frame the
capture files as the start-up transition, reference 352 — and the first composed frame is
iteration 1, reference 353 (`K0`). No key is waited on (the engine's contract: nothing
waits on a key), and `dis_muscode(3)` is not called: the port's `muscode` is 0 with no
player (D8), and the capture delivers no second key. The dot screen is not drawn; it is
the measurement that pins `d0` (below), not part of the animation.

**Why.** The reference's one start key selects exactly the state the port starts in — the
animation after the eye copy. The dots and the transition frame are both overwritten
before the first byte-reproducible loop frame; the substitution removes the pause without
moving anything that is compared.

**Measured difference:** **not separately measurable** — the start key selects the state the
port starts in, and the five compared frames carry **A41**'s 6/2/36/33/14 sampling residual,
not a difference from this substitution. The anchor is reference 353, and the port's frame 0
is that frame (Task 2's ruling puts `K0` on the first stable loop frame, not on the change at
352). Reproduce with
`cargo test -p secondreality --test vault grid_matches_the_reference_at_its_measured_start`
and the capture command in `docs/fidelity/grid.md`.

## A41 — GRID: the filed RGB frames catch the newest line mid-draw in its top rows (registered)

**Part:** GRID, Phase 8 Task 3.

**Original and capture.** The screen moves during the indexed read, so the oracle keeps
the RGB frame (D15/D19); `reference/parts/grid` holds five `.rgb` files and no `.idx`.
At each filed keyframe the emulator published its frame while the original was still
drawing the top of the current line, so the newest line is missing from, or one red rank
lower in, rows 14–15 — the topmost rows the curve reaches.

**Port.** The part composes the completed frame after the whole loop body (surface copy
at the end of `Grid::frame`), as the port's tick model requires.

**Why (registered, not fitted).** The difference is a sampling instant, not a model
error: every other pixel of every frame — the eye with its 100 unshifted bytes, the three
line colours, the palettes and the (−10,−8) fill-word artefacts — matches, and the
residual is an order of magnitude smaller than the ±1 neighbours' (about 10,000–12,000
pixels; the test prints them). The same class appeared in a scratch route-B comparison
(2/24 pixels at frames 800/1000; historical — the route-B capture is the `RUNP.BAT` recipe of
`reference-indices.md` §24.2 and the mapping is `K0` = 704, so the port frames are 96 and 296;
it is not a filed claim). No constant, anchor or palette phase was changed to
reduce it (D17/D18/D24).

**Measured difference:** **6 / 2 / 36 / 33 / 14** differing pixels at reference
400 / 500 / 600 / 700 / 800, all on rows 14–15, max channel delta 41. The minimum is at
offset 0 (the `K0` mapping) for all five. Reproduce with
`cargo test -p secondreality --test vault -- --nocapture
grid_matches_the_reference_at_its_measured_start`, which prints the three counts, the
minimum's offset, the max delta and the row histogram, and asserts that every differing row is
14 or 15.

## A42 — TWIST: the copper test is not registered (substitution; measured difference 0)

**Part:** TWIST (`TWIST/ASM.ASM:220-269`), Phase 8 Task 4.

**Original.** `main` calls `initcoppers` (`TWIST/MAIN.C:89`) after `inittwk`. It registers
`testcopper` in DIS service-7 slots 0 and 1; `testcopper` sets DAC entry 0 white, spins 100
`loop` iterations and sets it black again. It writes no CRTC register, no line compare and
no split.

**Port.** `Twist::init` registers no copper callback. The standalone `DIS/DIS.EXE` — the
only server the vault's T.EXE recipe can use — is built without `INDEMO`, and its service 7
is a bare `ret` (`DISINT.ASM:294-316` is inside `IFDEF INDEMO`; the shipped image's
`setcopper_7` at offset 0x2f7 is a `ret`). The reference therefore shows no flash, and the
engine's palette timeline could not express the intra-frame white→black band without a
non-literal translation and an unmeasured duration (research/twist.md §2.5).

**Why.** Registering the callback would invent a visible flash the artifact cannot produce;
registering a documented no-op adds state with no effect. The comparison's 0 residual at
all six keyframes is the measurement that the flash is absent.

**Measured difference:** **0**. Reproduce with
`cargo test -p secondreality --test vault -- --nocapture
twist_matches_the_reference_at_its_measured_start`; see also the capture in
`docs/fidelity/twist.md`.

## A43 — TWIST: any-key exit and the Scroll-Lock border become ESC/Ctrl (substitution; measured difference 0)

**Part:** TWIST (`TWIST/MAIN.C:21-28,49,102`), Phase 8 Task 4.

**Original.** `doit` exits on any pending BIOS key (`kbhit()`), and `main` consumes it with
`getch()`. `waitb` tests BIOS keyboard-flags byte `0000:0417` bit 4 — **Scroll Lock**, despite
the variable's `shiftstatus` name — and toggles the overscan colour to 0 before and 31 after
the retrace wait. Both are developer input; the reference capture sends no key, so neither
path runs.

**Port.** The part ends on `ctx.exit_requested`, which the shell sets for **ESC/Ctrl only**;
the Scroll-Lock border is not tested and the overscan stays black (`view.border = Some(0)`).
This is the port's input model, the same one every other part has: nothing waits on a key,
and the capture's vocabulary is one start/exit event, not arbitrary keys.

**Why.** The reference behaviour the captures exercise (no key at all) is identical; the
substitution removes a BIOS flag and a border colour the port cannot set from the same
source. The original's "any key" is wider than the port's ESC/Ctrl, which is recorded here
rather than hidden.

**Measured difference:** **0** in all six compared frames (and in the 4,444-frame run,
which never sees a key). Reproduce with
`cargo test -p secondreality --test vault -- --nocapture
twist_matches_the_reference_at_its_measured_start` and
`twist_draws_exactly_4444_frames_and_holds`.

## A44 — TWIST: one 70.086 Hz tick stands for one VGA retrace (registered mapping; no overrun model)

**Part:** TWIST (`TWIST/MAIN.C:21-28,49`), Phase 8 Task 4.

**Original.** The source has no timer. `doit`'s only time base is `waitb`, which polls VGA
input status 0x3DA for the start and then the end of vertical retrace and returns 1
unconditionally; `frame += waitb()` therefore counts retraces. `inittwk` leaves mode 13h's
vertical timing untouched (0x09's "400 rows" write is commented out), whose nominal rate is
25.175 MHz / (800 × 449) = **70.086 Hz**.

**Port.** One engine logic tick is one `doit` iteration, i.e. one displayed frame, at the
fixed 70.086 Hz tick (`crates/sr-engine/src/tick.rs`, `HZ_MILLI = 70_086`). The port does
not model a missed retrace: if a draw overran a frame the original would silently slow below
70.086 Hz while its `frame` still counted one per iteration, and the port cannot observe the
difference because the source cannot either. The draw is 200 × 80 = 16,000 plane bytes; the
capture report's timing estimate (`research/twist.md` §4.5, at the capture's 30,000
cycles/ms) puts it inside a frame, but that is the report's estimate, not this task's
measurement. The 4,444-iteration cap is 4,444 / 70.086 = **63.41 s** nominal; the compared
frames are phase-locked by tick index, never by wall clock.

**Why.** The retrace is the source's clock, and the fixed tick is the port's model of it for
every part (spec §6). The approximation is the rate, not the frame sequence; the six
keyframes match at 0 pixels, so no per-frame phase depends on it.

**Measured difference:** **0** per compared frame; the nominal duration is a derived
mapping, not a measurement. Reproduce the frame sequence with
`cargo test -p secondreality --test vault -- --nocapture
twist_matches_the_reference_at_its_measured_start`; the cap with
`twist_draws_exactly_4444_frames_and_holds`.

## A45 — TWIST: the comparison is RGB-only (registered limitation; measured difference 0)

**Part:** TWIST reference and port, Phase 8 Task 4.

**Capture.** The filed reference (`reference/parts/twist`, six frames 103..603) is RGB-only.
The moving screen fails the indexed read's coherence check (D15/D19), and TWIST has no
`.idx` even in principle: an odd part frame draws and displays page 2 at pixel 131,072, and
the oracle's indexed read always starts at plane offset 0, so it would read page 1. All six
keyframes are even part frames (effect frames 100..600), so their page is the right one, but
the read still cannot pass the coherence check while the picture moves.

**Port.** The comparison test dumps the port's `.rgb` at `R-K0-1, R-K0, R-K0+1` and compares
RGB. Indexed is normally the stricter side; the shared helper compares a reference `.idx`
where one exists (HARD's held 840) and only at the offset the RGB search chose, but no TWIST
reference in this tree carries one and the oracle as built cannot supply one for either
page.

**Why.** The limitation is the oracle's, not a substitution in the port; the RGB comparison
is exact, so nothing is lost for this effect. No constant or anchor was fitted to it.

**Measured difference:** **0** differing pixels at all six keyframes, offset 0. Reproduce
with `cargo test -p secondreality --test vault -- --nocapture
twist_matches_the_reference_at_its_measured_start`; the capture command is in
`docs/fidelity/twist.md`.

## A46 — HARD: the start gate is the capture's one key, and the 0xff cut is unreachable (substitution; measured difference 0)

**Part:** HARD (`HARD/B.C:138`, `HARD/BP.C:263`), Phase 8 Task 6.

**Original.** The standalone DIS is the non-INDEMO server: `muscode_6` returns 0 until
`checkkeys` consumes a key, then echoes the caller's wanted code once (`DIS/DISINT.ASM:242-282`,
`:154`). `B.EXE` opens with `while (dis_muscode(1) != 1 && !dis_exit());` and part 3 checks
`if (dis_muscode(0xff) == 0xff) NEXTMODE` once a frame; a **second** key would cut part 3 early.
The capture delivers exactly one key (`KBD_space`), which releases the gate and never reaches
the cut.

**Port.** The vault run has no player and no key: the part starts at `init` with the gate
passed and never models the echo. The `0xff` cut cannot fire (the port's `muscode` is 0 and
the capture sends no second key); `the_0xff_cut_is_unreachable_with_one_start` pins it on
the full run, where part 3 ends at its own count 800. This is the port's input model —
nothing waits on a key (spec §6, D8's measurement that the demo's own `muscode` is 0).

**Why.** A literal `muscode(1)` wait would hang for ever against the port's `muscode`, exactly
as it does against the reference's 0. The capture's one key is the source-supported start
event, and the gate is before `K0`, so no compared picture sees it.

**Measured difference:** **0**. The nine keyframes 480…1600 are exact and 840's `.idx` is
byte for byte; reproduce with `cargo test -p secondreality --test vault -- --nocapture
hard_matches_the_reference_at_its_keyframes`.

**Exit polling.** The source polls `dis_exit` at exactly two places: the 70-wait loop
(`B.C:162`) and the main loop's condition (`B.C:203`). The wipe and the fill (`B.C:143-161`)
do not poll. The port polls at the same points: a flag set during the wipe or the fill is
ignored until the next wait, the wait loop breaks without consuming a wait, the letterbox
still runs (`B.C:164-182`), and the main loop's first check retires the part without an
iteration. The substitution is only the source of the flag (the shell's ESC rather than the
DIS's `exitflag`), not its polling points;
`the_exit_flag_is_polled_where_the_source_polls_it` pins them.

## A47 — HARD: one 70.086 Hz tick stands for one waitb, with the display phase measured (registered mapping; measured difference 0)

**Part:** HARD (`HARD/B.C:203-324`, `BP.C:29`), Phase 8 Task 6.

**Original.** The source has no timer; every loop iteration ends in `dis_waitb`
(`B.C:207`), which polls VGA input status 0x3DA for the retrace. The display runs behind the
logic by one retrace: the frame at a retrace shows the previous iteration's drawing and the
palette that iteration uploaded. `part1init` calls its own `waitb` (`BP.C:29`), so mode 1's
count 0 occupies two display frames, and mode 4's band is uploaded at the top of the iteration
before the switch rebuilds it, so a frame shows the previous iteration's band.

**Port.** One engine logic tick is one source iteration, i.e. one displayed frame, at the
fixed 70.086 Hz tick (`crates/sr-engine/src/tick.rs`, `HZ_MILLI = 70_086`). The port's frame
`t` is the capture's reference `356 + t` (`K0 = 356`): the pre-roll is 119 ticks (20 wipe
steps, 28 fill iterations, 70 waits, the letterbox), `part1init` adds the measured extra
display tick, and the palette is shown one iteration behind in mode 4 and in the same
iteration in part 1. The port does not model a missed retrace inside a mode: if a draw
overran a frame the original would silently slow below 70.086 Hz while its loop still
counted one iteration. The one place the capture shows that cost is the mode-5 boundary's
two displays (A48), which the port inserts as two static ticks.

**Why.** The retrace is the source's clock and the fixed tick is the port's model of it for
every part (spec §6). The approximation is the rate, not the frame sequence: with the phase
above every filed keyframe before mode 5 is exact.

**Measured difference:** **0** pixels at all nine keyframes 480/560/630/700/770/840/910/
1050/1600, offset 0; 840's indexed plane is byte-identical. Reproduce with `cargo test -p
secondreality --test vault -- --nocapture hard_matches_the_reference_at_its_keyframes`; the
tick model's own counts are pinned by
`parts::vault::hard::part::tests::the_pre_roll_is_the_sources_tick_counts`,
`the_first_half_mode_sequence_is_the_sources` and `the_full_mode_sequence_is_the_sources`.

## A48 — HARD: the mode-5 boundary's two displays are inserted as static ticks (registered divergence; measured difference 0 on every compared frame)

**Part:** HARD (`HARD/B.C:242-293`, `HARD/BP.C:124-167`), Phase 8 Tasks 6–7.

**Original.** Mode 4 is 102 loop iterations (`count` 0..101, `count>100` → `NEXTMODE`), so
with mode 4's count 0 at reference 851 the source's part-2 count 0 is reference 953. The
artifact's is **955**. One contiguous Task 7 capture (`out/task7/hard-950-960`) shows 950–955
byte-identical — 40,860 band pixels at `(0,0,81)` and 100 at `(255,255,255)`, the mode-4
trail's index-189 last pixels — and 956/957 replacing exactly those 100 with `(4,255,255)`.
The first *visibly* changed frame is three after the source's 953 (which is what the Task 6
record counted); the count-0 frame itself is two after, and it is pixel-identical to the
mode-4 hold because `part2init`'s `pal[1] = (63,63,63)` is white like index 189. The source's
count 1 (the first visible grid frame) would be 954; the artifact's 956 is two displays after
it.

**The palette lead is closed.** 956/957's `(4,255,255)` is 6-bit `(1,63,63)` and it is
**entry 65**, not entry 1: at `zp = 3500` the first grid frame draws only the lattice's
`x = -500` face, every point of it projects to `depthcol[3000] = 65`, and `part2init`'s
gradient entry 65 (a=1, d=56) is `(1,63,63)`. 958 is count 3 — the first frame after the
state advances — with 71 pixels at `(4,255,255)` and 22 at `(8,255,255)` = entry 66. The
artifact's mode-5 palette is the vendored `BP.C:135-154` literally; nothing is substituted.
`the_first_grid_frame_is_the_shipped_grid1_list` pins entry 65, `depthcol[3000]` and the
GRID1 correspondence.

**Port.** The fixed tick cannot express a retrace overrun inside mode 4, so the port inserts
**two display frames with no loop iteration** between mode 4's advanced tick (port 596,
reference 952) and part 2's count 0 (port 599, reference 955); part 2's count 1 is port 600
(reference 956, the first grid frame). The inserted pictures are the static mode-4 band,
which is what the artifact shows at 953/954.

**Why.** The two extra displays are the shape of a missed-retrace overrun (D22), but not at
mode 4's count 0: port 495/496 (references 851/852) match at 0 pixels, so the heavy rebuild
did not overrun. The candidate remains the 768-byte palette upload at the top of a later
mode-4 iteration (`B.C:208`); the capture measures the total cost (two displays), not its
position. No keyframe depends on the position: 1050 (port 694) and 1600 (port 1244) are
exact.

**Measured difference:** **0 pixels** at references 950–960 and 2245–2257, offset 0 (the
port's numbering is reference − 356). Reproduce with

```bash
tools/vault_capture.sh hard --frames "$(seq -s, 950 960)"  --out out/task7/hard-950-960
tools/vault_capture.sh hard --frames "$(seq -s, 2245 2257)" --out out/task7/hard-2245-2257
target/debug/secondreality --part hard --headless \
    --dump-frames "$(seq -s, 594 604)" --dump-dir out/task7/port-boundary
target/debug/secondreality --part hard --headless \
    --dump-frames "$(seq -s, 1889 1901)" --dump-dir out/task7/port-end
python3 - <<'PY'
for capture, port, refs in [
    ("out/task7/hard-950-960", "out/task7/port-boundary", range(950, 961)),
    ("out/task7/hard-2245-2257", "out/task7/port-end", range(2245, 2258)),
]:
    for reference in refs:
        a = open(f"{capture}/frame_{reference:06}.rgb", "rb").read()
        b = open(f"{port}/hard_{reference - 356:06}.rgb", "rb").read()
        print(reference, sum(a[i:i + 3] != b[i:i + 3] for i in range(0, len(a), 3)))
PY
```

(every line prints 0). The keyframes themselves are exact (A46).

## A49 — HARD: the optional argument, its debug strip and the border are not ported (substitution; measured difference 0)

**Part:** HARD (`HARD/B.C:187-201`), Phase 8 Task 6.

**Original.** `B.EXE 1|2|3` selects `dots4`, jumps to part 2 or jumps to part 3, writes a
256-byte debug strip into the top two rows and sets `border = 1`; the code immediately after
overwrites the strip with 255 and sets `border = 0`, so a normal run's border is black and the
strip is the letterbox's own row 0/1. The border attribute write at `:166-167` is a one-off
black overscan.

**Port.** The vault runs take no arguments; the strip write and its overwrite are modelled
(the rows are already 255) and the border is the view's black `border = Some(0)`. Nothing
waits on or reads an argument.

**Why.** The capture's `RUN.BAT` runs `B.EXE` with no argument, and the argument's visible
effect is undone at `:200-201`; porting the phase-skip paths would add state no filed frame
uses. A run with an argument is a developer shortcut, not the effect.

**Measured difference:** **0**. The letterbox row 0/1 are 255 in the capture's 474–955 frames
and in the port; reproduce as in A47.

## A50 — HARD: two one-byte data questions are recorded, not fitted (registered data divergence; measured difference 0 in the compared frames)

**Part:** HARD's build-time data (`HARD/BDO.C`, `HARD/_GRID1.TMP`, `HARD/B.EXE`), Phase 8
Task 6.

**Original.** The four BDO streams are linked into `B.EXE` (`LINK.TMP:1-3`). Bands 0 and 3
match the extraction byte for byte at 55,875 and 64,977; bands 1 and 2 differ in their **last
terminator byte** only (56 and 72 where the extraction has 0), because the next linked
object's bytes abut. The 400-byte `_GRID1.TMP` matches `B.EXE`'s table at 69,265 for 399 of
400 bytes; the last byte is `0x4F` in the binary and `0x00` in the file, so the binary's last
word is `0x4F5A` rather than `0x005A`.

**Port.** The port extracts the streams from the curated `assets/HARD/FCRZ.U` with `BDO.C`'s
rule (terminator 0) and compiles `GRID1` from `_GRID1.TMP`. The terminator bytes are never
read as records (the spawner resets on a zero value), and the one-byte grid difference is the
y of the 100th pair. `the_dot_streams_are_the_bdo_extraction` and
`the_grid_table_is_the_shipped_grid1` pin both differences against the binaries.

**Why.** Neither byte changes a drawn pixel: the terminator is the stream's end marker in both
builds, and the grid word is only read when `doit2` reaches the 100th pair with its x from the
same pair. The capture's keyframes are exact with the file's values, which is the measurement.

**Measured difference:** **0** in the nine compared keyframes (and none of them exercises the
100th pair's y). Reproduce the data checks with
`cargo test -p secondreality --bin secondreality vault::hard::dots`.

## A51 — HARD: `do3dots`' out-of-array guard is a model for an unreachable branch (registered model; measured difference 0)

**Part:** HARD (`HARD/BA.ASM:221-222,262-270`), Phase 8 Task 6; part 2 is Task 7's.

**Original.** `_depthcol db 8192 dup(0)` sits in the code segment with 0..3,099 written
(`BP.C:128-133`). `do3dots` loads z's low word into `bp`, signed-skips values above 3,000
(`cmp bp,3000; jg`, `BA.ASM:233-235`), and reads `cs:_depthcol[bp]` with a **16-bit**
effective address. A negative low word passes the signed test; for `bp = 0xFF00` the 16-bit
address wraps to `_depthcol - 0x100` and reads a code byte (0x07 with the shipped link map),
not BSS zero. The branch is unreachable in part 2: over the 501 states the projected z′ is
full **1139..3964**, every drawn point's low word is 1139..3000 (the signed `jg` skips the
rest), and the low word is always positive.

**Port.** The guard returns colour 0 for a `bp` whose unsigned index is past the 8,192-byte
table. That is a **model of the unreachable branch**, not the source's address arithmetic;
the port has no load map for the code bytes before `_depthcol`. The `count <= 0`,
record-length, shadow and offset bounds checks are likewise guards for inputs part 2 never
produces (the projected x is bounded; the source's `rows[y] + x` is a 16-bit offset that
wraps within the A2D0 segment). The source's second clear is two **word** stores at
`[si+12]`/`[si+12]+320` (`BA.ASM:229-231`); the code that writes `[si+12]` is unreachable
behind `jmp @@4` (`:235`), so it stays 0 and the port's one-byte-per-row clear (0 and 320) is
a documented model of the unreachable branch, differing only in bytes 1/321 at offset 0.

**Why.** Reproducing the wrap would need the link map's code byte, and nothing in the effect's
reachable domain reads it. The rule's test pins the signed skip, the 16-bit load and the guard
as a model; it does not assert BSS zero as fact. The range is measured, not bounded:
`the_part2_grid_z_range_and_drawn_words_are_the_measured_domain` builds the actual 488-point
grid and walks part 2's 501 states, pinning the full 1139..3964 range, every z-passing low
word in 1139..3000, and the first frame's 100 points at `depthcol[3000]`. (A51 previously
carried 1134..4366, the loose `zp ± 500·√3` envelope; the recurrence never reaches either
end — `zp` is 3500 only while the angles are still zero.)

**Measured difference:** **0** in the reachable domain (the guard never fires for the filed
frames); the unreachable byte is not reproduced. Reproduce the rule with
`cargo test -p secondreality --bin secondreality
the_shadow_grid_dot_skips_positive_z_and_models_the_negative_word`, and the domain with
`cargo test -p secondreality --bin secondreality
the_part2_grid_z_range_and_drawn_words_are_the_measured_domain`.

## A52 — The vault's silent run: no player, no music service, and one modelled start key (substitution; measured difference 0)

**Parts:** GRID, HARD and TWIST under `--part` and `--vault`, Phase 8.

**Original.** The three effects are linked with the DIS client and run behind the vendored
standalone `DIS/DIS.EXE`, which is built without `INDEMO`: its music service returns 0 until
`checkkeys` consumes a key and then echoes the requested code once (`DIS/DISINT.ASM:242-282`,
`:154`), there is no player, and the copper service is a bare `ret` (`:294-316`). The
reference captures are therefore silent and deliver exactly one `KBD_space` to GRID and HARD
and none to TWIST (`tools/vault_capture.sh`; `reference-indices.md` §24.4).

**Port.** No vault part installs a player: `music_for_part` returns `None`, the registry rows
carry no `boot_anchor`, `ctx.music()` is `None`, and a silent run refuses `--audio-dump`. The
single start key is modelled per part at `init` (GRID's `getch()`, A40; HARD's `muscode(1)`
gate, A46; TWIST takes no key, A43), and the later key cuts are unreachable (GRID's
`muscode(3)`, HARD's `0xff`). A `--part` run numbers its frames from zero; the reference's
boot-relative indices are only the captures'.

**Why.** The artifact is silent; installing the demo's player would invent sound the
reference does not make. The one key is a start event, not a running input: nothing in the
port waits on a key, and every compared frame is after the gate.

**Measured difference:** **0** in all twenty compared frames (GRID 5 + TWIST 6 + HARD 9), and
the silent manifests have `music: null`, `anchor: null` and `index_origin: "part"`. Reproduce
with `cargo test -p secondreality --test vault -- --nocapture`; the manifest checks are
`a_part_run_of_grid_numbers_itself_from_zero_and_is_silent`,
`a_part_run_of_twist_is_silent_and_numbers_itself_from_zero` and
`a_part_run_of_hard_is_silent_and_numbers_itself_from_zero`.

## A53 — The full run's seams: every part is handed over in one tick, and the loader's frames are not modelled (registered; D31's question decided)

**Part:** the shell, between every two parts of a run (`--block 1` measured in full), Phase 9
Task 6.

**Original.** Each part is its own EXE. Between two of them the shell's `partexecute` loads the
next (`MAIN/U2.ASM:789-912`), and at two places it also reloads the music (`restartmus`,
`:813-818` before GLENZ and `:880-885` before U2E); the screen meanwhile shows whatever video
memory and the DAC hold — the previous part's last picture — or, at U2E, JPLOGO's logo under a white
DAC from ≤ 34100 to 34114 (`reference-indices.md` §26.3). How many frames a load takes is not a
constant of the source: it is a disk and memory cost per EXE.

**Port.** D31 §3: the next part's first tick is the tick after the last one's, the surface and
player continuous; the two restarts install their new player at that boundary (D34).

**Measured difference — the seam table** (`reference-indices.md` §26.8, from
`cargo test --release -p secondreality --test fullrun -- --nocapture`, which asserts both port
columns of `xtask/src/rediff/table.rs`'s `FULLRUN_SEAMS` — the one source — against the run): the port's first frame minus the original's, by §26.3's own definition of a
first frame, for the nineteen parts —

```
part        original  port   difference  change
alku            404    404          +0
u2a            5961   6008         +47     +47
pam            7707   7704          -3     -50
beglogo        7922   7885         -37     -34
glenz          8522   8770        +248    +285
tunneli       11158  11394        +236     -12
techno        12214  12458        +244      +8
panicend      16193  16423        +230     -14
mntscrl       16637  16875        +238      +8
lnszoom       18656  18913        +257     +19
plzpart       21927  22171        +244     -13
minvball      26673  26912        +239      -5
rayscrl       29174  29422        +248      +9
3dsinfld      31272  31531        +259     +11
jplogo        33263  33498        +235     -24
u2e           34120  34318        +198     -37
endlogo       37967  38199        +232     +34
cred          38583  38807        +224      -8
endscrl       46277  46432        +155     -69
cue exit      16630  16874        +244
```

(JPLOGO and U2E are aligned on bytes — JPLOGO's opening black and U2E's band — because §26.3 can
only bracket their first frames.) From GLENZ to ENDSCRL every difference carries A38's lag (the
`MUSIC1@0` session is 247–248 frames late, the `MUSIC0@18` one 235); the parts sit within −18…+11
of it until JPLOGO; ENDSCRL's −69 change is CRED's span (first frame to ENDSCRL's) being 69 frames
shorter in the port, 7,625 against 7,694 — why is not measured; A10's record of CRED running early is
a lead. The parts' own residuals at the seams are: **U2A's first frame is 47 frames late, so ALKU's
end is at least 47 frames late** (the original's last ALKU frame may be its loader holding the
screen; no keyframe of `alku` sees it; the cause is not established, and the leads are the batches' `dis_sync()<4+tptr`
waits and the scroll's `frame_count<SCRLF`, `ALKU/MAIN.C:138`, `:400`); **BEGLOGO's −37 is A5's
lead-in** (the port's white field is up from its first tick; the ramp is frame-locked, the port's
7930 being the original's byte for byte); **U2E's band comes 37 frames earlier than its restart's
lag** (+198 against +235): the port starts U2E at the restart's boundary, where the original's
reload holds JPLOGO's whitened screen; and the cue wait's exit is followed by MNTSCRL's first frame
after **7** frames in the original (16630 → 16637) and **1** in the port (16874 → 16875).

**Why the loader is not modelled (D31's question, D24's rule).** One `LOADER_TICKS` held between
parts would have to explain every seam the music does not decide. It does not: the changes across
the seams take both signs — the port is already *later* at ALKU→U2A (+47), MNTSCRL→LNS&ZOOM (+19)
and RAYSCRL→3DSINFLD (+11), and a delay inserted at a handover can only make it later still — and
the two seams the music pins on both sides need different constants (6 frames after the cue wait;
59 to 68 under the U2E reload — its clock at 34,054 ± 2, `reference-indices.md` §26.6, against
U2E's first frame in (34114, 34120] — where the port's is 0). Fitting a constant per seam is forbidden (D24), so the handover is
left as D31 decided it, and this entry is its record. **A later task should watch** ALKU's ≥ 47
frames, which are a part's, not a seam's.

**The filed frames at those differences.** Each `reference/parts/fullrun` frame is rendered at its
reference index plus its part's difference above — never chosen for the frame's own count — and
the three frames the original shows during a loader stretch the port does not have (7920, 16636,
34110) at the port's last frame of that screen (7884, 16874, 34285). `cargo xtask rediff --only
fullrun` compares all 60 (44 frames, 16 index planes) at their registered counts
(`xtask/src/rediff/table.rs`'s `fullrun/*` entries; the full list is `reference-indices.md` §26.8).
Every held frame is 0 in RGB but A25's 19000 (2,058), A10's 40600 (7,204) and 34110 (72,363: the
reload's white, which the port never shows; its index plane is 0). The moving frames this entry
registers are 5961 (63,788: U2A's opening, 47 frames late), 7930 (81,273, at BEGLOGO's −37),
11159 (192), 11900 (4,336), 16193 (108,923: PANICEND's squashed troll), 18656 (129), 24500
(45,808), 29174 (875), 31272 (4), 31901 (33,554; idx 34,036 — 3DSINFLD draws every second frame),
46280/46283/46286 (42/67/149); and three index planes under a 0 RGB, memory no screen shows: 7880
(48,786, PAM's picture under its white), 24540 (48,384, the cube screen under black) and 37970
(46,997, U2E's leftover video memory under ENDLOGO's white).

## A54 — Block 1's two long traces: `second1e`'s placement drift and `second1f`'s speed-1 row (registered)

**Part:** the music, block 1 after GLENZ (`reference/music/second1e`, 8,800…33,949, MUSIC1) and
after U2E (`second1f`, 34,105…48,276, MUSIC0 from order 18), Phase 9 Task 2's traces.

**Original.** The emulated machine's tick clock and frame counter (A12), sampled once a frame over
windows four and two and a half times `second1`'s; and in `second1f`'s order 26 (pattern 71,
frames 38,328…38,370) the module runs at **speed 1**, a row every 1.46 frames, so the trace's one
sample a frame publishes row 25 at 38,364 and row 27 at 38,366 and never row 26.

**Port.** The exact tick clock (A12); the port's phase in `second1f` is about ten frames later, so it
publishes row 26 at 38,376 — a frame on which that row *is* current, and which the trace's sampling
never caught.

**Why this is registered rather than fixed.** Neither is a seam: Task 6's seam work explains A38's
lag (the offset these windows are compared at), not the drift inside them. The drift is A12's
inter-session fact over a longer window — the port sits on one clock, the sessions on their own —
and the unsampled row is a sampling fact: a row shorter than one sample cannot be published by a
trace that samples once a frame. A player that skipped row 26 would be wrong.

**Measured difference.** `cargo test -p sr-audio --test trace -- --nocapture the_port_reproduces`
holds both traces to `KNOWN_DISAGREEMENTS`, exactly: `second1e` — content identical in every word,
**worst displacement 8** (`JITTER_FRAMES` 7; the port early by −1 at 10,827, −4 at 19,981 and −8
from 33,264); `second1f` — one content difference (`row`, index 502, trace 27, port 26, every other
value identical), **one frame, 38,376**, whose value the trace never publishes, **worst
displacement 12**. The port's whole block-1 run shows the same against them at their offsets
(`cargo test --release -p secondreality --test fullrun -- --nocapture`): `second1e`
17,439 of 25,143 frames within 3 (69.359 %), worst 8; `second1f` 6,187 of 13,934 (44.402 %), worst
12, and the one frame past the search.
