# Asset inventory — every asset the port's parts need, and what it is

Task A1 of Phase 1. This is the list every later task draws its test fixtures and its loader
work from: which files the 20 numbered parts (plus the hidden one, the shell and the setup
menu) load, in what format, and whether the bytes exist as a standalone file or only inside a
build blob.

**Source of record** — `upstream/SecondReality` at `071a82e`. Two things name an asset, per
the plan: the link inputs in a part's `MAKEFILE` (and the `.BAT`/`.LNK`/`.PRJ` equivalents
where a part has no makefile) and the filenames its own `.C`/`.ASM`/`.PAS` mention. Every row
below cites the line that names it. `MAIN/PACKFINA.INC:3-81` is the third source: it is what
the shipped `SECOND.EXE` carries, and §C cross-checks the three against each other.

**Format is decided by magic bytes, not by extension.** Where a file's leading bytes do not
match what its extension suggests, the row says so and §E records it as a finding. The
`0xFC…`/`UH`/`Uh` family's internal structure is established separately — the research report
at `.superpowers/sdd/01-assets/a2-format-research-report.md`, which task A2's brief requires
the implementer to read — so the rows below give each file's **literal leading bytes** and
what its header says, and name no family variant. The two documents then cannot drift apart,
and a file that fits no variant shows up here as literal bytes.

**Curated copies** live in `assets/`, mirrored from upstream; `assets/PROVENANCE.md` carries
their origin, digest and licence. Files whose bytes exist only inside a blob are the input to
task A5, which recovers them; §D lists them.

---

## A. The inventory

One row per asset. `form` is `standalone` when a file exists in the tree that the port can
read directly, `blob-only` when the bytes exist only inside an `.OBK`/`.OBJ`/`.EXE`.
`citation` quotes the line that names the asset, as `file:line`.

