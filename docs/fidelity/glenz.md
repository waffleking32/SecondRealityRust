# GLENZ — the intro and the part driver

`GLENZ/MAIN.C` (648 lines) over the part's own `VEC.ASM`/`MATH.ASM`/`NEW.ASM`,
with the zoom-and-bounce opening from `ZOOMER.C`. This note covers the driver
Task 5 lands: the tick model, the buffer and view, the palette phases, the
objects and the D24 questions the part touches. The layers it consumes are
documented where they live (`math.rs`, `poly.rs`, `span.rs`, and the tests in
each); the block-2 keyframe comparison is Task 7's.

## 1. The tick map

The source's clock is one `dis_waitb` a tick (`DIS/DISC.ASM:42-47`, the DIS's
frame-count call) and one logic tick is one displayed emulator frame. The
anchors are Task 1's, measured on
`reference/music/second2b` and the `SECOND 2` captures
(`docs/fidelity/reference-indices.md` §7.6); the port's offsets from the reset
are pinned by

```text
cargo test -p secondreality --bin secondreality \
    glenz::tests::the_intro_is_47_plus_177_ticks_and_the_measured_waits
```

| reference | call | what runs |
|---|---|---|
| 1110 | WaitMusic | `dis_musplus()` is −20 (`MAIN.C:290`) |
| 1111 | WaitMusic exits, **and `zoomer2`'s first body is the same call** | `setmframe(0)`, `zoomer2()` (`MAIN.C:291-293`); body `ZOOMER.C:38-69` |
| 1111..1157 | 47 zoomer bodies | the 47th caps `zy` at 260 |
| 1158 | the 48th call: break, epilogue, mode set, setup, **bounce body 1** | `ZOOMER.C:70-80`; chain-4 + `memset` + `int 10h` + `testasm` (`MAIN.C:296-304`); `fcrow`/DAC (`:306-318`); `MAIN.C:322-350` |
| 1158..1334 | 177 bounce bodies | the last draws rows 153..199 |
| 1335..1514 | Wait300 polls false | `while(!dis_exit() && dis_getmframe()<300);` (`:353`) |
| 1515 | zframe 300: the poll exits; the `dis_waitb` starts | |
| 1516 | that wait's post-actions: `backpal`, `tmppal`, the all-256 upload, `lightshift=9`, positions zeroed | `MAIN.C:354-396` |
| 1517 | the second `dis_waitb`'s post-action: `bgpic = vram`; the 333 poll starts | `MAIN.C:397-400` |
| 1518..1560 | Wait333 polls; zframe 333 at 1560 | |
| 1561 | `pal = backpal`, `dis_partstart()`, the DAC read, `dis_waitb` | `MAIN.C:402-407` |
| 1562 | the main loop's **first** `dis_waitb` (its top checks run here) | `MAIN.C:409-413` |
| 1563 | main-loop frame 1 | `MAIN.C:414-641` |
| 1606 | main-loop frame 44: the first object pixels | §5 |

The measured lines:

```text
$ awk -F, '$1 >= 1109 && $1 <= 1112' reference/music/second2b/trace.csv
1109,2,44,50,0,3,130,0,1,525
1110,2,44,50,0,3,130,0,1,526
1111,2,45,50,0,3,130,0,1,0
1112,2,45,50,0,3,130,0,1,1
$ awk -F, '$1 == 1515 || $1 == 1560 || $1 == 1562' reference/music/second2b/trace.csv
1515,5,16,28,0,3,130,0,0,300
1560,5,28,28,0,3,130,0,0,333
1562,5,28,28,0,3,130,0,0,335
```

**The one-tick distinction the model turns on.** A `dis_waitb` consumes the
tick it is called in; the source's post-wait work is drawn in the *next*
frame. The zoomer and the bounce draw *before* their wait, so their draw and
wait share a tick; the main loop's `repeat=dis_waitb()` comes *before* the
frame's work, so its first body is one tick after the first wait. That is why
the reset is 1111, the main loop's first wait is 1562, and main frame 1 is
1563: frame 44 lands on **1606**, not 1605. The test above pins the offset
(`Main` at reset + 451, frame 1 at reset + 452); the silhouettes in §5 pin the
result.

