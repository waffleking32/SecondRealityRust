//! Asset decoding for the port: the original's formats, read as they are.
//!
//! One module per family. Task A2 lands the Origin PIC container ([`origin`]),
//! A3 the IFF images ([`iff`]) and A4 the RIX3 screens ([`rix`], behind `.CLX`
//! and `.SCI`) and the headerless VGA palettes ([`pal`]); A6 the two animation
//! formats ([`anim`]) — the FLI `sniff` reads and the `.ANI` it cannot, which
//! `load_frames` is the door for. [`omf`] (A5) is the odd one out:
//! it reads the `.OBK`/`.OBJ` container the original's linker put assets in,
//! never an image, and exists to prove the tree complete. [`font`] (A7) is the
//! last of them and the only one that is not a decoder: it is the strip view a
//! part draws its text through, and the geometry it slices with is the part's,
//! read off its own text routine. [`visu`] reads the vector engine's four
//! packed scene formats — plain data, with no picture and no magic, so its
//! four functions are their doors.
//!
//! Three rules hold across every decoder here, and each one has cost a colour or
//! a pixel somewhere before:
//!
//! - **The extension never decides the format.** `.UH` holds four different
//!   containers in this tree, and `.U` holds both a headered one and a bare dump;
//!   [`sniff`] answers from the magic bytes alone — and for the two formats with
//!   no magic at all ([`Format::OriginBare`] and [`Format::Pal`]) it answers
//!   `None`, because a name or a size is not evidence. Those two are loaded by
//!   construction, through [`raw_surface`] and [`pal::decode`].
//! - **Palettes stay six bits.** A palette is read as the original stored it.
//!   The Origin family and RIX3 store six-bit values already and they are copied
//!   verbatim; `.PAL` is the same six-bit palette with no header, so
//!   [`pal::decode`] copies it verbatim too; the IFF `CMAP` stores eight and the
//!   original's own converters truncate them with `/4` (`GRAB/LBM2U.C:170` and
//!   four more copies of the same line). [`six_bit`] is neither of those rules:
//!   it is the inverse of the engine's 6→8 display widening, kept for the
//!   widening's round trip, and running it over an already-six-bit palette would
//!   darken every colour.
//! - **A decoder never reads past its input.** Malformed and truncated input is
//!   an [`AssetError`], never a panic and never a short read.

pub mod anim;
pub mod exe;
pub mod font;
pub mod iff;
pub mod omf;
pub mod origin;
pub mod pal;
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
/// [`Format::Pal`] is the same shape of absence — 768 bytes with no header — and
/// is refused by `load` for a different reason: it has no pixels to put in an
/// [`Image`]. [`pal::decode`] is its door. [`Format::Fli`] is the tree's one
/// animation container with a magic (a `u32` size then `11 AF`, `PAM/PRAX4.FLI`)
/// and [`Format::Ani`] the one without (a record stream that starts with a run
/// length), and neither has a single picture to return: [`load_frames`] is their
/// door. [`font`] (A7) reads a strip a part draws its text from and is not a
/// container either: no `Format` covers it, because a font strip is a decoded
/// [`Image`] plus the geometry its own part's text routine states. [`Format::Omf`] is not an image format
/// at all: it is the build container [`omf::extract_ledata`] reads, never
/// returned by [`sniff`] (an OMF file opens with a record type, not a magic) and
/// refused by [`load`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Origin,
    OriginBare,
    Iff,
    Rix3,
    Pal,
    Fli,
    Ani,
    Omf,
    /// The vector engine's four packed scene formats — `.00M`, `.0AA`, `.0AB`
    /// and `.00N`. None has a magic, so [`sniff`] never returns this;
    /// [`visu`]'s four readers are the doors.
    Visu,
    /// A DOS MZ executable — the shipped parts, packed. [`exe::unpack`] is its door.
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
            Format::Exe => "DOS executable",
        })
    }
}

#[derive(Debug)]
pub enum AssetError {
    /// Nothing in the first bytes matched a known magic.
    UnknownFormat,
    /// The bytes stop short of what the container promises.
    ///
    /// Both fields are file offsets into the input: `needed` is the byte the
    /// decoder wanted and `had` the last byte it had to work with. `had` is
    /// sometimes *more* than `needed` — the same variant reports a buffer that
    /// is not the length the geometry requires, in either direction — and it is
    /// the length of the input only when the payload runs to the end of it. An
    /// IFF picture is the case where it does not: its rows come from the `BODY`
    /// chunk, so a short row stream reports that chunk's end, which can be
    /// several bytes inside the file (`sr_assets::iff`'s module docs).
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
    // all: they are loaded by construction, never sniffed. A `.PAL` is *exactly*
    // 768 bytes, every one of the five in the tree — a "768 bytes, so it is a
    // palette" branch would claim any file of that length, which is why there is
    // none.
    None
}

