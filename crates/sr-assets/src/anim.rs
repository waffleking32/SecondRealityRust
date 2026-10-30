//! PAM's animation: the Autodesk FLI the part was authored in, and the `.ANI` record stream
//! the part's own packer made from it. [`crate::load_frames`] reads both.
//!
//! | file | what it is | read by |
//! |---|---|---|
//! | `PAM/PRAX4.FLI` | 42 frames of 320x200 FLI | [`decode_fli`], following `PAM/VFLI.C` |
//! | `PAM/OUT.ANI` | the same 42 frames as a run-length delta stream | [`decode_ani`], following `PAM/ANIM.C` and `PAM/ASMYT.ASM` |
//! | `PAM/PAL.PAL` | the palette both are shown with | `PAM/ANIM.C:19-21` |
//!
//! # The FLI
//!
//! A 128-byte header (`VFLI.C:63-69`: size, magic, frame count, width, height, then a seek to
//! 0x80) and a chain of frame records: `u32` size, `0xF1FA`, a chunk count and eight ignored
//! bytes (`VFLI.C:73-84`). Each chunk is a `u32` size that includes its six-byte header, a `u16`
//! type and a body. The record's size ends the frame (`VFLI.C:83`, `:91`). The chunk types are
//! `VFLI.C`'s: `0x000B` colour verbatim (`:149-160`), `0x0004` colour `>> 2` (`:161-172`),
//! `0x000C` line delta (`:173-202`), `0x0007` word delta (`:203-246`), `0x000F` byte run
//! (`:247-272`).
//!
//! - `PRAX4.FLI`'s only colour chunk is a `0x000B` whose 768 bytes are `PAM/PAL.PAL`, so it is
//!   copied verbatim ([`crate::six_bit_palette`]).
//! - The colour chunk's count and first entry are big-endian (`VFLI.C:152-156`), the only
//!   big-endian fields in either format. The range is applied at `first`, as `VFLI.C` writes
//!   the DAC (`:157`), not from entry 0 as it stores it (`:158`); A4.
//! - The canvas is the header's geometry, where `VFLI.C` hardcodes a 320 stride (A4), and
//!   starts at zeros (frame 0 is a byte run that writes every pixel).
//! - Refused, where `VFLI.C` has undefined behaviour (A3): a chunk type outside the table, a
//!   body that outruns its frame, a chunk smaller than its header, a colour range past 256, a
//!   `0x000B` byte above 63, and a delta packet that writes past the canvas. A frame record with
//!   no chunks repeats the previous frame, where `VFLI.C:71-91` would drop it. No shipped file
//!   reaches any of these.
//!
//! # The ANI
//!
//! `PAM/ANIM.C`'s `packframe` (`:36-60`) writes per record a signed run length and, for a
//! positive run, one colour byte; `0x00` ends a frame (`:59`), runs are capped at 120 (`:48`,
//! `:54`) and every frame starts on a 16-byte boundary (`:42`; `ASMYT.ASM:191-198` reads it back).
//! A negative run skips `-run` pixels (`ASMYT.ASM:46-48`).
//!
//! - **A frame is a delta from the frame two back**, and frames 0 and 1 start from a cleared
//!   screen: `PAM/OUTTAA.C:51-60` double-buffers between two pages, and `PAM/ANIM.C:28-30`
//!   encodes against the same. Every frame then matches the FLI's pixel for pixel.
//! - No palette (the frames come back with an empty one) and no geometry: the canvas is 320x200
//!   (`ANIM.C:44`, `TWEAK.ASM:20`), flat, so a skip may cross a line.

use sr_engine::{Palette, Surface};

use crate::{
    check_geometry, le16 as word, le32 as long, six_bit_palette, truncated, unsupported, AssetError,
    Format, Image,
};

/// The FLI header: `VFLI.C:69` seeks straight to 0x80 and reads nothing between.
const FLI_HEADER: usize = 0x80;
/// A frame record's own header: `u32` size, `u16` type, `u16` chunks, and the
/// eight bytes `VFLI.C:82` skips.
const FRAME_HEADER: usize = 16;
/// What `VFLI.C:78` tests for at every record boundary: `getw` on `fa f1`.
const FRAME_MAGIC: u16 = 0xF1FA;
/// A chunk record's own header: `u32` size (which includes it) and `u16` type.
const CHUNK_HEADER: usize = 6;

/// `VFLI.C:149-160`: bytes already in the DAC's range, written untouched.
const COLOR_64: u16 = 0x000B;
/// `VFLI.C:161-172`: eight-bit values, shifted down two into the DAC's range.
const COLOR_256: u16 = 0x0004;
/// `VFLI.C:203-246`: word-oriented delta.
const DELTA_SS2: u16 = 0x0007;
/// `VFLI.C:173-202`: line-compressed delta.
const DELTA_LC: u16 = 0x000C;
/// `VFLI.C:247-272`: byte-run, which is what a key frame is.
const BYTE_RUN: u16 = 0x000F;

