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
position matches the port's, and the worst displacement. Phase 4's four, Phase 5's three and
Phase 6's three are included; the nine window runs are phase-aligned to their own session
first (each prints its offset):

```
14 filed trace(s)
  hiddenu (MUSIC0): 5685 of 6084 frames agree (93.442 %); worst displacement 3 frame(s)
  hiddenu2: the session's phase is -16 frame(s) from the boot's
  hiddenu2 (MUSIC0): 1620 of 1692 frames agree (95.745 %); worst displacement 1 frame(s)
  second1 (MUSIC0): 3738 of 5538 frames agree (67.497 %); worst displacement 7 frame(s)
  second1b: the session's phase is +1 frame(s) from the boot's
  second1b (MUSIC0): 344 of 352 frames agree (97.727 %); worst displacement 1 frame(s)
  second1c: the session's phase is +1 frame(s) from the boot's
  second1c (MUSIC0): 1609 of 1692 frames agree (95.095 %); worst displacement 1 frame(s)
  second2: the session's phase is -5 frame(s) from the boot's
  second2 (MUSIC1): 3207 of 4590 frames agree (69.869 %); worst displacement 4 frame(s)
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
own phase offsets (−16 for `hiddenu2`, −5 for `second2`, +1 for `second1b`, `second3b`,
`second1c`, `second4` and `second3e`, +2 for `second3a`, +4 for `second3d`) are the boot's
session-to-session spread, measured in
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

**The release does not.** Fourteen diagnostic captures (`out/plz-diag-cube`
and `out/plz-diag-cube2`, the documented `SECOND 3` recipe at cycles 30000;
the filed 8500/9500 are reproduced byte-identically in the same session) show:

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
  the cube 34 px high). The x pair is unchanged (256; an x-scale sweep's
  minimum).

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
(640×200; a pixel differs iff any channel differs): 17200 matches at
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
