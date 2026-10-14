//! The tracker: the tick clock, the position, the order advance and the timeline effects.
//!
//! **Where this file's facts come from.** The original's player ships as an OMF object,
//! `upstream/SecondReality/MAIN/STMIK.300`, with no assembly source anywhere in the tree, so
//! the readings below are of that object's own segment image — every claim carries the offset
//! it was read at, and `crates/sr-audio/tests/player.rs`'s `the_objects_own_bytes_say_what_the_player_says`
//! rebuilds the image out of the object's `LEDATA`/`LIDATA` records and asserts the
//! load-bearing instructions byte for byte. What the object cannot settle — which *frame* a
//! row change lands on — is measured against the per-frame traces Task 1 filed under
//! `reference/music/` (`docs/fidelity/music-clock.md`), and where the two readings disagree
//! the trace wins (D12).
//!
//! Six things this file's shape is, each of them a reading:
//!
//! 1. **A tick is a sample count and a row is a tick count.** The tick's sample count is
//!    `mix_rate × 2.5/tempo` as the original computes it — an integer division
//!    (`+0x3d99`…`0x3da4`: `_zmixspeed × 0x7d/(0x32 × tempo)`). A row ends when a counter of
//!    ticks reaches the *published* speed (`+0x4397`…`0x43b0`: `incw 0x2000`,
//!    `cmp np_speed,0x2000`, `jge`), which is why a speed change lands on the tick it is
//!    processed on, mid-row. A poll's samples are fractional (`mix_rate/70.086` is not an
//!    integer) and the accumulator carries the fraction instead of rounding per poll — which
//!    is what the original's own countdown does (`+0x50f0` and `+0x5157` subtract whole
//!    samples and add a whole tick).
//!
//! 2. **The position the parts read comes through a ten-deep FIFO.** `_np_row`, `_np_pat`,
//!    `_np_ord` and `_np_zplus` are popped from a 256-byte ring of eight-byte entries at
//!    `+0x2150` (`+0x52c9`…`0x5326`), which the module initialiser primes with its write
//!    pointer ten entries ahead of its read pointer (`+0x2ab4` clears the ring; `+0x2ac1`
//!    writes `0x50` to `[0x214e]` and `0x0` to `[0x214c]`). With one `zpollme` per copper
//!    frame (`MAIN/COPPER.ASM:466`) that is a ten-frame delay, and it is visible in the
//!    traces: `second5`'s first row advance is at tick 7 and the trace publishes it at
//!    frame 427. `_np_loop`, `_np_speed` and `_np_tempo` are written **directly**
//!    (`+0x52c5`, `+0x3dd8`, `+0x3d7b`) and are not delayed — which is why the trace's
//!    `speed` word leads its `row` column.
//!
//! 3. **What feeds the ring is a row advance, never a seek.** The fed words — `[0x905]`
//!    (row), `[0x907]` (pattern), `[0x909]` (order), `[0x90d]` (zplus) — are written only on
//!    the tick routine's row-advance path (`+0x43e6`…`0x43fb`). `_zplaysong` calls the
//!    order-advance routine directly (`+0x2597`), and that routine *returns* at `+0x44ee`
//!    without touching them. So after a seek the ring keeps publishing the rows before it —
//!    exactly what `second5` and `second5b` show between the music's start and the first
//!    published row, where nothing but the stale `(0, 0, 0)` appears for nineteen frames.
//!
//! 4. **`zplus` is computed by that same routine and travels with the position**
//!    (`+0x442d`): clear it, then per order-list entry examined, `orders[o+1] == 254` sets bit
//!    0 and `orders[o] == 254` sets bit 1 and steps over the entry. This is where the note's
//!    §6 rule comes from; it is not an inference from the trace's columns.
//!
//! 5. **A `254` is stepped over and a `255` ends the order list.** Neither is stock ST3
//!    semantics guessed at: `+0x4452`…`0x4467` steps over `254` while accumulating `zplus`,
//!    and `+0x4469`…`0x449b` ends the song at `255` — or, if the player had already played a
//!    row, increments `_np_loop`, restarts the scan at the loop order `_zplaysong` recorded
//!    and ends if *that* entry is itself a `255`. No filed trace plays a `255`
//!    (`music-clock.md` §10), so this paragraph is the object's reading, not a measurement,
//!    and `docs/fidelity/music.md` says so.
//!
//! 6. **`A` and `T` are one handler told apart by their parameter** (`+0x3d73`:
//!    `cmp $0x20,%ax; jbe` — 32 and under is a speed, above it a tempo, Scream Tracker's own
//!    convention), `B` defers its jump to the row's end (`+0x3de2`) and `BFF` ends the song,
//!    and `C`'s parameter is **decimal** and unclamped (`+0x3dfe`: `mov %al,%ah; shr $0x4; and
//!    $0xf; aad $0xa`).
//!
//! The channel effects (D, E, F, G, H, J, K, O, Q, S) are Task 4's: a cell carrying one is
//! read and skipped here, and nothing else in this file depends on them.

