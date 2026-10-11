//! Task A5: the OMF reader, and the tree-level checks it makes possible.
//!
//! Three things are proved here, and none of them is a decode:
//!
//! 1. **The framing.** [`extract_ledata`] strips the three-byte record header and the
//!    trailing checksum byte from every `LEDATA` (`0xA0`) record and concatenates what is
//!    left. `GLENZ/_FC.OBK` is the worked example: 64 records, 64,784 bytes that equal
//!    `GLENZ/FC.UH` byte for byte. Equality is the check, not length, because reading the
//!    header as a segment and a `u16` offset shifts every record's data one byte while
//!    yielding the same 64,784 bytes, and keeping the trailing byte gives 64,848 — both
//!    would pass a size test.
//! 2. **The tree is complete from the EXE side.** Every blob's payload is compared with
//!    the standalone file it reproduces: the inventory's §D pairs, the four `COMAN`
//!    slices, and thirteen more pairs that the same walk finds elsewhere in the tree — 35
//!    blobs in all. The walk's three exceptions are named in §D: `ALKU/INCLUDE.OBJ` (§E.18,
//!    three palette bytes from `ALKU/HOI.U`), `WATER/MIEK.OBJ` (§E.19, one byte from
//!    `MIEKKA.SCI`, unlinked) and the two `CREDITS` picture blobs (§E.20, 21 pictures no
//!    file in the tree holds).
//! 3. **The payloads with no twin are answered.** `TECHNO/_CIRCLE2.OBK` is a 320x200
//!    one-bit mask, the exact output of `DDSTARS/DOIT2.C`, linked into the shipped
//!    TECHNO.EXE (`TECHNO/MAKEFILE:6,23`, `TECHNO/KOE.MAP:10-11`) and consumed at runtime
//!    (`TECHNO/KOEA.ASM:665`) — the one asset this phase recovers, as
//!    `assets/TECHNO/CIRCLE2.EGA`. `TECHNO/_PIC1.OBK` holds an Origin RLE container that
//!    stops 119 bytes into its 282nd row (281 of the 400 its header claims) and no shipped
//!    build links it (`TECHNO/MAKEFILE:7` comments the line out); `_PIC2.OBK` is the last
//!    20,859 bytes of the curated `TECHNO/TROLL.UP`. Both are build residue.
//!
//! Every path is read from `upstream/SecondReality` at the pinned commit `071a82e`, which
//! is the source of record; the curated copies under `assets/` are checked against it by
//! §G of the inventory. The blob table this file walks is §D.

use std::path::{Path, PathBuf};

use rstest::rstest;
use sr_assets::omf::extract_ledata;
use sr_assets::{load, sniff, AssetError, Format};

/// The vendored original, at the pinned commit. Read-only: nothing here writes into it.
const UPSTREAM: &str = "../../upstream/SecondReality";

fn upstream(rel: &str) -> Vec<u8> {
    let path = format!("{UPSTREAM}/{rel}");
    std::fs::read(&path).unwrap_or_else(|e| panic!("cannot read {path}: {e}"))
}

fn asset(rel: &str) -> Vec<u8> {
    let path = format!("../../assets/{rel}");
    std::fs::read(&path).unwrap_or_else(|e| panic!("cannot read {path}: {e}"))
}

/// One blob's reconstructed payload, with the blob named in the failure.
fn payload(blob: &str) -> Vec<u8> {
    extract_ledata(&upstream(blob)).unwrap_or_else(|e| panic!("{blob}: {e}"))
}

// -------------------------------------------------------------------------------------
// The reader itself
// -------------------------------------------------------------------------------------

