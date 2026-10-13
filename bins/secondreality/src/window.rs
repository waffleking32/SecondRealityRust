//! The windowed presenter: SDL2 video, and the demo's frames on the screen.
//!
//! SDL2 lives in the binary and only in the binary; the engine itself has no
//! runtime dependency at all. The window is an engine client like any other:
//! it feeds wall-clock time to `Sequence::advance` and blits the newest frame,
//! dropping the ticks the display fell behind on.
//!
//! It drives a **run**, not one part: `--block 5` plays the block's ported
//! parts in the demo's own order and `--part` plays one, on the same
//! [`Sequence`] `--headless` dumps. Where the run has got to is on the window's
//! title bar — the part whose picture is up, that frame's index in the
//! reference's own numbering, and the block the run came from — because a
//! moving picture with no index on it is not evidence of anything.
//!
//! The keys are the original's, and two of them do something
//! (`DIS/DISINT.ASM:112-156`): ESC sets the exit flag the running part's own
//! loop reads (`:151-153`, handed to it by `exit_2` `:186-190`), and Ctrl skips
//! the running part (`MAIN/U2.ASM:205-207`). Nothing here waits for a key: a
//! part runs to its own end, and a run of parts to the end of its plan — where
//! the screen holds what the last part left (`MAIN/U2.ASM:935`'s
//! `@@partsdone`), ESC closes the window, and the title bar says why nothing
//! is moving.

use std::error::Error;
use std::time::{Duration, Instant};

use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::PixelFormatEnum;
use sdl2::render::{Texture, TextureCreator, WindowCanvas};
use sdl2::video::WindowContext;
use sr_engine::present::{output_size, present, FrameBuffer, PresentOptions};
use sr_engine::{IndexedFrame, Tick};

use crate::cli::Cli;
use crate::shell::Sequence;

/// The window's title, and the first word of the status line.
const TITLE: &str = "Second Reality";

/// The texture format the presented frames are uploaded in.
///
/// `RGBA32`, not `RGBA8888` — the two are reverses of each other, and the name
/// that reads "RGBA" is the wrong one. SDL names packed 32-bit formats by *bit
/// significance*, most significant component first; on a little-endian host
/// that puts the first-named component in the *last* byte. `present` writes a
/// byte array, red first, so the format it needs is the one SDL defines as
/// "RGBA byte arrays of color data" (`SDL_pixels.h:273`), which SDL calls
/// `SDL_PIXELFORMAT_RGBA32` and resolves to `ABGR8888` on little-endian and
/// `RGBA8888` on big-endian (`SDL_pixels.h:274-284`). The crate's `RGBA32` is
/// that same alias, so this line is correct on either endianness.
///
/// Declaring `RGBA8888` here read the red byte as alpha — every pixel at full
/// red, green and blue swapped — and nothing failed: the engine was right, the
/// presenter was right, and only the screen was wrong.
const TEXTURE_FORMAT: PixelFormatEnum = PixelFormatEnum::RGBA32;

/// One logic tick's period, from the engine's own rate: the window is paced to
/// the tick, never to the display's refresh, so a 60 Hz panel shows some
/// frames twice rather than re-timing the demo.
const TICK_PERIOD: Duration = Duration::from_nanos(1_000_000_000_000 / Tick::HZ_MILLI);

