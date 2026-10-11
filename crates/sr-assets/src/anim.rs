//! PAM's animation: the Autodesk FLI the part was authored in, and the `.ANI`
//! record scheme the part's own packer made from it.
//!
//! | file | what it is | read by |
//! |---|---|---|
//! | `PAM/PRAX4.FLI` | 42 frames of 320x200 FLI, header and chunk records | [`decode_fli`], following `PAM/VFLI.C` |
//! | `PAM/OUT.ANI` | the same 42 frames as a run-length delta stream | [`decode_ani`], following `PAM/ANIM.C` and `PAM/ASMYT.ASM` |
//! | `PAM/OUT.IN0`-`IN4` | `OUT.ANI` as TASM `db` source, in five fragments | `PAM/INCLUDE.ASM:9-26`, by the assembler |
//! | `PAM/PAL.PAL` | the palette both animations are shown with | `PAM/ANIM.C:19-21` |
//!
//! `load_frames` is the door for both ([`crate::load_frames`]); `load` refuses
//! them, because a picture is one frame and these are 42.
//!
//! # The FLI
//!
//! A 128-byte header — `VFLI.C:63-69` reads a `u32` size, a `u16` magic, a `u16`
//! frame count that nothing ever reads again, the width and the height, and then
//! seeks to 0x80 — and after it a chain of frame records. Each record is a `u32`
//! size, the `0xF1FA` type, a chunk count and eight ignored bytes (`VFLI.C:73-84`);
//! each chunk is a `u32` size, a `u16` type and a body the size includes. The
//! record's size is the frame's end, not the last chunk's (`VFLI.C:83` notes it
//! and `:91` seeks there), so a chunk list that stops early still ends the frame
//! where the header said.
//!
//! The chunks `VFLI.C` decodes, and the only ones this module does:
//!
//! | type | `VFLI.C` | what it is |
//! |---|---|---|
//! | `0x000B` | `:149-160` | colour, verbatim — the file's own bytes are six-bit DAC values |
//! | `0x0004` | `:161-172` | colour, `>> 2` — eight-bit source, the IFF `CMAP`'s rule |
//! | `0x000C` | `:173-202` | line-compressed delta over a band of lines |
//! | `0x0007` | `:203-246` | word-oriented delta |
//! | `0x000F` | `:247-272` | byte-run, the key frame |
//!
//! **The colour rule is measured, and it is the verbatim one.** `PRAX4.FLI`'s
//! only colour chunk is a `0x000B` in frame 0 (chunk histogram: one `0x000B`, one
//! `0x000F`, 41 `0x000C`s) and its 768 payload bytes are `assets/PAM/PAL.PAL`
//! byte for byte — the six-bit palette `crate::pal` already rules on, the one
//! `PAM/ANIM.C:19-21` hands to `tw_setpalette` and `PAM/TWEAK.ASM:151-156` writes
//! to the DAC as it is. So `0x000B` is copied, and neither wrong rule survives
//! the comparison: [`crate::six_bit`] — the 6→8 *display* rule run backwards —
//! and the 8→6 `/4` each change 743 of the palette's 768 bytes, and entry 3's
//! `(6, 6, 7)` becomes `(1, 1, 2)` and `(1, 1, 1)`. `tests/anim.rs` pins both
//! counts, because a `<= 63` assertion passes under all three rules.
//!
//! Two details of the chunk are `VFLI.C`'s and not the usual reading:
//!
//! - **The count and the first entry are big-endian** — the only big-endian
//!   fields in either format. `VFLI.C:152-156` and `:163-168` build them as
//!   `getc()*256 + getc()`; the file agrees, `01 00` being 256 colours, and
//!   4 + 256*3 is the chunk's 778 bytes less its six-byte header exactly.
//! - **`VFLI.C` stores the payload from `palette[0]` whatever `first` says**
//!   (`:158`, `:170`) while it *writes the DAC* from `first` (`:157`, `:169`).
//!   The DAC is what the demo shows and what a palette is, so this decoder
//!   applies the range at `first`; `PRAX4.FLI`'s `first` is 0, where the two
//!   readings are the same file. `VFLI.C`'s own store is its `pal.pal` dump.
//!   This and the geometry note below are register entry A4 in
//!   `docs/fidelity/APPROXIMATIONS.md`, with a measured difference of zero.
//!
//! The initial canvas is zeros. `VFLI.C`'s VRAM starts as whatever DOS left
//! (`:6`) and its `lookup` table is the identity (`:39`), and `PRAX4.FLI` does
//! not depend on either: frame 0 is a `0x000F` byte-run that writes all 64,000
//! pixels (measured).
//!
//! # The ANI
//!
//! `PAM/ANIM.C`'s `packframe` (`:36-60`) walks a 64,000-byte frame against the
//! screen and writes, per record, one signed run length and — for a run that
//! writes — one colour byte, ending each frame with a `0x00` (`:59`). A positive
//! run writes that many pixels of the colour byte; a negative one skips
//! `-run` pixels, carrying the screen's pixels through untouched. Runs are capped
//! at 120 (`:48`, `:54`), and every frame starts on a 16-byte boundary (`:42`,
//! which is why `0x88` — `-120` — is the file's first byte). `PAM/ASMYT.ASM` is
//! the reader: `@@looppi` (`:41-45`) dispatches on the sign, `@@skip` (`:46-48`)
//! is `sub di, ax`, and `@@over` (`:191-198`) puts the next frame at
//! `ds + ceil(offset/16)`, which is the padding rule from the other side.
//!
//! **The base a frame is a delta from is the frame two back**, and frames 0 and
//! 1 come off a cleared screen. That is not a guess: `PAM/OUTTAA.C:51-60`
//! double-buffers, alternating `ulosta_frame(0x0a000)`/`tw_setstart(0)` with
//! `ulosta_frame(0x0a400)`/`tw_setstart(16384)`, so each frame is decoded into
//! the page the frame two back is still showing — and the encoder matches it:
//! `PAM/ANIM.C:28-30` copies `mpage1` into VRAM *before* `mpage2` moves into
//! `mpage1`. With a one-frame-back base only 4 of the 42 frames come out right
//! (`tests/anim.rs` measures that), while every one of the 42 matches the FLI's
//! own frames pixel for pixel — two readers that share no code, agreeing with
//! the bytes `ANIM.C` packed from what `VFLI.C:111` dumped.
//!
//! Two things the `.ANI` does not carry:
//!
//! - **No palette.** The frames come back with an empty one; `PAM/PAL.PAL` is
//!   the palette, the same 768 bytes frame 0 of the FLI carries.
//! - **No geometry.** The canvas is 320x200 by construction: `ANIM.C:44` counts
//!   to 64,000 and `PAM/TWEAK.ASM:20` opens mode 13h. The pixel order is flat —
//!   `ASMYT.ASM` derives the planar address from the index alone, with no line
//!   arithmetic anywhere — so a skip may cross a line and the next row simply
//!   continues, which is why nothing here treats the canvas as rows.
//!
//! The `.ANI` has no magic, so [`crate::sniff`] never answers it and it is loaded
//! by construction.
//!
//! # What is refused, and why
//!
//! `VFLI.C` prints `UNKNOWN BLOCK TYPE` and reads a key (`:275-276`) for a chunk
//! it does not implement, and its caller then seeks past the chunk (`:88`) —
//! a frame with missing pixels, silently. A chunk outside the table above is
//! [`AssetError::Unsupported`] here instead. (`0x0011` is `FLI_COPY` and
//! `0x0012` the postage stamp; no file in the tree carries either, and only
//! `0x000B`, `0x000C` and `0x000F` are exercised by any of them — the `0x0004`
//! and `0x0007` decodes are `VFLI.C`'s own, cited above, and unmeasured because
//! nothing in the tree reaches them.) So are a chunk body that outruns its
//! frame's end — `VFLI.C:87` decodes a body with no bound at all — a chunk
//! smaller than its own six-byte header, which would send `VFLI.C:73` seeking
//! backwards, a colour chunk whose range leaves the 256 entries, and a colour
//! byte above 63 in a `0x000B` (the DAC's six bits; `Palette::set_entry` would
//! mask it and show a different colour).
//!
//! **A delta packet that writes past the canvas is refused too.** `VFLI.C` has
//! no bound that stops one — its offset is a 16-bit `unsigned int` (`:177`, `:208`, `:251`) and
//! its only guard is `u<64000` *before* a packet, so a run that starts on the
//! last pixel writes into the rest of the VGA window and wraps through 0 — and
//! no file in the tree needs it: every packet of `PRAX4.FLI`'s 42 frames stays
//! inside its own 320-pixel line and inside the canvas (measured). This decoder
//! writes exactly the pixels the table above says, or refuses.
//!
//! Nothing here trusts a length: every index is preceded by the check that
//! bounds it, and a file that ends mid-record is [`AssetError::Truncated`] with
//! the byte it wanted and the byte it had, both file offsets.
//!
//! One shape completes without a divergence to declare: a frame record with no
//! chunks at all is a frame whose picture is the one before it, and it is
//! returned as such. `VFLI.C:71-91` would instead drop it — `blocksleft` reaches
//! zero only inside the chunk branch (`:89-90`), so a frame header read with an
//! empty chunk list leaves the counter untouched and the next record's header
//! resets it. No file in either tree carries one, which is why the choice is
//! visible only here.
//!
//! The refusals above and this one shape are the register's entry A3 in
//! `docs/fidelity/APPROXIMATIONS.md` — every place the port substitutes defined
//! behaviour for `VFLI.C`'s undefined — each with its measured difference, which
//! is zero: no shipped file reaches any of them.
//!
//! # A note on the geometry
//!
//! The canvas is the header's width and height, as elsewhere in the crate —
//! `VFLI.C` hardcodes 320 as the stride (`:183`, `:218`, `:254`, and again in the
//! unused pixel counter at `:98`) and
//! 64,000 as the end, which is the same decode for `PRAX4.FLI`, the tree's only
//! FLI; a file of another shape would be sheared onto a 320-wide canvas by
//! `VFLI.C` and is decoded at its own geometry here. Register entry A4.

