# MUSIC0 and MUSIC1 — the container, the player and the mixer

The demo's two modules, `assets/MAIN/MUSIC0.S3M` (391,312 bytes) and `MUSIC1.S3M` (600,860
bytes), are read **in their shipped, scrambled form**: the descrambling is part of the port, and
nothing writes a decoded copy to disk. Both files are byte-identical to the copies embedded in
`REALITY.FC` (D5), and `PROVENANCE.md` carries their digests.

This note owns the player and the mixer **as read from the original**: the loader
(`MAIN/STARTEMS.C`), and `MAIN/STMIK.300`'s segment image, disassembled. What the reference's own
player state shows frame by frame is `music-clock.md`'s; where the object and the traces disagree,
the trace wins (D12). The preserved behaviours are `bugs.md` §28.

| layer | code | evidence |
|---|---|---|
| the container (§1–§6) | `crates/sr-audio/src/module.rs` | `crates/sr-audio/tests/module.rs` |
| the player (§8) | `crates/sr-audio/src/player.rs` | `tests/player.rs`, `tests/trace.rs` |
| the channel effects (§9) | `crates/sr-audio/src/effects.rs` | `tests/effects.rs` |
| the mixer (§10, §13) | `crates/sr-audio/src/mix.rs` | `tests/mix.rs`, `tests/enhanced.rs` |
| the engine's seam (§11) | `crates/sr-engine/src/music.rs` | its unit tests |
| the run's music (§12) | `bins/secondreality/src/shell.rs`, `src/audio.rs` | their unit tests, `tests/audio.rs` |

The container's numbers below are printed by

```
cargo test -p sr-audio --test module -- --nocapture --test-threads=1
cargo test -p sr-audio --lib
```

and quoted blocks are pastes from it, with cargo's `test <name> …` prefix stripped and `(…)` for
dropped lines.

## 1. The container, as the original's loader reads it

The loader is a translation of `MAIN/STARTEMS.C`'s `loads3mems`, the function that read these two
files in 1993, so the layout is that function's and not the generic S3M documentation's.

```
STARTEMS.C:50-68    struct ds_fileheader        0x60 bytes, #pragma pack(1)
STARTEMS.C:28-48    struct ds_smp               one 0x50-byte sample header per instrument
STARTEMS.C:333-334  parains = module + 0x60 + ordNum;  parapat = parains + insNum
STARTEMS.C:343-353  each pattern: read a word at ptr*16, then read ptr*16+2 for (word - 2) bytes
STARTEMS.C:367-393  each sample: only type 1; its bytes are at memseg<<4 | memseg2<<20, `length` of them
STARTEMS.C:129-177  checkins: the clamps (volume 64, c2spd 65535, length 64000) and the 512-byte pad
```

```
MUSIC0.S3M: 391312 bytes, signature "SCRM" at 44, marker 26 at 28, type 16 at 29, title "UnreaL ][ - The 2ND Reality\0"
MUSIC1.S3M: 600860 bytes, signature "SCRM" at 44, marker 26 at 28, type 16 at 29, title "UnreaL ][ / PM \0\0\0\0\0\0\0\0\0\0\0\0\0"
(…)
MUSIC0.S3M: 80 orders at 0x60, 44 instrument pointers at 176, 77 pattern pointers at 264..418, the first pattern block at byte 3952
(…)
MUSIC1.S3M: 100 orders at 0x60, 54 instrument pointers at 196, 77 pattern pointers at 304..458, the first pattern block at byte 4784
```

The order list is at 0x60, then `insNum` instrument pointers, then `patNum` pattern pointers. The
pattern table's offset is in **bytes**: `STARTEMS.C:293`'s `char far *module` makes `parains`
byte arithmetic, and `parapat = parains + insNum` adds `insNum × 2` bytes (`:333-334`). Each table
**entry** is in paragraphs: `parapat[a] << 4` (`:347`).

### The block's length word includes its own two bytes

A pattern block is `ln` bytes from `ptr * 16`, and the packed data is the `ln - 2` bytes from
`ptr * 16 + 2` (`STARTEMS.C:348-351`). The evidence is that the blocks tile:

```
MUSIC0.S3M: 77 patterns, 76 consecutive pairs; the next block begins 0..15 bytes after ptr*16 + ln, present sizes [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15], every gap {0: 4, 1: 4, 2: 3, 3: 4, 4: 2, 5: 5, 6: 7, 7: 7, 8: 3, 9: 6, 10: 8, 11: 1, 12: 6, 13: 2, 14: 5, 15: 9}
MUSIC0.S3M: reading `ln` as the data's length instead breaks 8 of those 76 pairs
(…)
MUSIC1.S3M: 77 patterns, 76 consecutive pairs; the next block begins 0..15 bytes after ptr*16 + ln, present sizes [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15], every gap {0: 3, 1: 8, 2: 4, 3: 3, 4: 4, 5: 5, 6: 6, 7: 2, 8: 1, 9: 7, 10: 2, 11: 4, 12: 6, 13: 3, 14: 9, 15: 9}
MUSIC1.S3M: reading `ln` as the data's length instead breaks 11 of those 76 pairs
```

The second line of each pair is the counter-reading, computed in the same loop: taking `ln` as the
data's own length leaves 8 pairs in MUSIC0 and 11 in MUSIC1 that overlap.

### The packed cell's framing

1. **A row ends on a zero byte**, `flag == 0`, not `flag & 0x1f == 0`: the low five bits are the
   channel, and the modules carry 1,990 channel-0 cells in MUSIC0 and 2,835 in MUSIC1, each of
   which the second rule would read as a row end.
2. **Bit 5 means a note and an instrument**, two bytes in that order.
3. **The channel is the low five bits, and each row names a channel at most once**:

```
MUSIC0.S3M: 4928 rows, 8 cells in the widest row, highest channel 7, 1990 cells on channel 0, no row names a channel twice
MUSIC1.S3M: 4928 rows, 13 cells in the widest row, highest channel 14, 2835 cells on channel 0, no row names a channel twice
```

The channels come in ascending order within every row, so a 32-slot row represents the stream
losslessly. The loader refuses a row that names one channel twice
(`a_row_that_names_one_channel_twice_is_rejected`); neither module has one.

## 2. The keystream, and what it verifies

D5's keystream:

```
n      = (i >> 1) + 1
key(i) = (((n ^ (n >> 2)) << 3) | ((5*i + 2) & 7)) & 0xFF
```

applied as `data[ptr + 2 + i] ^= key(i)` for `i` in `0 .. ln-2`, restarting at index 0 for every
pattern. `module.rs`'s `the_keystream_is_d5s` asserts its first sixteen bytes
(`[10, 15, 20, 17, 30, 27, 40, 45, 34, 39, 60, 57, 54, 51, 80, 85]`) against the formula, and
`descramble_restarts_at_index_zero_for_every_pattern` checks the restart and that applying it
twice is the identity.

```
MUSIC0.S3M: 77 of 77 patterns decode as exactly 64 rows consuming exactly their data; 14980 cells in all
MUSIC1.S3M: 77 of 77 patterns decode as exactly 64 rows consuming exactly their data; 17746 cells in all
```

The loader enforces "consuming exactly their data": 64 rows must end on the block's last byte.
D5's "76 of 77" for MUSIC1 is the one cell whose command byte is 0 (§6); read as *no command*, it
decodes. The test walks both modules a second time with its own keystream, pointer arithmetic and
cell reader (`Walk`) and compares cell by cell, so the counts are two readers agreeing.

## 3. The two headers

```
MUSIC0.S3M: ordNum 80 insNum 44 patNum 77, global volume 64, initial speed 6, initial tempo 125, master volume 176 (stereo true, 8 channels enabled), channel table [0, 8, 1, 9, 2, 10, 3, 11, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255]
MUSIC1.S3M: ordNum 100 insNum 54 patNum 77, global volume 64, initial speed 3, initial tempo 130, master volume 176 (stereo true, 8 channels enabled), channel table [0, 8, 1, 9, 2, 10, 3, 11, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255]
```

Global volume is at offset 48, initial speed 49, initial tempo 50, master volume 51, the channel
table 64..96 (`STARTEMS.C:63-67`). Master volume 0xB0 has bit 7 set, so both files are stereo, and
the loader keeps the byte whole. The channel table is eight channels in four interleaved pairs; the
mixer reads it (§10.3). MUSIC1's patterns reach channel 14 despite it, the off-channel material D8
records.

**The tempo patch is MUSIC0's alone.** `MAIN/STARTMUS.C:257` writes `module[50] = 0x78` inside
`case 0` (`:252-265`), MUSIC0's load (`loadmodule2(0)`, `:255`); `case 1` (`:259-261`) has none.
`Module::load` reports the file's 125, and `apply_tempo_patch` makes it 120 and moves nothing else
(`module::tests::the_tempo_patch_changes_the_initial_tempo_and_nothing_else`); `Song::load`
applies it to MUSIC0 only (`SONGS`).

## 4. The order list, as data

The order list is kept as its bytes: nothing is normalised, and `254` and `255` stay
(`the_order_list_is_preserved_byte_for_byte`).

