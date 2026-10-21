# GRID — the line rider

GRID is one of the three unreleased development effects in the vault: a 320×200 mode-13h
program (`GRID/P.EXE`) that draws one self-intersecting parametric line of 1,000 segments
over the `EYE4.UH` eye, with three successive lines alive in three XOR colours. The port
is `bins/secondreality/src/parts/vault/grid.rs`; it is silent, unanchored, and reached by
`--part grid` (and, from Task 8, the vault menu). Its reference is `reference/parts/grid`.

## Capture (Phase 8 Task 2)

The reference is `reference/parts/grid`, a version-1 capture of the vendored `GRID/P.EXE`
(`d4e7f1d6e1b5e5c3ba3fd85fe8f3645c1c99be1f0d200c791968c656b5334ff0`) run behind the
standalone `DIS/DIS.EXE`, staged and mounted read-only by `tools/vault_capture.sh`, with one
key (`KBD_space`) to pass `getch()`:

```bash
tools/vault_capture.sh grid --frames 400,500,600,700,800
```

The manifest says `exe: "RUN.BAT"`, `arguments: []`, `boot: "autoexec"`,
`boot_wait: "graphics"`, `cycles: 30000`, `frame_origin: "boot"`, `skip_boot_frames: 0`,
320×200, `keys: ["KBD_space"]`, oracle `dosbox-automation 0.85.1 (6d100)`, upstream
`071a82e`. The batch is the script's CRLF `RUN.BAT`: `@ECHO OFF`, `DIS.EXE`, `P.EXE`.

### Start and `K0`

A dense sweep re-ran Task 1's measurement and reproduced it byte for byte
(`out/task2/grid-start`): 340–351 are the static pre-key dot screen `728e0d87…`, **352 is
the start-up transition frame** `d2ac61b7…`, and **353** is
`4a4ad86e2b918693179a5196a6de02a1e5ac813af0113151ff6466e8368dccbf`. Route B's 704 is the
same bytes as route A's 353; its 703 is the same kind of transition as 352. The port's frame
0 is its first composed loop frame, so **`K0` = 353, tolerance ±1** — the anchor is on the
first stable loop frame, not on the first changed (352). Effect frame `j = index − 353`; the
palette plane advances one step per effect frame in a 3-frame cycle (`research/grid.md` §1.2,
which numbers the same pictures one higher — its `j = 1` is reference 353).

### Keyframes

| index | effect `j` | what is on screen | `sha256` |
|---|---|---|---|
| 400 | 47 | line-work over the eye | `574a6fa980799b212f2eed29cd44ba182fce6fd994768896f6b3cca476d170ff` |
| 500 | 147 | line-work over the eye, other phase | `0241f663653f5266ca44c1563a2373c4f7f805f1ad2b8fbf40c661dbc65069b7` |
| 600 | 247 | line-work over the eye | `1fb437409a1881cd9e0fdb7e467926c33e0e77b3d0f3383bf399e7ccb23450d6` |
| 700 | 347 | line-work over the eye | `1969cf9a10c20744504ebd24a7e60d239bb7f7791de562fb613bf9cbbfa5912b` |
| 800 | 447 | line-work over the eye | `869aeee7b49dbb8e32b736d14bf419a28e3011533e6bfebf91024fac8e91aa68` |

100 apart, covering all three palette planes. All five are **RGB-only**: the moving screen
fails the indexed read's coherence check and the tool keeps the RGB frame (D15/D19), so there
is no `.idx` in this capture. The comparison test reads the `.rgb` and would compare an
`.idx` if a later capture carried one.

### Reproduced

Frame 600 was captured again in a second session (`out/task2/grid-repeat-600`) and is
byte-identical to the filed `1fb43740…`; the filed 400/500/600 also equal Task 1's smoke
captures from a different session, so three sessions agree. `cargo xtask reference verify
--tree reference` passes over the capture.

## 1. The artifact and the picture

`GRID/P.EXE` is `MAIN.C` (Microsoft C, large model, 16-bit `int`s) linked with `ASM.ASM`,
the generated `LINEBLIT.INC`, the `_EYE.OBK` blob of `EYE4.UH`, and the DIS client stub
(`GRID/MAKEFILE:20-23`, `GRID/LINK.TMP:1-2`). `GRID/DOLINE.EXE` is the line-blit
generator, not the effect (`DOLINE.C:98`); the earlier `MAIN.OK`/`ASM.OK`/`DOLINE.OK`
snapshots are not linked.

The static background is the 320×200 eye, shifted (below). The moving part is one
parametric curve of 1,000 segments through 1,001 points:

```
x_k = sinus1[(i + k) & 1023] + 160
y_k = sinus2[k]            + 100        (k = 0..=1000)
```

