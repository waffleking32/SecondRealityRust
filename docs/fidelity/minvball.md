# MINVBALL — the mini vector balls

`DOTS/MAIN.C` (291 lines) over `DOTS/ASM.ASM` (250 lines, the `drawdots`
projector) and `DOTS/SIN1024.INC` (the sine table both halves use). 512 dots
projected, depth-coloured and bounced over a grey floor, with four position
phases and a palette fade-out; music-cued at the end. `DOTS/MAKEFILE:20`
builds `dots.exe` and stages it as `minvball.exe`.

The port is `bins/secondreality/src/parts/minvball.rs`, registered as
`"minvball"`. A `--part minvball` run seeks `MUSIC1` to order 75
(`shell::music_for_part`), the order the block-3 trace holds when the part's
start window opens.

## 1. What the original does

| source | act |
|---|---|
| `MAIN.C:79-124` | `dis_partstart`; `dotnum=512`; `dottaul` shuffled by 500 random swaps; every dot set to `(0, 2560-22000, 0)`; 500 more random swaps of x/y/z; `dropper=22000`, `grav=3`, `gravd=13`, `gravitybottom=8105` |
| `:125-150` | mode 13h; the 16×4 colour ramp, the 100 greys at entries 64-163 and white's neighbours at 255; the DAC read back into `pal`, then zeroed |
| `:152-155` | rows 100-199 of video memory filled with `a+64`; `bgpic = vram` |
| `:156-167` | `depthtable1/2/3[a] = base + 0x04040404*c`, `c = 15 - clamp((a-31)*3/4 + 8, 0, 15)` |
| `:171-183` | the fade-in: 65 steps, two `dis_waitb()` each, uploading `pal - b` clamped at 0 |
| `:185-284` | the main loop: `while(!dis_exit() && frame<2450)`; `repeat = dis_waitb()`; `setpalette(pal2)` once `frame>2300`; the demo cue `-4 < dis_musplus() < 0`; the stage physics `repeat` times; `drawdots()` |
| `MAIN.C:202-269` | the four position phases (`frame<500`, `<900`, `<1700`, `<2360`) and the two palette phases (`<2400` blue ramp, `<2440` every entry to black) |
| `ASM.ASM:52-218` | `drawdots` |

## 2. The RNG is Microsoft C's, not Turbo C's

The brief names Turbo C's `rand()`. The source says otherwise, and the frames
agree with the source:

- `DOTS/MAKEFILE:10-16` compiles with **`cl /qc /AL`** — Microsoft C 6.0's
  large model — assembles with `tasm` and links with Microsoft's `link /E`.
- The linked `DOTS.EXE` carries the **“MS Run-Time Library - Copyright (c)
  1990, Microsoft Corp”** string (`strings -a DOTS/DOTS.EXE`).
- Microsoft C 6.0's generator is a 32-bit LCG with **modulus 2³²**,
  multiplier **214,013**, increment **2,531,011**, seed **1**, returning
  `(seed >> 16) & 0x7FFF`. Its first ten values are **41, 18,467, 6,334,
  26,500, 19,169, 15,724, 11,478, 29,358, 26,962, 24,464**
  (`the_rng_is_microsoft_cs_lcg_and_its_first_values_are_the_sequences`).
- The reference's dense 12,400 scatter frame is reproduced to **25 pixels**
  with this generator (`the_four_keyframes_match_the_reference_at_the_mapping`);
  the Borland generator (seed 1, multiplier `0x015A4E35`) is a different
  stream and does not place the scatter's dots where the released frame has
  them. The frames decide it (D24).

The source never seeds: the seed is the library's initial 1. Every use is
integer — `rand()%512` twice in each of the two setup shuffles, then
`rand()-16384`, `8000-rand()/2`, `rand()-16384` in the scatter. The setup's
exact values are pinned by
`the_setup_is_deterministic_and_its_permutation_is_fixed` (the first twelve
`dottaul` entries are 223, 464, 331, 287, 94, 64, 6, 173, 17, 208, 10, 216;
the 500 swaps are a permutation of 0-511). The second shuffle is a no-op on
the values — every dot starts identical — but its 1,000 calls are what put the
scatter's stream where the reference has it, so the port runs it.

