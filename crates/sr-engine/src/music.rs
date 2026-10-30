//! The music seam: what a part is handed in place of the Demo Interrupt Server's music calls.
//!
//! The parts call into the DIS (`DIS/DIS.H:37-59`), whose music entries read the words the music
//! engine keeps in its own segment: `_np_zinfo`, `_np_row`, `_np_ord`, `_np_zplus`, `_np_zframe`
//! (`MAIN/STMIK.H:125-131`). `muscode_6` (`DIS/DISINT.ASM:242-282`) is three calls read back three
//! ways by its wrappers (`DIS/DISC.ASM:91-123`); `musframe_9` (`:412-425`) reads and writes
//! `_np_zframe`; `sync_10` (`:449-463`) looks the position up in `ordersync1` (`:428-446`).
//!
//! The engine's only act on a [`Music`] is [`Music::poll`], once a tick. The implementation over
//! `sr_audio::Player` is the binary's (`audio::PlayerMusic`), since only the binary depends on
//! both crates. The DIS's own arithmetic ([`musplus_from`], [`muscode_from`], [`sync_from`]) is
//! here, on the parts' side of the seam.

/// The "no plus pending" sentinel `muscode_6` starts `%dx` with (`DIS/DISINT.ASM:252`).
pub(crate) const NO_PLUS: i32 = -32;

/// The published music position: the words the DIS hands the parts.
///
/// `order`, `pattern` and `row` are the ring-published words (`MAIN/STMIK.H:126-128`), ten polls
/// behind the player's own state; `speed` and `tempo` are written straight through by an `A` or
/// `T` command (`docs/fidelity/music.md` §8).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct MusicPosition {
    /// `_np_ord`.
    pub order: u8,
    /// `_np_pat`.
    pub pattern: u8,
    /// `_np_row`.
    pub row: u8,
    /// `_np_speed`.
    pub speed: u8,
    /// `_np_tempo`.
    pub tempo: u8,
}

/// The music as a part sees it: the six calls of `DIS/DIS.H:37-59`, plus the two the engine
/// itself needs. The shell's own uses of the player (fades, restarts) are not here.
pub trait Music {
    /// One logic tick's worth of music: the copper's `call _zpollme` (`MAIN/COPPER.ASM:466`).
    fn poll(&mut self);

    /// The published position; see [`MusicPosition`].
    fn position(&self) -> MusicPosition;

    /// `dis_musplus()` (`DIS/DISC.ASM:102-112`): [`musplus_from`] over the published words.
    fn musplus(&mut self) -> i32;

    /// `dis_musrow()` (`DIS/DISC.ASM:114-123`): `_np_row`, the published row.
    fn musrow(&mut self) -> i32;

    /// `dis_muscode(code)` (`DIS/DISC.ASM:91-100`): [`muscode_from`].
    fn muscode(&mut self, wanted: i32) -> i32;

    /// `dis_sync()` (`DIS/DISC.ASM:161-169`): [`sync_from`] over the published position.
    fn sync(&mut self) -> i32;

    /// `dis_getmframe()` (`DIS/DISC.ASM:150-159`): `_np_zframe`, the *tick* counter
    /// (`docs/fidelity/music-clock.md` §8).
    fn getmframe(&mut self) -> i32;

    /// `dis_setmframe(frame)` (`DIS/DISC.ASM:138-148`).
    fn setmframe(&mut self, frame: i32);
}

