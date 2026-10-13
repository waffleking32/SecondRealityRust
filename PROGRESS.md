# Progress

Phase status board. **Read this first if you are resuming work.**

- `not started` — no work done
- `in progress` — active; see the phase plan
- `done` — exit criteria met, evidence recorded, `phase-N` tag applied
- `complete — untagged` — exit criteria met and evidence recorded, tag deliberately not yet applied

| # | Phase | State | Plan | Tag |
|---|---|---|---|---|
| 0 | Foundation & harness | **done** | [00-foundation.md](docs/plans/00-foundation.md) | `phase-0` |
| 1 | Assets & references | **done** | [01-assets.md](docs/plans/01-assets.md) | `phase-1` |
| 2 | Shell + easy tier | in progress — **BEGLOGO is frame-diff-identical (exit 0)**; TUNNELI ported and measured at the reference's own alignment (0.28–0.68 %, A6) | [02-shell.md](docs/plans/02-shell.md) | — |
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

**Phase 1 is complete** — tag `phase-1` at `a17b1ab` — **and `master` holds it.** Its exit criteria
are met (the evidence is in the section below and in `docs/fidelity/reference-indices.md`), its plan
is `docs/plans/01-assets.md`, and its ledger (the recovery map, git-ignored scratch) is
`.superpowers/sdd/01-assets/progress.md`. The tag was withheld until the final whole-branch review
had run and its findings were applied (two fix dispatches, recorded at the end of the evidence
section below), because a tag that has to be moved is worse than a tag applied a few commits later
(B6's ruling, in the ledger).

**The branches, as of 2026-10-13.** `phase-0-foundation` and `phase-1-assets` were merged into
`master` at the user's instruction. The history was linear (`master` → `phase-0-foundation` →
`phase-1-assets`, no divergence), so this was a single fast-forward: `master` *is* `a17b1ab`, no
merge commit exists, and `master`'s tree hash equals `phase-1^{tree}` (`f5e2151b`) — what is tagged
and what is on `master` are the same bytes. The two branches are kept rather than deleted. **Phase 2
starts from `master`.**

### Phase 1's evidence (close-out, Task B6, 2026-10-12)

**Part A (assets).** `sr-assets` decodes Origin PIC (five variants plus a headerless case), IFF (ILBM
and PBM), RIX3, `.PAL`, OMF `LEDATA`, FLI and `.ANI`, plus a proportional font-strip view. `assets/`
holds **150 assets plus `PROVENANCE.md`** (149 curated plus one recovered from an EXE), 151 files.
`cargo xtask assets verify --tree --decode`, re-run at the close-out, exits **0** with `verdict: ok`:

```
decode: 150 asset(s) — 55 picture(s), 2 animation(s), 5 palette(s), 87 headerless with no decoder,
1 refused on record, 0 finding(s)
```

21 contact sheets come from the same pass. Four approximation entries; the plan was corrected in place
as each task measured what it had guessed — the commits touching `docs/plans/01-assets.md` run from
`3f49511` (the Origin PIC model) to this fix round's `7c07c7c`, Part A's and Part B's interleaved —
and every correction came from a measurement rather than from reasoning. (The close-out
said "17 plan corrections"; no enumeration reproduces that number, so it is withdrawn rather than
restated.) The human-look evidence is the user's own reading of
the sheets ("images look fine"); the noise-like tiles are non-picture data (3D scene files, TASM
listings, S3M music, lookup tables, palette strips) correctly rendered as byte views, and none of
them carries a picture magic. **The workspace suite is 405 passed, 0 failed, 1 ignored, 0 warnings**
(398 at the close-out, re-run after the two fix dispatches — the code round's five tests and this
round's two; it was 361 at Part A's close).

**Part B (references) — all 20 slots captured**: **83 keyframe frames in 22 capture directories**
under `reference/parts/`, every one of them carrying a `manifest.json` with the recipe and a row in
`docs/fidelity/reference-indices.md` §2 with its command, hashes and re-capture comparison. (TECHNO
and PLZPART each span two geometries and have one directory per screen; U2E is captured from a
`SECOND 4` boot, which `SECOND 3` also reaches — the screen does not depend on the boot, only the
index.) Held/moving is measured per keyframe, not assumed from the part (§1.4).

