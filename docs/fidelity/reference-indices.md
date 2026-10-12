# Reference indices — one row per part, and how the indices are found

Phase 1's reference ledger. One row per part, and for each part the boot indices its keyframes
were captured at, the geometry of those frames, what is on screen at each, the `sha256` of every
file the capture wrote, and the command that produced it — plus the re-capture that shows the
reference reproduces.

The rows are the deliverable; the method below them is not commentary. §1 is the procedure every
later capture task (B3, B4, B5) repeats, written down because each step of it cost a measurement
to find, and because two of the rules the plan gave for it turned out to be wrong in ways that
only show up on a part whose keyframes are far apart — which every part after BEGLOGO is. §2 is
the table. §3 is BEGLOGO, the first row.

**The recipe every capture in Part B uses** is fixed and is in the manifests:

```
--arg <block> --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000
```

`--boot autoexec` is B1's recipe (D17): the guest's own `[autoexec]` runs the mount, `AUTOTYPE`
answers the setup screen on the emulated clock, and the emulator runs silent, so the same boot
index is the same screen in every run. A capture's manifest records `boot`, `cycles` and `keys`,
so a frame set that was captured some other way says so. Indices are **boot-relative**: frame 0
is the emulator's first rendered frame at process start (D14), the setup screen's text is up at
16, and the demo's first graphics frame is 404.

---

## 1. The method

### 1.1 Sweep, then look at the sheet

```
cargo xtask reference capture --arg 2 --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames <a dozen ascending indices> --out out/<part>-sweep --sheet out/<part>-sweep.png
```

A dozen indices spanning the few seconds the part is expected to occupy, written into `out/` —
**never into `reference/`**. The sweep is discovery: what it captures is thrown away once the
indices have been read off the sheet, and the sheet is how they are read. The capture prints each
tile's position as it lays it out (`r0c0 frame 430 (frame_000430.rgb)`), so the sheet and the log
say which index is which tile and nothing has to be inferred from a grid position.

Read three things off it: where the part's picture starts, where it holds still, and where it
stops being this part. Where a part has a held screen, that is where the keyframes belong (§1.4).

The sheet holds what the *successful* capture took. A sweep that crosses a part boundary never
succeeds (§1.2), so the first frames of the *next* part come from the refusal's index and from the
first sweep taken inside that part — not from this sheet.

### 1.2 Where a part ends: the refusal is the discovery

A capture is one geometry (D15). A sweep that runs from one part into the next changes geometry
under it — BEGLOGO is 320×400 Mode X, GLENZ is 320×200 chain-4, and `video/frame` follows the
guest rather than normalising — so the capture stops at the first index of the new geometry:

```
error: the display changed geometry between frames: frame 1170 is 320x200 where the capture
started at 320x400; capture one screen at a time
exit: 2
```

That refusal is the boundary, and it is not to be worked around: sweep ascending, read the index
it names, and **the change lies between the last index that captured and the one that was
refused**. Two things about the mechanics matter:

- **A refused capture writes no manifest.** The frames taken before the refusal are on disk in the
  output directory (`out/beglogo-boundary/` holds `frame_001110.rgb`, `frame_001130.rgb` and
  `frame_001150.rgb`), but the manifest is written only after every requested index has been read,
  so nothing in that directory is a capture. The indices come from the refusal's message and from
  the narrower runs, not from the leftover frames.
- **Indices just below the change are grab-hostile.** The changeover stalls the renderer, and a
  requested index in that window is refused for a different reason — measured on BEGLOGO: 1155
  (`the emulator reached frame 1156 before frame 1155 could be captured`), 1158 (twice, reaching
  1159), and 444 in a dense probe of the fade's first frames. So narrow the boundary with **sparse
  single-index probes from both sides** rather than a dense run: a dense list spends its takes in
  the window that refuses them, and one miss refuses the whole capture.

BEGLOGO's boundary, measured that way, is in §3.4.

### 1.3 Two rules for a list of indices

**A run is read after it is taken.** Requested indices closer together than `INDEXED_READ_FRAMES`
(25) are all grabbed before any of them is read, because one indexed read (the DAC palette is 1024
of its requests) costs ~20 rendered frames and used to sit between two grabs (D19). The price is
deferred reads: a run of *k* indices spends ~20·*k* rendered frames reading afterwards, and the
next requested index must be further away than that or it arrives during the reads and is refused.
Measured again here, at 30000 cycles on this emulator: `--frames 429,430,431,479,480,481,…`
captures the three and then refuses the fourth — `the emulator reached frame 488 before frame 479
could be captured`, so three reads cost 57 frames, ~19 each. That is the number the plan's "~25·*k*"
names the ceiling of; leave **~20·*k*** after a run of *k*.

**A read's palette is the palette at read time, not at grab time.** The frame's pixels — RGB and
indices alike — are the ones taken, but the 256 DAC entries are read live when the run is read, so
a palette that moves in between makes the pair incoherent and D15's rule keeps the frame RGB-only.
Two consequences worth knowing before reading a sweep's output: **a screen whose palette is fading
is always RGB-only**, however still its indices are (the read is 20+ frames late, and that is
20+ fade steps); and **a frame inside a batch is read at the end of that batch**, so a batch that
spans a palette change is RGB-only even where a single-index capture of the same index keeps its
indices. Both are measured: BEGLOGO's 470 and 560 were RGB-only in all three sessions that took
them, and `out/beglogo-fade` — one batch across the whole fade — has 430–570 RGB-only while the
single-index capture of 430, taken at its own palette, has its indices.

"rgb only" in a capture's output therefore means *the indices would not have been honest*, which
is either a moving screen or a late read; it never means the RGB frame is the wrong frame.

**The reference holds keyframes only; the capture compared against it holds support frames.** The
asymmetry is D19's and the reason is `compare`'s rule: it pairs *every* frame of the reference, so
a support frame sitting in the reference is itself a reference index whose counterpart lies one
further out. The plan's form for the partner is `--frames <i-1,i,j,k,k+1>` — one support beyond
each *end* of the keyframe range. **That is only enough when the keyframes are contiguous**, which
is the case B1b measured it on. Two corrections, both from this task:

1. **Each keyframe that moves needs a support on each side, not just the outermost.** A session's
   phase is uniform across a whole capture (D17 measured it across five indices; nothing since has
   shown a mixed one), so a partner one step out of phase has to answer *every* keyframe from its
   neighbour — an interior keyframe answered from its own index pairs at offset 0 and reports the
   one-step difference (exit 1) rather than aligning. The partner therefore holds `K-1, K, K+1`
   around each keyframe that changes from frame to frame.
2. **A keyframe whose neighbours are the same picture needs no support at all.** A held screen's
   ±1 candidates are the same bytes, so the search finds offset 0 whatever the partner's phase is.
   That is measured, not assumed, per keyframe: 429, 430 and 431 are byte-identical (and equal to
   410), and so is every sampled index of the hold from 580 to 1100 — the one shared index that
   did differ, 1110, is the collapse's edge and is not a keyframe (§3.3) — so BEGLOGO's white and
   held keyframes carry no supports and only the two fade keyframes do.

Both corrections cost something the plan's form does not: because a keyframe's three supports are
one batch, **consecutive keyframes have to be ~60 frames apart in the partner's list** — that is
the measured cost of a 3-index batch (57 frames, above), and the keyframes are laid out to it with
whatever margin the part allows. Two keyframes 50 apart cannot both be supported; BEGLOGO's are 90
and 340 apart.

### 1.4 What "held" means, and why keyframes are chosen there

