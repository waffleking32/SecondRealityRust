//! GRID — the line rider (`GRID/P.EXE`).
//!
//! The shipped effect is `MAIN.C` (Microsoft C, 16-bit `int`s) linked with
//! `ASM.ASM`, the generated `LINEBLIT.INC` and the `EYE4.UH` blob
//! (`GRID/LINK.TMP:1-2`, `GRID/MAKEFILE:20-23`). A static 320×200 eye carries
//! one self-intersecting parametric curve of 1,000 segments; three successive
//! curves are on screen at once, each drawn in its own XOR colour through a
//! generated Bresenham, and the palette cycles one plane every frame.
//!
//! The port transliterates the source: the sine-table build with the
//! **measured** uninitialised `d` (`d0 = 0x07FE`), the eye shift that stops
//! 100 bytes short, the three palettes, the 357-entry line table with its
//! (−10,−8) fill word, and the 65,536-byte wrapping address space. `sin256`
//! and the 64 KB `muldata` are built by `MAIN.C` but read by nothing linked
//! into `P.EXE`; they are dead and not ported.
//!
//! `docs/fidelity/grid.md` is the full note: the capture, the substitutions
//! and the residuals.

use std::path::Path;

use sr_engine::part::{Part, PartCtx, PartError, PartStatus};
use sr_engine::Palette;

/// The registry name (`parts::registry`).
pub const GRID_NAME: &str = "grid";

/// `EYE4.UH` in the port's curated asset tree — byte-identical to
/// `GRID/EYE4.UH`, which `P.EXE` links as `_EYE.OBK`. The binary's working
/// directory is the repository root (the unit tests, which run from the
/// crate's directory, use `../../assets/...` themselves).
const EYE_PATH: &str = "assets/GLENZ/EYE4.UH";

/// The mode-13h screen: 320×200 bytes at `A000:0000`.
const SCREEN_WIDTH: i32 = 320;
const SCREEN_HEIGHT: i32 = 200;
const SCREEN_BYTES: usize = 64_000;

/// `vram` and `bufdata` are 64 KB segments, and the generated blit's `di` is
/// 16-bit (`ASM.ASM:3-6`, `:32-35`): an address past 64,000 is off the screen
/// but still inside the segment.
const BUFFER_BYTES: usize = 65_536;

/// `MAIN.C:141`'s `for(u=784;u<64684;u++) eye[u]<<=3` starts at the image's
/// first byte (file offset 784) and stops 100 bytes short of its 64,784-byte
/// end: the first 63,900 image bytes are shifted, the last 100 are not.
const SHIFTED_IMAGE_BYTES: usize = 64_684 - 784;

/// The starting value of the uninitialised `d` in the `sinus1` build.
///
/// `MAIN.C:33` declares `d` without a value; only `b` and `c` are zeroed at
/// `:51`. The object's own frame predicts `0x116`, but the shipped `P.EXE`'s
/// static dot screen measures `0x7FE (mod 0x2000)` exactly — every `d0` in
/// 0..65535 was evaluated against the capture's 982-pixel screen and only that
/// class matched all 982 (`docs/fidelity/grid.md`; D24's rule — the binary is
/// the artifact, not the object).
const INITIAL_D: u16 = 0x07FE;

/// `sin1[a] = (int)(sin(((a * pi) * 2) / 1024) * 256)` (`MAIN.C:44-49`),
/// truncated toward zero. The expression's left-to-right order is the
/// compiled one (`MAIN.C`'s own doubles).
#[allow(clippy::approx_constant)] // the source's own 13-digit literal, not std's PI
pub fn sin1() -> [i16; 1024] {
    std::array::from_fn(|a| {
        let f = (a as f64 * 3.141_592_653_589_f64) * 2.0 / 1024.0;
        (f.sin() * 256.0) as i16
    })
}

/// `sinus1` and `sinus2` from a chosen initial `d`, with the source's 16-bit
/// wrapping cursor adds, `>> 3 & 1023` indices and truncating division.
///
/// Published so the measured `d0` can be compared against the object's own
/// static prediction (`0x116`) in a test, not fitted.
fn sinus_tables(d0: u16) -> ([i16; 1024], [i16; 1024]) {
    sinus_tables_from(&sin1(), d0)
}

/// [`sinus_tables`] with the sine table supplied, so a scan over every initial
/// `d` builds `sin1` once.
fn sinus_tables_from(s: &[i16; 1024], d0: u16) -> ([i16; 1024], [i16; 1024]) {
    let mut b: u16 = 0;
    let mut c: u16 = 0;
    let mut d: u16 = d0;
    let sinus1: [i16; 1024] = std::array::from_fn(|_| {
        b = b.wrapping_add(97);
        c = c.wrapping_add(87);
        d = d.wrapping_add(67);
        let bi = ((b >> 3) & 1023) as usize;
        let ci = ((c >> 3) & 1023) as usize;
        let di = ((d >> 3) & 1023) as usize;
        let sum = i32::from(s[bi]) + 2 * i32::from(s[ci]) + i32::from(s[di]);
        // `mov cx,8; cwtd; idiv cx` (`MAIN.OBJ` text 0x19b-0x19f): signed
        // division truncating toward zero. Rust's `/` is the same.
        (sum / 8) as i16
    });
    let sinus2: [i16; 1024] = std::array::from_fn(|a| {
        let p = i32::from(sinus1[(a + 277) & 1023]);
        // `imul 7; idiv 10`, likewise truncating toward zero.
        ((p * 7) / 10) as i16
    });
    (sinus1, sinus2)
}

