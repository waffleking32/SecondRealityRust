# U2A — the landing pad and the saucer

**Sources.** `VISU/C/U2A.C` (515 lines) driving the VISU engine (`VISU/VISU.C`, `ACALC.ASM`,
`ADRAW.ASM` + `ADRAWCLP.ASM`, `AVIDFILL.ASM`, `AVID.ASM` + `AVIDM1.ASM`); the packed scene
`assets/VISU/C/SCENE/U2A.00M`, `.0AA`, `.0AB`, `.001`-`.003` and the clear image
`assets/VISU/C/U2ABG.UH`. **Port.** `bins/secondreality/src/parts/u2a.rs` (`"u2a"`) over the shared
engine in `parts/visu/{math,draw,mod}.rs`; the module docs hold the mechanism (the acts, the
pre-gate hold, the page ring). Integration test `bins/secondreality/tests/blocks.rs`. **Run.** Block
1's second part, no anchor of its own: its frames are in the block-1 run's boot numbering, and
U2A starts at **5956** (`FULLRUN_SEAMS`; the original's first 320×200 frame is 5961). **Music.**
Block 1's continuous `MUSIC0`; the gates read the published order and row.

## 1. Timeline

| boot | source | what is on screen |
|---|---|---|
| 5956-6985 | — | the pre-gate hold: `U2ABG.UH` under the merged palette (§4) |
| 6986 (port) / 6988 (`second1c`) | `U2A.C:229-258` | gate 1 (`order > 10 && row > 46`); setup and first draw on the same tick |
| first three loop ticks | `:259-336` | pages 0, 1, 2 drawn before the 16-retrace delay ends; the deadlock escape; then 8, 8, 4, 1 frames parsed on four draws |
| ~6988-7508 | `:259-431` | the saucer: 521 drawable stream frames, one a tick in the steady state |
| ~7508 | `:351-356`, `:432-441` | the stream's END: `resetscene(); xit=1`, all four pages cleared |
| 7508-7709 | — | PAM's gate is still closed; PAM holds U2A's cleared page |
| 7618 (`second1b`) | `:276` | gate 2 (order 12 row 55): dead, the END comes first |

The gate rows come from the filed traces (§7). The port's END lands 521 part-ticks after its
gate-open tick (`teardown_clears_all_four_pages_and_leaves_page_three_displayed`), and PAM starts at
chain 7,508 (`a_block_one_run_is_alku_then_u2a_then_pam_in_the_boots_numbering`); the reference's
dense scan shows the plain landscape from 7560 at the latest to 7700 (`reference-indices.md` §5.5,
§6.1).

## 2. Mapping and music

The block-1 music clock is the shell's (`part-sequence.md`
§10.2). The port's gate opens at boot **6986**, two frames before `second1c`'s row change:

```bash
cargo run --release --bin secondreality -- --block 1 --headless \
    --dump-frames 6985,6986 --dump-dir out/u2a-gate
# manifest music_position: 6985 "11:46", 6986 "11:47"
```

The trace is a different session (D17 measures sessions 1-4 frames apart). The port's clock is
`second1`'s: at boot 5900 it reads **9:51**, `second1`'s own value (zframe 3771), while `second1c`
reads **9:50** (zframe 3768) at the same index. `the_block_one_clock_matches_second1_at_boot_5900`
asserts both positions; the zframes are the rows of `reference/music/second1{,c}/trace.csv`.
At the keyframe the port and the trace agree: `second1c` at 7300 is `12:18`, and the integration
test asserts the run's 7300 frame says `12:18`. Changing the block-1 clock to match `second1c`
would move ALKU's verified frames (A29).

## 3. Decisions

