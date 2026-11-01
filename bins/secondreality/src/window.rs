//! The windowed presenter: SDL2 video, and the demo's frames on the screen.
//!
//! SDL2 lives in the binary only. The window feeds wall-clock time to `Sequence::advance`
//! and blits the newest frame, dropping the ticks the display fell behind on, and puts
//! where the run is on the title bar ([`status_line`]).
//!
//! Three surfaces share one [`Window`]: a run ([`run`]), the settings screen a windowed
//! demo run opens on (D44, [`crate::menu::settings`]), and the `--vault` menu (D40,
//! [`crate::menu::vault`]), whose ESC at a held part goes back to the menu. The run's keys
//! are the original's (`DIS/DISINT.ASM:112-156`): ESC sets the exit flag (`:151-153`) and
//! Ctrl skips the running part (`MAIN/U2.ASM:205-207`). A held run waits for ESC
//! (`@@partsdone`, `MAIN/U2.ASM:935`); a demo run closes itself after its exit stage (D41);
//! a looping run (D42) is left by the close button.

use std::error::Error;
use std::sync::Arc;
use std::time::{Duration, Instant};

use sdl2::event::{Event, WindowEvent};
use sdl2::keyboard::Keycode;
use sdl2::pixels::{Color, PixelFormatEnum};
use sdl2::rect::Rect;
use sdl2::render::{BlendMode, Canvas, RenderTarget, ScaleMode, Texture, TextureCreator, WindowCanvas};
use sdl2::video::WindowContext;
use sdl2::{EventPump, TimerSubsystem};
use sr_assets::font::Font;
use sr_engine::present::{output_size, present, FrameBuffer, PresentOptions};
use sr_engine::{IndexedFrame, Tick};

use crate::audio::{self, Device};
use crate::cli::Cli;
use crate::menu::settings::{self, Settings, Setup};
use crate::menu::vault::{self, Menu};
use crate::menu::{self, Key};
use crate::shell::{RunSettings, Sequence};

/// The window's title, and the first word of the status line.
const TITLE: &str = "Second Reality";

/// The vault menu's title bar; a run replaces it while it plays.
const VAULT_TITLE: &str = "Second Reality — vault";

/// The settings screen's title bar.
const SETUP_TITLE: &str = "Second Reality — setup";

/// How long a menu blocks waiting for an event: a bounded wait, so it cannot spin.
const MENU_WAIT_MS: u32 = 50;

/// The texture format presented frames are uploaded in. `present` writes bytes red first,
/// which SDL calls `RGBA32` ("RGBA byte arrays", `SDL_pixels.h:273`), not `RGBA8888`,
/// whose name is by bit significance. Pinned by
/// `the_texture_format_reads_the_presenters_bytes_in_order`.
const TEXTURE_FORMAT: PixelFormatEnum = PixelFormatEnum::RGBA32;

/// One logic tick: the window is paced to the tick, never to the display's refresh.
const TICK_PERIOD: Duration = Duration::from_nanos(1_000_000_000_000 / Tick::HZ_MILLI);

/// Run what the command line named in a window.
pub fn run(cli: &Cli) -> Result<(), Box<dyn Error>> {
    if cli.vault {
        return vault_run(cli);
    }
    if cli.shows_setup() {
        return setup_run(cli);
    }
    let opts = cli.present_options();
    let mut run = crate::new_sequence(cli)?;
    let sdl = sdl2::init()?;
    let audio = audio::open_for(&sdl, &mut run, cli)?;
    let raster = output_size(run.frame(), &opts);
    let mut window = Window::open(&sdl, status_line(&run, cli.block), raster)?;
    let mut screen = window.screen()?;
    play(&mut screen, &opts, &mut run, audio, cli.block)
}

