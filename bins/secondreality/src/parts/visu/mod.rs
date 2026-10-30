//! The VISU vector engine (`VISU/ACALC.ASM`, `VISU/ADRAW*.ASM`).
//!
//! `math` is the fixed-point core: the matrix multiply and apply, the vertex
//! and normal rotations, the projection window and per-vertex clipping flags,
//! and the camera-angle table. `draw` is the polygon pipeline. This module is
//! the engine's last layer, transliterated from `VISU/VISU.C:101-125` and the
//! two parts' loop bodies: the packed-scene loader, the scene's instances and
//! animation cursor (`U2A.C:148-227`, `U2E.C:208-271`), the four-page ring
//! driven by the copper interrupt (`U2A.C:105-133`), and the per-frame draw
//! (`U2A.C:284-324`, `U2E.C:386-433`).

pub mod draw;
pub mod math;

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use sr_engine::copper::COPPER_SLOTS;
use sr_engine::part::{PartCtx, PartError};
use sr_engine::View;

use sr_assets::visu::{
    read_animation, read_master, read_object, read_scene_list, Animation, Master, ObjectData,
};

use draw::{clip_and_fill, cull, face_sides, light, poly_flags, Page, F_GOURAUD};
use math::{
    apply_matrix, camera_angle, project_all, rotate_normals, rotate_vertices, single_z, window,
    Matrix, PVert, Projection, Rotated, RotatedNormal,
};

/// The packed scenes' directory (`U2A.C:148`, `U2E.C:208`).
const SCENE_DIR: &str = "assets/VISU/C/SCENE";

/// Both parts' displayed picture, the shipped 320x200 tweak (`U2A.C:242`,
/// `U2E.C:275`, `AVIDM1.ASM:17-69`).
pub const VIEW_WIDTH: u16 = 320;
pub const VIEW_HEIGHT: u16 = 200;
/// One ring page's pixels (`U2A.C:90-95`).
pub const PAGE_PIXELS: usize = 64_000;
/// The four-page ring (`cl[4]`, `U2A.C:90-94`, `U2E.C:97-103`).
pub const PAGES: usize = 4;
/// The surface pixels the four pages reach.
const SURFACE_PIXELS: usize = PAGES * PAGE_PIXELS;

/// Which of the two parts drives the engine: the ring's cadence and the
/// draw order's hacks differ between them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Variant {
    /// `VISU/C/U2A.C`: a one-retrace step and the city hacks.
    U2a,
    /// `VISU/C/U2E.C`: a two-retrace step and the name hacks.
    U2e,
}

/// `part`'s error unless the surface holds the four-page ring.
pub fn check_surface(ctx: &PartCtx, part: &'static str) -> Result<(), PartError> {
    let held = ctx.surface.pixels().len();
    if held < SURFACE_PIXELS {
        return Err(PartError::new(
            part,
            format!("the four-page ring reaches pixel {SURFACE_PIXELS} and the surface holds {held}"),
        ));
    }
    Ok(())
}

/// Both parts' view: `vid_init` is state only for the port (the shipped
/// 320x200 mode, rows packed at 320), with page 0 displayed until the ring's
/// copper moves the start.
pub fn view() -> View {
    crate::parts::mode_view(VIEW_WIDTH, VIEW_HEIGHT, Some(VIEW_WIDTH), None)
}

/// `while(repeat-- && !xit) { parse one animation frame }` (`U2A.C:336-430`,
/// `U2E.C:445-539`): up to `frames` stream frames, each frame's FOV becoming
/// the next draw's angle. True when the stream ended (`0xFF 0xFF` resets the
/// scene and sets `xit`) and the part tears down.
pub fn parse_frames(scene: &mut Scene, driver: &RefCell<Driver>, frames: usize) -> bool {
    for _ in 0..frames {
        match scene.step_frame() {
            Some(angle) => driver.borrow_mut().fov = angle,
            None => return true,
        }
    }
    scene.xit
}

/// The teardown's `dis_setcopper(2,NULL)` (`U2A.C:432`, `U2E.C:541`): the port
/// installs no-ops in all three slots, so nothing a slot held can run again.
pub fn clear_copper(ctx: &mut PartCtx) {
    for slot in 0..COPPER_SLOTS {
        ctx.set_copper(slot, Box::new(|_ctx: &mut PartCtx| {}));
    }
}

/// `F_DEFAULT` (`AD.INC:66`, `CD.H:123`): the flags `vis_loadobject` gives
/// every object (`VISU.C:19`). Neither part changes them, so every object is
/// `F_VISIBLE` (`VISU.C:107`'s return never runs) and carries `F_GOURAUD`,
/// which makes `vis_drawobject`'s `nnum`/`nnum1` choice always `nnum`
/// (`VISU.C:109`).
pub const F_DEFAULT: u16 = 0xF001;

/// `_projaspect` as every `AVIDM1.ASM` initialiser sets it
/// (`AVIDM1.ASM:40`, `:67`, `:94`).
const ASPECT: i32 = 225;

/// One entry of the original's `co[]` (`U2A.C:36-43`): an object, its
/// relative and camera-transformed matrices, whether the stream has it on,
/// and the object-file number it was loaded from (`0` for the camera).
#[derive(Debug)]
pub struct Instance {
    /// The shared geometry. Duplicate entries in the master's object table
    /// clone this `Rc`, exactly as `U2A.C:190-199` copies the object struct.
    pub object: Rc<ObjectData>,
    /// `co[a].o->r`: `r0` after `calc_applyrmatrix(o->r,&cam)`, rebuilt every
    /// draw (`U2A.C:295-296`).
    pub r: Matrix,
    /// `co[a].o->r0`: the stream's accumulated matrix, zeroed by `reset`.
    pub r0: Matrix,
    /// `co[a].on`: the last toggle the stream applied.
    pub on: bool,
    /// `co[a].index`: the `.00N` number this entry was loaded from
    /// (`U2A.C:183`). The camera is `co[0]` (`U2A.C:202`) and keeps `0`.
    pub index: u16,
}

/// A loaded scene: the master, `co[]`, the animation stream and its cursor.
#[derive(Debug)]
pub struct Scene {
    /// The `.00M` master: the city flag, the palette and the object table.
    pub master: Master,
    /// `co[]`, camera first.
    pub instances: Vec<Instance>,
    /// The first stream named by the `.0AA` scene list.
    pub animation: Animation,
    /// `cam`: the camera matrix the stream's object-0 commands move
    /// (`U2A.C:48`, `:202-204`).
    pub cam: Matrix,
    /// `sp`'s frame index into `animation.frames`.
    pub stream: usize,
    /// The original's `xit`: the stream's `0xFF 0xFF` has run.
    pub xit: bool,
}

