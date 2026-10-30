//! The shipped part EXEs under `assets/MAIN/DATA/`, unpacked.
//!
//! Each pin (length, relocation count, FNV-1a of the image) is the image that matched the
//! emulator's own load of the same file: run alone under DOS, every EXE unpacks itself and
//! exits on `ERROR: DIS not loaded`, and its memory, against this image with its relocations
//! applied, differs only in the C runtime's startup writes (MNTSCRL also decodes its `RIX3`
//! picture in place before it checks). `python3 -I tools/release_images.py` repeats that
//! check (`docs/fidelity/release-images.md`).

use sr_assets::exe::{unpack, Packer};
use sr_assets::AssetError;

fn fnv1a(b: &[u8]) -> u64 {
    b.iter().fold(0xcbf2_9ce4_8422_2325, |h, &x| {
        (h ^ x as u64).wrapping_mul(0x100_0000_01b3)
    })
}

fn exe(name: &str) -> Vec<u8> {
    std::fs::read(format!("../../assets/MAIN/DATA/{name}.EXE")).unwrap()
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
