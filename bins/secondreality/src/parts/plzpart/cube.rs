//! The cube: `vect()` (`VECT.C:110-130`) over `PLZFILL.C`'s scene.
//!
//! `tw_opengraph` (`TWEAK.ASM:18-37`) sets chain-4 off and a 160-byte plane
//! row, and CRTC 0x09 = 2 shows each row on three of the 400 scanlines, so the
//! screen is 134 rows (`PLZA.ASM:77` clips there). After the `musplus() >= 13`
//! gate each frame is `swappage`, the frame wait, `calculate(1)`, `draw(15)`,
//! `clear()`; the six-page flip (`VECT.C:265-298`) cycles three 0x5500-byte
//! buffers and shows the one drawn a frame earlier.
//!
//! **The geometry is the release's code** (image `0x12c7d-0x1323d`): `calculate`
//! masks its angles to 13 bits for the release's 8,192-entry table,
//! `count_const` builds a 1.15 matrix and `rotate` sums in 32 bits, `>> 11`, in
//! 1/16 units (`bugs.md` §17.4). `sort_faces`, `draw` and the loop are the tree's.
//!
//! **The beam is composed here.** The VGA's planes are 262,144 bytes of linear
//! memory and the display address wraps there; the engine's 640×400 surface is
//! 6,144 short, and the bottom rows of three pages (start 0xAA28) cross the
//! wrap. So the part keeps the four-plane image in `video` and writes the
//! visible 320×200 window into the surface itself.

use sr_engine::part::{PartCtx, PartStatus};

use super::copper::PalSource;
use super::raster::CLR_SLOTS;
use super::release::{ANGLE_MASK, KOSINIT_WORDS};
use super::{Phase, Plzpart};
use crate::parts::mode_view;

/// `TWEAK.ASM:18-37`'s 320×200 mode (`0x13 = 0x50`, a 160-byte plane row).
const CUBE_WIDTH: u16 = 320;
const CUBE_HEIGHT: u16 = 200;
/// The four-plane linear image: 4 bytes per plane byte, 65,536 addresses.
pub(super) const VIDEO_BYTES: usize = 4 * 65_536;
/// vga_draw.cpp's `linear_mask = vmemwrap - 1`.
const VIDEO_MASK: u32 = (VIDEO_BYTES - 1) as u32;
/// `VECT.C:270-296`: the six display starts, in page order.
const PAGE_START: [u16; 6] = [0xAA00 + 40, 40, 0x5500 + 40, 0xAA00 + 40, 40, 0x5500 + 40];
/// `VECT.C:270-296`: the six attribute-pan values, in page order.
const PAGE_PAN: [u8; 6] = [4, 0, 4, 0, 4, 0];
/// `VECT.OBJ`: `imul eax,eax,0x8e; add ax,0x42` — the vertical projection
/// `yyy = yy*142/zz + 66`, centred on row ~67 of the 134.
const PROJECTION_Y: (i32, i32) = (142, 66);
/// `VECT.C:119-120`: break when `-4 <= musplus < 0`.
const CUBE_EXIT_LOW: i32 = -4;

/// The linear address of output pixel (x, y). `TWEAK.ASM:31-33`: CRTC 0x09 =
/// 2 shows each image row on three scanlines of the 400-line display. The
/// 200-line frame is every other scanline (the reference's geometry), so line
/// y is scanline 2y, which is row floor(2y/3).
fn cube_linear(base: u32, y: u32, x: u32) -> u32 {
    (base + ((2 * y) / 3) * 640 + x) & VIDEO_MASK
}

/// `VECT.C:74-92`: the object's points and faces.
pub(super) const CUBE_POINTS: [[i32; 3]; 8] = [
    [125, 125, 125],
    [125, -125, 125],
    [-125, -125, 125],
    [-125, 125, 125],
    [125, 125, -125],
    [125, -125, -125],
    [-125, -125, -125],
    [-125, 125, -125],
];
pub(super) const CUBE_FACES: [[i32; 8]; 6] = [
    [1, 2, 3, 0, 0, 0, 0, 0],
    [7, 6, 5, 4, 0, 0, 0, 0],
    [0, 4, 5, 1, 0, 0, 0, 1],
    [1, 5, 6, 2, 0, 0, 0, 2],
    [2, 6, 7, 3, 0, 0, 0, 1],
    [3, 7, 4, 0, 0, 0, 0, 2],
];

