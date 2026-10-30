# TUNNELI — the dot tunnel

**Sources.** `TUNNELI/TUN10.PAS` (227 lines of Turbo Pascal with an inline-assembly inner loop),
staged as `tunneli.exe` by `TUNNELI/X.BAT:1`, over the linked data blobs `TUNNELI/TUNNELI.OBJ` and
`TUNNELI/SINIT.OBJ` (assets `assets/TUNNELI/TUNNEL.DAT`, `SINIT.DAT`, §2). **The build that shipped
is a later revision**: its second palette ramp is a fifth darker than `:122`'s (§5). **Port.**
`bins/secondreality/src/parts/tunneli.rs` (`"tunneli"`); its module doc holds the acts, the
one-sided clip, the self-modified colour, `waitr`'s entry 0 and the fade-out. Integration test
`bins/secondreality/tests/tunneli.rs`. **Run.** A mapping, not an anchor: `--part tunneli` numbers
its own ticks, and **reference index = tick + 3,722** (§7.2). **Music.** None; the part runs
`veke = 1060` frames.

The reference `reference/parts/tunneli/` (3800, 4200, 4500, 4600) is weak evidence:

- **No `.idx`**: 41 of 43 frames on this screen were refused as `the screen moved while it was
  read`; the two that came back, 3730 and 3750, are near-black (`reference-indices.md` §8.2). Every
  comparison is RGB.
- **Every keyframe moves**, fast: the reference's adjacent-index motion is 2.05-2.22 % of pixels a
  frame at 3800, 10.47-10.69 % at 4200, 6.22-6.40 % at 4500 and 8.74-9.08 % at 4600
  (`reference-indices.md` §8.4).
- **Five sessions hold three distinct pictures at 3800**, and **4500 and 4600 have no settled
  content**: the largest-support picture is presented by 2 of 5 sessions at 4500 and 1 of 6 at 4600
  (`reference-indices.md` §1.8, §2, §8.3).

## 1. Timeline

One tick is one vblank (`waitr`, `:44-53`), and `:200`'s `for sync := 1 to frames` runs once a
tick, so the source's `frame` is the part's tick and `veke`, `veke-102` are ticks.

| tick | what the screen is |
|---|---|
| 0-19 | black: every ring's colour is still the zero `:130-135` left (`:161`) |
| 20-96 | the tunnel forms, ring 80's colour first, then one ring a frame outward |
| 97-957 | the tunnel, 8 bright rings / 8 dim, the wave walking outward |
| 958-1051 | the fade: rings stop being drawn as `:220`'s 0 walks down the chain |
| 1052 | black, measured (`the_last_hundred_and_two_frames_set_the_new_rings_colour_to_zero`) |
| 1053-1060 | black; the part retires at `:221` (`the_part_retires_after_veke_frames`) |

The original is black in the (4,700, 4,770] window the reference records (`reference-indices.md`
§8.5). The fade is the rings stopping, not dots drawn in entry 0: with `c = 0`,
`bbc = round(x/1.3)` is below 64 for every drawn ring, so `:161` skips the ring, erase included,
and what goes black is the erases of the rings still moving.

Two facts of the animation step (`:202-221`) that the module doc does not carry: `putki[100].y`'s
third term `sinit[y and 4095]` is `sinit[0]` = 0, because the global `y` (`:71`) is set to 0 at
`:93` and never written again; and the chain copy moves 100 records
(`ofs(putki[100])-ofs(putki[0])`, six-byte `rengas`, `:11-14`), so `putki[99]` is the new head
whose colour `:219-220` sets (`the_ring_chain_copies_the_last_ring_forward`).

## 2. The two data files, and why `sr-assets` is not their door

Both files are the linked data segments of the originals' object files (`asset-inventory.md`), and
neither is a picture: `sniff` returns `None` for both (no magic), so `load_path` refuses them, and
`raw_surface` would need an invented geometry (35,328 is 384×92, 12,292 is 4×3,073). The port reads
the bytes (`the_data_files_are_not_pictures`):

| file | bytes | what it is | the source's read |
|---|---|---|---|
| `TUNNEL.DAT` | 35,328 | `pcalc[0..137,0..63]` of `(x, y)` word pairs | `:104` `move(…,pcalc,sizeof(pcalc))` |
| `SINIT.DAT` | 12,292 | 4,097 + 2,049 words: the sine tables `SINGEN.PAS` wrote | `:102-103`, which copy **4,097 and 2,048** words |

`SINIT.DAT` holds one word more than `:103` copies, so the port copies 2,048 and leaves `cosit`'s
last word 0, as the data segment held. The tree's generator `SINGEN.PAS` (not the formulas
`TUN10.PAS:107-108` has commented out) computes `round(sin(x/128·π) · ((x·3) div 128))` and
`round(cos(x/128·π) · ((x·4) div 64))`, equal to the file's 6,146 words.

