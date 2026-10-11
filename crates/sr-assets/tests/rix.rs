//! RIX3 — the container behind `.CLX` and `.SCI` — pinned against the files the
//! demo shipped and the parts that read them.
//!
//! Every file these tests read is a curated copy under `assets/`, verified
//! byte-for-byte against `upstream/SecondReality/` at commit `071a82e` by task
//! A1. The tree holds no writer for the format (it is an outside paint program's
//! — ColoRIX), so the layout is established from the files and from the parts
//! that read them, and the parts never read a single header field:
//!
//! - `WATER/KOE.PAS:35-40` — `assign(f,'bkg.clx')`, `seek(f,10)`,
//!   `blockread(f,pal,768)`, `setrgb(x,pal[x*3+0],pal[x*3+1],pal[x*3+2])` (which
//!   writes ports `0x3C8`/`0x3C9`, the DAC), then `blockread(f,mem[$a000:0],
//!   64000)` straight to the screen. Palette at 10, pixels at 778, one byte per
//!   pixel, row-major, and both offsets are the only two the part asks for.
//! - `FOREST/READ2.PAS:110-121` — `seek(f,x*640+778)` per row of `o2.sci`, and
//!   `move(mem[seg(hback):ofs(hback)+10],pal,768)` for the `hillback.clx` the
//!   linker put at `hback` (`FOREST/MAKEOB.BAT:1`).
//! - `WATER/DEMO.PAS:108-109` — `move(mem[seg(_miekka):ofs(_miekka)+10],pal,768)`
//!   and `move(…+778, font, 400*34)`.
//!
//! Header, 10 bytes, and the parts confirm the second half of it: `"RIX3"`, a
//! little-endian `u16` width, a little-endian `u16` height, one byte of palette
//! type (`0xAF`, a VGA DAC palette — which is what `KOE.PAS`'s `setrgb` writes
//! the 768 bytes as) and one byte of storage type (`0x00`, linear — which is what
//! `blockread` of 64000 bytes assumes). The tree's 11 RIX3 files all carry
//! `AF 00`, and the two bytes are two fields, not one `u16`: the file arithmetic
//! is 778 = 10 + 768, and a 12-byte header would put the palette at 12, where
//! every one of the 11 files has the first palette entry instead.
//!
//! **The palette is six bits and is copied verbatim** — the same rule as the
//! Origin family, and not one byte of the 6→8 display widening. The parts write
//! those bytes straight to the DAC; and three of the curated palettes are the
//! original's own `/4` of the `CMAP` of a picture beside them (measured against
//! `FOREST/FINAL.LBM`, `WATER/FINAL2.LBM`, `FOREST/FONA2.LBM` and
//! `WATER/SWORD.LBM`, 768 bytes of 768, all four of them upstream). The rule is
//! pinned below against the files' own bytes.
//!
//! **The payload is whole rows of the header's width**, and that is the one place
//! a file in the tree disagrees with its own header: `WATER/MIEKKA.SCI` declares a
//! 400x250 canvas and carries 35 rows of it (14,000 bytes), because it is the top
//! of the sword screen (`WATER/SWORD.SCI`) cut down for the part that reads it
//! (`WATER/DEMO.PAS:109` reads 400*34 of those bytes). Everything else agrees
//! exactly. See `every_rix3_asset_agrees_with_its_own_header`.

use std::path::{Path, PathBuf};

use rstest::rstest;
use sr_assets::{load, load_frames, load_path, six_bit, sniff, AssetError, Format};
use sr_engine::MAX_WIDTH;

/// Every file in the curated tree that *is* a RIX3 file, by the magic — not by
/// extension: the tree ships them as `.CLX` and `.SCI`, and the extension never
/// decides a format in this crate.
fn rix3_assets_in(root: &str) -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display())) {
            let path = entry.expect("directory entry").path();
            if path.is_dir() {
                walk(&path, out);
            } else if std::fs::read(&path)
                .map(|b| b.starts_with(b"RIX3"))
                .unwrap_or(false)
            {
                out.push(path);
            }
        }
    }

    let mut out = Vec::new();
    walk(Path::new(root), &mut out);
    out.sort();
    out
}