/// The `.ANI` canvas: 64,000 pixels of 320x200, `ANIM.C:44`'s `while(cnt<64000)`.
const ANI_CANVAS: usize = 64_000;
const ANI_WIDTH: usize = 320;
const ANI_HEIGHT: usize = 200;
/// `ANIM.C:42` pads the stream so a frame starts on a 16-byte boundary;
/// `ASMYT.ASM:191-198` computes the next frame's segment the same way.
const ANI_ALIGN: usize = 16;

// -------------------------------------------------------------------------------------------
// The FLI
// -------------------------------------------------------------------------------------------

/// Decode `PAM/PRAX4.FLI`'s format into the frames its record chain holds, each with the palette
/// as it stood when the frame completed.
pub(crate) fn decode_fli(bytes: &[u8]) -> Result<Vec<Image>, AssetError> {
    if bytes.len() < FLI_HEADER {
        return Err(truncated(Format::Fli, FLI_HEADER, bytes.len()));
    }
    // `load_frames` can be handed `Format::Fli` without a sniff.
    if !matches!(bytes[4], 0x11 | 0x12) || bytes[5] != 0xAF {
        return Err(AssetError::UnknownFormat);
    }
    let width = word(bytes, 8) as u32;
    let height = word(bytes, 10) as u32;
    check_geometry(Format::Fli, width, height)?;

    let (w, h) = (width as usize, height as usize);
    let mut canvas = vec![0u8; w * h];
    let mut palette = Palette::new();
    let mut frames = Vec::new();

    let mut at = FLI_HEADER;
    while at < bytes.len() {
        if at + FRAME_HEADER > bytes.len() {
            return Err(truncated(Format::Fli, at + FRAME_HEADER, bytes.len()));
        }
        let size = long(bytes, at) as usize;
        let kind = word(bytes, at + 4);
        if kind != FRAME_MAGIC {
            return Err(unsupported(
                Format::Fli,
                format!(
                    "a record of type {kind:#06x} at {at}: every record in the tree's FLI is \
                     0xf1fa (VFLI.C:78), and VFLI.C would decode this as a chunk of the frame \
                     before it (VFLI.C:87)"
                ),
            ));
        }
        if size < FRAME_HEADER {
            return Err(unsupported(
                Format::Fli,
                format!(
                    "a frame record of {size} bytes at {at} is smaller than its own 16-byte \
                     header; VFLI.C:73 would compute an end before the record and seek backwards"
                ),
            ));
        }
        let end = at + size;
        if end > bytes.len() {
            return Err(truncated(Format::Fli, end, bytes.len()));
        }
        let chunks = word(bytes, at + 6) as usize;

        let mut chunk_at = at + FRAME_HEADER;
        for _ in 0..chunks {
            // Bounded by the frame's end; VFLI.C:87 has no bound.
            if chunk_at + CHUNK_HEADER > end {
                return Err(truncated(Format::Fli, chunk_at + CHUNK_HEADER, end));
            }
            let chunk_size = long(bytes, chunk_at) as usize;
            let chunk_type = word(bytes, chunk_at + 4);
            if chunk_size < CHUNK_HEADER {
                return Err(unsupported(
                    Format::Fli,
                    format!(
                        "a chunk record of {chunk_size} bytes at {chunk_at} is smaller than its \
                         own six-byte header; VFLI.C:73 would seek backwards out of it"
                    ),
                ));
            }
            let chunk_end = chunk_at + chunk_size;
            if chunk_end > end {
                return Err(truncated(Format::Fli, chunk_end, end));
            }
            let mut body = Body::new(
                &bytes[chunk_at + CHUNK_HEADER..chunk_end],
                chunk_at + CHUNK_HEADER,
            );
            match chunk_type {
                COLOR_64 => read_colour(&mut body, &mut palette, 0)?,
                COLOR_256 => read_colour(&mut body, &mut palette, 2)?,
                DELTA_LC => delta_lc(&mut body, &mut canvas, w)?,
                DELTA_SS2 => delta_ss2(&mut body, &mut canvas, w)?,
                BYTE_RUN => byte_run(&mut body, &mut canvas, w, h)?,
                _ => {
                    return Err(unsupported(
                        Format::Fli,
                        format!(
                            "chunk type {chunk_type:#06x} at {chunk_at}: VFLI.C does not \
                             implement it, prints UNKNOWN BLOCK TYPE and skips the chunk \
                             (VFLI.C:273-277), and no file in the tree carries one"
                        ),
                    ))
                }
            }
            chunk_at = chunk_end;
        }

        // VFLI.C:102-113 dumps the frame here; :91 seeks to the record's end.
        let mut surface = Surface::new(w, h);
        surface.pixels_mut().copy_from_slice(&canvas);
        frames.push(Image { surface, palette });
        at = end;
    }
    Ok(frames)
}

