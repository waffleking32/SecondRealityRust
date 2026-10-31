//! The plasma's tables: the original's `.INC` fragments, parsed in their own
//! `db`/`dw` format (`ASMYT.ASM:12-24`'s `PSINI`, `LSINI4`, `LSINI16`;
//! `SPLINE.ASM:19-20`'s `SPLINE`; `PLZ.C:41-42`'s `PTAU`), and the `poro`
//! segment `plzline` addresses them through.

use sr_engine::part::PartError;

use super::NAME;

/// `ASMYT.ASM:12-24`: `psini` is `db 16384 dup` — 16,384 bytes.
const PSINI_LEN: usize = 16_384;
/// `ASMYT.ASM:18-24`: `lsini4` is `dw 8192 dup` — 8,192 words.
const LSINI_WORDS: usize = 8_192;
/// `PLZ.C:41-42`: `char ptau[256]`, of which `PTAU.INC` initialises 129.
const PTAU_LEN: usize = 129;
/// `SPLINE.ASM:19-20`: 1,024 `dw` coefficients.
pub(super) const SPLINE_WORDS: usize = 1_024;

/// The table bases, from `PLZPART.MAP:368-369,391`: `_psini 027D:0000`,
/// `_lsini4 027D:4000`, `_lsini16 027D:8000`. `ASMYT.ASM:46-61`'s `0C100h`..`0C400h`
/// are placeholders inside `plzline`'s own code (`_plzline` is `027D:C000`), which
/// `setplzparas` (`:74-95`) patches.
const MAP_PSINI: u16 = 0x0000;
const MAP_LSINI4: u16 = 0x4000;
const MAP_LSINI16: u16 = 0x8000;

/// The five `.INC` fragments the part reads, parsed.
pub(super) struct Tables {
    pub(super) psini: Vec<u8>,
    pub(super) lsini4: Vec<i16>,
    pub(super) lsini16: Vec<i16>,
    pub(super) ptau: Vec<i16>,
    pub(super) spline: Vec<i16>,
    /// The `poro` segment as the assembly addresses it: `psini` at 0x0000,
    /// `lsini4` at 0x4000 and `lsini16` at 0x8000 (`PLZPART.MAP`). A read
    /// below `lsini16` — `setplzparas` can place the lc2 displacement at
    /// `0x7FD8` — lands in `lsini4`'s last bytes, exactly as the original's
    /// 16-bit address does.
    pub(super) segment: Vec<u8>,
}

/// Parse one of the original's `.INC` fragments: the decimal values of its
/// `db`/`dw` lines, in file order. `;` starts a comment, and `PTAU.INC` has no
/// keyword at all (its values sit between braces).
pub(super) fn parse_inc(text: &str) -> Result<Vec<i32>, String> {
    let mut values = Vec::new();
    let mut saw_keyword = false;
    for (line_number, raw) in text.lines().enumerate() {
        let trimmed = raw.split(';').next().unwrap_or("").trim();
        if trimmed.is_empty() {
            continue;
        }
        let starts = trimmed.starts_with("db") || trimmed.starts_with("dw");
        if starts {
            saw_keyword = true;
        } else if saw_keyword {
            // Continuation lines do not exist in these files; anything else is
            // a comment or a label.
            continue;
        }
        let body = if starts { &trimmed[2..] } else { trimmed };
        for token in body.split(|c: char| c == ',' || c == '{' || c == '}' || c.is_whitespace()) {
            if token.is_empty() {
                continue;
            }
            let value = token
                .parse::<i32>()
                .map_err(|_| format!("line {}: {token:?} is not a number", line_number + 1))?;
            values.push(value);
        }
    }
    Ok(values)
}

/// The directory the `.INC` fragments are read from.
const TABLE_DIR: &str = "assets/PLZPART";

