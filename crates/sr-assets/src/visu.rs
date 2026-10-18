//! VISU's packed scene formats: the master `.00M`, the scene list `.0AA`, the
//! animation stream `.0AB`, and the object files `.00N`.
//!
//! The vector engine's own tool wrote these: the `.00M` master, the `.0AA`
//! scene list and the `.00N` block files from `VISU/C/MAIN.C` and `SAVE.C`,
//! and the `.0AB` stream from `VISU/C/READVUE.C:149-255`. The two parts read
//! them back (`VISU/C/U2A.C:148-219`, `U2E.C:208-263`, and the animation
//! decoders at `U2A.C:336-430`, `U2E.C:445-539`). None of them has a magic, so
//! each is *loaded by construction*: the caller knows which of the four it
//! holds, and this module's four functions are the doors.
//!
//! ## The master, `.00M`
//!
//! `MAIN.C:252-321` writes eight zero words, then the 768-byte material
//! palette at 16, then — at the offset stored in the header's bytes 4..8 — the
//! object count and that many minus one object numbers. Byte 15 is the city
//! flag (`MAIN.C:317-320`, `'C'` or `'R'`, zero otherwise), read back as
//! `scene0[15]` by both parts (`U2A.C:168-169`, `U2E.C:224-225`), and the
//! palette is copied out at 16 by both (`U2A.C:243-246`, `U2E.C:337`).
//!
//! The palette is six-bit and is copied verbatim through
//! [`Palette::set_entry`]; it is not run through [`crate::six_bit`], which is
//! the inverse of the engine's display widening (see [`crate::pal`]).
//!
//! ## The scene list, `.0AA`
//!
//! Pairs of little-endian `u16`s. The first word of each pair is a scene
//! number, which names the `.0AB` file (`a/10 + 'A'`, `a%10 + 'A'` —
//! `U2A.C:211-215`); the second is unused. The loader stops at a zero word
//! (`while(*ip)`) or at `0xFFFF` (`if(a==-1) break`), and so does
//! [`read_scene_list`]; neither sentinel is a scene.
//!
//! ## The animation stream, `.0AB`
//!
//! One frame after another, each ended by `0xFF b` with `b <= 0x7F`, which
//! sets the frame's field of vision to `b << 8` (`U2A.C:348`, `U2E.C:457`).
//! The parts pass that 16-bit value to `vid_cameraangle` (`U2A.C:287`,
//! `U2E.C:397`), so [`Frame::fov`] is its high byte and a consumer reconstructs
//! the angle as `u16::from(fov) << 8`. `0xFF 0xFF` ends the stream: the
//! original calls `resetscene()` and sets its exit flag (`U2A.C:351-355`).
//! The frame it terminates carries `end: true`, `fov: 0` and no commands; an
//! empty command list alone does not mean END, because U2A frames 1..=15 and
//! 30 of U2E's drawable frames are legitimately commandless. A command is one
//! byte, optionally extended by a `0xC0`-flagged byte carrying the object's
//! high six bits; its low nibble is the object number and its bits 6..7 toggle
//! that object on or off. Bits 4..5 select the size of a *pflag* byte — zero,
//! one, two or three bytes — and the pflag's bits 0..1, 2..3 and 4..5 select
//! the size of the x, y and z deltas (0, 1, 2 or 4 bytes), bit 6 whether the
//! nine matrix deltas are words or bytes, and bits 7..15 which of those nine
//! entries move (`U2A.C:382-428`). A `0xFF` followed by a byte above `0x7F`
//! other than `0xFF` is not a terminator: the original falls through with that
//! byte and decodes it as a command (`U2A.C:343-361`), which
//! [`read_animation`] mirrors. The deltas are signed and the caller adds them;
//! this module only decodes.
//!
//! An object number at or past `conum` is an error: the original returns 3
//! rather than reading past `co[]` (`U2A.C:337-340`, `U2E.C:491-494`).
//!
//! ## The object, `.00N`
//!
//! `SAVE.C:40-90` writes blocks of a four-byte tag and a little-endian `u32`
//! length, padded to a four-byte boundary: `VERS`, `NAME`, `VERT`, `NORM`,
//! `POLY`, one `ORD0`, any number of `ORDE`, and `END `. `VERS` is stored —
//! `vis_loadobject` prints "Version not 1.00" and loads the file anyway
//! (`VISU.C:49-53`), so a value other than 0x0100 is data, not a refusal.
//! `NAME` is the file's bytes up to its NUL, quotes included. `VERT` is a
//! `u16` count, a `u16` spare and that many 16-byte records; `NORM` is a `u16`
//! count, a `u16` basic-normal count and that many 8-byte records.
//!
//! **A face record is `6 + 2*sides` bytes**: the `sides` byte, the face-flag
//! byte (the record's high byte, [`Face::flags`]), a 16-bit colour, a 16-bit
//! normal index and `sides` 16-bit vertex indices. `CD.H`'s `polydata` comment
//! says "4+2*n", but the shipped records are 6+2*n: U2A.001's first two
//! records are 22 and 14 bytes, and that is where its lists say they are.
//! The `POLY` block's content begins with one zero word, skipped here.
//!
//! **The list base is the `POLY` block's first content byte.** Every shipped
//! `ORD0`/`ORDE` starts with the offset 2, and the first record sits at byte 2
//! of the block's content — one byte past the skipped zero word. The second
//! record of U2A.001 sits at 24, which is what the lists say (2 + 22). Any
//! other base, such as the first record, puts 2 inside a record and misses 24
//! entirely. [`Face::offset`] is therefore the record's offset from that
//! first content byte, and is exactly what a list word names.
//!
//! A list's first word is its **total** number of words, counting itself and
//! the final zero (`C.H`: "number of words in list (including last 0)"). The
//! second word is the centre vertex; the words between it and the final zero
//! are the face offsets. `lists[0]` is `ORD0`, `lists[1..]` the `ORDE` blocks
//! in file order — the original assigns `o->pl[0]` to `ORD0` and
//! `o->pl[plnum++]` to each `ORDE` (`VISU.C:69-80`).
//!
//! ## Errors
//!
//! Every length is bounded against the input before a byte is indexed, and a
//! short or inconsistent file is an [`AssetError`]: a block or record that
//! runs past its bytes is [`AssetError::Truncated`], and a construct the
//! shipped files do not contain (a list that names a byte which is not a
//! face, a missing `ORD0`, an object number past `conum`) is
//! [`AssetError::Unsupported`]. The `VERS` value is the one thing that is
//! deliberately not a gate.

