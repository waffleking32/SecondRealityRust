# Progress

Phase status board. **Read this first if you are resuming work.**

- `not started` — no work done
- `in progress` — active; see the phase plan
- `done` — exit criteria met, evidence recorded, `phase-N` tag applied

| # | Phase | State | Plan | Tag |
|---|---|---|---|---|
| 0 | Foundation & harness | **done** | [00-foundation.md](docs/plans/00-foundation.md) | `phase-0` |
| 1 | Assets & references | **in progress** | [01-assets.md](docs/plans/01-assets.md) | — |
| 2 | Shell + easy tier | not started | — | — |
| 3 | Music engine | not started | — | — |
| 4 | Music-synced tier A | not started | — | — |
| 5 | Mid tier | not started | — | — |
| 6 | Vector engine | not started | — | — |
| 7 | GLENZ | not started | — | — |
| 8 | Vault | not started | — | — |
| 9 | Assembly & fidelity pass | not started | — | — |

## Where to pick up

**Phase 0 is complete** (tag `phase-0`). The engine, the binary and the verification harness all
work; no demo part has been ported yet.

**Phase 1 is in progress.** Its plan is `docs/plans/01-assets.md`; its ledger (the recovery map, git-ignored
scratch) is `.superpowers/sdd/01-assets/progress.md`.

**Part A (assets) is complete** — A1 through A8. `sr-assets` decodes Origin PIC (five variants plus a
headerless case), IFF (ILBM and PBM), RIX3, `.PAL`, OMF `LEDATA`, FLI and `.ANI`, plus a proportional
font-strip view. **361 workspace tests**; `assets/` holds **150 assets plus `PROVENANCE.md`** (149 curated
plus one recovered from an EXE); `cargo xtask assets verify --tree --decode` exits **0** with `verdict: ok`
and writes 21 contact sheets. Four approximation entries; 17 plan corrections, each from a measurement
rather than from reasoning. The phase's human-look evidence is the user's own reading of the sheets
("images look fine") — the noise-like tiles are non-picture data (3D scene files, TASM listings, S3M
music, lookup tables, palette strips) correctly rendered as byte views; none of them carries a picture
magic.

**Part B (references) is the remaining work** — B1 through B6. B1 is done and its outcome is D17: the
boot repeats under `--boot autoexec` (`[autoexec]` + `AUTOTYPE`) with the emulator run silent
(`mixer_nosound`), and B2–B5 capture under that recipe at `--cycles 30000`. Phase 1's exit criterion is
not met until every part has a reference that re-captures identically.

Phase 2 is *Shell + easy tier*, whose exit criterion is **BEGLOGO frame-diff-identical** — that is the
project's central technical risk, and it is answered there, not in Phase 9. Phase 3 (music) has no
dependency on Phase 2 and may run in parallel if the music is worth retiring first.

Two things Phase 1 inherits and should not have to rediscover:

- The loader formats are listed in spec §4.4; the assets themselves are in the `upstream/SecondReality`
  submodule, and `REALITY.FC` there is byte-identical to the shipped one (`docs/decisions.md` D12), so
  decoding works on authentic media.
- Assets that exist only compiled into a shipped EXE are recovered from OMF `LEDATA` records
  (spec §9).

Phase 2 is *Shell + easy tier*, whose exit criterion is **BEGLOGO frame-diff-identical** — that is the
project's central technical risk, and it is answered there, not in Phase 9. Phase 3 (music) has no
dependency on Phase 2 and may run in parallel if the music is worth retiring first.

The reference oracle's input is `reference/original/` — the 1993 retail distribution
(`SECOND.EXE` + `REALITY.FC`), committed unmodified. See `docs/decisions.md` D12 and
`reference/original/PROVENANCE.md`. Do not modify those files: every frame diff in the
project is measured against those exact bytes.

## Harness evidence (Task 13)

`cargo xtask compare` was verified both ways against the Task 12 captures, with the
tool's own exit code as the verdict:

- **Identical inputs.** `cargo xtask compare --reference reference/check_a --rendered
  reference/check_a --out out/self` → every frame 0.0000% differing, `verdict:
  identical`, **exit code 0**. Reports: `out/self/part/report.json`,
  `out/self/title/report.json`.
