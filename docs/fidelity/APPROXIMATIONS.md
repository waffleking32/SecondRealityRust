# Approximations register

Every place the port knowingly differs from the original. An entry requires a justification and a
measured visual difference; it is not a parking place for "close enough".

## How an entry reads

A live entry carries five labels: **Part**, **Original**, **Port**, **Why** and **Measured
difference**. Where something stays open it adds a **Status** line that says what is carried and
why it cannot be closed. Every figure names the command or test that reproduces it (D25). The
derivation behind a figure, where it is long, is in the part's fidelity note, and the entry points
there. The mechanism the port follows is in the module's docs.

An entry that no longer registers a difference is a tombstone, `## A<n> — Retired: …` or
`## A<n> — Closed: …`, with a short paragraph (the closing evidence, or where it now lives), so
that the code's and the notes' citations still resolve. Numbers are never reused. The history of an
entry is in git.

`cargo test -p xtask --test approximations` checks that every live entry has the five labels, that
the numbers increase, and that every `register` string in `cargo xtask rediff`'s table
(`xtask/src/rediff/table.rs`) names a live entry.

**The release's images.** "The shipped part" is the part's EXE as `MAIN/DATA/` stages it, packed.
`sr_assets::exe::unpack` turns each into the load image DOS runs (D49; the layers and their
verification are `release-images.md`). The exceptions are `U2A.EXE` and `U2E.EXE`: the staged
copies are not packed and are not the release's builds (D12): their `MAIN/PACKFINA.INC` lengths are
54,432 and 46,278 bytes where the staged files are 111,894 and 89,102, so the release's U2A and U2E
exist only packed inside `SECOND.EXE`, in no form the port reads. Where an entry says an image is "not yet read", the image
unpacks, but the port has not read that item out of it.

## A1 — Overscan border is padded, not scanned out

**Part:** all; used deliberately by PANIC and the 320×400 parts.

**Original:** the border is the region of the display outside the active area, set by AC 0x11 and
scanned out by the CRTC.

**Port:** `View::border` is a colour. `present()` draws it as a flat frame of 8 320-column pixels per
side (`8 × scale` output pixels) at every geometry, around a picture it does not rescale (D48).

**Why:** the engine composes the active display only (`design.md` §4.2). The border must still
appear, but its thickness depends on CRTC timing the port does not model, and the timing does not
give a whole number of source pixels.

**Measured difference:** **0 on every compared frame**. The oracle renders no overscan region (its
VGA blank line is the black index; only a CGA's is the overscan colour,
`dosbox-automation/src/hardware/video/vga_draw.cpp:1160-1215`), so the captures hold no border, and
`xtask compare` reads the composed frame, not the presented one.

