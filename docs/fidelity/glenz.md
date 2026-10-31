# GLENZ — the intro and the glenz vectors

**Sources.** `GLENZ/MAIN.C` (648 lines) over the part's own `VEC.ASM`, `MATH.ASM`, `MATHSIN.INC`
and `NEW.ASM`, with the zoom-and-bounce opening from `ZOOMER.C`. **Port.**
`bins/secondreality/src/parts/glenz/` (`"glenz"`): `mod.rs` (the driver, the buffers, the tick
model, the palette, the exit), `math.rs`, `tables.rs`, `poly.rs`, `span.rs` and `intro.rs`; their
module docs hold the mechanism. Integration tests `bins/secondreality/tests/blocks.rs` (`glenz::`) and `tests/parts.rs`. **Run.** In
the boot numbering of the block that reaches it: block 2 from 576, block 1 from 8,057 (§10).
**Music.** `MUSIC1` order 0; the part waits on `dis_musplus()` and `dis_getmframe()`.

## 1. Timeline

One logic tick is one `dis_waitb` (`DIS/DISC.ASM:42-47`). Block 2, measured on
`reference/music/second2b` and the `SECOND 2` captures (`reference-indices.md` §7.6):

| reference | call | source |
|---|---|---|
| 1110 | WaitMusic: `dis_musplus()` is −20 | `MAIN.C:290` |
| 1111 | the wait exits, and `zoomer2`'s first body is the same call | `:291-293`; `ZOOMER.C:38-69` |
| 1111..1157 | 47 zoomer bodies; the 47th caps `zy` at 260 | |
| 1158 | the 48th call: break, epilogue, mode set, setup, bounce body 1 | `ZOOMER.C:70-80`; `MAIN.C:296-350` |
| 1158..1334 | 177 bounce bodies; the last draws rows 153..199 | |
| 1335..1515 | Wait300; zframe 300 at 1515; then `backpal`/`tmppal` (1516), `bgpic = vram` (1517) | `:353-400` |
| 1518..1561 | Wait333; zframe 333 at 1560; `pal = backpal`, `dis_partstart()` (1561) | `:400-407` |
| 1562 | the main loop's first `dis_waitb` (`L_r`); frame 1 at 1563 | `:409-641` |
| 1606 | main-loop frame 44: the first object pixels | §6 |

The trace rows are `second2b`'s (§11's `awk` lines): 1110 `2:44`, 1111 `2:45` with zframe 0, and
zframes 300, 333 and 335 at 1515, 1560 and 1562.
`the_intro_is_47_plus_177_ticks_and_the_measured_waits` pins the port's offsets from the reset
(`Main` at +451, frame 1 at +452) on a scripted clock at MUSIC1's tempo-130 rate (52,000/70,086 ≈
0.7419 zframes a frame), which passes the waits at the measured +404 and +449. The port's own run
is five ticks later (§11.1).

## 2. The buffers and the view

The two buffers (`vram` 320×400, `bgpic` 64,000 bytes, D36) are in the module doc. Decisions taken
here:

- **A run that starts at GLENZ** has no BEGLOGO picture to inherit: `init` installs the 320×400
  view itself and copies the surface's first 320 columns of rows 0..400, which in a fresh run are
  the shell's black. The opening plays over black; the geometry and tick map are the same.
- **Mode 13h is `height=200, double_scan=false`.** The guest's `is_double_scanned` (D17's geometry
  note) is not the engine's `View::double_scan`, which would display only surface rows 0..99; the
  reference's frame is the emulator's halved 320×200, 200 distinct rows. 1499's rediff entry (§6)
  fails with the flag set.
- **The mode-set tick composes at 320×400** (the engine sizes a frame from the view at the tick's
  start): one tick, 1158, outside every keyframe.
- **The compositor repaints every span** from `bgpic` instead of `ng_pass3`'s two-list walk
  (`NEW.ASM:66-172`): A39, measured difference **0**
  (`the_full_repaint_equals_the_source_two_list_walk`, over a 13-frame corpus including pixel
  63,999).

## 3. The copper, substituted

No copper callback (D37): the per-tick `set_palette_entry(0..15)` plus `poly::build`'s group writes
are the same DAC state at compose time. The upload never touches entries 16..255
(`the_low_palette_upload_never_touches_the_groups`). The displayed palette for main frame *N* is
`pal` as it stood after frame *N−1*: the source uploads before it computes (`MAIN.C:414-415`), so
the frame-790 `pal = backpal` shows from 791 (`bugs.md` §23).

## 4. The palette phases

