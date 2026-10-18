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
| 3 | Music engine | **done** — all four exit criteria met: both modules play end-to-end, the `(order, row)` cue matches a per-frame trace of the original's own player state, `_np_zinfo` is deferred with D8's record and the substitute cue named, and `dis_getmframe`'s rate is measured. ENDLOGO's gate is the source's own loop, its four keyframes exit 0 / 0.0000 % / both planes byte-identical under both `--block 5` and `--part endlogo`, and the suite holds (802 tests at the close-out, 806 after the final review's fix wave). Evidence below. The final whole-branch review ran and its findings are fixed and re-reviewed. The user has listened via `--audio-dump` and reports the music sounds right; their real-device skips are the VM's audio path (the same WAV is perfect on the host), not the port | [03-music.md](docs/plans/03-music.md) | `phase-3` |
| 4 | Music-synced tier A | **done** — spec §10's criterion met: ALKU, PAM, DDSTARS, DOTS (`minvball`), MNTSCRL and RAYSCRL verified against their reference keyframes (17 of the 24 reference keyframes byte-identical; the seven residuals are A19's four coincident-dot frames, A20's text phase at ALKU 5859, and the release's `MIEKKA.SCI` at RAYSCRL 14000/14500 — PAM's 7900 index plane is an additional registered comparison, A21, not an eighth keyframe); the five parts that wait on the music do so as their originals do, with the release's timings in A17/A18, and DDSTARS has no music cue (the shell starts its MUSIC0 recipe). Block 1 stops at U2A and block 3 at LNS&ZOOM; block U plays DDSTARS; PAM, MINVBALL and RAYSCRL run with `--part`. Evidence below. The final whole-branch review ran and its findings are fixed and re-reviewed; **the tag is applied at this commit** | [04-music-synced-tier-a.md](docs/plans/04-music-synced-tier-a.md) | `phase-4` |
| 5 | Mid tier | not started | — | — |
| 6 | Vector engine | not started | — | — |
| 7 | GLENZ | not started | — | — |
| 8 | Vault | not started | — | — |
| 9 | Assembly & fidelity pass | not started | — | — |

## Where to pick up

**Phase 4 (music-synced tier A) is done, tagged `phase-4` at this commit** on
`phase-4-music-synced-tier-a`. Its exit criterion (spec §10 — ALKU, PAM, DDSTARS, DOTS,
MNTSCRL, RAYSCRL verified) is met, every criterion was re-run at the committed state, and the
evidence is recorded below. Its plan is `docs/plans/04-music-synced-tier-a.md`, its per-task ledger
(git-ignored scratch) is `.superpowers/sdd/04-music-synced-tier-a/progress.md`, its six notes are
`docs/fidelity/{alku,pam,ddstars,minvball,mntscrl,rayscrl}.md`, and its decisions are **D30** (with
D24, D25 and D29 binding). **The final whole-branch review ran** (see its record below): its
findings are applied in `f18823f` and the scoped re-review is clean, so the tag goes on here, per
Phase 1/2/3's practice. The branch starts at `phase-3`
(`3d42b88`): its first commit is Phase 3's human-listen record, its second the Phase 4 plan, and
the rest the phase's ten tasks and their fix rounds.

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

**Phase 3 (music) is done on `phase-3-music`, and tagged `phase-3`** — its exit criteria are met,
its evidence is recorded below, and the final whole-branch review ran and its findings were fixed
and re-reviewed (the plan's Task 9 Step 5; Phase 1's and Phase 2's practice). Its plan is
`docs/plans/03-music.md`, its per-task ledger (git-ignored scratch) is
`.superpowers/sdd/03-music/progress.md`, its two notes are `docs/fidelity/music.md` and
`docs/fidelity/music-clock.md`, and its decisions are **D26**, **D27**, **D28** and **D29**. The
phase's own four exit criteria (spec §10) and how each is evidenced:

1. **Both modules play end-to-end** — `cargo test -p sr-audio --test player -- --nocapture
   each_module_plays`, MUSIC0 wrapping after 8,732 polls and MUSIC1 after 28,618, both asserted to
   read no table outside their own orders.
2. **The `(order, row)` cue timeline matches the parts' expectations** — the phase's oracle is a
   per-frame trace of the *original's own player state* (`reference/music/`, four sessions, a new
   `kind` the verifier refuses to walk past), the port is driven the same number of polls and must
   agree with it frame by frame (86.2–93.4 %, the residual being the oracle's own jitter, **A12**),
   and the phase's proof is ENDLOGO, whose gate is now `END.C:107-111`'s own loop with its four
   keyframes at exit 0.
3. **`_np_zinfo` deferred with a documented workaround** — D8's route 1 was performed, the word is
   **0 in every sample of every filed trace** (11,408), and D8 now names the substitute: the port
   computes `_np_zplus` by the rule in `music-clock.md` §6 and returns `_np_zinfo`'s measured 0.
4. **`dis_getmframe`'s rate measured** — `_np_zframe` advances 0.6839 a frame (47.93 a second) at
   tempo 120 and 0.7422 (52.01) at tempo 130, each the tempo's own `2.5/tempo`, over the longest run
   that never goes backwards.

**What Phase 3 leaves Phase 4, in one place.** Phase 4 (music-synced tier A) is the first phase
whose parts *wait on* the cue, so the five below are its inheritance:

- **The cue API, and the one-tick relationship it carries.** `PartCtx::music()` returns the run's
  player behind `sr-engine`'s `Music` trait (six calls: `musplus`, `musrow`, `muscode`, `sync`,
  `getmframe`, `setmframe`), and `sr-engine` knows nothing about Scream Tracker — the dependency
  guard is a test. The engine's poll sits between copper slot 0 and the first band, so a part that
  reads the music as the first act of its frame reads the words the tick's *own* poll published,
  where the original's check stands at the frame boundary and reads the previous frame's. **D28**'s
  placement is not ENDLOGO's rule but the engine's showing through, and every later part that gates
  on the music has the same one-tick relationship to work out (`docs/fidelity/endlogo.md` §5.1).
- **`dis_sync()` has a boundary at `0x8E2E`.** `sync_from` answers the position `(0, 0)` with the
  object's own word (−15609, `musframe_9`'s `pop es; ret`), and returns the table's own last code
  **4** for `0x4200 < key <= 0x8E2E` — the 16th compare matches the source's own `2e 8e`, and the
  answer is the same in every build. It **refuses** only above `0x8E2E`, where the word is
  linker-set and differs between builds. Both callers the demo has are Phase 4's —
  `PAM/OUTTAA.C:39` and `ALKU/MAIN.C:59`, `:66`, `:72`, `:79`, `:138` — and the phase **cannot
  show they stay inside `key <= 0x8E2E`**: how far MUSIC0's order list has got when they run is not
  measured until they are ported. **A15** registers the refusal; the answered band is
  reachability-measured (`hiddenu` holds it for 5,148 of its 6,351 frames, 81.1 %).
- **`zinfo` is 0, and a part that waits on it waits for ever — as it does against the reference.**
  `dis_muscode` returns the measured 0, so HARD's `HARD/B.C:138` (waiting on 1) and GRID's
  `GRID/MAIN.C:72`/`:160` (on 2 or 3) must be written against that measurement rather than against
  an inferred cue; the values they wait on were produced by no window the phase measured (D8's
  amendment).
- **`GATE_ALIGNMENT_TICKS` is a fact about this boot and `second5`, not about ENDLOGO.** It exists
  because the reference's *frames* (D12's record) put the fadeout's first step one frame past where
  the port's own words do, and a run whose music starts elsewhere has no reference frame to align
  to. A later part that gates on the music must make its own decision and must not inherit this
  constant; the part that does gate carries **no hold length** to inherit either.