use sr_engine::{Palette, Surface};

use crate::{check_geometry, AssetError, Format, Image};

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

fn word(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([bytes[at], bytes[at + 1]])
}

fn long(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

fn truncated(format: Format, needed: usize, had: usize) -> AssetError {
    AssetError::Truncated {
        format,
        needed,
        had,
    }
}

fn unsupported(format: Format, what: String) -> AssetError {
    AssetError::Unsupported { format, what }
}

// -------------------------------------------------------------------------------------------
// The FLI
// -------------------------------------------------------------------------------------------

/// Decode `PAM/PRAX4.FLI`'s format into the frames its record chain holds.
///
/// The canvas carries from frame to frame — that is what makes the `0x000C` and
/// `0x0007` chunks deltas — and each frame is returned with the palette state
/// when it completed, which is what a colour chunk later in the stream changes.
pub(crate) fn decode_fli(bytes: &[u8]) -> Result<Vec<Image>, AssetError> {
    if bytes.len() < FLI_HEADER {
        return Err(truncated(Format::Fli, FLI_HEADER, bytes.len()));
    }
    // The magic the crate's `sniff` reads, checked again because `load_frames`
    // can be handed a `Format::Fli` without a sniff having run.
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
        // A frame record. `at + FRAME_HEADER` cannot overflow: `at` is inside
        // the slice and the header is 16 bytes.
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
            // Bounded by the frame's own end. VFLI.C:87 reads a chunk body with
            // no bound at all and only re-seeks afterwards (:88), so a count
            // that overruns the frame reads the next frame's bytes as a body.
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

        // The frame is complete: this is where VFLI.C:102-113 dumps it, and
        // :91 seeks to the record's end rather than the last chunk's.
        let mut surface = Surface::new(w, h);
        surface.pixels_mut().copy_from_slice(&canvas);
        frames.push(Image { surface, palette });
        at = end;
    }
    Ok(frames)
}