use sr_engine::Palette;

use crate::{AssetError, Format};

/// A little-endian `u16` at `at`; every caller has bounded the read.
fn word(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([bytes[at], bytes[at + 1]])
}

fn truncated(needed: usize, had: usize) -> AssetError {
    AssetError::Truncated {
        format: Format::Visu,
        needed,
        had,
    }
}

fn unsupported(what: String) -> AssetError {
    AssetError::Unsupported {
        format: Format::Visu,
        what,
    }
}

/// `bytes[at..at + len]`, or a [`AssetError::Truncated`] naming the file offset
/// the read would have ended at and the last byte the input actually has.
fn take(bytes: &[u8], at: usize, len: usize) -> Result<&[u8], AssetError> {
    match at.checked_add(len) {
        Some(end) if end <= bytes.len() => Ok(&bytes[at..end]),
        _ => Err(truncated(at.saturating_add(len), bytes.len())),
    }
}

fn u16_at(bytes: &[u8], at: usize) -> Result<u16, AssetError> {
    let bytes = take(bytes, at, 2)?;
    Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
}

fn u32_at(bytes: &[u8], at: usize) -> Result<u32, AssetError> {
    let bytes = take(bytes, at, 4)?;
    Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

// ---------------------------------------------------------------------------
// The master, `.00M`
// ---------------------------------------------------------------------------

/// A decoded `.00M` scene master.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Master {
    /// Byte 15: `'C'` is 1, `'R'` is 2, anything else is 0
    /// (`VISU/C/MAIN.C:317-320`, read back at `U2A.C:168-169`).
    pub city: u8,
    /// The 768 bytes at 16, six bits per component.
    pub palette: Palette,
    /// The `conum - 1` object numbers the header's list holds. The count
    /// itself is not stored here: object 0 is the camera, so the scene's
    /// `conum` is `objects.len() + 1`.
    pub objects: Vec<u16>,
}