/// A demo run that opens on the settings screen. The run is built, and its device opened,
/// only after Enter, because Sound and Sound quality decide the card. ESC and the close
/// button leave without a run.
fn setup_run(cli: &Cli) -> Result<(), Box<dyn Error>> {
    let font = menu::font()?;
    let sdl = sdl2::init()?;
    let initial = Setup::new(Settings::from_cli(cli));
    let mut frame = IndexedFrame::new(settings::LAYOUT.width, settings::LAYOUT.height);
    initial.draw(&font, &mut frame);
    let raster = output_size(&frame, &initial.settings.present_options(cli.scale));
    let mut window = Window::open(&sdl, SETUP_TITLE.to_string(), raster)?;
    let mut screen = window.screen()?;

    let Some(chosen) = setup_loop(&mut screen, &font, initial, cli.scale)? else {
        return Ok(());
    };
    let mut cli = cli.clone();
    chosen.apply(&mut cli);
    let mut run = crate::new_sequence(&cli)?;
    let audio = audio::open_for(&sdl, &mut run, &cli)?;
    play(&mut screen, &cli.present_options(), &mut run, audio, cli.block)
}

/// Present a normal run until it closes, then report a WAV dump that failed.
fn play(
    screen: &mut Screen,
    opts: &PresentOptions,
    run: &mut Sequence,
    mut audio: Option<Device>,
    block: Option<char>,
) -> Result<(), Box<dyn Error>> {
    let end = present_run(screen, opts, run, &mut audio, block, false)?;
    debug_assert_eq!(end, RunEnd::Close, "a normal run has no menu to return to");
    // Dropping the device stops the callback; a dump's header is true at every write.
    drop(audio);
    audio::dump_result(run)
}

/// The settings screen's loop: the chosen settings on Enter, `None` on ESC or the close
/// button. A change to Aspect or Border presents under the new options, resizing the window.
fn setup_loop(
    screen: &mut Screen,
    font: &Font,
    mut setup: Setup,
    scale: u32,
) -> Result<Option<Settings>, Box<dyn Error>> {
    let mut frame = IndexedFrame::new(settings::LAYOUT.width, settings::LAYOUT.height);
    setup.draw(font, &mut frame);
    screen.present(&frame, &setup.settings.present_options(scale))?;
    loop {
        match screen.wait_input() {
            Input::Expose => screen.present(&frame, &setup.settings.present_options(scale))?,
            Input::Close => return Ok(None),
            Input::Key(key) => match setup.key(key) {
                settings::Action::Redraw => {
                    setup.draw(font, &mut frame);
                    screen.present(&frame, &setup.settings.present_options(scale))?;
                }
                settings::Action::Start => return Ok(Some(setup.settings)),
                settings::Action::Exit => return Ok(None),
            },
        }
    }
}

/// The `--vault` window: the menu, the one-part runs it starts, and the way back. A chosen
/// part runs through [`present_run`] with no device (the vault's parts are silent) and
/// with the default settings; ESC in the menu or the close button ends the window.
fn vault_run(cli: &Cli) -> Result<(), Box<dyn Error>> {
    let opts = cli.present_options();
    let sdl = sdl2::init()?;
    let font = menu::font()?;
    let mut menu = Menu::default();
    let mut frame = IndexedFrame::new(vault::LAYOUT.width, vault::LAYOUT.height);
    menu.draw(&font, &mut frame);
    let mut window = Window::open(&sdl, VAULT_TITLE.to_string(), output_size(&frame, &opts))?;
    let mut screen = window.screen()?;
    screen.present(&frame, &opts)?;

    loop {
        match screen.wait_input() {
            Input::Expose => screen.present(&frame, &opts)?,
            Input::Close => return Ok(()),
            Input::Key(key) => match menu.key(key) {
                vault::Action::Redraw => {
                    menu.draw(&font, &mut frame);
                    screen.present(&frame, &opts)?;
                }
                vault::Action::Close => return Ok(()),
                vault::Action::Choose(name) => {
                    let mut run = Sequence::new(vec![name], RunSettings::default())?;
                    let end = present_run(&mut screen, &opts, &mut run, &mut None, None, true)?;
                    if end == RunEnd::Close {
                        return Ok(());
                    }
                    // The selection did not move while the part played.
                    screen.title(VAULT_TITLE.to_string())?;
                    screen.present(&frame, &opts)?;
                }
            },
        }
    }
}

/// How a presented run ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RunEnd {
    /// The window closes.
    Close,
    /// A vault run's ESC at the hold goes back to the menu.
    Menu,
}

