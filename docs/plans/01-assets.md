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
- **6-bit palette internally.** But *how* a loader gets there depends on what the source stores, and
  the two cases are not the same rule:
  - **Origin palettes are already 6-bit** and are copied verbatim. Running them through
    `dac_expand`'s inverse is not the identity — `six_bit(32) == 8` — and would darken every Origin
    picture. A byte above 63 is refused.
  - **An IFF `CMAP` is 8-bit and converts by integer `/4`**, which is what every original converter
    and viewer does (`LBM2U.C:170`, `LBM2P.C:195`, `LBM16.C:194`, `VIEWLBM.C:157`, `VPL.C:97`) and
    what the shipped art holds. **Not** `six_bit`: measured against the converted twins, `/4`
    disagrees on 0 of 768 bytes while `six_bit` disagrees on 457 of 768 (`LENS/LENSPIC.LBM` vs
    `LENS/LENS.EXB`) and 1 of 768 (`BEG/SRTITLE.LBM` vs `BEG/SRTITLE.UP`). `/4` is also what makes
    an image and its converted twin show the same screen, which is the property the port needs.
  - `six_bit` remains the exact inverse of `dac_expand`, with a test proving the round trip over all
    64 values — it is the 6→8 *display* rule inverted, and is not the 8→6 conversion rule.
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
3. **An image too large for the surface the engine can allocate.** The rule is an **area** one —
   `w * h > MAX_WIDTH * MAX_HEIGHT`, i.e. more than 256,000 pixels — not a per-dimension one.
   A per-dimension rule would be wrong: `ALKU/FONA.UH` is **1500 wide** and is a required asset,
   so refusing anything wider than 640 would reject a font strip the demo ships. A 1024×400 header
   (409,600 px) must still be refused with a legible message, not clipped or wrapped. Pinned in A2.
4. **An 8-bit palette from an IFF CMAP.** The conversion is the original's own — integer `/4` — and
   the tempting alternative is worse than it looks: `dac_expand`'s inverse is the mathematically
   tidier 8→6 mapping and is **wrong here**, disagreeing with the shipped twins on 457 of 768 bytes.
   A round-trip test over 256 values passes either way; only comparison against a converted twin
   catches it. Pinned in A3 against `LENSPIC.LBM`/`LENS.EXB`.
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

