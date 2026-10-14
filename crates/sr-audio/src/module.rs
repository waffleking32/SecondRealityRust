//! The S3M container, as Second Reality ships it: `assets/MAIN/MUSIC0.S3M` and
//! `MUSIC1.S3M`, read in their original scrambled form.
//!
//! The loader is a translation of the original's own — `MAIN/STARTEMS.C`'s `loads3mems`,
//! which is the code that read these very files in 1993 — so the structure below is that
//! function's, not the generic S3M documentation's. Three of its readings are worth
//! stating before the code, because each is a place where a plausible-looking decoder
//! still "decodes" plausible-looking garbage:
//!
//! 1. **A pattern block is `ln` bytes from `ptr * 16`, and its packed data is the `ln - 2`
//!    bytes from `ptr * 16 + 2`.** `ln` includes its own two-byte length word
//!    (`STARTEMS.C:348-351`: `read(h,&u,2)`, then `read(h,p+2,u-2)`, and the player's copy
//!    keeps the length at `p` and the data after it). Read the other way round — `ln` as
//!    the data length — and 8 of MUSIC0's 76 consecutive pattern pairs and 11 of MUSIC1's
//!    stop tiling contiguously, which is how the mistake announces itself
//!    (`crates/sr-audio/tests/module.rs` measures both readings).
//! 2. **The packed cell's flag byte carries its channel in the low five bits, and a row
//!    ends on a byte that is zero.** Testing for "low five bits are zero" instead would
//!    read 1,990 of MUSIC0's 14,980 cells and 2,835 of MUSIC1's 17,746 — every cell on
//!    channel 0 — as the end of their row, and take the bytes after them as the next row's
//!    cells.
//! 3. **The pattern data is XOR-scrambled** (D5). Stock S3M decoders fail on these files.
//!    [`descramble`] undoes it with the recovered keystream, starting at index 0 of each
//!    pattern's own data.
//!
//! Nothing here interprets what a cell *means*: the order list keeps 254 and 255 as the
//! bytes they are, `Cell::instrument` keeps the file's own 1-based number, and a cell's
//! `volume = 0` is kept as a volume, not folded into "no cell" (189 of MUSIC1's cells
//! carry nothing but a zero volume — see [`Cell::present`]). That is deliberate: what the
//! player does with them is decided by the reference's own behaviour (Phase 3's Task 1
//! trace) and implemented in `player.rs`, not guessed at by the container.

/// The S3M channel count: the file always has a 32-byte channel table and a packed cell
/// may name any of 32 channels (`STARTEMS.C:67`, `:505-511`'s `zchn[32]`).
pub const CHANNELS: usize = 32;

/// Every S3M pattern is 64 rows (`upstream/SecondReality/MAIN/STMIK.300`'s player walks
/// 0..63; the format has no field for a pattern's row count).
pub const ROWS: usize = 64;

/// The file header's size, in bytes: `sizeof(struct ds_fileheader)`
/// (`upstream/SecondReality/MAIN/STARTEMS.C:50-68`) is 0x60 with `#pragma pack(1)`.
const HEADER_SIZE: usize = 0x60;

/// One sample header (`sizeof(struct ds_smp)`, `STARTEMS.C:28-48`).
const INSTRUMENT_SIZE: usize = 0x50;

/// The file's own offset of the `SCRM` signature (`STARTEMS.C:62`).
const SIGNATURE_OFFSET: usize = 44;

/// The `SCRM` field's value in these modules: the S3M magic.
const SIGNATURE: &[u8; 4] = b"SCRM";

/// The sample headers' own signature, at `ds_smp._magic` (`STARTEMS.C:47`).
///
/// `checkins` *writes* it at load time (`:134`) rather than checking it, so the loader does
/// not require it — but both shipped modules already carry it, which is a measured fact
/// about the files and is asserted as one in the tests.
pub const SAMPLE_MAGIC: &[u8; 4] = b"SCRS";

// ---------------------------------------------------------------------------------------
// The keystream
// ---------------------------------------------------------------------------------------

/// XOR D5's keystream over one pattern's packed data.
///
/// `pattern` is the data *after* the block's two-byte length word — the bytes at
/// `ptr * 16 + 2 .. ptr * 16 + ln` (`STARTEMS.C:348-351`) — because the keystream restarts
/// at index 0 for every pattern (D5; spec §7.1).
pub fn descramble(pattern: &mut [u8]) {
    for (i, byte) in pattern.iter_mut().enumerate() {
        *byte ^= key(i);
    }
}

/// The keystream byte for the packed data's `i`th byte (D5, spec §7.1):
///
/// ```text
/// n      = (i >> 1) + 1
/// key(i) = (((n ^ (n >> 2)) << 3) | ((5*i + 2) & 7)) & 0xFF
/// ```
fn key(i: usize) -> u8 {
    let n = (i >> 1) + 1;
    ((((n ^ (n >> 2)) << 3) | ((5 * i + 2) & 7)) & 0xFF) as u8
}

// ---------------------------------------------------------------------------------------
// Cells and patterns
// ---------------------------------------------------------------------------------------

/// One cell of a pattern: one channel's note, instrument, volume and effect for one row.
///
/// The file stores these in a packed form whose flag byte says which field groups follow
/// (see [`Cell::present`]); a cell that carries none of them cannot exist in the packed
/// stream, so [`Cell::EMPTY`] is the loader's own representation of an *absent* cell — a
/// channel slot the file's row never named.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Cell {
    /// The note byte, `(octave << 4) | semitone` (spec §7.1). ST3's two special values are
    /// 0xFE (note off) and 0xFF (no note), and both occur in these modules; 0 is the lowest
    /// real note, not "none". The player decides what they mean.
    pub note: u8,
    /// The instrument, **1-based as the file stores it** (0 = "no instrument change").
    ///
    /// Measured: MUSIC1 carries instrument byte 54 while `insNum` is 54 (and byte 0 on its
    /// 35 note-off cells). A 0-based reading would index a 55th instrument the file does
    /// not have.
    pub instrument: u8,
    /// The volume column, 0..64. Zero is a real value, not "absent" — see [`Cell::present`].
    pub volume: u8,
    /// The effect, in ST3's numeric alphabet (1..=26 = `A`..`Z`, 0x30..=0x39 = digits, 255 =
    /// none, spec §7.1). 0 is the second spelling of "no command": D5 records that MUSIC1 has
    /// one cell whose command byte is 0, and that 0 means *no command* — and **this crate's
    /// tests** are what locate it (MUSIC1's pattern 13, row 19, channel 5, flag byte 0x85,
    /// info 0x40; D5 records the cell, not where it is).
    pub command: u8,
    /// The effect's parameter byte.
    pub info: u8,
    /// Which field groups the packed flag byte carried: a mask of [`Cell::NOTE`],
    /// [`Cell::VOLUME`] and [`Cell::COMMAND`].
    ///
    /// This is not in the plan's field list and it is here because the distinction is
    /// measurable in these modules: a cell whose only content is a volume of zero is
    /// indistinguishable from an absent cell by its five fields alone, and both modules
    /// contain such cells (3 in MUSIC0, 189 in MUSIC1 — the test prints them). A player
    /// that treated them as absent would leave the channel's volume where it was instead of
    /// taking it to zero.
    pub present: u8,
}

