# Approximations register

Every place the port knowingly differs from the original. An entry requires a
justification and a measured visual difference; it is not a parking place for "close
enough".

## How an entry reads (Phase 9 Task 10)

Every entry carries five labels — **Part**, **Original**, **Port**, **Why** and **Measured
difference** — and an entry that names an open item carries a **Status (Phase 9)** line: *Closed*,
with the measurement and the command or test that produces it, or *Carried*, with the reason it
cannot be closed (the packed EXE, D12; what the oracle cannot see; an unreachable path). Where an
entry's earlier claim is no longer right it is restated at the measured figure and the old text is
kept as dated history, struck through or headed as such (D29). `cargo test -p xtask --test
approximations` checks every `## A` heading for the five labels, every entry that names an open
item for its status line, and every `register` string of `cargo xtask rediff`'s table for an entry
that exists.

**The packed EXEs (D24's lead).** Where an entry's lead is the shipped part, it is two things: the
part's EXE as `MAIN/DATA/` stages it, PKLITE-packed, and its line in `MAIN/PACKFINA.INC`, the list
the release's packer read. A line's fifth and sixth words are a length, low word then high
(`dw 01FBFh,0BA4Ah,00012h,012DFh,0A1BCh,00002h ;PAM.EXE` is 0x2A1BC = 172,476), and it is the
staged file's size for 18 of the list's 22 EXEs. The four it is not are `LNS&ZOOM.EXE` (staged
67,222, listed 67,226), `U2A.EXE` (111,894 against 54,432), `U2E.EXE` (89,102 against 46,278) and
`START.EXE` (11,458 against 11,329), and the staged `U2A.EXE` and `U2E.EXE` are the only two with
no PKLITE header: those two copies are not the images the release packs, and LNS&ZOOM's is four
bytes from it. Measured by

```bash
cd upstream/SecondReality && tr -d '\r' < MAIN/PACKFINA.INC | grep -n -i '\.EXE' |
while IFS= read -r l; do n=${l%%:*}; name=${l##*;}; w=$(echo "$l" | sed 's/.*dw //; s/ ;.*//')
  len=$(( 16#$(echo $w | cut -d, -f6 | tr -d h) * 65536 + 16#$(echo $w | cut -d, -f5 | tr -d h) ))
  echo "$n $name listed=$len staged=$(stat -c %s "MAIN/DATA/$name") pklite=$(grep -a -c -i pklite "MAIN/DATA/$name")"
done
```

## A1 — Overscan border is padded, not scanned out

**Part:** all; used deliberately by PANIC and the 320×400 parts.
**Original:** the border is the region of the display outside the active area, set by
AC 0x11 and scanned out by the CRTC.
**Port:** `View::border` is a colour; `present()` draws it as a frame of 8 source pixels
per side, scaled with the picture.
**Why:** the engine composes the active display only (spec §4.2, plan decision 8). The
border must still appear, but its exact thickness depends on CRTC timings the port does
not model.
**Measured difference:** **not measurable against the reference, and derived instead (Phase 9
Task 10).** The oracle renders no overscan region: its blank line on a VGA is the black index, and
only a CGA's is the overscan colour (`dosbox-automation/src/hardware/video/vga_draw.cpp:1160-1215`,
the CGA case at `:1182-1186`), so the filed captures are the display-enable area and hold no border.
What the border *is* follows from the CRTC timing the parts' modes program, and that is the same
everywhere: no part on the shipped path writes a timing register. Their CRTC writes are 0x09 (the
rows: `inittwk`'s `0x0009`, `BEG/ASM.ASM:21-24`, `END/ASM.ASM:21-24`, LENS's `or al,3`,
`LENS/ASM.ASM:223-231`), 0x0C/0x0D (start address), 0x13, 0x14, 0x17, 0x18 (line compare) and 0x07,
which PLZPART's `set_plzstart` writes as `0x0F | LC8`, keeping mode 13h's timing bits
(`PLZPART/ASMYT.ASM:103-112`); GLENZ's 240- and 350-line tables (`GLENZ/VIDINIT.ASM:10-25`,
`sethseq`) belong to `init320x240`/`init320x350unr`, and the shipped `init320x200` has its tweak
commented out (`GLENZ/VID.ASM:213`). `MAIN/VMODE.ASM` programs nothing: its save/load of modes
0Dh/13h/0Eh is assembled out (`IF 0`, `:131-152`) and its dispatch commented (`:22-31`). So the
timing is the BIOS's, which in the oracle is its own table: mode 13h and mode 0Eh (ENDSCRL,
`ENDSCRL/MAIN.C:22-26`) are both `htotal 100, hdispend 80, vtotal 449, vdispend 400`
(`src/ints/int10_modes.cpp:40`, `:45`), programmed at `:1352-1460` as CR00 95, CR01 79, CR02 80,
CR03 0x82 with CR05 bit 7 (blank end 98 & 0x3F = 34), CR06 0xBF, CR12 0x8F and CR15 0x96 with their
overflow bits (447, 399, 406), and CR16 = 441 & 0xFF = 0xB9. The oracle reads those back
(`vga_draw.cpp:2001-2095`) as a line of 100 characters, 80 displayed, blanked from 80 to
80 + ((34 − 80) & 0x3F) = 98, and a frame of 449 lines, 400 displayed, blanked from 406 + 1 = 407 to
407 + ((0x39 − 407) & 0x7F) + 1 = 442. The border is what is displayed outside the active area and
not blanked:

| side | characters / lines | 320×200 (2 lines a row) | 320×400 | 640×400 (8 pixels a character) |
|---|---|---|---|---|
| left | 98–99: 2 characters | **8** | **8** | **16** |
| right | none (blank starts at display end) | **0** | **0** | **0** |
| top | 442–448: 7 lines | **3.5** | **7** | **7** |
| bottom | 400–406: 7 lines | **3.5** | **7** | **7** |

in source pixels (a character is four pixels in the 256-colour modes, eight in ENDSCRL's
sixteen-colour one). **They are not whole at every geometry** — 320×200, most of the demo, has
3.5 rows top and bottom — so `present.rs`'s `BORDER_PX` stays **8** on every side: the phase adopts a derived border only if
it is whole source pixels at every geometry (`docs/plans/09-assembly.md`, Task 10 Step 3). Two further facts say a derived pad would not be a measurement either: the
registers read as the IBM convention documents them (blank from 406, ending at 441) give 3 and 4
rows at 320×200 where the oracle's reading gives 3.5 and 3.5, and nothing in the tree or the oracle
arbitrates the one line; and the horizontal border the registers give is one-sided, which a monitor
and the 256-colour pipeline's own delay shift and nothing here models. The pad's thickness is
therefore the port's presentation choice, recorded as such; no compared frame contains it (`xtask
compare` reads the composed frame, `present.rs`'s module docs), so its difference from the
original is **0 on every compared frame**. The derivation's arithmetic is the test
`a1_the_derived_border_is_not_whole_at_every_geometry` (`cargo test -p xtask --test
approximations`); its inputs are the source lines cited here.

**Status (Phase 9):** Closed — by derivation, not adopted: the border the modes' CRTC timing gives
is 8/0/3.5/3.5 source pixels (left, right, top, bottom) at 320×200, not whole, so the pad stays 8
(`cargo test -p xtask --test approximations a1_the_derived_border_is_not_whole_at_every_geometry`).
What cannot be measured is the oracle's: it renders no border.

*History (Phase 0–8).* The entry read: "8 source pixels per side at every geometry. The filed
reference captures are the displayed area, with no overscan border in them to measure, so the
padding's exact thickness stays unmeasured; the Phase 9 fidelity pass owns closing it against the
original."

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

**Status (Phase 9):** Carried — unreachable paths: no file the demo ships fires the three
malformed-input constructs or carries a `0x0004`/`0x0007` chunk (the histogram above).

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

**Measured difference (restated, Phase 9 Task 10).** Phase 9 Task 1 captured the window
(`reference-indices.md` §25.6), and the "zero" below no longer covers it: it holds for the RGB and
not for the index plane. **RGB: 0.** `beglogo-leadin/even` and `/odd` (430–450) are white through
446 on both sides, and the ramp's first displayed step is 447, every frame at 0 differing pixels.
**Index plane: the field, where the port already has the picture.** The original's plane is the
white field — every byte index 15 — at every index the oracle could read, **430–438**, while the
port's holds the picture from its blit at 436: `beglogo-leadin/plane-436` differs in **127,998 of
128,000** indices (the port's other 2 are picture pixels of index 15). Both counts are
`cargo xtask rediff --only beglogo-leadin`'s. From **439** the oracle cannot read the plane (§25.6:
the chain-4 probe answers mid-`lineblit`, or the DAC pass reaches the ramp), so where the
original's code blits is bounded below by 439 and not measured. The ramp's 447 is one frame after
this entry's 446, which is D17's session alignment (446 was measured from `c` at 470 and 560), not
a change in the port.

**Status (Phase 9):** Closed for 430–438 — the plane is the white field (index 15) and the port's
is the picture from 436, 127,998 of 128,000 (`cargo xtask rediff --only beglogo-leadin`);
Carried for 439–446 — the oracle cannot read the index plane there (§25.6), so the nine
unaccounted frames keep no cause.

**Measured difference as Phase 2 registered it (history).** **Zero on every frame the reference
holds.** 430 is the white field in both
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

**Original — the ramp.** `TUN10.PAS:122` writes `(64-x) *3 div 4` into DAC entries 128..192
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

**Measured difference (restated at the full run's and `rediff`'s numbers, Phase 9 Task 10).** The
part's own run is unchanged: **179 / 373 / 321 / 433** differing pixels of 64,000 at 3800 / 4200 /
4500 / 4600, at the same ticks (`cargo xtask rediff --only tunneli`, every frame at its count).
In the full `SECOND 1` run, where TUNNELI starts 7 frames early (A53), the two filed TUNNELI frames
differ by **192** (11159, its first) and **4,336** (11900) pixels (`cargo xtask rediff --only
fullrun/04-glenz-techno`); the second is this residual seen through the run's own phase, not a
new difference, and nothing in it is colour the ramp could change.

**Status (Phase 9):** Carried — the residual is unattributed: no session reproduces the 4500 and
4600 pictures (`reference-indices.md` §1.8), so the oracle cannot say whose it is, and the shipped
`MAIN/DATA/TUNNELI.EXE` is packed (D12).

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

**Original — the gate.** `END.C:107-111` is

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

**Port.** Since Task 8 the gate is the source's own loop, run against the run's player (the
retirement above, `docs/fidelity/endlogo.md` §5), with the one-frame `GATE_ALIGNMENT_TICKS`; the
lead-in draws the white field and blits at tick 36 (index 440), with no model of the ten ticks
below.

**Why.** The gate needs no substitute once the music plays; the lead-in's ten ticks are
not modelled for A5's reason — the port decodes the picture once, in `init`, and moving the blit
to where the original's "must have" been would be a fit against no evidence.

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

**Measured difference (restated, Phase 9 Task 10).** As A5's: Phase 9 Task 1 captured the window
(`reference-indices.md` §25.6). **RGB: 0** — `endlogo-leadin/even` and `/odd` (436–455) are white
through 451 on both sides and the ramp's first step is 452, every frame at 0. **Index plane: the
field where the port has the picture.** The original's plane is index **0** — `END.C` never fills
video memory, it whitens DAC entries 0–254 (`:62-63`, `:90-91`) — at every index the oracle could
read, **436–443**, while the port's holds the picture from its blit at 440:
`endlogo-leadin/plane-440` differs in **55,499 of 128,000** indices (the picture's index-0 pixels
are the rest). Both counts are `cargo xtask rediff --only endlogo-leadin`'s. From **444** the
plane cannot be read (§25.6).

**Status (Phase 9):** Closed for 436–443 — the plane is index 0 and the port's is the picture from
440, 55,499 of 128,000 (`cargo xtask rediff --only endlogo-leadin`); Carried for 444–450 — the
oracle cannot read the index plane there (§25.6). The ten frames keep no cause, and the common
candidate below (the decode-and-blit) is neither confirmed nor ruled out.

**Measured difference as Phase 2 registered it (history): zero on every frame the reference
holds.** 430 (tick 26) is inside the 32
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

**Original.** `#define FONAY 32` (`CREDITS/MAIN.C:29`) with
`extern char far font[FONAY][1500]` (`:31`): the routine reads **32 rows** of a
raster that has **40** (60,000 `db` values, the 1,500×40 raster the inventory's
§E.8 and §E.20 measure). It also states a **32-column cell** —
`fonap[32]=1500-32; fonaw[32]=8;` (`:270-271`), the routine's blank — on a strip
1,500 columns wide: `1500/32 = 46.875`, so the cell does not divide the raster.
The strip's real structure is 47 ink runs, one per character of `fonaorder`
(`:32`), which the routine *scans* (`:250-269`) rather than indexes by cell.