/// The framing test, and the one that matters. A blob whose payload has a standalone
/// twin must reproduce it **byte for byte** — equality, not a size check, because
/// treating the header's offset as a `u16` gives the right length and the wrong bytes.
/// `GLENZ/_FC.OBK` was measured at 64,784 bytes (`docs/fidelity/asset-inventory.md` §D).
#[test]
fn a_blob_reproduces_its_standalone_twin_byte_for_byte() {
    let payload = extract_ledata(&upstream("GLENZ/_FC.OBK")).unwrap();
    assert_eq!(payload.len(), 64_784, "the measured payload length");
    assert_eq!(payload, upstream("GLENZ/FC.UH"));
}

/// The byte positions, said explicitly by a one-record object. A `LEDATA` body is a
/// one-byte segment index, a `u16` little-endian data offset, the data itself and the
/// record's checksum byte — so the payload is `body[3..len-1]` and nothing else. (The
/// inventory §D describes the same three-and-one bytes as a `u16` segment and a repeated
/// offset byte; the byte positions are identical either way, and they are what this pins.)
#[test]
fn the_record_header_and_the_trailing_checksum_are_not_payload() {
    // Type 0xA0, body length 8, segment 1, offset 0x0400, four data bytes, checksum.
    let blob = [
        0xA0, 0x08, 0x00, // record header
        0x01, 0x00, 0x04, // segment index, data offset 0x0400
        b'D', b'A', b'T', b'A', // the data
        0x00, // checksum ("not used" is 0x00; 2,113 of the tree's 3,975 records say so)
    ];
    assert_eq!(extract_ledata(&blob).unwrap(), b"DATA");
}

/// A blob-only payload that holds an Origin RLE container — and a *finding*: it is not a
/// complete picture. `TECHNO/_PIC1.OBK`'s header says 320x400, 256 colours, but the stream
/// holds **281 of the 400 rows** and ends 119 bytes inside row 282 (the payload is exactly
/// 50,000 bytes — a slice-sized cut, not a write that ran out of room). So `load` refuses
/// it as truncation rather than clipping it, the same rule `COMAN/COMBG.UH` is refused
/// under (§E.2). It is also *not linked*: `TECHNO/MAKEFILE:7`'s `#_pic1.obk _pic2.obk` line
/// is commented out, so nothing shipped ever loaded what this blob carries (§E.13).
///
/// This is why the extraction test is `sniff` plus the header words rather than a decode:
/// what the extractor proves is that the payload is the container, byte for byte.
#[test]
fn a_blob_only_that_holds_a_picture_the_loader_refuses_as_truncated() {
    let payload = payload("TECHNO/_PIC1.OBK");
    assert_eq!(sniff(&payload), Some(Format::Origin));
    assert_eq!(u16::from_le_bytes([payload[2], payload[3]]), 320, "the header's width");
    assert_eq!(u16::from_le_bytes([payload[4], payload[5]]), 400, "the header's height");
    assert!(matches!(
        load(&payload, Format::Origin),
        Err(AssetError::Truncated { format: Format::Origin, needed: 50_119, had: 50_000 })
    ));
}

/// An object with no `LEDATA` record at all is not a data blob. `VISU/AMAIN.OBJ` is one
/// of the tree's measured examples: it walks cleanly and carries none.
#[test]
fn a_blob_with_no_ledata_is_refused() {
    assert!(matches!(extract_ledata(&upstream("VISU/AMAIN.OBJ")), Err(AssetError::UnknownFormat)));
    // And the empty input, which is the same statement with nothing to walk.
    assert!(matches!(extract_ledata(&[]), Err(AssetError::UnknownFormat)));
}

/// A length that overruns the blob must be an error, not a panic. The first byte is the
/// record type, the next two the little-endian length.
#[test]
fn a_record_that_runs_past_the_end_of_the_blob_is_refused() {
    assert!(matches!(
        extract_ledata(&[0xA0, 0xFF, 0xFF, 0x00]),
        Err(AssetError::Truncated { .. })
    ));
    // A tail too short to hold a record header at all is the same error.
    assert!(matches!(extract_ledata(&[0xA0, 0x00]), Err(AssetError::Truncated { .. })));
}

