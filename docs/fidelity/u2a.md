# U2A — the landing pad and the saucer

`SECOND 1`'s second part: the VISU vector engine's debut. The release holds a
bright landing pad through the first music gate, then draws a small saucer
against the same landscape for about 520 stream frames before clearing the
screen and holding the plain background for PAM.

The tree's sources are `VISU/C/U2A.C` (515 lines, the part), the engine it
drives — `VISU/VISU.C` (the object loop), `VISU/ACALC.ASM` (the fixed-point
matrix, rotation and projection), `VISU/ADRAW.ASM` + `ADRAWCLP.ASM` (culling,
clipping and span generation), `VISU/AVIDFILL.ASM` (the flat and Gouraud
fillers), `VISU/AVID.ASM` + `AVIDM1.ASM` (the screen, palette, window, pages
and clear) — and the packed scene `assets/VISU/C/SCENE/U2A.00M`, `.0AA`,
`.0AB`, `.001`–`.003` with the clear image `assets/VISU/C/U2ABG.UH`. The port
is `bins/secondreality/src/parts/u2a.rs`, registered as `"u2a"`, with its
integration test in `bins/secondreality/tests/u2a.rs`. The engine itself is
Tasks 3–5's (`parts/visu/{math,draw,mod}.rs`); this note covers what U2A adds
to it.

## 1. What the original does, stage by stage

- **`:145-227` load.** `readfile("U2A.00M")`; the palette merge at `:152`;
  the master's object table (the camera plus five entries, three distinct
  files, two copies); `U2A.0AA`'s first scene stream (`U2A.0AB`); then
  `resetscene()` (`:227`), which zeroes every `r`/`r0` and rewinds the stream.
- **`:229-240` the first gate.** `_asm { mov bx,6; int 0fch }` reads the
  published order into `a` and row into `b`; `if(a>10 && b>46) break;`, and
  `if(dis_exit()) return;`. Nothing is drawn and nothing is set up until it
  opens.
- **`:242-258` the setup.** `vid_init(3)` — stated as "oversample x 4" but
  `m1o_init` (`AVIDM1.ASM:71-99`) sets `_projoversampleshr` to 0 like the
  other two, so the mode is the shipped 320×200 tweak; the DAC upload
  (`:243-246`); `vid_window(0,319,25,174,512,9999999)` (`:247`);
  `dis_setcopper(2,copper2)` (`:249`); and the ring's initial state
  (`:251-258`): `coppercnt=syncframe=0`, `avgrepeat=1`, `cl[0..2].ready=0`,
  `cl[3].ready=1`.
- **`:259-431` the loop.** Each iteration checks the second gate
  (`if(a>11 && b>54) break;`, `:276`) and `dis_exit`, waits for `cl[clw]` to
  be free (breaking at `deadlock>16`, `:278-283`), points the draw segment at
  page `clw`, clears it with the background, sets the camera angle, sorts and
  draws the enabled objects, schedules the page (`:326-332`), and parses
  `repeat` stream frames (`:336-430`). `0xFF 0xFF` calls `resetscene()`, sets
  `xit`, and the outer loop ends (`:351-356`).
- **`:432-451` teardown.** `dis_setcopper(2,NULL)`; all four pages cleared
  with the background; `vid_setswitch(3,-1)` leaves the display on the last
  one; `vid_deinit()` only outside the demo.

The release does more **before** the gate than the tree: `U2A.C` draws
nothing until `:259`, while the reference's 6500 frame, 488 frames into the
pad hold, is the `U2ABG.UH` picture under a different DAC. Task 1 measured it
(`.superpowers/sdd/06-vector-engine/task-1-report.md` §4) and the controller's
ruling made it the port's `init`: set the view to 320×200, paint page 0 with
the background, upload the pad palette. The pad appears in
(5960, 5966] in the reference's captures, and this is the *release's*
behaviour, not the tree's (D24's first kind of difference: the shipped build
does something a source in the tree does not).

## 2. The screen and the four pages

The mode is the 320×200 tweak (`AVIDM1.ASM:219-...`), with an 80-byte plane
row. Drawing goes through `_vramseg`; the display through CRTC 0x0C.
`_vid_setswitch(int draw,int show)` (`AVID.ASM:341-358`) is:

```asm
	movpar	ax,0
	cmp	ax,-1
	je	@@1
	shl	ax,14-4
	add	ax,0a000h
	mov	ds:_vramseg,ax
@@1:	movpar	ax,1
	cmp	ax,-1
	je	@@2
	shl	ax,14
	mov	dx,3d4h
	mov	al,0ch
	out	dx,ax
@@2:
```

