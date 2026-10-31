//! The tracker: the tick clock, the position, the order advance and the timeline effects.
//!
//! `upstream/SecondReality/MAIN/STMIK.300` ships as an OMF object with no source, so this is
//! read out of its segment image: every `+0x…` is a segment offset into the image
//! `tests/player.rs`'s `segment_image` rebuilds (`objdump -b binary -m i8086 -D` lists it).
//! Which frame a row change
//! lands on is measured against the traces under `reference/music/` (`music-clock.md`), and where
//! the two disagree the trace wins (D12). `docs/fidelity/music.md` §8 has the readings:
//!
//! - A tick is `mix_rate × 2.5/tempo` samples, an integer division (`+0x3d99`…`+0x3da6`); a row
//!   ends when the tick counter reaches the speed (`+0x4397`…`0x43b0`). A poll's fractional
//!   samples carry to the next.
//! - The published row, pattern, order and `zplus` come through a ten-deep FIFO (`+0x52c9`…
//!   `0x5326`, primed at `+0x2ac1`), fed only by a row advance (`+0x43e6`…`0x43fb`), never by a
//!   seek. Speed, tempo and loop are published directly.
//! - The order advance (`+0x442d`) computes `zplus`, steps over `254` and ends or loops at `255`.
//! - `A` and `T` are separate handlers: `A` (`+0x3dd2`) has no upper bound, and only `T`
//!   (`+0x3d73`) splits at 32. `B` defers its jump to the row's end and `C`'s row is decimal.
//!
//! The channel effects are [`crate::effects`]; this file reads a row's cells, starts its notes and
//! dispatches its commands through the object's two tables.

use crate::effects::{note_period, scaled_period, Channel, Portamento};
use crate::mix::Mixer;
use crate::{Module, CHANNELS, ROWS};

/// The frame clock's denominator: the engine ticks at `70_086/1000` Hz (`sr_engine`'s
/// `HZ_MILLI`), so samples are counted in `1/70_086` of a sample to keep a poll's share exact.
const FRAME_UNITS: u64 = 70_086;

/// The ring's slot count: 256 bytes of eight-byte entries (`+0x2150`, cleared by the
/// `0x80`-iteration loop at `+0x2ab4`).
const FIFO_SLOTS: usize = 32;

/// The ring's fill after the module initialiser: the write pointer at `0x50`, the read
/// pointer at `0x0` (`+0x2ac1`, `+0x2ac8`) — ten entries, so ten polls of delay.
const FIFO_DEPTH: usize = 10;

/// `[0x93a] == 0`: the next row advance is a plain row advance.
const ADVANCE_NONE: u8 = 0;
/// `[0x93a] == 1`: the next row advance is an order advance (`B`, `C`).
const ADVANCE_ORDER: u8 = 1;
/// `[0x93a] == 0xff`: `BFF` — the song ends at the next row advance (`+0x43c9`).
const ADVANCE_STOP: u8 = 0xff;

/// `[0x939] == 0`: stopped.
const STATE_STOPPED: u8 = 0;
/// `[0x939] == 1`: playing.
const STATE_PLAYING: u8 = 1;
/// `[0x939] == 2`: started and no row played yet, where a `255` stops rather than loops
/// (`+0x446e`).
const STATE_STARTED: u8 = 2;

/// The mixing rate `_zmixspeed` (`MAIN/U2.ASM:737-743`, `:759`). The default is the 40000 the
/// reference runs at from the music's start on (`music-clock.md` §5).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum MixRate {
    /// 20000 Hz.
    Hz20000,
    /// 40000 Hz — the measured rate, and the port's default.
    #[default]
    Hz40000,
}

impl MixRate {
    /// The rate in samples a second, as `_zmixspeed` holds it.
    pub fn samples_per_second(self) -> u32 {
        match self {
            MixRate::Hz20000 => 20_000,
            MixRate::Hz40000 => 40_000,
        }
    }
}

/// How the mixer renders the voices. `Enhanced` is the port's own and off by default (D43,
/// `music.md` §13): the same clock, steps and published words, rendered at 48,000 frames a
/// second, interpolated.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Quality {
    /// The object's mixer, bit for bit.
    #[default]
    Authentic,
    /// The port's renderer: the same voices at 48,000 frames a second, interpolated.
    Enhanced,
}

/// The enhanced renderer's output rate in frames a second, 12/5 of the authentic 20,000.
pub const ENHANCED_RATE: u32 = 48_000;

