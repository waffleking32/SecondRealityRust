//! The channel effects: `D`, `E`, `F`, `G`, `H`, `J`, `K`, `O`, `Q` and `S`'s `SDx`.
//!
//! **Where this file's facts come from.** Like `player.rs`, everything here is read out of
//! `upstream/SecondReality/MAIN/STMIK.300`'s own segment image: the offsets in every doc
//! comment are that object's segment offsets, and `crates/sr-audio/tests/player.rs`'s claims
//! table asserts the load-bearing bytes at those offsets. Nothing here is read off Scream
//! Tracker documentation — STMIK deviates in places, and where the object and the usual ST3
//! reading disagree the object wins and the disagreement is named.
//!
//! Five readings the code below depends on, each with the bytes behind it:
//!
//! 1. **Two dispatch tables, and a command that only runs on some ticks.** A row's first tick
//!    walks the channels at `+0x3c44` (its table is at `+0x3bd8`) and every later tick at
//!    `+0x3d38` (table `+0x3c0e`). `G` and `H` have *no* first-tick handler — both tables'
//!    entry 7 and 8 are the bare `ret` at `+0x3d72` — `K` and `L` share the volume slide's
//!    body (`+0x3f00`/`+0x3f05` set `bp` and fall into `+0x3f08`), and `O` has no handler in
//!    either table at all: it is applied while the row's cells are read (`+0x48d6`).
//!
//! 2. **The row's first tick pre-processes every used channel before dispatching it**
//!    (`+0x3c4d`…`+0x3d36`): a channel with no command has its period restored from `aorgspd`
//!    (`+0x3d0c`) and a `D` row does the same (`+0x3d21`), and any command other than `D`,
//!    `H`, `K`, `I` and `R` marks the vibrato position stale (`+0x3cea`,
//!    `orb $0x80,0x26(%si)`) so that the next vibrato tick restarts the phase.
//!
//! 3. **The parameters are remembered per channel, in three different bytes.** `D`, `E`, `F`
//!    and `Q` fall back on `alastnfo` (`+0x39`, which the first-tick pass writes for *any*
//!    channel whose info byte is not zero, `+0x3cb9` — so one command's parameter is the
//!    next command's, on the same channel); `H` keeps its own `alasteff1` (`+0x2b`) and `K`
//!    reads *that* for its vibrato half (`+0x4000`); `G` keeps `alasteff2` (`+0x2c`) and `L`
//!    reads that (`+0x3f8f`).
//!
//! 4. **Sequences run in periods, and a period is `note_period × 8363/c2spd`.** The note
//!    table at `+0xa24` is the S3M 12-TET table four octaves up (C is 27392, so octave 4's C
//!    is the familiar 1712), shifted right by one byte per octave from `+0x97c`, and
//!    `+0x5813` scales it by the instrument's `c2spd` against the 8363 reference. Every
//!    effect is measured in that unit, and [`period_step`] is what the mixer will turn it
//!    back into (`8363 × 1712/period`, the object's `+0x582b`).
//!
//! 5. **The `Q` tables' zero slots are zero in the object.** The multiplier table at `+0x96c`
//!    is non-zero only for codes 6, 7, 14 and 15; its other slots, and the addend table's last
//!    two, are `LIDATA` runs of `0x00`. (Until Phase 9 Task 6b `tests/player.rs` decoded every
//!    `LIDATA` run as `0x01`, and this note called them `0x0101` filler cleared at runtime; the
//!    value the port uses was zero either way.) The zero is what sends the codes that are not
//!    multipliers down the addend path — see [`Channel::retrigger`].

