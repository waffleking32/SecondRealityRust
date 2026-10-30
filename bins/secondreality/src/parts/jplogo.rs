//! JPLOGO — the jelly logo.
//!
//! `JPLOGO/JP.C` (227 lines) over `JPLOGO/ASM.ASM` (`inittwk`, `setpalarea`,
//! `linezoom`), the generated `JPLOGO/ZOOM.INC` (one unrolled routine per even
//! scale 140..244) and `JPLOGO/READP.C`, with the linked `ICEKNGDM.UP`
//! (`_PIC.OBK`) and JPLOGO's own `SIN1024.INC` ([`SIN1024`]).
//!
//! `inittwk` (`ASM.ASM:36-74`) sets 400 lines, chain-4 off and leaves the row
//! at 80 plane bytes, so the picture is a flat 320×400 surface and
//! `scrolly(y)` (`JP.C:30-45`) is pixel origin `y*320`. A tick is one vblank:
//!
//! ```text
//! init         :107-210  the spring tables, the DAC cleared and the 512-byte upload,
//!                        readp's cropped rows, scrolly(400), one draw at scale 184
//! tick 0                 the scrolly(400) screen
//! ticks 1-83   :211-219  the entry: the start address walks from row 399 to 0
//! tick 84      :221      the dis_waitb() before doit
//! tick 85 on   :67-105   doit: the musplus < 4 gate, then one counter step a tick
//!                        (A27): y1/y2 from the tables until 511, the rows zoomed
//! ```
//!
//! Tick `t` is boot `17065 + t`, a mapping and not an anchor
//! (`docs/fidelity/jplogo.md` §3); `init` makes the music's head start itself
//! ([`MUSIC_PRE_ROLL`]). `doit`'s Scroll Lock border (`:74-76`) is not
//! modelled: the engine has no keyboard state, so a run never takes it.
//!
//! Preserved behaviour (`bugs.md` §4, §21): the 512-byte upload over the
//! cleared DAC ([`palette_upload`]); the crop at x=70 with the synthetic 65
//! border and the 0→64 remap ([`Jplogo::read_rows`]); this file's own sine
//! table; the interpolation's read past `framey1`/`framey2`
//! ([`interpolate_800`]); the unused `c=400` after 511; the palimpsest guard
//! ([`Jplogo::draw_doit_frame`]); `scrolly`'s two CRTC writes as one origin.

use sr_engine::part::{Part, PartCtx, PartError, PartStatus};
use sr_engine::Palette;

use crate::parts::sin1024::SIN1024 as COMMON_SIN1024;
use crate::parts::{asset, load_image, mode_view};

/// The part's registry name: what `--part` accepts.
const NAME: &str = "jplogo";

/// The linked picture (`_PIC.OBK`'s payload).
const PICTURE_PATH: &str = "assets/JPLOGO/ICEKNGDM.UP";

// --------------------------------------------------------------- screens --

/// `hdr->wid` × `hdr->hig`: the picture is the 320×400 screen.
const SCREEN_WIDTH: u16 = 320;
const SCREEN_HEIGHT: u16 = 400;
const PICTURE_WIDTH: usize = SCREEN_WIDTH as usize;
const PICTURE_HEIGHT: usize = SCREEN_HEIGHT as usize;

// ---------------------------------------------------------------- picture --

/// `memcpy(row[y],rowbuf+70,184)` (`JP.C:194`).
const CROP_LEFT: usize = 70;
const CROP_WIDTH: usize = 184;
/// `row[y][184]=65` (`:195`): the synthetic white border, source index 184.
const BORDER_INDEX: u8 = 65;
/// A source row and its border byte.
const ROW_BYTES: usize = CROP_WIDTH + 1;
/// The index the 0→64 remap writes (`:200`), whose palette entry `:188-190`
/// blacks.
const BLACK_INDEX: u8 = 64;

// ---------------------------------------------------------------- palette --

/// `setpalarea(palette,0,256)` uploads `2*count + start` = 512 bytes
/// (`ASM.ASM:104-126`): entries 0..169 whole, then entry 170's red and green.
const PALETTE_ENTRIES_WHOLE: usize = 512 / 3;

// ------------------------------------------------------------------ entry --

/// `a=64; y=400*64; while(y>0){ y-=a; a+=6; ... }` (`JP.C:211-219`).
const ENTRY_STEPS: usize = 83;
/// The tick the entry's step `i` first shows at (tick 0 is `scrolly(400)`).
const ENTRY_FIRST_TICK: u32 = 1;
/// `dis_waitb()` between the entry loop and `doit()` (`:221`).
const EXTRA_WAIT_TICK: u32 = ENTRY_FIRST_TICK + ENTRY_STEPS as u32;
/// The first tick `doit()`'s loop body runs on; its counter reads 1 there.
const DOIT_FIRST_TICK: u32 = EXTRA_WAIT_TICK + 1;

// -----------------------------------------------------------------  doit --

