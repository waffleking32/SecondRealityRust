# U2E — the city flythrough

`SECOND 4`'s only show part and block 3's tail after the mid-run restart: the
VISU engine's second and last part. It fades the screen in over a loader-black
hold, draws a border-and-letterbox frame, waits for `MUSIC0` order 18 to end,
then plays a 1,801-frame animation stream — the corridor, the towers and the
green craft, the console, the crescent ships — at one stream frame per two
retraces, and fades the captured DAC out when the stream ends.

The tree's source is `VISU/C/U2E.C` (628 lines) over the same VISU library as
U2A (`VISU/VISU.C`, `ACALC.ASM`, `ADRAW*.ASM`, `AVIDFILL.ASM`, `AVID.ASM`,
`AVIDM1.ASM`) and the packed scene `assets/VISU/C/SCENE/U2E.00M`, `.0AA`,
`.0AB`, `.001`–`.042`. The port is `bins/secondreality/src/parts/u2e.rs`,
registered as `"u2e"`, with its integration test in
`bins/secondreality/tests/u2e.rs`. The engine is Tasks 3–5's
(`parts/visu/{math,draw,mod}.rs`); this note covers what U2E adds.

## 1. What the original does, stage by stage

- **`:185-283` the mode check and the load.** `int 10h / ah=0fh` reads the
  current video mode into `a`; `if(a>3) jellywas=1` (`:198-205`). Then
  `U2E.00M` (city byte 15 = 0, so neither city hack applies; it is U2A's flag
  and U2E never reads it), one `.00N` object per distinct number (58 `co[]`
  entries: the camera plus 57), `U2E.0AA` → the single `.0AB` stream, and
  `resetscene()` (`:271`).
- **`:273-283` the intro palette.** A fresh boot (`jellywas == 0`) runs
  `vid_init(1)` and sets `fpal` to entries 1..63 = 63, the rest 0. The other
  path captures the live DAC:
  `outp(0x3c7,0); for(a=0;a<768;a++) fpal[a]=inp(0x3c9);` (`:281-282`).
- **`:285-335` the 100-wait intro.** 33 fade-in steps (`+2` to bytes 3..761,
  clamp 63, one `dis_waitb()` and one 768-byte upload each), 16 waits, then
  `fadeset` into page 0, a wait, CRTC 9 `(old & 0xf0) | 0x80`, a wait,
  `fadeset` into pages 1..3, 16 waits, and 33 fade-out steps (`-2` to bytes
  3..758, `+2` to bytes 759..764, both clamped). There are exactly **100
  `dis_waitb()` calls** in the intro.
- **`:336-353` the setup.** `vid_init(11)` (state only: the same 320x200 tweak
  without `tweak320x200`), the `.00M` palette with entries 252..255 patched,
  `vid_window(0,319,25,174,512,9999999)`, and `dis_setcopper(2,copper2)`.
- **`:357-366` the gate.** `_asm { mov bx,6; int 0fch; mov a,cx }`; break when
  the published order is `> 18`, exit-aware.
- **`:368-374` the ring reset.** `coppercnt=syncframe=0`, `avgrepeat=1`,
  `cl[0..2].ready=0`, `cl[3].ready=1`. `copperdelay`, `clr`, `clw` and
  `frames[]` are *not* reset; the copper has been running since `:353`, so by
  the shipped gate (boot 699, 181 ticks after the setup) its 16-retrace
  start-up delay is spent.
- **`:376-540` the loop.** The first iteration (`firstframe`) draws nothing,
  resets the clock and queues the writer page (page 0, the fadeset page);
  thereafter each iteration waits for a free page, clears it with
  `vid_clear255()` (all bytes 0xFF, `AVID.ASM:299-309`), sets the camera
  angle, sorts and draws the enabled objects, schedules and queues the page,
  and parses `repeat = (a+1)/2` stream frames, advancing `currframe` by
  `repeat*2`. `0xFF 0xFF` resets the scene and sets `xit`.
- **`:541-559` the teardown.** Slot 2 removed, `outp(0x3c7,0)` and the live
  DAC captured into `fpal`, then 16 steps of `+4` (clamp 63), each computed and
  uploaded a retrace later from DAC entry 255; the sixteenth upload is the
  part's last visible tick. No page is cleared.