What the border would be is derived from the timing instead. No part on the shipped path writes a
timing register: their CRTC writes are 0x09 (`inittwk`'s `0x0009`, `BEG/ASM.ASM:21-24`,
`END/ASM.ASM:21-24`; LENS's `or al,3`, `LENS/ASM.ASM:223-231`), 0x0C/0x0D, 0x13, 0x14, 0x17, 0x18,
and PLZPART's 0x07, written as `0x0F | LC8`, which keeps mode 13h's timing bits
(`PLZPART/ASMYT.ASM:103-112`). GLENZ's 240- and 350-line tables (`GLENZ/VIDINIT.ASM:10-25`) belong to
modes the part does not use; the shipped `init320x200` has its tweak commented out
(`GLENZ/VID.ASM:213`). `MAIN/VMODE.ASM` programs nothing (its mode save/load is `IF 0`, `:131-152`,
and its dispatch is commented, `:22-31`). So the timing is the BIOS's. In the oracle, mode 13h and
mode 0Eh (ENDSCRL, `ENDSCRL/MAIN.C:22-26`) are both `htotal 100, hdispend 80, vtotal 449,
vdispend 400` (`src/ints/int10_modes.cpp:40`, `:45`), programmed at `:1352-1460` as CR00 95, CR01
79, CR02 80, CR03 0x82 with CR05 bit 7 (blank end 34), CR06 0xBF, CR12 0x8F, CR15 0x96 with their
overflow bits (447, 399, 406) and CR16 0xB9. `vga_draw.cpp:2001-2095` reads them back as a line of
100 characters, 80 displayed, blanked from 80 to 80 + ((34 − 80) & 0x3F) = 98, and a frame of 449
lines, 400 displayed, blanked from 407 to 407 + ((0x39 − 407) & 0x7F) + 1 = 442. The border is what
is displayed outside the active area and not blanked:

| side | characters / lines | 320×200 | 320×400 | 640×400 |
|---|---|---|---|---|
| left | 98–99: 2 characters | **8** | **8** | **16** |
| right | none (blank starts at display end) | **0** | **0** | **0** |
| top | 442–448: 7 lines | **3.5** | **7** | **7** |
| bottom | 400–406: 7 lines | **3.5** | **7** | **7** |

These are source pixels (a character is four pixels in the 256-colour modes and eight in ENDSCRL's
16-colour one). They are not whole at 320×200, which is most of the demo, so `BORDER_PX` stays 8 on
every side. A derived pad would not be a measurement either: the IBM reading of the same registers
gives 3 and 4 rows at 320×200, and nothing arbitrates the one line; and the horizontal border is
one-sided, which a monitor and the 256-colour pipeline's own delay shift and nothing here models.

**Status:** carried as a presentation choice: the oracle renders no border to compare against.

## A2 — An RLE row that decodes wider than its header's width is clipped

**Part:** START (`START/HZPIC.P`); the case exists in any file of that vintage.

**Original:** `readp()` (`ENDPIC/READP.C:10-41`, `START/READP.C:10-42`) decodes a whole row into the
caller's buffer with no bound, and `START/MAIN.C:105-113` hands it `char rowbuf[640]`
(`START/MAIN.C:13`). The 153 of `HZPIC.P`'s 400 rows whose opcode streams yield more than 640
pixels (756 for most, 761 at the widest) write past the end of that buffer, into whatever the
linker placed after it. The row the CRTC scans out is the first 640 pixels either way. The row
length prefix, not the header's width, bounds the decoded stream.

**Port:** `sr_assets::origin`'s `decode_rows` takes the first `w` pixels of each row and drops the
rest, counting them.

**Why:** the visible output is those first 640 pixels. The demo needs the file, and the wide rows
are rows it reads (`START/MAIN.C:105-113` uses rows 0..255 and 300..331), so refusing it is not an
option, and guessing what the overflow corrupted would be worse.

**Measured difference:** none in the visible 640×400 output. 153 of 400 rows drop between 3 and 121
pixels each, 16,191 pixels of the decoded stream (6.32 % of 256,000), all beyond column 639.
Measured by decoding `assets/START/HZPIC.P` row by row.

## A3 — Three malformed-input paths in PAM's FLI decoder take defined behaviour where the tree's player has none

**Part:** PAM (`PAM/PRAX4.FLI`, decoded by `sr_assets::anim`). PAM's `.ANI` needs no entry: its one
malformed-input path, a run past its 64,000-pixel canvas, is one `PAM/ANIM.C`'s encoder cannot emit
(`cnt+a<64000`).

**Original:** three constructs in `PAM/VFLI.C` have no defined behaviour: (1) a run that writes
past the canvas, since the only guard is `u<64000` before a packet (`:184`, `:219`, `:256`) and a
16-bit offset wraps into the top-left corner; (2) a chunk body longer than its frame, which
`doblock` reads with no bound (`:87-88`) into the next frame; (3) a frame record with no chunks,
which is never dumped, because `blocksleft` reaches zero only inside the chunk branch (`:89-90`).

**Port:** the first two are refused (`Unsupported` and `Truncated`, naming the byte). The third
completes with the canvas unchanged, which is what a frame with no chunks means in the format.
`sr_assets::anim`'s module docs list the same three.

**Why:** the first two read or write outside the picture's or the frame's own bytes, and the crate
promises that no input panics and none is decoded past its end. Reproducing them would invent
output. The third drops a frame, and the port prefers the record chain it walks.

**Measured difference:** **0 on the tree's file**: over `PRAX4.FLI`'s 42 frame records every packet
stays inside its line and the canvas, every chunk fits its frame, the walk lands on the file's last
byte, and every record carries 1 or 2 chunks. The `0x0004` and `0x0007` decodes are ported as
`VFLI.C:161-172` and `:203-246` write them, but no file carries either chunk type (the histogram is
`0x0B:1 0x0C:41 0x0F:1`), so they are cited, not measured.

**Status:** carried as unreachable paths: no file the demo ships reaches them.

## A4 — Two FLI decoding choices where the port follows the format rather than VFLI.C's own reading

**Part:** PAM (`PAM/PRAX4.FLI`). A3 covers the malformed-input paths. These two are choices about
what a valid decode is.

**Original:** `VFLI.C:157` sets the DAC index from a colour chunk's `first` field, but `:158` stores
the payload into its own `palette[]` from index 0 (as `0x0004` does at `:169`/`:170`), the `pal.pal`
dump (`:47-52`) that `PAM/ANIM.C` reads back. And `VFLI.C` hardcodes the stride 320 (`:183`, `:218`,
`:254`) and the end 64,000, so a FLI of another shape would shear.

**Port:** a colour chunk's range is applied at `first`, which is what the DAC shows, and the canvas
is the header's width × height, as in every other decoder in `sr_assets`.

**Why:** the palette the port needs is the one the part displayed, and `VFLI.C`'s array is a dump
buffer for a second tool. The geometry is the file's own declaration; `VFLI.C`'s 320 is a limit of
the tool that converted this one animation.

**Measured difference:** **0 on the tree's file**, for both. `PRAX4.FLI`'s one colour chunk carries
`first` 0 and 256 entries (the chunk body's words `01 00 00 00` at offset 150), and the decoded
palette equals `PAM/PAL.PAL` entry for entry. The header declares 320×200 at offsets 8 and 10, and
the tree holds exactly one FLI.

## A5 — BEGLOGO's lead-in: nine frames of the original's 42 are not modelled

**Part:** BEGLOGO (`BEG/BEG.C`).

**Original:** the fade's first displayed step is reference index **446** (`beglogo.md` §3), and a
run that starts at BEGLOGO begins drawing at **404** (`shell::FIRST_GRAPHICS_FRAME`,
`reference-indices.md` §1.12). Of the **42** frames between, the source accounts for **33**: the 32
vertical blanks of `BEG/BEG.C:24` and the tick `:25-78` runs on. The other nine have no identified
cause. Two candidates are ruled out:

- **Not the EXE load.** `reference-indices.md` §3.3 files 410, 429, 430 and 431 as one white screen,
  and `BEG/BEG.C:20-21`'s memsets make that white, so the part's own code is running by 410.
- **Not `dis_partstart()`** (`BEG/BEG.C:17`). It is a version check (`DIS/DISC.ASM:51-64`) and costs
  no frame.

**Port:** the white field for the whole 42 ticks, and the blit at tick 32 (index 436), with no model
of anything between.

**Why:** the port decodes the picture once, in `init`, and cannot re-pay a cost whose shape it does
not know. Moving the blit to where the nine frames "must have" landed would be a fit against no
evidence.

**Measured difference:** the lead-in window is captured (`reference-indices.md` §25.6).
- **RGB: 0.** `beglogo-leadin/even` and `/odd` (430–450) are white through 446 on both sides, and
  the ramp's first displayed step is 447. That 447 is one frame after the 446 measured in §3, which
  is D17's session alignment, not the port.
- **Index plane: 127,998 of 128,000** at 436 (`beglogo-leadin/plane-436`). The original's plane is
  the white field (all index 15) at every index the oracle can read, 430–438, while the port's holds
  the picture from its blit at 436 (its other 2 indices are picture pixels of index 15). From 439
  the oracle cannot read the plane (§25.6), so where the original's code blits is bounded below by
  439 and not measured.

Both counts: `cargo xtask rediff --only beglogo-leadin`.

**Status:** carried for 439–446: the oracle cannot read the index plane there, so the nine frames
keep no cause. ENDLOGO's lead-in has the same shape (A7).

## A6 — TUNNELI: the shipped build's second palette ramp, and the residual left over it

**Part:** TUNNELI (`TUNNELI/TUN10.PAS`). One entry for two things: a ramp the tree's source and the
shipped build disagree about, and the residual left once the shipped one is ported.

**Original:** `TUN10.PAS:122` writes `(64-x) *3 div 4` into DAC entries 128..192, and the tree's
`TUN10.EXE` compiles exactly that (image `0x1d0`-`0x1e3`). The build every reference frame came from
uses **`(64 - x) * 3 div 5`** at the same 65 entries. The four keyframes exercise 42 of the ramp's
entries (38 with a non-black reference pixel, 5,275 informative pixels, 29 distinct greys), and every
one matches `3*(64-k) div 5`, including the 17 where rounding rather than truncation would differ. A
search over every `(mul, div)` pair with `mul ≤ 7` and `div ≤ 15` (81 candidates, 70 distinct ramps)
puts the port 1,108 pixels from the four frames at 3/5 and at least 4,200 at every other (the
source's 3/4 gives 6,376). `tunneli.md` §5 has the derivation.

**Port:** the shipped ramp, `3*(64-x) div 5`, with `TUN10.PAS:122` cited as the structure it
reproduces.

**Why:** D12: the distributed release is the reference for every frame diff, and the tree is the
reference for how the effect works. The entry exists because the port deliberately does not
transcribe a line of the tree's source.

**Measured difference:** **179 / 373 / 321 / 433** of 64,000 pixels at 3800 / 4200 / 4500 / 4600
(port ticks 78 / 478 / 778 / 878; `cargo xtask rediff --only tunneli`). The difference is presence,
not colour: of the 1,108, 1,091 are pixels the port inks where the reference shows background, and
17 are dots inked in different colours on both sides. The neighbouring ticks leave 754 and 953 at
3800, so the alignment is whole. The keyframes are not equally trustworthy (`reference-indices.md`
§1.8, `tunneli.md` §7.2): 3800 is settled in the census and 4200 half-settled, while 4500 and 4600
have no settled content, so nothing attributes the residual there.

In the full run (`SECOND 1`, where TUNNELI starts 7 frames early, A53), the two filed frames differ
by **192** (11159) and **4,336** (11900): `cargo xtask rediff --only fullrun/04-glenz-techno`.

**Status:** carried. The residual is unattributed: no session reproduces the 4500 and 4600
pictures, and the release's `TUNNELI.EXE` image (D49) is not yet read for it.

## A7 — ENDLOGO: the music gate's one-frame alignment, and ten lead-in frames are not modelled

**Part:** ENDLOGO (`END/END.C`).

**Original:** the gate is `END.C:107-111`:

```c
for(a=0;a<5000 && !dis_exit();a++) { dis_waitb(); if(dis_musplus()>-16) break; }
```

It holds until the music's position passes −16, so its length is a property of the music and of
where the run started it (`MAIN/U2.ASM:905`). Before the fade, the source's lead-in is 36 ticks to
`:94-99`'s `readp` and 400-row blit (`endlogo.md` §4 has the tick map), and the fade's first step is
displayed at tick 47 (from index 451). The source accounts for 37 of those ticks; the other ten have
no identified cause, as A5's nine. The same two candidates are ruled out: the part's own white is up
by 415 (`reference-indices.md` §19), and `dis_partstart()` (`END.C:50`) is a version check.

**Port:** the gate is the source's own loop against the run's player, with a one-frame
`GATE_ALIGNMENT_TICKS` (`endlogo.md` §5, §5.2): the port's music words and the reference's frames
are one frame apart, and the frames are the record. The lead-in draws the white field and blits at
tick 36 (index 440), with no model of the ten ticks before the fade.

**Why:** the alignment is a fact of the reference's boot and `second5`'s session, measured, not a
property of ENDLOGO. The lead-in's ten ticks are not modelled for A5's reason.

**Measured difference:** the hold the cue produces is the **567** ticks the reference's fade
constants give (`endlogo.md` §3), which the part's tests compare the cue's answer with, and the four
keyframes are 0 (`cargo xtask rediff --only endlogo`). The lead-in window (`reference-indices.md`
§25.6):
- **RGB: 0.** `endlogo-leadin/even` and `/odd` (436–455) are white through 451 on both sides, and
  the ramp's first step is 452.
- **Index plane: 55,499 of 128,000** at 440 (`endlogo-leadin/plane-440`). The original's plane is
  index 0 at every index the oracle can read, 436–443 (`END.C` whitens DAC entries 0–254 and never
  fills video memory), while the port's holds the picture from its blit at 440; the picture's index-0
  pixels are the rest. From 444 the plane cannot be read.

Both counts: `cargo xtask rediff --only endlogo-leadin`.

**Status:** carried for 444–450: the oracle cannot read the index plane there. BEGLOGO's nine and
this part's ten both sit between a 400-row Origin decode-and-blit and a palette fade, linked from
the same two source files (`BEG/ASM.ASM` and `END/ASM.ASM` differ by one comment character). That is
a common candidate, not a measured cause.

## A8 — CRED: the font strip the routine reads is not the strip the file is

**Part:** CRED (`CREDITS/MAIN.C` over `CREDITS/FONA.INC`). A data divergence: what the routine
states and what the file holds do not agree, and neither can be changed.

**Original:** `#define FONAY 32` (`CREDITS/MAIN.C:29`) with `extern char far font[FONAY][1500]`
(`:31`): the routine reads 32 rows of a raster that has 40 (`asset-inventory.md` §E.8, §E.20). It
also states a 32-column cell (`fonap[32]=1500-32; fonaw[32]=8;`, `:270-271`, the blank) on a strip
1,500 columns wide, which the cell does not divide. The strip's real structure is 47 ink runs, one
per character of `fonaorder` (`:32`), which the routine scans (`:250-269`).

The release's image has the same raster and the same window. Its font array, at image `0x29f0` of
the unpacked `CRED.EXE`, is `FONA.INC` byte for byte, 40 rows, and its scan and `prt` read 32 rows
(`cred.md` §9).

**Port:** the part reads the raster and `fonaorder` out of the release's image and translates
`:250-269`'s scan. It does not load the strip through `sr_assets::font::Font`, which refuses a cell
that does not divide the raster.

**Why:** both mismatches are the shipped art's. Reading the first 32 rows is what the original
draws and what the reference shows.

**Measured difference:** the eight unread rows hold **92** ink pixels, and no column is inked in
them that rows 0–31 leave blank, so all 47 runs, every `fonap` and every `fonaw` are the same whether
the scan reads 32 rows or 40
(`parts::cred::tests::the_eight_rows_the_routine_never_reads_do_not_move_a_run`). The display shows
every other logical line, so **46** of the 92 fall on displayed rows. On all five keyframes the
caption region is byte-identical to the reference's
(`tests/cred.rs`'s `the_five_keyframes_are_the_references_byte_for_byte`), and the image's font and
order are the tree's (`parts::cred::tests::the_images_font_and_order_are_the_trees_byte_for_byte`).

## A9 — Closed: ALKU's two font rows past `FONA.UH` are zero in the release's image

The routine's rows 30–31 are uninitialised padding after the 30-row strip (`ALKU/INCLUDE.ASM:19`),
and the release's image holds zeros there, so the port's 30 rows are everything it inks
(`the_release_font_is_the_strip_but_for_one_glyph_and_its_padding_is_zero`; `alku.md` §12).

## A10 — Retired: CRED's 21 screens are read from the shipped image

The nine screens the release changed (2, 6, 10, 11, 12, 17, 19, 20, 21) are read from
`MAIN/DATA/CRED.EXE` (D49, `cred.md` §9) and pinned by `the_release_changes_nine_of_the_trees_screens`;
every CRED frame is 0 (`cargo xtask rediff --only cred --only fullrun/12-cred`).

## A11 — ENDSCRL: block 5's chain starts the part 82 frames early

**Part:** ENDSCRL, in a `--block 5` run (`bins/secondreality/src/parts/endscrl.rs`).

**Original:** the shipped part is `MAIN/DATA/ENDSCRL.EXE`, whose code differs from the tree's
`ENDSCRL/MAIN.C` in six ways (control lines, a 25-row pitch, a 30×1,550 raster, an 80-character
`fonaorder`, the blank at 1530, and `exit(0)` on a `%` line), all read off the image
(`endscrl.md` §1). In the original's block 5, ENDSCRL starts at **8917**.

**Port:** the release's build: the part reads its raster and `fonaorder` out of the unpacked image,
and its `init` and `do_scroll` are that image's. It ends by itself at the `%` line, part tick 22,967
(`the_part_ends_by_itself_on_the_percent_line`). The port's chain starts it at **8835**, 82 frames
(41 ring lines) earlier, because the port's CRED ends 82 frames before the release's.

**Why:** the offset is CRED's span, not this part's drawing. CRED's image has the tree's loops and
tick map (`cred.md` §9), and the record's wipe frames show the release spending 3 to 4 frames a
screen that the port does not, between one screen's last close step and the next one's first open
step, while `screenin` uploads the palette and draws the caption and the picture a pixel at a time
(`cred.md` §7). That is the emulated CPU's time, which no image holds. Moving ENDSCRL's start to fit
it would be the per-seam constant D24 and A53 refuse.

**Measured difference:** `cargo xtask rediff --only endscrl`: **7,217 / 32,508 / 25,118 / 17,613**
of 256,000 pixels at 9100 / 10000 / 12000 / 14000. Rendered from the release's own start, 8917, all
four are **0** (`the_port_draws_the_references_frames_at_the_references_own_ticks`). In the full run
the three filed opening frames 46280 / 46283 / 46286 are 0 at A53's −88
(`cargo xtask rediff --only fullrun/13-endscrl`). The port's block-5 ENDSCRL ends at 31,802, the
same 82 frames before the release's `%` at 8917 + 22,967 = 31,884.

**Status:** carried: the 82 frames are CRED's drawing time between screens (A53's class), which the
port's tick map does not model.

## A12 — The music clock: the port's tick clock is exact, the reference's frame counter is not

**Part:** the music, under every part that plays it (the traces under `reference/music/`).

**Original:** the player's tick is `2.5/tempo` seconds of the emulated machine's time, and the
traces sample it once per frame of the emulator's frame counter (`music-clock.md` §1). The ratio
between the two is a property of the emulator's host timing, which D17 measured as a term the API
and `--set` do not reach.

**Port:** `Player::poll` adds exactly `mix_rate/70.086` samples per logic tick and advances a tick
whenever `mix_rate × 2.5/tempo` samples have accumulated: integer arithmetic, no drift. The port runs
48 ticks a second at tempo 120. The traces' `zframe` column gives 47.93 (`second5`), 48.04
(`hiddenu`), 48.06 (`second1`) and 52.01 (`second3`, tempo 130): the same clock to 0.15 %, not the
same to the frame.

**Why:** it cannot be fixed. Two sessions of one recipe, `second5` and `second5b`, disagree with each
other about which frame eight of their 131 row changes land on, by one frame each
(`cargo test -p sr-audio --test trace -- --nocapture the_two_sessions`, `music.md` §8). A clock that
matched one session would be a fit to it; the exact clock sits between them.

**Measured difference:** `cargo test -p sr-audio --test trace -- --nocapture the_port_reproduces`
prints, for every trace in `reference/music/`, the share of frames whose published position matches
the port's and the worst displacement, each window trace first phase-aligned to its own session
(the offsets are the boot's spread, `music-clock.md` §2). The largest displacements are `second1`'s
**7** over 5,538 frames and `second3b`'s **6** over 4,839, the traces whose rate is farthest from the
exact one. The test asserts that the port publishes exactly each trace's sequence of `order`,
`pattern`, `row`, `speed`, `tempo`, `loop` and `zplus`, each value within **7** frames, under one
6-tick row (8.76 frames). The publication delay and the row arithmetic are the original's own and
not part of this entry (`music.md` §8).

**Status:** carried: the oracle's frame counter has a host-timing term (D17) that neither the API nor
`--set` reaches. Block 1's longer traces are A54.

## A13 — The output curve's index is clamped at the table's ends

**Part:** the music, under every part that plays it (`sr_audio`'s `Curve::sample`,
`crates/sr-audio/src/mix.rs`).

**Original:** the mixer converts the accumulator through the table `+0x551c` builds (A14,
`music.md` §10.3). Its index is a 16-bit `bx` added to the base and read out of the object's own code
segment (`+0x2cf4`'s `mov %cs:(%bx),%ah`, `+0x2d08`'s `mov %cs:0x0(%bp),%al`), so an index past
either end reads the segment's neighbouring bytes: the note tables at `+0x97c`, 0x10E bytes past the
table's end at `cs:0x86e`, and the scale table at `+0x9c4` beyond.

**Port:** `Curve::sample` clamps the index to the table's 2048 entries. At the base `0x400 + value`,
the index passes the table's end only for a value at or above **+1024** (beyond the ±1000 these
modules reach) and its start only at or below **−1025**, which a sixteen-voice mono mix can reach.

**Why:** the port has no neighbouring bytes to read, and clamping invents no value.

**Measured difference:** none that any capture can show (the emulator runs silent, D17). What the
object returns there is its segment's bytes, not a defined output.

**Status:** carried: no capture holds the samples a clamped index would change.

## A14 — Closed: the curve builder's clobbered count is reproduced, and the index base is `0x400 + value`

`Curve::build` reproduces `+0x559d`'s `rep stosw` clobbering its own count (`bugs.md` §28.4,
`music.md` §10.3). The measurement: `cargo xtask music probe --at 0x525C --dump 0x6C:2 --dump
0x6E:16 … --dump 0x86C:2` at `--cycles 30000 --arg 5 --frames 400..1300` reads the base word
`cs:0x6C` as `6E 00` at every frame, and the table as `00` for entries 0…767, the ramp from 768 to
1279 and `FF` from 1280, unchanged through the window. (b): the MUSIC1 window (`--arg 3 --frames
600..900`) reads all 2048 bytes the same, since both sessions run `_zoutputmode` 5 and master volume
0xB0, so `mul` is 64 in both. (a): the flush writes zero (`+0x2ce5`/`+0x2cf1` stereo,
`+0x30d0`/`+0x30e8` mono), so there is no marker and the index is `0x400 + value`
(`cargo test -p sr-audio --test player the_flush_clears_the_accumulator_word_it_reads`).

## A15 — `dis_sync()` above `0x8E2E` is refused, not returned

**Part:** the engine's music seam (`sr_engine::sync_from`, `music.md` §11), reached by the parts
that call `dis_sync()`: `PAM/OUTTAA.C:39` and `ALKU/MAIN.C:59`, `:66`, `:72`, `:79`, `:138`.

**Original:** `sync_10` (`DIS/DISINT.ASM:449-463`) turns the published position into a part-start
code with the `ordersync1` table above it (`:428-446`). The loop is `mov cx,16` over fifteen
four-byte entries (`:455`), and the answer is read at `%bx - 2` (`:460`). Above the last key
(`0x4200`) the 16th iteration compares against the bytes after the table, which are `sync_10`'s first
instruction, `mov es, cs:stmikseg` (`2e 8e 06 …`):

| key | the object's 16th iteration | the port |
|---|---|---|
| `<= 0x8E2E` | the compare matches `2e 8e`, the same in every build, and the answer is the table's last code, **4** | returns 4 |
| `> 0x8E2E` | the compare fails, and the loop returns the `mov`'s modrm byte and the low byte of a linker-set displacement: `0xCD06` in `MAIN/U2.EXE`, `0xED06` in `MAIN/2NDFIX.EXE`, `0x9D06` in `U2.EXE` | panics, naming the position |

At `(0, 0)` the `%bx - 2` read lands on the two bytes before `ordersync1`, `pop es; ret` (`07 c3`,
`DIS/DISINT.ASM:420`, `:424`), the same in every shell build the tree holds.

**Port:** `sync_from` reproduces the table, the `%bx - 2` arithmetic and both answered reads:
**4** up to and including `0x8E2E`, and **−15609** (`0xC307` read as the parts' 16-bit Turbo C `int`)
at `(0, 0)`. Above `0x8E2E` it panics.

**Why:** a port cannot return a number that differs between builds, so only the second tier is
refused. The rest is the source's own bytes in every build, and it is reached: `hiddenu` sits above
`0x4200` for 5,148 of its 6,351 frames, and `second5` at `(0, 0)` for 278 samples.

**Measured difference:** none for the answered domain (`sync_reads_the_table_the_object_reads`,
`sync_refuses_the_first_key_past_the_sixteenth_compare`). Neither caller reaches the refused
region: the highest key ALKU's or PAM's gates ask about is `0x0D01`.

**Status:** carried as an unreachable path: no caller in the tree syncs above `0x8E2E`, and the answer
there is a linker-set word.

## A16 — The boot's twelve-frame music transient: a run installs its own module at its start, not the boot's

**Part:** the music of a run whose entry restart replaces the boot's module: `--block 5` and
`--part endlogo`, which install MUSIC0 at order 25.

**Original:** the shell's boot loads MUSIC1 at order 0 (`MAIN/U2.ASM:772-775`), and a block-5 run
replaces it with MUSIC0 at order 25 (`:898-905`) before ENDLOGO starts. The speed/tempo words change
at frame **396** (MUSIC1's 3/130) and again at **408** (MUSIC0's 6/120), with no tick between, and
`_np_zframe` is 0 throughout.

**Port:** the run installs the entry point's module at its start and never loads MUSIC1. The run's
first drawn frame is 404, so on frames 404…407 the part-facing `MusicPosition::speed`/`tempo` read
6/120 where the trace reads 3/130.

**Why:** reproducing it means a second player's lifetime for four frames. The published position is
`0:0` on both sides, so every cue a part could read is the same, and no part reads `speed` or `tempo`.

**Measured difference:** frames 404–407 of a `--block 5` run differ in `speed` and `tempo` only (D27);
from 408 they agree. The trace's readings are `reference/music/second5` frames 396 and 408
(`music.md` §12).

## A17 — MNTSCRL: the first wait is the release's 71-tick duration, not the tree's music gate

**Part:** MNTSCRL (`FOREST/READ2.PAS`). The port follows the release (D24): the reference is the
authority, and the run must match its four keyframes.

**Original:** `:153-159` busy-waits `dis_musplus() >= 0` after the fade-in and the font build. The
release's last held frame is boot **554** and its pre-roll's first `scr` boot **555**. With tick 0 at
boot 419 (`mntscrl.md` §6) that is ticks 65 through 135, a first wait of **71 ticks**. No `musplus`
threshold gives it: the published position is order 42 row 16 on both sides of the boundary
(`reference/music/second3/trace.csv` frames 554 and 555) and `zplus` is 0, so `musplus` is −32 at
both, and any `musplus >= X` either ends the wait at tick 65 or never at tick 135.

**Port:** the measured duration, `RELEASE_WAIT_TICKS = 71`. `Mntscrl::tree_wait` keeps the tree's
condition for the counterfactual test.

**Why:** one measured duration that no `musplus` condition can produce, with every other difference
in the part accounted for (D24's single-constant case). The mechanism behind it is unknown, and the
constant's name says so.

**Measured difference:** **0** at all four keyframes 500 / 1100 / 1700 / 2300, with `K±1`
distinguishable (`cargo test -p secondreality --test mntscrl the_four_keyframes_match_the_reference`;
`cargo xtask rediff --only mntscrl`). The tree's `musplus >= 0` would first hold at trace frame 2488,
and the part would still be holding at 1100, 1700 and 2300, **118,843 / 118,924 / 119,056** bytes
away (`the_trees_own_wait_would_hold_through_the_scroll_keyframes`). The part's other release value,
the `scp` bound, is `mntscrl.md` §7.

**Status:** carried: the condition behind the 71 ticks, if there is one, is in the release's
`MNTSCRL.EXE` image (D49), not yet read.

## A18 — RAYSCRL: the release's gates act seven ticks after the tree's condition

**Part:** RAYSCRL (`WATER/DEMO.PAS`). The port follows the release (D24).

**Original:** three waits, each acting on the first tick its condition holds: `:98-104`
`dis_musplus() >= 0`; `:135-143` `order != co` and `row >= 16`, with no `waitb`; and `:158-166`
`dis_musplus() == -11`, the cue. In the release, each leaves seven ticks after its condition first
holds on the port's clock, measured independently:

| gate | condition first holds | release leaves |
|---|---|---|
| init | boot 12962, order 88 row 0 (`reference/music/second3b/trace.csv`) | the blit at 12969 (the 13000 keyframe is the fade's k=14 hold, 31 ticks on) |
| stage 2 | boot 13609, order 89 row 16 | the main loop's first `scr` at 13616 (the 14000 keyframe is draw index 384) |
| cue | boot 14942, `musplus == -11` (the port's player publishes it one poll later) | the fade's first upload at 14950 (`reference-indices.md` §15.4's partner capture) |

The values are not thresholds: the row stays 16 for eight frames (13609–13616) and −11 for nine
(14942–14950), so the leaves fall inside those windows, not at their starts.

**Port:** the condition must read true on seven consecutive ticks, and the gate acts on the tick
after (`GATE_CONFIRM_TICKS = 7`). The cue stays exact: only −11 triggers the fade, which runs 65
uploads to black (`the_cue_is_exactly_minus_eleven_and_fades_sixty_five_ticks`).

**Why:** one rule, measured at three gates independently, reproduces the release's leave frames, and
the tree's immediate condition draws the keyframes early.

**Measured difference:** **0 / 0 / 3 / 42** pixels at 13000 / 13500 / 14000 / 14500 (13500 in both
planes; `cargo test -p secondreality --test rayscrl`; `cargo xtask rediff --only rayscrl`). The 3
and 42 are not timing but the release's embedded `MIEKKA.SCI`, which differs from the tree's at the
strip bytes those frames expose (`rayscrl.md` §6). With `Rayscrl::confirm_ticks = 0` (the tree's
condition) the main loop starts six draws early, and 14000 and 14500 differ by **8,375** and
**11,958** (`the_trees_immediate_gate_draws_the_keyframes_early`).

**Status:** carried: the release's conditions and its `MIEKKA.SCI` are in its `RAYSCRL.EXE` image
(D49), not yet read.

## A19 — MINVBALL: the RNG is Microsoft C's, and the uninitialised `j` is fitted

**Part:** MINVBALL (`DOTS/MAIN.C`, `DOTS/ASM.ASM`).

**Original:** `DOTS/MAKEFILE:10-16` compiles with `cl /qc /AL`, Microsoft's large-model compiler,
and the linked `DOTS.EXE` carries Microsoft's run-time copyright. Its `rand()` is the 32-bit LCG with
multiplier 214,013, increment 2,531,011 and seed 1, returning `(seed >> 16) & 0x7FFF`. The
permutation cursor `j` behind `dottaul[j]`, which picks the dot that draws a coincident position, is
never assigned (`MAIN.C:87`, `:200`): its value is whatever the stack held.

**Port:** Microsoft C's generator (`MsRand`;
`the_rng_is_microsoft_cs_lcg_and_its_first_values_are_the_sequences` pins its first ten values) and
`INITIAL_J = 88`, fitted to the reference. Everything else is the tree's.

**Why:** the generator is the compiler's own. `j` is the one value the source leaves free, and the
reference fixes it uniquely: of all 512 values replayed against the 10,900 frame, 88 alone gives 0
pixels and the next best leaves 283:

```bash
cargo test --release -p secondreality --bin secondreality \
    parts::minvball::tests::the_initial_j_is_fitted -- --ignored --nocapture
```

**Measured difference:** **0 / 0 / 0 / 0** of 64,000 pixels at 10,900 / 11,200 / 11,800 / 12,400,
with the shadow layer pixel for pixel (`cargo test --release -p secondreality --test minvball
the_four_keyframes -- --nocapture`; `cargo xtask rediff --only minvball`). 10,900 is the fit's
objective, so its 0 is by construction; the other three are out of sample. Each keyframe is
reproduced at exactly one frame counter (403 / 703 / 1303 / 1903), and the `K±1` neighbours are
different pictures.

## A20 — ALKU: the linked hzpic's three palette entries, and the release's credits loop with what it leaves

**Part:** ALKU (`ALKU/MAIN.C` over `ALKU/ASMYT.ASM`).

**Original:** the palette: `hzpic` is not `HOI.U`. `ALKU/INCLUDE.ASM:7-12` assembles `HOI.IN0` +
`HOI.IN1`, and the linked payload differs from `assets/ALKU/HOI.U` in exactly nine bytes, palette
entries 1–3 (`asset-inventory.md` §E.18). The picture bytes are the same:

| entry | the linked hzpic (shipped) | `HOI.U` |
|---|---|---|
| 1 | `(10, 10, 10)` | `(2, 2, 3)` |
| 2 | `(30, 30, 30)` | `(5, 5, 7)` |
| 3 | `(63, 63, 63)` | `(6, 6, 9)` |

They are the text palette's three levels: `fade2`'s entries 1–3 become 64, 128 and 192
(`MAIN.C:194-208`), which every glyph pixel uses.

The loop: the release's `main`, read from its unpacked `ALKU.EXE`, differs from the tree's in its
bound, the scroll mode after the fifth batch, 29-row loops, and a closing routine that compacts the
last window into the 320×200 page U2A's background continues (`alku.md` §12 has the table). After the
fifth batch `do_scroll(0)` neither carries the text nor waits, so the pass spins at CPU speed.

**Port:** `HOI.U` with the three entries overridden by `SHIPPED_PALETTE_ENTRIES`, and the release's
loop and closing routine (`MainPhase::Spin`, `HandoverStep`), pinned against the image by
`the_release_loop_and_its_closing_routine_are_the_images`, without the routine's three page copies,
which nothing shows before the exit.

**Why:** the palette is D24's adopt-the-release case (one data constant, the picture byte-identical,
everything else accounted for). The loop and the closing routine are D49's read-from-the-image case.
What remains is CPU time and session state that the image does not hold:

- **The spin's `f`.** The release's `if(f>7)` then reads a counter its CPU has stepped tens of
  thousands of times. The port takes the frames' answer, no final fade
  (`SPIN_LEAVES_F_ABOVE_THRESHOLD = false`): with it ALKU would close after the original's first
  320×200 frame (5961, `reference/parts/fullrun/02-u2a-pam`). The spin's other passes (the fades
  they restart, and a sixth batch's "BUG BUG BUG" if `cop_dofade` reached 0 with `f>50`) touch only
  `tbuf` and palette entries 64–255, which no pixel uses once `case 4` has cleared the text, so the
  port holds `f` at 51 and does not run them.
- **The emulator's deferred CRTC 9.** The oracle applies a CRTC 9 change half a frame late, so its
  capture holds one frame (5950) of the compacted page turning double-scanned at line 185
  (`alku.md` §12). The port applies the write at the start it waited on, as the hardware does.
- **The batches' phase.** The batches wait on `dis_sync()` and on the release's CPU time in
  `faddtext`/`fmaketext`. A dense capture of block 1, not filed under `reference/`, shows that
  session's text fading earlier than the port's from the second batch on (only at fade edges; no
  frame outside a fade shows text where the other shows none), and nothing says which of the two
  moved them.

**Measured difference:** 900, 1719, 2719 and 5859 are **0** (`cargo test -p secondreality --test
alku -- --nocapture`; `cargo xtask rediff --only alku`). With `HOI.U`'s own entries, 1719, 2719 and
5859 differ in **1,595 / 4,171 / 966** of 128,000 pixels:

```bash
cargo test -p secondreality --bin secondreality \
    parts::alku::tests::the_curated_palette_entries_leave_the_reference_different \
    -- --ignored --nocapture
```

ALKU ends 4 ticks after its last step, and U2A starts at 5956 in a block-1 run, 5 frames before the
original's first 320×200 frame (A53).

**Status:** carried: the spin's `f` and the batches' phase are the session's CPU time.

## A21 — PAM: the reference's 7900 index plane is the next part's white field, not PAM's screen

**Part:** PAM (`PAM/OUTTAA.C`). Registered, not adopted.

**Original:** the reference's 7900 RGB is the flat `(251,251,251)` wash. Its index plane is 64,000
bytes of **15**, and its manifest palette entry 15 is `[62,62,62]`, which widens to 251. That plane
cannot be PAM's frame: PAM's 7900 screen is its last animation frame (frame 39, 64 distinct indices)
under the near-white 63rd palette copy. Index 15 everywhere is BEGLOGO's white field
(`BEG/BEG.C:18-21`). The indexed read runs about twenty frames after the RGB (D15), and the
reference's 7900 is just before BEGLOGO's mode change in (7920, 7930] (`reference-indices.md` §6.5),
so the read caught BEGLOGO's field while PAM's palette was still in the DAC. Every index of the white
palette widens to one colour, so the coherence check cannot tell (`reference-indices.md` §1.6).

**Port:** the source's screen, animation frame 39 under palette copy 63.

**Why:** D24 step 2: the tree cannot produce the reference's plane at this index, because the plane
is not this part's screen. The comparison at 7900 is RGB. `pam.md` §6 has the frame-by-frame
reading.

**Measured difference:** RGB **0** at 7710, 7760, 7830 and 7900 (`cargo test -p secondreality --test
pam -- --nocapture`). Index plane at 7900: **60,375 of 64,000**, max delta 194 (the frame's highest
index 209 less 15): `cargo xtask rediff --only pam`. The port's plane is `OUT.ANI`'s frame 39
(`the_7900_index_plane_is_the_next_parts_white_field_not_pams_frame`).

## A22 — Closed: PLZPART's plasma exits at the release's fourth palette table, read from its image

`EXIT_PALETTE_RELEASE = 4` is the release's `plz()` (`cmp word [0192h],4` at image `0x1181c`, where
`PLZ.C:94` has 5), pinned by `the_release_exit_index_is_the_images` and
`the_release_exit_index_is_the_measured_one` (`plzpart.md` §10).

## A23 — PLZPART: the cube's four band frames keep a palette-timing residue

**Part:** PLZPART's cube (`PLZPART/VECT.C`, `PLZA.ASM`, `SPLINE.ASM`, `TWEAK.ASM`).

**Original:** a textured cube on a six-page flip, on a display the source calls "134 y":
`tw_opengraph` sets CRTC 0x09 to 2 (`TWEAK.ASM:31-33`), so each row is three of the 400 scanlines;
`PLZA.ASM:77` clips at 134 rows. A 320×200 reference frame shows scanline 2y, row ⌊2y/3⌋. The release
behaves the same, probed live at boot 8500 (`SECOND 3`, 30,000 cycles) with
`python3 -I tools/crtc_probe.py 3 8500 30 <outdir>`: 26 of 30 frames read CRTC 0x09 = 2, and the
display start cycles `0x0028` / `0x5528` / `0xAA28`.

The release's cube differs from the tree's in its camera table `buu`, an 8,192-entry sine turn,
`count_const`'s 1.15 matrix and a 32-bit `rotate`, all read or translated from the unpacked
`PLZPART.EXE` (`plzpart.md` §5).

**Port:** the 134-row screen (`CUBE_DRAWN_ROWS = 134`) composed at row ⌊2y/3⌋, the release's camera
path, sine table, `count_const` and `rotate` (`release_cube` reads them out of the image), and an
entry pre-roll of 8 polls that lands the `musplus() >= 13` gate on boot 8364.

**Why:** the camera path, sine table and rotation are data and code inside the shipped image, so the
port reads them (D49). What is left on the band frames is CPU timing, which no image or capture
records: `vect()` sets `cop_pal=fpal; do_pal=1` before `calculate` rewrites `fpal`
(`VECT.C:123-125`), and whether the copper's upload lands before or after that rewrite depends on
the original's CPU time.

**Measured difference:** `cargo xtask rediff --only plzpart/cube --only plzpart/band` (the band frames
are `reference/parts/plzpart/band/`, taken by `cargo xtask reference capture --arg 3 --boot autoexec
--keys "KBD_left KBD_enter" --cycles 30000 --frames 8400,8450,8500,8600,8700,8800 --out
reference/parts/plzpart/band`): **340 / 24 / 0 / 0 / 148 / 4** pixels at band 8400 / 8450 / 8500 /
8600 / 8700 / 8800, and **0** at the cube keyframes 8500 and 9500
(`cargo test --release -p secondreality --test plzpart the_cube_keyframes_match_the_reference`).
All 516 band pixels are painted on both sides, at a channel delta of at most 9,
in row runs on one face. Inside those frames the reference shows one colour index in two colours
(24 of the port's indices at 8400, 18 at 8700), so its palette changed while the frame was read. The
port's ±1 frames differ in 15,904–28,246 pixels, so the neighbouring ticks do not explain them.

**Status:** carried: the four band frames' palette-timing residue (340 / 24 / 148 / 4 pixels).

## A24 — PLZPART: the plasma's per-pass parameter latch is a CPU-timing tear the port does not reproduce

**Part:** PLZPART's plasma (`PLZPART/PLZ.C` over `PLZPART/COPPER.ASM`).

**Original:** `plz()`'s frame loop has no frame wait (`PLZ.C:74-122`), and the copper's once-a-vblank
`moveplz`/`pompota` can land between or inside its four plane passes. `setplzparas` latches the
`k`/`l` parameters at the start of each pass, so the screen tears at whatever pass boundary the
retrace lands on, which depends on the original's per-frame execution time.

**Port:** a fixed 70.086 Hz tick that draws the four passes with one parameter generation, the
loop's phase-locked steady state (the copper's walk runs at the tick's end). The arithmetic, the
timetable, the palette accumulators and the split/pan walk are the source's.

**Why:** the split the tear lands on needs the original's cycle costs. No fixed split matches all
three keyframes: a diagnostic that forced the walk before each pass gave the 6000 frame's residual
by split position as 4.15 / 5.23 / 4.38 / 8.42 / 11.21 %, while 7000 and 7600 sit at the frame
boundary (`plzpart.md` §4 has the table). The reference's own sessions agree within 0.09–3.08 %,
because a session's timing repeats.

**Measured difference:** **14,348 / 7,486 / 5,186** of 128,000 pixels (11.209 / 5.848 / 4.052 %) at
6000 / 7000 / 7600, max channel delta 16 / 21 / 17 (`cargo xtask rediff --only plzpart/plasma`;
`cargo test -p secondreality --test plzpart`).

**Status:** carried: the split depends on per-frame cycle costs that no capture or trace records.

## A25 — LNS&ZOOM: the release's face picture differs from the tree's `LENS.EXB`, and the part 3 hand-over costs one frame more than the source's waits

**Part:** LNS&ZOOM (`LENS/MAIN.C` over `LENS/ASM.ASM`).

**Original:** the release shows a later revision of the face picture than the tree's `LENS.EXB`. And
its hand-over from the lens to the rotazoomer costs eight frames where the source's waits are seven:
the lens loop's exit wait, the five-frame hold (`:179-182`) and the `waitb()` before `inittwk`
(`:199`). The reference's 4200 is the path's frame 552 and its 4900 frame 1252, both `T3 + frame`
with `T3 = 3648`, while the lens's last draw is at 3640 (`T2 = 2926`, pinned by the trace's `48:44`
at 2925 and the lens's frame 474 at 3400).

**Port:** the tree's `LENS.EXB`, in its original format, and two frames for the hand-over's setup
(`ROT_SETUP_FRAMES`): the rotpic90 rotation's 65,536 byte moves and `inittwk`'s 64 KB clear cost the
release a vblank.

**Why:** D24. The picture is a many-pixel asset difference the tree cannot carry, so it is
registered. The hand-over is one measured constant with everything else accounted for, so it is
adopted.

**Measured difference:** **2,058 / 1,935 / 0 / 1,396 / 0** pixels at 2700 / 3400 / 4200 / 4900 /
5650 (`cargo test -p secondreality --test lnszoom`; `cargo xtask rediff --only lnszoom`), and the full
run's 19000 at the same 2,058 (`cargo xtask rediff --only fullrun/06-panicend-lnszoom`). The 2,058
are the reference's held face (2700's `.idx`) against the tree's picture, all in the forehead (rows
33..91, columns 129..211), each a small palette-index change. Patching the tree's picture with the
reference's 2700 index plane makes 3400, 4200 and 4900 byte-identical
(`parts::lnszoom::tests::the_forehead_asset_difference_is_the_whole_residual`). The reference's
5650 `.idx` is the tool's planar memory read, a different geometry from the displayed picture, and
is not compared (`lnszoom.md` §7.2).

**Status:** carried: the release's picture is in its `LNS&ZOOM.EXE` image (D49), not yet read. The
pack line lists 67,226 bytes (`MAIN/PACKFINA.INC:53`), four more than the staged file's 67,222.

## A26 — 3DSINFLD: the render's vblank count is a fixed two, the reference's occasionally returns one

**Part:** 3DSINFLD (`COMAN/MAIN.C` over `COMAN/ASM.ASM` and the generated `COMAN/THELOOP.INC`).

**Original:** `frame += (frepeat = dis_waitb())` (`MAIN.C:105`, `:111`) takes the number of copper
frames since the previous call (`DIS/DISINT.ASM:161-183`).

**Port:** a fixed two (`VBLANKS_PER_ITERATION`), plus the frame-4 body's two waits (`MAIN.C:69-73`).
The four keyframes are the port's ticks 464, 966, 1470 and 1772 and the reference's frames 15500,
16000, 16500 and 16800, so the implied origin is 15036, 15034, 15030 and 15028: 2, 4 and 2 frames of
drift across the three spans. Across the first span the reference renders 1.992 frames an
iteration, where the port's two is about 0.4 % slow.

**Why:** a variable count would need the release's render time, which neither the tree nor the
reference records: the music traces carry `ord`/`row`/`zframe`, not the part's waits. The pictures
do not depend on the count (the camera and wave cursors advance once an iteration, and `startrise`
once a vblank), so the port keeps the model the pictures pin. `3dsinfld.md` §6 states the origin
per keyframe.

**Measured difference:** **0** at all four keyframes at the measured ticks
(`cargo test -p secondreality --test coman the_four_keyframes_at_the_measured_ticks`;
`cargo xtask rediff --only 3dsinfld`). In the full run its two filed frames differ by **4** (31272)
and **33,554** (31901, where the part draws every second frame and the run's phase is A53's):
`cargo xtask rediff --only fullrun/08-cube-3dsinfld`.

**Status:** carried: the release's render time per iteration is in no trace and no capture.

## A27 — JPLOGO: the `doit` counter's fixed one-vblank step, and the 17700 filing

**Part:** JPLOGO (`JPLOGO/JP.C` over `ASM.ASM`, the generated `ZOOM.INC` and `READP.C`).

**Original:** `frame += c` (`JP.C:77`) adds the copper frames the iteration took. A `doit` iteration
redraws up to 400 rows and can take several frames, depending on the host's speed, and a jump past
511 freezes `y1`/`y2` at the previous iteration's values (`JP.C:78-83`). The screen depends on the
path through the palimpsest guard (`lasty`/`lasts`, `JP.C:96-99`): how far the counter jumps decides
which rows an earlier state leaves behind.

**Port:** one vblank an iteration (`frame += 1`), the reference's own cadence at the filed keyframes.

**Why:** the reference records the pictures, not the iteration durations, and the counter is in no
trace. A cost model from `ZOOM.INC`'s instruction counts would need the emulator's cycles per
instruction and VGA wait states, which the tree does not record. The 17700 filing is outside its
own census (`reference-indices.md` §17.3, §17.5): its state is settled in 2 of 4 sessions
(`ec3de2bb…`), while the filed picture (`91d6ce6b…`) is presented by none, and twelve captures hold
ten distinct images at 17700. No deterministic model can be asked to reproduce that.

**Measured difference:** 17300 (tick 235) and 17500 (tick 435) are **0**. The port's 17700 is the
census's settled state (its `--dump-frames 635` RGB has sha256
`ec3de2bb823772bf275387ab122e12e17b1d27a140ec9be8a71faa7d1bd3cb37`), **51,445** of 128,000 pixels
from the filed picture, max channel delta 255 (`cargo xtask rediff --only jplogo`;
`tests/jplogo.rs` asserts the count).

**Status:** carried: the iterations' copper-frame counts are in no capture or trace, and the 17700
filing is outside its own census.

## A28 — U2A: the release's Gouraud shading differs at 7300

**Part:** U2A (`VISU/C/U2A.C` over the VISU engine, `bins/secondreality/src/parts/visu/`).
Registered, not adopted.

**Original:** the reference's 7300 is the release's rendering of stream state 294, the state
search's unique minimum (426 differing pixels; 892 at 293 and 858 at 295). The difference is in the
Gouraud shading of the three `U2A.001` instances. The silhouette and the flat-shaded `U2A.002` are
exact.

**Port:** the tree's algorithm (`ACALC.ASM`'s rotations, `ADRAW.ASM`/`ADRAWCLP.ASM`'s culling,
clipping and spans, `AVIDFILL.ASM`'s three Gouraud tiers). At state 294 it differs in 426 pixels,
all on the Gouraud faces: 75 on instance 1, 0 on 2, 115 on 4 and 236 on 5, with index deltas from −5
to +12.

**Why:** D24 step 3: no tree-carriable constant reproduces the release's output. The variant scan
in `u2a.md` §5 covers the face lists, draw and matrix order, normal rotation, camera multipliers,
Gouraud slope, tier and buffer variants and lighting, and an independent transliteration agrees
with the engine byte for byte at state 294. The tree's two fillers (`AVIDFILL.ASM`, `AVIDFILL.OK`)
differ only in copy loops. The release's U2A is not in the tree in a readable form (this register's
head), and a one-frame fit to one Gouraud picture could not be checked against another.

**Measured difference:** **426 of 64,000** (0.6656 %) at state 294, all on the Gouraud faces
(`cargo test -p secondreality the_reference_7300_is_the_measured_stream_state`). At the boot index
the frame is A29's.

**Status:** carried: the release's Gouraud constants are in an image the tree does not hold.

## A29 — U2A: the 7300 boot-index picture is a session-phase offset

**Part:** U2A (`VISU/C/U2A.C`).

**Original:** boot 7300 displays stream state 294 in the session whose keyframes are filed at
`reference/parts/u2a`.

**Port:** boot 7300 displays state 297, three states later.

**Why:** D17: the music clock's session phase, amplified by the part's page holds. The port's
block-1 clock matches `second1`: at boot 5900 it reads 9:51 / zframe 3771, the trace's own value,
while `second1c`, which supplied the gate-1 measurement, reads 9:50 / zframe 3768. Two to three
ticks of music phase move the gate, and the ring shows each page for four to five stream states.
Changing the block-1 clock to match `second1c` would move ALKU's verified frames.

**Measured difference:** **1,098 of 64,000** (1.7156 %), RGB and indices (`cargo xtask rediff --only
u2a`). `cargo test -p secondreality --test u2a the_saucer_frame_records_its_measured_divergence`
pins it, `the_block_one_clock_matches_second1_at_boot_5900` pins the clock, and
`cargo test -p secondreality the_reference_7300_is_the_measured_stream_state` shows 1,098 belongs to
state 297 and to no other of the 522.

**Status:** carried: the session's phase (D17), which the oracle's API does not reach.

## A30 — U2E: the release's Gouraud shading differs at the four keyframes

**Part:** U2E (`VISU/C/U2E.C` over the VISU engine). Registered, not adopted.

**Original:** at the reference's own stream state (the port's `K − 4` frame, the state search's
unique minimum for every keyframe) the silhouettes are exact, and the differences are confined to
the Gouraud faces of `s01` (123 of its 124 faces are Gouraud), `Tree01g` (64 of 70) and `Car02`
(all 10).

