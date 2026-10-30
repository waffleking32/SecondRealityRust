//! What BEGLOGO (`BEG/BEG.C`) and ENDLOGO (`END/END.C`) share: two copies of
//! one program, each over its own byte-identical `ASM.ASM` (`lineblit`,
//! `setpalarea`) and `READP.C`. Both read a 320×400 Origin picture, set a
//! 400-line screen, blit it once and fade it in from white through
//! [`setpalarea`]'s short upload (`docs/fidelity/bugs.md` §4).

use sr_engine::part::{PartCtx, PartError};
use sr_engine::Palette;

use crate::parts::{asset, load_image};

/// The picture's geometry, and the screen's: 320×400 Mode X (`BEG.C:47-53`,
/// `END.C:75-80` clear CRTC 9's double-scan bit and its low five bits).
pub const PICTURE_WIDTH: usize = 320;
pub const PICTURE_HEIGHT: usize = 400;

/// `readp(palette,-1,pic)` and `:74-78`'s 400-row decode (`BEG.C`; `END.C:94-99`):
/// the picture at `rel` and its six-bit palette, as one call decodes the whole
/// container.
pub fn load_picture(part: &'static str, rel: &str) -> Result<(Vec<u8>, Palette), PartError> {
    let image = load_image(part, rel)?;
    let (width, height) = (image.width() as usize, image.height() as usize);
    if (width, height) != (PICTURE_WIDTH, PICTURE_HEIGHT) {
        return Err(PartError::new(
            part,
            format!(
                "{} is {width}x{height}; this part's screen is \
                 {PICTURE_WIDTH}x{PICTURE_HEIGHT}",
                asset(rel)
            ),
        ));
    }
    Ok((image.pixels().to_vec(), image.palette))
}

/// The CRTC and AC writes both parts make before the picture (`BEG.C:42-71`,
/// `END.C:67-87`): display start 0 (one 16-bit register pair, so `x_offset`
/// goes with it), CRTC 9 cleared to a 400-line screen of one scanline a row,
/// and overscan entry `border`. Pan and the line compare are left as the run
/// left them.
///
/// **The line offset is the unset one.** Every part the demo runs before
/// either (PAM, U2E) leaves the 320-pixel tweak's 80-byte CRTC 0x13, which is
/// the stride `lineblit(vram+y*80U,rowbuf)` writes, so the original reads its
/// picture row by row. The port draws at the surface's own 640-pixel rows,
/// which the unset offset reads; the inherited `Some(320)` would read every
/// second scanline from the picture's right margin (`beglogo.md` §5).
pub fn set_400_line_view(ctx: &mut PartCtx, border: u8) {
    ctx.view.y_line_offset = None;
    ctx.view.set_origin(0);
    ctx.view.height = PICTURE_HEIGHT as u16;
    ctx.view.double_scan = false;
    ctx.view.border = Some(border);
}

/// `readp(rowbuf,y,pic)` and `lineblit(vram+y*80,rowbuf)`, one row per video
/// row (`BEG.C:74-78`, `END.C:95-99`). `lineblit` (`ASM.ASM:53-82`) writes
/// each row's 80 bytes to all four planes; the port writes the same 320 pixels
/// through [`Surface::set`](sr_engine::Surface::set), at the surface's own row
/// stride, where the chunky model folds the plane interleave away.
pub fn blit(picture: &[u8], ctx: &mut PartCtx) {
    for (y, row) in picture.chunks_exact(PICTURE_WIDTH).enumerate() {
        for (x, pixel) in row.iter().enumerate() {
            ctx.surface.set(x, y, *pixel);
        }
    }
}

/// `pal2[a] = ((128-c)*63 + palette[a]*c)/128` (`BEG.C:82`, `END.C:103`),
/// integer division over six-bit entries: white at `c = 0`, the picture at
/// `c = 128`. The original fills `a = 0..765` and this computes all 256
/// entries; the difference cannot be read, because [`setpalarea`] stops inside
/// entry 169.
pub fn white_to_picture(picture: &Palette, c: u32) -> Palette {
    let mut ramp = Palette::new();
    for index in 0..=255u8 {
        let entry = picture.entry(index);
        let mut rgb = [0u8; 3];
        for (channel, value) in rgb.iter_mut().enumerate() {
            *value = (((128 - c) * 63 + entry[channel] as u32 * c) / 128) as u8;
        }
        ramp.set_entry(index, rgb);
    }
    ramp
}

