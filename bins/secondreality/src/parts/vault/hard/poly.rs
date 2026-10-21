//! HARD's hidden-face test and polygon lists: `checkhiddenbx`
//! (`VEC.ASM:369-400`), `ceasypolylist` (`:583-622`), `cclipeasypolylist`
//! (`:834-902`) and their two colour rules `demo_norm`/`demo_hide`
//! (`:486-507`).
//!
//! HARD's hidden-face test is the **full shoelace**: for every edge
//! `(x_i - x_{i+1}) * (y_i + y_{i+1})` with the difference and the sum taken
//! in 16 bits and the products accumulated in 32, and the sign of the sum is
//! the visibility (positive visible). GLENZ's `checkhiddenbx` computes a
//! cross product of the first three points only, which can disagree with the
//! full sum on the quads part 3 feeds through the clipper.
//!
//! The colour path is index-only. `demo_norm` (`VEC.ASM:486-496`) stores the
//! colour word back unchanged — a no-op — and `demo_hide` (`:498-507`)
//! zeroes it on a hidden face; the compositor paints the word's low byte.
//! `demo_glz` and its DAC shading are dead in both mains (`VEC.ASM:509-580`;
//! only comments call it), so there is no slot allocator here.
//!
//! `cclipeasypolylist` clips each edge against `clipz = 4000`
//! (`VEC.ASM:673`) in 2-D, using the raw x/y words the projection kept, and
//! projects the intersection at z = 4000. It keeps the **near** half: an
//! endpoint is visible when z < 4000 (`xclipsidi` is the reverse of the
//! unused `zclipsidi`). The clipped list starts at the polygon's last vertex
//! (the first edge is last→first), keeps a closing duplicate (the source's
//! drop is commented out at `:883-884`) and is dropped when it has no
//! points. `checkhiddenbx` then runs and the source inverts the carry
//! (`cmc`, `:895`) before calling the colour rule, so `demo_hide` still
//! zeroes the negative-shoelace faces.

use super::math::{PVert, HARD_PROJECTION};

/// `clipz dd 4000` (`VEC.ASM:673`): the clipper's plane.
pub const CLIP_Z: i32 = 4000;

/// `mov eax,080008000h ;both hidden` (`VEC.ASM:687,755`): `xclipsidi`'s
/// sentinel, which `cclipeasypolylist` compares the whole pair against.
const BOTH_HIDDEN: (i16, i16) = (i16::MIN, i16::MIN);

/// One polygon of the source's table — `[count, colour, index, …]` — with the
/// count implicit in [`points`](PolyDef::points).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PolyDef {
    /// The colour word. HARD's tables use `0x4001..=0x4040` (`BP.C:95-110`),
    /// but the colour path reads only the low byte, which is the palette
    /// index.
    pub colour: u16,
    /// Vertex indices into the projected list.
    pub points: &'static [u16],
}

/// One output polygon: the source's `[count, colour, x0, y0, x1, y1, …]`
/// record with the count implicit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Poly {
    /// The colour rule's answer: the palette index (the colour word's low
    /// byte), or 0 for a hidden face under [`FaceMode::Hide`].
    pub colour: u16,
    /// The stored screen pairs, in order.
    pub points: Vec<(i16, i16)>,
}

/// Which `demo_*` rule the list calls: `ceasymode(0)` selects `demo_hide`
/// and `ceasymode(1)` selects `demo_norm` (`VEC.ASM:904-916`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FaceMode {
    /// `demo_norm` (`VEC.ASM:486-496`): the colour is kept on both paths.
    Norm,
    /// `demo_hide` (`VEC.ASM:498-507`): a hidden face's colour word is set
    /// to 0.
    Hide,
}