The unit rig scripts the music at MUSIC1's tempo-130 rate — 52 music ticks a
second against 70.086 video frames, i.e. 52,000/70,086 ≈ 0.7419 zframes a
frame (the brief's 0.7422 is that rate rounded) — from the reset tick. With
that rate the `getmframe` waits pass at +404 and +449 — the measured 1515 and
1560 — and the trace's own zframe at 1562 (335) is what the same formula gives
(335). The rate is a scripted clock, not a claim about the player's jitter:
the trace's zframe repeats 300 at 1515 and 1516, and the wait's crossing frame
is what the part reads.

## 2. The buffer, the blit and the view

The port keeps the original's two buffers as linear 320-pixel rows
(`docs/decisions.md` D36):

- `vram` — 320×400 = 128,000 bytes (`MAIN.C:18`). It starts as BEGLOGO's
  inherited picture: `init` copies `ctx.surface`'s first 320 columns of rows
  0..400. The zoomer paints it, the mode set clears it, the bounce fills its
  first 64,000 bytes, and the compositor writes the first 64,000 every main
  frame.
- `bgpic` — the 64,000-byte background (`MAIN.C:12`), snapshotted from `vram`
  at 1517 and never written again except by the frame-765..789 band clear.

Every tick, `blit` copies 400 rows (during the zoomer) or 200 rows (after the
mode set) into the surface's first 320 columns. The view's `y_line_offset` is
unset, so the beam reads the surface's own stride (640 in every run), and the
copy is at that stride.

**A run that starts at GLENZ has no BEGLOGO frame to inherit.** The demo
always reaches the part after BEGLOGO (`MAIN/U2.ASM:812-815`), whose last
screen is the 320×400 Mode X picture the zoomer closes over. `--part glenz`
(or a plan beginning there) enters whatever view and surface the shell
started with. `init` therefore installs the 320×400 view itself — 320 wide,
400 lines, origin 0, no line offset, no split — and copies the surface's
first 320 columns of rows 0..400 into `vram`; in a fresh run those rows are
the shell's black, so the opening plays over black instead of over BEGLOGO's
picture. The mode set discards the inherited rows a few hundred ticks later.
The unit tests enter through this standalone shape (`View::new(320, 200)`
before `init`); the substitution changes the opening's background, not its
geometry or tick map.

The original's Mode X plane masks and the shipped build's 92-byte CRTC row
(`GLENZ/VIDINIT.ASM:306`) are memory-layout facts, not image facts: the
reference's `frame_001499.idx` reproduces its RGB pixel for pixel under the
chunky reading (`reference-indices.md` §7.1, §7.3). The port never models the
planes.

**Mode 13h's double scan.** The brief calls the mode set `double_scan=true`;
that is the guest VGA's `is_double_scanned` flag (D17's geometry note,
`docs/decisions.md:449`). In this engine `View::double_scan` means "two
displayed scanlines sample one surface row", so `true` with a 320×200 frame
would display only surface rows 0..99. The reference's mode-13h frame is the
emulator's *halved* 320×200 picture — 200 distinct source rows — and the
bounce writes those rows directly. The port therefore sets
`height=200, double_scan=false`, and the held-frame test below is what says
so: with the flag set the compose would read the wrong half of the buffer.

The `int 10h` set is one tick: the mode's registers, `border=Some(0)`,
`origin 0`, no line offset, no split. The source's `memset(vram,0,65535)`
runs in chain-4, so its 65,535 addressed bytes cover all of the plane's 400
rows; the port clears the whole 128,000-byte buffer (the zoomer's epilogue has
already zeroed it).

**The compositor's paint is a substitution.** The source's `ng_pass3` walks the
previous frame's boundary list and the new one together and repaints only the
spans whose colour changed (`NEW.ASM:66-172`). Every span's pixels are
`colour | bgpic[pixel]`, so a skipped span already holds exactly the bytes a
repaint would write; the port drops the walk and repaints every span from
`bgpic`, never clearing the buffer. The substitution is **A39** with a measured
difference of **0**:
`span::tests::the_full_repaint_equals_the_source_two_list_walk` keeps the
source's two-list walk in the test module and compares whole frames over a
13-frame corpus, including pixel 63,999, which neither implementation writes.

## 3. The copper, substituted

The source registers `copper` (`MAIN.C:264-270`, `:408`), which uploads
`pal`'s 16 entries at every retrace, and the main loop also uploads them right
after each wait (`:414-415`). The port registers **no copper callback**: the
engine composes one finished frame per tick after the part's call, so the
part's per-tick `set_palette_entry(0..15)` and `poly::build`'s 16-entry group
writes (entries 16..255) are the same DAC state at compose time. The upload is
`set_palette_entry`, never `set_palette`: the group entries must survive the
0..15 write, and a test pins that
(`glenz::tests::the_low_palette_upload_never_touches_the_groups`).

The displayed palette for main frame *N* is `pal` as it stood at the end of
frame *N-1*: the source uploads before it computes the new one, which is why
the frame-790 `pal = backpal` copy shows from 791. The port preserves that
order (`upload_low_palette` then `MainState::step`).

The intro's DAC writes are not copper: the zoomer uploads its 128 faded
entries a tick (`ZOOMER.C:68`), the bounce writes all 256 black then entries
0..15 from the picture (`MAIN.C:315-318`), and Wait300 uploads `tmppal` for
all 256 (`:391`).

## 4. The palette phases

`pal` is 16 entries. `MainState::step` is `MAIN.C:417-591`'s inner body, and
its phases are pinned by

```text
cargo test -p secondreality --bin secondreality \
    glenz::tests::the_palette_phases_have_the_source_boundaries
cargo test -p secondreality --bin secondreality \
    glenz::tests::the_band_clear_is_at_rows_150_onwards_and_stops_at_789
```