use crate::{Module, ROWS};

/// The frame clock's denominator: the engine's logic tick is `70_086/1000` Hz
/// (`crates/sr-engine/src/tick.rs`'s `HZ_MILLI`). Samples are counted in `1/70_086` of a
/// sample so that a poll's `mix_rate/70.086` samples stay exact.
const FRAME_UNITS: u64 = 70_086;

/// The ring's slot count: 256 bytes of eight-byte entries (`+0x2150`, cleared by the
/// `0x80`-iteration loop at `+0x2ab4`).
const FIFO_SLOTS: usize = 32;

/// The ring's fill after the module initialiser: the write pointer at `0x50`, the read
/// pointer at `0x0` (`+0x2ac1`, `+0x2ac8`) — ten entries, so ten polls of delay.
const FIFO_DEPTH: usize = 10;

/// `_np_zinfo`'s value. D8's route 1 was performed and the word is **0 in every sample of
/// every filed trace** (`music-clock.md` §6), so D8 records the question as deferred with
/// evidence rather than answered: what its other values are is not measured, and the port
/// does not invent them.
const ZINFO: i32 = 0;

/// `[0x93a] == 0`: the next row advance is a plain row advance.
const ADVANCE_NONE: u8 = 0;
/// `[0x93a] == 1`: the next row advance is an order advance (`B`, `C`, `_zgotosong`).
const ADVANCE_ORDER: u8 = 1;
/// `[0x93a] == 0xff`: `BFF` — the song ends at the next row advance (`+0x43c9`).
const ADVANCE_STOP: u8 = 0xff;

/// `[0x939] == 0`: stopped.
const STATE_STOPPED: u8 = 0;
/// `[0x939] == 1`: playing.
const STATE_PLAYING: u8 = 1;
/// `[0x939] == 2`: started and no row played yet — the state a `255` stops in rather than
/// looping through (`+0x446e`).
const STATE_STARTED: u8 = 2;

/// The mixing rate: one of the four the shell offers (`MAIN/U2.ASM:737-743`, `:759`).
///
/// The port's default is the one the music *itself* runs at, measured on the guest rather
/// than read off the shell's menu order: `_zmixspeed` holds 20000 before the demo's music
/// engine starts and **40000** from the music's start on (`music-clock.md` §5,
/// `cargo xtask music probe --arg 5 --cycles 30000 --frames 408..1300 --at 0x5AC5`).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum MixRate {
    /// 12000 Hz.
    Hz12000,
    /// 16000 Hz.
    Hz16000,
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
            MixRate::Hz12000 => 12_000,
            MixRate::Hz16000 => 16_000,
            MixRate::Hz20000 => 20_000,
            MixRate::Hz40000 => 40_000,
        }
    }
}

/// The player's settings, fixed for the life of the instance — the original's are set before
/// `_zinit` (`MAIN/STMIK.H:63-96`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PlayerOptions {
    /// What a tick's sample count is computed from.
    pub mix_rate: MixRate,
    /// Stereo output: output mode 5 with the song's stereo bit (`MAIN/STMIK.H:73`).
    pub stereo: bool,
    /// Interpolate when mixing. Task 5's setting; nothing here reads it.
    pub interpolation: bool,
}

impl Default for PlayerOptions {
    fn default() -> Self {
        PlayerOptions {
            mix_rate: MixRate::default(),
            stereo: true,
            interpolation: false,
        }
    }
}

/// The words `DIS/DISINT.ASM` reads out of STMIK: `np_ord`, `np_pat`, `np_row`, `np_speed`,
/// `np_tempo`, `np_loop` (`MAIN/STMIK.H:117-131`).
///
/// All six are the *published* words, so the three position ones come through the ten-deep
/// ring and lag the player's own state by ten polls — see [`Player::state`].
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
    /// `_np_loop`: song loops completed. The shell restarts the music before either module
    /// reaches a `255` (`music-clock.md` §10), so every filed trace reads 0.
    pub r#loop: u8,
}