/// `setpalarea(pal,start,count)` (`BEG/ASM.ASM:93-115`, `END/ASM.ASM:93-115`)
/// **with the byte-count bug the original ships** (`docs/fidelity/bugs.md`
/// §4): `:106-108` loads `count` into `cx`, `shl cx,1` and `add cx,ax`, so
/// `REPOUTSB` moves `2*count + start` bytes from DAC index `start`, not
/// `3*count`. BEGLOGO's `count` 254 moves 508 bytes (entries 0-168 and 169's
/// red), ENDLOGO's 255 moves 510 (entries 0-169); entries 170-255 are never
/// written.
pub fn setpalarea(dac: &mut Palette, ramp: &Palette, start: u32, count: u32) {
    let bytes = 2 * count + start;
    for written in 0..bytes {
        let at = 3 * start + written;
        // The DAC's write index is eight bits and wraps; no call reaches it.
        let entry = (at / 3 % 256) as u8;
        let channel = (at % 3) as usize;
        let mut rgb = dac.entry(entry);
        rgb[channel] = ramp.entry(entry)[channel];
        dac.set_entry(entry, rgb);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_white_to_picture_ramp_truncates_where_the_original_truncates() {
        // A float version agrees on c = 0 and c = 128 and disagrees in the
        // last third.
        let mut pal = Palette::new();
        pal.set_entry(0, [63, 40, 1]);
        assert_eq!(white_to_picture(&pal, 1).entry(0)[0], ((127 * 63 + 63) / 128) as u8);
        assert_eq!(white_to_picture(&pal, 64).entry(0)[0], ((64 * 63 + 63 * 64) / 128) as u8);
        assert_eq!(white_to_picture(&pal, 1).entry(0)[1], ((127 * 63 + 40) / 128) as u8);
        assert_eq!(white_to_picture(&pal, 128).entry(0), [63, 40, 1], "c = 128 is the picture");
        assert_eq!(white_to_picture(&pal, 0).entry(0), [63, 63, 63], "c = 0 is white");
        // (127*63 + 1*1)/128 = 62.5 truncates to 62 where a rounded ramp
        // would leave it at 63.
        assert_eq!(white_to_picture(&pal, 1).entry(0)[2], 62);
    }

    #[test]
    fn the_upload_is_two_count_plus_start_bytes() {
        let mut ramp = Palette::new();
        for index in 0u8..=255 {
            ramp.set_entry(index, [index % 64, 5, 6]);
        }
        let held = |dac: &mut Palette| {
            dac.set_entry(169, [9, 9, 9]);
            dac.set_entry(170, [1, 2, 3]);
            dac.set_entry(254, [4, 5, 6]);
            dac.set_entry(255, [7, 8, 9]);
        };
        // ENDLOGO's 255: 510 bytes, entries 0-169 whole.
        let mut dac = Palette::new();
        held(&mut dac);
        setpalarea(&mut dac, &ramp, 0, 255);
        assert_eq!(dac.entry(0), ramp.entry(0));
        assert_eq!(dac.entry(169), ramp.entry(169), "the last entry written whole");
        assert_eq!(dac.entry(170), [1, 2, 3], "the 511th byte is never written");
        assert_eq!(dac.entry(254), [4, 5, 6]);
        assert_eq!(dac.entry(255), [7, 8, 9], "this routine never writes 255");
        // BEGLOGO's 254: 508 bytes, entries 0-168 whole and 169's red.
        let mut dac = Palette::new();
        held(&mut dac);
        setpalarea(&mut dac, &ramp, 0, 254);
        assert_eq!(dac.entry(168), ramp.entry(168), "the last entry written whole");
        assert_eq!(dac.entry(169), [ramp.entry(169)[0], 9, 9], "only 169's red");
        assert_eq!(dac.entry(170), [1, 2, 3], "the 509th byte is never written");
    }
}
