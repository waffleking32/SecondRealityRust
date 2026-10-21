# Bugs to preserve — the original's artefacts, as required behaviour

`CLAUDE.md`'s non-negotiables: **"Preserve the original's bugs. A 'correct' implementation of the
plasma looks wrong."** This file is the register that rule points at (D7), and spec §5.1 is where
the policy is stated. An entry is *required behaviour*, not a defect to fix: a port that renders the
artefact away has broken fidelity, and the reference comparison is what says so.

Each entry carries: **what the original does**, **where the source does it**, **what the port must
do**, and **evidence** — the reference frames the port is compared against, which live in
`reference/parts/` and are indexed in `reference-indices.md` §2. The register grows as parts are
ported: Phase 1 wrote the three entries the spec names; a later phase adds the artefact its own
measurement turns up, and records an `APPROXIMATIONS.md` entry instead if the port cannot reproduce
it at all (spec §5.2). An entry whose part is *inferred* says so and says from what; an entry whose
mechanism is only partly pinned says which half, and what would settle it.

**Search `upstream/` binary-safe.** Much of the vendored original is latin-1 or has non-ASCII bytes
in strings and comments (`PLZPART/TWEAK.ASM`, `ALKU/MAIN.C`, `PLZPART/COPPER.ASM` are all
`Non-ISO extended-ASCII`), and `grep` does not decode those files: a plain `grep -rn 'dis_sync'
upstream/` exits **1**, silently, with no match — a false negative that reads exactly like "nothing
calls this". Use `grep -a` (`-a`, `--text`, or `--binary-files=text`) for any source search over
`upstream/`, and treat a clean result as a *question*, not as evidence of absence. This is the same
shape as the failure the whole-branch review kept catching: a check that reports absence rather than
an error.

---

## 1. The GMX plasma's palette-leak banding — `PLZPART` (slot 11)

**What it is.** The plasma's colours are written to the DAC through the parts' own palette path, and
colour from the *previous* palette persists — the field carries the broad colour bands that a
correctly-cleared ramp would not, and a "fixed" plasma (a clean interpolated ramp) looks flat and
wrong beside it. Spec §5.1 records it as the first named required behaviour and D7 as the reason the
rule exists.

**Where the source does it.** `PLZPART/PLZ.C:44-45` holds the six 256-entry palette tables
(`pals[6][768]`) and `plz()` (`:62-122`, the switch at `:73` and `:81-83`) hands them to the fade a
table at a time
(`cop_fadepal = pals[curpal++]`, reset with `memset(fadepal,0,768)`); the tables themselves are built
in `init_plz()` from `ptau` at `:179-217`, and `init_plz()` fills each table from
entry **1** onward (`pptr=&pals[k][3]`, `:182` onward) and scales the rest in place at `:215-217`,
so what entry 0 and the un-written entries hold is part of the picture, not an oversight to tidy.

**The palette path is fully readable in the vendored tree**, and both layers are in `PLZPART/`:

- the library primitives, `PLZPART/TWEAK.ASM` (350 lines, the source of the `TWEAK.OBJ` the part
  links): **`tw_setpalette`** (`:168-189`) — `lds si,[pal]`, `mov cx,300h` (768), write index 0 at
  `03c8h`, then **`rep outsb`**, i.e. a full 768-byte upload of entries 0–255; **`tw_setpalarea`**
  (`:191-212`) — the ranged form: `mov cx,[cnt]`, `cx = cnt*3` bytes, the write index set to
  `[start]`, `rep outsb` (the primitive by which a part *can* leave entries holding the previous
  palette; this part's own upload, below, is the full-length one); and **`tw_setrgbpalette`**
  (`:331-348`), one entry at a time.
- the plasma's own upload, `PLZPART/COPPER.ASM:86-116` (`copper2`, registered as copper slot 2 by
  `init_copper`, `:44-65`): when `do_pal` is set (`:96-105`) it points the DAC's write index at 0,
  `lds` the pointer in `cop_pal` and `rep outsb`s **all 768 bytes**. Two buffers reach that pointer:
  a `pals[k]` table (`PLZ.C:73-83`),
  or **`fadepal`** — declared `db 768*2 dup(?)` (`COPPER.ASM:184`) and used as **768 16-bit
  accumulators whose high bytes are the colours and whose low bytes are the fraction**, built by the
  per-frame accumulate loop at `:238-254` (`add [cs:di+ccc+768], al` / `adc [cs:di+ccc], ah`; the
  loop itself is `@@l4`, `:241-252`). Note that
  the per-table reset is `memset(fadepal,0,768)` (`PLZ.C:81`): it zeroes the **colour** bytes and
  leaves the fractional half standing across a table switch.