/// The player's settings, fixed for the life of the instance (the original's are set before
/// `_zinit`, `MAIN/STMIK.H:63-96`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PlayerOptions {
    /// What a tick's sample count is computed from.
    pub mix_rate: MixRate,
    /// Stereo output: output mode 5 with the song's stereo bit (`MAIN/STMIK.H:73`).
    pub stereo: bool,
    /// How the voices are rendered. (The object's own interpolation byte `[0x919]` is set only
    /// by `S1x`, which neither module carries, `music.md` §9.12.)
    pub quality: Quality,
}

impl PlayerOptions {
    /// The frames a second the stream plays at: the mixing rate's bytes over the channel count
    /// (20,000 for the reference), or [`ENHANCED_RATE`].
    pub fn output_rate(&self) -> u32 {
        match self.quality {
            Quality::Authentic => self.authentic_rate(),
            Quality::Enhanced => ENHANCED_RATE,
        }
    }

    /// The authentic stream's frames a second: `mix_rate` bytes over the channel count.
    pub(crate) fn authentic_rate(&self) -> u32 {
        self.mix_rate.samples_per_second() / self.channels()
    }

    /// One i16 a frame in mono, two in stereo.
    pub(crate) fn channels(&self) -> u32 {
        if self.stereo {
            2
        } else {
            1
        }
    }
}

impl Default for PlayerOptions {
    fn default() -> Self {
        PlayerOptions {
            mix_rate: MixRate::default(),
            stereo: true,
            quality: Quality::Authentic,
        }
    }
}

/// The words `DIS/DISINT.ASM` reads out of STMIK: `np_ord`, `np_pat`, `np_row`, `np_speed`,
/// `np_tempo`, `np_loop` (`MAIN/STMIK.H:117-131`).
///
/// Order, pattern and row come through the ten-deep ring ([`Player::state`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Position {
    /// `_np_ord`: the order the counter holds.
    pub order: u8,
    /// `_np_pat`: the pattern that order names.
    pub pattern: u8,
    /// `_np_row`: the row the counter holds.
    pub row: u8,
    /// `_np_speed`: the current speed, set by an `A` command.
    pub speed: u8,
    /// `_np_tempo`: the current tempo, set by a `T` command.
    pub tempo: u8,
    /// `_np_loop`: song loops completed (0 in every filed trace, `music-clock.md` §10).
    pub r#loop: u8,
}

/// One ring entry, in the original's order (`+0x52d1`…`+0x52e9`).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
struct Entry {
    row: u16,
    pattern: u16,
    order: u16,
    zplus: u16,
}

/// The S3M tracker, as `MAIN/STMIK.300` implements it.
pub struct Player {
    module: Module,
    options: PlayerOptions,

    /// The sample accumulator in `1/70_086` sample units: a poll adds a frame's worth
    /// (fractional by construction) and every tick takes a whole tick's worth away.
    acc: u64,
    /// The whole samples this poll contributed.
    samples_this_poll: u32,
    /// The tick's length in samples, `[0x8e1]`: `mix_rate × 5/(2 × tempo)`, truncated.
    samples_per_tick: u32,

    /// `[0x940]`: the order the scan will try next — one past the one being played.
    next_order: u16,
    /// `[0x944]`: where a `255` sends the scan. `_zplaysong` clears it (`+0x256b`), so the
    /// shell's plain orders leave it at 0.
    loop_order: u16,
    /// The order being played (the original's `[0x940] - 1`; kept apart because a `B` writes
    /// `next_order` and a `C` does not).
    order: u16,
    /// `[0x94c]`: the pattern being played.
    pattern: u8,
    /// `[0x948]`: the row counter.
    row: u16,
    /// `[0x93b]`: the row the next order advance will enter (`C`).
    pending_row: u8,
    /// `[0x2000]`: ticks into the current row, compared against the speed.
    tick_in_row: u16,
    /// `[0x93a]`: none, an order advance pending, or a `BFF` stop.
    advance_flag: u8,
    /// `[0x938]`: the song is running (the mixing and the row advance both stop when it
    /// clears).
    running: bool,
    /// `[0x939]`: playing, started-but-no-row-yet, or stopped.
    state: u8,
    /// `[0x90b]`, published straight into `_np_loop` at `+0x52c5`.
    loops: u16,
    /// `[0x90d]`, the live `zplus` — fed to the ring only by a row advance.
    zplus: u16,
    /// `_np_speed` and `_np_tempo`, written directly and never delayed.
    pub_speed: u8,
    pub_tempo: u8,
    /// `_np_zframe`, the tick counter the parts set and wait on.
    zframe: i32,
    /// `_np_loop`.
    pub_loop: u8,
    /// True once the order scan has run off the end of the order list.
    at_end: bool,

