# TUNNELI — the dot tunnel

`SECOND 2`'s third part, slot 6 of `part-sequence.md` §1: 77 rings of 64 dots, chain-copied outward
one record per frame, one plot per dot with an `oldpos` erase, for `veke = 1060` frames. Turbo
Pascal (`TUNNELI/TUN10.PAS`, 227 lines, with an inline-assembly inner loop) over two linked data
blobs, `TUNNELI/TUNNELI.OBJ` and `TUNNELI/SINIT.OBJ`. **TUN10 is the shipped build**:
`TUNNELI/X.BAT:1` stages `tun10.exe` as `..\main\data\tunneli.exe`. `TUN9.PAS` is the earlier
revision and three things changed: `veke` 980 → 1060 (`:3`), a different palette ramp
(`:118-119` → `:121-122`), and the chain copy at `:204`, which TUN9 has commented out.

The port is `bins/secondreality/src/parts/tunneli.rs`, registered as `tunneli`. Its reference is
`reference/parts/tunneli/` — 3800, 4200, 4500, 4600 — and **that reference cannot be the verdict**:

- **No `.idx` at all** in the four frames. 41 of the 43 frames this block took on TUNNELI's screen
  were refused as `the screen moved while it was read`; the two that came back are 3730 and 3750,
  both near-black (`reference-indices.md` §8.2). So every comparison is RGB.
- **Every keyframe moves**, so every one needs support frames (§8.3), and the tunnel is fast: the
  reference's own adjacent-index motion is **2.05–2.22 %** of pixels a frame at 3800, **10.47–10.69 %**
  at 4200, **6.22–6.40 %** at 4500 and **8.74–9.08 %** at 4600 (§8.4).
- **Five sessions hold three distinct pictures at 3800**, one of them in three sessions (§8.3), and
  **4500 and 4600 are formally recorded as having no settled content** — the largest-support picture
  is presented by 2 of 5 sessions at 4500 and 1 of 6 at 4600 (`reference-indices.md` §1.8, §2).
- A run of the port that starts at this part **is not in the reference's numbering at all**: TUNNELI
  begins after GLENZ, which the port has not written, so `shell::boot_anchor("tunneli")` is `None`
  and the run's indices are its own ticks (§7).

So the evidence here is what the *source* pins (§8) and a **contact sheet a person reads** beside
the reference's own (§7.2), and the comparison is run and recorded rather than believed (§7.1).

## 1. What the original does

```
:99-104   aa1[x] := 64000; move(aa1,aa2,sizeof(aa1)); rows[x] := x*320;
          move(mem[seg(sini):ofs(sini)],        sinit, 4097*2);
          move(mem[seg(sini):ofs(sini)+4097*2], cosit, 2048*2);
          move(mem[seg(tun):ofs(tun)],          pcalc, sizeof(pcalc));
:110-115  mov ax,$13 / int $10                mode 13h — the part sets it itself
:121-128  setrgb(64+x,(64-x),(64-x),(64-x))   entries 64..128: a grey ramp
          setrgb(128+x,(64-x)*3 div 4,…)      entries 128..192: three quarters of it
          setrgb(68,0,0,0); setrgb(132,0,0,0); setrgb(255,0,63,0)
:144-148  frame := 0; waitr; sade[z] := round(16384 div ((z*7)+95))
:150      repeat
:151        waitr;                             one vblank, and DAC entry 0 rewritten (:46, :52)
:152-198    draw rings 80 downto 4, 64 dots each, erase then plot, skipping
            any ring whose bbc is below 64
:200-225    for sync := 1 to frames do         once per tick: the animation step
:227      until quit
```

The animation step, `:202-221`:

```
putki[100].x := cosit[sy and 2047] - sinit[sy*3 and 4095] - cosit[sx and 2047];
putki[100].y := sinit[sx*2 and 4095] - cosit[sx and 2047] + sinit[y and 4095];
move(putki[1], putki[0], ofs(putki[100])-ofs(putki[0]));   inc(sy); inc(sx);
if (sy and 15) > 7 then putki[99].c := 128 else putki[99].c := 64;
if frame >= veke-102 then putki[99].c := 0;
if frame = veke then quit := true else inc(frame);
```

Four things about that are load-bearing and each is a section below:

- **`putki[100].y`'s third term is dead.** `TUN10.PAS:71` declares a global `y`, `:93` sets it to 0,
  and nothing writes it again — the ring index in the draw loop is `x` (`:153`) and the y difference
  is `by` (`:156`) — so `sinit[y and 4095]` is `sinit[0]`, 0. The port writes the term out as the
  source has it.
- **The chain copy is 100 records, not 99.** The count is `ofs(putki[100])-ofs(putki[0])`, an offset
  difference, so `putki[0..99]` take `putki[1..100]`; each record is six bytes (`rengas`, `:11-14`
  — two `integer`s and a `byte`, word-aligned). `putki[99]` is therefore the **new head** after the
  copy, and `:219-220` then sets its colour: the colour wave the drawn rings `4..80` carry is
  `frame - (99 - x)`'s colour, one ring per frame.
- **A ring whose colour is below 64 is not drawn at all.** `:161` `if bbc >= 64 then` guards the
  whole assembly block — the erase included. With `c = 0`, `bbc = round(x/1.3)` is 3..62 for
  `x = 4..80`, so from `:220`'s tick on, the rings stop being drawn as the 0 walks down the chain;
  what goes black is the erases of the rings still moving. See §6.
- **The clip tests the x sum only** (`:179-190`) and the y term reads `rows` unchecked. §4.

## 2. The two data files, and why `sr-assets` is not their door

The plan's Interfaces line says `sr_assets::raw_surface`/`load_path` for
`assets/TUNNELI/{TUNNEL.DAT,SINIT.DAT}`. **Neither door fits, and it is measurable rather than a
preference.** Both files are the linked *data segments* of the originals' object files —
`asset-inventory.md` records `TUNNELI/SINIT.OBJ`→`SINIT.DAT` and `TUNNELI/TUNNELI.OBJ`→`TUNNEL.DAT`
among "35 blobs [that] reconstruct a standalone file byte-for-byte" — and neither is a picture:

- **`sniff` returns `None` for both.** They carry no magic (the inventory's own column says
  "35328 bytes of binary data, no magic" and "12292 bytes of binary data, no magic, leading zeros"),
  so `load_path` refuses them with `UnknownFormat`. The port's
  `the_data_files_are_not_pictures` pins that.
- **`raw_surface` would need a geometry the caller invents.** It takes `(bytes, width, height)` and
  requires `width * height == bytes.len()`; 35,328 is 384×92 and 12,292 is 4×3,073, and neither is a
  picture. Writing `raw_surface(&bytes, 384, 92)` would be smuggling a record table through a pixel
  door.
- **`omf::extract_ledata` is the door for the `.OBJ`s these came from**, not for the payload it has
  already extracted.

What they *are* is known exactly, and the port checks both:

| file | bytes | what it is | the source's own read |
|---|---|---|---|
| `TUNNEL.DAT` | 35,328 | `pcalc[0..137,0..63]` of `(x, y)` word pairs: 138 rows × 64 dots × 4 bytes | `:104` `move(mem[seg(tun):ofs(tun)],pcalc,sizeof(pcalc))` |
| `SINIT.DAT` | 12,292 | 4,097 + 2,049 words: the sine tables `SINGEN.PAS` wrote | `:102-103`, which copy **4,097 and 2,048** words |

Two details worth recording because they are one word wide each way. `SINIT.DAT` holds **one word
more** than `:103` copies — `cosit : array[0..2048]` is 2,049 words and the `move` takes 2,048 — so
the port copies 2,048 and leaves the array's last word 0, which is what the data segment held. And
the generator in the tree is `SINGEN.PAS`, whose formulas are **not** the ones `TUN10.PAS:107-108`
has commented out: `round(sin(x/128·π) · ((x·3) div 128))` and `round(cos(x/128·π) · ((x·4) div 64))`,
measured equal to the file's 6,146 words word for word.

## 3. The screen: mode 13h, a cleared 64 KB, and Part B's second convention

`:110-115` sets the mode itself, and its effects that a part can see are the geometry (320×200, one
byte per pixel), the addressing (`x + y*320` — the original's own `LINE_LEN`, which is what
`rows[x] := x*320` builds), the cleared video memory, and the DAC the mode set installs for
entries 0..63 (the same table `shell::hardware_default_palette` holds, which `:121-128` deliberately
does not touch).