So page `p`'s drawing segment is `0xA000 + p*0x400` (physical byte
`0xA0000 + p*0x4000`) and its display start is `p << 14` (CRTC 0x0C = the
high byte of the start, `p*0x4000`). Each page is 16,000 plane bytes of
picture in a 0x4000-byte slot. In the emulator's four-plane byte-group memory
a plane byte is four linear pixels (`docs/fidelity/pam.md` §2,
`reference-indices.md` §1.6), so the original's pages are **65,536 pixels**
apart, with a 1,536-pixel gap between pictures.

The port's surface is 640×400 = 256,000 pixels and its pages are packed at
`page * 64_000`, the controller's ruling. The packing is unobservable: every
page is cleared and drawn through the port's own origin and read back through
the same `View` origin, so the picture each page shows is identical to the
original's. The reference's 6500 and 7640 frames are page 0's picture at
origin 0, and its 7300 `.idx` was read from offset 0, which is page 0 — the
one page whose origin the two layouts share exactly.

The ring is `Ring::new(1)` (`parts/visu/mod.rs`): the four logical pages
`cl[0..3]`, the copper delay of 16 retraces, and the adaptive `avgrepeat`
schedule. The port's copper slot 2 runs `ring.isr()` and then sets the view
origin to `displayed() * 64_000`. Slot 2 runs after the tick's compose
(`sr-engine/engine.rs:278-282`), so a page switch shows in the next tick's
frame, the original's retrace timing.

## 3. The tick map

The two gates and the drawing window, from the filed traces:

```bash
awk -F, '$1==6987 || $1==6988 {print $1":"$2":"$3}' \
    reference/music/second1c/trace.csv     # 6987:11:46, 6988:11:47
awk -F, '$1==7617 || $1==7618 {print $1":"$2":"$3}' \
    reference/music/second1b/trace.csv     # 7617:12:54, 7618:12:55
```

- **Gate 1 at 6988** (`second1c`: order 11 row 47 — the first row above 46
  in order 11). Gate 2 at **7618** (`second1b`: order 12 row 55), but it is
  dead in the shipped data: the stream's END arrives first. 521 drawable
  frames plus END, one stream frame a tick in the steady state, is about 522
  ticks — the cleared hold begins around 7510, and the reference's dense scan
  shows the plain landscape from 7560 at the latest to 7700
  (`reference-indices.md` §5.5, §6.1).
- **The port's END lands 521 part-ticks after its gate-open tick.** The unit
  test `teardown_clears_all_four_pages_and_leaves_page_three_displayed`
  drives a gate that opens at tick 0 and finds `Finished` at tick 521; with
  an END at ~7510 this is the measured window.
- **The gate-open tick is the loop's first tick.** `U2A.C:238-249` has no
  retrace between the gate break, the setup and the first draw, so the port
  uploads the merged palette, installs the ring's copper and draws in one
  `frame()` call. There is no setup tick to count.
- **The port's own gate opens at boot 6986, two frames before the trace's
  row change.** The block-1 music clock is the shell's, not this part's, and
  the trace is a different session (D17 measures sessions 1–4 frames apart):

  ```bash
  cargo run --release --bin secondreality -- --block 1 --headless \
      --dump-frames 6985,6986 --dump-dir out/u2a-gate
  # manifest music_position: 6985 "11:46", 6986 "11:47"
  ```

  The port's position at the keyframe itself agrees with the trace:
  `second1c` at 7300 is `12:18`, and the integration test asserts the run's
  7300 frame says `12:18`.

- **The first three loop ticks draw pages 0, 1 and 2 before the 16-retrace
  delay ends**, then the part waits on page 3 until the deadlock escape, and
  the schedule catches up by parsing 8, 8, 4 and 1 frames on four successive
  draws. This is the original's own schedule (`U2A.C:278-336`) and the port's
  ring unit tests pin its cadence (`parts/visu/mod.rs`).

## 4. The palette merge and the measured pad

`U2A.C:152` is the merge, quoted:

```c
	memcpy(scene0+16+192*3,bg+16,64*3);
```

