//! Asset decoding for the port: the original's formats, read as they are.
//!
//! One module per family: [`origin`] (the Origin PIC container), [`iff`] (ILBM and PBM),
//! [`rix`] (RIX3, behind `.CLX` and `.SCI`), [`pal`] (headerless VGA palettes), [`anim`]
//! (PAM's FLI and `.ANI`), [`visu`] (the vector engine's scene formats), [`poslist`] (the
//! FOREST/WATER column lists), [`font`] (the strip a part draws its text from), [`omf`] (the
//! linker's object container) and [`exe`] (the shipped, packed part EXEs).
//!
//! Three rules hold across every decoder:
//!
//! - **The extension never decides the format.** `.UH` holds four different containers in this
//!   tree and `.U` both a headered one and a bare dump. [`sniff`] answers from the magic alone,
//!   and `None` for a format with no magic; those are loaded by construction, through their own
//!   functions ([`raw_surface`], [`pal::decode`], [`load_frames`] for the `.ANI`, the [`visu`]
//!   readers). A size is not evidence: three 64,000-byte files here have three geometries.
//! - **Palettes stay six bits** ([`six_bit_palette`]). The one 8→6 rule is the IFF `CMAP`'s
//!   integer `/4` (`GRAB/LBM2U.C:170`).
//! - **A decoder never reads past its input.** Malformed and truncated input is an
//!   [`AssetError`], never a panic and never a short read.

pub mod anim;
pub mod exe;
pub mod font;
pub mod iff;
pub mod omf;
pub mod origin;
pub mod pal;
pub mod poslist;
pub mod rix;
pub mod visu;

use std::path::{Path, PathBuf};

use sr_engine::surface::{MAX_HEIGHT, MAX_WIDTH};
use sr_engine::{Palette, Surface};

/// A decoded picture: pixels in an engine surface, and the palette they index.
pub struct Image {
    pub surface: Surface,
    pub palette: Palette,
}

impl Image {
    pub fn width(&self) -> u16 {
        self.surface.width() as u16
    }

    pub fn height(&self) -> u16 {
        self.surface.height() as u16
    }

    pub fn pixels(&self) -> &[u8] {
        self.surface.pixels()
    }
}

/// The geometry only: a picture's pixels do not belong in a failure message.
impl std::fmt::Debug for Image {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Image")
            .field("width", &self.width())
            .field("height", &self.height())
            .finish_non_exhaustive()
    }
}

/// The container a file is in. [`sniff`] returns only the first four, the magic-carrying ones
/// (`Origin` covers all five magic variants of its family); the rest name a format in errors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Origin,
    Iff,
    Rix3,
    Fli,
    /// No magic: a bare pixel dump, loaded by [`raw_surface`].
    OriginBare,
    /// No magic: 768 palette bytes, loaded by [`pal::decode`].
    Pal,
    /// No magic: PAM's `.ANI` record stream, loaded by [`load_frames`].
    Ani,
    /// A linker object, read by [`omf`].
    Omf,
    /// The vector engine's `.00M`, `.0AA`, `.0AB` and `.00N`, read by [`visu`].
    Visu,
    /// FOREST's and WATER's `.DAT` column lists, read by [`poslist`].
    PosList,
    /// A DOS MZ executable, read by [`exe::unpack`].
    Exe,
}

impl std::fmt::Display for Format {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Format::Origin => "Origin PIC",
            Format::OriginBare => "bare 8-bit dump",
            Format::Iff => "IFF",
            Format::Rix3 => "RIX3",
            Format::Pal => "VGA palette",
            Format::Fli => "FLI",
            Format::Ani => "ANI",
            Format::Omf => "OMF object",
            Format::Visu => "VISU scene",
            Format::PosList => "position list",
            Format::Exe => "DOS executable",
        })
    }
}