/// One channel's tracker state: the fields of `struct st_zchn` (`MAIN/STMIK.H:203-258`) that
/// the notes and the effects own.
///
/// The field names are the port's; each doc comment carries the object's own field name and
/// its offset in the channel struct, because that is where it was read. The mixer's fields
/// (`m_pos`'s advance, `m_end`, `m_loop`, `m_speed`, `m_safe`, `m_lsafe`, `m_vol`) are Task
/// 5's; the two this file keeps are the ones an effect writes.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Channel {
    /// `achannelused` (`+0x01`) as a flag: the dispatchers walk a channel only when the byte
    /// is not zero (`+0x3c4d`, `+0x3d41`). The row parse sets it to 1 (`+0x474a`), an `SDx`
    /// note start to `0x81` (`+0x475e`) and the "volume 0" path to 0 (`+0x3ca9`).
    pub used: bool,
    /// `aequ` (`+0x02`), the raw byte: the sample's playback state. A note start that has to
    /// move the position sets it to `0xff` (`+0x491a`) and the note start's tail replaces
    /// that with `avol + 0x10` (`+0x496a`) — so a channel playing at volume 0 still reads
    /// `0x10`, not 0. A note that only re-targets a `G` leaves it alone. The port's only
    /// readers are those two writes in `start_note`; the mixer plays from `volume`, not this
    /// byte (the object's voice routine reads `m_vol` at `+0x41`, `+0x37a6`).
    pub aequ: u8,
    /// `avol` (`+0x1a`): the channel's volume, 0..=63. Never 64: a cell's volume column is
    /// clamped to `0x3f` by the row parse (`+0x477b`) and a volume slide clamps itself
    /// (`+0x3f5e`), so the file's own 64 is not a volume the player ever holds.
    pub volume: u8,
    /// `aorgvol` (`+0x2a`): the volume the instrument or the cell last set — what `I`
    /// (tremor) restores and `R` (tremolo) modulates around.
    pub org_volume: u8,
    /// `aspd` (`+0x12`): the period the channel plays. Every pitch effect moves this one, and
    /// the note start sets it from the note and the instrument's `c2spd`.
    pub period: u16,
    /// `asldspd` (`+0x20`): the `G` (tone porta) target, written by the note start and read
    /// by the glide.
    pub target_period: u16,
    /// `ac2spd` (`+0x22`): the current instrument's `c2spd`, loaded with the instrument
    /// (`+0x4850`) and replaced whole by `S2x` (`+0x42d0`).
    pub c2spd: u16,
    /// `aorgspd` (`+0x24`): the period at the last note, which `H` (vibrato) modulates and
    /// `G` (tone porta) walks — and which a channel with no command, and a `D` row, is
    /// restored to on the row's first tick (`+0x3d0c`, `+0x3d24`).
    pub org_period: u16,
    /// `avibcnt` (`+0x26`): the vibrato position. Its low byte is the phase in steps of two
    /// through the waveform's byte offsets, its bit 7 is "stale — restart the phase" and its
    /// high byte stays zero: the waveform functions put the *position* in `bx` before the
    /// table read leaves the value in `ax` (`+0x4083`), so only `bl` is ever updated.
    pub vibrato_position: u16,
    /// `alasteff1` (`+0x2b`): the last `H` parameter — and the parameter `K`'s vibrato half
    /// reads (`+0x4000`). It is *not* the byte `D` remembers.
    pub last_effect1: u8,
    /// `alasteff2` (`+0x2c`): the last `G` parameter. The object's `L` (`+0x3f05`) reads it
    /// from `+0x3f8f` as its tone-porta step; **no cell in either module is an `L`** (0 of
    /// them), so the port keeps the field `G` writes and has no `L` dispatch — see the note's
    /// "absent" list.
    pub last_effect2: u8,
    /// `atreon` (`+0x2d`): `I` (tremor) is in its "on" half. The first-tick pass sets it to
    /// 1 with `atremor` 0 for every command but `I` (`+0x3cd3`).
    pub tremor_on: bool,
    /// `atrigcnt` (`+0x2e`): the `Q` retrigger counter *and* the `SDx` note-delay countdown —
    /// one field, two commands (`+0x420c`, `+0x4296`). Zeroed for a channel with no command
    /// and for a `D` row (`+0x3d0e`, `+0x3d24`).
    pub retrigger_count: u8,
    /// `atremor` (`+0x2f`): the `I` (tremor) tick counter.
    pub tremor_count: u8,
    /// `a0volcut` (`+0x31`): the "volume 0" optimisation's countdown (`+0x3c97`). Three rows
    /// with a zero volume and no new note free the channel.
    pub zero_volume_count: u8,
    /// `note` (`+0x32`): the row's note byte, `0xff` for "none" — which is what the row parse
    /// writes for every channel the row does not name (`+0x46ad`).
    pub note: u8,
    /// `ins` (`+0x33`): the row's instrument byte, 0 for "none" (`+0x46b3`).
    pub instrument: u8,
    /// `vol` (`+0x34`): the row's volume column, `0xff` for "none".
    pub cell_volume: u8,
    /// `cmd` (`+0x35`): the row's command byte, 0 for "none" — which is what the dispatchers
    /// test (`+0x3cc0`, `+0x3d4d`).
    pub command: u8,
    /// `info` (`+0x36`): the row's parameter byte. Several handlers write *back* into it when
    /// they fall back on a remembered parameter (`+0x3f14`, `+0x401d`, `+0x3fa4`).
    pub info: u8,
    /// `lastins` (`+0x37`): the last instrument that loaded a PCM sample; an `O` does nothing
    /// until one has (`+0x48d2`).
    pub last_instrument: u8,
    /// `lastnote` (`+0x38`): the last note played — what `J` (arpeggio) transposes (`+0x41bb`).
    pub last_note: u8,
    /// `alastnfo` (`+0x39`): the last non-zero parameter of *any* command on this channel
    /// (`+0x3cb9`), which `D`, `E`, `F` and `Q` fall back on.
    pub last_info: u8,
    /// `vibtretype` (`+0x3d`): the low bits select the vibrato waveform and bit 3 says "do
    /// not restart the phase"; the high nibble, shifted, is the tremolo waveform (`+0x42f1`,
    /// `+0x4303`). **Never written in either module** — `S3x`/`S4x` are its only writers and
    /// neither file carries one — so every channel runs the sine with the restart.
    pub waveform: u8,
    /// `a0clearcnt` (`+0x3e`): a countdown that wipes the channel's cell when it reaches zero
    /// (`+0x3c57`…`0x3c74`). `_zplaynote` is its only writer, and the demo never calls it, so
    /// the port carries the field for the dispatcher's sake and nothing sets it.
    pub clear_count: u8,
    /// `aadr`'s high word (`+0x06`): the offset the next note starts from, in *bytes* of the
    /// sample. `Oxy` writes `info << 8` here (`+0x48e1`).
    pub sample_offset: u32,
    /// `m_pos` (`+0x44`): where the mixer is in the sample. The note start sets it from
    /// [`Channel::sample_offset`], `Q` restarts it at 0, a note off parks it at 1 and the
    /// "volume 0" path at `0xffff`; advancing it is Task 5's.
    pub position: u32,
    /// `m_poslow` (`+0x42`): the position's fraction.
    pub position_low: u16,
}