/// Run what the command line named in a window, until the window is closed or
/// the run is over and ESC says so.
pub fn run(cli: &Cli) -> Result<(), Box<dyn Error>> {
    let opts = PresentOptions {
        aspect: cli.aspect,
        scale: cli.scale,
        border: cli.border,
    };
    // The same run `--headless` dumps: a block's ported parts in the demo's own
    // order, or the one part `--part` named, numbered in the reference's frame
    // space.
    let mut run = crate::new_sequence(cli)?;
    let block = cli.block;

    let sdl = sdl2::init()?;
    let video = sdl.video()?;
    let timer = sdl.timer()?;

    let mut raster = output_size(run.frame(), &opts);
    let mut status = status_line(&run, block);
    let window = video
        .window(&status, raster.0 as u32, raster.1 as u32)
        .position_centered()
        .build()?;
    let mut canvas = window.into_canvas().build()?;
    let creator = canvas.texture_creator();
    let mut texture = streaming_texture(&creator, raster)?;
    let mut buffer = FrameBuffer::new(0, 0);
    let mut events = sdl.event_pump()?;

    let mut sampled = Instant::now();
    let mut escape = EscapeKey::default();
    loop {
        let keys = keys_of(events.poll_iter());
        if keys.close {
            break;
        }
        if keys.exit {
            escape.press();
        }
        if escape.apply(&mut run) {
            // The exit flag has nothing left to act on: the run's parts are
            // over and the screen the last one left is being held. The
            // original's shell is at `@@partsdone` there (`MAIN/U2.ASM:935`) and
            // returns to DOS, which is this window closing.
            break;
        }
        if keys.skip {
            run.skip_part();
        }

        // The sequence has no clock of its own: it is handed the wall-clock
        // time since it was last asked, so a display that fell behind gets the
        // newest frame only, never a burst of stale ones. The timestamp has to
        // outlive the iteration — anchored inside the loop it would measure the
        // event drain alone, and the demo would advance a tick a minute.
        let now = Instant::now();
        run.advance(now - sampled);
        sampled = now;

        // `Engine::tick` sizes the frame from the view every tick, so a part
        // that changes the picture's geometry — double-scan takes it to 400
        // lines — changes the raster the texture and the window have to fit.
        let size = output_size(run.frame(), &opts);
        if size != raster {
            raster = size;
            texture = streaming_texture(&creator, raster)?;
            canvas.window_mut().set_size(size.0 as u32, size.1 as u32)?;
        }
        blit(&mut canvas, &mut texture, &mut buffer, run.frame(), &opts)?;

        // The status line, when it has changed. The index in it moves every
        // tick, so this is one window-manager call a tick and not a redraw:
        // only the text is compared, and the frame on the screen is untouched.
        let line = status_line(&run, block);
        if line != status {
            canvas.window_mut().set_title(&line)?;
            status = line;
        }

        // A run that has *ended* — a part the port has but could not start —
        // has no hold after it and nothing left to show.
        if run.finished() {
            break;
        }
        // SDL's frame delay does the waiting — out the rest of the tick, and
        // never less than a millisecond, so the loop cannot spin.
        let left = TICK_PERIOD.saturating_sub(now.elapsed());
        timer.delay(left.as_millis().max(1) as u32);
    }
    Ok(())
}

/// What the keys seen this drain ask of the run.
///
/// The decision is a function of the events and of nothing else — no SDL
/// object, no clock, no run — so the key behaviour is testable without a
/// display. That matters here more than anywhere else in the binary: what the
/// screen *looks* like can only be checked by a human, and this is the part of
/// the window no human look would catch.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Keys {
    /// ESC: the exit flag (`DIS/DISINT.ASM:151-153`) — ask the running part to
    /// quit, so that the run goes on to the next part. In the original the
    /// part's own loop returns (`exit_2` `:186-190`); in the port the shell
    /// retires it at the end of that tick, because no part polls the flag yet
    /// (D21).
    exit: bool,
    /// Ctrl: skip the running part (`MAIN/U2.ASM:205-207` — `partexecute`
    /// returns before `execute` when the key is down).
    skip: bool,
    /// The window was closed.
    close: bool,
}