/// `checkhiddenbx` (`VEC.ASM:369-400`): the full signed shoelace over every
/// edge, closing edge included.
///
/// Each edge is `(x_i - x_{i+1}) * (y_i + y_{i+1})` with the difference and
/// the sum wrapped in 16 bits and the signed 16×16 product accumulated in a
/// wrapping 32-bit sum (`imul dx; add cs:znorm[0],ax; adc cs:znorm[2],dx`).
/// The sign is the visibility: positive is visible (`cmp dx,0; jl` is the
/// hidden branch). For a triangle the sign matches GLENZ's first-three cross
/// product; for a quad it need not.
///
/// A list shorter than two pairs has no edges; the source would read past
/// the record (its `dec cx`/`loop` underflows for a one-point list), which
/// the shipped data does not reach.
pub fn shoelace(points: &[(i16, i16)]) -> i32 {
    if points.is_empty() {
        return 0;
    }
    let mut sum: i32 = 0;
    for (index, &(x, y)) in points.iter().enumerate() {
        let (next_x, next_y) = points[(index + 1) % points.len()];
        // `mov ax,es:[bx+0]; sub ax,es:[bx+4+0]; mov dx,es:[bx+2];
        // add dx,es:[bx+4+2]; imul dx` (`VEC.ASM:378-386`): the difference
        // and the sum wrap in 16 bits, and the signed product joins the
        // 32-bit accumulator through `add`/`adc` (`:387-388`), a wrapping
        // 32-bit add.
        let difference = x.wrapping_sub(next_x);
        let height = y.wrapping_add(next_y);
        sum = sum.wrapping_add(i32::from(difference) * i32::from(height));
    }
    sum
}

/// The colour rule for one face, with the shoelace's sign.
fn face_colour(def: &PolyDef, points: &[(i16, i16)], mode: FaceMode) -> u16 {
    // Positive is visible (`cmp dx,0; jl @@1`, `VEC.ASM:396-398`); the
    // clipper's `cmc` inverts the carry only to match `demo_hide`'s `jc`
    // convention, so both lists share this sign.
    let visible = shoelace(points) >= 0;
    if mode == FaceMode::Hide && !visible {
        // `demo_hide` (`VEC.ASM:498-507`): `xor ax,ax; mov es:[bx-2],ax`
        // zeroes the colour word of a hidden face.
        0
    } else {
        // `demo_norm` (`VEC.ASM:486-496`) stores the colour word back; the
        // pixel path reads only its low byte, so the port carries the
        // palette index alone.
        u16::from(def.colour as u8)
    }
}

/// `ceasypolylist` (`VEC.ASM:583-622`): build each polygon's point list.
///
/// A point whose index equals the one last copied is skipped (`adddot`'s
/// `cmp ax,bp`); the tracker starts at -1 per call and is carried across
/// polygons. After the polygon, the last stored pair is dropped when it
/// equals the first (the closing duplicate). The full [`shoelace`] then picks
/// the colour: [`FaceMode::Norm`] keeps the colour word's low byte, and
/// [`FaceMode::Hide`] sets a hidden face's colour word to 0.
///
/// No live path calls this non-clipping builder — part 3's lists go through
/// [`clip_easy_list`] — so it carries an item-level allow and is kept as the
/// source's `ceasypolylist` port with its test.
#[allow(dead_code)]
pub fn easy_list(defs: &[PolyDef], verts: &[PVert], mode: FaceMode) -> Vec<Poly> {
    let mut polys = Vec::with_capacity(defs.len());
    // `mov bp,-1` (`VEC.ASM:593`): adddot's index tracker, per call and
    // carried across polygons.
    let mut last_index: i32 = -1;
    for def in defs {
        let mut points = Vec::with_capacity(def.points.len());
        for &index in def.points {
            // `cmp ax,bp; je @@3` (`VEC.ASM:353`): a repeated neighbour
            // is not stored.
            if i32::from(index) == last_index {
                continue;
            }
            last_index = i32::from(index);
            let vertex = verts[usize::from(index)];
            points.push((vertex.x, vertex.y));
        }
        // `mov eax,es:[bx]; cmp eax,es:[di-4]; jne @@4; sub di,4`
        // (`VEC.ASM:607-611`): the closing duplicate is dropped.
        if points.len() > 1 && points.first() == points.last() {
            points.pop();
        }
        polys.push(Poly {
            colour: face_colour(def, &points, mode),
            points,
        });
    }
    polys
}

