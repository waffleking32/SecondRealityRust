//! Asset decoding for the port: the original's formats, read as they are.
//!
//! One module per family. Task A2 lands the Origin PIC container ([`origin`])
//! and A3 the IFF images ([`iff`]); RIX3 and `.PAL` (A4) and the two animation
//! formats (A6) follow and plug into [`load`] and [`sniff`].
//!
//! Three rules hold across every decoder here, and each one has cost a colour or
//! a pixel somewhere before:
//!
//! - **The extension never decides the format.** `.UH` holds four different
//!   containers in this tree, and `.U` holds both a headered one and a bare dump;
//!   [`sniff`] answers from the magic bytes alone.
//! - **Palettes stay six bits.** A palette is read as the original stored it —
//!   the Origin family already stores six-bit values, which must **not** be run
//!   through [`six_bit`], whose job is the other direction (8-bit IFF `CMAP`).
//! - **A decoder never reads past its input.** Malformed and truncated input is
//!   an [`AssetError`], never a panic and never a short read.

pub mod iff;
pub mod origin;

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

/// The geometry and nothing else — a picture's pixels do not belong in a test
/// failure's message, and an `Image` is what `Result::unwrap_err` has to drop.
impl std::fmt::Debug for Image {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Image")
            .field("width", &self.width())
            .field("height", &self.height())
            .finish_non_exhaustive()
    }
}

/// The container a file is in, decided by its magic bytes.
///
/// [`Format::Origin`] covers all five magic-carrying variants of the Origin
/// family, and [`load`] dispatches on the magic internally, so a caller never
/// has to tell raw from RLE from bit-planes. [`Format::OriginBare`] has no header
/// and therefore no magic: `sniff` never returns it and `load` refuses it,
/// because it cannot know the geometry — [`raw_surface`] is its door.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Origin,
    OriginBare,
    Iff,
    Rix3,
    Pal,
    Fli,
    Ani,
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
        })
    }
}

#[derive(Debug)]
pub enum AssetError {
    /// Nothing in the first bytes matched a known magic.
    UnknownFormat,
    /// The bytes stop short of what the container promises. `had` is
    /// sometimes *more* than `needed`: the same variant reports a buffer that is
    /// not the length the geometry requires, in either direction.
    Truncated { format: Format, needed: usize, had: usize },
    /// A construct no shipped file uses, where guessing would invent behaviour.
    Unsupported { format: Format, what: String },
    /// Larger than the largest surface the engine allocates — nothing is
    /// clipped to fit.
    TooLarge { width: u32, height: u32 },
    /// A palette outside its contract: out of range, or the wrong length.
    Palette { what: String },
    /// The file could not be read at all — [`load_path`] only. Every other
    /// variant describes bytes that *were* read. The path is kept whole, not
    /// stringified, so a caller can re-open it (and so a path that is not valid
    /// UTF-8 survives the round trip).
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

/// The format of `bytes`, by magic. `None` when nothing matches — a headerless
/// file has nothing to match, and is loaded by construction instead.
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
    // Autodesk FLI: a u32 file size, then the magic as a little-endian u16 —
    // bytes `11 AF` for 0xAF11 in this tree (`PAM/PRAX4.FLI` begins
    // `74 1e 05 00 11 af`), `12 AF` for the later 0xAF12 revision.
    if bytes.len() >= 6 && matches!(bytes[4], 0x11 | 0x12) && bytes[5] == 0xAF {
        return Some(Format::Fli);
    }
    // `.PAL` (768 raw bytes) and `.ANI` (frames back to back) have no magic at
    // all: they are loaded by construction, never sniffed.
    None
}