- **Differing inputs.** A second capture from a point that cannot match
  (`SECOND 5` — the end logo/credits block — at the same boot indices and geometry):
  `--arg 5 --keys "KBD_left KBD_enter" --cycles 25000`, frames 400,600 (320×400) and
  1100,1150 (320×200), into `out/check_neg/`. `cargo xtask compare --reference
  reference/check_a --rendered out/check_neg --out out/neg` → title 99.5609% differing
  (indexed), part 40.7172% differing (RGB: that capture's screen moved during the
  indexed read, so it wrote no indices and the comparison fell back to RGB),
  `verdict: different`, **exit code 1**, diff PNGs under `out/neg/` (`diff_000400.png`
  and `diff_001100.png` for that run; the re-run below writes them under `out/neg2/`
  as `diff_000700.png` and `diff_001350.png`, with the same percentages).

A comparison that cannot run at all (a geometry or frame-count mismatch, a missing
manifest or frame file) is refused with a legible message and exit code 2, so a script
can tell "the frames differ" from "the tool could not tell".

Both checks above were re-run after B1 re-captured `reference/check_a` (D17; the same two
screens at new indices, frames byte-identical to the ones they replace), and the numbers
are the same: self-comparison `verdict: identical` exit 0, the `SECOND 5` negative
`verdict: different` exit 1 with title 99.5609% and part 40.7172%.

## Phase log

### Phase 0 — Foundation & harness (done, tag `phase-0`)

Fourteen tasks on branch `phase-0-foundation`. **All tests pass: 160 across the workspace, zero
warnings** — 98 in `sr-engine`, 55 in `xtask`, 6 unit + 1 integration in `bins/secondreality`. (155
at close-out; the five added since are below.)

**What was built.** `sr-engine`, a dependency-free software VGA model: an indexed `Surface`, a
6-bit `Palette` with a per-scanline `PaletteTimeline`, `View` (the CRTC — display start, line
offset, fine pan, double-scan, line-compare split, overscan colour), a fixed 70.086 Hz `Tick`, beam
compose, the three copper slots, the `Part` trait, `Engine`/`Runner`, and `present` for
indexed→RGBA. Plus `bins/secondreality` (SDL2 window and a headless frame dump) with a synthetic
test card cycling the five visually-significant hardware behaviours of spec §2.4, and `xtask`
(`Oracle` trait, `DosboxOracle`, `reference capture`, `compare`).

**Evidence, per exit criterion.**

| Criterion (spec §10) | Evidence |
|---|---|
| `cargo test` green | 160 passed, 0 failed, no warnings |
| Test card at correct speed, exercising all five behaviours | **703 ticks in 10.040 s = 70.019 Hz** (`--run-seconds 10`); three further runs matched `floor(70.086 × elapsed)` exactly. One property test per behaviour in `testcard.rs` |
| `xtask reference capture` captures from the original | `reference/check_a/{title,part}/`. **Four independent captures agree byte-for-byte** — two runs, each capturing the 320×400 title screen and the 320×200 part |
| `xtask compare` produces a diff report | Identical → `verdict: identical`, exit 0. Differing → `verdict: different`, exit 1, diff PNGs. Hard errors → exit 2 |
| Spec §2.2 verified against `MAIN/U2.ASM` | `docs/fidelity/part-sequence.md`, with line citations. §2.2 was **wrong** — see below |

**Deviations from the plan, and what changed the approach.**

- **The split model was wrong, and it was load-bearing.** `View` counted post-split scanlines from
  the top of the frame. Line compare resets the CRTC's address *and* scanline counters, so a
  scanline below a split is `y - split.scanline` within the lower view. Counting from the frame top
  makes the register a no-op — with the same start address and no restart, the picture below the
  split would be identical to the picture with no split at all. The original settles it:
  `PLZPART/ASMYT.ASM:99` writes only CRTC 0x18 and 0x07 bit 4 and never the start address, while
  `PLZPART/COPPER.ASM:126` alternates line compare 60/61. Caught because Task 5's compose test and
  Task 8's test card encoded *opposite* models. Commits `349c705`, `10acc3b`.
- **`dac_expand` was wrong.** The emulator widens 6→8 bits by rounding, `(c*259+33)>>6`, not by bit
  replication; they differ on ten of the 64 values. The rounding rule is also the ideal even mapping
  `round(c·255/63)`, so it was adopted rather than merely tolerated. `deae14c`.
- **Task 8's brief had three linked defects**, found by an implementer that stopped rather than
  commit a red suite: the frame was sized once at construction so `view.height = 400` could never
  yield 400 lines (double-scan showed half the picture); `y_line_offset` must be the *surface*
  stride, not the view width, or a 320-wide view on the 640-wide surface reads a different layout
  than the part wrote; and a palette ramp masked to a sawtooth in six bits. `1dac062`.
