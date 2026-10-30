//! The OMF reader: the record framing, its refusals, and the blobs whose payload is a file in
//! the tree (`asset-inventory.md` §D). Every object is read from `upstream/SecondReality`.

mod common;

use common::upstream;
use rstest::rstest;
use sr_assets::omf::extract_ledata;
use sr_assets::{load, sniff, AssetError, Format};

/// One blob's reconstructed payload, with the blob named in the failure.
fn payload(blob: &str) -> Vec<u8> {
    extract_ledata(&upstream(blob)).unwrap_or_else(|e| panic!("{blob}: {e}"))
}

/// The framing: `GLENZ/_FC.OBK`'s 64 records reproduce `GLENZ/FC.UH`, 64,784 bytes, byte for byte.
/// Equality, because a header read one byte long yields the same length.
#[test]
fn a_blob_reproduces_its_standalone_twin_byte_for_byte() {
    let payload = extract_ledata(&upstream("GLENZ/_FC.OBK")).unwrap();
    assert_eq!(payload.len(), 64_784, "the measured payload length");
    assert_eq!(payload, upstream("GLENZ/FC.UH"));
}

/// A `LEDATA` body is segment, offset, data, checksum: the payload is `body[3..len-1]`.
#[test]
fn the_record_header_and_the_trailing_checksum_are_not_payload() {
    // Type 0xA0, body length 8, segment 1, offset 0x0400, four data bytes, checksum.
    let blob = [
        0xA0, 0x08, 0x00, // record header
        0x01, 0x00, 0x04, // segment index, data offset 0x0400
        b'D', b'A', b'T', b'A', // the data
        0x00, // checksum
    ];
    assert_eq!(extract_ledata(&blob).unwrap(), b"DATA");
}

/// An object with no `LEDATA` (`VISU/AMAIN.OBJ`), or no records at all, is not a data blob.
#[test]
fn a_blob_with_no_ledata_is_refused() {
    assert!(matches!(extract_ledata(&upstream("VISU/AMAIN.OBJ")), Err(AssetError::UnknownFormat)));
    assert!(matches!(extract_ledata(&[]), Err(AssetError::UnknownFormat)));
}

/// A length that overruns the blob, or a tail too short for a record header, is truncated.
#[test]
fn a_record_that_runs_past_the_end_of_the_blob_is_refused() {
    assert!(matches!(
        extract_ledata(&[0xA0, 0xFF, 0xFF, 0x00]),
        Err(AssetError::Truncated { .. })
    ));
    assert!(matches!(extract_ledata(&[0xA0, 0x00]), Err(AssetError::Truncated { .. })));
}

/// `0xA1` (Intel's 32-bit-offset `LEDATA`) appears in no file here and is refused.
#[test]
fn a_record_type_this_tree_never_uses_is_refused() {
    // 0xA1, body length 4, segment 1, offset 0, checksum.
    let blob = [0xA1, 0x04, 0x00, 0x01, 0x00, 0x00, 0x00];
    assert!(matches!(
        extract_ledata(&blob),
        Err(AssetError::Unsupported { format: Format::Omf, .. })
    ));
}

/// A `LEDATA` body shorter than its own header is truncated.
#[test]
fn a_ledata_body_too_short_to_hold_its_header_is_refused() {
    assert!(matches!(
        extract_ledata(&[0xA0, 0x02, 0x00, 0x01, 0x00]),
        Err(AssetError::Truncated { format: Format::Omf, .. })
    ));
}

/// `TECHNO/_PIC1.OBK` holds an Origin RLE container (320x400) cut at 50,000 bytes, 119 bytes into
/// its 282nd row; `load` refuses it (§E.13; `TECHNO/MAKEFILE:7` does not link it).
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

/// Every blob whose payload is a standalone file, as `(blob, file)`: `asset-inventory.md` §D's 22
/// whole-file rows, then 13 more pairs the same walk finds. Where two files are identical
/// (`GLENZ/EYE4.UH`, `VISU/C/U2ABG.UH`) the canonical one is named.
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
    // Pairs the same walk finds elsewhere.
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

/// Every one of the 35 blobs reproduces its standalone file byte for byte; the walk's exceptions
/// are named in §D and §E.18–E.20.
#[test]
fn every_blob_with_a_twin_reproduces_it_byte_for_byte() {
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

/// The four `COMAN` half-blobs are the slices `COMAN/DOOBJS.BAT:1-2` and `:7-8` cut with
/// `getslice … 0 32768` and `32768 32768`.
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