impl Tables {
    pub(super) fn load() -> Result<Tables, PartError> {
        let read = |name: &str| -> Result<&'static str, PartError> {
            let path = format!("{TABLE_DIR}/{name}");
            let bytes = crate::assets::read(&path).map_err(|error| PartError::new(NAME, error))?;
            std::str::from_utf8(bytes)
                .map_err(|error| PartError::new(NAME, format!("{path}: {error}")))
        };
        let parse = |name: &str, text: &str| -> Result<Vec<i32>, PartError> {
            parse_inc(text).map_err(|error| PartError::new(NAME, format!("{name}: {error}")))
        };
        let psini_values = parse("PSINI.INC", read("PSINI.INC")?)?;
        let lsini4_values = parse("LSINI4.INC", read("LSINI4.INC")?)?;
        let lsini16_values = parse("LSINI16.INC", read("LSINI16.INC")?)?;
        let ptau_values = parse("PTAU.INC", read("PTAU.INC")?)?;
        let spline_values = parse("SPLINE.INC", read("SPLINE.INC")?)?;
        let mut tables = Tables {
            psini: psini_values.iter().map(|&v| v as u8).collect(),
            lsini4: lsini4_values.iter().map(|&v| v as i16).collect(),
            lsini16: lsini16_values.iter().map(|&v| v as i16).collect(),
            ptau: ptau_values.iter().map(|&v| v as i16).collect(),
            spline: spline_values.iter().map(|&v| v as i16).collect(),
            segment: Vec::new(),
        };
        tables.check_sizes()?;
        tables.segment = tables.build_segment();
        Ok(tables)
    }

    fn check_sizes(&self) -> Result<(), PartError> {
        let checks = [
            ("PSINI.INC", self.psini.len(), PSINI_LEN),
            ("LSINI4.INC", self.lsini4.len(), LSINI_WORDS),
            ("LSINI16.INC", self.lsini16.len(), LSINI_WORDS),
            ("PTAU.INC", self.ptau.len(), PTAU_LEN),
            ("SPLINE.INC", self.spline.len(), SPLINE_WORDS),
        ];
        for (name, found, wanted) in checks {
            if found != wanted {
                return Err(PartError::new(
                    NAME,
                    format!("{name} holds {found} values; the original's array is {wanted}"),
                ));
            }
        }
        Ok(())
    }

    /// The `poro` segment's first 0xC000 bytes: the three tables in the map's
    /// order, so a 16-bit address that runs off one table's end reads the
    /// next one's bytes.
    fn build_segment(&self) -> Vec<u8> {
        let mut segment = vec![0u8; 0xC000];
        for (index, &value) in self.psini.iter().enumerate() {
            segment[MAP_PSINI as usize + index] = value;
        }
        for (table, base) in [(&self.lsini4, MAP_LSINI4), (&self.lsini16, MAP_LSINI16)] {
            for (index, &value) in table.iter().enumerate() {
                let at = base as usize + index * 2;
                segment[at..at + 2].copy_from_slice(&value.to_le_bytes());
            }
        }
        segment
    }

    /// `plzline`'s four lookups, at 16-bit segment addresses.
    pub(super) fn plasma_value(&self, r: i32, y: i32, c1: i32, c2: i32, c3: i32, c4: i32) -> u8 {
        let word_at = |address: u16| -> i32 {
            let at = address as usize;
            i16::from_le_bytes([self.segment[at], self.segment[at + 1]]) as i32
        };
        let byte_at = |address: u16| -> u8 { self.segment[address as usize] };
        // `mov bx,[ds:si+disp]`: lsini16 - 8r + 640 + 2c2 + 2y.
        let b = word_at(
            MAP_LSINI16
                .wrapping_add(2 * c2 as u16)
                .wrapping_sub(8 * r as u16)
                .wrapping_add(640)
                .wrapping_add(2 * y as u16),
        );
        // lc1: psini + c1 + 8r + bx.
        let px = byte_at(
            MAP_PSINI
                .wrapping_add(c1 as u16)
                .wrapping_add(8 * r as u16)
                .wrapping_add(b as u16),
        );
        // lc4: lsini4 + 32r + 2c4 + 2y.
        let d = word_at(
            MAP_LSINI4
                .wrapping_add(32 * r as u16)
                .wrapping_add(2 * c4 as u16)
                .wrapping_add(2 * y as u16),
        );
        // lc3: psini + 320 - 4r + c3 + bx + 2y.
        let py = byte_at(
            MAP_PSINI
                .wrapping_add(320)
                .wrapping_sub(4 * r as u16)
                .wrapping_add(c3 as u16)
                .wrapping_add(d as u16)
                .wrapping_add(2 * y as u16),
        );
        px.wrapping_add(py)
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use crate::parts::test_rig::repo;

    pub(in super::super) fn tables() -> Tables {
        Tables::load().expect("the five fragments load")
    }

    fn tree_text(name: &str) -> String {
        std::fs::read_to_string(repo(&format!("{TABLE_DIR}/{name}"))).unwrap()
    }

    /// The tree's `RATA.INC`, which the part does not read: its `kkk=100`
    /// symbol, `kkk*n` products and `REPT 100 … ENDM` tail expanded to plain
    /// `dw` lines for [`parse_inc`].
    pub(in super::super) fn tree_rata() -> Vec<i32> {
        let mut plain = String::new();
        let mut repeat: Option<(usize, String)> = None;
        for raw in tree_text("RATA.INC").lines() {
            let line = raw.split(';').next().unwrap_or("").trim();
            if let Some(count) = line.strip_prefix("REPT") {
                repeat = Some((count.trim().parse().unwrap(), String::new()));
            } else if line == "ENDM" {
                let (count, block) = repeat.take().unwrap();
                plain.push_str(&block.repeat(count));
            } else if let Some(values) = line.strip_prefix("dw") {
                let words: Vec<String> = values
                    .split(',')
                    .map(|token| {
                        let token = token.trim().replace("kkk", "100");
                        match token.split_once('*') {
                            Some((a, b)) => (a.parse::<i32>().unwrap() * b.parse::<i32>().unwrap())
                                .to_string(),
                            None => token,
                        }
                    })
                    .collect();
                let out = repeat.as_mut().map_or(&mut plain, |(_, block)| block);
                out.push_str(&format!("dw {}\n", words.join(",")));
            }
        }
        parse_inc(&plain).unwrap()
    }

    #[test]
    fn the_inc_fragments_parse_to_their_original_sizes_and_edges() {
        let t = tables();
        assert_eq!(t.psini.len(), PSINI_LEN);
        assert_eq!((t.psini[0], t.psini[1], t.psini[2]), (64, 64, 64));
        assert_eq!(&t.psini[PSINI_LEN - 4..], &[62, 63, 63, 63]);
        assert_eq!(t.lsini4.len(), LSINI_WORDS);
        assert_eq!((t.lsini4[0], t.lsini4[1]), (512, 513));
        assert_eq!(t.lsini4[LSINI_WORDS - 1], 510);
        assert_eq!(t.lsini16.len(), LSINI_WORDS);
        assert_eq!((t.lsini16[0], t.lsini16[1]), (1024, 1027));
        assert_eq!(t.lsini16[LSINI_WORDS - 1], 1020);
        assert_eq!(t.ptau.len(), PTAU_LEN);
        assert_eq!((t.ptau[0], t.ptau[1], t.ptau[128]), (0, 1, 1));
        assert_eq!(t.spline.len(), SPLINE_WORDS);
        assert_eq!(&t.spline[..4], &[0, 0, 0, 0]);
        // The tree's two fragments the cube does not read: SINIT.INC is 2,575 db bytes
        // (the 1,024-word table, quarter turn 256, and the stray repeat `INCLUDE.ASM`
        // links), RATA.INC 1,088 words with `kkk = 100`.
        let sinit = parse_inc(&tree_text("SINIT.INC")).unwrap();
        assert_eq!(sinit.len(), 2_575);
        let word = |at: usize| i16::from_le_bytes([sinit[2 * at] as u8, sinit[2 * at + 1] as u8]);
        assert_eq!((word(0), word(1), word(256)), (0, 201, 32766));
        let rata = tree_rata();
        assert_eq!(rata.len(), 1_088);
        assert_eq!(&rata[..8], &[0, 2000, 500, 0, 400, 600, 0, 0]);
    }
}