**What the port must do.** Reproduce the pixels, not the intention: copy `copper2`'s upload, the
`fadepal` accumulator and the fade loop as written, and **do not "clean" the palette** or substitute
an interpolated ramp — the difference is exactly what the comparison is for. What remains for
**Phase 5** is narrower than "the mechanism": it is to pin which of the two source-level candidates
produces the *visible* banding, against the frames below — the accumulator half surviving the
per-table `memset`, or the DAC shared with the plasma cube (`VECT.C:123` sets `cop_pal = fpal;
do_pal = 1` on the same callback). The lines are all in the tree; only their visible outcome is to
be measured.

**Evidence.** `reference/parts/plzpart/plasma/` — 6000, 7000 and 7600 (`reference-indices.md` §13,
§1.12): the filed plasma frames carry the banding (they are also three of the keyframes recorded as
having **no settled content**, so a non-zero diff there is partly the oracle's, §1.12 — read the row
before drawing a conclusion from one frame).

## 2. Shadow particles drawn over the particles in front of them — `MINVBALL` (slot 12)

**What it is.** Each dot's shadow is rasterised in the dot's own draw order, and there is **no
z-buffer**: a dot drawn later paints its shadow (and itself) over an earlier dot regardless of which
is nearer, so shadows can appear on top of closer particles. The spec calls it "shadow particles
rasterised over the cyan particles in the jumping-particle part"; the effect is plainly visible in
the reference frames — a ring of cyan particles with their shadows crossing them.

**The part is an inference, and it is the one thing here the spec does not state.** §5.1 names the
artefact for "the jumping-particle part" without naming which part that is. `MINVBALL` is
*identified*, not given, from three things that agree: the mechanism (a per-dot `oldposshadow` slot,
`DOTS/ASM.ASM:36`, and the `;shadow` block at `:98`), the sequence (`part-sequence.md` slot 12 is
`MINVBALL.EXE`, `DOTS/`, "MiniVectorBalls"), and the captures (`reference-indices.md` §14's frames
show the cyan particles and their shadows). A later phase that finds the phrase means a different
part should move the entry rather than delete it — the obligation is real either way.

**Where the source does it.** `DOTS/ASM.ASM:36` gives each dot a slot pair
(`x,y,z,oldposshadow,oldpos,…`) and `:98-108` is the shadow: it restores the pixel its own previous
shadow covered, then writes the shadow colour `87+87*256` at the dot's projected position. The ball
follows at `:120-179` with the three depth-shaded rows from `_depthtable1/2/3`
(`DOTS/MAIN.C:163-166` builds them around palette index 2). Both sprites are drawn in the dot loop's
order (one dot per `loop @@1` pass, shadow then ball), and what makes the overdraw a bug is what is
*absent* between them: no depth sort and no z-test, so the painter's order is the only ordering.

**What the port must do.** Copy the loop and the order — shadows and balls interleaved per dot, each
restoring only its own previous pixels. Sorting the dots by depth, or deferring shadows, removes the
artefact and changes the picture.

**Evidence.** `reference/parts/minvball/` — 10900, 11200, 11800, 12400 (`reference-indices.md` §14).
All four are moving keyframes at 3/3 (100 %) support, so a diff there is a real difference.

## 3. `dis_sync()`'s off-by-one — the music-position lookup (library; two callers)

**What it is.** The sync service does not return the code for the music position it is handed: it
returns the **previous** table entry's code, and at the start of the table it reads the word *before*
the table. Spec §5.1: "returns the previous entry's code; reads off the table start at order 0".
Parts that wait on it therefore wait until one step later than the table intends.