impl Channel {
    /// What `_zplaysong`'s song initialiser writes into a channel (`+0x5614`, called at
    /// `+0x5716` from the routine `_zplaysong` calls at `+0x258e`; `_zinitmodule` and
    /// `_zshutup` call the same routine), and **only** that: every field it does not store
    /// keeps the value it had (Phase 9 Task 6b, D45).
    ///
    /// The stores are `+0x5625`…`+0x5681`'s loop body, then `+0x5781` (called at `+0x5696`)
    /// and the silence pass at `+0x569a` (called after the initialiser at `+0x5719`). `ax` is
    /// zero throughout (`+0x561b`). Their other callees store nothing into `_zchn` on the
    /// measured card, SB Pro: `+0x4aa0` writes the AdLib register shadow and ports, `+0x2be7`
    /// and `+0x2bf8` are gated off by `[0x89e]` = 0, the driver's function `0xf` that `+0x2c7a`
    /// asks for is past the SB driver's table (`+0x5a60`), and `+0x4d88` is gated off by the
    /// `[0x8a3]` = 0 that `+0x4ade` has just written. In the port's fields:
    ///
    /// | field | offset | store |
    /// |---|---|---|
    /// | [`used`](Channel::used) | `+0x01` | `movb $0x80` (`+0x564d`), then `+0x5781`'s `andb $0x7f` (`+0x5791`), so 0 |
    /// | [`period`](Channel::period) | `+0x12` | `+0x5678` |
    /// | [`target_period`](Channel::target_period) | `+0x20` | `+0x5675` |
    /// | [`org_period`](Channel::org_period) | `+0x24` | `+0x5666` |
    /// | [`vibrato_position`](Channel::vibrato_position) | `+0x26` | `+0x566c` |
    /// | [`org_volume`](Channel::org_volume) | `+0x2a` | `+0x5669` |
    /// | [`last_effect1`](Channel::last_effect1), [`last_effect2`](Channel::last_effect2) | `+0x2b`, `+0x2c` | one word, `+0x5672` |
    /// | [`note`](Channel::note) | `+0x32` | `+0x5654` — 0, not the row reset's `0xff` |
    /// | [`instrument`](Channel::instrument) | `+0x33` | `+0x5651` |
    /// | [`cell_volume`](Channel::cell_volume) | `+0x34` | `+0x5657` — 0, not `0xff` |
    /// | [`command`](Channel::command), [`info`](Channel::info) | `+0x35`, `+0x36` | `+0x565a`, `+0x565d` |
    /// | [`last_instrument`](Channel::last_instrument) | `+0x37` | `+0x567e` |
    /// | [`last_note`](Channel::last_note) | `+0x38` | `+0x5660`, `+0x567b` |
    /// | [`last_info`](Channel::last_info) | `+0x39` | `+0x566f` |
    /// | [`position`](Channel::position) | `+0x44` | `movl $0x0` (`+0x5628`) |
    ///
    /// **Kept**, because no store names them: [`aequ`](Channel::aequ) (`+0x02`),
    /// [`sample_offset`](Channel::sample_offset) (`aadr`'s high word, `+0x06`),
    /// [`volume`](Channel::volume) (`avol`, `+0x1a`), [`c2spd`](Channel::c2spd) (`+0x22`),
    /// [`tremor_on`](Channel::tremor_on) (`+0x2d`), [`retrigger_count`](Channel::retrigger_count)
    /// (`+0x2e`), [`tremor_count`](Channel::tremor_count) (`+0x2f`),
    /// [`zero_volume_count`](Channel::zero_volume_count) (`a0volcut`, `+0x31`),
    /// [`waveform`](Channel::waveform) (`+0x3d`), [`clear_count`](Channel::clear_count)
    /// (`+0x3e`) and [`position_low`](Channel::position_low) (`m_poslow`, `+0x42`).
    ///
    /// The object's other stores go to words the port does not keep in this struct:
    /// `channelnum` (`+0x03`, the loop's own index, `+0x5625`), `+0x1b`, `lastadlins` (`+0x3a`,
    /// `0x65`), the silence pass's `+0x04` and `+0x3f` (`0xff`), and the mixer's half — `m_vol`
    /// (`+0x41`), `m_end` (`+0x48`/`+0x4a`), `m_speed` (`+0x50`/`+0x52`), `m_base`
    /// (`+0x58`/`+0x5a`; the sample base, `0x708c`: `mov 0x58(%si),%cx; shl $0xe,%ecx`) and
    /// the mixer's shadow of the position, `+0x5c` (`+0x6fb8` compares it with `m_pos`,
    /// `+0x6fc7` rewrites it) — which `Mixer::reset` clears. Every store and mixer read named
    /// here is an entry in `tests/player.rs`'s claims table, and that file's
    /// `the_song_initialiser_writes_exactly_what_initialise_writes` decodes the stores from the
    /// object and checks this table against them, field by field.
    ///
    /// **On a boot this is a fresh channel**: the object's own `_zchn` is zero in every byte
    /// (`tests/player.rs`'s `a_boot_starts_on_the_objects_zeroed_channels`), so the kept fields
    /// are [`Channel::default`]'s zeroes. **On a mid-song restart it is not**: `restartmus`
    /// (`MAIN/U2.ASM:348-397`) reaches it with the previous module's channels, and their volumes
    /// and countdowns are what the new song's first rows see ([`crate::Player::restart_from`]).
    pub fn initialise(&mut self) {
        self.used = false;
        self.period = 0;
        self.target_period = 0;
        self.org_period = 0;
        self.vibrato_position = 0;
        self.org_volume = 0;
        self.last_effect1 = 0;
        self.last_effect2 = 0;
        self.note = 0;
        self.instrument = 0;
        self.cell_volume = 0;
        self.command = 0;
        self.info = 0;
        self.last_instrument = 0;
        self.last_note = 0;
        self.last_info = 0;
        self.position = 0;
    }

