# TWIST — the planar twister

This note's capture section is written now (Phase 8 Task 2); the part's own task fills the
rest.

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
