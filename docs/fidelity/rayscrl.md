# RAYSCRL — the raytraced mirror-ball scroll

`WATER/DEMO.PAS` (190 lines) over `WATER/ROUTINES.ASM` (41 lines) and the
linked assets `BKG.CLX` (`BKR.PAS`'s `_tausta`), `MIEKKA.SCI` (`MIEK.PAS`'s
`_miekka`) and `WAT1-3.DAT` (`T1-3.PAS`). A 320×200 mode 13h raytraced picture,
scrolled by a colour-keyed column blitter, with a strip of `MIEKKA.SCI` that
scrolls across it; music-gated at its start and its end.

The port is `bins/secondreality/src/parts/rayscrl.rs`, registered as
`"rayscrl"`. A `--part rayscrl` run seeks `MUSIC1` to order 86
(`shell::music_for_part`), the order the block-3 trace holds when the part's
start window opens.

## 1. What the original does

| source | act |
|---|---|
| `:90-92` | mode 13h |
| `:94-95` | `dis_partstart` |
| `:98-104` | busy-wait `dis_musplus() >= 0`; store `co` from the same call |
| `:107` | `fillchar(fbuf,sizeof(fbuf),#0)` |
| `:108-109` | `MIEKKA.SCI`'s palette at +10 and its first `400*34` pixel bytes at +778 |
| `:111` | the DAC black |
| `:112-113` | `BKG.CLX`'s 64,000-byte picture at +778 rebased to +0, then blitted |
| `:115-116` | `tmppal = pal`; `pal` zeroed |
| `:117-132` | 127 fade-in iterations: on odd `y`, upload `pal` and step every channel toward `tmppal`; `scr(sss)` every iteration, `sss` cycling 0, 1, 2 |
| `:135-143` | a spin until the order differs from `co` **and** the row is at least 16, with no `waitb` |
| `:149-154` | `sss`, `scp`, `frame`, `quit`, `fadeout` reset; `tmppal` zeroed |
| `:155-166` | the main loop's `waitr` and the **`dis_musplus() == -11`** cue |
| `:167-178` | the 65-step fade-out: upload `pal`, step every channel toward zero |
| `:179-186` | `scr(sss)`; on `sss = 2`, `sss := 0`, shift the packed font one byte left, refill one band down from `font[x*400+scp]`, and `inc(scp)` while `scp < 390` |
| `:189` | `until dis_exit or (quit = 1)`; no frame bound |

## 2. The port's tick map, and the mapping

One tick is one `waitr` (`dis_waitb`). Tick 0 is the first poll of the init
wait — the tick the reference's *published* order 88 row 0 first stands at
(`reference/music/second3b/trace.csv`, frame 12962). The release's gates act on
the tick after seven confirming reads (§5), so:

```text
ticks 0-6       the init wait's seven confirming polls, drawing nothing
tick 7          the black DAC, the 64,000-byte blit (boot 12969)
ticks 8-134     the 127 fade-in iterations (boot 12970 … 13096)
ticks 135-653   the stage-2 spin, drawing nothing (boot 13097 … 13615)
tick 654        the main loop's first `scr` (boot 13616)
tick 1988       the cue: `musplus == -11` confirmed, the fade's first upload
                (boot 14950)
ticks 1988-2052 the 65-step fade-out; the part retires at boot 15014
```

**The mapping is `boot = 12962 + tick`, and it is a mapping, not an anchor.**
`shell::boot_anchor("rayscrl")` stays `None`: the part's first drawn frame is
the measured window `(12900, 13000]` (`reference-indices.md` §15), a run of
this part numbers its own frames from zero, and the manifest says
`index_origin: "part"`, `anchor: null`
(`the_run_is_numbered_in_its_own_frames_and_draws_320x200`).

The four numbers the mapping rests on are each pinned:

| boot | port tick | what pins it |
|---|---|---|
| 12969 | 7 | the 13000 keyframe: the fade's k=14 hold is 31 ticks after the blit, and the reference frame at 13000 is exactly the port's tick 38 (`the_fade_in_is_one_hundred_twenty_seven_iterations_and_pins_tick_zero`) |
| 13616 | 654 | the main-loop font phase: the reference at 14000 is the port's 384th main-loop draw, and the stage-2 confirmation puts the first draw at 13616 |
| 14000 | 1038 | the font phase above, and the reference frame |
| 14500 | 1538 | the same, 500 draws later |

The port's clock is the copper's 70.086 Hz tick, one `waitr` an iteration, so
the tick count and the boot frame advance together at 1:1 over the spans the
keyframes cover; the music trace's own frame labels and the drawing labels
agree within a frame at every one of the four.

## 3. The blitter and the font

`Putrouts1` (`ROUTINES.ASM:12-40`) walks one list's 5,372 columns (`158*34`,
`:21`). Each column is a count and that many 16-bit video offsets; a zero count
is a hidden column and writes nothing (`:23-26`). For each offset `di`:

