# Bugs to preserve — the original's artefacts, as required behaviour

`CLAUDE.md`'s non-negotiables: **"Preserve the original's bugs. A 'correct' implementation of the
plasma looks wrong."** This file is the register that rule points at (D7; the policy is
`docs/design.md` §5.1). An entry is *required behaviour*, not a defect to fix: a port that renders
the artefact away has broken fidelity, and the reference comparison is what says so.

Each entry gives **what the original does**, **where** (the `upstream/` file and line), **what the
port does** (the item that keeps it), **the frame** that shows it (or why none can) and **the test**
that pins it. A behaviour the original *executes* on the shipped path, where "fixing" it would move
a pixel, a palette value, a sample or a timing, is a full entry (§1-§28). One that is dead,
unreachable or invisible is a row of **the index** at the end. What the port cannot reproduce, or
knowingly does differently, is an `APPROXIMATIONS.md` entry instead. The mechanism lives in the
part's module doc; the measurements live in the part note.

Search `upstream/` with `grep -a`: several files are latin-1 (`PLZPART/TWEAK.ASM`, `ALKU/MAIN.C`,
`PLZPART/COPPER.ASM`), and a plain `grep` exits 1 on them with no match.

---

## 1. The GMX plasma's palette-leak banding — `PLZPART` (slot 11)

**What it is.** Colour from the previous palette table persists into the next table's fade, so the
plasma carries broad colour bands that a cleanly cleared ramp would not. A "fixed" plasma looks flat
and wrong beside it (`docs/design.md` §5.1's first named behaviour, D7).

**Where.** `PLZPART/PLZ.C:44-45` holds the six tables (`pals[6][768]`), built by `init_plz()` from
entry **1** onward (`:179-217`, `pptr=&pals[k][3]` at `:182`). `plz()` (`:62-122`, the switch at
`:73`, `:81-83`) hands them to the fade one at a time (`cop_fadepal = pals[curpal++]`) and resets
with `memset(fadepal,0,768)` (`:81`). `PLZPART/COPPER.ASM`'s `copper2` (`:86-116`, registered by
`init_copper` at `:44-65`) uploads all 768 bytes when `do_pal` is set (`:96-105`). `fadepal` is
`db 768*2 dup(?)` (`:184`): 768 16-bit accumulators, colour in the high byte and fraction in the
low, stepped by `@@l4` (`:238-254`). The `memset` clears only the 768 colour bytes, so each new
table's fade starts from the previous table's fractions.

**Port.** `CopperState` keeps 768 `u16` accumulators; `CopperState::clear_colour_half` keeps the low
byte. Do not substitute an interpolated ramp. The cube's DAC (`VECT.C:123`) is not a cause: its
hand-off is at boot ~8315 (`plzpart.md` §10), after the banded frames.

**Frame.** `reference/parts/plzpart/plasma/` 6000, 7000, 7600 (`reference-indices.md` §13, §1.12:
three keyframes with no settled content). The port's residuals there are A24's tear, every colour
delta within a sixteenth of the palette, not a missing band (`cargo xtask rediff`).

**Test.** `a_table_switch_zeroes_the_colour_half_and_keeps_the_fraction`: after the clear the
accumulators read `0x00F0` / `0x0001` / `0x0080` from `0x2AF0` / `0x3F01` / `0xFF80`, and the next
step (`0xF0 + 0x20`) carries into the colour byte (`0x0110`). A whole-word clear fails it.

## 2. Shadow particles drawn over the particles in front of them — `MINVBALL` (slot 12)

**What it is.** Each dot's shadow and ball are drawn in the dot's own order with no depth sort and
no z-test, so a later dot paints its shadow over a nearer one. The policy names it only for "the
jumping-particle part"; `MINVBALL` is identified from the mechanism, `part-sequence.md`'s slot 12
(`DOTS/`, "MiniVectorBalls") and the captures (`reference-indices.md` §14).

**Where.** `DOTS/ASM.ASM:36` gives each dot its slots (`x,y,z,oldposshadow,oldpos,…`); `:98-108`
restores the shadow's previous pixel and writes `87+87*256`; `:120-179` draws the ball from
`_depthtable1/2/3` (built at `DOTS/MAIN.C:163-166`). One dot per `loop @@1` pass, shadow then ball.

**Port.** `drawdots` keeps the loop and the order. Sorting by depth, or deferring shadows, changes
the picture.

**Frame.** `reference/parts/minvball/` 10900, 11200, 11800, 12400 (§14), all moving at 3/3 support;
the port is byte-identical at all four (`the_four_keyframes_match_the_reference_at_the_mapping`,
`cargo xtask rediff`). Coincident dots resolve by draw order with the fitted `j` (§18.4, A19).

**Test.** `shadows_are_drawn_over_balls_without_a_depth_test`: a near dot (bp 8000) and a far dot
(bp 9000) in both orders. Near first, the far shadow covers the near ball's first two pixels
(`[87, 87, ball, ball]`); far first, the ball covers the shadow.

## 3. `dis_sync()`'s off-by-one — the music-position lookup (library; two callers)

**What it is.** The sync service returns the **previous** table entry's code, and at the start of
the table it reads the word *before* the table (`docs/design.md` §5.1). Callers wait one step later
than the table intends.

**Where.** `DIS/DISINT.ASM`'s `sync_10` (service 10, called by `dis_sync`, `DIS/DISC.ASM:161-169`)
walks `ordersync1` (`0000h,0 / 0200h,1 / 0300h,2 / … / 0d00h,10 / 3d00h,1 / …`) in four-byte steps
to the first entry `>=` the position (`jbe @@1`) and returns `cs:[bx-2]`, the previous entry's code.
The callers: `PAM/OUTTAA.C:39` (`while(dis_sync()<10 && !dis_exit());`) and `ALKU/MAIN.C:59, 66,
72, 79` (the four card gates, `dis_sync()<1..4`) and `:138` (the horizon pan,
`dis_sync()<4+tptr`); `:512-513` documents the codes.

**The table's ends** are A15: at `(0, 0)` the read lands on `pop es; ret` (`07 c3`, the end of
`musframe_9`, `DIS/DISINT.ASM:420-424`), which Turbo C receives as **-15609**, so every
`dis_sync() < N` keeps waiting; past `0x8E2E` the port refuses. No caller reaches the refusal.

**ALKU's batch gate waits for code 5.** `switch(tptr++)` (`ALKU/MAIN.C:102`) leaves `tptr` at 1
before `:138` runs, so the first batch waits for code **5** (`alku.md` §5). Fixing either the
post-increment or the lookup moves a timing.

**Port.** `sr_engine::music::sync_from` walks `ORDERSYNC1` as written, off-by-one included, and
returns `BEFORE_THE_TABLE` below it.

**Frame.** None isolates a timing; the consumers are ALKU (`reference/parts/alku/` 900, 1719, 2719,
5859, §4; codes 1-4 reach at boot 1537, 2097, 2509, 3069 in `reference/music/second1`) and PAM
(`reference/parts/pam/` 7710-7900, §6; `dis_sync()<10` opens at `(13, 1)` on boot 7710,
`reference/music/second1b`).

**Tests.** `sync_reads_the_table_the_object_reads` (26 `(order, row)` cases, among them `(0, 0)`'s
-15609 and the band above the last key), `sync_refuses_the_first_key_past_the_sixteenth_compare`, `the_table_is_the_shipped_shells_own_bytes`
(found once in each of the three shell builds), `the_picture_fade_hands_over_to_a_batch_gated_on_code_five`.

## 4. `setpalarea` uploads `2*count + start` bytes — the ranged DAC write (BEGLOGO, ENDLOGO, JPLOGO and TECHNO)

**What it is.** The ranged palette upload moves **`2*count + start` bytes where it intends
`3*count`**: it reuses `ax`, still holding `start`, as the second addend. A fade over 254 entries
writes two-thirds of them, and the rest keep what the previous write left.