`MainState::step` is `MAIN.C:417-591`'s inner body, and
`the_palette_phases_have_the_source_boundaries` pins its phases by main-loop frame: `backpal`
through 700; the fade `backpal*(764-frame)/64` over 701..764; 0 through the band clear 765..789,
with `backpal` rebuilt at 786..789 from the bit pattern; `pal = backpal` at 790..794; unchanged
until the second fade `backpal*(2133-frame)/64` over 2070..2133; 0 from 2134.

The rebuilt `backpal` (`MAIN.C:557-584`) leaves entries 0 and 4 black or dark red in the second
half. The band clear (`:552-556`) writes rows 150..199 of both buffers, two a frame, before
`cglenzdone` paints (`:640`; `the_band_clear_is_at_rows_150_onwards_and_stops_at_789`). `tmppal`
(`:365-390`, `the_wait300_tick_uploads_tmppal`) is `backpal` in entries 0..15.

## 5. The objects and the physics

Both objects are 14 vertices and 24 triangles (`MAIN.C:60-149`; `poly.rs`). `rx += 32`, `ry += 7`
each frame, `% 10800`; `zpos` is 7500. The first object is on while `xscale > 4` (through frame
2119); the second from frame 803 through 2184; `lightshift` turns 10 at frame 858
(`the_object_switches_follow_the_scales`).

**The 16-bit arithmetic is the physics.** Microsoft C large model (`GLENZ/MAKEFILE`: `cl /qc /AL`),
and `V.EXE`'s disassembly (`MAIN_TEXT`, `V.MAP:3`) shows each product is a 16-bit `imul` with `DX`
discarded by the `cwd` before `idiv`. Two places wrap in the shipped animation:
`yposa = -yposa*boingm/boingd` (`MAIN.C:511`; −4,016×10 → 25,376, so `xscale` is 115 at frame 800)
and `oyp -= b*b/2` (`:477`, from frame 1801)
(`the_object_switches_follow_the_scales` pins `xscale` 115; `bugs.md` §23).

`sin1024` (`MAIN.C:456-471`) is `ZOOM.ASM:28-29`'s `_sin1024`, a different table from `SIN`/`COS`
(`parts::sin1024::SIN1024`). The `frame>1400+789` bscale-shrink arm is unreachable (`frame>1220+789`
catches every value first) and is kept in source order.

**Generated and self-modifying code.** The shipped path calls `ceasypolylist` +
`cglenzinit/polylist/done` + `NEW.ASM`, never `polygroup`/`polyf`. `ZOOMLOOP.INC` is linked
(`ZOOM.ASM:26`, `GLENZ/MAKEFILE:3`) but is the body of `_zoom`, which only the unshipped `zoomer1`
calls; `ZOOM.ASM` exports only `_zoom` and `_sin1024`. `VEC.ASM`'s matrix-in-immediates and patched
group loops are state only, held in variables.

## 6. The held opening, measured