/// What a menu's blocking wait returns.
enum Input {
    /// The frame must be presented again (a minimise, an occlusion).
    Expose,
    /// The close button.
    Close,
    Key(Key),
}

/// The SDL window and what lives as long as it: opened once, centred, at its first
/// frame's raster.
struct Window {
    canvas: WindowCanvas,
    creator: TextureCreator<WindowContext>,
    events: EventPump,
    timer: TimerSubsystem,
    title: String,
    raster: (usize, usize),
}

impl Window {
    fn open(sdl: &sdl2::Sdl, title: String, raster: (usize, usize)) -> Result<Window, Box<dyn Error>> {
        let timer = sdl.timer()?;
        let window = sdl
            .video()?
            .window(&title, raster.0 as u32, raster.1 as u32)
            .position_centered()
            .build()?;
        let canvas = window.into_canvas().build()?;
        let creator = canvas.texture_creator();
        let events = sdl.event_pump()?;
        Ok(Window { canvas, creator, events, timer, title, raster })
    }

    /// The presentation state over this window, with a texture of its raster.
    fn screen(&mut self) -> Result<Screen<'_>, Box<dyn Error>> {
        let texture = streaming_texture(&self.creator, self.raster)?;
        Ok(Screen {
            canvas: &mut self.canvas,
            creator: &self.creator,
            texture,
            events: &mut self.events,
            timer: &self.timer,
            buffer: FrameBuffer::new(0, 0),
            raster: self.raster,
            status: self.title.clone(),
        })
    }
}

/// The presentation state a window keeps across what it shows: the texture is rebuilt
/// when a frame's output raster changes, and the upload buffer is reused.
struct Screen<'a> {
    canvas: &'a mut WindowCanvas,
    creator: &'a TextureCreator<WindowContext>,
    texture: Texture<'a>,
    events: &'a mut EventPump,
    timer: &'a TimerSubsystem,
    buffer: FrameBuffer,
    raster: (usize, usize),
    /// The title bar's line, so only a changed title calls the window manager.
    status: String,
}

impl Screen<'_> {
    /// Show `frame`, first rebuilding the texture and resizing the window if its output
    /// raster changed.
    fn present(&mut self, frame: &IndexedFrame, opts: &PresentOptions) -> Result<(), Box<dyn Error>> {
        let size = output_size(frame, opts);
        if size != self.raster {
            self.raster = size;
            self.texture = streaming_texture(self.creator, size)?;
            self.canvas
                .window_mut()
                .set_size(size.0 as u32, size.1 as u32)?;
        }
        blit(self.canvas, &mut self.texture, &mut self.buffer, frame, opts)
    }

    /// Put `line` on the title bar, if it is not already there.
    fn title(&mut self, line: String) -> Result<(), Box<dyn Error>> {
        if line != self.status {
            self.canvas.window_mut().set_title(&line)?;
            self.status = line;
        }
        Ok(())
    }

    /// Block until an expose, the close button or a key.
    fn wait_input(&mut self) -> Input {
        loop {
            let Some(event) = self.events.wait_event_timeout(MENU_WAIT_MS) else {
                continue;
            };
            match event {
                Event::Window { win_event: WindowEvent::Exposed, .. } => return Input::Expose,
                Event::Quit { .. } => return Input::Close,
                Event::KeyDown { keycode, .. } => return Input::Key(menu_key(keycode)),
                _ => {}
            }
        }
    }
}

/// The menu key an SDL keycode asks for.
fn menu_key(keycode: Option<Keycode>) -> Key {
    match keycode {
        Some(Keycode::Up) => Key::Up,
        Some(Keycode::Down) => Key::Down,
        Some(Keycode::Left) => Key::Left,
        Some(Keycode::Right) => Key::Right,
        Some(Keycode::Return | Keycode::KpEnter) => Key::Enter,
        Some(Keycode::Escape) => Key::Escape,
        _ => Key::Other,
    }
}