impl Scene {
    /// `resetscene` (`U2A.C:74-88`): zero every `r` and `r0`, zero `cam`, and
    /// rewind the stream. `on` and `xit` are untouched, as in the C.
    pub fn reset(&mut self) {
        for instance in &mut self.instances {
            instance.r = Matrix::default();
            instance.r0 = Matrix::default();
        }
        self.cam = Matrix::default();
        self.stream = 0;
    }

    /// Parse one stream frame (`U2A.C:336-430`, `U2E.C:445-539`): apply its
    /// commands in file order — the toggle first, then the position deltas
    /// and the selected matrix deltas to `r0` (object 0's go to `cam`, which
    /// the original aliases through `camobject.r0`, `U2A.C:202-204`) — and
    /// return that frame's field of vision as `fov << 8`.
    ///
    /// A frame terminated by `0xFF 0xFF` calls [`reset`](Scene::reset), sets
    /// [`xit`](Scene::xit) and returns `None`; the original never draws it
    /// (`U2A.C:351-356`). The parts draw before stepping
    /// (`U2A.C:318-336`).
    pub fn step_frame(&mut self) -> Option<u16> {
        {
            let Scene {
                animation,
                instances,
                cam,
                stream,
                xit,
                ..
            } = self;
            let Some(frame) = animation.frames.get(*stream) else {
                // The reader always leaves the END frame last, and `xit`
                // stops the loop before this; refusing to read past the list
                // keeps a caller that ignores `xit` from panicking.
                *xit = true;
                return None;
            };

            for op in &frame.ops {
                if let Some(on) = op.toggle {
                    if let Some(instance) = instances.get_mut(op.object) {
                        instance.on = on;
                    }
                }
                let target = if op.object == 0 {
                    &mut *cam
                } else if let Some(instance) = instances.get_mut(op.object) {
                    &mut instance.r0
                } else {
                    continue;
                };
                target.x = target.x.wrapping_add(op.dx);
                target.y = target.y.wrapping_add(op.dy);
                target.z = target.z.wrapping_add(op.dz);
                for (entry, delta) in target.m.iter_mut().zip(op.dm.iter()) {
                    if let Some(delta) = delta {
                        *entry = entry.wrapping_add(*delta);
                    }
                }
            }

            if !frame.end {
                let angle = u16::from(frame.fov) << 8;
                *stream += 1;
                return Some(angle);
            }
        }

        // `resetscene(); xit=1` (`U2A.C:351-355`): the reset also rewinds `sp`
        // (`:77`). The field borrows above end here, so the END frame can use
        // the same reset as the rest of the scene.
        self.reset();
        self.xit = true;
        None
    }
}

/// An empty `ObjectData` standing in for `camobject` (`U2A.C:47`, `:202`).
fn camera_object() -> ObjectData {
    ObjectData {
        version: 0,
        name: Vec::new(),
        vertices: Vec::new(),
        normals: Vec::new(),
        polys: Vec::new(),
        lists: Vec::new(),
    }
}

/// The `a/10+'A'`, `a%10+'A'` scene-stream suffix (`U2A.C:213`): scene 1 is
/// `.0AB`. The C's `char` arithmetic wraps the same way.
fn stream_suffix(scene: u16) -> String {
    let tens = char::from(b'A'.wrapping_add((scene / 10) as u8));
    let ones = char::from(b'A'.wrapping_add((scene % 10) as u8));
    format!("{tens}{ones}")
}

/// Read a file below `dir`, naming it in the error.
fn read_file(dir: &Path, file: &str) -> Result<(PathBuf, Vec<u8>), String> {
    let path = dir.join(file);
    let bytes = std::fs::read(&path).map_err(|error| format!("{}: {error}", path.display()))?;
    Ok((path, bytes))
}

/// Load a packed scene from the scene directory (`U2A.C:148-227`,
/// `U2E.C:208-271`).
///
/// Reads `<name>.00M`; one object file for each distinct number in the
/// master's object table, with later duplicates sharing the `Rc` and taking
/// fresh matrices; `<name>.0AA`; and the first scene stream it names,
/// decoded with `conum` objects (the camera included).
pub fn load_scene(name: &str) -> Result<Scene, String> {
    let root = crate::parts::asset(SCENE_DIR);
    let dir = Path::new(&root);
    let (path, bytes) = read_file(dir, &format!("{name}.00M"))?;
    let master = read_master(&bytes).map_err(|error| format!("{}: {error}", path.display()))?;

    // `co[0]` is the camera and never has geometry (`U2A.C:202-204`).
    let mut instances = Vec::with_capacity(master.objects.len() + 1);
    instances.push(Instance {
        object: Rc::new(camera_object()),
        r: Matrix::default(),
        r0: Matrix::default(),
        on: false,
        index: 0,
    });

    for &number in &master.objects {
        // `e <= f` searches the earlier entries for the same object number
        // and copies the object struct (`U2A.C:187-200`); the camera is not
        // one of the file's objects, so the search starts past it.
        let shared = instances
            .iter()
            .skip(1)
            .find(|instance| instance.index == number)
            .map(|instance| Rc::clone(&instance.object));
        let object = match shared {
            Some(object) => object,
            None => {
                let (path, bytes) = read_file(dir, &format!("{name}.{number:03}"))?;
                Rc::new(
                    read_object(&bytes).map_err(|error| format!("{}: {error}", path.display()))?,
                )
            }
        };
        instances.push(Instance {
            object,
            r: Matrix::default(),
            r0: Matrix::default(),
            on: false,
            index: number,
        });
    }

    let (path, bytes) = read_file(dir, &format!("{name}.0AA"))?;
    let scenes = read_scene_list(&bytes).map_err(|error| format!("{}: {error}", path.display()))?;
    let scene_number = *scenes
        .scenes
        .first()
        .ok_or_else(|| format!("{}: the scene list is empty", path.display()))?;
    let (path, bytes) = read_file(dir, &format!("{name}.0{}", stream_suffix(scene_number)))?;
    let animation = read_animation(&bytes, instances.len())
        .map_err(|error| format!("{}: {error}", path.display()))?;

    Ok(Scene {
        master,
        instances,
        animation,
        cam: Matrix::default(),
        stream: 0,
        xit: false,
    })
}