**Where the source does it.** `DIS/DISINT.ASM`'s `sync_10` (service 10, called by `dis_sync`,
`DIS/DISC.ASM:161-169`) reads the player's order/row (`_np_ord`/`_np_row`), walks `ordersync1` in
steps of four bytes, stops at the first entry whose position is `>=` the current one (`jbe @@1`), and
then returns `cs:[bx-2]` — the **second word of the entry before the match**, i.e. the previous
row's code. When the position is at or before the first entry (order 0, row 0), that is the word
immediately *before* `ordersync1`. The table itself is the `dw` list at `DIS/DISINT.ASM`'s
`ordersync1` (`0000h,0 / 0200h,1 / 0300h,2 / … / 0d00h,10 / 3d00h,1 / …`).

**Two parts call it, at six live sites** — so the off-by-one is a Phase 2 dependency, not a
curiosity in one part:

- `PAM/OUTTAA.C:39` — `while(dis_sync()<10 && !dis_exit());`.
- `ALKU/MAIN.C:59, 66, 72, 79` — `while(dis_sync()<1..4 && !dis_exit());`, the gates on ALKU's four
  text cards — and `:138` — `while(((a&1) || dis_sync()<4+tptr) && !dis_exit() && a<319)`, the
  horizon pan's own gate. (`ALKU/MAIN.C:512-513` is the comment block that documents the codes:
  `0 = …`, `1 = fc_pres`.) The phase's own `reference-indices.md` records the same thing for ALKU
  (§4: "each card is gated on the music (`while(dis_sync()<1..4)`, `MAIN.C:59,66,72,79`)") and PAM
  (§6, `:39`).

**What the port must do.** Implement the lookup as written, off-by-one included: the code a caller
sees is the one belonging to the entry *before* the row it is standing on, and the order-0 case reads
whatever precedes the table (in the original that is not a value the table defines — reproduce the
ported equivalent, and record what the original's word actually is if a part's timing depends on it).

**Evidence.** No `.rgb` frame shows this — it is a timing/library behaviour — and the consumers are
exactly the ports that must show it in their captures: ALKU, whose cards are gated on it
(`reference/parts/alku/`, 900/1719/2719/5859, `reference-indices.md` §4), and PAM
(`reference/parts/pam/`, 7710/7760/7830/7900, §6). A wrong off-by-one shows there as a timing
difference; the check that pins it exactly is a unit test on the ported lookup against this table.

## 4. `setpalarea` uploads `2*count + start` bytes — the ranged DAC write (BEGLOGO, ENDLOGO and JPLOGO ported; the others behind them)

**What it is.** The ranged palette upload moves **`2*count + start` bytes where it intends
`3*count`**, so a fade that means to ramp 254 entries writes two-thirds of them: the last third of
the entries keeps whatever the previous write left, and a part that follows it inherits a DAC that
never faded. The error is one instruction: the routine reuses `ax`, which still holds `start`, as the
second addend instead of `count`.

**Where the source does it.** `BEG/ASM.ASM:93-115`. `:104-105` points the DAC's write index at
`start` (`mov dx,3c8h` / `out dx,al`), `:106` loads `count` into `cx`, and then

```
:107	shl	cx,1        ; 2*count
:108	add	cx,ax       ; + start, not + count
```

`REPOUTSB` (`:84-91`) then moves exactly `cx` bytes. The shipped binary carries it: disassembling
`BEG/BEG.EXE`, `_setpalarea` begins at file offset `0xcb7` and the three instructions are at
`0xccb`-`0x0cd1`, followed by the copy loop at `0xcd2`. The same routine, with the same two
instructions, is in `END/ASM.ASM:107-108`, `JPLOGO/ASM.ASM:116-117`, `ENDPIC/ASM.ASM:98-99` and
`TECHNO/KOEA.ASM:1054-1055`. The copies that are **correct** load `count` into `ax` before the
doubling — `LENS/ASM.ASM:253-255`, `START/ASM.ASM:192-194`, `COMAN/ASM.ASM:93-95`,
`TWIST/ASM.ASM:85-87` — so this is a mistake that was fixed in later copies of one routine, not a
convention of the tree.