## 3. The screen

Mode 13h set by the part (`:110-115`): 320×200 at the original's `LINE_LEN` of 320, written flat
through `Surface::pixels_mut()` with `View::y_line_offset = Some(320)`, so the surface's first 64 KB
is the original's video memory, wraparound and all (§4 depends on it). Entries 0..63 stay the mode
set's (`shell::hardware_default_palette`), which `:121-128` does not touch
(`the_mode_set_puts_the_screen_at_320x200_and_clears_video_memory`).

## 4. The clip, and the read past `rows`

`cmp di,319 / ja @yli` (`:181-182`) bounds the x sum only; the y term indexes `rows[bx]` unchecked
(`:183-186`). The port keeps the read (`Tunneli::rows_word`), modelled on the declaration order of
`TUN10.PAS:16-29`:

```
… putki, pcalc, aa1[0..100], rows[0..200], aa2[0..200], sinit[0..4096],
  cosit[0..2048], sade[0..102], frame, bor, frames, oldpos, …
```

Forward of `rows`: `aa2` (101 words of 64,000, then 100 of 0), `sinit`, `cosit`, `sade`.
Backwards, past the 64 KB segment wrap: `aa1` (all 64,000) and before it `pcalc`, read last word
first; `rows_word` masks the word index to 15 bits, the byte wrap in words.
`the_unchecked_read_covers_everything_the_arithmetic_can_reach` recomputes the reachable ±680 from
the data files (`by` within ±433, `pcalc`'s y within ±247) and asserts the modelled window covers
it, so the unmodelled scalars between `sade` and `pcalc` are unreachable by arithmetic.

**It is invisible.** `the_out_of_range_read_is_invisible_and_that_is_measured` walks the whole
1,061-tick run: more than 100,000 out-of-range reads, and every word read is 64,000 (the values
that occur are 201..248 forward and the wrapped side, which read `aa2`'s copy of `aa1` or `aa1`).
Such a dot plots at bytes 64,002-64,319, past the 64,000 a 200-line screen shows, so a bounded read
draws byte-identical frames.

**Zeroing it would be visible.** A port that substituted 0 would plot every such dot at its x sum,
which is already ≤ 319, so in row 0: `zeroing_the_read_would_paint_row_zero` counts **356,042** such
reads over the run, against a row the reference's four frames ink **34** pixels of (the port's own
frames, 32). That is why the read is modelled rather than zeroed (`bugs.md` §22).

## 5. The palette, the build that shipped, entry 0, and the self-modified colour

`:121-128` writes entries 64..192 and 255. The first ramp (64..128, `(64 - x)`) is the source's and
the reference agrees entry for entry. **The second ramp is not the source's** (D12, A6):

- `TUN10.PAS:122` says `(64-x) *3 div 4`, and the tree's `TUN10.EXE` compiles exactly that (image
  `0x1d0`-`0x1e3`: `mov ax,64 / sub ax,[x] / mov si,ax / shl ax,1 / add ax,si / cwd / mov cx,4 /
  idiv cx`, three times, the entry pushed as `128 + x` at `0x1cc`).