```text
al = fs:[bx]                     ; this column's font byte
if al == 0 then al = gs:[di]     ; ... a zero replays the background pixel
es:[di] = al
```

— a **colour key**, not MNTSCRL's additive +128
(`the_blitter_replays_the_background_where_the_font_byte_is_zero`). `scr`
(`:51-80`) picks one of the three lists.

The shift and refill are **not** in `scr`; they are in the main loop, after the
`scr(2)` that drew with the old font (`:183-185`):

- `move(fbuf[1],fbuf,sizeof(fbuf))` copies 5,373 bytes, one past the declared
  `array[0..158*34]`; the next word is `scp`, so the byte read is its low byte.
  The port keeps the extra byte and copies it, and the refill's x=33 write then
  overwrites its destination.
- `for x := 0 to 33 do fbuf[158+x*158] := font[x*400+scp]` writes at
  `158*(x+1)`, **one band below** the band each sample belongs to; index 0
  never receives a sample.
- `if scp < 390 then inc(scp)` stops ten columns short of the 400-wide source.

All three are kept and pinned by
`the_refill_is_one_band_down_reads_a_byte_past_fbuf_and_stops_at_390`.

## 4. The music: the seek, the pre-roll, and the gates

**The seek is order 86.** `reference/music/second3b` holds `MUSIC1` order 86
(pattern 60) from frame 12704 through 12961, and order 88 (pattern 35) from
12962. The part's window is `(12900, 13000]`, so 86 is the order in force when
the sequence reaches it, and `music_for_part` returns `(MUSIC1, 86)` — measured
from the trace.

The seek is load-bearing beyond the ordinal. Order-list entry 87 is a `254`
marker; playing *through* it is what sets `zplus` bit 1, the value 2 the
reference's player carries into order 88. A seek straight to 88 clears `zplus`
to 0, and the init's `musplus >= 0` gate would never pass (the derived half of
the record: the trace gives the order, the marker and the player's
`enter_order` give the `zplus`).

**The pre-roll is 268 polls.** `play_song(86)` publishes order 88 row 0 on its
269th poll; the run's tick 0 is that poll, so `init` pre-polls 268
(`the_music_pre_roll_puts_the_reference_state_at_tick_zero`). The consequence
is visible in the dump: the published position at each mapped tick is the
trace's own —

| boot | tick | dump | trace |
|---|---|---|---|
| 13000 | 38 | `88:4` | `88:4` |
| 13500 | 538 | `89:2` | `89:2` |
| 14000 | 1038 | `90:0` | `90:0` |
| 14500 | 1538 | `90:62` | `90:62` |

(`the_music_is_the_block_three_position_at_every_keyframe`).

**The release's gates act seven ticks after the tree's condition** — see
`docs/fidelity/APPROXIMATIONS.md` A18. The tree's three waits fire on the first
tick their condition holds; the release's leave timings, measured, are seven
ticks later:

| gate | tree's condition first holds | release leaves |
|---|---|---|
| init (`musplus >= 0`) | boot 12962 (the published 88:0) | the blit at 12969 |
| stage 2 (order ≠ `co`, row ≥ 16) | boot 13609 (the published 89:16) | the main loop's first draw at 13616 |
| cue (`musplus == -11`) | boot 14942 (the trace; the port's player one poll on, 14943) | the fade's first upload at 14950 |

The port implements one rule for all three: `GATE_CONFIRM_TICKS = 7` — the
condition is read true on seven consecutive ticks and the gate acts on the tick
after (`the_releases_confirmation_starts_the_main_loop_seven_ticks_after_the_gate`).
The tree's immediate condition is kept as the counterfactual and measured: with
it the main loop starts at boot 13610, and the reference's 14000 and 14500
frames differ by **8,375** and **11,958** pixels
(`the_trees_immediate_gate_draws_the_keyframes_early`).

**The cue is exactly -11.** `-10` and `-12` never trigger it
(`minus_ten_and_minus_twelve_never_trigger_the_cue`); a scripted `-11` fades
over 65 uploads and ends the part (`the_cue_is_exactly_minus_eleven_and_fades_sixty_five_ticks`).
The fade's onset is confirmed like the other two gates; the port's published
`musplus` is one poll behind the trace's, so the confirmation puts the first
upload on the release's own frame (14950).

## 5. The fade, and the bugs kept

The fade-in is **127** iterations, not 128: `for y := 0 to 63*2` is 0 through
126. On odd `y` the same loop uploads and steps (`:122-126`), so entry `x` is
uploaded before its own step but after entry `x-1`'s `pf = 3` step. Two
consequences, both kept:

- **The `pf := 0 to 3` loop steps the next entry's red.** `pal[x*3+3]` is
  entry `x+1`'s red, so every red but entry 0's advances twice a pass while
  green and blue advance once. At the k=14 upload the displayed red is
  `2k+1 = 29` and green/blue are 14; that is exactly the reference's 13000
  frame, byte for byte (35 colours, mean **23.1535**).
- **The last upload is one step short.** The 63rd upload (y=125) displays
  green and blue at 62 and then steps the stored array to 63; the fade ends
  with `pal` at 63 and the screen at 62. The reference's 13500 palette
  (`reference/parts/rayscrl/manifest.json`) is MIEKKA's with exactly **seven**
  blue channels at 62 instead of 63 — the entries whose target is 63
  (`the_fade_in_is_one_hundred_twenty_seven_iterations_and_pins_tick_zero`).

The fade-out (`:167-178`) is the same loop downward, 65 ticks, and it uploads
`pal` — the stored 63s — so its first upload is one step *brighter* than the
screen the main loop had been showing. The reference's first fade-out upload is
that step: `out/b4b-ray-sess1/frame_014950.rgb`, from the survey's partner
capture (`reference-indices.md` §15.4's second command, whose frame list carries
`14950,15000,15050`; `out/` is gitignored, and the command is the one that
reproduces it). The port's first upload lands on the same boot frame.

**The aliasing bug.** `pal` and `tmppal` are `array[0..768]`, and the `pf = 3`
step at x=255 touches index 768. In the original's data segment `pal[768]` is
`font[0]` (the next global) and `tmppal[768]` is `pal[0]`. The port keeps the
769th byte of each array and reads `pal[0]` as the alias target; with the real
palette entry 0 black, `pal[0]` stays 0 and the font is never touched
(`the_fade_loops_step_the_next_entrys_red_and_read_the_byte_past_the_array`).
The `x*3+3` write is the same test's first half.

## 6. The assets, and the keyframe residual

| asset | bytes | what the part uses |
|---|---|---|
| `BKG.CLX` | 64,778 | RIX3 320×200; palette at 10 (unused — MIEKKA's wins), picture at 778 |
| `MIEKKA.SCI` | 14,778 | RIX3 declaring 400×250 and carrying 35 whole rows; palette at 10, the first `400*34` pixel bytes at 778 |
| `WAT1-3.DAT` | 23,096 / 23,034 / 27,818 | 5,372 columns each, 20,858 positions in all (6,176 + 6,145 + 8,537) |

`the_assets_are_read_the_way_the_source_reads_them` compares every one of those
numbers against the files: MIEKKA's palette at +10 (and MIEKKA's 64 entries
that differ from BKG's), the font's first 13,600 bytes, the rebased background,
the lists' column count and 20,858 positions. The largest destination offset in
any list is **63,999**, inside the picture.

**The residual.** At 14000 and 14500 the port's RGB differs from the reference
by **3** and **42** pixels of 64,000 (0.0047 % and 0.0656 %), all of them in
the font-overlay region the strip has scrolled into (row 0 columns 2-4; rows
0-5, columns 3-27). 13000 and 13500 are byte-identical, the latter in both
planes. The cause is a release-versus-tree **asset** difference, not an
algorithm one: the differing pixels all draw from `MIEKKA.SCI`'s strip, and at
five sampled source positions the reference's byte is the tree's *previous*
source column (`font[15*400+34]` for `+35`, `font[14*400+201]` for `+202`,
`font[14*400+202]` for `+203`, `font[13*400+203]` for `+204`,
`font[15*400+201]` for `+202`) — a patched or re-rendered strip in the shipped
release. The tree's `MIEKKA.OBJ` carries the file's bytes exactly, so the port
reads the asset the tree links; patching bytes the release alone holds would
stop it being an original-format read. This is a measured difference, not a
convention: the five pairs were recovered from the two keyframes' differing
pixels.

## 7. The real run

The unit tests and the integration test are the reproducible commands:

```bash
cargo test -p secondreality --bin secondreality parts::rayscrl
cargo test -p secondreality --test rayscrl
```

The first is the cue-and-exit suite, the fade and font units, the assets, the
music pre-roll, the D24 counterfactual and the four keyframes against
`reference/parts/rayscrl`. The second drives the real binary headlessly from
the repository root over the support frames D19 requires (`K-1, K, K+1` around
every moving keyframe; the held 13500's neighbours are the same bytes) and
compares the frames at the mapping above. `cargo xtask compare` is **not** the
tool: it pairs frames by the manifest's index, and this run's tick 38 is not
the reference's index 13000; the byte comparison at the mapped tick is
stricter, because the port has no session-to-session phase to search.

The numbers, from the integration test: 13000 `0` differing bytes (RGB),
13500 `0` in both planes, 14000 `3` pixels, 14500 `42` pixels; every neighbour
differs from its keyframe except the 13000 upload/hold pair (tick 37 = tick 38,
the reference's own 64-pixel capture artefact) and the held 13500's pair. The
run's manifest says `index_origin: "part"`, `anchor: null`, `MUSIC1` at order
86.