    /// The ring and its two pointers.
    ring: [Entry; FIFO_SLOTS],
    write: usize,
    read: usize,
    /// The words the ring is fed from (`[0x905]`/`[0x907]`/`[0x909]`/`[0x90d]`) — written
    /// only by a row advance.
    pushed: Entry,
    /// What the last pop published.
    published: Entry,

    /// The object's `_zchn` at `+0x0da0`, indexed by the cell's channel number (the pointer
    /// table at `+0x1fa0` is the identity).
    channels: [Channel; CHANNELS],
    /// `_np_masterflags` (`+0x917`): the header's `flags` word (`+0x53ef`, `+0x5401`), 8 in both
    /// modules. Bit 3 turns the "volume 0" optimisation on.
    master_flags: u16,
    /// The vibrato's level shift (`+0x4043`): 4 when `flags & 1` (`[0x915]`, `+0x5448`), else 5.
    vibrato_shift: u8,

    /// The module's global-volume byte, which the shell's `fademusic` walks down.
    global_volume: u8,

    /// The voices, and the ring the device drains.
    mixer: Mixer,
}

impl Player {
    /// A player for `module`, as the module initialiser leaves one: the header's speed and
    /// tempo (the shell's patch is [`crate::Song::decode`]'s), and the ring primed with ten zero
    /// positions, which the traces show before the first tick.
    pub fn new(module: Module, options: PlayerOptions) -> Player {
        let speed = module.initial_speed();
        let tempo = module.initial_tempo();
        let global_volume = module.global_volume();
        let master_flags = module.flags();
        Player {
            channels: [Channel::default(); CHANNELS],
            master_flags,
            vibrato_shift: if master_flags & 1 == 1 { 4 } else { 5 },
            samples_per_tick: samples_per_tick(options.mix_rate, tempo),
            module,
            options,
            acc: 0,
            samples_this_poll: 0,
            next_order: 0,
            loop_order: 0,
            order: 0,
            pattern: 0,
            row: 0,
            pending_row: 0,
            tick_in_row: 0,
            advance_flag: ADVANCE_NONE,
            running: false,
            state: STATE_STOPPED,
            loops: 0,
            zplus: 0,
            pub_speed: speed,
            pub_tempo: tempo,
            zframe: 0,
            pub_loop: 0,
            at_end: false,
            ring: [Entry::default(); FIFO_SLOTS],
            write: FIFO_DEPTH,
            read: 0,
            pushed: Entry::default(),
            published: Entry::default(),
            global_volume,
            mixer: Mixer::for_options(&options),
        }
    }

    /// `_zplaysong(order)` (`MAIN/STMIK.H:43`; `MAIN/U2.ASM:786`, `:393-394`): play from `order`.
    ///
    /// The original (`+0x2555`) runs the routine at `+0x258e`, which clears the tick-in-row
    /// counter (`+0x56ec`, so a mid-song restart starts its row on the next tick), the pending
    /// row, row, pattern and published row, and runs [`Channel::initialise`] on every channel;
    /// then it sets "started" (`[0x939] = 2`), calls the order advance directly (so a seek
    /// publishes nothing new) and only then sets running (`+0x259a`). A seek onto a `255`
    /// therefore keeps ticking the old pattern.
    pub fn play_song(&mut self, order: u8) {
        self.tick_in_row = 0;
        for channel in &mut self.channels {
            channel.initialise();
        }
        // `+0x5614` clears each channel's mixer half too (`m_base`, `+0x5641`/`+0x5644`).
        self.mixer.reset();
        self.loop_order = 0;
        self.next_order = u16::from(order);
        self.pending_row = 0;
        // `+0x5705` zeroes the published `_np_loop`; its source `[0x90b]` is written only by the
        // `255` path (`+0x4481`), so `loops` stays.
        self.pub_loop = 0;
        self.published.row = 0;
        // `+0x5702`, `+0x5708`: only the order advance sets them again.
        self.row = 0;
        self.pattern = 0;
        self.advance_flag = ADVANCE_NONE;
        self.state = STATE_STARTED;
        self.at_end = false;
        self.enter_order();
        self.running = true;
    }

