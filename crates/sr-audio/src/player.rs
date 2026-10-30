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
//!    (`+0x3d99`…`+0x3da6`, one of the claims
//!    `tests/player.rs` asserts: `_zmixspeed × 0x7d/(0x32 × tempo)`). A row ends when a counter of
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
//! 6. **`A` and `T` are two handlers with two table entries, not one told apart by its
//!    parameter.** This file's first reading of them was the other way round and Task 4
//!    corrected it: `A` is the first-tick table's entry 1 and its whole body is `+0x3dd2` —
//!    `cmp $0x0,%al; je; xor %ah,%ah; mov %ax,%cs:0x2002` — with **no upper bound** on the
//!    parameter, while `T` is entry 20 and only it has the `cmp $0x20,%ax; jbe 0x3dce` split
//!    (32 and under goes to the driver call and never to the speed). The object's own header
//!    watch is the second witness: it calls `+0x3dd2` for the module's speed byte and
//!    `+0x3d73` for its tempo byte. `B` defers its jump to the row's end (`+0x3de2`) and `BFF`
//!    ends the song, and `C`'s parameter is **decimal** and unclamped (`+0x3dfe`: `mov %al,%ah;
//!    shr $0x4; and $0xf; aad $0xa`).
//!
//! The channel effects (D, E, F, G, H, J, K, O, Q, S) live in [`crate::effects`], on the
//! per-channel state this file owns and drives: a row's cells are read and installed first
//! (`start_note`), and then the row's commands are dispatched channel by channel through the
//! two tables the object keeps — the first-tick pass and the later-tick pass. Nothing here
//! moves the timeline on a channel effect's behalf.

use crate::effects::{note_period, scaled_period, Channel, Portamento};
use crate::mix::Mixer;
use crate::{Module, CHANNELS, ROWS};

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

/// How the mixer renders the voices: the object's own arithmetic, or the port's enhanced
/// renderer.
///
/// **`Enhanced` is the port's own and off by default** (D43, `docs/fidelity/music.md` §13). It
/// keeps the authentic clock exactly — the tick schedule, every voice's step and every published
/// word are the authentic path's — and changes only how a voice's samples become output: 48,000
/// frames a second, linearly interpolated, through the curve's small-signal gain rather than its
/// table. The authentic path is the reference; nothing compared against the original is mixed
/// by the enhanced one.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Quality {
    /// The object's mixer, bit for bit: one sample byte a frame, at 20,000 frames a second for
    /// the reference's stereo `Hz40000`, through the curve the machine holds.
    #[default]
    Authentic,
    /// The port's renderer: the same voices at 48,000 frames a second, interpolated.
    Enhanced,
}

/// The enhanced renderer's output rate, in frames a second — [`PlayerOptions::output_rate`]
/// for [`Quality::Enhanced`]. For the reference's stereo `Hz40000` it is 12/5 of the authentic
/// 20,000.
pub const ENHANCED_RATE: u32 = 48_000;

/// The player's settings, fixed for the life of the instance — the original's are set before
/// `_zinit` (`MAIN/STMIK.H:63-96`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PlayerOptions {
    /// What a tick's sample count is computed from.
    pub mix_rate: MixRate,
    /// Stereo output: output mode 5 with the song's stereo bit (`MAIN/STMIK.H:73`).
    pub stereo: bool,
    /// How the voices are rendered ([`Quality`]); authentic by default. This replaced an
    /// `interpolation` flag nothing read, which stood for the object's `[0x919]` — the byte
    /// `S1x` writes and only the interpolating dispatch entries read. Neither module carries an
    /// `S1x` (`music.md` §9.12), so that byte is 0 on every run and is not modelled.
    pub quality: Quality,
}

impl PlayerOptions {
    /// The frames a second the stream these options produce plays at — what the sound card and
    /// the WAV dump are opened at.
    ///
    /// Authentic: the mixing rate in the object's bytes, over the channel count — 20,000 for
    /// the reference's stereo `Hz40000` (`music.md` §10.2: "40000 bytes of two"). Enhanced:
    /// [`ENHANCED_RATE`], 48,000, whatever the authentic rate is.
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

