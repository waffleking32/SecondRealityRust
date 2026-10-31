# JPLOGO — the jelly logo

Sources: `JPLOGO/JP.C` (227 lines) over `JPLOGO/ASM.ASM` (156: `inittwk`, `lineblit`,
`setpalarea`, `linezoom`), the generated `JPLOGO/ZOOM.INC` (16,837 lines, 54 `@zoom` routines from
`DOL.C`) and `JPLOGO/READP.C` (64); assets `ICEKNGDM.UP` (58,566 bytes, `_PIC.OBK`'s payload) and
JPLOGO's **own** `SIN1024.INC`. Port: `bins/secondreality/src/parts/jplogo.rs` (`"jplogo"`); its
module doc has the screen, the three phases, the tick model and the preserved bugs, and its item docs
the spring, the interpolation, `zoom_row` and `palette_upload`. Run: mapping **`boot = 17065 +
tick`** (no boot anchor). Music: `MUSIC1` seeked to order 96, pre-roll 483 polls (D31).

## 1. The screen

320×400 Mode X at 80 plane bytes a row (module doc). Two emulator facts the module doc does not
carry: `M_VGA` multiplies `real_start` by four (`vga_draw.cpp:1655-1656`, `byte_panning_shift = 4`),
so `scrolly(y)`'s plane start `y*80` is pixel origin `y*320` (`View::set_origin(y*320)`); and
`inittwk`'s `rep stosw` of 32,768 words becomes two dword stores a word in the unchained handler, so
it clears all 256 KB. The view is `width = 320`, `height = 400`, `double_scan = false`,
`y_line_offset = Some(320)` (`the_view_is_320_by_400_at_80_plane_bytes_a_row`). `JP.C:176-185`
writes AC `0x11 = 0xFF`, recorded as border entry 255.

`scrolly` (`:30-45`) writes CRTC `0x0D` (low) then `0x0C` (high); the order matters only to the
hardware, and the port's origin is the one 16-bit display start the pair forms.

## 2. Init

### 2.1 The spring tables (`JP.C:117-171`)

`a` is **200** on the first pass (left by `:114-115`), so `la=a` reads 200, not an uninitialised
word. The first frames and the settle
(`the_spring_tables_are_the_sources_own_first_frames`): `framey1 = [500, 885, 1172, 1323, 1317,
1153, …]`, `framey2 = [6368, 6370, 6354, 6338, 6322, 6306, …]`, `framey1[95:] = 0` and
`framey2[95:] = 6400` exactly; the keyframes (§7) check the rest of the tables.

The fourfold interpolation (`:164-171`) reads `framey1[200]`/`framey2[200]` at `a = 796..799`,
which in the declaration order `:23-26` are `framey2[0]` and `framey1t[0]`. The four values are
`t1 = [0, 2122, 4245, 6368]` and `t2 = [6400, 4433, 2466, 500]`
(`the_interpolation_is_the_sources_own_and_its_overrun_is_unreachable`), and they are
**unobservable**: `doit` reads only `frame <= 511`, which `FREEZE_AFTER`'s compile-time assertion
pins.

### 2.2 The picture (`READP.C`, `JP.C:187-201`)

`ICEKNGDM.UP` is `FD FC` (RLE), 320×400, 256 colours, `add = 49` (pixels at 784).
`readp_is_the_sources_row_rle_and_palette` decodes the raw file through a transliteration of
`READP.C` and matches `parts::load_image` (the embedded copy, decoded by `sr_assets::load_any`)
on the palette and all 400 rows. The run opcode's count register is 8 bits (`READP.C:44-51`,
`dec ah / jnz l4`), so an opcode of exactly `0x80` writes **256** copies; the shipped writers never emit it and `sr-assets` refuses it.

The rows are the **184 bytes from x=70** plus byte 184 = **65**, a synthetic white border; entry 64
is blacked and every source `0` becomes `64`
(`the_rows_are_the_184_byte_crop_remapped_with_a_65_border`).

### 2.3 The DAC

`inittwk` writes 768 zeros to the DAC (`ASM.ASM:31-38`), then `setpalarea(palette,0,256)` uploads
`2*count + start` = 512 bytes (`bugs.md` §4). The post-init DAC is the file's entries 0..169 (64
black), entry 170's red and green over a **zero** blue, and 171..255 **black**
(`the_palette_upload_is_the_sources_512_bytes`, `init_clears_the_dac_before_the_512_byte_upload`).
In a block-3 chain this replaces 3DSINFLD's DAC. The picture's highest index is 65
(`the_picture_uses_only_colours_the_512_byte_upload_writes`), so the short upload is invisible here
and is kept because it is the DAC this part hands on.