**Port:** the tree's algorithm, the same engine as U2A's.

**Why:** A28's case: the engine reproduces the flat faces and the silhouettes byte for byte, and no
tree-carriable constant turns a whole object's Gouraud interpolation into the release's, whose U2E
the tree does not hold in a readable form.

**Measured difference:** **189 / 7,118 / 231 / 1,177** of 64,000 pixels (0.2953 / 11.1219 / 0.3609 /
1.8391 %) at 1000 / 1600 / 2600 / 3600, compared at `K − 4`; the index deltas run from −8 to +12 and
are mostly +1 (6,655 of 7,118 at 1600). `cargo xtask rediff --only u2e`; the state search is
`cargo test --release -p secondreality --bin secondreality search_reference_states -- --ignored
--nocapture`;
`cargo test -p secondreality --test u2e
the_keyframes_record_the_measured_session_phase_and_gouraud_residual`.

**Status:** carried: the release's Gouraud constants are in an image the tree does not hold.

## A31 — U2E: the block-4 keyframe boot indices are a session-phase intercept

**Part:** U2E (`VISU/C/U2E.C`).

**Original:** the filed capture displays stream state `(t − 702)/2` at boot `t`: states 149 / 449 /
949 / 1449 at 1000 / 1600 / 2600 / 3600.