## 3. The tick map, the mapping, and `repeat`

One tick is one `dis_waitb()`. The port's engine gives one tick per `frame()`
call, so the C's `frame` counter advances once a tick.

```text
tick 0            the setup is up; the fade's first wait
ticks 1-129 odd   the 65 fade uploads, `pal - b` for b = 64 down to 0
tick 130          the main loop's first iteration (frame counter 1)
```

**The mapping is `boot = 10,368 + tick`, and it is a mapping, not an
anchor.** `shell::boot_anchor("minvball")` stays `None`: the part's first
frame is the measured window `(10,300, 10,400]`
(`reference-indices.md` §14.5), a run of this part numbers its own frames from
zero, and the manifest says `index_origin: "part"`, `anchor: null`
(`the_run_is_numbered_in_its_own_frames_and_draws_320x200`). The mapping's
evidence is the fit's own:

- tick 0 is the fade's first wait at boot **10,368**; the main loop starts at
  tick **130**, frame counter 1 at boot **10,498**
  (`the_mapping_is_boot_equals_ten_thousand_three_hundred_sixty_eight_plus_tick`);
- the reference's four keyframes are reproduced at frame counters
  **403, 703, 1303, 1903**, i.e. ticks **532, 832, 1432, 2032**
  (`the_four_keyframes_match_the_reference_at_the_mapping`) — the fit's own
  evidence, not an independent check.

**The independent pin is the survey's 12,900 flat-grey frame**: the sessions
record `(239,239,239)` and `(231,231,231)` — the c=59 and c=57 uploads
(`reference-indices.md` §14.5). Under this mapping boot 12,900 is frame
counter 2403, whose upload is pal2(2402), the c=59 step; the sibling
session's c=57 is the same curve one frame on (a start one frame earlier).
A different capture and a different observable, it pins the mapping to ±1.

**`repeat` is one, and the difference is zero.** The original advances the
physics `repeat` times per iteration; the port advances it once per tick. The
reference's own frames measure the difference: the four keyframes' frame
counters differ by **300, 600 and 600**, exactly the boot-index differences
between 10,900/11,200/11,800/12,400, and the y state — one `drawdots`
integration a frame, not one an inner repeat — matches at every keyframe
(`the_repeat_difference_is_zero_at_the_keyframes`; the integration test's
residues are the frame-counter match). A `repeat` above one between two
keyframes would advance the counter without a draw and move the ball rows.
The reference's frame cap is reached at frame 2450, boot **12,947**; the
player publishes -3 from boot **10,368** (order 75 row 61, during the fade),
and the first -3 the main loop's own check would see is boot **12,950**, so
the cap ends the loop first in both the reference and the port. The `repeat`
measurement covers 10,900-12,400; the 12,947 end is the mapping's prediction
from the source's 2,450-frame cap, not a measured frame.

## 4. The drawing, and the bugs kept

For each dot, in array order:

- `z' = Z*cos - X*sin` gives `bp = z' + 9000`; the screen x is
  `((x'>>8)+(x'>>11))/bp + 160`, the shadow row `(8<<16)/bp + 100`, and the
  ball row `(y<<6)/bp + 100`.
- The shadow is a two-pixel word of colour **87** at `[bx]`; the ball is a
  two-pixel word at `bx+1`, a four-pixel dword at `bx+320` and a two-pixel
  word at `bx+641`, from `depthtable1/2/3` at `(bp>>6)&~3`.
- Before drawing, the dot's old shadow and old three ball rows are restored
  from `bgpic`.

Three features of the assembly are translated literally:

- **`mov ax,ax`** (`ASM.ASM:65`) is a no-op where the rotation's low word
  should have been saved (`mov bx,ax`). The z rotation therefore subtracts
  the *previous dot's screen offset* from `Z*cos`; only its high word reaches
  `bp`, so the carried register changes `bp` by at most the borrow. The port
  carries `bx` across the 512 dots.
