# BEGLOGO — the Future Crew logo, the white, and the 129-step fade

Slot 4 of `part-sequence.md` §1: `SECOND 2`'s first part, built from `BEG/`. 320×400 Mode X,
128,000 pixels of output, and **269 lines of source in three files** — `BEG/BEG.C` (87),
`BEG/ASM.ASM` (118) and `BEG/READP.C` (64). The reference is `reference/parts/beglogo/` — 430, 470,
560, 900 (`reference-indices.md` §3).

**The acceptance comparison is exit 0**: `mode: mixed`, four frames at offset 0, 0.0000 % differing,
0 pixels of 128,000, max channel delta 0 (§7). This is Phase 2's exit criterion.

The port is `bins/secondreality/src/parts/beglogo.rs`, registered as `beglogo`.

## 1. Which build shipped: `BEG/`, and it took three measurements to say so

`ENDPIC/MAKEFILE:22` also stages a `beg.exe` as `beglogo.exe`, so two directories build part 4, and
`part-sequence.md` §1 and `asset-inventory.md` item 10 both left it open. It matters beyond the
picture: `ENDPIC/BEG.C` sets its own video mode (`:18-20`) and **waits on the music**
(`dis_muscode(0xf0)`, `:45-49`), while `BEG/BEG.C` calls `dis_partstart`, `dis_waitb` and nothing
else.

Three measurements settle it, and none needs the packed binary unpacked:

1. **The held frame's palette.** Frame 900 is past the end of the fade, so the DAC in force is the
   picture's own palette, and `reference/parts/beglogo/manifest.json` records the 57 entries that
   frame's indices probe. Against those 57: `BEG/SRTITLE.UP`'s palette (bytes 16..784, copied
   verbatim by `readp`, `BEG/READP.C:17`) is **0 of 57 different**; `ENDPIC/SRTITLE.U`'s is
   **57 of 57 different** — its entry 1 is `(1,2,3)` where the reference has `(63,63,63)`.
2. **The held frame's picture.** Decoding `BEG/SRTITLE.UP` and comparing with
   `frame_000900.idx` gives **0 of 128,000 pixels** different — the strongest statement available
   that the port's decoder and the art agree. `ENDPIC/SRTITLE.U` decodes to a 320×400 picture too,
   and it is a *different* picture: 126,261 of 128,000 pixels differ.
3. **The staged EXE.** `MAIN/DATA/BEGLOGO.EXE` is 30,874 bytes and carries PKLITE's signature
   (`part-sequence.md` §1). `BEG/BEG.EXE` is 50,830 bytes and `ENDPIC/BEG.EXE` 13,286, so the
   staged size is a compressed 50,830 and not a compressed 13,286 — no packer grows a binary by
   2.3×, and packing these two is the only way to make the staged file at all.

**`BEG/` shipped.** Consequences the port takes: the picture is `BEG/SRTITLE.UP`, the palette is the
one inside it, the screen opens white (`BEG/BEG.C:20-21`), and **BEGLOGO has no music gate** — so
`_np_zinfo`'s exposure narrows to GRID, HARD and the vault (Phase 8), and spec §7.4, the risk table
and `docs/decisions.md` D8 are amended to say so.

## 2. What the original does

```
:17      dis_partstart()                     the cue the shell's parts synchronise on
:18-19   outp(0x3c4,2); outp(0x3c5,15)       sequencer map mask 0x0f: chain-4 off
:20-21   memset(vram,15,32768) ×2            64 KB of A-space filled with index 15
:24      for(a=0;a<32;a++) dis_waitb()       32 vertical blanks
:25-31   outp(0x3c8,0); 255× (63,63,63)      DAC entries 0-254 white
:32-34   outp(0x3c9,0) ×3                    entry 255 black
:35-71   AC 0x11, CRTC 0x0c/0x0d and 9, AC 0x11 again
:73      readp(palette,-1,pic)               the palette out of the container
:74-78   readp(rowbuf,y,pic); lineblit(...)  400 rows, one picture row per video row
:80-85   for(c=0;c<=128;c++) { pal2 = ramp(c); dis_waitb(); setpalarea(pal2,0,254); }
:86      setpalarea(palette,0,254)           the picture's own palette
:87      }                                   main() returns
```