- **The pre-gate hold** (A32, D24's first kind of difference). `U2A.C` draws nothing before the
  gate, but the release shows the background. The port paints it under the merged palette in
  `init`, which is what the original shows at four of five capture speeds (§4).
- **The Gouraud residual is registered, not fitted** (A28, §5).
- **The 7300 pose is the session's phase** (A29, §5).
- **Pages packed at `page * 64_000`.** The original's pages are 65,536 pixels apart (a plane byte
  is four linear pixels, `pam.md` §2); the port packs them into its 640×400 surface at 64,000.
  Every page is cleared, drawn and read through the same origin, so the packing is unobservable;
  the reference's 6500 and 7640 frames and its 7300 `.idx` are page 0, whose origin the two layouts
  share.
- **The mode is the plain 320×200 tweak.** `U2A.C:242`'s `vid_init(3)` is commented "oversample x
  4", but `m1o_init` (`AVIDM1.ASM:71-99`) sets `_projoversampleshr` to 0 as the other modes do.
- **The background is not de-interleaved.** `U2A.C:153-166` reorders `bg` in place; the reference's
  7640 index plane is the packed payload byte for byte, and `sr_assets` returns that payload as
  linear 320×200, so the port copies `Image::pixels()`.
- **The teardown's display origin** is set to page 3, which is unobservable: every page holds the
  same cleared background (§6).

## 4. The palette merge, and the pre-gate hold

`U2A.C:152`, `memcpy(scene0+16+192*3,bg+16,64*3)`: the scene's entries 0-191 stay, and the
background's 0-63 land at 192-255. The background picture indexes 0 and 192-242 (52 values; §7's
command), so its colours resolve through the background palette's entries 0-50 under the merged
DAC (`the_merged_palette_is_the_scene_then_the_background`).

