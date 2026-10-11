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
