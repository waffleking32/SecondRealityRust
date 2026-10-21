# TWIST — the planar twister

TWIST is one of the three unreleased development effects: a 320×200 mode-X program
(`TWIST/T.EXE`) that melts a static texture by selecting one of its 200 rows per scanline,
flipped between two VGA pages through the CRTC start address. The port is
`bins/secondreality/src/parts/vault/twist.rs`; it is silent, unanchored, and reached by
`--part twist` (and, from Task 8, the vault menu). Its reference is
`reference/parts/twist`.

## Capture (Phase 8 Task 2)

The reference is `reference/parts/twist`, a version-1 capture of the vendored `TWIST/T.EXE`
(`88d5e695a485b87b33bce8646b67f843d410e0fa822e76d8fbbb76cc9d33aab8`) run behind the
standalone `DIS/DIS.EXE`, staged and mounted read-only by `tools/vault_capture.sh`. TWIST has
no key gate (`TWIST/MAIN.C:49`) and **takes no key** — a stray key after `doit()` begins
exits the part:

```bash
tools/vault_capture.sh twist --frames 103,203,303,403,503,603
```

The manifest says `exe: "RUN.BAT"`, `arguments: []`, `boot: "autoexec"`,
`boot_wait: "graphics"`, `cycles: 30000`, `frame_origin: "boot"`, `skip_boot_frames: 0`,
320×200, `keys: []`, oracle `dosbox-automation 0.85.1 (6d100)`, upstream `071a82e`. The
batch is the script's CRLF `RUN.BAT`: `@ECHO OFF`, `DIS.EXE`, `T.EXE`.

### Start and `K0`

The boot's graphics anchor returns at boot frame 5 (six probes for frame 4: five named frame
5, one named frame 6), so frames 0–4 cannot be captured. An early dense sweep in two sessions
(`out/task2/twist-start`, `out/task2/twist-start-b`) identified the phase by the two-band
fingerprint: at boot index **5** rows 0..21 are generated row 0 and rows 22..199 are row 199
(boundary `ceil(64/3)` = effect frame **2**), and at **6** the boundary is 16 (effect frame
**3**) — the same bytes in both sessions (`e385838c…` and `d5ce27ce…`). A loaded session that
let the anchor return one frame earlier captured boot **4** directly: its boundary is **32**
(`ceil(64/2)`, effect frame **1**, `595bdf32…`). The phase is matched at three consecutive
boot indices (4→1, 5→2, 6→3), so the frame-0 two-band picture (rows 0..63 = row 0, rows
64..199 = row 199) sits at boot index **3**: **`K0` = 3, tolerance ±1**. Frame 3 itself is
below the anchor in every session tried and is derived from the matched phase, not from a
key. The 4,444-frame cap ends the part at boot `3 + 4444 = 4447`; the keyframes are far inside
it.

### Keyframes

| index | effect frame | `sha256` |
|---|---|---|
| 103 | 100 | `6615ba392a8f2ed3c2ffe979c0f3e730b531365eae00e62737ec852ff18513fe` |
| 203 | 200 | `bef51119bc9c880b7f06cf4d32aee13c62a472b58d9831d3107599bba8d291ad` |
| 303 | 300 | `55f222106b2a376529ec73920cb3c834f7dd241793c6d3a7bd57de470f406da1` |
| 403 | 400 | `42aa02f6aeb06deeb31dad96530550b977f7d5d27ce81ce51356f94b84789491` |
| 503 | 500 | `e9ab57c9a7b925a49b54c0ed15486120b4875e494aac2b4d86f593f78ffb3f95` |
| 603 | 600 | `7b288e5abdb02fb60c3f628ed6a81ca8066b51ab484a390cd8586f35527c4780` |