```
MUSIC0.S3M: 80 orders; 255 at [16, 17, 64, 69, 78, 79]; 254 at [27]
MUSIC1.S3M: 100 orders; 255 at [99]; 254 at [3, 14, 19, 25, 27, 38, 41, 46, 49, 61, 67, 76, 87, 92, 97]
```

Every other entry is a pattern number inside the 77 both files carry.

**How the song starts.** The shell hands the list to the player through `_zplaysong(order)`
(`MAIN/STMIK.H:43-44`: "starts playing the module at order [order]"). `MAIN/U2.ASM` calls it at
two sites: the **initial start** at `:785-786` (`push 0`, `call _zplaysong`, just after
`mov es:_zpollmix,1` at `:783`, the 0 → 1 move `music-clock.md` §5 measures), and `restartmus`
(`:348-397`), which pushes the order its caller left in `bx` (`:393`) and calls at `:394`. Its
callers pass 18 (`:883`), 70 (`:892`) and 25 (`:903`). The tree's other two copies of the source,
`MAIN/U2.BBK` and `MAIN/U2ASM.BBK`, carry the same two calls. The shell's restarts land past every
`255`, which is why no trace plays one (`music-clock.md` §10); what the player does at one is §8's
reading of the object.

## 5. The instruments and their samples

```
MUSIC0.S3M: 44 instruments, type census {0: 22, 1: 22}; 12 of them loop; 329186 sample bytes in all; 0 PCM headers with a zero paragraph pointer
MUSIC1.S3M: 54 instruments, type census {0: 13, 1: 41}; 9 of them loop; 520396 sample bytes in all; 0 PCM headers with a zero paragraph pointer
```

The type byte is `struct ds_smp`'s `type` (`STARTEMS.C:29`): **0 = empty and 1 = PCM, and no
third type occurs.** A type-2..7 instrument would take the adlib path (`:175-176`); neither module
has one, so the port models no OPL (`the_instrument_census_is_the_files_own`). No PCM header has a
zero paragraph pointer, so the loader's slicing condition (`type == 1 && length != 0`) agrees with
the original's (`STARTEMS.C:367`, `type == 1 && memseg`) on both files. The original's guard reads
`memseg` as the file carries it (`:338-339`); its overwrite with the allocated segment comes later,
inside the loop (`:389`).

An empty instrument still has a header with values in it:

```
MUSIC0's 22 empty instruments: instrument 19 carries length 0 memseg 0x0000 volume 64 flags 0x00 c2spd 8363
```

### The sample bytes

`STARTEMS.C:385-388` puts a sample at `memseg << 4 | memseg2 << 20` in the file and reads `length`
bytes there:

```
MUSIC0.S3M: 22 samples sliced, the first at byte 61968 and the last ending at byte 391312 of 391312
MUSIC1.S3M: 41 samples sliced, the first at byte 80192 and the last ending at byte 600860 of 600860
(…)
MUSIC0.S3M: the patterns end at byte 61954, the first sample begins at 61968, and the 14 bytes between them are [80, 80, 80, 80, 80, 80, 80, 80, 80, 80, 80, 80, 80, 80]
MUSIC1.S3M: the patterns end at byte 80178, the first sample begins at 80192, and the 14 bytes between them are [80, 80, 80, 80, 80, 80, 80, 80, 80, 80, 80, 80, 80, 80]
```

The last sample ends on the file's last byte, and the 14 bytes between the last pattern block and
the first sample are `0x80`, PCM silence: header, tables, blocks, filler, samples, end of file.
`checkins`'s clamps and its 512-byte pad behind every sample (`STARTEMS.C:129-177`) are the
original's in-memory form, and §10.2's end rule reads them; the loader slices the file's bytes.

### `SCRS`, and a sample header field by field

`SCRS` is `ds_smp._magic` (`STARTEMS.C:47`), at `+0x4C` of every PCM header (22 in MUSIC0, 41 in
MUSIC1) and of no empty one; `checkins` writes it at load time (`:134`), so the files carry it
because a copy went through a load. The loader requires `SCRM` (`:62`, offset 44) and not `SCRS`.

```
MUSIC0 instrument 0: type 1 filename "HISTRING\0\0\0\0" memseg 0x0f21:0 length 30208 loop 5720..30014 volume 64 disk 0 pack 0 flags 0x01 c2spd 27700 name "HiStrings.HiQual(Marcato)\0\0\0"
```

The offsets: type `+0x00`, filename `+0x01`, `memseg2 +0x0D`, `memseg +0x0E`, length `+0x10`, loop
begin `+0x14`, loop end `+0x18`, volume `+0x1C`, `disk +0x1D`, pack `+0x1E`, **flags `+0x1F`**,
**`c2spd` `+0x20`**, `name +0x30`. A reader four bytes early gets `c2spd` 16,777,280; the test
asserts the true values, and `module.rs`'s synthetic-header test puts a distinct byte in every
field.

## 6. The cells, and what the player has to play

An effect the demo never plays is *measured absent*. Over the decoded cells of all 77 patterns:

```
MUSIC0.S3M: 157696 channel slots, 146161 of them carrying no command byte; the rest: A:63, B:3, C:8, D:7348, E:827, F:1051, G:802, H:212, J:137, K:105, O:755, Q:84, S:140
MUSIC0.S3M: a command byte of 0 on []; of 255 on 0; invalid commands: 0
MUSIC1.S3M: 157696 channel slots, 148363 of them carrying no command byte; the rest: 0(none):1, A:54, C:4, D:3340, E:217, F:180, G:2607, H:365, K:62, O:2417, Q:38, S:48
MUSIC1.S3M: a command byte of 0 on [(13, 19, 5)]; of 255 on 0; invalid commands: 0
```

157,696 slots is 77 × 64 × 32. The letters are exactly **A, B, C, D, E, F, G, H, J, K, O, Q, S**:
thirteen in MUSIC0, eleven in MUSIC1 (no B, no J). No cell carries a digit command or 255. **One
cell carries a command byte of 0**, MUSIC1 pattern 13 row 19 channel 5, D5's exception:

```
MUSIC1 pattern 13 row 19 channel 5: present 0x80 (a command field follows), command 0, info 64
```

Its flag byte was 0x85: the file stored a two-byte command field whose command is 0. The loader
keeps the cell.

### Notes

```
MUSIC0.S3M: 5186 notes 0x10..0x64, 71 note-offs (0xFE), 0 no-notes (0xFF); the format's ceiling is 0x9B
MUSIC1.S3M: 11189 notes 0x26..0x80, 35 note-offs (0xFE), 610 no-notes (0xFF); the format's ceiling is 0x9B
```

