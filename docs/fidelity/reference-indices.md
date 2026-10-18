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

**§1.12 is the phase's non-determinism policy and a later phase should read it before its first
comparison**: what an index means and what makes a capture valid, the ±1 alignment search and why
spec §8.4's structural-similarity fallback was not needed, how a part that does not reproduce is
handled, and the keyframes and mechanisms this phase could **not** settle (the ones that will show a
Phase 2 residual that is the oracle's, not the port's).

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

### 1.7 Five things block 2 (GLENZ, TUNNELI, TECHNO, PANICEND) measured

**The stride fix is exercised, and it comes out right.** GLENZ's vector screen is the 92-byte
plane row that `GLENZ/VIDINIT.ASM:306` writes (`dw 02e13h ; offset 92`), and the read assembles it
at that stride: `reference/parts/glenz/frame_001499.idx` reproduces its RGB **pixel for pixel** by
the check in `indexed_matches_rgb`, and its non-black rows are the RGB's — §7.3 has the
measurement. The fix that landed before this dispatch was written for GLENZ and GLENZ is where it
was confirmed.

**The offset register is read as two port operations with the guest running in between, so it can
name another register's value, and nine frames of this block show it.** `out/b3-techpanic-boundary.log`
holds **nine `PlaneTooSmall` refusals** — `this screen's rows are 320 bytes and it is 400 rows tall,
so it needs 128000 bytes a plane; one plane holds 65536` — on 8700…8780, and in two other sessions
the *same* 8700 is refused only for motion, with byte-identical RGB (`40044edc…`), so the displayed
layout cannot really differ: the tool's `crtc_offset_register()` writes the index to 3d4h and reads
3d5h in a separate request, and TECHNO writes the CRTC index several times a frame, so the read can
return another register's value — and the value it returned is one the guest was writing: **160 =
0xa0 is `plv * 0x20` for `plv = 5`, one of the eight values `TECHNO/KOE.C:385-389`, `:445-449` and
`:573-577` write to CRTC register 0ch (start address low) in a rotating loop.** The tool read 0ch's
value as if it were 13h's. This is a tool-side question, not a
reference one — every frame that *did* carry indices passed the pixel-exact check, so nothing in the
tree is wrong — but it is why **a stride refusal in a log is not by itself evidence that the screen
is unreadable**, and it is the only stride refusal this block produced (§9.6).

**`.idx` presence is not predictable from the frame list, and the rule this section first
stated for it was wrong.** The first version of this section said a held keyframe keeps its indices
when it is read immediately and loses them inside a batch. **That is withdrawn**: the reviewer of
this block checked `out/glenz-probe1`, the very run the claim cited, and it holds `.idx` for all
three of 1499/1500/1501, byte-identical to the reference's (`75d561dc…`) — the batch kept them. What
the artefacts do support is narrower: **on GLENZ every *moving* keyframe came back RGB-only, and the
held picture carried indices in four captures and not in one** (`out/glenz-map1`'s 1250 has exactly
the bytes of 1499 and no `.idx`, while 1400, 1450 and 1499 in the same run have it). So the
determinant is not the read's position in the frame list, it has not been identified, and B4/B5
should capture, look at what came back, and not space indices on the strength of a rule.

**A screen can be refused for its colour depth rather than its stride, and TECHNO is the case.**
TECHNO's 320×200 screen is not 256-colour at all: the part pokes the video-mode registers between
VGA 256-colour and EGA 16-colour to make its interference (`TECHNO/KOE.C:236-258`, the GC mode
register and the attribute controller's colour-select), and DOSBox re-decides the mode from those
registers (`vga.cpp:60-96`, `VGA_DetermineMode`: M_VGA needs GC mode bit 6). The indexed read is
therefore refused with `UnmodelledMode` — *not* the flat-colour blind spot of §1.6 — and every
TECHNO frame on that screen is RGB-only, which the comparison reports as `mode: rgb` (§9.4).

**A moving screen's phase differs between sessions, and the *percentage* a comparison reports is a
lower bound on that difference, not a measure of it.** Three things measured on this block, held
frames byte-identical throughout:

- **The disagreement at a fixed index is a handful of frames, not tens, once the screen's own
  per-frame rate is divided out.** GLENZ's polyhedron moves 10.64–25.43 % of its pixels a frame and
  the same-index differences are 22.83–34.67 %, i.e. ≈1.5–2.5 frames; TUNNELI's dots move
  2.05–10.69 % a frame and its same-index differences are 8.23–10.29 %, i.e. ≈1–4 frames. The raw
  percentages looked like a chasm only because a rotating polyhedron and a dot tunnel each repaint
  most of what moves in a single frame.
- **Some screens saturate, and there the percentage cannot witness anything.** TECHNO's dithered
  interference moves **89.10–98.69 % of pixels in one frame** (measured between adjacent indices
  inside one capture: 8099→8100 is 98.69 %, 8100→8101 89.10 %, 6300→6301 98.04 %); so the
  re-capture's 97.5047 % there is a *one-frame* statement, and 70.6844 % is smaller than one frame.
- **The offset `compare` reports is the window it searched (±1 frame), not the gap between the
  sessions.** Some screens are quantised into a few discrete results rather than drifting smoothly —
  TUNNELI 3800 has three distinct pictures across five sessions of this block with three byte-identical
  (`f861b1f2…` ×3), and the review of this block measured five of seven byte-identical at 3800 with
  the others ≈0, ≈8 and ≈20–30 frames apart — and the error is corrected somewhere between, so it is
  not a fixed per-session offset either.

What a *music-gated* event does is a different size: TUNNELI's tunnel end and the TECHNO→PANICEND
change sit at different indices by tens of frames between sessions (§8.5, §10.1), because those are
gated on the music position, not on a frame count. One session also showed TECHNO's troll picture
already up at 8200 where three others show flat black — a ≥150-frame difference in that section's
timeline — and, per the review of this block, that must be read as **one session's observation whose
frames no longer exist** (one command's re-capture replaced them; what survives is the `sha256` and
colour census taken at the time, §9.5).

**What the phase error is and is not, measured after the fact.** The demo's own frame clock is the
VGA retrace — `MAIN/COPPER.ASM:300-320` reprograms the PIT, polls 3DAh for the retrace edge and
increments `copperframecount`, which `DIS/DISINT.ASM:161-181` hands back from `waitb` — so the
guest's frames and the capture's frame counter are one clock, and the music is the only *other*
clock in the machine. It was measured directly, by reading the music player's frame counter out of
guest memory — the 16-bit word at **physical address `0x105B0E`**, in the DOS-extender's data, which
advances **0.7424 times a frame** (the three sessions below measure 0.742016 / 0.742012 / 0.741909
per frame, i.e. ~52 Hz, where the video clock
advances 1.000 and the DOS tick 0.26): at video frame 900 it reads **371, 373, 374** in three
sessions and at ~8103 it reads **5717, 5716, 5715** — a spread of **~1 music frame, ≈1.3 video
frames**, with the rate identical to one part in ten thousand (0.742016 / 0.742012 / 0.741909).
(Those two numbers — the address and the rate — are what makes the mechanism checkable; B3 block 2's
report stated the counter's addresses dump-relative and the physical address survived only in the
phase's git-ignored ledger, so they are recorded here before that ledger goes.)

So the common few-frames spread has a mechanism and a size: a music-gated part *starts* a frame or
two either side, and a part whose pictures then advance by a frame count inherits that as a constant
offset — which is what GLENZ's ≈1.5–2.5 and TUNNELI's ≈1–4 frames are. It also means a
music-gated *boundary* is pinned to the music and does not wander: the five mode changes of this
block sit within 4 frames across four sessions (BEGLOGO 408/409/406/406, GLENZ 1163/1161/1162/1162 —
the capture probes of §3.4 bound that same change as **(1154, 1160]**, so the two readings' bounds
differ by about two frames, no more than the one-to-two-frame slop between the index a capture asks
for and the frame it actually grabs (D19); the instruments do not disagree
— TECHNO's 16-colour 4799/4799/4797/4801, TECHNO's 320×400 8194/8195/8196/8193, PANICEND
8784/8784/8784/8784), and the tunnel→TECHNO content boundary (a probe counting non-zero bytes in
VGA memory: a black plateau at 4774–4793 and the rings at 4797) is within 1–3 frames in two more.
**What is left over is occasional and unexplained**: one capture session of this block
(`out/b3-tunnel-boundaries`) had TECHNO's rings on screen by 4770 — ~30–40 frames early — while two
independent sessions watched with a VGA-content probe put them at 4797 in both. (The four sessions
watched through the video *mode* cannot see the rings' arrival at all: TECHNO draws them in the
tunnel's own 256-colour mode, before its mode change, and that change is 4797–4801 in every session
watched.) It is not the music clock (which agrees within a frame), it is not a fixed per-session
offset, and it is not the counter stalling (two sessions watched at 20 ms resolution to frame 4900
showed no gap over 150 ms and 0.0701 frames/ms in both). B4/B5 should read a re-capture that exits 1
by the numbers in its own row, and treat an index whose screen is a tens-of-frames outlier as one
session's observation until a second session agrees with it.

For the rows below: a *held* frame is reproducible byte for byte in every session seen; a *moving*
frame is one exact picture that the oracle reproduces when the two sessions happen to agree, and the
row records the offsets and the percentages rather than pretending to a tolerance.

**And the two-screen part's layout is `compare`'s own convention.** TECHNO's screens are two
geometries, so its capture is two captures: a directory with no `manifest.json` is resolved by
`compare` into every immediate subdirectory that has one, named by that subdirectory
(`xtask/src/compare.rs:401-443`, the `reference/check_a` layout). `reference/parts/techno/` holds
`interference/` (320×200) and `troll/` (320×400), and the partner side mirrors the names.

### 1.8 A moving keyframe's reference has to be checked against sessions, or it is a phantom diff

Sessions do not sample a moving screen continuously — they land in **discrete alignment groups**, and
the group sizes are wildly unequal. A reference frame drawn from a minority group is a target Phase 2
will miss for a reason that is the oracle's own, which is exactly the diff this phase exists to
prevent. **This section's first version got the check wrong in three ways, all found by the review of
block 2, and the rule below is the corrected one.** The defects, because they are easy to repeat:

- **Fixed-index grouping under-counts.** A session that presents the reference's picture at its
  neighbour's index is *agreeing with the oracle's own ambiguity*, which is what `compare`'s ±1
  window is for; counting only same-index matches called GLENZ 2300 a 3/6 split when the window
  makes it 6/9 for one picture.
- **Counting the reference tree as one of its own sessions** adds exactly +1 to the reference's own
  group in every row — a bias *toward passing*, in the one check whose job is to catch a bad
  reference. The reference's directory is its own capture and must be excluded.
- **"In the largest group" is vacuous when every group is a singleton**, which is what a fixed-index
  count eventually reports for a screen that moves 25 % of its pixels a frame.

**The rule, and B4/B5 should follow it literally.**

1. **Run the check only on captures that hold `N-1`, `N` and `N+1`.** Single-index probes can answer
   only offset 0 and inflate apparent disagreement. (The partner captures already hold the
   neighbourhood; a sweep does not.)
2. **For each picture, count the sessions that can present it at `N-1`, `N` or `N+1`.** This is a
   *support count over the window*, not a partition: one session's window holds several pictures and
   different sessions reach the same picture at different offsets. **Exclude the reference's own
   capture directory from the count.**
3. **File the picture with the largest support, and require it to pair at offset 0 or ±1 with
   `0.0000 %` in at least half the sessions. The percentage is the discriminator, not the offset** —
   a ±1 pairing with a large percentage is the window's edge, meaning the phase is two or more frames
   out, and it must not be read as agreement. (One caveat the rule needs: the filed file must be the
   frame some session actually took **at `N`**, so the candidates are the pictures seen at `N` — a
   picture that only ever appears at `N±1` cannot be filed at `N`, whatever its support.)
4. **If the largest support is below half, record the keyframe as having no settled content** and say
   so, rather than implying a majority.
5. **A row's headline number must come from the support count over sessions, never from a single
   partner captured in the same sitting** — which is exactly how the first version of this check went
   wrong: a partner taken two minutes after an in-place re-capture can be one frame behind it and
   report 0.0000 % for a reference that most sessions do not present at all.

**The census, by that rule**, over the 131 session directories that hold this recipe (`SECOND 2`,
30000 cycles, `autoexec`, these keys) — B1's captures included, the reference tree excluded:

| keyframe | sessions with a window | pictures at `N` | largest support | filed reference is it? | ≥ half the sessions? |
|---|---|---|---|---|---|
| GLENZ 2300 | 9 | 9 | `dbe96c0a…` 6/9 (67 %) | **yes** | yes |
| GLENZ 3000 | 21 | 15 | `05905a2c…` 8/21 (38 %) | **yes** | **no** — no settled content |
| GLENZ 3400 | 10 | 13 | `d02e362e…` 3/10 (30 %) | **yes** | **no** — no settled content |
| TUNNELI 3800 | 9 | 7 | `f861b1f2…` 7/9 (78 %) | **yes** | yes |
| TUNNELI 4200 | 6 | 9 | `af4763b3…` 3/6 (50 %) | **yes** (tied) | yes, exactly half |
| TUNNELI 4500 | 5 | 10 | `cb15de2d…` 2/5 (40 %) | **yes** (tied) | **no** |
| TUNNELI 4600 | 6 | 14 | `16f8c1fd…` 1/6 (17 %) | **yes** (tied) | **no** |
| TECHNO-A 4900 | 4 | 6 | `3dc29660…` 3/4 (75 %) | **yes** | yes |
| TECHNO-A 5000 | 5 | 1 | `20f3d62f…` 5/5 (100 %) | **yes** | yes |
| TECHNO-A 6300 | 6 | 6 | `3c2b0d8f…` 5/6 (83 %) | **yes** | yes |
| TECHNO-A 8100 | 4 | 6 | `6033bbc4…` 3/4 (75 %) | **yes** | yes |
| TECHNO-B 8300 | 4 | 4 | `17b9f92f…` 3/4 (75 %) | **yes** | yes |
| PANICEND 8792 | 1 (+2 refused/content) | 3 | `c9bf5114…` 1/1 | **yes** (tied) | trivially |

Held keyframes are not in the table because they need none of this: GLENZ 1499 is one picture in
7/7 sessions, TECHNO 8200 in 5/5, TECHNO 8400 in 5/5, PANICEND 8800 and 9000 in 3/3 each.

**Byte-identity is a knife edge on GLENZ, and the rule should be read with that in mind.**
`05905a2c…` (7 sessions) and `dd85288b…` (2 sessions) at GLENZ 3000 are **19 pixels apart** — one
phase split in two by byte-identity, not two alignments. So a "group" of one or two on that screen
means "a picture within a few pixels of its neighbour", not "a different picture"; the support
fractions there are a measure of the oracle's phase spread and not of a port's reachability, and the
rows say so.

**What filing the largest-support picture bought, measured against independent partners (captured
separately, never in the same sitting as the reference):**

| part | before | after |
|---|---|---|
| TUNNELI | 8.2250–10.1672 % on all four keyframes against `out/tunneli-recapture`, and 8.0672–10.1812 % against `out/tunneli-recapture2` — two partners, one pre-fix reference (§8.4) | 3800/4200/4500 **0.0000 %**, 4600 **0.0891 %** (57 px) |
| TECHNO 320×200 | 24.5938–72.8703 % on 4900/6300/8100 | **`exit 0, verdict: identical, 0.0000 %, {0: 4}`** |
| GLENZ | **against the review's independent fresh partner** (`out/b3b2-rerev`, `glenz-fresh3`): 18.6625/9.0125/16.2953 % at 2300/3000/3400 for the *second* reference, filed by mistake, two frames out, and 0.00/0.11/0.03 % for the first. **Against its own same-sitting partners** the *first* reference read 18.6625/9.2000/16.3281 % (`out/glenz-recapture2`) and the first re-capture read 0.0000/0.0500/0.0016 % (`out/glenz-recapture3`) — the §7.4 pair, and the reason a headline number is never quoted from one | 2300, 3000 and 3400 **byte-identical at offset 0** against the review's independent fresh partner, and 0/6/65 pixels against another (§7.4) |

**The GLENZ case is why rule 5 exists.** The first re-capture was measured against a partner taken
in the same sitting and read as "34.32 % → 0.0000 %"; the review's independent partner showed the
filed reference was **two frames** out at all three indices while the reference it replaced was one
frame out, i.e. the change had made the artefacts worse. The picture filed now is the one the
sessions present — the review's own fresh partner holds those exact bytes at 2300, 3000 and 3400 —
and the support counts above are what says so.

### 1.9 Three things block 3 (MNTSCRL, LNS&ZOOM, PLZPART, MINVBALL) measured

**The split's `+ 1` is measured now, not inferred.** §1's D16 caveat said "the first part that
splits must confirm the mapping against a capture rather than trust the `+ 1`". PLZPART is that part,
and the capture confirms it: `pompota` (`PLZPART/COPPER.ASM:126-143`) alternates CRTC 0x18 between
`0x3c` (60) and `0x3d` (61) frame by frame, and three consecutive captured frames show the lower
view's first content row at **62, 61, 62** (`out/b4-plz-probe1`, §13.2). `Split::scanline` is the
register plus one. Two things the measurement also puts on record: the register is *animated* —
`do_drop` (`COPPER.ASM:189-268`) walks it down the screen at the end of each plasma phase, and
framed at 8289/8290/8291 the first content row is 195/202/209 — and what sits above the split is
whatever the CRTC's start address points at, not a picture.

**A stride refusal can be the racy register read, and that race has a quiet outcome as well as a
loud one.** §1.7 records the read naming another register's value and the tool refusing the screen
(TECHNO's 0xa0 from CRTC 0x0c). PLZPART's refusal at 6000 names a **384-byte** plane row (§13.4), and
that number has **two explanations the capture cannot separate** — the message names the stride, not
the register:

- **the offset register read another register's value** (consistent with §1.7's mechanism): `0xC0` is
  exactly the low byte of `cop_start = 96*(682-400) = 27072 = 0x69C0`, which `PLZPART/COPPER.ASM:69-72`
  writes to CRTC register **0x0d** every frame; or
- **the colour depth was misread as chain-4** — the *other half of the same race*: 384 is also exactly
  `0x30 × 8`, the plasma's own offset value (48 units) taken as a chained row. `chain4_enabled()` is
  the same two-operation port read (`crtc_offset_register` writes the index to 3c4h, then reads 3c5h
  in a separate request), and `PLZ.C:95-118` writes 3c4h twice a frame, so a chain-4 bit caught
  mid-write gives the identical message.

The two differ in what they imply for the *quiet* failure, and that is why both are written down.
Under the first, the value the read names can **pass** `plan_indexed_read` and be used: had it
returned `pompota`'s other value, 0x3c, the plan would have accepted a **120-byte** plane row with
only the coherence check between that and a sheared `.idx` in the reference tree — and §1.6 shows a
flat palette blinds that check. Under the second, a racy *offset* value is multiplied by eight, so it
would nearly always exceed a plane and refuse loudly; the quiet case needs the offset to be the
register that is misread **and** the layout to be read right. Nothing in this block's artefacts is
wrong either way (every PLZPART frame is RGB-only), but which of the two it was is not settled here,
and the wider lesson stands: **any `.idx` filed from a part that writes CRTC registers is worth
reading by hand.**

**And the durable fix is in `xtask`'s oracle, not in this ledger** — recorded here so the next change
to the oracle sees it: `crtc_offset_register()` and `chain4_enabled()` each read the index/data port
pair in two requests with the guest running in between. Reading the pair coherently, or reading it
twice and requiring agreement, would remove the race for every part rather than documenting it part
by part. It is outside this task's scope and nothing in the tree is currently wrong because of it.

**A part's two screens can differ in what "the same index" means, and the difference is smaller than
one frame.** PLZPART's cube is byte-identical at its keyframes in every session captured (§13.5);
its plasma is byte-identical in *some* sessions and 118–3942 px of 128000 away in the others —
0.09–3.08 % on a screen whose own move is 70 % a frame, i.e. **a thousandth to a twenty-third of one
frame**. So §1.8's support count reads as "no settled content" there while the pictures are within a
few hundred pixels of each other, exactly the knife edge §1.8 records for GLENZ 3000 one decimal
place further down: **on a fast-moving screen the count measures the last decimal of the phase, not
the reachability of the picture**, and a row that quotes it must say which it is.**

### 1.10 Seven things block 4 (RAYSCRL, 3DSINFLD, JPLOGO, U2E) measured

**A capture refused for geometry still performs the indexed read for the index it refuses.** The
refused index's read is attempted before the geometry is compared: the loop grabs *and reads* a frame
(`xtask/src/reference.rs:604-627`, `oracle.frame()`) and only then checks `(width, height)` against
the capture's own, so a warning in the log can belong to an index that never landed. In
`out/b4b-ray-sweep2` the log carries 11 warnings for 13 frames on disk — the 11th is the refused
17100's read. The counts have no fixed relation (a frame can also be RGB-only for D15's ordinary
reasons), so read a refused log's warnings against the frames that landed, not against the frames
that were asked for.

**The racy register read of §1.7/§1.9 is live on every planar part of this block, and the value it
can return is one the plan accepts.** §1.9 recorded that a raced read "can pass `plan_indexed_read`
and be used", with `pompota`'s 0x3c as the plausible quiet value. Three parts of this block write
**CRTC 0x0c** (start address high) every frame — 3DSINFLD toggles it between `0x80` and `0x00` for
its page flip (`COMAN/MAIN.C:101-110`), JPLOGO writes it and 0x0d from `scrolly` (`JPLOGO/JP.C:30-45`),
and U2E reaches the same registers through `vid_setswitch` inside its copper
(`VISU/C/U2E.C:130-141`, `VISU/AVID.ASM:341-357`) — and 0x80 read back as the offset register is a
**256-byte** plane row, which `plan_indexed_read` accepts at 200 rows (51,200 ≤ 65,536). The loud half
of the race fired eight times on JPLOGO: eight `PlaneTooSmall` refusals naming a **320-byte** row
(160 units, the low byte of `y*80` that `scrolly` writes to 0x0d for `y ≡ 2 (mod 16)`) on a screen
whose rows `lineblit` writes as 80. **Nothing in the tree was produced by the race** — no `.idx` from
any of these three parts survives — but the hand-check §1.9 asks for is what stands between the quiet
value and a sheared plane. Only one `.idx` is filed by this block at all — RAYSCRL's 13500, whose
part writes no CRTC register — and it was checked twice: **0 mismatches of 64000 pixels** through the
manifest's palette, 190 indices to 190 colours (§15.3).

**A `.idx` can be honest and still weaker evidence than the ones already in the tree, when the palette
partly collapses.** A sweep frame of 3DSINFLD's (`out/b4b-ray-sweep2`'s 15100, not filed) has no
index carrying two RGB triples — so it is not sheared — but its **83 indices reach only 58 distinct
colours** (21 triples are reachable from two or
three indices), because the part builds its palette from a formula (`COMAN/MAIN.C:128-150`) that
rounds several entries together. On such a screen the coherence check has fewer distinct colours with
which to catch a shear than a one-to-one palette gives it (LNS&ZOOM 2700: 64 indices ↔ 64 triples,
§12.2), so "the tool accepted it" is not enough there either.

**A keyframe can be *held* without being reproducible, and a hold's states can be discrete.** JPLOGO
holds for ~350 frames — measured, 17500, 17501, 17600, 17699, 17700, 17701, 17800 and 17850 are one
`sha256` in `out/b4b-jp-sess1`, while 17499 and 17900 are not — and the source says why: `doit`
recomputes the two row-mapping values only while its own `frame <= 511` (`JPLOGO/JP.C:79-83`) and the
800-entry tables at `:123-165` saturate, so the `lasty/lasts` guard (`:96-99`) stops redrawing.
**But the held bytes are not the same in a different session**, and the states are **discrete with
measured repeats**: `out/b4b-jp-sweep1` holds `ae2c3d08…`, `-sess1` `c8d6faa0…`, and `-sess2` and the
reviewer's `out/review-b4b/jp-sess` hold the *same* `ec3de2bb…` (0 px of 128000 differ, measured), so
four sessions and a sweep produce three states. **Discrete, but not a small set**: twelve captures of
the recipe hold **ten distinct byte-images at 17700** and nine at 17500, and among the seven
re-capture probes the only repeat is `-ref5` = `-ref9` — so a state recurs and no particular one is
reliably reachable (§17.3, §17.5). They differ by **patches** of rows rather than by
a band or a shift (`sess1` vs `sess3` differ in rows 18–158 only; `sweep1` vs `sess1` in all but
397–399), which is the guard leaving an earlier state's pixels where a row's parameters repeat. So
the two lessons are: **look for the settle branch in a part's own loop before assuming a moving
screen has no held picture**, and **census the hold across sessions** — RAYSCRL's held 13500 is
byte-identical in every session that took it (§15.3), JPLOGO's hold is a draw from a wide, discrete
set whose width seven probes measured and could not land the state the sessions hold (§17.5). The
mechanism behind the draw is a *hypothesis* this block records rather than a finding: the doit
counter can jump past 511 in one step (`frame += waitb()`, `:74-77`), freezing `y1`/`y2` at the
pre-saturation value (§17.3).

**A screen's per-frame change is not stationary, so neither "held or moving" nor a rate may be
carried over from another index of the same part.** RAYSCRL's 12999→13000 changes **64 px (0.10 %)**
and 13000→13001 changes **23,908 px (37.36 %)**; 3DSINFLD's frames alternate **0 px and 37–40 %**
because each render iteration costs two vblanks and the picture therefore changes every *second*
frame. The triple around each keyframe (§1.3 rule 2) is what measures this, and it is not only for
the census: it is how 13500 was found to be held and 13000 not, on the same screen, 500 frames apart.