/// `while(!dis_exit() && frame<700)` (`JP.C:72`).
const FRAME_CAP: u32 = 700;
/// `if(frame>511) c=400; else { y1=...; y2=...; }` (`:78-83`).
const FREEZE_AFTER: u32 = 511;
/// Polls `init` makes on a fresh start: after the shell's seek to order 96
/// they put the player's row-58 boundary five ticks before tick 0, where the
/// trace's is (boot 17060), so 96:58 is published at tick 0 and every later
/// row boundary lands on the trace's.
const MUSIC_PRE_ROLL: u32 = 483;

// ----------------------------------------------------------------- tables --

/// `JPLOGO/SIN1024.INC`: the common table with ±255 at indices 256 and 768
/// instead of ±256 (`:10`, `:26`).
const SIN1024: [i16; 1024] = {
    let mut table = COMMON_SIN1024;
    table[256] = 255;
    table[768] = -255;
    table
};

// ------------------------------------------------------------- the spring --

/// A 16-bit C `int` assigned from `x`.
fn wrap16(x: i32) -> i32 {
    i32::from(x as i16)
}

/// `JP.C:117-163`: the two 200-entry spring tables, in the source's 16-bit
/// `int` arithmetic.
fn spring_tables() -> ([i16; 200], [i16; 200]) {
    let mut fy1 = [0i16; 200];
    let mut fy2 = [0i16; 200];
    let mut y1 = 0i32;
    let mut y1a = 500i32;
    let mut y2 = 399 * 16;
    let mut y2a = 500i32;
    let mut mika = 1i32;
    let mut halt = false;
    // `for(a=0;a<200;a++)` at `:114-115` leaves `a` at 200, which `la=a`
    // (`:140`) reads on the first pass.
    let mut a = 200i32;
    for frame in 0..200 {
        if !halt {
            y1 = wrap16(y1 + y1a);
            y2 = wrap16(y2 + y2a);

            y2a = wrap16(y2a + 16);
            if y2 > 400 * 16 {
                y2 = wrap16(y2 - y2a);
                y2a = wrap16(-y2a * mika / 8);
                if mika < 4 {
                    mika += 3;
                }
            }

            y1a = wrap16(y1a + 16);

            let la = a;
            a = wrap16((y2 - y1) - 400 * 16);
            if (a & 0x8000) ^ (la & 0x8000) != 0 {
                y1a = wrap16(y1a * 7 / 8);
            }
            y1a = wrap16(y1a + a / 8);
            y2a = wrap16(y2a - a / 8);
        }

        if frame > 90 {
            if y2 >= 399 * 16 {
                y2 = 400 * 16;
                halt = true;
            } else {
                y2a = 8;
            }
            y1 = wrap16(y2 - 400 * 16);
        }

        fy1[frame] = y1 as i16;
        fy2[frame] = y2 as i16;
    }
    (fy1, fy2)
}

/// `JP.C:164-171`: the 4× interpolation to 800 entries. For `a >= 796`,
/// `framey1[b+1]`/`framey2[b+1]` read one past each array, which in the
/// declaration order (`:23-26`) is `framey2[0]` and `framey1t[0]`; `doit`
/// never reads those four entries (`frame <= 511`).
fn interpolate_800(fy1: &[i16; 200], fy2: &[i16; 200]) -> ([i32; 800], [i32; 800]) {
    let mut t1 = [0i32; 800];
    let mut t2 = [0i32; 800];
    for a in 0..800 {
        let b = a / 4;
        let c = (a & 3) as i32;
        let d = 3 - c;
        let next1 = if b + 1 < 200 {
            i32::from(fy1[b + 1])
        } else {
            i32::from(fy2[0])
        };
        t1[a] = (i32::from(fy1[b]) * d + next1 * c) / 3;
        let next2 = if b + 1 < 200 {
            i32::from(fy2[b + 1])
        } else {
            t1[0]
        };
        t2[a] = (i32::from(fy2[b]) * d + next2 * c) / 3;
    }
    (t1, t2)
}

/// `JP.C:211-219`: the 83 start-address rows the entry loop walks.
fn entry_scrolls() -> Vec<u16> {
    let mut y = 400 * 64i32;
    let mut a = 64i32;
    let mut scrolls = Vec::with_capacity(ENTRY_STEPS);
    while y > 0 {
        y -= a;
        a += 6;
        if y < 0 {
            y = 0;
        }
        scrolls.push((y / 64) as u16);
    }
    scrolls
}

// --------------------------------------------------------------- the zoom --

/// One `linezoom(vram + y*80, src, a)` (`ASM.ASM:128-153` over `ZOOM.INC`).
///
/// The generated routines write every pixel of the fixed window `[32,288)`
/// and none outside it: source byte `((x-le)*185 + c/2)/c` for `x` in
/// `[le,ri]`, black otherwise. `a` is clamped to 318 with an unsigned compare
/// and rounded down to even (`:140-143`), and that index selects a label
/// (`DOL.C:31-38`): 0 is `@zoom0` (black), 2..138 `@zoom140`, 140..244 that
/// scale and 246..318 `@zoom244`. `src` `None` is `@zoom0`. The part's own
/// scales are 164..224 (`jplogo.md` §4.1), so the clamps are carried only.
fn zoom_row(pixels: &mut [u8], y: usize, src: Option<&[u8; ROW_BYTES]>, scale: i32) {
    let index = i32::from((scale as u16).min(318)) & !1;
    let a = if index == 0 { 0 } else { index.clamp(140, 244) };
    let row = &mut pixels[y * PICTURE_WIDTH..(y + 1) * PICTURE_WIDTH];
    let src = match src {
        Some(src) if a != 0 => src,
        _ => {
            row[32..288].fill(0);
            return;
        }
    };
    let le = 160 - a / 2;
    let ri = 160 + a / 2;
    let c = ri - le + 1;
    for (x, pixel) in (32..).zip(&mut row[32..288]) {
        *pixel = if x < le || x > ri { 0 } else { src[(((x - le) * 185 + c / 2) / c) as usize] };
    }
}

