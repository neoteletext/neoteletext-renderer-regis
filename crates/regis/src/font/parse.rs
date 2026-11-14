//! The text form of a font.

use super::{CELL_HEIGHT, Font, Strike, TALL_HEIGHT};
use crate::Error;
use crate::limits::MAX_FONT_BYTES;

pub(super) fn parse(text: &str) -> Result<Font, Error> {
    if text.len() > MAX_FONT_BYTES {
        return Err(Error::FontTooLong);
    }
    let mut font = Font::default();
    let mut height = None;
    for (index, line) in text.lines().enumerate() {
        let fault = Error::Font { line: index.saturating_add(1) };
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(header) = line.strip_prefix("[strike ").and_then(|rest| rest.strip_suffix(']')) {
            height = Some(match header.trim() {
                "10" => CELL_HEIGHT,
                "20" => TALL_HEIGHT,
                _ => return Err(fault),
            });
            continue;
        }
        let height = height.ok_or(fault)?;
        let (code, rows) = glyph_line(line, height).ok_or(fault)?;
        let strike = if height == TALL_HEIGHT { &mut font.tall } else { &mut font.short };
        strike.get_or_insert_with(|| Strike::new(height)).set(code, rows.get(..height).unwrap_or(&[]));
    }
    Ok(font)
}

/// A glyph line: a code and exactly `height` row bytes, all two-digit hex.
fn glyph_line(line: &str, height: usize) -> Option<(u8, [u8; TALL_HEIGHT])> {
    let mut fields = line
        .split_ascii_whitespace()
        .map(|field| if field.len() == 2 { u8::from_str_radix(field, 16).ok() } else { None });
    let code = fields.next()??;
    let mut rows = [0; TALL_HEIGHT];
    for row in rows.iter_mut().take(height) {
        *row = fields.next()??;
    }
    fields.next().is_none().then_some((code, rows))
}

#[cfg(test)]
mod tests {
    use crate::{Error, Font};

    #[test]
    fn a_strike_of_each_height_is_read_and_chosen_by_cell_height() {
        let text = "# test\n[strike 10]\n41 18 24 42 42 7E 42 42 42 00 00\n[strike 20]\n41 \
                    00 00 18 18 24 24 42 42 42 42 7E 7E 42 42 42 42 42 42 00 00\n";
        let font = Font::parse(text).unwrap();
        assert!(font.glyph(0x41, 10).unwrap().lit(3, 0, 8, 10));
        assert!(!font.glyph(0x41, 20).unwrap().lit(3, 0, 8, 20));
        assert!(font.glyph(0x41, 20).unwrap().lit(3, 2, 8, 20));
        assert!(font.glyph(0x42, 10).is_none());
    }

    #[test]
    fn malformed_text_names_its_line() {
        assert_eq!(Font::parse("41 00").unwrap_err(), Error::Font { line: 1 });
        assert_eq!(Font::parse("[strike 12]").unwrap_err(), Error::Font { line: 1 });
        assert_eq!(Font::parse("[strike 10]\n41 00 00").unwrap_err(), Error::Font { line: 2 });
        assert_eq!(Font::parse("[strike 10]\nZZ 00 00 00 00 00 00 00 00 00 00").unwrap_err(), Error::Font { line: 2 });
    }
}