**This part therefore takes Part B's *second* convention**: it writes flat through
`Surface::pixels_mut()` at `LINE_LEN` = 320, with `View::y_line_offset = Some(320)`, so the
surface's first 64 KB *is* the original's video memory — wraparound and all. That is not a
convenience: §4 depends on the 16-bit address arithmetic, and with the first convention (draw
through `Surface::set` at the surface's 640-wide row stride) a wrapped address would land in a
column the 320-wide view never reads. `View::source_index` reads `start_addr + ly*320 + x`, which
for `ly < 200` is exactly the original's `y*320 + x`.

## 4. The clip, and the read past `rows` that the reference's own frames prove

```
:179-180   mov di,word ptr ds:[si] / add di,dx      di = pcalc[br][i].x + (putki[x].x - putki[5].x)
:181-182   cmp di,319 / ja @yli                     the ONLY bound, and it is on x
:183-186   mov bx,bp / add bx,ds:[si+2] / add bx,bx / add di,ds:rows[bx]
                                                    di += rows[(by + pcalc[br][i].y)]
:187       mov byte ptr es:[di],15                  the plot, colour patched at :168
```

`bx` is `2 * yy` and `rows` is `array[0..200] of word`, so a `yy` outside 0..200 reads the words
that follow it in the data segment. That is not a detail the port may tidy away: **the reference's
own frames are drawn with it.** Measured with a scratch model of the loop that the port reproduces
pixel for pixel (§7.2), the port's frame at the aligned tick matches the reference on **85, 146 and
90 pixels that no other reading reaches** — treating an out-of-range y as 0 agrees on 0 of them, and
clipping it away on 4, 0 and 2 (`frame_004200.rgb`, `frame_004500.rgb`, `frame_004600.rgb`). A port
that "fixes" the bound draws one fewer artefact per dot than the original, exactly as the task said.
The reading itself is pinned in the port's own suite, on a hand-built ring, by
`a_dot_with_an_out_of_range_y_is_plotted_at_the_words_the_read_lands_on`.

The layout the read lands in is the declaration order of `TUN10.PAS:16-29`, and the port models it
in `rows_word`:

```
… putki, pcalc, aa1[0..100], rows[0..200], aa2[0..200], sinit[0..4096],
  cosit[0..2048], sade[0..102], frame, bor, frames, oldpos, …
```

- **Forward of `rows`**: `aa2` — `:100`'s copy of `aa1`, so its first 101 words are 64,000 and the
  rest are never written and are 0 — then `sinit`, then `cosit` (whose 2,049th word `:103` never
  writes), then `sade`.
- **Backwards, past the 64 KB wrap**: `aa1`'s 101 words, all 64,000 (`:99`), and before them
  `pcalc` — this part's own 35,328-byte table, read last word first. The wrap is the hardware's: the
  effective address is `DS:rows_off + bx` in a 64 KB segment, so the *byte* offset wraps at 65,536
  and the word index at half that. The port's `rows_word` masks `yy` to 15 bits of word index, which
  is that wrap in words.
- **Reachability is measured, not assumed.** The y the loop indexes with is
  `by + pcalc[br][i].y`; `by` is a difference of two chain positions; a chain position is `:203`'s
  `sinit[sx*2 and 4095] - cosit[sx and 2047]` (third term 0), or the initial 0. Over the tables as
  shipped that is **-680..=680**, and
  `the_unchecked_read_covers_everything_the_arithmetic_can_reach` recomputes that bound from the
  data files and asserts the modelled window covers it — so the ranges the port does *not* model
  (`frame`, `bor`, `frames`, `oldpos` and the scalars, between `sade` and `pcalc`) are unreachable
  by arithmetic, not by hope.

## 5. The palette, entry 0, and the colour that is self-modified

`:121-128` writes entries 64..192 and 255 only. `Palette::set_entry` masks to six bits, as the DAC
does, which matters at exactly one point: `x = 0`'s `setrgb(64, 64,64,64)` is entry 64 = `(0,0,0)`,
and no dot reaches entry 64 anyway (the lowest colour a drawn ring gets is 67).

`waitr` (`:44-53`) rewrites **DAC entry 0** every frame: `:46` `setrgb(0,0,0,0)` and then `:52`
`if mem[$40:$17] and 16 = 16 then setrgb(0,0,15)`. `$40:$17` is the BIOS keyboard-flags byte and bit
4 is **Scroll Lock**, so the entry is black normally and `(0,0,15)` — dark blue — while Scroll Lock
is on. **The port has no keyboard flags**: it writes the black branch every tick, and the reference
settles what the captured session had — every non-dot pixel of all four TUNNELI frames is exactly
`(0,0,0)`, so Scroll Lock was off and the two agree. A port that modelled the flags could only
differ here.

The colour immediate is patched by the loop itself (`:168` `mov byte ptr cs:[@c+3],al`, with the
plot at `:187`). The port passes the ring's `bbc` as a variable, which is what spec §5.2 asks for
with self-modifying code ("rewritten as variables/tables"). `bbc` is a byte in the source —
`bbc : byte`, `:158` — and the port keeps the byte's arithmetic (`wrapping_add`), though no ring
reaches 256.

## 6. What the port does, and the shape of the fade-out

`init` is `:99-148`: it reads the two data files (§2), builds `sade`, zeroes `oldpos` and `putki`,
sets the view to 320×200 with `y_line_offset = Some(320)` and `border = Some(0)` (§3), clears the
64 KB of video memory the mode set clears, and installs the palette (§5).

`frame` is one vblank of the loop: the entry-0 write, `draw` (`:152-198`), one `sync` (`:200-225`),
and the retirement check. One tick is one vblank is one frame, so `:200`'s `for sync := 1 to frames`
runs exactly once and the source's own `frame` (`:27`, `:221`) is the part's `PartCtx::frame` — which
is what makes the part's constants reference-free: `veke` and `veke-102` are *ticks*, and a run of
this part starts at tick 0.

**The fade is the rings stopping, and the brief's phrasing says otherwise.** The task's test name
(`the_last_hundred_and_two_frames_draw_in_colour_zero`) and the plan's comment ("the tunnel fades
out by drawing its dots in the black entry") describe a mechanism the source does not have: `c = 0`
makes `bbc = round(x/1.3)`, which is below 64 for every drawn ring, so `:161` skips the ring — no
erase, no plot, nothing written in entry 0. What the screen loses is the erases of the rings still
moving; the port's own suite measures the screen black at tick 1,052, and the original's is black in
the (4,700, 4,770] window the reference records (§8.5). The observable the brief describes — the
picture goes black without the palette moving — is right; the mechanism is not, and the test says
what the source does.

The tick map, in the part's own ticks (the reference's index of a run that started at this part
would be this plus the part's boot start, which is a window rather than a number — §7.2):

| tick | what the screen is |
|---|---|
| 0-19 | black: every ring's colour is still the zero `:130-135` left (`:161`) |
| 20-96 | the tunnel forms: ring 80's colour arrives first, then one ring per frame outward |
| 97-957 | the tunnel, its colour alternating 8 bright rings / 8 dim ones and the wave walking outward |
| 958-1051 | the fade: rings stop being drawn as the 0 walks down the chain |
| 1052 | black — measured; the last erases have taken the rest |
| 1053-1060 | black, and the part retires at `:221` |

## 7. The comparison: what it says, and what it cannot

### 7.1 The two commands the plan's Step 3 names

```bash
cargo run --bin secondreality -- --headless --part tunneli --dump-frames 900,1060,2000 \
    --dump-dir out/p2-tunneli
cargo xtask compare --reference reference/parts/tunneli --rendered out/p2-tunneli \
    --out out/p2-tunneli-diff
```

**Exit 2**, and the refusal is the honest answer:

```
error: reference frame 3800 has no rendered frame within ±1 of it that no earlier reference frame
has taken: the nearest rendered index is 2000, 1800 frames away, and the rendered side holds 3
frames. Refusing to pair a frame from further away, which would report a number that means nothing
```

Run instead at the reference's own indices — `--dump-frames 3800,4200,4500,4600`, a 4,600-tick run —
the comparison pairs by number and **exits 1**, and the number it prints is the *numbering's* gap,
not the port's: the run's tick 3,800 is past the last thing this part draws (tick 1,060), so the
port's frames are the black screen the run holds while the reference's are dots. Every differing
pixel is therefore a reference dot against black, which is why each frame's count is exactly the
reference's ink:

```
frame 3800: 3.7812% differing (2420/64000 px), max delta 190
frame 4200: 5.6312% differing (3604/64000 px), max delta 239
frame 4500: 3.1422% differing (2011/64000 px), max delta 231
frame 4600: 4.9344% differing (3158/64000 px), max delta 239
verdict: different
exit: 1
```

### 7.2 What the port *does* look like, measured at the alignment it can be measured at

`--part tunneli` numbers its frames by its own ticks, and the reference numbers its by the boot's.
The two are related by one number, and the reference itself supplies it: the port's frames at ticks
**78, 478, 778 and 878** are the pictures the reference holds at **3800, 4200, 4500 and 4600** — the
offset is **3,722** in all four, and it is the only alignment in the whole part that puts the
reference's own adjacent-index motion on the right side of the difference (a wrong tick is one
frame's motion away, 2–10 % of pixels). It is also a *measurement of the same kind* as §7.5's: the
reference's first ring is recorded at boot 3710 and this run's tick 0 answers 3,722, twelve frames
later — inside the tens-of-frames window the part's start is known to move in between sessions
(`reference-indices.md` §8.5), and the four keyframes agree on the offset among themselves, which is
what matters.

At that alignment the port and the reference differ by

| reference index | port ticks | differing pixels | % of 64,000 | the port's own motion per frame |
|---|---|---|---|---|
| 3800 | 78 | 1,270 | 1.98 % | 1,180 / 1,361 |
| 4200 | 478 | 2,125 | 3.32 % | 7,000 / 7,445 |
| 4500 | 778 | 1,214 | 1.90 % | 4,070 / 4,063 |
| 4600 | 878 | 1,965 | 3.07 % | 6,536 / 6,636 |

**That is less than one frame of the part's own animation at three of the four keyframes and about
one frame at 3800** (the table's last column), against a reference whose own re-capture differs by
8.07–10.18 % from two earlier sessions and whose five sessions hold three different pictures at 3800
(§8.4, §8.3).

**The residual is real, and its cause is not identified.** Whatever it is, it is not one of the
things this port had to get right and could have got wrong: a scratch model of the loop, written to
settle §4's read and §1's colour wave before the port existed, agrees with the port **pixel for
pixel at 14 ticks spanning the whole part** (0, 20, 60, 78, 120, 200, 478, 700, 778, 878, 1,000,
1,040, 1,052, 1,060), and it is that model the three ruled-out candidates below were measured on.
The model is not in the tree — it is a task working note, and the numbers in this section are its
record — but the port reproducing its frames exactly is what makes its findings the port's:

- **Not the phase.** A 16-step sweep of the colour wave's phase (which the source's `(sy and 15) > 7`
  gives one period of) puts the port's own phase best by a factor of two: 1,270 differing pixels at
  offset 0, 2,599 at the worst offset.
- **Not the shape.** The masks agree: at 3800 the port inks a pixel everywhere the reference inks
  one, and 179 more; at 4200 it inks 347 extra and misses 26.
- **Not the wrapped read's word offset**, and not the ring range or the chain stride — the three
  things the port's own tests pin, and changing either of the last two moves the difference by an
  order of magnitude.

A later phase that compares every frame will meet this residual again; it is registered in
`docs/fidelity/APPROXIMATIONS.md` A6 with these numbers, and the reference's own instability
(`reference-indices.md` §8.3, §8.4) is the reason its size cannot be attributed yet.

**The sheet, which is the evidence a person reads.** Both sides, same layout, nothing scaled:

```bash
cargo xtask frames sheet --dir out/p2-tunneli-aligned --out out/p2-tunneli-aligned-sheet.png
cargo xtask frames sheet --dir reference/parts/tunneli --out out/p2-tunneli-reference-sheet.png
```

Read side by side the tiles are the tunnel's four shapes: the **mouth** (a ring of dots around a
black centre, 3800 / tick 78), the **full tunnel** (dots to every edge, the centre skewed up-left,
4200 / 478), the **off-centre tunnel** (its mouth at the top right, walls sweeping across, 4500 /
778) and the **thinner one** (sparser, the mouth low right, 4600 / 878). The reference's own sheet
is byte-identical to `out/tunneli.png`, the one Phase 1's capture wrote, which is a check that this
command renders what `reference capture --sheet` renders.