/// Present `run` until it ends or the window closes: poll the keys, advance by wall-clock
/// time, present the newest frame, keep the status line current, and pace to the tick.
/// `return_to_menu` makes ESC at the hold [`RunEnd::Menu`] (a vault run).
fn present_run(
    screen: &mut Screen,
    opts: &PresentOptions,
    run: &mut Sequence,
    audio: &mut Option<Device>,
    block: Option<char>,
    return_to_menu: bool,
) -> Result<RunEnd, Box<dyn Error>> {
    let mut sampled = Instant::now();
    let mut escape = EscapeKey::default();
    loop {
        let keys = keys_of(screen.events.poll_iter());
        if keys.close {
            return Ok(RunEnd::Close);
        }
        if keys.exit {
            escape.press();
        }
        if escape.apply(run) {
            // Nothing left to play: `@@partsdone` returns to DOS (`MAIN/U2.ASM:935`).
            return Ok(run_end(run.holding(), return_to_menu));
        }
        if keys.skip {
            run.skip_part();
        }

        // Sampled across iterations: anchored inside the loop it would time the drain alone.
        let now = Instant::now();
        run.advance(now - sampled);
        sampled = now;

        // A mid-run restart (`MAIN/U2.ASM:880-885`) installs a new player; the card was
        // opened on the entry one, so the drain follows. The old ring's tail is dropped.
        if let (Some(device), Some(player)) = (audio.as_mut(), run.audio_player()) {
            if !Arc::ptr_eq(&device.player(), &player) {
                device.follow(player);
            }
        }

        // The frame is sized from the view every tick (double-scan takes it to 400 lines).
        screen.present(run.frame(), opts)?;
        screen.title(status_line(run, block))?;

        if run_is_over(run) {
            return Ok(RunEnd::Close);
        }
        let left = TICK_PERIOD.saturating_sub(now.elapsed());
        screen.timer.delay(left.as_millis().max(1) as u32);
    }
}

/// Whether the window closes by itself: the run has *finished* (a part could not start),
/// or a demo run has *ended* after its exit stage (`fatalexit`, `MAIN/U2.ASM:962-994`).
fn run_is_over(run: &Sequence) -> bool {
    run.finished() || run.ended()
}

/// Where an ESC at a held screen takes the window: back to the menu for a vault run,
/// closed otherwise.
fn run_end(holding: bool, return_to_menu: bool) -> RunEnd {
    if holding && return_to_menu {
        RunEnd::Menu
    } else {
        RunEnd::Close
    }
}

/// What the keys seen this drain ask of the run: a function of the events alone, so the
/// key behaviour is testable without a display.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Keys {
    /// ESC: the exit flag (`DIS/DISINT.ASM:151-153`), asking the running part to quit
    /// (`exit_2`, `:186-190`).
    exit: bool,
    /// Ctrl: skip the running part (`MAIN/U2.ASM:205-207`, `partexecute`).
    skip: bool,
    /// The window was closed.
    close: bool,
}

/// The keys in `events`. `checkkeys` (`DIS/DISINT.ASM:112-156`) also reads `0`, `9` and
/// `1` (the mouse and the loader), which the port has not, and passes the rest to the
/// music (`passmuscode`). A key up asks nothing.
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

/// ESC, kept until the run can act on it. One iteration after a part retires there is no
/// part to ask and no hold yet; the original keeps the key in the BIOS buffer for the next
/// part's `checkkeys` (`DIS/DISINT.ASM:119-123`), and so does this.
#[derive(Debug, Default, Clone, Copy)]
struct EscapeKey {
    /// A press not yet applied to a part.
    waiting: bool,
}

impl EscapeKey {
    /// Note a press (auto-repeat is the same press).
    fn press(&mut self) {
        self.waiting = true;
    }