| part | boot | keyframes | held / moving | reproducibility |
|---|---|---|---|---|
| BEGLOGO | `SECOND 2`, 320×400 Mode X | 430, 470, 560, 900 | 430 and 900 held; 470/560 moving — a palette fade over the held picture | **exit 0, identical, 0.0000 %, offsets `{0: 4}`**; 900 byte-identical in five sessions |
| ALKU | `SECOND 1`, 320×400, 176-byte rows, RGB only | 900, 1719, 2719, 5859 | 900/1719/2719 held; 5859 moving — the part's own 1 px-per-9-frames pan | exit 1, `mode: rgb`: three held frames 0.0000 % at offset 0; 5859 differs by a whole number of pan pixels (15.2797 % at offset 0, **0 px of 95,700 after the shift**). No tolerance added (§4.4) |
| U2A | `SECOND 1`, 320×200 | 6500, 7300, 7640 | 6500/7640 held; 7300 moving | exit 0, identical, 0.0000 %, offsets `{0: 3}` |
| PAM | `SECOND 1`, 320×200 | 7710, 7760, 7830, 7900 | 7830/7900 held; 7710/7760 moving | exit 0, identical, 0.0000 %, offsets `{0: 4}` |
| GLENZ | `SECOND 2`, 320×200, 92-byte plane row | 1499, 2300, 3000, 3400 | 1499 held (palette cycles); 2300/3000/3400 moving | exit 1, `mode: mixed`: 1499 identical at offset 0; the three moving frames are the largest-support pictures and are **byte-identical at offset 0** against the review's independent partner. **3000 and 3400: no settled content** (§1.8) |
| TUNNELI | `SECOND 2`, 320×200, RGB only | 3800, 4200, 4500, 4600 | all four moving | exit 1, `mode: rgb`: 3800/4200/4500 identical 0.0000 % at offset 0; 4600 0.0891 % (57 px). **4500 and 4600: no settled content** |
| TECHNO — interference | `SECOND 2`, 320×200 16-colour EGA, RGB only | 4900, 5000, 6300, 8100 | 4900/6300/8100 moving; 5000 a palette step that holds for one sampled pair and moves a frame away | **exit 0, identical, offsets `{0: 4}`** after the group-check re-capture (was 24.6–72.9 % before it) |
| TECHNO — troll | `SECOND 2`, 320×400 Mode X | 8200, 8300, 8400 | 8200/8400 held; 8300 moving (the fade-in) | exit 1, `mode: mixed`: 8200/8400 identical at offset 0; 8300 99.9883 % at offset −1, its picture the largest support (3/4) |
| PANICEND | `SECOND 2`, 320×200 | 8792, 8800, 9000 | 8800/9000 held; 8792 moving — the collapse itself | exit 0, identical, 0.0000 %, offsets `{0: 3}` |
| MNTSCRL | `SECOND 3`, 320×200 Mode 13h | 500, 1100, 1700, 2300 | 500 held; 1100/1700/2300 moving (2.4–3.1 % a frame) | exit 0, identical, offsets `{0: 4}`; the same against two more independent sessions (`-sess4`, `-sess6`); **6/6 (100 %)** support |
| LNS&ZOOM | `SECOND 3`, 320×200 Mode 13h | 2700, 3400, 4200, 4900, 5650 | 2700/5650 held; 3400/4200/4900 moving (6.7–39.8 % a frame) | exit 0, identical, offsets `{0: 5}`; **5/5 (100 %)** |
| PLZPART — plasma | `SECOND 3`, 320×400 Mode X, line-compare 60/61 | 6000, 7000, 7600 | all three moving (~70 % of pixels a frame) | exit 1, `mode: rgb`, offsets `{0: 3}`: 6000 0.0000 %/0.1641 % across two sessions; 7000 7.73/12.93 %; 7600 9.33/24.48 %. **No settled content on all three** (2/5, 0/4, 0/3) — §13.6 |
| PLZPART — cube | `SECOND 3`, 320×200 Mode X | 8500, 9500 | both moving (25–56 % a frame) | exit 0, identical, 0.0000 %, offsets `{0: 2}`; **4/4 (100 %)** |
| MINVBALL | `SECOND 3`, 320×200 Mode 13h, RGB only | 10900, 11200, 11800, 12400 | all four moving (4.7–7.1 % a frame) | exit 0, identical, offsets `{0: 4}` (one session pairs all four at offset −1, 0.0000 %); **3/3 (100 %)** |
| RAYSCRL | `SECOND 3`, 320×200 Mode 13h | 13000, 13500, 14000, 14500 | 13500 held; the rest moving | **exit 0, identical, 0.0000 %, offsets `{0: 4}`** against three sessions and the reviewer's; **4/4 (100 %)** — the cleanest row in the tree |
| 3DSINFLD | `SECOND 3`, 320×200, 80-byte plane rows, RGB only | 15500, 16000, 16500, 16800 | all four moving, in **two-frame steps** | **exit 0, identical, 0.0000 %** against two sessions; exit 1 against one two steps out of phase (38.90 %/41.79 %). Census 3/3, 3/3, 3/4, 2/3 — **all four settled** after the fix round re-filed 16500 and 16800 |
| JPLOGO | `SECOND 3`, 320×400 Mode X, RGB only | 17300, 17500, 17700 | all three held *within* a session, but the held bytes differ between sessions | exit 1, `mode: rgb`: 17300 0.0000 % in all four sessions; 17500 0.0000 % in two (2/4 = 50 %) and 39.4–41.4 % in the other two; 17700 40.19 % against two and 27.4–29.6 % against the others. **17700: settled as a state, unfiled as a file** — the state two sessions hold was not drawn by any of seven re-capture probes |
| U2E | `SECOND 4` (and `SECOND 3`, other indices), 320×200 | 1000, 1600, 2600, 3600 | all four moving; the buffer rotation is not a fixed period | **exit 0, identical, 0.0000 %, offsets `{0: 4}`** against the reviewer's independent session; exit 1 against the three census sessions, each missing one keyframe by 55.5–60.8 % (a phase group, not a different screen). Census 4/4, 3/4, 3/4, 3/4 — every keyframe settled |
| ENDLOGO | `SECOND 5`, 320×400 Mode X | 430, 500, 900, 1180 | 430/900 held; 500/1180 moving — palette fades over the held picture | **exit 0, identical, 0.0000 %, offsets `{0: 4}`** against three sessions; **4/4 (100 %)**. Only 900 is index-compared (no session holds a 430 `.idx`) |
| CRED | `SECOND 5`, 320×200, 160-byte plane rows | 1440, 3240, 5440, 7640, 8740 | all five held | **exit 0, identical, 0.0000 %, offsets `{0: 5}`** against three sessions; **4/4 (100 %)**; all 82 frames RGB-only — the *coherence* check, not a stride and not a mode (§20.2) |
| ENDSCRL | `SECOND 5`, **non-looping**, 640×400 mode 0eh 16-colour | 9100, 10000, 12000, 14000 | all four moving — the 401-line ring scrolls a line per two vblanks | **exit 0, identical, 0.0000 %, offsets `{0: 4}`** against three sessions. Its census is an **offset-0 count**, not a window count — see the caveat below: **9/9 at 9100, 5/5** at the rest |
| DDSTARS (the hidden part) | `SECOND U` only, 320×400 | 1900, 3000, 5000, 8000 | all four moving | **exit 0, identical, 0.0000 %, offsets `{0: 4}`** against three sessions; per-keyframe census **6/6, 5/5, 4/4, 5/5 (100 % each)** |

**Phase 1's exit criterion is met**: every slot of `part-sequence.md` §1 has a reference, a row and a
re-capture comparison, and the criterion is stated per screen kind because the oracle's own
reproducibility is per screen kind — a held screen shows exit 0, offset 0, byte-identical; a moving
one shows the measured offset and residue.