/// The fixed part of the `.00M`: eight words then a 768-byte palette.
const MASTER_HEADER: usize = 16;
const MASTER_PALETTE: usize = 16;
const MASTER_PALETTE_LEN: usize = 768;
/// A `u32` at 4 is where the object list begins (`LONGAT(scene0+4)`,
/// `U2A.C:170`).
const MASTER_OBJECTS_AT: usize = 4;

/// Read a `.00M` scene master.
///
/// The palette's 768 bytes are six-bit DAC values copied verbatim, like a
/// `.PAL`'s and unlike an IFF `CMAP`'s; a component above 63 is
/// [`AssetError::Palette`] rather than a masked colour.
pub fn read_master(bytes: &[u8]) -> Result<Master, AssetError> {
    // Bytes 15 and 4..8 are read below, so the whole header comes first.
    take(bytes, 0, MASTER_HEADER)?;
    let city = match bytes[15] {
        b'C' => 1,
        b'R' => 2,
        _ => 0,
    };

    let mut palette = Palette::new();
    for (index, rgb) in take(bytes, MASTER_PALETTE, MASTER_PALETTE_LEN)?
        .chunks_exact(3)
        .enumerate()
    {
        if let Some(&bad) = rgb.iter().find(|&&component| component > 63) {
            return Err(AssetError::Palette {
                what: format!(
                    "the .00M palette entry {index} has the value {bad}, which is not six bits; \
                     MAIN.C:234-241 copies the scene tool's .PAL into the .00M and every shipped \
                     palette holds six-bit values"
                ),
            });
        }
        palette.set_entry(index as u8, [rgb[0], rgb[1], rgb[2]]);
    }

    let objects_at = u32_at(bytes, MASTER_OBJECTS_AT)? as usize;
    let conum = u16_at(bytes, objects_at)? as usize;
    if conum == 0 {
        return Err(unsupported(
            "the object count is 0; object 0 is the camera and every scene has one".to_string(),
        ));
    }
    let list_at = objects_at
        .checked_add(2)
        .ok_or_else(|| truncated(objects_at, bytes.len()))?;
    let list = take(bytes, list_at, 2 * (conum - 1))?;
    let objects = list
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect();

    Ok(Master {
        city,
        palette,
        objects,
    })
}

// ---------------------------------------------------------------------------
// The scene list, `.0AA`
// ---------------------------------------------------------------------------

/// A decoded `.0AA` scene list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SceneList {
    /// The scene numbers, in file order, without the sentinel.
    pub scenes: Vec<u16>,
}

/// Read a `.0AA` scene list: little-endian `u16` pairs whose first word is a
/// scene number, stopped by a zero first word or `0xFFFF`.
pub fn read_scene_list(bytes: &[u8]) -> Result<SceneList, AssetError> {
    let mut scenes = Vec::new();
    let mut at = 0usize;
    loop {
        let pair = take(bytes, at, 4)?;
        let first = u16::from_le_bytes([pair[0], pair[1]]);
        if first == 0 || first == 0xFFFF {
            break;
        }
        scenes.push(first);
        at += 4;
    }
    Ok(SceneList { scenes })
}

// ---------------------------------------------------------------------------
// The animation stream, `.0AB`
// ---------------------------------------------------------------------------

/// One frame of an `.0AB` stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    /// The high byte of the field of vision this frame's `0xFF b` terminator
    /// sets; reconstruct the 16-bit angle as `u16::from(fov) << 8`, which is
    /// what the parts hand to `vid_cameraangle`. The `0xFF 0xFF` frame that
    /// ends a stream sets none, and holds 0; the original exits before drawing
    /// it.
    pub fov: u8,
    /// The commands in file order. Empty in the END frame.
    pub ops: Vec<Op>,
    /// True only for the stream's terminating `0xFF 0xFF` frame, which is
    /// always the last one. An empty [`ops`](Frame::ops) does **not** mean
    /// END: many drawable frames carry no commands.
    pub end: bool,
}