/// `LEDATA` is type `0xA0` in this tree and no file spells it any other way: 3,975 records
/// across the 151 `.OBK`/`.OBJ` files, not one of type `0xA1`. (In the standard Intel OMF
/// `0xA1` is `LEDATA` too, with a 32-bit offset and a longer body — a reader that treated
/// the two as the same record could not tell them apart, which is why this one refuses the
/// spelling the tree never uses.)
#[test]
fn a_record_type_this_tree_never_uses_is_refused() {
    // 0xA1, body length 4, segment 1, offset 0, checksum.
    let blob = [0xA1, 0x04, 0x00, 0x01, 0x00, 0x00, 0x00];
    assert!(matches!(
        extract_ledata(&blob),
        Err(AssetError::Unsupported { format: Format::Omf, .. })
    ));
}

/// A `LEDATA` body shorter than its own header is truncated, wherever it sits.
#[test]
fn a_ledata_body_too_short_to_hold_its_header_is_refused() {
    assert!(matches!(
        extract_ledata(&[0xA0, 0x02, 0x00, 0x01, 0x00]),
        Err(AssetError::Truncated { format: Format::Omf, .. })
    ));
}

// -------------------------------------------------------------------------------------
// Completeness: the 35 blobs whose payload is a standalone file
// -------------------------------------------------------------------------------------

/// Every blob whose payload is a standalone file, as `(blob, file)`. The first 22 rows are
/// `docs/fidelity/asset-inventory.md` §D's whole-file rows; the last 13 are pairs the same
/// walk finds elsewhere in the tree and §D does not name. `GRID/_EYE.OBK` and
/// `GLENZ/_BGPIC.OBK` both reproduce `GLENZ/EYE4.UH` (the two blobs hold identical bytes,
/// §B), and `VISU/C/_BG.OBK` also reproduces `VISU/C/PIC003.UH`, a second byte-identical
/// copy of `VISU/C/U2ABG.UH`; the canonical file is named in each pair.
const TWINS: &[(&str, &str)] = &[
    // §D's rows, in its order.
    ("BEG/_PIC.OBK", "BEG/SRTITLE.UP"),
    ("END/_PIC.OBK", "END/PIC.UH"),
    ("ENDPIC/_PIC.OBK", "ENDPIC/SRTITLE.U"),
    ("GLENZ/_FC.OBK", "GLENZ/FC.UH"),
    ("GLENZ/_BGPIC.OBK", "GLENZ/EYE4.UH"),
    ("GRID/_EYE.OBK", "GLENZ/EYE4.UH"),
    ("JPLOGO/_PIC.OBK", "JPLOGO/ICEKNGDM.UP"),
    ("VISU/C/_BG.OBK", "VISU/C/U2ABG.UH"),
    ("COMAN/_COMBG.OBK", "COMAN/COMBG.UH"),
    ("TECHNO/_CIRCLE.OBK", "DDSTARS/PIC.EGA"),
    ("DDSTARS/_TEXTPIC.OBK", "DDSTARS/TEXTS.16"),
    ("START/_HZPIC.OBJ", "START/HZPIC.P"),
    ("LENS/_LENSEXB.OBK", "LENS/LENS.EXB"),
    ("LENS/_LENSEXP.OBK", "LENS/LENS.EXP"),
    ("LENS/_LENSEX0.OBK", "LENS/LENS.EX0"),
    ("LENS/_LENSEX1.OBK", "LENS/LENS.EX1"),
    ("LENS/_LENSEX2.OBK", "LENS/LENS.EX2"),
    ("LENS/_LENSEX3.OBK", "LENS/LENS.EX3"),
    ("LENS/_LENSEX4.OBK", "LENS/LENS.EX4"),
    ("LENS/_FILLER.OBK", "LENS/FILLER"),
    ("COMAN/_W1DTA.OBJ", "COMAN/W1DTA.BIN"),
    ("COMAN/_W2DTA.OBJ", "COMAN/W2DTA.BIN"),
    // Pairs the same walk finds elsewhere: linked data objects whose payload is a
    // standalone file. They are the same claim over the rest of the tree.
    ("FOREST/HILLBACK.OBJ", "FOREST/HILLBACK.CLX"),
    ("FOREST/POS1.OBJ", "FOREST/POS1.DAT"),
    ("FOREST/POS2.OBJ", "FOREST/POS2.DAT"),
    ("FOREST/POS3.OBJ", "FOREST/POS3.DAT"),
    ("MAIN/PROT/RUN1.OBJ", "MAIN/PROT/RUN1.BIN"),
    ("TUNNELI/SINIT.OBJ", "TUNNELI/SINIT.DAT"),
    ("TUNNELI/TUNNELI.OBJ", "TUNNELI/TUNNEL.DAT"),
    ("WATER/BKG.OBJ", "WATER/BKG.CLX"),
    ("WATER/MIEKKA.OBJ", "WATER/MIEKKA.SCI"),
    ("WATER/WAT1.OBJ", "WATER/WAT1.DAT"),
    ("WATER/WAT2.OBJ", "WATER/WAT2.DAT"),
    ("WATER/WAT3.OBJ", "WATER/WAT3.DAT"),
    ("WATER/WAT4.OBJ", "WATER/WAT4.DAT"),
];