- **And the mid-run restart the port's model has no place for.** Two of D13's restarts are not
  entry-point-only: `MAIN/U2.ASM:880-885` (MUSIC0 at order 18) is guarded by `whattorun & 8` alone,
  so blocks 2 and 3 reach it *mid-run* (after JPLOGO, before U2E), and `:813-818` (MUSIC1 at order 0)
  fires for `SECOND 1` alone, in the middle of that block. Nothing the port can run today diverges
  (block 2 stops at `glenz`, block 5's mask has bit 3 clear), but **a run reaching U2E inside blocks
  2 or 3 needs a mid-run restart** the one-row-per-run model does not have. `shell::MUSIC_RESTARTS`
  and `docs/fidelity/music.md` §12 state it per row.

The phase also retires Phase 2's last music stand-in: **A7 is retired** (struck through in place,
the cue named as what replaced it, the measurement kept as the check, and its second half — ten
unmodelled ENDLOGO lead-in frames — left standing), and no other ported part polls music
(BEGLOGO's build has no gate — `docs/fidelity/beglogo.md` §1).

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
  frame boundary (`DIS/DISINT.ASM:161-181`'s wait on `copperframecount`, `MAIN/COPPER.ASM:305-311`'s vsync loop,
  `:320`'s increment) and
  the poll is a fifth of a frame later (`_zpollme`, `:466`, in `copper_doit`, the list's 20-of-100
  entry), while the port's engine polls before the part's code in the same tick — so the check tests
  the *previous* tick's words.
- **One frame stays, and it is an alignment** (§5.2): the port's own player publishes `26:49` from
  index **1144**, so the break would fall on 1145, and the reference's *frames* — bit-exactly, from
  500 and 1180 — put the fadeout's first step at **1147** (the break at 1146). The port holds one
  frame past its own break. **Which measurement moved is named**: not the player — its
  `_np_zframe` matches the trace's own column once the sample instant's one-index shift is taken out
  (**753 of 891 frames exact, worst 1 tick**, asserted frame by frame in `src/shell.rs`'s
  `the_runs_music_is_where_the_trace_holds_it_at_every_frame_the_trace_has`, `ZFRAME_SLOP`), and its
  published positions are the trace's samples one index on — but `second5`'s published row 49 begins
  one frame earlier, relative to the frame indices, than the session the reference's keyframes came
  from: D17's per-session counter alignment. The cost is stated where the constant
  is: one frame's disagreement with `second5`'s own session at this crossing, inside A12's ±3.

**The acceptance, re-run with the real cue** (`cargo xtask compare`, both commands the phase names):

```
--part endlogo   frame 430: 0.0000% differing (0/128000 px), max delta 0
                 frame 500: 0.0000% differing (0/128000 px), max delta 0
                 frame 900: 0.0000% differing (0/128000 px), max delta 0
                 frame 1180: 0.0000% differing (0/128000 px), max delta 0
                 (…)
                 verdict: identical      exit: 0
--block 5        the same four frames, the same numbers, the same report lines, verdict: identical, exit: 0
```

(`(…)` marks the tool's `report: <path>` and `captures: 1 measured` lines, and `exit: 0` is the
shell's reading of the command's exit code.)

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

### Phase 3's evidence (close-out, Task 9, 2026-10-15)

**Exit criteria (spec §10): both modules play end-to-end; the `(order, row)` cue timeline matches the
parts' expectations; `_np_zinfo` resolved or consciously deferred with a documented workaround;
`dis_getmframe`'s rate measured.** All four are met. Every number below was **re-run at the
committed state** (`b5e6f25`, branch `phase-3-music`) in one pass, each dump into a fresh output
directory, and the blocks are pastes of those runs: `(…)` marks an elision, and an `exit:` line is
the shell's reading of a command's exit code rather than a line of its output.

| criterion | how it is evidenced | the number |
|---|---|---|
| both modules play end-to-end | the end-of-song report (below) | MUSIC0 **8,732** polls / 5,983 ticks / 998 rows; MUSIC1 **28,618** / 21,240 / 5,283 |
| the cue matches the parts' expectations | the trace comparison, the shell's own music test, and ENDLOGO's gate (below) | 86.2 – 93.4 % of frames exact, worst 3; ENDLOGO **exit 0, 0.0000 %** |
| `_np_zinfo` deferred, workaround named | D8's 2026-10-14 amendment; `music analyse` §5 per trace | **0** in all **11,408** samples; the port returns it, and computes `zplus` |
| `dis_getmframe`'s rate measured | the traces' `_np_zframe` column over the longest un-reset run | **0.6839/frame** (47.93 a second) at tempo 120, **0.7422** (52.01) at tempo 130 |

**Both modules play to the end of their order lists** — `cargo test -p sr-audio --test player --
--nocapture each_module_plays`:

```
  MUSIC0: wrapped after 8732 polls (124.6 seconds of logic ticks) at order 0 pattern 0 row 0, loop counter 1
  MUSIC0: 8732 poll(s), 5983 tick(s) over 998 row(s)
  MUSIC0: 80 order(s), 77 pattern(s), ended at order 0 (1 loops)
  MUSIC1: wrapped after 28618 polls (408.3 seconds of logic ticks) at order 0 pattern 73 row 0, loop counter 1
  MUSIC1: 28618 poll(s), 21240 tick(s) over 5283 row(s)
  MUSIC1: 100 order(s), 77 pattern(s), ended at order 0 (1 loops)
```

(The second line of each pair is the same run counted a second way. `playing_past_the_end_neither_
panics_nor_runs_off_the_orders` then plays each module a further 100,000 polls and asserts that every
published order, pattern, row, speed and tempo stays inside the module's own tables — a player that
loops for ever is fine, one that reads past its orders is not.)

**The trace comparison, and the oracle it is made of.** Phase 3 has no audio reference — the
emulator runs silent (D17) — so its oracle is a **per-frame trace of the original's own player
state**: `STMIK.300`'s `PUBDEF` records give the position words' offsets, the segment is located in
guest memory, and one request a frame reads them into `reference/music/*/trace.csv` (four sessions,
a new `"kind": "music-trace"` that `reference verify` checks rather than walks past). The port's
player is driven for the same polls and must agree frame by frame.
`cargo test -p sr-audio --test trace -- --nocapture`, the run's own span, verbatim apart from
cargo's two preamble lines (the leading `(…)`), **including** the harness's five `test … ok` status
lines. What is stable in this output is the order **within one test**: the harness interleaves its
status lines as tests finish, and the tests run in parallel, so a fresh run may print the two-session
block before `4 filed trace(s)` where this one prints it after — four runs of this command gave
`4 filed trace(s)` first **twice** and the two-session block first **twice**. The block is honest
line by line; it is not a claim about the transcript's global order:

```
(…)
running 5 tests
test the_tick_counter_advances_at_the_tempos_rate ... ok
test zplus_is_a_function_of_the_published_position ... ok
4 filed trace(s)
second5 against second5b: 131 row change(s) each, 8 of them on a different frame:
  ord 25 row 18: frame 575 against 576
  ord 25 row 24: frame 629 against 628
  ord 25 row 53: frame 883 against 882
  ord 25 row 61: frame 953 against 952
  ord 26 row 6: frame 988 against 987
  ord 26 row 8: frame 991 against 990
  ord 26 row 19: frame 1007 against 1006
  ord 26 row 56: frame 1207 against 1206
test the_two_sessions_of_one_recipe_disagree_at_eight_row_changes ... ok
test zinfo_is_zero_in_every_filed_trace ... ok
  hiddenu (MUSIC0): 5687 of 6088 frames agree (93.413 %); worst displacement 3 frame(s)
  second3 (MUSIC1): 2144 of 2488 frames agree (86.174 %); worst displacement 2 frame(s)
  second5 (MUSIC0): 771 of 887 frames agree (86.922 %); worst displacement 2 frame(s)
  second5b (MUSIC0): 777 of 887 frames agree (87.599 %); worst displacement 2 frame(s)
test the_port_reproduces_every_filed_trace ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 8.42s
```

**The two-session block at the top is the phase's crux, measured rather than argued.** `second5` and
`second5b` are *one recipe in two sessions*, and they disagree with **each other** about which frame
eight of their 131 row changes land on, one frame each. So no player can be frame-identical to both,
the brief's literal "byte-for-byte agreement over the whole window" is unsatisfiable, and the
acceptance asserts the strongest thing that is true: the *content* (exactly the trace's sequence of
`(order, row)`, `(speed, tempo)`, pattern, loop and `zplus` values), the *placement* (every value one
the trace publishes within the measured spread), and the percentage with its worst displacement. The
residual is **A12**.

`cargo test -p secondreality --bin secondreality music -- --nocapture`. The four lines below are that
test's **whole stdout, in order, verbatim**; the `test … ok` lines around them in the transcript are
cargo's, and this test is the only one of the five that prints:

```
second5: 778 of 894 frames agree exactly (87.025 %); worst displacement 2 frame(s)
second5: 130 position change(s) compared; the port's frame minus the trace's: {-2: 8, -1: 100, 0: 22}
second5: 753 of 891 frames agree on `zframe` exactly (port index N vs trace frame N+1); worst displacement 1 tick(s)
second5: 353 of 894 frames agree on `zframe` at the strict same index (no shift); worst displacement 2 tick(s)
```

That is the *run's* music — `--block 5`'s plan, driven in process with the real player — against the
same trace, and the third line is the check D28 rests on: the port's `_np_zframe` against the trace's
own column at the sample instant's one-index shift, **753 of 891 exact and never more than one tick
apart**. The fourth is the **falsification of that shift**, printed and asserted beside it: the same
comparison with no shift at all gives **353 of 894**, worst 2 ticks — the shift's own tick, and
nothing that grows — where a drifting clock would diverge without bound. The 100/22/8 split is the
other half of the same argument (see the falsification list).

**ENDLOGO — the phase's proof, and both commands are the acceptance.** Task 8 replaced A7's measured
hold with `END.C:107-111`'s own loop, and the four keyframes are unchanged. The two runs' outputs are
identical except for the report path, so they are shown side by side with the command labels added
and the frame lines indented; `(…)` marks the tool's `report:` line in each, and `exit: 0` is the
shell's reading of the command's exit code:

```
--part endlogo   frame 430: 0.0000% differing (0/128000 px), max delta 0
                 frame 500: 0.0000% differing (0/128000 px), max delta 0
                 frame 900: 0.0000% differing (0/128000 px), max delta 0
                 frame 1180: 0.0000% differing (0/128000 px), max delta 0
                 (…)
                 verdict: identical      exit: 0
--block 5        frame 430: 0.0000% differing (0/128000 px), max delta 0
                 frame 500: 0.0000% differing (0/128000 px), max delta 0
                 frame 900: 0.0000% differing (0/128000 px), max delta 0
                 frame 1180: 0.0000% differing (0/128000 px), max delta 0
                 (…)
                 verdict: identical      exit: 0
```

(Both `(…)`s stand for the tool's `report: out/t9-endlogo-{part,block5}-diff/report.json` and
`captures: 1 measured` lines. `report: …/report.json`, `mode: mixed`, every offset 0.)

**The tool's verdict is not the whole claim.** Hashing the port's dumped files against the
reference's own gives **identical digests on every file both sides carry** — `430.rgb 85789170…`,
`430.idx eec19bc6…`, `500.rgb fb8fbf2a…`, `900.rgb f2514b29…`, `900.idx c0e0e259…`,
`1180.rgb 755f4747…`, all six equal to the values `reference-indices.md` records by hand — and
identical under both commands. 430 and 900 go through the **indexed** path and 500 and 1180 through
RGB, so the port is exact on the frames the indexed path skips as well as the ones it takes. The
gate's own readings: the port's published words cross at index **1144** (`26:49`, `zplus` 1 →
`row − 64` = −15), the tick the check breaks on is **1145**, `GATE_ALIGNMENT_TICKS` holds one more,
and the fadeout's `c = 63` is **1147** — the hold **567 ticks** from 580, which is A7's number,
supplied in two pieces: §5.1's placement gives one frame and §5.2's disclosed alignment the other.

#### The falsification list (the plan's Step 2)

Every constant and rule this phase introduced, in Phase 2's table shape: the value, how it was
derived, what a one-unit error would change, and the strongest **independent** check that exists for
it. "Independent" here means the same thing it did in Phase 2 — a measurement produced for another
purpose or by a different instrument, **not** the evidence the constant was derived from.

| constant or rule | value | how it was derived | a one-unit error would… | independent check |
|---|---|---|---|---|
| the music's boot anchor `shell::MUSIC_START_FRAME` | **408** | `music analyse`'s own columns: the first row change (frame 427, `_np_zframe` 13 there) back-extrapolated at the tick clock's measured 1.4622 frames a tick. Four sessions give 408.0/408.0/408.1/408.1 — a **0.1-frame spread** | move every row change in every trace comparison; the shell's test fails loudly, because a four-frame error publishes a `(order, row)` the trace's window never reaches | **half** — the anchor is measured four times and *derived* from each session's own columns rather than fitted (408.0, 408.0, 408.1, 408.1: two sessions of one recipe plus `second3`'s and `hiddenu`'s own boots), and the derivation has no free parameter; but all four measurements come from the **same instrument** — the trace's own columns — on the same emulator, and nothing independent of that instrument measures the boot's clock |
| `MusicStart`'s rule: wait, pre-poll, or neither | anchor ≤ 408 → wait `408 − anchor` polls; > 408 → **pre-poll** `anchor − 408` at the run's start; own numbering → start with the run | D27, from the trace's words around 408: the speed/tempo words change at 396 (the boot's `restartmus`) and at 408 (the block's), with **zero ticks between** — the twelve frames `_zsoundon`/`_asmcopperinit` take before `_zpollmix = 1` hands the player to the copper | a run anchored at 404 would start its music four frames early (every row change four frames out) or wait four frames too long | **no independent check** — every runnable entry point today is anchored at 404, so the **pre-poll branch is never taken by a run the port can make** (`pre` exists only for an anchor past 408, and its maximum today is **zero**): it is derived, unit-tested, and unexercised end-to-end. `SECOND 3` is the case it exists for, and Phase 4 owns it |
| the tick quantisation: `2.5/tempo` a tick, `speed` ticks a row | `samples_per_tick = mix_rate × 5/(2 × tempo)`, truncated; `833` at 40000/tempo 120, `769` at 130 | read out of `STMIK.300`'s rebuilt segment image (`+0x3d99`…`+0x3da6`'s `mul`/`div` pair is `_zmixspeed × 0x7d` over `0x32 × tempo`), not fitted | one tick a row and a whole row's effects move; at speed 6 the row boundary moves by 8.76 frames | **yes** — the trace's own row intervals: the longest window (296 intervals over 3,455 frames of `hiddenu`) is predicted to **−2.6 frames (0.08 %)** from the player's own speed and tempo words, and four sessions agree |
| the poll's placement in the engine | one `music.poll()` a tick, **between copper slot 0 and the first band** (`crates/sr-engine/src/engine.rs:206-219`) | `MAIN/COPPER.ASM:449-466`: `copper_doit` runs slot 0, then `call _zpollme` (`:466`), then `u2copperint`; the list runs that entry at 20 of the frame's 100 units (`copperlst0`, `:41-43`) | a part's `musplus()` at tick `T` would be the *previous* tick's words (poll after the part) or the tick's own (poll before it) — one frame on ENDLOGO's gate, which is exactly the gap §5.1 explains | **yes** — the instruments that fire are **the engine's own three** (`the_music_is_polled_between_copper_slot_zero_and_the_first_band`, whose recorded order is `["copper:0", "music:poll", "frame:0:0", "frame:0:1", "copper:1", "copper:2"]`, `a_tick_polls_the_music_exactly_once` and `a_part_reaches_the_music_through_its_context`) plus ENDLOGO's `the_runs_own_player_opens_the_gate_on_the_same_tick`, `tests/cred.rs`, `tests/endlogo.rs`, `tests/endscrl.rs` and `tests/headless.rs` — all failing by name when **the re-reviewer** moved `music.poll()` past the band loop in a scratch copy (a mutation this close-out did not run; the re-review disclosed the temporary edit and proved the revert with `sha256sum -c`). **Two things that mutation does *not* do, both measured:** it leaves `the_runs_music_is_where_the_trace_holds_it_at_every_frame_the_trace_has` **passing with identical numbers** (`778 of 894`, `753 of 891`, `353 of 894` — that test samples `music_position()` after each tick, so a move *within* a tick is invisible to it), and it makes ENDLOGO's acceptance **fail** (**exit 1**, frame 1180, **25.7117 % differing**, 32911/128000 px, max delta 5). So the acceptance does fire on this row; what it cannot do is attribute the failure between the placement and the `GATE_ALIGNMENT_TICKS` that compensates it — the limit the A7 row below states about the alignment |
| the `zplus` rule: the object's **two-bit OR** | `1` if the next order-list entry is `254`; `2` **OR-ed in** if the order being played *is* a `254` the walk stepped over; `0` otherwise; a seek clears it | the object's own order walk (`+0x4452`…`0x4467`, disassembled from the rebuilt segment image) — **the artifact, not the note**: `music-clock.md` §6 first read it as a choice between the two and was amended to the OR when the object disagreed | `[254, X, 254]` — a shape MUSIC1 carries at orders 25/26/27 — gives **3**, where a two-way reading gives 1. `musplus` then moves by a row, and ENDLOGO's gate with it | **half** — `music analyse`'s section 5 re-derives the rule from each trace's own order sequence: **11,408 samples, 0 mismatches** (1,150 + 1,152 + 2,755 + 6,351 across the four), which is a real check of every shape the traces hold. But the shape this row's own error names — `[254, X, 254]` — is in **no filed trace** (`music-clock.md` §6), so the count is blind to the error the row exists to catch. What *does* discriminate is the object's own `orw`s at `+0x4452`…`0x4467`, pinned by `xtask/src/music.rs`'s `zplus_rule(&music1.orders, &[24, 26]) == [1, 3]` — and that is the rule's derivation evidence, not an independent instrument |
| the mixer's samples-per-tick relation | one poll produces exactly `samples_this_poll` (`mix_rate/70.086`, fraction carried; mean `570.727` at 40000 Hz, and the first five polls read `[570, 571, 571, 570, 571]`) | `player.rs`'s `FRAME_UNITS` = 70,086 — integer arithmetic in 1/70,086 of a sample so the ratio stays exact | the mixer would drift against the tick clock by a fraction of a sample a poll, and the row boundaries would move after ~10⁴ polls | **yes** — `a_poll_produces_exactly_the_samples_it_says_it_does` and `the_polls_samples_are_the_frames_share_and_carry_their_fraction`; and the trace comparison over 8,732/28,618 polls is the end-to-end check, since a drifting sample count moves every row change |
| the ring's drain | 8,192 samples, bounded, **oldest dropped** when full, `dropped()` counts them; the drain moves no engine state | `player.rs`'s `RING_SAMPLES`, `mix.rs`'s capacity, and the object's own DMA buffer room; `MAIN/STMIK.H:100-103` for the position being the engine's rather than the device's | the device would either stall the tick (forbidden: never spin-wait) or repeat samples; a shorter ring drops music on a slow drain | **yes** — `a_device_that_stops_draining_loses_the_oldest_samples` and `the_drain_moves_no_engine_state`; and `--headless` dumps are byte-identical with and without `--no-audio`, which pins the player's advance to the tick and not to any consumer |
| the output curve and the table the machine holds | `K = 0x8000 / mul` ramp entries of `(k × (65536/K)) >> 8`; **zero fill `2·⌊(0x800 − K)/4⌋`**; tail `2·⌊rem/2⌋` bytes of `0xFF`; indexed at `0x400 + value` | the builder's own bytes (`+0x5565`–`+0x55c1`) **as the machine executes them**: `+0x559d`'s `rep stosw` leaves `CX` = 0, so `add %cx,%cx` doubles zero and `+0x55a1`'s `rep stosb` writes nothing — the zero region is half what the code intends, and `+0x55bd`'s shortfall fills `0xFF` | at the reference's `mul` 64 the boundaries move from 768/1280 to 1,536/2,048 — the *intended* table — and the mixer would index the ramp where the machine's memory holds zeros or `0xFF`; the port's own mono `mul` 48 is off by one entry (682 against 683) | **yes** — `cargo xtask music probe --dump` read the bytes off the machine, and the re-review read them twice: the **same table byte for byte in two modules' runs** (all 2,048, one change line, `cs:0x6C` = `6E 00` both times). `tests/mix.rs` builds **both** the machine's table and the intended one and asserts they differ at `mul` 48 |
| the stereo divisor | **21111** (`0x5277`), making the reference's stereo playback **5.26 % flat** (one semitone is 5.95 %) | the segment's own initial data at `[0x8d7]`, read on every path the reference takes and **written on none** of them — the only store is `+0x228f`, inside `_zoutputmode == 2`'s **Gravis** branch (`MAIN/U2.ASM:734`), which the SB Pro run never reaches | the pitch moves by a quarter-tone's worth of cents and every sample is stepped wrong; the port and the reference would disagree about the module's tempo in the audible sense | **no independent check** — no audio reference exists (D17), so nothing measures the pitch. The check is that the word is *read* and never written on the reference's path, which two independent readings agree on (`music`'s segment localisation and the `music probe` dump); the 5.26 % is arithmetic from the two numbers, not a measurement of sound |
| A7's 567 and Task 8's reconciliation | the hold is **567 ticks** (the break at 1146, the fadeout's first step at 1147), supplied by **two** pieces: the poll-placement (1144 → 1145) and `GATE_ALIGNMENT_TICKS = 1` | A7 (Phase 2) derived 1147 bit-exactly from two reference frames; Task 8 re-derived it from the *player's state* and got 1145 | the fadeout's first index moves one step: 1180's `c` becomes 31 or 29 instead of 30, and the keyframe stops matching — **the acceptance fails at 0 and at 2**, which is what makes the constant load-bearing rather than a fudge | **the placement: yes** (§5.1 is a chain of citations, corroborated quantitatively — the port's `_np_zframe` matches the trace's own column 753 of 891 frames at one tick, and the 100/22/8 split is what the poll geometry's ~20 % predicts, where a *shifted* clock would match **none**). **The alignment: no** — its value **equals the difference the acceptance absorbs**, so the acceptance cannot falsify it. The evidence is the placement, the trace and A12's ±3, and it is stated as such in the note, in D28 and by Task 8's review |
| the `sync` boundary | answers `(0, 0)` with **−15609** (`0xC307`, `musframe_9`'s `pop es; ret`); returns the table's last code, **4**, for `0x4200 < key <= 0x8E2E`; **refuses** only above `0x8E2E` | `DIS/DISINT.ASM:428-463` (the table, `mov cx,16`, the `%bx − 2` read) and `:420`/`:424` for the two bytes before it; the boundary from the word the 16th compare reads (`2e 8e`, `0x8E2E`) | getting `(0, 0)` wrong is a **behaviour divergence**, not a cosmetic one: the tree's callers compare `dis_sync()` with a small positive, so a wrong answer starts a part or hangs it. A one-unit error inside the table moves a caller's start by one row's worth of codes | **the answered domain: yes** — the word before the table and the last code field are the source's own instruction sequence (`the_table_is_the_shipped_shells_own_bytes` finds the 60-byte table exactly once in each of the three shells); `(0, 0)` is *reached* for 278 consecutive samples of `second5`, and the `0x4200 < key <= 0x8E2E` band is *reached* for 5,148 of `hiddenu`'s 6,351 frames (81.1 %). **The refused region: no** — nothing in the tree is *shown* to call above `0x8E2E` (both callers are block 1's, and how far MUSIC0's order list has got when they run is not measured until they are ported). See A15 |
| `ZFRAME_SLOP` | **1 tick** | the sample instant's one-index shift (D17's own reading of the emulator: a session's frame is published at the frame's *start*), measured as the check's own line: **753 of 891** frames exact at the shift, worst **1 tick** | at 0 the check fails on the shift the sample instant makes; at 2 it stops distinguishing the shift from a drifting port clock — and the strict same-index reading (no shift at all) gives **353 of 894**, worst **2 ticks** — the one-tick shift plus the one tick the shifted reading already allows, and nothing that grows with the window | **no independent check** — the *same test* prints both readings and asserts each one's bound (the shifted reading's `ZFRAME_SLOP`, the strict one's `ZFRAME_SLOP + 1`), so "the shift, not a drift" is a printed measurement rather than a phrase: `the_runs_music_is_where_the_trace_holds_it_at_every_frame_the_trace_has`'s third and fourth lines. It is the measurement the constant was derived from, read twice, not a second instrument — defensible as a falsification, not as an independent one under this table's preamble |
| `JITTER_FRAMES` (A12's ±3) | **3 frames** | the two filed sessions of one recipe (second5/second5b) disagree by one frame at 8 of their 131 row changes; three is the observed worst displacement in any direction across all four traces (`hiddenu`'s 3) | a smaller value fails on the oracle's own jitter; a larger one stops catching a displaced clock (the phase's placement test would pass a run whose music started up to three frames out) | **no independent check** — it is a *measurement of the oracle against itself*, re-derived from the same two committed CSVs the constant came from (8 of 131, each exactly ±1 frame, the same eight). The shell's test for a four-frame anchor error is an outside check on the constant's *use*, not on its value |
| the tick-in-row clear on a seek | `play_song` zeroes `[0x2000]` (`+0x56ec`) | Task 3 found the gap and Task 7 is where it became reachable: `restartmus` reaches `_zplaysong` **mid-song** with the player anywhere inside a row | a seek mid-song would begin the new order's row 0 on a *later* tick and that row's cells would never be read — a different song from the second restart on, in every block | **yes** — `a_seek_mid_song_clears_the_tick_counter_so_the_new_rows_cells_are_read` (fails without the mirror), and the object's byte says so; the pre-condition is itself measured (the stopped-state tick path *cycles* `[0x2000]` mod `_np_speed` rather than clearing it, so at a restart the counter is generally non-zero) |

**The count, measured rather than assumed.** The table has fourteen rows, and they divide three ways:

- **Six are checked by something independent of their own derivation**: the tick quantisation, the
  poll's placement, the samples-per-tick relation, the ring's drain, the curve's table, and the
  tick-in-row clear. Each names what the check *is* — a trace measured for another purpose, a
  mutation, the object's own instruction sequence, or two modules' runs where one was expected.
- **Four are half-checked, and the halves are named.** The anchor is four measurements of one number
  by one instrument (a spread of 0.1 frames, and no *other* instrument to check it with). The
  `zplus` rule's trace count is real but blind to the `[254, X, 254]` shape its own error names —
  the object's `orw`s decide that case, and they are the derivation evidence. A7's **placement** is
  mutation-tested while its **alignment**'s value equals the difference the acceptance absorbs. The
  sync boundary's **answered** domains are measured — the word before the table and the last code
  field are the source's own, `(0, 0)` is reached for 278 samples of `second5`, and the
  `0x4200 < key <= 0x8E2E` band for 5,148 of `hiddenu`'s 6,351 frames — while its **refused**
  region above `0x8E2E` is not: nothing shows the demo's callers stay inside the answered domain.
- **Four have no independent check at all**, and they are the ones the phase's reviews found:
  `MusicStart`'s pre-poll branch (unreachable by any run the port can make today), the stereo divisor
  (there is no audio reference to measure a pitch against), and `ZFRAME_SLOP` and `JITTER_FRAMES`
  (each is the measurement it was derived from, printed twice or re-derived from the same CSVs —
  defensible as a falsification, not as an independent instrument). The curve's table is measured on
  two modules' runs, but its **index base** is an open choice (**A14(a)**) — a fifth absence, in a
  row that otherwise passes.

The plan's Step 2 named **A7's 567 as *the* independent check on the cue**; the measurement splits
that into the piece that is one (the placement, mutation-tested) and the piece that is not (the
alignment, whose value is the difference the acceptance absorbs). It also adds checks the plan did
not name — the poll's placement, the ring's drain, the curve's table, `ZFRAME_SLOP`,
`JITTER_FRAMES` and the tick-in-row clear — and it names five absences. That difference is recorded
rather than smoothed over, as Phase 2 recorded its own two-and-two. The `zplus`, `ZFRAME_SLOP` and
`JITTER_FRAMES` rows were regraded from "yes" in the final review's fix wave: their checks are real
but none is independent of the evidence the value came from.

#### The verification commands, re-run

```
$ cargo xtask reference verify --tree reference
(…)
verdict: ok — 28 capture(s), 109 file(s), 26128467 bytes
exit: 0

$ cargo xtask assets verify --tree --decode
(…)
decode: 151 asset(s) — 55 picture(s), 2 animation(s), 5 palette(s), 88 headerless with no decoder,
1 refused on record, 0 finding(s)
verdict: ok
exit: 0

$ cargo test --workspace
(…)
test result: ok. 140 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 8.10s
(…)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

$ cargo clippy --workspace -- -D warnings
exit: 0
$ cargo clippy --workspace --all-targets -- -D warnings
exit: 0
```

(Each `(…)` is a marked elision: the 28 `ok <capture>` and 151 `ok <asset>` lines the two verifiers
print above their verdicts, and the 31 further `test result:` lines between the run's first target
and its last. No `test result:` line the tool printed says `802` — that figure is the sum, and the
sentence below it is where the arithmetic is.)

The **802** is the sum of the run's 33 per-target `test result:` lines, and **0 warnings** is
counted the same way over the same file (the run prints no `warning:` line at all). It is the same
total Task 8's tip recorded — the close-out re-ran the suite rather than adding to it. The **three ignored** are
the same three Phase 2 left and are named where they live: `sr-assets/tests/omf.rs`'s
write-into-`assets/` regenerator, and CRED's two record tests against a git-ignored scratch capture.
`reference verify` now covers **28 captures / 109 files** where Phase 2 closed at 24 / 105: the four
music traces are the phase's one new kind of reference, and the verifier recomputes each trace's
coverage from the file's own frames rather than believing its manifest.

**`reference/` gained one new kind of artefact and nothing else.** The four `reference/music/*`
traces are captures the phase's own tool wrote (`cargo xtask music trace`), each with its manifest;
no frame file is touched, and the `music-trace` kind is a manifest field `reference verify` checks —
a kind it does not know is **refused by name**, not walked past (measured in Task 1's review: a
`"video-trace"` manifest exits 2). `assets/` is unchanged at 151 files plus `PROVENANCE.md`.

#### What could not be confirmed — the phase's own negatives

- **There is no audio reference, and none is claimed.** The emulator runs silent (D17), so **the
  mixed sound is never verified by a null test against the original** — it is verified by
  construction (the mixer's arithmetic is hand-computed from the module's bytes in `tests/mix.rs`),
  by internal invariants (determinism, non-silence, the samples-per-tick relation, the loop/one-shot
  census), by the object's own bytes read out of `STMIK.300`'s rebuilt image, and by a person's ears.
  **The listen is the user's and has now happened**: they ran the demo with `--audio-dump` and
  report the music "sounds amazing" — the first question, whether it sounds like the demo's, answered
  by the only instrument that can answer it. Their real-device runs skipped; the same WAV plays
  perfectly on the Windows host outside the VM, and the skip follows the players rather than the file
  (VLC and the default player both skip it differently, while the browser's audio is clean), so the
  cause is the VirtualBox guest's audio path, not the port — recorded here rather than fixed in the
  port, because there is nothing in the port to fix. The second question — does ENDLOGO hold about as
  long as it sounds like it should — was not separately reported, and nothing here claims it. No test
  in the tree can stand in for a listen — Phase 0's single human look found a bug
  (the SDL texture-format misread, `03a3b11`) that no test in the repository could see.
- **The `--block 5` half of ENDLOGO's acceptance is a recorded comparison, not an assertion.** The
  automated keyframe test (`bins/secondreality/tests/endlogo.rs`) dumps under **`--part endlogo`
  only**; the same four keyframes under `--block 5` are compared with `cargo xtask compare` and
  their `0.0000 %` is recorded here, not asserted by a test. What the suite does pin through the
  block is the chain's timing — `tests/shell.rs`'s published positions at 404/430/500/900/1180 —
  and CRED's two indices through the same chain, so a chain-level timing change is caught; the
  block-5 pixels themselves are a manual result, which is B-M7's parked ruling.
- **The boot's twelve-frame transient is not modelled.** The modules' own speed/tempo words change at
  frame 396 (the boot's initial `restartmus`) and again at 408 (the block's restart), with **zero
  ticks between** them; on a `--block 5` run the only frames it is visible on are 404–407, where the
  port reads MUSIC0's patched 6/120 and the trace still reads MUSIC1's 3/130. The position words a
  part can read are identical either way, which is why it is recorded as a divergence (D27,
  `music.md` §12) rather than modelled; and no part in either runnable block can read the difference.
- **`zinfo` is 0 and that is all that is measured.** D8's route 1 was performed and the word is 0 in
  every sample of every filed trace; what its other values (`1`/`2`/`3`, `0xf0`, `0xff`) are is **not
  measured by anything**, so the port does not invent them — it returns 0, and a part waiting on a
  non-zero cue waits for ever against the port exactly as it does against the reference. GRID and
  HARD (Phase 8) are the parts that will meet this.
- **The order list: what the trace settled, and what it did not.** It settled `254` — the word is
  **stepped over**, MUSIC0's at order 27 taking the counter from 26 to 28 and MUSIC1's at order 46
  from 45 to 47, with both neighbours played — and it settled that **no `255` is ever played**: the
  shell's restarts jump past the early ones and pre-empt the two at orders 78/79. So everything
  about a played `255` is unmeasured (the port follows the object's bytes — the scan gives up;
  nothing stops — and unit-tests it) and `zplus`'s `2 → 0` return is extrapolation too: no filed
  trace holds that shape. Both are the object's code read out rather than the reference's behaviour
  measured.
- **The gate's alignment is an alignment, and its honest limit is that the acceptance cannot falsify
  it.** `GATE_ALIGNMENT_TICKS = 1` equals the difference between the port's own crossing and the
  reference's frames, so the four keyframes confirm it either way — the evidence is §5.1's placement,
  the trace and A12, not the diff. It is a fact about this boot and `second5`: a run whose music
  starts elsewhere has no reference frame to align to.
- **`dis_sync()` above `0x8E2E` is refused and cannot be answered.** Above the word the 16th
  compare reads (`0x8E2E`, the source's own `2e 8e`) the loop's fall-through returns one byte of
  instruction framing and the **low byte of a linker-set displacement**, measured `0xCD06` in
  `MAIN/U2.EXE`, `0xED06` in `MAIN/2NDFIX.EXE` and `0x9D06` in `U2.EXE`. A number that differs
  between builds of the original is not one this port returns. Up to and including `0x8E2E` the
  compare matches and the answer is the table's own last code, 4 — deterministic, and returned —
  so the phase's answered domain is `(order << 8) | row <= 0x8E2E` and nothing shows the demo's
  callers stay inside it (A15). The answered band is not hypothetical: `hiddenu` holds
  `(ord << 8) | row > 0x4200` for 5,148 of its 6,351 frames (81.1 %).
- **The three build divergences D24 registers are still open, and Phase 3 added none.** TUNNELI's
  second ramp (followed the release), CRED's data (**A10**, followed the tree) and ENDSCRL's build
  (**A11**, followed the tree) are unchanged. Phase 3's one candidate — a `_zpollmix` reading that
  looked like a tree-vs-release difference — was **resolved as no divergence at all**: the word is 1
  on the reference's path (`MAIN/U2.ASM:783` writes it, and `music probe --at 0x5AB0` reads it move),
  so the polled mode the port models is the reference's own.
- **The `#[ignore]`d couplings Phase 2 carried are still `#[ignore]`d** — CRED's two record tests
  read a git-ignored scratch capture, so a fresh clone cannot re-run them, and Phase 3 neither
  widened nor narrowed that. They are disclosed where they live (`cred.md` §3).
- **Two `grep` traps cost this phase two false claims, and any note resting on a search has to say
  which search.** The first: the upstream `.ASM` sources are **not UTF-8** (`MAIN/U2.ASM` is
  ISO-8859 with CRLF), so a search of them needs `grep -a` or it reports them as binary. The second:
  in this shell `grep` is a **wrapper** around `ugrep` with `--ignore-files` and `-I`, so a recursive
  search silently skips every ignore-listed path and every file that reads as binary — measured here:
  `grep -rl needle .` finds **0** where `command grep -rl needle .` finds **1**. Between them they
  produced a false "`_zplaysong` has no caller anywhere in the tree" (Task 2's report; `U2.ASM:394`
  and `:786` both call it) and a false "no surviving copy" (Task 6), the second found by a re-review.
  See D29.
- **The phase's most-repeated defect was a change that was never made.** Five times a round's report
  described an edit as landed while the tree still held the old text; the fifth instance's author
  found the mechanism (**an unchecked `str.replace` whose target did not match**, so the substitution
  silently no-opped while the summary was written from intent) and the phase's fix is not more care
  but a failing assertion at the point of edit, plus a review that reads the tree rather than the
  report. D29 records it, with the two `grep` traps above.
- **The oracle's own spread is the residual, and the port sits between the sessions.** No fixed
  clock can be frame-identical to two sessions that disagree with each other; the port's worst
  displacement from `second5` is 2 frames with 778 of 894 frames exact, and from `hiddenu` 3. A12 is
  where that lives, and nothing closes it short of a trace of the reference's frame clock beside a
  trace of the guest's.
- **`MusicStart`'s pre-poll branch is untested end-to-end** (see the falsification list): the branch
  is unit-tested, and every runnable entry point today takes the wait branch instead — `SECOND 3`,
  the case it exists for, is Phase 4's.

#### The whole-branch review, the fix wave, and the tag

**The review ran** over `53eb9c1..66ab915` in four slices and returned **0 Critical / 10 Important /
30 Minor**. The adjudicated fix wave is `316f2c0`, corrected by `c2e8266` and `7e80935`; its
per-finding record is `.superpowers/sdd/03-music/fix-wave-report.md` (git-ignored, like the ledger).
The fix wave was re-reviewed, and that re-review found six further false or stale claims — including
four the wave itself had introduced — each corrected and re-checked before the tag. The tag goes on
here, on Phase 1's and Phase 2's ruling that a tag which has to be moved is worse than a tag applied
a few commits later.

### Phase 4's evidence (close-out, Task 10, 2026-10-17)

**Exit criterion (spec §10): ALKU, PAM, DDSTARS, DOTS, MNTSCRL, RAYSCRL verified.** All six are.
Every number below was **re-run at the committed state** (`9c457e2`, branch
`phase-4-music-synced-tier-a`); this close-out's own commit adds records only, so the state the
commands ran against is the phase's tip. The workspace suite was re-run after the record edits and
its `test result:` lines in the verification block below are that run's. The blocks are pastes of
those runs: `(…)` marks
an elision, and an `exit:` line is the shell's reading of a command's exit code rather than a line
of its output.

| part | reference keyframes | planes compared | the verdict |
|---|---|---|---|
| ALKU | 900, 1719, 2719, 5859 | RGB only — all four (176 × 400 = 70,400 > 65,536, so the oracle refuses the indexed read, `reference-indices.md` §1.6) | **0 of 128,000** pixels at 900/1719/2719; **2,353 (1.8383 %)** at 5859, all inside the old text's glyph region — A20's batch phase; 350 of 400 rows byte-identical |
| PAM | 7710, 7760, 7830, 7900 | RGB all four; 7900 also carries `.idx` | **0 of 64,000** at all four RGB; 7900's index plane differs in **60,375/64,000**, max delta **194** — the plane is BEGLOGO's white field, not PAM's screen (**A21**, registered not adopted) |
| DDSTARS | 1900, 3000, 5000, 8000 | RGB only — the emulator names the screen 16-colour and refuses the indexed read (§22.5) | **0 differing bytes** at all four |
| DOTS (`minvball`) | 10900, 11200, 11800, 12400 | RGB only — every keyframe is a moving frame | **67 / 63 / 37 / 25** px of 64,000 (0.1047 / 0.0984 / 0.0578 / 0.0391 %), the release's coincident-dot overlay and the fitted `j` (**A19**) |
| MNTSCRL | 500, 1100, 1700, 2300 | 500 indexed + RGB; the other three RGB only | **0 differing bytes** at all four; 500 byte-identical in both planes |
| RAYSCRL | 13000, 13500, 14000, 14500 | 13500 indexed + RGB; the other three RGB only | **0** at 13000 and 13500 (13500 in both planes); **3** and **42** px at 14000 and 14500, all in the font-overlay region — the release's `MIEKKA.SCI` differs from the tree's (§6 of the note), not a timing difference |

The six commands and what they printed. The four parts whose keyframe tests print no per-frame line
assert the verdicts in the test body — MNTSCRL's `the_four_keyframes_match_the_reference` asserts 0
bytes at 500 in both planes and at 1100/1700/2300 in RGB, MINVBALL's
`the_four_keyframes_match_the_reference_at_the_mapping` asserts the exact residues above (so a
change that moves them fails), RAYSCRL's and DDSTARS's tests assert their counts the same way — so
the harness's `test … ok` is the printed verdict for those. The `should panic` lines are the
truncated-frame tests' own expected output, not failures.

```text
$ cargo test -p secondreality --test alku -- --nocapture
(…)
running 4 tests
test the_run_is_numbered_in_boot_frames_and_draws_320x400 ... ok
5859: 2353/128000 px differ (1.8383 %), all in rows 150-243, columns 40-279
test the_four_keyframes_against_the_reference ... ok
second1 at the keyframes: [(900, "0:56", "0:56"), (1719, "2:21", "2:21"), (2719, "4:7", "4:8"), (5859, "9:46", "9:46")]
test the_runs_music_is_the_block_one_recipes_at_the_keyframes ... ok
test a_block_one_run_starts_at_alku_and_is_the_same_run ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 35.64s

$ cargo test -p secondreality --test pam -- --nocapture
(…)
running 3 tests
test the_run_is_numbered_in_boot_frames_and_plays_block_ones_music ... ok
7900 index plane: 60375/64000 px differ, max delta 194
test the_7900_index_plane_is_the_next_parts_white_field_not_pams_frame ... ok
boot 7710: 0/64000 px differ (0.0000 %)
boot 7760: 0/64000 px differ (0.0000 %)
boot 7830: 0/64000 px differ (0.0000 %)
boot 7900: 0/64000 px differ (0.0000 %)
test the_four_keyframes_match_the_reference_with_their_support_frames ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.36s

$ cargo test -p secondreality --test mntscrl -- --nocapture
(…)
running 8 tests
(… the `a_truncated_frame_is_refused_rather_than_counted` panic text is that `#[should_panic]` test's expected output …)
test a_truncated_frame_is_refused_rather_than_counted - should panic ... ok
test a_block_three_run_starts_here_and_is_numbered_the_same_way ... ok
test the_held_opening_is_the_references_frame_500_in_both_planes ... ok
test the_music_is_eleven_polls_in_at_the_parts_first_tick ... ok
test the_run_is_numbered_in_its_own_frames_and_draws_320x200 ... ok
test the_releases_first_wait_is_seventy_one_ticks ... ok
test the_four_keyframes_match_the_reference ... ok
test the_real_players_cue_fades_the_part_out_and_the_run_holds_black ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.07s

$ cargo test -p secondreality --test minvball -- --nocapture
(…)
running 4 tests
(… the same expected `#[should_panic]` output …)
test a_truncated_frame_is_refused_rather_than_counted - should panic ... ok
test the_run_is_numbered_in_its_own_frames_and_draws_320x200 ... ok
test the_four_keyframes_match_the_reference_at_the_mapping ... ok
test the_run_reaches_the_frame_cap_and_retires ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.92s

$ cargo test -p secondreality --test rayscrl -- --nocapture
(…)
running 4 tests
(… the same expected `#[should_panic]` output …)
test a_truncated_frame_is_refused_rather_than_counted - should panic ... ok
test the_run_is_numbered_in_its_own_frames_and_draws_320x200 ... ok
test the_music_is_the_block_three_position_at_every_keyframe ... ok
test the_four_keyframes_match_the_reference_at_the_mapping ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.91s

$ cargo test -p secondreality --test ddstars -- --nocapture
(…)
running 3 tests
(… the same expected `#[should_panic]` output …)
test a_truncated_frame_is_refused_rather_than_counted - should panic ... ok
test the_run_is_numbered_in_its_own_frames_and_draws_320x400 ... ok
test the_four_keyframes_and_their_support_frames_match_at_the_mapping ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 22.72s
```

(ALKU's music line is the block-1 recipe at the keyframes: the port and the trace agree on the
order everywhere and on the row at three of four; 2719 is one row apart (`4:7` against `4:8`),
which is the block-1 clock's 2–4-frame lag and the trace's sample instant (A12), and the test
allows one row either side. The four keyframe frames themselves are the reference's — the music
does not touch ALKU's picture.)

#### The falsification list (the plan's Step 2)

Every constant and rule this phase introduced, in Phase 2/3's table shape: the value, how it was
derived, what a one-unit error would change, and the strongest **independent** check that exists for
it. "Independent" means the same thing it did in Phase 3 — a measurement produced for another
purpose or by a different instrument, **not** the evidence the constant was derived from.

| constant or rule | value | how it was derived | a one-unit error would… | independent check |
|---|---|---|---|---|
| MNTSCRL's and RAYSCRL's cue `dis_musplus() == -11` exactly | exact equality, not `< -10` and not `<= -11` | `FOREST/READ2.PAS:200`, `WATER/DEMO.PAS:158-166`; both unit tests drive -10 and -12 and only -11 fires the 65-tick fade; the real player reaches -11 at MNTSCRL `45:53` (tick 1,979) and RAYSCRL boot 14942/14943, where the release's seven confirming reads put the first upload at 14950 | starts the fade one row (~11 ticks) early or late, or never: RAYSCRL's fade-onset palette is a step out; MNTSCRL's four keyframes are all before its cue | **RAYSCRL: half** — the survey's partner capture `out/b4b-ray-sess1/frame_014950.rgb` is the release's first fade-out upload (the port's onset matches its palette at j = 1 but differs in 1,061 px, the release's `MIEKKA` strip), and `out/` is git-ignored scratch, so a fresh clone cannot re-run it. **MNTSCRL: none** — no reference frame after its cue; the source's own `cmp dx,-11` and the player's published position are the evidence |
| MINVBALL's cue `-4 < musplus < 0` (the open interval) | -3, -2 or -1; not -4 and not 0 | `DOTS/MAIN.C:191-195`; `the_cue_window_is_the_open_interval_minus_four_to_zero` drives -4, -5, 0 and -1 and asserts only the open window breaks; `a_skipped_cue_leaves_the_part_continuing_and_drawing` covers a window stepped past | `>= -4` breaks one row early; `<= 0` breaks at 0, the common reading, and would end the part on its first main-loop tick | **none that exercises the break** — the reference's first main-loop -3 is boot **12,950**, past the frame cap at **12,947**; the -3s at 10,368–10,379 are inside the fade, which does not check the cue; all four keyframes are before both. The source line and the trace's reading are the evidence, not a measurement of the break |
| ALKU's `dis_sync() < 1/<2/<3/<4` gates and PAM's `< 10` | 1, 2, 3, 4, 10 | `ALKU/MAIN.C:59`, `:66`, `:72`, `:79`; `PAM/OUTTAA.C:39`; `second1`'s code transitions are 1 at **1537**, 2 at **2097**, 3 at **2509**, 4 at **3069** (5 at 3630), and `second1b`'s 7710 is `(13, 1)` = code **10**. A card holds 428 ticks (64 fade + 300 + 64) | a threshold one code higher delays a card or the picture by that code's span (~560 frames in block 1) or never opens it; one lower opens it at the previous code | **partial** — the reference's cards at 1719/2719 are exact, so the gates that place them are checked, and PAM's four keyframes are exact; but an intermediate error can move card 2 without moving either ALKU keyframe (with `<1` at the second gate, card 2 starts at 1965 instead of 2097, and 2719 is still card 3's hold). The trace's transitions are the derivation instrument |
| ALKU's `a&1` / `tptr` off-by-one | batch 0 waits for code **5**, not 4 | `MAIN.C:138` `dis_sync() < 4+tptr` with `:102`'s `switch(tptr++)`: case 0 leaves `tptr` at 1; `the_picture_fade_hands_over_to_a_batch_gated_on_code_five` asserts 5 | fixing it to 4 runs batch 0 at code 4 (3069) instead of 5 (3630) — ~561 frames earlier — and the text's phase at 5859 changes | **none** — 5859 is the only frame where the batches show, and its residual is A20's CPU-timing phase; the phase scan shows the picture exact at other phases, so the frames cannot separate the off-by-one from A20. The source's post-increment is the evidence |
| ALKU's 9-tick scroll | `SCRLF = 9` | `MAIN.C:399-401` `while(frame_count<9); frame_count-=9`; `the_scroll_advances_one_step_every_nine_ticks` pins the first step on the ninth tick and the second on the eighteenth, `(a=1,p=1)` then `(a=2,p=0)` | an 8- or 10-tick cadence puts a different `a`/`p` at 5859; the picture band and the refresh column move | **yes, at one frame** — the 5859 reference frame: 350 of 400 rows byte-identical and every differing pixel inside the text region, so the pan and the picture band are the reference's (`the_four_keyframes_against_the_reference` asserts both) |
| ALKU's pan arithmetic | `cop_start = a/4 + p*88`, `cop_scrl = (a&3)*2`; `start_addr = 4·cop_start`, `pan = cop_scrl/2` | `MAIN.C:403`; the AC register's unit is half a pixel; `the_pan_is_the_crtc_start_and_pel_panning` pins 20 `(a,p)` pairs and the first displayed pixel `a + p*352` | a one-byte or one-pixel error shifts or shears the window; at 5859 the picture band stops matching | **yes** — the 5859 reference frame's band and pan (above), plus the unit test's `source_index` for the first pixel |
| PAM's 4-tick cadence and `FRAME_COUNT_AT_ANCHOR = 2` | 4 ticks a step; the counter reads 2 at boot 7710 | `OUTTAA.C:46` `while(frame_count<4); frame_count=0`; the reference's own support frames force the boundary — 7759 ≠ 7760 and 7761 = 7760 put step 13 at 7760, so `T_1 = 7712` and the counter at 7710 is 2 (`reference-indices.md` §6.3) | a 3- or 5-tick cadence shifts every step, and the four keyframes land on different animation frames; a counter of 1 or 3 moves the first step a tick | **yes** — the reference's support triples (7759/7760/7761, 7829/7830/7831, 7899/7900/7901) are a different observable from the source's `4`, and the keyframes fail under either alternative (`the_four_keyframes_match_the_reference_with_their_support_frames`) |
| the block-1 music clock and PAM's pre-roll | `BLOCK_ONE_MUSIC_START_FRAME = 400`; 7,310 polls for PAM | `second1`'s first row change (frame 420, `_np_zframe` 14) fits **399.6** (direct reading 405, a 5.4-frame sample-instant gap); 7710 − 400 = 7,310; `the_block_one_pre_roll_puts_the_player_at_the_traces_7710_position` polls the real MUSIC0 and reads `(13, 1)`, zframe 5008, at 7,310 polls — the trace's row at 7710 | 399/401 or 7,309/7,311 puts the player one tick off; the row stays `(13, 1)` for six ticks so the gate still reads 10, but the published position/zframe at the keyframe differs; a 408 clock would wait four frames instead of pre-polling four, so the player is four ticks behind at the run's first drawn frame | **half** — `second1b` is a different session from the 400 fit and confirms the row at 7705/7710, but the row is a coarse observable (six ticks) and the pre-roll is arithmetic from the 399.6 fit, so a ±2-poll error is invisible to both |
| MINVBALL's Microsoft C LCG | multiplier **214,013**, increment **2,531,011**, modulus 2³², seed 1, returning `(seed >> 16) & 0x7FFF` | `DOTS/MAKEFILE:10-16` (`cl /qc /AL`, Microsoft C 6.0 large model) and the linked `DOTS.EXE`'s “MS Run-Time Library” string; first ten values 41, 18,467, 6,334, 26,500, 19,169, 15,724, 11,478, 29,358, 26,962, 24,464 | Borland's generator (seed 1, `0x015A4E35`) is a different stream and places the scatter's dots elsewhere; the 12,400 frame would not be 25 px from the reference | **yes** — the reference's 12,400 scatter is reproduced to **25 px** with the Microsoft stream, and the Borland counterfactual does not place the dots (`the_rng_is_microsoft_cs_lcg_and_its_first_values_are_the_sequences`, `the_four_keyframes_match_the_reference_at_the_mapping`) |
| MINVBALL's initial `j` | **88** | the source never assigns it (`MAIN.C:87`, `:200`); the ignored fit replays all 512 cursors against the 10,900 frame and 88 is the minimum — best five `[(67, 88), (341, 171), (353, 168), (354, 14), (356, 386)]` | `j = 171` raises the 10,900 residue from 67 to 341 px; the other keyframes' residues grow | **no independent check** — the release's stack value is inside the packed `MINVBALL.EXE`; the fit at 10,900 is the only evidence. The other three keyframes (63/37/25 px) show the value generalizes but are not independent of the same objective; `the_entry_bx_is_unobservable_at_the_keyframe` shows the other uninitialised register is unobservable |
| DDSTARS's LCG | multiplier **`0x343FD`**, increment **`0x269EC3`**, seed 0, returning bits 32–47 of the *product* (the `mul`'s `dx`), not the new state's high word | `STARS.ASM:33-42`; first twelve returns 0, 126, 25208, 45290, 7958, 4852, 38525, 27318, 39895, 10061, 34431, 60585; first table entries `(255,-512,-386)`, `(254,120,-278)`, `(253,278,244)`, `(252,125,182)` | a stream one step out moves every star; the four keyframes' 0-byte match fails | **yes** — all four reference keyframes are byte-identical, and the 11-frame survey sweep at the mapping (a second capture, git-ignored scratch) is 0 bytes at each tick (`the_rng_is_the_originals_lcg_and_its_first_values_are_the_sequence`, `the_four_keyframes_match_the_reference_at_the_mapping`) |
| DDSTARS's ring and the `+17` second write | 64 units of 8 KB, never cleared; `staradd2` writes unit `u` and `(u + 17) & 63` | `STARS.ASM:260-267`, `:378-397`; `the_ring_wraps_every_sixty_four_ticks_and_never_clears` finds the planted bit after a whole ring; `the_second_star_pass_writes_the_same_bits_into_the_plus_seventeen_unit` pins the pair | without the `+17` write the survey's 1700 frame differs in **729 px / 2,187 RGB bytes**; a 63-unit ring or a cleared unit loses the persistence | **yes** — the survey's 1700 frame is a different capture from the four keyframes and is byte-identical only with both writes (`the_plus_seventeen_write_is_what_the_survey_frame_shows`, ignored; `out/` scratch) |
| DDSTARS's text clip (D24) | the tree's commented-out guards are **not** applied; the text stays visible | `STARS.ASM:692-696`, `:711-716`, `:737-742`, `:756-762` are commented out; the reference shows text and holds **1,025** star pixels in the 31 clip-window rows at 1900 | applying the clip would remove those rows' stars and the 1900 keyframe would differ | **yes** — `the_star_clip_is_assembled_out_and_the_keyframes_hold_its_pixels` counts the reference's 1,025 pixels in the clip rows and the port's same count; the artifact question is decided by the frames, since `MAIN/DATA/DDSTARS.EXE` is packed |
| A17 — MNTSCRL's first wait | **71 ticks** | the release's last held frame is boot **554** and its pre-roll's first `scr` **555**; tick 0 at 419 gives ticks 65–135; the trace's position is order 42 row 16, `zplus` 0, `musplus` −32 on **both** sides, so no `musplus` threshold can end it there | one tick early/late and the scroll starts a tick out; the 1100/1700/2300 keyframes move 2.4–3.1 % of pixels a frame and fail | **half** — the four keyframes (not the boundary frames the value was fitted to) are byte-identical at 71, and the tree's own `musplus >= 0` is measured at **118,843 / 118,924 / 119,056** bytes against the reference's 1100/1700/2300; but the mechanism is unknown, and a single reference run cannot distinguish a duration from a release-side condition its frames do not expose |
| A18 — RAYSCRL's gate confirmation | **`GATE_CONFIRM_TICKS = 7`** | three intervals measured independently: init 12962 → blit 12969; stage 2 13609 → first draw 13616; cue 14942/14943 → first upload 14950; the published row stays 16 for eight frames and −11 for the trace's nine, so the leaves fall inside the windows, not at their starts | 6 or 8 moves the main loop or the fade one tick; the tree's immediate condition starts the main loop six draws early and the 14000/14500 frames differ by **8,375 / 11,958** px | **yes, with a caveat** — the three gates are different observables (a trace row window, the font phase, the fade-onset palette) and all give 7, and the tree's counterfactual is measured by `the_trees_immediate_gate_draws_the_keyframes_early`; but all three are read from the same reference sessions, and the cue's evidence is a scratch capture |
| MNTSCRL's `scp` bound | **640** (release), where the tree's `READ2.PAS:88` says 639 | D24's single-constant branch: under 639 boot 2300 differs in **401 bytes / 146 px**, under 640 in **0**; 1100/1700 are before the bound and identical under both; the array's extra element at 19,840 is exactly what a stride-640 read at column 640 needs for row 30 | the tree's 639 loses the 507th shift's appended column and 2300 fails; a bound of 641 would append a column the array does not hold | **yes** — the 2300 reference frame, with 1100/1700 showing the difference is the bound's and not something else's (`the_releases_scp_bound_is_sixty_forty_where_the_tree_says_639`) |
| the mid-run music positions and pre-rolls | MNTSCRL **11** polls (tick 0 at 419, clock 408); RAYSCRL **MUSIC1@86**, **268** polls; MINVBALL **MUSIC1@75**, **256** polls | MNTSCRL: 419 − 408, and the dump at tick 81 (boot 500) reads `42:10`, the trace's frame 500. RAYSCRL: `second3b` holds order 86 through 12961 and 88 from 12962; order 87 is a `254` marker that sets `zplus` bit 1, and `play_song(86)` publishes 88:0 on poll 269. MINVBALL: `second3b` holds `75:61` at 10,368, and a player seeked to 75 reaches it on poll 257 | one poll early/late puts the published position one row off (a row holds ~6 ticks); for RAYSCRL, seeking 85 or 87 changes the order/`zplus` and the init gate, and seeking straight to 88 clears `zplus` so the gate never passes | **half** — each position is checked against the trace at the part's tick 0 (MNTSCRL `the_music_is_eleven_polls_in_at_the_parts_first_tick`, RAYSCRL `the_music_is_the_block_three_position_at_every_keyframe`, MINVBALL's manifest `75:61`), but the trace is also the derivation instrument, and no keyframe's pixels depend on the position |
| MINVBALL's `repeat` | one tick per frame in the port; the reference's `repeat` measured **1** over 10,900–12,400 | the four keyframes' frame counters differ by **300, 600, 600**, exactly the boot differences; the y state — one `drawdots` integration a frame — matches at every keyframe (`the_repeat_difference_is_zero_at_the_keyframes`) | a `repeat` above one between two keyframes advances the counter without a draw and moves the ball rows; the residues would grow | **yes over 10,900–12,400** — four reference frames and their support neighbours; **the 12,947 end is a prediction** from the source's 2,450-frame cap, not a measured frame |
| A20 — ALKU's text-batch phase | no constant: the port runs the source's trace-faithful batch phase, and its `fmaketext`/`faddtext` cost zero CPU | `MAIN.C:138`/`:102`, and the phase scan moves the batch transitions while the four gates stay fixed: shift −9 gives **0** pixels, shift +0 gives **2,358**, shift +10 gives **0** at 5859 | the old text's XOR pixels appear or vanish at 5859; the picture band, pan and palette are unchanged | **none** — the phase is the original's CPU time in `fmaketext`/`faddtext`/`prt` plus the session's music clock, neither of which the port models; the scan is a counterfactual, and the reference's own direction is not established (one session). The note says so (`alku.md` §8, §11); the scan is `the_5859_phase_sensitivity` |
| the `PosList` format | u16 count, then that many u16 offsets; a zero count writes nothing | `FOREST/ROUTINES.ASM:21-35` and `WATER/ROUTINES.ASM:21-36` agree; the six real files parse to **237×31 columns / 15,592 positions** and **158×34 / 20,858**; a synthetic truncated stream is refused | a field error fails the parse or shifts every offset; the blitter writes the wrong columns and the keyframes fail | **yes** — two independent parts' files and the survey's totals agree, and each part's asset test compares every byte of the other payloads as well (`the_forest_lists_measure_as_the_survey_says`, `the_water_lists_measure_as_the_survey_says`) |
| the four part-relative mappings (D30) | MNTSCRL `419 + tick`; RAYSCRL `12962 + tick`; MINVBALL `10368 + tick`; DDSTARS `414 + tick` — none is a `boot_anchor` | MNTSCRL: the fade's frame means (6.08 at 425, 8.91 at 430, 10.25 at 435, 10.53 at 440) place tick 0 at 419. RAYSCRL: the published 88:0 at 12962, the blit at 12969, the fade's k=14 hold, and the font phase (14000 = draw 384). MINVBALL: the fade's first wait at 10,368 and the keyframes' frame counters 403/703/1303/1903. DDSTARS: the text band's open height at 1900 (28 rows ⇒ `A = 32` ⇒ `starframe 1487`), the stars' pages (tick 1485), and 11 survey frames | one tick and every index shifts: each part's four keyframes land on different frames and fail | **half for MINVBALL, yes for the rest** — MINVBALL's `10,368 + tick` is pinned to **±1** by the survey's 12,900 flat-grey frame: it is the fade-out's `c=59` upload, which puts frame counter 2402/2403 at boot 12,900 (the sibling session's `c=57` is the same curve one frame on) — a different capture and a different observable — while the four keyframes `the_four_keyframes…` holds are the fit's own evidence, not an independent check. DDSTARS keeps its 11-frame survey sweep (a different capture), and MNTSCRL and RAYSCRL are checked at keyframes beyond those that fixed them. **The start is not a frame**, which is why none is an anchor (D30) |

**The count, measured rather than assumed.** The table has twenty-one rows, and they divide three
ways (one row, the cue row, is counted by part):

- **Eleven are checked by something independent of their own derivation**: the 9-tick scroll, the
  pan arithmetic, PAM's cadence and counter, the Microsoft LCG, the DDSTARS LCG, the ring and its
  `+17` write, the text clip, A18's seven ticks (with the caveat that all three gates are the same
  sessions), the `scp` bound, `repeat` (over its measured window), and the `PosList` format.
- **Six are half-checked, and the halves are named**: ALKU/PAM's gates (the keyframes check the
  gates they land in, not an intermediate one), the block-1 clock and PAM's pre-roll (the row is a
  six-tick observable and the pre-roll is arithmetic from a fit), A17 (the keyframes check the
  value, not the unknown mechanism), the mid-run positions (the trace is both derivation and
  check), RAYSCRL's cue (a scratch capture outside the reference tree), and the four mappings
  (MINVBALL's ±1 pin is the survey's 12,900 `c=59` observation, while the keyframe test it also
  runs is the fit's own evidence; DDSTARS has the independent survey sweep, and MNTSCRL/RAYSCRL
  are checked at frames beyond their fit — the row is graded by its weakest value).
- **Five have no independent check at all**: MNTSCRL's cue (no reference frame after it),
  MINVBALL's cue window (the reference's own break falls past the frame cap), ALKU's `tptr`
  off-by-one (A20's phase cannot separate it), MINVBALL's `j` (inside the packed release), and
  A20's phase itself.

This is broader than the plan's Step 2 named: it asked for the cue forms, the cadences, the stage
bounds, the pan, the LCGs, the release waits and the fitted values, and the table adds the `scp`
bound, the mid-run positions, `repeat`, the `PosList` format and the four mappings because each is
load-bearing for a keyframe the phase claims. The absences are recorded rather than smoothed over.

#### The verification commands, re-run

```text
$ cargo test --workspace
(…)
test result: ok. 239 passed; 0 failed; 6 ignored; 0 measured; 0 filtered out; finished in 8.55s
(…)
test result: ok. 17 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
(…)
exit: 0

$ cargo clippy --workspace -- -D warnings
exit: 0
$ cargo clippy --workspace --all-targets -- -D warnings
exit: 0

$ cargo xtask reference verify --tree reference
(…)
verdict: ok — 32 capture(s), 113 file(s), 26511402 bytes
exit: 0

$ cargo xtask assets verify --decode
(…)
decode: 151 asset(s) — 55 picture(s), 2 animation(s), 5 palette(s), 88 headerless with no decoder, 1 refused on record, 0 finding(s)
verdict: ok
exit: 0

$ cargo xtask assets verify --tree --decode
(…)
tree: 151 file(s) recorded, 151 in the tree, 0 finding(s)
decode: 151 asset(s) — 55 picture(s), 2 animation(s), 5 palette(s), 88 headerless with no decoder, 1 refused on record, 0 finding(s)
verdict: ok
exit: 0
```

(The two `test result:` lines shown are the two targets that carry ignored tests; `(…)` elides the
other 37 targets' lines. **935 tests pass, zero fail, seven are ignored**, summed from the run's 39
`test result:` lines, and the run prints no `warning:` line. The seven ignored are the same three
Phase 2 left — `sr-assets/tests/omf.rs`'s write-into-`assets/` regenerator and CRED's two record
tests — plus four Phase-4 measurements: ALKU's curated-palette counterfactual and 5859 phase scan,
MINVBALL's `j` fit, and DDSTARS's `+17` survey check. The three CRED/DDSTARS ones read git-ignored
scratch captures, so a fresh clone cannot re-run them; the ALKU two and MINVBALL's fit read
committed files. `reference verify` covers **32 captures / 113 files / 26,511,402 bytes** where
Phase 3 closed at 28 / 109 / 26,128,467: the four new music traces are the phase's one new kind of
reference, and the verifier recomputes each trace's coverage from the file's own frames. `assets/`
is unchanged at 151 files plus `PROVENANCE.md`.

**The `assets` command's spelling.** The brief's `cargo xtask assets verify --decode` exists and was
run as written; `--decode` alone decodes every asset but does **not** re-hash the tree, so the
stronger `--tree --decode` (Phase 3's form) was run as well and prints the extra `tree:` line above.
Both exit 0.)

#### What could not be confirmed — the phase's own negatives

- **Which cues rest on which evidence, and which on synthetic sequences only.** The new traces are
  `second1` (ALKU: the code transitions 1537/2097/2509/3069/3630 and the 638-position block-1
  comparison), `second1b` (PAM: `(13, 1)` at 7710 and the row change at 7705), `second3b` (RAYSCRL:
  the order-86/88 seek and the four published positions; MINVBALL: `75:61` at 10,368 and the
  seek), and `hiddenu2` (DDSTARS: the position at 8000, which the part never reads — the trace
  places it in demo time). MNTSCRL's cue evidence is the **Phase 3** trace `second3` (the
  `42:10` pre-roll position and the tree's counterfactual frame 2488). Every part's cue **window**
  is additionally driven by a scripted `Music` in its unit tests: the exact `-11`, the open
  interval, the skipped cue, the exit-while-waiting, and the stage transitions are synthetic,
  because a scripted player is the only way to drive a cue the reference run does not reach.
  MINVBALL's cue is never reached in the reference run at all (the frame cap at 12,947 precedes
  the first main-loop -3 at 12,950); DDSTARS has no cue.
- **DOTS's `repeat` and the tick model.** The original advances the physics `repeat` times per
  iteration, where `dis_waitb`'s return can exceed one; the port advances once per tick. The
  reference measures the difference as **zero over 10,900–12,400** (frame counters 300/600/600
  apart, the y state matching), but the end at 12,947 is the mapping's prediction from the source's
  2,450-frame cap, not a measured frame. A `repeat` above one between two keyframes would move the
  ball rows; nothing measures whether one occurred outside the keyframe span.
- **DDSTARS's D24 text divergence.** The tree's star-clip guards are commented out and the
  reference shows text; the port follows the reference and the source's assembled-out state. The
  four keyframes are 0 bytes and the 1900 frame holds the 1,025 clip-window star pixels the guards
  would have removed. The shipped `MAIN/DATA/DDSTARS.EXE` is packed, so which build put the frames
  on screen is not readable from the files — the frames decide, and they decide against the clip.
- **A9's font shift.** The port reads `FONA.UH` and draws the 30 rows the file holds; the shift
  `[783:]` is invisible (all 76 ink runs are the same glyph bytes one column over, byte 783 is
  zero). The two rows past the strip are uninitialised linker padding: their visible half is blank
  in the reference's held cards, and the half a black palette hides remains unmeasured. The
  resolution is cited, not re-derived; the lead is the packed `MAIN/DATA/ALKU.EXE`.
- **Which keyframes are RGB-only, and why.** **ALKU**'s four are RGB-only because 176 × 400 =
  70,400 exceeds one plane's 65,536 bytes, so the oracle refuses the indexed read (§1.6) — the
  comparison is RGB and the 6→8-bit widening is load-bearing. **DDSTARS**'s four are RGB-only
  because the emulator names the 320×400 screen 16-colour and refuses the indexed read (§22.5); the
  port composes its own index from planes 0–2 and the set/reset text bit. **MINVBALL**'s four are
  RGB-only because every keyframe moves, so the indexed read (taken 20+ rendered frames late) would
  not be honest. **MNTSCRL** files `.idx` at 500 (held) and not at the three moving scroll frames;
  **RAYSCRL** files it at 13500 (held) and not at the three moving ones. **PAM** files it only at
  7900, and that plane is BEGLOGO's white field — A21 — so the index comparison there is
  registered, not adopted, while the RGB is byte-identical.
- **The same-format caveat.** Every comparison is same-format: `.idx` against `.idx` where both
  sides have one, `.rgb` against `.rgb` otherwise. There is **no cross-format claim**, and where the
  reference is RGB-only the port's index plane is not checked at all — PAM's 7900 is the case where
  the RGB agrees and the planes do not (A21). `cargo xtask compare` is not used for the four
  part-relative parts: it pairs by the manifest's reference index, and a run numbered from zero is
  not that index; the integration tests compare bytes at the mapped tick, which is stricter than
  `compare`'s ±1 pairing because there is no session phase to search.
- **The part-relative block-3/U dumps, and the stop at the first unported part.** `--block 3` and
  `--block U` runs are `index_origin: "part"`, `anchor: null`, so their dumped indices are not
  reference indices; their shell tests assert the first part, the music row and the numbering, and
  the keyframe tests run `--part <name>` and compare at the mapping. More sharply: because a block
  **stops at the first part it has not written**, `--block 1` stops at U2A (so PAM is unreachable
  through block 1), `--block 3` stops at LNS&ZOOM (so MINVBALL and RAYSCRL are unreachable through
  block 3), and only ALKU, MNTSCRL and DDSTARS start a block run. The three mid-block parts'
  acceptance is `--part`, not a block run; the block plans are untruncated and a test derives each
  stop from the registry.
- **The release's mechanisms are unknown, and the phase says so.** MNTSCRL's 71-tick wait and
  RAYSCRL's seven-tick confirmation are measured durations/behaviours; a single reference run
  cannot distinguish them from release-side conditions its frames do not expose. The packed
  `MNTSCRL.EXE`, `RAYSCRL.EXE` and `MINVBALL.EXE` are the leads. A17/A18/A19 record each.
- **The fits, named as fits.** MINVBALL's `j = 88` is a fit over the release's uninitialised stack,
  not a read value; A20's 5859 phase is the original's CPU time, not modelled; PAM's 7,310-poll
  pre-roll is arithmetic from the 399.6 fit; the block-1 clock's 399.6 sits 5.4 frames from the
  trace's direct 405. None of these is re-anchored to make a diff pass; the fits that exist are
  the ones the source leaves free.
- **RAYSCRL's 14000/14500 residual is an asset, not a timing, difference.** The 3 and 42 px are all
  in the font-overlay region the strip has scrolled into; the reference's bytes are the tree's
  previous source column at five sampled positions, and the tree's `MIEKKA.OBJ` carries the file's
  bytes exactly. Patching bytes only the release holds would stop it being an original-format read.
- **The `#[ignore]`d record tests.** Phase 2's three remain (CRED's two and `omf.rs`'s regenerator),
  and Phase 4 adds four measurements. Three of the seven need git-ignored scratch (`out/`), so a
  fresh clone cannot re-run them; they are disclosed where they live (`cred.md` §3,
  `ddstars.md` §10). The suite's green does not cover them.
- **The block-3/U pixel comparisons are not asserted end to end.** Block 1's block-vs-part equality
  is asserted for ALKU (`a_block_one_run_starts_at_alku_and_is_the_same_run`), but block 3 stops
  before its mid-block parts and block U's part is part-numbered; there is no block-level pixel
  comparison for MNTSCRL/DDSTARS beyond the part runs and the shell manifests.
- **A15's refused band is still unentered.** The two `dis_sync()` callers Phase 4 ported stay in
  orders ≤ 13 (second1's highest published order is 9; PAM's gate is at `(13,1)`), so the
  `> 0x8E2E` refusal remains a hypothetical case; A15 records it, now with the callers measured.
- **DDSTARS's exit is one frame short by construction.** The port polls `exit_requested` at the top
  of `frame`, so the tick that receives the flag is not drawn, where the source draws the iteration
  before its DIS function 2 check. No filed frame is near an exit.
- **The suite green is not a claim that every keyframe is exact.** Seventeen of the twenty-four
  keyframes are byte-identical; the seven residuals are named, attributed and asserted at their
  measured counts (A19, A20, the `MIEKKA` asset), and PAM's index plane is A21's registered
  divergence.

#### What Phase 4 leaves Phase 5, in one place

- **The mid-block parts are not reachable through their blocks yet.** PAM waits behind U2A (Phase 6,
  vector engine); MINVBALL and RAYSCRL wait behind LNS&ZOOM and PLZPART (Phase 5). Their mappings
  are exact at the reference keyframes but their block positions in a real chain are not measured
  until the intervening parts are ported and the chain can run.
- **The part-level music pre-polls assume an entry-point run, and the mid-chain seam is scheduled.**
  MINVBALL's `init` pre-polls the run's player **256** times (`MUSIC_PRE_ROLL`), RAYSCRL's **268**
  and MNTSCRL's **11**, because an unanchored run starts the player with the part and the
  reference's player was already that far in. That is true for `--part` and for MNTSCRL as block
  3's entry, and false for a part reached mid-chain with the player already running: the first
  runnable block-3 chain (Phase 5's LNS&ZOOM/PLZPART) reaches MINVBALL and RAYSCRL with the player
  continuous since MNTSCRL, and their inits would advance it 256/268 polls past the reference's
  position (a poll a tick, so hundreds of ticks of music). PAM has the same shape at its open
  gate: `init` treats every gate-already-open start as the anchored 7710 start and sets
  `frame_count = 2`, while a
  block-1 chain reaching PAM after U2A meets the source's ordinary post-gate setup, which sets 0,
  and PAM would step one animation frame two ticks early. The seam is a `PartCtx` signal that the
  player was already running (or moving the entry pre-roll into the shell's per-part entry path);
  the chain test with the real player comes when the intervening parts land. The assumption is
  recorded in all four notes; no Phase 4 code changes.
- **The block-2 → block-3 alignment wait is still unmodelled.** `MAIN/U2.ASM:829-853` waits on
  `muscode` between PANICEND and MNTSCRL on a full run; MNTSCRL's release timing was measured from
  `SECOND 3`, whose entry restarts MUSIC1 at order 42 instead. A full run through block 2 will need
  the wait's own evidence.
- **The mid-run restarts remain unmodelled.** `:813-818` (MUSIC1 at order 0, `SECOND 1` only) and
  `:880-885` (MUSIC0 at order 18, before U2E) are reachable only once U2E/U2A and the block-2 parts
  exist; the one-row-per-run model still has no place for them (D13, `music.md` §12).
- **The packed release is still the only route to A17/A18/A19's mechanisms.** Unpacking
  `MNTSCRL.EXE`, `RAYSCRL.EXE`, `MINVBALL.EXE` or `ALKU.EXE` would turn each fitted constant into a
  readable condition (and settle A9's padding and MINVBALL's stack `j`); no unpacker was written.
- **A20's phase and the original's CPU cost are not modelled.** A fidelity pass that models the
  cost of `fmaketext`/`faddtext`/`prt` (or a capture at a different session phase) is the lead;
  until then 5859 is a measured residual.
- **The DDSTARS text decision is the frames', not the packed build's.** The reference and the
  tree's assembled-out state agree; a later unpacker that finds the shipped clip active would
  reopen D24 on this part, and the 1,025-pixel count is the discriminator.
- **A15's far band and `zinfo`'s non-zero values remain unmeasured.** A Phase-5 part that syncs
  above `0x8E2E` would meet the refusal; a part waiting on a non-zero `zinfo` would wait for ever
  against the port as it does against the reference. Neither has a caller yet.
- **The phase's instruments are reusable.** The `music trace` windows (four new), the per-recipe
  `MusicStart` rule (D30), the `PosList` decoder, the D19 support-frame shape and the per-part
  "mapping, not anchor" pattern are what Phase 5's six parts will use; a new part with a measured
  first frame gets an anchor, and one that begins in a window does not.
- **`GATE_ALIGNMENT_TICKS` stays Phase 3's.** No Phase-4 part inherits it; each carries its own
  measured timing (A17/A18) or a stated reason the one-tick difference is invisible (MINVBALL,
  D30). Phase 5 parts must make the same choice, not inherit a number.

#### The phase's records, and what this close-out changed

- `docs/decisions.md` gains **D30** — the block-1 clock at 400 and the per-recipe `MusicStart`, the
  anchor-only-for-a-measured-first-frame policy, and the no-inherited-cue-alignment rule. The
  phase's other release-versus-tree resolutions are D24's procedure applied per part, registered as
  A17–A21; D30 is the only new decision entry, and nothing else in `decisions.md` needed changing.
- `docs/fidelity/APPROXIMATIONS.md`: **A9's part label is corrected from Phase 5 to Phase 4** (the
  spec's phase table puts ALKU in Phase 4, and Phase 4 is where it was ported), and A9's closing
  sentence now says what Phase 4 settled (the 30-row drawing) and what it did not (the hidden
  padding bits). A15's "not measured until they are ported" is corrected to the measured windows.
  A17–A21 are the phase's approximations, each with its `**Part:** … Phase 4` line, and
  `minvball.md`'s misdirected "A17's lead" now points at A19's.
- `CLAUDE.md`'s `## Running it` is corrected (controller ruling R5): the port now has **twelve**
  registry names (the test card plus ALKU, PAM, BEGLOGO, TUNNELI, MNTSCRL, MINVBALL, RAYSCRL,
  DDSTARS, ENDLOGO, CRED and ENDSCRL), block 1 starts ALKU and stops at U2A, block 3 starts MNTSCRL
  and stops at LNS&ZOOM, block U runs DDSTARS, and blocks 2 and 5 are unchanged. The ESC sentence
  was corrected where it said no part polls the exit flag — the six Phase-4 parts read it where
  their sources call `dis_exit` and in their non-spinning waits — and the Ctrl and title-bar
  statements are unchanged and still accurate.
- `docs/fidelity/part-sequence.md` needed no change: Task 9 checked it against the code (run order
  §1, restart sites §2, block masks §4) and the phase added no sequence fact. The six part notes
  are complete, each with its original/port/substitution/measurement sections and its commands.

#### The whole-branch review, the fix wave, and the tag

**The review ran** over `02d3e77..7d6248c` (base..head) on **fable** and returned **With fixes:
0 Critical / 2 Important / 4 Minor**. The two Important findings were both record defects, not
behaviour: (1) the per-part entry pre-polls — MINVBALL's **256**, RAYSCRL's **268**, MNTSCRL's
**11** — and PAM's open-gate `frame_count = 2` assume an entry-point run and are wrong for a part
reached mid-chain with the player already running; the assumption is now recorded in all four
notes and scheduled in the Phase 5 hand-off (a `PartCtx` signal, or the pre-roll moved into the
shell's entry path), with the chain test to come when the intervening parts land; and (2)
MINVBALL's mapping row is regraded from "yes" to **half**: its `10,368 + tick` is pinned to ±1 by
the survey's 12,900 flat-grey frame — the fade-out's `c=59` upload, frame counter 2402/2403, a
different capture and a different observable — while the four keyframes the integration test
holds are the fit's own evidence. The falsification count is now **11 independent / 6 half /
5 none**. The fix wave is `f18823f`, and a scoped re-review confirmed every finding addressed with
**no new Critical or Important**. One residual is parked for the next doc pass: `alku.md:171`'s
"do-while" for a `for` in the source description, non-load-bearing and not a measurement.
**The tag is applied at this commit**, per Phase 1/2/3's practice.

## Phase log

### Phase 4 — Music-synced tier A (done, tag `phase-4`)

Ten tasks on branch `phase-4-music-synced-tier-a`, from `phase-3` (`3d42b88`) to this close-out
(20 commits; the first is Phase 3's human-listen record and the second the phase's plan).
**935 tests pass, zero failures, seven ignored, zero warnings** (Phase 3 closed at 806, three
ignored), and both clippy commands exit 0. Every number in the evidence section above was re-run at
the committed state.

**What was built.** Six parts under `bins/secondreality/src/parts/`, each a transliteration of its
source into the `Part` API: **ALKU** (three copper slots, the 640×200 picture windowed by CRTC start
and AC pan, the four `dis_sync` gates and the 9-tick scroll), **PAM** (the two-page 42-frame ANI
flip, the 4-tick cadence, the 64-copy palette wash), **MNTSCRL** and **RAYSCRL** (the shared `PosList`
decoder and two different blitters — additive with a +128 bias, colour-keyed — plus their
music-gated starts and `-11` fades), **MINVBALL** (Microsoft C's `rand`, the 512-dot projection, the
two assembly bugs, the frame cap) and **DDSTARS** (the 1024-star LCG, the 64-unit ring with the
`+17` second write, the text wipe, the two pages and planes). The shell gained the six names in
their blocks, `music_for_part`'s rows, `boot_anchor`'s measured anchors, and
`BLOCK_ONE_MUSIC_START_FRAME`. Four new music traces were filed (`second1`, `second1b`,
`second3b`, `hiddenu2`) and the trace test learned to phase-align a window. The phase's decisions
are **D30**; its approximations are **A17–A21**.

**The phase's method was the reference's own frames, and D24 decided four of the six parts.** Every
part is accepted against four reference keyframes with D19's support frames, at a mapping the note
states and the test asserts; every cue is driven both by a scripted player and, where the run
reaches it, by the real player against a filed trace. Where the release and the tree disagree,
D24's procedure ran per part: MNTSCRL's 71-tick wait and 640 `scp` bound, RAYSCRL's seven-tick
gates, and MINVBALL's Microsoft RNG follow the release (A17–A19), PAM's 7900 index plane is
registered rather than adopted (A21), and ALKU's three palette entries are adopted with the curated
entries' counterfactual measured (A20). The result is 17 of the 24 keyframes byte-identical, with
every residual asserted at its measured count.

**What it could not confirm, in one place.** The release's mechanisms (A17/A18/A19) are fitted to
frames the packed EXEs cannot confirm; MINVBALL's `j` is inside that packed image; A20's phase is
the original's CPU time, not modelled; MNTSCRL's cue has no reference frame after it; MINVBALL's
cue break is never reached (the cap precedes it); ALKU's `tptr` off-by-one is not separable from
A20; the padding rows of A9 stay unmeasured; and three of the seven ignored tests read git-ignored
scratch, so a fresh clone cannot re-run them. The four part-relative mappings are mappings, not
anchors (D30), and the mid-block parts are unreachable through their blocks until Phase 5/6 port
the parts before them. All of it is in the evidence section's negatives, item by item.

**The final whole-branch review ran** (fable, `02d3e77..7d6248c`): **With fixes** — 0 Critical /
2 Important / 4 Minor — fixed in `f18823f` and confirmed by a scoped re-review with no new
Critical or Important. **The tag is applied at this commit**, per Phase 1/2/3's practice.

### Phase 3 — Music engine (done, `phase-3`)

Nine tasks on branch `phase-3-music`, from `53eb9c1` to this commit (63 commits).
**806 tests pass, zero failures, three ignored, zero warnings** (Phase 2 closed at
590, three ignored), and both clippy commands exit 0. Every number in the evidence section above was
re-run at the committed state.

**What was built.** `sr-audio` from nothing: the S3M container read **in its shipped, scrambled
form** (the keystream is part of the port, not a conversion), the player (`STMIK.300`'s arithmetic
read out of the object's own rebuilt segment image, byte-asserted claim by claim), the thirteen
channel effects, and the mixer — the voices, the object's own volume table, and the **mis-built
output curve** the machine's memory holds. `sr-engine` gained the music seam (a `Music` trait with
the six DIS calls, the poll between copper slot 0 and the first band) and the shell gained the run's
music: `MUSIC_RESTARTS` with its `U2.ASM` citations, `MUSIC_START_FRAME`, `MusicStart`'s
wait/pre-poll rule, the SDL device as a **drain** with no stall and no resample, `--audio-dump` /
`--no-audio`, the status line, and `fademusic`. `xtask` gained the phase's instrument —
`music {symbols,trace,analyse,probe}`. ENDLOGO's gate became `END.C:107-111`'s own loop.

**The oracle is a new kind of reference, and it is the phase's method.** There is no audio reference
(D17), so the phase built one out of the original's *own player state*: `STMIK.300`'s `PUBDEF`
symbols locate the position words in guest memory, one request a frame reads them, and the port's
player — driven the same number of polls — has to agree frame by frame on the sequence of
`(order, row)`, `(speed, tempo)`, pattern, loop and `zplus`. Four sessions are filed under
`reference/music/`; `reference verify` knows the `music-trace` kind, recomputes each trace's coverage
from the file's own frames, and **refuses a kind it does not know by name**. The exit criterion the
plan made decidable — "the `(order, row)` cue timeline matches the parts' expectations" — is this
comparison plus ENDLOGO.

**The crux was measured, not argued, and it made the acceptance weaker than the brief's words.** The
two filed sessions of *one recipe* disagree with each other about which frame eight of their 131 row
changes land on, so no player can be frame-identical to both; the brief's literal demand was
unsatisfiable, and the implementer proved it with a test rather than an argument. The acceptance
asserts the strongest thing that is true instead — content exact, placement within the measured
spread — and the residual is **A12**.

**What the phase learned about its own rules, which is the part later phases inherit.** Three things
stand out. (1) **The artifact beats the note, and the phase applied that to itself**: `zplus`'s rule
was written in a note as a choice between two bits, the object turned out to **OR** them, and a shape
MUSIC1 actually carries (`[254, X, 254]` at orders 25/26/27) separates them — so the note was
amended, the tool's check followed it, and the traces were re-run to the same 0 mismatches, because
no filed trace holds the shape at all. (2) **The machine is a third witness**: two readings of the
output curve disagreed, a probe dumped the live segment, and the memory showed the builder's own
`rep stosw` bug — `CX` clobbered, the zero fill halved, the tail `0xFF`. Then a *review* was itself
refuted by a reviewer who re-ran the machine instead of re-reading the argument: the same table, byte
for byte, in two modules' runs. (3) **A claim is worth more as an asserted offset than as a
sentence**, and a **correction has to be swept rather than pointed**: three rounds fixed a corrected
claim at the site a reviewer named while copies stood in a module doc, a test's *name* and a note 130
lines below.

**And the phase's most-repeated defect has a mechanical cause, now written down.** Five times a
round's report described an edit as landed while the tree still held the old text; the fifth
instance's author found it — an **unchecked `str.replace` whose target did not match**, so the edit
silently no-opped while the summary was written from intent. Two `grep` traps compounded it (the
upstream `.ASM` files are not UTF-8, and this shell's `grep` is a wrapper that skips ignore-listed
paths) and between them produced two false "no caller exists" claims. **D29** records the rules that
follow: guard every substitution at the point of edit, name the search a claim rests on, and read the
tree rather than the report.

**What it could not confirm, in one place.** No audio reference exists, so the mixed sound is
verified by construction, invariants and a person's ears — never by a null test against the original
(the negations are listed in full in the evidence section). The twelve-frame boot transient is not
modelled. `zinfo` is 0 in every sample and its other values are unmeasured. `zplus`'s `2 → 0` return
and everything about a `255` are the object's code read out rather than the reference's behaviour
measured. `GATE_ALIGNMENT_TICKS` is an alignment whose value equals the difference the acceptance
absorbs, so the acceptance cannot falsify it. `dis_sync`'s far tier is refused and cannot be
answered. **The human listen is done and is recorded above**: the user heard the music through
`--audio-dump` and reports it "sounds amazing"; their real-device skips are the VM's audio path, not
the port (the same WAV is perfect on the host). The ENDLOGO-duration question was not separately
reported.

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