impl Cell {
    /// The flag byte's bit 5: a note **and** an instrument follow, two bytes
    /// (`STARTEMS.C:505-511`'s unpacker; the two are one field group, which is the trap that
    /// desynchronises a decoder that reads bit 5 as "note" alone).
    pub const NOTE: u8 = 0x20;

    /// The flag byte's bit 6: a volume byte follows.
    pub const VOLUME: u8 = 0x40;

    /// The flag byte's bit 7: an effect and its info byte follow.
    pub const COMMAND: u8 = 0x80;

    /// A channel slot the file's row never named.
    pub const EMPTY: Cell = Cell {
        note: 0,
        instrument: 0,
        volume: 0,
        command: 0,
        info: 0,
        present: 0,
    };

    /// True when the row never named this channel — a presence test, not a value test.
    pub fn is_empty(&self) -> bool {
        self.present == 0
    }

    /// True when the file's cell carried a note (with it, an instrument).
    pub fn has_note(&self) -> bool {
        self.present & Self::NOTE != 0
    }

    /// True when the file's cell carried a volume column, whatever its value.
    pub fn has_volume(&self) -> bool {
        self.present & Self::VOLUME != 0
    }

    /// True when the file's cell carried an effect.
    pub fn has_command(&self) -> bool {
        self.present & Self::COMMAND != 0
    }
}

/// One pattern: 64 rows of 32 channel slots each.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Pattern {
    /// 64 rows (`[ROWS]`), each holding one cell per channel, indexed by the channel number
    /// the file's own flag byte carried.
    pub rows: Vec<[Cell; CHANNELS]>,
    /// How many channel slots this pattern addresses: one past the highest channel its cells
    /// name, or 0 for a pattern with no cells at all. Derived from the cells, not declared
    /// by the file (the S3M container has no per-pattern channel count).
    pub channel_count: u8,
}

// ---------------------------------------------------------------------------------------
// Instruments
// ---------------------------------------------------------------------------------------

/// One sample header, `struct ds_smp` (`STARTEMS.C:28-48`), field for field.
///
/// The loader stores the header as the file wrote it. `STARTEMS.C:129-177`'s `checkins`
/// clamps three of these fields (volume to 64, `c2spd` to 65535, length to 64000) and pads
/// every sample with 512 bytes after its end for the mixer's benefit; none of that is done
/// here, because it is what the *player* did to its copy in memory and Task 5 reproduces it
/// against the same numbers.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Instrument {
    /// The type byte: 0 = empty, 1 = PCM, 2..=7 = adlib (`STARTEMS.C:132`, `:175-176`).
    pub sample_type: u8,
    /// The DOS filename, 12 bytes, NUL-padded.
    pub filename: [u8; 12],
    /// The high byte of the sample's paragraph pointer (`STARTEMS.C:386`).
    pub memseg2: u8,
    /// The low 16 bits of the sample's paragraph pointer (`STARTEMS.C:385`).
    pub memseg: u16,
    /// The sample's length in bytes.
    pub length: u32,
    /// The loop's first byte.
    pub loop_begin: u32,
    /// The loop's end byte.
    pub loop_end: u32,
    /// The default volume (the file's own value, see the type's doc comment).
    pub volume: u8,
    /// The disk byte. Zero in both modules, and unused by the original's loader.
    pub disk: u8,
    /// The packing byte: 0 = unpacked, 1 = Amiga, 2 = ADPCM (`STARTEMS.C:39`).
    pub pack: u8,
    /// The flags: bit 0 = loop (`STARTEMS.C:40`).
    pub flags: u8,
    /// The "C2SPD" playback rate for middle C.
    pub c2spd: u32,
    /// The 28-byte sample name.
    pub name: [u8; 28],
}

impl Instrument {
    /// The type byte of an empty instrument (`STARTEMS.C:132`: only type 1 is a sample).
    pub const EMPTY: u8 = 0;

    /// The type byte of a PCM instrument.
    pub const PCM: u8 = 1;

    /// True for the 22-of-44 and 41-of-54 instruments that carry a PCM sample.
    pub fn is_pcm(&self) -> bool {
        self.sample_type == Self::PCM
    }

    /// True for the type-0 instruments. These are a normal case (`checkins` treats a
    /// length-0 sample the same way, `STARTEMS.C:135-139`), not a malformed file.
    pub fn is_empty(&self) -> bool {
        self.sample_type == Self::EMPTY
    }

    /// True when the file asks for the sample to loop (`STARTEMS.C:40`, `:149`).
    pub fn loops(&self) -> bool {
        self.flags & 1 != 0
    }

    /// The sample's offset in the module file: `memseg << 4` plus `memseg2 << 20`
    /// (`STARTEMS.C:385-386`).
    pub fn sample_offset(&self) -> usize {
        (usize::from(self.memseg) << 4) | (usize::from(self.memseg2) << 20)
    }
}

// ---------------------------------------------------------------------------------------
// The module
// ---------------------------------------------------------------------------------------

/// A loaded S3M module: the container's own contents, with the patterns descrambled and
/// decoded into cells.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Module {
    orders: Vec<u8>,
    channel_pan: [u8; CHANNELS],
    instruments: Vec<Instrument>,
    samples: Vec<Vec<u8>>,
    patterns: Vec<Pattern>,
    initial_speed: u8,
    initial_tempo: u8,
    global_volume: u8,
    master_volume: u8,
    flags: u16,
}