The screen never moves after `:78`; everything from 446 to 575 is the DAC. The code's own helpers:

- **`lineblit`** (`BEG/ASM.ASM:53-82`, the four-plane loop at `:63-76`) writes one row through the
  planes: for each plane it copies 80 bytes with `es:[di+zzz]` from `ds:[si+(zzz)*4+zpl]`, i.e.
  every fourth byte of the row buffer into one plane. Four planes × 80 bytes = 320 pixels, at plane
  offset `y*80`.
- **`setpalarea`** (`:93-115`) is the ranged DAC upload, and it ships with a byte-count bug the port
  preserves: §6.
- **`readp`** (`BEG/READP.C`) is the Origin RLE row reader: `row = -1` copies the palette, a row
  walks the row-offset table and expands one run-length stream. `sr_assets`' Origin decoder reads
  the same container in one call (§4).

## 3. The fade's first index is 446, by matching the reference bit for bit

`FADE_FIRST_INDEX` is the reference index the fade's step `c = 0` is displayed on. It is **not** a
fit: `the_fade_constants_are_derived_by_matching_the_reference_bit_for_bit` (`src/parts/beglogo.rs`)
runs the source's own arithmetic over the whole range `c = 0…128`, widens the DAC with the engine's
own rule `(c*259+33)>>6`, renders all 320×400 pixels of the picture through the result, and asks
which `c` reproduces each moving keyframe **exactly**:

| frame | `c` values whose 320×400 RGB is that frame, byte for byte |
|---|---|
| 470 | **[24]** — unique over 0…128 |
| 560 | **[114]** — unique over 0…128 |
| 900 | [126, 127, 128] — the ramp has converged; the three are one palette |
| 430 | all 129 — the frame is the white field, one index, before the ramp |

470 − 24 = **446** and 560 − 114 = **446**: two keyframes 90 frames apart, each unique, both landing
on the same first index. That agreement is the derivation; the constant is the number they agree on.

**Why a fit gives a different answer, and why that answer is wrong.** `reference-indices.md` §3.3
records, for each sweep frame, the mean difference from the held picture: 23.815 at 440 down to
0.156 at 570. Fitting a line through those means puts the white screen's last frame near 441. A mean
is not linear in `c` — the mean of a truncated integer ramp is a staircase, not a line — and a
five-frame fit cannot tell a whole step from the truncation inside one. The scan above can, because
it compares every pixel instead of averaging them: it separates `c = 24` from `c = 23` and `c = 25`
where the sampled means cannot. This is plan Review Focus 5's failure mode, and the reason the
constant's comment names the derivation rather than the number.

**The two residues, stated rather than smoothed over.**

- **900 does not pin the constant.** `c = 126, 127, 128` are three names for a converged palette, so
  the held frame can only say the ramp *ended* where it did. It cannot say where it began, and it is
  not asked to.
- **430 does not pin the constant either**, and this is worth stating because the obvious reading —
  "430 is `c = 0`, so it is the anchor" — is a coincidence rather than a measurement. The reference's
  430 is one index (15) repeated 128,000 times, and entry 15 is `(63,63,63)` in the picture's own
  palette, so *every* `c` renders that frame as white. What 430 pins is the **surface**: the frame is
  the white field, and no picture is in video memory yet (§5).