**Port:** `(t − 698)/2`, the same slope to the frame with a four-tick (two-state) intercept.

**Why:** D17. The port's music clock reaches order 19 at boot 698, where the filed `second4` trace
reads it at 699, and the keyframe capture is another session: A29's class. The intro is exact
against the sessions that agree with it (boot 460..660 at 0), which pins the 418 anchor and the tick,
and moving the block-4 start or the gate would move it.

**Measured difference:** **11,006 / 11,805 / 17,278 / 16,234** of 64,000 pixels (17.1969 / 18.4453 /
26.9969 / 25.3656 %) at the exact keyframes, with the aligned-state residual A30's
(`cargo test -p secondreality --test u2e
the_keyframes_record_the_measured_session_phase_and_gouraud_residual`).

**Status:** carried: the session's phase (D17).

## A32 — U2A: the pre-gate hold shows the normal landscape, not the 30,000-cycle reference's neon

**Part:** U2A (`VISU/C/U2A.C`). A registered divergence, adopted on the user's decision.

**Original:** the tree draws nothing before its first music gate (`U2A.C:229-240`). The release
shows the `U2ABG.UH` picture from the part's start to the gate, about 13.5 s, under a palette that
depends on the emulated CPU speed. At **30,000** cycles (`reference/parts/u2a/frame_006500.rgb`,
sha256 `11a66eb0…`) it is a neon yellow-green, the background palette through `(3v + 5) & 63` with
entry 195 black. At **15,000, 25,000, 35,000 and 60,000** cycles the same recipe gives `0a68ffb9…`
at boot 6400 and 6600 every time (`reference/parts/u2a-speed/`, `reference-indices.md` §27), which is
the picture under the merged palette (`U2A.C:152`): the port's own post-gate frames (7000, and PAM's
held 7600/7640) are the same bytes. The neon's mechanism is not traced.

