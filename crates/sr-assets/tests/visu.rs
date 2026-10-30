//! VISU's packed scene formats (`.00M`, `.0AA`, `.0AB`, `.00N`) against the shipped scenes and
//! their writers (`VISU/C/SAVE.C:40-90`, `VISU/C/MAIN.C:252-321`, `READVUE.C:149-255`). The list
//! base and the `6+2*n` face record are the data's, not `C.H`'s or `CD.H`'s comments, and
//! `every_shipped_object_resolves_its_lists_and_matches_its_own_vertex_count` is what settles
//! them: no other base resolves all 405 shipped lists.

mod common;

use sr_assets::visu::{read_animation, read_master, read_object, read_scene_list};
use sr_assets::AssetError;

const SCENE: &str = "../../assets/VISU/C/SCENE";

fn asset(name: &str) -> Vec<u8> {
    common::asset(&format!("VISU/C/SCENE/{name}"))
}

/// Every shipped object file: `<scene>.NNN` for U2A's 001-003 and U2E's
/// 001-042, in name order.
fn shipped_objects() -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(SCENE).unwrap_or_else(|e| panic!("cannot read {SCENE}: {e}")) {
        let path = entry.expect("directory entry").path();
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .expect("a UTF-8 asset name")
            .to_owned();
        let Some((_, extension)) = name.rsplit_once('.') else {
            continue;
        };
        if extension.len() == 3 && extension.chars().all(|c| c.is_ascii_digit()) {
            out.push((name, std::fs::read(&path).expect("a readable asset")));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// A block's data as `(offset in the file, bytes)`, found by walking the block
/// chain exactly as `vis_loadobject` does. The reader does not expose the
/// counts inside `VERT`/`NORM`, so the test reads them from the file itself.
fn block_at<'a>(bytes: &'a [u8], tag: &[u8; 4]) -> (usize, &'a [u8]) {
    let mut at = 0usize;
    loop {
        assert!(
            at + 8 <= bytes.len(),
            "no {:?} block",
            String::from_utf8_lossy(tag)
        );
        let length =
            u32::from_le_bytes([bytes[at + 4], bytes[at + 5], bytes[at + 6], bytes[at + 7]])
                as usize;
        let end = at + 8 + length;
        assert!(end <= bytes.len(), "block at {at} runs past the file");
        if &bytes[at..at + 4] == tag {
            return (at + 8, &bytes[at + 8..end]);
        }
        at = end;
    }
}

fn block_data<'a>(bytes: &'a [u8], tag: &[u8; 4]) -> &'a [u8] {
    block_at(bytes, tag).1
}

#[test]
fn the_u2a_master_reads_its_city_palette_and_object_numbers() {
    let bytes = asset("U2A.00M");
    assert_eq!(bytes.len(), 796);
    let master = read_master(&bytes).unwrap();

    assert_eq!(master.city, 0, "U2A.00M byte 15 is not 'C' or 'R'");
    assert_eq!(master.objects, [1, 2, 3, 1, 1]);
    // `conum` counts the camera as object 0, which is why the file carries
    // conum-1 numbers (`U2A.C:170-172` starts `c` at 1).
    assert_eq!(master.objects.len() + 1, 6);

    // The palette is the file's own 768 bytes at 16..784, six-bit values
    // copied through Palette::set_entry.
    for i in 0..256usize {
        let at = 16 + i * 3;
        assert_eq!(
            master.palette.entry(i as u8),
            [bytes[at], bytes[at + 1], bytes[at + 2]],
            "palette entry {i}"
        );
    }
}

#[test]
fn the_u2e_master_reads_fifty_eight_objects_and_its_duplicates() {
    let bytes = asset("U2E.00M");
    assert_eq!(bytes.len(), 900);
    let master = read_master(&bytes).unwrap();

    assert_eq!(master.city, 0);
    let expected: Vec<u16> = (1..=42)
        .chain([9, 9, 9, 9, 6, 25, 32, 32, 35, 35, 32, 32, 32, 4, 32])
        .collect();
    assert_eq!(master.objects, expected);
    assert_eq!(master.objects.len() + 1, 58, "conum counts the camera");
}

