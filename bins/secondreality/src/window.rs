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
//! is moving. A **demo run** — a block's, or no argument's — does not hold for
//! ever: its hold is the exit stage (the fade, the blank), the window closes by
//! itself when the stage has run (`Sequence::ended`), and an ESC during the
//! stage closes it at once (D41). `main` then prints the end box.
//! A **looping** run (`--loop`, D42) never gets there: it goes round from CRED
//! to ALKU for ever, the title bar names the pass from the second on, ESC in the
//! seam between two passes acts on nothing, and the close button is the way out
//! (the box is printed then).
//!
//! A windowed **demo run** opens on the settings screen first ([`crate::settings`], D44):
//! five rows drawn in the demo's own font, whose choices become the run's settings. Enter
//! starts the run, ESC leaves without one (`main` prints the end box, as `fatalexit` does),
//! and `--no-setup` or `/gravis` skip it. The loop is the vault's: draw, wait in bounded
//! `wait_event_timeout`s, re-present on an expose, and re-present under the new
//! [`PresentOptions`] when Aspect or Border change.
//!
//! `--vault` adds a menu over the three unreleased effects (`parts::vault`):
//! it draws a 320×200 picture ([`crate::vault_menu`]), starts the chosen one as
//! a one-part run on the same loop, and — because the menu is where the run
//! came from — sends ESC at the held screen back to the menu rather than
//! closing the window. ESC in the menu, or the close button, ends it. The
//! normal run's keys, status line and pacing are untouched.

use std::error::Error;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use sdl2::event::{Event, WindowEvent};
use sdl2::keyboard::Keycode;
use sdl2::pixels::{Color, PixelFormatEnum};
use sdl2::rect::Rect;
use sdl2::render::{BlendMode, Canvas, RenderTarget, ScaleMode, Texture, TextureCreator, WindowCanvas};
use sdl2::video::WindowContext;
use sdl2::{EventPump, VideoSubsystem};
use sr_assets::font::Font;
use sr_assets::load_path;
use sr_engine::present::{output_size, present, FrameBuffer, PresentOptions};
use sr_engine::{IndexedFrame, Tick};

use crate::audio::Device;
use crate::cli::Cli;
use crate::settings::{self, Settings, SettingsAction, SettingsKey, Setup};
use crate::shell::Sequence;
use crate::vault_menu::{self, Menu, MenuAction, MenuKey};

/// The window's title, and the first word of the status line.
const TITLE: &str = "Second Reality";

/// The menu's title bar. A run replaces it with its status line while it
/// plays; the menu puts it back when the run ends.
const VAULT_TITLE: &str = "Second Reality — vault";

/// The settings screen's title bar, until the run replaces it with its status line.
const SETUP_TITLE: &str = "Second Reality — setup";

/// The menu's font: `ALKU/FONA.UH` read as `ALKU/MAIN.C`'s text routine reads
/// it — 20-column cells, 30 rows, first cell `A` (`sr_assets::font` documents
/// the geometry). Every part reads its own media by the same relative path.
const MENU_FONT: &str = "assets/ALKU/FONA.UH";
const MENU_CELL: (u16, u16) = (20, 30);

/// How long the menu blocks waiting for an event. The menu is static, so this
/// is a wait and not a poll: nothing happens until a key or the close button
/// arrives.
const MENU_WAIT_MS: u32 = 50;

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
    let opts = cli.present_options();

    // The vault menu. It builds its own one-part runs as it plays them, so the
    // command line cannot name one (the CLI refuses it); nothing below the
    // branch changes for a normal run.
    if cli.vault {
        let sdl = sdl2::init()?;
        let video = sdl.video()?;
        let timer = sdl.timer()?;
        let mut events = sdl.event_pump()?;
        return vault_loop(&video, &timer, &mut events, &opts);
    }

    // A windowed demo run asks for its settings first (D44); the run is built from
    // what they say, so it is not built yet.
    if cli.shows_setup() {
        return setup_run(cli);
    }

    // The same run `--headless` dumps: a block's ported parts in the demo's own
    // order, or the one part `--part` named, numbered in the reference's frame
    // space.
    let mut run = crate::new_sequence(cli)?;
    let block = cli.block;

    let sdl = sdl2::init()?;
    let video = sdl.video()?;
    let timer = sdl.timer()?;
    // The sound card, or the dump, or nothing. A run that asked for a WAV writes the file
    // and opens no device — the ring has one consumer and a second one would take samples
    // away from the first — and a run that asked for neither (or for `--no-audio`) plays
    // nothing at all, which the music does not notice: its position is the tick's.
    let mut audio = audio_of(&sdl, &mut run, cli)?;

    let raster = output_size(run.frame(), &opts);
    let status = status_line(&run, block);
    let window = video
        .window(&status, raster.0 as u32, raster.1 as u32)
        .position_centered()
        .build()?;
    let mut canvas = window.into_canvas().build()?;
    let creator = canvas.texture_creator();
    let mut texture = streaming_texture(&creator, raster)?;
    let mut screen = Screen {
        canvas: &mut canvas,
        creator: &creator,
        buffer: FrameBuffer::new(0, 0),
        raster,
        status,
    };
    let mut events = sdl.event_pump()?;
    let end = present_run(
        &mut screen,
        &mut texture,
        &mut events,
        &timer,
        &opts,
        &mut run,
        &mut audio,
        block,
        false,
    )?;
    debug_assert_eq!(end, RunEnd::Close, "a normal run has no menu to return to");
    drop(screen);
    // The device is dropped with `audio`, which stops the callback; a dump is complete at
    // the last tick it wrote, because its header is patched on every write.
    drop(audio);
    if let Some(error) = run.audio_error() {
        return Err(format!("--audio-dump: {error}").into());
    }
    Ok(())
}