/// The phase's evidence from the EXE side: every one of these 35 blobs reproduces a
/// standalone file byte for byte, and `TWINS` is every blob in the tree that does (the
/// exceptions the walk found are not twins and are named in §D and §E.18–E.20). This is
/// the measurement turned into a check — the comparison is byte equality, because a length
/// check cannot see the one-byte drift.
#[test]
fn every_blob_with_a_twin_reproduces_it_byte_for_byte() {
    // A guard, like the loops in the sibling test files: a table that emptied itself must
    // fail rather than pass vacuously.
    assert_eq!(TWINS.len(), 35, "22 rows from §D plus 13 found by the walk");

    for (blob, twin) in TWINS {
        let payload = payload(blob);
        let file = upstream(twin);
        assert_eq!(
            payload.len(),
            file.len(),
            "{blob} reconstructs {} bytes, {twin} is {}",
            payload.len(),
            file.len()
        );
        if payload != file {
            let at = payload.iter().zip(&file).position(|(a, b)| a != b).unwrap();
            let differing = payload.iter().zip(&file).filter(|(a, b)| a != b).count();
            panic!(
                "{blob} does not reproduce {twin} byte for byte: {differing} bytes differ, \
                 first at {at} (blob {:#04x}, file {:#04x})",
                payload[at], file[at]
            );
        }
    }
}

/// The four `COMAN` half-blobs reproduce slices, not whole files: `COMAN/DOOBJS.BAT:1-2`
/// cuts `W1DTA.BIN` with `getslice … 0 32768` and `32768 32768`, and `:7-8` does the same
/// to `W2DTA.BIN`. (§D cites the second pair at `:5-6`, which is the `del` of the cut
/// halves; A5 corrected it.) §D lists them as the two halves of the file; this pins them.
#[rstest]
#[case("COMAN/_W1DTA1.OBJ", "COMAN/W1DTA.BIN", 0)]
#[case("COMAN/_W1DTA2.OBJ", "COMAN/W1DTA.BIN", 32_768)]
#[case("COMAN/_W2DTA1.OBJ", "COMAN/W2DTA.BIN", 0)]
#[case("COMAN/_W2DTA2.OBJ", "COMAN/W2DTA.BIN", 32_768)]
fn the_dta_halves_reproduce_the_slices_the_build_script_cuts(
    #[case] blob: &str,
    #[case] twin: &str,
    #[case] at: usize,
) {
    let payload = payload(blob);
    let whole = upstream(twin);
    assert_eq!(payload.len(), 32_768);
    assert_eq!(payload.as_slice(), &whole[at..at + 32_768], "{blob} vs {twin}[{at}..]");
}

