# PAM — the Praxis explosion

`SECOND 1`'s third part: the 42-frame Praxis animation, flipped between two
VGA pages while a music-synced fade washes it in and out. About three seconds
— 180 frames of loop at four copper frames a step — and the last part of the
boot's opening region, whose first drawn frame the reference names at 7710.

The tree's sources are `PAM/OUTTAA.C` (67 lines, the part), `PAM/ASMYT.ASM`
(322 lines, `ulosta_frame` and the dead `ruudulle`), `PAM/COPPER.ASM` (87
lines, `frame_count` and the palette upload), and the assets `PAM/OUT.ANI`
(the packed 42-frame delta stream), `PAM/PAL.PAL` (the six-bit palette) and
`PAM/PRAX4.FLI` (the Autodesk source the `.ANI` was packed from). The port is
`bins/secondreality/src/parts/pam.rs`, registered as `"pam"`, with its
integration test in `bins/secondreality/tests/pam.rs`.

## 1. What the original does, stage by stage

- **`:34` `dis_partstart()`** — the loader's handover; the run's numbering
  begins at the reference's **7710** (see §4).
- **`:36-37` the palette wash**:
  `for(a=1;a<64;a++) for(b=0;b<768;b++) pal[a*768+b] = (63*a+(64-a)*pal[b])/64;`
  Copy 0 is `PAL.PAL` untouched; copies 1-63 are washes of *that base copy*,
  not iterative fades, because `pal[b]` is always copy 0's byte.
- **`:39` the music wait** — `while(dis_sync()<10 && !dis_exit());`. The
  block-1 clock (`_zplaysong(0)`, fitted at 400) reaches `ordersync1`'s code
  10 at order 13 row 1; the trace at boot 7710 holds exactly that.
- **The gate precedes the setup.** `:36-37` builds the palette copies before
  the wait, but the mode set, the near-white palette and
  `init_copper`/`frame_count=0` are *after* it (`:40-45`). The anchored run's
  gate is already open at 7710, so its setup is the two copper frames before
  the anchor (§4); a run whose gate is still closed at its start keeps the
  predecessor's screen, performs the setup when the gate opens, and takes its
  first step four ticks later. The unit test
  `the_sync_gate_holds_until_code_ten_then_sets_up_and_waits_four_ticks`
  drives that path.
- **`:40-45` the setup** — `tw_waitvr()`, `tw_setpalette(&pal[768*63])` (the
  near-white copy), `tw_opengraph()` (mode 13h with chain-4 off, which clears
  the 64 KB window through all four planes), `init_uframe(FP_SEG(memblock))`,
  `init_copper()`, `frame_count=0`.
- **`:46-64` the loop** — `while(!dis_exit() && f++<45)`; each body waits
  `while(frame_count<4); frame_count=0;`, draws one animation frame if
  `f<=40`, flips the display start, and queues `pal[768*wfade[f]]` for the
  copper's next retrace.
- **`:65` `close_copper()`** — the three DIS copper slots are unregistered.

`PAM/COPPER.ASM` registers `copper3` in slots 1 and 0 (a bare `retf`,
`:83-85`) and `copper2` in slot 2 (`:60-81`: increment `frame_count`, then
upload `cop_pal` when `do_pal` is set). The port maps the three engine slots
one for one: two no-ops and the slot-2 counter/upload. The upload lands in the
next tick's frame, which is the source's retrace upload seen one frame later.

## 2. The screen: 320×200, planar memory, two pages

`PAM/TWEAK.ASM:18-41` is ALKU's mode set without the CRTC offset write: mode
13h, sequencer 4 = `0x06` (chain-4 off, odd/even off), map mask `0x0F`, CRTC
0x14 = 0 and 0x17 = `0xe3`. The screen is 320×200 with an **80-byte plane
row** (`reference-indices.md` §1.6, §6); 0x13 is never written, which is why
the reference's 7900 frame keeps an `.idx` at all.