- **`mov ds:[si+6],ax`** (`:194-196`) stores a clobbered register as the next
  frame's shadow offset when the shadow is off screen. The three 32-bit
  restores before it (`:186`, `:189`, `:192`) each load `eax` from
  `bgpic[old2+640..644]`, so `ax` at `:195` is that dword's **low word** —
  the first two background pixels of the old ball's third row — not the
  shadow's row. (For the defined floor bytes the word is 0 or a pair of
  indices 64-163, i.e. 0x4040-0xA3A3; past 64,000 it is the source's undefined
  read.) The next frame restores from that word as a pixel offset. The
  port keeps the 64 KB video segment and a matching `bgpic` for the reads and
  writes this produces
  (`the_shadow_restore_bug_stores_the_old_balls_third_row_word`); the largest
  byte any write touches is **64,642**, and only the first 64,000 are
  displayed. `bgpic`'s bytes past 64,000 are the source's undefined read and
  the port's zero fill (D24).
- **The gravity integration is inside the ball block and skipped when the
  shadow is off screen.** `ja @@2` (`:96`, `:105`) jumps past `yadd +=
  gravity`, so an off-screen dot's `y` holds still until its shadow returns.
  `integrate` is called only on the shadow's on-screen branch.

The shadow's colour 87 sits inside the background's grey ramp (64-163), so a
shadow pixel is a slightly darker grey than the floor it covers; the port
writes it as it stands (`the_palette_is_the_sources_and_the_dac_starts_black`).
The ball's rows and the depth entry are pinned by
`the_ball_rows_are_the_three_depth_table_writes`; the depth ramp by
`the_depth_tables_are_the_sources_ramp`.

The initial `j` is never assigned (`MAIN.C:87` declares it beside the
initialized locals; `:200` reads it). The port starts it at **88**
(`INITIAL_J`), fitted to the reference: the drawing is identical for every
dot's position, and `j` only decides which dot draws a coincident position,
so the reference's overlaps are the only evidence the value has. The fit is
committed and reproducible:

```bash
cargo test --release -p secondreality --bin secondreality \
    parts::minvball::tests::the_initial_j_is_fitted -- --ignored --nocapture
# best five (pixels, j): [(67, 88), (341, 171), (353, 168), (354, 14), (356, 386)]
```

88 is the minimum of all 512 (`the_initial_j_is_fitted_to_the_reference`). The
residue §6 measures is what remains under that fit.

## 5. The cue, `dis_indemo`, and the music

`MAIN.C:191-195` breaks the loop when `dis_musplus()` is in the open interval
`(-4, 0)`; the port tests `reading > -4 && reading < 0` and retires on that
tick, before the physics
(`the_cue_window_is_the_open_interval_minus_four_to_zero`). A skipped cue — a
script that walks past the window without ever landing in it — leaves the
part continuing and drawing (`a_skipped_cue_leaves_the_part_continuing_and_drawing`).

**There is no `dis_indemo` in the port.** The run is always "in demo", so the
cue applies to standalone runs too; the part's `standalone` flag decides
nothing here. The source's non-demo branch (`:285-289`, the mode set back to
3h) has no picture and no port equivalent.

The part reads the cue as the first act of its main-loop tick, so it reads the
words the engine's own poll published for that tick (D28: the original's check
sits at the frame boundary and reads the previous frame's). **The port
declines D28's held-reading pattern here**, and records the decision: the only
reading the difference could change is one in the window, and the reference's
first main-loop -3 is boot 12,950, past the frame cap at 12,947 (below); the
earlier -3s (boot 10,368-10,379) fall inside the fade, which does not check
the cue. Holding the previous reading would move the check one tick later and
change no picture in this part's run. The decision is a choice about this
part, not a ruling on other parts' cues.

**The music's seek and pre-roll.** The shell seeks `MUSIC1` to order **75**,
the order `reference/music/second3b/trace.csv` holds at the part's start
window (frames 10,300-10,368). The reference's published position at boot
10,368 — the part's tick 0 — is **75:61** (`zplus` 1, `musplus` -3). A player
seeked to order 75 reaches that on its **257th** poll, so `init` pre-polls
**256** times and the engine's tick-0 poll is the 257th
(`the_music_pre_roll_puts_the_reference_state_at_tick_zero`). The fade's 130
ticks then carry the player into order 77 (the trace's position at boot
10,498), so order 75's cue rows fall inside the fade, where the source does
not check the cue. With no player (a unit-test rig) the pre-roll is skipped;
the part does not need music to run.