**Port:** `init` paints page 0 with the `U2ABG.UH` payload and uploads the merged palette for the
whole hold, so the hold, the first draw and PAM's hold share one DAC.

**Why:** four speeds of five show the normal landscape, so the neon is a timing race the 30,000-cycle
reference happens to hit, not a stable property of the release. The user chose the normal
landscape. This is a divergence from that one capture, not a claim that it is wrong at its speed.

**Measured difference:** **0** against all eight `u2a-speed` frames
(`cargo test --release -p secondreality --test u2a
the_pre_gate_hold_compares_at_zero_with_the_four_speed_captures`;
`the_pre_gate_hold_is_the_four_speeds_captures`), and **64,000 of 64,000** against the 30,000-cycle
6500, where every pixel's palette differs (`the_30000_cycle_neon_6500_differs_by_the_registered_count`;
`cargo xtask rediff --only u2a`). In the full run, 6500 is 64,000 and U2A's first frame, 5961 paired
at 5956, is 60,160 (`cargo xtask rediff --only fullrun/02-u2a-pam`; A53).

## A33 — U2A: the first draw uses `fov = 0` for the C's uninitialised variable

**Part:** U2A (`VISU/C/U2A.C`). A substitution with no measured difference.

**Original:** `int fov;` is declared inside the outer loop (`U2A.C:261`) and read by
`vid_cameraangle(fov)` (`:287`) before the first parse assigns it (`:348`). Its value is whatever
the stack slot held.

**Port:** `Driver::new` starts with `fov = 0` (`parts/visu/mod.rs`). The stream's `0x22` (`0x2200`)
takes over from the first parsed frame.

**Why:** every object is off on the first draw (`U2A.0AB` frame 0 has no toggle; object 2 turns on
at frame 16), so the camera angle multiplies nothing and the frame is the cleared background
whatever the angle is. 0 is the deterministic choice, not a guess at the release's stack.

**Measured difference:** **0 of 64,000** (`cargo test -p secondreality --bin secondreality
the_first_displayed_page_is_the_cleared_background`, under both palettes;
`the_first_stream_frame_leaves_every_object_off_and_frame_sixteen_turns_object_two_on`).

## A34 — The vector engine fills a linear page, not the original's planar VGA memory

**Part:** U2A and U2E (`VISU/AVIDFILL.ASM`, `ADRAW.ASM`, `ADRAWCLP.ASM`). A substitution with no
measured difference.

**Original:** the polygon pipeline emits a `{FILL-DATA}` stream, and a planar filler writes it in
unchained 320×200: a row is 80 addresses, an address holds four pixels, one per plane, selected by
the map mask.

**Port:** the span generators emit the same bands, and `write_address` expands each planar byte write
to the pixels `4 × address + plane` for each plane the mask selects, in a linear 320-wide page. The
tiers' quantisation, `AFILLDIV.INC`'s `65536/len` slope, the `0x8000` X bias, the row loop and the
right-exclusive stop are the source's (`parts/visu/draw.rs`).

**Why:** the surface is a linear indexed image everywhere else in the engine. What is preserved is
the pixel set and its colours, which is what the reference compares.

**Measured difference:** **0** on every shipped comparison. U2A's 6500-hold and 7640 are 0, U2E's
intro is 0 at nine indices, and the Gouraud residuals A28/A30 have exact silhouettes, so they are
colour deltas on faces the pipeline drew. `cargo test -p secondreality --bin secondreality
parts::visu::draw`; `cargo test -p secondreality --test u2a`; `cargo test -p secondreality --test u2e`.

## A35 — The four VGA pages are packed at 64,000 pixels, not the original's 65,536-pixel slots

**Part:** U2A and U2E (`VISU/C/U2A.C:105-166`, `U2E.C:115-183` over `AVID.ASM:341-358`). A
substitution with no measured difference.

**Original:** `vid_setswitch` gives page `p` the segment `0xA000 + p*0x400` and the display start
`p << 14`, a 0x4000-byte slot: 65,536 linear pixels apart, a 64,000-pixel picture and a 1,536-pixel
gap.

**Port:** the four pages at `page * 64_000` pixels (`PAGE_PIXELS`), with no gap. Every page is
cleared, drawn and read through its own origin. U2E's `fadeset` writes the same plane addresses into
each page, and the port's page bound (16,000 addresses) never fires for the shipped pattern, whose
largest address is 15,999 (`u2e.md` §4).

**Why:** the pages are a linear surface, not VGA memory, and a gap would have to be modelled in every
page slice for nothing.

**Measured difference:** **0** on every shipped comparison, as A34's. The reference's U2A keyframes
are page 0's picture at origin 0, the one origin both layouts share.
`cargo test -p secondreality --bin secondreality the_fadeset_writes_the_source_s_pattern_within_its_page`
pins the fadeset's expansion and bound.

**Status:** carried: U2E's keyframes are RGB-only, so its page origins are not established, and the
slot gap is memory no compared frame shows.

## A36 — GLENZ: the block-2 keyframes are five ticks late, and the reference capture's DAC leads its surface

**Part:** GLENZ (`GLENZ/MAIN.C`), in block 2.

**Original:** the music gate (`MAIN.C:290`, `musplus < -19`) exits at boot 1111 in
`reference/music/second2b`, and the intro from the reset to main-loop frame 44 is **495** ticks
(first object pixels at 1606, so `L_r = 1562`). The filed moving keyframes 2300 / 3000 / 3400 carry a
surface/DAC phase: their surfaces lag the counter by 0 / 1 / 1 frames, and in each the DAC is one
frame ahead of the surface, D17's sample/publish instant (`glenz.md` §11.1; the pictures re-capture
byte for byte, `reference-indices.md` §7.4).

**Port:** the gate exits at 1117, and the intro is **494** ticks (first object pixels at 1611), so
`L_p = 1567` and the aligned index is `K + 5`. The port pairs surface *N* with the palette after
*N − 1*, the source's upload-before-work order.

**Why:** D17, A29/A31's class: the port's player reaches zframe 300/333 one tick earlier relative
to the reset than `second2b`, and the rest is the capture's sampling instant. Nothing was fitted.

**Measured difference:** at `K + 5` (2305 / 3005 / 3405) **5,208 / 3,114 / 5,261** of 64,000 pixels.
At the surface minima (2305, 3004, 3404) **5,208 / 2,289 / 5,941**, which `cargo xtask rediff --only
glenz` registers (A37). At `K + 6` 9,777 / 10,806 / 12,001. `cargo test -p secondreality --test glenz
the_loop_start_is_1567`; `cargo test -p secondreality --test glenz
the_moving_keyframes_record_the_registered_alignments_and_residuals`; `glenz.md` §11.

**Status:** carried: the session's phase and the capture's sampling instant.

## A37 — GLENZ: the reference identifies the low-byte colour key, and the surface minima's residual is unexplained

**Part:** GLENZ (`GLENZ/VEC.ASM:403-506`).

**Original:** at boot 2305 (the filed 2300's surface, main frame 738) the 5,208 differing pixels are
exactly palette entries 0..15, and none has an index of 16 or more: the reference's colours are the
fade at `b = 26` and the port's at `b = 27`, the capture's one-frame DAC lead (A36), not a colour
model. The tree's `demo_glz` keys `rolcol` by the full colour word for visible faces (`VEC.ASM:450`)
and by the low byte for hidden ones (`:417`). With the 256-byte table those maps are independent, so
the slots fill and every post-exhaustion face takes slot 32 at DAC entry 0 and vanishes
(`glz_pool_exhaustion_uses_slot_32_and_entry_0`). The reference's picture identifies the in-bounds
low-byte key on both paths: one `slots: [u8; 256]`, with a hidden face freeing the slot its visible
face took.

**Port:** the low-byte key on both paths (`glenz::poly::tests::glz_hidden_face_frees_the_visible_slot`).

**Why:** D24 step 3: the key is one expression the tree's algorithm can carry, and the picture is the
evidence. With it, every object pixel at 2305 is byte-identical (the object box 9,989 px). The same
`INIT_320X200` constants and the 320-pixel-row model reproduce the 2300 object pixels, so no shipped
constant is adopted.

**Measured difference:** **5,208 / 2,289 / 5,941** pixels (8.1375 / 3.5766 / 9.2828 %) at 2300 / 3000 /
3400, at the surface minima (`cargo xtask rediff --only glenz`). At 3000 and 3400, **711** and **839**
of the 35,896 and 43,938 object pixels are not explained by the index structure (710 of 711 at
indices below 16 at 3004, all 839 at 3404): the second object's interior XOR boundaries, its outer red
silhouette matching within 118 / 180 px. No constant, shade or offset resolves them without breaking
the exact 2305 match. `cargo test -p secondreality --test glenz
the_moving_keyframes_record_the_registered_alignments_and_residuals`.

**Status:** carried: the 711 / 839 pixels have only a hypothesis (a sub-pixel edge or phase). The
code that would settle it is in the release's `GLENZ.EXE` image (D49), not yet read.

## A38 — GLENZ: block 1's zoomer starts 6 frames after the reference's, the restarted clock's residual

**Part:** GLENZ (`GLENZ/MAIN.C:290`) through the shell's block-1 restart (`MAIN/U2.ASM:813-818`).

**Original:** `restartmus(1, 0, 48700)` fires between BEGLOGO and GLENZ. `second1d`'s rows put the
restart at **8,056** (`crates/sr-audio/tests/trace.rs`'s `RESTART_TRACES`, derived by
`block_ones_music1_restart_takes_order_0s_break`). The WaitMusic exit is **8,521**: `second1d` reads
order 2 row 45 there, and the original's picture first differs on 8,522 (D17's sample/publish
instant).

**Port:** the restart installs MUSIC1@0 at the BEGLOGO→GLENZ boundary, **8,057**, with D34's
four-tick `waiting`, and the new player continues from MUSIC0's channels (`Player::restart_from`,
D45): channel 2 is still at volume 63, so MUSIC1's order 0 takes its row-3 `C00` and lasts 4 rows,
and the restart publishes `1:0` at 8,087. The wait ends on the first published `2:45`, and the first
zoomer body is at **8,527**, 470 ticks after the boundary.

**Why:** the residual is the restarted clock's. The port's clock begins at 8,061, its boundary plus
D34's four ticks, which are the restart recipes' boot relation (404 → 408), not a measurement of a
mid-run reload (A53). The run is +5 against `second1d` at its first and last row changes, and the
picture's exit is the same +5 (8,522 → 8,527, `FULLRUN_SEAMS`' GLENZ row); the 6 is that plus the
trace's sample-to-picture frame. 5 is within A12's 7-frame inter-session spread but outside the
shell's 3-frame `JITTER_FRAMES`, so it stays registered.