/// The x waveform: `MAIN.C:51-58`, with `d0 = 0x07FE`.
pub fn sinus1() -> [i16; 1024] {
    sinus_tables(INITIAL_D).0
}

/// The y waveform: `MAIN.C:59-62`, static because `d` is reset to 0 at the top
/// of every frame (`MAIN.C:162-163`).
pub fn sinus2() -> [i16; 1024] {
    sinus_tables(INITIAL_D).1
}

/// The eye picture and its palette, after `MAIN.C:141`'s shift.
#[derive(Clone)]
pub struct Eye {
    /// The file's 768-byte six-bit palette, at file offset 16, verbatim.
    pub palette: [u8; 768],
    /// The 64,000 image bytes with the source's shift bound applied.
    pub pixels: [u8; SCREEN_BYTES],
}

/// Load `EYE4.UH` through the port's Origin `Uh` decoder and apply the
/// source's shift exactly: image bytes `0..63,900` become `(v << 3) & 0xff`,
/// the last 100 (row 199, x=220..319) stay raw.
pub fn load_eye(path: &Path) -> Result<Eye, String> {
    let image =
        sr_assets::load_path(path).map_err(|error| format!("{}: {error}", path.display()))?;
    if image.width() != SCREEN_WIDTH as u16 || image.height() != SCREEN_HEIGHT as u16 {
        return Err(format!(
            "{}: the eye is {}×{}, not 320×200",
            path.display(),
            image.width(),
            image.height()
        ));
    }
    let mut pixels = [0u8; SCREEN_BYTES];
    pixels.copy_from_slice(image.pixels());
    for v in &mut pixels[..SHIFTED_IMAGE_BYTES] {
        *v = v.wrapping_shl(3);
    }
    let mut palette = [0u8; 768];
    for index in 0..256 {
        let rgb = image.palette.entry(index as u8);
        palette[index * 3..index * 3 + 3].copy_from_slice(&rgb);
    }
    Ok(Eye { palette, pixels })
}

/// The three palettes (`MAIN.C:95-126`), six bits per channel.
///
/// `eye_palette` is the eye's 768-byte palette as [`Eye::palette`] holds it;
/// the source's `eye[16 + (x >> 3) * 3 + channel]` is therefore group `x >> 3`'s
/// three bytes at `3 * (x >> 3)` here. Only palette entries 0..31 are read, so
/// each group of eight indices shares a base colour and its low three bits
/// choose a red rank. Palette 0 adds +40/+20/+10 for bits 0/2/1, palette 1 for
/// bits 1/0/2, palette 2 for bits 2/1/0; the sum clamps at 63. Green and blue
/// are the base, never above 15.
pub fn palettes(eye_palette: &[u8; 768]) -> [[u8; 768]; 3] {
    let mut out = [[0u8; 768]; 3];
    for (plane, palette) in out.iter_mut().enumerate() {
        for x in 0..256 {
            let at = (x >> 3) * 3;
            let base = [
                eye_palette[at] / 4,
                eye_palette[at + 1] / 4,
                eye_palette[at + 2] / 4,
            ];
            let red = match plane {
                0 => {
                    (if x & 1 != 0 { 40 } else { 0 })
                        + (if x & 4 != 0 { 20 } else { 0 })
                        + (if x & 2 != 0 { 10 } else { 0 })
                }
                1 => {
                    (if x & 2 != 0 { 40 } else { 0 })
                        + (if x & 1 != 0 { 20 } else { 0 })
                        + (if x & 4 != 0 { 10 } else { 0 })
                }
                _ => {
                    (if x & 4 != 0 { 40 } else { 0 })
                        + (if x & 2 != 0 { 20 } else { 0 })
                        + (if x & 1 != 0 { 10 } else { 0 })
                }
            };
            palette[x * 3] = (base[0] + red).min(63);
            palette[x * 3 + 1] = base[1];
            palette[x * 3 + 2] = base[2];
        }
    }
    out
}

/// The mode-13h screen pair: the visible frame in `vram`, mirrored into
/// `bufdata`. Every generated routine reads and writes both (`mov al,gs:[di];
/// xor al,dl; mov gs:[di],al; mov es:[di],al`), so they stay in lockstep.
pub struct LineBlitter {
    vram: [u8; BUFFER_BYTES],
    buf: [u8; BUFFER_BYTES],
}

impl Default for LineBlitter {
    fn default() -> Self {
        LineBlitter::new()
    }
}

impl LineBlitter {
    pub fn new() -> LineBlitter {
        LineBlitter {
            vram: [0; BUFFER_BYTES],
            buf: [0; BUFFER_BYTES],
        }
    }

    /// `MAIN.C:142`/`:156`: both buffers start as the same shifted eye.
    fn set_image(&mut self, image: &[u8; SCREEN_BYTES]) {
        self.vram[..SCREEN_BYTES].copy_from_slice(image);
        self.buf[..SCREEN_BYTES].copy_from_slice(image);
    }

