# Part sequence — transcription of `MAIN/U2.ASM`

This file is the reference for **what order the original runs its parts in**, and **where
the music is repositioned**. It replaces the survey-derived table that used to sit in
spec §2.2. Every claim here is a transcription of a line of source, not a recollection.

**Source of record:** `upstream/SecondReality` (submodule, pinned at `071a82e`,
`mtuomi/SecondReality`). File: `MAIN/U2.ASM`. Line numbers are 1-based, as printed by
`nl`/your editor, and refer to that file at that commit only. The file is CRLF
ISO-8859-1; the assembly listing is unchanged by that.

Spec §2.2 carries the same table in condensed form; where this file and §2.2 disagree,
**this file wins** (see `docs/decisions.md` D13 for what changed and why).

## 1. The main sequence

`Order` is the port's own sequence number, carried over from spec §2.2 (see §2 below for
the numbering scheme). The original numbers its part slots with assembler labels
`exe1` … `exe23`; those labels are quoted in **Source** so a reader can jump straight to
the definition and the call site.

`Directory` is the **source directory in the upstream tree**, not a runtime path — see
the note at the end of this section.

| Order | Binary | Directory | Dir. source | Effect (the original's own label) | Source (`MAIN/U2.ASM`) |
|---|---|---|---|---|---|
| — | START.EXE | MAIN | `MAIN/MAKEFILE:48-49` | setup menu (text mode): sound card, mixing rate, looping; not a show part | def `exe0` 58; run 703-705 |
| — | STARTMUS.EXE | MAIN | `MAIN/MAKEFILE:51-52` | music helper: loads module *n* out of `REALITY.FC` for `restartmus`, and unloads on exit; not a show part | def `exemus` 57; run 364-365, 959-960 |
| 1 | ALKU.EXE | ALKU | `ALKU/X.BAT:1` | `Alkutekstit I (WILDF)` — intro text I + horizon pan (CRTC start address) | def `exe1` 62; run 794-795 |
| 2 | U2A.EXE | VISU/C | `VISU/C/MAKEFILE:41` | `Alkutekstit II (PSI)` — intro text II | def `exe2` 64; run 796-797 |
| 3 | PAM.EXE | PAM | `PAM/PAM.PRJ` (see note) | `Alkutekstit III (TRUG/WILDF)` — animated intro picture | def `exe3a` 66; run 798-799 |
| 4 | BEGLOGO.EXE | BEG | `BEG/MAKEFILE:22` | (no label) — Future Crew logo fading in from white | def `exe3` 67; run 810-811 |
| 5 | GLENZ.EXE | GLENZ | `GLENZ/MAKEFILE:16` | `Glenz (PSI)` — translucent glenz logo | def `exe4` 69; run 820-821 |
| 6 | TUNNELI.EXE | TUNNELI | `TUNNELI/X.BAT:1` | `Dottitunneli (TRUG)` — dot tunnel | def `exe5` 71; run 822-823 |
| 7 | TECHNO.EXE | TECHNO | `TECHNO/MAKEFILE:24` | `Techno (PSI)` — techno bars + EGA interference | def `exe6` 73; run 824-825 |
| 8 | PANICEND.EXE | PANIC | `PANIC/X.BAT:1` | `Panicfake (WILDF)` — fake ending, screen shrinks to a dot | def `exe7` 75; run 826-827 |
| 9 | MNTSCRL.EXE | FOREST | `FOREST/X.BAT:1` | `Vuori-Scrolli (TRUG)` — mountain/landscape scroller | def `exe9` 78; run 865-866 |
| 10 | LNS&ZOOM.EXE | LENS | `LENS/MAKEFILE:26` | `Lens (PSI)` + `Rotazoomer (PSI)` — two effects in one binary | def `exe1112` 84; run 867-868 |
| 11 | PLZPART.EXE | PLZPART | `PLZPART/X.BAT:1` | `Plasma (WILDF)` + `Plasmacube (WILDF)` | def `exe1314` 87; run 869-870 |
| 12 | MINVBALL.EXE | DOTS | `DOTS/MAKEFILE:20` | `MiniVectorBalls (PSI)` | def `exe15` 89; run 871-872 |
| 13 | RAYSCRL.EXE | WATER | `WATER/X.BAT:1` | `Peilipalloscroll (TRUG)` — raytraced mirror-ball scroll | def `exe16` 91; run 873-874 |
| 14 | 3DSINFLD.EXE | COMAN | `COMAN/MAKEFILE:18` | `3D-Sinusfield (PSI)` — voxel terrain ("Comanche") | def `exe17` 93; run 875-876 |
| 15 | JPLOGO.EXE | JPLOGO | `JPLOGO/MAKEFILE:24` | `Jellypic (PSI)` — wobbling jelly logo | def `exe18` 95; run 877-878 |
| 16 | U2E.EXE | VISU/C | `VISU/C/MAKEFILE:45` | `Vector Part II` — space battle | def `exe19` 97; run 886-887 |
| 17 (hidden) | DDSTARS.EXE | DDSTARS | `DDSTARS/MAKEFILE:3` | `Desert Dream Stars (PSI)` — **hidden part**, see §5 | def `exehid` 60; run 895-896 |
| 18 | ENDLOGO.EXE | END | `END/MAKEFILE:22` | `Endpictureflash (?)` | def `exe2021` 101; run 910-911 |
| 19 | CRED.EXE | CREDITS | `CREDITS/CRED.PRJ`, `CREDITS/PICS/` | `Credits/Greetings scrl. (?)` — credits pictures | def `exe22` 102; run 912-913 |
| 20 | ENDSCRL.EXE | ENDSCRL | `ENDSCRL/ENDSCRL.PRJ` | `Credits/Greetings scrl. (?)` — greetings scroller | def `exe23` 103; run 916-917 |

**Effect column:** the text in backticks is the original's own part label, copied from the
commented-out `txtNN` strings in `U2.ASM` lines 61-100 (the same labels appear, in menu
order, in `MAIN/PARTS`). Where a row has no label, the source has none. The label
attributions in parentheses are the authors' (`WILDF`, `PSI`, `TRUG`). The trailing
descriptions ("dot tunnel", "landscape scroller", …) are the spec's, kept because they
are useful, and they are **descriptive, not transcribed** — the label is the citation.

**`MAIN/PARTS` is not this table.** It is a 20-entry label list in *menu* order, and it
differs from the run order: it has no entry for BEGLOGO, and its `txt9`/`txt16` labels
(`Peilipalloscroll` / `Vuori-Scrolli`) are swapped relative to the `U2.ASM` comments.
Do not use it to establish run order; use it only as a second copy of the labels.

**Run order within the runner** is fixed by the source text at lines 790-918 and is
*not* data-driven: the `mov si,OFFSET exeN / call partexecute` pairs appear in the order
listed above, every one of them guarded only by the `partsmask` bits (§4).

### The `Directory` column is a build-tree fact, not a runtime path

`U2.ASM` contains no directory information at all: the names are bare 8.3 filenames
(lines 57-103), and the routine that would have derived a directory from the executable
path is stubbed out — `file_getexepath PROC NEAR` / `ret` immediately, `MAIN/U2A.ASM:986-987`.
The shipped demo is flat: all 20 part EXEs (plus `START.EXE`, `STARTMUS.EXE`, `U2END.BIN`,
`ENDSCROL.TXT`, the `U2A.0xx`/`U2E.0xx` scene data, `O2.SCI`, `TROLL.UP`) are packed into
`SECOND.EXE` — the manifest is the 79-entry table at `MAIN/PACKFINA.INC:3-81`, whose
comments name every one of them. No part file exists on disk in the distribution:
`reference/original/README.1ST:34-38` lists five files, of which only `SECOND.EXE` and
`REALITY.FC` are the demo's, and the shell's own missing-file error (`U2.ASM:569-571`)
names only those two.

`Directory` therefore records where the part was *built*, taken from the `copy` step that
stages each built EXE into `MAIN/DATA/`, or from the part's own project file. Three rows
have no staging step in the tree (PAM, CRED, ENDSCRL) — their directory is established only
by the tree layout (the directory is named for the part and holds its sources), and that
column entry should be read as such.

Two build notes worth knowing:

- `ENDPIC/MAKEFILE:22` also stages `beg.exe` as `beglogo.exe`, so **two** directories
  contain a version of part 4. `ENDPIC/` holds `BEG.C` + `SRTITLE.LBM`/`PIC.LBM`, whose
  picture is linked in from `_PIC.OBK` (`:21`, and its payload is `SRTITLE.U` — every
  16-byte window of that file is in the OBK); `BEG/` holds its own picture plus its own
  `_pic.obk` blob. They are two different title pictures, not two packings of one: the two
  decoded `.UP`/`.U` payloads differ in 126,261 of their 128,000 pixels and in 255 of
  their 256 palette entries. Both build the same target name, and
  **`BEG/` is the one the 1993 release shipped** — settled in `docs/fidelity/beglogo.md`
  §1 by three measurements rather than by unpacking the binary: the reference's held frame
  900 probes 57 palette entries that `BEG/SRTITLE.UP`'s palette matches entry for entry and
  `ENDPIC/SRTITLE.U`'s matches none of; `SRTITLE.UP` decodes to that frame's 128,000
  indices exactly while `ENDPIC/SRTITLE.U` decodes to a different picture (126,261 of
  128,000 pixels apart); and the staged `MAIN/DATA/BEGLOGO.EXE` is 30,874 bytes, which a
  packer can produce from `BEG/BEG.EXE`'s 50,830 and cannot from `ENDPIC/BEG.EXE`'s
  13,286. The two builds are not interchangeable — `ENDPIC/BEG.C:18-20` sets its own video
  mode and `:45-48` waits on the music, where `BEG/BEG.C` calls `dis_partstart`,
  `dis_waitb` and nothing else — so part 4's port takes the `BEG/` reading throughout.
- Every part EXE staged in `MAIN/DATA/` — all 20, plus `START.EXE` — carries the PKLITE
  signature, except `U2A.EXE` and `U2E.EXE`, whose staged bytes are identical to
  `VISU/C/U2A.EXE` and `VISU/C/U2E.EXE` (checked by hash). That difference is compression,
  not a different build, so do not read `MAIN/DATA` sizes as build differences.

## 2. Numbering

- The port's `Order` numbers are its own and are kept so that phase plans and part notes
  stay stable. Slot **17 is reserved for the hidden part** — the hidden part is not in the
  main sequence but occupies that slot in the table so the ending keeps 18/19/20.
- The original's slot labels are `exe1` … `exe23` and contain **gaps and merges**:
  `exe8` never existed — its commented-out label `txt8` `Vector Part I - Space battle`
  (line 76) has no `exe` and no run site, so that part was cut; `exe10` (`DDSTARS.EXE`) is
  commented out at line 80; `exe11` and `exe13` do not exist either, the label at line 82
  being a bare `;see below` above the merged `exe1112` and `exe1314`; and `exe2021`
  merges `txt20` with `txt21`. So do not assume `exeN` implies order N.
- The port's 19 numbered parts + 1 hidden = 20 part EXEs, and the pack manifest
  `MAIN/PACKFINA.INC:3-81` names exactly those 20 (`ALKU`, `U2A`, `PAM`, `BEGLOGO`,
  `GLENZ`, `TUNNELI`, `TECHNO`, `PANICEND`, `MNTSCRL`, `LNS&ZOOM`, `PLZPART`, `MINVBALL`,
  `RAYSCRL`, `3DSINFLD`, `JPLOGO`, `U2E`, `DDSTARS`, `ENDLOGO`, `CRED`, `ENDSCRL`) plus
  `START.EXE` and `STARTMUS.EXE`.

## 3. Music restart points

The shell's music entry point is `restartmus` (`MAIN/U2.ASM:348-397`). Its three
arguments, established from the code and the STMIK interface:

| Arg | Register | Meaning | Evidence |
|---|---|---|---|
| module | `ax` (low byte at `msg+12`) | `0` = `MUSIC0`, `1` = `MUSIC1` | `STARTMUS.C:251-262` switches on that byte and calls `loadmodule2(0)` / `loadmodule2(1)`, whose commented-out neighbours are `//loadmodule("music0.s3m")` / `//loadmodule("music1.s3m")`; it also patches `module[50]=0x78` (tempo 120) for module 0 only, matching spec §2.5 |
| order | `bx` | the order to start playing at; `bx = -1` loads without starting | `MAIN/STMIK.H:43-44` — `extern int zplaysong(int order); /* this routine starts playing the module at order [order] */`; passed through at `U2.ASM:390-395` |
| speed adjust | `cx` | written to `_zspeedadjust2`, but **only when `_zoutputmode` is 7 (no sound) or 2 (Gravis)** — skipped for SB/SBPro | `U2.ASM:384-390` |

Every call site, in execution order. The "Guard" column is the `whattorun` bit test that
protects the call; `—` means unconditional.

Sites marked `[entry]` are *start-of-run* positions: they exist so that entering the demo
part-way reproduces the music position a full run would have reached, and they are
**skipped when the preceding blocks also ran**. This is the whole reason the table is
worth transcribing: the restart that a given part hears depends on the entry argument.

| Site (`MAIN/U2.ASM`) | Guard | Module | Order | Speed | When it fires |
|---|---|---|---|---|---|
| 767-775 + 785-786 | — | — | — | — | initial load and start of the run (see below) |
| 813-818 | `whattorun & 2` (the region `:801-802` opens, closed at `:855`) **and** `whattorun & 1` | MUSIC1 | 0 | 48700 | after BEGLOGO, immediately before GLENZ — only when block 1 ran: `SECOND 1`, or no argument |
| 860-863 `[entry]` | `whattorun & 4` **and not** `whattorun & 2` | MUSIC1 | 42 | 48700 | only `SECOND 3`; **not** on any run that played block 2 |
| 882-885 | `whattorun & 8` | MUSIC0 | 18 | 45000 | immediately before U2E — every run that includes block 3: no argument, `SECOND 1`..`SECOND 4` |
| 891-894 | `whattorun & 80h` | MUSIC0 | 70 | 45000 | only `SECOND U` |
| 902-905 `[entry]` | `whattorun & 16` **and not** `whattorun & 8` | MUSIC0 | 25 | 45000 | only `SECOND 5`; **not** on any run that played U2E |
| 929-932 | loop-back | MUSIC0 | 0 | 45000 | start of every pass after the first (loop mode) |

Initial load (lines 767-786) is **conditional on the entry argument**, which the spec did
not record:

```
767  mov ax,0 / mov bx,-1 / mov cx,45000      ; MUSIC0, load only (order -1 = don't play)
770  test cs:whattorun,1
771  jnz @@sndz                               ; block 1 is running: keep MUSIC0
772  mov ax,1 / mov bx,0 / mov cx,48700       ; block 1 skipped: MUSIC1, order 0, play it
775  @@sndz: call restartmus
786  call _zplaysong                         ; push 0 — plays the loaded module at order 0
```

So `SECOND 1` / no argument load **MUSIC0 and start it at order 0**; `SECOND 2..5` load
**MUSIC1 and start it at order 0** instead (line 786 re-issues order 0, which is where the
module already is). The line 786 `_zplaysong(0)` is unconditional — it is what actually
starts MUSIC0 on a full run, because the load at 767 deliberately passed `bx = -1`.

### What this means for the port

On a **full run** (`SECOND 1` or no argument), the music timeline is:

1. MUSIC0 at order 0, from before ALKU.
2. MUSIC1 at order 0, after BEGLOGO, i.e. from just before GLENZ.
3. MUSIC1 continues untouched through TUNNELI, TECHNO and PANICEND. After PANICEND the
   shell *waits for a music cue* (lines 829-853, §6) and then starts MNTSCRL with the
   music where it already is — **there is no restart here**; the wait is the alignment.
4. MUSIC0 at order 18, immediately before U2E.
5. MUSIC0 continues untouched through the hidden part's slot (skipped), ENDLOGO, CRED and
   ENDSCRL — **there is no restart in the ending either**.

The `[entry]` sites are what a full run is expected to have reached at those points:
order 42 of MUSIC1 at the mountain scroller, order 25 of MUSIC0 at the end picture. Their
guards (`not whattorun & 2`, `not whattorun & 8`) are exactly "we did not play the blocks
that led up to this point". Treat the correspondence as the evident intent; the port will
confirm the actual positions when the parts run against real captures.

## 4. Block arguments and `partsmask`

`checkcmdline` (`MAIN/U2.ASM:595-630`) walks the PSP command tail (`mov si,80h`, then
`inc si` before each read) and picks one byte out of the `partsmask` table:

| `partsmask` index | Source bytes | `whattorun` | Bits set | Reachable by |
|---|---|---|---|---|
| 0 | `11111b` (111) | `0x1f` | 0-4 | **no** — `'0'` is rejected (`cmp al,'0'` / `jle` at 611-612) |
| 1 | `11111b` (112) | `0x1f` | 0-4 | `SECOND 1` |
| 2 | `11110b` (113) | `0x1e` | 1-4 | `SECOND 2` |
| 3 | `11100b` (114) | `0x1c` | 2-4 | `SECOND 3` |
| 4 | `11000b` (115) | `0x18` | 3-4 | `SECOND 4` |
| 5 | `10000b` (116) | `0x10` | 4 | `SECOND 5` |
| 6 | `00000b` (117) | `0x00` | none | **no** — `'6'`..`'9'` rejected at 617-618 |
| 7 | `00000b` (118) | `0x00` | none | **no** |
| 8 | `80h` (119) | `0x80` | 7 | `SECOND U`, `SECOND u` (626-627: `mov al,8`) |
| 9 | `00000b` (120) | `0x00` | none | **no** |

Indices 0, 6, 7 and 9 are dead entries: there is no path from `checkcmdline` to them.

What each bit gates (`; &1=start, &2=logo..panic &4=mount..comanche &8=vector,
&16=scrollers`, `MAIN/U2.ASM:109`, plus the undocumented bit 7):

| Bit | Value | Parts gated | Test line |
|---|---|---|---|
| 0 | `1` | ALKU, U2A, PAM ("start") | 792 |
| 1 | `2` | BEGLOGO, GLENZ, TUNNELI, TECHNO, PANICEND ("logo..panic") | 801 |
| 2 | `4` | MNTSCRL, LNS&ZOOM, PLZPART, MINVBALL, RAYSCRL, 3DSINFLD, JPLOGO ("mount..comanche") | 856 |
| 3 | `8` | U2E ("vector") | 880 |
| 4 | `16` | ENDLOGO, CRED, ENDSCRL ("scrollers") | 898 |
| 7 | `80h` | DDSTARS — set **only** by `SECOND U` | 889 |

Default `whattorun` is `7fh` (`U2.ASM:108`) — bits 0-6 set, bit 7 clear. The runner never
tests bits 5 (`0x20`) or 6 (`0x40`), so the default and `partsmask[1]` (`0x1f`) behave
identically: **`SECOND 1` and no argument are equivalent**, which is what the shipped
README says.

### The README and the code disagree

`reference/original/README.1ST:66-79` (the distributor's description — what users read
and experienced):

| Arg | README says |
|---|---|
| `1` | "Starts the demo from the very beginning (the same as without any parameter)" |
| `2` | "Starts from the title screen" |
| `3` | "Starts from the landscape scroller" |
| `4` | "Starts from the vectorpart" |
| `5` | "Starts from the end credits" |

Cross-checked against the code, **each of the five agrees** with the block it selects:
`2` → first part is BEGLOGO (the Future Crew logo = the title screen); `3` → MNTSCRL (the
mountain scroller); `4` → U2E (the vector part); `5` → ENDLOGO/CRED/ENDSCRL (the ending);
`1` → everything, identical to no argument.

Three disagreements, in increasing order of importance:

1. **The README does not document `U`.** The code accepts `U`/`u` and it selects the
   hidden part, `partsmask[8]`, `whattorun = 0x80` (lines 613-616, 626-627). A user
   reading only the README would not know the hidden part exists. The code wins: it is
   what the binary does.
2. **The code's own error message contradicts the code.** `error2` (`MAIN/U2.ASM:557-559`)
   prints "execute \"SECOND #\" where # is a number between 2 and 5", but `SECOND 1` is
   accepted and documented. Treat the message as a wording bug in the original, not as a
   specification.
3. **`'/'`-prefixed arguments are silently ignored by `checkcmdline`** (lines 605-606,
   `cmp al,'/'` / `je @@f0`, falling into the bare `ret` at 629 — leaving `whattorun` at
   its default). The tail is then parsed by `START.EXE` itself, which understands exactly
   one switch, `/gravis` (`MAIN/START.C:89-92`: forces `m_looping = 2`,
   `m_soundcard = 3`, and skips the menu).
   So `SECOND /gravis` is a valid full-run invocation, and an unrecognised `/switch` is
   accepted by the shell and then ignored by the menu.

Anything else in the tail — `0`, `6`-`9`, or any other first character — sets `errorcode 2`
and aborts before the demo starts (line 628-629, printed at 557-559).

**Which the port follows:** the code, not the README. The port's CLI accepts `1`-`5` and
`U`/`u` with those exact masks, treats no argument and `1` identically, and ignores
unrecognised `/`-prefixed switches. The README's text is the *user-facing description*
and is worth keeping as documentation, but every behavioural question is answered by
`checkcmdline`. Record the `U` omission in the port's own user docs so the port does not
reproduce a documentation gap as a functional one. `/gravis` selects a Gravis card and
`m_looping = 2`; the port has no Gravis, so accept and ignore the switch rather than
rejecting it.

## 5. The hidden part

`DDSTARS.EXE` (`Desert Dream Stars`, PSI) is unreachable in a normal run:

- The default `whattorun` (`7fh`, line 108) has bit 7 clear, and `SECOND 1` / no argument
  is `0x1f` / `0x7f` — neither has bit 7.
- The runner's end-of-pass reset is `mov cs:whattorun,07fh` (line 919), which also clears
  bit 7, so loop mode never reaches it either.
- `SECOND U` sets `whattorun = 0x80`, which has **no other bit set**, so that invocation
  runs the hidden part *alone*: it loads MUSIC0 at order 70 (lines 891-894) and executes
  `exehid` (895-896). It is a standalone entry, not an append to a full run.
- In the runner it sits between the vector block (line 888 `@@skipp4`) and the scrollers
  block (line 898), i.e. after U2E and before ENDLOGO.

## 6. What happens between PANICEND and MNTSCRL

On any run that includes block 2, after PANICEND exits (line 827) the shell runs a
music-position wait before starting MNTSCRL (lines 829-853):

```
829  call ctrldown / jc @@skip5      ; Ctrl held skips the wait entirely
833  @@zh5: bx=1 int 0fch            ; fn 1 = waitb (DIS/DISINT.ASM:24)
835        bx=2 int 0fch / or ax,ax / jnz @@skip5   ; fn 2 = exit: any key aborts the wait
839        bx=6 int 0fch            ; fn 6 = muscode (DIS/DISINT.ASM:242-282)
841        cmp dx,0 / jge @@zh5     ; wait while dx >= 0
843  @@zh4: (same three calls)
851        cmp dx,0 / jg @@skip5    ; then wait while dx <= 0
```

The value compared is `dx`, computed by `muscode_6` from STMIK's `_np_zplus` and `_np_row`
(`DIS/DISINT.ASM:252-271`): `-32` when `_np_zplus == 0`, `row - 64` when `_np_zplus == 1`,
`row` when `_np_zplus == 2`. The net effect is "wait until the music passes a boundary and
then leave it", used to line the full run's MUSIC1 playback up with the mountain scroller.

**Not determined here, and not guessed:** the exact musical event that `_np_zplus`
encodes. The reading above is the literal behaviour of the code; which order/row the wait
actually lands on depends on STMIK's `_np_zplus` semantics, which the project has not yet
resolved — it is tracked as D8 in `docs/decisions.md`, with `_np_zinfo` named as the
exposure. Phase 3 should settle it from live memory rather than from inference.
What *is* certain: this wait is the only synchronisation before MNTSCRL on a full run,
and on `SECOND 3` the equivalent effect is produced by restarting MUSIC1 at order 42.

The wait is skipped entirely if Ctrl is held down when the block-2 section ends.

## 7. Loop mode

- **Entry point:** `@@partsagain:` — `MAIN/U2.ASM:790`. This is the top of the sequence
  (the first thing it does is `test cs:whattorun,1` for block 1), so "loop" re-enters the
  same code path a fresh run starts on, not a separate one.
- **Before re-entering,** at line 919, `whattorun` is reset: `mov cs:whattorun,07fh`. The
  reset happens on *every* pass, including the first. Consequence: after one pass, all of
  blocks 1-4 run even if the demo was started with `SECOND 3`. Bit 7 is not in `07fh`, so
  the hidden part never appears on a loop pass.
- **Conditions** (lines 920-925): re-enter only if Ctrl is not held **and** `looping != 0`
  **and** `looping != 2`.
- **Music:** `fademusic` with `cx = 1` (lines 926-928), then MUSIC0 at order 0, speed 45000
  (lines 929-932), then `jmp @@partsagain` (933). `fademusic` (lines 333-346) steps the
  module's **global-volume** byte `module[0x30]` — the S3M header's offset 0x30, **0x40** in
  both `MUSIC0.S3M` and `MUSIC1.S3M` — down by `cx` once a `dis_waitb` (`int 0fch` function 1,
  line 340), and stores 0 when it is done: at `cx = 1` the fade is **64** frames. (This note
  called it the tempo byte until Phase 9; it is not — the tempo is offset 0x32. Corrected in
  Phase 9 Task 4; `audio.rs`'s `fade_music` and D41 already walk the global volume.)
- **The port** (Phase 9 Task 4, D42): `--loop` is `looping` 1. `shell::run_plan` drops
  ENDSCRL; when the plan runs out the shell's loop seam holds the last part's picture
  (CRED's on every block but U) for the 64 fade ticks (`EXIT_FADE_TICKS`), installs
  `LOOP_RESTART` (MUSIC0, order 0) at the fade's end, holds `LOOP_RELOAD_TICKS` = **11**
  more ticks — the original's 9093–9103, from its fade's end (9092) to ALKU's first frame
  `L` (9104), `reference-indices.md` §25.3 — and starts pass 2 at ALKU with block 1's plan
  without ENDSCRL. The port's seam is 2 frames late against `C` (its CRED shows the closed
  iris on two ticks before it retires; `tests/loop.rs` pins it). `/gravis` is `looping` 2:
  no ENDSCRL, then `@@partsdone` (§9).
- `looping` is the setup menu's answer, read at lines 745-746 from `msg[6]`. `MENU.C:167-168`
  clamps the menu toggle to 0 or 1; the value **2** is set only by `/gravis`
  (`START.C:91`) and means "no menu, Gravis, and do not loop" — with 2 the demo skips the
  end scroller and then exits.
- **ENDSCRL is skipped whenever `looping != 0`** (lines 914-915), so with `/gravis` it is
  skipped even though the demo does not loop. The README's phrasing —
  "When looping, the end scroller will not be displayed" — is right for the normal case
  but not for `/gravis`.

## 8. What this file does not establish

- **The shipped binary's bytes were not disassembled.** This is a transcription of the
  source tree at `071a82e`. `reference/original/SECOND.EXE` is packed twice over
  (LZEXE + FCPROT: `MAIN/MAKEFILE:36-37, 44`), so it yields no readable strings and no
  filename table; confirming that the shipped build's runner matches this source would
  require unpacking it, which was not attempted. The pack manifest at
  `MAIN/PACKFINA.INC` does list exactly these 20 part EXEs plus the two helper programs,
  which corroborates the *set* of parts but not the ordering code.
- **`_np_zplus` / `_np_zinfo` semantics** (§6) — tracked as D8. Part 4 is no longer one of
  its dependents: `BEG/` shipped (§1), so BEGLOGO calls no music service at all.
- **The order-42 and order-25 correspondence** to where a full run's music actually is at
  those moments is the evident intent of the code (§3); it is not measured here.

## 9. How the demo ends: `@@partsdone` and `fatalexit`

`MAIN/U2.ASM:935-994`, transcribed, with what the port does at each line. `@@partsdone` is
reached from `:921`, `:923` or `:925` (§7): Ctrl held, `looping` 0, or `looping` 2.

| line(s) | the original | the port |
|---|---|---|
| 935-936 | `@@partsdone: mov cs:notextmode,0` | — (no text mode to keep or restore) |
| 937-939 | `mov cx,1; call fademusic` — 64 `dis_waitb`s of the global-volume fade (§7) | the exit stage's fade: the hold's first `EXIT_FADE_TICKS` = 64 ticks, the last part's picture up and the volume stepping down a tick (`Sequence::exit_stage`, `music_after_tick`; D41) |
| 940-942 | `_asmcopperdeinit`, `_zsoundoff` | nothing to stop: the player is at volume 0, and the run's end drops it |
| 943-960 | `@@exitnow`: hand the EMS page frame and handle to the music loader and run it with `ds:[bx+12] = -1`, "shut down all music" | — (no EMS, no separate loader) |
| 962-964 | `fatalexit: mov cs:notextmode,0; call vmode_deinit` | — |
| 965-968 | `fatalexit0: call dis_resetint; mov ax,3; int 10h` — the interrupt server off, text mode on | the exit stage's blank: every pixel index 0 through an all-black palette |
| 969-973 | `mov cx,20` / `@@cwb1: call copperwaitborder` / `loop @@cwb1` — twenty retraces | `EXIT_BLANK_TICKS` = 20 of the blank, then `Sequence::ended()` and the window closes |
| 974 | `call printend` — `U2END.BIN` into text-mode memory (`:506-535`) | `main` prints the box to the launching terminal, for a windowed demo run (D41) |
| 975-976 | `printdebug`, `file_resetint` | — |
| 977-987 | `fatalexit00`: restore the interrupt vector table from `zeropage` | — |
| 988 | `call printerror` — the error text, if `errorcode` is set | an error is the run's own `Err`, printed by `main` after the box |
| 989-992 | `IF DEBUG … int 3` | — |
| 993-994 | `mov ax,4c00h; int 21h` — return to DOS | the process exits |

**A looping run never reaches `@@partsdone`** except by Ctrl at `:920`; the port's Ctrl is
D21's one-part skip, so a `--loop` run is left by closing the window, and the box is printed
then (D42). `/gravis` (`looping` 2) reaches it after CRED, through the same exit stage as a
`looping` 0 run reaches after ENDSCRL.