/// The city flag is byte 15 and only the two letters mean a city
/// (`MAIN.C:317-320`, `U2A.C:168-169`); the shipped scenes carry neither.
#[test]
fn the_city_flag_is_read_from_byte_fifteen() {
    let mut bytes = asset("U2A.00M");
    for (byte, city) in [(b'C', 1), (b'R', 2), (0, 0), (b'c', 0), (0xFF, 0)] {
        bytes[15] = byte;
        assert_eq!(read_master(&bytes).unwrap().city, city, "byte 15 = {byte:#04x}");
    }
}

/// The command encoding's uncommon branches, built by hand because the two
/// shipped streams only use some of them together: a `0xC0` extension naming
/// the object's high bits, the on/off toggle, pflags of one, two and three
/// bytes, a word matrix and a four-byte position delta.
#[test]
fn the_command_encoding_reads_extensions_toggles_sizes_and_signs() {
    let stream = [
        0x10, 0x01, 0x02, // object 0, one-byte pflag, dx = +2
        0xC1, 0x82, // extension 0x1_, then object 0x12 with its on bit set
        0x30, 0xC0, 0x00, 0x00, 0xFE, 0xFF, // three-byte pflag, word-matrix entry 0 = -2
        0x20, 0x03, 0x00, 0xFE, 0xFF, 0xFF, 0xFF, // two-byte pflag, 4-byte dx = -2
        0xFF, 0x11, // end of frame, fov 0x11
        0xFF, 0xFF, // end of stream
    ];
    let animation = read_animation(&stream, 0x13).unwrap();

    assert_eq!(animation.frames.len(), 2, "the END-terminated frame included");
    assert_eq!(animation.frames[0].fov, 0x11);
    assert!(!animation.frames[0].end);
    let ops = &animation.frames[0].ops;
    assert_eq!(ops.len(), 4);
    assert_eq!((ops[0].object, ops[0].toggle, ops[0].dx), (0, None, 2));
    assert_eq!((ops[1].object, ops[1].toggle), (0x12, Some(true)));
    // The `0x30` and `0x20` commands carry no extension, so the object's high
    // nibble is the 0x10 the previous command set, exactly as
    // `onum=(onum&0xff0)|(a&0xf)` leaves it (`U2A.C:383`).
    assert_eq!(ops[2].object, 0x10);
    assert_eq!(ops[2].dm[0], Some(-2), "a word-matrix delta");
    assert_eq!(ops[3].object, 0x10);
    assert_eq!(ops[3].dx, -2, "a four-byte signed delta");
    assert_eq!(animation.frames[1].fov, 0, "the END frame has no fov");
    assert!(animation.frames[1].end);
    assert!(animation.frames[1].ops.is_empty());
}

/// `0xFF` is a terminator only when the byte after it is `<= 0x7F` (a frame)
/// or `0xFF` (END). Anything else falls through as the command byte, which is
/// `U2A.C:343-361`'s `if`/`else if` with no `else`. The shipped streams never
/// contain it — `READVUE.C` cannot emit a command byte `0xFF` — so this is
/// built by hand; `0x80` here is "object 0 on" and `0xFF 0x22` ends the frame.
#[test]
fn a_0xff_byte_that_is_not_a_terminator_falls_through_as_a_command() {
    let animation = read_animation(&[0xFF, 0x80, 0xFF, 0x22, 0xFF, 0xFF], 1).unwrap();

    assert_eq!(animation.frames.len(), 2);
    assert_eq!(animation.frames[0].fov, 0x22);
    assert!(!animation.frames[0].end);
    assert_eq!(animation.frames[0].ops.len(), 1);
    assert_eq!(
        (
            animation.frames[0].ops[0].object,
            animation.frames[0].ops[0].toggle,
        ),
        (0, Some(true))
    );
    assert!(animation.frames[1].end);
}