**The look itself is deferred to the phase boundary**: this note records what the two sheets
contain, not that a person has judged them.

## 8. Tests

`src/parts/tunneli.rs` (thirteen, beside the part):

- `the_tunnel_draws_seventy_seven_rings_of_sixty_four_dots` — the witness is `ry`, the inner loop's
  byte offset into `oldpos`: 77 × 64 × 2 after one tick of a formed tunnel. The ring count, the
  dot count and the erase stride all move it.
- `the_ring_chain_copies_the_last_ring_forward` — every field of every record shifts with its
  neighbour, and `putki[99]` is the *new head*, which is what makes the count 100 records and the
  copy's order (before the colour write) visible.
- `the_animation_advances_once_per_tick` — `sx`/`sy` advance by one per tick, and the head after
  one tick is `:202-203` evaluated at zero.
- `the_last_hundred_and_two_frames_set_the_new_rings_colour_to_zero` — the boundary at tick 958,
  and the screen black by tick 1,052.
- `the_part_retires_after_veke_frames` — `veke = 1060`, `Continuing` at 1,059, `Finished` at 1,060,
  and the last tick advanced the animation before it ended.
- `a_dot_with_an_out_of_range_y_is_plotted_at_the_words_the_read_lands_on` — a dot whose y sums to
  445 lands at `x + sinit[43]`, not at row 0 and not nowhere.