/// `PLZA.ASM:262-287`: `(source * shade) >> 6`, 192 bytes from `offset`.
fn shadepal(fpal: &mut [u8], pal: &[u8], offset: usize, shade: i32) {
    let shade = shade as u32 as u8;
    let out = &mut fpal[offset..offset + 192];
    let source = &pal[offset..offset + 192];
    for (out, source) in out.iter_mut().zip(source) {
        *out = (((*source as u32) * (shade as u32)) >> 6) as u8;
    }
}

impl Plzpart {
    /// `PLZFILL.C:25-68` `initvect`.
    pub(super) fn initvect(&mut self) {
        self.video = vec![0; VIDEO_BYTES];
        let mut sini = vec![0i16; 2000];
        for (index, value) in sini.iter_mut().enumerate().take(1524) {
            let angle = index as f64 / 1024.0 * std::f64::consts::PI * 4.0;
            *value = (angle.sin() * 127.0) as i16;
        }
        self.sini = sini;
        let sini = &self.sini;
        // `PLZFILL.C:37-55`: the three colour bands.
        let mut pal = [0u8; 768];
        for a in 1..32usize {
            pal[a * 3] = 0;
            pal[a * 3 + 1] = 0;
            pal[a * 3 + 2] = (a * 2) as u8;
        }
        for a in 0..32usize {
            pal[a * 3 + 32 * 3] = (a * 2) as u8;
            pal[a * 3 + 1 + 32 * 3] = (a * 2) as u8;
            pal[a * 3 + 2 + 32 * 3] = 63;
        }
        for a in 0..32usize {
            pal[192 + a * 3] = (a * 2) as u8;
            pal[192 + a * 3 + 1] = 0;
            pal[192 + a * 3 + 2] = 0;
        }
        for a in 0..32usize {
            pal[192 + a * 3 + 32 * 3] = 63;
            pal[192 + a * 3 + 1 + 32 * 3] = (a * 2) as u8;
            pal[192 + a * 3 + 2 + 32 * 3] = 0;
        }
        for a in 0..32usize {
            pal[384 + a * 3] = a as u8;
            pal[384 + a * 3 + 1] = 0;
            pal[384 + a * 3 + 2] = (a * 2 / 3) as u8;
        }
        for a in 0..32usize {
            pal[384 + a * 3 + 32 * 3] = (31 - a) as u8;
            pal[384 + a * 3 + 1 + 32 * 3] = (a * 2) as u8;
            pal[384 + a * 3 + 2 + 32 * 3] = 21;
        }
        self.pal = pal;
        // `PLZFILL.C:57-62`: the three textures, 64 rows of a 256-byte stride
        // (the linked segment is 16,384 bytes, `INCLUDE.ASM:5-9`).
        let mut kuva1 = vec![0u8; 16_384];
        let mut kuva2 = vec![0u8; 16_384];
        let mut kuva3 = vec![0u8; 16_384];
        for y in 0..64usize {
            for x in 0..256usize {
                let index = ((y as i32 * 4 + sini[x * 2] as i32) & 511) as usize;
                let value = sini[index] / 4 + 32;
                kuva1[y * 256 + x] = value as u8;
                kuva2[y * 256 + x] = (value + 64) as u8;
                kuva3[y * 256 + x] = (value + 128) as u8;
            }
        }
        self.kuva1 = kuva1;
        self.kuva2 = kuva2;
        self.kuva3 = kuva3;
        // `PLZFILL.C:64-65`: `dist1[y][x] = sini[y*8]/3`, a 256-byte row.
        let mut dist1 = vec![0u8; 128 * 256];
        for y in 0..128usize {
            let value = (sini[y * 8] as i32 / 3) as i8;
            for x in 0..256usize {
                dist1[y * 256 + x] = value as u8;
            }
        }
        self.dist1 = dist1;
        // `PLZFILL.C:67`: every edge-list slot starts empty.
        self.clrtau = vec![[640, 0]; CLR_SLOTS * 256];
        self.clrptr = 0;
    }