| frame | source | `pal` after the step |
|---|---|---|
| ≤700 | no branch | `backpal` |
| 701..764 | `pal = backpal*(764-frame)/64` | 701: ×63; 764: 0 |
| 765..789 | the band clear; no `pal` change | 0 |
| 786..789 | `backpal` rebuilt from the bit pattern | 0 |
| 790..794 | `pal = backpal` | the rebuilt `backpal` |
| 795..2069 | no branch | unchanged |
| 2070..2133 | `pal = backpal*(2133-frame)/64` | 2070: ×63; 2133: 0 |
| ≥2134 | b clamped to 0 | 0 |

The rebuilt `backpal` (`MAIN.C:557-584`): `a=0..15`, R = `10·(a&1) +
30·(a&2) + 20·(a&4) + 16·(a&8)`, G = B = `16·(a&8)`, every channel clamped at
63. Entries 0 and 4 (the hidden faces' reduced colours) therefore stay black
or dark red in the second half.

The **band clear** (`MAIN.C:552-556`) writes two rows of both buffers at
`y = 150 + (frame-765)*2`: rows 150/151 at 765 through 198/199 at 789, 50
rows in all. It runs in the physics, **before** `cglenzdone` paints
(`MAIN.C:640`) — the order the plan records, and the reason the object is
drawn against a cleared floor from 790 on. It is pinned by
`the_band_clear_is_at_rows_150_onwards_and_stops_at_789` (which also checks
that 790 clears nothing).

`tmppal` (`MAIN.C:365-390`, test
`glenz::tests::the_wait300_tick_uploads_tmppal`): for `a < 16`, entry `a` is
`backpal[a]`; for `a ≥ 16`, the base is `backpal[a&7]` and `a&8` lifts every
channel by 16, clamped at 63. Entries 0..15 are exactly `backpal`, which is
what the part-start DAC read returns.

## 5. The objects and the physics

Both objects are 14 vertices (`points`, `pointsb`; `MAIN.C:60-149`) and 24
triangles each (`epolys`, `epolysb`; `poly.rs`). A main frame does
(`MAIN.C:593-640`):

1. `compositor.clear()` (`cglenzinit`).
2. If `xscale > 4`: `matrix_yxz(rx, ry, rz)`, rotate `points`, the diagonal
   scale matrix `(xscale*64, yscale*64, zscale*64)` with the translation
   `(oxp, ypos+1500+oyp, zpos+ozp)`, rotate again, `clip_y(1500)` while
   `frame < 800`, project with `INIT_320X200`, `build(EPOLYS, Glz)`, add each.
3. If `frame > 800 && bscale > 4`: `matrix_yxz(3600-rx/3, 3600-ry/3,
   3600-rz/3)`, the same pipeline on `pointsb` with the uniform `bscale*64`
   and `(oxb, ypos+1500+oyb, zpos+ozb)`, no clip, `build(EPOLYSB, Glz2)`.
4. `compositor.paint(bgpic, vram[..64000])` (`cglenzdone`).

`rx += 32`, `ry += 7`, `rz += 0` each frame, all `% 10800`; `zpos` is 7500.
The first object is on while `xscale > 4` — through frame 2119 — and the
second from frame 803 (bscale 2, 4, then 6) through 2184, off from 2185 when
the post-2009 shrink takes bscale to 4. `lightshift` turns 10 at frame 858,
when `bscale` first overtakes `xscale`. Tests:
`glenz::tests::the_object_switches_follow_the_scales` and, for the second
object's draw path itself, `glenz::tests::the_second_object_draws_when_bscale_exceeds_4`.

**The 16-bit arithmetic is the physics.** The source is Microsoft C, large
model (`GLENZ/MAKEFILE`: `cl /qc /AL`), so every `int` in `main` is 16 bits.
`V.EXE`'s disassembly (the map's `MAIN_TEXT` at `V.MAP:3`) confirms each
product is a 16-bit `imul` with only the low word kept: the code sign-extends
`AX` with `cwd` before the following `idiv`, discarding `DX`. Two places
wrap in the shipped animation:

- `yposa = -yposa*boingm/boingd` (`MAIN.C:511`): the product passes 32,767
  during the intro and wraps (the test pins a wrapped product, −4,016×10 →
  25,376), which moves the jello trajectory — `xscale` is 115 at frame 800,
  not what unbounded arithmetic gives.
- `oyp -= b*b/2` (`MAIN.C:477`) from frame 1801: `oyp` is 16-bit and wraps
  every few frames, so the first object's fall after 1800 is a wrapped
  offset, not a large one.

The port stores the source's `int` state in `i16` and routes the products
through `mul16`; `glenz::tests::the_sixteen_bit_products_wrap_like_the_source`
pins the frame-667 product and the subtraction, and the object-switch test
pins the frame-800 scale the wrap produces.

