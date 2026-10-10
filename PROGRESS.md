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

Phase 3 has no dependency on Phase 2 and may be run in parallel if the music is worth
retiring first.

## Phase log

(Append one entry per completed phase: what was built, evidence, deviations,
and anything that changes the approach.)
