//! The music seam: what a part is handed in place of the Demo Interrupt Server's music calls.
//!
//! The original's parts never touch the player. They call into the DIS, whose music entries read
//! the words the music engine keeps in its own segment — `_np_zinfo`, `_np_row`, `_np_ord`,
//! `_np_zplus`, `_np_zframe` (`MAIN/STMIK.H:125-131`) — and which are declared for C in
//! `DIS/DIS.H:37-59`. Three of the six calls are one service, `muscode_6`
//! (`DIS/DISINT.ASM:242-282`), read back three ways by its wrappers (`DIS/DISC.ASM:91-123`):
//! `%ax` is `_np_zinfo`, `%bx` is the reloaded `_np_row`, and `%dx` is the row offset the same
//! block computes. `musframe_9` (`:412-425`) reads and writes `_np_zframe`. `sync_10` (`:449-463`)
//! turns the published position into a part-start code through the `ordersync1` table above it
//! (`:428-446`).
//!
//! **This module is that interface and nothing else.** `sr-engine` must not learn about Scream
//! Tracker (the phase plan's Review Focus 4): there is no module, pattern, instrument or sample
//! rate here — a [`Music`] is eight methods, and the engine's only act on one is [`Music::poll`],
//! once per tick, where the original's copper calls `_zpollme` (`MAIN/COPPER.ASM:466`).
//!
//! **The real implementation is the binary's, and it stays there.** `sr-audio` does not depend on
//! `sr-engine`, and `sr-engine` has no runtime dependencies at all, so neither crate can name the
//! other's types: `impl Music for Player` (Task 7) lives in `bins/secondreality`, which depends on
//! both. Putting the impl in `sr-audio` would either move the engine's vocabulary into the music
//! crate or make every part crate depend on the player; putting the trait in `sr-audio` would do
//! the same from the other side. A reader who wants to move this boundary should re-read the
//! manifest test in `lib.rs` first — it is the rule that keeps the engine's hardware model here.
//!
//! The three translations below ([`musplus_from`], [`muscode_from`], [`sync_from`]) are here
//! rather than in the adapter because they are the *DIS's* arithmetic — the parts' side of the
//! seam, read out of `DIS/DISINT.ASM` — and the adapter's whole job is to hand them the player's
//! published words.

/// The "no plus pending" sentinel `muscode_6` starts `%dx` with (`DIS/DISINT.ASM:252`).
const NO_PLUS: i32 = -32;

/// The published music position — the words the DIS hands the parts, and what the window's
/// status line shows.
///
/// The three position words are the *published* ones (`_np_ord`, `_np_pat`, `_np_row`,
/// `MAIN/STMIK.H:126-128`; `muscode_6` reads two of them at `DIS/DISINT.ASM:250-251`), so they
/// come through the ten-poll ring and lag the player's own state by ten polls
/// (`docs/fidelity/music.md` §8) — that lag is the point rather than an artefact, because a part
/// that syncs is asking about the music the machine is playing *now*, not the row the engine has
/// already scheduled. `speed` and `tempo` are not ring words at all: an `A` and a `T` command
/// write `_np_speed` and `_np_tempo` straight through (`music.md` §8), which is why a trace's
/// speed column can lead its row column.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct MusicPosition {
    /// `_np_ord`: the order the counter holds.
    pub order: u8,
    /// `_np_pat`: the pattern that order names.
    pub pattern: u8,
    /// `_np_row`: the row the counter holds.
    pub row: u8,
    /// `_np_speed`: the current speed, written straight through by an `A` command.
    pub speed: u8,
    /// `_np_tempo`: the current tempo, written straight through by a `T` command.
    pub tempo: u8,
}