`sin1024` (the orbit offsets, `MAIN.C:456-471`) is `ZOOM.ASM:28-29`'s
`_sin1024`: 1,024 words of a 256-amplitude sine, indexed `(a*n)&1023` on the
16-bit product's low word. It is a different table from the driver's
`SIN`/`COS` (§`tables.rs`), and `tables::SIN1024` is transcribed with its own
source parse test. The `frame>1800` products use the source's explicit
`(long)` casts; the `frame>1400+789` bscale-shrink arm is unreachable
(`frame>1220+789` catches every value first) and is kept in source order.

The generated-code question the phase's Global Constraints raise is settled
by what the shipped path calls: `ceasypolylist` + `cglenzinit/polylist/done`
+ `NEW.ASM`, never `polygroup`/`polyf`. `ZOOMLOOP.INC` **is** linked —
`ZOOM.ASM:26` includes it, and `zoom.obj` is on the link line
(`GLENZ/MAKEFILE:3`) — but it is the body of `_zoom`, which only the
unshipped `zoomer1` calls; `zoomer2`, the shipped intro, uses none of it and
`ZOOM.ASM` exports only `_zoom` and `_sin1024`. VEC.ASM's
matrix-in-immediates and the patched group loops are state-only here, so the
port keeps the source's structure and lets the compiler hold the matrix.

## 6. The held opening, measured

The final bounce frame is the reference's held 1499 picture. The unit test
builds the frame exactly as the part does — 47 zoomer bodies, the epilogue,
the mode-13h clear, 177 bounce bodies — and compares all 64,000 indices:

```text
cargo test -p secondreality --bin secondreality \
    glenz::intro::tests::the_held_intro_is_the_reference_index_plane
```

It reports 0 differing indices against `reference/parts/glenz/frame_001499.idx`;
the file's content is rows 154..199 (row 153, `fcrow[0]`, is black), matching
§7.3's census. The same test pins the 177 draws, the final band rows
153..200, and the scaled `fcrow` rows in rows 153..199
(`the_bounce_draws_177_ticks_and_ends_on_rows_153_to_200`,
`the_final_bounce_band_is_the_source_scaled_picture_rows`).

The first object is the Task 1 corpus: frame 44, then the six silhouettes
1606..1611. The port's own frame-44..49 boxes, counts and centroids match
`python3 -I tools/glenz_physics.py entry` exactly (8/21/60/86/111/137 pixels,
centroids (175.5,0.0) … (165.5,2.0)); the test is
`glenz::tests::the_first_object_appears_on_frame_44_with_the_measured_silhouettes`.
The first main frame is `bgpic` (the object is off-screen), with pixel 63999
the compositor's sentinel inherited from `bgpic`
(`the_first_main_frame_is_the_background`).

## 7. D24 questions this part touches

The reference decides each; nothing here was fitted.

- **The shipped 92-byte CRTC row and its projection constants (D36).** The
  port reads the reference through the chunky model and projects with
  `init320x200`'s 256/213/160/130. Task 1's 2300 box (x 97..217, y 38..149,
  centroid 156.8,94.1) was computed with those constants, and the port's
  frame 44..49 boxes, pixel counts and centroids match that model's output.
  Task 7's surface comparison is the stronger check: boot **2305** (main
  frame 738, the filed 2300's surface) reproduces that box and centroid
  exactly — 9,989 px, (156.8, 94.1) — and every object pixel is byte-identical
  (§11.2). No shipped constant is adopted.
- **The visible-path `rolcol` key (D7/D24).** The tree keys the visible path
  by the **full colour word** (`movzx bx,es:[bx-2]`, `VEC.ASM:450`) and the
  hidden path by the low byte, so with a 256-byte `rolcol` the two maps are
  independent. The packed EXE cannot be disassembled to see which the shipped
  build does, so the **reference's picture is the arbiter**: it is keying the
  visible path by the **low byte** (one shared in-bounds map) that reproduces
  the reference's faces and its untouched low palette. Adopted — `poly.rs`
  keeps a single `slots: [u8; 256]` keyed by the low byte, shared by the
  visible and hidden paths. The measurement and the two models' outputs are
  §11.2.
- **`L_r` against `L_p`.** The block-2 music wait exits at **1117** in the
  port against `second2b`'s **1111**, but the port's intro runs one tick
  shorter from the reset to frame 44 (**494** against the reference's
  **495**), so the port's loop start is **`L_p = 1567`** against `L_r = 1562`
  and the plan's aligned comparison index is **`K + 5`** (§11.1). Two
  independent measurements give 1567: the first object pixels at boot 1611
  (8 px, frame 44) and the first faded surface at boot 2269 (frame 702,
  b = 63). The one-tick difference is the port's real player reaching
  zframe 300/333 one tick earlier relative to the reset than `second2b`;
  the phase machine's own relative ticks match the source and the
  scripted-clock unit test. The reference's captured moving frames carry a
  surface/DAC phase on top of that (0/1-frame surface lag at
  2300/3000/3400, DAC one frame ahead of the surface), registered as A36.
  The held opening is unaffected: the bounce's last body draws at 1334 and
  the first object pixels are at 1606, so the picture holds for **272
  ticks** — 1334..1605, the 228 ticks from the last bounce body to the main
  loop's first wait at 1562 plus the 44 main-loop ticks before frame 44
  draws.