/// A cursor over one chunk's body that remembers where the body sits in the
/// file, so a short read can name the byte it wanted. Both `Truncated` fields
/// are file offsets, and a body that stops short reports its own end — which is
/// inside the file, exactly as the IFF decoder reports a `BODY` chunk's end.
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

    /// The colour chunks' field order: `VFLI.C:152-156` reads `getc()*256+getc()`
    /// for both the count and the first entry, and nothing else in either format
    /// is big-endian.
    fn be_word(&mut self) -> Result<u16, AssetError> {
        let hi = self.u8()? as u16;
        let lo = self.u8()? as u16;
        Ok(hi << 8 | lo)
    }
}

/// A colour chunk: a big-endian count, a big-endian first entry, and three bytes
/// per entry. `shift` is 0 for `0x000B` — the six-bit values `PRAX4.FLI` carries
/// — and 2 for `0x0004`'s eight-bit ones (`VFLI.C:170`'s `>>2`).
///
/// `Palette::set_entry` masks with `& 0x3F`, so a byte that is not six bits is
/// refused rather than shown as a different colour.
fn read_colour(body: &mut Body, palette: &mut Palette, shift: u32) -> Result<(), AssetError> {
    let count = body.be_word()? as usize;
    let first = body.be_word()? as usize;
    if first + count > 256 {
        return Err(AssetError::Palette {
            what: format!(
                "a colour chunk names entries {first} to {} of 255; VFLI.C:157 writes the range \
                 to the DAC from `first` and :158 stores it from entry 0, and the tree's file \
                 carries one chunk, first 0, count 256",
                first + count - 1
            ),
        });
    }
    // The whole range in one bound, so a body that stops short of its count
    // reports the end of the payload the count promised as `needed` — the shape
    // `pal::decode` uses — and not the byte the entry-at-a-time loop would have
    // reached.
    let payload = body.take(count * 3)?;
    for (entry, rgb) in payload.chunks_exact(3).enumerate() {
        let rgb = [rgb[0] >> shift, rgb[1] >> shift, rgb[2] >> shift];
        if let Some(&bad) = rgb.iter().find(|&&c| c > 63) {
            return Err(AssetError::Palette {
                what: format!(
                    "entry {} is {} {} {}, and {bad} is not six bits; the FLI's 0x000b chunk \
                     carries the DAC values PAM/PAL.PAL already holds (ANIM.C:19-21 writes it to \
                     ports 0x3c8/0x3c9 untouched), so this is not that palette",
                    first + entry,
                    rgb[0],
                    rgb[1],
                    rgb[2]
                ),
            });
        }
        palette.set_entry((first + entry) as u8, rgb);
    }
    Ok(())
}