impl Module {
    /// Decode a module from the bytes of an S3M file, exactly as
    /// `upstream/SecondReality/MAIN/STARTEMS.C`'s `loads3mems` reads it.
    ///
    /// The bytes are the *shipped* file: the pattern data is scrambled and is descrambled
    /// here (D5), and no copy of the modules is converted on disk.
    pub fn load(bytes: &[u8]) -> Result<Module, ModuleError> {
        // The header, and the signature that says these bytes are an S3M at all. The magic
        // is at offset 44 rather than 0 (`STARTEMS.C:62`'s `_magic_signature`); the bytes
        // before it are the module's title.
        let header = bytes.get(..HEADER_SIZE).ok_or(ModuleError::Truncated {
            what: "the file header",
            at: bytes.len(),
            wanted: HEADER_SIZE,
        })?;
        let signature: [u8; 4] = header[SIGNATURE_OFFSET..SIGNATURE_OFFSET + 4]
            .try_into()
            .expect("four bytes");
        if &signature != SIGNATURE {
            return Err(ModuleError::Signature(signature));
        }

        let order_count = u16_at(bytes, 32);
        let instrument_count = u16_at(bytes, 34);
        let pattern_count = u16_at(bytes, 36);

        // The tables: the order list, then one paragraph pointer per instrument, then one
        // per pattern (`STARTEMS.C:333-334` — `parains` right after the order list, and
        // `parapat` `insNum` entries later).
        let orders_at = HEADER_SIZE;
        let instruments_at = orders_at + usize::from(order_count);
        let patterns_at = instruments_at + 2 * usize::from(instrument_count);
        let tables_end = patterns_at + 2 * usize::from(pattern_count);
        if bytes.len() < tables_end {
            return Err(ModuleError::Truncated {
                what: "the order and pointer tables",
                at: bytes.len(),
                wanted: tables_end,
            });
        }

        let orders = bytes[orders_at..instruments_at].to_vec();

        // The file's 32-byte channel table, the last field of `ds_fileheader`
        // (`STARTEMS.C:67`, at offset 64: 28 title, 1 marker, 1 type, 2 unused, six words to
        // offset 44, the signature, four bytes, then the 13 reserved ones).
        let mut channel_pan = [0u8; CHANNELS];
        channel_pan.copy_from_slice(&header[64..64 + CHANNELS]);

        // The header's own values (`STARTEMS.C:56-66`'s `ds_fileheader` order): global
        // volume, initial speed, initial tempo, then the byte the S3M format calls the
        // master volume — which is where the stereo bit lives for `cwtv` 0x1300 files.
        let global_volume = header[48];
        let initial_speed = header[49];
        let initial_tempo = header[50];
        let master_volume = header[51];

        // `ds_fileheader.flags` (`STARTEMS.C:58`), the file's offset 38 — the word the
        // player watches as `_np_masterflags` (`+0x53ef`…`+0x5401`) and whose bit 0 becomes
        // its amiga-compatibility byte (`+0x5424`). Both modules carry 8.
        let flags = u16_at(bytes, 38);

        // The instruments' sample bytes are sliced here, by paragraph pointer; the loader
        // does not pad or loop-extend them (`checkins`, `STARTEMS.C:129-177`).
        let mut instruments = Vec::with_capacity(usize::from(instrument_count));
        let mut samples = Vec::with_capacity(usize::from(instrument_count));
        for index in 0..usize::from(instrument_count) {
            let pointer = usize::from(u16_at(bytes, instruments_at + 2 * index));
            let at = pointer * 16;
            let block = bytes
                .get(at..at + INSTRUMENT_SIZE)
                .ok_or(ModuleError::Truncated {
                    what: "a sample header",
                    at: bytes.len(),
                    wanted: at + INSTRUMENT_SIZE,
                })?;
            let instrument = Instrument {
                sample_type: block[0x00],
                filename: block[0x01..0x0D].try_into().expect("twelve bytes"),
                memseg2: block[0x0D],
                memseg: u16::from_le_bytes([block[0x0E], block[0x0F]]),
                length: u32::from_le_bytes(block[0x10..0x14].try_into().expect("four bytes")),
                loop_begin: u32::from_le_bytes(block[0x14..0x18].try_into().expect("four bytes")),
                loop_end: u32::from_le_bytes(block[0x18..0x1C].try_into().expect("four bytes")),
                volume: block[0x1C],
                disk: block[0x1D],
                pack: block[0x1E],
                flags: block[0x1F],
                c2spd: u32::from_le_bytes(block[0x20..0x24].try_into().expect("four bytes")),
                name: block[0x30..0x4C].try_into().expect("twenty-eight bytes"),
            };

            // The port's condition here is `is_pcm() && length != 0`; the original's is
            // `type == 1 && memseg` (`STARTEMS.C:367`, the guard on its sample-loading loop).
            // The two differ only for a type-1 header with `memseg == 0` and a non-zero
            // length, and neither module has one — the test counts every PCM instrument's
            // `memseg` and asserts none is zero — so nothing here is built on the
            // difference. This is the condition the port wants: the bytes a sample has are
            // the file's `length` of them at the header's paragraph pointer, and a header
            // whose length is zero has none (`checkins`'s no-sample case, `:135-139`),
            // whatever its pointer says.
            let sample = if instrument.is_pcm() && instrument.length != 0 {
                let at = instrument.sample_offset();
                let end = at + instrument.length as usize;
                bytes
                    .get(at..end)
                    .ok_or(ModuleError::Sample {
                        index,
                        at,
                        length: instrument.length,
                    })?
                    .to_vec()
            } else {
                Vec::new()
            };

            instruments.push(instrument);
            samples.push(sample);
        }

        // The patterns, in pointer-table order. A pointer of zero is not an error and not a
        // block: the original skips it and leaves an empty pattern for the player
        // (`STARTEMS.C:346`, `if(!b) continue;`).
        let mut patterns = Vec::with_capacity(usize::from(pattern_count));
        for index in 0..usize::from(pattern_count) {
            let pointer = usize::from(u16_at(bytes, patterns_at + 2 * index));
            patterns.push(if pointer == 0 {
                Pattern {
                    rows: vec![[Cell::EMPTY; CHANNELS]; ROWS],
                    channel_count: 0,
                }
            } else {
                let at = pointer * 16;
                // `ln` includes its own two bytes (`STARTEMS.C:348-351`), so the block's
                // data is `ln - 2` bytes from `at + 2`.
                let Some(length_word) = bytes.get(at..at + 2) else {
                    return Err(ModuleError::Pattern {
                        index,
                        row: 0,
                        fault: PatternFault::BlockRuns { at },
                    });
                };
                let length = usize::from(u16::from_le_bytes(
                    length_word.try_into().expect("two bytes"),
                ));
                let data_end = at + length;
                if length < 2 || data_end > bytes.len() {
                    return Err(ModuleError::Pattern {
                        index,
                        row: 0,
                        fault: PatternFault::BlockRuns { at },
                    });
                }
                let mut data = bytes[at + 2..data_end].to_vec();
                descramble(&mut data);
                decode_pattern(index, &data)?
            });
        }

        Ok(Module {
            orders,
            channel_pan,
            instruments,
            samples,
            patterns,
            initial_speed,
            initial_tempo,
            global_volume,
            master_volume,
            flags,
        })
    }