The final bounce frame is the reference's held 1499: rediff's `glenz` entry gives **0** differing
indices against `frame_001499.idx` (rows 154..199; row 153, `fcrow[0]`, is black;
`reference-indices.md` §7.3's census) and 0 in RGB. The picture holds for 272 ticks, 1334..1605.

The first object at frames 44..49 matches `python3 -I tools/glenz_physics.py entry` exactly
(8/21/60/86/111/137 pixels, centroids (175.5,0.0) … (165.5,2.0);
`the_first_object_appears_on_frame_44_with_the_measured_silhouettes`). The first main frame is
`bgpic` (`the_first_main_frame_is_the_background`).

## 7. Decisions (D24)

Each is decided by the reference; nothing was fitted.

- **(a) Projection and window.** The tree's `INIT_320X200` (256/213/160/130) and the 320-pixel-row
  picture reproduce the reference's 2300 object pixels exactly at 2305: 9,989 px, centroid
  (156.8, 94.1), every object pixel byte-identical (§11.2). No shipped constant is adopted; the
  92-byte CRTC row (`GLENZ/VIDINIT.ASM:306`) stays a memory-layout fact (D36,
  `reference-indices.md` §7.1, §7.3).
- **(b) The visible colour key.** Adopted: the low byte, one shared map (§11.2, A37).
- **(c) `L_r` against `L_p`.** Measured, **+5** (§11.1, A36).
- **The display pairing.** The port pairs surface *N* with `pal` after *N−1*; the capture shows
  surface *N−1* with the DAC of *N* (A36). No upload order is changed to chase it.

## 8. The unreached guards

- **`cross`'s short-list guard.** `poly::cross` returns 0 (visible) when fewer than three pairs
  are stored; the asm reads three unconditionally. The shipped tables always store three; the guard
  is reachable only if two projected vertices coincide and the closing-duplicate drop fires. No
  measured frame shows a short list (the 2305 residual is palette-only; 3004/3404 differ only on the
  second object's boundary), so nothing is registered.
- **`NEW.ASM`'s `nep`/`ne` bound.** The original's 256-word row table is followed by the edge
  arena (`NEW.ASM:11-16`), so a projected upper y above 255 would land inside it. A conservative
  bound puts the worst shipped case near y≈252, so it is not proven unreachable; the port's edge
  list is a `Vec`. The keyframes' boxes end at rows 191/199, so nothing is registered.

## 9. Exits

- **ESC during the music wait or the zoomer** still plays all 47 zoomer bodies: `MAIN.C:290` exits
  and continues into `zoomer2`, which checks only `kbhit` (`ZOOMER.C:38`). The bounce
  (`MAIN.C:320`) and both `dis_getmframe` loops (`:353`, `:400`) test `dis_exit`, so ESC before the
  bounce skips the bounce and both waits; the `dis_waitb`s, `backpal`/`tmppal`, the `bgpic`
  snapshot and the part-start still run, and the part retires on its first main-loop check.
- **The main loop** (`:409-411`): a cut, ESC or the frame-7000 cap first seen on a body tick keeps
  that frame; one seen at the loop entry draws nothing (`a_stop_at_the_main_wait_draws_nothing`,
  `a_stop_on_a_body_tick_draws_then_finishes`).

## 10. Music and the block boundaries

The restart and the post-PANICEND cue wait are the shell's: part-sequence.md (block seams /
pre-roll / mapping conventions), D38. GLENZ's numbers:

- **Block 2** keeps its player (it already plays `MUSIC1` from order 0, `MAIN/U2.ASM:772-775`).
  GLENZ starts at **576** with `start_frame` 408
  (`a_block_two_run_reaches_glenz_without_reloading_music_one`). The wait exits at **1,117** in the
  port (`2:44` through 1,116) against `second2b`'s **1,111**, a session-phase difference larger
  than A12's 5-frame worst for `second2b`; registered with A36.
- **Block 1** restarts `MUSIC1@0` at **8,057** (`MAIN/U2.ASM:813-818`, inside D38's measured
  interval (8049, 8100); `the_block_one_run_restarts_music1_at_glenz`). The restart keeps
  `MUSIC0`'s channel volumes (D45), so order 0 takes its row-3 `C00` and lasts 4 rows (`1:0` at
  8,087). The wait ends on `2:45` at **8,527**, 470 ticks after the boundary and **6 frames after
  the reference's 8,521**: 5 are the restarted clock's four-tick wait (D34), 1 the trace's sample
  (the picture first differs at 8,522, D17). That is A38. `fullrun/04-glenz-techno`'s 8572 and 8900
  are 0 in both planes (A53).

## 11. The block-2 comparison

```text
cargo run --release --bin secondreality -- --block 2 --headless \
    --dump-frames 1499,2304,2305,2306,3003,3004,3005,3006,3403,3404,3405,3406 \
    --dump-dir out/glenz-final
cargo run --release --bin secondreality -- --block 2 --headless \
    --dump-frames 1499,2300,3000,3400 --dump-dir out/glenz-indexed
cargo xtask compare --reference reference/parts/glenz \
    --rendered out/glenz-indexed --out out/glenz-indexed-diff
cargo test -p secondreality --test blocks glenz::
cargo xtask rediff --only glenz
awk -F, '$1 >= 1109 && $1 <= 1112' reference/music/second2b/trace.csv       # §1
awk -F, '$1 == 1515 || $1 == 1560 || $1 == 1562' reference/music/second2b/trace.csv
```

The manifest says `index_origin: boot`, `anchor: 404`, `music: MUSIC1@0, start_frame 408`. At the
filed indices `compare` reports **1499 at 0.0000 %** and exits 1: the moving frames at their own
indices are five ticks behind, 22,632 / 15,333 / 19,708 px (35.3625 / 23.9578 / 30.7938 %) at
2300/3000/3400.

### 11.1 The measured phase

