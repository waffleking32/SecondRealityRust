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
| 2 | Shell + easy tier | **done** — BEGLOGO and ENDLOGO frame-diff-identical (exit 0, every keyframe, both planes byte-identical); TUNNELI, CRED and ENDSCRL play, each residual measured and attributed (exit 1, A6/A10/A11); `--block 5` sequences them. Evidence below | [02-shell.md](docs/plans/02-shell.md) | `phase-2` |
| 3 | Music engine | **in progress** — Tasks 1-7 landed on branch `phase-3-music` (the trace, the player, the mixer, the shell's restarts and clock, the device); Task 8 replaced ENDLOGO's stand-in gate with the source's own loop and re-ran the phase's acceptance: four keyframes, exit 0, 0.0000 %, both planes byte-identical. Evidence below | [03-music.md](docs/plans/03-music.md) | — |
| 4 | Music-synced tier A | not started | — | — |
| 5 | Mid tier | not started | — | — |
| 6 | Vector engine | not started | — | — |
| 7 | GLENZ | not started | — | — |
| 8 | Vault | not started | — | — |
| 9 | Assembly & fidelity pass | not started | — | — |

## Where to pick up

**Phase 2 is complete** — its exit criteria are met, its evidence is recorded below, and **the
`phase-2` tag is applied at this commit.** The work is on branch **`phase-2-shell`**, not yet merged
to `master`: the close-out commit is `339f4bd`, the comparisons below were re-run at `0c78cd3`, and
the commits after that point are the close-out's own record and the whole-branch review's fix round
(§ "The whole-branch review, and its fix round"). Its plan is `docs/plans/02-shell.md` and its ledger
(the rulings and the per-task results, git-ignored scratch) is
`.superpowers/sdd/02-shell/progress.md`.

The tag was withheld until the final whole-branch review had run, on Phase 1's ruling that a tag
which has to be moved is worse than a tag applied a few commits later. That review came back *needs
fixes* with no Critical — thirteen findings, all records plus one clippy item, **nothing that changes
what any part draws** — and it re-derived the exit criteria itself rather than reading them: it
rebuilt the release binary, re-dumped and re-compared four parts, re-hashed every frame the reference
files, and flipped a byte in both planes of `reference verify` to watch it fail. The fix round
addressed all thirteen and a scoped re-review confirmed each one, verifying the clippy arithmetic by
staged runs in a scratch tree (5 → 6 → 7 under the plan's command, 11 more under `--all-targets`: 18
in all) and checking every code change for behaviour-neutrality. Its words: "claims that do not
reproduce are the exception here, not the rule".

**Phase 3 (music) is in progress on `phase-3-music`** — its plan is `docs/plans/03-music.md`, its
per-task ledger `.superpowers/sdd/03-music/progress.md`, and Tasks 1-7 are on the branch. Task 8, the
phase's proof, is recorded below. What Phase 3 inherits from Phase 2:

- **Every music gate in the port is a measured frame, and Phase 3 replaces them.** The phase's one
  gate is ENDLOGO's hold — `END.C:107-111`'s `dis_musplus() > -16` loop, replaced by its measured
  567 ticks (`APPROXIMATIONS.md` **A7**, `docs/fidelity/endlogo.md` §5). Phase 3's cue is the music
  player's position, sampled once a tick, with `:110`'s −16 threshold. No other ported part polls
  music (BEGLOGO's build has no gate — `docs/fidelity/beglogo.md` §1).
- **The reference material it needs is already measured**: `reference-indices.md` §1.7's music
  counter (`0x105B0E`, 0.7424/frame) and D13's correction of spec §2.2's restart table, plus D20.
- **`_np_zinfo` is still open** in the narrowed form D8 now has: the ported parts do not touch it,
  and the parts that do are GRID, HARD and the vault (Phase 8).
- **The divergence rule it will need** if a sync cue disagrees with a faithful port is **D24** (and
  for any number it writes into a note, **D25**).

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
started from `master`** (`5cf3e88`, which is `phase-1` plus the merge record and the Phase 2 plan)
and lives on `phase-2-shell`; it is merged to `master` when its tag goes on.

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

### Phase 2's evidence (close-out, Task 11, 2026-10-13)

**Exit criteria (spec §10): BEGLOGO frame-diff-identical; TUNNELI, ENDSCRL, CREDITS and ENDLOGO
play; a runnable binary sequences them.** All three are met. Every comparison below was **re-run at
the committed state** (`0c78cd3`, branch `phase-2-shell`) in one pass, each dump into a fresh output
directory, and the numbers are the ones the phase's notes recorded when they were first taken (the
dumps here were `--release` binaries; the notes' were `cargo run --bin` — the two agree on every
figure and, for BEGLOGO and ENDLOGO, on every digest).

| part | exit | offsets | differing | attribution of the non-zero frames |
|---|---|---|---|---|
| BEGLOGO | **0** | all 0 (`{0: 4}`) | **0.0000 %** on all four (0/128,000 px) | — (and both planes are byte-identical, below) |
| TUNNELI, at the reference's own indices | 1 | all 0 | 3.7812 / 5.6312 / 3.1422 / 4.9344 % | the port holds the black screen its tick 1,060 left: a `--part tunneli` run has no boot anchor, so those indices are not its own — the numbering's gap, not the port's |
| TUNNELI, at `--align 3722` | 1 | net 0 on all four | 0.2797 / 0.5828 / 0.5016 / 0.6766 % (179/373/321/433 px) | **A6**, per keyframe: 3800 settled (7/9, re-capture byte-identical) so its 179 px are the port's or a sub-frame effect; 4200 exactly half; 4500/4600 no settled content |
| ENDLOGO | **0** | all 0 (`{0: 4}`) | **0.0000 %** on all four | — (and both planes byte-identical, below) |
| CRED | 1 | all 0 (`{0: 5}`) | 0.0000 / 11.2563 / 9.5125 / 0.0000 / 5.8453 % | **A10**: the shipped `CRED.EXE` is not the tree's data — captions the source cannot draw, a different picture, 9 of 21 screens |
| ENDSCRL | 1 | all 0 (`{0: 4}`) | 2.8473 / 11.0676 / 9.9617 / 6.6828 % | **A11** (the shipped build's pitch, `[NN` control lines, font order, raster and CRs) **plus the chain's 82 frames** (below) |

**BEGLOGO — the exit criterion — is exit 0, and on both planes.** Verbatim, at `0c78cd3`:

```
$ cargo run --release --bin secondreality -- --headless --part beglogo \
      --dump-frames 430,470,560,900 --dump-dir out/t11-beglogo
$ cargo xtask compare --reference reference/parts/beglogo --rendered out/t11-beglogo \
      --out out/t11-beglogo-diff
frame 430: 0.0000% differing (0/128000 px), max delta 0
frame 470: 0.0000% differing (0/128000 px), max delta 0
frame 560: 0.0000% differing (0/128000 px), max delta 0
frame 900: 0.0000% differing (0/128000 px), max delta 0
report: out/t11-beglogo-diff/report.json
captures: 1 measured
verdict: identical
exit: 0
```

`mode: mixed`, every frame paired at offset 0 — 430 and 900 through the **indexed** path (the
reference files an `.idx` for both) and 470/560 through RGB. **The tool's verdict is not the whole
claim**: hashing the port's six dumped files against the reference's own six gives **identical
digests**, `430.rgb 85789170…`, `430.idx 283babe8…`, `470.rgb d0abd8cc…`, `560.rgb 05a130da…`,
`900.rgb cbdc67eb…`, `900.idx e10481c7…`, and every one equals the value
`reference-indices.md` §3.1 records **by hand** from Phase 1 — so the port is exact on the frames the
indexed path skips as well as the ones it takes, and a wrong palette could not hide behind agreeing
indices.

**TUNNELI cannot be diffed, and its two readings say what it is instead.** At the reference's own
indices — a numbering this part does not have — `compare` exits 1:

```
frame 3800: 3.7812% differing (2420/64000 px), max delta 190
frame 4200: 5.6312% differing (3604/64000 px), max delta 239
frame 4500: 3.1422% differing (2011/64000 px), max delta 231
frame 4600: 4.9344% differing (3158/64000 px), max delta 239
verdict: different     exit: 1
```

Every differing pixel is ink on the *reference* side, because the port's run retired at tick 1,060
and holds the black screen its fade left there. At the alignment the reference supplies — port ticks
78/478/778/878 answering 3800/4200/4500/4600, one offset of **3,722** — the same command with
`--align 3722` exits 1 with **0.2797 / 0.5828 / 0.5016 / 0.6766 %** (179/373/321/433 px of 64,000),
max channel delta 12 at 3800 (the other three are ~230, where a port dot meets the reference's
black). That residual is **A6**, and it is attributed per keyframe rather than averaged: 3800 is
settled (7 of 9 sessions; a re-capture reproduced it byte-for-byte) so its 179 px are the port's or a
sub-frame difference in it; 4200 is exactly half; 4500 and 4600 have no settled content and nothing
can attribute them. What is *not* claimed: that the frames agree. They do not, and the note's §7 and
A6 carry the numbers a later phase meets again.

**ENDLOGO — exit 0 on all four keyframes, and byte-identical on every file the reference files (RGB
at all four, an index plane at 430 and 900):**

```
frame 430: 0.0000% differing (0/128000 px), max delta 0
frame 500: 0.0000% differing (0/128000 px), max delta 0
frame 900: 0.0000% differing (0/128000 px), max delta 0
frame 1180: 0.0000% differing (0/128000 px), max delta 0
verdict: identical     exit: 0
```

Digests: `430.rgb 85789170…` — **the same bytes as BEGLOGO's 430**, the two parts reaching the same
white by opposite routes (BEGLOGO writes entry 255 black; ENDLOGO leaves 255 unwritten) — plus
`430.idx eec19bc6…`, `500.rgb fb8fbf2a…`, `900.rgb f2514b29…`, `900.idx c0e0e259…`,
`1180.rgb 755f4747…`, all equal to the values Phase 1's §19 records.

**CRED — exit 1, all five keyframes paired at offset 0, `mode: rgb`:**

```
$ cargo run --release --bin secondreality -- --block 5 --headless \
      --dump-frames 1440,3240,5440,7640,8740 --dump-dir out/t11-cred
$ cargo xtask compare --reference reference/parts/cred --rendered out/t11-cred --out out/t11-cred-diff
frame 1440: 0.0000% differing (0/64000 px), max delta 0
frame 3240: 11.2563% differing (7204/64000 px), max delta 255
frame 5440: 9.5125% differing (6088/64000 px), max delta 255
frame 7640: 0.0000% differing (0/64000 px), max delta 0
frame 8740: 5.8453% differing (3741/64000 px), max delta 255
verdict: different     exit: 1
```

Two of the five are exact — through the same tick map, palette repair and geometry as the three that
are not, which is what makes the difference the **data**: the shipped `CRED.EXE` draws captions the
tree's source does not contain (`SKETCH` appears in no source file) and at least one different
picture (3240's band: 3,524 of its 16,000 px are in none of that screen's 256 palette entries), with
**nine** of the 21 screens off the tree's data at one hold sample each — **A10**, whose lead is the
packed `MAIN/DATA/CRED.EXE` itself. The run is `--block 5` because 1440 is only reachable if
ENDLOGO's measured length is right; the chain is part of this test.

**ENDSCRL — exit 1, all four at offset 0, `mode: rgb`, max delta 243:**

```
$ cargo run --release --bin secondreality -- --block 5 --headless \
      --dump-frames 9100,10000,12000,14000 --dump-dir out/t11-endscrl
$ cargo xtask compare --reference reference/parts/endscrl --rendered out/t11-endscrl \
      --out out/t11-endscrl-diff
frame 9100: 2.8473% differing (7289/256000 px), max delta 243
frame 10000: 11.0676% differing (28333/256000 px), max delta 243
frame 12000: 9.9617% differing (25502/256000 px), max delta 243
frame 14000: 6.6828% differing (17108/256000 px), max delta 243
verdict: different     exit: 1
```

Two residuals in one number, and they are separated: **the chain's 82 frames** (next section) and
**A11** — the shipped build's text pitch of 25 rows a line where `MAIN.C:6` says `FONAY 30`, its
`[NN` control lines (163 in the shipped text) that advance the ring and draw nothing, its
`fonaorder` naming six characters the tree's does not, its raster's 53 px of the header band, and
its renderer's text having no carriage returns. **A single alignment cannot absorb them** — the
pitch moves the content 5 rows per line apart and the control lines skip blocks the port renders — and
the shifted run shows both directions: `--align 82` on a dump of the shifted indices improves 9100
(2.8473 → **2.1758 %**, 5570 px) and worsens 10000 (11.0676 → **12.5437 %**, 32112 px), with 12000
9.5895 % and 14000 7.1887 %. The port stays where the chain puts it.

#### The chain's residual, measured (the plan's Step 2)

The port's own boundaries, read from the manifest's `part` field one index at a time:

```
$ cargo run --release --bin secondreality -- --block 5 --headless \
      --dump-frames 1205,…,1215,8830,…,8840 --dump-dir out/t11-chain
1210 endlogo 320x400   the fadeout's last step
1211 endlogo 320x400   the tick that reports Finished — the loader's hold
1212 cred    320x200   CRED's tick 0
8834 cred    320x200   screen 21's last tick
8835 endscrl 320x200   ENDSCRL's tick 0: the screen CRED left
8837 endscrl 640x400   its tick 2: the mode's own screen
```

- **ENDLOGO's end lands inside the reference's window.** Its last drawn frame is 1210 (its Finished
  tick, 1211, draws nothing new) and `SECOND 5`'s geometry change to CRED is in **(1200, 1240]**
  (`reference-indices.md` §20, where 1200 is ENDLOGO's 320×400 screen and 1240 is CRED's 320×200).
  ✓
- **CRED's end does not — it is 6 to 105 frames early, and that is the finding.** The port's last
  CRED frame is **8834**; the record's own sweep refuses its index 8940 as `640x400` while 8840 is
  still CRED's hold, so the reference's last CRED frame is in **[8840, 8939]** (`cred.md` §7,
  `reference-indices.md` §20). The gap is the same order as CRED's own per-screen drift, and this is
  measured, not asserted: the record's hold samples put the reference's screen at
  `(8840 − 1440) / 20 = 370 ± 10` ticks against the port's **363** (the source's own loops —
  `screen_ticks()` = 1 parked + 1 show + 102 open + 200 hold + 59 close), so the port leads 2–12
  frames a screen and by screen 21 leads 40–240. The reference's scroller start, measured from the
  rows its first text line is drawn on — the one thing both builds agree on — is **8917 ±1**, so the
  port's 8835 is **82 frames (41 scrolled lines) early**, inside the bracket (`endscrl.md` §6.1).
  **It is left where the chain puts it**: the two numbers that put it there are ENDLOGO's measured
  hold and CRED's source-derived screen length, and re-anchoring ENDSCRL to make its own diff pass is
  the fit Review Focus 5 forbids.

So the chain is **consistent with every measurement the tree holds** for ENDLOGO, and for CRED it is
early by an amount the tree's own measurements bracket and partially explain (a per-screen length the
port derives from the source against a reference length measured only to ±10 ticks per screen). The
one thing this cannot settle is which of the two is right: the port's 363 is the source's arithmetic,
and the reference's 360–380 is a measurement with ±10 of slack, so the 82 could be CRED's build
differing, the port's translation differing, or both — and nothing in the tree separates them today.

