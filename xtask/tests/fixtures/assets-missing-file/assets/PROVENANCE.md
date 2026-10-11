# Provenance of `assets/` — fixture

A fixture for `xtask/tests/assets.rs`: the provenance lists a file the tree does not hold.
`cargo xtask assets verify --tree` must report that as a failed check (**exit 1**) — a
provenance row with no file is a check that failed, not a tool that could not run.

| path | origin | commit | bytes | sha256 | licence | parts |
|---|---|---|---|---|---|---|
| `GONE.TXT` | `upstream/SecondReality/GONE.TXT` | `071a82e` | 12 | `1111111111111111111111111111111111111111111111111111111111111111` | Unlicense / public domain | ALKU |
