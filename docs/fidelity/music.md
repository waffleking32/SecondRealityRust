# MUSIC0 and MUSIC1 — the container, the keystream, and the data

Phase 3's two modules, `assets/MAIN/MUSIC0.S3M` (391,312 bytes) and `MUSIC1.S3M` (600,860 bytes),
read **in their shipped, scrambled form**: the descrambling is part of the port, not a conversion
step, and nothing here writes a decoded copy to disk (`CLAUDE.md`). Both files are byte-identical to
the copies embedded in `REALITY.FC` (D5), and `PROVENANCE.md` carries their digests.

This is the note's first half — the container and the data. What the *player* does with the data is
Tasks 3–5's and is decided by the reference's own behaviour (`docs/fidelity/music-clock.md`'s trace),
not by a reading of ST3 lore; §7 says which questions this half deliberately leaves open.

The code is `crates/sr-audio/src/module.rs` (the loader, `Cell`, `Pattern`, `Instrument`) and the
measurements are `crates/sr-audio/tests/module.rs`. Two commands print every number below — the first
the two modules decoded, the second the keystream checks and the loader's refusals:

```
cargo test -p sr-audio --test module -- --nocapture --test-threads=1
cargo test -p sr-audio --lib
```

The loader is a translation of the original's own, `MAIN/STARTEMS.C`'s `loads3mems` — the function
that read these two files in 1993 — so the layout below is that function's and not the generic S3M
documentation's.

Every quoted block below is a paste from one run of that first command, with two markings: `(…)` for
dropped lines, and each line with a `test <name> …` prefix stripped of it — that prefix is cargo's,
interleaved with the first line a test prints, and nothing else is removed.

## 1. The container, as the original's loader reads it

```
STARTEMS.C:50-68    struct ds_fileheader        0x60 bytes, #pragma pack(1)
STARTEMS.C:28-48    struct ds_smp               one 0x50-byte sample header per instrument
STARTEMS.C:333-334  parains = module + 0x60 + ordNum;  parapat = parains + insNum
STARTEMS.C:343-353  each pattern: read a word at ptr*16, then read ptr*16+2 for (word - 2) bytes
STARTEMS.C:367-393  each sample: only type 1; its bytes are at memseg<<4 | memseg2<<20, `length` of them
STARTEMS.C:129-177  checkins: the clamps (volume 64, c2spd 65535, length 64000) and the 512-byte pad
```

The two files, by their own bytes, and where their tables and first block sit:

```
MUSIC0.S3M: 391312 bytes, signature "SCRM" at 44, marker 26 at 28, type 16 at 29, title "UnreaL ][ - The 2ND Reality\0"
MUSIC1.S3M: 600860 bytes, signature "SCRM" at 44, marker 26 at 28, type 16 at 29, title "UnreaL ][ / PM \0\0\0\0\0\0\0\0\0\0\0\0\0"
(…)
MUSIC0.S3M: 80 orders at 0x60, 44 instrument pointers at 176, 77 pattern pointers at 264..418, the first pattern block at byte 3952
(…)
MUSIC1.S3M: 100 orders at 0x60, 54 instrument pointers at 196, 77 pattern pointers at 304..458, the first pattern block at byte 4784
```

so the container is one layout with two tables between the header and the blocks: the order list at
0x60, `insNum` instrument pointers next, then `patNum` pattern pointers — the offset the plan calls
`0x60 + ordNum + insNum*2`. It is a **byte** offset: `STARTEMS.C:293`'s `char far *module` makes
`parains` byte arithmetic and `parapat = parains + insNum` adds `insNum × 2` **bytes**
(`STARTEMS.C:333-334`), and the loader uses it that way. What is in paragraphs is each table
**entry**: `parapat[a] << 4` (`STARTEMS.C:347`).

### The block's length word includes its own two bytes

A pattern block is `ln` bytes from `ptr * 16`, and the packed data is the `ln - 2` bytes from
`ptr * 16 + 2` (`STARTEMS.C:348-351`: `read(h,&u,2)`, then `read(h,p+2,u-2)`). The evidence is that
**the blocks tile**: all 76 consecutive pairs in each module put the next block 0..15 bytes after
`ptr*16 + ln`, with every padding size from 0 to 15 occurring in both files. Pasted:

```
MUSIC0.S3M: 77 patterns, 76 consecutive pairs; the next block begins 0..15 bytes after ptr*16 + ln, present sizes [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15], every gap {0: 4, 1: 4, 2: 3, 3: 4, 4: 2, 5: 5, 6: 7, 7: 7, 8: 3, 9: 6, 10: 8, 11: 1, 12: 6, 13: 2, 14: 5, 15: 9}
MUSIC0.S3M: reading `ln` as the data's length instead breaks 8 of those 76 pairs
(…)
MUSIC1.S3M: 77 patterns, 76 consecutive pairs; the next block begins 0..15 bytes after ptr*16 + ln, present sizes [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15], every gap {0: 3, 1: 8, 2: 4, 3: 3, 4: 4, 5: 5, 6: 6, 7: 2, 8: 1, 9: 7, 10: 2, 11: 4, 12: 6, 13: 3, 14: 9, 15: 9}
MUSIC1.S3M: reading `ln` as the data's length instead breaks 11 of those 76 pairs
```

The second line of each pair is the counter-claim, computed in the same loop: the other reading —
`ln` taken as the data's own length — is not merely "also possible", it leaves 8 pairs in MUSIC0 and
11 in MUSIC1 that no longer tile at all (the gaps of 0 and 1 in the histograms above become −2 and
−1). That is how the mistake announces itself, and it is why the loader's arithmetic is pinned by a
test rather than by this sentence.

### The packed cell's framing

Three readings of the flag byte, each of which a wrong decoder can still "decode" with:

1. **A row ends on a byte that is zero.** The flag byte's low five bits are the *channel*, so the
   rule is `flag == 0` and not `flag & 0x1f == 0`: the modules carry **1,990 channel-0 cells in
   MUSIC0 and 2,835 in MUSIC1**, and the second rule would read every one of them as the end of its
   row and take the two bytes after it as the next row's cells instead. It would stop being a
   decoder and start being a shuffle, and it would still produce 64 rows of plausible-looking cells.
2. **Bit 5 means a note *and* an instrument**, two bytes in that order (`STARTEMS.C:343-353`'s
   unpacker in the player keeps the same pairing). Reading it as "a note, and work the instrument out
   some other way" desynchronises the rest of the pattern while still producing cells.
3. **The channel is the low five bits, and each row names a channel at most once.** Measured:

```
MUSIC0.S3M: 4928 rows, 8 cells in the widest row, highest channel 7, 1990 cells on channel 0, no row names a channel twice
MUSIC1.S3M: 4928 rows, 13 cells in the widest row, highest channel 14, 2835 cells on channel 0, no row names a channel twice
```

4,928 rows is 77 patterns × 64, and the channel-0 counts are item 1's evidence. The channels come in
strictly ascending order within every row, so "the order the stream names its cells" and "the order of
the channels" are the same order — which is what makes a 32-slot row a lossless representation of the
stream. The loader *refuses* a row that names one channel twice rather than dropping the second cell
silently; neither module does, so the refusal is a guard and not a behaviour (see
`a_row_that_names_one_channel_twice_is_rejected`).

## 2. The keystream, and what it verifies

D5's keystream, unchanged:

```
n      = (i >> 1) + 1
key(i) = (((n ^ (n >> 2)) << 3) | ((5*i + 2) & 7)) & 0xFF
```

applied as `data[ptr + 2 + i] ^= key(i)` for `i` in `0 .. ln-2`, restarting at index 0 for every
pattern. The keystream's first sixteen bytes are
`[10, 15, 20, 17, 30, 27, 40, 45, 34, 39, 60, 57, 54, 51, 80, 85]`, asserted by
`crates/sr-audio/src/module.rs`'s `the_keystream_is_d5s` against the formula written out again inside
the test, and `descramble_restarts_at_index_zero_for_every_pattern` checks the two properties the
restart needs: the scrambled bytes are the plaintext XOR the keystream, and applying it twice is the
identity.

**Every pattern of both modules decodes**:

```
MUSIC0.S3M: 77 of 77 patterns decode as exactly 64 rows consuming exactly their data; 14980 cells in all
MUSIC1.S3M: 77 of 77 patterns decode as exactly 64 rows consuming exactly their data; 17746 cells in all
```

"Consuming exactly their data" is the strong part, and the loader enforces it: 64 rows must end on
the block's last byte, no earlier and no later. D5 reported MUSIC1 as 76 of 77 because of one cell
whose command byte is 0, and records that 0 means *no command*; **where** that cell is, is this
task's find (§6). With 0 read that way — the reading D5 records and the one this loader takes,
leaving what a player makes of the cell to Task 3 — MUSIC1 is 77 of 77 as well.

The counts above are the loader's cells; the same test walks both modules a second time with its own
keystream, its own pointer arithmetic and its own cell reader (`Walk`, in the test file) and compares
the two cell by cell, so the number 14,980 and the number 17,746 are two readers agreeing rather than
one reader counting itself.

## 3. The two headers

```
MUSIC0.S3M: ordNum 80 insNum 44 patNum 77, global volume 64, initial speed 6, initial tempo 125, master volume 176 (stereo true, 8 channels enabled), channel table [0, 8, 1, 9, 2, 10, 3, 11, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255]
MUSIC1.S3M: ordNum 100 insNum 54 patNum 77, global volume 64, initial speed 3, initial tempo 130, master volume 176 (stereo true, 8 channels enabled), channel table [0, 8, 1, 9, 2, 10, 3, 11, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255]
```

The offsets, named: global volume 48, initial speed 49, initial tempo 50, master volume 51, channel
table 64..96 (`STARTEMS.C:63-67`). Master volume 176 = 0xB0: bit 7 set, so `stereo()` is true for
both files, and the loader keeps the byte whole rather than masking it down to the format's own
volume field. Both files' channel tables are the same bytes — `0, 8, 1, 9, 2, 10, 3, 11` and then 24
bytes of `255` — eight channels named in four interleaved pairs, the rest unused. The loader keeps
the table raw and does not decide what the numbers mean; the mixer is what reads them (Task 5).
MUSIC1's *patterns* reach channel 14 despite that table, which is the off-channel material D8
records.

**The tempo patch is MUSIC0's alone.** `MAIN/STARTMUS.C:257` writes `module[50] = 0x78` and `:252-265`
puts that line in `case 0`, which is MUSIC0's load (`loadmodule2(0)`, `:255`); MUSIC1's `case 1`
(`:259-261`) has no patch. The test applies it where the shell does:

```
after the shell's patch: MUSIC0's tempo 120 (from 125), MUSIC1's tempo 130 (never patched) — and MUSIC0's speed 6, global volume 64 and master volume 176 are where they were
```

so the file's own 125 is what `Module::load` reports, 120 is what the shell's `apply_tempo_patch`
makes of it, and MUSIC1's 130 is never touched.

## 4. The order list, as data

The order list is read as the bytes it is: **nothing is normalised, and 254 and 255 are kept.** A
loader that "fixed" them here would hide what the demo actually asks its player to do, and that
question is Task 3's, answered by the reference's own order counter (`music-clock.md` §1, §10). The
exact sequences are asserted byte for byte in `the_order_list_is_preserved_byte_for_byte`; what the
test prints about them:

```
MUSIC0.S3M: 80 orders; 255 at [16, 17, 64, 69, 78, 79]; 254 at [27]
MUSIC1.S3M: 100 orders; 255 at [99]; 254 at [3, 14, 19, 25, 27, 38, 41, 46, 49, 61, 67, 76, 87, 92, 97]
```

Six `255`s and one `254` in MUSIC0; fifteen `254`s and one `255`, last, in MUSIC1. Every other entry
is a pattern number inside the 77 both files carry.

**How the song starts, and who starts it.** The shell hands the list to the player through
`_zplaysong(order)`, which `MAIN/STMIK.H:43-44` declares with exactly that meaning ("this routine
starts playing the module at order [order]"). The call has two sites in `MAIN/U2.ASM`: the
**initial start** at `:785-786` — `push 0`, `call _zplaysong`, just after `mov es:_zpollmix,1` at
`:783`, which is the 0 → 1 transition `music-clock.md` §5 measures — and `restartmus`
(`:348-397`), which pushes the order its caller left in `bx` (`:393`) and calls at `:394`. The three
callers `music-clock.md` §10 names pass 18 (`:883`), 70 (`:892`) and 25 (`:903`). (The tree's other
two copies of the source, `MAIN/U2.BBK` and `MAIN/U2ASM.BBK`, carry the same two calls at `:394` and
`:783`, and `:393` and `:785`.) That is why the trace never sees a `255` played: the shell restarts
past them (§10, read off the order counter's own sequence), and what the player itself would do at
one is measured by nothing in the tree. Modelling `_zplaysong` is Task 3's.

## 5. The instruments and their samples

```
MUSIC0.S3M: 44 instruments, type census {0: 22, 1: 22}; 12 of them loop; 329186 sample bytes in all; 0 PCM headers with a zero paragraph pointer
MUSIC1.S3M: 54 instruments, type census {0: 13, 1: 41}; 9 of them loop; 520396 sample bytes in all; 0 PCM headers with a zero paragraph pointer
```

The type byte is `struct ds_smp`'s `type` (`STARTEMS.C:29`): **0 = empty and 1 = PCM, and no third
type occurs in either file.** Type 0 is a normal case, not a malformed file — `checkins` treats a
zero-length sample the same way (`:135-139`) — and a type-2 (adlib) instrument would have meant OPL
in this phase (`:175-176` puts types 2..7 on the adlib path). **Neither module has one**, which is
why Phase 3 models no OPL: that is a measurement over both files' 44 and 54 instruments, printed by
`the_instrument_census_is_the_files_own`. **Nor does either file have a PCM header with a zero
paragraph pointer** — the `0 PCM headers with a zero paragraph pointer` at the end of the two lines
above is that count — which is what makes the loader's own slicing condition agree with the
original's: the loader slices when `type == 1 && length != 0`, `STARTEMS.C:367` loads when
`type == 1 && memseg`, and the two can only disagree on a header that names no sample memory.

An empty instrument still has a header with values in it. The first of MUSIC0's 22:

```
MUSIC0's 22 empty instruments: instrument 19 carries length 0 memseg 0x0000 volume 64 flags 0x00 c2spd 8363
```

### The sample bytes

`STARTEMS.C:385-388` puts a sample at `memseg << 4 | memseg2 << 20` in the file and reads `length`
bytes there. Both ends of every slice are checked against the file:

```
MUSIC0.S3M: 22 samples sliced, the first at byte 61968 and the last ending at byte 391312 of 391312
MUSIC1.S3M: 41 samples sliced, the first at byte 80192 and the last ending at byte 600860 of 600860
(…)
MUSIC0.S3M: the patterns end at byte 61954, the first sample begins at 61968, and the 14 bytes between them are [80, 80, 80, 80, 80, 80, 80, 80, 80, 80, 80, 80, 80, 80]
MUSIC1.S3M: the patterns end at byte 80178, the first sample begins at 80192, and the 14 bytes between them are [80, 80, 80, 80, 80, 80, 80, 80, 80, 80, 80, 80, 80, 80]
```

The last sample ends on the file's own last byte in both modules, and the 14 bytes between the last
pattern block and the first sample are `0x80` — the PCM silence value — in both. That is the whole
container accounted for: header, tables, blocks, filler, samples, end of file.

**What the loader does *not* do to a sample** is `checkins`'s business (`STARTEMS.C:129-177`): the
clamps, and the 512 bytes it writes after every sample (either the loop's own bytes, for a looping
sample, or `128` filler, `:148-173`), are what the original did to its in-memory copy for the mixer.
Task 5 reproduces them against these same numbers; the loader slices the file's bytes and stops.

One more difference between the loader and `STARTEMS.C:367`, recorded because it is a difference
even though it is inert: the original's sample-loading loop guards on `(ins+a)->memseg` — a
paragraph pointer **as the header itself carries it**, read from the file at `:338-339`
(`lseek(h,filebase+((long)parains[a]<<4L),SEEK_SET); read(h,ins+a,0x50);`) — while its own
overwrite of that field with the segment it allocated happens *later*, inside the loop body
(`:389`, `(ins+a)->memseg=seg;`). So the guard is a test about the file, not about memory that has
already been moved. The loader's `sample_bytes` is about the file too — the bytes are there or
they are not — and the mixer is what will need the original's allocated-and-padded form.

### `SCRS` and `SCRM`

```
MUSIC0.S3M: SCRM at 44; SCRS at +0x4C of 22 sample headers, and four zero bytes at +0x4C of 22
MUSIC1.S3M: SCRM at 44; SCRS at +0x4C of 41 sample headers, and four zero bytes at +0x4C of 13
```

`SCRM` is `ds_fileheader._magic_signature` (`STARTEMS.C:62`), at the file's offset 44. `SCRS` is
`ds_smp._magic` (`:47`), the last four bytes of each 0x50-byte sample header, and it is present on
**every** PCM header and on none of the empty ones. It is a curious fact about the shipped bytes:
`checkins` *writes* that magic at load time (`:134`) rather than reading it, so the files carry it
because a copy of them went through a load once. The loader therefore requires `SCRM` (a file without
it at 44 is not one of these) but not `SCRS`, which is a claim about the shipped bytes and lives in
the test instead.

### A sample header field by field

```
MUSIC0 instrument 0: type 1 filename "HISTRING\0\0\0\0" memseg 0x0f21:0 length 30208 loop 5720..30014 volume 64 disk 0 pack 0 flags 0x01 c2spd 27700 name "HiStrings.HiQual(Marcato)\0\0\0"
MUSIC1 instrument 0: type 1 filename [00, 00, 00, 00, 00, 00, 00, 00, 00, 00, 00, 00] memseg 0x1394:0 length 40066 loop 0..0 volume 64 disk 0 pack 0 flags 0x00 c2spd 8630 name "By Purple Motion of\0\0\0\0\0\0\0\0\0"
```

Every field is read at its own offset — type `+0x00`, filename `+0x01..0x0C`, `memseg2 +0x0D`,
`memseg +0x0E..0x0F`, length `+0x10..0x13`, loop begin `+0x14`, loop end `+0x18`, volume `+0x1C`,
`disk +0x1D`, pack `+0x1E`, **flags `+0x1F`**, **`c2spd` `+0x20..0x23`**, `name +0x30..0x4B`, `SCRS
+0x4C`. The two fields in bold are the pair a reader four bytes early gets exciting but wrong numbers
for: MUSIC0's instrument 0 read that way reports `flags` 0 and `c2spd` **16,777,280** (0x0100_0040 —
its loop end's low byte `0x00`, then volume 64, `disk` 0, pack 0, flags 1), where the file says
`flags` 1 and `c2spd` 27,700. The test asserts all four values and the filename that comes with them,
and `crates/sr-audio/src/module.rs`'s synthetic-header test puts a distinguishable byte in every
field so the shift cannot pass there either.

## 6. The cells, and what the phase has to play

The effect histogram is this phase's scope evidence (the plan's Review Focus 3): an effect the demo
never plays has to be *measured absent*, not assumed. Counted from the decoded cells of every one of
the 77 patterns in each file:

```
MUSIC0.S3M: 157696 channel slots, 146161 of them carrying no command byte; the rest: A:63, B:3, C:8, D:7348, E:827, F:1051, G:802, H:212, J:137, K:105, O:755, Q:84, S:140
MUSIC0.S3M: a command byte of 0 on []; of 255 on 0; invalid commands: 0
MUSIC1.S3M: 157696 channel slots, 148363 of them carrying no command byte; the rest: 0(none):1, A:54, C:4, D:3340, E:217, F:180, G:2607, H:365, K:62, O:2417, Q:38, S:48
MUSIC1.S3M: a command byte of 0 on [(13, 19, 5)]; of 255 on 0; invalid commands: 0
```

157,696 slots is 77 × 64 rows × 32 channels, the whole space. Over it:

- The effect letters that occur are exactly **A, B, C, D, E, F, G, H, J, K, O, Q, S** — thirteen in
  MUSIC0, eleven in MUSIC1 (no B, no J). The letters that do *not* occur anywhere in either file are
  the other thirteen of the alphabet, and the counts are what the tests assert rather than the
  absence being inferred from a spec's list.
- **Zero invalid command bytes** in the ST3 numeric alphabet (spec §7.1: 1–26 = A–Z, 0x30–0x39 =
  digits, 255 = none). No cell in either file carries a digit command, and none carries 255 — the
  "none" spelling simply does not appear, because a cell with no effect carries no command field at
  all.
- **One cell carries a command byte of 0**, in MUSIC1, at pattern 13 row 19 channel 5 — the cell D5
  records as its exception, located by this task. It is a real cell, not an error:

```
MUSIC1 pattern 13 row 19 channel 5: present 0x80 (a command field follows), command 0, info 64
```

  The flag byte was 0x85 — bit 7 (command and info) with channel 5 in the low bits, which the test
  pins as `present == 0x80` on channel 5 — so the file really did store a two-byte command field
  whose command byte is 0, with info 0x40. Treating 0 as an error is what made D5's first verification
  pass report "76 of 77"; the loader keeps the cell and lets the player decide, and no pattern fails
  to decode.

The totals the test prints beside those lines:

```
MUSIC0's histogram: 11535 cells carry a command over 13 effect letters, all of them A..Z
MUSIC1's histogram: 9332 cells over 11 effect letters, plus one whose command byte is 0 (pattern 13 row 19 channel 5)
```

### Notes

```
MUSIC0.S3M: 5186 notes 0x10..0x64, 71 note-offs (0xFE), 0 no-notes (0xFF); the format's ceiling is 0x9B
MUSIC1.S3M: 11189 notes 0x26..0x80, 35 note-offs (0xFE), 610 no-notes (0xFF); the format's ceiling is 0x9B
```

Note bytes are `(octave << 4) | semitone`, valid to 0x9B. The highest note each module plays is
MUSIC0's 0x64 and MUSIC1's **0x80** — above 0x60, so a decoder capped at 96 rejects MUSIC1's high
material, which is the trap D5 records. The two special values are counted as they are found rather
than folded into the range: 0xFE (ST3's note-off) 71 times in MUSIC0 and 35 in MUSIC1, and 0xFF (no
note, the instrument alone) 610 times in MUSIC1 and never in MUSIC0. What the player does with each
is Task 3/4's and is not decided here.

### The instrument byte is 1-based, and a volume of zero is a cell

Two readings the loader deliberately keeps as the file has them:

- **`Cell::instrument` is the file's own byte.** MUSIC1's highest instrument byte is 54 while its
  `insNum` is 54, and its 35 note-off cells carry instrument byte 0: so 0 means "no instrument
  change" and 1..54 name the headers, a 0-based reading would index a 55th instrument the file does
  not have, and the loader leaves the conversion to the player.
- **A cell whose only content is `volume = 0` is a cell, not an absent one.** Measured:

```
MUSIC0.S3M: highest instrument byte 29 of 44 instruments; 74 cells name instrument 0; 3 cells carry nothing but a volume of 0 (3 of which have all five fields zero)
MUSIC1.S3M: highest instrument byte 54 of 54 instruments; 35 cells name instrument 0; 189 cells carry nothing but a volume of 0 (189 of which have all five fields zero)
```

  192 such cells, all of whose five fields are zero. By its five fields alone such a cell is
  indistinguishable from a channel slot the file's row never named — and a player that mistook it for
  one would leave the channel's volume where it was instead of taking it to zero. So `Cell` carries a
  sixth field, `present`: the flag byte's three "what follows" bits, which is the file's own answer to
  "was this field written". This is one deliberate addition to the plan's field list, and it is the
  one the measurements above force; `is_empty()` is a presence test, so `Cell::EMPTY` means "the row
  never named this channel" and nothing weaker.

The other end of the same measurement: MUSIC0's 14,980 named cells in 157,696 slots, MUSIC1's 17,746
in 157,696 — the rest are unnamed, and `the_cells_a_row_names_are_the_only_ones_that_are_not_empty`
checks that the two notions agree cell by cell.

## 7. What this half does not decide

Recorded so that "not yet decided" is not read as "assumed":

- **Order list 254 and 255.** Kept as data (§4), and the shell's `_zplaysong` calls are named there
  too. What the player does at them is **§8's**: `254` is stepped over (measured in the traces and
  read out of the object), and `255` ends the order list or loops the song, which is the object's
  reading because no trace reaches one (`music-clock.md` §10). No reading of stock ST3 semantics
  is used anywhere.
- **Note 0xFE and 0xFF, instrument 0 and command 0.** Counted and located (§6), not interpreted.
- **The samples' in-memory form** — the clamps and the 512-byte pad of `checkins`
  (`STARTEMS.C:129-177`) — is Task 5's; the loader slices the file.
- **Whether the modules' `S` commands are all `SDx`** (spec §7.2's scope claim, and D8's `_np_zinfo`
  question) is a reading of `Cell::info` on those cells and belongs with the effects in Task 4.