`FADE_FIRST_TICK` = 446 − 404 = **42** is the same constant in the part's own tick numbering: a run
starting at BEGLOGO is anchored at the demo's first graphics frame (`shell::FIRST_GRAPHICS_FRAME` =
404 = `shell::boot_anchor("beglogo")`), and `PartCtx::frame` counts the part's ticks from there.
**33 of those 42 ticks are the source's** — 32 vertical blanks (`:24`) and the tick `:25-78` runs on
— and **the other nine are frames the port does not model, with their cause unidentified**
(`docs/fidelity/APPROXIMATIONS.md` A5). The honest statement of the residual matters more here than
a plausible one, and two plausible ones are ruled out by evidence already in the tree: it is **not**
the EXE load (`reference-indices.md` §3.3 files 410, 429, 430 and 431 as one `sha256`, byte-identical
white — and `:20-21`'s memsets are what make that screen white, so the part's own code was running by
410), and it is **not** `dis_partstart()` (`:17`), which is a version check that returns immediately
(`DIS/DISC.ASM:51-64`) and costs no frame at all. The port draws the white field through all 42
ticks, which is what the reference shows at every index inside the window that it holds; 446 itself
is measured from the frames and does not depend on the split.

## 4. What the port does, act by act

**`init` does `:18-21` and `:40-78`'s read.** The surface is cleared to 15 — one `clear`, where the
original needs two `memset`s and a chain-4 write, because the port's surface is chunky: the plane
interleave is a hardware detail the chunky model folds away, and 64 KB of plane memory is the whole
screen either way. The view becomes 320×400 (`:42-53`: display start 0, CRTC 9's double-scan bit and
max-scan-line field cleared), with `y_line_offset` written unset (§5) — Part B's first convention: a part
whose picture *is* the display writes through `Surface` at the surface's own stride. The border is
AC 0x11 = 255 (`:66-68`), which is an entry number: `:32-34` is what makes entry 255 black, and it is
what makes this part's overscan black. The picture is read with
`sr_assets::load_path("assets/BEG/SRTITLE.UP")` — one call for the container `readp` reads row by row,
decoding to the same 128,000 pixels (§1), and refused if the geometry is not 320×400.

**The blit is not in `init`, and this is the one place the plan's sketch was wrong.** `:24`'s 32
vertical blanks come *before* `:73-78`, so the part's first 32 ticks hold the white field and the
blit runs on the tick after them (`LEAD_IN_TICKS` — tick 32, reference index 436). Two things say so
and they agree:

- **The source.** `memset` at `:20-21`, `dis_waitb()` ×32 at `:24`, then `:73-78`. The picture cannot
  be in video memory before the waits end.
- **The reference, and this one is load-bearing.** `frame_000430.idx` is 128,000 bytes of 15, and
  `compare` **prefers the indexed path** whenever both sides have indices — which they do, because
  the port dumps `.idx` for every frame. A port that blitted in `init` would render 430 white (entry
  15 is white, so the RGB matches at every `c`) while its `.idx` held the *picture's* indices, and
  the comparison would report 430 as a diff. The frame is answerable as indices only if video memory
  really is the white field at 430.

**The DAC write is on that same tick, not in `init`.** `:25-34` is after `:24`, so the port's first
32 ticks run over the DAC the shell installed — `hardware_default_palette`'s entry 15 is white, which
is *the* reason 430 is white, and entries the part never writes are still the shell's at 430. The
part writes 255 triples of `(63,63,63)` from index 0 and one `(0,0,0)` at 255 (`install_white_dac`).
This is also why the part never resets the DAC: a part that cleared it would render 430 black and
fail a keyframe before doing anything (plan Review Focus 3).

**`frame` is the fade.** One `c` step per tick from tick 42: `pal2 = ramp(c)`, then the upload. `c =
128` is the picture's palette exactly, and the tick after it re-uploads that palette with the
original's `:86` and returns `PartStatus::Finished` — `:87` closes `main()`. Everything the screen
does afterwards belongs to the loader and GLENZ's music wait (`reference-indices.md` §3.5), which is
what `Sequence`'s hold models; the reference's 900 is that hold, and the port answers it with the
picture the part left.

The tick map, in the reference's own numbering:

| index | tick | what the screen is |
|---|---|---|
| 404-435 | 0-31 | the white field, through the shell's DAC (`:20-24`) |
| 436 | 32 | the white DAC and the picture blitted (`:25-78`) — still white |
| 437-445 | 33-41 | white; nine frames the port does not model (§3, A5) |
| 446-574 | 42-170 | the ramp, `c = index − 446` (`:80-85`) |
| 575 | 171 | the picture's palette once more (`:86`), and the part ends |
| 576-900 | — | the hold: the loader and GLENZ's wait (`:87`) |