/// The keys in `events`, as what they ask of the run.
///
/// Those two are all of them. `checkkeys` (`DIS/DISINT.ASM:112-156`) also reads
/// `'0'` and `'9'` (mouse speed) and `'1'` (the loader's file), none of which
/// the port has, and everything else is `passmuscode` — the music player's
/// business, which Phase 2 does not have at all.
///
/// A key *up* asks nothing: the original reads a level (the BIOS flag byte, or
/// a keystroke waiting in the buffer), but the port's loop drains SDL's events
/// once a tick and a part runs to its own end either way.
fn keys_of(events: impl IntoIterator<Item = Event>) -> Keys {
    let mut keys = Keys::default();
    for event in events {
        match event {
            Event::Quit { .. } => keys.close = true,
            Event::KeyDown { keycode: Some(Keycode::Escape), .. } => keys.exit = true,
            Event::KeyDown { keycode: Some(Keycode::LCtrl | Keycode::RCtrl), .. } => {
                keys.skip = true
            }
            _ => {}
        }
    }
    keys
}

/// ESC, and the press the run may not be able to act on yet.
///
/// **A press between two parts must not be lost.** One iteration after a part
/// retires there is no running part to ask and the run is not holding yet, so a
/// press can land where nothing can take it — and that is exactly where the
/// second press of "get me out" lands when the one before it ended the last
/// part of a block. The original does not lose it either, and for a different
/// reason: the key waits in the BIOS buffer and the *next* part's `checkkeys`
/// consumes it (`DIS/DISINT.ASM:119-121`, `mov ah,1 / int 16h` to peek at what
/// is waiting, `:122-123` to take it).
#[derive(Debug, Default, Clone, Copy)]
struct EscapeKey {
    /// A press that has not been applied to a part yet.
    waiting: bool,
}

impl EscapeKey {
    /// Note a press. Idempotent: SDL reports auto-repeat as more presses, and
    /// the original's buffered keystroke is one flag either way.
    fn press(&mut self) {
        self.waiting = true;
    }

    /// Apply a waiting press to `run`, and say whether the window should close.
    ///
    /// The press is asked of the running part if there is one; otherwise it
    /// keeps waiting, because the run may be about to start the next part. It
    /// is spent when it reaches a part and kept when it does not — so it is
    /// never simply dropped, which is what a keystroke is: something the user
    /// did, and did deliberately.
    ///
    /// With nothing left to play — the run is holding the screen its parts left
    /// (`@@partsdone`, `MAIN/U2.ASM:935`) — the press is the way out of the
    /// window. The other state with nothing to play, a run that has *ended*, is
    /// not reached here: the loop breaks in the iteration the run ends in,
    /// before the next press can arrive.
    fn apply(&mut self, run: &mut Sequence) -> bool {
        if !self.waiting {
            return false;
        }
        if run.request_exit() {
            self.waiting = false;
            return false;
        }
        run.holding()
    }
}

/// The window's status line: the part whose picture is up, where the run is in
/// the reference's own numbering, and the entry point it came from.
///
/// This is what makes a window run evidence rather than a moving picture. The
/// index is a boot-relative emulator frame — the space every file in
/// `reference/` is measured in — so a run can be read against the reference
/// while it plays, and a run that began anywhere else says so rather than
/// showing a number that looks comparable and is not.
///
/// The part named is the one that drew the frame on the screen. After a part
/// retires the shell holds its screen, so the line keeps naming it while the
/// hold lasts: that is whose picture is up, and naming the *next* part would be
/// the line claiming a part the run is not playing. A run that has stopped says
/// why — a window that simply stopped moving would not.
fn status_line(run: &Sequence, block: Option<char>) -> String {
    let mut line = TITLE.to_string();
    if let Some(block) = block {
        line.push_str(" — block ");
        line.push(block);
    }
    line.push_str(" — ");
    line.push_str(run.part());
    match run.frame_index() {
        Some(index) => line.push_str(&format!(" — frame {index}")),
        None => line.push_str(" — starting"),
    }
    if !run.index_is_reference() {
        line.push_str(" (its own frame numbering, not the reference's)");
    }
    if run.holding() {
        match run.stopped_at() {
            Some(part) => {
                line.push_str(" — holding; ");
                line.push_str(part);
                line.push_str(" is not ported yet");
            }
            None => line.push_str(" — holding; the run's parts are over"),
        }
    }
    line
}