// -------------------------------------------------------------------------------------
// The payloads with no twin
// -------------------------------------------------------------------------------------

/// `TECHNO/_CIRCLE2.OBK` is the one blob-only payload the shipped parts use, so it is the
/// one file this phase recovers. It is 8,000 bytes = 320x200 bits — a one-bit-per-pixel
/// mask with no header — and it is pulled into the shipped image by `TECHNO/MAKEFILE:6,23`
/// (`data=_circle2.obk`, linked into `koe.exe`) and read at runtime by
/// `init_interference` (`TECHNO/KOEA.ASM:646-745`; the load at `:665`).
#[test]
fn the_recovered_mask_is_the_shipped_blob_payload() {
    let payload = payload("TECHNO/_CIRCLE2.OBK");
    assert_eq!(payload.len(), 8_000, "= 320 * 200 / 8, one bit per pixel");
    assert_eq!(asset("TECHNO/CIRCLE2.EGA"), payload);
}

/// The mask is exactly what the generator in the tree writes: the loop at
/// `DDSTARS/DOIT2.C:21-52` (`r = sqrt((x-320)^2+(y-200)^2)`, `a = (r/12)&1`, one plane,
/// eight pixels per byte, high bit first) reproduces all 8,000 bytes. That is what makes
/// the recovered file's name and its provenance row right — and it is *not*
/// `DDSTARS/PIC2.EGA`, the stale file of the same nominal name, which differs from the
/// shipped mask in 7,162 of 8,000 bytes (§D).
#[test]
fn the_recovered_mask_is_what_the_generator_in_the_tree_writes() {
    let mut generated = Vec::with_capacity(8_000);
    for y in 0..200i32 {
        for x in (0..320i32).step_by(8) {
            let mut byte = 0u8;
            for k in 0..8i32 {
                let dx = (x + k - 320) as f64;
                let dy = (y - 200) as f64;
                let r = (dx * dx + dy * dy).sqrt() as i32;
                if (r / 12) & 1 == 1 {
                    byte |= 1 << (7 - k);
                }
            }
            generated.push(byte);
        }
    }
    assert_eq!(asset("TECHNO/CIRCLE2.EGA"), generated, "DDSTARS/DOIT2.C:21-52");

    let stale = upstream("DDSTARS/PIC2.EGA");
    let differing = generated.iter().zip(&stale).filter(|(a, b)| a != b).count();
    assert_eq!(differing, 7_162, "DDSTARS/PIC2.EGA is not the shipped mask, size aside");
}

/// The other two blob-only payloads answer as residue:
///
/// - `TECHNO/_PIC2.OBK`'s 20,859 bytes are the last 20,859 bytes of `TECHNO/TROLL.UP`
///   (70,718 = 49,859 + 20,859), byte for byte. TROLL.UP is the picture the shipped part
///   loads at runtime (`TECHNO/KOE.C:605`), so the blob holds a slice of an asset that is
///   already curated and nothing else.
/// - `TECHNO/_PIC1.OBK` is a truncated Origin container that no shipped build links (the
///   test above); it stays in the tree as build residue.
#[test]
fn the_unlinked_picture_blob_is_a_slice_of_the_curated_picture() {
    let payload = payload("TECHNO/_PIC2.OBK");
    assert_eq!(payload.len(), 20_859);
    assert!(
        upstream("TECHNO/TROLL.UP").ends_with(&payload),
        "_PIC2.OBK is not the tail of TROLL.UP"
    );
}

/// Regenerates the one recovered asset from the blob it ships in. Not part of a normal
/// run — it writes into the repository — and it is the tool that wrote the file:
///
/// ```text
/// cargo test -p sr-assets --test omf -- --ignored write_the_recovered_asset
/// ```
#[test]
#[ignore = "writes into assets/; run by hand to regenerate the recovered file"]
fn write_the_recovered_asset() {
    let out = Path::new("../../assets/TECHNO/CIRCLE2.EGA");
    std::fs::write(out, payload("TECHNO/_CIRCLE2.OBK")).unwrap();
    println!("wrote {}", out.display());
}