    /// The order list, **as the file wrote it**: 254 and 255 are kept, not normalised and
    /// not filtered. What the player does with them is the player's business, decided by the
    /// reference's own order counter (`docs/fidelity/music-clock.md` §10) and implemented in
    /// `player.rs` — not by this loader and not by a reading of ST3 semantics. The shell is
    /// what hands the list over: `_zplaysong(order)` (`MAIN/U2.ASM:786` for the initial
    /// start, and `restartmus`'s call at `:393-394`).
    pub fn orders(&self) -> &[u8] {
        &self.orders
    }

    /// `ordNum` — the order list's length.
    pub fn order_count(&self) -> usize {
        self.orders.len()
    }

    /// `patNum` — the number of patterns.
    pub fn pattern_count(&self) -> usize {
        self.patterns.len()
    }

    /// `insNum` — the number of sample headers.
    pub fn instrument_count(&self) -> usize {
        self.instruments.len()
    }

    /// The header's initial speed byte (`STARTEMS.C:64`, offset 49). The player's *speed*
    /// starts here; a pattern's `A` command changes it.
    pub fn initial_speed(&self) -> u8 {
        self.initial_speed
    }

    /// The header's initial tempo byte (`STARTEMS.C:65`, offset 50), **before** the shell's
    /// patch — see [`Module::apply_tempo_patch`].
    pub fn initial_tempo(&self) -> u8 {
        self.initial_tempo
    }

    /// The header's global volume byte (`STARTEMS.C:63`, offset 48). MUSIC0 and MUSIC1 both
    /// carry 64.
    pub fn global_volume(&self) -> u8 {
        self.global_volume
    }

    /// The byte after the initial tempo (offset 51). The 1993 S3M format calls it the master
    /// volume; for `cwtv` >= 0x1300 its bit 7 is the stereo flag (spec §7.2, `MENU.C`'s
    /// output-mode menu). Both modules carry 176 = `0xB0`.
    pub fn master_volume(&self) -> u8 {
        self.master_volume
    }

    /// True when the master volume byte's stereo bit is set.
    pub fn stereo(&self) -> bool {
        self.master_volume & 0x80 != 0
    }

    /// The header's `flags` word (`STARTEMS.C:58`'s `unsigned flags`, the file's offset 38).
    ///
    /// The player reads it as `_np_masterflags` — the word the STMIK documentation's flag
    /// list belongs to (`MAIN/STMIK.H:104-107`: bit 3 "vol 0 optimizations", bit 4 "full
    /// amiga compatibility") — through the header watch at `+0x53ef`/`+0x5401`, and takes
    /// its **bit 0** as `[0x915]`, the byte that picks the vibrato's level shift
    /// (`+0x5424`, `+0x5448`…`+0x5450`). Both modules carry **8**, so the shift is 5 and the
    /// "volume 0" optimisation is on.
    pub fn flags(&self) -> u16 {
        self.flags
    }

    /// The file's 32-byte channel table (`STARTEMS.C:67`), as bytes. Both modules carry
    /// `0, 8, 1, 9, 2, 10, 3, 11` and then 24 bytes of `255`; what the numbers mean is the
    /// mixer's business, so the loader keeps them as the file wrote them.
    pub fn channel_pan(&self) -> &[u8; CHANNELS] {
        &self.channel_pan
    }

    /// The `index`th sample header, 0-based as the file's own instrument table is.
    pub fn instrument(&self, index: usize) -> Option<&Instrument> {
        self.instruments.get(index)
    }

    /// The `index`th pattern, 0-based.
    pub fn pattern(&self, index: usize) -> Option<&Pattern> {
        self.patterns.get(index)
    }

    /// The `index`th instrument's sample bytes, exactly the `length` bytes the header
    /// names, sliced from `memseg << 4 | memseg2 << 20` (`STARTEMS.C:385-388`).
    ///
    /// `None` when there is no sample to slice: an out-of-range index, an instrument that
    /// is not PCM, or one whose length is zero (`checkins`'s no-sample case,
    /// `STARTEMS.C:135-139`).
    pub fn sample_bytes(&self, index: usize) -> Option<&[u8]> {
        let sample = self.samples.get(index)?;
        if sample.is_empty() {
            None
        } else {
            Some(sample)
        }
    }

    /// The shell's tempo patch: `MAIN/STARTMUS.C:257` writes `module[50] = 0x78` — the
    /// initial-tempo byte — but **only** in `case 0`, which is MUSIC0 (`:252-265`). MUSIC1
    /// is played at its file's own 130.
    ///
    /// The patch belongs to the caller, not to [`Module::load`]: the module does not know
    /// which file it came from, and the decision is the shell's.
    pub fn apply_tempo_patch(&mut self, tempo: u8) {
        self.initial_tempo = tempo;
    }
}