/// The music, as a part sees it: the six calls of `DIS/DIS.H:37-59`, plus the two the engine
/// itself needs.
///
/// Deliberately not more. The shell's own use of the player — fading it (`MAIN/U2.ASM:333-346`),
/// restarting it mid-run (`:348-397`), starting the module (`:786`) — is not a part's to make, so
/// it is not here: a trait that carried it would let any part restart the music, and the things
/// that *do* restart it are two whole files up from this seam.
pub trait Music {
    /// One logic tick's worth of music: the copper's `call _zpollme` (`MAIN/COPPER.ASM:466`).
    ///
    /// No arguments, deliberately. The engine says "a frame passed", never "here are samples":
    /// how much music a frame is worth, and what to do with the output, is the player's business
    /// (`Player::poll`, `docs/fidelity/music-clock.md` §5), and a seam that took samples would
    /// either move that arithmetic into the engine or make the engine aware of a mixing rate.
    fn poll(&mut self);

    /// The position a part reads: `_np_ord`, `_np_pat` and `_np_row` — the ring-published words,
    /// ten polls behind the player's own state — and `_np_speed`/`_np_tempo`, which an `A` and a
    /// `T` command write straight through. See [`MusicPosition`].
    fn position(&self) -> MusicPosition;

    /// `dis_musplus()` (`DIS/DISC.ASM:102-112`): the row offset a "plus coming" marker asks for —
    /// [`musplus_from`]'s arithmetic over `_np_zplus` and `_np_row` (`DIS/DISINT.ASM:252-271`).
    fn musplus(&mut self) -> i32;

    /// `dis_musrow()` (`DIS/DISC.ASM:114-123`): `_np_row`, the published row.
    fn musrow(&mut self) -> i32;

    /// `dis_muscode(code)` (`DIS/DISC.ASM:91-100`): `_np_zinfo`, whose value is [`muscode_from`]'s
    /// and whose deferral D8 records.
    fn muscode(&mut self, wanted: i32) -> i32;

    /// `dis_sync()` (`DIS/DISC.ASM:161-169`): the part-start code `ordersync1` gives the published
    /// position — [`sync_from`], including its declared boundary.
    fn sync(&mut self) -> i32;

    /// `dis_getmframe()` (`DIS/DISC.ASM:150-159`): `_np_zframe`, the *tick* counter the parts wait
    /// on — not the frame counter (`docs/fidelity/music-clock.md` §8).
    fn getmframe(&mut self) -> i32;

    /// `dis_setmframe(frame)` (`DIS/DISC.ASM:138-148`): the parts write this word, which is how it
    /// goes backwards mid-song (`docs/fidelity/music-clock.md` §8; `second3`'s 1516 → 0).
    fn setmframe(&mut self, frame: i32);
}

/// A borrow of the engine's music, with its object lifetime bound spelled out.
///
/// `&'a mut dyn Music` would read the same and mean something else: the engine holds the
/// implementation behind a `Box<dyn Music>`, i.e. a `dyn Music + 'static`, and a `&mut` trait
/// object is invariant in the lifetime bound it carries — so `&'a mut (dyn Music + 'static)` is
/// the type that can be reborrowed out of the box, and `&'a mut (dyn Music + 'a)` is one no
/// implementor could ever be stored as. Writing the bound once, here, is what makes
/// [`PartCtx::music`](crate::part::PartCtx::music) and [`Engine::set_music`] the same type.
///
/// [`Engine::set_music`]: crate::engine::Engine::set_music
pub type MusicRef<'a> = &'a mut (dyn Music + 'static);

/// What a part sees when the run has no player at all.
///
/// **Not a degraded music.** A stub that answered as if a track were playing would make a part
/// that waits on a cue look like a part that works, which is the failure this type exists to
/// prevent: the values here are the *null* readings, and a part that needs a cue meets one it can
/// never be handed.
///
/// - [`poll`](Music::poll) does nothing — there is nothing to advance.
/// - [`position`](Music::position) is all zeroes: no order, no row, no speed, no tempo.
/// - [`musplus`](Music::musplus) is [`musplus_from`] over a `zplus` of 0 and a row of 0 — the
///   object's own sentinel, `-32`, which is what a player with no marker pending gives.
/// - [`musrow`](Music::musrow) is 0.
/// - [`muscode`](Music::muscode) is [`muscode_from`] — 0, D8's substitute, so the parts that wait
///   for 1, 2, 3 or 0xf0 wait for ever here exactly as they do against the reference's own word.
/// - [`sync`](Music::sync) **refuses loudly**: there are no `_np_ord`/`_np_row` words to look a
///   code up in, and the object's own `sync_10` has no answer for that state either — its INDEMO
///   half reads the words out of a segment that would not exist, and its non-INDEMO half leaves
///   `%ax` as the caller left it (`DIS/DISINT.ASM:449-463`). A part that ignores music never
///   calls it; a part that waits on a start code fails where the port can see it rather than
///   waiting on a number the port made up.
/// - [`getmframe`](Music::getmframe) is 0 and [`setmframe`](Music::setmframe) writes nothing —
///   `_np_zframe` is a word in a segment that does not exist, so a part that waits for the
///   counter to arrive waits for ever, and a part that reads back its own write does not.
///
/// A run with no music at all is not this type: the engine's `music` field is `None` and
/// `PartCtx::music()` returns `None`, so a part that needs a cue says so. The shell refuses to
/// start such a part when the run has no music (Task 7) instead of letting it wait for ever.
/// `NoMusic` is what a *test* installs when it wants a part to be handed something rather than
/// nothing.
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
            "NoMusic::sync(): there is no player, so there is no position to look up — \
             `sync_10` (`DIS/DISINT.ASM:449-463`) reads `_np_ord` and `_np_row` out of the music \
             engine's own segment, and a run without one has no such words (`stmikseg` is never \
             set). Install a real `Music` (Task 7's adapter), or write the part so that it does \
             not sync when the run has no music."
        )
    }

    fn getmframe(&mut self) -> i32 {
        0
    }

    fn setmframe(&mut self, _frame: i32) {}
}