/// Decode `bytes` as `format`. [`Format::Origin`] reads its own magic to choose
/// between the family's five variants; [`Format::Iff`] reads its `FORM` type the
/// same way, between `ILBM` and `PBM `.
///
/// Every other format is [`AssetError::Unsupported`] until its decoder lands:
/// RIX3 and `.PAL` with A4, FLI and ANI with A6.
pub fn load(bytes: &[u8], format: Format) -> Result<Image, AssetError> {
    match format {
        Format::Origin => origin::decode(bytes),
        Format::Iff => iff::decode(bytes),
        Format::OriginBare => Err(AssetError::Unsupported {
            format,
            what: "no header, so no geometry: raw_surface(bytes, width, height) is its door"
                .to_string(),
        }),
        Format::Rix3 | Format::Pal | Format::Fli | Format::Ani => Err(AssetError::Unsupported {
            format,
            what: "no decoder for this format yet".to_string(),
        }),
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

/// Every frame in `bytes`. A still container is one frame; only FLI and the
/// `.ANI` scheme carry more than one, and those land with task A6.
pub fn load_frames(bytes: &[u8], format: Format) -> Result<Vec<Image>, AssetError> {
    match format {
        Format::Origin | Format::Iff => Ok(vec![load(bytes, format)?]),
        _ => Err(AssetError::Unsupported {
            format,
            what: "not implemented; only the Origin family and IFF are containers this decoder \
                   reads yet"
                .to_string(),
        }),
    }
}

/// A headerless pixel dump — `PANIC/MONSTER.U`, `LENS/LENS.U`,
/// `GRAB/BLOWFNT.1BM` — whose geometry only the caller knows, because there is
/// nothing in the file to read it from. Requires `bytes.len() == width * height`
/// exactly: a size that merely looks plausible is not evidence, three files of
/// 64,000 bytes in this tree have three different geometries.
pub fn raw_surface(bytes: &[u8], width: u16, height: u16) -> Result<Image, AssetError> {
    let format = Format::OriginBare;
    check_geometry(format, width as u32, height as u32)?;
    let needed = width as usize * height as usize;
    if bytes.len() != needed {
        return Err(AssetError::Truncated {
            format,
            needed,
            had: bytes.len(),
        });
    }
    let mut surface = Surface::new(width as usize, height as usize);
    surface.pixels_mut().copy_from_slice(bytes);
    // A bare dump carries no palette; its part supplies one from the `.PAL`
    // beside it (task A4).
    Ok(Image {
        surface,
        palette: Palette::new(),
    })
}

/// The inverse of [`sr_engine::dac_expand`]'s rounding: `round(v * 63 / 255)`,
/// where `dac_expand` is `round(v * 255 / 63)`.
///
/// For loaders that read **8-bit** palette data — the IFF `CMAP` (task A3) and
/// nothing else. The Origin family stores six-bit values already and must be
/// copied verbatim: this is not the identity over six-bit input (`six_bit(32)`
/// is 8, and running it over an Origin palette would darken every picture by
/// roughly a factor of four).
pub const fn six_bit(v: u8) -> u8 {
    ((v as u32 * 63 + 127) / 255) as u8
}

/// Refuse an image the engine's surface cannot hold, before anything is
/// allocated for it.
///
/// The bound is the *area* of the widest surface the engine allocates,
/// `MAX_WIDTH * MAX_HEIGHT` = 256,000 pixels, not the width alone: the demo's
/// files include a 1500x30 font strip (`ALKU/FONA.UH`), which is wider than
/// `MAX_WIDTH` yet a quarter of the largest surface. Every shipped picture fits
/// the area rule — 320x200, 320x400, 640x200, 640x400 and 1500x30 all do — and a
/// 1024x400 header, which no part ships, does not.
pub(crate) fn check_geometry(format: Format, width: u32, height: u32) -> Result<(), AssetError> {
    let (w, h) = (width as usize, height as usize);
    if w == 0 || h == 0 {
        return Err(AssetError::Unsupported {
            format,
            what: format!("a {width}x{height} image has no pixels"),
        });
    }
    if w * h > MAX_WIDTH * MAX_HEIGHT {
        return Err(AssetError::TooLarge { width, height });
    }
    Ok(())
}
