# GRID — the line rider

This note's capture section is written now (Phase 8 Task 2); the part's own task fills the
rest.

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
fails the indexed read's coherence check and the tool keeps the RGB frame (D15/D19), so
there is no `.idx` in this capture.

### Reproduced

Frame 600 was captured again in a second session (`out/task2/grid-repeat-600`) and is
byte-identical to the filed `1fb43740…`; the filed 400/500/600 also equal Task 1's smoke
captures from a different session, so three sessions agree. `cargo xtask reference verify
--tree reference` passes over the capture.