/// One command in an `.0AB` frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Op {
    /// The object's index into the scene's `co[]`, camera first.
    pub object: usize,
    /// `Some(true)` turns the object on, `Some(false)` off, `None` leaves it.
    pub toggle: Option<bool>,
    /// Position deltas, to be added by the caller.
    pub dx: i32,
    pub dy: i32,
    pub dz: i32,
    /// The matrix-entry deltas this command selects, in file order; `None`
    /// where the pflag did not select that entry.
    pub dm: [Option<i32>; 9],
}

/// A decoded `.0AB` animation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Animation {
    pub frames: Vec<Frame>,
}

/// A cursor that bounds every read against the stream.
struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Reader<'a> {
        Reader { bytes, at: 0 }
    }

    fn byte(&mut self) -> Result<u8, AssetError> {
        let byte = take(self.bytes, self.at, 1)?[0];
        self.at += 1;
        Ok(byte)
    }

    fn u16(&mut self) -> Result<u16, AssetError> {
        let bytes = take(self.bytes, self.at, 2)?;
        self.at += 2;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    fn u24(&mut self) -> Result<u32, AssetError> {
        let bytes = take(self.bytes, self.at, 3)?;
        self.at += 3;
        Ok(u32::from(bytes[0]) | (u32::from(bytes[1]) << 8) | (u32::from(bytes[2]) << 16))
    }

    fn i32(&mut self) -> Result<i32, AssetError> {
        let bytes = take(self.bytes, self.at, 4)?;
        self.at += 4;
        Ok(i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }
}

/// `lsget`: the low two bits select 0, 1, 2 or 4 bytes, signed
/// (`U2A.C:38-58`).
fn read_delta(reader: &mut Reader<'_>, selector: u32) -> Result<i32, AssetError> {
    match selector & 3 {
        0 => Ok(0),
        1 => Ok(i32::from(reader.byte()? as i8)),
        2 => Ok(i32::from(reader.u16()? as i16)),
        _ => reader.i32(),
    }
}

/// Read a `.0AB` animation stream for a scene whose objects number `conum`
/// (the camera included).
///
/// The frame terminated by `0xFF 0xFF` is kept, with no field of vision and
/// whatever commands preceded the terminator: every shipped stream ends in
/// one, and it is a frame of the stream even though the original never draws
/// it.
pub fn read_animation(bytes: &[u8], conum: usize) -> Result<Animation, AssetError> {
    let mut reader = Reader::new(bytes);
    let mut frames = Vec::new();

    loop {
        let mut ops = Vec::new();
        // The C declares `onum` outside the command loop and zeroes it once
        // per frame, so a command without a `0xC0` extension keeps the high
        // bits of the object the previous command named (`U2A.C:355-384`).
        let mut onum = 0usize;

        let fov = loop {
            let mut a = reader.byte()?;
            if a == 0xFF {
                let next = reader.byte()?;
                if next <= 0x7F {
                    break next;
                }
                if next == 0xFF {
                    // resetscene() and xit in the original; the stream ends
                    // and this frame is never drawn. READVUE.C:253-254 writes
                    // it with no commands.
                    frames.push(Frame {
                        fov: 0,
                        end: true,
                        ops: Vec::new(),
                    });
                    return Ok(Animation { frames });
                }
                // The original falls through with the byte it just read, and
                // decodes it as a command byte.
                a = next;
            }

            if a & 0xC0 == 0xC0 {
                onum = (usize::from(a & 0x3F)) << 4;
                a = reader.byte()?;
            }
            onum = (onum & 0xFF0) | usize::from(a & 0x0F);

            let toggle = match a & 0xC0 {
                0x80 => Some(true),
                0x40 => Some(false),
                _ => None,
            };
            if onum >= conum {
                return Err(unsupported(format!(
                    "object {onum} is not one of the scene's {conum} objects (the camera is \
                     object 0); U2A.C:337-340 and U2E.C:491-494 return 3 rather than reading past \
                     co[]"
                )));
            }

            let pflag = match (a >> 4) & 3 {
                0 => 0,
                1 => u32::from(reader.byte()?),
                2 => u32::from(reader.u16()?),
                _ => reader.u24()?,
            };

            // x, y and z selectors are the pflag's low three pairs of bits.
            let dx = read_delta(&mut reader, pflag)?;
            let dy = read_delta(&mut reader, pflag >> 2)?;
            let dz = read_delta(&mut reader, pflag >> 4)?;

            let mut dm = [None; 9];
            if pflag & 0x40 != 0 {
                // Word matrix: nine signed 16-bit deltas, in file order.
                for (index, entry) in dm.iter_mut().enumerate() {
                    if pflag & (0x80 << index) != 0 {
                        *entry = Some(read_delta(&mut reader, 2)?);
                    }
                }
            } else {
                // Byte matrix.
                for (index, entry) in dm.iter_mut().enumerate() {
                    if pflag & (0x80 << index) != 0 {
                        *entry = Some(read_delta(&mut reader, 1)?);
                    }
                }
            }

            ops.push(Op {
                object: onum,
                toggle,
                dx,
                dy,
                dz,
                dm,
            });
        };

        frames.push(Frame {
            fov,
            end: false,
            ops,
        });
    }
}

// ---------------------------------------------------------------------------
// The object, `.00N`
// ---------------------------------------------------------------------------

/// A decoded `.00N` object file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectData {
    /// The `VERS` block's first word. Stored, never enforced: the loader only
    /// prints when it is not 0x0100 (`VISU.C:49-53`).
    pub version: u16,
    /// The `NAME` block's bytes up to its NUL. The file's quotes are kept.
    pub name: Vec<u8>,
    pub vertices: Vec<Vertex>,
    pub normals: Vec<Normal>,
    /// The `POLY` block's face records, in file order.
    pub polys: Vec<Face>,
    /// `ORD0` first, then the `ORDE` blocks in file order.
    pub lists: Vec<FaceList>,
}