    /// Apply a waiting press, and say whether the window should close. A running part
    /// spends it; with no part it keeps waiting, unless the run is holding (`@@partsdone`,
    /// or a demo run's exit stage, which ESC cuts short, D41), where it closes.
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

/// The title bar: the part whose picture is up (during a hold, the last part's), the
/// frame's index in the reference's numbering, the block, the loop pass from the second
/// (`MAIN/U2.ASM:933`), and the music's published `order:row` (`docs/fidelity/music.md` §8).
fn status_line(run: &Sequence, block: Option<char>) -> String {
    let mut line = TITLE.to_string();
    if let Some(block) = block {
        line.push_str(" — block ");
        line.push(block);
    }
    if run.pass() > 1 {
        line.push_str(&format!(" — pass {}", run.pass()));
    }
    line.push_str(" — ");
    line.push_str(run.part());
    match run.frame_index() {
        Some(index) => line.push_str(&format!(" — frame {index}")),
        None => line.push_str(" — starting"),
    }
    if let Some(position) = run.music_position() {
        line.push_str(&format!(" — music {}:{}", position.order, position.row));
    }
    if !run.index_is_reference() {
        line.push_str(" (its own frame numbering, not the reference's)");
    }
    if run.holding() {
        line.push_str(" — holding; the run's parts are over");
    }
    line
}

/// Present `frame` into `buffer` and upload it. SDL never scales: `present` has already
/// magnified by whole pixel blocks (D48), and the texture is copied 1:1, centred
/// ([`placement`]), so a window of another size is not stretched.
fn blit<T: RenderTarget>(
    canvas: &mut Canvas<T>,
    texture: &mut Texture,
    buffer: &mut FrameBuffer,
    frame: &IndexedFrame,
    opts: &PresentOptions,
) -> Result<(), Box<dyn Error>> {
    present(frame, opts, buffer);
    texture.update(None, buffer.rgba(), buffer.width() * 4)?;
    let target = placement(canvas.output_size()?, (buffer.width(), buffer.height()));
    canvas.set_draw_color(Color::RGB(0, 0, 0));
    canvas.clear();
    canvas.copy(texture, None, Some(target))?;
    canvas.present();
    Ok(())
}

/// Where a `raster` texture goes in an `output`-sized renderer: centred, at its own size.
fn placement(output: (u32, u32), raster: (usize, usize)) -> Rect {
    let (w, h) = (raster.0 as u32, raster.1 as u32);
    let x = (output.0 as i64 - w as i64) / 2;
    let y = (output.1 as i64 - h as i64) / 2;
    Rect::new(x as i32, y as i32, w, h)
}

/// A streaming texture of `raster`, nearest-neighbour and unblended on the texture itself,
/// so no hint or environment variable can turn filtering on.
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

