# The reference oracle

The original retail distribution of Second Reality, retained unmodified. Every frame diff
in this project is measured against this binary. See `docs/decisions.md` D12 for why this
rather than a build from the source tree.

Do not edit, rename, patch or repack anything in this directory. A capture is only
reproducible if the bytes it ran against are the bytes recorded here.

| File | Size | SHA-256 |
|---|---|---|
| `SECOND.EXE` | 1,451,093 | `a0b637b1c6729aeee8874cb7de701f608fe6ec3028fd72652a962a3a2bd25d8f` |
| `REALITY.FC` | 992,188 | `cffb5839d0660f646c2474ff46452543997c3fc9469fa64401556c2d271aa63b` |

Recorded 1993-10-07. `README.1ST` is the distribution's own readme, kept because it is the
authoritative statement of the command line and the hardware the demo expected.

## Layout

The demo is exactly two files, and both must be in the same working directory —
`SECOND.EXE` loads `REALITY.FC` from the current directory. `xtask reference capture`
therefore takes a `--cwd` as well as an `--exe`.

## Command line

From `README.1ST`:

| Argument | Starts at |
|---|---|
| *(none)* or `1` | the very beginning |
| `2` | the title screen |
| `3` | the landscape scroller |
| `4` | the vector part |
| `5` | the end credits |
| `U` | the hidden part (undocumented; `checkcmdline` maps `u`/`U` to `partsmask[8]`) |

The demo is freeware, freely distributable without modification and never sold. It is
redistributed here unmodified, at no charge, and is only ever executed under emulation as
a measurement reference — never as part of the port. The port reads none of these files
at runtime.

## Cross-checks already done

`REALITY.FC` here is **byte-identical** to `upstream/SecondReality/MAIN/DATA/REALITY.FC`.
The media the source release ships is the shipped media, so the asset formats decoded in
Phase 1 are authentic rather than a revision.

The upstream source tree contains no runnable demo binary — only two different developer
builds of the shell, `MAIN/U2.EXE` and `U2.EXE`, plus unpacked parts in `MAIN/DATA/`.
Whether those parts are byte-identical to the ones packed inside `SECOND.EXE` is an open
question recorded in D12; it does not block anything before Phase 2.