#[derive(Debug)]
pub enum AssetError {
    /// Nothing in the first bytes matched a known magic.
    UnknownFormat,
    /// The bytes stop short of what the container promises, or are not the length it requires.
    ///
    /// Both fields are file offsets: `needed` the byte the decoder wanted, `had` the last byte it
    /// had to work with — the input's length, or the end of the chunk being read (an IFF `BODY`,
    /// an FLI chunk). `had` exceeds `needed` when the input is too long.
    Truncated { format: Format, needed: usize, had: usize },
    /// A construct no shipped file uses, where guessing would invent behaviour.
    Unsupported { format: Format, what: String },
    /// Larger than the largest surface the engine allocates; nothing is clipped to fit.
    TooLarge { width: u32, height: u32 },
    /// A palette outside its contract: out of range, or the wrong length.
    Palette { what: String },
    /// The file could not be read at all ([`load_path`] only).
    Io { path: PathBuf, what: String },
}

impl std::fmt::Display for AssetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AssetError::UnknownFormat => write!(f, "no known format magic in the first bytes"),
            AssetError::Truncated { format, needed, had } => {
                write!(f, "{format} needs {needed} bytes, {had} present")
            }
            AssetError::Unsupported { format, what } => write!(f, "{format}: {what}"),
            AssetError::TooLarge { width, height } => write!(
                f,
                "{width}x{height} is larger than the engine's largest surface, {MAX_WIDTH}x{MAX_HEIGHT}"
            ),
            AssetError::Palette { what } => write!(f, "palette: {what}"),
            AssetError::Io { path, what } => write!(f, "cannot read {}: {what}", path.display()),
        }
    }
}

impl std::error::Error for AssetError {}

pub(crate) fn truncated(format: Format, needed: usize, had: usize) -> AssetError {
    AssetError::Truncated {
        format,
        needed,
        had,
    }
}

pub(crate) fn unsupported(format: Format, what: impl Into<String>) -> AssetError {
    AssetError::Unsupported {
        format,
        what: what.into(),
    }
}

/// A little-endian `u16` at `at`; the caller has bounded the read.
pub(crate) fn le16(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([bytes[at], bytes[at + 1]])
}