## 8. The plan's carried items

- **`cross`'s short-list guard.** `poly::cross` returns 0 (the visible
  answer) when fewer than three pairs are stored; the asm reads three pairs
  unconditionally, so a short list would read stale `polylist` bytes. The
  shipped tables always store three; the guard is reachable only if two
  distinct projected vertices coincide and the closing-duplicate drop fires.
  The frames this task can check — the held 1499 (exact) and the frame
  44..49 silhouettes (matching bounding boxes, pixel counts and centroids) —
  show no such frame; Task 7's 2300/3000/3400 diffs are the arbiter. If one
  triggers it, it is a new `APPROXIMATIONS.md` entry with that frame; none is
  filed now.
- **`NEW.ASM`'s `nep`/`ne` bound.** The original's 256-word row table is
  followed in memory by the edge arena, so a projected upper y above 255
  would read or write inside it. The Task 4 review's conservative bound puts
  the worst shipped case near y≈252, a few rows of margin, so it is not
  proven unreachable; the port's edge list is a `Vec` and has no such
  adjacency. The reference frames measured in §7.6 put the objects inside
  rows 0..199, and the frames this task checks carry no upper y above 6
  (frame 44..49). Task 7's moving keyframes are the arbiter; registered only
  if one is reachable.
- **The clear-before-paint order.** Frames 765..789 clear `bgpic` and `vram`
  in the physics, before `cglenzdone` paints. The port preserves it —
  `MainState::step` runs the clear, `draw_objects` then paints — and
  `the_band_clear_is_at_rows_150_onwards_and_stops_at_789` pins it.

## 9. What is ported and what is not