    /// Stub parts under the demo's names, which `shell::boot_anchor` keys on: `endlogo`
    /// retires at once, so a run of it reaches the hold; the others keep drawing.
    fn stub_source(name: &str) -> Option<Box<dyn Part>> {
        let retires = (name == "endlogo").then_some(0);
        Some(Box::new(Stub { retires }))
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

    /// A key press, as SDL delivers one, built because an `EventPump` needs a display.
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

    #[test]
    fn the_menu_keys_map_to_the_menus_own() {
        for (code, key) in [
            (Keycode::Up, Key::Up),
            (Keycode::Down, Key::Down),
            (Keycode::Left, Key::Left),
            (Keycode::Right, Key::Right),
            (Keycode::Return, Key::Enter),
            (Keycode::KpEnter, Key::Enter),
            (Keycode::Escape, Key::Escape),
            (Keycode::Space, Key::Other),
            (Keycode::LCtrl, Key::Other),
            (Keycode::A, Key::Other),
            (Keycode::Tab, Key::Other),
        ] {
            assert_eq!(menu_key(Some(code)), key, "{code:?}");
        }
        assert_eq!(menu_key(None), Key::Other);
    }

    /// A 320×200 frame whose every pixel differs from its neighbours, under a palette of 256
    /// distinct colours, so a blended output pixel cannot match.
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
    fn a_square_window_shows_a_320x200_frame_as_640x400_of_duplicated_pixels() {
        // `--aspect square` through SDL: every source pixel is exactly a 2×2 block on the
        // screen, its colour the palette's, and no output pixel is a blend.
        let frame = distinct_frame();
        let cli = Cli::parse(["--aspect", "square"].map(String::from)).expect("valid flags");
        let opts = cli.present_options();
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
        let opts = PresentOptions { aspect: Aspect::SquarePixels, ..PresentOptions::default() };
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

    /// The declared format must decode the bytes `present` wrote, in order: getting it
    /// wrong turns the screen red and no frame test notices.
    #[test]
    fn the_texture_format_reads_the_presenters_bytes_in_order() {
        let masks = TEXTURE_FORMAT
            .into_masks()
            .expect("SDL knows its own formats");
        // One pixel as `Palette::write_rgba` lays it out in memory, as the texture gets it.
        let packed = u32::from_ne_bytes([10, 20, 30, 255]);
        let channel = |mask: u32| ((packed & mask) >> mask.trailing_zeros()) as u8;
        assert_eq!(channel(masks.rmask), 10, "red");
        assert_eq!(channel(masks.gmask), 20, "green");
        assert_eq!(channel(masks.bmask), 30, "blue");
        assert_eq!(channel(masks.amask), 255, "alpha");
    }

    #[test]
    fn escape_asks_the_running_part_to_exit_and_ctrl_skips_it() {
        // The two keys `checkkeys` acts on (`DIS/DISINT.ASM:112-156`, `MAIN/U2.ASM:205-207`).
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

        // Everything else asks nothing: `passmuscode` (`:154`), and `0`, `9`, `1` are the
        // mouse and the loader, which the port has not.
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
        // A key up is not a press.
        assert_eq!(keys_of([key_up(Keycode::Escape)]), Keys::default());
        assert_eq!(keys_of([key_up(Keycode::LCtrl)]), Keys::default());

        // ESC ends the part it was pressed in; Ctrl skips it; the numbering keeps counting.
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
    fn an_escape_after_the_last_part_and_before_the_hold_is_kept_and_then_closes_the_window() {
        // After the plan's last part retires there is no part and no hold yet: the press
        // waits, and closes the window as soon as the run holds.
        let mut run = run_of(vec!["endlogo"]);
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
        assert!(run.holding(), "the plan has run out");
        assert!(escape.apply(&mut run), "and now the press is the way out");
    }

    #[test]
    fn an_escape_between_two_parts_is_asked_of_the_next_one() {
        // With a part still to play, the waiting press is spent on it, as the next part's
        // `checkkeys` takes the buffered key (`DIS/DISINT.ASM:119-123`).
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
        // `--part beglogo`: the same numbering, no block to name.
        assert_eq!(
            status_line(&run, None),
            "Second Reality — beglogo — frame 405"
        );
    }

    #[test]
    fn a_run_with_no_reference_anchor_says_its_index_is_not_the_references() {
        let mut run = run_of(vec!["testcard"]);
        run.tick();
        assert_eq!(
            status_line(&run, None),
            "Second Reality — testcard — frame 0 (its own frame numbering, not the reference's)"
        );
    }

    /// Block 5 with the real player ([`crate::audio::load_player`]) and stub parts that
    /// never retire.
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
            Some(crate::audio::load_player),
            crate::shell::RunSettings::default(),
        )
        .unwrap()
    }

    #[test]
    fn the_status_line_shows_where_the_music_is() {
        // The published `order:row` a part reads (`docs/fidelity/music.md` §8).
        let mut run = music_run();
        assert_eq!(
            status_line(&run, Some('5')),
            "Second Reality — block 5 — endlogo — starting — music 0:0",
            "the seek publishes nothing: the ring still holds its priming"
        );
        // Frame 433: the filed trace (`reference/music/second5`) holds order 25, row 1.
        run_out(&mut run, 30);
        assert_eq!(
            status_line(&run, Some('5')),
            "Second Reality — block 5 — endlogo — frame 433 — music 25:1"
        );
        // A run with no player says nothing about music.
        let mut plain = run_of(vec!["beglogo"]);
        plain.tick();
        assert_eq!(
            status_line(&plain, None),
            "Second Reality — beglogo — frame 404"
        );
    }

    #[test]
    fn a_held_run_says_its_parts_are_over() {
        let mut run = run_of(vec!["endlogo"]);
        run_out(&mut run, 4);
        assert_eq!(
            status_line(&run, Some('5')),
            "Second Reality — block 5 — endlogo — frame 407 — holding; the run's parts are over"
        );
    }
}