/// `xclipsidi` (`VEC.ASM:743-810`): clip one end of an edge against
/// `clipz`, keeping the near half (z < 4000).
///
/// With both endpoints at or behind the plane the sentinel [`BOTH_HIDDEN`]
/// is returned; with the `si` end already near, its projected pair is
/// returned; otherwise the intersection is interpolated from the raw x/y
/// words (`+12`/`+14`), projected at z = 4000 with the 240/200 multipliers
/// and 160/64 centres, and returned as the `si` end's replacement.
fn clip_end(si: &PVert, di: &PVert) -> (i16, i16) {
    // `xclipsidi` (`VEC.ASM:743-810`) is the reverse clip: the kept half is
    // z < clipz.
    if si.z < CLIP_Z {
        // `;si is visible, just return it.` — the projected pair.
        return (si.x, si.y);
    }
    if di.z >= CLIP_Z {
        // `mov eax,080008000h ;both hidden` (`VEC.ASM:754-755`).
        return BOTH_HIDDEN;
    }

    // `xor eax,eax; div ecx; shr eax,16` (`VEC.ASM:767-770`): the 16.16
    // fraction from si toward di, (zs - clipz) * 65536 / (zs - zd). The
    // quotient cannot overflow 32 bits because zd < clipz <= zs.
    let scaled = ((si.z - CLIP_Z) as u64) << 32;
    let multiplier = ((scaled / (si.z - di.z) as u64) >> 16) as i32;

    // `mov ax,fs:[di+12]; sub ax,fs:[si+12]; movsx eax,ax; imul ecx;
    // shld edx,eax,16; movsx eax,word ptr fs:[si+12]; add edx,eax`
    // (`VEC.ASM:772-781`): the raw difference wraps in the word, the 32x32
    // product shifted right 16 keeps its low 32 bits, and the si-side raw
    // coordinate is added at 32 bits.
    let interpolate = |from: i16, to: i16| -> i32 {
        let difference = to.wrapping_sub(from);
        let product = i64::from(i32::from(difference)) * i64::from(multiplier);
        ((product >> 16) as i32).wrapping_add(i32::from(from))
    };
    let x = interpolate(si.raw_x, di.raw_x);
    let y = interpolate(si.raw_y, di.raw_y);

    // `imul gs:projymul; idiv cs:clipz; add ax,gs:projyadd` and the x pair
    // (`VEC.ASM:791-800`): the intersection is projected at z = clipz.
    let project = |value: i32, mul: i32, add: i16| -> i16 {
        let quotient = (i64::from(value) * i64::from(mul) / i64::from(CLIP_Z)) as i32;
        (quotient as i16).wrapping_add(add)
    };
    (
        project(x, HARD_PROJECTION.mul_x, HARD_PROJECTION.add_x),
        project(y, HARD_PROJECTION.mul_y, HARD_PROJECTION.add_y),
    )
}

/// `eclipsidi` (`VEC.ASM:812-831`): both ends of one edge, `si` first. The
/// source calls `xclipsidi` with the two pointers and then with them
/// swapped (the two `pop`s exchange them); `eax` is the `si` end and `edx`
/// the `di` end.
fn clipped_edge(prev: &PVert, cur: &PVert) -> ((i16, i16), (i16, i16)) {
    (clip_end(prev, cur), clip_end(cur, prev))
}