`ASMYT.ASM`'s `ulosta_frame` writes a 64,000-byte linear image one byte a
pixel: pixel `n` goes to plane `n & 3` at plane-byte `n >> 2`. In the
emulator's four-byte-group VGA memory that is linear byte `n`
(`vga_memory.cpp`'s unchained handler; `reference-indices.md` §1.6), and the
engine's surface is indexed the same way ALKU's is — pixel `4*plane_byte +
plane` — so:

```text
page 0     surface pixels 0 .. 63,999                `ulosta_frame(0x0a000)`, `tw_setstart(0)`
page 1     surface pixels 0x10000 .. 0x10000+63,999  `ulosta_frame(0x0a400)`, `tw_setstart(16384)`
```

`tw_setstart`'s 0x4000 is a plane-byte address (`TWEAK.ASM:188-206`,
`CRTC 0x0c/0x0d`), and `View::set_origin` takes pixels, so the flip is
`PAGE_ORIGIN = [0, 0x4000 * 4]` — the same ×4 ALKU's copper1 applies to its
own plane-byte start. The view's `y_line_offset` is `Some(320)`: the pages are
flat 320-pixel rows inside the 640-wide engine surface.

**The delta base is two frames back.** `OUTTAA.C:51-60` alternates the two
pages, so step `n` unpacks into the page step `n-2` last wrote, and frames 0
and 1 come off the cleared screen. The packer matches it: `ANIM.C:28-30`
copies `mpage1` into VRAM *before* `mpage2` moves into `mpage1`. The port's
decoder (`sr_assets::load_frames`, `Format::Ani`) applies that same n−2 base;
its two-reader cross-check — all 42 `.ANI` frames against the FLI's, pixel for
pixel — is `crates/sr-assets/tests/anim.rs`. The port then writes the decoded
frame into the page that is off screen at that step, which is the same bytes
`ulosta_frame`'s in-place delta leaves there.

## 3. The loop: 45 steps, four ticks a step, 40 drawn frames

One tick is one copper frame: `copper2` increments `frame_count` once a
retrace, so `while(frame_count<4);` is a four-tick wait, not a spin. The port
keeps the counter (initial `FRAME_COUNT_AT_ANCHOR = 2`, §4), increments it in
slot 2 once a tick, and checks it at the top of each `frame()` call. The
`exit_requested` flag is read before that check on every tick, so an exit
during any waiting tick ends the part on that tick and no loop ever spins.

Step `f` (`f = 1..=45`) therefore lands at **tick `2 + 4(f-1)`**, i.e. boot
`7712 + 4(f-1)`:

```text
f 1..40   draw animation frame f-1 (0-based) into page (f-1)&1, flip the view,
          queue palette copy wfade[f] for the retrace (visible next tick)
f 41..45  draw nothing, queue wfade[f] = 63
```

Only **40 of the container's 42 frames** are ever drawn (`OUTTAA.C:49`'s
`if(f<=40)`), and the last drawn frame is 0-based 39. `wfade` is declared
`[100]` but has **59 initialisers**, and only entries 1-45 are indexed; the
port keeps the 59 and the used range, with `wfade[0] = 63 = wfade[45]` as the
closing wash. The first step's copy is `wfade[1] = 32` — the half wash, not
63; the opening white is the pre-loop `tw_setpalette(&pal[768*63])`.

## 4. The anchor, and the cadence the support frames pin

**The anchor is 7710** — the flat `(251,251,251)` wash that is the first
reference frame with PAM's content (`reference-indices.md` §6.1): 7700 is
still U2A's landscape, 7710 is the wash, and §6.4 records 7710 byte-identical
in three sessions, with 7709 and 7711 the same bytes in the recapture's
leftovers. `shell::PAM_ANCHOR` is this number; `boot_anchor("pam")` returns
it.

The tick mapping is not an assumption. The reference's own support frames
(`reference-indices.md` §6.3) force it, given the source's four-tick wait:

- 7759 and 7760 differ, and 7761 holds 7760. With a four-tick cadence the
  step boundary must fall exactly on **7760**: a boundary at 7759 would put
  7759 and 7760 in the same step — same animation frame, and the palette is
  copy 0 across that whole stretch — and they would be identical; a boundary
  at 7761 would make both 7759 and 7760 the previous step. So step 13
  (`f = 13`) starts at 7760.
- `T_f = T_1 + 4(f-1)` gives `T_1 = 7760 - 48 = 7712`. The anchor is two
  ticks earlier, so the source's counter reads **2** at 7710 — two copper
  frames (the setup's and the first wait's) have elapsed since
  `frame_count=0` — and the run's two wait ticks (7710, 7711) complete the
  four before step 1 at 7712. `FRAME_COUNT_AT_ANCHOR` is that 2.
- The same mapping checks against the rest of the record: `T_30 = 7828`, so
  7829-7831 are one held frame (the §6.3 triple); `T_40 = 7868`, so 7870 is
  frame 39 under `wfade[40] = 56`, which is the scan's `644acaf3…` sample
  (§6.1); `T_41 = 7872`, so the closing `wfade[41] = 63` is up from 7873, and
  7880-7920 are the white hold the scan measured (§6.1).

**The 2 is calibrated for the anchored entry point.** `init`'s
`open_at_start` branch treats every gate-already-open start as this anchored
7710 start and sets the counter to 2. That is right for `--part pam`; a block-1
chain reaches PAM after U2A with the player already running, which is the
source's ordinary case instead: the gate is open at the part's own start
because the chain is continuous, yet the source still runs its post-gate setup
(`OUTTAA.C:40-45`) on that tick, which zeroes
`frame_count`, and its first step is four copper frames later. The port would
set 2 and step one animation frame two ticks early. Telling the two cases
apart is what the Phase 5 seam does: `PartCtx::music_fresh` (D31) is true only
for the anchored entry run, and the already-open gate sets the counter to 2
only then, taking the source's `0` otherwise
(`a_mid_chain_start_with_the_gate_open_starts_the_counter_at_zero`). **Block 1
now reaches PAM this way** — it runs ALKU → U2A → PAM → BEGLOGO and stops at
GLENZ — and `tests/shell.rs`'s
`a_block_one_run_is_alku_then_u2a_then_pam_in_the_boots_numbering` asserts the
hand-over and PAM's measured first drawn frame at boot 7,710; the pixel
comparison remains the standalone keyframes in `tests/pam.rs`.

What the tick mapping is **not** is a claim that every intermediate frame is
four ticks apart in the original: §6.1 measures the animation's *window* as
(7705, 7920] — 215 frames, which includes the pre-loop setup and the closing
white hold — while the step loop itself is 45 four-frame waits plus the time
each `ulosta_frame` takes. The port's clock is the fixed four-tick one the
task specifies; the evidence above is the filed keyframes and the support
frames, and they all match. One unfiled scan sample does not: the scan's 7720
hash `10bb3b95…` is found at the port's 7721-7723 (`out/pam-timeline`, §8), a
one-frame phase difference in a sample that is neither a keyframe nor from
the reference's session. The scan's 7870 `644acaf3…` matches the port at
7870.

## 5. The music: the block-1 pre-roll, measured

PAM is block 1's third part (`MAIN/U2.ASM:793-799`), before any block restart,
so its recipe is the initial `_zplaysong(0)` whose clock starts at
`shell::BLOCK_ONE_MUSIC_START_FRAME = 400`. A `--part pam` run draws from
7710, so `MusicStart::for_part("pam", Some(7710))` is

```text
frame 7710, waiting 0, pre 7310
```

and the shell pre-polls the player 7,310 times before the first tick; the
engine's own tick then polls once more before the part reads `dis_sync()`.

**Measured, and it needs no calibration.** The unit test
`the_block_one_pre_roll_puts_the_player_at_the_traces_7710_position` polls the
real MUSIC0 (the tempo-patched file) and asserts these published readings:

| polls | order | row | zframe |
|---|---|---|---|
| 7,304 | 13 | 0 | 5004 |
| 7,305 | 13 | 1 | 5004 |
| 7,310 | 13 | 1 | 5008 |
| 7,311 (the engine's own tick) | 13 | 1 | 5009 |

and `dis_sync()` is 10 at `(13, 1)`. The trace says the same row, and the row
change is on the same frame
(`awk -F, '$1==7705 || $1==7710' reference/music/second1b/trace.csv`):

```text
7705,13,1,13,0,6,120,0,0,5005
7710,13,1,13,0,6,120,0,0,5008
```

`(13, 1)` is `sync_from`'s code **10**, so the gate `dis_sync()<10` opens on
the run's first tick. The `zframe` column is the player's tick counter, and
its sampling boundary differs: the trace samples at the frame (5005 at 7705,
5008 at 7710) while the port reads after a tick's poll (5004 after 7,305
polls, 5008 after 7,310), so the two may sit one tick either side. The
published *row* — what the gate reads — agrees at both.

The 7,310-poll catch-up is longer than the player's 8,192-sample ring, so the
ring drops its oldest samples as it fills — the `install_music` comment's
long-catch-up case, now real. Nothing about the position depends on the ring;
the headless dump asks for no audio device. The catch-up itself is the
`--part pam` entry's: a block-1 chain's player has been continuous since ALKU
and applies no per-part catch-up, so a chain changes the counter question §4
records, not this one.

## 6. The reference comparison, and 7900's index plane

The four keyframes are the reference's own files
(`reference/parts/pam/frame_0077{10,60}.rgb`, `..._007830.rgb`,
`..._007900.{rgb,idx}`). Command:

```text
cargo test -p secondreality --test pam -- --nocapture
```

which runs the real binary headlessly from the repository root over the D19
support frames. Output:

```text
boot 7710: 0/64000 px differ (0.0000 %)
boot 7760: 0/64000 px differ (0.0000 %)
boot 7830: 0/64000 px differ (0.0000 %)
boot 7900: 0/64000 px differ (0.0000 %)
```

Every one is byte-identical in **RGB**. The relations the record measured
hold: 7711 = 7710 (the record's leftover; 7709 is unreachable); 7759 ≠ 7760
and 7761 = 7760; 7829 = 7830 = 7831. The record's support set for the held
7900 is the scan's 7890 and 7910; the port's dump carries its own extra hold
check at 7899 and 7901, and those are the same white. The hashes
(`sha256sum out/pam-port/pam_00*.rgb`) match §6.3's recorded values entry for
entry: 7759 `23fa5096…`, 7761 `5d558081…`, 7829/7831 `e1967ed2…`; the
keyframes are `1fef46de…`, `5d558081…`, `e1967ed2…`, `1fef46de…`, and the
port's own 7899/7901 are `1fef46de…` as well.

Because the anchor makes 7709 unreachable, the dump carries 7710's `K+1`
only; 7759 and 7761 cover 7760, and the held keyframes carry their triples.
The D19 asymmetry the record describes is unchanged.

**7900 is the one frame the reference files with `.idx`, and that `.idx` is
not PAM's screen.** The reference's plane is 64,000 bytes of index **15**, and
its manifest's palette entry 15 is `[62,62,62]` — PAM's 63rd copy, widened to
251. Index 15 is BEGLOGO's `WHITE_FIELD` (`BEG/BEG.C:18-21` fills A-space with
15 before its own DAC writes), and the reference's 7900 is inside the window
where the indexed read runs late: D15's read costs about twenty rendered
frames, and BEGLOGO's white field is up while PAM's 63rd copy is still the
DAC — the manifest's `[62,62,62]` is PAM's wash, not BEGLOGO's own white —
before BEGLOGO's 320×400 mode change in (7920, 7930] (§6.5). The coherence
check
cannot see the difference on a collapsed palette (`reference-indices.md` §1.6,
the blind spot that lets a wrong plane pass when every index widens to one
colour). So the reference's 7900 RGB is byte-identical to the port's — both
are the flat wash — while the port's index plane is the last animation frame
PAM drew, 0-based frame 39.