/// Write `run` pixels of one colour, or refuse a run that leaves the canvas.
///
/// The refusal is this decoder's one deliberate divergence from `VFLI.C`, which
/// has no bound on a run that has already started: its offset is 16-bit
/// (`VFLI.C:177`) and its only guard is `u<64000` before a packet, so the rest of
/// a long run lands in the 1,536 bytes of the VGA window past the screen and
/// `u` wrapping through 65,536 would write the top-left corner. No packet in the
/// tree's FLI does either — all 42 frames' packets stay inside their own line
/// (measured) — so the refusal cannot reject a shipped asset.
fn fill(canvas: &mut [u8], at: usize, run: usize, value: u8) -> Result<(), AssetError> {
    let last = canvas.len();
    let slots = canvas
        .get_mut(at..at + run)
        .ok_or_else(|| past_the_canvas(at, run, last))?;
    slots.fill(value);
    Ok(())
}

/// The same for a literal run, whose bytes come from the stream.
fn copy(canvas: &mut [u8], at: usize, data: &[u8]) -> Result<(), AssetError> {
    let last = canvas.len();
    let slots = canvas
        .get_mut(at..at + data.len())
        .ok_or_else(|| past_the_canvas(at, data.len(), last))?;
    slots.copy_from_slice(data);
    Ok(())
}

fn past_the_canvas(at: usize, run: usize, canvas: usize) -> AssetError {
    unsupported(
        Format::Fli,
        format!(
            "a delta packet writes {run} pixels from {at}, past the last of {canvas}; VFLI.C has \
             no bound between packets (:184, :219, :256) and its 16-bit offset would wrap into \
             the top-left corner, and no file in the tree needs it"
        ),
    )
}