/// A `vlist`: three 32-bit coordinates, then a `u16` normal index and a `u16`
/// the original reserves (`CD.H`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Vertex {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub normal: i16,
}

/// An `nlist`: three 16-bit components and a reserved word (`CD.H`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Normal {
    pub x: i16,
    pub y: i16,
    pub z: i16,
}

/// One face record of the `POLY` block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Face {
    /// The record's first byte.
    pub sides: u8,
    /// The record's second byte, in bits 8..16 — `F_FLIP`, `F_2SIDE`,
    /// `F_SHADE*`, `F_GOURAUD`, `F_TEXTURE` (`CD.H`).
    pub flags: u16,
    /// The 16-bit colour word that follows the flag byte. The low byte is the
    /// colour; the high byte is the record's reserved byte and is kept as
    /// stored, as the C structure keeps it.
    pub color: i16,
    /// The normal index that follows the colour.
    pub normal: i16,
    pub vertices: Vec<u16>,
    /// The record's byte offset from the first byte of the `POLY` block's
    /// content, which is the base the `ORD0`/`ORDE` words index.
    pub offset: u16,
}

/// One `ORD0` or `ORDE` block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FaceList {
    /// The block's second word: the centre/sort vertex (`VISU.C:106-112`,
    /// `CD.H`'s "sort polygon for this list").
    pub center: u16,
    /// The face offsets, without the list's final zero word.
    pub offsets: Vec<u16>,
}

/// `sides`, `flags`, a 16-bit colour, a 16-bit normal and the vertices.
const FACE_HEADER: usize = 6;
const VERTEX_LEN: usize = 16;
const NORMAL_LEN: usize = 8;

/// A block's data as `(file offset, bytes)`.
type Block = (usize, Vec<u8>);

