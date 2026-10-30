# U2E — the city flythrough

Sources: `VISU/C/U2E.C` (628 lines) over the VISU library U2A uses (`VISU/VISU.C`, `ACALC.ASM`,
`ADRAW*.ASM`, `AVIDFILL.ASM`, `AVID.ASM`, `AVIDM1.ASM`); the packed scene
`assets/VISU/C/SCENE/U2E.00M`, `.0AA`, `.0AB`, `.001`–`.042`. Port:
`bins/secondreality/src/parts/u2e.rs` (`"u2e"`) over `parts/visu/{math,draw,mod}.rs`; its module
doc lists the six acts, and `prepare`'s doc the jelly substitution. Run: `SECOND 4`'s only part, anchor
**418** (`shell::U2E_ANCHOR`, derived, §3); in blocks 1–3 it follows JPLOGO after the mid-run `MUSIC0@18` restart (`MAIN/U2.ASM:880-885`).
Music: `MUSIC0` from order 18 (D34); the gate waits for the published order to pass 18.

## 1. Timeline

A `--block 4` run, boot = 418 + part tick.

| part tick | boot | source | on screen |
|---|---|---|---|
| 0..32 | 418..450 | `U2E.C:285-295` | 33 fade-in steps (`+2` to bytes 3..761, clamp 63) over the loader's black |
| 33..48 | 451..466 | `:297-300` | 16 waits; still black |
| 49 | 467 | `:303` | `fadeset` into page 0: the white band |
| 50..67 | 468..485 | `:304-319` | CRTC 9 `(old & 0xf0) \| 0x80`, `fadeset` into pages 1..3, 16 waits |
| 68..100 | 486..518 | `:321-335` | 33 fade-out steps (`-2` to bytes 3..758, `+2` to 759..764); 100 intro waits in all |
| 100 | 518 | `:336-353` | the setup: `vid_init(11)` (state only), the main palette, `vid_window`, copper slot 2 |
| 280 | 698 | `:357-366` | the gate: order 19 published (`second4` reads it at 699) |
| 281.. | 699.. | `:368-540` | the ring reset; the first iteration skips the draw; then one stream frame per two ticks over 1,801 frames |
| end + 1..16 | | `:541-559` | the teardown: the captured DAC faded `+4` over 16 retraces; the 16th upload is the last visible tick |

`:368-374` resets `coppercnt`, `syncframe`, `avgrepeat` and `cl[].ready` but not `copperdelay`,
`clr`, `clw` or `frames[]`; the copper has run since the setup, so by the gate (181 ticks later) its
16-retrace start-up delay is spent.

## 2. The pages, the ring and the cadence

The four-page ring, `copper2` (`:115-143`) and the page packing at `page * 64_000` are shared with
U2A: the mechanism is `parts/visu/mod.rs` (`Ring`, `Driver`), and the packing is A35 (`u2a.md` §3).
U2E calls `vid_setswitch(-1, clr)` (`AVID.ASM:341-358`) from its copper, so only the display start moves; slot 2 runs after the tick's
compose, so a page switch is visible from the next tick, the original's retrace timing.

U2E's own cadence: each iteration clears its page with `vid_clear255()` (all 0xFF,
`AVID.ASM:299-309`) and parses `repeat = (a+1)/2` stream frames, advancing `currframe` by
`repeat*2`. A busy writer page is that tick's wait, not a spin. The result is one stream frame per
two ticks (`the_cadence_over_eight_ticks_advances_four_stream_frames`). `0xFF 0xFF` resets the scene
and sets `xit`.

## 3. The anchor and the intro evidence

**The anchor is derived.** The loader's flat black holds through 466 and the fadeset's white band
appears at **467** in two independent dense scans (`reference-indices.md` §18.7). The source draws
the band after 49 waits and before the fadeset's own `dis_waitb()`, so it is part tick 49 and the
intro's tick 0 is `467 - 49 = 418` (`the_fadeset_lands_on_tick_49_and_paints_the_measured_band`;
417 would put the band at 466). `u2e_runs_are_numbered_from_their_derived_intro_anchor` pins the
numbering.