**And §1.8's remedy works, twice in this block, when it is applied to a support count rather than
a hunch.** 3DSINFLD's first reference had 15500 in one session's window only (1/3) while all three
sessions' windows hold `be95db6c…`; re-capturing (a separate sitting) drew that picture and its
support went **1/3 → 3/3**, with 16000 and 16800 keeping the bytes they had. U2E's first reference had
2600 at **0/4** — a picture no session holds — while three sessions draw `58293d89…` at `N`; the
re-capture drew it and the support went **0/4 → 3/4**, with 1000, 1600 and 3600 unchanged. (Both
counts are over the sessions that hold the keyframe's window: three partners for 3DSINFLD's 15500, and
four for U2E's 2600 — the three partners plus the reviewer's `out/review-b4b/u2e-sess`, which the
census of §18.5 counts and B4b's first pass did not.) Both are the same procedure §1.8 rule 5
describes, and both were checked with the same sessions rather than a fresh partner — which is
the part of the rule that keeps the re-roll from being a dice throw.

**A part can be reached by more boots than its row first assumed, and the test is the `partsmask`
byte, not the block's name.** `partsmask`'s entries are TASM **binary literals**
(`MAIN/U2.ASM:110-119`: `db 11100b ;3`), so `SECOND 3` selects `0b11100` = `0x1c` = bits 2, 3 and 4 —
and bit 3 is `vector`, i.e. **U2E**, which `test whattorun,8` at `:880-887` then runs. This ledger
first said `SECOND 3` could not reach U2E, by misreading `0x1c` as "bits 2 and 4"; the reviewer's
`out/review-b4b/s3-after` shows U2E's opening (`befaf38ae1…`) at 18000 and 18100 on a `SECOND 3` boot,
and `out/b4b-s3-u2e2` reproduces it (§18.1, §18.6). **What changes between boots is the boot
*index*, not the screen**: `VISU/C/U2E.C:195-215` skips `vid_init(1)` when the BIOS mode is 13h, but
`fadeset()` and **CRTC 9 = `(old & 0xf0) | 0x80`** at `U2E.C:302-313` re-establish the 200-line display
themselves, so the `SECOND 4` reference and the `SECOND 3` frames are byte-identical. A row must
record which arguments reach the part *and* at which indices.

---

### 1.11 Seven things block 5 (ENDLOGO, CRED, ENDSCRL, DDSTARS) measured

**`SECOND 5` starts the end sequence at its own beginning, and the end sequence is three geometries
in about 8000 frames.** `partsmask[5]` is the literal `10000b` (`MAIN/U2.ASM:116`) = `0x10` = bit 4
alone, and the shell does the preparation itself before `exe2021` (`restartmus` for MUSIC0 at order
25, mode 13h, `inittwk`: `:898-909`). Measured on this boot: ENDLOGO is **320×400** (Mode X, 80-byte
plane rows) and runs to the geometry change in **(1200, 1240]**; CRED is **320×200** as the emulator
names it, from that change to the next one in **(8840, 8940]**; ENDSCRL is **640×400** (mode 0eh,
16 colours, CRTC 9 = 0) and does not end. No index from the plan or from any earlier note was used:
every one of them was read off a sweep of this boot (§§19–21). The plan's own B5 note warned that an
earlier version of it had the end sequence's *start index* wrong by thousands of frames, and this
block confirms the shape of that warning rather than its number: the sequence's position follows
from which bits the boot sets, not from a remembered index.

**A capture can be refused before it starts for a reason that is neither §1.2's geometry nor §1.3's
deferred-read backlog: the grab loop cannot take two adjacent frames on this block's 640×400
screen.** `--frames 9099,9100,9101,…` is refused **at the first batch — five sessions record it and a
sixth was killed before it could** — with `the emulator reached frame 9101 before frame 9100 could be
captured` (three of the five with nothing else running, two with the core-sharing remedy in place),
and a two-index list (`9099,9100`) is refused the same way. A **two-frame gap works once** (`9098,9100`
captures) but not in a three-index batch (refused at 9102), while a **three-frame gap works every
time**: `9097,9100,9103` and the twelve-index lists of §21.4 all capture. The same three-index shape is fine on every 320×200
screen in this ledger and worked four times over on ENDLOGO's 320×400 screen in this same block, so
this is not a property of the index list. It is the per-grab cost crossing one emulated frame's wall
time: a grab is three HTTP requests to the emulator (a counter read, a raw frame, a counter read) and
the screen's frames are three times the bytes of a 320×200 one — and it is marginal rather than
absolute, since the same shape failed in **one of three** sessions on DDSTARS's 320×400 screen, at
5001.

**Two remedies were tried and both are recorded because both failed, and why they failed is the
useful part.** (a) *Sharing one core between the tool and three busy loops* (`taskset -c 0` around
the whole capture): the emulated frame counter did slow — a session that took ~2 minutes to reach
9101 unloaded took ~7 loaded — but so did the requests, because **the tool's grabs are served by the
emulator's own process**; all three sessions were refused exactly as before. (b) *Pinning only the
emulator to a starved core* (`taskset -pc 0` on the child, three loops on the same core, the tool
left on cores 1–3): refused too, for the same reason — a starved emulator answers slowly. No
available knob changes the *ratio* of the emulator's frame period to its own request latency, and the
knob that would (a smaller frame: the read is `format=raw`, 768,000 bytes) is in the tool. **What
this block did instead** is in §21.4: the sessions are captured with **three-frame gaps** (`9097,
9100,9103` and the same around each keyframe — two-frame gaps were tried and refused as well, at
9102) and the census is an **offset-0 count** with the reason recorded, rather than a window count
that cannot be taken.
The refused sessions are not rehearsals either way: §1.8's rule counts a run that was refused later
if it holds the window, and `out/b5-endscrl-sess1`–`-sess3`'s first attempts (refused at 9101) hold
nothing, while `out/b5-ddstars-sess3`'s first attempt holds 1899–1901 and 2999–3001 but not the
5000 or 8000 windows.

**`looping` starts at 0, so ENDSCRL is on screen by default and this block's capture pins the
non-looping case** — `m_looping = 0` (`MAIN/MENU.C:30`, "Demo looping: Disabled", `:233`), the
recipe's `KBD_left KBD_enter` leaves that toggle alone, and a running ENDSCRL is itself the proof
(`cmp cs:looping,0 / jne @@skipp6`, `MAIN/U2.ASM:914-915`). A reference for this part is only valid
for a session that answered the menu the same way, and the row says so (§21).

**Two parts of this block never end, and their references are windows rather than whole parts.**
ENDSCRL's loop is `while(!dis_exit())` (`ENDSCRL/MAIN.C:45-49`) and DDSTARS's ends only on DIS
function 2 (`STARS.ASM:774-778`); `dis_exit()` is the DIS server's `exitflag`, which only a key press
sets — nothing in either part, and no music position, stops them. Measured: DDSTARS is still drawing
at 20000. A row that named an "end" for either would be inventing one.

**A screen that is still, with a still palette, can still be refused an `.idx` — and CRED is the
first part in this ledger where that is the case.** All 82 frames of the two CRED runs that made the reference (the 77-frame map and the five-frame
reference capture) are RGB-only, refused by
`indexed_matches_rgb` (the tool's "does not reproduce the RGB frame" warning) and **not** by a stride
or a mode refusal; and the screen is *not* moving: three consecutive frames are byte-identical at
every keyframe, and every hold is 200 frames by the source's own loop (`MAIN.C:194`) against a
~20-frame read. §1.7's list of causes for
a missing `.idx` therefore needs its fourth entry — **the read's model of where the screen starts**.
`capture_indexed` reads each plane from offset 0 by design, and CRED's screens begin at
`tw_setstart(160*200 + pan)` with CRTC 0x13 = `0x5013` (§20.2). The two candidate mechanisms (the
ignored start address, and a stride rule that is right for the display and wrong for the guest's own
pen) are not separable from these artefacts; the row names both rather than one.

**DDSTARS's screen is 320×400, and the part does it to itself.** `init_stars` clears CRTC 9's
scan-doubling bit (`STARS.ASM:72-78`) *after* setting mode 13h, taking the 200-line display to 400 —
and the emulator still names the screen **16-colour** (`vga.gfx.mode & 0x40` is what
`VGA_DetermineMode` needs for `M_VGA`, `hardware/video/vga.cpp:74`, and the part writes the graphics
controller), so the indexed read is refused with `UnmodelledMode` on **20 of 20** sweep frames. **The
loud message is the one to quote**: nothing was read at a guessed stride, and the noise-like picture
this part produces is not "a screen that moved".

**A census that counts by index alone counts the wrong sessions, and this block measured several
cases where the wrong session agrees perfectly.** `out/rev-batch-probe` — a directory from an earlier
block, with no manifest — holds `85789170…` at 429, 430 **and** 431, which is exactly ENDLOGO's
white, because **BEGLOGO's 430 on a `SECOND 2` boot and ENDLOGO's 430 on a `SECOND 5` boot are the
same bytes** (`reference/parts/beglogo/frame_000430.rgb` and
`reference/parts/endlogo/frame_000430.rgb` are one `sha256`). The two parts get there differently and
they still coincide: `END/END.C:62-63` points the DAC's write index at **0** and writes 63 **765
times** — 765 *bytes*, which is the three channels of DAC entries **0–254** (**entry 255 is not
written**; the loop stops three bytes short of it), while `BEG/BEG.C:25-33` writes 63 into **0–254**
and then **black into 255**, and
BEGLOGO's white *screen* is index 15 (`memset(vram,15,…)`, `:20-21`) — a different mechanism giving
the same all-white pixels. **Unfiltered, ENDLOGO's four windows have ten wrong-boot holders**:
three at 429–431 (`out/b2-batch-probe`, `out/review-b3-readcost`, `out/rev-batch-probe`), six at
499–501 (`out/b4-mnt-sess1`–`-sess6`, MNTSCRL's `e68aa887…` on the `SECOND 3` boot) and one at
899–901 (`out/b3-alku-probe`, ALKU's `fec9afb5…`). (`out/b2-white-probe` holds 429 and 431 but not
430, so it is not a window holder at all and is not counted here.) The counts in this
block's rows are therefore **filtered by recipe** — a session counts only when its manifest names the
same `arguments`, `boot`, `cycles` and `keys`, and a manifest-less directory counts only when this
block made it, with §19.4 naming the one same-recipe directory that rule sets aside and saying why.
Byte-identity is not identity of a screen; the row's boot column is what says which part a frame
belongs to.

**And the end sequence's timeline is anchored: every keyframe of all four parts reproduces at its own
boot index across separate sessions.** ENDLOGO's white, held picture and two palette fades are
byte-identical in four sessions (§19.4); CRED's five holds are byte-identical in four, with
`K-1 = K = K+1` in every one (§20.3); DDSTARS's four moving keyframes are byte-identical in six, five
and four sessions, per keyframe, plus a probe (§22.4); and ENDSCRL's four are byte-identical in nine
(9100) and five (the others) (§21.4). That is the MNTSCRL shape of §23, and it is the opposite of
block 2's music-gated parts: nothing in this block needed a *state* census to decide what to file,
only a count to prove the filing was taken at `N`. **One part's count is an offset-0 count rather
than a window count** — ENDSCRL's, because the screen refuses adjacent grabs (§21.4) — and the
difference is stated in its row rather than papered over.

---

### 1.12 The non-determinism policy — what an index means, and what is not settled

Sections 1.1–1.11 are method, written as each block found it. This section is **the policy**: what an
index under `reference/parts/` means, what makes a capture valid, how a part that does not reproduce
is handled, and the things this phase measured and could **not** settle. It was written at the close
of Phase 1 (B6) and it is the phase's own statement of its evidence — where an earlier section and
this one disagree, this one is the current reading.

**The digests in this document now live in the tree, and a command checks them there.** Since Phase 2's
Task 2, `reference capture` writes every frame file's `sha256` into the manifest beside it (`sha256`
for the RGB frame, `idx_sha256` for the index plane), and `cargo xtask reference verify --tree
reference` walks `reference/` with `compare`'s all-the-way-down rule and checks each capture against
its own manifest: a named file that is missing, a `frame_*` file the manifest does not name, a length
its geometry does not imply, or a digest that does not match its bytes is a finding and exit 1, while
a tree it cannot check at all is exit 2. The `sha256` values quoted throughout this document remain the
record of what was captured and re-captured; what changed is that a machine reads the record now, so
**a byte changed in place** — the one failure this document could not report — fails a command. The
manifests were backfilled once, under out-of-band hashes taken before and after it; `docs/plans/02-shell.md`
Task 2 carries that exception, and `PROGRESS.md` the evidence.

**What a capture index means.** A keyframe index is a **boot-relative emulator frame counter**:
frame 0 is the emulator's first rendered frame at process start (D14), the counter counts emulated
frames (`GFX_EndUpdate`, D17), and the setup screen's text is up at 16 and the demo's first graphics
frame at 404 under this phase's recipe. An index names a *screen* only as precisely as the boot
repeats — and it repeats exactly under one recipe and no other.

**A capture index is valid only under the B1 recipe, at the pinned cycles, with the silent
emulator.** Every capture in the tree was taken with

```
--arg <block> --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000
```

and an emulator run **silent** (`SDL_AUDIODRIVER=dummy` + `mixer_nosound=true`). `--boot autoexec`
puts the mount, the command line and the setup-screen answer on the *emulated* clock (the guest's own
`[autoexec]` runs it; `AUTOTYPE` schedules through `PIC_AddEvent`), which is why the same boot index
is the same screen in five runs out of five, where the typed recipe's position is the host's timing
(a frame of jitter at its best on an idle host, 43–45 frames between two D15 runs under load).
**Silence is not
optional decoration**: with a real audio device the mixer follows the *host's* clock and the demo's
animation drifts against the frame counter, continuously and differently per run (two autoexec
captures still differed 14.3/35.3/24.6 % of bytes until the emulator was also run silent — D17).
The cycles matter less — capturing the held index 900 at 25000 and at 30000 gives byte-identical
files (§1.5, measured) — but a capture that does not pin them is **refused** rather than produced,
because an unpinned capture is not reproducible. Every manifest records `boot`, `cycles`, `keys` and
`arguments`; a frame set whose manifest says something else was captured some other way and is not
comparable with these references.

**What is reproducible, and what is not.** Three separate claims, and a reader of a row needs to
know which one it is making.

- **The boot repeats.** Menu frame 16, first graphics frame 404, five runs in five (D17).
- **A held screen repeats byte for byte**, within and across sessions (BEGLOGO 900 in five sessions;
  CRED's five holds with `K-1 = K = K+1` in four).
- **A moving screen's alignment is not reproducible across emulator sessions.** Sessions land in
  **discrete alignment groups**, not a continuum, and the majority group is byte-identical; a second
  capture lands in the same group about half the time (D17). The mechanism is measured rather than
  guessed: DOSBox subtracts measured host present time from its tick budget
  (`gui/sdl_gui.cpp:2555-2570`), so the emulated clock carries a host-timing term that neither the
  API nor `--set` reaches.

**The comparison searches ±1 frame and reports the offset — and that is a search of the alignment,
never a tolerance on the pixels.** `compare` tries the rendered frame at `N-1`, `N` and `N+1`,
pairs the candidate with the fewest differing pixels, and records a signed per-frame `offset` and the
capture's offset distribution (D18). A paired frame is **0.0000 % or it is a difference**; no pixel
tolerance was added anywhere in the toolchain, and a reference index with no candidate within the
window is **refused, exit 2**, not paired from further away. The window is a constant
(`ALIGNMENT_WINDOW = 1`), not a knob. The asymmetry that makes the search usable is D19's: **the
reference holds keyframes only; the capture compared against it holds `K-1, K, K+1` around each
keyframe that moves and nothing around a held one** (§1.3 rules 1–2; the ends-only form is enough
only for a contiguous range).

**And a run now says how much it measured.** Since the final fix round, `compare` prints
`captures: N measured` immediately before `verdict:` — a partial measurement must not be readable as
a complete one (the whole-tree form once measured 18 of `reference/parts`'s 22 captures and still
printed `verdict: identical`, exit 0). Every transcript in §§3–22 was taken before that line existed,
so a quotation that goes straight from the per-frame lines to `verdict:` is a record of the tool as
it then was, not a contradictory output.

**§8.4's structural-similarity fallback was not needed, and the reason is exactness.** Spec §8.4
allows a part that proves non-deterministic to be "captured across multiple runs and compared on
structural similarity". **No part needed it.** The ±1 frame search is a different instrument from
structural similarity, and the difference is the whole point: the search keeps the comparison
**pixel-exact** and moves only *which* rendered frame answers for a reference one — the pictures are
compared byte for byte and the alignment is stated. Structural similarity would give up exactness: it
would replace "the pictures are the same" with "the pictures are similar enough", which is a
judgement the port cannot audit and exactly the kind of tolerance this phase refused at every turn.
Where the ±1 window cannot reach the difference, the phase **records the residue and says what it
is** (ALKU's pan, §4.4; the no-settled-content rows below) rather than defining it away. The
fallback remains available in the spec if some later phase meets a screen neither the window nor a
recorded residue can describe; nothing in this phase did.

**How a part that does not reproduce is handled — the ladder, in order.**

1. **Census it against every session that holds the keyframe's window, by §1.8's rule**, and
   **re-capture it into the largest alignment group**. This is the remedy that worked, and it is not
   marginal: TUNNELI went from 8.07–10.18 % on all four keyframes (8.0672–10.1812 % against one
   pre-fix partner and 8.2250–10.1672 % against another, §1.8, §8.4) to 0.0000 % on three and 0.0891 %
   on the fourth; 3DSINFLD's 16500 from a 38.9031 % residual to `verdict: identical` (§16.4); U2E's 2600
   from 0/4 support to 3/4 (§18.5); TECHNO's 320×200 row to `exit 0, identical`. A re-capture is judged against
   **independent** sessions, never against a partner captured in the same sitting — rule 5 exists
   because that mistake once made a reference *worse* (GLENZ, §1.8).
2. **If the largest support is below half the sessions, the keyframe has no settled content** and
   the row says so (§1.8 rule 4); the best *filable* picture stays filed, and the residual is the
   oracle's.
3. **If the picture is exact but its phase is something the ±1 window cannot reach**, the row
   records the measurement and no tolerance is added — ALKU's 5859 is the case: the picture differs
   between sessions by a whole number of pixels of the part's own 1 px-per-9-frames pan, 0 px of
   95,700 remain after the shift, and the spatial phase is not a temporal offset the search can
   reach (§4.4).
4. **If the ±1 window cannot be captured at all**, the census becomes an **offset-0 count** and the
   row says so, naming the reason — ENDSCRL, below.
5. **Never**: a pixel tolerance, a widened window, a structural-similarity substitution, or an
   unrecorded re-roll.

**The keyframes with no settled content, and what a Phase 2 diff there means.** These are the
keyframes where the largest support over the ±1 window is below half the sessions, so no picture is
presented by a majority:

| keyframe | sessions with a window | support of the filed picture | why |
|---|---|---|---|
| GLENZ 3000 | 21 | 8/21 (38 %) | the screen's phase is split finer than byte-identity — `05905a2c…` and `dd85288b…` are **19 pixels apart**, one phase cut in two (§1.8) |
| GLENZ 3400 | 10 | 3/10 (30 %) | same screen, same knife edge |
| TUNNELI 4500 | 5 | 2/5 (40 %) | the dotted tunnel quantises into several groups |
| TUNNELI 4600 | 6 | 1/6 (17 %) | as above; its partner comparison is still 0.0891 % (57 px) |
| PLZPART plasma 6000 | 5 | 2/5 (40 %) | the plasma moves 70 % of its pixels a frame; the largest support *in the window* (3/5) belongs to a picture every session takes only at `N-1`, which §1.8 rule 3's caveat forbids filing at `N` |
| PLZPART plasma 7000 | 4 | 0/4 | every picture taken at `N` is a singleton |
| PLZPART plasma 7600 | 3 | 0/3 | nine pictures, 1/3 each |
| JPLOGO 17700 | 4 | 2/4 for the *state*, 0/4 for the *file* | settled as a state and **unfiled as a file**: the state two sessions hold (`ec3de2bb…`) was not drawn by any of seven re-capture probes, and the filed `91d6ce6b…` is taken at `N` by none of the four (§17.5) |

**A keyframe on this list is a Phase 2 residual that is the oracle's, not the port's.** A faithful
port reproducing any of them will get a non-zero diff at that index against the filed picture; the
row is what says so. Two knife-edge cases are *not* on the list but should be read beside it:
TUNNELI 4200 and JPLOGO 17500 sit exactly at half (3/6 and 2/4), so a single further session can move
them either way.

**Unresolved — the oracle's rare, real excursion.** One session had TECHNO's rings **~30–40 frames
early**. It did **not** reproduce in either of two VGA-content probes, and it is not any of the
mechanisms the phase could test: **not the music clock** (the player's counter — the 16-bit word at physical `0x105B0E`, advancing
0.7424 a frame, §1.7 — agrees to about one
music frame — ≈1.3 video frames — across sessions and its rate is identical to 1e-4, so the mixer's
wall-clock pacing cannot move a music-gated boundary by tens of frames), **not a fixed offset** and
**not a counter stall** (0 stalls ≥150 ms at 20 ms resolution up to frame 4900). It is recorded as
rare, real, unexplained and not reproducible. The compensating evidence is that the *ordinary* spread
is pinned: five mode transitions within 4 frames across four sessions, and the tunnel→TECHNO content
boundary at 4797 in both sessions watched. What a later phase should do if it meets one: the census
counts sessions that hold the keyframe's window, so a session with an excursion simply does not
count — do not let one decide a reference, and do not re-open the mechanism without a reproducing
case.

**Unresolved — ENDSCRL's deviation from §1.8's window rule, and what a one-frame-out diff there
means.** ENDSCRL is the one screen in the tree where the tool **cannot take two frames one index
apart**: `--frames 9099,9100,9101` is refused at the first batch with `the emulator reached frame
9101 before frame 9100 could be captured` — five sessions record it and a sixth attempt was killed
before it could write one, and a two-index list is refused the same way. The cause is the per-grab cost crossing one emulated
frame's wall time (three HTTP requests and a 768,000-byte raw frame on a 640×400 screen), and **two
remedies were tried and both failed** — sharing one core between the tool and busy loops, and pinning
only the emulator to a starved core — **because the grabs are served by the emulator's own process**,
so starving either starves both. The sessions are therefore captured with **three-frame gaps**
(`9097,9100,9103`), and the census is an **offset-0 count**: 9/9 sessions byte-identical at 9100 and
5/5 at each other keyframe.

Accepted, and the acceptance is a measurement rather than a preference: §1.8 rule 1 warns that a
single-index probe "can answer only offset 0 and inflate apparent disagreement", and that warning is
about sessions that **differ** at `N` — here every session that took the index is byte-identical at
offset 0, so there is no disagreement for the rule to inflate. It is not free, and the row says so:

- **A one-frame-out diff at these keyframes is a difference, not an alignment.** A session one frame
  out of phase would show the neighbouring line of scroll text — measured at **1.2–7.4 % of pixels
  at a three-frame step**, and the picture advances a line per two vblanks — and `compare` would
  report it as differing. ENDSCRL's reference has **no alignment slack for a port to hide behind, and
  no ±1 evidence either**: the offset-0 agreement proves the sessions present these bytes at exactly
  these indices; it does **not** prove that a ±1 pairing would have matched, because no session in
  the tree holds one.
- What the deviation does *not* affect: the four keyframes' own bytes, the three sessions'
  `exit 0, verdict: identical, 0.0000 %`, or the row's comparability with the other nineteen on the
  substance — every reference was filed from a picture sessions actually take at `N`. What it
  affects is the *strength* of the support claim, and the row and §21.4 state the weaker one.

**Unresolved (tool-side) — the CRC-register read race in `xtask`'s oracle.** `crtc_offset_register()`
and `chain4_enabled()` each read the CRTC's index/data port pair in **two requests with the guest
running in between**, so the value that comes back can belong to the other register — or to a
mid-write state. It is live and it has fired: JPLOGO's log holds eight `PlaneTooSmall` refusals
naming a 320-byte row for a picture whose rows are 80 (CRTC 0x0d's value read as 0x13's), and
PLZPART's 384-byte refusal has two readings the capture cannot separate (§1.9). The loud half refuses
the frame; the **quiet half is the one to know about**: a raced value can pass `plan_indexed_read`
and be *used*, and the coherence check that would catch a resulting shear is blind on a screen whose
palette collapses (ALKU's case, §1.6). **Nothing in the tree is wrong because of it** — every `.idx`
filed from a part that writes CRTC registers was read by hand before it was trusted, and the rows say
which and what the read showed — but the durable fix belongs to whoever next touches the oracle: read the index/data
pair coherently, or read it twice and require agreement. That removes the race for every part instead
of documenting it part by part. **Until then, any `.idx` filed from a part that writes CRTC registers
is worth reading by hand**, which is a rule about the tool, not about the references.

**And the phase's own day-apart probe.** B2–B5's re-captures prove reproducibility *within* a
sitting or across minutes. B6 re-ran the oldest two captures a day later (2026-10-11 → 2026-10-12),
in fresh emulator sessions, and compared them against the filed files: BEGLOGO — the first reference
captured, whose 470 and 560 are moving in the palette — gave `exit 0, verdict: identical` on all
four keyframes, every filed file `sha256`-equal to the new capture; ALKU — the oldest continuously
drawn screen — gave `exit 0, verdict: identical` on all four, **5859 included**, its new capture
landing on the reference's own pan phase. So a capture still means the same thing a day later for
these two, which is the weakest and most directly tested form of the claim this section states.

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
| GLENZ | `SECOND 2` | 320×200, 92-byte plane row; 1499 carries `.idx` | 1499, 2300, 3000, 3400 | `out/glenz-recapture4`, and the review's independent `out/b3b2-rerev/fresh-partner` | exit 1, `mode: mixed` — 1499 identical, 0.0000%, offset 0; 2300 **byte-identical at offset 0** against the review's independent partner and 0.0000% at offset -1 against ours; 3000 and 3400 byte-identical at offset 0 against the review's partner, 0.0094% (6 px) and 0.1016% (65 px) against ours. Every filed picture is the largest-support one at its index; 3000 and 3400 are below half the sessions and are recorded as **no settled content** (§1.8, §7.4) |
| TUNNELI | `SECOND 2` | 320×200; RGB only | 3800, 4200, 4500, 4600 | `out/tunneli-recapture3` | exit 1, `mode: rgb` — 3800, 4200 and 4500 identical, 0.0000%, offset 0, and 4600 0.0891% (57 px), offset 0. The reference is the largest-support picture at every index after the fix round 2 re-capture (3800: 7/9 sessions, 4200: 3/6, 4500: 2/5, 4600: 1/6, counted over the ±1 window; 4500 and 4600 are below half and are recorded as no settled content — §1.8) (§8.4) |
| TECHNO — the 320×200 screen | `SECOND 2` | 320×200, 16-colour EGA mode; RGB only | 4900, 5000, 6300, 8100 | `out/techno-recapture3/interference` | exit 0, `verdict: identical`, 0.0000%, offsets `{0: 4}` — the fix round 2 re-capture put the reference in the largest group on all four (4900: 4/5, 5000: 6/6, 6300: 6/7, 8100: 4/5) where it had been outside every group at 6300 and 8100 (supports now 4900: 3/4, 5000: 5/5, 6300: 5/6, 8100: 3/4, counted over the ±1 window — §1.8) (§9.4) |
| TECHNO — the 320×400 screen | `SECOND 2` | 320×400 Mode X, 160-byte plane row where the read succeeded (320-byte rows later, §9.6); 8200 carries `.idx` | 8200, 8300, 8400 | `out/techno-recapture/troll` | exit 1, `mode: mixed` — 8200 and 8400 identical, 0.0000%, offset 0; 8300 (the fade-in) 99.9883% at offset -1, its picture the largest support at 3/4 sessions over the ±1 window (§9.5) |
| PANICEND | `SECOND 2` | 320×200; 9000 carries `.idx` | 8792, 8800, 9000 | `out/panicend-recapture` | exit 0, `verdict: identical`, 0.0000%, offsets `{0: 3}` — including 8792, the collapse itself, whose picture is the one session with a ±1 window and two refused captures also present (§10.3) |
| MNTSCRL | `SECOND 3` | 320×200 Mode 13h; 500 carries `.idx` | 500, 1100, 1700, 2300 | `out/b4-mnt-sess1` | exit 0, `verdict: identical`, 0.0000%, offsets `{0: 4}`, `mode: mixed`; the same result against `out/b4-mnt-sess4` and `out/b4-mnt-sess6`. 500 is held (`e68aa887…` in every one of the 8 sessions that took it); 1100, 1700 and 2300 move within a session (~2.4–3.1 % of pixels a frame) but the same bytes at the same index in all 6 independent sessions, so each is supported **6/6 (100 %)** over the ±1 window (§11.4) |
| LNS&ZOOM | `SECOND 3` | 320×200 Mode 13h; 2700 and 5650 carry `.idx` | 2700, 3400, 4200, 4900, 5650 | `out/b4-lns-sess1` | exit 0, `verdict: identical`, 0.0000%, offsets `{0: 5}`, `mode: mixed`; the same against `out/b4-lns-sess3` and `out/b4-lns-sess5`. 2700 (the held face) and 5650 (the flat white the part exits on) are held; 3400 (the lens), 4200 and 4900 (the rotazoomer) move 6.7–39.8 % of pixels a frame and still carry the same bytes at the same index in all 5 independent sessions — **5/5 (100 %)** support each (§12.4). 5650's `.idx` is a flat-palette plane the coherence check cannot vouch for (§12.2) |
| PLZPART — the 320×400 plasma | `SECOND 3` | 320×400 Mode X, 96-byte plane row, line-compare 60/61 (the lower view starts at scanline 61/62); RGB only | 6000, 7000, 7600 | `out/b4-plz-sess1-plasma` and `out/b4-plz-sess3-plasma` | exit 1, `mode: rgb`, offsets `{0: 3}` — 6000 **0.0000 %** (0 px) against `sess3` and 0.1641 % (210 px) against `sess1`; 7000 7.7344 % / 12.9266 %; 7600 9.3281 % / 24.4844 %, every one at max channel delta ≤ 25. Census over the sessions holding a window (5 at 6000, 4 at 7000, 3 at 7600): **no settled content on all three** — the filed picture is presented by 2/5 (40 %), 0/4 and 0/3, and the largest supports in the ±1 windows (3/5 at 6000, 2/4 at 7000) belong to pictures taken only at `N±1`, which rule 3's caveat forbids filing at `N`. The filed pictures are still the best *filable* ones at 6000; at 7000 and 7600 every picture taken at `N` is a singleton (§13.6) |
| PLZPART — the 320×200 cube | `SECOND 3` | 320×200 Mode X, 160-byte plane row; RGB only | 8500, 9500 | `out/b4-plz-sess1-cube` | exit 0, `verdict: identical`, 0.0000%, offsets `{0: 2}`, `mode: rgb`; the same against `sess2`, `sess3` and `sess4`. Both keyframes are byte-identical in every session captured — **4/4 (100 %)** over the ±1 window (§13.6) |
| MINVBALL | `SECOND 3` | 320×200 Mode 13h; RGB only | 10900, 11200, 11800, 12400 | `out/b4-minv-sess1` | exit 0, `verdict: identical`, 0.0000%, offsets `{0: 4}`, `mode: rgb`; the same against `sess2` and `sess3` — and `sess3` pairs all four at **offset −1** with 0.0000% (0 px, max delta 0), its own 10899 being the reference's 10900 picture's place in the animation. Every keyframe is moving (4.7–7.1 % of pixels a frame) and every session presents the filed picture over the ±1 window — **3/3 (100 %)** (§14.4) |
| RAYSCRL | `SECOND 3` | 320×200 Mode 13h; 13500 carries `.idx` | 13000, 13500, 14000, 14500 | `out/b4b-ray-sess1` | **exit 0, `verdict: identical`, 0.0000 %, offsets `{0: 4}`, `mode: mixed`** against all three of `out/b4b-ray-sess1`–`-sess3` **and** the reviewer's independent `out/review-b4b/ray-sess`. 13500 is **held** (13499 = 13500 = 13501 in every session, and its `.idx` is byte-identical across sessions); 13000, 14000 and 14500 move 0.10–6.37 % of pixels a frame. Census over the four sessions that hold a window: **4/4 (100 %)** on all four, every one presented at offset 0 (§15.4) |
| 3DSINFLD | `SECOND 3` | 320×200, chain-4 off, 80-byte plane rows; RGB only | 15500, 16000, 16500, 16800 | `out/b4b-ray-sess1` | **exit 0, `verdict: identical`, 0.0000 %** against `out/b4b-ray-sess1` (offsets `{0: 3, +1: 1}`) and against `out/b4b-ray-sess2` (offsets `{0: 4}`), `mode: rgb`; exit 1 against `out/b4b-ray-sess3`, whose 15500 and 16000 are 0.0000 % at offset −1 and whose 16500/16800 are 38.9031 % and 41.7875 % (it sits two two-frame steps out of phase). Census: 15500 and 16000 **3/3 (100 %)**, 16500 **3/4 (75 %)**, 16800 **2/3 (67 %)** — **every keyframe settled**, each filed picture taken at `N` by at least one session. The reference was re-captured twice in fix round 1: the first row's 16500 was a picture no session takes at `N` (and its "no settled content" verdict for it was a misread of §1.8 rule 3), and its 16800 was taken only at `N−1` (§16.4) |
| JPLOGO | `SECOND 3` | 320×400 Mode X, 80-byte plane rows; RGB only | 17300, 17500, 17700 | `out/b4b-jp-sess1`, and the reviewer's `out/review-b4b/jp-sess` | exit 1, `mode: rgb`, offsets `{0: 3}` — 17300 **0.0000 %** in all four sessions (`-sess1`–`-sess3` and `jp-sess`); 17500 0.0000 % against `-sess2` and `jp-sess` (which hold the same bytes) and 41.3773 %/39.3516 % against `-sess1`/`-sess3`; 17700 40.1914 % against `-sess2`/`jp-sess` and 29.5719 %/27.3664 % against `-sess1`/`-sess3`. Census over the four sessions: 17300 **4/4 (100 %)** and 17500 `aab611ef…` **2/4 (50 %)** settled; **17700 is settled as a state and unfiled as a file** — the state two sessions hold (`ec3de2bb…`, 2/4, taken at `N` by both) was not drawn by any of the **seven re-capture probes** this round ran (`out/b4b-jp-ref3…ref9`, each the same recipe, keyframes only, ~10 minutes of emulated run), and the filed `91d6ce6b…` is taken at `N` by none of the four. Twelve captures of the recipe hold **ten distinct** byte-images at 17700, so the residual there is the oracle's state spread and not a port error (§17.3, §17.5) |
| U2E | `SECOND 4` (this capture) — `SECOND 3` reaches it too, at its own indices | 320×200; RGB only | 1000, 1600, 2600, 3600 | `out/b4b-u2e-sess1` | **exit 0, `verdict: identical`, 0.0000 %, offsets `{0: 4}`** against the reviewer's independent `out/review-b4b/u2e-sess`; exit 1, `mode: rgb`, offsets `{0: 4}` against `out/b4b-u2e-sess1`–`-sess3`, each of which misses one keyframe at 55.4672–60.8406 % (a phase group, not a different screen). Census over the four sessions holding a window: 1000 **4/4 (100 %)** and 1600, 2600 and 3600 **3/4 (75 %)** each, every filed picture taken at `N` — **every keyframe settled** (§18.4, §18.5). The reference was re-captured once: its first 2600 was a picture no session holds (0/4) and the re-capture drew the one three of them do. `SECOND 3` reaches U2E as well and shows the same screens at different indices (§18.1, §18.6) |
| ENDLOGO | `SECOND 5` (this capture); a full run reaches it too, at its own indices | 320×400 Mode X, 80-byte plane rows; the reference carries `.idx` at 430 and 900, and **only 900 is index-compared** (no session holds a 430 `.idx`) | 430, 500, 900, 1180 | `out/b5-endo-sess1` (`-sess2`, `-sess3` identical) | **exit 0, `verdict: identical`, 0.0000 %, offsets `{0: 4}`, `mode: mixed`** against all three sessions, every keyframe. Census over the four sessions holding a window: **4/4 (100 %)** on all four — 430 (white) and 900 (the held picture) held outright, 500 and 1180 the two palette fades of the held picture; each filed picture is taken at `N` by all four (§19.3, §19.4) |
| CRED | `SECOND 5` (this capture); a full run reaches it too | 320×200, 160-byte plane rows from CRTC 0x13 = `0x5013`; RGB only | 1440, 3240, 5440, 7640, 8740 | `out/b5-cred-sess1` (`-sess2`, `-sess3` identical) | **exit 0, `verdict: identical`, 0.0000 %, offsets `{0: 5}`, `mode: rgb`** against all three sessions, every keyframe. Every keyframe **held** (three consecutive frames byte-identical in every session). Census over the four sessions holding a window: **4/4 (100 %)** on all five, the filed picture presented at `K−1`, `K` and `K+1` in every session. 82 frames of this part, all refused by the *coherence* check and not by a stride or a mode — the read's model of where the screen starts is what does not match (§20.2) |
| ENDSCRL | `SECOND 5`, **non-looping** (`looping == 0`; skipped entirely when looping is on — `MAIN/U2.ASM:914-915`); a full run reaches it too | 640×400, mode 0eh 16 colours, CRTC 9 = 0; RGB only | 9100, 10000, 12000, 14000 | `out/b5-es2-sess1` (`-sess2`, `-sess3` identical) | **exit 0, `verdict: identical`, 0.0000 %, offsets `{0: 4}`, `mode: rgb`** against all three sessions. Every keyframe **moving** (the 401-line ring scrolls a line per two vblanks). **No session in the tree holds this part's ±1 window** — the tool cannot take two frames one index apart on this 640×400 screen (five recorded refusals and a sixth attempt killed — §1.11, §21.4) — so the census is an **offset-0 count**: **9/9 (100 %) at 9100** and **5/5 (100 %)** at 10000, 12000 and 14000, every session that took the index byte-identical to the filed picture (§21.3, §21.4) |
| DDSTARS | `SECOND U` only — slot 17, the hidden part, reachable by no numbered boot (`partsmask[8]` = `80h`) | 320×400 (mode 13h with CRTC 9's scan-doubling bit cleared), 80-byte plane rows; RGB only | 1900, 3000, 5000, 8000 | `out/b5-ddstars-sess1` (`-sess2`, `-sess3b` identical) | **exit 0, `verdict: identical`, 0.0000 %, offsets `{0: 4}`, `mode: rgb`** against all three sessions. Every keyframe **moving** (three distinct pictures at every triple). Census **per keyframe** over the sessions that hold that keyframe's window: 1900 **6/6 (100 %)**, 3000 **5/5 (100 %)**, 5000 **4/4 (100 %)**, 8000 **5/5 (100 %)** — every filed picture taken at `N` by every session, and the same `N±1` neighbours in all of them (§22.3, §22.4). All 20 sweep frames refused with `not one palette index a byte` (§22.5); `out/b5-ddstars-sess3`, refused at 5001, counts for 1900 and 3000 and not for 5000 or 8000 |

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
| 1499 | the **held opening** — the checkered floor with the glenz object's top edge at the horizon; 1499, 1500 and 1501 are one picture, and 1250, 1400, 1450 and 1499 (in other captures) are the same bytes again. | `.rgb` + `.idx` | rgb `e45ab076…`, idx `75d561dc…` |
| 2300 | the **glenz polyhedron alone** — the translucent blue/grey object rotating against the pale background, the floor a thin band at the bottom. | `.rgb` only | rgb `dbe96c0a…` |
| 3000 | the **red polyhedron with the glenz object inside it** — mid-rotation. | `.rgb` only | rgb `05905a2c…` |
| 3400 | the **red polyhedron with the glenz object at its edge** — the last of the two-object phase, ~250 frames before the glenz object leaves the frame. | `.rgb` only | rgb `d02e362e…` |

One keyframe per screen the part has: the held opening, the glenz object, the red polyhedron with
the glenz object inside it, and the red polyhedron with the glenz object at its edge. 2300 and
3000 are ~700 frames apart and 3400 ~400 beyond them, so every one of them can be supported (§1.3).

Full hashes of the five files in `reference/parts/glenz/`:

| index | file | `sha256` |
|---|---|---|
| 1499 | `frame_001499.rgb` | `e45ab0764b860f50376d560cf5a4d0f0e7ecca935021780385e3735e865fc28e` |
| 1499 | `frame_001499.idx` | `75d561dc779e17cadaf4493b3e295378a9d02ed186049dc0edd754b3a130b32b` |
| 2300 | `frame_002300.rgb` | `dbe96c0a69f93c6427de2b4c3108ad017fe55eb463fca948536fe525450d04cf` |
| 3000 | `frame_003000.rgb` | `05905a2ce6a2ed25058df37f267c42d998eaaf62a0d692af925a8d7026650337` |
| 3400 | `frame_003400.rgb` | `d02e362e964b03fcd280e4e3faa2dee8e9712a91ad95889442c1b5a05cee2760` |

### 7.3 Held or moving, measured — and where GLENZ's indices come from

The keyframe triple probe (`out/glenz-probe1`, 15 indices 1199…3401) and the map hashed:

| keyframe | neighbours | reading |
|---|---|---|
| 1499 | 1499 = 1500 = 1501, one `sha256`; and 1250, 1400, 1450, 1499 in `out/glenz-map1` and 1500 in `out/glenz-stride-probe` are the same bytes | **held** — but held in the *index* plane with a palette that cycles: 1300 and 1620 are the same picture over a grey background, and the palette returns |
| 2300 | 2099, 2100, 2101 are three different `sha256`; the partner's 2299/2300/2301 likewise | **moving** |
| 3000 | 2699/2700/2701 and 2999/3000/3001 are three each | **moving** |
| 3400 | 3399/3400/3401 are three | **moving** |

**The hold is bounded, and this is where the first version of this section overstated it.** 1499,
1500 and 1501 are one picture and 1400/1450/1250 hold it too, but **1600 does not**: in
`out/glenz-stride-probe`, whose `--frames 1500,1600` is the run cited for the indices, 1600 is
`86d42e74…`, **82.54 %** of its pixels away from 1499, mean brightness 100.3 against 13.2 — the
grey-background palette phase. (In `out/b3-glenz-sweep1`, a *different session*, 1600 is
`e45ab076…`, the held picture: another instance of §1.7's session dependence, and the reason the
index alone does not decide what is on screen.)

**`.idx` presence on this screen is not something this block can predict, and the rule its first
version stated has been withdrawn.** The measurement is short and goes both ways in the artefacts
already on disk:

- `out/glenz-stride-probe` (`--frames 1500,1600`, two single-index reads): **1500 carries `.idx`**
  (rgb `e45ab076…`, idx `75d561dc…`).
- `out/glenz-probe1` (`--frames …,1499,1500,1501,…`, one 3-index batch): **all three carry `.idx`**,
  each byte-identical to the reference's 1499 — so a batch's later read did *not* cost them.
- `out/glenz-map1` (13 indices, ≥50 apart): 1250 — the same held bytes — has **no** `.idx`, while
  1400, 1450 and 1499 in the same run keep theirs.

So the read's position in the frame list is not the determinant, and what is left standing is only
this: **every moving keyframe of GLENZ came back RGB-only, and its held picture carried indices in
four captures and not in one.** The first version of this section claimed more than the artefacts
support; the review of this block caught it, and the claim is withdrawn rather than repaired.

**And that `.idx` is what confirms the 92-byte stride.** `indexed_matches_rgb` compares every pixel
of the widened index plane with the RGB frame — not a sample, not a colour census — so
`frame_001499.idx` reproducing its RGB is the stride being right, and the independent check agrees:
both the RGB and the plane have non-black content in **rows 154–199 only**, 200 of 200 rows agree
on content-versus-black, and both have exactly **11,720** non-black pixels, across 8 distinct
indices. A read sheared by two bytes a row against 92 (the old `width/4` = 80) could not do that:
the second pass of this phase shipped exactly such a plane for ALKU and it passed the colour check
only because that palette collapsed (§1.6).

### 7.4 The re-capture: the held frame identical, the moving three the pictures the sessions present

```bash
# the reference: keyframes only. Filed from a session captured into out/ and verified against the
# census of §1.8 BEFORE it was filed — see the paragraph below for why that route and not a re-run
cargo xtask reference capture --arg 2 --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames 1499,2300,3000,3400 --out reference/parts/glenz --sheet out/glenz.png

# the partner: a support frame each side of the three moving keyframes
cargo xtask reference capture --arg 2 --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames 1499,2299,2300,2301,2999,3000,3001,3399,3400,3401 --out out/glenz-recapture4
```

`out/glenz-recapture4` (exit 0 as a capture) — an independent partner, captured after the reference
was filed and in a separate sitting:

```
frame 1499: 0.0000% differing (0/64000 px), max delta 0
frame 2300: 0.0000% differing (0/64000 px), max delta 0, rendered index 2299 (offset -1)
frame 3000: 0.0094% differing (6/64000 px), max delta 41, rendered index 2999 (offset -1)
frame 3400: 0.1016% differing (65/64000 px), max delta 121, rendered index 3399 (offset -1)
verdict: different, mode: mixed, offsets {"-1": 3, "0": 1}
exit: 1
```

**And against the review's own independent fresh partner (`out/b3b2-rerev/fresh-partner`, captured
by the reviewer's harness in a different sitting again), all three moving keyframes are
byte-identical at offset 0** — 0 pixels of 64,000 on each, checked by comparing the files directly
because that capture does not hold 1499 and so cannot be compared whole.

**Why this reference and not the two before it.** The first reference's moving frames were 18.6625,
9.2000 and 16.3281 % away from the sessions that took them, and the first re-capture — filed with a
partner taken **two minutes later, in the same sitting** — looked like 0.0000 %, 0.0500 % and
0.0016 % and was in fact the *worse* artefact: the review's independent partner put it **two frames**
out at all three indices where the reference it had replaced was one frame out. That is rule 5 of
§1.8, and it is the reason this section's numbers are now quoted against partners captured
separately. The pictures filed here are the ones the census of §1.8 finds the sessions presenting —
`dbe96c0a…` supported by 6 of 9 sessions at 2300, `05905a2c…` by 8 of 21 at 3000, `d02e362e…` by 3
of 10 at 3400 — and the review's fresh partner holds exactly those three at exactly those indices.

**And the two low ones are recorded as such rather than dressed up.** 8/21 and 3/10 are below half:
at 3000 the two leading pictures are **19 pixels apart**, so the oracle's spread there is a knife
edge that byte-identity splits into groups of 7, 6, 2 and singletons, and at 3400 the largest group
is three sessions. **Phase 2 should read GLENZ's 3000 and 3400 as one exact phase of an animation the
oracle spreads over a few frames**, and 2300 as a picture two thirds of sessions present. No
tolerance is added; the pixels stay exact.

**How far apart, in frames.** Measured inside a partner capture, adjacent indices one frame apart:
2300 moves **25.43 %** of pixels a frame, 3000 **10.64 %**, 3400 **17.07 %**. So the 18.66–34.32 %
that started all this was **≈1.5–2.5 frames**, and a one-frame difference is the whole of what
`compare` may search. **The ±1 window does not reach two frames**, which is why the *filed* picture
has to be one the sessions actually present rather than merely a near neighbour of one.

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
out/b3-tunneli-sweep1    3730,3760,3800,3900,4000,4100,4200,4300,4400,4500,4600,4650,4700,4750  exit 0
out/b3-tunnel-boundaries 3680…3750 and 4740…4790, ten apart — the run in §7.5                     exit 0
```

`b3-tunneli-sweep1.png`'s tiles, in the order the capture laid them out: **3730** black;
**3760** a small dotted ellipse (the tunnel's mouth); **3800** the ring larger; **3900**, **4000**
the tunnel filling the frame; **4100**, **4200** the walls skewing; **4300**, **4400**, **4500**
the tunnel off-centre with its mouth moving up; **4600**, **4650** the dots thinning and the mouth
dark; **4700**, **4750** the last sparse dots. In the boundary run **4740, 4750 and 4760** are all
black while **4770** is TECHNO's first interference ring (§9.1) — i.e. in that session the tunnel
was over by 4740, while in the sweep's session 4750 still had dots. **That run is the one session
§1.7 records as anomalous** (its rings ~27–30 frames early, and the four mode-watch sessions cannot
see the rings' arrival at all); the two VGA-content probes put them at **4797**, after a black
plateau at 4774–4793, so the tunnel's *end* drifts by tens of frames between sessions — the same
phenomenon as §7.4 and §9.5 — and the content boundary is 4797, not 4770 (§8.5).

### 8.2 The keyframes, and why these

| index | what is on screen | frame files | `sha256` |
|---|---|---|---|
| 3800 | the **tunnel's mouth** — a bright ring of dots around a black centre, the part's opening shape. | `.rgb` only | rgb `f861b1f2…` |
| 4200 | the **full tunnel** — dots filling the frame, the mouth skewed. | `.rgb` only | rgb `af4763b3…` |
| 4500 | the **tunnel off-centre**, its mouth up and to the left, walls sweeping across the frame. | `.rgb` only | rgb `cb15de2d…` |
| 4600 | the **thin tunnel** — the same structure with the dots sparse and the mouth dark, the part's last distinctive shape before the screen goes black. | `.rgb` only | rgb `16f8c1fd…` |

Four screens across the one effect it is: the mouth, the full tunnel, the off-centre tunnel, the
thinning one. **4700 was the obvious fourth and was dropped**: the partner capture for it was
refused as a miss (`the emulator reached frame 4702 before frame 4701 could be captured`,
`out/tunneli-recapture`'s first attempt), and the boundary run shows 4740/4750 black — the window
is both grab-hostile and session-dependent, which is not what a reference index should be.

Full hashes of the four files in `reference/parts/tunneli/` — all `.rgb`, no `.idx`. **Almost every
frame of this part taken in the block came back without indices, and it is "almost"**: of the 43
frames in the tunnel's index range (3700–4800) across every capture this block took, exactly **two**
carry `.idx` — `out/b3-tunneli-sweep1`'s 3730 and `out/b3-glenz-sweep2`'s 3750 — and the other 41
were refused with `the screen moved while it was read`. Both exceptions are near-black frames, and
both were looked at by hand as §1.6 requires: 3730's plane is all zeros against an RGB with no
non-black pixel (consistent, and worthless as stride evidence), while 3750's holds exactly 128
non-zero pixels over indices 0/119/121 in rows 63–70 against an RGB with exactly 128 non-black
pixels — the tunnel's first ring, and a small honest validation. Four keyframes were chosen, none of
them one of those two: the part has no *held* screen to hang a keyframe on.

| index | file | `sha256` |
|---|---|---|
| 3800 | `frame_003800.rgb` | `f861b1f2b2f0cd854d91eac3b47c2a9085e0190317c4d12d13bc16851f5e8ddf` |
| 4200 | `frame_004200.rgb` | `af4763b3e0e838bfafec1e7e12f50c87f6e56e9198f9025ed4503ecc799dd00c` |
| 4500 | `frame_004500.rgb` | `cb15de2d852e3abfb757d593c6366fe2a9ce15c2942cf3207b674ff7566290ef` |
| 4600 | `frame_004600.rgb` | `16f8c1fd8ae0372db86e64f4801f0b6b010d1efaca71c672a699248f147f2e88` |

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
(`e4d3646e…`, the reference of that time) matches neither of them; the reference now in the tree is
`f861b1f2…`, and `out/tunneli-recapture2`'s 3800 is `f861b1f2…` too, so
across the five sessions of this block that took 3800 there are **three distinct pictures, one of
them in three sessions** — the quantisation §1.7 records, not a smooth drift, and the same caveat
§5.4 attaches to U2A.

### 8.4 The re-capture: three of four keyframes byte-identical after the group check

```bash
# the reference, re-captured once by §1.8's alignment-group check: its 3800 was a singleton
# where 7 of 9 sessions agree on `f861b1f2…`, and the re-capture's 3800 is in that group
cargo xtask reference capture --arg 2 --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames 3800,4200,4500,4600 --out reference/parts/tunneli --sheet out/tunneli.png
cargo xtask reference capture --arg 2 --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames 3799,3800,3801,4199,4200,4201,4499,4500,4501,4599,4600,4601 --out out/tunneli-recapture3
```

`out/tunneli-recapture3` (exit 0 as a capture):

```
frame 3800: 0.0000% differing (0/64000 px), max delta 0
frame 4200: 0.0000% differing (0/64000 px), max delta 0
frame 4500: 0.0000% differing (0/64000 px), max delta 0
frame 4600: 0.0891% differing (57/64000 px), max delta 97
verdict: different, mode: rgb, offsets {"0": 4}
exit: 1
```

**Three of the four keyframes are byte-identical and the fourth is 57 pixels**, against one fresh
session; the reference is in the largest group on all four (7/9, 3/6, 2/5, 1/6 — §1.8's census),
and 3800's group is the one the *previous* reference was outside of. That is what the alignment-group
check bought: the same `compare`, the same ±1 window, the same indices, and one keyframe that the
oracle reproduces rather than one it does not.

**And the size is worth keeping in frames, not in pixels.** Measured inside a partner capture,
adjacent indices one frame apart: the tunnel moves **2.05–2.22 %** of pixels a frame at 3800,
**10.47–10.69 %** at 4200, **6.22–6.40 %** at 4500 and **8.74–9.08 %** at 4600. So the
8.0672–10.1812 % that `out/tunneli-recapture2` showed on every keyframe — and the 8.2250–10.1672 %
that `out/tunneli-recapture` showed; two partners, one pre-fix reference, §1.8's table — was **≈1
frame at 4200, 4500 and 4600 and ≈4
frames at 3800** — a screen made of moving dots repaints most of what moves in a single frame, which
is why a raw percentage overstates the distance (§1.7), and why a reference four frames out of phase
at 3800 read as 8 % rather than as a wall. **No tolerance is added and the pixels stay exact**; the
earlier partners' numbers are the history of a reference that has been replaced.

### 8.5 Where the part ends

`out/b3-tunnel-boundaries` took 4740…4790 at ten-frame steps, and `out/b3-tunneli-sweep1` took
4650…4750: in the boundary run **4740, 4750 and 4760 are black** and **4770 is TECHNO's first
interference ring**, while in the sweep's session 4700 and 4750 still have the tunnel's last dots.
**The boundary run is the one anomalous session §1.7 records, and its 4770 is not the content
boundary**: the two independent VGA-content probes put the rings at **4797**, after a black plateau
at 4774–4793, within 1–3 frames of each other, and the four mode-watch sessions bound TECHNO's
16-colour transition at 4797–4801 in every session. So the tunnel's last dots are in **(4700, 4770]**
and TECHNO's picture starts at **4797** — one of the boundary sets §3.4, §5.5 and §6.5 have in
common: a content change with no geometry refusal (both screens are 320×200), found by the sheet and
a hash rather than by the capture. That is also why 4700 is not a keyframe (§8.2): it is inside the
window, and the partner capture that asked for it was refused as a miss.

The sizes are worth separating for B4/B5: the *animation* disagreement between sessions at a fixed
index in this part is ≈1–4 frames (§8.4), while the *boundary* moves by tens of frames, because
TUNNELI's exit is gated on its own frame count and the part's start is gated on the music
(`TUNNELI/TUN10.PAS:3`, `veke = 1060`; the tunnel enters after the loader's music restart of
`part-sequence.md` §3).

---

## 9. TECHNO

`SECOND 2`'s fourth part: the techno bars and the EGA interference, then the "panic picture"
(troll) that the next part collapses. **It is the first part this phase has captured whose screens
span two geometries** — a 320×200 screen from 4797 (its first rings; §8.5) to the change in (8150, 8200], and a 320×400
one from there to 8780 — which is B3 step 2's case and is captured here as two captures with a row
each (§1.7,
`reference/parts/techno/interference/` and `reference/parts/techno/troll/`).

**The 320×200 screen is not 256-colour, and that is the part's own doing.** TECHNO switches the
display between VGA 256-colour and EGA 16-colour to make the interference: it writes the graphics
controller's mode register and the attribute controller's colour-select
(`TECHNO/KOE.C:236-258`: `mov ax,0013h+256*20` on the CRTC, then the ATC's index 13h/20h pair),
and DOSBox re-decides the display mode from those registers rather than from the BIOS mode
(`vga.cpp:60-96`, `VGA_DetermineMode`, which returns `M_VGA` only while GC mode bit 6 is set).
Every TECHNO frame on that screen therefore comes back RGB-only with
`indexed capture failed (… not 256-colour …)`, which is a *different* refusal from ALKU's stride
one and from the moving-screen coherence one. The 320×400 screen does carry indices — where the
offset register reads a row the read can describe (§9.6), which is 8200 and 8250 of the frames
below.

### 9.1 The sweeps, and what the sheets showed

```
out/b3-glenz-sweep1   1200…6000 step 400   exit 0 — its 5200…6000 tiles are this part's screen (§7.1)
out/b3-tech-sweep1    5200…9200 step 400   exit 2, refused at 8400 (320×400 — the capture started 320×200)
out/b3-techno-sweep1  4850,4900,4950,5000,5050,5100,5200,5400,5600,5900,6300,6700,7100,7500,7800,8100   exit 0
out/b3-panic-sweep1   8000…8350 step 50    exit 2, refused at 8200 (320×400)
out/b3-panic-sweep2   8200,8250,8300,8350,8400,8450                                       exit 0 (320×400)
out/b3-panic-sweep3   8600,8700            exit 2, refused at 8800 (320×200)
out/b3-panic-sweep4   8800…9500 step 100   exit 0
out/b3-tunnel-boundaries 4740…4790 step 10 exit 0 — its rings at 4770 are the anomalous session
                                           (§1.7); the VGA-content probes put them at 4797 (§8.5)
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
change to ~8290, a white-wash fade at ~8300, and the finished picture held from ~8350 to
8780, where the 320×200 screen returns (§10).

### 9.2 The keyframes, and why these

**The 320×200 screen** (`reference/parts/techno/interference/`):

| index | what is on screen | frame files | `sha256` |
|---|---|---|---|
| 4900 | the **interference rings** — concentric blue rings on black, the part's first drawn screen. | `.rgb` only | rgb `3dc29660…` |
| 5000 | the **white flash** — every pixel is `255,255,255`; the same bytes at 5050 (a hold of at least 50 frames). | `.rgb` only | rgb `20f3d62f…` |
| 6300 | the **diagonal-striped interference** — the moiré evolved into stripes across the frame. | `.rgb` only | rgb `3c2b0d8f…` |
| 8100 | the **crosshatch**, the last screen before the 320×400 mode change. | `.rgb` only | rgb `6033bbc4…` |

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
| `interference/frame_006300.rgb` | `3c2b0d8fa60368394143752e7fcead77e18d823c3d98c561afbd783f6123bee7` |
| `interference/frame_008100.rgb` | `6033bbc4cded77af33513f7147d6153e25ad0064dd5aac600ac097cea9f92003` |
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

### 9.4 The 320×200 re-capture: identical, after the group check

```bash
cargo xtask reference capture --arg 2 --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames 4900,5000,6300,8100 --out reference/parts/techno/interference
cargo xtask reference capture --arg 2 --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames 4899,4900,4901,4999,5000,5001,6299,6300,6301,8099,8100,8101 \
    --out out/techno-recapture3/interference
```

`out/techno-recapture3/interference` (exit 0 as a capture):

```
frame 4900: 0.0000% differing (0/64000 px), max delta 0
frame 5000: 0.0000% differing (0/64000 px), max delta 0
frame 6300: 0.0000% differing (0/64000 px), max delta 0
frame 8100: 0.0000% differing (0/64000 px), max delta 0
verdict: identical, mode: rgb, offsets {"0": 4}
exit: 0
```

**All four keyframes are byte-identical**, against one fresh session, with the reference in the
largest group on all four (4/5, 6/6, 6/7, 4/5 — §1.8) and with 6300 and 8100 that were singletons
before the re-capture. The two earlier partners' numbers (24.5938 %, 72.8703 % and 70.6844 %, then
0.0000 %, 0.0000 %, 97.5047 % and 71.7000 %) are the history of a reference that has been replaced —
and they are also the reason the check exists: with the old reference, *which* frames reproduced
looked like a property of timing, because the reference was simply outside the group.

**And on these two screens the percentage is a lower bound with no room in it at all.** Adjacent
indices one frame apart inside the partner capture: 4899→4900 **24.67 %**, 4900→4901 **24.91 %**
(the rings screen), 6299→6300 **96.33 %**, 6300→6301 **98.04 %**, 8099→8100 **98.69 %**, 8100→8101
**89.10 %** (the dither). So the older partners' 72.8703 % and 97.5047 % at 6300, and 70.6844 % and
71.7000 % at 8100, were **≈one-frame** statements all along — the screen saturates — and none of them
could witness a gap larger than a frame. That is the correction §1.7 records: read a moving row's
percentage next to the screen's own per-frame rate before calling it a distance, and read
`compare`'s offset as the ±1 window it searched, not as the gap.

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

**The first capture of this reference was a session whose timeline did not match, it was caught and
re-taken — and what is left of it is one session's observation, not a measurement anyone can
re-derive.** In it the troll picture was already on screen at 8200 and 8300, where the partner,
`b3-panic-sweep2`, `techno-troll-map2` and the re-captured reference all show flat black at 8200,
the wash at 8300 and the picture from 8350. **The frames no longer exist**: the re-capture writes
into the same directory and `out/techno-troll.png` was overwritten with it, so all that survives is
what was printed at the time — 8200's `sha256` was `a0949c8436d1087…` and both 8200 and 8300 had 78
distinct colours and 93,200 non-black pixels, the finished picture. Read it as **one session
observed doing something three others did not**, with no artefact a reviewer can check, and as the
reason a single capture is not enough to claim an index means one screen. If the difference was
real it is ≥150 frames of this section's timeline, which is the size §1.7 attributes to a
music-gated event (the fade waits on `muscode 6`, `TECHNO/KOE.C:638-649`) rather than to the
animation phase.

### 9.6 Where the part ends, and what the 320×400 screen's rows really are

Two things belong here, and the first corrects this section's own first version.

**The 320×400 screen is not one row stride for its whole span.** `out/b3-techpanic-boundary.log`
holds **nine `PlaneTooSmall` refusals** on 8700…8780 — `this screen's rows are 320 bytes and it is
400 rows tall, so it needs 128000 bytes a plane; one plane holds 65536, and the display wraps` — so
the offset register named a **320-byte plane row** there, not the 160-byte one the successful
8200/8250 frames were read at. In two other sessions the same 8700 is refused only for motion
(`out/b3-panic-sweep3`, `out/techno-troll-probe1`), with byte-identical RGB (`40044edc…`), so the
displayed layout cannot differ between them: what differs is the register the tool's two-operation
read returned (§1.7), and 0xa0 — the value it named — is exactly one of the eight `plv * 0x20` values
`TECHNO/KOE.C:385-389` writes to CRTC **0ch**, not 13h. The row's geometry column says 160-byte plane row because that is what the
frames that carry `.idx` were read at, and this is what the rest of the span did.

**And the part ends at the geometry change in (8780, 8790]**, where PANICEND takes the screen down
to 200 lines. It is TECHNO's own second screen that the 8400 refusal belongs to — a capture started
at 5200 (320×200) refusing at 8400 because that index is 320×400 — **not** the next part's: after
8780 the 320×400 screen is gone and the section's last index taken is 8780 (`40044edc…`, the held
picture). `TECHNO/KOE.C:588-616` is what put it there: mode 13h, `inittwk()`, CRTC 13h = 80, then
`TROLL.UP` blitted 400 rows deep. The music-cue wait and MNTSCRL's 320×200 screen come after
PANICEND, at ~9230 (§10.4), and there is no geometry refusal at *that* boundary at all.

---

## 10. PANICEND

`SECOND 2`'s fifth part: the fake ending — TECHNO's picture collapsing to a line and then to a
single dot on black. 320×200, from the geometry change in (8780, 8790], whose first visible effect
is the 320×400 screen's 400 lines being halved back to 200 (the `SHUTDOWN.C` asm writes
CRTC 09h = 41h,
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

**All three are byte-identical, including the moving one** — and 8792's bytes are the same in
three separate sessions (`out/panic-probe1`, the reference and the partner all hold
`c9bf5114b3e70b84…`), so the agreement is not one lucky pair. 8792 is a frame in the middle of the
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

---

## 11. MNTSCRL

`SECOND 3`'s first part, and the first row in this ledger from a `SECOND 3` boot. Its source is
`FOREST/` (staged as `mntscrl.exe` by `FOREST/X.BAT:1`); the runner restarts MUSIC1 at order 42
before it (`MAIN/U2.ASM:860-863`, the `[entry]` site guarded by `whattorun & 4` and not
`whattorun & 2`, i.e. exactly `SECOND 3`) and then runs `exe9` (`:865-866`).

**What the part does.** `READ2.PAS:101-104` sets mode 13h and calls `dis_partstart`; `:110-115`
reads 31 rows of 640 bytes out of `O2.SCI`; `:120-123` takes its palette from `hback+10` and its
64000-byte picture from `hback+778` (`BGR.PAS:2-6` links `HILLBACK.CLX` as `hback`, built by
`MAKEOB.BAT`), blits it to `$a000` and sets the DAC to all-black (`:124-126`); `:139-147` fades the
palette up over 64 frames; `:153-159` **waits for a music cue** (`mov bx,6 / int 0fch`, `cmp dx,0 /
jl @a` — loop while `muscode` is negative); `:170-181` runs 128 more frames while `scr(sss)` cycles
the drawing phase; and `:195-224` is the main loop, whose exit is the music (`dx = -11` at `:200`
sets `fadeout`), not a frame count: `{inc(frame);}` is **commented out** at `:221`, so the
`frame = veke` half of the `until` at `:224` can never fire and `veke = 2800` (`:3`) never ends
anything. `scr()` (`:56-93`) does the drawing: `ROUTINES.ASM`'s `Putrouts` walks the mask data and
writes `background byte + mask byte` per pixel, and the `pos = 2` arm shifts the mask one byte with
`move(font[1],font[0],237*31)` (`:86`) and pulls one new column out of `O2.SCI` with `scp`
(`:87-88`), walking `scp` from 133 (`:151`) to 639. The screen is that background with a scrolling
leaf overlay added over it — 320×200 the whole way, mode 13h's 80-byte plane rows, so nothing here
exercises the stride fix.

### 11.1 The sweeps, and what the sheets showed

```
out/b4-mnt-sweep1        400,600,…,2000 step 200            exit 2, refused at 400 as a MISS
out/b4-mnt-start1        405,435                            exit 2, refused at 405 as a MISS
out/b4-mnt-start3        415,445                            exit 2, refused at 415 as a MISS
out/b4-mnt-start2        425,455                            exit 0
out/b4-mnt-start4        420,450                            exit 0
out/b4-mnt-sweep1        440,600,…,2000 step 200            exit 0 (9 frames)
out/b4-mnt-fade1         425,430,…,500 step 5               exit 0 (16 frames)
out/b4-mnt-fade2         505,515,…,615 step 10              exit 0 (12 frames)
out/b4-mnt-sweep2        2200,2500,…,4600 step 300          exit 0 (9 frames)
out/b4-mnt-lns-boundary  2100,2150,…,2500 step 50          exit 0 (9 frames)
out/b4-mnt-lns-edge      2440,2450,…,2490 step 10          exit 0 (6 frames)
out/b4-mnt-motion1       1499-1505 and 2099-2105            exit 0 (14 frames)
out/b4-mnt-hold-edge     500,545,555,560,565               exit 0 (5 frames)
out/b4-mnt-sess1 … -sess6  499,500,501,1099,1100,1101,1699,1700,1701,2299,2300,2301   exit 0 each (12 frames)
```

The three refusals are all the same failure and they locate the part's first graphics frame: the
mode change stalls the renderer, and **400, 405 and 415 are grab-hostile** ("the emulator reached
frame 420 before frame 405 could be captured", "reached 419 before frame 415"). **420 captures and
is already 320×200** — flat black, `ea0787f6…`, the same bytes as every other black screen in this
ledger (§4.2, §7.5) — so MNTSCRL's first graphics frame is in **(415, 420]** and indices just below
it cannot be taken at all. This is §1.2's grab-hostile window seen from the other side: B2 met it
just *below* a changeover and found the change by narrowing; here the change is inside the window.

Tiles, in the order each run laid them out. `sweep1` (the 9 that captured): **440** the leaf
overlay only, very dark, on black; **600** the same overlay over the landscape, full brightness;
**800 / 1000 / 1200 / 1400 / 1600 / 1800 / 2000** the landscape with the leaf branch across it,
shifting. `fade1` fills in the opening at five-frame steps: a fade (**425** mean 6.08, **430** 8.91,
**435** 10.25, **440** 10.53) that saturates at **445** and does not move again. `fade2` shows the
hold ending: the first 19-pixel change at **555** and the background appearing at **560-615** (mean
15.48 → 47.65 as its palette entries fade up). `sweep2` and `lns-boundary` show the part still
scrolling at **2200-2350**, fading out at **2400** (mean 14.54), black at **2450**, and the next
part's face already fading in at **2460** (mean 16.13) — §11.5.

### 11.2 The keyframes, and why these

| index | what is on screen | frame files | `sha256` |
|---|---|---|---|
| 500 | the **held opening** — the leaf overlay alone against black, its palette faded up to its resting values; byte-identical from 445 to 545, and the same bytes in all 8 sessions that took it. The part's own music wait (`READ2.PAS:153-159`) is this hold. | `.rgb` + `.idx` | rgb `e68aa887…`, idx `7be5af37…` |
| 1100 | the **scroller**, early — the landscape with the leaf branch in the middle of the frame and its branches reaching up and right. | `.rgb` only | rgb `deb4d8fd…` |
| 1700 | the **scroller**, middle — the same landscape with the branch shifted left and the pale ridge of the background to the left of it. | `.rgb` only | rgb `dc205228…` |
| 2300 | the **scroller**, late — the branch over the middle-right of the landscape, ~80 frames before the fadeout starts. | `.rgb` only | rgb `eb4598d4…` |

Four keyframes: one for the held opening, three spread 600 frames apart across the ~1900-frame
scroll (which is one continuous motion — §11.3 measures no hold anywhere inside it), the last
ending ~80 frames short of the fadeout so it is not in a palette fade. They are 600 frames apart,
far more than the ~60 a three-index support batch needs (§1.3).

Full hashes of the five files in `reference/parts/mntscrl/`:

| index | file | `sha256` |
|---|---|---|
| 500 | `frame_000500.rgb` | `e68aa8870c98cbc4757f46dbea41ccb8dbbe68bbd2b5526c1d8eb05c9a5d7646` |
| 500 | `frame_000500.idx` | `7be5af378a454a4d715e88bd5db721ef38e325c7060d618febaaaeb20a0ca5da` |
| 1100 | `frame_001100.rgb` | `deb4d8fdd3b08538dcde01f82da7a6e3d462fb4d41de673c3e95fdf866e9413f` |
| 1700 | `frame_001700.rgb` | `dc2052288fc3a2152df85fbf2d59bcf339edcf873a0a5f4f6a3bebab1511eba6` |
| 2300 | `frame_002300.rgb` | `eb4598d4c64ecebb29adb226f5f7035f915679ed56acc8af9e1a63a68aa112f9` |

**500's `.idx` was checked by hand, and the plane is honest.** It carries 107 distinct indices and
**no zero bytes at all** (the palette, not the index plane, is what makes the picture look dark),
and every one of the 64000 pixels has *exactly one* RGB triple for its index — 0 indices with more
than one. So §1.6's flat-colour blind spot does not apply here: the widening rule reproduces the
RGB from the plane (idx 42 ↔ `(24,65,24)`, idx 71 ↔ `(73,65,49)`, idx 126 ↔ `(49,49,49)`, each the
`(c*259+33)>>6` widening of the manifest's 6-bit entry), and the 24,620 non-black RGB pixels are
exactly the 24,620 pixels whose index widens to something other than black.

### 11.3 Held or moving, measured

| keyframe | neighbours | reading |
|---|---|---|
| 500 | 499 = 500 = 501 (`out/b4-mnt-sess1`, and the same in sess2–sess6); 445…500 in `out/b4-mnt-fade1` and 500, 545 in `out/b4-mnt-hold-edge` are one `sha256` | **held** |
| 1100 | 1099 / 1100 / 1101 three `sha256`; 1552 px (2.4250 %) and 1542 px (2.4094 %) apart | **moving** |
| 1700 | 1699 / 1700 / 1701 three; 1554 px (2.4281 %) and 1576 px (2.4625 %) | **moving** |
| 2300 | 2299 / 2300 / 2301 three; 1972 px (3.0812 %) and 1960 px (3.0625 %) | **moving** |

The per-frame numbers are measured **inside one capture**, between adjacent indices, so they are
the screen's own rate and not a session-to-session difference (§1.7's rule for reading a
percentage). The hold is bounded by measurement on both sides: 440 → 445 is a **269-pixel
(0.4203 %)** step — the palette's last one, and 6998 px (10.9344 %) is the *previous* step, 435→440 —
so the hold starts in (440, 445]; 545 → 555 is a 19-pixel (0.0297 %) step, so it ends in
(545, 555). Nothing inside the scroll holds: 425→430 is 20325 px (31.7578 %, the fade),
555→560 is 39423 px (61.5984 %, the background's palette fade), and every step measured between
1099 and 2301 is 2.4–3.1 % — there is no pair of identical frames anywhere in that span.

**And unlike GLENZ's and TUNNELI's moving screens (§1.8), this part's moving frames are the *same
bytes at the same index in every session*.** Every index that two captures both hold agrees exactly,
whatever the gap between them (the pairs below are 2 minutes to half an hour apart, and both sweeps
in the list were taken before the reference existed):

| index | sessions that took it | `sha256` |
|---|---|---|
| 425 | `b4-mnt-fade1`, `b4-mnt-start2` | `0f2daa91da02…` both |
| 440 | `b4-mnt-sweep1`, `b4-mnt-fade1` | `86eeadce81e9…` both |
| 455 | `b4-mnt-fade1`, `b4-mnt-start2` | `e68aa8870c98…` both |
| 500 | `b4-mnt-fade1`, `b4-mnt-hold-edge`, `b4-mnt-sess1` … `sess6` (8) | `e68aa8870c98…` all |
| 2100 | `b4-mnt-lns-boundary`, `b4-mnt-motion1` | `0e99c02d4f26…` both |
| 2200 | `b4-mnt-lns-boundary`, `b4-mnt-sweep2` | `291f3e851e1f…` both |
| 2500 | `b4-mnt-lns-boundary`, `b4-mnt-sweep2` | `6a152a319d26…` both |

The mechanism is the one §1.7 identified for PANICEND: the part's timeline is anchored to *one*
music cue and then advances by `dis_waitb()` counts, so a session difference in when the cue lands
does not survive into the part — every session draws the same frame at the same boot index. The
`READ2.PAS:153-159` wait is that anchor.

### 11.4 The re-capture and the census

```bash
# the reference: keyframes only
cargo xtask reference capture --arg 3 --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames 500,1100,1700,2300 --out reference/parts/mntscrl --sheet out/mntscrl.png

# the partner: a support frame each side of every keyframe (500's are the same bytes)
cargo xtask reference capture --arg 3 --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames 499,500,501,1099,1100,1101,1699,1700,1701,2299,2300,2301 --out out/b4-mnt-sess1
```

```
$ cargo xtask compare --reference reference/parts/mntscrl --rendered out/b4-mnt-sess1 \
      --out out/b4-mnt-recapture-diff
frame 500: 0.0000% differing (0/64000 px), max delta 0
frame 1100: 0.0000% differing (0/64000 px), max delta 0
frame 1700: 0.0000% differing (0/64000 px), max delta 0
frame 2300: 0.0000% differing (0/64000 px), max delta 0
verdict: identical, mode: mixed, offsets {"0": 4}
exit: 0
```

`out/b4-mnt-sess4` and `out/b4-mnt-sess6` — two **more independent sessions**, separate captures
(the six span about ten minutes of wall-clock, the reference among them) — give the identical report
(`exit 0`, `verdict: identical`, 0.0000 %, offsets `{0: 4}`, `mode: mixed`). No headline number here
comes from a single partner (§1.8 rule 5), and the agreement is cross-checked rather than assumed:
discovery runs taken **before the reference was filed** already hold the same bytes at 500
(`out/b4-mnt-fade1`, `out/b4-mnt-hold-edge`) and at 2300 (`out/b4-mnt-lns-boundary`).

**The census**, by §1.8's rule — captures that hold `N-1, N, N+1` (the six sessions above), the
reference's own directory excluded:

| keyframe | sessions with a window | pictures at `N` | largest support | filed reference is it? | ≥ half the sessions? |
|---|---|---|---|---|---|
| MNTSCRL 500 (held) | 6 | 1 | `e68aa887…` **6/6 (100 %)** | **yes** | yes |
| MNTSCRL 1100 | 6 | 3 | `deb4d8fd…` **6/6 (100 %)** | **yes** | yes |
| MNTSCRL 1700 | 6 | 3 | `dc205228…` **6/6 (100 %)** | **yes** | yes |
| MNTSCRL 2300 | 6 | 3 | `eb4598d4…` **6/6 (100 %)** | **yes** | yes |

Every one of the six sessions presents the filed picture **at `N`** (offset 0), not merely
somewhere in the window — so the support count and the `0.0000 %` pairs are the same fact here, and
this is the one part in the ledger whose moving keyframes are not a phase of an animation the
oracle spreads over several frames but a picture every session draws at that index.

### 11.5 Where the part begins and ends

`out/b4-mnt-lns-edge` took 2440…2490 in tens:

| index | 2440 | 2450 | 2460 | 2470 | 2480 | 2490 |
|---|---|---|---|---|---|---|
| `sha256` | `fd5afb16…` | `ea0787f6…` | `5217d4e0…` | `40732954…` | `1ee6e8fb…` | `6a152a31…` |
| what | the scroller, faded to mean 3.92 | flat black | mean 16.13 | mean 35.04 | mean 38.95 | the next part's face, full |

The last full-brightness scroller frame is **2350** (mean 56.77); 2400 is the fadeout (mean 14.54,
every pixel changed — the palette dropping); 2450 is the black gap; **the next part's first frame is
in (2450, 2460]** and LNS&ZOOM's face is *fading in* at 2460–2480 (61 distinct colours at 2460
against 64 at 2490) before it holds at 2490 — 2490 and 2500 are the same bytes (`6a152a31…`) and
carry `.idx`. So **MNTSCRL is ~2030 frames of the `SECOND 3` boot, from its first graphics frame in
(415, 420] to a last drawn frame at ~2445, ≈29 s** — and, as at U2A→PAM and GLENZ→TUNNELI, **the
boundary is a content change with no geometry refusal at all**: both screens are 320×200, so
nothing in `--frames` can find it, and the sheet is the discovery (§1.6). The black gap is two or
three frames wide — narrower than the tens of frames GLENZ leaves — and it is the same
`ea0787f6…` black as everywhere else.

---

## 12. LNS&ZOOM

`SECOND 3`'s second part, and one binary holding two effects — `LENS/MAKEFILE:26` builds `lens.exe`
and stages it as `lns&zoom.exe`. It is three routines one after another: `part1` (`LENS/MAIN.C:89-131`),
`part2` (`:133-183`, the lens) and `part3` (`:185-272`, the rotazoomer), run from `main` as
`part1(); while(dis_musplus()<-20); part2(); part3();` (`:379-386`). 320×200 mode 13h throughout
(`setvmode(0x13)`, `:292`), and every frame of it carries `.idx` only when the palette happens to be
still while the read runs — the lens and the rotazoomer both move a palette area every frame, so
most of the part is RGB only.

**What is on screen.** `part1` wipes the demon-face picture onto the screen with two vertical edges
walking across it (`cp[x]=dp[x]`, `:114-129`), for at most 300 frames (`:107`), after a music wait
(`:104`). `part2` runs 715 frames with `drawlens(x,y)` steering a bubble along `pathdata1` (`:159-165`),
and `part3` is the rotazoomer: `rotpic90=back` (`:191`), a rotated/zoomed copy of the face, drawn
until `frame = 2000` (`:211`) or a music break (`:213`), fading out from its frame 1872 (`:261-267`).
It ends by loading the palette with 63 everywhere — `for(a=0;a<768;a++) palette[a]=63;` (`:274-275`)
— which is the **flat white screen** 5650 is.

**The part's extent** is **from its first frame in (2452, 2456] to the white screen that is still up
at 5720** — about 3270 frames, ≈47 s, the longest part in this ledger so far.

### 12.1 The sweeps, and what the sheets showed

```
out/b4-mnt-lns-boundary  2100,2150,…,2500 step 50          exit 0 (9 frames)  — §11.5
out/b4-mnt-sweep2        2200,2500,…,4600 step 300          exit 0 (9 frames)
out/b4-lns-start1        2452,2456,2460,2464,2468           exit 0 (5 frames)
out/b4-lns-sweep2        2500,2550,…,3200 step 50           exit 0 (15 frames)
out/b4-lns-sweep3        3300,3400,…,4300 step 100          exit 0 (11 frames)
out/b4-lns-sweep1        4700,4900,…,6900 step 200           exit 2, refused at 5900 (320×200 → 320×400)
out/b4-lns-end1          5450,5500,…,5750 step 50            exit 2, refused at 5750 (same change)
out/b4-plz-start1        5710,5720,5730,5740                exit 2, refused at 5730 (same change)
out/b4-lns-hold2         3560,3580,…,3680 step 20            exit 0 (7 frames)
out/b4-lns-sess1 … -sess5  2699,2700,2701,…,5649,5650,5651  exit 0 each (15 frames)
```

Tiles. `b4-lns-sweep2`: **2500 … 2900** the face, one picture; **2950** a dark blue lens arriving at
the top-left; **3000 / 3050 / 3100 / 3150 / 3200** the lens swimming across the face — left eye,
centre, right eye, lower right. `b4-lns-sweep3`: **3300** the lens at the lower left; **3400** the
lens over both eyes; **3500** the lens leaving to the right; **3600** the plain face again; **3700**
the face brighter, mid-change; **3800 / 3900 / 4000 / 4100 / 4200 / 4300** the rotazoomer — the face
rotated, magnified and sliding, with the blue bubbles scattered over black. `b4-lns-sweep1` (the
five that captured): **4700 / 4900 / 5100 / 5300** the rotazoomer in different phases; **5500** it
dark (mean 9.35). `b4-lns-end1`: **5550** mean 94.44, **5600** mean 177.10, **5650 = 5700** one
`sha256` at mean 255.00 — **flat white**.

### 12.2 The keyframes, and why these

| index | what is on screen | frame files | `sha256` |
|---|---|---|---|
| 2700 | the **face**, held — the demon face with the pentagram and yellow eyes, blue bubbles at the corners; byte-identical from 2490 to ~2945. | `.rgb` + `.idx` | rgb `6a152a31…`, idx `70175500…` |
| 3400 | the **lens** — the bubble crossing the face over both eyes, the face behind it distorted. | `.rgb` only | rgb `6c81eadc…` |
| 4200 | the **rotazoomer**, early — the rotated face at the right of the frame with the blue bubbles over black. | `.rgb` only | rgb `19e5682f…` |
| 4900 | the **rotazoomer**, late — a magnified face fragment filling the lower half, bubbles above it. | `.rgb` only | rgb `248f6b83…` |
| 5650 | the **white screen** the part ends on, held — one colour, 64000 pixels, `(255,255,255)`. | `.rgb` + `.idx` | rgb `20f3d62f…`, idx `f8636cd3…` |

One keyframe per screen the part draws: the face it holds on, the lens, the rotazoomer at two points
through its ~1790 frames, and the white flash it exits on. They are 700–800 frames apart, well over
the ~60 a three-index support batch needs (§1.3). 2700's picture is the same bytes as `2490`, the
frame MNTSCRL's black gap is followed by (§11.5) — the same face, drawn by the same blit, filed
under two parts.

Full hashes of the seven files in `reference/parts/lnszoom/`:

| index | file | `sha256` |
|---|---|---|
| 2700 | `frame_002700.rgb` | `6a152a319d26f6c4f0c6cea6f9127f5a5b7b07c00da1d99a215446e1d4d21fd3` |
| 2700 | `frame_002700.idx` | `70175500011ef854281c7ff89a82b449269ec15081a89ff181da84de60c55509` |
| 3400 | `frame_003400.rgb` | `6c81eadc9c9f119cb3cbcbc2baee37733e7feff0bb47a2d733d915a4d2cb4d63` |
| 4200 | `frame_004200.rgb` | `19e5682fdaa94598ea87b8236286807069c95546f855e65455b4db38107c5430` |
| 4900 | `frame_004900.rgb` | `248f6b830e146d0454dc2b8a15f46c6030f8a1704b22626150736d7edce3b93f` |
| 5650 | `frame_005650.rgb` | `20f3d62fced3ac8938933a1f3aca1dbe92b058808f88bdea2937ce5d788f9df8` |
| 5650 | `frame_005650.idx` | `f8636cd395797afa4aa9ecad7839711fc32df00209fb7163a2690e8901f1788e` |

**Both `.idx` files were checked by hand, and one of them cannot vouch for anything.** 2700's plane
holds 64 distinct indices over 64000 pixels, every index has exactly one RGB triple, and the
`(c*259+33)>>6` widening of the manifest's palette reproduces the RGB with **0 mismatches** — an
honest plane on a picture, and the check that would catch a stride error if mode 13h's stride were
wrong. **5650's cannot do that job**: its palette is 63 in every one of its 63 used entries, so
every index widens to white and the coherence check passes whatever the plane says. Read by hand, the
plane is *not* flat — 63 distinct indices, 50308 of them zero, the leftover index picture of the
rotazoomer's last frame under a white palette — and it is the same bytes in all five sessions that
took it, so it is reproducible. But it is the §1.6 blind spot in its purest form: **the white
keyframe's `.idx` is evidence of nothing about the stride**, and Phase 2's indexed comparison of
5650 compares a plane no one can check visually.

### 12.3 Held or moving, measured

| keyframe | neighbours | reading |
|---|---|---|
| 2700 | 2699 = 2700 = 2701, one `sha256`, and 2500…2900 sampled at 50 (9 points) are the same bytes in `b4-lns-sweep2` | **held** |
| 3400 | three `sha256`; 4312 px (6.7375 %) and 4367 px (6.8234 %) a frame | **moving** |
| 4200 | three `sha256`; 10052 px (15.7063 %) and 8248 px (12.8875 %) a frame | **moving** |
| 4900 | three `sha256`; 25440 px (39.7500 %) and 24876 px (38.8687 %) a frame | **moving** |
| 5650 | 5649 = 5650 = 5651, one `sha256`, and 5700 is the same bytes again | **held** |

All the per-frame numbers are measured inside one capture between adjacent indices. The face's hold
is bounded on both sides: 2950 is 2940 px (4.5938 %) away from 2900, so the hold ends in (2900, 2950],
and the lens phase ends by returning the screen to *exactly* the hold's picture — 3600, 3620 and 3640
are `6a152a31…` again, 3580 is 3051 px (4.7672 %) away from 3600 and 3660 is 64000 px (100 %) — so
the face is held a second time for ~60 frames at (3580, 3660) before the rotazoomer starts. The
rotazoomer is the fastest screen in this part: 3700→3800 is 93.3438 % of pixels, 3800→3900
98.9062 % and 3900→4000 98.1312 %, so a one-frame phase difference there moves most of the
picture (§1.7).

### 12.4 The re-capture and the census

```bash
cargo xtask reference capture --arg 3 --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames 2700,3400,4200,4900,5650 --out reference/parts/lnszoom --sheet out/lnszoom.png
cargo xtask reference capture --arg 3 --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames 2699,2700,2701,3399,3400,3401,4199,4200,4201,4899,4900,4901,5649,5650,5651 \
    --out out/b4-lns-sess1
```

```
$ cargo xtask compare --reference reference/parts/lnszoom --rendered out/b4-lns-sess1 \
      --out out/b4-lns-recapture-diff1
frame 2700: 0.0000% differing (0/64000 px), max delta 0
frame 3400: 0.0000% differing (0/64000 px), max delta 0
frame 4200: 0.0000% differing (0/64000 px), max delta 0
frame 4900: 0.0000% differing (0/64000 px), max delta 0
frame 5650: 0.0000% differing (0/64000 px), max delta 0
verdict: identical, mode: mixed, offsets {"0": 5}
exit: 0
```

`out/b4-lns-sess3` and `out/b4-lns-sess5` — independent sessions, separate captures about ten
minutes apart — give the same report (`exit 0`, `verdict: identical`, 0.0000 %, offsets `{0: 5}`,
`mode: mixed`). The corroboration that does not rest on that spacing: the discovery runs captured
**before the reference** (`out/b4-lns-sweep2`, `-sweep3`, `-sweep1`, `-end1`) hold the same bytes at
every one of the five keyframes — all five checked by hash, not assumed.

**The census**, over the five sessions that hold a window (the reference's own directory excluded):

| keyframe | sessions with a window | pictures at `N` | largest support | filed reference is it? | ≥ half the sessions? |
|---|---|---|---|---|---|
| LNS 2700 (held) | 5 | 1 | `6a152a31…` **5/5 (100 %)** | **yes** | yes |
| LNS 3400 | 5 | 3 | `6c81eadc…` **5/5 (100 %)** | **yes** | yes |
| LNS 4200 | 5 | 3 | `19e5682f…` **5/5 (100 %)** | **yes** | yes |
| LNS 4900 | 5 | 3 | `248f6b83…` **5/5 (100 %)** | **yes** | yes |
| LNS 5650 (held) | 5 | 1 | `20f3d62f…` **5/5 (100 %)** | **yes** | yes |

As with MNTSCRL, every session presents the filed picture **at `N`** — including the three screens
that move 6–40 % of their pixels a frame — so the support counts and the `0.0000 %` pairings are the
same fact. The index planes agree too: 2700's `.idx` is `70175500…` and 5650's `f8636cd3…` in all
five sessions and in their `±1` neighbours.

### 12.5 Where the part begins and ends

The beginning is §11.5 read from the other side: MNTSCRL's last drawn frame is ~2445, **2450 and
2452 are flat black** (`ea0787f6…`), and **2456 is already the face at mean 7.53** — so LNS&ZOOM's
first frame is in **(2452, 2456]**, the black gap is three or four frames wide, and this boundary is
also a content change with no geometry refusal. The face's palette then rises (2460 mean 16.13, 2480
38.95) and settles by 2490 (mean 40.30, 25480 non-black pixels), which is the hold's first frame
that is byte-identical to 2700.

The end is a geometry change, so the refusal finds it: `b4-lns-sweep1` refused at **5900**, `end1` at
**5750** and `plz-start1` at **5730** — each `320x200 → 320x400`. Narrowed by `plz-start1`, **5710 and
5720 are still the white** (one `sha256` with 5650) and **5730 is already 320×400**, so the white
that `part3`'s `palette[a]=63` (`MAIN.C:274-275`) leaves on the screen holds to ~5725 and PLZPART's
Mode X screen follows it directly. Between 5450 (the rotazoomer, mean 39.59) and 5650 (white) the
screen goes through the rotazoomer's own fade (5500 mean 9.35) and back up (5550 94.44, 5600 177.10)
— the palette reaching white, with the picture still in the index plane underneath.

---

## 13. PLZPART

`SECOND 3`'s third part, and the part the split convention was written for. `PLZPART/X.BAT:1` stages
`plzpart.exe`; `PLZPART/MAIN.C` runs `init_copper(); initvect(); plz(); vect(); close_copper()`, and
those two calls are **two different screens**:

- **plasma** (`plz()`, `PLZ.C:62-121`): `init_plz()` (`:171-179`) calls `tw_opengraph2()`
  (`PLZPART/TWEAK.ASM:44-67`) — chain-4 off, CRTC 0x14 = 0, 0x17 = `0xe3`, 0x09 = 0 (400 lines),
  **0x13 = `0x3013h` → 48 units → a 96-byte plane row**, and 400 rows. `set_plzstart(60)`
  (`PLZ.C:177`) writes CRTC 0x18 = 60 through `ASMYT.ASM:97-119`. The displayed screen is 320×400
  with a **line-compare split at 60**; the plasma itself is drawn into video memory by `plzline`
  (`PLZ.C:99-118`, `0x0a000 + y*6 + YADD*6` per line), and the split's reset is what shows part of
  that memory in the lower region — where it starts and where it ends is measured in §13.2.
- **cube** (`vect()`, `VECT.C:113-131`): `tw_opengraph()` (`TWEAK.ASM:19-37`) — CRTC 0x09 = 2,
  **0x13 = `0x5013h` → 80 units → a 160-byte plane row**, 200 rows. This is the register the brief
  named: **the memory layout is twice as wide as the 320-pixel picture**, and the old
  `width / 4` = 80 assumption would have sheared every index plane of it.

Both screens are 256-colour planar (`ScreenLayout::Planar256`), so the read's stride is
`scan_len * 2`: 96 for the plasma's 0x30 and 160 for the cube's 0x50. **Every frame of both screens
came back RGB-only anyway** — the plasma and the cube each redraw the whole picture every frame
(§13.5 measures 24–70 % of pixels a frame) — so no `.idx` from this part exercises either stride, and
the comparison runs in `mode: rgb`. What the stride arithmetic *did* do here is refuse a screen it
could not describe, twice, for a reason that is the tool's and not the part's — §13.4.

### 13.1 The sweeps, and what the sheets showed

```
out/b4-plz-start1       5710,5720,5730,5740            exit 2, refused at 5730 (320×200 → 320×400)
out/b4-plz-sweep1       5800,6000,…,8000 step 200      exit 0 (12 frames, 320×400)
out/b4-plz-sweep2       8200,8400,…                    exit 2, refused at 8400 (320×400 → 320×200)
out/b4-plz-cube-edge1   8250,8300,8350                 exit 2, refused at 8350 (same change)
out/b4-plz-cube-edge2   8310,8340                      exit 2, refused at 8340 (same change)
out/b4-plz-cube-sweep   8500,9000,…,12500 step 500     exit 0 (9 frames, 320×200)
out/b4-plz-minv-edge    10100,10200,10300,10400       exit 0 (4 frames, 320×200)
out/b4-plz-probe1       6989,6990,6991, 8289,8290,8291, 10151…   exit 2, refused at 10151 (320×400 → 320×200)
out/b4-plz-idx1         6000,7000                      exit 0 (2 frames)
```

Tiles. `b4-plz-sweep1` (the twelve, all 320×400): **5800** a pale, washed-out plasma (the part's
`cop_drop=128` entrance), **6000 / 6200 / 6400 / 6600** a blue-and-magenta plasma in the middle of
the frame with flat bands above and below — black by 7000 and `(4,4,4)` at 6000 — **6800 / 7000 /
7200** green and blue, **7400 / 7600**
cyan and orange, **7800 / 8000** almost black with a faint grey shape. `b4-plz-cube-sweep`:
**8500** a small rainbow-textured cube in the upper left over black, **9000** the cube filling the
frame, **9500** it larger still and open, **10000** a green plasma face across the whole frame,
**10500** black above and a grey gradient below (the next part's backdrop), **11000 / 11500 / 12000 /
12500** small blue dots scattered and rotating over that gradient (MINVBALL, §14).

The plasma's shape is the split, and it is visible in every tile from 6800 on: **rows 0-60 black,
rows 61-340 the plasma, rows 341-399 black** — 280 rows of content on a 400-row screen.

### 13.2 The split, measured — and where the capture puts it

This is the first capture in the tree that can say where the hardware's line-compare register lands,
and it says `line_compare + 1`, which is what `Split::scanline` means (D16).

`out/b4-plz-probe1` took three consecutive frames on the plasma screen, and their first non-black
row is not the same for all three:

| index | first row with a non-black pixel | last |
|---|---|---|
| 6989 | **62** | 341 |
| 6990 | **61** | 340 |
| 6991 | **62** | 341 |

The split alternates 61/62 frame to frame, and that is exactly `COPPER.ASM:126-143`'s `pompota`,
which sets CRTC register 0x18 to `0x3c` (60) on one frame and `0x3d` (61) on the next (with the 8th
bit cleared through register 0x07 at `:126-129`). **The displayed first line of the lower view is the
register's value plus one, 60→61 and 61→62, measured on the hardware rather than inferred from the
emulator's source.** The port writes `Split { scanline: 61 }` and `62` to reproduce these two frames.

Two caveats the measurement carries: the upper view is what is black above the split (those frames'
CRTC start address, `cop_start = 96*(682-400) = 27072` set at `PLZ.C:176`, points past the drawn
plasma), so the *boundary* measured here is where the lower view begins and not a picture change; and
the split does not stay at 60/61 for the whole part — `do_drop` (`COPPER.ASM:189-268`) walks the same
register up the screen at the end of each plasma phase, and `b4-plz-probe1`'s 8289/8290/8291 show the
first content row at **195 / 202 / 209**, moving down ~7 rows a frame. The 61/62 pair is the split in
its steady state; the drop is the same register being animated.

### 13.3 The keyframes, and why these

| index | what is on screen | frame files | `sha256` |
|---|---|---|---|
| 6000 | the **plasma**, first palette — blue and magenta over the middle 280 rows, with a flat **`(4,4,4)`** above and below (not black: the filed file has no zero byte in it at all; at 7000 and 7600 the same bands are `(0,0,0)`, a palette phase). | `.rgb` only | rgb `308d4d3a…` |
| 7000 | the **plasma**, second palette — green and blue. | `.rgb` only | rgb `6f41f697…` |
| 7600 | the **plasma**, third palette — cyan and orange. | `.rgb` only | rgb `9a8f3fe5…` |
| 8500 | the **cube**, small — a rainbow-textured cube in the upper left over black. | `.rgb` only | rgb `11fd1186…` |
| 9500 | the **cube**, large — the cube's open face across the top half of the frame. | `.rgb` only | rgb `4190291b…` |

Two screens, one keyframe per screen per row: three across the plasma's ~2600 frames (the plasma
steps its palette through the five tables in `pals` as `cop_drop` fires, so one per palette state is
what covers it) and two across the cube's ~2000, one while the cube is small and one when it is
large. Every one of them is at least 500 frames from its neighbours, so every one can carry its
`±1` support batch (§1.3).

Full hashes of the five files in `reference/parts/plzpart/`:

| index | file | `sha256` |
|---|---|---|
| 6000 | `plasma/frame_006000.rgb` | `308d4d3afe6dd2c33c3ae652eeba3b36855afd656a9ea5a37921714e5ac33504` |
| 7000 | `plasma/frame_007000.rgb` | `6f41f697305f68deaccc8d30281ee05f5e20f02b6be4473b3bd24b3eea542299` |
| 7600 | `plasma/frame_007600.rgb` | `9a8f3fe599ea5099058146bac43a1da953359baff2483defbcb6e6b2da586d88` |
| 8500 | `cube/frame_008500.rgb` | `11fd11863e06ea02257007c929cecf660c96b40cbbd4ce04b9013e9d8222dec9` |
| 9500 | `cube/frame_009500.rgb` | `4190291b8b84f4795e8414a01f22b81096f587ce770a4e477792dbae1c0a812e` |

### 13.4 The refusals the tool made, and what 0xC0 is

Two of this part's reads were refused as **`PlaneTooSmall`**, and the message names the stride:

```
$ cargo xtask reference capture … --frames 6000,7000 --out out/b4-plz-idx1
warning: indexed capture failed (the screen is not one the indexed read can describe, so this frame
         keeps its RGB: this screen's rows are 384 bytes and it is 400 rows tall, so it needs 153600
         bytes a plane; one plane holds 65536, and the display wraps); keeping RGB only
warning: indexed capture does not reproduce the RGB frame (the screen moved while it was read); keeping RGB only
```

**384 bytes a plane row is not a stride this part ever sets** — the plasma's offset register is
`0x30` (`TWEAK.ASM:65-66`) and the cube's is `0x50` (`TWEAK.ASM:34-35`) — and both of the ways it can
arise are the same two-operation port read §1.7 records. **The message names the stride, not the
register, so the two cannot be told apart from this log**, and both are written down (§1.9):

- **consistent with the offset register reading another register's value**: `0xC0` is exactly the low
  byte of `cop_start`: `PLZ.C:176` sets `cop_start = 96*(682-400) = 27072 = 0x69C0`, and
  `COPPER.ASM:69-72` writes that low byte to CRTC register **0x0d** and its high byte to 0x0c, every
  frame, from the copper interrupt. TECHNO's version of this was 0x0c's `plv * 0x20`;
- **or the colour depth being read as chain-4**: `chain4_enabled()` is the same racy pair (index to
  3c4h, data from 3c5h, a separate request each), `PLZ.C:95-118` writes 3c4h twice a frame, and
  384 is exactly `0x30 × 8` — the plasma's own offset taken as a chained row.

It happened at 6000 twice (the reference capture, then a single-index re-run) and not at 7000 in
either of the same two runs, so it is phase-of-the-frame dependent rather than permanent, and **it is
a tool-side observation: no artefact in this tree was produced by it**, because every frame of this
part is RGB-only anyway (the screens move).

It is worth recording what the race *could* have done here and did not. If the offset register is the
one misread, 0xC0 fails the plan loudly — but register **0x18**, which `pompota` writes 0x3c/0x3d to
every frame, would have **passed** it silently as a 120-byte plane row, leaving only the coherence
check between that and a sheared `.idx` in the reference tree; on this screen that check is never
reached (the picture moves), but the same race on a *held* screen whose part writes CRTC registers
could ship a sheared plane with a plausible stride. If instead the *layout* is what was misread, a
racy offset is multiplied by eight and refuses loudly, so the quiet case needs the offset to be the
misread register. **The fix belongs in the oracle** — read the pair coherently or twice, and require
agreement — and is recorded in §1.9 for whoever next touches it. Nothing in this task's scope is
wrong.

### 13.5 Held or moving, measured, and what the two screens do across sessions

Every keyframe of this part is a **moving** frame, and the two screens move at very different rates.
Measured between adjacent indices inside one capture (`out/b4-plz-sess1-plasma`,
`out/b4-plz-sess1-cube`, `out/b4-plz-probe1`):

| keyframe | neighbours | reading | rate, of the frame's own pixel count |
|---|---|---|---|
| plasma 6000 | 5999 / 6000 / 6001 three `sha256` | **moving** | 89719 px of 128000 = **70.0930 %**; then 70.0828 % |
| plasma 7000 | 6999 / 7000 / 7001 three | **moving** | 87805 px of 128000 = **68.5977 %**; then 68.5664 % |
| plasma 7600 | 7599 / 7600 / 7601 three | **moving** | 89920 px of 128000 = **70.2500 %**; then 70.2500 % |
| cube 8500 | 8499 / 8500 / 8501 three | **moving** | 16056 px of 64000 = **25.0875 %**; then 24.1250 % |
| cube 9500 | 9499 / 9500 / 9501 three | **moving** | 35711 px of 64000 = **55.7984 %**; then 59.6313 % |

The plasma saturates the way TECHNO's interference does (§1.7): **70 % of the screen changes every
frame**, so the percentage a comparison prints there is a one-frame statement and cannot witness
anything larger. (The large rate is the `pompota` alternation doing more than it looks: a split that
moves one row moves every row below it.)

**The cube's moving frames are byte-identical at the same index in every session; the plasma's are
the same picture to within a fraction of one frame.** Every session captured at the cube's indices
holds `11fd1186…` at 8500 and `4190291b…` at 9500 exactly (`out/b4-plz-cube-sweep`,
`out/b4-plz-sess1-cube`, `out/b4-plz-sess2-cube`, and the reference's own capture). At the plasma's
indices the sessions are *not* all identical, but they are all **close**, and the numbers say how
close: against the filed 6000, the other sessions' 6000 frames are **0 px** (`out/b4-plz-sess3-plasma`),
118 px (0.0922 %), 210 px (0.1641 %), 316 px (0.2469 %) and 3942 px (3.0797 %) away. The plasma's own
move is 70.09 % of the frame per frame, so those spreads are **a thousandth to a twenty-third of one
frame**: the sessions are in phase on this screen, and byte-identity at the ±1 window's resolution
splits them into singletons at the last decimal place. That is the same knife edge §1.8 records for
GLENZ 3000 ("a group of one or two means a picture within a few pixels of its neighbour"), one
decimal place further down — and it is why the census below reports support counts of one while the
pictures are within a few hundred pixels of each other.

**What the ±1 window is worth here is nothing, measured the other way.** A session's own neighbour
frames are 68.57–70.25 % away from its frame at `N` (the table above), so no choice of `N±1` in a
partner can recover a picture this screen drew — and none is needed, because the spread is already
below the window's resolution.

**And the mechanism is not a frame-level phase drift, which the numbers rule out**: a start that
differed by one frame between sessions would show up as a ~70 % difference at a fixed index, not
0.09–3 %. What the sessions differ by is sub-frame, so it is the point *within* a frame at which two
things meet: the plasma's C loop, which draws the even rows with one parameter set and the odd rows
with another and then does the whole thing again with the planes swapped (`PLZ.C:95-118`), and the
copper's once-a-frame `moveplz`/`pompota` rewriting of those parameters (`COPPER.ASM:104-107`,
`:126-143`). That is one thing this block did not resolve, and it is recorded with its size measured
rather than guessed at.

### 13.6 The re-capture and the census

```bash
# the reference: keyframes only, one capture per screen
cargo xtask reference capture --arg 3 --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames 6000,7000,7600 --out reference/parts/plzpart/plasma --sheet out/plzpart-plasma.png
cargo xtask reference capture --arg 3 --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames 8500,9500 --out reference/parts/plzpart/cube --sheet out/plzpart-cube.png

# the partners: a support frame each side of every keyframe, per screen
cargo xtask reference capture … --frames 5999,6000,6001,6999,7000,7001,7599,7600,7601 --out out/b4-plz-sess1-plasma
cargo xtask reference capture … --frames 8499,8500,8501,9499,9500,9501 --out out/b4-plz-sess1-cube
```

```
$ cargo xtask compare --reference reference/parts/plzpart/cube --rendered out/b4-plz-sess1-cube \
      --out out/b4-plzcube-diff1
frame 8500: 0.0000% differing (0/64000 px), max delta 0
frame 9500: 0.0000% differing (0/64000 px), max delta 0
verdict: identical, mode: rgb, offsets {"0": 2}
exit: 0
```

The same report comes back against `out/b4-plz-sess2-cube`, `out/b4-plz-sess3-cube` and
`out/b4-plz-sess4-cube` — **the cube's keyframes are byte-identical at offset 0 in every session
captured**. The plasma's do not compare
identical, and the numbers are the row:

```
$ cargo xtask compare --reference reference/parts/plzpart/plasma --rendered out/b4-plz-sess3-plasma
frame 6000: 0.0000% differing (0/128000 px), max delta 0
frame 7000: 7.7344% differing (9900/128000 px), max delta 21
frame 7600: 9.3281% differing (11940/128000 px), max delta 17
verdict: different, mode: rgb, offsets {"0": 3}
exit: 1

$ … --rendered out/b4-plz-sess1-plasma
frame 6000: 0.1641% differing (210/128000 px), max delta 16
frame 7000: 12.9266% differing (16546/128000 px), max delta 20
frame 7600: 24.4844% differing (31340/128000 px), max delta 25
verdict: different, mode: rgb, offsets {"0": 3}
exit: 1
```

Every frame pairs **at offset 0** in both — the ±1 window is never used, because a neighbour on this
screen is 68.57–70.25 % away (§13.5).

**The census**, by §1.8's rule. Rule 1's criterion is *captures that hold `N-1`, `N` and `N+1`*, and
rule 2 excludes only the reference's own directory — so **a run that was refused later still counts
when it holds the window**, which is why the census below has five sessions at 6000 and four at 7000:
`out/b4-plz-sess2-plasma` was refused as a miss at 7601 and writes no manifest, but it holds valid
windows at 6000 and 7000, and `out/review-b4a/plz-probe2` (the reviewer's own session) holds one at
6000. The "support of the filed picture" column is the count over those windows; the "largest
support" column is the same count over *every* picture in them, which is **not** the same as the
largest *filable* picture — rule 3's caveat forbids filing at `N` a picture that only ever appears at
`N±1`, so both numbers are given.

| keyframe | sessions with a window | filable pictures (seen at `N`) | support of the filed picture | largest support over the ±1 window | filed picture ≥ half? |
|---|---|---|---|---|---|
| cube 8500 | 4 | 4 | `11fd1186…` **4/4 (100 %)** | 4/4 (its neighbours) | yes |
| cube 9500 | 4 | 4 | `4190291b…` **4/4 (100 %)** | 4/4 (its neighbours) | yes |
| plasma 6000 | **5** | 5 | `308d4d3a…` **2/5 (40 %)** | `390341ec…` 3/5, **only at `N−1`** — not filable | **no** — no settled content |
| plasma 7000 | **4** | 4 | `6f41f697…` **0/4** | `8501036521b3…` and `ac34f7c7334f…` **2/4**, each only at `N−1`/`N+1` — not filable | **no** — no settled content |
| plasma 7600 | **3** | 3 | `9a8f3fe5…` **0/3** | nine pictures, 1/3 (33 %) each | **no** — no settled content |

**So all three plasma keyframes are recorded as having no settled content, 6000 included** — §1.8
rule 4 applies to each of them, and the paragraph that follows says why the filed bytes stay where
they are anyway. The counts in detail:

- **6000: the filed `308d4d3a…` has 2 of 5 (40 %).** `out/b4-plz-sess3-plasma` and
  `out/b4-plz-sess4-plasma` present it at their `N` — their windows are the same three hashes,
  `390341ec1d8c`, `308d4d3a…`, `970eda5970e3` — while the reviewer's `out/review-b4a/plz-probe2` has
  a *different* draw at `N` and `N+1` and shares only the first of those three with them
  (`390341ec…`, at its `N−1`). So the picture with the largest support in the windows is
  `390341ec1d8c` at **3/5 — presented by three sessions, every one of them at `N−1`** — which rule 3's
  caveat will not let a row file at 6000 at all. Among pictures that *are* taken at 6000 the filed one
  is the largest: 2/5, against 1/5 each for `sess1`'s, `sess2`'s and `plz-probe2`'s. And 2/5 is below
  half.
- **7000: the filed `6f41f697…` has 0 of 4.** Every picture taken at 7000 by any session is a
  singleton, so the largest *filable* support there is 1 of 4 (25 %), and the two pictures with 2/4
  (`sess3`'s and `sess4`'s `8501036521b3…` at their `N−1`, `ac34f7c7334f…` at their `N+1`) are not
  filable at `N`. **This is why rule 4 applies rather than rule 3: there is no picture at 7000 for
  the rule to file.**
- **7600: nine pictures over three sessions, every one a singleton.** "In the largest group" is
  vacuous, which is the third defect §1.8 lists, and no re-filing can lift it above 1/3.

Nothing was re-filed, and the reasons are measured rather than preferred:

- **the spread is sub-frame, not a phase away.** Against the filed 6000 the other sessions' 6000
  frames are 0 px, 118 px, 210 px, 316 px and 3942 px apart (0.0922 %, 0.1641 %, 0.2469 %, 3.0797 %);
  at 7000 the pairwise distances are 0.0187–16.16 % and at 7600 0.0422–25.45 %, every one of them
  with a **max channel delta of 29 or less**. On a screen whose own move is 70 % of the frame
  (§13.5), that is a *fraction of a frame* — a picture drawn at a slightly different point in the
  frame, not a different picture. On GLENZ §1.8 says a "group" of one or two means "a picture within
  a few pixels of its neighbour"; here the singletons themselves are within a few thousand pixels of
  each other and within 29/255 of a colour.
- **no filing can do better than the one in place.** At 6000 the filed picture is the largest-support
  picture *taken at `N`*; at 7000 and 7600 every picture taken at `N` is a singleton, so any choice
  is as (un)settled as this one.
- **and a re-capture would be a dice roll against a measurement that is already in the tree.** The
  filed reference is a real keyframe-only capture; re-rolling for 7000 would put the filed frame at
  the mercy of the same sub-frame draw that produced the current spread, and could drop 6000 out of
  the state it does share — the mistake §1.8 rule 5 exists to prevent.

**The row records it rather than dressing it up**: PLZPART's plasma keyframes are one exact point in
a frame of a screen the oracle draws at a slightly different point from session to session, and
**none of the three is a majority picture** — 2/5, 0/4 and 0/3. Phase 2 comparing against them should
read a residual of a few per cent at these indices as this, and not as a port error — and should read
8500/9500, which are exact in every session, as the part's precise targets.

---

### 13.7 Where the part begins, ends, and changes screens

`out/b4-plz-start1` took 5710, 5720, 5730 and 5740, and **refused at 5730**: `the display changed
geometry between frames: frame 5730 is 320x400 where the capture started at 320x200`. So LNS&ZOOM's
white screen holds through 5720 and PLZPART's 320×400 screen is up by 5730 — the change is in
**(5720, 5730]**, it is the `tw_opengraph2` mode set inside `init_plz()` (`PLZ.C:175`), and this is
the one boundary in the block the refusals find by themselves.

The plasma→cube change is the same kind and is also found by refusal: `out/b4-plz-cube-edge2` took
8310 (320×400) and refused at 8340 (320×200), and `out/b4-plz-cube-edge1` refused at 8350, so **the
320×400 screen ends in (8310, 8340]** and `tw_opengraph()` (`VECT.C:114`) is what ends it.

The cube→MINVBALL change is **not** a geometry change — both screens are 320×200 — so it was found
by content, the fourth such boundary in this ledger (U2A→PAM §6.5, GLENZ→TUNNELI §7.5, PANICEND→
MNTSCRL §10.4). `out/b4-plz-minv-edge` took 10100, 10200, 10300 and 10400:

| index | 10100 | 10200 | 10300 | 10400 |
|---|---|---|---|---|
| what | the cube, 156 colours | 79 colours, dark olive | 51 colours, dark olive | **pure black**, one colour |

10100 is the cube by its palette (the plasma's greens and golds); 10400 is the next part's screen
with its palette zeroed — `DOTS/MAIN.C:153-155` writes 0 to all 768 DAC entries right after the mode
set, and the grey background only becomes visible when its 130-frame fade raises the palette
(§14). Between them 10200 and 10300 are the cube dimming. **So the cube's screen runs to somewhere
in (10300, 10400] and MINVBALL's black screen is up at 10400.**


## 14. MINVBALL

`SECOND 3`'s fourth part. `DOTS/MAKEFILE:20` builds `dots.exe` and stages it as `minvball.exe`; the
source is `DOTS/MAIN.C` (512 dots, `:91`; the physics constants at `:102-113`) plus `DOTS/ASM.ASM`'s
`drawdots` (`:52-218`), which blits the background picture and then draws the dots into it.

**What it does.** `:125-126` sets mode 13h; `:127-147` builds a palette of 16 colours (from `cols[]`,
`:72`), 100 greys and white at 255; `:148-150` reads that palette back; `:151` **zeroes the DAC**;
`:152-155` fills rows 100-199 of video memory with a rising index ramp; `:168-169` copies that to
`bgpic`; `:171-183` fades the palette up over 65 steps of **two** vblanks each — 130 frames — and then
`:185` starts the main loop, `while(!dis_exit() && frame<2450)`, which advances the dot simulation by
the vblanks that passed and calls `drawdots()` once a frame (`:283`). The loop also breaks on the
music (`dis_musplus()` between -4 and 0, `:191-195`) and re-loads its darker palette after frame 2300
(`:189`). The dots themselves go through four position phases inside those 2450 frames
(`frame<500`, `<900`, `<1700`, `<2360`) and then the palette brightens (`<2400`) and whitens
(`<2440`).

320×200 mode 13h throughout — 80-byte plane rows, 64000 pixels.

### 14.1 The sweeps, and what the sheets showed

```
out/b4-plz-minv-edge   10100,10200,10300,10400        exit 0 (4 frames)  — §13.7
out/b4-minv-sweep1     10600,10900,…,13000 step 300   exit 0 (9 frames)
```

Tiles, in the order the capture laid them out. **10600** a handful of dots in the upper right and a
faint ellipse low in the frame, over the grey floor; **10900** a ring of dots across the top with a
scatter below it; **11200** a large ellipse over a **vortex** of dots; **11500** the same ellipse with
the dots in a dense band; **11800** a fountain or tree shape of dots; **12100** the same, narrower and
taller; **12400** sparse dots scattered across the frame; **12700** a wide cloud of dots;
**13000** a different picture entirely — dark blue with orange and blue spheres, RAYSCRL's screen —
so **MINVBALL's own drawing ends in (12700, 13000]** and the next part's screen is up at 13000 with no
geometry change (the sweep exited 0).

The dots' four position phases are the source's own (`DOTS/MAIN.C:202-241`: `frame<500`, `<900`,
`<1700`, `<2360`), 500/400/800/690 frames long, and the sheet shows exactly four shapes in that
order — a ring, a vortex, a fountain, then a scatter. (Where each phase starts follows from where the
part's main loop starts, which this block did not measure directly: the first dots are visible at
10600 and the background alone is up at 10500, so the loop begins in (10500, 10600] and the phases
fall roughly 10530/11030/11430/12230 after it.)

### 14.2 The keyframes, and why these

| index | what is on screen | frame files | `sha256` |
|---|---|---|---|
| 10900 | the **ring** — a ring of dots across the upper third over a scatter, above the grey floor. | `.rgb` only | rgb `726ec7e5…` |
| 11200 | the **vortex** — a wide ellipse of dots over a swirling column. | `.rgb` only | rgb `954e4b30…` |
| 11800 | the **fountain** — dots rising into a tree shape. | `.rgb` only | rgb `20764312…` |
| 12400 | the **scatter** — dots thrown across the whole frame. | `.rgb` only | rgb `33dce4e8…` |

Full hashes of the four files in `reference/parts/minvball/` (all `.rgb` only — the dots move every
frame, so no frame of this part carries `.idx`):

| index | file | `sha256` |
|---|---|---|
| 10900 | `frame_010900.rgb` | `726ec7e5e24d4f52f3421b80ea249304cf5985a9ee4ed54c3a46a318d2ca382e` |
| 11200 | `frame_011200.rgb` | `954e4b30c825fe3839f80405245fca6953793cf201553d4fcda008e1efb7f720` |
| 11800 | `frame_011800.rgb` | `207643123be0a4c4faad6e9f5d30404bc6e5e9de42a3e21d03808da1b377704f` |
| 12400 | `frame_012400.rgb` | `33dce4e8add8345720310cfd17bb8000a687654d48b6f9307227b631d947179e` |

One keyframe per position phase, spread 300–600 frames apart — far more than the ~60 a three-index
support batch needs (§1.3) — so a comparison sees each of the shapes the part draws.

### 14.3 Held or moving, measured

| keyframe | neighbours | reading | rate, of 64000 pixels a frame |
|---|---|---|---|
| 10900 | 10899 / 10900 / 10901 three `sha256` | **moving** | 3306 px = **5.1656 %**, then 3201 px = 5.0016 % |
| 11200 | 11199 / 11200 / 11201 three | **moving** | 4548 px = **7.1063 %**, then 4425 px = 6.9141 % |
| 11800 | 11799 / 11800 / 11801 three | **moving** | 4222 px = **6.5969 %**, then 4557 px = 7.1203 % |
| 12400 | 12399 / 12400 / 12401 three | **moving** | 2999 px = **4.6859 %**, then 3324 px = 5.1937 % |

All measured between adjacent indices inside `out/b4-minv-sess1`. Every keyframe of this part is a
moving screen — the background is static, but the 512 dots are redrawn every frame — and **the
moving frames are byte-identical at the same index in every session captured**: the reference's
10900, 11200, 11800 and 12400 are the same `sha256` as `out/b4-minv-sweep1`'s (a discovery session
captured first) and as the census sessions' (§14.4). Like MNTSCRL and LNS&ZOOM, this part's timeline
is anchored at its start and does not drift.

### 14.4 The re-capture and the census

```bash
# the reference: keyframes only
cargo xtask reference capture --arg 3 --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames 10900,11200,11800,12400 --out reference/parts/minvball --sheet out/minvball.png

# the partners: a support frame each side of every keyframe
cargo xtask reference capture --arg 3 --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames 10899,10900,10901,11199,11200,11201,11799,11800,11801,12399,12400,12401 \
    --out out/b4-minv-sess1
```

```
$ cargo xtask compare --reference reference/parts/minvball --rendered out/b4-minv-sess1 \
      --out out/b4-minv-recapture-diff1
frame 10900: 0.0000% differing (0/64000 px), max delta 0
frame 11200: 0.0000% differing (0/64000 px), max delta 0
frame 11800: 0.0000% differing (0/64000 px), max delta 0
frame 12400: 0.0000% differing (0/64000 px), max delta 0
verdict: identical, mode: rgb, offsets {"0": 4}
exit: 0
```

**The census**, by §1.8's rule, over the three sessions that hold a window (the reference's own
directory excluded):

| keyframe | sessions with a window | pictures at `N` | largest support | filed reference is it? | ≥ half the sessions? |
|---|---|---|---|---|---|
| MINVBALL 10900 | 3 | 3 | `726ec7e5…` **3/3 (100 %)** | **yes** | yes |
| MINVBALL 11200 | 3 | 3 | `954e4b30…` **3/3 (100 %)** | **yes** | yes |
| MINVBALL 11800 | 3 | 3 | `20764312…` **3/3 (100 %)** | **yes** | yes |
| MINVBALL 12400 | 3 | 3 | `33dce4e8…` **3/3 (100 %)** | **yes** | yes |

Every session presents the filed picture **at `N` or `N−1`** — `out/b4-minv-sess3` answers all four
at offset −1 with `0.0000 %` (`frame 10900: 0.0000% differing (0/64000 px), max delta 0, rendered
index 10899 (offset -1)`), which is exactly what the ±1 window is for and is not a difference in the
pictures: its own 10900 is the reference's 10899 picture's neighbour in the animation, and the
percentage is what says so (0.0000 %, max delta 0). Moving screens whose frames are byte-identical at
the same index — as this part's are (§14.3) — still give the alignment search something to do when a
session's *start* lands one frame either side.

### 14.5 Where the part begins and ends

The beginning is §13.7 from the other side: 10100, 10200 and 10300 are PLZPART's cube, 10400 is this
part's screen with its palette zeroed (`DOTS/MAIN.C:151`), and 10500 is the background rising through
the 130-frame fade — so **MINVBALL's first frame is in (10300, 10400]**, and, as at U2A→PAM,
GLENZ→TUNNELI and PLZPART→MINVBALL, the boundary is a content change with **no geometry refusal**:
both screens are 320×200 and every sweep crossed it at exit 0.

The end is the same kind of boundary, and `out/b4-minv-sess3`'s two extra indices pin it:

| index | 12700 | 12800 | **12900** | 13000 |
|---|---|---|---|---|
| colours | 47 | 47 | **1** | 35 |
| what | dots scattered across the frame | dots (the scatter phase, still) | **flat `(231,231,231)`** — the part's white-out | dark blue with orange and blue spheres — RAYSCRL |

12800 is still the scatter; **12900 is a single flat colour — `(231,231,231)` in this session and
`(239,239,239)` in the reviewer's fresh one, so the invariant is "one colour in the last ~40 frames",
not a value** — which is the part's own ending, `DOTS/MAIN.C:242-256` re-adding the palette scaled by how far past frame 2360
the frame is and `:258-270` then taking every entry down from 63 together (`c=63-a*2`), so that at
some frame in the last 40 the whole palette is one value and the dots' indices underneath stop
mattering. **13000 is RAYSCRL's screen**. So MINVBALL's own drawing ends in **(12900, 13000]** — about 2550 frames after
its first, matching the 2450-frame cap on its own loop plus the 130-frame fade — and the next part
follows it with no geometry change.

---

## 15. RAYSCRL

`SECOND 3`'s fifth part — `WATER/X.BAT:1` stages `demo.exe` as `rayscrl.exe`, and the whole part is
`WATER/DEMO.PAS` (190 lines of Turbo Pascal) plus `WATER/ROUTINES.ASM`'s `Putrouts1`. **320×200 mode
13h, one index a byte, chain-4** (`mov ax,$13 / int $10`, `DEMO.PAS:91-92`), and **the part never
writes a CRTC register**: its only ports are the DAC's (`port[$3c8]`/`port[$3c9]`, `DEMO.PAS:3-8`).

**What it draws.** A raytraced scene — a large glossy blue sphere, a smaller one below it,
concentric ripple rings on a blue floor, and one small object the part animates across the picture —
copied into video memory once (`DEMO.PAS:112`) and then **scrolled**: `scr(sss)` (`:51-80`)
calls `Putrouts1` with one of three position maps (`wat1`, `wat2`, `wat3`) and the font buffer `fbuf`,
and `sss` cycles 0,1,2 (`:130` in the fade-in, `:180-181` in the main loop), so the picture advances
every frame and the font column one step every third frame. The palette fades in over 128 frames (`for y := 0 to 63*2`, `:117-132`, body at `:119-131`) and out
over 64 (`:167-178`) when the music reaches the position the loop polls for (`cmp dx,-11` at `:162`, in the loop at
`:158-166`).

**Its extent on a `SECOND 3` boot is from (12900, 13000] to (14900, 15000]** — about 2000 frames,
≈28 s. The start is §14.5's boundary seen from this side; the end is measured here (14900 full
picture at 230 colours, 15000 the fadeout's tail at mean byte 0.11) and 3DSINFLD's terrain is
entering at 15100.

### 15.1 The sweeps, and what the sheets showed

```
out/b4b-ray-sweep1  13000,13500,14000,14500,15000,15500,16000,16500,17000,17500   exit 2, refused
                    at 17500: "the display changed geometry between frames: frame 17500 is 320x400
                    where the capture started at 320x200; capture one screen at a time"
out/b4b-ray-sweep2  14600,14700,14800,14900,15000,15100,15200,15300,15400,15500,16800,16900,
                    17000,17100,17200,17300,17400                                 exit 2, refused at
                    17100, same message — so the 320×200 screen ends in (17000, 17100]
```

Both refusals are §1.2's discovery working: the 320×200 screen of this boot ends in **(17000, 17100]**
and a 320×400 screen follows it. (The MINVBALL→RAYSCRL boundary could not be found this way — both
screens are 320×200 — and is §14.5's.)

| index | what is on screen |
|---|---|
| 13000 | the raytraced scene, mid fade-in: the large glossy sphere upper right, a smaller one below it, ripple rings on a blue floor, a small orange ellipse low-left. 35 colours, mean 23.15. |
| 13500 | the same scene scrolled left, no object in frame. 190 colours, mean 27.81. |
| 14000 | the same scene with a **golden comet** with a long wavy trail crossing from the upper left. 217 colours, mean 32.16. |
| 14500 | the same scene with a **green-and-grey screwdriver-like object** at the left and a thin scanning line across the sphere. 230 colours, mean 44.21. |
| 14600–14900 | the object decomposing into a spray of small white/orange squares that drifts right and shrinks. |
| 15000 | **11 colours, mean byte 0.11**, 63303 of 64000 px black — the fadeout's tail. |
| 15100 | (3DSINFLD) the voxel terrain entering from the lower left. |

### 15.2 The keyframes, and why these

| index | what is on screen | frame files | `sha256` |
|---|---|---|---|
| 13000 | the scene **mid fade-in** | `.rgb` only | rgb `62ef53c7…` |
| 13500 | the scene scrolled, no object | `.rgb` + `.idx` | rgb `92dc2c9e…`, idx `afda560c…` |
| 14000 | the scene with the **golden comet** | `.rgb` only | rgb `6715eece…` |
| 14500 | the scene with the **screwdriver object** | `.rgb` only | rgb `d1cb7f59…` |

One keyframe per object the part animates across its held background, 500 frames apart — far more
than the ~60 a three-index support batch needs (§1.3) — plus one in the fade-in so the part is
covered from its first second.

Full hashes of the five files in `reference/parts/rayscrl/`:

| index | file | `sha256` |
|---|---|---|
| 13000 | `frame_013000.rgb` | `62ef53c7e8922139a30b07e4c3325f915bca582161ba67bf088782076888a52f` |
| 13500 | `frame_013500.rgb` | `92dc2c9e51af55183c05db9e643e8209ff96d2235f0689278cdd4c518982db20` |
| 13500 | `frame_013500.idx` | `afda560cbf29fce267ec3e29e949bb4deeb47ea4a81e57992354d916ff03e660` |
| 14000 | `frame_014000.rgb` | `6715eece4a6dde27132007cda3f78c873cba92bd426393159b29cd6cfc5149f1` |
| 14500 | `frame_014500.rgb` | `d1cb7f593fee5f9f09ea7d805556c630519502129f62cdfac26ac07a98c7e804` |

### 15.3 Held or moving, measured — and the `.idx` hand-check

The triples around each keyframe, hashed inside `out/b4b-ray-sess1` and `out/b4b-ray-sess2`
(the same bytes in both sessions at every index):

| keyframe | `K-1` | `K` | `K+1` | reading | per-frame change of its own 64000 px |
|---|---|---|---|---|---|
| 13000 | `ccb4bb02…` | `62ef53c7…` | `8f456780…` | **moving** | 12999→13000 **64 px (0.1000 %)**, 13000→13001 23908 px (37.3563 %) |
| 13500 | `92dc2c9e…` | `92dc2c9e…` | `92dc2c9e…` | **held** | 0 px either side |
| 14000 | `36f5c972…` | `6715eece…` | `8ee47d77…` | **moving** | 2059 px (3.2172 %) and 1962 px (3.0656 %) |
| 14500 | `0158b387…` | `d1cb7f59…` | `123f0056…` | **moving** | 4079 px (6.3734 %) and 3100 px (4.8438 %) |

13500 is the part's **held** keyframe and the reason it is one: 13499, 13500 and 13501 are
byte-identical (and equal to the reference's frame), so a session's ±1 candidates are the same
picture and the search finds offset 0 whatever the session's phase is (§1.3 rule 2). 13000's tiny
`K-1` difference is worth noticing — a screen whose per-frame change is 0.10 % at one step and 37 %
at the next (§1.10) — and it is why held-or-moving is measured per keyframe and not assumed from the
part. `.idx` is present at 13499, 13500, 13501, 15050 and 17000 in every session that took them and
absent at the other indices (13000, 14000, 14500, 14950, 15499–16950); §5.3 records that this is the
session's business rather than a property of held-or-moving, and the pattern here — the three frames
of the 13500 hold, the flat black 15050 and 17000 — is the coherence rule, not a rule about holds.

**RAYSCRL writes no CRTC register**, so §1.9's hand-check rule is not triggered by this part; the
check was done anyway, twice. `frame_013500.idx` has 190 distinct indices and 190 distinct RGB
triples with no index carrying two triples (`/tmp/b4b/idxcheck.py`), and against the manifest's own
palette with the `(c*259+33)>>6` widening it reproduces the RGB with **0 mismatches of 64000 pixels**
(`/tmp/b4b/handcheck.py`). The palette does not collapse, so that is a real check and the plane is not
sheared. It is also **byte-identical** to the plane `out/b4b-ray-sweep1` and `out/b4b-ray-sess1`
wrote at the same index (`afda560c…`), so the indexed plane reproduces across sessions as well as the
RGB does.

### 15.4 The re-capture and the census

```bash
# the reference: keyframes only
cargo xtask reference capture --arg 3 --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames 13000,13500,14000,14500 --out reference/parts/rayscrl --sheet out/rayscrl.png

# the partners: a support frame each side of every keyframe
cargo xtask reference capture --arg 3 --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames 12900,12950,12999,13000,13001,13499,13500,13501,13999,14000,14001,14499,14500,14501,\
14950,15000,15050,15499,15500,15501,15999,16000,16001,16499,16500,16501,16799,16800,16801,16900,\
16950,17000 --out out/b4b-ray-sess1
```

(The partner list carries 3DSINFLD's keyframes as well — the two parts are adjacent on the same
320×200 screen, so one session answers both, and the same command was run three times into
`out/b4b-ray-sess1`, `-sess2` and `-sess3`.)

```
$ cargo xtask compare --reference reference/parts/rayscrl --rendered out/b4b-ray-sess1 \
      --out out/b4b-ray-cmp1
frame 13000: 0.0000% differing (0/64000 px), max delta 0
frame 13500: 0.0000% differing (0/64000 px), max delta 0
frame 14000: 0.0000% differing (0/64000 px), max delta 0
frame 14500: 0.0000% differing (0/64000 px), max delta 0
verdict: identical, mode: mixed, offsets {"0": 4}
exit: 0
```

**`out/b4b-ray-sess2` and `out/b4b-ray-sess3` give the identical report** — three independent
sessions, each captured minutes apart, `exit 0`, `verdict: identical`, 0.0000 %, offsets `{0: 4}`.
`mode: mixed` because 13500 is compared as indices (both sides hold `.idx` there) and the other three
as RGB.

**The census**, by §1.8's rule, over the **four** sessions that hold a window — `out/b4b-ray-sess1`,
`-sess2`, `-sess3` and the reviewer's `out/review-b4b/ray-sess` (the reference's own directory
excluded; every directory under `out/` was scanned for the frames, not just those with manifests, so a
refused run holding a window would have counted — none does):

| keyframe | sessions with a window | pictures at `N` | largest support | filed reference is it? | ≥ half the sessions? |
|---|---|---|---|---|---|
| RAYSCRL 13000 | 4 | 3 | `62ef53c7…` **4/4 (100 %)** | **yes** | yes |
| RAYSCRL 13500 (held) | 4 | 1 | `92dc2c9e…` **4/4 (100 %)** | **yes** | yes |
| RAYSCRL 14000 | 4 | 3 | `6715eece…` **4/4 (100 %)** | **yes** | yes |
| RAYSCRL 14500 | 4 | 3 | `d1cb7f59…` **4/4 (100 %)** | **yes** | yes |

Every session presents the filed picture **at offset 0** on all four indices — including the held
13500, where all three frames of each session's window are the filed picture (offsets `[-1, 0, 1]` in
each). This is the MNTSCRL/LNS&ZOOM/MINVBALL shape (§19): the part's timeline is anchored and every
session draws the same bytes at the same boot index.


---

## 16. 3DSINFLD

`SECOND 3`'s sixth part. `COMAN/MAKEFILE:18` builds `c.exe` and stages it as `3dsinfld.exe`; the
sources are `COMAN/MAIN.C`, `COMAN/ASM.ASM` (`inittwk`), `COMAN/THELOOP.ASM` and the wave tables
`w1dta.bin`/`w2dta.bin`. 320×200: `dis_partstart(); mov ax,13h; int 10h` (`MAIN.C:124-126`), then
`inittwk()` (`ASM.ASM:36-78`) clears the palette, writes **CRTC 0x14 = 0 and 0x17 = 0xe3**, turns
**chain-4 off** and sets the all-plane mask — and never writes CRTC 0x13, so the row stride stays mode
13h's own offset 40, **80 bytes to a plane row**.

**What it draws, and how it moves.** A voxel terrain — "3D-Sinusfield", the Comanche-style
heightfield — under a black sky (the sky is index 0; the terrain's palette is built from a formula at
`MAIN.C:128-150`, blues and magentas). `doit()` (`MAIN.C:66-118`) waits on the music
(`while(!dis_exit() && dis_musplus()<0)`), then runs `while(!dis_exit() && frame<4444)` with a second
music break (`a=dis_musplus(); if(a>-8 && a<0) break;` — so the 4444-frame cap is never reached on
this boot), rendering a column at a time with `docol` into the off-screen page and **double buffering
by toggling CRTC 0x0c between 0x80 and 0x00** (`MAIN.C:101-110`), then `docopy`. `frame +=
(frepeat = dis_waitb())` counts **vblanks**, and each render iteration costs ~2 of them, so **the
terrain's picture changes every second frame**: measured, at each of 15500, 16000 and 16500 one
neighbour is byte-identical and the other is ~37–39 % away, and at 16800 it is the other way round.

**Its extent is from (15000, 15100] to (16900, 17000]**, about 1900 frames. 15100 is the terrain
*entering from the lower left* (58 colours, mean 7.95, content only in rows 167–199); 16900 is still
the terrain; 17000 is flat black, one colour, `ea0787f6…` — the same black §11.5 records for MNTSCRL's
gap — and JPLOGO's 320×400 screen is up by 17100.

### 16.1 The sweeps, and what the sheets showed

```
out/b4b-ray-sweep1  13000,13500,…,17000,17500            exit 2, refused at 17500
out/b4b-ray-sweep2  14600…15500 step 100, 16800…17400    exit 2, refused at 17100
```

| index | what is on screen |
|---|---|
| 15100 | the voxel terrain entering from the lower left; 58 colours, mean 7.95, content rows 167–199, the rest black. |
| 15200, 15300 | the same terrain higher (15300: content rows 90–199). |
| 15400, 15500 | the terrain under a thin black sky band (15400: 59 colours, rows 90–199; 15500: 45–46 colours, rows 88–199). |
| 16000 | the terrain with a wide black band above it and a peak rising in the middle. |
| 16500 | the terrain lower again with a red-magenta ridge at the right (64–67 colours). |
| 16800, 16900 | the terrain in the lower half with a wide black band above (16800: 46–47 colours, rows 86–199; 16900: 54 colours, rows 109–199). |
| 17000 | flat black, one colour. |

### 16.2 The keyframes, and why these

Four, 300–500 frames apart, one for each stage of the flight: the terrain newly entered (15500),
mid-flight (16000, 16500) and near its end (16800). Every one is at least 300 frames from its
neighbours, well over the ~60 a three-index support batch needs (§1.3).

| index | file | `sha256` |
|---|---|---|
| 15500 | `frame_015500.rgb` | `be95db6c1fa85a8776a334fe658dfa037e2f793f3b9ca509fb4a1b87bd1e5dc6` |
| 16000 | `frame_016000.rgb` | `2d349adaa4150d235fcbdc99d95ffeb438e2e5d32489dec2b49a11189f6921e8` |
| 16500 | `frame_016500.rgb` | `9d129ad09b8225e9395abf47a43b37b43b057c1334e93032ae8e7ee620264f9d` |
| 16800 | `frame_016800.rgb` | `6e5994bf7165c3c6c5852d045fca617ed7f0b3f896af6458f5bcf208e31a6ee1` |

**Two of the four files are from the fix round's re-captures** (§16.4): 16500 was the first
reference's `538942e6…` and is now `9d129ad0…` — the picture three of the four sessions take at `N`
— and 16800 was `47a0b326…`, which no session takes at `N`, and is now `6e5994bf…`, which the two
that present it take *at* `N`. 15500 moved from the first reference's `6a891af9…` (presented by 1 of
3 windows) to `be95db6c…` (3 of 3) in an earlier pass. 16000 kept its bytes throughout.

**All four are RGB-only**, which is the tool's ordinary refusal for a moving screen (the terrain
redraws every second frame; measured 37–40 % of pixels between consecutive pictures). **No `.idx` is
filed from this part**, so §1.9's hand-check rule has nothing to cover here even though it applies —
**this part writes CRTC 0x0c every frame**, and the value it writes (`0x80`) is one
`plan_indexed_read` *accepts* as a 256-byte plane row at 200 rows. Nothing in the tree was produced
by that.

### 16.3 Held or moving, measured

Every keyframe is **moving**, in **two-frame steps**: the terrain redraws every second frame, so a
session's window holds one picture twice and the next once. The triples, from `out/b4b-ray-sess1`:

| keyframe | `K-1`, `K`, `K+1` in `-sess1` | reading |
|---|---|---|
| 15500 | `6a891af9…`, `be95db6c…`, `be95db6c…` | moving; the filed picture is on screen for two frames |
| 16000 | `2d349ada…`, `2d349ada…`, `2744df3f…` | moving, two-frame step |
| 16500 | `9d129ad0…`, `9d129ad0…`, `19a8a5b0…` | moving, two-frame step |
| 16800 | `47a0b326…`, `6e5994bf…`, `6e5994bf…` | moving, two-frame step |

The step is what makes the ±1 window enough for some sessions and not others: a session one frame out
of phase presents the same picture at `N±1` (offset `+1` or `-1`, 0.0000 %), and a session *two* steps
out (as `-sess3` is at 16500 and 16800) sits on a different picture and reports 38.9031 % or
41.7875 %.

### 16.4 The re-capture and the census

```bash
# the reference, as first captured
cargo xtask reference capture --arg 3 --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames 15500,16000,16500,16800 --out reference/parts/3dsinfld --sheet out/3dsinfld.png

# the partners: the same three sessions as RAYSCRL's (one geometry, both parts)
cargo xtask reference capture --arg 3 --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames 12900,12950,12999,13000,13001,13499,13500,13501,13999,14000,14001,14499,14500,14501,\
14950,15000,15050,15499,15500,15501,15999,16000,16001,16499,16500,16501,16799,16800,16801,16900,\
16950,17000 --out out/b4b-ray-sess1        # and -sess2, -sess3
```

```
$ cargo xtask compare --reference reference/parts/3dsinfld --rendered out/b4b-ray-sess1 \
      --out out/b4b-sinfld-cmp1
frame 15500: 0.0000% differing (0/64000 px), max delta 0, rendered index 15501 (offset +1)
frame 16000: 0.0000% differing (0/64000 px), max delta 0
frame 16500: 0.0000% differing (0/64000 px), max delta 0
frame 16800: 0.0000% differing (0/64000 px), max delta 0
verdict: identical, mode: rgb, offsets {"0": 3, "1": 1}
exit: 0

$ … --rendered out/b4b-ray-sess2
frame 15500 … frame 16800: 0.0000% differing (0/64000 px) each
verdict: identical, mode: rgb, offsets {"0": 4}
exit: 0

$ … --rendered out/b4b-ray-sess3
frame 15500: 0.0000% differing (0/64000 px), max delta 0, rendered index 15499 (offset -1)
frame 16000: 0.0000% differing (0/64000 px), max delta 0, rendered index 15999 (offset -1)
frame 16500: 38.9031% differing (24898/64000 px), max delta 255, rendered index 16499 (offset -1)
frame 16800: 41.7875% differing (26744/64000 px), max delta 231
verdict: different, mode: rgb, offsets {"-1": 3, "0": 1}
exit: 1
```

**Two of the three partner sessions now compare `identical`** — the reference was re-captured twice
in the fix round to get there, and the first version of this row was wrong about *two* of the four
filings:

- **15500** was `6a891af9…`, presented by **1 of 3** windows, while all three windows hold
  `be95db6c…`. The first re-capture (`out/b4b-sinfld-ref2`, a separate sitting) drew it: **1/3 → 3/3**.
- **16500** was `538942e6…` and the first version of this row recorded the keyframe as "no settled
  content". **That was a misapplication of §1.8 rule 3**: the candidates are the pictures seen **at
  `N`**, and among those the largest is `9d129ad0…` at **3 of the 4 sessions that hold a window**
  (`-sess1`, `-sess2`, the reviewer's `out/review-b4b/ray-sess`) — at or above half, so rule 4 does not
  apply and the keyframe is settled. The row's own census output said so two lines below the verdict.
- **16800** was `47a0b326…`, which the two sessions that present it take only at **`N−1`**; rule 3's
  caveat says the filed file must be one a session took *at* `N`, and `6e5994bf…` is that picture
  (also 2/3).
- **16000** kept its bytes throughout.

The re-captures are `out/b4b-sinfld-ref3` (6e5994bf at 16800 ✓, but `19a8a5b0…` at 16500 — a picture
no session takes at `N`, so not filable), then `out/b4b-sinfld-ref4` (missed both) and finally
`out/b4b-sinfld-ref5`, which drew all four targets and **is the filed reference — its four frames and
manifest were copied into `reference/parts/3dsinfld/`** (the table in §16.2 is that copy's `sha256`,
byte-for-byte). The terrain's phase
differs between sessions by whole two-frame steps, so a re-capture here is a draw from a **small** set
— two or three pictures, unlike JPLOGO's hold, whose state set turned out to be wide (§17.3).

**The census**, by §1.8's rule, over the sessions that hold a window (three for 15500/16000/16800 —
the reviewer's `ray-sess` holds no window there — and **four** for 16500, which it does hold):

| keyframe | sessions with a window | pictures at `N` | largest support | filed reference is it? | ≥ half the sessions? |
|---|---|---|---|---|---|
| 3DSINFLD 15500 | 3 | 3 | `be95db6c…` **3/3 (100 %)**, taken at `N` by one session and in all three windows | **yes** | yes |
| 3DSINFLD 16000 | 3 | 2 | `2d349ada…` **3/3 (100 %)** (tied with `2744df3f…`, also 3/3) | **yes** | yes |
| 3DSINFLD 16500 | **4** | 3 | `9d129ad0…` **3/4 (75 %)**, taken at `N` by **three** sessions | **yes** (after the re-captures) | yes |
| 3DSINFLD 16800 | 3 | 2 | `6e5994bf…` **2/3 (67 %)**, taken at `N` by **both** | **yes** (after the re-captures) | yes |

**All four keyframes are settled**, and the residual the row reports is a phase group rather than an
unsettled picture: `-sess3` sits two two-frame steps away at 16500 and 16800 and reports 38.9031 %
and 41.7875 %, which is what a faithful port would also have to be diffed against `-sess1`/`-sess2`'s
identical verdicts to interpret.

## 17. JPLOGO

`SECOND 3`'s seventh and last part. `JPLOGO/MAKEFILE:24` builds `jp.exe` and stages it as
`jplogo.exe`; the sources are `JPLOGO/JP.C` and `JPLOGO/ASM.ASM` (linked with `_PIC.OBK` and
`READP.C`). **320×400 Mode X**: `mov ax,13h / int 10h` (`JP.C:173-174`) then `inittwk()`
(`ASM.ASM:36-74`), which writes **CRTC 0x09 = 0 (400 rows)**, 0x14 = 0, 0x17 = `0xe3`, turns chain-4
off and clears 64 KB — and never writes 0x13, so the stride stays mode 13h's offset 40, **80 bytes to
a plane row**, which is what `lineblit` (`ASM.ASM:76-100`) writes.

**What it draws.** The "Jellypic" — a woman in armour on a white horse with a sword — read row by row
out of the picture object (`readp`, `JP.C:181-186`) into 400 rows of 184 bytes and drawn with
`linezoom(vram+y*80, row[b], a)` (`JP.C:96-99`). Three phases:

- an **entry** of ~83 frames (`while(y>0) { y -= a; a += 6; scrolly(y/64); dis_waitb(); }`,
  `JP.C:210-220`), in which `scrolly` (`:30-45`) walks **CRTC 0x0d and 0x0c** down the picture and the
  picture slides in. It is *held in pairs*: `out/b4b-jp-sess1` and `-sess2` both hold one `sha256`
  (`4b31e501a3…`) at 17300 **and** 17301, and every session so far has it at 17300;
- a **wobble** after a music wait (`:71`, `while(!dis_exit() && dis_musplus()<4)`):
  17400→17500 10.8250 %, 17500→17600 9.9680 % in `out/b4b-jp-sweep1`;
- a **hold** of ~350 frames: `out/b4b-jp-sess1` is one `sha256` (`c8d6faa0…`) at 17500, 17501, 17600,
  17699, 17700, 17701, 17800 and 17850, while 17499 is `50e5a1c6…` and 17900 is the white-out. The
  source explains a settle — `doit` recomputes `y1`/`y2` only while its own `frame <= 511`
  (`JP.C:79-83`) and the tables saturate (`framey1[95:] = 0`, `framey2[95:] = 6400` by the source's own
  arithmetic) — but **which state it holds is the session's** (below): the states are discrete and
  they recur, so the hold is not by itself a reproducible target and §1.8's census decides.

**Its extent is from (17000, 17100] to (17900, 18000]** — about 900 frames — and it is the last
320×400 screen of this boot (the 320×200 screen after it is **U2E's**, not the ending's — §18.1,
§18.6). 17000 is the loader's flat black; 17900 is a **white rectangle** (2
colours, 74221 px white in `b4b-jp-sweep1`) which is ENDLOGO's opening white over JPLOGO's leftover
memory (`END/END.C:61-63` points the DAC's write index at 0 and writes 63 into 765 **bytes** — DAC
entries 0–254; entry 255 is not written).

### 17.1 The sweep, and what the sheet showed

```
out/b4b-jp-sweep1  17100,17200,17300,17400,17500,17600,17700,17800,17900,18000,18100,18300,18600,18900
                   exit 2, refused at 18000: "frame 18000 is 320x200 where the capture started at 320x400"
```

| index | what is on screen |
|---|---|
| 17100 | a band of the picture across the top of the frame: the entry has just begun. |
| 17200, 17300, 17400 | the picture further in — the woman, the horse's head, the sword — with black bands still at its edges. |
| 17500–17800 | the picture at full size, wobbling, the scale's comb pattern at its edges. |
| 17900 | a white rectangle on black — ENDLOGO's white, not this part's drawing. |

### 17.2 The keyframes, and why these

| index | what is on screen | frame files | `sha256` |
|---|---|---|---|
| 17300 | the picture **mid-entry** | `.rgb` only | `4b31e501a342b9797696c09e274d5fe048bd1304aa5712ce46cf92ecdffdb3d3` |
| 17500 | the picture **wobbling** | `.rgb` only | `aab611ef273c3286ac0329ca295eedf263cdd55ffa4997ed222946b09f60d976` |
| 17700 | the picture **held** in the reference's own session | `.rgb` only | `91d6ce6beb6bcec4401c8e17d96dac837e6ec4e2bd69a15dffa51cfee8295c41` |

All three are RGB-only: this part's own rail refuses the indexed read loudly (§17.4) and D15's
moving-screen rule refuses the rest.

### 17.3 Held or moving: the hold is real, its states are discrete, and they recur

| keyframe | neighbours in the sessions | reading |
|---|---|---|
| 17300 | `-sess1`: 17299 `87c7fd8d…`, 17300 = 17301 = `4b31e501…`; all four sessions the same | **held** (in pairs at least), and the picture every session agrees on |
| 17500 | `-sess1` `f8896c45…`, `-sess2` = `jp-sess` = the reference `aab611ef…`, `-sess3` `fe7c6802…` | **held per session — two distinct states across the four sessions** |
| 17700 | part of each session's long hold: `-sweep1` `ae2c3d08…`, `-sess1` `c8d6faa0…`, `-sess2` = `jp-sess` `ec3de2bb…`, `-sess3` `c60e2fbc…` | **held per session — three distinct states across five captures** |

**The states are discrete, they recur, and the set of them is not small.** `out/review-b4b/jp-sess`'s
17700 is **byte-identical** to `out/b4b-jp-sess2`'s — 0 px of 128000 differ, measured here — and so is
its 17500; that is the reviewer's fresh session landing in an existing state, so a session picks one
rather than getting a unique picture. Across four sessions and a sweep there are **three** states:
`{sess2, jp-sess}`, `{sess1}`, `{sess3}`, with the sweep's fourth. **But the set a session picks from
is wide, and fix round 1 measured how wide**: across twelve captures of the recipe (the four census
sessions, the sweep, and **seven re-capture probes**, `out/b4b-jp-ref3…-ref9`) there are **ten
distinct byte-images at 17700** and nine at 17500, and the only repeat among the probes is `-ref5` =
`-ref9` — the whole state, `4b94ea6d…` at 17500 and `e9bc3a26…` at 17700, drawn twice. So a state does
recur, at both scales, and no *particular* state — `ec3de2bb…` included — is reliably reachable by
re-capturing (§17.5).

**The states differ by *patches* of rows, not by a band and not by a shift.** Row-by-row counts
between the states at 17700:

| pair | differing px | rows with no difference at all |
|---|---|---|
| `sess2` vs `jp-sess` | **0** | all 400 |
| `sess1` vs `sess3` | 13519 | rows 0–17 and 159–399 (the difference is confined to rows 18–158) |
| `sweep1` vs `sess1` | 39919 | only rows 397–399 |
| `sess1` vs `sess2` | 53138 | none |
| `reference` vs `sess1` | 37852 | only rows 393–399 |

No integer vertical shift explains any of them (best +0 at 31.19 % for sweep1 vs sess1; ±1…4 are
35–41 %), and the same picture and palette are in all of them (56 of 57 colours in common). **The
shape is a palimpsest**: `doit` redraws a row only when its `(b, a)` differ from the last time
(`lasty/lasts`, `JP.C:96-99`), so a row whose parameters repeat keeps whatever an *earlier* state
drew there — and which rows those are is what differs between sessions.

**What is modelled and what is not.** A replication of `JP.C:110-165`'s integer arithmetic gives
`framey1[95:] = 0` and `framey2[95:] = 6400`, so `(y1, y2) = (0, 400)` for every doit frame from 379
up — the state that fills all 400 rows. **The hypothesis this block records for the variation** is
that the doit counter can jump past 511 in one step — `frame += waitb()` (`JP.C:74-77`) adds however
many vblanks the iteration took — which would freeze `y1`/`y2` at a pre-saturation value instead of
the (0, 400) one, and the palimpsest then preserves different rows. `a` is **not** the uninitialised
variable it might look like: `for(a=0;a<200;a++)` leaves `a` at 200 deterministically before `la=a`
at `JP.C:140`. The hypothesis is recorded as a hypothesis, not a finding, the way §13.5 records
PLZPART's plasma spread.

**And the consequence for §1.8 is the opposite of what §17.3 first said.** With four sessions in the
census, 17500's filed `aab611ef…` has **2/4 (50 %)** — settled — and 17700's largest state taken at
`N` is `ec3de2bb…` at **2/4**, also settled, while the first reference's `91d6ce6b…` is presented by
**0/4**. Both earlier "no settled content" verdicts were knife edges of a three-session count, not
statements about the part. **What the census settles is the *state*, and §17.5 records that the
*file* at 17700 could not be brought into it**: seven re-capture probes did not draw `ec3de2bb…`.

### 17.4 The racy offset read, measured on this part

`out/b4b-jp-sweep1`'s log holds **eight `PlaneTooSmall` refusals, every one naming a 320-byte row**
for a picture whose rows are 80. 320 bytes = 160 register units, and 160 is the low byte of `y*80`
for `y ≡ 2 (mod 16)` — exactly what `scrolly` writes to CRTC 0x0d during the entry (`JP.C:36-45`).
**The read returned 0x0d's value as if it were 0x13's** (§1.7's mechanism, in its loud half: at 400
rows a 320-byte stride needs 128000 bytes a plane). **JPLOGO writes CRTC registers, so §1.9's
hand-check rule applies to it** — no `.idx` was filed from the reference (all three keyframes are
RGB-only), and the one the *partners* produced was checked anyway: `out/b4b-jp-sess2`'s
`frame_017700.idx` reproduces its RGB with **0 mismatches of 128000 pixels** through the manifest's
palette (59 indices, 57 distinct colours), so the held screen's plane is real at the 80-byte stride
and not a sheared one.

### 17.5 The re-capture and the census

```bash
# the reference: keyframes only
cargo xtask reference capture --arg 3 --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames 17300,17500,17700 --out reference/parts/jplogo --sheet out/jplogo.png

# the partners: a support frame each side of every keyframe
cargo xtask reference capture --arg 3 --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames 17299,17300,17301,17400,17499,17500,17501,17600,17699,17700,17701,17800,17850,17900 \
    --out out/b4b-jp-sess1        # and -sess2, -sess3

# the seven re-capture probes of fix round 1: the reference's own command, re-run
cargo xtask reference capture --arg 3 --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames 17300,17500,17700 --out out/b4b-jp-ref3    # … -ref4 … -ref9
```

```
$ cargo xtask compare --reference reference/parts/jplogo --rendered out/b4b-jp-sess1 \
      --out out/b4b-jp-cmp1
frame 17300: 0.0000% differing (0/128000 px), max delta 0
frame 17500: 41.3773% differing (52963/128000 px), max delta 255
frame 17700: 29.5719% differing (37852/128000 px), max delta 255
verdict: different, mode: rgb, offsets {"0": 3}
exit: 1

$ … --rendered out/b4b-jp-sess2
frame 17300: 0.0000% differing (0/128000 px), max delta 0
frame 17500: 0.0000% differing (0/128000 px), max delta 0
frame 17700: 40.1914% differing (51445/128000 px), max delta 255
verdict: different, mode: rgb, offsets {"0": 3}
exit: 1

$ … --rendered out/b4b-jp-sess3
frame 17300: 0.0000% differing (0/128000 px), max delta 0
frame 17500: 39.3516% differing (50370/128000 px), max delta 255
frame 17700: 27.3664% differing (35029/128000 px), max delta 255
verdict: different, mode: rgb, offsets {"0": 3}
exit: 1

$ … --rendered out/review-b4b/jp-sess          # the reviewer's independent session
frame 17300: 0.0000% differing (0/128000 px), max delta 0
frame 17500: 0.0000% differing (0/128000 px), max delta 0
frame 17700: 40.1914% differing (51445/128000 px), max delta 255
verdict: different, mode: rgb, offsets {"0": 3}
exit: 1
```

**The census**, by §1.8's rule, over the four sessions that hold a window:

| keyframe | sessions with a window | pictures at `N` | largest support | filed reference is it? | ≥ half the sessions? |
|---|---|---|---|---|---|
| JPLOGO 17300 (held) | 4 | 2 | `4b31e501…` **4/4 (100 %)**, presented at `N` by **all four** (and at `N+1`) | **yes** | yes |
| JPLOGO 17500 | 4 | **3** (six pictures in the windows) | `aab611ef…` **2/4 (50 %)** — the filed one | **yes** | yes, exactly half |
| JPLOGO 17700 (held) | 4 | 3 | `ec3de2bb…` **2/4 (50 %)** — the state `-sess2` and the reviewer's `jp-sess` both hold | **no** — the filed `91d6ce6b…` is presented by **0/4**, and seven re-captures did not draw the state | yes for the state, no for the file |

**The census is over four sessions, not three** — `out/review-b4b/jp-sess` holds `17299,17300,17301`,
`17499,17500,17501` and `17699,17700,17701` and is a genuine session of the same recipe, so it counts
(§1.8 rule 1). The first version of this row counted three, and on three the same numbers read 1/3 and
0/3 — **both keyframes are knife edges of the count, not statements about the part** (§17.3).

**17500 is settled as filed**: `aab611ef…` is at `N` in two of the four windows and pairs at 0.0000 %
— the comparison above records it, against `-sess2` **and** against the reviewer's `jp-sess`, which
holds the same bytes. **17700 is settled as a *state* and unfiled as a *file*.** §1.8's count over the
sessions that hold a window — four — gives `ec3de2bb…` **2/4 (50 %)**, taken at `N` by both of the
sessions that present it, and gives the first reference's `91d6ce6b…` **0/4**. §1.8's remedy for a
filing that sits outside its own census is a re-capture, and **it was run and did not reach the
state**: seven probes this round (`out/b4b-jp-ref3` … `out/b4b-jp-ref9`), each the same recipe,
keyframes only, each ~10 minutes of emulated run, drew at 17700 `bd189d56…`, `b617d280…`,
`e9bc3a26…`, `267894aa…`, `871848e1…`, `27ca885d…` and `e9bc3a26…` again — **six distinct states,
none of them held by any census session** — and one of them (`-ref4`) reproduced 17500's filed
picture (`aab611ef…`) while drawing an unheld state at 17700, which is what a draw from this set
looks like (§17.3 measured the set: ten distinct byte-images at 17700 across twelve captures). The
reference therefore keeps 17500 = `aab611ef…` (settled, 2/4) and 17700 = `91d6ce6b…` (**taken at `N`
by none of the four**), and this row records that index as having **no settled filing**: the state the
sessions hold exists and is countable, but no capture in this block drew it, so a Phase 2 diff at
17700 must expect a residual there and must not read it as a port error (§1.8, §17.3).

---

## 18. U2E

`SECOND 4`'s only part — block 4 is the vector part and nothing else. `VISU/C/MAKEFILE:45` builds
`u2e.exe` and stages it into `main\data`; the source is `VISU/C/U2E.C` (628 lines) over the VISU
library. It is the phase's only part whose *screen size depends on which boot reaches it*.

**What it draws.** A copper-driven scene player: `readfile(scene.00M)` plus animation streams
(`.0AA`, `.0XY`), drawn with the `vid_*` primitives and switched with `dis_setcopper(2, copper2)` and
`vid_window(0,319,25,174,512,9999999)` (`U2E.C:352-353`). The copper (`:115-141`) rotates **four
display buffers** through the CRTC start address (`vid_setswitch(-1, clr)`), counting `syncframe`
once a vertical blank, and the main loop (`:376-540`) services the animation stream with
`currframe += repeat*2`, ending when the stream ends (`xit`) or the demo exits.

### 18.1 Which boot reaches it, and what changes between boots

- **Two boots reach it: `SECOND 3` and `SECOND 4`.** `partsmask[3]` (what `SECOND 3` selects) is the
  literal `11100b` (`MAIN/U2.ASM:110-119`) — **0x1c = 28, bits 2, 3 and 4** — and the table's own
  comment names them "`&4=mount..comanche &8=vector &16=scrollers`". Bit 3 is set, so the
  `test whattorun,8` at `:880-887` runs `exe19` (U2E) on a `SECOND 3` boot too, after JPLOGO and
  before the scrollers. `SECOND 4` = `partsmask[4]` = `11000b` = `0x18` = bits 3 and 4 runs U2E as the
  **first** show part after the loader. **This row's indices are `SECOND 4` boot indices.**
  *(Corrected by fix round 1: the first version of this row said `SECOND 3` could not reach U2E. It
  can, and the same screens appear on it at different boot indices — measured, see the next bullet.)*
- **The screens do not change between the two boots — the indices do.** With no mode set, U2E calls
  `vid_init(1)` itself; after JPLOGO it sets `jellywas = 1` and skips it (`U2E.C:195-215`). **But it
  normalises the display either way**: `fadeset()` on all four 64 KB buffers and **CRTC 9 =
  `(old & 0xf0) | 0x80`** — scan doubling, a 200-line display — at `U2E.C:302-313`, in the same
  planar 80-byte layout the reference is captured in. **Measured:** `out/review-b4b/s3-after`'s 18000
  and 18100 (a `SECOND 3` boot) are `befaf38ae1…`, byte-identical to the `SECOND 4` boot's opening
  hold (`out/b4b-u2e-sweep2`'s 530 = 590 = 660), and `out/b4b-s3-u2e2` reproduces that independently
  (§18.6). *(Corrected by fix round 1: the first version said a full run would draw into JPLOGO's
  320×400 screen. It does not — it re-establishes the 200-line display itself.)*
- **The music is `MUSIC0` at order 18** (`U2.ASM:882-885`, immediately before `exe19`) — the restart
  the brief names. On `SECOND 4` the initial load starts `MUSIC1` at order 0 (`:767-775`, block 1
  skipped) and U2E's restart replaces it; on `SECOND 3` the block-2 group has run first, so what a
  given boot index shows before U2E's own restart is not the same on the two boots.
- **U2E writes the CRTC every frame**, through `vid_setswitch(-1, clr)` → `_vid_setswitch` writes
  register 0x0c (`VISU/AVID.ASM:341-357`), and CRTC 9 once at its start (`:305-308`). §1.9's
  hand-check rule therefore applies to any `.idx` filed from it; none is.

### 18.2 The sweeps, and what the sheets showed

```
out/b4b-u2e-sweep   420,500,600,700,800,900,1000,1200,1400,1600,1800,2000,2300,2600,3000,3500,
                    4000,5000                                                 exit 0 (18 frames)
out/b4b-u2e-sweep2  430,450,470,490,520,560,640,760,820,…                  exit 2, refused as a
                    *miss* at 520: four indices 20 apart are one 4-index batch (§1.3) and the next
                    index was only 30 frames past its read
out/b4b-u2e-sweep2  430,480,530,590,660,760,850,950,3100,3200,3400,3700,3900,3950,4050,4100,
                    4200,4400,4600,4800                                       exit 2, refused at
                    4400: "frame 4400 is 320x400 where the capture started at 320x200"
```

| index | what is on screen |
|---|---|
| 420 | flat black, one colour, `ea0787f6…` — still the loader's screen. |
| 500 | a palette fade-in under way: 4 colours, mean 168. |
| 600 = 700 | one `sha256`, `ef539f9e…` (4 colours, mean 158.30) — the fade's hold. |
| 800 | 37 colours, mean 133.61: a corridor scene with white door panels and a small metallic craft. |
| 900 | 53 colours — the same scene, the craft closer. |
| 1000 | 54 colours: a console scene, vertical red bars and instrument panels. |
| 1200 | 42 colours: a grey building/corridor in a red frame. |
| 1400, 1600, 1800 | 86–92 colours: the fly-by over a city of striped towers with a green craft. |
| 2000 | 11 colours, mean 133.06: an interior corridor, a scene change. |
| 2300 | 57 colours: a structure with white square windows on a red floor. |
| 2600, 3000 | 63–66 colours: the same structure from other angles. |
| 3500 | 51 colours: a purple-grey wall and a lattice. |
| 4000 | 40 colours, mean 92.59: **"Future Crew"** in 3-D letters over a red field. |
| 5000 | 118 colours, mean 12.84: **the credits screen** — `GRAPHICS - MARVEL / MUSIC - SKAVEN / CODE - WILDFIRE` — which is the ending, not this part. |

### 18.3 The keyframes, and why these

Four, spread 600–1000 frames apart across the part's ~3800 frames of drawing, one per screen the
sheets show: the corridor scene, the saucer under the towers, the scene change after the first dark
transition, and the crescent ships over a plane.

| index | what is on screen | frame files | `sha256` |
|---|---|---|---|
| 1000 | a dark grey-brown **corridor wall** with a grey panel at its left (36 colours, mean 81.52) | `.rgb` only | `9cf5d972d36168e7d3e4d7a767a88460abd4491b8f8632cc6a4948e557c6c37a` |
| 1600 | the **green saucer** in the foreground with grey buildings behind (79 colours, mean 80.25) | `.rgb` only | `e7a10cd78e7478d52ed64742b9209fc00dfb9ac9c9e198d1286be73eb2711811` |
| 2600 | the **saucer under vertical striped towers** (56 colours, mean 97.17 — the filed picture's own census; 89 colours / 81.79 is another session's frame at that index) | `.rgb` only | `58293d898530080a96bed5fdaa4c8a56821cb66dd115312cbaaecfa427aad2a0` |
| 3600 | two small **metallic crescent craft** over a grey plane on black (54 colours, mean 62.77) | `.rgb` only | `f8d4a0e8bd79d1f4e41178b63fc66eb8559eac76d7db0c2cda9d49cd24c3d23e` |

All four are RGB-only — D15's coherence rule refuses a screen whose buffers rotate under the read —
and **no `.idx` is filed from this part**. That matters because U2E writes the CRTC start address
every frame through `vid_setswitch` inside its copper (§18.1); had a plane been filed, §1.9's
hand-check rule would have applied to it, and no sheared one can have entered the tree.

### 18.4 Held or moving, measured

Every keyframe is **moving** — the scenes animate and the displayed buffer rotates under the read —
and the rotation is **not locked to a fixed period**, so a keyframe is not uniformly "two frames
long": the triples, read per session,

| keyframe | `out/b4b-u2e-sess1` | `-sess2` | `-sess3` | `out/review-b4b/u2e-sess` |
|---|---|---|---|---|
| 1000 | 999 = 1000, 1001 differs | 999 = 1000 | 999 = 1000 | 999 = 1000 |
| 1600 | three distinct | **three distinct** | **three distinct** | 1600 = 1601 |
| 2600 | three distinct | 2599 = 2600 | 2599 = 2600 | 2599 = 2600 |
| 3600 | 3599 = 3600 | three distinct | 3599 = 3600 | 3599 = 3600 |

A session's phase is therefore worth ±1 frame at some keyframes and not at others: where it is, the
±1 window lands on the same picture and the comparison reports **0.0000 %**; where it is not, the
session sits two or more buffer steps away and its frame differs by **55.4672–60.8406 %**. The
rotation rate comes from the schedule the main loop writes into `cl[].frames` (`U2E.C:434-444`),
which is read off `syncframe` — a music-synced counter — so the drift is the same animal as
3DSINFLD's two-frame terrain steps (§16.4), not a difference in the kind of screen.

### 18.5 The re-capture and the census

```bash
# the reference, re-captured once after the first census showed 2600 outside every group
cargo xtask reference capture --arg 4 --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames 1000,1600,2600,3600 --out reference/parts/u2e --sheet out/u2e.png

# the partners: a support frame each side of every keyframe
cargo xtask reference capture --arg 4 --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames 999,1000,1001,1599,1600,1601,2599,2600,2601,3599,3600,3601 --out out/b4b-u2e-sess1
```

```
$ cargo xtask compare --reference reference/parts/u2e --rendered out/review-b4b/u2e-sess \
      --out out/b4b-u2e-cmp-rev
frame 1000: 0.0000% differing (0/64000 px), max delta 0
frame 1600: 0.0000% differing (0/64000 px), max delta 0
frame 2600: 0.0000% differing (0/64000 px), max delta 0
frame 3600: 0.0000% differing (0/64000 px), max delta 0
verdict: identical, mode: rgb, offsets {"0": 4}
exit: 0
```

**The reviewer's independent session reproduces all four keyframes byte for byte** — `exit 0`,
`verdict: identical` — which is the strongest form this phase can file. Against my own three partner
sessions the report is `exit 1`, `mode: rgb`, offsets `{0: 4}`, with each session missing exactly one
keyframe: `-sess1` misses 2600 (58.4328 %), `-sess2` misses 3600 (55.4672 %), `-sess3` misses 1600
(60.8406 %).

**The census**, by §1.8's rule, over the four sessions that hold a window:

| keyframe | sessions with a window | pictures at `N` | largest support | filed reference is it? | ≥ half the sessions? |
|---|---|---|---|---|---|
| U2E 1000 | 4 | 2 | `9cf5d972…` **4/4 (100 %)**, taken at `N` by all four | **yes** | yes |
| U2E 1600 | 4 | 6 | `e7a10cd7…` **3/4 (75 %)**, taken at `N` by three | **yes** | yes |
| U2E 2600 | 4 | 5 | `58293d89…` **3/4 (75 %)**, taken at `N` by three | **yes** (after the re-capture) | yes |
| U2E 3600 | 4 | 5 | `f8d4a0e8…` **3/4 (75 %)**, taken at `N` by three | **yes** | yes |

**Every keyframe is settled, and the re-capture is what made 2600 one.** The first reference's 2600
was `085bffff…`, which appears in **none** of the sessions' windows (48.6516 %, 57.0719 % and
57.0719 % away against three of them), while the picture three sessions draw at `N` is
`58293d89…`; re-capturing moved the filed picture into that group and its support from **0/4 to
3/4**, and 1000, 1600 and 3600 kept the bytes they had. U2E's timeline drifts between sessions — by
2600 and 3600 some sessions sit two or three buffer steps apart, which is why those keyframes are 3/4
rather than 4/4 — but every keyframe is inside three of the four sessions' ±1 windows, **which is what
"settled" means here**.

### 18.6 `SECOND 3` reaches U2E too — measured

`MAIN/U2.ASM:110-119`'s `partsmask` entries are TASM **binary literals**: `db 11100b ;3` is
`0b11100` = `0x1c` = bits 2, 3 and 4, and the table's comment names bit 3 "`vector`". `test
whattorun,8` at `:880-887` therefore runs `exe19` (U2E) on a **`SECOND 3` boot** as well, after JPLOGO
and before the scrollers. *This ledger's first version of this row said `SECOND 3` could not reach
U2E; that was a misreading of `0x1c`, and it propagated to the row, §18.1, §1.10, `PROGRESS.md` and
the plan's B5 bullets before fix round 1 caught it.*

| index (`SECOND 3`) | frame | what it is |
|---|---|---|
| 17900 | `668f1e1364…`, 320×400 | the white-out over JPLOGO's memory — **not** U2E. My own first attempt, `out/b4b-s3-u2e`, was refused at 18000 for exactly that reason: *"frame 18000 is 320x200 where the capture started at 320x400"* |
| 18000 | `befaf38ae151…`, 2 colours, mean 191.25 | **U2E's opening hold**, byte-identical to the `SECOND 4` boot's `out/b4b-u2e-sweep2` 530 = 590 = 660 |
| 18100 | `befaf38ae151…` | the same hold |
| 18200 | `09e7a22c15d9…`, 42 colours, mean 138.70 | U2E's first scene resolving |
| 18500, 19000, 19500, 20000, 20500, 21000 | 44, 82, 13, 57, 52, 54 colours | U2E's scenes — the same kind of frames as the `SECOND 4` boot's 800–3600 |

(`out/review-b4b/s3-after`, a `SECOND 3` capture of 9 frames, read off here; `out/b4b-s3-u2e2` is this
block's own repeat of it — a separate session on the same boot — and its 18000 and 18100 are
`befaf38ae151…` too, 2 colours, mean 191.25, with a `.idx` each, so the identity is reproduced rather
than read off one session. Its later frames are the same scenes in another phase — 18500 differs from
this table's 18500 by 9.5307 % of bytes, the §18.4 shape — where the opening hold is byte-exact.)

**So the 320×200 screen on a `SECOND 3` boot after JPLOGO is U2E's and it is ~4000 frames long**, and
the ending is only after it — which is what the plan's B5 bullet now says (§1.10's last item). Two
things follow that the rows use: the reference in `reference/parts/u2e/` is **usable on any boot that
reaches U2E** (the screens are byte-identical), and its **indices are `SECOND 4` boot indices**, so a
`SECOND 3` or full-run diff must use the indices of *that* boot.

### 18.7 The part's first drawn frame, measured (Phase 6)

U2E had no start index before Phase 6; it has one now. On the `SECOND 4` boot the loader's
screen is the flat black `ea0787f6…` through frame 466, and **467** is the first frame with a
second colour: a white band at columns 68–252 on black, 185 white and 135 black pixels in every
row (37,000 white, 27,000 black), `6eb4e4cd…` — the frame the intro's `fadeset` draws. The
probe, run twice in independent sessions:

```bash
cargo xtask reference capture --arg 4 --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames 456,457,458,459,460,461,462,463,464,465,466,467,468,469,470,471,472,473 \
    --out out/u2e-dense-a
# the same command into out/u2e-dense-b: every index's sha256 is identical, 456…466 the
# loader black and 467…473 the band.
```

The plan's coarser grid (`--frames 404,410,420,430,440,450,460,470,480,490,500`) cannot start
at 404 on this host — the capture's demo-start detection returns at frame 419–420, and the
command was refused twice with `the emulator reached frame 419/420 before frame 404 could be
captured`. Run from 420, both sessions agree with the dense scan at every sampled index: 420…
460 are `ea0787f6…`, 470 is the band. The anchor the port numbers a `--block 4` run from is
therefore **467**, and the intro's earlier palette steps are invisible over the black screen
(they change entries the flat black does not use). `docs/decisions.md` D33 records the model
and the two-retrace cadence.

## 19. ENDLOGO

Part 18 of the sequence and the first part of `SECOND 5`'s block. `SECOND 5` is
`partsmask[5]` = the binary literal `10000b` (`MAIN/U2.ASM:116`) = `0x10` = **bit 4 only**, the
`&16=scrollers` bit, so that boot runs ENDLOGO, CRED and ENDSCRL and nothing else — the end
sequence **from its own beginning**, which is what this row's indices are. The shell does the
preparation itself before `exe2021`: `restartmus` for MUSIC0 at order 25, `mov ax,13h / int 10h`,
`inittwk`, then the part (`MAIN/U2.ASM:898-909`) — so the mode 13h screen ENDLOGO starts from is the
shell's, not the loader's.

The source is `END/END.C` (130 lines) over `END/ASM.ASM` (`lineblit`), `readp.c` and the packed
picture `_PIC.OBK`. **Its screen is 320×400 with 80-byte plane rows**: mode 13h puts the bytes one
per pixel, and `END.C:67-80` then writes CRTC 0x0c, 0x0d = 0 and **CRTC 9 = `(old & ~0x80) & ~0x1f`**
— the 400-line, one-scanline-a-row display — before `lineblit(vram+y*80, rowbuf)` walks **400** rows
(`END.C:95-99`).

**What it draws, in the order it draws it.** `END.C:51-63` unlocks the attribute controller, points
the DAC's write index at **0** (`outp(0x3c8,0)`) and then writes 63 **765 times**
(`for(a=0;a<768-3;a++) outp(0x3c9,63);`, `:62-63`) — that is 765 **byte** writes, one byte per
request, so it fills the three channels of DAC entries **0–254** and **leaves entry 255 unwritten**
(the fix round that corrected this sentence first asserted the opposite — "765 entries, 0–764, so
255 *is* written" — which confuses bytes with entries; three bytes make an entry, and 765 bytes stop
one entry short of the 256th). So the screen is a white field
over whatever the previous part's memory held — on this boot, over the loader's leftovers.
`:67-92` re-points the CRTC to start address 0, takes the display to 400 lines and whitens again,
then waits **32 vblanks**. Only then does it read its own picture (`readp(palette,-1,pic)`) and blit
400 rows (`:94-99`); a **129-frame palette fade-in** from the white follows (`:101-106`), then a
music wait — `for(a=0;a<5000 && !dis_exit();a++) { dis_waitb(); if(dis_musplus()>-16) break; }`,
`:107-111` — and a **64-frame fadeout** (`:112-117`).

**Its extent is from the boot's first graphics frame to the geometry change in (1200, 1240]** —
about 800 frames. Measured on the sweep `out/b5-s5-sweep2` (430…1200): 430 is a **plain white field,
one colour, mean 255.00**; 460 is 16 colours, mean 237.44 (the picture showing through the white);
500 is 56 colours, mean 167.31 (mid fade-in); 540 is 66 colours, mean 99.54; **580 is 69 colours,
mean 33.23 and every sample from there to 1120 is the same `sha256`** (`f2514b29…`); 1160 is 65
colours, mean 25.15 (the fadeout under way); 1200 is 30 colours, mean 4.29 (nearly black); and a
capture started on this screen is refused at 1240 — `frame 1240 is 320x200 where the capture started
at 320x400` — which is CRED's mode 13h screen (§20). **The white starts before 430**: a second run
(`out/b5-endo-probe`) measures 415 = 445 = the same `85789170…` white, so the part's opening white is
already up 15 frames earlier than the first sweep sampled and holds for at least 30 frames.
**The changeover itself is grab-hostile**: a capture asking for 1220 is refused for a different
reason (`the emulator reached frame 1221 before frame 1220 could be captured`), so the boundary is
narrowed from both sides rather than probed at (§1.2). The demo's own first graphics frame, 404, is
refused the same way (`reached frame 412`).

**A `SECOND 3` boot reaches this part too, at its own index.** `partsmask[3]` = `0x1c` has bit 4 set,
so the scrollers block runs there as well — after U2E, which on that boot is ~3900 frames long
(§18.6). Measured here rather than inferred: a sweep of that boot's tail (`out/b5-s3-tail`,
20500…23000) finds 20500, 21000 and 21500 on the 320×200 screen and is refused at 22000
(`frame 22000 is 320x400 where the capture started at 320x200`), so **U2E's screen ends in
(21500, 22000] and ENDLOGO's follows it**. That is the plan's B5 note's "~22 000" confirmed by a
sweep — and it is worth recording as the reason such a note exists: where the end sequence sits on a
boot is a property of that boot's music restarts and its parts' lengths, not of anything in ENDLOGO.

### 19.1 The sweeps, and what the sheets showed

```
out/b5-s5-sweep1  500,700,900,1100,1400,…            exit 2, refused at 1400 (320x200 where the
                                                     capture started at 320x400)
out/b5-s5-sweep2  430,460,500,540,580,620,660,700,740,780,840,900,960,1020,1080,1120,1160,1200,
                  1240,1280,1320,1360,1400,1450,1500   exit 2, refused at 1240, same message
```

| index | what is on screen |
|---|---|
| 430 | a **white field**, one colour, mean 255.00 — `END.C:61-63`/`:89-92`'s white over the loader's memory. |
| 460 | the same white with the picture faintly through it: 16 colours, mean 237.44. |
| 500 | the **fade-in under way**: the "FUTURE CREW" 3-D logo block, washed towards white, 56 colours, mean 167.31. |
| 540 | nearly settled, 66 colours, mean 99.54. |
| 580 … 1120 | the settled picture — the two rounded blocks lettered FUTURE and CREW on black, 69 colours, mean 33.23 — **byte-identical at every sample**. |
| 1160 | the **fadeout**: the same picture at 65 colours, mean 25.15. |
| 1200 | the fadeout's tail, 30 colours, mean 4.29. |
| 1240+ | (CRED, §20) mode 13h's 320×200 screen. |

The reference capture's own sheet, `out/endlogo.png`, is four tiles: **r0c0** = 430 (white),
**r0c1** = 500 (fade-in), **r1c0** = 900 (settled), **r1c1** = 1180 (fadeout under way).

### 19.2 The keyframes, and why these

| index | what is on screen | frame files | `sha256` |
|---|---|---|---|
| 430 | the **opening white** | `.rgb` + `.idx` | rgb `857891706b1698637cb83d2e7a6e005c5af26909187cc1426195a088fd1e2561` |
| 500 | the **fade-in** over the held picture | `.rgb` only | rgb `fb8fbf2a4401b08668adcc0edf890a46bf30e559649ec46ccf1c9823ce0f3a21` |
| 900 | the **settled picture**, held | `.rgb` + `.idx` | rgb `f2514b291f572e3dbd028cadd7b2795be972740b5466f8520b5b8da468ace94d` |
| 1180 | the **fadeout** | `.rgb` only | rgb `755f474763bf19617724d92a227e7b04df4cbee908366571c53c3c7753c0d670` |

One keyframe per state the part has: its opening white, its fade-in, its hold, its fadeout. The
gaps are **70** (430→500), **400** (500→900) and **280** (900→1180) — every one far more than the
~60 a three-index support batch needs (§1.3). 900 is chosen in the middle of the hold, not at its edge, because only the hold's
*start* is deterministic: the hold ends when the music reaches `dis_musplus() > -16` (`END.C:107-111`)
and the fadeout's 64 frames follow it.

Full hashes of the six files in `reference/parts/endlogo/`:

| index | file | `sha256` |
|---|---|---|
| 430 | `frame_000430.rgb` | `857891706b1698637cb83d2e7a6e005c5af26909187cc1426195a088fd1e2561` |
| 430 | `frame_000430.idx` | `eec19bc6af0b3b6dfb97a08782c65f4bb3c3203e789a015d2008b0d689ad08be` |
| 500 | `frame_000500.rgb` | `fb8fbf2a4401b08668adcc0edf890a46bf30e559649ec46ccf1c9823ce0f3a21` |
| 900 | `frame_000900.rgb` | `f2514b291f572e3dbd028cadd7b2795be972740b5466f8520b5b8da468ace94d` |
| 900 | `frame_000900.idx` | `c0e0e259e96dfffd984763818a7891c6af610bdcd8e282375a10336c6637b03c` |
| 1180 | `frame_001180.rgb` | `755f474763bf19617724d92a227e7b04df4cbee908366571c53c3c7753c0d670` |

### 19.3 Held or moving, measured — and the `.idx` hand-check

The triples around each keyframe, hashed inside `out/b5-endo-sess1` (and the same bytes in `-sess2`
and `-sess3` at every index):

| keyframe | `K-1` | `K` | `K+1` | reading | per-frame change of its own 128000 px |
|---|---|---|---|---|---|
| 430 | `85789170…` | `85789170…` | `85789170…` | **held** | 0 px either side |
| 500 | `8807e602…` | `fb8fbf2a…` | `bab01979…` | **moving** (palette fade) | 499→500 115331 px (90.1023 %), max delta 4; 500→501 41568 px (32.4750 %), max delta 5 |
| 900 | `f2514b29…` | `f2514b29…` | `f2514b29…` | **held** | 0 px either side |
| 1180 | `da11d611…` | `755f474f…` | `67eeee93…` | **moving** (palette fade) | 1179→1180 32911 px (25.7117 %), 1180→1181 42751 px (33.3992 %), max delta 5 |

The two moving keyframes are moving **in the palette and in nothing else** — their per-pixel
differences are large in *count* and tiny in *amplitude* (max channel delta 4–5), which is the
signature §1.4 records for BEGLOGO's fade, and it is why both came back RGB-only: an indexed read is
20+ frames late and 20+ fade steps have passed by the time the DAC is read, so D15's coherence rule
refuses the pair (§1.3). 430 is held outright — 32 vblanks of one white (`END.C:92`) — and 900 is
the middle of a ~570-frame hold.

**ENDLOGO writes CRTC registers** (0x09, 0x0c, 0x0d at `END.C:67-80`), so §1.9's hand-check rule
applies to the two `.idx` files it filed, and both were checked with `/tmp/b5/idxcheck.py` against
the manifest's own palette through the `(c*259+33)>>6` widening:

- `frame_000900.idx`: **72 distinct indices, 69 distinct RGB triples, no index carrying two triples,
  and 0 mismatches of 128000 pixels.** The palette does not collapse — 69 colours is enough for the
  check to have caught a shear — so this is a real check and the plane is not sheared.
- `frame_000430.idx`: 1 index, 1 triple, 0 mismatches of 128000 px. That is the §1.6 blind spot
  rather than a reading: a *one-colour* white screen has nothing for the coherence check to catch a
  shear with, so this `.idx` is filed on the strength of the screen being the part's own white
  (`END.C:61-63`) and of nothing else — it is the weakest `.idx` in this block and the row says so.

**And 430 is not index-compared, although the reference holds an `.idx` for it.** `compare` takes the
indexed path only when *both* sides hold one (`xtask/src/compare.rs:606,746`), and **no session holds
`frame_000430.idx`**: measured, `ls out/b5-endo-sess1` gives `frame_000429.idx frame_000899.idx
frame_000900.idx frame_000901.idx` and no 430, so that frame is judged on its pixels. **Only 900 is
index-compared.** The hand-check of 430's plane is therefore a check on the *filed artefact* and not
on a comparison this row ran: it says the reference's own `.idx` is honest, which is what any later
comparison in which both sides hold one rests on.

### 19.4 The re-capture and the census

```bash
# the reference: keyframes only
cargo xtask reference capture --arg 5 --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames 430,500,900,1180 --out reference/parts/endlogo --sheet out/endlogo.png

# the partners: a support frame each side of every keyframe
cargo xtask reference capture --arg 5 --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames 429,430,431,499,500,501,899,900,901,1179,1180,1181 --out out/b5-endo-sess1
    # and -sess2, -sess3, minutes apart
```

```
$ cargo xtask compare --reference reference/parts/endlogo --rendered out/b5-endo-sess1 \
      --out out/b5-endo-cmp1
frame 430: 0.0000% differing (0/128000 px), max delta 0
frame 500: 0.0000% differing (0/128000 px), max delta 0
frame 900: 0.0000% differing (0/128000 px), max delta 0
frame 1180: 0.0000% differing (0/128000 px), max delta 0
verdict: identical, mode: mixed, offsets {"0": 4}
exit: 0
```

**`out/b5-endo-sess2` and `-sess3` give the identical report** — `exit 0`, `verdict: identical`,
0.0000 %, offsets `{0: 4}`, `mode: mixed` — **900 is the one keyframe compared as indices**; 430 is
RGB because the reference holds an `.idx` its partners do not, and 500 and 1180 because the fade
refuses theirs (§19.3).
Three sessions of the same recipe, run separately, present the reference's bytes at its own index.

**The census**, by §1.8's rule, over every session in the tree that holds a window — the three
census sessions `out/b5-endo-sess1`–`-sess3`, and the fix round's own re-run of them,
`out/b5rev-endo-sess1b` (a `SECOND 5` capture of the same recipe, its frames byte-identical at every
index). The scan covers all **319** directories under `out/` and `reference/parts/` that hold frames,
not just those with manifests, so a refused run holding a window would have counted — and it is
**filtered by recipe** as §1.11 records, which here excludes **ten** wrong-boot window holders:
`out/b2-batch-probe`, `out/review-b3-readcost` and `out/rev-batch-probe` at 429–431 (all three
holding `85789170…`, the white a `SECOND 2` boot produces just as a `SECOND 5` one does),
`out/b4-mnt-sess1`–`-sess6` at 499–501 (MNTSCRL's `e68aa887…` on the `SECOND 3` boot) and
`out/b3-alku-probe` at 899–901 (ALKU's `fec9afb5…`). **A fifth same-recipe directory holds 430's
window and is not counted**: `out/b5rev-endo-sess1`, a refused run of the fix round that carries no
manifest — it holds 429–431, but its boot cannot be established from its bytes, which are the very
white a `SECOND 2` boot gives at the same index (§1.11); it is named here rather than folded in.

| keyframe | sessions with a window | pictures at `N` | largest support | filed reference is it? | ≥ half the sessions? |
|---|---|---|---|---|---|
| ENDLOGO 430 (held) | 4 | 1 | `85789170…` **4/4 (100 %)** | **yes** | yes |
| ENDLOGO 500 | 4 | 1 | `fb8fbf2a…` **4/4 (100 %)** | **yes** | yes |
| ENDLOGO 900 (held) | 4 | 1 | `f2514b29…` **4/4 (100 %)** | **yes** | yes |
| ENDLOGO 1180 | 4 | 1 | `755f4747…` **4/4 (100 %)** | **yes** | yes |

Every session's window holds the filed picture **at offset 0**, and the neighbours each session
holds at `N±1` are the *same three* pictures in all four sessions (`8807e602…`/`fb8fbf2a…`/`bab01979…`
at 500, `da11d611…`/`755f4747…`/`67eeee93…` at 1180) — so this part's timeline is **anchored to the
boot index** the way MNTSCRL's, LNS&ZOOM's and MINVBALL's are (§23), music-gated steps included.

**What Phase 2 will see.** Every keyframe is a settled target: a port that draws ENDLOGO's white at
the boot index this row names, its fade-in 70 frames later, its picture, and its fadeout, will diff
at 0.0000 %. The two fade keyframes are palette-only, so a port whose *picture* is right but whose
fade curve is off will show a difference in every pixel at a small max delta there — that is the
fade's arithmetic (`(128-c)*63+palette[a]*c)/128`, `END.C:103`), not the geometry. There is no
unsettled index in this part: no residual is expected at any of the four.

---

## 20. CRED

Part 19 and `SECOND 5`'s second part (`exe22`, `MAIN/U2.ASM:912-913`). The source is
`CREDITS/MAIN.C` (318 lines) over `CREDITS/TWEAK.ASM` (the same `tw_*` tweak library PAM and ALKU
use) and 21 packed pictures in `CREDITS/PICS/`.

**What it draws.** 21 credit screens in a row, each the same shape: `screenin(pic,text)`
(`MAIN.C:161-213`) clears the screen, sets the picture's palette, lays the picture into a band and
the credit text under it, **opens an iris** over ~127 frames
(`for(y=200*128;y>0;y=y*12L/13)`, `:176-192`), **holds for 200 frames** (`:194`), then **closes the
iris** over ~59 frames (`for(y=0,v=0;y<128*200;y=y+v,v+=15)`, `:196-212`). The iris is
`tw_setsplit(y/128+200)` — the line-compare register — plus `tw_setstart(160*200 + yy/4)` and a
horizontal-pan write through the attribute controller (`:182-191`); `tw_opengraph`
(`TWEAK.ASM:18-...`) had set mode 13h, chain-4 off, **CRTC 0x13 = `0x5013`** ("640 wide"), CRTC 9 =
`0x0109`, CRTC 0x18 = `0x64` and the AC's colour-select register.

**The five keyframes are the 1st, 6th, 12th, 18th and 21st of its 21 holds**, and the sweep that
found them (`out/b5-s5-cred-map`, every 100th frame from 1240 to 8840, 77 frames) is how each hold
is placed:

| screen | hold samples (byte-identical) | `sha256` prefix | what the sheet shows |
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

**Its extent, measured.** It begins at the geometry change in **(1200, 1240]** — 1200 is ENDLOGO's
320×400 screen, 1240 is CRED's 320×200 (`out/b5-s5-sweep3b`'s refusal names the boundary:
`frame 1240 is …`; `out/b5-s5-sweep4a` narrows the other end) — and it ends at the change in
**(8840, 8940]**, where 8840 is still this part's last hold and 8950 is ENDSCRL's 640×400 screen
(`frame 8950 is 640x400 where the capture started at 320x200`). That is **21 screens in ~7700
frames**, which is the source's own count: `MAIN.C:41-156` calls `screenin` 21 times
(`pic1`…`pic18`, with `pic5b`, `pic10b` and `pic14b` making three of them a second screen of the
same picture).

### 20.1 The keyframes, and why these

| index | what is on screen | frame files | `sha256` |
|---|---|---|---|
| 1440 | screen **1**'s hold: the "Code PSI / Trug / Wildfire" card | `.rgb` only | `f1ea2ca5bb03…` |
| 3240 | screen **6**'s hold: the dotted vortex | `.rgb` only | `98de65f36999…` |
| 5440 | screen **12**'s hold: the tiled texture | `.rgb` only | `cc2df71ea4fa…` |
| 7640 | screen **18**'s hold: the drawn figure | `.rgb` only | `921555739373…` |
| 8740 | screen **21**'s hold: the closing "Second Reality" card | `.rgb` only | `cd5f27a573c9…` |

One keyframe in every third or fourth screen's hold, 1800–2200 frames apart — far more than the ~60
a three-index support batch needs (§1.3) — plus the part's own last hold. **The holds are where the
keyframes belong and this part has nothing else that stands still**: the iris moves the picture and
its palette is set once per screen.

Full hashes of the five files in `reference/parts/cred/`:

| index | file | `sha256` |
|---|---|---|
| 1440 | `frame_001440.rgb` | `f1ea2ca5bb03fbd170978ea61f81357f5150f4e94e66e68a447d69a8b8d19598` |
| 3240 | `frame_003240.rgb` | `98de65f369992a08f966d671b1ef52c3376121741ec4ffe0171c55b8467d925a` |
| 5440 | `frame_005440.rgb` | `cc2df71ea4fabd7c680b189876a52e70ea954d0763e94ce069cc71849da6668d` |
| 7640 | `frame_007640.rgb` | `9215557393733c471c344096934035511f4f06b42efd5029423118eb352eb9f7` |
| 8740 | `frame_008740.rgb` | `cd5f27a573c90adea0e1a98969b89b57b35b2a170ec014ffbf33d1aa07271797` |

### 20.2 Held or moving, measured — and why no `.idx`

Every keyframe is **held**, and held across sessions, not just within one: in each of
`out/b5-cred-sess1`–`-sess3` the three frames around every keyframe are **byte-identical**, and the
bytes are the same three sessions:

| keyframe | `K-1` = `K` = `K+1` in every session | reading |
|---|---|---|
| 1440 | `f1ea2ca5…` | **held** |
| 3240 | `98de65f3…` | **held** |
| 5440 | `cc2df71e…` | **held** |
| 7640 | `92155573…` | **held** |
| 8740 | `cd5f27a5…` | **held** |

The same three sessions' sweeps agree: `out/b5-s5-cred-map`'s 1340 = 1440, 3140 = 3240 = 3340,
5340 = 5440 = 5540, 7540 = 7640 = 7740 and 8640 = 8740 = 8840. So CRED is the MNTSCRL shape (§23):
a part whose timeline is anchored and whose pictures are the same bytes at the same boot index.

**All 82 CRED frames in this block came back RGB-only, and the read's refusal is the coherence
check, not a stride and not a mode.** The three things §1.8 asks to be told apart are separated by
the artefacts themselves:

- **Not `PlaneTooSmall`/`UnmodelledMode`.** The capture log carries no stride refusal for this part
  — the plan *accepted* the screen (320 wide, 200 rows, `scan_len` 0x50 → 160 bytes to a plane row,
  32000 bytes, under one plane's 65,536), and the mode is a 256-colour one (the loud
  "not one palette index a byte" refusal belongs to ENDSCRL and DDSTARS, not here).
- **Not the read being late on a moving screen.** `out/b5-s5-cred-map`'s warnings are all
  `indexed capture does not reproduce the RGB frame (the screen moved while it was read)`, which is
  the tool's wording for the coherence check — but this screen *is not moving* at the indices
  measured: three consecutive frames are byte-identical everywhere a keyframe is, and a hold is
  200 frames by the source's own loop (`for(a=0;a<200 && !dis_exit();a++) dis_waitb();`, `MAIN.C:194`)
  — at least the 101 frames between two identical samples 100 apart — against a read that costs ~20
  (§1.3). A still screen read late still matches, so the lateness rule is not what refused it.
- **Not the palette.** Each screen's palette is written once, by `tw_setpalette` at the top of
  `screenin` (`MAIN.C:169`), and the holds write nothing at all — so the DAC does not move under the
  read either.

What is left is the read's model of **where the screen is**, and the strongest candidate is the
**display start address**. `capture_indexed`'s own doc comment says its read "starts at offset 0 of
each plane because that is where the demo's screens begin", and CRED's screens do not begin there:
`tw_opengraph` writes CRTC 0x13 = `0x5013` and every screen then calls
`tw_setstart(160*200 + yy/4)` — a nonzero start, moved a further ~80 bytes per iris step. The
stride is *not* the suspect: the tool takes it from the same CRTC 0x13 register the display advances
by, and the `scan_len * 2` rule is confirmed on four other parts (§1.6, §1.7). *One alternative is
not excluded by the artefacts and is recorded rather than dismissed*: the same rule producing 160
for a screen whose guest pixel writer steps its rows by **80** bytes (`tw_putpixel`'s
`shl ax,4 / add bx,ax / shl ax,2 / add bx,ax` = `y*80`) would also break the check — i.e. the two
candidate mechanisms are "the read ignores the start" and "the read's stride is right for the
display but not for the pen", and the log cannot separate them. What keeps the second from being
read as *proof* of the first is worth stating: the tree does hold two `.idx` from a part that writes
CRTC 0x0c every frame (U2E's opening hold at 18000 and 18100, `out/b4b-s3-u2e2`, §18.6), but those
are **two-colour flat screens** that pass the coherence check whatever plane is read — the §1.6
blind spot — so they carry no weight here either way. **Nothing in the tree is wrong
either way**: no `.idx` was produced by CRED, and the row and Phase 2 use the RGB path, where the
6→8-bit widening `(c*259+33)>>6` is load-bearing (as ALKU's and TECHNO's rows are).

### 20.3 The re-capture and the census

```bash
# the reference: keyframes only
cargo xtask reference capture --arg 5 --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames 1440,3240,5440,7640,8740 --out reference/parts/cred --sheet out/cred.png

# the partners: a support frame each side of every keyframe
cargo xtask reference capture --arg 5 --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames 1439,1440,1441,3239,3240,3241,5439,5440,5441,7639,7640,7641,8739,8740,8741 \
    --out out/b5-cred-sess1        # and -sess2, -sess3
```

```
$ cargo xtask compare --reference reference/parts/cred --rendered out/b5-cred-sess1 \
      --out out/b5-cred-cmp1
frame 1440: 0.0000% differing (0/64000 px), max delta 0
frame 3240: 0.0000% differing (0/64000 px), max delta 0
frame 5440: 0.0000% differing (0/64000 px), max delta 0
frame 7640: 0.0000% differing (0/64000 px), max delta 0
frame 8740: 0.0000% differing (0/64000 px), max delta 0
verdict: identical, mode: rgb, offsets {"0": 5}
exit: 0
```

**`out/b5-cred-sess2` and `-sess3` give the identical report** — `exit 0`, `verdict: identical`,
0.0000 %, offsets `{0: 5}`, `mode: rgb` on all three.

**The census**, by §1.8's rule, over every session in the tree that holds a window — the three
census sessions plus the fix round's own `out/b5rev-cred-sess1` (a `SECOND 5` capture of the same
recipe whose frames are byte-identical at every index). The scan covers all **319** directories under
`out/` and `reference/parts/` that hold frames, the reference's own directory excluded, and the count
is filtered by recipe as §1.11 records:

| keyframe | sessions with a window | pictures at `N` | largest support | filed reference is it? | ≥ half the sessions? |
|---|---|---|---|---|---|
| CRED 1440 (held) | 4 | 1 | `f1ea2ca5…` **4/4 (100 %)** | **yes** | yes |
| CRED 3240 (held) | 4 | 1 | `98de65f3…` **4/4 (100 %)** | **yes** | yes |
| CRED 5440 (held) | 4 | 1 | `cc2df71e…` **4/4 (100 %)** | **yes** | yes |
| CRED 7640 (held) | 4 | 1 | `92155573…` **4/4 (100 %)** | **yes** | yes |
| CRED 8740 (held) | 4 | 1 | `cd5f27a5…` **4/4 (100 %)** | **yes** | yes |

Every session's window holds the filed picture at `K-1`, `K` *and* `K+1`, so the count is 4/4 at
offset 0 with no ambiguity to weigh.

**What Phase 2 will see.** Five settled targets, compared as RGB; a port that draws these screens
will diff at 0.0000 %. The residual to expect is at the *iris* frames, not at these keyframes, and it
is the panning/split arithmetic (`tw_setstart`/`tw_setsplit` with the AC pan write) that a port has
to reproduce there — the reference says nothing about the 590 frames a screen spends opening and
closing, only about the 200 it spends still.

---

## 21. ENDSCRL

Part 20, the last part of the sequence and `SECOND 5`'s third (`exe23`, `MAIN/U2.ASM:914-917`). The
source is `ENDSCRL/MAIN.C` (125 lines) over `ENDSCRL/FONA.LBM` and the text file `ENDSCROL.TXT`.

**It is skipped when looping is on** — `cmp cs:looping,0 / jne @@skipp6` (`MAIN/U2.ASM:914-915`) —
and **this capture pins the non-looping case**: the setup menu's looping toggle starts at
`m_looping = 0` (`MAIN/MENU.C:30`) and its own wording is "Demo looping: Disabled" at 0
(`MENU.C:233`), the recipe's keys (`KBD_left KBD_enter`) choose the sound card and leave the menu
without touching that toggle, and the effect is on screen at 8950 — a running ENDSCRL is itself the
proof that `looping == 0` in this session. (`/gravis` would also skip it while *not* looping, per
part-sequence §7; this recipe passes no switch.)

**What it draws.** `MAIN.C:22-26` sets **mode 0eh (640×350, 16 colours)** and then writes **CRTC 9 =
0** — the 400-line display — and sets up a 16-entry grey ramp. `do_scroll` (`:60-96`) renders one
**80-byte** line of the font strip into four bit-planes (`outport(0x3c4, 0x0102)` … `0x0802` — one
plane at a time) at `80*yscrl` and again at `80*(yscrl+401)`, and then moves the display start:
`setstart(yscrl*80+80)`, `yscrl=(yscrl+1)%401` (`:93-95`). So the text is a 401-line ring, one line
scrolled per iteration, and the loop waits two vblanks an iteration (`if(dis_waitb()==1)
dis_waitb()`, `:47`).

**Its extent is unbounded, measured.** It begins at the geometry change in **(8840, 8940]** and it
does not end on its own: the loop is `while(!dis_exit())` (`:45-49`) and `dis_exit()` is the DIS
server's `exitflag`, which only a key press sets (`DIS/DISINT.ASM`'s `exit_2`, `checkkeys`) — nothing
in the part, and no music position, stops it. Captured to 20000 and still scrolling — the sweep's
last three tiles are further blocks of the same text. **So this reference is a window into a part
that runs until the user stops it**, and the row says so rather than implying an end.

### 21.1 The sweep, and what the sheet showed

```
out/b5-s5-sweep5  8950,9100,9300,9500,9700,10000,10500,11000,12000,13000,14000,16000,18000,20000
                  exit 0 (14 frames), every one RGB only
```

| index | what is on screen |
|---|---|
| 8950 | the scroller's first frames: 4 colours, mean 0.81 — the top of the greetings text. |
| 9100 | "Second Reality / Copyright (C) 1993 by the Future Crew" entering at the foot of the frame. |
| 9300, 9500 | the same two lines climbing, with the Assembly'93 paragraph entering. |
| 9700, 10000 | "This demo won the Assembly'93 annual international demo competition held in Finland, summer 1993." plus the sponsors list. |
| 10500, 11000 | the sponsors (Advanced Gravis, Epic MegaGames, Waite Group Press, Terton, HiCompu, Toptronics, Pro Component, Lan Vision, Data Fellows) climbing off the top. |
| 12000 | the "Future Crew greets the following groups" block: Sonic PC, Access Denied, Silents PC, Avalanche, Xography, Witan, Triton, Epical. |
| 13000, 14000 | the personal greetings: "The members of Future Crew send their personal greetings to:" and the names (Daryll, Tran & White Shadow, Dr.Weird, Martti Palonen, Jussi Markula, Mikko Vierula, Jussi Leppänen, Wildfire …). |
| 16000, 18000, 20000 | still scrolling (the sheet's last three tiles are further blocks of the same text). |

**The scroll is quantised in pairs of frames**, not in single frames: `do_scroll` runs once per two
vblanks (`MAIN.C:47`), so the picture on screen at `K` is usually also the picture at `K+1` — the
triples below measure it rather than the source's wording.

### 21.2 The keyframes, and why these

| index | what is on screen | frame files | `sha256` |
|---|---|---|---|
| 9100 | "**Second Reality / Copyright (C) 1993 by the Future Crew**" entering at the foot of the frame (content rows 309–376 of 400) | `.rgb` only | `b6beb9f0fbc9…` |
| 10000 | the **Assembly'93 competition** paragraph: "This demo won the Assembly'93 annual international demo competition held in Finland, summer 1993." and the sponsors opening ("As the PC organizers of Assembly'93, we would like to thank the following companies for sponsoring the event: Advanced Gravis / Canada / Epic MegaGames / USA / Waite Group Press …") | `.rgb` only | `26d55ae1cd1b…` |
| 12000 | the **greetings groups** block: "read the FCINFO10.TXT file first. …" and "Future Crew greets the following groups:" with Sonic PC, Access Denied, Silents PC, Avalanche, Xography, Witan, Triton, Epical | `.rgb` only | `c308d1b20f97…` |
| 14000 | the **personal greetings** block: "Lord Sylis / Access Denied", "Daryll / Sun Projects", "Tran & White Shadow", "Dr.Weird / DanTe", Martti Palonen, Jussi Markula, Mikko Vierula, Jussi Leppänen, "Wildfire:", "Emphii, Mace & Shaft" | `.rgb` only | `fb2d67a3c3da…` |

One per text block, 1000–2000 frames apart — the part has no holds at all, so these are chosen for
what is on them rather than for stillness. The text is drawn centred on x = 320 of the 640-wide
frame (`tstart=(639-tstart)/2`, `MAIN.C:73`); measured, 9100's content sits in rows 309–376, 12000's
spans rows 4–396 with x 31–605, and 14000's rows 0–392 with x 123–513.

Full hashes of the four files in `reference/parts/endscrl/`:

| index | file | `sha256` |
|---|---|---|
| 9100 | `frame_009100.rgb` | `b6beb9f0fbc9c4c3a0fe752e9a40e8289bbe2140429670dd531cda56f789cd50` |
| 10000 | `frame_010000.rgb` | `26d55ae1cd1b3164e21ad75240fee89a3cfaec70b914acd7574e8053109394d7` |
| 12000 | `frame_012000.rgb` | `c308d1b20f97798b2fe669706d130b313741bb35eb2efcdbe88a7d375d4d22ea` |
| 14000 | `frame_014000.rgb` | `fb2d67a3c3da89ba6dfb31d768afa544ec66d3deee8c16ec6da127419ede7b6b` |


### 21.3 Held or moving, measured — and the window that could not be taken

Every keyframe is **moving**: `do_scroll` runs once per two vblanks (`MAIN.C:47`), so the picture
advances a line at a time and every neighbour is a different picture. Measured in `out/b5-es2-sess1`
at the three-frame gaps the sessions were captured with (§21.4):

| keyframe | `K-3` | `K` | `K+3` | `K-3`→`K` | `K`→`K+3` |
|---|---|---|---|---|---|
| 9100 | `06526561…` | `b6beb9f0…` | `f73dbb06…` | 3074 px (1.2008 %), 38 rows | 3901 px (1.5238 %), 40 rows |
| 10000 | `6ed5877e…` | `26d55ae1…` | `4aad4318…` | 14711 px (5.7465 %), 183 rows | 18866 px (7.3695 %), 199 rows |
| 12000 | `22de4c9e…` | `c308d1b2…` | `828bc0d3…` | 11546 px (4.5102 %), 197 rows | 14807 px (5.7840 %), 212 rows |
| 14000 | `4661302d…` | `fb2d67a3…` | `db758053…` | 8661 px (3.3832 %), 156 rows | 11078 px (4.3273 %), 167 rows |

The differences are confined to the rows the text occupies, which is what a one-line scroll step
looks like: at 9100 only rows 309–377 differ, and how many *pixels* that is depends on how much ink
the line carries — 3074 px for the two-line header, 14711 px for the sponsor block.

**There is no held moment in this part, and there is no `±1` window either — not because the screen
lacks one, but because the tool cannot take it here.** Measured: `--frames 9099,9100,9101,…` is
refused at the first batch with `the emulator reached frame 9101 before frame 9100 could be captured`
— **five sessions record that refusal** (three with nothing else running, two with the core-sharing
remedy of §1.11 in place, one of those naming `9102`) and a sixth attempt was killed before it could
write one; and a two-index list, `9099,9100`, is refused the same way (`out/b5-es-test2`, and again
as `b5-es-test3` with the emulator alone pinned to a starved core). §1.11 records the two remedies
that were tried and why both failed. **What the sessions were captured with instead is a three-frame
gap** — `9097,9100,9103` and the same around each keyframe — which every attempt to take it succeeded
with (`out/b5-es-test5` for the probe, and the three sessions of §21.4, twelve indices each); a
two-frame gap captures once (`out/b5-es-test4`, 9098 and 9100) but not in a three-index batch
(refused at `9102`).

### 21.4 The re-capture and the census

```bash
# the reference: keyframes only
cargo xtask reference capture --arg 5 --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames 9100,10000,12000,14000 --out reference/parts/endscrl --sheet out/endscrl.png

# the sessions: each keyframe and its three-frame neighbours (the widest gap this screen takes)
cargo xtask reference capture --arg 5 --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames 9097,9100,9103,9997,10000,10003,11997,12000,12003,13997,14000,14003 \
    --out out/b5-es2-sess1        # and -sess2, -sess3
```

```
$ cargo xtask compare --reference reference/parts/endscrl --rendered out/b5-es2-sess1 \
      --out out/b5-es2-cmp1
frame 9100: 0.0000% differing (0/256000 px), max delta 0
frame 10000: 0.0000% differing (0/256000 px), max delta 0
frame 12000: 0.0000% differing (0/256000 px), max delta 0
frame 14000: 0.0000% differing (0/256000 px), max delta 0
verdict: identical, mode: rgb, offsets {"0": 4}
exit: 0
```

**`out/b5-es2-sess2` and `-sess3` give the identical report** — three sessions, each run separately,
`exit 0`, `verdict: identical`, 0.0000 %, `mode: rgb`, offsets `{0: 4}`.

**The census, and its one deviation from §1.8's rule.** The scan finds **no session in the tree that
holds this part's `N-1, N` and `N+1`** — the count is 0 at every keyframe, for the measured reason
above — so the count below is an **offset-0 count**: sessions of the recipe that took the keyframe's
own index, and whether their frame is byte-identical to the filed picture.

Every same-recipe directory in the tree that took the index is counted, not only the ones captured
*for* the census — the discovery captures and the fix round's re-runs hold these indices too, and
their frames are byte-identical:

| keyframe | sessions holding `N` | byte-identical to the filed picture at `N` | ≥ half the sessions? |
|---|---|---|---|
| ENDSCRL 9100 | **9** — `-es2-sess1`, `-es2-sess2`, `-es2-sess3`, `-es-test5`, `-es-test4`, `-es-diag`, `-s5-sweep5`, `b5rev-es-keys`, `b5rev-es-gap2` | **9/9 (100 %)** | yes |
| ENDSCRL 10000 | **5** — `-es2-sess1`–`-sess3`, `-s5-sweep5`, `b5rev-es-keys` | **5/5 (100 %)** | yes |
| ENDSCRL 12000 | **5** — the same five | **5/5 (100 %)** | yes |
| ENDSCRL 14000 | **5** — the same five | **5/5 (100 %)** | yes |

Those extra sessions are worth naming because they were not run for the census: `out/b5-es-test4` and
`out/b5-es-test5` are the probes that established the three-frame-gap remedy, `out/b5-es-diag` is the
single-index diagnostic run, `out/b5-s5-sweep5` is the discovery sweep, and `out/b5rev-es-keys` and
`out/b5rev-es-gap2` are the fix round's own runs. All seven are of the recipe (`SECOND 5`, `autoexec`,
30000 cycles, these keys) and all seven hold the filed picture's bytes at the index. The same scan
also shows why the recipe filter matters even here: `out/b4b-ray-sess1`–`-sess3`, `out/b4b-ray-sweep1`
and `reference/parts/rayscrl` — `SECOND 3` captures of a 320×200 screen — hold index 14000, and
`out/b5-su-sweep2`, the hidden part's own sweep, holds 10000 through 14000.

**What the count is and what it is not.** It is not a support count over a window, and the row says
so. §1.8 rule 1 warns that a single-index probe "can answer only offset 0 and inflate apparent
disagreement" — and that warning is about sessions that **differ** at `N`. Here **every session that
took the index is byte-identical to the filed picture at offset 0**, so no session needs a `±1`
answer and there is no disagreement for the rule to inflate. What the window would have covered is a
session one frame out of phase: such a session's frame at `N` would show the neighbouring line of
text (measured, 1.2–7.4 % of pixels at a three-frame step) and would be reported as a difference
rather than as an alignment. **Phase 2 should read a diff at these keyframes in that light**: the
reference is the picture these sessions present at exactly these boot indices, and this part has no
alignment slack for a port to hide behind.

**What Phase 2 will see.** Four moving targets, RGB-only, each reproduced byte-for-byte by three
sessions; a port that draws ENDSCRL's 401-line ring with the same two-vblank step will diff at
0.0000 %. The mode is 16-colour, so the comparison is RGB and the 6→8-bit widening is load-bearing,
and the *text* is the part's own `ENDSCROL.TXT` through `FONA.LBM`'s strip — a port that reads those
two assets in their original formats has everything the picture needs.

### 21.5 Why there is no `.idx`

All 14 sweep frames and the four keyframes carry the tool's **loud** refusal, *"the screen is not one
palette index a byte (not 256-colour, or a depth the emulator did not name)"* — the `UnmodelledMode`
case, not the moving-screen warning and not a stride refusal: the read never happens. The reason is
the part's own mode: mode 0eh is a 16-colour screen, and its pixels are packed as **bits**, not one
index a byte (`MAIN.C:22-24` sets it and `:25-26` writes CRTC 9 = 0 for the 400-line display). That
is the same shape as TECHNO's interference screen and DDSTARS's (§1.7, §22.5), and it is a different
cause from CRED's (§20.2): ENDSCRL does also have a nonzero display start (`setstart(yscrl*80+80)`,
`MAIN.C:95`) and 80-byte rows, but the mode refusal comes first and nothing about the start address
arises.

---

## 22. DDSTARS — the hidden part

Slot 17, and **not in the main sequence at all**: it is reachable only by `SECOND U`, which sets
`whattorun = 0x80` (`partsmask[8]` = the literal `80h`, `MAIN/U2.ASM:119`) — bit 7 alone — and the
runner then runs `exehid` by itself after a `restartmus` of MUSIC0 at order 70 (`:890-896`). No
numbered boot sets bit 7 (`0x1f`, `0x1e`, `0x1c`, `0x18`, `0x10` and the default `7fh` all have it
clear) and the end-of-pass reset writes `07fh` (`:919`), so **this reference is the only way the part
can be reached** — part-sequence §5. Its source is `DDSTARS/KOE.ASM` (99 lines) over `STARS.ASM`
(803) and `POLYEGA.ASM` (421); `DOIT1.C`/`DOIT2.C`/`DOIT3.C` are the authors' generators for the EGA
art (`pic.ega`, `pic2.ega`), not part of the effect.

**What it draws.** A 3-D starfield: `init_stars` (`STARS.ASM:56-178`) sets mode 13h, clears the
attribute controller's palette to 0..15, **clears CRTC 9's scan-doubling bit** (`mov al,9 / out dx,al
/ inc dx / in al,dx / and al,127 / out dx,al`, `:72-78`) and loads the star tables; `do_stars`
(`:540-778`) is an endless loop of `waitb`, a ~32-frame palette fade-in (`starpalfade`, `:559-648`),
`risetext` (the picture/text band, `:413-540`) and the star drawing, with **text opened when its own
`starframe` counter passes 1200 and again at 3200** (`:653-662`) and a page flip through CRTC 0x0c
between the two 64 KB halves (`:544-553`).

**Its screen is 320×400, and that is the part's own doing**, not the loader's: clearing CRTC 9's bit
7 takes mode 13h's 200 lines to 400, in mode 13h's own 80-byte rows (it never writes CRTC 0x13). The
emulator nonetheless names it a **16-colour** screen. `vga.gfx.mode & 0x40` is what the emulator's
`VGA_DetermineMode` needs to call a screen `M_VGA` (`hardware/video/vga.cpp:74`), and mode 13h's own
mode set is not the last word on that register once a part writes the graphics controller — which
this one does (`mov ax,0ff08h`, the bit-mask register, `POLYEGA.ASM:322`; sequencer plane masks
throughout `STARS.ASM`). The indexed read is therefore refused with `not one palette index a
byte`, **20 of 20 frames in this block**, and nothing is read at a guessed stride.

**Its extent is unbounded, measured.** It starts in the boot's first frames — a probe at 415, 450
and 480 finds the screen already up (4 colours, mean 0.00 → 0.08 → 0.11: the palette fade-in's first
steps, `starpalfade`), and 500 is mean 0.13 — and **never ends on its own**: `do_stars`'s loop
exits only when DIS function 2 returns non-zero (`mov bx,2 / int 0fch / or ax,ax / jz @@aga`,
`:774-778`), which is `exitflag` — set by a key press and by nothing else (`DIS/DISINT.ASM`). A sweep
to 8000 and a second to 20000 both show the same effect still running.

### 22.1 The sweeps, and what the sheets showed

```
out/b5-su-sweep1  500,700,900,1100,1300,1500,1700,1900,2100,2400,2700,3000,3300,3600,4000,4500,
                  5000,6000,7000,8000        exit 0 (20 frames), every one RGB only
out/b5-su-sweep2  10000,12000,14000,16000,20000    exit 0
```

| index | what is on screen |
|---|---|
| 500 | nearly black, 4 colours, mean 0.13 — the starfield's first fade steps. |
| 700, 900, 1100, 1300, 1500 | a **sparse starfield**, one or two pixels brighter every sample (mean 0.52 → 2.47). |
| 1700, 1900 | the same field, with the **first line of text entering at the bottom**: "A hidden part? No, just an experiment …" |
| 2100, 2400 | the text block ("A hidden part? No, just an experiment we left out. …") fully written in the upper middle. |
| 2700, 3000 | the text climbing, with the part's own "Future Crew 1993" artwork faintly visible behind the stars. |
| 3300, 3600 | the text scrolled off; a denser field. |
| 4000, 4500, 5000 | a second text block ("And then we have …") over a denser field. |
| 6000, 7000, 8000 | the field still densifying (mean 44.68 → 57.24), the text faded. |
| 10000 … 20000 | the same effect still running: 4 colours, mean 67.37, 75.15, 81.17, 85.94 and 92.67 at those five samples, a different `sha256` at every one — the field still densifying 12000 frames after the last keyframe. |

### 22.2 The keyframes, and why these

| index | what is on screen | frame files | `sha256` |
|---|---|---|---|
| 1900 | the **first text entering** the field | `.rgb` only | `c9228e9f21c4…` |
| 3000 | the text block **fully open** | `.rgb` only | `b0693ed8a259…` |
| 5000 | a **second text block** over a denser field | `.rgb` only | `d948981ec330…` |
| 8000 | the **late field**, text faded | `.rgb` only | `ecc2dadd1e0d…` |

1000–3000 frames apart, one per stage of the effect. The part has **no held frames to choose
instead**: every keyframe's `K-1`, `K` and `K+1` are three different pictures (measured below), and
the field's density only ever increases.

Full hashes of the four files in `reference/parts/ddstars/`:

| index | file | `sha256` |
|---|---|---|
| 1900 | `frame_001900.rgb` | `c9228e9f21c40c66f370cf60cbd0c79edd6f7a14aab0fdbe7257f42f5765f70a` |
| 3000 | `frame_003000.rgb` | `b0693ed8a259f983dde3f3dd57ecc3bade93fcb0702368679631404fa7bb04a1` |
| 5000 | `frame_005000.rgb` | `d948981ec3304418fb5cd25033f4c3930785f807f04645a0b0242a8de6ecdbf5` |
| 8000 | `frame_008000.rgb` | `ecc2dadd1e0df5984ba0adf4efcdfe45d6473d656e59632574296cf2dc408df4` |

### 22.3 Held or moving, measured

Every keyframe is **moving**, and the triples are three distinct pictures at every index (measured
in `out/b5-su-fine1` and again in all three partner sessions):

| keyframe | `K-1` | `K` | `K+1` | reading |
|---|---|---|---|---|
| 1900 | `de6d1676…` | `c9228e9f…` | `a1697853…` | **moving** |
| 3000 | `f768ffbb…` | `b0693ed8…` | `5eb100a6…` | **moving** |
| 5000 | `062d91c4…` | `d948981e…` | `0af0a960…` | **moving** |
| 8000 | `fb4b902c…` | `ecc2dadd…` | `328201f6…` | **moving** |

### 22.4 The re-capture and the census

```bash
# the reference: keyframes only
cargo xtask reference capture --arg U --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames 1900,3000,5000,8000 --out reference/parts/ddstars --sheet out/ddstars.png

# the partners: a support frame each side of every keyframe
cargo xtask reference capture --arg U --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames 1899,1900,1901,2999,3000,3001,4999,5000,5001,7999,8000,8001 --out out/b5-ddstars-sess1
    # and -sess2, -sess3b (a third session, -sess3, was refused at 5001 and holds only two windows)
```

```
$ cargo xtask compare --reference reference/parts/ddstars --rendered out/b5-ddstars-sess1 \
      --out out/b5-ddstars-cmp1
frame 1900: 0.0000% differing (0/128000 px), max delta 0
frame 3000: 0.0000% differing (0/128000 px), max delta 0
frame 5000: 0.0000% differing (0/128000 px), max delta 0
frame 8000: 0.0000% differing (0/128000 px), max delta 0
verdict: identical, mode: rgb, offsets {"0": 4}
exit: 0
```

```
$ cargo xtask compare --reference reference/parts/ddstars --rendered out/b5-ddstars-sess2 \
      --out out/b5-ddstars-cmp2                    # and -sess3b; all three identical
```

**The census**, by §1.8's rule, **per keyframe** and over every session in the tree that holds that
keyframe's window — the three full census sessions, the refused `out/b5-ddstars-sess3`, the earlier
probe `out/b5-su-fine1` and the fix round's own `out/b5rev-dd-sess1` (all of the same recipe, all
byte-identical at every index). Filtered by recipe, §1.11; the reference's own directory excluded;
`out/b5-ddstars-sess3` is exactly the case rule 2 is about — its run was refused at 5001, so it
counts at 1900 and 3000 and not at 5000 or 8000:

| keyframe | sessions with a window | pictures at `N` | largest support | filed reference is it? | ≥ half the sessions? |
|---|---|---|---|---|---|
| DDSTARS 1900 | **6** | 1 | `c9228e9f…` **6/6 (100 %)** | **yes** | yes |
| DDSTARS 3000 | **5** | 1 | `b0693ed8…` **5/5 (100 %)** | **yes** | yes |
| DDSTARS 5000 | **4** | 1 | `d948981e…` **4/4 (100 %)** | **yes** | yes |
| DDSTARS 8000 | **5** | 1 | `ecc2dadd…` **5/5 (100 %)** | **yes** | yes |

Every window in every session holds the filed picture **at `N`** (offset 0), and the neighbours each
session keeps at `N±1` (`de6d1676…`/`a1697853…` at 1900, `f768ffbb…`/`5eb100a6…` at 3000,
`062d91c4…`/`0af0a960…` at 5000, `fb4b902c…`/`328201f6…` at 8000) are the *same* pictures in every
session — so this part is deterministic at its boot index too, moving screen and all, and **the file
needed no state census**: the support count only had to show the filing was taken at `N`.

**What Phase 2 will see.** Four settled, moving targets compared as RGB — 0.0000 % against all three
partner sessions. Because the screen is 320×400 and RGB-only, this row is also the first place the
port is measured on a 16-colour-named screen whose content is a 256-colour starfield: a port that
reproduces the part's *own* register sequence will render it through whatever its own colour path is,
and the comparison is on pixels, not on indices (§1.6's consequence for the rows).

### 22.5 Why there is no `.idx`

**The screen is not one palette index a byte, and the refusal says so**: all 20 frames of
`out/b5-su-sweep1` carry *"the screen is not one palette index a byte (not 256-colour, or a depth the
emulator did not name)"*, which is the loud `UnmodelledMode` refusal — the same one TECHNO's
interference screen produces. So this is **not** "the screen moved while it was read" (the warning
this block's captures would otherwise carry), and it is not a stride refusal either; the read never
happens. **What this row establishes and what it does not**: established is the emulator's own name
for the screen (16-colour, from the refusal) and where the decision is made (`vga.gfx.mode & 0x40`,
`hardware/video/vga.cpp:74`); *not* established is which write of DDSTARS's clears that bit — the
part writes the graphics controller's bit-mask register and the sequencer's plane masks, and a grep
of `KOE.ASM`/`STARS.ASM`/`POLYEGA.ASM` finds no write to the graphics controller's mode register
(index 5). A port has to reproduce the part's own register sequence either way, so the question is
Phase 2's, not this phase's; what this row pins is what the oracle shows.

**What Phase 2 will see.** Four settled, moving targets compared as RGB — 0.0000 % against three
separate sessions. Because the screen is 320×400 and RGB-only, this row is also the first place the
port is measured on a 16-colour-named screen whose content is a 256-colour starfield: a port that
reproduces the part's *own* mode state will render it through whatever the port's own colour path
is, and the comparison is on pixels, not on indices (§1.6's consequence for the rows).

---

---

## 23. What block 3 has that block 2 did not

of this block's four parts are **deterministic across sessions at every index measured** — MNTSCRL
(§11.3, seven indices over eight sessions), LNS&ZOOM (§12.3, five sessions) and MINVBALL (§14.3,
three sessions and a discovery run) all draw the same bytes at the same boot index every time, moving
screens included — and so is PLZPART's cube (§13.5, four sessions). Only PLZPART's plasma is not, and
that one is sub-frame rather than a phase (§13.6). Block 2's moving screens were the opposite case
for every keyframe it filed, which is what §1.8 was written for; the difference is the anchor: a part
whose timeline is pinned to one music cue draws the same frames at the same indices, and a part whose
*start* is a music wait with a *pinned* end (PLZPART's plasma) is the one case that drifts.