/// Decode `bytes` as `format`. [`Format::Origin`] reads its own magic to choose
/// between the family's five variants; [`Format::Iff`] reads its `FORM` type the
/// same way, between `ILBM` and `PBM `; [`Format::Rix3`] reads its 10-byte header
/// and the two type bytes in it.
///
/// The two formats with no magic cannot be reached here: [`Format::OriginBare`]
/// has no geometry to read, and [`Format::Pal`] has no pixels to return. Both
/// name their own door in the error — [`raw_surface`] and [`pal::decode`].
///
/// The two animations are refused for a reason of their own: they are 42
/// pictures, not one, and returning the first would be a silent guess about what
/// the caller wanted. Both say so and name [`load_frames`].
/// [`Format::Omf`] is refused for good, because a blob's payload is not a picture
/// — [`omf::extract_ledata`] is its door.
pub fn load(bytes: &[u8], format: Format) -> Result<Image, AssetError> {
    match format {
        Format::Origin => origin::decode(bytes),
        Format::Iff => iff::decode(bytes),
        Format::Rix3 => rix::decode(bytes),
        Format::Pal => Err(AssetError::Unsupported {
            format,
            what: "palette: pal::decode(bytes) returns it; an Image has pixels and a .PAL has none"
                .to_string(),
        }),
        Format::OriginBare => Err(AssetError::Unsupported {
            format,
            what: "no header, so no geometry: raw_surface(bytes, width, height) is its door"
                .to_string(),
        }),
        Format::Fli | Format::Ani => Err(AssetError::Unsupported {
            format,
            what: "an animation, not a picture: load_frames(bytes, format) is its door".to_string(),
        }),
        Format::Omf => Err(AssetError::Unsupported {
            format,
            what: "a build container, not a picture: omf::extract_ledata(bytes) is its door"
                .to_string(),
        }),
        Format::Visu => Err(AssetError::Unsupported {
            format,
            what: "scene data, not a picture: visu::read_master, read_scene_list, \
                   read_animation and read_object are its doors"
                .to_string(),
        }),
        Format::Exe => Err(AssetError::Unsupported {
            format,
            what: "a program, not a picture: exe::unpack(bytes) is its door".to_string(),
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

/// Every frame in `bytes`. A still container is one frame — the Origin family,
/// IFF and RIX3 — and the two animations are decoded to all of theirs: the FLI's
/// record chain ([`anim::decode_fli`], 42 frames in the tree's file) and the
/// `.ANI`'s record stream ([`anim::decode_ani`], the same 42). A trailing
/// partial frame is [`AssetError::Truncated`], never a short list.
pub fn load_frames(bytes: &[u8], format: Format) -> Result<Vec<Image>, AssetError> {
    match format {
        Format::Origin | Format::Iff | Format::Rix3 => Ok(vec![load(bytes, format)?]),
        Format::Fli => anim::decode_fli(bytes),
        Format::Ani => anim::decode_ani(bytes),
        Format::Visu => Err(AssetError::Unsupported {
            format,
            what: "scene data, not a container of frames: visu::read_animation(bytes, conum) is \
                   its door"
                .to_string(),
        }),
        _ => Err(AssetError::Unsupported {
            format,
            what: "not a container of frames: pal::decode, raw_surface and omf::extract_ledata \
                   are the doors for a palette, a bare dump and a build container"
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
/// **This is the display rule's inverse, not the 8→6 conversion rule.** The
/// engine widens six bits to eight for output, and this is the value that comes
/// back; a loader handed eight-bit palette data converts it the way the
/// original's own tools did, which for the IFF `CMAP` is the integer `/4` of
/// `GRAB/LBM2U.C:170` — not this. The two are different functions: they disagree
/// on 42 of the 256 values here (every `4k+3` up to 83, and every fourth value
/// from 172 to 252), which is one 6-bit step on each of the tree's `.UH`,
/// `.UP`, `.16` and `.EXB` palettes that a `/4` produced.
///
/// Nor is it the identity over six-bit input — `six_bit(32)` is 8 — so it must
/// not be run over a palette that is six bits already, which is what every
/// Origin family file's, every RIX3 file's and every `.PAL`'s is.
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