The measured milestones:

| boot | screen | session |
|---|---|---|
| 420–466 | flat black, one colour, `ea0787f6…`, the loader | start, dense-a/-b |
| 467–473 | the band, `6eb4e4cd…`: 185 white at x 68..252 and 135 black in every row (37,000 / 27,000) | dense-a/-b |
| 490, 500 | the fade-out: at 490 entry 252 = 53 and 254 = 10; at 500, 33 and 30 | start |
| 530, 590, 660 | the post-fade hold, `befaf38a…`: rows 25..174 white, the rest black (mean 191.25); `reference-indices.md` §18.6 ties it to a `SECOND 3` boot's 18000/18100 | sweep2 |
| 698 (port) / 699 (`second4`) | order 19 first published | `the_block_four_clock_reaches_order_19_at_698` |
| 1000 | the console, 36 colours | `reference/parts/u2e/frame_001000.rgb` |

**The intro is byte-exact**: a `--block 4` run's 460, 467, 470, 480, 490, 500, 530, 590 and 660 are
identical to the sessions' RGB captures. The fade-out's step arithmetic: the first mutation is part
tick 67 and its upload tick 68, so boot 486 is step 1, 490 (tick 72) step 5 and 500 (tick 82) step
15 (`the_fade_out_starts_on_tick_68_and_matches_the_measured_490_and_500`). To reproduce:

```bash
for args in "456,457,458,459,460,461,462,463,464,465,466,467,468,469,470,471,472,473 out/u2e-dense-a" \
            "420,430,440,450,460,470,480,490,500 out/u2e-start" \
            "430,480,530,590,660,760,850,950,3100,3200,3400,3700,3900,3950,4050,4100,4200,4400,4600,4800 out/u2e-sweep2"; do
  set -- $args
  cargo xtask reference capture --arg 4 --boot autoexec --keys "KBD_left KBD_enter" \
      --cycles 30000 --frames "$1" --out "$2"
done
cargo run --release --bin secondreality -- --block 4 --headless \
    --dump-frames 460,467,470,480,490,500,530,590,660 --dump-dir out/u2e-intro
for f in 460 467 470; do cmp out/u2e-intro/u2e_000$f.rgb out/u2e-dense-a/frame_000$f.rgb; done
for f in 480 490 500; do cmp out/u2e-intro/u2e_000$f.rgb out/u2e-start/frame_000$f.rgb; done
for f in 530 590 660; do cmp out/u2e-intro/u2e_000$f.rgb out/u2e-sweep2/frame_000$f.rgb; done
```

Every `cmp` exits 0. The sweep2 capture exits 2, refused at 4400 (320×400 there;
`reference-indices.md` §18.2); the frames through 4200 are written, and 530/590/660 are the hold.

## 4. The palette, the jelly substitution and the fadeset

**The fadeset** (`U2E.C:145-183`) writes under map mask 15, so one address is linear pixels
`4A..4A+3`; the `3c5=2+4+8` write at byte 63 sets planes 1..3 only, so x 252 keeps plane 0's byte
and x 253..255 take the special value:

| rows | x 0..67 | x 68..252 | x 253..255 | x 256..319 |
|---|---|---|---|---|
| 0..24 | 0 | 252 | 0 | **not written** |
| 25..174 | 254 | 253 (x 252 = 253) | 254 | 254 |
| 175..199 | 0 | 252 | 0 | 0 |

The top block's last write is `memset(vram+y*80+252,0,16)`, 172 addresses past its row, landing in
rows `y+3..` that the later passes overwrite; the middle and bottom blocks write `+64`, within their
own row. Every stray address is in-page: the largest is **15,999**, so the port's 16,000-address page
bound never fires (A35). Under the saturated fade-in palette (entry 0 black, 252/253 white, 254
black) this is exactly the measured band at 467; the top rows' x 256..319 keep the loader page's
black (`the_fadeset_writes_the_source_s_pattern_within_its_page`).