    /// `restartmus` (`MAIN/U2.ASM:348-397`; D45): take over what the object keeps when it
    /// replaces `previous`'s module with this one, then [`Player::play_song`]. A boot is
    /// `play_song` on a fresh player.
    ///
    /// The object is one player. Its stop, shut-up, load and `_zplaysong` write only what
    /// [`Channel::initialise`] writes into a channel, so the channels carry over, and so do the
    /// loop counter `[0x90b]` and `_np_zframe` (only the playing tick increments it, `+0x4357`).
    /// The speed, tempo, global volume and flags are the new module's.
    ///
    /// Not carried, because what the original puts in them happens during a reload the port
    /// does not have (A53, A55): the position ring and its fed words, and the tick countdown
    /// `[0xbe0]` (the restarted clock's phase is D34's). A55 has the argument and
    /// `a_carried_ring_publishes_what_the_stopped_previous_player_would` the counterfactual.
    pub fn restart_from(&mut self, previous: &Player, order: u8) {
        self.channels = previous.channels;
        self.loops = previous.loops;
        self.zframe = previous.zframe;
        self.play_song(order);
    }

    /// `_zstopsong()` (`MAIN/STMIK.H:49`, `+0x25c5`): stop playing. The port does not tick at
    /// all once stopped, where the object's stopped tick still dispatches effects
    /// (`+0x515f` → `+0x4404`); only tests call it (A55's counterfactual).
    pub fn stop(&mut self) {
        self.running = false;
        self.state = STATE_STOPPED;
    }

    /// The module's global-volume byte (`ds:[30h]`, header offset 48), which the shell's
    /// `fademusic` (`MAIN/U2.ASM:333-346`) walks down.
    pub fn set_global_volume(&mut self, volume: u8) {
        self.global_volume = volume;
    }

    /// The module's global-volume byte as the shell has left it.
    pub fn global_volume(&self) -> u8 {
        self.global_volume
    }

    /// The `index`th channel's state, by S3M channel number.
    pub fn channel(&self, index: usize) -> Option<&Channel> {
        self.channels.get(index)
    }

    /// One video frame has passed: mix its samples and run the ticks they cover.
    ///
    /// The reference runs polled, one `zpollme` a copper frame (`MAIN/U2.ASM:783`,
    /// `MAIN/COPPER.ASM:466`; `music-clock.md` §5). Its order is kept: mix up to a tick
    /// (`+0x510c`), run the tick (`+0x5150`), repeat, then push the position and pop the one ten
    /// entries old (`+0x52c2`…`+0x5326`).
    pub fn poll(&mut self) {
        let before = self.acc / FRAME_UNITS;
        self.acc += u64::from(self.options.mix_rate.samples_per_second()) * 1000;
        self.samples_this_poll = (self.acc / FRAME_UNITS - before) as u32;

        let threshold = u64::from(self.samples_per_tick) * FRAME_UNITS;
        let mut pending = self.samples_this_poll as usize;
        while self.acc >= threshold {
            self.acc -= threshold;
            // Tick-sized chunks keep a row's effects on the samples after them.
            let take = pending.min(self.samples_per_tick as usize);
            self.mix_samples(take);
            pending -= take;
            if self.running {
                self.tick();
            }
        }
        self.mix_samples(pending);

        self.push_position();
        self.pop_position();
    }

    /// Mix `samples` of the object's DMA bytes, one i16 each, into the drain's ring.
    fn mix_samples(&mut self, samples: usize) {
        let options = self.options;
        self.mixer.mix(
            &self.module,
            &mut self.channels,
            &options,
            self.global_volume,
            samples,
        );
    }

    /// The whole samples this poll contributed to the authentic stream, whatever the
    /// [`Quality`]. The fraction carries, so the mean is exactly `40000/70.086` a poll.
    pub fn samples_this_poll(&self) -> u32 {
        self.samples_this_poll
    }

    /// The device's drain: copy up to `out.len()` mixed samples, oldest first and interleaved
    /// in stereo, and return how many. It moves no engine state; a device that falls behind
    /// loses the oldest samples ([`Player::dropped_samples`]), never time.
    pub fn take_samples(&mut self, out: &mut [i16]) -> usize {
        self.mixer.take(out)
    }

    /// How many samples the drain's ring holds before it drops the oldest.
    pub fn ring_capacity(&self) -> usize {
        self.mixer.capacity()
    }

    /// The samples the mixer has produced and the drain has not taken.
    pub fn pending_samples(&self) -> usize {
        self.mixer.pending()
    }

    /// The samples the ring's overflow policy has dropped since the player was made.
    pub fn dropped_samples(&self) -> u64 {
        self.mixer.dropped()
    }

