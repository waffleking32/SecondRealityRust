//! The release's cube data, read from the shipped `MAIN/DATA/PLZPART.EXE`'s
//! unpacked image (D49, `docs/fidelity/release-images.md`): the camera path
//! `buu` and the 8,192-entry sine table. The tree's `RATA.INC` and `SINIT.INC`
//! are not the shipped build's: the release's `buu` has every rotation and light
//! word 8× the tree's and its own `dz` in rows 14-31, and its `sinit` is a
//! 13-bit turn with `kosinit` 2,048 words on (A23, `plzpart.md` §5).

/// `SPLINE.ASM:15-16`: `buu`, the camera control points, 136 × 8 words.
pub(super) const RATA_WORDS: usize = 1_088;

/// The shipped part, packed (PKLITE large over LZEXE 0.91, `docs/fidelity/release-images.md`).
/// [`sr_assets::exe::unpack`] gives its 223,376-byte load image; every offset below is an
/// offset into that image, and a segment word is relative to its origin.
pub(super) const EXE_PATH: &str = "assets/MAIN/DATA/PLZPART.EXE";
/// The image's length, which [`release_cube`] checks before it reads.
const IMAGE_BYTES: usize = 223_376;
/// `getspl`'s code segment: `calculate` calls it `call far 1389h:1084h` (image `0x12c8a`),
/// so `SPLINE.ASM`'s CODESEG starts at image `0x13890`.
const SPLINE_SEGMENT: usize = 0x1389 * 16;
/// `getspl` itself, `cs:1084h`: `enter 0,0; push ds bp si di; mov si,0004h; push cs; pop ds`
/// (`SPLINE.ASM:26-30`). The `mov si` is `mov si, OFFSET buu` (image `0x1491c`).
const GETSPL_AT: usize = SPLINE_SEGMENT + 0x1084;
/// The prologue [`GETSPL_AT`] holds in this build: the `0004h` in it is `buu`'s offset.
const GETSPL_PROLOGUE: [u8; 13] = [
    0xc8, 0x00, 0x00, 0x00, 0x1e, 0x55, 0x56, 0x57, 0xbe, 0x04, 0x00, 0x0e, 0x1f,
];
/// `LABEL buu WORD` (`SPLINE.ASM:13-14`), found through `getspl`'s `mov si,0004h`:
/// `cs:0004h`, image **`0x13894`**. 136 rows of `dx, dy, dz, kx, ky, kz, l_kx, l_ky`, the
/// shape of the tree's `RATA.INC`, with the release's own values.
const BUU_AT: usize = SPLINE_SEGMENT + 0x0004;
/// `sinit`: `mov ax,28b1h; mov es,ax; mov ax,es:[bx+1000h]` in `calculate` (image `0x12cc3`)
/// and in `count_const` — segment `0x28b1`, image `0x28b10`. `kosinit` is the same segment at
/// `+1000h` bytes, **2,048 words** on: a quarter of an **8,192-entry** turn, where the tree's
/// is 256 words on a 1,024-entry turn (`INCLUDE.ASM:36`).
const SINIT_AT: usize = 0x28b1 * 16;
/// The release's `kosinit = sinit + 1000h` bytes, in words.
pub(super) const KOSINIT_WORDS: usize = 0x800;
/// `and ax,1fffh` on `kx, ky, kz, ls_kx, ls_ky` (image `0x12c93-0x12cb7`, `VECT.C:138-142`'s
/// `&1023`): the release's angles are 13 bits. The table therefore holds
/// `8,191 + 2,048 + 1` = 10,240 words; the next segment starts at image `0x2db10`.
pub(super) const ANGLE_MASK: i32 = 0x1fff;
pub(super) const RELEASE_SINIT_WORDS: usize = ANGLE_MASK as usize + 1 + KOSINIT_WORDS;

/// What the cube reads out of the shipped image.
#[derive(Debug)]
pub(super) struct ReleaseCube {
    /// `buu`, 1,088 words.
    pub(super) buu: Vec<i16>,
    /// `sinit`, 10,240 words (`kosinit` included).
    pub(super) sinit: Vec<i16>,
}