> **Corrected after pre-flight research.** This section originally claimed *one* format — magic
> `0xFCFC`/`0xFCFD`, 16-byte header, always-768-byte palette — and pointed at `GRAB/SP.C` as the
> authority. That was wrong: `SP.C` is the *animation* viewer and its `0xFCFC` is a mouse-interrupt
> argument, not a magic. The authority is the **writers** (`GRAB/LBM2U.C`, `LBM16.C`, `LBM2P.C`,
> and `GRAB/GLVP.C`'s `saveunp`/`saveunpheader`/`saveunpheaderpal`) plus the runtime reader
> `ENDPIC/READP.C`. Measurements, decoder pseudocode and file-by-file verification are in
> `.superpowers/sdd/01-assets/a2-format-research-report.md`; the implementer must read it.
>
> Three consequences worth stating up front, because each one silently corrupts colour if missed:
> **the palette is `colors*3` bytes, not 768**; **it is already 6-bit and must not be re-quantised**
> (see the Global Constraints amendment below); and **the extension does not identify the format** —
> `ENDPIC/PIC.UH` is RLE, `COMAN/COMBG.UH` is `UH`, `GLENZ/FC.UH` is `Uh`.

The container behind `.UH`, `.U`, `.UP`, `.P`, `.16` and `.X` is **one family of five variants plus
a headerless case**, all sharing a 16-byte little-endian header:

| variant | bytes 0-1 | palette | pixels |
|---|---|---|---|
| `FCFC` raw (`LBM2U.C`) | `FC FC` | `colors*3` at 16, 6-bit | raw 8-bit at `para*16` |
| `FCFC` bit-planes (`LBM16.C`) | `FC FC` | 48 bytes at 16 | `bpls` planes at `para*16` |
| `FCFD` RLE (`LBM2P.C`) | `FD FC` | `colors*3` at 16, 6-bit | row-prefixed RLE at `para*16` |
| `UH` (`GLVP.C` `saveunpheader`) | `55 48` | none | raw 8-bit at **16** |
| `Uh` (`GLVP.C` `saveunpheaderpal`) | `55 68` | 768 at 16, 6-bit | raw 8-bit at **784** |
| bare (`GLVP.C` `saveunp`) | — | none | raw 8-bit at 0, geometry from the caller |

Only the first three carry a `para` field, and only there is the offset computed —
`(16 + colors*3 + 15) / 16 * 16`. The `UH` and `Uh` writers hardcode 16 and 49 paragraphs. A bare
file has no header at all, so `sniff` returns `None` for it and it is loaded by construction.

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

/// `Origin` covers all five magic-carrying variants; `load` dispatches on the magic
/// internally, so callers never need to tell raw from RLE from bit-planes.
/// `OriginBare` has no header and therefore no magic: `sniff` never returns it and
/// `load` refuses it, because it cannot know the geometry. `raw_surface` is its door.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format { Origin, OriginBare, Iff, Rix3, Pal, Fli, Ani }

#[derive(Debug)]
pub enum AssetError {
    UnknownFormat,
    Truncated { format: Format, needed: usize, had: usize },
    Unsupported { format: Format, what: String },
    /// An **area** rule: `width * height > MAX_WIDTH * MAX_HEIGHT`. Not a
    /// per-dimension one — `ALKU/FONA.UH` is 1500 wide and must load.
    TooLarge { width: u32, height: u32 },
    Palette { what: String },
    /// The file could not be read at all. `load_path` reports this rather than
    /// inventing a `Truncated` for a file it never opened.
    Io { path: std::path::PathBuf, what: String },
}
impl std::fmt::Display for AssetError { /* one line per variant */ }
impl std::error::Error for AssetError {}

/// By magic. `None` when nothing matches.
pub fn sniff(bytes: &[u8]) -> Option<Format>;
pub fn load(bytes: &[u8], format: Format) -> Result<Image, AssetError>;
pub fn load_path(path: &std::path::Path) -> Result<Image, AssetError>;   // sniffs, errors if None
pub fn load_frames(bytes: &[u8], format: Format) -> Result<Vec<Image>, AssetError>;

/// A headerless pixel dump — `PANIC/MONSTER.U`, `LENS/LENS.U`, `GRAB/BLOWFNT.1BM` —
/// whose geometry only the caller knows. Requires `bytes.len() == width * height`.
pub fn raw_surface(bytes: &[u8], width: u16, height: u16) -> Result<Image, AssetError>;

/// The exact inverse of `sr_engine::dac_expand` over 0..=63. For loaders that read
/// **8-bit** palette data — that is the IFF `CMAP` (A3) and nothing else. The Origin
/// family stores 6-bit values already and must be copied verbatim.
pub fn six_bit(v: u8) -> u8;
```

**Palette precision is not uniform, and the Global Constraints' rule needs one amendment.**
"Every palette a loader produces goes through `dac_expand`'s inverse" is true of 8-bit sources and
**false of the Origin family**, which stores 6-bit values directly (`LBM2U.C:170` writes
`getc(f1)/4`, already 6-bit). Running `six_bit` over an already-6-bit value is not the identity —
`six_bit(32) == 8` — so applying it there would darken every Origin picture. Origin palettes are
copied verbatim and a byte above 63 is refused as out of contract; the round-trip test stays, and
pins `six_bit` against `dac_expand` rather than against a file.

- [ ] **Step 1: Write the failing tests** in `crates/sr-assets/tests/origin.rs`

Real-file agreement is the phase's own test of "matches the source art": the tree hands us files
whose headers were read off the bytes and confirmed against the writers, so these tests pin the
decoder rather than anyone's transcription. (The cross-format equality test — `SRTITLE.UP` against
`SRTITLE.LBM` — now lives in A3, because it needs A3's IFF decoder to run.)

```rust
#[test]
fn a_thirty_pixel_tall_font_strip_reads_as_one() {
    // ALKU/FONA.UH, read off the file: magic FC FC, 1500 x 30, colours 256,
    // para 0x31 (= 784), so 16 + 768 palette bytes then 45,000 raw pixels.
    let img = load_path(Path::new("../../assets/ALKU/FONA.UH")).unwrap();
    assert_eq!((img.width(), img.height()), (1500, 30));
    assert_eq!(img.pixels().len(), 45_000);
}

/// The point of this family, in one test: the extension says nothing. `END/PIC.UH`
/// is RLE, `GLENZ/FC.UH` is `Uh` with a palette, and `COMAN/COMBG.X` is `UH`
/// without one. All three are `.UH`/`.X` and all three are different formats.
///
/// The plan originally named `ENDPIC/PIC.UH` as the `Uh` example. It is not in
/// the curated tree and has no consumer in the original, so `GLENZ/FC.UH` — the
/// same variant at the same 320x200 — stands in.
#[test]
fn the_extension_never_decides_the_format() {
    let rle = load_path(Path::new("../../assets/END/PIC.UH")).unwrap();
    assert_eq!((rle.width(), rle.height()), (320, 400));

    let uh_pal = load_path(Path::new("../../assets/GLENZ/FC.UH")).unwrap();
    assert_eq!((uh_pal.width(), uh_pal.height()), (320, 200));

    let uh = load_path(Path::new("../../assets/COMAN/COMBG.X")).unwrap();
    assert_eq!((uh.width(), uh.height()), (320, 200));

    assert_eq!(sniff(&std::fs::read("../../assets/BEG/SRTITLE.LBM").unwrap()), Some(Format::Iff));
}

/// The `Uh` variant's palette is the one place a 768-byte palette is guaranteed,
/// and it is already 6-bit — so this also pins that no re-quantising happens.
#[test]
fn a_uh_file_carries_its_own_six_bit_palette() {
    let img = load_path(Path::new("../../assets/GLENZ/FC.UH")).unwrap();
    for i in 0..=255u8 {
        assert!(img.palette.entry(i).iter().all(|&c| c <= 63), "entry {i} is not 6-bit");
    }
    assert_ne!(img.palette.entry(0), img.palette.entry(255), "palette was not read");
}

/// `LBM16.C` writes a fixed 16-colour palette and bit-planes, and `TEXTS.16` was
/// made with it at two planes (`DOTEXTS.BAT:1`).
#[test]
fn a_bit_plane_file_decodes_to_the_geometry_in_its_header() {
    let img = load_path(Path::new("../../assets/DDSTARS/TEXTS.16")).unwrap();
    assert_eq!((img.width(), img.height()), (320, 200));
    assert_eq!(img.pixels().len(), 64_000);
}

/// A bare dump has no header, so `sniff` cannot see it and the geometry is the
/// caller's to supply. `PANIC/MONSTER.U` is exactly 320 x 200 bytes of pixels.
#[test]
fn a_headerless_dump_needs_its_geometry_from_the_caller() {
    let raw = std::fs::read("../../assets/PANIC/MONSTER.U").unwrap();
    assert_eq!(sniff(&raw), None, "a bare dump has no magic to sniff");
    let img = raw_surface(&raw, 320, 200).unwrap();
    assert_eq!((img.width(), img.height()), (320, 200));
    assert!(matches!(raw_surface(&raw, 320, 199), Err(AssetError::Truncated { .. })));
}

/// Every Origin file in the curated tree decodes, and what it decodes to fills the
/// geometry its header claimed. The decoder refuses any payload that is neither
/// `w*h` nor a whole number of bit-planes, so a misread header field cannot pass.
/// `seen` guards the other failure mode: a loop that matches nothing also passes.
#[test]
fn every_origin_asset_decodes_and_fills_the_geometry_its_header_claims() {
    let mut seen = 0;
    for path in origin_assets_in("../../assets") {
        let raw = std::fs::read(&path).unwrap();
        if sniff(&raw) != Some(Format::Origin) {
            continue; // bare dumps are the caller's geometry, tested above
        }
        let img = load(&raw, Format::Origin)
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert_eq!(
            img.pixels().len(),
            img.width() as usize * img.height() as usize,
            "{}: decoded pixels vs header geometry",
            path.display()
        );
        seen += 1;
    }
    // The curated tree holds 16 Origin-family files: 15 carry a magic and one is
    // bare. Fourteen decode; the fifteenth is the truncated `COMAN/COMBG.UH`,
    // asserted separately below. The guard is what stops a loop that matches
    // nothing from passing — the plan's original `>= 20` came from the
    // whole-upstream tally of 26 and was never reachable against the curated tree.
    assert!(seen >= 14, "expected every decodable Origin asset, saw {seen}");
    assert_eq!(refused, 1, "only the truncated asset should be refused");
}

#[test]
fn a_truncated_pixel_run_is_refused_not_panicked_on() {
    // Half of an RLE file: the row length word at the cut promises more bytes than
    // remain, which is a truncation the decoder must detect, not a panic.
    let mut bytes = std::fs::read("../../assets/BEG/SRTITLE.UP").unwrap();
    bytes.truncate(bytes.len() / 2);
    assert!(matches!(load(&bytes, Format::Origin), Err(AssetError::Truncated { .. })));
}

/// A file that is genuinely short in the tree. `COMAN/COMBG.UH` is the first
/// 32,768 bytes of `COMAN/COMBG.X`; its `UH` header claims 320 x 200 and its
/// payload is 32,752 of the 64,000 pixels. It is a corrupt asset, and the decoder
/// refusing it is the correct outcome — see the finding in `asset-inventory.md`.
#[test]
fn the_truncated_asset_in_the_tree_is_refused() {
    let bytes = std::fs::read("../../assets/COMAN/COMBG.UH").unwrap();
    assert!(matches!(load(&bytes, Format::Origin), Err(AssetError::Truncated { .. })));
}

#[test]
fn an_oversized_header_is_refused_rather_than_clipped() {
    let mut bytes = std::fs::read("../../assets/BEG/SRTITLE.UP").unwrap();
    bytes[2..4].copy_from_slice(&1024u16.to_le_bytes());
    assert!(matches!(load(&bytes, Format::Origin), Err(AssetError::TooLarge { .. })));
}

/// `START/HZPIC.P` has 153 of its 400 rows decoding wider than the 640 its header
/// claims, up to 761 pixels, and a shipped file relying on it means this is not a
/// malformed file. The original writes every pixel into a 640-byte row buffer and
/// overflows it; the port takes the first `w` and drops the rest, which is what the
/// visible output shows. That substitution is recorded in APPROXIMATIONS.md.
#[test]
fn a_run_row_wider_than_the_header_is_clipped_not_refused() {
    let img = load_path(Path::new("../../assets/START/HZPIC.P")).unwrap();
    assert_eq!((img.width(), img.height()), (640, 400));
    assert_eq!(img.pixels().len(), 640 * 400);
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

Then implement `crates/sr-assets/src/origin.rs`. **Read
`.superpowers/sdd/01-assets/a2-format-research-report.md` §7 first** — it is the decoder, written
out precisely enough to implement without re-reading the C, and it is the authority for this task.
Its §1 variant table, §5 palette encoding and §8 file-by-file tally are all load-bearing.

The essentials, so the shape is clear before you open the report:

```
dispatch on B[0..2], little-endian, and on nothing else:
  FD FC  RLE        (0xFCFD as stored — the discriminator is B[0], not B[1])
  FC FC  raw or bit-planes
  55 48  "UH"       raw, no palette, pixels at 16
  55 68  "Uh"       raw, 768-byte palette at 16, pixels at 784
  other  bare       no header at all

header, 16 bytes: magic, then seven u16 words w1..w7
  FCFC/FCFD:  w=w1, h=w2, colours=w3, para=w4; palette at 16, colours*3 bytes;
              pixels at para*16, which the writer computes as ((16+colours*3+15)/16)*16
  UH:         w=w2, h=w3; pixels at 16; w1 is 1 in every file
  Uh:         w=w2, h=w3; palette 768 bytes at 16; pixels at w1*16 (w1 is 49)
```

Three things the original art does that a naive decoder gets wrong, all of them load-bearing:

- **The palette is 6-bit already.** Do not run it through `six_bit`. A byte above 63 is out of
  contract and is refused rather than clamped.
- **The RLE rows may decode wider than `w`.** `START/HZPIC.P` does this on 153 of 400 rows. Take
  the first `w` pixels; do not reject the file. Record the substitution in
  `docs/fidelity/APPROXIMATIONS.md` in this task's commit.
- **A run opcode of `0x80` means a run of zero**, and the two readers in the tree disagree about it
  (`READP.C`'s assembly copies 256 pixels, its C copy copies 0). The writer never emits it. Refuse
  it rather than picking a reading the original does not have.

`.X` is not a format: it is `GLVP.C`'s `saveunpheader` output, which is the `UH` variant, and
`COMAN/COMBG.X` is the tree's only one. `.U` likewise holds both bare dumps and `FCFC` files.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p sr-assets`
Expected: PASS — all eleven, including the two real-file truncation refusals and the `HZPIC.P`
clip.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml crates/sr-assets docs/fidelity/APPROXIMATIONS.md
git commit -m "Add sr-assets and decode the Origin PIC container's five variants"
```

The commit message cites `GRAB/LBM2U.C:57-67` (the `colors*3` palette and the offset formula),
`GRAB/GLVP.C`'s `saveunpheader`/`saveunpheaderpal` (the `UH`/`Uh` magics) and `ENDPIC/READP.C`
(the runtime RLE reader), as the Global Constraints require.

---

### Task A3: IFF — ILBM and PBM

The tree carries **both** IFF flavours and the tally is not obvious from any one file. Measured
over all 81 `.lbm`/`.1bm` files: **38 are `FORM .... ILBM`** (planar, `nPlanes` bitplanes) and
**42 are `FORM .... PBM `** (one byte per pixel). The plan originally said 46 `PBM `; the corrected
count is a direct measurement. §4.4's "stencil/EHB, multiple bitplane counts" applies to the ILBM
half only; check whether either feature is actually used before supporting it, and record the
answer.

The 81st file is not IFF at all: `GRAB/BLOWFNT.1BM` is 64,000 bytes of bare 320×200 pixels with a
twelve-byte all-zero head. It does not belong to this decoder — A2's `raw_surface` is its door, and
A7 decides whether it is a font strip.

**Files:**
- Create: `crates/sr-assets/src/iff.rs`, `crates/sr-assets/tests/iff.rs`
- Read: `upstream/SecondReality/GRAB/LOADLBM.C`, `GRAB/VIEWLBM.C`, `GRAB/DEFLBM.INC`

**Interfaces:**
- Consumes: `AssetError`, `Image`, `Format::Iff`, `six_bit`, `load`, `load_path` (A2). Also A2's
  Origin decoder, for the cross-format equality test moved here from A2.
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

/// **Moved here from A2**, because it needs both decoders: it was A2's strongest
/// evidence that the Origin decoder matches the source art, and it cannot run until
/// this task lands. Verified during pre-flight: 0 differing pixels of 128,000, and
/// the `UP` palette is byte-equal to the LBM's `CMAP` divided by 4.
#[test]
fn the_packed_title_and_the_iff_title_are_the_same_picture() {
    let up = load_path(Path::new("../../assets/BEG/SRTITLE.UP")).unwrap();
    let lbm = load_path(Path::new("../../assets/BEG/SRTITLE.LBM")).unwrap();
    assert_eq!((up.width(), up.height()), (lbm.width(), lbm.height()));
    assert_eq!(up.pixels(), lbm.pixels());
}

/// A2's tree-wide property does not transfer to a compressed container: an IFF
/// `BODY` is ByteRun1, so there is no payload-equality to assert. What does hold,
/// and catches the same class of misread header field, is that every IFF file
/// decodes and that what it decodes to fills its own `BMHD`'s `w`/`h`.
#[test]
fn every_iff_asset_decodes_and_fills_the_geometry_its_bmhd_claims() {
    let mut seen = 0;
    for path in iff_assets_in("../../assets") {
        let raw = std::fs::read(&path).unwrap();
        let img = load(&raw, Format::Iff).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert_eq!(
            img.pixels().len(),
            img.width() as usize * img.height() as usize,
            "{}: decoded pixels vs BMHD geometry",
            path.display()
        );
        seen += 1;
    }
    assert!(seen >= 20, "expected both IFF flavours, saw {seen}");
}

/// **The test that pins the CMAP rule, and the only one with teeth.** This was
/// originally written as a `six_bit` round trip, which is the wrong rule and
/// passes under either. An `rstest` over two converted twin pairs, asserting this
/// decoder's palette equals the twin's — and asserting each case's *discrimination
/// count*, so a case that stopped discriminating fails rather than passing weakly:
///   LENS/LENSPIC.LBM vs LENS/LENS.EXB   457 of 768 bytes at stake
///   BEG/SRTITLE.LBM  vs BEG/SRTITLE.UP    1 of 768
#[rstest]
#[case("../../assets/LENS/LENSPIC.LBM", "../../assets/LENS/LENS.EXB", 457)]
#[case("../../assets/BEG/SRTITLE.LBM", "../../assets/BEG/SRTITLE.UP", 1)]
fn the_palette_is_the_originals_own_truncation_of_the_eight_bit_cmap(
    #[case] lbm: &str, #[case] twin: &str, #[case] at_stake: usize,
) { /* decode both, assert equal, assert the count */ }

/// `six_bit` is still the exact inverse of the engine's widening — it is the 6→8
/// *display* rule inverted. It is **not** the 8→6 conversion rule, and this test
/// governs `six_bit` alone.
#[test]
fn six_bit_is_the_inverse_of_dac_expand_over_all_256_values() {
    for c in 0u8..=63 { assert_eq!(six_bit(dac_expand(c)), c, "6-bit value {c}"); }
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
write down its offsets. The header is **10 bytes** — `"RIX3"`, `u16 width`, `u16 height`,
`u16 0x00AF` — then **768 palette bytes at offset 10**, then pixels at 778. Verified by file
arithmetic rather than assumed: `WATER/BKG.CLX` is 64,778 = 10 + 768 + 64,000 and `FOREST/O2.SCI`
is 21,258 = 10 + 768 + 20,480, and both land exactly.

The plan originally described a `u16 0x0000` field between `0x00AF` and the palette, which cannot
be right — those fields sum to 12, not 10 — and the bytes at offset 10 are the first palette entry
(`00 00 00`). `0x00AF` itself is still unexplained by the files alone, and the part's own read is
what decides it; record what you find either way. Same for `.PAL`: exactly 768 bytes, 6-bit, in
`PANIC/MONSTER.PAL`, `PAM/PAL.PAL`, `START/HZPIC.PAL`.

Note that `FOREST/O2.SCI` is a verified duplicate of `MAIN/DATA/O2.SCI`, and A1's curated tree keeps
only the `FOREST/` copy — which is the path the tests here name.

- [ ] **Step 2: Write the failing tests**

```rust
#[rstest]
#[case("../../assets/WATER/BKG.CLX", 320, 200)]  // header read off the file
#[case("../../assets/FOREST/O2.SCI", 640, 32)]    // header read off the file
fn rix3_decodes_a_screen(#[case] path: &str, #[case] w: u16, #[case] h: u16) {
    let img = load_path(Path::new(path)).unwrap();
    assert_eq!((img.width(), img.height()), (w, h));
}

/// The same property A2 and A3 use, with one measured exception. `WATER/MIEKKA.SCI`
/// declares 400x250 and carries 14,000 payload bytes — 35 whole rows, not 250 — and
/// it is a deliberate crop, not a corrupt file: its payload is the first 14,000
/// bytes of `WATER/SWORD.SCI` with that file's palette, `WATER/MAKEOB.BAT:2` links
/// it, and `MIEKKA.OBJ`'s LEDATA payload is this file byte for byte. The original's
/// own read is bounded the same way (`WATER/DEMO.PAS:109` reads only 400*34 of it).
/// So the decoder yields `width x rows-the-payload-carries` and neither pads nor
/// refuses: padding would invent pixels the demo never shows, and refusing would
/// reject a shipped asset the part links.
///
/// The test therefore asserts the property for the three files that hold it and
/// pins the exception by name, so a fifth file that quietly stops filling its
/// header cannot hide inside the exception.
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

> **Corrected after A1's inventory.** This task was planned around a claim — "eleven blobs have no
> standalone file of the same size" — that turned out to rest on a bad signal. A blob is *larger*
> than its payload because of OMF record overhead, so comparing file sizes said nothing. A1 walked
> every blob and reconstructed its payload: **eighteen of them reproduce a standalone file byte for
> byte** (`cmp`-identical), so those assets were never EXE-only and there is nothing to recover.
> A1's `docs/fidelity/asset-inventory.md` §D is the measurement and the authority for this task.
>
> The task still earns its place, with a different job: **prove the tree is complete** rather than
> filling gaps. `extract_ledata` is how that proof is made, and the four blobs that genuinely have
> no twin are the ones worth a recorded answer.

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
/// The framing test, and the one that matters. A blob whose payload has a
/// standalone twin must reproduce it **byte for byte** — equality, not a size
/// check, because treating the offset pair as a u16 gives the right length and
/// the wrong bytes. Every one of the eighteen twins in the inventory's §D is a
/// candidate; `GLENZ/_FC.OBK` was measured at 64,784 bytes.
#[test]
fn a_blob_reproduces_its_standalone_twin_byte_for_byte() {
    let blob = std::fs::read("../../upstream/SecondReality/GLENZ/_FC.OBK").unwrap();
    let twin = std::fs::read("../../upstream/SecondReality/GLENZ/FC.UH").unwrap();
    assert_eq!(extract_ledata(&blob).unwrap(), twin);
}

/// A blob-only payload still yields something decodable: `TECHNO/_PIC1.OBK` holds
/// an Origin RLE picture, 320x400 by its header. (It is also *not linked* — the
/// `_pic1.obk` line in `TECHNO/MAKEFILE:7` is commented out — so it is build
/// residue rather than an asset the shipped EXE carries. Recovering it proves the
/// extractor works; recording that it is unlinked is the honest part.)
#[test]
fn a_blob_only_that_holds_a_picture_yields_one_that_decodes() {
    let bytes = std::fs::read("../../upstream/SecondReality/TECHNO/_PIC1.OBK").unwrap();
    let payload = extract_ledata(&bytes).unwrap();
    let img = load(&payload, Format::Origin).unwrap();
    assert_eq!((img.width(), img.height()), (320, 400));
}

#[test]
fn a_blob_with_no_ledata_is_refused() { … }

#[test]
fn a_record_that_runs_past_the_end_of_the_blob_is_refused() {
    // The first byte is the record type, the next two the length: a length that
    // overruns must be an error, not a panic.
    assert!(matches!(extract_ledata(&[0xA0, 0xFF, 0xFF, 0x00]), Err(AssetError::Truncated { .. })));
}
```

- [ ] **Step 2: Run to verify it fails.** `cargo test -p sr-assets omf`

- [ ] **Step 3: Implement `omf.rs`**

OMF records are a type byte followed by a `u16` little-endian length. **`LEDATA` is `0xA0`**, not
`0xA1` — both are legal OMF spellings and only `0xA0` occurs in this tree. Its payload is
`u16 segment index`, then **one leading offset byte**, then up to 1,024 bytes of data, then **the
same offset byte again**. Strip both offset bytes and concatenate the data in record order.

That framing is not a guess and the distinction is not cosmetic: **the trailing byte is what makes
the reconstruction byte-exact.** Keeping it drifts the output by one byte per record — `GLENZ/_FC.OBK`
comes out 64,912 bytes instead of 64,784 — and treating the pair as a `u16` offset instead yields
the right *length* with the wrong *bytes*, which is the failure that would survive a size check.
Both were measured; see the inventory's §D.

- [ ] **Step 4: Recover the assets and extend the provenance**

The measurement A1 already made says **nothing needs recovering**. Turn that from a claim into a
checked result, and record it as the task's evidence:

1. **Prove completeness.** For every blob in the inventory's §D that has a standalone twin, assert
   in a test that `extract_ledata(blob) == std::fs::read(twin)`. **Twenty-five of the twenty-nine
   data blobs** reconstruct a standalone file byte-exactly; the other four are the blob-only
   payloads step 2 answers. That is the phase's evidence for "provenance file complete" from the
   EXE side: nothing the parts link exists only in a build artifact.
2. **Answer the four blob-only payloads by name.** `TECHNO/_CIRCLE.OBK` (24,000) and
   `_CIRCLE2.OBK` (8,000) are linked but carry no image header and are not picture sizes;
   `_PIC1.OBK` and `_PIC2.OBK` hold pictures but are **not linked** — `TECHNO/MAKEFILE:7`'s
   `_pic1.obk _pic2.obk` line is commented out. Say what each is and whether the port needs it.
   Nothing is recovered speculatively.
3. **Confirm against the shipped EXE.** `MAIN/PACKFINA.INC`'s 79 entries are what `SECOND.EXE`
   actually carries. Check that every entry is either a part executable or a file already in
   `assets/`, so "no asset is EXE-only" is a checked statement rather than an inference from the
   blobs.

`PROVENANCE.md` gains its second table only if step 1–3 finds something to recover. If they do not,
the tables stay as they are and the finding is recorded in `asset-inventory.md` and in this task's
commit message — **a task that finds nothing missing has succeeded, not failed,** and saying so
plainly is the point.

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

`ALKU/FONA.UH`, `ENDSCRL/FONA.UH` (both 1500×30 by header, and **not** the same strip — same size,
different pixels and a different palette), `ALKU/FONA.LBM`, `ALKU/FONA2.LBM`: pictures that a part
slices into glyph cells. The loader work is therefore the `Font` view over a decoded `Image`, plus
the cell geometry — which is **not** to be guessed: read the part's own text routine (`ENDSCRL/`,
`ALKU/`, `CREDITS/`) for the cell size and the character order, and pin both.

`GRAB/BLOWFNT.1BM` is on this list but is **not an LBM**: it is 64,000 bytes of bare 320×200 pixels
with a twelve-byte all-zero head, so it is A2's `raw_surface` and not A3's IFF loader. Whether it is
a font strip at all is for this task to establish from the part that uses it, and if it is not, say
so and drop it rather than inventing a cell grid for it.

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
