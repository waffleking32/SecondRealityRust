//! The polygon pipeline: culling, lighting, clipping, span generation and the
//! two fillers, transliterated from `VISU/ADRAW.ASM`, `VISU/ADRAWCLP.ASM`,
//! `VISU/AVIDFILL.ASM`, `VISU/ADATA.ASM` and `VISU/AFILLDIV.INC`.
//!
//! # The fill stream
//!
//! The original writes a `{FILL-DATA}` stream into `_polydrw` and consumes it
//! with a VGA-planar filler: `poly_nrm`/`poly_grd` emit `(flag, colour, X,
//! X-add, Y-count)` bands and `drawfill_nrm`/`drawfill_grd` walk them in the
//! unchained 320x200 mode, where one page row is 80 addresses and one address
//! holds four pixels (one per plane, `x >> 2`, plane `x & 3`). The stream and
//! the plane masks are a VGA implementation detail; this port generates the
//! bands and fills a linear 320-wide page directly. What is preserved is the
//! emitted pixel set and its colours: every planar byte write is expanded to
//! `x = 4 * address + plane` for each plane the map mask selects, so the tiers'
//! 1-, 2- and 4-pixel colour quantisation and every off-by-one they carry
//! come through unchanged. `AFILLDIV.INC`'s `65536/len` slope, the `0x8000`
//! X bias, the increment-before-use row loop and the right-exclusive stop are
//! transliterated literally.
//!
//! The original's `poly1`/`poly2` working buffers hold `MAXPOLYSIDES` = 16
//! vertices each (`AD.INC:51`, `POLYSIZE` at `AD.INC:64`). A 12-sided shipped
//! face (U2E.039) through the near clip and the four 2-D passes can produce
//! more than 16 vertices, and the original writes past the buffer. The port
//! widens the cap to `MAX_POLY_SIDES` and notes the overflow at the guard;
//! every shipped face's clipped list stays inside the widened cap, so shipped
//! output is unchanged.

use sr_assets::visu::Face;

use super::math::{
    project, PVert, Projection, Rotated, RotatedNormal, VF_DOWN, VF_FAR, VF_LEFT, VF_NEAR,
    VF_RIGHT, VF_UP,
};

/// `F_FLIP` (`AD.INC:68`).
///
/// No shipped face carries it and neither part's loop reads it; it is kept
/// with the flag word's other bits because the word is read as a whole.
#[allow(dead_code)]
pub const F_FLIP: u16 = 0x0100;
/// `F_2SIDE` (`AD.INC:69`).
pub const F_2SIDE: u16 = 0x0200;
/// `F_SHADE8` (`AD.INC:70`).
///
/// The three shade constants name the bits `calclight` masks with
/// `F_SHADE32`; U2A's shipped faces use the 16- and 32-colour tiers.
#[allow(dead_code)]
pub const F_SHADE8: u16 = 0x0400;
/// `F_SHADE16` (`AD.INC:71`).
#[allow(dead_code)]
pub const F_SHADE16: u16 = 0x0800;
/// `F_SHADE32` (`AD.INC:72`); the shade bits are `F_SHADE8 | F_SHADE16`.
pub const F_SHADE32: u16 = 0x0C00;
/// `F_GOURAUD` (`AD.INC:73`).
pub const F_GOURAUD: u16 = 0x1000;
/// `F_TEXTURE` (`AD.INC:74`).
///
/// No shipped face sets it and the pipeline has no texture path; kept with
/// the flag word's other bits.
#[allow(dead_code)]
pub const F_TEXTURE: u16 = 0x2000;

/// The mask `calclight` tests (`and ax,F_SHADE32`).
const SHADE_MASK: u16 = F_SHADE32;

/// `MAXPOLYSIDES` (`AD.INC:51`): the original's fixed vertex buffers.
const MAX_FACE_SIDES: usize = 16;

/// The port's widened working-list cap. The original's `poly1`/`poly2` hold
/// exactly `MAXPOLYSIDES` = 16 vertices (`POLYSIZE` = 208 bytes,
/// `AD.INC:64`); a 12-sided shipped face can leave 16 after the near clip
/// alone, and each of the five clipping passes (near plus four 2-D) can at
/// most double an arbitrary list, so the mechanical bound is 12 * 2^5 = 384
/// and 16 * 2^5 = 512. The original writes past the end of `poly2` into the
/// code segment and corrupts the list; 512 keeps the 16-sided worst case in
/// bounds. Shipped faces produce the same vertices as the original and
/// therefore the same pixels.
const MAX_POLY_SIDES: usize = 512;

/// `MAXCOLS` (`A.INC:150`), the Gouraud colour buffer's length.
const MAXCOLS: usize = 640;

/// `NOOVERSAMPLECENTER` (`ADRAW.ASM:129` etc.): the half-pixel in the 16.16 X.
const NO_OVERSAMPLE_CENTER: i32 = 0x8000;

/// The 320x200 destination page.
///
/// The original draws into a VGA page with one byte per address per plane; the
/// port draws into a linear byte page of `width` pixels per row. `pixels`
/// holds `height * width` bytes and is indexed `pixels[y * width + x]`.
pub struct Page<'a> {
    /// The page's pixels, row-major.
    pub pixels: &'a mut [u8],
    /// Pixels per row. The shipped parts use 320.
    pub width: usize,
}