**Measured difference:** **8,527 − 8,521 = 6 frames** at the wait's exit. `cargo test -p
secondreality --test shell a_block_one_run_is_alku_then_u2a_then_pam_in_the_boots_numbering` pins
8,057, `1:0` at 8,087, 8,527, 470 and 6; `the_block_one_run_restarts_music1_at_glenz` pins the
install, `waiting == 4` and the carried channels; `cargo test -p sr-audio --test trace
block_ones_music1_restart_takes_order_0s_break -- --nocapture` derives 8,056; and
`awk -F, '$1 == 8521' reference/music/second1d/trace.csv` reads `2:45`.

**Status:** carried: a second restart constant for a reload the port does not have would need its
own measurement (D24, A53, A55).

## A39 — GLENZ: the span compositor repaints every span instead of walking the previous frame's list

**Part:** GLENZ (`GLENZ/NEW.ASM:66-172`, `ng_pass3`). A substitution with measured difference 0.

**Original:** the screen is not cleared between frames. `ng_pass3` walks the previous frame's
boundary list and the new one together and repaints `[prev, position)` only where the two
accumulated colours differ. Every span's pixels are `colour | bgpic[pixel]`, so a span whose colour
did not change already holds the bytes a repaint would write.

**Port:** `span.rs`'s `Compositor::paint` repaints every span from `bgpic`, never clearing the
buffer.

**Why:** no pixel differs, and the port drops a stateful walk it would otherwise carry between ticks.

**Measured difference:** **0**: the test module keeps the source's two-list walk beside the port and
compares whole 64,000-byte frames over a 13-frame corpus (twelve generated frames of moving, rotating
and held polygons and an empty one, including the sentinel pixel 63,999):
`cargo test -p secondreality --bin secondreality
parts::glenz::span::tests::the_full_repaint_equals_the_source_two_list_walk`.

## A40 — GRID: the developer pause is a start event, not a key wait

**Part:** GRID (`GRID/MAIN.C:144-153`), in the vault. A substitution.

**Original:** after the eye is shifted and copied and 1,024 points are plotted in colour 15,
`getch()` (`MAIN.C:153`) blocks until a key. The capture (`tools/vault_capture.sh grid`,
`--keys KBD_space`) delivers exactly one `KBD_space` there. The dots are overwritten by the eye copy
at `:156`. The loop's other key-sensitive calls are `dis_exit()` (`:158`) and `dis_muscode(3)`
(`:160`), which the standalone DIS answers 0 until a consumed key.

**Port:** `Grid::init` performs the source's loop iteration 0, the frame the capture files as the
start-up transition (reference 352), and the first composed frame is iteration 1, reference 353
(`K0`). No key is waited on, `dis_muscode(3)` is not called (the port's `muscode` is 0 with no
player, A56), and the dot screen is not drawn.

**Why:** the one start key selects the state the port starts in, and the dots and the transition
frame are overwritten before the first compared frame.

**Measured difference:** not separately measurable: the five compared frames carry A41's sampling
residual and nothing of this substitution. `cargo test -p secondreality --test vault
grid_matches_the_reference_at_its_measured_start`; the capture command is in `grid.md`.

## A41 — GRID: the filed RGB frames catch the newest line mid-draw in its top rows

**Part:** GRID, in the vault.

**Original:** the screen moves during the indexed read, so the oracle keeps the RGB frame (D15, D19),
and `reference/parts/grid` holds five `.rgb` files. At each keyframe the emulator published its frame
while the original was still drawing the top of the current line, so the newest line is missing
from, or one red rank lower in, rows 14–15.

**Port:** the part composes the completed frame after the whole loop body (`Grid::frame`).

**Why:** a sampling instant, not a model error. Every other pixel matches (the eye with its 100
unshifted bytes, the three line colours, the palettes and the fill-word artefacts), and the residual
is an order of magnitude below the ±1 neighbours' (about 10,000–12,000 pixels, printed by the test).
Nothing was changed to reduce it.

**Measured difference:** **6 / 2 / 36 / 33 / 14** pixels at 400 / 500 / 600 / 700 / 800, all on rows
14–15, max channel delta 41, the minimum at offset 0 for all five (`cargo xtask rediff --only grid`;
`cargo test -p secondreality --test vault -- --nocapture
grid_matches_the_reference_at_its_measured_start`, which asserts every differing row is 14 or 15).

**Status:** carried: the filed frames are published mid-draw, an instant the port's completed frames
cannot reproduce.

## A42 — TWIST: the copper test is not registered

**Part:** TWIST (`TWIST/ASM.ASM:220-269`), in the vault. A substitution with measured difference 0.

**Original:** `main` calls `initcoppers` (`TWIST/MAIN.C:89`), which registers `testcopper` in DIS
service-7 slots 0 and 1: it sets DAC entry 0 white, spins 100 `loop` iterations and sets it black.
The standalone `DIS/DIS.EXE` the vault's recipe uses is built without `INDEMO`, and its service 7 is a
bare `ret` (`DISINT.ASM:294-316` is inside `IFDEF INDEMO`; the image's `setcopper_7` at 0x2f7 is a
`ret`), so the reference shows no flash.

**Port:** `Twist::init` registers no copper callback.

**Why:** registering it would invent a flash the artifact cannot produce, and the engine's palette
timeline could not express an intra-frame band without an unmeasured duration.

**Measured difference:** **0** at all six keyframes (`cargo test -p secondreality --test vault --
--nocapture twist_matches_the_reference_at_its_measured_start`).

**Status:** carried: the standalone DIS has no copper service, so no capture can show the flash.

## A43 — TWIST: any-key exit and the Scroll-Lock border become ESC/Ctrl

**Part:** TWIST (`TWIST/MAIN.C:21-28,49,102`), in the vault. A substitution with measured difference 0.

**Original:** `doit` exits on any pending BIOS key (`kbhit()`), and `main` consumes it with `getch()`.
`waitb` tests keyboard-flags byte `0000:0417` bit 4 (Scroll Lock, despite the `shiftstatus` name)
and toggles the overscan colour around the retrace wait. The capture sends no key, so neither path
runs.

**Port:** the part ends on `ctx.exit_requested`, which the shell sets for ESC and Ctrl only; the
border stays black (`view.border = Some(0)`).

**Why:** the port's input model, the same for every part: nothing waits on a key. The original's
"any key" is wider than ESC/Ctrl, which this entry records.

**Measured difference:** **0** in all six compared frames and in the 4,444-frame run
(`twist_matches_the_reference_at_its_measured_start`; `twist_draws_exactly_4444_frames_and_holds`).

## A44 — TWIST: one 70.086 Hz tick stands for one VGA retrace

**Part:** TWIST (`TWIST/MAIN.C:21-28,49`), in the vault. A registered mapping with no overrun model.

**Original:** the source has no timer. `waitb` polls input status 0x3DA for the start and end of
vertical retrace and returns 1, so `frame += waitb()` counts retraces. `inittwk` leaves mode 13h's
vertical timing alone, whose nominal rate is 25.175 MHz / (800 × 449) = 70.086 Hz.

**Port:** one tick is one `doit` iteration, one displayed frame, at the fixed 70.086 Hz
(`crates/sr-engine/src/tick.rs`). A draw that overran a frame would slow the original below 70.086 Hz
while `frame` still counted one per iteration; the port does not model that, and the source cannot
observe it either. The 4,444-iteration cap is 63.41 s nominal.

**Why:** the retrace is the source's clock, and the fixed tick is the port's model of it for every
part (`design.md` §6).

**Measured difference:** **0** per compared frame
(`twist_matches_the_reference_at_its_measured_start`; the cap `twist_draws_exactly_4444_frames_and_holds`).

**Status:** carried: a missed retrace would not change the original's frame count, and neither the
source nor the capture can observe it.

## A45 — TWIST: the comparison is RGB-only

**Part:** TWIST, reference and port. A registered limitation with measured difference 0.

**Original:** the filed reference (`reference/parts/twist`, six frames 103..603) is RGB-only. The
moving screen fails the indexed read's coherence check (D15, D19). An odd part frame draws page 2 at
pixel 131,072, and the oracle's indexed read always starts at plane offset 0, so even a still frame
would read page 1 there; the six keyframes are even frames.

**Port:** the test dumps the port's `.rgb` at `R − K0 − 1`, `R − K0`, `R − K0 + 1` and compares RGB.

**Why:** the limitation is the oracle's, and the RGB comparison is exact.

**Measured difference:** **0** at all six keyframes, offset 0
(`twist_matches_the_reference_at_its_measured_start`; the capture command is in `twist.md`).

## A46 — HARD: the start gate is the capture's one key, and the 0xff cut is unreachable

**Part:** HARD (`HARD/B.C:138`, `HARD/BP.C:263`), in the vault. A substitution with measured
difference 0.

**Original:** the standalone DIS's `muscode_6` returns 0 until `checkkeys` consumes a key, then
echoes the caller's code once (`DIS/DISINT.ASM:242-282`, `:154`). `B.EXE` opens with
`while (dis_muscode(1) != 1 && !dis_exit());`, and part 3 checks `if (dis_muscode(0xff) == 0xff)
NEXTMODE` each frame, so a second key would cut part 3 early. The capture delivers exactly one key.
The source polls `dis_exit` at two places only: the 70-wait loop (`B.C:162`) and the main loop's
condition (`B.C:203`).

**Port:** the part starts at `init` with the gate passed and never models the echo; the `0xff` cut
cannot fire (`the_0xff_cut_is_unreachable_with_one_start`, on the full run, where part 3 ends at its
own count 800). The exit flag is polled at the source's two points, and the letterbox still runs
after a flag set during the wipe or the fill
(`the_exit_flag_is_polled_where_the_source_polls_it`).

**Why:** a literal `muscode(1)` wait would hang against the port's `muscode` of 0 (A56), as it does
against the reference's. The key is a start event before `K0`, so no compared frame sees it.

**Measured difference:** **0**: the nine keyframes 480…1600 are exact and 840's `.idx` is byte for
byte (`cargo test -p secondreality --test vault -- --nocapture
hard_matches_the_reference_at_its_keyframes`).

## A47 — HARD: one 70.086 Hz tick stands for one waitb, with the display phase measured

**Part:** HARD (`HARD/B.C:203-324`, `BP.C:29`), in the vault. A registered mapping with measured
difference 0.

**Original:** the source has no timer; every loop iteration ends in `dis_waitb` (`B.C:207`). The
display runs one retrace behind the logic. `part1init` calls its own `waitb` (`BP.C:29`), so mode 1's
count 0 takes two display frames, and mode 4 uploads its band before the switch rebuilds it, so a
frame shows the previous iteration's band.

**Port:** one tick is one source iteration at the fixed 70.086 Hz. The port's frame `t` is the
capture's `356 + t` (`K0 = 356`): a 119-tick pre-roll (20 wipe steps, 28 fill iterations, 70 waits,
the letterbox), `part1init`'s extra display tick, and the palette one iteration behind in mode 4 and
in the same iteration in part 1. A retrace overrun inside a mode is not modelled; the one the capture
shows is A48's.

**Why:** the retrace is the source's clock, and the fixed tick is the port's model of it
(`design.md` §6).

**Measured difference:** **0** at all nine keyframes 480 / 560 / 630 / 700 / 770 / 840 / 910 / 1050 /
1600, offset 0, and 840's index plane (`hard_matches_the_reference_at_its_keyframes`;
`cargo xtask rediff --only hard`). The tick counts are pinned by
`parts::vault::hard::part::tests::the_pre_roll_is_the_sources_tick_counts`,
`the_first_half_mode_sequence_is_the_sources` and `the_full_mode_sequence_is_the_sources`.

## A48 — HARD: the mode-5 boundary's two displays are inserted as static ticks

**Part:** HARD (`HARD/B.C:242-293`, `HARD/BP.C:124-167`), in the vault. A registered divergence with
measured difference 0 on every compared frame.

**Original:** mode 4 is 102 iterations, so with its count 0 at reference 851 the source's part-2
count 0 would be 953. The artifact's is **955**: a contiguous capture of 950–960 shows 950–955
byte-identical (the mode-4 band, with the trail's index-189 last pixels white), and 956/957 replacing
exactly those 100 pixels with `(4,255,255)`. That colour is entry 65 of `part2init`'s gradient
(`BP.C:135-154`), which every point of the first grid frame's `x = -500` face projects to
(`depthcol[3000] = 65`; `the_first_grid_frame_is_the_shipped_grid1_list`). The count-0 frame looks
like the mode-4 hold because `part2init`'s `pal[1]` is white like index 189.

**Port:** two display frames with no loop iteration between mode 4's advanced tick (port 596,
reference 952) and part 2's count 0 (port 599, reference 955). The inserted pictures are the static
mode-4 band, which is what the artifact shows at 953/954.

**Why:** the two displays are the shape of a missed-retrace overrun (D22), but not at mode 4's count
0, where 851/852 match at 0. The candidate is the 768-byte palette upload at the top of a later mode-4
iteration (`B.C:208`). The capture measures the total cost, two displays, not its position, and no
keyframe depends on the position (1050 and 1600 are exact).

**Measured difference:** **0** at references 950–960 and 2245–2257, offset 0 (the port's frame is
reference − 356): capture with `tools/vault_capture.sh hard --frames "$(seq -s, 950 960)"` (and
2245–2257), dump the port's with `--part hard --headless --dump-frames`, and compare frame by frame.

**Status:** carried: where inside mode 4 the overrun falls is not measured; no keyframe depends on it.

## A49 — HARD: the optional argument, its debug strip and the border are not ported

**Part:** HARD (`HARD/B.C:187-201`), in the vault. A substitution with measured difference 0.

**Original:** `B.EXE 1|2|3` selects `dots4` or jumps to part 2 or 3, writes a 256-byte debug strip
into the top two rows and sets `border = 1`. The code right after overwrites the strip with 255 and
sets `border = 0`, so a normal run's border is black and the strip is the letterbox's row 0/1.

**Port:** no arguments; the rows are 255 and the border is `Some(0)`.

**Why:** the capture's `RUN.BAT` runs `B.EXE` with no argument, and the argument's visible effect is
undone at `:200-201`. The phase-skip paths are a developer shortcut.

**Measured difference:** **0**: rows 0/1 are 255 in the capture's 474–955 frames and in the port (as
A47).

## A50 — HARD: two one-byte data questions are recorded, not fitted

**Part:** HARD's build-time data (`HARD/BDO.C`, `HARD/_GRID1.TMP`, `HARD/B.EXE`), in the vault.

**Original:** the four BDO streams are linked into `B.EXE` (`LINK.TMP:1-3`). Bands 0 and 3 match the
extraction byte for byte at 55,875 and 64,977. Bands 1 and 2 differ in their last terminator byte only
(56 and 72 for 0), where the next object's bytes abut. `_GRID1.TMP` matches `B.EXE`'s table at 69,265
for 399 of 400 bytes; the last is `0x4F` in the binary and `0x00` in the file.

**Port:** the streams extracted from `assets/HARD/FCRZ.U` by `BDO.C`'s rule (terminator 0), and
`GRID1` from `_GRID1.TMP` (`the_dot_streams_are_the_bdo_extraction`,
`the_grid_table_is_the_shipped_grid1`).

**Why:** neither byte changes a drawn pixel: the terminator is never read as a record (the spawner
resets on a zero value), and the grid word is read only when `doit2` reaches the 100th pair.

**Measured difference:** **0** in the nine compared keyframes, none of which reaches the 100th pair's
y (`cargo test -p secondreality --bin secondreality vault::hard::dots`).

**Status:** carried: no compared frame exercises either byte.

## A51 — HARD: `do3dots`' out-of-array guard is a model for an unreachable branch

**Part:** HARD (`HARD/BA.ASM:221-222,262-270`), in the vault.

**Original:** `_depthcol db 8192 dup(0)` sits in the code segment with 0..3,099 written
(`BP.C:128-133`). `do3dots` loads z's low word into `bp`, signed-skips values above 3,000
(`BA.ASM:233-235`) and reads `cs:_depthcol[bp]` with a 16-bit address, so a negative low word would
wrap to a code byte before the table. The second clear is two word stores at `[si+12]`/`[si+12]+320`
(`BA.ASM:229-231`), and the code that writes `[si+12]` is unreachable behind `jmp @@4` (`:235`).

**Port:** colour 0 for an unsigned index past the table, and a one-byte-per-row clear (0 and 320),
which differs only in bytes 1/321 at offset 0. Both are models of unreachable branches; the port has
no link map for the code bytes.

**Why:** nothing in the reachable domain reads them. Over part 2's 501 states the projected z′ is
1139..3964, every drawn point's low word is 1139..3000, and the low word is always positive.

**Measured difference:** **0** in the reachable domain; the unreachable byte is not reproduced.
`cargo test -p secondreality --bin secondreality
the_shadow_grid_dot_skips_positive_z_and_models_the_negative_word` pins the rule, and
`the_part2_grid_z_range_and_drawn_words_are_the_measured_domain` the domain.

**Status:** carried as an unreachable path: the code byte is the link map's, which the tree does not
give.

## A52 — The vault's silent run: no player, no music service, and one modelled start key

**Parts:** GRID, HARD and TWIST under `--part` and `--vault`. A substitution with measured
difference 0.

**Original:** the three effects run behind the standalone `DIS/DIS.EXE`, built without `INDEMO`: its
music service returns 0 until a consumed key and then echoes the requested code once
(`DIS/DISINT.ASM:242-282`, `:154`), there is no player, and the copper service is a bare `ret`
(`:294-316`). The captures are silent and deliver one `KBD_space` to GRID and HARD and none to TWIST
(`tools/vault_capture.sh`; `reference-indices.md` §24.4).

**Port:** no vault part installs a player (`music_for_part` returns `None`, no `boot_anchor`), and a
silent run refuses `--audio-dump`. The start key is modelled per part (A40, A46; TWIST takes none,
A43), and the later key cuts are unreachable. A `--part` run numbers its frames from zero.

**Why:** the artifact is silent, and the key is a start event before every compared frame.

**Measured difference:** **0** in all twenty compared frames (GRID 5, TWIST 6, HARD 9), and the
manifests carry `music: null`, `anchor: null` and `index_origin: "part"` (`cargo test -p secondreality
--test vault -- --nocapture`; `a_part_run_of_grid_numbers_itself_from_zero_and_is_silent` and its two
siblings for TWIST and HARD).

## A53 — The full run's seams: every part is handed over in one tick, and the loader's frames are not modelled

**Part:** the shell, between every two parts of a run (`--block 1` measured in full).

**Original:** each part is its own EXE. Between two of them `partexecute` loads the next
(`MAIN/U2.ASM:789-912`), and at two places it also reloads the music (`restartmus`, `:813-818` before
GLENZ and `:880-885` before U2E). Meanwhile the screen shows what video memory and the DAC hold: the
previous part's last picture, or at U2E JPLOGO's logo under a white DAC up to 34114
(`reference-indices.md` §26.3). A load's length is a disk and memory cost per EXE, not a constant of
the source.

**Port:** D31: the next part's first tick is the tick after the last one's, with the surface and the
player continuous. The two restarts install their new player at that boundary (D34, D45).

**Measured difference:** the seam table, the port's first frame minus the original's, by §26.3's
definition of a first frame (`xtask/src/rediff/table.rs`'s `FULLRUN_SEAMS`, which
`cargo test --release -p secondreality --test fullrun -- --nocapture` asserts against the run and
prints):

```
part        original  port   difference  change
alku            404    404          +0
u2a            5961   5956          -5      -5
pam            7707   7704          -3      +2
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

