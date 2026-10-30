//! The channel effects: `D`, `E`, `F`, `G`, `H`, `J`, `K`, `O`, `Q` and `S`'s `SDx`.
//!
//! Read out of `upstream/SecondReality/MAIN/STMIK.300`'s segment image: every `+0x…` below is
//! that object's segment offset, and `tests/player.rs`'s `the_ports_tables_are_the_objects_own`
//! holds the tables to the image. Where the object and the usual ST3 reading disagree, the object wins.
//! `docs/fidelity/music.md` §9 has the readings and what each one is measured against.
//!
//! - **Two dispatch tables.** A row's first tick walks the channels at `+0x3c44` (table
//!   `+0x3bd8`), every later tick at `+0x3d38` (table `+0x3c0e`). `G` and `H` have no first-tick
//!   handler, `K` and `L` share the volume slide's body (`+0x3f08`), and `O` is applied while the
//!   row's cells are read (`+0x48d6`).
//! - **Three remembered parameters.** `D`, `E`, `F`, `J` and `Q` fall back on `alastnfo`
//!   (`+0x39`, written for any non-zero info, `+0x3cb9`); `H` keeps `alasteff1` (`+0x2b`), which
//!   `K` also reads (`+0x4000`); `G` keeps `alasteff2` (`+0x2c`).
//! - **Everything is measured in periods**, `note_period × 8363/c2spd` (`+0x5813`).
//! - **The `Q` tables' zero slots are zero in the object** (`+0x96c`), which sends the codes
//!   that are not multipliers down the addend path (`Channel::retrigger`).

/// One channel's tracker state: the fields of `struct st_zchn` (`MAIN/STMIK.H:203-258`) that
/// the notes and the effects own, each with the object's field name and offset.
///
/// The object's `aequ` (`+0x02`), `aorgvol` (`+0x2a`), `atreon`/`atremor` (`+0x2d`/`+0x2f`),
/// `vibtretype` (`+0x3d`) and `a0clearcnt` (`+0x3e`) are not kept: nothing the modules reach
/// reads them (no `I`, `R`, `S3x` or `S4x` cell, and `_zplaynote` is never called).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Channel {
    /// `achannelused` (`+0x01`): the dispatchers walk a channel only when it is set (`+0x3c4d`,
    /// `+0x3d41`). Set by the row parse (`+0x474a`), cleared by the "volume 0" path (`+0x3ca9`).
    pub used: bool,
    /// `avol` (`+0x1a`): the channel's volume, 0..=63. The row parse clamps the volume column
    /// to `0x3f` (`+0x477b`) and a slide clamps itself (`+0x3f5e`).
    pub volume: u8,
    /// `aspd` (`+0x12`): the period the channel plays.
    pub period: u16,
    /// `asldspd` (`+0x20`): the `G` target.
    pub target_period: u16,
    /// `ac2spd` (`+0x22`): the instrument's `c2spd`, loaded with it (`+0x4850`).
    pub c2spd: u16,
    /// `aorgspd` (`+0x24`): the period at the last note, which `H` modulates and `G` walks, and
    /// which a row with no command or a `D` restores (`+0x3d0c`, `+0x3d24`).
    pub org_period: u16,
    /// `avibcnt` (`+0x26`): the vibrato position. Its low byte steps by two through the
    /// waveform's byte offsets and bit 7 means "stale: restart the phase"; the high byte stays
    /// zero, because only `bl` is ever updated (`+0x4083`).
    pub vibrato_position: u16,
    /// `alasteff1` (`+0x2b`): the last `H` parameter, which `K`'s vibrato half reads.
    pub last_effect1: u8,
    /// `alasteff2` (`+0x2c`): the last `G` parameter (`L`'s source at `+0x3f8f`; no module
    /// carries an `L`).
    pub last_effect2: u8,
    /// `atrigcnt` (`+0x2e`): the `Q` retrigger counter and the `SDx` countdown, one field for
    /// both (`+0x420c`, `+0x4296`).
    pub retrigger_count: u8,
    /// `a0volcut` (`+0x31`): the "volume 0" countdown (`+0x3c97`).
    pub zero_volume_count: u8,
    /// `note` (`+0x32`): the row's note byte, `0xff` for none (`+0x46ad`).
    pub note: u8,
    /// `ins` (`+0x33`): the row's instrument byte, 0 for none (`+0x46b3`).
    pub instrument: u8,
    /// `vol` (`+0x34`): the row's volume column, `0xff` for none.
    pub cell_volume: u8,
    /// `cmd` (`+0x35`): the row's command byte, 0 for none (`+0x3cc0`, `+0x3d4d`).
    pub command: u8,
    /// `info` (`+0x36`): the row's parameter byte. Handlers that fall back on a remembered
    /// parameter write it back here (`+0x3f14`, `+0x401d`, `+0x3fa4`).
    pub info: u8,
    /// `lastins` (`+0x37`): the last instrument that loaded a PCM sample; an `O` does nothing
    /// until one has (`+0x48d2`).
    pub last_instrument: u8,
    /// `lastnote` (`+0x38`): the last note played, which `J` transposes (`+0x41bb`).
    pub last_note: u8,
    /// `alastnfo` (`+0x39`): the last non-zero parameter of any command (`+0x3cb9`).
    pub last_info: u8,
    /// `aadr`'s high word (`+0x06`): the byte the next note starts from. `Oxy` writes
    /// `info << 8` (`+0x48e1`).
    pub sample_offset: u32,
    /// `m_pos` (`+0x44`): where the mixer is in the sample. A note off parks it at 1 and the
    /// "volume 0" path at `0xffff`.
    pub position: u32,
    /// `m_poslow` (`+0x42`): the position's fraction.
    pub position_low: u16,
}