/// `dis_musplus()`'s answer, from `_np_zplus` and `_np_row` (`DIS/DISINT.ASM:252-271`).
///
/// The block loads `%dx` with `-32` (`:252`) and then, in the listing's own order:
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
/// So a `zplus` of 0 gives the sentinel always; 1 gives `row - 64` clamped up to `-32` (rows 32
/// and up, from `-32` at 32 to `191` at 255); 2 gives the row itself while it is under 32 and the
/// sentinel from 32 on; and 3 — the only other value a two-bit `zplus` can hold
/// (`docs/fidelity/music-clock.md` §6) — dispatches on `row > 32`: the row below 32, `row - 64`
/// above it, and the sentinel at exactly 32, where the two halves meet.
///
/// Both arguments are the **published** words: `muscode_6` reads `es:_np_zplus` and `es:_np_row`,
/// so a caller must pass `Player::zplus()` and `Player::state().row`, not the engine's own row ten
/// polls ahead of them. (The block reads `_np_row` a second time on the way out, `:272`, into
/// `%bx` for `dis_musrow`; the port has one reading and no race to model.)
///
/// The demo's own parts use the answer as a countdown to the marker: `GLENZ/MAIN.C:290` waits for
/// it to reach `-19`, `TECHNO/KOE.C:213` and `:275` for `-4` and `-3`, `PLZPART/PLZ.C:68` for 0,
/// `PLZPART/VECT.C:116` for 13 and `JPLOGO/JP.C:71` for 4, while `GLENZ/MAIN.C:411` breaks when it
/// lands between `-16` and 0 — every one of them reading how many rows are left before the plus.
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
        // `zplus == 3` (and any value the word could not actually hold): `@@1eb`'s unsigned `ja`
        // sends rows above 32 to the row-64 form and everything else through `@@1e`.
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

/// `dis_muscode()`'s answer: `_np_zinfo` (`DIS/DISINT.ASM:249`), which **is 0**, by D8's
/// deferral — the value the reference's own word held in every sample of every filed trace
/// (11,408 across the four traces; `docs/fidelity/music-clock.md` §6).
///
/// The parameter is the code the caller is waiting for, and the INDEMO build ignores it: the block
/// loads `%ax` with `_np_zinfo` before it looks at anything else (`:249`), so
/// `dis_muscode(1)` and `dis_muscode(0xf0)` both hand back the word. It is in the signature
/// because the parts pass one and because the *substitute* has to be swappable: a later phase that
/// finds a mechanism for `_np_zinfo`'s other values replaces this body with a function of the
/// position, and nothing else moves.
///
/// The callers are why that matters: `GRID/MAIN.C:72` waits for 2 and `:160` for 3,
/// `HARD/B.C:138` for 1 and `HARD/BP.C:263` for 0xff (`ENDPIC/BEG.C:48`'s 0xf0 is the build of
/// part 4 that did not ship — D8's amendment). Each waits for ever against a word this phase
/// measured as always 0, and D8's decision is to leave them waiting and write those parts in
/// Phase 8 against a measured cue rather than an inferred one.
pub fn muscode_from(_position: MusicPosition, _wanted: i32) -> i32 {
    0
}

