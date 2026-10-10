//! The windowed presenter: SDL2 video, and the demo's frames on the screen.
//!
//! SDL2 lives in the binary and only in the binary; the engine itself has no
//! runtime dependency at all. The window is an engine client like any other:
//! it feeds wall-clock time to `Runner::advance` and blits the newest frame,
//! dropping the ticks the display fell behind on.

use std::error::Error;
use std::time::{Duration, Instant};

use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::PixelFormatEnum;
use sdl2::render::{Texture, TextureCreator, WindowCanvas};
use sdl2::video::WindowContext;
use sdl2::EventPump;
use sr_engine::part::PartStatus;
use sr_engine::present::{output_size, present, FrameBuffer, PresentOptions};
use sr_engine::{IndexedFrame, Tick};

use crate::cli::Cli;

/// The window's title.
const TITLE: &str = "Second Reality";

/// One logic tick's period, from the engine's own rate: the window is paced to
/// the tick, never to the display's refresh, so a 60 Hz panel shows some
/// frames twice rather than re-timing the demo.
const TICK_PERIOD: Duration = Duration::from_nanos(1_000_000_000_000 / Tick::HZ_MILLI);

/// Run the part in a window until ESC, the window is closed, or the part asks
/// to quit.
pub fn run(cli: &Cli) -> Result<(), Box<dyn Error>> {
    let opts = PresentOptions {
        aspect: cli.aspect,
        scale: cli.scale,
        border: cli.border,
    };
    let mut runner = crate::new_runner(&cli.part)?;

    let sdl = sdl2::init()?;
    let video = sdl.video()?;
    let timer = sdl.timer()?;

    let mut raster = output_size(runner.frame(), &opts);
    let window = video
        .window(TITLE, raster.0 as u32, raster.1 as u32)
        .position_centered()
        .build()?;
    let mut canvas = window.into_canvas().build()?;
    let creator = canvas.texture_creator();
    let mut texture = streaming_texture(&creator, raster)?;
    let mut buffer = FrameBuffer::new(0, 0);
    let mut events = sdl.event_pump()?;

    let mut sampled = Instant::now();
    loop {
        if quit_requested(&mut events) {
            break;
        }
        // The runner has no clock of its own: it is handed the wall-clock time
        // since it was last asked, so a display that fell behind gets the
        // newest frame only, never a burst of stale ones. The timestamp has to
        // outlive the iteration — anchored inside the loop it would measure the
        // event drain alone, and the demo would advance a tick a minute.
        let now = Instant::now();
        runner.advance(now - sampled);
        sampled = now;

        // `Engine::tick` sizes the frame from the view every tick, so a part
        // that changes the picture's geometry — double-scan takes it to 400
        // lines — changes the raster the texture and the window have to fit.
        let size = output_size(runner.frame(), &opts);
        if size != raster {
            raster = size;
            texture = streaming_texture(&creator, raster)?;
            canvas.window_mut().set_size(size.0 as u32, size.1 as u32)?;
        }
        blit(&mut canvas, &mut texture, &mut buffer, runner.frame(), &opts)?;

        if runner.exit_requested() || runner.status() == PartStatus::Finished {
            break;
        }
        // SDL's frame delay does the waiting — out the rest of the tick, and
        // never less than a millisecond, so the loop cannot spin.
        let left = TICK_PERIOD.saturating_sub(now.elapsed());
        timer.delay(left.as_millis().max(1) as u32);
    }
    Ok(())
}

/// Whether the user asked to quit: ESC, or the window's close button.
fn quit_requested(events: &mut EventPump) -> bool {
    events.poll_iter().any(|event| {
        matches!(
            event,
            Event::Quit { .. } | Event::KeyDown { keycode: Some(Keycode::Escape), .. }
        )
    })
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
        PixelFormatEnum::RGBA8888,
        raster.0 as u32,
        raster.1 as u32,
    )?)
}