/// One ring entry: the four words that travel through the FIFO together, in the order the
/// original stores them (`+0x52d1`…`+0x52e9`: row, pattern, order, zplus).
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
    /// The whole samples this poll contributed — Task 5's mixer consumes them.
    samples_this_poll: u32,
    /// The tick's length in samples, `[0x8e1]`: `mix_rate × 5/(2 × tempo)`, truncated.
    samples_per_tick: u32,

    /// `[0x940]`: the order the scan will try next — one past the one being played.
    next_order: u16,
    /// `[0x944]`: where a `255` sends the scan. `_zplaysong` clears it (`+0x256b`), so the
    /// shell's plain orders leave it at 0.
    loop_order: u16,
    /// The order being played — the one the last order advance entered. (The original keeps
    /// this as `[0x940] - 1`; the port keeps both, because the excess is what a `B` writes and
    /// a `C` does not.)
    order: u16,
    /// `[0x94c]`: the pattern being played.
    pattern: u8,
    /// `[0x948]`: the row counter.
    row: u16,
    /// `[0x93b]`: the row the next order advance will enter (`_zgotosong`, `C`).
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

    /// The module's global-volume byte, which the shell's `fademusic` walks down.
    global_volume: u8,
    /// `_zshutup` was called: the voices are silent. Task 5 owns the voices; this records the
    /// request so the engine seam can see it.
    silenced: bool,
}

impl Player {
    /// A player for `module`, left the way the module initialiser leaves one.
    ///
    /// The player starts at the module's own speed and tempo bytes; the shell's patch
    /// (`MAIN/STARTMUS.C:257`, MUSIC0's 125 → 120) belongs to the caller — see
    /// [`Module::apply_tempo_patch`]. The position words start at zero, and the ring is
    /// primed ten entries deep with those zeroes, which is what the traces show before the
    /// music's first tick: `second5` frames 148…426 read `ord 0 row 0 pat 0`.
    pub fn new(module: Module, options: PlayerOptions) -> Player {
        let speed = module.initial_speed();
        let tempo = module.initial_tempo();
        let global_volume = module.global_volume();
        Player {
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
            silenced: false,
        }
    }

    /// `_zplaysong(order)` (`MAIN/STMIK.H:43`; the initial start at `MAIN/U2.ASM:786` and
    /// `restartmus`'s call at `:393-394`): play from `order` now.
    ///
    /// The original's routine (`+0x2555`) clears the loop order, masks the argument to nine
    /// bits into `[0x940]`, sets `[0x939] = 2` ("started, no row played yet"), calls the
    /// order-advance routine **directly** — which is why a seek publishes no position: the
    /// ring is fed only by a row advance (this module's header, point 3) — and only then sets
    /// `[0x938] = 1` (`+0x259a`). The routine it calls at `+0x258e` clears `[0x93b]`, the row
    /// counter, the pattern, the loop counter and **`[0x8fd]`**, the published row
    /// (`+0x5711`, `+0x570b`).
    ///
    /// The order of those two is load-bearing: a seek that lands on a `255` takes the
    /// scan's stop path (`+0x4476` clears `[0x938]`) and is then put back to running by
    /// `+0x259a`, so the player ends up *ticking with no new order entered* — the old pattern
    /// still playing, the position where it was. That is what the port does too.
    pub fn play_song(&mut self, order: u8) {
        self.loop_order = 0;
        self.next_order = u16::from(order);
        self.pending_row = 0;
        self.loops = 0;
        self.pub_loop = 0;
        self.published.row = 0;
        self.advance_flag = ADVANCE_NONE;
        self.state = STATE_STARTED;
        self.at_end = false;
        self.enter_order();
        self.running = true;
    }

    /// `_zgotosong(order, row)` (`MAIN/STMIK.H:46`): seek to `order`, `row`.
    ///
    /// The original (`+0x2513`) writes the row into `[0x93b]`, the order into `[0x940]`,
    /// raises the order-advance flag, sets `[0x2000] = np_speed` so the very next tick enters
    /// the row, and sets `[0x939] = 2`. It clears **nothing** else — in particular not the
    /// published row, which only `_zplaysong`'s own path reaches (`+0x570b`, inside the
    /// routine called at `+0x258e`), because `_zgotosong` does not call it.
    pub fn goto_song(&mut self, order: u8, row: u8) {
        self.pending_row = row;
        self.next_order = u16::from(order);
        self.advance_flag = ADVANCE_ORDER;
        self.tick_in_row = u16::from(self.pub_speed);
        self.running = true;
        self.state = STATE_STARTED;
        self.at_end = false;
    }

