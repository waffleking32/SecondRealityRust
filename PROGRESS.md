# Progress

Phase status board. **Read this first if you are resuming work.**

- `not started` — no work done
- `in progress` — active; see the phase plan
- `done` — exit criteria met, evidence recorded, `phase-N` tag applied

| # | Phase | State | Plan | Tag |
|---|---|---|---|---|
| 0 | Foundation & harness | not started | [00-foundation.md](docs/plans/00-foundation.md) | — |
| 1 | Assets & references | not started | — | — |
| 2 | Shell + easy tier | not started | — | — |
| 3 | Music engine | not started | — | — |
| 4 | Music-synced tier A | not started | — | — |
| 5 | Mid tier | not started | — | — |
| 6 | Vector engine | not started | — | — |
| 7 | GLENZ | not started | — | — |
| 8 | Vault | not started | — | — |
| 9 | Assembly & fidelity pass | not started | — | — |

## Where to pick up

Nothing implemented yet. The design spec is approved:
`docs/spec/2026-10-10-second-reality-rust-design.md`.

**Next action:** execute `docs/plans/00-foundation.md` (14 tasks). Phase 0 delivers the
engine skeleton, the SDL2 presenter, and the reference-capture and comparison tooling —
no demo parts. Tasks 1–10 need no reference oracle; Tasks 11–13 do and are marked
`[oracle]` in the plan.

The reference oracle's input is `reference/original/` — the 1993 retail distribution
(`SECOND.EXE` + `REALITY.FC`), committed unmodified. See `docs/decisions.md` D12 and
`reference/original/PROVENANCE.md`. Do not modify those files: every frame diff in the
project is measured against those exact bytes.

Phase 3 has no dependency on Phase 2 and may be run in parallel if the music is worth
retiring first.

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
  `verdict: different`, **exit code 1**, diff PNGs at `out/neg/title/diff_000400.png`
  and `out/neg/part/diff_001100.png`.

A comparison that cannot run at all (a geometry or frame-count mismatch, a missing
manifest or frame file) is refused with a legible message and exit code 2, so a script
can tell "the frames differ" from "the tool could not tell".

## Phase log

(Append one entry per completed phase: what was built, evidence, deviations,
and anything that changes the approach.)