/// The four-page ring and the copper interrupt that cycles it
/// (`U2A.C:90-133`, `U2E.C:97-143`).
///
/// `ready[i]` is the original's `cl[i].ready`: `0` free, `1` queued for
/// display, `2` displayed and about to be freed. `frames[i]` is
/// `cl[i].frames`, the number of retraces the page stays up.
#[derive(Debug)]
pub struct Ring {
    ready: [u8; 4],
    frames: [usize; 4],
    clr: usize,
    clw: usize,
    copperdelay: i32,
    syncframe: i32,
    currframe: i32,
    avgrepeat: i32,
    // `coppercnt` (`U2A.C:97`, `:117`) is written and never read, so it is not kept.
    deadlock: i32,
    displayed_page: usize,
    variant: Variant,
}

impl Ring {
    /// The ring in the state both parts set up before their loops
    /// (`U2A.C:251-258`, `U2E.C:368-374`): page 3 pre-queued, the display on
    /// page 0, and sixteen retraces of start-up delay.
    pub fn new(variant: Variant) -> Ring {
        Ring {
            ready: [0, 0, 0, 1],
            frames: [0; 4],
            clr: 0,
            clw: 0,
            copperdelay: 16,
            syncframe: 0,
            currframe: 0,
            avgrepeat: 1,
            deadlock: 0,
            displayed_page: 0,
            variant,
        }
    }

    /// One copper interrupt: `copper2` verbatim (`U2A.C:105-133`).
    pub fn isr(&mut self) {
        self.syncframe += 1;

        for ready in &mut self.ready {
            if *ready == 2 {
                *ready = 0;
            }
        }

        self.deadlock += 1;

        if self.copperdelay > 0 {
            self.copperdelay -= 1;
        }
        if self.copperdelay > 0 {
            return;
        }
        self.copperdelay = 0;
        if self.ready[self.clr] != 0 {
            // `cl[(clr-1)&3].ready=2; vid_setswitch(-1,clr)`
            // (`U2A.C:127-130`): the previous page starts its last retrace
            // while `clr` becomes the displayed one.
            self.ready[(self.clr + 3) & 3] = 2;
            self.displayed_page = self.clr;
            self.copperdelay = self.frames[self.clr] as i32;
            self.clr = (self.clr + 1) & 3;
        } else {
            self.avgrepeat += 1;
        }
    }

    /// Take the writer page, or wait one tick (`U2A.C:278-283`).
    ///
    /// The original spins in `while(cl[clw].ready)` until the copper frees
    /// the page, breaking once `deadlock > 16`. The port's caller runs at
    /// the fixed 70 Hz tick, so a busy page is one tick of waiting: `None`.
    /// A successful wait clears the counter, which only the copper
    /// increments while the page stays busy.
    pub fn next_page(&mut self) -> Option<usize> {
        if self.ready[self.clw] == 0 || self.deadlock > 16 {
            self.deadlock = 0;
            Some(self.clw)
        } else {
            None
        }
    }

    /// Queue `page` for `frames` retraces: `cl[clw].frames=repeat;
    /// cl[clw].ready=1; clw++` (`U2A.C:330-332`, `U2E.C:438-441`).
    pub fn queue(&mut self, page: usize, frames: usize) {
        self.frames[page] = frames;
        self.ready[page] = 1;
        self.clw = (self.clw + 1) & 3;
    }

    /// The drawn frame's cadence after the draw: `(page frames, stream
    /// frames)`.
    ///
    /// U2A's one-retrace step (`U2A.C:326-336`) smooths the lateness into
    /// `avgrepeat`, queues at least one retrace, and advances the stream by
    /// the same count.
    ///
    /// U2E's two-retrace step (`U2E.C:434-444`) computes `a =
    /// syncframe-currframe`, queues `max(a+1, 1)` retraces and advances the
    /// stream by `(a+2)/2` frames of two retraces.
    pub fn schedule(&mut self) -> (usize, usize) {
        if self.variant == Variant::U2a {
            self.avgrepeat = (self.avgrepeat + (self.syncframe - self.currframe) + 1) / 2;
            let repeat = self.avgrepeat.max(1);
            self.currframe += repeat;
            (repeat as usize, repeat as usize)
        } else {
            let repeat = (self.syncframe - self.currframe + 1).max(0);
            let page = if repeat == 0 { 1 } else { repeat };
            let stream = (repeat + 1) / 2;
            self.currframe += stream * 2;
            (page as usize, stream as usize)
        }
    }

    /// The page the CRTC is displaying: `vid_setswitch`'s target.
    pub fn displayed(&self) -> usize {
        self.displayed_page
    }

    /// The schedule counter, for `u2e`'s `search_reference_states`.
    #[cfg(test)]
    pub(crate) fn set_currframe(&mut self, value: i32) {
        self.currframe = value;
    }
}

/// What `vid_clearbg`/`vid_clear255` puts on a fresh page.
pub enum Clear<'a> {
    /// U2A's `vid_clearbg(bg)` (`U2A.C:285`): a linear 320x200 image. The
    /// decoder delivers `U2ABG.UH` linear, so `U2A.C:153-166`'s de-interleave
    /// is not needed.
    Background(&'a [u8]),
    /// U2E's `vid_clear255()` (`U2E.C:395`): one byte over the whole page.
    Byte(u8),
}

/// The per-frame driver: the ring, the projection and the fov the last
/// stepped frame set.
#[derive(Debug)]
pub struct Driver {
    /// The four-page ring and its copper state.
    pub ring: Ring,
    /// `_proj*`: the window and the two multipliers `camera_angle` updates.
    pub proj: Projection,
    /// The current field of vision, as the stream's `fov << 8`.
    pub fov: u16,
    /// U2E's `firstframe`: the first draw is skipped while this is set
    /// (`U2E.C:386-387`, `:433`). U2A constructs its driver with `false`.
    pub first_draw: bool,
}

impl Driver {
    /// A driver with both parts' window `vid_window(0,319,25,174,512,
    /// 9999999)` (`U2A.C:247`, `U2E.C:351`), fov 0 and the chosen ring. Its x
    /// range is the one `math::HALF_WIDTH` assumes.
    pub fn new(ring: Ring, first_draw: bool) -> Driver {
        Driver {
            ring,
            proj: window(0, 319, 25, 174, 512, 9_999_999),
            fov: 0,
            first_draw,
        }
    }