#### The falsification list (the plan's Step 3)

Every measured constant this phase introduced, with what a one-frame error would change and the
strongest check that exists for it. "Independent" here means a measurement produced for another
purpose (`reference-indices.md` §19.1's sweep, written a phase before ENDLOGO's constants were
derived) or by a different instrument (the tree's own `TUN10.EXE`, disassembled) — not the evidence
the constant was derived from.

| constant | value | how it was derived | one frame out would… | independent check |
|---|---|---|---|---|
| the boot anchor `shell::FIRST_GRAPHICS_FRAME` (`boot_anchor` for `beglogo`/`endlogo`) | 404 | `reference-indices.md` §1.12: the demo's first graphics frame under D17's recipe, measured in five runs of five | shift every index of a `--block 2`/`--block 5` run by one: BEGLOGO's 470 would hold `c = 25` and 560 `c = 115`, ENDLOGO's 500 `c = 50`, so all eight keyframes stop matching bit-exactly | **yes** — the comparisons themselves: the port's frames are diffed at the reference's indices, so an anchor error is a red diff at the eight keyframes, not a silent shift |
| BEGLOGO `FADE_FIRST_INDEX` | 446 | 470 = ramp step 24 and 560 = step 114, **each unique** over 0…128 by bit-exact match on all 320×400 px; 470 − 24 = 560 − 114 = 446 (`beglogo.md` §3) | the whole 129-step ramp sits one frame early or late: 470/560 stop matching (a whole palette step, on every pixel the step moves), the acceptance run is red on two of its four frames, and every unheld frame of the ramp (476–556) is one step out too | **no** — 430 pins nothing (it is one index, 15, and entry 15 is white in the picture's own palette, so *all* 129 `c` render it), 900 pins only the ramp's end (`c ∈ {126,127,128}` are one palette), and the second keyframe is the same kind of evidence 90 frames away. The mean-fit over §3.3 gives 441 and is wrong |
| ENDLOGO `FADE_IN_FIRST_INDEX` | 451 | 500 = ramp step 49, **uniquely**; the ramp counts up, so step 0 is 451 (`endlogo.md` §3) | 500 stops matching (`c = 48/50`) and the fade sits a frame early/late through its length; the hold's first frame becomes 579 or 581 | **yes** — §19.1's sweep, recorded in Phase 1 for another purpose, names 460 and 540 separately with a colour count and mean (two decimals over 384,000 bytes); the derivation test reproduces both, and a ±1 shift moves both |
| ENDLOGO `FADE_OUT_FIRST_INDEX` (and `HOLD_TICKS` = 567 = 1147 − 580, `PART_LAST_INDEX` = 1210) | 1147 | 1180 = ramp step 30, **uniquely**; the ramp counts down, so `c = 63` is displayed at 1147 | 1180 stops matching (`c = 31/29`), the hold is 566 or 568, the last frame moves to 1209/1211 | **yes** — the same sweep names 1160 and 1200 separately, inside this ramp (`c = 50` and `c = 10` under 1147). The geometry window (1200, 1240] does **not** catch a one-frame error: 1210 and 1211 are both inside it |
| ENDLOGO's lead-in tick map (blit at tick 36 = index 440, fade at tick 47 = 451) | 36 / 47 | `END.C:89-99`'s blanks and decode, counted into the tick map (`endlogo.md` §4) | the picture would enter video memory one frame earlier or later; 430 (tick 26, inside the blanks) cannot see it | **no** — ten of the 47 ticks are frames the port does not model and their cause is unidentified (**A7**); the map is consistent with every frame the reference holds, and the only frames that could check it (441–450, in the index plane) are not held |
| ENDSCRL's chain (its tick 0 at index 8835; the reference's scroller start 8917 ±1; the residual's chain part 82 frames) | 82 | the rows the first text line is drawn on, the one thing both builds draw identically; ±1 because a picture is two ticks wide (`endscrl.md` §6.1) | the scroller's content moves half a line at each frame (the origin steps one 640-px line every two ticks) — real, but not separable from A11, whose pitch already moves the content 5 rows per line | **bracketed, not checked** — CRED's measured bracket (6 to 105 frames early) contains 82, and 81 or 83 would sit inside it too; nothing else in the tree can see a one-frame error |
| CRED's screen length | 363 ticks (1 parked + 1 show + 102 open + 200 hold + 59 close) | the source's own loops (`MAIN.C:176`, `:194`, `:196`) and their exact integer sequences (`cred.md` §3) | every later screen's hold and the chain's offset move by (screen − 1) ticks — ±21 at screen 21; the keyframes still land inside their holds (the tightest has 34 frames to spare), and the chain's offset stays inside CRED's bracket | **yes, in the record's sweep** — its 16 wipe-state frames are 0 of 192,000 bytes from the port's, and a one-tick error moves each one step — **but those tests are `#[ignore]`d** (the capture is git-ignored scratch), so a fresh clone cannot re-run that check. The unconditional tests pin the ramps' endpoints and the split mapping, not the coupling |
| TUNNELI's shipped second ramp `(64 - x) * 3 div 5` | 5 | **unique** over 81 `(mul, div)` candidates (70 distinct ramps) rendered against the four reference frames: 1,108 px at 3/5, ≥4,200 at every other, the tree's own 3/4 at 6,376 (`tunneli.md` §5) | the tree's `div 4` puts the port 6,376 px from the reference's colour field instead of 1,108, and the 17 entries where truncation and rounding differ stop matching | **yes** — the tree's own `TUN10.EXE` disassembles to `idiv 4` (`0x1d0`-`0x1e3`), so which build put the frames on screen is measured on two artifacts independently of the frames |
| TUNNELI's alignment (offset 3,722) | 3,722 | the port's ticks 78/478/778/878 answer 3800/4200/4500/4600 — all four at one offset | one frame either way and each frame is its neighbour's picture: at 3800 the residual is 179 px against 754 and 953 at ticks 77 and 79 | **yes** — the neighbours are the check, and it is a permanent test (`the_alignment_is_whole_tick_or_nothing`) |
| A11's shipped-build numbers: the 25-row pitch and the `[NN` rule | 25 / N | read off frames 10000, 12000 and 14000: every gap a multiple of 25 and the smallest 25; the `[NN` rule predicts ring rows 0/25/65/90/130 at 10000 and six 37-row steps at 14000 **from the tree's own text file** | a one-row pitch error moves every predicted band by one row and the rule stops predicting the frames it was not fitted to | **yes** — it is a *prediction* of frames recorded before it (the reviewer's model is 26,756 px off at 12000 without the rule against 1,120 with it). This is a measurement of the **release**, not a port constant |

**The count, measured rather than assumed.** Of the port's own constants, **two** are checked by an
independent fact — ENDLOGO's two fade indices, by §19.1's sweep — and **two** are not: BEGLOGO's
`FADE_FIRST_INDEX`, whose only evidence is the pair of keyframes it was derived from, and ENDSCRL's
chain, which is bracketed but not pinned. The plan's Step 3 expected two independent checks and one
absence; the measurement says two and two, and the difference is recorded here rather than smoothed
over. Two further entries in the table (the boot anchor and TUNNELI's alignment) are checked by the
comparisons that use them, and CRED's screen length is checked only by tests a fresh clone cannot run.

#### The verification commands, re-run

```
$ cargo xtask reference verify --tree reference
ok   check_a/part: 2 frame(s), 4 file(s)
… 24 captures …
verdict: ok — 24 capture(s), 105 frame file(s), 25792000 bytes
exit: 0

$ cargo xtask assets verify --tree --decode
decode: 151 asset(s) — 55 picture(s), 2 animation(s), 5 palette(s), 88 headerless with no decoder,
1 refused on record, 0 finding(s)
verdict: ok
exit: 0

$ cargo test --workspace
… 21 targets …
test result: ok. 590 passed; 0 failed; 3 ignored; 0 warnings; exit 0

$ cargo clippy --workspace -- -D warnings
exit: 101 at this state — five findings, and two more hidden behind them, see below
```

(That last line is the close-out's own re-run, at `0c78cd3`. The review made the gate an exit item
and the fix round fixed all of it: **both clippy commands exit 0 at the tip**.)

The 590 is the sum of the run's per-target `test result:` lines — 146 in `secondreality` (119 + 5 +
6 + 5 + 2 + 2 + 5 + 2), 161 in `sr-assets` (30 + 18 + 37 + 17 + 25 + 14 + 20, plus its empty lib
target), 104 in `sr-engine`, 179 in `xtask` (149 + 12 + 5 + 13), with two empty doc-test targets.
The three ignored are `sr-assets/tests/omf.rs`'s write-into-`assets/` regenerator and CRED's two
record tests. Zero rustc warnings, checked after forcing a recompile of every crate
(`cargo check --workspace --all-targets` on touched sources: 0 warnings, exit 0); the suite takes
about five minutes because several integration tests drive whole block-5 runs in the debug binary.

The asset tree is **151 assets plus `PROVENANCE.md`** (152 files): Phase 2 curated one more,
`CREDITS/FONA.INC` (Task 7), which is why the decode summary's headerless count is 88 where Phase 1's
was 87.

**`cargo clippy --workspace -- -D warnings` was not clean at the close-out, and the review made it
an exit item rather than polish.** The close-out measured five lints and stopped there, because
`sr-engine`'s failure stops the dependents from being checked at all — the true count was **seven
under the plan's command**, the five in `sr-engine` (`copper.rs:29`, `palette.rs:33`, `part.rs:79`'s
ten-argument `PartCtx::new`, `testcard.rs:92`, `tick.rs:25`: four `new_without_default`s and one
`too_many_arguments`) plus `secondreality/src/parts.rs`'s `type_complexity` and
`xtask/src/compare.rs`'s `manual_contains`, and **eleven more under `--all-targets`**, all in test
code (`sr-engine`'s `engine.rs:507`/`present.rs:278`, and nine in the parts' own test modules). All
eighteen are fixed in the fix round below — every one pre-existing, none a behaviour change: four
`impl Default` blocks beside the `new()`s they mirror, an `#[allow(clippy::too_many_arguments)]` on
`PartCtx::new` carrying the disjoint-borrow reason (the arity *is* the design: the constructor exists
because `&mut Engine` cannot be split), a `PartEntry` type alias for the registry's row,
`contains` for `iter().any()`, and eleven mechanical test-code reductions (two in `sr-engine`'s test
modules, nine in the parts'). Both commands exit **0** now, and the tests are re-run after them.

#### What could not be confirmed — the phase's own negatives

- **The human look: done, and it passed.** The plan's Task 10 Step 3 was
  `cargo run --release --bin secondreality -- --block 5` (and `--block 2`) watched by the user. **The
  user ran it and reports that it looks good** — the phase's only claim no test could reach, and
  deliberately left to a person, because Phase 0's single human look found a bug no test in the
  repository could see (the SDL texture-format misread, `03a3b11`). It is recorded as the user's own
  report and not interpreted: it says the picture is right, not that any particular frame was
  measured. What the tree reproduces mechanically stays named beside it — the status line's text and
  the key handling are unit tests in `window.rs`, the two hand-overs are the `--block 5` end-to-end test,
  and the rate is the headless run's own printed line
  (`cargo run --bin secondreality -- --block 5 --headless --run-seconds 60` printed
  `cred: 4205 ticks in 60.008 s = 70.074 Hz` in Task 10 — the window's title was read back with
  `xwininfo` in that task too, and that reading lives in its report,
  `.superpowers/sdd/02-shell/task-10-report.md`, which is git-ignored scratch). No command in the tree
  says what the picture *looks like* — that part is the user's, and it has now been given.
- **Music is deferred, not stubbed.** No audio is decoded, mixed or played in this phase; the one
  gate a faithful port would wait on (ENDLOGO's `dis_musplus()` loop) is its measured 567 ticks
  (**A7**), and Phase 3 replaces it with the real cue. Nothing else ported here touches music.
- **Nine and ten frames the port does not model, causes unidentified** — BEGLOGO's 42-tick lead-in
  (33 accounted for, **A5**) and ENDLOGO's 47 (**A7**). Both windows are between a 400-row
  Origin decode-and-blit and a palette fade, both candidates ruled out are ruled out *by evidence*
  (the loader cannot be it; `dis_partstart` is a version check costing no frame), and the frames
  where a difference would appear (436–444, 441–450) are not held by the reference.
- **Three parts diverge from the released demo, and the rule is D24's; the port is faithful to the
  tree.** TUNNELI's one expression (followed the release), CRED's data (**A10**, followed the tree)
  and ENDSCRL's build (**A11**, followed the tree). What each would take to *close* is named in its
  entry — the packed EXEs and the pack table are the leads.
- **TUNNELI's four keyframes are not a verdict** and its aligned residual (179/373/321/433 px) is
  the port's or the oracle's at 3800 and 4200 and unattributable at 4500 and 4600 (**A6**, per
  keyframe).
- **ENDSCRL's ring equivalence is the phase's largest modelling claim and the reference cannot test
  it end-to-end** (the reference is another build), so it rests on the unit test — a simulation of
  the source's own arithmetic against the port's — plus the reviewer's independent reconstruction
  (`endscrl.md` §2, §6.2).
- **The chain's 82 frames are bracketed, not pinned** (above), and the phase does not claim to know
  whether CRED's shipped build, the port's translation, or both produce them.
- **One CRED record test's couplings are `#[ignore]`d** (the wipe states against the scratch
  capture), disclosed in `cred.md` §3 and judged acceptable as disclosed by Task 8's review.

#### The whole-branch review, and its fix round (2026-10-13)

**No Critical, no behaviour defect, all three exit criteria met on evidence the reviewer re-derived
itself** — it rebuilt the binary at the tip, re-dumped and re-compared four parts, re-hashed every
frame, and flipped a byte in **both** planes of a copy of `reference verify`'s tree (exit 1, with the
file and both digests named). Thirteen findings — twelve in the record and one in a test — with no
behaviour defect; every headline number it re-checked reproduced to the digit (the parts'
percentages, the aligned sets, the block-5 boundaries, the chain's windows, the 163 control lines,
`FONA.UH`'s digest, 593 tests = 590 + 3 ignored, the two byte-identical sheets).

**The three Importants, all fixed.**

1. **"Seven characters" was six.** The count appeared in `endscrl.md` §6.2, A11, D24, this file (twice)
   and as a test's name, and the register contradicted itself four lines apart (the same paragraph
   said "`fonaw` is 0 for all six"). The packed EXE's appended run is the six bytes `5a 84 94 59 2f 26`
   — `Z \x84 \x94 Y / &` — so the shipped order is **80**, the text uses five of the six, and `0x94`
   is the one it never asks for. Corrected everywhere, and the test (`the_shipped_order_names_six_
   characters_the_trees_does_not`) now **asserts** the three counts it sits beside: `FONT_ORDER.len()`
   is 74, the appended run six, their sum 80, and five of the six appear in the shipped text.
2. **D24's step 3 had no tiebreaker.** Read literally, "a single expression or constant the release
   changed → port the release's value" covered TUNNELI's `div 5` (adopted) and ENDSCRL's `FONAY 25`
   (not adopted) equally. It now states the discriminator: **adopt only when the difference is one
   value the tree's algorithm can carry *and nothing else in that part differs*; register when the
   build differs in data, art, text or behaviour elsewhere** — and A11's closing paragraph records
   the pitch's deliberate non-adoption and the reason, so it does not read as a weaker measurement.
3. **Clippy was an exit item, not polish, and the count in the close-out was incomplete** (see the
   verification section above: seven under the plan's command — five of them had been hiding the
   other two — and eleven more under `--all-targets`). All eighteen fixed, both commands exit 0, and
   the tests re-run after them.

**The ten Minors, all fixed**: `cred.rs`'s screen-18 comparison now goes through a length-asserting
`differing_bytes` (the `zip` that cannot fail, in its fifth appearance); three stale counts
(`tunneli.md`'s "fifteen" is 17, `cred.md`'s "14 unit tests" is 20 — 18 unconditional and 2
`#[ignore]`d — and `beglogo.md`'s "the four" is five); this file's HEAD line; spec §4.2's sketch line
(`y_line_offset` is `Option<u16>` counting pixels, stale since Phase 0's review and parked onto this
pass, which had done the other two fields and not this one); `tunneli.md`'s §7.3 moved back under §7
where it belongs; A8's "A7's ruling" disambiguated as **Phase 1's task** A7; A10's "Unpacking it
yields" reduced to what is measured (the count is `INCLUDE.OBJ`'s 12 plus `INCLUD2.OBJ`'s 9 at a
16,784-byte stride, and the layout is those objects' blobs — nothing here has been unpacked); and the
human-look bullet's `xwininfo` reading replaced by what the tree reproduces, with the reading filed
to Task 10's report.

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
the four reference frames exercise 42 of the ramp's entries (38 with a non-black reference pixel,
5,275 informative pixels, 29 distinct greys) and every one matches, exactly — including the 17 where
truncation and rounding differ; and a search over 81 candidate `(mul, div)` pairs — 70 distinct
ramps — rendered against the four reference frames puts the port's colour field 1,108 pixels from
them at 3/5 and at least 4,200 at every other (the source's own 3/4 is 6,376). Of the 1,108, 1,091
are ink presence and 17 are dot overlap (10 of them on first-ramp entries). The port takes the shipped ramp —
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
3800 is 12, against ~235 at the other three, where a port dot meets the reference's black; 17 pixels
over the four frames are inked by both sides in different colours, 10 of them on first-ramp entries
— dot overlap, not the ramp). The
residual is attributed honestly rather than excused by the oracle: at 3800 the reference is
**settled** (7 of 9 sessions, and the re-capture reproduced it byte for byte), so 179 pixels there
are the port's or a sub-frame effect in it; at 4200 the census is exactly half; and at 4500 and 4600
— **no settled content** — nothing can attribute them. `APPROXIMATIONS.md` A6 carries the table and
what is ruled out (the ramp; a whole-tick error — 754 and 953 pixels at ticks 77 and 79 against
179 at 78; and the pre-fix
partners' 8.07-10.18 %, which §8.4 itself reframes as "≈1 frame … ≈4 frames" — the history of a
reference that has been replaced).

**The unchecked read past `rows` (`:179-190`) is preserved because the source has it, and it is
invisible.** The path that *is* visible is substituting 0, the value of `aa2`'s unwritten half: the
run takes that branch 356,042 times and every one of those dots would land in row 0, where the
reference's four frames ink 34 pixels in total. Every y the part's own run puts outside `rows` reads a word of 64,000 — `aa2`'s copy of
`aa1` (`:99-100`) on one side, `aa1` past the wrap on the other — so the dot plots at `x + 64,000`,
past the 64,000 bytes a 200-line mode 13h screen reads: more than 100,000 such reads in a full run,
**not one** of them on screen, and a bounded read gives byte-identical frames. The path that *is*
visible is substituting 0 (the value of `aa2`'s unwritten half): that plots at row 0 — the branch is
only entered when the x sum is ≤ 319, so all 356,042 of them go there — which is why the read is
modelled rather than zeroed.

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

#### Task 6 — ENDLOGO: the end picture, and the first part the shell's own mode belongs to

`bins/secondreality/src/parts/endlogo.rs`, registered as `endlogo`; `END/END.C` translated act for
act over `END/ASM.ASM`'s `lineblit` and `setpalarea` and `END/READP.C`, with the picture read as
`assets/END/PIC.UH` through `sr_assets::load_path`. **Ten unit tests** beside the part and five
through the real binary.

**The comparison is exit 0.** The two commands, verbatim:

```
$ cargo run --bin secondreality -- --headless --part endlogo --dump-frames 430,500,900,1180 \
      --dump-dir out/p2-endlogo
$ cargo xtask compare --reference reference/parts/endlogo --rendered out/p2-endlogo \
      --out out/p2-endlogo-diff
frame 430: 0.0000% differing (0/128000 px), max delta 0
frame 500: 0.0000% differing (0/128000 px), max delta 0
frame 900: 0.0000% differing (0/128000 px), max delta 0
frame 1180: 0.0000% differing (0/128000 px), max delta 0
verdict: identical
exit: 0
```

`mode: mixed`, offsets `{"0": 4}`. 430 and 900 go through the **indexed** path (the reference files
an `.idx` for both) and 500 and 1180 through RGB, so the DAC widening is load-bearing on the two
fade frames and bypassed on the two held ones — the same reading BEGLOGO's note predicted for the
same mechanism.

**The two fade constants are derived, not fitted, and the plan's brackets were 1 and 2 frames out.**
`the_fade_constants_are_derived_by_matching_the_reference_bit_for_bit` runs each ramp over its whole
range — the source's arithmetic, the engine's `(c*259+33)>>6` widening, all 320×400 pixels — and asks
which `c` reproduces the reference's frame **exactly**: 500 is the fade-in at `c = 49` uniquely, 1180
the fadeout at `c = 30` uniquely, so the fade-in's step 0 is displayed at **451** and the fadeout's
`c = 63` at **1147**. The plan's Step 1 brackets ("near 450", "near 1145", both from a fit over
§19.1's sweep means) are 450 and 1145, and the mutation check shows the test failing on each with
`the fade-in's step 0 is displayed at 451 / left: 451 / right: 450`. The two facts the plan asked to
be checked against both hold: the settled picture's first frame is 451 + 129 = **580**, which is the
first byte-identical sample in §19.1's own sweep, so the hold is 1147 − 580 = **567** frames; and the
64-step fadeout ends at **1210**, inside the (1200, 1240] window where `SECOND 5` changes geometry to
CRED. The derivation reproduces every state §19.1's table names separately — colour count and mean, to
two decimals, for 430, 460, 500, 540, 580, 900, 1120, 1160 and 1200 — which is a sweep recorded
before this task and a prediction of the two constants rather than a fit to them.

**The brief's byte count for `setpalarea` is wrong, and the correction is the source's.**
`END.C:105`/`:116` pass **255** where `BEG/BEG.C:84` passes 254, and `END/ASM.ASM:107-108`'s
`2*count + start` then moves **510** bytes for this part, not 508 — entries 0-169 whole, entries
170-255 untouched. The brief and the plan's Step 3 both said 508, which is BEGLOGO's number; both
call sites of this part's source read `setpalarea(pal2,0,255)`. The port writes 510 and
`the_upload_is_the_originals_two_count_plus_start_bytes` pins the entry-169/170 boundary. It is
invisible on this art either way — the picture indexes 0-71, and the scan returns the same `c` with
254 — which is why `bugs.md` §4 is unchanged and the number is in this part's note (§6).

**The music gate is a measured length, in the register.** `END.C:107-111` waits on
`dis_musplus() > -16`, which no part of this phase can evaluate, so the hold is its measured 567
ticks — `APPROXIMATIONS.md` **A7**, which carries the measurement, "zero against this reference and
unknown against any other music position or machine speed", and Phase 3's replacement (the real cue,
sampled once a tick, with `END.C:110`'s threshold). A7 also registers the **ten lead-in ticks the
port does not model** — the source's blanks and acts account for 37 of the 47 — the same class as
A5's nine for BEGLOGO, with the same two candidates ruled out (not the EXE load: the part's own
white is up by 415, `inittwk`'s palette clear being commented out at `END/ASM.ASM:14-20`; not
`dis_partstart()`, a version check).

**Three traps the part does not fall into, each pinned by a test.** It does not set its own video
mode (`END.C:64-66`'s is commented out; the shell's `MAIN/U2.ASM:906-908` did it, and `inittwk`'s
byte-mode CRTC write has no analogue in a chunky model — note §2); **it does not clear the surface**,
which is why 430's `.idx` is index 0 in all 128,000 pixels and the indexed comparison there passes;
and **it does not write DAC entry 255** — the 765 bytes of `:62-63` fill entries 0-254 and stop one
entry short, the opposite of BEGLOGO's black entry 255, and both screens are the one `sha256`. The
hold's 567 ticks rest on the picture itself: the blit lands on tick 36 (**index 440**), so 430 is
inside the 32 blanks and video memory there is the shell's zeroed surface.

**The sheet, and the look.** `cargo xtask frames sheet --dir out/p2-endlogo --out
out/p2-endlogo-sheet.png` (4 tiles, 2 columns, 650×810) sits beside the reference's own
`out/endlogo.png`, and **the two PNGs are byte-identical** — `sha256sum` gives `4a3afb5c…` for both,
281,156 bytes each, from two different tools a day apart — which is what four byte-identical frames
and a shared sheet layout have to produce. **The human look is deferred to the phase boundary**:
nothing here claims a person has judged the picture.

`cargo test --workspace`: **536 passed, 0 failed, 1 ignored, 0 warnings** (was 521; +10 unit and +5
integration). Mutation checks — seven of them, each reddening the test that pins it, the two bracket
mutations taking the sweep, hold and blit tests as well — are in the note's §8: the two plan brackets,
the white over all 256 entries, `setpalarea` with the intended `3*count`, the blit moved onto the
fade's first tick, the fadeout's first step taken at the picture instead of one step below it, and the
lead-in one blank short.

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

#### Task 9 — ENDSCRL: the ring, and the shipped build the reference holds

`bins/secondreality/src/parts/endscrl.rs`, registered as `endscrl`; `ENDSCRL/MAIN.C` translated act
for act over `ENDSCRL/ASMYT.ASM`'s `setstart`/`setrgbpalette`, with the strip read as
`assets/ENDSCRL/FONA.UH` through `sr_assets::font::Font` (which **accepts** this one: its cell is
`(20, 30)` and `1500/20 = 75` — unlike CRED's, A8) and the text as `assets/MAIN/DATA/ENDSCROL.TXT`,
the shipped 6,141 bytes and not the 1,007-byte file beside the source. **Ten unit tests** beside the
part and four through the real binary.

**The ring is 400 lines, not 401, and the beam's wrap is the second copy.** The port's surface is
400 × 640 = 256,000 = the whole ring, and `View::source_index` is modulo it, so the original's
`memcpy` to `80*(yscrl+401)` has no address and each line is written once; the display origin is
`(yscrl + 1) * 640`, up to **256,000** — 124,930 past what the `u16` pair could sum to, which is
Task 3's widening. The equivalence is not argued: `the_four_hundred_line_ring_shows_what_the_four_
hundred_and_one_line_ring_shows` runs the original's own arithmetic — a 401-line ring written twice,
start `yscrl*80 + 80` — beside the port's at every iteration of a 500-line run, comparing all 400
displayed rows as bytes.

**The comparison is exit 1, and it is a build difference, not an offset.** The two commands,
verbatim:

```
$ cargo run --bin secondreality -- --block 5 --headless --dump-frames 9100,10000,12000,14000 \
      --dump-dir out/p2-endscrl
$ cargo xtask compare --reference reference/parts/endscrl --rendered out/p2-endscrl \
      --out out/p2-endscrl-diff
frame 9100: 2.8473% differing (7289/256000 px), max delta 243
frame 10000: 11.0676% differing (28333/256000 px), max delta 243
frame 12000: 9.9617% differing (25502/256000 px), max delta 243
frame 14000: 6.6828% differing (17108/256000 px), max delta 243
verdict: different
exit: 1
```

**Two residuals, measured separately.** (1) **The chain's 82 frames**: the port's ENDSCRL tick 0 is
index **8835** (measured through the binary — CRED's last frame at 8834, the geometry change at
8837) and the reference's scroller begins at **8917 ±1**, measured from the one thing both builds
agree on (the *rows* the text's first line is drawn on) — inside CRED's own bracket of 6 to 105
frames early (`cred.md` §7), so it is the chain's. `--align 82` on a dump of the shifted indices
shows the same shift from the other side: 9100 2.8473 % → 2.1758 %, and 10000 *worse* (11.0676 % →
12.5437 %), which is the second residual. (2) **The shipped build**, five differences, the first of
which is the dominant one:

- **The `[NN` control lines**: the tree's `MAIN.C` has no rule for them and draws the digits; the
  reference advances the ring by **the number in the marker** and draws nothing, and every other
  line advances it 25. The shipped text holds **163** of them (`[8` ×6, `[10` ×14, `[12` ×135,
  `[15` ×8), and the rule *predicts every band* of frames 10000 and 14000 from this tree's own
  file: frame 10000's lines 16, 17, 19, 20, 22 sit at ring rows **0, 25, 65, 90, 130** (steps of
  **40 = 25 + 15** across each `[15`), and frame 14000 shows **six** `[12` steps of **37 = 25 + 12**.
  The `[10`/`[8` steps are not visible in any of the four frames — recorded as **unmeasured**.
- **The pitch**: **25 rows a line where `MAIN.C:6` says `FONAY 30`** (frame 12000's twelve whole
  lines: every gap a multiple of 25, the smallest 25).
- **The shipped `fonaorder` names six characters the tree's does not** (`Z Y / & \x84 \x94`; the
  packed `MAIN/DATA/ENDSCRL.EXE` carries the tree's tail plus those six — the appended run is the six
  bytes `5a 84 94 59 2f 26` in `xxd -s 9286 -l 40` — so its order is 80, not 81), and the text uses
  five of them: on frame 14000 the lines naming one are **14–15 columns wider** than the port's
  rendering, the lines naming none within 1. *(The count was recorded as seven at Task 9 and
  corrected at the close-out: the note's own dump shows six, and its own next sentence says `fonaw`
  is 0 "for all six".)*
- **The raster** differs from `FONA.UH` in **53 of the header band's 11,520 pixels** (geometry
  identical: the same 13 ink-column runs).
- **The text has no carriage returns** (the centring is one column further left than the tree's CRLF
  file gives); the mechanism is unestablished.

The first two drift, which is why no alignment reaches zero. Registered as **A11**; the reasoning and
the numbers are `docs/fidelity/endscrl.md` §6.

**The sheets, and the look.** `cargo xtask frames sheet --dir out/p2-endscrl --out
out/p2-endscrl-sheet.png` sits beside the reference's own sheet. **The human look is deferred to the
phase boundary**: nothing here claims a person has judged the frames.

### Phase 3 — Task 8: the gate is the source's own loop, and the phase's proof (2026-10-15)

**The task.** ENDLOGO was the one part that waited on music (`END/END.C:107-111`'s
`dis_musplus() > -16` loop), and Phase 2 replaced it with its measured length — 567 ticks, registered
as **A7**. Task 8 retired the stand-in: the part now runs the source's loop against the run's player
(one iteration a tick, the 5000-iteration cap kept), and A7's number survives as the *check*.

**The check, and the one frame.** The cue's own answer and A7's derived 567 came out one frame apart,
and the reconciliation is in `docs/fidelity/endlogo.md` §5 with the constants beside the code
(`GATE_ALIGNMENT_TICKS`, `bins/secondreality/src/parts/endlogo.rs`) and the decision in **D28**:

- **The part's read is one tick behind the copper's poll** (§5.1): the source's check stands at the
  frame boundary (`DIS/DISINT.ASM:160-180`'s wait on `copperframecount`, `MAIN/COPPER.ASM:320`) and
  the poll is a fifth of a frame later (`_zpollme`, `:466`, in `copper_doit`, the list's 20-of-100
  entry), while the port's engine polls before the part's code in the same tick — so the check tests
  the *previous* tick's words.
- **One frame stays, and it is an alignment** (§5.2): the port's own player publishes `26:49` from
  index **1144**, so the break would fall on 1145, and the reference's *frames* — bit-exactly, from
  500 and 1180 — put the fadeout's first step at **1147** (the break at 1146). The port holds one
  frame past its own break. **Which measurement moved is named**: the port's player is `second5`'s to
  the frame (its `zframe` matches the trace's column; its published positions are the trace's samples
  one index on, which is what the trace's sample instant makes of them), and `second5`'s published
  row 49 begins one frame earlier, relative to the frame indices, than the session the reference's
  keyframes came from — D17's per-session counter alignment. The cost is stated where the constant
  is: one frame's disagreement with `second5`'s own session at this crossing, inside A12's ±3.

**The acceptance, re-run with the real cue** (`cargo xtask compare`, both commands the phase names):

```
--part endlogo   frame 430: 0.0000% differing (0/128000 px), max delta 0
                 frame 500: 0.0000% differing (0/128000 px), max delta 0
                 frame 900: 0.0000% differing (0/128000 px), max delta 0
                 frame 1180: 0.0000% differing (0/128000 px), max delta 0
                 verdict: identical      exit: 0
--block 5        the same four frames, the same numbers, verdict: identical, exit: 0
```

**The phase's proof, in the two directions it can be stated.** The port's cue, driven by
`reference/music/second5`'s own words a tick at a time, opens the gate on the tick Phase 2 derived
from the reference's frames (`the_gate_opens_on_the_tick_the_reference_frames_derived`); and the same
part, driven through the shell by the run's own player with the real asset, draws §3's held picture
at 1146 and the fadeout's `c = 63` at 1147 (`the_runs_own_player_opens_the_gate_on_the_same_tick`).
The readings the task asked for beside them:

| reading | where |
|---|---|
| the trace's `musplus` either side of the crossing | frame 1144 `zplus` 1 row 48 → **−16** (not `> -16`); frame 1145 row 49 → **−15** (the crossing); the trace shows row 48 over 1136-1144 and row 49 from 1145 |
| the port's own `(order, row)` at the hold's two ends | index 580 → **25:18**, index 1146 → **26:49** (`out/t8-ends/manifest.json`) — the trace's own values at those frames |

**A7 is retired, not deleted**: `APPROXIMATIONS.md`'s entry is struck through in place, with the cue
named as what replaced it, the measurement kept as the check, and the ten unmodelled lead-in frames
(its second half) left standing. **The human listen is the user's and is not claimed here** — see the
task report's paragraph.

## Phase log

### Phase 2 — Shell + easy tier (done, tag `phase-2`)

Eleven tasks on branch `phase-2-shell`, from `5cf3e88` to `0c78cd3`. **590 tests pass, zero
failures, three ignored, zero warnings** (Phase 1 closed at 405, one ignored). Every comparison in
the phase was re-run at the final commit; the numbers are in the evidence section above.

**What was built.** `shell::Sequence` — one run, one frame counter in the **reference's own
boot-index space**, so a diff needs no translation (D21) — with block entry (`--block 1..5,U`), a
part registry, and the original's own keys (ESC ends the part and the run goes on, Ctrl skips one;
D21). Five parts, each translated from its own source with a fidelity note in the same commit:
**BEGLOGO** (the white, the 129-step fade, the blit, the preserved `setpalarea` byte-count bug),
**TUNNELI** (77 rings, one plot per dot per frame, the source's own unchecked read), **ENDLOGO** (the
end picture, two 129/64-step ramps, the music gate as its measured length), **CRED** (21 screens, the
font the loader refuses, the iris's three registers, the first pan a measurement corrected) and
**ENDSCRL** (a 400-line ring, the beam's wrap standing in for the second copy, the shipped
greetings text). `sr-engine` took three changes, each recorded in D21/D16: the shell's DAC needed
`Engine::palette_mut()`, the view origin widened to `u32` (D16's open item, closed), and the exit flag
gained a door from outside (`Engine::request_exit`). Two tools joined `xtask`: `compare --align <N>`
and `frames sheet --dir`.

**The exit criteria.** BEGLOGO is frame-diff-identical — `compare` exits 0 on all four keyframes and
every file the reference carries (RGB at all four, an index plane at 430 and 900) is byte-identical
to it, with every digest equal to the value Phase 1 recorded by hand. ENDLOGO is the same. TUNNELI,
ENDSCRL, CRED and ENDLOGO each run their own
effect to their own end through the real binary. `--block 5` plays ENDLOGO → CRED → ENDSCRL in a
window and a test asserts both hand-overs on consecutive reference indices (1211/1212, 8834/8835).

**What the phase learned about its own source, and it is the phase's biggest finding.** The vendored
tree is a **development snapshot, not the released build**, in three independent ways: TUNNELI's
second palette ramp is one expression different (and the tree's own `.PAS` and `.EXE` agree with each
other and disagree with the release), CRED's shipped credits are different *data* (captions the
source cannot draw), and ENDSCRL's shipped scroller is a different build again (a 25-row pitch where
the source says 30, control lines the source has no notion of, six more characters in its font
order, and no carriage returns). D12's rule already decided which artifact wins — the release, because
the release is what the reference frames measure — and **D24 now records the procedure** for the
next phase that meets it: measure what the release does, check whether the tree can produce it at
all, then decide and register the divergence with its evidence and a lead.

**And the phase's method is now written down too.** Four review rounds across Tasks 4 and 5 were
spent not on code but on records claiming more than their measurements supported — an absolute over a
space nobody measured, a count that did not reproduce, a proof for something unobservable, an unknown
dressed as a known — and the same shapes recurred in the reviews of Tasks 6 to 10. **D25** is the rule
those rounds produced: every number in a note is reproducible by a command it names or produced by a
test in the same commit; absolutes need a measurement over their whole space; an unobservable claim
is stated as one; measured constants are derived by exhaustive match required to be unique, never by
a fit; and nothing is re-anchored to make a diff pass.

**What it could not confirm, in one place.** The window has since been watched: the user ran
`--block 5` and reports that it looks good (the human look, recorded under the negatives above — the
one item on this list that is now closed). No music is decoded or played, and ENDLOGO's hold is a measured
frame standing in for a cue (A7). Nine and ten frames of the two parts' lead-ins are not modelled and
their cause is unidentified (A5, A7). Three parts' references are other builds (A6, A10, A11).
TUNNELI's residual is the oracle's or the port's and cannot be resolved further with what is
captured. `cargo clippy --workspace -- -D warnings` was dirty at the close-out — the review made it
an exit item, and the fix round below closes it (seven findings under the plan's command, eleven more
under `--all-targets`, all pre-existing and all fixed without a behaviour change; both commands exit
0).

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