    /// `VECT.C:114-116` `tw_opengraph`, then the wait's display state.
    pub(super) fn set_cube_mode(&mut self, ctx: &mut PartCtx) {
        // `TWEAK.ASM:18-37`: chain-4 off and the 64 KB clear, all planes.
        self.video.fill(0);
        *ctx.view = mode_view(CUBE_WIDTH, CUBE_HEIGHT, Some(640), Some(0));
        self.page = 0;
        self.frames = 0;
        self.clrptr = 0;
        self.clrtau.iter_mut().for_each(|entry| *entry = [640, 0]);
    }

    /// `VECT.C:117-128`: one pass of the cube's loop. The caller enters only
    /// after the gate's `swappage`, which is the source loop's first; the
    /// iteration's exit check and next swap are at the end.
    pub(super) fn cube_tick(&mut self, ctx: &mut PartCtx) -> PartStatus {
        let count = self.copper.borrow().frame_count;
        if count < 1 {
            // `while(frame_count<1);` — one tick in the port.
            self.compose_cube(ctx);
            return PartStatus::Continuing;
        }
        {
            let mut state = self.copper.borrow_mut();
            state.frame_count = 0;
            state.pal_source = PalSource::Cube;
            state.do_pal = true;
        }
        self.frames += count as i32;
        self.calculate();
        self.draw_cube();
        self.clear();
        self.compose_cube(ctx);
        // The loop repeats within the same frame boundary: check the music,
        // then swap for the next iteration and wait for the next vblank.
        let reading = ctx.musplus();
        if (CUBE_EXIT_LOW..0).contains(&reading) {
            self.phase = Phase::Done;
            return PartStatus::Finished;
        }
        self.swap_page();
        self.phase = Phase::Cube;
        PartStatus::Continuing
    }

    /// `VECT.C:265-298` `swappage`.
    pub(super) fn swap_page(&mut self) {
        self.page = (self.page + 1) % 6;
        let mut state = self.copper.borrow_mut();
        state.cop_start = u32::from(PAGE_START[self.page]);
        state.cop_scrl = PAGE_PAN[self.page];
    }

    /// The beam's 320×200 window over the four-plane image.
    fn compose_cube(&self, ctx: &mut PartCtx) {
        let (start, pan) = {
            let state = self.copper.borrow();
            (state.display_start, state.display_pan)
        };
        let base = 4 * start + u32::from(pan);
        let surface = ctx.surface.pixels_mut();
        for y in 0..CUBE_HEIGHT as usize {
            let row = &mut surface[y * 640..y * 640 + CUBE_WIDTH as usize];
            for (x, out) in row.iter_mut().enumerate() {
                let linear = cube_linear(base, y as u32, x as u32);
                *out = self.video[linear as usize];
            }
        }
    }

    /// `VECT.C:132-151` `calculate`, as the release builds it (image `0x12c7d`): `getspl`
    /// over the release's `buu`, then the five angles masked to 13 bits (`and ax,1fffh`) and
    /// the light read through the release's 8,192-entry table.
    fn calculate(&mut self) {
        let tables = self.tables.as_ref().expect("init loaded the tables");
        let position = ((4 * 256 + self.frames * 4) & 0xFFFF) as usize;
        let segment = position >> 8;
        let fraction = position & 255;
        let mut out = [0i32; 8];
        for (word, value) in out.iter_mut().enumerate() {
            let control = |offset: usize| self.buu[segment * 8 + offset] as i32;
            let coefficient = |offset: usize| tables.spline[fraction + offset] as i32;
            let sum = control(24 + word)
                .wrapping_mul(coefficient(0))
                .wrapping_add(control(16 + word).wrapping_mul(coefficient(256)))
                .wrapping_add(control(8 + word).wrapping_mul(coefficient(512)))
                .wrapping_add(control(word).wrapping_mul(coefficient(768)));
            // `shld cx,bx,1; push cx`: bits 15..30 of the 32-bit sum, a word.
            *value = ((sum >> 15) as i16) as i32;
        }
        // `SPLINE.ASM:52-75` (image `0x14ae0-0x14b04`): the pops are ls_ky, ls_kx, kz, ky,
        // kx, dis, ty, tx.
        self.tx = out[0];
        self.ty = out[1];
        self.dis = out[2];
        self.kx = out[3] & ANGLE_MASK;
        self.ky = out[4] & ANGLE_MASK;
        self.kz = out[5] & ANGLE_MASK;
        self.ls_kx = out[6] & ANGLE_MASK;
        self.ls_ky = out[7] & ANGLE_MASK;
        // `VECT.C:144-146` (image `0x12cbd-0x12d2c`):
        // `ls_y = kosinit[ls_kx]>>8`, `ls_x = (sinit[ls_kx]>>8)*(sinit[ls_ky]>>8)>>7`,
        // `ls_z = (sinit[ls_kx]>>8)*(kosinit[ls_ky]>>8)>>7`, each product a 16-bit `imul`.
        let s_kx = self.sine(self.ls_kx) >> 8;
        let c_kx = self.cosine(self.ls_kx) >> 8;
        let s_ky = self.sine(self.ls_ky) >> 8;
        let c_ky = self.cosine(self.ls_ky) >> 8;
        self.ls_y = c_kx;
        self.ls_x = ((s_kx.wrapping_mul(s_ky) as i16) as i32) >> 7;
        self.ls_z = ((s_kx.wrapping_mul(c_ky) as i16) as i32) >> 7;
        self.count_const();
        self.rotate();
        self.sort_faces();
    }