// ------------------------------------------------------------ the palette --

/// The DAC upload `setpalarea(palette,0,256)` performs on the DAC state
/// `dac`: entries 0..169 from the file (64 blacked, `JP.C:188-190`), entry
/// 170's red and green over `dac`'s blue, the rest untouched. `init` passes
/// black: `inittwk` (`ASM.ASM:31-38`) has written 768 zeros first.
fn palette_upload(file: &Palette, dac: Palette) -> Palette {
    let mut out = dac;
    for entry in 0..PALETTE_ENTRIES_WHOLE {
        let blacked = if entry == usize::from(BLACK_INDEX) {
            [0, 0, 0]
        } else {
            file.entry(entry as u8)
        };
        out.set_entry(entry as u8, blacked);
    }
    let partial = PALETTE_ENTRIES_WHOLE as u8;
    let [red, green, _] = file.entry(partial);
    out.set_entry(partial, [red, green, dac.entry(partial)[2]]);
    out
}

// --------------------------------------------------------------- the part --

/// JPLOGO's state: the decoded rows, the two interpolated tables, the entry's
/// start addresses, the palimpsest guard and the `doit` counter.
pub struct Jplogo {
    rows: Vec<[u8; ROW_BYTES]>,
    t1: [i32; 800],
    t2: [i32; 800],
    scrolls: Vec<u16>,
    lasty: [i32; PICTURE_HEIGHT],
    lasts: [i32; PICTURE_HEIGHT],
    /// `doit`'s `frame`.
    frame: u32,
    /// `doit`'s `y1`/`y2`, carried between iterations.
    y1: i32,
    y2: i32,
    gate_open: bool,
    done: bool,
}

impl Jplogo {
    /// A fresh part: `init` reads `assets/JPLOGO/ICEKNGDM.UP`.
    pub fn new() -> Jplogo {
        Jplogo {
            rows: Vec::new(),
            t1: [0; 800],
            t2: [0; 800],
            scrolls: Vec::new(),
            lasty: [-1; PICTURE_HEIGHT],
            lasts: [-1; PICTURE_HEIGHT],
            frame: 0,
            y1: 0,
            y2: 0,
            gate_open: false,
            done: false,
        }
    }

    /// The 400 rows `readp` decodes and `:191-201` crops: 184 bytes from x=70,
    /// source 0 remapped to 64, and the synthetic 65 border.
    fn read_rows(image: &sr_assets::Image) -> Vec<[u8; ROW_BYTES]> {
        let mut rows = Vec::with_capacity(PICTURE_HEIGHT);
        for y in 0..PICTURE_HEIGHT {
            let source = image.surface.row(y);
            let mut row = [0u8; ROW_BYTES];
            row[..CROP_WIDTH].copy_from_slice(&source[CROP_LEFT..CROP_LEFT + CROP_WIDTH]);
            for byte in &mut row[..CROP_WIDTH] {
                if *byte == 0 {
                    *byte = BLACK_INDEX;
                }
            }
            row[CROP_WIDTH] = BORDER_INDEX;
            rows.push(row);
        }
        rows
    }

    /// One `doit` body after the counter and `y1`/`y2` are settled
    /// (`JP.C:85-103`): a row in `[y1,y2)` is redrawn only when its `(b,a)`
    /// differ from `lasty`/`lasts`, so a repeated row keeps whatever an
    /// earlier state drew (the palimpsest); a row outside is cleared.
    fn draw_doit_frame(&mut self, pixels: &mut [u8]) {
        let xsc = (400 - (self.y2 - self.y1)) / 8;
        for y in 0..PICTURE_HEIGHT {
            let yi = y as i32;
            if yi < self.y1 || yi >= self.y2 {
                zoom_row(pixels, y, None, 0);
            } else {
                let b = (yi - self.y1) * 400 / (self.y2 - self.y1);
                let a = 184
                    + (i32::from(SIN1024[b as usize * 32 / 25]) * xsc + 32) / 64;
                let a = a & !1;
                if self.lasty[y] != b || self.lasts[y] != a {
                    zoom_row(pixels, y, Some(&self.rows[b as usize]), a);
                    self.lasty[y] = b;
                    self.lasts[y] = a;
                }
            }
        }
    }
}