    /// The published words `DIS/DISINT.ASM` reads and the traces record. Order, pattern and row
    /// lag the player's own state by ten polls.
    pub fn state(&self) -> Position {
        Position {
            order: (self.published.order & 0xff) as u8,
            pattern: (self.published.pattern & 0xff) as u8,
            row: (self.published.row & 0xff) as u8,
            speed: self.pub_speed,
            tempo: self.pub_tempo,
            r#loop: self.pub_loop,
        }
    }

    /// `_np_zplus`, the `254` marker, as the parts read it: through the ring with the position.
    pub fn zplus(&self) -> i32 {
        i32::from(self.published.zplus)
    }

    /// The `zplus` the engine computed for the order it plays now (`[0x90d]`, `+0x4452`,
    /// `+0x4461`), before the ring. Tests read it to pin the arithmetic rather than the delay.
    pub fn engine_zplus(&self) -> i32 {
        i32::from(self.zplus)
    }

    /// `_np_zframe`: the tick counter the parts set and wait on (`DIS/DISC.ASM:138-159`). It
    /// advances once a tick, not a frame (`music-clock.md` §8).
    pub fn zframe(&self) -> i32 {
        self.zframe
    }

    /// The options this player was built with, which describe the stream it produces.
    pub fn options(&self) -> PlayerOptions {
        self.options
    }

    /// `_dis_setmframe(frame)` (`DIS/DISC.ASM:138-148`): the parts write the tick counter.
    pub fn set_zframe(&mut self, frame: i32) {
        self.zframe = frame;
    }

    /// True once the order scan has run off the order list (a `255`, or past the end). A finished
    /// player may still be polled.
    pub fn finished(&self) -> bool {
        self.at_end
    }

    /// The player's own position, before the ring: ten polls ahead of [`Player::state`].
    pub fn engine_position(&self) -> Position {
        Position {
            order: (self.order & 0xff) as u8,
            pattern: self.pattern,
            row: self.row as u8,
            speed: self.pub_speed,
            tempo: self.pub_tempo,
            r#loop: self.pub_loop,
        }
    }
}

// -------------------------------------------------------------------------------------------
// The tick
// -------------------------------------------------------------------------------------------

impl Player {
    /// One tick (`+0x4353`): count `_np_zframe` (`+0x4357`), process the row on its first tick
    /// (`+0x4371`) or run its effects on a later one (`+0x4394`), and advance the row when the
    /// counter reaches the speed (`+0x4397`…`0x43b0`).
    fn tick(&mut self) {
        self.zframe = self.zframe.wrapping_add(1);

        if self.tick_in_row == 0 {
            self.process_row();
        } else {
            self.tick_effects();
        }

        self.tick_in_row += 1;
        if self.tick_in_row < u16::from(self.pub_speed) {
            return;
        }
        self.advance_row();
    }

    /// The row's first tick, in the object's three passes: wipe every channel's cell (`+0x46a4`),
    /// read the row's cells and start their notes (`+0x46c2`), then run the commands through the
    /// first-tick table (`+0x3c44`).
    fn process_row(&mut self) {
        for channel in &mut self.channels {
            channel.clear_cell();
        }
        // A pattern past `patNum` is an empty row here, where the object would read past its
        // table. Neither module's order list reaches one.
        let cells = self
            .module
            .pattern(usize::from(self.pattern))
            .and_then(|pattern| pattern.rows.get(usize::from(self.row)))
            .copied();

        if let Some(cells) = cells {
            for (index, &cell) in cells.iter().enumerate() {
                if cell.is_empty() {
                    continue;
                }
                let defers = {
                    let channel = &mut self.channels[index];
                    channel.used = true;
                    channel.install_cell(&cell);
                    let defers = channel.defers_note();
                    if !defers {
                        channel.clamp_cell();
                    }
                    defers
                };
                // `+0x4783`: channels above 15 take the adlib path, off here (`+0x47a6`).
                if !defers && index <= 0x0f {
                    self.start_note(index);
                }
            }
        }

        for index in 0..CHANNELS {
            if !self.channels[index].row_start(self.master_flags) {
                continue;
            }
            self.dispatch(index, true);
        }
    }

    /// Every tick after the row's first (`+0x3d38`): each channel through the second table.
    fn tick_effects(&mut self) {
        for index in 0..CHANNELS {
            if self.channels[index].dispatches_on_tick() {
                self.dispatch(index, false);
            }
        }
    }