/// A little-endian `u32` at `at`; the caller has bounded the read.
pub(crate) fn le32(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

/// Copy six-bit DAC triples into `palette` from entry `first`, verbatim.
///
/// Every palette the tree stores outside an IFF `CMAP` is already six bits: the Origin
/// writers' `getc(f1)/4` (`GRAB/LBM2U.C:170`), GLVP's palettes, the `.PAL` files and the RIX3
/// palettes (several of them a `CMAP / 4` byte for byte), the FLI's `0x000B` chunk and the VISU
/// `.00M`. Converting again would darken every colour by four. A component above 63 is refused,
/// because [`Palette::set_entry`] would mask it into a different colour.
pub(crate) fn six_bit_palette(rgb: &[u8], first: usize, palette: &mut Palette) -> Result<(), AssetError> {
    for (i, c) in rgb.chunks_exact(3).enumerate() {
        let entry = first + i;
        if let Some(&bad) = c.iter().find(|&&v| v > 63) {
            return Err(AssetError::Palette {
                what: format!(
                    "entry {entry} is {} {} {}, and {bad} is not six bits",
                    c[0], c[1], c[2]
                ),
            });
        }
        palette.set_entry(entry as u8, [c[0], c[1], c[2]]);
    }
    Ok(())
}

/// The format of `bytes`, by magic. `None` when nothing matches: a format with no magic is
/// loaded by construction instead.
pub fn sniff(bytes: &[u8]) -> Option<Format> {
    // The Origin family, byte 0 first: `FCFD` is stored FD FC (LBM2P.C:61) and
    // `0x6855` is stored 55 68 (GLVP.C:995).
    if let [b0, b1, ..] = bytes {
        match (*b0, *b1) {
            (0xFC, 0xFC) | (0xFD, 0xFC) | (0x55, 0x48) | (0x55, 0x68) => {
                return Some(Format::Origin)
            }
            _ => {}
        }
    }
    // IFF: `FORM` <size> `PBM ` (chunky) or `ILBM` (planar).
    if bytes.len() >= 12 && &bytes[0..4] == b"FORM" && matches!(&bytes[8..12], b"PBM " | b"ILBM") {
        return Some(Format::Iff);
    }
    if bytes.len() >= 4 && &bytes[0..4] == b"RIX3" {
        return Some(Format::Rix3);
    }
    // Autodesk FLI: a u32 file size, then the magic as a little-endian u16 — bytes `11 AF` for
    // 0xAF11 (`PAM/PRAX4.FLI` begins `74 1e 05 00 11 af`), `12 AF` for the 0xAF12 revision.
    if bytes.len() >= 6 && matches!(bytes[4], 0x11 | 0x12) && bytes[5] == 0xAF {
        return Some(Format::Fli);
    }
    None
}

/// Decode one picture: [`Format::Origin`], [`Format::Iff`] or [`Format::Rix3`], each choosing
/// its variant from its own header. Every other format is refused with the function that reads
/// it named in the error.
pub fn load(bytes: &[u8], format: Format) -> Result<Image, AssetError> {
    match format {
        Format::Origin => origin::decode(bytes),
        Format::Iff => iff::decode(bytes),
        Format::Rix3 => rix::decode(bytes),
        Format::Pal => Err(unsupported(format, "a palette, not a picture: pal::decode reads it")),
        Format::OriginBare => Err(unsupported(
            format,
            "no header, so no geometry: raw_surface(bytes, width, height) reads it",
        )),
        Format::Fli | Format::Ani => Err(unsupported(
            format,
            "an animation, not a picture: load_frames(bytes, format) reads it",
        )),
        Format::Omf => Err(unsupported(format, "a linker object: omf::extract_ledata reads it")),
        Format::Visu => Err(unsupported(format, "vector data: the visu::read_* functions read it")),
        Format::PosList => Err(unsupported(format, "column lists: PosList::parse reads it")),
        Format::Exe => Err(unsupported(format, "an executable: exe::unpack reads it")),
    }
}

/// Read a file and load it as whatever its magic says it is.
pub fn load_path(path: &Path) -> Result<Image, AssetError> {
    let bytes = std::fs::read(path).map_err(|e| AssetError::Io {
        path: path.to_path_buf(),
        what: e.to_string(),
    })?;
    let format = sniff(&bytes).ok_or(AssetError::UnknownFormat)?;
    load(&bytes, format)
}

/// Every frame in `bytes`: a still picture is one frame, and the two animations are decoded to
/// all of theirs ([`anim`]). A trailing partial frame is [`AssetError::Truncated`].
pub fn load_frames(bytes: &[u8], format: Format) -> Result<Vec<Image>, AssetError> {
    match format {
        Format::Origin | Format::Iff | Format::Rix3 => Ok(vec![load(bytes, format)?]),
        Format::Fli => anim::decode_fli(bytes),
        Format::Ani => anim::decode_ani(bytes),
        _ => Err(unsupported(format, "not a container of frames")),
    }
}

/// A headerless pixel dump (`PANIC/MONSTER.U`, `LENS/LENS.U`, `GRAB/BLOWFNT.1BM`), whose
/// geometry only the caller knows. `bytes.len()` must be `width * height` exactly. The dump
/// carries no palette; its part supplies one.
pub fn raw_surface(bytes: &[u8], width: u16, height: u16) -> Result<Image, AssetError> {
    let format = Format::OriginBare;
    check_geometry(format, width as u32, height as u32)?;
    let needed = width as usize * height as usize;
    if bytes.len() != needed {
        return Err(truncated(format, needed, bytes.len()));
    }
    let mut surface = Surface::new(width as usize, height as usize);
    surface.pixels_mut().copy_from_slice(bytes);
    Ok(Image {
        surface,
        palette: Palette::new(),
    })
}

/// Refuse an image the engine's surface cannot hold, before anything is allocated for it.
///
/// The bound is the *area* `MAX_WIDTH * MAX_HEIGHT`, not the width: `ALKU/FONA.UH` is a 1500x30
/// strip, wider than `MAX_WIDTH` yet a quarter of the largest surface.
pub(crate) fn check_geometry(format: Format, width: u32, height: u32) -> Result<(), AssetError> {
    let (w, h) = (width as usize, height as usize);
    if w == 0 || h == 0 {
        return Err(unsupported(format, format!("a {width}x{height} image has no pixels")));
    }
    if w * h > MAX_WIDTH * MAX_HEIGHT {
        return Err(AssetError::TooLarge { width, height });
    }
    Ok(())
}
