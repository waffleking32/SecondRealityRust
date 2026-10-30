//! The shipped part EXEs under `assets/MAIN/DATA/`, unpacked.
//!
//! Each pin (length, relocation count, FNV-1a of the image) is the image that matched the
//! emulator's own load of the same file: run alone under DOS, every EXE unpacks itself and
//! exits on `ERROR: DIS not loaded`, and its memory, against this image with its relocations
//! applied, differs only in the C runtime's startup writes (MNTSCRL also decodes its `RIX3`
//! picture in place before it checks). `python3 -I tools/release_images.py` repeats that
//! check (`docs/fidelity/release-images.md`).

mod common;

use common::fnv1a;
use sr_assets::exe::{unpack, Packer};
use sr_assets::AssetError;

fn exe(name: &str) -> Vec<u8> {
    common::asset(&format!("MAIN/DATA/{name}.EXE"))
}

const PINS: &[(&str, usize, usize, u64)] = &[
    ("3DSINFLD", 172368, 54, 0xba02c59c38acda39),
    ("ALKU", 343472, 238, 0x9ce530a2bedb5f89),
    ("BEGLOGO", 52656, 46, 0xd2ed0aba92d2e84d),
    ("CRED", 426384, 261, 0xa14d697a26f58fa0),
    ("DDSTARS", 42048, 3, 0x93f2540212622d93),
    ("ENDLOGO", 54176, 50, 0x7573185abc7b88e0),
    ("ENDSCRL", 135424, 90, 0xcc2eb8f951528c93),
    ("GLENZ", 398912, 211, 0xb28bcf260b93299f),
    ("JPLOGO", 209408, 48, 0x1e048ce85bbb4c42),
    ("LNS&ZOOM", 170208, 167, 0x784cdf9e23402a92),
    ("MINVBALL", 31808, 113, 0x874ce9497f516e22),
    ("MNTSCRL", 145664, 58, 0xf54f1485a372de14),
    ("PAM", 381312, 59, 0xd515e4d6ae715000),
    ("PANICEND", 141216, 68, 0x2d03984ab043782c),
    ("PLZPART", 223376, 298, 0x40e120bfaaf6fce6),
    ("RAYSCRL", 157792, 45, 0x505761d7e8c981f6),
    ("START", 21296, 244, 0xe8d416779a3f4d1e),
    ("STARTMUS", 15328, 185, 0x6fa459606320f275),
    ("TECHNO", 72336, 180, 0xb33e883f66bcd23b),
    ("TUNNELI", 52928, 29, 0x88d97296b3a0db04),
];

#[test]
fn every_shipped_part_unpacks_to_its_verified_image() {
    for &(name, len, nrel, hash) in PINS {
        let p = unpack(&exe(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(
            (p.image.len(), p.relocations.len(), fnv1a(&p.image)),
            (len, nrel, hash),
            "{name} ({:?})",
            p.layers
        );
    }
}

#[test]
fn the_layers_are_pklite_over_lzexe_or_exepack() {
    let layers = |n| unpack(&exe(n)).unwrap().layers;
    assert_eq!(layers("ENDSCRL"), [Packer::PkliteSmall, Packer::Lzexe091]);
    assert_eq!(layers("PLZPART"), [Packer::PkliteLarge, Packer::Lzexe091]);
    assert_eq!(layers("GLENZ"), [Packer::PkliteLarge, Packer::Exepack]);
    assert_eq!(layers("CRED"), [Packer::PkliteLarge]);
}

#[test]
fn the_released_endscrl_carries_its_eighty_character_font_order() {
    let p = unpack(&exe("ENDSCRL")).unwrap();
    let order: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXabcdefghijklmnopqrstuvwxyz0123456789!?,.:\x8f\x8f()+-*='Z\x84\x94Y/&\0";
    assert!(p.image.windows(order.len()).any(|w| w == order));
}

#[test]
fn the_released_credits_name_the_sketch() {
    let p = unpack(&exe("CRED")).unwrap();
    assert!(p.image.windows(6).any(|w| w == b"SKETCH"));
}

#[test]
fn a_truncated_stream_is_refused_not_guessed() {
    let mut f = exe("ENDSCRL");
    f.truncate(f.len() - 40);
    assert!(matches!(
        unpack(&f),
        Err(AssetError::Truncated { .. }) | Err(AssetError::Unsupported { .. })
    ));
}

#[test]
fn a_file_that_is_not_mz_is_unknown() {
    assert!(matches!(
        unpack(b"not a program"),
        Err(AssetError::UnknownFormat)
    ));
}

/// An MZ file whose entry segment holds an EXEPACK `RB` header at image offset 16, with `head`
/// as the 16 image bytes before it: the command stream is read backwards from `head`'s end.
fn exepack_file(head: [u8; 16]) -> Vec<u8> {
    let mut image = head.to_vec();
    let mut rb = [0u8; 16];
    rb[12] = 1; // dest_len: one paragraph
    rb[14..16].copy_from_slice(b"RB");
    image.extend_from_slice(&rb);
    let size = 32 + image.len();
    let mut file = vec![0u8; 32];
    file[0..2].copy_from_slice(b"MZ");
    file[2..4].copy_from_slice(&(size as u16 % 512).to_le_bytes());
    file[4..6].copy_from_slice(&1u16.to_le_bytes()); // pages
    file[8..10].copy_from_slice(&2u16.to_le_bytes()); // header paragraphs
    file[0x16..0x18].copy_from_slice(&1u16.to_le_bytes()); // cs
    file[0x18..0x1a].copy_from_slice(&0x1cu16.to_le_bytes()); // relocation table
    file.extend_from_slice(&image);
    file
}

/// A command stream that runs off the front of the image is refused, not an index panic: all
/// sixteen bytes `0xFF` padding, and a stream whose second command sits at byte 2.
#[test]
fn an_exepack_stream_that_runs_before_the_image_is_refused() {
    let mut runs_off = [0u8; 16];
    runs_off[2] = 0xb0; // a fill, with no byte left for its value
    runs_off[13..15].copy_from_slice(&10u16.to_le_bytes());
    runs_off[15] = 0xb2; // copy ten bytes, then the command at 2
    for head in [[0xFF; 16], runs_off] {
        assert!(
            matches!(unpack(&exepack_file(head)), Err(AssetError::Unsupported { .. })),
            "{head:02x?}"
        );
    }
}

/// Writes every unpacked image to `out/release-images/NAME.img` with its relocation offsets in
/// `NAME.rel` (one per line), for `tools/release_images.py` to compare against the emulator.
#[test]
#[ignore = "writes out/release-images for tools/release_images.py"]
fn dump_the_unpacked_images() {
    let dir = std::path::Path::new("../../out/release-images");
    std::fs::create_dir_all(dir).unwrap();
    for &(name, ..) in PINS {
        let p = unpack(&exe(name)).unwrap();
        std::fs::write(dir.join(format!("{name}.img")), &p.image).unwrap();
        let rel: Vec<String> = p.relocations.iter().map(u32::to_string).collect();
        std::fs::write(dir.join(format!("{name}.rel")), rel.join("\n")).unwrap();
    }
}