**The step-1 probe (B6) — the day-apart claim, which the in-session checks cannot reach.** The phase
did span days: the first captures were committed 2026-10-11 and the probe ran 2026-10-12. B6
re-captured the oldest references in fresh emulator sessions and compared them against the filed
files. **BEGLOGO** (the first reference captured; its 470 and 560 move in the palette): all four
keyframes **0.0000 %, `verdict: identical`, exit 0, offsets `{0: 4}`**, and all six filed files
(`430.rgb`, `430.idx`, `470.rgb`, `560.rgb`, `900.rgb`, `900.idx`) **`sha256`-equal** to the new
capture. **ALKU** (the oldest continuously drawn screen, and the one row whose moving keyframe does
not reproduce pixel-for-pixel): all four keyframes **identical at offset 0, exit 0**, including 5859
— the new session drew the reference's own pan phase, and its 5858/5859/5860 are one `sha256` again.
So a capture still means the same thing a day later for both. This is the weaker claim the plan
wanted probed and it is now measured, not assumed; what it does **not** cover is a screen outside
these two kinds (a continuously redrawn screen that lands in a minority alignment group), which is
what §1.8's census exists for.

**What could not be confirmed — the negatives, which a resuming session must read as carefully as
the table.** `docs/fidelity/reference-indices.md` §1.12 is the policy and carries the detail.

- **Keyframes the filed reference does not fully settle** — the largest-support picture is presented
  by fewer than half the sessions, so the row records **no settled content**: GLENZ 3000 and 3400,
  TUNNELI 4500 and 4600, PLZPART plasma 6000, 7000 and 7600, and JPLOGO 17700 (*settled as a state,
  unfiled as a file* — the filed picture is taken at `N` by none of the four sessions, and seven
  probes never drew the state two of them hold). **A Phase 2 diff at these indices is the oracle's
  residual, not a port error.** Two knife edges sit just outside the list at exactly half: TUNNELI
  4200 (3/6) and JPLOGO 17500 (2/4).
- **The oracle's rare excursion, unexplained.** One session had TECHNO's rings ~30–40 frames early.
  It did not reproduce, and it is not the music clock (measured to ≈1 music frame across sessions),
  not a fixed offset and not a counter stall (0 stalls ≥150 ms at 20 ms resolution to frame 4900).
  Recorded as real, rare, not reproducible.
- **ENDSCRL's deviation from the window rule, and what it costs.** No session in the tree holds this
  part's ±1 window — the tool cannot take two frames one index apart on that 640×400 screen (five
  sessions record the refusal and a sixth attempt was killed; two remedies tried and both failed,
  because the grabs are served by the emulator's own process), so its census is an offset-0 count. Accepted *because* every session that took the index
  is byte-identical at offset 0, so there is no disagreement for the rule to inflate — but a
  one-frame-out diff there **is a difference, not an alignment**, and the offset-0 agreement proves
  nothing about a ±1 pairing, because no session holds one.
- **The CRC-register read race in `xtask`'s oracle, still live.** `crtc_offset_register()` and
  `chain4_enabled()` each read the index/data port pair in two requests with the guest running in
  between; the loud half refuses a frame (JPLOGO: eight refusals), the quiet half can be *used* and
  the coherence check is blind to it on a palette-collapsing screen. Nothing in the tree is wrong
  because of it — every `.idx` from a part that writes CRTC registers was hand-checked — but the
  durable fix (read the pair coherently, or twice and require agreement) belongs to whoever next
  touches the oracle.
- **No `phase-1` tag, deliberately.** See the top of this section.

**Carried into Phase 2 — make the reference record machine-readable, then verify it.** The reference
tree's bytes are pinned by `docs/fidelity/reference-indices.md` alone: no test reads
`reference/parts/`, and the natural self-check (`compare` of the tree against itself) can catch a
*missing or truncated* file but never a byte changed in place. A `xtask reference verify` was
deliberately not built at the close-out, and the reason is a measurement: the hash check is **not
machine-addressable against the document** — 90 rows carry a full digest in two naming shapes
(77 bare, 13 path-prefixed), they name 78 distinct files of which 10 carry two or three different
digests, and 24 of the tree's 105 `frame_*` files have no row by name at all (their digests sit in
per-part tables whose directory is named only in the prose). A verifier resting on that would fail
on 24 correctly-filed files today and would stop checking whenever a sentence is reworded. The fix is
to make the record machine-readable **where it is written**: `reference capture` should put a
`sha256` into each frame entry of the manifest it writes (the manifest already has a `version` field
for exactly this kind of change), after which `xtask reference verify` can walk the tree with
`compare`'s all-the-way-down rule, fail loudly on a file the manifest does not name and on a named
file that is missing, and compare every digest against the manifest rather than against prose. Until
then, a byte changed in place is the one failure nothing in the tree reports.

**The deferred minors, triaged by the whole-branch review (41 items).** **Eleven were "action now"**
and are applied in the final fix round: the `origin.rs` sweep comment that claimed cuts the loop
does not make and the `anim.rs` bias comment whose alphabet never reaches the dispatch it names (both
now say what they do), the §E.18 nine-byte record now **pinned by a test** (`tests/omf.rs`, because
Phase 2's ALKU work is its consumer), `font.rs`'s 15,000-byte padding figure and `glyph`'s garbled
doc sentence, a new `tests/font.rs` case for a dividing width that is not the routine's, the
`ANIM.C`/`U2A.C` citation slips, `palette.rs`'s five-file claim, `reference.rs`'s geometry-guard doc,
the decode summary that did not sum (the code dispatch) and the music counter's address and rate —
`0x105B0E`, 0.7424/frame — copied out of the git-ignored ledger into `reference-indices.md` §1.7 and
D20 before the ledger goes. **Fifteen items are "later"**, to fix when the file is next touched: A1's §E
citations and two wording slips; A3's `Unsupported` wording for malformed input, its duplicated chunk
walk, its unexercised `Ok` arm and its "nearly all" comment; A4's `pal.rs:6-10` argument and
`tests/rix.rs`'s "depend on"; A5's three count/range slips; A7's absent `APPROXIMATIONS.md` entry for
ALKU's 32-vs-30 rows (**Phase 2 owns it** — its durable home must exist before the Phase 2 plan is
written) and `lib.rs:94`'s over-long sentence; A8's present-but-unreadable provenance fixture and its
empty-frame advice; B1's "17 keypad keys" and D17's loose referent. **Seven were dropped** as
working-artifact prose or unreachable/labelled cases, and **one was already stale**: A1's ":188's
'the only copy…' is untrue" had been removed by A5 (`df6610e`) and is struck from the list rather
than carried as live.