/// A borrow of the engine's `Box<dyn Music>`. The `'static` object bound is spelled out because
/// a `&mut` trait object is invariant in it: `&'a mut dyn Music` would mean `dyn Music + 'a`,
/// which cannot be reborrowed out of the box.
pub type MusicRef<'a> = &'a mut (dyn Music + 'static);

/// The null readings: what a part would meet with no player at all, for a test that wants a
/// part handed something rather than `None`.
///
/// A zero position, the `-32` sentinel, row 0, `muscode` 0, a frame counter that reads 0 and
/// ignores writes, and a [`sync`](Music::sync) that panics: there are no `_np_ord`/`_np_row`
/// words to look up (`DIS/DISINT.ASM:449-463`), so a part that waits on a start code fails where
/// it can be seen.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoMusic;

impl Music for NoMusic {
    fn poll(&mut self) {}

    fn position(&self) -> MusicPosition {
        MusicPosition::default()
    }

    fn musplus(&mut self) -> i32 {
        musplus_from(0, 0)
    }

    fn musrow(&mut self) -> i32 {
        0
    }

    fn muscode(&mut self, wanted: i32) -> i32 {
        muscode_from(MusicPosition::default(), wanted)
    }

    fn sync(&mut self) -> i32 {
        panic!(
            "NoMusic::sync(): there is no player, so no `_np_ord`/`_np_row` for `sync_10` \
             (`DIS/DISINT.ASM:449-463`) to look up"
        )
    }

    fn getmframe(&mut self) -> i32 {
        0
    }

    fn setmframe(&mut self, _frame: i32) {}
}

/// `dis_musplus()`'s answer, from the published `_np_zplus` and `_np_row`
/// (`DIS/DISINT.ASM:252-271`).
///
/// ```text
///   cmp es:_np_zplus,0 ; je @@1a        ; 0  -> the sentinel, done
///   cmp es:_np_zplus,1 ; je @@1b        ; 1  -> the "plus coming" form
///   cmp es:_np_zplus,2 ; je @@1e        ; 2  -> the plain-row form
/// @@1eb: cmp bx,32     ; ja  @@1b        ; any other value: rows above 32 go to @@1b (unsigned)
/// @@1e:  mov dx,bx     ; cmp dx,32; jb @@1a ; the row while it is below 32 …
///                       mov dx,-32           ; … and the sentinel from 32 on
/// @@1b:  mov dx,bx     ; sub dx,64; cmp dx,-32; jge @@1a ; the row less 64, clamped up at -32
///                       mov dx,-32                        ; (signed compare)
/// ```
///
/// `zplus` 3 is the only other value the two-bit word holds (`docs/fidelity/music-clock.md` §6).
/// The parts read the answer as a countdown to the marker (`GLENZ/MAIN.C:290`, `TECHNO/KOE.C:213`,
/// `PLZPART/PLZ.C:68`, `JPLOGO/JP.C:71`, …; `docs/fidelity/music.md` §11).
pub fn musplus_from(zplus: i32, row: i32) -> i32 {
    match zplus {
        0 => NO_PLUS,
        1 => (row - 64).max(NO_PLUS),
        2 => {
            if row < 32 {
                row
            } else {
                NO_PLUS
            }
        }
        // `@@1eb`'s unsigned `ja` sends rows above 32 to the row-64 form, the rest through `@@1e`.
        _ => {
            if row > 32 {
                (row - 64).max(NO_PLUS)
            } else if row < 32 {
                row
            } else {
                NO_PLUS
            }
        }
    }
}

/// `dis_muscode()`'s answer: `_np_zinfo` (`DIS/DISINT.ASM:249`), which is 0 in every sample of
/// every filed trace (D8, `docs/fidelity/music-clock.md` §6). The INDEMO build ignores `wanted`.
pub fn muscode_from(_position: MusicPosition, _wanted: i32) -> i32 {
    0
}

/// The `ordersync1` table (`DIS/DISINT.ASM:428-446`), key then code, in source order. A key is
/// `(order << 8) | row`, the word `sync_10` builds from `_np_ord` and `_np_row` (`:452-453`).
pub const ORDERSYNC1: &[(u16, i32)] = &[
    (0x0000, 0),
    (0x0200, 1),
    (0x0300, 2),
    (0x032f, 3),
    (0x042f, 4),
    (0x052f, 5),
    (0x062f, 6),
    (0x072f, 7),
    (0x082f, 8),
    (0x0900, 9),
    (0x0d00, 10),
    (0x3d00, 1),
    (0x3f00, 2),
    (0x4100, 3),
    (0x4200, 4),
];

/// The word before `ordersync1`: `musframe_9`'s closing `pop es; ret` (`DIS/DISINT.ASM:420`,
/// `:424`), `07 c3`, which a C caller reads as the `int` -15609.
const BEFORE_THE_TABLE: i32 = i16::from_le_bytes([0x07, 0xc3]) as i32;

/// The word after `ordersync1`: `2e 8e`, the first two bytes of `sync_10`'s
/// `mov es, cs:stmikseg` (`DIS/DISINT.ASM:451`), the same in every build.
const PAST_THE_TABLE: u16 = u16::from_le_bytes([0x2e, 0x8e]);

/// `dis_sync()`'s answer for a published position: `sync_10` (`DIS/DISINT.ASM:449-463`).
///
/// The loop runs sixteen times over fifteen entries (`:455`), stops at the first key at or above
/// the position (unsigned `jbe`, `:456-457`), and reads the answer at `%bx - 2` (`:460`): the code
/// of the entry *before* the match. So `(2, 0)` reads 0 and `(2, 1)` reads 1. At `(0, 0)` the read
/// lands before the table ([`BEFORE_THE_TABLE`]). Above the last key the 16th compare is against
/// [`PAST_THE_TABLE`]: up to `0x8E2E` it matches and the answer is the last code, 4. See
/// `docs/fidelity/music.md` §11 and A15.
///
/// # Panics
///
/// Above `0x8E2E`, where the fall-through returns a linker-set word that differs between builds
/// of the original.
pub fn sync_from(order: u8, row: u8) -> i32 {
    let key = (u16::from(order) << 8) | u16::from(row);
    if key == 0 {
        return BEFORE_THE_TABLE;
    }
    if let Some(index) = ORDERSYNC1.iter().position(|&(key_at, _)| key <= key_at) {
        return ORDERSYNC1[index - 1].1;
    }
    if key <= PAST_THE_TABLE {
        return ORDERSYNC1[ORDERSYNC1.len() - 1].1;
    }
    panic!(
        "dis_sync() at (order {order}, row {row}): the position is above the word `sync_10`'s 16th \
         iteration compares against (`0x8E2E`), and the word it returns differs between builds of \
         the original (0xCD06 in MAIN/U2.EXE, 0xED06 in MAIN/2NDFIX.EXE, 0x9D06 in U2.EXE); \
         see `docs/fidelity/music.md` §11 and A15"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every `zplus` against the rows where `DIS/DISINT.ASM:252-271` changes its answer.
    const MUSPLUS_CASES: &[(i32, i32, i32)] = &[
        (0, 0, -32),
        (0, 31, -32),
        (0, 32, -32),
        (0, 33, -32),
        (0, 64, -32),
        (0, 255, -32),
        // zplus 1: `row - 64`, clamped up at -32.
        (1, 0, -32),
        (1, 31, -32),
        (1, 32, -32),
        (1, 33, -31),
        (1, 63, -1),
        (1, 64, 0),
        (1, 95, 31),
        (1, 96, 32),
        (1, 255, 191),
        // zplus 2: the row under 32, the sentinel from 32 on.
        (2, 0, 0),
        (2, 31, 31),
        (2, 32, -32),
        (2, 33, -32),
        (2, 64, -32),
        (2, 255, -32),
        // zplus 3: `@@1eb`.
        (3, 0, 0),
        (3, 31, 31),
        (3, 32, -32),
        (3, 33, -31),
        (3, 64, 0),
        (3, 95, 31),
        (3, 96, 32),
        (3, 255, 191),
    ];

    #[test]
    fn musplus_is_the_objects_own_two_phase_rule() {
        for &(zplus, row, expected) in MUSPLUS_CASES {
            assert_eq!(musplus_from(zplus, row), expected, "zplus {zplus}, row {row}");
        }
    }

    /// One case per boundary of the table: each key at itself (the previous code) and one row
    /// later (its own code); `(0, 0)` reads the word before the table and the last three rows the
    /// band above it, up to the 16th compare's own word.
    const SYNC_CASES: &[(u8, u8, i32)] = &[
        (0, 0, -15609),
        (0, 1, 0),
        (1, 0, 0),
        (2, 0, 0),
        (2, 1, 1),
        (3, 0, 1),
        (3, 1, 2),
        (3, 0x2f, 2),
        (3, 0x30, 3),
        (4, 0x2f, 3),
        (4, 0x30, 4),
        (8, 0x2f, 7),
        (9, 0, 8),
        (9, 1, 9),
        (13, 0, 9),
        (13, 1, 10),
        (61, 0, 10),
        (61, 1, 1),
        (63, 0, 1),
        (63, 1, 2),
        (65, 0, 2),
        (65, 1, 3),
        (66, 0, 3), // the table's last key
        (66, 1, 4),
        (0x43, 0, 4),
        (0x8e, 0x2e, 4), // `2e 8e`, the 16th compare's word
    ];

    #[test]
    fn sync_reads_the_table_the_object_reads() {
        for &(order, row, expected) in SYNC_CASES {
            assert_eq!(sync_from(order, row), expected, "order {order}, row {row}");
        }
    }

    #[test]
    #[should_panic(expected = "the position is above the word `sync_10`'s 16th iteration compares against (`0x8E2E`)")]
    fn sync_refuses_the_first_key_past_the_sixteenth_compare() {
        sync_from(0x8e, 0x2f);
    }

    /// The sixty bytes the port carries appear exactly once in each shell build the tree holds,
    /// with `07 c3` before them, `2e 8e` after them, and then a word that differs between builds.
    #[test]
    fn the_table_is_the_shipped_shells_own_bytes() {
        let mut bytes = Vec::with_capacity(ORDERSYNC1.len() * 4);
        for &(key, code) in ORDERSYNC1 {
            bytes.extend_from_slice(&key.to_le_bytes());
            bytes.extend_from_slice(&(code as u16).to_le_bytes());
        }

        for (path, no_match_return) in [
            ("../../upstream/SecondReality/MAIN/U2.EXE", 0xcd06),
            ("../../upstream/SecondReality/MAIN/2NDFIX.EXE", 0xed06),
            ("../../upstream/SecondReality/U2.EXE", 0x9d06),
        ] {
            let image = std::fs::read(path).unwrap_or_else(|e| {
                panic!("{path}: {e}; the upstream tree is a submodule: `git submodule update --init`")
            });
            let hits: Vec<usize> = image
                .windows(bytes.len())
                .enumerate()
                .filter(|(_, window)| *window == bytes.as_slice())
                .map(|(at, _)| at)
                .collect();
            assert_eq!(hits.len(), 1, "{path}: the table appears {} times", hits.len());
            let at = hits[0];
            let word = |offset: usize| u16::from_le_bytes([image[offset], image[offset + 1]]);
            assert_eq!(word(at - 2), 0xc307, "{path}: the word before the table is `pop es; ret`");
            assert_eq!(word(at + bytes.len()), PAST_THE_TABLE, "{path}: `sync_10`'s first two bytes");
            assert_eq!(word(at + bytes.len() + 2), no_match_return, "{path}: the no-match return");
        }
    }

    #[test]
    fn no_music_answers_as_a_run_with_no_player() {
        let mut music = NoMusic;
        music.poll();
        assert_eq!(music.position(), MusicPosition::default());
        assert_eq!(music.musplus(), -32, "the object's own no-plus sentinel");
        assert_eq!(music.musrow(), 0);
        assert_eq!(music.muscode(1), 0);
        assert_eq!(music.getmframe(), 0);
        music.setmframe(1516);
        assert_eq!(music.getmframe(), 0, "there is no word to write without a player");
    }

    #[test]
    #[should_panic(expected = "NoMusic::sync()")]
    fn no_music_refuses_to_sync() {
        NoMusic.sync();
    }
}
