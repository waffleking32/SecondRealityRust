# Reference indices — one row per part, and how the indices are found

The reference ledger. §1 is the method and the policy: how a capture is taken, what an index under
`reference/` means, how held and moving keyframes are told apart and checked against sessions, what
the oracle cannot see, and which keyframes are not settled. §2 is the one-row-per-part table, and
§§3–27 are one compact ledger per capture: the keyframes, what is on screen at each, held or moving,
the support each has across sessions, where the part begins and ends, and the source lines that
explain it.

The ledger records what the **original** does. What the port draws against these frames is owned
elsewhere: the per-frame counts by `cargo xtask rediff` (`xtask/src/rediff/table.rs`), the
mechanism by each part's module doc, the measured agreement by the per-part note
`docs/fidelity/<part>.md`, and each residual's argument by `APPROXIMATIONS.md`.

**Digests.** Every frame file's `sha256` is in its capture's `manifest.json` (`sha256` for the RGB
frame, `idx_sha256` for the index plane), and `cargo xtask reference verify --tree reference`
checks them (D23). This ledger quotes abbreviated hashes (`05905a2c…`) only to tell pictures
apart, including pictures from scratch sessions that are not filed.

**The recipe every capture of the demo uses** is in the manifests (the loop seam adds three menu
keys, §25.1):

```
--arg <block> --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000
```

The vault parts use their own route (§24). Indices are **boot-relative**: frame 0 is the
emulator's first rendered frame at process start (D14), the setup screen's text is up at 16, and
the demo's first graphics frame is 404. Session counts in this ledger come from scratch captures of
the same recipe (not in git); where a number is the only record of a measurement, it is kept.

---

## 1. The method

### 1.1 Sweep, then look at the sheet

```
cargo xtask reference capture --arg 2 --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames <a dozen ascending indices> --out out/<part>-sweep --sheet out/<part>-sweep.png
```

A dozen indices across the seconds the part is expected to occupy, written to scratch, **never into
`reference/`**. The capture prints each tile's index as it lays it out (`r0c0 frame 430
(frame_000430.rgb)`). Read three things off the sheet: where the part's picture starts, where it
holds still, and where it stops being this part. A sweep that crosses a part boundary never
succeeds (§1.2), so the next part's first frames come from the refusal's index and from a sweep
inside that part.

### 1.2 Where a part ends: the refusal is the boundary

A capture is one geometry (D15). A sweep that runs into a part of another geometry stops at the
first index of the new one:

```
error: the display changed geometry between frames: frame 1170 is 320x200 where the capture
started at 320x400; capture one screen at a time
exit: 2
```

The change lies between the last index that captured and the one that was refused.

- **A refused capture writes no manifest.** Frames taken before the refusal are left on disk, but
  nothing in that directory is a capture.
- **Indices just below the change are grab-hostile.** The changeover stalls the renderer, and an
  index in that window is refused for lateness (BEGLOGO: 1155, `the emulator reached frame 1156
  before frame 1155 could be captured`; 1158 twice; 444 in a dense probe of the fade). Narrow a
  boundary with **sparse single-index probes from both sides**: one miss refuses a whole dense run.

Where two consecutive parts share a geometry, no refusal marks the change, and the sheet (or a
dense scan matched against a known hash) is the discovery instead (§1.6).

### 1.3 Two rules for a list of indices

**A run is read after it is taken.** Indices closer together than `INDEXED_READ_FRAMES` (25) are
all grabbed before any is read (D19). One indexed read costs ~20 rendered frames (the DAC palette
is 1024 of its requests), so a run of *k* indices spends **~20·*k*** frames reading afterwards, and
the next index must be further away than that. Measured at 30000 cycles: `--frames
429,430,431,479,…` captures the three and refuses 479 (`reached frame 488`), so three reads cost
57 frames.

**A read's palette is the palette at read time, not at grab time.** The pixels are the ones taken,
but the 256 DAC entries are read when the run is read, so a palette that moves in between makes the
pair incoherent and D15's rule keeps the frame RGB-only. **A screen whose palette is fading is
always RGB-only**, however still its indices; and **a frame inside a batch is read at the end of
the batch**. "rgb only" means the indices would not have been honest; it never means the RGB frame
is the wrong frame.

**The reference holds keyframes only; the capture compared against it holds the supports** (D19).
`compare` pairs *every* frame of the reference, so a support frame in the reference would itself be
a reference index. Two rules for the partner:

1. **Each keyframe that moves needs a support on each side.** A session's phase is uniform across a
   capture (D17), so a partner one step out has to answer *every* moving keyframe from its
   neighbour. The partner holds `K-1, K, K+1` around each keyframe that changes frame to frame. The
   ends-only form (`--frames <i-1,i,…,k,k+1>`) is enough only for a contiguous range.
2. **A keyframe whose neighbours are the same picture needs no support.** Its ±1 candidates are
   the same bytes, so the search finds offset 0 whatever the partner's phase. This is measured per
   keyframe, not assumed.

Because a keyframe's three supports are one batch, **consecutive supported keyframes have to be
~60 frames apart** in the partner's list (the 57-frame cost above).

### 1.4 What "held" means, and why keyframes are chosen there

A keyframe is *held* when the frames either side of it are the same picture. That is where the
reference reproduces best and where a port's diff means most. A palette fade over a still image is
held in the index plane and moving in the palette, and its frames are RGB-only (§1.3). Held is
checked, not assumed: the triple `K-1, K, K+1` is hashed in each session, and a shared index taken
by two runs is the same bytes or it is not.

### 1.5 Cycles

Every capture of the demo is at `--cycles 30000` (D17). The cycles change how long a capture takes
in wall time and nothing about its content: the held index 900 captured at 25000 and at 30000 is
byte-identical (`frame_000900.rgb` `cbdc67eb…`, `frame_000900.idx` `e10481c7…`), and `compare`
reports `verdict: identical`, offset 0. One counted frame is one emulated vertical refresh whatever
the cycles.

### 1.6 The read's row stride, the flat-palette blind spot, pans and content boundaries

**The read takes the row stride from CRTC 0x13 and refuses a screen it cannot describe**
(`xtask/src/oracle/dosbox.rs`, `row_stride_bytes` and `capture_indexed`). A 256-colour screen draws
a line every `scan_len * 8` bytes of the emulator's address space (`vga_draw.cpp:1785-1792`), and a
guest plane byte at offset `o` sits at four-byte group `o` (`vga_memory.cpp:129-135`), so one plane
row is `scan_len * 2` bytes: 80 for mode 13h's offset 40, 176 for ALKU's 88 (`ALKU/TWEAK.ASM:46`
writes CRTC 0x13 = `0x5813`; `ALKU/MAIN.C:26` strides its rows by 176). The *mode* comes from the
emulator's own colour depth (`webserver/video.cpp`, `video_mode.color_depth`): only a 256-colour
screen is one index a byte, so a 16-colour screen is refused (`UnmodelledMode`) rather than
assembled at a wrong stride. When `stride × height` exceeds one plane (ALKU: 176 × 400 = 70,400 >
65,536) the read is refused with `PlaneTooSmall`, a message that names the stride. **A missing
`.idx` is honest; a sheared one is not.** Every `.idx` in the tree reproduces its RGB only at the
stride the part writes: 80 for `check_a`, BEGLOGO, U2A and PAM (PAM's `tw_opengraph`,
`PAM/TWEAK.ASM:18-41`, never writes 0x13).

**The coherence check is blind on a collapsed palette.** `indexed_matches_rgb` compares colours, so
a frame whose every index widens to one colour passes at any stride. A sheared ALKU 900 plane (34.8 %
of covered pixels wrong, the picture at row 110 where `ALKU/MAIN.C:165-166` blits it at row 50)
passes it. **The check cannot catch a stride error on a screen whose
indices all map to one colour.** A part whose `.idx` is refused is compared in `mode: rgb`, where
the 6→8-bit widening `(c*259+33)>>6` is load-bearing (D14).

**A moving screen's session-to-session difference can be an exact integer pan, which no ±1 frame
support can bridge.** ALKU's picture advances one pixel per 9-frame step and holds the same bytes in
between (`ALKU/MAIN.C:7` `#define SCRLF 9`, `:87`), so sessions agree **after a whole number of
pixels**: the reference's 5859 is 0 px of 95,700 from one session after a 1-pixel shift and from
another after a 2-pixel shift, against 10.975 % and 15.2797 % at offset 0 (§4.4). The row records
the pan rather than hiding it.

**A part boundary is not always a geometry change.** U2A and PAM are both 320×200, so no refusal
marks the change; the sweeps' refusals at 7950 and 8400 belong to the part after PAM. A dense scan
plus a hash already known to belong to the next part settles it: §6.1 finds PAM's first frame by
matching a flat white frame against `wfade`'s own law, and §6.5 finds PAM's end by matching a held
logo against BEGLOGO's hash.

### 1.7 The racy register read, the phase spread, and the music clock

**The CRTC offset register is read as two port operations with the guest running in between, so it
can return another register's value.** `crtc_offset_register()` writes the index to 3d4h and reads
3d5h in a separate request, and `chain4_enabled()` does the same at 3c4h/3c5h. On TECHNO's 320×400
screen nine `PlaneTooSmall` refusals name a 320-byte row on 8700…8780, where two other sessions refuse
the same 8700 only for motion with byte-identical RGB: the value read, **160 = 0xa0, is `plv * 0x20`
for `plv = 5`**, one of the values `TECHNO/KOE.C:385-389`, `:445-449` and `:573-577` write to CRTC
0x0c (§9.6). **A stride refusal in a log is not by itself evidence that the screen is unreadable.**
The quiet half of the race is §1.9's. The stride rule itself is confirmed on GLENZ's 92-byte rows
(§7.3).

**`.idx` presence is not predictable from the frame list.** On GLENZ the held picture carried indices
in four captures and not in a fifth, and every moving keyframe came back RGB-only (§7.3). Capture,
look at what came back, and do not space indices on the strength of a rule.

**A screen can be refused for its colour depth** (`UnmodelledMode`): TECHNO's interference pokes the
mode registers between VGA 256-colour and EGA 16-colour and DOSBox re-decides the mode from them
(`vga.cpp:60-96`, `VGA_DetermineMode`: `M_VGA` needs GC mode bit 6); so are ENDSCRL and DDSTARS
(§9, §21.5, §22.5).

**A moving screen's phase differs between sessions, and the reported percentage is a lower bound on
that difference, not a measure of it.**

- Divided by the screen's own per-frame rate, the disagreement is a handful of frames: GLENZ's
  polyhedron moves 10.64–25.43 % of its pixels a frame and same-index differences are
  22.83–34.67 % (≈1.5–2.5 frames); TUNNELI's dots move 2.05–10.69 % a frame and differ 8.23–10.29 %
  (≈1–4 frames).
- Some screens saturate: TECHNO's interference moves **89.10–98.69 % of pixels in one frame**, so a
  97.5 % difference there is a one-frame statement (§9.4).
- `compare`'s offset is the window it searched (±1), not the gap between sessions. Screens quantise
  into a few discrete results: TUNNELI 3800 is byte-identical in five of seven sessions, with the
  others ≈0, ≈8 and ≈20–30 frames apart.

**The music clock.** The demo's frame clock is the VGA retrace (`MAIN/COPPER.ASM:300-320`
reprograms the PIT, polls 3DAh and increments `copperframecount`, which `DIS/DISINT.ASM:161-181`
hands back from `waitb`), so the guest's frames and the capture's counter are one clock; the music
is the only other clock in the machine. The music player's frame counter is the 16-bit word at
**physical address `0x105B0E`**, in the DOS extender's data, advancing **0.7424 a frame** (three
sessions: 0.742016 / 0.742012 / 0.741909, ~52 Hz). At video frame 900 it reads **371, 373, 374** in
three sessions and at ~8103 **5717, 5716, 5715**: a spread of ~1 music frame (≈1.3 video frames),
the rate identical to 1e-4. That word is not `_np_zframe` (`music-clock.md` §8).

So a music-gated part *starts* a frame or two either side, and a part that then advances by frame
count inherits that as a constant offset (GLENZ's ≈1.5–2.5, TUNNELI's ≈1–4). A music-gated
*boundary* is pinned to the music: the mode changes sit within 4 frames across four sessions
(BEGLOGO 408/409/406/406; GLENZ 1163/1161/1162/1162, which §3.4's probes bound as (1154, 1160], two
frames of request-to-grab slop, D19; TECHNO's 16-colour 4799/4799/4797/4801; TECHNO's 320×400
8194/8195/8196/8193; PANICEND 8784 ×4), and the tunnel→TECHNO content boundary (a black plateau at
4774–4793, the rings at 4797) is within 1–3 frames in two more. Music-gated *events* inside a part
(TUNNELI's tunnel end, the TECHNO→PANICEND change) differ by tens of frames between sessions (§8.5,
§10.1). The rare excursion is §1.12's.

For the rows: a *held* frame is reproducible byte for byte in every session seen; a *moving* frame
is one exact picture the oracle reproduces when two sessions agree, and the row records the support
rather than a tolerance.

**A two-screen part is two captures.** Every directory below a tree that holds a `manifest.json`
is a capture, named by its path below the tree (`capture::capture_directories`, the
`reference/check_a` layout): `techno/{interference,troll}`, `plzpart/{plasma,cube}`.

### 1.8 The census rule: a moving keyframe is checked against sessions, or it is a phantom diff

Sessions do not sample a moving screen continuously. They land in **discrete alignment groups** of
very unequal size, and a reference drawn from a minority group is a target a port misses for a
reason that is the oracle's. The rule:

1. **Count only captures that hold `N-1`, `N` and `N+1`.** A single-index probe answers only
   offset 0 and inflates disagreement.
2. **For each picture, count the sessions that present it at `N-1`, `N` or `N+1`.** This is a
   support count over the window, not a partition. **Exclude the reference's own capture**: counting
   it biases toward passing. Fixed-index grouping under-counts (GLENZ 2300 is 3/6 by index, 6/9 over
   the window).
3. **File the picture with the largest support**, and require it to pair at offset 0 or ±1 with
   `0.0000 %` in at least half the sessions. The percentage is the discriminator, not the offset: a
   ±1 pairing with a large percentage is the window's edge. **The filed file must be a frame some
   session took at `N`**: a picture seen only at `N±1` cannot be filed at `N`.
4. **If the largest support is below half, record the keyframe as having no settled content.**
5. **A headline number comes from the support count over sessions, never from a single partner
   taken in the same sitting.** A partner captured minutes after an in-place re-capture can be one
   frame behind it and report 0.0000 % for a picture most sessions never present (GLENZ: a
   same-sitting partner read "34.32 % → 0.0000 %" for a re-capture that an independent partner showed
   two frames out at all three indices).

**The census of the `SECOND 2` moving keyframes**, over the 131 session directories of this recipe,
the reference tree excluded:

| keyframe | sessions with a window | pictures at `N` | largest support | filed reference is it? | ≥ half? |
|---|---|---|---|---|---|
| GLENZ 2300 | 9 | 9 | `dbe96c0a…` 6/9 (67 %) | yes | yes |
| GLENZ 3000 | 21 | 15 | `05905a2c…` 8/21 (38 %) | yes | **no** — no settled content |
| GLENZ 3400 | 10 | 13 | `d02e362e…` 3/10 (30 %) | yes | **no** — no settled content |
| TUNNELI 3800 | 9 | 7 | `f861b1f2…` 7/9 (78 %) | yes | yes |
| TUNNELI 4200 | 6 | 9 | `af4763b3…` 3/6 (50 %) | yes (tied) | exactly half |
| TUNNELI 4500 | 5 | 10 | `cb15de2d…` 2/5 (40 %) | yes (tied) | **no** |
| TUNNELI 4600 | 6 | 14 | `16f8c1fd…` 1/6 (17 %) | yes (tied) | **no** |
| TECHNO-A 4900 | 4 | 6 | `3dc29660…` 3/4 (75 %) | yes | yes |
| TECHNO-A 5000 | 5 | 1 | `20f3d62f…` 5/5 (100 %) | yes | yes |
| TECHNO-A 6300 | 6 | 6 | `3c2b0d8f…` 5/6 (83 %) | yes | yes |
| TECHNO-A 8100 | 4 | 6 | `6033bbc4…` 3/4 (75 %) | yes | yes |
| TECHNO-B 8300 | 4 | 4 | `17b9f92f…` 3/4 (75 %) | yes | yes |
| PANICEND 8792 | 1 (+2 refused/content) | 3 | `c9bf5114…` 1/1 | yes (tied) | trivially |

Held keyframes need none of this: GLENZ 1499 is one picture in 7/7 sessions, TECHNO 8200 and 8400 in
5/5, PANICEND 8800 and 9000 in 3/3.

**Byte-identity is a knife edge on fast screens.** `05905a2c…` (7 sessions) and `dd85288b…` (2) at
GLENZ 3000 are **19 pixels apart**: one phase split in two by byte-identity. A group of one or two
there means "within a few pixels of its neighbour", and the support fractions measure the oracle's
phase spread, not a port's reachability.

Filing the largest-support picture is what moved the rows: TUNNELI from 8.07–10.18 % on all four
keyframes to 0.0000 % on three and 0.0891 % (57 px) on 4600; TECHNO's 320×200 screen from
24.59–72.87 % on 4900/6300/8100 to `exit 0, verdict: identical`; GLENZ 2300, 3000 and 3400 to
byte-identical at offset 0 against an independent partner.

### 1.9 The split, the quiet half of the race, and fast screens

**The split's `+ 1` is measured.** `pompota` (`PLZPART/COPPER.ASM:126-143`) alternates CRTC 0x18
between `0x3c` (60) and `0x3d` (61) frame by frame, and three consecutive captured frames show the
lower view's first content row at **62, 61, 62**: `Split::scanline` is the register plus one (D16,
§13.2). The register is also animated by `do_drop` (`COPPER.ASM:189-268`).

**The race has a quiet outcome as well as a loud one.** PLZPART's 384-byte refusal has two readings
the capture cannot separate: CRTC 0x0d's `0xC0` read as the offset, or the colour depth misread as
chain-4 (§13.4). Under the first, a raced value can **pass** `plan_indexed_read` and be used:
`pompota`'s 0x3c would give an accepted 120-byte row, with only the coherence check (blind on a flat
palette, §1.6) between it and a sheared `.idx`; 3DSINFLD's 0x80 is the same case (§1.10). **Any `.idx`
filed from a part that writes CRTC registers is read by hand before it is trusted.** The durable fix
is in the oracle: read the index/data pair coherently, or twice and require agreement.

**On a fast-moving screen the census measures the last decimal of the phase.** PLZPART's plasma is
byte-identical in some sessions and 118–3942 px of 128,000 away in others, a thousandth to a
twenty-third of a frame on a screen that moves 70 % a frame (§13.5). The count reads "no settled
content" while the pictures are within a few hundred pixels, and the row says so.

### 1.10 Refused reads, collapsing palettes, discrete holds and boots

**A capture refused for geometry still performs the read for the index it refuses.** The loop grabs
and reads a frame (`reference.rs`, `oracle.frame()`) before comparing its geometry, so a warning in
the log can belong to an index that never landed.