/// Read [`ReleaseCube`] out of an unpacked image, refusing one whose layout is not this build's.
pub(super) fn release_cube(image: &sr_assets::exe::LoadImage) -> Result<ReleaseCube, String> {
    let bytes = &image.image;
    if bytes.len() != IMAGE_BYTES {
        return Err(format!(
            "the image is {} bytes; this build's is {IMAGE_BYTES}",
            bytes.len()
        ));
    }
    if bytes[GETSPL_AT..GETSPL_AT + GETSPL_PROLOGUE.len()] != GETSPL_PROLOGUE {
        return Err(format!("getspl at {GETSPL_AT:#x} is not this build's"));
    }
    let buu_end = BUU_AT + RATA_WORDS * 2;
    let sinit_end = SINIT_AT + RELEASE_SINIT_WORDS * 2;
    // A relocation inside what is read would be a segment word, not data.
    let read = [(BUU_AT, buu_end), (SINIT_AT, sinit_end)];
    if let Some(at) = image
        .relocations
        .iter()
        .map(|&at| at as usize)
        .find(|at| read.iter().any(|(from, to)| (*from..*to).contains(at)))
    {
        return Err(format!("a relocation at {at:#x} lies inside the data read"));
    }
    let words = |from: usize, to: usize| -> Vec<i16> {
        bytes[from..to]
            .chunks_exact(2)
            .map(|pair| i16::from_le_bytes([pair[0], pair[1]]))
            .collect()
    };
    Ok(ReleaseCube {
        buu: words(BUU_AT, buu_end),
        sinit: words(SINIT_AT, sinit_end),
    })
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use crate::parts::plzpart::tables::tests::{tables, tree_rata};
    use crate::parts::plzpart::tables::SPLINE_WORDS;

    /// The shipped part, unpacked.
    pub(in super::super) fn image() -> sr_assets::exe::LoadImage {
        sr_assets::exe::unpack(crate::assets::read(EXE_PATH).unwrap()).unwrap()
    }

    /// The release's own `plz()` holds the 4 the frames measured (A22): `curpal` is DGROUP
    /// `0192h` — `cop_fadepal = pals[curpal++]` is `mov ax,[0192h]; imul 600h` and
    /// `inc word [0192h]` (image `0x1178b`, `0x117a4`) — and the exit is
    /// `cmp word [0192h],4; jne; cmp es:[000eh],40h; jle` (image `0x1181c`), the tree's
    /// `PLZ.C:94` with 4 for its 5. The timetable it switches on (DGROUP `0194h`) is the
    /// tree's.
    #[test]
    fn the_release_exit_index_is_the_images() {
        let image = image();
        assert_eq!(&image.image[0x1178b..0x1178e], &[0xa1, 0x92, 0x01]);
        assert_eq!(&image.image[0x117a4..0x117a8], &[0xff, 0x06, 0x92, 0x01]);
        assert_eq!(
            &image.image[0x1181c..0x11823],
            &[0x83, 0x3e, 0x92, 0x01, 0x04, 0x75, 0x10]
        );
        assert_eq!(
            &image.image[0x11828..0x1182f],
            &[0x26, 0x83, 0x3e, 0x0e, 0x00, 0x40, 0x7e]
        );
        assert_eq!(i32::from(image.image[0x11820]), super::super::plasma::EXIT_PALETTE_RELEASE);
        // DGROUP is `mov ax,2ee0h` in `getspl` (image `0x14adb`).
        assert_eq!(&image.image[0x14adb..0x14ade], &[0xb8, 0xe0, 0x2e]);
        let timetable: Vec<i32> = image.image[0x2ee00 + 0x194..][..12]
            .chunks_exact(2)
            .map(|pair| i32::from(i16::from_le_bytes([pair[0], pair[1]])))
            .collect();
        assert_eq!(timetable, super::super::plasma::TIMETABLE);
    }

    /// `buu` is read at image `0x13894`, the offset `getspl`'s `mov si,0004h` names in the
    /// segment `calculate` calls it in, and the table has the tree's `RATA.INC` shape with
    /// the release's values: every rotation and light word is **8x** the tree's (the
    /// release's angles are 13 bits), and `dz` differs in rows 14-31 alone.
    #[test]
    fn the_release_camera_table_is_read_from_the_image() {
        let image = image();
        // `calculate`'s `call far 1389h:1084h`, and the `mov si, OFFSET buu` in `getspl`.
        assert_eq!(
            &image.image[0x12c8a..0x12c8f],
            &[0x9a, 0x84, 0x10, 0x89, 0x13]
        );
        assert_eq!(
            (SPLINE_SEGMENT, GETSPL_AT, BUU_AT),
            (0x13890, 0x14914, 0x13894)
        );
        assert_eq!(&image.image[0x1491c..0x1491f], &[0xbe, 0x04, 0x00]);
        let buu = release_cube(&image).unwrap().buu;
        let tree = tree_rata();
        assert_eq!(buu.len(), RATA_WORDS);
        let row = |at: usize| -> [i32; 8] { core::array::from_fn(|w| buu[at * 8 + w] as i32) };
        assert_eq!(row(0), [0, 2000, 500, 0, 3200, 4800, 0, 0]);
        assert_eq!(row(15), [0, 0, 300, 800, 2400, 4000, 2048, 0]);
        assert_eq!(row(35), [0, 0, 500, 800, 800, 800, 1536, 4096]);
        let dz: Vec<i32> = (14..32).map(|at| row(at)[2]).collect();
        assert_eq!(
            dz,
            [
                325, 300, 300, 325, 350, 350, 350, 350, 350, 350, 350, 350, 350, 350, 350, 325,
                300, 325
            ]
        );
        for at in 0..RATA_WORDS / 8 {
            let tree_row: [i32; 8] = core::array::from_fn(|w| tree[at * 8 + w]);
            assert_eq!(row(at)[..2], tree_row[..2], "row {at}'s dx, dy");
            if !(14..32).contains(&at) {
                assert_eq!(row(at)[2], tree_row[2], "row {at}'s dz");
            } else {
                assert_eq!(tree_row[2], 300, "the tree's row {at} is a flat 300");
            }
            for (w, (&release, &tree)) in row(at).iter().zip(&tree_row).enumerate().skip(3) {
                assert_eq!(release, 8 * tree, "row {at}'s word {w} is 8x the tree's");
            }
        }
        // The `REPT 100` tail: the tree's `256,512` light is the release's `2048,4096`.
        for at in 36..RATA_WORDS / 8 {
            assert_eq!(row(at), [0, 0, 500, 0, 0, 0, 2048, 4096]);
        }
    }

    /// `splinecoef` follows `buu` in the image, and its 1,024 words are the tree's
    /// `SPLINE.INC`: the part reads the fragment and loses nothing.
    #[test]
    fn the_release_reads_the_trees_spline_coefficients() {
        let image = image();
        // `LABEL splinecoef WORD` (`SPLINE.ASM:16-17`), right after `buu`'s 1,088 words.
        const SPLINECOEF_AT: usize = SPLINE_SEGMENT + 0x0884;
        assert_eq!(SPLINECOEF_AT, BUU_AT + RATA_WORDS * 2);
        // `imul word cs:[di+0884h]`, the first coefficient read in `getspl`.
        assert_eq!(
            &image.image[0x14937..0x1493c],
            &[0x2e, 0xf7, 0xad, 0x84, 0x08]
        );
        let coefficients: Vec<i16> = image.image[SPLINECOEF_AT..SPLINECOEF_AT + SPLINE_WORDS * 2]
            .chunks_exact(2)
            .map(|pair| i16::from_le_bytes([pair[0], pair[1]]))
            .collect();
        assert_eq!(coefficients, tables().spline);
    }

    /// The release's sine table: segment `0x28b1` (`mov ax,28b1h; mov es,ax`), an 8,192-entry
    /// turn whose quarter is 2,048 words — `kosinit` is `es:[bx+1000h]` — read for the
    /// 13-bit angles `and ax,1fffh` leaves.
    #[test]
    fn the_release_sine_table_is_an_8192_entry_turn() {
        let image = image();
        assert_eq!(
            &image.image[0x12cc3..0x12ccd],
            &[0xb8, 0xb1, 0x28, 0x8e, 0xc0, 0x26, 0x8b, 0x87, 0x00, 0x10]
        );
        assert_eq!(&image.image[0x12c93..0x12c96], &[0x25, 0xff, 0x1f]);
        assert_eq!(
            (SINIT_AT, ANGLE_MASK, KOSINIT_WORDS * 2),
            (0x28b10, 0x1fff, 0x1000)
        );
        let sinit = release_cube(&image).unwrap().sinit;
        assert_eq!(sinit.len(), 10_240);
        assert_eq!(&sinit[..4], &[0, 25, 50, 75]);
        let at = |k: usize| sinit[k];
        assert_eq!(
            (at(2048), at(4096), at(6144), at(8192)),
            (32759, 50, -32759, -200)
        );
        // The greatest word read is `kosinit[8191]`, the table's last.
        assert_eq!(at(ANGLE_MASK as usize + KOSINIT_WORDS), 32758);
        // The next segment's first words, which no read reaches.
        assert_eq!(
            &image.image[SINIT_AT + 2 * RELEASE_SINIT_WORDS..][..4],
            &[0, 0, 1, 0]
        );
    }

    /// A build whose `getspl` is not this one's is refused rather than read.
    #[test]
    fn an_image_with_another_layout_is_refused() {
        let mut image = image();
        image.image[GETSPL_AT + 9] = 0x06;
        assert!(release_cube(&image).unwrap_err().contains("getspl"));
        let mut image = self::image();
        image.relocations.push(BUU_AT as u32 + 8);
        assert!(release_cube(&image).unwrap_err().contains("relocation"));
        let mut image = self::image();
        image.image.pop();
        assert!(release_cube(&image).unwrap_err().contains("bytes"));
    }
}