impl Part for Jplogo {
    fn name(&self) -> &'static str {
        NAME
    }

    fn init(&mut self, ctx: &mut PartCtx) -> Result<(), PartError> {
        ctx.require_music(
            NAME,
            "this part opens with a music gate (`JP.C:71` \
             `while(!dis_exit() && dis_musplus()<4)`) and the run has no music",
        )?;

        let image = load_image(NAME, PICTURE_PATH)?;
        if (image.width() as usize, image.height() as usize) != (PICTURE_WIDTH, PICTURE_HEIGHT) {
            return Err(PartError::new(
                NAME,
                format!(
                    "{} is {}x{}; the picture's rows are {PICTURE_WIDTH}x{PICTURE_HEIGHT}",
                    asset(PICTURE_PATH),
                    image.width(),
                    image.height()
                ),
            ));
        }

        // `:117-171`'s tables, `:188-196`'s rows, `:211-219`'s entry.
        let (fy1, fy2) = spring_tables();
        let (t1, t2) = interpolate_800(&fy1, &fy2);
        *self = Jplogo {
            rows: Self::read_rows(&image),
            t1,
            t2,
            scrolls: entry_scrolls(),
            ..Jplogo::new()
        };
        ctx.set_palette(palette_upload(&image.palette, Palette::new()));

        // `inittwk`: 400 lines at 320 pixels a row, AC 0x11 = 0xFF, and the
        // `rep stosw` that clears all 256 KB through the unchained handler;
        // then `scrolly(400)` and one 400-row draw at scale 184 (`:204-210`).
        *ctx.view = mode_view(SCREEN_WIDTH, SCREEN_HEIGHT, Some(SCREEN_WIDTH), Some(255));
        ctx.view.set_origin(400 * u32::from(SCREEN_WIDTH));
        let pixels = ctx.surface.pixels_mut();
        pixels.fill(0);
        for y in 0..PICTURE_HEIGHT {
            zoom_row(pixels, y, Some(&self.rows[y]), 184);
        }

        // The head start belongs to the entry run alone.
        if ctx.music_fresh() {
            for _ in 0..MUSIC_PRE_ROLL {
                if let Some(music) = ctx.music() {
                    music.poll();
                }
            }
        }
        Ok(())
    }

    fn frame(&mut self, ctx: &mut PartCtx) -> PartStatus {
        if self.done {
            return PartStatus::Finished;
        }
        if ctx.exit_requested {
            self.done = true;
            return PartStatus::Finished;
        }

        // The entry: ticks 1..83 walk the start address, tick 84 is `:221`'s
        // wait.
        let tick = ctx.frame;
        if tick < DOIT_FIRST_TICK {
            let step = tick.checked_sub(ENTRY_FIRST_TICK);
            if let Some(&row) = step.and_then(|step| self.scrolls.get(step as usize)) {
                ctx.view.set_origin(u32::from(row) * u32::from(SCREEN_WIDTH));
            }
            return PartStatus::Continuing;
        }

        // `doit`'s gate (`JP.C:71`): wait for `musplus >= 4`. The wait is a
        // spin in the source; here it costs ticks, not `doit` frames.
        if !self.gate_open {
            let musplus = ctx.musplus();
            if musplus < 4 {
                return PartStatus::Continuing;
            }
            self.gate_open = true;
        }

        if self.frame >= FRAME_CAP {
            self.done = true;
            return PartStatus::Finished;
        }

        // `frame += c` (`:77`) with `c` the vblank count: the model is one a
        // tick (`docs/fidelity/APPROXIMATIONS.md`). `if(frame>511) c=400;`
        // (`:78`) never assigns `y1`/`y2` and `c` is not read again.
        self.frame += 1;
        if self.frame <= FREEZE_AFTER {
            self.y1 = self.t1[self.frame as usize] / 16;
            self.y2 = self.t2[self.frame as usize] / 16;
        }

        self.draw_doit_frame(ctx.surface.pixels_mut());
        PartStatus::Continuing
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::PlayerMusic;
    use crate::parts::read_asset;
    use crate::parts::test_rig::{repo, trace_position, Reading, Rig, ScriptedMusic};
    use sr_engine::music::Music;

    // ------------------------------------------------------- the source ----

    fn u16_at(bytes: &[u8], at: usize) -> usize {
        usize::from(u16::from_le_bytes([bytes[at], bytes[at + 1]]))
    }

    /// `READP.C:30-63`'s RLE loop. A byte >= 0x80 is a run of `b & 0x7f`
    /// (`dec ah / jnz`, 8-bit: 0x80 would wrap to 256 copies).
    fn rle_decode(data: &[u8], out: &mut Vec<u8>) {
        let mut i = 0;
        while i < data.len() {
            let b = data[i];
            i += 1;
            if b >= 0x80 {
                let mut ah = b & 0x7f;
                let value = data[i];
                i += 1;
                loop {
                    out.push(value);
                    ah = ah.wrapping_sub(1);
                    if ah == 0 {
                        break;
                    }
                }
            } else {
                out.push(b);
            }
        }
    }

    /// `READP.C`'s row read, transliterated over the file's bytes.
    fn reference_readp_row(bytes: &[u8], row: i32) -> Vec<u8> {
        let hig = u16_at(bytes, 4);
        if row >= 0 && row as usize >= hig {
            return Vec::new();
        }
        let add = u16_at(bytes, 8);
        let mut src = add * 16;
        let mut left = row;
        while left > 0 {
            let len = u16_at(bytes, src);
            src += len + 2;
            left -= 1;
        }
        let count = u16_at(bytes, src);
        src += 2;
        let mut out = Vec::new();
        rle_decode(&bytes[src..src + count], &mut out);
        out
    }

    fn reference_palette(bytes: &[u8]) -> Vec<u8> {
        let cols = u16_at(bytes, 6);
        bytes[16..16 + cols * 3].to_vec()
    }

    fn asset() -> sr_assets::Image {
        load_image(NAME, PICTURE_PATH).expect("ICEKNGDM.UP")
    }

    fn patterned_palette() -> Palette {
        let mut pal = Palette::new();
        for index in 0..256u16 {
            pal.set_entry(
                index as u8,
                [(index % 7) as u8, (index % 11) as u8, (index % 13) as u8],
            );
        }
        pal
    }

    // ------------------------------------------------------ the tables ----

    #[test]
    fn the_sine_table_is_the_sources_own_1024_words() {
        let text = std::fs::read_to_string(repo("upstream/SecondReality/JPLOGO/SIN1024.INC"))
            .expect("SIN1024.INC");
        let mut words = Vec::new();
        for line in text.lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("dw") {
                for word in rest.split(',') {
                    let word = word.trim();
                    if !word.is_empty() {
                        words.push(word.parse::<i16>().unwrap());
                    }
                }
            }
        }
        assert_eq!(words.len(), 1024);
        assert_eq!(&words[..], &SIN1024[..]);
        assert_eq!([SIN1024[256], SIN1024[768]], [255, -255], "not the common copy's ±256");
    }

    #[test]
    fn the_spring_tables_are_the_sources_own_first_frames() {
        let (fy1, fy2) = spring_tables();
        assert_eq!(&fy1[..6], &[500, 885, 1172, 1323, 1317, 1153]);
        assert_eq!(&fy2[..6], &[6368, 6370, 6354, 6338, 6322, 6306]);
        assert!(fy1[95..].iter().all(|&v| v == 0), "framey1[95:]");
        assert!(fy2[95..].iter().all(|&v| v == 6400), "framey2[95:]");
    }

    #[test]
    fn the_interpolation_is_the_sources_own_and_its_overrun_is_unreachable() {
        let (fy1, fy2) = spring_tables();
        let (t1, t2) = interpolate_800(&fy1, &fy2);
        assert_eq!(&t1[..6], &[500, 628, 756, 885, 885, 980]);
        assert_eq!(&t2[..6], &[6368, 6368, 6369, 6370, 6370, 6364]);
        assert_eq!(&t1[796..], &[0, 2122, 4245, 6368]);
        assert_eq!(&t2[796..], &[6400, 4433, 2466, 500]);
        // The settle: the first counter whose two entries are the whole
        // screen.
        assert_eq!((t1[379] / 16, t2[379] / 16), (0, 400));
        // `doit` reads `framey1t[frame]` for `frame <= 511`; the four entries
        // the out-of-bounds read computes are never read.
        const { assert!(FREEZE_AFTER <= 511) };
    }

    #[test]
    fn the_entry_walks_eighty_three_start_rows_from_399_to_zero() {
        let scrolls = entry_scrolls();
        assert_eq!(scrolls.len(), ENTRY_STEPS);
        assert_eq!(scrolls[0], 399);
        assert_eq!(scrolls[1], 397);
        assert_eq!(scrolls[2], 396);
        assert_eq!(*scrolls.last().unwrap(), 0);
        assert!(scrolls.windows(2).all(|w| w[0] > w[1]), "strictly decreasing");
    }

    // -------------------------------------------------------- the zoom ----

    #[test]
    fn the_scale_dispatch_is_the_tables_and_the_reachable_range_stays_inside() {
        let image = asset();
        let rows = Jplogo::read_rows(&image);
        let render = |scale: i32| {
            let mut pixels = vec![0xAAu8; PICTURE_WIDTH * PICTURE_HEIGHT];
            zoom_row(&mut pixels, 0, Some(&rows[0]), scale);
            pixels[..PICTURE_WIDTH].to_vec()
        };
        // `DOL.C`'s table: 246..318 is `@zoom244`, 2..138 is `@zoom140`,
        // 140..244 is that label and 0 is `@zoom0`.
        assert_eq!(render(318), render(244));
        assert_eq!(render(320), render(244));
        assert_eq!(render(-3), render(244));
        assert_eq!(render(2), render(140));
        assert_eq!(render(141), render(140));
        // `@zoom0` and a clear (`linezoom(...,NULL,0)`) write black inside
        // the window and leave the fixed margins alone.
        let mut zero = vec![0xAAu8; PICTURE_WIDTH];
        zero[32..288].fill(0);
        assert_eq!(render(0), zero);
        let mut cleared = vec![0xAAu8; PICTURE_WIDTH];
        zoom_row(&mut cleared, 0, None, 184);
        assert_eq!(cleared, zero);
        // The part's own reachable scales, over every `doit` frame.
        let (fy1, fy2) = spring_tables();
        let (t1, t2) = interpolate_800(&fy1, &fy2);
        let (mut lo, mut hi) = (i32::MAX, i32::MIN);
        for frame in 1..=FREEZE_AFTER as usize {
            let y1 = t1[frame] / 16;
            let y2 = t2[frame] / 16;
            let xsc = (400 - (y2 - y1)) / 8;
            let first = y1.max(0);
            let last = y2.min(PICTURE_HEIGHT as i32);
            for y in first..last {
                let b = (y - y1) * 400 / (y2 - y1);
                let a = 184
                    + (i32::from(SIN1024[b as usize * 32 / 25]) * xsc + 32) / 64;
                let a = a & !1;
                lo = lo.min(a);
                hi = hi.max(a);
            }
        }
        assert_eq!((lo, hi), (164, 224), "inside 140..=244: the clamps are unreachable");
    }

    #[test]
    fn scale_184_is_the_identity_over_the_185_source_bytes() {
        let image = asset();
        let rows = Jplogo::read_rows(&image);
        let row = &rows[7];
        let mut surface = vec![0xAAu8; PICTURE_WIDTH * PICTURE_HEIGHT];
        zoom_row(&mut surface, 0, Some(row), 184);
        let out = &surface[..PICTURE_WIDTH];
        assert_eq!(out[68], row[0]);
        assert_eq!(out[251], row[183]);
        assert_eq!(out[252], BORDER_INDEX, "source index 184 is the 65 border");
        assert_eq!(out[67], 0);
        assert_eq!(out[253], 0);
    }

    // ----------------------------------------------------- the picture ----

    #[test]
    fn readp_is_the_sources_row_rle_and_palette() {
        let bytes = read_asset(NAME, PICTURE_PATH).unwrap();
        assert_eq!(u16_at(&bytes, 0), 0xFCFD, "FD FC, the RLE variant");
        assert_eq!(u16_at(&bytes, 2), 320);
        assert_eq!(u16_at(&bytes, 4), 400);
        assert_eq!(u16_at(&bytes, 6), 256);
        assert_eq!(u16_at(&bytes, 8), 49);
        let image = asset();
        assert_eq!(
            reference_palette(&bytes),
            image.palette.to_vga().iter().map(|b| b >> 2).collect::<Vec<u8>>(),
            "the palette at +16 is the asset's, entry for entry"
        );
        for row in 0..400 {
            let decoded = reference_readp_row(&bytes, row);
            assert_eq!(decoded.len(), 320, "row {row} decodes its 320 pixels");
            assert_eq!(
                &decoded[..],
                image.surface.row(row as usize),
                "row {row} of READP.C"
            );
        }
    }

    #[test]
    fn the_rows_are_the_184_byte_crop_remapped_with_a_65_border() {
        let image = asset();
        let rows = Jplogo::read_rows(&image);
        assert_eq!(rows.len(), PICTURE_HEIGHT);
        let bytes = read_asset(NAME, PICTURE_PATH).unwrap();
        for y in [0usize, 1, 100, 399] {
            let raw = reference_readp_row(&bytes, y as i32);
            for b in 0..CROP_WIDTH {
                let expected = if raw[CROP_LEFT + b] == 0 {
                    BLACK_INDEX
                } else {
                    raw[CROP_LEFT + b]
                };
                assert_eq!(rows[y][b], expected, "row {y} byte {b}");
            }
            assert_eq!(rows[y][CROP_WIDTH], BORDER_INDEX);
        }
        assert!(!rows.iter().any(|row| row[..CROP_WIDTH].contains(&0)));
    }

    #[test]
    fn the_picture_uses_only_colours_the_512_byte_upload_writes() {
        let image = asset();
        let rows = Jplogo::read_rows(&image);
        let mut highest = 0u8;
        for row in &rows {
            highest = highest.max(*row.iter().max().unwrap());
        }
        assert_eq!(highest, BORDER_INDEX, "below entry 169, the last whole one");
    }

    // ----------------------------------------------------- the palette ----

    #[test]
    fn init_clears_the_dac_before_the_512_byte_upload() {
        // `inittwk`'s `mov cx,768 / out dx,al` (`ASM.ASM:31-38`) zeroes the
        // DAC before the upload, so an inherited palette cannot survive past
        // entry 169 — and entry 170's blue is that zero, not the old DAC's.
        let mut rig = real_rig();
        rig.palette = patterned_palette();
        let mut part = Jplogo::new();
        part.init(&mut rig.init_ctx(0)).unwrap();
        let file = asset().palette;
        assert_eq!(
            rig.palette.entry(170),
            [file.entry(170)[0], file.entry(170)[1], 0],
            "entry 170's blue is the cleared DAC's"
        );
        for entry in 171..256u16 {
            assert_eq!(
                rig.palette.entry(entry as u8),
                [0, 0, 0],
                "entry {entry} was not written by the 512 bytes and stayed cleared"
            );
        }
        assert_eq!(rig.palette.entry(64), [0, 0, 0], "the blacked entry");
        assert_eq!(rig.palette.entry(1), file.entry(1), "the file's first entry");
    }

    #[test]
    fn the_palette_upload_is_the_sources_512_bytes() {
        let image = asset();
        let file = image.palette;
        let live = patterned_palette();
        let out = palette_upload(&file, live);
        for entry in 0..PALETTE_ENTRIES_WHOLE {
            let expected = if entry == usize::from(BLACK_INDEX) {
                [0, 0, 0]
            } else {
                file.entry(entry as u8)
            };
            assert_eq!(out.entry(entry as u8), expected, "entry {entry}");
        }
        // The 512th byte is entry 170's green; its blue is the DAC's old one.
        let partial = PALETTE_ENTRIES_WHOLE as u8;
        assert_eq!(
            out.entry(partial),
            [file.entry(partial)[0], file.entry(partial)[1], live.entry(partial)[2]]
        );
        for entry in PALETTE_ENTRIES_WHOLE + 1..256 {
            assert_eq!(out.entry(entry as u8), live.entry(entry as u8), "entry {entry}");
        }
        assert_eq!(file.entry(0), [0, 0, 0], "the picture's own entry 0 is black too");
    }

    // -------------------------------------------------------- the part ----

    /// A rig with the real player, `MUSIC1` seeked to order 96. The part sets
    /// no copper, so a step is the engine's poll and the part's `frame`.
    fn real_rig() -> Rig<PlayerMusic> {
        use crate::audio::{lock, test_player};
        use crate::shell::ModuleId;
        let player = test_player(ModuleId::Music1).unwrap();
        lock(&player).play_song(96);
        Rig::new(SCREEN_WIDTH, SCREEN_HEIGHT).with(PlayerMusic::new(player, 0))
    }

    #[test]
    fn the_view_is_320_by_400_at_80_plane_bytes_a_row() {
        let mut rig = real_rig();
        let mut part = Jplogo::new();
        part.init(&mut rig.init_ctx(0)).unwrap();
        assert_eq!((rig.view.width, rig.view.height), (SCREEN_WIDTH, SCREEN_HEIGHT));
        assert!(!rig.view.double_scan, "CRTC 0x09 = 0");
        assert_eq!(rig.view.y_line_offset, Some(SCREEN_WIDTH));
        assert_eq!(rig.view.start_addr, 400 * 320, "scrolly(400)");
        assert_eq!(rig.view.pan, 0);
        assert_eq!(rig.view.split, None);
        assert_eq!(rig.view.border, Some(255), "AC 0x11 = 0xFF");
    }

    #[test]
    fn the_entry_ticks_walk_scroll_y_from_399_to_zero() {
        let mut rig = real_rig();
        let mut part = Jplogo::new();
        part.init(&mut rig.init_ctx(0)).unwrap();
        // Tick 0 shows the start-400 screen `init` left.
        rig.step(&mut part);
        assert_eq!(rig.view.start_addr, 400 * 320);
        let scrolls = entry_scrolls();
        for (step, &row) in scrolls.iter().enumerate() {
            rig.step(&mut part);
            assert_eq!(
                rig.view.start_addr,
                u32::from(row) * 320,
                "tick {} is entry step {step}",
                step + 1
            );
        }
        // The extra `dis_waitb()` before `doit()` holds start 0.
        rig.step(&mut part);
        assert_eq!(rig.view.start_addr, 0);
    }

    #[test]
    fn the_entry_pre_roll_reproduces_the_traces_positions() {
        // The pre-roll's derivation: from a fresh seek, the published row-58
        // window begins at the 479th poll. The trace's row 58 begins at boot
        // 17060, five frames before tick 0's 17065, and tick 0's poll is the
        // (pre-roll + 1)th — so 483 + 1 - 5 = 479.
        {
            use crate::audio::{lock, test_player};
            use crate::shell::ModuleId;
            let player = test_player(ModuleId::Music1).unwrap();
            lock(&player).play_song(96);
            let mut music = PlayerMusic::new(player, 0);
            let mut polls = 0u32;
            loop {
                music.poll();
                polls += 1;
                let position = music.position();
                if position.order == 96 && position.row == 58 {
                    break;
                }
            }
            assert_eq!(polls, 479);
            assert_eq!(MUSIC_PRE_ROLL + 1 - 5, polls);
        }
        let mut rig = real_rig();
        let mut part = Jplogo::new();
        part.init(&mut rig.init_ctx(0)).unwrap();
        // The player is fresh from `_zplaysong(96)`; 483 polls leave it at
        // the row-58 window, whose start is five ticks before tick 0.
        let wanted = [(0u32, 96u8, 58u8), (3, 96, 59), (44, 98, 0), (76, 98, 4),
                      (84, 98, 5), (235, 98, 23), (435, 98, 48), (635, 98, 49)];
        for (tick, order, row) in wanted {
            while rig.tick < tick {
                rig.step(&mut part);
            }
            if rig.tick == tick {
                rig.step(&mut part);
            }
            assert_eq!(
                rig.player().position().order,
                order,
                "tick {tick}: the trace at {} is {order}:{row}",
                17065 + tick
            );
            assert_eq!(rig.player().position().row, row, "tick {tick}");
            assert_eq!(
                (rig.player().position().order, rig.player().position().row),
                trace_position("second3d", 17065 + tick),
                "tick {tick} against the trace"
            );
            if tick == 84 {
                // By the extra wait before `doit()` the gate reads
                // `musplus >= 4` (order 98 row 5).
                assert!(rig.player().musplus() >= 4);
            }
        }
    }

    #[test]
    fn a_mid_chain_start_does_not_pre_poll() {
        let mut rig = real_rig();
        let mut part = Jplogo::new();
        part.init(&mut rig.init_ctx_chain(0)).unwrap();
        // `_zplaysong` publishes nothing until the ring drains: no pre-poll.
        assert_eq!(rig.player().position().order, 0);
        assert_eq!(rig.player().position().row, 0);
    }

    #[test]
    fn the_gate_waits_for_musplus_at_least_four() {
        let music = ScriptedMusic::musplus(vec![-6]).never_syncs("the gate does not sync");
        let mut rig = Rig::new(SCREEN_WIDTH, SCREEN_HEIGHT).with(music);
        let mut part = Jplogo::new();
        part.init(&mut rig.ctx(0)).unwrap();
        let mut tick = |part: &mut Jplogo, plus: i32, frame: u32| {
            rig.player().script = vec![Reading { musplus: plus, ..Reading::default() }];
            part.frame(&mut rig.ctx(frame))
        };
        // The entry, then the extra wait.
        for frame in 0..=EXTRA_WAIT_TICK {
            assert_eq!(tick(&mut part, -6, frame), PartStatus::Continuing);
        }
        // The first `doit` tick with the gate closed must not advance the
        // counter.
        assert_eq!(tick(&mut part, -6, DOIT_FIRST_TICK), PartStatus::Continuing);
        assert_eq!(part.frame, 0, "the gate has not opened");
        // `musplus == 4` is enough (`< 4` is the wait).
        assert_eq!(tick(&mut part, 4, DOIT_FIRST_TICK), PartStatus::Continuing);
        assert_eq!(part.frame, 1);
    }

    #[test]
    fn the_doit_guard_is_a_palimpsest() {
        let mut rig = real_rig();
        let mut part = Jplogo::new();
        part.init(&mut rig.init_ctx(0)).unwrap();
        part.y1 = 0;
        part.y2 = 400;
        part.lasty[100] = 100;
        part.lasts[100] = 184;
        // A pixel the zoom would overwrite if the row were redrawn.
        rig.surface.pixels_mut()[100 * 320 + 100] = 0x77;
        part.draw_doit_frame(rig.surface.pixels_mut());
        assert_eq!(
            rig.surface.pixels()[100 * 320 + 100],
            0x77,
            "row 100's (b,a) repeated, so it was not redrawn"
        );
        assert_eq!(part.lasty[101], 101);
        assert_eq!(part.lasts[101], 184);
        // Forgetting the row's state makes the next pass redraw it.
        part.lasty[100] = -1;
        part.draw_doit_frame(rig.surface.pixels_mut());
        assert_ne!(rig.surface.pixels()[100 * 320 + 100], 0x77);
        // A row outside [y1,y2) is cleared in the zoom window and keeps the
        // fixed margins.
        part.y1 = 0;
        part.y2 = 100;
        rig.surface.pixels_mut()[200 * 320] = 0x33;
        rig.surface.pixels_mut()[200 * 320 + 100] = 0x44;
        part.draw_doit_frame(rig.surface.pixels_mut());
        assert_eq!(rig.surface.pixels()[200 * 320], 0x33, "margin untouched");
        assert_eq!(rig.surface.pixels()[200 * 320 + 100], 0);
    }

    #[test]
    fn the_counter_freezes_after_511_and_reads_the_table_before_it() {
        let mut rig = real_rig();
        let mut part = Jplogo::new();
        part.init(&mut rig.init_ctx(0)).unwrap();
        part.gate_open = true;
        // Frame 510 takes the table's 511th entry, which is the settled
        // (0, 400).
        rig.tick = DOIT_FIRST_TICK;
        part.frame = 510;
        rig.step(&mut part);
        assert_eq!(part.frame, 511);
        assert_eq!((part.y1, part.y2), (0, 400));
        // The 512th leaves `y1`/`y2` alone: `if(frame>511) c=400;` never
        // assigns them, and `c` is not read anywhere again.
        part.y1 = 7;
        part.y2 = 123;
        rig.step(&mut part);
        assert_eq!(part.frame, 512);
        assert_eq!((part.y1, part.y2), (7, 123), "the freeze keeps the values");
    }

    #[test]
    fn the_frame_cap_and_exit_end_the_part() {
        let mut rig = real_rig();
        let mut part = Jplogo::new();
        part.init(&mut rig.init_ctx(0)).unwrap();
        part.gate_open = true;
        rig.tick = DOIT_FIRST_TICK;
        part.frame = FRAME_CAP;
        let status = rig.step(&mut part);
        assert_eq!(status, PartStatus::Finished);
        assert!(part.done);

        let mut rig = real_rig();
        let mut part = Jplogo::new();
        part.init(&mut rig.init_ctx(0)).unwrap();
        rig.tick = DOIT_FIRST_TICK;
        assert_eq!(rig.step_exit(&mut part), PartStatus::Finished);
        assert!(part.done);
    }
}
