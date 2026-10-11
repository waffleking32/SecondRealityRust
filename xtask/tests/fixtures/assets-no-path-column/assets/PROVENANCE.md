# Provenance of `assets/` — fixture

A fixture for `xtask/tests/assets.rs`: no table here has a `path` column, so there is no
provenance to check against. The real file's *Collapsed duplicates* and *Identical files that
are not copied at all* tables look like this — a `dropped path` column, a `sha256` column —
and they are records **about** the tree rather than rows **of** it. A tool that parsed by
position, or that took any `sha256` column as evidence, would read them as curated files.
This one must be refused as malformed (**exit 2**).

| dropped path | collapsed onto | sha256 | why it is safe to drop |
|---|---|---|---|
| `MAIN/DATA/O2.SCI` | `assets/FOREST/O2.SCI` | `81693b533a77bca4ededc840042486693920b108e1d032d46cab6f81df40386e` | the pack's staging copy |