    /// `_zstopsong()` (`MAIN/STMIK.H:49`): stop playing, keeping the mixing engine on.
    ///
    /// The original (`+0x25e3`) silences every channel and clears `[0x938]`; its ticks then
    /// take the not-playing path (`+0x4404`), which counts but never advances a row and never
    /// touches `_np_zframe` — which is why the tick counter is still 0 at the music's start.
    pub fn stop(&mut self) {
        self.running = false;
        self.state = STATE_STOPPED;
    }

    /// `_zshutup()` (`MAIN/STMIK.H:39`): "shuts up all the sounds playing without
    /// deinitializing the player" — the voices are Task 5's, so this records the request.
    pub fn shut_up(&mut self) {
        self.silenced = true;
    }

    /// True after [`Player::shut_up`] and until a voice is started (Task 5's business).
    pub fn is_silenced(&self) -> bool {
        self.silenced
    }

    /// The shell's `fademusic` (`MAIN/U2.ASM:333-346`) walks the **module's own**
    /// global-volume byte (`ds:[30h]` — the S3M header's offset 48, `STARTEMS.C:63`) down
    /// towards zero; this is that byte.
    pub fn set_global_volume(&mut self, volume: u8) {
        self.global_volume = volume;
    }

    /// The module's global-volume byte as the shell has left it.
    pub fn global_volume(&self) -> u8 {
        self.global_volume
    }

    /// One video frame has passed: mix this frame's samples and advance the tracker by
    /// however many whole ticks that covers.
    ///
    /// The reference is run **polled**: `_zpollmix` is 0 at the module's load and **1 from
    /// the music's start on** (`MAIN/U2.ASM:783` writes it and `MAIN/COPPER.ASM:466` calls
    /// `zpollme` once per copper frame), so one `poll` per logic tick is the faithful model
    /// rather than a simplification of one (`music-clock.md` §5).
    ///
    /// Inside the original's `zpollme` the work is ordered: mix, doing a tick whenever the
    /// sample countdown runs out (`+0x5132`…`0x515c`); then **push** the position and **pop**
    /// the one ten entries old (`+0x52c2`…`0x5326`). That order is kept here.
    pub fn poll(&mut self) {
        let before = self.acc / FRAME_UNITS;
        self.acc += u64::from(self.options.mix_rate.samples_per_second()) * 1000;
        self.samples_this_poll = (self.acc / FRAME_UNITS - before) as u32;

        let threshold = u64::from(self.samples_per_tick) * FRAME_UNITS;
        while self.acc >= threshold {
            self.acc -= threshold;
            if self.running {
                self.tick();
            }
        }

        self.push_position();
        self.pop_position();
    }

    /// The whole samples this poll contributed — what Task 5's mixer mixes. Fractional
    /// samples are carried into the next poll rather than rounded away, so the rate does not
    /// drift by construction: at 40000 Hz this reads 570 four polls in five and 571 on the
    /// fifth.
    pub fn samples_this_poll(&self) -> u32 {
        self.samples_this_poll
    }

    /// The player's published words — the ones `DIS/DISINT.ASM` reads and the traces record.
    ///
    /// Order, pattern and row come through the ring and lag the player's own state by ten
    /// polls; speed, tempo and loop are written straight through.
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

    /// `_np_zplus`: the marker value the order list's `254` entries produce. Computed by the
    /// order advance and delayed with the position, through the same ring the position travels
    /// (this module's header, point 4) — so this is the value the *parts* read, ten polls after
    /// the engine computed it.
    pub fn zplus(&self) -> i32 {
        i32::from(self.published.zplus)
    }

    /// The `zplus` the engine computed for the order it is playing now — the value the ring
    /// will hand out [`FIFO_DEPTH`] polls from now. The object's own word is `[0x90d]`, written
    /// by the order advance's two `orw`s (`+0x4452`, `+0x4461`), and the port's tests read it
    /// here to pin the arithmetic rather than the delay.
    pub fn engine_zplus(&self) -> i32 {
        i32::from(self.zplus)
    }

    /// `_np_zinfo`. Zero, and D8 says why (see [`ZINFO`]).
    pub fn zinfo(&self) -> i32 {
        ZINFO
    }

