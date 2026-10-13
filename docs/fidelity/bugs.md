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

## 4. `setpalarea` uploads `2*count + start` bytes — the ranged DAC write (BEGLOGO and ENDLOGO ported; JPLOGO and others behind them)

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

---

## Adding an entry

An entry needs a measurement behind it, not a recollection: what the original draws (from a reference
frame or a decompiled line), the source that makes it happen, the obligation it puts on the port, and
where the evidence lives. If the port *cannot* reproduce the artefact, that is the other register —
`APPROXIMATIONS.md`, with the measured visual difference (spec §5.2). The line between the two is
intent: this file is for what the original does on purpose-as-shipped and the port must copy;
`APPROXIMATIONS.md` is for what the port knowingly does differently.