/// Present `frame` into `buffer` and upload it.
fn blit(
    canvas: &mut WindowCanvas,
    texture: &mut Texture,
    buffer: &mut FrameBuffer,
    frame: &IndexedFrame,
    opts: &PresentOptions,
) -> Result<(), Box<dyn Error>> {
    present(frame, opts, buffer);
    // A streaming texture is write-only: updating it is what gets the frame to
    // the renderer, and `canvas` then paints the whole window with it.
    texture.update(None, buffer.rgba(), buffer.width() * 4)?;
    canvas.clear();
    canvas.copy(texture, None, None)?;
    canvas.present();
    Ok(())
}

/// A streaming RGBA texture of `raster`, to upload presented frames into.
fn streaming_texture(
    creator: &TextureCreator<WindowContext>,
    raster: (usize, usize),
) -> Result<Texture<'_>, Box<dyn Error>> {
    Ok(creator.create_texture_streaming(
        TEXTURE_FORMAT,
        raster.0 as u32,
        raster.1 as u32,
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell::{start_view, Sequence, SequenceStatus, SURFACE_HEIGHT, SURFACE_WIDTH};
    use sr_engine::part::{Part, PartCtx, PartError, PartStatus};

    /// A part that draws nothing. `retires` is the first `ctx.frame` it reports
    /// `Finished` from, or `None` to run for the whole run.
    ///
    /// What is under test here is the run's numbering, its keys and its status
    /// line, so the picture is deliberately nothing: the parts themselves are
    /// Tasks 4-9's, and they are tested against their own frames.
    struct Stub {
        retires: Option<u32>,
    }

    impl Part for Stub {
        fn name(&self) -> &'static str {
            "stub"
        }
        fn init(&mut self, _ctx: &mut PartCtx) -> Result<(), PartError> {
            Ok(())
        }
        fn frame(&mut self, ctx: &mut PartCtx) -> PartStatus {
            match self.retires {
                Some(from) if ctx.frame >= from => PartStatus::Finished,
                _ => PartStatus::Continuing,
            }
        }
    }

    /// The parts these tests run, by what each name stands for. The names are
    /// the demo's where the name is what matters — `shell::boot_anchor` keys on
    /// `beglogo` and `endlogo` — and pure stand-ins otherwise.
    fn stub_source(name: &str) -> Option<Box<dyn Part>> {
        match name {
            // An anchored block entry (`MAIN/U2.ASM:806-808`) that keeps
            // drawing, so the key tests have a part to press a key in.
            "beglogo" => Some(Box::new(Stub { retires: None })),
            // The other anchored block entry, retiring at once, so a run of it
            // reaches the hold.
            "endlogo" => Some(Box::new(Stub { retires: Some(0) })),
            // A part the port has not written.
            "glenz" => None,
            _ => Some(Box::new(Stub { retires: None })),
        }
    }

    fn run_of(plan: Vec<&'static str>) -> Sequence {
        Sequence::with_source(SURFACE_WIDTH, SURFACE_HEIGHT, start_view(), plan, stub_source)
            .unwrap()
    }

    fn run_out(run: &mut Sequence, limit: u32) {
        for _ in 0..limit {
            if run.tick() == SequenceStatus::Finished {
                break;
            }
        }
    }

    /// A key press, as SDL delivers one. Built rather than polled: the decision
    /// is what is under test, and an `EventPump` needs a display to exist.
    fn key_down(keycode: Keycode) -> Event {
        Event::KeyDown {
            timestamp: 0,
            window_id: 0,
            keycode: Some(keycode),
            scancode: None,
            keymod: sdl2::keyboard::Mod::NOMOD,
            repeat: false,
        }
    }

    fn key_up(keycode: Keycode) -> Event {
        Event::KeyUp {
            timestamp: 0,
            window_id: 0,
            keycode: Some(keycode),
            scancode: None,
            keymod: sdl2::keyboard::Mod::NOMOD,
            repeat: false,
        }
    }

    /// The declared format must decode the bytes `present` wrote, in the order
    /// it wrote them. Getting this wrong does not fail loudly: the picture comes
    /// up in wrong colours and every test above still passes, because none of
    /// them touches a texture.
    #[test]
    fn the_texture_format_reads_the_presenters_bytes_in_order() {
        let masks = TEXTURE_FORMAT.into_masks().expect("SDL knows its own formats");
        // One pixel as `Palette::write_rgba` lays it out in memory: red 10,
        // green 20, blue 30, opaque. `from_ne_bytes` is the bytes as they are,
        // which is what the texture is handed.
        let packed = u32::from_ne_bytes([10, 20, 30, 255]);
        let channel = |mask: u32| ((packed & mask) >> mask.trailing_zeros()) as u8;
        assert_eq!(channel(masks.rmask), 10, "red");
        assert_eq!(channel(masks.gmask), 20, "green");
        assert_eq!(channel(masks.bmask), 30, "blue");
        assert_eq!(channel(masks.amask), 255, "alpha");
    }

    #[test]
    fn escape_asks_the_running_part_to_exit_and_ctrl_skips_it() {
        // The two keys `checkkeys` acts on (`DIS/DISINT.ASM:112-156`), as the
        // decision they are. ESC sets the exit flag, Ctrl is what `partexecute`
        // reads before running a part (`MAIN/U2.ASM:205-207`).
        assert_eq!(keys_of([key_down(Keycode::Escape)]), Keys { exit: true, ..Keys::default() });
        assert_eq!(keys_of([key_down(Keycode::LCtrl)]), Keys { skip: true, ..Keys::default() });
        assert_eq!(keys_of([key_down(Keycode::RCtrl)]), Keys { skip: true, ..Keys::default() });
        assert_eq!(
            keys_of([Event::Quit { timestamp: 0 }]),
            Keys { close: true, ..Keys::default() }
        );

        // Everything else is `passmuscode` (`:154`) and asks nothing of the
        // run: not the keys the original's `checkkeys` ignores, and not the
        // ones it uses for other things — `0`, `9` and `1` are the mouse and
        // the loader, and the port has neither.
        for other in [
            Keycode::Space,
            Keycode::Return,
            Keycode::Q,
            Keycode::F10,
            Keycode::Num0,
            Keycode::Num9,
            Keycode::Num1,
        ] {
            assert_eq!(keys_of([key_down(other)]), Keys::default(), "{other:?} asks nothing");
        }
        // A key *up* is not a press, and an auto-repeat is the same press.
        assert_eq!(keys_of([key_up(Keycode::Escape)]), Keys::default());
        assert_eq!(keys_of([key_up(Keycode::LCtrl)]), Keys::default());

        // And what each asks of a *run*: ESC hands the flag to the part it was
        // pressed in and the run goes on without it; Ctrl does not ask, it
        // skips, and the numbering keeps counting across the seam.
        let mut run = run_of(vec!["beglogo", "cred", "endscrl"]);
        run.tick();
        assert!(run.request_exit(), "ESC: there is a running part to ask");
        run.tick();
        run.tick();
        assert_eq!(run.part(), "cred", "ESC ends the part it was pressed in");
        assert!(run.skip_part(), "Ctrl: there is a running part to skip");
        run.tick();
        assert_eq!(run.part(), "endscrl", "Ctrl skips the part it was pressed in");
        assert_eq!(run.index(), 404 + 4, "and the run's numbering never restarts");
    }

    #[test]
    fn an_escape_between_two_parts_is_kept_and_then_closes_the_window() {
        // The seam after the last part of a block: `endlogo` retires on its
        // first tick, so the iteration that follows has no running part and no
        // hold yet — `Sequence::request_exit` has nothing to ask. That press is
        // "get me out", and a window that consumed it would need a third press
        // to close. It waits instead, and acts as soon as there is nothing left
        // to play.
        let mut run = run_of(vec!["endlogo", "glenz"]);
        let mut escape = EscapeKey::default();

        run.tick();
        assert!(!run.holding(), "the part retired this tick; the hold is next");
        escape.press();
        assert!(!escape.apply(&mut run), "nothing to ask, and somewhere to go: the press waits");

        run.tick();
        assert!(run.holding(), "the next part is one the port has not written");
        assert!(escape.apply(&mut run), "and now the press is the way out");
    }

    #[test]
    fn an_escape_between_two_parts_is_asked_of_the_next_one() {
        // The other side of the same seam: the run *does* have a part to play,
        // so the press is spent on it — the original's buffered keystroke, which
        // the next part's `checkkeys` consumes (`DIS/DISINT.ASM:119-123`).
        let mut run = run_of(vec!["endlogo", "cred"]);
        let mut escape = EscapeKey::default();

        run.tick();
        escape.press();
        assert!(!escape.apply(&mut run), "there is no part to ask yet");
        run.tick();
        assert_eq!(run.part(), "cred");
        assert!(!escape.apply(&mut run), "the next part is asked, and the window stays open");
        assert!(!escape.waiting, "the press has been spent");

        run.tick();
        assert_eq!(run.part(), "cred", "and that part is on its way out");
        assert!(!escape.apply(&mut run), "a spent press is not a second one");

        escape.press();
        run.tick();
        assert!(run.holding(), "the plan ran out with cred");
        assert!(escape.apply(&mut run), "and a fresh press is the way out");
    }

    #[test]
    fn the_status_line_names_the_part_the_reference_index_and_the_block() {
        let mut run = run_of(vec!["beglogo"]);
        assert_eq!(
            status_line(&run, Some('5')),
            "Second Reality — block 5 — beglogo — starting"
        );
        run.tick();
        assert_eq!(
            status_line(&run, Some('5')),
            "Second Reality — block 5 — beglogo — frame 404",
            "the anchored run's first frame is the boot's own 404"
        );
        run.tick();
        // The same run with no block: `--part beglogo` is the same numbering
        // and has no entry point to name.
        assert_eq!(status_line(&run, None), "Second Reality — beglogo — frame 405");
    }

    #[test]
    fn a_run_with_no_reference_anchor_says_its_index_is_not_the_references() {
        // `--part tunneli` and the test card number their own frames from zero;
        // a status line that showed that number bare would look like a
        // reference index and be one the reference tree cannot answer for.
        let mut run = run_of(vec!["testcard"]);
        run.tick();
        assert_eq!(
            status_line(&run, None),
            "Second Reality — testcard — frame 0 (its own frame numbering, not the reference's)"
        );
    }

    #[test]
    fn a_run_that_stopped_says_so_and_names_the_part_that_holds_the_screen() {
        // `SECOND 2` runs BEGLOGO and then GLENZ, which the port has not
        // written: the run holds BEGLOGO's screen rather than ending silently,
        // and the status line is where the reason is. The part it names is the
        // one whose picture is up — not the one the run could not reach.
        let mut run = run_of(vec!["endlogo", "glenz"]);
        run_out(&mut run, 4);
        assert_eq!(
            status_line(&run, Some('2')),
            "Second Reality — block 2 — endlogo — frame 407 — holding; glenz is not ported yet"
        );
        assert_eq!(run.stopped_at(), Some("glenz"));

        // And a run whose own plan simply ran out says that instead.
        let mut run = run_of(vec!["endlogo"]);
        run_out(&mut run, 4);
        assert_eq!(
            status_line(&run, Some('5')),
            "Second Reality — block 5 — endlogo — frame 407 — holding; the run's parts are over"
        );
        assert_eq!(run.stopped_at(), None);
    }
}