At 15,000, 25,000, 35,000 and 60,000 cycles the original shows that merged hold at boot 6400 and
6600 (`0a68ffb9…`, eight frames in `reference/parts/u2a-speed`, `reference-indices.md` §27), and the
port compares at **0 of 64,000** against all eight. The 30,000-cycle reference's 6500
(`11a66eb0…`) is a neon palette, the background through `(3v + 5) & 63` with entry 195 black; its
mechanism is not traced (the release's U2A is PKLITE-packed), and the port differs from it in
**64,000 of 64,000** pixels (A32). An 8.8 fixed-point copper fade running ~3× its count would give
~3v and wrap at 64; that is unproven. The gate-open tick re-uploads the same palette before the
first draw.

## 5. Verification, and the 7300 divergence

| index | port frame | result | register |
|---|---|---|---|
| 6500 | `u2a_006500` | **64,000** RGB pixels (the neon); **0** against the four speed captures' 6400/6600 | A32 |
| 7300 | `u2a_007300` | **1,098** indices and RGB pixels | A29 |
| 7640 | `pam_007640` | **0** indices and RGB pixels | exact |

Pinned by `cargo xtask rediff` entries `u2a`, `u2a-speed/c*` and `fullrun/02-u2a-pam` (5961 at
60,160 and 6500 at 64,000, A53). The 7640 file is PAM's by name because PAM is running there with its gate still closed
(`dis_sync()<10`), holding U2A's cleared page.

**The reference's 7300 is stream state 294.** `the_reference_7300_is_the_measured_stream_state`
renders all 522 states through the part's driver and shipped window: the minimum is **426** pixels
at state 294 (892 at 293, 858 at 295), and 1,098 belongs to state 297 and no other.

**At state 294 the residual is on the Gouraud faces only** (A28). The silhouette is exact: **0**
pixels differ between object and background against the reference's index plane. Attributing each
differing pixel to the topmost object that writes it gives
`[instance 1: 75, instance 2: 0, instance 4: 115, instance 5: 236]`. Instance 2 is `U2A.002`
("Sippi", 75 faces, all `F_SHADE16`, flat-filled) and is exact; instances 1, 4 and 5 are `U2A.001`
("s01", 124 faces, `F_SHADE32`/`F_SHADE16` Gouraud), with index deltas
`{-5,-4,-3,-2,-1,1,2,3,4,5,6,7,8,10,11,12}`.

**The variant scan.** An independent second transliteration of the same sources renders state 294
identically to the Rust engine, and no plausible single difference improves on 426: every face-list
choice and object draw order; reversed matrix multiplication; `nnum1`-only, source, dword and
unsigned normal rotations; camera-only and transposed matrices; ±6 around the shipped multipliers;
±2 camera-entry perturbations; the Gouraud slope divisor, tier bounds, buffer direction and
`calclight`/`normallight` variants. The tree's two filler versions (`AVIDFILL.ASM`, `AVIDFILL.OK`)
differ only in their copy loops. The release's code cannot be read: `SECOND.EXE` carries no `U2A`
or `AVISTAN` strings and no six-byte light-vector sequence (`12118,10603,3030`).

**The live run shows state 297 at 7300.** The gate's two-frame phase (§2) and the ring's page holds
(each page shows four to five stream states) put the port's displayed page three states later: the
whole pose differs, 1,098 pixels in both planes (A29).

## 6. Preserved behaviours

Each is in `bugs.md` §12 or its index; the port's handling in one line:

- **The uninitialised `fov`** (`U2A.C:261`, read at `:287`, set at `:348`): the driver starts at
  fov 0; the first draw is background-only, so the value is invisible.
- **The deadlock escape** (`:278-283`): `next_page` returns the busy page once the copper has
  counted past 16 (`ring_matches_the_u2a_cadence_over_twenty_ticks`).
- **The second gate is dead** (`:351-355`): the check stays first in the loop
  (`the_second_gate_tears_down_on_the_first_tick_it_is_open`).
- **The dead city hacks** (`:301-310`): `U2A.00M`'s byte 15 is 0, so neither branch runs; the
  driver implements both (`the_city_hacks_are_u2a_s_and_guard_out_of_range_instances`).
- **The teardown clears all four pages** (`:432-441`) and `vid_setswitch(3,-1)` names page 3 the
  draw page without moving the display (`AVID.ASM:341-358`). The port installs no-ops in all three
  slots.
- **The two exits leave different screens.** ESC in the gate's wait is a bare `return` (`:239`):
  the hold stays up (`the_gate_holds_until_the_scripted_cue_and_exits_on_request`). ESC in the loop
  falls through to the teardown (`:432`).
- **No music is a refusal**: a run with no player is a `PartError` at `init`.

## 7. Reproduce

```bash
cargo test -p secondreality --bin secondreality u2a
cargo test -p secondreality --test blocks -- --nocapture
cargo xtask rediff --only u2a

# the gates, from the filed traces (6987:11:46, 6988:11:47; 7617:12:54, 7618:12:55)
awk -F, '$1==6987 || $1==6988 {print $1,$2,$3}' reference/music/second1c/trace.csv
awk -F, '$1==7617 || $1==7618 {print $1,$2,$3}' reference/music/second1b/trace.csv

# the comparison by hand: 7640 at 0.0000 %, 7300 at 1.7156 % (1098/64000, max delta 238),
# exit 1 (the port's page is a different stream state, so no ±1 match applies)
cargo run --release --bin secondreality -- --block 1 --headless \
    --dump-frames 6500,7299,7300,7301,7640 --dump-dir out/u2a
cargo xtask compare --reference reference/parts/u2a --rendered out/u2a --out out/u2a-diff

# the background payload's indices: {0} ∪ {192..242}, 52 values
python3 -c "d=open('assets/VISU/C/U2ABG.UH','rb').read()[784:784+64000]; \
print(sorted(set(d))[0], sorted(set(d))[1], sorted(set(d))[-1], len(set(d)))"
sha256sum reference/parts/u2a/frame_007640.rgb        # 0a68ffb9…
```

## 8. Open items

- The neon 6500's mechanism (A32) and the release's Gouraud constants (A28) are in the packed
  image, which the tree's staged `MAIN/DATA/U2A.EXE` is not.

**Register.** `bugs.md` §12 (the page cadence, the two exits), with the dead second gate, city
hacks and `fov` in the index.