The scene's entries 0–191 stay from `U2A.00M`; the background's 0–63 land at
192–255. The background picture itself indexes 0 and 192–242 (the payload's own
values, §7's command), so its colours resolve through the background
palette's entries 0–50 under the merged DAC. The unit test `the_merged_palette_is_the_scene_then_the_background`
builds the expected palette from both files and asserts it, plus that entry
191 is still the scene's and that the scene's own 192 differs from the
background's 0 (so the merge is observable).

The release's pad palette is **not** the merged one. It is the background
palette passed through `(3*v + 5) & 63` on every channel of every entry, with
entry 195 forced black: Task 1 reconstructed 52 entries from the 6500 RGB
against the 7640 index plane with no conflicts, and the formula reproduces
57,516 of the 57,600 pixels that use indices ≥192, the other 84 being the
index-195 stars the black entry renders. The port applies the formula to all
256 entries (`pad_entry`) and blacks entry 195; the resulting whole frame
compares at **0 of 64,000 pixels** against `frame_006500.rgb`
(`the_pad_hold_is_the_reference_6500`, and the integration test).

At gate 1 the merged palette goes up (`U2A.C:243-246` in the source, after
the gate); the port uploads it on the gate-open tick, before the first draw.

## 5. The reference comparison, and the 7300 divergence

Three keyframes, captured with `SECOND 1` (`reference/parts/u2a/`:
6500 the pad, 7300 the saucer, 7640 the cleared hold):

| index | port | result |
|---|---|---|
| 6500 | `u2a_006500` | **0 differing** RGB pixels (`the_pad_reference_6500_compares_at_zero`, and the unit test) |
| 7640 | `pam_007640` | **0 differing** indices and RGB pixels (`the_cleared_hold_reference_7640_compares_at_zero_in_both_planes`) |
| 7300 | `u2a_007300` | **1098 differing** indices and RGB pixels (`the_saucer_frame_records_its_measured_divergence`) |

The 7640 file is PAM's by name because PAM is the part running there: U2A's
END has retired it, and PAM's own music gate (`dis_sync()<10`, code 9 at
order 12) is still closed, so PAM inherits and holds U2A's cleared page. Both
planes agree with the reference exactly — the page is the `U2ABG` picture
under the merged palette, which is the `0a68ffb9…` frame the reference holds.

The 7300 divergence, measured rather than fitted:

- **The reference's picture is stream state 294.** The unit test
  `the_reference_7300_is_the_measured_stream_state` renders all 522 states
  through the same driver and shipped window the part uses. The minimum
  differs by **426** pixels at state 294; the states either side differ by
  892 and 858.
- **The residual at that best state is on the Gouraud-shaded faces only.**
  The object's silhouette is exact — **0** pixels differ between "object" and
  "background" against the reference's own index plane. Attributing every
  differing pixel to the topmost object that writes it, by drawing each
  object alone at the same state, gives

  ```
  [instance 1: 75, instance 2: 0, instance 4: 115, instance 5: 236]
  ```

  Instance 2 is `U2A.002` ("Sippi", 75 faces, all flags `0x0800` =
  `F_SHADE16`, flat-filled); instances 1, 4 and 5 are `U2A.001` ("s01", 124
  faces, `F_SHADE32`/`F_SHADE16` Gouraud). The flat object reproduces
  exactly; the Gouraud copies leave small index deltas on their faces:
  `{-5,-4,-3,-2,-1,1,2,3,4,5,6,7,8,10,11,12}`.
- **The port's live run does not show state 294 at 7300.** Its schedule
  displays a different page of the same animation, and the whole pose
  differs: 1098 pixels, in both planes. The gate tick measured above (6986)
  and the fixed-tick ring's page holds put the port's displayed page at a
  later state than the reference's page at this index, so the pose itself is
  different. This is D17's session phase, made visible because the part's
  animation is a stream of integer states and a page holds several of them,
  not a continuous clock.
- **The 426-pixel Gouraud residual is a D24 item, not a transliteration
  slip.** The port's draw path is `parts/visu/draw.rs`'s transliteration of
  `ADRAW.ASM`/`ADRAWCLP.ASM`/`AVIDFILL.ASM`, whose tier, clipping and
  lighting tests are hand-derived from those files; the tree's two filler
  versions (`AVIDFILL.ASM`, `AVIDFILL.OK`) differ only in their copy loops,
  not in the Gouraud tier code; and every flat-shaded face matches the
  reference exactly (the attribution above). The residual is therefore the
  release's Gouraud output against the tree's algorithm: small (±1–12
  palette-index steps) and confined to the Gouraud faces, but not localised
  to a single constant the tree's algorithm can carry, so D24's step 3
  registers it rather than adopting a fitted value. The note records the
  measurement for the controller's ruling.