| part | upstream path | bytes | magic | header / description | form | citation |
|---|---|---|---|---|---|---|
| ALKU | `ALKU/FONA.UH` | 45784 | FC FC | Origin raw PIC: 1500x30, colours=256, pixels at paragraph 0x31 (784) | standalone | linked: ALKU/INCLUDE.ASM:17 INCLUDE 'fona.inc' |
| ALKU | `ALKU/FONA.LBM` | 15360 | FORM | IFF PBM  (FORM .... PBM ) | standalone | source art of ALKU/FONA.UH (floor list) |
| ALKU | `ALKU/FONA2.LBM` | 15268 | FORM | IFF PBM  (FORM .... PBM ) | standalone | second font strip (floor list) |
| ALKU | `ALKU/HOI.U` | 128784 | FC FC | Origin raw PIC: 640x200, colours=256, pixels at paragraph 0x31 (784) | standalone | not the file that is linked: `ALKU/INCLUDE.ASM:7-12` assembles `INCLUDE 'hoi.in0'` + `'hoi.in1'` as the `PUBLIC` symbol `hzpic`, and those bytes differ from this file in three palette entries (§E.18) |
| ALKU | `ALKU/HOI.LBM` | 33328 | FORM | IFF PBM  (FORM .... PBM ) | standalone | source art of ALKU/HOI.U |
| U2A | `VISU/C/SCENE/U2A.00M` | 796 | 46 43 FC 1A | scene material file: 'FC' then a u32 material-table offset at offset 4 (read as LONGAT(scene0+4), VISU/C/U2A.C:170) | standalone | runtime scene data: U2A.C:148 sprintf("%s.00M") and :150 readfile; object names at :178; the scene script at :206/:208; the animation names at :213/:215 |
| U2A | `VISU/C/SCENE/U2A.0AA` | 8 | 01 00 00 00 | 8 bytes = two 4-byte records of a u16 scene index and a u16 spare: index 1 (naming the .0AB scene) then the 0xFFFF end sentinel (the reader takes *ip and steps ip += 2 ints, VISU/C/U2A.C:208-219) | standalone | runtime scene data: U2A.C:148 sprintf("%s.00M") and :150 readfile; object names at :178; the scene script at :206/:208; the animation names at :213/:215 |
| U2A | `VISU/C/SCENE/U2A.0AB` | 16081 | 20 EA D8 23 | 16081 bytes of binary data, no magic | standalone | runtime scene data: U2A.C:148 sprintf("%s.00M") and :150 readfile; object names at :178; the scene script at :206/:208; the animation names at :213/:215 |
| U2A | `VISU/C/SCENE/U2A.001` | 8928 | VERS | Visu object file: VERS / NAME / VERT chunks, u32 lengths (VISU/C.H's format) | standalone | runtime scene data: U2A.C:148 sprintf("%s.00M") and :150 readfile; object names at :178; the scene script at :206/:208; the animation names at :213/:215 |
| U2A | `VISU/C/SCENE/U2A.002` | 7684 | VERS | Visu object file: VERS / NAME / VERT chunks, u32 lengths (VISU/C.H's format) | standalone | runtime scene data: U2A.C:148 sprintf("%s.00M") and :150 readfile; object names at :178; the scene script at :206/:208; the animation names at :213/:215 |
| U2A | `VISU/C/SCENE/U2A.003` | 2032 | VERS | Visu object file: VERS / NAME / VERT chunks, u32 lengths (VISU/C.H's format) | standalone | runtime scene data: U2A.C:148 sprintf("%s.00M") and :150 readfile; object names at :178; the scene script at :206/:208; the animation names at :213/:215 |
| U2E | `VISU/C/SCENE/U2E.00M` | 900 | 46 43 FC 1A | scene material file: 'FC' then a u32 material-table offset at offset 4 (read as LONGAT(scene0+4), VISU/C/U2E.C:214) | standalone | runtime scene data: U2E.C:208 sprintf("%s.00M") and :210 readfile; object names at :222; the scene script at :250/:252; the animation names at :257/:259 |
| U2E | `VISU/C/SCENE/U2E.0AA` | 8 | 01 00 00 00 | 8 bytes = two 4-byte records of a u16 scene index and a u16 spare: index 1 (naming the .0AB scene) then the 0xFFFF end sentinel (the reader takes *ip and steps ip += 2 ints, VISU/C/U2E.C:252-263) | standalone | runtime scene data: U2E.C:208 sprintf("%s.00M") and :210 readfile; object names at :222; the scene script at :250/:252; the animation names at :257/:259 |
| U2E | `VISU/C/SCENE/U2E.0AB` | 77069 | 20 EB FD 78 | 77069 bytes of binary data, no magic | standalone | runtime scene data: U2E.C:208 sprintf("%s.00M") and :210 readfile; object names at :222; the scene script at :250/:252; the animation names at :257/:259 |
| U2E | `VISU/C/SCENE/U2E.001` | 832 | VERS | Visu object file: VERS / NAME / VERT chunks, u32 lengths (VISU/C.H's format) | standalone | runtime scene data: U2E.C:208 sprintf("%s.00M") and :210 readfile; object names at :222; the scene script at :250/:252; the animation names at :257/:259 |
| U2E | `VISU/C/SCENE/U2E.002` | 1756 | VERS | Visu object file: VERS / NAME / VERT chunks, u32 lengths (VISU/C.H's format) | standalone | runtime scene data: U2E.C:208 sprintf("%s.00M") and :210 readfile; object names at :222; the scene script at :250/:252; the animation names at :257/:259 |
| U2E | `VISU/C/SCENE/U2E.003` | 836 | VERS | Visu object file: VERS / NAME / VERT chunks, u32 lengths (VISU/C.H's format) | standalone | runtime scene data: U2E.C:208 sprintf("%s.00M") and :210 readfile; object names at :222; the scene script at :250/:252; the animation names at :257/:259 |
| U2E | `VISU/C/SCENE/U2E.004` | 800 | VERS | Visu object file: VERS / NAME / VERT chunks, u32 lengths (VISU/C.H's format) | standalone | runtime scene data: U2E.C:208 sprintf("%s.00M") and :210 readfile; object names at :222; the scene script at :250/:252; the animation names at :257/:259 |
| U2E | `VISU/C/SCENE/U2E.005` | 916 | VERS | Visu object file: VERS / NAME / VERT chunks, u32 lengths (VISU/C.H's format) | standalone | runtime scene data: U2E.C:208 sprintf("%s.00M") and :210 readfile; object names at :222; the scene script at :250/:252; the animation names at :257/:259 |
| U2E | `VISU/C/SCENE/U2E.006` | 1292 | VERS | Visu object file: VERS / NAME / VERT chunks, u32 lengths (VISU/C.H's format) | standalone | runtime scene data: U2E.C:208 sprintf("%s.00M") and :210 readfile; object names at :222; the scene script at :250/:252; the animation names at :257/:259 |
| U2E | `VISU/C/SCENE/U2E.007` | 1024 | VERS | Visu object file: VERS / NAME / VERT chunks, u32 lengths (VISU/C.H's format) | standalone | runtime scene data: U2E.C:208 sprintf("%s.00M") and :210 readfile; object names at :222; the scene script at :250/:252; the animation names at :257/:259 |
| U2E | `VISU/C/SCENE/U2E.008` | 1260 | VERS | Visu object file: VERS / NAME / VERT chunks, u32 lengths (VISU/C.H's format) | standalone | runtime scene data: U2E.C:208 sprintf("%s.00M") and :210 readfile; object names at :222; the scene script at :250/:252; the animation names at :257/:259 |
| U2E | `VISU/C/SCENE/U2E.009` | 4044 | VERS | Visu object file: VERS / NAME / VERT chunks, u32 lengths (VISU/C.H's format) | standalone | runtime scene data: U2E.C:208 sprintf("%s.00M") and :210 readfile; object names at :222; the scene script at :250/:252; the animation names at :257/:259 |
| U2E | `VISU/C/SCENE/U2E.010` | 448 | VERS | Visu object file: VERS / NAME / VERT chunks, u32 lengths (VISU/C.H's format) | standalone | runtime scene data: U2E.C:208 sprintf("%s.00M") and :210 readfile; object names at :222; the scene script at :250/:252; the animation names at :257/:259 |
| U2E | `VISU/C/SCENE/U2E.011` | 532 | VERS | Visu object file: VERS / NAME / VERT chunks, u32 lengths (VISU/C.H's format) | standalone | runtime scene data: U2E.C:208 sprintf("%s.00M") and :210 readfile; object names at :222; the scene script at :250/:252; the animation names at :257/:259 |
| U2E | `VISU/C/SCENE/U2E.012` | 1404 | VERS | Visu object file: VERS / NAME / VERT chunks, u32 lengths (VISU/C.H's format) | standalone | runtime scene data: U2E.C:208 sprintf("%s.00M") and :210 readfile; object names at :222; the scene script at :250/:252; the animation names at :257/:259 |
| U2E | `VISU/C/SCENE/U2E.013` | 796 | VERS | Visu object file: VERS / NAME / VERT chunks, u32 lengths (VISU/C.H's format) | standalone | runtime scene data: U2E.C:208 sprintf("%s.00M") and :210 readfile; object names at :222; the scene script at :250/:252; the animation names at :257/:259 |
| U2E | `VISU/C/SCENE/U2E.014` | 564 | VERS | Visu object file: VERS / NAME / VERT chunks, u32 lengths (VISU/C.H's format) | standalone | runtime scene data: U2E.C:208 sprintf("%s.00M") and :210 readfile; object names at :222; the scene script at :250/:252; the animation names at :257/:259 |
| U2E | `VISU/C/SCENE/U2E.015` | 800 | VERS | Visu object file: VERS / NAME / VERT chunks, u32 lengths (VISU/C.H's format) | standalone | runtime scene data: U2E.C:208 sprintf("%s.00M") and :210 readfile; object names at :222; the scene script at :250/:252; the animation names at :257/:259 |
| U2E | `VISU/C/SCENE/U2E.016` | 448 | VERS | Visu object file: VERS / NAME / VERT chunks, u32 lengths (VISU/C.H's format) | standalone | runtime scene data: U2E.C:208 sprintf("%s.00M") and :210 readfile; object names at :222; the scene script at :250/:252; the animation names at :257/:259 |
| U2E | `VISU/C/SCENE/U2E.017` | 1600 | VERS | Visu object file: VERS / NAME / VERT chunks, u32 lengths (VISU/C.H's format) | standalone | runtime scene data: U2E.C:208 sprintf("%s.00M") and :210 readfile; object names at :222; the scene script at :250/:252; the animation names at :257/:259 |
| U2E | `VISU/C/SCENE/U2E.018` | 1432 | VERS | Visu object file: VERS / NAME / VERT chunks, u32 lengths (VISU/C.H's format) | standalone | runtime scene data: U2E.C:208 sprintf("%s.00M") and :210 readfile; object names at :222; the scene script at :250/:252; the animation names at :257/:259 |
| U2E | `VISU/C/SCENE/U2E.019` | 796 | VERS | Visu object file: VERS / NAME / VERT chunks, u32 lengths (VISU/C.H's format) | standalone | runtime scene data: U2E.C:208 sprintf("%s.00M") and :210 readfile; object names at :222; the scene script at :250/:252; the animation names at :257/:259 |
| U2E | `VISU/C/SCENE/U2E.020` | 756 | VERS | Visu object file: VERS / NAME / VERT chunks, u32 lengths (VISU/C.H's format) | standalone | runtime scene data: U2E.C:208 sprintf("%s.00M") and :210 readfile; object names at :222; the scene script at :250/:252; the animation names at :257/:259 |
| U2E | `VISU/C/SCENE/U2E.021` | 1760 | VERS | Visu object file: VERS / NAME / VERT chunks, u32 lengths (VISU/C.H's format) | standalone | runtime scene data: U2E.C:208 sprintf("%s.00M") and :210 readfile; object names at :222; the scene script at :250/:252; the animation names at :257/:259 |
| U2E | `VISU/C/SCENE/U2E.022` | 448 | VERS | Visu object file: VERS / NAME / VERT chunks, u32 lengths (VISU/C.H's format) | standalone | runtime scene data: U2E.C:208 sprintf("%s.00M") and :210 readfile; object names at :222; the scene script at :250/:252; the animation names at :257/:259 |
| U2E | `VISU/C/SCENE/U2E.023` | 12672 | VERS | Visu object file: VERS / NAME / VERT chunks, u32 lengths (VISU/C.H's format) | standalone | runtime scene data: U2E.C:208 sprintf("%s.00M") and :210 readfile; object names at :222; the scene script at :250/:252; the animation names at :257/:259 |
| U2E | `VISU/C/SCENE/U2E.024` | 3704 | VERS | Visu object file: VERS / NAME / VERT chunks, u32 lengths (VISU/C.H's format) | standalone | runtime scene data: U2E.C:208 sprintf("%s.00M") and :210 readfile; object names at :222; the scene script at :250/:252; the animation names at :257/:259 |
| U2E | `VISU/C/SCENE/U2E.025` | 796 | VERS | Visu object file: VERS / NAME / VERT chunks, u32 lengths (VISU/C.H's format) | standalone | runtime scene data: U2E.C:208 sprintf("%s.00M") and :210 readfile; object names at :222; the scene script at :250/:252; the animation names at :257/:259 |
| U2E | `VISU/C/SCENE/U2E.026` | 1252 | VERS | Visu object file: VERS / NAME / VERT chunks, u32 lengths (VISU/C.H's format) | standalone | runtime scene data: U2E.C:208 sprintf("%s.00M") and :210 readfile; object names at :222; the scene script at :250/:252; the animation names at :257/:259 |
| U2E | `VISU/C/SCENE/U2E.027` | 2212 | VERS | Visu object file: VERS / NAME / VERT chunks, u32 lengths (VISU/C.H's format) | standalone | runtime scene data: U2E.C:208 sprintf("%s.00M") and :210 readfile; object names at :222; the scene script at :250/:252; the animation names at :257/:259 |
| U2E | `VISU/C/SCENE/U2E.028` | 8968 | VERS | Visu object file: VERS / NAME / VERT chunks, u32 lengths (VISU/C.H's format) | standalone | runtime scene data: U2E.C:208 sprintf("%s.00M") and :210 readfile; object names at :222; the scene script at :250/:252; the animation names at :257/:259 |
| U2E | `VISU/C/SCENE/U2E.029` | 796 | VERS | Visu object file: VERS / NAME / VERT chunks, u32 lengths (VISU/C.H's format) | standalone | runtime scene data: U2E.C:208 sprintf("%s.00M") and :210 readfile; object names at :222; the scene script at :250/:252; the animation names at :257/:259 |
| U2E | `VISU/C/SCENE/U2E.030` | 1756 | VERS | Visu object file: VERS / NAME / VERT chunks, u32 lengths (VISU/C.H's format) | standalone | runtime scene data: U2E.C:208 sprintf("%s.00M") and :210 readfile; object names at :222; the scene script at :250/:252; the animation names at :257/:259 |
| U2E | `VISU/C/SCENE/U2E.031` | 448 | VERS | Visu object file: VERS / NAME / VERT chunks, u32 lengths (VISU/C.H's format) | standalone | runtime scene data: U2E.C:208 sprintf("%s.00M") and :210 readfile; object names at :222; the scene script at :250/:252; the animation names at :257/:259 |
| U2E | `VISU/C/SCENE/U2E.032` | 1572 | VERS | Visu object file: VERS / NAME / VERT chunks, u32 lengths (VISU/C.H's format) | standalone | runtime scene data: U2E.C:208 sprintf("%s.00M") and :210 readfile; object names at :222; the scene script at :250/:252; the animation names at :257/:259 |
| U2E | `VISU/C/SCENE/U2E.033` | 796 | VERS | Visu object file: VERS / NAME / VERT chunks, u32 lengths (VISU/C.H's format) | standalone | runtime scene data: U2E.C:208 sprintf("%s.00M") and :210 readfile; object names at :222; the scene script at :250/:252; the animation names at :257/:259 |
| U2E | `VISU/C/SCENE/U2E.034` | 4092 | VERS | Visu object file: VERS / NAME / VERT chunks, u32 lengths (VISU/C.H's format) | standalone | runtime scene data: U2E.C:208 sprintf("%s.00M") and :210 readfile; object names at :222; the scene script at :250/:252; the animation names at :257/:259 |
| U2E | `VISU/C/SCENE/U2E.035` | 1048 | VERS | Visu object file: VERS / NAME / VERT chunks, u32 lengths (VISU/C.H's format) | standalone | runtime scene data: U2E.C:208 sprintf("%s.00M") and :210 readfile; object names at :222; the scene script at :250/:252; the animation names at :257/:259 |
| U2E | `VISU/C/SCENE/U2E.036` | 3696 | VERS | Visu object file: VERS / NAME / VERT chunks, u32 lengths (VISU/C.H's format) | standalone | runtime scene data: U2E.C:208 sprintf("%s.00M") and :210 readfile; object names at :222; the scene script at :250/:252; the animation names at :257/:259 |
| U2E | `VISU/C/SCENE/U2E.037` | 2716 | VERS | Visu object file: VERS / NAME / VERT chunks, u32 lengths (VISU/C.H's format) | standalone | runtime scene data: U2E.C:208 sprintf("%s.00M") and :210 readfile; object names at :222; the scene script at :250/:252; the animation names at :257/:259 |
| U2E | `VISU/C/SCENE/U2E.038` | 3356 | VERS | Visu object file: VERS / NAME / VERT chunks, u32 lengths (VISU/C.H's format) | standalone | runtime scene data: U2E.C:208 sprintf("%s.00M") and :210 readfile; object names at :222; the scene script at :250/:252; the animation names at :257/:259 |
| U2E | `VISU/C/SCENE/U2E.039` | 1900 | VERS | Visu object file: VERS / NAME / VERT chunks, u32 lengths (VISU/C.H's format) | standalone | runtime scene data: U2E.C:208 sprintf("%s.00M") and :210 readfile; object names at :222; the scene script at :250/:252; the animation names at :257/:259 |
| U2E | `VISU/C/SCENE/U2E.040` | 1116 | VERS | Visu object file: VERS / NAME / VERT chunks, u32 lengths (VISU/C.H's format) | standalone | runtime scene data: U2E.C:208 sprintf("%s.00M") and :210 readfile; object names at :222; the scene script at :250/:252; the animation names at :257/:259 |
| U2E | `VISU/C/SCENE/U2E.041` | 796 | VERS | Visu object file: VERS / NAME / VERT chunks, u32 lengths (VISU/C.H's format) | standalone | runtime scene data: U2E.C:208 sprintf("%s.00M") and :210 readfile; object names at :222; the scene script at :250/:252; the animation names at :257/:259 |
| U2E | `VISU/C/SCENE/U2E.042` | 796 | VERS | Visu object file: VERS / NAME / VERT chunks, u32 lengths (VISU/C.H's format) | standalone | runtime scene data: U2E.C:208 sprintf("%s.00M") and :210 readfile; object names at :222; the scene script at :250/:252; the animation names at :257/:259 |
| U2A | `VISU/C/SCENE/U2A.LBM` | 5916 | FORM | IFF ILBM (FORM .... ILBM) | standalone | scene bitmap: the material palette is copied out of the .00M at offset 16 (U2A.C:243), and the viewer reads .MAT/.PAL beside it (VISU/C/MAIN.C:225-239) |
| U2A | `VISU/C/SCENE/U2A.MAT` | 304 | 3B 66 6F 72 | ASCII text | standalone | scene material list |
| U2A | `VISU/C/SCENE/U2A.INF` | 19 | 66 6F 76 20 | ASCII text | standalone | scene info text |
| U2E | `VISU/C/SCENE/U2E.LBM` | 5916 | FORM | IFF ILBM (FORM .... ILBM) | standalone | scene bitmap: the material palette is copied out of the .00M at offset 16 (U2E.C:337), and the viewer reads .MAT/.PAL beside it (VISU/C/MAIN.C:225-239) |
| U2E | `VISU/C/SCENE/U2E.MAT` | 480 | 3B 66 6F 72 | ASCII text | standalone | scene material list |
| U2E | `VISU/C/SCENE/U2E.INF` | 19 | 66 6F 76 20 | ASCII text | standalone | scene info text |
| U2A | `VISU/C/U2ABG.UH` | 64784 | Uh1 00 | Origin Uh1 PIC: header says 320x200 = 64000 pixels, file is 64784 bytes (no palette in the file) | standalone | linked: VISU/C/MAKEFILE:40 u2a.obj+_bg.obk |
| U2A | `VISU/C/U2ABG.LBM` | 19906 | FORM | IFF ILBM (FORM .... ILBM) | standalone | source art of VISU/C/U2ABG.UH |
| U2A | `VISU/C/U2A.PAL` | 768 | 00 00 00 01 | 768 bytes, no magic: a VGA palette | standalone | the city scene palette |
| U2E | `VISU/C/SCENE/U2E.PAL` | 768 | 00 00 00 0F | 768 bytes, no magic: a VGA palette | standalone | the U2E scene palette |
| PAM | `PAM/OUT.IN0` | 198707 | 0A 09 64 62 | ASCII INCBIN fragment (\tdb ... lines), not the binary; it parses to the bytes of the file it replaces | standalone | linked: PAM/INCLUDE.ASM:9 INCLUDE 'out.in0' |
| PAM | `PAM/OUT.IN1` | 192299 | 0A 09 64 62 | ASCII INCBIN fragment (\tdb ... lines), not the binary; it parses to the bytes of the file it replaces | standalone | linked: PAM/INCLUDE.ASM:14 INCLUDE 'out.in1' |
| PAM | `PAM/OUT.IN2` | 190153 | 0A 09 64 62 | ASCII INCBIN fragment (\tdb ... lines), not the binary; it parses to the bytes of the file it replaces | standalone | linked: PAM/INCLUDE.ASM:18 INCLUDE 'out.in2' |
| PAM | `PAM/OUT.IN3` | 189960 | 0A 09 64 62 | ASCII INCBIN fragment (\tdb ... lines), not the binary; it parses to the bytes of the file it replaces | standalone | linked: PAM/INCLUDE.ASM:22 INCLUDE 'out.in3' |
| PAM | `PAM/OUT.IN4` | 179997 | 0A 09 64 62 | ASCII INCBIN fragment (\tdb ... lines), not the binary; it parses to the bytes of the file it replaces | standalone | linked: PAM/INCLUDE.ASM:26 INCLUDE 'out.in4' |
| PAM | `PAM/OUT.ANI` | 324625 | 88 88 88 88 | the ANI delta stream PAM/ANIM.C's packframe() writes: no magic, frames back to back | standalone | the animation the OUT.IN* fragments carry |
| PAM | `PAM/PAL.PAL` | 768 | 00 00 00 02 | 768 bytes, no magic: a VGA palette | standalone | runtime palette: PAM/ANIM.C:19 fopen("pal.pal"); PAM/PAL.INC is its INCBIN text |
| PAM | `PAM/PRAX4.FLI` | 335476 | 74 1E 05 00 | Autodesk FLI animation: magic AF 11 at offset 4, 320x200; the first u32 is the file size | standalone | the FLI the animation was built from (task A6 reads it) |
| BEGLOGO | `BEG/SRTITLE.UP` | 44575 | FD FC | Origin RLE PIC: 320x400, colours=256, pixels at paragraph 0x31 (784) | standalone | linked: BEG/MAKEFILE:21 link ...+_pic.obk |
| BEGLOGO | `BEG/SRTITLE.LBM` | 53254 | FORM | IFF PBM  (FORM .... PBM ) | standalone | source art of BEG/SRTITLE.UP |
| GLENZ | `GLENZ/FC.UH` | 64784 | Uh1 00 | Origin Uh1 PIC: header says 320x200 = 64000 pixels, file is 64784 bytes (no palette in the file) | standalone | linked: GLENZ/MAKEFILE:3 _fc.obk |
| GLENZ | `GLENZ/FC.LBM` | 25552 | FORM | IFF PBM  (FORM .... PBM ) | standalone | source art of GLENZ/FC.UH |
| TUNNELI | `TUNNELI/TUNNEL.DAT` | 35328 | A0 00 6E 00 | 35328 bytes of binary data, no magic | standalone | linked: TUNNELI/TUN10.PAS:31 {$L tunneli.obj} |
| TUNNELI | `TUNNELI/SINIT.DAT` | 12292 | 00 00 00 00 | 12292 bytes of binary data, no magic, leading zeros | standalone | linked: TUNNELI/TUN10.PAS:33 {$L sinit.obj} |
| TECHNO | `TECHNO/TROLL.UP` | 70718 | FD FC | Origin RLE PIC: 320x400, colours=256, pixels at paragraph 0x31 (784) | standalone | runtime: TECHNO/KOE.C:605 open("troll.up") |
| TECHNO | `TECHNO/PANICPIC.LBM` | 82456 | FORM | IFF PBM  (FORM .... PBM ) | standalone | source art: TECHNO/DOPIC.BAT:1 lbm2p panicpic.lbm troll.up |
| TECHNO | `DDSTARS/PIC.EGA` | 24000 | AD 29 6B 4B | 24000 bytes of binary data, no magic | standalone | the standalone form of TECHNO/_CIRCLE.OBK's payload (byte-identical, 24,000 bytes): written by DDSTARS/DOIT1.C:30 and linked into TECHNO by TECHNO/MAKEFILE:6 (data=_circle.obk _circle2.obk) |
| PANICEND | `PANIC/MONSTER.U` | 64000 | 00 00 00 00 | 64000 bytes, no magic: an unheadered 320x200 screen | standalone | the part's picture (floor list) |
| PANICEND | `PANIC/MONSTER.PAL` | 768 | 00 00 00 3F | 768 bytes, no magic: a VGA palette | standalone | the part's palette (floor list) |
| MNTSCRL | `FOREST/O2.SCI` | 21258 | RIX3 | RIX3 screen: 640x32, palette at offset 10 | standalone | runtime: FOREST/READ2.PAS:110 assign(f,'o2.sci') |
| MNTSCRL | `FOREST/HILLBACK.CLX` | 64778 | RIX3 | RIX3 screen: 320x200, palette at offset 10 | standalone | linked: FOREST/MAKEOB.BAT:1 binobj hillback.clx hillback hback |
| MNTSCRL | `FOREST/POS1.DAT` | 25088 | 00 00 00 00 | 25088 bytes of binary data, no magic, leading zeros | standalone | linked: FOREST/AOS1.PAS:2 {$L pos1.obj} |
| MNTSCRL | `FOREST/POS2.DAT` | 25064 | 00 00 00 00 | 25064 bytes of binary data, no magic, leading zeros | standalone | linked: FOREST/AOS2.PAS:2 {$L pos2.obj} |
| MNTSCRL | `FOREST/POS3.DAT` | 25114 | 00 00 00 00 | 25114 bytes of binary data, no magic, leading zeros | standalone | linked: FOREST/AOS3.PAS:2 {$L pos3.obj} |
| LNS&ZOOM | `LENS/LENS.EX0` | 215 | 98 00 74 00 | 215 bytes of binary data, no magic | standalone | linked: LENS/MAKEFILE:4-5 _lensex0.obk |
| LNS&ZOOM | `LENS/LENS.EX1` | 19588 | E0 01 00 00 | 19588 bytes of binary data, no magic | standalone | linked: LENS/MAKEFILE:4-5 _lensex1.obk |
| LNS&ZOOM | `LENS/LENS.EX2` | 6860 | E0 01 00 00 | 6860 bytes of binary data, no magic | standalone | linked: LENS/MAKEFILE:4-5 _lensex2.obk |
| LNS&ZOOM | `LENS/LENS.EX3` | 1538 | E0 01 00 00 | 1538 bytes of binary data, no magic | standalone | linked: LENS/MAKEFILE:4-5 _lensex3.obk |
| LNS&ZOOM | `LENS/LENS.EX4` | 6252 | E0 01 15 00 | 6252 bytes of binary data, no magic | standalone | linked: LENS/MAKEFILE:4-5 _lensex4.obk |
| LNS&ZOOM | `LENS/LENS.EXB` | 64784 | Uh1 00 | Origin Uh1 PIC: header says 320x200 = 64000 pixels, file is 64784 bytes (no palette in the file) | standalone | linked: LENS/MAKEFILE:4 _lensexb.obk |
| LNS&ZOOM | `LENS/LENS.EXP` | 18864 | 00 00 96 05 | 18864 bytes of binary data, no magic | standalone | linked: LENS/MAKEFILE:5 _lensexp.obk |
| LNS&ZOOM | `LENS/FILLER` | 4224 | 21 21 21 21 | 4224 bytes of binary data, no magic | standalone | linked: LENS/MAKEFILE:4 _filler.obk (the 4K pad LENS/MAKEFILE:3 explains) |
| LNS&ZOOM | `LENS/LENSPIC.LBM` | 26708 | FORM | IFF PBM  (FORM .... PBM ) | standalone | source art of LENS.EXB: LENS/DOOBJS.BAT:2 glvp lenspic.lbm -xv! |
| LNS&ZOOM | `LENS/LENS.U` | 64000 | 00 00 00 00 | 64000 bytes, no magic: an unheadered 320x200 screen | standalone | the lens effect's source screen: LENS/CALC.C:66 fopen("lens.u") |
| RAYSCRL | `WATER/BKG.CLX` | 64778 | RIX3 | RIX3 screen: 320x200, palette at offset 10 | standalone | linked: WATER/BKR.PAS:2 {$L bkg.obj} |
| RAYSCRL | `WATER/MIEKKA.SCI` | 14778 | RIX3 | RIX3 screen: header claims 400x250, palette at offset 10; the payload is 14,000 bytes = 35 rows, so the picture is 400x35 — the header's height overclaims (§E.16) | standalone | linked: WATER/MIEK.PAS:2 {$L miekka.obj} |
| RAYSCRL | `WATER/WAT1.DAT` | 23096 | 00 00 00 00 | 23096 bytes of binary data, no magic, leading zeros | standalone | linked: WATER/T1.PAS:2 {$L wat1.obj} |
| RAYSCRL | `WATER/WAT2.DAT` | 23034 | 00 00 00 00 | 23034 bytes of binary data, no magic, leading zeros | standalone | linked: WATER/T2.PAS:2 {$L wat2.obj} |
| RAYSCRL | `WATER/WAT3.DAT` | 27818 | 00 00 00 00 | 27818 bytes of binary data, no magic, leading zeros | standalone | linked: WATER/T3.PAS:2 {$L wat3.obj} |
| 3DSINFLD | `COMAN/COMBG.UH` | 32768 | UH 01 00 | Origin UH PIC: header says 320x200 = 64000 pixels, file is 32768 bytes | standalone | linked: COMAN/_COMBG.OBK (COMAN/DOPIC.BAT:2) |
| 3DSINFLD | `COMAN/COMBG.X` | 64016 | UH 01 00 | Origin UH PIC: header says 320x200 = 64000 pixels, file is 64016 bytes | standalone | the palette-less variant of COMBG (floor list) |
| 3DSINFLD | `COMAN/COMBG.LBM` | 1796 | FORM | IFF PBM  (FORM .... PBM ) | standalone | source art: COMAN/DOPIC.BAT:1 lbm2u combg.lbm combg.uh |
| 3DSINFLD | `COMAN/W1DTA.BIN` | 65536 | 00 00 00 00 | 65536 bytes of binary data, no magic, leading zeros | standalone | linked: COMAN/MAKEFILE:4 dobjs; halves at COMAN/DOOBJS.BAT:1-2 |
| 3DSINFLD | `COMAN/W2DTA.BIN` | 65536 | 00 00 00 00 | 65536 bytes of binary data, no magic, leading zeros | standalone | linked: COMAN/MAKEFILE:4 dobjs; halves at COMAN/DOOBJS.BAT:7-8 |
| JPLOGO | `JPLOGO/ICEKNGDM.UP` | 58566 | FD FC | Origin RLE PIC: 320x400, colours=256, pixels at paragraph 0x31 (784) | standalone | linked: JPLOGO/MAKEFILE:23 +_pic.obk; the blob's payload is this file |
| JPLOGO | `JPLOGO/ICEKNGDM.LBM` | 66862 | FORM | IFF PBM  (FORM .... PBM ) | standalone | source art of JPLOGO/ICEKNGDM.UP |
| DDSTARS | `DDSTARS/TEXTS.16` | 16064 | FC FC | Origin raw PIC: 320x200, colours=16, pixels at paragraph 0x4 (64) | standalone | linked: DDSTARS/MAKEFILE:2 +_textpic.obk (DDSTARS/DOTEXTS.BAT:2) |
| DDSTARS | `DDSTARS/TEXTS.LBM` | 21784 | FORM | IFF PBM  (FORM .... PBM ) | standalone | source art: DDSTARS/DOTEXTS.BAT:1 lbm16 texts.lbm texts.16 2 |
| ENDLOGO | `END/PIC.UH` | 45884 | FD FC | Origin RLE PIC: 320x400, colours=256, pixels at paragraph 0x31 (784) | standalone | linked: END/MAKEFILE:5 _pic.obk (build recipe END/DOPIC.BAT:1-2) |
| ENDLOGO | `END/PIC.LBM` | 53576 | FORM | IFF PBM  (FORM .... PBM ) | standalone | source art of END/PIC.UH; NUTS.LBM is its twin |
| CRED | `CREDITS/PICS/PIC01.LBM` | 7878 | FORM | IFF ILBM (FORM .... ILBM) | standalone | linked: CREDITS/UNPACK.BAT:1-22 INCBIN.EXE PICS\PIC01.UH pic01.inc, included by CREDITS/INCLUDE.ASM and INCLUD2.ASM |
| CRED | `CREDITS/PICS/PIC02.LBM` | 7000 | FORM | IFF ILBM (FORM .... ILBM) | standalone | linked: CREDITS/UNPACK.BAT:1-22 INCBIN.EXE PICS\PIC02.UH pic02.inc, included by CREDITS/INCLUDE.ASM and INCLUD2.ASM |
| CRED | `CREDITS/PICS/PIC03.LBM` | 12078 | FORM | IFF ILBM (FORM .... ILBM) | standalone | linked: CREDITS/UNPACK.BAT:1-22 INCBIN.EXE PICS\PIC03.UH pic03.inc, included by CREDITS/INCLUDE.ASM and INCLUD2.ASM |
| CRED | `CREDITS/PICS/PIC04.LBM` | 12202 | FORM | IFF ILBM (FORM .... ILBM) | standalone | linked: CREDITS/UNPACK.BAT:1-22 INCBIN.EXE PICS\PIC04.UH pic04.inc, included by CREDITS/INCLUDE.ASM and INCLUD2.ASM |
| CRED | `CREDITS/PICS/PIC05.LBM` | 9962 | FORM | IFF ILBM (FORM .... ILBM) | standalone | linked: CREDITS/UNPACK.BAT:1-22 INCBIN.EXE PICS\PIC05.UH pic05.inc, included by CREDITS/INCLUDE.ASM and INCLUD2.ASM |
| CRED | `CREDITS/PICS/PIC05B.LBM` | 11524 | FORM | IFF ILBM (FORM .... ILBM) | standalone | linked: CREDITS/UNPACK.BAT:1-22 INCBIN.EXE PICS\PIC05B.UH pic05b.inc, included by CREDITS/INCLUDE.ASM and INCLUD2.ASM |
| CRED | `CREDITS/PICS/PIC06.LBM` | 15092 | FORM | IFF ILBM (FORM .... ILBM) | standalone | linked: CREDITS/UNPACK.BAT:1-22 INCBIN.EXE PICS\PIC06.UH pic06.inc, included by CREDITS/INCLUDE.ASM and INCLUD2.ASM |
| CRED | `CREDITS/PICS/PIC07.LBM` | 17400 | FORM | IFF ILBM (FORM .... ILBM) | standalone | linked: CREDITS/UNPACK.BAT:1-22 INCBIN.EXE PICS\PIC07.UH pic07.inc, included by CREDITS/INCLUDE.ASM and INCLUD2.ASM |
| CRED | `CREDITS/PICS/PIC08.LBM` | 17156 | FORM | IFF ILBM (FORM .... ILBM) | standalone | linked: CREDITS/UNPACK.BAT:1-22 INCBIN.EXE PICS\PIC08.UH pic08.inc, included by CREDITS/INCLUDE.ASM and INCLUD2.ASM |
| CRED | `CREDITS/PICS/PIC09.LBM` | 16474 | FORM | IFF ILBM (FORM .... ILBM) | standalone | linked: CREDITS/UNPACK.BAT:1-22 INCBIN.EXE PICS\PIC09.UH pic09.inc, included by CREDITS/INCLUDE.ASM and INCLUD2.ASM |
| CRED | `CREDITS/PICS/PIC10.LBM` | 12986 | FORM | IFF ILBM (FORM .... ILBM) | standalone | linked: CREDITS/UNPACK.BAT:1-22 INCBIN.EXE PICS\PIC10.UH pic10.inc, included by CREDITS/INCLUDE.ASM and INCLUD2.ASM |
| CRED | `CREDITS/PICS/PIC10B.LBM` | 16658 | FORM | IFF ILBM (FORM .... ILBM) | standalone | linked: CREDITS/UNPACK.BAT:1-22 INCBIN.EXE PICS\PIC10B.UH pic10b.inc, included by CREDITS/INCLUDE.ASM and INCLUD2.ASM |
| CRED | `CREDITS/PICS/PIC11.LBM` | 11754 | FORM | IFF ILBM (FORM .... ILBM) | standalone | linked: CREDITS/UNPACK.BAT:1-22 INCBIN.EXE PICS\PIC11.UH pic11.inc, included by CREDITS/INCLUDE.ASM and INCLUD2.ASM |
| CRED | `CREDITS/PICS/PIC12.LBM` | 13982 | FORM | IFF ILBM (FORM .... ILBM) | standalone | linked: CREDITS/UNPACK.BAT:1-22 INCBIN.EXE PICS\PIC12.UH pic12.inc, included by CREDITS/INCLUDE.ASM and INCLUD2.ASM |
| CRED | `CREDITS/PICS/PIC13.LBM` | 9762 | FORM | IFF ILBM (FORM .... ILBM) | standalone | linked: CREDITS/UNPACK.BAT:1-22 INCBIN.EXE PICS\PIC13.UH pic13.inc, included by CREDITS/INCLUDE.ASM and INCLUD2.ASM |
| CRED | `CREDITS/PICS/PIC14.LBM` | 15478 | FORM | IFF ILBM (FORM .... ILBM) | standalone | linked: CREDITS/UNPACK.BAT:1-22 INCBIN.EXE PICS\PIC14.UH pic14.inc, included by CREDITS/INCLUDE.ASM and INCLUD2.ASM |
| CRED | `CREDITS/PICS/PIC14B.LBM` | 8298 | FORM | IFF ILBM (FORM .... ILBM) | standalone | linked: CREDITS/UNPACK.BAT:1-22 INCBIN.EXE PICS\PIC14B.UH pic14b.inc, included by CREDITS/INCLUDE.ASM and INCLUD2.ASM |
| CRED | `CREDITS/PICS/PIC15.LBM` | 13408 | FORM | IFF ILBM (FORM .... ILBM) | standalone | linked: CREDITS/UNPACK.BAT:1-22 INCBIN.EXE PICS\PIC15.UH pic15.inc, included by CREDITS/INCLUDE.ASM and INCLUD2.ASM |
| CRED | `CREDITS/PICS/PIC16.LBM` | 5512 | FORM | IFF ILBM (FORM .... ILBM) | standalone | linked: CREDITS/UNPACK.BAT:1-22 INCBIN.EXE PICS\PIC16.UH pic16.inc, included by CREDITS/INCLUDE.ASM and INCLUD2.ASM |
| CRED | `CREDITS/PICS/PIC17.LBM` | 9182 | FORM | IFF ILBM (FORM .... ILBM) | standalone | linked: CREDITS/UNPACK.BAT:1-22 INCBIN.EXE PICS\PIC17.UH pic17.inc, included by CREDITS/INCLUDE.ASM and INCLUD2.ASM |
| CRED | `CREDITS/PICS/PIC18.LBM` | 4684 | FORM | IFF ILBM (FORM .... ILBM) | standalone | linked: CREDITS/UNPACK.BAT:1-22 INCBIN.EXE PICS\PIC18.UH pic18.inc, included by CREDITS/INCLUDE.ASM and INCLUD2.ASM |
| ENDSCRL | `ENDSCRL/FONA.UH` | 45784 | FC FC | Origin raw PIC: 1500x30, colours=256, pixels at paragraph 0x31 (784) | standalone | linked: ENDSCRL/ASMYT.ASM:73 INCLUDE 'fona.inc' |
| ENDSCRL | `ENDSCRL/FONA.LBM` | 9730 | FORM | IFF PBM  (FORM .... PBM ) | standalone | source art of ENDSCRL/FONA.UH (floor list) |
| ENDSCRL | `ENDSCRL/ENDSCROL.TXT` | 1007 | 4C 65 72 74 | ASCII text | standalone | the scroller text as it sat in the build directory: ENDSCRL/MAIN.C:101 open("endscrol.txt") |
| ENDSCRL | `MAIN/DATA/ENDSCROL.TXT` | 6141 | 53 65 63 6F | ASCII text | standalone | the shipped scroller text: MAIN/PACKFINA.INC:51 gives ENDSCROL.TXT as 0x17FD bytes, which is this file (6141), not ENDSCRL/ENDSCROL.TXT (1007) |
| MAIN | `MAIN/MUSIC0.S3M` | 391312 | SCRM @44 | Scream Tracker 3 module: SCRM at offset 44 the S3M magic (patterns XOR-scrambled, decision D5) | standalone | the first music module (decision D5) |
| MAIN | `MAIN/MUSIC1.S3M` | 600860 | SCRM @44 | Scream Tracker 3 module: SCRM at offset 44 the S3M magic (patterns XOR-scrambled, decision D5) | standalone | the second music module (decision D5) |
| MAIN | `MAIN/DATA/U2END.BIN` | 4000 | DA 08 C4 08 | 4000 bytes, no magic: 80 x 25 text-mode cells of (character, attribute), the shell's end screen (MAIN/U2.ASM:506) | standalone | the shell's end screen: named at MAIN/U2.ASM:506, packed at MAIN/PACKFINA.INC:49, copied to the text page by printend (MAIN/U2.ASM:511-535, called at :974) |
| START | `START/HZPIC.P` | 60706 | FD FC | Origin RLE PIC: 640x400, colours=256, pixels at paragraph 0x31 (784) | standalone | linked: START/MAKEFILE:3 _hzpic.obj (built by START/DOHZ.BAT:1) |
| START | `START/HZPIC.PAL` | 768 | 03 00 02 00 | 768 bytes, no magic: a VGA palette | standalone | the setup menu's palette (floor list) |
| START | `START/HZPIC.LBM` | 59352 | FORM | IFF PBM  (FORM .... PBM ) | standalone | source art: START/DOHZ.BAT:1 lbm2p hzpic.lbm |
| GRAB | `GRAB/BLOWFNT.1BM` | 64000 | 00 00 00 00 | 64000 bytes, no magic: an unheadered 320x200 screen | standalone | a 30-pixel font strip (floor list) |
| GLENZ | `GLENZ/EYE4.UH` | 64784 | Uh1 00 | Origin Uh1 PIC: header says 320x200 = 64000 pixels, file is 64784 bytes (no palette in the file) | standalone | the payload of GLENZ/_BGPIC.OBK (not on GLENZ/MAKEFILE:3's link line); GRID/_EYE.OBK carries the identical file, and GRID is Phase 8's vault part |

### A.2 Assets whose bytes exist only inside a build blob: one recovered, two answered

Task A5 walked these three with its OMF `LEDATA` reader; **one of them is now recovered**, so this
section is A5's result as much as A1's list. `bytes` is the size of the blob's `LEDATA` payload
(measured, §D), not of the blob.

| part | blob | payload bytes | magic | header / description | form | citation |
|---|---|---|---|---|---|---|
| TECHNO | `TECHNO/_CIRCLE2.OBK` | 8000 | `FE 00 07 FF` | **recovered by A5** as `assets/TECHNO/CIRCLE2.EGA`: a 320×200 one-bit mask (8,000 = 320×200/8), byte-identical to what the generator at `DDSTARS/DOIT2.C:21-52` writes | recovered | linked: `TECHNO/MAKEFILE:6` and `:23` link it into `koe.exe`; it sits in the shipped image (`TECHNO/KOE.MAP:11`, `DATA__circle2` = 0x1F40 bytes) and `TECHNO/KOEA.ASM:665` reads it |
| TECHNO | `TECHNO/_PIC1.OBK` | 50000 | `FD FC` | Origin RLE container, 320×400 by its header, but the stream stops 119 bytes into row 282 — **281 of the 400 rows** (§E.13); the loader refuses it as truncation | blob-only | **not linked**: `TECHNO/MAKEFILE:7` comments `_pic1.obk _pic2.obk` out |
| TECHNO | `TECHNO/_PIC2.OBK` | 20859 | `83 2E 2B 27` | no magic; A5 measured it as `TECHNO/TROLL.UP`'s last 20,859 bytes, byte for byte (70,718 = 49,859 + 20,859) | blob-only | **not linked**: `TECHNO/MAKEFILE:7` |

Every other blob in the tree whose payload is an asset has a standalone twin, and §A carries
that twin's row; §D names the pairing. The exception is `ALKU/INCLUDE.OBJ`, whose payload
differs from `ALKU/HOI.U` in three palette bytes (§E.18) — so this row's file is the curated
copy and the linked bytes are the `.IN` text, not the other way round. `TECHNO/_CIRCLE.OBK`
sat in this table until a reconstruction showed its payload **is** `DDSTARS/PIC.EGA` byte for
byte (`c42cf0062e81fa19…`, 24,000 bytes each on both sides), so its twin's row is in §A like
every other.

A5's walk covers **all 151 `.OBK`/`.OBJ` files**, not only the rows above; the pairs it found
are §D's, and the three payloads that are not a standalone file are §D's table there. The one
recovered file is the only entry in `assets/` with no upstream path (§G).

---

## B. Spec §9's dedup list, verified

`sha256sum` over each claimed group. A claim that does not hold is corrected here and carried
into §E as a finding.

| # | §9 claims | sha256 verdict | what actually holds |
|---|---|---|---|
| B.1 | `TMP.BBM` ×4 | **holds** | `BEG/TMP.BBM` = `END/TMP.BBM` = `ENDPIC/TMP.BBM` = `JPLOGO/TMP.BBM`, all 12,178 bytes, `1fa068ca…2dc77aef9c`. No part names the file. |
| B.2 | `ENDPIC/PIC.LBM` ≡ `JPLOGO/PIC.LBM` | **holds** | both 15,538 bytes, `a4f0705c…f056c9da`. |
| B.3 | `END/PIC.LBM` ≡ `NUTS.LBM` | **holds** | both 53,576 bytes, `fb5172fc…9fa0e67`. `END/DOPIC.BAT:1` names `pic.lbm`, so that is the canonical path. |
| B.4 | the U2A/U2E scene sets duplicated between `VISU/C/SCENE` and `MAIN/DATA` | **holds, and completely** | every one of the 51 files present in both directories is byte-identical: `U2A.00M/.0AA/.0AB/.001/.002/.003` and `U2E.00M/.0AA/.0AB/.001`–`.042`. `VISU/C/SCENE` additionally holds `.LBM`/`.MAT`/`.PAL`/`.INF`/`.PRJ`, which `MAIN/DATA` does not. `MAIN/DATA` additionally holds `U2A.EXE`, `U2E.EXE` and `U2END.BIN`. |
| B.5 | `TROLL.UP` ×2 | **holds** | `TECHNO/TROLL.UP` = `MAIN/DATA/TROLL.UP`, 70,718 bytes, `6a9fe2c5…07032e`. |
| B.6 | `O2.SCI` ×2 | **holds** | `FOREST/O2.SCI` = `MAIN/DATA/O2.SCI`, 21,258 bytes, `81693b53…4731f0`. |
| B.7 | `SIN1024.INC` ×9 | **does not hold** | nine files exist; **eight** are identical (`COMAN`, `DDSTARS`, `DOTS`, `GLENZ`, `HARD`, `LENS`, `TECHNO`, `TWIST`, 4,468 bytes, `fa6d0a6a…7ff8f959`) and the ninth, `JPLOGO/SIN1024.INC`, differs (4,468 bytes, `76ee2420…3b1261`). The count is right and the grouping is wrong. |

Three more duplicate sets turned up while checking these, none of them claimed by §9:
`ENDPIC/PIC.UH` ≡ `JPLOGO/PIC.UH` (64,784 bytes, `89e821e6…f6ba7844`) with their source art
`ENDPIC/PIC.LBM` ≡ `JPLOGO/PIC.LBM`; `GLENZ/EYE4.UH` ≡ `GRID/EYE4.UH` (64,784 bytes,
`a4972606…76b65f4c`); and a blob-vs-file pair, `TECHNO/_CIRCLE.OBK`'s payload ≡
`DDSTARS/PIC.EGA` (24,000 bytes, `c42cf0062e81fa19…`) — the same stream reached two different
parts by two different routes (`DDSTARS/DOIT1.C:30` writes it, `TECHNO/MAKEFILE:6` links it).
§E has the detail.

---

## C. Cross-check against the pack manifest

`MAIN/PACKFINA.INC:3-81` is the 79-entry table of what `SECOND.EXE` carries (line 1 is
`dw 79`; the 79 rows that follow each end in a `;NAME` comment, six `dw` fields each). Checked
mechanically rather than by eye: **79 rows, 79 names, no duplicate names, and every name
resolves to a file in `MAIN/DATA`.** 75 of the 79 size fields equal the staged file's size
exactly; the four that do not are all part EXEs, whose packed size the table records:

| entry | manifest size | file size in `MAIN/DATA` |
|---|---|---|
| `LNS&ZOOM.EXE` | 67,226 | 67,222 |
| `U2A.EXE` | 54,432 | 111,894 |
| `U2E.EXE` | 46,278 | 89,102 |
| `START.EXE` | 11,329 | 11,458 |

That is the staging difference part-sequence §1 already records (`U2A.EXE`/`U2E.EXE` are staged
uncompressed while the table holds their packed size), and `MAIN/PACK.C:199-204` patches the
first two bytes of every entry whose name contains `EXE`, so the table is a name list first and
a size list second.

The manifest confirms the three-way agreement this inventory rests on:

The 79 entries fall into exactly three groups, and they account for every one of them:

| manifest group | entries | agrees with |
|---|---|---|
| the 20 part EXEs (19 numbered + `DDSTARS`) and `START.EXE`, `STARTMUS.EXE` | 22 | `docs/fidelity/part-sequence.md` §1 (the 20 numbered parts + the hidden one) |
| `U2A.*` (6: `.00M`, `.0AA`, `.0AB`, `.001`–`.003`) and `U2E.*` (45: `.00M`, `.0AA`, `.0AB`, `.001`–`.042`) | 51 | the files §A lists under `VISU/C/SCENE` — byte-identical, §B.4 |
| `O2.SCI`, `TROLL.UP`, `ENDSCROL.TXT`, `U2END.BIN` and the two text files | 6 | the four assets `SECOND.EXE` itself carries: `O2.SCI` (MNTSCRL, `FOREST/READ2.PAS:110`), `TROLL.UP` (TECHNO, `TECHNO/KOE.C:605`), `ENDSCROL.TXT` (ENDSCRL, `ENDSCRL/MAIN.C:101`), `U2END.BIN` (the shell's end screen, `MAIN/U2.ASM:506`). The remaining two are the distributor's text files, read by the setup program's copy-to-disk step (`MAIN/START.C:40,53`) and displayed by nothing; §F says why they are not copied |

Two consequences worth stating:

- **Nothing else is in the shipped pack.** No `.LBM`, `.UH`, `.SCI`, `.PAL` or `.S3M` is
  packed: every picture a part shows is already compiled into its part EXE (as an `.OBK`
  blob's payload or an `INCBIN` fragment), and the only assets `SECOND.EXE` hands to a part
  at runtime are the four named above.
- `ENDSCROL.TXT` in the pack is 0x17FD = 6,141 bytes (`MAIN/PACKFINA.INC:51`), which is
  `MAIN/DATA/ENDSCROL.TXT`, **not** the 1,007-byte `ENDSCRL/ENDSCROL.TXT` that sits in the
  build directory. The two are different files; §E.9.

---

## D. Blobs: standalone file, or bytes that exist only in a build artifact

Only A5's `LEDATA` recovery has to know this, but every row in §A depends on it, so it was
measured rather than inferred.

Method: walk each `.OBK`/`.OBJ`'s OMF records (type byte, then a `u16` length), take the
`LEDATA` (0xA0) payloads in record order, and reconstruct the segment. Each `LEDATA` payload
is `segment:u16`, then a leading offset byte, then up to 1,024 bytes of data, then the same
offset byte again — the leading/trailing byte has to be stripped or the reconstruction drifts
by one byte per record. With that, the payload of every blob below reproduces its standalone
file **byte for byte** (`cmp`-identical):

| blob | payload | reproduces | standalone file |
|---|---|---|---|
| `BEG/_PIC.OBK` | 44,575 | `SRTITLE.UP` | `BEG/SRTITLE.UP` |
| `END/_PIC.OBK` | 45,884 | `PIC.UH` | `END/PIC.UH` |
| `ENDPIC/_PIC.OBK` | 6,936 | `SRTITLE.U` | `ENDPIC/SRTITLE.U` |
| `GLENZ/_FC.OBK` | 64,784 | `FC.UH` | `GLENZ/FC.UH` |
| `GLENZ/_BGPIC.OBK` | 64,784 | `EYE4.UH` | `GLENZ/EYE4.UH` (unlinked blob; the file is in `assets/` beside it) |
| `GRID/_EYE.OBK` | 64,784 | `EYE4.UH` | `GLENZ/EYE4.UH` (identical bytes; the curated copy lives there) |
| `JPLOGO/_PIC.OBK` | 58,566 | `ICEKNGDM.UP` | `JPLOGO/ICEKNGDM.UP` |
| `VISU/C/_BG.OBK` | 64,784 | `U2ABG.UH` | `VISU/C/U2ABG.UH` |
| `COMAN/_COMBG.OBK` | 32,768 | `COMBG.UH` | `COMAN/COMBG.UH` |
| `TECHNO/_CIRCLE.OBK` | 24,000 | `PIC.EGA` | `DDSTARS/PIC.EGA` (sha256 `c42cf0062e81fa19…` on both sides) |
| `DDSTARS/_TEXTPIC.OBK` | 16,064 | `TEXTS.16` | `DDSTARS/TEXTS.16` |
| `START/_HZPIC.OBJ` | 60,706 | `HZPIC.P` | `START/HZPIC.P` |
| `LENS/_LENSEXB.OBK` | 64,784 | `LENS.EXB` | `LENS/LENS.EXB` |
| `LENS/_LENSEXP.OBK` | 18,864 | `LENS.EXP` | `LENS/LENS.EXP` |
| `LENS/_LENSEX0`–`4.OBK` | 215 / 19,588 / 6,860 / 1,538 / 6,252 | `LENS.EX0`–`EX4` | `LENS/LENS.EX0`–`EX4` |
| `LENS/_FILLER.OBK` | 4,224 | `FILLER` | `LENS/FILLER` |
| `COMAN/_W1DTA.OBJ`, `_W2DTA.OBJ` | 65,536 | `W1DTA.BIN`, `W2DTA.BIN` | `COMAN/W1DTA.BIN`, `W2DTA.BIN` |
| `COMAN/_W1DTA1/2.OBJ`, `_W2DTA1/2.OBJ` | 32,768 each | the two halves of the file | as `COMAN/DOOBJS.BAT:1-2,7-8` says (`getslice … 0 32768`, `32768 32768`) |

**Blobs that hold bytes with no standalone twin** — three of them, the input to A5. The
reconstruction is A1's; the descriptions are A5's, taken from the reconstructed payloads:

| blob | payload | first bytes | what it is |
|---|---|---|---|
| `TECHNO/_CIRCLE2.OBK` | 8,000 | `FE 00 07 FF F0 00 1F FF` | linked by `TECHNO/MAKEFILE:6`; a **320×200 one-bit mask** (8,000 = 320×200/8), not a picture: A5 measured it as byte-identical to what the generator at `DDSTARS/DOIT2.C:21-52` writes, and recovered it as `assets/TECHNO/CIRCLE2.EGA` |
| `TECHNO/_PIC1.OBK` | 50,000 | `FD FC 40 01 90 01` | an Origin RLE container, 320×400 by its header, that **does not hold a complete picture**: 281 of the 400 rows decode and the stream ends 119 bytes into row 282 (§E.13); **not** linked (the `#_pic1.obk _pic2.obk` line in `TECHNO/MAKEFILE:7` is commented out) |
| `TECHNO/_PIC2.OBK` | 20,859 | `83 2E 2B 27` | no magic; A5 measured it as the last 20,859 bytes of `TECHNO/TROLL.UP`, byte for byte, so it carries a slice of an asset already curated; also not linked |

`DDSTARS/PIC.EGA` and `DDSTARS/PIC2.EGA` are both outputs of the `DDSTARS/DOIT*.C` tools, and
both are 24,000 and 8,000 bytes — the sizes of the two `_CIRCLE` payloads. The first **is**
`_CIRCLE.OBK`'s stream byte for byte, so it is in `assets/` as that asset's standalone form
(§A, `parts: TECHNO`); the second only looks as if it should be, and differs from
`_CIRCLE2.OBK` in 7,162 of its 8,000 bytes (A5 confirmed both counts). Size is not evidence:
hash both sides. A5 also settled which side of that pair is the stale one: `_CIRCLE2.OBK`'s
payload is exactly what the `DOIT2.C` in the tree generates, and `DDSTARS/PIC2.EGA` is not.

**A5's verification of this section.** `tests/omf.rs` reconstructs every row of the first table
above and asserts byte equality with the named file, plus the four `COMAN` half-blobs against
the slices `COMAN/DOOBJS.BAT:1-2,7-8` cuts; all hold. The same walk over **all 151 `.OBK`/`.OBJ`
files** (not only the rows above) found thirteen pairs beyond this table, the same relation on
files §A lists as standalone: `FOREST/HILLBACK.OBJ`→`HILLBACK.CLX`,
`FOREST/POS1–3.OBJ`→`POS1–3.DAT`, `MAIN/PROT/RUN1.OBJ`→`RUN1.BIN`,
`TUNNELI/SINIT.OBJ`→`SINIT.DAT`, `TUNNELI/TUNNELI.OBJ`→`TUNNEL.DAT`,
`WATER/BKG.OBJ`→`BKG.CLX`, `WATER/MIEKKA.OBJ`→`MIEKKA.SCI` and `WATER/WAT1–4.OBJ`→`WAT1–4.DAT`
— **35 blobs reconstruct a standalone file byte-for-byte in all.**

The walk also found that **not every blob payload is a standalone file**, and A5 records each
case rather than smoothing it over — three of them, four blobs:

| payload | what it is | where |
|---|---|---|
| `ALKU/INCLUDE.OBJ` | `decode(HOI.IN0)`+`decode(HOI.IN1)`+`decode(FONA.INC)` byte for byte; the picture part differs from `assets/ALKU/HOI.U` in exactly 9 bytes at offsets 19–27 — three palette entries | §E.18 |
| `WATER/MIEK.OBJ` | `WATER/MIEKKA.SCI` with 1 byte changed (offset 2,563); nothing links it — `WATER/MIEK.PAS:2` links `miekka.obj`, which is byte-exact | §E.19 |
| `CREDITS/INCLUDE.OBJ`, `CREDITS/INCLUD2.OBJ` | 12 and 9 raw `FC FC` 160×100 pictures at a 16,784-byte stride (plus the credits font in the first); no file in the tree holds those pictures' bytes | §E.20 |

Of the blobs the shipped parts link, four payloads are asset data that is not a standalone
file: the three in the table above, plus `TECHNO/_CIRCLE2.OBK`, which A5 recovered (§A.2).
Everything else a part links reproduces a standalone file byte for byte. (`TECHNO/_PIC1.OBK`
and `_PIC2.OBK` are not linked at all.)

---

## E. Findings

Each of these is something a later task must know before it writes a loader or a test.

1. **`PANIC/MONSTER.U`, `LENS/LENS.U` and `GRAB/BLOWFNT.1BM` are 64,000 bytes with no header
   at all** (first four bytes `00 00 00 00`). A 320×200 screen of one byte per pixel,
   palette not included: the part supplies it (`PANIC/MONSTER.PAL` sits beside the picture).
2. **`COMAN/COMBG.UH` is truncated, and it is a prefix of `COMAN/COMBG.X`.** Verified byte by
   byte: `COMBG.UH` (32,768 bytes) equals `COMBG.X[:32768]` exactly, and both headers are the
   same 16 bytes — `55 48 01 00`, 320×200, colours=0, `add`=0. `COMBG.X` is 64,016 = 16 +
   64,000, a complete 320×200 byte-per-pixel image; `COMBG.UH` carries 32,752 bytes after its
   header where the header implies 64,000, i.e. 102 whole 320-byte rows plus a 112-byte stub.
   **A loader that trusts the header must refuse it** (`Truncated`), and it must not be
   "repaired": the copy in `assets/` is the file as it shipped. Four further measured facts,
   because a later task will otherwise trip on them:
   - `COMAN/COMBG.LBM` is the 320×**90** source: its `BMHD` reads `w=320 h=90`, and its decoded
     pixels equal `COMBG.X[16 : 16+28800]` exactly. `COMBG.X`'s remaining 35,200 pixel bytes
     (rows 90–199) are all zero, so the truncation is not visible in the picture either file
     shows.
   - The furthest byte `COMAN/MAIN.C` reads is 29,582 — `79*4 + 89*320 + 2 + 768 + 16`, from
     the loops at `COMAN/MAIN.C:166-176` with the reads at `:163,170,174` — which is inside
     `COMBG.UH`. So the *linked* blob, whose payload is this same truncated file (§D), is big
     enough for the shipped part. The truncation is real and harmless at once; do not "fix"
     either file.
   - The palette read (`COMAN/MAIN.C:163`) takes `combg[736..783]`, and those bytes — along
     with the whole 768-byte palette region at offset 16 — are **zero** in both files. The
     part therefore sets 16 black entries. A fidelity question for Phase 5, not a decode
     failure.
   - Both files' `add` field (bytes 8–9) is 0, unlike the `0x31` the other shipped files in
     this family carry, so "pixels at 16" for these two rests on the file size and on the
     writer the archive implies, not on the header. The `55 48 01 00` magic alone does not say
     where the pixels start.

   The independent research report reaches the same conclusion at its §"outlier 2"; these
   numbers were measured separately, from the files.
3. **`VISU/C/SCENE/U2ABG.UH`, `GLENZ/FC.UH`, `LENS/LENS.EXB`, `GLENZ/EYE4.UH` and
   `ENDPIC/PIC.UH` ≡ `JPLOGO/PIC.UH` start `55 68 31 00` (`Uh1\0`)** — a different magic from
   `COMBG.*`'s `55 48 01 00` (`UH\1\0`) — and are 64,784 bytes for 64,000 pixels: 784 bytes of
   header and palette then the pixels.
4. **`DDSTARS/TEXTS.16` is `FC FC` with a 16-colour palette of 48 bytes and a 16,000-byte
   payload for 64,000 pixels** — four bits per pixel, so the `.16` extension tells the truth
   and the `0xFC…` family does not have one palette size. Header: 320×200, colours=16, pixels
   at paragraph 4 (offset 64 = 16 + 48).
5. **`PAM/PRAX4.FLI`'s magic is `AF 11`, not `AF 12`.** A classic Autodesk FLI (the plan for
   task A6 says `0xAF12`): the first `u32` is the file size (0x00051E74 = 335,476) and the
   magic sits at offset 4, followed by 320×200 at offsets 8 and 10. A6 must accept `AF 11`.
6. **`START/HZPIC.P` is the one file in this family that declares 640×400** (the others are
   320×200, 320×400, 640×200 or 1500×30) — and the reason `START/HZPIC.PAL` sits beside it as
   a separate file.
7. **`SIN1024.INC` is nine files, not nine identical files** (§B.7): `JPLOGO/SIN1024.INC`
   differs from the other eight. Nothing the port loads depends on a `.INC`; the sine table
   those parts use has to come from the file that part's directory actually included.
8. **The three `FONA` fonts are three different fonts**: `ALKU/FONA.UH` (45,784 bytes,
   1,500×30), `ENDSCRL/FONA.UH` (45,784 bytes, the same geometry, a different picture) and
   `CREDITS/FONA.INC`, whose `db` values parse to 60,000 bytes of raster — a 1,500×40 font,
   one row taller than the other two. `ENDSCRL/FONA.INC` parses to 45,000 values equal to `ENDSCRL/FONA.UH[784:]` — the pixel payload
   without its header and palette. `ALKU/FONA.INC` parses to 45,001 values equal to
   `ALKU/FONA.UH[783:]`, the same relation one byte earlier, so the two fragments are not
   interchangeable.
9. **`ENDSCRL/ENDSCROL.TXT` is not the text the demo scrolls.** It is 1,007 bytes of the
   developer's `Lerto has coded this hilariously fantastic…` test text; the pack entry is
   6,141 bytes (`MAIN/PACKFINA.INC:51`) and is `MAIN/DATA/ENDSCROL.TXT`, the real credits
   text. Both are in `assets/`, both are cited, and the port must use the second.
10. **Two builds of part 4 exist, and `assets/` holds neither `ENDPIC/`'s set nor a guess.**
    `ENDPIC/MAKEFILE:22` stages `beg.exe` as `beglogo.exe`, the same target name as
    `BEG/MAKEFILE:21`; part-sequence §1 says which one shipped is undetermined. `BEG/`'s files
    are copied (they are what the floor list names); `ENDPIC/`'s are not, because its picture
    files are `SRTITLE.U` (unique to it, 6,936 bytes, and the payload of its `_PIC.OBK`) plus
    `PIC.UH`/`PIC.LBM`, which are byte-identical to `JPLOGO/`'s unused copies — copying the
    directory as a set would put three duplicate files in the tree for no gain. Deciding which
    build shipped is Phase 2's question and it can be answered from `upstream/`.
11. **`MAIN/DATA/U2END.BIN` (4,000 bytes) is a DOS text-mode screen, and the shell prints it.**
    `MAIN/U2.ASM:506` names it (`endansiname db 'U2END.BIN',0`); `printend`
    (`MAIN/U2.ASM:511-535`, called at `:974`) reads exactly 4,000 bytes into a `getmem` block
    and `rep movsd`s them to the text page at `0xB8000`. 4,000 = 80 × 25 × 2, i.e. one text-mode
    page of (character, attribute) cells, and the first cell is `DA 08` — CP437 `┌` in dark
    grey. So: **no magic, no header, 80×25 CP437 text-mode cells, which the port's ending
    needs as a font-and-attributes image**, not as a bitmap.
12. **`MAIN/DATA/FCINFO10.TXT` and `MAIN/DATA/README.1ST` are packed but shown by nothing.**
    The setup program reads both and writes them back out (`MAIN/START.C:40,53`), a
    copy-to-disk step whose `fwrite` passes the already-closed `f1`, so even that write does
    nothing. They are the distributor's text files; the port has its own, so they are not in
    `assets/`.
13. **The two unlinked TECHNO blobs are build residue, and A5 measured why.** `_PIC1.OBK`
    holds an Origin RLE *container* — 320×400, 256 colours by its header — that is not a
    complete picture: the stream holds **281 of the 400 rows** and ends 119 bytes into row
    282 (its payload is exactly 50,000 bytes, so the cut is slice-shaped, not a full write).
    `sr_assets::load` therefore refuses it as `Truncated` rather than clipping it, the same
    rule `COMAN/COMBG.UH` is refused under (§E.2). `_PIC2.OBK`'s 20,859 bytes are
    `TECHNO/TROLL.UP`'s last 20,859 bytes, byte for byte (70,718 = 49,859 + 20,859). Both
    are tempting as "the techno picture" — the linked one is `TROLL.UP`, loaded at runtime
    from the pack (`TECHNO/KOE.C:605`) — and neither is needed: `TECHNO/MAKEFILE:7`
    comments `_pic1.obk _pic2.obk` out of the link.
14. **`GRAB/BLOWFNT.1BM` has no magic and no header** (64,000 bytes) yet the `.1BM` extension
    is an Amiga-style one. It is a font strip, one byte per pixel, 320×200 by its size; the
    port's font-strip task (A7) reads it with geometry supplied by the caller, exactly as it
    does for `ALKU/FONA.UH`.
15. **`LENS/LENS.EXP` has a one-word header**: `u16 0` at offset 0 then a `u16` count at
    offset 2, which is what `LENS/MAIN.C:296-298` reads (`*(int *)(lensexp+2)`). It is data
    the part generates (`LENS/MAIN.C:374` writes it) as well as data it links, so the copy in
    `assets/` is the shipped one.
16. **`WATER/MIEKKA.SCI`'s header overclaims its height: it declares 400x250 and carries 35
    rows.** The file is 14,778 bytes and its first ten bytes are `52 49 58 33 90 01 fa 00 af 00`
    — `RIX3`, width 400, height **250** — but the pixel payload is 14,778 − 778 = **14,000
    bytes, which is exactly 35 rows of 400, with no partial row**: the last pixel row ends on
    the file's last byte. It is a crop of the sword screen, not a truncation, and the crop
    relationship is exact: those 14,000 pixel bytes are `WATER/SWORD.SCI`'s pixels from offset
    778 for 14,000 bytes (that file being the same picture complete, 400x250, 100,778 bytes),
    and the 768 bytes at offset 10 are `WATER/SWORD.SCI`'s palette byte for byte — which is in
    turn `WATER/SWORD.LBM`'s `CMAP` divided by four, 768 of 768 bytes. (Both `SWORD` files are
    in the upstream tree only; §F records why they are not curated.) The part depends on the
    crop and not on the canvas: `WATER/MAKEOB.BAT:2` links this file (`binobj miekka.sci miekka
    _miekka`), `WATER/MIEKKA.OBJ`'s `LEDATA` payload is this file byte for byte, and
    `WATER/DEMO.PAS:109` reads `400*34` = 13,600 bytes from offset 778, which fits inside the
    14,000. **The RIX3 decoder loads this file as 400x35.** Compare §E.2: `COMAN/COMBG.UH` is
    the tree's other header-versus-payload disagreement, and it is a truncation whose cut does
    not land on a row boundary. Row count confirmed with

    ```sh
    python3 -I -c "import sys; d=open(sys.argv[1],'rb').read(); print('file', len(d), 'payload', len(d)-778, 'rows', (len(d)-778)//400, 'remainder', (len(d)-778)%400)" assets/WATER/MIEKKA.SCI
    # file 14778 payload 14000 rows 35 remainder 0
    ```
17. **The one asset recovered from a build blob: `TECHNO/_CIRCLE2.OBK` → `assets/TECHNO/CIRCLE2.EGA`.**
    It is a 320×200 **one-bit mask** — 8,000 bytes with no header, 8,000 = 320×200/8, high bit
    leftmost — and it is the shipped part's `_circle2`: `TECHNO/MAKEFILE:6` links the blob,
    `:23` links it into `koe.exe`, `TECHNO/KOE.MAP:11` sizes the segment (`DATA__circle2`,
    0x1F40 = 8,000) and `TECHNO/KOEA.ASM:646-745` (`init_interference`; the load is at `:665`)
    blits it into the EGA planes. Its bytes are exactly what the generator at
    `DDSTARS/DOIT2.C:21-52` writes — which
    is why the recovered file is named after the linker symbol, not after `DDSTARS/PIC2.EGA`,
    the stale file of that name that differs from it in 7,162 of 8,000 bytes. A later TECHNO
    task therefore needs a 1-bpp mask path, not the 8-bpp `raw_surface`; `assets/PROVENANCE.md`
    carries the file's origin as a blob and its records.
18. **`ALKU`'s shipped palette is not the curated `HOI.U`'s: the linked hzpic's entries 1–3 are
    three brightened greys — two of which occur at no index of `HOI.LBM`'s `CMAP` at all.**
    This is the tree's only *linked* payload whose bytes differ by content from the curated
    copy of the same art (the other divergence §D names, `WATER/MIEK.OBJ`, is unlinked), and
    it is A5's, found by comparing the reconstruction with the `.IN` text. `ALKU/INCLUDE.OBJ`'s
    payload (173,786 bytes) is `decode(HOI.IN0)` (65,535) + `decode(HOI.IN1)` (63,250) +
    `decode(FONA.INC)` (45,001) byte for byte; `ALKU/INCLUDE.ASM:7-12` assembles those
    fragments as the `PUBLIC` symbol `hzpic`. (`decode(HOI.IN0)+decode(HOI.IN1)` is one byte
    longer than `HOI.U`: its last byte is the `-1` that ends `HOI.IN1`, an INCBIN-text artifact of the same kind as PAM's
    fragments.) The picture bytes that follow are `assets/ALKU/HOI.U`'s with **exactly nine
    bytes changed, at offsets 19–27** — palette entries 1, 2 and 3:

    | entry | the linked bytes (shipped) | `assets/ALKU/HOI.U` | `HOI.LBM` `CMAP ÷ 4`, same index |
    |---|---|---|---|
    | 1 | `(10, 10, 10)` | `(2, 2, 3)` | `(2, 2, 3)` |
    | 2 | `(30, 30, 30)` | `(5, 5, 7)` | `(5, 5, 7)` |
    | 3 | `(63, 63, 63)` | `(6, 6, 9)` | `(6, 6, 9)` |

    `HOI.U`'s three entries are the LBM's `CMAP` divided by four at the same indices, so `HOI.U`
    is the ordinary conversion. **The shipped `(10, 10, 10)` and `(30, 30, 30)` occur at no
    index of that `CMAP ÷ 4` at all**, and the shipped `(63, 63, 63)` occurs only as the ramp's
    saturated white (indices 64–254), not at index 3 — so the shipped palette cannot be
    reproduced by re-quantising `HOI.LBM`, only by reading the `.IN` text (or the 9 bytes
    above). Which of the two an ALKU port should draw is a Phase 2 decision that needs
    `ALKU`'s own loader read; **A5 does not curate `HOI.IN0`/`HOI.IN1`**, and this table is the
    record so the values do not have to be re-derived.
19. **`WATER/MIEK.OBJ` is `MIEKKA.SCI` with one byte changed, and it is not the linked file.**
    Its payload is 14,778 bytes, identical to `WATER/MIEKKA.SCI` except at offset 2,563
    (`0x54` against `MIEKKA.SCI`'s `0x00`, i.e. one pixel byte in row 4 of the RIX3 payload).
    `WATER/MIEK.PAS:2` links `miekka.obj`, and `MIEKKA.OBJ` **is** byte-exact (§D); `MIEK.OBJ`
    is an intermediate nothing links, so the one differing byte cannot reach the shipped part.
20. **`CREDITS/INCLUDE.OBJ` and `CREDITS/INCLUD2.OBJ` hold the credits pictures raw, and no
    standalone file in the tree holds them.** Each payload is a run of `FC FC` 160×100
    256-colour pictures at a 16,784-byte stride (16 + 768 palette + 16,000 pixels, pixels at
    paragraph `0x31` = 784): 12 pictures in `INCLUDE.OBJ` (`CREDITS/INCLUDE.ASM:3-73`, PIC01,
    02, 03, 04, 05, 05B, 06, 07, 08, 09, 10, 10B) plus the credits font —
    `decode(CREDITS/FONA.INC)`, 60,000 bytes, the 1,500×40 raster §E.8 names — and 9 in
    `INCLUD2.OBJ` (`CREDITS/INCLUD2.ASM`, PIC11, 12, 13, 14, 14B, 15, 16, 17, 18). Each
    picture's raster is byte-identical to the decoded pixels of the matching
    `CREDITS/PICS/PICNN.LBM` (measured for all 21), but **no file in the tree holds any of the
    21 pictures' bytes**: the `.UH` files `CREDITS/UNPACK.BAT:1-22` names do not exist in the
    tree (only the `.LBM` source art does), and the `picNN.inc` intermediates are not built
    either. So these are the only pictures in the shipment whose container bytes exist nowhere
    but in the blobs — a fact for Phase 7 (CREDITS), which is the part that would need them;
    A5 records it and recovers nothing.

---

## F. Files present upstream but not copied, and why

The tree is large and most of it is not media. Recorded so that no later task has to wonder
whether a file was missed.

| what | why it is not in `assets/` |
|---|---|
| `.EXE`, `.OBJ`, `.OBK`, `.LIB`, `.TPU`, `.MAP`, `.LST`, `.DSK`, `.PRJ` | build artifacts. `.OBK`/`.OBJ` are the *containers* A5 reads; the assets inside them are already in `assets/` as standalone files, except the three rows in §D with no standalone twin — A5 recovered one of those as a standalone file (`assets/TECHNO/CIRCLE2.EGA`) and the other two are unlinked residue |
| `.C`, `.ASM`, `.PAS`, `.H`, `.INC`, `.BBK`, `.CUT`, `.PRE`, `.TPU`, `.LIB` | source and source-data fragments. The `INCBIN` fragments (`FONA.INC`, `PAL.INC`, the `picNN.inc` intermediate) are ASCII text of a binary that is in `assets/`; the port reads the binary |
| `ALKU/HOI.IN0`, `HOI.IN1` | the `INCBIN` text of `ALKU/HOI.U`, which is in `assets/`; the binary is what the port reads. **`PAM/OUT.IN0`–`OUT.IN4` are the exception and are copied** — task A6's plan names them, and they are PAM's shipped data rather than a redundant text form (§A) |
| `ALKU/HOIKKA.LBM`, `ALKU/PIC001.LBM`, `ALKU/RYPPIS.LBM`, `ALKU/U2-MOVIE.LBM` | working pictures in ALKU's directory that no ALKU source, makefile or project file names |
| `GLENZ/PIC001.LBM`, `GLENZ/PIC002.LBM`, `GLENZ/FCX.LBM`, `GLENZ/FC.BBK`, `GLENZ/EDGE.*`, `GLENZ/NEW.*`, `GLENZ/VEC.*` | GLENZ's working files and the EDGE tool's sources; none is linked or loaded |
| `GLENZ/_BGPIC.OBK` | an unused blob whose payload is `GLENZ/EYE4.UH`, the file next to it in `assets/` — nothing is lost by not copying the blob |
| `FOREST/*.LBM`, `FOREST/FONA*.LBM`, `FOREST/GREEN.CLX`, `FOREST/O.SCI`, `*.GIF`, `*.TPU` | MNTSCRL's working art and its `read`/`koe` tools' inputs: `READ2.PAS:110` opens `o2.sci` and its units link `hillback.obj`/`posN.obj`; nothing else in the directory is named by the part |
| `WATER/SWORD.SCI`, `WATER/RAY.SCI`, `WATER/GREEN.CLX`, `WATER/TAUSTA.LBM`, `WATER/FINAL*.LBM`, `WATER/*.LBM`, `WATER/DATA.*`, `WATER/MASK.MTV`, `WATER/WAT4.DAT`, `WATER/*.PAS`, `*.TPU` | the shipped part links `bkg.obj` (`BKR.PAS:2`), `miekka.obj` (`MIEK.PAS:2`) and `wat1..3.obj` (`T1/T2/T3.PAS:2`). `WAT4.DAT` is linked only by `T4.PAS`, which the shipped `DEMO.PAS:1` does not `uses` |
| `TUNNELI/COLORS.CLX`, `TUNNELI/TUNNELI.OBJ`, `TUNNELI/*.PAS` (the generators) | `COLORS.CLX` is a 320×200 RIX3 screen that no TUNNELI source names (checked across every `.PAS`, `.ASM` and the project files); `TUNNEL.DAT` and `SINIT.DAT` are the linked data |
| `LENS/G2.UH`, `LENS/MONSTER.LBM`, `LENS/LENS.LBM`, `LENS/LENSPIC1.LBM`, `LENS/LENSPICX.LBM` | not named by `LENS/MAIN.C`, `LENS/ASM.ASM`, `LENS/MAKEFILE` or `LENS/DOOBJS.BAT` |
| `PLZPART/*` | the part has no media file: its plasma tables are `.INC` fragments (`PSINI.INC`, `LSINI4/16.INC`, `SINIT.INC`, `TILE.INC`, `SPLINE.INC`, `RATA.INC`, `PTAU.INC`), linked through `ASMYT.ASM`/`INCLUDE.ASM`/`SPLINE.ASM` |
| `DOTS/*` | none: `DOTS/ASM.ASM:243` includes `face.inc`, which `DOTS/MAKEFILE:5-7` regenerates from `FACE.C` at build time, and `SIN1024.INC` is a source fragment |
| `CREDITS/TEST.LBM`, `CREDITS/TEST.INC` | a test picture and its fragment; neither is included by `CREDITS/INCLUDE.ASM` or `INCLUD2.ASM` |
| `CREDITS/PICS/PIC19.*` (absent), `CREDITS/UNPACK.BAT:22` | the batch file names `PICS\PIC19.UH`, which does not exist in the tree, and no `INCLUDE` uses `pic19.inc`. A dead line in a build script, not a missing asset |
| `DDSTARS/PIC2.EGA`, `DDSTARS/*.C` (the `DOIT` tools), `DDSTARS/TEST/` | outputs of the `DOIT*.C` tools, not inputs to the shipped part (`DDSTARS/MAKEFILE:2` links only `_textpic.obk` and `koe.obj`). `DDSTARS/PIC.EGA` is copied after all: it is `TECHNO/_CIRCLE.OBK`'s payload byte for byte, so the port needs it as TECHNO's data (§D) |
| `ENDPIC/*` | the alternate build of part 4 — §E.10 |
| `MAIN/DATA/*.EXE`, `MAIN/DISTR/*`, `MAIN/PROT/*`, `MAIN/TT.*`, `MAIN/U2*.BBK` | the pack's staged part binaries, the distributor's text files (`FCINFO10.TXT`, `README.1ST`, both cited in §C) and the protection tool's data |
| `MAIN/DATA/REALITY.FC` | the shipped music container: an 8-word header (patched by `MAIN/PACK.C:56-63`) followed by `MUSIC0.S3M` and `MUSIC1.S3M` verbatim. Both modules are in `assets/`; the container itself is not, because the port plays the modules (decision D5, `reference/original/PROVENANCE.md`) |
| `MAIN/PACKING.INC`, `MAIN/PACKING.INB`, `MAIN/PACKFINA.INC` | the pack manifest, cited in §C but not media |
| `GRID/*`, `HARD/*`, `TWIST/*` | the three unreleased development effects, which are Phase 8's (task B5's note, `docs/plans/01-assets.md`) and are not in `SECOND.EXE`'s manifest. `GRID/_EYE.OBK` embeds the identical bytes to `assets/GLENZ/EYE4.UH`, which is that pair's canonical copy (§B) |
| `3DS/*`, `PICS/*`, `UTIL/*`, `FCP/*`, `CODE/*`, `SCRIPT/*`, `DESIGN/*`, `IDEAS/*` | the party's tool, design and source directories: inputs to the original's own toolchain, not to the demo |

---

## G. How the curated tree stays honest

`assets/` is a copy, so it can be checked:

```sh
cd /home/ew/workspace/reality/SecondRealityRust
for f in $(find assets -type f ! -name PROVENANCE.md); do
  up="upstream/SecondReality/${f#assets/}"
  # The one recovered file (A5) has no upstream path; PROVENANCE.md carries its origin.
  [ -f "$up" ] || { echo "RECOVERED: $f"; continue; }
  cmp -s "$f" "$up" || echo "DIFFERS: $f vs $up"
done
echo "checked $(find assets -type f ! -name PROVENANCE.md | wc -l) files"
```

Every file in `assets/` has a path that exists upstream — **except the one task A5 recovered
from a blob**, whose origin is a record, not a path — which is what makes the loop one line
plus that guard. As of the review-fix commit the loop printed no `DIFFERS` lines and
`checked 149 files`; as of A5 it prints one `RECOVERED: assets/TECHNO/CIRCLE2.EGA`, still no
`DIFFERS` lines, and `checked 150 files`.