## 2. The screen, the four pages and the copper

The mode is the shipped 320x200 tweak: one page row is 80 plane addresses, each
address four linear pixels. `vid_setswitch(int draw,int show)` (`AVID.ASM:
341-358`) sets the drawing segment and, for `show != -1`, CRTC 0x0C to
`page << 14`; U2E calls `vid_setswitch(-1, clr)` from its copper, so only the
display start moves. The port's surface packs the four pages at
`page * 64_000` pixels (`PAGE_PIXELS`), the same packing U2A uses; each page is
a linear 320x200 picture and the view reads the displayed page's origin.

`copper2` (`:115-143`) is a direct transliteration in `parts/visu/mod.rs`'s
`Ring`: every retrace increments `syncframe`, frees the pages marked 2, counts
`deadlock` and `coppercnt`, runs the `copperdelay` countdown, and when it
reaches zero displays `clr` for `cl[clr].frames` retraces. The port's slot 2
runs `ring.isr()` and then `view.set_origin(displayed() * 64_000)`. Slot 2 runs
after the tick's compose (`sr-engine/engine.rs:278-282`), so a page switch is
visible from the next tick — the original's retrace timing.

The port's loop is one `frame()` call per fixed 70 Hz tick: the first loop tick
queues page 0 and parses one stream frame; each later tick takes the writer
page if the ring has freed it (a busy page is that tick's wait, not a spin),
draws, schedules, queues and parses. The resulting cadence is one stream frame
per two ticks; eight ticks advance four frames
(`the_cadence_over_eight_ticks_advances_four_stream_frames`).

## 3. The tick map and the intro evidence

The run anchor is derived, not measured directly: the loader's flat black
holds through 466 and the fadeset's white band appears at **467**, in two
independent dense scans (`reference-indices.md` §18.7). The source draws that
band after 33 fade-in waits and 16 further waits and *before* the fadeset's own
`dis_waitb()`, so it is the part's tick **49** and the intro's tick 0 is
`467 - 49 = **418**` (`shell::U2E_ANCHOR`). The part's unit test
`the_fadeset_lands_on_tick_49_and_paints_the_measured_band` drives 49 ticks of
black and finds the band on tick 49; the candidate 417 would put the band at
boot **466** instead of 467.

The measured milestones (`cargo xtask reference capture --arg 4 --boot autoexec
--keys "KBD_left KBD_enter" --cycles 30000 --frames …`):

| boot | screen | source |
|---|---|---|
| 420–450 | flat black, one colour, `ea0787f6…` — the loader | `out/u2e-start`, `-b` (the grid's 420/430/440/450) |
| 456–466 | the same flat black | `out/u2e-dense-a`, `-b` |
| 467–473 | the band, `6eb4e4cd…`: 185 white at x 68..252 and 135 black in every row (37,000 white, 27,000 black) | `out/u2e-dense-a`, `-b` |
| 480, 490, 500 | the fade-out: the band's top/bottom (index 252) dims and the middle sides (index 254) brighten; at 490 entry 252 = 53 (215) and entry 254 = 10 (40), at 500 33 (134) and 30 (121) | `out/u2e-start`, `-b` |
| 530, 590, 660 | the post-fade hold, `befaf38a…`: two colours, mean 191.25 — rows 25..174 white, the rest black | `out/b4b-u2e-sweep2`; `reference-indices.md` §18.6 ties it to a `SECOND 3` boot's 18000/18100 |
| 600 = 700 | a *different* 4-colour hold (`ef539f9e…`, mean 158.30) in the older `out/b4b-u2e-sweep` session | `reference-indices.md` §18.2; see §7's concern |
| 698 (port) / 699 (`second4`) | order 19 first published — the gate; the port's clock is one frame ahead of the trace | `tests/u2e.rs`'s `the_block_four_clock_reaches_order_19_at_698`; `reference/music/second4/trace.csv` |
| 800 | the corridor, 37 colours | `out/b4b-u2e-sweep` |
| 1000 | the console, 36 colours | `reference/parts/u2e/frame_001000.rgb` |

The port reproduces the intro **byte for byte**: a `--block 4` run's 460, 467,
470, 480, 490, 500, 530, 590 and 660 compare at **0 differing** against those
sessions' RGB captures. The dump and comparison are:

```bash
cargo run --release --bin secondreality -- --block 4 --headless \
    --dump-frames 460,467,470,480,490,500,530,590,660 --dump-dir out/u2e-intro
cmp out/u2e-intro/u2e_000460.rgb out/u2e-dense-a/frame_000460.rgb
cmp out/u2e-intro/u2e_000467.rgb out/u2e-dense-a/frame_000467.rgb
cmp out/u2e-intro/u2e_000470.rgb out/u2e-dense-a/frame_000470.rgb
cmp out/u2e-intro/u2e_000480.rgb out/u2e-start/frame_000480.rgb
cmp out/u2e-intro/u2e_000490.rgb out/u2e-start/frame_000490.rgb
cmp out/u2e-intro/u2e_000500.rgb out/u2e-start/frame_000500.rgb
cmp out/u2e-intro/u2e_000530.rgb out/b4b-u2e-sweep2/frame_000530.rgb
cmp out/u2e-intro/u2e_000590.rgb out/b4b-u2e-sweep2/frame_000590.rgb
cmp out/u2e-intro/u2e_000660.rgb out/b4b-u2e-sweep2/frame_000660.rgb
```

Every `cmp` exits 0. The fade-out's step arithmetic is pinned too: the first
fade-out mutation is on part tick 67 and its upload on tick 68, so boot 486 is
step 1 and the measured 490 (tick 72) is step 5 and 500 (tick 82) is step 15
(`the_fade_out_starts_on_tick_68_and_matches_the_measured_490_and_500`).

## 4. The palette, the jelly substitution and the fadeset

`fadeset` (`U2E.C:145-183`) is a planar transliteration. The map mask 15 makes
a `memset` address write all four planes, i.e. linear pixels `4A..4A+3`; the
`3c5=2+4+8` write at byte 63 sets only planes 1..3, so x 252 keeps the byte's
plane 0 and x 253..255 take the special value. The three blocks are:

| rows | x 0..67 | x 68..252 | x 253..255 | x 256..319 |
|---|---|---|---|---|
| 0..24 | 0 | 252 | 0 | **not written** |
| 25..174 | 254 | 253 (x 252 = 253) | 254 | 254 |
| 175..199 | 0 | 252 | 0 | 0 |

**The top block's last write is `memset(vram+y*80+252,0,16)`** — 172 addresses
past the row just written, which lands in rows `y+3..` and is overwritten by
the later row passes. The brief for this task calls it the "top/bottom blocks'
`+252` write"; the source has `+252` only in the top block (the middle and
bottom blocks write `+64`, within their own row), and the source is the
authority. Every stray address is in-page and overwritten; the port bounds
writes at the page's 16,000 addresses anyway, which is the original's 0x4000
page slot's 384-byte gap and never the next page.

The fadeset's visible effect at 467 is exactly the measured band under the
fade-in's saturated palette: entry 0 black, 252/253 white, 254 black; the top
rows' x 256..319 keep the loader page's pixels (black). The unit test
`the_fadeset_writes_the_source_s_pattern_within_its_page` pins the whole
table, the unwritten top columns and the page bound.

**The jelly branch, and why the port always takes the capture.** The source's
`jellywas` is the BIOS video mode (`a > 3` is mode 13h and above, `:198-205`):
`jellywas == 0` runs `vid_init(1)` and initialises `fpal` to entries 1..63 =
63, the rest 0 (`:274-277`); `jellywas == 1` captures the live DAC
(`outp(0x3c7,0); fpal[a]=inp(0x3c9)`, `:281-282`).

**The port runs the graphics-mode capture in every run shape.** Every way a
run reaches U2E begins in the port's graphics surface, so the mode the source
reads is always above 3: `--block 4`/`--part u2e` captures the shell's boot
DAC (`hardware_default_palette`, the emulator's mode-set table), a mid-chain
entry captures the predecessor's live DAC. The text-mode `vid_init(1)`
initialisation is unreachable and is not implemented. This replaces the
earlier `PartCtx::music_fresh()` proxy, which the U2E-boundary restart broke:
`set_music` re-arms the flag, so a mid-chain U2E would have taken the
text-mode branch even though the screen it inherits is graphics.

**This is not an approximation and is not registered in `APPROXIMATIONS.md`.**
The text-mode `vid_init(1)` initialisation has no counterpart in the port's
run shapes, so there is no source behaviour the port differs from: the branch
that *is* reachable — the captured DAC — is the source's own, and the dead one
is deliberately not implemented rather than approximated. The source selects it
only when the BIOS mode is 3 or below (`U2E.C:198-205`), and no port run shape
begins there.

**It changes no measured intro frame.** The fade-in mutates bytes 3..761
(`+2`, clamp 63) for 33 steps, so every byte it touches saturates to 63 from
either starting value; the untouched bytes are entry 0 and entries 254/255.
For the block-4 shape the shell's boot DAC has those black — its table is 248
entries and the last eight are unset — which is what the text-mode branch left
there too; for a mid-chain run the capture was already the source's branch, so
nothing changes. The fadeset's palette (entry 0 black, 252/253 white, 254
black) and the fade-out's (1..252 → 0, 253/254 → 63) are therefore identical
either way, and a block-4 run's intro still compares at 0 differing — the
dense/start/sweep2 intro; the older `sweep` session's hold differs — §7.

**The main palette.** `cp = scenem+16` is the `.00M`'s 768 bytes; entries 252,
255 are set black and 253, 254 to 63 (`U2E.C:337-350`) and `vid_setpal(cp)`
uploads all 256. The port clones the reader's master palette, patches the four
entries, and uploads it on the setup tick (part tick 100, boot 518).

## 5. The reference comparison

`reference/parts/u2e/` holds four RGB-only keyframes; no `.idx` is filed
because the part writes CRTC 0x0C every retrace (`reference-indices.md`
§18.3). A `--block 4` run from the repository root:

```bash
cargo run --release --bin secondreality -- --block 4 --headless \
    --dump-frames 996,997,999,1000,1001,1596,1597,1599,1600,1601,2596,2597,2599,2600,2601,3596,3597,3599,3600,3601 \
    --dump-dir out/u2e
```

The run's manifest says `part: "u2e"`, `index_origin: "boot"`, `anchor: 418`,
320x200, `MUSIC0` order 18 aligned to the run's **418** (`start_frame`; the
clock itself starts at 408, and the ten frames 408..=417 are pre-polled at the
run's start). The
port's published positions are `19:34` at 1000, `20:39` at 1600, `22:25` at
2600 and `24:11` at 3600; `second4` reads `19:34`, `20:38`, `22:25` and
`24:11` — 1600 sits on a row boundary within a frame, the D17 session class.

**The intro is exact and the keyframes are not.** The port's displayed stream
state at boot `t` is `(t - 698)/2`; the filed keyframes are `(t - 702)/2` —
the same slope to the frame, a **four-tick intercept** (two stream states).
The port's music clock reaches order 19 at 698 where `second4` reads it at
699, and the keyframe capture is another session: this is D17's session phase,
the same class **A29** registers for U2A's 7300. No anchor or timing is fitted
to it. **Ruled and registered as A31** in
`docs/fidelity/APPROXIMATIONS.md`; the port keeps the tree's clock and the
byte-exact intro.

At the exact keyframes the RGB comparisons differ by **11,006 / 11,805 /
17,278 / 16,234** pixels (17.1969 / 18.4453 / 26.9969 / 25.3656 %). At the
reference's **own** state — the port's
`K - 4` frame — the silhouettes are exact: **0** pixels disagree between
"background" and "object". The remaining colour residual is **189 / 7,118 /
231 / 1,177** pixels, and every differing pixel's topmost writer is a
Gouraud-shaded object: `s01` (124 faces; 123 Gouraud) at all four, `Tree01g`
(70 faces; 64 Gouraud) at 1600, and `Car02` (10 Gouraud faces) at 3600. The
reference-derived index deltas are small and mostly `+1`:

```
1000  s01         {-2:3, -1:7, 1:138, 2:13, 6:2, 7:4, 8:5, 9:4, 10:3, 11:7, 12:3}
1600  Tree01g/s01 {-4:2, -3:2, -2:41, -1:322, 1:6655, 2:78, 3:17, 4:1}
2600  s01         {-8:3, -7:1, -6:1, -5:1, -4:3, -3:10, -2:12, -1:16,
                   1:103, 2:30, 3:10, 4:8, 5:17, 6:12, 7:4}
3600  s01/Car02   {-7:1, -6:1, -4:8, -3:11, -2:65, -1:266,
                   1:616, 2:136, 3:14, 4:24, 5:4, 6:8, 7:11, 8:7, 9:5}
```

Every flat-shaded object reproduces exactly; the residual is confined to the
Gouraud faces, the release-versus-tree difference **A28** already registers
for U2A (`docs/fidelity/u2a.md` §5). **Ruled and registered as A30** in
`docs/fidelity/APPROXIMATIONS.md`: the four numbers are 189 / 7,118 / 231 /
1,177 of 64,000 (0.2953 / 11.1219 / 0.3609 / 1.8391 %), the index deltas run
from −8 to +12 and 6,655 of the 7,118 at 1600 are `+1`. The port keeps the
tree's translation; no constant was fitted to a keyframe (D24/D17). The
ignored unit test `search_reference_states` reproduces the states, the topmost
attribution and the deltas; the integration test asserts the numbers above.

**The block-3 seam is nine ticks early, and it is measured rather than fitted.**
The mid-run `MUSIC0@18` restart (`MAIN/U2.ASM:880-885`) is installed by the
shell at the run's U2E boundary, and the block-3 chain test
(`bins/secondreality/tests/shell.rs`) records where that boundary lands: the
port's U2E starts at chain **17,418** (JPLOGO's last frame is 17,417), where
D34 derives **17,427** from the block's mapping (chain tick 0 = boot 419,
boundary boot 17,846.2). The nine frames are the port's **gapless handover** —
D31 §3's per-seam difference, which this part's boundary is the last block-3
seam to accumulate — not a timing error in the restart: the new player is
seeked at the port's boundary and waits D34's four engine ticks
(`MUSIC_START_FRAME - FIRST_GRAPHICS_FRAME`), its ten-deep publish ring primes
at `0:0`, first publishes `18:1` at chain 17,440, and against `second3e` at
the mapping-derived indices (chain = boot − 419) the published positions are
within one MUSIC0 row (8.76 frames at speed 6, tempo 120): `18:17` against
`18:16` at 17,581, `18:19` against `18:18` at 17,600, and the order-19
boundary `19:1` against `19:0` at 17,721. Nothing was shifted to close the
offset; a later full-run comparison will need the seam's own offset, as D31
§3 says.