/// The geometry and the two type bytes a file's own header carries, read here and
/// not by the decoder: little-endian `u16`s at 4 and 6, then the palette and
/// storage bytes. What makes the tree-wide test worth running is that this and
/// the decoder can disagree.
fn header_of(file: &[u8]) -> (u16, u16, u8, u8) {
    assert!(file.len() >= 10, "a RIX3 file has a 10-byte header");
    (
        u16::from_le_bytes([file[4], file[5]]),
        u16::from_le_bytes([file[6], file[7]]),
        file[8],
        file[9],
    )
}

const PIXELS_AT: usize = 778;

/// The brief's first RIX3 test: the header read off each file decides the
/// geometry, and the file's own pixels come out of the pixels' offset.
#[rstest]
#[case("../../assets/WATER/BKG.CLX", 320, 200)] // header read off the file
#[case("../../assets/FOREST/O2.SCI", 640, 32)] // header read off the file
#[case("../../assets/FOREST/HILLBACK.CLX", 320, 200)]
fn rix3_decodes_a_screen(#[case] path: &str, #[case] w: u16, #[case] h: u16) {
    let raw = std::fs::read(path).unwrap();
    assert_eq!(header_of(&raw).0, w, "{path}: the file's own width field");
    assert_eq!(header_of(&raw).1, h, "{path}: the file's own height field");
    let img = load_path(Path::new(path)).unwrap_or_else(|e| panic!("{path}: {e}"));
    assert_eq!((img.width(), img.height()), (w, h));
    assert_eq!(
        img.pixels(),
        &raw[PIXELS_AT..],
        "{path}: the file's own pixels"
    );
}

/// The property A2 and A3 run, in the form this family's one cropped file
/// forces: every RIX3 file in the tree decodes, its width is the header's width,
/// and its height is the header's height wherever the payload is the canvas the
/// header claims.
///
/// The exception is measured rather than assumed, and named: `WATER/MIEKKA.SCI`
/// declares 400x250 and carries 14,000 bytes, which is 35 whole rows of 400. Its
/// payload is the first 14,000 pixel bytes of `WATER/SWORD.SCI` and its palette
/// is `SWORD.LBM`'s `CMAP` divided by four (`WATER/DEMO.PAS:109` reads 400*34 of
/// them into a font strip). The decoder decodes those 35 rows rather than
/// refusing the file — a decoder that refused it would refuse a file the shipped
/// part links and reads (`WATER/MAKEOB.BAT:2`, and `MIEKKA.OBJ`'s `LEDATA`
/// payload is this file byte for byte) — and rather than padding it to 250 rows
/// with pixels the file does not contain. `pixels()` is the file's own bytes
/// either way; the geometry is not invented.
#[test]
fn every_rix3_asset_agrees_with_its_own_header() {
    let mut seen = 0;
    let mut full = 0;
    let mut cropped = 0;
    for path in rix3_assets_in("../../assets") {
        let raw = std::fs::read(&path).unwrap();
        let (w, h, palette_type, storage) = header_of(&raw);
        assert_eq!(palette_type, 0xAF, "{}: not a VGA palette", path.display());
        assert_eq!(storage, 0x00, "{}: not linear storage", path.display());
        assert!(raw.len() >= PIXELS_AT, "{}: no pixels", path.display());

        let img = load(&raw, Format::Rix3).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let payload = raw.len() - PIXELS_AT;
        assert_eq!(img.width(), w, "{}: the header's width", path.display());
        assert!(
            payload % w as usize == 0 && payload <= w as usize * h as usize,
            "{}: {payload} pixel bytes is not whole rows of {w} within {w}x{h}",
            path.display()
        );
        assert_eq!(
            img.height(),
            (payload / w as usize) as u16,
            "{}: the rows the file carries",
            path.display()
        );
        assert_eq!(
            img.pixels().len(),
            img.width() as usize * img.height() as usize,
            "{}: decoded pixels vs decoded geometry",
            path.display()
        );
        // The bytes, not just their count: a decoder that padded, reordered or
        // zero-filled a row would pass every assertion above.
        assert_eq!(
            img.pixels(),
            &raw[PIXELS_AT..],
            "{}: decodes to its own bytes",
            path.display()
        );

        if payload == w as usize * h as usize {
            assert_eq!(
                img.height(),
                h,
                "{}: a full payload fills its canvas",
                path.display()
            );
            full += 1;
        } else {
            assert!(
                path.to_string_lossy().ends_with("WATER/MIEKKA.SCI"),
                "{}: the tree's only RIX3 file that does not fill its canvas",
                path.display()
            );
            cropped += 1;
        }
        seen += 1;
    }
    assert_eq!(seen, 4, "the curated tree carries four RIX3 files");
    assert_eq!(
        (full, cropped),
        (3, 1),
        "three full canvases and the one measured crop"
    );
}