for the source loop's iteration `i` (`MAIN.C:161-171`). Each segment is drawn by a
generated Bresenham routine that XORs its colour into video memory and the background
buffer; three successive iterations' curves are on screen at once, each in its own colour
(1, 2 or 4), and each lives three frames. A pixel visited twice by **one** curve cancels
(XOR of the same colour); a pixel visited by two **different** curves stays lit — the
visible mask is the union of the three self-XOR line masks, not an XOR of masks.

The developer's 1,024-point dot screen before `getch()` (`MAIN.C:144-153`) is not part of
the animation: the eye copy at `:156` overwrites it. It is the measurement that pins `d0`
(§2), and the port does not draw it.

## 2. The sine tables, and the measured uninitialised `d`

`MAIN.C:44-49` builds `sin1` as `(int)(sin(((a * pi) * 2) / 1024) * 256)` — the compiled
expression's left-to-right order, with the double-to-int conversion truncating toward
zero; `pi` is exactly `3.141592653589`. `sinus1` (`:51-58`) runs three 16-bit cursors
`b += 97`, `c += 87`, `d += 67` and stores

```
sinus1[a] = (sin1[(b>>3)&1023] + 2*sin1[(c>>3)&1023] + sin1[(d>>3)&1023]) / 8
```

with the adds wrapping at 16 bits, the index mask dropping the sign extension, and `idiv`
truncating toward zero. `sinus2[a] = sinus1[(a+277)&1023] * 7 / 10` (`:59-62`), likewise
truncating.