impl<'a> Page<'a> {
    /// A page over `pixels` with `width` pixels per row.
    pub fn new(pixels: &'a mut [u8], width: usize) -> Page<'a> {
        Page { pixels, width }
    }

    /// Write one pixel. A write outside the page is dropped: the original
    /// would land in the next page or off the end of video memory, and after
    /// clipping the shipped pipeline never leaves the window.
    fn set(&mut self, x: i32, y: i32, color: u8) {
        if x < 0 || y < 0 {
            return;
        }
        let (x, y) = (x as usize, y as usize);
        if self.width == 0 || x >= self.width {
            return;
        }
        if let Some(index) = y.checked_mul(self.width).and_then(|row| row.checked_add(x)) {
            if index < self.pixels.len() {
                self.pixels[index] = color;
            }
        }
    }
}

/// `_draw_polylist`'s `POLYFLAGS` (`ADRAW.ASM:465-471`): `or ax,0f00h` then
/// `and ax,cx`, where `cx` is the face record's sides-and-flags word. The
/// face's high byte is kept, bits 8..11 are forced on, and `F_GOURAUD` /
/// `F_TEXTURE` (bits 12/13) survive only if the object enables them.
pub fn poly_flags(face_flags: u16, object_flags: u16) -> u16 {
    (object_flags | 0x0F00) & face_flags
}

/// `checkculling` and the per-face tests around it (`ADRAW.ASM:74-95`,
/// `:474-493`): true when the face draws nothing.
///
/// `screen_flags` is the AND of the face's projected vertices' `VF_*` flags —
/// `calc_project`'s result for the face, or the object's `o->vf` for the
/// whole object (`VISU.C:111-112`). Any shared bit means every vertex is on
/// the same out-of-window side (or behind the near plane), so no pixel can
/// survive the clips. Pass 0 when the caller has not computed it.
pub fn cull(
    face: &Face,
    object_flags: u16,
    rotated_normals: &[RotatedNormal],
    rotated_vertices: &[Rotated],
    screen_flags: u16,
) -> bool {
    // `cmp word ptr ds:[si+2],-1 ; je @@cull2` (`ADRAW.ASM:475-476`): the
    // whole colour word, reserved byte included, is compared.
    if face.color == -1 {
        return true;
    }
    // Any bit shared by every projected vertex puts the whole face outside
    // one window plane (or behind the near plane), where no clip can leave a
    // pixel. This is `vis_drawobject`'s object-level early out
    // (`VISU.C:111-112`) generalised to the face.
    let all_flags = VF_UP | VF_DOWN | VF_LEFT | VF_RIGHT | VF_NEAR | VF_FAR;
    if screen_flags & all_flags != 0 {
        return true;
    }

    let flags = poly_flags(face.flags, object_flags);
    if flags & F_2SIDE != 0 {
        return false;
    }

    // `checkculling` (`ADRAW.ASM:74-95`): the rotated normal read with
    // `movsx` as a signed word, the rotated vertex as a dword, the three
    // 32x32 products accumulated in `ecx:ebp` (64 bits), then `rcl ecx,1 ;
    // cmc`: hidden when the sign bit is clear, i.e. when the dot is >= 0.
    let normal = rotated_normals
        .get(face.normal as usize)
        .copied()
        .unwrap_or_default();
    let first = face
        .vertices
        .first()
        .and_then(|&index| rotated_vertices.get(index as usize))
        .copied()
        .unwrap_or_default();
    let dot = i64::from(normal.x)
        .wrapping_mul(i64::from(first.x))
        .wrapping_add(i64::from(normal.y).wrapping_mul(i64::from(first.y)))
        .wrapping_add(i64::from(normal.z).wrapping_mul(i64::from(first.z)));
    dot >= 0
}

/// `calclight`/`normallight` (`ADRAW.ASM:20-72`): the shade added to a face's
/// base colour.
///
/// `face_flags` is the combined polygon flags ([`poly_flags`]), because
/// `calclight` reads the shade bits from `POLYFLAGS`. `normal` is the rotated
/// normal (`nlist`, 16-bit components, `UNIT` = 16384 for 1.0): `normallight`
/// reads all three components, so a single word cannot carry the parameter.
pub fn light(normal: &RotatedNormal, face_flags: u16) -> u8 {
    let shade = face_flags & SHADE_MASK;
    if shade == 0 {
        // `and ax,F_SHADE32 ; jz @@nc` (`ADRAW.ASM:25-26`) then `xor ax,ax`
        // falls into the 1..=30 clamp, so "no shade" is one, not zero.
        return 1;
    }
    let brightness = normallight(normal);
    // `shr dx,10 ; mov cx,6 ; sub cx,dx ; shr ax,cl` (`:29-34`): the 8-colour
    // band shifts by 5, the 16-colour by 4 and the 32-colour by 3.
    let shift = 6 - (shade >> 10);
    let value = u16::from(brightness) >> shift;
    value.clamp(1, 30) as u8
}

/// `normallight` (`ADRAW.ASM:46-72`): the dot of the rotated normal with
/// `newlight` `(12118,10603,3030)`, taken from the high word and shifted
/// right five more, plus 128, clamped to 0..=255.
fn normallight(normal: &RotatedNormal) -> u8 {
    // Three `imul` products accumulated in 16-bit `bp:cx` (`add`/`adc`); the
    // 32-bit sum is exact for the signed-word inputs, so i32 wrapping is the
    // same bits.
    let dot = i32::from(normal.x)
        .wrapping_mul(12118)
        .wrapping_add(i32::from(normal.y).wrapping_mul(10603))
        .wrapping_add(i32::from(normal.z).wrapping_mul(3030));
    // `mov ax,dx ; sar ax,5 ; add ax,128` (`:61-63`).
    let high = (dot >> 16) as i16;
    let value = i32::from(high >> 5) + 128;
    value.clamp(0, 255) as u8
}

/// Draw one face: build the polygon list, run `newclip`'s five passes and fill.
///
/// `rotated` is the object's rotated vertex list (`vlist`) and `pverts` its
/// projected list (`pvlist`); `face.vertices` indexes both. `gouraud_colors`
/// is `Some` with one 8.8 colour word per face vertex when the combined flags
/// carry `F_GOURAUD` (`(base + shade) << 8`, `ADRAW.ASM:536-560`); `color` is
/// the flat fill's already-lit colour byte (`base + calclight`). The near
/// clip interpolates x/y in 3-D and copies the previous output vertex's
/// colour — `ZCLIPADDVX`, `ADRAWCLP.ASM:229-242`, preserved deliberately.
pub fn clip_and_fill(
    page: &mut Page<'_>,
    face: &Face,
    rotated: &[Rotated],
    pverts: &[PVert],
    projection: &Projection,
    gouraud_colors: Option<&[u16]>,
    color: u8,
) {
    // `_draw_polylist` copies exactly `sides` vertices (`REPT MAXPOLYSIDES`,
    // `ADRAW.ASM:517-532`); a record with more than 16 would make the
    // original overwrite `poly1`'s colour array, so the port takes 16. The
    // shipped maximum is 12 (U2E.039).
    let sides = usize::from(face.sides)
        .min(MAX_FACE_SIDES)
        .min(face.vertices.len());
    if sides == 0 {
        return;
    }

    let mut list = Vec::with_capacity(MAX_FACE_SIDES);
    let mut and_vf = 0xFFFFu16;
    let mut or_vf = 0u16;
    for (i, &index) in face.vertices[..sides].iter().enumerate() {
        let vertex = pverts.get(index as usize).copied().unwrap_or_default();
        and_vf &= vertex.vf;
        or_vf |= vertex.vf;
        push_vert(
            &mut list,
            PolyVert {
                x: vertex.x,
                y: vertex.y,
                gr: gouraud_colors
                    .and_then(|colors| colors.get(i))
                    .copied()
                    .unwrap_or(0),
                vx: index as usize,
            },
        );
    }

    // `newclip` (`ADRAWCLP.ASM:484-551`) in the source's order.
    //
    // `test dh,VF_FAR ; jnz @@cl7`: the AND of every vertex's flags carries
    // VF_FAR only when all of them are beyond the far plane.
    if and_vf & VF_FAR != 0 {
        return;
    }

    let mut vis = or_vf;
    if vis & VF_NEAR != 0 {
        // `clipnear` must run first: the near vertices' projected x/y are
        // undefined, so the 2-D clippers cannot see them yet.
        let (clipped, new_flags) = clip_near(&list, rotated, projection);
        list = clipped;
        vis |= new_flags;
        if list.is_empty() {
            return;
        }
    }

    let interpolate = gouraud_colors.is_some();
    if vis & VF_UP != 0 {
        list = clip_2d(
            &list,
            projection.clip_y[0] as i16,
            Axis::Y,
            true,
            interpolate,
        );
    }
    if vis & VF_DOWN != 0 {
        list = clip_2d(
            &list,
            projection.clip_y[1] as i16,
            Axis::Y,
            false,
            interpolate,
        );
    }
    if vis & VF_LEFT != 0 {
        list = clip_2d(
            &list,
            projection.clip_x[0] as i16,
            Axis::X,
            true,
            interpolate,
        );
    }
    if vis & VF_RIGHT != 0 {
        list = clip_2d(
            &list,
            projection.clip_x[1] as i16,
            Axis::X,
            false,
            interpolate,
        );
    }
    if list.is_empty() {
        return;
    }

    if interpolate {
        fill_gouraud(page, &list);
    } else {
        fill_flat(page, &list, color);
    }
}

// ---------------------------------------------------------------------------
// The working polygon
// ---------------------------------------------------------------------------

/// One vertex of `poly1`/`poly2`: the projected x/y words, the Gouraud colour
/// word (`ADRAW.ASM:553-556`) and the index of the rotated vertex the near
/// clip reads (`POLYVX`; not carried through the 2-D clippers, which never
/// copy it).
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
struct PolyVert {
    x: i16,
    y: i16,
    gr: u16,
    vx: usize,
}

/// Append a vertex unless the widened cap is reached.
///
/// The original's `poly2` is `POLYSIZE` = 208 bytes: room for 16 vertices.
/// The near clip alone can leave 16 out of a 12-gon, and each of the four
/// 2-D passes can add another, so a shipped 12-sided face (U2E.039) can
/// overflow it; the original then writes over the statics that follow. The
/// widened cap keeps the five-pass worst case (16 * 2^5 = 512) representable;
/// a face that somehow exceeds it is truncated rather than allowed to
/// corrupt memory.
fn push_vert(out: &mut Vec<PolyVert>, vertex: PolyVert) {
    if out.len() < MAX_POLY_SIDES {
        out.push(vertex);
    }
}

/// `NEWCLIPCOPY` / `ZCLIPADDVISVX`: copy a vertex, skipping it when its x and
/// y equal the previous output vertex's (`ADRAWCLP.ASM:110-119`, `:212-227`).
///
/// Each clipper writes `0x7FFF` at `POLYX - 4` of its destination before the
/// list (`:259`, `:314`, …), and the adjacent `POLYY - 4` word is never
/// written, so the first output's "previous" vertex is `(0x7FFF, 0)`.
fn push_copy(out: &mut Vec<PolyVert>, vertex: &PolyVert) {
    let previous = out.last().copied().unwrap_or(PolyVert {
        x: 0x7FFF,
        y: 0,
        gr: 0,
        vx: 0,
    });
    if previous.x != vertex.x || previous.y != vertex.y {
        push_vert(out, *vertex);
    }
}

/// The clipped-coordinate axis of one 2-D clipper.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Axis {
    X,
    Y,
}

impl Axis {
    fn get(self, vertex: &PolyVert) -> i16 {
        match self {
            Axis::X => vertex.x,
            Axis::Y => vertex.y,
        }
    }

    fn other(self, vertex: &PolyVert) -> i16 {
        match self {
            Axis::X => vertex.y,
            Axis::Y => vertex.x,
        }
    }

    fn with(self, clipped: i16, other: i16) -> (i16, i16) {
        match self {
            Axis::X => (clipped, other),
            Axis::Y => (other, clipped),
        }
    }
}

// ---------------------------------------------------------------------------
// The five clips
// ---------------------------------------------------------------------------

/// `clipnear` (`ADRAWCLP.ASM:244-296`): drop the vertices behind
/// `projection.clip_z[0]`, interpolating the crossing in 3-D and re-projecting
/// it, and return the OR of the new vertices' `VF_*` flags.
///
/// The boundary vertex's Gouraud colour is the previous *output* vertex's
/// word (`ZCLIPADDVX`, `:229-242`) rather than an interpolation — the
/// original's TODO bug, preserved. For the first output the original reads
/// `poly2[POLYGR - 4]`, the last `POLYVX` word, which no clipper ever writes
/// and which the zero-initialised buffer leaves 0.
fn clip_near(
    src: &[PolyVert],
    rotated: &[Rotated],
    projection: &Projection,
) -> (Vec<PolyVert>, u16) {
    let n = src.len();
    if n == 0 {
        return (Vec::new(), 0);
    }
    let clip_z = projection.clip_z[0];
    // The visible loop's test is inclusive (`cmp ecx,z ; jle @@2`)...
    let visible = |vertex: &PolyVert| {
        rotated
            .get(vertex.vx)
            .is_none_or(|rotated| rotated.z >= clip_z)
    };
    // ... and the hidden loop's is strict (`cmp ecx,z ; jge @@1`): a vertex
    // exactly on the plane stays hidden when it follows a hidden one.
    let still_hidden = |vertex: &PolyVert| {
        rotated
            .get(vertex.vx)
            .is_none_or(|rotated| rotated.z <= clip_z)
    };

    // The source's two loops are one state machine: `@@2` copies a visible
    // vertex and `@@1` skips a hidden one, and a visibility change on an
    // edge emits the crossing before the new vertex is handled. Entering
    // hidden on the first vertex runs `@@1` first, which is why the first
    // crossing is emitted before anything is copied.
    let mut out = Vec::with_capacity(n + 2);
    let mut vis = 0u16;
    let mut si = 0usize;
    let mut count = n + 1;
    let mut in_visible = visible(&src[0]);
    loop {
        if in_visible {
            push_copy(&mut out, &src[si]);
        }
        // `ZCLIPGETVX` (`:196-210`).
        let last = si;
        count -= 1;
        if count < 1 {
            break;
        }
        if count > 1 {
            si += 1;
        } else {
            si = 0;
        }
        let now_visible = if in_visible {
            visible(&src[si])
        } else {
            !still_hidden(&src[si])
        };
        if now_visible != in_visible {
            let (vertex, flags) = near_crossing(&src[last], &src[si], rotated, projection, &out);
            vis |= flags;
            push_copy(&mut out, &vertex);
            in_visible = now_visible;
        }
    }
    (out, vis)
}

/// `ZCLIPCLIP` (`ADRAWCLP.ASM:123-194`) plus `ZCLIPADDVX` (`:229-242`).
fn near_crossing(
    last: &PolyVert,
    current: &PolyVert,
    rotated: &[Rotated],
    projection: &Projection,
    out: &[PolyVert],
) -> (PolyVert, u16) {
    let a = rotated.get(last.vx).copied().unwrap_or_default();
    let b = rotated.get(current.vx).copied().unwrap_or_default();
    // `cmp eax,edx ; jg l1 ; xchg eax,edx ; xchg bx,bp`: bx ends pointing at
    // the farther vertex (the larger z), bp at the nearer.
    let (far, near) = if a.z > b.z { (a, b) } else { (b, a) };
    let denominator = far.z.wrapping_sub(near.z);
    let numerator = far.z.wrapping_sub(projection.clip_z[0]);
    let interpolate_axis = |near_axis: i32, far_axis: i32| -> i32 {
        if denominator == 0 {
            return far_axis;
        }
        let delta = i64::from(near_axis.wrapping_sub(far_axis));
        let scaled = delta.wrapping_mul(i64::from(numerator)) / i64::from(denominator);
        (scaled as i32).wrapping_add(far_axis)
    };
    let x = interpolate_axis(near.x, far.x);
    let y = interpolate_axis(near.y, far.y);

    // The crossing is exactly on the near plane, so `calc_project` runs with
    // the substituted clip z and never sets VF_NEAR or VF_FAR.
    let mut projected = PVert::default();
    project(
        &Rotated {
            x,
            y,
            z: projection.clip_z[0],
            normal: 0,
        },
        &mut projected,
        projection,
    );

    // The preserved bug: `mov ax,ds:[di-4+POLYGR]` copies the previous
    // output vertex's word. With no previous output it reads the
    // zero-initialised `poly2`'s last POLYVX word.
    let gr = out.last().map_or(0, |previous| previous.gr);
    (
        PolyVert {
            x: projected.x,
            y: projected.y,
            gr,
            vx: 0,
        },
        projected.vf,
    )
}