- `a_dot_leaves_its_previous_position_black` — one hand-built ring, moved one pixel between two
  draws: the old pixel goes back to 0 and the new one takes the colour. The erase is the only thing
  that ever removes ink, and this is the test that fails when it is taken out.
- `the_unchecked_read_reads_the_data_segments_own_words` — every boundary of the modelled window:
  `rows`' ends, `aa2`'s two halves, `sinit`/`cosit`/`sade`, `aa1` past the wrap, and `pcalc` read
  backwards.
- `the_unchecked_read_covers_everything_the_arithmetic_can_reach` — the reachable range recomputed
  from the data files, asserted inside the modelled windows.
- `the_ring_colour_rounds_unambiguously` — `round(x/1.3)` is at least 0.02 from a half over 4..80.
- `the_palette_is_the_parts_own_ramps_and_leaves_0_to_63_alone` — both ramps' ends, the two black
  writes, entry 255, and entry 0 rewritten on the first tick while the rest of 0..63 is untouched.
- `the_mode_set_puts_the_screen_at_320x200_and_clears_video_memory` — the geometry, the 320-pixel
  line stride, the overscan entry, and exactly 64 KB cleared.
- `the_data_files_are_not_pictures` — `sniff` refuses both, and both lengths are the record counts
  §2 gives.