**The race is live on every planar part that writes CRTC 0x0c** (3DSINFLD's page flip, JPLOGO's
`scrolly`, U2E's `vid_setswitch`: §16, §17.4, §18.1). 0x80 read as the offset register is a 256-byte
row, which `plan_indexed_read` accepts at 200 rows (51,200 ≤ 65,536). No `.idx` from these parts is
filed.

**A `.idx` can be honest and still weak evidence when the palette partly collapses.** A 3DSINFLD
sweep frame (15100, not filed) has no index carrying two triples, but its **83 indices reach only 58
colours**, because the palette formula (`COMAN/MAIN.C:128-150`) rounds entries together; LNS&ZOOM
2700's 64 indices map to 64 triples (§12.2).

**A keyframe can be held without being reproducible, and a hold's states can be discrete.** JPLOGO's
~350-frame hold is one `sha256` within a session, but twelve captures hold ten distinct images at 17700
(§17.3). Look for the settle branch in a part's loop before assuming a moving screen has no held
picture, and census the hold across sessions.

**A screen's per-frame change is not stationary.** RAYSCRL's 12999→13000 changes **64 px (0.10 %)**
and 13000→13001 **23,908 px (37.36 %)**; 3DSINFLD alternates **0 px and 37–40 %** because each
render costs two vblanks. Neither "held or moving" nor a rate carries over from another index of the
same part.

**The census remedy works when applied to a support count**: 3DSINFLD 15500 went 1/3 → 3/3 (§16.4)
and U2E 2600 0/4 → 3/4 (§18.5), both judged against the same independent sessions.

**A boot reaches the parts its `partsmask` byte selects.** The entries are TASM binary literals
(`MAIN/U2.ASM:110-119`: `db 11100b ;3`), so `SECOND 3` selects bits 2, 3 and 4, and bit 3 is U2E. What
changes between boots is the *index*, not the screen (§18.1, §18.6). A row records which arguments
reach the part and at which indices.

### 1.11 The end sequence: adjacent grabs, the start address, and wrong-boot identity

**`SECOND 5` starts the end sequence at its own beginning** (`partsmask[5]` = `10000b`,
`MAIN/U2.ASM:116`; the shell's preparation at `:898-909`): ENDLOGO is 320×400 to **(1200, 1240]**,
CRED 320×200 to **(8840, 8940]**, ENDSCRL 640×400 (mode 0eh, 16 colours, CRTC 9 = 0) from there
(§§19–21).

**The grab loop cannot take two adjacent frames on the 640×400 screen.** `--frames 9099,9100,9101`
is refused at the first batch in every session tried (`the emulator reached frame 9101 before frame
9100 could be captured`), and so is `9099,9100`. A two-frame gap works once but not in a three-index
batch; a **three-frame gap works every time** (`9097,9100,9103`). The same shapes are fine on 320×200
and on ENDLOGO's 320×400, and failed once in three sessions on DDSTARS's 320×400 (at 5001), so the
cause is the per-grab cost (a counter read, a 768,000-byte `format=raw` frame, a counter read) crossing
one emulated frame's wall time. Core pinning does not help: the grabs are served by the emulator's own
process. ENDSCRL's census is therefore an offset-0 count (§1.12, §21.4).

**`looping` starts at 0** (`m_looping = 0`, `MAIN/MENU.C:30`, `:233`), the recipe's keys leave it, and
`cmp cs:looping,0 / jne @@skipp6` (`MAIN/U2.ASM:914-915`) runs ENDSCRL: its reference is valid for a
non-looping session only.

**ENDSCRL and DDSTARS are captured as windows.** ENDSCRL's source loops `while(!dis_exit())` (the
release's build ends at its text's last line, D49, long after the captured 20000), and DDSTARS ends
only on DIS function 2, which only a key sets; DDSTARS is still drawing at 20000.

**A still screen with a still palette can be refused an `.idx`**: all 82 CRED frames are refused by
the coherence check while every keyframe is held. The read's model of where the screen starts
(`capture_indexed` reads each plane from offset 0) is the suspect (§20.2).

**Byte-identity is not identity of a screen.** BEGLOGO's 430 on a `SECOND 2` boot and ENDLOGO's 430
on a `SECOND 5` boot are one `sha256` (`reference/parts/beglogo/frame_000430.rgb` and
`reference/parts/endlogo/frame_000430.rgb`), by different mechanisms: `END/END.C:62-63` writes 63 765
times from DAC index 0 (entries 0–254; **255 is not written**), while `BEG/BEG.C:25-33` writes 63 into
0–254 and black into 255, and BEGLOGO's white screen is index 15 (`:20-21`). Unfiltered, ENDLOGO's
windows had ten wrong-boot holders (three BEGLOGO sessions at 429–431, six MNTSCRL sessions at 499–501
holding `e68aa887…`, one ALKU session at 899–901 holding `fec9afb5…`). **Census counts are filtered by
recipe**: a session counts only when its manifest names the same `arguments`, `boot`, `cycles` and
`keys`.

**Parts anchored to one music cue draw identical frames at identical indices; parts that start on a
music wait drift.** Every keyframe of the end sequence reproduces at its own boot index across
separate sessions, as MNTSCRL, LNS&ZOOM, MINVBALL, RAYSCRL and the PLZPART cube do on `SECOND 3`
(their timelines anchored to one cue, then advanced by `dis_waitb()` counts). The music-gated `SECOND 2`
parts needed a state census to decide what to file; these needed only a count proving the filing was
taken at `N`.

### 1.12 The non-determinism policy — what an index means, and what is not settled

**What a capture index means.** A keyframe index is a **boot-relative emulator frame counter**:
frame 0 is the emulator's first rendered frame at process start (D14), the counter counts emulated
frames (`GFX_EndUpdate`, D17), the setup screen is up at 16 and the first graphics frame is 404. An
index names a screen only as precisely as the boot repeats, and it repeats exactly under one recipe.

**A capture index is valid only under the recipe, at the pinned cycles, with the silent emulator**
(`SDL_AUDIODRIVER=dummy` + `mixer_nosound=true`). `--boot autoexec` puts the mount, the command line
and the setup answer on the emulated clock (the guest's `[autoexec]`; `AUTOTYPE` schedules through
`PIC_AddEvent`), so the same index is the same screen in five runs of five, where the typed recipe's
position is the host's timing (43–45 frames between two runs under load). **Silence is required**:
with a real audio device the mixer follows the host's clock and the animation drifts against the
counter (two autoexec captures differed 14.3/35.3/24.6 % of bytes until the emulator was also silent,
D17). A capture that does not pin cycles is refused. Every manifest records `boot`, `cycles`, `keys`
and `arguments`.

**What is reproducible.**

- **The boot repeats.** Menu frame 16, first graphics frame 404, five runs in five (D17).
- **A held screen repeats byte for byte**, within and across sessions (BEGLOGO 900 in five sessions;
  CRED's five holds with `K-1 = K = K+1` in four). Re-run a day apart in fresh sessions, BEGLOGO and
  ALKU (5859 included, landing on the reference's own pan phase) gave `exit 0, verdict: identical`,
  every filed file `sha256`-equal to the new capture.
- **A moving screen's alignment is not reproducible across sessions.** Sessions land in discrete
  alignment groups, and a second capture lands in the same group about half the time (D17). DOSBox
  subtracts measured host present time from its tick budget (`gui/sdl_gui.cpp:2555-2570`), so the
  emulated clock carries a host-timing term no API or `--set` reaches.

**The comparison searches ±1 frame and reports the offset; it never tolerates pixels.** `compare`
tries `N-1`, `N` and `N+1`, pairs the candidate with the fewest differing pixels, and records the
signed offset and the capture's offset distribution (D18). A paired frame is 0.0000 % or it is a
difference; a reference index with no candidate in the window is refused, exit 2. The window is a
constant (`ALIGNMENT_WINDOW = 1`). `compare` prints `captures: N measured` before `verdict:`, so a
partial measurement is not read as a complete one.

**The design's structural-similarity fallback (`docs/design.md` §8.4) was not needed.** The ±1
search keeps the comparison pixel-exact and moves only which rendered frame answers; structural
similarity would replace "the same" with "similar enough", a judgement the port cannot audit. Where
the window cannot reach a difference, the row records the residue and says what it is (ALKU's pan,
§4.4; the no-settled-content rows below).

**How a part that does not reproduce is handled, in order.**

1. **Census it by §1.8's rule and re-capture into the largest alignment group**, judged against
   independent sessions, never a same-sitting partner. This is what fixed TUNNELI, TECHNO's 320×200
   row, 3DSINFLD 16500 (from 38.9031 % to `verdict: identical`, §16.4) and U2E 2600 (0/4 → 3/4).
2. **If the largest support is below half**, the keyframe has no settled content; the best filable
   picture stays filed and the residual is the oracle's.
3. **If the picture is exact but its phase is out of the window's reach**, the row records the
   measurement (ALKU 5859's whole-pixel pan, §4.4).
4. **If the ±1 window cannot be captured at all**, the census is an offset-0 count and the row names
   the reason (ENDSCRL, below).
5. **Never**: a pixel tolerance, a widened window, a structural-similarity substitution, or an
   unrecorded re-roll.

**The keyframes with no settled content** (the largest support over the window is below half the
sessions). A faithful port gets a non-zero diff at these against the filed picture, and that residual
is the oracle's:

| keyframe | sessions with a window | support of the filed picture | why |
|---|---|---|---|
| GLENZ 3000 | 21 | 8/21 (38 %) | the phase is split finer than byte-identity: `05905a2c…` and `dd85288b…` are 19 pixels apart (§1.8) |
| GLENZ 3400 | 10 | 3/10 (30 %) | same screen, same knife edge |
| TUNNELI 4500 | 5 | 2/5 (40 %) | the dotted tunnel quantises into several groups |
| TUNNELI 4600 | 6 | 1/6 (17 %) | as above; the partner comparison is 0.0891 % (57 px) |
| PLZPART plasma 6000 | 5 | 2/5 (40 %) | the plasma moves 70 % a frame; the largest window support (3/5) is a picture every session takes only at `N-1`, which §1.8 rule 3 forbids filing at `N` |
| PLZPART plasma 7000 | 4 | 0/4 | every picture taken at `N` is a singleton |
| PLZPART plasma 7600 | 3 | 0/3 | nine pictures, 1/3 each |
| JPLOGO 17700 | 4 | 2/4 for the *state*, 0/4 for the *file* | the state two sessions hold (`ec3de2bb…`) was drawn by none of seven re-capture probes, and the filed `91d6ce6b…` is taken at `N` by none of the four (§17.5) |

TUNNELI 4200 and JPLOGO 17500 sit exactly at half (3/6 and 2/4); one more session can move either.

**Unresolved: the oracle's rare excursion.** One session had TECHNO's rings ~30–40 frames early (on
screen by 4770 where two VGA-content probes put them at 4797). It did not reproduce, and it is not
the music clock (the `0x105B0E` counter of §1.7 agrees to ≈1.3 video frames across sessions), not a
fixed offset, and not a counter stall (no gap ≥150 ms at 20 ms resolution up to frame 4900, 0.0701
frames/ms). The ordinary spread is pinned (§1.7). The census counts sessions that hold the window,
so an excursion does not decide a reference; do not reopen the mechanism without a reproducing case.

**Unresolved: ENDSCRL's offset-0 census** (§1.11, §21.4): 9/9 sessions byte-identical at 9100 and
5/5 at each other keyframe. §1.8 rule 1's warning is about sessions that differ at `N`; here none do.
The cost: **a one-frame-out diff at these keyframes is a difference, not an alignment** (a
neighbouring line of scroll text, 1.2–7.4 % at a three-frame step), and nothing in the tree proves a
±1 pairing would have matched.

**Unresolved (tool-side): the CRTC register read race** (§1.7, §1.9, §1.10). The loud half refuses a
frame; the quiet half can pass `plan_indexed_read` and be used, and the coherence check is blind on a
collapsed palette (§1.6). Nothing in the tree is wrong because of it: every `.idx` filed from a part
that writes CRTC registers was read by hand, and the rows say what the read showed. The fix belongs
in the oracle (read the pair coherently, or twice and require agreement).

### 1.13 Fit corpora — a third kind of reference

A **fit corpus** is a run of reference frames used to determine a constant or a phase by fitting a
model over them; it is neither a keyframe nor a trace (D19). The tree holds one,
`reference/parts/plzpart/band/`, six frames **8400, 8450, 8500, 8600, 8700, 8800**:

```bash
cargo xtask reference capture --arg 3 --boot autoexec \
    --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames 8400,8450,8500,8600,8700,8800 --out reference/parts/plzpart/band
```

It is A23's whole-frame evidence (the counts are `cargo xtask rediff --only plzpart/band`'s), and
its 8500 frame pins the phase state (frames 136, page 4, display start 0x0028, music 68:46). The
200-line frames show row ⌊2y/3⌋ of a 134-row image (CRTC 0x09 = 2, `TWEAK.ASM:31-33`). Its manifest's
digests are checked by `reference verify` like any capture (D23).

Its indices are **not** keyframes: nothing pairs the port against them with `compare`, they are not
rows of §2, and a difference at one is a fit residual. It is committed rather than left in scratch
because a fit a clone cannot re-run is not evidence (D25).

## 2. The index table

One row per captured screen (a part spanning two geometries has a row per screen). The support
column is the census of §1.8 over independent sessions of the recipe, the reference excluded. The
capture's section has the keyframes, what is on screen and the boundaries. Every reference in this
table is one command, keyframes only:

```bash
cargo xtask reference capture --arg <boot> --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames <keyframes> --out reference/parts/<part>
```

with the row's boot and keyframes; a two-screen part's screens go to their subdirectories
(`techno/interference`, `techno/troll`, `plzpart/plasma`, `plzpart/cube`). The sessions compared
against it hold `K-1, K, K+1` around each moving keyframe (§1.3) unless the section says otherwise.

| part | boot | geometry | keyframes | re-capture against independent sessions | support at `N` |
|---|---|---|---|---|---|
| BEGLOGO | `SECOND 2` | 320×400 Mode X | 430, 470, 560, 900 | exit 0, identical, `{0: 4}` | held; the two fade keyframes byte-identical across sessions (§3) |
| ALKU | `SECOND 1` | 320×400 Mode X, 176-byte row stride; RGB only | 900, 1719, 2719, 5859 | exit 1, `mode: rgb`: three held identical; 5859 a 2-pixel pan (15.2797 % at offset 0, 0 px once shifted) | held ×3; 5859 an integer pan (§4.4) |
| U2A | `SECOND 1` | 320×200 | 6500, 7300, 7640 | exit 0, identical, `{0: 3}` | the moving keyframe in the same group (§5.4) |
| PAM | `SECOND 1` | 320×200 | 7710, 7760, 7830, 7900 | exit 0, identical, `{0: 4}` | every frame (§6.4) |
| GLENZ | `SECOND 2` | 320×200, 92-byte plane row; 1499 carries `.idx` | 1499, 2300, 3000, 3400 | exit 1, `mode: mixed`: byte-identical at offset 0 against an independent partner; 6 and 65 px at 3000/3400 against another | 1499 7/7; 2300 6/9; **3000 8/21, 3400 3/10 no settled content** (§7.4) |
| TUNNELI | `SECOND 2` | 320×200; RGB only | 3800, 4200, 4500, 4600 | exit 1, `mode: rgb`: three identical, 4600 0.0891 % (57 px) | 7/9, 3/6, **2/5, 1/6 no settled content** (§8.4) |
| TECHNO — 320×200 | `SECOND 2` | 320×200, 16-colour EGA mode; RGB only | 4900, 5000, 6300, 8100 | exit 0, identical, `{0: 4}` | 3/4, 5/5, 5/6, 3/4 (§9.4) |
| TECHNO — 320×400 | `SECOND 2` | 320×400 Mode X, 160-byte plane row; 8200 carries `.idx` | 8200, 8300, 8400 | exit 1, `mode: mixed`: 8200, 8400 identical; 8300 (the fade-in) differs at offset −1 | held 5/5 ×2; 8300 3/4 (§9.5) |
| PANICEND | `SECOND 2` | 320×200; 9000 carries `.idx` | 8792, 8800, 9000 | exit 0, identical, `{0: 3}` | 8792 1/1 (+2 refused); held 3/3 (§10.3) |
| MNTSCRL | `SECOND 3` | 320×200 Mode 13h; 500 carries `.idx` | 500, 1100, 1700, 2300 | exit 0, identical, `{0: 4}`, three sessions | held 8/8; moving 6/6 each (§11.4) |
| LNS&ZOOM | `SECOND 3` | 320×200 Mode 13h; 2700 and 5650 carry `.idx` | 2700, 3400, 4200, 4900, 5650 | exit 0, identical, `{0: 5}`, three sessions | 5/5 each (§12.4) |
| PLZPART — plasma | `SECOND 3` | 320×400 Mode X, 96-byte plane row, line compare 60/61; RGB only | 6000, 7000, 7600 | exit 1, `mode: rgb`: 0–0.16 % at 6000, 7.7–12.9 % at 7000, 9.3–24.5 % at 7600, max delta ≤ 25 | **2/5, 0/4, 0/3 no settled content** (§13.6) |
| PLZPART — cube | `SECOND 3` | 320×200 Mode X, 160-byte plane row; RGB only | 8500, 9500 | exit 0, identical, `{0: 2}`, four sessions | 4/4 (§13.6) |
| MINVBALL | `SECOND 3` | 320×200 Mode 13h; RGB only | 10900, 11200, 11800, 12400 | exit 0, identical; one session pairs all four at offset −1 | 3/3 each (§14.4) |
| RAYSCRL | `SECOND 3` | 320×200 Mode 13h; 13500 carries `.idx` | 13000, 13500, 14000, 14500 | exit 0, identical, `{0: 4}`, four sessions | 4/4 each (§15.4) |
| 3DSINFLD | `SECOND 3` | 320×200, chain-4 off, 80-byte rows; RGB only | 15500, 16000, 16500, 16800 | exit 0, identical against two sessions; exit 1 against a third two steps out | 3/3, 3/3, 3/4, 2/3 (§16.4) |
| JPLOGO | `SECOND 3` | 320×400 Mode X, 80-byte rows; RGB only | 17300, 17500, 17700 | exit 1, `mode: rgb`: 17300 identical in four sessions; 17500 in two | 4/4; 2/4; **17700 unfiled as a file** (§17.5) |
| U2E | `SECOND 4`; `SECOND 3` reaches it at its own indices | 320×200; RGB only | 1000, 1600, 2600, 3600 | exit 0, identical against an independent session; exit 1 against three, each missing one keyframe (a phase group) | 4/4, 3/4, 3/4, 3/4 (§18.5) |
| ENDLOGO | `SECOND 5`; a full run reaches it at its own indices | 320×400 Mode X, 80-byte rows; `.idx` at 430 and 900, only 900 index-compared | 430, 500, 900, 1180 | exit 0, identical, `{0: 4}`, `mode: mixed`, three sessions | 4/4 each (§19.4) |
| CRED | `SECOND 5`; a full run reaches it | 320×200, 160-byte rows (CRTC 0x13 = `0x5013`); RGB only | 1440, 3240, 5440, 7640, 8740 | exit 0, identical, `{0: 5}`, three sessions | held, 4/4 each (§20.3) |
| ENDSCRL | `SECOND 5`, non-looping (`MAIN/U2.ASM:914-915`); a full run reaches it | 640×400, mode 0eh 16 colours, CRTC 9 = 0; RGB only | 9100, 10000, 12000, 14000 | exit 0, identical, `{0: 4}`, three sessions | **offset-0 count**: 9/9, 5/5 ×3 (§21.4) |
| DDSTARS | `SECOND U` only (slot 17, `partsmask[8]` = `80h`) | 320×400 (mode 13h, CRTC 9 doubling cleared), 80-byte rows; RGB only | 1900, 3000, 5000, 8000 | exit 0, identical, `{0: 4}`, three sessions | 6/6, 5/5, 4/4, 5/5 (§22.4) |

The vault captures (GRID, TWIST, HARD), the loop seam, the full `SECOND 1` run and U2A's other CPU
speeds are §§24–27.

### Adding a row

1. Sweep and find the indices (§1.1, §1.2); pick keyframes at held or structurally significant
   moments (§1.4).
2. Capture the reference, keyframes only, into `reference/parts/<part>/`.
3. Capture independent sessions holding `K-1, K, K+1` around every keyframe that moves (§1.3).
4. Census each moving keyframe (§1.8) and re-capture into the largest group if needed.
5. `cargo xtask compare --reference reference/parts/<part> --rendered <session>`; record the exit
   code, offsets and support in the row and the part's section.

---

## 3. BEGLOGO

`SECOND 2`'s first part: the Future Crew logo, white, fading in over a held picture. 320×400 Mode X.
The part draws from the first graphics frame, 404, to the end of its fade in 571–579 (§3.3). The
screen then stands until GLENZ's opening collapses it from ~1115 and changes mode in (1154, 1160]
(§3.4, §3.5).

### 3.1 The keyframes

| index | what is on screen | files | rgb |
|---|---|---|---|
| 430 | the opening **white screen**: every pixel index 15, whose entry is `(63,63,63)`; white from 404 to the fade's first step; 429 and 431 are the same bytes | `.rgb` + `.idx` (1 entry) | `85789170…` |
| 470 | the **early fade**: logo pale, 27 colours, mean difference from the held picture 18.448 of the white's 23.815 (~23 % through the ramp) | `.rgb` only | `d0abd8cc…` |
| 560 | the **late fade**: ~93 % in (mean difference 1.718; 52 colours) | `.rgb` only | `05a130da…` |
| 900 | the **held logo**, 57 entries, held from 580 to 1110 | `.rgb` + `.idx` (57 entries) | `cbdc67eb…`, idx `e10481c7…` |

470 and 560 are RGB-only because the fade moves the palette while the read runs (§1.3), so the row
compares in `mode: mixed` and two of its four frames go through the RGB path, where the 6→8-bit
widening is load-bearing (D14).

Supports: one each side of the two fade keyframes; 430 and 900 are held and need none.

### 3.2 The re-capture

An independent session holding `430,469,470,471,559,560,561,900` gives exit 0, `verdict: identical`,
0.0000 %, offsets `{0: 4}`, `mode: mixed`; all four files `sha256`-identical. The held 900 is
byte-identical in five sessions. A re-capture a day later in a fresh session gives the same (§1.12).

### 3.3 The fade curve, measured

Sampled every 10 frames from 430 to 700 in one batch:

| index | 440 | 450 | 470 | 500 | 520 | 550 | 570 | 580 |
|---|---|---|---|---|---|---|---|---|
| distinct colours | 1 | 5 | 27 | 43 | 48 | 51 | 52 | 52 |
| mean difference from the held picture | 23.815 | 22.0 | 18.448 | 12.927 | 9.236 | 3.767 | 0.156 | 0 |

The ramp's first changing frame is in 441–449 and its last in 571–579 (570 differs by mean 0.156,
max channel delta 5). That is `BEG/BEG.C:80-85`: `for(c=0;c<=128;c++)` around `dis_waitb()` and a
`setpalarea`, ~129 steps, one per vertical blank. The picture is blitted once before the fade
(`BEG/BEG.C:73-78`); the white opening is `memset(vram,15,…)` and the all-white palette
(`BEG/BEG.C:20-33`).

Cross-session identity: 410, 429, 430, 431 one `sha256` in 4 captures; 470 and 560 byte-identical in
3; 690 and 1040 in 2 each. 1110 is identical in 2 of 3 captures and differs in the third on its last
scanline only (320 px, max delta 255): it is the collapse's first frame in that session, the only
shared index of the part that ever differed.

### 3.4 Where the part ends

A sweep of 1110, 1130, 1150, 1170 is refused at 1170 (320×200). Sparse probes: **1154 is still
320×400**, **1160 and 1164 are 320×200**; 1155 and 1158 cannot be grabbed (three attempts). The
change lies in **(1154, 1160]**.

From ~1115 the picture collapses: 1000–1110 are byte-identical to the held frame, and 1120, 1130,
1140 and 1150 all differ (128,000, 122,880, 106,240 and 77,760 px between consecutive samples). The
picture shrinks toward a band (rows 46–377 at 1130, 106–348 at 1140, a flat grey band at 191–307 at
1150) and the surround goes black. No keyframe sits in it.

### 3.5 What the collapse is: GLENZ's opening, not BEGLOGO's ending

The collapse is GLENZ's `zoomer2()`, run in the outgoing 320×400 geometry before GLENZ's own mode
set: `GLENZ/MAIN.C:288-303` is `dis_partstart()`, the music wait (`while(!dis_exit() &&
dis_musplus()<-19);`), `zoomer2()`, and only then `mov ax,13h` / `int 10h`. The frames match it:

- **The band law.** `zya++; zy += zya/4` (`GLENZ/ZOOMER.C:41-44`), so `zy(n) = n(n+1)/8`, with a top
  band through `zy` and a bottom band at `zy2 = 125*zy/260` (`:45-58`). The top of the non-black
  region is row 11, 46, 106 and 191 at 1120–1150, second difference exactly 1/4 row per frame²; the
  bottom edge is `399 - 125*zy/260` within a row each time (378, 349, 308). Probed two frames apart,
  the top edge advances 14, 15, 16, …, 21 rows.
- **The grey.** `pal2[a] = (pal1[a]*b + 30*c) >> 5` with `c` = `frame` capped at 32
  (`ZOOMER.C:59-68`), over entries 0–127: six-bit 30, which widens to 121. The band at 1150 is
  `(121,121,121)`.
- **The black bands.** `zoomer2` writes index 255 into both bands (`ZOOMER.C:48`, `:56`) and fades
  only 0–127, and `BEG/BEG.C:25-34` leaves entry 255 at `(0,0,0)`.

BEGLOGO's own drawing ends with its fade: `BEG/BEG.C:86` is the last `setpalarea` and `:87` closes
`main()`. The 580–1110 hold is the loader and GLENZ's music wait leaving BEGLOGO's last screen up, so
reproducing 900 means reproducing a hold that belongs to the next part's wait. That holds for either
BEGLOGO build: `ENDPIC/BEG.C:45-48` would wait at most 300 frames, less than the ~530-frame hold
(`beglogo.md` §1 owns the build question).

---

## 4. ALKU

`SECOND 1`'s first part: three title cards over black, then a scrolled horizon picture, 320×400
throughout, from the first graphics frame to a mode change in (5950, 5960]. Each card is gated on
the music (`while(dis_sync()<1..4)`, `ALKU/MAIN.C:59,66,72,79`), drawn once (`prtc`, `:61-63`), faded
in 64 steps, held 300 frames and faded out 64 (`dofade(fade1,fade2); wait(300);
dofade(fade2,fade1)`, `:64`, `:301-311`). After the fourth sync the picture fades in over 128 frames
(`cop_dofade=128`, `:83`) and `for(f=60;a<320;...)` scrolls it while five text batches are laid over
it (`:87-135`). The row stride is 176 bytes and the read refuses the screen, so every frame is
RGB-only (§1.6).

### 4.1 The sweeps

Black from 404 to the first card; card 1 `A / Future Crew / Production` (1620–1900); black; card 2
`First Presented / at Assembly 93` (~2220–2420); card 3 `in` over the Dolby Surround logo (~2720);
black; then the landscape picture, already scrolling by 3320, alternately bare and with text:
`Graphics / Marvel / Pixel` at 3920, `Music / Purple Motion / Skaven` at 4520, `Code / Psi / Trug /
Wildfire` at 5120, `Additional Design / Abyss / Gore` at 5720.

### 4.2 The card, measured frame by frame

Every 20th frame from 1500 to 2000, one batch:

| index | 1500–1540 | 1560 | 1580 | 1600 | 1620–1900 | 1920 | 1940 | 1960 | 1980–2000 |
|---|---|---|---|---|---|---|---|---|---|
| whole-frame mean byte | 0 | 0.68 | 1.48 | 2.30 | 2.49 | 1.96 | 1.16 | 0.35 | 0 |
| rgb | `fec9afb5…` ×3 | `ea09bd61…` | `5ca8d435…` | `e56da741…` | `6a27ec50…` ×15 | `55bc5ebe…` | `0bb265c7…` | `3d577743…` | `fec9afb5…` ×2 |

Black until the first fade step lands in (1540, 1560], 64 steps in, one `sha256` over the hold, 64
out, black by 1980. Card 1's `6a27ec50…` is the same bytes in six sessions. Cards 2 and 3 are one
`sha256` per sampled triple (2419–2421 `6a2de862…`, 2719–2721 `5e3c3884…`). The black screen,
`fec9afb5…`, is one `sha256` at eighteen indices in five captures: every pixel `(0,0,0)`, and what the
plane holds beneath it the oracle cannot read (§1.6).

### 4.3 The keyframes

| index | what is on screen | rgb |
|---|---|---|
| 900 | the **black screen**, from the first graphics frame to the first card's fade in (1540, 1560] | `fec9afb5…` |
| 1719 | **card 1**, mid-hold (fade in ends ~1615, fade out starts ~1905) | `6a27ec50…` |
| 2719 | **card 3**, `in` over the Dolby Surround logo (`prtc`, `MAIN.C:74-76`) | `5e3c3884…` |
| 5859 | the **scrolled picture**, bare of text; 5859, 5860 and 5861 are one `sha256` in the reference's session, so the step boundary is at 5862 or later | `be6f6c32…` |

All four are `.rgb` only. Card 2 is not a keyframe: it is card 1's code path with other strings
(`MAIN.C:68-69` against `:61-63`). 900, 1719 and 2719 carry no supports (each triple is one `sha256`);
5859's window is captured because its ±1 identity depends on where the step falls. Nothing is
keyframed in the first frames after 404: which of 404–412 the grab loop can catch varies by session
(`--frames 404,405,406,410` was refused as a miss, `412,416` and `411,412,416` captured).

### 4.4 The re-capture: three held frames identical, the picture a whole pan away

Two independent sessions holding `900,1719,2719,5858,5859,5860`: 900, 1719 and 2719 are 0.0000 % at
offset 0 in both; 5859 differs, `mode: rgb`, exit 1. Over the picture band (rows 50–349; the rest is
black in all three):

| session | 5858–5860 | differing at offset 0 | best shift | differing after the shift |
|---|---|---|---|---|
| A | one `sha256`, `a77b4991…` | 14,048 px (10.9750 %) | 1 px horizontally | **0 px of 95,700** |
| B | one `sha256`, `069964a4…` | 19,558 px (15.2797 %) | 2 px horizontally | **0 px of 95,700** |

The sessions differ by a whole number of pixels of the part's own pan (one pixel per 9-frame scroll
step, `MAIN.C:7`, `:87`) and by nothing else: four sessions give three pictures, the reference's and
one and two pixels along. Session B's 5850 is session A's picture and its 5858 one pixel along, so the
pan moved once in those eight frames. The ±1 window cannot reach it (§1.6, §1.12 ladder step 3). A
port has an exact target at 900, 1719 and 2719, and at 5859 a target the oracle's sessions do not
agree on. A day-apart re-capture landed on the reference's own pan phase (§1.12).

### 4.5 Where the part ends

A sweep of 5420…8120 is refused at 6020 (320×200). Sparse probes bound the change to **(5950,
5960]**; no index there refuses as a miss.

5800, 5860, 5920 and 5940 have rows 50–349 picture and black above and below. **5950** has rows 0–24
black, 25–174 picture, 175–184 black, 185–349 picture, 350–399 black. It is ALKU's own closing
routine (image `0x13ce`): it compacts the last window into a 200-row page and sets CRTC 9 = 1, which
the emulator applies half a frame late, mid-frame (`alku.md` §12, A20).

The changeover is one to a few frames wide by session: one session's 5960 is still 320×400 ALKU,
another's is 320×200 U2A (§5.5). No keyframe belongs near it.

---

## 5. U2A

`SECOND 1`'s second part: a held "landing pad" scene, a flying saucer over the same landscape, then
the landscape alone. 320×200, from 5960 or 5961 to the end of its drawing between 7510 and 7560, its
cleared background staying up until PAM's first frame at 7710. U2A and PAM share the geometry, so
nothing in the geometry marks their boundary; the content does (§5.5).

`VISU/C/U2A.C` loads its scene data (`U2A.0xx`) and a background, waits for the music
(`:227-239`, `int 0fch` with `a>10 && b>46`), sets the screen (`vid_init(3)` at `:242`,
`vid_window(0L,319L,25L,174L,512L,…)` at `:247`), and runs a four-buffer animation loop
(`cl[0..3]`, `copper2` at `:105-130`, the loop at `:259-…`) until the music reaches `a>11 && b>54`
(`:276`).

### 5.1 The sweeps

Every 50th frame from 5960 to 7810 in one session: 5960 plain landscape; **6010–6960 the pad**, the
landscape with a yellow wireframe grid, one `sha256` for all twenty samples; 7010 plain; 7060–7260 a
grey saucer that fills the frame and recedes; 7310–7410 a small blue-white craft shrinking against
the sky; 7460–7660 the plain landscape (7560–7660 one `sha256`); 7710 onwards PAM (§6). Other
sessions agree that the pad arrives and leaves at about the same place. Sweeps from 5960 to 8700 are
refused at 7950 and 8400: those are PAM's end (§5.5).

### 5.2 The keyframes

| index | what is on screen | files | hashes |
|---|---|---|---|
| 6500 | the **pad**, inside the 6010–6960 hold | `.rgb` only | rgb `11a66eb0…` |
| 7300 | the **saucer receding**, mid-animation | `.rgb` + `.idx` | rgb `0a7c2216…`, idx `b50f52f4…` |
| 7640 | the **plain landscape**, inside the 7560–7700 hold | `.rgb` + `.idx` | rgb `0a68ffb9…`, idx `c51ba997…` |

6500 sits deep in the pad's hold, 490 frames clear of the ALKU→U2A change (§5.5).

### 5.3 Held or moving, measured — and which frames carry indices

| keyframe | `K-1` | `K` | `K+1` | reading |
|---|---|---|---|---|
| 6500 | `11a66eb0…` | `11a66eb0…` | `11a66eb0…` | **held** |
| 7300 | `9f5fe99c…` | `0a7c2216…` | `0a7c2216…` | **moving**: changes between 7299 and 7300 |
| 7640 | `0a68ffb9…` | `0a68ffb9…` | `0a68ffb9…` | **held** |

**Which frames carry `.idx` is the session's, not a property of held or moving.** The reference
kept indices at 7300 and 7640; a partner kept them at 7639–7641 and not at 7299–7301, with 7300's
RGB byte-identical in both, and a third session had 7300's again. The pad frames never kept them and
the plain-landscape frames did. The read takes each plane from offset 0, which matches the screen
only when the displayed buffer is the one there, and the part rotates four buffers through its
display start (`VISU/C/U2A.C:105-130`, `vid_setswitch(-1,clr)` in `copper2`). The RGB is the same
bytes in every session.

### 5.4 The re-capture

Against an independent session holding the three triples: exit 0, `verdict: identical`,
`mode: mixed`, offsets `{0: 3}`, the moving 7300 included. That is the same-alignment-group case
(D17 measures about half): the held pair carries the reproducibility, and 7300's step (changed from
the frame before, held for the frame after) gives the ±1 search something to bite on.

Which trace session the port's block-1 clock matches, and the 7300 residual that follows from this
capture being a third session, are `u2a.md`'s (A28, A29).

### 5.5 Where the part ends — and how, when nothing changes geometry

The sweeps' 320×400 refusals at 7950 and 8400, and at 7930 in a PAM scan, are **BEGLOGO taking the
screen after PAM** (the change is in (7920, 7930], §6.5). U2A's last drawn frames are the saucer
sequence ending in the plain-landscape hold (7560 at the latest to 7700), and PAM's first frame is
7710, found by content: the landscape stops and a flat white frame matching `wfade[0] = 63`'s wash
starts (§6.1).

U2A's start is as fuzzy: 5960 is 320×200 U2A in one session and 320×400 ALKU in another (§4.5), so
no keyframe sits in the part's first few tens of frames.

---

## 6. PAM

`SECOND 1`'s third part: the animated intro picture that leads into BEGLOGO. **320×200, U2A's
geometry**, from its first drawn frame in (7700, 7710] to the end of its animation at about 7920; the
screen changes to BEGLOGO's 320×400 in (7920, 7930].

`PAM/OUTTAA.C` builds 63 washed variants of its palette (`pal[a*768+b]=(63*a+(64-a)*pal[b])/64`,
`:36-37`), waits for sync 10 (`:39`), sets the screen (`tw_setpalette(&pal[768*63])`,
`tw_opengraph()`, `:41-42`), and plays a **45-step** animation at four frames a step (`f++<45` at
`:46`, `while(frame_count<4)` at `:48`), unpacking each frame into one of two buffers and flipping
the display start (`:53-59`) with a palette from its fade table (`cop_pal=&pal[768*wfade[f]]`,
`:62-63`). `wfade[100] = {63,32,16,8,4,2,1,0,…0,1,2,4,6,9,14,20,28,37,46,56,63,63,…63}` (`:22-32`):
washed white, full contrast in about 28 frames, washed white again at the end.