## 8. The player — Task 3

Task 3 makes the port's player reproduce the traces. **The original's player has no assembly
source in the tree**: `MAIN/STMIK.300` is an OMF object, and everything below is read out of its
own segment image — the 29,607 bytes the symbol map records for `text_stmik300_text`, rebuilt
from the object's 105 `LEDATA` and 118 `LIDATA` records (`crates/sr-audio/tests/player.rs`'s
`the_objects_own_bytes_say_what_the_player_says`, which rebuilds it and asserts the load-bearing
instructions at their own offsets (24 of them when Task 3 wrote it, 131 after Task 4 added the
effects', and **151 at the tip** — Task 5's mixer claims plus the fix wave's four gate/clamp
claims — the count the test itself prints)). Offsets below are that object's segment offsets, and the
listings are that image disassembled (`objdump -b binary -m i8086 -D`) — the lines are pasted as
it prints them, `(…)` marks an elision, and the `;` comments are this note's, not the tool's. What
the object cannot settle — which *frame* a row change lands on — is measured against the traces,
and where the two disagree the trace wins (D12).

### The tick, the row, and the effect that moves them

```
  3D92: b8 32 00              mov    $0x32,%ax
  3D95: f7 e1                 mul    %cx        ; × the tempo
  3D97: 8b d8                 mov    %ax,%bx
  3D99: 2e a1 d5 08           mov    %cs:0x8d5,%ax   ; _zmixspeed
  3D9D: b9 7d 00              mov    $0x7d,%cx       ; 125
  3DA0: f7 e1                 mul    %cx
  3DA2: f7 f3                 div    %bx
  3DA4: 2e a3 e1 08           mov    %ax,%cs:0x8e1   ; [0x8e1] = mixspeed × 125/(50 × tempo)
```

A tick is that many samples — `40000 × 125/(50 × 120)` = 833 at tempo 120, 769 at 130 — and it is
computed by the tempo setter and nowhere else. A row ends when a counter of ticks reaches the
**published** speed:

```
  4397: ff 06 00 20           incw   0x2000
  439B: 2e a1 02 20           mov    %cs:0x2002,%ax   ; _np_speed
  (…)
  43AC: 39 06 00 20           cmp    %ax,0x2000
  43B0: 7d 01                 jge    0x43b3            ; → the row advance
```

so an `A` command lands on the tick it is processed on, *mid-row*, which is the whole of §3's
post-speed-change transient. The row's cells are read on the row's first tick
(`+0x4371: cmpw $0x0,[0x2000]`), which is where `A` writes `_np_speed` (`+0x3dd8`) — the trace's
`speed` word therefore leads its `row` column, and the plan's third implementation detail is this
instruction and not a convention.

A poll's samples are fractional (`40000/70.086` = 570.727…), and `player.rs` carries the fraction
in a `1/70_086`-sample accumulator rather than rounding per poll. The original carries the
remainder the same way, in the mixing countdown that a tick tops up
(`+0x50f0: sub %ax,0xbe0` … `+0x5157: add %ax,0xbe0`).

### The published position is a ten-poll-old copy, and a seek does not refresh it

`_np_row`, `_np_pat`, `_np_ord` and `_np_zplus` are not written where the player is: they are
**popped from a ring**.

```
  52C9: 2e 8b 36 4e 21        mov    %cs:0x214e,%si        ; the write pointer
  52CE: a1 05 09              mov    0x905,%ax
  52D1: 2e 89 84 50 21        mov    %ax,%cs:0x2150(%si)   ; followed by 0x907, 0x909, 0x90d
  (…)
  52FA: 2e 8b 36 4c 21        mov    %cs:0x214c,%si        ; the read pointer
  52FF: 2e 8b 84 50 21        mov    %cs:0x2150(%si),%ax
  5304: a3 fd 08              mov    %ax,0x8fd             ; → _np_row
  (…)
  5326: 2e 89 36 4c 21        mov    %si,%cs:0x214c
```

and the ring is primed **ten entries deep** by the module initialiser:

```
  2AC1: 2e c7 06 4e 21 50 00  movw   $0x50,%cs:0x214e   ; 10 × 8 bytes ahead
  2AC8: 2e c7 06 4c 21 00 00  movw   $0x0,%cs:0x214c
```

Ten entries at one `zpollme` a copper frame (`MAIN/COPPER.ASM:466`) is a ten-frame delay, and
that is the phase the traces show: the port publishes `second5`'s first row change at frame 426
where the trace has it at 427 — one frame, which is the drift the residual below measures — and a
player that published the engine's own row instead would publish it at 416, ten frames early.
`_np_speed`, `_np_tempo` and `_np_loop` are written straight through (`+0x3dd8`, `+0x3d7b`,
`+0x52c5`) and are not delayed, which is the same fact seen from the other side.

What the ring is *fed* from — `[0x905]` (row), `[0x907]` (pattern), `[0x909]` (order), `[0x90d]`
(zplus) — is written **only** on the tick routine's row-advance path:

```
  43E6: a1 40 09              mov    0x940,%ax
  43E9: 3d ff ff              cmp    $0xffff,%ax
  43EC: 74 01                 je     0x43ef
  43EE: 48                    dec    %ax
  43EF: a3 09 09              mov    %ax,0x909
  43F2: a1 48 09              mov    0x948,%ax
  43F5: a3 05 09              mov    %ax,0x905
  43F8: a1 4c 09              mov    0x94c,%ax
  43FB: a3 07 09              mov    %ax,0x907
```

`_zplaysong` calls the order-advance routine *directly* instead (`+0x2597: call 0x442d`), and that
routine returns at `+0x44ee` — so a seek never touches those four words, and the ring keeps
publishing the previous rows until the first row advance works its way out ten polls later. That
is exactly `second5`: frames 408…426 read `ord 0 row 0 pat 0` and the first published row is 427,
which is the same ten polls.

### `zplus`, `254` and `255`

The order-advance routine is `+0x442d`, and §6's rule is this code, not an inference from the
trace's columns — **the §6 that this task amended**: its original "unless the entry after the one
entered is itself a `254`, in which case 1" had no cause in the object, whose two `orw`s cannot
produce a 1 there. The listing below is the two bits themselves.

```
  442D: 2e c7 06 0d 09 00 00  movw   $0x0,%cs:0x90d
  4435: 2e a1 40 09           mov    %cs:0x940,%ax       ; the order to try next
  4439: 40                    inc    %ax
  443A: 2e a3 40 09           mov    %ax,%cs:0x940
  443E: 48                    dec    %ax
  (…)
  444B: 8a 27                 mov    (%bx),%ah           ; orders[o]
  444D: 8a 47 01              mov    0x1(%bx),%al        ; orders[o + 1]
  4450: 8a dc                 mov    %ah,%bl
  4452: 3c fe                 cmp    $0xfe,%al
  4454: 75 06                 jne    0x445c
  4456: 2e 83 0e 0d 09 01     orw    $0x1,%cs:0x90d      ; the next entry is a 254 → bit 0
  445C: 80 fb fe              cmp    $0xfe,%bl
  445F: 75 08                 jne    0x4469
  4461: 2e 83 0e 0d 09 02     orw    $0x2,%cs:0x90d      ; this one is → bit 1
  4467: eb cc                 jmp    0x4435              ; … and it is stepped over
  4469: 80 fb ff              cmp    $0xff,%bl
  (…)
  4481: 2e ff 06 0b 09        incw   %cs:0x90b           ; _np_loop's source: the loop path
```

`254` is stepped over — measured in the traces (§10) *and* read here — and a `255` leaves the scan:
if the player has not yet played a row (`[0x939] == 2`, which `_zplaysong` sets) the song ends at
`+0x4476`…`0x4480`; otherwise `_np_loop` increments (`+0x4481`) and the scan restarts at the loop
order `_zplaysong` recorded (0, for every plain order the shell hands it — `+0x256b` clears it).
If *that* entry is itself a `255`, `+0x4499`'s `je` jumps to `+0x447f` — which is **past** the
stop at `+0x4476` — so it returns with the loop order in `ax` and the pattern, the order and the
mixing flag untouched: the scan gives up and the player keeps playing what it was playing.

**Three things about the order scan are worth stating because no trace can reach them** — neither
module has a `255` at its first order, and the third is not a `255` path at all — and the port
follows the bytes in all three rather than choosing:

- A `_zplaysong` seek onto a `255` is *not* a stop. The scan's stop path clears `[0x938]`
  (`+0x4476`) and the seek sets it back to 1 four instructions later (`+0x259a`), so the player is
  left ticking; the row counter (`+0x5702`), the pattern (`+0x5708`) and the published row
  (`+0x570b`) were each zeroed by the seek's own routine and nothing entered an order, so the
  engine sits at
  row 0 of pattern 0 — while the ring, fed `[0x940] - 1`, hands the parts the index of the `255`
  itself. `crates/sr-audio/tests/player.rs`'s
  `a_255_ends_the_order_list_and_loops_when_the_song_was_already_running` asserts exactly that
  state.
- Reaching the give-up branch with the loop order already a `255` leaves `[0x940]` where the scan
  stopped, so the fed order is the last index looked at rather than an order that was entered —
  which is what `entry_now` computes (`next_order - 1`, `+0x43e6`..`+0x43ef`) and not the order the
  engine reports.
- The same routine zeroes **the tick-in-row counter** (`+0x56ec`, `movw $0x0,0x2000`), and
  **Task 7 decided it: `play_song` mirrors it.** Until then `player.rs`'s `play_song` left the
  counter alone, which was unobservable because nothing in the tree seeks mid-song — every test
  and every filed trace seeks on a fresh player, whose counter is already 0. It stopped being
  unobservable in this task, because the shell's restart *is* a mid-song seek: `restartmus`
  (`MAIN/U2.ASM:348-397`) reaches `_zplaysong` (`:394`) with the player anywhere inside a row
  (it is called at `:775`, `:818`, `:863`, `:885`, `:894` and `:905`, and the last four are entry
  points an already-playing player is restarted into). A seek that left a *stale* counter would
  begin the new order's row 0 on a later tick — `+0x4371`'s `cmpw $0x0,0x2000` false — so the
  row's cells would not be read and its notes and commands would never start, which is a visible
  divergence rather than a subtle one. The object's byte says what to do (`+0x56ec` is the
  routine's own first instruction, asserted byte for byte in
  `crates/sr-audio/tests/player.rs`'s claims), so the port does it, and
  `a_seek_mid_song_clears_the_tick_counter_so_the_new_rows_cells_are_read` is the test that fails
  without it: it seeks with the counter part-way through a row and watches the new row 0's `A`
  command land. One line, and a state the phase can now reach.