/// The one exception, on its own, with the numbers the geometry rule rests on:
/// `WATER/MIEKKA.SCI` is a 35-row strip of its 400x250 canvas, and the 13,600
/// bytes the shipped part reads from offset 778 fit inside it.
#[test]
fn the_cropped_sword_strip_carries_the_rows_the_part_reads() {
    let raw = std::fs::read("../../assets/WATER/MIEKKA.SCI").unwrap();
    assert_eq!(raw.len(), 14_778, "the file's own length");
    assert_eq!(header_of(&raw), (400, 250, 0xAF, 0x00), "a 400x250 canvas");

    // 14,778 - 778 = 14,000 = 35 rows of 400: the payload stops on a row boundary,
    // which is why it is a crop and not a truncation (a cut lands there once in
    // 400 bytes), and it is the top of the sword screen — the same 14,000 bytes
    // `WATER/SWORD.SCI` carries first, with the same palette.
    let payload = raw.len() - PIXELS_AT;
    assert_eq!(payload, 14_000);
    assert_eq!(payload % 400, 0);

    let img = load(&raw, Format::Rix3).unwrap();
    assert_eq!(
        (img.width(), img.height()),
        (400, 35),
        "the rows the file carries"
    );
    assert_eq!(img.pixels(), &raw[PIXELS_AT..]);
    // `WATER/DEMO.PAS:109` reads 400*34 bytes from there into `font`.
    assert!(400 * 34 <= img.pixels().len(), "the part's read has to fit");
}

/// The palette rule, per file: the decoded palette is the file's own 768 bytes at
/// offset 10, verbatim, and those bytes are six bits. The equality is the
/// assertion with teeth — a needless `six_bit` leaves every component `<= 63` and
/// would still move 682, 765, 652 and 710 of the 768 bytes (asserted per file).
#[test]
fn the_palette_is_the_files_own_six_bit_bytes() {
    let mut seen = 0;
    for path in rix3_assets_in("../../assets") {
        let raw = std::fs::read(&path).unwrap();
        let img = load(&raw, Format::Rix3).unwrap();
        for i in 0..256usize {
            let stored = [raw[10 + i * 3], raw[10 + i * 3 + 1], raw[10 + i * 3 + 2]];
            assert_eq!(
                img.palette.entry(i as u8),
                stored,
                "{}: entry {i}",
                path.display()
            );
            assert!(
                stored.iter().all(|&c| c <= 63),
                "{}: entry {i} is not six bits",
                path.display()
            );
        }
        let moved = raw[10..778].iter().filter(|&&v| six_bit(v) != v).count();
        assert!(
            moved > 600,
            "{}: six_bit changes only {moved} of 768 palette bytes, so this file cannot tell the \
             two rules apart",
            path.display()
        );
        seen += 1;
    }
    assert_eq!(seen, 4, "the curated tree carries four RIX3 files");
}

/// The same refusal as the Origin family's: a byte above 63 is not a six-bit DAC
/// value, and `Palette::set_entry` would mask it silently.
#[test]
fn a_palette_byte_above_six_bits_is_refused_not_clamped() {
    let mut raw = std::fs::read("../../assets/WATER/BKG.CLX").unwrap();
    raw[10 + 3] = 64;
    assert!(matches!(
        load(&raw, Format::Rix3),
        Err(AssetError::Palette { .. })
    ));
}

