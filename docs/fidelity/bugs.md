# Bugs to preserve — the original's artefacts, as required behaviour

`CLAUDE.md`'s non-negotiables: **"Preserve the original's bugs. A 'correct' implementation of the
plasma looks wrong."** This file is the register that rule points at (D7), and spec §5.1 is where
the policy is stated. An entry is *required behaviour*, not a defect to fix: a port that renders the
artefact away has broken fidelity, and the reference comparison is what says so.

Each entry carries: **what the original does**, **where the source does it** (file:line), **what the
port must do**, **a reference frame that shows it** (a capture under `reference/parts/` indexed in
`reference-indices.md`, or the stated reason no frame can) and **a pinning test** (spec §5.1's rule).
The register grows as parts are ported: Phase 1 wrote the three entries the spec names; a later phase
adds the artefact its own measurement turns up, and records an `APPROXIMATIONS.md` entry instead if
the port cannot reproduce it at all (spec §5.2). An entry whose part is *inferred* says so and says
from what; an entry whose mechanism is only partly pinned says which half, and what would settle it.

**The filter** (Phase 9 Task 9). Every behaviour the part notes call preserved, kept or required is
classified once. It gets a **full entry** (§1-§28) if the original *executes* it on the shipped path
and a reference frame shows it or "fixing" it would change a drawn pixel, a palette value, a sample or
a timing. Otherwise — dead code, an unreachable branch, a read whose value never reaches the screen —
it gets a row of the **index** at the end (part, behaviour, source line, why no frame can show it, the
note that holds it). A *timing* counts: the music's own behaviours (§28) and the gates' exact
conditions (§15.4, §19.5) are entries although no `.rgb` frame isolates them, and say so. The
closing **classification** lists every audited line and proves none is missing. Each entry has a stable
heading (`§N`, `§N.M`) that the part notes and `APPROXIMATIONS.md` cite.

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
an interpolated ramp — the difference is exactly what the comparison is for.

**The mechanism is settled: it is the accumulator's half.** `memset(fadepal,0,768)` (`PLZ.C:81`)
clears the first 768 of `fadepal`'s 1,536 bytes — the colour bytes — and leaves the 768 fraction
bytes standing across the table switch, so each new table's fade begins from the previous table's
fractional state (`plzpart.md:119-122`). The first sum of a fade carries that leftover fraction into
the colour byte; a clean 16-bit clear would start every channel at the step alone. The port keeps
both halves as 768 `u16` accumulators and its reset (`CopperState::clear_colour_half`) keeps the low
byte. The other candidate the entry used to hold open — the DAC shared with the plasma cube
(`VECT.C:123` sets `cop_pal = fpal; do_pal = 1` on the same callback) — is the part's *next* screen:
the cube's hand-off is at boot ~8315 (`plzpart.md` §10) and the banding is filed on 6000, 7000 and
7600, long before the cube's code runs, so it cannot be their cause.

**Evidence.** `reference/parts/plzpart/plasma/` — 6000, 7000 and 7600 (`reference-indices.md` §13,
§1.12): the filed plasma frames carry the banding (they are also three of the keyframes recorded as
having **no settled content**, so a non-zero diff there is partly the oracle's, §1.12 — read the row
before drawing a conclusion from one frame). The port's residuals against them are the A24 tear's —
11.209 %, 5.848 % and 4.052 % of pixels, every colour delta within a sixteenth of the palette
(`plzpart.md` §4; `cargo test -p secondreality --test plzpart -- --nocapture`) — not a missing band.

**Test.** `a_table_switch_zeroes_the_colour_half_and_keeps_the_fraction`
(`bins/secondreality/src/parts/plzpart.rs`): after the switch's clear the accumulators read
`0x00F0` / `0x0001` / `0x0080` from `0x2AF0` / `0x3F01` / `0xFF80`, and the next fade's first step
(`0xF0 + 0x20`) carries into the colour byte (`0x0110`); a clear that zeroes the whole word fails it
(`0x0000` for all three).

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
All four are moving keyframes at 3/3 (100 %) support, so a diff there is a real difference. The
port is byte-identical at all four (0 differing pixels each, and every shadow pixel shared:
`the_four_keyframes_match_the_reference_at_the_mapping`; `cargo xtask rediff` registers them at 0), and
where two dots coincide the winner is the draw order, with the initial `j` fitted (§18.4, A19).

**Test.** `shadows_are_drawn_over_balls_without_a_depth_test`
(`bins/secondreality/src/parts/minvball.rs`): a near dot (bp 8000) and a far dot (bp 9000) are
drawn through `drawdots` in both orders. With the near dot first, the far dot's shadow covers the
first two pixels of the near ball's wide row (`[87, 87, ball, ball]`); with the far dot first, the
near ball covers the shadow and the row is the ball's four bytes. Sorting the dots by depth, or
drawing every shadow first, fails the first assertion.

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

**The two ends of the table, both answered** (A15, `APPROXIMATIONS.md`). *Below* it: at `(0, 0)` the
`%bx - 2` read lands on the two bytes before `ordersync1`, `pop es; ret` (`07 c3`, the end of
`musframe_9`, `DIS/DISINT.ASM:420-424`), which a Turbo C caller receives as the 16-bit `int`
**-15609** — negative, so every `dis_sync() < N` keeps waiting, as the reference's ALKU does for the
278 samples from frame 148 to 426 that `second5` holds at `(0, 0)`. *Above* it: past the last key
(`0x4200`) the loop's 16th compare reads the bytes after the table, and up to `0x8E2E` that matches
and the answer is the table's own last code, **4**; above `0x8E2E` the answer is a linker-set word
that differs between the three shell builds in the tree, so the port refuses it (a panic naming the
position). No caller reaches the refusal: ALKU's highest published order is 9, PAM's gate is crossed
at `(13, 1)`.

**The consequence the callers' source adds: ALKU's batch gate waits for code 5, not 4.**
`ALKU/MAIN.C:102` is `switch(tptr++)`, so the branch taken as case 0 leaves `tptr` at 1 before
`:138`'s `dis_sync()<4+tptr` is evaluated: the wait is for code **5**. It moves every text batch by
one table step, so fixing either the post-increment or the lookup changes a timing
(`alku.md` §5).

**Evidence.** No `.rgb` frame isolates this — it is a timing/library behaviour — but the consumers
are exactly the ports whose captures a wrong lookup would move: ALKU, whose cards are gated on it
(`reference/parts/alku/`, 900/1719/2719/5859, `reference-indices.md` §4; the codes reach 1, 2, 3 and
4 at boot 1537, 2097, 2509 and 3069 in `reference/music/second1`), and PAM (`reference/parts/pam/`,
7710/7760/7830/7900, §6; the gate `dis_sync()<10` opens at `(13, 1)` on boot 7710,
`reference/music/second1b`). The reference's own session-to-session race at ALKU's *second* pass is
not this behaviour and is indexed below.

**Tests.** `sync_reads_the_table_the_object_reads` walks 23 `(order, row)` cases against the table,
each key at the key (the previous entry's code) and one row later (its own);
`sync_answers_the_position_before_the_table_with_its_own_word` pins `(0, 0)` to -15609 and
`BEFORE_THE_TABLE` to `0xC307` (all in `crates/sr-engine/src/music.rs`);
`sync_answers_the_band_above_the_tables_last_key_with_the_tables_last_code` and
`sync_refuses_the_first_key_past_the_sixteenth_compare` are the A15 edges;
`the_table_is_the_shipped_shells_own_bytes` finds the table once in each of the three shell builds;
`the_picture_fade_hands_over_to_a_batch_gated_on_code_five` (`parts/alku.rs`) pins the code 5.

## 4. `setpalarea` uploads `2*count + start` bytes — the ranged DAC write (BEGLOGO, ENDLOGO, JPLOGO and TECHNO)

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
entries 170-254 keep whatever the previous write left, and entry 255 is untouched. **JPLOGO and TECHNO
pass 256**: `setpalarea(palette,0,256)` writes **512** bytes — entries 0-169 whole and the red and
green bytes of entry 170 (`JPLOGO/ASM.ASM:116-117`, `TECHNO/KOEA.ASM:1054-1055`; the troll's calls,
`TECHNO/KOE.C`) — and entries 171-255 keep what they held. JPLOGO's `inittwk` first writes 768 zero
bytes to the DAC (`JPLOGO/ASM.ASM:31-38`), so its 512 build on black: entry 170's blue and entries
171-255 are zero, not the previous part's. The copy in `ENDPIC/ASM.ASM:98-99` belongs to the other
directory that stages a `beg.exe`, which did not ship (`beglogo.md` §1). Neither a partial entry nor
a skipped range may be tidied: the DAC is a byte stream with an auto-incrementing index, and the bug
is a byte count.

**Evidence, and what makes this entry unusual: on BEGLOGO's art the bug is invisible.** The screen
indexes 1, 2, 3, 14-58 and 71-79 (the picture, and the white field is 15), all inside the written
range, so a port that uploaded the full 762 bytes matches all four keyframes — measured, not assumed:
`the_fade_constants_are_derived_by_matching_the_reference_bit_for_bit` in
`bins/secondreality/src/parts/beglogo.rs` reruns the fade scan with a 762-byte upload and gets the
same `c` for 470, 560 and 900. `reference/parts/beglogo/` (430, 470, 560, 900) is therefore evidence
that the port *did not* fix it only in the sense that the comparison is exit 0 either way; the
deciding evidence is the disassembly above, and the code writes 508.

**Tests.** One per caller, each starting from a patterned live palette and pinning the written
region, the partial entry and the untouched tail:
`the_fade_upload_is_the_originals_508_bytes_and_stops_inside_entry_169`
(`bins/secondreality/src/parts/beglogo.rs`; count 254, only entry 169's red byte lands),
`the_upload_is_the_originals_two_count_plus_start_bytes` (`parts/endlogo.rs`; count 255, 510 bytes,
and the 254 case as BEGLOGO's), `the_palette_upload_is_the_sources_512_bytes` and
`init_clears_the_dac_before_the_512_byte_upload` (`parts/jplogo.rs`), and
`the_palette_upload_is_two_bytes_a_colour_and_truncates_at_512` (`parts/techno.rs`; the 512 and the
general rule, start 4 / count 2 → 8 bytes at offset 4).

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

## 10. ALKU — the picture's palette alias, the refresh that reads past the row, the text buffer's short clear, and the gate for code 5 (slot 1)

Tests are in `bins/secondreality/src/parts/alku.rs`. Frames are `reference/parts/alku/` — 900, 1719,
2719, 5859 (`reference-indices.md` §4); the port is byte-identical at the first three and 2,353 of
128,000 pixels off at 5859, which is A20's text-batch phase, not any entry below.

### 10.1 The palette write-order alias — `palette[192..768] = palette[..576]`

**Original.** After `palette2` and `fade2` are built, one loop repeats entries 0-63 three times, so
every entry 64-255 is entry `v mod 64`. **Where.** `ALKU/MAIN.C:212`. **Port must.** Keep the alias:
it is what makes the text's XOR (`0x40/0x80/0xC0`) and the `and 0x3f` keep the picture's low six bits,
so the picture is unchanged under the text. A "clean" 256-colour palette recolours the picture under
every card. **Frame.** 1719 and 2719 (cards mid-hold): 0 of 128,000 pixels differ. **Tests.**
`the_picture_is_hoi_u_with_the_three_entries_the_release_ships` (all 256 entries),
`the_text_operations_keep_the_pictures_low_six_bits`.

### 10.2 The refresh reads past the picture's row

**Original.** The scroll's last refresh is at `a = 316`; its source dword `(a/4+86)*4` is columns
660-663 of a 640-column picture, so its last byte and neighbours are the *next row's* columns 20-23.
**Where.** `ALKU/MAIN.C:407-408`. **Port must.** Read the dword where the source does, not wrap it
inside the row. **Frame.** None: the pan ends at `a = 319` and the filed 5859 is mid-scroll (`a = 309`
displayed), so no filed frame is at `a = 316`. **Test.**
`the_picture_blit_writes_the_band_and_the_refresh_reads_past_the_row` (column 165 of `a = 316` is
`picture[660..664] == picture[640+20..640+24]`).

### 10.3 `memset` clears 186×320 of a 186×352 text buffer

**Original.** `tbuf` is `char far tbuf[186][352]` (`:45`) and `memset(tbuf,0,186*320)` (`:101`)
clears the first 320 columns' worth of bytes, so the array's last 16 rows survive each batch.
**Where.** `ALKU/MAIN.C:101` (`alku.md` cites `:99`; the file's line is 101). **Port must.** Clear
exactly 186×320 bytes of the 186×352 array. **Frame.** None isolates it: the surviving rows are the
text buffer's own, redrawn by the next `fmaketext`, and the filed frames are cards (1719, 2719) and
one mid-batch scroll (5859) whose residual is the phase. **Test.**
`the_tbuf_clear_keeps_the_arrays_last_sixteen_rows` (asserts `TBUF_CLEAR == 186*320`, then runs the
part to its first batch with a marker in the buffer and reads what the production clear left: the
first 59,520 bytes cleared, columns 320.. of rows 170..185 and the last byte still the marker; a
full clear or a short one fails it).

### 10.4 The batch gate waits for code 5 (the post-incremented `tptr`)

Registered with `dis_sync()` in §3: `switch(tptr++)` (`ALKU/MAIN.C:102`) leaves `tptr` at 1 when
`:138`'s `dis_sync()<4+tptr` runs, so the first batch waits for code **5**. A timing, and a drawn
pixel by consequence (the batch lands one table step later). **Frame.** None isolates it (the codes
are music positions, `reference/music/second1`). **Test.**
`the_picture_fade_hands_over_to_a_batch_gated_on_code_five`.

## 11. PAM — the animation loop draws 40 of the container's 42 frames (slot 3)

**Original.** `PRAX4.FLI`'s header counts 41 frames and `OUT.ANI` holds 42. The loop is
`while(!dis_exit() && f++<45)` with `if(f<=40)` guarding the draw: **40** frames are drawn, the last
is 0-based 39, and steps 41-45 only queue the closing white copy. None of the three counts is the
drawn one. **Where.** `PAM/OUTTAA.C:46`, `:49` (`VFLI.C:66` reads the header word and nothing reads
it again). **Port must.** Draw 40; a loop that trusts the header (41) or the container (42) draws
one or two more pictures before the wash. **Frame.** `reference/parts/pam/` 7760 and 7830 are
animation frames and byte-identical (`reference-indices.md` §6.2); the frame-39 picture itself is
the unfiled scan sample at 7870 (`644acaf3…`, `pam.md` §3) and 7900's `.idx` is the next part's
white field (A21), so **no filed frame stands on frames 40 or 41** — the reason is the capture's
spacing, not the part's. **Test.** `the_fli_header_counts_forty_one_and_the_container_forty_two`
(`parts/pam.rs`; reads both counts from the files and pins 40 drawn).

## 12. U2A and U2E — the page ring's escape, the first draw U2E skips, and U2E's one-tick-late palette rotation (slots 2 and 16)

Tests are in `bins/secondreality/src/parts/u2a.rs`, `u2e.rs` and the shared `parts/visu/mod.rs`. Frames: U2A 6500, 7300 and 7640
(`reference-indices.md` §5), U2E 1000, 1600, 2600, 3600 (§18).

### 12.1 The deadlock escape and the start-up burst: the page cadence

**Original.** The draw loop waits on `while(cl[clw].ready)` and breaks at `deadlock>16`, drawing
into a page the display may still hold; the 16-retrace start-up delay lets the first pages be drawn
before the copper's delay expires, so the schedule parses 8, 8, 4 and 1 frames on the first four
draws. **Where.** `VISU/C/U2A.C:278-283`; `VISU/C/U2E.C:388-391` is the same loop. **Port must.** Model the
ring as the copper counts it — `next_page` returns the busy page once the count passes 16 — not
as a clean double buffer. **Frame.** 6500/7640 (U2A) and 1000-3600 (U2E) are drawn through that
cadence; U2A is exact at 6500 and 7640, and 7300's residual is a stream-state difference, not the
cadence (`u2a.md` §5). **Tests.** `ring_matches_the_u2a_cadence_over_twenty_ticks`,
`the_cadence_over_eight_ticks_advances_four_stream_frames` (U2E).

### 12.2 U2A's two exits leave different screens

**Original.** `dis_exit()` inside the gate's wait is a bare `return` (`VISU/C/U2A.C:239`): no copper is
installed and no page cleared, so the pad stays up. Inside the loop, `while(!dis_exit() && !xit)`
falls through to the teardown (`:432-441`), so the four cleared pages show. **Port must.** Keep both:
exit in the gate leaves the pad's page and DAC untouched; exit in the loop reaches the teardown.
**Frame.** None: no reference frame sits at an exit. **Test.**
`the_gate_holds_until_the_scripted_cue_and_exits_on_request` (asserts the untouched page and DAC).

### 12.3 U2E's first draw is skipped

**Original.** `firstframe`'s branch draws nothing and resets `syncframe`/`coppercnt`
(`VISU/C/U2E.C:433`), so nothing is drawn from the first parse. **Port must.**
`Driver::draw_frame` performs the reset without touching the page; the first loop tick still
schedules and queues page 0 (the fadeset page). **Frame.** None isolates it: it decides the
state every U2E keyframe (1000-3600) is drawn from (`reference-indices.md` §18). **Test.** `the_first_loop_tick_draws_nothing_and_queues_the_fadeset_page`.

### 12.4 U2E's teardown writes 768 bytes from DAC entry 255, one tick late

**Original.** Each teardown step is `fpal[a]+=4; dis_waitb(); outp(0x3c8,255);` then 768 data bytes
(`VISU/C/U2E.C:543-556`): the write starts at entry 255 and wraps, so entry 255 takes `fpal`'s entry 0 and
entry `j < 255` takes entry `j + 1`; the trigger tick only computes step 1, step `n` is visible on
tick `T+n`, and the part ends on the tick that shows step 16. **Port must.** Reproduce the rotation
and the one-tick lag. **Frame.** None: the filed keyframes (1000-3600) are all before the teardown.
**Test.** `the_teardown_fades_sixteen_steps_from_dac_entry_255_and_finishes` (derives the rotation
from the source and pins every entry; `a_sticky_exit_flag_runs_the_whole_sixteen_step_teardown`).

### 12.5 U2E's name hacks run

**Original.** `name[1] == '_'` forces an object's sort distance to 1,000,000,000 (`VISU/C/U2E.C:408`) and
`currframe > 1800 && currframe < 2200` forces `s01`'s distance to 1 (`:409-416`: `900*2` and `1100*2`, strict bounds, in
`currframe`'s two-retrace units); the shipped scene has nine `_` objects and `U2E.028` is `"s01"`.
**Port must.** Keep both, with the strict bounds. **Frame.** U2E 2600 and 3600 draw the sorted
scene (`reference-indices.md` §18). **Test.** `the_scene_is_the_u2e_city_with_the_name_hacks_object`.

## 13. PANICEND — `getpal`'s decimal zero, `copyline`'s latch, the fade tables from byte 3, and the collapse that ends at `a = 3` (slot 8)

Tests are in `bins/secondreality/src/parts/panicend.rs`. Frames: `reference/parts/panicend/` — 8792,
8800, 9000 (`reference-indices.md` §10); all three are byte-identical.

### 13.1 `getpal` reads from DAC entry 0 because `0d` is decimal

**Original.** `mov al, 0d` in the tree's assembler is the decimal zero (hex needs `0dh`, as
`TWEAK.ASM:200` writes), so the DAC read index is 0 and the fade tables are built from the inherited
palette unrotated; the intended rotation by thirteen never runs. **Where.**
`PANIC/SHUTDOWN.C:148`. **Port must.** Read from entry 0. **Frame.** 8792 and 8800: tables built
from an unrotated palette reproduce both byte for byte, a 13-entry rotation does not.
**Test.** `getpal_reads_from_dac_entry_zero_because_0d_is_decimal`.

### 13.2 `copyline` moves 80 words and the VGA latch stores the pair's second byte twice

**Original.** `rep movsw` with the count in words, in GC write mode 1: the destination data is the
latch, loaded by the source read, and a 16-bit read leaves the second planar byte's value, which the
16-bit write stores to both destination bytes. **Where.** `PANIC/ASMYT.ASM:9-24`,
`SHUTDOWN.C:70-79`. **Port must.** Each 8-pixel group becomes the source's second 4-pixel group
twice. **Frame.** 8792 and 8800: the collapsed line is 234 pixels at x = 40..276 with x = 273..275
black; a straight copy gives 233 at 44..276. **Test.**
`copyline_moves_eighty_words_and_stores_the_second_byte_of_each_pair_twice`.

### 13.3 The fade tables start at byte 3, so entry 0 is black in all 64

**Original.** `for(b=3;b<768;b++)` over zero-initialised tables. **Where.**
`PANIC/SHUTDOWN.C:62`. `:81`'s white entry 0 is overwritten at `:84`. **Port must.** Start at 3.
**Frame.** 8792/8800 (entry 0 is the black the display shows) and 9000 (entry 0 = (0,0,0), entry 1 =
(1,1,1)). **Test.** `the_fade_tables_leave_entry_zero_black_and_start_at_byte_three`.

### 13.4 The collapse ends at `a = 3` and writes even rows only

**Original.** `for(a=32;a>2;a=a*5/6)` runs `32, 26, 21, 17, 14, 11, 9, 7, 5, 4, 3`; the third loop's
destinations are `200 + 2(b - a/2)` — all even, so `a = 3` writes rows 198, 200, 202 and the two
closing copies black 202 and 198, leaving row 200. **Where.** `PANIC/SHUTDOWN.C:91-105`. **Port
must.** Keep the loop bound and the row arithmetic. **Frame.** 8792 (the `a = 7` pass) and 8800 (the
`a = 3` pass, row 200 alone). **Tests.** `the_handoff_matches_the_collapse_8792_and_the_line_8800`,
`the_reference_frames_are_the_collapse_and_the_line_they_describe`.

## 14. TECHNO — the pattern window that only moves down, the rotation that stops 64 bytes short, and the polygon whose first edge is not clipped (slot 7)

Tests are in `bins/secondreality/src/parts/techno.rs`. Frames: `reference/parts/techno/interference/`
(4900, 5000, 6300, 8100) and `troll/` (8200, 8300, 8400) (`reference-indices.md` §9). `setpalarea`'s
count is §4.

### 14.1 `patdir` only moves towards smaller offsets

**Original.** `patdir` is set to -3 at rows ≡ 0 or 4 (mod 8) and never to +3, so the palette
window's `>= 24` wrap fires only from below 0. **Where.** `TECHNO/KOEB.ASM:509`, `:512` (first
pass), `KOEA.ASM:827`, `:830` (second pass); the step is `add si,cs:patdir` at `KOEB.ASM:421`,
`KOEA.ASM:758`. **Port must.** Keep the negative-only step. **Frame.** 4900 and 5000 (first pass,
fading to white), and the second pass's exact live captures (`techno.md` §4.1; no second-pass
frame is filed). **Test.** `the_first_pass_reaches_white_at_its_two_hundred_and_first_iteration`
(asserts `palanimc == 9` after 101 iterations at -3).

### 14.2 `rotate1` leaves the last 64 bytes as they were

**Original.** `mov cx,32000/32-2` rotates 31,936 of 32,000 bytes; the first carry comes from the
caller's flags and is zero on the shipped path. **Where.** `TECHNO/KOEA.ASM:605-627`. **Port
must.** Copy the chained rotate including the short count. **Frame.** The second pass's exact
captures confirm all eight rotations (`techno.md` §7, §4.1); none is filed. **Test.**
`rotate1s_last_sixty_four_bytes_are_untouched`.

### 14.3 `clipanypoly`'s first edge is the pre-clipped closing edge

**Original.** `cap3` ends with `jmp cap35`, which skips the first loop iteration's clipping.
**Where.** `TECHNO/POLYCLIP.ASM:63` (`cap3:`), `:78` (`jmp cap35`), `:94` (`cap35:`). **Port must.** Not model it as a pseudo-point: that
produces a sparse picture and fails 6300 at once. **Frame.** 6300 (the bars). **Test.**
`the_box_renderer_fills_the_clipped_polygon`.

## 15. MNTSCRL — the font buffer's extra byte, the forced-black bands, the one-step shortfall and the exact cue (slot 9)

Tests are in `bins/secondreality/src/parts/mntscrl.rs`. Frames: `reference/parts/mntscrl/` — 500
(`.idx`), 1100, 1700, 2300 (`reference-indices.md` §11); all four are byte-identical.

### 15.1 `fbuf`'s loop runs one byte past the array, and the release reads it

**Original.** `for x := 0 to sizeof(fbuf) do if fbuf[x] > 0 then inc(fbuf[x],128)` covers index
19,841, one past `array[0..640*31]`; in the original that byte is `font[0]`. The release's `scp`
bound is 640, so a stride-640 read at column 640 reaches exactly that element for row 30 (D24, §7 of
the note). **Where.** `FOREST/READ2.PAS:119` (`:88` is the tree's 639 cap). **Port must.** Keep an
explicit overrun byte and apply the bias to it. **Frame.** 2300: byte-identical under 640 and 401
bytes / 146 pixels off under the tree's 639 (`mntscrl.md` §7). **Tests.**
`the_bias_includes_the_loops_one_byte_overrun`, `the_releases_scp_bound_is_sixty_forty_where_the_tree_says_639`.

### 15.2 The forced-black bands and their ramp back

**Original.** `fpal` is `pal` with entries 0-31 and 128-159 zeroed, uploaded, and the pre-roll's odd
ticks ramp entries 0-176 back up; entries 177-255 stay where `:148` left them. **Where.**
`FOREST/READ2.PAS:126-127`, `:146-147`, `:165-178`. **Port must.** Both the black bands and the ramp,
at the odd-tick rhythm. **Frame.** 500's `.idx` and palette are mid-ramp (`reference-indices.md`
§11.1); the ramp's end state is on 1100-2300. **Test.**
`the_pre_roll_uploads_on_odd_frames_and_ramps_entries_zero_to_176`.

### 15.3 The pre-roll's last upload is one step short on a blue-63 channel

**Original.** The last odd iteration uploads step 62 and then steps to 63, so a channel whose
target is 63 is displayed at 62 until the fade-out's first upload; entry 151 is the one such channel
in the bands. **Where.** `FOREST/READ2.PAS:165-178` (the same loop). **Port must.** Upload before
stepping. **Frame.** No filed frame isolates the entry: 1100/1700/2300 are RGB. **Test.** the same,
`the_pre_roll_uploads_on_odd_frames_and_ramps_entries_zero_to_176` (pins entry 151).

### 15.4 The fade-out fires at `dis_musplus() == -11` exactly

**Original.** `cmp dx,-11 / jnz` — not `<=`, not `< -10`. **Where.** `FOREST/READ2.PAS:200`.
**Port must.** Fire on -11 only; -10 and -12 keep scrolling with the full palette, -11 starts the
65-tick fade. A relaxed test changes the tick the fade begins. **Frame.** None filed: the cue is at
tick 1,979 (`mntscrl.md` §6) and the keyframes stop at 2300's scroll; the position is
`reference/music/second3`. **Tests.** `the_cue_is_exactly_minus_eleven`,
`the_fade_out_is_sixty_five_ticks_and_ends_the_part`.

## 16. LNS&ZOOM — the aspect block's place (slot 10)

Tests are in `bins/secondreality/src/parts/lnszoom.rs`. Frames: `reference/parts/lnszoom/` — 2700,
3400, 4200, 4900, 5650 (`reference-indices.md` §12). The 1,536-byte overrun at `LENS/MAIN.C:302` is
an index row: its source bytes are all zero.

### 16.1 `rotate`'s aspect-ratio block runs after the displacement table is patched

**Original.** The 307/256 scaling of `xadd`/`yadd` comes after the table is built and before the row
loop, so the in-row sample steps keep the uncorrected vector and only the row-to-row advance is
scaled. **Where.** `LENS/ASM.ASM:274-417`. **Port must.** Keep the order; the "obvious" order is a
different animation. **Frame.** 4200 (the rotazoomer). **Test.**
`the_aspect_ratio_scales_the_row_advance_not_the_in_row_steps`.

## 17. PLZPART — beyond §1: the clear's mask underflow, the cut-short last fade, the double `+160` and the shift precedence (slot 11)

Tests are in `bins/secondreality/src/parts/plzpart.rs`. Frames: `reference/parts/plzpart/` plasma
(6000, 7000, 7600), cube (8500, 9500), band (8400-8800) (`reference-indices.md` §13). The cube's
residuals are registered (A23: 26.161 % at 8500, 99.483 % at 9500), so no cube frame is exact.

### 17.1 `do_clear`'s zero-count mask underflows into the previous table

**Original.** `end_mask[x-1]` at `x = 0` reads the byte before the table, `start_mask`'s last
`0x08`, not the `0x0F` the periodic pattern would give. **Where.** `PLZPART/PLZA.ASM:350-363`.
**Port must.** Read the same flat table, with the 16-bit displacement wrapping. **Frame.** None
shows it, measured: replacing the underflowed byte by the `0x0F` the periodic pattern would give
leaves 8500 and 9500 at exactly 26.161 % and 99.483 % (**an uncommitted mutation**: in
`Plzpart::do_clear` replace the `end_mask` read by `if x == 0xFFFF { 0x0F } else { <the read> }`, run
`cargo test --release -p secondreality --test plzpart -- --nocapture`, which prints the two
percentages and passes, then `git checkout` the file), so the keyframes never take the `x = 0` case.
It is pinned at the table level. **Test.**
`do_clears_mask_underflow_reads_the_byte_before_end_mask`.

### 17.2 The last drop step resets the split and accumulates nothing

**Original.** `do_drop`'s 65th step sets the line compare to 400 and re-initialises the parameters
and returns before the accumulate loop, so **every** table's fade is cut short by one step (31
accumulating steps, 66..96, of the 32 the 65..96 range holds). **Where.**
`PLZPART/COPPER.ASM:189-268`. **Port must.** Return at step 65 without accumulating (steps 66-96
and 128-255 do). **Frame.** 7000 and 7600, both after the first switch: letting step 65 accumulate
moves 7000 from 5.848 % to 70.000 % differing pixels and 7600 from 4.052 % to 70.000 % (**an
uncommitted mutation**: delete the `return;` after `self.initpparas();` in `CopperState::do_drop`, and
raise the two expected percentages in `tests/plzpart.rs` so the run reaches 7600; `cargo test
--release -p secondreality --test plzpart the_plasma_keyframes -- --nocapture` prints both; then `git
checkout` the two files); 6000 is before the first switch and does not move. The unmutated figures
are `cargo test --release -p secondreality --test plzpart -- --nocapture`. **Test.**
`the_last_drop_step_resets_the_split_and_adds_nothing`.

### 17.3 The projection adds 160 twice

**Original.** `xxx = (xx*256)/zz + 160 + 160`. **Where.** `PLZPART/VECT.C:198`. **Port must.** Add 320.
**Frame.** `band/` 8400-8800 (the projection fit, 0.46 % mean mask difference). **Test.**
`the_cube_projection_is_the_releases_centred_scale` (x = `(125*256)/445 + 160 + 160`).

### 17.4 `count_const` keeps the C precedence

**Original.** `>>15+7` is `>>22`, and `cyx`/`cyy`/`czx`/`czy` subtract inside the final shift.
**Where.** `PLZPART/VECT.C:166-199`. **Port must.** Shift by 22 and subtract inside. **Frame.** Cube
8500/9500 (residual registered). **Test.** `count_const_keeps_the_c_precedence_shifts`.

## 18. MINVBALL — beyond §2: the inert `mov ax,ax`, the clobbered offset, the skipped gravity and the uninitialised `j` (slot 12)

Tests are in `bins/secondreality/src/parts/minvball.rs`. Frames: `reference/parts/minvball/` — 10900,
11200, 11800, 12400 (`reference-indices.md` §14); all four are byte-identical (A19).

### 18.1 The z rotation drops the low word's borrow, so `mov ax,ax` and the stale `bx` are inert

**Original.** Where the rotation's low word should have been saved (`mov bx,ax`) the source has a
no-op (`mov ax,ax`). The subtraction is `sub ax,bx` / `sub dx,cx` (`:69-70`, object bytes
`2b c3 2b d1` — `grep -obUaP '\x2b\xc3\x2b\xd1' upstream/SecondReality/DOTS/ASM.OBJ upstream/SecondReality/DOTS/DOTS.EXE` finds them once each, at offsets 782 and 3926 —): `sub dx,cx` ignores the carry, and `ax` is overwritten at `:74`, so the stale `bx`
(the previous dot's screen offset) never reaches `bp`. `bp = hi(Z*cos) - hi(X*sin) + 9000`. **Where.**
`DOTS/ASM.ASM:65`, `:69-70`, `:74`. **Port must.** Subtract the high words and nothing else:
carrying a borrow in from a register, or carrying `bx` across the 512 dots, is not the original. (The
port once did, and the four keyframes differed by 67 / 63 / 37 / 25 pixels for it; removed in Phase 9
Task 9, A19.) **Why it stays a full entry.** With no `sbb`, "fixing" `mov ax,ax` itself (saving
the low word) changes nothing, but the natural reading of the routine — a 32-bit subtract that
carries — moves `bp` by one where the low word borrows, and a borrow moves a shadow row at some Z: the
port's own former variant of it moved all four keyframes (67 / 63 / 37 / 25 pixels). The entry guards
that regression. **Frame.** The four keyframes, now 0 differing pixels each. **Test.** `the_stale_bx_never_reaches_bp` (a dot at `Z = -3228`
has `bp` 8193 and shadow row 163 whatever the previous dot left, alone or after another dot).

### 18.2 The off-screen shadow stores a clobbered register as the next offset

**Original.** The three 32-bit restores leave `eax` holding the old ball's third row; `mov
ds:[si+6],ax` stores its low word, not the shadow row, and the next frame restores from it as a pixel
offset. **Where.** `DOTS/ASM.ASM:186-195`. **Port must.** Store that word; keep the 64 KB segment so
the restore reads defined bytes. **Frame.** The four keyframes (byte-identical). **Test.** `the_shadow_restore_bug_stores_the_old_balls_third_row_word`.

### 18.3 The gravity integration is skipped while the shadow is off screen

**Original.** `ja @@2` jumps past `yadd += gravity`, so an off-screen dot's `y` holds still until
its shadow returns. **Where.** `DOTS/ASM.ASM:96`, `:105`. **Port must.** Integrate only on the
shadow's on-screen branch. **Frame.** The four keyframes. **Test.**
`an_off_screen_shadow_holds_the_dots_y_still` (the off-screen dot keeps `y` 100 and `yadd` 7; the
same dot on screen integrates to 110 / 10).

### 18.4 `j` is never initialised

**Original.** `int a,b,c,d,i,j,mode;` declares `j` with the uninitialised locals and `i=dottaul[j]`
reads it, so which dot draws a coincident position depends on the stack. **Where.**
`DOTS/MAIN.C:87`, `:200`. **Port must.** Start at **88**, the value the reference's overlaps fit
(`INITIAL_J`); the drawing is identical at every dot's position, only the coincident-pixel winner
moves. **Frame.** All four keyframes: 0 pixels differ at 10,900 under 88, and the next best
candidate leaves 283, so the fit is unique.
**Tests.** `the_initial_j_is_fitted_to_the_reference` (ignored; the fit command is in `minvball.md`
§4), `the_setup_is_deterministic_and_its_permutation_is_fixed`.

## 19. RAYSCRL — the refill, the fade loops and the fade-out's source (slot 13)

Tests are in `bins/secondreality/src/parts/rayscrl.rs`. Frames: `reference/parts/rayscrl/` — 13000,
13500 (`.idx`), 14000, 14500 (`reference-indices.md` §15); 13000 and 13500 are byte-identical and
14000 / 14500 differ by 3 and 42 pixels (the release's `MIEKKA.SCI` strip, an asset difference, `rayscrl.md` §6).

### 19.1 The refill writes one band below, reads a byte past `fbuf`, and stops at column 390

**Original.** `move(fbuf[1],fbuf,sizeof(fbuf))` copies 5,373 bytes, one past `array[0..158*34]`
(the byte is `scp`'s low byte); `fbuf[158+x*158] := font[x*400+scp]` writes at `158*(x+1)`, one band
below its sample, so index 0 never receives one; `if scp < 390 then inc(scp)` stops ten columns short
of the 400-wide source. **Where.** `WATER/DEMO.PAS:183-185`.
**Port must.** Keep all three. **Frame.** 14000 and 14500 (the font overlay's strip). **Test.**
`the_refill_is_one_band_down_reads_a_byte_past_fbuf_and_stops_at_390`.

### 19.2 The fade-in is 127 iterations and its inner loop steps the next entry's red

**Original.** `for y := 0 to 63*2` is 0..126; the `pf := 0 to 3` loop steps `pal[x*3+3]`, the next
entry's red, so every red but entry 0's advances twice a pass. At the k=14 upload the displayed red
is `2k+1 = 29`, green and blue 14. **Where.** `WATER/DEMO.PAS:122-126`. **Port must.**
Reproduce the double step. **Frame.** 13000, byte for byte (35 colours, mean 23.1535). **Tests.**
`the_fade_in_is_one_hundred_twenty_seven_iterations_and_pins_tick_zero`,
`the_fade_loops_step_the_next_entrys_red_and_read_the_byte_past_the_array`.

### 19.3 The last upload is one step short on the seven entries whose blue target is 63

**Original.** The 63rd upload (y = 125) displays those blues at 62 and then steps the stored array
to 63. **Where.** `WATER/DEMO.PAS:122-126`. **Port must.** Upload before stepping. **Frame.** 13500: its palette is MIEKKA's with exactly
seven blue channels at 62 and no other channel short. **Test.** the same,
`the_fade_in_is_one_hundred_twenty_seven_iterations_and_pins_tick_zero`.

### 19.4 The fade-out uploads `pal`, the array the fade-in left

**Original.** `WATER/DEMO.PAS:167-178` is the same loop downward and uploads `pal`, not the palette the screen was
showing: its first upload moves both ways (red down a step; the seven blue-63 entries up a step to
63). **Port must.** Upload the stored array. **Frame.** None filed: the first fade-out upload is at
boot 14950, which only an unfiled partner capture holds (`rayscrl.md` §5). **Test.**
`the_cue_is_exactly_minus_eleven_and_fades_sixty_five_ticks` (entry 39's blue is 63 on the first
upload).

### 19.5 The fade is cued at `dis_musplus() == -11`, confirmed seven ticks

**Original.** The fade-out fires on `cmp dx,-11 / jnz`, not `<=`; a relaxed test moves the tick the
fade begins (A18's seven-tick confirmation applies to it). **Where.** `WATER/DEMO.PAS:162`. **Port
must.** Fire on -11 only. **Frame.** None: the cue is at boot 14,942, after the last keyframe (14500).
**Tests.** `minus_ten_and_minus_twelve_never_trigger_the_cue`,
`the_cue_is_exactly_minus_eleven_and_fades_sixty_five_ticks`.

## 20. 3DSINFLD — the 18-row clear that moves the copy, and the masked coordinates (slot 14)

(`cameralevel`, written and never read, is an index row.)

Tests are in `bins/secondreality/src/parts/coman.rs` and `bins/secondreality/tests/coman.rs`.
Frames: `reference/parts/3dsinfld/` — 15500, 16000, 16500, 16800 (`reference-indices.md` §16);
exact against two of three sessions (0.0000 %).

### 20.1 `docopy`'s clear advances `di`, so the copy starts at row 70

**Original.** `mov di,52*80 / mov cx,18*80/4 / rep stosd` clears 18 rows and leaves `di` 1,440 bytes
on, so the 140-row copy lands at destination row **70**, not 52; rows 52..69 stay black, rows
200..209 never show, and vbuf's last 10 rows are never displayed. After the copy, rows 68..97 of
vbuf are cleared. **Where.** `COMAN/ASM.ASM:178-183` (and `ASM.ASM:216-229` for the vbuf clear).
**Port must.** Clear first, then copy from row 70. **Frame.** All four. **Tests.**
`docopy_doubles_each_column_and_starts_below_its_eighteen_row_clear`,
`docopy_shifts_by_one_row_a_startrise_unit`.

### 20.2 `docol` masks all four coordinate arguments with `~1`, and the wave tables are byte offsets

**Original.** `mov ax,not 1` then `and si/di/cx/dx,ax` makes every coordinate even; `si` and `di`
are 16-bit byte offsets into the 64 KB wave tables, and `mov bx,fs:[si]` masks the low bit.
**Where.** `COMAN/ASM.ASM:115-124`. **Port must.** Mask, and index the tables as bytes. **Frame.**
All four. **Test.** `the_ray_march_matches_the_shipped_generated_program` (byte for byte).

## 21. JPLOGO — the crop and its border, the zero remap, its own sine table and the palimpsest guard (slot 15)

Tests are in `bins/secondreality/src/parts/jplogo.rs`. Frames: `reference/parts/jplogo/` — 17300,
17500, 17700 (`reference-indices.md` §17); 17300 and 17500 are exact, 17700 is the settled state
(A27: the filed picture is outside its own census). `setpalarea`'s count and the DAC clear are §4.

### 21.1 The crop is 184 bytes from x = 70 with a synthetic 65 at byte 184

**Original.** Each of the 400 rows keeps 184 bytes from x = 70 and sets byte 184 to 65, a white
border the zoom samples as source index 184. **Where.** `JPLOGO/JP.C:191-196`. **Port must.** Crop
and border exactly. **Frame.** 17300 and 17500. **Test.**
`the_rows_are_the_184_byte_crop_remapped_with_a_65_border` (and
`scale_184_is_the_identity_over_the_185_source_bytes` for the entry's strip).

### 21.2 Palette entry 64 is blacked and source 0 is remapped to 64

**Original.** Entry 64 is zeroed and every source `0` becomes `64`, so black in the artwork is drawn
through entry 64. **Where.** `JPLOGO/JP.C:188-190` (entry 64), `:198-201` (the remap). **Port
must.** Both. **Frame.** 17300 and 17500. **Test.** the same.

### 21.3 JPLOGO's sine table differs from the common copy

**Original.** `SIN1024.INC` here holds ±255 at indices 256 and 768 where the common copy has ±256.
**Where.** `JPLOGO/SIN1024.INC:10` (index 256 is 255) and `:26` (index 768 is -255), counting the `dw`
words from the label. **Port must.** Use this file's table.
**Frame.** 17300/17500 (the wobble). **Test.**
`the_sine_table_is_the_sources_own_1024_words` (all 1,024 words from the shipped file).

### 21.4 `doit` redraws only rows whose `(b, a)` changed — the palimpsest

**Original.** `if(lasty[y] != b || lasts[y] != a)` skips a repeated row, which keeps whatever an
earlier state put there; how far `frame` jumps between draws is the host's speed, which is why the
hold's pixels are the session's (`jplogo.md` §6). **Where.** `JPLOGO/JP.C` `doit` (`:72-104`).
**Port must.** Keep the guard and the deterministic model of the settled state. **Frame.** 17500
(exact) and 17700 (the settled `ec3de2bb…`, 40.1914 % off the filed picture, registered as A27).
**Test.** `the_doit_guard_is_a_palimpsest`.

## 22. TUNNELI — the unchecked read past `rows` (slot 6)

**Original.** The y bound is never checked: `bx = 2*yy` indexes `rows` (`array[0..200] of word`),
so a `yy` outside 0..200 reads the data segment's neighbours (`aa2`, `sinit`, `cosit`, `sade`
forward; `aa1` and `pcalc` backward past the 64 KB wrap). The only bound is on x. **Where.**
`TUNNELI/TUN10.PAS:181-186` (`cmp di,319 / ja @yli`, then `add bx,bx` and `add di,word ptr ds:rows[bx]`). **Port must.** Model the read;
substituting 0 for the word is visible, a clamp is not. **Frame.** `reference/parts/tunneli/` —
3800, 4200, 4500, 4600 (`reference-indices.md` §8): the run takes the branch 356,042 times, all into
a row the reference inks 34 pixels of, and zeroing the read repaints that row (320 pixels), so the
read is held visible by those frames; a *bounded* read draws byte-identical frames, so no frame can
say which of the two models the original is. **Tests.**
`zeroing_the_read_would_paint_row_zero`, `the_out_of_range_read_is_invisible_and_that_is_measured`,
`a_dot_with_an_out_of_range_y_is_plotted_at_the_words_the_read_lands_on`,
`the_unchecked_read_reads_the_data_segments_own_words`.

## 23. GLENZ — beyond §5: the 16-bit products, the palette's upload-then-compute order and the clear before the paint (slot 5)

Tests are in `bins/secondreality/src/parts/glenz/mod.rs`. Frames: `reference/parts/glenz/` — 1499
(`.idx`, held), 2300, 3000, 3400 (`reference-indices.md` §7).

### 23.1 `int` is 16 bits and the products wrap

**Original.** Microsoft C large model: every product in `main` is a 16-bit `imul` with the low word
kept. `yposa = -yposa*boingm/boingd` passes 32,767 in the intro and wraps (-4,016×10 → 25,376),
which moves the jello trajectory (`xscale` is 115 at frame 800); `oyp -= b*b/2` from frame 1801
wraps every few frames. **Where.** `GLENZ/MAIN.C:511`, `:477`. **Port must.** Store `i16` and route
products through `mul16`. **Frame.** 2300, 3000 and 3400 are all drawn from the post-wrap state. **Tests.**
`the_sixteen_bit_products_wrap_like_the_source`, `the_object_switches_follow_the_scales`.

### 23.2 The DAC is uploaded before the new palette is computed

**Original.** The displayed palette for main frame N is `pal` as it stood at the end of frame N-1,
which is why the frame-790 `pal = backpal` copy shows from 791. **Where.** `GLENZ/MAIN.C:415` (the upload, ahead of the per-frame palette code at `:540-590`) and
`:586-589` (the `memcpy(pal,backpal,16*3)` for frames below 795). **Port must.** `upload_low_palette` then `MainState::step`. **Frame.** 2300 is main
frame 738 (before 790); the boundary itself falls between filed frames. **Test.**
`the_palette_phases_have_the_source_boundaries`.

### 23.3 Frames 765..789 clear `bgpic` and `vram` before `cglenzdone` paints

**Original.** The band clear is in the physics, before the paint. **Where.** `GLENZ/MAIN.C:546-556` (the
`memset(bgpic+y*320,0,640)` / `memset(vram+y*320,0,640)` pair for frames 765..789). **Port must.** Run the clear, then `draw_objects`. **Frame.** 2300. **Test.**
`the_band_clear_is_at_rows_150_onwards_and_stops_at_789`.

## 24. GRID (vault) — beyond §6: the eye shift that stops 100 bytes short, and the dispatch default

Tests are in `bins/secondreality/src/parts/vault/grid.rs`. Frames: `reference/parts/grid/` — 400-800.

### 24.1 `eye[u] <<= 3` stops at `u < 64684`

**Original.** The image is `eye[784..64783]`, so the loop leaves the last 100 bytes (row 199,
x = 220..319) raw: palette indices 24..28, the eye palette's cyan group 3 under the line tints.
**Where.** `GRID/MAIN.C:141`. **Port must.** Shift 63,900 bytes. **Frame.** 400-800 (bottom row).
**Tests.** `the_eye_shift_stops_100_bytes_short`, `the_buffers_start_as_the_same_shifted_eye`.

### 24.2 Every dispatch entry the generator did not fill points at the first routine

**Original.** `lineblit_init` fills the 64 KB table with the first entry, `lblt_54b56`, the line to
(-10,-8); segments with dy = 56 (segment 746 of every iteration) or, from iteration 24, dx = 82
draw that fill word once per line. **Where.** `GRID/ASM.ASM:213-217`, `LINEBLIT.INC:2-4`. **Port
must.** The same default. **Frame.** 400-800 (the (-10,-8) artefacts match everywhere else, A41).
**Test.** `the_line_blit_is_the_generated_xor_bresenham`.

## 25. HARD (vault) — beyond §7-§9: the dot engine's unsigned clips and colour decrement, the 16-bit z load, and the record that is never read

Tests are in `bins/secondreality/src/parts/vault/hard/`. Frames: `reference/parts/hard/` — all nine
keyframes are exact (480, 560, 630, 700, 770, 840, 910, 1050, 1600); the dot frames are 560-770.

### 25.1 `sar 6`, then unsigned clips

**Original.** `dot3d` draws when `y <= 127` and `x <= 319` as *unsigned* compares after the shift, so
a negative coordinate is clipped, and the x advance is skipped when y is out. **Where.**
`HARD/BA.ASM:75-122`. **Port must.** Clip as unsigned words after the arithmetic shift. **Frame.**
560-770 are drawn through it (exact); no test or frame isolates a negative coordinate. **Test.**
`the_dot_engine_leaves_the_66_frame_trail` (pins the record, the restore, the life, the clips and the
decrement).

### 25.2 `dot3d2`'s `dec dl`

**Original.** The colour decrements every frame, so a mode-4 dot spawned at 254 ends at 189 after 65
frames and the 66th call kills it undrawn; the 930-955 hold's 100 white pixels are those last
positions. **Where.** `HARD/BA.ASM:144`. **Port must.** Decrement and draw in that order. **Frame.**
910 (the mode-4 band). **Test.** `the_dot_engine_leaves_the_66_frame_trail`.

### 25.3 `do3dots` loads z's low word and signed-skips above 3000

**Original.** `mov bp,ds:[si+8] / cmp bp,3000 / jg @@2` reads the 16-bit low word of the dword z and
compares it signed; a negative low word passes and indexes past `depthcol`. **Where.**
`HARD/BA.ASM:232-234`. **Port must.** Load the low word. (The word a negative z would read is
modelled as zero; the index records that this branch is unreachable.) **Frame.** 1050 (part 2's grid).
**Tests.** `the_shadow_grid_dot_skips_positive_z_and_models_the_negative_word`,
`the_part2_grid_z_range_and_drawn_words_are_the_measured_domain`.

### 25.4 Record 0 of a dot stream is never read

**Original.** The spawner's `dotsp += 4` precedes the first read and a terminator resets `dotsp` to
0, so the next read is record 1 again: a stream of R records yields `N - N/R` dot calls. **Where.**
`HARD/BP.C:62`, `HARD/B.C:251-261`. **Port must.** Add before reading. **Frame.** 560-770 (the
streams' dots). **Test.** `the_dot_streams_are_the_bdo_extraction` (and
`the_first_spawn_is_the_sources_rand_sequence`).

## 26. TWIST (vault) — the 16-bit row arithmetic and the texture the generator shipped

Tests are in `bins/secondreality/src/parts/vault/twist.rs`. Frames: `reference/parts/twist/` — 103,
203, 303, 403, 503, 603 (all exact).

### 26.1 `rows(frame)`'s two traps

**Original.** `ra` is a 16-bit `int` incremented per frame and `r += ra` wraps; `w` is then reduced
by `twister()`'s modulo-200 clamp, so the sine peak selects row 0, not 199, and frame 327's final add
wraps (200·328 = 65,600). **Where.** `TWIST/MAIN.C:52-58`, `TWIST/TWIST.ASM:26-34`. **Port must.**
`ra = frame + 1` as a `u16`, `(y as u16).wrapping_mul(ra)`, C's truncating `/`, `w % 200`. **Frame.**
103-603. **Test.** `the_selected_rows_are_the_sources_16_bit_phase`.

### 26.2 The shipped include's left edge

**Original.** `TWSTLOOP.INC`, generated from `TMP.UH`, holds 121 rows whose first byte is `0x3F`
(the generator's order-dependent quirk: a visible four-pixel blue left edge). **Where.**
`TWIST/DOL.C:108` (`v[0]=63`), the include `TWIST/TWSTLOOP.INC`. **Port must.** Consume the include
as shipped and not re-derive a clean texture. **Frame.** 103-603. **Test.**
`the_texture_table_is_the_shipped_twstloop`.

## 27. DDSTARS — the star clip is assembled out

**Original.** The tree's star clip is compiled out; the text is visible and the stars are not
clipped, so the band's star rows are drawn. The shipped build has no clip either: the four
keyframes reproduce with 0 differing bytes *with* 1,025 clip-row pixels present. **Where.**
`DDSTARS/STARS.ASM:692-696`, `:711-716`, `:737-742`, `:756-762` (`cmp di,_nostar1/_nostar2`, commented out; `ddstars.md` §6). **Port must.** Not clip. **Frame.**
`reference/parts/ddstars/` — 1900, 3000, 5000, 8000 (`reference-indices.md` §22). **Test.**
`the_star_clip_is_assembled_out_and_the_keyframes_hold_its_pixels`.

## 28. The music player — five behaviours of the shipped player that every part's timing or sound stands on

No `.rgb` frame shows any of these; their evidence is the traces under `reference/music/`
(boot-numbered `(order, row)` per frame) and, for the sample-level ones, the segment bytes read by
`cargo xtask music probe --dump`. Tests are in `crates/sr-audio/`.

### 28.1 A restart keeps what `_zplaysong`'s initialiser does not write (D45)

**Original.** `restartmus` runs `_zstopsong`, `_zshutup`, `_zinitmodule` and `_zplaysong`; the
initialiser's loop and its silence pass never store to `avol`, `a0volcut`, `aequ`, `aadr`'s high
word, `ac2spd`, `atreon`, `atrigcnt`, `atremor`, `vibtretype`, `a0clearcnt` or `m_poslow`, so a
mid-song restart keeps the previous song's channel volumes. **Where.** `MAIN/U2.ASM:348-397`
(`restartmus`), `MAIN/STMIK.300` initialiser at `+0x5614`…`+0x56b1`, `+0x5781` (the table in D45).
**Port must.** `Player::restart_from(&old)` before `play_song`, and `Channel::initialise` writing
exactly the initialiser's stores. **Timing it decides.** Block 1's MUSIC0 hands channel 2 over at
volume 63, so MUSIC1's order 0 (pattern 73) takes the `C00` at row 3: **4 rows, not 64**, and the
port publishes `1:0` at 8,087 (0:4 without the carry-over). It moved every later seam by about
245 frames (`second1d` +247 → +5; D45's table). **Frame.** None; the music traces `second1d`,
`second1e`, `second1f`. **Tests.** `block_ones_music1_restart_takes_order_0s_break`
(`crates/sr-audio/tests/trace.rs`), `the_song_initialiser_writes_exactly_what_initialise_writes`
and `a_boot_starts_on_the_objects_zeroed_channels` (`tests/player.rs`),
`a_block_one_run_is_alku_then_u2a_then_pam_in_the_boots_numbering` (shell).

### 28.2 The "volume 0" optimisation frees a silent channel before a pattern break can run

**Original.** MUSIC1's order 0 row 3 channel 2 is a `C00`; the channel has been silent since its
row 0 note-off, so the first-tick pass counts three such rows and frees it before the dispatch
table is reached, and the break never runs. **Where.** `MAIN/STMIK.300` `+0x3c77`…`+0x3cad`
(on because both modules' header flags words are 8). **Port must.** Free the channel first.
**Timing.** +243 polls, +180 ticks, +60 rows over a player that dispatches the freed channel's
command. **Test.** `the_volume_zero_optimisation_stops_a_pattern_break_from_running`
(`tests/effects.rs`). (`music.md` §8 notes that `trace.rs` cannot see this one, since its run starts
at order 42.)

### 28.3 The `A` effect has no upper bound: parameter 0x7F is speed 127

**Original.** MUSIC1's pattern 72 row 48 (order 98) carries `A7F`; the object's `A` handler
(`+0x3dd2`) has no bound, so the speed becomes 127 on that row's first tick and the sixteen rows
from the cell to the end of order 98 each take 127 ticks, the `A`'s own row included. **Port must.**
Treat any parameter as speed. **Timing.** +2606 polls and +1937 ticks. **Test.**
`music1s_a7f_row_takes_127_ticks_and_takes_its_own_row_with_it` (`tests/effects.rs`).

### 28.4 The output-curve builder's `rep stosw` clobbers its own count (A14)

**Original.** `+0x559d`'s `rep stosw` leaves `CX` = 0, so `+0x559f`'s `add %cx,%cx` doubles zero and
`+0x55a1`'s `rep stosb` writes nothing: the zero region is `2·⌊(0x800 − K)/4⌋` bytes — half of the
intended length — and `+0x55bd` fills the remainder with `0xFFFF`. For `mul` 64 that is 768 zeros,
the ramp at 768..1279 and `0xFF` from 1280, which the probe read from the running segment. **Where.**
`MAIN/STMIK.300` `+0x5565`…`+0x55c1`. **Port must.** `Curve::build` writes these counts, bug
included; the mixer indexes it at `0x400 + value`. **Sound it decides.** Every sample the card
plays. **Frame.** None: the emulator runs silent (D17). **Tests.**
`the_curve_is_the_objects_own_table` (`crates/sr-audio/src/mix.rs`; literal boundary entries),
`the_files_hand_arithmetic_is_the_builders_at_every_entry`,
`a_solo_voice_is_audible_through_the_table_the_machine_holds` (`tests/mix.rs`).

### 28.5 The shell never writes `[0x8d7]`, so stereo plays 5.26 % flat

**Original.** The mixing rate is `_zmixspeed` (mono) or `[0x8d7]` (stereo); the shell doubles
`_zmixspeed` to 40000 for SB Pro (`MAIN/U2.ASM:759`) and never writes `[0x8d7]`, so stereo voices
divide by the segment's initial **21111** where the frame rate is 20000: 94 cents flat. **Where.**
`MAIN/STMIK.300` `+0x539b`…`+0x53a9`. **Port must.** Keep the object's word. **Sound it decides.**
Every stereo voice's pitch. **Frame.** None (silent emulator). **Test.**
`a_step_is_the_objects_two_divisions` (`crates/sr-audio/src/mix.rs`).

---

## The index: preserved behaviours no frame can show, and the reason each cannot

These are preserved because the original does them, and each is **dead, unreachable, or invisible**
on the shipped path — fixing it would not change a drawn pixel, a palette value, a sample or a
timing, so by the filter at the top it is a row and not an entry. A row is a statement that was
measured or read from the source, not a place to put an inconvenient behaviour: where the note holds
a counterfactual test (a "fix" that *would* show), the row names it.

| Part | Behaviour | Source | Why no frame can show it | Held in |
|---|---|---|---|---|
| ALKU | The final `dofade(palette2,palette)` runs only when `f>7`, and `fonapois()` after either branch | `ALKU/MAIN.C:147-150` | The fade touches entries 64-255, which the picture never indexes | `alku.md` §6 |
| PAM | `ruudulle`, a second copy of the unpacker, is never called | `PAM/ASMYT.ASM:207-320` | Dead code | `pam.md` §7 |
| PAM | Steps 41-45 of the 45-step loop draw nothing and only re-queue copy 63 | `PAM/OUTTAA.C:46-49` | The re-queue is the closing white hold, already the wash (§11) | `pam.md` §7 |
| U2A | `fov` is uninitialised when first read | `VISU/C/U2A.C:261`, `:287`, `:348` | The first draw is background-only, so the stack value never reaches a pixel | `u2a.md` §6 |
| U2A | The second gate is dead: END's `resetscene(); xit=1` ends the loop before order 12 row 55 | `VISU/C/U2A.C:351-355` | Nothing after the loop's end runs; kept first in the port's loop (`the_second_gate_tears_down_on_the_first_tick_it_is_open`) | `u2a.md` §6 |
| U2A | The city hacks force sort distances for `co[2]`, `co[7]`, `co[13]`, `co[14]` | `VISU/C/U2A.C:301-310` | `U2A.00M`'s byte 15 is 0, so neither branch runs; implemented and guarded (`the_city_hacks_are_u2a_s_and_guard_out_of_range_instances`) | `u2a.md` §6 |
| U2A | The teardown clears all four pages and names page 3 the draw page | `VISU/C/U2A.C:432-441`, `AVID.ASM:341-358` | The display never moves during teardown and every page holds the same cleared background | `u2a.md` §6 |
| U2E | `fov` is uninitialised when first read | `VISU/C/U2E.C:378` | The first draw is skipped (§12.3), so the read is invisible | `u2e.md` §6 |
| U2E | The city hacks are dead: `city` is assigned and never read | `VISU/C/U2E.C:18`, `:212-213` | `U2E.00M` byte 15 is 0 and nothing in U2E reads `city` | `u2e.md` §6 |
| TECHNO | `setborder` returns before its assembly | `TECHNO/KOE.C:68-80` | The Scroll Lock border is not a frame; no call has an effect | `techno.md` §7 |
| TECHNO | `flash(-2)` returns `pal1` without capturing or uploading; `mode16` writes `flash_pal` and uploads nothing | `TECHNO/KOE.C:82-103` | No DAC write | `techno.md` §7; `flash_minus_two_uploads_nothing` |
| TECHNO | `mov cx,400` in `init_interference` is reloaded before it is read | `TECHNO/KOEB.ASM:356`, reloaded at `:360` | The register's first value is never used | `techno.md` §7 |
| TECHNO | The KOEB plane-8 loop is behind `jmp @@OVER3`; `sizefade` stays 0 and its `add ax,256` is commented out | `TECHNO/KOEB.ASM:454` (`jmp @@OVER3`), `:524` (`;add ax,256`) | The first pass's display start is the constant 8020 either way; the port keeps `first_pass_move` with `p1_sizefade` 0 | `techno.md` §3 |
| TECHNO | `power1` is computed and never read | `TECHNO/KOE.C:173-211` | A dead table | `techno.md` §3 |
| TECHNO | `dis_partstart()` is called a second time | `TECHNO/KOE.C:327` | The version check draws nothing and waits on nothing | `techno.md` §5 |
| TECHNO | `pic = halloc(20000,4)` is 80,000 bytes and the segment poke folds the second read into it | `TECHNO/KOE.C:317` | The 400 decoded rows are identical either way (`the_troll_rle_is_the_origin_readers_rows`) | `techno.md` §6 |
| PANICEND | The part never polls the exit flag: `SHUTDOWN.C` contains no `dis_exit()` call, so it runs all 121 waits whatever the key state | `PANIC/SHUTDOWN.C:1-157` (`grep -c dis_exit` prints 0) | The reference is captured with no key pressed, so no frame is at an exit | `panicend.md` §9-10 |
| MNTSCRL | `frame = veke` can never fire: `inc(frame)` is commented out | `FOREST/READ2.PAS:3`, `:221-224` | Dead bound (`the_dead_veke_bound_never_ends_the_part`) | `mntscrl.md` §8 |
| LNS&ZOOM | `memcpy(back+64000, back+64000-1536, 1536)` copies the picture's last 1,536 bytes into the pad past it | `LENS/MAIN.C:302` | The copied bytes are all 0 (`tail -c 1536 assets/LENS/LENS.EXB | od -An -v -tu1 | tr -s ' ' '\n' | sort -u | grep -v '^$'` prints only `0`; the picture ends at file offset 64,784) and the port's buffer starts at 0, so the copy changes nothing: with it removed (**an uncommitted mutation**: replace `self.back[VIDEO_BYTES..].copy_from_slice(&tail);` in `Lnszoom::init` by `let _ = tail;`) the three filed lens/rotazoomer residuals stay 3.2156 / 3.0234 / 2.1812 % (`cargo test --release -p secondreality --test lnszoom -- --nocapture` prints them) and `the_tables_are_bounded_and_the_overrun_is_the_1536_byte_copy` still passes, so that test pins the buffer's layout, not the copy's contents | `lnszoom.md` §2 |
| LNS&ZOOM | `if(dis_musplus>-30)` tests the function pointer, so the wait always runs | `LENS/MAIN.C:104` | At the part's entry the player publishes -5 (`the_entry_pre_roll_lands_the_reference_position_at_tick_zero`), so the wait passes either way; the counterfactual at -32 is `the_missing_call_in_the_part1_guard_is_preserved` | `lnszoom.md` §3 |
| LNS&ZOOM | The `a > 3` clamp on the frame advance | `LENS/MAIN.C:176` | The port runs one frame a tick, so `frame` never advances by more than one (the original's advanced by `min(waitb(),3)`) | `lnszoom.md` §4 |
| LNS&ZOOM | `setpalarea` at `:200` is erased by `inittwk`'s DAC clear | `LENS/MAIN.C:200` | Uploaded and cleared inside one tick | `lnszoom.md` §5 |
| LNS&ZOOM | `rotpic90 = back` overwrites the picture buffer; `rotatez` is never called | `LENS/MAIN.C:191`, `LENS/ASM.ASM:419-564` | Nothing reads `back` after part 3 begins; `rotatez_is_dead` | `lnszoom.md` §5-6 |
| PLZPART | `plzline` writes 84 bytes into an 80-byte row | `PLZPART/ASMYT.ASM:41-69` | The last four land in the 96-byte stride's gap, which the view never shows | `plzpart.md` §1, §9 |
| PLZPART | `do_drop`'s `dtau` reaches 404 | `PLZPART/COPPER.ASM:175-181` | The split walks off the bottom of the 400-line screen; the port writes 405, which is inert | `plzpart.md` §9 |
| PLZPART | `timetable[4] = 2778` can never be reached | `PLZPART/PLZ.C:46` | The exit fires 65 ticks after the previous switch | `plzpart.md` §9; `the_timetable_events_happen_at_the_getmframe_thresholds` |
| PLZPART | The stale `PLZPART/MAIN.ASM` | `PLZPART/MAIN.ASM` | Not linked; `MAIN.C` is the entry point | `plzpart.md` §9 |
| RAYSCRL | `pal[768]` is `font[0]` and `tmppal[768]` is `pal[0]`: the `pf = 3` step at x = 255 touches index 768 | `WATER/DEMO.PAS:125`, `:174` | With palette entry 0 black `pal[0]` stays 0 and the font is never touched (`the_fade_loops_step_the_next_entrys_red_and_read_the_byte_past_the_array`) | `rayscrl.md` §5 |
| 3DSINFLD | `combg` is `char combg[16]` and every use is an overrun; `combguse` is read only under `PXLSUX`, which the build does not define | `COMAN/MAIN.C:8`, `:166-176`, `ASM.ASM:143-172` | There is no background; the overrun's destination (DAC 240..255) is unreachable (colour ≤ 127) | `3dsinfld.md` §2 |
| 3DSINFLD | The palette formula, the `*9/6` pass, the bar at 232..255 and the `combg` overwrite of 240..255 | `COMAN/MAIN.C:127-164` | Every displayed colour is ≤ 127; entry 0 is forced black (`the_palette_is_the_formula_with_the_bar_and_the_unreachable_overrun`) | `3dsinfld.md` §2 |
| 3DSINFLD | The frame-4 `setpalarea` re-uploads the palette in force | `COMAN/MAIN.C:69-72` | Same bytes (the wait it spends is not dead) | `3dsinfld.md` §3 |
| 3DSINFLD | `frame < 4444` | `COMAN/MAIN.C:57` | The music break ends the part first (`the_part_ends_on_the_music_break_not_the_dead_frame_cap`) | `3dsinfld.md` §3 |
| 3DSINFLD | `cameralevel` is written (-270), loaded and stored, and never read | `COMAN/MAIN.C:54`, `THELOOP.INC` | Nothing reads `theloop_heigth` | `3dsinfld.md` §7 |
| JPLOGO | The interpolation reads `framey1[200]`/`framey2[200]` at `a = 796..799` | `JPLOGO/JP.C:164-171` | `doit` reads the tables only for `frame <= 511` (`the_interpolation_is_the_sources_own_and_its_overrun_is_unreachable`) | `jplogo.md` §2.1 |
| JPLOGO | `if(frame>511) c=400;` is a skipped assignment: `c` is never read again | `JPLOGO/JP.C:78` | The freeze is `y1`/`y2` keeping their values, which the port writes as `frame <= FREEZE_AFTER` | `jplogo.md` §4 |
| JPLOGO | `scrolly` writes CRTC `0x0D` then `0x0C` | `JPLOGO/JP.C:30-45` | The order matters only to hardware; the port's origin is the 16-bit pair | `jplogo.md` §1 |
| JPLOGO | `doit`'s `halt`, `storea`, `ysb`; `calc` (`:53-65`) never called; `lineblit` (`ASM.ASM:76-100`) never called; the `storea = a` and its `printf` | `JPLOGO/JP.C:69`, `:109`, `:220-225` | Dead | `jplogo.md` §8 |
| JPLOGO | The run opcode `0x80` writes 256 copies (an 8-bit count that wraps: `dec ah / jnz l4`) | `JPLOGO/READP.C:44-51` | The shipped writers never emit it; `sr-assets` refuses it (`a_0x80_run_opcode_wraps_to_256_copies`) | `jplogo.md` §2.2 |
| GLENZ | The `frame>1400+789` bscale-shrink arm is unreachable: `frame>1220+789` catches every value first | `GLENZ/MAIN.C:483`, `:490` | Kept in source order | `glenz.md` §5 |
| GLENZ | `NEW.ASM`'s 256-word row table is followed in memory by the edge arena | `GLENZ/NEW.ASM:11-16` (`nec`, `nep`, `ne`) | Not shown reachable: the worst shipped case is near y = 252 and the port's list is a `Vec`; an open item in `glenz.md` §8, registered only if a moving keyframe reaches it | `glenz.md` §8 |
| GRID | `sin256` and the 64 KB `muldata` are built and read by nothing linked; `_asmloop2`, the unused slot words and `_line` | `GRID/MAIN.C:38-43`, `:130-139`, `ASM.ASM:113-134`, `:247-269` | Dead | `grid.md` §2 |
| GRID | `d = sin1[k&1023]/4+256; d = 0;` is a dead store: the y waveform is static | `GRID/MAIN.C:162-163` | The overwritten value is never read | `grid.md` §6 |
| HARD | `depthcol[bp]` with a negative low word reads a code byte (0x07), modelled as zero | `HARD/BA.ASM:232-234` | Over part 2's 501 states the low word is always positive (`the_part2_grid_z_range_and_drawn_words_are_the_measured_domain`) | `hard.md` |
| HARD | `do3dots`'s shadow store to `[si+12]` is behind `jmp @@4`; the word clear that precedes it (`:229-231`) runs every iteration | `HARD/BA.ASM:229-235` | The store is unreachable, so `[si+12]` stays 0; the port's one-byte-per-row clear is a *model* of it, differing only in bytes 1/321 at offset 0, which no reached frame depends on (`hard.md` "The dot engine") | `hard.md` |
| HARD | `_GRID1.TMP`'s last byte is `0x4F` in the linked copy and `0x00` in the file | `HARD/B.C:112-121` | The affected word is the y of the 100th pair; no filed keyframe depends on it (`the_grid_table_is_the_shipped_grid1`) | `hard.md` |
| HARD | The `count>400` branch and the `0xff` cut; `setborder` | `HARD/B.C:243-246`, `:26`, `:196`, `:201` | Dead / inert (`the_0xff_cut_is_unreachable_with_one_start`) | `hard.md` |
| TWIST | `line()`/`leftline`/`rightline` are linked but never called; the `memset` and `vram[a]=a` are overwritten before the first display; the 8-byte record's last six bytes are never read | `TWIST/MAIN.C:30-42`, `ASM.ASM:97-218` | None is observable | `twist.md` §2 |
| CRED | `tw_setsplit(400)` | `CREDITS/MAIN.C:165` | A line compare of 400 below a 200-row display is inert | `cred.md` §1 |
| ENDSCRL | The scan's end-of-buffer case reads past `text[64000]` | `ENDSCRL/MAIN.C:103-122` | Unreachable in the shipped text: the part ends 184 lines before it | `endscrl.md` §4 |
| ENDLOGO | The surface is not cleared and DAC entry 255 is never written | `END/END.C:66` (`//inittwk();`), `:90-91` (the white loop stops at byte 764, entry 254) | The first is index 0 anyway (`the_opening_screen_is_the_shells_surface_through_the_shells_dac`); the second is displayed by no path (`the_white_leaves_entry_255_unwritten`) | `endlogo.md` §4 |
| Music | The order list's `254` and `255` are kept as bytes; `254` is stepped over and `255` ends or loops the list | `MUSIC0.S3M`/`MUSIC1.S3M` | No order the demo plays reaches a `255`; the `254`'s effect (RAYSCRL's `zplus`) is §28's trace | `music.md` §4, §7; `the_order_list_is_preserved_byte_for_byte` |
| Music | The period clamp `[64, 32767]` | `MAIN/STMIK.300` `+0x5876`…`+0x5896` | Inaudible: the 2,408 / 2,635 probe hits above `0x7FFF` are all on volume-0 channels | `music.md` §10 |
| Music | The flags word's bit 4 would write the clamped period back; both modules carry flags 8 | `MAIN/STMIK.300` `+0x583d`, `+0x586e`, `+0x589a` | The write-back never runs | `music.md` §10 |
| Shell | `error2` says "a number between 2 and 5" where `SECOND 1` is accepted | `MAIN/U2.ASM:557-559` | Text-mode wording; the port's refusal text is its own | `part-sequence.md` §4 |

## Not in this register, and why

These came up while classifying and are deliberately elsewhere:

- **ALKU's pass-2 `dis_sync` race (D42).** The first `dis_sync()` of the loop's second pass races the
  restarted player (`ALKU/MAIN.C:59`, `DIS/DISINT.ASM:449-463`): in 1 session of 7 it starts early
  (`reference-indices.md` §25.3). It is the host-timing term (D17), session-dependent, not a
  deterministic behaviour a test can pin. The port reproduces the filed majority and does not carry
  the minority; the carried-ring counterfactual is A55 / D42.
- **A55** — the original's stopped tick feeds order `0xffff` into the position ring at a reload.
  The port does not carry the ring; it is a registered approximation (`APPROXIMATIONS.md`), not a
  behaviour the port preserves.
- **The BEGLOGO/ENDLOGO doubled picture** Task 6 fixed was a port defect (an inherited line
  offset), not the original's.
- **A14(a)'s marker.** §28.4 describes only the builder's count bug. The flush writes 0 and the
  index is `0x400 + value`; A14(a) is restated and closed on it
  (`the_flush_clears_the_accumulator_word_it_reads`).
- **HARD's two extra displays at the mode-4/part-2 boundary** (`hard.md` §"The mode-5 boundary",
  A48): the original's missed retrace, reproduced by two static ticks — a registered timing
  approximation.
- **D24 adoptions** (the release over the tree): MNTSCRL's `scp` 640, PLZPART's exit index 4 and the
  cube's 213/100 projection, GLENZ's low-byte `rolcol` key (A37; the exhaustion it avoids is §5's
  "not in this register"), RAYSCRL's gate confirmation, LNS&ZOOM's forehead asset. Each is an
  `APPROXIMATIONS.md` entry; none is a bug of the original's code.
- **Port deviations recorded in the notes**: ALKU's exit path skips `:147-152`'s three closing
  calls (`alku.md` §6), MNTSCRL's wait honours `dis_exit` where `:153-159` does not
  (`mntscrl.md` §8), PLZPART's frame never polls the exit flag (`plzpart.md` §10). They are what
  the port does differently, not what the original does, so they belong in `APPROXIMATIONS.md`
  — A62.

## The classification: every audited behaviour, and the grep that finds none missing

The audit is the set of lines in `docs/fidelity/*.md` (this file excluded) that call a behaviour
*preserved*, *kept*, *required* or *D7*:

```bash
grep -nEi 'preserv|\bkept\b|\brequired\b|\bD7\b' docs/fidelity/*.md | grep -v '^docs/fidelity/bugs.md'
```

At this commit it prints **72** lines. The check that none is missing from the table below is that
every `file:line` it prints appears as a code span **inside this closing section** (the section is
cut out first, so a mention of the same `file:line` elsewhere in this file does not count):

```bash
sect() { awk '/^## The classification/{f=1} /^## Adding an entry/{f=0} f' docs/fidelity/bugs.md; }
grep -nEi 'preserv|\bkept\b|\brequired\b|\bD7\b' docs/fidelity/*.md | grep -v '^docs/fidelity/bugs.md' \
  | cut -d: -f1,2 | sed 's#^docs/fidelity/##' \
  | while read -r hit; do sect | grep -qF -- "$hit\`" || echo "UNCLASSIFIED $hit"; done
```

It prints nothing. (Line numbers are this commit's: a later edit to a note moves them and the check
reports the hits it moved, which is its job.)

**The wider grep.** Notes also call behaviours "bugs" or "artefacts" without the four words, so the
same pipeline was run with that alternation:

```bash
grep -nEi 'preserv|\bkept\b|\brequired\b|\bD7\b|\bbugs?\b|artefact|artifact' docs/fidelity/*.md \
  | grep -v '^docs/fidelity/bugs.md' | wc -l
```

prints **180** at this commit, **108** of them lines the first grep does not print (`comm -13` of the
two `file:line` lists). Those 108 add no behaviour the table lacks, and each falls into one of these
groups (a line can be in more than one):

- the one-line `**Register.**` back-reference appended to each of 24 part notes by this task (the
  line names `bugs.md`, which `\bbugs?\b` matches) and the §4 `setpalarea` back-references
  (`beglogo.md`, `endlogo.md`, `techno.md`, `panicend.md`), and `APPROXIMATIONS.md`'s
  back-references to this file (A14's §28.4, A57's §27, A62's "Not in this register");
- lines inside a section the first grep already classifies (`alku.md` §6, `rayscrl.md` §5,
  `minvball.md` §4, `plzpart.md` §9, `jplogo.md` §8);
- "artifact" meaning the shipped binary (D24: `grid.md`, `hard.md`, `ddstars.md`, `u2a.md`,
  `reference-indices.md`'s HARD rows, `APPROXIMATIONS.md` A57's "which artifact to follow"), or a
  build, capture or trace artefact (`asset-inventory.md`, `music.md:1638`, `music-clock.md`, most of `reference-indices.md`);
- port-side mentions: the port's own page-slot spill (A23: `plzpart.md`, `APPROXIMATIONS.md`), the
  conversion "bug" at `APPROXIMATIONS.md:216`, and `music.md:762`'s "a fix of a Task 3 timeline bug",
  which is the port's own earlier player (§28.2 is the original's behaviour that fixed it);
- a behaviour classified above under another name: A14's "bug" (§28.4, in `APPROXIMATIONS.md`
  A14 and `music.md` §13's list), GRID's "(-10,-8) fill-word artefacts" (§24.2), HARD's "the fade is the source's bug" (§9) and A48's two extra displays (the
  register's "Not in this register" list).

**Classes.** **E** = a full entry in this file; **I** = a row of the index above; **N** = not a
preserved behaviour of the original's (a port representation, a counterfactual, capture bookkeeping
or a registered approximation), with the reason.

| Hit | The note's behaviour | Class | Where |
|---|---|---|---|
| `alku.md:139` | the code-5 gate (`tptr++`) | E | §3, §10.4 |
| `alku.md:147` | §6: the palette alias; the refresh past the row; the `memset`; the `f>7` fade; the exit path; the 9-tick spin | E, E, E, I, N (a port deviation: A62), N (the cadence is not a bug) | §10.1, §10.2, §10.3; index |
| `3dsinfld.md:495` | §7: the unlinked background; the palette aliasing; the dead frame-4 `setpalarea`; the dead `4444` cap; `cameralevel`; `docol`'s `~1`; `docopy`'s bands; the wave tables | I, I, I, I, I, E, E, E | index; §20.1, §20.2 |
| `beglogo.md:66` | `setpalarea`'s byte count | E | §4 |
| `endlogo.md:43` | `setpalarea`'s byte count (510) | E | §4 |
| `mntscrl.md:66` | the packed font array kept as one buffer | N (the source's own `move` shifts it; a representation) | — |
| `mntscrl.md:204` | §8: the `fbuf` overrun; the `veke` bound; the forced-black bands; the exact cue; the wait honouring `dis_exit` | E, I, E, E, N (a port deviation: A62) | §15.1, §15.2, §15.4; index |
| `mntscrl.md:216` | the forced-black bands and their ramp | E | §15.2, §15.3 |
| `mntscrl.md:232` | the `hback` rebase is not performed; the rebased layout is kept | N (a substitution, `mntscrl.md` §9) | — |
| `grid.md:139` | the eye shift stops 100 bytes short | E | §24.1 |
| `grid.md:199` | the dispatch table's default entry | E | §24.2 |
| `rayscrl.md:101` | the refill (one band below, a byte past, stops at 390) | E | §19.1 |
| `rayscrl.md:163` | the tree's immediate gate "kept as the counterfactual" | N (a counterfactual, A18) | §19.5 |
| `rayscrl.md:175` | §5: the 127-iteration fade; the next-red step; the last-upload shortfall; the fade-out's source; the `pal[768]` alias | E, E, E, E, I | §19.2-§19.4; index |
| `rayscrl.md:180` | the two consequences "both kept" | E | §19.2, §19.3 |
| `jplogo.md:455` | §8: the DAC clear and the 512-byte upload; the sine table; the crop and border; source 0→64; the interpolation read; the freeze's `c=400`; the palimpsest guard; `scrolly`; the dead locals; `lineblit`; the dead switches | E (§4), E, E, E, I, I, E, I, I, I, I | §4, §21.1-§21.4; index |
| `ddstars.md:232` | the star clip assembled out | E | §27 |
| `glenz.md:152` | the frame-790 palette order | E | §23.2 |
| `glenz.md:229` | the 16-bit products | E | §23.1 |
| `glenz.md:252` | the unreachable bscale-shrink arm | I | index |
| `glenz.md:304` | the visible-path `rolcol` key (D7/D24) | N (a D24 adoption, A37; the rejected reading is §5's "not in this register") | — |
| `glenz.md:355` | the clear before the paint | E | §23.3 |
| `lnszoom.md:125` | the missing-call guard | I | index |
| `lnszoom.md:127` | its pinning test (`the_missing_call_in_the_part1_guard_is_preserved`) | I | index |
| `lnszoom.md:182` | the `a > 3` clamp | I | index |
| `lnszoom.md:207` | `setpalarea` erased by `inittwk` | I | index |
| `minvball.md:107` | §4: `mov ax,ax`; the clobbered offset; the skipped gravity; `j` | E, E, E, E | §18.1-§18.4 |
| `music-clock.md:673` | the restart keeps the old module's channel state | E | §28.1 |
| `music.md:169` | the order list kept as bytes (254, 255) | I | index |
| `music.md:172` | `the_order_list_is_preserved_byte_for_byte` | I | index |
| `music.md:370` | order list 254 and 255 kept as data | I | index |
| `music.md:1731` | a part whose cue is *required* | N (a shell requirement, not a behaviour) | — |
| `music.md:2051` | the enhanced renderer keeps eight fraction bits | N (the port's own renderer, D43) | — |
| `pam.md:286` | §7: the FLI header counts; `ruudulle`; the 4-tick spin; the 45-step loop | E, I, N (a cadence), I | §11; index |
| `panicend.md:339` | §10: `getpal`; the fade tables; `copyline`; the collapse; nothing beyond row 0 cleared; no exit poll | E, E, E, E, I, I | §13.1-§13.4; index |
| `part-sequence.md:54` | the spec's part descriptions kept | N (names) | — |
| `part-sequence.md:111` | the port's `Order` numbers kept | N (names) | — |
| `plzpart.md:66` | the 84-byte overrun "kept and invisible" | I | index |
| `plzpart.md:439` | §9: the overrun; the mask underflow; the banding; the cut-short fade; the dead `ttptr`; the 404 split; the double `+160`; `count_const`; the stale `MAIN.ASM` | I, E, E, E, I, I, E, E, I | §1, §17.1-§17.4; index |
| `techno.md:129` | `power1`, the dead table | I | index |
| `techno.md:216` | `flash(-2)` kept as written | I | index |
| `techno.md:294` | `dis_partstart()` twice | I | index |
| `techno.md:398` | the literal `READP.C` transcription kept under `#[cfg(test)]` | N (test code) | — |
| `techno.md:403` | §7 heading | see the next row | — |
| `techno.md:405` | §7: `setpalarea`; `setborder`; `flash(-2)`; `mov cx,400`; the KOEB loop and `sizefade`; `patdir`; `rotate1`; `pic = halloc`; `clipanypoly` | E (§4), I, I, I, I, E, E, I, E | §4, §14.1-§14.3; index |
| `tunneli.md:160` | the unchecked read, preserved as the source's own quirk | E | §22 |
| `tunneli.md:458` | the same read, "preserved because the source has it" | E | §22 |
| `twist.md:194` | the two arithmetic traps | E | §26 |
| `u2a.md:270` | §6: `fov`; the deadlock escape; the burst; the dead second gate; the dead city hacks; the teardown pages; the two exits; the de-interleave; the tick wait; no music | I, E, E, I, I, I, E, N (a port representation), N (cadence), N (a port refusal) | §12.1, §12.2; index |
| `u2e.md:279` | §6: `fov`; the skipped first draw; the deadlock escape; the DAC rotation; the city hacks; the name hacks; the tick waits; no music | I, E, E, E, I, E, N, N | §12.1, §12.3-§12.5; index |
| `reference-indices.md:217` | "a port can be *required* to produce that picture" | N (capture method) | — |
| `reference-indices.md:268`, `reference-indices.md:1503`, `reference-indices.md:1508`, `reference-indices.md:3640`, `reference-indices.md:3718`, `reference-indices.md:4098` | a captured frame "kept its bytes" through a batch | N (capture bookkeeping) | — |
| `reference-indices.md:3843` | the palimpsest "preserves different rows" | E | §21.4 |
| `reference-indices.md:5073` | a read of 480 "optional rather than *required*" | N (capture method) | — |
| `reference-indices.md:5635`, `reference-indices.md:5636`, `reference-indices.md:5639`, `reference-indices.md:5640` | frames "not kept" by the indexed capture | N (capture bookkeeping) | — |
| `APPROXIMATIONS.md:387` | what was "kept from the substitution" (ENDLOGO's cue) | N (a register's own history) | — |
| `APPROXIMATIONS.md:1192` | an earlier version "kept the tree's wait" | N (a register's own history) | — |
| `APPROXIMATIONS.md:1240` | "the exact cue is kept: only `-11`" (RAYSCRL) | E | §19.5 |
| `APPROXIMATIONS.md:2098` | "what is preserved is the pixel set" (a representation) | N | — |
| `APPROXIMATIONS.md:2898`, `APPROXIMATIONS.md:2996` | the restart keeps the old module's channel state / the kept words | E | §28.1 |
| `APPROXIMATIONS.md:14` | earlier text "kept as dated history" | N (the register's own convention) | — |
| `APPROXIMATIONS.md:3112`, `APPROXIMATIONS.md:3123` | an ESC between two parts "kept" by the port where `flushkbd` drops it | N (a port deviation, A58) | — |

The behaviours the audit's grep cannot find because their note never says *preserved*, *kept* or
*required* — LNS&ZOOM's aspect order (§16), HARD's dot engine (§25), the music
player's volume-0 optimisation, `A7F`, curve builder and stereo word (§28.2-§28.5), U2E's
`firstframe`, GRID and HARD's vault entries (§6-§9) and TWIST's texture (§26) — were found by
reading each note's section on the part's own arithmetic, and are entries above so the register
does not depend on a note's word choice.

---

## Adding an entry

An entry needs a measurement behind it, not a recollection: what the original draws (from a reference
frame or a decompiled line), the source that makes it happen, the obligation it puts on the port, and
where the evidence lives. If the port *cannot* reproduce the artefact, that is the other register —
`APPROXIMATIONS.md`, with the measured visual difference (spec §5.2). The line between the two is
intent: this file is for what the original does on purpose-as-shipped and the port must copy;
`APPROXIMATIONS.md` is for what the port knowingly does differently.