    /// `_np_zframe`: the tick counter the parts set and wait on
    /// (`DIS/DISC.ASM:138-159`, forwarded by `DIS/DISINT.ASM:412-425`). It advances one per
    /// **tick**, not per frame — measured at 47.93, 48.04 and 52.01 a second for the traces'
    /// two tempos, i.e. exactly `2.5/tempo` (`music-clock.md` §8).
    pub fn zframe(&self) -> i32 {
        self.zframe
    }

    /// `_dis_setmframe(frame)` (`DIS/DISC.ASM:138-148`): the parts write this word, which is
    /// why it can go backwards mid-song — `second3` frame 2452 reads 1516 → 0 with the same
    /// pattern still playing.
    pub fn set_zframe(&mut self, frame: i32) {
        self.zframe = frame;
    }

    /// True once the order scan has run off the order list: a `255`, or past the last entry.
    ///
    /// A finished player keeps polling — the original's ticks still run, they just never
    /// advance a row — so this is "the song is over", not a promise that [`Player::poll`] may
    /// not be called again.
    pub fn finished(&self) -> bool {
        self.at_end
    }

    /// The player's own position, before the ring — what the next row advance would feed.
    ///
    /// The parts never see this and neither does anything the trace records: it is the
    /// engine's row, ten polls ahead of [`Player::state`]. It exists so a reader — a test, or
    /// the engine seam that has to hand the row to the mixer — can ask what is coming.
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
    /// One tick: `+0x4353`, called from the mixing loop's `call 0x4353` (`+0x5150`,
    /// `+0x51b1`).
    ///
    /// The tick increments `_np_zframe` (`+0x4357`), processes the row's cells on the row's
    /// *first* tick (`+0x4371`: `cmpw $0x0,[0x2000]` — zero means first, and that is where an
    /// `A` command writes `_np_speed`), counts the tick, and advances the row when the
    /// counter reaches the speed (`+0x4397`…`0x43b0`).
    fn tick(&mut self) {
        self.zframe = self.zframe.wrapping_add(1);

        if self.tick_in_row == 0 {
            self.process_row();
        }

        self.tick_in_row += 1;
        if self.tick_in_row < u16::from(self.pub_speed) {
            return;
        }
        self.advance_row();
    }

    /// The row's first tick: read the row's cells and apply what this file owns.
    ///
    /// The original's row-0 path (`+0x4378`…`0x4385`) walks the 32 channel structures and
    /// dispatches each cell's command through the table at `+0x3c0e`.
    fn process_row(&mut self) {
        self.silenced = false;
        let Some(pattern) = self.module.pattern(usize::from(self.pattern)) else {
            return;
        };
        let Some(cells) = pattern.rows.get(usize::from(self.row)) else {
            return;
        };
        // The channel structures are walked in channel order and every cell's command is
        // dispatched as it is met; the commands are copied out so the borrow ends here.
        let commands: [(u8, u8); crate::CHANNELS] = std::array::from_fn(|channel| {
            let cell = cells[channel];
            if cell.has_command() {
                (cell.command, cell.info)
            } else {
                (0, 0)
            }
        });
        for (command, info) in commands {
            match command {
                1 => self.set_speed_or_tempo(info),
                2 => {
                    // `B` (`+0x3de2`): the flag goes up now, so the jump lands at the row's
                    // end and the row still plays out.
                    self.advance_flag = ADVANCE_ORDER;
                    if info == 0xff {
                        // `BFF` (`+0x3df1`): the song ends at the next row advance, unless
                        // the player is already stopped.
                        if self.state != STATE_STOPPED {
                            self.advance_flag = ADVANCE_STOP;
                        }
                    } else {
                        self.next_order = u16::from(info);
                    }
                }
                3 => {
                    // `Cxy` (`+0x3dfe`): the row is **decimal** and is stored unclamped.
                    self.pending_row = (info >> 4) * 10 + (info & 0x0f);
                    self.advance_flag = ADVANCE_ORDER;
                }
                // Task 4's: `D`/`E`/`F`/`G`/`H`/`J`/`K`/`O`/`Q`/`S` never move the timeline.
                _ => {}
            }
        }
    }

    /// `A`/`T`, one handler: `+0x3d73`, whose `cmp $0x20,%ax; jbe` sends 32 and under to the
    /// speed and everything above it to the tempo.
    fn set_speed_or_tempo(&mut self, value: u8) {
        if value == 0 {
            return;
        }
        if value <= 0x20 {
            self.pub_speed = value;
        } else {
            self.pub_tempo = value;
            // The tick's sample count is recomputed by the tempo setter (`+0x3d99`…`0x3da4`)
            // and only there; the accumulator's carried remainder is what lets a tempo change
            // land mid-row without a jump.
            self.samples_per_tick = samples_per_tick(self.options.mix_rate, value);
        }
    }