`tests/tunneli.rs` (two, through the real binary from the repository root): the run's numbering is
its own (`index_origin: part`, `anchor: null`, 320×200 per frame, tick 0 black and tick 200 dotted)
and the retirement — tick 1,040 has dots, tick 1,060 is black, and the run **holds** that screen at
1,061 and 1,200, byte for byte. What the two add over the unit tests is the wiring: the registry,
the two asset paths as a run resolves them, the mode geometry reaching the dumped frame, and the
hold.

## 9. What is not modelled, and why it costs nothing here

- **`dis_partstart` / `dis_exit`.** `:223` `if dis_exit then quit := true` is the demo server's
  "stop now" flag; in the port that is the engine's exit request, which the shell acts on between
  parts. Nothing of it is a frame count, so nothing of it is a constant here.
- **The vertical-blank *rate*.** `waitr`'s DIS call returns how many vblanks elapsed; the port's
  tick is one frame (Global Constraints), and §7.2's alignment is what says the two clocks agree to
  better than one frame over 878 ticks.
- **The Scroll Lock branch of `:52`** (§5) — the port has no keyboard flags, and the reference's own
  background says the captured session had none set.
- **CRTC timing registers.** `:110-115` is a BIOS mode set, and the port models its geometry, its
  addressing and its clear; the horizontal and vertical timing registers have no analogue in an
  engine that composes a rectangle rather than scanning one out.