/// A cursor over one chunk's body that knows its file offset, so a short read reports file
/// offsets (the body's end as `had`).
struct Body<'a> {
    bytes: &'a [u8],
    at: usize,
    cursor: usize,
}

impl<'a> Body<'a> {
    fn new(bytes: &'a [u8], at: usize) -> Body<'a> {
        Body {
            bytes,
            at,
            cursor: 0,
        }
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], AssetError> {
        let end = self.cursor + count;
        let slice = self
            .bytes
            .get(self.cursor..end)
            .ok_or_else(|| truncated(Format::Fli, self.at + end, self.at + self.bytes.len()))?;
        self.cursor = end;
        Ok(slice)
    }

    fn u8(&mut self) -> Result<u8, AssetError> {
        Ok(self.take(1)?[0])
    }

    fn word(&mut self) -> Result<u16, AssetError> {
        let lo = self.u8()? as u16;
        let hi = self.u8()? as u16;
        Ok(lo | hi << 8)
    }

    /// `VFLI.C:152-156`'s `getc()*256+getc()`.
    fn be_word(&mut self) -> Result<u16, AssetError> {
        let hi = self.u8()? as u16;
        let lo = self.u8()? as u16;
        Ok(hi << 8 | lo)
    }
}

/// A colour chunk: a big-endian count, a big-endian first entry, and three bytes per entry,
/// shifted right by `shift`: 0 for `0x000B`, 2 for `0x0004` (`VFLI.C:170`'s `>>2`).
fn read_colour(body: &mut Body, palette: &mut Palette, shift: u32) -> Result<(), AssetError> {
    let count = body.be_word()? as usize;
    let first = body.be_word()? as usize;
    if first + count > 256 {
        return Err(AssetError::Palette {
            what: format!(
                "a colour chunk names entries {first} to {} of 255; the tree's file carries one \
                 chunk, first 0, count 256",
                first + count - 1
            ),
        });
    }
    // One bound for the whole range, so `needed` is the end of the payload the count promised.
    let payload = body.take(count * 3)?;
    let shifted: Vec<u8> = payload.iter().map(|&c| c >> shift).collect();
    six_bit_palette(&shifted, first, palette)
}

/// `canvas[at..at + run]`, or a refusal: `VFLI.C` has no bound once a run has started (its 16-bit
/// offset, `:177`, would wrap into the top-left corner), and every packet of the tree's FLI stays
/// inside its own line (A3).
fn span(canvas: &mut [u8], at: usize, run: usize) -> Result<&mut [u8], AssetError> {
    let last = canvas.len();
    canvas.get_mut(at..at + run).ok_or_else(|| {
        unsupported(
            Format::Fli,
            format!(
                "a delta packet writes {run} pixels from {at}, past the last of {last}; VFLI.C \
                 has no bound between packets (:184, :219, :256)"
            ),
        )
    })
}

/// `0x000C`, `VFLI.C:173-202`: a first line and a count, then per line a packet
/// count, then per packet a skip and a signed run — a positive run of literals,
/// a negative one of a repeated byte. A packet may spill into the next row (`VFLI.C:184`).
fn delta_lc(body: &mut Body, canvas: &mut [u8], width: usize) -> Result<(), AssetError> {
    let first_line = body.word()? as usize;
    let lines = body.word()? as usize;
    for line in first_line..first_line + lines {
        let packets = body.u8()? as usize;
        let mut at = line * width;
        for _ in 0..packets {
            if at >= canvas.len() {
                break;
            }
            at += body.u8()? as usize;
            let size = body.u8()?;
            if size < 0x80 {
                let data = body.take(size as usize)?;
                span(canvas, at, data.len())?.copy_from_slice(data);
                at += size as usize;
            } else {
                let run = 256 - size as usize;
                span(canvas, at, run)?.fill(body.u8()?);
                at += run;
            }
        }
    }
    Ok(())
}

/// `0x0007`, `VFLI.C:203-246`: a line count, then a packet count per line, a
/// negative one standing for that many lines skipped, then skip/run pairs whose
/// runs consume two bytes per pixel, written to consecutive positions (`VFLI.C:226-230`,
/// `:237-241`). No file in the tree exercises it.
fn delta_ss2(body: &mut Body, canvas: &mut [u8], width: usize) -> Result<(), AssetError> {
    let lines = body.word()? as usize;
    let mut skip_lines = 0usize;
    for line in 0..lines {
        let mut packets = body.word()? as i16;
        while packets < 0 {
            skip_lines = (-(packets as i32) as usize) * width;
            packets = body.word()? as i16;
        }
        let mut at = line * width + skip_lines;
        for _ in 0..packets as usize {
            if at >= canvas.len() {
                break;
            }
            at += body.u8()? as usize;
            let size = body.u8()?;
            if size < 0x80 {
                for _ in 0..size as usize {
                    let pair = [body.u8()?, body.u8()?];
                    span(canvas, at, 2)?.copy_from_slice(&pair);
                    at += 2;
                }
            } else {
                let run = 256 - size as usize;
                let pair = [body.u8()?, body.u8()?];
                for _ in 0..run {
                    span(canvas, at, 2)?.copy_from_slice(&pair);
                    at += 2;
                }
            }
        }
    }
    Ok(())
}

/// `0x000F`, `VFLI.C:247-272`: every line of the canvas in turn, a packet count
/// per line, and per packet a signed run — the key frame's encoder.
///
/// **The run forms are the other way round from the deltas'**: below `0x80` a repeated byte,
/// at or above it `256 - size` literals (`VFLI.C:258-268`, against `:189-198`, `:224-242`).
fn byte_run(
    body: &mut Body,
    canvas: &mut [u8],
    width: usize,
    height: usize,
) -> Result<(), AssetError> {
    for line in 0..height {
        let packets = body.u8()? as usize;
        let mut at = line * width;
        for _ in 0..packets {
            if at >= canvas.len() {
                break;
            }
            let size = body.u8()?;
            if size < 0x80 {
                let run = size as usize;
                span(canvas, at, run)?.fill(body.u8()?);
                at += run;
            } else {
                let data = body.take(256 - size as usize)?;
                span(canvas, at, data.len())?.copy_from_slice(data);
                at += data.len();
            }
        }
    }
    Ok(())
}

// -------------------------------------------------------------------------------------------
// The ANI
// -------------------------------------------------------------------------------------------

/// Decode `PAM/OUT.ANI`'s record scheme into its frames.
///
/// The frames come back with an empty palette for the caller to fill from `PAM/PAL.PAL`, as
/// `ANIM.C:19-21` does.
pub(crate) fn decode_ani(bytes: &[u8]) -> Result<Vec<Image>, AssetError> {
    let mut canvases: Vec<Vec<u8>> = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        // The double buffer (OUTTAA.C:51-60, ANIM.C:28-30).
        let mut canvas = match canvases.len() {
            0 | 1 => vec![0u8; ANI_CANVAS],
            n => canvases[n - 2].clone(),
        };
        let mut cursor = 0;
        let mut p = at;
        loop {
            let run = *bytes
                .get(p)
                .ok_or_else(|| truncated(Format::Ani, p + 1, bytes.len()))?
                as i8;
            p += 1;
            if run == 0 {
                break;
            }
            if run > 0 {
                let colour = *bytes
                    .get(p)
                    .ok_or_else(|| truncated(Format::Ani, p + 1, bytes.len()))?;
                p += 1;
                fill_ani(&mut canvas, cursor, run as usize, Some(colour))?;
                cursor += run as usize;
            } else {
                // A skip writes nothing (ASMYT.ASM:46-48's `sub di, ax`). `unsigned_abs`,
                // because `0x80` is `i8::MIN`, which negating overflows.
                let skip = run.unsigned_abs() as usize;
                fill_ani(&mut canvas, cursor, skip, None)?;
                cursor += skip;
            }
        }
        canvases.push(canvas);
        // ASMYT.ASM:191-198 adds `(si>>4) + ((si&15)!=0)` to the segment.
        at = p + (ANI_ALIGN - p % ANI_ALIGN) % ANI_ALIGN;
    }

    Ok(canvases
        .into_iter()
        .map(|pixels| {
            let mut surface = Surface::new(ANI_WIDTH, ANI_HEIGHT);
            surface.pixels_mut().copy_from_slice(&pixels);
            Image {
                surface,
                palette: Palette::new(),
            }
        })
        .collect())
}

/// A run or a skip, bounded by the 64,000-pixel canvas `ANIM.C`'s encoder cannot leave
/// (`cnt+a<64000`).
fn fill_ani(
    canvas: &mut [u8],
    at: usize,
    run: usize,
    colour: Option<u8>,
) -> Result<(), AssetError> {
    let slots = canvas.get_mut(at..at + run).ok_or_else(|| {
        unsupported(
            Format::Ani,
            format!(
                "a record runs {run} pixels from {at}, past the last of {ANI_CANVAS}; ANIM.C's \
                 encoder stops at 64,000 (ANIM.C:44, :48)"
            ),
        )
    })?;
    if let Some(colour) = colour {
        slots.fill(colour);
    }
    Ok(())
}