**Port.** The part reads the raster itself — since 2026-10-29 out of the shipped image, whose
array is this file's values byte for byte (below) — and translates `:250-269`'s scan. It does **not** load the strip through
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
agrees with the reference: on all five keyframes the caption region (rows 100-199) is
byte-identical to the reference's (`bins/secondreality/tests/cred.rs`,
`the_five_keyframes_are_the_references_byte_for_byte`, which asserts the band and the caption
separately). Until 2026-10-29 5440 and 8740 differed there for another reason — the shipped
build's caption text, A10, retired.

**Re-measured against the release's image (2026-10-29, D49).** The strip the shipped routine reads
is the far array at image `0x29f0` of the unpacked `MAIN/DATA/CRED.EXE` (`mov dx,029fh` at
`0x1e83`/`0x1ec5` in `init`, `0x1db0` in `prt`; row stride `mov dx,05dch`, 1,500): 40 rows, the
60,000 bytes up to the first picture at `0x11450`, and **the tree's `FONA.INC` byte for byte**.
`fonaorder` (the far pointer at DGROUP `0x92`) is the tree's 47 characters, and the release's scan
and `prt` read 32 rows (`cmp word [bp-4],20h` at `0x1e95`, `y+20h` at `0x1d60`). So this entry
stands as it was: the release has the same 40-row raster and the same 32-row window. The part now
reads the raster and the order out of the image rather than from `FONA.INC`
(`parts::cred::tests::the_images_font_and_order_are_the_trees_byte_for_byte`), and the measured
difference is unchanged — the same 92 pixels, 46 on displayed rows. One thing the reading
corrected: the port's own copy of `fonaorder` had `"` and `.` swapped (`…:,".()+-` where
`MAIN.C:32` and the image have `…:,."()+-`), so the two marks' runs were each other's. No caption
contains either, so no frame moved.

## A9 — ALKU: two rows of the font strip the routine reads are not in the file

**Part:** ALKU (`ALKU/MAIN.C` over `ALKU/FONA.UH`), Phase 4 — carried into the
register here because this is where a font strip is next touched: Phase 1's task
A7 found it (`docs/plans/01-assets.md`, A7's Interfaces block) and its ledger
says the entry belongs to the phase that ports the part.

**Original.** `ALKU/MAIN.C:169` with `:218` and `:225` scan **32** rows, and
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

**Status (Phase 9):** Carried — the padding bits a black palette hides are in the packed
`MAIN/DATA/ALKU.EXE` (D12; its pack line is `MAIN/PACKFINA.INC:47`), and no screen shows them; the
visible half is measured (the held cards 1719 and 2719 at 0, `cargo xtask rediff --only alku`).

## A10 — CRED: ~~three of the five keyframes are the shipped build's credits, not the tree's~~ **retired 2026-10-29 — the 21 screens are read from the shipped image**

**Retired 2026-10-29 (D49), and what replaced it.** The entry registered that nine of the 21 credit
screens were the shipped build's, whose captions and pictures the tree does not hold, with "inside
the packed EXE" as the lead. The packed EXE is now read: `sr_assets::exe::unpack` turns
`MAIN/DATA/CRED.EXE` (PKLITE 1.14 large, one layer) into its 426,384-byte load image, and the part
builds its 21 screens out of it (`bins/secondreality/src/parts/cred.rs`, `docs/fidelity/cred.md`
§9). Every frame this entry measured is now 0. What the entry recorded before, kept as the record
of what the release changed: the three keyframes' band and caption bytes (3240: 20,122 / 0;
5440: 0 / 18,264; 8740: 5,702 / 5,520), the nine screens the record's hold samples put off the
tree's data (2, 6, 10, 11, 12, 17, 19, 20, 21), and the pixel counts 7,204 / 6,088 / 3,741 at
3240 / 5440 / 8740 and 7,204 at the full run's 40600.

**Part:** CRED (`CREDITS/MAIN.C` over the pictures `main` links), Phase 2; retired under D49.

**Original.** The release's `main` (image `0x190c-0x1b68`) is 21 `if(!dis_exit()) screenin(pic,
text)` calls at image `0x193a`, `0x1955`, …, `0x1b56`, 27 bytes apart, each
`push ds; push text; push pic_seg; push 0; lcall 0190:0269` (`1e 68 TT TT 68 SS SS 68 00 00 9a 69
02 90 01`). The picture is the `FC FC` blob at image `SS SS × 16` — segments `0x1145` to `0x6339`,
`0x419` paragraphs (16,784 bytes) apart, each a 16-byte header (160×100, 256 colours, pixels at
paragraph 49), a 768-byte palette at 16 and the pixels at 784, the tree's own blob layout. The
caption is the NUL-separated lines at DGROUP (`0x67520`, `mov dx,6752h` at image 0) + `TT TT`,
ending at a double NUL. Against the tree's `CREDITS/MAIN.C:41-156` and `CREDITS/PICS/`, **nine
screens differ** — the nine this entry found in the record — and twelve are the tree's
(`parts::cred::tests::the_release_changes_nine_of_the_trees_screens`):

| screen | picture pixels that differ (of 16,000) | shown palette entries that differ | caption |
|---|---|---|---|
| 2 | 15,996 | 223 | "+ OBJECTS - WILDFIRE" |
| 6 | 15,997 | 44 | the tree's |
| 10 | 15,452 | 231 | the tree's |
| 11 | 0 | 0 | "SKETCH - SKAVEN / GRAPHICS - PIXEL / …" |
| 12 | 0 | 0 | the same as 11 |
| 17 | 15,992 | 156 | the tree's |
| 19 | 15,998 | 234 | "MUSIC - SKAVEN / CODE - PSI / WORLD - TRUG" |
| 20 | 15,990 | 239 | "GRAPHICS - PIXEL / MUSIC - SKAVEN" |
| 21 | 15,882 | 32 | "GRAPHICS - PIXEL / MUSIC - SKAVEN / CODE - WILDFIRE" |

A *shown* entry is one the screen indexes: the caption's ramp (0-9) or `v + 16` for a picture value
`v`. On seven of the twelve unchanged screens the blob's palette differs from the LBM's in entries no
pixel of theirs indexes (1: 129, 3: 83, 4: 74, 7: 128, 8: 58, 13: 27, 15: 81), which no frame shows.

**Port.** The release's: the 21 calls are found by scanning `main` for that pattern (refusing an
image with another count, or whose picture word is not a relocation), and each screen's picture,
palette and caption are read at the offsets its own call names
(`the_screens_are_mains_twenty_one_calls_read_from_the_image` pins all 21). The tick map, the
palette repair and the geometry are unchanged — the image's `init` and `screenin` are the tree's
code in those respects (`cred.md` §9). The tree's 21 `PIC*.LBM` files are no longer read by the
part; the tests read them to measure the release against, and they stay in `assets/`.

**Why.** D49: the shipped captions and pictures are data inside the release's own artifact, so the
port reads them from it rather than registering their absence.

**Measured difference: 0.** `cargo xtask rediff --only cred`: 0 / 0 / 0 / 0 / 0 at 1440 / 3240 /
5440 / 7640 / 8740 (were 0 / 7,204 / 6,088 / 0 / 3,741); `cargo xtask rediff --only
fullrun/12-cred`: 0 at 40600 (was 7,204). Against the record's sweep (`out/b5-s5-cred-map`,
scratch), all 49 hold samples are the port's hold of their own screen, 0 of 192,000 bytes each, and
all 28 wipe frames are one state of one screen byte for byte
(`the_records_hold_samples_are_the_releases_screens`,
`the_records_transition_frames_are_the_states_this_part_writes`, both `#[ignore]`d on the scratch
capture).

**Status (Phase 9):** Closed — the screens are the release's, read from its image (D49).

## A11 — ENDSCRL: the shipped build is read from its image; the block-5 chain offset remains

**Part:** ENDSCRL, Phase 2; rewritten 2026-10-29 under D49. A6's and A10's class — this tree is not
the released build — until the release's image became readable.

**Original.** The shipped part is `MAIN/DATA/ENDSCRL.EXE` (9,965 bytes, PKLITE over LZEXE 0.91,
`MAIN/PACKFINA.INC:50`). Unpacked (`sr_assets::exe::unpack`), its code differs from the tree's
`ENDSCRL/MAIN.C` in six ways, every one read off the image (offsets into the unpacked image,
`docs/fidelity/endscrl.md` §1): a control line (`[` first) draws nothing (`0x13c3`) and lasts
`(t[1]-'0')*10 + t[2]-'0'` rows (`0x1662`); every other line lasts **25** rows (`0x165a`), not
`FONAY` 30; the raster is `font[30][1550]` at image `0x112e0` (`0x140a`, `0x1423`), scanned over
25 rows; `fonaorder` is **80** characters (DGROUP `0xb2`), the tree's 74 and `Z ä \x94 Y / &`;
the blank is at 1530 (`0x17a8`); and a `%` line calls `exit(0)` (`0x169a-0x16a3`). Both builds open
the text without `O_BINARY`, so Borland's text-mode `read` (`0xc62`, `_fmode` = `0x4000`) strips its
carriage returns.

**Port.** **The release's build.** The part reads the raster and `fonaorder` out of the unpacked
image, and its `init` and `do_scroll` are that image's (`bins/secondreality/src/parts/endscrl.rs`,
each constant citing its instruction). What this entry registered before and has moved from
"registered" to "read from the image":

1. the `[NN` control lines — drawn as text before ("15" under "Canada"), now blank and NN rows
   long, with `[8`'s 42 rows the original's own arithmetic (D7);
2. the 25-row pitch — 30 before, which drifted 5 rows a line further from the release's;
3. the 80-character order — `/`, `&`, `Z`, `Y` and `ä` were two-column gaps before;
4. the raster — the tree's `FONA.UH` before, 53 pixels off in the header band alone;
5. the carriage returns — the port kept them before, a deviation from **both** builds;

and one the entry did not know: the part now **ends by itself** at the `%` line, part tick 22,967
(`the_part_ends_by_itself_on_the_percent_line`), and the shell's end follows as after ESC.

**Why — what is left is not this part's.** Rendered from the release's own start, 8917, all four
keyframes of `reference/parts/endscrl` differ in **0** pixels of 256,000
(`the_port_draws_the_references_frames_at_the_references_own_ticks`). The port's chain starts the
part at **8835**, 82 frames (41 ring lines) earlier, because the port's CRED in block 5 ends 82
frames before the release's — inside CRED's own measured 6-to-105-frame bracket (`cred.md` §7) and
the same cause as A53's CRED→ENDSCRL change (−69 in the full run, where the offset is 88). It is a
seam, not this part's drawing; moving this part's start to fit it would be the per-seam constant
D24 and A53 refuse.

**Measured difference.** `cargo xtask rediff --only endscrl`: **7,217 / 32,508 / 25,118 / 17,613**
pixels of 256,000 at 9100 / 10000 / 12000 / 14000 — the same four counts the unit test renders from
8835, and each 0 from 8917. Before this rewrite they were 7,289 / 28,333 / 25,502 / 17,108. In the
full run (`cargo xtask rediff --only fullrun/13-endscrl`) the three filed opening frames
46280 / 46283 / 46286, paired at A53's −88, are now **0 / 0 / 0** (they were 42 / 67 / 149: the
raster and the CR's one column).

**The end, against the original.** The controller's capture of the original (`SECOND 5`, the
standard recipe, 30,000 cycles, a frame every 250 from 30,500; git-ignored scratch) shows 640×400
up to 31,750, black at 31,500 and 31,750, and text mode with the end box from 32,000. The release's
`%` at 8917 + 22,967 = 31,884, plus the 64-frame fade, puts text mode near 31,948, inside
(31,750, 32,000]. The port's block-5 run ends its ENDSCRL at 31,802, the same 82 frames early.

**Where the 82 frames are, measured in CRED (2026-10-29).** CRED's own image does not explain
them: its loops are the tree's (102 open steps, a 200-wait hold, 59 close steps), and its tick map
is the port's (`docs/fidelity/cred.md` §9). The record's 28 wipe frames
(`out/b5-s5-cred-map`, each one state of one screen byte for byte now that the screens are the
image's) say where the frames go instead: the release shows each state **14** frames after the
port's chain at screen 1 and **77** after it at screen 20, 3 to 4 more a screen, while an open
frame and its close partner are 300 frames apart in both
(`parts::cred::tests::the_records_transition_frames_are_the_states_this_part_writes`). So the
release spends 3 to 4 frames a screen that the port does not, between one screen's last close step
and the next one's first open step — where `screenin` uploads the palette and draws the caption and
the 16,000-pixel picture one far `tw_putpixel` call a pixel, and DIS's `waitb` waits for the next
frame whatever has passed (`DIS/DISINT.ASM:161-181`). Twenty-one screens at that rate is ~70
frames, which is A53's −69 CRED→ENDSCRL change in the full run; with the 14 frames already behind at
screen 1 it is the block-5 82. It is the emulated CPU's time, which no image settles, so the port's
tick map stays the source's and the offset stays here.

**Status (Phase 9):** Carried — the block-5 offset of 82 frames is CRED's span (A53's class), not
ENDSCRL's: CRED's drawing between screens costs the release 3 to 4 frames a screen of CPU time the
port's tick map does not model (measured above). Everything this entry registered about the part's
own drawing is closed, read from the release's image (D49).

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