**What the port must do.** Copy the arithmetic as written: `bytes = 2*count + start` from the DAC's
write index. **The length is the call's, and the tree's two callers differ**: `BEG/BEG.C:84` passes
254, so `setpalarea(ramp, 0, 254)` writes **508** bytes — entries 0-168 whole and the **red byte of
entry 169** — while `END.C:105` and `:116` pass 255, so `setpalarea(pal2, 0, 255)` writes **510**,
entries 0-169 whole and nothing of 170-254. (Task 6 measured the second; this entry said 508 for both
until then, which is BEGLOGO's call read across to a part whose source says 255.) In both parts
entries 170-254 keep whatever the previous write left, and entry 255 is untouched. Neither a partial
entry nor a skipped range may be tidied: the DAC is a byte stream with an auto-incrementing index, and
the bug is a byte count.

**Evidence, and what makes this entry unusual: on BEGLOGO's art the bug is invisible.** The screen
indexes 1, 2, 3, 14-58 and 71-79 (the picture, and the white field is 15), all inside the written
range, so a port that uploaded the full 762 bytes matches all four keyframes — measured, not assumed:
`the_fade_constants_are_derived_by_matching_the_reference_bit_for_bit` in
`bins/secondreality/src/parts/beglogo.rs` reruns the fade scan with a 762-byte upload and gets the
same `c` for 470, 560 and 900. `reference/parts/beglogo/` (430, 470, 560, 900) is therefore evidence
that the port *did not* fix it only in the sense that the comparison is exit 0 either way; the
deciding evidence is the disassembly above, and the code writes 508.

**What it will cost someone.** This is a bug with a *deferred* consequence, and it is the first entry
here with one: the DAC state the part leaves is part of its output. BEGLOGO leaves entries 170-254
**white** (they were filled at `BEG/BEG.C:25-31` and the upload never reached them), which is the
state GLENZ starts from — and GLENZ's `zoomer2()` (`GLENZ/ZOOMER.C:59-68`) writes palette index 255
into its bands and fades entries 0-127, a range that does not overlap them either. ENDLOGO leaves them
white too, from its own `END.C:90-91`, with 510 bytes a step where BEGLOGO moves 508 — and on ENDLOGO
the entry-169/170 boundary the two calls straddle is invisible for the same reason this whole entry
is: `assets/END/PIC.UH` indexes 0-71 and the screen before its blit is index 0. The first part to
paint a fade into entries 170-254 will meet a region that has been white since BEGLOGO (or since
ENDLOGO, on `SECOND 5`), and the difference will show as colour that does not move. Whoever ports that
part should start here.

## 5. GLENZ's span raster: the 1..319 clamp, the broken same-x merge, the fresh slot's shade, and pixel 63999

**What it is.** Four artefacts of `GLENZ/NEW.ASM`'s transparent compositor and `VEC.ASM`'s colour
allocator. Each decides pixels or palette entries, so each is required behaviour rather than
something to tidy:

- **The boundary clamp is 1..319, not 0..319.** `ng_pass2` reads the low word of `x>>16` signed
  and clamps it with signed compares (`NEW.ASM:246-253`): an x of 0 becomes 1 and 320 becomes
  319. A span's left edge therefore never lands on column 0 of its row, and a boundary at the
  right edge sits on column 319. The port clamps the same
  (`boundaries_clamp_to_columns_one_and_319`; the row-crossing case is
  `a_row_crossing_span_uses_the_global_position`).
- **The same-x merge compares the wrong register.** `ng_pass2` tries to XOR a boundary whose
  clamped x equals the previous one's into the previous entry (`:255-260`), but the word
  compared is `cx`, which the preceding `mov cx,ax` (`:267`) has loaded with the stored **colour
  word**, not the clamped x — the linked `V.EXE` confirms it (`cmp %ax,%cx` at load offset
  0x3daf). The branch fires only when a colour word equals a column 1..319; GLENZ's colour
  words are all ≥ `0x4000`, so in the shipped path it is inert. The port emits every boundary
  and sorts by position; XOR is commutative, so the pixels are identical
  (`coincident_edges_xor_like_the_merged_edge`, and the equivalence test below asserts the
  corpus never leaves GLENZ's colour domain).
- **A fresh colour slot is shaded 0 and its R is the allocation scan's residual.**
  `VEC.ASM:454-460`: when no slot is reused, the scan (`mov cx,15` / `loop`, then `mov ax,bp`)
  has clobbered **CL** and zeroed AH, so the stored shade is 0; for a bit-1 word R is
  `16 − slot/2` (slot 2 → 15 … slot 32 → 0), G = B = 0. A reused slot keeps the computed
  shade. The port reproduces both branches
  (`glz_fresh_allocation_uses_the_scan_residual_and_zero_shade`,
  `glz_allocates_even_slots_in_order`,
  `glz_group_ramp_restores_the_base_and_clamps_each_entry`).
- **Pixel 63999 is never painted.** `ng_pass3`'s sentinel list ends `(63999, 0), (-1, 0)`
  (`:279-285`); the fill runs to the boundary exclusive and `-1` stops the walk, so the last
  byte of the 64,000-byte buffer is never written by a span. The port leaves it alone
  (`edges_off_the_screen_are_dropped_or_clipped`,
  `the_full_repaint_equals_the_source_two_list_walk`).

**Not in this register: the slot-32 exhaustion.** With the tree's full-word visible key the
256-byte `rolcol` pool exhausts and the sixteenth word's group is written at DAC entry 0,
painting every later face with colour 0 (`glz_pool_exhaustion_uses_slot_32_and_entry_0`). The
reference's picture does not show that model — it identifies the in-bounds low-byte key, which
the port adopts (A37) — so the exhaustion is the mechanism of a **rejected** reading, pinned by
a unit test and registered in A37, not behaviour the shipped part has.

**Where the source does it.** `GLENZ/NEW.ASM:174-418` (`ng_pass2`/`ng_pass3`),
`GLENZ/VEC.ASM:403-506` (`demo_glz`'s pool and groups).

**What the port must do.** Keep the clamp, the sentinel walk and both colour branches literal;
keep the boundaries independent (the source's merge is inert) and never clear between frames
(the background comes from `bgpic`; the full repaint is A39's proven substitution).

**Evidence.** The tests above; the reference's 1499 held frame (0 differing indices) and the
2300/3000/3400 keyframes (`docs/fidelity/glenz.md` §11).

## 6. GRID's `sinus1` reads an uninitialised `d` — `GRID` (the vault)

**What it is.** `sinus1`'s third cursor `d` is never initialised, so the shipped `P.EXE`'s
x waveform — and every curve of the effect — depends on whatever its stack frame held. The
object's own static prediction is `0x116`; the binary measures `0x7FE (mod 0x2000)`.

**Where the source does it.** `GRID/MAIN.C:33` declares `d` without a value and `:51` zeroes
only `b` and `c`; `:51-58` advances `d += 67` and indexes `sin1[(d>>3)&1023]`. The 1,024-point
developer screen before `getch()` is what pins the artifact's value: an exhaustive match over
0..65535 against the capture's 982-pixel screen finds only the `0x7FE` class, while `0x116`
matches 47 (D24: the binary is the artifact, not the object).

**What the port must do.** Start the tables from `d0 = 0x07FE`
(`the_initial_d_is_the_measured_0x7fe`, `the_measured_d_is_the_only_residue_whose_tables_match`);
do not "clean" it to 0 or to the object's `0x116`. A clean table moves the whole curve.

**Evidence.** `reference/parts/grid/` 400/500/600/700/800, which differ only by A41's
6/2/36/33/14-pixel sampling residual; `docs/fidelity/grid.md` §2.

## 7. HARD's `part2init` loop reads an uninitialised `b` — `HARD` (the vault)

**What it is.** `part2init`'s `int b` is not initialised before the `depthcol` loop reads it
in the `a < 1000 || b >= 3000` test. The loop assigns it at `a = 0` and the short-circuit
keeps every later read after an assignment, so the shipped table is determinate: 127 for
0..1031 and `127 - (a-1000)/32` down to 62 at 3099. The read is benign, but the table it
produces is the depth-colour ramp the grid's dots are drawn with.

**Where the source does it.** `HARD/BP.C:124-132`: `int a,r,g,b,...;` then
`for(a=0;a<3100;a++) { if(a<1000 || b>=3000) b=127; else b=127-(a-1000)/32;
depthcol[a]=b; }`. The first test's `b` is uninitialised; `a = 0` writes 127 before the
second iteration reads it.

**What the port must do.** Reproduce the carried value and the table, not a "clean"
initialisation (`part2init_reads_the_uninitialised_b`). The port's `b` starts at 0 as a model
of the uninitialised local, but the first read is an `a < 1000` short-circuit, so the value
never reaches the table.

**Evidence.** `reference/parts/hard/` 1050 (part 2's grid) is exact; `depthcol[3000] = 65` is
what makes the first grid frame's 100 pixels `(1,63,63)` (A48). `docs/fidelity/hard.md`, the
part-2 section.

## 8. HARD's mode-4 band forces entries 189 and 1 to white — `HARD` (the vault)

**What it is.** The mode-4 band's falling palette ramp forces `pal[254-65]` = `pal[189]` and
`pal[1]` to `(63,63,63)`, not the `251`/`3` the plan and the research report assumed. The
trail's last-drawn colour is 189, so the forced entries are what leave the 100 white trail
pixels the mode-5 rebuild looks for.

**Where the source does it.** `HARD/B.C:281-286`: `a=(254-65); pal[a*3+{0,1,2}]=63;
pal[1*3+{0,1,2}]=63;`. `B.C:297-301`'s mode-5 rebuild tests `vram[u]==254-65` and converts
those pixels to index 1.

**What the port must do.** Force 189 and 1, not 251/3; the 66-entry ramp is
`a = 252 - pal1colp`, clamped at 254. `the_mode_four_band_palette_is_the_sources` pins the
entries.

**Evidence.** `reference/parts/hard/` 910 is exact; the held 930–955 band's only
distinguishable trail pixels are 100 white ones at index 189
(`out/task2/hard-930-1000`). `docs/fidelity/hard.md`, "The palettes are the source's byte
writes".

## 9. HARD's part-2 fade dims by 255/256, not by its intended ramp — `HARD` (the vault)

**What it is.** `part2`'s fade calls `fadepal(pal,pal2,a*16)` with `a = (500-16)-count`,
which is negative for every count that reaches the call (485..499). `a*16` is a negative
16-bit `int`; `fadepal`'s unsigned `cmp dx,256` clamps `dl` to 255, so the multiply is
`src*255/256`: the palette dims by 255/256 and the intended fade never runs.

**Where the source does it.** `HARD/BP.C:173-176` and `HARD/BA.ASM:174-215` (`fadepal`'s
`mov al,[si]; mul dl; mov [di],ah` per channel, the blue bias, and the unsigned clamp). The
capture shows the consequence: the band background `(0,0,81)` at 1440 and `(0,0,77)` at 1445 —
6-bit blue 20 → 19, one 256th per fade step.

**What the port must do.** Reproduce the clamp and the 8-bit multiply's high byte; do not
"fix" the negative `a`. `fadepal_is_the_high_byte_multiply` pins the clamp, the blue bias and
the negative `dl`; the part-2 keyframe 1050 and the part-3 1600 carry the dimmed palette.

**Evidence.** `reference/parts/hard/` 1050; `out/task2/hard-1430-1500`'s 1440/1445 pixels;
`docs/fidelity/hard.md`, the part-2 fade paragraph.

---

## Adding an entry

An entry needs a measurement behind it, not a recollection: what the original draws (from a reference
frame or a decompiled line), the source that makes it happen, the obligation it puts on the port, and
where the evidence lives. If the port *cannot* reproduce the artefact, that is the other register —
`APPROXIMATIONS.md`, with the measured visual difference (spec §5.2). The line between the two is
intent: this file is for what the original does on purpose-as-shipped and the port must copy;
`APPROXIMATIONS.md` is for what the port knowingly does differently.