The port's reset-to-frame-44 span is measured two ways: the first above-floor object pixels are at
boot **1611** (8 px, box 172..179 × 0..0; none at 1610, 21 px at 1612), and the first surface
carrying the fade is **2269** (`pal` after frame 701, b = 63; 2268 is b = 64). Both give
**`L_p = 1611 − 44 = 2269 − 702 = 1567`** against `L_r = 1606 − 44 = 1562`, so the span is **494**
ticks against the reference's **495**. The phase machine's relative ticks match the source (§1);
the one tick is the real player reaching zframe 300/333 one tick earlier relative to the reset than
`second2b`, the same class as the six-tick wait exit. **Main-loop frame `f` is boot `1567 + f` in
the port and `1562 + f` in the reference, so the filed `K` is answered by `K + 5`.**

The capture adds a surface/DAC phase (A36):

| filed index | reference counter frame | filed surface frame | filed DAC |
|---|---|---|---|
| 2300 | 738 | 738 (lag 0) | `pal` after 738 (b = 26), i.e. mid-frame-739 |
| 3000 | 1438 | 1437 (lag 1) | the frame-1438 group writes |
| 3400 | 1838 | 1837 (lag 1) | the frame-1838 group writes |

The capture samples after the source's upload or group writes but before the surface is painted, so
its DAC leads its surface; the lag growing from 0 to 1 is the capture's scheduling (D17). The
surface-aligned frames (2305, 3004, 3404) are the **unique minima** of the index-structure metric:
their neighbours' unexplained counts are 1,859/1,740 (2304/2306), 4,486/2,837 (3003/3005) and
5,876/4,780 (3403/3405), against 0/711/839 at the minima.

### 11.2 The colour key: the reference identifies the low-byte reading (adopted)

`VEC.ASM:450` keys the tree's visible path by the full colour word (`movzx bx,es:[bx-2]`) and
`:417` the hidden path by the low byte, so with a 256-byte `rolcol` a hidden face never frees a
visible slot and the fifteen-slot pool fills with every word ever visible. The packed EXE cannot be
read, so the picture decides:

| model | boot 2306 (main frame 739) | low palette |
|---|---|---|
| full-word key (the tree) | object box (99,216, 37,149), **8,066 px**; sky `(97,97,97)` | DAC entry 0 is a slot-32 group: the background reads the group ramp, and every post-exhaustion face is painted with colour 0 and vanishes |
| low-byte key (adopted) | object box (99,217, 37,149), **9,927 px**, centroid (157.6, 94.2), asserted at 2306 | entry 0 black; every face drawn |

The full-word row's picture numbers are from the tree before the adoption (commit `63b4267`, §11's
dump command); the mechanism they show is pinned on the current tree by
`glz_pool_exhaustion_uses_slot_32_and_entry_0`. The adopted reading is one `slots: [u8; 256]` keyed
by the low byte, shared by both paths (`glz_hidden_face_frees_the_visible_slot`; D36, A37). At 2305
the object box is the filed box exactly, and **all 5,208** differing pixels are palette entries
0..15: the reference's colours are the fade at **b = 26**, the port's **b = 27**, the capture's
one-frame DAC lead.

### 11.3 The comparison, at the measured alignments

| port boot | reference | differing px | unexplained | reading |
|---|---|---|---|---|
| 1499 | 1499 | **0** (indices and RGB) | 0 | the held opening |
| 2305 | 2300 | **5,208** (8.1375 %) | 0 | `K + 5`; the 2300 surface (lag 0), DAC phase only |
| 3005 | 3000 | **3,114** (4.8656 %) | 2,837 | `K + 5`; one frame after the filed surface |
| 3405 | 3400 | **5,261** (8.2203 %) | 4,780 | the same |
| 3004 | 3000 | **2,289** (3.5766 %) | 711 | the surface minimum (`K + 4`, lag 1) |
| 3404 | 3400 | **5,941** (9.2828 %) | 839 | the surface minimum |

`cargo xtask rediff` registers 1499 at 0 in both planes and 2305/3004/3404 at 5,208 / 2,289 / 5,941
(A37). At the surface minima the unexplained pixels are **711 / 839** of 35,896 / 43,938 object
pixels, the second object's interior XOR boundaries (710 of 711 at indices < 16 at 3004, all 839 at
3404); the outer red silhouette matches within **118 / 180 px** (port 27,057 / 33,312, reference
27,143 / 33,416). The first object matches exactly at 2305.

## 12. Open items

- The 711 / 839 surface-minimum pixels are registered in A37 as **unexplained**: a sub-pixel edge or
  phase cause is a hypothesis, and no constant, shade or offset resolves them without breaking the
  exact 2305 match.

**Register.** `bugs.md` §5 (the span raster) and §23 (the 16-bit products, the upload order, the
clear before the paint).