MUSIC1 plays up to **0x80**, so a decoder capped at 96 rejects it (D5's trap); §9.2 is what the
player does with it.

### The instrument byte is 1-based, and a volume of zero is a cell

- **`Cell::instrument` is the file's own byte.** MUSIC1's highest instrument byte is 54, its
  `insNum`, and its note-offs carry 0: 0 means "no instrument change" and 1..54 name the headers.
- **A cell whose only content is `volume = 0` is a cell, not an absent one.**

```
MUSIC0.S3M: highest instrument byte 29 of 44 instruments; 74 cells name instrument 0; 3 cells carry nothing but a volume of 0 (3 of which have all five fields zero)
MUSIC1.S3M: highest instrument byte 54 of 54 instruments; 35 cells name instrument 0; 189 cells carry nothing but a volume of 0 (189 of which have all five fields zero)
```

  By its five fields such a cell is indistinguishable from a slot the row never named, and a
  player that mistook it would leave the volume where it was. So `Cell` carries `present`, the
  flag byte's three "what follows" bits, and `Cell::EMPTY` means "the row never named this
  channel" (`every_pattern_decodes_to_exactly_sixty_four_rows`).

## 7. Retired: the container's open questions (§8–§10 answer them)

## 8. The player

The original's player has no assembly source in the tree: `MAIN/STMIK.300` is an OMF object, and
everything below is read out of its segment image, the 29,607 bytes of `text_stmik300_text`
rebuilt from its 105 `LEDATA` and 118 `LIDATA` records by `crates/sr-audio/tests/player.rs`'s
`segment_image`; `the_ports_tables_are_the_objects_own` holds the port's tables to it. Offsets are segment offsets, the listings are that image through
`objdump -b binary -m i8086 -D`, `(…)` marks an elision, and the `;` comments are this note's.
Which *frame* a row change lands on is the traces' (`music-clock.md`).

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

A tick is that many samples (833 at tempo 120 and 40000, 769 at 130), computed by the tempo setter
and nowhere else. A row ends when a counter of ticks reaches the **published** speed:

```
  4397: ff 06 00 20           incw   0x2000
  439B: 2e a1 02 20           mov    %cs:0x2002,%ax   ; _np_speed
  (…)
  43AC: 39 06 00 20           cmp    %ax,0x2000
  43B0: 7d 01                 jge    0x43b3            ; → the row advance
```

So an `A` lands on the tick it is processed on, mid-row: `music-clock.md` §3's post-speed-change
transient. The row's cells are read on its first tick (`+0x4371: cmpw $0x0,[0x2000]`), where `A`
writes `_np_speed` (`+0x3dd8`), so the trace's `speed` word leads its `row` column.

A poll's samples are fractional (`40000/70.086` = 570.727…). The original carries the remainder in
the mixing countdown a tick tops up (`+0x50f0: sub %ax,0xbe0` … `+0x5157: add %ax,0xbe0`), and the
port carries it in a `1/70_086`-sample accumulator.

### The published position is a ten-poll-old copy, and a seek does not refresh it

`_np_row`, `_np_pat`, `_np_ord` and `_np_zplus` are **popped from a ring**:

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

primed **ten entries deep** by the module initialiser:

```
  2AC1: 2e c7 06 4e 21 50 00  movw   $0x50,%cs:0x214e   ; 10 × 8 bytes ahead
  2AC8: 2e c7 06 4c 21 00 00  movw   $0x0,%cs:0x214c
```

At one `zpollme` a copper frame (`MAIN/COPPER.ASM:466`) that is a ten-frame delay, and the traces
show it: the port publishes `second5`'s first row change at 426 against the trace's 427, where a
player publishing the engine's own row would publish it at 416. `_np_speed`, `_np_tempo` and
`_np_loop` are written straight through (`+0x3dd8`, `+0x3d7b`, `+0x52c5`).

What feeds the ring — `[0x905]` (row), `[0x907]` (pattern), `[0x909]` (order), `[0x90d]`
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

`_zplaysong` calls the order-advance routine directly (`+0x2597: call 0x442d`), which returns at
`+0x44ee`, so a seek never touches those four words: the ring keeps publishing the previous rows
until the first row advance works through, ten polls later. `second5` reads `ord 0 row 0 pat 0`
from 408 to 426.

### `zplus`, `254` and `255`

The order advance is `+0x442d`, and `music-clock.md` §6 checks this rule against every trace:

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

The rule:

- **bit 0** is set while the order being played has a `254` as its next entry;
- **bit 1** is set when the order was reached by stepping over a `254`;
- so a plain advance reads 0 or 1, a marker crossing 2, and a crossing into an order whose own
  successor is a `254` reads **3**, because nothing clears either bit;
- a seek or a restart credits no marker behind it: the scan starts at the order it was given and
  looks forward, so `SECOND 3`'s restart at order 42, whose predecessor is a `254`, reads 0.

`254` is stepped over. A `255` leaves the scan: if the player has not yet played a row
(`[0x939] == 2`, which `_zplaysong` sets) the song ends at `+0x4476`…`0x4480`; otherwise `_np_loop`
increments (`+0x4481`) and the scan restarts at the loop order `_zplaysong` recorded (0 for every
plain order, `+0x256b`). If that entry is itself a `255`, `+0x4499`'s `je` jumps to `+0x447f`,
past the stop at `+0x4476`: the scan gives up and the player keeps playing what it was playing.

No trace reaches a `255` (`music-clock.md` §10), so this is the object's reading, and the port
follows the bytes in three cases no trace can show:

- **A `_zplaysong` seek onto a `255` is not a stop.** The stop path clears `[0x938]` (`+0x4476`)
  and the seek sets it back to 1 (`+0x259a`), so the player keeps ticking at row 0 of pattern 0
  (the seek zeroed `+0x5702`, `+0x5708`, `+0x570b`) while the ring, fed `[0x940] - 1`, hands the
  parts the index of the `255` itself
  (`a_255_ends_the_order_list_and_loops_when_the_song_was_already_running`).
- **The give-up branch** leaves `[0x940]` where the scan stopped, so the fed order is the last
  index looked at (`entry_now`, `next_order - 1`, `+0x43e6`..`+0x43ef`).
- **A seek zeroes the tick-in-row counter** (`+0x56ec`, `movw $0x0,0x2000`, the routine's first
  instruction). The shell's restarts are mid-song seeks (`restartmus` is reached at
  `MAIN/U2.ASM:775`, `:818`, `:863`, `:885`, `:894` and `:905`), and a stale counter would skip the
  new row 0's cells (`a_seek_mid_song_clears_the_tick_counter_so_the_new_rows_cells_are_read`).

`B` (`+0x3de2`) raises the advance flag and overwrites the order to try next, so the jump lands at
the row's end; `BFF` (`+0x3df1`) ends the song. MUSIC0's three `B`s are at orders 63, 66 and 77,
past every trace. `C`'s parameter is decimal and stored unclamped (`+0x3dfe`: `mov %al,%ah;
shr $0x4,%ah; and $0xf,%al; aad $0xa`); MUSIC0's pattern 71 row 61 `C00` is the trace's ord 26 →
ord 28 over the `254` at 27.

### `A` and `T`, and the row timer's adjuster

- **`A` and `T` are two handlers with two table entries.** Entry 1 of the first-tick table (§9.1)
  is `A`'s body, eleven bytes with **no upper bound**; entry 20 is `T`'s, whose
  `cmp $0x20,%ax; jbe 0x3dce` sends 32 and under to the driver call and never to the speed:

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

  The object's header watch is the second witness: it calls `+0x3dd2` for the module's initial
  speed byte (offset 49, read at `+0x53ca`, called at `+0x53d8`) and `+0x3d73` for the tempo byte
  (offset 50, read at `+0x53b9`, called at `+0x53c7`). So MUSIC1's one `A7F` sets a speed of 127
  (§9.13). Neither module carries a `T` (`the_effect_histogram_is_the_modules_own`: MUSIC0's A×63,
  B×3, C×8, MUSIC1's A×54, C×4).
- **The row timer's fractional adjuster** sits between the `_np_speed` load and the compare:

  ```
    439F: 2e 8b 16 cd 08        mov    %cs:0x8cd,%dx
    43A4: 2e 01 16 cf 08        add    %dx,%cs:0x8cf
    43A9: 73 01                 jae    0x43ac
    43AB: 48                    dec    %ax             ; one tick off the row
  ```

  The addend is `_zspeedadjust` (`0x8cd`), a `LIDATA` run of zero in the image
  (`the_speed_adjusters_are_zero_in_the_image`), and the live word reads 0 (`music-clock.md` §5,
  D46), so it contributes nothing: `second5`'s speed-6 run is 61 row intervals in 534 frames
  against the six-tick rule's 534.4 (`music-clock.md` §3).

### How the player is held to the traces, and the whole play

`crates/sr-audio/tests/trace.rs` holds the player to every filed trace (`music-clock.md` §2; A12,
A54). `cargo test -p sr-audio --test player -- --nocapture each_module_plays` plays each module
from order 0 (`MAIN/U2.ASM:786`) to its loop:

```
  MUSIC0: wrapped after 8732 polls (124.6 seconds of logic ticks) at order 0 pattern 0 row 0, loop counter 1
  MUSIC0: 8732 poll(s), 5983 tick(s) over 998 row(s)
  MUSIC0: 11 wrap(s) in 100,000 polls, none past the order list
  MUSIC1: wrapped after 28618 polls (408.3 seconds of logic ticks) at order 0 pattern 73 row 0, loop counter 1
  MUSIC1: 28618 poll(s), 21240 tick(s) over 5283 row(s)
  MUSIC1: 3 wrap(s) in 100,000 polls, none past the order list