## 6. The bugs and hardware behaviours, preserved

- **The uninitialised `fov`.** `int fov;` is declared inside the loop
  (`U2E.C:378`) and read by `vid_cameraangle(fov)` before the first parse sets
  it. The first draw is skipped, so the first real draw uses the first parsed
  frame's fov; the stream's fov is 0x1c for all 1,801 frames. The port's
  driver starts at 0 and the skip makes the read invisible.
- **The first draw is skipped.** `firstframe`'s branch draws nothing and
  resets `syncframe`/`coppercnt` (`:433`); the port's `Driver::draw_frame`
  performs that reset without touching the page, and the first loop tick still
  schedules and queues page 0 (the fadeset page).
- **The deadlock escape.** `while(cl[clw].ready) { if(deadlock>16)
  break; }` draws into a page the display may still hold; the port's
  `Ring::next_page` returns the busy page once the copper has counted past 16.
- **The 768-write DAC rotation, one tick late.** The teardown's iteration is
  `fpal[a]+=4; dis_waitb(); outp(0x3c8,255); write 768 bytes` (`:545-554`), so
  the trigger tick only captures and computes step 1; step `n` is visible on
  tick `T+n`, and the part ends on the tick that shows step 16. The write
  starts at DAC entry 255 and wraps: entry 255 takes `fpal`'s entry 0 and
  entry `j < 255` takes `fpal`'s entry `j + 1`; `fpal`'s entry 255 lands in
  entry 254 and nothing is left unwritten. The unit test derives the rotation
  from the source, pins every entry, and asserts the first/last visible
  palette values.
