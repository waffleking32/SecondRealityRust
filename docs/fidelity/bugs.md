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
it at all (spec §5.2).

---

## 1. The GMX plasma's palette-leak banding — `PLZPART` (slot 11)

**What it is.** The plasma's colours are written to the DAC through the parts' own palette path, and
colour from the *previous* palette persists — the field carries the broad colour bands that a
correctly-cleared ramp would not, and a "fixed" plasma (a clean interpolated ramp) looks flat and
wrong beside it. Spec §5.1 records it as the first named required behaviour and D7 as the reason the
rule exists.

**Where the source does it.** `PLZPART/PLZ.C:44-45` holds the six 256-entry palette tables
(`pals[6][768]`) and `plz()` (`:62-117`, the switch at `:73` and `:81-83`) hands them to the fade a
table at a time
(`cop_fadepal = pals[curpal++]`, reset with `memset(fadepal,0,768)`); the tables themselves are built
in `init_plz()` from `ptau` at `:179-217`, and the write path to the DAC is the **TWEAK library**
(`PLZPART/TWEAK.H:5-7`: `tw_setpalette`, `tw_setpalarea`, `tw_setrgbpalette`), whose implementation
is the binary `PLZPART/TWEAK.OBJ` and has no source in the tree. `init_plz()` fills each table from
entry **1** onward (`pptr=&pals[k][3]`, `:182` onward) and scales the rest in place at `:215-217`,
so what entry 0 and the un-written entries hold is part of the picture, not an oversight to tidy.

**What the port must do.** Reproduce the pixels, not the intention. The mechanism is not settled by
this register — it is a Phase 5 job to pin against the frames, because the DAC write sits inside a
library whose source is not vendored. **Do not "clean" the palette**, and do not substitute an
interpolated ramp: the difference is exactly what the comparison is for.

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

## 3. `dis_sync()`'s off-by-one — the music-position lookup (library, all parts)

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
`ordersync1` (`0000h,0 / 0200h,1 / 0300h,2 / … / 0d00h,10 / 3d00h,1 / …`). One caller in the tree:
`PAM/OUTTAA.C:39` — `while(dis_sync()<10 && !dis_exit());`.

**What the port must do.** Implement the lookup as written, off-by-one included: the code a caller
sees is the one belonging to the entry *before* the row it is standing on, and the order-0 case reads
whatever precedes the table (in the original that is not a value the table defines — reproduce the
ported equivalent, and record what the original's word actually is if a part's timing depends on it).

**Evidence.** No `.rgb` frame shows this — it is a timing/library behaviour. The evidence is the
source above, and the port's first consumer is whichever part waits on `dis_sync()`
(`PAM/OUTTAA.C:39`); its frames in `reference/parts/pam/` (7710, 7760, 7830, 7900,
`reference-indices.md` §6) are where a wrong off-by-one would surface as a timing difference, but the
check that pins it is a unit test on the ported lookup against this table.

---

## Adding an entry

An entry needs a measurement behind it, not a recollection: what the original draws (from a reference
frame or a decompiled line), the source that makes it happen, the obligation it puts on the port, and
where the evidence lives. If the port *cannot* reproduce the artefact, that is the other register —
`APPROXIMATIONS.md`, with the measured visual difference (spec §5.2). The line between the two is
intent: this file is for what the original does on purpose-as-shipped and the port must copy;
`APPROXIMATIONS.md` is for what the port knowingly does differently.