```

Every row is asserted to have had 1 to 255 ticks. MUSIC1's total includes its `A7F` row and the
fifteen after it at 127 ticks each (§9.13) and order 0's full 64 rows (the volume-0 free, §9.1).
Over the whole 100,000 polls every published word stays inside the module's tables.

## 9. The channel effects

`D`, `E`, `F`, `G`, `H`, `J`, `K`, `O`, `Q` and `SDx` move a channel (`A`, `B`, `C` are §8's).
The handlers are `effects.rs`, the row parse and the tick passes `player.rs`. The evidence,
`cargo test -p sr-audio --test effects -- --nocapture --test-threads=1`, is one test per effect on
the modules' own cells (or a module written for the test where no shipped cell reaches a form).
Where the object and stock ST3 disagree, the object wins. The traces cannot judge any of this (a
wrong `D` does not move the timeline), so the tests hold each handler to the object's arithmetic,
whose tables `tests/player.rs`'s `the_ports_tables_are_the_objects_own` reads from the image.

### 9.1 Three dispatch paths, not one

A command is reached from three places, and which one decides when it runs:

- the row's **first tick** (`+0x3c44`, table at `+0x3bd8`), after the row's cells are read;
- every **later tick** of the row (`+0x3d38`, table at `+0x3c0e`);
- the **row parse itself** (`+0x46c2`): `O`'s offset, taken as its cell is installed (`+0x48d6`),
  and the note start every cell's note gets (`+0x4810`).

The two tables differ. Entries 1, 2 and 3 (`A` at `+0x3dd2`, `B`, `C`) are first-tick only; the
later ticks' entries are the bare `ret` at `+0x3d72`. `G` (`+0x3f94`) and `H` (`+0x4005`) have no
first-tick handler, and `K` (`+0x3f00`) and `L` (`+0x3f05`) set `bp` and fall into `D`'s body
(`+0x3f08`), whose tail continues into the vibrato for `K` (`+0x4000`) and the tone porta for `L`
(`+0x3f8f`). Entry 20 (`T`) is `+0x3d73`.

Before it dispatches, the first-tick pass, per channel: restores the period from `aorgspd` and
clears `atrigcnt` for a channel with **no command** (`+0x3d0c`) and for a `D` row (`+0x3d24`);
remembers every non-zero parameter in `alastnfo` (`+0x3cb9`); marks the vibrato position **stale**
for any command but `D`, `H`, `K`, `I` or `R` (`+0x3cea`, `orb $0x80,0x26(%si)`); and **frees** a
channel whose volume is 0, whose cell carries no note, instrument or volume, and which has had
three such rows in a row (`+0x3c97`…`0x3ca9`). The free is on because both headers' `flags` word
is 8, whose bit 3 is "vol 0 optimizations" (`MAIN/STMIK.H:107`). It stops MUSIC1's order 0 row 3
`C00` on channel 2, silent since its row-0 note-off, from running, so the pattern plays its 64 rows
(`the_volume_zero_optimisation_stops_a_pattern_break_from_running`; `bugs.md` §28).

The row parse resets first: `+0x46a4` writes `note = 0xff`, `volume = 0xff` and instrument, command
and info 0 into all 48 channel structures, so an unnamed channel has no command and the previous
row's `D` does not run again. The named cells are installed (`+0x4677`…`0x469c`), clamped
(`+0x4762`…`0x4780`: an instrument above `0x65` becomes 0 and **a volume above `0x3f` becomes
63**), and, for the first sixteen channels (the mixer's; above them is the adlib path), given their
note (`+0x4810`).

The pointer table at `+0x1fa0` maps the 32 channel numbers to their structures and is the identity
(`0x0da0`, `0x0e00`, `0x0e60`, …), so cell channel 5 *is* `_zchn[5]`. Panning is the header's
channel table (§10.3).

### 9.2 A note, and the four readings inside it

`+0x4810` is the routine every note comes through:

- **An instrument clears the sample offset** (`+0x481d`, `movw $0x0,0x6(%si)`) before the note can
  set it, so an `O` with an instrument starts that note from the offset and an `O` without one
  leaves the last offset for the next note.
- **A note under a `G` does not take the period.** `+0x490e` tests `aorgspd` and `+0x4944` the
  command: for a `G` on a playing channel, `aspd` is not written while `aorgspd` and `asldspd` take
  the new period (`+0x4954`, `+0x4957`). The glide walks `aspd` towards `aorgspd`.
- **The volume column replaces both the volume and `aorgvol`** (`+0x495a`), including a volume of
  0, which is why `Cell::present` exists (§6).
- **`0xfe` is the note off**: the position parks at 1, `m_poslow` at 0, the volume at 0, and
  `aorgspd` and `asldspd` at **`0xffff`** (`+0x4906`…`0x4957`), the mixer's "no sample". The live
  period stays, so a `D` row after a note off restores `0xffff`.

The note's period is `+0x593a`: `0xfe` is 0, otherwise the 12-TET table at `+0xa24` indexed by the
semitone and shifted right by the per-octave byte at `+0x97c`. The table is four octaves above the
format's (its C is 27392, so octave 4's C is 1712). **The shift table is eight bytes and the
object indexes it with `note >> 4` unchecked**, so octave 8 reads the byte after it (0, octave 0's
shift) and octaves 9…15 read further. MUSIC1's note `0x80` takes that ninth byte and plays octave
0's period. The port keeps the sixteen bytes the lookup reads
(`the_ports_tables_are_the_objects_own`).

The period is then scaled by the instrument: `+0x5813` is `period × 8363/c2spd`, one integer
division with the two guards an 8086 `div` needs (`+0x581f`, `+0x5825`). Over every PCM instrument
of both files `c2spd` runs from **8163 to 27700**
(`a_notes_period_is_scaled_by_its_instruments_c2spd`).

### 9.3 D — the volume slide, and the fine markers are the other way round

`+0x3f08`, in both tables. The low nibble slides **down** and the high nibble **up**, tested low
first (`+0x3f22`). `D?F` (a low nibble of `0xf`) adds the **high** nibble once, on the first tick
(`+0x3f73` → `+0x3f51`), and `DF?` subtracts the **low** nibble once (`+0x3f81` → `+0x3f31`).
`D0F` and `DF0` fall through to the ordinary 15-a-tick slide.

A zero parameter takes the channel's `alastnfo` and writes it back into the cell's info byte
(`+0x3f11`, `+0x3f14`). The first-tick pass writes `alastnfo` for any command with a non-zero
parameter (`+0x3cb9`), so MUSIC0's 5,795 `D00` cells slide by the last non-zero parameter of
whatever command carried it. The volume clamps to **0…63** (`+0x3f3c`, `+0x3f5e`).

### 9.4 E and F — the portamentos

`+0x3e4f` (`E`, the period grows) and `+0x3e10` (`F`, it shrinks) are one body with one
instruction's sign flipped, in both tables, and do nothing with no note playing (`aorgspd == 0`,
`+0x3e53`):

| tick | info | step |
|---|---|---|
| first | `≤ 0xe0` | nothing |
| first | `0xe1`…`0xf0` | the low nibble, once |
| first | `> 0xf0` | the low nibble **× 4**, once |
| later | `≥ 0xe0` | nothing |
| later | `< 0xe0` | the whole info byte **× 4**, every tick |

(`+0x3e3b`, `+0x3e43`, `+0x3e6a`, `+0x3e6c`.) `aspd` and `aorgspd` move together (`+0x3e70`,
`+0x3e73`), so a `D` row's restore keeps the slid period. A zero info repeats `alastnfo`, written
back into the cell (`+0x3e59`).

### 9.5 G — the tone portamento

`+0x3f94`, later-tick table only. The period walks towards `asldspd` by `info × 4` a tick
(`+0x3fb6`) and clamps onto it (`+0x3fbc`, `+0x3fe3`), with `aorgspd` moving alongside
(`+0x3fc4`). The parameter is remembered in `alasteff2` (`+0x3fa1`), `L`'s source and nothing
else's. The interpolation handler at `+0x57b0` runs only when `[0x919]` is set, the byte `S1x`
writes, which neither module carries (§9.12). `g_walks_the_period_to_the_notes_target_four_a_tick`
drives MUSIC0's `G01` at pattern 8 row 16 channel 1.

### 9.6 H — the vibrato, the stale bit, and which shift the demo runs

`+0x4005`, later-tick only. The parameter is `alasteff1` (`+0x2b`), and a missing high nibble is
**merged** into the previous parameter (`+0x4010`), so `H08` after `H81` is `H88`. The waveform is
the sine at `+0xa50`, 64 signed words of ±255, read at the position's byte offset; the position
steps by `speed × 2` through the even low seven bits.

The level is `value × depth` arithmetic-shifted by **5**, because `[0x915]`, the byte selecting
the shift, is the header flags word's bit 0 (`+0x5424`, `+0x5448`…`+0x5450`), clear in both
files: the amplitude tops out at ±119.

Bit 7 of the position is the **stale flag** (§9.1), tested by each waveform function (`+0x407b`'s
`test $0x80,%ax`): a stale position plays 0 and restarts the phase at `speed × 2`. A channel with no
command does not set it, nor does a `D` row. MUSIC0's `H81` at pattern 47 row 6 follows an `A06` on
row 0, so the vibrato restarts there (`h_vibratos_on_the_objects_sine_table`).

`K`'s vibrato half reads `alasteff1` (`+0x4000`) and `L`'s porta half `alasteff2` (`+0x3f8f`),
while both volume halves are `D`'s body on `alastnfo`: three bytes, not one
(`k_reads_h_s_parameter_for_the_vibrato_and_alastnfo_for_the_slide`).

### 9.7 J — the arpeggio

`+0x4190`, in both tables. `+0x419a` takes the engine's own tick counter mod 3: the note, then the
high nibble's semitones, then the low nibble's. The transposition starts from `lastnote` and
carries one octave past B (`+0x41c8`); only `aspd` is rebuilt, so a row with no command restores
the note. MUSIC0's 137 `J7C` cells (channel 7 of patterns 42–44) at instrument 4, the file's one
instrument at 8363, cycle **1712, 1140, 856** (`j_transposes_the_note_by_the_tick_modulo_three`).

### 9.8 O — the sample offset, in bytes and in multiples of 256

`O` is in neither table. `+0x48d6` runs while the row's cells are read: `cmd == 0xf` writes
`info << 8` into the channel's address field, so `Oxy` is `y × 256` bytes. The offset persists
until a cell names an instrument (`+0x481d`) and applies to the next note to start (`+0x491e`),
whether or not a sample was already playing: both of `+0x490e`'s branches reach `+0x491e`
(`o_starts_the_next_note_at_the_offset_in_bytes`, on MUSIC0's `O05`).

Neither module has an `O` without an instrument (755 and 2417 `O` cells;
`o_without_an_instrument_leaves_the_last_offset_in_place` drives the form from a module written
for the test).

### 9.9 Q — the retrigger

`+0x41ec`, in both tables. The interval is the low nibble and the volume code the high one
(`+0x41f6`); the test is `interval <= atrigcnt` (`+0x4205`), so `y = 0` never retriggers and `y > 0`
fires on the ticks congruent to 0 mod `y`. The counter is `atrigcnt`, which only a `D` row or a row
with no command clears, so the phase runs across rows. The retrigger is `m_pos = 0` (`+0x420f`) plus
the volume variation.

The variation is the tables at `+0x95c` (signed addends) and `+0x96c` (multipliers in sixteenths):
the code's multiplier when it has one, its addend otherwise. The multiplier table holds 10, 8, 24
and 32 at codes 6, 7, 14 and 15 and zero elsewhere (`LIDATA` zero runs). MUSIC0's 84 `Q` cells
carry codes `{0, 4, 7, 12, 14, 15}` on intervals `{0, 3, 4, 6}` and MUSIC1's 38 carry code 0 on
interval 3 (`the_cells_the_tests_drive_are_the_modules_own`), so code 6 is on neither file.
`q_multiplies_when_its_code_has_a_multiplier` drives MUSIC0's `QF4`: 1 → 2 → 4 → 8 → 16 → 32 → 63.

### 9.10 S — `SDx`, the note deferral

`S` goes through a second pair of tables (`+0x424e` first tick, `+0x4261` later, each indexing 16
entries at `+0x3b98` and `+0x3bb8`). **Both modules' `S` cells are `SDx` and nothing else**, and the
test refuses any other subcommand.

`SDx` defers the note: the row parse sees an info of `0xd0`…`0xdf` and returns before starting the
note (`+0x4751`…`+0x475d`), still installing the cell and marking the channel used. Tick 0 puts the
low nibble into `atrigcnt` (`+0x4296`), each later tick decrements it (`+0x429a`), and the tick that
empties it calls the install continuation at `+0x475e`: the clamps the parse skipped, and the note,
with the cell's own volume and instrument.

The files carry delays 2, 3 and 6 (MUSIC1's `SD2` 48 cells, MUSIC0's `SD3` 77 and `SD6` 63), and
`sdx_delays_the_note_to_its_own_tick` asserts each fires on its tick, with its period and volume.

### 9.11 The period's step, and a constant 400 short of the product

`+0x582b` turns a period into playback. Its base frequency is the object's literal:
`mov $0xda,%dx; mov $0x7600,%ax; div period` (`+0x58b9`), **`0x00da7600` = 14,317,056**, which is
400 short of `8363 × 1712` = 14,317,456. So C-4 at 8363 (period 1712) steps at **8362** and the
octave above at **16,725**. The port uses the object's number
(`the_ports_tables_are_the_objects_own`). A period of 0 zeroes the step and returns
(`+0x5876`). §10.2 has the rest of the step.

### 9.12 What the modules do not carry

Counted absent:

- **`I`, `L`, `M`, `N`, `P`, `R`, `U`, `V`, `W`, `X`, `Y` and `Z`: 0 cells in each file.** The
  port keeps the fields `R` and `L` would write (`atreon`, `atremor`, `alasteff2`) and dispatches
  neither; `K`'s vibrato half is `H`'s and is carried.
- **`T`: 0 cells.** Entry 20 still points at `+0x3d73` and the port dispatches it there.
- **`S` subcommands other than `D`**: 0 cells. `S1x`'s byte `[0x919]` is therefore 0 on every
  run, so the interpolating paths it gates are never taken. The other subcommands have handlers in
  the object; the port has none.
- **The other vibrato waveforms**: the selector is written only by `S3x`/`S4x`, so every channel
  runs the sine at `+0xa50`. The ramp (`+0xb50`), square (`+0xad0`) and sine-plus-random
  (`+0x40b4`) are unreachable.

### 9.13 `A7F`, and the seek's initialiser

Two readings of the object that change what the modules play (a fresh note's offset is §9.8):

1. **`A7F` sets a speed of 127** (§8's two handlers). MUSIC1's `A` parameters are
   `{3: 30, 6: 23, 127: 1}`; the `A7F` at pattern 72 row 48 (order 98) makes that row and the
   fifteen after it take 127 ticks each, 2032 ticks against 96 at speed 6
   (`music1s_a7f_row_takes_127_ticks_and_takes_its_own_row_with_it`). `second3d` measures it
   (`music-clock.md` §4).
2. **A seek runs the song initialiser on every channel** (`+0x5614`, reached from `_zplaysong` at
   `+0x258e`): the period, the remembered parameters and the vibrato position start from zero.
   It never stores to `avol`, `a0volcut` or the effect counters, so a mid-song restart keeps them
   (`Channel::initialise`, D45, A38).

## 10. The mixer

`mix.rs` holds the voices and the ring the device drains. The evidence is
`cargo test -p sr-audio --test mix -- --nocapture --test-threads=1` and `--lib mix::`.

### 10.1 Where the mixer is, and how it is called

`_zpollme` (`+0x261b`) checks `_zpollmix` and calls `+0x51c5`; the reference runs polled
(`music-clock.md` §5), once per copper frame (`MAIN/COPPER.ASM:466`). Inside, the DMA loop
(`+0x4ff8`'s tail into `+0x50d0`):

| offset | what it does |
|---|---|
| `+0x50d0` | how many bytes the DMA buffer still wants |
| `+0x50e6` | the chunk: that count, or the tick's countdown, rounded **up to a multiple of 64** (`add $0x41,%ax; and $0xffc0,%ax`) |
| `+0x510c` | **mix the chunk** — `call 0x3905` |
| `+0x512d` | copy the chunk to the DMA buffer's real home (the `movsb` loop at `+0x3ad7`, for EMS) |
| `+0x5150` | **`call 0x4353`, the tick** |
| `+0x5157` | `[0xbe0] += [0x8e1]`, one tick's samples, and loop |

So the samples before a tick are mixed before it runs, and `Player::poll` keeps that order.

The mixer's entry is `+0x3905`. In stereo it halves the count (`+0x396a`: `shrw $1,[0xbde]`),
walks **eight** channel structs for one side (`+0x396f`…`+0x398c`, `bp` from `0xda0` to `0x10a0`),
sets the pass flag `[0x211e] = 1` (`+0x3996`) and walks the **next eight** for the other
(`+0x39ac`…`+0x39c9`); in mono it walks all sixteen into one accumulator (`+0x39cd`,
`[0x211c] = 8`). Each walk calls the voice routine `+0x3765` for a channel with a sample and a
nonzero step (`+0x377e`). The voice reads the sample (`es` = `m_base`'s high word, `+0x3795`/
`+0x37a2`; `si` = `m_pos`, a **byte offset**, `+0x378a`), the bounds `m_end` (`+0x48`) and `m_loop`
(`+0x4c`, read at `+0x383c`), and the cached `m_speed` (`+0x50`) and `m_vol` (`+0x41`).

**The voice routine has two gates.** It tests `m_speed` again at `+0x3783`, then
`m_pos == 0xffff` at `+0x378e`/`+0x3791`, and both return before the sample is read. `0xffff` is
the volume-0 free's park (§9.1), so the object never walks a freed voice's position, and neither
does `voice_contribution`; without the gate, a channel that later becomes audible without a note
restart would read a different sample.

### 10.2 The voice: the arithmetic the port reproduces

**The volume table** is built by `+0x551c` (called from `+0x22d0`, `+0x2792`, `+0x53af`, `+0x5420`:
the sound init, each module load, and each master-volume change the header watch sees). It
allocates 0x5000 bytes at `cs:[0xbe4]` (`+0x29a9`), zeroes the accumulators at `+0x2943`, and fills
**64 levels × 256 sample values** with `(sample × scale[level]) >> 8` (`+0x5553`, `+0x5555`).
`scale` is the 64-byte table at `+0x9c4`, exactly **`floor(level × 255/63)`** (asserted at `+0x9c4`,
`+0x9ff` and `+0xa03`): a sample of 255 at level 63 maps to 254. A sample byte is unsigned with 128
the centre, which `+0x37ae`'s `mov $0x80,%bl; mov %fs:(%bx),%al` reads and `+0x30b4` subtracts.

**The volume** is `m_vol = (avol × master) >> 8` (`+0x5925`), with `[0x913]` =
`min(global volume, 64) × 4` (`+0x55c4`…`+0x55e1`). Both modules' global volume is 64, so the
master is 256 and `m_vol` is the channel's volume. `fademusic` (`MAIN/U2.ASM:333-346`) walks that
byte down (`Player::set_global_volume`).

**The step** is `m_speed`, 16.16 sample bytes an output frame: `+0x58b9` divides `0x00da7600` by
the period as a 32-bit integer (the ≤ 218 branch at `+0x58a9` computes it in two halves; the
remainder is dropped at `+0x58c1`), and `+0x58c9`…`+0x58d8` divides that by the mixing rate, the
quotient into the high word and the remainder's next 16 bits into the low.

**The period is clamped first.** `+0x582b` compares `aspd` with the words at `0x958`/`0x95A`, which
hold **`0x7FFF`** and **`0x0040`**, and substitutes the bound (`+0x5863`/`+0x5869`,
`+0x5890`/`+0x5896`), with the zero test between (`+0x5876`), so the divisions never see a period
outside `[64, 32767]`. On the shipped modules every period above `0x7FFF` is on a volume-0 channel
(a note-off's `0xffff` plus a vibrato), and the low bound never binds. The object writes the
clamped word back only when the flags word at `0x917` has bit 4 set (`+0x583d`, `+0x586e`,
`+0x589a`); both modules carry 8, so the port does not model it (`bugs.md` §28).

**The mixing rate** is `[0x8d9]`, chosen by the header watch (`+0x539b`…`+0x53a9`) as `_zmixspeed`
(mono) or `[0x8d7]` (stereo) from the master volume's bit 7. The shell doubles `_zmixspeed` to
40000 for SB Pro (`MAIN/U2.ASM:759`) and **never writes `[0x8d7]`**: its only store is `+0x228f`,
in the PC-speaker initialiser, which mode 5 never reaches. So stereo voices divide by the initial
**21111**. The frame rate is 20000 a second in stereo (40000 bytes of two), so stereo plays **5.26 %
flat** (`1/1.0556`, 94 cents) and mono is exact. The port keeps the word (`bugs.md` §28.5;
`a_step_is_the_objects_two_divisions`, `a_step_clamps_the_period_the_way_the_object_does`).

**The fetch and the accumulation**: `+0x346e` reads one byte at `es:(si)`, no interpolation;
`+0x3473` looks it up at `(m_vol << 8) | sample`; `+0x3482`'s 32-bit `add %eax,(%di)` adds two
frames into two 16-bit accumulator words at once. The sound init fills the buffer with `0x8080`
(`+0x293b`), and the flush stores **zero** over each pair it reads (`+0x2ce5`, `+0x2cf1`, `+0x30d0`,
`+0x30e8`; `the_flush_clears_the_accumulator_word_it_reads`), so each chunk adds into a clean plane.

**The window clamps** `m_safe`/`m_lsafe` (`+0x58e3`…`+0x5916`) are
`((m_end - m_pos) << 16 - m_poslow) / m_speed`, forced odd, incremented, and clamped to 0x8000; a
voice that cannot reach its end inside the window is stopped (`+0x3834`). That is the object's
chunking; the port mixes a frame at a time and tests the end per frame, which gives the same
samples because the rewrap's displacement is a whole number of loop lengths either way.

**The end.** `m_end` and `m_loop` come from the instrument header at the note start
(`+0x4872`…`+0x48cb`). `+0x4890` reads **`lend`** (offset 0x18 of `struct ds_smp`,
`STARTEMS.C:28-48`) and `+0x489c`'s `cmp $0x0,%ax; je` sends a loop end of zero down the one-shot
path, while `+0x48a6` stores **`lbeg`** (0x14) into `m_loop`. So a sample loops whenever its flag is
set and its loop end is past its begin (`checkins` clears the flag when `lend <= lbeg`,
`STARTEMS.C:145`), and five of MUSIC0's twelve looping samples and one of MUSIC1's nine loop the
whole sample from 0 (`the_modules_loops_and_the_objects_zero_loop_end_rule`). A `lend` at or below
500 is reduced by whole loop lengths (`+0x48ab`…`+0x48b9`) to stay inside `checkins`'s 512-byte pad
(`STARTEMS.C:157-161`); no shipped sample takes that path. A one-shot takes `+0x48be`: `m_end` =
length + 0x20, inside the pad of 0x80 (`STARTEMS.C:167-172`), and `m_loop` = 0xffff. At `m_end`,
`+0x3840` tests the marker: a loop rewraps by adding `m_loop - m_end` (`+0x3856`/`+0x385a`), keeping
the overshoot, and a one-shot stops, `m_speed = 0` (`+0x384d`).

### 10.3 The output: the table the machine holds, and the base it is indexed at

The accumulators are converted at the end of the buffer by `+0x2c89` (stereo) or `+0x30ad` (mono).
Both add `0x400 - [0x2250]`, 1024 less the channels' summed centres, to every word (`+0x2cb4`,
`+0x30b4`) and index the table at `cs:[0x6c]` (`+0x551c`'s second half). So the index is
**`0x400 + value`**, the signed sample about the table's middle; the flush writes zero after the
read, and `mix.rs`'s `FLUSH_BASE` is that base (A14(a)).

The table is a gain curve built from `_np_mastermul` (`+0x5569` reads the byte `[0x916]`, the master
volume's low seven bits, floored at 0x10). The reference's master volume 0xB0 gives
`mul = min((48 × 11) >> 3 - 2, 127) = 64` (`+0x5362`…`+0x538f`). `+0x5397` stores the byte
`[0x916]` as `0xC0`, the 64 with bit 7 (stereo) above it, which it then tests; the byte above that
is `_np_masterflags`' 8, so the published word reads `0x08C0` = 2240 (`music-clock.md` §5). The builder's loop (`+0x5565`…`+0x55c1`) means a zero fill of the 0x800
entries below the ramp, then `K = 0x8000/mul` ramp entries of `(k × (65536/K)) >> 8`.

**The machine holds a different table.** `+0x559d`'s `rep stosw` leaves `CX` = 0, so `+0x55a1`'s
`rep stosb` writes nothing and `+0x55bd`'s tail fills the shortfall with `0xFFFF`: the zero fill is
half its intended length, the ramp starts 768 bytes early and the rest is `0xFF`. A14 is the
measurement, and it reads the same table under MUSIC0 and MUSIC1 (A14(b)). `Curve::build` is the
loop as the machine executes it: `2·⌊(0x800 − K)/4⌋` zeros (the two `shr $1,%cx` at `+0x5592` and
`+0x559b`), the ramp, then `2·⌊rem/2⌋` bytes of `0xFF`. For `mul` 64 that is zeros to 767, the ramp
at 768…1279 (one output level per two entries) and `0xFF` from 1280; for the port's mono `mul` 48
the zero fill is 682, not a plain halving's 683. `multiplier` is the header watch's derivation (64
stereo, 48 mono, because the stereo bit doubles it). `Mixer::frame` forms each voice's value less
its centre, indexes the curve, and widens the byte: `(byte − 128) × 256`.

So a frame's byte is `⌊(value + 256)/2⌋` on the ramp, ±64 levels for ±128 of signal, `0x00` at or
below −257 and `0xFF` from +256: audible, neither gated nor clipped inside the ramp. The curve's
index is clamped where the object's 16-bit `bx` would read past the table (A13).

**The stereo** is the module's channel table: `+0x37b5` reads `amixtype` (`+0x1b`), masks it to
three bits, flips its low bit with the pass flag (`+0x37c1`'s `xor 0x211e,%bl`) and adds the pass
base (`+0x37c5`), selecting the mixing loop (`+0x38c4`'s table) and the shift tables at
`+0x38e4`/`+0x38f4`. The table (`Module::channel_pan`) is `0, 8, 1, 9, 2, 10, 3, 11`, and 0…7 are
the left positions and 8…15 the right, so the eight channels are hard-panned L, R, L, R, L, R, L, R
(`Side::of`). A mono run has one accumulator and no pan.

### 10.4 The port's numbers

`tests/mix.rs` computes its expected samples by hand from the S3M file's bytes, never from a
recording of the port. The load-bearing one, `one_voice_is_the_modules_own_sample_bytes`, takes
MUSIC0's instrument 11 on channel 0 at level 63 and period 1712:

```
MUSIC0 instrument 11, channel 0, level 63, period 1712, step 13700 (16.16), mono, linear curve:
  sample bytes [115, 115, 115, 115, 115, 176, 176, 176]
  the first eight samples [-512, -512, -512, -512, -512, 1536, 1536, 1536]