**The jelly branch: the port always takes the capture.** `jellywas` is the BIOS video mode
(`a > 3`, `:198-205`): 0 runs `vid_init(1)` and sets `fpal` to entries 1..63 = 63, the rest 0
(`:274-277`); 1 captures the live DAC (`:281-282`). Every port run shape begins in the graphics
surface, so the capture is the branch: `--block 4`/`--part u2e` captures the shell's boot DAC
(`hardware_default_palette`), a mid-chain entry its predecessor's. The text-mode branch is
unreachable and not implemented. **This is not an approximation**, so it is not registered: the
reachable branch is the source's own. It changes no measured frame: the fade-in saturates every byte
it touches (3..761) from either start, and the untouched entry 0 and 254/255 are black in the boot
DAC (its table is 248 entries, the last eight unset), as the text-mode branch leaves them.
`a_jelly_boot_captures_the_live_dac` and `a_block_four_shaped_init_captures_the_live_dac_too` pin
it. The flag is the BIOS mode, not `PartCtx::music_fresh`, because `set_music` re-arms that flag at
the U2E restart.

**The main palette.** `cp = scenem+16` is the `.00M`'s 768 bytes; entries 252 and 255 are set black
and 253, 254 to 63 (`:337-350`), and all 256 are uploaded on the setup tick (part tick 100, boot
518).

## 5. Verification

`reference/parts/u2e/` holds four RGB-only keyframes (the part writes CRTC 0x0C every retrace;
`reference-indices.md` §18.3). The run's manifest says `index_origin: "boot"`, `anchor: 418`,
`MUSIC0` order 18 aligned to 418 (the clock starts at 408, and 408..=417 are pre-polled).

| keyframe | port / `second4` music | exact frame | at `K − 4` | register |
|---|---|---|---|---|
| 1000 | 19:34 / 19:34 | 11,006 (17.1969 %) | 189 | A31, A30 |
| 1600 | 20:39 / 20:38 | 11,805 (18.4453 %) | 7,118 | A31, A30 |
| 2600 | 22:25 / 22:25 | 17,278 (26.9969 %) | 231 | A31, A30 |
| 3600 | 24:11 / 24:11 | 16,234 (25.3656 %) | 1,177 | A31, A30 |

The `K − 4` counts are the `u2e` rows of `xtask/src/rediff/table.rs`; the exact-frame counts are
asserted by `the_keyframes_record_the_measured_session_phase_and_gouraud_residual`. 1600's music sits
on a row boundary within a frame (D17).

**The session-phase intercept (A31).** The port displays stream state `(t - 698)/2` at boot `t`; the
filed keyframes are `(t - 702)/2`, the same slope and a four-tick intercept. The port's clock reaches
order 19 at 698 where `second4` reads 699, and the keyframe capture is another session. Nothing is
fitted to it: the port keeps the tree's clock and the byte-exact intro.

**The Gouraud residual (A30).** At `K − 4` the silhouettes are exact (0 pixels disagree between
background and object). Every differing pixel's topmost writer is Gouraud-shaded: `s01` (123 of 124
faces Gouraud) at all four, `Tree01g` (64 of 70) at 1600, `Car02` (10) at 3600. The index deltas,
reference minus port:

```
1000  s01         {-2:3, -1:7, 1:138, 2:13, 6:2, 7:4, 8:5, 9:4, 10:3, 11:7, 12:3}
1600  Tree01g/s01 {-4:2, -3:2, -2:41, -1:322, 1:6655, 2:78, 3:17, 4:1}
2600  s01         {-8:3, -7:1, -6:1, -5:1, -4:3, -3:10, -2:12, -1:16,
                   1:103, 2:30, 3:10, 4:8, 5:17, 6:12, 7:4}
3600  s01/Car02   {-7:1, -6:1, -4:8, -3:11, -2:65, -1:266,
                   1:616, 2:136, 3:14, 4:24, 5:4, 6:8, 7:11, 8:7, 9:5}
```