/// The END frame keeps the commands the stream places before its terminator:
/// the original applies them and then `resetscene()` clears the scene
/// (`U2A.C:343-361`, `:351-355`). Every shipped stream gives it none —
/// `READVUE.C` writes `0xFF 0xFF` straight after a frame's `0xFF fov` — so the
/// case is built by hand: `0x80` is "object 0 on" before the terminator.
#[test]
fn the_end_frame_keeps_the_commands_the_stream_places_before_the_terminator() {
    let animation = read_animation(&[0xFF, 0x22, 0x80, 0xFF, 0xFF], 1).unwrap();

    assert_eq!(animation.frames.len(), 2);
    assert!(!animation.frames[0].end);
    assert!(animation.frames[0].ops.is_empty());
    assert!(animation.frames[1].end);
    assert_eq!(animation.frames[1].fov, 0, "the END frame has no fov");
    assert_eq!(
        animation.frames[1].ops.len(),
        1,
        "the command before the terminator is kept"
    );
    assert_eq!(animation.frames[1].ops[0].object, 0);
    assert_eq!(animation.frames[1].ops[0].toggle, Some(true));
}

#[test]
fn the_scene_lists_name_scene_one_and_stop_at_their_sentinel() {
    for name in ["U2A.0AA", "U2E.0AA"] {
        let bytes = asset(name);
        assert_eq!(bytes.len(), 8);
        assert_eq!(read_scene_list(&bytes).unwrap().scenes, [1], "{name}");
    }

    // A zero first word ends the list as surely as 0xFFFF does
    // (`U2A.C:210`'s `while(*ip)`), and neither sentinel is stored.
    assert_eq!(
        read_scene_list(&[2, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0])
            .unwrap()
            .scenes,
        [2, 3]
    );
    assert_eq!(
        read_scene_list(&[2, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0])
            .unwrap()
            .scenes,
        [2]
    );
    // The sentinel pair is two words, and a pair that is cut in half is a
    // truncated file, not a shorter list.
    assert!(read_scene_list(&[2, 0, 1]).is_err());
    assert!(read_scene_list(&[]).is_err());
}

#[test]
fn the_u2a_animation_pins_its_frames_commands_and_toggles() {
    let animation = read_animation(&asset("U2A.0AB"), 6).unwrap();

    assert_eq!(
        animation.frames.len(),
        522,
        "the END-terminated frame included"
    );
    assert_eq!(
        animation
            .frames
            .iter()
            .map(|frame| frame.ops.len())
            .sum::<usize>(),
        1272
    );

    let last = animation.frames.len() - 1;
    for (index, frame) in animation.frames.iter().enumerate() {
        if index == last {
            // 0xFF 0xFF ends the stream without setting a field of vision;
            // the original exits before drawing this frame.
            assert_eq!(frame.fov, 0, "the END-terminated frame");
            assert!(frame.end, "the last frame is the END frame");
            assert!(frame.ops.is_empty(), "the END frame carries no commands");
        } else {
            assert_eq!(frame.fov, 0x22, "frame {index}");
            assert!(!frame.end, "frame {index} is not the END frame");
        }
    }
    assert_eq!(
        animation.frames.iter().filter(|frame| frame.end).count(),
        1,
        "exactly one END frame"
    );
    // Frames 1..=15 are commandless and are not END frames: an empty op list
    // is not a terminator.
    for index in 1..=15 {
        assert!(
            animation.frames[index].ops.is_empty(),
            "frame {index} carries no commands"
        );
        assert!(
            !animation.frames[index].end,
            "commandless frame {index} is not the END frame"
        );
    }

    // Frame 0: one camera command, from the movement the shipped stream
    // applies to the matrix's position, with five matrix entries changed.
    let first = &animation.frames[0];
    assert_eq!(first.ops.len(), 1);
    let camera = &first.ops[0];
    assert_eq!(camera.object, 0);
    assert_eq!(camera.toggle, None);
    assert_eq!((camera.dx, camera.dy, camera.dz), (-221, -323, 7088));
    assert_eq!(
        camera.dm.iter().filter(|delta| delta.is_some()).count(),
        5,
        "five of the nine matrix entries move in frame 0"
    );

    // Frame 16 switches object 2 on and moves it down.
    let toggle_on = animation.frames[16]
        .ops
        .iter()
        .find(|op| op.object == 2 && op.toggle == Some(true))
        .expect("frame 16 turns object 2 on");
    assert_eq!(toggle_on.dy, -9178);

    let toggles = |index: usize| -> Vec<(usize, bool)> {
        animation.frames[index]
            .ops
            .iter()
            .filter_map(|op| op.toggle.map(|on| (op.object, on)))
            .collect()
    };
    assert_eq!(toggles(259), [(1, true), (4, true)]);
    assert_eq!(toggles(270), [(5, true)]);
    assert_eq!(toggles(514), [(1, false)]);
    assert_eq!(toggles(517), [(5, false)]);
    assert_eq!(toggles(519), [(4, false)]);
    assert_eq!(toggles(520), [(5, true)]);
}