~100 apart — the spacing Task 1 measured safe for TWIST, whose indexed read is expensive (the
brief's 45-frame `355,400,600` gap refuses under D19; `355,500,600` and this set exit 0). All
six are **RGB-only**. There is no `.idx` even in principle: even part frames draw and display
buffer 1 at origin 0, odd frames buffer 2 at 131,072 (`TWIST/MAIN.C:60-76`), and the oracle's
indexed read always starts at plane offset 0, so an odd frame would be read from the wrong
buffer before the moving screen's coherence check even runs.

### Reproduced

Frame 603 was captured again in a fresh session (`out/task2/twist-repeat-603`) and is
byte-identical to the filed `7b288e5a…`; 103 is likewise identical in two further sessions.
One six-frame trial session (`out/task2/twist-reference-trial`) was one effect frame off at
every keyframe (`6a898f21…` at 103, `3d33fdcb…` at 603) — D17's session alignment, the ±1
`compare` searches, not a new anchor. `cargo xtask reference verify --tree reference` passes
over the capture.

### The ±1, measured (Phase 8 Task 4)

The trial session's one-effect-frame shift had only six differing hashes behind it. A
three-frame support capture in one session, all taken before any read, settles it:

```bash
tools/vault_capture.sh twist --frames 102,103,104 --out out/task4/twist-support
sha256sum out/task4/twist-support/frame_000102.rgb \
          out/task4/twist-support/frame_000103.rgb \
          out/task4/twist-support/frame_000104.rgb
```

```text
frame_000102.rgb  6a898f215b02d9b39dbfc94463d9ce5fdb01569accfb490383b882be3c796e7b
frame_000103.rgb  6615ba392a8f2ed3c2ffe979c0f3e730b531365eae00e62737ec852ff18513fe
frame_000104.rgb  7d0beec6793bfa9c69abcceef5ecfc77062b3e2babdc9e9fbd1bf4d6921288ba
```

The support session is in the filed phase: its 102/103/104 are three consecutive effect
frames, and the port's own dumps of frames 99/100/101 are the same three hashes:

```bash
cargo run -q -p secondreality --bin secondreality -- --part twist --headless \
    --dump-frames 99,100,101 --dump-dir out/task4/twist-port
sha256sum out/task4/twist-port/twist_000099.rgb \
          out/task4/twist-port/twist_000100.rgb \
          out/task4/twist-port/twist_000101.rgb
```

```text
twist_000099.rgb  6a898f215b02d9b39dbfc94463d9ce5fdb01569accfb490383b882be3c796e7b
twist_000100.rgb  6615ba392a8f2ed3c2ffe979c0f3e730b531365eae00e62737ec852ff18513fe
twist_000101.rgb  7d0beec6793bfa9c69abcceef5ecfc77062b3e2babdc9e9fbd1bf4d6921288ba
```

**Effect frames 99 and 101 are odd**, so they draw and display page 2 at origin 131,072;
their match measures the odd-page display, which the six filed (even) keyframes do not
exercise.

The trial's six hashes are the port's effect frames 99..599 — `index - 4` at every
keyframe. The trial was Task 2's second session of the same six indices:

```bash
tools/vault_capture.sh twist --frames 103,203,303,403,503,603 \
    --out out/task2/twist-reference-trial
cargo run -q -p secondreality --bin secondreality -- --part twist --headless \
    --dump-frames 99,199,299,399,499,599 --dump-dir out/task4/twist-trial-model
sha256sum out/task2/twist-reference-trial/*.rgb out/task4/twist-trial-model/*.rgb
```

The two six-line `sha256sum` outputs are the same hashes in the two numbering spaces. The
trial session's 103 is `6a898f21…` — the support capture's **102**, one boot
index earlier. So the trial session's whole six-frame set is the filed phase shifted by one
boot index (`effect = index - 4`), which is D17's session alignment and what `compare`'s ±1
searches; it is not a new anchor and no constant was moved. The limitation of the earlier
record is closed: the shift is now shown by an adjacent-index capture, not only by six
differing hashes.

A re-run of either capture command can land in either phase — the session is the emulator's
(D17/D18). What is measured here is which phase each session above fell in; the port's own
dumps are deterministic, and they are what the ±1 search compares against.

## 1. The effect

**The loop** (`TWIST/MAIN.C:44-78`). `doit` runs `while(!kbhit() && frame<4444)`. Each
iteration sets the back buffer, increments `ra` (`ra = frame + 1`), resets a 16-bit `r`,
selects one texture row per scanline, calls the generated `twister`, and flips the CRTC
start address. `waitb` polls VGA input status 0x3DA and returns 1 unconditionally, so one
iteration is one retrace and `frame` counts iterations.

**Row selection** (`MAIN.C:53-59`, `TWIST.ASM:16-46`). For row `y`:

```text
r   = (y * ra) mod 65536                 ; `r += ra` in the compiler's `bx`
idx = ((r >> 6) & 511) + 256             ; never reaches sin1024[768] = -256
w   = trunc(sin1024[idx] * 25 / 64) + 100
row = w % 200                            ; 200 -> 0
```

`MAIN.OBJ`'s `doit` confirms each step: `and si,0x7fc0; sar si,5` masks bit 15 out of `r`;
the sign-restore sequence `cwtd/xor/sub/sar/xor/sub` divides toward zero; and the store is
a 16-bit word in an 8-byte `twist` record. `w` is therefore in 1..=200, and the twister's
clamp is modulo 200: **the sine peak selects row 0**, not row 199. The first selected row
always has `r = 0`, so every frame's top scanline is generated row 0.

**The texture** (`TWIST/TWSTLOOP.INC`, 11,752 lines). It is generated from `TMP.UH` by
`DOL.C`: palette-red decode, edge cleanup, the left half from source row `y`, the right
half from the sheared source row `6 + y*192/200`, and the `v[0]=63` write after each right
routine. The shipped include holds 200 left and 200 right routines, each 20 word stores
covering 40 plane bytes (8,000 stores in all), 1,549 pixel immediates, 55 distinct byte
values in 0..=63, 191 distinct rows, and **121 rows whose first byte is `0x3F`** — the
generator's order-dependent quirk, a visible four-pixel blue left edge. The port consumes
the include as shipped; it does not re-derive a "clean" texture.

**The draw.** Each generated byte is written with sequencer map mask `0x0F`, so it becomes
four identical adjacent pixels. For destination row `y`, the selected row's 80 bytes fill
`y*320 + 0..319` in groups of four.

**The two pages.** `vram1 = A000:0000` and `vram2 = A800:0000` (`MAIN.C:13-14`); the second
is plane-byte offset 0x8000 inside the same VGA window, i.e. pixel 131,072 in the port's
chunky convention. `flip` starts 0 and is incremented before its switch, so part frame 0
takes case 1: it draws buffer 1 and writes CRTC start 0x00; frame 1 draws buffer 2 and
writes start 0x80. **Even frames draw and display page 1 (origin 0), odd frames page 2
(origin 131,072).** The page just drawn is the one displayed; there is no one-frame lag.
The mode leaves CRTC 0x13 at 80 plane bytes (320 chunky pixels) per row.

**The mode.** `inittwk` (`ASM.ASM:34-63`) writes CRTC 0x14 and 0x17, turns chain-4 and
odd/even off, sets the map mask, clears all four planes and clears the DAC to black. Its
"400 rows" CRTC 0x09 write is **commented out** (`:43-46`), so mode 13h's 0x41 remains and
the emulator reports a 320×200 double-scanned frame; JPLOGO's otherwise identical
`inittwk` has the write active and is 320×400. The port models the emulator's 200 raw
rows.

**The palette** (`MAIN.C:91-99`). The C writes the bytes `(a/3, 2a/3, a)`; the DAC masks
each write to its low six bits, so entry `a` is `((a/3)&63, (2a/3)&63, a&63)`. Entry 63 is
`(21,42,63)`, entry 191 white and entry 192 black. `vram[a]=a` and `memset(vram,0,64000)`
are dead: the first twister draw overwrites the visible page.

**No runtime file, no music.** `MAIN.C`/`TWIST.ASM`/`ASM.ASM` open no data file and call no
music service; the texture is compiled in. TWIST's only DIS calls are `dis_partstart` and
`dis_indemo`.

## 2. The port

- **`rows(frame)`** is the source formula with the two arithmetic traps preserved: `ra =
  frame + 1` as a `u16`, `r = (y as u16).wrapping_mul(ra)`, Rust's `/` for the C's
  truncating division, and `w % 200`. The unit test pins the full 200-byte selection at
  frames 0, 1, 2, 3, 99, 327, 999 and 4,443; the opening boundaries (`ceil(64/(f+1))` =
  64/32/22/16); the fact that frame 326's iteration stays inside a 16-bit word (200·327 =
  65,400) while frame 327's final, unused add wraps (200·328 = 65,600), the first *selected*
  row not wrapping until frame 329's y=199 (199·330 = 65,670 → 134); truncation at both
  `sin = -255` (row 1, where floor gives 0) and a non-peak negative; and the peak mapping
  200 → row 0 inside a wrapped frame.