/// Decode one pattern's descrambled packed data into 64 rows of 32 slots.
fn decode_pattern(index: usize, data: &[u8]) -> Result<Pattern, ModuleError> {
    let mut rows = vec![[Cell::EMPTY; CHANNELS]; ROWS];
    let mut channel_count = 0u8;
    let mut position = 0usize;

    for (row, cells) in rows.iter_mut().enumerate() {
        let mut named = 0u32;
        loop {
            let Some(&flag) = data.get(position) else {
                return Err(ModuleError::Pattern {
                    index,
                    row,
                    fault: PatternFault::RanOut,
                });
            };
            position += 1;
            // A row ends on a byte that is zero. The low five bits are the *channel*, so
            // `flag & 0x1f == 0` would end the row at every channel-0 cell instead — and
            // 1,990 of MUSIC0's cells and 2,835 of MUSIC1's sit on channel 0.
            if flag == 0 {
                break;
            }
            let channel = usize::from(flag & 0x1F);

            let mut cell = Cell::EMPTY;
            // Bit 5 is a note **and** an instrument, two bytes, in that order. Reading it
            // as a note alone desynchronises the whole remainder of the pattern while still
            // decoding "cells".
            if flag & Cell::NOTE != 0 {
                let (note, instrument) = match (data.get(position), data.get(position + 1)) {
                    (Some(&note), Some(&instrument)) => (note, instrument),
                    _ => {
                        return Err(ModuleError::Pattern {
                            index,
                            row,
                            fault: PatternFault::RanOut,
                        })
                    }
                };
                position += 2;
                cell.note = note;
                cell.instrument = instrument;
                cell.present |= Cell::NOTE;
            }
            if flag & Cell::VOLUME != 0 {
                match data.get(position) {
                    Some(&volume) => {
                        position += 1;
                        cell.volume = volume;
                        cell.present |= Cell::VOLUME;
                    }
                    None => {
                        return Err(ModuleError::Pattern {
                            index,
                            row,
                            fault: PatternFault::RanOut,
                        })
                    }
                }
            }
            if flag & Cell::COMMAND != 0 {
                let (command, info) = match (data.get(position), data.get(position + 1)) {
                    (Some(&command), Some(&info)) => (command, info),
                    _ => {
                        return Err(ModuleError::Pattern {
                            index,
                            row,
                            fault: PatternFault::RanOut,
                        })
                    }
                };
                position += 2;
                cell.command = command;
                cell.info = info;
                cell.present |= Cell::COMMAND;
            }

            // One cell per channel per row is what ST3 wrote, and a 32-slot row cannot hold
            // two: the second would silently replace the first.
            if named & (1 << channel) != 0 {
                return Err(ModuleError::Pattern {
                    index,
                    row,
                    fault: PatternFault::ChannelTwice(channel as u8),
                });
            }
            named |= 1 << channel;
            cells[channel] = cell;
            channel_count = channel_count.max(channel as u8 + 1);
        }
    }

    // Every byte of the packed data belongs to a cell: 64 rows of cells and terminators is
    // exactly what the block holds. A decoder that stopped early, or one whose cell sizes
    // are wrong, lands somewhere other than the last byte.
    if position != data.len() {
        return Err(ModuleError::Pattern {
            index,
            row: ROWS - 1,
            fault: PatternFault::Leftover(data.len() - position),
        });
    }

    Ok(Pattern {
        rows,
        channel_count,
    })
}

/// The little-endian word at `at`. Every caller in `load` has checked the file holds the
/// field it is about to read, so a short file is a caller bug and not a data error; the
/// out-of-range panic is the loud version of that bug, and it cannot fire on a file the
/// length checks passed.
fn u16_at(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes(bytes[at..at + 2].try_into().expect("two bytes"))
}

// ---------------------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------------------

/// Why a module could not be decoded.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ModuleError {
    /// The file is shorter than the named structure: `wanted` bytes are needed and the file
    /// holds `at`.
    Truncated {
        what: &'static str,
        at: usize,
        wanted: usize,
    },
    /// The four bytes at the file's offset 44 are not `SCRM` (the bytes found).
    Signature([u8; 4]),
    /// A pattern block did not decode.
    Pattern {
        index: usize,
        row: usize,
        fault: PatternFault,
    },
    /// A PCM instrument's sample runs past the end of the file.
    Sample {
        index: usize,
        at: usize,
        length: u32,
    },
}

/// What was wrong with a pattern block.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PatternFault {
    /// The block at `at` is not inside the file: its length word (which includes its own two
    /// bytes, `STARTEMS.C:348-351`) runs past the end, or the word itself is past it, or it
    /// is under 2 and so describes no data at all.
    BlockRuns { at: usize },
    /// The packed data ended inside a cell, or before the current row's terminator.
    RanOut,
    /// One row named the same channel twice.
    ChannelTwice(u8),
    /// 64 rows were decoded but this many bytes of the block were left over.
    Leftover(usize),
}

impl std::fmt::Display for ModuleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModuleError::Truncated { what, at, wanted } => {
                write!(f, "the file is {at} bytes long, but {what} needs {wanted}")
            }
            ModuleError::Signature(bytes) => write!(
                f,
                "offset 44 holds {bytes:02X?}, not the S3M signature `SCRM`"
            ),
            ModuleError::Pattern { index, row, fault } => {
                write!(f, "pattern {index} row {row}: {fault}")
            }
            ModuleError::Sample { index, at, length } => write!(
                f,
                "instrument {index}'s sample at {at} of {length} bytes runs past the file"
            ),
        }
    }
}

impl std::fmt::Display for PatternFault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PatternFault::BlockRuns { at } => write!(
                f,
                "its block at {at} is not inside the file (or claims no data)"
            ),
            PatternFault::RanOut => write!(f, "the packed data ran out before the row ended"),
            PatternFault::ChannelTwice(channel) => {
                write!(f, "two cells on channel {channel} in one row")
            }
            PatternFault::Leftover(left) => {
                write!(f, "{left} bytes of the block were left over after row 63")
            }
        }
    }
}