/// One Sutherland-Hodgman pass, transliterated with the original's traversal
/// (`newclipup`/`newclipdown`/`newclipleft`/`newclipright`,
/// `ADRAWCLP.ASM:300-482`).
///
/// The original processes `sides + 1` source entries, wrapping to the first
/// after the last and copying it a second time; the duplicate checks usually
/// suppress it but not when the last source vertex differs from the first,
/// which is where the extra side comes from. `keep_above` selects `>=` (up,
/// left) or `<=` (down, right) as "inside".
fn clip_2d(
    src: &[PolyVert],
    limit: i16,
    axis: Axis,
    keep_above: bool,
    interpolate: bool,
) -> Vec<PolyVert> {
    let n = src.len();
    if n == 0 {
        return Vec::new();
    }
    // The visible loop is inclusive (`jge`/`jle`); the hidden loop is strict
    // (`jle`/`jge`, i.e. equality stays hidden coming from hidden).
    let inside = |vertex: &PolyVert| {
        let coord = axis.get(vertex);
        if keep_above {
            coord >= limit
        } else {
            coord <= limit
        }
    };
    let still_hidden = |vertex: &PolyVert| {
        let coord = axis.get(vertex);
        if keep_above {
            coord <= limit
        } else {
            coord >= limit
        }
    };

    // `@@2` copies a visible vertex, `@@1` skips a hidden one, and a
    // visibility change emits the crossing first; the initial branch enters
    // whichever loop the first vertex belongs to. `sides + 1` source
    // entries are processed, wrapping to the first.
    let mut out = Vec::with_capacity(n + 2);
    let mut si = 0usize;
    let mut count = n + 1;
    let mut in_visible = inside(&src[0]);
    loop {
        if in_visible {
            push_copy(&mut out, &src[si]);
        }
        // `NEWCLIPNEXT` (`:95-108`).
        let last = si;
        count -= 1;
        if count < 1 {
            break;
        }
        if count > 1 {
            si += 1;
        } else {
            si = 0;
        }
        let now_inside = if in_visible {
            inside(&src[si])
        } else {
            !still_hidden(&src[si])
        };
        if now_inside != in_visible {
            let vertex = clip_2d_crossing(&src[last], &src[si], limit, axis, interpolate);
            push_copy(&mut out, &vertex);
            in_visible = now_inside;
        }
    }
    out
}

/// `NEWCLIPCALC` (`ADRAWCLP.ASM:36-93`) and `newclip_grd` (`:24-34`): the
/// edge from `last` (BX) to `current` (SI) meets the limit; interpolate the
/// other coordinate in projected space and the Gouraud word by the same
/// 0..16384 multiplier.
fn clip_2d_crossing(
    last: &PolyVert,
    current: &PolyVert,
    limit: i16,
    axis: Axis,
    interpolate: bool,
) -> PolyVert {
    // `cmp cx,bx ; jl l3 ; xchg` (`ADRAWCLP.ASM:42-51`): cx starts as the
    // *current* vertex's clipped coordinate and bx as the last one; with no
    // swap `small` is the current endpoint, and the `xchg` makes it the last
    // one only when current >= last.
    let (small, large) = if axis.get(current) < axis.get(last) {
        (current, last)
    } else {
        (last, current)
    };
    let small_clipped = axis.get(small);
    let large_clipped = axis.get(large);
    // `mov dx,bp ; xor ax,ax ; shrd ax,dx,2 ; sar dx,2 ; idiv bx`: the
    // 16-bit difference times 16384, divided by the 16-bit extent.
    let numerator = i32::from(limit.wrapping_sub(small_clipped)) * 16384;
    let denominator = i32::from(large_clipped.wrapping_sub(small_clipped));
    let multiplier = if denominator == 0 {
        0
    } else {
        (numerator / denominator) as i16
    };

    let diff = i32::from(large.gr.wrapping_sub(small.gr) as i16);
    // `imul bx ; shld dx,ax,2 ; add dx,ds:[si+POLYGR]`: the 8.8 word, not
    // its high byte, is interpolated.
    let delta = (diff.wrapping_mul(i32::from(multiplier)) >> 14) as i16;
    let gr = if interpolate {
        small.gr.wrapping_add(delta as u16)
    } else {
        0
    };

    let small_other = axis.other(small);
    let large_other = axis.other(large);
    // `imul bx ; shld dx,ax,2`: the 32-bit product's bits 14..29, an
    // arithmetic shift (floor), then the small endpoint added as a word.
    let product =
        i32::from(large_other.wrapping_sub(small_other)).wrapping_mul(i32::from(multiplier));
    let delta_other = (product >> 14) as i16;
    let interpolated = small_other.wrapping_add(delta_other);
    let (x, y) = axis.with(limit, interpolated);
    PolyVert { x, y, gr, vx: 0 }
}

// ---------------------------------------------------------------------------
// Span generation and filling
// ---------------------------------------------------------------------------

/// `POLYSIDECALC` (`ADRAW.ASM:121-148`) with `_projoversampleshr` = 0, as
/// `m1o_init` sets it (`AVIDM1.ASM:92`): the 16.16 X start carries the
/// `0x8000` half-pixel bias and the per-row step is the vertex delta divided
/// by the edge's height with truncation toward zero.
fn side_x(from: &PolyVert, to: &PolyVert, height: i32) -> (i32, i32) {
    let start = (i32::from(from.x) << 16) | NO_OVERSAMPLE_CENTER;
    let end = (i32::from(to.x) << 16) | NO_OVERSAMPLE_CENTER;
    let delta = end.wrapping_sub(start);
    let add = ((i64::from(delta)) / i64::from(height)) as i32;
    (start, add)
}

/// `POLYSIDECALC_GRD`'s colour part (`ADRAW.ASM:261-299`): the from vertex's
/// 8.8 word and `(to - from) / height`, a 16-bit `cwd`/`idiv` division.
fn side_gr(from: &PolyVert, to: &PolyVert, height: i32) -> i16 {
    let diff = to.gr.wrapping_sub(from.gr) as i16;
    (i32::from(diff) / height) as i16
}

/// The original's top-vertex search (`ADRAW.ASM:155-181`, `:306-332`): the
/// first vertex with the strictly smallest Y; returns `(index, min, max)`.
fn top_vertex(verts: &[PolyVert]) -> (usize, i32, i32) {
    let mut top = 0usize;
    let mut min = i32::from(verts[0].y);
    let mut max = min;
    for (index, vertex) in verts.iter().enumerate().skip(1) {
        let y = i32::from(vertex.y);
        if y > max {
            max = y;
        }
        if y < min {
            min = y;
            top = index;
        }
    }
    (top, min, max)
}

/// One flat side segment: the 16.16 X and its per-row add.
#[derive(Clone, Copy, Default)]
struct FlatSide {
    x: i32,
    add: i32,
}

/// One Gouraud side segment: the 8.8 colour word and its per-row add, plus
/// the 16.16 X and its add.
#[derive(Clone, Copy, Default)]
struct GrSide {
    color: u16,
    color_add: i16,
    x: i32,
    x_add: i32,
}

/// `poly_nrm` (`ADRAW.ASM:150-259`) followed by `drawfill_nrm`
/// (`AVIDFILL.ASM:76-184`): walk the two chains from the top vertex and fill
/// each band's rows between them.
///
/// Each row adds the X increments *before* using them (the filler's loop
/// starts with `add vdf_leftx,eax`), and the span stops before the larger X:
/// `dec dx ; don't fill the rightmost pixel`.
fn fill_flat(page: &mut Page<'_>, verts: &[PolyVert], color: u8) {
    let n = verts.len();
    if n == 0 {
        return;
    }
    let (top, min_y, max_y) = top_vertex(verts);
    if min_y == max_y {
        // Height zero: `cmp dx,cx ; je @@d0` overwrites the first band's flag
        // with 0xFFFF (not StartY, which is written earlier at stream offset
        // 2), and the filler returns on the flag's sign bit without drawing a
        // row. Nothing is emitted, as here.
        return;
    }

    let mut left = top;
    let mut right = top;
    let mut left_height = 0i32;
    let mut right_height = 0i32;
    let mut current_y = min_y;
    let mut left_side = FlatSide::default();
    let mut right_side = FlatSide::default();
    let mut y = min_y;

    loop {
        // `@@a4`: walk back from the current left vertex over horizontal
        // edges until the chain turns downwards; an upward turn closes the
        // polygon.
        if i32::from(verts[left].y) == current_y {
            loop {
                let previous = if left == 0 { n - 1 } else { left - 1 };
                let diff = i32::from(verts[previous].y) - i32::from(verts[left].y);
                if diff < 0 {
                    return;
                }
                if diff == 0 {
                    left = previous;
                    continue;
                }
                left_height = diff;
                left_side = {
                    let (x, add) = side_x(&verts[left], &verts[previous], diff);
                    FlatSide { x, add }
                };
                left = previous;
                break;
            }
        }
        // `@@b4`: the same walk forwards for the right chain.
        if i32::from(verts[right].y) == current_y {
            loop {
                let next = (right + 1) % n;
                let diff = i32::from(verts[next].y) - i32::from(verts[right].y);
                if diff < 0 {
                    return;
                }
                if diff == 0 {
                    right = next;
                    continue;
                }
                right_height = diff;
                right_side = {
                    let (x, add) = side_x(&verts[right], &verts[next], diff);
                    FlatSide { x, add }
                };
                right = next;
                break;
            }
        }

        let shorter = left_height.min(right_height);
        if shorter <= 0 {
            return;
        }
        for _ in 0..shorter {
            left_side.x = left_side.x.wrapping_add(left_side.add);
            right_side.x = right_side.x.wrapping_add(right_side.add);
            let left_x = (left_side.x >> 16) as i16;
            let right_x = (right_side.x >> 16) as i16;
            if left_x != right_x {
                // The original swaps when the right X is the smaller one and
                // then decrements the larger: the span runs from the smaller
                // X up to, but not including, the larger.
                let (low, high) = if right_x > left_x {
                    (left_x, right_x)
                } else {
                    (right_x, left_x)
                };
                for x in i32::from(low)..i32::from(high) {
                    page.set(x, y, color);
                }
            }
            y += 1;
        }
        left_height -= shorter;
        right_height -= shorter;
        current_y += shorter;
    }
}