A keyframe is *held* when the frames either side of it are the same picture, because that is where
the reference's reproducibility is strongest and where a port's diff means something. A palette
fade over a held image is held in the index plane and moving in the palette: BEGLOGO's fade is
exactly that, and its frames are RGB-only because the palette changes while the indexed read
runs (D15's coherence rule) — 22 of the 28 frames of `out/beglogo-fade` came back RGB only, and
both fade keyframes did. A logo hold is held outright. Held is checked, not assumed: the sweep's
frames are hashed against each other (§3.3), and a shared index captured by two runs is the same
bytes or it is not.

### 1.5 Cycles

Everything in Part B is captured at `--cycles 30000`, the recipe B1 measured (D17). Phase 0's
`reference/check_a` was captured at 25000 and re-captured at 30000 in B1, so the tree has one
convention — but whether that matters was measured rather than assumed, because if a boot index
only means something next to the cycles that produced it, every capture would have to be compared
only with a capture of the same vintage:

**capturing the held index 900 at 25000 and at 30000, everything else identical, gives byte-
identical files** — `frame_000900.rgb` is `cbdc67eb…` and `frame_000900.idx` is `e10481c7…` in
both, and `cargo xtask compare` reports `verdict: identical`, 0.0000%, offset 0, exit 0. The
cycles change how long a capture takes in wall time and nothing about what it contains: the
counter counts emulator-rendered frames, and one frame is one emulated vertical refresh whatever
the cycles. `reference/check_a` (30000) can therefore be compared against any Part B capture.

### 1.6 Three things blocks 1 and 3 of B3 measured that the rules above do not cover

**A screen's rows are not always `width/4` bytes long, and the first pass of this phase shipped a
sheared plane because it assumed they were.** The read used to assemble the planes at `width/4` rows
unconditionally. That is right for the packed Mode X screen `reference/parts/beglogo`'s parts use and
wrong for ALKU, whose rows are 176 bytes: `ALKU/TWEAK.ASM:46` writes CRTC 0x13 = `0x5813` (88 units)
and `ALKU/MAIN.C:26` strides its own rows by 176 (`char (* far vvmem)[176]`, used at `:165-166`,
`:367-368`). `reference/parts/alku/frame_000900.idx` was the result — a plane whose picture began at
row 110 where `ALKU/MAIN.C:165-166` blits it at row 50, 34.8 % of covered pixels wrong — and it
**passed the coherence check**, because that frame's every pixel uses a palette entry that widens to
black. `indexed_matches_rgb` compares colours, so a collapsed palette leaves it nothing to see:
**that check cannot catch a stride error on a screen whose indices all map to one colour.**

**The tool now reads the stride, and refuses a screen it cannot describe** (fix round 1,
`xtask/src/oracle/dosbox.rs`'s `row_stride_bytes` and `capture_indexed`). The stride is the CRTC's
own arithmetic: a 256-colour screen draws a line every `scan_len * 8` bytes of the emulator's address
space (`vga_draw.cpp:1785-1792`), and a guest plane byte at offset `o` sits at four-byte group `o`
in that space (`vga_memory.cpp:129-135`), so one *plane* row is `scan_len * 2` bytes — 80 for mode
13h's offset 40, 176 for ALKU's 88. The **mode** is what says which units apply, and it comes from
the emulator's own name for the screen's colour depth (`webserver/video.cpp`'s
`video_mode.color_depth`) rather than from the chain-4 bit alone: only 256-colour screens are one
index a byte, so a 16-colour screen — whose pixels are packed as *bits* — is refused rather than
assembled at a plausible-looking wrong stride.

When `stride × height` is more than one plane holds, the guest's display wraps and the read cannot
say what it shows; that is ALKU (176 × 400 = 70,400 > 65,536). Its frames come back RGB-only with a
message that names the stride, and the message says **which** kind of thing happened — the refusal
is its own error, not a decode failure, so a capture log distinguishes a deliberate permanent
refusal from a broken read. **A missing `.idx` is honest; a sheared one is not.**

**And the tables above already agree:** `check_a`'s four frames, BEGLOGO's 430 and 900, U2A's 7300
and 7640 and PAM's 7900 were each re-read locally, and *the only row stride that reproduces their
RGB* is 80 in the planar model (and 320 in the linear one — the same screen). ALKU's 900 fits at
several because its palette collapses, which is the check's blind spot and not a reading. PAM's
`tw_opengraph` (`PAM/TWEAK.ASM:18-41`) is the same routine as ALKU's but stops before the CRTC
writes: it sets mode 13h, chain-4 off, 0x14 = 0 and 0x17 = `0xe3`, and never writes 0x13, so its
rows stay 80 and its held frame keeps its indices (§6.4). Reading a part's own mode set is still the
cheap way to know which it is — the difference is that the tool no longer needs to be told.

**A moving screen's session-to-session difference can be an exact integer pan, which no ±1 frame
support can bridge.** D17's moving case is a screen that is redrawn continuously: a session phase
difference of one frame leaves a **0.2–0.6 % residue**, because the one-frame shift is not uniform
across the picture. ALKU's scrolled picture is a different kind of moving screen — it advances its
window by one pixel per step and holds the same bytes in between (`ALKU/MAIN.C:7` `#define SCRLF 9`,
`MAIN.C:87` `for(f=60;a<320;...)` around `do_scroll(1)`), so the whole picture is identical between
sessions **after a whole number of pixels**: measured, the reference's 5859 against one session's is
**0 pixels differing of 95,700** after a 1-pixel shift and against another's after a 2-pixel shift,
while at offset 0 they differ by 10.975 % and 15.2797 % (§4.4). Two things follow, and B4/B5 should
expect them wherever a part pans:
- **The ±1 support rule buys nothing here.** The support window is ±1 *frame*, and the difference is
  a whole number of 9-frame scroll steps, so no choice of neighbouring index recovers the
  reference's picture.
- **A row is still worth having.** The reference frame is one exact picture; a port can be required
  to produce *that* picture, and the row says what the sessions can differ by instead of hiding it.

Held screens are untouched by either: 900, 1719 and 2719 below are byte-identical in every session
that took them.

**Consequence for the rows: a part whose `.idx` the tool refuses is compared in `mode: rgb`.** ALKU
is one — all four of its keyframes go through the RGB path, where the 6→8-bit widening
`(c*259+33)>>6` is load-bearing (`docs/decisions.md:422-423`) — and PAM's three animated frames are
RGB-only for D15's ordinary reason. `compare`'s report says which mode it used; nothing else needs
to be inferred from the file names.

**A part boundary is not always a geometry change, and §1.2 finds only mode changes.** U2A and PAM
are both 320×200 (the second part inherits the first's screen and its own `tw_opengraph` leaves it
in the same size), so *no* refusal marks where one ends and the other begins — the change is in the
content, and the sweeps' last refusals (7950, 8400) belong to the part after PAM, not to either
(§5.5, §6.5). B2's rule that "the refusal is the discovery" holds where consecutive parts differ in
geometry, which is the case B2 had; where they do not, the sheet is the discovery instead, and the
boundary is where the picture stops being the part's. What settled this one was a dense scan across
the suspected change plus a hash already known to belong to the next part: §6.1 found PAM's first
frame by matching a flat white frame against `wfade`'s own law, and §6.5 found where PAM ends by
matching a held logo against B2's BEGLOGO hash.

### 1.7 Four things block 2 (GLENZ, TUNNELI, TECHNO, PANICEND) measured

**The stride fix is exercised, and it comes out right.** GLENZ's vector screen is the 92-byte
plane row that `GLENZ/VIDINIT.ASM:306` writes (`dw 02e13h ; offset 92`), and the read assembles it
at that stride: `reference/parts/glenz/frame_001499.idx` reproduces its RGB **pixel for pixel** by
the check in `indexed_matches_rgb`, and its non-black rows are the RGB's — §7.3 has the
measurement. No row in this block needed a stride refusal; the fix that landed before it was
written for GLENZ and GLENZ is where it was confirmed.

**`.idx` presence is a read-*timing* property as well as a session one.** The same index with the
same RGB bytes can carry indices in one capture and not in another depending only on **when the
frame is read**. GLENZ 1500 is a held screen (1499 = 1500 = 1501 = 1600, one `sha256`) and it
carries indices when it is captured on its own — the read happens ~20 frames after the grab — and
none when it is the middle of a 3-index batch, whose read happens ~60 frames later: the palette
moves over that window (§7.3). The practical consequence for B4/B5: **list a keyframe that should
carry indices so that it is read immediately**, i.e. more than ~25 frames from its neighbours.

**A screen can be refused for its colour depth rather than its stride, and TECHNO is the case.**
TECHNO's 320×200 screen is not 256-colour at all: the part pokes the video-mode registers between
VGA 256-colour and EGA 16-colour to make its interference (`TECHNO/KOE.C:236-258`, the GC mode
register and the attribute controller's colour-select), and DOSBox re-decides the mode from those
registers (`vga.cpp:60-96`, `VGA_DetermineMode`: M_VGA needs GC mode bit 6). The indexed read is
therefore refused with `UnmodelledMode` — *not* the flat-colour blind spot of §1.6 — and every
TECHNO frame on that screen is RGB-only, which the comparison reports as `mode: rgb` (§9.4).

**A moving screen's phase is not reproducible in every session, and the difference can be much
larger than D17's 0.2–0.6 %.** Measured on this block, with the *held* frames of the same runs
byte-identical throughout: GLENZ's rotating polyhedron differs between two re-captures of the same
index by **18.66–34.32 %** of pixels; TUNNELI's dots by **8.07–10.18 %**; TECHNO's dithered
interference by **70.68–97.50 %**; and TECHNO's troll picture was already on screen at 8200 in one
session where three others show a flat black frame (a ≥150-frame difference in that section's
timeline). Two commands differing only in their `--out` directory, run back to back, gave
different bytes at 6300 (`3c2b0d8f…` against `c1460454…`). D17 measured three sessions identical
at 2125/2250/2400 — the same part this block keyframes — so the reproducibility D17 measured is
real but **conditional**, and these runs, on a host carrying other work, are the condition failing.
What it means for the rows below: a *held* frame is reproducible byte for byte in every session
seen; a *moving* frame is one exact picture that the oracle reproduces only when the two sessions
run the demo's animation on the same clock, and the row records the spread rather than pretending
to a tolerance.

**And the two-screen part's layout is `compare`'s own convention.** TECHNO's screens are two
geometries, so its capture is two captures: a directory with no `manifest.json` is resolved by
`compare` into every immediate subdirectory that has one, named by that subdirectory
(`xtask/src/compare.rs:401-443`, the `reference/check_a` layout). `reference/parts/techno/` holds
`interference/` (320×200) and `troll/` (320×400), and the partner side mirrors the names.

---

## 2. The index table

One row per part. `keyframes` are the boot indices in `reference/parts/<part>/`; `the partner`
is the scratch capture the re-capture comparison was run against, which also holds the support
frames and is not a reference. A part whose screens span two geometries gets one row per screen
(B3 step 2). Every row cites its own command and its per-frame hashes in the section below the
table.

| part | boot | geometry | keyframes | re-capture | re-capture result |
|---|---|---|---|---|---|
| BEGLOGO | `SECOND 2` | 320×400 Mode X | 430, 470, 560, 900 | `out/beglogo-recapture` | exit 0, `verdict: identical`, 0.0000%, offsets `{0: 4}` |
| ALKU | `SECOND 1` | 320×400 Mode X, 176-byte row stride; RGB only | 900, 1719, 2719, 5859 | `out/alku-recapture2` | exit 1, `mode: rgb` — 900/1719/2719 identical, 0.0000%, offset 0; 5859 differs by a 2-pixel pan, 15.2797% at offset 0 and 0.0000% once shifted (§4.4) |
| U2A | `SECOND 1` | 320×200 | 6500, 7300, 7640 | `out/u2a-recapture` | exit 0, `verdict: identical`, 0.0000%, offsets `{0: 3}` — including the moving keyframe, which landed in the same alignment group (§5.4) |
| PAM | `SECOND 1` | 320×200 | 7710, 7760, 7830, 7900 | `out/pam-recapture` | exit 0, `verdict: identical`, 0.0000%, offsets `{0: 4}` — every frame, moving and held alike (§6.4) |
| GLENZ | `SECOND 2` | 320×200, 92-byte plane row; 1499 carries `.idx` | 1499, 2300, 3000, 3400 | `out/glenz-recapture` | exit 1, `mode: mixed` — 1499 identical, 0.0000%, offset 0; 2300/3000/3400 differ by 34.3219/23.9516/29.0187% at offset -1, and by 18.6625/9.2000/16.3281% in the re-run (§7.4) |
| TUNNELI | `SECOND 2` | 320×200; RGB only | 3800, 4200, 4500, 4600 | `out/tunneli-recapture` | exit 1, `mode: rgb` — every frame differs by 8.2250–10.1672%, offsets `{0: 2, -1: 2}`; the re-run 8.0672–10.1812% at `{-1: 1, +1: 3}` (§8.4) |
| TECHNO — the 320×200 screen | `SECOND 2` | 320×200, 16-colour EGA mode; RGB only | 4900, 5000, 6300, 8100 | `out/techno-recapture` | exit 1, `mode: rgb` — 5000 identical, 0.0000%, offset 0; 4900/6300/8100 differ by 24.5938/72.8703/70.6844%; the re-run has 4900 and 5000 identical and 6300/8100 at 97.5047/71.7000% (§9.4) |
| TECHNO — the 320×400 screen | `SECOND 2` | 320×400 Mode X, 160-byte plane row; 8200 carries `.idx` | 8200, 8300, 8400 | `out/techno-recapture/troll` | exit 1, `mode: mixed` — 8200 and 8400 identical, 0.0000%, offset 0; 8300 (the fade-in) 99.9883% at offset -1 (§9.5) |
| PANICEND | `SECOND 2` | 320×200; 9000 carries `.idx` | 8792, 8800, 9000 | `out/panicend-recapture` | exit 0, `verdict: identical`, 0.0000%, offsets `{0: 3}` — including 8792, the collapse itself (§10.3) |

### Adding a row

1. Sweep and find the indices (§1.1, §1.2); pick the keyframes at held or structurally
   significant moments (§1.4).
2. Capture the reference: keyframes only, into `reference/parts/<part>/`, with the sheet going to
   `out/`.
3. Capture the partner into `out/<part>-recapture`, holding `K-1, K, K+1` around every keyframe
   that changes (§1.3).
4. `cargo xtask compare --reference reference/parts/<part> --rendered out/<part>-recapture
   --out out/<part>-recapture-diff`; record the exit code, the offsets and the percentage.
5. Fill the row in, and add the part's section below with the commands, per-frame hashes and what
   is on screen at each index.

A row that is a *moving* screen and exits 1 is re-run once (D19) — same alignment group and it is
byte-identical; different group again and the row records the offset and the residue, which is a
statement about the oracle rather than a failure of the reference. **No pixel tolerance is added
anywhere**: the alignment is searched, the pixels stay exact.

---

## 3. BEGLOGO

`SECOND 2`'s first part: the Future Crew logo, white, fading in over a held picture. 320×400
Mode X. What *this part* draws spans the demo's first graphics frame, 404, to the end of its fade
in 571–579 (§3.3) — about 2.5 seconds. The screen then stands still until GLENZ closes it down at
~1115 and changes mode in (1154, 1160] (§3.4, §3.5), so the 320×400 screen lasts roughly 750
frames — about 10.7 seconds — of which the first ~176 are this part's drawing. That distinction is
§3.5's, and it matters to Phase 2.

### 3.1 The keyframes, and why these

| index | what is on screen | frame files | `sha256` |
|---|---|---|---|
| 430 | the opening **white screen** — the whole frame is palette index 15, whose only palette entry is `(63,63,63)`, so every pixel is `255,255,255`. The screen is white from the part's first frame (404) until the fade's first step; 429 and 431 are byte-identical to it (§3.3). | `.rgb` + `.idx` (1 palette entry) | rgb `85789170…` |
| 470 | the **early fade** — the logo is visible and pale. 27 distinct colours; its mean absolute difference from the held picture is 18.448 of the white screen's 23.815, so it is ~23 % of the way through the white→picture palette ramp. | `.rgb` only | rgb `d0abd8cc…` |
| 560 | the **late fade** — the same picture ~93 % faded in (mean difference 1.718; 52 colours). The last sampled frame that still differs from the finished picture is 570 (§3.3). | `.rgb` only | rgb `05a130da…` |
| 900 | the **held logo** — the finished picture, 57 palette entries, held from 580 to 1110. | `.rgb` + `.idx` (57 palette entries) | rgb `cbdc67eb…`, idx `e10481c7…` |

430 and 900 are carried with indices because their palettes hold still across the indexed read;
470 and 560 are RGB-only because the fade moves the palette while the read runs, which is D15's
coherence rule refusing to record indices that do not widen back to the frame taken.

**What that means for the comparison is worth stating, because it is not the usual case.** With two
of the four frames RGB-only, this row's `compare` run reports `mode: mixed` and **two of the four
frames that judge Phase 2's exit criterion go through the RGB path** — where the DAC's 6→8-bit
widening, `(c*259+33)>>6`, is load-bearing rather than bypassed (`docs/decisions.md:422-423`; an
indexed comparison never depends on it, which is `compare.rs`'s reason for preferring indices). So
a port whose palette widening differed would show up on 470 and 560 and not on 430 and 900, and
Phase 2 should expect exactly that pair of readings rather than treating the row as four
indexed comparisons.

Full hashes and the commands:

| index | file | `sha256` |
|---|---|---|
| 430 | `frame_000430.rgb` | `857891706b1698637cb83d2e7a6e005c5af26909187cc1426195a088fd1e2561` |
| 430 | `frame_000430.idx` | `283babe8ef08242dfe2917f4eb6e7e8f050cc70891ccbda5c79ba851c01e130d` |
| 470 | `frame_000470.rgb` | `d0abd8cc06b5cfb4923d297effb54dfb4fcddd0a36950090f3a26875002f8e79` |
| 560 | `frame_000560.rgb` | `05a130da7b95c3527ed5699a72ebfb389f4b74881d9c7a079b8159908af8e76a` |
| 900 | `frame_000900.rgb` | `cbdc67eba977d5a58f09e5e14cfd04f04acdee716304ec3f15b830142cae1abc` |
| 900 | `frame_000900.idx` | `e10481c71b2ac341c8424a071e59c0a5c652c0517687f0c1f7d6817d5c004b08` |

```bash
# the reference: keyframes only
cargo xtask reference capture --arg 2 --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames 430,470,560,900 --out reference/parts/beglogo --sheet out/beglogo.png

# the partner: the same keyframes plus a support frame each side of each fade keyframe
cargo xtask reference capture --arg 2 --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames 430,469,470,471,559,560,561,900 --out out/beglogo-recapture
```

The sheet's four tiles, in the order the capture laid them out: **r0c0** frame 430 is white;
**r0c1** frame 470 shows the logo pale and washed out; **r1c0** frame 560 shows it nearly solid;
**r1c1** frame 900 is the finished picture. Thumbnail-sized, 560 and 900 are hard to tell apart —
which is what "93 % faded" means — and the two are not the same bytes (`05a130da…` against
`cbdc67eb…`), so the middle of the ramp is where the fade is visibly still moving.

### 3.2 The re-capture: exit 0, identical, offsets `{0: 4}`

```bash
cargo xtask compare --reference reference/parts/beglogo --rendered out/beglogo-recapture \
    --out out/beglogo-recapture-diff
```

Two boots, two sessions:

```
frame 430: 0.0000% differing (0/128000 px), max delta 0
frame 470: 0.0000% differing (0/128000 px), max delta 0
frame 560: 0.0000% differing (0/128000 px), max delta 0
frame 900: 0.0000% differing (0/128000 px), max delta 0
verdict: identical
exit: 0
```

`mode: mixed` (430 and 900 compared as indices, 470 and 560 as RGB), every offset 0, and all four
frame files `sha256`-identical between the two directories. The supports were not needed for this
pair — the two sessions landed in the same alignment group — but they are what makes the result
mean something if a later pair does not.

**This is the strong claim and it is worth saying what kind of screen it is a claim about.** 430
and 900 are held screens, and 470 and 560 are a palette fade over a held picture: the indices do
not move at all, only the DAC does. All four are byte-identical across sessions, and the *held
logo at 900 is byte-identical in five separate sessions* (the sweep, this reference, the partner,
and the 25000- and 30000-cycle probes of §1.5), which is the strongest form of the claim this
phase can make. What is *not* claimed here is that a screen the demo is continuously drawing
repeats this way — D17 measured that it does not, about half the time, and no part captured so
far has tested it.

### 3.3 The fade curve, measured

`out/beglogo-fade` captured 430–700 in tens. The white screen is `255,255,255` at 430 and 440
(one distinct colour); 450 is the first frame with more than one colour (5); of the samples, 570 is
the last that still differs from the finished picture (mean difference 0.156, max channel delta 5)
and 580 onwards is the held picture exactly — so the ramp's first changing frame is in 441–449 and
its last in 571–579, and nothing in the sampling narrows those two windows further.

| index | 440 | 450 | 470 | 500 | 520 | 550 | 570 | 580 |
|---|---|---|---|---|---|---|---|---|
| distinct colours | 1 | 5 | 27 | 43 | 48 | 51 | 52 | 52 |
| mean difference from the held picture | 23.815 | 22.0 | 18.448 | 12.927 | 9.236 | 3.767 | 0.156 | 0 |

That is a white→picture ramp of ~129 steps, one per frame — which is what `BEG/BEG.C:80-85` is:
`for(c=0;c<=128;c++)` around `dis_waitb()` and a `setpalarea`, one step per vertical blank. The
picture is blitted into video memory once, before the fade starts (`BEG/BEG.C:73-78`), and the
white opening is the `memset(vram,15,…)` and the all-white palette at `BEG/BEG.C:20-33` — so the
part's picture never moves; only the palette does.

Cross-run identity of the shared indices, which is what makes the two fade keyframes worth
choosing:

| index | sessions | result |
|---|---|---|
| 410, 429, 430, 431 (white) | 4 captures | byte-identical, one `sha256` |
| 470 | 3 captures | byte-identical |
| 560 | 3 captures | byte-identical |
| 690, 1040 (held) | 2 captures each | byte-identical |
| 1110 | 3 captures | **2 identical, 1 different** — its last scanline only (320 px of 128,000, max channel delta 255) |

1110 is the edge of the transition in §3.4: one session's collapse had already begun there. It is
the only shared index in the whole part that ever differed, and it is ±1 frame of the collapse's
start, not a difference in the picture.

### 3.4 Where the part ends, and what is in between

Sweeping up, `out/beglogo-boundary` captured 1110, 1130 and 1150 at 320×400 and was refused at
1170:

```
error: the display changed geometry between frames: frame 1170 is 320x200 where the capture
started at 320x400; capture one screen at a time
exit: 2
```

Narrowed from both sides with sparse probes: **1154 is still 320×400** (captured alone, `out/
b2-boundary-probe4`), **1160 and 1164 are already 320×200** (`out/b2-boundary-probe2`, a capture
that starts at the new geometry and so is not refused). Indices 1155 and 1158 could not be
captured at all, in three attempts between them — `the emulator reached frame 1156 before frame
1155 could be captured` and, twice, `…frame 1159 before frame 1158 could be captured` — so the
change lies in **(1154, 1160]**, with the window's middle un-grabbable rather than un-sampled.

Two things sit in that window and neither is a keyframe:

- **From ~1115 the picture collapses.** `out/beglogo-tail` (1000–1150 in tens) shows 1000–1110
  byte-identical to the held frame; 1120, 1130, 1140 and 1150 all differ from each other and from
  1110 (128,000, 122,880, 106,240 and 77,760 pixels between consecutive samples). The picture
  shrinks vertically toward a band around the middle of the screen — at 1130 it occupies rows
  46–377 with black above and below, at 1140 rows 106–348, at 1150 a flat grey band at rows
  191–307 — and the surround goes to black. It is a transition, not a held screen and not a
  freeze. A sheet shows it only from a sweep that stops short of the boundary; a sweep that
  crosses it is the one that gets refused.
- **The GLENZ side of the boundary is B3's row**, and its sweep starts inside GLENZ (1160, say)
  for the reason in §1.2.

### 3.5 What the collapse is: GLENZ's opening, not BEGLOGO's ending

This was left open in this task's first pass, with two `BEGLOGO.EXE` builds as the candidates, and
that was a dead end because the collapse is not BEGLOGO's at all. Fix round 1's review attributed
it, and the attribution is checkable in the sources and against these frames: **it is the first
thing the next part draws**, GLENZ's `zoomer2()` — a vertical close run before GLENZ's own mode
set, in the outgoing 320×400 geometry. `GLENZ/MAIN.C:288-303` is `dis_partstart()`, a wait on the
music (`while(!dis_exit() && dis_musplus()<-19);`), then `zoomer2()`, and only after it `mov ax,13h`
/ `int 10h`. Three things in the frames match the source line for line:

- **The band law is `zoomer2`'s.** It advances a quadratic — `zya++` then `zy += zya/4`
  (`GLENZ/ZOOMER.C:41-44`), so `zy(n) = n(n+1)/8` — and fills a top band through `zy` and a bottom
  band tracking `zy2 = 125*zy/260` (`ZOOMER.C:45-58`). Both edges follow it: the top of the
  non-black region is row 11 at 1120, 46 at 1130, 106 at 1140 and 191 at 1150, whose second
  difference is **exactly 1/4 row per frame²** — which is `n(n+1)/8`'s second difference, and the
  law the counter in `zy += zya/4` gives — and the bottom black edge at 1130, 1140 and 1150 is
  `399 - 125*zy/260` for the `zy` the top edge implies, within a row each time (378, 349, 308). The
  review that attributed this probed the band two frames apart and measured the top edge advancing
  14, 15, 16, …, 21 rows per two frames, which is the `(n+1)/2` that `zy += zya/4` gives.
- **The grey it converges to is `zoomer2`'s target.** The picture entries fade to a flat grey —
  `c` is `frame` capped at 32, `b = 32-c`, `pal2[a] = (pal1[a]*b + 30*c) >> 5`, applied with
  `setpalarea(pal2,0,128)` (`ZOOMER.C:59-68`) — so the target is six-bit **30**, which widens to
  **121**. The band measured at 1150 is exactly `(121,121,121)`.
- **Why the bands stay black while the picture greys.** `zoomer2` writes palette index **255**
  into both bands (`memset(v,255,80)`, `ZOOMER.C:48` and `:56`) and fades only entries 0–127, and
  `BEG/BEG.C:25-34` is what leaves entry 255 at `(0,0,0)` — white into 0–254, black into 255. The
  polarity is that combination and not a coincidence: a fade that touched 255 would take the bands
  grey with the picture.

**And a knock-on Phase 2 should be told rather than left to discover:** BEGLOGO's own drawing ends
with its fade. `BEG/BEG.C:86` is the last `setpalarea` and `BEG.C:87` closes `main()` — there is no
loop after it — so the 580–1110 hold is not BEGLOGO holding anything: it is the loader and GLENZ's
music wait leaving BEGLOGO's last screen up for ~530 frames, and the collapse then plays on top of
it. The leading reading is that **"reproduce index 900" means reproducing a hold that belongs to
the next part's music wait or to the loader, not to a BEGLOGO drawing loop**, and it is a reading
rather than a settled fact because nothing here measured the port. It is also robust to which
BEGLOGO build shipped: the other source in the tree would itself wait on a music code before
exiting (`ENDPIC/BEG.C:45-48`, capped at 300 frames), and 300 frames is less than the ~530-frame
hold, so GLENZ's wait is in the hold either way. What it does not change: 900 is still the right
picture and the right bytes, and no keyframe sits in the collapse.

---

## 4. ALKU

`SECOND 1`'s first part: three title cards over black, then a scrolled horizon picture, one
geometry (320×400) throughout. It is the longest part captured so far — from the demo's first
graphics frame to a mode change in (5950, 5960], about 79 seconds — and it is the phase's first
part with text, with a palette-driven card, and with a *moving* picture. The part is
`ALKU/MAIN.C`: each card is gated on the music (`while(dis_sync()<1..4)`, `MAIN.C:59,66,72,79`),
drawn once (`prtc`, `MAIN.C:61-63`) and then faded in, held and faded out
(`dofade(fade1,fade2); wait(300); dofade(fade2,fade1)`, `MAIN.C:64` — 64 steps in, 300 frames held,
64 out, one step a frame per `dofade`'s own loop, `MAIN.C:301-311`); after the fourth sync the picture
fades in over 128 frames (`cop_dofade=128`, `MAIN.C:83`) and `for(f=60;a<320;...)` scrolls it a step
at a time while five text batches are laid over it (`MAIN.C:87-135`).

### 4.1 The sweeps, and what the sheets showed

```
cargo xtask reference capture --arg 1 --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames 420,620,820,1020,1220,1420,1620,1820,2020,2220 --out out/b3-alku-sweep1 \
    --sheet out/b3-alku-sweep1.png
cargo xtask reference capture --arg 1 --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames 2420,2720,3020,3320,3620,3920,4220,4520,4820,5120 --out out/b3-alku-sweep2 \
    --sheet out/b3-alku-sweep2.png
```

Both at 320×400, both exit 0. `b3-alku-sweep1.png`'s tiles, in the order the capture printed them:
**r0c0–r1c1** (420, 620, 820, 1020, 1220, 1420) are plain black; **r1c2, r1c3** (1620, 1820) are the
card `A / Future Crew / Production`; **r2c0** (2020) is black again; **r2c1** (2220) is
`First Presented / at Assembly 93`. `b3-alku-sweep2.png`: **r0c0** (2420) the same
`First Presented` card, **r0c1** (2720) `in` over the **Dolby Surround logo**, **r0c2** (3020)
black, **r0c3** (3320) the landscape picture, faded in but already scrolling; **r1c0–r2c1**
(3620, 3920, 4220, 4520, 4820, 5120) the same picture alternately bare and with a text batch —
`Graphics / Marvel / Pixel` at 3920, `Music / Purple Motion / Skaven` at 4520,
`Code / Psi / Trug / Wildfire` at 5120.

So the sheet alone says: black → card → black → card → black → card → picture with texts. The
minimum over each card's position is what the sweep cannot give, and §4.2 measures it for the first
one. A third sweep (`out/b3-alku-sweep3`, 5420…8120 step 300) was **refused**, which is how the
part's end was found (§4.5); the two frames it took before the refusal (5420, 5720) show the picture
bare at 5420 and with the last text batch `Additional Design / Abyss / Gore` at 5720.

### 4.2 The card, measured frame by frame

`out/b3-alku-card1` took every 20th frame from 1500 to 2000 in one run (26 indices, exit 0, 35 s) —
one batch, so every frame in it is the frame that was asked for:

| index | 1500–1540 | 1560 | 1580 | 1600 | 1620–1900 | 1920 | 1940 | 1960 | 1980–2000 |
|---|---|---|---|---|---|---|---|---|---|
| whole-frame mean byte | 0 | 0.68 | 1.48 | 2.30 | 2.49 | 1.96 | 1.16 | 0.35 | 0 |
| `sha256` | `fec9afb5…` ×3 | `ea09bd61…` | `5ca8d435…` | `e56da741…` | `6a27ec50…` ×15 | `55bc5ebe…` | `0bb265c7…` | `3d577743…` | `fec9afb5…` ×2 |

That is the source's shape exactly: black until the first fade step lands in (1540, 1560], 64 steps
of fade-in, a hold the sampling shows as one `sha256` at all fifteen samples from 1620 to 1900
(280 frames sampled, no change), 64 steps out, black by 1980. The card's `6a27ec50…` is the same
bytes in five other captures (1620 and 1820 of the sweep, 1719–1721 of `out/b3-alku-probe`, the
reference and both partners), so the hold is a held screen in the sense §1.4 means it, measured
across six sessions rather than assumed from the source.

The same three-sample pattern holds for cards 2 and 3 where they were sampled (`out/b3-alku-probe`
took 2419/2420/2421 and 2719/2720/2721 and got one `sha256` per triple, `6a2de862…` and `5e3c3884…`),
and the black screen is one `sha256` (`fec9afb5…`) at 420, 620, 820, 1020, 1220, 1420 and 2020
(sweep 1), 1500, 1520, 1540, 1980 and 2000 (the card run), 3020 (sweep 2), 412 and 416
(`out/b3-alku-start`) and 899–901 (the probe) — **eighteen indices in five captures**, plus 900 in
the reference and both partners. The screen is flat black in the RGB — one colour, `(0,0,0)`, on
every pixel — and what the plane holds underneath that black is not something this oracle can read
(§1.6): the sheared index plane the first pass committed carried sixteen distinct indices under it,
all of whose palette entries widen to black, and that is how it passed a check it should not have.

### 4.3 The keyframes, and why these

| index | what is on screen | frame files | `sha256` |
|---|---|---|---|
| 900 | the **black screen** — every pixel is `(0,0,0)`. The part opens on it (the first graphics frame is 404, D17) and holds it until the first card's fade starts in (1540, 1560]. About 16 seconds of the part. | `.rgb` only | rgb `fec9afb5…` |
| 1719 | **card 1** — `A`, `Future Crew`, `Production` in the FONA font, mid-hold (fade in ended ~1615, fade out starts ~1905). | `.rgb` only | rgb `6a27ec50…` |
| 2719 | **card 3** — `in` over the Dolby Surround logo glyphs, mid-hold (`prtc`, `MAIN.C:74-76`). | `.rgb` only | rgb `5e3c3884…` |
| 5859 | the **scrolled picture**, bare of text, mid-step: it is the same bytes at 5859, 5860 and 5861 (measured, `out/b3-alku-probe`), so its ±1 neighbours are the same picture *in this session*. | `.rgb` only | rgb `be6f6c32…` |

**Nothing is keyframed in the first frames after the demo's first graphics frame**, and that is a
race rather than a line: `out/b3-alku-start`'s `--frames 404,405,406,410` was refused as a *miss*
(`the emulator reached frame 411 before frame 404 could be captured`), while the same run's
`--frames 412,416` captured both — and a later session's `411,412,416` took all three. The grab loop
is trying to land on indices within a frame or two of a mode change, so which of 404–412 it can catch
varies; 420 is the first index this task took *and* relied on (§4.2), and it is the black screen.

The three held keyframes are the part's three distinct held screens (black, a text card, the logo
card); card 2 at 2419 is deliberately *not* a keyframe — it is the same code path and the same kind
of screen as 1719 with different strings (`MAIN.C:68-69` against `:61-63`), so a fourth held card
would buy nothing a fourth slot should buy. The fourth slot goes to the picture, which is the only
screen this part has that moves.

**Why 5859 is mid-step and not at a step boundary**: 5859, 5860 and 5861 are one `sha256`, so the
step boundary is at 5862 or later. It is worth saying what that does and does not buy: it makes the
*supports* cheap, but it is not what makes the frame reproducible — across sessions this frame is
not (§4.4).

Each of 900, 1719 and 2719 carries **no support frames**, and that is a measurement rather than a
saving: 899/900/901, 1719/1720/1721 and 2719/2720/2721 are each byte-identical within their triple
(`out/b3-alku-probe`, one `sha256` each), so a support frame there is the same picture and could
only ever answer at offset 0 (§1.3). 5859's support window was captured anyway, because its ±1
identity is a property of where the step boundary happens to fall, not of the screen.

Full hashes of the five files in `reference/parts/alku/` — all `.rgb`, no `.idx`, because the fix
round 1 tool refuses this screen's stride (§1.6):

| index | file | `sha256` |
|---|---|---|
| 900 | `frame_000900.rgb` | `fec9afb531a8e036eba1d81651896e1b2c1f78b0234dbadc8e7549563f09407b` |
| 1719 | `frame_001719.rgb` | `6a27ec502abe606d39c25af842594335fb71d30e14a1f245e9492f3b4148cebe` |
| 2719 | `frame_002719.rgb` | `5e3c38846788c1cc4d8d23c65dfdb620779d9691b598e97ee2d79a3f4f0f0bd3` |
| 5859 | `frame_005859.rgb` | `be6f6c32e8a328a04a93c5e54129830f58c187c5deb4dbb399e1819f261240d2` |

The four RGBs above are byte-identical to the ones the first pass captured — the re-capture changed
nothing but the `.idx` that should never have been there, and this session landed in the reference
session's alignment group (§4.4).

### 4.4 The re-capture: three held frames identical, the picture a whole pan away

```bash
# the reference: keyframes only (re-captured once, after the stride fix of §1.6 removed a sheared
# index plane; all four RGBs came back byte-identical)
cargo xtask reference capture --arg 1 --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames 900,1719,2719,5859 --out reference/parts/alku --sheet out/alku.png

# the partner: the same keyframes plus a support frame each side of the picture keyframe
cargo xtask reference capture --arg 1 --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames 900,1719,2719,5858,5859,5860 --out out/alku-recapture

# the re-run (§2's rule for a moving row that exits 1), with one extra frame a scroll step earlier
cargo xtask reference capture --arg 1 --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames 900,1719,2719,5850,5858,5859,5860 --out out/alku-recapture2
```

First comparison (`out/alku-recapture`, exit 0 as a capture):

```
frame 900:  0.0000% differing (0/128000 px), max delta 0
frame 1719: 0.0000% differing (0/128000 px), max delta 0
frame 2719: 0.0000% differing (0/128000 px), max delta 0
frame 5859: 10.9750% differing (14048/128000 px), max delta 255
verdict: different, mode: rgb, offsets {"0": 4}
exit: 1
```

Re-run (`out/alku-recapture2`, exit 0 as a capture):

```
frame 900:  0.0000% differing (0/128000 px), max delta 0
frame 1719: 0.0000% differing (0/128000 px), max delta 0
frame 2719: 0.0000% differing (0/128000 px), max delta 0
frame 5859: 15.2797% differing (19558/128000 px), max delta 255
verdict: different, mode: rgb, offsets {"0": 4}
exit: 1
```

Both are `mode: rgb`: neither side of an ALKU comparison holds a `.idx` any more (§1.6), so all four
frames are judged on their pixels. `compare` pairs by index first (§1.3) and searches ±1 frame,
which is why the offsets read `{0: 4}` even though the picture differs.

**What the picture difference is, measured.** The three `sha256`s that share an index are not three
unrelated pictures: they are one picture at different pan positions. Comparing the reference's 5859
against each partner over the picture band (rows 50–349, where the part's window sits; rows 0–49 and
350–399 are black in all three frames):

| partner | differing at offset 0 | best shift | differing after the shift |
|---|---|---|---|
| `out/alku-recapture` 5858, 5859, 5860 (one `sha256`, `a77b4991…`) | 14,048 px | 1 px horizontally | **0 px of 95,700** |
| `out/alku-recapture2` 5858, 5859, 5860 (one `sha256`, `069964a4…`) | 19,558 px | 2 px horizontally | **0 px of 95,700** |

So the sessions differ by a whole number of pixels of the part's own horizontal pan, and by nothing
else: the four sessions that took an index in this window produced three distinct pictures — the
reference's (5859 in the reference and the probe, 5860 in `out/b3-alku-boundary1`), one 1 pixel
along, and one 2 pixels along. The part advances that
pan one pixel per scroll step (`MAIN.C:7`, `:87`), nine frames apart, which is why the difference is
*pixel-exact* and why the ±1 support window cannot reach it: the reference's picture is a whole
number of steps away in time, not one frame. `out/alku-recapture2`'s extra frame says the same thing
from the other side — its 5850 is `a77b4991…`, the *other* partner's picture, and 5858 is
`069964a4…`, one pixel along: within one session the pan moved once in those eight frames.

**This is the phase's first moving-row evidence, and it is a different animal from D17's.** D17's
moving case leaves a 0.2–0.6 % residue because its one-frame shift is not uniform across the
picture; here nothing is left over at all once a *whole* number of pixels is removed, because the
part's motion is a discrete step. `compare` exits 1 and is right to: the bytes at the reference
index really do differ between sessions. No tolerance was added.

The columns this puts Phase 2 in: alku is compared in `mode: rgb` — all four frames, because the
reference holds no `.idx` at all any more (§1.6) — where the DAC's widening rule is load-bearing. And
a port of ALKU has an unambiguous target for 900, 1719 and 2719 (reproduce the bytes) but not for
5859 (reproduce the reference's pan position, which the oracle's own sessions do not agree on).

### 4.5 Where the part ends, and the transition

`out/b3-alku-sweep3` (5420…8120 step 300) was refused at its third index:

```
error: the display changed geometry between frames: frame 6020 is 320x200 where the capture
started at 320x400; capture one screen at a time
exit: 2
```

Narrowed with sparse single-index probes from both sides — `out/b3-alku-boundary1`
(5800, 5860, 5920, 5980) captured three and was refused at **5980**; `out/b3-alku-boundary2`
(5940, 5950, 5960, 5970) captured two and was refused at **5960** — so the change lies in
**(5950, 5960]**. No index in this window refused as a *miss*; the misses here are at the other end
of the part, in the first frames after the demo's first graphics frame (404–410, §4.3).

The last frames of the 320×400 screen are a transition, not the picture: 5800, 5860, 5920 and 5940
all have the same row structure — rows 50–349 picture, black above and below — while **5950** has
rows 0–24 black, 25–174 picture, 175–184 black, 185–349 picture, 350–399 black: the window is being
split and shifted as the next part takes the screen. No keyframe is taken in it, and 5950 is the
last 320×400 index sampled under `out/b3-alku-boundary2`.

**The window is a measurement of where the change *can* be, not a fixed frame, and the next part's
own first frames sit inside it.** `out/b3-alku-sweep1` was refused at 6300 with the message
`frame 6300 is 320x200 where the capture started at 320x400`, so *that* session's 5960 was still
320×400 ALKU, while `out/b3-u2a-scan` (another session) captured 5960 as 320×200 U2A (§5.5). The
changeover is one to a few frames wide depending on the session, which is why no reference keyframe
belongs near either side of it. U2A is 320×200 and §5 is its row.

---

## 5. U2A

`SECOND 1`'s second part: a held "landing pad" scene, then a flying saucer over the same landscape,
then the landscape alone. 320×200, one geometry, from 5960 or 5961 to the end of its own drawing
between 7510 and 7560, its cleared background staying up until PAM's first frame at 7710 — about
22 seconds of drawing and 2.5 more of held screen. It is the first part in the phase whose screen the oracle reads
as *indices* for some of its frames and as RGB for others (§5.3), and the first whose keyframes
include moving ones that still re-capture byte-identically.

**This section was corrected by what §6 measured, and the correction is the interesting part.** The
first pass ended U2A at the mode change the sweeps hit at 7950 — but that refusal is **PAM's end,
not this part's**: PAM is 320×200 too (§6), so nothing in the *geometry* marks the U2A→PAM boundary
at all. U2A's own last frame is at ~7700 and PAM's first drawn frame at 7710, and the only reason it
is known is that the content changes there (§5.5). A fourth keyframe at 7760 was captured in the
first pass and is **not U2A's**: it is PAM's second animated frame, and it moved to §6's row. The
reference now holds three keyframes.

The source is `VISU/C/U2A.C`: it loads its scene data (`U2A.0xx`) and a background picture, waits
for the music to reach one position (`VISU/C/U2A.C:227-239`, a `for(;;)` around `int 0fch` with
`a>10 && b>46`), sets the screen up (`vid_init(3) ////// oversample x 4` at `:242`,
`vid_window(0L,319L,25L,174L,512L,…)` at `:247`), and then runs a four-buffer animation loop
(`cl[0..3]`, `copper2` at `:105-130`, the loop at `:259-…`) that ends when the music reaches a
second position (`a>11 && b>54`, `:276`) — `xit` then unwinds it.

### 5.1 The sweeps, and what the sheets showed

```
out/b3-u2a-sweep1   5960,6300,6600,6900,7200,7500,7800,8100,8400,8700   exit 2, refused at 6300
out/b3-u2a-sweep2   6300,7000,7700,8400,9100,9800                       exit 2, refused at 8400
out/b3-u2a-sweep3   5960,6200,6450,6700,6950,7200,7450,7700,7950,8100,8200   exit 2, refused at 7950
out/b3-u2a-scan     5960…7810 step 50 (38 indices)                      exit 0
```

The refusals are the boundary story (§5.4). The frames they left on disk, and the scan, give the
part's shape. Tiles, in the order each run laid them out:

- **sweep3's 6300** — the landscape with a **yellow wireframe grid** across its lower half: the
  "pad". **7000** and **7700** — the same landscape with no grid at all. (Sweep 2, another session,
  had 6300 as the pad and 7000 and 7700 plain, so the pad arrives and leaves at about the same
  place in both.)
- **the scan**, the part in one session, every 50th frame: 5960 plain landscape; **6010–6960 the
  pad, one `sha256` for all twenty samples** — a hold of at least 950 frames; 7010 plain; **7060,
  7110, 7160, 7210, 7260 a grey saucer that fills the frame and recedes**; 7310, 7360, 7410 a small
  blue-white craft shrinking against the sky; 7460, 7510 the plain landscape; 7560, 7610, 7660 the
  plain landscape again, one `sha256`; 7710 onwards the next part's animation (§6).

So the sheet says: pad hold → saucer → plain hold. The pad hold and the plain hold are where this
part's keyframes belong, and only the saucer moves. The last three of the scan's tiles were read as
a "second view" of U2A at the time; §6 shows they are PAM's, and the flat white frame among them is
PAM's first, not a U2A flash.

### 5.2 The keyframes, and why these

| index | what is on screen | frame files | `sha256` |
|---|---|---|---|
| 6500 | the **pad** — the landscape with the yellow wireframe grid, 950 frames into a hold that the scan shows running from 6010 to 6960. | `.rgb` only | rgb `11a66eb0…` |
| 7300 | the **saucer receding** — a small blue-white craft against the sky, mid-animation. | `.rgb` + `.idx` | rgb `0a7c2216…`, idx `b50f52f4…` |
| 7640 | the **plain landscape**, inside a hold that the scan shows from 7560 to 7660 and the dense scan of §6.1 from 7660 to 7700. | `.rgb` + `.idx` | rgb `0a68ffb9…`, idx `c51ba997…` |

One keyframe per screen the part has — the pad, the saucer, the cleared background — and two of the
three are held. 6500 is chosen deep inside the pad's 950-frame hold rather than at its edge (6010),
because the hold's first frames are within a few tens of frames of U2A's own mode change: an index
that close to it is not safe to put in a reference (§5.5).

Full hashes of the five files in `reference/parts/u2a/`:

| index | file | `sha256` |
|---|---|---|
| 6500 | `frame_006500.rgb` | `11a66eb08a14b3cf65528ff982c272791393af06d9afacd3a2e85eb1dc81cf7e` |
| 7300 | `frame_007300.idx` | `b50f52f4e5b2389f281cddce87031ec9da2fd90e50a30edd6df2d5115703bd24` |
| 7300 | `frame_007300.rgb` | `0a7c2216cb3a7964d0221af01cfa07e8aab5372f39d9ef4f097e270decf3d590` |
| 7640 | `frame_007640.idx` | `c51ba997410ec820a3695e18bfe0622a856e65c0163b60dc83b9addff5753f1c` |
| 7640 | `frame_007640.rgb` | `0a68ffb92f0e208a3eb34ace29fa1ea91a51bebfea3ec2733ca30cd5d8b5bbd6` |

### 5.3 Held or moving, measured — and which frames carry indices

```bash
# the reference: keyframes only (re-captured after §6 re-attributed 7760 to PAM)
cargo xtask reference capture --arg 1 --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames 6500,7300,7640 --out reference/parts/u2a --sheet out/u2a.png

# the partner: a support frame each side of every keyframe, so the held/moving question is measured
cargo xtask reference capture --arg 1 --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames 6499,6500,6501,7299,7300,7301,7639,7640,7641,7759,7760,7761 --out out/u2a-recapture
```

The partner's triples, hashed:

| keyframe | `K-1` | `K` | `K+1` | reading |
|---|---|---|---|---|
| 6500 | `11a66eb0…` | `11a66eb0…` | `11a66eb0…` | **held** — one `sha256` across the triple |
| 7300 | `9f5fe99c…` | `0a7c2216…` | `0a7c2216…` | **moving** — the frame changes between 7299 and 7300 |
| 7640 | `0a68ffb9…` | `0a68ffb9…` | `0a68ffb9…` | **held** — one `sha256` |

So 6500 and 7640 need no support frames, and 7300 needs its pair — and the supports were captured
for all three anyway (plus a triple around 7760, which is PAM's, §6.1), which is what turns this
table from an assumption into a measurement.

**Which frames carry `.idx` is not a property of held-or-moving, and not a property of one run
either — it is the session's.** The reference run kept indices at 7300 and 7640; the partner run kept
them at 7639, 7640 and 7641 and **not** at 7299–7301 — while 7300's RGB is byte-identical in both
(`0a7c2216…`, the re-capture above is 0.0000%). So the same frame on the same screen carries indices
in one session and not in another, and a support frame one index away can carry them where its
keyframe does not. (A later session of the reviewer's had 7300's indices again.) Across the scan the
plain-landscape frames at 7010 and 7560–7660 kept them and every sample of the pad (6010–6960) did
not. Reading the
planes from offset 0 (`xtask/src/oracle/dosbox.rs:618-629`) only matches the screen when the
*displayed* buffer is the one at offset 0, and this part rotates four animation buffers through its
display start (`VISU/C/U2A.C:105-130`, `vid_setswitch(-1,clr)` inside `copper2`); the measurements
above are consistent with that and are not evidence of motion. Either way the RGB is the frame that
was asked for: it is the same bytes in every session.

### 5.4 The re-capture: exit 0, identical, moving frames included

```
$ cargo xtask compare --reference reference/parts/u2a --rendered out/u2a-recapture \
      --out out/u2a-recapture-diff
frame 6500: 0.0000% differing (0/64000 px), max delta 0
frame 7300: 0.0000% differing (0/64000 px), max delta 0
frame 7640: 0.0000% differing (0/64000 px), max delta 0
verdict: identical, mode: mixed, offsets {"0": 3}
exit: 0
```

All three frames are byte-identical between the two directories, including the one that moves. **That
is the same-alignment-group case, and it should be read as one**: D17 measured that two sessions
land in the same group about half the time, and this pair did. It is the first time in the phase
that a moving screen has re-captured this way, and it is a stronger claim than ALKU's picture could
make (§4.4) — but it is not a claim that this part's animation is session-independent, and nothing
here tested the other group. The held pair (6500, 7640) is what carries the reproducibility;
the moving frame is what shows the same group exists for this part too.

**And the animation is coarse enough that the alignment has something to bite on**: 7300's picture
changed from the frame before it and held for the frame after, so a session one frame out of phase
would find its answer at offset ±1 — exactly the case the support frames exist for. The other
group was not observed, so this is a statement about the frames' spacing, not about a result.

### 5.5 Where the part ends — and how, when nothing changes geometry

```
out/b3-u2a-sweep3   (5940…, 320×200)      refused at 7950: "frame 7950 is 320x400 where the capture
                                          started at 320x200; capture one screen at a time"   exit 2
out/b3-u2a-sweep2   (6300…, 320×200)      refused at 8400: same refusal                         exit 2
out/b3-u2a-scan     (5960…7810, 320×200)  exit 0 — 7810 is 320×200
out/b3-pam-scan     (7660…7930, 320×200)  refused at 7930: the same 320x400 refusal              exit 2
out/b3-pam-probe2   (7950…8130, 320x400)  exit 0 — 7950 is 320×400
```

**Both refusals belong to PAM's end, not U2A's.** PAM is 320×200 as well (§6), so the U2A→PAM
boundary is invisible to the geometry check: the 320×400 change at (7920, 7930] is the *next* part
after PAM — BEGLOGO — taking the screen. U2A's own last drawn frames are the saucer sequence,
ending in the plain-landscape hold that the dense scan of §6.1 shows running from 7560 (at the
latest) to 7700, and PAM's first drawn frame is 7710. That boundary was found by **content**, not by
a refusal: the plain landscape stops and a flat white frame — PAM's, matching `wfade[0] = 63`'s
wash exactly (§6.1) — starts. §1.2's rule finds *mode changes*; where two consecutive parts share a
geometry it says nothing, and the sheet and the hashes are what say where the part changed.

Two things this section was wrong about in its first pass, now fixed: the 7950 refusal is not
U2A's end (§6 explains it), and 7760 — captured as U2A's fourth keyframe then — is PAM's second
animated frame, now in §6.1 with the rest of PAM's.

**U2A's own start is the same kind of fuzziness**, and it is worth stating because it constrains
where U2A's keyframes can go: 5960 is 320×200 plain landscape in `out/b3-u2a-scan` and still
320×400 ALKU in `out/b3-alku-sweep1`. An index in the first few tens of frames of a part can belong
to either side of a mode change depending on the session, so no reference keyframe should sit
there — which is why U2A's earliest keyframe is 6500, 490 frames clear of it.

---

## 6. PAM

`SECOND 1`'s third part: the animated intro picture that leads into BEGLOGO. **320×200 — the same
geometry as U2A** — from its first drawn frame in (7700, 7710] to the end of its animation at about
7920, when the screen changes to BEGLOGO's 320×400 logo in (7920, 7930]. About 3 seconds, the
shortest part in the phase so far. It is the part that produced the phase's first *same-geometry*
boundary (§5.5) and the phase's first frame set in which the RGB-only status of a moving frame is
D15's moving case and nothing else (§6.3).

The source is `PAM/OUTTAA.C`: it builds 63 washed variants of its palette
(`for(a=1;a<64;a++) … pal[a*768+b]=(63*a+(64-a)*pal[b])/64`, `PAM/OUTTAA.C:36-37`), waits for the
music to reach sync 10 (`while(dis_sync()<10&&!dis_exit())`, `:39`), sets the screen
(`tw_setpalette(&pal[768*63])` then `tw_opengraph()`, `:41-42`), and then plays a **45-step**
animation loop at four frames a step (`while(!dis_exit() && f++<45)` at `:46` with
`while(frame_count<4); frame_count=0;` at `:48`), each step unpacking one frame of the animation
into one of two buffers and flipping the display start between them (`ulosta_frame(0x0a400)` /
`tw_setstart(16384)` against `ulosta_frame(0x0a000)` / `tw_setstart(0)`, `:53-59`) and pointing the
copper at a palette from its fade table (`cop_pal=&pal[768*wfade[f]]; do_pal=1;`, `:62-63`).
`wfade[100] = {63,32,16,8,4,2,1,0,…0,1,2,4,6,9,14,20,28,37,46,56,63,63,…63}` (`:22-32`) — the
picture starts washed out to white, comes to full contrast in about 28 frames, and washes back out
to white at the end.

### 6.1 The dense scan, and what it measured

```
out/b3-pam-scan  7660,7670,…,7930 (28 indices, 320×200)   exit 2, refused at 7930
```

Every 10th frame from the end of U2A's hold to the mode change, hashed in the order it was taken:

| index | 7660–7700 | 7710 | 7720…7860 (15 samples) | 7870 | 7880–7920 |
|---|---|---|---|---|---|
| `sha256` | `0a68ffb9…` ×5 | `1fef46de…` | a different one at **every** sample | `644acaf3…` | `1fef46de…` ×5 |
| distinct colours | 40 | 1 | 63 → 163 → 76 | 24 | 1 |

Three things fall out of it, and all three are what the part's row rests on:

- **U2A's hold ends at 7700 and PAM's animation starts at 7710**, in the *same* geometry — the
  boundary is a content change, not a refusal (§5.5).
- **The flat `(251,251,251)` frame is PAM's, and it is exactly what `wfade` predicts.** Its colour is
  `251 = widen(62)` and six-bit `62` is what `(63*63 + pal)/64` gives for `wfade[f] = 63` — the wash
  the animation opens and closes on. It appears at **7710** in three separate sessions (§6.4), and
  as a hold of at least 40 frames at the end (7880–7920).
- **The animation changes every frame it was sampled at** — fifteen samples, fifteen hashes — which
  is why every keyframe in the middle of it is a moving one and why the RGB-only warning on those
  frames is D15's moving case, not §1.6's read-layout case.

The source's own count agrees with the measurement: 45 steps at four frames each is 180 frames, and
the animation spans (7705, 7920] — 215 frames, the difference being the time each `ulosta_frame`
takes beyond its four-frame wait.

### 6.2 The keyframes, and why these

| index | what is on screen | frame files | `sha256` |
|---|---|---|---|
| 7710 | **the washed opening frame** — a flat `(251,251,251)`, one colour: `wfade[f]=63` applied over the first animation frame. | `.rgb` only | rgb `1fef46de…` |
| 7760 | an **animation frame**, the landscape with a group of glowing objects above the horizon. | `.rgb` only | rgb `5d558081…` |
| 7830 | an **animation frame from the second half**, the same landscape with a wide band across it. | `.rgb` only | rgb `e1967ed2…` |
| 7900 | **the washed closing hold** — the same flat `(251,251,251)` as 7710, and the same bytes at 7880, 7890, 7910 and 7920. | `.rgb` + `.idx` | rgb `1fef46de…`, idx `4333e44e…` |

7710 and 7900 are the same bytes, and that is the part's shape rather than a coincidence: `wfade`
returns to 63 at both ends of the animation. They are the same picture with very different
reproducibility: 7900 is inside a hold of at least 40 frames (7880–7920, one `sha256`), while 7710's
wash lasts only about one loop step — 7709 and 7711 are the same bytes as 7710, and 7720 is already
the next animation frame — so an index one frame away from 7710 is still this picture but an index
ten frames away is not. Both are worth having in one row.

Full hashes of the five files in `reference/parts/pam/`:

| index | file | `sha256` |
|---|---|---|
| 7710 | `frame_007710.rgb` | `1fef46de229b96373d36639c7250856afa2d5b39c1401a2f4d55511cd597aee0` |
| 7760 | `frame_007760.rgb` | `5d558081c851f3f1b8a01ef777834f83757308a01207a23290d890ccdee77378` |
| 7830 | `frame_007830.rgb` | `e1967ed23c3709d84462489083c43fa9ac64e85bfbae3e1fae61a5e001f54f93` |
| 7900 | `frame_007900.idx` | `4333e44e17fb9ecad2fd1a623897ef6d12a59431e4f20ca6a993d10d07b056ef` |
| 7900 | `frame_007900.rgb` | `1fef46de229b96373d36639c7250856afa2d5b39c1401a2f4d55511cd597aee0` |

### 6.3 Held or moving, measured — and the spacing rule, twice

```bash
# the reference: keyframes only
cargo xtask reference capture --arg 1 --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames 7710,7760,7830,7900 --out reference/parts/pam --sheet out/pam.png

# the partner: a support frame each side of the two moving keyframes
cargo xtask reference capture --arg 1 --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames 7710,7759,7760,7761,7829,7830,7831,7900 --out out/pam-recapture
```

The partner's frames, hashed:

| keyframe | `K-1` | `K` | `K+1` | reading |
|---|---|---|---|---|
| 7710 | (7700 `0a68ffb9…`, U2A's, from the scan) | `1fef46de…` | (7720 `10bb3b95…`, from the scan) | **moving, but only just**: the wash holds for about one loop step, not one frame — 7709 and 7711 are byte-identical to 7710 (leftovers in the refused first attempt's directory, `out/pam-recapture/frame_00770{9,1}.rgb`), and the next animation frame is at 7720 |
| 7760 | `23fa5096…` | `5d558081…` | `5d558081…` | **moving** — changes between 7759 and 7760 |
| 7830 | `e1967ed2…` | `e1967ed2…` | `e1967ed2…` | **held** — one `sha256` across the triple |
| 7900 | (7890, 7910 `1fef46de…`, from the scan) | `1fef46de…` | (identical) | **held** — one `sha256` across the scan's samples |

**The first attempt at this partner was refused, and the refusal is §1.3's rule landing on a part
it was written for:**

```
error: the emulator reached frame 7773 before frame 7759 could be captured; the capture would not
       be of the requested frame
exit: 2
```

The list was `7709,7710,7711,7759,7760,7761,…`: a three-index batch at 7709–7711 whose reads cost
~57 rendered frames, with the next index only 48 frames behind it. The batch's reads ran past 7759
and the capture refused rather than take a later frame under an earlier name. The partner above is
the same list with the gap widened past the batch cost — and it is a useful reminder that the rule
binds *between* batches, not only before a distant single index.

### 6.4 The re-capture: exit 0, identical

```
$ cargo xtask compare --reference reference/parts/pam --rendered out/pam-recapture \
      --out out/pam-recapture-diff
frame 7710: 0.0000% differing (0/64000 px), max delta 0
frame 7760: 0.0000% differing (0/64000 px), max delta 0
frame 7830: 0.0000% differing (0/64000 px), max delta 0
frame 7900: 0.0000% differing (0/64000 px), max delta 0
verdict: identical, mode: mixed, offsets {"0": 4}
exit: 0
```

All four are byte-identical, and the same indices are identical in three sessions — the dense scan,
the reference and the partner — with 7710 and 7760 also agreeing with the earlier
`out/b3-u2a-scan` session. So this part's animation, unlike ALKU's pan, **is** session-stable at a
fixed index: the frames above are the same bytes in every session that took them (four sessions for
7710 and 7760, three for 7830 and 7900). That is worth saying explicitly because it is the opposite
of what a 45-step animation with an adaptive step *could* have done, and it is the reason PAM can
have moving keyframes at all.

**Only 7900 carries `.idx`.** The frame it holds is the washed white one, and the palette holds
still while the indices are read, so D15 keeps both; every animated frame of this part is RGB-only
because the screen really does move during the ~20-frame read — D15's moving case, not §1.6's
read-layout case. So three of the reference's four frames judge Phase 2 through the RGB path, where
the DAC's widening rule is load-bearing (`docs/decisions.md:422-423`).

### 6.5 Where the part ends, and what follows it

The screen becomes 320×400 in **(7920, 7930]** — `out/b3-pam-scan` captured 7920 as 320×200 and was
refused at 7930 — and what comes up is **BEGLOGO**: `out/b3-pam-probe2` (starting at 7950, 320×400,
exit 0) shows the logo fading in at 7950, 7980, 8010 and 8040 and then holding at 8070, 8100 and
8130, all three of those the **same `sha256`, `cbdc67eb…`, as B2's held BEGLOGO logo under
`SECOND 2`**. So the mode change is the next part's, and PAM's own screen is the 320×200 one it
inherits and keeps — which is exactly why §5.5's boundary is not in the geometry.

---

## 7. GLENZ

`SECOND 2`'s second part: the FC-logo band that BEGLOGO's picture collapses into, then a
translucent "glenz" polyhedron over a checkered floor, then a red polyhedron with the glenz object
moving through it. **320×200 throughout, and its rows are 92 bytes a plane** — the first part in
the tree whose `.idx` exercises the stride the §1.6 fix was written for. It is also the first part
this phase has captured whose *held* screen carries indices **only when the frame is read at the
right moment** (§7.3), and the first whose *moving* screens do not reproduce at all across
sessions (18.7–34.3 % of pixels, §7.4).

The part's extent, measured on the `SECOND 2` boot: the 320×200 screen is up from the change
§3.4/§3.5 puts in (1154, 1160] — `out/b3-glenz-sweep1`'s first frame, 1200, is already the FC-logo
band — and the part's own drawing ends with the red polyhedron leaving the frame at ~3650. A flat
black screen follows: 3680, 3690 and 3700 are all `ea0787f6…`, which is the same bytes as ALKU's
black at `SECOND 1`'s 420–2020 and PAM's neighbours (§4.2) — with no picture up, the parts agree.
TUNNELI's first tunnel ring is at 3710 (§7.5). So GLENZ is ~2500 frames of the `SECOND 2` boot,
≈36 s.

### 7.1 The sweeps, and what the sheets showed

```
out/b3-glenz-sweep1          1200,1600,…,6000 step 400              exit 0 (13 frames, 1 m 26 s)
out/b3-glenz-sweep2          3650,3700,…,4000 step 50, then 4200…5200 exit 0
out/b3-glenz-sweep3          1800,1900,2100,…,3500 step 100         exit 0
out/glenz-map1               1250,1300,1400,1450,1499,1620,1700,1800,2000,2300,2600,3000,3400  exit 0
out/b3-tunnel-boundaries     3680,…3750 step 10, then 4740,…4790     exit 0
```

Tiles, in the order each run laid them out. `sweep1`: **1200** black sky with a purple checkered
floor across the bottom; **1600** the same; **2000** a blue/grey translucent polyhedron above the
floor; **2400** the same object larger; **2800** a red polyhedron with the glenz object in front of
it; **3200** larger; **3600** the red object alone; **4000** the dot tunnel (TUNNELI, §8);
**4400** the tunnel; **4800** black; **5200–6000** TECHNO's interference (§9). `sweep3` fills in
between: the glenz object rises from the horizon (1800, 1900), stands alone against a pale grey
background (2300), and the red polyhedron takes over from ~2500. `glenz-map1` pins the opening:
**1250** floor + object at the horizon, **1300** the same picture over a **grey** background (a
different palette state), **1400 / 1450 / 1499** back to the black background and byte-identical to
each other, **1620** grey again, **1700** the object higher, **1800** black again.

### 7.2 The keyframes, and why these

| index | what is on screen | frame files | `sha256` |
|---|---|---|---|
| 1499 | the **held opening** — the checkered floor with the glenz object's top edge at the horizon; the bytes of 1400, 1450, 1499, 1500, 1501, 1600 and 1250 are all one picture. | `.rgb` + `.idx` | rgb `e45ab076…`, idx `75d561dc…` |
| 2300 | the **glenz polyhedron alone** — the translucent blue/grey object rotating against the pale background, the floor a thin band at the bottom. | `.rgb` only | rgb `9c42d079…` |
| 3000 | the **red polyhedron with the glenz object inside it** — mid-rotation. | `.rgb` only | rgb `71dd12f6…` |
| 3400 | the **red polyhedron with the glenz object at its edge**, the part's second-to-last screen before the object leaves the frame. | `.rgb` only | rgb `fab974e9…` |

One keyframe per screen the part has: the held opening, the glenz object, the red polyhedron with
the glenz object inside it, and the red polyhedron with the glenz object at its edge. 2300 and
3000 are ~700 frames apart and 3400 ~400 beyond them, so every one of them can be supported (§1.3).

Full hashes of the five files in `reference/parts/glenz/`:

| index | file | `sha256` |
|---|---|---|
| 1499 | `frame_001499.rgb` | `e45ab0764b860f50376d560cf5a4d0f0e7ecca935021780385e3735e865fc28e` |
| 1499 | `frame_001499.idx` | `75d561dc779e17cadaf4493b3e295378a9d02ed186049dc0edd754b3a130b32b` |
| 2300 | `frame_002300.rgb` | `9c42d079e2734f57533879a270db2dfd29b5f0cb3028d26c4641b29a6b299551` |
| 3000 | `frame_003000.rgb` | `71dd12f678be32c8785a6b562294de4822a77a18f4d910e857d0d51e7cf9a4ec` |
| 3400 | `frame_003400.rgb` | `fab974e99d7507ca56531c5751490b0309890448dfdb7168b0c9ca0a56e28c7f` |

### 7.3 Held or moving, measured — and where GLENZ's indices come from

The keyframe triple probe (`out/glenz-probe1`, 15 indices 1199…3401) and the map hashed:

| keyframe | neighbours | reading |
|---|---|---|
| 1499 | 1499 = 1500 = 1501, one `sha256` (and 1400, 1450, 1499 and 1600 are the same bytes in other runs) | **held** — but held in the *index* plane with a palette that cycles: 1300 and 1620 are the same picture over a grey background, and the palette returns |
| 2300 | 2099, 2100, 2101 are three different `sha256`; the partner's 2299/2300/2301 likewise | **moving** |
| 3000 | 2699/2700/2701 and 2999/3000/3001 are three each | **moving** |
| 3400 | 3399/3400/3401 are three | **moving** |

**Which is why 1499's `.idx` needs a second measurement, and the second measurement is the one
worth having.** 1499 carries `.idx` in the reference capture. The same screen does *not* when it is
read late:

- `out/glenz-stride-probe` captured `--frames 1500,1600` — two single-index reads — and **1500
  carries `.idx`** (rgb `e45ab076…`, idx `75d561dc…`, byte-identical to the reference's 1499).
- `out/glenz-probe1` captured `--frames …,1499,1500,1501,…` — one 3-index batch, whose reads run
  ~60 frames after the grabs — and **all three came back RGB-only**.

So GLENZ is §1.3's "a frame inside a batch is read at the end of that batch" case, on a *held*
screen whose palette moves. The screen is held; the DAC is not, and the indexed read is honest
about the second.

**And that `.idx` is what confirms the 92-byte stride.** `indexed_matches_rgb` compares every pixel
of the widened index plane with the RGB frame — not a sample, not a colour census — so
`frame_001499.idx` reproducing its RGB is the stride being right, and the independent check agrees:
both the RGB and the plane have non-black content in **rows 154–199 only**, 200 of 200 rows agree
on content-versus-black, and both have exactly **11,720** non-black pixels, across 8 distinct
indices. A read sheared by two bytes a row against 92 (the old `width/4` = 80) could not do that:
the second pass of this phase shipped exactly such a plane for ALKU and it passed the colour check
only because that palette collapsed (§1.6).

### 7.4 The re-capture: the held frame identical, three moving frames a whole phase away

```bash
# the reference: keyframes only
cargo xtask reference capture --arg 2 --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames 1499,2300,3000,3400 --out reference/parts/glenz --sheet out/glenz.png

# the partner: a support frame each side of the three moving keyframes
cargo xtask reference capture --arg 2 --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames 1499,2299,2300,2301,2999,3000,3001,3399,3400,3401 --out out/glenz-recapture

# the re-run (§2's rule for a moving row that exits 1)
cargo xtask reference capture --arg 2 --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames 1499,2299,2300,2301,2999,3000,3001,3399,3400,3401 --out out/glenz-recapture2
```

First comparison (`out/glenz-recapture`, exit 0 as a capture):

```
frame 1499: 0.0000% differing (0/64000 px), max delta 0
frame 2300: 34.3219% differing (21966/64000 px), max delta 154, rendered index 2299 (offset -1)
frame 3000: 23.9516% differing (15329/64000 px), max delta 162, rendered index 2999 (offset -1)
frame 3400: 29.0187% differing (18572/64000 px), max delta 162, rendered index 3399 (offset -1)
verdict: different, mode: mixed, offsets {"-1": 3, "0": 1}
exit: 1
```

Re-run (`out/glenz-recapture2`, exit 0 as a capture):

```
frame 1499: 0.0000% differing (0/64000 px), max delta 0
frame 2300: 18.6625% differing (11944/64000 px), max delta 146, rendered index 2299 (offset -1)
frame 3000: 9.2000% differing (5888/64000 px), max delta 162, rendered index 2999 (offset -1)
frame 3400: 16.3281% differing (10450/64000 px), max delta 162, rendered index 3399 (offset -1)
verdict: different, mode: mixed, offsets {"-1": 3, "0": 1}
exit: 1
```

**What kind of difference this is.** The held keyframe is byte-identical in both partners and in
every session that has taken it, which is the strong claim. The three moving keyframes are not: the
pictures are the *same scene at a different rotation* — `out/glenz-2300-compare.png` puts the
reference's 2300, the partner's and the map's side by side and the polyhedron's orientation differs
by more than a frame's worth — and the residue is 9–34 % of pixels rather than the 0.2–0.6 % D17
measured, because a rotating polyhedron moves most of its edges in one frame and the sessions here
are several frames apart in phase. **The ±1 support does not reach it**: both partners paired
every
moving keyframe at the same offset (-1), and the difference stayed large, which is the §4.4 shape
of result — a whole number of animation steps apart, not one frame — with the difference that
GLENZ's step is not a pixel pan and cannot be removed by shifting.

**This is the row's honest reading, and it is worth separating the two halves**: 1499 is a
reproducible reference; 2300, 3000 and 3400 are one exact picture each that the oracle reproduces
only in a session whose animation phase matches, which two of two re-captures did not. No
tolerance is added; the pixels stay exact and the row carries the numbers.

### 7.5 Where the part ends

`out/b3-tunnel-boundaries` took 3680…3750 at ten-frame steps:

| index | 3680 | 3690 | 3700 | 3710 | 3720 | 3730 | 3740 | 3750 |
|---|---|---|---|---|---|---|---|---|
| `sha256` | `ea0787f6…` | `ea0787f6…` | `ea0787f6…` | `d0f33e4a…` | `c75d7771…` | `36b6940b…` | `3654de07…` | `83a918e2…` |
| what | black | black | black | the tunnel's first small ring | ring | ring | ring | ring growing |

`sweep1`'s 3600 is the red polyhedron alone and 3650 (in `sweep2`) is the same object moving off
the top-left corner, so **GLENZ's own drawing ends at ~3650, a 30-frame flat black screen follows,
and TUNNELI's first tunnel frame is at 3710** — a content boundary with no geometry change, the
second one this phase has had to find that way (§1.6, §7.5 here). The black is `ea0787f6…`, the
same bytes as `out/b3-pam-probe2`'s black in §6 and ALKU's black in §4.2: with no picture on
screen the parts agree, which is why an index in the gap is not worth a keyframe.

---

## 8. TUNNELI

`SECOND 2`'s third part: the dot tunnel. 320×200, mode 13h (`TUNNELI/TUN10.PAS:110-115`,
`mov ax,$13` / `int $10`), from its first ring at 3710 to the last of its dots in the
(4700, 4770] window — about 1060 of its own frames by the source's own count
(`TUN10.PAS:3`, `const veke = 1060; { frame count to exit }`), ≈15 s.

**Every keyframe in this part is a moving screen, and the tunnel's motion is fast enough that a
one-frame phase difference moves most of its dots**: the re-captures differ by 8–10 % of pixels
where ALKU's pan left 0 % and D17's moving case left 0.2–0.6 % (§8.4). That is the row's
character: the reference is one exact frame of a fast animation, and the row says what sessions
can differ by instead of implying they cannot.

### 8.1 The sweeps, and what the sheets showed

```
out/b3-tunneli-sweep1  3730,3760,3800,3900,4000,4100,4200,4300,4400,4500,4600,4650,4700,4750  exit 0
out/b3-tunnel-boundaries  (the run above, §7.5) 4740,4750,4760,4770,4780,4790                 exit 0
```

`b3-tunneli-sweep1.png`'s tiles, in the order the capture laid them out: **3730** black;
**3760** a small dotted ellipse (the tunnel's mouth); **3800** the ring larger; **3900**, **4000**
the tunnel filling the frame; **4100**, **4200** the walls skewing; **4300**, **4400**, **4500**
the tunnel off-centre with its mouth moving up; **4600**, **4650** the dots thinning and the mouth
dark; **4700**, **4750** the last sparse dots. In the boundary run **4740, 4750 and 4760** are all
black while **4770** is TECHNO's first interference ring (§9.1) — i.e. in that session the tunnel
was over by 4740, while in the sweep's session 4750 still had dots: the tunnel's *end* drifts by
tens of frames between sessions, which is the same phenomenon as §7.4 and §9.5.

### 8.2 The keyframes, and why these

| index | what is on screen | frame files | `sha256` |
|---|---|---|---|
| 3800 | the **tunnel's mouth** — a bright ring of dots around a black centre, the part's opening shape. | `.rgb` only | rgb `e4d3646e…` |
| 4200 | the **full tunnel** — dots filling the frame, the mouth skewed. | `.rgb` only | rgb `d831c2c3…` |
| 4500 | the **tunnel off-centre**, its mouth up and to the left, walls sweeping across the frame. | `.rgb` only | rgb `7f0f47ef…` |
| 4600 | the **thin tunnel** — the same structure with the dots sparse and the mouth dark, the part's last distinctive shape before the screen goes black. | `.rgb` only | rgb `65c7d6de…` |

Four screens across the one effect it is: the mouth, the full tunnel, the off-centre tunnel, the
thinning one. **4700 was the obvious fourth and was dropped**: the partner capture for it was
refused as a miss (`the emulator reached frame 4702 before frame 4701 could be captured`,
`out/tunneli-recapture`'s first attempt), and the boundary run shows 4740/4750 black — the window
is both grab-hostile and session-dependent, which is not what a reference index should be.

Full hashes of the four files in `reference/parts/tunneli/` — all `.rgb`, no `.idx`: every one of
them is a moving screen, and the read 20 frames late never reproduces it (measured on 16 frames
across four captures, every one refused with `the screen moved while it was read`):

| index | file | `sha256` |
|---|---|---|
| 3800 | `frame_003800.rgb` | `e4d3646eb6453a18270eff03c63c13e56aacc3631e1fe0c7b025f9afe9c1b717` |
| 4200 | `frame_004200.rgb` | `d831c2c3e12402e754ec94b059d2a112f9c70a408122e9d073e06421a2ff6834` |
| 4500 | `frame_004500.rgb` | `7f0f47ef6a510ce1c233b68ee60b9950bc66a5fe9490f9df81739bf674604ba2` |
| 4600 | `frame_004600.rgb` | `65c7d6de46aeada6d0b2b58e8c2f0b0dac28289b9a62f95f2b0621b0c46ebcd9` |

### 8.3 Held or moving, measured

`out/tunneli-recapture`'s triples, hashed — three distinct `sha256` in every triple, so every
keyframe is moving and every one needs its supports:

| keyframe | `K-1` | `K` | `K+1` | reading |
|---|---|---|---|---|
| 3800 | `4cda6e68…` | `f65f2210…` | `b7d7f1a1…` | moving |
| 4200 | `efbd8db4…` | `ff72ebb3…` | `931ac683…` | moving |
| 4500 | `ac2b50d3…` | `a8547d71…` | `64440e1c…` | moving |
| 4600 | `0ea6f434…` | `7dfaf5a9…` | `67b1dc97…` | moving |

**The tunnel is not byte-identical between sessions even at indices where two sessions have
agreed.** `out/b3-glenz-sweep2` and `out/b3-tunneli-sweep1` took 3800, 3900 and 4000 and matched
exactly (`f861b1f2…`, `259c3bbb…`, `96056aba…`), while 4200 and 4400 differed between those same
two sessions by 0.0859 % and 0.1578 % of pixels — and then the reference capture's 3800
(`e4d3646e…`) matches neither of them. So the agreement above was two sessions landing in one
alignment group, not a property of the index, and it is the same caveat §5.4 attaches to U2A.

### 8.4 The re-capture: exit 1, a one-frame phase away, twice

```bash
cargo xtask reference capture --arg 2 --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames 3800,4200,4500,4600 --out reference/parts/tunneli --sheet out/tunneli.png
cargo xtask reference capture --arg 2 --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames 3799,3800,3801,4199,4200,4201,4499,4500,4501,4599,4600,4601 --out out/tunneli-recapture
cargo xtask reference capture --arg 2 --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames 3799,3800,3801,4199,4200,4201,4499,4500,4501,4599,4600,4601 --out out/tunneli-recapture2
```

`out/tunneli-recapture` (exit 0 as a capture):

```
frame 3800: 9.0328% differing (5781/64000 px), max delta 239
frame 4200: 10.1672% differing (6507/64000 px), max delta 239, rendered index 4199 (offset -1)
frame 4500: 8.2250% differing (5264/64000 px), max delta 239, rendered index 4499 (offset -1)
frame 4600: 9.9516% differing (6369/64000 px), max delta 239
verdict: different, mode: rgb, offsets {"0": 2, "-1": 2}
exit: 1
```

Re-run (`out/tunneli-recapture2`, exit 0 as a capture):

```
frame 3800: 8.1297% differing (5203/64000 px), max delta 239, rendered index 3799 (offset -1)
frame 4200: 10.1812% differing (6516/64000 px), max delta 239, rendered index 4201 (offset +1)
frame 4500: 8.0672% differing (5163/64000 px), max delta 239, rendered index 4501 (offset +1)
frame 4600: 10.0813% differing (6452/64000 px), max delta 239, rendered index 4601 (offset +1)
verdict: different, mode: rgb, offsets {"-1": 1, "1": 3}
exit: 1
```

**The two partners disagree with the reference in opposite directions** — the first pairs two
frames at -1 and two at 0, the second pairs three at +1 — which is the alignment group being
sampled either side, and the residue is the same size either way. `max delta 239` is the whole
story of the size: a dot that has moved is a dot at a different place, and the pixels where it was
go past every threshold. Since the screen is *made* of moving dots, the residue is a large
fraction of the non-black pixels (≈3,000–6,000 of 64,000) rather than the 0.2–0.6 % D17 measured
on a screen where most pixels hold still. **No tolerance is added and the row carries both runs.**

---

## 9. TECHNO

`SECOND 2`'s fourth part: the techno bars and the EGA interference, then the "panic picture"
(troll) that the next part collapses. **It is the first part this phase has captured whose screens
span two geometries** — a 320×200 screen from ~4770 to ~8150 and a 320×400 one from ~8160 to 8780 —
which is B3 step 2's case and is captured here as two captures with a row each (§1.7,
`reference/parts/techno/interference/` and `reference/parts/techno/troll/`).

**The 320×200 screen is not 256-colour, and that is the part's own doing.** TECHNO switches the
display between VGA 256-colour and EGA 16-colour to make the interference: it writes the graphics
controller's mode register and the attribute controller's colour-select
(`TECHNO/KOE.C:236-258`: `mov ax,0013h+256*20` on the CRTC, then the ATC's index 13h/20h pair),
and DOSBox re-decides the display mode from those registers rather than from the BIOS mode
(`vga.cpp:60-96`, `VGA_DetermineMode`, which returns `M_VGA` only while GC mode bit 6 is set).
Every TECHNO frame on that screen therefore comes back RGB-only with
`indexed capture failed (… not 256-colour …)`, which is a *different* refusal from ALKU's stride
one and from the moving-screen coherence one. The 320×400 screen is plain Mode X and does carry
indices.

### 9.1 The sweeps, and what the sheets showed

```
out/b3-glenz-sweep1   1200…6000 step 400   exit 0 — its 5200…6000 tiles are this part's screen (§7.1)
out/b3-tech-sweep1    5200…9200 step 400   exit 2, refused at 8400 (320×400 — the capture started 320×200)
out/b3-techno-sweep1  4850,4900,4950,5000,5050,5100,5200,5400,5600,5900,6300,6700,7100,7500,7800,8100   exit 0
out/b3-panic-sweep1   8000…8350 step 50    exit 2, refused at 8200 (320×400)
out/b3-panic-sweep2   8200,8250,8300,8350,8400,8450                                       exit 0 (320×400)
out/b3-panic-sweep3   8600,8700            exit 2, refused at 8800 (320×200)
out/b3-panic-sweep4   8800…9500 step 100   exit 0
out/b3-tunnel-boundaries 4740…4790 step 10 exit 0 — TECHNO's first rings are at 4770
out/techno-troll-map2 8200…8700 step 50    exit 0
out/techno-troll-probe1 8199,8200,8201,8399,8400,8401,8699,8700,8701,8779,8780,8781
                                           exit 2, refused as a miss at 8781
out/det-a, out/det-b  6300,8200            both exit 2 — the geometry refusal; see §1.7
```

Tiles. `b3-techno-sweep1.png`, in the order the capture laid them out: **4850**, **4900**
concentric rings (cyan on black); **4950** the same rings over a pale ground; **5000**, **5050**
flat white; **5100** the pink moiré; **5200**, **5400**, **5600**, **5900** the moiré mutating;
**6300** diagonal stripes; **6700** purple diamonds; **7100** purple triangles; **7500** black;
**7800** triangles again; **8100** a crosshatch. `b3-panic-sweep2.png`: **8200**, **8250**
black; **8300** the troll picture washed out to white; **8350–8450** the picture at full contrast.
`techno-troll-map2`: **8200**, **8250** black; **8300** the wash; **8350, 8400, 8450, 8500, 8600,
8700** the same bytes (`40044edc…`) — the picture is held.

The 320×400 section's shape, from the frames and their hashes: a flat black screen from the mode
change (~8160) to ~8290, a white-wash fade at ~8300, and the finished picture held from ~8350 to
8780, where the 320×200 screen returns (§10).

### 9.2 The keyframes, and why these

**The 320×200 screen** (`reference/parts/techno/interference/`):

| index | what is on screen | frame files | `sha256` |
|---|---|---|---|
| 4900 | the **interference rings** — concentric blue rings on black, the part's first drawn screen. | `.rgb` only | rgb `3dc29660…` |
| 5000 | the **white flash** — every pixel is `255,255,255`; the same bytes at 5050 (a hold of at least 50 frames). | `.rgb` only | rgb `20f3d62f…` |
| 6300 | the **diagonal-striped interference** — the moiré evolved into stripes across the frame. | `.rgb` only | rgb `3bec1847…` |
| 8100 | the **crosshatch**, the last screen before the 320×400 mode change. | `.rgb` only | rgb `a1b5d861…` |

**The 320×400 screen** (`reference/parts/techno/troll/`):

| index | what is on screen | frame files | `sha256` |
|---|---|---|---|
| 8200 | the **flat black screen** the mode change leaves — one colour, every pixel `(0,0,0)`. | `.rgb` + `.idx` | rgb `fec9afb5…`, idx `eec19bc6…` |
| 8300 | the **fade-in** — the troll picture washed to white, 43 colours, every pixel non-black. | `.rgb` only | rgb `17b9f92f…` |
| 8400 | the **held picture** — the troll at full contrast, 78 colours, 93,200 non-black pixels. | `.rgb` only | rgb `40044edc…` |

**The `.idx` at 8200 was looked at by hand, because §1.6 says a flat-coloured frame's indices are
the check's blind spot.** It is 128,000 bytes of zero — every index 0, which widens to the black
the RGB holds — so it is *consistent* and it is also *worthless as evidence*: it cannot say whether
the 160-byte row stride is right. That is recorded rather than counted as a validation.

Full hashes:

| file | `sha256` |
|---|---|
| `interference/frame_004900.rgb` | `3dc29660e9228e3d2de7c86129dc8ae2cbdd30c9f16d57408d2a077438f9f465` |
| `interference/frame_005000.rgb` | `20f3d62fced3ac8938933a1f3aca1dbe92b058808f88bdea2937ce5d788f9df8` |
| `interference/frame_006300.rgb` | `3bec1847cfb05e2b6876626c1fc0cde57b10fa320d8c0dff41e601c89bb05f25` |
| `interference/frame_008100.rgb` | `a1b5d8610d7c3c1646c240c52f3822f45f9e7e9b6c89b9048579be1d7394c4aa` |
| `troll/frame_008200.rgb` | `fec9afb531a8e036eba1d81651896e1b2c1f78b0234dbadc8e7549563f09407b` |
| `troll/frame_008200.idx` | `eec19bc6af0b3b6dfb97a08782c65f4bb3c3203e789a015d2008b0d689ad08be` |
| `troll/frame_008300.rgb` | `17b9f92fc65b414d0fd7f36b126bb12278363d27b4cbbfeb3f6cd47ee77964a1` |
| `troll/frame_008400.rgb` | `40044edca7d81510908734f1a3b15d670e724dff68d767348533e079f73fd81e` |

### 9.3 Held or moving, measured

| keyframe | neighbours | reading |
|---|---|---|
| 4900 | 4899/4900/4901 three different `sha256` (partner) | **moving** |
| 5000 | 5000 = 5050 (sweep); 4999/5000/5001 three different in the partner | **held across the two samples the sweep took, moving across a frame** — the flash is a palette step, and one frame away is a different colour |
| 6300 | 6299/6300/6301 three different | **moving** |
| 8100 | 8099/8100/8101 three different | **moving** |
| 8200 | 8199 = 8200 = 8201, one `sha256` (flat black) | **held** |
| 8300 | 8299/8300/8301 three different | **moving** |
| 8400 | 8399 = 8400 = 8401, one `sha256`; and the same bytes at 8350, 8450, 8500, 8600, 8700 | **held** |

### 9.4 The 320×200 re-capture

```bash
cargo xtask reference capture --arg 2 --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames 4900,5000,6300,8100 --out reference/parts/techno/interference
cargo xtask reference capture --arg 2 --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames 4899,4900,4901,4999,5000,5001,6299,6300,6301,8099,8100,8101 \
    --out out/techno-recapture/interference
cargo xtask reference capture --arg 2 --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames 4899,4900,4901,4999,5000,5001,6299,6300,6301,8099,8100,8101 \
    --out out/techno-recapture2/interference          # the re-run
```

`out/techno-recapture` (exit 0 as a capture):

```
frame 4900: 24.5938% differing (15740/64000 px), max delta 138, rendered index 4899 (offset -1)
frame 5000:  0.0000% differing (0/64000 px), max delta 0
frame 6300: 72.8703% differing (46637/64000 px), max delta 206, rendered index 6301 (offset +1)
frame 8100: 70.6844% differing (45238/64000 px), max delta 206, rendered index 8099 (offset -1)
verdict: different, mode: rgb, offsets {"-1": 2, "0": 1, "1": 1}
exit: 1
```

Re-run (`out/techno-recapture2`, exit 0 as a capture):

```
frame 4900:  0.0000% differing (0/64000 px), max delta 0
frame 5000:  0.0000% differing (0/64000 px), max delta 0
frame 6300: 97.5047% differing (62403/64000 px), max delta 255, rendered index 6299 (offset -1)
frame 8100: 71.7000% differing (45888/64000 px), max delta 206
verdict: different, mode: rgb, offsets {"-1": 1, "0": 3}
exit: 1
```

**Which frames reproduce is not a property of the frame's kind but of the index's timing**, and the
two runs show it both ways: 5000 (the flat white hold) is identical in both; 4900 is 24.5938 % away
in one run and identical in the other; 6300 and 8100 are 70–97 % away in both. Those two screens
are a fine dither, so a phase difference of a frame or two repaints most of the frame — the same
mechanism as §7.4 with a screen that has no structure to survive it.

### 9.5 The 320×400 re-capture, and the one session that disagreed

```bash
cargo xtask reference capture --arg 2 --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames 8200,8300,8400 --out reference/parts/techno/troll --sheet out/techno-troll.png
cargo xtask reference capture --arg 2 --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames 8200,8299,8300,8301,8400 --out out/techno-recapture/troll
```

```
troll frame 8200:  0.0000% differing (0/128000 px), max delta 0
troll frame 8300: 99.9883% differing (127985/128000 px), max delta 206, rendered index 8299 (offset -1)
troll frame 8400:  0.0000% differing (0/128000 px), max delta 0
verdict: different, mode: mixed, offsets {"-1": 1, "0": 2}
exit: 1
```

Both held frames are byte-identical between the two sessions; only the fade-in differs, and it
differs by essentially every pixel, because at that index one session has the wash at one
brightness and the other at another.

**The first capture of this reference was a session whose timeline did not match, and it was
caught and re-taken.** In it, the troll picture was *already on screen* at 8200 and 8300 — 78
colours, 93,200 non-black pixels, i.e. the finished picture — where the partner, `b3-panic-sweep2`,
`techno-troll-map2` and the re-captured reference all show flat black at 8200, the wash at 8300 and
the picture from 8350. That is a ≥150-frame difference in this section's timeline between sessions,
the largest instance of §1.7's finding in the block, and it is named here rather than deleted: the
reference now in the tree is the one that agrees with three other sessions, and the outlier is one
command's worth of evidence that a *single* capture is not enough to claim an index means one
screen.

---

## 10. PANICEND

`SECOND 2`'s fifth part: the fake ending — TECHNO's picture collapsing to a line and then to a
single dot on black. 320×200, from the geometry change at ~8790, whose first visible effect is the
320×400 screen's 400 lines being halved back to 200 (the `SHUTDOWN.C` asm writes CRTC 09h = 41h,
one more scanline repeat), to the next part's first drawn frame at ~9230. **It is the shortest
part in the block and the one whose screens reproduce best.**

The source is `PANIC/SHUTDOWN.C`: it takes the palette the previous part left (`getpal`,
`:43-48`), then `shutdown()` writes a line at row 200 of a 640-wide layout from both edges inward
(`for(x=20;x<=160;x+=3) … tw_putpixel(x,200,0) …`, `:116-131`), leaves a pixel at (160, 200), and
pulses its palette for 60 frames (`:132-138`). The panel that shows only a dot is that last loop
and the `sleep(1)` after it.

### 10.1 The sweeps, and what the sheets showed

```
out/b3-techpanic-sweep3 8600,8700,8800,8900,9000,9100,9200,9300,9400,9500   exit 2, refused at 8800 (320×200 → the capture started 320×400)
out/b3-panic-sweep4     8800,8900,9000,9100,9200,9300,9400,9500            exit 0
out/panic-probe1        8791…8799 step 1, then 8999,9000,9001              exit 2, refused as a miss at 8999
out/b3-panic-mnt-boundary 9200…9300 step 10                                exit 0
```

Tiles: `b3-panic-sweep4.png` — **8800** a horizontal line across a black frame with a dot at its
right end; **8900** black; **9000**, **9100**, **9200** black; **9300** the next part's picture
(foliage) partly drawn; **9400**, **9500** that picture at full contrast. The pixel counts behind
those thumbnails, which is what the thumbnails cannot show: at 8800 the line is 234 non-black
pixels in row 100 from x = 40 to x = 276, and **8900, 9000, 9100 and 9200 are a single non-black
pixel at (160, 100)** — the dot. `panic-probe1`'s 8791–8799 show the same thing at one-frame
resolution: 8791–8794 are a horizontal *band* (the collapsed picture, still ~10 rows tall) and
8795–8799 are the thin line.

### 10.2 The keyframes, and why these

| index | what is on screen | frame files | `sha256` |
|---|---|---|---|
| 8792 | the **collapse** — the previous part's picture squashed into a horizontal band around row 100, ~10 rows tall, with the black screen above and below. | `.rgb` only | rgb `c9bf5114…` |
| 8800 | the **line and the dot** — a 234-pixel line in row 100 with the nucleus at (160, 100). | `.rgb` only | rgb `bb38c96b…` |
| 9000 | the **dot alone** — one non-black pixel at (160, 100), its palette pulsing; the same bytes at 9100 and 9200. | `.rgb` + `.idx` | rgb `b412a265…`, idx `9d7106ca…` |

Three keyframes, one per screen the part draws: the collapse, the line, the held dot. The dot's
index is the part's hold — 8900 to 9230 is one frame repeated at least 340 times — and 9000 is
deep inside it rather than at its edge.

Full hashes of the five files in `reference/parts/panicend/`:

| index | file | `sha256` |
|---|---|---|
| 8792 | `frame_008792.rgb` | `c9bf5114b3e70b841884ae7a075684ee2757983760355a7d8a8aedb914ae9484` |
| 8800 | `frame_008800.rgb` | `bb38c96bfd2d1da8ae99a5e1569d5c2e99431b3e005a8ca0b5db989a173a3665` |
| 9000 | `frame_009000.rgb` | `b412a26518f5e0281790d05c16ed0c7644b7382c5debb8f16a9437b7a6aa84e7` |
| 9000 | `frame_009000.idx` | `9d7106cabab6980fa3e0f574a5fe2de7a3f8370fd5f336fb2bdaebb48b842c06` |

**9000's `.idx` was checked by hand, and it is not a flat-colour case**: it holds exactly **one**
non-zero index, `1`, at (160, 100) — the dot — against 63,999 zeros, which is what the RGB shows.
So the panel is honest, and the one pixel is the only thing in it that could have caught a stride
error.

### 10.3 Held or moving, measured — and the re-capture is identical, collapse included

| keyframe | neighbours | reading |
|---|---|---|
| 8792 | 8791 / 8792 / 8793 three different `sha256` (in both `panic-probe1` and the partner) | **moving** — the band is still shrinking |
| 8800 | 8799 = 8800 = 8801, one `sha256` (partner), and 8795–8799 are the same bytes | **held** |
| 9000 | 9000 = 9100 = 9200, one `sha256` (sweep) | **held** |

```bash
cargo xtask reference capture --arg 2 --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames 8792,8800,9000 --out reference/parts/panicend --sheet out/panicend.png
cargo xtask reference capture --arg 2 --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames 8791,8792,8793,8799,8800,9000 --out out/panicend-recapture
```

```
$ cargo xtask compare --reference reference/parts/panicend --rendered out/panicend-recapture \
      --out out/panicend-recapture-diff
frame 8792: 0.0000% differing (0/64000 px), max delta 0
frame 8800: 0.0000% differing (0/64000 px), max delta 0
frame 9000: 0.0000% differing (0/64000 px), max delta 0
verdict: identical, mode: mixed, offsets {"0": 3}
exit: 0
```

**All three are byte-identical, including the moving one.** 8792 is a frame in the middle of the
collapse and it re-captured exactly, which is the opposite of GLENZ's and TUNNELI's moving frames
in the same block — evidence that the session-to-session difference §1.7 records is a property of
*some* animated screens, not of "moving" as a category: PANICEND's collapse is driven by
`dis_waitb()` counts (one step a frame, no per-iteration variability), while GLENZ's rotation and
TUNNELI's tunnel advance by however many vblanks the host let pass. Two of the three frames are
held outright, and the row's reproducibility rests on them and on 8792 together.

### 10.4 Where the part ends

`out/b3-panic-mnt-boundary` took 9200…9300 in tens: **9200, 9210, 9220** are the dot's black,
**9230** is the next part's picture arriving dark from the top-right, and 9240 onward is that
picture fading in — the same bytes in the reference run and in the earlier `b3-panic-sweep4` at
9300. The picture is `FOREST/HILLBACK.CLX`, an MNTSCRL asset (the MNTSCRL contact sheet renders
it), so **PANICEND's own last drawn frame is the dot, the loader's music wait holds it, and the
next part's first drawn frame is ~9230**. There is no geometry refusal at that boundary — both
screens are 320×200 — so it was found by content, the third such boundary in this block.