impl Channel {
    /// The song initialiser `_zplaysong` runs on every channel (`+0x5614`, called at `+0x5716`),
    /// and only its stores: every field it does not name keeps its value (D45).
    ///
    /// The stores are `+0x5625`…`+0x5681`, `+0x5781` (which leaves `used` 0) and the silence
    /// pass at `+0x569a`; on SB Pro their other callees store nothing into `_zchn`. Kept:
    /// `sample_offset`, `volume`, `c2spd`, `retrigger_count`, `zero_volume_count` and
    /// `position_low`. `tests/player.rs`'s
    /// `the_song_initialiser_writes_exactly_what_initialise_writes` decodes the stores from the
    /// object and checks this function against them field by field.
    ///
    /// On a boot the kept fields are zero (the object's `_zchn` is zeroed,
    /// `a_boot_starts_on_the_objects_zeroed_channels`); on a restart they are the previous
    /// song's ([`crate::Player::restart_from`]).
    pub fn initialise(&mut self) {
        self.used = false;
        self.period = 0;
        self.target_period = 0;
        self.org_period = 0;
        self.vibrato_position = 0;
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
    /// The row's first tick, per channel, before the command is dispatched (`+0x3c4d`…`0x3d36`):
    /// the "volume 0" test, `alastnfo`, the period restores and the stale-vibrato mark.
    ///
    /// Returns `false` when the channel is skipped: not used, or freed by the "volume 0"
    /// optimisation. The caller then dispatches nothing.
    pub(crate) fn row_start(&mut self, master_flags: u16) -> bool {
        if !self.used {
            return false;
        }
        // The "volume 0" optimisation (`+0x3c77`…`0x3cad`), on when `_np_masterflags`' bit 3
        // is (both modules' flags word is 8).
        if master_flags & 0x8 != 0
            && self.volume == 0
            && self.cell_volume == 0xff
            && self.instrument == 0
            && self.note == 0xff
        {
            // `decb 0x31(%si); jne`: decrement first, so 3 frees it on the third silent row.
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
            // `+0x3cb9`: remembered for any command, so a `D00` after an `F31` slides by 0x31.
            self.last_info = self.info;
        }
        if command == 0 {
            // `+0x3d0c`: no command, so the period goes back to the note's own, which undoes a
            // vibrato or an arpeggio.
            self.restore_period();
            return true;
        }

        if command == 0x4 {
            // `+0x3d24`: a `D` row restores the period too, and then dispatches.
            self.restore_period();
        }
        if !matches!(command, 0x8 | 0xb | 0x12) {
            // `+0x3cea`: mark the vibrato position stale, so the next `H`/`K` restarts the phase.
            self.vibrato_position |= 0x80;
        }
        true
    }

    /// `+0x3d0e`/`+0x3d24`: clear the retrigger counter and restore the last note's period.
    fn restore_period(&mut self) {
        self.retrigger_count = 0;
        if self.period != self.org_period {
            self.period = self.org_period;
        }
    }

    /// Every tick after the row's first (`+0x3d38`): skip a channel with no command or one
    /// above `0x1b` (the file's 255 spelling of "none").
    pub(crate) fn dispatches_on_tick(&self) -> bool {
        self.used && self.command != 0 && self.command <= 0x1b
    }
}

// ---------------------------------------------------------------------------------------
// D — the volume slide, and the two commands that share its body
// ---------------------------------------------------------------------------------------

impl Channel {
    /// `Dxy` (`+0x3f08`), and `Kxy` through the same body. The low nibble slides **down** and
    /// the high nibble **up**, tested low nibble first:
    ///
    /// | info | first tick | later ticks |
    /// |---|---|---|
    /// | `D0y` (y≠0) | `volume -= y` | `volume -= y` |
    /// | `Dx0` (x≠0) | `volume += x` | `volume += x` |
    /// | `DxF` (x≠0, low nibble f) | `volume += x` | nothing |
    /// | `DFx` (x≠0, high nibble f) | `volume -= x` | nothing |
    /// | `D0F`, `DF0` | as `D0y`/`Dx0` with y/x = 15 | the same |
    ///
    /// (`+0x3f22` sends a low `0xf` to the fine-up path `+0x3f73`, `+0x3f27` a high `0xf` to
    /// the fine-down path `+0x3f81`.) A zero info falls back on `last_info` (`+0x3f11`), and
    /// the volume clamps to 0…63 (`+0x3f3c`, `+0x3f5e`).
    pub(crate) fn volume_slide(&mut self, info: u8, tick_in_row: u16) {
        let info = self.info_or_last(info);
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

    /// A zero info falls back on `last_info` and writes it back into the cell (`+0x3f11`,
    /// `+0x3e59`, `+0x41f0`).
    fn info_or_last(&mut self, info: u8) -> u8 {
        if info != 0 {
            info
        } else {
            self.info = self.last_info;
            self.last_info
        }
    }

    /// `+0x3f33`/`+0x3f55`: the slide, clamped to 0…63.
    fn add_volume(&mut self, delta: i16) {
        self.volume = (i16::from(self.volume) + delta).clamp(0, 0x3f) as u8;
    }
}

// ---------------------------------------------------------------------------------------
// E and F — the portamentos
// ---------------------------------------------------------------------------------------

/// Which way a row slides the period: `E` (`+0x3e4f`) grows it, `F` (`+0x3e10`) shrinks it.
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
    /// Nothing happens with no note playing (`aorgspd == 0`, `+0x3e53`); a zero info falls back
    /// on `last_info` (`+0x3e59`). On the first tick only the fine forms move: `0xe1`…`0xf0`
    /// by the low nibble, above `0xf0` by the low nibble times four, and `0xe0` or less not at
    /// all (`+0x3e3b`). On later ticks the step is `info × 4` (`+0x3e6c`), and `0xe0` or more
    /// stops it (`+0x3e6a`). Both `aspd` and `aorgspd` move (`+0x3e70`, `+0x3e73`).
    pub(crate) fn portamento(&mut self, info: u8, tick_in_row: u16, direction: Portamento) {
        if self.org_period == 0 {
            return;
        }
        let info = self.info_or_last(info);
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
    /// `Gxx` (`+0x3f94`), later ticks only: the period walks towards `asldspd` by `info × 4` a
    /// tick, clamped onto the target (`+0x3fbc`, `+0x3fe3`), and `aorgspd` moves with it
    /// (`+0x3fc4`). The parameter is remembered in `alasteff2` (`+0x3fa1`). The object's
    /// interpolation call at `+0x57b0` is gated on `[0x919]`, which no module sets (no `S1x`).
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
    /// `Hxy` (`+0x4005`), later ticks only. A zero high nibble is merged from the previous
    /// `alasteff1` (`+0x4010`…`+0x4018`), so `H08` after `H81` is `H88`.
    ///
    /// The level is `sine × depth >> shift` and the period `aorgspd + level`: the sine at
    /// `+0xa50` is read at the position's byte offset, a stale position (bit 7) reads 0 and
    /// restarts the phase, and the position then steps by `speed × 2`, masked to `0x7e`
    /// (`+0x4060`). `shift` is 5 when the header flags' bit 0 is clear (`+0x4043`, `+0x5448`),
    /// which it is in both modules.
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
        // The position is even, so the object's byte offset is twice the index here.
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
    /// `Jxy` (`+0x4190`): the note cycles base, `+x`, `+y` semitones by `tick_in_row % 3`
    /// (`+0x419a`). The sum carries one octave at most (`+0x41c8`), and only `aspd` is rebuilt,
    /// so `aorgspd` keeps the note's own period.
    pub(crate) fn arpeggio(&mut self, info: u8, tick_in_row: u16, c2spd: u16) {
        let info = self.info_or_last(info);
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
    /// `Qxy` (`+0x41ec`): restart the sample (`m_pos = 0`, `+0x420f`) once `atrigcnt` has
    /// reached `y` (`+0x4205`, so `y = 0` never fires), with `x`'s volume variation: its
    /// multiplier from `+0x96c` if non-zero (codes 6, 7, 14, 15), else its addend from
    /// `+0x95c`. The counter counts every call (`+0x4246`); a zero info falls back on
    /// `last_info` (`+0x41f0`).
    pub(crate) fn retrigger(&mut self, info: u8) {
        let info = self.info_or_last(info);
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
    /// `SDx` on the row's first tick (`+0x4296`): the delay goes into `atrigcnt`.
    pub(crate) fn note_delay(&mut self, info: u8) {
        self.retrigger_count = info & 0x0f;
    }

    /// `SDx` on later ticks (`+0x429a`): count down, and return true on the tick that reaches
    /// zero, where the deferred note starts (`call 0x475e`).
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

/// `+0x593a`: a note byte to a period. `0xfe` (note off) is 0; otherwise the semitone's entry
/// in the table at `+0xa24` (four octaves above the S3M table), shifted by the octave's byte
/// from `+0x97c`.
///
/// The shift table has eight entries and is indexed without a range check (`+0x594a`), so
/// MUSIC1's note `0x80` reads the ninth byte, 0, and plays octave 0's period (`music.md` §9.2).
/// A semitone of 12…15 occurs in neither module; the port clamps it.
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

/// `+0x5813`: `period × 8363/c2spd`, one integer division, answered `0x7fff` when the
/// product's high word reaches the divisor (`+0x581f`, the 8086 `div` guard) or the quotient
/// does not fit a signed word (`+0x5825`).
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

/// The object's base frequency, `0x00da7600` = 14,317,056 (`+0x58b9`: `mov $0xda,%dx;
/// mov $0x7600,%ax; div period`). It is 400 short of `8363 × 1712`, so C-4 at the 8363
/// reference steps at 8362, not 8363 (`music.md` §9.11).
pub(crate) const BASE_FREQ: u32 = 0x00da_7600;

/// `+0x582b`'s step: `BASE_FREQ` over the period, 0 for a period of 0 (`+0x5876`). The
/// mixer divides it by the mixing rate ([`crate::step`]).
pub fn period_step(period: u16) -> u32 {
    if period == 0 {
        return 0;
    }
    BASE_FREQ / u32::from(period)
}

// ---------------------------------------------------------------------------------------
// The object's tables
// ---------------------------------------------------------------------------------------

/// The vibrato sine at `+0xa50`: 64 words, `round(255 × sin(2πi/64))`. It is the only
/// waveform the modules reach: no cell is an `S3x`/`S4x`.
const VIBRATO_SINE: [i16; 64] = [
    0, 24, 49, 74, 97, 120, 141, 161, 180, 197, 212, 224, 235, 244, 250, 253, 255, 253, 250, 244,
    235, 224, 212, 197, 180, 161, 141, 120, 97, 74, 49, 24, 0, -24, -49, -74, -97, -120, -141,
    -161, -180, -197, -212, -224, -235, -244, -250, -253, -255, -253, -250, -244, -235, -224, -212,
    -197, -180, -161, -141, -120, -97, -74, -49, -24,
];

/// The twelve periods at `+0xa24`, four octaves above the S3M table.
const NOTE_PERIODS: [u16; 12] = [
    27392, 25856, 24384, 23040, 21696, 20480, 19328, 18240, 17216, 16256, 15360, 14512,
];

/// The shift table at `+0x97c` as the unchecked lookup reads it: eight entries, then the eight
/// bytes the segment holds after them.
const NOTE_SHIFTS: [u8; 16] = [0, 1, 2, 3, 4, 5, 6, 7, 0, 5, 11, 17, 27, 32, 42, 47];

/// The `Q` volume-variation addends at `+0x95c`, signed. Slots 6, 7, 14 and 15 belong to the
/// multiply codes and are never read.
const RETRIGGER_ADD: [i8; 16] = [0, -1, -2, -4, -8, -16, 0, 0, 0, 1, 2, 4, 8, 16, 0, 0];

/// The `Q` volume-variation multipliers at `+0x96c`, in sixteenths.
const RETRIGGER_MULTIPLY: [u8; 16] = [0, 0, 0, 0, 0, 0, 10, 8, 0, 0, 0, 0, 0, 0, 24, 32];