    /// The row reset (`+0x46a4`): note `0xff`, volume `0xff`, instrument, command and info 0,
    /// for the object's 48 channels, *before* the row's own cells are read. A channel the row
    /// does not name therefore has no command at all, which is what stops the previous row's
    /// `D` from running again.
    pub(crate) fn clear_cell(&mut self) {
        self.note = 0xff;
        self.cell_volume = 0xff;
        self.instrument = 0;
        self.command = 0;
        self.info = 0;
    }

    /// The cell the object holds for this channel once the row's cells are read: a note *and*
    /// an instrument together, a volume column, a command and its info — the parts the row
    /// named (`+0x4677`…`0x469c`). `lastnote` and `lastins` are updated here too
    /// (`+0x467c`, `+0x4689`).
    pub(crate) fn install_cell(&mut self, cell: &crate::Cell) {
        if cell.has_note() {
            self.note = cell.note;
            self.instrument = cell.instrument;
            if cell.note != 0xff {
                self.last_note = cell.note;
            }
            if cell.instrument != 0 {
                self.last_instrument = cell.instrument;
            }
        }
        if cell.has_volume() {
            self.cell_volume = cell.volume;
        }
        if cell.has_command() {
            self.command = cell.command;
            self.info = cell.info;
        }
    }

    /// The row parse's clamps (`+0x4762`…`0x4780`): an instrument byte above `0x65` becomes
    /// 0 and a volume column above `0x3f` becomes `0x3f`. They run *after* an `SDx` cell has
    /// been deferred (`+0x475d` returns before them) and before the note and the instrument
    /// are looked at.
    pub(crate) fn clamp_cell(&mut self) {
        if self.instrument > 0x65 {
            self.instrument = 0;
        }
        if self.cell_volume != 0xff && self.cell_volume > 0x3f {
            self.cell_volume = 0x3f;
        }
    }

    /// True when the row parse must *not* start the note: `cmd == S` with an info whose high
    /// nibble is `0xd0` (`+0x4751`…`0x475d`). The cell's fields are installed and the channel
    /// marked used, but the note waits for the delay's countdown.
    pub(crate) fn defers_note(&self) -> bool {
        self.command == 0x13 && self.info & 0xf0 == 0xd0
    }

    /// `Oxy` (`+0x48d6`): the sample offset in bytes, `info × 256`. A cell that names an
    /// instrument clears the field on its way in (`+0x481d`); the command itself needs one to
    /// have been loaded before (`+0x48d2`), and an `O` with no instrument of its own leaves
    /// the last offset in place for the next note to start from.
    pub(crate) fn set_sample_offset(&mut self) {
        self.sample_offset = u32::from(self.info) << 8;
    }
}

// ---------------------------------------------------------------------------------------
// The row's first tick, and every later one
// ---------------------------------------------------------------------------------------

impl Channel {
    /// The row's first tick, per channel: the object's `+0x3c4d`…`0x3d36`, which runs before
    /// the command is dispatched and does the bookkeeping the command needs — the
    /// `a0clearcnt` countdown, the "volume 0" test, `alastnfo`, the period restores and the
    /// stale-vibrato mark.
    ///
    /// Returns `false` when the channel is skipped entirely — not used, or freed by the
    /// "volume 0" optimisation — which is also when the caller must not dispatch anything.
    pub(crate) fn row_start(&mut self, master_flags: u16) -> bool {
        if !self.used {
            return false;
        }
        // The `a0clearcnt` countdown (`+0x3c57`…`0x3c74`): at zero it wipes the cell, and
        // then the wiped values are what everything below sees. Only `_zplaynote` sets it, so
        // on these modules it is a countdown that never runs.
        if self.clear_count != 0 {
            self.clear_count -= 1;
            if self.clear_count == 0 {
                self.clear_cell();
            }
        }
        // The "volume 0" optimisation (`+0x3c77`…`0x3cad`), on when `_np_masterflags`'s bit 3
        // is. That word is the module header's own flags word (`+0x53ef`, `+0x5401`) and both
        // files carry 8, so it is on.
        if master_flags & 0x8 != 0
            && self.volume == 0
            && self.cell_volume == 0xff
            && self.instrument == 0
            && self.note == 0xff
        {
            // `decb 0x31(%si); jne` — the decrement happens first and the channel is freed
            // when the *result* is zero, so a countdown of 3 frees it on the third silent row.
            self.zero_volume_count = self.zero_volume_count.wrapping_sub(1);
            if self.zero_volume_count == 0 {
                self.used = false;
                self.position = 0xffff;
                return false;
            }
        } else {
            self.zero_volume_count = 3;
        }

        let command = self.command;
        if self.info != 0 {
            // `+0x3cb9`: remembered for *any* command, which is how a `D00` after an `F31`
            // slides by the `F`'s parameter.
            self.last_info = self.info;
        }
        if command == 0 {
            // `+0x3d0c`: no command — the counter is cleared and the period goes back to the
            // note's own, which is what undoes a vibrato or an arpeggio at the row's end.
            self.restore_period();
            return true;
        }

        if command == 0x4 {
            // `+0x3d24`: a `D` row restores the period too, and then dispatches.
            self.restore_period();
        }
        if command != 0x9 {
            self.tremor_count = 0;
            self.tremor_on = true;
        }
        if !matches!(command, 0x8 | 0xb | 0x12) {
            // `+0x3cea`: the vibrato position is stale — the next `H`/`K` tick restarts the
            // phase (each waveform function's `test $0x80,%ax`) rather than continuing it.
            self.vibrato_position |= 0x80;
        }
        true
    }

    /// `+0x3d0e`/`+0x3d24`: the retrigger counter is cleared and the period goes back to the
    /// last note's, and the channel is the mixer's again if it moved.
    fn restore_period(&mut self) {
        self.retrigger_count = 0;
        if self.period != self.org_period {
            self.period = self.org_period;
        }
    }