/// A demo run that opens on the settings screen: the screen, then the run it chose.
///
/// The window is opened once, on the screen's own frame, and the run is built and its
/// device opened only after Enter — a changed Sound quality has to reach the card's open
/// rate (`PlayerOptions::output_rate`), and a changed Sound has to decide whether there is
/// a card. The window then resizes to the run's frames as it does between a vault menu and
/// the part it starts. ESC and the close button leave without a run.
fn setup_run(cli: &Cli) -> Result<(), Box<dyn Error>> {
    let font = menu_font()?;
    let sdl = sdl2::init()?;
    let video = sdl.video()?;
    let timer = sdl.timer()?;

    let initial = Settings::from_cli(cli);
    let mut frame = IndexedFrame::new(settings::FRAME_WIDTH, settings::FRAME_HEIGHT);
    settings::draw(&initial, 0, &mut frame, &font);
    let opts = initial.present_options(cli.scale);
    let raster = output_size(&frame, &opts);
    let window = video
        .window(SETUP_TITLE, raster.0 as u32, raster.1 as u32)
        .position_centered()
        .build()?;
    let mut canvas = window.into_canvas().build()?;
    let creator = canvas.texture_creator();
    let mut texture = streaming_texture(&creator, raster)?;
    let mut screen = Screen {
        canvas: &mut canvas,
        creator: &creator,
        buffer: FrameBuffer::new(0, 0),
        raster,
        status: SETUP_TITLE.to_string(),
    };
    let mut events = sdl.event_pump()?;

    let Some(chosen) = setup_loop(&mut screen, &mut texture, &mut events, &font, initial, cli.scale)?
    else {
        return Ok(());
    };

    // The choices are the command line the run is built from.
    let mut cli = cli.clone();
    chosen.apply(&mut cli);
    let opts = chosen.present_options(cli.scale);
    let mut run = crate::new_sequence(&cli)?;
    let mut audio = audio_of(&sdl, &mut run, &cli)?;
    let end = present_run(
        &mut screen,
        &mut texture,
        &mut events,
        &timer,
        &opts,
        &mut run,
        &mut audio,
        cli.block,
        false,
    )?;
    debug_assert_eq!(end, RunEnd::Close, "a normal run has no menu to return to");
    drop(screen);
    drop(audio);
    if let Some(error) = run.audio_error() {
        return Err(format!("--audio-dump: {error}").into());
    }
    Ok(())
}

/// The settings screen's loop: the settings the user chose when Enter was pressed, or
/// `None` for ESC or the close button.
///
/// The screen is static between keys, so the loop blocks in bounded waits and cannot spin.
/// An expose presents the same frame again; a key that changes Aspect or Border presents the
/// redrawn frame under the new options, which resizes the window.
fn setup_loop<'a>(
    screen: &mut Screen<'a>,
    texture: &mut Texture<'a>,
    events: &mut EventPump,
    font: &Font,
    initial: Settings,
    scale: u32,
) -> Result<Option<Settings>, Box<dyn Error>> {
    let mut setup = Setup::new(initial);
    let mut frame = IndexedFrame::new(settings::FRAME_WIDTH, settings::FRAME_HEIGHT);
    settings::draw(&setup.settings, setup.selected, &mut frame, font);
    screen.present(texture, &frame, &setup.settings.present_options(scale))?;

    loop {
        let Some(event) = events.wait_event_timeout(MENU_WAIT_MS) else {
            continue;
        };
        if is_expose(&event) {
            screen.present(texture, &frame, &setup.settings.present_options(scale))?;
            continue;
        }
        let action = match event {
            Event::Quit { .. } => return Ok(None),
            Event::KeyDown { keycode, .. } => setup.key(settings_key(keycode)),
            _ => continue,
        };
        match action {
            SettingsAction::Redraw => {
                settings::draw(&setup.settings, setup.selected, &mut frame, font);
                screen.present(texture, &frame, &setup.settings.present_options(scale))?;
            }
            SettingsAction::Start => return Ok(Some(setup.settings)),
            SettingsAction::Exit => return Ok(None),
        }
    }
}

/// The settings key an SDL keycode asks for; a key the screen does not use is
/// [`SettingsKey::Other`].
fn settings_key(keycode: Option<Keycode>) -> SettingsKey {
    match keycode {
        Some(Keycode::Up) => SettingsKey::Up,
        Some(Keycode::Down) => SettingsKey::Down,
        Some(Keycode::Left) => SettingsKey::Left,
        Some(Keycode::Right) => SettingsKey::Right,
        Some(Keycode::Return | Keycode::KpEnter) => SettingsKey::Enter,
        Some(Keycode::Escape) => SettingsKey::Escape,
        _ => SettingsKey::Other,
    }
}

/// How a presented run ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RunEnd {
    /// The window closes: the close button, a run that could not start, or —
    /// for a normal run — ESC at the hold.
    Close,
    /// A vault run's ESC at the hold goes back to the menu.
    Menu,
}

/// The presentation state a window keeps across the runs it shows: the canvas,
/// its texture creator, and the upload buffer.
///
/// The texture is rebuilt when a frame's output raster changes (a part's
/// double-scan is the usual reason), and the upload buffer is reused. The
/// vault menu and the runs it starts share one `Screen`, so the window is
/// opened once and a run does not have to reopen it.
struct Screen<'a> {
    canvas: &'a mut WindowCanvas,
    creator: &'a TextureCreator<WindowContext>,
    buffer: FrameBuffer,
    raster: (usize, usize),
    /// The line currently on the title bar, so a tick only calls the window
    /// manager when the title changed.
    status: String,
}

impl<'a> Screen<'a> {
    /// Upload `frame` through `texture` and show it, rebuilding the texture
    /// and resizing the window first when the frame's output raster changed.
    ///
    /// The texture's lifetime is the screen's: both were created from the same
    /// texture creator, so a rebuilt texture is the same kind of object.
    fn present(
        &mut self,
        texture: &mut Texture<'a>,
        frame: &IndexedFrame,
        opts: &PresentOptions,
    ) -> Result<(), Box<dyn Error>> {
        let size = output_size(frame, opts);
        if size != self.raster {
            self.raster = size;
            *texture = streaming_texture(self.creator, size)?;
            self.canvas
                .window_mut()
                .set_size(size.0 as u32, size.1 as u32)?;
        }
        blit(self.canvas, texture, &mut self.buffer, frame, opts)
    }

    /// Put `line` on the title bar, if it is not already there.
    fn title(&mut self, line: String) -> Result<(), Box<dyn Error>> {
        if line != self.status {
            self.canvas.window_mut().set_title(&line)?;
            self.status = line;
        }
        Ok(())
    }
}

