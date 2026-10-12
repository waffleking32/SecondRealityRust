//! `.PAL` — 768 bytes of VGA DAC palette, no header and no magic — pinned
//! against the files the demo shipped.
//!
//! Every file these tests read is a curated copy under `assets/`, verified
//! byte-for-byte against `upstream/SecondReality/` at commit `071a82e` by task
//! A1. The format has no magic, so it is established from the parts that read it
//! and from what the bytes are:
//!
//! - `PAM/ANIM.C:19-21` — `fopen("pal.pal","rb")`, `fread(pal,1,768,data)`,
//!   `tw_setpalette(pal)`: the 768 bytes go to the DAC as they are.
//! - `VISU/C/MAIN.C:234-241` — `fread(matpal,768,1,in)` for `<scene>.PAL`, then
//!   `fwrite(matpal,768,1,outb3)` into the `.00M` (`:262`) whose offset 16 the
//!   parts read as the material palette (`VISU/C/U2A.C:243`, written to the DAC
//!   at `:245-246`), and `vid_setpal`
//!   at `:386`.
//! - `PANIC/SHUTDOWN.C:41-42` — `read(fff,kuvapal,768)`, and the fade table two
//!   lines later (`:62`) computes `(a*63 + kuvapal[b]*(64-a))/64` — arithmetic
//!   over 0..63, i.e. the bytes are six-bit DAC values.
//!
//! **So a `.PAL` is copied verbatim, and the 6-bit rule is not re-applied.** The
//! conversion an 8-bit palette needs is the original's `/4` (`GRAB/LBM2U.C:170`),
//! and the `.PAL` files were written *after* it: three of the five curated
//! palettes are the `/4` of the `CMAP` of the picture they belong to, byte for
//! byte (below). `six_bit` — the inverse of the engine's *display* widening —
//! would divide them by four a second time, and
//! `Palette::from_vga`'s `>> 2` would do the same; both are the 8→6 rule and
//! neither may run here. The failure is invisible to a `<= 63` assertion, which
//! is why the tests below compare the entries with the file's own bytes and with
//! the picture's `CMAP`.

use std::path::{Path, PathBuf};

use rstest::rstest;
use sr_assets::{load, pal, six_bit, sniff, AssetError, Format};

/// Every file in the curated tree shipped under the `.PAL` extension. The
/// extension is the only thing to go on — the format has no magic — so this is
/// one of the two paths in the crate where a name decides what a file is, and
/// `sniff` must keep answering `None` for all of them.
fn pal_assets_in(root: &str) -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display())) {
            let path = entry.expect("directory entry").path();
            if path.is_dir() {
                walk(&path, out);
            } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if ext.eq_ignore_ascii_case("PAL") {
                    out.push(path);
                }
            }
        }
    }

    let mut out = Vec::new();
    walk(Path::new(root), &mut out);
    out.sort();
    out
}

/// A `.PAL`'s bytes as the 256 entries a decoder has to produce: three bytes
/// each, entry 0 first, and nothing else in the file.
fn entries_of(bytes: &[u8]) -> Vec<[u8; 3]> {
    assert_eq!(bytes.len(), 768, "a .PAL is 768 bytes");
    bytes.chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect()
}

/// The payload of an IFF file's `CMAP` chunk, walked the way A3's decoder walks
/// it, so a `.PAL` can be compared with the eight-bit palette it was made from.
/// Deliberately a test-local walk: if the decoder's own walk were wrong, this one
/// would have to be wrong the same way to hide it.
fn cmap_of(file: &[u8]) -> &[u8] {
    let mut at = 12;
    while at + 8 <= file.len() {
        let size =
            u32::from_be_bytes([file[at + 4], file[at + 5], file[at + 6], file[at + 7]]) as usize;
        if &file[at..at + 4] == b"CMAP" {
            return &file[at + 8..at + 8 + size];
        }
        at += 8 + size + (size & 1);
    }
    panic!("no CMAP chunk in this file");
}

/// The brief's first `.PAL` test: 256 entries, every component six bits.
#[test]
fn a_pal_file_is_256_entries_of_six_bits() {
    let p = pal::decode(&std::fs::read("../../assets/PAM/PAL.PAL").unwrap()).unwrap();
    for i in 0..=255u8 {
        assert!(p.entry(i).iter().all(|&c| c <= 63), "entry {i}");
    }
    assert_ne!(p.entry(0), p.entry(255), "a palette that was never read");
}