    /// Every tick after the row's first: the object's `+0x3d38`. No pre-pass and no parameter
    /// bookkeeping — a channel with no command, or a command above `0x1b` (the file's 255
    /// spelling of "none"), is skipped.
    pub(crate) fn dispatches_on_tick(&self) -> bool {
        self.used && self.command != 0 && self.command <= 0x1b
    }
}

// ---------------------------------------------------------------------------------------
// D — the volume slide, and the two commands that share its body
// ---------------------------------------------------------------------------------------

impl Channel {
    /// `Dxy` (`+0x3f08`), and `Lxy`/`Kxy` through the same body.
    ///
    /// The two nibbles slide the volume by *different* amounts and in opposite directions:
    /// the low nibble slides **down** and the high nibble **up**, and the object tests them
    /// in that order, low nibble first.
    ///
    /// | info | first tick | later ticks |
    /// |---|---|---|
    /// | `D0y` (y≠0) | `volume -= y` | `volume -= y` |
    /// | `Dx0` (x≠0) | `volume += x` | `volume += x` |
    /// | `DxF` (x≠0, low nibble f) | `volume += x` | nothing |
    /// | `DFx` (x≠0, high nibble f) | `volume -= x` | nothing |
    /// | `D0F`, `DF0` | as `D0y`/`Dx0` with y/x = 15 | the same |
    ///
    /// (`+0x3f22` sends the low nibble `0xf` to the fine-**up** path at `+0x3f73` and
    /// `+0x3f27` sends the high nibble `0xf` to the fine-**down** path at `+0x3f81`; the
    /// degenerate `D0F`/`DF0` have a zero in the other nibble, which sends them back to the
    /// ordinary 15-a-tick slide.) A zero info takes the channel's `last_info` and writes it
    /// back into the cell's info byte (`+0x3f11`). The volume clamps to 0…63 (`+0x3f3c`,
    /// `+0x3f5e`) — 64 is not reachable through a slide.
    pub(crate) fn volume_slide(&mut self, info: u8, tick_in_row: u16) {
        let info = if info != 0 {
            info
        } else {
            self.info = self.last_info;
            self.last_info
        };
        let down = info & 0x0f;
        let up = info >> 4;

        if down == 0x0f {
            if up != 0 {
                // `DFx`: fine, and the first tick is its only tick.
                if tick_in_row == 0 {
                    self.add_volume(i16::from(up));
                }
                return;
            }
            self.add_volume(-15);
        } else if up == 0x0f {
            if down != 0 {
                // `DxF`: fine, and the first tick is its only tick.
                if tick_in_row == 0 {
                    self.add_volume(-i16::from(down));
                }
                return;
            }
            self.add_volume(15);
        } else if down != 0 {
            self.add_volume(-i16::from(down));
        } else {
            self.add_volume(i16::from(up));
        }
    }

    /// `+0x3f33`/`+0x3f55`: the slide, clamped at both ends (0 and **63**).
    fn add_volume(&mut self, delta: i16) {
        self.volume = (i16::from(self.volume) + delta).clamp(0, 0x3f) as u8;
    }
}

// ---------------------------------------------------------------------------------------
// E and F — the portamentos
// ---------------------------------------------------------------------------------------

/// Which way a row slides the period: `E` (`+0x3e4f`) grows it and the note falls, `F`
/// (`+0x3e10`) shrinks it and the note rises. The two handlers are the same code with the
/// sign of the one instruction that matters flipped.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Portamento {
    /// `E`: the period grows.
    Down,
    /// `F`: the period shrinks.
    Up,
}

impl Channel {
    /// `Exy` (`+0x3e4f`) and `Fxy` (`+0x3e10`).
    ///
    /// With no note playing (`aorgspd == 0`) the command does nothing at all (`+0x3e53`), and
    /// an info of 0 repeats the channel's `last_info` (`+0x3e59`, written back into the cell).
    /// On the row's first tick only the two fine forms move anything — an info at or below
    /// `0xe0` is a no-op (`+0x3e3b`), `0xe1`…`0xf0` slides once by the low nibble, and above
    /// `0xf0` (the extra-fine form) by the low nibble **times four**. On every later tick the
    /// whole info byte is the step, times four (`+0x3e6c`), and an info of `0xe0` or more
    /// stops the slide entirely (`+0x3e6a`).
    ///
    /// Both `aspd` and `aorgspd` move (`+0x3e70`, `+0x3e73`), so a `D` row's restore keeps the
    /// slid period and a later vibrato measures from it.
    pub(crate) fn portamento(&mut self, info: u8, tick_in_row: u16, direction: Portamento) {
        if self.org_period == 0 {
            return;
        }
        let info = if info != 0 {
            info
        } else {
            self.info = self.last_info;
            self.last_info
        };
        let step = if tick_in_row == 0 {
            if info <= 0xe0 {
                return;
            }
            let low = u16::from(info & 0x0f);
            if info > 0xf0 {
                low * 4
            } else {
                low
            }
        } else {
            if info >= 0xe0 {
                return;
            }
            u16::from(info) * 4
        };
        let (period, org_period) = match direction {
            Portamento::Down => (
                self.period.wrapping_add(step),
                self.org_period.wrapping_add(step),
            ),
            Portamento::Up => (
                self.period.wrapping_sub(step),
                self.org_period.wrapping_sub(step),
            ),
        };
        self.period = period;
        self.org_period = org_period;
    }
}

// ---------------------------------------------------------------------------------------
// G — the tone portamento
// ---------------------------------------------------------------------------------------

