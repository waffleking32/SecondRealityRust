# Decisions and findings

Anything that changed the approach, or that a future session would otherwise have to
rediscover. Newest last.

---

## D1 — Fidelity target: faithful now, enhancements opt-in later (2026-10-10)

Accepted: same 320×200 / 320×400 indexed output, same fixed-point maths, same palette
and timing. Target pixel-identical to the original, verified by frame-diffing against
emulator captures. A small number of documented approximations are permitted where an
effect lives inside esoteric generated assembly. The renderer leaves a seam for an
optional enhanced mode later.

Rejected: strictly pixel-perfect (would require a VGA register-level emulation layer
and hand-decompiling multi-thousand-line unrolled assembly); modern remake (different
visual identity).

## D2 — Scope: full demo + hidden part + dev-parts vault (2026-10-10)

Accepted: the 20-part sequence, the hidden DDSTARS part, a modern replacement for the
DOS setup screen, and a vault menu exposing the three unreleased development effects
(GRID, HARD, TWIST).

Rejected: main sequence only (leaves GRID/HARD/TWIST unported).

## D3 — Architecture: software VGA model + per-part modules (2026-10-10)

Accepted: a small engine layer owning an indexed surface, a per-scanline palette, a
view transform replacing the CRTC, a fixed 70.086 Hz tick, and a `Part` trait replacing
the DIS interrupt server. Parts are ordinary Rust modules writing indexed pixels.

Rejected: flat framebuffer with per-part special cases (pushes hardware concepts into
a dozen parts, makes frame-diffs harder); instruction-level emulation (not a port).

## D4 — Pixel aspect: 4:3 default, square-pixel toggle (2026-10-10)

Accepted: default to 4:3 with rectangular pixels, matching a VGA monitor. Square-pixel
16:10 available as a setting.

Rejected: square pixels only (what the existing C++ ports do — geometrically wrong);
4:3 only (no flexibility).

## D5 — The music pattern data is scrambled, and the keystream is recovered (2026-10-10)

**Finding.** `MAIN/MUSIC0.S3M` and `MUSIC1.S3M` (byte-identical copies of what is
embedded in `REALITY.FC`) are standard S3Ms in every respect — header, instrument
tables, correct pattern pointer table tiling on 16-byte boundaries, plain PCM samples —
**except that the packed pattern data is XOR-scrambled.** Stock S3M decoders fail.

**Keystream.** For byte index `i` within a pattern's packed data, from 0 at `ptr+2`,
restarting per pattern:

```
n      = (i >> 1) + 1
key(i) = (((n ^ (n >> 2)) << 3) | ((5*i + 2) & 7)) & 0xFF
```

XOR for `i` in `0 .. ln-2`. Verified 77/77 patterns for MUSIC0, 76/77 for MUSIC1, zero
invalid commands, and the resulting effect histogram reproduces an independently
derived one exactly.

**Two traps found while verifying:** S3M command bytes are ST3 numeric (1–26 = A–Z,
0x30–0x39 = digits), not ASCII; and note bytes are `(octave << 4) | semitone`, valid up
to 0x9B.

**Consequence:** the music is not a research spike; it is roughly ten lines of
descrambling plus a normal ST3-semantics player. This materially reduced Phase 3.

## D6 — Do not copy the C++ ports' shortcuts (2026-10-10)

`SecondRealityW32` and `SecondRealityPlusPlus` stub `dis_setcopper()` to nothing and
upload one palette per frame per whole screen. Consequence: they lost the techno part's
interference seam, a visible quarter-screen discontinuity where the beam catches the
screen mid-redraw. They also widen the palette to 8-bit and spin-wait to 70 Hz.

Adopted instead: preserve the three copper callbacks; keep the palette at 6-bit
precision; fixed timestep decoupled from display refresh; and add **beam compose** —
optional scanline-banded rendering so a part's per-frame work is committed to the
output as the beam would have drawn it.

Also noted: `SecondRealityW32` stubs `dis_muscode()` to `return 0`, so the parts that
wait on it must have been patched. We will implement it properly.

## D7 — Preserve the original's bugs (2026-10-10)

The GMX plasma's palette-leak banding and the no-z-buffer particle overdraw are part of
the visual identity. A "correct" implementation looks wrong. These are recorded as
required behaviour in `docs/fidelity/bugs.md` with reference frames.

## D8 — Open: `_np_zinfo` (2026-10-10)

`dis_muscode()` returns STMIK's `_np_zinfo`, which is not present in the module data
(all `S` commands are standard `SDx`; MUSIC0 has no cells outside channels 0–7;
MUSIC1's only off-channel cells are five near-empty ones on channels 10–14). Exposure:
BEGLOGO waits for `0xf0`, GRID for 1/2/3, HARD for 1 and 0xff.

Resolution ladder: (1) parse `STMIK.300`'s OMF symbol records for `_np_zinfo` /
`_np_zplus` offsets, then read them live from the emulated original via
`dosbox-automation`'s memory API and correlate against order/row; (2) read `st3play`'s
source for the semantics of "plus flags"; (3) disassemble the STMIK segment.

Escalation trigger: if route 1 fails within Phase 3, implement affected parts against a
substitute cue and record the difference. Do not let it block the phase.