**The whole-branch review and its fix round.** The review (2026-10-12, on `da3d553`) found no
Critical and confirmed the phase finished with its exit criteria met, re-deriving every load-bearing
number itself — including three captures re-taken byte-identically in fresh sessions. Its findings
were applied in two dispatches: the code half (`acfe436`: `compare` walks a capture tree all the way
down and prints `captures: N measured` beside the verdict, the `--out`/`--sheet` guard collapses `..`
lexically, the decode summary adds up, and two latent width bounds are documented) and the
documentation half (the support-frame rule in `CLAUDE.md`/D19/the plan's criterion, the stale numbers
in D20/§1.12/§1.10, D16's now-closed split caveat, the TUNNELI→TECHNO boundary, `docs/fidelity/bugs.md`
written at last, the spec's display-mode table annotated, and the eleven minors above). Nothing
reopened a captured frame, a decoded byte or a claim the phase rests on, and no emulator run was
needed. **The `phase-1` tag is still the last act and is not applied here.**

**One Phase 2 result already came out of Part B.** PLZPART's plasma confirms on captured frames that the
port's `Split::scanline` is the emulator's line-compare register **plus one** (`62/61/62` for a register
alternating 60/61), which is what `Split`'s doc comment and D16 assert and what Phase 2 depends on.

**The policy a later phase should read first** is `docs/fidelity/reference-indices.md` §1.12: what an
index means, that it is valid only under the B1 recipe (`--boot autoexec`, the silent emulator) at the
pinned cycles, that a moving screen's alignment is not reproducible between sessions so `compare`
searches ±1 frame and reports the offset (D18), why spec §8.4's structural-similarity fallback was
**not needed** (±1 keeps the comparison pixel-exact and searches only the alignment; structural
similarity would give up exactness), how a part that does not reproduce is handled, and the unresolved
items above. `docs/decisions.md` D20 records the same policy as a decision.

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

### Phase 2's evidence, appended task by task

#### Task 2 — the reference record is machine-readable, and the tree is verified against it

The reference tree's bytes were pinned by `docs/fidelity/reference-indices.md` alone, which the
close-out above says is the gap: no test read `reference/`, and `compare` of the tree against itself
caught a *missing or truncated* file but never **a byte changed in place**. `reference capture` now
writes each frame file's digest into the manifest beside it — `sha256` for the RGB frame,
`idx_sha256` for the index plane, which `compare` prefers and an RGB-only digest would leave
unwatched — and `cargo xtask reference verify --tree reference` walks the tree with `compare`'s own
all-the-way-down walk, checking every capture against its own manifest: every file it names is there
at the length its geometry implies, every `frame_*` file beside it is one it names, and every digest
matches the bytes.

**The backfill, and the three conditions the plan put on writing inside `reference/`.**

```
$ cargo xtask reference backfill --tree reference
backfill: 24 capture(s), 105 digest(s) recorded into 24 manifest(s)
```

- **Hashed out of band, before and after.** `find reference -name 'frame_*' -exec sha256sum {} + |
  sort` before the backfill and after it: both lists 105 lines, `diff` empty, the two files hashing
  to the same `465b5eb036dc0fbce15039bc4abee54be8d3855eec5c04cd1376ff961ab29ea9`. `git status
  --short reference` lists 24 files, every one a `manifest.json`; no frame file is touched.
- **Idempotent.** A second run prints `every digest already recorded and matching — nothing written`,
  and a `sha256sum` of all 133 files under `reference/` is identical before and after it.
- **Checked against the pre-change list, not against the tool's own work.** All 105 digests in the
  manifests were compared with the out-of-band list taken *before* the backfill: 105 checked, 0 wrong.

**The verifier passes on the tree** — `cargo xtask reference verify --tree reference`, exit 0:

```
ok   check_a/part: 2 frame(s), 4 file(s)
ok   check_a/title: 2 frame(s), 4 file(s)
ok   parts/3dsinfld: 4 frame(s), 4 file(s)
ok   parts/alku: 4 frame(s), 4 file(s)
ok   parts/beglogo: 4 frame(s), 6 file(s)
ok   parts/cred: 5 frame(s), 5 file(s)
ok   parts/ddstars: 4 frame(s), 4 file(s)
ok   parts/endlogo: 4 frame(s), 6 file(s)
ok   parts/endscrl: 4 frame(s), 4 file(s)
ok   parts/glenz: 4 frame(s), 5 file(s)
ok   parts/jplogo: 3 frame(s), 3 file(s)
ok   parts/lnszoom: 5 frame(s), 7 file(s)
ok   parts/minvball: 4 frame(s), 4 file(s)
ok   parts/mntscrl: 4 frame(s), 5 file(s)
ok   parts/pam: 4 frame(s), 5 file(s)
ok   parts/panicend: 3 frame(s), 4 file(s)
ok   parts/plzpart/cube: 2 frame(s), 2 file(s)
ok   parts/plzpart/plasma: 3 frame(s), 3 file(s)
ok   parts/rayscrl: 4 frame(s), 5 file(s)
ok   parts/techno/interference: 4 frame(s), 4 file(s)
ok   parts/techno/troll: 3 frame(s), 4 file(s)
ok   parts/tunneli: 4 frame(s), 4 file(s)
ok   parts/u2a: 3 frame(s), 5 file(s)
ok   parts/u2e: 4 frame(s), 4 file(s)
verdict: ok — 24 capture(s), 105 frame file(s), 25792000 bytes
```

**And it fails when it should.** A copy of `reference/parts/beglogo` under `out/` with one byte of
`frame_000560.rgb` changed (offset 1234), exit 1:

```
$ cp -r reference/parts/beglogo out/verify-flip/ && printf '\x2a' | dd of=out/verify-flip/beglogo/frame_000560.rgb bs=1 seek=1234 conv=notrunc
$ cargo xtask reference verify --tree out/verify-flip
FAIL beglogo/frame_000560.rgb: the manifest records sha256 05a130da7b95c3527ed5699a72ebfb389f4b74881d9c7a079b8159908af8e76a and the file is 0d434ffebcc038ab1999359f11c58e7a20aac82ef56a29ff84be2215e3623aa3
verdict: 1 finding(s) — 1 capture(s), 6 frame file(s) checked
```

`reference backfill` is the one command in the project allowed to write inside `reference/` — the
phase plan's single, evidenced exception (`docs/plans/02-shell.md`, Task 2). It writes manifest
metadata and no frame file, refuses a tree it cannot read without writing anything, and refuses to
write over a digest that disagrees with the file: a backfill run after a frame changed would
otherwise record the change as the truth, which is worse than no digest at all.

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

A comparison that cannot run at all (a geometry mismatch, a reference index with no
rendered frame within one frame of it, a missing manifest or frame file) is refused with a
legible message and exit code 2, so a script can tell "the frames differ" from "the tool
could not tell".

Both checks above were re-run after B1 re-captured `reference/check_a` (D17; the same two
screens at new indices, frames byte-identical to the ones they replace), and the numbers
are the same: self-comparison `verdict: identical` exit 0, the `SECOND 5` negative
`verdict: different` exit 1 with title 99.5609% and part 40.7172%.

**The negative check above was measured before B1b, and B1b changes its outcome.** The two
captures are 300 frames apart by index (`check_a` at 700/900/1350/1450, `check_neg` at
400/600/1100/1150), and the old pairing — which compared by position in the list, not by
index — zipped them and printed those percentages. Pairing is now by index within ±1 frame
(D18), so the same command is **refused, exit code 2**, naming reference frame 1350 and the
nearest rendered index 1150. The identical-input check is unaffected: `check_a` against
itself is still `verdict: identical`, every offset 0, exit 0.

What a negative check now needs is two captures *at the same indices* that show different
screens, and there is one in `out/` (scratch, not in the tree): `out/a8-capture` (title
screen, `SECOND 2`, frames 400/600, indexed) against `out/check_neg/title` (`SECOND 5`'s end
logo, same frames) → both frames 99.5609% differing, every offset 0, `verdict: different`,
exit 1. D18's table carries the same point on the moving part screen (14.8531% and 0.0063%,
exit 1).

