# TWIST — the planar twister

Sources: `TWIST/T.EXE`, which is `MAIN.C` (Microsoft C, 16-bit `int`s) linked with `TWIST.ASM`,
`ASM.ASM` and the generated `TWSTLOOP.INC` (`TWIST/MAKEFILE:1-16`). The texture is generated from
`TMP.UH` by `DOL.C` and compiled in; T.EXE opens no file and calls no music service (its only DIS
calls are `dis_partstart` and `dis_indemo`).
Port: `bins/secondreality/src/parts/vault/twist.rs`, registered as `twist`, reached by
`--part twist` and the vault menu. The item docs own the mechanism: `rows` (the 16-bit row phase),
`TEXTURE`, `draw_page`, `PAGE_1`/`PAGE_2` and `twist_palette`.
Run: unanchored and silent. The mapping is `boot = 3 + frame` (§1).
Reference: `reference/parts/twist`, keyframes 103, 203, 303, 403, 503, 603, all RGB-only. The capture
record (recipe, the anchor's return at boot 5, the phase sweep, the repeats and the trial session) is
`reference-indices.md` §24.1, §24.3, §24.6 and §24.8; `K0` = 3 ±1. **All six are byte-identical.**

## 1. Timeline

`doit` (`TWIST/MAIN.C:44-78`) runs `while(!kbhit() && frame<4444)`: one iteration is one retrace
(`waitb` returns 1 unconditionally), and there is no key gate (`:49`), so the effect is animating by
boot 20.

| boot | effect frame | on screen |
|---|---|---|
| 3 | 0 | the two-band picture: rows 0..63 are generated row 0, rows 64..199 row 199 |
| 3 + `f`, `f` small | `f` | the first row differing from row 0 is `ceil(64/(f+1))`: 64/32/22/16 for `f` = 0..3 |
| 103 … 603 | 100 … 600 | the keyframes |
| 4446 | 4,443 | the last drawn frame; the 4,445th call returns `Finished` and the shell holds it |

`K0` = 3 is derived from the phase, not from a captured frame: the boot anchor returns at 5, and
boots 4, 5 and 6 show boundaries 32, 22 and 16, effect frames 1, 2 and 3
(`the_first_frames_are_the_measured_two_band_phase`, `reference-indices.md` §24.6).
`twist_draws_exactly_4444_frames_and_holds` (`tests/vault.rs`) dumps 4,442/4,443/4,444/4,500: 4,443 is
a new picture and 4,444 and 4,500 are its held bytes. 4,444 ticks are 63.41 s nominal.

Facts the item docs do not hold:

- **The pages.** `flip` starts 0 and is incremented before its switch, so effect frame 0 draws buffer
  1 and writes CRTC start 0x00; frame 1 draws buffer 2 and writes 0x80. Even frames draw and display
  page 1 (origin 0), odd frames page 2 (origin 131,072). The page just drawn is the one displayed;
  there is no one-frame lag (`the_pages_alternate_origins_zero_and_131072`).
- **The mode.** `inittwk` (`ASM.ASM:34-63`) turns chain-4 and odd/even off, clears all four planes and
  the DAC. Its "400 rows" CRTC 0x09 write is **commented out** (`:43-46`), so the frame is 320×200
  double-scanned; JPLOGO's otherwise identical `inittwk` has it active and is 320×400.
- **The texture**, `TWSTLOOP.INC` (11,752 lines): 200 left and 200 right routines of 20 word stores
  each (8,000 stores), 1,549 pixel immediates, 55 distinct byte values in 0..=63, 191 distinct rows,
  and **121 rows whose first byte is `0x3F`**: `DOL.C`'s order-dependent `v[0]=63` quirk, a visible
  four-pixel blue left edge. The port consumes it as shipped
  (`the_texture_table_is_the_shipped_twstloop`; `bugs.md` §26).
- **The row arithmetic** is confirmed by `MAIN.OBJ`'s `doit`: `and si,0x7fc0; sar si,5` masks bit 15
  out of `r`; `cwtd/xor/sub/sar/xor/sub` divides toward zero; the store is a 16-bit word in an 8-byte
  `twist` record. Frame 326's iteration stays inside 16 bits (200·327 = 65,400); frame 327's final,
  unused add wraps (200·328 = 65,600); the first *selected* row to wrap is frame 329's y = 199
  (199·330 = 65,670 → 134) (`the_selected_rows_are_the_sources_16_bit_phase`).

## 2. The port, and what is not ported

`Twist::init` sets the linear view, clears the surface (`inittwk`'s plane clear) and uploads the
palette; `frame` checks `ctx.exit_requested` and `LAST_FRAME` at the top, draws the page for its
parity and sets the origin in the same call.

Dead code is not ported (`bugs.md` index): `line()`/`leftline`/`rightline` (`MAIN.C:30-42`,
`ASM.ASM:97-218`) are linked but never called; the `memset(vram,0,64000)` and `vram[a]=a` are
overwritten by the first twister draw; the `twist` record's last six bytes are never read.
`initcoppers`/`dis_indemo` are not modelled: the standalone DIS returns `indemo = 0`, so T.EXE
restores text mode after `doit`; the shell owns presentation, and the part does not drain after exit.

## 3. Comparison

| keyframe | effect frame | −1 | 0 | +1 | register |
|---|---|---|---|---|---|
| 103 | 100 | 3,104 | **0** | 3,460 | exact |
| 203 | 200 | 2,512 | **0** | 3,220 | exact |
| 303 | 300 | 3,732 | **0** | 3,616 | exact |
| 403 | 400 | 3,064 | **0** | 3,324 | exact |
| 503 | 500 | 2,884 | **0** | 3,404 | exact |
| 603 | 600 | 3,284 | **0** | 3,280 | exact |

Differing RGB pixels of 64,000 against the port's frames `R−K0−1`, `R−K0`, `R−K0+1`
(`twist_matches_the_reference_at_its_measured_start`, `tests/vault.rs`, prints them). The comparison
is RGB-only (A45): the moving screen fails the indexed read's coherence check, and an odd frame's page
2 is not at plane offset 0, where the oracle's read starts.

**The ±1, measured.** A three-frame support capture in one session (`--frames 102,103,104`, every
grab before any read) gives

| boot | rgb sha256 | the port's frame with the same bytes |
|---|---|---|
| 102 | `6a898f215b02d9b39dbfc94463d9ce5fdb01569accfb490383b882be3c796e7b` | 99 |
| 103 | `6615ba392a8f2ed3c2ffe979c0f3e730b531365eae00e62737ec852ff18513fe` | 100 |
| 104 | `7d0beec6793bfa9c69abcceef5ecfc77062b3e2babdc9e9fbd1bf4d6921288ba` | 101 |

so that session is in the filed phase, and frames 99 and 101, which are odd, measure the page-2
display the six (even) keyframes do not exercise. The trial session of `reference-indices.md` §24.8, one effect frame off at
every keyframe, is the port's frames 99..599 (`effect = index − 4`): its 103 is the support capture's
102. That is D17's session alignment, which the ±1 search answers; no constant was moved. A re-run
can land in either phase; the port's dumps are deterministic.

## 4. Decisions

- **The copper is dropped** (A42). `initcoppers` (`ASM.ASM:220-269`) registers `testcopper` in DIS
  slots 0 and 1, but the standalone `DIS.EXE` has no INDEMO build: its service 7 is a bare `ret`
  (`DISINT.ASM:294-316` is inside `IFDEF INDEMO`), and the reference shows no flash.
- **Any-key exit and the Scroll-Lock border become ESC/Ctrl** (A43): `waitb`'s border toggle
  (`MAIN.C:23,26`) and `kbhit`/`getch` (`:49,102`) are developer conveniences, and the capture sends
  no key.
- **One 70.086 Hz tick stands for one retrace** (A44). The source has no timer and cannot observe a
  missed retrace.
- **RGB-only** (A45), as above.

## 5. Reproduce

```bash
cargo xtask rediff --only twist
cargo test -p secondreality --test vault -- --nocapture
cargo test -p secondreality --bin secondreality vault::twist
tools/vault_capture.sh twist --frames 103,203,303,403,503,603            # the filed capture
tools/vault_capture.sh twist --frames 102,103,104 --out out/twist-support # the ±1 support capture
cargo run --release --bin secondreality -- --part twist --headless \
    --dump-frames 99,100,101 --dump-dir out/twist-port                    # then sha256sum both
```

**Register.** `bugs.md` §26 (the row arithmetic, the shipped texture) and its index (the dead code);
A42-A45.