**`music-clock.md` §10 says the trace cannot show what the player does at a `255`. This is that
question's answer, and it is a reading of the object rather than a measurement**: the note records
it as the port's behaviour, with the trace's silence stated beside it. The same goes for `B`
(`+0x3de2`: the advance flag goes up and the order to try next is overwritten, so the jump lands at
the row's *end*; `BFF` at `+0x3df1` ends the song) — no filed window reaches a `B`, and MUSIC0's
three are at orders 63, 66 and 77, past every trace. What the traces *do* exercise is `A` and `C`:
MUSIC0's pattern 70 row 0 sets speed 6, pattern 71 row 0 sets 1 and row 36 sets 6, and pattern 71
row 61's `C00` is the trace's ord 26 → ord 28. `C`'s parameter is decimal and is stored unclamped
(`+0x3dfe`: `mov %al,%ah; shr $0x4,%ah; and $0xf,%al; aad $0xa`).

### Two readings that are in the object, and one this half misread

- **`A` and `T` are two handlers, not one told apart by the parameter** — and this section's first
  version had it the other way round. What the object has is two entries in the first-tick table
  (§9.1): entry 1 is `A`'s body, eleven bytes with **no upper bound** on the parameter, and entry
  20 is `T`'s, whose `cmp $0x20,%ax; jbe 0x3dce` sends 32 and under to the *driver* call and never
  to the speed:

  ```
    3DD2: 3c 00                 cmp    $0x0,%al         ; A, the first-tick table's entry 1
    3DD4: 74 f8                 je     0x3dce
    3DD6: 32 e4                 xor    %ah,%ah
    3DD8: 2e a3 02 20           mov    %ax,%cs:0x2002   ; _np_speed — no bound on %al
    (…)
    3D73: 32 e4                 xor    %ah,%ah         ; T, entry 20
    3D75: 3d 20 00              cmp    $0x20,%ax
    3D78: 76 54                 jbe    0x3dce           ; 32 and under: the driver call
    3D7A: 50                    push   %ax
    3D7B: 2e a2 0f 09           mov    %al,%cs:0x90f    ; _np_tempo
  ```

  The object's own **header watch** is the second witness, and it is the one that settles it
  without an argument about parameters: it calls `+0x3dd2` for the module's initial *speed* byte
  (offset 49, read at `+0x53ca` into `%al`, called at `+0x53d8`) and `+0x3d73` for its initial
  *tempo* byte (offset 50, read at `+0x53b9`, called at `+0x53c7`). All six offsets — the two
  handlers and each one's read and call — are claims in `tests/player.rs`, which is what made
  that table 131 rather than 117. Two handlers, two call sites, no parameter
  test in either. This was not a reading that contributed nothing: MUSIC1 carries one `A7F` cell
  and Task 3's player played it as a *tempo*, which §9.13 and §8's end-to-end numbers follow
  through. Neither module carries a `T`
  (`crates/sr-audio/tests/player.rs`'s `the_timeline_effects_the_modules_carry` counts MUSIC0's
  A×63, B×3, C×8 and MUSIC1's A×54, C×4, and no `T` in either).
- The row timer has a fractional adjuster — the three lines between the `_np_speed` load and the
  comparison above:

  ```
    439F: 2e 8b 16 cd 08        mov    %cs:0x8cd,%dx
    43A4: 2e 01 16 cf 08        add    %dx,%cs:0x8cf
    43A9: 73 01                 jae    0x43ac
    43AB: 48                    dec    %ax             ; one tick off the row
  ```

  The addend lives in a word the object's image carries as `0x0101` — the filler these objects
  use for slots the program clears before it runs: the segment's other BSS-like words read the
  same way, `_np_zframe` among them, and every trace that holds the music's start reads
  `_np_zframe` as 0 before the music's first tick, so those slots do not survive into the
  running program. And the traces
  agree the adjuster contributes nothing here: the published row changes of `second5`'s speed-6
  run are **exactly** six ticks apart (`cargo xtask music analyse --trace reference/music/second5`
  prints 61 gaps in 534 frames where the rule says 534.4, residual −0.4).

### 254 and the pattern break, as the trace has them

Read from `second5`'s own `frame,ord,row` columns (not a command's output; the values are the
CSV's, checked against the trace file). Two transitions are the evidence for the `C`/`254`
reading:

| frames | transition | what it is |
|---|---|---|
| 1251 → 1260 | ord 26 row 61 → ord 28 row 0 | the `C00`, over the 254 at order 27 |
| 970 → 979 | ord 25 row 63 → ord 26 row 0 | pattern 71's `A01` at speed 1 |

Both are in `crates/sr-audio/tests/trace.rs`, which walks `reference/music/`, drives a `Player`
from the boot's own music start (frame 408, derived from each trace's own columns and checked
against §2's anchor), and compares **every frame the trace holds** — never skipping a frame the
trace shows no change on, and never renumbering around a gap: the coverage each comparison runs
over is recomputed from the trace file and checked against the manifest's window and gap list
field by field.

### What the comparison asserts, and the residual it measures

The reference's tick clock and the port's are both uniform, but the reference's is sampled by an
emulated machine whose frame counter carries D17's host-timing term — and **the two sessions of
one recipe disagree with each other**:

`cargo test -p sr-audio --test trace -- --nocapture the_two_sessions`:

```
second5 against second5b: 131 row change(s) each, 8 of them on a different frame:
  ord 25 row 18: frame 575 against 576
  ord 25 row 24: frame 629 against 628
  ord 25 row 53: frame 883 against 882
  ord 25 row 61: frame 953 against 952
  ord 26 row 6: frame 988 against 987
  ord 26 row 8: frame 991 against 990
  ord 26 row 19: frame 1007 against 1006
  ord 26 row 56: frame 1207 against 1206
```

so no player can be frame-identical to both. **The plan's four anomalous intervals are this
jitter**, and the diagnosis is the one the sessions actually support: over `second5`'s speed-6 run
the row intervals spread over **8, 9 and 10 frames where the rule's 8.76 says 8 or 9** — three
distinct gap sizes, one interval outside the two the row length brackets — and *which* interval
comes out fatter is not a function of the position, because the two sessions disagree about eight
of their 131 row changes, so the fat interval is not even in the same place from one capture to
the next (`cargo xtask music analyse --trace reference/music/second5` prints the histogram;
`cargo test -p sr-audio --test trace -- --nocapture the_two_sessions` prints which rows move).
**A per-row mapping of the anomalies is not producible by any command**: rows 28, 49 and 57 of the
`second5` run sit on the *same* frame in both sessions, so nothing singles them out — and that is
the finding, not an omission: what the oracle's jitter moves is the interval, not a set of rows.
A test that listed the anomalies would be fitting; one that demanded exactness would be demanding
the impossible. So the comparison asserts what a port can be held to —

1. **content, word by word**: for each of the seven words the trace records — `order`, `pattern`,
   `row`, `speed`, `tempo`, `loop` and `zplus` — the sequence of values the port publishes over
   the trace's frames is the trace's. No row skipped or invented, no order advance early or late,
   no effect lost, no pattern published for the wrong order, no `zplus` bit left unset. What each
   key can catch: `order` and `row` the order advance and the `254` step-over, `pattern` a wrong
   placement of an order, `speed`/`tempo` a lost, doubled or mis-rowed `A`, `zplus` the marker
   arithmetic. `loop` is **0 in every filed trace**, so a key that never moves can catch only a
   wrong *value* — and the comparison's own shape says which wrong values: the two value sequences
   may differ in length by one and are compared over their common prefix, so a port whose counter
   took a wrong value anywhere in the window would fail **at the first of them** (a loop count of
   its own from the window's start fails; a counter that stayed 0 and then moved once, later, is
   inside the one-entry slack and is invisible), while one that froze the counter at zero matches
   what the traces do.
2. **placement**: every published value is one the trace publishes at that frame or within
   7 frames of it;

— and *measures* the rest. `cargo test -p sr-audio --test trace -- --nocapture` prints (Phase
4's, Phase 5's and Phase 6's traces are included; the nine window runs first print their
session's phase offset):

```
14 filed trace(s)
  hiddenu (MUSIC0): 5685 of 6084 frames agree (93.442 %); worst displacement 3 frame(s)
  hiddenu2: the session's phase is -16 frame(s) from the boot's
  hiddenu2 (MUSIC0): 1620 of 1692 frames agree (95.745 %); worst displacement 1 frame(s)
  second1 (MUSIC0): 3738 of 5538 frames agree (67.497 %); worst displacement 7 frame(s)
  second1b: the session's phase is +1 frame(s) from the boot's
  second1b (MUSIC0): 344 of 352 frames agree (97.727 %); worst displacement 1 frame(s)
  second1c: the session's phase is +1 frame(s) from the boot's
  second1c (MUSIC0): 1609 of 1692 frames agree (95.095 %); worst displacement 1 frame(s)
  second2: the session's phase is -5 frame(s) from the boot's
  second2 (MUSIC1): 3207 of 4590 frames agree (69.869 %); worst displacement 4 frame(s)
  second3 (MUSIC1): 2141 of 2484 frames agree (86.192 %); worst displacement 2 frame(s)
  second3a: the session's phase is +2 frame(s) from the boot's
  second3a (MUSIC1): 6089 of 7291 frames agree (83.514 %); worst displacement 2 frame(s)
  second3b: the session's phase is +1 frame(s) from the boot's
  second3b (MUSIC1): 3179 of 4839 frames agree (65.695 %); worst displacement 6 frame(s)
  second3d: the session's phase is +4 frame(s) from the boot's
  second3d (MUSIC1): 2716 of 2792 frames agree (97.278 %); worst displacement 2 frame(s)
  second3e: the session's phase is +1 frame(s) from the boot's
  second3e (MUSIC0): 3857 of 3992 frames agree (96.618 %); worst displacement 1 frame(s)
  second4: the session's phase is +1 frame(s) from the boot's
  second4 (MUSIC0): 3822 of 3892 frames agree (98.201 %); worst displacement 1 frame(s)
  second5 (MUSIC0): 769 of 883 frames agree (87.089 %); worst displacement 2 frame(s)
  second5b (MUSIC0): 775 of 883 frames agree (87.769 %); worst displacement 2 frame(s)
```

On the Phase 3 windows the frames that disagree are the ±1-frame placement of a row boundary —
one per row change — because the port's clock is exact and the reference's is not; the port
sits *between* the two sessions (a frame early in `second5`, a frame late in `hiddenu`) which is
the best a fixed clock can do. The larger displacements are `second1`'s 7 frames and
`second3b`'s 6 — the windows whose reference rate is 0.1 % from the port's exact one, not the
longest (the longest, `second3a`'s 7,291 compared frames, reach 2) — still under one row at
their speeds; the content is asserted word by word, not by frames.
`docs/fidelity/APPROXIMATIONS.md` A12 records the measurement and its cause.

### The end-to-end play, shown

`cargo test -p sr-audio --test player -- --nocapture each_module_plays`:

```
  MUSIC0: wrapped after 8732 polls (124.6 seconds of logic ticks) at order 0 pattern 0 row 0, loop counter 1
  MUSIC0: 8732 poll(s), 5983 tick(s) over 998 row(s)
  MUSIC0: 80 order(s), 77 pattern(s), ended at order 0 (1 loops)
  MUSIC1: wrapped after 28618 polls (408.3 seconds of logic ticks) at order 0 pattern 73 row 0, loop counter 1
  MUSIC1: 28618 poll(s), 21240 tick(s) over 5283 row(s)
  MUSIC1: 100 order(s), 77 pattern(s), ended at order 0 (1 loops)
```

(the second line of each pair is the same run counted a second way — the engine's row visits
against its own tick counter, every row asserted to have had between 1 and 255 ticks.)

**MUSIC1's four numbers changed in Task 4, and two separate changes account for it.** Each is
measured by reverting it alone and re-running this command at the committed state:

- **The `A` handler's correction** (§9.13) is the larger part: **+2606 polls and +1937 ticks**,
  and not one more row. MUSIC1 carries one `A` cell whose parameter is `0x7f` (pattern 72, row
  48, order 98 — its `A` parameters are `{3: 30, 6: 23, 127: 1}`), and Task 3's player read a
  parameter above 32 as a *tempo*. The object's `A` handler has no upper bound (`+0x3dd2`), so
  that row plays at speed 127 — and the `A`'s own row *with* it, because the speed is written
  on the row's first tick and the row timer reads the published word: the sixteen rows from
  the cell to the end of order 98 each take 127 ticks where the rows before them take 6, which
  `crates/sr-audio/tests/effects.rs`'s
  `music1s_a7f_row_takes_127_ticks_and_takes_its_own_row_with_it` prints as 2032 ticks against
  the 96 the same sixteen would take at 6 — **+1936** of the **+1937 ticks** measured; the
  last one is not a row at all: it is the ten-poll tail the run is cut at, where the old reading's
  tempo of 127 makes the ticks slower than the module's own 130, so that tail holds one tick fewer
  (8 against 7, measured by the same revert as the totals above).
- **The "volume 0" optimisation arriving** is the rest: **+243 polls, +180 ticks and +60 rows** —
  and that is a *fix of a Task 3 timeline bug*, not a new behaviour. MUSIC1's order 0 (pattern
  73) row 3 channel 2 is a `C00`, a pattern break to the next order, and channel 2 has been
  silent since its row 0 note-off: the first-tick pass counts three such rows and **frees the
  channel** (`+0x3c77`…`+0x3cad`, on because both header flags words are 8) before the dispatch
  table is reached, so the break never runs and the pattern plays its 64 rows out. Task 3 had no
  such path and dispatched the freed channel's command, jumping to order 1 at row 3; the sixty
  rows are that difference.
  `crates/sr-audio/tests/effects.rs`'s
  `the_volume_zero_optimisation_stops_a_pattern_break_from_running` pins the mechanism — the cell,
  the free and the order still being 0 at row 63.

The arithmetic closes on the numbers this command prints: 25769 + 243 = **26012** polls with the
`A` correction still absent, 26012 + 2606 = **28618** with it; 19123 + 180 = **19303** and
19303 + 1937 = **21240** ticks; 5223 + 60 = **5283** rows. **The intermediate figures —
26012/19303 and 25769/19123/5223 — are not printed by any command at the committed state**:
they were measured by reverting one change at a time in the tree and re-running the same command,
which is how this list was made and which no committed test asserts. Only its ends are printed
above — 28618 polls, 21240 ticks and 5283 rows are the pasted block — so the split into
+243/+180/+60 and +2606/+1937/0 is measurement rather than arithmetic a command can repeat. **The trace comparison cannot see
either the bug or its fix**: `tests/trace.rs`'s `run()` starts MUSIC1 at the first order the trace
*publishes*, which is 42, so order 0 is never compared — and the trace numbers quoted above are
the same before and after.

Each module is played from order 0 — the order the demo's own boot starts it at
(`MAIN/U2.ASM:786` pushes 0) — through its order list to the first `255`, where it loops back
(the shell's own restarts are what the demo hears; the module's own list ends where the `255`s
are). `playing_past_the_end_neither_panics_nor_runs_off_the_orders` then plays each module a
further 100,000 polls and asserts that every published order, pattern, row, speed and tempo stays
inside the module's own tables — a player that loops for ever is fine, one that reads past its
orders is not.

## 9. The channel effects — Task 4

The thirteen effect letters the two modules carry are `A`, `B`, `C`, `D`, `E`, `F`, `G`, `H`, `J`,
`K`, `O`, `Q` and `S` (§6's histogram). Tasks 3 and 4 together implement them: `A`, `B` and `C`
move the timeline and are Task 3's, `D`, `E`, `F`, `G`, `H`, `J`, `K`, `O`, `Q` and `S`'s `SDx`
move a channel and are this section's. The code is `crates/sr-audio/src/effects.rs` (the channel
state and every handler) and the second half of `crates/sr-audio/src/player.rs` (the row parse,
the two tick passes and the dispatch), and the evidence is
`crates/sr-audio/tests/effects.rs` — 22 tests, one per effect plus the scope, the period
arithmetic, the `A` correction's own row (§8) and the volume-0 path's effect on the timeline
(§8) — whose printed numbers this section quotes:

```
cargo test -p sr-audio --test effects -- --nocapture
cargo test -p sr-audio --test player -- --nocapture the_objects_own_bytes
```

Every offset below is `MAIN/STMIK.300`'s segment offset, and every one of them is **asserted
against the object's own bytes** by `tests/player.rs`'s claims table, which Task 4 grew from 24
claims to **131**: the handlers, the two dispatch tables, the `S` subcommand tables, the waveform
entries and every table the arithmetic is measured in. Where the object and the ST3 semantics one would expect disagree, the
object wins and the disagreement is named here.

### 9.1 Three dispatch paths, not one

A command can be reached from three places, and which one decides *when* it runs:

- the row's **first tick** (`+0x3c44`, table at `+0x3bd8`), after the row's cells are read;
- every **later tick** of the row (`+0x3d38`, table at `+0x3c0e`);
- the **row parse itself** (`+0x46c2`), for the two pieces of work no table dispatches: `O`'s
  offset, taken as its cell is installed (`+0x48d6`), and the note start every cell's note gets
  (`+0x4810`).

The two tables are not the same table. Their entries 1, 2 and 3 differ (the `A` handler
`+0x3dd2`, `B` and `C` are first-tick only; the later ticks' entries are the bare `ret` at
`+0x3d72`), `G` (`+0x3f94`) and `H` (`+0x4005`) have **no first-tick handler at all**, and `K`
(`+0x3f00`) and `L` (`+0x3f05`) have none either — they set `bp` and fall into `D`'s body
(`+0x3f08`), whose tail continues into the vibrato for `K` (`+0x4000`) and the tone porta for `L`
(`+0x3f8f`). The first-tick table's entry 20 (`T`) is the A/T handler at `+0x3d73`, which is the
one place the parameter is looked at.

What the first-tick pass does per channel *before* it dispatches anything is where the odd
behaviour lives: a channel with **no command** has its period restored from
`aorgspd` and its `atrigcnt` cleared (`+0x3d0c`), a `D` row does the same (`+0x3d24`), every
non-zero parameter is remembered in `alastnfo` (`+0x3cb9`), and any command that is not `D`, `H`,
`K`, `I` or `R` marks the vibrato position **stale** (`+0x3cea`, `orb $0x80,0x26(%si)`). A channel
whose volume is 0, whose cell carries no note, instrument or volume column, and which has had
three such rows in a row is **freed** (`+0x3c97`…`0x3ca9`, on because the module headers'
`flags` word is 8 and its bit 3 is `_np_masterflags`'s "vol 0 optimizations" —
`MAIN/STMIK.H:107`).

The row parse is a separate pass with its own reset: `+0x46a4` writes `note = 0xff`,
`volume = 0xff` and instrument, command and info 0 into all 48 of the object's channel
structures, so a channel the row does not name has no command at all — which is what stops the
previous row's `D` from running again. The named channels' cells are then installed
(`+0x4677`…`0x469c`), clamped (`+0x4762`…`0x4780`: an instrument above `0x65` becomes 0 and **a
volume column above `0x3f` becomes 63** — the file's own 64 is not a volume the player ever
holds), and — for the first sixteen channels, the mixer's; above them is the adlib path this
build has no card for — given their note (`+0x4810`).

The channel index is the S3M channel number: the pointer table at `+0x1fa0` maps the 32 numbers
to their structures and is the identity (`0x0da0`, `0x0e00`, `0x0e60`, …), so cell channel 5 *is*
`_zchn[5]`. The panning the mixer will apply is the header's channel table, which is a different
thing (§3).

### 9.2 A note, and the four readings inside it

`+0x4810` is the routine every note a channel plays comes through, and four of its readings are
worth naming because none is the obvious one:

- **An instrument of its own clears the sample offset** (`+0x481d`, `movw $0x0,0x6(%si)`) *before*
  the note it came with can set it — so an `O` on a cell that names an instrument starts that note
  from the offset, and an `O` without one leaves the last offset for the next note.
- **A note under a `G` does not take the period.** `+0x490e` tests `aorgspd` and `+0x4944` tests
  the command:
  for a `G` on a channel that is already playing, `aspd` is *not* written (the live period stays
  where the glide left it) while `aorgspd` **and** `asldspd` both take the new note's period
  (`+0x4954`, `+0x4957`). The period the glide walks from is `aspd`; the one it walks towards, and
  writes back to on the way, is `aorgspd` — the two only meet when the glide arrives.
- **The volume column replaces both the volume and `aorgvol`** (`+0x495a`), including a volume of
  0 — which is why `Cell::present` exists (§6).
- **`0xfe` is the note off**: the position parks at 1, `m_poslow` at 0, the volume at 0, and
  `aorgspd` and `asldspd` at **`0xffff`** (`+0x4906`…`0x4957`) — the value the mixer reads as "no
  sample". The live period is left where it was, so a `D` row after a note off restores `0xffff`.

And one that is not a reading but an absence: a note on a channel that was not already playing
does **not** set `aequ` (`+0x491a` is on the far side of `+0x490e`'s test). Only a note that
restarts a playing sample sets the `0xff` whose tail becomes `avol + 0x10` (`+0x496a`), so a
first note leaves `aequ` at whatever the channel had — zero, on a fresh module.

The note itself is `+0x593a`: `0xfe` is 0, otherwise the 12-TET table at `+0xa24` indexed by the
note's semitone and shifted right by one byte per octave from `+0x97c`. That table is four
octaves up from the S3M table the format documents — its C is 27392, so octave 4's C is the
familiar 1712 — and the shift is why one table covers every octave. **The shift table is eight
bytes long and the object indexes it with `note >> 4` without a range check**, so octave 8 reads
the byte after it (0, the same shift as octave 0) and octaves 9…15 read further into the next
table. That is not a formality: MUSIC1 plays note `0x80` — the highest note either file writes,
§6 — and its shift is that ninth byte, so the file's highest-written note plays octave 0's
period. The port keeps the sixteen bytes the lookup reads, in the object's own order, and
`every_octave_of_the_note_table_is_the_objects_own` asserts the whole table one line an octave.

The period is then scaled by the instrument: `+0x5813` is `period × 8363/c2spd`, one integer
division with the two guards an 8086 `div` needs (`+0x581f`, `+0x5825`). A `c2spd` of 8363 — the
file's own reference, MUSIC0's instrument 4 — leaves the period alone, so the note plays at the
rate its header declares: over every PCM instrument of both files the `c2spd` field runs from
**8163 to 27700** (63 of them — the range is asserted and printed by
`a_notes_period_is_scaled_by_its_instruments_c2spd`), which is why the same note is a period on
one channel and three and a half times it on another.

### 9.3 D — the volume slide, and the fine markers are the other way round

`+0x3f08`, in both tables, and `K` and `L` reach it through their two-instruction stubs. The low
nibble slides **down** and the high nibble **up**, and the object tests them low first
(`+0x3f22`). What the fine forms do is the surprise: `D?F` — a **low** nibble of `0xf` — adds the
**high** nibble once, on the first tick (`+0x3f73` → `+0x3f51`), and `DF?` — a **high** nibble of
`0xf` — subtracts the **low** nibble once (`+0x3f81` → `+0x3f31`). The two degenerate forms
(`D0F`, `DF0`) have a zero in the other nibble and fall back through the same paths to the
ordinary 15-a-tick slide.

A zero parameter takes the channel's `alastnfo` and **writes it back into the cell's info byte**
(`+0x3f11`, `+0x3f14`) — and `alastnfo` is not D's private byte: the first-tick pass writes it for
*any* command with a non-zero parameter (`+0x3cb9`), so MUSIC0's 5,795 `D00` cells slide by the
channel's last non-zero parameter whatever command carried it (and not at all when there is
none).

The volume clamps to **0…63** (`+0x3f3c`, `+0x3f5e`): a slide can never reach 64, and neither can
a cell's volume column, which the row parse has already clamped to `0x3f` by the time the slide
sees it.

### 9.4 E and F — the portamentos

`+0x3e4f` (`E`, the period grows) and `+0x3e10` (`F`, it shrinks) are one body with the sign of
one instruction flipped, in both tables, and with no note playing (`aorgspd == 0`) they do
nothing at all (`+0x3e53`). The steps:

| tick | info | step |
|---|---|---|
| first | `≤ 0xe0` | nothing |
| first | `0xe1`…`0xf0` | the low nibble, once |
| first | `> 0xf0` | the low nibble **× 4**, once |
| later | `≥ 0xe0` | nothing |
| later | `< 0xe0` | the whole info byte **× 4**, every tick |

(`+0x3e3b`, `+0x3e43`, `+0x3e6c`.) Both `aspd` and `aorgspd` move together (`+0x3e70`, `+0x3e73`),
so a `D` row's restore keeps the slid period and a later vibrato measures from it. A zero info
repeats `alastnfo`, written back into the cell (`+0x3e59`).

Both fine paths share the ordinary step's tail, which is why a fine form moves the period exactly
once: every later tick's guard sends an info at or above `0xe0` away before the step
(`+0x3e6a`).

### 9.5 G — the tone portamento

`+0x3f94`, in the **later-tick table only**: a `G` row's first tick does nothing, because both
tables' entry 7 is the bare `ret`. The period walks towards `asldspd` by `info × 4` a tick
(`+0x3fb6`) and clamps exactly onto it, never past (`+0x3fbc`, `+0x3fe3`), and `aorgspd` moves
with it (`+0x3fc4`) so the walk cannot slip. The parameter is remembered in `alasteff2`
(`+0x3fa1`), which is `L`'s source and nothing else's, and a zero info reuses it. The object
calls the interpolation handler at `+0x57b0` when `[0x919]` is set — the byte `S1x` writes, which
neither module carries, so that byte stays 0 and the call never happens.

`g_walks_the_period_to_the_notes_target_four_a_tick` drives MUSIC0's `G01` at pattern 8 row 16
channel 1 and asserts the walk tick by tick: the channel enters the row at period 1301 with a
target of 2603 (note `0x30` at instrument 7's `c2spd` 11000), the note sets the target and leaves
the live period alone, and the glide then moves four a tick. Its volume column is `0x40` and the
port holds **63** — §9.1's clamp, asserted there.

### 9.6 H — the vibrato, the stale bit, and which shift the demo runs

`+0x4005`, later-tick only. The parameter is the channel's `alasteff1` (`+0x2b`), and a missing
high nibble is **merged** into the previous parameter rather than replacing it (`+0x4010`), so
`H08` after `H81` is `H88`. The waveform is the sine at `+0xa50` — 64 signed words, ±255 — read
at the position's **byte** offset, and the position steps by `speed × 2` through the even low
seven bits, so the low byte is a phase in table entries and the high byte is never written (the
waveform functions put the *position* in `bx` before the table read leaves the value in `ax`).

The level is `value × depth` arithmetic-shifted by **5** — because `[0x915]`, the byte the shift
selects, is the **module header's flags word's bit 0** (`+0x5424`, `+0x5448`…`+0x5450`) and both
files' flags word is 8. So the demo runs the quieter of the object's two forms: the amplitude tops
out at ±119 rather than ±239.

Bit 7 of the position is a **stale flag**: the first-tick pass sets it for every command that is
not `D`, `H`, `K`, `I` or `R` (`+0x3cea`), and each waveform function tests it
(`+0x407b`'s `test $0x80,%ax`) — a stale position plays **0** and restarts the phase at
`speed × 2` instead of continuing it. A channel with no command does *not* set it (its pass takes
the no-command branch), and a `D` row does not either. In MUSIC0 the `H81` at pattern 47 row 6
follows an `A06` on row 0 and five rows with no command, so **the vibrato restarts there** —
which `h_vibratos_on_the_objects_sine_table` asserts, position and period both, against the sine
table written out in the test.

`K`'s vibrato half reads the same byte (`+0x4000`) and `L`'s tone-porta half reads `alasteff2`
(`+0x3f8f`) — while both their volume halves are `D`'s body and fall back on `alastnfo`. That is
the classic trap and it is three bytes, not one:
`k_reads_h_s_parameter_for_the_vibrato_and_alastnfo_for_the_slide` builds a module with `H81`,
then a `D10` row (which overwrites `alastnfo` with `0x10`), then `K01`, and asserts that the
slide follows the `D`'s byte while the vibrato follows `H`'s — a port that used one byte for both
would vibrate with a depth of 0 and never move the period.

### 9.7 J — the arpeggio

`+0x4190`, in both tables. `+0x419a` divides the engine's **own tick counter** by three and uses
the remainder: the first tick is the note, the second adds the high nibble's semitones and the
third the low nibble's. The transposition starts from `lastnote` and carries **one** octave when
the semitone passes B (`+0x41c8`), and the period is rebuilt from scratch — `aspd` only, so
`aorgspd` stays at the note's own and a row with no command restores the note exactly.

MUSIC0's `J7C` — 137 cells, all of them on channel 7 of patterns 42, 43 and 44 — is driven by
`j_transposes_the_note_by_the_tick_modulo_three` at instrument 4, the file's one instrument at the
8363 reference, so the periods are the table's own: **1712, 1140, 856**, cycling. The row runs
twelve ticks and the test asserts every one of them.

### 9.8 O — the sample offset, in bytes and in multiples of 256

`O` is in **neither** dispatch table. `+0x48d6` is inside the note work and runs while the row's
cells are read: `cmd == 0xf` writes `info << 8` into the channel's address field, so `Oxy` is
`y × 256` bytes — samples, for these modules' 8-bit samples. The offset is **persistent**: only a
cell that names an instrument clears it (`+0x481d`), and the offset applies to the *next* note to
start (`+0x491e`), which may be rows later. Neither module has an `O` without an instrument — the
count is asserted in `o_without_an_instrument_leaves_the_last_offset_in_place`, which drives that
form from a module written for the test:

```
cargo test -p sr-audio --test effects -- --nocapture o_without_an_instrument
  MUSIC0: 755 O cell(s), 0 without an instrument
  MUSIC1: 2417 O cell(s), 0 without an instrument
```

so the form the port implements for completeness is one no shipped cell reaches, and the
persistence it exists for (an `O` that leaves the offset for a later note) is not measured on
these files either.

### 9.9 Q — the retrigger, and what the object's `0x0101` filler means in a table

`+0x41ec`, in both tables. The interval is the **low** nibble and the volume code the **high** one
(`+0x41f6`), and the object's own test is `interval <= atrigcnt` (`+0x4205`): `y = 0` never
retriggers, and `y > 0` retriggers once the counter has reached `y`, after which the counter goes
to 0 and counts up again — so the fires land on the ticks congruent to 0 mod `y`. The counter is
the channel's `atrigcnt`, the same field `SDx` counts down in, and only a `D` row or a row with no
command clears it, so the phase runs **across rows**: the two `Q` tests here assert the fires at
their ticks over four and five consecutive rows. The retrigger itself is `m_pos = 0` (`+0x420f`)
plus the volume variation.

The variation is the pair of tables at `+0x95c` (signed addends) and `+0x96c` (multipliers in
sixteenths): `x`'s multiplier when it has one and its addend otherwise. **The multiplier table's
zeros are the object image's `0x0101` filler** — the same filler `_np_zframe` and `_zspeedadjust`
read as, §8 — and that is what the rule looks like applied to a *table* rather than a variable: the
image holds real bytes at 6, 7, 14 and 15 (10, 8, 24 and 32) and `0x0101` everywhere else, and at
runtime those slots are 0. Read the image literally and every code but those four would take a
multiplier of 257 — a sixteenth of which is the ×16 the code's shift makes of it — instead of the
addend they are meant to take; read it as the running program does and the two paths are the ST3
tables they look like. Which of the codes the files reach is measured by the scope test with the
letters (`cargo test -p sr-audio --test effects -- --nocapture the_cells_the_tests_drive`): MUSIC0's
84 `Q` cells carry codes `{0, 4, 7, 12, 14, 15}` on intervals `{0, 3, 4, 6}` and MUSIC1's 38 carry
code 0 on interval 3 — so three of the four multiplier codes (7, 14, 15) are on shipped cells and
**6 on neither file**, and the addends reached are 0 (both files), −8 (code 4, two cells) and +8
(code 12, two — the same code the `QC4` row of the table below drives).
`q_multiplies_when_its_code_has_a_multiplier` drives MUSIC0's `QF4` (code 15) and asserts the
doubling: 1 → 2 → 4 → 8 → 16 → 32 → 63 (the clamp), four ticks apart.

### 9.10 S — `SDx`, the note deferral

`S` is dispatched through a second pair of tables (`+0x424e` for the first tick, `+0x4261` for the
later ones, each indexing 16 subcommand entries at `+0x3b98` and `+0x3bb8`). **Both modules'
cells are `SDx` (subcommand 13) and nothing else** — the test asserts the subcommand set is
`{0x0d}` for each file and refuses any other — so the port carries `SDx` and records the rest as
absent rather than guessing at them.

`SDx` is not a delay in the sense of holding a note back: it is a *deferral*. The row parse sees
`cmd == S` with an info of `0xd0`…`0xdf` and **returns before it starts the note** (`+0x4751`…
`+0x475d`) while still installing the cell's fields and marking the channel used. The note then
begins wherever the countdown reaches zero: tick 0 puts the low nibble into `atrigcnt`
(`+0x4296`), every later tick decrements it (`+0x429a`) and the tick that empties it calls the
install continuation at `+0x475e` — the `0x81`-flagged channel, the clamps the row parse skipped,
and the note. So the note starts on tick `x`, with the cell's own volume column and instrument.

The three delays the files carry are **2, 3 and 6** (MUSIC1's `D2` 48 cells, MUSIC0's `D3` 77 and
`D6` 63), and each fires on the tick its low nibble names:

```
cargo test -p sr-audio --test effects -- --nocapture sdx_delays_the_note_to_its_own_tick
  MUSIC0 order 45: SD3 on pattern 41 row 0 channel 5, note 0x40 wants 977, speed 12
    it fired on tick 3
  MUSIC0 order 44: SD6 on pattern 40 row 1 channel 5, note 0x40 wants 977, speed 12
    it fired on tick 6
  MUSIC1 order 42: SD2 on pattern 30 row 2 channel 3, note 0x41 wants 922, speed 6
    it fired on tick 2
```

The speed matters, and the test reads it from the player rather than assuming it: a delay larger
than the row is long would never fire, and the port's own countdown is what makes that visible
(tick `x` is the `x`th tick after the row's first, and a row that ends before it has no tick `x`).
At these three cells the rows are long enough for all three — MUSIC0's two sit at speed 12 and
MUSIC1's at 6 — and the test asserts the tick, the period the note finally takes, and the cell's
own volume column arriving with it.

### 9.11 The period's step, and a constant 400 short of the product

Every pitch effect above is measured in periods; what turns a period back into playback is
`+0x582b`, and Task 5's mixer will consume it. Its base frequency is the object's own literal:
`mov $0xda,%dx; mov $0x7600,%ax; div period` (`+0x58b9`), i.e. **`0x00da7600` = 14,317,056** as a
16.16 value over the period. That is **not** `8363 × 1712` = 14,317,456: the object's constant is
400 short of the product of the format's two reference numbers, so C-4 at the 8363 reference
(period 1712) is stepped at **8362** and the octave above at **16,725** rather than twice 8362.
The port uses the object's number, and `a_period_s_step_is_the_base_frequency_over_it` asserts
the table's own periods against it. A period of 0 zeroes the mixer's speed and returns
(`+0x5876`).

### 9.12 What the modules do not carry

Recorded as absent **with the count**, so that "not implemented" is not read as "assumed zero":

- **The letters `I`, `L`, `M`, `N`, `P`, `R`, `U`, `V`, `W`, `X`, `Y` and `Z`: 0 cells in each
  file.** (`R` is tremolo and `L` is `D` + `G`, both of which the object implements in full — the
  port keeps the fields they would write, `atreon`, `atremor` and `alasteff2`, and has no dispatch
  for either; a `K`'s vibrato half is `H`'s and is carried.)
- **`T`: 0 cells**, as the phase plan's amendment says. The first-tick table's entry 20 still
  points at the A/T handler and the port dispatches it there, because that is what the object
  would do — it is the object's table and not a behaviour the data reaches.
- **`S` subcommands other than `d`**: 0 cells. `SAx` (panning), `SBx`, `SCx`, `SEx`, `S0x`…`S4x`
  all have handlers in the object's two tables; the port has none, and would need the bytes of the
  subcommand it was asked for before it wrote one.
- **The other three vibrato waveforms**: the selector `vibtretype` is written only by `S3x`/`S4x`
  (0 cells), so every channel in both modules runs selector 0 — the sine at `+0xa50`. The ramp
  (`+0xb50`), the square (`+0xad0`) and the sine-plus-random (`+0x40b4`) are in the object and
  unreachable from this data, and the port implements the sine.
- **`aequ`'s other values**: the mixer's, for Task 5. The port carries the byte and the four
  writes to it that these effects make.

### 9.13 What Task 4 corrected in Task 3's reading

Three places where the object and the earlier reading disagreed. All three are in the object's
bytes, all three are asserted by tests in this commit, and the first two changed a number the
note quotes:

1. **`A` and `T` are *not* one handler told apart by the parameter** — the *command* picks the
   handler and the tables say which. Command 1 is the first-tick table's entry 1, `+0x3dd2`,
   whose whole body is `cmp $0x0,%al; je; xor %ah,%ah; mov %ax,0x2002` — **no upper bound**, so
   `A7F` sets a speed of 127. Command 20 is entry 20, `+0x3d73`, the only handler with the
   `cmp $0x20,%ax; jbe` split, and its ≤ 32 branch goes to the driver call at `+0x3dce`, never to
   the speed. Task 3's player had the split in the handler both commands shared; MUSIC1's `A`
   parameters are `{3: 30, 6: 23, 127: 1}` and its one `A7F` (pattern 72, row 48, order 98) set a
   *tempo*. No filed trace reaches that cell (MUSIC1's trace covers orders 0 and 42…48) and the
   end-to-end play does: this correction is +2606 of the +2849 polls §8's MUSIC1 numbers moved
   by, the rest being the volume-0 path's fix — §8 separates them and measures each.
2. **A note on a channel that is not playing starts at the sample offset too.** `+0x491e` is
   reached from *both* of `+0x490e`'s branches — the fresh one jumps straight to it — so the
   `m_pos` the `O` command left is taken whether or not a sample was already playing. Task 3's
   port had the assignment inside the not-fresh branch, which would have left the first note of
   an `O` row at position 0; `o_starts_the_next_note_at_the_offset_in_bytes` asserts the position
   and the offset both, on MUSIC0's own `O05` cell.
3. **A seek clears every channel** (`+0x5614`, which `_zplaysong` reaches at `+0x258e`), so the
   effects start from zero after a `play_song` — the period, the volume, the remembered
   parameters and the vibrato position. Task 3's port did not, which no trace could show (every
   filed trace seeks on a fresh player) but which a test that seeks mid-song can: the `K` test
   above finds `alasteff1` at zero because the seek cleared it.

### 9.14 The tests, and the numbers they print

One test per effect, each driving the **modules' own cells** and asserting the channel state tick
by tick; where the shipped data does not reach a form (the fine volume slides' isolated starting
volume, `O` without an instrument, `K`'s shared parameter, the stale vibrato bit) the test builds
a module of its own and says so. Every expected value is the object's arithmetic written out
again in the test file — the note table, the shifts, the sine and the `Q` tables are pasted there
as the object's own bytes, and `tests/player.rs`'s claims assert those same bytes in the image.

```
  MUSIC0: A:63 B:3 C:8 D:7348 E:827 F:1051 G:802 H:212 J:137 K:105 O:755 Q:84 S:140
  MUSIC1: 0x00:1 A:54 C:4 D:3340 E:217 F:180 G:2607 H:365 K:62 O:2417 Q:38 S:48
  MUSIC0: S subcommands {13: 140}, delays {3: 77, 6: 63}
  MUSIC1: S subcommands {13: 48}, delays {2: 48}
  MUSIC0: Q codes {0: 49, 4: 2, 7: 11, 12: 2, 14: 1, 15: 19}, intervals {0: 48, 3: 7, 4: 28, 6: 1}
  MUSIC1: Q codes {0: 38}, intervals {3: 38}
  MUSIC0: 27 fine-up (0x?f) and 144 fine-down (0xf?) D cells
```

Those six lines are one test's output (`the_cells_the_tests_drive_are_the_modules_own`) in the
order it prints them; the seventh is printed by a **different** test,
`the_fine_volume_slides_move_once_on_the_first_tick`, which is the next test in the serial run.
The capture is `cargo test -p sr-audio --test effects -- --nocapture --test-threads=1`;
the harness's own prefix and result dots are stripped, the lines are otherwise as printed.

The per-effect tests and their cells:

| effect | cell the test drives | what it asserts |
|---|---|---|
| `D` | MUSIC0 order 0, pattern 0, row 0, channel 0 (`D10`, then `D00`) | 2, 3, 4, 5, 6, 7 then 8…13, and that the `D00` reused row 0's parameter |
| `D0F` | MUSIC0 order 2, pattern 9, row 60, channel 7 | fifteen a tick from the volume the row found, clamped at 0 |
| `D`'s fine forms | MUSIC0 order 1, pattern 1, rows 8 (`D1F`) and 15 (`DF1`), channel 5 | ±1 once, on the first tick only |
| `E`, `F` | MUSIC0 order 0, pattern 0, rows 11–13, channel 3 | `E02`/`E00`/`F02` at 8 a tick from the note's own 3579, and the fine path doing nothing |
| `F` extra-fine | MUSIC0 order 11 pattern 8 row 32, and order 45 pattern 41 row 47 | 4 × the low nibble once |
| `G` | MUSIC0 order 11, pattern 8, row 16, channel 1 (`G01`) | the target from the note, the period gliding 4 a tick, and the volume column clamped to 63 |
| `H` | MUSIC0 order 24, pattern 47, row 6, channel 6 (`H81`) | the phase and the period, tick by tick, from the sine table — stale at the row's first tick |
| `H` stale bit | a module written for the test (`E00` then `H82`) | the phase restarts at 0 instead of continuing |
| `J` | MUSIC0 order 46, pattern 42, row 8, channel 7 (`J7C`) | 1712, 1140, 856 cycling over the row's twelve ticks |
| `K` | a module written for the test (`H81`, `D10`, `K01`) | the slide from `alastnfo`, the vibrato from `alasteff1` |
| `O` | MUSIC0 order 1, pattern 1, row 0, channel 2 (`O05`) | 1280 in the offset and the position |
| `Q` | MUSIC0 order 36, pattern 23, row 58, channel 7 (`QC4`) | the fires every fourth tick over four rows, +8 each |
| `Q`'s multiplier | MUSIC0 order 24, pattern 47, row 56, channel 2 (`QF4`) | the doubling and the clamp at 63 |
| `SD2`, `SD3`, `SD6` | MUSIC1 order 42, MUSIC0 orders 45 and 44 | the tick each fires on |
| the volume-0 path | MUSIC1 order 0, pattern 73, row 3, channel 2 (`C00`) | the channel freed before the break, and the order still 0 at row 63 |
| the `A` correction's own row | MUSIC1 order 98, pattern 72, row 48 (`A7F`) | 48 rows at 6 then 16 at 127 — 2320 ticks for the order, 1936 more than those sixteen would take at 6 |
| the periods | the tables themselves | every octave of the note table, the c2spd scale and the step |

The trace comparison cannot judge any of this (a wrong `D` does not move the timeline), which is
why the tables in this section are asserted twice: once here, as numbers, and once against the
object's image in `tests/player.rs`.

## 10. The mixer — Task 5

The mixer is `crates/sr-audio/src/mix.rs`: the voices, and the ring the audio device drains. It is
the end of the chain `module.rs` (the container) → `player.rs` (the tracker) → `effects.rs` (the
channel effects) started, and it is what makes the two modules audible. The evidence is

```
cargo test -p sr-audio --test mix -- --nocapture
cargo test -p sr-audio --lib mix::                          # the tables and the step
cargo test -p sr-audio --test player -- --nocapture the_objects_own_bytes
```

Every offset below is `MAIN/STMIK.300`'s segment offset and every load-bearing one is **asserted
against the object's own bytes** by `tests/player.rs`'s claims table, which this task grew from
**131 claims to 147**, and the fix wave to **151**: the per-frame loop's body, the volume table's
build, the scale table, the centre read and its pan shifts, the two pan shift tables, the two
divisions of the step, the stereo divisor's selection, the two end paths, the note start's
bounds, the output conversion's two corrections, and the fix wave's period clamp and `m_pos`
gate.

### 10.1 Where the mixer is, and how it is called

`_zpollme` (`+0x261b`) checks `_zpollmix` and calls `+0x51c5`; the reference runs polled
(`_zpollmix` is 1 from the music's start, `music-clock.md` §5), so that call is once per copper
frame (`MAIN/COPPER.ASM:466`). Inside it, the DMA loop — `+0x4ff8`'s tail falling into `+0x50d0`
— is:

| offset | what it does |
|---|---|
| `+0x50d0` | how many bytes the DMA buffer still wants |
| `+0x50e6` | the chunk: that count, or the tick's countdown, rounded **up to a multiple of 64** (`add $0x41,%ax; and $0xffc0,%ax`) |
| `+0x510c` | **mix the chunk** — `call 0x3905` |
| `+0x512d` | copy the chunk to the DMA buffer's real home (the `movsb` loop at `+0x3ad7`, for EMS) |
| `+0x5150` | **`call 0x4353`, the tick** |
| `+0x5157` | `[0xbe0] += [0x8e1]`, one tick's samples, and loop |

So the samples before a tick are mixed before that tick runs. The port's `Player::poll` keeps that
order: it hands the mixer the samples up to each tick boundary, then runs the tick, then mixes
what is left of the poll.

The mixer's own entry is `+0x3905`. It halves the count in stereo (`+0x396a`: `shrw $1,[0xbde]` —
a byte count becomes a frame count), walks **eight** channel structs for the first side
(`+0x396f`…`+0x398c`, `bp` from `0xda0` to `0x10a0`), sets the pass flag `[0x211e] = 1`
(`+0x3996`) and walks the **next eight** for the other (`+0x39ac`…`+0x39c9`, `bp` from `0x10a0` to
`0x13a0`); in mono it walks all sixteen into one accumulator (`+0x39cd`, `[0x211c] = 8`). Each
walk calls the voice routine `+0x3765` for a channel with a sample and a nonzero step
(`+0x377e`'s `or`/`je`).

`+0x3765` is the whole voice, and it is where the three numbers the port keeps come from:

- **the sample** — `es` = `m_base`'s high word (`+0x3795`/`+0x37a2`) and `si` = `m_pos`
  (`+0x378a`), so the object's position is a **byte offset into the sample**;
- **the bounds** — `m_end` (`+0x48`) and `m_loop` (`+0x4c`), read at `+0x383c`;
- **the two cached words** — `m_speed` (`+0x50`) and `m_vol` (`+0x41`).

**Which channels it walks is two tests, not one.** The walk's caller tests `m_speed`
(`+0x377e`'s `or`/`je`), and the voice routine tests it again at `+0x3783` and then tests
`m_pos == 0xffff` at `+0x378e`/`+0x3791` — both jump to the routine's return before the sample or
the centre is read. The second is the "volume 0" optimisation's marker: a channel that path frees
has its position parked at `0xffff`, and the object's mixer then skips it entirely. The port's
`voice_contribution` tests the same word. At volume 0 the freed voice's immediate contribution is
zero, so the gate changes nothing *while the channel stays silent* — but it is not value-free over
a window: without it the port walks the position of a voice the object leaves alone, and a channel
that later becomes audible without a note restart reads a different sample. The measured cost over
MUSIC0's first 4,120 polls is 33 of the window's 2,351,396 samples above the silence entry:
**972,036 with the gate off against 972,003 with it on** (`the_window_is_not_silent` prints the
972,003; the gate is on in the tree). The gate fires 398 times in MUSIC0 and 1,068 in MUSIC1 over
120,000 polls, every one at volume 0.

### 10.2 The voice: the arithmetic the port reproduces

**The volume table** is built by `+0x551c` (called from `+0x22d0`, `+0x2792`, `+0x53af` and
`+0x5420`, i.e. at the sound init, at each module load and whenever the header watch sees the
master volume move). It allocates (`+0x29a9`) 0x5000 bytes at `cs:[0xbe4]`, zeroes the second
0x1000 — the accumulators — at `+0x2943`, and fills **64 levels × 256 sample values** with
`(sample × scale[level]) >> 8` (`+0x5553`'s `mul %bx`, `+0x5555`'s `mov %ah`). `scale` is the
object's own 64-byte table at `+0x9c4`, whose 64 values are exactly **`floor(level × 255/63)`** —
read entry by entry out of the image and asserted by `tests/player.rs` at `+0x9c4` (0, 4, 8, 12),
`+0x9ff` (level 59's 238, where four a level would be 236) and `+0xa03` (level 63's 255). So the
top of the range is a hair below full scale — a sample of 255 at level 63 maps to 254 — and
`level × 4` is the table's shape only up to level 20. **A sample byte is the S3M file's own unsigned value**: 128 is the centre,
which is what `+0x37ae`'s `mov $0x80,%bl; mov %fs:(%bx),%al` reads (`bh` still holding `m_vol`) and
what `+0x30b4` subtracts again.

**The volume** is `m_vol = (avol × master) >> 8` (`+0x5925`: `mulw 0x913`, `mov %ah`), where
`[0x913]` is `min(global volume byte, 64) × 4` (`+0x55c4`…`+0x55e1`). Both modules' global-volume
byte is **64** (§5), so the master is **256** and `m_vol` is the channel's own volume — which the
row parse already clamps to 63 (`+0x477b`). The shell's `fademusic` (`MAIN/U2.ASM:333-346`) walks
that module byte down, which is what `Player::set_global_volume` carries.

**The step** is `m_speed` (`+0x50`), 16.16 sample bytes an output frame: `+0x58b9` divides the
object's own `0x00da7600` by the period **as a 32-bit integer** (the ≤ 218 branch at `+0x58a9`
computes the same value in two halves; the remainder is discarded at `+0x58c1`'s `xor %dx,%dx`),
and `+0x58c9`…`+0x58d8` divides *that* by the mixing rate, the quotient into the step's high word
and the remainder's next 16 bits into its low word. A period of 0 returns with `m_speed = 0`
without dividing (`+0x5876`), which is how a note off silences a voice.

**The period is clamped before the division.** `+0x582b` compares `aspd` with the two words at
`0x958`/`0x95A` — the image holds **`0x7FFF`** and **`0x0040`** — and substitutes the bound when
the period is outside them (`cmp 0x958,%cx; jbe` at `+0x5863` then `mov 0x958,%cx` at `+0x5869`;
`cmp 0x95A,%cx; jae` at `+0x5890` then `mov 0x95A,%cx` at `+0x5896`), with the zero test between
the two (`+0x5876`) so 0 still returns 0. The divisions above never see a period outside
`[64, 32767]`, and `step` clamps the same way. **On the shipped modules the clamp is inaudible**:
the probe measured periods above `0x7FFF` **2,408** times in MUSIC0 and **2,635** in MUSIC1, every
one on a channel whose volume is 0 (a note-off parks `org_period` at `0xffff`, `player.rs:890-891`,
and a vibrato adds its level to it), and the low bound never binds at all — the smallest non-zero
period seen was `0x81`. The object also writes the clamped word back to `aorgspd`/`aspd`, but only
when the module's flags word at `0x917` has bit 4 set (`+0x583d`, `+0x586e`, `+0x589a`); both
modules carry flags 8, so that write-back never runs and the port does not model it.

**The mixing rate** is `[0x8d9]`, chosen by the header watch at `+0x539b`…`+0x53a9` as
`_zmixspeed` (mono) or `[0x8d7]` (stereo) from the master volume byte's bit 7. The shell doubles
`_zmixspeed` to 40000 for SB Pro (`MAIN/U2.ASM:759`) and **never writes `[0x8d7]`** — its only
store is `+0x228f`, in the PC-speaker initialiser, which the reference's mode 5 never reaches — so
the reference's stereo voices divide by the segment's initial data, **21111**. The frame rate is
20000 a second in stereo (40000 bytes of two), so the reference's stereo playback is
**5.26 % flat** — `21111` is 5.56 % larger than 20000 and `1/1.0556` is the flatness, 94 cents,
just short of a semitone's 5.95 % — while its mono playback is exact. The port keeps the object's word; a
run at `MixRate::Hz40000` in stereo is flat by the same factor, and `MixRate::Hz40000` in mono is
not. `mix.rs`'s `a_step_is_the_objects_two_divisions` and
`a_step_clamps_the_period_the_way_the_object_does` print and assert the steps.

**The fetch and the accumulation**: `+0x346e` reads one byte at `es:(si)` — no interpolation, and
the interpolating dispatch entries are `S1x`-gated with `[0x919]` = 0 in both files (§9.12) —
`+0x3473` looks it up in the volume table at `(m_vol << 8) | sample`, and `+0x3482`'s 32-bit `add
%eax,(%di)` puts two frames' values into the two 16-bit accumulator words at once. **The channel's
accumulator is 16 bits and is biased**: the sound init fills the buffer with `0x8080` (`+0x293b`)
and the flush replaces each pair with `0x01010101` (`+0x2ce5`, `+0x30d0`) — the same filler
`+0x5628` writes into `m_pos`, and a marker rather than a sample value in both places.

**The two clamps of the position's window** are `m_safe`/`m_lsafe`, computed at `+0x58e3`…`+0x5916`
as `((m_end - m_pos) << 16 - m_poslow) / m_speed`, forced odd (`or $1`), incremented, and
**clamped to 0x8000** when it does not fit — `cmp $0x7ffe` then `or $1; inc`, so a voice that
cannot reach its end inside the window is *stopped* (`+0x3834`'s `cmpw $0x8000,0x56(%bp)`). That is
the object's chunking device: the port mixes a frame at a time and tests the end per frame, which
produces the same samples because the rewrap's displacement is congruent to the loop's length
either way.

**The end**: `m_end` and `m_loop` come from the instrument header at the note start (`+0x4872`…
`+0x48cb`), and they are the header's own two loop fields: **`+0x4890` reads `lend`** — the
loader's `struct ds_smp` is the file's 0x50-byte header read verbatim (`STARTEMS.C:28-48`: type,
filename, memseg2, memseg, length at 0x10, **lbeg at 0x14, lend at 0x18**, vol, disk, pack, flags
at 0x1f) — and `+0x489c`'s `cmp $0x0,%ax; je` therefore sends a sample whose **loop end is zero**
down the one-shot path, while `+0x48a6` stores **`lbeg`** (`es:0x14`) into `m_loop`. Reading those
two the other way round gets the same two *bounds* and the wrong *rule*: the object lets a sample
loop whenever its flag is set and its loop end is past its loop begin — `checkins` clears the flag
when `lend <= lbeg` (`STARTEMS.C:145`) — and **five of MUSIC0's twelve looping samples and one of
MUSIC1's nine are full-sample loops with `lbeg` 0**, which that first reading stopped after one
pass (`tests/mix.rs`'s census prints the six). `m_end` is not always `lend`: when `lend` is at or
below 500 it is reduced by whole loop lengths until it is small (`+0x48ab`…`+0x48b9`), a
**memory-layout hack** so that a mixer reading past the position stays inside the 512 bytes the
loader copies in behind the loop (`STARTEMS.C:157-161`) — and on both shipped files **that
reduction never runs at all**, because every looping sample's loop end is well past 500. A sample
that does not loop takes `+0x48be`'s path: `m_end` = length + 0x20 — inside the 512 bytes of 0x80
`checkins` writes behind a one-shot (`STARTEMS.C:167-172`), so its slack holds no sample data —
and `m_loop` = 0xffff. When the position reaches `m_end`, `+0x3840` tests that marker, and a voice
with a real loop rewraps by adding `m_loop - m_end` to the position (`+0x3856`/`+0x385a`) — the
overshoot and therefore the loop's phase survive — while a one-shot stops: `m_speed = 0`
(`+0x384d`). The port keeps `lend` itself rather than the reduced value: the displacement
`m_loop - m_end` is a whole number of loop lengths either way, so the phase is the same, and on
these modules the two are the same number.

### 10.3 The output: the table the machine holds, and the base it is indexed at

Each voice's contribution is added into a 16-bit accumulator and the sum is converted at the end
of the buffer by `+0x2c89` (the stereo flush) or `+0x30ad` (the mono one). Both add the object's
own DC correction — `0x400 - [0x2250]`, the sum of the channels' centre values — to every
accumulator word (`+0x2cb4`, `+0x30b4`) and index the table the same builder wrote at
`cs:[0x6c]` (`+0x551c`'s second half). Since 0x400 *is* 1024 and the table's own middle is index
1024, the quantity the object indexes with is exactly **the accumulator less the centre sum**: the
signed sample, in a range of about ±1000 at these modules' volumes.

That table is a **per-run gain curve** built from `_np_mastermul` (`+0x5569`: the master volume
byte's low seven bits, floored at 0x10). The reference's master volume byte is 0xB0 (§5: 176 =
stereo | 48) → `mul = min((48 × 11) >> 3 - 2, 127) = 64` (`+0x5362`…`+0x538f`) → 2240, the word
`music-clock.md` measured — and the builder's own loop (`+0x5565`…`+0x55c1`) is a zero fill of
the span's 0x800 entries down to where the ramp starts, then `K = 0x8000/mul` ramp entries of
`(k × (65536/K)) >> 8`, one output level per `65536/K` entries, which at `mul` 64 is one level
per two entries. **That is the loop's intent, and not what the machine's memory holds**: the
zero fill is half the length the arithmetic implies and the span above the ramp is `0xFF`, for
the reason the next paragraphs measure.
(The measured 2240 is the *published word*, not the multiplier: `+0x5397` stores a **byte** into
`[0x916]` and the byte above it is `_np_masterflags`' low byte, which the header watch has set to
the modules' 8 — so the word reads `0x08C0`. The builder reads the byte: `+0x5569`'s `mov
0x916,%bl`.)

**The port reproduces the table the machine holds.** `mix.rs`'s `multiplier` is the header watch's
derivation from the module's master-volume byte — the modules' 0xB0 gives `mul` 64 on a stereo
output and 48 on a mono one, because the stereo bit is what doubles the volume — and
`Curve::build` is the builder's own loop **as the machine executes it**: `K = 0x8000/mul` ramp
entries of `(k × (65536/K)) >> 8` after `2·⌊(0x800 − K)/4⌋` zero entries (the two `shr $1,%cx` at `+0x5592`
and `+0x559b` halve the count twice), then `+0x55b5`'s shortfall as `2·⌊rem/2⌋` bytes of `0xFF`.
For `mul` 64 that is 768 zeros, the ramp at 768…1279 and `0xFF` from 1280 — the boundaries the
probe read — and for the port's own **mono** `mul` 48 the zero fill is 682 where a plain halving
would write 683, which the unit test builds both ways and asserts differ.
`Mixer::frame` forms each voice's contribution as its table value less its own centre (the
quantity the object forms by subtracting the summed centres at `+0x30b4`), indexes the curve with
it, and widens the byte the card would play: `(byte − 128) × 256`.

**The index's base is `0x400 + value`, and the marker is not applied.** The flush does re-mark
what it reads — `+0x2cf1` and `+0x30e8` write `0x0101` *after* the read, and `+0x3482` adds into
the plane — so the code's own index would be `0x501 + value`, 257 entries higher. The
measurement says the audible path does not land there: the DMA buffer (`[0xbe6]:0xC9B0`) is
centred on the byte 0x80, which is the ramp's `(1024 − 768)/2` at index 1024, while `0x501`
would put silence inside the `0xFF` tail the bug leaves. The port follows the machine
(`Mixer::base`) and keeps the marker's bookkeeping in the code unapplied; **why** the audible
index behaves otherwise is open and named in A14 — a re-dump of the DMA buffer beside the plane
it came from is what would settle it.

**The table the machine's memory holds is not the table the builder writes**, and the
difference is measured: `cargo xtask music probe --dump` reads `cs:0x6E+0x000`…`+0x7FF` across a
whole window and finds the zero region at **half** its length, the ramp 768 bytes early and the
rest filled with `0xFF` — because `+0x559d`'s `rep stosw` leaves `CX` = 0 and `+0x55a1`'s `rep
stosb` therefore writes nothing, which makes `+0x55bd`'s tail fill the shortfall with `0xFFFF`.
`APPROXIMATIONS.md` A14 is the measurement. The port **plays the table the machine holds**: the
halved zero fill, the ramp where the bug leaves it, the `0xFF` tail, indexed at `0x400 + value`
— the base the audible buffer supports, since `[0xbe6]:0xC9B0` holds a waveform centred on 0x80,
which is the ramp's byte 128 at index 1024. One clause of A14 is open and named there: the
flush's `0x0101` marker, which would put silence 257 entries higher, inside the tail. The table
is not one run's: the re-review's dump of the **MUSIC1** window (`--cycles 30000 --arg 3
--frames 600..900`) reads all 2048 bytes of the span *identical* to MUSIC0's, one change line,
`cs:0x6C` = `6E 00` in both — so the port's table is the release of record's own behaviour, in
either module's run. (Round 3's report of a second, gentler table under MUSIC1 was withdrawn in
round 4: the offsets it quoted are absolute segment offsets read as if they were table-relative,
and the boundaries it predicted came from `0x8000/48` = 341 where the instruction divides by
682. A14(b) is the record of the withdrawal.)

**What that means for the sound, measured.** The table the machine holds is not a gate: 768
zero entries, the `mul` 64 ramp at 768…1279 (one level per two entries) and `0xFF` from 1280, so
a frame's byte is the ramp's own one level per two of value — `⌊(value + 256)/2⌋`, ±64 byte
levels for a ±128 signal — and flat outside it: `0x00` at or below −257 (the zero region) and
`0xFF` from +256 (the tail). Over MUSIC0's first 4120 polls (`cargo test -p sr-audio --test mix --
--nocapture the_window_is_not_silent`): **972,003 samples above the silence entry, peak 32512,
256 distinct values**, and its first 120 polls: **3,719 of the window's samples are the silence
entry, 115 distinct values, −15104 … 15360**. That is the release of record's own table doing
what it does, and A14 carries the measurement.

**The stereo** is the module's own channel table: `+0x37b5` reads the channel's `amixtype` byte
(`+0x1b`), masks it to three bits, flips its low bit with the pass flag (`+0x37c1`'s
`xor 0x211e,%bl`) and adds the pass's base (`+0x37c5`), which selects both the mixing loop
(`+0x38c4`'s table) and the two shift tables at `+0x38e4`/`+0x38f4`. The file's 32-byte table
(`Module::channel_pan`) is `0, 8, 1, 9, 2, 10, 3, 11` for channels 0…7 and 255 for the rest, and
the S3M convention reads 0…7 as the left positions and 8…15 as the right ones — so the eight
channels the music uses are hard-panned L, R, L, R, L, R, L, R, and the port's `side_of` reads
exactly that. A **mono** run has one accumulator and no pan at all (`+0x39cd` walks all sixteen
channels into it), which the port keeps.

### 10.4 The port's numbers

`tests/mix.rs` computes its expected samples **by hand from the S3M file's own bytes** — the
tables pasted into the test file, the two divisions of the step written out, the 16.16 position
accumulated a frame at a time — and never from a recording of the port. The load-bearing one takes
MUSIC0's eleventh sample (instrument byte 11), channel 0 (the module's leftmost pan), level 63,
and the note C-4's period 1712:

```
MUSIC0 instrument 11, channel 0, level 63, period 1712, step 13700 (16.16), mono, linear curve:
  sample bytes [115, 115, 115, 115, 115, 176, 176, 176]
  the first eight samples [-512, -512, -512, -512, -512, 1536, 1536, 1536]
```

The step is 14317056/1712 × 65536 / 40000 = **13700** in 16.16 — 0.209 of a sample byte a frame, so
the positions of the first eight frames are 0 five times and then 1; the level's scale is 255 and
its centre (128 × 255) >> 8 = **127**, so the sample's 115 is **−13** and its 176 is **+48**.
Every sample is read at the audible base — the index is `0x400 + value`, the same base for the
whole window, first chunk included — so −13 gives 1011 → the byte 126 → **−512**, and +48 gives
1072 → the byte 134 → **1536**. (The marker's `0x501 + value` would make those two reads 1268 and
1329, the bytes 158 and 166 — 7680 and 9728 — which is why the port does not apply it: A14(a).)
The curve is the *linear* one because the run is mono and this test's module asks for it by its
master-volume byte, so the voice's own arithmetic is visible through it. The modules' own byte
gives `mul` 64, which the next test pins on the table the machine holds: a solo voice's value is
at most ±127, so its byte spans the ramp's 64…191 — **audible, neither gated nor clipped**, +63
above the centre and −64 below it, one level per two of value. The other hand-computed tests do
the mono sum of two channels through one curve, both loop kinds and the one-shot's stop against
the modules' own instruments, a level of 0 as the curve's own zero entry, and `_zshutup`'s
silence at any volume.

The stream's properties, gathered from **six tests** in `cargo test -p sr-audio --test mix --
--nocapture --test-threads=1`. Each line is a paste and the `(…)` marks the elision between
tests — the lines are never adjacent in a run, so without the markers the block would read as
one test's output. The tests, in the order their lines appear:
`a_poll_produces_exactly_the_samples_it_says_it_does`,
`the_window_is_not_silent` (two lines), `a_window_of_rows_carries_its_samples`,
`two_runs_of_a_window_are_byte_identical`, `a_device_that_stops_draining_loses_the_oldest_samples`
and `the_modules_loops_and_the_objects_zero_loop_end_rule`.

```
the first five polls' sample counts: [570, 571, 571, 570, 571]
  (…)
MUSIC0's first 120 polls: 68487 samples, 3719 of them the curve's silence (0), 115 distinct values, quietest -15104, loudest 15360
  (…)
MUSIC0's first 4120 polls (2351396 samples): 972003 above the silence entry, peak 32512, 256 distinct values
  (…)
64 rows at speed 6: 320178 samples, predicted 319872, difference 306 (0.0957 %), one tick is 833
  (…)
a 600-poll window of MUSIC1: 342436 samples
  (…)
after 15 undrained polls the ring holds 8192 samples and has dropped 368
  (…)
MUSIC0: 12 samples loop, 5 of them from 0 to the sample's end; 10 do not loop, 0 of them with the flag set but no loop
  (…)
MUSIC1: 9 samples loop, 1 of them from 0 to the sample's end; 32 do not loop, 0 of them with the flag set but no loop
```

- **The poll's count is exact.** The five counts are Task 3's pinned `samples_this_poll` sequence,
  which is the strongest form of "the poll produces the samples it says it does": the drain sees
  exactly that many i16 every poll. A stereo budget that is odd — the counts `570, 571, …` and
  the tick's own 833 both are — splits one frame across two calls: one i16 is held back and sent
  at the head of the next, which is what a byte-stream DMA does when a boundary falls between a
  frame's two bytes.
- **The samples-per-tick relation holds.** 64 rows at speed 6 is 64 × 6 = 384 ticks of 833
  samples = 319,872 predicted against 320,178 mixed: a difference of **306 samples, 0.0957 %**,
  well inside one tick's worth (833), and the residue is the row-boundary sampling of a window
  whose polls do not divide evenly into ticks.
- **Two runs of a window are byte-identical** (600 polls of MUSIC1, 342,436 samples, == itself),
  and a different order is a *different* stream, so the equality is not vacuous.
- **The window is not silent.** Over the first 120 polls 3,719 of the 68,487 samples are the
  curve's silence entry (0 — the ramp's centre, the byte 0x80) and the rest take **115** distinct
  values, −15104 … 15360; over 4120 polls — a minute — **972,003** of 2,351,396 samples are above
  that entry, the peak is **32512** and the window reaches **256** distinct values, i.e. every
  byte from 0x00 to 0xFF. The mixer is producing signal through the table the machine holds, and
  that table is not a gate: a value of ±128 moves the byte ±64, so a voice is audible without
  being zeroed and without being clipped.
- **The drain takes whole samples and moves no state**: `take_samples` leaves the engine
  position, the published position and `_np_zframe` where they were, and a second drain returns 0.
- **The overflow policy is "drop the oldest"**, exercised through the player: after 15 undrained
  polls the ring is full at 8192 samples and has dropped 368, and what remains is the newest.

### 10.5 Why there is no null test against the original, and what stands in for one

**There is no audio reference.** The emulator runs silent by D17 (`SDL_AUDIODRIVER=dummy` plus
`mixer_nosound=true`), so nothing in `reference/` holds a sample of the original's output and no
null test is possible — not "not yet run", but *not obtainable by this port's oracle at all*.
What this section's evidence therefore is:

1. **By construction**: the tables, the two divisions and the 16.16 position are the object's own
   bytes, asserted byte for byte in `tests/player.rs`'s claims (151 in all), and the expected
   samples are computed from the S3M file's own sample data in `tests/mix.rs`.
2. **By invariant**: determinism, non-silence, the exact per-poll count and the samples-per-tick
   relation, each asserted separately above.
3. **By ear**: `cargo run --release --bin secondreality` — the engine had no seam to the mixer
   when this section was written (that is §11, Task 6) and the adapter and the device that would
   carry the samples to a card are Task 7's, so listening to it is Task 7's acceptance test and
   not this task's claim.

**What is not claimed here.** That the mixed sound is the sound of the 1993 release. One thing is
known to differ and one to be odd: the curve's index is **clamped** where the object's 16-bit `bx`
would read its own segment's neighbours past the table's end, and the arithmetic below the curve
is the object's as its bytes have it — including the 5.26 % stereo flatness its own `[0x8d7]`
produces and the table its own `mul` of 64 builds, bug and all. The measurement of that table is
in §10.3, and it is the phase's to judge, not this section's to soften.

## 11. The engine's seam — Task 6

`crates/sr-engine/src/music.rs` is the whole of what a part knows about the music: a `Music` trait
whose eight methods are the six `dis_*` calls `DIS/DIS.H:37-59` declares, the poll the engine makes
on the part's behalf, and the position (`MusicPosition`) a status line or a test can read. The
engine holds one as `Option<Box<dyn Music>>`; a part reaches it through `PartCtx::music()`, which is
`None` when the run has no player.

**Where the impl lives, and why it is not here.** The demonstration's own implementation,
`impl Music for Player`, belongs to the **binary** (Task 7), not to either crate: `sr-audio` does
not depend on `sr-engine` and `sr-engine` has no runtime dependencies at all — a rule the manifest
test in `crates/sr-engine/src/lib.rs` reads as text — so neither crate can name the other's types,
and the only crate that can is the one that depends on both. This task ships the trait, the wiring
and the stub; nothing in `sr-engine` mentions a module, a pattern or a sample rate. The three
translations below are here rather than in the adapter because they are the *DIS's* arithmetic —
the parts' side of the seam, read out of `DIS/DISINT.ASM` — and the adapter's whole job is to hand
them the player's published words.

### Where the poll goes

`copper_doit` (`MAIN/COPPER.ASM:449-468`) runs, in this order: slot 0's routine if the flag allows
it (`:458-464`), **`call _zpollme`** (`:466`), and `u2copperint` (`:468`) — which is not a copper
slot at all but the shell's control-key check (`MAIN/U2.ASM:286-289`: `call ctrldown`). Slots 1 and
2 are the retrace routine's (`:302`, `:322`). So the port's tick is **slot 0, the music's poll, the
bands, slots 1 and 2**, and two tests pin it: `the_music_is_polled_between_copper_slot_zero_and_the_first_band`
(a stub that logs its poll against a part that logs its bands and three logging copper slots —
`["copper:0", "music:poll", "frame:0:0", "frame:0:1", "copper:1", "copper:2"]`) and
`a_tick_polls_the_music_exactly_once` (one poll per tick however many bands the tick draws, which is
what makes a poll one frame of the player's own clock, §5).

### The six calls, as the object computes them

| call | what the object does | the port |
|---|---|---|
| `dis_muscode(code)` | `%ax` = `_np_zinfo` (`DIS/DISINT.ASM:249`), the parameter never read | `muscode_from` → **0**, D8's deferral |
| `dis_musplus()` | `%dx` — the block's own arithmetic (`:252-271`) | `musplus_from(zplus, row)` |
| `dis_musrow(code)` | `%bx` = `_np_row`, reloaded on the way out (`:272`) | the published `row`; the trait takes no argument, because the wrapper reads none (`DIS/DISC.ASM:114-123`) |
| `dis_getmframe()` | `_np_zframe` (`:419`) | `Player::zframe()` |
| `dis_setmframe(frame)` | writes `_np_zframe` (`:418`) | `Player::set_zframe(frame)` |
| `dis_sync()` | the `ordersync1` lookup (`:449-463`) | `sync_from(order, row)` — with the boundary below |

The three that share a service are read back by three wrappers in `DIS/DISC.ASM:91-123`, one per
register: `_dis_muscode` returns `%ax`, `_dis_musplus` returns `%dx` (`:111`), `_dis_musrow`
returns `%bx`. All three read the **published** words — the ones the ten-deep ring hands out
(§8) — and so must the port: `musplus_from` and `sync_from` take `Player::zplus()` and
`Player::state()`, never the engine's own row ten polls ahead.

**`musplus`'s rule** is the block's two compares and the row it dispatches on, and the port
transliterates it (`DIS/DISINT.ASM:252-271`): `%dx` starts at the sentinel `-32` (`:252`); a
`zplus` of 0 keeps it; 1 takes the "plus coming" branch (`@@1b`), `row - 64` clamped up at `-32`
by a **signed** `jge` (`:270`); 2 takes the plain-row branch (`@@1e`), the row itself while it is
below 32 and the sentinel from 32 on; and any other value — 3, the only one a two-bit `zplus` can
otherwise hold (§6) — dispatches on `row > 32` with an **unsigned** `ja` (`:260`), sending rows
above 32 to the row-64 form and the rest through `@@1e`, so row 32 exactly reads the sentinel and
row 33 reads `-31`. `musplus_is_the_objects_own_two_phase_rule` writes every boundary out as a
table (four `zplus` values × the rows where the answer changes, 0/31/32/33/63/64/95/96/255), and
`musplus_over_every_row_matches_the_objects_decision_tree` checks all 1,024 pairs against the same
rule written as row ranges. The demo's own parts read the answer as a countdown to the marker:
`GLENZ/MAIN.C:290` waits for it to reach `-19`, `TECHNO/KOE.C:213` and `:275` for `-4` and `-3`,
`PLZPART/PLZ.C:68` for 0, `PLZPART/VECT.C:116` for 13, `JPLOGO/JP.C:71` for 4, and
`GLENZ/MAIN.C:411` breaks when it lands between `-16` and 0.

**`muscode` is 0, and the substitute keeps `_np_zinfo`'s shape.** D8's route 1 walked the object's
symbols and read the word live: it is at segment offset `0x08F7` and it is **0 in every sample of
every filed trace** (38,803 across the eleven — Phase 3's four, Phase 4's four and Phase 5's
three — covering MUSIC0's orders 0…9, 12, 13, 25…31 and 70…77 and MUSIC1's 0 and 18…98, the
`254` entries stepped over), so the port returns 0
(`music-clock.md` §6). `muscode_from` takes the
position and the wanted code and ignores both, so the phase that finds a mechanism for its other
values replaces one body; the parts that wait on it — `GRID/MAIN.C:72` for 2, `:160` for 3,
`HARD/B.C:138` for 1, `HARD/BP.C:263` for 0xff — therefore wait for ever here exactly as they do
against the reference's own 0, and they are Phase 8's to write against a cue this phase measured
rather than inferred.

### `sync`: the table, the read before it, and the read past it

`ordersync1` (`DIS/DISINT.ASM:428-446`, fifteen entries, keys `(order << 8) | row`, codes 0…10 and
then 1…4) is carried in `crates/sr-engine/src/music.rs` verbatim, and
`the_table_is_the_shipped_shells_own_bytes` finds those sixty bytes exactly once in each of the
three shell builds the tree holds — `MAIN/U2.EXE`, `MAIN/2NDFIX.EXE`, `U2.EXE` (the same bytes are
in `MAIN/U2.OBJ`, where they are object records rather than an image). And the table is itself a
build artefact: `MAIN/PROT/TEST.OLD`, the protection test build the shell's Makefile stages
(`MAIN/MAKEFILE:39`, `:43-44`), carries **`0x030b`** — order 3, row 11 — where those three carry
`0x0300`, which `an_older_build_carries_a_different_table` pins. What the port follows is
DISINT.ASM's table as the shipping build assembled it.

Two things about the loop around it (`:454-460`) decide the port's behaviour, and neither is
visible in the table:

1. **The answer is read at `%bx - 2`** (`:460`) — the code of the entry *before* the one whose key
   matched, not of the matching entry. So the pairs are upper bounds: the position `(2, 0)`
   (key `0x0200`) reads **0**, and `(2, 1)` reads **1**; every key's own row still answers with the
   previous entry's code. The port keeps that: `sync_reads_the_table_the_object_reads` shows each
   of six keys twice, at the key and one row later.
2. **The loop runs sixteen times over fifteen entries** (`:455`'s `mov cx,16`), so a position
   above the last key (`0x4200`) makes the 16th iteration compare against the bytes *past* the
   table, which are `sync_10`'s own first instruction — `mov es, cs:stmikseg`, `2e 8e 06 …`. That
   gives the region **two tiers**, and they are different in kind:
   - `0x4200 < key <= 0x8E2E`: the compare *matches* (`2e 8e` read as a word; the same two bytes
     in every build, because they are the instruction's prefix and opcode), and the answer read at
     `%bx - 2` is the table's own last code, **4** — deterministic here in every build the tree
     holds;
   - `key > 0x8E2E`: the compare fails and `%bx` advances one entry further, so the loop's
     fall-through returns **the word after the one it just compared** — not the two bytes past
     the table, which are the compare itself, but the two *after* those: the modrm byte `06` and
     the *low byte* of the `mov`'s displacement, measured **`0xCD06`** in `MAIN/U2.EXE`,
     **`0xED06`** in `MAIN/2NDFIX.EXE` and **`0x9D06`** in `U2.EXE`, i.e. one byte of instruction
     framing and a **linker-set** address. That is the half a port cannot have. (The distinction
     matters because the two words are three bytes apart in the same instruction: `2e 8e 06 cd 16
     …` is `mov es, cs:stmikseg`'s own encoding, and the tier above this one answers with the
     *table's* last code because that compare *matched*.)

**What the port does at each end, and why.**

- **Before the table it answers.** At `(0, 0)` the `%bx - 2` read is the word before `ordersync1`,
  which is `pop es; ret` — `07 c3` — because `ordersync1` follows `musframe_9`, whose INDEMO body
  ends `pop es` (`:420`) then `ret` (`:424`). That is not linker luck but the source's own
  instruction sequence, and the same two bytes sit there in all three builds the tree holds
  (pinned by `the_table_is_the_shipped_shells_own_bytes`), so `sync_from(0, 0)` returns it — as
  the C callers receive it, a 16-bit `int`, i.e. **-15609** (the parts' C is Turbo C —
  `PAM/PAM.PRJ` names `Turbo C Project File`, includes from `G:\BC\INCLUDE\` — whose `int` is
  16 bits). `sync_answers_the_position_before_the_table_with_its_own_word` pins the value, and
  the position is **reached**: in `second5` every sample from frame **148** to **426** reads
  `ord 0 row 0 pat 0` — 278 of the trace's samples, its coverage dropping frame 409 — the first
  published row is frame **427**, and the music's own start is frame 408 (`cargo xtask music
  analyse --trace reference/music/second5`: §2's row anchor, "that row began at frame 408.0";
  §1's "first advancing frame: 410" is the frame that tick is *seen* on. The values are the
  trace's own `frame,ord,row,pat` columns, which `crates/sr-audio/tests/trace.rs` drives a
  `Player` over frame by frame.) What that buys is a number a caller lives with:
  every `dis_sync() < N` in the tree (`PAM/OUTTAA.C:39`'s `< 10`, `ALKU/MAIN.C:59`'s `< 1`, and
  the rest) keeps waiting, exactly as it does against the reference. Note which trace this is and
  which it is not: `second5` is a `SECOND 5` session, and ALKU does not run in it (`MAIN/U2.ASM:792`
  guards `exe1` — `:62`'s `'ALKU.EXE'` — on `whattorun & 1`, which block 5's mask does not set).
  ALKU's *first* `dis_sync()` is at this position by its own source: `ALKU/MAIN.C:59` **is** the
  `while(dis_sync() < 1)` wait, and ALKU makes that call before the code has reached 1 — which is
  what `part-sequence.md` §1's running ALKU first in block 1 means here. The trace is what shows
  the position itself is one a run holds, not what shows ALKU holding it.
- **Above the table it answers the deterministic tier and refuses only the varying one.**
  `sync_from` returns the table's own last code, **4**, for `0x4200 < key <= 0x8E2E`: the
  16th compare is against the source's own first instruction (`2e 8e`), it *matches*, and the
  read at `%bx - 2` is the last entry's code field — the same 4 in every build the tree holds.
  Above `0x8E2E` the compare fails, the fall-through returns a word whose low byte is a
  link-time displacement, and `sync_from` **panics** with the position, the reason and the
  citations rather than answering a number that differs between builds.
  `sync_answers_the_band_above_the_tables_last_key_with_the_tables_last_code` pins the `4` at
  `0x4201`, `0x4300` and `0x8E2E`, and `sync_refuses_the_first_key_past_the_sixteenth_compare`
  pins the refusal at `0x8E2F`; `the_answerable_bands_upper_edge_is_the_word_past_the_table`
  ties the boundary to the image's own word. `APPROXIMATIONS.md` A15 registers the refusal.

  The band is not a theoretical one. The `hiddenu` trace — `SECOND U`, a filed trace of the
  shipped shell driving a module — holds `(order << 8) | row > 0x4200` for **5,148 of its
  6,351 frames (81.1 %)**, while `second5`, `second5b` and `second3` hold 0 of their 1,150 /
  1,152 / 2,755. A Phase-4 part waiting on `dis_sync()` while MUSIC1 or HIDDENU plays meets
  the answered band immediately, which is why the deterministic tier is modelled rather than
  refused.

The two callers in the tree are both block 1's and both reach the *answered* domain by a wide
margin: `PAM/OUTTAA.C:39` waits for `dis_sync() < 10`, which the table's order-13 key gives, and
`ALKU/MAIN.C:59`, `:66`, `:72`, `:79` and `:138` wait for 1 through 4, from order 2 on. What the
phase cannot show is that they never call it above `0x8E2E` — both parts are Phase 4's, and how
far MUSIC0's order list has got while they run is not measured. That is why A15 exists rather
than a claim that the refusal's region is unreachable.

### `zframe`, and what a part sees with no player

`dis_getmframe`/`dis_setmframe` are the two halves of `musframe_9` (`DIS/DISINT.ASM:412-425`): one
word, `_np_zframe`, which is the **tick** counter and not the frame counter (`music-clock.md` §8;
`second3` reads 1516 → 0 at frame 2452 when a part calls `dis_setmframe(0)`). The port's trait hands
it over as the player holds it, and `NoMusic` — the engine's own stub, and the type a test installs
when it wants a part handed *something* — answers the way a run with no player at all does: a zero
position, the `-32` sentinel, row 0, `_np_zinfo`'s 0, a frame counter that reads 0 and cannot be
written, and a `sync()` that refuses. It is deliberately **not** a degraded music: a part that
ignores music never calls it, and a part that waits on a cue is wrong where it can be seen rather
than handed a plausible number. A run with no music at all is `None` — `PartCtx::music()` returns
`None` — and the shell refuses to start a part whose cue is required (Task 7).

The seam's own tests: `music.rs` has **thirteen**, in the file's own order —
`musplus_is_the_objects_own_two_phase_rule`,
`musplus_over_every_row_matches_the_objects_decision_tree`,
`sync_reads_the_table_the_object_reads`,
`sync_answers_the_position_before_the_table_with_its_own_word`,
`sync_answers_the_band_above_the_tables_last_key_with_the_tables_last_code` (the `4` at
`0x4201`, `0x4300` and `0x8E2E`),
`sync_refuses_the_far_end_of_the_order_range` (**`should_panic`**, `(255, 255)`),
`sync_refuses_the_first_key_past_the_sixteenth_compare` (**`should_panic`**, `(0x8e, 0x2f)`),
`the_answerable_bands_upper_edge_is_the_word_past_the_table` (which pins the comparison the
refusal is chosen by),
`the_table_is_the_shipped_shells_own_bytes`,
`an_older_build_carries_a_different_table`,
`muscode_is_the_deferrals_substitute_over_every_code_the_parts_wait_for`,
`no_music_answers_as_a_run_with_no_player`,
`no_music_refuses_to_sync` (**`should_panic`**). And `engine.rs`'s: the four
poll-and-context tests above plus
`the_music_outlives_the_part_that_is_running` (a part boundary is not a music boundary: `start`
clears the copper slots and the frame counter, and leaves the player) and
`no_music_is_the_null_player_a_part_would_meet_without_one`.

## 12. The run's music — Task 7

The player above is a library: nothing played it in a run until this task. What a *run* does
is three things — which module it starts and at which order, when that music's own clock
starts, and where its samples go — and all three are the shell's
(`bins/secondreality/src/shell.rs`, `src/audio.rs`).

### Which module, at which order: the demo's own restart table

`SECOND n` selects a `partsmask` byte (`MAIN/U2.ASM:110-120`, `:595-630`) and the shell's
`restartmus` (`:348-397`) is what loads a module and seeks it: the call's `ax` is the module
(`0` = MUSIC0, `1` = MUSIC1), `bx` the order, and `cx` the speed adjustment it writes to
`_zspeedadjust2` *only* for output modes 7 and 2 (`:383-389`), which the reference's card is
not. The table, as `shell::MUSIC_RESTARTS` carries it, with the lines each row is read from:

| the block | module | order | the lines |
|---|---|---|---|
| `SECOND 1` (and no argument — `whattorun = 7fh`, `:108`) | MUSIC0 | 0 | `:766-775` (`mov ax,0` / `mov bx,-1`), `:785-787` (`push 0` / `call _zplaysong`) |
| every other block | MUSIC1 | 0 | `:772-775` (`mov ax,1` / `mov bx,0` / `mov cx,48700`) |
| `SECOND 3` | MUSIC1 | 42 | `:856-863`, guarded `whattorun & 4` and not `& 2` |
| `SECOND 4` | MUSIC0 | 18 | `:880-885`, guarded `& 8` — **and not entry-point-only** (below) |
| `SECOND U` | MUSIC0 | 70 | `:889-894`, guarded `& 80h` |
| `SECOND 5` | MUSIC0 | 25 | `:898-905`, guarded `& 16` and not `& 8` |

Two of the six are what happens when a bit is **not** set (`mov bx,-1` means "no seek here":
the shell calls `_zplaysong(0)` itself once the sound is on, `:785-787`).
`every_row_of_the_restart_table_is_the_originals_own` reads
`upstream/SecondReality/MAIN/U2.ASM` — **as bytes**, because the file is not UTF-8 and
a decoded read misplaces every line after the first non-ASCII byte — and checks each row's
instructions against its cited lines, in order, with its guard against the block's own
`partsmask` byte.

**Three of the four restart sites are entry-point-only, and `:880-885` is not.** Its guard is
`whattorun & 8` **alone** (`:880`, no second test), so it fires for every block whose mask has
bit 3 — `SECOND 1` (0x1f), `2` (0x1e), `3` (0x1c) and `4` (0x18). It is block 4's entry point
(its plan begins at U2E, `:886-887`, and the tier before it is skipped by `:856`'s `& 4` test);
for the other three the shell reaches it **mid-run**, between JPLOGO (`exe18`, `:877-878`) and
U2E, and restarts MUSIC0 at order 18 under a player that has been playing something else.
`SECOND 1` has a second mid-run restart (`:813-818`: MUSIC1 at order 0 between BEGLOGO and
GLENZ, `exe3`/`exe4`), whose guard is **two** bits rather than one — `whattorun & 2`, the
region `:801-802` opens and `:855` closes, **and** `whattorun & 1` (`:813-814`) — so it fires
for every block with bit 0, and every such mask carries bit 1 as well. The row for `SECOND 1`
above is that block's *start*, not this restart, and
`the_second_mid_run_restart_is_two_guards_and_the_masks_carry_both` is the test that reads
`:801-802`, `:813-818` and the region's close at `:855-855` — the label `:802`'s `jz` lands on, which
is what makes "the site is inside `& 2`'s region" an assertion rather than a reading of the line
numbers' order — and pins it (nothing else in the suite looks at this site).

**The port reaches the `:880-885` site today; it does not yet reach `:813-818`.** `--block 3`
runs MNTSCRL → LNS&ZOOM → PLZPART → MINVBALL → RAYSCRL → 3DSINFLD → JPLOGO → U2E and, before
U2E, fires `:880-885`'s mid-run restart: the shell installs a fresh MUSIC0 seeked to order 18,
which waits four engine ticks (D34; `MUSIC_RESTART_AT`, `MusicStart::restart`), and the block-3
chain test in `tests/shell.rs` asserts the boundary, the first published row and the positions
against `second3e`. `--block 4` takes the same call as its entry-point install. The
one-row-per-run model now has that restart seam; `--block 5`'s mask has bit 3 clear, so `:880`
is skipped for it. `--block 1` runs ALKU → U2A → PAM → BEGLOGO and stops at GLENZ, the first
part the port lacks, so it does not reach `:813-818` (MUSIC1@0, which sits between `exe3` and
`exe4`); `--block 2` stops at GLENZ before both sites. The table's rows and D27 remain the
sites' record, and
`one_row_of_the_table_is_not_entry_point_only_and_the_table_says_which` pins it, site by site:
`:880-885`'s guard has no `unless` bit and all four of those masks carry bit 3; `:856-863`'s and
`:898-905`'s `unless` bits are asserted (and are what keep them entry-point-only); and
`:889-894` — the fourth site — has **one** gating test as well, with **the masks** keeping it
entry-point-only (only `SECOND U` carries bit 7), which that test asserts rather than assumes.

**A run takes the row of the block whose plan begins where it does**, which for the port is
the run's first part: `--block 5` and `--part endlogo` both begin at ENDLOGO and both get
`SECOND 5`'s row (`music_for_part`). A part no entry point starts at is not played from block
1's row — that sentence was in this note and in `music_for_part`'s own doc until the final
review showed it false for TUNNELI. What the demo's `whattorun` guards put in force is:

| part | the sequences that reach it | module and order at its start | how the value is known |
|---|---|---|---|
| TUNNELI | blocks 1 and 2 | **MUSIC1, 0** | the module is the restart's: block 1 takes `:813-818`'s `mov ax,1` / `mov bx,0` (its bit 0 is set) and block 2, whose bit 0 is clear, took `:772-775`'s other initial branch — also MUSIC1 at 0. **The order in force when TUNNELI begins is not established**: GLENZ has played since the restart and no block-1/2 music trace exists, so the restart's own order (0) is the value the evidence names |
| CRED | **every block** — every `partsmask` carries bit 4 (`0x1f`, `0x1e`, `0x1c`, `0x18`, `0x10`) | **MUSIC0, 26** | the module is MUSIC0 in every sequence: block 5 (bit 3 clear) takes the entry restart at `:898-905` (order 25), and blocks 1–4 (bit 3 set) take the mid-run `:880-885` restart at order 18 and skip `:898-905`. The block-5 traces `second5`/`second5b` put the published music at **26:56 at reference index 1212**, CRED's tick 0 — measured |
| ENDSCRL | **every block**, the same two restart sites | **MUSIC0, 43** | blocks 1–4 reach it under `:880-885`'s MUSIC0@18 and block 5 under `:898-905`'s MUSIC0@25 — the sequence the filed traces cover. Those traces end at frame 1299, ~7,600 frames before ENDSCRL begins (~8917), so this is **derived, not measured**: advancing MUSIC0 from the block-5 restart (order 25 at frame 408) with the port's own player puts the engine at order 43 across the reference's whole ENDSCRL-start window (8841..8938) |

`music_for_part`'s doc in `shell.rs` carries the same table and the same two caveats; its test
asserts a row for every name `--part` accepts. The test card, which is the port's own part and
not the demo's, has no music at all: `PartCtx::music()` is `None` and a `--audio-dump` of such
a run is refused.

### When the music's clock starts: the anchor, and what a run does about it

`MUSIC_START_FRAME` is **408** — `music-clock.md` §2's measurement, stable to a tenth of a
frame across four sessions. It is where the player's tick clock starts, and the traces show
what surrounds it: `second5` reads `ord 0 row 0 pat 0` from its first sample (frame 148) to
frame **426**, `_np_zframe` 0 until 408, and the first published row is **427**. The words
around it also say *why* it is the same frame in every restart recipe: the modules' own
speed/tempo words change at 396 (MUSIC1 loaded by the initial `restartmus` at `:775`) and
again at 408 (MUSIC0, patched, by the block's restart at `:905`) — with **zero ticks in
between**, which is
the twelve frames `_zsoundon` (`:779`) and `_asmcopperinit` (`:781`) take before
`_zpollmix = 1` (`:783`) puts the player under the copper's per-frame `zpollme`
(`MAIN/COPPER.ASM:466`). The tick clock starts when the polling does, and the polling starts
there in every restart recipe. **`SECOND 1` is the exception Phase 4 filed**: block 1 never
takes the block restart at `:905` (its row loads with `bx = -1`, no seek), so its initial
start is its only one and it measures **399.6**, 8.4 frames earlier (`music-clock.md` §2,
`reference/music/second1`). (The readings *before* 396 — speed 6, tempo 125, from frame 148 on —
are the segment's own initialisers and not a loaded module, `music-clock.md` §2: nothing is
loaded at all until 396, and 6/125 is what the object's statics hold.)

A run reaches that clock in one of two ways, and the shell computes both from its own anchor
(`MusicStart`):

- **At or before 408**, the run *waits*: the module is loaded and seeked at the run's start,
  the engine's adapter is installed so that a part may ask it anything, and nothing advances
  it until the frame the original's own first tick falls on. Block 2 and block 5 both draw
  from 404, four frames early. This is what the trace holds at 404…407 — a seek publishes
  nothing, so the position is the ring's priming — and it is why the port's first published
  row change is frame **426** against the trace's 427.
- **After 408**, the run *pre-polls*: the frames between 408 and the run's first drawn frame
  are polled at the run's start, because the music's advance is deterministic and does not
  depend on a frame being drawn. `SECOND 3` is the case — MNTSCRL's first graphics frame is in
  (415, 420] while the music starts at 408 — and no run the port can make today is one:
  `MusicStart::pre` is `anchor - 408` and exists only for an anchor *after* 408, while
  `shell::boot_anchor` answers `Some(404)` (which waits) or `None` (`pre` 0). **The largest
  catch-up any run can have today is zero**, and the `3` elsewhere in this note is A12's
  *placement* tolerance, a different number. (If one ever runs, a catch-up longer than the
  ring's 8192 samples would have its oldest samples dropped rather than saved — the ring's
  overflow policy, the same one an undrained device meets — so a pre-polled run with
  `--audio-dump` would begin slightly clipped.)
- A run whose numbering is **its own** has no reference frame to synchronise to, so its music
  starts with it, on its frame 0. `--part beglogo` and `--part endlogo` are *not* that case —
  both parts carry measured anchors (`shell::boot_anchor`), so those runs wait for 408 exactly
  as the blocks they stand in for do — but `--part tunneli`, `--part cred` and
  `--part endscrl` are: a `--part cred` run installs **MUSIC0 at order 26** (the order the
  block-5 traces put in force at CRED's tick 0, `music_for_part`) and reads its first published
  rows from there.

**The evidence, and its two halves.** `shell.rs`'s
`the_runs_music_is_where_the_trace_holds_it_at_every_frame_the_trace_has` drives a `--block 5`
run in process — real player, stub parts, the real plan and anchor — over every frame of
`reference/music/second5` from 404 on, and compares the published position two ways:
**placement** (every position the port publishes is one the trace publishes at that frame or
within the 3 frames A12 measures between two sessions of one recipe) and **content** (the
sequence of distinct positions is the trace's, in order). It prints:

```
second5: 778 of 894 frames agree exactly (87.025 %); worst displacement 2 frame(s)
```

— against the same trace driven directly through a `Player` in
`crates/sr-audio/tests/trace.rs` (769 of 883, 87.089 %, worst 2).

The frames it walks are the trace's **own `trace.csv`** rows, not the manifest's coverage
list — a deviation from the brief's wording, and the same set of frames read the other way
round: the gaps are missing *rows*, so iterating the rows cannot include one. What holds the
two readings together is `crates/sr-audio/tests/trace.rs`'s `check_coverage`, which recomputes
the coverage from the CSV and asserts it is the manifest's window minus its gaps, field by
field, for every filed trace. A four-frame error in the
anchor fails both assertions: with `MUSIC_START_FRAME` set to 404 the same test reports *the
run published [(1298, (28, 5)), (1299, (28, 5))], which the trace publishes nowhere in frames
404..=1299 — its music is ahead of the trace's*, because the run has played a row the trace's
window does not reach. And four of ENDLOGO's frames are **exact** through the real binary:
`a_block_five_run_plays_music_one_at_the_order_its_entry_point_names` dumps 404, 430, 500, 900
and 1180 and finds `0:0`, `25:1`, `25:9`, `25:55`, `26:53` — the trace's own `(order, row)` at
each.

**What the port does not reproduce.** The boot's own twelve-frame transient — MUSIC1 loaded at
396 and never ticked, then replaced at 408 — is not modelled: a run installs its entry point's
module once, at its start. The *position* words are identical either way (both are the ring's
priming, `0:0`), and the speed/tempo words at 404…407 differ (the port reads MUSIC0's patched
6/120 where the trace still reads MUSIC1's 3/130), which is the one thing about this seam the
phase does not carry. **What makes that invisible is which words a part can ask for, not which
frame a part starts on** (ENDLOGO draws 404…407 itself): the six `dis_*` entries the demo's
parts call (`DIS/DIS.H:37-59`) hand back a sync code (`dis_muscode`, `dis_musplus`,
`dis_sync`), the published row (`dis_musrow`) and the tick counter (`_dis_getmframe`,
`_dis_setmframe`) — **none of them returns `_np_speed` or `_np_tempo`**, and neither does any
`DIS` wrapper in the tree. `MusicPosition::speed` and `::tempo` are the port's own, for the
status line and the manifest, which the shell reads **and no part reads today** — the method is
on the part-facing `Music` trait, so a part *can* read the two words; what the demo's parts do
is ask for the six `dis_*` answers, and no ported part calls the music at all yet (ENDLOGO's
gate is Task 8's, and it waits on `dis_musplus`). So the difference is visible to a reader of
the status line and of `Player::state`, and to a part only if one ever asks for the port's own
extension.

### Where the samples go: the device, the dump, and the status line

The **adapter** (`impl Music for PlayerMusic`, `bins/secondreality/src/audio.rs`) is the
binary's, for the reason §11 gives: neither crate can name the other's types. It is a forwarder
over the eight methods — the three DIS translations are `sr-engine`'s — and its only state is
the wait above. The player itself is behind an `Arc<Mutex<Player>>` because the *tick* polls it
and the sound card's callback *drains* it, and neither does the other's job.

The **device** is SDL2's, in window mode only, and it is a drain and nothing else: the callback
copies what `Player::take_samples` gives up and **fills the rest of its buffer with zeroes**.
That is the underrun policy, stated as behaviour and unit-tested against the callback
(`an_empty_ring_under_runs_into_silence`, `the_device_drains_at_the_mixers_own_rate_and_never_resamples`):
silence, never a stall, never a repeat, never a resample — the music's position is the *tick's*,
and a device that asked for more would be pulling the picture's clock into the card's. The
device is opened at the mixer's own specification with SDL's `allowed_changes` at zero, so a
card that cannot give that rate is not resampled into one that can: it is not opened at all,
and the window prints `no audio device (…); the run plays silent` and runs on. `--no-audio` is
the same state asked for rather than discovered.

`--audio-dump <FILE>` writes the tick's samples as a 16-bit signed PCM WAV instead of opening a
card — the ring has one consumer, so a dump is silent to watch — at the mixer's own rate and
channel count: **20000 frames a second stereo**, because `MixRate::Hz40000` counts the 40000
*i16 samples* a second the mixer produces and two channels make a frame (§10.2's "the frame
rate is 20000 a second in stereo", which is also the rate the reference's 5.26 %-flat stereo
divides against). The header's two size fields are patched on every write, so an interrupted
run leaves a readable file rather than one whose header claims nothing. The dump is the phase's
artifact for the human listen (`bins/secondreality/tests/audio.rs` checks its header, its
byte count as one poll a tick from 408 on, and that it is not silent); there is still no
reference audio to compare it against (§10.5).

The **status line** — the window's title — gains the music position as `order:row`, the
*published* one a part reads, ten polls behind the player's own (§8). It is what a person
debugging a sync problem needs and what Task 8's human check is about. A run with no player
says nothing about music rather than `0:0`.

The **fade** is `audio::fade_music`, one pass of `MAIN/U2.ASM:333-346`'s loop body: the
module's own global-volume byte (`ds:[30h]`) less `step`, saturating at zero. The source's
loop is a step a *frame* — its `int 0fch` is `dis_waitb()` — so the shell calls it once per
tick with the step the demo passes (`mov cx,1` at `:927` and `:938`), from the moment the run
has nothing left to play. The demo's other call site (`:927`) is **not wired**, and it is not
an oversight: it sits in the looping path the menu can select (`:919-933`), where the shell
fades the music, restarts it at order 0 and jumps back to `@@partsagain`. That path is reached
only when `looping` is **1**: `:922-923` sends 0 to `@@partsdone` and `:924-925` sends **2**
there as well, and 2 is what `/gravis` sets (`MAIN/START.C:89-92`, `m_looping=2`; D13's own
note) — so `/gravis` is precisely the mode that does *not* reach `:927`, and the menu's looping
option is the two-valued one (`MENU.C:167-168`, `:230-238`). The port has neither mode and no
equivalent of that path, so `@@partsdone`'s call is the one that has a place here. That moment is the port's hold, which is `@@partsdone`
(`MAIN/U2.ASM:935-942`) in the shell's own terms: the plan has run out or the next part is one
the port has not written. The fade is 64 frames at both modules' own `0x40`, and a run that
holds at an unported part fades too — the original would be playing that part — which is the
port's hold being what marks the end of the run rather than a claim about the original.

### The tests this task adds

| what | where |
|---|---|
| the restart table against the shell's own source, line by line, and its guards against `partsmask` | `src/shell.rs`'s `every_row_of_the_restart_table_is_the_originals_own` |
| the entry point's row is the one a run's plan finds | `src/shell.rs`'s `a_blocks_first_part_is_the_entry_point_its_row_is_about` |
| **`SECOND 4`'s row is not entry-point-only**, and which rows are | `src/shell.rs`'s `one_row_of_the_table_is_not_entry_point_only_and_the_table_says_which` |
| **the second mid-run restart** (`:813-818`), its two-bit guard, the region that guard encloses (`:801-802` open, `:855` close), and the masks that carry both | `src/shell.rs`'s `the_second_mid_run_restart_is_two_guards_and_the_masks_carry_both` |
| the music's start: the wait, the catch-up, and a run with its own numbering | `src/shell.rs`'s `the_music_waits_for_its_anchor_and_catches_up_past_it` |
| **the published position against every frame of `reference/music/second5`** | `src/shell.rs`'s `the_runs_music_is_where_the_trace_holds_it_at_every_frame_the_trace_has` |
| the adapter: the published words it hands over, and the wait | `src/audio.rs`'s `the_adapter_hands_the_engine_the_players_published_words`, `the_adapter_waits_out_the_frames_before_the_music_starts` |
| the WAV's header, its patching on every write, and a write that fails | `src/audio.rs`'s `the_dump_is_a_wav_at_the_mixers_own_rate_and_width`, `the_header_is_true_at_every_tick_not_only_at_the_end`, `a_write_that_cannot_be_made_is_kept_rather_than_lost` |
| **the underrun policy: silence, no stall, no resample** | `src/audio.rs`'s `an_empty_ring_under_runs_into_silence`, `the_device_drains_at_the_mixers_own_rate_and_never_resamples` |
| the fade walks the module's byte to zero and stops (`fademusic`'s own arithmetic) | `src/audio.rs`'s `the_fade_walks_the_modules_own_byte_down_to_zero_and_stops` |
| **and it is wired to the hold**: a step a tick from the moment there is nothing left to play | `src/shell.rs`'s `a_run_with_nothing_left_to_play_fades_the_music_out_a_step_a_tick` |
| **the four keyframes' positions, through the real binary** | `tests/shell.rs`'s `a_block_five_run_plays_music_one_at_the_order_its_entry_point_names` |
| a run with no music says so in its manifest | `tests/shell.rs`'s `a_run_whose_entry_point_has_no_music_says_so_in_its_manifest` |
| **`--headless` with and without `--no-audio`: the same frames, byte for byte** | `tests/audio.rs`'s `a_headless_run_plays_the_music_with_and_without_a_device` |
| the dump's header, its sample count, and that it is not silent | `tests/audio.rs`'s `the_audio_dump_is_the_ticks_samples_at_the_mixers_own_rate` |
| **a machine with no audio device prints a line and keeps running** | `tests/audio.rs`'s `a_machine_with_no_audio_device_says_so_and_keeps_running` (`SDL_AUDIODRIVER` names a driver that is not there) |
| a run with no music refuses to dump, and a dump that cannot be written fails the run | `tests/audio.rs`'s `a_run_with_no_music_refuses_to_dump_audio`, `a_dump_that_cannot_be_written_fails_the_run` |
| the status line shows `order:row`, and nothing for a run with no player | `src/window.rs`'s `the_status_line_shows_where_the_music_is` |
| the seek clears the tick-in-row counter (§8's decision) | `crates/sr-audio/tests/player.rs`'s `a_seek_mid_song_clears_the_tick_counter_so_the_new_rows_cells_are_read` |
