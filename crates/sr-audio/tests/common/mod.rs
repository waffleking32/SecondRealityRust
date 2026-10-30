//! What sr-audio's integration tests share: the shipped modules as the shell loads them, the
//! drain, and an in-memory S3M builder for the cells the shipped data does not carry.
//! [`oracle`] holds the object's tables and arithmetic, written out independently of the port.

#![allow(dead_code)]

pub mod oracle;

use std::path::PathBuf;

use sr_audio::{descramble, Cell, Module, Player, PlayerOptions, Song, ROWS};

/// The workspace root, which the shipped modules and `reference/` are read under.
pub fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn song(name: &str) -> &'static Song {
    Song::named(name).unwrap_or_else(|| panic!("{name} is not a song"))
}

/// The module as the shell loads it: [`Song::load`], MUSIC0 patched to tempo 120
/// (`MAIN/STARTMUS.C:252-265`).
pub fn module(name: &str) -> Module {
    song(name)
        .load(&root())
        .unwrap_or_else(|error| panic!("{error}"))
}

/// A fresh player of [`module`] with the default (authentic) options.
pub fn player(name: &str) -> Player {
    Player::new(module(name), PlayerOptions::default())
}

/// The song's file as shipped: scrambled, and with no tempo patch.
pub fn shipped(name: &str) -> Vec<u8> {
    let path = root().join(song(name).file);
    std::fs::read(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

/// Everything the ring holds, as a device would take it.
pub fn drain(player: &mut Player) -> Vec<i16> {
    let mut out = vec![0i16; player.pending_samples()];
    let taken = player.take_samples(&mut out);
    assert_eq!(taken, out.len(), "the drain takes what the ring holds");
    out
}

/// One cell of a built module's row.
#[derive(Clone, Copy)]
pub struct Note {
    channel: u8,
    note: u8,
    instrument: u8,
    volume: u8,
    command: u8,
    info: u8,
    flags: u8,
}

impl Note {
    /// A note with an instrument and a volume column.
    pub fn new(channel: u8, note: u8, instrument: u8, volume: u8) -> Note {
        Note {
            channel,
            note,
            instrument,
            volume,
            command: 0,
            info: 0,
            flags: Cell::NOTE | Cell::VOLUME,
        }
    }

    /// A command with no note and no volume column.
    pub fn command(channel: u8, command: u8, info: u8) -> Note {
        Note {
            channel,
            note: 0xff,
            instrument: 0,
            volume: 0xff,
            command,
            info,
            flags: Cell::COMMAND,
        }
    }

    /// The cell packed as the format packs it: the flag byte (channel in the low five bits),
    /// then the note and instrument, the volume, the command and info its bits announce.
    fn pack(&self, out: &mut Vec<u8>) {
        out.push(self.channel | self.flags);
        if self.flags & Cell::NOTE != 0 {
            out.extend([self.note, self.instrument]);
        }
        if self.flags & Cell::VOLUME != 0 {
            out.push(self.volume);
        }
        if self.flags & Cell::COMMAND != 0 {
            out.extend([self.command, self.info]);
        }
    }
}

/// A one-order, one-pattern module with its own instruments, built row by row.
pub struct Builder {
    rows: Vec<Vec<Note>>,
    instruments: Vec<(u32, u8)>,
}

impl Builder {
    pub fn new() -> Builder {
        Builder {
            rows: Vec::new(),
            instruments: Vec::new(),
        }
    }

    /// Add a PCM instrument of length 0 at `c2spd` and `volume`.
    pub fn instrument(&mut self, c2spd: u32, volume: u8) {
        self.instruments.push((c2spd, volume));
    }

    /// Append a row; the call order is the row order.
    pub fn row(&mut self, notes: &[Note]) {
        self.rows.push(notes.to_vec());
    }

    pub fn build(self) -> Module {
        s3m(&[0], &self.instruments, &[self.rows])
    }
}

/// A module of `orders` over patterns whose row 0 is `rows[i]` and whose other rows are empty,
/// with no instruments.
pub fn first_rows(orders: &[u8], rows: &[&[Note]]) -> Module {
    let patterns: Vec<Vec<Vec<Note>>> = rows.iter().map(|row| vec![row.to_vec()]).collect();
    s3m(orders, &[], &patterns)
}

/// An S3M in memory: the 0x60-byte header (speed 6, tempo 125, global volume 64, master
/// volume 0xB0, no channel table), the order list, the instrument and pattern pointer tables,
/// then paragraph-aligned instrument headers and pattern blocks, the pattern data scrambled by
/// the keystream the loader undoes.
fn s3m(orders: &[u8], instruments: &[(u32, u8)], patterns: &[Vec<Vec<Note>>]) -> Module {
    let mut bytes = vec![0u8; 0x60];
    bytes[28] = 26;
    bytes[29] = 16;
    bytes[32..34].copy_from_slice(&(orders.len() as u16).to_le_bytes());
    bytes[34..36].copy_from_slice(&(instruments.len() as u16).to_le_bytes());
    bytes[36..38].copy_from_slice(&(patterns.len() as u16).to_le_bytes());
    bytes[44..48].copy_from_slice(b"SCRM");
    bytes[48] = 64;
    bytes[49] = 6;
    bytes[50] = 125;
    bytes[51] = 176;
    bytes[64..96].fill(255);
    bytes.extend_from_slice(orders);
    let instruments_at = bytes.len();
    let patterns_at = instruments_at + 2 * instruments.len();
    bytes.resize(patterns_at + 2 * patterns.len(), 0);

    let paragraph = |bytes: &mut Vec<u8>| {
        let pad = (16 - bytes.len() % 16) % 16;
        bytes.extend(std::iter::repeat_n(0u8, pad));
        (bytes.len() / 16) as u16
    };
    let mut pointers = Vec::new();
    for (c2spd, volume) in instruments {
        let mut header = [0u8; 0x50];
        header[0] = 1;
        header[0x1C] = *volume;
        header[0x20..0x24].copy_from_slice(&c2spd.to_le_bytes());
        header[0x4C..0x50].copy_from_slice(b"SCRS");
        pointers.push(paragraph(&mut bytes));
        bytes.extend_from_slice(&header);
    }
    for rows in patterns {
        let mut data = Vec::new();
        for row in rows {
            for note in row {
                note.pack(&mut data);
            }
            data.push(0);
        }
        data.resize(data.len() + ROWS - rows.len(), 0);
        descramble(&mut data);
        pointers.push(paragraph(&mut bytes));
        bytes.extend_from_slice(&((data.len() + 2) as u16).to_le_bytes());
        bytes.extend_from_slice(&data);
    }
    for (index, pointer) in pointers.iter().enumerate() {
        let at = instruments_at + 2 * index;
        bytes[at..at + 2].copy_from_slice(&pointer.to_le_bytes());
    }
    Module::load(&bytes).expect("the built module loads")
}