**The 256 is calibrated for an entry-point run.** It stands for a run that
starts the player with the part — `--part minvball`, whose shell seeks
`MUSIC1` to order 75 — and not for a part reached mid-chain with the player
already running. The first runnable block-3 chain (Phase 5's LNS&ZOOM/PLZPART)
reaches this part with the player continuous since MNTSCRL; the player is
then already at the reference's position, and `init`'s 256 pre-polls would
advance it 256 polls past it. A mid-chain start therefore requires the entry
pre-roll to be skipped, through the seam `PROGRESS.md`'s Phase 5 hand-off
schedules (a `PartCtx` signal that the player was already running, or moving
the pre-roll into the shell's per-part entry path). No chain can run until the
intervening parts land.

## 6. The keyframes, and the measured residue

The reference files four RGB-only frames (`reference/parts/minvball/`: the
dots move every frame, so no frame carries `.idx`; the manifest's hashes are
the four `sha256` values). All four are moving screens, and the integration
test holds `K-1, K, K+1` for each (D19), so the mapping is exact and not a
search's nearest fit.

| reference | port tick | frame counter | differing pixels of 64,000 | percentage |
|---|---|---|---|---|
| 10,900 | 532 | 403 | **67** | 0.1047 % |
| 11,200 | 832 | 703 | **63** | 0.0984 % |
| 11,800 | 1,432 | 1,303 | **37** | 0.0578 % |
| 12,400 | 2,032 | 1,903 | **25** | 0.0391 % |

The counts are asserted exactly by
`the_four_keyframes_match_the_reference_at_the_mapping`, so a change that
moves them fails the test.

**The geometry is exact.** The shadow — colour 87, a grey whose value is 11 in
the six-bit palette and 45 in the widened RGB — is at the same pixels in the
port's and the reference's frames for at least **99 %** of the reference's
shadow pixels at every keyframe (the same test). The projection, the rotation
and the scatter's random positions are therefore right; the residue is at
coincident dots, where two balls or a ball and a shadow land on one pixel and
the last writer wins.

**The D24 record.** The vendor tree and the shipped release are different
artifacts:

```bash
ls -l upstream/SecondReality/MAIN/DATA/MINVBALL.EXE upstream/SecondReality/DOTS/DOTS.EXE
# 7395 ... MINVBALL.EXE   (PKLITE self-extractor; the header carries "PKLITE Copr.")
# 10372 ... DOTS.EXE      (the development build of the source ported here)
```

The release's part is the packed one, while the tree's `DOTS/DOTS.EXE` is the
development build of the source ported here (A19's lead applies). The
`dottaul` permutation is the RNG's and the port reproduces it — the 25-pixel
scatter at 12,400 is the evidence — so the only unreadable input is the
release's **initial stack `j`**, which is inside the packed image. The source
leaves `j` uninitialized, so the port fits the one value the source permits to
the reference (88, §4). The residual above is that unobservable overlap resolution, and it
is recorded in `docs/fidelity/APPROXIMATIONS.md` A19. Everything the source
*does* say — the projection, the depth tables, the two bugs, the y-integration
skip, the Microsoft RNG, the cue and the frame cap — is implemented, and the
scatter's 25-pixel residue at 12,400 is the evidence that the random stream is
the reference's.

## 7. The real run

```bash
cargo test -p secondreality --bin secondreality parts::minvball               # 17 unit tests (+1 ignored measurement)
cargo test -p secondreality --test minvball                                    # 4 integration tests
```

The integration test drives `CARGO_BIN_EXE_secondreality --part minvball
--headless` from the repository root and asserts the manifest
(`index_origin: "part"`, `anchor: null`, `MUSIC1` at order 75), the 320×200
geometry, the black DAC and the floor ramp at tick 0, the four keyframes at
the mapping with their exact residues, the 99 % shadow agreement, the frame
cap at tick 2,579 (boot 12,947) and that the retired part holds its last
frame. The unit tests pin the RNG, the setup, the cue window and the skipped
cue, the exit, the frame cap, the stage boundary, the gravity decrement, the
depth tables, the palette, the fade uploads, the shadow bug, the ball rows,
the pre-roll and the mapping.
