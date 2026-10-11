# Phase 1 — Assets & references · Implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to
> implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make every asset the port needs decode from its original format and prove it matches the
source art, and capture reference frames for every part in the sequence.

**Architecture:** A new `crates/sr-assets` holds one decoder per original format family, each writing
into an `sr-engine` `Surface` and returning a 6-bit `Palette`; `assets/` holds the curated subset
copied out of the pinned upstream tree with a provenance file; `xtask assets verify` re-checks the
tree, decodes everything and renders contact sheets a human can look at. Separately, `xtask reference
capture` grows a boot recipe that repeats exactly, so a whole-part reference set becomes reproducible
rather than a one-off, and `docs/fidelity/reference-indices.md` records the frame index of each part.

**Tech stack:** Rust 2021, `rstest` for table tests, the `png` crate (already an `xtask` dependency)
for contact sheets. Hashing is `sha256sum` in the shell steps — no hashing crate is added, and
`sr-assets` depends on `sr-engine` and nothing else.

**Spec:** `docs/spec/2026-10-10-second-reality-rust-design.md` — §4.4 (the format table), §8.1–8.4
(verification and its limits), §9 (asset handling, the dedup list), §10 (this phase's exit criteria).

## Global Constraints

- **`sr-engine` keeps zero runtime dependencies.** `sr-assets` may depend on `sr-engine` only. No
  image, archive or codec crate: the decoders are the deliverable.
- **Assets are read in their original formats.** No LBM→PNG, no re-encoding, no "cleaned" copies.
  A file that needs repair is a finding, not a fix.
- **`reference/` and `upstream/` are input-only.** Nothing any task writes may land inside them;
  `compare`'s existing guard is the precedent for how that is enforced.
- **6-bit palette internally.** Every palette a loader produces goes through
  `sr_engine::dac_expand`'s inverse, and a test proves the round trip over all 64 values.
- **`sr-assets` manifests set `license = "MIT"`**; workspace inheritance for the rest.
- **Every claim about the original cites `file:line`** in the commit message, as Phase 0 did. The
  upstream commit is `071a82e` (`mtuomi/SecondReality`).
- **Preserve the original's bugs** (`docs/fidelity/bugs.md`); an asset that decodes "wrongly" in a
  way the demo depends on is decoded wrongly here too, with the reasoning recorded.

## Review Focus

The five input classes most likely to bite, each with the task that pins it:

1. **A truncated or corrupt asset.** Every loader must return `AssetError`, never panic and never
   read past its input. Pinned in A2 (Origin), A3 (IFF), A4 (RIX3) with deliberately cut buffers.
2. **An asset whose magic contradicts its extension.** `sniff` decides; the extension is a hint at
   most. A `.UH` that is really an IFF must decode as an IFF. Pinned in A2.
3. **An image larger than the surface the engine can allocate.** `MAX_WIDTH`/`MAX_HEIGHT` are
   640×400; a 1024-wide asset must be refused with a legible message, not clipped or wrapped.
   Pinned in A2.
4. **An 8-bit palette from an IFF CMAP.** Converting 8→6 bits through anything but the exact inverse
   of `dac_expand` silently shifts every colour in the demo. Pinned in A3 over all 256 values.
5. **A part whose geometry changes during the frames being captured.** `video/frame` follows the
   guest; one manifest carries one geometry (D15). Pinned in B2.

---

## Part A — the asset pipeline

### Task A1: Asset inventory, curated tree, provenance

**Files:**
- Create: `docs/fidelity/asset-inventory.md`, `assets/PROVENANCE.md`
- Create: `assets/<part>/<file>` — the curated copies
- Read: `upstream/SecondReality/<part>/MAKEFILE`, the part's `.C`/`.ASM`, spec §9

**Interfaces:**
- Produces: the inventory table (part → asset → format → standalone or EXE-only) that A5's input list
  and every later task's test fixtures are drawn from; and the `assets/` paths the loaders' tests
  read.

- [ ] **Step 1: Build the inventory**

For each of the 20 numbered parts in `docs/fidelity/part-sequence.md` §1, read the part's source
directory. Two sources name an asset: the link inputs in the part's `MAKEFILE` (`_*.OBK`, `_*.OBJ`)
and the filenames in its `.C`/`.ASM`. Record one row per asset: part, upstream path, size, **format
decided by magic bytes, not by extension**, and whether a standalone file exists or the bytes only
exist inside an `.OBK`/`.OBJ` blob.

Cross-check with `MAIN/PACKFINA.INC:3-81`, which lists what the shipped `SECOND.EXE` carries.

- [ ] **Step 2: Verify spec §9's dedup list by hash, and correct it**

§9 claims `TMP.BBM` ×4, `ENDPIC/PIC.LBM` ≡ `JPLOGO/PIC.LBM`, `END/PIC.LBM` ≡ `NUTS.LBM`, the
U2A/U2E scene sets duplicated between `VISU/C/SCENE` and `MAIN/DATA`, `TROLL.UP` ×2, `O2.SCI` ×2,
`SIN1024.INC` ×9. Run `sha256sum` over each claimed group and record the result per claim. A claim
that does not hold is a finding written into the inventory, not a silent omission.

- [ ] **Step 3: Copy the needed subset into `assets/`**

Mirror the upstream layout (`assets/<PART>/<FILE>`), so a copy's origin is obvious from its path.
Collapse each verified duplicate set to one canonical copy.

- [ ] **Step 4: Write `assets/PROVENANCE.md`**

One table row per file in `assets/`: path, upstream path, upstream commit `071a82e`, bytes, sha256,
licence (Unlicense / public domain), the parts that need it, and for a collapsed duplicate the
canonical path it was collapsed onto.

- [ ] **Step 5: Verify every copy is byte-identical to its origin**

Run, and record the output in the commit message:

```bash
cd /home/ew/workspace/reality/SecondRealityRust
for f in $(find assets -type f ! -name PROVENANCE.md); do
  up="upstream/SecondReality/${f#assets/}"
  cmp -s "$f" "$up" || echo "DIFFERS: $f vs $up"
done
echo "checked $(find assets -type f ! -name PROVENANCE.md | wc -l) files"
```

Expected: no `DIFFERS` lines. Files recovered from a blob in A5 are the only ones exempt, and they
do not exist yet.

- [ ] **Step 6: Commit**

```bash
git add assets docs/fidelity/asset-inventory.md
git commit -m "Inventory the assets each part needs, and copy the subset into assets/"
```

---

### Task A2: `sr-assets` skeleton and the Origin PIC container

The container behind `.UH`, `.U`, `.UP`, `.P` and `.16` — five extensions, one format: magic
`0xFCFC` (raw) or `0xFCFD` (RLE), a 16-byte header, a 768-byte palette, chunky pixels. `.X` is the
same family's palette-less variant with the ASCII magic `UH\x01\x00`. This one task therefore
retires five of §4.4's rows plus `.X`.

**Files:**
- Create: `crates/sr-assets/Cargo.toml`, `crates/sr-assets/src/lib.rs`, `crates/sr-assets/src/origin.rs`
- Create: `crates/sr-assets/tests/origin.rs`
- Modify: workspace `Cargo.toml` (`members`)
- Read: `upstream/SecondReality/GRAB/SP.C` (the viewer — the format's authority),
  `GRAB/GFXSAVE.H`

**Interfaces:**
- Consumes: `sr_engine::{Palette, Surface, dac_expand}`, `sr_engine::surface::{MAX_WIDTH, MAX_HEIGHT}`.
- Produces, relied on by every later task:

```rust
// crates/sr-assets/src/lib.rs
pub struct Image { pub surface: Surface, pub palette: Palette }
impl Image { pub fn width(&self) -> u16; pub fn height(&self) -> u16; pub fn pixels(&self) -> &[u8]; }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format { Origin, OriginX, Iff, Rix3, Pal, Fli, Ani }

#[derive(Debug)]
pub enum AssetError {
    UnknownFormat,
    Truncated { format: Format, needed: usize, had: usize },
    Unsupported { format: Format, what: String },
    TooLarge { width: u32, height: u32 },
    Palette { what: String },
}
impl std::fmt::Display for AssetError { /* one line per variant */ }
impl std::error::Error for AssetError {}

/// By magic. `None` when nothing matches.
pub fn sniff(bytes: &[u8]) -> Option<Format>;
pub fn load(bytes: &[u8], format: Format) -> Result<Image, AssetError>;
pub fn load_path(path: &std::path::Path) -> Result<Image, AssetError>;   // sniffs, errors if None
pub fn load_frames(bytes: &[u8], format: Format) -> Result<Vec<Image>, AssetError>;

/// The exact inverse of `sr_engine::dac_expand` over 0..=63.
pub fn six_bit(v: u8) -> u8;
```

- [ ] **Step 1: Write the failing tests** in `crates/sr-assets/tests/origin.rs`

Real-file agreement is the phase's own test of "matches the source art", and the tree hands us free
pairs — the same picture in two formats:

```rust
#[test]
fn the_packed_title_and_the_iff_title_are_the_same_picture() {
    let up = load_path(Path::new("../../assets/BEG/SRTITLE.UP")).unwrap();
    let lbm = load_path(Path::new("../../assets/BEG/SRTITLE.LBM")).unwrap();
    assert_eq!((up.width(), up.height()), (lbm.width(), lbm.height()));
    assert_eq!(up.pixels(), lbm.pixels());
}

#[test]
fn a_thirty_pixel_tall_font_strip_reads_as_one() {
    // The header of ALKU/FONA.UH, read off the file: 0xFCFC, 1500 x 30, 768
    // palette bytes, pixels at paragraph 0x31 (= 784).
    let img = load_path(Path::new("../../assets/ALKU/FONA.UH")).unwrap();
    assert_eq!((img.width(), img.height()), (1500, 30));
}

/// Every Origin file in the curated tree decodes, and the geometry its header
/// claims matches the payload it actually carries. This is the property that
/// catches a misread header field across all five extensions at once, so it
/// does not depend on anyone having transcribed dimensions correctly.
#[test]
fn every_origin_asset_agrees_with_its_own_header() {
    for path in origin_assets_in("../../assets") {
        let raw = std::fs::read(&path).unwrap();
        let img = load(&raw, Format::Origin).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let header_pixels = img.width() as usize * img.height() as usize;
        let payload = raw.len() - origin_pixel_offset(&raw);
        if raw[1] == 0xFC {
            assert_eq!(payload, header_pixels, "{}: raw payload", path.display());
        } else {
            assert!(payload <= header_pixels, "{}: RLE payload cannot exceed", path.display());
        }
    }
}

#[test]
fn a_palette_less_x_file_decodes_and_keeps_the_callers_palette() { /* COMAN/COMBG.X */ }

#[test]
fn a_raw_and_an_rle_file_of_the_same_picture_agree() { /* COMAN/COMBG.X vs COMBG.UH */ }

#[test]
fn a_truncated_pixel_run_is_refused_not_panicked_on() {
    let mut bytes = std::fs::read("../../assets/BEG/SRTITLE.UP").unwrap();
    bytes.truncate(bytes.len() / 2);
    assert!(matches!(load(&bytes, Format::Origin), Err(AssetError::Truncated { .. })));
}

#[test]
fn an_oversized_header_is_refused_rather_than_clipped() {
    let mut bytes = std::fs::read("../../assets/BEG/SRTITLE.UP").unwrap();
    bytes[2..4].copy_from_slice(&1024u16.to_le_bytes());
    assert!(matches!(load(&bytes, Format::Origin), Err(AssetError::TooLarge { .. })));
}

#[test]
fn the_extension_never_overrides_the_magic() {
    // An IFF file named .UH is an IFF: `sniff` looks at bytes only.
    assert_eq!(sniff(&std::fs::read("../../assets/BEG/SRTITLE.LBM").unwrap()), Some(Format::Iff));
}

#[test]
fn six_bit_is_the_exact_inverse_of_the_engines_widening() {
    for c in 0u8..=63 { assert_eq!(six_bit(dac_expand(c)), c, "6-bit value {c}"); }
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p sr-assets`
Expected: FAIL — `error[E0433]: failed to resolve: use of undeclared crate or module sr_assets`.

- [ ] **Step 3: Create the crate and implement the container**

Cargo manifest: `sr-engine = { path = "../sr-engine" }`, `rstest` as a dev-dependency, `license = "MIT"`.

Then implement `crates/sr-assets/src/origin.rs`. **Read `GRAB/SP.C` first** — it is the format's
authority and the ASCII art in this task is only what the files showed:

```
0x00 u16 magic        0xFCFC raw, 0xFCFD RLE (`.X`: ASCII "UH\x01\x00")
0x02 u16 width
0x04 u16 height
0x06 u16 colours      (SP.C defines it; several files hold 1 here and still carry 768 palette bytes)
0x08 u16 pixel offset in 16-byte paragraphs  (0x31 in every file inspected, i.e. 784 = 16 + 768)
0x0A 6 bytes          zero in every file inspected
0x10 768 bytes        palette, 3 bytes per entry, each byte a 6-bit value (0..=63)
para*16              pixel data, one byte per pixel, row-major
```

The RLE opcode scheme for `0xFCFD` is SP.C's — do not infer it from the file sizes. `.X` carries no
palette, so `load` returns `Image` with `Palette::BLACK` and `load_path` documents that the caller
supplies the palette (the part that uses `COMBG.X` loads `COMBG.UH`'s palette).

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p sr-assets`
Expected: PASS, including the cross-format equality.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml crates/sr-assets
git commit -m "Add sr-assets and decode the Origin PIC container (.UH/.U/.UP/.P/.16/.X)"
```

---

### Task A3: IFF — ILBM and PBM

The tree carries **both** IFF flavours and the tally is not obvious from any one file: 38 files are
`FORM .... ILBM` (planar, `nPlanes` bitplanes) and 46 are `FORM .... PBM ` (one byte per pixel).
§4.4's "stencil/EHB, multiple bitplane counts" applies to the ILBM half only; check whether either
feature is actually used before supporting it, and record the answer.

**Files:**
- Create: `crates/sr-assets/src/iff.rs`, `crates/sr-assets/tests/iff.rs`
- Read: `upstream/SecondReality/GRAB/LOADLBM.C`, `GRAB/VIEWLBM.C`, `GRAB/DEFLBM.INC`

**Interfaces:**
- Consumes: `AssetError`, `Image`, `Format::Iff`, `six_bit` (A2).
- Produces: `sr_assets::iff::decode(bytes: &[u8]) -> Result<Image, AssetError>`, reached through
  `load(bytes, Format::Iff)`. The chunk walker and ByteRun1 are private to the module.

- [ ] **Step 1: Write the failing tests** in `crates/sr-assets/tests/iff.rs`

```rust
/// A 4x2 ILBM built here, so the expected pixels are known independently of
/// any asset: two bitplanes, ByteRun1-compressed, plus a CMAP. This is the
/// test that pins the bitplane interleave and the RLE together.
#[test]
fn a_hand_built_planar_ilbm_decodes_to_its_known_pixels() {
    let img = load(&hand_built_ilbm_4x2(), Format::Iff).unwrap();
    assert_eq!((img.width(), img.height()), (4, 2));
    assert_eq!(img.pixels(), &[0, 1, 2, 3, 3, 2, 1, 0]);
    assert_eq!(img.palette.entry(3), [63, 0, 0]);   // from the hand-built CMAP
}

#[test]
fn a_pbm_decodes_to_the_dimensions_in_its_header() {
    // Any chunky file from assets/; A1's inventory names them.
}

#[test]
fn every_iff_asset_agrees_with_its_own_bmhd() { /* as in A2: payload vs header */ }

#[test]
fn an_eight_bit_cmap_becomes_six_bits_through_the_inverse_widening() {
    for v in 0u8..=255 {
        let six = six_bit(v);
        assert!((dac_expand(six) as i32 - v as i32).abs() <= 2, "8-bit {v} -> 6-bit {six}");
        assert_eq!(six, six_bit(v), "stable");
    }
}

#[test]
fn a_run_that_ends_exactly_at_the_row_boundary_is_not_a_truncation() { /* hand-built BODY */ }

#[test]
fn a_body_shorter_than_bmhd_promises_is_refused() {
    assert!(matches!(load(&cut, Format::Iff), Err(AssetError::Truncated { .. })));
}

#[test]
fn unknown_chunks_are_skipped_not_fatal() { /* insert a CRNG chunk into a real file */ }
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p sr-assets iff`
Expected: FAIL — `Format::Iff` decodes to `AssetError::Unsupported`.

- [ ] **Step 3: Implement `iff.rs`**

Chunk walk: `FORM` → `ILBM`/`PBM` → `BMHD` (20 bytes: `w u16, h u16, x u16, y u16, nPlanes u8,
masking u8, compression u8, pad u8, transparent u16, xAspect u8, yAspect u8, pageW i16, pageH i16`),
`CMAP` (3 bytes per entry, 8-bit), `BODY`. Ignore everything else. Decompress ByteRun1 per row
(decode to the row's byte width, `nPlanes * ((w + 15) / 16) * 2` for ILBM, `w` for PBM), then
interleave bitplanes for ILBM. `masking == 1` (a stencil plane) and `CAMG & 0x80` (EHB) are refused
with `Unsupported` until a file is found that needs them — the inventory says whether any is.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p sr-assets`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/sr-assets
git commit -m "Decode IFF ILBM and PBM, both flavours the tree actually uses"
```

---

### Task A4: RIX3 and `.PAL`

`RIX3` is the magic of an external paint program (the tree holds no writer), so the format is
established from the files and from how the parts read them. `.PAL` is 768 bytes of 6-bit VGA
palette, no header.

**Files:**
- Create: `crates/sr-assets/src/rix.rs`, `crates/sr-assets/src/pal.rs`,
  `crates/sr-assets/tests/rix.rs`, `crates/sr-assets/tests/pal.rs`
- Read: the loading code in the parts that use them — `TUNNELI/` (`COLORS.CLX`), `WATER/`
  (`BKG.CLX`, `GREEN.CLX`, `SWORD.SCI`), `FOREST/` (`O2.SCI`), `VISU/C/` (`CITY.PAL`)

**Interfaces:**
- Consumes: `AssetError`, `Image`, `Palette`, `six_bit` (A2).
- Produces:
  - `Format::Rix3` through `load`/`sniff` (`"RIX3"` is a magic).
  - `sr_assets::pal::decode(bytes: &[u8]) -> Result<Palette, AssetError>` — a `.PAL` has **no
    magic**, so it is loaded by construction and never sniffed. `Format::Pal` exists in the enum for
    `load(bytes, Format::Pal)` to route to the same function; `sniff` never returns it.

- [ ] **Step 1: Establish the header from the parts' own reads**

Before writing a test, find the code that consumes a `.SCI`/`.CLX` in the part that ships it, and
write down its offsets. The observed header is `"RIX3"` + `u16 width` + `u16 height` + `u16 0x00AF` +
`u16 0x0000`, then 768 palette bytes at offset 10 — but `0x00AF` is unexplained by the files alone
and the part's read is what decides. Same for `.PAL`: exactly 768 bytes, 6-bit, in `PANIC/MONSTER.PAL`,
`PAM/PAL.PAL`, `START/HZPIC.PAL`.

- [ ] **Step 2: Write the failing tests**

```rust
#[rstest]
#[case("../../assets/WATER/BKG.CLX", 320, 200)]  // header read off the file
#[case("../../assets/FOREST/O2.SCI", 640, 32)]    // header read off the file
fn rix3_decodes_a_screen(#[case] path: &str, #[case] w: u16, #[case] h: u16) {
    let img = load_path(Path::new(path)).unwrap();
    assert_eq!((img.width(), img.height()), (w, h));
}

/// The same property A2 and A3 use: the geometry a header claims must match the
/// payload the file carries, over every RIX3 file in the tree.
#[test]
fn every_rix3_asset_agrees_with_its_own_header() { … }

#[test]
fn a_pal_file_is_256_entries_of_six_bits() {
    let p = pal::decode(&std::fs::read("../../assets/PAM/PAL.PAL").unwrap()).unwrap();
    for i in 0..=255u8 { assert!(p.entry(i).iter().all(|&c| c <= 63), "entry {i}"); }
}

#[test]
fn a_pal_file_that_is_not_768_bytes_is_refused() {
    assert!(matches!(pal::decode(&[0u8; 767]), Err(AssetError::Truncated { .. })));
}

#[test]
fn a_rix3_file_whose_pixels_stop_early_is_refused() { … }
```

- [ ] **Step 3: Run to verify they fail, then implement `rix.rs` and `pal.rs`** — `Format::Rix3`
  through `load`/`sniff`, `.PAL` through `pal::decode` as the Interfaces block specifies.

- [ ] **Step 4: Run the tests to verify they pass.** `cargo test -p sr-assets`

- [ ] **Step 5: Commit**

```bash
git add crates/sr-assets
git commit -m "Decode RIX3 screens and 768-byte VGA palettes"
```

---

### Task A5: Recover the EXE-only assets from OMF `LEDATA` records

Eleven `.OBK` blobs in the tree have **no standalone file of the same size** — `GRID/_EYE.OBK`,
`LENS/_LENSEX0.OBK`, `LENS/_LENSEXB.OBK`, `JPLOGO/_PIC.OBK`, `GLENZ/_FC.OBK`, `GLENZ/_BGPIC.OBK`,
`VISU/C/_BG.OBK`, `TECHNO/_PIC1.OBK`, `TECHNO/_PIC2.OBK`, `TECHNO/_CIRCLE.OBK`,
`TECHNO/_CIRCLE2.OBK`. (A1's inventory decides which of these actually *are* assets; some may be
code.) The other ten blobs duplicate a standalone file and are already covered.

**Files:**
- Create: `crates/sr-assets/src/omf.rs`, `crates/sr-assets/tests/omf.rs`
- Create (via the tool): the recovered files under `assets/<PART>/`
- Modify: `assets/PROVENANCE.md` (each recovered file's origin is a blob + record, not a file)

**Interfaces:**
- Consumes: A2's `sniff`/`load`.
- Produces: `sr_assets::omf::extract_ledata(bytes: &[u8]) -> Result<Vec<u8>, AssetError>` — the
  concatenated `LEDATA` payload of an OMF object, in record order.

**§4.4 lists `.OBK`/`.OBJ` as "not needed", and this task reads them.** There is no contradiction and
the distinction matters: `.OBK` is not an asset format the port ever loads at runtime — it is the
*container* the original's linker put the asset in. §9 is the clause that applies: "assets that exist
only compiled into a shipped EXE are recovered from OMF LEDATA records during Phase 1". A recovered
asset is written out as a standalone file in its own format and from then on is loaded like any
other; nothing in `sr-parts` or the binary ever sees an `.OBK`.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn a_blob_that_holds_a_picture_yields_one_that_decodes() {
    let bytes = std::fs::read("../../upstream/SecondReality/TECHNO/_PIC1.OBK").unwrap();
    let payload = extract_ledata(&bytes).unwrap();
    let at = payload.windows(2).position(|w| w == [0xFC, 0xFC] || w == [0xFD, 0xFC])
        .expect("an Origin PIC magic inside the object");
    let img = load(&payload[at..], Format::Origin).unwrap();
    assert!(img.width() > 0 && img.height() > 0);
}

#[test]
fn a_blob_with_no_ledata_is_refused() { … }

#[test]
fn a_record_that_runs_past_the_end_of_the_blob_is_refused() {
    // The first byte is the record type, the next two the length: a length that
    // overruns must be an error, not a panic.
    assert!(matches!(extract_ledata(&[0xA1, 0xFF, 0xFF, 0x00]), Err(AssetError::Truncated { .. })));
}
```

- [ ] **Step 2: Run to verify it fails.** `cargo test -p sr-assets omf`

- [ ] **Step 3: Implement `omf.rs`**

OMF records are a type byte followed by a `u16` little-endian length; `LEDATA` is `0xA1` and its
payload is `u16 segment index` then data. Concatenate payloads in record order.

- [ ] **Step 4: Recover the assets and extend the provenance**

For each blob A1's inventory marked as holding an asset, recover it, name it for what the part calls
it, and add it under `assets/<PART>/`. `PROVENANCE.md` gains a second table — "recovered assets" —
with the blob path, the record kind, the byte range inside the payload, and the sha256 of the
recovered file. Nothing is recovered speculatively: a blob that only holds code is recorded as such.

- [ ] **Step 5: Run the tests to verify they pass, then commit**

```bash
git add crates/sr-assets assets docs/fidelity/asset-inventory.md
git commit -m "Recover the EXE-only assets from OMF LEDATA records"
```

---

### Task A6: PAM's animation — FLI delta frames and the `.ANI` scheme

PAM is Phase 4's part, so this is the first task whose consumer is not Phase 2. It is separate from
A7 because the two share nothing but their lateness.

**Files:**
- Create: `crates/sr-assets/src/anim.rs`, `crates/sr-assets/tests/anim.rs`
- Read: `upstream/SecondReality/PAM/VFLI.C` (a FLI player in the tree — the authority for the delta
  decode), `PAM/ANIM.C` (the `.ANI` record scheme), `PAM/PRAX4.FLI`, `PAM/OUT.ANI` + `OUT.IN0..IN4`

**Interfaces:**
- Consumes: `AssetError`, `Image`, `Format::{Fli, Ani}`.
- Produces: `Format::Fli` and `Format::Ani` through `load_frames`.

- [ ] **Step 1: Write the failing tests** — a FLI decodes to a frame count and a stable first frame;
  a delta frame that only touches part of the screen leaves the rest as the previous frame; a
  truncated chunk is refused. For `.ANI`, the test asserts the record scheme `ANIM.C` implements,
  including how `OUT.IN0..IN4` join `OUT.ANI`.

- [ ] **Step 2: Run to verify they fail.** `cargo test -p sr-assets anim`

- [ ] **Step 3: Implement `anim.rs`.** FLI: 128-byte header (`0xAF12` magic), then per-frame chunk
  records (`0xF1FA` frame type, `0x0004`/`0x000C` colour chunks, `0x0007`/`0x000B`/`0x000C` delta
  types) applied to a 320×200 chunky canvas. Follow VFLI.C's decode for the delta forms it actually
  uses; refuse the ones it does not.

- [ ] **Step 4: Run the tests to verify they pass, then commit**

```bash
git add crates/sr-assets
git commit -m "Decode PAM's FLI animation and its ANI record scheme"
```

---

### Task A7: Font strips

`ALKU/FONA.LBM`, `ALKU/FONA2.LBM`, `ENDSCRL/FONA.UH` (1500×30), `GRAB/BLOWFNT.1BM`: pictures that a
part slices into glyph cells. The loader work is therefore the `Font` view over a decoded `Image`,
plus the cell geometry — which is **not** to be guessed: read the part's own text routine
(`ENDSCRL/`, `ALKU/`, `CREDITS/`) for the cell size and the character order, and pin both.

**Files:**
- Create: `crates/sr-assets/src/font.rs`, `crates/sr-assets/tests/font.rs`

**Interfaces:**
- Consumes: `Image` from A2/A3.
- Produces:

```rust
pub struct Font { /* cells over a decoded Image */ }
impl Font {
    /// `cell` is (width, height) in pixels; `first` is the character the first
    /// cell holds. Both come from the part's own text routine, cited in the commit.
    pub fn from_image(image: Image, cell: (u16, u16), first: char) -> Result<Font, AssetError>;
    pub fn glyph(&self, ch: char) -> Option<&[u8]>;   // cell-sized, row-major
    pub fn cell(&self) -> (u16, u16);
}
```

- [ ] **Step 1: Find the cell geometry in `ENDSCRL`'s text routine and write it down** in the test's
  own comment, with the `file:line` it came from.
- [ ] **Step 2: Write the failing test** — 1500×30 with the cell size the source gives yields a whole
  number of cells, and a glyph's pixels equal the corresponding rectangle of the decoded image.
  A cell size that does not divide the strip is refused.
- [ ] **Step 3: Run it, implement `font.rs`, run again.** `cargo test -p sr-assets font`
- [ ] **Step 4: Commit**

```bash
git add crates/sr-assets
git commit -m "Add the font-strip view over decoded images"
```

---

### Task A8: `xtask assets verify`, contact sheets, and the human look

The phase's exit criterion is that every asset "decodes **and matches the source art**". The
cross-format tests in A2/A3 are the automated half; the other half is looking — this project's one
windowed bug was found by a person looking at output, and no test could have found it.

**Files:**
- Create: `xtask/src/assets.rs`, `xtask/src/sheet.rs`, `xtask/tests/assets.rs`
- Modify: `xtask/src/main.rs` (a new `assets verify` subcommand, and a `--sheet` flag on
  `reference capture`)
- Create: `xtask/tests/fixtures/` entries for the failure cases

**Interfaces:**
- Consumes: `sr_assets::{load, load_path, sniff, AssetError}` and A1's `assets/PROVENANCE.md`
  (parsed as text — the tables' shape is fixed by A1).
- Produces:
  - `cargo xtask assets verify [--tree] [--decode] [--sheet <DIR>]`, exit codes matching `compare`'s
    convention: **0** everything checked out, **1** a check failed, **2** the tool could not run
    (missing `assets/`, unreadable provenance).
  - `cargo xtask reference capture … --sheet <PATH>` — one PNG of the frames that capture just took.
    **Part B depends on this**: identifying a part's frames by looking at a sweep is how B2–B5 find
    their indices, and it must not be invented twice.
  - `sr_assets`-free helper in `sheet.rs`: `pub fn contact_sheet(tiles: &[Tile], columns: usize) ->
    (u32, u32, Vec<u8>)` where `Tile { width, height, rgb, label }`, so both callers share one
    layout and one PNG writer.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn a_tree_whose_hash_does_not_match_the_provenance_is_reported() { … }   // exit 1

#[test]
fn an_asset_provenance_lists_but_the_tree_lacks_is_reported() { … }      // exit 1

#[test]
fn every_asset_in_the_tree_decodes() { … }                              // exit 0, the real tree

#[test]
fn a_sheet_is_written_for_every_part_that_has_assets() { … }            // PNGs exist, non-empty
```

- [ ] **Step 2: Run to verify they fail.** `cargo test -p xtask assets`
- [ ] **Step 3: Implement `sheet.rs`, then `assets.rs`.** `sheet.rs` lays tiles out in a grid and
  writes one PNG with the `png` crate `compare`'s diff writer already uses. `--tree` re-hashes every
  file and compares against `PROVENANCE.md`; `--decode` decodes every asset and reports dimensions
  and palette size; `--sheet` writes one PNG per part with each asset as a tile. Then wire
  `--sheet <PATH>` into `reference capture` so a capture can be looked at.
- [ ] **Step 4: Run the tests and the real command**

```bash
cargo test -p xtask
cargo xtask assets verify --tree --decode --sheet out/assets
```

Expected: exit 0, and a sheet per part under `out/assets/`.

- [ ] **Step 5: Look at every sheet and write down what you saw**

Open each sheet. The question is not "did it decode" but "is this the picture the part shows": a
font strip looks like glyphs, a background looks like a scene, a palette strip looks like a palette.
Record the outcome per part in the commit message, and any asset that looks wrong as a finding in
`docs/fidelity/asset-inventory.md` — including the possibility that the format is right and the part
was already shipping a broken asset.

- [ ] **Step 6: Commit**

```bash
git add xtask
git commit -m "Add xtask assets verify, and record what the contact sheets showed"
```

---

## Part B — references for all parts

Phase 0 captured two frames of one still screen from a boot whose timing depended on how fast the
host answered the setup screen. D15 measured the consequence: **the demo is deterministic in
demo-relative time, not in boot time** — two runs put the same screen 43–45 frames apart, so a fixed
boot index lands at a demo position only as precisely as the boot repeats. That is good enough for a
title screen that holds still, and not good enough for a scroller. This part removes the jitter
first, then captures everything against it.

### Task B1: A boot that repeats (spike)

**Two candidate routes, both recorded in D14 and neither tested:**

1. **`[autoexec]` + `AUTOTYPE`.** DOSBox's own shell can inject keystrokes from the config
   (`AUTOTYPE left enter` with `-w`/`-p`), which removes the host round-trips from the boot
   entirely. D14:526-529 names this as "would remove the input round-trips".
2. **One `input/sequence`.** Send `SECOND 2\n`, then `KBD_left`, then `KBD_enter` as a *single*
   sequence with absolute `t` (ms on the sequence timeline) — the emulator's own timeline decides
   when each lands, so the host's response time stops mattering.

**Files:**
- Modify: `xtask/src/oracle/dosbox.rs` (whichever route wins), `xtask/src/reference.rs`
  (`CaptureOptions` gains the recipe), `docs/decisions.md` (a new D17 — the measured outcome,
  either way)

**Interfaces:**
- Produces: a boot that repeats, and the `CaptureOptions`/CLI surface that expresses it.
  `CaptureOptions` keeps `keys` as its surface; if the winning route needs more (a timed sequence, an
  `[autoexec]` line) that is one new optional field whose default preserves today's behaviour — the
  shape follows the measurement rather than being guessed here. `--keys "KBD_left KBD_enter"`
  (Phase 0's flag) must keep working or be replaced by a documented equivalent, because
  `reference/check_a/`'s manifest records it.

- [ ] **Step 1: Capture a *moving* screen twice with the current recipe, to see the jitter**

Pick a part with continuous motion (`SECOND 2`, then TECHNO's bars, or TUNNELI's tunnel) and capture
the same boot index in two runs, into two directories. Expected: **the frames differ** — that is the
problem this task exists to solve, and it is worth having the evidence before the fix. Record the
differing-pixel percentage via `cargo xtask compare`.

- [ ] **Step 2: Try route 1.** Add the `AUTOTYPE` boot to the oracle and capture the same index twice.
- [ ] **Step 3: If route 1 does not repeat, try route 2.** Capture the same index twice again.
- [ ] **Step 4: Compare all three pairs and record the numbers**

```bash
cargo xtask compare --reference out/boot-route1-a --rendered out/boot-route1-b --out out/boot-route1-diff
```

Expected for the winning route: every frame 0.0000 % differing, `verdict: identical`, exit 0.

- [ ] **Step 5: Write D17** — which route won, the measured numbers, the wall-clock cost of the boot,
  and whether the boot-relative indices in `reference/check_a/` still denote the same screens. If the
  recipe changes what frame 400 means, re-capture `check_a` and say so rather than leaving two
  index conventions in the tree.

- [ ] **Step 6: Commit**

```bash
git add xtask docs/decisions.md reference
git commit -m "Make the boot repeat so boot-relative indices mean one screen"
```

**If neither route repeats**, stop and report: the fallback is §8.4's — capture each part across
several runs and compare on structural similarity — and that decision changes B2–B5, so it is the
user's to make, not the implementer's.

---

### Task B2: Capture BEGLOGO, and start the index table

BEGLOGO is Phase 2's exit criterion, so it is the reference that matters first, and it is the proof
that the recipe from B1 works end to end on a real part.

**Files:**
- Create: `docs/fidelity/reference-indices.md`, `reference/parts/beglogo/`
- Modify: `docs/plans/01-assets.md` only if a step proves wrong (record it, do not silently diverge)

**Interfaces:**
- Consumes: B1's repeatable boot; `xtask reference capture`'s existing flags.
- Produces: the index-table format every later capture task appends to. One row per part: part name,
  the argument that reaches it (`SECOND 2`), the boot frame indices captured, the geometry of each,
  what is on screen at each, `sha256` of each frame file, and the capture command as run.

- [ ] **Step 1: Confirm the mixed-geometry refusal is pinned**

D15 says a capture refuses a frame list that changes geometry. Before relying on it, run
`cargo test -p xtask` and check that a test actually pins it; if none does, add one to
`xtask/tests/` with a mixed-geometry frame list and the expected refusal. It is load-bearing for
every capture in this part and for Phase 2's BEGLOGO comparison.

- [ ] **Step 2: Find BEGLOGO's frames by sweeping, not by guessing**

Capture a coarse sweep — a dozen or so indices spanning a few seconds from a `SECOND 2` boot — with
`--sheet out/beglogo-sweep.png` (A8) rendering them as a contact sheet. Identify from the sheet which
indices hold the logo fading in, the logo held, and the first frames of GLENZ. This is the discovery
procedure every later capture task repeats; write it in `reference-indices.md` so the next person
does not reinvent it.

- [ ] **Step 3: Capture BEGLOGO's keyframes**

Three to four per part, per §8.2, at structurally significant moments — for BEGLOGO: early fade,
mid fade, held logo. One geometry per capture directory (D15).

```bash
cargo xtask reference capture --arg 2 --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames <i,j,k> --out reference/parts/beglogo --sheet out/beglogo.png
```

- [ ] **Step 4: Re-capture and prove the reference is reproducible**

Run the same command into a second directory and `cargo xtask compare` the two. Expected: exit 0,
`verdict: identical`. **This is the actual deliverable of Part B** — a reference that cannot be
reproduced cannot be a reference. Record the exit code and the percentage in the index table.

- [ ] **Step 5: Write the index table** with BEGLOGO's row filled in and the method section above it.

- [ ] **Step 6: Commit**

```bash
git add docs/fidelity/reference-indices.md reference/parts
git commit -m "Capture BEGLOGO's reference frames and record how the indices are found"
```

---

### Task B3: Blocks 1 and 2 — ALKU, U2A, PAM, GLENZ, TUNNELI, TECHNO, PANICEND

**Files:**
- Modify: `docs/fidelity/reference-indices.md`
- Create: `reference/parts/<name>/` for each

**Interfaces:**
- Consumes: B2's method and table format.
- Produces: one row and one directory per part, each with its re-capture check.

- [ ] **Step 1:** For each part in turn: sweep to find its indices (a `SECOND 1` or `SECOND 2` boot,
  whichever reaches it with the fewest wasted frames), capture three to four keyframes, re-capture,
  and compare.
- [ ] **Step 2:** Where a part's frames span two geometries — a 320×400 Mode X screen and a 320×200
  chain-4 one — capture them as two captures with a row each. `capture` refuses a mixed list (D15);
  do not work around it.
- [ ] **Step 3:** Append every row to `reference-indices.md`, including the sha256 of each frame.
- [ ] **Step 4:** Commit after each part or each small group, not once at the end — a capture run is
  minutes of emulator time and losing one to a single failed commit wastes it.

```bash
git add docs/fidelity/reference-indices.md reference/parts
git commit -m "Capture references for <parts>"
```

---

### Task B4: Blocks 3 and 4 — MNTSCRL, LNS&ZOOM, PLZPART, MINVBALL, RAYSCRL, 3DSINFLD, JPLOGO, U2E

Same procedure as B3, from a `SECOND 3` and a `SECOND 4` boot. Two things are different here and both
are worth a note in the table:

- **PLZPART is the part the split convention was written for** (D16, `Split`'s doc comment). Its
  capture is the first evidence that the port's `Split::scanline` maps to the emulator's line-compare
  register as documented, so where a captured frame puts the split is worth recording in the row.
- **U2E follows a music restart** (`MUSIC0` at order 18, D13). Its row records which boot argument
  reaches it, since on a full run it is preceded by parts that a `SECOND 4` boot skips.

**Files, interfaces, steps:** as B3.

---

### Task B5: Block 5 and the hidden part — ENDLOGO, CRED, ENDSCRL, DDSTARS

A `SECOND 5` boot, plus `SECOND U` for the hidden part, whose mask is `0x80` and which runs DDSTARS
**alone** (D13, part-sequence §5). ENDSCRL is skipped when looping is on, so its capture pins the
non-looping case and says so.

**Files, interfaces, steps:** as B3.

**The vault is not in this task, or this phase.** GRID, HARD and TWIST are Phase 8's parts and are not
in `SECOND.EXE`'s pack manifest at all — each has its own EXE in the tree (`GRID/DOLINE.EXE`,
`HARD/B.EXE`, `TWIST/DOL.EXE`), so they are not reachable through the `SECOND n` command line this
part uses. Their references need a different boot and belong to the phase that ports them. "All
parts" in this phase's exit criterion means the 20 numbered parts plus the hidden one.

---

### Task B6: Close the phase's reference evidence, and the non-determinism policy

**Files:**
- Modify: `PROGRESS.md`, `docs/fidelity/reference-indices.md`, `docs/decisions.md` (D18 if the policy
  needs recording)

- [ ] **Step 1: Re-capture one moving part from each block, a day's work apart or more**

The B2–B5 checks prove reproducibility within a session. This one probes the weaker claim — that a
capture still means the same thing later. Any part that fails gets the §8.4 treatment.

- [ ] **Step 2: Write the policy down** in `reference-indices.md`: what a capture index means, that
  it is only valid under the B1 recipe with the pinned cycles, and that a part which cannot be
  reproduced gets captured across runs and compared structurally rather than pixel-exactly.

- [ ] **Step 3: State the phase's evidence on the board.** `PROGRESS.md` gets the asset count, the
  verify exit code, the part count captured and the reproducibility result per part. Anything that
  could not be confirmed is written there rather than left implied.

- [ ] **Step 4: Commit, and tag**

```bash
git add PROGRESS.md docs
git commit -m "Close phase 1: assets verified, references captured for the sequence"
git tag phase-1
```

---

## Task order and why

`A1 → A2 → A3 → A4` first: Phase 2 cannot start without the Origin PIC, IFF and RIX3 decoders, and
A1 is what tells every later task which files to test against. `A5` follows because the inventory it
needs exists by then. `A6`/`A7` serve Phases 4 and 2 respectively and are the deferrable tail.
`A8` last among the asset tasks, because it verifies what A2–A7 built.

Part B is independent of Part A and can run in parallel or after it — nothing in B imports
`sr-assets`. If the phase runs short of time, B is what should be cut, because Phase 2's own part
(BEGLOGO) has its reference after B2 and nothing else in Phase 2 needs one.

## Phase completion

Exit criteria from spec §10, with the evidence each task leaves:

| Criterion | Evidence |
|---|---|
| Every needed asset decodes | `cargo xtask assets verify --decode` exit 0 |
| …and matches the source art | cross-format tests (A2/A3) plus the contact sheets and the recorded look (A8) |
| Provenance file complete | `cargo xtask assets verify --tree` exit 0 against `assets/PROVENANCE.md` |
| References captured for all parts | `reference/parts/<name>/` for all 20 numbered parts plus the hidden one, one row each in `docs/fidelity/reference-indices.md`, each with a re-capture comparison at exit 0 |