- **Exit behaviour.** `MAIN.C:290`'s `while(!dis_exit() &&
  dis_musplus()<-19);` exits on either condition and **continues**:
  `dis_setmframe(0)` and `zoomer2()` (`:291-293`) still run, and the zoomer
  checks only `kbhit` (`ZOOMER.C:38`), so ESC during the gate or the zoomer
  still plays all 47 bodies. The bounce is `while(!dis_exit())`
  (`MAIN.C:320`; V.EXE's main calls `dis_exit` at map 0x8f7 and skips the
  body on a true answer), and the two `dis_getmframe` loops test `dis_exit`
  as well (`:353`, `:400`), so ESC before the bounce's first body skips every
  bounce body and both waits. The unconditional `dis_waitb`s and the
  `backpal`/`tmppal` build, `bgpic` snapshot and part-start still run; the
  main loop's `while(frame<7000 && !dis_exit())` is not entered, and the
  part retires on its first main-loop check. The port follows exactly that:
  `WaitMusic` continues into the zoomer on ESC, the zoomer ignores it, and
  the bounce and both waits see it.
- The main loop's exit order (`MAIN.C:409-411`). The loop condition and the
  music cut sit at the top of the iteration, but the body reaches that top
  **after** `cglenzdone` in the same frame: the frame is drawn and the loop
  then breaks, so a cut, ESC or the frame-7000 cap first visible on a body
  tick keeps that frame and retires the part on the same tick. A cut or ESC
  already visible at the loop entry (the port's `MainWait`) breaks before the
  wait and the body, so that tick draws nothing. Tests:
  `the_music_cut_at_the_main_wait_draws_nothing` /
  `the_music_cut_on_a_body_tick_draws_then_finishes`,
  `escape_at_the_main_wait_finishes_without_a_drain` /
  `escape_on_a_body_tick_draws_then_finishes`, and
  `the_frame_cap_draws_7000_then_finishes` /
  `the_frame_cap_at_the_main_wait_draws_nothing`. The part has no teardown,
  so `drains_after_exit()` is false.
- The source's `dis_setcopper(0,copper)` and its teardown
  `dis_setcopper(0,NULL)` are not ported as callbacks (see §3); there is
  nothing to remove on exit.
- The flash of 400 geometry at the mode-set tick: the engine sizes a tick's
  frame from the view as it stood at the tick's start, so the tick that
  installs mode 13h composes at 320×400 (rows 200..399 are the zoomer's last
  copy). It is one tick, at reference 1158, outside every keyframe; the next
  tick is 320×200. Recorded here rather than hidden.
- The standalone-entry substitution of §2: a run that starts at GLENZ opens
  over the shell's surface rather than BEGLOGO's last frame; the view and the
  tick map are the same.

## 10. The two shell seams

The part is reached through the shell, and two seams are the shell's, not
GLENZ's: block 1's mid-run `MUSIC1@0` restart and the post-PANICEND cue wait.
D38 has the decision; this section records what the port does and measures.

**The block-1 restart.** `MAIN/U2.ASM:813-818` sits between BEGLOGO (`exe3`)
and GLENZ (`exe4`): `test whattorun,1 / jz @@skip2b` inside the `whattorun & 2`
region — both bits, so the site fires for `SECOND 1`'s run alone — and
`restartmus(ax=1, bx=0, cx=48700)`. The port's row is
`MUSIC_RESTART_AT = [("u2e", Music0, 18), ("glenz", Music1, 0)]`; a plan that
reaches GLENZ under any other module installs a new player seeked to MUSIC1
order 0 at the boundary, with `MusicStart::restart`'s four-tick wait — one that
continues from the old player's channel state (D45). Block 2
skips the site (`whattorun & 1` clear) and its initial start already sought
MUSIC1 order 0 (`:772-775`), so `Sequence::restart_music`'s equality check
must — and does — keep the player and its clock.

- **Block 2's boundary is 576.** BEGLOGO is 172 ticks from the boot's 404:
  32 vertical blanks, the mode-set tick, the nine frames A5 records as
  unmodelled, 129 fade steps and the closing write (32+1+9+129+1 = 172), and
  GLENZ's first tick draws 576 under the entry player:
  `music.start_frame` is still **408**, not a restart at 576
  (`a_block_two_run_reaches_glenz_without_reloading_music_one`,
  `the_block_two_run_does_not_reload_at_glenz`).
- **Block 1's boundary is 8057.** PAM retires on 7,884 in the port's own
  chain (its part started at 7,508, not the reference's 7,710), BEGLOGO draws
  7,885..8,056, and the restart installs MUSIC1@0 at 8,057 with `waiting == 4`
  (`a_block_one_run_is_alku_then_u2a_then_pam_in_the_boots_numbering`,
  `the_block_one_run_restarts_music1_at_glenz`). D38's measured restart
  interval is (8049, 8100); 8,057 is inside it.
- **The block-1 wait is measured, not fitted.** The seek starts at 0:0; the
  wait (`MAIN.C:290`, `musplus < -19`) ends on the first published order 2
  row 45 (`musplus` −19) — the same position the reference's own exit answers
  (8,521 is `2:45` in `second1d`) — at **8,527**, 470 ticks after the boundary
  and **6 frames after the reference's 8,521**. Phase 9 Task 6b made the
  restart keep MUSIC0's channel volumes, as `_zplaysong` does (D45): MUSIC1's
  order 0 then takes its row-3 `C00` and lasts 4 rows, not 64, and the restart
  publishes `1:0` at 8,087. Before it the wait ended at 8,770, 713 ticks after
  the boundary and 249 frames late. Of the 6 frames left, 5 are the
  restarted clock's (D34's four-tick wait from the boundary; `music-clock.md`)
  and 1 is the trace's: 8,521 is its `2:45` sample and the reference picture
  first differs at 8,522 (D17), so against the picture the port is +5;
  **A38** carries the measured values and the commands, not a fitted start.
- **The block-2 wait against `second2b`.** The reference exits at **1,111**
  (`2:45`); the port's position is `2:44` through 1,116 and `2:45` at
  **1,117**, six ticks later — a session-phase difference of A12's class,
  larger than A12's measured 5-frame worst for `second2b` itself (the table's
  overall worst is 7, on `second1`), so it is registered rather than absorbed.
  The six-tick wait offset is not the picture offset: the port's intro from
  the reset to frame 44 is one tick shorter than the reference's, so
  `L_p − L_r = 5` and Task 7's plan alignment is `K + 5` (§11.1, A36).

**The post-PANICEND cue wait.** `MAIN/U2.ASM:826-853`, the wait at `:833-853`:
`@@zh5`/`@@zh4`, entered after `exe7` (PANICEND) returns. One `dis_waitb` and
one `dis_exit` a poll, then `dis_musplus` — wait while it is `>= 0`, then,
after the first negative, while it is `<= 0`, and continue on the first `> 0`.
The port installs a private `CueWait` part in place of the next plan part
whenever `PartStatus::Finished` retires `panicend` (not when the exit flag
retires it); it draws nothing, holds PANICEND's surface and picture, consumes
one tick a call and is exit-aware. A plan that ends at `panicend` enters the
wait and then the hold.

- **The calibration** (`reference/music/second2`, D38): PANICEND retires at
  boot **8903** with `musplus` 19 (`zplus` 2, row 19) — non-negative, so the
  first phase keeps waiting rather than ending at the part's retirement;
  **8953** is the first negative (row 32 → −32); **9219** is the first
  positive (row 1 → 1), the exit. The phase rules are pinned tick by tick in
  `the_post_panicend_wait_ends_on_a_positive_musplus`, the install and the
  held screen in `the_post_panicend_wait_is_between_the_part_and_the_next_one`
  and `the_cue_wait_holds_the_screen`.

## 11. The block-2 comparison (Task 7)

The dump and the two comparisons:

```text
cargo run --release --bin secondreality -- --block 2 --headless \
    --dump-frames 1499,2304,2305,2306,3003,3004,3005,3006,3403,3404,3405,3406 \
    --dump-dir out/glenz-final