- **`TEXTURE`** is the include's 16,000 bytes transcribed as `[[u8; 80]; 200]`. A unit test
  reads `upstream/SecondReality/TWIST/TWSTLOOP.INC`, walks its labels and
  `mov ax,0XXXXh`/`mov ds:[di+N],ax` stores, reconstructs the 200 rows, and asserts
  equality — plus the shape above, every byte ≤63, the 121 rows beginning `0x3F`, and 191
  distinct rows.
- **The draw** is `draw_page`: the selected row's 80 bytes copied to `page + y*320`, each
  byte four times.
- **The pages** are pixel origins 0 and 131,072 in the engine's 640×400 surface;
  `view.y_line_offset = Some(320)`, width 320, height 200, no split, `double_scan = false`.
  `frame` draws the page for its parity and sets `view.start_addr` in the same call, so the
  engine composes the page just drawn. The unit test ticks four frames and checks the origin
  alternates, page 2's last row ends at 195,071, and the composed rows are the selected
  texture expanded four-fold.
- **The palette** is built with `Palette::set_entry`, not `from_vga`; the unit test pins
  every entry, the three named entries, and that `from_vga` of the same bytes is four times
  darker.
- **The Part** is `Twist`: `init` sets the view, clears the surface (`inittwk`'s plane
  clear), and uploads the 256-entry palette; `frame` checks `ctx.exit_requested` and
  `ctx.frame >= LAST_FRAME` at the top (both draw nothing), draws its page and flips the
  origin. `LAST_FRAME = 4444`: frames 0..4,443 draw; the 4,445th call returns `Finished`
  and the shell holds the last picture. The process test dumps 4,442/4,443/4,444/4,500 and
  asserts 4,443 is a new picture while 4,444 and 4,500 are its held bytes (RGB and indexed).