- **Spec §2.2 was wrong about the music.** The "MUSIC1 restarts at order 42" row is not part of a
  full run at all — it is a `SECOND 3`-only path — and a MUSIC0 order-25 restart was missing
  entirely (`SECOND 5` only). A full run has exactly **three** music events: MUSIC0@0, MUSIC1@0
  after BEGLOGO, MUSIC0@18 before U2E. A port built from the old table would have restarted at
  orders 42 and 25 mid-run and drifted against every later part. D13, commit `692e6e8`.
- **Capture indices are boot-relative and had to move.** Frame 0 is the emulator's first rendered
  frame; `SECOND.EXE` is not reached until counter ~1657, so frames 10/60/120 are DOS-shell frames
  and the title screen is around frame 1700. `d0ed148`.
- **`compare`'s `--out` defaulted into the reference tree**, writing reports into the project's
  input-of-record. Now defaults to `out/compare`, with a guard refusing an explicit `--out` inside
  any reference tree. `22eff72`.
- **Open item for Phase 2: `View`'s origin is `u16` while the surface is 256,000 pixels.** Recorded
  in `docs/decisions.md` D16. ENDSCRL's 401-line circular buffer needs ~256,000; Phase 0 left the
  fields as the spec declares them.

**Post-review fixes.** The closing fidelity review found all three of the phase's contested decisions
sound — the split model, the DAC widening rule and the independent line stride — and left three
findings, all now fixed. **`phase-0` was moved forward to include them**; `9cecc1b` is the close-out
as reviewed, and every commit after it is Phase 0 work — these three fixes, the window's colour bug,
and the records of both. A phase that ships a known trap is not done, and the tag is what a resuming
session checks out.

The three findings:

- **The line stride's default was a trap, and it had already sprung.** `View::new` defaulted
  `y_line_offset` to the view's own width, so a 320-wide picture drawn through `Surface` on the
  640-wide surface was read back sheared; `bins/secondreality/src/main.rs` worked only because
  `TestCard::init` overwrote the view. `y_line_offset` is now `Option<u16>`, and `None` resolves to
  the surface's row stride inside `View::source_index` — per effective view, so a split's lower view
  resolves its own. `TestCard` no longer sets a stride at all, which makes the card a live test of
  the default. `cc28a4e`.
- **The split's two conventions were unwritten** — the review's judgement was that this was the
  model's single biggest risk. The hardware forces the post-split origin to **0** and clears panning
  (`VGA_ProcessSplit`, `vga_draw.cpp:1139-1158`), so a faithful translation of `set_plzstart` leaves
  the lower view's origin alone; and the emulator applies the line-compare register on
  **register + 1** (`vga_draw.cpp:1586-1593`, `:1276`), so a value copied from the assembly is one
  less than `Split::scanline`. Both are now on `Split`'s doc comment and in D16, with the two
  unsettled caveats (`lines_scaled`, `vblank_skip`) that the first splitting part must measure.
  `00e3181`.
- **A citation slip in D14**: `src/utils/rgb.h:31-36` is `rgb5_to_8`; `rgb6_to_8` is 22-28.
  `334f82b`.

**And the window was finally watched — which found a bug no test could have.** Task 9's step 5 was to
verify the windowed run by hand, and it was the one thing no agent could do; the phase recorded that
gap rather than hiding it. The user looked, and reported the colours as *mostly reddish*. The engine
was right — a headless dump is a correct sixteen-colour spectrum — and so were the captures from the
original. The window declared `SDL_PIXELFORMAT_RGBA8888` for a buffer holding a byte array with red
first, and SDL names packed formats by bit significance rather than memory order, so on little-endian
it read `present`'s **red byte as alpha**: every pixel at full red, green and blue swapped. Fixed in
`03a3b11` by declaring SDL's own "RGBA byte array" format, `RGBA32` (which resolves to `ABGR8888` on
little-endian), with a test that decodes a known pixel through SDL's format tables — it failed against
the old value for exactly the reported reason. Reproduced from a dumped frame first: the misread puts
R at 255 on 64,000 of 64,000 pixels against 16,000 of 64,000 for the frame the engine drew.
**Re-run after the fix, the colours are correct** — which closes the phase's only unverified visual
claim, and it was worth closing: the first look found a bug that no test in the repository could see.

**Working note.** The phase's ledger — every ruling, every deferred minor, every plan correction —
is at `.superpowers/sdd/00-foundation/progress.md`. It is git-ignored scratch and will be deleted
once the final whole-branch review closes, so anything durable was copied into
`docs/decisions.md` or the fidelity notes. The deferred minors points worth knowing about are in D16.