#[test]
fn the_u2e_animation_pins_its_frames_and_commands() {
    let animation = read_animation(&asset("U2E.0AB"), 58).unwrap();

    assert_eq!(animation.frames.len(), 1802);
    assert_eq!(
        animation
            .frames
            .iter()
            .map(|frame| frame.ops.len())
            .sum::<usize>(),
        4047
    );
    let last = animation.frames.len() - 1;
    for (index, frame) in animation.frames.iter().enumerate() {
        if index == last {
            assert_eq!(frame.fov, 0, "the END-terminated frame");
            assert!(frame.end, "the last frame is the END frame");
            assert!(frame.ops.is_empty(), "the END frame carries no commands");
        } else {
            assert_eq!(frame.fov, 0x1C, "frame {index}");
            assert!(!frame.end, "frame {index} is not the END frame");
        }
    }
    assert_eq!(
        animation.frames.iter().filter(|frame| frame.end).count(),
        1,
        "exactly one END frame"
    );
    // Thirty of U2E's drawable frames carry no commands; none is an END.
    assert_eq!(
        animation.frames[..last]
            .iter()
            .filter(|frame| frame.ops.is_empty())
            .count(),
        30,
        "the commandless drawable frames"
    );
    assert!(
        animation.frames[..last].iter().all(|frame| !frame.end),
        "only the last frame is END"
    );
}

#[test]
fn the_u2a_object_pins_its_counts_and_its_first_face() {
    let object = read_object(&asset("U2A.001")).unwrap();

    assert_eq!(object.version, 0x0100);
    assert_eq!(object.name, b"\"s01\"", "the file's quotes are kept");
    assert_eq!(object.vertices.len(), 159);
    assert_eq!(object.normals.len(), 269);
    assert_eq!(object.polys.len(), 124);
    assert_eq!(object.lists.len(), 9, "ORD0 plus eight ORDE blocks");

    let face = &object.polys[0];
    assert_eq!(face.sides, 8);
    assert_eq!(face.flags, 0x1C00, "the record's high byte, in bits 8..16");
    assert_eq!(face.color, 32);
    assert_eq!(face.normal, 0);
    assert_eq!(face.vertices, [4, 5, 6, 7, 0, 1, 2, 3]);
    assert_eq!(face.offset, 2, "after the POLY block's leading 0 word");

    // Both list words of U2A.001's ORD0 are sorted on a different vertex; the
    // centre word is the vertex the original sorts the list by.
    assert_eq!(object.lists[0].center, 150);
    assert_eq!(object.lists[0].offsets.len(), 124);
    assert_eq!(object.lists[8].center, 158);
    assert_eq!(object.lists[8].offsets.len(), 124);
}