/// The two type bytes, measured: all 11 RIX3 files in either tree carry `0xAF`
/// (a VGA palette) and `0x00` (linear storage), and the parts' own reads depend
/// on exactly that — `setrgb` writes the palette to the DAC, and a raw
/// `blockread` is what linear storage means. Anything else is a palette or a
/// storage this decoder cannot read, and is refused rather than guessed at.
#[rstest]
#[case(8, 0x00, "not a VGA palette")]
#[case(8, 0x8F, "not a VGA palette")]
#[case(9, 0x80, "compressed storage")]
#[case(9, 0x40, "an extension block")]
#[case(9, 0x01, "planar storage")]
fn a_header_type_byte_this_decoder_cannot_read_is_refused(
    #[case] at: usize,
    #[case] value: u8,
    #[case] what: &str,
) {
    let mut raw = std::fs::read("../../assets/WATER/BKG.CLX").unwrap();
    raw[at] = value;
    let err = load(&raw, Format::Rix3).unwrap_err();
    assert!(
        matches!(
            err,
            AssetError::Unsupported {
                format: Format::Rix3,
                ..
            }
        ),
        "{what}: {err}"
    );
}

/// The brief's third RIX3 test, with the offsets spelled out: the payload is
/// whole rows of the header's width, so a file that stops part-way through a row
/// is a truncation and not a smaller picture, and it names the byte it wanted
/// (the end of the incomplete row) and the last byte it had.
#[test]
fn a_rix3_file_whose_pixels_stop_early_is_refused() {
    let raw = std::fs::read("../../assets/WATER/BKG.CLX").unwrap();
    assert_eq!(raw.len(), 64_778);
    // 100 bytes short of the end: 63,900 pixel bytes, which is 199 rows (63,680
    // bytes) and 220 bytes of a 200th. The row that stops short is row 199, so
    // the decoder wants byte 778 + 200*320 = 64,778.
    let cut = raw.len() - 100;
    let err = load(&raw[..cut], Format::Rix3).unwrap_err();
    assert!(
        matches!(
            err,
            AssetError::Truncated {
                format: Format::Rix3,
                needed: 64_778,
                had: 64_678
            }
        ),
        "{err}"
    );
}

/// A cut inside the palette wants the palette's end, not the canvas's.
#[test]
fn a_rix3_file_that_stops_inside_its_palette_is_refused() {
    let raw = std::fs::read("../../assets/WATER/BKG.CLX").unwrap();
    let err = load(&raw[..500], Format::Rix3).unwrap_err();
    assert!(
        matches!(
            err,
            AssetError::Truncated {
                format: Format::Rix3,
                needed: 778,
                had: 500
            }
        ),
        "{err}"
    );
}

/// And a cut inside the 10-byte header wants the header.
#[test]
fn a_rix3_file_that_stops_inside_its_header_is_refused() {
    let raw = std::fs::read("../../assets/WATER/BKG.CLX").unwrap();
    let err = load(&raw[..7], Format::Rix3).unwrap_err();
    assert!(
        matches!(
            err,
            AssetError::Truncated {
                format: Format::Rix3,
                needed: 10,
                had: 7
            }
        ),
        "{err}"
    );
}

/// The other direction of the same rule: bytes past the canvas are not a bigger
/// picture, because the header is what says how big the canvas is. Ten of the 11
/// RIX3 files in the tree are exactly `10 + 768 + w*h`; the eleventh is short,
/// none is long.
#[test]
fn a_rix3_file_with_bytes_past_its_canvas_is_refused() {
    let mut raw = std::fs::read("../../assets/WATER/BKG.CLX").unwrap();
    raw.push(0);
    let err = load(&raw, Format::Rix3).unwrap_err();
    assert!(
        matches!(
            err,
            AssetError::Truncated {
                format: Format::Rix3,
                needed: 64_778,
                had: 64_779
            }
        ),
        "{err}"
    );
}