    /// The draw order, for `u2e`'s `search_reference_states`.
    #[cfg(test)]
    pub(crate) fn order_for_test(&self, scene: &mut Scene) -> Vec<usize> {
        self.order_instances(scene)
    }

    /// U2E's `if(!firstframe) ... else { syncframe=0; firstframe=0;
    /// coppercnt=1; }` (`U2E.C:386-387`, `:433`): no camera angle, no clear,
    /// no draw, only the clock reset.
    pub fn skip_first_draw(&mut self) {
        self.ring.syncframe = 0;
        self.first_draw = false;
    }

    /// The loop body of `U2A.C:284-324` / `U2E.C:386-433`: clear the page,
    /// order the enabled objects and draw them.
    pub fn draw_frame(&mut self, scene: &mut Scene, page: &mut Page<'_>, clear: Clear<'_>) {
        // `vid_cameraangle(fov)` (`U2A.C:287`, `U2E.C:397`) with the
        // `AVIDM1.ASM` aspect both parts install.
        let (mul_x, mul_y) = camera_angle(self.fov, ASPECT);
        self.proj.mul_x = mul_x;
        self.proj.mul_y = mul_y;

        match clear {
            Clear::Background(background) => {
                // `vid_clearbg(bg)` copies 64,000 bytes. A shorter slice
                // cannot be read past its end; the shipped image is exact.
                let copied = page.pixels.len().min(background.len());
                page.pixels[..copied].copy_from_slice(&background[..copied]);
            }
            Clear::Byte(byte) => page.pixels.fill(byte),
        }

        let order = self.order_instances(scene);
        for index in order {
            self.draw_instance(scene, index, page);
        }
    }

    /// Build and sort `order[]` (`U2A.C:289-317`, `U2E.C:399-424`), applying
    /// the camera and computing each enabled object's sort distance.
    ///
    /// The city hacks are U2A's (`U2A.C:301-310`); U2E never uses its city
    /// flag. The name hacks are U2E's (`U2E.C:408-416`); U2A has neither.
    /// Both shipped masters say city 0.
    fn order_instances(&self, scene: &mut Scene) -> Vec<usize> {
        let u2a = self.ring.variant == Variant::U2a;
        let cam = scene.cam;
        let mut order = Vec::new();
        let mut distances = vec![0i32; scene.instances.len()];

        // `for(a=1;a<conum;a++) if(co[a].on)`: the camera is never ordered
        // (`U2A.C:290-291`).
        for (index, distance) in distances.iter_mut().enumerate().skip(1) {
            if !scene.instances[index].on {
                continue;
            }
            order.push(index);
            let instance = &mut scene.instances[index];
            instance.r = instance.r0;
            apply_matrix(&mut instance.r, &cam);

            let object = Rc::clone(&instance.object);
            if !u2a && object.name.get(1) == Some(&b'_') {
                // `if(co[a].o->name[1]=='_') co[a].dist=1000000000L`
                // (`U2E.C:408`); the stored name keeps its opening quote.
                *distance = 1_000_000_000;
                continue;
            }

            let center = object.lists.first().map_or(0, |list| list.center);
            *distance = if usize::from(center) < object.vertices.len() {
                single_z(&object.vertices, center, &instance.r)
            } else {
                0
            };
        }

        if u2a && scene.master.city == 1 {
            // `U2A.C:301-306`, with the index guard the C lacks.
            for index in [2usize, 7, 13] {
                if let Some(distance) = distances.get_mut(index) {
                    *distance = 1_000_000_000;
                }
            }
        }
        if u2a && scene.master.city == 2 {
            // `U2A.C:307-310`.
            if let Some(distance) = distances.get_mut(14) {
                *distance = 1_000_000_000;
            }
        }

        if !u2a && self.ring.currframe > 900 * 2 && self.ring.currframe < 1100 * 2 {
            // `currframe>900*2 && currframe<1100*2` and the quoted name's
            // `s01` (`U2E.C:410-416`).
            for &index in &order {
                let name = &scene.instances[index].object.name;
                if name.get(1..4) == Some(b"s01") {
                    distances[index] = 1;
                }
            }
        }

        // `for(a=0..) { dis=...; for(b=a-1;b>=0 && dis>...;b--) ... }`
        // (`U2A.C:311-317`): a stable descending insertion sort.
        for a in 0..order.len() {
            let index = order[a];
            let distance = distances[index];
            let mut b = a;
            while b > 0 && distance > distances[order[b - 1]] {
                order[b] = order[b - 1];
                b -= 1;
            }
            order[b] = index;
        }
        order
    }

    /// `vis_drawobject` (`VISU.C:101-125`) and `_draw_polylist`
    /// (`ADRAW.ASM:437-604`) for one instance.
    fn draw_instance(&self, scene: &Scene, index: usize, page: &mut Page<'_>) {
        let instance = &scene.instances[index];
        let object = Rc::clone(&instance.object);
        let flags = F_DEFAULT;

        // `calc_rotate(vnum,v,v0,r)` and, because F_GOURAUD is always set,
        // `calc_nrotate(nnum,n,n0,r)` (`VISU.C:108-109`).
        let mut rotated = vec![Rotated::default(); object.vertices.len()];
        rotate_vertices(&mut rotated, &object.vertices, &instance.r);
        let mut rotated_normals = vec![RotatedNormal::default(); object.normals.len()];
        rotate_normals(&mut rotated_normals, &object.normals, &instance.r);

        // `o->vf=calc_project(...); if(o->vf) return;` (`VISU.C:111-112`).
        let mut pverts = vec![PVert::default(); rotated.len()];
        if project_all(&rotated, &mut pverts, &self.proj) != 0 {
            return;
        }

        // `a=0; al=0x7fffffff; for(b=1;b<plnum;b++) ...` (`VISU.C:113-123`).
        let list_index = pick_list(&object, &rotated);
        let Some(list) = object.lists.get(list_index) else {
            return;
        };

        for &offset in &list.offsets {
            let Some(face) = object.polys.iter().find(|face| face.offset == offset) else {
                continue;
            };
            let combined = poly_flags(face.flags, flags);

            // `and dh,al` over the face's projected vertices
            // (`ADRAW.ASM:517-532`), capped like the pipeline's copy loop.
            let sides = face_sides(face);
            let mut screen_flags = 0xFFFFu16;
            for &vertex in &face.vertices[..sides] {
                if let Some(projected) = pverts.get(vertex as usize) {
                    screen_flags &= projected.vf;
                }
            }
            if cull(face, flags, &rotated_normals, &rotated, screen_flags) {
                continue;
            }

            let base = face.color as u8;
            let face_normal = rotated_normals
                .get(face.normal as usize)
                .copied()
                .unwrap_or_default();
            let gouraud = combined & F_GOURAUD != 0;
            // `calclight` plus the face's base colour (`ADRAW.ASM:495-503`),
            // except that a Gouraud face keeps the unlit base (`:502-503`).
            let flat = if gouraud {
                base
            } else {
                base.wrapping_add(light(&face_normal, combined))
            };

            // `mov bx,fs:[bx+vlist_normal]` (`ADRAW.ASM:545-556`): each vertex
            // uses its *own* rotated normal, not the face's.
            let colors: Option<Vec<u16>> = if gouraud {
                Some(
                    face.vertices[..sides]
                        .iter()
                        .map(|&vertex| {
                            let normal = rotated
                                .get(vertex as usize)
                                .map_or(0, |rotated| rotated.normal);
                            let normal = rotated_normals
                                .get(normal as usize)
                                .copied()
                                .unwrap_or_default();
                            u16::from(base.wrapping_add(light(&normal, combined))) << 8
                        })
                        .collect(),
                )
            } else {
                None
            };

            clip_and_fill(
                page,
                face,
                &rotated,
                &pverts,
                &self.proj,
                colors.as_deref(),
                flat,
            );
        }
    }
}