    /// One generated routine, selected the way `asmloop1` selects it: by the
    /// **low bytes** of `(y1−y0, x1−x0)` (`ASM.ASM:183-192`). `sub cl,bl`
    /// keeps only the y delta's low byte, and `mov bh,cl; add bx,bx` doubles
    /// the 16-bit key: the doubling drops bit 7 of that byte, so the table's y
    /// key is the 7-bit signed `dy_low & 0x7F` — low bytes `0x78..=0x7F`
    /// select the negative-dy routines (−8..−1) and `0x80..=0x88` the positive
    /// ones (0..8). This signed-byte range test agrees outside that band and
    /// picks the fill word inside it; the static y waveform's adjacent deltas
    /// are −7..=56, so no segment reaches it
    /// (`the_static_y_waveform_never_reaches_the_dropped_bit`). The table
    /// covers dx ∈ −10..=10, dy ∈ −8..=8; every other entry of the 64 KB
    /// dispatch table is the fill word `lblt_54b56`, the line to (−10,−8),
    /// because
    /// `lineblit_init` reads the first table entry (`ASM.ASM:213-217`,
    /// `LINEBLIT.INC:2-4`). The endpoint is not plotted (`DOLINE.C:64` marks
    /// it 1 and the emitter ignores it); the start is.
    pub fn line(&mut self, x0: i16, y0: i16, x1: i16, y1: i16, colour: u8) {
        let dx = x1.wrapping_sub(x0);
        let dy = y1.wrapping_sub(y0);
        // `di = rows[Y1*2] + X1` in 16-bit arithmetic (`ASM.ASM:183-187`).
        let base = (y0 as u16)
            .wrapping_mul(SCREEN_WIDTH as u16)
            .wrapping_add(x0 as u16);
        // The source's y key drops bit 7 (`ASM.ASM:189-191`); the reachable
        // deltas never enter the band where that matters (the doc comment
        // above and `the_static_y_waveform_never_reaches_the_dropped_bit`).
        let (key_dx, key_dy) = match ((dx as u8) as i8, (dy as u8) as i8) {
            (dx, dy) if (-10..=10).contains(&dx) && (-8..=8).contains(&dy) => (dx, dy),
            _ => (-10, -8),
        };
        self.blit(base, key_dx, key_dy, colour);
    }

    /// Replays one stored routine at its stored `di` and colour (the erase
    /// pass, `ASM.ASM:155-169`). The generated code's address is
    /// `di + x + y*320` in 16-bit wrapping arithmetic.
    fn blit(&mut self, base: u16, dx: i8, dy: i8, colour: u8) {
        let (x1, y1) = (i32::from(dx), i32::from(dy));
        let (mut x, mut y) = (0i32, 0i32);
        let (xds, yds) = (if x1 < 0 { -1 } else { 1 }, if y1 < 0 { -1 } else { 1 });
        let (xd, yd) = (x1.abs(), y1.abs());
        if xd > yd {
            // `d = -xd/2` with C's truncation toward zero, then the loop tests
            // the endpoint before every plot (`DOLINE.C:36-52`).
            let mut d = -xd / 2;
            while x != x1 || y != y1 {
                self.plot(base, x, y, colour);
                x += xds;
                d += yd;
                if d > 0 {
                    d -= xd;
                    y += yds;
                }
            }
        } else {
            let mut d = -yd / 2;
            while x != x1 || y != y1 {
                self.plot(base, x, y, colour);
                y += yds;
                d += xd;
                if d > 0 {
                    d -= yd;
                    x += xds;
                }
            }
        }
    }

    fn plot(&mut self, base: u16, x: i32, y: i32, colour: u8) {
        let at = (i32::from(base) + x + y * SCREEN_WIDTH) as u16 as usize;
        self.vram[at] ^= colour;
        self.buf[at] ^= colour;
    }
}

/// One drawn segment as the slot stores it (`ASM.ASM:185-194`): the source
/// keeps the screen offset `di` and the routine word. The port keeps the
/// segment's endpoints, from which the same routine and offset follow.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Segment {
    x0: i16,
    y0: i16,
    x1: i16,
    y1: i16,
}

/// The GRID part.
pub struct Grid {
    blitter: LineBlitter,
    palettes: [Palette; 3],
    sinus1: [i16; 1024],
    sinus2: [i16; 1024],
    /// The **source loop's** iteration counter. The port's composed frame `j`
    /// is source iteration `j + 1`: iteration 0 is the start-up transition
    /// frame (reference 352), which Task 2's `K0` excludes, so `init` performs
    /// it and the first composed frame is iteration 1 = reference 353.
    iteration: u32,
    /// The three plane slots (`xyzdata`'s +8/+16/+24), each holding the line
    /// drawn three frames ago in its plane.
    slots: [Option<Vec<Segment>>; 3],
}

impl Default for Grid {
    fn default() -> Self {
        Grid::new()
    }
}

impl Grid {
    pub fn new() -> Grid {
        Grid {
            blitter: LineBlitter::new(),
            palettes: [Palette::BLACK; 3],
            sinus1: sinus1(),
            sinus2: sinus2(),
            iteration: 0,
            slots: [None, None, None],
        }
    }