JPLOGO and U2E are aligned on bytes, because §26.3 can only bracket their first frames. From GLENZ
to JPLOGO the parts follow the restarted MUSIC1@0 session, 5 frames late at its first rows and 3
early at its last (A38); the MUSIC0@18 session after U2E is 8 frames early at its first row and level
at its last. U2A's −5 is ALKU's release loop and closing routine (A20). BEGLOGO's −37 is A5's
lead-in (the port's 7930 is the original's byte for byte). U2E is 45 frames early because the port
starts it at the restart's boundary, where the original's reload holds JPLOGO's whitened screen.
ENDSCRL's −69 is CRED's span being 69 frames shorter, 7,625 against 7,694 (A11). After the cue wait,
MNTSCRL's first frame follows after 7 frames in the original and 1 in the port.

**Why the loader is not modelled:** D31's question, decided under D24. One `LOADER_TICKS` held between
parts would have to explain every seam the music does not decide, and the changes take both signs.
The table's change column is positive at eight seams, where a delay at a handover could only make
the port later still, and negative at the other ten. The two seams the music pins on both sides would need different constants: 6 frames after
the cue wait, and 59 to 68 under the U2E reload (`reference-indices.md` §26.6). A constant per seam
is forbidden (D24).

**The filed frames.** Each `reference/parts/fullrun` frame is rendered at its reference index plus its
part's difference, and the three frames the original shows during a loader stretch (7920, 16636,
34110, `FULLRUN_LOADER_FRAMES`) at the port's last frame of that screen (7884, 16632, 34042).
`cargo xtask rediff --only fullrun` compares all 60 (44 RGB, 16 index planes) at their registered
counts (`reference-indices.md` §26.8 lists them). Every held frame is 0 in RGB except U2A's 6500
(64,000, A32), A25's 19000 (2,058) and 34110 (72,363: the reload's white, which the port never
shows; its index plane is 0). The moving frames registered here are 5961 (60,160: U2A's opening, the
same picture as the original's in rows 0–179 under another DAC, A20, A32), 7930 (81,273, at BEGLOGO's
−37), 11159 (192), 11900 (4,336), 16193 (108,923, PANICEND's squashed troll), 18656 (129), 24500
(45,808), 29174 (875), 31272 (4) and 31901 (33,554, index plane 34,036; 3DSINFLD draws every second
frame). Three index planes under a 0 RGB are memory no screen shows: 7880 (48,786, PAM's picture
under its white), 24540 (48,384, the cube screen under black) and 37970 (46,997, U2E's leftover video
memory under ENDLOGO's white).

**Status:** carried: the loader's frames, whose per-seam cost is the session's disk and memory time;
and PANICEND's first frame (108,923 px at 16193), which depends on the screen and DAC TECHNO hands
over (as A63 at block 2's 8792/8800) and which no capture brackets.

## A54 — Block 1's two long traces: `second1e`'s placement drift and `second1f`'s speed-1 row

**Part:** the music in block 1 after GLENZ (`reference/music/second1e`, 8,800…33,949, MUSIC1) and
after U2E (`second1f`, 34,105…48,276, MUSIC0 from order 18).

**Original:** the emulated tick clock and frame counter (A12), sampled once a frame over windows four
and two and a half times `second1`'s. In `second1f`'s order 26 (pattern 71, frames 38,328…38,370) the
module runs at speed 1, a row every 1.46 frames, so the trace publishes row 25 at 38,364 and row 27 at
38,366 and never row 26.

**Port:** the exact tick clock (A12). Its phase in `second1f` is about ten frames later, so it
publishes row 26 at 38,376, a frame on which that row is current and which the trace never sampled.

**Why:** the drift is A12's inter-session fact over a longer window, and the unsampled row is a
sampling fact: a row shorter than one sample cannot be published by a trace that samples once a
frame. A player that skipped row 26 would be wrong. Neither is a seam or the restart (A38, D45).

**Measured difference:** `cargo test -p sr-audio --test trace -- --nocapture the_port_reproduces`
holds both traces to `KNOWN_DISAGREEMENTS` exactly, each simulated through the session its restart
replaces (D45). `second1e`: content identical in every word, worst displacement **8** (`JITTER_FRAMES`
7), first reaching 1 at 8,813, 4 at 16,190 and 8 at 33,305. `second1f`: one content difference
(`row`, index 502, trace 27, port 26), on one frame, 38,376, and worst displacement **12**. The block-1
run against them (`cargo test --release -p secondreality --test fullrun -- --nocapture`): `second1e`
18,928 of 25,143 frames within 3 (75.281 %) at +5, worst 8; `second1f` 6,352 of 14,168 (44.833 %) at
−8, worst 12.

**Status:** carried: the drift is A12's inter-session clock, and the speed-1 row is shorter than one
sample.

## A55 — A mid-run restart does not carry the position ring or the tick countdown

**Part:** the player under the shell's three restarts: block 1's MUSIC1@0 at GLENZ
(`MAIN/U2.ASM:813-818`), MUSIC0@18 at U2E (`:880-885`) and the loop's MUSIC0@0 (`:929-932`).

**Original:** `restartmus` (`:348-397`) stops the song, reloads the module and calls `_zplaysong` on
the one player object (D45 lists what that keeps). The position ring (`+0x2150`) is primed only by
`_zinit` (`+0x2a8f`), every poll pushes and pops it (`+0x52c9`), and a stopped song writes `0xffff`
into the fed order word (`+0x4408`). So after the stop the ring publishes the old song's last
positions for the rest of its ten polls, then order −1, until the new song's first row comes
through. How far a reload gets is the session's: pass 2's ALKU reads the stale `(43,43)` in one loop
session of seven (D42, `reference-indices.md` §25.3). The tick countdown `[0xbe0]` runs on through
the reload (`+0x5157`).

**Port:** `Player::restart_from` carries the channels, the loop counter and `_np_zframe`, not these.
The new player's ring is fresh, so it publishes `0:0` from the restart until its first row comes
through, and its clock starts D34's four frames after the boundary.

**Why:** what the original puts in both words happens during the reload, and the port has none (A53).
A carried ring would publish the old song's last position at every restart, which is D42's minority
session, never the filed one. `cargo test -p sr-audio --lib
a_carried_ring_publishes_what_the_stopped_previous_player_would` shows such a player publishing, in
the loop's scenario, the stale `43:34` for 18 polls. `cargo test --release -p secondreality --test
loop a_carried_ring_would_hand_pass_two_the_old_songs_position -- --nocapture` shows the run
publishing `0:0` (`dis_sync` −15609) at pass 2's ALKU frames 8910…8920, so ALKU's first gate waits,
where `43:34` (`dis_sync` 10) would pass all four. The −1 itself cannot be published (the port's
order is a `u8`, and U2E's `order > 18` gate would read 255 as open); with the fresh ring GLENZ's wait
and U2E's gate make the decisions the −1 gives them.

**Measured difference:** **0** in every compared frame. No filed trace holds these frames (every
post-restart window opens after the new song's first rows), and `cargo test --release -p
secondreality --test fullrun`, `--test loop` and `cargo xtask rediff` hold every frame at its count.
The published position differs from the original's for 18 frames at GLENZ (8,057…8,074) and 22 at
U2E (34,043…34,064), printed by `cargo test --release -p secondreality --test fullrun
the_full_run_plays_every_part_in_the_originals_order -- --nocapture`, and for 23 at the loop (block-5
numbering 8,898…8,920), pinned by `tests/loop.rs`.

**Status:** carried: the ring and the countdown change during a reload, whose polls are the session's
(D42), and the port has no reload (A53).

## A56 — `dis_muscode()` returns 0: the `_np_zinfo` substitute

**Part:** the engine's music seam (`sr_engine::music::muscode_from`),
under every part that calls `dis_muscode()`. In the demo none does: the callers are the vault's
`GRID/MAIN.C:72`, `:160`, `HARD/B.C:138`, `HARD/BP.C:263`, and `ENDPIC/BEG.C:48`, the unshipped build
of part 4 (D8). A substitution with measured difference 0.

**Original:** `muscode_6` returns STMIK's `_np_zinfo` (`DIS/DISINT.ASM:242-282`, the load at `:249`),
a word nothing in either module produces (every `S` command is a standard `SDx`, D8).

**Port:** `muscode_from` returns 0 whatever the caller waits for, which is what the reference's word
held in every sample of every filed trace (`design.md` §7.4's substitute cue). The vault's parts run
behind the standalone DIS, whose `dis_muscode` is key-driven, and model their one start key instead
(A40, A46, A52).

**Why:** the measured value is the only one the port can stand behind; the word's other values would
be a cue no frame supports. The substitute is one function body.

**Measured difference:** **0** in every compared frame, because no part the demo runs calls it; and
`cargo test -p sr-audio --test trace zinfo_is_zero_in_every_filed_trace` asserts `zinfo` 0 at every
sample of every trace under `reference/music/`.

**Status:** carried: `_np_zinfo`'s non-zero values occur in no trace, and its callers are unshipped or
run behind the standalone DIS.

## A57 — DDSTARS: the star clip is assembled out, and the port does not clip

**Part:** DDSTARS (`DDSTARS/STARS.ASM`), the hidden part (`--block U`).

**Original:** `risetext` still computes the clip window `_nostar1`/`_nostar2` (`STARS.ASM:436-445`,
`:525-526`), but the guards that would skip a star row inside it are commented out at `:692-696`,
`:711-716`, `:737-742` and `:756-762`. The shipped build's frames show the stars in the window
(`ddstars.md` §6).

**Port:** no clip: the tree's assembled-out state, and the release's.

**Why:** D24: the source and the reference agree. The entry exists because the source reads as though
it clips, and a port that followed `_nostar1/_nostar2` would lose the band's star rows (`bugs.md` §27
keeps the behaviour).

**Measured difference:** **0 bytes** at all four keyframes 1900 / 3000 / 5000 / 8000, with the
reference's 1,025 star pixels in the window's rows at 1900 present in both
(`cargo test -p secondreality --bin secondreality the_star_clip_is_assembled_out_and_the_keyframes_hold_its_pixels`;
`cargo xtask rediff --only ddstars`).

## A58 — The shell's keys: Ctrl skips one part, and an ESC between two parts is kept where `flushkbd` drops it

**Part:** the shell, in a window (`bins/secondreality/src/window.rs`'s `EscapeKey`, D21). A
substitution with measured difference 0.

**Original:** `partexecute` (`MAIN/U2.ASM:205-215`) tests `ctrldown` before each part, and `ctrldown`
(`:185-203`) keeps its carry from the first time it sees the key, so one Ctrl skips every later part.
After each part it calls `flushkbd` (`:211`; `MAIN/U2A.ASM:537-546`), which discards up to 256 waiting
keystrokes, so a key pressed between two parts is thrown away.

**Port:** Ctrl drops the running part, and the run goes on with the next (D21). An ESC that lands
where no part is running is kept by `EscapeKey` and asked of the next part, or closes the window when
nothing is left to play.

**Why:** the port's window is inside a part, not in front of one, and it drains its events once a
tick, so a press between two parts would otherwise be lost to the port's own scheduling rather than to
the original's flush. Keeping it serves the "get me out" press that ends a block's last part (D21).

**Measured difference:** **0** in every compared frame: no capture presses Ctrl or ESC. The behaviour
is pinned by `cargo test -p secondreality --bin secondreality window::tests`
(`escape_asks_the_running_part_to_exit_and_ctrl_skips_it`,
`an_escape_between_two_parts_is_kept_and_then_closes_the_window`,
`an_escape_between_two_parts_is_asked_of_the_next_one`).

## A59 — A demo run's two text-mode screens: the settings screen for `START.EXE`'s menu, and the end box printed to the terminal

**Part:** a windowed demo run's start and end (`settings.rs`, `endscreen.rs`, D41, D44). A
substitution with measured difference 0.

**Original:** `START.EXE` runs `menu()` in text mode on every start but `/gravis`
(`MAIN/START.C:89-95`, `MAIN/MENU.C`). At the end `fatalexit` sets text mode, waits twenty retraces and
calls `printend`, which copies `U2END.BIN`'s 80×25 cells into text memory with the cursor on row 14
(`MAIN/U2.ASM:506-535`, `:965-974`). The exit stage reads no key (`:333-346`, `:969-973`).

**Port:** the settings screen is drawn in `ALKU/FONA.UH` on the port's own 520×325 frame, nothing
saved (D44). The exit stage's 64 + 20 ticks are the original's (D41). The end box's rows 0–13
(`0..CURSOR_ROW`) are printed to the launching terminal after the window closes, with SGR colours
when stdout is a terminal; rows 14–24 are blank in the shipped file, and DOS's prompt starts at row
14. An ESC during the exit stage closes the window at once.

**Why:** the port has no text mode, and the port replaces `START.EXE` (`design.md` §1). Both screens
are the port's own surfaces (D41, D44).

**Measured difference:** **0** on every compared frame: headless runs never show the settings screen,
and the end box goes to stdout. The box's characters and colours are the shipped file's
(`cargo test -p secondreality --bin secondreality endscreen`: `the_end_screen_is_the_shipped_box`,
`the_end_screen_colours_are_its_attributes`), the stage's ticks are
`a_demo_run_fades_then_blanks_then_ends`, and the screen is `settings::tests`.

## A60 — The enhanced sound option: the port's own renderer, off by default, the clock untouched

**Part:** the music's output (`sr_audio::Quality::Enhanced`, `--sound-quality enhanced`, D43).

**Original:** the object's mixer: one byte fetched a frame with no interpolation, through the 8-bit
output table, at 20,000 frames a second in the reference's stereo mode (A13, A14).

**Port:** the authentic renderer is the default and unchanged byte for byte. The enhanced one is
opt-in: the same voices and tick schedule, each authentic frame rendered as 2 or 3 interpolated frames
at 48,000 a second, converted with the curve's small-signal gain (`music.md` §13).

**Why:** a labelled option of the port's own, which claims nothing about the original (D43). It is
registered because a run with it sounds different from the original.

**Measured difference:** no picture and no published position moves: every word at each of 30,000
polls of both modules is the authentic schedule's (`cargo test -p sr-audio --test enhanced
enhanced_keeps_the_authentic_tick_schedule`), and the authentic stream is pinned to its digests
(`cargo test -p sr-audio --test mix the_authentic_stream_is_pinned_byte_for_byte`). The sound differs
by at most **0.089 %** in pitch over nine sustained notes and by **−0.18 dB** (MUSIC0) and **−0.57 dB**
(MUSIC1) in level over ten seconds (`cargo test --release -p sr-audio --test enhanced -- --nocapture`).

## A61 — Loop mode: the seam's residuals between CRED and pass 2's ALKU

**Part:** the shell's loop seam (`--loop`, D42; `MAIN/U2.ASM:919-933`), against `reference/parts/loop`
(`reference-indices.md` §25.3).

**Original:** in the loop capture's numbering (115 frames after the block-1 recipe's, §25.1), CRED's
closed iris `C` is 9029, the fade 9029–9092, ALKU's first pass-2 frame `L` 9104, the restarted song's
`(0,1)` 9112, ALKU's first lit frame in (10236, 10238] and its "A Future Crew Production" hold from
(10296, 10298].

**Port:** CRED shows its closed iris on two ticks before it retires (8833, 8834), so the fade is
8835–8898, then `LOOP_RELOAD_TICKS` = 11 and `L` at 8910. The restart publishes `(0,1)` at 8921, and
ALKU's fade-in is lit at 10045 and holds from 10106.

**Why:** each residual is a property of something the seam inherits, or a constant D24 does not let
the port fit. CRED's retirement tick is CRED's (the fade's end, 9092, is trace-measured, so the
two-tick tail is the port's CRED, D42). The restart recipe's 3 frames would be a second constant
needing its own measurement. ALKU follows the music. CRED's chain drift is CRED's span (A11, A53).

**Measured difference:** `cargo test --release -p secondreality --test loop -- --nocapture`
(`the_loop_seam_is_the_originals` pins each):

- **CRED's 2-tick closed-iris tail**: the fade and `L` are 2 frames late against `C` (`L − C` 77
  against 75), so the port's 8908 is CRED's 320×200 screen where the original's 9104 is ALKU's. The
  iris frames 9029 / 9055 / 9081 and ALKU's 9600 / 10419 / 11419 are 0 (`cargo xtask rediff --only
  loop`; 9104 is paired with the port's `L`, 8910).
- **Pass 2's music +5 frames** at `(0,1)`: 2 of them the tail's, 3 the restart recipe's.
- **ALKU's pass-2 fade-in +3…5 and its hold +4…6** frames against the original's two-frame brackets.
- **CRED's 81-frame chain drift**, which shifts every frame of pass 2 in absolute numbering
  (9029 − 115 − 8833 = 81).
- **Δ**: `L − 404` is 8700 if 404 is ALKU's first frame and 8698 if it is the mode set, so
  `LOOP_RELOAD_TICKS` is 11 or 9 (D42).

**Status:** carried, each for the reason under **Why**; Δ needs a reading of whether 404 is a mode
set, which the oracle does not give.

## A62 — Three parts' exits: ALKU's, MNTSCRL's and PLZPART's ESC paths

**Part:** ALKU (`ALKU/MAIN.C`), MNTSCRL (`FOREST/READ2.PAS`) and PLZPART (`PLZPART/PLZ.C`, `VECT.C`).
A substitution with measured difference 0.

**Original:** ALKU: the release's `main` calls its closing routine (image `0x13ce`) right after the
loop when `dis_exit()` is set (`0x1394`), and so do `fonapois`, `ffonapois` and every card gate
(`0x1c2e`, `0x2738`, `0xf30`). On an exit the routine sets `cop_pal` to `fade1`, which is black, waits
one copper frame, calls `close_copper` and `exit(1)`s (`0x13d4`-`0x1428`), so neither the final fade
nor `fonapois`'s text clear runs, and the screen's last frame is black (`alku.md` §12). MNTSCRL's first
wait, `READ2.PAS:153-159`, does not test `dis_exit`. PLZPART polls `dis_exit()` at every loop
boundary (`PLZ.C:68`, `:76`; `VECT.C:116-117`).

**Port:** ALKU returns `Finished` at the top of the tick that sees `ctx.exit_requested`, without
drawing (`alku.md` §6). MNTSCRL's wait ends on the tick an exit is seen (`mntscrl.md` §8). PLZPART's
`frame` never consults the flag, and the shell retires it at the end of the tick the flag was set in
(`Sequence::tick`, `plzpart.md` §10).

**Why:** ALKU's is the release's path but for its one black frame. MNTSCRL's source wait would hold
an ESC until the tree's `musplus >= 0` gate, and the port does not spin. PLZPART's ESC ends it on the
same tick with or without the poll, because the flag is sticky and the shell retires a part that
does not poll. (DIS's flag is sticky within a part too: ESC sets it, `DIS/DISINT.ASM:151-153`,
`exit_2` returns it, `:186-190`, and only `version_0`, `:90`, which `dis_partstart` calls, clears it.)

**Measured difference:** **0** in every compared frame: no capture presses ESC. The port's behaviour
is pinned by `an_exit_request_ends_every_wait_on_the_tick_it_is_seen` (ALKU),
`exit_requested_ends_the_wait_on_that_tick` (MNTSCRL) and the shell's
`the_exit_flag_asks_the_running_part_to_quit_and_the_run_goes_on`
(`cargo test -p secondreality --bin secondreality`).

**Status:** carried: the one black frame the release shows on ALKU's ESC path, which the port does not
draw, is unmeasured, because no capture presses ESC.

## A63 — PANICEND: a standalone run cannot answer the handoff frames 8792 and 8800

**Part:** PANICEND (`PANIC/SHUTDOWN.C`), `--part panicend` against `reference/parts/panicend`. A
registered limitation, 0 through the handoff.

**Original:** block 2's PANICEND starts over the screen and DAC TECHNO leaves, and its first filed
frames are states of that inherited picture: 8792 is the collapse's seven rows 97–103 of 234 pixels,
8800 the line, row 100 (`panicend.md` §1, §5).

**Port:** reached through the chain, it inherits the same. A standalone `--part panicend` run starts
on a blank surface and the default DAC, so its ticks 10 and 18, the mapping's numbers for 8792 and
8800, are a different picture.

**Why:** a part run has no previous part to inherit from. The handoff test drives the frames from
TECHNO's state instead.

**Measured difference:** through the handoff, **0**: `the_handoff_matches_the_collapse_8792_and_the_line_8800`
(`cargo test -p secondreality --bin secondreality parts::panicend`). Standalone, **1,638** and **234**
pixels, exactly the reference's non-black pixels (7 × 234 and 234): `cargo xtask rediff --only
panicend`, and the 4,914 and 702 bytes that
`the_standalone_run_cannot_answer_the_handoffs_8792_and_8800_and_says_so` pins (`cargo test -p
secondreality --test panicend`).

## Generated and self-modifying code

The design (`design.md` §5.2) named ten candidates, six generated unrolled loops and four
self-modifying sites. None is a registered difference: eight are bit-identical where the reference
can see them, and two are not on the shipped path.

| candidate | outcome | evidence |
|---|---|---|
| `GLENZ/ZOOMLOOP.INC` | not applicable: linked (`ZOOM.ASM:26`, `GLENZ/MAKEFILE:3`) as the body of `_zoom`, which only the unshipped `zoomer1` calls; the shipped `zoomer2` uses none of it | `glenz.md` §5 |
| `JPLOGO/ZOOM.INC` | bit-identical: the 160-entry dispatch and the row images are `DOL.C`'s formula | `the_zoom_window_is_the_source_bytes_at_each_scale`, `the_scale_dispatch_is_the_tables_and_the_reachable_range_stays_inside`; 17300/17500 at 0 (A27) |
| `TWIST/TWSTLOOP.INC` | bit-identical, consumed as data: its 16,000 texture bytes | `the_texture_table_is_the_shipped_twstloop`; six keyframes at 0 (A42–A45) |
| `GRID/LINEBLIT.INC` | bit-identical: the generator's XOR Bresenham against all 357 committed routines | `the_line_blit_is_the_committed_generated_code` |
| `COMAN/THELOOP.INC` | bit-identical: the 5,230-line include parsed and executed against the port's `theloop`, whole 64 KB vbuf, five columns | `the_ray_march_matches_the_shipped_generated_program`; four keyframes at 0 (A26) |
| `HARD/LINEBLIT.INC` | not applicable: on no link line (`HARD/LINK.TMP:1-3`) | `hard.md` |
| TUNNELI's "patched y-base" | bit-identical; what `TUN10.PAS` patches is the plot's colour immediate (`:168` `mov byte ptr cs:[@c+3],al`, the plot at `:187`), not a y-base; the port passes `bbc` as a variable | `tunneli.md` §5 |
| `GLENZ/VEC.ASM`'s matrix-in-immediates | bit-identical: state only, held in variables | `glenz.md` §5; `the_held_opening_1499_is_the_reference_index_plane` |
| `LENS/ASM.ASM`'s patched addressing | bit-identical: the patched displacement table is evaluated as a table | `the_aspect_ratio_scales_the_row_advance_not_the_in_row_steps`; `the_forehead_asset_difference_is_the_whole_residual` (A25) |
| PLZPART's "jump table" | bit-identical; `setplzparas` (`ASMYT.ASM:74-95`) patches four table bases into `plzline`'s instructions (the placeholders `0xC100`…`0xC400`, `:46-61`), not a jump table; the port evaluates the patched index expression over the map's flat segment | `the_table_bases_are_the_maps_not_the_assembly_placeholders`, `the_plasma_line_is_the_assemblers_index_expression` |