/// Read a `.00N` object file.
///
/// Unknown block tags are skipped by their declared length, which is what
/// `vis_loadobject` does after printing "Unknown block" (`VISU.C:80-84`);
/// every other structural failure is an [`AssetError`].
pub fn read_object(bytes: &[u8]) -> Result<ObjectData, AssetError> {
    let mut version = 0u16;
    let mut name = Vec::new();
    let mut vertices = Vec::new();
    let mut normals = Vec::new();
    let mut poly: Option<Block> = None;
    let mut ord0: Option<Block> = None;
    let mut ordes: Vec<Block> = Vec::new();

    let mut at = 0usize;
    let mut ended = false;
    while at < bytes.len() {
        let header = take(bytes, at, 8)?;
        let tag = [header[0], header[1], header[2], header[3]];
        let length = u32::from_le_bytes([header[4], header[5], header[6], header[7]]) as usize;
        let data_at = at
            .checked_add(8)
            .ok_or_else(|| truncated(at, bytes.len()))?;
        let data = take(bytes, data_at, length)?;

        match &tag {
            b"VERS" => {
                if length < 2 {
                    return Err(truncated(data_at + 2, data_at + length));
                }
                version = u16::from_le_bytes([data[0], data[1]]);
            }
            b"NAME" => {
                let end = data
                    .iter()
                    .position(|&byte| byte == 0)
                    .unwrap_or(data.len());
                name = data[..end].to_vec();
            }
            b"VERT" => vertices = read_vertices(data, data_at)?,
            b"NORM" => normals = read_normals(data, data_at)?,
            b"POLY" => poly = Some((data_at, data.to_vec())),
            b"ORD0" => ord0 = Some((data_at, data.to_vec())),
            b"ORDE" => ordes.push((data_at, data.to_vec())),
            b"END " => {
                ended = true;
                break;
            }
            // vis_loadobject prints "Unknown block" and steps over it.
            _ => {}
        }
        at = data_at + length;
    }

    if !ended {
        return Err(unsupported(
            "the block stream ends without an `END ` block; VISU.C:25-96 loops until it sees one"
                .to_string(),
        ));
    }
    let (poly_at, poly_data) = poly.ok_or_else(|| {
        unsupported("no POLY block: vis_drawobject passes o->pd to draw_polylist".to_string())
    })?;
    let (ord0_at, ord0_data) = ord0.ok_or_else(|| {
        unsupported(
            "no ORD0 block: the original reads pl[0] as the list that sorts the object".to_string(),
        )
    })?;

    let polys = read_faces(&poly_data, poly_at)?;
    let face_offsets: Vec<u16> = polys.iter().map(|face| face.offset).collect();

    let mut lists = Vec::with_capacity(1 + ordes.len());
    lists.push(read_list(&ord0_data, ord0_at, &face_offsets)?);
    for (at, data) in &ordes {
        lists.push(read_list(data, *at, &face_offsets)?);
    }

    Ok(ObjectData {
        version,
        name,
        vertices,
        normals,
        polys,
        lists,
    })
}

/// `VERT`: a `u16` count, a `u16` spare, then that many 16-byte records.
fn read_vertices(data: &[u8], data_at: usize) -> Result<Vec<Vertex>, AssetError> {
    if data.len() < 4 {
        return Err(truncated(data_at + 4, data_at + data.len()));
    }
    let count = word(data, 0) as usize;
    let needed = 4 + count * VERTEX_LEN;
    if data.len() < needed {
        return Err(truncated(data_at + needed, data_at + data.len()));
    }
    let mut vertices = Vec::with_capacity(count);
    for record in data[4..needed].chunks_exact(VERTEX_LEN) {
        let x = i32::from_le_bytes([record[0], record[1], record[2], record[3]]);
        let y = i32::from_le_bytes([record[4], record[5], record[6], record[7]]);
        let z = i32::from_le_bytes([record[8], record[9], record[10], record[11]]);
        let normal = i16::from_le_bytes([record[12], record[13]]);
        // record[14..16] is the reserved word the C structure carries.
        vertices.push(Vertex { x, y, z, normal });
    }
    Ok(vertices)
}