/// The brief's second: a `.PAL` that is not 768 bytes is refused, and the error
/// names the length it wanted and the length it had — both file offsets, as
/// `AssetError::Truncated` documents. A longer file is refused too: the palette
/// is all 768 bytes of the file, so a 769th byte is not part of it.
#[rstest]
#[case(0, 768, 0)]
#[case(767, 768, 767)]
#[case(768 + 1, 768, 769)]
fn a_pal_file_that_is_not_768_bytes_is_refused(
    #[case] len: usize,
    #[case] needed: usize,
    #[case] had: usize,
) {
    let err = pal::decode(&vec![0u8; len]).unwrap_err();
    assert!(
        matches!(
            err,
            AssetError::Truncated { format: Format::Pal, needed: n, had: h } if n == needed && h == had
        ),
        "{err}"
    );
}

/// The rule, per file: every entry is the file's own three bytes, and the file's
/// own bytes are six bits. Both halves matter — a needless `six_bit` over all 768
/// bytes would still leave every component `<= 63`, so only the equality has
/// teeth. It would move 564..761 of these bytes (asserted per file, at least
/// 300), and `>> 2` would move nearly all of them.
#[test]
fn every_pal_asset_is_its_own_bytes_and_they_are_six_bit() {
    let mut seen = 0;
    for path in pal_assets_in("../../assets") {
        let raw = std::fs::read(&path).unwrap();
        assert_eq!(raw.len(), 768, "{}: a .PAL is 768 bytes", path.display());
        let p = pal::decode(&raw).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        for (i, want) in entries_of(&raw).into_iter().enumerate() {
            assert_eq!(p.entry(i as u8), want, "{}: entry {i}", path.display());
            assert!(
                want.iter().all(|&c| c <= 63),
                "{}: entry {i} is not six bits",
                path.display()
            );
        }
        // The rule has to be distinguishable: the display-rule inverse would move
        // most of these bytes.
        let moved = raw.iter().filter(|&&v| six_bit(v) != v).count();
        assert!(
            moved > 300,
            "{}: six_bit changes only {moved} of 768 bytes, so this file cannot tell the two \
             rules apart",
            path.display()
        );
        seen += 1;
    }
    assert_eq!(seen, 5, "the curated tree carries five .PAL files");
}

/// The cross-check that settles the bit width, through the crate's own two doors:
/// three curated `.PAL` files are the `/4` of the `CMAP` of the picture beside
/// them, all 768 bytes of each. `six_bit` would differ on 92, 282 and 234 bytes
/// respectively, and `/4` on none — and each case asserts its own discrimination
/// count, so a case that stopped telling the two rules apart fails rather than
/// passes.
///
/// `HZPIC.PAL` is the setup menu's palette, `U2E.PAL`/`U2A.PAL` are the scene
/// palettes the `VISU/C` viewer reads (`VISU/C/MAIN.C:234`).
#[rstest]
#[case("../../assets/START/HZPIC.PAL", "../../assets/START/HZPIC.LBM", 92)]
#[case(
    "../../assets/VISU/C/SCENE/U2E.PAL",
    "../../assets/VISU/C/SCENE/U2E.LBM",
    282
)]
#[case(
    "../../assets/VISU/C/U2A.PAL",
    "../../assets/VISU/C/SCENE/U2A.LBM",
    234
)]
fn a_pal_file_is_the_originals_truncation_of_the_pictures_cmap(
    #[case] pal_path: &str,
    #[case] lbm_path: &str,
    #[case] six_bit_would_move: usize,
) {
    let pal_bytes = std::fs::read(pal_path).unwrap();
    let lbm_bytes = std::fs::read(lbm_path).unwrap();
    let cmap = cmap_of(&lbm_bytes);
    assert_eq!(
        cmap.len(),
        768,
        "{lbm_path}: a full CMAP for this comparison"
    );

    let p = pal::decode(&pal_bytes).unwrap();
    let moved = (0..768)
        .filter(|&i| six_bit(cmap[i]) != cmap[i] / 4)
        .count();
    assert_eq!(
        moved, six_bit_would_move,
        "{pal_path}: the two conversion rules must differ on this many bytes for the case to mean \
         anything"
    );
    for i in 0..256usize {
        let want = [cmap[i * 3] / 4, cmap[i * 3 + 1] / 4, cmap[i * 3 + 2] / 4];
        assert_eq!(
            p.entry(i as u8),
            want,
            "{pal_path}: entry {i} is not the picture's CMAP divided by four"
        );
    }

    // And the door into the same picture's palette agrees with the `.PAL` door:
    // A3's IFF decoder converts the CMAP itself, so this compares two loaders
    // rather than one loader with a test-local expectation.
    let img = load(&lbm_bytes, Format::Iff).unwrap();
    assert_eq!(img.palette, p, "{pal_path}: the .PAL and the .LBM disagree");
}