### 6.1 The dense scan, and what it measured

Every 10th frame from 7660 to 7920 (refused at 7930):

| index | 7660–7700 | 7710 | 7720…7860 (15 samples) | 7870 | 7880–7920 |
|---|---|---|---|---|---|
| rgb | `0a68ffb9…` ×5 | `1fef46de…` | a different one at every sample | `644acaf3…` | `1fef46de…` ×5 |
| distinct colours | 40 | 1 | 63 → 163 → 76 | 24 | 1 |

- **U2A's hold ends at 7700 and PAM starts at 7710**, in the same geometry.
- **The flat `(251,251,251)` frame is PAM's wash**: `251 = widen(62)`, and six-bit 62 is what
  `(63*63 + pal)/64` gives for `wfade[f] = 63`. It is at 7710 in three sessions, and holds at least 40
  frames at the end (7880–7920).
- **The animation changes at every sample**, so its RGB-only frames are D15's moving case, not a
  read-layout refusal.

45 steps at four frames is 180 frames; the animation spans (7705, 7920], 215 frames, the difference
being each `ulosta_frame`'s cost beyond its wait.

### 6.2 The keyframes

| index | what is on screen | files | hashes |
|---|---|---|---|
| 7710 | **the washed opening frame**, flat `(251,251,251)`: `wfade[f]=63` over the first animation frame; 7709 and 7711 are the same bytes, 7720 is the next frame | `.rgb` only | rgb `1fef46de…` |
| 7760 | an **animation frame**, the landscape with glowing objects above the horizon | `.rgb` only | rgb `5d558081…` |
| 7830 | an **animation frame from the second half**, a wide band across the landscape | `.rgb` only | rgb `e1967ed2…` |
| 7900 | **the washed closing hold**, the same bytes as 7710 and as 7880–7920 | `.rgb` + `.idx` | rgb `1fef46de…`, idx `4333e44e…` |

### 6.3 Held or moving, measured — and the spacing rule