#[test]
fn the_u2e_object_pins_its_counts_and_its_first_face() {
    let object = read_object(&asset("U2E.001")).unwrap();

    assert_eq!(object.version, 0x0100);
    assert_eq!(object.name, b"\"_platform\"");
    assert_eq!(object.vertices.len(), 29);
    assert_eq!(object.normals.len(), 1);
    assert_eq!(object.polys.len(), 5);
    assert_eq!(object.lists.len(), 9);

    let face = &object.polys[0];
    assert_eq!(face.sides, 4);
    assert_eq!(face.flags, 0x0800);
    assert_eq!(face.color, 16);
    assert_eq!(face.vertices, [0, 1, 2, 3]);
    assert_eq!(face.offset, 2);
}

/// The base derivation, stated as the two offsets that distinguish it: the
/// first face is the block's byte 2, the second is 2 plus the first record's
/// `6 + 2*8` bytes, and the `ORD0` list says the same two numbers.
#[test]
fn the_lists_index_from_the_poly_blocks_first_content_byte() {
    let object = read_object(&asset("U2A.001")).unwrap();

    let first = &object.polys[0];
    let second = &object.polys[1];
    assert_eq!(first.sides, 8);
    assert_eq!(first.offset, 2);
    assert_eq!(
        usize::from(second.offset),
        2 + (6 + 2 * usize::from(first.sides))
    );

    let ord0 = &object.lists[0];
    assert_eq!(ord0.offsets[0], first.offset);
    assert_eq!(ord0.offsets[1], second.offset);

    // And every record in the file is one of the offsets the block's content
    // start names — not one of the offsets a base at the first record would.
    let offsets: Vec<u16> = object.polys.iter().map(|face| face.offset).collect();
    for offset in &ord0.offsets {
        assert!(
            offsets.binary_search(offset).is_ok(),
            "ORD0's offset {offset} is not a face of the block-content base"
        );
    }
}

/// The census the shipped tree holds, and the two properties the objects were
/// read for: every list word names a face, and every `VERT` block's own count
/// is the number of vertices the reader produced.
#[test]
fn every_shipped_object_resolves_its_lists_and_matches_its_own_vertex_count() {
    let objects = shipped_objects();
    assert_eq!(objects.len(), 45, "U2A's three object files and U2E's 42");

    let mut vertices = 0usize;
    let mut normals = 0usize;
    let mut faces = 0usize;
    let mut lists = 0usize;
    for (name, bytes) in &objects {
        let object = read_object(bytes).unwrap_or_else(|e| panic!("{name}: {e}"));

        let vnum =
            u16::from_le_bytes([block_data(bytes, b"VERT")[0], block_data(bytes, b"VERT")[1]]);
        assert_eq!(
            object.vertices.len(),
            vnum as usize,
            "{name}: VERT's own count"
        );
        assert_eq!(object.lists.len(), 9, "{name}");
        assert!(
            object.polys.iter().all(|face| face.sides <= 16),
            "{name}: a face with more than 16 sides"
        );

        let offsets: Vec<u16> = object.polys.iter().map(|face| face.offset).collect();
        for (index, list) in object.lists.iter().enumerate() {
            for offset in &list.offsets {
                assert!(
                    offsets.binary_search(offset).is_ok(),
                    "{name}: list {index} names offset {offset}, which is not a face"
                );
            }
        }

        vertices += object.vertices.len();
        normals += object.normals.len();
        faces += object.polys.len();
        lists += object.lists.len();
    }

    assert_eq!(
        (vertices, normals, faces, lists),
        (2285, 1598, 1206, 405),
        "the shipped objects' totals"
    );
}