    /// One channel's command through this tick's table: `+0x3bd8` on the first tick, `+0x3c0e`
    /// on later ones. `A`, `B`, `C` and `T` are first-tick only; `G`, `H` and `K` later only.
    fn dispatch(&mut self, index: usize, first_tick: bool) {
        let channel = &mut self.channels[index];
        let command = channel.command;
        let info = channel.info;
        let tick = self.tick_in_row;
        let c2spd = channel.c2spd;
        let shift = self.vibrato_shift;
        match (command, first_tick) {
            (1, true) => self.set_speed(info),
            (2, true) => {
                // `B` (`+0x3de2`): the jump lands at the row's end.
                self.advance_flag = ADVANCE_ORDER;
                if info == 0xff {
                    // `BFF` (`+0x3df1`): the song ends at the next row advance.
                    if self.state != STATE_STOPPED {
                        self.advance_flag = ADVANCE_STOP;
                    }
                } else {
                    self.next_order = u16::from(info);
                }
            }
            (3, true) => {
                // `Cxy` (`+0x3dfe`): the row is decimal, unclamped.
                self.pending_row = (info >> 4) * 10 + (info & 0x0f);
                self.advance_flag = ADVANCE_ORDER;
            }
            // `T` (`+0x3d73`): no module carries one (`tests/module.rs`'s
            // `the_effect_histogram_is_the_modules_own`).
            (20, true) => self.set_tempo(info),
            (4, _) => self.channels[index].volume_slide(info, tick),
            (5, _) => self.channels[index].portamento(info, tick, Portamento::Down),
            (6, _) => self.channels[index].portamento(info, tick, Portamento::Up),
            (7, false) => self.channels[index].tone_portamento(info),
            (8, false) => self.channels[index].vibrato(info, shift),
            (10, _) => self.channels[index].arpeggio(info, tick, c2spd),
            // `K` (`+0x3f00`): the volume slide, then a vibrato on `alasteff1`.
            (11, false) => {
                self.channels[index].volume_slide(info, tick);
                self.channels[index].vibrato_last(shift);
            }
            (17, _) => self.channels[index].retrigger(info),
            // `S` (`+0x424e`, `+0x4261`): only `SDx` occurs in the modules (`tests/effects.rs`);
            // any other subcommand falls through to the no-op below.
            (19, true) if info >> 4 == 0x0d => self.channels[index].note_delay(info),
            (19, false) if info >> 4 == 0x0d => self.note_delay_tick(index),
            // `O` has no handler: `start_note` applies it (`+0x48d6`).
            _ => {}
        }
    }

    /// `SDx`'s later ticks (`+0x429a`): the countdown runs on every channel, and the tick that
    /// empties it continues the row parse at `+0x475e` (the clamps, then the note on a mixer
    /// channel, `+0x4783`).
    fn note_delay_tick(&mut self, index: usize) {
        if !self.channels[index].note_delay_tick() {
            return;
        }
        if index <= 0x0f {
            self.channels[index].clamp_cell();
            self.start_note(index);
        }
    }

    /// A cell's instrument and note work on a mixer channel (`+0x4810`): load the instrument
    /// (`+0x4815`…`0x48cb`), then start the note (`+0x48e5`…`0x4975`). `music.md` §9.2:
    ///
    /// - an instrument clears the sample offset (`+0x481d`) before the cell's `O` can set it;
    /// - a note under a `G` on a playing channel only moves the target (`+0x4944`);
    /// - the volume column replaces the volume, including a volume of 0 (`+0x495a`);
    /// - `0xfe` is the note off: position 1, volume 0, target and base periods `0xffff`
    ///   (`+0x4906`…`0x4957`), the live period left where it was.
    fn start_note(&mut self, index: usize) {
        let channel = &mut self.channels[index];
        if channel.instrument != 0 {
            channel.sample_offset = 0;
            channel.last_instrument = channel.instrument;
            let header = self
                .module
                .instrument(usize::from(channel.instrument) - 1)
                .filter(|header| header.is_pcm());
            match header {
                Some(header) => {
                    // `checkins`'s clamp (`STARTEMS.C:139`): the 32-bit `c2spd` into a word
                    // (`+0x4850`), and the volume clamped to 0…63 (`+0x485b`…`0x4867`).
                    channel.c2spd = header.c2spd.min(0xffff) as u16;
                    channel.volume = header.volume.min(0x3f);
                }
                None => channel.last_instrument = 0,
            }
            // `+0x4872`…`0x48cb`: the mixer's voice: which sample, and its bounds.
            self.mixer.start_note(index, &self.module, channel.instrument);
        }
        // `+0x48ce`: nothing to play a note on until an instrument has loaded.
        if channel.last_instrument == 0 {
            return;
        }
        if channel.command == 0x0f {
            channel.set_sample_offset();
        }
        match channel.note {
            // `+0x495a`: no note, only the volume column.
            0xff => {}
            0xfe => {
                channel.position = 1;
                channel.position_low = 0;
                channel.volume = 0;
                channel.org_period = 0xffff;
                channel.target_period = 0xffff;
            }
            note => {
                // `+0x490e`: only a `G` note on a playing channel keeps the position and the
                // period; any other note restarts from the `O` offset (`+0x491e`).
                let restart = channel.org_period == 0 || channel.command != 0x07;
                if restart {
                    channel.position = channel.sample_offset;
                    channel.position_low = 0;
                }
                channel.last_note = note;
                let period = scaled_period(note_period(note), channel.c2spd);
                if restart {
                    channel.period = period;
                    channel.vibrato_position = 0;
                }
                channel.org_period = period;
                channel.target_period = period;
            }
        }
        if channel.cell_volume != 0xff {
            channel.volume = channel.cell_volume;
        }
    }