/// The Origin family's rule, applied to this family: a byte above 63 is out of
/// contract (`Palette::set_entry` masks with `& 0x3F`, so an unchecked decode
/// would silently change the colour), and every channel is checked.
#[rstest]
#[case(0)]
#[case(1)]
#[case(2)]
fn a_palette_byte_above_six_bits_is_refused_not_clamped(#[case] channel: usize) {
    let mut bytes = vec![0u8; 768];
    bytes[7 * 3 + channel] = 64;
    assert!(
        matches!(pal::decode(&bytes), Err(AssetError::Palette { .. })),
        "a 64 in channel {channel} is not a six-bit value"
    );
}

/// All three components of one entry, and the entry itself, in the message: a
/// `Palette` error that named the wrong byte would send a reader to the wrong
/// place in a 768-byte file.
#[test]
fn a_refused_palette_names_the_offending_entry() {
    let mut bytes = vec![0u8; 768];
    bytes[100 * 3 + 1] = 200;
    let err = pal::decode(&bytes).unwrap_err();
    let AssetError::Palette { what } = &err else {
        panic!("expected a Palette error, got {err}");
    };
    assert!(what.contains("100"), "the entry number is missing: {what}");
    assert!(what.contains("200"), "the value is missing: {what}");
}

/// The ruling this task turns on: **`sniff` never returns `Format::Pal`.** A
/// `.PAL` has no magic, and all five curated ones are exactly 768 bytes — so
/// their size identifies nothing, and a size-based branch would hand any
/// 768-byte file to the palette decoder. `load` therefore cannot be reached for
/// a `.PAL` by sniffling; `pal::decode` is the door, and `load` says so.
#[test]
fn a_pal_is_loaded_by_construction_and_never_sniffed() {
    let mut seen = 0;
    for path in pal_assets_in("../../assets") {
        let raw = std::fs::read(&path).unwrap();
        assert_eq!(sniff(&raw), None, "{}: a .PAL has no magic", path.display());
        let err = load(&raw, Format::Pal).unwrap_err();
        assert!(
            matches!(err, AssetError::Unsupported { format: Format::Pal, ref what } if what.contains("pal::decode")),
            "{}: {err}",
            path.display()
        );
        seen += 1;
    }
    assert_eq!(seen, 5, "the curated tree carries five .PAL files");

    // A 768-byte file of zeroes is the case a size-based sniff would get wrong:
    // it is a valid palette and it is also what a truncated anything looks like.
    assert_eq!(sniff(&vec![0u8; 768]), None);
    assert!(pal::decode(&vec![0u8; 768]).is_ok());

    // `sniff` answers from the magic alone, so a RIX3 file — whose own palette
    // sits at offset 10 — is Rix3 and not Pal.
    assert_eq!(
        sniff(&std::fs::read("../../assets/WATER/BKG.CLX").unwrap()),
        Some(Format::Rix3)
    );
}

/// Every prefix of a real `.PAL` is refused, and no prefix panics or reads past
/// its input. There is no partial palette to return: the file is its 768 bytes.
#[test]
fn no_prefix_of_a_pal_asset_is_accepted() {
    let mut seen = 0;
    let mut checked = 0;
    let mut refused = 0;
    for path in pal_assets_in("../../assets") {
        let raw = std::fs::read(&path).unwrap();
        for cut in (0..raw.len()).step_by(7) {
            match pal::decode(&raw[..cut]) {
                Ok(_) => panic!("{} cut to {cut} decoded as a palette", path.display()),
                Err(_) => refused += 1,
            }
            checked += 1;
        }
        seen += 1;
    }
    assert_eq!(seen, 5, "the curated tree carries five .PAL files");
    assert!(checked > 500, "expected a lot of cuts, made {checked}");
    assert_eq!(
        refused, checked,
        "every prefix is a palette of the wrong length"
    );
}