/// `VISU.C:49-53` prints "Version not 1.00" and loads the file anyway: the
/// value is data, not a gate.
#[test]
fn a_version_that_is_not_one_hundred_is_stored_not_refused() {
    let mut bytes = asset("U2A.001");
    assert_eq!(&bytes[0..4], b"VERS");
    bytes[8..10].copy_from_slice(&0x0200u16.to_le_bytes());

    let object = read_object(&bytes).unwrap();
    assert_eq!(object.version, 0x0200);
    assert_eq!(
        object.vertices.len(),
        159,
        "the rest of the file still loads"
    );
}

#[test]
fn a_truncated_animation_or_a_command_for_a_missing_object_is_an_error() {
    let full = asset("U2A.0AB");
    for cut in [0, 1, full.len() - 2, full.len() - 1] {
        assert!(
            read_animation(&full[..cut], 6).is_err(),
            "a stream cut at {cut} of {} bytes",
            full.len()
        );
    }

    // A command byte of 0x06 names object 6 with no extensions and no pflag;
    // with conum 6 the loader's `if(onum>=conum) return(3)` is the outcome.
    let error = read_animation(&[0x06, 0xFF, 0x22, 0xFF, 0xFF], 6).unwrap_err();
    assert!(
        matches!(&error, AssetError::Unsupported { what, .. } if what.contains("object 6")),
        "{error}"
    );
    // The last object the scene has is object 5, and it is accepted.
    assert!(read_animation(&[0x05, 0xFF, 0x22, 0xFF, 0xFF], 6).is_ok());
}

#[test]
fn truncated_masters_and_scene_lists_are_errors() {
    let master = asset("U2A.00M");
    for cut in [0, 15, 783, master.len() - 2] {
        assert!(
            read_master(&master[..cut]).is_err(),
            "a master cut at {cut}"
        );
    }
    assert!(read_scene_list(&asset("U2A.0AA")[..3]).is_err());
}

#[test]
fn truncated_and_inconsistent_object_blocks_are_errors() {
    let full = asset("U2A.001");
    for cut in [0, 3, 7, 40, full.len() - 2] {
        assert!(read_object(&full[..cut]).is_err(), "an object cut at {cut}");
    }

    // A list count that claims more words than its block holds is a truncated
    // list, and must not be read past the block.
    let mut bad = full.clone();
    let at = block_at(&bad, b"ORD0").0;
    bad[at..at + 2].copy_from_slice(&0xFFFFu16.to_le_bytes());
    assert!(read_object(&bad).is_err());
}

/// The writer's `endblock` pads a block to four bytes; a face record starts at
/// the `POLY` block's byte 2 and has an even length, so a whole block can end
/// with 0 or 2 trailing bytes and nothing else. One, three, four or five
/// trailing bytes are a malformed block, not padding (four and five cannot
/// hold the six-byte minimum record).
#[test]
fn a_poly_block_whose_trailing_bytes_are_not_the_writers_padding_is_an_error() {
    fn block(tag: &[u8; 4], data: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(8 + data.len());
        bytes.extend_from_slice(tag);
        bytes.extend_from_slice(&(data.len() as u32).to_le_bytes());
        bytes.extend_from_slice(data);
        bytes
    }

    for tail in [1usize, 3, 4, 5] {
        // The leading zero word, then bytes that cannot be a record.
        let mut poly = vec![0u8, 0];
        poly.extend(std::iter::repeat_n(0xAA, tail));
        // A minimal list: its own count, a centre and the final 0.
        let ord0 = [3u16.to_le_bytes(), 0u16.to_le_bytes(), 0u16.to_le_bytes()].concat();
        let object = [
            block(b"VERS", &0x0100u16.to_le_bytes()),
            block(b"POLY", &poly),
            block(b"ORD0", &ord0),
            block(b"END ", &[]),
        ]
        .concat();

        let error = read_object(&object).unwrap_err();
        assert!(
            error.to_string().contains("trailing"),
            "{tail} trailing byte(s): {error}"
        );
    }
}
