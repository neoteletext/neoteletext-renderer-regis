//! NeoTeletext's 55 provided character sets, read from the engine's
//! checkout for `--teletext` (renderer-parity.md PAR-12, as
//! `naplps-render --teletext` reads them for REQ-6668): the code of every
//! character of every set from the engine's generated list
//! `crates/neoteletext-core/src/glyphs/required.txt`, each character's 12
//! by 20 glyph from the Noto Teletext BDF files in `third_party/noto/`, and
//! for the plain Arabic-script sets (group 1001, sets 44 to 51) each
//! letter's joining type from the engine's table and its joined forms from
//! `crates/neoteletext-core/src/glyphs/forms.txt` (PAR-13). The glyphs are
//! the project's own font and nothing else (DEC-6669).

use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::Path;

use regis::limits::PROVIDED_SETS;
use regis::{JoinedForm, JoiningType, ProvidedSet};

/// The engine's list of every character a set must draw, under its checkout.
const REQUIRED_LIST: &str = "crates/neoteletext-core/src/glyphs/required.txt";
/// The engine's list of every joined form its font draws: code point,
/// letter, form.
const FORMS_LIST: &str = "crates/neoteletext-core/src/glyphs/forms.txt";
/// The engine's source holding the joining type of each letter of group
/// 1001, in a block its script generates (Unicode `ArabicShaping.txt`).
const JOINING_SOURCE: &str = "crates/neoteletext-core/src/charset/arabic_script.rs";
/// The Noto Teletext font, four files of one font (the engine's
/// `third_party/noto/PROVENANCE.md`).
const NOTO_FILES: [&str; 4] = [
    "third_party/noto/noto-teletext.bdf",
    "third_party/noto/noto-teletext-composed.bdf",
    "third_party/noto/noto-teletext-arabic.bdf",
    "third_party/noto/noto-teletext-japanese.bdf",
];
/// The engine's cell: 12 pixels by 20 lines (its `CELL_WIDTH` and
/// `CELL_HEIGHT`).
const CELL_WIDTH: u8 = 12;
/// See [`CELL_WIDTH`].
const CELL_HEIGHT: u8 = 20;
/// The largest list, source or font file read: four megabytes, far beyond
/// the engine's (the list is 127 kilobytes, the largest font 157).
const MAX_FILE_BYTES: u64 = 4 * 1024 * 1024;
/// Characters one BDF file may hold: the engine's REQ-FNT-002 allows 1,024.
const MAX_BDF_CHARACTERS: usize = 1024;
/// The bits of a BDF row of a 12-pixel glyph: two bytes, the leftmost pixel
/// the top bit.
const BDF_ROW_BITS: u32 = 16;
/// The plain Arabic-script sets, group 1001, whose letters are joined
/// (REQ-CHR-015): `G0-Arabic-Persian` to `G0-Arabic-Plain`.
const JOINED_SETS: core::ops::RangeInclusive<usize> = 44..=51;
/// The markers of the engine's generated joining table.
const JOINING_TABLE_START: &str = "const JOINING_TYPES";
const JOINING_TABLE_END: &str = "];";

/// Each set's name in the engine's `FONT_SETS`, in its order: set `n` is
/// alphabet 100 + `n` (the order of REQ-6668's table).
pub(crate) const SET_NAMES: [&str; PROVIDED_SETS as usize] = [
    "G0-Latin",
    "G0-Latin-English",
    "G0-Latin-German",
    "G0-Latin-SwedishFinnishHungarian",
    "G0-Latin-Italian",
    "G0-Latin-French",
    "G0-Latin-PortugueseSpanish",
    "G0-Latin-CzechSlovak",
    "G0-Latin-Polish",
    "G0-Latin-Turkish",
    "G0-Latin-SerbianCroatianSlovenian",
    "G0-Latin-Rumanian",
    "G0-Latin-Estonian",
    "G0-Latin-LettishLithuanian",
    "G0-Cyrillic-1",
    "G0-Cyrillic-2",
    "G0-Cyrillic-3",
    "G0-Greek",
    "G0-Arabic",
    "G0-Hebrew",
    "G2-Latin",
    "G2-Cyrillic",
    "G2-Greek",
    "G2-Arabic",
    "G0-Latin-HausaFula",
    "G0-Latin-WestAfrican",
    "G0-Latin-Ewe",
    "G0-Latin-KabyleTamazight",
    "G0-Latin-Azerbaijani",
    "G0-Latin-KanuriMossi",
    "G0-Vietnamese",
    "G0-Igbo",
    "G0-Cyrillic-Ukrainian",
    "G0-Cyrillic-Belarusian",
    "G0-Cyrillic-Macedonian",
    "G0-Cyrillic-Serbian",
    "G0-Cyrillic-KyrgyzMongolian",
    "G0-Cyrillic-Tatar",
    "G0-Cyrillic-Tajik",
    "G0-Georgian",
    "G0-Armenian",
    "G0-Tifinagh",
    "G2-Vietnamese",
    "G2-Armenian",
    "G0-Arabic-Persian",
    "G0-Arabic-Urdu",
    "G0-Arabic-Pashto",
    "G0-Arabic-Sindhi",
    "G0-Arabic-Sorani",
    "G0-Arabic-Uyghur",
    "G0-Arabic-Balochi",
    "G0-Arabic-Plain",
    "G0-Latin-JapaneseRomaji",
    "G0-Hiragana",
    "G0-Katakana",
];