## 3. The comparison

All six filed keyframes are effect frames 100..600 under `K0 = 3` (`effect = index - 3`),
and the port reproduces every one **exactly**. The test dumps the port's candidates
`R-K0-1, R-K0, R-K0+1`, compares RGB and prints the three counts; the minimum is 0 pixels
at offset 0 for all six:

```text
reference 103: offsets -1/0/+1 give [(-1, 3104), (0, 0), (1, 3460)]; minimum 0 pixel(s) at offset +0 (max channel delta 0); rows {}
reference 203: offsets -1/0/+1 give [(-1, 2512), (0, 0), (1, 3220)]; minimum 0 pixel(s) at offset +0 (max channel delta 0); rows {}
reference 303: offsets -1/0/+1 give [(-1, 3732), (0, 0), (1, 3616)]; minimum 0 pixel(s) at offset +0 (max channel delta 0); rows {}
reference 403: offsets -1/0/+1 give [(-1, 3064), (0, 0), (1, 3324)]; minimum 0 pixel(s) at offset +0 (max channel delta 0); rows {}
reference 503: offsets -1/0/+1 give [(-1, 2884), (0, 0), (1, 3404)]; minimum 0 pixel(s) at offset +0 (max channel delta 0); rows {}
reference 603: offsets -1/0/+1 give [(-1, 3284), (0, 0), (1, 3280)]; minimum 0 pixel(s) at offset +0 (max channel delta 0); rows {}
```