impl Channel {
    /// `Gxx` (`+0x3f94`), reached from the *second* dispatch table: `G` has no first-tick
    /// handler, so a `G` row's first tick does nothing at all.
    ///
    /// The period walks towards `asldspd` — the target the note start wrote when a note came
    /// with a `G` on it — by `info × 4` a tick, and **`aorgspd` moves with it** so the walk
    /// does not slip (`+0x3fc4`). The parameter is remembered in `alasteff2` (`+0x3fa1`,
    /// which is `L`'s source) and a zero info reuses it. The step clamps exactly onto the
    /// target and never past it (`+0x3fbc`, `+0x3fe3`), and a channel already there does
    /// nothing (`+0x3faf`).
    ///
    /// The object calls the interpolation handler at `+0x57b0` when `[0x919]` is set — the
    /// byte `S1x` writes, which neither module carries (the byte is 0, so the call never
    /// happens).
    pub(crate) fn tone_portamento(&mut self, info: u8) {
        let info = if info != 0 { info } else { self.last_effect2 };
        self.last_effect2 = info;
        self.info = info;
        self.slide_to_target(info);
    }

    fn slide_to_target(&mut self, info: u8) {
        if self.org_period == 0 || self.period == self.target_period {
            return;
        }
        let step = u16::from(info) * 4;
        if self.period < self.target_period {
            self.period = self.period.saturating_add(step).min(self.target_period);
        } else {
            self.period = self.period.saturating_sub(step).max(self.target_period);
        }
        self.org_period = self.period;
    }
}

// ---------------------------------------------------------------------------------------
// H — the vibrato, and K's second half
// ---------------------------------------------------------------------------------------

impl Channel {
    /// `Hxy` (`+0x4005`) — the body `K` reaches at `+0x4000`.
    ///
    /// The parameter is the channel's `alasteff1`, and a missing high nibble is *merged* into
    /// the previous parameter rather than replaced (`+0x4010`…`+0x4018`), so `H08` after
    /// `H81` is `H88`. The share is the classic trap in the other direction too: `K`'s vibrato
    /// half reads this byte (`+0x4000`) while `K`'s volume half reads `alastnfo`.
    ///
    /// The waveform is the sine at `+0xa50`, 64 signed words, ±255, read at the position's
    /// **byte** offset — the position steps by two, so its low seven bits are twice the table
    /// index. A position whose bit 7 is set (the stale flag [`Channel::row_start`] leaves)
    /// returns 0 and restarts the phase at `speed × 2`.
    ///
    /// The level is `value × depth`, arithmetic-shifted by `shift`, and the period is
    /// `aorgspd + level`. `shift` is 5 when `[0x915]` is 0 (`+0x4043`) — and that byte is the
    /// module header's flags word's bit 0 (`+0x5448`, `+0x5450`), which is 0 in both files
    /// because their flags word is 8 — so the amplitude tops out at ±119. Two forms exist at
    /// shift 4 and 5; the demo runs the 5.
    ///
    /// The position then advances by `speed × 2` and is masked to the even low seven bits
    /// (`+0x4060`), which also clears the stale bit.
    pub(crate) fn vibrato(&mut self, info: u8, shift: u8) {
        let info = if info != 0 { info } else { self.last_effect1 };
        let info = if info & 0xf0 == 0 {
            (info & 0x0f) | (self.last_effect1 & 0xf0)
        } else {
            info
        };
        self.last_effect1 = info;
        self.info = info;
        self.vibrato_level(info, shift);
    }

    /// `K`'s half (`+0x4000`): `alasteff1` is the parameter, with no merge and no write-back.
    pub(crate) fn vibrato_last(&mut self, shift: u8) {
        self.vibrato_level(self.last_effect1, shift);
    }

    fn vibrato_level(&mut self, info: u8, shift: u8) {
        if self.org_period == 0 {
            return;
        }
        let depth = i32::from(info & 0x0f);
        let speed = u16::from(info >> 4);
        let position = self.vibrato_position;
        let stale = position & 0x80 != 0;
        // The position is always even (it steps by `speed × 2` and is masked to `0x7e`), so
        // the byte offset the object indexes the table with is twice its index here.
        let value = if stale {
            0
        } else {
            i32::from(VIBRATO_SINE[usize::from(position & 0x7f) / 2])
        };
        let level = (value * depth) >> shift;
        self.period = self.org_period.wrapping_add(level as u16);
        let from = if stale { 0 } else { position & 0xff };
        self.vibrato_position = (from + speed * 2) & 0x7e;
    }
}

// ---------------------------------------------------------------------------------------
// J — the arpeggio
// ---------------------------------------------------------------------------------------

impl Channel {
    /// `Jxy` (`+0x4190`): the note cycles base, `+x` semitones, `+y` semitones with the tick
    /// — `tick_in_row % 3`, taken from the engine's own row counter (`+0x419a`'s
    /// `mov 0x2000,%ax; div`), so the first tick of every row is the note itself.
    ///
    /// The transposition starts from `lastnote`'s own semitone, carries **one** octave when it
    /// passes B (`+0x41c8`: a single subtract-and-carry, so a sum that overflowed twice would
    /// not), and the period is then rebuilt from scratch — `aspd` only. `aorgspd` stays at the
    /// note's own period, which is why a row with no command restores the note exactly.
    pub(crate) fn arpeggio(&mut self, info: u8, tick_in_row: u16, c2spd: u16) {
        let info = if info != 0 {
            info
        } else {
            self.info = self.last_info;
            self.last_info
        };
        let offset = match tick_in_row % 3 {
            1 => info >> 4,
            2 => info & 0x0f,
            _ => 0,
        };
        let note = self.last_note;
        let mut semitone = (note & 0x0f).wrapping_add(offset);
        let mut octave = note & 0xf0;
        if semitone >= 0x0c {
            semitone -= 0x0c;
            octave = octave.wrapping_add(0x10);
        }
        self.period = scaled_period(note_period(octave | semitone), c2spd);
    }
}