    /// `A` (`+0x3dd2`: `cmp $0x0,%al; je; xor %ah,%ah; mov %ax,0x2002`): any non-zero parameter
    /// is the speed, with no upper bound, so MUSIC1's `A7F` plays at speed 127 (`bugs.md`
    /// §28.3).
    fn set_speed(&mut self, value: u8) {
        if value != 0 {
            self.pub_speed = value;
        }
    }

    /// `T` (`+0x3d73`): 32 and under goes to the driver call at `+0x3dce` and changes nothing;
    /// above it sets the tempo and the tick's sample count (`+0x3d99`…`0x3da6`). The header
    /// watch calls it with the initial tempo (`+0x53c7`).
    fn set_tempo(&mut self, value: u8) {
        if value > 0x20 {
            self.pub_tempo = value;
            self.samples_per_tick = samples_per_tick(self.options.mix_rate, value);
        }
    }

    /// The row advance (`+0x43b3`…`0x4403`): an order advance for a `B` or `C`, a stop for a
    /// `BFF` (which feeds nothing), or the next row, entering the next order after row 63.
    fn advance_row(&mut self) {
        match self.advance_flag {
            ADVANCE_STOP => {
                self.advance_flag = ADVANCE_NONE;
                self.end_song();
                return;
            }
            ADVANCE_ORDER => {
                self.advance_flag = ADVANCE_NONE;
                self.enter_order();
            }
            _ => {
                if self.row + 1 < ROWS as u16 {
                    self.row += 1;
                } else {
                    self.enter_order();
                }
            }
        }
        self.tick_in_row = 0;
        self.pushed = self.entry_now();
    }

    /// The order advance (`+0x442d`), walking the order list from `[0x940]` with `zplus` clear:
    ///
    /// - `orders[o + 1] == 254` sets bit 0;
    /// - `orders[o] == 254` sets bit 1 and steps over the entry;
    /// - `orders[o] == 255` ends the song if no row has played (`[0x939] == 2`), else counts a
    ///   loop and restarts at the loop order;
    /// - anything else is the pattern to play, from the pending row.
    fn enter_order(&mut self) {
        self.zplus = 0;
        let mut at = usize::from(self.next_order);

        loop {
            let orders = self.module.orders();
            if at >= orders.len() {
                self.end_song();
                return;
            }
            // The object's order counter is nine bits; the scan index is not masked (no module
            // has 512 orders).
            self.next_order = (at as u16 + 1) & 0x1ff;
            let current = orders[at];
            let next = orders.get(at + 1).copied().unwrap_or(0);

            if next == 254 {
                self.zplus |= 1;
            }
            if current == 254 {
                self.zplus |= 2;
                at += 1;
                continue;
            }
            if current == 255 {
                if self.state == STATE_STARTED {
                    self.end_song();
                    return;
                }
                self.loops = self.loops.wrapping_add(1);
                self.pub_loop = self.loops as u8;
                if orders
                    .get(usize::from(self.loop_order))
                    .copied()
                    .unwrap_or(255)
                    == 255
                {
                    // `+0x4499`'s `je 0x447f` skips the stop: the row becomes the loop order and
                    // the player keeps playing. Unreachable in both modules.
                    self.row = self.loop_order;
                    return;
                }
                at = usize::from(self.loop_order);
                continue;
            }

            self.order = at as u16;
            self.pattern = current;
            self.row = u16::from(self.pending_row);
            self.pending_row = 0;
            self.state = STATE_PLAYING;
            self.at_end = false;
            return;
        }
    }