/// `cclipeasypolylist` (`VEC.ASM:834-902`): clip each polygon against
/// `clipz = 4000` and build its list.
///
/// The first edge is the last index to the first (the tracker starts at the
/// table's last index), and each edge contributes its `prev` end then its
/// `cur` end; [`adddotxy`](clip_easy_list)-style dedupe drops a pair equal to
/// the last one stored. Identical adjacent points are therefore stored once,
/// but the list keeps the closing duplicate (the source's drop is commented
/// out). A polygon that stores no points is dropped. The full [`shoelace`]
/// runs, the source's `cmc` inverts the carry, and the [`FaceMode`] colour
/// rule runs — so, as in [`easy_list`], negative is hidden.
pub fn clip_easy_list(defs: &[PolyDef], verts: &[PVert], mode: FaceMode) -> Vec<Poly> {
    let mut polys = Vec::with_capacity(defs.len());
    for def in defs {
        // `mov ax,ds:[si-2]; mov cs:cllastdi,ax` (`VEC.ASM:854-855`): the
        // tracker starts at the table's last index, so the first edge is
        // last -> first.
        let Some(&last) = def.points.last() else {
            continue;
        };
        let mut previous_index = usize::from(last);
        let mut points: Vec<(i16, i16)> = Vec::new();
        // `mov ebp,-1` (`VEC.ASM:857`): adddotxy's 32-bit adjacent-dedupe
        // tracker, initialised to the pair (-1,-1).
        let mut last_pair = (-1i16, -1i16);
        for &index in def.points {
            let current_index = usize::from(index);
            let (first, second) = clipped_edge(&verts[previous_index], &verts[current_index]);
            previous_index = current_index;
            for pair in [first, second] {
                // The sentinel is compared as a whole pair before adddotxy
                // (`VEC.ASM:871-878`), which then drops a pair equal to the
                // last stored one (`adddotxy`, `:663-669`).
                if pair == BOTH_HIDDEN || pair == last_pair {
                    continue;
                }
                last_pair = pair;
                points.push(pair);
            }
        }
        // `cmp ax,0; jne @@10; mov di,cs:cntoff; sub di,4` (`VEC.ASM:888-891`):
        // a polygon that clipped away entirely is dropped.
        if points.is_empty() {
            continue;
        }
        polys.push(Poly {
            colour: face_colour(def, &points, mode),
            points,
        });
    }
    polys
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vert(x: i16, y: i16, z: i32, raw_x: i16, raw_y: i16) -> PVert {
        PVert {
            x,
            y,
            flags: 0,
            z,
            raw_x,
            raw_y,
        }
    }

    /// The full shoelace, its sign, the first-three cross's disagreement and
    /// the 16/32-bit wraps.
    #[test]
    fn the_shoelace_is_the_full_signed_sum() {
        let square = [(0, 0), (10, 0), (10, 10), (0, 10)];
        assert_eq!(shoelace(&square), 200);
        let mut reversed = square;
        reversed.reverse();
        assert_eq!(shoelace(&reversed), -200);

        // GLENZ's cross product on the first three points is +100, but the
        // fourth point makes the full sum -100: the full sum decides.
        let quad = [(0, 0), (10, 0), (0, 10), (20, 0)];
        assert_eq!(crate::parts::glenz::poly::cross(&quad), 100);
        assert_eq!(shoelace(&quad), -100);

        // Two edges of 2^30 and a closing edge whose 16-bit sum wraps to 0
        // add to 2^31, which stores as i32::MIN.
        let wrapped = [(0, -32768), (-32768, 0), (0, -32768)];
        assert_eq!(shoelace(&wrapped), i32::MIN);

        // Degenerate lists have no edges; the source would read past the
        // record.
        assert_eq!(shoelace(&[]), 0);
        assert_eq!(shoelace(&[(7, 9)]), 0);
    }

    /// `ceasypolylist`: the repeated-neighbour and closing-duplicate drops,
    /// the tracker carried across polygons, the low-byte colour and the
    /// hidden branch.
    #[test]
    fn the_easy_list_is_index_only_and_drops_duplicates() {
        let verts = [
            vert(0, 0, 1000, 0, 0),
            vert(10, 0, 1000, 10, 0),
            vert(0, 10, 1000, 0, 10),
            vert(20, 20, 1000, 20, 20),
        ];
        let defs = [
            PolyDef {
                colour: 0x4001,
                points: &[0, 1, 1, 2, 0],
            },
            PolyDef {
                colour: 0x4002,
                points: &[2, 1, 0],
            },
            PolyDef {
                colour: 0x4003,
                points: &[0, 3, 2],
            },
        ];

        let norm = easy_list(&defs, &verts, FaceMode::Norm);
        assert_eq!(norm.len(), 3);
        // The repeated neighbour 1 and the closing 0 are dropped.
        assert_eq!(norm[0].points, [(0, 0), (10, 0), (0, 10)]);
        // Index-only: the low colour byte, not the 0x40 word.
        assert_eq!(norm[0].colour, 0x01);
        // A reversed winding is hidden; Norm keeps the colour anyway.
        assert_eq!(norm[1].points, [(0, 10), (10, 0), (0, 0)]);
        assert_eq!(norm[1].colour, 0x02);
        // The tracker carries across polygons: this polygon starts with 0,
        // the index the previous one ended on, so its first point is lost.
        assert_eq!(norm[2].points, [(20, 20), (0, 10)]);
        assert_eq!(norm[2].colour, 0x03);

        let hide = easy_list(&defs, &verts, FaceMode::Hide);
        assert_eq!(hide[0].colour, 0x01); // shoelace 100: visible
        assert_eq!(hide[1].colour, 0); // shoelace -100: hidden, zeroed
        assert_eq!(hide[2].colour, 0x03); // shoelace 0: visible
    }

    /// `cclipeasypolylist`: the z = 4000 clip, the raw-x/y intersection, the
    /// adjacent dedupe, the kept closing duplicate, the empty-polygon drop
    /// and the inverted carry.
    #[test]
    fn the_clipper_clips_at_4000_and_projects_the_intersection() {
        let verts = [
            // 0..2: the crossing face. v1 is behind the plane and its raw
            // pair (240,70) differs from its projected pair (200,60), so the
            // intersections must come from +12/+14.
            vert(100, 50, 3000, 100, 50),
            vert(200, 60, 5000, 240, 70),
            vert(150, 150, 3000, 150, 150),
            // 3..5: an all-visible reversed triangle.
            vert(0, 0, 2000, 0, 0),
            vert(10, 0, 2000, 10, 0),
            vert(0, 10, 2000, 0, 10),
            // 6..8: wholly behind the plane, so the polygon is dropped.
            vert(0, 0, 5000, 0, 0),
            vert(10, 10, 6000, 10, 10),
            vert(20, 20, 7000, 20, 20),
        ];
        let defs = [
            PolyDef {
                colour: 0x4007,
                points: &[0, 1, 2],
            },
            PolyDef {
                colour: 0x4009,
                points: &[3, 5, 4],
            },
            PolyDef {
                colour: 0x4011,
                points: &[6, 7, 8],
            },
        ];

        let norm = clip_easy_list(&defs, &verts, FaceMode::Norm);
        assert_eq!(norm.len(), 2, "the wholly-clipped polygon is dropped");
        // Visible ends keep their projected pairs; the crossings interpolate
        // the raw pairs at z = 4000 and project there: (170,60) -> (170,67)
        // and (195,110) -> (171,69). The list starts at the last index (the
        // first edge is last->first) and keeps the closing duplicate: the
        // source's drop is commented out.
        assert_eq!(
            norm[0].points,
            [(150, 150), (100, 50), (170, 67), (171, 69), (150, 150)]
        );
        // Shoelace 6273: visible, and index-only.
        assert_eq!(norm[0].colour, 0x07);
        // The all-visible face keeps its closing duplicate too.
        assert_eq!(norm[1].points, [(10, 0), (0, 0), (0, 10), (10, 0)]);
        assert_eq!(norm[1].colour, 0x09);

        let hide = clip_easy_list(&defs, &verts, FaceMode::Hide);
        // The cmc adapts checkhiddenbx's carry to demo_hide's convention:
        // the visible face survives and the reversed (negative) one is
        // zeroed.
        assert_eq!(hide[0].colour, 0x07);
        assert_eq!(hide[1].colour, 0);
    }
}