**Status (Phase 9):** Carried — what the oracle cannot see: its frame counter's host-timing term
(D17) is reachable by neither the API nor `--set`, and two sessions of one recipe disagree with
each other. Phase 9's longer traces are A54.

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

**Part:** the music, under every part that plays it (`Curve::sample`, `crates/sr-audio/src/mix.rs`).

**Original:** the curve's index is a 16-bit `bx` read out of the object's own code segment, so an
index past either end of the table reads the segment's neighbouring bytes (the paragraph below).

**Port:** `Curve::sample` clamps the index to the table's 2048 entries.

**Why:** the port has no such neighbours to read, and clamping invents no value (below).

**A13 (as it stands) — the curve's index is clamped at the table's ends.** The object's index is a
16-bit `bx` added to the base and read out of its own code segment (`+0x2cf4`'s `mov %cs:(%bx),%ah`,
`+0x2d08`'s `mov %cs:0x0(%bp),%al`), so a signal further from silence than the table is wide reads
whatever the linker put after it — the note tables at `+0x97c` are 0x10E bytes
past the table's end at `cs:0x86e`, and the scale table at `+0x9c4` further on. The port has no such neighbours to read
and clamps to the table's 2048 entries instead of inventing a value: `Curve::sample`. At the base
the port indexes with (`0x400 + value`) the index reaches past the table's end only for a value
at or above **+1024** — past the ±1000 these modules' signal reaches — and below the table's
start only for a value at or below **−1025**, which a sixteen-voice mono mix can reach.
~~(The marker's `0x501 + value`, which the port does not apply, would move both thresholds 257
lower; A14(a) is that question.)~~ *(Phase 9 Task 10: there is no marker — the flush writes zero,
A14(a) — so the two thresholds are the object's own.)*

**Measured difference:** none that any capture can show (the emulator runs silent, D17); the
object's value there is not a defined output but its own segment's bytes, which is the finding
rather than a number.

**Status (Phase 9):** Carried — what the oracle cannot see: the emulator runs silent (D17), so no
capture holds the sample a clamped index would change, and the object's value there is its
segment's bytes, not an output.

## A14 — The curve the machine's memory holds is not the curve the builder's formula describes: the `rep stosw` clobbers its own count ~~, and the index's base is open~~ (the base is `0x400 + value`: settled, Phase 9 Task 10)

**Part:** the music, under every part that plays it (the re-review's probe, `cargo xtask music
probe --dump`).

**Original, measured** at `--cycles 30000 --arg 5 --frames 400..1300` (`music probe --at 0x525C --dump
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
*(Phase 9 Task 10: the "marker" here is a misreading — (a) below. The code's own index is
`0x400 + value`, so the buffer and the code agree.)*

**Port.** `Curve::build` reproduces the builder **with its bug**: the zero
region is `2·⌊(0x800 − K)/4⌋` entries, the ramp follows it, and `+0x55bd`'s shortfall fills the
remainder of the span with `0xFF` for `2·⌊rem/2⌋` bytes. For `mul` 64 that is 768 zeros, the ramp
at 768…1279 and `0xFF` from 1280 — the boundaries the probe read — and `tests/mix.rs`'s
`curve_with` is the same arithmetic written out again, so a test that computed the *intended*
table (1536 zeros, the ramp at 1536…2047) or the linear map fails. The mixer indexes it at
**`0x400 + value`** (the audible base — see (a)).

**Why.** The table is behaviour on the reference's own path, built from a parameter the port
already loads (A13's ruling): the port plays what the machine's memory holds, bug and all, because
that is what the release of record plays in both modules' runs ((b) below).

**(a) The marker — settled (Phase 9 Task 10): there is none, and the code's index is the
measured one.** The flush loads a zero into a 32-bit register and stores it back over every
accumulator word it reads: the stereo flush's `mov $0x0,%esp` (`+0x2ce5`) and `mov %esp,(%si)`
(`+0x2cf1`), the mono flush's `mov $0x0,%ecx` (`+0x30d0`) and `mov %ecx,(%si)` (`+0x30e8`). The
immediates' four zero bytes are `LIDATA` runs, and the segment builder in `tests/player.rs` decoded
every such run as `0x01` until Phase 9 Task 6b fixed it — which is where the `0x0101` "marker" below
came from. So the plane a flush leaves is zero, the object's index is `0x400 + value` in every
chunk, and the code, the DMA buffer's centre and the port's `Mixer::base` are one base. The test is
`cargo test -p sr-audio --test player the_flush_clears_the_accumulator_word_it_reads`, which reads
the ten instructions out of the rebuilt segment. The builder's count bug (point 3 above;
`bugs.md` §28.4) stands.

**(a) as it stood until Task 6b's parser fix (history).** The flush writes `0x0101` into every accumulator word it reads (`+0x2ce5`,
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
~~remains open is whether the audible index carries the marker (a)~~ *(settled: it carries none,
(a))*. No capture can quantify the
audible result (the emulator runs silent, D17). `cargo xtask music
probe --dump` reproduces the **segment's** bytes above — the base word and the whole table span,
in either module's run — and does **not** reach the DMA buffer's contents (the waveform and its
0x7C/0xA2 per-channel DCs), which live at `[0xbe6]:0xC9B0` outside the located segment; ~~a dump of
that buffer beside the accumulator plane it came from is what would close (a)~~ — the code closed
it instead (a).

**Status (Phase 9):** Closed — (a): the flush writes zero, so the index is `0x400 + value`, the
port's base (`cargo test -p sr-audio --test player the_flush_clears_the_accumulator_word_it_reads`);
the table, bug included, is the probe's (`cargo xtask music probe --dump`). What stays unmeasured is
the audible result, which the silent emulator cannot hold (D17).

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

**Status (Phase 9):** Carried — an unreachable path: no caller in the tree syncs above `0x8E2E`
(`sync_refuses_the_first_key_past_the_sixteenth_compare` pins the refusal), and the answer there is
a linker-set word that differs between the tree's builds.

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

**Why (D24).** The release's wait is one measured duration that no `musplus` condition can
produce (above), and every other difference in the part is accounted for, so the port carries the
duration; the mechanism behind it is unknown, and the constant says so by its name.

**Measured difference:** **0** differing pixels at all four keyframes through the real binary
(`cargo test -p secondreality --test mntscrl the_four_keyframes_match_the_reference`; `cargo
xtask rediff --only mntscrl`); the tree's own wait would differ by 118,843 / 118,924 / 119,056
bytes at 1100 / 1700 / 2300 (the counterfactual above).

**Replaces:** the earlier version of this entry, which kept the tree's wait and registered the
divergence. **Leads for whoever takes it:** the tree's shipped copy is
`upstream/SecondReality/MAIN/DATA/MNTSCRL.EXE` (77,047 bytes, PKLITE-packed), and
`MAIN/PACKFINA.INC:56` names `MNTSCRL.EXE` in the demo's pack list. What is measured here is the
wait's duration; the packed image's own condition, if it has one, would need an unpacker.

**Status (Phase 9):** Carried — the condition behind the 71 ticks, if it is one, is inside the
packed `MAIN/DATA/MNTSCRL.EXE` (D12).

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

**Why (D24).** One rule, measured independently at three gates, reproduces the release's leave
frames, and the tree's immediate condition draws the keyframes early (the counterfactual above);
the port carries the rule as one named constant and keeps the exact cue.

**Measured difference:** **0** at 13000 and 13500, **3 / 42** pixels at 14000 / 14500 — the
release's embedded `MIEKKA.SCI`, not timing (`cargo test -p secondreality --test rayscrl`; `cargo
xtask rediff --only rayscrl`); the tree's gate would differ by 8,375 / 11,958.

**Leads for whoever takes it:** the shipped part is `upstream/SecondReality/MAIN/DATA/RAYSCRL.EXE`
(90,351 bytes, PKLITE-packed, like MNTSCRL's; `MAIN/PACKFINA.INC:62`). What is measured here is each
wait's leave frame; the packed image's own conditions would need an unpacker.

**Status (Phase 9):** Carried — the conditions behind the seven-tick confirmation are inside the
packed `MAIN/DATA/RAYSCRL.EXE` (D12), and the strip bytes the 3/42 pixels expose are its embedded
art.

---

## A19 — MINVBALL: the RNG is Microsoft's, and the uninitialised `j` is fitted (the former 67/63/37/25-pixel residual was a port defect, fixed in Phase 9 Task 9)

**Part:** MINVBALL (`DOTS/MAIN.C` + `DOTS/ASM.ASM`), Phase 4. Under D24: the source is the
evidence, the reference's frames are the authority, and the shipped release is the tiebreaker.

**Original — the RNG, identified from the compile.** The brief names Turbo C; the source says Microsoft C
6.0. `DOTS/MAKEFILE:10-16` uses `cl /qc /AL` — Microsoft's large-model compiler — and the linked
`DOTS.EXE` carries the **“MS Run-Time Library - Copyright (c) 1990, Microsoft Corp”** string. Its
`rand()` is the 32-bit LCG with modulus 2³², multiplier **214,013**, increment **2,531,011**,
seed 1, returning `(seed >> 16) & 0x7FFF`; the first ten values are 41, 18,467, 6,334, 26,500,
19,169, 15,724, 11,478, 29,358, 26,962, 24,464
(`the_rng_is_microsoft_cs_lcg_and_its_first_values_are_the_sequences`). The port implements that
generator, not Borland's.

**Original — `j` is never initialised, and the port fits its value.** `dottaul[j]` picks which dot draws a
coincident position, and the permutation's cursor `j` is never assigned (`MAIN.C:87`, `:200`): the
value is whatever the stack held. The port fits the one value the source leaves free to the
reference, **88** (`INITIAL_J`). The fit is committed as the ignored measurement test
`the_initial_j_is_fitted_to_the_reference`, which replays all 512 values against the 10,900 frame:

```bash
cargo test --release -p secondreality --bin secondreality \
    parts::minvball::tests::the_initial_j_is_fitted -- --ignored --nocapture
# best five (pixels, j): [(0, 88), (283, 171), (296, 168), (297, 386), (300, 14)]
```

88 is the only value that reproduces the frame (0 pixels; the next best leaves 283), so the fit is
unique, not a minimum among near-ties. `the_setup_is_deterministic_and_its_permutation_is_fixed`
pins the setup around it. The shipped part is
`upstream/SecondReality/MAIN/DATA/MINVBALL.EXE`, a 7,395-byte PKLITE-packed self-extractor (the same
shape as A17/A18's leads; its pack line is `dw 03BC1h,06634h,00008h,01456h,01CE3h,00000h
;MINVBALL.EXE`, `MAIN/PACKFINA.INC:57`, whose length 0x1CE3 is the staged file's), but nothing
needs unpacking: the tree's source, with this one fitted cursor, reproduces every filed frame.

**Port.** Microsoft C's generator (`MsRand`) and `INITIAL_J = 88`; everything else is the tree's
`DOTS/MAIN.C` and `DOTS/ASM.ASM`.

**Why.** The generator is the compiler's own, so it is not a choice; `j` is the one value the
source leaves to whatever the stack held, and the reference fixes it uniquely (above).

**Measured difference, through the real binary: none.** The four keyframes are byte-identical,
0 / 0 / 0 / 0 differing pixels of 64,000 at 10,900 / 11,200 / 11,800 / 12,400, and the shadow layer
agrees pixel for pixel (1,860, 2,159, 2,187 and 2,170 shared of as many). **Not four independent
confirmations:** 10,900 is the objective the `j` fit minimises, so its 0 is by construction; 11,200,
11,800 and 12,400 are out of sample (the fit never saw them) and are the evidence that `j = 88` and the
rest of the model generalise. Commands:

```bash
cargo test --release -p secondreality --test minvball the_four_keyframes -- --nocapture
cargo xtask rediff        # the four minvball rows, registered at 0, verdict ok
```

Every keyframe is reproduced at exactly one frame counter (403/703/1303/1903; the `K±1` neighbours
are different pictures, asserted by the test, so the mapping is not a loose fit).

**What this entry used to say, and why it was wrong.** Until Phase 9 Task 9 the four keyframes
differed by **67 / 63 / 37 / 25** pixels (0.1047 / 0.0984 / 0.0578 / 0.0391 %), and this entry
attributed them to "the release's own stack-`j` difference", with a 99 % shadow agreement. The cause
was a defect of the port's: `project_xz` subtracted a borrow from the previous dot's screen offset
(`bx`) in the z rotation. The original does not: `DOTS/ASM.ASM:69-70` is `sub ax,bx` / `sub dx,cx`
(object bytes `2b c3 2b d1` — `grep -obUaP '\x2b\xc3\x2b\xd1' upstream/SecondReality/DOTS/ASM.OBJ upstream/SecondReality/DOTS/DOTS.EXE` finds them once each, at offsets 782 and 3926 —), the carry from the low word is ignored and `ax` is overwritten at
`:74`, so the stale `bx` never reaches `bp` (`bugs.md` §18.1). Removing the borrow, a behaviour the
original does not have, took all four residuals to 0 and made `j = 88`
unique. Nothing in this entry is an approximation any more except the fitted `j`, which the source
leaves open.

## A20 — ALKU: the linked hzpic's three palette entries (adopted), and the text batches' phase (registered)

**Part:** ALKU (`ALKU/MAIN.C` over `ALKU/ASMYT.ASM`), Phase 4. Two entries in
one, because both are about one asset family and one frame: a D24 adoption of
three palette bytes and a residual the port's fixed tick cannot carry.

**Original — the palette.** `hzpic` is not `HOI.U`: `ALKU/INCLUDE.ASM:7-12`
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

**Original — the text batches' phase.** The fifth text batch is the empty
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

**Why.** The palette is D24's adopt-the-release case (the *Port* paragraph above); the phase is
the port's fixed tick giving `fmaketext`/`faddtext` no CPU cost and the session's own music
clock, neither of which the port can reproduce without a fit (the phase paragraph above).

**Measured difference:** 1719 and 2719 byte-identical with the shipped entries; 5859 differs by
**2,353 of 128,000** pixels, all in the old text's glyph region (`cargo test -p secondreality
--test alku -- --nocapture`; `cargo xtask rediff --only alku` registers the same 2,353).

**Status (Phase 9):** Carried — the batches' phase is the session's (D17) and the release's CPU
cost, which no filed frame records; the released bytes are the packed `MAIN/DATA/ALKU.EXE` (D12).

## A21 — PAM: the reference's 7900 index plane is the next part's white field, not PAM's screen (registered, not adopted)

**Part:** PAM (`PAM/OUTTAA.C`), Phase 4.

**Original:** the reference's 7900
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

## A22 — PLZPART: the plasma exits at the release's fourth palette table, not the tree's fifth (adopted; read from the image 2026-10-29)

**Part:** PLZPART (`PLZPART/PLZ.C`), Phase 5.

**Original:** `plz()` advances
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

**Read from the image (2026-10-29, D49).** The shipped `plz()` holds the 4 the frames
measured. In the unpacked `MAIN/DATA/PLZPART.EXE`, `curpal` is DGROUP `0192h` (DGROUP is
segment `2ee0h`, `mov ax,2ee0h` in `getspl` at image `0x14adb`): `cop_fadepal=pals[curpal++]`
is `mov ax,[0192h]; mov dx,0600h; imul dx` at image `0x1178b` and `inc word [0192h]` at
`0x117a4`, and the exit is `cmp word [0192h],4; jne; … cmp es:[000eh],40h; jle` at **`0x1181c`**
— `PLZ.C:94` with 4 for the tree's 5. The timetable at DGROUP `0194h` is the tree's
`{723, 1491, 1875, 2259, 2778, 0}`. The port's constant does not change;
`the_release_exit_index_is_the_images` pins the bytes and the timetable. The adoption is now a
reading: nothing about this exit is left registered.

## A23 — PLZPART: the cube is the release's, read from its image — a 134-row screen (CRTC 0x09 = 2) drawn at 142/66 and shown at row ⌊2y/3⌋, with the release's camera path, sine table and rotation (corrected 2026-10-26, read 2026-10-29); four band frames keep a palette-timing residue (registered)

**Part:** PLZPART (`PLZPART/VECT.C`, `PLZA.ASM`, `SPLINE.ASM`, `TWEAK.ASM`), Phase 5.
**Original:** a textured cube on a six-page flip whose music gate the reference pins to boot
8364, on a display the source itself calls "134 y", turned by a camera spline.

**Read from the image (2026-10-29, D49).** The shipped part is `MAIN/DATA/PLZPART.EXE` (PKLITE
large over LZEXE 0.91, a 223,376-byte image; `docs/fidelity/release-images.md`). Its cube is not
the tree's in four places, all of them now read or translated from the image:

- **The camera path.** `calculate` calls `getspl` as `call far 1389h:1084h` (image `0x12c8a`), and
  `getspl` begins `enter 0,0; push ds bp si di; mov si,0004h; push cs; pop ds` (`0x14914`): its
  `mov si, OFFSET buu` puts `buu` at `cs:0004h`, image **`0x13894`** (`BUU_AT`). The table has
  `RATA.INC`'s shape (136 rows of `dx, dy, dz, kx, ky, kz, l_kx, l_ky`), with every rotation and
  light word **8×** the tree's (`kkk` 800 for 100; `l_kx`/`l_ky` in 256-steps for 32-steps; the
  `REPT 100` tail `…,2048,4096` for `…,256,512`) and `dz` its own in rows 14–31 (325, 300, 300,
  325, 350 ×11, 325, 300, 325 where the tree has a flat 300). `splinecoef` follows at `cs:0884h`
  (`0x14114`) and is `SPLINE.INC` word for word; `getspl`'s code is the tree's.
- **The angle unit.** The 8× is not cancelled by a shift. `calculate` masks the five angles with
  `and ax,1fffh` (`0x12c93`–`0x12cb7`) and reads `sinit` at segment `28b1h` (`mov ax,28b1h; mov
  es,ax`, `0x12cc3`), image **`0x28b10`** (`SINIT_AT`), with `kosinit` at `es:[bx+1000h]`: 2,048
  words on, a quarter of an **8,192-entry turn**, where the tree's table is 1,024 entries with
  `kosinit = sinit + 512` bytes. The release's angles are 13 bits through an 8× finer table, so
  the cube turns at the tree's rate with 8× the angular resolution. The table (10,240 words,
  values `0, 25, 50, …`, 32,759 at the quarter) is read whole.
- **`count_const`** (`0x12d40`): the tree's expressions with `>>15` for every `>>15+7` and
  `cxz = -SY` for `-SY>>7` (`mov cl,0fh` into `LXRSH@`; `neg ax`) — a 1.15 matrix.
- **`rotate`** (`0x1307a`): not the tree's 16-bit sum. Each axis is `(x*cxx + y*cxy + z*cxz) >> 11`
  in 32 bits (`LXMUL@`, `mov cl,0bh`), plus `tx << 4` — a word in 1/16 units. The point keeps
  `>> 4` of it (`sar ax,4`, which `sort_faces` reads), and the projection divides the 1/16 words:
  `xxx = (xx16 << 8) / zz16 + 140h` (`LXLSH@`, `LDIV@`), `yyy = yy16 * 142 / zz16 + 42h`.

`sort_faces` (the `/250000 + 32` shade), `draw`, the vect loop and the projection constants are
the tree's.

**A port bug, found on the way.** The port read the light's `ls_y` as `kosinit[ls_ky] >> 8`;
`VECT.C:144` and the release (`0x12cbd`) read `kosinit[ls_kx] >> 8`. With `ls_kx` a quarter turn
and `ls_ky` 0 — the close-up's light — the port's `ls_y` was 127 where the release's is 0, the
light vector up to √2 too long, and `shadepal`'s factor left `0..=64`: a face's colours wrapped
past the 6-bit DAC into hard bands of the wrong colour (green and black on a red/yellow face, yellow
and black on a blue one, navy on a green one). Restoring the bug alone in the corrected build puts
those bands back in the full-run close-up (boots 24990/25620/26111 at 37,460/50,183/60,804
differing pixels, against 108/188/0 without it).

**The screen model (corrected 2026-10-26, unchanged).** `PLZPART/TWEAK.ASM:31-33`,
`tw_opengraph`, sets CRTC 0x09 (maximum scan line) to 2 — `mov ax,0209h / out dx,ax ; 134 y` — so
every row is shown on three scanlines of the 400-line display (400/3 = 133.3 rows); `PLZA.ASM:77`
clips at 134 rows and the three draw buffers 0x5500 bytes apart are 136 rows of 160 bytes. The
320×200 reference frames show scanline 2y, row ⌊2y/3⌋ of the image, which once made the cube look
1.5× taller and was fitted as a 200-row draw at 213/100; that draw overflowed its slot (the
"page-slot spill"). The shipped release does the same as the tree, probed live in the emulator at
boot 8500 (`SECOND 3`, 30,000 cycles) with `python3 -I tools/crtc_probe.py 3 8500 30 <outdir>`
(readings in `out/glitches/cube-crtc-8500.json`, scratch): 26 of 30 frames read CRTC 0x09 = 0x02,
20 read 0x09 = 2, 0x12 = 143 and 0x07 = 31 together, 16 of them with 0x13 = 80, and the display start
cycles 0x0028 / 0x5528 / 0xAA28 — the tree's six-page flip.

**Port.** `CUBE_DRAWN_ROWS = 134`, `CLEAR_LINES = 135`, `compose_cube` reading row ⌊2y/3⌋, the
entry pre-roll of **8 polls** (the `musplus() >= 13` gate at manifest 2641 = boot 8364, the first
draw at 8365, and at 8500 frames 136, page 4, start 0x0028). `init` unpacks the EXE and
`release_cube` reads `buu` and `sinit` at the offsets above, refusing an image of another length,
another `getspl` prologue, or a relocation inside either range. `calculate`, `count_const` and
`rotate` are the release's code; `ANGLE_MASK = 0x1fff`, `KOSINIT_WORDS = 0x800`.

**Tests** (each fails if its value reverts):
`the_release_camera_table_is_read_from_the_image` (the call and `mov si` bytes, rows 0/15/35, the
`dz` of rows 14–31, every other word against `RATA.INC` and the 8× rule, the tail),
`the_release_reads_the_trees_spline_coefficients`, `the_release_sine_table_is_an_8192_entry_turn`,
`an_image_with_another_layout_is_refused`,
`getspl_interpolates_the_release_camera_spline_at_the_keyframes` (frames 0/136/1136/3000; the
close-up's `dis` is 349 where the tree's was 299), `count_const_is_the_releases_one_point_fifteen_matrix`,
`the_cube_projection_is_the_trees_142_66` (through the 1/16 words), and
`the_light_keeps_every_shade_inside_its_band` (every visible face's shade in `0..=64` over frames
0–2199; it fails at frames 369 with shade 66 if `ls_y` reads `ls_ky`). The screen model keeps
`the_cube_is_a_134_row_screen`, `the_cube_frame_shows_row_two_y_over_three`,
`a_134_row_draw_stays_inside_its_slot` and
`the_music_pre_roll_lands_the_cube_gate_on_the_reference_tick`.

**Measured difference.** `cargo xtask rediff --only plzpart/cube --only plzpart/band` (the six band
frames are `reference/parts/plzpart/band/`, taken by `cargo xtask reference capture --arg 3 --boot
autoexec --keys "KBD_left KBD_enter" --cycles 30000 --frames 8400,8450,8500,8600,8700,8800 --out
reference/parts/plzpart/band`):

| frame | differing pixels now | with the tree's camera (2026-10-26 to 10-29) | before the 2026-10-26 correction |
|---|---|---|---|
| band 8400 | 340 | 6,531 | 20,315 |
| band 8450 | 24 | 6,092 | 21,832 |
| band 8500 = cube 8500 | **0** | 11,249 | 16,743 |
| band 8600 | **0** | 21,722 | 32,287 |
| band 8700 | 148 | 11,190 | 14,979 |
| band 8800 | 4 | 33,457 | 37,374 |
| cube 9500 | **0** | 61,165 | 63,669 |

`the_cube_keyframes_match_the_reference` (`cargo test --release -p secondreality --test plzpart`)
pins 8500 and 9500 at 0.000 % and max delta 0 (they were 17.577 % and 95.570 %).
The full run's cube frames in `reference/parts/fullrun` (24540, before the first draw) do not move.

**What the four band frames keep, registered.** All 516 pixels are both-set (painted in port and
reference), at a channel delta of at most 9 (5/8/9/5 at 8400/8450/8700/8800), in row runs on one
face (8400: 336 in the second colour block, rows 74–155). Inside those frames the reference shows
one colour index in two colours — 24 of the port's indices at 8400, 18 at 8700 — so its palette
changed while the frame was read: `vect()` sets `cop_pal=fpal; do_pal=1` before `calculate`
rewrites `fpal` (`VECT.C:123-125`), and whether the copper's upload lands before or after that
rewrite depends on the original's CPU time, not on anything the image holds. The neighbouring
ticks do not explain them (the port's ±1 frames differ in 15,904–28,246 pixels). The 8500/8600
band frames and both keyframes show none of it. This residue is the registration.

**The dense close-up.** Over the full run's close-up, the original's `out/dense-orig` frames
24990–26215 (36 frames, one every 35) against the port's `--block 1 --headless --dump-frames` at
**+38**, the dense capture's own offset from the port's numbering (the same +35–40 the plasma and
MINVBALL show in that corpus; the committed `fullrun` capture pairs PLZPART at +1): **3,614**
differing pixels in all (15 frames at 0, the largest 1,716), where the tree's camera gave
**2,120,856** (58,913 a frame). Over the whole cube, 24570–26530 (57 frames), every frame is
within 1,716 pixels. The sheets are `out/glitches/plzcube-sheet-{before,after}.png`
(scratch).

**Why — D24's procedure and D49.** The camera path, sine table and rotation are data and code
inside the shipped image, so the port reads and translates them rather than registering them
(D49). What was registered here as "the shade/pose residual, not explained" and "absent release
camera data" was the release's finer rotation and its own `buu`, plus the port's `ls_y` bug. The
remaining band residue is CPU timing, which no image or capture records.

**D24's lead.** Followed: the image is `sr_assets::exe::unpack(assets/MAIN/DATA/PLZPART.EXE)`,
pack line `dw 02BDCh,0E129h,00006h,0142Eh,0E2EEh,00000h ;PLZPART.EXE` (`MAIN/PACKFINA.INC:55`).

**Status (Phase 9):** Carried — only the four band frames' palette-timing residue (340 / 24 / 148 /
4 pixels). The extent, projection and spill were closed on 2026-10-26; the camera path, angle unit,
rotation and the `ls_y` bug on 2026-10-29.

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

**Why — the decision.** The port keeps the steady state (the boundary); the residual is
registered with the split measurements. `docs/fidelity/plzpart.md` §4 carries
the mechanism and the table. The diagnostic renders are under `out/`.

**Status (Phase 9):** Carried — what the oracle cannot see: the split the tear lands on depends on
the original's per-frame cycle costs, which no capture or trace records; the residual stays 14,348 /
7,486 / 5,186 px at 6000 / 7000 / 7600 (`cargo xtask rediff --only plzpart/plasma`).

## A25 — LNS&ZOOM: the release's face picture differs from the tree's `LENS.EXB` (registered), and the part 3 hand-over costs one frame more than the source's waits (adopted)

**Part:** LNS&ZOOM (`LENS/MAIN.C` over `LENS/ASM.ASM`), Phase 5.

**Original.** The release's `LNS&ZOOM.EXE` shows a later revision of the face picture than the
tree's `LENS.EXB`, and its hand-over from the lens to the rotazoomer costs eight frames where the
source's waits are seven (both below).

**Port.** The tree's `LENS.EXB`, read in its original
format; the hand-over's setup charged two frames, `ROT_SETUP_FRAMES` (below).

**Why.** D24: the picture is a many-pixel asset difference the tree cannot carry, so it is
registered; the hand-over is one measured constant with everything else accounted for, so it is
adopted.

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

**Measured difference:** **2,058 / 1,935 / 0 / 1,396 / 0** pixels at 2700 / 3400 / 4200 / 4900 /
5650 (`cargo test -p secondreality --test lnszoom`; `cargo xtask rediff --only lnszoom`), and the
full run's 19000 at the same 2,058 (`cargo xtask rediff --only fullrun/06-panicend-lnszoom`, A53).

**D24's lead.** The shipped part is `MAIN/DATA/LNS&ZOOM.EXE`, 67,222 bytes, PKLITE-packed; its pack
line `dw 045CCh,085D4h,00004h,01439h,0069Ah,00001h ;LNS&ZOOM.EXE` (`MAIN/PACKFINA.INC:53`) lists
0x1069A = 67,226, four bytes more than the staged file (this register's head), so the staged copy
is not quite the packed image either.

**Status (Phase 9):** Carried — the release's picture is inside the packed EXE (D12), and the
staged copy is not the one the pack line lists.

## A26 — 3DSINFLD: the render's vblank count is a fixed two, the reference's occasionally returns one (registered)

**Part:** 3DSINFLD (`COMAN/MAIN.C` over `COMAN/ASM.ASM` and the generated
`COMAN/THELOOP.INC`, Phase 5).

**Original.** The source's `frame += (frepeat = dis_waitb())` (`MAIN.C:105`, `:111`) takes
whatever the DIS's INDEMO wait returns — the number of copper frames since
the previous call (`DIS/DISINT.ASM:161-183`).

**Port.** The port's model is a fixed
**two** (`VBLANKS_PER_ITERATION`), plus the frame-4 body's two waits
(`MAIN.C:69-73`). It reproduces every keyframe byte for byte at measured
ticks, but it drifts against the reference's own clock: the four keyframes
are the port's ticks **464, 966, 1470 and 1772** and the reference's frames
**15500, 16000, 16500 and 16800**, so the implied origin is **15036, 15034,
15030 and 15028** — 2, 4 and 2 frames of drift across the 500-, 500- and
300-frame spans. In iteration terms the reference renders 500 / ((966-464)/2)
= **1.992** frames an iteration across the first span, where the port's two
is about 0.4 % slow.

**Measured difference.** The pictures do not depend on the count: the camera and wave cursors advance
once an iteration, the palette is static, and `startrise` moves once a
vblank, which the model reproduces at one a tick. The comparison is
byte-identical at all four keyframes
(`tests/coman.rs::the_four_keyframes_at_the_measured_ticks`, 0 differing
pixels each), and `docs/fidelity/3dsinfld.md` §6 states the origin per
keyframe rather than as an anchor.

**Why — not adopted, and why the other side is not readable.** A variable model
would need the release's render time, and neither the tree nor the reference
records it: the block-3 music traces carry `ord`/`row`/`zframe`, not the
part's waits, and `waitb_1`'s non-INDEMO arm is a literal 1 marked `TEMP!`
(`:178-179`) while the INDEMO arm returns the count. The port keeps the
model the pictures pin.

**Status (Phase 9):** Carried — what the oracle cannot see: the release's render time per
iteration is in no trace and no capture. In the full run its two filed frames differ by 4 (31272,
its first) and 33,554 (31901: the part draws every second frame, and the run's phase is A53's)
pixels (`cargo xtask rediff --only fullrun/08-cube-3dsinfld`).

## A27 — JPLOGO: the `doit` counter's fixed one-vblank step, and the 17700 filing (registered)

**Part:** JPLOGO (`JPLOGO/JP.C` over `ASM.ASM`, the generated `ZOOM.INC` and
`READP.C`), Phase 5.

**Original.** The source's `frame += c` (`JP.C:77`) adds the number of copper frames the
iteration took: `dis_waitb` returns the frames since the previous call
(`DIS/DISINT.ASM:161-183`). A `doit` iteration redraws up to 400 rows with
the unrolled `ZOOM.INC` routines and can take several; how many depends on
the host's speed, and a jump past 511 freezes `y1`/`y2` at whatever the
previous iteration used (`JP.C:78-83`).

**Port.** The port models **one vblank an
iteration** (`frame += 1`), which is the reference's own cadence at the filed
keyframes: 17300 is tick 235 and 17500 tick 435, 0 differing pixels each.

**Measured difference.** The row *images* depend only on the counter's current `y1`/`y2`; the
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

**Why — not adopted, and why the other side is not readable.** The reference
records the picture, not the iteration durations; the copper frame counter
is not in any trace, and the part's `waitb` returns a value no filed
capture carries. A cost model built from the `ZOOM.INC` instruction counts
would need the emulator's cycles-per-instruction and VGA wait states, none
of which the tree records.

**Status (Phase 9):** Carried — what the oracle cannot see: the `doit` iterations' copper-frame
counts are in no capture or trace, and the 17700 filing is outside its own census; the residual
stays 51,445 px (`cargo xtask rediff --only jplogo`).

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

**D24's lead.** The shipped part's pack line is `dw 058A4h,0AD8Ah,00008h,012A9h,0D4A0h,00000h
;U2A.EXE` (`MAIN/PACKFINA.INC:58`), a 54,432-byte file, and the tree's staged
`MAIN/DATA/U2A.EXE` is not it: 111,894 bytes, with no PKLITE header (this register's head). So the
release's U2A image is in `SECOND.EXE` only, packed.

**Status (Phase 9):** Carried — the release's Gouraud constants are in the packed image (D12),
which the tree's staged copy is not.

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

**D24's lead:** A28's (`MAIN/PACKFINA.INC:58`). **Status (Phase 9):** Carried — the session's phase
(D17), which the oracle's API does not reach; the port's clock is `second1`'s and moving it would
move ALKU's verified frames.

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
the_keyframes_record_the_measured_session_phase_and_gouraud_residual`; `cargo xtask rediff --only
u2e` holds the four at the same counts.

**D24's lead.** The pack line is `dw 02D44h,0AD0Ah,00009h,012ADh,0B4C6h,00000h ;U2E.EXE`
(`MAIN/PACKFINA.INC:59`), a 46,278-byte file; the tree's staged `MAIN/DATA/U2E.EXE` is 89,102 bytes
with no PKLITE header, so, as for U2A, the release's image is in `SECOND.EXE` only.

**Status (Phase 9):** Carried — the release's Gouraud constants are in the packed image (D12).

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

**D24's lead:** A30's (`MAIN/PACKFINA.INC:59`). **Status (Phase 9):** Carried — the session's phase
(D17); the port's intro is exact against the sessions that agree with it, and moving the anchor
would move it.

## A32 — U2A: the pre-gate hold shows the normal landscape, not the 30,000-cycle reference's neon (registered divergence, adopted on the user's decision)

**Part:** U2A (`VISU/C/U2A.C` over the VISU engine), Phase 6; revised 2026-10-26 (`fix-user-glitches`).

**Original:** the tree draws nothing before its first music gate (`U2A.C:229-240`); the release
shows the `U2ABG.UH` picture from the part's start to the gate (boot ~5961 to ~7000, ~13.5 s). What
palette it shows depends on the emulated CPU speed. The reference at **30,000** cycles
(`reference/parts/u2a/frame_006500.rgb`, sha256 `11a66eb0...`) holds a neon yellow-green palette,
the background palette through `(3v + 5) & 63` with entry 195 black; the `out/glitch-u2a-pad`
scratch capture (every 100 frames, 6100-7700) has 6100-6900 all `11a66eb0...` and 7000 `0a68ffb9...`.
At **15,000, 25,000, 35,000 and 60,000** cycles the same recipe gives, at boot 6400 and 6600,
`0a68ffb9...` every time (eight frames, filed as `reference/parts/u2a-speed/c{15000,25000,35000,60000}`,
`reference-indices.md` §27). `0a68ffb9...` is the `U2ABG.UH` picture under the **merged** palette
(`U2A.C:152`): the port's own post-gate frames (7000, and PAM's held 7600/7640) are the same bytes. So
four speeds out of five agree, and the neon is a timing race the 30,000-cycle reference happens to hit.
**Its mechanism is not traced**: the release's U2A is PKLITE-packed, and a guest-memory dump found the
gate loop at linear 0x23a11 (`a > 10 && b > 46`, as the tree) but did not establish where the neon DAC
comes from. An 8.8 fixed-point copper fade running about 3x its count (giving ~3v, wrapping at 64) is a
plausible reading and is **unproven**.

**Port:** `init` paints page 0 with the `U2ABG.UH` payload and uploads the **merged** palette
(`U2A.C:152`) for the whole pre-gate hold; the same palette stays at gate 1, so the hold, the first
draw and PAM's hold share one DAC. The previous `(3v + 5) & 63` pad palette (`pad_entry`, entry 195
black) is removed.

**Why (D24, applied to all the evidence; the user's decision).** The earlier entry adopted the
30,000-cycle frame as "the release's" behaviour on the evidence of that one capture speed. Four other
speeds show the normal landscape, so the neon is not a stable property of the release; the user
chose the normal landscape. This is a divergence from the 30,000-cycle reference, registered here,
not a claim that the reference is wrong at its speed.

**Measured difference:** the port's pre-gate frame compares at **0/64,000** against all eight
`u2a-speed` frames (`cargo test --release -p secondreality --test u2a
the_pre_gate_hold_compares_at_zero_with_the_four_speed_captures`, and the unit test
`the_pre_gate_hold_is_the_four_speeds_captures`) and at **64,000/64,000** against the 30,000-cycle
6500 frame (`the_30000_cycle_neon_6500_differs_by_the_registered_count`, both in the unit and the
integration tests; every pixel is off because the palette differs). `cargo xtask rediff` registers it
for `u2a` 6500 (64,000) and for `fullrun/02-u2a-pam` 6500 (64,000) and 5961 (60,160, was 63,788
under the neon; the frame is U2A's first, 47 frames late, A53). The old 57,600 / 84 / 57,516 pad-formula
counts are retired with the test that produced them.

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

**Status (Phase 9):** Carried — what the oracle cannot see: U2E's keyframes are RGB-only, so its
page origins are not established, and the gap the original's slots carry is memory no compared
frame shows.

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

**Status (Phase 9):** Carried — the session's phase (D17) and the capture's surface/DAC sampling
instant, neither of which the port can draw or the oracle's API reaches; the residuals stay 5,208 /
2,289 / 5,941 px at the surface minima (`cargo xtask rediff --only glenz`, A37).

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

**D24's lead.** The shipped part is `MAIN/DATA/GLENZ.EXE`, 57,206 bytes, PKLITE-packed, its pack line
`dw 04C66h,0D940h,00005h,01381h,0DF76h,00000h ;GLENZ.EXE` (`MAIN/PACKFINA.INC:54`), whose length is
the staged file's. The adopted key is the reading the picture identifies; the packed code that
would confirm it is not disassembled.

**Status (Phase 9):** Carried — the 711 / 839 unexplained pixels have only a hypothesis (sub-pixel
edge or phase), and the code that would settle it is the packed EXE (D12).

## A38 — GLENZ: block 1's zoomer starts 6 frames after the reference's, the restarted clock's residual (registered; restated by Phase 9 Task 6b — it was 249)

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

**Port (Phase 9 Task 6b).** The restart installs MUSIC1@0 at the port's BEGLOGO→GLENZ boundary,
**8,057**, with D34's four-tick `waiting`, and the new player **continues from MUSIC0's state**
(`Player::restart_from`, D45): `_zplaysong`'s song initialiser never writes a channel's volume, so
channel 2 is still at 63, MUSIC1's order 0 takes its row-3 `C00` and lasts **4 rows**, and the
restart publishes `1:0` at **8,087**. The wait (`MAIN.C:290`, `musplus < -19`) ends on the first
published order 2 row 45: the port's first zoomer body is at boot **8,527** — **470 ticks after the
boundary** and **6 frames after the reference's 8,521**. 8,527 is the first frame that differs from
the held picture and publishes `2:45`; 8,526 still holds at `2:44`.

**Why the residual is registered, not fitted.** It is the restarted clock's. `second1d`'s rows put
the reference's restart at **8,056** (`crates/sr-audio/tests/trace.rs`'s `RESTART_TRACES`, derived by
`block_ones_music1_restart_takes_order_0s_break` from the restarted player's own polls), and the
port's clock begins at 8,061 — its boundary, 8,057, plus D34's four ticks, which are the restart
recipes' boot relation (404 → 408) and not a measurement of a mid-run reload (A53: the port has no
loader stretch). The block-1 run is **+5** against `second1d` at its first and last row changes and
+5…−3 across `second1e` (`cargo test --release -p secondreality --test fullrun -- --nocapture`).
Against the reference's *picture* the wait's exit is that same +5: the original's picture first
differs at **8,522** (this entry's *Original*, D17's sample/publish instant), where its trace samples
`2:45` one frame earlier at 8,521, and the port publishes `2:45` and draws its first zoomer body on
the same frame, 8,527 — `FULLRUN_SEAMS`' GLENZ row, 8,522 → 8,527, is +5. The 6 against 8,521 is
that +5 and the trace's sample-to-picture frame. The 5 frames are within A12's 7-frame
inter-session spread and outside the shell's 3-frame `JITTER_FRAMES`, so the entry stays registered.
No row, order, frame or tick was adjusted to reduce it.

**Measured difference:** **8,527 − 8,521 = 6 frames** (the wait's exit). Reproduce with

```text
cargo test -p secondreality --test shell \
    a_block_one_run_is_alku_then_u2a_then_pam_in_the_boots_numbering
cargo test -p secondreality --bin secondreality \
    the_block_one_run_restarts_music1_at_glenz
cargo test -p sr-audio --test trace block_ones_music1_restart_takes_order_0s_break -- --nocapture
awk -F, '$1 == 8521' reference/music/second1d/trace.csv
```

The chain test pins 8,057, `1:0` at 8,087, 8,527, 470 and 6 and reads `second1d`'s 8,521 = `2:45`
(with the shell's `restart_from` removed it publishes `0:4` at 8,087 and fails); the restart test
pins the install, `waiting == 4` and that every channel of the new player is the old one's,
initialised; the trace test derives the 8,056.

**History.** What follows is the entry as Phase 7 registered it and Phase 9 Task 6 extended it;
its figures are the lag before Task 6b's fix.

**Port (Phase 7).** The restart installs MUSIC1@0 at the port's BEGLOGO→GLENZ boundary, **8,057**
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
lands and re-measures it. **Not changed here**: modelling it — the channel clear keeping what the initialiser keeps, and the shell's
restart handing the new player the old one's channels — is a player change that moves this entry's
pinned 8,770/249, the trace test's `RESTART_TRACES` rows for `second1d`/`second1e` (7,815, the
back-extrapolation through a 64-row order 0) and possibly the U2E and loop restarts, so it is Task
6b's. *(Task 6b made that change — the entry's head above — and measured what it moved: the 249
becomes 6, `RESTART_TRACES`' 7,815 becomes 8,056, and the U2E (`second3e`, `second1f`) and loop
restarts move by nothing, MUSIC0's orders 18 and 0 having no row whose length depends on a carried
volume.)*

**Status (Phase 9):** Carried — the +5 is D34's boot-derived four-tick wait applied at a mid-run
reload, whose real duration happens during a reload the port does not have (A53, A55); a
mid-run restart timing would be a second constant D24 does not allow without its own measurement.
Re-measured: `second1d` at +5 at its first and last rows (`cargo test --release -p secondreality
--test fullrun -- --nocapture`).

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

**Status (Phase 9):** Carried — what the oracle cannot see: the filed RGB frames are published
mid-draw, a sampling instant the port's completed frames cannot reproduce; the residual stays 6 / 2
/ 36 / 33 / 14 px (`cargo xtask rediff --only grid`).

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

**Status (Phase 9):** Carried — what the oracle cannot see: the standalone DIS the capture runs
behind has no copper service, so no capture can show the flash or its duration.

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

**Status (Phase 9):** Carried — what the oracle cannot see: a missed retrace would slow the original
below 70.086 Hz without changing its frame count, and neither the source nor the capture can observe
it; every compared frame is 0.

## A45 — TWIST: the comparison is RGB-only (registered limitation; measured difference 0)

**Part:** TWIST reference and port, Phase 8 Task 4.

**Original — the capture.** The filed reference (`reference/parts/twist`, six frames 103..603) is RGB-only.
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

**Status (Phase 9):** Carried — the overrun's position inside mode 4 is not measured (the candidate
is `B.C:208`'s palette upload); the capture measures its total, two displays, and no keyframe
depends on where it falls.

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

**Status (Phase 9):** Carried — the two bytes stay recorded, not fitted: no compared frame exercises
either (the terminator is never read as a record, and no keyframe reaches the 100th grid pair's y).

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

**Status (Phase 9):** Carried — an unreachable path: the guard never fires on part 2's measured
domain, and the code byte the original would read is the link map's, which the tree does not give.

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
Task 6; re-measured by Task 6b after the restart carry-over (D45), which moved every first frame
from GLENZ on and no `fullrun` count.

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
glenz          8522   8527          +5     +42
tunneli       11158  11151          -7     -12
techno        12214  12216          +2      +9
panicend      16193  16181         -12     -14
mntscrl       16637  16633          -4      +8
lnszoom       18656  18670         +14     +18
plzpart       21927  21928          +1     -13
minvball      26673  26670          -3      -4
rayscrl       29174  29180          +6      +9
3dsinfld      31272  31288         +16     +10
jplogo        33263  33255          -8     -24
u2e           34120  34075         -45     -37
endlogo       37967  37956         -11     +34
cred          38583  38564         -19      -8
endscrl       46277  46189         -88     -69
cue exit      16630  16632          +2
```

(JPLOGO and U2E are aligned on bytes — JPLOGO's opening black and U2E's band — because §26.3 can
only bracket their first frames.) From GLENZ to JPLOGO the parts follow the restarted `MUSIC1@0`
session, which is 5 frames late at its first rows and 3 early at its last (A38's residual), and sit
within −12…+16 of the original; the `MUSIC0@18` session after U2E is 8 frames early at its first row
and level at its last. *(Task 6 measured +230…+259 here, A38's lag before the restart kept the
channel volumes.)* ENDSCRL's −69 change is CRED's span (first frame to ENDSCRL's) being 69 frames
shorter in the port, 7,625 against 7,694 — *(2026-10-29: measured in CRED's record frames — the release
spends 3 to 4 frames a screen drawing between one screen's close and the next one's open, ~70 over
21 screens, A11.)* The parts' own residuals at the seams are: **U2A's first frame is 47 frames late, so ALKU's
end is at least 47 frames late** (the original's last ALKU frame may be its loader holding the
screen; no keyframe of `alku` sees it; the cause is not established, and the leads are the batches' `dis_sync()<4+tptr`
waits and the scroll's `frame_count<SCRLF`, `ALKU/MAIN.C:138`, `:400`); **BEGLOGO's −37 is A5's
lead-in** (the port's white field is up from its first tick; the ramp is frame-locked, the port's
7930 being the original's byte for byte); **U2E's band is 45 frames early** (37 earlier than
JPLOGO's −8): the port starts U2E at the restart's boundary, where the original's reload holds
JPLOGO's whitened screen; and the cue wait's exit is followed by MNTSCRL's first frame after **7**
frames in the original (16630 → 16637) and **1** in the port (16632 → 16633).

**Why the loader is not modelled (D31's question, D24's rule).** One `LOADER_TICKS` held between
parts would have to explain every seam the music does not decide. It does not: the changes across
the seams take both signs. All eighteen are listed. The port is already *later* at eight (ALKU→U2A +47, BEGLOGO→GLENZ +42, TUNNELI→TECHNO +9, PANICEND→MNTSCRL +8, MNTSCRL→LNS&ZOOM +18, MINVBALL→RAYSCRL +9, RAYSCRL→3DSINFLD +10, U2E→ENDLOGO +34),
and a delay inserted at a handover can only make those later still. It is earlier at the other ten
(U2A→PAM −50, PAM→BEGLOGO −34, GLENZ→TUNNELI −12, TECHNO→PANICEND −14, LNS&ZOOM→PLZPART −13, PLZPART→MINVBALL −4, 3DSINFLD→JPLOGO −24, JPLOGO→U2E −37, ENDLOGO→CRED −8, CRED→ENDSCRL −69). And the two seams the music pins on both sides need
different constants (6 frames after the cue wait; 59 to 68 under the U2E reload — its clock at
34,055, in [34,052, 34,055] (*2026-10-25 (Phase 9): was "34,054 ± 2"*), `reference-indices.md` §26.6, against U2E's first frame in (34114, 34120] — where the
port's is 0). *(Task 6 decided this under the lag, at +47/+19/+11; Task 6b re-checked it on the
corrected run under the same one-constant rule, and the decision and its reasons stand.)* Fitting a constant per seam is forbidden (D24), so the handover is
left as D31 decided it, and this entry is its record. **A later task should watch** ALKU's ≥ 47
frames, which are a part's, not a seam's.

**The filed frames at those differences.** Each `reference/parts/fullrun` frame is rendered at its
reference index plus its part's difference above — never chosen for the frame's own count — and
the three frames the original shows during a loader stretch the port does not have (7920, 16636,
34110) at the port's last frame of that screen (7884, 16632, 34042). `cargo xtask rediff --only
fullrun` compares all 60 (44 frames, 16 index planes) at their registered counts — the same 60
counts at Task 6's pairings and at Task 6b's, every one re-measured
(`xtask/src/rediff/table.rs`'s `fullrun/*` entries; the full list is `reference-indices.md` §26.8).
Every held frame is 0 in RGB but U2A's 6500 (64,000: the 30,000-cycle neon, A32), A25's 19000 (2,058), 40600 (7,204 under A10 until 2026-10-29; 0 since CRED's screens are read from its image) and 34110 (72,363: the
reload's white, which the port never shows; its index plane is 0). The moving frames this entry
registers are 5961 (60,160: U2A's opening, 47 frames late), 7930 (81,273, at BEGLOGO's −37),
11159 (192), 11900 (4,336), 16193 (108,923: PANICEND's squashed troll), 18656 (129), 24500
(45,808), 29174 (875), 31272 (4), 31901 (33,554; idx 34,036 — 3DSINFLD draws every second frame),
46280/46283/46286 (42/67/149 until 2026-10-29; 0/0/0 since ENDSCRL reads the release's image, A11); and three index planes under a 0 RGB, memory no screen shows: 7880
(48,786, PAM's picture under its white), 24540 (48,384, the cube screen under black) and 37970
(46,997, U2E's leftover video memory under ENDLOGO's white).

**Status (Phase 9):** Carried, three items, each re-measured at this commit (`cargo xtask rediff
--only fullrun`, every frame at its count; `cargo test --release -p secondreality --test fullrun --
--nocapture` prints the table above unchanged):

- **U2A's opening, 60,160 px at 5961** — the frame is U2A's first, 47 frames late; that ALKU's
  end is at least as late is measured, its cause is not established (the leads are above), and no
  `alku` keyframe sees ALKU's end.
- **PANICEND's first frame, 108,923 px at 16193** — not attributed: a moving frame at the part's
  −12 that depends on the screen and DAC TECHNO hands over (A63 is the same dependency at block 2's
  8792/8800), and no capture brackets the original's frames around it.
- **The loader's frames** — not modelled (D31): one constant cannot explain seams that move in both
  directions, and the per-seam loader cost is the session's disk and memory time.

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

**Why this is registered rather than fixed.** Neither is a seam, nor the restart: Task 6's seam
work explained A38's lag (the offset these windows are compared at) and Task 6b removed it (D45),
and neither the drift inside them nor the speed-1 row moved. The drift is A12's
inter-session fact over a longer window — the port sits on one clock, the sessions on their own —
and the unsampled row is a sampling fact: a row shorter than one sample cannot be published by a
trace that samples once a frame. A player that skipped row 26 would be wrong.

**Measured difference.** `cargo test -p sr-audio --test trace -- --nocapture the_port_reproduces`
holds both traces to `KNOWN_DISAGREEMENTS`, exactly, each simulated through the session its
restart replaces (D45): `second1e` — content identical in every word, **worst displacement 8**
(`JITTER_FRAMES` 7; the worst first reaches 1 at 8,813, 4 at 16,190 and 8 at 33,305 — the test
prints each; Task 2's fresh-restart simulation reached them at 10,827, 19,981 and 33,264);
`second1f` — one content difference (`row`, index 502, trace 27, port 26, every other value
identical), **one frame, 38,376**, whose value the trace never publishes, **worst displacement 12**,
every figure as before Task 6b. The port's whole block-1 run shows the same against them at their
offsets (`cargo test --release -p secondreality --test fullrun -- --nocapture`): `second1e` 18,928 of
25,143 frames within 3 (75.281 %) at +5, worst 8; `second1f` 6,352 of 14,168 (44.833 %) at −8, worst
12, and the one frame past the search (Task 6, at +248 and +235: 17,439 of 25,143 and 6,187 of
13,934).

**Status (Phase 9):** Carried — what the oracle cannot see: the drift is A12's inter-session clock,
and the speed-1 row is shorter than the trace's one sample a frame.

## A55 — A mid-run restart does not carry the position ring or the tick countdown (registered; measured difference 0 in every compared frame)

**Part:** the player under the shell's three restarts — block 1's `MUSIC1@0` at GLENZ
(`MAIN/U2.ASM:813-818`), `MUSIC0@18` at U2E (`:880-885`) and the loop's `MUSIC0@0` (`:929-932`) —
Phase 9 Task 6b.

**Original.** `restartmus` (`:348-397`) stops the song, reloads the module and calls `_zplaysong`
on the one player object. D45 lists what that keeps; two of the kept words are the subject here.
The **position ring** (`+0x2150`, its pointers `+0x214c`/`+0x214e` and the words a row advance
feeds it, `[0x905]`…`[0x90e]`) is primed only by `_zinit` (`+0x2a8f`, called once at `+0x2390`), and
every poll pushes and pops it, running or not (`+0x52c9`). Every tick of a stopped song
(`+0x515f` → `+0x4404`) writes `0xffff` into the fed order word (`+0x4408`). So once the song stops
the ring goes on publishing the old song's last positions for as many polls as remain of its ten,
then **order −1** with the old row (the trace tool refuses such a frame, `_np_ord reads -1`,
`music-clock.md` §7), until the new song's first row advance has come through. How far a reload
gets is the session's: at the loop, pass 2's ALKU reads the stale `(43,43)` at its first
`dis_sync` in one session of seven and the new song in six (D42's race, `reference-indices.md`
§25.3). The **tick countdown** `[0xbe0]` runs on through the reload's polls (`+0x5157`).

**Port.** `Player::restart_from` carries the channels, the loop counter and `_np_zframe`, and
not these: the new player's ring is the fresh one, so it publishes `0:0` from the restart until
its first row comes through (`0:1` at 8,075 after GLENZ's restart at 8,057), and its clock starts
D34's four frames after the boundary.

**Why it is registered rather than carried.** What the original puts in both words happens during
the reload, and the port has none (A53). Carrying the old ring would publish the old song's last
position at every restart, which is D42's minority session and never the filed one. That is
measured, not argued:

- `cargo test -p sr-audio --lib a_carried_ring_publishes_what_the_stopped_previous_player_would`
  shows that a new player carrying the old ring, its pointers and the fed words publishes, poll for
  poll, what the old player publishes when stopped and polled on. In the loop's scenario this holds
  for 18 polls (the new song's first row is fed on poll 9 and comes through ten polls later), and
  the stale position is `43:34`.
- `cargo test --release -p secondreality --test loop
  a_carried_ring_would_hand_pass_two_the_old_songs_position -- --nocapture` plays block 5's pass-1
  MUSIC0 itself and checks it frame for frame against the run's own `--positions` (404…8897). It
  then stops it at the loop's restart tick (8898) and polls it from the end of D34's four frames
  of waiting.
  - At every one of pass 2's ALKU frames before the new song's first row (8910…8920), the run
    publishes `0:0` (`dis_sync` −15609), so ALKU's first gate `dis_sync() < 1` waits.
  - A carried ring would publish `43:34` (`dis_sync` 10), past all four of ALKU's gates at once.
    D42's minority session left at once the same way, on its stale `(43,43)`, sync 10.

*(Unquantified note: an uncommitted experiment that carried the ring in the shell's restart also
made `tests/loop.rs`'s pass-2 ALKU hold stop matching the original's capture. It is not
reproducible from this tree; the two tests above are the record.)*

The −1 itself cannot be published: the port's published order is a `u8`, and U2E's `order > 18`
gate would read 255 as open. With the fresh ring, GLENZ's wait sees `musplus` −32 (the order 0 the
restart enters has `zplus` 0) and U2E's gate stays closed, which are the decisions the −1 gives
them. For pass 2's ALKU the −1's `dis_sync` key is above the table (A15), so the comparison is the
loop test's measurement, not an argument. The countdown's phase is D34's measured wait.

**Measured difference: 0 in every compared frame.**
- No filed trace holds these frames: every post-restart window opens after the new song's first
  rows (`second1d` at 8,100, `second1f` at 34,105, the loop's seam trace's `(0,1)` at 9,112).
- `cargo test --release -p secondreality --test fullrun`, `--test loop` and `cargo xtask rediff`
  hold every compared frame at its count with the fresh ring.
- The published position differs from the original's between a restart and the new song's first
  published row:
  - 18 frames at GLENZ (8,057…8,074, then `0:1` at 8,075) and 22 at U2E (34,043…34,064, then
    `18:1` at 34,065), pinned and printed by `cargo test --release -p secondreality --test fullrun
    the_full_run_plays_every_part_in_the_originals_order -- --nocapture`;
  - 23 at the loop (block-5 numbering: the restart tick 8,898, `0:1` at 8,921), pinned by
    `tests/loop.rs`.

**Status (Phase 9):** Carried — what the original puts in the ring and the countdown happens
during a reload, whose polls are the session's (D42's race), and the port has no reload (A53).

## A56 — `dis_muscode()` returns 0: the `_np_zinfo` substitute (substitution; measured difference 0)

**Part:** the engine's music seam (`sr_engine::music::muscode_from`, `sr_audio::Player::zinfo`),
under every part that calls `dis_muscode()`. In the demo none does: the callers are the vault's
`GRID/MAIN.C:72`, `:160`, `HARD/B.C:138`, `HARD/BP.C:263`, and `ENDPIC/BEG.C:48`, the build of part 4
that did not ship (D8's amendment). Phase 9 Task 10 registers what Phase 3 decided.

**Original:** `muscode_6` returns STMIK's `_np_zinfo` (`DIS/DISINT.ASM:242-282`, the load at
`:249`), a word nothing in either module produces — every `S` command is a standard `SDx` (D8) —
and whose other values no measured window shows.

**Port:** `muscode_from` returns **0** whatever the caller waits for, which is what the
reference's own word held in every sample of every filed trace. This is spec §7.4's escalation
clause taken: route 1 was performed (D8, amended 2026-10-14), it found the word but not its
mechanism, and "implement the affected parts against a substitute cue and record the difference"
is what this entry records. The vault's parts run behind the standalone DIS, whose
`dis_muscode` is key-driven, and model their one start key instead (A40, A46, A52).

**Why:** the measured value is the only one the port can stand behind; inventing the word's other
values would be a cue no frame supports. The substitute is one function body, so a later
measurement replaces it without moving anything else (`muscode_from`'s docs).

**Measured difference:** **0** in every compared frame, because no part the demo runs calls
`dis_muscode`; and the substitute is the reference's own reading:
`cargo test -p sr-audio --test trace zinfo_is_zero_in_every_filed_trace` asserts `zinfo` 0 at every
sample of every trace under `reference/music/`.

**Status (Phase 9):** Carried — what the oracle cannot see: `_np_zinfo`'s non-zero values occur
in no window any trace measured, and the parts that would wait on them are unshipped or run behind
the standalone DIS.

## A57 — DDSTARS: the star clip is assembled out, and the port does not clip (D24; measured difference 0 bytes)

**Part:** DDSTARS (`DDSTARS/STARS.ASM`), the hidden part (`--block U`).

**Original:** the tree's source still computes the clip window — `risetext` sets `_nostar1` and
`_nostar2` (`STARS.ASM:436-445`, `:525-526`) — but the `cmp di,_nostar1/_nostar2` guards that would
skip a star row inside it are commented out at `:692-696`, `:711-716`, `:737-742` and `:756-762`.
The shipped build is the packed `MAIN/DATA/DDSTARS.EXE` (10,573 bytes, `MAIN/PACKFINA.INC:61`),
and its frames show the stars in the window (`docs/fidelity/ddstars.md` §6).

**Port:** no clip — the tree's assembled-out state, and the release's.

**Why (D24):** the source and the reference agree, and the register carries it because the
source reads as though it clips: a port that followed the computed `_nostar1/_nostar2` rather than
the commented-out guards would lose the band's star rows. It is a decision about which artifact
to follow, recorded where the next reader looks (`bugs.md` §27 keeps the behaviour).

**Measured difference:** **0 bytes** at all four keyframes 1900 / 3000 / 5000 / 8000, with the
reference's **1,025** star pixels in the clip window's rows at 1900 present in both
(`cargo test -p secondreality --bin secondreality the_star_clip_is_assembled_out_and_the_keyframes_hold_its_pixels`;
`cargo xtask rediff --only ddstars`).

## A58 — The shell's keys: Ctrl skips one part, and an ESC between two parts is kept where `flushkbd` drops it (substitution; measured difference 0)

**Part:** the shell, in a window (`bins/secondreality/src/window.rs`'s `EscapeKey`, D21).

**Original:** `partexecute` (`MAIN/U2.ASM:205-215`) tests `ctrldown` before each part, and
`ctrldown` (`:185-203`) keeps its carry from the first time it sees the key: one Ctrl skips every
later part. After each part it calls `flushkbd` (`:211`; the routine is `MAIN/U2A.ASM:537-546`),
which reads and discards up to 256 waiting BIOS keystrokes — so a key pressed after a part has
stopped reading the keyboard and before the next one starts is thrown away.

**Port:** Ctrl drops the running part and the run goes on with the next (D21). An ESC press that
lands where no part is running is kept by `EscapeKey` and asked of the next part, or closes the
window when nothing is left to play. *(D21's sentence that "the original does not lose it either"
is contradicted by `:211`; D21 carries a dated note.)*

**Why:** the port's window is inside a part, not in front of one (D21's reason for Ctrl), and it
drains its events once a tick, so a press between two parts would otherwise be lost to the
port's own scheduling rather than to the original's flush; keeping it was D21's choice for the
"get me out" press that ends a block's last part.

**Measured difference:** **0** in every compared frame: no capture presses Ctrl or ESC (the
recipes' keys are the setup menu's `KBD_left KBD_enter`). The port's behaviour is pinned by
`cargo test -p secondreality --bin secondreality window::tests` —
`escape_asks_the_running_part_to_exit_and_ctrl_skips_it`,
`an_escape_between_two_parts_is_kept_and_then_closes_the_window` and
`an_escape_between_two_parts_is_asked_of_the_next_one`.

## A59 — A demo run's two text-mode screens: the settings screen for `START.EXE`'s menu, and the end box printed to the terminal (substitution; measured difference 0)

**Part:** a windowed demo run's start and end (`settings.rs`, `endscreen.rs`, D41, D44).

**Original:** `START.EXE` runs `menu()` in text mode on every start but `/gravis`
(`MAIN/START.C:89-95`, `MAIN/MENU.C`); at the end `fatalexit` sets text mode, waits twenty
retraces and calls `printend`, which copies `U2END.BIN`'s 80×25 cells into text-mode memory with
the cursor on row 14 (`MAIN/U2.ASM:506-535`, `:965-974`). The exit stage reads no key
(`fademusic` and the `copperwaitborder` loop, `:333-346`, `:969-973`).

**Port:** the settings screen is drawn in `ALKU/FONA.UH` on the port's own 520×325 frame, rows in
the original's order and then the port's own, nothing saved (D44); the exit stage's 64 + 20 ticks
are composed as the original's (D41), the end box's rows 0–13 (`0..CURSOR_ROW`, 14 lines; *2026-10-25 (Phase
9), the final review: was "rows 0–14"*. Rows 14–24, which `printend` also copies, are blank in
the shipped file, and DOS's prompt starts at row 14) are printed to the launching
terminal after the window closes — with SGR colours when stdout is a terminal — and an ESC during
the exit stage closes the window at once (D41's courtesy).

**Why:** the port has no text mode, and spec §1 replaces `START.EXE`; both screens are the port's
own surfaces, labelled so (D41, D44).

**Measured difference:** **0** on every compared frame — headless runs never show the settings
screen and the end box goes to stdout, so neither is a frame. The box's characters and colours
are the shipped file's (`cargo test -p secondreality --bin secondreality endscreen`:
`the_end_screen_is_the_shipped_box`, `the_end_screen_colours_are_its_attributes`), the stage's ticks
are D41's tests (`a_demo_run_fades_then_blanks_then_ends`), and the screen's reducer and frame are
`settings::tests`.

## A60 — The enhanced sound option: the port's own renderer, off by default, the clock untouched (D43; measured difference 0 on every compared frame)

**Part:** the music's output (`sr_audio::Quality::Enhanced`, `--sound-quality enhanced`, D43).

**Original:** the object's mixer — one byte fetched a frame with no interpolation, through the
8-bit output table, at 20,000 frames a second in the reference's stereo mode (A13, A14).

**Port:** the authentic renderer is the default and is unchanged byte for byte; the enhanced one is
opt-in: the same voices and tick schedule, each authentic frame rendered as 2 or 3 interpolated
frames at 48,000 a second, converted with the curve's small-signal gain (`music.md` §13).

**Why:** a labelled option of the port's own, which claims nothing about the original (D43); the
register carries it because a run with it sounds different from the original, and says how far.

**Measured difference:** no picture and no published position moves — every word at each of
30,000 polls of both modules is the authentic schedule's
(`cargo test -p sr-audio --test enhanced enhanced_keeps_the_authentic_tick_schedule`), and the
authentic stream is pinned to its pre-option digests
(`cargo test -p sr-audio --test mix the_authentic_stream_is_pinned_byte_for_byte`). The sound
differs by at most **0.089 %** in pitch over nine sustained notes and by **−0.18 dB** (MUSIC0) and
**−0.57 dB** (MUSIC1) in level over ten seconds (`cargo test --release -p sr-audio --test enhanced
-- --nocapture`).

## A61 — Loop mode: the seam's residuals between CRED and pass 2's ALKU (registered)

**Part:** the shell's loop seam (`--loop`, D42; `MAIN/U2.ASM:919-933`), measured against
`reference/parts/loop` (`reference-indices.md` §25.3).

**Original:** in the loop capture's numbering (115 frames after the B1 recipe's, §25.1) CRED's
closed iris `C` is 9029, the fade 9029–9092, ALKU's first pass-2 frame `L` 9104, the restarted
song's `(0,1)` 9112, ALKU's first lit frame in (10236, 10238] and its "A Future Crew Production"
hold from (10296, 10298].

**Port:** CRED shows its closed iris on two ticks before it retires (8833, 8834), so the fade is
8835–8898, then `LOOP_RELOAD_TICKS` = 11 and `L` at 8910; the restart publishes `(0,1)` at 8921;
ALKU's fade-in is lit at 10045 and holds from 10106.

**Why:** each residual is a property of something the seam inherits or a constant D24 does not let
the port fit: CRED's retirement tick is CRED's (the fade's end, 9092, is trace-measured, §25.3,
so the two-tick tail is the port's CRED: D42's cost); the restart recipe's 3 frames would be a
second constant needing its own measurement; ALKU follows the music; and CRED's 81-frame chain
drift is CRED's span (A11, A53: 3 to 4 frames a screen of drawing time).

**Measured difference**, every figure re-measured by
`cargo test --release -p secondreality --test loop -- --nocapture` (`the_loop_seam_is_the_originals`
pins each):

- **CRED's 2-tick closed-iris tail**: the fade and `L` 2 frames late against `C` (`L − C` 77
  against 75), so the port's 8908 is CRED's 320×200 screen where the original's 9104 is ALKU's;
  the iris frames 9029 / 9055 / 9081 and ALKU's 9600 / 10419 / 11419 are 0 differing pixels.
- **Pass 2's music +5 frames** at `(0,1)`: 2 of them the tail's, 3 the restart recipe's
  (`MusicStart::restart`'s timing against `restartmus` here).
- **ALKU's pass-2 fade-in +3…5 and its hold +4…6** frames (10045 and 10106 against the original's
  two-frame brackets).
- **CRED's 81-frame chain drift**, which shifts every frame of pass 2 in absolute numbering
  (9029 − 115 − 8833 = 81).
- **Δ open**: `L − 404` is 8700 if 404 is ALKU's first frame and 8698 if it is the mode set, so
  `LOOP_RELOAD_TICKS` is 11 or 9 (D42).

**Status (Phase 9):** Carried — the tail is CRED's retirement tick (CRED's work); Δ needs a
reading of whether 404 is a mode set, which the oracle did not give; the +5's recipe part would
be a second constant without its own measurement; ALKU's lag follows the music; the 81 is
CRED's span, unexplained (A53).

## A62 — Three parts' exits: ALKU's skipped closing calls, MNTSCRL's exit-aware wait, PLZPART's unpolled flag (substitution; measured difference 0)

**Part:** ALKU (`ALKU/MAIN.C`), MNTSCRL (`FOREST/READ2.PAS`) and PLZPART (`PLZPART/PLZ.C`,
`VECT.C`) — the three port deviations `bugs.md`'s "Not in this register" names.

**Original:** ALKU's `:147-152` run after either ending of the `for` loop (`:87`), the `a=319`
completion and `dis_exit` alike: `if(f>7) dofade(…)`, `fonapois()` and `close_copper()`.
MNTSCRL's first wait, `READ2.PAS:153-159`, does not test `dis_exit`. PLZPART polls `dis_exit()`
at every loop boundary — `PLZ.C:68` and `:76` for the plasma, `VECT.C:116-117` for the cube.

**Port:** ALKU returns `Finished` at the top of the tick that sees `ctx.exit_requested` and runs
none of the three (`parts/alku.rs`, `alku.md` §6). MNTSCRL's wait ends on the tick an exit is seen
(`mntscrl.md` §8). PLZPART's `frame` never consults the flag; the shell retires it at the end of
the tick the flag was set in (`Sequence::tick`, `plzpart.md` §10).

**Why:** ALKU — the prompt-exit, no-spin rule ends the part on that tick rather than fading it,
and nothing visible depends on the three calls (the engine clears the copper on the next start,
`fonapois` only clears the text's top two bits). *(2026-10-25 (Phase 9), the final review —
**restated; the ALKU clause above is superseded.** The no-spin rule does not require dropping the
calls, and "nothing visible" was not shown. Here is the source's account. **The fade is zero steps
on an exit in the original too.** `dofade`'s loop is `for(a=0;a<64 && !dis_exit();a++)`
(`ALKU/MAIN.C:306`), and DIS's exit flag is sticky within a part. ESC sets it
(`DIS/DISINT.ASM:151-153`) and `exit_2` returns it (`:186-190`). Only `version_0` clears it
(`:90`), and `dis_partstart` calls that at a part's start (`DIS/DISC.ASM:51-53`). So when the
`for` loop at `:87` leaves on `dis_exit`, `:148`'s `dofade(palette2,palette)` fails its first test
and writes no palette. The port's skip matches the original's ESC path step for step. (The
original runs the 64 steps only on the `a=319` completion with `f>7`, and the port does too: `parts/alku.rs`,
`MAIN.C:147-149`.) **`fonapois()` (`:150`, `:250-270`) is the one skipped call whose effect could
be seen.** It is unconditional, and it clears bits 6–7 of every byte of rows 64–319 on the four
planes, which erases whatever text the batch had up from ALKU's last screen. Whether a frame shows
that before U2A's first picture replaces the screen depends on the handover, which is A53's, and it
is **unmeasured**. This is a knowing deviation on the ESC path. `close_copper()` restores the
interrupt, and the engine clears the copper on the next start.)* MNTSCRL — the port does not spin, and the
source's own wait would hold an ESC until the tree's `musplus >= 0` gate (trace frame 2488);
PLZPART — the engine's flag is sticky and the shell retires a part that does not poll on the same
tick, so ESC ends it on that tick with or without the poll.

**Measured difference:** **0** in every compared frame — no capture presses ESC. The port's
behaviour is pinned by `an_exit_request_ends_every_wait_on_the_tick_it_is_seen` (ALKU),
`exit_requested_ends_the_wait_on_that_tick` (MNTSCRL) and the shell's
`the_exit_flag_asks_the_running_part_to_quit_and_the_run_goes_on`
(`cargo test -p secondreality --bin secondreality`).

**Status (Phase 9):** Carried — what no capture shows: whether a frame on ALKU's ESC path shows the
skipped `fonapois()` text clear is unmeasured, because no capture presses ESC and the frames between
ALKU's end and U2A's first picture are A53's handover. The skipped fade is not open: on that path the
original runs none of its steps either (above).

## A63 — PANICEND: a standalone run cannot answer the handoff frames 8792 and 8800 (registered limitation; 0 through the handoff)

**Part:** PANICEND (`PANIC/SHUTDOWN.C`), `--part panicend` against `reference/parts/panicend`.

**Original:** block 2's PANICEND starts over the screen and DAC TECHNO leaves, and its first filed
frames are states of that inherited picture: 8792 the collapse's seven rows 97–103 of 234 pixels,
8800 the line, row 100 (`docs/fidelity/panicend.md` §1, §5).

**Port:** reached through the chain, it inherits the same; a standalone `--part panicend` run
starts on a blank surface and the machine's default DAC, so its ticks 10 and 18 — the mapping's
own numbers for 8792 and 8800 — are a different picture.

**Why:** a part run has no previous part to inherit from; the frames are the handoff's, and the
handoff test drives them from TECHNO's state instead.

**Measured difference:** through the handoff, **0** — `the_handoff_matches_the_collapse_8792_and_the_line_8800`
(`cargo test -p secondreality --bin secondreality parts::panicend`) reproduces both byte for byte.
Standalone, **1,638** and **234** pixels — exactly the reference's non-black pixels, 7 × 234 and
234 (`cargo xtask rediff --only panicend`), the 4,914 and 702 bytes
`the_standalone_run_cannot_answer_the_handoffs_8792_and_8800_and_says_so` pins
(`cargo test -p secondreality --test panicend`).

## Generated and self-modifying code (spec §5.2), Phase 9 Task 10

Spec §5.2 named ten candidates — six generated unrolled loops and four self-modifying sites — "to
be confirmed per part". The outcome of each, with its evidence:

| candidate (spec §5.2) | outcome | evidence |
|---|---|---|
| `GLENZ/ZOOMLOOP.INC` | **not applicable**: linked (`ZOOM.ASM:26`, `GLENZ/MAKEFILE:3`) as the body of `_zoom`, which only the unshipped `zoomer1` calls; the shipped `zoomer2` uses none of it | `glenz.md` §5 |
| `JPLOGO/ZOOM.INC` | **bit-identical**: the 160-entry dispatch and the routines' row images are `DOL.C`'s formula, per pixel | `the_zoom_window_is_the_source_bytes_at_each_scale`, `the_scale_dispatch_is_the_tables_and_the_reachable_range_stays_inside` (`parts/jplogo.rs`); 17300/17500 at 0 (`tests/jplogo.rs`'s `the_three_keyframes_at_the_measured_ticks`); 17700's residual is A27's step, not the routines |
| `TWIST/TWSTLOOP.INC` | **bit-identical**, consumed as data: the include's 16,000 texture bytes, read back out of its stores | `the_texture_table_is_the_shipped_twstloop` (`parts/vault/twist.rs`); six keyframes at 0 (A42–A45) |
| `GRID/LINEBLIT.INC` | **bit-identical**: the generator's XOR Bresenham against all 357 committed routines | `the_line_blit_is_the_committed_generated_code` (`parts/vault/grid.rs`) |
| `COMAN/THELOOP.INC` | **bit-identical**: the 5,230-line include parsed and executed against the port's `theloop`, whole 64 KB vbuf, five columns | `the_ray_march_matches_the_shipped_generated_program` (`parts/coman.rs`); four keyframes at 0 (A26) |
| `HARD/LINEBLIT.INC` | **not applicable**: on no link line (`HARD/LINK.TMP:1-3`, `hard.md`'s head) | `hard.md` |
| TUNNELI's "patched y-base" | **bit-identical in colour, and the spec's wording is wrong**: what `TUN10.PAS` patches is the plot's **colour immediate** (`:168` `mov byte ptr cs:[@c+3],al`, the plot at `:187`), not a y-base; the port passes `bbc` as a variable | `tunneli.md` §5; the colour field is exact (A6: the residual is presence, not colour) |
| `GLENZ/VEC.ASM`'s matrix-in-immediates | **bit-identical**: state only, held in variables | `glenz.md` §5; 1499 at 0 in both planes (`the_held_opening_1499_is_the_reference_index_plane`), and every object pixel at 2305 (A37) |
| `LENS/ASM.ASM`'s patched addressing | **bit-identical**: the displacement table patched into the unrolled draw is evaluated as a table, in the source's order | `the_aspect_ratio_scales_the_row_advance_not_the_in_row_steps` (`parts/lnszoom.rs`); 3400/4200/4900 byte-identical once the forehead is the reference's (`the_forehead_asset_difference_is_the_whole_residual`, A25) |
| PLZPART's "jump table" | **bit-identical in the line, and the spec's wording is wrong**: `setplzparas` (`ASMYT.ASM:74-95`) patches **four table bases** into `plzline`'s instructions (the placeholders `0xC100`…`0xC400`, `:46-61`), not a jump table; the port evaluates the patched index expression over the map's flat segment | `the_table_bases_are_the_maps_not_the_assembly_placeholders`, `the_plasma_line_is_the_assemblers_index_expression` (`parts/plzpart.rs`); the plasma frames' residual is A24's tear, registered there |

None of the ten is a registered difference of its own: eight are bit-identical where the reference
can see them and two are not on the shipped path.