/// The `pl[a]` choice (`VISU.C:113-123`): list 0 unless an `ORDE` list's
/// centre vertex has a smaller rotated z. List 0's own centre is only the
/// default and is never compared.
fn pick_list(object: &ObjectData, rotated: &[Rotated]) -> usize {
    let mut best = 0usize;
    let mut best_z = i32::MAX;
    for (index, list) in object.lists.iter().enumerate().skip(1) {
        let z = rotated
            .get(list.center as usize)
            .map_or(i32::MAX, |vertex| vertex.z);
        if z < best_z {
            best_z = z;
            best = index;
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use sr_assets::load_path;
    use sr_assets::visu::{FaceList, Frame, Normal, Op, Vertex};
    use sr_engine::Palette;
    use std::path::Path;

    const BACKGROUND: &str = "../../assets/VISU/C/U2ABG.UH";

    /// One vertex and one `ORD0` list, enough for `single_z` and the order.
    fn plane_object_named(name: &[u8], z: i32) -> Rc<ObjectData> {
        Rc::new(ObjectData {
            version: 0x100,
            name: name.to_vec(),
            vertices: vec![Vertex {
                x: 0,
                y: 0,
                z,
                normal: 0,
            }],
            normals: vec![Normal { x: 0, y: 0, z: 0 }],
            polys: Vec::new(),
            lists: vec![FaceList { center: 0, offsets: Vec::new() }],
        })
    }

    fn plane_object(z: i32) -> Rc<ObjectData> {
        plane_object_named(b"\"plane\"", z)
    }

    /// A scene whose camera is instance 0 and whose `objects` are instances
    /// 1.., all on, with an identity camera.
    fn scene_with(objects: Vec<Rc<ObjectData>>) -> Scene {
        let mut instances = vec![Instance {
            object: Rc::new(camera_object()),
            r: Matrix::default(),
            r0: Matrix::default(),
            on: false,
            index: 0,
        }];
        for (number, object) in objects.into_iter().enumerate() {
            instances.push(Instance {
                object,
                r: Matrix::default(),
                r0: Matrix::identity(),
                on: true,
                index: number as u16 + 1,
            });
        }
        Scene {
            master: Master {
                city: 0,
                palette: Palette::new(),
                objects: Vec::new(),
            },
            instances,
            animation: Animation { frames: Vec::new() },
            cam: Matrix::identity(),
            stream: 0,
            xit: false,
        }
    }

    // ---------------------------------------------------------------------
    // Loading and resetting the scene
    // ---------------------------------------------------------------------

    #[test]
    fn load_scene_builds_u2a_s_six_instances_and_shares_the_duplicates() {
        // U2A.00M's object list is `[1, 2, 3, 1, 1]` (`conum` = 6 with the
        // camera): three objects load, and the last two share the first's
        // `Rc` while carrying their own matrices (`U2A.C:170-204`).
        let mut scene = load_scene("U2A").unwrap();
        assert_eq!(scene.instances.len(), 6);
        assert_eq!(scene.master.objects, vec![1, 2, 3, 1, 1]);
        assert_eq!(scene.master.city, 0);
        assert_eq!(scene.stream, 0);
        assert!(!scene.xit);

        // Instance 0 is the camera: a shared empty object that is never on.
        assert_eq!(scene.instances[0].index, 0);
        assert!(!scene.instances[0].on);
        assert!(scene.instances[0].object.vertices.is_empty());
        assert!(scene.instances[0].object.lists.is_empty());

        let indices: Vec<u16> = scene
            .instances
            .iter()
            .map(|instance| instance.index)
            .collect();
        assert_eq!(indices, vec![0, 1, 2, 3, 1, 1]);
        assert!(scene.instances[1..].iter().all(|instance| !instance.on));

        // `e <= f` copies the object struct and makes fresh r/r0
        // (`U2A.C:187-200`): the geometry is shared, the matrices are not.
        assert!(Rc::ptr_eq(
            &scene.instances[1].object,
            &scene.instances[4].object
        ));
        assert!(Rc::ptr_eq(
            &scene.instances[1].object,
            &scene.instances[5].object
        ));
        assert!(!Rc::ptr_eq(
            &scene.instances[1].object,
            &scene.instances[2].object
        ));
        scene.instances[4].r0.x = 77;
        scene.instances[5].r0.m[0] = 1234;
        assert_eq!(scene.instances[1].r0.x, 0);
        assert_eq!(scene.instances[1].r0.m[0], 0);
        assert_eq!(scene.instances[4].r0.m[0], 0);

        // The first scene in U2A.0AA is 1, so the stream is U2A.0AB, read
        // with `conum = 6` (`U2A.C:206-219`).
        assert!(!scene.animation.frames.is_empty());
    }

    #[test]
    fn reset_zeroes_the_matrices_and_rewinds_but_keeps_on_and_xit() {
        let mut scene = load_scene("U2A").unwrap();
        // Frame 0 moves the camera and frame 16 turns instance 2 on.
        for _ in 0..=16 {
            scene.step_frame().unwrap();
        }
        assert!(scene.instances[2].on);
        assert_ne!(scene.cam, Matrix::default());
        assert_ne!(scene.instances[2].r0, Matrix::default());
        scene.stream = 9;
        scene.xit = true;

        scene.reset();
        assert_eq!(scene.stream, 0);
        assert!(scene.xit);
        assert!(scene.instances[2].on);
        assert_eq!(scene.cam, Matrix::default());
        for instance in &scene.instances {
            assert_eq!(instance.r, Matrix::default());
            assert_eq!(instance.r0, Matrix::default());
        }
    }

    // ---------------------------------------------------------------------
    // The animation cursor
    // ---------------------------------------------------------------------

    #[test]
    fn step_frame_applies_toggle_then_deltas_and_handles_the_end_frame() {
        let mut scene = scene_with(vec![plane_object(0)]);
        // A freshly loaded scene starts with every matrix zeroed
        // (`resetscene`, `U2A.C:78-82`), unlike `scene_with`'s ordering
        // fixtures.
        scene.cam = Matrix::default();
        scene.instances[1].r0 = Matrix::default();
        scene.animation = Animation {
            frames: vec![
                Frame {
                    fov: 0x22,
                    end: false,
                    ops: vec![
                        Op {
                            object: 0,
                            toggle: None,
                            dx: 7,
                            dy: 8,
                            dz: 9,
                            dm: [
                                Some(1),
                                None,
                                None,
                                None,
                                Some(-2),
                                None,
                                None,
                                None,
                                Some(3),
                            ],
                        },
                        Op {
                            object: 1,
                            toggle: Some(true),
                            dx: 10,
                            dy: 20,
                            dz: 30,
                            dm: [
                                Some(-5),
                                None,
                                None,
                                None,
                                Some(6),
                                None,
                                None,
                                None,
                                Some(-7),
                            ],
                        },
                    ],
                },
                Frame {
                    fov: 0x1c,
                    end: false,
                    ops: vec![Op {
                        object: 1,
                        toggle: Some(false),
                        dx: 5,
                        dy: 0,
                        dz: 0,
                        dm: [None; 9],
                    }],
                },
                Frame {
                    fov: 0,
                    end: true,
                    ops: Vec::new(),
                },
            ],
        };

        // The camera is object 0: its deltas land in `cam`, not in the
        // camera instance's `r0` (`U2A.C:186-190`, `:202-204`).
        assert_eq!(scene.step_frame(), Some(0x2200));
        assert_eq!(scene.stream, 1);
        assert_eq!(scene.cam.x, 7);
        assert_eq!(scene.cam.y, 8);
        assert_eq!(scene.cam.z, 9);
        assert_eq!(scene.cam.m[0], 1);
        assert_eq!(scene.cam.m[4], -2);
        assert_eq!(scene.cam.m[8], 3);
        assert_eq!(scene.instances[0].r0, Matrix::default());
        assert!(scene.instances[1].on);
        assert_eq!(scene.instances[1].r0.x, 10);
        assert_eq!(scene.instances[1].r0.y, 20);
        assert_eq!(scene.instances[1].r0.z, 30);
        assert_eq!(scene.instances[1].r0.m[0], -5);
        assert_eq!(scene.instances[1].r0.m[4], 6);
        assert_eq!(scene.instances[1].r0.m[8], -7);

        // A toggle still receives its deltas: the original sets `on` and
        // then adds to `r0` unconditionally (`U2A.C:365-424`).
        assert_eq!(scene.step_frame(), Some(0x1c00));
        assert_eq!(scene.stream, 2);
        assert!(!scene.instances[1].on);
        assert_eq!(scene.instances[1].r0.x, 15);

        // 0xFF 0xFF calls resetscene(), sets xit and never draws
        // (`U2A.C:351-356`).
        assert_eq!(scene.step_frame(), None);
        assert!(scene.xit);
        assert_eq!(scene.stream, 0);
        assert_eq!(scene.cam, Matrix::default());
        assert_eq!(scene.instances[1].r0, Matrix::default());
        assert!(!scene.instances[1].on);
    }

    // ---------------------------------------------------------------------
    // Ordering and list selection
    // ---------------------------------------------------------------------

    #[test]
    fn order_instances_sorts_enabled_objects_by_descending_distance() {
        // single_z on the ORD0 centre vertex with an identity camera and
        // r0 (`VISU.C:108`, `U2A.C:291-299`): the distances are the local z.
        let mut scene = scene_with(vec![plane_object(100), plane_object(50)]);
        let driver = Driver::new(Ring::new(Variant::U2a), false);
        assert_eq!(driver.order_instances(&mut scene), vec![1, 2]);

        let mut scene = scene_with(vec![plane_object(10), plane_object(40)]);
        let driver = Driver::new(Ring::new(Variant::U2a), false);
        assert_eq!(driver.order_instances(&mut scene), vec![2, 1]);

        // Equal distances keep their order (`dis > co[order[b]].dist` is
        // strict, `U2A.C:311-317`).
        let mut scene = scene_with(vec![plane_object(50), plane_object(50)]);
        assert_eq!(driver.order_instances(&mut scene), vec![1, 2]);
    }

    #[test]
    fn order_instances_skips_the_camera_and_the_objects_that_are_off() {
        // The loop starts at 1 to skip the camera (`U2A.C:290-291`), and a
        // camera that is somehow on still never enters the order.
        let mut scene = scene_with(vec![plane_object(30), plane_object(20)]);
        scene.instances[0].on = true;
        scene.instances[2].on = false;
        let driver = Driver::new(Ring::new(Variant::U2a), false);
        assert_eq!(driver.order_instances(&mut scene), vec![1]);
    }

    /// An object with one list per centre: `ORD0` first, then the `ORDE`s.
    fn object_with_lists(centers: &[u16]) -> ObjectData {
        ObjectData {
            lists: centers.iter().map(|&center| FaceList { center, offsets: Vec::new() }).collect(),
            ..camera_object()
        }
    }

    fn at_z(z: i32) -> Rotated {
        Rotated { z, ..Default::default() }
    }

    #[test]
    fn pick_list_chooses_the_nearest_center_and_ignores_ord0_s_center() {
        // `a = 0`, then every ORDE centre with a smaller z replaces it
        // (`VISU.C:113-123`): ORD0's centre is never compared, so its -100
        // does not win.
        let rotated = [at_z(-100), at_z(5), at_z(40)];
        assert_eq!(pick_list(&object_with_lists(&[0, 1, 2]), &rotated), 1);
        // With no ORDE the default list 0 is `plnum == 1` (`VISU.C:31`).
        assert_eq!(pick_list(&object_with_lists(&[0]), &rotated), 0);
        // `bl < al` is strict (`VISU.C:118`): a tie keeps the first.
        let tied = [at_z(0), at_z(5), at_z(5)];
        assert_eq!(pick_list(&object_with_lists(&[0, 1, 2]), &tied), 1);
    }

    // ---------------------------------------------------------------------
    // The U2A-only and U2E-only ordering hacks
    // ---------------------------------------------------------------------

    #[test]
    fn the_city_hacks_are_u2a_s_and_guard_out_of_range_instances() {
        // co[2], co[7] and co[13] are forced to 1e9 for city 1
        // (`U2A.C:301-306`). With distances equal to the instance numbers the
        // unforced objects stay in descending order behind them.
        let objects = (1..=14).map(plane_object).collect::<Vec<_>>();
        let mut scene = scene_with(objects);
        scene.master.city = 1;
        let driver = Driver::new(Ring::new(Variant::U2a), false);
        assert_eq!(
            driver.order_instances(&mut scene),
            vec![2, 7, 13, 14, 12, 11, 10, 9, 8, 6, 5, 4, 3, 1]
        );

        // `city == 2` forces only co[14] (`U2A.C:307-310`). It is ignored
        // when the scene has fewer instances.
        let objects = (1..=15).map(plane_object).collect::<Vec<_>>();
        let mut scene = scene_with(objects);
        scene.master.city = 2;
        assert_eq!(
            driver.order_instances(&mut scene),
            vec![14, 15, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1]
        );

        let mut scene = scene_with(vec![plane_object(1)]);
        scene.master.city = 1;
        assert_eq!(driver.order_instances(&mut scene), vec![1]);

        // U2E.C has no city hack, so the U2E cadence ignores the flag.
        let objects = (1..=14).map(plane_object).collect::<Vec<_>>();
        let mut scene = scene_with(objects);
        scene.master.city = 1;
        let u2e = Driver::new(Ring::new(Variant::U2e), false);
        assert_eq!(
            u2e.order_instances(&mut scene),
            vec![14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1]
        );
    }

    #[test]
    fn the_u2e_name_hacks_read_the_quoted_name() {
        // U2E stores the quotes: `"_p"` has `name[1] == b'_'`, and the
        // distance becomes 1e9 instead of the computed z (`U2E.C:408`).
        let mut scene = scene_with(vec![plane_object_named(b"\"_p\"", 10), plane_object(100)]);
        let driver = Driver::new(Ring::new(Variant::U2e), false);
        assert_eq!(driver.order_instances(&mut scene), vec![1, 2]);

        // U2A.C has neither name hack: the same scene sorts by distance.
        let u2a = Driver::new(Ring::new(Variant::U2a), false);
        assert_eq!(u2a.order_instances(&mut scene), vec![2, 1]);

        // `s01` is forced to 1 only strictly inside 1800 < currframe < 2200
        // (`U2E.C:410-416`).
        let mut scene = scene_with(vec![plane_object_named(b"\"s01\"", 100), plane_object(50)]);
        let mut driver = Driver::new(Ring::new(Variant::U2e), false);
        driver.ring.currframe = 1800;
        assert_eq!(driver.order_instances(&mut scene), vec![1, 2]);
        driver.ring.currframe = 1801;
        assert_eq!(driver.order_instances(&mut scene), vec![2, 1]);
        driver.ring.currframe = 2199;
        assert_eq!(driver.order_instances(&mut scene), vec![2, 1]);
        driver.ring.currframe = 2200;
        assert_eq!(driver.order_instances(&mut scene), vec![1, 2]);

        // A short name must not be indexed past its end.
        let mut scene = scene_with(vec![plane_object_named(b"\"", 10), plane_object(100)]);
        assert_eq!(driver.order_instances(&mut scene), vec![2, 1]);
    }

    // ---------------------------------------------------------------------
    // The four-page ring
    // ---------------------------------------------------------------------

    /// `(tick, page, page frames, stream frames)` for each queued page.
    type Queued = Vec<(usize, usize, usize, usize)>;

    /// One fixed 70 Hz tick: the copper interrupt runs, then the part's loop
    /// takes the writer page if one is free, schedules, queues and parses the
    /// stream (`U2A.C:259-336`). "Displayed" is sampled after the tick.
    fn ring_ticks(variant: Variant, ticks: usize) -> (Queued, Vec<usize>) {
        let mut ring = Ring::new(variant);
        let mut queued = Vec::new();
        let mut displayed = Vec::new();
        for tick in 1..=ticks {
            ring.isr();
            if let Some(page) = ring.next_page() {
                let (page_frames, stream_frames) = ring.schedule();
                ring.queue(page, page_frames);
                queued.push((tick, page, page_frames, stream_frames));
            }
            displayed.push(ring.displayed());
        }
        (queued, displayed)
    }

    #[test]
    fn ring_matches_the_u2a_cadence_over_twenty_ticks() {
        // U2A.C:326-336: avgrepeat tracks the lag, and the page is queued for
        // that many retraces; the stream advances the same number of frames.
        let (queued, displayed) = ring_ticks(Variant::U2a, 20);
        assert_eq!(
            queued,
            vec![
                (1, 0, 1, 1),
                (2, 1, 1, 1),
                (3, 2, 1, 1),
                (17, 3, 8, 8),
                (18, 0, 8, 8),
                (19, 1, 4, 4),
                (20, 2, 1, 1),
            ]
        );
        let mut expected = vec![0usize; 16];
        expected.extend([1, 2, 3, 3]);
        assert_eq!(displayed, expected);
    }

    #[test]
    fn ring_matches_the_u2e_cadence_over_twenty_ticks() {
        // U2E.C:434-444: `repeat = a + 1` (clamped at 0), the page is queued
        // for `max(repeat, 1)` retraces and the stream advances `(repeat+1)/2`
        // two-retrace steps.
        let (queued, displayed) = ring_ticks(Variant::U2e, 20);
        assert_eq!(
            queued,
            vec![
                (1, 0, 2, 1),
                (2, 1, 1, 1),
                (3, 2, 1, 0),
                (17, 3, 14, 7),
                (19, 0, 2, 1),
                (20, 1, 1, 1),
            ]
        );
        let mut expected = vec![0usize; 17];
        expected.extend([1, 2, 3]);
        assert_eq!(displayed, expected);
    }

    #[test]
    fn next_page_waits_for_the_copper_then_escapes_a_deadlock() {
        let mut ring = Ring::new(Variant::U2a);
        ring.ready[0] = 1;
        assert_eq!(ring.next_page(), None);
        // The original spins while `cl[clw].ready` and breaks at deadlock >
        // 16 (`U2A.C:278-283`); the counter is reset by a successful wait.
        ring.deadlock = 16;
        assert_eq!(ring.next_page(), None);
        assert_eq!(ring.deadlock, 16);
        ring.deadlock = 17;
        assert_eq!(ring.next_page(), Some(0));
        assert_eq!(ring.deadlock, 0);

        ring.ready[0] = 0;
        ring.deadlock = 5;
        assert_eq!(ring.next_page(), Some(0));
        assert_eq!(ring.deadlock, 0);
    }

    // ---------------------------------------------------------------------
    // The per-frame draw
    // ---------------------------------------------------------------------

    #[test]
    fn u2a_draws_the_background_and_frame_sixteen_shows_object_two() {
        let mut scene = load_scene("U2A").unwrap();
        let background = load_path(Path::new(BACKGROUND)).unwrap();
        assert_eq!(background.pixels().len(), 320 * 200);

        let mut pixels = vec![0xFFu8; 320 * 200];
        let mut driver = Driver::new(Ring::new(Variant::U2a), false);
        // The acceptance checks the object's first on-frame against the full
        // 320x200 page. At frame 16 every one of the saucer's projected
        // vertices is still above the shipped 25..174 window, so
        // `vis_drawobject`'s object-level cull (`VISU.C:111-112`) would hide
        // it; the shipped window is pinned separately below.
        driver.proj = window(0, 319, 0, 199, 512, 9_999_999);
        let draw = |scene: &mut Scene, driver: &mut Driver, pixels: &mut Vec<u8>| {
            let mut page = Page::new(pixels, 320);
            driver.draw_frame(scene, &mut page, Clear::Background(background.pixels()));
        };

        // The first draw is before the first stream frame: fov is 0 and every
        // object is off, so the page is the cleared background, and
        // `vid_cameraangle(0)` has already run (`U2A.C:287-288`).
        assert!(scene.instances.iter().all(|instance| !instance.on));
        draw(&mut scene, &mut driver, &mut pixels);
        assert_eq!(pixels, background.pixels());
        assert_eq!((driver.proj.mul_x, driver.proj.mul_y), (3256, 2861));

        // Frames 0..=15 move the camera (frame 0) and leave every object off.
        for _ in 0..16 {
            driver.fov = scene.step_frame().unwrap();
            draw(&mut scene, &mut driver, &mut pixels);
            assert_eq!(pixels, background.pixels());
        }

        // Frame 16 turns object 2 (instance 2, file U2A.002) on
        // (`U2A.C:336-430`); the following draw has its pixels.
        assert!(!scene.instances[2].on);
        driver.fov = scene.step_frame().unwrap();
        assert_eq!(driver.fov, 0x2200);
        assert!(scene.instances[2].on);
        draw(&mut scene, &mut driver, &mut pixels);
        let drawn = pixels
            .iter()
            .zip(background.pixels())
            .filter(|(pixel, clear)| pixel != clear)
            .count();
        assert!(drawn > 0, "object 2 drew nothing");
    }

    #[test]
    fn u2a_s_shipped_window_culls_the_saucer_until_frame_twenty_two() {
        // With `vid_window(0,319,25,174,512,9999999)` (`U2A.C:247`) every
        // one of U2A.002's projected vertices is above `clip_y[0] = 25` when
        // frame 16 turns it on, so `calc_project`'s AND is VF_UP and
        // `vis_drawobject` returns before the list (`VISU.C:111-112`). The
        // stream's negative `dy` deltas descend it into the window only at
        // frame 22: frames 16..=21 draw nothing and frame 22 draws.
        let mut scene = load_scene("U2A").unwrap();
        let background = load_path(Path::new(BACKGROUND)).unwrap();
        let mut pixels = vec![0u8; 320 * 200];
        let mut driver = Driver::new(Ring::new(Variant::U2a), false);
        let draw = |scene: &mut Scene, driver: &mut Driver, pixels: &mut Vec<u8>| {
            let mut page = Page::new(pixels, 320);
            driver.draw_frame(scene, &mut page, Clear::Background(background.pixels()));
        };

        draw(&mut scene, &mut driver, &mut pixels);
        for _ in 0..16 {
            driver.fov = scene.step_frame().unwrap();
            draw(&mut scene, &mut driver, &mut pixels);
        }
        driver.fov = scene.step_frame().unwrap();
        assert!(scene.instances[2].on);
        draw(&mut scene, &mut driver, &mut pixels);
        assert_eq!(pixels, background.pixels());

        // Frames 17..=21 still leave every projected vertex above the window.
        for _ in 0..5 {
            driver.fov = scene.step_frame().unwrap();
            draw(&mut scene, &mut driver, &mut pixels);
            assert_eq!(pixels, background.pixels());
        }

        // Frame 22's descent is the first one the cull lets through.
        driver.fov = scene.step_frame().unwrap();
        draw(&mut scene, &mut driver, &mut pixels);
        let drawn = pixels
            .iter()
            .zip(background.pixels())
            .filter(|(pixel, clear)| pixel != clear)
            .count();
        assert!(drawn > 0, "object 2 never entered the shipped window");
    }

    #[test]
    fn u2e_skips_the_first_draw_and_resets_the_clock() {
        // `firstframe`: the whole draw is skipped, and the clock is zeroed
        // (`U2E.C:386-387`, `:433`).
        let mut scene = scene_with(Vec::new());
        let mut pixels = vec![0x5Au8; 320 * 200];
        let mut driver = Driver::new(Ring::new(Variant::U2e), true);
        driver.ring.syncframe = 99;
        driver.skip_first_draw();
        assert!(!driver.first_draw);
        assert_eq!(driver.ring.syncframe, 0);
        // The next draw is a real one.
        {
            let mut page = Page::new(&mut pixels, 320);
            driver.draw_frame(&mut scene, &mut page, Clear::Byte(0));
        }
        assert!(pixels.iter().all(|&pixel| pixel == 0));
    }
}