/// A canvas the engine's largest surface could not hold is refused before
/// anything is allocated, as in the other two decoders.
#[test]
fn an_oversized_canvas_is_refused_rather_than_clipped() {
    // 1024x400 is over the engine's area bound only while 1024 is wider than the
    // widest surface it allocates; the literal below is what the test asserts.
    const { assert!(MAX_WIDTH < 1024) };
    let mut raw = std::fs::read("../../assets/WATER/BKG.CLX").unwrap();
    raw[4..6].copy_from_slice(&1024u16.to_le_bytes());
    raw[6..8].copy_from_slice(&400u16.to_le_bytes());
    assert!(matches!(
        load(&raw, Format::Rix3),
        Err(AssetError::TooLarge {
            width: 1024,
            height: 400
        })
    ));
}

/// A buffer that is not RIX3 at all: `load` refuses it by magic exactly as the
/// other two decoders do, and `sniff` never offered it in the first place.
#[test]
fn a_file_that_is_not_rix3_is_refused() {
    let mut bytes = std::fs::read("../../assets/WATER/BKG.CLX").unwrap();
    bytes[0..4].copy_from_slice(b"RIX4");
    assert_eq!(sniff(&bytes), None);
    assert!(matches!(
        load(&bytes, Format::Rix3),
        Err(AssetError::UnknownFormat)
    ));

    // The magic alone is enough to sniff, and not enough to load.
    assert_eq!(sniff(b"RIX3"), Some(Format::Rix3));
    assert!(matches!(
        load(b"RIX3", Format::Rix3),
        Err(AssetError::Truncated {
            format: Format::Rix3,
            needed: 10,
            had: 4
        })
    ));
}

/// The plan's "never reads past its input", on this family: every prefix of every
/// RIX3 file in the tree is either refused or decodes to whole rows of the file's
/// own bytes — never a padded row, never a panic, and never a picture the file
/// does not contain. A prefix that stops mid-row is refused; one that stops on a
/// row boundary is a smaller picture in the same format, which is the one legal
/// way a cut decodes (A2's bit-plane case is the same shape).
#[test]
fn no_prefix_of_a_rix3_asset_decodes_to_anything_but_the_files_own_rows() {
    let mut checked = 0;
    let mut refused = 0;
    let mut decoded = 0;
    for path in rix3_assets_in("../../assets") {
        let raw = std::fs::read(&path).unwrap();
        let (w, h, ..) = header_of(&raw);
        let step = (raw.len() / 200).max(1);
        for cut in (0..raw.len()).step_by(step) {
            match load(&raw[..cut], Format::Rix3) {
                Ok(img) => {
                    let payload = cut - PIXELS_AT;
                    assert_eq!(img.width(), w, "{} cut to {cut}", path.display());
                    assert_eq!(
                        payload % w as usize,
                        0,
                        "{} cut to {cut}: a partial row decoded",
                        path.display()
                    );
                    assert_eq!(img.height(), (payload / w as usize) as u16);
                    assert!(payload <= w as usize * h as usize);
                    assert_eq!(
                        img.pixels(),
                        &raw[PIXELS_AT..cut],
                        "{} cut to {cut}",
                        path.display()
                    );
                    decoded += 1;
                }
                Err(_) => refused += 1,
            }
            checked += 1;
        }
    }
    assert!(checked > 700, "expected a lot of cuts, made {checked}");
    // The cuts that land on a row boundary are the only ones that decode, and
    // there are some: a sweep where nothing decodes would prove nothing.
    assert!(
        decoded > 0,
        "no cut decoded at all, so the sweep is vacuous"
    );
    // A 320- or 640-wide file is cut mid-row almost every time.
    assert!(
        refused * 10 > checked * 9,
        "only {refused} of {checked} cuts refused"
    );
}

/// A still container is one frame, as with the Origin family and IFF; A6 adds
/// the two formats that carry more than one.
#[test]
fn a_rix3_screen_loads_as_one_frame() {
    let bytes = std::fs::read("../../assets/FOREST/O2.SCI").unwrap();
    let frames = load_frames(&bytes, Format::Rix3).unwrap();
    assert_eq!(frames.len(), 1);
    assert_eq!((frames[0].width(), frames[0].height()), (640, 32));
}