This is registered, not adopted (D24's procedure, step 2: the tree cannot
produce the reference's plane at 7900 because the plane is not this part's
screen; the entry is **A21** in `APPROXIMATIONS.md`). The measurement, from `the_7900_index_plane_is_the_next_parts_white_field_not_pams_frame`:

```text
7900 index plane: 60375/64000 px differ, max delta 194
```

and `cargo xtask compare` reports it in the same terms (`94.3359 %`, max delta
194 — the frame's highest index 209 less 15) while the three RGB frames report
`0.0000 %`. The integration test also asserts that the port's plane equals
`OUT.ANI`'s frame 39, so the divergence is pinned to the source's own drawing
rather than to a decode difference.

## 7. The bugs, preserved

- **The FLI header counts 41 and the container holds 42.** `VFLI.C:66` reads
  the word and nothing reads it again; the record walk finds 42
  (`asset-inventory.md` §E.5; `crates/sr-assets/tests/anim.rs` pins both). The
  loop draws **40** of them, so none of the three counts is the drawn count.
  The unit test `the_fli_header_counts_forty_one_and_the_container_forty_two`
  reads `PRAX4.FLI`'s word (41) and the `.ANI`'s frames (42) and pins 40.
- **`ruudulle` is dead.** `ASMYT.ASM:207-320` is a second copy of the
  unpacker, never called; the port has only `ulosta_frame`'s reader.
- **The `while(frame_count<4);` spin** is a tick-consuming wait; the exit
  flag is read on every waiting tick (`exit_requested_while_waiting_ends_the_part_on_that_tick`).
- **The 45-step loop outlives its drawing**: steps 41-45 draw nothing and only
  re-queue copy 63, which is the closing white hold.

## 8. The assets, the decoder, and how to reproduce the numbers

`OUT.ANI` and `PAL.PAL` are read in their original formats. The decoder's
malformed-input behaviour and two format choices are **A3** and **A4** in
`APPROXIMATIONS.md`; nothing here re-derives them. The `.ANI` itself has no
malformed-input path in the tree's files — its only refusal, a run past
64,000 pixels, is impossible from `ANIM.C`'s encoder (`cnt+a<64000`) — and A3
says so. The port reads `PAL.PAL` through `sr_assets::pal::decode` (six-bit
verbatim, the rule `tests/anim.rs` measures against both wrong conversions)
and `OUT.ANI` through `load_frames(Format::Ani)`.

The numbers above come from these commands:

```text
# the unit tests: the gate, cadence, wfade, flip, exit, the pre-roll, the
# keyframes composed from the engine's own pieces
cargo test -p secondreality --bin secondreality parts::pam -- --nocapture

# the integration test: the manifest, the four keyframes, the support
# frames, and the 7900 index plane
cargo test -p secondreality --test pam -- --nocapture

# the reference comparison of the port's own dump (the RGB frames and the
# mixed-mode 7900 result)
cargo run --release --bin secondreality -- --part pam --headless \
    --dump-frames 7710,7711,7759,7760,7761,7829,7830,7831,7899,7900,7901 \
    --dump-dir out/pam-port
sha256sum out/pam-port/pam_00*.rgb
cargo xtask compare --reference reference/parts/pam --rendered out/pam-port \
    --out out/pam-port-diff

# the recorded scan samples: the port's timeline around the anchor
cargo run --release --bin secondreality -- --part pam --headless \
    --dump-frames "$(seq -s, 7710 7930)" --dump-dir out/pam-timeline
sha256sum out/pam-timeline/pam_007720.rgb out/pam-timeline/pam_007721.rgb \
          out/pam-timeline/pam_007870.rgb
```

**Register.** The behaviours of this part that the original does and the port must keep are in `bugs.md` §11 (the 40-frame loop), with the dead `ruudulle` in the index.