**Where.** `BEG/ASM.ASM:93-115`: `:104-105` set the write index to `start`, `:106` loads `count`,
then `:107 shl cx,1` / `:108 add cx,ax`; `REPOUTSB` (`:84-91`) moves `cx` bytes. In `BEG/BEG.EXE`
`_setpalarea` starts at file offset `0xcb7`, the instructions at `0xccb`-`0xcd1`. The same two lines
are in `END/ASM.ASM:107-108`, `JPLOGO/ASM.ASM:116-117`, `ENDPIC/ASM.ASM:98-99` and
`TECHNO/KOEA.ASM:1054-1055`. Correct copies load `count` into `ax` first: `LENS/ASM.ASM:253-255`,
`START/ASM.ASM:192-194`, `COMAN/ASM.ASM:93-95`, `TWIST/ASM.ASM:85-87`.

**Port.** Each part's upload writes `2*count + start` bytes from the write index:

| Caller | Call | Bytes | Written | Untouched |
|---|---|---|---|---|
| BEGLOGO | `BEG/BEG.C:84`, count 254 | 508 | 0-168 and entry 169's red | 170-255 |
| ENDLOGO | `END.C:105`, `:116`, count 255 | 510 | 0-169 | 170-255 |
| JPLOGO, TECHNO | `setpalarea(palette,0,256)`, `TECHNO/KOE.C` | 512 | 0-169 and entry 170's red and green | 171-255 |

JPLOGO's `inittwk` first writes 768 zeros (`JPLOGO/ASM.ASM:31-38`), so its tail is black. BEGLOGO
and ENDLOGO leave entries 170-254 white (`BEG/BEG.C:25-31`, `END.C:90-91`), and that DAC is what the
next part inherits. `ENDPIC/` stages a `beg.exe` that did not ship (`beglogo.md` §1).

**Frame.** On BEGLOGO's art the bug is invisible: the picture indexes 1-3, 14-58 and 71-79, all
inside the written range, so a 762-byte upload also matches `reference/parts/beglogo/` 430, 470,
560, 900 (`the_fade_constants_are_derived_by_matching_the_reference_bit_for_bit` reruns the fade
scan with 762 bytes and gets the same `c` at 470, 560 and 900). ENDLOGO's 510 is invisible for the
same reason (`endlogo.md` §6). The deciding evidence is the disassembly.

**Tests.** `logo::tests::the_upload_is_two_count_plus_start_bytes` (BEGLOGO's 508 and ENDLOGO's
510, one shared routine),
`the_palette_upload_is_the_sources_512_bytes` and `init_clears_the_dac_before_the_512_byte_upload`
(JPLOGO), `the_palette_upload_is_two_bytes_a_colour_and_truncates_at_512` (TECHNO; start 4 /
count 2 → 8 bytes at offset 4).

## 5. GLENZ's span raster: the 1..319 clamp, the broken same-x merge, the fresh slot's shade, and pixel 63999

Four artefacts of `GLENZ/NEW.ASM`'s compositor (`ng_pass2`/`ng_pass3`, `:174-418`) and
`GLENZ/VEC.ASM`'s colour allocator (`demo_glz`, `:403-506`):

- **The clamp is 1..319.** `ng_pass2` clamps the signed low word of `x>>16` with signed compares
  (`NEW.ASM:246-253`): 0 becomes 1, 320 becomes 319. Tests:
  `boundaries_clamp_to_columns_one_and_319`, `a_row_crossing_span_uses_the_global_position`.
- **The same-x merge compares the wrong register.** The merge (`:255-260`) compares `cx`, which
  `mov cx,ax` (`:267`) loaded with the stored colour word (`cmp %ax,%cx` at `V.EXE` load offset
  0x3daf). GLENZ's colour words are all ≥ `0x4000`, so it never fires. The port emits every
  boundary and sorts by position; XOR commutes, so the pixels are identical
  (`coincident_edges_xor_like_the_merged_edge`).
- **A fresh slot is shaded 0 and its R is the scan's residual.** `VEC.ASM:454-460`: the scan
  (`mov cx,15` / `loop`, then `mov ax,bp`) clobbers CL and zeroes AH, so the shade is 0; for a
  bit-1 word R is `16 − slot/2`, G = B = 0. A reused slot keeps its shade. Tests:
  `glz_fresh_allocation_uses_the_scan_residual_and_zero_shade`, `glz_allocates_even_slots_in_order`,
  `glz_group_ramp_restores_the_base_and_clamps_each_entry`.
- **Pixel 63999 is never painted.** `ng_pass3`'s sentinels end `(63999, 0), (-1, 0)` (`:279-285`).
  Tests: `edges_off_the_screen_are_dropped_or_clipped`, `the_full_repaint_equals_the_source_two_list_walk`.

The port never clears between frames (the background is `bgpic`; the full repaint is A39). The
slot-32 exhaustion (`glz_pool_exhaustion_uses_slot_32_and_entry_0`) belongs to the full-word key
the reference rejects, so it is A37's, not this register's.

**Frame.** `reference/parts/glenz/` 1499 (held) and 2300/3000/3400 (`glenz.md` §11).

## 6. GRID's `sinus1` reads an uninitialised `d` — `GRID` (the vault)

**What it is.** `sinus1`'s third cursor `d` is never initialised, so every curve depends on the
stack. The object predicts `0x116`; the shipped `P.EXE` measures `0x7FE (mod 0x2000)`.

**Where.** `GRID/MAIN.C:33` declares `d`; `:51` zeroes only `b` and `c`; `:51-58` advance
`d += 67` and index `sin1[(d>>3)&1023]`. An exhaustive match over 0..65535 against the capture's
982-pixel developer screen finds only the `0x7FE` class (D24: the binary is the artefact).

**Port.** Start from `d0 = 0x07FE`. **Frame.** `reference/parts/grid/` 400-800, which differ only
by A41's sampling residual (`grid.md` §2). **Tests.** `the_initial_d_is_the_measured_0x7fe`,
`the_measured_d_is_the_only_residue_whose_tables_match`.

## 7. HARD's `part2init` loop reads an uninitialised `b` — `HARD` (the vault)

**What it is.** The `depthcol` loop reads `int b` in `a < 1000 || b >= 3000` before any assignment;
`a = 0` assigns it and the short-circuit keeps every later read after an assignment, so the table
is 127 for 0..1031 and `127 - (a-1000)/32` down to 62 at 3099.

**Where.** `HARD/BP.C:124-132`. **Port.** Reproduce the table (`b` starts at 0, never read).
**Frame.** `reference/parts/hard/` 1050 is exact; `depthcol[3000] = 65` makes the first grid
frame's 100 pixels `(1,63,63)` (A48). **Test.** `part2init_reads_the_uninitialised_b`.

## 8. HARD's mode-4 band forces entries 189 and 1 to white — `HARD` (the vault)

**What it is.** The falling ramp forces `pal[254-65]` = `pal[189]` and `pal[1]` to `(63,63,63)`.
The trail's last colour is 189, so the forced entries are the white trail pixels mode 5 rebuilds.