cargo run --release --bin secondreality -- --block 2 --headless \
    --dump-frames 1499,2300,3000,3400 --dump-dir out/glenz-indexed
cargo xtask compare --reference reference/parts/glenz \
    --rendered out/glenz-indexed --out out/glenz-indexed-diff
cargo test -p secondreality --test glenz
```

The block-2 dump exits **0**, writes every index, and its manifest says
`part: glenz`, `index_origin: boot`, `anchor: 404`, 320x200, `music:
MUSIC1@0, start_frame 408` (the clock's measured 408; block 2's boundary is
576). The index-keyed `xtask compare` over a dump at the filed indices
(`out/glenz-indexed`, frames 1499/2300/3000/3400) reports **1499 at 0.0000 %
(0/64,000)** and exit 1 overall, because the moving frames at their own
indices are five ticks behind the reference's: 22,632 / 15,333 / 19,708
(35.3625 / 23.9578 / 30.7938 %) at 2300/3000/3400.

### 11.1 The measured phase

Both sides of the boundary are measured, and neither is moved. The block-2
music wait exits at **1117** in the port (`second2b`'s is **1111**). The
intro's own tick map is the source's, and the port's reset-to-frame-44 span
is measured two ways: the first above-floor object pixels are at boot
**1611** (8 px, box 172..179 × 0..0 — the reference frame 44's silhouette;
none at 1610, 21 px at 1612), and the first surface carrying the fade is
boot **2269** (`pal` after main frame 701, b = 63) after 2268 (b = 64; the
fade starts at frame 701). Both give **`L_p = 1611 − 44 = 2269 − 702 =
1567`** against Task 1's `L_r = 1606 − 44 = 1562`. The reset-to-frame-44
span is therefore **494** in the port against the reference's **495**.

*The one tick is the player, not the phase machine.* The 494-vs-495 span
means the port's real player reaches the two `getmframe` crossings (300 and
333) one tick earlier relative to the reset than `second2b` does; that is
the same session-phase class as its six-tick wait exit. The phase machine's
own relative ticks — 47 zoomer bodies, 177 bounce bodies, the two
`dis_waitb` waits, the part-start wait and the main loop's first wait —
match the source and are pinned by
`the_intro_is_47_plus_177_ticks_and_the_measured_waits`; no transition was
changed to move the loop start.

**The plan's alignment is `K + 5`.** Main-loop frame `f` is at boot
`1567 + f` in the port and `1562 + f` in the reference, so the filed `K` is
answered by the port's `K + 5`.

**The capture adds a surface/DAC phase.** With `L_p = 1567`:

| filed index | reference counter frame | filed surface frame | filed DAC |
|---|---|---|---|
| 2300 | 738 | 738 (lag 0) | `pal` after 738 (b = 26), i.e. mid-frame-739 |
| 3000 | 1438 | 1437 (lag 1) | the frame-1438 group writes, i.e. mid-frame-1438 |
| 3400 | 1838 | 1837 (lag 1) | the frame-1838 group writes |

The port pairs surface *N* with `pal` after *N−1* (its upload-before-step
order, `MAIN.C:414-590`); the capture samples after the source's upload or
its group writes but before the surface is painted, so its DAC leads its
surface. The surface lag grows from 0 to 1 between 2300 and 3000 — the
capture's `repeat`/sample scheduling (D17), not a value the port can draw.
The surface-aligned frames (2305, 3004, 3404) are the **unique minima** of
the index-structure metric: their neighbours' unexplained counts are
1,859/1,740 (2304/2306), 4,486/2,837 (3003/3005) and 5,876/4,780
(3403/3405), against 0/711/839 at the minima — so they are measurements,
not a chosen offset.

### 11.2 The colour key: the reference identifies the low-byte reading (adopted)

`VEC.ASM:450` keys the tree's visible path by the **full colour word**
(`movzx bx,es:[bx-2]`) and `:417` keys the hidden path by the **low byte**.
With a 256-byte `rolcol` those are separate maps: a hidden face never frees a
visible slot, so the fifteen-slot pool fills with every word that has ever
been visible. The tree is a development snapshot (D24) and its packed shipped
EXE cannot be disassembled to see which key the release uses, so the
reference's picture decides:

| model | boot 2306 (main frame 739) | low palette |
|---|---|---|
| full-word key (the tree) | object box (99,216, 37,149), **8,066 px**; sky `(97,97,97)` | DAC entry 0 is a slot-32 group (`16 − slot/2` residual): the background reads the group ramp, and every post-exhaustion face is painted with colour 0 and vanishes |
| low-byte key (adopted) | object box (99,217, 37,149), **9,927 px**, centroid (157.6, 94.2) — asserted at 2306 | entry 0 black; every face drawn |

The full-word row's picture numbers were measured on the **pre-adoption tree**
(commit `63b4267`) with §11's dump command; the mechanism they show — the
sixteenth word's group written at DAC entry 0, its colour 0 — is pinned by
`glenz::poly::tests::glz_pool_exhaustion_uses_slot_32_and_entry_0` on the
committed tree.

Adopted, as one expression the tree's algorithm carries: a single
`slots: [u8; 256]` keyed by the colour word's low byte, shared by the
visible and hidden paths, so a hidden face frees the slot its visible face
took. The derivation's permanent test is
`glenz::poly::tests::glz_hidden_face_frees_the_visible_slot`; the picture
evidence is the 2305 comparison: boot 2305 is the filed 2300's surface
(main frame 738), its object box is the filed box exactly — 9,989 px,
(156.8, 94.1) — and of the 5,208 differing pixels **all 5,208 are palette
entries 0..15** (**0** with an index ≥ 16). The reference's colours there are
exactly the fade at **b = 26** (the value computed at main frame 738), the
port's exactly **b = 27** (its surface 738 pairs with `pal` after 737). That
is the capture's one-frame DAC lead, not a colour-model difference.

### 11.3 The comparison, at the measured alignments

| port boot | reference | differing px | unexplained | reading |
|---|---|---|---|---|
| 1499 | 1499 | **0** (indices and RGB) | 0 | the held opening (Task 5's intro) |
| 2305 | 2300 | **5,208** (8.1375 %) | 0 | plan `K + 5`; the 2300 surface (lag 0), DAC phase only |
| 3005 | 3000 | **3,114** (4.8656 %) | 2,837 | plan `K + 5`; one frame after the filed surface |
| 3405 | 3400 | **5,261** (8.2203 %) | 4,780 | the same |
| 3004 | 3000 | **2,289** (3.5766 %) | 711 | the surface minimum (K+4, the capture's lag 1) |
| 3404 | 3400 | **5,941** (9.2828 %) | 839 | the surface minimum |
| 2306 | 2300 | **9,777** (15.2766 %) | 1,740 | `K + 6`, one past the plan's alignment (the DAC phase) |
| 3006 | 3000 | **10,806** (16.8844 %) | 5,762 | the same |
| 3406 | 3400 | **12,001** (18.7516 %) | 7,974 | the same |

At the surface minima the port's *index* structure matches the reference's
face coverage almost exactly: the unexplained pixels are **711 / 839** of the
35,896 / 43,938 object pixels, and they are the second object's interior XOR
boundaries — **710 of 711** at indices < 16 at 3004 (one at ≥ 16) and all
839 at 3404. Its outer red silhouette matches within **118 / 180 px** (the
port's red masks hold 27,057 / 33,312 px, the reference's 27,143 / 33,416).
The first object matches exactly at 2305. The equal `INIT_320X200` constants and the
chunky 320-row model reproduce the reference's 2300 object pixels exactly at
2305, so D24 (a)'s shipped projection/window is **not** contradicted and no
shipped constant is adopted (D36's 92-byte row stays a memory-layout fact).

### 11.4 What this settles, and what it carries

- **The D24 questions.** (a) Projection/window: the tree's `INIT_320X200` and
  the 320-pixel-row picture reproduce the reference's 2300 object pixels
  exactly (2305, 0 differing with an index ≥ 16); no shipped constant needed.
  (b) The visible colour key: the reference's picture identifies the
  in-bounds low-byte reading, adopted in §11.2. (c) `L_r` against `L_p`:
  measured, **+5**, A36; the +6 wait exit and the 494/495 intro span are the
  player's phase, recorded with it.
- **The registered-unexplained residual.** The 711 / 839 surface-minimum
  pixels are registered in A37 as *unexplained*: the sub-pixel edge/phase
  cause is a hypothesis, and D24's "every other difference is accounted for"
  is not claimed for them. No constant, shade or offset resolves them
  without breaking the exact 2305 match.
- **`cross`'s short-list guard.** No measured frame shows a short polygon
  list: the 2305 residual is palette-only (5,208 px, 0 with an index ≥ 16) and
  the 3004/3404 index structures match the reference's except for the second
  object's boundary pixels (711/839 px), so the guard is not reachable in
  these frames and is not registered.
- **`NEW.ASM`'s `nep`/`ne` bound.** The keyframes show no screen data outside
  the objects' spans: the objects' boxes end at row 191/199 (the pass-2 loop
  bounds), and the port's edge list is a `Vec` with no row-table adjacency.
  The bound is not exercised visibly in the measured frames and is not
  registered.
- **The display pairing.** The port pairs surface *N* with `pal` after *N−1*
  (and the group writes of frame *N*); the reference's capture transiently
  shows surface *N−1* with the DAC of *N*. No upload order was changed to
  chase it — that would break the source's own order (`MAIN.C:414-415`
  uploads, then the body computes the next palette).