| keyframe | `K-1` | `K` | `K+1` | reading |
|---|---|---|---|---|
| 7710 | (7700 `0a68ffb9…`, U2A's) | `1fef46de…` | (7720 `10bb3b95…`) | **moving, but only just**: the wash lasts about one loop step |
| 7760 | `23fa5096…` | `5d558081…` | `5d558081…` | **moving** |
| 7830 | `e1967ed2…` | `e1967ed2…` | `e1967ed2…` | **held** |
| 7900 | (7890 `1fef46de…`) | `1fef46de…` | (7910, identical) | **held** |

A partner list of `7709,7710,7711,7759,…` is refused (`the emulator reached frame 7773 before frame
7759 could be captured`): the three-index batch's reads cost ~57 frames with the next index 48
behind. §1.3's spacing binds between batches. The partner used is
`7710,7759,7760,7761,7829,7830,7831,7900`.

### 6.4 The re-capture

Exit 0, `verdict: identical`, `mode: mixed`, offsets `{0: 4}`. The same indices are byte-identical in
every session that took them (four for 7710 and 7760, three for 7830 and 7900), so PAM's animation is
session-stable at a fixed index. Only 7900 carries `.idx`; the animated frames are RGB-only because
the screen moves during the read, so three of the four frames compare through the RGB path (D14).
PAM never writes CRTC 0x13, so its rows are 80 (§1.6).

### 6.5 Where the part ends, and what follows it

The screen becomes 320×400 in **(7920, 7930]**, and what comes up is **BEGLOGO**: frames from 7950
show the logo fading in at 7950–8040 and holding at 8070, 8100 and 8130 with the same `sha256`,
`cbdc67eb…`, as BEGLOGO's held logo under `SECOND 2` (§3.1). The mode change is the next part's, and
PAM keeps U2A's 320×200 screen.

---

## 7. GLENZ

`SECOND 2`'s second part: the FC-logo band BEGLOGO's picture collapses into, a translucent "glenz"
polyhedron over a checkered floor, then a red polyhedron with the glenz object moving through it.
**320×200, 92-byte plane rows** (`GLENZ/VIDINIT.ASM:306`). On the `SECOND 2` boot the screen is up
from the change in (1154, 1160] (§3.4) and the part's drawing ends with the red polyhedron leaving
the frame at ~3650. A flat black screen follows (3680–3700 `ea0787f6…`, the same bytes as ALKU's and
PAM's black), and TUNNELI's first ring is at 3710 (§7.5).

### 7.1 The sweeps

1200 and 1600 black sky over a purple checkered floor; 2000 the blue/grey translucent polyhedron
above the floor; 2400 larger; 2800 the red polyhedron with the glenz object in front; 3200 larger;
3600 the red object alone; 4000–4400 TUNNELI; 4800 black; 5200–6000 TECHNO. Between: the glenz
object rises from the horizon (1800, 1900), stands alone against a pale background (2300), and the
red polyhedron takes over from ~2500. The opening: 1250 the held floor over black; **1300 still the
bounce** (its band at rows 153–199 with indices 12–14, `515b1ea0…`, mean 13.66, its plane 8,797 bytes
from 1499's); 1400, 1450 and 1499 the held floor over black, byte-identical; 1620 the held floor over
a grey (purple) palette phase (`314c802f…`, mean 103.0, sky `(105,81,109)`); 1700 the object higher.

### 7.2 The keyframes

| index | what is on screen | files | hashes |
|---|---|---|---|
| 1499 | the **held opening**: the checkered floor with the object's top edge at the horizon | `.rgb` + `.idx` | rgb `e45ab076…`, idx `75d561dc…` |
| 2300 | the **glenz polyhedron alone** against the pale background, the floor a thin band | `.rgb` only | rgb `dbe96c0a…` |
| 3000 | the **red polyhedron with the glenz object inside it**, mid-rotation | `.rgb` only | rgb `05905a2c…` |
| 3400 | the **red polyhedron with the glenz object at its edge**, ~250 frames before the glenz object leaves | `.rgb` only | rgb `d02e362e…` |

### 7.3 Held or moving, measured — and where GLENZ's indices come from

| keyframe | neighbours | reading |
|---|---|---|
| 1499 | 1499 = 1500 = 1501; 1250, 1400, 1450 the same bytes in other captures | **held**, over black (and over a palette phase in some sessions, §7.6) |
| 2300 | 2099/2100/2101 and 2299/2300/2301 three `sha256` each | **moving** |
| 3000 | 2699/2700/2701 and 2999/3000/3001 three each | **moving** |
| 3400 | 3399/3400/3401 three | **moving** |

The hold is bounded and session-dependent: in one session 1600 is `86d42e74…`, 82.54 % from 1499
(mean 100.3 against 13.2, a palette phase), in another it is `e45ab076…`, the held picture.

**`.idx` presence on this screen is unpredictable** (§1.7): every moving keyframe came back RGB-only,
and the held picture carried indices in four captures (single reads and a three-index batch alike)
and not in one.

**That `.idx` confirms the 92-byte stride.** `indexed_matches_rgb` compares every pixel of the
widened plane with the RGB, and independently both have non-black content in **rows 154–199 only**
(200 of 200 rows agree on content versus black) and exactly **11,720** non-black pixels across 8
indices. A read at `width/4` = 80 bytes a row, 12 short, could not do that.

### 7.4 The re-capture: the held frame identical, the moving three the pictures the sessions present

Against an independent partner, captured in a separate sitting after filing:

```
frame 1499: 0.0000% differing (0/64000 px), max delta 0
frame 2300: 0.0000% differing (0/64000 px), max delta 0, rendered index 2299 (offset -1)
frame 3000: 0.0094% differing (6/64000 px), max delta 41, rendered index 2999 (offset -1)
frame 3400: 0.1016% differing (65/64000 px), max delta 121, rendered index 3399 (offset -1)
verdict: different, mode: mixed, offsets {"-1": 3, "0": 1}
exit: 1
```

Against a second independent partner (which does not hold 1499, so the files were compared
directly), all three moving keyframes are **byte-identical at offset 0**.

The filed pictures are the census's largest-support ones (§1.8): `dbe96c0a…` 6/9 at 2300,
`05905a2c…` 8/21 at 3000, `d02e362e…` 3/10 at 3400. **3000 and 3400 are below half and have no
settled content**: at 3000 the two leading pictures are 19 pixels apart, split by byte-identity into
groups of 7, 6, 2 and singletons. Read them as one exact phase of an animation the oracle spreads over
a few frames.

Per-frame rates inside a partner: 2300 moves 17.30–26.53 % of pixels a frame by session pair, 3000
**10.64 %**, 3400 **17.07 %**. The first reference's 18.66–34.32 % from the sessions was therefore
≈1.5–2.5 frames, beyond the ±1 window, which is why the filed picture has to be one the sessions
present.

### 7.5 Where the part ends

| index | 3680 | 3690 | 3700 | 3710 | 3720 | 3730 | 3740 | 3750 |
|---|---|---|---|---|---|---|---|---|
| rgb | `ea0787f6…` | `ea0787f6…` | `ea0787f6…` | `d0f33e4a…` | `c75d7771…` | `36b6940b…` | `3654de07…` | `83a918e2…` |
| what | black | black | black | the tunnel's first small ring | ring | ring | ring | ring growing |

3600 is the red polyhedron alone and 3650 the same object leaving by the top-left corner: **GLENZ's
drawing ends at ~3650, 30 black frames follow, and TUNNELI's first frame is 3710**, a content
boundary with no geometry change (§1.6).

### 7.6 The opening measured: the intro start, the loop start and the object boxes

Two traces (`music-clock.md` §2.2) and scratch captures of the recipe measure the opening. Each
capture is `cargo xtask reference capture --arg <1|2> --boot autoexec --keys "KBD_left KBD_enter"
--cycles 30000 --frames <list> --out <scratch>` with these lists:

| measurement | `--arg` | `--frames` |
|---|---|---|
| the intro start, `SECOND 2` | 2 | `"$(seq -s, 1100 1154)"` |
| the opening's phases | 2 | `1250,1300,1350,1400,1450,1499,1550,1600,1650,1700,1750,1800` |
| the first object pixels (two sessions) | 2 | `"$(seq -s, 1590 1630)"`, `"$(seq -s, 1598 1615)"` |
| the fade and floor clear | 2 | `"$(seq -s, 2250 10 2350)"` |
| the intro start, `SECOND 1` | 1 | `8440,8460,8480,8500,8510,8520,8530,8540,8550` and `"$(seq -s, 8516 8528)"` |
| the collapse | 2 | `"$(seq -s, 1000 10 1150)"` |

**The intro start is the zoomer's first tick.** On `SECOND 2` the trace `second2b` reads
`_np_zframe` 526 at 1110 and 0 at 1111, and a fresh boot is byte-identical through 1110 and first
differs on **1111** (320 pixels). On the block-1 boot the intro is at **8521**: `second1d` reads 5588
→ 0 there, and the capture holds through 8521 and first differs on **8522**, one frame after the
reset sample (the sample/publish instant D17 quantises). **The zoomer plays over BEGLOGO's final
frame**, `cbdc67eb…` (`reference/parts/beglogo/frame_000900`'s `sha256`, 320×400) in both boots, not
over the later 320×200 1499 hold.

**The loop start `L_r = 1562`**, defined as `first_object_index − first_object_tick`. The first
object pixels are at **1606** in two sessions, frame for frame, and the source's physics gives frame
**44**, the first frame on which a face's span crosses row 0. `1606 − 44 = 1562`, and the trace's
waits agree (reset 1111, `getmframe()<300` at 1515, `getmframe()<333` at 1560, three `dis_waitb`
ticks and the main loop's first wait; `music-clock.md` §2.2): main-loop frame f's picture is at
reference `1562 + f`. The session spread is zero.

The above-floor box (rows 0–149, every pixel that differs from the frame's own sky pixel `(0,0)`)
for 1606–1611 against the model:

```
frame  captured box (x0,x1,y0,y1)   n    centroid       model frame
1606   (172,179,0,0)                8    (175.5, 0.0)   44
1607   (166,179,0,1)               21    (173.3, 0.3)   45
1608   (157,180,0,3)               60    (170.5, 1.0)   46
1609   (153,180,0,4)               86    (168.8, 1.4)   47
1610   (149,179,0,5)              111    (166.6, 1.7)   48
1611   (146,180,0,6)              137    (165.5, 2.0)   49
```

The model side is the source's recurrence (`GLENZ/MAIN.C:504-536`'s `ypos`/`jello`/scale terms,
`VEC.ASM`'s `crotlist`/`projlist` with `init320x200`'s 256/213/160/130, `NEW.ASM`'s spans) in
`python3 -I tools/glenz_physics.py entry`, which prints `first drawn main-loop frame: 44 (ypos -8254,
jello 0, xscale 120)` and the same six rows, every count and centroid equal.

**The object boxes** of the filed frames, by the same rule (rows 0–149 and the full frame):

```text
python3 -I - <<'PY'
W, H = 320, 200
for n in (2300, 3000, 3400):
    b = open(f'reference/parts/glenz/frame_{n:06d}.rgb', 'rb').read()
    sky = tuple(b[0:3])
    for top, label in ((150, 'above-floor'), (200, 'full-frame')):
        pts = [(i % W, i // W) for i in range(top * W) if tuple(b[i*3:i*3+3]) != sky]
        xs = [p[0] for p in pts]; ys = [p[1] for p in pts]
        print(n, label, 'x', min(xs), max(xs), 'y', min(ys), max(ys), 'n', len(pts),
              'centroid', round(sum(xs)/len(pts), 1), round(sum(ys)/len(pts), 1))
PY
```

| frame | above-floor box (rows 0…149) | pixels | centroid | full-frame box |
|---|---|---|---|---|
| 2300 | x **97–217**, y **38–149** | **9,989** | **(156.8, 94.1)** | x 2–319, y 38–199 (with the floor band) |
| 3000 | x **27–270**, y **0–149** | **30,004** | **(151.9, 82.2)** | x 27–270, y 0–192, 35,977 px, (151.7, 96.4) |
| 3400 | x **27–313**, y **0–149** | **34,720** | **(169.3, 78.8)** | x 27–313, y 0–199, 44,080 px, (169.8, 98.8) |

At 3000 and 3400 the floor is gone (the main loop's frame 765–789 clear, `MAIN.C:552-585`, has emptied
`bgpic`'s rows 150–199 by main frames 1438 and 1838), so the full-frame box is the object's extent.
A fresh session's 2300 box is x 99–217, y 37–149, 9,927 px, (157.6, 94.2): a one-frame phase
difference on a screen that moves 17–22 % a frame.

**The palette phases.** The frames fall into phases named by the frame's own sky colour:

| frames | sky colour | mean brightness | reading |
|---|---|---|---|
| 1250, 1350, 1400, 1450, 1499, 1550 | (0,0,0) | 13.21 | the held picture over black, byte-identical |
| 1300 | (0,0,0) | 13.66 | still the bounce |
| 1590…1605 | (0,0,0) | 13.21 | held, no object |
| 1600 (another session) | (174,0,0) | 60.59 | held, no object, the DAC in a red phase |
| 1650/1700/1750/1800 | (174,0,0) | 63.4/64.6/64.6/62.5 | the object over the red phase |
| 1620/1700/1800 (another session) | (105,81,109) | ~101 | a purple phase |
| 1600 (another session) | (146,174,0) | 100.27 | a green phase |

The phase is the session's DAC at the read, not the frame's (D17).

**The palette cross-check on `L_r`.** The loop's fade is `MAIN.C:544-551` (`frame>700`: `pal[a] =
backpal[a]·(764−frame)/64`, black at 764) and the `backpal` rebuild `:552-584` (`frame<790`, rewritten
from `frame>785`). With `L_r = 1562` those fall at reference **2263…2326** and **2348**. Frames
2250…2350 (main 688…788) have mean brightness **22.8, 27.0, 23.2, 23.4, 21.3, 18.1, 17.3, 15.3, 13.6,
14.9, 13.2**: the fade's path, with the floor's clear (`MAIN.C:552-556`) inside it. The filed 2300 is
main frame 738, inside the fade, so its sky is black and its floor colours are the dimmed `pal`.

### 7.7 The port's phase and the block-2 comparison

Retired from this ledger: the port's side (the held opening, `L_p`, the moving keyframes' aligned
residuals) is `glenz.md` §11, asserted by `bins/secondreality/tests/blocks.rs`, with A36 and A37.

---

## 8. TUNNELI

`SECOND 2`'s third part: the dot tunnel. 320×200 mode 13h (`TUNNELI/TUN10.PAS:110-115`), from its
first ring at 3710 to its last dots in (4700, 4770]: about 1060 of its own frames
(`TUN10.PAS:3`, `const veke = 1060; { frame count to exit }`). Every keyframe is moving, and a
one-frame phase difference moves most of its dots.

### 8.1 The sweeps

3730 black; 3760 a small dotted ellipse (the tunnel's mouth); 3800 the ring larger; 3900–4000 the
tunnel filling the frame; 4100–4200 the walls skewing; 4300–4500 the tunnel off-centre, its mouth
moving up; 4600–4650 the dots thinning and the mouth dark; 4700–4750 the last sparse dots. The end
drifts by tens of frames between sessions (§8.5).

### 8.2 The keyframes

| index | what is on screen | rgb |
|---|---|---|
| 3800 | the **tunnel's mouth**: a bright ring of dots around a black centre | `f861b1f2…` |
| 4200 | the **full tunnel**: dots filling the frame, the mouth skewed | `af4763b3…` |
| 4500 | the **tunnel off-centre**, its mouth up and to the left | `cb15de2d…` |
| 4600 | the **thin tunnel**: sparse dots, the mouth dark, the last distinctive shape | `16f8c1fd…` |

All `.rgb` only. 4700 is not a keyframe: its partner was refused as a miss (`reached frame 4702
before frame 4701`), and some sessions are black by 4740.

Of the 43 frames in 3700–4800 across every capture taken, two carry `.idx` (3730 and 3750); the rest
are refused for motion. Both were read by hand: 3730's plane is all zeros against an all-black RGB
(worthless as evidence); 3750's holds exactly 128 non-zero pixels over indices 0/119/121 in rows
63–70 against an RGB with exactly 128 non-black pixels, the first ring.

### 8.3 Held or moving, measured

| keyframe | `K-1` | `K` | `K+1` | reading |
|---|---|---|---|---|
| 3800 | `4cda6e68…` | `f65f2210…` | `b7d7f1a1…` | moving |
| 4200 | `efbd8db4…` | `ff72ebb3…` | `931ac683…` | moving |
| 4500 | `ac2b50d3…` | `a8547d71…` | `64440e1c…` | moving |
| 4600 | `0ea6f434…` | `7dfaf5a9…` | `67b1dc97…` | moving |

(One session's triple; its `K` is not the filed picture.) Sessions quantise: two sessions match
exactly at 3800, 3900 and 4000 (`f861b1f2…`, `259c3bbb…`, `96056aba…`) and differ at 4200 and 4400 by
0.0859 % and 0.1578 %, and five sessions at 3800 give three pictures, one of them in three (§1.7).

### 8.4 The re-capture

The reference was re-captured once into the largest group (its first 3800 was a singleton where 7 of
9 sessions agree on `f861b1f2…`). Against an independent session holding the four triples:

```
frame 3800: 0.0000% differing (0/64000 px), max delta 0
frame 4200: 0.0000% differing (0/64000 px), max delta 0
frame 4500: 0.0000% differing (0/64000 px), max delta 0
frame 4600: 0.0891% differing (57/64000 px), max delta 97
verdict: different, mode: rgb, offsets {"0": 4}
exit: 1
```

Support 7/9, 3/6, 2/5, 1/6 (§1.8): **4500 and 4600 have no settled content**. The tunnel moves
**2.05–2.22 %** of pixels a frame at 3800, **10.47–10.69 %** at 4200, **6.22–6.40 %** at 4500 and
**8.74–9.08 %** at 4600, so the first reference's 8.07–10.18 % from two sessions was ≈1 frame at
4200–4600 and ≈4 frames at 3800.

### 8.5 Where the part ends

The tunnel's last dots are in **(4700, 4770]**, and TECHNO's picture starts at **4797**, after a black
plateau at 4774–4793, by two VGA-content probes within 1–3 frames of each other; TECHNO's 16-colour
mode change is at 4797–4801 in four more sessions. Both screens are 320×200, so it is a content
boundary. One session had TECHNO's rings at 4770 with black from 4740: the excursion of §1.12, not
the boundary.

At a fixed index the sessions disagree by ≈1–4 frames (§8.4); the boundary moves by tens, because the
exit is TUNNELI's own frame count (`veke = 1060`) and its start follows the loader's music restart
(`part-sequence.md` §3).

---

## 9. TECHNO

`SECOND 2`'s fourth part: the techno bars and the EGA interference, then the "panic picture" (the
troll) that PANICEND collapses. Two geometries, captured as two captures (§1.7): a 320×200 screen from
4797 (§8.5) to the change in (8150, 8200] (`reference/parts/techno/interference/`), and a 320×400 one
from there to 8780 (`reference/parts/techno/troll/`).

**The 320×200 screen is not 256-colour.** TECHNO writes the graphics controller's mode register and
the attribute controller's colour-select (`TECHNO/KOE.C:236-258`), and DOSBox re-decides the mode
from them (`vga.cpp:60-96`, `VGA_DetermineMode`), so every frame there is RGB-only with `indexed
capture failed (… not 256-colour …)` (§1.7). The 320×400 screen carries indices where the offset
register reads a row the read can describe (8200 and 8250; §9.6).

### 9.1 The sweeps

320×200: 4850–4900 concentric rings (cyan on black); 4950 the rings over a pale ground; 5000–5050 flat
white; 5100 the pink moiré; 5200–5900 the moiré mutating; 6300 diagonal stripes; 6700 purple
diamonds; 7100 purple triangles; 7500 black; 7800 triangles again; 8100 a crosshatch. 320×400: black
from the mode change to ~8290, a white-wash fade at ~8300, the picture held from ~8350 to 8780
(`40044edc…` at 8350, 8400, 8450, 8500, 8600 and 8700). Sweeps across either change are refused (at
8200 and 8400 going up, at 8800 going down). Two sessions' `6300,8200` captures are refused for
geometry, as any list spanning both screens must be.

### 9.2 The keyframes

**320×200** (`interference/`), all `.rgb` only:

| index | what is on screen | rgb |
|---|---|---|
| 4900 | the **interference rings**, concentric blue rings on black, the first drawn screen | `3dc29660…` |
| 5000 | the **white flash**: every pixel `255,255,255`; the same bytes at 5050 | `20f3d62f…` |
| 6300 | the **diagonal-striped interference** | `3c2b0d8f…` |
| 8100 | the **crosshatch**, the last screen before the 320×400 change | `6033bbc4…` |

**320×400** (`troll/`):

| index | what is on screen | files | hashes |
|---|---|---|---|
| 8200 | the **flat black screen** the mode change leaves | `.rgb` + `.idx` | rgb `fec9afb5…`, idx `eec19bc6…` |
| 8300 | the **fade-in**: the troll washed to white, 43 colours, every pixel non-black | `.rgb` only | rgb `17b9f92f…` |
| 8400 | the **held picture** at full contrast, 78 colours, 93,200 non-black pixels | `.rgb` only | rgb `40044edc…` |

8200's `.idx`, read by hand, is 128,000 zeros: consistent with the black RGB and worthless as
evidence for the 160-byte stride (§1.6).

### 9.3 Held or moving, measured

| keyframe | neighbours | reading |
|---|---|---|
| 4900 | 4899/4900/4901 three `sha256` | **moving** |
| 5000 | 5000 = 5050; 4999/5000/5001 three | **moving across a frame**: the flash is a palette step |
| 6300 | 6299/6300/6301 three | **moving** |
| 8100 | 8099/8100/8101 three | **moving** |
| 8200 | 8199 = 8200 = 8201 | **held** |
| 8300 | 8299/8300/8301 three | **moving** |
| 8400 | 8399 = 8400 = 8401, and 8350–8700 | **held** |

### 9.4 The 320×200 re-capture

The reference was re-captured once into the largest group (its 6300 and 8100 had been singletons).
Against an independent session holding the four triples: exit 0, `verdict: identical`,
`mode: rgb`, offsets `{0: 4}`. Support 3/4, 5/5, 5/6, 3/4 (§1.8).

On this screen the percentage has no room in it: adjacent frames differ 4899→4900 **24.67 %**,
4900→4901 **24.91 %** (the rings), 6299→6300 **96.33 %**, 6300→6301 **98.04 %**, 8099→8100
**98.69 %**, 8100→8101 **89.10 %** (the dither). The first reference's 70–97 % at 6300 and 8100 were
one-frame statements (§1.7).

### 9.5 The 320×400 re-capture, and the one session that disagreed

Against a session holding `8200,8299,8300,8301,8400`:

```
troll frame 8200:  0.0000% differing (0/128000 px), max delta 0
troll frame 8300: 99.9883% differing (127985/128000 px), max delta 206, rendered index 8299 (offset -1)
troll frame 8400:  0.0000% differing (0/128000 px), max delta 0
verdict: different, mode: mixed, offsets {"-1": 1, "0": 2}
exit: 1
```

The held frames are byte-identical; the fade-in differs by essentially every pixel, one session's
wash at another brightness. 8300's filed picture has support 3/4 (§1.8).

One session had the troll picture already up at 8200 and 8300 (8200's `sha256` `a0949c8436d1087…`,
78 colours and 93,200 non-black pixels at both, the finished picture) where every other session shows
black at 8200, the wash at 8300 and the picture from 8350. Its frames were overwritten by the
re-capture; only those printed figures survive. It is one session's observation. If real, it is ≥150
frames of this section's timeline, the size of a music-gated event (the fade waits on `muscode 6`,
`TECHNO/KOE.C:638-649`).

### 9.6 Where the part ends, and what the 320×400 screen's rows really are

**The 320×400 screen's read is not one row stride for its whole span.** One session's log holds nine
`PlaneTooSmall` refusals on 8700…8780 naming a **320-byte plane row** (`this screen's rows are 320
bytes and it is 400 rows tall, so it needs 128000 bytes a plane; one plane holds 65536, and the display
wraps`), where 8200 and 8250 were read at 160. In two other sessions 8700 is refused only for motion,
with byte-identical RGB (`40044edc…`), so the displayed layout is the same: the value read, 0xa0, is
one of the eight `plv * 0x20` values `TECHNO/KOE.C:385-389` writes to CRTC 0x0c (§1.7).

**The part ends at the geometry change in (8780, 8790]**, where PANICEND halves the screen back to
200 lines; the last index of the 320×400 screen taken is 8780 (`40044edc…`). `TECHNO/KOE.C:588-616`
put the picture there: mode 13h, `inittwk()`, CRTC 13h = 80, then `TROLL.UP` blitted 400 rows deep.

---

## 10. PANICEND

`SECOND 2`'s fifth part: the fake ending, TECHNO's picture collapsing to a line and then to a single
dot on black. 320×200, from the geometry change in (8780, 8790] (the `SHUTDOWN.C` asm writes CRTC 09h =
41h, halving the 400 lines) to the next part's first drawn frame at ~9230.

`PANIC/SHUTDOWN.C` takes the palette the previous part left (`getpal`, `:43-48`); `shutdown()` draws a
line at row 200 of a 640-wide layout from both edges inward (`for(x=20;x<=160;x+=3) …
tw_putpixel(x,200,0)`, `:116-131`), leaves a pixel at (160, 200), and pulses its palette for 60 frames
(`:132-138`).

### 10.1 The sweeps

8800 a horizontal line across black with a dot at its right end (234 non-black pixels in row 100,
x = 40 to 276); **8900, 9000, 9100 and 9200 a single non-black pixel at (160, 100)**; 9300 the next
part's picture partly drawn; 9400–9500 that picture at full contrast. At one-frame resolution
8791–8794 are a horizontal band (the collapsed picture, ~10 rows tall) and 8795–8799 the thin line.
The TECHNO→PANICEND change is pinned to the music (8784 in four sessions, §1.7).

### 10.2 The keyframes

| index | what is on screen | files | hashes |
|---|---|---|---|
| 8792 | the **collapse**: the picture squashed into a band around row 100, ~10 rows tall | `.rgb` only | rgb `c9bf5114…` |
| 8800 | the **line and the dot**: a 234-pixel line in row 100 with the nucleus at (160, 100) | `.rgb` only | rgb `bb38c96b…` |
| 9000 | the **dot alone**, its palette pulsing; the same bytes at 9100 and 9200 | `.rgb` + `.idx` | rgb `b412a265…`, idx `9d7106ca…` |

9000 is deep in the hold (8900–9230). Its `.idx`, read by hand, holds exactly one non-zero index, `1`,
at (160, 100) against 63,999 zeros, as the RGB shows.

### 10.3 Held or moving, measured — and the re-capture is identical, collapse included

| keyframe | neighbours | reading |
|---|---|---|
| 8792 | 8791/8792/8793 three `sha256` in two sessions | **moving**: the band is still shrinking |
| 8800 | 8799 = 8800 = 8801, and 8795–8799 | **held** |
| 9000 | 9000 = 9100 = 9200 | **held** |

Against a session holding `8791,8792,8793,8799,8800,9000`: exit 0, `verdict: identical`,
`mode: mixed`, offsets `{0: 3}`. 8792's bytes are the same in three sessions. The collapse is driven
by `dis_waitb()` counts, one step a frame, while GLENZ's rotation and TUNNELI's tunnel advance by
however many vblanks passed, so session spread is a property of some animated screens and not of
"moving" as such.

### 10.4 Where the part ends

9200, 9210 and 9220 are the dot's black; **9230** is the next part's picture arriving dark from the
top-right, fading in from 9240. The picture is `FOREST/HILLBACK.CLX`, an MNTSCRL asset, so PANICEND's
last drawn frame is the dot, the shell's music-cue wait holds it, and MNTSCRL's first frame is ~9230.
Both screens are 320×200: a content boundary.

---

## 11. MNTSCRL

`SECOND 3`'s first part. Its source is `FOREST/` (staged as `mntscrl.exe` by `FOREST/X.BAT:1`); the
runner restarts MUSIC1 at order 42 before it (`MAIN/U2.ASM:860-863`, guarded by `whattorun & 4` and
not `whattorun & 2`, i.e. exactly `SECOND 3`) and runs `exe9` (`:865-866`). 320×200 mode 13h
throughout.

`READ2.PAS:101-104` sets mode 13h and calls `dis_partstart`; `:110-115` reads 31 rows of 640 bytes
from `O2.SCI`; `:120-126` takes the palette and picture from `HILLBACK.CLX` (`BGR.PAS:2-6`), blits it
and sets the DAC black; `:139-147` fades the palette up over 64 frames; `:153-159` **waits for a music
cue** (`mov bx,6 / int 0fch`, loop while `muscode` is negative); `:170-181` runs 128 more frames; and
the main loop `:195-224` exits on the music (`dx = -11` at `:200`), not a frame count (`{inc(frame);}`
is commented out at `:221`, so `veke = 2800` never fires). The mechanism is the module's.

### 11.1 The sweeps

**400, 405 and 415 are grab-hostile** (`the emulator reached frame 420 before frame 405 could be
captured`, `reached 419 before frame 415`); **420 captures and is already 320×200**, flat black
(`ea0787f6…`). MNTSCRL's first graphics frame is in **(415, 420]**. Then: 425–440 the leaf overlay
alone, fading up (means 6.08, 8.91, 10.25, 10.53), saturating at 445; the hold to 545; the first
19-pixel change at 555 and the background fading in at 560–615 (mean 15.48 → 47.65); 600–2350 the
landscape with the leaf branch scrolling across it; the fadeout at 2400 (mean 14.54); black at 2450;
the next part's face fading in from 2460 (§11.5).

### 11.2 The keyframes

| index | what is on screen | files | hashes |
|---|---|---|---|
| 500 | the **held opening**: the leaf overlay alone against black at its resting palette; byte-identical from 445 to 545 and in all 8 sessions that took it. The music wait (`READ2.PAS:153-159`) is this hold | `.rgb` + `.idx` | rgb `e68aa887…`, idx `7be5af37…` |
| 1100 | the **scroller**, early: the leaf branch mid-frame, reaching up and right | `.rgb` only | rgb `deb4d8fd…` |
| 1700 | the **scroller**, middle: the branch shifted left, the pale ridge to its left | `.rgb` only | rgb `dc205228…` |
| 2300 | the **scroller**, late: the branch middle-right, ~80 frames before the fadeout | `.rgb` only | rgb `eb4598d4…` |

500's `.idx`, read by hand: 107 distinct indices, no zero bytes, every pixel exactly one RGB triple
per index (idx 42 ↔ `(24,65,24)`, idx 71 ↔ `(73,65,49)`, idx 126 ↔ `(49,49,49)`), and the 24,620
non-black RGB pixels are exactly the pixels whose index widens to non-black.

### 11.3 Held or moving, measured

| keyframe | neighbours | reading |
|---|---|---|
| 500 | 499 = 500 = 501 in six sessions; 445…545 one `sha256` | **held** |
| 1100 | three `sha256`; 1552 px (2.4250 %) and 1542 px (2.4094 %) a frame | **moving** |
| 1700 | three; 1554 px (2.4281 %) and 1576 px (2.4625 %) | **moving** |
| 2300 | three; 1972 px (3.0812 %) and 1960 px (3.0625 %) | **moving** |

The hold starts in (440, 445] (440→445 is the palette's last step, 269 px; 435→440 is 6998 px) and
ends in (545, 555) (a 19-px step). No frame inside the scroll holds: every step between 1099 and 2301
is 2.4–3.1 %.

**The moving frames are the same bytes at the same index in every session**, sweeps taken before the
reference included:

| index | sessions | rgb |
|---|---|---|
| 425 | 2 | `0f2daa91da02…` |
| 440 | 2 | `86eeadce81e9…` |
| 455 | 2 | `e68aa8870c98…` |
| 500 | 8 | `e68aa8870c98…` |
| 2100 | 2 | `0e99c02d4f26…` |
| 2200 | 2 | `291f3e851e1f…` |
| 2500 | 2 | `6a152a319d26…` |

The timeline is anchored to one music cue (the `READ2.PAS:153-159` wait) and then advances by
`dis_waitb()` counts, so every session draws the same frame at the same index (§1.11).

### 11.4 The re-capture and the census

Against three independent sessions holding all four triples: exit 0, `verdict: identical`,
`mode: mixed`, offsets `{0: 4}`. Census over the six sessions with a window: **6/6 at every
keyframe**, each presented at `N` (offset 0), with one picture at `N` for 500 and three in each moving
window.

### 11.5 Where the part begins and ends

| index | 2440 | 2450 | 2460 | 2470 | 2480 | 2490 |
|---|---|---|---|---|---|---|
| rgb | `fd5afb16…` | `ea0787f6…` | `5217d4e0…` | `40732954…` | `1ee6e8fb…` | `6a152a31…` |
| what | the scroller, faded to mean 3.92 | flat black | mean 16.13 | mean 35.04 | mean 38.95 | the next part's face, full |

The last full-brightness scroller frame is **2350** (mean 56.77); 2400 is the fadeout; 2450 the black
gap; **LNS&ZOOM's first frame is in (2452, 2456]** (§12.5). MNTSCRL runs from (415, 420] to a last
drawn frame at ~2445. Both screens are 320×200: a content boundary, with a black gap of two or three
frames.

---

## 12. LNS&ZOOM

`SECOND 3`'s second part: one binary holding two effects (`LENS/MAKEFILE:26` builds `lens.exe` and
stages it as `lns&zoom.exe`). `main` runs `part1(); while(dis_musplus()<-20); part2(); part3();`
(`LENS/MAIN.C:379-386`): `part1` (`:89-131`) wipes the demon-face picture on with two vertical edges
(`cp[x]=dp[x]`, `:114-129`) after a music wait (`:104`); `part2` (`:133-183`) steers the lens along
`pathdata1` for 715 frames (`:159-165`); `part3` (`:185-272`) is the rotazoomer (`rotpic90=back`,
`:191`), drawn until `frame = 2000` (`:211`) or a music break (`:213`), fading out from its frame 1872
(`:261-267`), and ending with `palette[a]=63` everywhere (`:274-275`), the **flat white screen**.
320×200 mode 13h (`setvmode(0x13)`, `:292`), from (2452, 2456] to the white that holds to ~5725.

### 12.1 The sweeps

2500–2900 the face, one picture; 2950 a dark blue lens arriving at the top-left; 3000–3500 the lens
crossing the face (left eye, centre, right eye, lower right, lower left, both eyes, leaving right);
3600 the plain face again; 3700 the face brighter, mid-change; 3800–5300 the rotazoomer (the face
rotated, magnified and sliding, blue bubbles over black); 5500 dark (mean 9.35); 5550 mean 94.44; 5600
mean 177.10; **5650 = 5700 flat white**, mean 255.00. Sweeps past ~5725 are refused (320×400).

### 12.2 The keyframes

| index | what is on screen | files | hashes |
|---|---|---|---|
| 2700 | the **face**, held: the demon face with the pentagram and yellow eyes, blue bubbles at the corners; byte-identical from 2490 to ~2945 | `.rgb` + `.idx` | rgb `6a152a31…`, idx `70175500…` |
| 3400 | the **lens** over both eyes, the face distorted behind it | `.rgb` only | rgb `6c81eadc…` |
| 4200 | the **rotazoomer**, early: the rotated face at the right, bubbles over black | `.rgb` only | rgb `19e5682f…` |
| 4900 | the **rotazoomer**, late: a magnified face fragment filling the lower half | `.rgb` only | rgb `248f6b83…` |
| 5650 | the **white screen** the part ends on, held: every pixel `(255,255,255)` | `.rgb` + `.idx` | rgb `20f3d62f…`, idx `f8636cd3…` |

Both `.idx` files were read by hand. 2700's holds 64 indices, each with exactly one RGB triple, and the
widened palette reproduces the RGB with **0 mismatches**. **5650's cannot vouch for anything**: its
palette is 63 in every used entry, so every index widens to white. Its plane is not flat (63 distinct
indices, 50,308 zeros: the rotazoomer's last index picture under a white palette) and is the same
bytes in all five sessions, but it says nothing about the stride (§1.6).

### 12.3 Held or moving, measured

| keyframe | neighbours | reading |
|---|---|---|
| 2700 | 2699 = 2700 = 2701; 2500…2900 at 50 the same bytes | **held** |
| 3400 | three `sha256`; 4312 px (6.7375 %) and 4367 px (6.8234 %) a frame | **moving** |
| 4200 | three; 10052 px (15.7063 %) and 8248 px (12.8875 %) | **moving** |
| 4900 | three; 25440 px (39.7500 %) and 24876 px (38.8687 %) | **moving** |
| 5650 | 5649 = 5650 = 5651, and 5700 | **held** |

The face's hold ends in (2900, 2950] (2950 is 2940 px from 2900). The lens phase returns the screen to
exactly the hold's picture: 3600, 3620 and 3640 are `6a152a31…` again (3580 is 3051 px from 3600,
3660 is 100 % different), a second hold in (3580, 3660). The rotazoomer moves 93.3–98.9 % of pixels
per 100 frames sampled (3700→3800 93.3438 %, 3800→3900 98.9062 %, 3900→4000 98.1312 %).

### 12.4 The re-capture and the census

Against three independent sessions holding all five triples: exit 0, `verdict: identical`,
`mode: mixed`, offsets `{0: 5}`. Census over five sessions: **5/5 at every keyframe**, each at `N`.
Sweeps taken before the reference hold the same bytes at all five keyframes, and the `.idx` planes are
the same in all five sessions and their ±1 neighbours.

### 12.5 Where the part begins and ends

MNTSCRL's last drawn frame is ~2445; **2450 and 2452 are flat black** (`ea0787f6…`); **2456 is
already the face** at mean 7.53. LNS&ZOOM's first frame is in **(2452, 2456]**, a content boundary.
The palette rises (2460 mean 16.13, 2480 38.95) and settles by 2490 (mean 40.30, 25,480 non-black
pixels), the hold's first frame byte-identical to 2700.

The end is a geometry change: sweeps are refused at 5900, 5750 and 5730 (`320x200 → 320x400`);
**5710 and 5720 are still the white** (one `sha256` with 5650) and **5730 is 320×400**. The white
holds to ~5725 and PLZPART's Mode X screen follows directly. Between 5450 and 5650 the screen goes
through the rotazoomer's fade (5500 mean 9.35) and up to white (5550, 5600), the picture still in the
index plane.

---

## 13. PLZPART

`SECOND 3`'s third part, and the part the split convention was written for. `PLZPART/X.BAT:1` stages
`plzpart.exe`; `PLZPART/MAIN.C` runs `init_copper(); initvect(); plz(); vect(); close_copper()`, and
`plz()` and `vect()` are **two screens**, filed as `reference/parts/plzpart/plasma/` and `cube/`:

- **plasma** (`plz()`, `PLZ.C:62-121`): `init_plz()` (`:171-179`) calls `tw_opengraph2()`
  (`TWEAK.ASM:44-67`): chain-4 off, CRTC 0x14 = 0, 0x17 = `0xe3`, 0x09 = 0 (400 lines), **0x13 =
  `0x3013h` → a 96-byte plane row**, 400 rows. `set_plzstart(60)` (`PLZ.C:177`) writes CRTC 0x18 = 60
  (`ASMYT.ASM:97-119`): a 320×400 screen with a **line-compare split at 60**.
- **cube** (`vect()`, `VECT.C:113-131`): `tw_opengraph()` (`TWEAK.ASM:19-37`), CRTC 0x09 = 2, **0x13 =
  `0x5013h` → a 160-byte plane row**, 200 rows: the memory layout is twice as wide as the picture.

Every frame of both screens is RGB-only (each redraws most of the picture every frame, §13.5), so no
`.idx` exercises either stride and the comparison is `mode: rgb`. A third capture, `band/`, is the fit
corpus of §1.13.

### 13.1 The sweeps

Plasma (320×400): 5800 a pale, washed-out plasma (the `cop_drop=128` entrance); 6000–6600 blue and
magenta with flat bands above and below (`(4,4,4)` at 6000, black by 7000); 6800–7200 green and blue;
7400–7600 cyan and orange; 7800–8000 almost black with a faint grey shape. From 6800 every tile has
**rows 0–60 black, rows 61–340 the plasma, rows 341–399 black**. Cube (320×200): 8500 a small
rainbow-textured cube in the upper left over black; 9000 the cube filling the frame; 9500 larger and
open; 10000 a green plasma face across the frame; 10500 black above a grey gradient (MINVBALL's
backdrop); 11000–12500 MINVBALL's dots.

### 13.2 The split, measured — and where the capture puts it

Three consecutive plasma frames:

| index | first row with a non-black pixel | last |
|---|---|---|
| 6989 | **62** | 341 |
| 6990 | **61** | 340 |
| 6991 | **62** | 341 |

`pompota` (`COPPER.ASM:126-143`) sets CRTC 0x18 to `0x3c` (60) on one frame and `0x3d` (61) on the
next (the 8th bit cleared through 0x07 at `:126-129`). **The lower view's first displayed line is the
register plus one**, 60→61 and 61→62 (D16, `Split::scanline`).

The upper view is black because its start address (`cop_start = 96*(682-400) = 27072`, `PLZ.C:176`)
points past the drawn plasma, so the measured boundary is where the lower view begins, not a picture
change. `do_drop` (`COPPER.ASM:189-268`) animates the same register at the end of each plasma phase:
8289/8290/8291 have their first content row at **195 / 202 / 209**.

### 13.3 The keyframes

| index | screen | what is on screen | rgb |
|---|---|---|---|
| 6000 | plasma | first palette: blue and magenta over the middle 280 rows, a flat **`(4,4,4)`** above and below (the file has no zero byte; at 7000 and 7600 the bands are `(0,0,0)`) | `308d4d3a…` |
| 7000 | plasma | second palette: green and blue | `6f41f697…` |
| 7600 | plasma | third palette: cyan and orange | `9a8f3fe5…` |
| 8500 | cube | small: a rainbow-textured cube in the upper left over black | `11fd1186…` |
| 9500 | cube | large: the cube's open face across the top half | `4190291b…` |

All `.rgb` only. The plasma steps its palette through the tables in `pals` as `cop_drop` fires, so
one keyframe per palette state.

### 13.4 The refusals the tool made, and what 0xC0 is

```
warning: indexed capture failed (the screen is not one the indexed read can describe, so this frame
         keeps its RGB: this screen's rows are 384 bytes and it is 400 rows tall, so it needs 153600
         bytes a plane; one plane holds 65536, and the display wraps); keeping RGB only
```

384 bytes is not a stride this part sets (the plasma's offset is `0x30`, `TWEAK.ASM:65-66`; the cube's
`0x50`, `:34-35`). It is either 0xC0, the low byte of `cop_start = 0x69C0` that `COPPER.ASM:69-72`
writes to CRTC 0x0d every frame, read as the offset register; or `0x30 × 8`, the plasma's own offset
read as a chained row because `chain4_enabled()` raced `PLZ.C:95-118`'s 3c4h writes (§1.9). It
happened at 6000 in two runs and not at 7000 in the same runs, and produced no artefact: every frame
here is RGB-only for motion anyway.

### 13.5 Held or moving, measured, and what the two screens do across sessions

| keyframe | neighbours | reading | rate a frame |
|---|---|---|---|
| plasma 6000 | three `sha256` | **moving** | 89,719 px of 128,000 = **70.0930 %**; then 70.0828 % |
| plasma 7000 | three | **moving** | 87,805 px = **68.5977 %**; then 68.5664 % |
| plasma 7600 | three | **moving** | 89,920 px = **70.2500 %**; then 70.2500 % |
| cube 8500 | three | **moving** | 16,056 px of 64,000 = **25.0875 %**; then 24.1250 % |
| cube 9500 | three | **moving** | 35,711 px = **55.7984 %**; then 59.6313 % |

The plasma saturates (a split that moves one row moves every row below it), so a percentage there is
a one-frame statement.

**The cube's frames are byte-identical at the same index in every session; the plasma's are the same
picture to within a fraction of a frame.** Against the filed 6000, other sessions' 6000 frames are
**0 px**, 118 px (0.0922 %), 210 px (0.1641 %), 316 px (0.2469 %) and 3942 px (3.0797 %) away: a
thousandth to a twenty-third of the screen's own per-frame move. A one-frame start difference would
show ~70 % at a fixed index, so the spread is sub-frame: the point within a frame at which the plasma's
C loop (even rows with one parameter set, odd rows with another, then again with the planes swapped,
`PLZ.C:95-118`) meets the copper's once-a-frame `moveplz`/`pompota` rewrite (`COPPER.ASM:104-107`,
`:126-143`). That is not resolved further. A neighbour frame is 68.57–70.25 % away, so the ±1 window
recovers nothing here.

### 13.6 The re-capture and the census

**Cube**: against four independent sessions holding `8499…8501,9499…9501`: exit 0, `verdict:
identical`, `mode: rgb`, offsets `{0: 2}`. **Plasma**, against two sessions:

| session | 6000 | 7000 | 7600 |
|---|---|---|---|
| A | 0.0000 % (0 px) | 7.7344 % (9,900 px), max delta 21 | 9.3281 % (11,940 px), max delta 17 |
| B | 0.1641 % (210 px), max delta 16 | 12.9266 % (16,546 px), max delta 20 | 24.4844 % (31,340 px), max delta 25 |

Every frame pairs at offset 0. The census counts every capture that holds the window, including runs
refused later (one session refused at 7601 holds valid windows at 6000 and 7000):

| keyframe | sessions with a window | filable pictures (seen at `N`) | support of the filed picture | largest support over the window | filed ≥ half? |
|---|---|---|---|---|---|
| cube 8500 | 4 | 4 | `11fd1186…` **4/4** | 4/4 | yes |
| cube 9500 | 4 | 4 | `4190291b…` **4/4** | 4/4 | yes |
| plasma 6000 | 5 | 5 | `308d4d3a…` **2/5 (40 %)** | `390341ec…` 3/5, **only at `N−1`**, not filable | **no** — no settled content |
| plasma 7000 | 4 | 4 | `6f41f697…` **0/4** | `8501036521b3…` and `ac34f7c7334f…` 2/4, only at `N−1`/`N+1` | **no** — no settled content |
| plasma 7600 | 3 | 3 | `9a8f3fe5…` **0/3** | nine pictures, 1/3 each | **no** — no settled content |

At 6000 the filed picture is the largest among those taken at `N` (2/5 against 1/5 each); at 7000 and
7600 every picture taken at `N` is a singleton, so no filing can do better. Nothing was re-filed: the
pairwise distances are 0.0187–16.16 % at 7000 and 0.0422–25.45 % at 7600, every one with a max channel
delta of 29 or less, a fraction of a frame on this screen, and a re-capture would roll the same
sub-frame draw again. A residual of a few per cent at these indices is the oracle's; 8500 and 9500
are the part's exact targets.

### 13.7 Where the part begins, ends, and changes screens

- **LNS&ZOOM → plasma**: refused at 5730 (`frame 5730 is 320x400 where the capture started at
  320x200`); the change is in **(5720, 5730]**, the `tw_opengraph2` mode set in `init_plz()`
  (`PLZ.C:175`).
- **Plasma → cube**: 8310 is 320×400 and 8340 is refused (320×200); the change is in **(8310,
  8340]**, `tw_opengraph()` (`VECT.C:114`).
- **Cube → MINVBALL**: both 320×200, a content boundary.

| index | 10100 | 10200 | 10300 | 10400 |
|---|---|---|---|---|
| what | the cube, 156 colours | 79 colours, dark olive | 51 colours, dark olive | **pure black**, one colour |

10200 and 10300 are the cube dimming; 10400 is MINVBALL's screen with its palette zeroed
(`DOTS/MAIN.C:151-155`). The cube's screen runs to (10300, 10400].

---

## 14. MINVBALL

`SECOND 3`'s fourth part. `DOTS/MAKEFILE:20` builds `dots.exe` and stages it as `minvball.exe`; the
source is `DOTS/MAIN.C` (512 dots, `:91`; the physics constants at `:102-113`) and `DOTS/ASM.ASM`'s
`drawdots` (`:52-218`). `:125-126` sets mode 13h; `:127-150` builds and reads back a palette; `:151`
zeroes the DAC; `:152-169` fills rows 100–199 with an index ramp and copies it to `bgpic`; `:171-183`
fades the palette up over 65 steps of two vblanks (130 frames); `:185` starts `while(!dis_exit() &&
frame<2450)`, breaking also on the music (`:191-195`). The dots go through four position phases
(`frame<500`, `<900`, `<1700`, `<2360`; `:202-241`), then the palette brightens and whitens. 320×200
mode 13h.

### 14.1 The sweeps

10600 a handful of dots and a faint ellipse over the grey floor; 10900 a ring of dots across the top
with a scatter below; 11200 a large ellipse over a vortex; 11500 the ellipse with a dense band; 11800
a fountain or tree shape; 12100 the same, narrower; 12400 sparse dots across the frame; 12700 a wide
cloud; 13000 RAYSCRL's screen (no geometry change). Four shapes in the order of the source's four
phases (500/400/800/690 frames). The first dots are visible at 10600 and the background alone at
10500, so the loop begins in (10500, 10600].

### 14.2 The keyframes

| index | what is on screen | rgb |
|---|---|---|
| 10900 | the **ring**: dots across the upper third over a scatter | `726ec7e5…` |
| 11200 | the **vortex**: a wide ellipse over a swirling column | `954e4b30…` |
| 11800 | the **fountain**: dots rising into a tree shape | `20764312…` |
| 12400 | the **scatter**: dots across the whole frame | `33dce4e8…` |

All `.rgb` only (the dots move every frame).

### 14.3 Held or moving, measured

| keyframe | neighbours | reading | rate, of 64,000 pixels a frame |
|---|---|---|---|
| 10900 | three `sha256` | **moving** | 3306 px = **5.1656 %**, then 3201 px = 5.0016 % |
| 11200 | three | **moving** | 4548 px = **7.1063 %**, then 4425 px = 6.9141 % |
| 11800 | three | **moving** | 4222 px = **6.5969 %**, then 4557 px = 7.1203 % |
| 12400 | three | **moving** | 2999 px = **4.6859 %**, then 3324 px = 5.1937 % |

The moving frames are byte-identical at the same index in every session, a sweep taken before the
reference included: the timeline is anchored at its start (§1.11).

### 14.4 The re-capture and the census

Against three independent sessions holding all four triples: exit 0, `verdict: identical`,
`mode: rgb`. Census **3/3 at every keyframe**. One session answers all four at **offset −1** with
0.0000 % (max delta 0): its start landed one frame early, which is what the ±1 window is for.

### 14.5 Where the part begins and ends

**MINVBALL's first frame is in (10300, 10400]** (§13.7): 10400 is its screen with the palette zeroed
(`DOTS/MAIN.C:151`), 10500 the background rising through the fade. A content boundary.

| index | 12700 | 12800 | **12900** | 13000 |
|---|---|---|---|---|
| colours | 47 | 47 | **1** | 35 |
| what | dots scattered | the scatter, still | **flat `(231,231,231)`**, the white-out | RAYSCRL |

12900 is one flat colour, `(231,231,231)` in one session and `(239,239,239)` in another: the invariant
is "one colour in the last ~40 frames", not a value. `DOTS/MAIN.C:242-256` re-adds the palette scaled
by how far past frame 2360 the frame is, and `:258-270` takes every entry down from 63 together
(`c=63-a*2`). MINVBALL's drawing ends in **(12900, 13000]**, about 2550 frames after its first (the
2450-frame cap plus the 130-frame fade), with no geometry change.

---

## 15. RAYSCRL

`SECOND 3`'s fifth part: `WATER/X.BAT:1` stages `demo.exe` as `rayscrl.exe`, and the part is
`WATER/DEMO.PAS` plus `WATER/ROUTINES.ASM`'s `Putrouts1`. **320×200 mode 13h, chain-4**
(`DEMO.PAS:91-92`), and **the part writes no CRTC register**: its only ports are the DAC's
(`DEMO.PAS:3-8`). A raytraced scene (a large glossy blue sphere, a smaller one below, ripple rings on a
blue floor, and one animated object) is copied into video memory once (`:112`) and scrolled:
`scr(sss)` (`:51-80`) calls `Putrouts1` with one of three position maps, `sss` cycling 0,1,2 (`:130`,
`:180-181`). The palette fades in over 128 frames (`:117-132`) and out over 64 (`:167-178`) when the
music reaches the polled position (`cmp dx,-11`, `:158-166`). Its extent on `SECOND 3` is (12900,
13000] to (14900, 15000].

### 15.1 The sweeps

The scene mid fade-in at 13000 (35 colours, mean 23.15); scrolled with no object at 13500 (190
colours); a golden comet at 14000; a green-and-grey screwdriver-like object at 14500 (230 colours);
the object decomposing into a spray of small squares at 14600–14900; the fadeout's tail at 15000 (11
colours, mean byte 0.11, 63,303 of 64,000 px black); 3DSINFLD's terrain at 15100. Sweeps from 13000
are refused at 17100 and 17500 (320×400): the boot's 320×200 screen ends in **(17000, 17100]**.

### 15.2 The keyframes

| index | what is on screen | files | hashes |
|---|---|---|---|
| 13000 | the scene **mid fade-in** | `.rgb` only | rgb `62ef53c7…` |
| 13500 | the scene scrolled, no object | `.rgb` + `.idx` | rgb `92dc2c9e…`, idx `afda560c…` |
| 14000 | the scene with the **golden comet** | `.rgb` only | rgb `6715eece…` |
| 14500 | the scene with the **screwdriver object** | `.rgb` only | rgb `d1cb7f59…` |

### 15.3 Held or moving, measured — and the `.idx` hand-check

The same bytes in two sessions at every index:

| keyframe | `K-1` | `K` | `K+1` | reading | per-frame change of 64,000 px |
|---|---|---|---|---|---|
| 13000 | `ccb4bb02…` | `62ef53c7…` | `8f456780…` | **moving** | 12999→13000 **64 px (0.1000 %)**, 13000→13001 23,908 px (37.3563 %) |
| 13500 | `92dc2c9e…` | `92dc2c9e…` | `92dc2c9e…` | **held** | 0 px either side |
| 14000 | `36f5c972…` | `6715eece…` | `8ee47d77…` | **moving** | 2059 px (3.2172 %) and 1962 px (3.0656 %) |
| 14500 | `0158b387…` | `d1cb7f59…` | `123f0056…` | **moving** | 4079 px (6.3734 %) and 3100 px (4.8438 %) |

`.idx` is present at 13499–13501, 15050 and 17000 (the held frames and two flat blacks) in every
session and absent elsewhere: the coherence rule, not a rule about holds.

`frame_013500.idx` read by hand: 190 distinct indices to 190 distinct RGB triples, no index carrying
two, and against the manifest's palette with the `(c*259+33)>>6` widening it reproduces the RGB with
**0 mismatches of 64,000 pixels**. The palette does not collapse, so the check is real. The plane is
byte-identical (`afda560c…`) across sessions.

### 15.4 The re-capture and the census

The partner list carries 3DSINFLD's keyframes as well (one session answers both parts):

```bash
cargo xtask reference capture --arg 3 --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames 12900,12950,12999,13000,13001,13499,13500,13501,13999,14000,14001,14499,14500,14501,\
14950,15000,15050,15499,15500,15501,15999,16000,16001,16499,16500,16501,16799,16800,16801,16900,\
16950,17000 --out <session>
```

Against three such sessions and a fourth independent one: exit 0, `verdict: identical`,
`mode: mixed` (13500 by indices), offsets `{0: 4}`. Census over the four: **4/4 at every keyframe**,
every one at offset 0. The timeline is anchored (§1.11).

---

## 16. 3DSINFLD

`SECOND 3`'s sixth part. `COMAN/MAKEFILE:18` builds `c.exe` and stages it as `3dsinfld.exe`; the
sources are `COMAN/MAIN.C`, `COMAN/ASM.ASM` (`inittwk`), `COMAN/THELOOP.ASM` and the wave tables
`w1dta.bin`/`w2dta.bin`. 320×200: `mov ax,13h; int 10h` (`MAIN.C:124-126`), then `inittwk()`
(`ASM.ASM:36-78`) clears the palette, writes CRTC 0x14 = 0 and 0x17 = 0xe3, turns chain-4 off and never
writes 0x13: **80-byte plane rows**.

A voxel terrain under a black sky (index 0; the palette is a formula, `MAIN.C:128-150`). `doit()`
(`MAIN.C:66-118`) waits on the music, then runs `while(!dis_exit() && frame<4444)` with a music break
(`a=dis_musplus(); if(a>-8 && a<0) break;`, so the cap is never reached here), rendering into the
off-screen page and **toggling CRTC 0x0c between 0x80 and 0x00** (`:101-110`). `frame += (frepeat =
dis_waitb())` counts vblanks and each render costs ~2, so **the picture changes every second frame**.

Its extent is **(15000, 15100] to (16900, 17000]**. 15100 is the terrain entering from the lower left
(58 colours, mean 7.95, content rows 167–199); 17000 is flat black (`ea0787f6…`); JPLOGO's 320×400
screen is up by 17100.

### 16.1 The sweeps

The terrain enters from the lower left at 15100 (58 colours, content rows 167–199), climbs (15300:
rows 90–199), flies under a thin black sky band (15400–15500), a wide band and a rising peak (16000),
a red-magenta ridge at the right (16500, 64–67 colours), and sits in the lower half under a wide band
(16800: rows 86–199; 16900: rows 109–199). 17000 is flat black.

### 16.2 The keyframes

| index | what is on screen | rgb |
|---|---|---|
| 15500 | the terrain newly entered, under a thin sky band | `be95db6c…` |
| 16000 | mid-flight, a peak rising | `2d349ada…` |
| 16500 | mid-flight, the red-magenta ridge | `9d129ad0…` |
| 16800 | near the end, the terrain low | `6e5994bf…` |

All `.rgb` only (the terrain redraws every second frame, 37–40 % of pixels between pictures). No
`.idx` is filed, although the part writes CRTC 0x0c every frame and `0x80` read as the offset register
is a 256-byte row `plan_indexed_read` accepts (§1.10).

The filed files are one re-capture session's (§16.4).

### 16.3 Held or moving, measured

| keyframe | `K-1`, `K`, `K+1` in one session | reading |
|---|---|---|
| 15500 | `6a891af9…`, `be95db6c…`, `be95db6c…` | moving, two-frame step |
| 16000 | `2d349ada…`, `2d349ada…`, `2744df3f…` | moving, two-frame step |
| 16500 | `9d129ad0…`, `9d129ad0…`, `19a8a5b0…` | moving, two-frame step |
| 16800 | `47a0b326…`, `6e5994bf…`, `6e5994bf…` | moving, two-frame step |

A session one frame out presents the same picture at `N±1` (0.0000 %); a session two steps out sits on
another picture and reports 38.9031 % or 41.7875 %.

### 16.4 The re-capture and the census

Against the three RAYSCRL sessions (§15.4):

| session | offsets | result |
|---|---|---|
| A | `{0: 3, +1: 1}` | exit 0, identical (15500 at +1) |
| B | `{0: 4}` | exit 0, identical |
| C | `{-1: 3, 0: 1}` | exit 1: 15500 and 16000 0.0000 % at −1; 16500 **38.9031 %** (24,898 px) and 16800 **41.7875 %** (26,744 px), two two-frame steps out |

The reference was re-captured into the largest groups: 15500 was `6a891af9…` (1/3) and is
`be95db6c…` (3/3); 16500 was `538942e6…` and is `9d129ad0…`, taken at `N` by three of the four
sessions that hold its window; 16800 was `47a0b326…`, which sessions present only at `N−1`, and is
`6e5994bf…`, taken at `N` by both that present it. 16000 kept its bytes. A re-capture here is a draw
from a small set of two or three pictures (unlike JPLOGO's, §17.3).

| keyframe | sessions with a window | pictures at `N` | largest support | filed? | ≥ half? |
|---|---|---|---|---|---|
| 15500 | 3 | 3 | `be95db6c…` **3/3**, in all three windows | yes | yes |
| 16000 | 3 | 2 | `2d349ada…` **3/3** (tied with `2744df3f…`) | yes | yes |
| 16500 | 4 | 3 | `9d129ad0…` **3/4 (75 %)**, at `N` in three | yes | yes |
| 16800 | 3 | 2 | `6e5994bf…` **2/3 (67 %)**, at `N` in both | yes | yes |

**All four keyframes are settled**; session C's residual is a phase group.

---

## 17. JPLOGO

`SECOND 3`'s seventh and last part. `JPLOGO/MAKEFILE:24` builds `jp.exe` and stages it as
`jplogo.exe`; the sources are `JPLOGO/JP.C` and `JPLOGO/ASM.ASM` (linked with `_PIC.OBK` and
`READP.C`). **320×400 Mode X**: mode 13h (`JP.C:173-174`) then `inittwk()` (`ASM.ASM:36-74`): CRTC 0x09
= 0 (400 rows), 0x14 = 0, 0x17 = `0xe3`, chain-4 off, and never 0x13, so **80-byte plane rows**, which
`lineblit` (`ASM.ASM:76-100`) writes.

The "Jellypic" (a woman in armour on a white horse with a sword) is read into 400 rows of 184 bytes
(`readp`, `JP.C:181-186`) and drawn with `linezoom(vram+y*80, row[b], a)` (`:96-99`). Three phases:

- an **entry** of ~83 frames (`while(y>0) { y -= a; a += 6; scrolly(y/64); dis_waitb(); }`,
  `:210-220`), in which `scrolly` (`:30-45`) walks CRTC 0x0d and 0x0c and the picture slides in; held in
  pairs (17300 = 17301 in two sessions);
- a **wobble** after a music wait (`:71`, `dis_musplus()<4`): 17400→17500 10.8250 %, 17500→17600
  9.9680 %;
- a **hold** of ~350 frames: in one session 17500, 17501, 17600, 17699, 17700, 17701, 17800 and 17850
  are one `sha256` (`c8d6faa0…`), 17499 is `50e5a1c6…` and 17900 the white-out. `doit` recomputes
  `y1`/`y2` only while `frame <= 511` (`JP.C:79-83`) and the tables saturate, but **which state it
  holds is the session's** (§17.3).

Its extent is **(17000, 17100] to (17900, 18000]**, the last 320×400 screen of this boot (the 320×200
screen after it is U2E's, §18.6). 17000 is the loader's flat black; 17900 is a white rectangle (2
colours, 74,221 px white): ENDLOGO's opening white over JPLOGO's leftover memory (`END/END.C:61-63`
writes 63 into 765 bytes from DAC index 0, entries 0–254).

### 17.1 The sweep

17100 a band of the picture across the top (the entry just begun); 17200–17400 the picture further in
(the woman, the horse's head, the sword) with black bands at its edges; 17500–17800 the picture at
full size, wobbling, the scale's comb pattern at its edges; 17900 the white rectangle. Refused at
18000 (`frame 18000 is 320x200`).

### 17.2 The keyframes

| index | what is on screen | rgb |
|---|---|---|
| 17300 | the picture **mid-entry** | `4b31e501…` |
| 17500 | the picture **wobbling** | `aab611ef…` |
| 17700 | the picture **held**, in the reference's own session | `91d6ce6b…` |

All `.rgb` only: the part's own register writes refuse the read loudly (§17.4) and motion refuses the
rest.

### 17.3 Held or moving: the hold is real, its states are discrete, and they recur

Four sessions (A–D) hold all three windows; a sweep (S) holds 17500 and 17700.

| keyframe | across sessions | reading |
|---|---|---|
| 17300 | A: 17299 `87c7fd8d…`, 17300 = 17301 = `4b31e501…`; all four the same | **held** (in pairs), one picture in every session |
| 17500 | A `f8896c45…`; B = D = the reference `aab611ef…`; C `fe7c6802…` | **held per session**, several states |
| 17700 | S `ae2c3d08…`; A `c8d6faa0…`; B = D `ec3de2bb…`; C `c60e2fbc…` | **held per session**, several states |

**The states are discrete, they recur, and the set is wide.** B's and D's 17700 and 17500 are
byte-identical (0 px of 128,000): D, a fresh session, landed in an existing state. Across twelve
captures of the recipe (the four sessions, the sweep and seven re-capture probes) there are **ten
distinct byte-images at 17700** and nine at 17500, and the only repeat among the probes is one pair
(`4b94ea6d…` at 17500 and `e9bc3a26…` at 17700, drawn twice). No particular state is reliably
reachable by re-capturing.

**The states differ by patches of rows**, at 17700:

| pair | differing px | rows with no difference |
|---|---|---|
| B vs D | **0** | all 400 |
| A vs C | 13,519 | rows 0–17 and 159–399 (the difference is confined to rows 18–158) |
| S vs A | 39,919 | only rows 397–399 |
| A vs B | 53,138 | none |
| reference vs A | 37,852 | only rows 393–399 |

No integer vertical shift explains any of them (best +0 at 31.19 % for S vs A; ±1…4 are 35–41 %), and
56 of 57 colours are shared. `doit` redraws a row only when its `(b, a)` differ from last time
(`lasty/lasts`, `JP.C:96-99`), so a row whose parameters repeat keeps an earlier state's pixels.

A replication of `JP.C:110-165`'s integer arithmetic gives `framey1[95:] = 0` and `framey2[95:] =
6400`, so `(y1, y2) = (0, 400)` for every doit frame from 379 up. **The hypothesis** for the variation:
the doit counter can jump past 511 in one step (`frame += waitb()`, `JP.C:74-77`), freezing `y1`/`y2`
at a pre-saturation value, which the palimpsest then preserves. `a` is not uninitialised:
`for(a=0;a<200;a++)` leaves it at 200 before `la=a` (`JP.C:140`).

### 17.4 The racy offset read, measured on this part

The sweep's log holds **eight `PlaneTooSmall` refusals naming a 320-byte row** for a picture whose
rows are 80: 160 units, the low byte of `y*80` for `y ≡ 2 (mod 16)`, which `scrolly` writes to CRTC
0x0d during the entry (`JP.C:36-45`). 0x0d's value read as 0x13's (§1.7, §1.10). No `.idx` is filed;
the one a session produced (B's 17700) reproduces its RGB with **0 mismatches of 128,000 pixels**
(59 indices, 57 colours), so the held plane is real at 80 bytes.

### 17.5 The re-capture and the census

Against the four sessions (each holding `17299…17301`, `17499…17501`, `17699…17701` and more),
`mode: rgb`, offsets `{0: 3}`, exit 1:

| session | 17300 | 17500 | 17700 |
|---|---|---|---|
| A | 0.0000 % | 41.3773 % (52,963 px) | 29.5719 % (37,852 px) |
| B | 0.0000 % | 0.0000 % | 40.1914 % (51,445 px) |
| C | 0.0000 % | 39.3516 % (50,370 px) | 27.3664 % (35,029 px) |
| D | 0.0000 % | 0.0000 % | 40.1914 % (51,445 px) |

| keyframe | sessions with a window | pictures at `N` | largest support | filed reference is it? | ≥ half? |
|---|---|---|---|---|---|
| 17300 (held) | 4 | 2 | `4b31e501…` **4/4**, at `N` in all four | yes | yes |
| 17500 | 4 | 3 (six in the windows) | `aab611ef…` **2/4 (50 %)**, the filed one | yes | exactly half |
| 17700 (held) | 4 | 3 | `ec3de2bb…` **2/4 (50 %)**, the state B and D hold | **no**: the filed `91d6ce6b…` is **0/4** | yes for the state, no for the file |

**17700 is settled as a state and unfiled as a file.** Seven re-capture probes of the reference's own
command drew at 17700 `bd189d56…`, `b617d280…`, `e9bc3a26…`, `267894aa…`, `871848e1…`, `27ca885d…`
and `e9bc3a26…` again: six distinct states, none held by a census session (one of them reproduced
17500's `aab611ef…` while drawing an unheld 17700). The reference keeps 17500 = `aab611ef…` and 17700
= `91d6ce6b…`, taken at `N` by none of the four, and a residual at 17700 is the oracle's (§1.12).

---

## 18. U2E

`SECOND 4`'s only part. `VISU/C/MAKEFILE:45` builds `u2e.exe`; the source is `VISU/C/U2E.C` over the
VISU library. A copper-driven scene player: `readfile(scene.00M)` plus animation streams (`.0AA`,
`.0XY`), drawn with the `vid_*` primitives under `dis_setcopper(2, copper2)` and
`vid_window(0,319,25,174,512,9999999)` (`U2E.C:352-353`). The copper (`:115-141`) rotates **four
display buffers** through the CRTC start address (`vid_setswitch(-1, clr)`), counting `syncframe` once
a vblank, and the main loop (`:376-540`) advances `currframe += repeat*2` until the stream ends. 320×200.

### 18.1 Which boot reaches it, and what changes between boots

- **`SECOND 3` and `SECOND 4` reach it.** `partsmask[3]` is `11100b` (`MAIN/U2.ASM:110-119`), bits 2, 3
  and 4 (the table's comment: "`&4=mount..comanche &8=vector &16=scrollers`"), so `test whattorun,8`
  (`:880-887`) runs `exe19` on `SECOND 3` after JPLOGO. `partsmask[4]` is `11000b`, bits 3 and 4: U2E is
  the first show part. **This row's indices are `SECOND 4` boot indices.**
- **The screens do not change between the boots; the indices do.** After JPLOGO U2E skips
  `vid_init(1)` (`jellywas = 1`, `U2E.C:195-215`), but `fadeset()` on all four buffers and **CRTC 9 =
  `(old & 0xf0) | 0x80`** (`U2E.C:302-313`) re-establish the 200-line planar display either way
  (§18.6).
- **The music is `MUSIC0` at order 18** (`U2.ASM:882-885`, before `exe19`). On `SECOND 4` the initial
  load starts `MUSIC1` at order 0 (`:767-775`) and U2E's restart replaces it.
- **U2E writes the CRTC every frame** (`_vid_setswitch` writes 0x0c, `VISU/AVID.ASM:341-357`; CRTC 9
  once, `U2E.C:305-308`), so any `.idx` from it would need the hand-check (§1.10); none is filed.

### 18.2 The sweeps

420 is still the loader's flat black (`ea0787f6…`); a palette fade-in is under way at 500 (4 colours);
the opening hold is `befaf38ae1…` at 530 = 590 = 660 and `ef539f9e…` at 600 = 700. Then the scenes: a
corridor with white door panels and a small craft (800–900), a console with red bars (1000), a grey
building in a red frame (1200), the fly-by over striped towers with a green craft (1400–1800), an
interior corridor after a scene change (2000, 11 colours), a structure with white square windows on a
red floor (2300–3000), a purple-grey wall and lattice (3500), **"Future Crew"** in 3-D letters over red
(4000). 5000 is CRED's credits screen. A sweep is refused at 4400 (`frame 4400 is 320x400`), ENDLOGO's
screen. Four indices 20 apart are refused as a miss (one batch, the next index 30 frames past its read,
§1.3).

### 18.3 The keyframes

| index | what is on screen | rgb |
|---|---|---|
| 1000 | a dark grey-brown **corridor wall** with a grey panel at its left (36 colours, mean 81.52) | `9cf5d972…` |
| 1600 | the **green saucer** in the foreground with grey buildings behind (79 colours, mean 80.25) | `e7a10cd7…` |
| 2600 | the **saucer under vertical striped towers** (56 colours, mean 97.17) | `58293d89…` |
| 3600 | two small **metallic crescent craft** over a grey plane on black (54 colours, mean 62.77) | `f8d4a0e8…` |

All `.rgb` only: the buffers rotate under the read.

### 18.4 Held or moving, measured

Every keyframe is **moving**, and the buffer rotation is not locked to a fixed period. Triples per
session (A–C, and an independent D):

| keyframe | A | B | C | D |
|---|---|---|---|---|
| 1000 | 999 = 1000, 1001 differs | 999 = 1000 | 999 = 1000 | 999 = 1000 |
| 1600 | three distinct | three distinct | three distinct | 1600 = 1601 |
| 2600 | three distinct | 2599 = 2600 | 2599 = 2600 | 2599 = 2600 |
| 3600 | 3599 = 3600 | three distinct | 3599 = 3600 | 3599 = 3600 |

Where a session's phase is within ±1 the comparison reports 0.0000 %; where it is two or more buffer
steps away its frame differs by **55.4672–60.8406 %**. The rotation follows the schedule the main loop
writes into `cl[].frames` (`U2E.C:434-444`) from `syncframe`, a music-synced counter.

### 18.5 The re-capture and the census

Against D: exit 0, `verdict: identical`, `mode: rgb`, offsets `{0: 4}`. Against A, B and C: exit 1,
each missing exactly one keyframe (A 2600 at 58.4328 %, B 3600 at 55.4672 %, C 1600 at 60.8406 %).

| keyframe | sessions with a window | pictures at `N` | largest support | filed? | ≥ half? |
|---|---|---|---|---|---|
| 1000 | 4 | 2 | `9cf5d972…` **4/4**, at `N` in all four | yes | yes |
| 1600 | 4 | 6 | `e7a10cd7…` **3/4 (75 %)**, at `N` in three | yes | yes |
| 2600 | 4 | 5 | `58293d89…` **3/4 (75 %)**, at `N` in three | yes | yes |
| 3600 | 4 | 5 | `f8d4a0e8…` **3/4 (75 %)**, at `N` in three | yes | yes |

**Every keyframe is settled.** The reference was re-captured once: its first 2600, `085bffff…`, was in
no session's window (48.65–57.07 % from them), and the re-capture drew `58293d89…`, 0/4 → 3/4, with the
other three keyframes unchanged.

### 18.6 `SECOND 3` reaches U2E too — measured

| index (`SECOND 3`) | frame | what it is |
|---|---|---|
| 17900 | `668f1e1364…`, 320×400 | the white-out over JPLOGO's memory, not U2E (a capture starting there is refused at 18000, `frame 18000 is 320x200`) |
| 18000 | `befaf38ae151…`, 2 colours, mean 191.25 | **U2E's opening hold**, byte-identical to the `SECOND 4` boot's 530 = 590 = 660 |
| 18100 | `befaf38ae151…` | the same hold |
| 18200 | `09e7a22c15d9…`, 42 colours, mean 138.70 | the first scene resolving |
| 18500–21000 (500 apart) | 44, 82, 13, 57, 52, 54 colours | U2E's scenes |

A second `SECOND 3` session reproduces 18000 and 18100 (`befaf38ae151…`, with a `.idx` each); its later
frames are the same scenes in another phase (18500 differs by 9.5307 % of bytes, §18.4). The 320×200
screen after JPLOGO on `SECOND 3` is U2E's, ~4000 frames long, ending in (21500, 22000] (§19). The
reference serves any boot that reaches U2E; a `SECOND 3` or full-run comparison uses that boot's
indices.

### 18.7 The part's first visible frame, and the derived run anchor

On the `SECOND 4` boot the loader's flat black `ea0787f6…` holds through 466, and **467** is the first
frame with a second colour: a white band at columns 68–252, 185 white and 135 black pixels in every row
(37,000 white, 27,000 black), `6eb4e4cd…`, the frame the intro's `fadeset` draws. Run twice in
independent sessions, every index identical:

```bash
cargo xtask reference capture --arg 4 --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames 456,457,458,459,460,461,462,463,464,465,466,467,468,469,470,471,472,473 --out <scratch>
```

(A coarser grid from 404 is refused on this host, `the emulator reached frame 419/420 before frame 404
could be captured`; from 420 it agrees: 420…460 black, 470 the band.)

**467 is the first visible frame, not the run's first tick.** `U2E.C:285-335` runs 33 fade-in waits and
16 more before `fadeset` writes the displayed page, so the band is drawn after **49** pre-visible waits:
the run anchor is **467 − 49 = 418** (417 if the fadeset's own wait counts before the write). The
earlier palette steps change entries the black does not use. D33 records the model and the two-retrace
cadence; the port's tick map is `u2e.md`'s.

---

## 19. ENDLOGO

Part 18 of the sequence and the first of `SECOND 5`'s block. `partsmask[5]` is `10000b`
(`MAIN/U2.ASM:116`), bit 4 alone (`&16=scrollers`), so that boot runs ENDLOGO, CRED and ENDSCRL from the
end sequence's own beginning. The shell prepares it before `exe2021`: `restartmus` for MUSIC0 at order
25, mode 13h, `inittwk` (`:898-909`).

`END/END.C` over `END/ASM.ASM` (`lineblit`), `readp.c` and `_PIC.OBK`. **320×400, 80-byte plane
rows**: `END.C:67-80` writes CRTC 0x0c, 0x0d = 0 and **CRTC 9 = `(old & ~0x80) & ~0x1f`** before
`lineblit(vram+y*80, rowbuf)` walks 400 rows (`:95-99`). `:51-63` points the DAC's write index at 0
and writes 63 765 times (`for(a=0;a<768-3;a++) outp(0x3c9,63);`): 765 bytes, entries 0–254, **entry
255 unwritten**. `:67-92` re-points the start address, takes the display to 400 lines, whitens again
and waits **32 vblanks**; `:94-99` reads and blits the picture; a **129-frame fade-in** from white
(`:101-106`); a music wait (`for(a=0;a<5000 && !dis_exit();a++) { dis_waitb();
if(dis_musplus()>-16) break; }`, `:107-111`); a **64-frame fadeout** (`:112-117`).

### 19.1 The sweeps

415–445 a **white field**, one colour (`85789170…`), over the loader's memory; 460 the picture faintly
through it (16 colours, mean 237.44); 500 the **fade-in** of the "FUTURE CREW" 3-D logo (56 colours,
mean 167.31); 540 nearly settled (66 colours); **580 … 1120 the settled picture** (two rounded blocks
lettered FUTURE and CREW on black, 69 colours, mean 33.23), byte-identical at every sample
(`f2514b29…`); 1160 the **fadeout** (65 colours, mean 25.15); 1200 its tail (30 colours, mean 4.29);
1240 refused (`frame 1240 is 320x200`, CRED). The change is in **(1200, 1240]**; 1220 is grab-hostile
(`reached frame 1221`), and so is the first graphics frame 404 (`reached frame 412`).

**On `SECOND 3`** `partsmask[3]` has bit 4 set too: a sweep of 20500…23000 finds 20500–21500 on the
320×200 screen and is refused at 22000 (`frame 22000 is 320x400`), so U2E's screen ends in **(21500,
22000]** and ENDLOGO follows.

### 19.2 The keyframes

| index | what is on screen | files | hashes |
|---|---|---|---|
| 430 | the **opening white** | `.rgb` + `.idx` | rgb `85789170…`, idx `eec19bc6…` |
| 500 | the **fade-in** over the held picture | `.rgb` only | rgb `fb8fbf2a…` |
| 900 | the **settled picture**, held | `.rgb` + `.idx` | rgb `f2514b29…`, idx `c0e0e259…` |
| 1180 | the **fadeout** | `.rgb` only | rgb `755f4747…` |

900 sits mid-hold because only the hold's start is deterministic: it ends when the music reaches
`dis_musplus() > -16` (`END.C:107-111`). 430's RGB is BEGLOGO's 430's bytes by another mechanism
(§1.11).

### 19.3 Held or moving, measured — and the `.idx` hand-check

The same bytes in three sessions at every index:

| keyframe | `K-1` | `K` | `K+1` | reading | per-frame change of 128,000 px |
|---|---|---|---|---|---|
| 430 | `85789170…` | `85789170…` | `85789170…` | **held** | 0 px |
| 500 | `8807e602…` | `fb8fbf2a…` | `bab01979…` | **moving** (palette fade) | 115,331 px (90.1023 %), max delta 4; then 41,568 px (32.4750 %), max delta 5 |
| 900 | `f2514b29…` | `f2514b29…` | `f2514b29…` | **held** | 0 px |
| 1180 | `da11d611…` | `755f4747…` | `67eeee93…` | **moving** (palette fade) | 32,911 px (25.7117 %), 42,751 px (33.3992 %), max delta 5 |

The moving keyframes move in the palette only (large in count, max delta 4–5), so they are RGB-only
(§1.3).

**ENDLOGO writes CRTC registers**, so both `.idx` files were read by hand against the manifest's
palette:

- `frame_000900.idx`: **72 indices, 69 RGB triples, no index carrying two, 0 mismatches of 128,000
  pixels**. A real check.
- `frame_000430.idx`: 1 index, 1 triple, 0 mismatches. A one-colour screen is the blind spot (§1.6):
  the weakest `.idx` in the tree.

**Only 900 is index-compared.** `compare` takes the indexed path only when both sides hold an `.idx`,
and no session holds one at 430 (they hold 429, 899, 900 and 901).

### 19.4 The re-capture and the census

Against three sessions holding all four triples: exit 0, `verdict: identical`, `mode: mixed` (900 by
indices), offsets `{0: 4}`.

The census is filtered by recipe (§1.11). It counts the three sessions and a fourth same-recipe
re-run; it excludes the ten wrong-boot window holders, and one same-recipe directory, a refused run
with no manifest whose 429–431 cannot be told from a `SECOND 2` boot's white.

| keyframe | sessions with a window | pictures at `N` | largest support | filed? | ≥ half? |
|---|---|---|---|---|---|
| 430 (held) | 4 | 1 | `85789170…` **4/4** | yes | yes |
| 500 | 4 | 1 | `fb8fbf2a…` **4/4** | yes | yes |
| 900 (held) | 4 | 1 | `f2514b29…` **4/4** | yes | yes |
| 1180 | 4 | 1 | `755f4747…` **4/4** | yes | yes |

Every session holds the filed picture at offset 0 and the same `N±1` neighbours: the timeline is
anchored to the boot index, music-gated steps included. The fade keyframes are palette-only, so a port
whose fade arithmetic (`((128-c)*63+palette[a]*c)/128`, `END.C:103`) differs shows a difference in
every pixel at a small max delta.

---

## 20. CRED

Part 19 and `SECOND 5`'s second part (`exe22`, `MAIN/U2.ASM:912-913`). The source is `CREDITS/MAIN.C`
over `CREDITS/TWEAK.ASM` (the `tw_*` library PAM and ALKU use) and 21 packed pictures. Each of 21
screens is `screenin(pic,text)` (`MAIN.C:161-213`): clear, set the picture's palette, lay the picture
and caption, **open an iris** over ~127 frames (`for(y=200*128;y>0;y=y*12L/13)`, `:176-192`), **hold
200 frames** (`:194`), **close the iris** over ~59 frames (`:196-212`). The iris is
`tw_setsplit(y/128+200)` plus `tw_setstart(160*200 + yy/4)` and a horizontal-pan write through the
attribute controller (`:182-191`); `tw_opengraph` sets mode 13h, chain-4 off, **CRTC 0x13 = `0x5013`**
("640 wide"), CRTC 9 = `0x0109`, CRTC 0x18 = `0x64`.

Its extent is **(1200, 1240] to (8840, 8940]** (1240 is 320×200; 8950 is ENDSCRL's 640×400), 21
screens in ~7700 frames: `MAIN.C:41-156` calls `screenin` 21 times. In the release's unpacked
`MAIN/DATA/CRED.EXE` `main` is 21 `screenin` calls (image `0x193a` to `0x1b56`, 27 bytes apart), and
their captions read out of the image are the ones below; its pictures are 21 distinct `FC FC` blobs, so
`pic5b`, `pic10b` and `pic14b` are pictures of their own (D49, `cred.md`).

Every 100th frame from 1240 to 8840 places each hold:

| screen | hold samples (byte-identical) | rgb prefix | what is on screen |
|---|---|---|---|
| 1 | 1340, 1440 | `f1ea2ca5` | a small "Code PSI / Trug / Wildfire" title card; "GRAPHICS – MARVEL / MUSIC – SKAVEN / CODE – WILDFIRE" |
| 2 | 1740, 1840 | `1f4945b0` | a spaceship over a planet; "+ OBJECTS – WILDFIRE, CODE – PSI" |
| 3 | 2040, 2140, 2240 | `5610a233` | a landscape under a sky band; "+ ANIMATION – TRUG" |
| 4 | 2440, 2540 | `7fc89a38` | the **SECOND REALITY** logo; "GRAPHICS – PIXEL" |
| 5 | 2840, 2940 | `1fca5b76` | a 3-D cube on a chequered floor; "+ MUSIC – PURPLE MOTION, CODE – PSI" |
| 6 | 3140, 3240, 3340 | `98de65f3` | a **dotted vortex**; "MUSIC – PURPLE MOTION / CODE – TRUG" |
| 7 | 3540, 3640 | `394d2207` | a pink **Moiré swirl**; "MUSIC – PURPLE MOTION / CODE – PSI" |
| 8 | 3940, 4040 | `84030beb` | pink/magenta **diagonal stripes**; same text |
| 9 | 4240, 4340, 4440 | `999b10a8` | a cartoon figure; "GRAPHICS – PIXEL / MUSIC – PURPLE MOTION" |
| 10 | 4640, 4740 | `250d836a` | a blue landscape with a plant; "+ CODE – TRUG, RENDERING – TRUG" |
| 11 | 5040, 5140 | `1ff4791c` | a bald head; "SKETCH – SKAVEN / GRAPHICS – PIXEL / …" |
| 12 | 5340, 5440, 5540 | `cc2df71e` | a **tiled texture of objects and faces**; same text as 11 |
| 13 | 5740, 5840 | `101a096f` | a purple/pink haze; "MUSIC – PURPLE MOTION / CODE – WILDFIRE" |
| 14 | 6140, 6240 | `77629d84` | a 3-D prism with a rainbow edge; same text |
| 15 | 6440, 6540, 6640 | `31013ee1` | a ring of dots; "MUSIC – PURPLE MOTION / CODE – PSI" |
| 16 | 6840, 6940 | `98db193a` | a blue sphere with orange comets; "+ GRAPHICS – PIXEL, RENDERING – TRUG" |
| 17 | 7240, 7340 | `b558ccaf` | a blue-and-red abstract; "MUSIC – PURPLE MOTION / CODE – PSI" |
| 18 | 7540, 7640, 7740 | `92155573` | a drawn figure on white; "GRAPHICS – MARVEL / MUSIC – PURPLE MOTION / CODE – PSI" |
| 19 | 7940, 8040 | `0261eba9` | a green creature on a white wall; "MUSIC – SKAVEN / CODE – PSI / WORLD – TRUG" |
| 20 | 8340, 8440 | `a7cef452` | the **FUTURE CREW** ring logo; "GRAPHICS – PIXEL / MUSIC – SKAVEN" |
| 21 | 8640, 8740, 8840 | `cd5f27a5` | a small "Second Reality" card; "GRAPHICS – PIXEL / MUSIC – SKAVEN / CODE – WILDFIRE" |

### 20.1 The keyframes

| index | what is on screen | rgb |
|---|---|---|
| 1440 | screen **1**'s hold, the title card | `f1ea2ca5…` |
| 3240 | screen **6**'s hold, the dotted vortex | `98de65f3…` |
| 5440 | screen **12**'s hold, the tiled texture | `cc2df71e…` |
| 7640 | screen **18**'s hold, the drawn figure | `92155573…` |
| 8740 | screen **21**'s hold, the closing card | `cd5f27a5…` |

All `.rgb` only (§20.2). The holds are the only still moments; the iris moves the picture.

### 20.2 Held or moving, measured — and why no `.idx`

Every keyframe is **held** across sessions: in each of three sessions `K-1 = K = K+1`, the same bytes
in all three. The sweep agrees (1340 = 1440, 3140 = 3240 = 3340, 5340 = 5440 = 5540, 7540 = 7640 =
7740, 8640 = 8740 = 8840).

**All 82 CRED frames came back RGB-only, refused by the coherence check** (the tool's "does not
reproduce the RGB frame (the screen moved while it was read)" wording), and the artefacts rule out the
other causes:

- **Not a stride or mode refusal.** The plan accepted the screen (`scan_len` 0x50 → 160 bytes a plane
  row, 32,000 bytes, under a plane) and the mode is 256-colour.
- **Not lateness on a moving screen.** Three consecutive frames are byte-identical at every keyframe,
  and a hold is 200 frames (`for(a=0;a<200 && !dis_exit();a++) dis_waitb();`, `MAIN.C:194`) against a
  ~20-frame read.
- **Not the palette.** Each screen's palette is written once (`tw_setpalette`, `MAIN.C:169`); the holds
  write nothing.

What is left is the read's model of **where the screen is**. `capture_indexed` reads each plane from
offset 0, and CRED's screens start at `tw_setstart(160*200 + yy/4)`. The alternative not excluded: the
`scan_len * 2` rule gives 160 for a screen whose guest pen steps its rows by 80 bytes
(`tw_putpixel`'s `shl ax,4 / add bx,ax / shl ax,2 / add bx,ax` = `y*80`). The log cannot separate them.
The two `.idx` from U2E's opening hold (a part that writes CRTC 0x0c, §18.6) are two-colour flat screens
and carry no weight either way. The comparison is RGB, where the widening is load-bearing.

### 20.3 The re-capture and the census

Against three sessions holding `K-1, K, K+1` around all five: exit 0, `verdict: identical`,
`mode: rgb`, offsets `{0: 5}`. Census (filtered by recipe, §1.11) over four sessions: **4/4 at every
keyframe**, each window holding the filed picture at `K-1`, `K` and `K+1`. The keyframes say nothing
about the ~186 frames a screen spends opening and closing its iris.

---

## 21. ENDSCRL

Part 20, the last of the sequence and `SECOND 5`'s third (`exe23`, `MAIN/U2.ASM:914-917`). The source
is `ENDSCRL/MAIN.C` over `ENDSCRL/FONA.LBM` and `ENDSCROL.TXT`. **It is skipped when looping is on**
(`cmp cs:looping,0 / jne @@skipp6`, `:914-915`), and this capture is the non-looping case (§1.11).

`MAIN.C:22-26` sets **mode 0eh (640×350, 16 colours)**, writes **CRTC 9 = 0** (the 400-line display)
and a 16-entry grey ramp. `do_scroll` (`:60-96`) renders one 80-byte line of the font strip into the
four bit-planes at `80*yscrl` and again at `80*(yscrl+401)`, then `setstart(yscrl*80+80)`,
`yscrl=(yscrl+1)%401` (`:93-95`): a 401-line ring, one line per iteration, two vblanks an iteration
(`if(dis_waitb()==1) dis_waitb()`, `:47`). The text is centred on x = 320 (`tstart=(639-tstart)/2`,
`:73`).

Its extent starts at **(8840, 8940]** and is unbounded on this capture's terms: the loop is
`while(!dis_exit())` (`:45-49`), and captured to 20000 it is still scrolling. (The release's build ends
at the `%` line of its text; that is `endscrl.md`'s, D49.)

### 21.1 The sweep

From 8950 (4 colours, mean 0.81) the greetings text climbs: "Second Reality / Copyright (C) 1993 by
the Future Crew" enters at 9100; the Assembly'93 paragraph ("This demo won the Assembly'93 annual
international demo competition held in Finland, summer 1993.") and the sponsors (Advanced Gravis, Epic
MegaGames, Waite Group Press, Terton, HiCompu, Toptronics, Pro Component, Lan Vision, Data Fellows) at
9700–11000; "Future Crew greets the following groups" (Sonic PC, Access Denied, Silents PC, Avalanche,
Xography, Witan, Triton, Epical) at 12000; the personal greetings (Daryll, Tran & White Shadow,
Dr.Weird, Martti Palonen, Jussi Markula, Mikko Vierula, Jussi Leppänen, Wildfire …) at 13000–14000;
still scrolling at 16000–20000.

### 21.2 The keyframes

| index | what is on screen | rgb |
|---|---|---|
| 9100 | "**Second Reality / Copyright (C) 1993 by the Future Crew**" entering (content rows 309–376 of 400) | `b6beb9f0…` |
| 10000 | the **Assembly'93 competition** paragraph and the sponsors opening | `26d55ae1…` |
| 12000 | the **greetings groups** block, "read the FCINFO10.TXT file first. …" above it (rows 4–396, x 31–605) | `c308d1b2…` |
| 14000 | the **personal greetings** block (rows 0–392, x 123–513) | `fb2d67a3…` |

All `.rgb` only (§21.5); the part has no holds, so these are chosen for content.

### 21.3 Held or moving, measured — and the window that could not be taken

Every keyframe is **moving**. At the three-frame gaps the sessions use (§1.11):

| keyframe | `K-3` | `K` | `K+3` | `K-3`→`K` | `K`→`K+3` |
|---|---|---|---|---|---|
| 9100 | `06526561…` | `b6beb9f0…` | `f73dbb06…` | 3074 px (1.2008 %), 38 rows | 3901 px (1.5238 %), 40 rows |
| 10000 | `6ed5877e…` | `26d55ae1…` | `4aad4318…` | 14,711 px (5.7465 %), 183 rows | 18,866 px (7.3695 %), 199 rows |
| 12000 | `22de4c9e…` | `c308d1b2…` | `828bc0d3…` | 11,546 px (4.5102 %), 197 rows | 14,807 px (5.7840 %), 212 rows |
| 14000 | `4661302d…` | `fb2d67a3…` | `db758053…` | 8661 px (3.3832 %), 156 rows | 11,078 px (4.3273 %), 167 rows |

The differences are confined to the rows the text occupies (at 9100 only rows 309–377). The ±1 window
cannot be taken on this screen (`--frames 9099,9100,9101` and `9099,9100` refused in every session;
`9098,9100` captures once but not in a three-index batch; `9097,9100,9103` always captures), for the
reason §1.11 gives.

### 21.4 The re-capture and the census

Three sessions holding `9097,9100,9103,9997,10000,10003,11997,12000,12003,13997,14000,14003`: exit 0,
`verdict: identical`, `mode: rgb`, offsets `{0: 4}`.

No session holds this part's `N-1, N, N+1`, so the census is an **offset-0 count** over every
same-recipe capture that took the index (the three sessions, the probes and diagnostic run that
established the three-frame gap, the discovery sweep and two re-runs):

| keyframe | sessions holding `N` | byte-identical to the filed picture |
|---|---|---|
| 9100 | 9 | **9/9** |
| 10000 | 5 | **5/5** |
| 12000 | 5 | **5/5** |
| 14000 | 5 | **5/5** |

The recipe filter matters here too: `SECOND 3` captures (RAYSCRL's sessions and reference) hold index
14000, and the hidden part's sweep holds 10000–14000. What the offset-0 count does and does not prove is
§1.12's.

### 21.5 Why there is no `.idx`

All 14 sweep frames and the four keyframes carry the loud refusal *"the screen is not one palette index
a byte (not 256-colour, or a depth the emulator did not name)"* (`UnmodelledMode`): mode 0eh packs its
pixels as bits (`MAIN.C:22-26`), so the read never happens. The nonzero display start
(`setstart(yscrl*80+80)`, `:95`) that may refuse CRED (§20.2) never arises.

---

## 22. DDSTARS — the hidden part

Slot 17, outside the main sequence: only `SECOND U` reaches it, setting `whattorun = 0x80`
(`partsmask[8]` = `80h`, `MAIN/U2.ASM:119`), and the runner runs `exehid` alone after a `restartmus` of
MUSIC0 at order 70 (`:890-896`). No numbered boot sets bit 7 and the end-of-pass reset writes `07fh`
(`:919`) (`part-sequence.md` §5). The source is `DDSTARS/KOE.ASM` over `STARS.ASM` and `POLYEGA.ASM`;
`DOIT1.C`–`DOIT3.C` generate the EGA art and are not part of the effect.

`init_stars` (`STARS.ASM:56-178`) sets mode 13h, clears the attribute controller's palette to 0..15,
**clears CRTC 9's scan-doubling bit** (`:72-78`) and loads the star tables. `do_stars` (`:540-778`) is
an endless loop of `waitb`, a ~32-frame palette fade-in (`starpalfade`, `:559-648`), `risetext`
(`:413-540`) and the stars, with text opened when `starframe` passes 1200 and 3200 (`:653-662`) and a
page flip through CRTC 0x0c (`:544-553`). **320×400** in mode 13h's 80-byte rows, named 16-colour by the
emulator (§1.11, §22.5).

Its extent starts in the boot's first frames (415, 450 and 480 already up: 4 colours, mean 0.00 →
0.08 → 0.11, the fade's first steps) and never ends on its own: the loop exits only when DIS function 2
returns non-zero (`mov bx,2 / int 0fch / or ax,ax / jz @@aga`, `:774-778`).

### 22.1 The sweeps

500 nearly black (4 colours, mean 0.13); a sparse starfield brightening at 700–1500 (mean 0.52 →
2.47); the first line of text entering at the bottom at 1700–1900 ("A hidden part? No, just an
experiment …"); the block written in the upper middle at 2100–2400; climbing at 2700–3000 with the
part's "Future Crew 1993" artwork faintly behind the stars; scrolled off at 3300–3600; a second block
("And then we have …") at 4000–5000; the field densifying with the text faded at 6000–8000 (mean
44.68 → 57.24); still running at 10000, 12000, 14000, 16000 and 20000 (mean 67.37, 75.15, 81.17, 85.94,
92.67), a different `sha256` at each.

### 22.2 The keyframes

| index | what is on screen | rgb |
|---|---|---|
| 1900 | the **first text entering** the field | `c9228e9f…` |
| 3000 | the text block **fully open** | `b0693ed8…` |
| 5000 | a **second text block** over a denser field | `d948981e…` |
| 8000 | the **late field**, text faded | `ecc2dadd…` |

All `.rgb` only. The part has no held frames.

### 22.3 Held or moving, measured

The same in every session:

| keyframe | `K-1` | `K` | `K+1` | reading |
|---|---|---|---|---|
| 1900 | `de6d1676…` | `c9228e9f…` | `a1697853…` | **moving** |
| 3000 | `f768ffbb…` | `b0693ed8…` | `5eb100a6…` | **moving** |
| 5000 | `062d91c4…` | `d948981e…` | `0af0a960…` | **moving** |
| 8000 | `fb4b902c…` | `ecc2dadd…` | `328201f6…` | **moving** |

### 22.4 The re-capture and the census

Against three sessions holding the four triples: exit 0, `verdict: identical`, `mode: rgb`, offsets
`{0: 4}`. A fourth session was refused at 5001 and counts for 1900 and 3000 only (§1.8 rule 1). Census
per keyframe, filtered by recipe:

| keyframe | sessions with a window | pictures at `N` | largest support |
|---|---|---|---|
| 1900 | 6 | 1 | `c9228e9f…` **6/6** |
| 3000 | 5 | 1 | `b0693ed8…` **5/5** |
| 5000 | 4 | 1 | `d948981e…` **4/4** |
| 8000 | 5 | 1 | `ecc2dadd…` **5/5** |

Every session holds the filed picture at `N` and the same `N±1` neighbours: deterministic at its boot
index, moving screen and all.

### 22.5 Why there is no `.idx`

All 20 sweep frames carry the loud `UnmodelledMode` refusal (*"the screen is not one palette index a
byte"*), as TECHNO's interference screen does. The emulator decides it at `vga.gfx.mode & 0x40`
(`hardware/video/vga.cpp:74`), and the part writes the graphics controller (the bit-mask register,
`mov ax,0ff08h`, `POLYEGA.ASM:322`; sequencer plane masks throughout `STARS.ASM`). Which write clears
that bit is not established: a grep of `KOE.ASM`/`STARS.ASM`/`POLYEGA.ASM` finds no write to the
graphics controller's mode register (index 5).

---

## 23. Retired: anchored and drifting parts

Folded into §1.11's last paragraph.

---

## 24. The vault captures

The three unreleased effects are separate programs: each links the DIS client and needs the vendored
standalone `DIS/DIS.EXE` resident, and none draws `SECOND.EXE`'s setup screen. The capture waits for
the first graphics frame instead (`--boot-wait graphics`, `reference.rs`'s `BootWait`), and
`tools/vault_capture.sh` stages and captures one effect (D39).

### 24.1 The recipe

```
tools/vault_capture.sh <grid|hard|twist> --frames <A,B,...> [--out <dir>]
```

stages `upstream/SecondReality/DIS/DIS.EXE`
(`270f5fe2c7c7d17b4720c2019c2832f4bd3863f97e750315974692911f0ead08`) and the effect EXE (`GRID/P.EXE`
`d4e7f1d6…`, `HARD/B.EXE` `1f8a7bda…`, `TWIST/T.EXE` `88d5e695…`) into a fresh stage after checking all
four digests, writes `RUN.BAT` (`@ECHO OFF`, `DIS.EXE`, the effect), and runs

```
cargo xtask reference capture --exe <stage>/RUN.BAT --cwd <stage> \
    --boot autoexec --boot-wait graphics --cycles 30000 --out <dir> [--keys KBD_space]
```

The default `--out` is `reference/parts/<part>`. `--keys KBD_space` is GRID's and HARD's starter key:
the recipe writes `AUTOTYPE -w 5 space` before the program, so one key reaches DIS's `checkkeys`
(`DIS/DISINT.ASM:112-157`) after the effect has started: GRID's `getch()` (`GRID/MAIN.C:153`) and
HARD's `while (dis_muscode(1) != 1 …)` (`HARD/B.C:138`). TWIST has no key gate (`TWIST/MAIN.C:49`).
Every filed manifest says `exe: "RUN.BAT"`, `arguments: []`, `boot: "autoexec"`, `boot_wait:
"graphics"`, `cycles: 30000`, `frame_origin: "boot"`, 320×200, and `keys: ["KBD_space"]` (GRID, HARD)
or `[]` (TWIST).

### 24.2 GRID's start, by route

A dense sweep (`--frames "$(seq -s, 340 365)"`): 340–351 are the static pre-key dot screen
`728e0d87…`, **352** is `d2ac61b7…`, **353** is `4a4ad86e…`. An alternative route that `PAUSE`s for a
second key before `P.EXE` has its last static frame at 702, 703 `e1ac8916…`, 704 `4a4ad86e…`: 351 frames
later (a second key at 10 s against the command line's `AUTOTYPE` at 5 s). 352 and 703 differ while 353
and 704 are byte-identical, so **the first changed frame is a transition frame** and the shift is exact
from the first stable loop frame, where `K0` goes.

### 24.3 TWIST starts as soon as DIS and T.EXE load

TWIST is animating by frame 20: single-index probes at **20** (`03b276c7…`), **40** (`edfb150c…`) and
**80** (`063223de…`) all differ. `--frames 355,400,600` is refused reproducibly (`the emulator reached
frame 427 before frame 400 could be captured`, and 428): one indexed read of TWIST's planar screen costs
far more than the 25-frame estimate, so its indices are spaced by about 100 and its dense sweeps taken
as one batch. GRID's 100-frame spacing works because GRID is chain-4 mode 13h, the cheap read (D14).

### 24.4 The three filed captures

| directory | staged batch | key | `--frames` | frames | indexed |
|---|---|---|---|---|---|
| `reference/parts/grid` | `DIS.EXE`, `P.EXE` | `KBD_space` | `400,500,600,700,800` | 5 | none |
| `reference/parts/twist` | `DIS.EXE`, `T.EXE` | none | `103,203,303,403,503,603` | 6 | none |
| `reference/parts/hard` | `DIS.EXE`, `B.EXE` | `KBD_space` | `480,560,630,700,770,840,910,1050,1600` | 9 | `840` |

All three manifests also carry `version: 1`, `oracle: "dosbox-automation 0.85.1 (6d100)"`,
`upstream_commit: "071a82e"` and `skip_boot_frames: 0`; HARD's `840` carries `frame_000840.idx`,
`idx_sha256` and the `palette` map.

```bash
tools/vault_capture.sh grid  --frames 400,500,600,700,800
tools/vault_capture.sh twist --frames 103,203,303,403,503,603
tools/vault_capture.sh hard  --frames 480,560,630,700,770,840,910,1050,1600
```

### 24.5 GRID — `K0` = 353 ±1, five keyframes

A re-run of the dense sweep reproduces §24.2 byte for byte. **`K0` = 353**: the port's frame 0 is its
first composed loop frame (`GRID/MAIN.C:161-171`), and 352 is the one-off start-up transition as the
key is consumed. Tolerance ±1 (D17/D18). Effect frame `j = index − 353`; the palette plane advances one
step per effect frame in a 3-frame cycle (`grid.md`), so five keyframes 100 apart cover all three
planes.

| index | effect `j` | what is on screen | rgb |
|---|---|---|---|
| 400 | 47 | line-work over the eye | `574a6fa9…` |
| 500 | 147 | line-work over the eye, other phase | `0241f663…` |
| 600 | 247 | line-work over the eye | `1fb43740…` |
| 700 | 347 | line-work over the eye | `1969cf9a…` |
| 800 | 447 | line-work over the eye | `869aeee7…` |

All RGB-only (the screen moves during the read). 400/500/600 are the same bytes in another session,
and 600 in a third.

### 24.6 TWIST — `K0` = 3 ±1, six keyframes

**The boot anchor returns at frame 5.** `--frames 0` is refused (`the emulator reached frame 5 before
frame 0 could be captured`), and six probes of 4 were refused the same way (five naming 5, one 6).

**The phase at 5 and 6, measured in two sessions** (`--frames "$(seq -s, 5 30)"`). TWIST's frame 0 is
the two-band fingerprint (rows 0..63 generated row 0, rows 64..199 generated row 199), and for early
frame `f` the first row that differs from row 0 is `ceil(64/(f+1))` (`twist.md`):

| boot index | boundary row | effect frame | rgb (both sessions) |
|---|---|---|---|
| 5 | 22 = `ceil(64/3)` | **2** | `e385838c…` |
| 6 | 16 = `ceil(64/4)` | **3** | `d5ce27ce…` |

So frame 0 sits at boot index **3**: `K0` = 3 ±1. One probe under host load captured boot 4: boundary
**32**, effect frame **1** (`595bdf32…`), matching the phase at three consecutive indices. Boot 3
stayed out of reach in twelve loaded probes. The ±1 is measured: one session displayed effect frame 3
twice (boot 6 = 7), and a six-frame trial session was one effect frame off at every keyframe (§24.8).
The 4,444-frame cap ends the effect at boot `3 + 4444 = 4447`.

| index | effect frame | rgb |
|---|---|---|
| 103 | 100 | `6615ba39…` |
| 203 | 200 | `bef51119…` |
| 303 | 300 | `55f22210…` |
| 403 | 400 | `42aa02f6…` |
| 503 | 500 | `e9ab57c9…` |
| 603 | 600 | `7b288e5a…` |

All RGB-only, and TWIST has no `.idx` even in principle: even frames display buffer 1 at pixel origin 0,
odd frames buffer 2 at 131,072 (`TWIST/MAIN.C:60-76`), and the read starts at plane offset 0.

### 24.7 HARD — `K0` = 356 ±1, nine keyframes and the mode starts

HARD's gate is GRID's key (`HARD/B.C:138`), then four `waitb` (`:139-142`), the pre-roll wipe (one
row-100 pixel cleared per x with a `waitb` every 16 columns, `:143-147`), the accelerating fill from
row 100 down (`:148-161`) and 70 waits (`:162`). In two sessions (`--frames "$(seq -s, 340 380)"` and
`"$(seq -s, 350 362)"`): **340–355 are the white screen** `20f3d62f…`; **356 is the first changed
frame** (`e7d2f716…`, **16 black pixels at row 100, x=304..319**); 357 has 32, 358 48, … 375 the whole
row; 376–378 repeat 375; 379 has rows 100–101 black and 380 rows 100–102. Every step is byte-identical
across the sessions.

**`K0` = 356**: the port's frame 0 is the first wipe frame, the unique 16-pixel state. ±1: 355 is white
and 357 the 32-pixel step.

**The mode starts** (the counts are `B.C:203-324`'s and `BP.C`'s, the indices this oracle's):

| boot index | what changes | rgb |
|---|---|---|
| 474 | first letterbox frame (473 is the half-black filled screen) | `1930616d…` |
| 474–511 | letterbox, no dots (38 frames: mode 0's 34 iterations, `part1init`'s two `waitb` frames and part 1's first) | `1930616d…` |
| 512 | first spawned dots (mode 1, logo phase 1) | — |
| ~512–585 | phase 1, `dots1`: **FUTURE** traced in | 560 `318503a6…` |
| ~587–660 | phase 2, `dots2`: **CREW** over FUTURE | 630 `788e0a7e…` |
| ~662–735 | phase 3, `dots3`: **RULES** over CREW | 700 `9574495d…` |
| ~737–809 | phase 4, `dots4`: **FOREVER** over RULES | 770 `cbdf6e0c…` |
| 810–851 | mode 3: dots dead, FOREVER held, byte-identical | `f2718de8…` |
| 852 | first mode-4 band frame | `963a0272…` |
| 852–955 | mode 4 band; 930–955 hold the last band state | `9cced0af…` |
| 955 | `part2init` (mode 5's count 0, pixel-identical to the hold) | `9cced0af…` |
| 956 | first visible part-2 grid frame (count 1; the 100 trail pixels become entry 65) | `0495730e…` |
| 958 | part 2's count 3, the first frame after the state advances | — |
| 1455 | last part-2 frame (fading grid) | `996c4880…` |
| 1456 | part 2's fade leaves the empty band (474's bytes) | `1930616d…` |
| 1458 | first part-3 cube frame | `e31eb951…` |

Each row comes from one of these batches (contiguous or five apart, so no read sits between two
grabs):

```bash
tools/vault_capture.sh hard --frames "$(seq -s, 470 520)"     --out <scratch>
tools/vault_capture.sh hard --frames "$(seq -s, 500 10 830)"  --out <scratch>
tools/vault_capture.sh hard --frames 840,845,850,855,860,865,870,875,880 --out <scratch>
tools/vault_capture.sh hard --frames "$(seq -s, 846 860)"     --out <scratch>
tools/vault_capture.sh hard --frames "$(seq -s, 930 5 1000)"  --out <scratch>
tools/vault_capture.sh hard --frames "$(seq -s, 948 962)"     --out <scratch>
```

**The mode-5 boundary is two displays late.** The source's mode 4 is 102 iterations, so with count 0 at
851 part 2's count 0 would be 953; the capture's `part2init` is **955** (950–955 byte-identical, 956 the
first visible grid frame). The extra displays are a missed-retrace overrun in the mode-4 loop (D22);
the port's handling is A48. The one key never reaches the `0xff` cut (`BP.C:263` needs a second), so
part 3 runs to its own `count>=800` end, and every filed index is inside the 320×200 window.

| index | mode / phase | what is on screen | rgb |
|---|---|---|---|
| 480 | 0 | letterbox, empty band | `1930616d…` |
| 560 | 1 / phase 1 | `dots1` tracing FUTURE | `318503a6…` |
| 630 | 1 / phase 2 | `dots2` tracing CREW over FUTURE | `788e0a7e…` |
| 700 | 1 / phase 3 | `dots3` tracing RULES over CREW | `9574495d…` |
| 770 | 1 / phase 4 | `dots4` tracing FOREVER over RULES | `cbdf6e0c…` |
| 840 | 3 | FOREVER held, dots dead | `f2718de8…` |
| 910 | 4 | the grey/blue band | `59f2d5fc…` |
| 1050 | 5 | part 2's 3-D dot grid | `c1743f6a…` |
| 1600 | 6 | part 3's glenz cube | `5894039b…` |

840 and 480 are the held keyframes. 840's indexed read reproduced its RGB. The filed 480 is RGB-only:
in the filed run its read landed after mode 0 had ended (a trial session caught it still held), so an
indexed comparison at 480 is optional. The seven moving frames are RGB-only.

### 24.8 The repeats, and one measured ±1

| part | repeated index | result in a second session |
|---|---|---|
| GRID | 600 | **byte-identical** (`1fb43740…`) |
| TWIST | 603 | **byte-identical** (`7b288e5a…`); 103 identical in two more sessions |
| HARD | 480 | **byte-identical** (`1930616d…`) |
| HARD | 1600 | **byte-identical** (`5894039b…`) |

A separate HARD trial session is byte-identical at all nine indices. One TWIST trial session differs
from the filed capture at every keyframe by the same one effect frame (`6a898f21…` at 103 against
`6615ba39…`; `3d33fdcb…` at 603 against `7b288e5a…`); the filed bytes are the majority picture, and the
±1 search answers such a session.

`--frames "$(seq -s, 475 25 2200)"` on HARD is refused after the first frame (`reached frame 538 before
frame 500`): 25-frame spacing is inside one read's cost. 100-frame spacing and contiguous batches work.

---

## 25. The loop seam and the lead-ins

Eight captures in three directories:

| directory | captures | why more than one |
|---|---|---|
| `reference/parts/loop` | `cred` (320×200), `alku` (320×400) | CRED's end and ALKU's second pass are two geometries (D15); a survey across them is refused at the first 320×400 frame (§25.2) |
| `reference/parts/beglogo-leadin` | `even`, `odd`, `plane-436` | adjacent 320×400 indices slip, so the window is two runs; a batched run carries no `.idx`, so the index plane is a third, unbatched capture (§25.6) |
| `reference/parts/endlogo-leadin` | `even`, `odd`, `plane-440` | the same |

`$(seq …)` below is expanded by the shell.

### 25.1 The loop recipe, and what it does to the boot's numbering

**The keys**, read against `MAIN/MENU.C`: the defaults are `m_soundcard=3` (Gravis) and
`m_looping=0` (`:28`, `:30`); the cursor is clamped to row 1, the soundcard row (`:158`); extended
codes `1000+scan` move the row on 1072/1080 and the entry on 1075/1077 (`:94-108`); rows 1, 2, 3 are
soundcard, quality and looping (`:190`, `:229`), each wrapping (`:163-168`). So `KBD_left KBD_down
KBD_down KBD_right KBD_enter` is soundcard 3→2 (SoundBlaster Pro), row 1→3, looping 0→1, continue.
`START.C:103` hands `m_looping` to the shell (`ip[3]`) and `U2.ASM:746` stores it in `looping`. Every
loop manifest says `arguments: ["5"]`, these five keys, `boot: "autoexec"`, `cycles: 30000`.

**Three more keys move the whole run 115 frames later.** `AUTOTYPE` paces its presses, so every index
in `reference/parts/loop` is in this recipe's boot space, not the two-key recipe's (§1.12). ENDLOGO's
fade-in is frame-locked (`END.C:101-106`), and two-key 498–502 (`b4415a61…`, `8807e602…`,
`fb8fbf2a…` = `reference/parts/endlogo`'s 500, `bab01979…`, `e6b4b1f1…`) are byte-identical to five-key
613–617:

```bash
cargo xtask reference capture --arg 5 --boot autoexec --cycles 30000 \
    --keys "KBD_left KBD_enter" --frames 498,499,500,501,502 --out <scratch>
cargo xtask reference capture --arg 5 --boot autoexec --cycles 30000 \
    --keys "KBD_left KBD_down KBD_down KBD_right KBD_enter" --frames "$(seq -s, 590 2 650)" --out <scratch>
cargo xtask reference capture --arg 5 --boot autoexec --cycles 30000 \
    --keys "KBD_left KBD_down KBD_down KBD_right KBD_enter" --frames 611,613,615,617,619 --out <scratch>
```

The music agrees: `cargo xtask music trace --arg 5 --cycles 30000 --keys "KBD_left KBD_down KBD_down
KBD_right KBD_enter" --frames 380..700` puts MUSIC0's rows (25,1), (25,2), … at 543, 551, 560, …,
where `reference/music/second5` has them at 427, 436, 445, …: nineteen row changes, offsets 115 or 116.
A port `--block 5` run anchored at 404 numbers its frames 115 lower than this capture.

### 25.2 The survey

`--frames "$(seq -s, 8700 50 10700)"` on the loop recipe is refused at 9150 (`frame 9150 is 320x400`),
so it is taken as two screens (`8700 50 9100` and `9150 50 10700`). **No 640×400 frame appears**:
ENDSCRL is skipped (`U2.ASM:914-915`). 8700 is CRED's screen 21 irising open; 8750–8950 its hold
(`cd5f27a5…`, §20's last keyframe's bytes, 115 frames later); 9000 the iris closing; 9050 and 9100 the
closed, all-black screen (`ea0787f6…`). Then 9150–10200 ALKU's black (`fec9afb5…`); 10250 "A / Future
Crew / Production" fading in; 10300–10600 its hold (`6a27ec50…`, `alku`'s 1719); 10650 its fade-out;
10700 black.

### 25.3 `C`, the fade, `L` and Δ

**`C` = 9029**, the first frame of CRED's closed screen. One batch of 8951–9050 (320×200, exit 0):
8951–8975 are screen 21's hold; 8976–9028 are 53 distinct frames of the closing iris
(`CREDITS/MAIN.C:196-212`; 8976 `7f3f38a0…`, 9028 `88532193…`); **9029–9050 are one picture,
`ea0787f6…`**, all black. It holds to 9101 and is the picture at 9103.

**The fade is 64 frames, 9029–9092.** After CRED returns the shell sets `whattorun` to `7fh`, checks
Ctrl and `looping` (`U2.ASM:919-925`) and calls `fademusic` with `cx=1` (`:927-928`), lowering the
module's global-volume byte `ds:[30h]` by one per `waitb_1` (`DIS/DISINT.ASM:161-181`) until it is 0
(`U2.ASM:333-346`). That byte is **64** in both shipped modules (`assets/MAIN/MUSIC0.S3M`,
`MUSIC1.S3M`), so the fade is 64 blanks (±1 for where CRED's last blank falls). A music trace of
8960..9200 on the loop recipe: order 43 plays through the fade (row 42 at 9088); `zframe` stops at 5870
from **9092** to 9095 (`_zstopsong`, `:354`); (43,43) is held from 9097 to 9111 while `restartmus`
reloads (`:347-396`); **the restarted song is at (0,1) at 9112** (`_zplaysong(0)`, `:394`, from
`:929-932`). The trace's one missing frame is 9102.

**`L` = 9104**, ALKU's first frame on pass 2, the first 320×400 frame. `--frames 9102` is refused in two
sessions (`reached frame 9103 before frame 9102`): **9102 is never published**, the counter advancing
over ALKU's `tw_opengraph` mode reset (`ALKU/MAIN.C:158`; D17). 9103 is 320×200 `ea0787f6…`; 9104 is
320×400 `fec9afb5…`. So **Δ = L − 404 = 8700**. (Under `SECOND 1` the capture cannot reach 404 to
re-measure it; 404 is D17's BIOS-mode-byte measurement. If it is the mode set rather than the first
320×400 frame, its pass-2 counterpart is 9102 and Δ would be 8698; the held keyframes cannot tell.)

**ALKU's content on pass 2 sits at pass-1 + 8692–8694.** ALKU's first screen waits on `dis_sync()<1`
(`ALKU/MAIN.C:59`), a lookup of the player's (order, row) in `ordersync1` (`DIS/DISINT.ASM:427-463`),
and pass 2's music starts 8 frames earlier relative to ALKU: the seam's (0,1) at 9112 is 8692 after
`reference/music/second1`'s at 420, and the next nine rows 8693–8697 after theirs. Sampled every second
frame (`--arg 1 … "$(seq -s, 1500 2 1620)"` and the loop recipe's `"$(seq -s, 10200 2 10320)"`), pass
1's first non-black frame is 1544 and its hold starts at 1606; pass 2's are **10238** and **10298**.

**One session in seven passed ALKU's first wait at once**: its 9112 is already the fade-in
(`c8feffd2…`, the bytes of the others' 10238). (43,43) maps to sync **10** (between `ordersync1`'s
`0d00h,10` and `3d00h,1`), so an ALKU whose first `dis_sync()` runs before the restarted player's first
tick leaves the loop at once. It is a race between ALKU's `init()` (`ALKU/MAIN.C:154-…`) and the player,
the host-timing term of D17. **The filed capture is the majority group** (six of seven), in which ALKU
waits for order 2.

### 25.4 The filed loop seam

```bash
cargo xtask reference capture --arg 5 --boot autoexec --cycles 30000 \
    --keys "KBD_left KBD_down KBD_down KBD_right KBD_enter" \
    --frames 9029,9055,9081 --out reference/parts/loop/cred
cargo xtask reference capture --arg 5 --boot autoexec --cycles 30000 \
    --keys "KBD_left KBD_down KBD_down KBD_right KBD_enter" \
    --frames 9104,9600,10419,11419 --out reference/parts/loop/alku
```

No `.idx` (CRED's black screen fails the coherence check; ALKU's 176-byte rows wrap a plane, §1.6).

| capture | index | what it is | rgb |
|---|---|---|---|
| `loop/cred` | 9029 | **`C`**: CRED's closed screen, all black | `ea0787f6…` |
| `loop/cred` | 9055 | inside the fade | `ea0787f6…` |
| `loop/cred` | 9081 | inside the fade | `ea0787f6…` |
| `loop/alku` | 9104 | **`L`**: ALKU's first frame, all black | `fec9afb5…` |
| `loop/alku` | 9600 | 900 + Δ, ALKU's black wait | `fec9afb5…` = `alku`'s 900 |
| `loop/alku` | 10419 | 1719 + Δ, "A Future Crew Production" | `6a27ec50…` = `alku`'s 1719 |
| `loop/alku` | 11419 | 2719 + Δ | `5e3c3884…` = `alku`'s 2719 |

The held keyframes reproduce pass 1 byte for byte at + Δ. `C` holds only to its right (9028 is the
iris's last step).

### 25.5 The repeat

A second session of both commands gives all seven `sha256`s equal to the filed capture's; a third (a
trial `alku` run) is equal at its four indices.

### 25.6 The lead-in windows

Dense 320×400 batches slip: `"$(seq -s, 430 450)"` on `--arg 2` is refused (`reached frame 438 before
frame 437`), `"$(seq -s, 436 455)"` on `--arg 5` too (`… 440 before 439`), and so are their halves after
the split. The refused frames capture alone. A run of adjacent 320×400 grabs slips after four to nine
frames on this host (437, 439, 445, 446, 598, 647, 1507, 9109 and 10236 were each missed once), while a
hundred adjacent 320×200 grabs did not. **Two-frame spacing never slipped** in fourteen runs, so each
window is filed as its even and odd indices:

```bash
cargo xtask reference capture --arg 2 --boot autoexec --cycles 30000 --keys "KBD_left KBD_enter" \
    --frames 430,432,434,436,438,440,442,444,446,448,450 --out reference/parts/beglogo-leadin/even
cargo xtask reference capture --arg 2 --boot autoexec --cycles 30000 --keys "KBD_left KBD_enter" \
    --frames 431,433,435,437,439,441,443,445,447,449 --out reference/parts/beglogo-leadin/odd
cargo xtask reference capture --arg 5 --boot autoexec --cycles 30000 --keys "KBD_left KBD_enter" \
    --frames 436,438,440,442,444,446,448,450,452,454 --out reference/parts/endlogo-leadin/even
cargo xtask reference capture --arg 5 --boot autoexec --cycles 30000 --keys "KBD_left KBD_enter" \
    --frames 437,439,441,443,445,447,449,451,453,455 --out reference/parts/endlogo-leadin/odd
```

**A batched run carries no `.idx`.** A batch's first index is read after its last (D19), here after the
ramp has begun, so the coherence check drops them. The index plane is measured one index per session
(the planes read straight after the grab, the DAC after them), over BEGLOGO 430–450 and ENDLOGO 436–455,
and counted with a byte histogram, e.g. `python3 -c 'import collections,sys;
print(collections.Counter(open(sys.argv[1],"rb").read()))' <frame>.idx`:

| window | indices | RGB | index plane |
|---|---|---|---|
| BEGLOGO | 430–438 | white, `85789170…` | **every byte index 15** (`283babe8…`, palette `{15: 63,63,63}`) |
| BEGLOGO | 439, 440, 442 | white | not read: *"this screen's rows are 320 bytes and it is 400 rows tall … the display wraps"* |
| BEGLOGO | 441, 443–446 | white | not kept: *"indexed capture does not reproduce the RGB frame"* |
| BEGLOGO | 447–450 | the ramp (`b6fe147d…` at 447 and 448, `54b03e8e…` 449, `5c73a056…` 450) | not kept |
| ENDLOGO | 436–443 | white, `85789170…` | **every byte index 0** (`eec19bc6…`, palette `{0: 63,63,63}`) |
| ENDLOGO | 444–447 | white | not read: the same 320-byte refusal |
| ENDLOGO | 448–451 | white | not kept |
| ENDLOGO | 452–455 | the ramp (`b3bb52bd…` at 452 and 453, `a330f340…` 454, `83e878d2…` 455) | not kept |

No readable plane in either window is anything but one value. ENDLOGO's field is index 0, not 15:
`END.C` never fills video memory, it only whitens DAC entries 0–254 (`:62-63`, `:90-91`). The even/odd
halves agree with the single sessions on every RGB: white through **446** (BEGLOGO) and **451**
(ENDLOGO), the ramp's first step displayed twice (447 = 448, 452 = 453). A5 and A7 own the port's side.

**Why the read stops at 439 and 444 is not settled.** The 320-byte refusal means the chain-4 probe
(write `3c4h=4`, then read `3c5h`) answered with bit 3 set, and `lineblit` writes `3c4h` with map mask
`1<<plane` every row (`BEG/ASM.ASM:65-67`), leaving it at 8: the blit loop (`BEG.C:73-78`,
`END.C:94-99`) running. But the boundary is also exactly eight frames before each ramp's first step,
where a DAC pass reaching the ramp is dropped as incoherent whatever the planes hold. Measured: the
plane is still the field at **436–438** (BEGLOGO) and **440–443** (ENDLOGO).

Filed one unbatched index each:

```bash
cargo xtask reference capture --arg 2 --boot autoexec --cycles 30000 --keys "KBD_left KBD_enter" \
    --frames 436 --out reference/parts/beglogo-leadin/plane-436
cargo xtask reference capture --arg 5 --boot autoexec --cycles 30000 --keys "KBD_left KBD_enter" \
    --frames 440 --out reference/parts/endlogo-leadin/plane-440
```

| capture | index | rgb | idx | plane |
|---|---|---|---|---|
| `beglogo-leadin/plane-436` | 436 | `85789170…` | `283babe8…` | 128,000 × index 15 |
| `endlogo-leadin/plane-440` | 440 | `85789170…` | `eec19bc6…` | 128,000 × index 0 |

### 25.7 Key numbers

- The loop capture's numbering is the five-key recipe's, 115 frames later than the two-key recipe the
  port's `--block 5` anchor (404) uses: `C` = 9029, the fade 9029–9092, the unpublished mode set 9102,
  `L` = 9104 (8914, 8914–8977, 8987 and 8989 in the port's numbering).
- **Δ = 8700** by `L − 404`; ALKU's music-synchronised content is at pass-1 + 8692–8694.
- ALKU's first wait on pass 2 is a race in the original; the filed capture is the majority group.
- The lead-in planes are the field (15 for BEGLOGO, 0 for ENDLOGO) at 430–438 and 436–443; from 439 /
  444 the oracle cannot read them; the RGB is white through 446 / 451.

---

## 26. The full `SECOND 1` run

The original's whole block-1 run: every part boundary, **one capture per screen geometry**
(`reference/parts/fullrun/01-alku` … `13-endscrl`, 44 frames), and **two music traces**
(`reference/music/second1e`, `second1f`, split at the U2E restart). The recipe is the two-key one, so
every index is in the per-part references' boot space and the first graphics frame is 404 (§1.12; the
loop recipe runs 115 later, §25.1). Every capture command here is

```bash
cargo xtask reference capture --arg 1 --boot autoexec --cycles 30000 --keys "KBD_left KBD_enter" \
    --frames <F> --out <D>
```

with `<F>` given in the tables; `$(seq …)` is expanded by the shell. Every session is its own boot.

### 26.1 The survey

A capture holds one geometry and the run changes geometry twelve times, so the survey is consecutive
sessions, each restarted at the index the previous one was refused at:

| session | `<F>` | result line | what it shows |
|---|---|---|---|
| s01 | `$(seq -s, 400 100 50000)` | `error: the emulator reached frame 410 before frame 400 could be captured; …` | the capture's demo-start wait returns after 404 |
| s02–s09 | `40N,$(seq -s, 500 100 50000)`, N = 1…8 | `… reached frame 409` (s02, s03, s05, s07, s08, s09) or `410` (s04, s06) `before frame 40N could be captured` | 400–408 cannot be captured |
| s10 | `409,$(seq -s, 500 100 50000)` | `error: the display changed geometry between frames: frame 6000 is 320x200 where the capture started at 320x400; capture one screen at a time` | 409–5900: ALKU, 320×400 |
| s11 | `$(seq -s, 6000 100 50000)` | `… frame 8000 is 320x400 where the capture started at 320x200 …` | U2A, PAM |
| s12 | `$(seq -s, 8000 100 50000)` | `… frame 8600 is 320x200 …` | BEGLOGO |
| s13 | `$(seq -s, 8600 100 50000)` | `… frame 15700 is 320x400 …` | GLENZ, TUNNELI, TECHNO's rings |
| s14 | `$(seq -s, 15700 100 50000)` | `… frame 16200 is 320x200 …` | TECHNO's troll |
| s15 | `$(seq -s, 16200 100 50000)` | `… frame 22000 is 320x400 …` | PANICEND, MNTSCRL, LNS&ZOOM |
| s16 | `$(seq -s, 22000 100 50000)` | `… frame 24600 is 320x200 …` | PLZPART's plasma |
| s17 | `$(seq -s, 24600 100 50000)` | `… frame 33300 is 320x400 …` | PLZPART's cube, MINVBALL, RAYSCRL, 3DSINFLD |
| s18 | `$(seq -s, 33300 100 50000)` | `… frame 34200 is 320x200 …` | JPLOGO |
| s19 | `$(seq -s, 34200 100 50000)` | `… frame 38000 is 320x400 …` | U2E |
| s20 | `$(seq -s, 38000 100 50000)` | `… frame 38600 is 320x200 …` | ENDLOGO |
| s21 | `$(seq -s, 38600 100 50000)` | `… frame 46300 is 640x400 where the capture started at 320x200 …` | CRED |
| s22 | `$(seq -s, 46300 100 50000)` | exit 0 | ENDSCRL, 46300–50000 |

The pictures are named by hash against every filed reference frame: 1700–1900 are `alku`'s 1719
(`6a27ec50…`), 6000–6900 `u2a`'s 6500 (`11a66eb0…`), 8100–8500 `beglogo`'s 900 (`cbdc67eb…`),
8800–9000 `glenz`'s 1499 (`e45ab076…`), 15800–16100 `techno/troll`'s 8400 (`40044edc…`), 16400–16600
`panicend`'s 9000 (`b412a265…`), 18700–19100 `lnszoom`'s 2700 (`6a152a31…`), 29300–29800 `rayscrl`'s
13500 (`92dc2c9e…`), 33800–34000 the JPLOGO state `ec3de2bb…` (§17.3), 34200–34300 U2E's opening hold
(`befaf38a…`, §18.1), 38200–38500 `endlogo`'s 900 (`f2514b29…`), and CRED's five filed holds at 38700,
40500, 42700, 44900 and 46000; the moving screens by eye. Two moving frames match exactly, pinning
their blocks' offsets: 12310 is `techno/interference`'s 4900 (`3dc29660…`) — **block 2 + 7,410** — and
32200 is `3dsinfld`'s 16000 (`2d349ada…`) — **block 3 + 16,200**. Block 4's U2E sits at **+33,647**
(§26.6) and block 5's CRED iris at **+37,358** (46271 is `loop/cred`'s 9028, `88532193…`, two-key
8913).

### 26.2 Refining the boundaries

Two levels: 10-frame steps across each 100-frame bracket (**L1**), then 1-frame steps across each
10-frame bracket (**L2**), spaced per D19, each session also taking `K−1, K, K+1` around a candidate
hold. Two refusals carry information:

- **`the display changed geometry … frame N is …`** names the first frame of the new geometry in that
  session.
- **`the emulator reached frame X before frame Y could be captured`** means Y was never grabbed. A
  single-index **probe** session (`p…`) then decides whether the counter steps over Y in every
  session (refused again: **reproduced**) or a host stalled once (captured: **not reproduced**).
  Reproduced: **7921, 8568, 16192, 24520, 33260, 38580, 46276** (two sessions each; 33260 three times,
  once as 33259). Not reproduced: 15603, 21925, 26570, 34114, 37965. Every reproduced one sits at a part
  load or mode change: a video-mode stop publishes no frame (§1.2, `music-clock.md` §7). Adjacent
  320×400 grabs also slip (§25.6: b2r 7929, k2r 38566, i3 34116).

| s | `<F>` | result line |
|---|---|---|
| L1 a | `1718,1719,1720,$(seq -s, 5910 10 5990)` | `… frame 5970 is 320x200 where the capture started at 320x400 …` |
| L1 b | `6499,6500,6501,$(seq -s, 7710 10 7890),7899,7900,7901,$(seq -s, 7910 10 7990)` | `… frame 7930 is 320x400 where the capture started at 320x200 …` |
| L1 c | `8299,8300,8301,$(seq -s, 8510 10 8590)` | `… frame 8570 is 320x200 …` |
| L1 d | `8899,8900,8901,$(seq -s, 11110 10 11190),11499,11500,11501,11899,11900,11901,$(seq -s, 12110 10 12390),12399,12400,12401,$(seq -s, 15610 10 15690)` | `… frame 15610 is 320x400 …` |
| L1 e | `15999,16000,16001,$(seq -s, 16110 10 16190)` | exit 0 (16190 is still 320×400) |
| L1 f | `16499,16500,16501,$(seq -s, 16610 10 16690),16999,17000,17001,$(seq -s, 18610 10 18690),18999,19000,19001,$(seq -s, 21910 10 21990)` | `… frame 21930 is 320x400 …` |
| L1 g | `22499,22500,22501,$(seq -s, 24510 10 24540),24549,24550,24551,$(seq -s, 24560 10 24590)` | `… reached frame 24521 before frame 24520 …` |
| L1 h | `25499,25500,25501,$(seq -s, 26510 10 26690),27499,27500,27501,$(seq -s, 29010 10 29190),29799,29800,29801,$(seq -s, 31110 10 31290),31899,31900,31901,32499,32500,32501,$(seq -s, 33210 10 33290)` | `… reached frame 26571 before frame 26570 …` |
| L1 i | `33899,33900,33901,$(seq -s, 34110 10 34190)` | `… frame 34120 is 320x200 …` |
| L1 j | `34249,34250,34251,$(seq -s, 37910 10 37990)` | `… frame 37970 is 320x400 …` |
| L1 k | `38299,38300,38301,$(seq -s, 38510 10 38590)` | `… reached frame 38581 before frame 38580 …` |
| L1 l | `40599,40600,40601,$(seq -s, 46210 10 46290)` | `… frame 46280 is 640x400 …` |
| L2 a2 | `$(seq -s, 5961 5969)` | exit 0 (all 320×200) |
| L2 b2 | `$(seq -s, 7701 7709),$(seq -s, 7720 20 7900),$(seq -s, 7911 7929)` | `… reached frame 7922 before frame 7921 …` |
| L2 c2 | `$(seq -s, 8521 2 8569)` | exit 0 (all 320×400) |
| L2 c3 | `$(seq -s, 8522 2 8568)` | `… reached frame 8569 before frame 8568 …` |
| L2 d2 | `$(seq -s, 11101 11109),11130,$(seq -s, 11151 11161),$(seq -s, 12181 12189),12200,$(seq -s, 12211 12219),$(seq -s, 15601 15609)` | `… reached frame 15604 before frame 15603 …` (up to 12219 read first) |
| L2 e2 | `$(seq -s, 16191 16199)` | `… reached frame 16193 before frame 16192 …` |
| L2 f2 | `$(seq -s, 16631 16639),16650,16660,16670,16679,16680,16681,$(seq -s, 18641 18659),$(seq -s, 21921 21929)` | `… reached frame 21926 before frame 21925 …` (up to 18659 read first) |
| L2 g2 | `23999,24000,24001,24499,24500,24501,24505,24510,24515,24521,24525,$(seq -s, 24530 10 24590)` | `… frame 24521 is 320x200 where the capture started at 320x400 …` |
| L2 g3 | `24520` | `… reached frame 24521 before frame 24520 …` |
| L2 h2 | `26499,26500,26501,26505,$(seq -s, 26510 10 26550),$(seq -s, 26560 26569),$(seq -s, 26571 26579),$(seq -s, 26580 10 26690),26699,26700,26701,27999,28000,28001,$(seq -s, 29010 10 29190),29799,29800,29801,$(seq -s, 31110 10 31240),31249,31250,31251,$(seq -s, 31260 10 31290),31899,31900,31901,32499,32500,32501,$(seq -s, 33210 10 33290)` | `… reached frame 33261 before frame 33260 …` (up to 32501 read first) |
| L2 h4 | `26570` | exit 0 (26570 is black) |
| L2 i2 | `$(seq -s, 34111 34119)` | `… reached frame 34116 before frame 34114 …` |
| L2 j2 | `$(seq -s, 37951 37969)` | `… reached frame 37966 before frame 37965 …` |
| L2 k2 | `$(seq -s, 38510 10 38570),38572,38574,38576,38578,38582,38584,38586,38588,38590` | `… frame 38584 is 320x200 where the capture started at 320x400 …` |
| L2 k3 | `38580` | `… reached frame 38581 before frame 38580 …` |
| L2 l2 | `$(seq -s, 46271 46279)` | `… reached frame 46277 before frame 46276 …` |
| b2r | `$(seq -s, 7701 7709),$(seq -s, 7720 20 7900),$(seq -s, 7911 7920),$(seq -s, 7922 7929)` | `… reached frame 7930 before frame 7929 …` (adjacent 320×400) |
| c3r | `8519,8520,8521` | exit 0 |
| p7921 | `6499,6500,6501,7921` | `… reached frame 7922 before frame 7921 …` |
| p8568 | `8299,8300,8301,8568` | `… reached frame 8569 before frame 8568 …` |
| p15603 | `12399,12400,12401,15603` | exit 0 (15603 is 320×200) |
| e2r | `16189,16191,16193,16195,16197,16199` | `… frame 16195 is 320x200 where the capture started at 320x400 …` |
| p16192 | `15999,16000,16001,16192` | `… reached frame 16193 before frame 16192 …` |
| f2r | `$(seq -s, 21921 21924),$(seq -s, 21926 21929)` | `… frame 21927 is 320x400 where the capture started at 320x200 …` |
| p21925 | `18999,19000,19001,21925` | exit 0 |
| h3 | `$(seq -s, 29061 29069),$(seq -s, 31211 31219),33110,33130,33150,33260` | `… reached frame 33261 before frame 33260 …` (all before read first) |
| h5 | `$(seq -s, 29171 29179),$(seq -s, 31271 31279),$(seq -s, 33251 33259),$(seq -s, 33261 33269)` | `… reached frame 33260 before frame 33259 …` (up to 31279 read first) |
| i2r | `34111,34113,34116,34118` | `… reached frame 34114 before frame 34113 …` |
| p34114 | `33899,33900,33901,34114` | exit 0 (34114 is 320×400) |
| j2r | `$(seq -s, 37951 37964),$(seq -s, 37966 37969)` | `… frame 37967 is 320x400 where the capture started at 320x200 …` |
| p37965 | `34249,34250,34251,37965` | exit 0 |
| k2r | `$(seq -s, 38561 38569),38579,38581,38583` | `… reached frame 38567 before frame 38566 …` (adjacent 320×400) |
| l2r | `46271,46273,46275,46277,46279` | `… frame 46277 is 640x400 where the capture started at 320x200 …` |
| p46276 | `40599,40600,40601,46276` | `… reached frame 46277 before frame 46276 …` |
| b2r2 | `$(seq -s, 7701 7709),$(seq -s, 7720 20 7900),$(seq -s, 7911 7920),7922` | exit 0 (7922 is still 320×200) |
| h6 | `33261,33263,33265,33267` | `… frame 33263 is 320x400 where the capture started at 320x200 …` |
| i3 | `$(seq -s, 34115 34119)` | `… reached frame 34117 before frame 34116 …` |
| k4 | `38561,38563,38565,38567,38569,38583` | `… frame 38583 is 320x200 where the capture started at 320x400 …` |
| h7 | `$(seq -s, 26671 26679),$(seq -s, 29131 29139),$(seq -s, 33155 5 33195)` | exit 0 |

A refused session leaves the frames it read before the refusal on disk, and they were hashed; a
session refused at take time loses the unread batch before it, which is why b2, c3, i2, j2, k2r and l2
were taken again.

### 26.3 The boundary table

"First" is the first frame that is the part's (its own geometry, or its own first change to the
screen); "last" is the last frame of the previous part's picture. **Where a black gap separates two
parts of one geometry, the handover is invisible**: the row gives the last visible frame of one, the
first visible frame of the next, and the gap. Indices that differ between sessions are given with the
sessions that measured them (D17's ±1; three boundaries move by 2).

| # | part | first frame | last frame | session(s) | geometry |
|---|---|---|---|---|---|
| 1 | ALKU | **404**, the first graphics frame (D17); this tool refuses 400–408 (s01–s09); 409 is the black `fec9afb5…` = `alku`'s 900 | **5960** (still 320×400, the screen being split, §4.5) | s10, L1 a | 320×400 |
| 2 | U2A | **5961**, the first 320×200 frame: in a2 `0a68ffb9…`; in the filed session already `0b95e3d6…` (a2's 5962), one frame earlier | **7706** (U2A's landscape `0a68ffb9…` through 7706) | a2, L1 a, b2r2, `fullrun/02` | 320×200 |
| 3 | PAM | **7707** (`1fef46de…`, PAM's white) | **7919** (L1 b, where 7920 is BEGLOGO's) / **7920** (b2r2 and `fullrun/02`, where 7920 is still PAM's white) | b2r2, L1 b, `fullrun/02` | 320×200 |
| 4 | BEGLOGO | **7920** (L1 b) / **7922** (b2r2): `b94ba42a…`, rows 0–110 index-15 white (255) over PAM's 251, the `memset(vram,15,…)` of `BEG.C:18-21` in progress; **7921 is never published** (b2, p7921); first 320×400 frame in (7922, 7930] | **8520 or 8521**, the hold `cbdc67eb…` (L1 c 8520, c3r 8521) | L1 b, b2r2, p7921, L1 c, c3r | 320×200 → 320×400 |
| 5 | GLENZ | **8521** (c2) / **8522** (c3r and `music-clock.md` §2.2): the collapse's first changed frame, `dis_setmframe(0)` at 8521 (`second1d`); 320×400 through 8569 (c2), **8568 never published** (c3, p8568), 320×200 from 8570 (L1 c) | **11102**, last visible (11103–11158 black) | c2, c3r, c3, p8568, L1 c, d2 | 320×400 → 320×200 |
| 6 | TUNNELI | **11159** (d2: 64 px) / **11158** (`fullrun/04` has d2's 11160 bytes at 11159) | **12184**, last dots (12185–12213 black) | d2, `fullrun/04` | 320×200 |
| 7 | TECHNO | **12214** (d2 and `fullrun/04`, `eb6b85c9…`; 12310 is `techno/interference`'s 4900) | **16191**, the troll `40044edc…` (e2r); rings 320×200 through 15603 (p15603), troll 320×400 from ≤ 15610 (L1 d) | d2, p15603, L1 d, e2r | 320×200 → 320×400 |
| 8 | PANICEND | **16193**: `1f039cb7…`, the troll squashed by PANICEND's CRTC write, **still 320×400** (e2r, `fullrun/05`); **16192 never published** (e2, p16192); 320×200 from 16195 (e2r) | **16636**, the dot `b412a265…` (f2, `fullrun/06`) | e2r, p16192, f2 | 320×400 → 320×200 |
| — | **PANICEND's cue wait** (`MAIN/U2.ASM:833-853`) | exits at **16630**, the first frame `musplus` > 0 (`second1e`, §26.6); the dot stays to **16636**; **16637** is the first changed frame | | `second1e`, f2, `fullrun/06` | |
| 9 | MNTSCRL | **16637**, black `ea0787f6…` (the mode set); the forest fades in from 16640 (L1 f) | **18648**, last visible (18649–18655 black) | f2, L1 f | 320×200 |
| 10 | LNS&ZOOM | **18656** (`c3ac679a…`, f2 and `fullrun/06`) | **21926**, its white `20f3d62f…` (f2r; `lnszoom`'s 5650 is the same white) | f2, f2r | 320×200 |
| 11 | PLZPART | **21927**, the first 320×400 frame, white `85789170…` (f2r, `fullrun/07`); plasma → cube: 320×400 through 24515 (g2), **24520 never published** (L1 g, g3), 320×200 from 24521 in g2 but still 320×400 black at 24521 in the first filing session | **26561**, last visible cube (26562–26672 black) | f2r, g2, g3, h2 | 320×400 → 320×200 |
| 12 | MINVBALL | **26673** (`d43168f0…`, h7 and `fullrun/08`) | **29133**, the end of its white→black fade (`DOTS/MAIN.C:242-266`; 29134 black) | h7 | 320×200 |
| 13 | RAYSCRL | **29174** (`d37d5b68…`, h5 and `fullrun/08`; 29134–29173 black) | **31216**, last visible (31217–31271 black) | h5, h3 | 320×200 |
| 14 | 3DSINFLD | **31272** (h5: 8 px, `d69d90c5…`); still black at 31272 in `fullrun/08`'s session | in **(33190, 33195]**: last visible 33190 (104 px), 33195 black | h5, h7 | 320×200 |
| 15 | JPLOGO | first 320×400 frame in **(33261, 33263]** (h6: 33261 is 320×200 black, 33263 320×400 black `fec9afb5…`); **33260 never published** (h2, h3; h5 33259) | **34114**, its white `668f1e13…`, 320×400 (p34114); the white is up from ≤ 34100 (s18) | h6, h2, h3, h5, p34114 | 320×400 |
| — | **the U2E `MUSIC0@18` restart** (`MAIN/U2.ASM:880-885`) | the restarted clock starts at **34,055**, in [34,052, 34,055] (§26.6); the reload runs under JPLOGO's white: `second1e` is valid through 33949, and windows ending at 34004 and 34069 are refused | | `second1e`, `second1f` | |
| 16 | U2E | first 320×200 frame in **(34114, 34120]**; **34120** is U2E's band `6eb4e4cd…` (§18.7's 467); 34113, 34114 and 34116 refused in i2r, i2 and i3, 34114 captured in p34114 | **37966**: U2E's fade to white is complete at 37962 (`20f3d62f…`) and 320×200 lasts to 37966 (j2r) | L1 i, i2, i2r, i3, p34114, `fullrun/10`, j2r | 320×200 |
| 17 | ENDLOGO | **37967**, first 320×400 frame (j2r); refused in the first filing session (`… reached frame 37968 before frame 37967 …`), and `second1f` skips it | **38569**, last visible (k4; 38570–38582 black, k2) | j2r, k4, k2 | 320×400 |
| 18 | CRED | **38583**, first 320×200 frame (k4; k2's 38582 is 320×400); **38580 never published** (L1 k, k3) | **46275**, the closed black (46271 is the iris's last step `88532193…`, 46273 and 46275 black) | k4, k2, k3, l2r | 320×200 |
| 19 | ENDSCRL | **46277**, first 640×400 frame (l2r); **46276 never published** (l2, p46276); still 320×200 black in the first filing session | — (still scrolling at 50000, the survey's last frame; the release's build ends at its text's `%` line, D49) | l2r, p46276 | 640×400 |

**The run's length to ENDSCRL is 46277 − 404 = 45,873 frames**: 654.5 s at 70.086 Hz.

**Holds, measured** (`K−1 = K = K+1`, one `sha256`): ALKU 1718–1720 (L1 a), U2A 6499–6501 (L1 b,
p7921), PAM 7899–7901 (L1 b), BEGLOGO 8299–8301 (L1 c, p8568), GLENZ 8899–8901 (L1 d), TECHNO
12399–12401 (the white, L1 d and p15603) and 15999–16001 (the troll, L1 e and p16192), PANICEND
16499–16501 (L1 f), MNTSCRL 16679–16681 (f2), LNS&ZOOM 18999–19001 (L1 f, p21925), RAYSCRL
29799–29801 (h2), JPLOGO 33899–33901 (L1 i, p34114), U2E 34249–34251 (L1 j, p37965), ENDLOGO
38299–38301 (L1 k), CRED 40599–40601 (L1 l, p46276). **Four parts have none**; the filed frame is the
least-moving one measured, by differing pixels to each neighbour:

| part | triplet | `K−1/K`, `K/K+1` | filed |
|---|---|---|---|
| TUNNELI | 11499–11501 / **11899–11901** (L1 d) | 6,384, 6,274 / **4,125, 3,939** of 64,000 | 11900 |
| PLZPART | 22499–22501 (L1 g) / **24499–24501** (g2) / 25499–25501 (h) | 89,379, 89,413 / **22,166, 20,662** of 128,000 / 56,999, 57,054 of 64,000 | 24500 |
| MINVBALL | **26699–26701** (h2) / 27999–28001 (h2) | **0, 31,360** / 4,557, 4,527 of 64,000 | 26700 |
| 3DSINFLD | **31899–31901** / 32499–32501 (h2) | **27,978, 0** / 26,592, 0 (it draws every second frame) | 31901 |

g2's 23999–24001 are three black frames, but another session's 24000 is not black (`6d2dc87c…`), so
that is a moment of the plasma's fade, not a hold.

**Key numbers.** The never-published frames 7921, 8568, 16192, 24520, 33260, 38580 and 46276 have no
reference for a port frame to pair with. The cue-wait exit is 16630 and the screen changes at 16637.
The U2E restart is 34,055 in [34,052, 34,055], its reload under JPLOGO's white (≤ 34100 … 34114), U2E's
band at 34120. Block offsets: block 2 + 7,410, block 3 + 16,200, block 4 + 33,647 (at order 18), block
5 + 37,358 at CRED's last iris step.

### 26.4 The filed captures

One session per geometry segment (the fixed prefix is the command above):

```bash
… --frames 1719 --out reference/parts/fullrun/01-alku
… --frames 5961,6500,7707,7880,7920 --out reference/parts/fullrun/02-u2a-pam
… --frames 7930,8300,8522 --out reference/parts/fullrun/03-beglogo
… --frames 8572,8900,11159,11900,12214,12400 --out reference/parts/fullrun/04-glenz-techno
… --frames 16000,16193 --out reference/parts/fullrun/05-troll-panicend
… --frames 16500,16636,16637,16690,18656,19000 --out reference/parts/fullrun/06-panicend-lnszoom
… --frames 21927,24500 --out reference/parts/fullrun/07-plasma
… --frames 24540,26673,26700,29174,29800,31272,31901 --out reference/parts/fullrun/08-cube-3dsinfld
… --frames 33263,33900,34110 --out reference/parts/fullrun/09-jplogo
… --frames 34120,34250 --out reference/parts/fullrun/10-u2e
… --frames 37970,38300 --out reference/parts/fullrun/11-endlogo
… --frames 38583,40600 --out reference/parts/fullrun/12-cred
… --frames 46280,46283,46286 --out reference/parts/fullrun/13-endscrl
```

Three segments start later than §26.3's first frames (24540, 37970, 46280), the first frames every
session agrees are in the new geometry: asking for 24521, 37967 or 46277 was refused in the filing
session.

**Every `.idx` is from an unbatched read.** The indices are ≥ 25 apart except the cue-wait pair
16636/16637 and ENDSCRL's 46280/46283/46286, which are batched and carry no `.idx` (§25.6). JPLOGO's
33900 and 34110 have the **same** index plane (`201e9833…`): the white is a DAC change over the logo's
pixels.

| capture | index | what | rgb | `.idx` |
|---|---|---|---|---|
| `01-alku` | 1719 | ALKU, held (= `alku`'s 1719) | `6a27ec50…` | — |
| `02-u2a-pam` | 5961 | U2A's first frame (a2's 5962 bytes) | `0b95e3d6…` | — |
| | 6500 | U2A, held (= `u2a`'s 6500) | `11a66eb0…` | — |
| | 7707 | PAM's first frame, its white | `1fef46de…` | `4f798803…` |
| | 7880 | PAM, held | `1fef46de…` | `f5480708…` |
| | 7920 | PAM's white, the last 320×200 index before BEGLOGO's fill in this session | `1fef46de…` | — |
| `03-beglogo` | 7930 | BEGLOGO's ramp (= `beglogo-leadin`'s 449) | `54b03e8e…` | — |
| | 8300 | BEGLOGO, held (= `beglogo`'s 900) | `cbdc67eb…` | `e10481c7…` |
| | 8522 | GLENZ's first frame, the collapse | `da1f9be6…` | — |
| `04-glenz-techno` | 8572 | GLENZ's first 320×200 index | `35ad5005…` | `f1a7aa1a…` |
| | 8900 | GLENZ, held (= `glenz`'s 1499) | `e45ab076…` | `75d561dc…` |
| | 11159 | TUNNELI's first frame (d2's 11160) | `d0f33e4a…` | — |
| | 11900 | TUNNELI, least-moving (= L1 d's 11900) | `a2e82430…` | — |
| | 12214 | TECHNO's first frame (= d2's 12214) | `eb6b85c9…` | — |
| | 12400 | TECHNO, held white | `20f3d62f…` | — |
| `05-troll-panicend` | 16000 | TECHNO's troll, held (= `techno/troll`'s 8400) | `40044edc…` | — |
| | 16193 | PANICEND's first frame (= e2r's 16193) | `1f039cb7…` | — |
| `06-panicend-lnszoom` | 16500 | PANICEND, held dot (= `panicend`'s 9000) | `b412a265…` | `9d7106ca…` |
| | 16636 | the last dot frame, after the cue wait's exit at 16630 | `b412a265…` | — |
| | 16637 | MNTSCRL's first frame, black | `ea0787f6…` | — |
| | 16690 | MNTSCRL, held (= `mntscrl`'s 500) | `e68aa887…` | — |
| | 18656 | LNS&ZOOM's first frame (= f2's 18656) | `c3ac679a…` | — |
| | 19000 | LNS&ZOOM, held (= `lnszoom`'s 2700) | `6a152a31…` | `70175500…` |
| `07-plasma` | 21927 | PLZPART's first frame, 320×400 white | `85789170…` | — |
| | 24500 | PLZPART, least-moving (= g2's 24500) | `e161804c…` | — |
| `08-cube-3dsinfld` | 24540 | PLZPART's cube screen, first 320×200 index every session agrees on (black) | `ea0787f6…` | `4f798803…` |
| | 26673 | MINVBALL's first frame (= h7's 26673) | `d43168f0…` | — |
| | 26700 | MINVBALL, least-moving (= h2's 26700) | `bbdf2ae7…` | — |
| | 29174 | RAYSCRL's first frame (= h5's 29174) | `d37d5b68…` | — |
| | 29800 | RAYSCRL, held (= `rayscrl`'s 13500) | `92dc2c9e…` | `afda560c…` |
| | 31272 | 3DSINFLD's first frame in h5; black in this session | `ea0787f6…` | — |
| | 31901 | 3DSINFLD, least-moving | `7f1d2fd1…` | `6bbaf70b…` |
| `09-jplogo` | 33263 | JPLOGO's first 320×400 index, black | `fec9afb5…` | `eec19bc6…` |
| | 33900 | JPLOGO, held (§17.3's state `ec3de2bb…`) | `ec3de2bb…` | `201e9833…` |
| | 34110 | JPLOGO's white, held under the U2E restart | `668f1e13…` | `201e9833…` |
| `10-u2e` | 34120 | U2E's first visible frame, the band (§18.7) | `6eb4e4cd…` | — |
| | 34250 | U2E, held (§18.1's `befaf38a…`) | `befaf38a…` | `09b1f319…` |
| `11-endlogo` | 37970 | ENDLOGO's white | `85789170…` | `4ebd6f58…` |
| | 38300 | ENDLOGO, held (= `endlogo`'s 900) | `f2514b29…` | `c0e0e259…` |
| `12-cred` | 38583 | CRED's first frame, black | `ea0787f6…` | — |
| | 40600 | CRED, held (= `cred`'s 3240) | `98de65f3…` | — |
| `13-endscrl` | 46280 | ENDSCRL, its 4th frame | `b231a5f2…` | — |
| | 46283 | | `6b5e1a45…` | — |
| | 46286 | | `4a120a81…` | — |

**Nine moving frames reproduce a refinement session's bytes exactly**: 12214, 16193, 18656, 24500,
26673, 26700 and 29174 at their own index, 5961 and 11159 one index later (a2's 5962, d2's 11160).
34120 is the `SECOND 4` boot's band (§18.7) byte for byte. Where a filed first frame differs from the
refinement session's (31272 black here, 8 px in h5) it is the session phase D17 measures.

### 26.5 The repeat

The held frames once more, one session per segment (`--frames 1719`; `6500,7880`; `8300`;
`8900,12400`; `16000`; `16500,16636,16690,19000`; `29800`; `33900,34110`; `34250`; `38300`; `40600`):
all eleven exit 0, **all seventeen frames' `sha256` equal the filed capture's**, and so do all ten
index planes both sessions read (7880, 8300, 8900, 16500, 19000, 29800, 33900, 34110, 34250, 38300).
The comparison is the two manifests' digests (`<repeat>` is the directory holding the eleven
sessions):

```bash
python3 -c 'import json,glob; [print(m, all(f["sha256"]==g["sha256"] for f in json.load(open(m))["frames"] for g in json.load(open(m.replace("<repeat>","reference/parts/fullrun")))["frames"] if f["index"]==g["index"])) for m in sorted(glob.glob("<repeat>/*/manifest.json"))]'
```

The moving frames are not repeated: §26.3 and §1.12 say they differ by session.

### 26.6 The music traces

```bash
cargo xtask music trace --arg 1 --cycles 30000 --frames 8800..33950 --out reference/music/second1e
cargo xtask music trace --arg 1 --cycles 30000 --frames 34105..48277 --out reference/music/second1f
```

`second1e` is MUSIC1 orders 4…98 (`traced 25143 frame(s) … 7 frame(s) missing (12207, 15603, 16192,
21926, 24520, 26570, 33261)`); `second1f` MUSIC0 orders 18…45 (`traced 14168 frame(s) … 4 frame(s)
missing (34116, 37967, 38569, 46264)`). Only 16192 and 24520 are refusals the captures **reproduced**
(§26.2), and 33261 sits beside the reproduced 33260. 15603, 21926 (≈ 21925), 26570, 34116 (≈ 34114)
and 37967 (≈ 37965) sit where a capture was refused once and captured on a retry: unreproduced host
stalls at the same load, with no shown common cause. 12207 is TECHNO's mode change, inside the
TUNNELI→TECHNO black gap (12185–12213). **38569 and 46264 are unexplained** (k4 captured 38569; L1 l
asked for 46260 and 46270 and was refused only at 46280, for geometry). `music-clock.md` §7 carries
the same qualifier.

**The `second1e` window is narrowed**, as `second3d`'s was (`music-clock.md` §1). Two longer windows
are refused:

```
$ cargo xtask music trace --arg 1 --cycles 30000 --frames 8800..34070 --out reference/music/second1e
error: _np_ord reads -1, which is not a byte value: the base 0x0051F0 is not this segment's
$ cargo xtask music trace --arg 1 --cycles 30000 --frames 8800..34005 --out <scratch>
error: the needle is in 2 places and the bytes around them are NOT one memory: the copy at 0x1051F0 differs from the one at 0x0051F0 at offset 0x0068 (192 against 64), so one of them is a stale copy and a trace read through it would be a record of dead bytes
```

so the reload is under way by the end-of-trace re-scan after 34004, and the filed window ends at
33949. `second1f` starts at the restart + 50; a first attempt from 34170 agrees on the row-change
frames to within −2…+3.

**The restart is 34,055, in [34,052, 34,055].** `restartmus` does not zero `_np_zframe`
(`music-clock.md` §2.1), so the restart is read from the order-18 rows' offsets against `second4` (the
block-4 boot's own restart, whose start is 408.2):

```bash
python3 -c 'import csv
def f(p): d={}; [d.setdefault((int(r["ord"]),int(r["row"])),int(r["frame"])) for r in csv.DictReader(open(p))]; return d
a=f("reference/music/second1f/trace.csv"); b=f("reference/music/second4/trace.csv")
[print(k, a[k]-b[k]) for k in sorted(a) if k in b and k[0]==18]'
```

It prints 33,647 at 17 of order 18's 23 rows, 33,646 at 5, and 33,644 at one, `(18, 9)`, which is
`second4`'s first sample (frame 500, the window's edge) rather than a row change; the row changes alone
give [34,054, 34,055]. The point value 34,055 = 408.2 + 33,647 is `crates/sr-audio/tests/trace.rs`'s
`RESTART_TRACES` row. U2E's band follows at 34120 (R + 65; block 4: 467 = R + 59). **The two sessions
drift**: orders 19…25 start 33,646, 33,644, 33,642, 33,641, 33,639, 33,636, 33,635 frames after
`second4`'s, and stay there: 11 frames over the 3,421 frames of orders 19–25.

**The PANICEND cue wait exits at 16630.** `musplus` from `second1e`'s `zplus` and `row` (the
`DIS/DISINT.ASM:252-271` rule): −32 from 16363 to 16621 (order 39 row 32 … order 40 row 31), 0 at
16622 (order 42 row 0; order 40 breaks to 42), **1 at 16630** (order 42 row 1). The wait
(`MAIN/U2.ASM:833-853`) loops while `musplus` ≥ 0, then while ≤ 0, and leaves on the first value > 0.

```bash
python3 -c 'import csv; [print(r["frame"],r["ord"],r["row"],r["zplus"]) for r in csv.DictReader(open("reference/music/second1e/trace.csv")) if 16616<=int(r["frame"])<=16632]'
```

The port's player against these traces is A54 and `music-clock.md` §2.3 (`cargo test -p sr-audio
--test trace`).

### 26.7 Key numbers

Folded into §26.3's last paragraph.

### 26.8 The port's column

This ledger records the original; the port's side of the full run is owned elsewhere:

- **The port's first frame** of each part is the first frame at or after the part's start that meets
  §26.3's definition (its own geometry, its first change to the screen, or its first visible frame
  after a black gap); GLENZ's is the collapse, the first frame no longer BEGLOGO's held picture. The
  original's is §26.3's in the session the part's `fullrun` capture was filed in, where §26.3 gives
  two (BEGLOGO 7922, GLENZ 8522, TUNNELI 11158; 3DSINFLD h5's 31272). JPLOGO and U2E, which §26.3 can
  only bracket, are aligned on bytes: JPLOGO's opening black (`fullrun/09`'s 33263) and U2E's band
  (`fullrun/10`'s 34120).
- **The seam table** is `FULLRUN_SEAMS` in `xtask/src/rediff/table.rs`, asserted against the run by
  `bins/secondreality/tests/fullrun.rs` (`cargo test --release -p secondreality --test fullrun --
  --nocapture`); its argument, and the loader's frames the port does not model, are A53.
- **The pairing of the filed frames**: each `fullrun` frame is rendered at its reference index plus its
  part's start difference; the three loader-stretch frames (7920, 16636, 34110) pair with the port's
  last frame of that screen, and BEGLOGO's 8300 lies across a handover (`FULLRUN_ACROSS_HANDOVER`).
  `xtask/tests/rediff.rs`'s `every_fullrun_row_is_at_its_parts_start_difference` holds the rule; the
  counts are `cargo xtask rediff --only fullrun`'s.
- **The music, row by row**, against the six block-1 traces is `music-clock.md` §2 (A38, A54); the
  restart carry-over is D45.
- **BEGLOGO's and ENDLOGO's own row stride** after a predecessor that set one: `beglogo.md`,
  `endlogo.md`.

---

## 27. U2A's pre-gate hold at four other CPU speeds

The 30,000-cycle reference's `u2a/frame_006500.rgb` (`11a66eb0…`) is a neon palette (A32). The same
recipe at four other speeds, two frames each, one session per speed:

```bash
cargo xtask reference capture --arg 1 --boot autoexec --keys "KBD_left KBD_enter" --cycles <N> \
    --frames 6400,6600 --out reference/parts/u2a-speed/c<N>
```

| directory | cycles | 6400 | 6600 |
|---|---|---|---|
| `u2a-speed/c15000` | 15000 | `0a68ffb9…` | `0a68ffb9…` |
| `u2a-speed/c25000` | 25000 | `0a68ffb9…` | `0a68ffb9…` |
| `u2a-speed/c35000` | 35000 | `0a68ffb9…` | `0a68ffb9…` |
| `u2a-speed/c60000` | 60000 | `0a68ffb9…` | `0a68ffb9…` |

`0a68ffb9…` is the `U2ABG.UH` picture under the merged palette, the bytes of U2A's 7640 and PAM's
7660–7700 (§5.2, §6.1). At 30,000 cycles (every 100 frames, 6100–7700) 6100–6900 are `11a66eb0…` and
7000 is `0a68ffb9…`. The port's side is `cargo xtask rediff --only u2a-speed`. These are fidelity evidence, not
keyframes with ±1 supports: the hold is held (§1.3).