```

The step is 14317056/1712 × 65536 / 40000 = **13700**, 0.209 of a byte a frame. Level 63's centre is
(128 × 255) >> 8 = 127, so 115 is −13 (index 1011, byte 126, **−512**) and 176 is +48 (1072, byte
134, **1536**). The curve is the linear one because the test's module asks for it by its
master-volume byte; `a_solo_voice_is_audible_through_the_table_the_machine_holds` pins the `mul` 64
table, where a solo voice's ±127 spans bytes 64…191.

The stream's invariants, each its own test:

- **The poll's count is exact**: the first five polls carry 570, 571, 571, 570, 571 samples
  (`a_poll_produces_exactly_the_samples_it_says_it_does`); an odd stereo budget holds one i16 back
  for the next call, as a byte-stream DMA does.
- **The samples-per-tick relation holds**: 64 rows at speed 6 mix 320,178 samples against
  384 × 833 = 319,872, inside one tick (`a_window_of_rows_carries_its_samples`).
- **The drain moves no state** (`the_drain_moves_no_engine_state`), and **the overflow policy drops
  the oldest**: after 15 undrained polls the ring holds 8192 and has dropped 368
  (`a_device_that_stops_draining_loses_the_oldest_samples`).
- **The authentic stream is pinned whole** (`the_authentic_stream_is_pinned_byte_for_byte`: FNV-1a
  over 12,000 polls from MUSIC0's orders 0 and 25 and MUSIC1's 0 and 42), which also pins it
  deterministic and not silent.

### 10.5 Why there is no null test against the original

The emulator runs silent by D17 (`SDL_AUDIODRIVER=dummy`, `mixer_nosound=true`), so no sample
of the original's output exists. What stands in for a null test is construction (the object's
tables and divisions, read from its image by `tests/player.rs`, and samples computed from the file's bytes), the
invariants of §10.4, and a listen to `--audio-dump` (§12). The sound is not claimed to be the 1993
release's: the curve's index clamp (A13) differs, and the rest is the object's arithmetic, the
5.26 % flatness and the table the machine holds included.

## 11. The engine's seam

`crates/sr-engine/src/music.rs` is all a part knows about the music: a `Music` trait whose
methods are the six `dis_*` calls `DIS/DIS.H:37-59` declares, the engine's poll, and the position
(`MusicPosition`). The engine holds an `Option<Box<dyn Music>>`; a part reaches it through
`PartCtx::music()`, `None` when the run has no player. `impl Music for PlayerMusic` is the binary's
(§12), because neither crate may depend on the other.

### Where the poll goes

`copper_doit` (`MAIN/COPPER.ASM:449-468`) runs slot 0's routine (`:458-464`), **`call _zpollme`**
(`:466`), then `u2copperint` (`:468`), the shell's control-key check (`MAIN/U2.ASM:286-289`). Slots 1
and 2 are the retrace routine's (`:302`, `:322`). So the port's tick is slot 0, the music's poll,
the bands, slots 1 and 2 (`the_music_is_polled_between_copper_slot_zero_and_the_first_band`), one
poll a tick however many bands (`a_tick_polls_the_music_exactly_once`).

### The six calls, as the object computes them

| call | what the object does | the port |
|---|---|---|
| `dis_muscode(code)` | `%ax` = `_np_zinfo` (`DIS/DISINT.ASM:249`), the parameter never read | `muscode_from` → **0**, D8's deferral |
| `dis_musplus()` | `%dx` — the block's own arithmetic (`:252-271`) | `musplus_from(zplus, row)` |
| `dis_musrow(code)` | `%bx` = `_np_row`, reloaded on the way out (`:272`) | the published `row`; the trait takes no argument, because the wrapper reads none (`DIS/DISC.ASM:114-123`) |
| `dis_getmframe()` | `_np_zframe` (`:419`) | `Player::zframe()` |
| `dis_setmframe(frame)` | writes `_np_zframe` (`:418`) | `Player::set_zframe(frame)` |
| `dis_sync()` | the `ordersync1` lookup (`:449-463`) | `sync_from(order, row)` — with the boundary below |

The wrappers in `DIS/DISC.ASM:91-123` return one register each (`%ax`, `%dx` at `:111`, `%bx`), and
all read the **published** words (§8): `musplus_from` and `sync_from` take `Player::zplus()` and
`Player::state()`, never the engine's own row.

**`musplus`** (`DIS/DISINT.ASM:252-271`): `%dx` starts at the sentinel `-32` (`:252`); `zplus` 0
keeps it; 1 takes `@@1b`, `row - 64` clamped up at `-32` by a **signed** `jge` (`:270`); 2 takes
`@@1e`, the row while it is below 32 and the sentinel from 32; any other value (3, §8) dispatches
on `row > 32` with an **unsigned** `ja` (`:260`), so row 32 reads the sentinel and row 33 reads
`-31`. `musplus_is_the_objects_own_two_phase_rule` tables every boundary. The parts read
it as a countdown: `GLENZ/MAIN.C:290` waits for `-19`, `TECHNO/KOE.C:213` and `:275` for `-4` and
`-3`, `PLZPART/PLZ.C:68` for 0, `PLZPART/VECT.C:116` for 13, `JPLOGO/JP.C:71` for 4, and
`GLENZ/MAIN.C:411` breaks between `-16` and 0.

**`muscode` is 0.** `_np_zinfo` (segment offset `0x08F7`) is 0 in every sample of every filed
trace (`music-clock.md` §6), so `muscode_from` ignores its arguments and returns 0 (D8). The parts
that wait on other values (`GRID/MAIN.C:72` for 2, `:160` for 3, `HARD/B.C:138` for 1,
`HARD/BP.C:263` for 0xff) wait for ever, as they do against the reference.

### `sync`: the table, the read before it, and the read past it

`ordersync1` (`DIS/DISINT.ASM:428-446`, fifteen entries, keys `(order << 8) | row`, codes 0…10 then
1…4) is carried verbatim. `the_table_is_the_shipped_shells_own_bytes` finds its sixty bytes exactly
once in each shell build the tree holds (`MAIN/U2.EXE`, `MAIN/2NDFIX.EXE`, `U2.EXE`).

Two things about the loop (`:454-460`) decide the behaviour:

1. **The answer is read at `%bx - 2`** (`:460`), the code of the entry *before* the matching one.
   So `(2, 0)` (key `0x0200`) reads 0 and `(2, 1)` reads 1
   (`sync_reads_the_table_the_object_reads`).
2. **The loop runs sixteen times over fifteen entries** (`:455`'s `mov cx,16`), so above the last
   key (`0x4200`) the 16th compare reads the bytes past the table, `sync_10`'s first instruction
   `mov es, cs:stmikseg` (`2e 8e 06 …`):
   - `0x4200 < key <= 0x8E2E`: the compare matches `2e 8e`, the same in every build, and the
     answer is the table's last code, **4**;
   - `key > 0x8E2E`: the compare fails and the fall-through returns the word two bytes further:
     the modrm `06` and the low byte of a linker-set displacement (`0xCD06` in `MAIN/U2.EXE`,
     `0xED06` in `MAIN/2NDFIX.EXE`, `0x9D06` in `U2.EXE`).

What the port does at each end:

- **Before the table it answers.** At `(0, 0)` the read is the word before `ordersync1`, `07 c3`
  (`pop es` at `:420`, `ret` at `:424`, the end of `musframe_9`), in all three builds: **-15609** as
  the parts' 16-bit Turbo C `int` (`PAM/PAM.PRJ` is a Turbo C project;
  `sync_reads_the_table_the_object_reads`).
  `second5` holds `ord 0 row 0` until 426, so a `dis_sync() < N` there (`PAM/OUTTAA.C:39`'s `< 10`,
  `ALKU/MAIN.C:59`'s `< 1`) keeps waiting, as against the reference.
- **Above the table it answers 4 and panics above `0x8E2E`** rather than answer a build-dependent
  number (`sync_reads_the_table_the_object_reads`,
  `sync_refuses_the_first_key_past_the_sixteenth_compare`; A15). `hiddenu` holds keys above
  `0x4200` for 5,148 of its 6,351 frames (81.1 %).

### `zframe`, and what a part sees with no player

`dis_getmframe`/`dis_setmframe` are the two halves of `musframe_9` (`DIS/DISINT.ASM:412-425`): one
word, `_np_zframe`, the **tick** counter (`music-clock.md` §8). `NoMusic`, the engine's stub,
answers as a run with no player: a zero position, the `-32` sentinel, row 0, `_np_zinfo`'s 0, a
frame counter that reads 0 and cannot be written, and a `sync()` that refuses
(`no_music_answers_as_a_run_with_no_player`, `no_music_refuses_to_sync`). A part that waits on a
cue is wrong where it can be seen rather than handed a plausible number. A part boundary is not a
music boundary (`the_music_outlives_the_part_that_is_running`).

## 12. The run's music

What a run does with the player is three things: which module it starts and at which order, when
that music's clock starts, and where its samples go. All three are the shell's
(`bins/secondreality/src/shell.rs`, `src/audio.rs`).

### Which module, at which order: the demo's own restart table

`SECOND n` selects a `partsmask` byte (`MAIN/U2.ASM:110-120`, `:595-630`), and `restartmus`
(`:348-397`) loads a module and seeks it: `ax` is the module (0 = MUSIC0, 1 = MUSIC1), `bx` the
order, and `cx` a speed adjustment written to `_zspeedadjust2` only for output modes 7 and 2
(`:383-389`), which the reference's card is not. `shell::MUSIC_RESTARTS`:

| the block | module | order | the lines |
|---|---|---|---|
| `SECOND 1` (and no argument — `whattorun = 7fh`, `:108`) | MUSIC0 | 0 | `:766-775` (`mov ax,0` / `mov bx,-1`), `:785-787` (`push 0` / `call _zplaysong`) |
| every other block | MUSIC1 | 0 | `:772-775` (`mov ax,1` / `mov bx,0` / `mov cx,48700`) |
| `SECOND 3` | MUSIC1 | 42 | `:856-863`, guarded `whattorun & 4` and not `& 2` |
| `SECOND 4` | MUSIC0 | 18 | `:880-885`, guarded `& 8` — **and not entry-point-only** (below) |
| `SECOND U` | MUSIC0 | 70 | `:889-894`, guarded `& 80h` |
| `SECOND 5` | MUSIC0 | 25 | `:898-905`, guarded `& 16` and not `& 8` |

`mov bx,-1` means "no seek": the shell calls `_zplaysong(0)` itself once the sound is on
(`:785-787`). `every_row_of_the_restart_table_is_the_originals_own` reads `MAIN/U2.ASM` **as
bytes** (the file is not UTF-8) and checks each row's instructions and guard.

**Two restart sites fire mid-run.** `:880-885`'s guard is `whattorun & 8` alone, so it fires for
every mask with bit 3: `SECOND 1` (0x1f), `2` (0x1e), `3` (0x1c) and `4` (0x18). It is block 4's
entry point (`:886-887`); for the other three the shell reaches it between JPLOGO (`exe18`,
`:877-878`) and U2E and restarts MUSIC0 at order 18 under a player that was playing MUSIC1.
`:813-818` (MUSIC1 at order 0, between BEGLOGO and GLENZ) is guarded by `whattorun & 2` (the region
`:801-802` opens and `:855` closes) and `whattorun & 1` (`:813-814`), so only block 1 takes it.
`:856-863`'s and `:898-905`'s second bits keep them entry-point-only, and `:889-894` is kept so by
the masks (only `SECOND U` carries bit 7)
(`one_row_of_the_table_is_not_entry_point_only_and_the_table_says_which`,
`the_second_mid_run_restart_is_two_guards_and_the_masks_carry_both`). The port fires both mid-run
sites: the new player continues from the old one's channel state (D45, §9.13) and waits four
engine ticks (D34; `MusicStart::restart`).

**A run takes the row of the block whose plan begins where it does**: `--block 5` and
`--part endlogo` both begin at ENDLOGO and get `SECOND 5`'s row (`music_for_part`). Three parts no
entry point starts at:

| part | the sequences that reach it | module and order at its start | how the value is known |
|---|---|---|---|
| TUNNELI | blocks 1 and 2 | **MUSIC1, 0** | both blocks are under MUSIC1@0 (`:813-818` for block 1, `:772-775` for block 2). The order at TUNNELI's start is not measured: GLENZ's 7,000-frame main loop lies between the filed GLENZ traces and TUNNELI, so the restart's own order is the value used |
| CRED | every block (every `partsmask` carries bit 4) | **MUSIC0, 26** | `second5`/`second5b` put the published music at **26:56 at index 1212**, CRED's tick 0 — measured |
| ENDSCRL | every block | **MUSIC0, 43** | **derived**: the traces end ~7,600 frames before ENDSCRL (~8917), and the port's player advanced from the block-5 restart (order 25 at 408) is at order 43 across the reference's ENDSCRL-start window (8841..8938) |

The test card has no music: `PartCtx::music()` is `None`, and `--audio-dump` refuses it.

### When the music's clock starts: the anchor, and what a run does about it

`MUSIC_START_FRAME` is **408**, `music-clock.md` §2's anchor for the restart recipes. The modules'
speed/tempo words change at 396 (MUSIC1 loaded by the initial `restartmus` at `:775`) and at 408
(MUSIC0, patched, by the block's restart at `:905`) with zero ticks between: `_zsoundon` (`:779`)
and `_asmcopperinit` (`:781`) take those twelve frames before `_zpollmix = 1` (`:783`) puts the
player under the copper's `zpollme`. Block 1 never takes that restart (its row has `bx = -1`), so
its initial start is its only one, at **399.6**, used as 400 (`music-clock.md` §2). Before 396 the
words read the segment's static initialisers, speed 6 and tempo 125.

A run reaches that clock by waiting, by pre-polling, or by starting with it (`MusicStart`):

- **At or before the anchor it waits.** Blocks 2 and 5 draw from 404; a seek publishes nothing, so
  the position is the ring's priming, and the first published row change is 426 against the
  trace's 427.
- **After it, it pre-polls** `anchor - start_frame` frames (`MusicStart::pre`): 7,310 for
  `--part pam` (its 7,710 anchor against block 1's 400), 10 for `--block 4` (U2E's 418;
  `shell::boot_anchor`). A catch-up longer than the ring's 8192 samples drops its oldest, so a
  pre-polled `--audio-dump` begins clipped.
- **A run numbering its own frames** starts the music on its frame 0 (`--part tunneli`, `cred`,
  `endscrl`).

`shell.rs`'s `the_runs_music_is_where_the_trace_holds_it_at_every_frame_the_trace_has` drives a
`--block 5` run in process over every row of `reference/music/second5/trace.csv` from 404, holding
content and placement (within A12's 3 frames):

```
second5: 778 of 894 frames agree exactly (87.025 %); worst displacement 2 frame(s)
```

`a_block_five_run_plays_music_one_at_the_order_its_entry_point_names` finds the trace's `0:0`,
`25:1`, `25:9`, `25:55`, `26:53` at 404, 430, 500, 900 and 1180 through the real binary.

**The boot's 396→408 transient is not reproduced**: a run installs its entry point's module once.
The position words agree (the ring's `0:0`); the speed/tempo words at 404…407 differ (MUSIC0's 6/120
against MUSIC1's 3/130). No `dis_*` call returns them and no ported part reads
`MusicPosition::speed` or `::tempo`, so only the status line and `Player::state` can see it.

### Where the samples go

The adapter (`PlayerMusic`), the device and the WAV dump are `bins/secondreality/src/audio.rs`'s
(its module doc has the drain policy: silence on underrun, never a stall or a resample). The device
is opened at the mixer's own specification or not at all; `--audio-dump` writes 16-bit PCM at
**20000 frames a second stereo**, because `MixRate::Hz40000` counts i16 samples and two make a
frame (§10.2) (`bins/secondreality/tests/audio.rs`). The status line shows the *published*
`order:row`, ten polls behind the player (§8; `the_status_line_shows_where_the_music_is`).

The **fade** is `audio::fade_music`, one pass of `MAIN/U2.ASM:333-346`'s loop body: the module's
global-volume byte (`ds:[30h]`) less `step`, saturating at zero, called once a tick with the demo's
step of 1 once the run has nothing left to play, `@@partsdone` (`:935-942`, `mov cx,1` at `:938`);
the looping path's call (`:927`) is D42's. From both modules' `0x40` it takes 64 ticks.

## 13. Enhanced quality — the port's own renderer, off by default

`--sound-quality enhanced` (`Quality::Enhanced`, `crates/sr-audio/src/player.rs`) mixes with a
renderer the original does not have. **Authentic is the default**, and every comparison against
the original mixes authentically (D43, A60).

### What it is

The authentic path holds one sample byte for each of 20,000 stereo frames a second (§10.2), and
that stepping is audible. The enhanced path (`mix.rs`, `Mixer::for_options`) renders the same
voices at **48,000 frames a second** (`PlayerOptions::output_rate`):

- **The clock is untouched**: the same `Player::poll`, `samples_per_tick` and voice `advance`, so
  every published word is the same at every poll (`enhanced_keeps_the_authentic_tick_schedule`).
- **Each authentic frame becomes 2 or 3 output frames** by an exact accumulator, `floor(k × 12/5)`
  after `k` (`enhanced_emits_twelve_frames_for_every_five`).
- **Each voice is interpolated linearly** at its position plus `j/n` of its step, and each side's
  sum takes the authentic curve's small-signal gain about its centre (`Curve::sample` at
  `0x400 ± 1`), clamped to `i16`.

Every player a run installs takes the run's options, restarts included
(`a_restart_keeps_the_runs_sound_quality`).

### What it measures against the authentic path

`cargo test -p sr-audio --test enhanced -- --nocapture --test-threads=1`:

```
MUSIC0: 3,000 polls, 856091 authentic frames, 2054618 enhanced frames
MUSIC1: 3,000 polls, 856091 authentic frames, 2054618 enhanced frames
MUSIC0: 30000 polls, the same words at every one; both end at 6:52, zframe 20554
MUSIC1: 30000 polls, the same words at every one; both end at 6:21, zframe 22265
MUSIC0: ten seconds from order 0, RMS authentic 3933.8, enhanced 3854.2, -0.177 dB
MUSIC1: ten seconds from order 0, RMS authentic 2159.2, enhanced 2023.3, -0.565 dB
(…)
MUSIC0 order 6 row 32 channel 3: authentic 494.809 Hz, enhanced 494.369 Hz, -0.0890 % (zero crossings, unfiltered: 1178 and 1183)
(…)
MUSIC1 order 43 row 12 channel 2: instrument 53, 20866 bytes a second, loop 49098..57850
```

The **pitch** is the same by construction, the step being shared, the 5.26 % stereo flatness
included. It is measured on every sustained, looped note the survey finds, each rendered alone for a
second, by the autocorrelation's fundamental: the worst agrees to 0.089 % against a 0.5 %
tolerance. Zero crossings count the waveform's shape, which differs by design, so they are printed
and not used. MUSIC1's order 43 row 12 channel 2 is left out: its loop starts at byte 49,098 and
the voice moves 20,866 bytes a second, so the second is all attack. The **level** is the RMS over
ten seconds from order 0, within 1 dB.

**The device's margin.** At 48000 Hz stereo SDL2 2.30.0 gives a 4096-frame callback on the
`dummy`, `alsa`, `pulseaudio` and `pipewire` drivers, against 1024 at 20000 Hz; the enhanced ring
(`Player::ring_capacity`, 19,660 samples) is a 2.4× margin over it, the authentic one 4×.
`window::run` prints both.

### What it does not reproduce

- **The curve's table and its clamp.** The enhanced sum uses only the ramp's slope and clamps at
  `i16`; a signal inside the ramp has the same gain on both paths, and one beyond it saturates at a
  slightly different level (`0xFF` is 32,512, `i16::MAX` 32,767). A13's clamp and A14's ends are
  the table's, which the enhanced path never reads.
- **The stepping itself**, which is the point: the held byte, its 20 kHz images, and the
  quantisation to one output level per two entries.