### 2.4 The first state

`:203` sets every `lasty`/`lasts` to -1; `:204-210` wait, `scrolly(400)`, wait, and draw all 400
rows at scale 184. The initial draw does not touch `lasty`/`lasts`; they stay -1 until `doit`'s
first iteration.

## 3. Timeline and mapping

| tick | boot | source | on screen |
|---|---|---|---|
| 0 | 17065 | `:204-210` | the `scrolly(400)` screen `init` left (black) |
| 1..83 | 17066..17148 | `:211-219` | the entry: start rows `399, 397, 396, …, 6, 0`, one a vblank |
| 84 | 17149 | `:221` | the `dis_waitb()` before `doit`; the gate reads `musplus` 5 |
| 85.. | 17150.. | `:72-104` | `doit`, its counter reading 1 on tick 85 |
| 85..462 | 17150..17527 | | the wobble (counter 1..378) |
| 463.. | 17528.. | | the hold (counter 379 on): static to the part's end |

The 83-row walk is `the_entry_walks_eighty_three_start_rows_from_399_to_zero` and
`the_entry_ticks_walk_scroll_y_from_399_to_zero`.

**The mapping, `boot = 17065 + tick`, is derived from the pictures**: one origin fits all three
keyframes (17300 at tick 235, 17500 at 435, 17700 at 635; §7). The same origin puts the sweep's
17100 at tick 35, entry step 34, start 309, which is `reference-indices.md` §17.1's "band of the
picture across the top: the entry has just begun" (a corroboration, not a filed reference). There is
no boot anchor: the screen enters in the window `(17000, 17100]` (`reference-indices.md` §17) and
the run is numbered `index_origin: "part"`, `anchor: null`. Conventions: part-sequence.md (block
seams / pre-roll / mapping conventions).

The `doit` counter advances **one a tick**. The source's `frame += c` (`:77`) adds whatever
`dis_waitb` returns; the fixed one reproduces the reference's keyframes, and the model is A27.

## 4. `doit`

The gate `while(!dis_exit() && dis_musplus()<4);` (`:71`) spins in order 96 (`zplus` 1, so
`musplus = row-64`) and opens in order 98 (`zplus` 2, `musplus = row`) from **row 4**. At tick 84
the published position is 98:5, so the gate passes without a spent iteration
(`the_gate_waits_for_musplus_at_least_four`).

- **`if(frame>511) c=400;`** (`:78`): `c` is never read again. The freeze is the skipped
  `y1`/`y2` assignment, written as `frame <= FREEZE_AFTER`
  (`the_counter_freezes_after_511_and_reads_the_table_before_it`).
- **The palimpsest guard** (`:96-99`): a row whose `(b,a)` repeats is not redrawn
  (`the_doit_guard_is_a_palimpsest`). It is the mechanism behind the session-specific holds (§6).
- **Scroll Lock** (`:74-76`): with bit 4 of the BIOS flags at 0x0417 set, the border is 0 around
  the wait and 127 after it. The engine has no keyboard state, so a run never takes the branch and
  the port does not model it.
- `b*32/25` reads the sine at 0..510 only.

### 4.1 The zoom routines

`zoom_row`'s item doc has the dispatch and the per-pixel model. Over every `doit` frame 1..511 the
reachable `a` is **[164, 224]**, so the unsigned 318 clamp and the label fallbacks are unreachable
here (`the_scale_dispatch_is_the_tables_and_the_reachable_range_stays_inside`, which also pins
318/320/-3 → `@zoom244`, 2/141 → `@zoom140`, 0 → `@zoom0`). The generated group stores write exactly
the zeros the per-pixel model does, and the same test pins the clear's untouched margins; 17300
and 17500 at 0 check the model at the part's own scales.
`scale_184_is_the_identity_over_the_185_source_bytes` pins the entry's strip: x=68..251 are source bytes 0..183, x=252 the 65 border, x=67/x=253 black.

## 5. The music

`shell::music_for_part("jplogo")` seeks **MUSIC1 order 96**. The `second3d` trace holds order 96
across the whole window (17000..17100 are rows 50..62 of pattern 71) and steps over the `254` at
order 97 into order 98 at frame 17109.

`jplogo::MUSIC_PRE_ROLL` is **483**: seeked to 96:0, the player's row-58 window opens on poll
**479**, and the trace's row 58 starts at boot 17060, five frames before tick 0; tick 0's poll is
the 484th, so `483 + 1 - 5 = 479`. The published positions
(`the_entry_pre_roll_reproduces_the_traces_positions` asserts every row against the trace):