#### Task 3 — the view origin widened to `u32`, and D16's open item closed

`View::start_addr` and `View::x_offset` were both `u16` (spec §4.2), so the largest origin the beam
could start from was 131,070 pixels — while the model's own surface is 256,000. Two parts in this
phase need more than that: CRED displays 200 rows from logical line 400 of a 320-pixel logical line,
`tw_setstart(160*200)` (`CREDITS/MAIN.C:167`, 160 planar bytes per row per CRTC 0x13 = `0x5013`),
**128,000** pixels; and ENDSCRL's display start steps one 640-pixel line per written line, up to
**256,000** (`ENDSCRL/MAIN.C:93-95`). Both would have wrapped *silently* — the failure is not a
crash but a live-looking picture read from several hundred lines away.

`start_addr` is `u32`; `set_origin(pixel: u32)` now writes `start_addr` and zeroes `x_offset`
(`x_offset` stays `u16`: an offset within a displayed line, not an origin); `source_index` is
unchanged in shape and sums in `usize`. Spec §4.2 is amended in place below its struct sketch, and
D16's open item is closed with the arithmetic and with what the `u16` was ever protecting (nothing —
the fields were copied from a register pair that counts bytes within a plane).

**RED, before the change: the tests cannot even be written against the old type.**

```
$ cargo test -p sr-engine
error: this arithmetic operation will overflow
   --> crates/sr-engine/src/view.rs:227:22
    |
227 |         v.set_origin(400 * 320);
    |                      ^^^^^^^^^ attempt to compute `400_u16 * 320_u16`, which would overflow
error: literal out of range for `u16`
   --> crates/sr-engine/src/view.rs:208:22
    |
208 |         v.set_origin(256_000);
    |                      ^^^^^^^
    = note: the literal `256_000` does not fit into the type `u16` whose range is `0..=65535`
```