**What is not modelled, and why it cannot be.** The CRTC's horizontal and timing registers are not
written by `BEG.C` at all, and have no analogue in an engine that composes a rectangle rather than
scanning one out. The other thing `BEG.C` writes that the port folds is the attribute controller's
*index* register: each of `:36-38`, `:54-60` and `:64-70` ends by writing `0x20` to `0x3c0`
(`outp(0x3c0,0x20)`, `:59-60` and `:69-70`), which is the index register set to 0 with bit 5 — the
palette address source — set. PAS = 1 is the state in which the DAC's palette is displayed at all,
and the port's engine always displays it, so there is nothing there to set. The overscan (AC 0x11,
index 0x11) is written three times: 255 (`:37`), 0 (`:57`) and 255 again (`:67`), so the last
write wins, and that is what the port states once (`border = Some(255)`). The only thing the port
knowingly leaves out is *timing*, and it is registered rather than described away:
**`APPROXIMATIONS.md` A5** carries the nine unmodelled frames of the lead-in, what is ruled out as
their cause, and what a later phase should watch. Nothing else is owed: where the fold is
measurable at all it is zero, because the reference's own 410, 429, 430 and 431 are byte-identical
white frames (`reference-indices.md` §3.3) and the port is white at all of them.

## 5. The two conventions, stated for the next part

**The picture is written through `Surface::set` at the surface's own stride, `y_line_offset` unset.**
`lineblit` writes plane bytes; the chunky model has no planes, so the port writes the picture's 320
pixels of each row at the row's own place in the 640-wide surface, and the view — 320 wide, no line
offset — reads back exactly that half. Setting `y_line_offset` to 320 here would shear the picture
against itself, with no error anywhere (CLAUDE.md's caveat; Part B's first convention).

**And so `init` sets it unset, rather than leaving it** (Phase 9 Task 6). `BEG.C` writes CRTC 0x13
nowhere: it inherits the line offset the part before it left, and in the demo that is always the
320-pixel tweak's 80 bytes a row — the stride `lineblit(vram+(unsigned)y*80U,rowbuf)` draws at — so
the original reads its picture row by row. In the port, block 1's PAM leaves `Some(320)`, which on
the 640-wide surface read every second scanline from the picture's right margin: block 1's lead-in
and fade were the picture's top half, doubled, with black lines between, until the full run's
comparison found it (`reference-indices.md` §26.8). `init` now writes `y_line_offset = None`, which
is what a run that starts here already had, so no count of this note moved; block 1's 7930 is the
original's 7930 byte for byte (`cargo test --release -p secondreality --test fullrun`),
and `the_picture_is_read_at_its_own_row_stride_whatever_the_part_before_left` pins the write.

**The upload is byte-exact, not entry-exact.** §6.

## 6. The bug the part ships: `setpalarea`'s byte count