/// The `ordersync1` table (`DIS/DISINT.ASM:428-446`), key then code, in source order.
///
/// The keys are `(order << 8) | row`: `sync_10` loads `%dh` from `_np_ord` and `%dl` from
/// `_np_row` (`:452-453`) and compares the pair as one 16-bit word. The codes are what a part
/// waits for — `PAM/OUTTAA.C:39`'s `while(dis_sync()<10)` and `ALKU/MAIN.C:59`, `:66`, `:72`,
/// `:79`, `:138`.
///
/// The port carries the table as the object carries it, which is the point of the test
/// `the_table_is_the_shipped_shells_own_bytes` — the same 60 bytes, in the same order, in the
/// shell builds the tree holds.
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

/// The word the `%bx - 2` read finds at the table's first entry: the two bytes before
/// `ordersync1`, which are `pop es; ret` — `07 c3` — because `ordersync1` follows `musframe_9`,
/// whose INDEMO body ends `pop es` (`DIS/DISINT.ASM:420`) and then `ret` (`:424`).
///
/// Every build the tree holds carries exactly those two bytes there — the test below finds them
/// — and a caller sees them the way the C callers do, as a 16-bit `int`: `0xC307` read signed is
/// **-15609**. That is what `dis_sync()` answers for the position `(0, 0)`, and it is a value a
/// caller lives with rather than a number the port invented: the tree's own callers compare it
/// with a small positive (`PAM/OUTTAA.C:39`'s `dis_sync() < 10`, `ALKU/MAIN.C:59`'s `< 1`), so
/// they keep waiting exactly as they do against the reference.
const BEFORE_THE_TABLE: i32 = i16::from_le_bytes([0x07, 0xc3]) as i32;

/// The word `sync_10`'s 16th iteration compares against — the two bytes *after* `ordersync1`,
/// which are `2e 8e`: the CS prefix and the `mov Sreg, r/m16` opcode of the `mov es, cs:stmikseg`
/// that follows the table (`DIS/DISINT.ASM:451`).
///
/// Those two bytes are the source's own, so they are the same in every build — and that is what
/// makes the first tier of the refused region answerable: a key at or below this word *matches*
/// the 16th compare, and the object then returns the table's own last code. Only the two bytes
/// after them — the instruction's displacement, which the linker sets — are not.
const PAST_THE_TABLE: u16 = u16::from_le_bytes([0x2e, 0x8e]);