/// `NORM`: a `u16` count, a `u16` basic-normal count, then that many 8-byte
/// records.
fn read_normals(data: &[u8], data_at: usize) -> Result<Vec<Normal>, AssetError> {
    if data.len() < 4 {
        return Err(truncated(data_at + 4, data_at + data.len()));
    }
    let count = word(data, 0) as usize;
    let needed = 4 + count * NORMAL_LEN;
    if data.len() < needed {
        return Err(truncated(data_at + needed, data_at + data.len()));
    }
    let mut normals = Vec::with_capacity(count);
    for record in data[4..needed].chunks_exact(NORMAL_LEN) {
        let x = i16::from_le_bytes([record[0], record[1]]);
        let y = i16::from_le_bytes([record[2], record[3]]);
        let z = i16::from_le_bytes([record[4], record[5]]);
        // record[6..8] is the reserved word.
        normals.push(Normal { x, y, z });
    }
    Ok(normals)
}

/// The `POLY` block: a skipped zero word, then `6 + 2*sides`-byte records
/// until the block's four-byte-aligned padding.
fn read_faces(poly: &[u8], poly_at: usize) -> Result<Vec<Face>, AssetError> {
    if poly.len() < 2 {
        return Err(truncated(poly_at + 2, poly_at + poly.len()));
    }
    let mut faces = Vec::new();
    let mut at = 2usize;
    while poly.len() - at >= FACE_HEADER {
        let sides = poly[at];
        let record_len = FACE_HEADER + 2 * usize::from(sides);
        if at + record_len > poly.len() {
            return Err(truncated(poly_at + at + record_len, poly_at + poly.len()));
        }
        let offset = u16::try_from(at).map_err(|_| {
            unsupported(format!(
                "a face starts at byte {at} of the POLY block, past the 16-bit offsets the lists \
                 hold"
            ))
        })?;
        let flags = u16::from(poly[at + 1]) << 8;
        let color = i16::from_le_bytes([poly[at + 2], poly[at + 3]]);
        let normal = i16::from_le_bytes([poly[at + 4], poly[at + 5]]);
        let vertices = poly[at + FACE_HEADER..at + record_len]
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .collect();
        faces.push(Face {
            sides,
            flags,
            color,
            normal,
            vertices,
            offset,
        });
        at += record_len;
    }
    // endblock pads a block to a four-byte boundary. Records begin at 2 and
    // have even lengths, so the writer's whole block leaves exactly 0 or 2
    // bytes; 1 or 3 is not a padding the writer makes, and 4 or 5 cannot hold
    // a six-byte record.
    let trailing = poly.len() - at;
    if trailing != 0 && trailing != 2 {
        return Err(unsupported(format!(
            "the POLY block has {trailing} trailing byte(s); the writer pads it to 0 or 2 and \
             nothing else is a face record"
        )));
    }
    Ok(faces)
}

/// One `ORD0`/`ORDE` block: a total word count, a centre vertex, the face
/// offsets, and a final zero that is not stored.
fn read_list(data: &[u8], data_at: usize, faces: &[u16]) -> Result<FaceList, AssetError> {
    // A list is at least its count, a centre and the final zero.
    if data.len() < 6 {
        return Err(truncated(data_at + 6, data_at + data.len()));
    }
    let words = word(data, 0) as usize;
    if words < 3 {
        return Err(unsupported(format!(
            "a list block says it holds {words} word(s); a list is its count, a centre and the \
             final 0"
        )));
    }
    let needed = 2 * words;
    if data.len() < needed {
        return Err(truncated(data_at + needed, data_at + data.len()));
    }
    let list = &data[..needed];
    if word(list, needed - 2) != 0 {
        return Err(unsupported(
            "a list block's last word is not its 0 terminator".to_string(),
        ));
    }
    let center = word(list, 2);
    let offsets: Vec<u16> = list[4..needed - 2]
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect();
    for &offset in &offsets {
        if faces.binary_search(&offset).is_err() {
            return Err(unsupported(format!(
                "a list names byte offset {offset} of the POLY block, which is not the start of a \
                 face; the lists index from the block's first content byte, where the first face \
                 is offset 2"
            )));
        }
    }
    Ok(FaceList { center, offsets })
}