/// The engine's list's name for a set: spaces for hyphens, and the sets of
/// a script beside it written without the script's name.
pub(crate) fn list_name(name: &str) -> String {
    let spaced = name.replace('-', " ");
    for (prefix, script) in [("G0 Cyrillic ", "Cyrillic"), ("G0 Arabic ", "Arabic")] {
        if let Some(rest) = spaced.strip_prefix(prefix) {
            return match rest {
                "1" | "2" | "3" => format!("G0 {script}{rest}"),
                "Plain" => "G0 ArabicPlain".to_owned(),
                _ => format!("G0 {rest}"),
            };
        }
    }
    spaced
}

/// Every set's code table: for each set, (code, Unicode scalar) of each
/// position listed.
type CodeTables = Vec<Vec<(u8, u32)>>;

/// Reads a file of at most [`MAX_FILE_BYTES`], refusing a longer one before
/// reading it whole.
fn read(path: &Path) -> Result<String, String> {
    let file = fs::File::open(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let mut bytes = Vec::new();
    file.take(MAX_FILE_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| format!("{}: {error}", path.display()))?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_FILE_BYTES {
        return Err(format!("{} is more than {MAX_FILE_BYTES} bytes", path.display()));
    }
    String::from_utf8(bytes).map_err(|_| format!("{}: not UTF-8 text", path.display()))
}

/// A `U+XXXX` code point.
fn scalar(field: &str) -> Option<u32> {
    u32::from_str_radix(field.strip_prefix("U+")?, 16).ok()
}

/// The code tables from the engine's list: lines `U+XXXX CHARACTER USE;
/// USE; ...`, each use a set's name and a position `C/R`, or a note.
fn code_tables(list: &str) -> Result<CodeTables, String> {
    let names: Vec<String> = SET_NAMES.iter().map(|name| list_name(name)).collect();
    let numbers: HashMap<&str, usize> =
        names.iter().enumerate().map(|(number, name)| (name.as_str(), number)).collect();
    let mut tables = vec![Vec::new(); SET_NAMES.len()];
    for (index, line) in list.lines().enumerate() {
        let line_number = index.saturating_add(1);
        if line.starts_with(';') || line.trim().is_empty() {
            continue;
        }
        let mut fields = line.splitn(3, ' ');
        let point = fields.next().and_then(scalar).ok_or_else(|| format!("line {line_number}: no code point"))?;
        let _character = fields.next();
        for usage in fields.next().unwrap_or("").split("; ") {
            let Some((name, position)) = usage.rsplit_once(' ') else { continue };
            let Some(&number) = numbers.get(name) else { continue };
            let code = position_code(position).ok_or_else(|| format!("line {line_number}: position {position:?}"))?;
            if let Some(table) = tables.get_mut(number) {
                table.push((code, point));
            }
        }
    }
    Ok(tables)
}

/// The code of a position `C/R` (column 2 to 7, row 0 to F).
fn position_code(position: &str) -> Option<u8> {
    let (column, row) = position.split_once('/')?;
    let column = u8::from_str_radix(column, 16).ok().filter(|column| (2..=7).contains(column))?;
    let row = u8::from_str_radix(row, 16).ok().filter(|row| *row <= 0xF)?;
    column.checked_mul(16)?.checked_add(row)
}

/// The glyphs of a BDF file of 12 by 20 glyphs: Unicode scalar to rows from
/// the top, the leftmost pixel the most significant of 12 bits.
fn bdf_glyphs(text: &str, glyphs: &mut HashMap<u32, Vec<u16>>) -> Result<(), String> {
    let mut lines = text.lines().enumerate();
    let mut count = 0usize;
    while let Some((_, line)) = lines.next() {
        if !line.starts_with("STARTCHAR") {
            continue;
        }
        count = count.saturating_add(1);
        if count > MAX_BDF_CHARACTERS {
            return Err(format!("more than {MAX_BDF_CHARACTERS} characters"));
        }
        let mut encoding = None;
        let mut rows = Vec::new();
        let mut in_bitmap = false;
        for (index, line) in lines.by_ref() {
            let line_number = index.saturating_add(1);
            if line == "ENDCHAR" {
                break;
            }
            if in_bitmap {
                if rows.len() >= usize::from(CELL_HEIGHT) {
                    return Err(format!("line {line_number}: more than {CELL_HEIGHT} rows"));
                }
                let row =
                    u16::from_str_radix(line.trim(), 16).map_err(|_| format!("line {line_number}: bitmap row"))?;
                rows.push(row.checked_shr(BDF_ROW_BITS.saturating_sub(u32::from(CELL_WIDTH))).unwrap_or(0));
            } else if let Some(value) = line.strip_prefix("ENCODING ") {
                encoding = value.trim().parse::<u32>().ok();
            } else if let Some(value) = line.strip_prefix("BBX ") {
                if value.split_whitespace().collect::<Vec<_>>()
                    != [CELL_WIDTH.to_string(), CELL_HEIGHT.to_string(), "0".into(), "0".into()]
                {
                    return Err(format!("line {line_number}: a glyph is not the {CELL_WIDTH} by {CELL_HEIGHT} cell"));
                }
            } else if line == "BITMAP" {
                in_bitmap = true;
            }
        }
        let encoding = encoding.ok_or("a character has no ENCODING")?;
        if rows.len() != usize::from(CELL_HEIGHT) {
            return Err(format!("U+{encoding:04X} has {} rows, not {CELL_HEIGHT}", rows.len()));
        }
        glyphs.insert(encoding, rows);
    }
    Ok(())
}

/// The joining types of the engine's generated table: lines
/// `('\u{0622}', JoiningType::Right), // NAME`. The engine's join-causing
/// tatweel joins as a dual-joining letter does.
fn joining_types(source: &str) -> Result<HashMap<u32, JoiningType>, String> {
    let table = source
        .split_once(JOINING_TABLE_START)
        .and_then(|(_, rest)| rest.split_once(JOINING_TABLE_END))
        .map(|(table, _)| table)
        .ok_or("no joining table")?;
    let mut types = HashMap::new();
    for line in table.lines().filter(|line| line.trim_start().starts_with("('")) {
        let point = line
            .split_once("\\u{")
            .and_then(|(_, rest)| rest.split_once('}'))
            .and_then(|(hex, _)| u32::from_str_radix(hex, 16).ok())
            .ok_or_else(|| format!("joining table line {line:?}: no code point"))?;
        let kind = match line.split_once("JoiningType::").and_then(|(_, rest)| rest.split_once(')')) {
            Some(("Dual" | "Causing", _)) => JoiningType::Dual,
            Some(("Right", _)) => JoiningType::Right,
            Some(("None", _)) => JoiningType::None,
            Some(("Transparent", _)) => JoiningType::Transparent,
            _ => return Err(format!("joining table line {line:?}: no joining type")),
        };
        types.insert(point, kind);
    }
    Ok(types)
}

/// The joined forms of the engine's list: letter to (form, code point).
fn joined_forms(list: &str) -> Result<HashMap<u32, Vec<(JoinedForm, u32)>>, String> {
    let mut forms: HashMap<u32, Vec<(JoinedForm, u32)>> = HashMap::new();
    for (index, line) in list.lines().enumerate() {
        if line.starts_with(';') || line.trim().is_empty() {
            continue;
        }
        let fields: Vec<&str> = line.split_whitespace().collect();
        let entry = match fields.as_slice() {
            [point, letter, form] => scalar(point).zip(scalar(letter)).zip(match *form {
                "final" => Some(JoinedForm::Final),
                "initial" => Some(JoinedForm::Initial),
                "medial" => Some(JoinedForm::Medial),
                _ => None,
            }),
            _ => None,
        };
        let ((point, letter), form) = entry.ok_or_else(|| format!("line {}: not a form", index.saturating_add(1)))?;
        forms.entry(letter).or_default().push((form, point));
    }
    Ok(forms)
}

/// Every set, from the engine's checkout at `engine`, with the joining data
/// of the plain Arabic-script sets.
pub(crate) fn load(engine: &Path) -> Result<Vec<ProvidedSet>, String> {
    let in_file = |path: &str, error: String| format!("{}: {error}", engine.join(path).display());
    let tables = code_tables(&read(&engine.join(REQUIRED_LIST))?).map_err(|error| in_file(REQUIRED_LIST, error))?;
    let kinds = joining_types(&read(&engine.join(JOINING_SOURCE))?).map_err(|error| in_file(JOINING_SOURCE, error))?;
    let forms = joined_forms(&read(&engine.join(FORMS_LIST))?).map_err(|error| in_file(FORMS_LIST, error))?;
    let mut glyphs = HashMap::new();
    for file in NOTO_FILES {
        bdf_glyphs(&read(&engine.join(file))?, &mut glyphs).map_err(|error| in_file(file, error))?;
    }
    let glyph =
        |point: u32, name: &str| glyphs.get(&point).ok_or_else(|| format!("{name}: no glyph for U+{point:04X}"));
    tables
        .iter()
        .zip(SET_NAMES)
        .enumerate()
        .map(|(number, (table, name))| {
            let mut set = ProvidedSet::new(CELL_WIDTH, CELL_HEIGHT).map_err(|error| error.to_string())?;
            let joined = JOINED_SETS.contains(&number);
            for &(code, point) in table {
                let fault = |error: regis::Error| format!("{name} {code:#04x}: {error}");
                set.set_glyph(code, glyph(point, name)?).map_err(fault)?;
                if !joined {
                    continue;
                }
                set.set_joining(code, kinds.get(&point).copied().unwrap_or_default()).map_err(fault)?;
                for &(form, form_point) in forms.get(&point).map_or(&[][..], Vec::as_slice) {
                    set.set_form(code, form, glyph(form_point, name)?).map_err(fault)?;
                }
            }
            Ok(set)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_list_gives_each_set_its_codes() {
        let list =
            "; a comment\nU+0041 A G0 Latin 4/1; G0 Greek 4/1; spacing mark (decision E.26)\nU+0391 Α G0 Greek 4/1\n";
        let tables = code_tables(list).unwrap();
        assert_eq!(tables[0], [(0x41, 0x41)]);
        assert_eq!(tables[17], [(0x41, 0x41), (0x41, 0x391)]);
        assert!(code_tables("U+0041 A G0 Latin 8/1").is_err(), "a position outside columns 2 to 7");
        assert!(code_tables("0041 A G0 Latin 4/1").is_err(), "no code point");
        let persian = code_tables("U+0633 س G0 Persian 5/4; G0 ArabicPlain 5/4\n").unwrap();
        assert_eq!((persian[44].as_slice(), persian[51].as_slice()), (&[(0x54, 0x633)][..], &[(0x54, 0x633)][..]));
    }

    fn character(bbx: &str, rows: usize) -> String {
        format!("STARTCHAR x\nENCODING 65\nBBX {bbx}\nBITMAP\n{}ENDCHAR\n", "FFF0\n".repeat(rows))
    }

    #[test]
    fn a_bdf_glyph_must_be_the_cell() {
        let mut glyphs = HashMap::new();
        bdf_glyphs(&character("12 20 0 0", 20), &mut glyphs).unwrap();
        assert_eq!(glyphs[&65], vec![0x0FFF; 20]);
        assert!(bdf_glyphs(&character("8 16 0 0", 20), &mut glyphs).is_err());
        assert!(bdf_glyphs(&character("12 20 0 0", 21), &mut glyphs).is_err());
        assert!(bdf_glyphs(&character("12 20 0 0", 19), &mut glyphs).is_err());
        assert!(bdf_glyphs(&character("12 20 0 0", 20).repeat(MAX_BDF_CHARACTERS + 1), &mut glyphs).is_err());
    }

    #[test]
    fn the_joining_table_and_the_forms_are_read() {
        let source = "x\nconst JOINING_TYPES: [(char, JoiningType); 3] = [\n    ('\\u{0621}', JoiningType::None), // HAMZA\n    ('\\u{0627}', JoiningType::Right), // ALEF\n    ('\\u{0640}', JoiningType::Causing), // TATWEEL\n];\n";
        let types = joining_types(source).unwrap();
        assert_eq!(
            (types[&0x621], types[&0x627], types[&0x640]),
            (JoiningType::None, JoiningType::Right, JoiningType::Dual)
        );
        assert!(joining_types("no table").is_err());
        let forms = joined_forms("; generated\nU+FE91 U+0628 initial\nU+FE90 U+0628 final\n").unwrap();
        assert_eq!(forms[&0x628], [(JoinedForm::Initial, 0xFE91), (JoinedForm::Final, 0xFE90)]);
        assert!(joined_forms("U+FE91 U+0628 sideways").is_err());
    }
}