| tick | boot | published = trace |
|---|---|---|
| 0 | 17065 | 96:58 |
| 3 | 17068 | 96:59 |
| 44 | 17109 | 98:0 |
| 76 | 17141 | 98:4 |
| 84 | 17149 | 98:5 |
| 235 | 17300 | 98:23 |
| 435 | 17500 | 98:48 |
| 635 | 17700 | 98:49 |

A mid-chain start polls nothing (`a_mid_chain_start_does_not_pre_poll`).

## 6. The phases, the holds and the 17700 filing

The hold begins at counter 379: `framey1t[379]/16 = 0` and `framey2t[379]/16 = 400`, so `xsc = 0`
and every row is `(y, 184)`. The first such iteration redraws every row not already `(y,184)`; every
later one draws nothing.

**The hold's pixels are the session's.** The guard keeps rows from earlier states, and how far
`frame` jumps between drawings is the host's speed: `frame += c` can step past the 379..511 window
in one iteration and freeze `y1`/`y2` before the settle, leaving a different patchwork
(`reference-indices.md` §17.3 records this as a hypothesis). The census (§17.3, §17.5):

- **17300**: the filed picture is 4/4.
- **17500**: the filed picture is 2/4, the settled state.
- **17700**: the settled state (`ec3de2bb…`) is 2/4, and the filed picture (`91d6ce6b…`) is 0/4.
  The twelve captures hold ten distinct images at 17700, and seven re-capture probes did not draw the
  filed one.

The port is deterministic and reproduces the **settled** states. Its 17700 is `ec3de2bb…`, the
census's `-sess2` state:

```bash
cargo run -q --release --bin secondreality -- --part jplogo --headless \
    --dump-frames 635 --dump-dir out/jplogo
sha256sum out/jplogo/jplogo_000635.rgb   # ec3de2bb823772bf275387ab122e12e17b1d27a140ec9be8a71faa7d1bd3cb37
```

so its residual against the filed picture equals `-sess2`'s. The filed 17700 is outside its own
census: the residual is the **reference's** (D24), recorded as A27 and registered in rediff's `jplogo`
entry.

## 7. Verification

| keyframe | tick | plane | differing | register |
|---|---|---|---|---|
| 17300 | 235 (and 236) | RGB | 0 | exact |
| 17500 | 435 (and 436) | RGB | 0 | exact |
| 17700 | 635 | RGB | 51,445 px (40.1914 %, max delta 255) | A27 |

The counts are the `jplogo` rows of `xtask/src/rediff/table.rs`. All three are **RGB only**: the
part writes CRTC `0x0C`/`0x0D` during the entry and the oracle's indexed read of them is racy, so no
`.idx` is filed. D19's support from the rendered side: 17300 and
17500 hold on the next tick and the tick before each differs (as the census's 17299 and 17499 do);
at 17700 ticks 634, 635 and 636 are identical, as the census's 17699/17700/17701 are in every
session that holds the state. The manifest says `index_origin: "part"`, `anchor: null`, 320×400,
MUSIC1 order 96 at frame 0.

## 8. Decisions and dead code

- **One counter step a tick** (A27, §3); the 17700 filing is the reference's caveat (A27, §6).
- **Dead, not carried**: `doit`'s `halt`, `storea` and `ysb`; `calc` (`:53-65`); `lineblit`
  (`ASM.ASM:76-100`, never called by `JP.C`); the `storea = a` and its commented-out `printf`
  (`:220-225`).
- **Not modelled**: the Scroll Lock border (`:74-76`, §4; the engine has no keyboard state); the
  0x80 run opcode's 256-copy wrap (`READP.C:44-51`, §2.2; the shipped writers never emit it and
  `sr-assets` refuses it).
- **Carried and unreachable**: the interpolation overrun (§2.1), the zoom clamps (§4.1), the unused
  `c=400` (§4).
- **Carried and visible**: the crop and 65 border, the 0→64 remap, JPLOGO's own sine table (indices
  256 and 768 are ±255; `the_sine_table_is_the_sources_own_1024_words`), the palimpsest guard
  (`bugs.md` §21); the DAC clear and 512-byte upload (`bugs.md` §4).

## 9. Reproduce

```bash
cargo test -p secondreality --bin secondreality parts::jplogo
cargo test -p secondreality --test parts jplogo:: -- --nocapture
cargo xtask rediff --only jplogo
awk -F, '$1 >= 17000 && $1 <= 17100' reference/music/second3d/trace.csv
awk -F, '$1 == 17065 || $1 == 17149 || $1 == 17300 || $1 == 17500 || $1 == 17700' reference/music/second3d/trace.csv
```

**Register.** `bugs.md` §4 and §21; A27.