There is no residual to register: the port's model — the 16-bit phase, the truncating
division, the shipped texture, the four-pixel expansion, the page-1 origin and the masked
palette — is the captured picture at every keyframe. All six are RGB-only: the moving screen
fails the indexed read's coherence check, and an odd frame's page 2 is not at plane offset 0,
so the oracle's indexed read would see the wrong buffer.

## 4. Substitutions

- **The copper is dropped** (`ASM.ASM:220-269`). `initcoppers` registers `testcopper` in DIS
  slots 0 and 1, but the shipped standalone `DIS.EXE` has no INDEMO build: its service 7 is
  a bare `ret` (`DISINT.ASM:294-316` is inside `IFDEF INDEMO`), and the reference shows no
  flash. The port registers nothing and invents nothing. `APPROXIMATIONS.md` A42.
- **The developer input is dropped.** `waitb`'s Scroll-Lock border toggle
  (`MAIN.C:23,26`) and `kbhit`/`getch` (`:49,102`) are developer conveniences. The
  original exits on **any** key; the port's input model is ESC/Ctrl, which is what the
  shell has, and the reference capture sends no key. `APPROXIMATIONS.md` A43.
- **The fixed 70.086 Hz tick stands for the retrace.** The source has no timer; one `doit`
  iteration waits for one VGA retrace. One engine tick is one iteration, and the port does
  not model a missed retrace (which the source could not observe either: `waitb` always
  returns 1). 4,444 ticks at 70.086 Hz = 4,444 / 70.086 = **63.41 s** nominal. The compared
  frames are phase-locked by tick index, not by wall clock. `APPROXIMATIONS.md` A44.
- **The comparison is RGB-only** because the reference is: the moving screen fails the
  indexed read's coherence check, and an odd frame's page 2 is not at plane offset 0. Indexed
  would be the stricter side, but no TWIST reference in this tree carries one, and the oracle
  as built cannot produce one for either page; the RGB comparison is exact.
  `APPROXIMATIONS.md` A45.
- **Dead code is not ported.** `line()`/`leftline`/`rightline` (`MAIN.C:30-42`,
  `ASM.ASM:97-218`) are linked but never called by TWIST; the `memset` and `vram[a]=a` are
  overwritten before the first display; the 8-byte `twist` record's last six bytes are
  never read. None is observable and none is modelled.
- **`initcoppers`/`dis_indemo` are not modelled.** The standalone DIS returns `indemo = 0`,
  so T.EXE restores text mode after `doit`; the port has no text mode and the shell owns
  presentation. The part does not drain after exit.

## 5. Reproduce

```bash
# the unit tests: the row phase, the shipped texture, the pages, the palette, the bands
cargo test -p secondreality --bin secondreality vault::twist
# the process tests: silent/unanchored, the six keyframes, the 4,444/4,500 split
cargo test -p secondreality --test vault -- --nocapture
# the filed capture (Task 2), and the ±1 support capture above
tools/vault_capture.sh twist --frames 103,203,303,403,503,603
tools/vault_capture.sh twist --frames 102,103,104 --out out/task4/twist-support
# the include's line count quoted above
wc -l upstream/SecondReality/TWIST/TWSTLOOP.INC    # 11,752
```