/// `dis_sync()`'s answer for a published position — `sync_10` (`DIS/DISINT.ASM:449-463`) over the
/// table above, transliterated including the reads at its two ends.
///
/// The loop is the object's: `%cx` is 16 (`:455`) for the 15 entries, `%bx` walks the table four
/// bytes at a time, and the first entry whose key is at or below the position ends it (`:456-457`,
/// `jbe` — an *unsigned* compare of the whole `(order << 8) | row` word). The answer is then read
/// at **`%bx - 2`** (`:460`) — the *code of the entry before* the one that matched, not the code
/// of the matching entry. So the table's pairs are upper bounds: code `c` is what the position
/// reads from one row after the key before it, up to its own key inclusive — `(2, 0)` reads 0 and
/// `(2, 1)` reads 1.
///
/// **The read before the table is answered; the reads past it are refused.** At `(0, 0)` — the
/// position a run holds before the music's first row is published, and one the phase's own traces
/// hold for hundreds of frames — the read lands on `musframe_9`'s `pop es; ret`, and the port
/// returns it ([`BEFORE_THE_TABLE`]), because that word is *pinned by the source* and identical
/// in every build the tree holds. Above the last key (`0x4200`) the 16th iteration reads the
/// bytes that follow the table, which are `sync_10`'s own first instruction, `mov es, cs:stmikseg`
/// (`2e 8e 06 …`) — and there the port refuses, because the word it would return is one byte of
/// instruction framing (`06`) and the *low byte* of a **linker-set** displacement: measured
/// `0xCD06` in `MAIN/U2.EXE`, `0xED06` in `MAIN/2NDFIX.EXE` and `0x9D06` in `U2.EXE`. A port
/// cannot return a number that differs between builds of the original, so its domain is
/// `(order << 8) | row <= 0x4200` and above it this function panics with the reason and the
/// citations. (In the band `0x4200 < key <= 0x8E2E` the 16th compare *does* match — `2e 8e`
/// itself, pinned by the source and by all three builds — and the object answers with the last
/// entry's code, 4; the port refuses that band too, on the brief's own boundary rather than on
/// the value's variance. See `docs/fidelity/music.md` §11.)
///
/// The callers the demo has reach the domain by a wide margin — `PAM/OUTTAA.C:39` waits for code
/// 10, which the table gives from order 13 on, and `ALKU/MAIN.C:59-79` waits for 1 through 4, from
/// order 2 on — but the phase cannot show that they never call it above the last key (both parts
/// are Phase 4's, and how far MUSIC0's order list has got when they run is not measured).
/// `APPROXIMATIONS.md` A15 registers the boundary, and `docs/fidelity/music.md` §11 carries the
/// evidence.
///
/// # Panics
///
/// For a position above the table's last key — `(order << 8) | row > 0x4200` — where the object's
/// 16th iteration reads the code that follows the table.
pub fn sync_from(order: u8, row: u8) -> i32 {
    let key = (u16::from(order) << 8) | u16::from(row);
    // Only the position `(0, 0)` has key 0, and for it the object's compare matches at `%bx` =
    // the table's base, so its answer is the word at `%bx - 2` — before the table.
    if key == 0 {
        return BEFORE_THE_TABLE;
    }
    let Some(index) = ORDERSYNC1.iter().position(|&(key_at, _)| key <= key_at) else {
        // No key matched: the position is above the table's last entry, and the object's 16th
        // iteration (`DIS/DISINT.ASM:455-460`, `mov cx,16` over fifteen entries) compares against
        // the bytes that follow the table. What it does there depends on the position, so the
        // refusal says which of the two things it is refusing.
        let because = if key <= PAST_THE_TABLE {
            "Here the compare matches, and what the object answers with is the table's own last \
             code, 4. The port refuses this band on the boundary it draws — above the \
             table's last key the answer stops being the table's arithmetic and becomes a read of \
             whatever the assembler left after it — and not because the value varies."
        } else {
            "Here the compare fails, and the loop's fall-through returns the word after the table \
             instead: one byte of instruction framing (the `mov`'s modrm byte, `06`) and the low \
             byte of a link-time displacement, measured as 0xCD06 in MAIN/U2.EXE, 0xED06 in \
             MAIN/2NDFIX.EXE and 0x9D06 in U2.EXE. A number that differs between builds of the \
             original is one this port will not return."
        };
        panic!(
            "dis_sync() at (order {order}, row {row}): the position is above the table's last key \
             (`0x4200`), where `sync_10`'s 16th iteration compares against the bytes past the \
             table — the `mov es, cs:stmikseg` that follows it. {because} See \
             `docs/fidelity/music.md` §11 and `APPROXIMATIONS.md` A15."
        )
    };
    ORDERSYNC1[index - 1].1
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every `zplus` value against the rows where the object's decision tree changes its answer.
    /// Read off `DIS/DISINT.ASM:252-271`; the four groups are the four values the block can
    /// dispatch on, and `32`, `33` and `64` are where its two compares sit.
    const MUSPLUS_CASES: &[(i32, i32, i32)] = &[
        // zplus 0: the sentinel, always.
        (0, 0, -32),
        (0, 31, -32),
        (0, 32, -32),
        (0, 33, -32),
        (0, 64, -32),
        (0, 255, -32),
        // zplus 1: `row - 64`, clamped up at -32 — so 32..=63 read -32..=-1.
        (1, 0, -32),
        (1, 31, -32),
        (1, 32, -32),
        (1, 33, -31),
        (1, 63, -1),
        (1, 64, 0),
        (1, 95, 31),
        (1, 96, 32),
        (1, 255, 191),
        // zplus 2: the row while it is under 32, the sentinel from 32 on.
        (2, 0, 0),
        (2, 31, 31),
        (2, 32, -32),
        (2, 33, -32),
        (2, 64, -32),
        (2, 255, -32),
        // zplus 3: `@@1eb` — rows above 32 to the row-64 form, the rest through `@@1e`.
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
            assert_eq!(
                musplus_from(zplus, row),
                expected,
                "zplus {zplus}, row {row} (DIS/DISINT.ASM:252-271)"
            );
        }
    }

    /// The answer over every row, as data: for each `zplus`, the row ranges and the form the
    /// answer takes on each — `(first row, last row, form)`. The three forms are the whole of
    /// what `%dx` can hold coming out of the block: the row itself, the row less 64 (its clamp
    /// already applied at the low end), or the `-32` it was loaded with.
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    enum Form {
        Row,
        RowMinus64,
        Sentinel,
    }

    fn form_value(form: Form, row: i32) -> i32 {
        match form {
            Form::Row => row,
            Form::RowMinus64 => (row - 64).max(-32),
            Form::Sentinel => -32,
        }
    }

    /// For one `zplus`: the row ranges and the form the answer takes on each, as
    /// `(first row, last row, form)`.
    type Ranges = &'static [(i32, i32, Form)];

    const DOMAIN: &[(i32, Ranges)] = &[
        (0, &[(0, 255, Form::Sentinel)]),
        (
            1,
            &[(0, 31, Form::Sentinel), (32, 255, Form::RowMinus64)],
        ),
        (
            2,
            &[(0, 31, Form::Row), (32, 255, Form::Sentinel)],
        ),
        (
            3,
            &[
                (0, 31, Form::Row),
                (32, 32, Form::Sentinel),
                (33, 255, Form::RowMinus64),
            ],
        ),
    ];

    #[test]
    fn musplus_over_every_row_matches_the_objects_decision_tree() {
        let mut checked = 0;
        for &(zplus, ranges) in DOMAIN {
            for row in 0..=255 {
                let (_, _, form) = ranges
                    .iter()
                    .find(|&&(first, last, _)| (first..=last).contains(&row))
                    .unwrap_or_else(|| panic!("row {row} is in no range of zplus {zplus}"));
                assert_eq!(
                    musplus_from(zplus, row),
                    form_value(*form, row),
                    "zplus {zplus}, row {row}"
                );
                checked += 1;
            }
        }
        assert_eq!(checked, 4 * 256, "every (zplus, row) pair the word can hold");
    }

    /// What `sync_10` returns for a position, one case per boundary of the table
    /// (`DIS/DISINT.ASM:428-446`, read against the loop at `:454-460`). Each key is shown twice —
    /// at the key itself, where the *previous* entry's code still answers, and one row later,
    /// where its own code does — and `(0, 0)` is the read *before* the table, `pop es; ret` seen
    /// as the 16-bit `int` a C caller gets.
    const SYNC_CASES: &[(u8, u8, i32)] = &[
        (0, 0, -15609),
        (0, 1, 0),
        (1, 0, 0),
        (2, 0, 0),   // key 0x0200 itself: still code 0
        (2, 1, 1),   // one row later: code 1
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
    ];

    #[test]
    fn sync_reads_the_table_the_object_reads() {
        for &(order, row, expected) in SYNC_CASES {
            assert_eq!(sync_from(order, row), expected, "order {order}, row {row}");
        }
    }

    #[test]
    fn sync_answers_the_position_before_the_table_with_its_own_word() {
        // `dis_sync()` at `(0, 0)` is `musframe_9`'s `pop es; ret` read as a word — the same two
        // bytes in every build the tree holds, and a *negative* 16-bit `int` to the C callers,
        // so a caller's `dis_sync() < 1` waits on exactly as the reference's does.
        assert_eq!(sync_from(0, 0), -15609, "0xC307 as the `int` the callers see");
        assert_eq!(BEFORE_THE_TABLE, i16::from_le_bytes([0x07, 0xc3]) as i32);
    }

    #[test]
    #[should_panic(expected = "what the object answers with is the table's own last code, 4")]
    fn sync_refuses_the_position_past_the_tables_last_key() {
        // The first key of the refused region: `0x4201`, inside the band where the 16th compare
        // matches. The panic says so, rather than claiming a variance that does not apply here.
        sync_from(66, 1);
    }

    #[test]
    #[should_panic(expected = "A number that differs between builds of the original")]
    fn sync_refuses_the_far_end_of_the_order_range() {
        sync_from(255, 255);
    }

    #[test]
    #[should_panic(expected = "what the object answers with is the table's own last code, 4")]
    fn sync_refuses_the_band_whose_16th_compare_matches() {
        // `0x4200 < key <= 0x8E2E` (`sync_10`'s own first instruction, `2e 8e`, read as the 16th
        // compare) is answerable in every build the tree holds — the object returns the last
        // entry's code, 4 — but the port's boundary is the brief's, above the table's last key,
        // and this test is what says the refusal is a choice rather than a constraint. The
        // message has to agree: it names the 4, not the linker-set word.
        sync_from(0x8e, 0x2e);
    }

    #[test]
    fn the_refused_regions_two_tiers_are_the_ones_the_object_draws() {
        // The band's upper edge is the word past the table (`2e 8e`), read from the source's own
        // instruction; `sync_from` answers at or below the last key, and the first key above the
        // last key is in the band. The tier the panic message chooses is decided by exactly this
        // comparison, so it is worth pinning beside the message tests.
        assert_eq!(PAST_THE_TABLE, 0x8e2e);
        assert!(u16::from(0x42u8) << 8 | u16::from(0x01u8) <= PAST_THE_TABLE);
        assert!(u16::from(0xffu8) << 8 | u16::from(0xffu8) > PAST_THE_TABLE);
    }

    /// The table is the shipped shell's own bytes: the sixty bytes the port carries are searched
    /// for in each shell build the vendored tree holds, exactly once each, and the words the
    /// object reads at the table's two ends are read out beside them — the one `sync_from`
    /// *answers* with at `(0, 0)`, and the ones it refuses above the last key, measured rather
    /// than asserted.
    ///
    /// The shells are read from `upstream/SecondReality` at the submodule's pinned commit, as the
    /// asset tests read their files. This is a test-time read of the original, not a dependency
    /// of the engine: nothing here is linked into the library.
    #[test]
    fn the_table_is_the_shipped_shells_own_bytes() {
        let mut bytes = Vec::with_capacity(ORDERSYNC1.len() * 4);
        for &(key, code) in ORDERSYNC1 {
            bytes.extend_from_slice(&key.to_le_bytes());
            bytes.extend_from_slice(&(code as u16).to_le_bytes());
        }

        // Per build: where the table sits, the word before it (the position `(0, 0)`'s read),
        // the word past its last entry (the 16th iteration's compare), and the word after that
        // (what the 16th iteration returns when it does not match — `sync_10`'s own first
        // instruction, a `mov es, cs:stmikseg` whose displacement the linker sets).
        let mut seen = Vec::new();
        for path in [
            "../../upstream/SecondReality/MAIN/U2.EXE",
            "../../upstream/SecondReality/MAIN/2NDFIX.EXE",
            "../../upstream/SecondReality/U2.EXE",
        ] {
            let image = std::fs::read(path).unwrap_or_else(|e| {
                panic!(
                    "{path}: {e} — the upstream tree is a submodule; run `git submodule update \
                     --init`"
                )
            });
            let hits: Vec<usize> = image
                .windows(bytes.len())
                .enumerate()
                .filter(|(_, window)| *window == bytes.as_slice())
                .map(|(at, _)| at)
                .collect();
            assert_eq!(hits.len(), 1, "{path}: the table appears {} times", hits.len());
            let at = hits[0];
            assert!(at >= 4, "{path}: the table is at the image's start");
            let word = |offset: usize| u16::from_le_bytes([image[offset], image[offset + 1]]);
            seen.push((
                path,
                word(at - 2),
                word(at + bytes.len()),
                word(at + bytes.len() + 2),
            ));
        }

        for (path, before, past_the_table, no_match_return) in &seen {
            assert_eq!(*before, 0xc307, "{path}: the word before the table is `pop es; ret`");
            assert_eq!(
                *past_the_table, 0x8e2e,
                "{path}: the 16th iteration compares with `sync_10`'s own first two bytes"
            );
            // The return when no key matched is code, and it is *not* the same code in two
            // builds of the same shell — that half is what *forces* the refusal above the last
            // key. (The band where the 16th compare matches is answerable in every build, and
            // the port refuses it on the brief's boundary instead: see `sync_from`.)
            assert!(
                matches!(*no_match_return, 0xcd06 | 0xed06 | 0x9d06),
                "{path}: the no-match return reads {no_match_return:#06x}"
            );
        }
        assert_ne!(
            seen[0].3, seen[1].3,
            "the no-match return differs between the tree's two builds of the shell"
        );
        let no_match_return_of = |path: &str| {
            seen.iter()
                .find(|row| row.0 == path)
                .unwrap_or_else(|| panic!("{path} was read above"))
                .3
        };
        assert_eq!(
            no_match_return_of("../../upstream/SecondReality/MAIN/U2.EXE"),
            0xcd06
        );
        assert_eq!(
            no_match_return_of("../../upstream/SecondReality/MAIN/2NDFIX.EXE"),
            0xed06
        );
        assert_eq!(no_match_return_of("../../upstream/SecondReality/U2.EXE"), 0x9d06);
    }

    /// And the table itself is not the same in every build the tree holds: `MAIN/PROT/TEST.OLD`,
    /// the protection test build the shell's Makefile stages (`MAIN/MAKEFILE:39`, `:43-44`),
    /// carries **0x030b** — order 3, row 11 — where the three shells' third key is `0x0300`. In
    /// other words the *keys* move between builds too, which is the same rule the boundary reads
    /// are refused on: what the port carries is DISINT.ASM's table as the shipping build assembled
    /// it, and an older artefact's numbers are not that.
    #[test]
    fn an_older_build_carries_a_different_table() {
        let bytes_of = |table: &[(u16, i32)]| {
            let mut bytes = Vec::with_capacity(table.len() * 4);
            for &(key, code) in table {
                bytes.extend_from_slice(&key.to_le_bytes());
                bytes.extend_from_slice(&(code as u16).to_le_bytes());
            }
            bytes
        };
        let ours = bytes_of(ORDERSYNC1);
        let mut older = ORDERSYNC1.to_vec();
        older[2] = (0x030b, 2);

        let path = "../../upstream/SecondReality/MAIN/PROT/TEST.OLD";
        let image = std::fs::read(path).unwrap_or_else(|e| {
            panic!(
                "{path}: {e} — the upstream tree is a submodule; run `git submodule update \
                 --init`"
            )
        });
        assert_eq!(
            image.windows(ours.len()).filter(|w| *w == ours.as_slice()).count(),
            0,
            "TEST.OLD does not carry the shipping table"
        );
        let older = bytes_of(&older);
        assert_eq!(
            image.windows(older.len()).filter(|w| *w == older.as_slice()).count(),
            1,
            "…but one with 0x030b in the third key, exactly once"
        );
    }

    #[test]
    fn muscode_is_the_deferrals_substitute_over_every_code_the_parts_wait_for() {
        // The codes the demo's parts hand in: GRID's 2 and 3, HARD's 1 and 0xff, ENDPIC's 0xf0.
        for wanted in [0, 1, 2, 3, 0xf0, 0xff] {
            assert_eq!(muscode_from(MusicPosition::default(), wanted), 0);
            assert_eq!(
                muscode_from(
                    MusicPosition { order: 26, pattern: 70, row: 12, speed: 1, tempo: 125 },
                    wanted
                ),
                0,
                "the substitute is 0 at every position this phase measured, not only at (0, 0)"
            );
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
