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
even though it is inert: the original's sample-loading loop *overwrites* the header's `memseg` with
the segment it allocated, so its "is there a sample" test is about memory, not about the file. The
loader's `sample_bytes` is about the file — the bytes are there or they are not — and the mixer is
what will need the original's allocated-and-padded form.

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
  too. What the player does at them is Task 3's, decided by the reference's own order counter
  (`music-clock.md` §10) — no reading of ST3 semantics is used here.
- **Note 0xFE and 0xFF, instrument 0 and command 0.** Counted and located (§6), not interpreted.
- **The samples' in-memory form** — the clamps and the 512-byte pad of `checkins`
  (`STARTEMS.C:129-177`) — is Task 5's; the loader slices the file.
- **Whether the modules' `S` commands are all `SDx`** (spec §7.2's scope claim, and D8's `_np_zinfo`
  question) is a reading of `Cell::info` on those cells and belongs with the effects in Task 4.