- **The city hacks are dead.** `U2E.00M` byte 15 is 0, and U2E never reads
  `city` anyway; the driver's city branches are U2A's cadence only.
- **The name hacks run.** `name[1] == '_'` forces distance 1,000,000,000
  (`:408`), and `currframe > 1800 && currframe < 2200` forces `s01`'s distance
  to 1 (`:410-416`) — strict bounds, in `currframe`'s two-retrace units. The
  shipped scene has **nine** `_` objects (U2E.001, .003, .011, .014, .015,
  .016, .017, .020, .022) and `U2E.028` is `"s01"`;
  `the_scene_is_the_u2e_city_with_the_name_hacks_object` counts them.
- **The waits are ticks, not spins.** Every intro wait, every gate poll and
  every busy writer page is one `frame()` call; `ctx.exit_requested` is read
  in the gate and in the loop.
- **No music is a refusal.** The gate reads the published order; a run with no
  player is a `PartError` at `init`, following PAM and U2A.

## 7. Reproducing the numbers

```bash
# the part's unit tests, including the fadeset table, the measured 490/500,
# the hold, the gate, the first-draw skip, the cadence and the teardown
cargo test -p secondreality --bin secondreality u2e

# the state/attribution diagnostic: which stream state each keyframe is, the
# topmost object and the index deltas at that state
cargo test --release -p secondreality --bin secondreality search_reference_states \
    -- --ignored --nocapture

# the integration test: the manifest, the music and the measured keyframe
# residuals
cargo test -p secondreality --test u2e -- --nocapture

# the reference comparison by hand (from the repository root)
cargo run --release --bin secondreality -- --block 4 --headless \
    --dump-frames 996,1000,1596,1600,2596,2600,3596,3600 --dump-dir out/u2e
cargo xtask compare --reference reference/parts/u2e --rendered out/u2e \
    --out out/u2e-diff

# the intro sessions the port reproduces at 0 differing
cargo xtask reference capture --arg 4 --boot autoexec --keys "KBD_left KBD_enter" \
    --cycles 30000 --frames 456,457,458,459,460,461,462,463,464,465,466,467,468,469,470,471,472,473 \
    --out out/u2e-dense-a
cargo xtask reference capture --arg 4 --boot autoexec --keys "KBD_left KBD_enter" \
    --cycles 30000 --frames 420,430,440,450,460,470,480,490,500 --out out/u2e-start
# the hold session the 530/590/660 comparisons use (`reference-indices.md` §18.2):
cargo xtask reference capture --arg 4 --boot autoexec --keys "KBD_left KBD_enter" \
    --cycles 30000 \
    --frames 430,480,530,590,660,760,850,950,3100,3200,3400,3700,3900,3950,4050,4100,4200,4400,4600,4800 \
    --out out/b4b-u2e-sweep2
# §18.2 records this run as exit 2, refused at 4400 (320×400 there): the frames through
# 4200 were written, and 530/590/660 are the hold compared above.
```

**The older sweep session's hold is a reference anomaly.** `out/b4b-u2e-sweep`'s
600 and 700 are one 4-colour screen (`ef539f9e…`, mean 158.30) with the
fadeset page under a red/cyan palette (entry 252 = `(20,0,0)`, 253 =
`(0,63,63)`, 254 = `(63,63,63)`), while `out/b4b-u2e-sweep2`'s 530, 590 and
660 are the 2-colour grey hold (`befaf38a…`, mean 191.25) that
`reference-indices.md` §18.6 independently matches on a `SECOND 3` boot, and
the port matches sweep2 at 0 differing. The two sessions' 500 frames are
byte-identical (`a830146f…`), so they are at the same fade step at 500; the
sweep's 600 screen cannot be produced by either intro path or by the patched
main palette (every path reaches black/white at the hold). It is therefore a
**reference anomaly**, not a port approximation: the port follows the source
and the sessions that agree with it (and the filed keyframes' city scenes),
and the anomaly is **not registered in `APPROXIMATIONS.md`** for that reason.
The `+252` correction above is source-faithful and unrelated to it.
