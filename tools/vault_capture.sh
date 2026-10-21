#!/usr/bin/env bash
#
# Stage one vault effect and capture it from the vendored upstream tree.
#
# The three unreleased effects are separate programs: each links the DIS client
# and needs the standalone DIS.EXE resident first. They never draw the demo's
# sound-card setup screen, so the capture boots them with
# `--boot autoexec --boot-wait graphics`: the guest runs `RUN.BAT` (DIS, then
# the effect) and the capture waits for the first graphics frame instead of the
# setup text (Phase 8 Task 1; `xtask/src/reference.rs`'s `BootWait`).
#
# GRID and HARD block until a key reaches DIS (`getch()` in GRID/MAIN.C:153,
# `while (dis_muscode(1) != 1 ...)` in HARD/B.C:138), and the standalone DIS
# only answers after `checkkeys` consumes one (`DIS/DISINT.ASM:112-157`). The
# 5 s AUTOTYPE key the autoexec recipe already writes is that key:
# `--keys "KBD_space"` schedules it before `RUN.BAT`, it fires after the effect
# has started, and nothing else is sent. TWIST has no such gate and takes no
# key.
#
# Usage:
#   tools/vault_capture.sh <grid|hard|twist> --frames A,B,... [--out DIR]
#                          [--stage DIR] [--sheet PATH]
#
#   --out    where the capture (frames + manifest.json) goes
#            (default: reference/parts/<part>, Task 2's home for it)
#   --stage  where DIS.EXE, the effect EXE and RUN.BAT are staged; made fresh
#            for every run (default: out/vault-stage/<part>)
#   --sheet  one PNG of the captured frames, for a person to look at
#            (the capture tool refuses a sheet inside reference/)
#
# `upstream/` and `reference/` are input-only: the script writes nothing there
# except the capture itself under --out. The smoke runs of Task 1 pass
# `--out out/vault-smoke/<part>`.
#
# Every binary is checked against its upstream SHA-256 before anything runs,
# so a capture cannot be taken from a tree that is not the pinned one.

set -euo pipefail

ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd -- "$ROOT"

# The pinned upstream binaries (Appendix B of the Phase 8 capture research;
# sizes 71,170 / 71,174 / 39,674 / 1,282 bytes).
DIS_SHA256=270f5fe2c7c7d17b4720c2019c2832f4bd3863f97e750315974692911f0ead08
GRID_SHA256=d4e7f1d6e1b5e5c3ba3fd85fe8f3645c1c99be1f0d200c791968c656b5334ff0
HARD_SHA256=1f8a7bdae29a34af42c3ffde63163661ca82a6884a01cb6eb775e81ac600c81d
TWIST_SHA256=88d5e695a485b87b33bce8646b67f843d410e0fa822e76d8fbbb76cc9d33aab8

usage() {
    cat <<'EOF'
Stage one vault effect and capture it from the vendored upstream tree.

Usage:
  tools/vault_capture.sh <grid|hard|twist> --frames A,B,... [--out DIR]
                         [--stage DIR] [--sheet PATH]

  --out    where the capture (frames + manifest.json) goes
           (default: reference/parts/<part>, Task 2's home for it)
  --stage  where DIS.EXE, the effect EXE and RUN.BAT are staged; made fresh
           for every run (default: out/vault-stage/<part>)
  --sheet  one PNG of the captured frames, for a person to look at
           (the capture tool refuses a sheet inside reference/)

`upstream/` and `reference/` are input-only: the script writes nothing there
except the capture itself under --out. The smoke runs of Task 1 pass
`--out out/vault-smoke/<part>`.
EOF
}

die() {
    echo "vault_capture: $*" >&2
    exit 2
}

sha256_of() {
    local actual
    actual=$(sha256sum -- "$1")
    printf '%s\n' "${actual%% *}"
}

verify_sha256() {
    local file=$1 expected=$2 actual
    [ -f "$file" ] || die "missing upstream binary: ${file#"$ROOT"/}"
    actual=$(sha256_of "$file")
    if [ "$actual" != "$expected" ]; then
        die "sha256 mismatch for ${file#"$ROOT"/}: expected $expected, got $actual"
    fi
}

case "${1:-}" in
    -h | --help)
        usage
        exit 0
        ;;
esac
[ $# -ge 1 ] || { usage >&2; exit 2; }
PART=$1
shift
case "$PART" in
    grid | hard | twist) ;;
    *) die "unknown vault part: $PART (expected grid, hard or twist)" ;;