// ---------------------------------------------------------------------------------------
// Q — the retrigger
// ---------------------------------------------------------------------------------------

impl Channel {
    /// `Qxy` (`+0x41ec`): retrigger the note every `y` ticks with volume variation `x`.
    ///
    /// The counter is the channel's `atrigcnt` — the same field `SDx` counts down in. The
    /// object's test is `y <= atrigcnt` (`+0x4205`), so `y = 0` never retriggers at all and
    /// `y > 0` retriggers once the counter has reached `y`, after which the counter is 0
    /// again; the counter is incremented at the end of every call (`+0x4246`).
    ///
    /// The retrigger itself is `m_pos = 0` (`+0x420f`) — the sample restarts — plus the volume
    /// variation of the pair of tables at `+0x95c`/`+0x96c`: `x`'s *multiplier* if it has one
    /// (only 6, 7, 14 and 15 do: ×10/16, ×8/16, ×24/16 and ×32/16), otherwise `x`'s signed
    /// addend — 0, -1, -2, -4, -8, -16 for 0…5 and +1, +2, +4, +8, +16 for 9…13. The
    /// multiplier table's zero slots (this module's header, point 5) are what send those codes
    /// down the addend path. A zero info repeats the channel's `last_info` (`+0x41f0`).
    pub(crate) fn retrigger(&mut self, info: u8) {
        let info = if info != 0 {
            info
        } else {
            self.info = self.last_info;
            self.last_info
        };
        let interval = info & 0x0f;
        if interval != 0 && interval <= self.retrigger_count {
            self.retrigger_count = 0;
            self.position = 0;
            let code = usize::from(info >> 4);
            let multiply = RETRIGGER_MULTIPLY[code];
            if multiply != 0 {
                self.volume = ((u16::from(self.volume) * u16::from(multiply)) >> 4).min(0x3f) as u8;
            } else {
                self.volume =
                    (i16::from(self.volume) + i16::from(RETRIGGER_ADD[code])).clamp(0, 0x3f) as u8;
            }
        }
        self.retrigger_count = self.retrigger_count.wrapping_add(1);
    }
}

// ---------------------------------------------------------------------------------------
// S — the one subcommand the modules carry
// ---------------------------------------------------------------------------------------

impl Channel {
    /// `SDx` on the row's first tick (`+0x4296`): the delay goes into `atrigcnt` — the counter
    /// `Q` also uses — and the note the row parse deferred waits for it.
    pub(crate) fn note_delay(&mut self, info: u8) {
        self.retrigger_count = info & 0x0f;
    }

    /// `SDx` on every later tick (`+0x429a`): the counter counts down and, when it reaches
    /// zero, the note starts — `call 0x475e`, the only place the deferred cell's note begins.
    /// True on the tick that fires, and the countdown stops there.
    pub(crate) fn note_delay_tick(&mut self) -> bool {
        if self.retrigger_count == 0 {
            return false;
        }
        self.retrigger_count -= 1;
        self.retrigger_count == 0
    }
}

// ---------------------------------------------------------------------------------------
// The period arithmetic everything above is measured in
// ---------------------------------------------------------------------------------------

/// `+0x593a`: a note byte to a period. `0xfe` (ST3's note off) is 0; otherwise the period is
/// the 12-TET table at `+0xa24` for the note's semitone, shifted right by one byte per octave
/// from `+0x97c`.
///
/// `0xa24`'s twelve words are four octaves up from the S3M table the format documents (C is
/// 27392, so octave 4's C is 1712), which is what lets a single shift cover every octave.
///
/// The shift table is **eight bytes long** and the object indexes it with `note >> 4` without
/// a range check (`+0x594a`), so octave 8 reads the byte after it and octaves 9…15 read
/// further into the segment's next table. That is not a formality: MUSIC1 plays note `0x80`
/// (the note-range measurement in `docs/fidelity/music.md` §6 counts it), whose shift is that
/// ninth byte — **0**, the same shift as octave 0 — so the file's highest-written note plays
/// its lowest period. [`NOTE_SHIFTS`] keeps the sixteen bytes the object's lookup reads.
///
/// A semitone of 12…15 is not a note the format has, and neither module carries one (both
/// files' semitone histograms stop at 11): the object would read the four words the next
/// table begins with, and the port clamps instead of modelling bytes nothing can reach.
pub fn note_period(note: u8) -> u16 {
    if note == 0xfe {
        return 0;
    }
    let semitone = usize::from(note & 0x0f);
    debug_assert!(semitone < 12, "semitone {semitone} is not a note");
    let period = NOTE_PERIODS[semitone.min(NOTE_PERIODS.len() - 1)];
    let shift = NOTE_SHIFTS[usize::from(note >> 4)];
    period >> shift
}

/// `+0x5813`: the note's period scaled by the instrument's `c2spd` against the 8363 reference
/// — `period × 8363/c2spd`, one integer division, answered `0x7fff` when the product's high
/// word has reached the divisor (`+0x581f`, the overflow guard an 8086 `div` needs) or when
/// the quotient itself does not fit a signed word (`+0x5825`).
///
/// A `c2spd` of 8363 leaves the period alone, which is the format's own reference: the note
/// plays at the rate the instrument declares.
pub fn scaled_period(period: u16, c2spd: u16) -> u16 {
    let product = u32::from(period) * 8363;
    if product >> 16 >= u32::from(c2spd) {
        return 0x7fff;
    }
    let quotient = product / u32::from(c2spd);
    if quotient > 0x7fff {
        0x7fff
    } else {
        quotient as u16
    }
}