    /// The channels the effects run on: the object's `_zchn` at `+0x0da0`, one 0x60-byte
    /// entry per channel (the object's array is 48 long; a module can name 32, and the cell's
    /// channel number *is* the array index — the pointer table at `+0x1fa0` is the identity).
    channels: [Channel; CHANNELS],
    /// `_np_masterflags` (`+0x917`): the module header's `flags` word, which the header watch
    /// copies in (`+0x53ef`, `+0x5401`). Its bit 3 turns the "volume 0" optimisation on, and
    /// **both modules carry 8**, so it is on.
    master_flags: u16,
    /// `[0x915]`: `flags & 1` (`+0x5448`, `+0x5450`), which the vibrato's level shift reads
    /// (`+0x4043`): **4 when it is 1 and 5 when it is 0**. Both modules' flags word is 8, so
    /// every filed channel vibratos at the shift of 5 — the quieter of the two forms.
    vibrato_shift: u8,

    /// The module's global-volume byte, which the shell's `fademusic` walks down.
    global_volume: u8,

    /// The mixing engine: the voices, and the ring the device drains (Task 5).
    mixer: Mixer,
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

    /// `_zplaysong(order)` (`MAIN/STMIK.H:43`; the initial start at `MAIN/U2.ASM:786` and
    /// `restartmus`'s call at `:393-394`): play from `order` now.
    ///
    /// The original's routine (`+0x2555`) clears the loop order, masks the argument to nine
    /// bits into `[0x940]`, sets `[0x939] = 2` ("started, no row played yet"), calls the
    /// order-advance routine **directly** — which is why a seek publishes no position: the
    /// ring is fed only by a row advance (this module's header, point 3) — and only then sets
    /// `[0x938] = 1` (`+0x259a`). The routine it calls at `+0x258e` clears the pending row
    /// (`+0x5711`, `[0x93b]`), the row counter (`+0x5702`, `[0x948]`), the pattern (`+0x5708`,
    /// `[0x94c]`), the loop counter, the tick-in-row counter (`+0x56ec`, `[0x2000]`) and the
    /// published row (`+0x570b`, `[0x8fd]`).
    ///
    /// The order of those two is load-bearing: a seek that lands on a `255` takes the
    /// scan's stop path (`+0x4476` clears `[0x938]`) and is then put back to running by
    /// `+0x259a`, so the player ends up *ticking with no new order entered* — the old pattern
    /// still playing, the position where it was. That is what the port does too.
    ///
    /// That routine also **runs the song initialiser on every channel** (`+0x5614`, called at
    /// `+0x5716`; [`Channel::initialise`]): the period, the remembered parameters, the vibrato
    /// position, `lastins` and the cell fields go back to zero — **and the volume `avol`, its
    /// "volume 0" countdown `a0volcut` and the effect counters do not**, because no store in
    /// the routine names them. On a fresh player those are zero (a boot, D45); after
    /// [`Player::restart_from`] they are the previous song's.
    ///
    /// **And it clears `[0x2000]`, the tick-in-row counter** (`+0x56ec`, `movw $0x0,0x2000` —
    /// the routine's own first instruction). The port mirrors it, because the shell's restarts
    /// are seeks made *mid-song*: `restartmus` (`MAIN/U2.ASM:348-397`) is reached with the
    /// player part-way through a row, and a stale counter would begin the new order's row 0 on
    /// a later tick — `+0x4371`'s `cmpw $0x0,0x2000` would be false, so the row's cells would
    /// not be processed at all. Nothing else in the port seeks mid-song, so on a fresh player
    /// this line changes nothing; it is load-bearing for the shell's restart.
    pub fn play_song(&mut self, order: u8) {
        // `+0x56ec`: the counter is zeroed first, before anything else the routine does.
        self.tick_in_row = 0;
        for channel in &mut self.channels {
            channel.initialise();
        }
        // `+0x5614` clears each channel's mixer half too — `m_base` at `+0x5641`/`+0x5644` —
        // so a seek is a fresh start for the voices as well as for the effects.
        self.mixer.reset();
        self.loop_order = 0;
        self.next_order = u16::from(order);
        self.pending_row = 0;
        // `+0x5705` zeroes the published `_np_loop` (`[0x903]`); the counter it is republished
        // from at every poll, `[0x90b]` (`+0x52c2`), is written by nothing but the `255` path's
        // `incw` (`+0x4481`), so `loops` is left as it is (0 on every filed run).
        self.pub_loop = 0;
        self.published.row = 0;
        // The row and the pattern are zeroed by that same routine (`+0x5702`, `+0x5708`) and
        // are only given values again by the order advance — so a seek that lands on a `255`
        // (the scan returning without entering an order) leaves the engine at row 0 of pattern
        // 0, and feeds those. Unreachable in both modules and unmeasured; the port follows the
        // bytes.
        self.row = 0;
        self.pattern = 0;
        self.advance_flag = ADVANCE_NONE;
        self.state = STATE_STARTED;
        self.at_end = false;
        self.enter_order();
        self.running = true;
    }