**Where.** `HARD/B.C:281-286`; the mode-5 rebuild tests `vram[u]==254-65` and writes index 1
(`:297-301`). **Port.** Force 189 and 1; the 66-entry ramp is `a = 252 - pal1colp`, clamped at
254. **Frame.** `reference/parts/hard/` 910 is exact (`hard.md`, "The palettes are the source's
byte writes"). **Test.** `the_mode_four_band_palette_is_the_sources`.

## 9. HARD's part-2 fade dims by 255/256, not by its intended ramp — `HARD` (the vault)

**What it is.** `fadepal(pal,pal2,a*16)` with `a = (500-16)-count` is negative for every count that
reaches it (485..499). `fadepal`'s unsigned `cmp dx,256` clamps `dl` to 255, so each step is
`src*255/256` and the intended fade never runs.

**Where.** `HARD/BP.C:173-176`, `HARD/BA.ASM:174-215` (`mov al,[si]; mul dl; mov [di],ah`, the blue
bias, the clamp). **Port.** Keep the clamp and the high-byte multiply. **Frame.**
`reference/parts/hard/` 1050 and 1600 carry the dimmed palette (`hard.md`, the part-2 fade).
**Test.** `fadepal_is_the_high_byte_multiply`.

---

## 10. ALKU — the picture's palette alias, the refresh that reads past the row, the text buffer's short clear, and the gate for code 5 (slot 1)

Frames: `reference/parts/alku/` 900, 1719, 2719, 5859 (`reference-indices.md` §4), all exact. The
entries hold for the release's image as well as the tree (`alku.md` §12).

### 10.1 The palette write-order alias — `palette[192..768] = palette[..576]`

**Original.** One loop repeats entries 0-63 three times, so entry `v` is entry `v mod 64`, and the
text's XOR (`0x40/0x80/0xC0`) and `and 0x3f` leave the picture unchanged under the text.
**Where.** `ALKU/MAIN.C:212`. **Frame.** 1719 and 2719 (cards mid-hold), exact. **Test.**
`the_picture_is_hoi_u_with_the_three_entries_the_release_ships`.

### 10.2 The refresh reads past the picture's row

**Original.** The refresh at `a = 316` reads the dword `(a/4+86)*4`, columns 660-663 of a 640-column
picture: the next row's 20-23. The release's loop runs to `a = 321` (`alku.md` §12), so its last
refresh reads the next row's 24-27, in columns the window does not show. **Where.**
`ALKU/MAIN.C:407-408`. **Frame.** None: the filed 5859 is at `a = 309`. **Test.**
`the_picture_blit_writes_the_band_and_the_refresh_reads_past_the_row`.

### 10.3 `memset` clears 186×320 of a 186×352 text buffer

**Original.** `char far tbuf[186][352]` (`:45`) is cleared with `memset(tbuf,0,186*320)`, so the
array's last 16 rows' worth survive each batch. **Where.** `ALKU/MAIN.C:101`. **Port.**
`TBUF_CLEAR` = 186×320. **Frame.** None isolates it: the next `fmaketext` redraws those rows.
**Test.** `the_tbuf_clear_keeps_the_arrays_last_sixteen_rows`.

### 10.4 The batch gate waits for code 5 (the post-incremented `tptr`)

See §3. **Frame.** None (a music position, `reference/music/second1`). **Test.**
`the_picture_fade_hands_over_to_a_batch_gated_on_code_five`.

## 11. PAM — the animation loop draws 40 of the container's 42 frames (slot 3)

**Original.** `PRAX4.FLI`'s header counts 41 frames and `OUT.ANI` holds 42; the loop
`while(!dis_exit() && f++<45)` with `if(f<=40)` draws **40**, and steps 41-45 only queue the closing
white copy. **Where.** `PAM/OUTTAA.C:46`, `:49` (`VFLI.C:66` reads the header word, unused).
**Port.** Draw 40. **Frame.** `reference/parts/pam/` 7760 and 7830 are exact
(`reference-indices.md` §6.2); no filed frame stands on frames 40 or 41 (7900 is A21's white
field). **Test.** `four_ticks_a_step_and_forty_of_the_containers_frames_drawn` (the 40), and
sr-assets' `the_fli_decodes_to_the_frames_its_own_record_walk_finds` (41 and 42).

## 12. U2A and U2E — the page ring's escape, the first draw U2E skips, and U2E's one-tick-late palette rotation (slots 2 and 16)

Frames: U2A 6500, 7300, 7640 (`reference-indices.md` §5); U2E 1000, 1600, 2600, 3600 (§18).

### 12.1 The deadlock escape and the start-up burst: the page cadence

**Original.** The loop waits on `while(cl[clw].ready)` and breaks at `deadlock>16`, drawing into a
page the display may still hold; the 16-retrace start-up delay makes the first four draws parse 8,
8, 4 and 1 frames. **Where.** `VISU/C/U2A.C:278-283`, `VISU/C/U2E.C:388-391`. **Port.**
`next_page` returns the busy page once the count passes 16. **Frame.** All the U2A and U2E
keyframes are drawn through it (`u2a.md` §5). **Tests.**
`ring_matches_the_u2a_cadence_over_twenty_ticks`,
`the_cadence_over_eight_ticks_advances_four_stream_frames`.

### 12.2 U2A's two exits leave different screens

**Original.** An exit in the gate's wait is a bare `return` (`VISU/C/U2A.C:239`): the pad stays up.
An exit in the loop falls through to the teardown (`:432-441`). **Frame.** None is at an exit.
**Test.** `the_gate_holds_until_the_scripted_cue_and_exits_on_request`.

### 12.3 U2E's first draw is skipped

**Original.** `firstframe`'s branch draws nothing and resets `syncframe`/`coppercnt`
(`VISU/C/U2E.C:433`). **Port.** `Driver::draw_frame` resets without touching the page; the first
tick still queues page 0. **Frame.** None isolates it; every U2E keyframe is drawn from that state.
**Test.** `the_first_loop_tick_draws_nothing_and_queues_the_fadeset_page`.

### 12.4 U2E's teardown writes 768 bytes from DAC entry 255, one tick late

**Original.** Each step is `fpal[a]+=4; dis_waitb(); outp(0x3c8,255);` and 768 bytes
(`VISU/C/U2E.C:543-556`): entry 255 takes `fpal`'s entry 0 and entry `j < 255` takes `j + 1`; step
`n` shows on tick `T+n`, and the part ends on the tick that shows step 16. **Frame.** None: every
filed keyframe precedes it. **Tests.**
`the_teardown_fades_sixteen_steps_from_dac_entry_255_and_finishes`,
`a_sticky_exit_flag_runs_the_whole_sixteen_step_teardown`.

### 12.5 U2E's name hacks run

**Original.** `name[1] == '_'` forces a sort distance of 1,000,000,000 (`VISU/C/U2E.C:408`), and
`currframe > 1800 && currframe < 2200` forces `s01`'s to 1 (`:409-416`, strict bounds); the scene
has nine `_` objects and `U2E.028` is `"s01"`. **Frame.** U2E 2600 and 3600. **Test.**
`the_scene_is_the_u2e_city_with_the_name_hacks_object`.

## 13. PANICEND — `getpal`'s decimal zero, `copyline`'s latch, the fade tables from byte 3, and the collapse that ends at `a = 3` (slot 8)

Frames: `reference/parts/panicend/` 8792, 8800, 9000 (`reference-indices.md` §10), all exact.

### 13.1 `getpal` reads from DAC entry 0 because `0d` is decimal

**Original.** `mov al, 0d` is decimal zero (hex is `0dh`, as `TWEAK.ASM:200` writes), so the fade
tables are built from the inherited palette unrotated. **Where.** `PANIC/SHUTDOWN.C:148`. **Frame.**
8792 and 8800 (a 13-entry rotation does not reproduce them). **Test.**
`the_handoff_matches_the_collapse_8792_and_the_line_8800`.

### 13.2 `copyline` moves 80 words and the VGA latch stores the pair's second byte twice

**Original.** `rep movsw` in GC write mode 1: a 16-bit read leaves the second planar byte in the
latch and the write stores it to both bytes, so each 8-pixel group is the source's second 4-pixel
group twice. **Where.** `PANIC/ASMYT.ASM:9-24`, `SHUTDOWN.C:70-79`. **Frame.** 8792 and 8800: the
line is 234 pixels at x = 40..276 with 273..275 black (a straight copy gives 233 at 44..276).
**Test.** `copyline_moves_eighty_words_and_stores_the_second_byte_of_each_pair_twice`.

### 13.3 The fade tables start at byte 3, so entry 0 is black in all 64

**Original.** `for(b=3;b<768;b++)` over zeroed tables; `:81`'s white entry 0 is overwritten at
`:84`. **Where.** `PANIC/SHUTDOWN.C:62`. **Frame.** 8792, 8800, 9000. **Test.**
`the_fade_tables_leave_entry_zero_black_and_start_at_byte_three`.

### 13.4 The collapse ends at `a = 3` and writes even rows only

**Original.** `for(a=32;a>2;a=a*5/6)` runs `32, 26, 21, 17, 14, 11, 9, 7, 5, 4, 3`; destinations
`200 + 2(b - a/2)` are all even, so `a = 3` writes rows 198, 200 and 202 and the two closing copies
black 202 and 198, leaving row 200. **Where.** `PANIC/SHUTDOWN.C:91-105`.
**Frame.** 8792 (`a = 7`) and 8800 (`a = 3`). **Tests.**
`the_handoff_matches_the_collapse_8792_and_the_line_8800`,
`the_reference_frames_are_the_collapse_and_the_line_they_describe`.

## 14. TECHNO — the pattern window that only moves down, the rotation that stops 64 bytes short, and the polygon whose first edge is not clipped (slot 7)

Frames: `reference/parts/techno/interference/` 4900, 5000, 6300, 8100 and `troll/` 8200, 8300,
8400 (`reference-indices.md` §9). `setpalarea` is §4.

### 14.1 `patdir` only moves towards smaller offsets

**Original.** `patdir` is set to -3 at rows ≡ 0 or 4 (mod 8), never +3, so the `>= 24` wrap fires
only from below 0. **Where.** `TECHNO/KOEB.ASM:509`, `:512`, `KOEA.ASM:827`, `:830`; the step is
`add si,cs:patdir` at `KOEB.ASM:421`, `KOEA.ASM:758`. **Frame.** 4900 and 5000 (`techno.md` §7).
**Test.** `the_first_pass_reaches_white_at_its_two_hundred_and_first_iteration`.

### 14.2 `rotate1` leaves the last 64 bytes as they were

**Original.** `mov cx,32000/32-2` rotates 31,936 of 32,000 bytes; the first carry is the caller's,
zero on the shipped path. **Where.** `TECHNO/KOEA.ASM:605-627`. **Frame.** None filed
(`techno.md` §4.1, §7). **Test.** `rotate1s_last_sixty_four_bytes_are_untouched`.

### 14.3 `clipanypoly`'s first edge is the pre-clipped closing edge

**Original.** `cap3` ends with `jmp cap35`, skipping the first iteration's clip. **Where.**
`TECHNO/POLYCLIP.ASM:63`, `:78`, `:94`. **Frame.** 6300 (a pseudo-point model fails it). **Test.**
`the_box_renderer_fills_the_clipped_polygon`.

## 15. MNTSCRL — the font buffer's extra byte, the forced-black bands, the one-step shortfall and the exact cue (slot 9)

Frames: `reference/parts/mntscrl/` 500 (`.idx`), 1100, 1700, 2300 (`reference-indices.md` §11), all
exact.

### 15.1 `fbuf`'s loop runs one byte past the array, and the release reads it

**Original.** `for x := 0 to sizeof(fbuf)` biases index 19,841, one past `array[0..640*31]`
(`font[0]` in the original); the release's `scp` bound of 640 reads exactly that byte for row 30
(D24, `mntscrl.md` §7). **Where.** `FOREST/READ2.PAS:119` (`:88` is the tree's 639). **Frame.**
2300 (exact under 640; the tree's 639 leaves 146 pixels off). **Tests.**
`the_bias_adds_128_to_non_zero_bytes_and_wraps`,
`the_releases_scp_bound_is_sixty_forty_where_the_tree_says_639`.

### 15.2 The forced-black bands and their ramp back

**Original.** `fpal` is `pal` with entries 0-31 and 128-159 zeroed; the pre-roll's odd ticks ramp
0-176 back; 177-255 stay as `:148` left them. **Where.** `FOREST/READ2.PAS:126-127`, `:146-147`,
`:165-178`. **Frame.** 500 (mid-ramp, §11.1) and 1100-2300. **Test.**
`the_pre_roll_uploads_on_odd_frames_and_ramps_entries_zero_to_176`.

### 15.3 The pre-roll's last upload is one step short on a blue-63 channel

**Original.** The last odd iteration uploads step 62, then steps to 63; entry 151 shows 62 until the
fade-out. **Where.** `FOREST/READ2.PAS:165-178`. **Frame.** None (the later frames are RGB).
**Test.** `the_pre_roll_uploads_on_odd_frames_and_ramps_entries_zero_to_176` (entry 151).

### 15.4 The fade-out fires at `dis_musplus() == -11` exactly

**Original.** `cmp dx,-11 / jnz`. **Where.** `FOREST/READ2.PAS:200`. **Port.** Fire on -11 only;
-11 starts the 65-tick fade. **Frame.** None: the cue is at tick 1,979 (`mntscrl.md` §6),
`reference/music/second3`. **Tests.** `the_cue_is_exactly_minus_eleven`,
`the_fade_out_is_sixty_five_ticks_and_ends_the_part`.

## 16. LNS&ZOOM — the aspect block's place (slot 10)

Frames: `reference/parts/lnszoom/` 2700, 3400, 4200, 4900, 5650 (`reference-indices.md` §12).

### 16.1 `rotate`'s aspect-ratio block runs after the displacement table is patched

**Original.** The 307/256 scaling of `xadd`/`yadd` comes after the table is built, so only the
row-to-row advance is scaled. **Where.** `LENS/ASM.ASM:274-417`. **Frame.** 4200. **Test.**
`the_rotazoomer_composes_160_samples_each_shown_twice_and_100_rows_twice`.

## 17. PLZPART — beyond §1: the clear's mask underflow, the cut-short last fade, the double `+160` and the subtraction inside the shift (slot 11)

Frames: `reference/parts/plzpart/` plasma 6000, 7000, 7600; cube 8500, 9500; band 8400-8800
(`reference-indices.md` §13). The cube keyframes and band 8500/8600 are exact; the other band frames
are A23's.

### 17.1 `do_clear`'s zero-count mask underflows into the previous table

**Original.** `end_mask[x-1]` at `x = 0` reads `start_mask`'s last byte, `0x08`, not the periodic
`0x0F`. **Where.** `PLZPART/PLZA.ASM:350-363`. **Port.** `Plzpart::do_clear` reads the flat table
with a wrapping 16-bit displacement. **Frame.** None: a mutation that reads `0x0F` at `x = 0` leaves
8500 and 9500 at 0 %, so the keyframes never take the case. **Test.**
`do_clears_mask_underflow_reads_the_byte_before_end_mask`.

### 17.2 The last drop step resets the split and accumulates nothing

**Original.** `do_drop`'s 65th step sets the line compare to 400, re-initialises the parameters and
returns before the accumulate loop, so every table's fade has 31 accumulating steps (66..96), not
32. **Where.** `PLZPART/COPPER.ASM:189-268`. **Port.** `CopperState::do_drop` returns after
`initpparas`. **Frame.** 7000 and 7600: a mutation that lets step 65 accumulate moves both to
70.000 %; 6000 precedes the first switch. **Test.** `the_last_drop_step_resets_the_split_and_adds_nothing`.

### 17.3 The projection adds 160 twice

**Original.** `xxx = (xx*256)/zz + 160 + 160`. **Where.** `PLZPART/VECT.C:198`; the release compiles
it to one `add ax,140h` (image `0x131f9`). **Frame.** Cube 8500/9500, band 8500/8600. **Test.**
`the_cube_projection_is_the_trees_142_66`.

### 17.4 `count_const` subtracts inside the final shift (the `>>15+7` precedence is the tree's only)

**Original.** `cyx`/`cyy`/`czx`/`czy` subtract inside the final shift. **Where.**
`PLZPART/VECT.C:166-176`; the release's `count_const` (image `0x12d40`) does each `sub`/`sbb` before
`mov cl,0fh` and `LXRSH@`. The tree's `>>15+7` = `>>22` (`VECT.OBJ`'s `sar $0x16`) is not in the
shipped build, which shifts by 15 and feeds a 1.15 matrix to its 32-bit `rotate` (A23); the port
follows the release. **Frame.** Cube 8500/9500. **Test.**
`count_const_is_the_releases_one_point_fifteen_matrix`.

## 18. MINVBALL — beyond §2: the inert `mov ax,ax`, the clobbered offset, the skipped gravity and the uninitialised `j` (slot 12)

Frames: `reference/parts/minvball/` 10900, 11200, 11800, 12400 (`reference-indices.md` §14), all
exact.

### 18.1 The z rotation drops the low word's borrow, so `mov ax,ax` and the stale `bx` are inert

**Original.** Where the low word should be saved (`mov bx,ax`) the source has `mov ax,ax`
(`DOTS/ASM.ASM:65`). The subtraction `sub ax,bx` / `sub dx,cx` (`:69-70`; bytes `2b c3 2b d1`,
`grep -obUaP '\x2b\xc3\x2b\xd1'` finds them at offset 782 of `DOTS/ASM.OBJ` and 3926 of
`DOTS/DOTS.EXE`) ignores the carry, and `ax` is overwritten at `:74`, so the stale `bx` never
reaches `bp`: `bp = hi(Z*cos) - hi(X*sin) + 9000`. **Port.** Subtract the high words only. Saving
the low word changes nothing, but the natural reading, a 32-bit subtract that carries, moves `bp` by
one where the low word borrows, and that moves a shadow row at some Z. **Frame.** The four
keyframes, exact.
**Test.** `the_stale_bx_never_reaches_bp` (`Z = -3228` gives `bp` 8193 and shadow row 163 whatever
the previous dot left).

### 18.2 The off-screen shadow stores a clobbered register as the next offset

**Original.** The three 32-bit restores leave `eax` holding the old ball's third row; `mov
ds:[si+6],ax` stores its low word, and the next frame restores from it. **Where.**
`DOTS/ASM.ASM:186-195`. **Port.** Store that word, in a 64 KB segment so the restore reads defined
bytes. **Test.** `the_shadow_restore_bug_stores_the_old_balls_third_row_word`.

### 18.3 The gravity integration is skipped while the shadow is off screen

**Original.** `ja @@2` jumps past `yadd += gravity`. **Where.** `DOTS/ASM.ASM:96`, `:105`. **Test.**
`an_off_screen_shadow_holds_the_dots_y_still` (`y` 100 and `yadd` 7 held; on screen 110 / 10).

### 18.4 `j` is never initialised

**Original.** `int a,b,c,d,i,j,mode;` and `i=dottaul[j]` read it, so which dot wins a coincident
pixel depends on the stack. **Where.** `DOTS/MAIN.C:87`, `:200`. **Port.** `INITIAL_J` = **88**,
fitted to the reference: 0 pixels differ at 10900, the next candidate leaves 283 (A19). **Tests.**
`the_initial_j_is_fitted_to_the_reference` (ignored; the fit is `minvball.md` §4),
`the_setup_is_deterministic_and_its_permutation_is_fixed`.

## 19. RAYSCRL — the refill, the fade loops and the fade-out's source (slot 13)

Frames: `reference/parts/rayscrl/` 13000, 13500 (`.idx`), 14000, 14500 (`reference-indices.md`
§15); 13000 and 13500 are exact, 14000/14500 are A18's.

### 19.1 The refill writes one band below, reads a byte past `fbuf`, and stops at column 390

**Original.** `move(fbuf[1],fbuf,sizeof(fbuf))` copies one byte past `array[0..158*34]` (`scp`'s low
byte); `fbuf[158+x*158] := font[x*400+scp]` writes one band below its sample; `if scp < 390` stops
ten columns short. **Where.** `WATER/DEMO.PAS:183-185`. **Frame.** 14000, 14500. **Test.**
`the_refill_is_one_band_down_reads_a_byte_past_fbuf_and_stops_at_390`.

### 19.2 The fade-in is 127 iterations and its inner loop steps the next entry's red

**Original.** `for y := 0 to 63*2` is 127 passes; the `pf := 0 to 3` loop steps `pal[x*3+3]`, so
every red but entry 0's advances twice a pass. **Where.** `WATER/DEMO.PAS:122-126`. **Frame.** 13000
(35 colours, mean 23.1535). **Tests.**
`the_fade_in_is_one_hundred_twenty_seven_iterations_and_pins_tick_zero`,
`the_fade_loops_step_the_next_entrys_red_and_read_the_byte_past_the_array`.

### 19.3 The last upload is one step short on the seven entries whose blue target is 63

**Original.** The 63rd upload shows those blues at 62, then steps the array to 63. **Where.**
`WATER/DEMO.PAS:122-126`. **Frame.** 13500 (exactly seven channels at 62). **Test.**
`the_fade_in_is_one_hundred_twenty_seven_iterations_and_pins_tick_zero`.

### 19.4 The fade-out uploads `pal`, the array the fade-in left

**Original.** The downward loop uploads `pal`, so its first upload moves both ways (red down; the
seven blue-63 entries up a step to 63). **Where.** `WATER/DEMO.PAS:167-178`. **Frame.** None filed (boot
14950, `rayscrl.md` §5). **Test.** `the_cue_is_exactly_minus_eleven_and_fades_sixty_five_ticks`.

### 19.5 The fade is cued at `dis_musplus() == -11`, confirmed seven ticks

**Original.** `cmp dx,-11 / jnz` (A18's seven-tick confirmation applies). **Where.**
`WATER/DEMO.PAS:162`. **Frame.** None: the cue is at boot 14,942. **Tests.**
`minus_ten_and_minus_twelve_never_trigger_the_cue`,
`the_cue_is_exactly_minus_eleven_and_fades_sixty_five_ticks`.

## 20. 3DSINFLD — the 18-row clear that moves the copy, and the masked coordinates (slot 14)

Frames: `reference/parts/3dsinfld/` 15500, 16000, 16500, 16800 (`reference-indices.md` §16).
`cameralevel` is an index row.

### 20.1 `docopy`'s clear advances `di`, so the copy starts at row 70

**Original.** `mov di,52*80 / mov cx,18*80/4 / rep stosd` leaves `di` 1,440 bytes on, so the
140-row copy lands at row **70**; rows 52..69 stay black and vbuf's last 10 rows never show; rows
68..97 of vbuf are cleared after. **Where.** `COMAN/ASM.ASM:178-183`, `:216-229`. **Frame.** All
four. **Tests.** `docopy_doubles_each_column_and_starts_below_its_eighteen_row_clear`,
`docopy_shifts_by_one_row_a_startrise_unit`.

### 20.2 `docol` masks all four coordinate arguments with `~1`, and the wave tables are byte offsets

**Original.** `mov ax,not 1` then `and si/di/cx/dx,ax`; `si`/`di` are byte offsets into the 64 KB
tables. **Where.** `COMAN/ASM.ASM:115-124`. **Frame.** All four. **Test.**
`the_ray_march_matches_the_shipped_generated_program`.

## 21. JPLOGO — the crop and its border, the zero remap, its own sine table and the palimpsest guard (slot 15)

Frames: `reference/parts/jplogo/` 17300, 17500 (exact), 17700 (A27) (`reference-indices.md` §17).
`setpalarea` and the DAC clear are §4.

### 21.1 The crop is 184 bytes from x = 70 with a synthetic 65 at byte 184

**Original.** Each row keeps 184 bytes from x = 70 and sets byte 184 to 65, a white border.
**Where.** `JPLOGO/JP.C:191-196`. **Frame.** 17300, 17500. **Tests.**
`the_rows_are_the_184_byte_crop_remapped_with_a_65_border`,
`scale_184_is_the_identity_over_the_185_source_bytes`.

### 21.2 Palette entry 64 is blacked and source 0 is remapped to 64

**Original.** Entry 64 is zeroed and source `0` becomes `64`. **Where.** `JPLOGO/JP.C:188-190`,
`:198-201`. **Test.** `the_rows_are_the_184_byte_crop_remapped_with_a_65_border`.

### 21.3 JPLOGO's sine table differs from the common copy

**Original.** This `SIN1024.INC` holds ±255 at indices 256 and 768 where the common copy has ±256.
**Where.** `JPLOGO/SIN1024.INC:10`, `:26`. **Frame.** 17300/17500. **Test.**
`the_sine_table_is_the_sources_own_1024_words`.

### 21.4 `doit` redraws only rows whose `(b, a)` changed — the palimpsest

**Original.** `if(lasty[y] != b || lasts[y] != a)` keeps a repeated row's old pixels; how far
`frame` jumps is the host's speed (`jplogo.md` §6). **Where.** `JPLOGO/JP.C` `doit` (`:72-104`).
**Frame.** 17500 (exact), 17700 (A27). **Test.** `the_doit_guard_is_a_palimpsest`.

## 22. TUNNELI — the unchecked read past `rows` (slot 6)

**Original.** Only x is bounded (`cmp di,319 / ja @yli`); `bx = 2*yy` indexes `rows`
(`array[0..200] of word`), so a `yy` outside 0..200 reads the data segment's neighbours. **Where.**
`TUNNELI/TUN10.PAS:181-186`. **Port.** Model the read; substituting 0 is visible, a clamp is not.
**Frame.** `reference/parts/tunneli/` 3800, 4200, 4500, 4600 (`reference-indices.md` §8); a bounded
read draws identical frames, so no frame decides between the two. **Tests.**
`zeroing_the_read_would_paint_row_zero`, `the_out_of_range_read_is_invisible_and_that_is_measured`,
`a_dot_with_an_out_of_range_y_is_plotted_at_the_words_the_read_lands_on`,
`the_unchecked_read_reads_the_data_segments_own_words`.

## 23. GLENZ — beyond §5: the 16-bit products, the palette's upload-then-compute order and the clear before the paint (slot 5)

Frames: `reference/parts/glenz/` 1499 (held), 2300, 3000, 3400 (`reference-indices.md` §7).

### 23.1 `int` is 16 bits and the products wrap

**Original.** Every product in `main` is a 16-bit `imul`. `yposa = -yposa*boingm/boingd` wraps in
the intro (-4,016×10 → 25,376); `oyp -= b*b/2` wraps from frame 1801. **Where.**
`GLENZ/MAIN.C:511`, `:477`. **Port.** `i16` and `mul16`. **Frame.** 2300, 3000, 3400. **Test.**
`the_object_switches_follow_the_scales` (`xscale` 115 at frame 800 only with the wrap).

### 23.2 The DAC is uploaded before the new palette is computed

**Original.** Frame N shows `pal` as frame N-1 left it, so the frame-790 `pal = backpal` shows from
791. **Where.** `GLENZ/MAIN.C:415` (the upload), `:540-590`, `:586-589`. **Port.**
`upload_low_palette`, then `MainState::step`. **Frame.** 2300 (main frame 738). **Test.**
`the_palette_phases_have_the_source_boundaries`.

### 23.3 Frames 765..789 clear `bgpic` and `vram` before `cglenzdone` paints

**Original.** The band clear is in the physics, before the paint. **Where.** `GLENZ/MAIN.C:546-556`.
**Frame.** 2300. **Test.** `the_band_clear_is_at_rows_150_onwards_and_stops_at_789`.

## 24. GRID (vault) — beyond §6: the eye shift that stops 100 bytes short, and the dispatch default

Frames: `reference/parts/grid/` 400-800.

### 24.1 `eye[u] <<= 3` stops at `u < 64684`

**Original.** The image is `eye[784..64783]`, so the last 100 bytes (row 199, x = 220..319) stay
raw. **Where.** `GRID/MAIN.C:141`. **Frame.** 400-800. **Tests.**
`the_eye_shift_stops_100_bytes_short`, `the_buffers_start_as_the_same_shifted_eye`.

### 24.2 Every dispatch entry the generator did not fill points at the first routine

**Original.** `lineblit_init` fills the table with `lblt_54b56`, the line to (-10,-8); segments with
dy = 56, or dx = 82 from iteration 24, draw it. **Where.** `GRID/ASM.ASM:213-217`,
`LINEBLIT.INC:2-4`. **Frame.** 400-800 (A41). **Test.** `the_line_blit_is_the_generated_xor_bresenham`.

## 25. HARD (vault) — beyond §7-§9: the dot engine's unsigned clips and colour decrement, the 16-bit z load, and the record that is never read

Frames: `reference/parts/hard/` 480, 560, 630, 700, 770, 840, 910, 1050, 1600, all exact; the dot
frames are 560-770.

### 25.1 `sar 6`, then unsigned clips

**Original.** `dot3d` draws when `y <= 127` and `x <= 319` as unsigned compares after the shift, and
skips the x advance when y is out. **Where.** `HARD/BA.ASM:75-122`. **Test.**
`the_dot_engine_leaves_the_66_frame_trail`.

### 25.2 `dot3d2`'s `dec dl`

**Original.** A mode-4 dot spawned at 254 ends at 189 after 65 frames and the 66th call kills it.
**Where.** `HARD/BA.ASM:144`. **Frame.** 910. **Test.** `the_dot_engine_leaves_the_66_frame_trail`.

### 25.3 `do3dots` loads z's low word and signed-skips above 3000

**Original.** `mov bp,ds:[si+8] / cmp bp,3000 / jg @@2` compares the dword z's low word signed.
**Where.** `HARD/BA.ASM:232-234`. **Frame.** 1050. **Tests.**
`the_shadow_grid_dot_skips_positive_z_and_models_the_negative_word`,
`the_part2_grid_z_range_and_drawn_words_are_the_measured_domain`.

### 25.4 Record 0 of a dot stream is never read

**Original.** `dotsp += 4` precedes the first read and a terminator resets `dotsp` to 0. **Where.**
`HARD/BP.C:62`, `HARD/B.C:251-261`. **Frame.** 560-770. **Tests.**
`the_dot_streams_are_the_bdo_extraction`, `the_first_spawn_is_the_sources_rand_sequence`.

## 26. TWIST (vault) — the 16-bit row arithmetic and the texture the generator shipped

Frames: `reference/parts/twist/` 103, 203, 303, 403, 503, 603, all exact.

### 26.1 `rows(frame)`'s two traps

**Original.** `ra` is a 16-bit `int` and `r += ra` wraps; `twister()`'s modulo-200 clamp then maps
the sine peak to row 0, and frame 327's add wraps (200·328 = 65,600). **Where.**
`TWIST/MAIN.C:52-58`, `TWIST/TWIST.ASM:26-34`. **Test.**
`the_selected_rows_are_the_sources_16_bit_phase`.

### 26.2 The shipped include's left edge

**Original.** `TWSTLOOP.INC` holds 121 rows whose first byte is `0x3F`, a four-pixel blue left edge.
**Where.** `TWIST/DOL.C:108` (`v[0]=63`), `TWIST/TWSTLOOP.INC`. **Port.** Use the include as
shipped. **Test.** `the_texture_table_is_the_shipped_twstloop`.

## 27. DDSTARS — the star clip is assembled out

**Original.** The star clip is commented out, so the band's star rows are drawn; the shipped build
agrees (the keyframes reproduce with the 1,025 clip-row pixels present). **Where.**
`DDSTARS/STARS.ASM:692-696`, `:711-716`, `:737-742`, `:756-762` (`ddstars.md` §6). **Frame.**
`reference/parts/ddstars/` 1900, 3000, 5000, 8000 (`reference-indices.md` §22). **Test.**
`the_star_clip_is_assembled_out_and_the_keyframes_hold_its_pixels`.

## 28. The music player — five behaviours of the shipped player that every part's timing or sound stands on

No frame shows these; the evidence is the traces under `reference/music/` and the segment bytes
from `cargo xtask music probe --dump`.

### 28.1 A restart keeps what `_zplaysong`'s initialiser does not write (D45)

**Original.** `restartmus` (`MAIN/U2.ASM:348-397`) runs `_zstopsong`, `_zshutup`, `_zinitmodule`
and `_zplaysong`, whose initialiser (`MAIN/STMIK.300` `+0x5614`…`+0x56b1`, `+0x5781`) never stores
`avol`, `a0volcut`, `aequ`, `aadr`'s high word, `ac2spd`, `atreon`, `atrigcnt`, `atremor`,
`vibtretype`, `a0clearcnt` or `m_poslow`, so a restart keeps the previous song's channel volumes.
MUSIC0 hands channel 2 over at volume 63, so MUSIC1's order 0 takes its `C00` at row 3 (4 rows, not
64). **Port.**
`Player::restart_from` before `play_song`; `Channel::initialise` writes exactly the initialiser's
stores. The seam figures are D45's. **Tests.** `block_ones_music1_restart_takes_order_0s_break`,
`the_song_initialiser_writes_exactly_what_initialise_writes`,
`a_boot_starts_on_the_objects_zeroed_channels`,
`a_block_one_run_is_alku_then_u2a_then_pam_in_the_boots_numbering`.

### 28.2 The "volume 0" optimisation frees a silent channel before a pattern break can run

**Original.** MUSIC1's order 0 row 3 channel 2 is a `C00` on a channel silent since row 0; the
first-tick pass frees it before dispatch, so the break never runs. **Where.** `MAIN/STMIK.300`
`+0x3c77`…`+0x3cad` (on because both modules' flags words are 8). **Timing.** +243 polls, +180
ticks, +60 rows against a player that dispatches it. **Test.**
`the_volume_zero_optimisation_stops_a_pattern_break_from_running`.

### 28.3 The `A` effect has no upper bound: parameter 0x7F is speed 127

**Original.** MUSIC1's pattern 72 row 48 (order 98) is `A7F`; the handler (`+0x3dd2`) has no bound,
so the sixteen rows to the end of order 98 take 127 ticks each. **Timing.** +2606 polls, +1937
ticks. **Test.** `music1s_a7f_row_takes_127_ticks_and_takes_its_own_row_with_it`.

### 28.4 The output-curve builder's `rep stosw` clobbers its own count (A14)

**Original.** `+0x559d`'s `rep stosw` leaves `CX` = 0, so `+0x55a1`'s `rep stosb` writes nothing:
the zero region is `2·⌊(0x800 − K)/4⌋` bytes, half its intended length, and `+0x55bd` fills the
remainder with `0xFFFF`. For `mul` 64 that is 768 zeros, the ramp at 768..1279 and `0xFF` from 1280,
the boundaries the probe reads from the running segment (`music.md` §10.3). **Where.**
`MAIN/STMIK.300` `+0x5565`…`+0x55c1`. **Port.** `Curve::build`, indexed at `0x400 + value`. **Sound.**
Every sample. **Tests.** `the_curve_is_the_objects_own_table`,
`the_files_hand_arithmetic_is_the_builders_at_every_entry`,
`a_solo_voice_is_audible_through_the_table_the_machine_holds`.

### 28.5 The shell never writes `[0x8d7]`, so stereo plays 5.26 % flat

**Original.** Stereo divides by `[0x8d7]`, which the shell never writes (it doubles `_zmixspeed` to
40000 for SB Pro, `MAIN/U2.ASM:759`), so voices use the segment's initial **21111** against a frame
rate of 20000: 94 cents flat. **Where.** `MAIN/STMIK.300` `+0x539b`…`+0x53a9`. **Sound.** Every
stereo voice's pitch. **Test.** `a_step_is_the_objects_two_divisions`.

---

## The index: preserved behaviours no frame can show, and the reason each cannot

Each row is kept because the original does it, and is dead, unreachable or invisible on the shipped
path. Where a note holds a counterfactual test, the row names it.

| Part | Behaviour | Source | Why no frame can show it | Held in |
|---|---|---|---|---|
| ALKU | The final `dofade(palette2,palette)` runs only when `f>7`, and `fonapois()` after either branch | `ALKU/MAIN.C:147-150` | The fade touches entries 64-255, which the picture never indexes | `alku.md` §6 |
| PAM | `ruudulle`, a second copy of the unpacker, is never called | `PAM/ASMYT.ASM:207-320` | Dead code | `pam.md` §7 |
| PAM | Steps 41-45 of the 45-step loop draw nothing and only re-queue copy 63 | `PAM/OUTTAA.C:46-49` | The re-queue is the closing white hold (§11) | `pam.md` §7 |
| U2A | `fov` is uninitialised when first read | `VISU/C/U2A.C:261`, `:287`, `:348` | The first draw is background-only | `u2a.md` §6 |
| U2A | The second gate is dead: END's `resetscene(); xit=1` ends the loop before order 12 row 55 | `VISU/C/U2A.C:351-355` | Nothing after the loop's end runs (`the_second_gate_tears_down_on_the_first_tick_it_is_open`) | `u2a.md` §6 |
| U2A | The city hacks force sort distances for `co[2]`, `co[7]`, `co[13]`, `co[14]` | `VISU/C/U2A.C:301-310` | `U2A.00M`'s byte 15 is 0, so neither branch runs (`the_city_hacks_are_u2a_s_and_guard_out_of_range_instances`) | `u2a.md` §6 |
| U2A | The teardown clears all four pages and names page 3 the draw page | `VISU/C/U2A.C:432-441`, `AVID.ASM:341-358` | Every page holds the same cleared background | `u2a.md` §6 |
| U2E | `fov` is uninitialised when first read | `VISU/C/U2E.C:378` | The first draw is skipped (§12.3) | `u2e.md` §6 |
| U2E | The city hacks are dead: `city` is assigned and never read | `VISU/C/U2E.C:18`, `:212-213` | `U2E.00M` byte 15 is 0 and nothing reads `city` | `u2e.md` §6 |
| TECHNO | `setborder` returns before its assembly | `TECHNO/KOE.C:68-80` | The border is not a frame | `techno.md` §7 |
| TECHNO | `flash(-2)` returns `pal1` without capturing or uploading; `mode16` writes `flash_pal` and uploads nothing | `TECHNO/KOE.C:82-103` | No DAC write | `techno.md` §7; `flash_minus_two_uploads_nothing` |
| TECHNO | `mov cx,400` in `init_interference` is reloaded before it is read | `TECHNO/KOEB.ASM:356`, `:360` | The first value is never used | `techno.md` §7 |
| TECHNO | The KOEB plane-8 loop is behind `jmp @@OVER3`; `sizefade` stays 0 and its `add ax,256` is commented out | `TECHNO/KOEB.ASM:454`, `:524` | The display start is 8020 either way; the port writes it as `FIRST_PASS_START` | `techno.md` §3 |
| TECHNO | `power1` is computed and never read | `TECHNO/KOE.C:173-211` | A dead table | `techno.md` §3 |
| TECHNO | `dis_partstart()` is called a second time | `TECHNO/KOE.C:327` | The version check draws and waits on nothing | `techno.md` §5 |
| TECHNO | `pic = halloc(20000,4)` is 80,000 bytes and the segment poke folds the second read into it | `TECHNO/KOE.C:317` | The 400 decoded rows are the Origin reader's either way; 8300 and 8400 are at 0 | `techno.md` §7, §8 |
| PANICEND | The part never polls the exit flag (`grep -c dis_exit` prints 0) | `PANIC/SHUTDOWN.C:1-157` | No reference frame is at an exit | `panicend.md` §9-10 |
| MNTSCRL | `frame = veke` can never fire: `inc(frame)` is commented out | `FOREST/READ2.PAS:3`, `:221-224` | Dead bound (no `frame` counter in the port) | `mntscrl.md` §8 |
| LNS&ZOOM | `memcpy(back+64000, back+64000-1536, 1536)` copies the picture's last 1,536 bytes into the pad past it | `LENS/MAIN.C:302` | The copied bytes are all 0 (`tail -c 1536 assets/LENS/LENS.EXB`) and the buffer starts at 0; a mutation that drops the copy leaves the lens residuals unchanged. `the_tables_are_bounded_and_the_overrun_is_the_1536_byte_copy` pins the layout | `lnszoom.md` §2 |
| LNS&ZOOM | `if(dis_musplus>-30)` tests the function pointer, so the wait always runs | `LENS/MAIN.C:104` | At entry the player publishes -5 (`the_entry_pre_roll_lands_the_reference_position_at_tick_zero`); the counterfactual is `the_missing_call_in_the_part1_guard_is_preserved` | `lnszoom.md` §3 |
| LNS&ZOOM | The `a > 3` clamp on the frame advance | `LENS/MAIN.C:176` | The port advances one frame a tick | `lnszoom.md` §4 |
| LNS&ZOOM | `setpalarea` at `:200` is erased by `inittwk`'s DAC clear | `LENS/MAIN.C:200` | Uploaded and cleared inside one tick | `lnszoom.md` §5 |
| LNS&ZOOM | `rotpic90 = back` overwrites the picture buffer; `rotatez` is never called | `LENS/MAIN.C:191`, `LENS/ASM.ASM:419-564` | Nothing reads `back` after part 3 begins; the port has no `rotatez` | `lnszoom.md` §5-6 |
| PLZPART | `plzline` writes 84 bytes into an 80-byte row | `PLZPART/ASMYT.ASM:41-69` | The last four land in the stride's hidden gap | `plzpart.md` §1, §9 |
| PLZPART | `do_drop`'s `dtau` reaches 404 | `PLZPART/COPPER.ASM:175-181` | The split is off the 400-line screen; the port's 405 is inert | `plzpart.md` §9 |
| PLZPART | `timetable[4] = 2778` can never be reached | `PLZPART/PLZ.C:46` | The exit fires 65 ticks after the previous switch (`the_plasma_switches_and_exits_on_the_reference_ticks`) | `plzpart.md` §9 |
| PLZPART | The stale `PLZPART/MAIN.ASM` | `PLZPART/MAIN.ASM` | Not linked | `plzpart.md` §9 |
| RAYSCRL | `pal[768]` is `font[0]` and `tmppal[768]` is `pal[0]`: the `pf = 3` step at x = 255 touches index 768 | `WATER/DEMO.PAS:125`, `:174` | Entry 0 is black, so the font is never touched (`the_fade_loops_step_the_next_entrys_red_and_read_the_byte_past_the_array`) | `rayscrl.md` §5 |
| 3DSINFLD | `combg` is `char combg[16]` and every use is an overrun; `combguse` is read only under `PXLSUX`, which the build does not define | `COMAN/MAIN.C:8`, `:166-176`, `ASM.ASM:143-172` | No background; DAC 240..255 is unreachable (colour ≤ 127) | `3dsinfld.md` §2 |
| 3DSINFLD | The palette formula, the `*9/6` pass, the bar at 232..255 and the `combg` overwrite of 240..255 | `COMAN/MAIN.C:127-164` | Every displayed colour is ≤ 127 (`the_palette_is_the_formula_with_the_bar_and_the_unreachable_overrun`) | `3dsinfld.md` §2 |
| 3DSINFLD | The frame-4 `setpalarea` re-uploads the palette in force | `COMAN/MAIN.C:69-72` | Same bytes (the wait it spends is not dead) | `3dsinfld.md` §3 |
| 3DSINFLD | `frame < 4444` | `COMAN/MAIN.C:57` | The music break ends the part first (`the_part_ends_on_the_music_break_not_the_dead_frame_cap`) | `3dsinfld.md` §3 |
| 3DSINFLD | `cameralevel` is written (-270), loaded and stored, and never read | `COMAN/MAIN.C:54`, `THELOOP.INC` | Nothing reads `theloop_heigth` | `3dsinfld.md` §7 |
| JPLOGO | The interpolation reads `framey1[200]`/`framey2[200]` at `a = 796..799` | `JPLOGO/JP.C:164-171` | `doit` reads them only for `frame <= 511` (`the_interpolation_is_the_sources_own_and_its_overrun_is_unreachable`) | `jplogo.md` §2.1 |
| JPLOGO | `if(frame>511) c=400;` is never read again | `JPLOGO/JP.C:78` | The freeze is `y1`/`y2` keeping their values (`FREEZE_AFTER`) | `jplogo.md` §4 |
| JPLOGO | `scrolly` writes CRTC `0x0D` then `0x0C` | `JPLOGO/JP.C:30-45` | The order matters only to hardware | `jplogo.md` §1 |
| JPLOGO | `doit`'s `halt`, `storea`, `ysb`; `calc` (`:53-65`) and `lineblit` (`ASM.ASM:76-100`) never called; the `storea = a` and its `printf` | `JPLOGO/JP.C:69`, `:109`, `:220-225` | Dead | `jplogo.md` §8 |
| JPLOGO | The run opcode `0x80` writes 256 copies (`dec ah / jnz l4` wraps) | `JPLOGO/READP.C:44-51` | The shipped writers never emit it; `sr-assets` refuses it (`a_0x80_run_opcode_wraps_to_256_copies`) | `jplogo.md` §2.2 |
| GLENZ | The `frame>1400+789` bscale-shrink arm is unreachable: `frame>1220+789` catches every value first | `GLENZ/MAIN.C:483`, `:490` | Kept in source order | `glenz.md` §5 |
| GLENZ | `NEW.ASM`'s 256-word row table is followed in memory by the edge arena | `GLENZ/NEW.ASM:11-16` | Not shown reachable (worst case near y = 252; the port's list is a `Vec`) | `glenz.md` §8 |
| GRID | `sin256` and the 64 KB `muldata` are built and read by nothing linked; `_asmloop2`, the unused slot words and `_line` | `GRID/MAIN.C:38-43`, `:130-139`, `ASM.ASM:113-134`, `:247-269` | Dead | `grid.md` §2 |
| GRID | `d = sin1[k&1023]/4+256; d = 0;` is a dead store | `GRID/MAIN.C:162-163` | The overwritten value is never read | `grid.md` §6 |
| HARD | `depthcol[bp]` with a negative low word reads a code byte (0x07), modelled as zero | `HARD/BA.ASM:232-234` | The low word is always positive over part 2 (`the_part2_grid_z_range_and_drawn_words_are_the_measured_domain`) | `hard.md` |
| HARD | `do3dots`'s shadow store to `[si+12]` is behind `jmp @@4`; the word clear before it (`:229-231`) runs every iteration | `HARD/BA.ASM:229-235` | `[si+12]` stays 0; the port's one-byte-per-row clear differs only in bytes 1/321 at offset 0, which no reached frame reads | `hard.md` |
| HARD | `_GRID1.TMP`'s last byte is `0x4F` in the linked copy and `0x00` in the file | `HARD/B.C:112-121` | No filed keyframe depends on the 100th pair's y (`the_grid_table_is_the_shipped_grid1`) | `hard.md` |
| HARD | The `count>400` branch and the `0xff` cut; `setborder` | `HARD/B.C:243-246`, `:26`, `:196`, `:201` | Dead / inert (`the_0xff_cut_is_unreachable_with_one_start`) | `hard.md` |
| TWIST | `line()`/`leftline`/`rightline` are never called; the `memset` and `vram[a]=a` are overwritten before display; the record's last six bytes are never read | `TWIST/MAIN.C:30-42`, `ASM.ASM:97-218` | None is observable | `twist.md` §2 |
| CRED | `tw_setsplit(400)` | `CREDITS/MAIN.C:165` | A line compare of 400 below a 200-row display is inert | `cred.md` §1 |
| ENDSCRL | The line read runs past `text[64000]` if the text has no `%` line | `ENDSCRL/MAIN.C:66-74`; image `0x1351-0x13a3` | The shipped text's `%` line (537 of 542) ends the part (`0x169a`) | `endscrl.md` §5 |
| ENDLOGO | The surface is not cleared and DAC entry 255 is never written | `END/END.C:66` (`//inittwk();`), `:90-91` | Index 0 anyway (`the_opening_screen_is_the_shells_surface_through_the_shells_dac`); entry 255 is displayed by no path (`the_white_leaves_entry_255_unwritten`) | `endlogo.md` §4 |
| Music | The order list's `254` and `255` are kept as bytes; `254` is stepped over and `255` ends or loops the list | `MUSIC0.S3M`/`MUSIC1.S3M` | No played order reaches a `255`; the `254` is in §28's traces (`the_order_list_is_preserved_byte_for_byte`) | `music.md` §4, §7 |
| Music | The period clamp `[64, 32767]` | `MAIN/STMIK.300` `+0x5876`…`+0x5896` | The probe's 2,408 (MUSIC0) and 2,635 (MUSIC1) periods above `0x7FFF` are all on volume-0 channels | `music.md` §10 |
| Music | The flags word's bit 4 would write the clamped period back; both modules carry flags 8 | `MAIN/STMIK.300` `+0x583d`, `+0x586e`, `+0x589a` | The write-back never runs | `music.md` §10 |
| Shell | `error2` says "a number between 2 and 5" where `SECOND 1` is accepted | `MAIN/U2.ASM:557-559` | Text-mode wording; the port's refusal text is its own | `part-sequence.md` §4 |

## Not in this register, and why

- **ALKU's pass-2 `dis_sync` race** (`ALKU/MAIN.C:59`, `DIS/DISINT.ASM:449-463`): host timing
  (D17), in 1 session of 7 (`reference-indices.md` §25.3); the port follows the majority (D42, A55).
- **The stopped tick's `0xffff` in the position ring at a reload:** not carried, A55.
- **The flush's zero marker** (A14(a)): `the_flush_clears_the_accumulator_word_it_reads`; §28.4 is
  only the builder's count.
- **HARD's two extra displays at the mode-5 boundary:** a timing approximation, A48.
- **D24 adoptions** (MNTSCRL's `scp` 640, PLZPART's exit index 4, GLENZ's low-byte `rolcol` key,
  RAYSCRL's gate confirmation, LNS&ZOOM's forehead asset): `APPROXIMATIONS.md` entries, not bugs of
  the original's code.
- **Port deviations at three parts' exits** (ALKU's skipped closing calls, MNTSCRL's exit-aware
  wait, PLZPART's unpolled flag): A62.

## Adding an entry

An entry needs a measurement: a reference frame or a disassembled line, the source that does it, the
item that keeps it and the test that pins it. What the port cannot or knowingly does not reproduce
is an `APPROXIMATIONS.md` entry with its measured difference (`docs/design.md` §5.2).