// -------------------------------------------------------------------------------------
// The EXE side: what SECOND.EXE carries
// -------------------------------------------------------------------------------------

/// The distributor's two text files are packed and shown by nothing: the setup program
/// reads and rewrites them (`MAIN/START.C:40,53`, a copy whose `fwrite` passes the
/// already-closed `f1`), and no part displays either. They are not port assets (§C, §E.12,
/// §F), which is why they are the only pack entries with no file in `assets/`.
const DISTRIBUTOR_TEXT: [&str; 2] = ["FCINFO10.TXT", "README.1ST"];

/// `MAIN/PACKFINA.INC:1-81` is the 79-entry table of what `SECOND.EXE` carries, six `dw`
/// fields each. Every entry is either a part executable (22 of them) or a file whose bytes
/// are already in `assets/` (55) — so everything the pack hands out beside the part
/// executables is accounted for in the curated tree — except the distributor's two text
/// files, named above. What lives *inside* a part executable is §D's subject, and one of
/// those assets was recovered (§A.2).
#[test]
fn the_shipped_pack_carries_part_executables_and_assets() {
    let manifest = format!("{UPSTREAM}/MAIN/PACKFINA.INC");
    let text = std::fs::read_to_string(&manifest).unwrap();

    let mut lines = text.lines().filter(|l| !l.trim().is_empty());
    let declared: usize = lines
        .next()
        .unwrap()
        .trim()
        .strip_prefix("dw ")
        .expect("the table opens with `dw <count>`")
        .parse()
        .unwrap();
    assert_eq!(declared, 79, "{manifest}:1");

    let mut names = Vec::new();
    for line in lines {
        let (fields, name) = line.split_once(';').expect("every row ends in ;NAME");
        assert_eq!(fields.split(',').count(), 6, "{line}");
        names.push(name.trim().to_string());
    }
    assert_eq!(names.len(), 79, "one row per declaration");
    let mut sorted = names.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), 79, "duplicate pack entry");

    // Every name resolves to a staged file in MAIN/DATA (§C).
    let staged: Vec<(String, Vec<u8>)> = names
        .iter()
        .map(|name| {
            (
                name.clone(),
                upstream(&format!("MAIN/DATA/{name}")),
            )
        })
        .collect();

    // Every file in assets/, by name, once.
    let mut curated: Vec<(String, PathBuf)> = Vec::new();
    fn walk(dir: &Path, out: &mut Vec<(String, PathBuf)>) {
        for entry in std::fs::read_dir(dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display())) {
            let path = entry.expect("directory entry").path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.file_name().and_then(|n| n.to_str()) != Some("PROVENANCE.md") {
                let name = path.file_name().unwrap().to_str().unwrap().to_string();
                out.push((name, path));
            }
        }
    }
    walk(Path::new("../../assets"), &mut curated);

    let (mut executables, mut in_assets, mut distributor) = (0, 0, 0);
    for (name, bytes) in &staged {
        if name.ends_with(".EXE") {
            executables += 1;
            continue;
        }
        if DISTRIBUTOR_TEXT.contains(&name.as_str()) {
            distributor += 1;
            continue;
        }
        let matches = curated
            .iter()
            .filter(|(candidate, _)| candidate == name)
            .any(|(_, path)| std::fs::read(path).unwrap() == *bytes);
        assert!(
            matches,
            "{name}: packed ({} bytes) and not a part EXE, but no file of that name in \
             assets/ has its bytes",
            bytes.len()
        );
        in_assets += 1;
    }
    assert_eq!((executables, in_assets, distributor), (22, 55, 2));
    assert_eq!(executables + in_assets + distributor, 79);
}