/// `poly_grd` (`ADRAW.ASM:301-410`) followed by `drawfill_grd`
/// (`AVIDFILL.ASM:189-523`): the Gouraud equivalent of [`fill_flat`],
/// including the three accuracy tiers.
fn fill_gouraud(page: &mut Page<'_>, verts: &[PolyVert]) {
    let n = verts.len();
    if n == 0 {
        return;
    }
    let (top, min_y, max_y) = top_vertex(verts);
    if min_y == max_y {
        return;
    }

    let mut left = top;
    let mut right = top;
    let mut left_height = 0i32;
    let mut right_height = 0i32;
    let mut current_y = min_y;
    let mut left_side = GrSide::default();
    let mut right_side = GrSide::default();
    let mut y = min_y;

    loop {
        if i32::from(verts[left].y) == current_y {
            loop {
                let previous = if left == 0 { n - 1 } else { left - 1 };
                let diff = i32::from(verts[previous].y) - i32::from(verts[left].y);
                if diff < 0 {
                    return;
                }
                if diff == 0 {
                    left = previous;
                    continue;
                }
                left_height = diff;
                left_side = {
                    let (x, x_add) = side_x(&verts[left], &verts[previous], diff);
                    GrSide {
                        color: verts[left].gr,
                        color_add: side_gr(&verts[left], &verts[previous], diff),
                        x,
                        x_add,
                    }
                };
                left = previous;
                break;
            }
        }
        if i32::from(verts[right].y) == current_y {
            loop {
                let next = (right + 1) % n;
                let diff = i32::from(verts[next].y) - i32::from(verts[right].y);
                if diff < 0 {
                    return;
                }
                if diff == 0 {
                    right = next;
                    continue;
                }
                right_height = diff;
                right_side = {
                    let (x, x_add) = side_x(&verts[right], &verts[next], diff);
                    GrSide {
                        color: verts[right].gr,
                        color_add: side_gr(&verts[right], &verts[next], diff),
                        x,
                        x_add,
                    }
                };
                right = next;
                break;
            }
        }

        let shorter = left_height.min(right_height);
        if shorter <= 0 {
            return;
        }
        for _ in 0..shorter {
            // `add vdfg_leftc,leftca` and the X adds, before the row's span.
            left_side.color = left_side.color.wrapping_add(left_side.color_add as u16);
            right_side.color = right_side.color.wrapping_add(right_side.color_add as u16);
            left_side.x = left_side.x.wrapping_add(left_side.x_add);
            right_side.x = right_side.x.wrapping_add(right_side.x_add);

            let left_x = (left_side.x >> 16) as i16;
            let right_x = (right_side.x >> 16) as i16;
            if left_x != right_x {
                // After the sort, colour1 belongs to the smaller X and
                // colour2 to the larger; the right address is one before the
                // larger X (right-exclusive).
                let (low, high, color1, color2) = if right_x > left_x {
                    (left_x, right_x, left_side.color, right_side.color)
                } else {
                    (right_x, left_x, right_side.color, left_side.color)
                };
                gouraud_row(page, y, low, high, color1, color2);
            }
            y += 1;
        }
        left_height -= shorter;
        right_height -= shorter;
        current_y += shorter;
    }
}

/// One Gouraud scanline: `drawfill_grd`'s endpoint writes and the tier
/// dispatch. `low`/`high` are the two edges' integer X, with the span ending
/// before `high`.
fn gouraud_row(page: &mut Page<'_>, y: i32, low: i16, high: i16, color1: u16, color2: u16) {
    let low_word = low as u16;
    let right = high.wrapping_sub(1) as u16; // the inclusive right X

    // Endpoints: `test bl,1 ; jz` at the start (write when L is odd) and
    // `test bl,1 ; jnz` at the end (write when R-1 is even).
    if low_word & 1 != 0 {
        page.set(i32::from(low), y, (color1 >> 8) as u8);
    }
    if right & 1 == 0 {
        page.set(i32::from(right as i16), y, (color2 >> 8) as u8);
    }

    // `inc ax ; shr ax,1` and `dec dx ; shr dx,1`: both are 16-bit logical
    // shifts, so a zero right edge wraps to 0x7FFF and makes the length
    // negative (the bulk is skipped).
    let a = low_word.wrapping_add(1) >> 1;
    let b = right.wrapping_sub(1) >> 1;
    let n = b.wrapping_sub(a).wrapping_add(1) as i16;
    if n <= 0 {
        return;
    }
    let n = n as u16;
    // After clipping every vertex is inside the 320-pixel window, so a span
    // is at most 320 pixels wide and n <= 160; the original's colour buffers
    // hold MAXCOLS = 640 entries. This guard is therefore unreachable from
    // clipped shipped data, but an unclipped caller must not index past the
    // buffers (the original would run off them).
    if n > (MAXCOLS / 2) as u16 {
        return;
    }

    let dx = gouraud_slope(color1, color2, n);
    if dx >= 128 || dx <= -128 {
        if dx >= 400 || dx <= -400 {
            gouraud_one_pixel(page, y, a, n, color2, dx);
        } else {
            gouraud_two_pixel(page, y, a, n, color2, dx);
        }
    } else if n < 4 {
        // `cmp cx,4 ; jl gouraud2`: a short span has no four-pixel blocks.
        gouraud_two_pixel(page, y, a, n, color2, dx);
    } else {
        gouraud_four_pixel(page, y, a, n, color2, dx);
    }
}

/// The per-2-pixel colour slope: `|color1 - color2| * (65536 / n) >> 16`,
/// signed by the difference (`AVIDFILL.ASM:304-315`). The `add bx,bx` in the
/// asm converts the word index `n` to a byte offset into `_afilldiv`, so the
/// divisor is `65536/n`; `AFILLDIV.INC`'s first entry saturates at 65535.
fn gouraud_slope(color1: u16, color2: u16, n: u16) -> i16 {
    let delta = color1.wrapping_sub(color2) as i16;
    let magnitude = if delta < 0 {
        delta.wrapping_neg() as u16
    } else {
        delta as u16
    };
    let divisor = if n <= 1 { 65535 } else { 65536 / u32::from(n) };
    let slope = ((u32::from(magnitude).wrapping_mul(divisor) >> 16) as u16) as i16;
    // `neg dx` after the multiply when the left colour is the darker one.
    if delta < 0 {
        slope.wrapping_neg()
    } else {
        slope
    }
}

/// One planar byte: every plane its mask selects at `address` becomes the
/// pixel `4 * address + plane`.
fn write_address(page: &mut Page<'_>, y: i32, address: i32, mask: u8, color: u8) {
    for plane in 0..4i32 {
        if mask & (1 << plane) != 0 {
            page.set(address * 4 + plane, y, color);
        }
    }
}

/// `gouraud1` (`AVIDFILL.ASM:333-389`): one pixel of accuracy, one buffer
/// byte per pixel, four single-plane passes.
#[allow(clippy::too_many_arguments)]
fn gouraud_one_pixel(page: &mut Page<'_>, y: i32, a: u16, n: u16, color2: u16, slope: i16) {
    let dx = slope >> 1; // `sar dx,1`
    let count = 2 * usize::from(n);
    let mut buffer = [0u8; MAXCOLS];
    let mut value = color2;
    for i in 0..count {
        value = value.wrapping_add(dx as u16);
        let zzz = count - 1 - i;
        let index = (zzz & 3) * (MAXCOLS / 4) + (zzz >> 2);
        if index < MAXCOLS {
            buffer[index] = (value >> 8) as u8;
        }
    }

    let mut address = i32::from(a >> 1);
    let mut mask = if a & 1 == 0 { 0x11u8 } else { 0x44u8 };
    // `mov cx,si ; add cx,cx ; ... add cx,3` then one pass per plane, each
    // copying one byte fewer.
    let mut remaining = 2 * u32::from(n) + 3;
    for pass in 0..4 {
        let length = (remaining >> 2) as usize;
        let offset = pass * (MAXCOLS / 4);
        for j in 0..length {
            if let Some(&color) = buffer.get(offset + j) {
                write_address(page, y, address + j as i32, mask, color);
            }
        }
        let carry = mask >> 7;
        mask = mask.rotate_left(1);
        if carry != 0 {
            address += 1;
        }
        remaining -= 1;
    }
}

/// `gouraud2` (`AVIDFILL.ASM:393-446`): two pixels of accuracy, one buffer
/// byte per two-pixel block, two two-plane passes.
#[allow(clippy::too_many_arguments)]
fn gouraud_two_pixel(page: &mut Page<'_>, y: i32, a: u16, n: u16, color2: u16, slope: i16) {
    let count = usize::from(n);
    let mut buffer = [0u8; MAXCOLS];
    let mut value = color2;
    for i in 0..count {
        value = value.wrapping_add(slope as u16);
        let zzz = count - 1 - i;
        let index = (zzz & 1) * (MAXCOLS / 2) + (zzz >> 1);
        if index < MAXCOLS {
            buffer[index] = (value >> 8) as u8;
        }
    }

    let base = i32::from(a >> 1);
    let first_length = usize::from((n + 1) >> 1);
    let second_length = usize::from(n >> 1);
    // The first pass always reads buffer offset 0; the mask phase is the
    // left edge's 2-pixel parity. The second pass reads MAXCOLS/2 and swaps
    // the masks, stepping the address on when the first mask came from the
    // odd phase (`rol al,2 ; jnc`).
    let (first_mask, second_mask, second_base) = if a & 1 == 0 {
        (0x33u8, 0xCCu8, base)
    } else {
        (0xCCu8, 0x33u8, base + 1)
    };
    for (j, &color) in buffer[..first_length].iter().enumerate() {
        write_address(page, y, base + j as i32, first_mask, color);
    }
    for (j, &color) in buffer[MAXCOLS / 2..MAXCOLS / 2 + second_length]
        .iter()
        .enumerate()
    {
        write_address(page, y, second_base + j as i32, second_mask, color);
    }
}