    /// One source loop body (`MAIN.C:161-171`, `ASM.ASM:136-200`): advance the
    /// plane, erase the line that plane drew three iterations ago, rebuild the
    /// 1,001 points and draw/store the 1,000 segments.
    fn advance(&mut self) {
        let i = self.iteration;
        let plane = ((i + 1) % 3) as usize;
        let colour = [1u8, 2, 4][plane];
        if i >= 3 {
            if let Some(old) = self.slots[plane].take() {
                for segment in &old {
                    // The erase replays the stored `di` and routine with the
                    // current plane colour, which is the colour it was drawn
                    // in (`ASM.ASM:155-169`).
                    self.blitter
                        .line(segment.x0, segment.y0, segment.x1, segment.y1, colour);
                }
            }
        }
        let mut points = [(0i16, 0i16); 1001];
        for (k, point) in points.iter_mut().enumerate() {
            let x = self.sinus1[(i as usize + k) & 1023] + 160;
            let y = self.sinus2[k] + 100;
            *point = (x, y);
        }
        let mut segments = Vec::with_capacity(1000);
        for pair in points.windows(2) {
            let (x0, y0) = pair[0];
            let (x1, y1) = pair[1];
            self.blitter.line(x0, y0, x1, y1, colour);
            segments.push(Segment { x0, y0, x1, y1 });
        }
        self.slots[plane] = Some(segments);
        self.iteration += 1;
    }
}