/// `0x000C`, `VFLI.C:173-202`: a first line and a count, then per line a packet
/// count, then per packet a skip and a signed run — a positive run of literals,
/// a negative one of a repeated byte. The packets are bounded by the canvas end
/// and not by the line (`VFLI.C:184`), so one may spill into the next row, which
/// is what `VFLI.C` does and what the tree's file never asks for.
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
                copy(canvas, at, data)?;
                at += size as usize;
            } else {
                let run = 256 - size as usize;
                fill(canvas, at, run, body.u8()?)?;
                at += run;
            }
        }
    }
    Ok(())
}

/// `0x0007`, `VFLI.C:203-246`: a line count, then a packet count per line, a
/// negative one standing for that many lines skipped, then skip/run pairs whose
/// runs consume two bytes per pixel. The two bytes go to consecutive canvas
/// positions (`VFLI.C:226-230`, `:237-241`), which is how the tree's player
/// reads a word-oriented chunk — a decode no file in the tree exercises.
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
                    let a = body.u8()?;
                    let b = body.u8()?;
                    fill(canvas, at, 1, a)?;
                    at += 1;
                    fill(canvas, at, 1, b)?;
                    at += 1;
                }
            } else {
                let run = 256 - size as usize;
                let a = body.u8()?;
                let b = body.u8()?;
                for _ in 0..run {
                    fill(canvas, at, 1, a)?;
                    at += 1;
                    fill(canvas, at, 1, b)?;
                    at += 1;
                }
            }
        }
    }
    Ok(())
}

/// `0x000F`, `VFLI.C:247-272`: every line of the canvas in turn, a packet count
/// per line, and per packet a signed run — the key frame's encoder.
///
/// **The two run forms are the other way round from `0x000C`'s and `0x0007`'s.**
/// Here a size below `0x80` is a run of that many copies of one byte and a size
/// at or above it is a literal run of `256 - size` (`VFLI.C:258-268`), where the
/// deltas read a run of literals for the small size and one repeated byte for the
/// large one (`:189-198`, `:224-242`). Both are the format's, and swapping them
/// reads a whole key frame's worth of bytes from the wrong places — the ANI
/// cross-check in `tests/anim.rs` is what catches that.
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
                fill(canvas, at, run, body.u8()?)?;
                at += run;
            } else {
                let data = body.take(256 - size as usize)?;
                copy(canvas, at, data)?;
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
/// The stream carries neither geometry nor palette: the canvas is 320x200
/// (`ANIM.C:44`, `TWEAK.ASM:20`) and the palette is `PAM/PAL.PAL` (the same 768
/// bytes `PRAX4.FLI`'s colour chunk holds), so the frames come back with an
/// empty palette for the caller to fill the way `ANIM.C:19-21` does.
pub(crate) fn decode_ani(bytes: &[u8]) -> Result<Vec<Image>, AssetError> {
    let mut canvases: Vec<Vec<u8>> = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        // The double buffer: a frame is a delta from the canvas two frames back,
        // and the first two come off the cleared screen (OUTTAA.C:51-60,
        // ANIM.C:28-30).
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
                // A skip writes nothing: the pixels the frame before left there
                // are the frame (ASMYT.ASM:46-48's `sub di, ax`). `unsigned_abs`
                // and not `-run`: as an `i8` the byte is negative, and `0x80` is
                // `i8::MIN`, which negating overflows — the one run byte that
                // panics where every other one would not. `ANIM.C` cannot write
                // it (runs are capped at 120, `:48`, `:54`), and malformed input
                // must be an error here, never a panic (`crate`'s promise).
                let skip = run.unsigned_abs() as usize;
                fill_ani(&mut canvas, cursor, skip, None)?;
                cursor += skip;
            }
        }
        canvases.push(canvas);
        // The next frame starts at the next 16-byte boundary — ASMYT.ASM:191-198
        // adds `(si>>4) + ((si&15)!=0)` to the segment, which is this rounding.
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

/// A run or a skip, bounded by the 64,000-pixel canvas. `ANIM.C`'s encoder
/// cannot leave it — every run is capped by `cnt+a<64000` — so a stream that
/// does is malformed, and `ASMYT.ASM` would write past the screen's planar words
/// rather than wrap.
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
                 encoder stops at 64,000 (ANIM.C:44, :48) and ASMYT.ASM would write past the \
                 screen instead"
            ),
        )
    })?;
    if let Some(colour) = colour {
        slots.fill(colour);
    }
    Ok(())
}