`BEG/BEG.C:84`'s `setpalarea(pal2,0,254)` is `BEG/ASM.ASM:93-115`, and `:106-108` computes the
transfer length as `2*count + start`, not `3*count` — `mov cx,[bp+12]`, `shl cx,1`, `add cx,ax` (the
`start` the DAC's write index was already pointed at, `:104-105`). So the fade uploads **508** bytes
where it intends 762: entries 0-168 whole, the red byte of entry 169, and nothing at all of entries
170-254. Those keep the white `:25-31` left them, for the whole part, and entry 255 keeps its black.

The shipped binary contains exactly this. Disassembling `BEG/BEG.EXE`: `_setpalarea` begins at file
offset `0xcb7` (load `0xab7`, after the `0x200`-byte header), and the three length instructions are
at `0xccb`-`0x0cd1`, followed by the `mov al,ds:[si] / inc si / out dx,al / dec cx / jnz` loop at
`0xcd2`. `END/ASM.ASM`, `JPLOGO/ASM.ASM`, `ENDPIC/ASM.ASM` and `TECHNO/KOEA.ASM` carry the same
`2*count + start`; `LENS/ASM.ASM:253-255`, `START/ASM.ASM:192-194`, `COMAN/ASM.ASM:93-95` and
`TWIST/ASM.ASM:85-87` do the correct `3*count` by loading `count` into `ax` before doubling.

**It is invisible on this art, and that is exactly why it is written down.** The picture indexes 1,
2, 3, 14-58 and 71-79 and the white field is 15 — every index the screen uses is inside the 169
entries that *are* written — so a port that uploaded the full 762 bytes answers all four keyframes
identically. The derivation test asserts precisely that: the scan of §3, rerun with a 762-byte
upload, returns the same `c` for all three moving frames. The port writes the 508 because the
original does, and `docs/fidelity/bugs.md` §4 carries the entry that says which part contracts it
next: entries 170-254 go into GLENZ still white, and the first part to paint there will meet a DAC
that never faded.

## 7. The comparison

```bash
cargo run --bin secondreality -- --headless --part beglogo --dump-frames 430,470,560,900 \
    --dump-dir out/p2-beglogo
cargo xtask compare --reference reference/parts/beglogo --rendered out/p2-beglogo \
    --out out/p2-beglogo-diff
```

```
frame 430: 0.0000% differing (0/128000 px), max delta 0
frame 470: 0.0000% differing (0/128000 px), max delta 0
frame 560: 0.0000% differing (0/128000 px), max delta 0
frame 900: 0.0000% differing (0/128000 px), max delta 0
verdict: identical
exit: 0
```

`mode: mixed` and every offset 0 (`out/p2-beglogo-diff/report.json`). 430 and 900 go through the
indexed path — they are held screens and their indices are the surface the part left — and 470 and
560 through RGB, because the indexed read is late by about twenty fade steps
(`reference-indices.md` §1.3, D15). The DAC's 6→8-bit widening `(c*259+33)>>6` is therefore
load-bearing on the middle pair and bypassed on the outer pair, exactly as §3.1 predicted: 470 and
560 are the frames that would have shown a widening that differed, and they show none.

## 8. Tests

`src/parts/beglogo.rs` (unit, six): the opening white over the shell's DAC and the marker entry the
part leaves alone; the white DAC's 255 + 1 writes; the ramp's truncating divide (including the
`c = 1, palette = 0` case where it lands on 62, one below a rounded ramp); the 508-byte upload's
exact edges; the blit at tick 32 and the still-white first fade step; and the derivation scan of §3.

`tests/beglogo.rs` (five, through the real binary from the repository root): the four keyframes in
the reference's own numbering (`index_origin: boot`, anchor 404, 320×400 each); 430 as 128,000 bytes
of 15 and pure white RGB; 470 and 560 byte-for-byte against `reference/parts/beglogo`'s own RGB
frames, the pair that would show the DAC's 6→8-bit widening if it differed; the held 900 equal to
`assets/BEG/SRTITLE.UP` as decoded; and the guard that makes those comparisons able to fail at all.

What the five add over the unit tests is **the path a run really takes**, end to end: the registry,
the shell's anchor and numbering, the asset read at `assets/BEG/SRTITLE.UP` from the repository root,
the DAC the shell installed, and the frames the dump wrote to disk and read back — where the unit
tests drive the part's arithmetic and its screen directly. The one reference *file* the suite does
not compare byte-for-byte is `frame_000430.idx`: 430's content is pinned as index 15 and one palette
entry, from the frame's own record, rather than by reading those 128,000 bytes.

The fifth test is a guard with a guard: `differing_bytes` asserts both frames are the same length
before it counts anything, because `iter().zip()` stops at the shorter side and a *truncated* dump is
the one failure a comparison test exists to catch. `a_truncated_frame_is_refused_rather_than_counted`
proves that assertion fires, rather than describing it.

**Register.** The behaviours of this part that the original does and the port must keep are in `bugs.md` §4 (`setpalarea`'s byte count).