/// `+0x582b`'s step: the object's own base frequency over the period, which the code divides
/// as a 16.16 value — `mov $0xda,%dx; mov $0x7600,%ax; div period` (`+0x58b9`), i.e.
/// **`0x00da7600` = 14,317,056**, with the `≤ 218` branch at `+0x58a9` dividing the two
/// halves separately, which is the same thing whenever the quotient fits a word.
///
/// 14,317,056 is *not* `8363 × 1712` = 14,317,456: the object's constant is 400 short of the
/// product of the format's two reference numbers, so C-4 at the 8363 reference (`period`
/// 1712) is stepped at **8362**, not 8363, and an octave above at 16,725 rather than twice
/// 8362. The port uses the object's number, not the product.
///
/// The mixer divides this by the mixing rate (`+0x58c9`) and that is Task 5's; a period of
/// zero zeroes the mixer's speed and returns (`+0x5876`).
pub fn period_step(period: u16) -> u32 {
    const BASE_FREQ: u32 = 0x00da_7600;
    if period == 0 {
        return 0;
    }
    BASE_FREQ / u32::from(period)
}

// ---------------------------------------------------------------------------------------
// The object's tables
// ---------------------------------------------------------------------------------------

/// The vibrato waveform the object carries at `+0xa50`: 64 signed words, `round(255 ×
/// sin(2πi/64))`, which the waveform functions read at *byte* offset `position & 0x7f`. The
/// position steps by two, so the byte offset is twice the index here and this is a full cycle.
///
/// It is the only waveform the two modules reach: `S3x`/`S4x` set `vibtretype` and neither
/// file carries one, so every channel's selector is 0 — the sine, with the phase restart. The
/// other three tables the selector can reach (a ramp at `+0xb50`, a square at `+0xad0` and the
/// sine plus a random offset at `+0x40b4`) are in the note's "absent" list with their counts.
const VIBRATO_SINE: [i16; 64] = [
    0, 24, 49, 74, 97, 120, 141, 161, 180, 197, 212, 224, 235, 244, 250, 253, 255, 253, 250, 244,
    235, 224, 212, 197, 180, 161, 141, 120, 97, 74, 49, 24, 0, -24, -49, -74, -97, -120, -141,
    -161, -180, -197, -212, -224, -235, -244, -250, -253, -255, -253, -250, -244, -235, -224, -212,
    -197, -180, -161, -141, -120, -97, -74, -49, -24,
];

/// The twelve periods at `+0xa24`, semitone by semitone, four octaves above the S3M table the
/// format documents.
const NOTE_PERIODS: [u16; 12] = [
    27392, 25856, 24384, 23040, 21696, 20480, 19328, 18240, 17216, 16256, 15360, 14512,
];

/// The shift table at `+0x97c` **as the object reads it**: eight real entries (0…7, one per
/// octave) followed by the eight bytes the segment holds after them, which octaves 8…15 look
/// up because the object does not check the index.
const NOTE_SHIFTS: [u8; 16] = [0, 1, 2, 3, 4, 5, 6, 7, 0, 5, 11, 17, 27, 32, 42, 47];

/// The `Q` volume-variation addends at `+0x95c`, signed — with the same reading of the image
/// applied as the multipliers below: 6, 7, 14 and 15 are the codes that take the *multiply*
/// path (the only ones whose multiplier is not zero), and code 8's addend is a real zero byte of
/// its own. The last two slots, 14 and 15, are zero too and belong to multiply codes, so they
/// are never read here.
const RETRIGGER_ADD: [i8; 16] = [0, -1, -2, -4, -8, -16, 0, 0, 0, 1, 2, 4, 8, 16, 0, 0];

/// The `Q` volume-variation multipliers at `+0x96c`, in sixteenths; the zero slots are zero
/// bytes of the object.
const RETRIGGER_MULTIPLY: [u8; 16] = [0, 0, 0, 0, 0, 0, 10, 8, 0, 0, 0, 0, 0, 0, 24, 32];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_note_table_is_the_s3m_table_four_octaves_up() {
        // The object's own twelve words, shifted into the octave the format documents:
        // octave 4's C is 1712 and B is 907, the numbers the S3M format's table holds.
        let octave4: Vec<u16> = NOTE_PERIODS.iter().map(|period| period >> 4).collect();
        assert_eq!(
            octave4,
            vec![1712, 1616, 1524, 1440, 1356, 1280, 1208, 1140, 1076, 1016, 960, 907]
        );
    }

    #[test]
    fn a_note_above_octave_seven_reads_the_byte_after_the_shift_table() {
        // MUSIC1's highest note, 0x80: the shift is the ninth byte (0), so it lands on
        // octave 0's period rather than being refused.
        assert_eq!(NOTE_SHIFTS[8], 0);
        assert_eq!(note_period(0x80), NOTE_PERIODS[0]);
        assert_eq!(note_period(0x00), NOTE_PERIODS[0]);
        assert_eq!(note_period(0x40), 1712);
        assert_eq!(note_period(0xfe), 0);
    }

    #[test]
    fn a_step_is_the_base_frequency_over_the_period() {
        // The object's constant, 0x00da7600 = 14,317,056, over the octave-4 periods: C is
        // 8362 (the table's own 1712, and not the 8363 the format's reference suggests —
        // the constant is 400 below 8363 × 1712), the octave above 16,725, and C-2 4181.
        assert_eq!(period_step(1712), 8362);
        assert_eq!(period_step(856), 16725);
        assert_eq!(period_step(3424), 4181);
        assert_eq!(period_step(0), 0);
    }
}