impl std::error::Error for ModuleError {}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------------------
    // The keystream, written out here rather than called
    // -----------------------------------------------------------------------------------

    /// D5's keystream, written out in the test from the formula as D5 states it — the same
    /// arithmetic [`key`] performs, but not the same code, so a transcription slip in
    /// either one shows up as a disagreement.
    fn keystream(len: usize) -> Vec<u8> {
        (0..len)
            .map(|i| {
                let n = (i >> 1) + 1;
                ((((n ^ (n >> 2)) << 3) | ((5 * i + 2) & 7)) & 0xFF) as u8
            })
            .collect()
    }

    #[test]
    fn the_keystream_is_d5s() {
        // The formula's first sixteen bytes, evaluated from D5's text when this test was
        // written. `keystream` recomputes them with this file's own arithmetic, and the two
        // endpoints are `key`'s — the crate's own — so a transcription slip in either `key`
        // or the rewritten formula shows up here, and a failure message carries both values.
        let expected = [
            10, 15, 20, 17, 30, 27, 40, 45, 34, 39, 60, 57, 54, 51, 80, 85,
        ];
        assert_eq!(
            keystream(16),
            expected,
            "the keystream's first sixteen bytes"
        );
        assert_eq!(key(0), 10);
        assert_eq!(key(15), 85);
    }

    #[test]
    fn descramble_restarts_at_index_zero_for_every_pattern() {
        let plain: Vec<u8> = (0..4096u32).map(|i| (i * 7 + 3) as u8).collect();
        let mut scrambled = plain.clone();
        descramble(&mut scrambled);
        let want: Vec<u8> = plain
            .iter()
            .zip(keystream(plain.len()))
            .map(|(byte, key)| byte ^ key)
            .collect();
        assert_eq!(scrambled, want, "the scrambled bytes are the plain ^ key");

        // Applying it twice is the identity — the keystream has no state between calls, so
        // a second pattern's descrambling starts again at index 0.
        descramble(&mut scrambled);
        assert_eq!(scrambled, plain);
    }

    // -----------------------------------------------------------------------------------
    // A synthetic module, so the awkward cases can be built instead of waited for
    // -----------------------------------------------------------------------------------

    /// Build a module the way the files are built: a header, an order list, the two pointer
    /// tables, then blocks on paragraph boundaries. What goes *in* the blocks is the test's.
    struct Builder {
        bytes: Vec<u8>,
        instruments_at: usize,
        patterns_at: usize,
    }

    impl Builder {
        fn new(orders: &[u8], instruments: usize, patterns: usize) -> Builder {
            let mut bytes = vec![0u8; HEADER_SIZE];
            bytes[28] = 26; // the marker byte both modules carry
            bytes[29] = 16; // the type byte both modules carry
            bytes[32..34].copy_from_slice(&(orders.len() as u16).to_le_bytes());
            bytes[34..36].copy_from_slice(&(instruments as u16).to_le_bytes());
            bytes[36..38].copy_from_slice(&(patterns as u16).to_le_bytes());
            bytes[44..48].copy_from_slice(SIGNATURE);
            bytes[48] = 64; // global volume
            bytes[49] = 6; // initial speed
            bytes[50] = 125; // initial tempo
            bytes[51] = 176; // master volume, stereo bit set
            bytes[64..96].fill(255);
            bytes.extend_from_slice(orders);
            let instruments_at = bytes.len();
            let patterns_at = instruments_at + 2 * instruments;
            bytes.resize(patterns_at + 2 * patterns, 0);
            Builder {
                bytes,
                instruments_at,
                patterns_at,
            }
        }

        /// Append `block` at the next paragraph boundary and return its paragraph.
        fn append(&mut self, block: &[u8]) -> u16 {
            let pad = (16 - self.bytes.len() % 16) % 16;
            self.bytes.extend(std::iter::repeat_n(0, pad));
            let paragraph = (self.bytes.len() / 16) as u16;
            self.bytes.extend_from_slice(block);
            paragraph
        }

        /// Append a pattern block: a length word that includes itself, then scrambled data.
        fn pattern(&mut self, data: &[u8]) -> u16 {
            let mut payload = data.to_vec();
            descramble(&mut payload);
            let mut block = ((data.len() + 2) as u16).to_le_bytes().to_vec();
            block.extend_from_slice(&payload);
            self.append(&block)
        }

        fn instrument(&mut self, header: [u8; INSTRUMENT_SIZE]) -> u16 {
            self.append(&header)
        }

        /// Fill with zeroes up to `offset`, so the next thing lands exactly there.
        fn reserve_to(&mut self, offset: usize) {
            if self.bytes.len() < offset {
                self.bytes.resize(offset, 0);
            }
        }

        fn finish(mut self, instrument_pointers: &[u16], pattern_pointers: &[u16]) -> Vec<u8> {
            for (i, pointer) in instrument_pointers.iter().enumerate() {
                self.bytes[self.instruments_at + 2 * i..self.instruments_at + 2 * i + 2]
                    .copy_from_slice(&pointer.to_le_bytes());
            }
            for (i, pointer) in pattern_pointers.iter().enumerate() {
                self.bytes[self.patterns_at + 2 * i..self.patterns_at + 2 * i + 2]
                    .copy_from_slice(&pointer.to_le_bytes());
            }
            self.bytes
        }
    }

    /// One row's worth of packed data, then 63 terminators: a whole pattern of 64 rows. The
    /// cells' row needs a terminator of its own, so the data is the cells plus one byte per
    /// row.
    fn one_row(cells: &[u8]) -> Vec<u8> {
        let mut data = cells.to_vec();
        data.resize(cells.len() + ROWS, 0);
        data
    }

    #[test]
    fn a_cell_carries_exactly_what_its_flag_says() {
        // Channel 3, every field group at once: note and instrument, volume, command, info.
        // One flag byte carries all three groups (bit 5 | bit 6 | bit 7) and the channel is
        // its low five bits — 0xE3.
        let data = one_row(&[0xE3, 0x31, 0x05, 0x40, 0x0F, 0x21]);
        let pattern = decode_pattern(0, &data).expect("decodes");
        let cell = pattern.rows[0][3];
        assert_eq!(cell.note, 0x31);
        assert_eq!(cell.instrument, 0x05);
        assert_eq!(cell.volume, 0x40);
        assert_eq!(cell.command, 0x0F);
        assert_eq!(cell.info, 0x21);
        assert_eq!(
            cell.present,
            Cell::NOTE | Cell::VOLUME | Cell::COMMAND,
            "every group is marked present"
        );
        assert!(cell.has_note() && cell.has_volume() && cell.has_command());
        assert_eq!(pattern.channel_count, 4, "channel 3 is the highest named");
        assert_eq!(pattern.rows.len(), ROWS);
    }

    #[test]
    fn a_cells_groups_are_read_in_the_files_own_order() {
        // The groups are stored note, instrument, volume, command, info — the order the flag
        // bits say. Reading the volume before the note's two bytes shifts every later byte.
        let data = one_row(&[0xE1, 0x2A, 0x07, 0x20, 0x0C, 0x11]);
        let pattern = decode_pattern(0, &data).expect("decodes");
        let cell = pattern.rows[0][1];
        assert_eq!(
            (
                cell.note,
                cell.instrument,
                cell.volume,
                cell.command,
                cell.info
            ),
            (0x2A, 0x07, 0x20, 0x0C, 0x11)
        );
    }

    #[test]
    fn a_channel_zero_cell_is_not_a_row_terminator() {
        // The flag byte's low five bits are the channel, so 0x20 is channel 0 with a note
        // and an instrument — not "end of row". A decoder testing `flag & 0x1f == 0` would
        // drop this cell and read the two bytes after it as a new row's flag bytes.
        let data = one_row(&[0x20, 0x10, 0x02]);
        let pattern = decode_pattern(0, &data).expect("decodes");
        assert_eq!(pattern.rows[0][0].note, 0x10);
        assert_eq!(pattern.rows[0][0].instrument, 0x02);
        assert_eq!(pattern.channel_count, 1);
        assert!(pattern.rows[0][1..].iter().all(Cell::is_empty));
        assert!(pattern.rows[1..].iter().flatten().all(Cell::is_empty));
    }

    #[test]
    fn a_volume_of_zero_is_a_cell_and_not_an_absent_one() {
        let data = one_row(&[0x42, 0x00]);
        let pattern = decode_pattern(0, &data).expect("decodes");
        let cell = pattern.rows[0][2];
        assert_eq!(pattern.channel_count, 3);
        assert_eq!(cell.volume, 0);
        assert!(cell.has_volume(), "the file carried a volume column");
        assert!(
            !cell.is_empty(),
            "and it is a named channel slot, not an unnamed one"
        );
        assert_eq!(
            Cell::EMPTY.volume,
            0,
            "which its five values alone cannot say"
        );
    }

    #[test]
    fn an_empty_row_makes_an_empty_pattern() {
        let pattern = decode_pattern(0, &one_row(&[])).expect("decodes");
        assert!(pattern.rows.iter().flatten().all(Cell::is_empty));
        assert_eq!(pattern.channel_count, 0);
    }

    #[test]
    fn a_cell_that_carries_no_bytes_at_all_is_rejected() {
        // Bit 5 promises two bytes; the row's terminator is read as them, and the stream
        // then ends inside a later row. The fault is the point: the packed data ran out.
        let data = one_row(&[0x20]);
        match decode_pattern(0, &data) {
            Err(ModuleError::Pattern {
                fault: PatternFault::RanOut,
                ..
            }) => {}
            other => panic!("expected a run-out, got {other:?}"),
        }
    }

    #[test]
    fn a_row_that_names_one_channel_twice_is_rejected() {
        let data = one_row(&[0x21, 0x10, 0x01, 0x21, 0x20, 0x02]);
        assert_eq!(
            decode_pattern(0, &data),
            Err(ModuleError::Pattern {
                index: 0,
                row: 0,
                fault: PatternFault::ChannelTwice(1)
            })
        );
    }

    #[test]
    fn leftover_bytes_after_row_sixty_three_are_rejected() {
        let mut data = one_row(&[0x21, 0x10, 0x01]);
        data.push(0x00); // a 65th row terminator the format has no room for
        assert_eq!(
            decode_pattern(0, &data),
            Err(ModuleError::Pattern {
                index: 0,
                row: ROWS - 1,
                fault: PatternFault::Leftover(1)
            })
        );
    }

    #[test]
    fn data_that_runs_out_before_the_last_row_is_rejected() {
        let rows = one_row(&[0x21, 0x10, 0x01]);
        let data = rows[..rows.len() - 1].to_vec();
        assert_eq!(
            decode_pattern(0, &data),
            Err(ModuleError::Pattern {
                index: 0,
                row: ROWS - 1,
                fault: PatternFault::RanOut
            })
        );
    }

    #[test]
    fn a_short_file_is_refused_by_name() {
        assert_eq!(
            Module::load(&[0u8; 10]),
            Err(ModuleError::Truncated {
                what: "the file header",
                at: 10,
                wanted: HEADER_SIZE
            })
        );
    }

    #[test]
    fn a_file_whose_tables_do_not_fit_is_refused_by_name() {
        // A complete header claiming 80 orders and 77 patterns in an empty file.
        let mut bytes = vec![0u8; HEADER_SIZE];
        bytes[44..48].copy_from_slice(SIGNATURE);
        bytes[32..34].copy_from_slice(&80u16.to_le_bytes());
        bytes[36..38].copy_from_slice(&77u16.to_le_bytes());
        assert_eq!(
            Module::load(&bytes),
            Err(ModuleError::Truncated {
                what: "the order and pointer tables",
                at: HEADER_SIZE,
                wanted: 0x60 + 80 + 2 * 77
            })
        );
    }

    #[test]
    fn a_file_without_the_signature_is_refused_by_name() {
        let bytes = Builder::new(&[7, 9, 11], 0, 0).finish(&[], &[]);
        assert_eq!(Module::load(&bytes).expect("loads").order_count(), 3);

        let mut not_an_s3m = bytes.clone();
        not_an_s3m[44..48].copy_from_slice(b"SCRX");
        assert_eq!(
            Module::load(&not_an_s3m),
            Err(ModuleError::Signature(*b"SCRX"))
        );
    }

    #[test]
    fn the_headers_own_bytes_are_read_at_their_own_offsets() {
        let bytes = Builder::new(&[7, 9, 11], 0, 0).finish(&[], &[]);
        let module = Module::load(&bytes).expect("loads");
        assert_eq!(module.orders(), &[7, 9, 11]);
        assert_eq!(module.order_count(), 3);
        assert_eq!(module.pattern_count(), 0);
        assert_eq!(module.instrument_count(), 0);
        assert_eq!(module.global_volume(), 64);
        assert_eq!(module.initial_speed(), 6);
        assert_eq!(module.initial_tempo(), 125);
        assert_eq!(module.master_volume(), 176);
        assert!(module.stereo());
        assert_eq!(module.channel_pan()[0], 255);
        assert_eq!(module.channel_pan().len(), CHANNELS);
    }

    #[test]
    fn a_sample_headers_fields_are_read_at_their_own_offsets() {
        // Distinguishable bytes in every field the census reads, so a reader four bytes off
        // — the `flags`/`c2spd` trap — cannot pass. None of these is a filed module's value.
        let mut header = [0u8; INSTRUMENT_SIZE];
        header[0x00] = 1; // type
        header[0x01..0x0D].copy_from_slice(b"FILE.11\0\0\0\0\0");
        header[0x0D] = 0x00; // memseg2
        header[0x0E..0x10].copy_from_slice(&0x0100u16.to_le_bytes()); // memseg
        header[0x10..0x14].copy_from_slice(&0x0000_0040u32.to_le_bytes()); // length
        header[0x14..0x18].copy_from_slice(&0x0000_0008u32.to_le_bytes()); // loop begin
        header[0x18..0x1C].copy_from_slice(&0x0000_0020u32.to_le_bytes()); // loop end
        header[0x1C] = 0x11; // volume
        header[0x1D] = 0x22; // disk
        header[0x1E] = 0x33; // pack
        header[0x1F] = 0x01; // flags
        header[0x20..0x24].copy_from_slice(&0x2000u32.to_le_bytes()); // c2spd
        header[0x30..0x4C].copy_from_slice(b"Name of the sample_________.");
        header[0x4C..0x50].copy_from_slice(SAMPLE_MAGIC);

        // The sample sits at memseg << 4 = 0x1000, past the tables and the header block.
        let at = 0x0100usize << 4;
        let mut builder = Builder::new(&[0], 1, 0);
        let pointer = builder.instrument(header);
        builder.reserve_to(at + 0x40);
        let mut bytes = builder.finish(&[pointer], &[]);
        for (i, byte) in bytes[at..at + 0x40].iter_mut().enumerate() {
            *byte = i as u8;
        }

        let module = Module::load(&bytes).expect("loads");
        let instrument = module.instrument(0).expect("one instrument");
        assert_eq!(instrument.sample_type, 1);
        assert!(instrument.is_pcm() && !instrument.is_empty());
        assert_eq!(&instrument.filename, b"FILE.11\0\0\0\0\0");
        assert_eq!(instrument.memseg2, 0x00);
        assert_eq!(instrument.memseg, 0x0100);
        assert_eq!(instrument.length, 0x40);
        assert_eq!(instrument.loop_begin, 8);
        assert_eq!(instrument.loop_end, 0x20);
        assert_eq!(instrument.volume, 0x11);
        assert_eq!(instrument.disk, 0x22);
        assert_eq!(instrument.pack, 0x33);
        assert_eq!(instrument.flags, 0x01);
        assert!(instrument.loops());
        assert_eq!(instrument.c2spd, 0x2000);
        assert_eq!(&instrument.name, b"Name of the sample_________.");
        assert_eq!(instrument.sample_offset(), at);
        let sample = module.sample_bytes(0).expect("a sample");
        assert_eq!(sample.len(), 0x40);
        assert_eq!(sample[0], 0);
        assert_eq!(sample[0x3F], 0x3F, "sliced from the pointer, not from 0");

        // The paragraph pointer's high byte is the file's own second word
        // (`STARTEMS.C:385-386`), so it is part of the offset even where both modules
        // happen to carry zero.
        let mut high = instrument.clone();
        high.memseg = 0x0002;
        high.memseg2 = 0x03;
        assert_eq!(high.sample_offset(), 0x30_0020);
    }

    #[test]
    fn an_empty_instrument_has_no_sample_to_slice() {
        let header = [0u8; INSTRUMENT_SIZE]; // type 0, length 0
        let mut builder = Builder::new(&[0], 1, 0);
        let pointer = builder.instrument(header);
        let bytes = builder.finish(&[pointer], &[]);
        let module = Module::load(&bytes).expect("loads");
        assert!(module.instrument(0).expect("one").is_empty());
        assert_eq!(module.sample_bytes(0), None);
        assert_eq!(module.sample_bytes(1), None, "and past the table's end");
    }

    #[test]
    fn a_pcm_instrument_of_length_zero_has_no_sample_to_slice() {
        // `checkins` treats a length-0 sample as no sample (`STARTEMS.C:135-139`).
        let mut header = [0u8; INSTRUMENT_SIZE];
        header[0x00] = 1;
        let mut builder = Builder::new(&[0], 1, 0);
        let pointer = builder.instrument(header);
        let bytes = builder.finish(&[pointer], &[]);
        let module = Module::load(&bytes).expect("loads");
        assert!(module.instrument(0).expect("one").is_pcm());
        assert_eq!(module.sample_bytes(0), None);
    }

    #[test]
    fn a_pattern_pointer_of_zero_is_an_empty_pattern() {
        // `STARTEMS.C:346`, `if(!b) continue;` — the original leaves such a pattern's
        // paragraph entry at zero and its player sees an empty pattern, not an error.
        let mut builder = Builder::new(&[0], 0, 2);
        let real = builder.pattern(&one_row(&[0x21, 0x40, 0x00]));
        let bytes = builder.finish(&[], &[0, real]);
        let module = Module::load(&bytes).expect("loads");
        assert_eq!(module.pattern_count(), 2);

        let empty = module.pattern(0).expect("the zero-pointer pattern");
        assert_eq!(empty.rows.len(), ROWS);
        assert_eq!(empty.channel_count, 0);
        assert!(empty.rows.iter().flatten().all(Cell::is_empty));

        let real = module.pattern(1).expect("the real block");
        assert_eq!(real.rows.len(), ROWS);
        assert_eq!(real.channel_count, 2);
        assert_eq!(real.rows[0][1].note, 0x40);
        assert_eq!(
            real.rows[0][1].instrument, 0x00,
            "the file's instrument byte is kept as it is, 1-based and 0 for none"
        );
    }

    #[test]
    fn a_pattern_block_past_the_files_end_is_refused_by_name() {
        let mut builder = Builder::new(&[0], 0, 1);
        let _ = builder.pattern(&one_row(&[]));
        let mut bytes = builder.finish(&[], &[1]);
        // Claim a block far longer than the file: 0xFFF0 bytes from paragraph 1.
        bytes[16..18].copy_from_slice(&0xFFF0u16.to_le_bytes());
        assert_eq!(
            Module::load(&bytes),
            Err(ModuleError::Pattern {
                index: 0,
                row: 0,
                fault: PatternFault::BlockRuns { at: 16 }
            })
        );
    }

    #[test]
    fn a_sample_past_the_files_end_is_refused_by_name() {
        let mut header = [0u8; INSTRUMENT_SIZE];
        header[0x00] = 1;
        header[0x0E..0x10].copy_from_slice(&0x4000u16.to_le_bytes()); // 0x4000 << 4 = 0x40000
        header[0x10..0x14].copy_from_slice(&0x100u32.to_le_bytes());
        let mut builder = Builder::new(&[0], 1, 0);
        let pointer = builder.instrument(header);
        let bytes = builder.finish(&[pointer], &[]);
        assert_eq!(
            Module::load(&bytes),
            Err(ModuleError::Sample {
                index: 0,
                at: 0x40000,
                length: 0x100
            })
        );
    }

    #[test]
    fn the_tempo_patch_changes_the_initial_tempo_and_nothing_else() {
        let bytes = Builder::new(&[0], 0, 0).finish(&[], &[]);
        let mut module = Module::load(&bytes).expect("loads");
        assert_eq!(module.initial_tempo(), 125);
        module.apply_tempo_patch(0x78);
        assert_eq!(module.initial_tempo(), 120);
        assert_eq!(module.initial_speed(), 6, "the patch moves one byte only");
        assert_eq!(module.global_volume(), 64);
    }
}
