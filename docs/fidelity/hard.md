# HARD — the dots, the grid and the cube

This note's capture section is written now (Phase 8 Task 2); the part's own task fills the
rest.

## Capture (Phase 8 Task 2)

The reference is `reference/parts/hard`, a version-1 capture of the vendored `HARD/B.EXE`
(`1f8a7bdae29a34af42c3ffde63163661ca82a6884a01cb6eb775e81ac600c81d`) run behind the
standalone `DIS/DIS.EXE`, staged and mounted read-only by `tools/vault_capture.sh`, with one
key (`KBD_space`) to open `while (dis_muscode(1) != 1 …)` (`HARD/B.C:138`):

```bash
tools/vault_capture.sh hard --frames 480,560,630,700,770,840,910,1050,1600
```

The manifest says `exe: "RUN.BAT"`, `arguments: []`, `boot: "autoexec"`,
`boot_wait: "graphics"`, `cycles: 30000`, `frame_origin: "boot"`, `skip_boot_frames: 0`,
320×200, `keys: ["KBD_space"]`, oracle `dosbox-automation 0.85.1 (6d100)`, upstream
`071a82e`. The batch is the script's CRLF `RUN.BAT`: `@ECHO OFF`, `DIS.EXE`, `B.EXE`. The one
key never reaches the `0xff` cut (`BP.C:263` needs a second key), so part 3 runs to its own
`count>=800` end.

### Start and `K0`

The gate is followed by four `waitb` calls (`B.C:139-142`) and the pre-roll wipe — one row-100
pixel cleared per x, a `waitb` every 16 columns, then the accelerating fill
(`B.C:143-161`) — then 70 waits (`B.C:162`). A dense sweep and its repeat
(`out/task2/hard-start`, `out/task2/hard-start-b`, byte-identical) measured: 340–355 are the
same white screen `20f3d62f…`; **356 is the first changed frame**
`e7d2f71620f10ce4cbb92a7e42f402c47635a57deeca300100b31d928d1c632d`, with **16 black pixels
at row 100, x=304..319**; 357 has 32, 358 48, … and 375 the whole row; 376–378 repeat 375 (the
fill's first iterations clear no new row); 379 has rows 100–101 black, 380 rows 100–102. The
port counts the wipe as a tick, so its frame 0 is the first wipe frame: **`K0` = 356,
tolerance ±1** (355 is still white, 357 is the 32-pixel step, so a one-frame session
alignment is visible and is what `compare`'s ±1 answers).

### Measured mode starts

| boot index | what changes |
|---|---|
| 474 | first letterbox frame (473 is the half-black filled screen); 474–511 hold with no dots |
| 512 | first spawned dots (mode 1, logo phase 1) |
| ~512–585 | phase 1, `dots1`, **FUTURE** traced in |
| ~587–660 | phase 2, `dots2`, **CREW** over FUTURE |
| ~662–735 | phase 3, `dots3`, **RULES** over CREW |
| ~737–809 | phase 4, `dots4`, **FOREVER** over RULES |
| 810–851 | mode 3: dots dead, FOREVER held (byte-identical throughout) |
| 852 | first mode-4 band frame |
| 930–955 | mode 4's last band state holds |
| 956 | first mode-5 frame (`part2init`); its first distinct grid frame is 958 |
| 1455 | last part-2 frame (fading grid) |
| 1456 | part 2's fade leaves the empty band (the 474 letterbox bytes) |
| 1458 | first part-3 cube frame |

### Keyframes

| index | mode / phase | what is on screen | `sha256` |
|---|---|---|---|
| 480 | 0 | letterbox, empty band | `1930616d436601884fe0f39831ec2ba1309c942d011f238b4c687562756f655b` |
| 560 | 1 / phase 1 | `dots1` tracing FUTURE | `318503a68ad4dd6489124a0b4496dcecc6db1085eda16323eca1fb5c2281063a` |
| 630 | 1 / phase 2 | `dots2` tracing CREW over FUTURE | `788e0a7ee7d0f58e3865443a1edc7b6f6ebfd37ed9f1aca77a5aa59ac43cfc19` |
| 700 | 1 / phase 3 | `dots3` tracing RULES over CREW | `9574495dda2d6aa237bffd29f01ce376a4c7317abe30e16a78f0c99fc572e920` |
| 770 | 1 / phase 4 | `dots4` tracing FOREVER over RULES | `cbdf6e0cb30941281d2bc00ce45b878995f0222ae46b0fe1fad10b7535a331e2` |
| 840 | 3 | FOREVER held, dots dead | `f2718de8cb4357740cdc467cc13a511a56e3415bd47c2f6b841b944264c016c7` |
| 910 | 4 | the grey/blue band | `59f2d5fc1cc9d6839344acbc230e8b9c7c2c20457a4fbf3b7ea9ef8c8985caa2` |
| 1050 | 5 | part 2's 3-D dot grid | `c1743f6af0375ecb76c4f939724b7f2b1897a2665af9c93f44cabb7aa76a9367` |
| 1600 | 6 | part 3's glenz cube | `5894039bea2564ce78bde3c479c80a986f144f59d76af15919e7a238a1b9858d` |

70–550 apart, one per stage. `840` is the one held keyframe whose indexed read reproduced its
RGB, so it carries `frame_000840.idx` and `idx_sha256`; `480` holds too, but its read landed
after mode 0 had ended in the filed run and kept RGB only. The seven moving frames are
RGB-only (D15/D19).

### Reproduced

480 and 1600 (the two most different pictures) were captured again in a second session
(`out/task2/hard-repeat-480-1600`) and are byte-identical to the filed `1930616d…` and
`5894039b…`; a separate nine-frame trial session is byte-identical at all nine indices too.
`cargo xtask reference verify --tree reference` passes over the capture.