    /// The row advance: `+0x43b3`…`0x4403`.
    ///
    /// Three ways in, exactly as the original has them: the advance flag is `1` (a `B`, a
    /// `C`, a seek), the flag is `0xff` (`BFF` — stop, without feeding the ring), or the row
    /// counter has passed 63 (an order advance). Only the two advancing paths feed the ring,
    /// which is why a seek publishes nothing.
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

    /// The order advance: `+0x442d`, the routine `_zplaysong` calls directly.
    ///
    /// Clear `zplus`; then, walking the order list from the order `[0x940]` names:
    ///
    /// - `orders[o + 1] == 254` sets bit 0 — the note's §6 "1 while playing an order whose
    ///   next entry is `254`";
    /// - `orders[o] == 254` sets bit 1 and the walk steps over the entry, recomputing the
    ///   bits on the next pass, so a `254` behind a `254` cannot be missed;
    /// - `orders[o] == 255` ends the song when no row has been played yet (`[0x939] == 2`),
    ///   and otherwise increments `_np_loop`, restarts the walk at the loop order and ends if
    ///   that entry is a `255` too;
    /// - anything else is the pattern to play, entered at the row `[0x93b]` holds (which the
    ///   walk clears).
    ///
    /// The order kept for the ring is `[0x940] - 1`: the original's word is one *ahead* of
    /// the order being played (`+0x4435`…`0x443e`, published at `+0x43e6`…`0x43ef`).
    fn enter_order(&mut self) {
        self.zplus = 0;
        let mut at = usize::from(self.next_order);

        loop {
            let orders = self.module.orders();
            if at >= orders.len() {
                self.end_song();
                return;
            }
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
                if orders.get(usize::from(self.loop_order)).copied().unwrap_or(255) == 255 {
                    // `+0x4499`'s `je 0x447f` — and `0x447f` is *past* the stop at
                    // `+0x4476`, so this returns to the caller with the loop order in `ax`
                    // (the caller's `mov %ax,0x948` makes the row the loop order) and the
                    // pattern, the order and the mixing flag all untouched: the scan gives
                    // up, the player keeps playing what it was playing. Unreachable in both
                    // modules (neither has a `255` at its first order) and unmeasured.
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

    /// The four words the ring carries for the player's current position.
    fn entry_now(&self) -> Entry {
        Entry {
            row: self.row,
            pattern: u16::from(self.pattern),
            order: self.order & 0x1ff,
            zplus: self.zplus,
        }
    }
}

// -------------------------------------------------------------------------------------------
// The ring
// -------------------------------------------------------------------------------------------

impl Player {
    /// Push the position — `+0x52c9`…`0x52f5`. The value is what a row advance last wrote,
    /// not the player's live state.
    fn push_position(&mut self) {
        self.ring[self.write] = self.pushed;
        self.write = (self.write + 1) % FIFO_SLOTS;
    }

    /// Pop the entry ten pushes old and publish it — `+0x52fa`…`0x5326`. The loop word is
    /// written straight through from `[0x90b]` first (`+0x52c5`).
    fn pop_position(&mut self) {
        self.pub_loop = self.loops as u8;
        self.published = self.ring[self.read];
        self.read = (self.read + 1) % FIFO_SLOTS;
    }
}

/// One tick's worth of samples, as the original computes it: `_zmixspeed × 2.5/tempo`,
/// truncated (`+0x3d99`…`0x3da4`, whose `mul`/`div` pair is `_zmixspeed × 0x7d` over
/// `0x32 × tempo` — `mixspeed × 125/(50 × tempo)`, an integer division).
fn samples_per_tick(mix_rate: MixRate, tempo: u8) -> u32 {
    let tempo = u32::from(tempo.max(1));
    mix_rate.samples_per_second() * 5 / (2 * tempo)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_ticks_samples_are_the_objects_own_division() {
        // 40000 × 125/(50 × 120) = 833.33, truncated to 833 — the multiplication and the
        // integer division the object performs, not a float.
        assert_eq!(samples_per_tick(MixRate::Hz40000, 120), 833);
        assert_eq!(samples_per_tick(MixRate::Hz40000, 130), 769);
        assert_eq!(samples_per_tick(MixRate::Hz20000, 120), 416);
    }
}
