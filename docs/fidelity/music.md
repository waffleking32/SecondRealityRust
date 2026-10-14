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
`0x60 + ordNum + insNum*2`, in paragraph units, which is `STARTEMS.C:333-334` read literally.

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
`the_objects_own_bytes_say_what_the_player_says`, which rebuilds it and asserts 24 load-bearing
instructions at their own offsets (the count the test itself prints)). Offsets below are that object's segment offsets, and the
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

A poll's samples are fractional (`40000/70.086` = 570.733…), and `player.rs` carries the fraction
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

**Two things about those two `255` paths are worth stating because no trace can reach them** —
neither module has a `255` at its first order — and the port follows the bytes in both rather than
choosing:

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
  `player.rs`'s `play_song` does **not** mirror that today: nothing in the tree seeks mid-song
  (every test, and every filed trace, seeks on a fresh player, whose counter is already 0), so the
  difference is unobservable. It stops being unobservable the moment something restarts the music
  mid-run — the shell's `restartmus` (`MAIN/U2.ASM:348-397`, the restart path Task 7 owns) does
  exactly that — and a seek that leaves a *stale* counter begins the new row's first tick with the
  counter non-zero, so that row's cells are not processed at all. Whoever puts the shell's restart
  into the engine has to decide it there, against `+0x56ec`'s bytes.

**`music-clock.md` §10 says the trace cannot show what the player does at a `255`. This is that
question's answer, and it is a reading of the object rather than a measurement**: the note records
it as the port's behaviour, with the trace's silence stated beside it. The same goes for `B`
(`+0x3de2`: the advance flag goes up and the order to try next is overwritten, so the jump lands at
the row's *end*; `BFF` at `+0x3df1` ends the song) — no filed window reaches a `B`, and MUSIC0's
three are at orders 63, 66 and 77, past every trace. What the traces *do* exercise is `A` and `C`:
MUSIC0's pattern 70 row 0 sets speed 6, pattern 71 row 0 sets 1 and row 36 sets 6, and pattern 71
row 61's `C00` is the trace's ord 26 → ord 28. `C`'s parameter is decimal and is stored unclamped
(`+0x3dfe`: `mov %al,%ah; shr $0x4,%ah; and $0xf,%al; aad $0xa`).

### Two readings that are in the object but contribute nothing here

- `A` and `T` are one handler told apart by the *parameter*:

  ```
    3D73: 32 e4                 xor    %ah,%ah
    3D75: 3d 20 00              cmp    $0x20,%ax
    3D78: 76 54                 jbe    0x3dce           ; 32 and under: a speed
    3D7A: 50                    push   %ax
    3D7B: 2e a2 0f 09           mov    %al,%cs:0x90f    ; _np_tempo
    (…)
    3DD2: 3c 00                 cmp    $0x0,%al
    3DD4: 74 f8                 je     0x3dce
    3DD6: 32 e4                 xor    %ah,%ah
    3DD8: 2e a3 02 20           mov    %ax,%cs:0x2002   ; _np_speed
  ```

  Neither module carries a `T`
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
  same way, `_np_zframe` among them, and every filed trace reads `_np_zframe` as 0 before the
  music's first tick, so those slots do not survive into the running program. And the traces
  agree the adjuster contributes nothing here: the published row changes of `second5`'s speed-6
  run are **exactly** six ticks apart (`cargo xtask music analyse --trace reference/music/second5`
  prints 61 gaps in 534 frames where the rule says 534.4, residual −0.4).

### 254 and the pattern break, as the trace has them

```
reference/music/second5: frames 1251 → 1260  ord 26 row 61 → ord 28 row 0   (the C00, over the 254 at order 27)
reference/music/second5: frames 970 → 979    ord 25 row 63 → ord 26 row 0   (pattern 71's A01 at speed 1)
```

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
   wrong *value* — a port that published a loop count of its own would fail, while one that froze
   the counter at zero matches what the traces do.
2. **placement**: every published value is one the trace publishes at that frame or within
   3 frames of it;

— and *measures* the rest. `cargo test -p sr-audio --test trace -- --nocapture` prints:

```
4 filed trace(s)
  hiddenu (MUSIC0): 5687 of 6088 frames agree (93.413 %); worst displacement 3 frame(s)
  second3 (MUSIC1): 2144 of 2488 frames agree (86.174 %); worst displacement 2 frame(s)
  second5 (MUSIC0): 771 of 887 frames agree (86.922 %); worst displacement 2 frame(s)
  second5b (MUSIC0): 777 of 887 frames agree (87.599 %); worst displacement 2 frame(s)
```

The frames that disagree are the ±1-frame placement of a row boundary — one per row change —
because the port's clock is exact and the reference's is not; the port sits *between* the two
sessions (a frame early in `second5`, a frame late in `hiddenu`) which is the best a fixed clock
can do. `docs/fidelity/APPROXIMATIONS.md` records the measurement and its cause.

### The end-to-end play, shown

`cargo test -p sr-audio --test player -- --nocapture each_module_plays`:

```
  MUSIC0: wrapped after 8732 polls (124.6 seconds of logic ticks) at order 0 pattern 0 row 0, loop counter 1
  MUSIC0: 8732 poll(s), 5983 tick(s) over 998 row(s)
  MUSIC0: 80 order(s), 77 pattern(s), ended at order 0 (1 loops)
  MUSIC1: wrapped after 25769 polls (367.7 seconds of logic ticks) at order 0 pattern 73 row 0, loop counter 1
  MUSIC1: 25769 poll(s), 19123 tick(s) over 5223 row(s)
  MUSIC1: 100 order(s), 77 pattern(s), ended at order 0 (1 loops)
```

(the second line of each pair is the same run counted a second way — the engine's row visits
against its own tick counter, every row asserted to have had between 1 and 32 ticks.)

Each module is played from order 0 — the order the demo's own boot starts it at
(`MAIN/U2.ASM:786` pushes 0) — through its order list to the first `255`, where it loops back
(the shell's own restarts are what the demo hears; the module's own list ends where the `255`s
are). `playing_past_the_end_neither_panics_nor_runs_off_the_orders` then plays each module a
further 100,000 polls and asserts that every published order, pattern, row, speed and tempo stays
inside the module's own tables — a player that loops for ever is fine, one that reads past its
orders is not.