impl Part for Grid {
    fn name(&self) -> &'static str {
        GRID_NAME
    }

    fn init(&mut self, ctx: &mut PartCtx) -> Result<(), PartError> {
        let eye = load_eye(Path::new(EYE_PATH)).map_err(|message| PartError {
            part: GRID_NAME,
            message,
        })?;
        self.palettes = palettes(&eye.palette).map(|raw| {
            let mut palette = Palette::new();
            for index in 0..256 {
                let at = index * 3;
                palette.set_entry(index as u8, [raw[at], raw[at + 1], raw[at + 2]]);
            }
            palette
        });
        self.blitter.set_image(&eye.pixels);
        // `mov ax,13h; int 10h` (`MAIN.C:65-68`): the linear 320×200 screen is
        // packed at the start of the engine's surface, so the beam reads it at
        // stride 320; no split, origin 0, and the blanking border the source
        // sets to 0 (`setborder(0)`, invisible in dumps).
        ctx.view.width = SCREEN_WIDTH as u16;
        ctx.view.height = SCREEN_HEIGHT as u16;
        ctx.view.double_scan = false;
        ctx.view.y_line_offset = Some(SCREEN_WIDTH as u16);
        ctx.view.set_origin(0);
        ctx.view.pan = 0;
        ctx.view.border = Some(0);
        ctx.view.split = None;
        // The DAC a mode set leaves; the source's first palette upload comes
        // only at the end of the first loop iteration.
        ctx.set_palette(crate::shell::hardware_default_palette());
        // Source iteration 0 — the start-up transition frame — is drawn before
        // the first frame the capture calls `K0`, and is never composed.
        self.iteration = 0;
        self.slots = [None, None, None];
        self.advance();
        ctx.surface.pixels_mut()[..SCREEN_BYTES]
            .copy_from_slice(&self.blitter.vram[..SCREEN_BYTES]);
        Ok(())
    }

    fn frame(&mut self, ctx: &mut PartCtx) -> PartStatus {
        // `while(!dis_exit())` (`MAIN.C:158`): the flag is checked at the top.
        if ctx.exit_requested {
            return PartStatus::Finished;
        }
        let i = self.iteration;
        // The capture displays iteration `i` under the palette the source
        // uploaded at the end of iteration `i - 1`: `pal[i % 3]`. The engine
        // stamps a palette set during `frame` onto this tick's frame, so the
        // upload is performed at the **start** of the tick it presents
        // (recorded in `docs/fidelity/grid.md`).
        ctx.set_palette(self.palettes[(i % 3) as usize]);
        self.advance();
        ctx.surface.pixels_mut()[..SCREEN_BYTES]
            .copy_from_slice(&self.blitter.vram[..SCREEN_BYTES]);
        // `dis_muscode(3) == 3` (`MAIN.C:160`) can never fire: the port's
        // `muscode` is 0 with no player (D8), and the capture delivers no
        // second key.
        PartStatus::Continuing
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    /// Where the unit tests run from: the crate's own directory.
    const ASSET: &str = "../../assets/GLENZ/EYE4.UH";

    fn eye() -> Eye {
        load_eye(Path::new(ASSET)).expect("the committed EYE4.UH loads")
    }

    /// The 1,024-point screen `MAIN.C:144-152` plots before `getch`, for a
    /// chosen initial `d`.
    fn dot_set(d0: u16) -> BTreeSet<(i16, i16)> {
        let (s1, s2) = sinus_tables(d0);
        (0..1024).map(|k| (s1[k] + 160, s2[k] + 100)).collect()
    }

    #[test]
    fn the_sine_tables_are_the_sources_own() {
        let s = sin1();
        // `((a * pi) * 2) / 1024`, sin, ×256, truncated toward zero.
        assert_eq!(s[0], 0);
        assert_eq!(s[128], 181);
        assert_eq!(s[256], 256);
        assert_eq!(s[512], 0);
        assert_eq!(s[768], -256);
        assert_eq!(s[1023], -1);
        assert_eq!(&s[..8], &[0, 1, 3, 4, 6, 7, 9, 10]);

        let s1 = sinus1();
        assert_eq!(&s1[..12], &[37, 44, 50, 57, 63, 68, 74, 79, 84, 88, 92, 96]);
        assert_eq!(s1.iter().copied().min(), Some(-123));
        assert_eq!(s1.iter().copied().max(), Some(120));

        let s2 = sinus2();
        assert_eq!(&s2[..12], &[4, 6, 7, 8, 9, 10, 11, 11, 12, 13, 14, 14]);
        assert_eq!(s2.iter().copied().min(), Some(-86));
        assert_eq!(s2.iter().copied().max(), Some(84));

        // The cyclic seam: `sinus1[1023] = -45`, `sinus1[0] = 37`, an 82 jump.
        assert_eq!(s1[1023], -45);
        assert_eq!(s1[0], 37);
        assert_eq!(i32::from(s1[0]) - i32::from(s1[1023]), 82);
    }

    #[test]
    fn the_initial_d_is_the_measured_0x7fe() {
        let measured = dot_set(INITIAL_D);
        assert_eq!(measured.len(), 982, "unique pixels of the 1,024-point plot");
        let xs: Vec<i16> = measured.iter().map(|p| p.0).collect();
        let ys: Vec<i16> = measured.iter().map(|p| p.1).collect();
        assert_eq!(
            (
                *xs.iter().min().unwrap(),
                *xs.iter().max().unwrap(),
                *ys.iter().min().unwrap(),
                *ys.iter().max().unwrap()
            ),
            (37, 280, 14, 184)
        );
        // `d + 0x2000` adds 1,024 to `d >> 3`, which `& 1023` discards.
        assert_eq!(dot_set(0x27FE), measured);
        assert_eq!(dot_set(0x47FE), measured);
        // The object's own static prediction (`__aFftol`'s return IP 0x116)
        // is not what the shipped P.EXE does: 1,004 pixels, 47 of them the
        // measured screen's.
        let object = dot_set(0x0116);
        assert_eq!(object.len(), 1004);
        assert_eq!(measured.intersection(&object).count(), 47);
    }

    #[test]
    fn the_measured_d_is_the_only_residue_whose_tables_match() {
        // The whole 16-bit word is covered by the 0x2000 residues: adding
        // 0x2000 adds 1,024 to `d >> 3`, which the `& 1023` index discards.
        let s = sin1();
        let measured = sinus_tables_from(&s, INITIAL_D);
        let matching: Vec<u16> = (0..0x2000u16)
            .filter(|&d0| sinus_tables_from(&s, d0) == measured)
            .collect();
        assert_eq!(matching, vec![0x07FE]);
        for step in 0..8u16 {
            assert_eq!(
                sinus_tables_from(&s, INITIAL_D.wrapping_add(step * 0x2000)),
                measured
            );
        }
    }

    #[test]
    fn the_sinus_tables_use_the_sources_wrapping_arithmetic() {
        // `MAIN.C:51-58`'s cursors are 16-bit `int`s: 97*1024, 87*1024 and
        // 67*1024 all pass 0xFFFF, so the wrap is exercised by the table.
        let steps = [97u32, 87, 67];
        assert!(
            steps.iter().all(|step| step * 1024 > 0xFFFF),
            "a cursor must wrap for the test to exercise the 16-bit arithmetic"
        );
        let s = sin1();
        let index = |step: u32| (((step * 1024) & 0xFFFF) >> 3 & 1023) as usize;
        let d_index = (((0x07FEu32 + 67 * 1024) & 0xFFFF) >> 3 & 1023) as usize;
        let sum = i32::from(s[index(97)]) + 2 * i32::from(s[index(87)]) + i32::from(s[d_index]);
        // Recompute the last entry from the wrapped cursors: the table's own
        // value and this independent evaluation agree.
        assert_eq!(sinus1()[1023], (sum / 8) as i16);

        // Signed division truncates toward zero, where an arithmetic shift
        // would round down: a = 42's sum is -27, so -27/8 = -3, not -4.
        assert_eq!(sinus1()[42], -3);
        // And `sinus2[a] = sinus1[(a+277)&1023] * 7 / 10`: a = 34's product is
        // -7, so -7/10 = 0, not -1.
        assert_eq!(sinus2()[34], 0);
    }

    #[test]
    fn the_eye_shift_stops_100_bytes_short() {
        let e = eye();
        let raw = std::fs::read(ASSET).expect("EYE4.UH");
        assert_eq!(raw.len(), 64_784);
        for (u, &pixel) in e.pixels.iter().enumerate() {
            let source = raw[784 + u];
            let expected = if u < SHIFTED_IMAGE_BYTES {
                source << 3
            } else {
                source
            };
            assert_eq!(pixel, expected, "image byte {u}");
        }
        // The boundary is exactly the source's: 63,900 shifted, the last 100
        // raw (row 199, x = 220..319).
        assert_eq!(SHIFTED_IMAGE_BYTES, 63_900);
        assert_eq!(e.pixels[63_899], raw[784 + 63_899] << 3);
        assert_eq!(e.pixels[63_900], raw[784 + 63_900]);
        assert_eq!(e.pixels[63_999], raw[784 + 63_999]);
        // The palette is the file's 768-byte six-bit table at offset 16,
        // verbatim — the shift loop runs past it only from 784.
        for (i, &byte) in e.palette.iter().enumerate() {
            assert_eq!(byte, raw[16 + i], "palette byte {i}");
        }
        // A missing file is an error, not a panic.
        assert!(load_eye(Path::new("../../assets/GLENZ/no-such-file.UH")).is_err());
    }

    #[test]
    fn the_buffers_start_as_the_same_shifted_eye() {
        let e = eye();
        let mut blitter = LineBlitter::new();
        blitter.set_image(&e.pixels);
        assert_eq!(&blitter.vram[..SCREEN_BYTES], &e.pixels[..]);
        assert_eq!(&blitter.buf[..SCREEN_BYTES], &e.pixels[..]);
        assert_eq!(
            &blitter.vram[SCREEN_BYTES..],
            &[0u8; BUFFER_BYTES - SCREEN_BYTES][..]
        );
        assert_eq!(blitter.vram, blitter.buf);

        // Every draw and erase keeps them in lockstep
        // (`mov gs:[di],al; mov es:[di],al`).
        for (dx, dy) in [(-10, -8), (0, 0), (7, 3), (82, 56), (-4, -8)] {
            blitter.line(160, 100, 160 + dx, 100 + dy, 0x2A);
            assert_eq!(blitter.vram, blitter.buf);
        }
    }

    #[test]
    fn the_three_palettes_are_the_sources_own() {
        let e = eye();
        let p = palettes(&e.palette);
        for (plane, palette) in p.iter().enumerate() {
            for group in 0..32u16 {
                let x = (group * 8) as usize;
                let at = group as usize * 3;
                let base = [
                    e.palette[at] / 4,
                    e.palette[at + 1] / 4,
                    e.palette[at + 2] / 4,
                ];
                // `x & 7 == 0` is the same entry in all three palettes.
                assert_eq!(
                    &palette[x * 3..x * 3 + 3],
                    &base,
                    "plane {plane} group {group}"
                );
            }
        }
        for palette in &p {
            for x in 0..256usize {
                let at = (x >> 3) * 3;
                let base_g = e.palette[at + 1] / 4;
                let base_b = e.palette[at + 2] / 4;
                assert_eq!(palette[x * 3 + 1], base_g, "green at {x}");
                assert_eq!(palette[x * 3 + 2], base_b, "blue at {x}");
                assert!(palette[x * 3 + 1] <= 15 && palette[x * 3 + 2] <= 15);
                assert!(palette[x * 3] <= 63);
            }
        }
        // The ranks: bit0 / bit2 / bit1 carry +40 / +20 / +10 in palette 0,
        // bit1 / bit0 / bit2 in palette 1, bit2 / bit1 / bit0 in palette 2.
        for group in 0..32usize {
            // The same low bits rank differently per palette.
            assert_eq!(
                p[0][(group * 8 + 1) * 3],
                (e.palette[group * 3] / 4 + 40).min(63)
            );
            assert_eq!(
                p[1][(group * 8 + 1) * 3],
                (e.palette[group * 3] / 4 + 20).min(63)
            );
            assert_eq!(
                p[2][(group * 8 + 1) * 3],
                (e.palette[group * 3] / 4 + 10).min(63)
            );
            assert_eq!(
                p[0][(group * 8 + 2) * 3],
                (e.palette[group * 3] / 4 + 10).min(63)
            );
            assert_eq!(
                p[1][(group * 8 + 2) * 3],
                (e.palette[group * 3] / 4 + 40).min(63)
            );
            assert_eq!(
                p[2][(group * 8 + 2) * 3],
                (e.palette[group * 3] / 4 + 20).min(63)
            );
            assert_eq!(
                p[0][(group * 8 + 4) * 3],
                (e.palette[group * 3] / 4 + 20).min(63)
            );
            assert_eq!(
                p[1][(group * 8 + 4) * 3],
                (e.palette[group * 3] / 4 + 10).min(63)
            );
            assert_eq!(
                p[2][(group * 8 + 4) * 3],
                (e.palette[group * 3] / 4 + 40).min(63)
            );
            // All three bits: +70 clamps at 63 in every palette.
            for palette in &p {
                assert_eq!(palette[(group * 8 + 7) * 3], 63);
            }
        }
        // The entries the two buffers' shifted image actually addresses are
        // multiples of 8 (groups 16..31), so the background is identical in
        // all three palettes; the unshifted last 100 bytes use group 3.
        assert_eq!(p[0][128 * 3..128 * 3 + 3], p[1][128 * 3..128 * 3 + 3]);
        assert_eq!(p[0][128 * 3..128 * 3 + 3], p[2][128 * 3..128 * 3 + 3]);
        assert_eq!(
            &p[1][24 * 3..24 * 3 + 3],
            &[
                e.palette[3 * 3] / 4,
                e.palette[3 * 3 + 1] / 4,
                e.palette[3 * 3 + 2] / 4
            ]
        );
        // x = 24 is `x & 7 == 0`, so it is the group base in all three.
        assert_eq!(p[0][24 * 3..24 * 3 + 3], p[1][24 * 3..24 * 3 + 3]);
        assert_eq!(p[1][24 * 3..24 * 3 + 3], p[2][24 * 3..24 * 3 + 3]);
    }

    // ------------------------------------------------------- line blitter --

    /// `DOLINE.C:13-59`'s `line()`, translated directly: the pixels plotted
    /// from the start to **before** the endpoint, in the generator's own
    /// arithmetic (`d = -xd/2` with C truncation, the `xd > yd` branch).
    fn doline(dx: i32, dy: i32) -> Vec<(i32, i32)> {
        let xds = if dx < 0 { -1 } else { 1 };
        let yds = if dy < 0 { -1 } else { 1 };
        let (xd, yd) = (dx.abs(), dy.abs());
        let (mut x, mut y) = (0, 0);
        let mut out = Vec::new();
        if xd > yd {
            let mut d = -xd / 2;
            while x != dx || y != dy {
                out.push((x, y));
                x += xds;
                d += yd;
                if d > 0 {
                    d -= xd;
                    y += yds;
                }
            }
        } else {
            let mut d = -yd / 2;
            while x != dx || y != dy {
                out.push((x, y));
                y += yds;
                d += xd;
                if d > 0 {
                    d -= yd;
                    x += xds;
                }
            }
        }
        out
    }

    fn base_of(x0: i16, y0: i16) -> u16 {
        (y0 as u16).wrapping_mul(320).wrapping_add(x0 as u16)
    }

    /// The direct DOLINE pixels XORed into a zero buffer at `base`.
    fn direct(base: u16, dx: i32, dy: i32, colour: u8) -> [u8; BUFFER_BYTES] {
        let mut v = [0u8; BUFFER_BYTES];
        for (mx, my) in doline(dx, dy) {
            let at = ((base as i32) + mx + my * 320) as u16 as usize;
            v[at] ^= colour;
        }
        v
    }

    #[test]
    fn the_line_blit_is_the_generated_xor_bresenham() {
        // The whole filled table, dx −10..=10 × dy −8..=8, at a start that is
        // never near an edge.
        let (x0, y0) = (160i16, 100i16);
        for dy in -8..=8 {
            for dx in -10..=10 {
                let mut blit = LineBlitter::new();
                blit.line(x0, y0, x0 + dx, y0 + dy, 0x5A);
                let expected = direct(base_of(x0, y0), i32::from(dx), i32::from(dy), 0x5A);
                assert_eq!(blit.vram, expected, "dx {dx} dy {dy}");
                assert_eq!(blit.buf, expected, "dx {dx} dy {dy}");
            }
        }

        // The endpoint is excluded and the start included; the next segment's
        // start is this endpoint, so a polyline has no double-drawn joint.
        assert!(doline(4, 3).contains(&(0, 0)));
        assert!(!doline(4, 3).contains(&(4, 3)));
        let mut blit = LineBlitter::new();
        blit.line(10, 10, 14, 13, 1);
        assert_eq!(blit.vram[base_of(10, 10) as usize], 1);
        assert_eq!(blit.vram[base_of(14, 13) as usize], 0);
        // The dot entry plots one pixel and the (0,0) entry plots none.
        assert_eq!(doline(1, 0).len(), 1);
        assert!(doline(0, 0).is_empty());
        // The fill word is the (−10,−8) line, not the dot and not nothing.
        assert_eq!(doline(-10, -8).len(), 10);

        // Negative directions, edges, a bottom-row crossing and 16-bit
        // address wrap.
        for &(sx, sy) in &[(0i16, 0i16), (319, 199), (2, 0), (300, 190), (0, 197)] {
            for &(dx, dy) in &[
                (-10, -8),
                (10, 8),
                (-7, 3),
                (9, -6),
                (0, 8),
                (10, 0),
                (-1, -1),
                (4, 3),
            ] {
                let mut blit = LineBlitter::new();
                blit.line(sx, sy, sx + dx, sy + dy, 0x33);
                let expected = direct(base_of(sx, sy), i32::from(dx), i32::from(dy), 0x33);
                assert_eq!(blit.vram, expected, "start {sx},{sy} delta {dx},{dy}");
                assert_eq!(blit.buf, expected, "start {sx},{sy} delta {dx},{dy}");
            }
        }

        // Selection is by the **low byte** of the deltas, not a signed range:
        // a 266-pixel x delta selects the (10,0) routine...
        let mut blit = LineBlitter::new();
        blit.line(0, 0, 266, 0, 1);
        assert_eq!(blit.vram, direct(0, 10, 0, 1));
        // ...and an out-of-domain delta uses the fill word, not the dot.
        let mut blit = LineBlitter::new();
        blit.line(0, 0, 82, 0, 1);
        assert_eq!(blit.vram, direct(0, -10, -8, 1));
        assert_ne!(blit.vram, direct(0, 1, 0, 1));

        // The measured out-of-domain segments. i = 746's y delta is 56 in
        // every iteration (the y waveform is static), so every frame looks up
        // an unfilled word.
        let s1 = sinus1();
        let s2 = sinus2();
        assert_eq!(s2[747] - s2[746], 56);
        for i in 0..1024usize {
            let xa = s1[(i + 746) & 1023] + 160;
            let xb = s1[(i + 747) & 1023] + 160;
            let ya = s2[746] + 100;
            let yb = s2[747] + 100;
            assert_eq!(yb - ya, 56);
            let mut blit = LineBlitter::new();
            blit.line(xa, ya, xb, yb, 2);
            assert_eq!(blit.vram, direct(base_of(xa, ya), -10, -8, 2), "i = {i}");
        }
        // The cyclic seam: from source iteration 24 the line contains segment
        // k = 1023 − i, whose dx is sinus1[0] − sinus1[1023] = 82.
        assert_eq!(s1[0] - s1[1023], 82);
        for i in 24..1024usize {
            let k = 1023 - i;
            let xa = s1[(i + k) & 1023] + 160;
            let xb = s1[(i + k + 1) & 1023] + 160;
            let ya = s2[k] + 100;
            let yb = s2[k + 1] + 100;
            assert_eq!(xb - xa, 82, "seam at i = {i}");
            let mut blit = LineBlitter::new();
            blit.line(xa, ya, xb, yb, 4);
            assert_eq!(
                blit.vram,
                direct(base_of(xa, ya), -10, -8, 4),
                "seam i = {i}"
            );
        }

        // Two different colours crossing stay lit: the visible mask is the
        // union of the self-XOR lines (1 ^ 2 = 3), not an XOR of masks. The
        // deltas stay inside the table (|dy| ≤ 8); they cross at (15, 14).
        let mut blit = LineBlitter::new();
        blit.line(10, 10, 20, 18, 1);
        blit.line(10, 18, 20, 10, 2);
        assert_eq!(blit.vram[15 + 14 * 320], 3);
        assert_eq!(blit.buf, blit.vram);
        // The same colour twice cancels, which is a curve's self-intersection.
        let mut blit = LineBlitter::new();
        blit.line(10, 10, 20, 18, 1);
        blit.line(10, 18, 20, 10, 1);
        assert_eq!(blit.vram[15 + 14 * 320], 0);
    }

    /// The dispatch's dropped bit. `GRID/ASM.ASM:183-191` keeps the y delta's
    /// **low byte** and then doubles the 16-bit key (`mov bh,cl; add bx,bx`),
    /// discarding bit 7 of that byte: the table's y key is the 7-bit signed
    /// `dy_low & 0x7F`, so low bytes `0x78..=0x7F` select the negative-dy
    /// routines (−8..−1) and `0x80..=0x88` the positive ones (0..8), where the
    /// port's signed-byte range test would take the fill word. The static y
    /// waveform never reaches that band: its 1,000 adjacent deltas are
    /// −7..=56, measured here rather than assumed.
    #[test]
    fn the_static_y_waveform_never_reaches_the_dropped_bit() {
        let s2 = sinus2();
        let mut min = i16::MAX;
        let mut max = i16::MIN;
        for k in 0..1000usize {
            let dy = s2[k + 1] - s2[k];
            min = min.min(dy);
            max = max.max(dy);
            let low = dy as u8;
            assert!(
                !(0x78..=0x88).contains(&low),
                "segment {k}: dy {dy} has low byte {low:#04x}"
            );
        }
        assert_eq!((min, max), (-7, 56));
    }

    /// The generated routines `LINEBLIT.INC` writes, parsed from the committed
    /// file: each routine's label and the 16-bit `di` offsets its stores
    /// touch. The word form (`mov ax,gs:[di+N]`) sets two adjacent bytes.
    fn committed_routines() -> Vec<((i8, i8), BTreeSet<u16>)> {
        let text = std::fs::read_to_string("../../upstream/SecondReality/GRID/LINEBLIT.INC")
            .expect("the committed GRID/LINEBLIT.INC");
        let mut routines: Vec<((i8, i8), BTreeSet<u16>)> = Vec::new();
        let mut current: Option<((i8, i8), BTreeSet<u16>)> = None;
        for line in text.lines() {
            let line = line.trim_end_matches('\r');
            if let Some(label) = line.strip_suffix(':') {
                let (x, y) = label
                    .strip_prefix("lblt_")
                    .and_then(|rest| rest.split_once('b'))
                    .map(|(x, y)| {
                        (
                            x.parse::<i8>().expect("the label's x"),
                            y.parse::<i8>().expect("the label's y"),
                        )
                    })
                    .unwrap_or_else(|| panic!("unexpected LINEBLIT label {label}"));
                if let Some(done) = current.take() {
                    routines.push(done);
                }
                current = Some(((x, y), BTreeSet::new()));
                continue;
            }
            let Some((_, offsets)) = current.as_mut() else {
                continue;
            };
            let (rest, width) = if let Some(rest) = line.strip_prefix("mov al,gs:[di+") {
                (rest, 1u16)
            } else if let Some(rest) = line.strip_prefix("mov ax,gs:[di+") {
                (rest, 2u16)
            } else {
                continue;
            };
            let offset = rest
                .trim_end_matches(']')
                .parse::<i32>()
                .expect("the store's offset") as u16;
            for delta in 0..width {
                offsets.insert(offset.wrapping_add(delta));
            }
        }
        if let Some(done) = current.take() {
            routines.push(done);
        }
        routines
    }

    #[test]
    fn the_line_blit_is_the_committed_generated_code() {
        // The generator's loops (`DOLINE.C:64-85`, `:102-121`) emit 21×17
        // routines in y = −8..=8, x = −10..=10 order, labelled by the
        // endpoint's low six bits. This reads the committed include, not a
        // translation of the generator: each routine's stores must be exactly
        // the pixels the port's blit XORs for that delta.
        let routines = committed_routines();
        assert_eq!(routines.len(), 21 * 17, "the filled table's routines");
        assert_eq!(routines[0].0, (54, 56), "the (−10,−8) fill word");
        for (index, ((label_x, label_y), offsets)) in routines.iter().enumerate() {
            let index = index as i32;
            let dy = index / 21 - 8;
            let dx = index % 21 - 10;
            assert_eq!(
                (i32::from(*label_x), i32::from(*label_y)),
                (dx & 63, dy & 63),
                "routine {index}'s label"
            );
            let mut blit = LineBlitter::new();
            blit.line(0, 0, dx as i16, dy as i16, 1);
            let expected: BTreeSet<u16> = blit
                .vram
                .iter()
                .enumerate()
                .filter(|(_, &pixel)| pixel != 0)
                .map(|(at, _)| at as u16)
                .collect();
            assert_eq!(offsets, &expected, "dx {dx} dy {dy}");
        }
        // The table's dot entry is the (1,0) routine.
        assert!(routines.iter().any(|((x, y), _)| (*x, *y) == (1, 0)));
    }
}