/// Present `run` in `screen` until it ends or the window closes.
///
/// This is the loop a window run has always had: poll the keys, advance the
/// run by wall-clock time, present the newest frame, keep the status line
/// current, and pace to the tick. The one new outcome is [`RunEnd::Menu`]: a
/// vault run whose screen is being held and whose ESC was pressed goes back to
/// the menu that started it. `return_to_menu` is false for every other run, so
/// that outcome is unreachable for them and their ESC semantics are unchanged.
///
/// `audio` is the run's device, if it has one; the vault passes `None` — its
/// three parts have no music — and the normal path passes what `audio_of`
/// opened.
#[allow(clippy::too_many_arguments)] // the window loop's own state, kept as arguments so it stays testable and explicit
fn present_run<'a>(
    screen: &mut Screen<'a>,
    texture: &mut Texture<'a>,
    events: &mut EventPump,
    timer: &sdl2::TimerSubsystem,
    opts: &PresentOptions,
    run: &mut Sequence,
    audio: &mut Option<Device>,
    block: Option<char>,
    return_to_menu: bool,
) -> Result<RunEnd, Box<dyn Error>> {
    let mut sampled = Instant::now();
    let mut escape = EscapeKey::default();
    loop {
        let keys = keys_of(events.poll_iter());
        if keys.close {
            return Ok(RunEnd::Close);
        }
        if keys.exit {
            escape.press();
        }
        if escape.apply(run) {
            // The exit flag has nothing left to act on: the run's parts are
            // over and the screen the last one left is being held. The
            // original's shell is at `@@partsdone` there (`MAIN/U2.ASM:935`) and
            // returns to DOS, which is this window closing — unless the run
            // came from the vault menu, where the press goes back to the menu.
            return Ok(run_end(run.holding(), return_to_menu));
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

        // A mid-run restart installs a new player (`MAIN/U2.ASM:880-885`, at U2E's
        // boundary in block 3), and the card was opened on the entry one. Hand the drain
        // the run's current player; the old ring's tail is dropped, as the module reload
        // interrupts it in the original too. `advance` above is what may have taken the
        // restart, and the next tick polls the new player.
        if let (Some(device), Some(player)) = (audio.as_mut(), run.audio_player()) {
            if !Arc::ptr_eq(&device.player(), &player) {
                device.follow(player);
            }
        }

        // `Engine::tick` sizes the frame from the view every tick, so a part
        // that changes the picture's geometry — double-scan takes it to 400
        // lines — changes the raster the texture and the window have to fit.
        screen.present(texture, run.frame(), opts)?;

        // The status line, when it has changed. The index in it moves every
        // tick, so this is one window-manager call a tick and not a redraw:
        // only the text is compared, and the frame on the screen is untouched.
        screen.title(status_line(run, block))?;

        // A run that has *finished* — a part the port has but could not start —
        // has no hold after it and nothing left to show; a demo run that has
        // *ended* has shown its exit stage's last blank frame, just presented.
        if run_is_over(run) {
            return Ok(RunEnd::Close);
        }
        // SDL's frame delay does the waiting — out the rest of the tick, and
        // never less than a millisecond, so the loop cannot spin.
        let left = TICK_PERIOD.saturating_sub(now.elapsed());
        timer.delay(left.as_millis().max(1) as u32);
    }
}

/// Whether the run has nothing left to show, so the window closes by itself:
/// it has *finished* (a part the port has could not start, and there is no hold
/// after it), or it has *ended* — a demo run's exit stage has faded and blanked,
/// and `fatalexit` returns to DOS (`MAIN/U2.ASM:962-994`).
fn run_is_over(run: &Sequence) -> bool {
    run.finished() || run.ended()
}

/// Which way the window goes when an ESC press reaches a held screen.
///
/// A normal run's hold is the original's `@@partsdone` and the press returns to
/// DOS — the window closes. A vault run was started by the menu, so the same
/// press goes back there instead.
fn run_end(holding: bool, return_to_menu: bool) -> RunEnd {
    if holding && return_to_menu {
        RunEnd::Menu
    } else {
        RunEnd::Close
    }
}

/// The menu key an SDL keycode asks for. A key the menu does not use is
/// [`MenuKey::Other`], which moves nothing; the close button is the caller's
/// event, not a key.
fn menu_key(keycode: Option<Keycode>) -> MenuKey {
    match keycode {
        Some(Keycode::Up) => MenuKey::Up,
        Some(Keycode::Down) => MenuKey::Down,
        Some(Keycode::Return | Keycode::KpEnter) => MenuKey::Enter,
        Some(Keycode::Escape) => MenuKey::Escape,
        _ => MenuKey::Other,
    }
}

/// Whether an SDL event is a window expose.
///
/// The menu is static — no part is running — so an expose needs no redraw,
/// but the frame must be presented again after a minimise/restore, an
/// occlusion or any other expose, or the menu can stay blank or stale until
/// a key arrives.
fn is_expose(event: &Event) -> bool {
    matches!(
        event,
        Event::Window {
            win_event: WindowEvent::Exposed,
            ..
        }
    )
}

/// The `--vault` window: the menu, the one-part runs it starts, and the way
/// back.
///
/// The menu is drawn once and redrawn only when the selection moves; between
/// events the loop blocks in SDL, so a static menu costs nothing. An expose
/// re-presents the same frame rather than redrawing it, so a minimise/restore
/// or an occlusion cannot leave the menu blank until a key. A chosen
/// part runs through the same [`present_run`] a normal run uses, with no
/// device (the vault's parts are silent) and with `return_to_menu` set; the
/// close button, or ESC in the menu, ends the window.
fn vault_loop(
    video: &VideoSubsystem,
    timer: &sdl2::TimerSubsystem,
    events: &mut EventPump,
    opts: &PresentOptions,
) -> Result<(), Box<dyn Error>> {
    let font = menu_font()?;
    let mut menu = Menu::new();
    let mut frame = IndexedFrame::new(vault_menu::FRAME_WIDTH, vault_menu::FRAME_HEIGHT);
    vault_menu::draw(&menu, &font, &mut frame);

    let raster = output_size(&frame, opts);
    let window = video
        .window(VAULT_TITLE, raster.0 as u32, raster.1 as u32)
        .position_centered()
        .build()?;
    let mut canvas = window.into_canvas().build()?;
    let creator = canvas.texture_creator();
    let mut texture = streaming_texture(&creator, raster)?;
    let mut screen = Screen {
        canvas: &mut canvas,
        creator: &creator,
        buffer: FrameBuffer::new(0, 0),
        raster,
        status: VAULT_TITLE.to_string(),
    };
    screen.present(&mut texture, &frame, opts)?;

    loop {
        // Block on the queue: the menu has nothing to do until a key, an
        // expose or the close button arrives, and a timed wait cannot spin.
        let Some(event) = events.wait_event_timeout(MENU_WAIT_MS) else {
            continue;
        };
        if is_expose(&event) {
            // The frame has not changed; only its presentation was lost.
            screen.present(&mut texture, &frame, opts)?;
            continue;
        }
        let action = match event {
            Event::Quit { .. } => return Ok(()),
            Event::KeyDown { keycode, .. } => menu.key(menu_key(keycode)),
            _ => continue,
        };
        match action {
            MenuAction::Redraw => {
                vault_menu::draw(&menu, &font, &mut frame);
                screen.present(&mut texture, &frame, opts)?;
            }
            MenuAction::Close => return Ok(()),
            MenuAction::Choose(name) => {
                // The vault's runs hold where they end and have no player, so the default
                // settings are theirs whatever `--sound-quality` said.
                let mut run =
                    crate::new_sequence_for_part(name, crate::shell::RunSettings::default())?;
                // The vault's parts are silent: `shell::music_for_part` has no
                // row for them, so the run has no player and no device is
                // opened. The slot is the same one the normal loop drains.
                let mut audio: Option<Device> = None;
                let end = present_run(
                    &mut screen,
                    &mut texture,
                    events,
                    timer,
                    opts,
                    &mut run,
                    &mut audio,
                    None,
                    true,
                )?;
                if end == RunEnd::Close {
                    return Ok(());
                }
                // Back in the menu: its title, and the frame it was showing.
                // The selection did not move while the part played, so the
                // frame is re-presented rather than redrawn.
                screen.title(VAULT_TITLE.to_string())?;
                screen.present(&mut texture, &frame, opts)?;
            }
        }
    }
}

/// Load the menu's font strip.
fn menu_font() -> Result<Font, Box<dyn Error>> {
    let image = load_path(Path::new(MENU_FONT)).map_err(|error| format!("{MENU_FONT}: {error}"))?;
    Ok(Font::from_image(image, MENU_CELL, 'A').map_err(|error| format!("{MENU_FONT}: {error}"))?)
}

/// Open what the command line asked the run to play through: a sound card, a WAV file, or
/// nothing.
///
/// **The device is a drain and nothing else**, and it is opened only in a window
/// (`--headless` never touches it). A machine that cannot open one — no card, no audio
/// subsystem, a driver that will not give the mixer's own rate — prints a line and runs
/// silent rather than failing: the demo must still run, and the picture is not the card's.
/// `--no-audio` is the same state asked for rather than discovered, and is why it prints
/// nothing.
fn audio_of(
    sdl: &sdl2::Sdl,
    run: &mut Sequence,
    cli: &Cli,
) -> Result<Option<Device>, Box<dyn Error>> {
    if let Some(path) = cli.audio_dump.as_deref() {
        run.dump_audio(path)?;
        println!(
            "secondreality: writing the music to {} (16-bit WAV at the mixer's own rate); \
             no sound card is opened",
            path.display()
        );
        return Ok(None);
    }
    if cli.no_audio {
        return Ok(None);
    }
    let Some(player) = run.audio_player() else {
        return Ok(None);
    };
    // Unreachable as an `Option`: a run with a player has a format, and the two come from
    // the same `Option<RunMusic>` — but the rate is the *player's* (that is the point of
    // `wav_format`) and re-typing a fallback here would be a second copy of it.
    let Some((rate, channels)) = run.audio_format() else {
        return Ok(None);
    };
    match Device::open(sdl, player, rate, channels as u8) {
        Ok(device) => {
            let (opened_rate, opened_channels, opened_samples) = device.spec();
            // The callback size is SDL's, not the desired spec's: report it, because the
            // margin is the player's ring (`Player::ring_capacity`: 8192 authentic, 19,660
            // enhanced) over `opened_samples × channels`.
            let ring = crate::audio::lock(&device.player()).ring_capacity();
            println!(
                "secondreality: audio at {opened_rate} Hz, {opened_channels} channel(s), \
                 {opened_samples} frame(s) a callback, a ring of {ring} samples"
            );
            Ok(Some(device))
        }
        // Not a failure: the demo runs silent, which is what a machine without a card
        // does. The line is the difference between "this machine has no audio" and "the
        // port forgot to play the music".
        Err(message) => {
            println!("secondreality: no audio device ({message}); the run plays silent");
            Ok(None)
        }
    }
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
            Event::KeyDown {
                keycode: Some(Keycode::Escape),
                ..
            } => keys.exit = true,
            Event::KeyDown {
                keycode: Some(Keycode::LCtrl | Keycode::RCtrl),
                ..
            } => keys.skip = true,
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
    /// window. A demo run's exit stage is that hold, so a press during its fade or
    /// its blank closes the window there and then: the original's stage reads no
    /// key, and not making a deliberate ESC wait it out is the port's courtesy
    /// (D41). The other state with nothing to play, a run that has *ended*, is
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
    // A looping run's later passes (`MAIN/U2.ASM:933`): the index keeps counting across
    // the seam, so the pass is what says the run has gone round.
    if run.pass() > 1 {
        line.push_str(&format!(" — pass {}", run.pass()));
    }
    line.push_str(" — ");
    line.push_str(run.part());
    match run.frame_index() {
        Some(index) => line.push_str(&format!(" — frame {index}")),
        None => line.push_str(" — starting"),
    }
    // The music, as `order:row` — the *published* position the parts read, which is the
    // one a person debugging a sync problem needs (`docs/fidelity/music.md` §8: it comes
    // through the ring and lags the player's own by ten polls, and a part that syncs is
    // asking about the music the machine is playing *now*). A run with no player says
    // nothing rather than `0:0`.
    if let Some(position) = run.music_position() {
        line.push_str(&format!(" — music {}:{}", position.order, position.row));
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
///
/// **SDL never scales the picture.** `present` has already magnified it — whole
/// blocks of duplicated pixels (D48) — and the texture is exactly that raster,
/// so it is copied into a destination rectangle of its own size
/// ([`placement`]): one texel to one output pixel, with nothing for a filter to
/// interpolate. A window the window manager made another size than the one
/// asked for gets the picture centred at 1:1, not stretched to fit.
fn blit<T: RenderTarget>(
    canvas: &mut Canvas<T>,
    texture: &mut Texture,
    buffer: &mut FrameBuffer,
    frame: &IndexedFrame,
    opts: &PresentOptions,
) -> Result<(), Box<dyn Error>> {
    present(frame, opts, buffer);
    // A streaming texture is write-only: updating it is what gets the frame to
    // the renderer, and `canvas` then paints it at its own size.
    texture.update(None, buffer.rgba(), buffer.width() * 4)?;
    let target = placement(canvas.output_size()?, (buffer.width(), buffer.height()));
    canvas.set_draw_color(Color::RGB(0, 0, 0));
    canvas.clear();
    canvas.copy(texture, None, Some(target))?;
    canvas.present();
    Ok(())
}

/// Where a `raster` texture goes in an `output`-sized renderer: centred, at its
/// own size — never stretched, so the renderer has no scaling to do and no
/// chance to filter. When the output is the raster (the window was given the
/// size it asked for) the rectangle is the whole output.
fn placement(output: (u32, u32), raster: (usize, usize)) -> Rect {
    let (w, h) = (raster.0 as u32, raster.1 as u32);
    let x = (output.0 as i64 - w as i64) / 2;
    let y = (output.1 as i64 - h as i64) / 2;
    Rect::new(x as i32, y as i32, w, h)
}

/// A streaming RGBA texture of `raster`, to upload presented frames into.
///
/// Its scale mode is set to **nearest** and its blend mode to **none**, on the
/// texture itself rather than through `SDL_HINT_RENDER_SCALE_QUALITY`, so no
/// environment variable or earlier hint can turn linear filtering on: if
/// anything ever did scale it, it would duplicate texels, and the copy is a
/// straight overwrite of the frame's opaque pixels.
fn streaming_texture<C>(
    creator: &TextureCreator<C>,
    raster: (usize, usize),
) -> Result<Texture<'_>, Box<dyn Error>> {
    let mut texture =
        creator.create_texture_streaming(TEXTURE_FORMAT, raster.0 as u32, raster.1 as u32)?;
    texture.set_scale_mode(ScaleMode::Nearest);
    texture.set_blend_mode(BlendMode::None);
    Ok(texture)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell::{
        start_view, Ending, Looping, RunSettings, Sequence, SequenceStatus, EXIT_BLANK_TICKS,
        EXIT_FADE_TICKS, LOOP_RELOAD_TICKS, SURFACE_HEIGHT, SURFACE_WIDTH,
    };
    use sr_engine::part::{Part, PartCtx, PartError, PartStatus};
    use sr_engine::present::Aspect;

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
            // A demo part this stub declines, which is what a part the port has
            // not written looks like to `Sequence`: the run holds and names it.
            // (Every part of the demo's order is registered now, so the hold
            // path is reached through a source that does not build one.)
            "techno" => None,
            _ => Some(Box::new(Stub { retires: None })),
        }
    }

    fn run_of(plan: Vec<&'static str>) -> Sequence {
        Sequence::with_source(
            SURFACE_WIDTH,
            SURFACE_HEIGHT,
            start_view(),
            plan,
            stub_source,
        )
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

    /// The static menu's one non-key event: an expose re-presents the frame
    /// (`vault_loop`'s `is_expose` branch) where another event falls through.
    #[test]
    fn an_expose_is_recognised_and_a_key_is_not() {
        let expose = Event::Window {
            timestamp: 0,
            window_id: 0,
            win_event: WindowEvent::Exposed,
        };
        assert!(is_expose(&expose));
        assert!(!is_expose(&key_down(Keycode::Up)));
        let other = Event::Window {
            timestamp: 0,
            window_id: 0,
            win_event: WindowEvent::FocusGained,
        };
        assert!(!is_expose(&other));
    }

    #[test]
    fn the_menu_keys_map_to_the_menu_actions() {
        // The SDL keycodes the menu listens for.
        assert_eq!(menu_key(Some(Keycode::Up)), MenuKey::Up);
        assert_eq!(menu_key(Some(Keycode::Down)), MenuKey::Down);
        assert_eq!(menu_key(Some(Keycode::Return)), MenuKey::Enter);
        assert_eq!(menu_key(Some(Keycode::KpEnter)), MenuKey::Enter);
        assert_eq!(menu_key(Some(Keycode::Escape)), MenuKey::Escape);
        // Everything else — including the keys a *run* acts on, which the menu
        // does not — is Other, and a key event with no keycode asks nothing.
        for other in [Keycode::Space, Keycode::LCtrl, Keycode::A, Keycode::F1] {
            assert_eq!(menu_key(Some(other)), MenuKey::Other, "{other:?}");
        }
        assert_eq!(menu_key(None), MenuKey::Other);

        // And the reducer through that mapping: Up and Down move the
        // selection (wrapping), Enter chooses the row it is on, Escape closes,
        // and an unhandled key leaves the selection where it was.
        let mut menu = Menu::new();
        assert_eq!(menu.key(menu_key(Some(Keycode::Up))), MenuAction::Redraw);
        assert_eq!(
            menu.key(menu_key(Some(Keycode::A))),
            MenuAction::Redraw,
            "an unhandled key moves nothing"
        );
        assert_eq!(
            menu.key(menu_key(Some(Keycode::Return))),
            MenuAction::Choose("twist"),
            "Up wrapped to the last row and A did not move it"
        );
        assert_eq!(menu.key(menu_key(Some(Keycode::Down))), MenuAction::Redraw);
        assert_eq!(
            menu.key(menu_key(Some(Keycode::Return))),
            MenuAction::Choose("grid"),
            "Down wrapped from the last row to the first"
        );
        assert_eq!(menu.key(menu_key(Some(Keycode::Escape))), MenuAction::Close);
    }

    #[test]
    fn the_settings_keys_map_to_the_settings_actions() {
        assert_eq!(settings_key(Some(Keycode::Up)), SettingsKey::Up);
        assert_eq!(settings_key(Some(Keycode::Down)), SettingsKey::Down);
        assert_eq!(settings_key(Some(Keycode::Left)), SettingsKey::Left);
        assert_eq!(settings_key(Some(Keycode::Right)), SettingsKey::Right);
        assert_eq!(settings_key(Some(Keycode::Return)), SettingsKey::Enter);
        assert_eq!(settings_key(Some(Keycode::KpEnter)), SettingsKey::Enter);
        assert_eq!(settings_key(Some(Keycode::Escape)), SettingsKey::Escape);
        for other in [Keycode::Space, Keycode::LCtrl, Keycode::A, Keycode::Tab] {
            assert_eq!(settings_key(Some(other)), SettingsKey::Other, "{other:?}");
        }
        assert_eq!(settings_key(None), SettingsKey::Other);
        // Through the reducer: Down, Right on Sound quality, then Enter starts with it.
        let mut setup = Setup::new(Settings::from_cli(&Cli::default()));
        for key in [Keycode::Down, Keycode::Right] {
            assert_eq!(setup.key(settings_key(Some(key))), SettingsAction::Redraw);
        }
        assert_eq!(setup.settings.quality, sr_audio::Quality::Enhanced);
        assert_eq!(setup.key(settings_key(Some(Keycode::Return))), SettingsAction::Start);
        assert_eq!(setup.key(settings_key(Some(Keycode::Escape))), SettingsAction::Exit);
    }

    /// The font strip as unit tests find it: they run in the binary's directory.
    fn settings_font() -> Font {
        let image = load_path(Path::new("../../assets/ALKU/FONA.UH")).expect("the font strip");
        Font::from_image(image, MENU_CELL, 'A').expect("the strip read as ALKU reads it")
    }

    #[test]
    fn changing_the_aspect_resizes_the_window() {
        // The raster the window is sized to is `output_size` of the screen's frame under the
        // options the settings give; Aspect is one of the options' fields, so changing it
        // changes the size, and `Screen::present` resizes on exactly that comparison.
        let font = settings_font();
        let mut frame = IndexedFrame::new(settings::FRAME_WIDTH, settings::FRAME_HEIGHT);
        let mut setup = Setup::new(Settings::from_cli(&Cli::default()));
        settings::draw(&setup.settings, setup.selected, &mut frame, &font);
        let size = |setup: &Setup| output_size(&frame, &setup.settings.present_options(1));
        assert_eq!(setup.settings.aspect, Aspect::SquarePixels, "the default is square (D48)");
        let square = size(&setup);

        setup.selected = 3; // Aspect
        assert_eq!(setup.key(SettingsKey::Right), SettingsAction::Redraw);
        assert_eq!(setup.settings.aspect, Aspect::FourThree, "the row wraps to 4:3");
        let four_three = size(&setup);
        assert_ne!(four_three, square, "4:3 is another raster, so the window resizes");
        // 4:3 stretches 520x325 by 6/5 into a 4:3 box (624x468); square keeps 520x325. The
        // border pads both by the same amount.
        let pad = (four_three.0 - 624) / 2;
        assert!(pad > 0, "the default run has its border");
        assert_eq!(four_three, (624 + 2 * pad, 468 + 2 * pad));
        assert_eq!(square, (520 + 2 * pad, 325 + 2 * pad));
        assert_eq!(setup.key(SettingsKey::Left), SettingsAction::Redraw);
        assert_eq!(size(&setup), square, "and back");
        // At the window's own scale the screen is exactly doubled: 1040x650 inside the pad.
        let doubled = output_size(&frame, &setup.settings.present_options(Cli::default().scale));
        assert_eq!(doubled, (1040 + 4 * pad, 650 + 4 * pad));
    }

    #[test]
    fn changing_the_border_resizes_the_window() {
        // The settings frame carries an overscan colour, so the Border row pads the raster.
        let font = settings_font();
        let mut frame = IndexedFrame::new(settings::FRAME_WIDTH, settings::FRAME_HEIGHT);
        let mut setup = Setup::new(Settings::from_cli(&Cli::default()));
        settings::draw(&setup.settings, setup.selected, &mut frame, &font);
        let with = output_size(&frame, &setup.settings.present_options(1));
        setup.selected = 4; // Border
        setup.key(SettingsKey::Right);
        assert!(!setup.settings.border);
        let without = output_size(&frame, &setup.settings.present_options(1));
        assert_eq!(without, (520, 325), "square, unpadded");
        assert!(with.0 > without.0 && with.1 > without.1, "{with:?} vs {without:?}");
    }

    /// A 320×200 frame whose every pixel differs from its neighbours, under a
    /// palette of 256 distinct colours, so a blended output pixel cannot match.
    fn distinct_frame() -> IndexedFrame {
        let mut frame = IndexedFrame::new(320, 200);
        for y in 0..200usize {
            for x in 0..320usize {
                frame.pixels[y * 320 + x] = ((x + 3 * y) % 256) as u8;
            }
        }
        let mut palette = sr_engine::Palette::BLACK;
        for i in 0..=255u8 {
            palette.set_entry(i, [i & 63, (i >> 6) * 21, 63 - (i & 63)]);
        }
        frame.palettes.reset(palette);
        frame
    }

    /// The RGBA colour index `index` widens to under `frame`'s palette.
    fn colour(frame: &IndexedFrame, index: u8) -> [u8; 4] {
        let mut table = [0u8; 1024];
        frame.palettes.base().write_rgba(&mut table);
        let at = index as usize * 4;
        [table[at], table[at + 1], table[at + 2], table[at + 3]]
    }

    /// Run `frame` through the window's own upload path — `streaming_texture` and
    /// `blit` — into a software renderer of `output`, and read back what it drew.
    /// No display is needed: SDL's software renderer draws into a surface.
    fn through_sdl(frame: &IndexedFrame, opts: &PresentOptions, output: (u32, u32)) -> Vec<u8> {
        let surface = sdl2::surface::Surface::new(output.0, output.1, TEXTURE_FORMAT)
            .expect("a surface to render into");
        let mut canvas = surface.into_canvas().expect("SDL's software renderer");
        let creator = canvas.texture_creator();
        let mut texture = streaming_texture(&creator, output_size(frame, opts)).expect("a texture");
        assert!(
            matches!(texture.scale_mode(), ScaleMode::Nearest),
            "the texture is never filtered"
        );
        let mut buffer = FrameBuffer::new(0, 0);
        blit(&mut canvas, &mut texture, &mut buffer, frame, opts).expect("the blit");
        canvas.read_pixels(None, TEXTURE_FORMAT).expect("the rendered pixels")
    }

    #[test]
    fn the_window_shows_a_320x200_frame_as_640x400_of_duplicated_pixels() {
        // The default options, through SDL: every source pixel is exactly a 2×2 block on
        // the screen, its colour the palette's, and no output pixel is a blend.
        let frame = distinct_frame();
        let opts = Cli::default().present_options();
        assert_eq!(opts, PresentOptions { aspect: Aspect::SquarePixels, scale: 2, border: true });
        assert_eq!(output_size(&frame, &opts), (640, 400), "no border colour, no pad");
        let drawn = through_sdl(&frame, &opts, (640, 400));
        for y in 0..400usize {
            for x in 0..640usize {
                let at = (y * 640 + x) * 4;
                let source = frame.pixels[(y / 2) * 320 + x / 2];
                assert_eq!(
                    drawn[at..at + 4],
                    colour(&frame, source),
                    "screen ({x}, {y}) is not source ({}, {})",
                    x / 2,
                    y / 2
                );
            }
        }
    }

    #[test]
    fn a_window_of_another_size_shows_the_picture_centred_and_unscaled() {
        // A window manager that does not give the size asked for: SDL is still not asked
        // to stretch, so the picture keeps its 2×2 blocks and the rest is black.
        let frame = distinct_frame();
        let opts = PresentOptions::default();
        assert_eq!(placement((640, 400), (640, 400)), Rect::new(0, 0, 640, 400));
        assert_eq!(placement((700, 450), (640, 400)), Rect::new(30, 25, 640, 400));
        assert_eq!(placement((600, 380), (640, 400)), Rect::new(-20, -10, 640, 400));
        let drawn = through_sdl(&frame, &opts, (700, 450));
        for y in 0..450usize {
            for x in 0..700usize {
                let at = (y * 700 + x) * 4;
                let expected = if (30..670).contains(&x) && (25..425).contains(&y) {
                    colour(&frame, frame.pixels[((y - 25) / 2) * 320 + (x - 30) / 2])
                } else {
                    [0, 0, 0, 255]
                };
                assert_eq!(drawn[at..at + 4], expected, "screen ({x}, {y})");
            }
        }
    }

    #[test]
    fn a_640x400_part_is_the_size_of_its_320x200_neighbours_in_the_window() {
        // ENDSCRL's 640×400 and ALKU's 320×400 are the size U2A's 320×200 is, so the
        // window keeps its size across the seams, and each is duplicated, not resampled.
        let opts = Cli::default().present_options();
        for (w, h, block) in [(320, 200, (2, 2)), (320, 400, (2, 1)), (640, 400, (1, 1))] {
            assert_eq!(output_size(&IndexedFrame::new(w, h), &opts), (640, 400), "{w}x{h}");
            assert_eq!(sr_engine::present::pixel_block(w, h, opts.scale), block, "{w}x{h}");
        }
    }

    #[test]
    fn an_escape_on_a_held_vault_part_returns_to_the_menu() {
        // A run with nothing left to play: `endlogo` retires on its first
        // tick, and the screen its picture left is held.
        let mut run = run_of(vec!["endlogo"]);
        run_out(&mut run, 4);
        assert!(run.holding(), "the run has nothing left to play");

        let mut escape = EscapeKey::default();
        escape.press();
        assert!(escape.apply(&mut run), "the press ends the held screen");
        assert_eq!(
            run_end(run.holding(), true),
            RunEnd::Menu,
            "a vault run goes back to the menu"
        );
        assert_eq!(
            run_end(run.holding(), false),
            RunEnd::Close,
            "a normal run's hold still closes the window"
        );

        // While a part is playing, the same press is the exit flag and does
        // not take the window anywhere, vault or not.
        let mut live = run_of(vec!["beglogo"]);
        live.tick();
        let mut escape = EscapeKey::default();
        escape.press();
        assert!(!escape.apply(&mut live), "there is a running part to ask");
        assert_eq!(run_end(live.holding(), true), RunEnd::Close);
    }

    /// A demo run (`Ending::Exit`) of stub parts: `endlogo` retires on its
    /// first tick, so the run's exit stage begins on the second.
    fn demo_run_of(plan: Vec<&'static str>) -> Sequence {
        Sequence::with_run(
            SURFACE_WIDTH,
            SURFACE_HEIGHT,
            start_view(),
            plan,
            stub_source,
            None,
            RunSettings { ending: Ending::Exit, ..RunSettings::default() },
        )
        .unwrap()
    }

    #[test]
    fn an_ended_run_closes_the_window() {
        // `fatalexit` returns to DOS once `printend` has run (`MAIN/U2.ASM:962-994`): the
        // window closes the tick the exit stage's last blank frame is up.
        let mut run = demo_run_of(vec!["endlogo"]);
        run.tick();
        assert!(!run_is_over(&run), "the part has retired; the exit stage is next");
        for _ in 0..EXIT_FADE_TICKS + EXIT_BLANK_TICKS - 1 {
            run.tick();
            assert!(!run_is_over(&run), "the fade and the blank are still on screen");
        }
        run.tick();
        assert!(run.ended());
        assert!(run_is_over(&run), "the stage is done and the window closes");

        // A held run never ends, so it never closes by itself.
        let mut held = run_of(vec!["endlogo"]);
        run_out(&mut held, 1_000);
        assert!(held.holding());
        assert!(!run_is_over(&held), "a part run holds until ESC or the close button");
    }

    #[test]
    fn escape_during_the_exit_stage_closes_at_once() {
        // The original's `fademusic` and twenty `copperwaitborder`s cannot be skipped
        // (`MAIN/U2.ASM:333-346`, `:969-973` — neither acts on a key); closing on a
        // deliberate ESC is the port's courtesy (D41).
        let mut run = demo_run_of(vec!["endlogo"]);
        run_out(&mut run, 10);
        assert!(run.holding(), "the run is ten ticks into its exit stage");
        assert!(!run.ended(), "and the stage has not run out");
        let mut escape = EscapeKey::default();
        escape.press();
        assert!(escape.apply(&mut run), "ESC during the stage closes the window");
        assert_eq!(run_end(run.holding(), false), RunEnd::Close);
    }

    /// A looping demo run (`--loop`) of stub parts: `endlogo` retires on its first
    /// tick, so the loop seam begins on the second, and pass 2's `alku` never retires.
    fn looping_run_of(plan: Vec<&'static str>) -> Sequence {
        Sequence::with_run(
            SURFACE_WIDTH,
            SURFACE_HEIGHT,
            start_view(),
            plan,
            stub_source,
            None,
            RunSettings { ending: Ending::Exit, looping: Looping::On, ..RunSettings::default() },
        )
        .unwrap()
    }

    #[test]
    fn a_looping_run_is_left_by_closing_the_window() {
        // ESC ends only the running part (`DIS/DISINT.ASM:151-153`); the seam between
        // two passes is not one, so a press there closes nothing, and a looping run
        // never ends by itself. The original's way out is its sticky Ctrl
        // (`MAIN/U2.ASM:920`); the port's Ctrl is D21's one-part skip, so the way out is
        // the window's close button.
        let mut run = looping_run_of(vec!["endlogo"]);
        run_out(&mut run, 10);
        assert!(!run.holding(), "ten ticks into the seam, the run is not at its end");
        let mut escape = EscapeKey::default();
        escape.press();
        assert!(!escape.apply(&mut run), "ESC in the seam does not close the window");
        run_out(&mut run, EXIT_FADE_TICKS + LOOP_RELOAD_TICKS);
        assert_eq!(run.pass(), 2);
        assert_eq!(run.part(), "alku", "and the press did not reach pass 2's first part");
        run_out(&mut run, 1_000);
        assert!(!run_is_over(&run), "a looping run never ends by itself");
        escape.press();
        assert!(!escape.apply(&mut run), "ESC in a part ends the part");
    }

    #[test]
    fn the_status_line_counts_the_pass_from_the_second_on() {
        let mut run = looping_run_of(vec!["endlogo"]);
        run.tick();
        assert_eq!(
            status_line(&run, Some('5')),
            "Second Reality — block 5 — endlogo — frame 404",
            "pass 1 says nothing about passes"
        );
        run_out(&mut run, EXIT_FADE_TICKS + LOOP_RELOAD_TICKS);
        assert_eq!(run.pass(), 1, "the seam is the end of pass 1");
        run.tick();
        let first = 405 + EXIT_FADE_TICKS + LOOP_RELOAD_TICKS;
        assert_eq!(
            status_line(&run, Some('5')),
            format!("Second Reality — block 5 — pass 2 — alku — frame {first}")
        );
    }

    /// The declared format must decode the bytes `present` wrote, in the order
    /// it wrote them. Getting this wrong does not fail loudly: the picture comes
    /// up in wrong colours and every test above still passes, because none of
    /// them touches a texture.
    #[test]
    fn the_texture_format_reads_the_presenters_bytes_in_order() {
        let masks = TEXTURE_FORMAT
            .into_masks()
            .expect("SDL knows its own formats");
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
        assert_eq!(
            keys_of([key_down(Keycode::Escape)]),
            Keys {
                exit: true,
                ..Keys::default()
            }
        );
        assert_eq!(
            keys_of([key_down(Keycode::LCtrl)]),
            Keys {
                skip: true,
                ..Keys::default()
            }
        );
        assert_eq!(
            keys_of([key_down(Keycode::RCtrl)]),
            Keys {
                skip: true,
                ..Keys::default()
            }
        );
        assert_eq!(
            keys_of([Event::Quit { timestamp: 0 }]),
            Keys {
                close: true,
                ..Keys::default()
            }
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
            assert_eq!(
                keys_of([key_down(other)]),
                Keys::default(),
                "{other:?} asks nothing"
            );
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
        assert_eq!(
            run.part(),
            "endscrl",
            "Ctrl skips the part it was pressed in"
        );
        assert_eq!(
            run.index(),
            404 + 4,
            "and the run's numbering never restarts"
        );
    }

    #[test]
    fn an_escape_between_two_parts_is_kept_and_then_closes_the_window() {
        // The seam after the last part of a block: `endlogo` retires on its
        // first tick, so the iteration that follows has no running part and no
        // hold yet — `Sequence::request_exit` has nothing to ask. That press is
        // "get me out", and a window that consumed it would need a third press
        // to close. It waits instead, and acts as soon as there is nothing left
        // to play.
        let mut run = run_of(vec!["endlogo", "techno"]);
        let mut escape = EscapeKey::default();

        run.tick();
        assert!(
            !run.holding(),
            "the part retired this tick; the hold is next"
        );
        escape.press();
        assert!(
            !escape.apply(&mut run),
            "nothing to ask, and somewhere to go: the press waits"
        );

        run.tick();
        assert!(
            run.holding(),
            "the next part is one this run's source does not build"
        );
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
        assert!(
            !escape.apply(&mut run),
            "the next part is asked, and the window stays open"
        );
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
        assert_eq!(
            status_line(&run, None),
            "Second Reality — beglogo — frame 405"
        );
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

    /// A run of `plan` with the *real* player — the modules read at the path a unit test
    /// reads them by ([`crate::audio::test_music`]) — and parts that draw nothing.
    ///
    /// The window's own stubs retire early (the key tests watch the handover); a run whose
    /// music is under test has to keep ticking, so `endlogo` and the two parts after it
    /// never retire here.
    fn music_run() -> Sequence {
        Sequence::with_run(
            SURFACE_WIDTH,
            SURFACE_HEIGHT,
            start_view(),
            crate::shell::block_plan('5').unwrap(),
            |name| match name {
                "endlogo" | "cred" | "endscrl" => Some(Box::new(Stub { retires: None })),
                other => stub_source(other),
            },
            Some(crate::audio::test_music),
            crate::shell::RunSettings::default(),
        )
        .unwrap()
    }

    #[test]
    fn the_status_line_shows_where_the_music_is() {
        // `order:row`, the *published* position a part reads (`docs/fidelity/music.md` §8:
        // it comes through the ring, ten polls behind the player's own) — which is what a
        // person debugging a sync problem is looking at, and what the phase's human check
        // is about.
        let mut run = music_run();
        assert_eq!(
            status_line(&run, Some('5')),
            "Second Reality — block 5 — endlogo — starting — music 0:0",
            "the seek publishes nothing: the ring still holds its priming"
        );
        // 30 ticks from the block's 404 put the run at frame 433, and the filed trace
        // (`reference/music/second5`) holds order 25, row 1 there — the first row the seek
        // published, ten polls after the engine's own row advance.
        run_out(&mut run, 30);
        assert_eq!(
            status_line(&run, Some('5')),
            "Second Reality — block 5 — endlogo — frame 433 — music 25:1"
        );
        // A run with no player says nothing about music rather than `0:0`.
        let mut plain = run_of(vec!["beglogo"]);
        plain.tick();
        assert_eq!(
            status_line(&plain, None),
            "Second Reality — beglogo — frame 404"
        );
    }

    #[test]
    fn a_run_that_stopped_says_so_and_names_the_part_that_holds_the_screen() {
        // A plan whose next part the run's source does not build: the run holds
        // the screen it has rather than ending silently, and the status line is
        // where the reason is. The part it names is the one whose picture is up
        // — not the one the run could not reach. (Every part of the demo's
        // order is registered now, so this stub declines TECHNO to reach the
        // path a not-yet-written part used to reach.)
        let mut run = run_of(vec!["endlogo", "techno"]);
        run_out(&mut run, 4);
        assert_eq!(
            status_line(&run, Some('2')),
            "Second Reality — block 2 — endlogo — frame 407 — holding; techno is not ported yet"
        );
        assert_eq!(run.stopped_at(), Some("techno"));

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