- The reference is drawn with `(64 - x) * 3 div 5`. **Exact:** the four frames exercise 42 of the
  ramp's entries (ring `x` has `k = round(x/1.3)`, entry `128 + k`), 38 with a non-black pixel, over
  5,275 pixels and 29 greys, and every one matches `3*(64-k) div 5`, including the 17 where rounding
  would differ. **Unique:** `the_second_ramp_is_derived_by_matching_the_reference_bit_for_bit`
  renders the four aligned ticks through every `(mul, div)` with `mul ≤ 7`, `div ≤ 15` (81
  candidates, 70 distinct ramps) and counts the port's pixels that no reference frame shows in that
  colour: **1,108 at 3/5**, at least 4,200 elsewhere (7/12 is 4,200; the source's 3/4 is 6,376). Of
  the 1,108, 1,091 are port ink where the reference shows background (§7.2's presence residual)
  and 17 are dot overlap, 10 on first-ramp entries (72, 75, 76, 83, 89) no second-ramp value
  reaches.
- **The port takes the shipped constant** under D12: the release is the reference of record for
  frames, the tree for how the effect works. For TUNNELI, `MAIN/DATA/`'s build is not the tree's
  `TUN10.EXE`.

`Palette::set_entry` masks to six bits, so the first ramp's `x = 0` writes 64 into entry 64 as
`(0,0,0)`; no dot reaches entry 64 (the lowest drawn colour is 67)
(`the_palette_is_the_parts_own_ramps_and_leaves_0_to_63_alone`).

`waitr` writes **entry 0** every frame (`:46`, `:52`; `TUN10.EXE` image `0x2a`-`0x6d`): black, or
`(0,0,15)` while Scroll Lock (`$40:$17` bit 4) is set. The port writes the black; every non-dot
pixel of the four reference frames is exactly `(0,0,0)`, so the captured session had Scroll Lock
off.

The colour immediate patched at `:168` (`mov byte ptr cs:[@c+3],al`, plot at `:187`) is a variable,
as `docs/design.md` §5.2 asks of self-modifying code; `bbc` keeps the byte's arithmetic (`:158`).

## 6. What is not modelled

- **`dis_exit`** (`:223`) is the engine's exit request, acted on by the shell.
- **The vertical-blank rate**: the port's tick is one frame, and §7.2's alignment holds to the tick
  over 878 ticks.
- **The Scroll Lock branch** (§5).
- **CRTC timing registers**: `:110-115` is a BIOS mode set; the port models its geometry,
  addressing and clear.

## 7. Verification

### 7.1 At the reference's own indices

A `--part tunneli` run is past its end (tick 1,060) at 3800-4600, so `compare` there measures only
the numbering's gap: every differing pixel is a reference dot against the held black. The port is
measured at the alignment the reference supplies.

### 7.2 The alignment, and the residual

The port's ticks **78, 478, 778 and 878** are the reference's **3800, 4200, 4500 and 4600**: offset
**3,722** in all four, the only alignment that puts the reference's own adjacent-index motion on the
right side of the difference. The reference's first ring is at boot 3710 and tick 0 answers 3,722,
inside the window the part's start moves in between sessions (`reference-indices.md` §8.5).
`cargo xtask compare --align <N>` reads the rendered indices `N` higher for pairing
(`xtask/tests/compare.rs` pins it both ways).

| reference | port tick | differing px | % | port's own motion per frame | census (`reference-indices.md` §1.8) | register |
|---|---|---|---|---|---|---|
| 3800 | 78 | 179 | 0.28 % | 1,180 / 1,361 | 7/9 sessions, settled | A6 |
| 4200 | 478 | 373 | 0.58 % | 7,000 / 7,445 | 3/6, the reference the largest group | A6 |
| 4500 | 778 | 321 | 0.50 % | 4,070 / 4,063 | 2/5, no settled content | A6 |
| 4600 | 878 | 433 | 0.68 % | 6,536 / 6,636 | 1/6, no settled content | A6 |

Max channel deltas are 12, 239, 231 and 239. `cargo xtask rediff` (entry `tunneli`) registers the
four counts. In block 1 TUNNELI starts 7 frames early (`FULLRUN_SEAMS`: 11,151 against 11,158), and
`fullrun/04-glenz-techno`'s 11159 and 11900 differ by 192 and 4,336 (A6, A53).

**The residual is presence, not colour.** With the shipped ramp the colour field matches on every
pixel both sides ink except 17 at 4500 (dot overlap, §5). The rest is port ink against reference
background and vice versa (179/0 at 3800, 347/26 at 4200). It is not a whole-tick error: at 3800
tick 78 leaves 179 pixels, 77 leaves 754 and 79 leaves 953 (`the_alignment_is_whole_tick_or_nothing`),
and the original advances one vblank at a time. The re-capture reproduced 3800, 4200 and 4500 byte
for byte and 4600 to 57 pixels (`reference-indices.md` §8.4), so at 3800 and 4200 the residual is
not the oracle's spread; it is the port's own geometry or a sub-frame difference in it, unidentified.
At 4500 and 4600 no session reproduces the picture, so the residual is unattributable.

**The sheets.** `cargo xtask frames sheet` lays out the two sides at the same scale: the mouth
(3800 / tick 78), the full tunnel with its centre up-left (4200 / 478), the off-centre tunnel, mouth
top right (4500 / 778), and the thinner one, mouth low right (4600 / 878). The reference's sheet is
byte-identical to the one `reference capture --sheet` writes.

`tests/tunneli.rs` checks the run's own numbering (`index_origin: part`, `anchor: null`) and the
retirement and hold (tick 1,040 has dots, 1,060 is black, held at 1,061 and 1,200).

## 8. Reproduce

```bash
cargo test -p secondreality --bin secondreality parts::tunneli
cargo test -p secondreality --test tunneli
cargo xtask rediff --only tunneli
cargo run --release --bin secondreality -- --headless --part tunneli \
    --dump-frames 78,478,778,878 --dump-dir out/tunneli-aligned
cargo xtask compare --reference reference/parts/tunneli --rendered out/tunneli-aligned \
    --out out/tunneli-aligned-diff --align 3722          # §7.2's table; exit 1
cargo xtask frames sheet --dir out/tunneli-aligned --out out/tunneli-aligned-sheet.png
cargo xtask frames sheet --dir reference/parts/tunneli --out out/tunneli-reference-sheet.png
```

## 9. Open items

- The 179 / 373 pixels at 3800 / 4200 are unattributed (A6), and the release's `TUNNELI.EXE`
  image (D49) is not yet read for them.

**Register.** `bugs.md` §22 (the unchecked read past `rows`).