**`d` is never initialised** (`:33`; only `b` and `c` are zeroed at `:51`). The compiled
object's own frame predicts `0x116`, but the shipped `P.EXE` does something else: an
exhaustive match over every `d0` in 0..65535 against the capture's 982-pixel static dot
screen (`out/task2/grid-start`'s 340 frame) found only the class `0x7FE (mod 0x2000)` matching
all 982, while `0x116` matches 47 (D24 — the binary is the artifact, not the object). The
scan was run in Task 3 and `research/grid.md` §2.1 is git-ignored scratch; the durable record
is the committed tests: `the_measured_d_is_the_only_residue_whose_tables_match` scans the
0x2000 residues over the whole 16-bit word (adding 0x2000 adds 1,024 to `d >> 3`, which the
`& 1023` index discards) and finds only `0x07FE`, and `the_initial_d_is_the_measured_0x7fe`
pins the 982-pixel set, its bbox and the object's 1,004-pixel/47-shared disagreement. The
minimal representative is **`d0 = 0x07FE`**.

The tests pin the result:
`parts::vault::grid::tests::the_sine_tables_are_the_sources_own` (the first values,
`sinus1 ∈ [−123, 120]`, `sinus2 ∈ [−86, 84]`, the `−45 → 37` seam, an 82 jump),
`the_sinus_tables_use_the_sources_wrapping_arithmetic` (16-bit cursor wrap, truncating
`/8` and `/10`: `sinus1[42] = −3`, `sinus2[34] = 0`), and
`the_initial_d_is_the_measured_0x7fe` (982 unique pixels of the 1,024-point plot, bbox
x 37..280 × y 14..184, the class `0x27FE`/`0x47FE` identical; `0x116` gives 1,004 pixels
and 47 of them shared).

`sin256` (`MAIN.C:38-43`) and the 64 KB `muldata` (`:130-139`) are built by `MAIN.C` but
read by nothing linked into `P.EXE` — `ASM.ASM`/`LINEBLIT.INC` contain no `muldata` — so
they are dead and not ported. `_asmloop2` (`ASM.ASM:113-134`), the unused second slot
words and `_line` (`:247-269`) are dead too.

## 3. The eye, the shift bound, and the 100 unshifted bytes

`MAIN.C:141` shifts the image in place:

```c
for(u=784;u<64684;u++) eye[u]<<=3;
```

The image is `eye[784 .. 64783]` (64,000 bytes), so the loop stops **100 bytes short**:
the first 63,900 image bytes become `(v << 3) & 0xff`, and the last 100 — row 199,
x = 220..319 — stay raw. Those pixels therefore keep palette indices 24..28 and display
the eye palette's cyan group 3 under the line tints instead of the shifted grey ramp. Both
`bufdata` and VRAM start as the same picture (`:142`, `:156`). This bug is required
behaviour, not a typo to fix.

The port loads the file through `sr_assets::load_path` (the Origin `Uh` decoder: palette at
file offset 16, pixels at 784, 320×200) and applies the same bound. The palette is the
file's 768 six-bit bytes verbatim. Pinned by
`the_eye_shift_stops_100_bytes_short` (every one of the 64,000 bytes against the file, the
63,900/100 boundary, the palette bytes) and
`the_buffers_start_as_the_same_shifted_eye` (both 65,536-byte buffers carry the picture in
their first 64,000 bytes and stay in lockstep through every draw).

## 4. The three palettes

`MAIN.C:95-126` builds three 768-byte palettes from **eye palette entries 0..31 only**:

```
base_rgb = eye[16 + (x>>3)*3 + {0,1,2}] / 4        // six bits, 0..15
pal[0]: red += (x&1 ? 40:0) + (x&4 ? 20:0) + (x&2 ? 10:0)
pal[1]: red += (x&2 ? 40:0) + (x&1 ? 20:0) + (x&4 ? 10:0)
pal[2]: red += (x&4 ? 40:0) + (x&2 ? 20:0) + (x&1 ? 10:0)
red = min(red, 63);  green = blue = base                  // never above 15
```

Each group of eight indices shares a base colour; the low three bits choose a red rank.
Entries with `x & 7 == 0` are therefore identical in all three palettes, which is why the
shifted background (multiples of 8) never changes hue with the cycle — only the curve
colours and the unshifted tail do. The clamp matters for overlapping curves.

Pinned by `the_three_palettes_are_the_sources_own`: the group-0 identity in all three, the
per-palette rank of bits 0/2/1, the `x&7==7` clamp at 63, and green/blue being the base and
never above 15.

## 5. The generated line blit

`DOLINE.C:13-59` is a standard Bresenham with two source choices that are part of the
picture:

- **The endpoint is not plotted.** The loop plots the start and every intermediate pixel
  and stops at the endpoint (`:36-59`); the generator marks the endpoint with 1 and emits
  only the 2s (`:64-85`). The next segment's start is this endpoint, so the polyline stays
  contiguous without a double-drawn joint.
- **The branch is `xd > yd` with `d = -xd/2`** (C truncation toward zero), otherwise the
  y-major `d = -yd/2`.

The emitter writes `mov al,gs:[di+off]; xor al,dl; mov gs:[di+off],al; mov es:[di+off],al`
(or a 16-bit pair when two adjacent pixels are set), so every pixel is the colour XORed
into **both** `bufdata` and VRAM, at the 16-bit address `di + x + y*320`. The research
regenerated `LINEBLIT.INC` byte-identically from this algorithm after CRLF normalisation
(`research/grid.md` §3.4, scratch). The durable check is committed:
`the_line_blit_is_the_committed_generated_code` reads
`upstream/SecondReality/GRID/LINEBLIT.INC`, pairs its 357 routines with the generator's
y = −8..=8, x = −10..=10 order, and asserts that every routine's 16-bit `di` stores are
exactly the pixels the port's blit XORs for that delta; the older
`the_line_blit_is_the_generated_xor_bresenham` is the independent `DOLINE.C` translation.

`asmloop1` selects a routine by the **low byte** of the two deltas:
`(bh<<8 | bl)` with `bl = X2−X1`, `bh = Y2−Y1` (`ASM.ASM:189-192`). The 357 filled entries
cover dx ∈ −10..=10, dy ∈ −8..=8. Every other entry of the 64 KB dispatch table was filled
by `lineblit_init` with the **first table entry** (`ASM.ASM:213-217`), which is
`lblt_54b56`, the line to (−10,−8) (`LINEBLIT.INC:2-4`) — not the header dot, and not
nothing. This is a required artefact.

With the measured tables two out-of-domain segments occur: **segment 746 of every
iteration has dy = 56** (the y waveform is static), and from **iteration 24** the cyclic
seam segment `k = 1023 − i` has dx = 82 (`sinus1[0] − sinus1[1023]`). Both draw the
(−10,−8) fill word, once per line, in the current colour.

The port replaces the generated code with a loop that produces the same pixel set and
XORs the colour into both buffers at the same wrapping address; every segment is drawn
separately (so cross-colour overlaps stay lit). Pinned by
`the_line_blit_is_the_generated_xor_bresenham`: the whole filled table at a representative
start against a direct `DOLINE.C` translation, edges, negative directions, a bottom-row
crossing, 16-bit address wrap, the endpoint exclusion, low-byte selection (a 266-pixel
delta selects the (10,0) routine), the fill-word default, both measured out-of-domain
segments, and the different-colour/same-colour crossing cases.

## 6. The loop, and the port's frame mapping

Each source iteration (`MAIN.C:157-177`, `ASM.ASM:136-202`):

1. `dis_exit()` at the top; `dis_muscode(3)` check (`:160`).
2. `plane = (plane + 1) % 3` from 0 (`ASM.ASM:141-150`), so iteration 0 uses plane 1 and
   colour 2, iteration 1 plane 2 and colour 4, iteration 2 plane 0 and colour 1.
3. Erase the line that **this plane** drew three iterations earlier: replay its stored
   `di` and routine with the current colour, XORing it off both buffers (`:155-169`).
   The first three iterations' slots hold `@@ret`, so they erase nothing.
4. Rebuild the 1,001 points and draw 1,000 segments, storing each segment's `di` and
   routine in the plane's slot (`:170-200`). `d = sin1[k&1023]/4+256; d = 0;`
   (`MAIN.C:162-163`) is a dead store: the y waveform is static.
5. `setborder(0)`, `dis_waitb()`, `setborder(1)`, `setpalette(pal[plane])`.

The palette upload happens at the end of the iteration, after the retrace, so the frame
**published** for iteration `i` is displayed under the palette uploaded at the end of
iteration `i−1`: `pal[i mod 3]`. This is measured, not assumed: the filed 400's line tints
are the +40/+20/+10 ranks of `pal[0]`, 500's of `pal[1]`, 600's of `pal[2]`, and 700/800
repeat the cycle; the port's comparison matches only under that mapping.

**The port's frame numbering.** Reference 353 is route B's 704, whose content is source
iteration **1** (iteration 0 is the start-up transition frame, reference 352, which Task
2's `K0` excludes). The port therefore:

- performs iteration 0 in `init`, storing its line, but never composes it;
- has its composed frame `j` (`j = reference index − 353`) draw source iteration
  `j + 1`;
- sets the display palette `pal[i mod 3]` at the **start** of the tick that draws
  iteration `i` — the source's end-of-previous-iteration upload, moved to the tick's
  start because the engine stamps the live palette onto the frame this tick composes
  (`Engine::tick`'s `compose_band` then `timeline.push`), not onto the next one. This is
  the presentation convention the capture measures; no constant or anchor was changed
  (D17/D18).

The display is the source's linear mode-13h screen: `vram` and `bufdata` are 65,536-byte
segments because the generated `di` is 16-bit and wraps; the visible picture is the first
64,000 bytes. The port packs that picture at the start of the engine's 640×400 surface and
sets the view to 320×200, origin 0, `y_line_offset = Some(320)`, no split, border 0
(invisible in dumps). `init` also installs the mode-13h default DAC
(`shell::hardware_default_palette`, the table the shell already gives every run's first
part); the first palette upload comes at frame 0.

**Substitutions** (registered in `APPROXIMATIONS.md` A40): `getch()` is not ported as a
key wait — the reference capture's one `KBD_space` selects the state the port starts in.
`dis_muscode(3) == 3` can never fire: the port's `muscode` is 0 with no player (D8), and
the capture delivers no second key. The dot screen is not drawn (§1). The `#if 0` wipe
and the commented `muscode(2)` wait (`MAIN.C:70-93`, `:155`) do not ship.

## 7. Verification

```bash
cargo test -p secondreality --bin secondreality parts::vault::grid
cargo test -p secondreality --test vault -- --nocapture
```

The unit tests pin the tables and `d0`, the eye shift and palettes, the buffer start and
the full line-blit domain; the process tests run the real binary:

- `a_part_run_of_grid_numbers_itself_from_zero_and_is_silent` — `--part grid --headless
  --dump-frames 0,1,10`: manifest `index_origin: "part"`, `anchor: null`, `music: null`,
  320×200 frames, `part: "grid"`.
- `grid_matches_the_reference_at_its_measured_start` — for every filed keyframe `R`, dumps
  port frames `R−K0−1`, `R−K0`, `R−K0+1` and compares each RGB with
  `reference/parts/grid/frame_{R:06}.rgb`, printing the three counts, the minimum's
  offset, the max channel delta and the residual rows. It asserts that every differing row is
  14 or 15 and that the row histogram sums to the residual.

**Residual (registered, `APPROXIMATIONS.md` A41).** The minimum differing-pixel count is
**6 / 2 / 36 / 33 / 14** at 400 / 500 / 600 / 700 / 800, at offset 0 (max channel delta
41); the ±1 neighbours differ by about 10,000–12,000 pixels. Every residual pixel is on
rows 14–15, the topmost rows the curve reaches: the capture published its frame while the
original was still drawing the top of the newest line, so those pixels are missing it or
carry the previous red rank. The test asserts this row claim (every histogram key is 14 or
15, and the histogram sums to the residual), not only prints it. This is a sampling instant,
not a model error — the eye, the 100 unshifted bytes, the three line colours, the palettes
and the (−10,−8) artefacts match everywhere else. The same class appeared in a scratch
route-B comparison (2/24 pixels at 800/1000; historical — the route-B capture is the
`RUNP.BAT` recipe of `reference-indices.md` §24.2 and the mapping is `K0` = 704, so the port
frames are 96 and 296; it is not a filed claim). No constant, anchor or palette phase was
fitted to reduce it (D17/D18/D24/D25).