esac

FRAMES=
OUT=
STAGE=
SHEET=
while [ $# -gt 0 ]; do
    case "$1" in
        --frames)
            [ $# -ge 2 ] || die "--frames needs a value, e.g. 400,500,600"
            FRAMES=$2
            shift 2
            ;;
        --out)
            [ $# -ge 2 ] || die "--out needs a directory"
            OUT=$2
            shift 2
            ;;
        --stage)
            [ $# -ge 2 ] || die "--stage needs a directory"
            STAGE=$2
            shift 2
            ;;
        --sheet)
            [ $# -ge 2 ] || die "--sheet needs a path"
            SHEET=$2
            shift 2
            ;;
        -h | --help)
            usage
            exit 0
            ;;
        *)
            die "unknown argument: $1 (see --help)"
            ;;
    esac
done

[ -n "$FRAMES" ] || die "--frames is required, e.g. --frames 400,500,600"
case "$FRAMES" in
    *[!0-9,]* | ,* | *, | *,,*) die "--frames must be comma-separated indices, got $FRAMES" ;;
esac

OUT=${OUT:-reference/parts/$PART}
STAGE=${STAGE:-out/vault-stage/$PART}

# The stage is deleted and re-made, so never let it name anything the
# repository owns: upstream/ and reference/ are input-only, and the root is not
# a staging directory.
STAGE_ABS=$(realpath -m -- "$STAGE")
ROOT_ABS=$(realpath -m -- "$ROOT")
[ "$STAGE_ABS" != "/" ] || die "refusing to use / as a staging directory"
[ "$STAGE_ABS" != "$ROOT_ABS" ] || die "refusing to use the repository root as a staging directory"
case "$STAGE_ABS/" in
    "$ROOT_ABS/upstream/"*) die "--stage must not be under upstream/ (input-only)" ;;
    "$ROOT_ABS/reference/"*) die "--stage must not be under reference/ (input-only)" ;;
esac

# The four shipped binaries, checked wherever they are used: all of them, so a
# wrong upstream tree is refused before the part is even staged.
verify_sha256 "$ROOT/upstream/SecondReality/DIS/DIS.EXE" "$DIS_SHA256"
verify_sha256 "$ROOT/upstream/SecondReality/GRID/P.EXE" "$GRID_SHA256"
verify_sha256 "$ROOT/upstream/SecondReality/HARD/B.EXE" "$HARD_SHA256"
verify_sha256 "$ROOT/upstream/SecondReality/TWIST/T.EXE" "$TWIST_SHA256"

case "$PART" in
    grid)
        EFFECT_DIR=GRID
        EFFECT=P.EXE
        WITH_KEY=1
        ;;
    hard)
        EFFECT_DIR=HARD
        EFFECT=B.EXE
        WITH_KEY=1
        ;;
    twist)
        EFFECT_DIR=TWIST
        EFFECT=T.EXE
        WITH_KEY=0
        ;;
esac

# A fresh, empty directory per run: a stale DIS or effect from another part
# would make the manifest name a program that was not the one staged.
rm -rf -- "$STAGE_ABS"
mkdir -p -- "$STAGE_ABS"

cp -- "$ROOT/upstream/SecondReality/DIS/DIS.EXE" "$STAGE_ABS/DIS.EXE"
cp -- "$ROOT/upstream/SecondReality/$EFFECT_DIR/$EFFECT" "$STAGE_ABS/$EFFECT"
# CRLF: the guest's shell is DOS, and a batch is a DOS file.
printf '@ECHO OFF\r\nDIS.EXE\r\n%s\r\n' "$EFFECT" >"$STAGE_ABS/RUN.BAT"

echo "vault_capture: staged $PART in ${STAGE_ABS#"$ROOT"/} (DIS.EXE + $EFFECT + RUN.BAT)"

args=(
    xtask reference capture
    --exe "$STAGE_ABS/RUN.BAT"
    --cwd "$STAGE_ABS"
    --boot autoexec
    --boot-wait graphics
    --cycles 30000
    --frames "$FRAMES"
    --out "$OUT"
)
if [ "$WITH_KEY" = 1 ]; then
    args+=(--keys KBD_space)
fi
if [ -n "$SHEET" ]; then
    args+=(--sheet "$SHEET")
fi

exec cargo "${args[@]}"