    /// `sinit[k]`, a 13-bit angle into the release's table.
    fn sine(&self, index: i32) -> i32 {
        self.sinit[index as usize] as i32
    }

    /// `kosinit[k]`: the word [`KOSINIT_WORDS`] (2,048, a quarter turn) past `sinit[k]`
    /// (`es:[bx+1000h]`).
    fn cosine(&self, index: i32) -> i32 {
        self.sinit[index as usize + KOSINIT_WORDS] as i32
    }

    /// `count_const` as the release builds it (image `0x12d40-0x13079`): the tree's
    /// expressions (`VECT.C:166-176`) with every `>>15+7` a `>>15` and `cxz = -SY`, not
    /// `-SY>>7`. Each product is a 32-bit `LXMUL@` (`call far 0000:0e31h`) and each shift a
    /// signed `LXRSH@` (`0000:0f1ah`, `mov cl,0fh`); the subtractions and additions are inside
    /// the final shift, as in the tree, and each element is stored as a word. The matrix is
    /// therefore in 1.15 (32,767 at unity), where the tree's is in 8.8.
    fn count_const(&mut self) {
        let sx = self.sine(self.kx);
        let cx = self.cosine(self.kx);
        let sy = self.sine(self.ky);
        let cy = self.cosine(self.ky);
        let sz = self.sine(self.kz);
        let cz = self.cosine(self.kz);
        let word = |value: i32| -> i32 { (value as i16) as i32 };
        self.cxx = word(cy.wrapping_mul(cz) >> 15);
        self.cxy = word(cy.wrapping_mul(sz) >> 15);
        self.cxz = word(sy.wrapping_neg());
        self.cyx = word(
            ((sx.wrapping_mul(cz).wrapping_add(0x4000)) >> 15)
                .wrapping_mul(sy)
                .wrapping_sub(cx.wrapping_mul(sz))
                >> 15,
        );
        self.cyy = word(
            ((sx.wrapping_mul(sy).wrapping_add(0x4000)) >> 15)
                .wrapping_mul(sz)
                .wrapping_add(cx.wrapping_mul(cz))
                >> 15,
        );
        self.cyz = word(cy.wrapping_mul(sx) >> 15);
        self.czx = word(
            ((cx.wrapping_mul(cz).wrapping_add(0x4000)) >> 15)
                .wrapping_mul(sy)
                .wrapping_add(sx.wrapping_mul(sz))
                >> 15,
        );
        self.czy = word(
            ((cx.wrapping_mul(sy).wrapping_add(0x4000)) >> 15)
                .wrapping_mul(sz)
                .wrapping_sub(sx.wrapping_mul(cz))
                >> 15,
        );
        self.czz = word(cy.wrapping_mul(cx) >> 15);
    }