**GREEN: 483 passed, 0 failed, 0 warnings** across the workspace (101 in `sr-engine`; the count was
480 and the three new tests are the widening's). The suite did not shrink anywhere else, and
`sr-engine`'s `[dependencies]` is still empty.

**The test card's pictures are unchanged, and that was measured rather than assumed.** The card's
`SPLIT_WALK_ROWS` derived itself from `u16::MAX` so it would follow a widening — and following it
asks for a walk of ~6.7 million rows at one surface row per step, which is a hang. The constant is a
plain `3` (the walking origin is 64,000..65,280 either way, unchanged), and the card's composed
frames were dumped before and after the change, spanning all five phases, phase 2's walk and the
phase wrap:

```
$ cargo run --bin secondreality -- --part testcard --headless \
    --dump-frames 3,31,40,47,64,65,66,95,100,130,140,159,160,200,300 --dump-dir out/task3-after
$ (cd out/task3-after  && sha256sum * | sha256sum)
67b7e332e0907691e1077b0cc7283e56703682ef136c1e257c4b07cb176ee7c7  -
$ # and the same command with the change stashed, into out/task3-before:
67b7e332e0907691e1077b0cc7283e56703682ef136c1e257c4b07cb176ee7c7  -
```

Fifteen frames, byte for byte. The card still exercises both fields — its scroll phase pans within
the line through `x_offset`, its split window walks `start_addr` — so the widening is covered
end-to-end by the card as well as by the view's own unit tests.

**Fix round 1 (review).** The review approved the change and found two small things. D16 item 1 and
the plan both claimed a test asserted `sr-engine`'s empty `[dependencies]`, and no test in the tree
did — the guard existed only in prose, just as Tasks 4–10 start adding crates to the binary. It is
real now: `the_engine_has_no_runtime_dependencies` in `crates/sr-engine/src/lib.rs` reads the
manifest's own text (not through a TOML crate, which would be the first dependency it forbids) and
fails on any entry under `[dependencies]` or on a `[dependencies.x]` table. Mutation-checked both
ways — a plain line and a table were each caught, naming the line — and the manifest ends the round
byte-identical to `ee63aa7`'s. The second thing was a word: the two places that said §4.2's
amendment sits "above the struct sketch" now say below, which is where it is. 484 passed, 0 failed,
0 warnings.

#### Task 5 — TUNNELI: the dot tunnel, and a palette the tree's source does not have

`bins/secondreality/src/parts/tunneli.rs`, registered as `tunneli`; `TUNNELI/TUN10.PAS` translated
act for act with its two linked data blobs read in their original form (`TUNNELI/TUNNEL.DAT`,
`TUNNELI/SINIT.DAT` — neither is a picture, so neither goes through a decoder; the note's §2 says so
with the measurement). **Fifteen unit tests** beside the part and two through the real binary.

**The one place the port does not follow the tree's source, and the measurement that says why.**
`TUN10.PAS:122` writes a second grey ramp of `(64-x) *3 div 4` at DAC entries 128..192, and the
tree's own `TUN10.EXE` compiles exactly that (`3*(64-x)` then `idiv 4`, image offsets `0x1d0`-0x1e3).
**The build that shipped is a later revision of that one expression: its ramp is `(64-x) *3 div 5`.**
25 of the ramp's entries are readable through the part's own dots and every one matches, exactly
(including the five where truncation and rounding differ); and a search over 21 candidate ramps
rendered against the four reference frames puts the port's colour field 1,108 pixels from them at 3/5
and at least 4,200 at every other (the source's own 3/4 is 6,376). The port takes the shipped ramp —
D12: "the distributed release is the reference of record for every frame diff… the source tree
remains the reference for *how* every effect works" — with the derivation in
`docs/fidelity/tunneli.md` §5, the entry in `APPROXIMATIONS.md` A6, and D12's own open item
(are the tree's part EXEs the packed ones?) gained as its first data point in `docs/decisions.md`.

**The comparison, run and recorded.** The plan's Step 3 command (`--dump-frames 900,1060,2000`) is
**refused, exit 2** — `reference frame 3800 has no rendered frame within ±1 of it … the nearest
rendered index is 2000, 1800 frames away`. Run at the reference's own indices
(`--dump-frames 3800,4200,4500,4600`) it is **exit 1**: `3800: 3.7812 % (2420/64000 px), 4200:
5.6312 % (3604), 4500: 3.1422 % (2011), 4600: 4.9344 % (3158)` — and every differing pixel is a
reference dot, because a `--part tunneli` run draws its own ticks from zero and is holding the black
screen at 3,800 while the reference holds the tunnel. That number is the numbering's gap, not the
port's.

At the alignment the reference itself supplies — offset **3,722**, the port's ticks 78/478/778/878
answering 3800/4200/4500/4600 — the port differs by **179 / 373 / 321 / 433 pixels of 64,000 (0.28 /
0.58 / 0.50 / 0.68 %)**, and the difference is *presence*, not colour (the largest channel delta at
3800 is 12, against ~235 at the other three, where a port dot meets the reference's black). The
residual is attributed honestly rather than excused by the oracle: at 3800 the reference is
**settled** (7 of 9 sessions, and the re-capture reproduced it byte for byte), so 179 pixels there
are the port's or a sub-frame effect in it; at 4200 the census is exactly half; and at 4500 and 4600
— **no settled content** — nothing can attribute them. `APPROXIMATIONS.md` A6 carries the table and
what is ruled out (the ramp; a whole-tick error, which would cost thousands; and the pre-fix
partners' 8.07-10.18 %, which §8.4 itself reframes as "≈1 frame … ≈4 frames" — the history of a
reference that has been replaced).

**The unchecked read past `rows` (`:179-190`) is preserved because the source has it, and it is
invisible.** Every y the part's own run puts outside `rows` reads a word of 64,000 — `aa2`'s copy of
`aa1` (`:99-100`) on one side, `aa1` past the wrap on the other — so the dot plots at `x + 64,000`,
past the 64,000 bytes a 200-line mode 13h screen reads: more than 100,000 such reads in a full run,
**not one** of them on screen, and a bounded read gives byte-identical frames. The path that *is*
visible is substituting 0 (the value of `aa2`'s unwritten half): that plots at row 0 and costs 85,
142 and 88 extra differing pixels at 4200, 4500 and 4600, which is why the read is modelled rather
than zeroed.

**The sheets, which are the evidence a person reads.** `out/p2-tunneli-aligned-sheet.png` (the
port's four aligned frames) beside `out/p2-tunneli-reference-sheet.png` (the reference's own four):
the mouth, the full tunnel, the off-centre tunnel and the thinner one, the same four shapes on both.
**The look itself is deferred to the phase boundary**: nothing here claims a person has judged them.

**And the phase has two tools it did not have.** `cargo xtask compare --align <N>` reads the rendered
side's frame indices `N` higher when pairing them — the frames' own indices are not rewritten, the
alignment is printed before the frames, and every reported offset is the net one — which is the only
way a part the port cannot anchor can be measured at all (TUNNELI, CRED and ENDSCRL all have
`boot_anchor == None`); it is pinned both ways in `xtask/tests/compare.rs`. And
`cargo xtask frames sheet --dir <DIR> --out <PNG>` sheets any directory of dumped frames, because
the plan's Step 3 named `xtask reference capture --sheet` for that and `--sheet` is wired into
`assets verify` and `reference capture`, neither of which reads an arbitrary directory.

#### Task 4 — BEGLOGO: the phase's exit criterion, met

**The comparison is exit 0.** The two commands, verbatim:

```
$ cargo run --bin secondreality -- --headless --part beglogo --dump-frames 430,470,560,900 \
      --dump-dir out/p2-beglogo
$ cargo xtask compare --reference reference/parts/beglogo --rendered out/p2-beglogo \
      --out out/p2-beglogo-diff
frame 430: 0.0000% differing (0/128000 px), max delta 0
frame 470: 0.0000% differing (0/128000 px), max delta 0
frame 560: 0.0000% differing (0/128000 px), max delta 0
frame 900: 0.0000% differing (0/128000 px), max delta 0
verdict: identical
exit: 0
```

`mode: mixed`, every offset 0 (`{0: 4}` in the report), 0 pixels of 128,000. 430 and 900 are the
indexed pair and 470 and 560 the RGB pair, exactly as `reference-indices.md` §3.1 predicted — so the
DAC's 6→8-bit widening was live on two of the four frames that judge the phase, and it agrees.

**The part.** `bins/secondreality/src/parts/beglogo.rs`, registered as `beglogo`; `BEG/BEG.C`'s four
acts translated act for act and tick for tick, with `sr_assets::load_path` reading
`assets/BEG/SRTITLE.UP` in its original format. Eleven tests: six beside the part (unit) and five
through the real binary from the repository root (`tests/beglogo.rs`) — **495 passed, 0 failed, 0
warnings** (was 484; the count went up, not down).

Four things this task settled rather than assumed, and each changed what the code does:

- **Which of part 4's two builds shipped: `BEG/`.** `ENDPIC/MAKEFILE:22` stages a second `beg.exe`
  as `beglogo.exe`, and the two are not interchangeable — `ENDPIC/BEG.C:18-20` sets its own mode and
  `:45-48` waits on `dis_muscode(0xf0)`. Settled by three measurements: the reference's held frame
  900 probes 57 palette entries that `BEG/SRTITLE.UP`'s palette matches **57/57** and
  `ENDPIC/SRTITLE.U`'s matches **0/57**; `SRTITLE.UP` decodes to that frame's indices exactly (0 of
  128,000 pixels) while `ENDPIC/SRTITLE.U` decodes to a different picture (126,261 of 128,000 pixels
  apart); and the staged `MAIN/DATA/BEGLOGO.EXE` is 30,874 bytes, which a packer produces from
  `BEG/BEG.EXE`'s 50,830 and not from `ENDPIC/BEG.EXE`'s 13,286. **So BEGLOGO has no music gate**,
  `_np_zinfo`'s exposure narrows to GRID, HARD and the vault, and spec §7.4, the risk table and D8
  are amended in place.
- **The fade's first index is 446, by exhaustive match and not by fit.** The scan — the source's own
  ramp, run over every `c` in 0…128, widened and compared over all 320×400 pixels — gives 470 at
  **c = 24** and 560 at **c = 114**, each *unique*, and 470 − 24 = 560 − 114 = 446. It is a permanent
  test (`the_fade_constants_are_derived_by_matching_the_reference_bit_for_bit`), so the constant
  cannot drift from its evidence. 900's scan gives c ∈ {126,127,128} — the ramp has converged, so it
  pins the end and not the start — and 430 gives all 129 values: the frame is one index (15)
  repeated, and entry 15 is white in the picture's own palette, so *no* `c` is distinguishable
  there. The plan's note said 430 matched only at c = 0; the measurement says otherwise, and §1 of
  the part's note records it. The fit over means (§3.3) gives ≈441 and remains wrong.
- **The blit cannot be in `init`.** The plan's sketch put it there, and the keyframe refuses it:
  `frame_000430.idx` is 128,000 bytes of 15, and `compare` prefers the *indexed* path when both
  sides have indices. A port that had blitted before tick 0 would render 430 white in RGB while its
  `.idx` held the picture's indices — a diff at a keyframe that is supposed to be trivially white.
  `BEG/BEG.C:24`'s 32 `dis_waitb()`s come before `:73-78` in the source too, so the blit is the
  tick after them (tick 32, index 436) and the DAC write of `:25-34` is on that same tick: the
  white at 430 is entry 15 of the table the *shell* installed, untouched by the part.
- **`setpalarea`'s byte-count bug is written, not fixed.** `BEG/ASM.ASM:106-108` computes
  `2*count + start`, so `setpalarea(pal2,0,254)` uploads 508 bytes: entries 0-168 whole, the red
  byte of 169, nothing of 170-254. Confirmed in the shipped binary — the routine is at `BEG.EXE`
  file offset `0xcb7` and the arithmetic at `0xccb`-`0x0cd1`. It is **invisible on this art**
  (measured: the same scan with a full 762-byte upload returns the same `c`), which is exactly why
  it is a decision and a `docs/fidelity/bugs.md` entry (§4) rather than a coincidence — the next
  part to paint into entries 170-254 will meet them still white.

The plan's Step 6 file list is followed, plus `docs/fidelity/asset-inventory.md` (its item 10 asked
Phase 2 to settle the build), `docs/fidelity/APPROXIMATIONS.md` (A5, §below) and this file:
`bins/secondreality/{Cargo.toml,src/parts.rs,src/parts/beglogo.rs,tests/beglogo.rs}`,
`docs/fidelity/{beglogo.md,bugs.md,part-sequence.md,asset-inventory.md,APPROXIMATIONS.md}`,
`docs/spec/…-design.md`, `docs/decisions.md`. The full note — the derivation table, the tick map, the
two Part B conventions and what is not modelled — is `docs/fidelity/beglogo.md`.

**Fix round 1 (review).** The review approved the task and **re-derived the headline result with its
own decoder**, from the source and the artefacts, so the phase's central risk is answered
independently of this report. Seven findings, all applied:

- **Three false numbers in the note's opening line** ("704 lines of C and assembly in four files,
  `BEG/BEG.C` 70 of them"): `BEG/` holds **three** source files, **269** lines — `BEG.C` 87,
  `ASM.ASM` 118, `READP.C` 64. It is the same error the module doc had already been fixed for, so
  the two files contradicted each other in one commit.
- **The nine-frame residual was attributed to the loader as a fact, and the evidence does not
  support it.** `reference-indices.md` §3.3's 410 sample is byte-identical white — `BEG/BEG.C:20-21`'s
  memsets had already run — so the loader cannot occupy 437-445; and `dis_partstart()` is a *version
  check* that returns immediately (`DIS/DISC.ASM:51-64`), costing no frame, so "the frames it stands
  for are inside `FADE_FIRST_TICK`" was empty. The part's comments and the note now say only what is
  measured: **nine frames of the 42 are frames the port does not model, and the cause is
  unidentified**.
- **`APPROXIMATIONS.md` A5** registers it: the magnitude (9 of 42), both candidates ruled out with
  their sources, and what a later phase should watch — Phase 9 compares every frame, and 436-444 is
  where the port's *index plane* holds the picture while a later start would still hold the white
  field, both RGBs white either way. The register's bar is not "non-zero difference": A2 and A4
  already record zero.
- **`lineblit`'s citation was the inner loop.** The procedure is `BEG/ASM.ASM:53-82`; `:63-76` is the
  four-plane loop inside it.
- **The origin comment contradicted the code.** `set_origin(0)` zeroes `x_offset` as well as
  `start_addr` — CRTC 0x0c/0x0d is one 16-bit display start, and both halves of the port's origin are
  what those two writes set — so the comment now says that, and keeps the registers `BEG.C` does not
  write as the ones left alone.
- **The border claim.** The test comment justified `:32-34`'s entry 255 by "the *border* does [check
  it]", but no comparison carries a border (`headless.rs` dumps with `border: false`). The comment now
  says what the write is for: the next part, where entry 255 black is why GLENZ's `zoomer2` bands are
  black over a greying picture (`reference-indices.md` §3.5).
- **470 and 560 are pinned by the suite, not only by the tool**: `tests/beglogo.rs` byte-compares the
  dumped RGB of both against `reference/parts/beglogo`'s own — the pair that would show the DAC
  widening if it differed.

The comparison was not re-run for the round: nothing changed in the part's tick mapping, its
arithmetic or its palette path, so exit 0 is untouched by these edits.

**Fix round 2 (scoped re-review).** Two things, both the same defect in different clothes — a check
that could not fail, which is the shape this phase's reviews keep finding:

- **The new comparison in `tests/beglogo.rs` counted over whatever two frames had in common.**
  `iter().zip()` stops at the shorter side, so a *truncated* dump compared a prefix, reported zero
  differences and passed, while the message printed the reference's length as though it were what had
  been compared. The helper now asserts both lengths first — and
  `a_truncated_frame_is_refused_rather_than_counted` (`#[should_panic]`) proves the assertion fires
  rather than describing it. The same idiom in the part's own derivation test (the decode against
  `frame_000900.idx`) is guarded the same way.
- **A false count in `beglogo.md` §8** — "the three reference frames the part's own arithmetic does
  not already pin" was not a set of three (the unit derivation already matches 470 and 560, and the
  decode is 900's indices). The paragraph now says what the integration tests actually add: the path
  a run really takes, end to end, with the one reference *file* they do not compare byte-for-byte
  named (`frame_000430.idx`).

## Phase log

### Phase 1 — Assets & references (done, tag `phase-1`)

Sixteen tasks on branch `phase-1-assets` (A1–A8, B1, B1b, B2–B6). **405 tests pass, zero failures, one
ignored, zero warnings.**

**What was built.** `sr-assets` reads the original's media in its own formats — Origin PIC in five
variants plus a headerless case, IFF (ILBM and PBM), RIX3, `.PAL`, OMF `LEDATA`, FLI and `.ANI`, and a
proportional font-strip view — with `cargo xtask assets verify` checking the curated tree against
`assets/PROVENANCE.md` and rendering a contact sheet per part. `cargo xtask reference capture` drives
the emulated original to capture reference frames, and `cargo xtask compare` diffs against them.

**The asset tree.** 150 assets plus `PROVENANCE.md`; `assets verify --tree --decode` exits 0 with
`verdict: ok`. All 149 curated copies were hashed against `upstream/SecondReality` at the close-out and
every one is byte-identical; the 150th was recovered from an EXE's `LEDATA` records.

**The references.** Every slot in `docs/fidelity/part-sequence.md` §1 — 19 sequence parts plus the
hidden DDSTARS — has a capture under `reference/parts/`, 22 capture directories and 83 keyframes, each
row in `docs/fidelity/reference-indices.md` carrying its indices, geometry, hashes and re-capture
result.

**What the phase learned about its own oracle, because it is what the references mean.** The boot
repeats exactly once the host is out of it (`--boot autoexec` plus a silent emulator, D17), and the
frame grab is counter-verified on both sides. But the emulator's picture at a given boot index is not
fully reproducible: sessions fall into **discrete alignment groups**, the majority group byte-identical,
so `compare` searches **±1 frame** and reports the offset while leaving every pixel exact (D18). A
keyframe is filed only if at least half the sessions can present that picture at 0.0000 % within that
window, with the reference's own capture excluded from the count (D19); where none can, the row records
it as having **no settled content** rather than implying a majority. The §8.4 structural-similarity
fallback was **not needed** (D20). One excursion to ~30–40 frames remains real, unreproduced and
unexplained, and is recorded as such.

**Phase 2 got one result out of this phase before starting.** PLZPART's plasma confirms on captured
frames that the port's `Split::scanline` is the emulator's line-compare register **plus one** —
`62/61/62` for a register alternating 60/61, with `do_drop`'s own `dtau` table cross-checking it.

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
  and the title screen is around frame 1700. `d0ed148`. **(The 1657/1700 landmarks are Phase 0's
  typed-recipe numbers and are superseded by D17: under B1's `--boot autoexec` recipe the setup
  screen is at 16 and the demo's first graphics frame at 404, and every reference in the tree is
  boot-relative to that recipe. The numbers are left standing as the record of what was measured
  then.)**
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