    /// The end of the order list: `+0x4476`…`0x4480` (a `255` met from a fresh start) and
    /// the `BFF` path `+0x43d0`…`0x43da`.
    fn end_song(&mut self) {
        self.running = false;
        self.state = STATE_STOPPED;
        self.pending_row = 0;
        self.at_end = true;
    }

    /// The four words the ring carries; the order is `[0x940] - 1` (`+0x43e6`…`+0x43ef`).
    fn entry_now(&self) -> Entry {
        Entry {
            row: self.row,
            pattern: u16::from(self.pattern),
            order: self.next_order.wrapping_sub(1) & 0x1ff,
            zplus: self.zplus,
        }
    }
}

// -------------------------------------------------------------------------------------------
// The ring
// -------------------------------------------------------------------------------------------

impl Player {
    /// Push what the last row advance fed (`+0x52c9`…`0x52f5`).
    fn push_position(&mut self) {
        self.ring[self.write] = self.pushed;
        self.write = (self.write + 1) % FIFO_SLOTS;
    }

    /// Publish the entry ten pushes old (`+0x52fa`…`0x5326`), and the loop word (`+0x52c5`).
    fn pop_position(&mut self) {
        self.pub_loop = self.loops as u8;
        self.published = self.ring[self.read];
        self.read = (self.read + 1) % FIFO_SLOTS;
    }
}

/// One tick's samples: `_zmixspeed × 0x7d / (0x32 × tempo)`, truncated (`+0x3d99`…`+0x3da6`).
fn samples_per_tick(mix_rate: MixRate, tempo: u8) -> u32 {
    let tempo = u32::from(tempo.max(1));
    mix_rate.samples_per_second() * 5 / (2 * tempo)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_ticks_samples_are_the_objects_own_division() {
        // 40000 × 125/(50 × 120) = 833.33, truncated.
        assert_eq!(samples_per_tick(MixRate::Hz40000, 120), 833);
        assert_eq!(samples_per_tick(MixRate::Hz40000, 130), 769);
        assert_eq!(samples_per_tick(MixRate::Hz20000, 120), 416);
    }

    /// A55's counterfactual: a restart that also carried the ring would publish, poll for poll,
    /// what the stopped previous player publishes, until the new song's first fed row comes
    /// through. `bins/secondreality/tests/loop.rs`'s
    /// `a_carried_ring_would_hand_pass_two_the_old_songs_position` relies on it. The scenario
    /// is that test's: block 5's MUSIC0 from order 25, polled from 408 to the restart at 8898.
    #[test]
    fn a_carried_ring_publishes_what_the_stopped_previous_player_would() {
        let root = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."));
        let module = || crate::SONGS[0].load(root).unwrap();
        let pass_one = || {
            let mut player = Player::new(module(), PlayerOptions::default());
            player.play_song(25);
            for _ in 408..=8898 {
                player.poll();
            }
            player
        };
        let old = pass_one();
        let mut stopped = pass_one();
        stopped.stop();
        let mut carried = Player::new(module(), PlayerOptions::default());
        carried.restart_from(&old, 0);
        // What `restart_from` deliberately leaves behind (A55), carried here only.
        carried.ring = old.ring;
        carried.write = old.write;
        carried.read = old.read;
        carried.pushed = old.pushed;

        let words = |player: &Player| {
            let entry = player.published;
            (entry.order, entry.pattern, entry.row, entry.zplus)
        };
        let mut first_feed = None;
        let mut agreeing = 0;
        for poll in 1..=40 {
            carried.poll();
            stopped.poll();
            if first_feed.is_none() && carried.pushed != old.pushed {
                first_feed = Some(poll);
            }
            if words(&carried) != words(&stopped) {
                break;
            }
            agreeing = poll;
        }
        let first_feed = first_feed.expect("the new song advances a row");
        assert_eq!(
            agreeing,
            first_feed + FIFO_DEPTH - 1,
            "equal until the first fed row comes through the ring"
        );
        assert_eq!((first_feed, agreeing), (9, 18));
        assert_eq!(
            (words(&stopped).0, words(&stopped).2),
            (43, 34),
            "the old song's last fed row, which pass 2's ALKU would read"
        );
    }
}