    /// `rotate` as the release builds it (image `0x1307a-0x1323d`), not the tree's 16-bit
    /// `VECT.C:180-201`. Per point and axis, in 32 bits: `(x*cxx + y*cxy + z*cxz) >> 11`
    /// (`mov cl,0bh` into `LXRSH@`), its low word plus `tx << 4` — a word in **1/16 units**,
    /// kept in a local. The point's `xx` is that word `>> 4` (`sar ax,4`); the projection
    /// divides the 1/16 words: `xxx = (xx16 << 8) / zz16 + 140h` (`LXLSH@`, `LDIV@`) and
    /// `yyy = yy16 * 142 / zz16 + 42h`.
    fn rotate(&mut self) {
        let (y_scale, y_origin) = PROJECTION_Y;
        let word = |value: i32| -> i32 { (value as i16) as i32 };
        let fine = |x: i32, y: i32, z: i32, m: [i32; 3], offset: i32| -> i32 {
            let sum = x
                .wrapping_mul(m[0])
                .wrapping_add(y.wrapping_mul(m[1]))
                .wrapping_add(z.wrapping_mul(m[2]));
            word((sum >> 11).wrapping_add(offset << 4))
        };
        for index in 0..self.points.len() {
            let [x, y, z] = self.points[index];
            let xx = fine(x, y, z, [self.cxx, self.cxy, self.cxz], self.tx);
            let yy = fine(x, y, z, [self.cyx, self.cyy, self.cyz], self.ty);
            let zz = fine(x, y, z, [self.czx, self.czy, self.czz], self.dis);
            self.xx[index] = [xx >> 4, yy >> 4, zz >> 4];
            // The tree's `+160+160` is the release's one `add ax,140h`.
            let xxx = word(word((xx << 8) / zz).wrapping_add(160 + 160));
            // `VECT.OBJ`'s and the release's `yy*142/zz + 66` on the 134-row screen.
            let yyy = word(word(yy.wrapping_mul(y_scale) / zz).wrapping_add(y_origin));
            self.xxx[index] = [xxx, yyy];
        }
    }

    /// `VECT.C:203-246` `sort_faces`.
    fn sort_faces(&mut self) {
        let mut a = 0usize;
        let mut p = 0usize;
        while a < self.pg.len() {
            let pg = self.pg[a];
            let p1 = &self.xx[pg[0] as usize];
            let (x, y, z) = (p1[0], p1[1], p1[2]);
            let p2 = &self.xx[pg[1] as usize];
            let ax = p2[0].wrapping_sub(x);
            let ay = p2[1].wrapping_sub(y);
            let az = p2[2].wrapping_sub(z);
            let p3 = &self.xx[pg[2] as usize];
            let bx = p3[0].wrapping_sub(x);
            let by = p3[1].wrapping_sub(y);
            let bz = p3[2].wrapping_sub(z);
            let nx = ay.wrapping_mul(bz).wrapping_sub(az.wrapping_mul(by));
            let ny = az.wrapping_mul(bx).wrapping_sub(ax.wrapping_mul(bz));
            let nz = ax.wrapping_mul(by).wrapping_sub(ay.wrapping_mul(bx));
            let kx = -x;
            let ky = -y;
            let kz = -z;
            let s = kx
                .wrapping_mul(nx)
                .wrapping_add(ky.wrapping_mul(ny))
                .wrapping_add(kz.wrapping_mul(nz));
            if s > 0 {
                a += 1;
                continue;
            }
            let shade = (self
                .ls_x
                .wrapping_mul(nx)
                .wrapping_add(self.ls_y.wrapping_mul(ny))
                .wrapping_add(self.ls_z.wrapping_mul(nz)))
                / 250_000
                + 32;
            self.light_src[p] = shade;
            let color = pg[7] as usize;
            if self.lls[p] != shade {
                let mut fpal = self.copper.borrow().fpal;
                shadepal(&mut fpal, &self.pal, color * 64 * 3, shade);
                self.copper.borrow_mut().fpal = fpal;
                self.lls[p] = shade;
            }
            self.ptodraw[p] = a as i32;
            p += 1;
            a += 1;
        }
        self.polys = p as i32;
    }