    /// `restartmus` (`MAIN/U2.ASM:348-397`; D45): take over the state the object keeps when it
    /// replaces `previous`'s module with this one, then `_zplaysong(order)` —
    /// [`Player::play_song`], which writes what the song initialiser writes over it. One call,
    /// so the seek cannot come before the take-over; a boot is `play_song` on a fresh player.
    ///
    /// The object is one player, not one per module. `restartmus` runs `_zstopsong`
    /// (`+0x25c5`: `[0x939] = 0`, nothing else), `_zshutup` (`+0x25e3`: the song initialiser
    /// `+0x5614`, a DMA-buffer clear `+0x2922`, `[0x938] = 0`), then the loader's
    /// `_zinitmodule` (`+0x2400`: `+0x56bc`, which ends in the initialiser, and the instrument
    /// loads) and `_zplaysong(order)` (`+0x2555`: the header watch `+0x5424`, `+0x56bc` again,
    /// the order advance). Every one of those writes what [`Channel::initialise`] writes and
    /// nothing more into a channel, so a restart keeps, per channel, `avol`, `a0volcut`, the
    /// effect counters and the other fields that table lists as kept. Globally it keeps:
    ///
    /// - the loop counter `[0x90b]`, which only the `255` path writes (`+0x4481`);
    /// - `_np_zframe` (`+0x8fb`), which only the *playing* tick increments (`+0x4357`) — the
    ///   reload's ticks take the stopped path (`+0x515f` → `+0x4404`) — and which every part
    ///   that reads it sets first (`GLENZ/MAIN.C:291`, `PLZPART/PLZ.C:69`, `TECHNO/KOE.C:214`).
    ///
    /// Everything else is the new module's, as [`Player::new`] reads it: the speed, tempo and
    /// global volume (the header watch's cached bytes are reset at `+0x5424` so all three are
    /// read again), the master flags and the vibrato shift (both modules' flags word is 8).
    ///
    /// **Two pieces of global state the object also keeps are not carried, because what the
    /// original puts in them happens during the reload, which the port does not have** (A53:
    /// the original spends 59–68 frames reloading at U2E, the port none; A55):
    ///
    /// - **the position ring, its pointers and the fed words** (`+0x2150`, `+0x214c`,
    ///   `[0x905]`…`[0x90e]`). Only `_zinit` primes the ring (`+0x2a8f`, called once at
    ///   `+0x2390`), and the poll pushes and pops it whether a song is running or not
    ///   (`+0x52c9`; the `[0x938]` test at `+0x5228` gates only the mixing) — but every tick of
    ///   a *stopped* song (`+0x515f` → `+0x4404`) writes `0xffff` into the fed order word
    ///   (`+0x4408`). So once the song stops the ring goes on publishing the old song's last
    ///   positions for as many polls as remain of its ten, then **order −1** with the old row
    ///   (the trace tool's own refusal at a reload, `_np_ord reads -1`, `music-clock.md` §7),
    ///   until the new song's first row advance has come through. How far a reload gets is the
    ///   session's (D42's race: pass 2's ALKU reads the stale position in one session of seven).
    ///   Carrying the old ring would put the port in that minority session at every restart:
    ///   pass 2's ALKU would read the old song's `43:34`, `dis_sync` 10, and pass its four gates
    ///   at once (`bins/secondreality/tests/loop.rs`'s
    ///   `a_carried_ring_would_hand_pass_two_the_old_songs_position`, made exact by this file's
    ///   `a_carried_ring_publishes_what_the_stopped_previous_player_would`; A55). And the −1
    ///   itself does not fit the published `u8` order (U2E's `order > 18` gate would read
    ///   255). The fresh ring's `0:0` gives GLENZ's wait (`musplus` −32: the new song's order 0
    ///   has `zplus` 0) and U2E's gate (closed) the decision the −1 gives them; for pass 2's
    ///   ALKU the −1's `dis_sync` key is above the table (A15), and `0:0` is what the loop
    ///   test holds the original's frames to.
    /// - **the tick countdown** `[0xbe0]`, which the reload's polls keep running
    ///   (`+0x5157`). The restarted clock's phase is D34's measured four-frame wait from the
    ///   boundary (`MusicStart::restart`), not the previous song's.
    pub fn restart_from(&mut self, previous: &Player, order: u8) {
        self.channels = previous.channels;
        self.loops = previous.loops;
        self.zframe = previous.zframe;
        self.play_song(order);
    }

