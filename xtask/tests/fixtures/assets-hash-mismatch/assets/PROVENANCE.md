# Provenance of `assets/` — fixture

A fixture for `xtask/tests/assets.rs`: the shape of the real `assets/PROVENANCE.md`, with one
deliberate difference — the digest recorded for `BAD.TXT` is not the digest of the file beside
it. `cargo xtask assets verify --tree` must report that as a failed check (**exit 1**), not as a
tool failure and not as a pass.

| path | origin | commit | bytes | sha256 | licence | parts |
|---|---|---|---|---|---|---|
| `BAD.TXT` | `upstream/SecondReality/BAD.TXT` | `071a82e` | 58 | `0000000000000000000000000000000000000000000000000000000000000000` | Unlicense / public domain | ALKU |