    /// `VECT.C:249-263` `draw`.
    fn draw_cube(&mut self) {
        let shift = (self.page & 1) as i32 * 2;
        for i in 0..self.polys as usize {
            let face = self.ptodraw[i] as usize;
            let pg = self.pg[face];
            let point = |index: i32| -> (i32, i32) {
                let at = index as usize;
                (self.xxx[at][0] + shift, self.xxx[at][1])
            };
            let coords = [point(pg[0]), point(pg[1]), point(pg[2]), point(pg[3])];
            self.do_poly(coords, pg[7] as usize, self.frames & 63);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parts::plzpart::release::RELEASE_SINIT_WORDS;
    use crate::parts::plzpart::tables::tests::tables;

    /// A part holding what `init` reads for the cube — the fragments and the release's
    /// camera path and sine table — without a run.
    fn cube_part() -> Plzpart {
        use super::super::release::{release_cube, tests::image};
        let release = release_cube(&image()).expect("the shipped image is this build");
        let mut part = Plzpart::new();
        part.tables = Some(tables());
        part.sinit = release.sinit;
        part.buu = release.buu;
        part
    }

    /// `getspl` over the release's `buu`, then `calculate`'s masks and light, at the frames
    /// the keyframes draw: boot 8500 is frames 136, boot 9500 frames 1136 (the gate is boot
    /// 8364), and frames 3000 sits in the `REPT 100` tail.
    #[test]
    fn getspl_interpolates_the_release_camera_spline_at_the_keyframes() {
        let mut part = cube_part();
        let mut camera_at = |frames: i32| -> [i32; 8] {
            part.frames = frames;
            part.calculate();
            [
                part.tx, part.ty, part.dis, part.kx, part.ky, part.kz, part.ls_kx, part.ls_ky,
            ]
        };
        assert_eq!(camera_at(0), [0, 432, 499, 3999, 5599, 3999, 0, 0]);
        assert_eq!(camera_at(136), [0, 0, 499, 5676, 3914, 2314, 79, 0]);
        // The close-up: the camera at dis 349, where the tree's flat 300 put it at 299.
        assert_eq!(camera_at(1136), [0, 0, 349, 5416, 4181, 2582, 2047, 0]);
        assert_eq!(camera_at(3000), [0, 0, 499, 0, 0, 0, 2047, 4095]);
    }

    /// `ls_y = kosinit[ls_kx]>>8` (`VECT.C:144`; image `0x12cbd-0x12cd0` reads `ls_kx`), not
    /// `kosinit[ls_ky]`: the light is then a vector of length 127 and, over the cube's whole
    /// run, every visible face's shade stays inside its 64-entry palette band, `0..=64`.
    #[test]
    fn the_light_keeps_every_shade_inside_its_band() {
        let mut part = cube_part();
        part.frames = 1136;
        part.calculate();
        // The close-up's light: `ls_kx` a quarter turn, `ls_ky` 0, so `ls_y` is the
        // quarter turn's cosine, 0 — and `kosinit[ls_ky]` would have been 127.
        assert_eq!((part.ls_kx, part.ls_ky), (2047, 0));
        assert_eq!((part.ls_x, part.ls_y, part.ls_z), (0, 0, 126));
        assert_eq!(part.cosine(part.ls_ky) >> 8, 127);
        let mut drawn = 0;
        for frames in 0..2200 {
            part.frames = frames;
            part.calculate();
            for p in 0..part.polys as usize {
                let shade = part.light_src[p];
                assert!((0..=64).contains(&shade), "frames {frames}: shade {shade}");
                drawn += 1;
            }
        }
        assert!(drawn > 2200, "the run drew faces ({drawn})");
    }

    #[test]
    fn the_cube_projection_is_the_trees_142_66() {
        let mut part = Plzpart::new();
        part.cxx = 32767; // a 1.15 identity: the release's matrix scale
        part.cyy = 32767;
        part.czz = 32767;
        part.dis = 320;
        part.tx = 0;
        part.ty = -50;
        part.rotate();
        // (125,125,125) with the identity: (125*32767) >> 11 = 1999 in 1/16 units, plus
        // tx<<4, ty<<4 = -800 and dis<<4 = 5120 — 1999, 1199 and 7119.
        // The point keeps `>> 4` of each: 124, 74, 444.
        assert_eq!(part.xx[0], [124, 74, 444]);
        // The projection divides the 1/16 words: x = xx16*256/zz16 + 320 (the tree's
        // literal 160+160, the release's `add ax,140h`), y = yy16*142/zz16 + 66.
        assert_eq!(part.xxx[0][0], (1999 * 256) / 7119 + 160 + 160);
        assert_eq!(part.xxx[0][1], (1199 * 142) / 7119 + 66);
        assert_eq!(part.xxx[0], [391, 89]);
    }

    /// Output line y shows row floor(2y/3) of the image (CRTC 0x09 = 2,
    /// `TWEAK.ASM:31-33`; the 200-line frame is every other scanline).
    #[test]
    fn the_cube_frame_shows_row_two_y_over_three() {
        let rows: Vec<u32> = (0..200u32).map(|y| (cube_linear(0, y, 0)) / 640).collect();
        assert_eq!(&rows[..7], &[0, 0, 1, 2, 2, 3, 4]);
        assert_eq!(rows[199], 132);
        // Line 200 would be row 133: the 134-row image is exactly covered.
        assert_eq!(2 * 199 / 3, 132);
        // The column is added unchanged, and the address wraps at the image.
        assert_eq!(cube_linear(160, 3, 5), 160 + 2 * 640 + 5);
        assert_eq!(cube_linear(VIDEO_MASK, 0, 1), 0);
    }

    /// The release's `count_const` (image `0x12d40`): the tree's expressions with `>>15`
    /// where the tree has `>>15+7`, and `cxz = -SY` where it has `-SY>>7` — a 1.15 matrix.
    #[test]
    fn count_const_is_the_releases_one_point_fifteen_matrix() {
        let image = super::super::release::tests::image();
        // `cxx`: `call far LXMUL@; mov cl,0fh; call far LXRSH@; mov [7570h],ax`.
        assert_eq!(
            &image.image[0x12d69..0x12d78],
            &[
                0x9a, 0x31, 0x0e, 0x00, 0x00, 0xb1, 0x0f, 0x9a, 0x1a, 0x0f, 0x00, 0x00, 0xa3, 0x70,
                0x75
            ]
        );
        // `cxz`: `neg ax; mov [7574h],ax`, no shift.
        assert_eq!(
            &image.image[0x12dbd..0x12dc2],
            &[0xf7, 0xd8, 0xa3, 0x74, 0x75]
        );
        let mut part = Plzpart::new();
        part.sinit = vec![0; RELEASE_SINIT_WORDS];
        part.sinit[2048] = 32759; // sinit[2048], the quarter turn
        part.sinit[4096] = 32759; // `kosinit[2048] = sinit[2048 + 2048]`
        part.kx = 0;
        part.ky = 2048;
        part.kz = 2048;
        part.count_const();
        // SX = sinit[0] = 0, CX = kosinit[0] = 32759, and SY = CY = SZ = CZ = 32759.
        assert_eq!(part.cxx, (32759 * 32759) >> 15);
        assert_eq!(part.cxx, 32750);
        assert_eq!(part.cxz, -32759);
        // cyx subtracts inside the final shift: (((SX*CZ+0x4000)>>15)*SY - CX*SZ) >> 15, and
        // with SX = 0 the rounded inner product is 0x4000 >> 15 = 0.
        assert_eq!(part.cyx, (-32759 * 32759) >> 15);
        assert_eq!(part.cyx, -32751);
        // With kx a quarter turn too, every sine and cosine is 32759 and the rounded inner
        // product shows: cyx = (((32759*32759 + 0x4000) >> 15) * 32759 - 32759*32759) >> 15.
        part.kx = 2048;
        part.count_const();
        let inner = (32759 * 32759 + 0x4000) >> 15;
        assert_eq!(inner, 32750);
        assert_eq!(part.cyx, (inner * 32759 - 32759 * 32759) >> 15);
        assert_eq!(part.cyx, -9, "the subtraction is inside the shift");
    }

    #[test]
    fn the_face_palette_is_shaded_at_its_own_block() {
        let mut pal = [0u8; 768];
        pal[0] = 63;
        pal[192] = 32;
        let mut fpal = [0u8; 768];
        shadepal(&mut fpal, &pal, 0, 32);
        // `shadepal` covers 192 bytes: entries 0..63 of the band.
        assert_eq!(fpal[0], ((63u16 * 32) >> 6) as u8);
        assert_eq!(fpal[192], 0, "the same 192-byte block only");
        shadepal(&mut fpal, &pal, 192, 32);
        assert_eq!(fpal[192], ((32u16 * 32) >> 6) as u8);
        assert_eq!(fpal[1], 0);
    }
}