    /// `_zstopsong()` (`MAIN/STMIK.H:49`): stop playing, keeping the mixing engine on.
    ///
    /// The original (`+0x25c5`) writes `[0x939] = 0` and nothing else. The mixing loop then
    /// takes the not-playing tick (`+0x513a` → `+0x515f` → `+0x4404`), which still counts the
    /// tick and dispatches the row's effects but never advances a row, feeds order `0xffff`
    /// (`+0x4408`) and never touches `_np_zframe` — which is why the tick counter is still 0 at
    /// the music's start. The port's stop clears `running` as well and so does not tick at all;
    /// nothing in the port calls this but its own tests (the shell's restarts are
    /// [`Player::restart_from`], D45).
    pub fn stop(&mut self) {
        self.running = false;
        self.state = STATE_STOPPED;
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

    /// The `index`th channel's state — the fields the notes and the effects own, in the
    /// object's own terms (`crate::effects::Channel`). This is what a reader needs to see
    /// what a row did: Task 4's tests assert it tick by tick, and Task 5's mixer reads it.
    ///
    /// The index is the S3M channel number: the object's channel-pointer table at `+0x1fa0`
    /// maps each of the 32 numbers to its own structure and is the identity, so cell channel
    /// 5 *is* `_zchn[5]`.
    pub fn channel(&self, index: usize) -> Option<&Channel> {
        self.channels.get(index)
    }

    /// One video frame has passed: mix this frame's samples and advance the tracker by
    /// however many whole ticks that covers.
    ///
    /// The reference is run **polled**: `_zpollmix` is 0 at the module's load and **1 from
    /// the music's start on** (`MAIN/U2.ASM:783` writes it and `MAIN/COPPER.ASM:466` calls
    /// `zpollme` once per copper frame), so one `poll` per logic tick is the faithful model
    /// rather than a simplification of one (`music-clock.md` §5).
    ///
    /// Inside the original's `zpollme` the work is ordered: mix a chunk of the DMA buffer
    /// (`+0x510c`'s `call 0x3905`, reached from the loop at `+0x50d0`), run the tick that the
    /// chunk ended on (`+0x5150`'s `call 0x4353`), mix the next chunk, and so on; then **push**
    /// the position and **pop** the one ten entries old (`+0x52c2`…`+0x5326`). That order is
    /// kept here: the samples before a tick are mixed before that tick runs.
    pub fn poll(&mut self) {
        let before = self.acc / FRAME_UNITS;
        self.acc += u64::from(self.options.mix_rate.samples_per_second()) * 1000;
        self.samples_this_poll = (self.acc / FRAME_UNITS - before) as u32;

        let threshold = u64::from(self.samples_per_tick) * FRAME_UNITS;
        let mut pending = self.samples_this_poll as usize;
        while self.acc >= threshold {
            self.acc -= threshold;
            // The bytes this poll contributes up to the tick: the object mixes a chunk bounded
            // by the DMA buffer's room and by `m_safe`, never by more than a tick's worth at a
            // time in the steady state. The port's chunks are tick-sized, which is what keeps
            // a row's effects on the samples after them.
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

    /// Mix `samples` output samples — the object's DMA bytes, one i16 each — into the drain's
    /// ring. The samples a poll produces are [`Player::samples_this_poll`]'s count exactly.
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

    /// The whole samples this poll contributed — what Task 5's mixer mixes. These are the
    /// **authentic** stream's whatever the [`Quality`]: the clock's count, which the enhanced
    /// renderer turns into 12/5 as many output frames. Fractional
    /// samples are carried into the next poll rather than rounded away, so the rate does not
    /// drift by construction: at 40000 Hz the mean is exactly `40000/70.086` = 570.727 a
    /// poll, and the first five read `[570, 571, 571, 570, 571]` — the pattern follows the
    /// carried fraction, so it is **not** a fixed four-and-one. [`Player::poll`] hands
    /// exactly this many samples to the mixer, so the drain sees the same count Task 3's
    /// tests pinned.
    pub fn samples_this_poll(&self) -> u32 {
        self.samples_this_poll
    }

    /// **The device's drain, and nothing else.** Copy up to `out.len()` of the mixed samples
    /// the engine has produced and the device has not taken, oldest first, and return how many
    /// were copied.
    ///
    /// The samples are interleaved as the output mode says: one a frame in mono, a left and a
    /// right in stereo, so `MixRate::Hz40000` in stereo is 20000 frames a second of two i16
    /// each — or, under [`Quality::Enhanced`], 48,000 ([`PlayerOptions::output_rate`]). This
    /// moves no engine state — the position the parts read is [`Player::state`]'s, and the
    /// device is behind it by whatever this has not drained (`MAIN/STMIK.H:100-103`).
    ///
    /// A device that does not keep up loses samples, not time: the ring is bounded and drops
    /// the *oldest* ones when it is full ([`Mixer::dropped`] counts them), because a poll is
    /// one 70.086 Hz frame of the demo's timeline and may never wait.
    pub fn take_samples(&mut self, out: &mut [i16]) -> usize {
        self.mixer.take(out)
    }

    /// How many samples the drain's ring holds before it drops the oldest — the margin a
    /// device's callback is measured against: 8,192 authentic, 19,660 enhanced.
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

    /// The `zplus` the engine computed for the order it is playing now. The object's own word
    /// is `[0x90d]`, written by the order advance's two `orw`s (`+0x4452`, `+0x4461`), and the
    /// port's tests read it here to pin the arithmetic rather than the delay.
    ///
    /// Where it is the value the *ring* will publish [`FIFO_DEPTH`] polls from now, the ring
    /// carries the order this is asserted against: after the order advance of a natural row
    /// advance, a `B`, a `C` or a `_zgotosong` seek. It is **not** after a `_zplaysong` seek,
    /// where nothing new enters the ring until the next row advance and this word describes an
    /// order the parts will not be told about.
    pub fn engine_zplus(&self) -> i32 {
        i32::from(self.zplus)
    }

    /// `_np_zframe`: the tick counter the parts set and wait on
    /// (`DIS/DISC.ASM:138-159`, forwarded by `DIS/DISINT.ASM:412-425`). It advances one per
    /// **tick**, not per frame — measured at 47.93, 48.04 and 52.01 a second for the traces'
    /// two tempos, i.e. exactly `2.5/tempo` (`music-clock.md` §8).
    pub fn zframe(&self) -> i32 {
        self.zframe
    }

    /// The options this player was built with — the mixing rate and channel count the samples
    /// it produces actually have. A reader that describes the stream (the WAV dump's header,
    /// say) must ask the player rather than rebuild a `Default`, or the header can describe a
    /// stream the mixer is not producing.
    pub fn options(&self) -> PlayerOptions {
        self.options
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
    /// The tick increments `_np_zframe` (`+0x4357`), reads the row's cells and processes the
    /// row's commands on the row's *first* tick (`+0x4371`: `cmpw $0x0,[0x2000]` — zero means
    /// first), dispatches the row's *effects* on every later one (`+0x4394`), counts the tick,
    /// and advances the row when the counter reaches the speed (`+0x4397`…`0x43b0`).
    ///
    /// The two dispatches are different tables in the object — `+0x3c44` reads the table at
    /// `+0x3bd8` and `+0x3d38` the one at `+0x3c0e` — and the difference matters: `G` and `H`
    /// have no first-tick handler at all, `K` and `L` none either.
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

    /// The row's first tick, in three passes, which is the object's order (`+0x4378`…`0x4385`
    /// then `+0x3c44`): wipe every channel's cell (`+0x46a4`), read the row's cells and start
    /// their notes (`+0x46c2`, whose install path is `+0x4733` and whose note work is
    /// `+0x4810`), and only then run the row's commands through the first-tick table.
    fn process_row(&mut self) {
        for channel in &mut self.channels {
            channel.clear_cell();
        }
        // **A pattern index past `patNum`, or a row past the pattern's, is treated as an empty
        // row.** The object would read the paragraph table's neighbouring entry — a read past
        // an array, which a Rust port cannot reproduce — and the loader cannot validate the
        // case (the order list is read before the patterns), so `None` is the safe reading. No
        // shipped module reaches it: both order lists name patterns inside their `patNum`
        // (77 in each). It is documented rather than changed because silence is the behaviour
        // the object cannot be asked about here, not a value the port chose over a known one.
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
                // `+0x4783`: the note and instrument work is for the mixer's own channels —
                // anything above channel 15 takes the object's adlib path, which this build
                // has no card for (`_zoutputladlib` is 0, `+0x47a6`).
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

    /// Every tick after the row's first: `+0x3d38`, the channels walked in order and each
    /// one's command dispatched through the second table.
    fn tick_effects(&mut self) {
        for index in 0..CHANNELS {
            if self.channels[index].dispatches_on_tick() {
                self.dispatch(index, false);
            }
        }
    }

    /// One channel's command, through the table that belongs to this tick: `first_tick` picks
    /// between the object's `+0x3bd8` and its `+0x3c0e`.
    ///
    /// **Both tables matter to both kinds of command.** The timeline's `A` (`+0x3dd2`), `B`
    /// (`+0x3de2`) and `C` (`+0x3dfe`) are the first-tick table's entries 1, 2 and 3 — the
    /// later-tick table's are the bare `ret` — and among the channel effects only `D`, `E`, `F`,
    /// `J` and `Q` are in both tables: `G`, `H` and `K` are the later ticks' alone, and `K`
    /// has a second half the first tick never runs.
    fn dispatch(&mut self, index: usize, first_tick: bool) {
        let channel = &mut self.channels[index];
        let command = channel.command;
        let info = channel.info;
        let tick = self.tick_in_row;
        let c2spd = channel.c2spd;
        let shift = self.vibrato_shift;
        match (command, first_tick) {
            // `A`, `B` and `C` are in the *first-tick* table only: the later-tick table's
            // entries 1, 2 and 3 are the bare `ret` at `+0x3d72`.
            (1, true) => self.set_speed(info),
            (2, true) => {
                // `B` (`+0x3de2`): the flag goes up now, so the jump lands at the row's end
                // and the row still plays out.
                self.advance_flag = ADVANCE_ORDER;
                if info == 0xff {
                    // `BFF` (`+0x3df1`): the song ends at the next row advance, unless the
                    // player is already stopped.
                    if self.state != STATE_STOPPED {
                        self.advance_flag = ADVANCE_STOP;
                    }
                } else {
                    self.next_order = u16::from(info);
                }
            }
            (3, true) => {
                // `Cxy` (`+0x3dfe`): the row is **decimal** and is stored unclamped.
                self.pending_row = (info >> 4) * 10 + (info & 0x0f);
                self.advance_flag = ADVANCE_ORDER;
            }
            // `T` (`+0x3d73`) — the first-tick table's entry 20, the same handler the header
            // watch calls. Neither module carries one (`the_timeline_effects_the_modules_carry`
            // counts 0 in both), so this branch is the object's table entry and not a
            // behaviour the data reaches; the later ticks' entry is the bare ret.
            (20, true) => self.set_tempo(info),
            // `D` (`+0x3f08`) — the volume slide, on both ticks.
            (4, _) => self.channels[index].volume_slide(info, tick),
            // `E` (`+0x3e4f`) and `F` (`+0x3e10`) — the portamentos, on both ticks.
            (5, _) => self.channels[index].portamento(info, tick, Portamento::Down),
            (6, _) => self.channels[index].portamento(info, tick, Portamento::Up),
            // `G` (`+0x3f94`) and `H` (`+0x4005`): second table only.
            (7, false) => self.channels[index].tone_portamento(info),
            (8, false) => self.channels[index].vibrato(info, shift),
            // `J` (`+0x4190`) — the arpeggio, on both ticks.
            (10, _) => self.channels[index].arpeggio(info, tick, c2spd),
            // `K` (`+0x3f00`): second table only, and its second half is a *vibrato* whose
            // parameter is `alasteff1` — the byte `H` remembers, not the one `D` does.
            (11, false) => {
                self.channels[index].volume_slide(info, tick);
                self.channels[index].vibrato_last(shift);
            }
            // `Q` (`+0x41ec`) — the retrigger, on both ticks.
            (17, _) => self.channels[index].retrigger(info),
            // `S` (`+0x424e` on the first tick, `+0x4261` later), subcommand `d` only. Both
            // modules' cells are `SDx` and nothing else — the subcommand set is asserted in
            // `tests/effects.rs` — so every other subcommand is **silently ignored** rather
            // than guessed at: it falls through these guards to the `_ => {}` below, which is
            // a no-op and not a refusal. (The object has handlers for subcommands 3, 4, 12
            // and 14 — `tests/player.rs`'s claims table reads their first-tick entries at
            // `0x3b98`/`0x3bb8` — but no cell in either module carries one, so the crate's
            // "absent" lists cover them; this comment says what the code does.)
            (19, true) if info >> 4 == 0x0d => self.channels[index].note_delay(info),
            (19, false) if info >> 4 == 0x0d => self.note_delay_tick(index),
            // `O` (the sample offset) has no handler in either table: it is applied where the
            // row's cells are read (`+0x48d6`, in `start_note` below).
            _ => {}
        }
    }

    /// `SDx`'s later ticks (`+0x429a`): the countdown runs on every channel — it is the
    /// channel's `atrigcnt`, and the object's handler is unconditional — and the tick that
    /// empties it calls the install continuation at `+0x475e`, where the *note* begins: the
    /// `0x81` channel flag, the clamps the row parse skipped and the note work itself. That
    /// last part is the mixer channels' only (`+0x4783`), which is where the channel test
    /// belongs: after the countdown, not before it.
    fn note_delay_tick(&mut self, index: usize) {
        if !self.channels[index].note_delay_tick() {
            return;
        }
        if index <= 0x0f {
            self.channels[index].clamp_cell();
            self.start_note(index);
        }
    }

    /// The note and instrument work a row's cell does on a mixer channel: the object's
    /// `+0x4810`, which loads an instrument a cell names (`+0x4815`…`0x48cb`) and then starts a
    /// note (`+0x48e5`…`0x4975`).
    ///
    /// Four readings are worth naming here, because none of them is the obvious one:
    ///
    /// - An instrument of its own **clears the sample offset** (`+0x481d`) before the note it
    ///   came with can set it, so an `O` on a cell with an instrument starts that note from the
    ///   offset and an `O` without one leaves the last offset for the next note.
    /// - A note played while a `G` is on the row is *not* started: the period stays where it
    ///   is and only the target moves (`+0x4944`), which is the whole of the tone porta — the
    ///   glide is what closes the gap, a tick at a time.
    /// - The volume column, when the cell has one, replaces both the volume and `aorgvol`
    ///   (`+0x495a`) — including a volume of 0, which is why `Cell::present` exists.
    /// - `0xfe` is the note off: the position parks at 1, the volume goes to 0 and **the
    ///   target and base periods become `0xffff`** (`+0x4906`…`0x4957`), the value the mixer
    ///   reads as "no sample"; the live period is left where it was.
    fn start_note(&mut self, index: usize) {
        {
            let channel = &mut self.channels[index];
            if channel.instrument != 0 {
                channel.sample_offset = 0;
                channel.last_instrument = channel.instrument;
                let instrument = usize::from(channel.instrument) - 1;
                let loaded = self
                    .module
                    .instrument(instrument)
                    .filter(|header| header.is_pcm())
                    .map(|header| {
                        // `checkins`'s clamp (`STARTEMS.C:139`) is what the object reads here:
                        // the header's 32-bit `c2spd` goes into a word (`+0x4850`).
                        header.c2spd.min(0xffff) as u16
                    });
                match loaded {
                    Some(c2spd) => {
                        channel.c2spd = c2spd;
                        let header_volume = self
                            .module
                            .instrument(instrument)
                            .map_or(0, |header| header.volume);
                        // `+0x485b`…`0x4867`: the header's volume byte, clamped to 0…63.
                        channel.volume = header_volume.min(0x3f);
                    }
                    None => channel.last_instrument = 0,
                }
                // `+0x4872`…`0x48cb`: the note start hands the mixer its voice — which sample
                // it plays and where the sample ends. `checkins` has already decided the
                // instrument is a PCM one; the mixer reads its header's loop for itself.
                self.mixer
                    .start_note(index, &self.module, channel.instrument);
            }
            // `+0x48ce`: nothing to play a note on until an instrument has loaded.
            if channel.last_instrument == 0 {
                return;
            }
            if channel.command == 0x0f {
                channel.set_sample_offset();
            }
            if channel.note == 0xff {
                // `+0x495a`: the volume column alone. (Nothing else about the cell is a note.)
            } else if channel.note == 0xfe {
                channel.position = 1;
                channel.position_low = 0;
                channel.volume = 0;
                channel.org_period = 0xffff;
                channel.target_period = 0xffff;
            } else {
                // `+0x490e`: a note on a channel with no period yet starts a sample, and a
                // note with no `G` under it restarts the position — both take the offset the
                // `O` command left (`+0x491e`). Only a `G` note on a playing channel leaves
                // the position alone, which is what makes the glide a glide.
                let fresh = channel.org_period == 0;
                if fresh || channel.command != 0x07 {
                    channel.position = channel.sample_offset;
                    channel.position_low = 0;
                }
                channel.last_note = channel.note;
                let period = scaled_period(note_period(channel.note), channel.c2spd);
                if fresh || channel.command != 0x07 {
                    channel.period = period;
                    channel.vibrato_position = 0;
                }
                channel.org_period = period;
                channel.target_period = period;
            }
            if channel.cell_volume != 0xff {
                channel.volume = channel.cell_volume;
            }
        }
    }

    /// `A` (`+0x3dd2`, the first-tick table's entry 1): the speed takes the parameter, and
    /// there is no upper bound on it — the object's handler is `cmp $0x0,%al; je; xor %ah,%ah;
    /// mov %ax,0x2002`, so an `A` whose parameter is above 32 sets the *speed* just the same.
    ///
    /// **This is a correction to what this file did before Task 4 read the tables**: the
    /// command, not the parameter, is what picks the handler, and one of MUSIC1's 54 `A` cells
    /// carries `0x7f` as its parameter (its `A` infos are `{3: 30, 6: 23, 127: 1}`) — under the
    /// old reading that cell set a tempo of 127 and left the speed alone, where the object
    /// plays the row at speed 127. No filed trace window reaches that cell (MUSIC1's trace,
    /// `second3`, covers orders 0 and 42…48), which is why the difference had not shown.
    fn set_speed(&mut self, value: u8) {
        if value != 0 {
            self.pub_speed = value;
        }
    }

    /// `T` (`+0x3d73`, the first-tick table's entry 20): 32 and under is a *no-op* — the
    /// parameter test is inside this handler and sends it to the driver call at `+0x3dce`,
    /// never to the speed — and above it the tempo is set and the tick's sample count
    /// recomputed (`+0x3d99`…`0x3da6`), which is the only place that arithmetic runs. The
    /// accumulator's carried remainder is what lets a tempo change land mid-row without a jump.
    ///
    /// The header watch reaches this handler with the module's own initial-tempo byte
    /// (`+0x53c7`), which is how the shell's patch at `STARTMUS.C:257` gets in.
    fn set_tempo(&mut self, value: u8) {
        if value > 0x20 {
            self.pub_tempo = value;
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
            // The nine-bit mask is the object's own: `_zplaysong`'s order counter is nine bits
            // wide, and `next_order` carries that mask. The scan index `at` itself is **not**
            // masked, and `self.order` stores it whole (`at as u16`), so an order list longer
            // than 512 entries would make the published `entry_now()` (masked) and
            // `engine_position()` (not) disagree. No shipped module can reach it — 80 and 100
            // entries — and only a malformed one could; the asymmetry is the object's mask
            // over the port's plain index, kept and documented rather than changed.
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
    ///
    /// The order is `[0x940] - 1` (`+0x43e6`..`+0x43ef`), which is the order being played on
    /// every path but one: the scan's give-up branch returns *without* entering an order, and
    /// there this is the last index the scan looked at — what the original would feed too.
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
/// truncated (`+0x3d99`…`+0x3da6`, whose `mul`/`div` pair is `_zmixspeed × 0x7d` over
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

    /// **A55's counterfactual is exact**: a restart that also carried the position ring, its
    /// pointers and the fed words would publish, poll for poll, what the previous player
    /// publishes when it is stopped and polled on — until the new song's first row advance has
    /// come through the ring. A stopped player's poll pushes its frozen fed words and pops the
    /// ring exactly as the carried ring would. `bins/secondreality/tests/loop.rs`'s
    /// `a_carried_ring_would_hand_pass_two_the_old_songs_position` uses this to read what pass
    /// 2's ALKU would see, without the port carrying anything it does not.
    ///
    /// The scenario is that test's: block 5's MUSIC0 from order 25, 8,491 polls (frames 408 to
    /// the loop's restart tick 8898), then the loop's MUSIC0 restart at order 0.
    #[test]
    fn a_carried_ring_publishes_what_the_stopped_previous_player_would() {
        let module = || {
            let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/MAIN/MUSIC0.S3M");
            let mut module = Module::load(&std::fs::read(path).unwrap()).unwrap();
            module.apply_tempo_patch(0x78);
            module
        };
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