Flat faces reproduce exactly; this is A28's release-versus-tree Gouraud difference (`u2a.md` §5).
The states, the attribution and the deltas come from the ignored diagnostic
`search_reference_states`.

**The block-3 seam.** The port's U2E starts at chain **17,418** (JPLOGO's last frame is 17,417),
nine ticks before D34's mapping-derived 17,427: the gapless handover (D31 §3; part-sequence.md (block
seams / pre-roll / mapping conventions)), not the restart. The restarted player waits D34's four
ticks, primes its ten-deep ring at `0:0`, first publishes `18:1` at chain 17,440, and against
`second3e` (chain = boot − 419) is within one MUSIC0 row (8.76 frames): `18:17` against `18:16` at
17,581, `18:19` against `18:18` at 17,600, and `19:1` against `19:0` at 17,721
(`a_block_three_run_is_the_whole_chain_and_its_mid_chain_entries_do_not_pre_roll`).

## 6. Preserved behaviours

- **`fov` is uninitialised** (`U2E.C:378`) when `vid_cameraangle(fov)` first reads it. The first
  draw is skipped, so the read is invisible; the stream's fov is 0x1c for all 1,801 frames.
- **The first draw is skipped**: `firstframe` draws nothing and resets `syncframe`/`coppercnt`
  (`:433`); the first loop tick still queues page 0, the fadeset page
  (`u2e_skips_the_first_draw_and_resets_the_clock`).
- **The deadlock escape**: `while(cl[clw].ready) { if(deadlock>16) break; }` draws into a page the
  display may still hold (`Ring::next_page`).
- **The teardown's DAC rotation, one tick late** (`:545-554`): each step is `fpal[a]+=4;
  dis_waitb(); outp(0x3c8,255);` then 768 writes, so the trigger tick only captures and computes step
  1, step `n` is visible on tick `T+n`, and the part ends on the tick that shows step 16. The write
  starts at entry 255 and wraps: entry 255 takes `fpal[0]`, entry `j < 255` takes `fpal[j+1]`, and
  nothing is left unwritten.
- **The city hacks are dead**: `U2E.00M` byte 15 is 0 and U2E never reads `city`.
- **The name hacks run**: `name[1] == '_'` forces distance 1,000,000,000 (`:408`), and
  `1800 < currframe < 2200` forces `s01`'s distance to 1 (`:410-416`). The scene has **nine** `_`
  objects (U2E.001, .003, .011, .014, .015, .016, .017, .020, .022) and `U2E.028` is `"s01"`
  (`the_scene_is_the_u2e_city_with_the_name_hacks_object`).
- **The waits are ticks, not spins**: every intro wait, gate poll and busy writer page is one
  `frame()` call, and `ctx.exit_requested` is read in the gate and in the loop.
- **No music is a refusal**: the gate reads the published order, so a run with no player is a
  `PartError` at `init`.

`bugs.md` §12 holds the cadence, the skipped draw, the rotation and the name hacks; `fov` and the
city hacks are in its index.

## 7. Reproduce and open items

```bash
cargo test -p secondreality --bin secondreality parts::u2e
cargo test -p secondreality --test u2e -- --nocapture
cargo xtask rediff --only u2e
cargo test --release -p secondreality --bin secondreality search_reference_states -- --ignored --nocapture
```

**The older sweep session's hold is a reference anomaly.** A first `--arg 4` sweep session's 600 and
700 are one 4-colour screen (`ef539f9e…`, mean 158.30): the fadeset page under a red/cyan palette
(entry 252 = `(20,0,0)`, 253 = `(0,63,63)`, 254 = `(63,63,63)`), `reference-indices.md` §18.2. The
sweep2 session's 530/590/660 are the 2-colour hold that §18.6 independently matches on a `SECOND 3`
boot, and the port matches sweep2. Both sessions' 500 frames are identical (`a830146f…`), and no
intro path or palette reaches the 600 screen (every path ends black/white at the hold). The port
follows the source and the sessions that agree with it, so nothing is registered.

**Register.** `bugs.md` §12 and the index; A30, A31 (A28, A34, A35 shared with U2A).