/// `gouraud4` (`AVIDFILL.ASM:450-512`): four pixels of accuracy, one buffer
/// byte per four-pixel address, one all-plane pass.
#[allow(clippy::too_many_arguments)]
fn gouraud_four_pixel(page: &mut Page<'_>, y: i32, a: u16, n: u16, color2: u16, slope: i16) {
    let dx = (slope as u16) << 1; // `sal dx,1`
    let q = usize::from(n >> 1);
    let mut buffer = [0u8; MAXCOLS + 1];
    // `mov bx,color2 ; sub bx,dx`: the extra entry is one step before the
    // right colour.
    let mut value = color2.wrapping_sub(dx);
    for i in 0..=q {
        value = value.wrapping_add(dx);
        buffer[q - i] = (value >> 8) as u8;
    }

    let mut address = i32::from(a >> 1);
    let mut offset = 0usize;
    if a & 1 != 0 {
        // `@@f3`: the first two pixels are the right half of an address.
        write_address(page, y, address, 0x0C, buffer[0]);
        address += 1;
        offset = 1;
    }
    let mut count = n;
    if a & 1 == 0 {
        count += 1;
    }
    let carry = count & 1;
    let count = count >> 1;
    if carry == 1 {
        for j in 0..usize::from(count) {
            if let Some(&color) = buffer.get(offset + j) {
                write_address(page, y, address + j as i32, 0x0F, color);
            }
        }
    } else {
        // `@@f5`: one address fewer with all planes, then two pixels.
        let length = usize::from(count.saturating_sub(1));
        for j in 0..length {
            if let Some(&color) = buffer.get(offset + j) {
                write_address(page, y, address + j as i32, 0x0F, color);
            }
        }
        if let Some(&color) = buffer.get(offset + length) {
            write_address(page, y, address + length as i32, 0x03, color);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parts::visu::math;

    const W: usize = 320;
    const H: usize = 200;

    fn blank() -> Vec<u8> {
        vec![0u8; W * H]
    }

    fn face(vertices: &[u16], flags: u16) -> Face {
        Face {
            sides: vertices.len() as u8,
            flags,
            color: 0x0A,
            normal: 0,
            vertices: vertices.to_vec(),
            offset: 0,
        }
    }

    fn pv(x: i16, y: i16) -> PVert {
        PVert { x, y, vf: 0 }
    }

    /// The projection used by the flat and Gouraud span tests: the window is
    /// the full 320x200 page, and the projected vertices are supplied
    /// directly.
    fn page_projection() -> Projection {
        Projection {
            mul_x: 0,
            mul_y: 0,
            add_x: 0,
            add_y: 0,
            clip_x: [0, 319],
            clip_y: [0, 199],
            clip_z: [512, 1_000_000_000],
        }
    }

    fn row(pixels: &[u8], y: usize, len: usize) -> &[u8] {
        &pixels[y * W..y * W + len]
    }

    fn run_flat(face: &Face, pverts: &[PVert], color: u8, projection: &Projection) -> Vec<u8> {
        let mut pixels = blank();
        {
            let mut page = Page::new(&mut pixels, W);
            clip_and_fill(&mut page, face, &[], pverts, projection, None, color);
        }
        pixels
    }

    fn run_gouraud(
        face: &Face,
        rotated: &[Rotated],
        pverts: &[PVert],
        colors: &[u16],
        projection: &Projection,
    ) -> Vec<u8> {
        let mut pixels = blank();
        {
            let mut page = Page::new(&mut pixels, W);
            clip_and_fill(
                &mut page,
                face,
                rotated,
                pverts,
                projection,
                Some(colors),
                0,
            );
        }
        pixels
    }

    /// A vertex with its `VF_*` flags derived from the projection's window.
    fn flagged(projection: &Projection, x: i16, y: i16) -> PVert {
        let mut vf = 0;
        if i32::from(x) < projection.clip_x[0] {
            vf |= VF_LEFT;
        }
        if i32::from(x) > projection.clip_x[1] {
            vf |= VF_RIGHT;
        }
        if i32::from(y) < projection.clip_y[0] {
            vf |= VF_UP;
        }
        if i32::from(y) > projection.clip_y[1] {
            vf |= VF_DOWN;
        }
        PVert { x, y, vf }
    }

    // ---------------------------------------------------------------------
    // Flat spans
    // ---------------------------------------------------------------------

    #[test]
    fn flat_triangle_fills_the_hand_computed_rows() {
        // `poly_nrm` on (0,0), (4,0), (0,4). The top vertex is v0; the left
        // edge is v0->v2 (x = 0, no increment) and the right is v1->v2, whose
        // slope is (0<<16 + 0x8000 - (4<<16 + 0x8000)) / 4 = -0x10000.
        // The filler adds the increment before the first row, so the row at
        // y = 0 uses the right-edge X at y = 1: 0x48000 - 0x10000 = 0x38000,
        // high word 3, and the row stops before it: x = 0..=2. Rows 1 and 2
        // follow one pixel in, and row 3 starts and ends at x = 0 (skipped).
        let projection = page_projection();
        let f = face(&[0, 1, 2], 0);
        let pixels = run_flat(&f, &[pv(0, 0), pv(4, 0), pv(0, 4)], 9, &projection);
        assert_eq!(row(&pixels, 0, 5), &[9, 9, 9, 0, 0]);
        assert_eq!(row(&pixels, 1, 5), &[9, 9, 0, 0, 0]);
        assert_eq!(row(&pixels, 2, 5), &[9, 0, 0, 0, 0]);
        assert_eq!(row(&pixels, 3, 5), &[0, 0, 0, 0, 0]);
    }

    #[test]
    fn flat_quad_pins_the_half_pixel_bias_and_the_row_increment() {
        // (0,0), (5,0), (2,2), (0,2). The right edge runs from x = 5 to x = 2
        // over two rows with slope -1.5. Its 16.16 start is
        // 5<<16 | 0x8000; the first row adds -0x18000 and the high word is 4,
        // where an unbiased 5.0 - 1.5 = 3.5 would truncate to 3. The second
        // row lands on 2 and stops before it (x = 0..=1).
        let projection = page_projection();
        let f = face(&[0, 1, 2, 3], 0);
        let pixels = run_flat(
            &f,
            &[pv(0, 0), pv(5, 0), pv(2, 2), pv(0, 2)],
            5,
            &projection,
        );
        assert_eq!(row(&pixels, 0, 6), &[5, 5, 5, 5, 0, 0]);
        assert_eq!(row(&pixels, 1, 6), &[5, 5, 0, 0, 0, 0]);
        assert_eq!(row(&pixels, 2, 6), &[0; 6]);
    }

    #[test]
    fn flat_quad_is_right_exclusive() {
        // A 4x4 square with vertical edges: every row fills x = 0..=3 and
        // never the right edge x = 4 (the filler decrements the right X).
        let projection = page_projection();
        let f = face(&[0, 1, 2, 3], 0);
        let pixels = run_flat(
            &f,
            &[pv(0, 0), pv(4, 0), pv(4, 4), pv(0, 4)],
            7,
            &projection,
        );
        for y in 0..4 {
            assert_eq!(row(&pixels, y, 6), &[7, 7, 7, 7, 0, 0], "row {y}");
        }
        assert_eq!(row(&pixels, 4, 6), &[0; 6]);
    }

    #[test]
    fn flat_quad_persists_a_side_across_bands() {
        // (0,0), (4,0), (4,2), (0,4): the left edge is four rows tall and
        // the right chain's first segment only two, so the stream's second
        // band keeps the left side (`flag 0`) while it recomputes the right
        // one from (4,2) to (0,4). The first two rows span 0..=3; the third
        // spans 0..=1; the fourth starts and ends at 0 and is skipped.
        let projection = page_projection();
        let f = face(&[0, 1, 2, 3], 0);
        let pixels = run_flat(
            &f,
            &[pv(0, 0), pv(4, 0), pv(4, 2), pv(0, 4)],
            9,
            &projection,
        );
        assert_eq!(row(&pixels, 0, 5), &[9, 9, 9, 9, 0]);
        assert_eq!(row(&pixels, 1, 5), &[9, 9, 9, 9, 0]);
        assert_eq!(row(&pixels, 2, 5), &[9, 9, 0, 0, 0]);
        assert_eq!(row(&pixels, 3, 5), &[0; 5]);
    }

    #[test]
    fn flat_three_four_five_triangle_truncates_the_slope() {
        // (0,0), (4,0), (0,3): the right edge's 16.16 adder is
        // -0x40000 / 3, which `idiv` truncates to -0x15555 = -87381 (the
        // floor would be -87382). Row 0: 0x48000 - 0x15555 = 0x32AAB, high
        // word 3, so x = 0..=2; row 1 reaches 1 -> x = 0..=0; row 2 reaches
        // 0 and is skipped.
        let projection = page_projection();
        let f = face(&[0, 1, 2], 0);
        let pixels = run_flat(&f, &[pv(0, 0), pv(4, 0), pv(0, 3)], 9, &projection);
        assert_eq!(row(&pixels, 0, 5), &[9, 9, 9, 0, 0]);
        assert_eq!(row(&pixels, 1, 5), &[9, 0, 0, 0, 0]);
        assert_eq!(row(&pixels, 2, 5), &[0; 5]);
        // The truncating division itself, not just the rows it produced.
        let from = PolyVert {
            x: 4,
            y: 0,
            gr: 0,
            vx: 0,
        };
        let to = PolyVert {
            x: 0,
            y: 3,
            gr: 0,
            vx: 0,
        };
        assert_eq!(side_x(&from, &to, 3).1, -87381);
    }

    #[test]
    fn zero_height_polygon_draws_nothing() {
        // `cmp dx,cx ; je @@d0` replaces the first band flag with 0xFFFF;
        // the filler's `or ax,ax ; js` returns before any row. Nothing is
        // drawn.
        let projection = page_projection();
        let f = face(&[0, 1, 2], 0);
        let pixels = run_flat(&f, &[pv(0, 5), pv(4, 5), pv(2, 5)], 9, &projection);
        assert!(pixels.iter().all(|&pixel| pixel == 0));
    }

    // ---------------------------------------------------------------------
    // Gouraud spans and the three accuracy tiers
    // ---------------------------------------------------------------------

    #[test]
    fn gouraud_square_uses_the_one_pixel_tier() {
        // A 4x4 square, colours 0x1000/0x2000/0x3000/0x4000 at the corners.
        // The edge colour adders are (0x4000-0x1000)/4 = 0xC00 and
        // (0x3000-0x2000)/4 = 0x400; the filler adds them before row 0.
        // Row 0: left 0x1C00, right 0x2400, dx = -0x800/(4/2) = -0x400,
        // which selects gouraud1; its halved adder 0x200 walks 28, 30, 32,
        // 34. Row 1 is flat 40. Row 2 is the positive slope 52, 50, 48, 46.
        // Row 3: 64, 60, 56, 52.
        let projection = page_projection();
        let f = face(&[0, 1, 2, 3], F_GOURAUD | F_SHADE32);
        let rotated = [];
        let colors = [0x1000, 0x2000, 0x3000, 0x4000];
        let pixels = run_gouraud(
            &f,
            &rotated,
            &[pv(0, 0), pv(4, 0), pv(4, 4), pv(0, 4)],
            &colors,
            &projection,
        );
        assert_eq!(row(&pixels, 0, 4), &[28, 30, 32, 34]);
        assert_eq!(row(&pixels, 1, 4), &[40, 40, 40, 40]);
        assert_eq!(row(&pixels, 2, 4), &[52, 50, 48, 46]);
        assert_eq!(row(&pixels, 3, 4), &[64, 60, 56, 52]);
        assert_eq!(row(&pixels, 4, 4), &[0; 4]);
    }

    #[test]
    fn gouraud_square_uses_the_two_pixel_tier() {
        // An 8x4 square whose left edge is 0x1000 and right edge 0x1400, so
        // every row is the same: n = 4 two-pixel blocks, dx = -0x400/4 =
        // -0x100, which is in [128, 400) -> gouraud2. The buffer's entries
        // are 16, 18 in the even segment and 17, 19 in the odd one; the two
        // address passes write even and odd blocks, giving 2-pixel plateaus.
        let projection = page_projection();
        let f = face(&[0, 1, 2, 3], F_GOURAUD | F_SHADE32);
        let colors = [0x1000, 0x1400, 0x1400, 0x1000];
        let pixels = run_gouraud(
            &f,
            &[],
            &[pv(0, 0), pv(8, 0), pv(8, 4), pv(0, 4)],
            &colors,
            &projection,
        );
        for y in 0..4 {
            assert_eq!(
                row(&pixels, y, 8),
                &[16, 16, 17, 17, 18, 18, 19, 19],
                "row {y}"
            );
        }
        assert_eq!(row(&pixels, 4, 8), &[0; 8]);
    }

    #[test]
    fn gouraud_square_uses_the_four_pixel_tier() {
        // Same square, left edge 0x1010 and right edge 0x1200: dx = -124,
        // inside (-128, 128), and n = 4 so gouraud4 is not downgraded. Its
        // doubled adder spreads one colour over each 4-pixel address; the
        // two buffer bytes it copies are 16 and 17.
        let projection = page_projection();
        let f = face(&[0, 1, 2, 3], F_GOURAUD | F_SHADE32);
        let colors = [0x1010, 0x1200, 0x1200, 0x1010];
        let pixels = run_gouraud(
            &f,
            &[],
            &[pv(0, 0), pv(8, 0), pv(8, 4), pv(0, 4)],
            &colors,
            &projection,
        );
        for y in 0..4 {
            assert_eq!(
                row(&pixels, y, 8),
                &[16, 16, 16, 16, 17, 17, 17, 17],
                "row {y}"
            );
        }
    }

    #[test]
    fn gouraud_two_pixel_tier_handles_an_odd_left_edge() {
        // x = 1..11 with constant edge colours 0x1000 and 0x1400. The left
        // edge is at block 1 (A = (1+1)>>1 = 1, odd), so the first pass uses
        // the 0xCC mask and the second pass steps its address on: the
        // two-pixel colour blocks are phase-shifted by one address. n = 4,
        // dx = -0x400/4 = -256 -> gouraud2; the buffer holds 16, 18 in the
        // even segment and 17, 19 in the odd one. The endpoint at x = 1
        // takes colour1 (16) and the even right endpoint x = 10 takes
        // colour2 (20).
        let projection = page_projection();
        let f = face(&[0, 1, 2, 3], F_GOURAUD | F_SHADE32);
        let colors = [0x1000, 0x1400, 0x1400, 0x1000];
        let pixels = run_gouraud(
            &f,
            &[],
            &[pv(1, 0), pv(11, 0), pv(11, 4), pv(1, 4)],
            &colors,
            &projection,
        );
        for y in 0..4 {
            assert_eq!(
                row(&pixels, y, 12),
                &[0, 16, 16, 16, 17, 17, 18, 18, 19, 19, 20, 0],
                "row {y}"
            );
        }
    }

    #[test]
    fn gouraud_four_pixel_tier_handles_an_odd_left_edge() {
        // x = 1..12 with edge colours 0x1010 and 0x1110: n = 5, dx = -51,
        // so gouraud4. The odd left edge makes the first address write two
        // pixels (mask 0x0C) and the next two write four each (mask 0x0F);
        // the endpoint at x = 1 takes colour1, and x = 11 is the last bulk
        // pixel (R-1 = 11 is odd, so no right endpoint).
        let projection = page_projection();
        let f = face(&[0, 1, 2, 3], F_GOURAUD | F_SHADE32);
        let colors = [0x1010, 0x1110, 0x1110, 0x1010];
        let pixels = run_gouraud(
            &f,
            &[],
            &[pv(1, 0), pv(12, 0), pv(12, 4), pv(1, 4)],
            &colors,
            &projection,
        );
        for y in 0..4 {
            assert_eq!(
                row(&pixels, y, 13),
                &[0, 16, 16, 16, 16, 16, 16, 16, 17, 17, 17, 17, 0],
                "row {y}"
            );
        }
    }

    #[test]
    fn gouraud_one_pixel_tier_handles_an_odd_left_edge() {
        // x = 1..5 with constant edge colours 0x1000 and 0x1800: n = 1 and
        // the slope is -2047 (the 65536/1 entry), so gouraud1. The halved
        // adder -0x400 gives 16 and 20 for the two buffer entries; the odd
        // left edge starts the mask phase at 0x44 and leaves the carry to
        // step the address only after 0x88. The endpoints are 16 at x = 1
        // and 24 at x = 4.
        let projection = page_projection();
        let f = face(&[0, 1, 2, 3], F_GOURAUD | F_SHADE32);
        let colors = [0x1000, 0x1800, 0x1800, 0x1000];
        let pixels = run_gouraud(
            &f,
            &[],
            &[pv(1, 0), pv(5, 0), pv(5, 4), pv(1, 4)],
            &colors,
            &projection,
        );
        for y in 0..4 {
            assert_eq!(row(&pixels, y, 6), &[0, 16, 16, 20, 24, 0], "row {y}");
        }
    }

    // ---------------------------------------------------------------------
    // The near clip and its preserved colour-copy bug
    // ---------------------------------------------------------------------

    #[test]
    fn near_clip_copies_the_previous_output_colour() {
        // A quad whose bottom two vertices are behind the near plane
        // (clip_z[0] = 512; the visible pair is at z = 1024, the hidden pair
        // at z = 0). The 3-D crossings at z = 512 project to (4,2) and
        // (0,2), cutting the quad into a 4x2 rectangle in projected space.
        //
        // The crossing after v1 takes v1's colour word 0x2000 (the previous
        // output vertex), not an interpolated 0x2000 -> 0x4000 colour; the
        // second crossing then takes the first crossing's colour, so both
        // left-edge endpoints are 0x2000. The left edge of the fill runs
        // 0x1000 -> 0x2000 over two rows, so row 0 starts at 0x1800 = 24;
        // the right edge is constant 0x2000. The span's -8 colour units over
        // four pixels select the 1-pixel tier: 24, 26, 28, 30.
        //
        // A correct interpolation would have given the crossings
        // (0x2000 + 0x4000) / 2 = 0x3000 and (0x6000 + 0x1000) / 2 =
        // 0x3800, and the row would not be 24..30.
        let projection = Projection {
            mul_x: 1024,
            mul_y: 1024,
            add_x: 0,
            add_y: 0,
            clip_x: [0, 319],
            clip_y: [0, 199],
            clip_z: [512, 1_000_000_000],
        };
        let f = face(&[0, 1, 2, 3], F_GOURAUD | F_SHADE32);
        let rotated = [
            Rotated {
                x: 0,
                y: 0,
                z: 1024,
                normal: 0,
            },
            Rotated {
                x: 4,
                y: 0,
                z: 1024,
                normal: 0,
            },
            Rotated {
                x: 0,
                y: 2,
                z: 0,
                normal: 0,
            },
            Rotated {
                x: 0,
                y: 2,
                z: 0,
                normal: 0,
            },
        ];
        let pverts = [
            pv(0, 0),
            pv(4, 0),
            PVert {
                x: 0,
                y: 0,
                vf: VF_NEAR,
            },
            PVert {
                x: 0,
                y: 0,
                vf: VF_NEAR,
            },
        ];
        let colors = [0x1000, 0x2000, 0x4000, 0x6000];
        let pixels = run_gouraud(&f, &rotated, &pverts, &colors, &projection);
        assert_eq!(row(&pixels, 0, 4), &[24, 26, 28, 30]);
        // The left edge adds 0x800 per row, so the second row starts at
        // 0x2000 and matches the constant right edge: a flat 32.
        assert_eq!(row(&pixels, 1, 4), &[32, 32, 32, 32]);
        assert_eq!(row(&pixels, 2, 4), &[0; 4]);
    }

    #[test]
    fn near_clip_first_output_crossing_takes_the_zero_colour() {
        // The face's first two vertices are behind the near plane, so the
        // first output is the crossing on the hidden -> visible edge and
        // `ZCLIPADDVX` has no previous output vertex: it reads `poly2`'s last
        // POLYVX word, which no clipper writes and which is 0. The crossing
        // at (0,2) therefore carries colour 0. The left edge of the fill runs
        // 0x1000 -> 0 over two rows: row 0's span colours are 8 and 32, and
        // the -0x1800 slope (1-pixel tier) gives 8, 14, 20, 26; row 1's
        // 0 -> 0x2000 slope gives 0, 8, 16, 24.
        let projection = Projection {
            mul_x: 1024,
            mul_y: 1024,
            add_x: 0,
            add_y: 0,
            clip_x: [0, 319],
            clip_y: [0, 199],
            clip_z: [512, 1_000_000_000],
        };
        let f = face(&[0, 1, 2, 3], F_GOURAUD | F_SHADE32);
        let rotated = [
            Rotated {
                x: 0,
                y: 2,
                z: 0,
                normal: 0,
            },
            Rotated {
                x: 0,
                y: 2,
                z: 0,
                normal: 0,
            },
            Rotated {
                x: 0,
                y: 0,
                z: 1024,
                normal: 0,
            },
            Rotated {
                x: 4,
                y: 0,
                z: 1024,
                normal: 0,
            },
        ];
        let pverts = [
            PVert {
                x: 0,
                y: 0,
                vf: VF_NEAR,
            },
            PVert {
                x: 0,
                y: 0,
                vf: VF_NEAR,
            },
            pv(0, 0),
            pv(4, 0),
        ];
        let colors = [0x4000, 0x6000, 0x1000, 0x2000];
        let pixels = run_gouraud(&f, &rotated, &pverts, &colors, &projection);
        assert_eq!(row(&pixels, 0, 4), &[8, 14, 20, 26]);
        assert_eq!(row(&pixels, 1, 4), &[0, 8, 16, 24]);
        assert_eq!(row(&pixels, 2, 4), &[0; 4]);
    }

    // ---------------------------------------------------------------------
    // One polygon crossing each window edge
    // ---------------------------------------------------------------------

    #[test]
    fn clip_crossing_the_right_edge() {
        // x = 20..340 with the right pair outside; the right clip cuts at
        // 319, leaving the vertical rectangle x = 20..319. The fill stops
        // before 319, so every row is 20..=318.
        let projection = page_projection();
        let f = face(&[0, 1, 2, 3], 0);
        let pverts = [
            flagged(&projection, 20, 0),
            flagged(&projection, 340, 0),
            flagged(&projection, 340, 4),
            flagged(&projection, 20, 4),
        ];
        let pixels = run_flat(&f, &pverts, 3, &projection);
        for y in 0..4 {
            assert_eq!(pixels[y * W + 19], 0, "row {y} left of the span");
            assert_eq!(pixels[y * W + 20], 3, "row {y} first");
            assert_eq!(pixels[y * W + 318], 3, "row {y} last");
            assert_eq!(pixels[y * W + 319], 0, "row {y} right edge");
        }
        assert_eq!(pixels[4 * W + 20], 0);
    }

    #[test]
    fn clip_crossing_the_left_edge() {
        // The mirror: x = -20..300, clipped at 0, so each row is 0..=299.
        let projection = page_projection();
        let f = face(&[0, 1, 2, 3], 0);
        let pverts = [
            flagged(&projection, -20, 0),
            flagged(&projection, 300, 0),
            flagged(&projection, 300, 4),
            flagged(&projection, -20, 4),
        ];
        let pixels = run_flat(&f, &pverts, 4, &projection);
        for y in 0..4 {
            assert_eq!(pixels[y * W], 4, "row {y} first");
            assert_eq!(pixels[y * W + 299], 4, "row {y} last");
            assert_eq!(pixels[y * W + 300], 0, "row {y} after the span");
        }
    }

    #[test]
    fn clip_crossing_the_top_edge() {
        // y = -10..2, clipped at 0: the clipped list is
        // (4,0), (4,2), (0,2), (0,0), and the two filled rows are 0 and 1,
        // each x = 0..=3.
        let projection = page_projection();
        let f = face(&[0, 1, 2, 3], 0);
        let pverts = [
            flagged(&projection, 0, -10),
            flagged(&projection, 4, -10),
            flagged(&projection, 4, 2),
            flagged(&projection, 0, 2),
        ];
        let pixels = run_flat(&f, &pverts, 6, &projection);
        for y in 0..2 {
            assert_eq!(row(&pixels, y, 5), &[6, 6, 6, 6, 0], "row {y}");
        }
        assert_eq!(row(&pixels, 2, 5), &[0; 5]);
    }

    #[test]
    fn clip_crossing_the_bottom_edge() {
        // y = 1..10, clipped at 3: the fill's rows are 1 and 2 (the bottom
        // row itself is excluded), each x = 0..=3.
        let projection = Projection {
            clip_y: [0, 3],
            ..page_projection()
        };
        let f = face(&[0, 1, 2, 3], 0);
        let pverts = [
            flagged(&projection, 0, 1),
            flagged(&projection, 4, 1),
            flagged(&projection, 4, 10),
            flagged(&projection, 0, 10),
        ];
        let pixels = run_flat(&f, &pverts, 8, &projection);
        assert_eq!(row(&pixels, 0, 5), &[0; 5]);
        assert_eq!(row(&pixels, 1, 5), &[8, 8, 8, 8, 0]);
        assert_eq!(row(&pixels, 2, 5), &[8, 8, 8, 8, 0]);
        assert_eq!(row(&pixels, 3, 5), &[0; 5]);
    }

    #[test]
    fn slanted_crossing_orders_the_endpoints_like_the_asm() {
        // Up clip at 0, the edge from (0,10) (last, inside) to (25,-15)
        // (current, hidden). `cmp cx,bx ; jl l3` (`ADRAWCLP.ASM:42-51`) takes
        // the *current* endpoint as the small one here, so
        // t = (0 - (-15))*16384/25 = 9830 and
        // x = 25 + (-25*9830 >> 14) = 25 - 15 = 10. With the endpoints
        // reversed (the bug) the crossing is at 9. The closing edge crosses
        // at x = -25 + (25*9830 >> 14) = -11.
        let src = [
            PolyVert {
                x: 0,
                y: 10,
                gr: 0,
                vx: 0,
            },
            PolyVert {
                x: 25,
                y: -15,
                gr: 0,
                vx: 0,
            },
            PolyVert {
                x: -25,
                y: -15,
                gr: 0,
                vx: 0,
            },
        ];
        let out = clip_2d(&src, 0, Axis::Y, true, false);
        assert_eq!(
            out,
            vec![
                PolyVert {
                    x: 0,
                    y: 10,
                    gr: 0,
                    vx: 0
                },
                PolyVert {
                    x: 10,
                    y: 0,
                    gr: 0,
                    vx: 0
                },
                PolyVert {
                    x: -11,
                    y: 0,
                    gr: 0,
                    vx: 0
                },
                PolyVert {
                    x: 0,
                    y: 10,
                    gr: 0,
                    vx: 0
                },
            ]
        );
    }

    #[test]
    fn clipped_gouraud_crossing_sign_extends_the_colour_difference() {
        // Left clip at x = 10 between last (x 0, gr 0x4000) and current
        // (x 20, gr 0x1000). The current's clipped coordinate is larger, so
        // small = last and t = (10 - 0)*16384/20 = 8192. `newclip_grd`
        // (`ADRAWCLP.ASM:24-34`) computes a 16-bit signed difference,
        // -0x3000, so delta = -0x3000*8192 >> 14 = -0x1800 and the crossing
        // colour is 0x4000 - 0x1800 = 0x2800. A zero-extended difference gives
        // 0xA800. The other coordinate is 0 + (20*8192 >> 14) = 10.
        let last = PolyVert {
            x: 0,
            y: 0,
            gr: 0x4000,
            vx: 0,
        };
        let current = PolyVert {
            x: 20,
            y: 20,
            gr: 0x1000,
            vx: 0,
        };
        let crossing = clip_2d_crossing(&last, &current, 10, Axis::X, true);
        assert_eq!((crossing.x, crossing.y, crossing.gr), (10, 10, 0x2800));
    }

    #[test]
    fn a_vertex_on_the_limit_after_a_hidden_one_stays_hidden() {
        // Up clip at 0. The hidden run is (5,-15), (10,0), (15,-10); the
        // hidden loop's strict test (`y <= limit`) keeps the on-limit (10,0)
        // hidden, so the crossing is taken from the last hidden vertex
        // (15,-10) to the first visible one (20,10) at x = 17. An inclusive
        // hidden loop would end the run at (10,0) and insert it as an extra
        // vertex.
        let src = [
            PolyVert {
                x: 0,
                y: 10,
                gr: 0,
                vx: 0,
            },
            PolyVert {
                x: 5,
                y: -15,
                gr: 0,
                vx: 0,
            },
            PolyVert {
                x: 10,
                y: 0,
                gr: 0,
                vx: 0,
            },
            PolyVert {
                x: 15,
                y: -10,
                gr: 0,
                vx: 0,
            },
            PolyVert {
                x: 20,
                y: 10,
                gr: 0,
                vx: 0,
            },
        ];
        let out = clip_2d(&src, 0, Axis::Y, true, false);
        assert_eq!(
            out,
            vec![
                PolyVert {
                    x: 0,
                    y: 10,
                    gr: 0,
                    vx: 0
                },
                PolyVert {
                    x: 2,
                    y: 0,
                    gr: 0,
                    vx: 0
                },
                PolyVert {
                    x: 17,
                    y: 0,
                    gr: 0,
                    vx: 0
                },
                PolyVert {
                    x: 20,
                    y: 10,
                    gr: 0,
                    vx: 0
                },
                PolyVert {
                    x: 0,
                    y: 10,
                    gr: 0,
                    vx: 0
                },
            ]
        );
    }

    #[test]
    fn near_clip_keeps_an_on_plane_vertex_hidden_after_a_hidden_one() {
        // clip_z[0] = 512. The vertices run z = 1024 (visible), 0 (hidden),
        // 512 (exactly on the plane), 0 (hidden), 1024 (visible), all on
        // y = 0 with x = 0, 10, 20, 30, 40; mul = 1024, so a crossing at
        // z = 512 projects x = 2 * x3d. The strict hidden loop
        // (`clip_z >= z`, ADRAWCLP.ASM:266-268) keeps the on-plane vertex
        // hidden: the visible run resumes at the last vertex and the
        // crossing is (40 + (30-40)/2) * 2 = 70. An inclusive hidden loop
        // would instead insert the on-plane vertex as an extra (40,0).
        let projection = Projection {
            mul_x: 1024,
            mul_y: 1024,
            add_x: 0,
            add_y: 0,
            clip_x: [0, 319],
            clip_y: [0, 199],
            clip_z: [512, 1_000_000_000],
        };
        let rotated = [
            Rotated {
                x: 0,
                y: 0,
                z: 1024,
                normal: 0,
            },
            Rotated {
                x: 10,
                y: 0,
                z: 0,
                normal: 0,
            },
            Rotated {
                x: 20,
                y: 0,
                z: 512,
                normal: 0,
            },
            Rotated {
                x: 30,
                y: 0,
                z: 0,
                normal: 0,
            },
            Rotated {
                x: 40,
                y: 0,
                z: 1024,
                normal: 0,
            },
        ];
        // The list's projected x: v0 (0,0); v4 (40,0); the hidden vertices'
        // x/y are undefined and only matter if one is copied, which the
        // fixed path never does.
        let src: Vec<PolyVert> = (0..5)
            .map(|index| PolyVert {
                x: if index == 0 { 0 } else { 40 },
                y: 0,
                gr: [0x1000, 0x3000, 0x4000, 0x5000, 0x2000][index],
                vx: index,
            })
            .collect();
        let (out, _) = clip_near(&src, &rotated, &projection);
        assert_eq!(
            out,
            vec![
                PolyVert {
                    x: 0,
                    y: 0,
                    gr: 0x1000,
                    vx: 0
                },
                // The two crossings take the previous output's word: v0's
                // 0x1000 for both.
                PolyVert {
                    x: 10,
                    y: 0,
                    gr: 0x1000,
                    vx: 0
                },
                PolyVert {
                    x: 70,
                    y: 0,
                    gr: 0x1000,
                    vx: 0
                },
                PolyVert {
                    x: 40,
                    y: 0,
                    gr: 0x2000,
                    vx: 4
                },
                PolyVert {
                    x: 0,
                    y: 0,
                    gr: 0x1000,
                    vx: 0
                },
            ]
        );
    }

    // ---------------------------------------------------------------------
    // The widened working-list cap
    // ---------------------------------------------------------------------

    #[test]
    fn twelve_gon_through_all_five_planes_is_simple_and_does_not_panic() {
        // A 12-gon of radius 200 about (160,100) crosses all four window
        // edges; vertex 0 is additionally behind the near plane (z = 0,
        // visible neighbours at z = 512 = clip_z[0]), so the near clip runs
        // and the four 2-D passes run after it. Shipped data reaches 12
        // sides (U2E.039); the original's 16-vertex buffers cannot hold the
        // result, the widened cap must. A convex polygon's filled rows are
        // one contiguous run each, which is the simplicity check.
        let projection = Projection {
            mul_x: 512,
            mul_y: 512,
            add_x: 0,
            add_y: 0,
            clip_x: [0, 319],
            clip_y: [25, 174],
            clip_z: [512, 1_000_000_000],
        };
        let mut pverts = Vec::new();
        let mut rotated = Vec::new();
        for i in 0..12 {
            let angle = f64::from(i) * std::f64::consts::PI / 6.0;
            let x = (160.0 + 200.0 * angle.cos()).round() as i16;
            let y = (100.0 + 200.0 * angle.sin()).round() as i16;
            let mut v = flagged(&projection, x, y);
            let z = if i == 0 {
                v.vf |= VF_NEAR;
                0
            } else {
                512
            };
            pverts.push(v);
            rotated.push(Rotated {
                x: i32::from(x),
                y: i32::from(y),
                z,
                normal: 0,
            });
        }
        let f = face(&(0..12).collect::<Vec<u16>>(), 0);
        let pixels = run_flat(&f, &pverts, 11, &projection);

        let mut set = 0;
        for y in 25..=174 {
            let mut runs = 0;
            let mut in_run = false;
            for x in 0..W {
                let on = pixels[y * W + x] == 11;
                if on {
                    set += 1;
                    assert!(
                        x <= projection.clip_x[1] as usize,
                        "row {y} wrote past the right clip"
                    );
                    if !in_run {
                        runs += 1;
                        in_run = true;
                    }
                } else {
                    in_run = false;
                }
                assert!(
                    pixels[y * W + x] == 0 || pixels[y * W + x] == 11,
                    "row {y} holds a colour the fill did not write"
                );
            }
            assert!(runs <= 1, "row {y} is not a simple span ({runs} runs)");
        }
        assert!(set > 0, "the clipped 12-gon filled nothing");
    }

    // ---------------------------------------------------------------------
    // Culling
    // ---------------------------------------------------------------------

    #[test]
    fn cull_rejects_a_colour_minus_one_face() {
        let normals = [RotatedNormal {
            x: 0,
            y: 0,
            z: 16384,
        }];
        let vertices = [Rotated {
            x: 0,
            y: 0,
            z: -1,
            normal: 0,
        }];
        let mut f = face(&[0], 0);
        f.color = -1;
        assert!(cull(&f, 0xF001, &normals, &vertices, 0));
    }

    #[test]
    fn cull_uses_the_sign_of_normal_dot_first_vertex() {
        // A normal (0,0,UNIT) and a first rotated vertex (0,0,-1): the dot
        // is negative, so the face is visible (`checkculling` returns
        // carry=1=visible when the 64-bit sum is negative).
        let normals = [RotatedNormal {
            x: 0,
            y: 0,
            z: 16384,
        }];
        let front = [Rotated {
            x: 0,
            y: 0,
            z: -1,
            normal: 0,
        }];
        let back = [Rotated {
            x: 0,
            y: 0,
            z: 1,
            normal: 0,
        }];
        let f = face(&[0], 0);
        assert!(!cull(&f, 0xF001, &normals, &front, 0));
        assert!(cull(&f, 0xF001, &normals, &back, 0));
    }

    #[test]
    fn cull_keeps_a_two_sided_face_with_a_positive_dot() {
        // `test cs:poly1[POLYFLAGS],F_2SIDE; jnz @@2side` skips the dot test.
        let normals = [RotatedNormal {
            x: 0,
            y: 0,
            z: 16384,
        }];
        let back = [Rotated {
            x: 0,
            y: 0,
            z: 1,
            normal: 0,
        }];
        let f = face(&[0], F_2SIDE);
        assert!(!cull(&f, 0xF001, &normals, &back, 0));
    }

    #[test]
    fn cull_skips_a_face_all_of_whose_vertices_share_an_out_flag() {
        // The caller's AND of the projected flags is nonzero: every vertex
        // is right of the window, so no pixel survives the right clip.
        let normals = [RotatedNormal {
            x: 0,
            y: 0,
            z: 16384,
        }];
        let front = [Rotated {
            x: 0,
            y: 0,
            z: -1,
            normal: 0,
        }];
        let f = face(&[0], 0);
        assert!(cull(&f, 0xF001, &normals, &front, VF_RIGHT));
        assert!(!cull(&f, 0xF001, &normals, &front, 0));
    }

    #[test]
    fn poly_flags_forces_the_face_byte_and_gates_gouraud_on_the_object() {
        // `or ax,0f00h ; and ax,cx`: F_2SIDE, the shade bands and F_FLIP are
        // always taken from the face; F_GOURAUD only when the object enables
        // it (F_DEFAULT 0xF001 has bit 12 set).
        assert_eq!(poly_flags(0x1C00, 0xF001), 0x1C00);
        assert_eq!(poly_flags(0x1C00, 0x0001), 0x0C00);
        assert_eq!(poly_flags(0x0200, 0x0001), 0x0200);
        assert_eq!(poly_flags(0x0100, 0x0000), 0x0100);
    }

    // ---------------------------------------------------------------------
    // Lighting
    // ---------------------------------------------------------------------

    #[test]
    fn light_of_the_eight_principal_normals() {
        // normallight = clamp(((dot >> 16) as i16 >> 5) + 128, 0, 255), then
        // calclight shifts by 6 - (flags >> 10) and clamps 1..=30. The light
        // vector is (12118, 10603, 3030) and UNIT is 16384. With the 32-shade
        // band (shift 3): the six axes are +x 27, -x 4, +y 26, -y 5, +z 18,
        // -z 13, and the eight cube corners follow.
        let principal = [
            (
                RotatedNormal {
                    x: 16384,
                    y: 0,
                    z: 0,
                },
                27,
            ),
            (
                RotatedNormal {
                    x: -16384,
                    y: 0,
                    z: 0,
                },
                4,
            ),
            (
                RotatedNormal {
                    x: 0,
                    y: 16384,
                    z: 0,
                },
                26,
            ),
            (
                RotatedNormal {
                    x: 0,
                    y: -16384,
                    z: 0,
                },
                5,
            ),
            (
                RotatedNormal {
                    x: 0,
                    y: 0,
                    z: 16384,
                },
                18,
            ),
            (
                RotatedNormal {
                    x: 0,
                    y: 0,
                    z: -16384,
                },
                13,
            ),
        ];
        for (normal, expected) in principal {
            assert_eq!(light(&normal, F_SHADE32), expected, "{normal:?}");
        }

        // The eight cube corners: +++ 30, ++- 30, +-+ 20, +-- 14, -++ 17,
        // -+- 11, --+ 1, --- 1.
        let corners = [
            (1, 1, 1, 30),
            (1, 1, -1, 30),
            (1, -1, 1, 20),
            (1, -1, -1, 14),
            (-1, 1, 1, 17),
            (-1, 1, -1, 11),
            (-1, -1, 1, 1),
            (-1, -1, -1, 1),
        ];
        for (x, y, z, expected) in corners {
            let normal = RotatedNormal {
                x: x * 16384,
                y: y * 16384,
                z: z * 16384,
            };
            assert_eq!(light(&normal, F_SHADE32), expected, "{normal:?}");
        }
    }

    #[test]
    fn light_sweeps_the_three_shade_bands() {
        // `shr ax,cl` with cl = 6 - (flags >> 10): the 8-colour band shifts
        // by 5, the 16-colour by 4 and the 32-colour by 3. +x's brightness is
        // 222: 222>>5 = 6, 222>>4 = 13, 222>>3 = 27.
        let n = RotatedNormal {
            x: 16384,
            y: 0,
            z: 0,
        };
        assert_eq!(light(&n, F_SHADE8), 6);
        assert_eq!(light(&n, F_SHADE16), 13);
        assert_eq!(light(&n, F_SHADE32), 27);
        // No shade bit: `xor ax,ax` then the 1..=30 clamp gives 1.
        assert_eq!(light(&n, 0), 1);
        assert_eq!(light(&n, F_2SIDE), 1);
    }

    #[test]
    fn light_clamps_at_one_and_thirty() {
        // A normal opposite every light component: normallight is 0, so the
        // shifted value is 0 and the low clamp returns 1.
        let dark = RotatedNormal {
            x: -16384,
            y: -16384,
            z: -16384,
        };
        assert_eq!(light(&dark, F_SHADE32), 1);
        // The light vector itself: dot = 268,450,433, high word 4096,
        // normallight = 256 -> 255; 255>>3 = 31 -> the high clamp returns 30.
        let lit = RotatedNormal {
            x: 12118,
            y: 10603,
            z: 3030,
        };
        assert_eq!(light(&lit, F_SHADE32), 30);
    }

    #[test]
    fn page_writes_outside_the_page_are_dropped() {
        // The transliteration must not panic on a page smaller than the
        // window it is handed (the tests above use the full page; this pins
        // the guard itself).
        let mut pixels = [0u8; 4 * 2];
        let mut page = Page::new(&mut pixels, 4);
        page.set(-1, 0, 1);
        page.set(4, 0, 1);
        page.set(0, 2, 1);
        page.set(1, 1, 9);
        assert_eq!(pixels, [0, 0, 0, 0, 0, 9, 0, 0]);
    }

    // The maths module is otherwise only used through these tests; keep the
    // import exercised so the module link is explicit.
    #[test]
    fn page_projection_matches_the_maths_window() {
        // The test helper supplies its own projected vertices, so the
        // multipliers and centre are zero, but the three clips must be the
        // maths window's.
        let window = math::window(0, 319, 0, 199, 512, 1_000_000_000);
        let p = page_projection();
        assert_eq!(p.clip_x, window.clip_x);
        assert_eq!(p.clip_y, window.clip_y);
        assert_eq!(p.clip_z, window.clip_z);
    }
}