## 6. The bugs and hardware behaviours, preserved

- **The uninitialised `fov`.** `int fov;` is declared inside the outer loop
  (`U2A.C:261`) and read by `vid_cameraangle(fov)` (`:287`) before the first
  parse assigns it (`:348`). On the first draw every object is still off, so
  the value is invisible; from the second draw on the stack slot holds the
  last parsed frame's `0x2200`. The port's `Driver` starts at fov 0 and the
  first draw is background-only, then the stream's `0x22` takes over.
- **The deadlock escape draws into a queued page.** `while(cl[clw].ready)`
  breaks at `deadlock>16` and draws anyway (`:278-283`); the port's
  `next_page` returns the busy page once the copper has counted past 16.
- **The 16-retrace start-up delay parses a burst.** The first pages are
  drawn before the copper delay expires; the schedule then queues 8, 8, 4, 1
  frames on four draws. The cadence is Task 5's modelled ring and is pinned
  by `ring_matches_the_u2a_cadence_over_twenty_ticks`.
- **The second gate is dead in the shipped stream.** The END's `resetscene();
  xit=1` (`:351-355`) ends the loop before order 12 row 55; the port keeps
  the check first in its loop (`the_second_gate_tears_down_on_the_first_tick_it_is_open`).
- **The teardown clears all four pages and leaves the display on page 3.**
  `U2A.C:432-441`; the source removes slot 2, the port installs no-ops in all
  three the way PAM does, so nothing an unregistered slot held can run.
- **The background is not de-interleaved.** `U2A.C:153-166` reorders `bg` in
  place before the loop (`for(z=0;z<4;z++) for(y=0;y<200;y++)
  for(x=z;x<320;x+=4) bg2[u++]=bg[16+768+x+y*320]`). Task 1 measured the
  reference's 7640 index plane as the packed payload byte for byte, and Task
  A8's decoder already returns that payload as linear 320×200, so the port
  copies `Image::pixels()` and does not port the shuffle.
- **The wait is a tick, not a spin.** Every `while(cl[clw].ready)` and every
  gate iteration is one `frame()` call at the fixed 70 Hz tick;
  `ctx.exit_requested` is read every tick, including the pad hold (where the
  source's `if(dis_exit()) return;` leaves the pad and the shell retires the
  part).
- **No music is a refusal.** The gates read the player's published order and
  row; a run with no player is a `PartError` at `init`, following PAM.

## 7. Reproducing the numbers

```bash
# the part's unit tests: the pad palette and merge, the two gates, the first
# stream frame and frame 16's toggle, the END's tick and the teardown, and the
# 7300 state search with its silhouette and per-object attribution
cargo test -p secondreality --bin secondreality u2a

# the integration test: the manifest, the 6500 and 7640 keyframes at zero,
# and the measured 7300 divergence
cargo test -p secondreality --test u2a -- --nocapture

# the reference comparison by hand (from the repository root):
cargo run --release --bin secondreality -- --block 1 --headless \
    --dump-frames 6500,7299,7300,7301,7640 --dump-dir out/u2a
cargo xtask compare --reference reference/parts/u2a --rendered out/u2a \
    --out out/u2a-diff

# the gates, from the filed traces:
awk -F, '$1==6987 || $1==6988 {print $1,$2,$3}' reference/music/second1c/trace.csv
awk -F, '$1==7617 || $1==7618 {print $1,$2,$3}' reference/music/second1b/trace.csv

# the port's own gate tick, from its manifest:
cargo run --release --bin secondreality -- --block 1 --headless \
    --dump-frames 6985,6986 --dump-dir out/u2a-gate

# the background payload's own indices (the line §4 quotes): its distinct
# values are {0} U {192..242}, 52 of them
python3 -c "d=open('assets/VISU/C/U2ABG.UH','rb').read()[784:784+64000]; \
print(sorted(set(d))[0], sorted(set(d))[1], sorted(set(d))[-1], len(set(d)))"

# and the reference's held 7640 hash (§5's 0a68ffb9...):
sha256sum reference/parts/u2a/frame_007640.rgb
```

`cargo xtask compare` pairs by index and searches ±1 frame. On the dump above
it reports `0.0000%` at 6500 and 7640, `1.7156%` (1098/64000, max delta 238)
at 7300, `verdict: different`, and exits 1: the port's page at that index is
a different stream state, so no ±1 phase match applies.
