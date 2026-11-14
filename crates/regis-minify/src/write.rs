//! Write controls: which settings a `W` command makes, and which of them
//! change nothing.

use crate::token::Token;

/// One setting of a write command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Setting {
    /// `I` and its value, as written.
    Colour(Vec<Token>),
    /// `P` with a pattern, a multiplier, or both.
    Pattern { bits: Option<Vec<u8>>, multiplier: Option<Vec<u8>> },
    /// `V`, `R`, `C` or `E`.
    Mode(u8),
    /// `N`, `F` or `M` with a number.
    Numbered(u8, Vec<u8>),
    /// Shading: it reads the present position and text, so it is never
    /// judged to change nothing.
    Shading(Vec<Token>),
}

impl Setting {
    pub(crate) fn write(&self, out: &mut Vec<u8>) {
        match self {
            Self::Colour(value) => {
                out.push(b'I');
                for token in value {
                    token.write(out);
                }
            }
            Self::Pattern { bits, multiplier } => {
                out.push(b'P');
                out.extend_from_slice(bits.as_deref().unwrap_or_default());
                if let Some(multiplier) = multiplier {
                    out.extend_from_slice(b"(M");
                    out.extend_from_slice(multiplier);
                    out.push(b')');
                }
            }
            Self::Mode(letter) => out.push(*letter),
            Self::Numbered(letter, number) => {
                out.push(*letter);
                out.extend_from_slice(number);
            }
            Self::Shading(value) => {
                out.push(b'S');
                for token in value {
                    token.write(out);
                }
            }
        }
    }
}

/// Takes a parenthesised group, parentheses included, from the front.
fn group<'a>(tokens: &mut &'a [Token]) -> Option<&'a [Token]> {
    let mut depth = 0_u32;
    for (index, token) in tokens.iter().enumerate() {
        match token {
            Token::Open => depth = depth.saturating_add(1),
            Token::Close => depth = depth.checked_sub(1)?,
            _ => {}
        }
        if depth == 0 {
            let (taken, rest) = tokens.split_at_checked(index.saturating_add(1))?;
            *tokens = rest;
            return Some(taken);
        }
    }
    None
}

fn number(tokens: &mut &[Token]) -> Option<Vec<u8>> {
    let (first, rest) = tokens.split_first()?;
    let Token::Number(digits) = first else { return None };
    *tokens = rest;
    Some(digits.clone())
}

fn pattern(tokens: &mut &[Token]) -> Option<Setting> {
    let bits = number(tokens);
    let multiplier = match tokens {
        [Token::Open, Token::Letter(b'M'), Token::Number(value), Token::Close, rest @ ..] => {
            let value = value.clone();
            *tokens = rest;
            Some(value)
        }
        _ => None,
    };
    (bits.is_some() || multiplier.is_some()).then_some(Setting::Pattern { bits, multiplier })
}

fn shading(tokens: &mut &[Token]) -> Setting {
    let mut value = Vec::new();
    loop {
        match tokens.first() {
            Some(Token::Open) => match group(tokens) {
                Some(taken) => value.extend_from_slice(taken),
                None => break,
            },
            Some(token @ (Token::Number(_) | Token::Quoted(_) | Token::Bracket(_))) => {
                value.push(token.clone());
                *tokens = tokens.get(1..).unwrap_or_default();
            }
            _ => break,
        }
    }
    Setting::Shading(value)
}

/// The settings of a write command's arguments, or None if they are not
/// understood, in which case the command is left exactly as written.
pub(crate) fn parse(arguments: &[Token]) -> Option<Vec<Setting>> {
    let mut whole = arguments;
    let outer = group(&mut whole)?;
    if !whole.is_empty() {
        return None;
    }
    let mut tokens = outer.get(1..outer.len().checked_sub(1)?)?;
    let mut settings = Vec::new();
    while let Some((first, rest)) = tokens.split_first() {
        tokens = rest;
        let setting = match first {
            Token::Comma => continue,
            Token::Letter(b'I') => match tokens.first()? {
                Token::Open => Setting::Colour(group(&mut tokens)?.to_vec()),
                _ => Setting::Colour(vec![Token::Number(number(&mut tokens)?)]),
            },
            Token::Letter(b'P') => pattern(&mut tokens)?,
            Token::Letter(letter @ (b'V' | b'R' | b'C' | b'E')) => Setting::Mode(*letter),
            Token::Letter(letter @ (b'N' | b'F' | b'M')) => Setting::Numbered(*letter, number(&mut tokens)?),
            Token::Letter(b'S') => shading(&mut tokens),
            _ => return None,
        };
        settings.push(setting);
    }
    Some(settings)
}

/// What is known of the write controls in force.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Controls {
    colour: Option<Vec<Token>>,
    bits: Option<Vec<u8>>,
    pattern_multiplier: Option<Vec<u8>>,
    mode: Option<u8>,
    numbered: [Option<Vec<u8>>; 3],
}

const fn slot(letter: u8) -> usize {
    match letter {
        b'N' => 0,
        b'F' => 1,
        _ => 2,
    }
}

impl Controls {
    /// Applies a setting. False if it changed nothing and can be left out.
    pub(crate) fn apply(&mut self, setting: &Setting) -> bool {
        match setting {
            Setting::Colour(value) => replace(&mut self.colour, value),
            Setting::Pattern { bits, multiplier } => {
                let bits_change = bits.as_ref().is_some_and(|bits| replace(&mut self.bits, bits));
                let multiplier_changes =
                    multiplier.as_ref().is_some_and(|value| replace(&mut self.pattern_multiplier, value));
                bits_change || multiplier_changes
            }
            Setting::Mode(letter) => replace(&mut self.mode, letter),
            Setting::Numbered(letter, number) => {
                self.numbered.get_mut(slot(*letter)).is_none_or(|known| replace(known, number))
            }
            Setting::Shading(_) => true,
        }
    }
}

/// Stores a value; false if it was already there.
fn replace<T: Clone + PartialEq>(known: &mut Option<T>, value: &T) -> bool {
    if known.as_ref() == Some(value) {
        return false;
    }
    *known = Some(value.clone());
    true
}

/// Which earlier setting a later one makes pointless: the same kind of
/// setting, when nothing was drawn in between.
pub(crate) fn overrides(later: &Setting, earlier: &Setting) -> bool {
    match (later, earlier) {
        (Setting::Colour(_), Setting::Colour(_)) | (Setting::Mode(_), Setting::Mode(_)) => true,
        (Setting::Numbered(a, _), Setting::Numbered(b, _)) => a == b,
        // A later pattern replaces an earlier one only if it sets all the
        // earlier one set.
        (Setting::Pattern { bits: a, multiplier: b }, Setting::Pattern { bits: c, multiplier: d }) => {
            (a.is_some() || c.is_none()) && (b.is_some() || d.is_none())
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::{Controls, Setting, parse};
    use crate::token::Lexer;

    fn settings(text: &str) -> Option<Vec<Setting>> {
        let tokens: Vec<_> = Lexer::new(text.as_bytes()).collect();
        parse(&tokens)
    }

    fn written(settings: &[Setting]) -> String {
        let mut out = Vec::new();
        for setting in settings {
            setting.write(&mut out);
        }
        String::from_utf8(out).unwrap()
    }

    #[test]
    fn the_settings_of_a_write_command_are_read_and_written_back() {
        assert_eq!(written(&settings("(I3,P11100100(M3),V)").unwrap()), "I3P11100100(M3)V");
        assert_eq!(written(&settings("(I(H120L50S100)N1F15M4)").unwrap()), "I(H120L50S100)N1F15M4");
        assert_eq!(written(&settings("(S\"A\")").unwrap()), "S\"A\"");
        assert_eq!(written(&settings("(S(X)[400]I2)").unwrap()), "S(X)[400]I2");
        assert_eq!(written(&settings("(P(M4))").unwrap()), "P(M4)");
    }

    #[test]
    fn a_command_that_is_not_understood_is_refused() {
        assert!(settings("(Q3)").is_none());
        assert!(settings("(I3)V").is_none());
        assert!(settings("(I").is_none());
    }

    #[test]
    fn a_setting_already_in_force_changes_nothing() {
        let mut controls = Controls::default();
        let colour = &settings("(I3)").unwrap()[0];
        assert!(controls.apply(colour));
        assert!(!controls.apply(colour));
        let pattern = &settings("(P1010(M2))").unwrap()[0];
        assert!(controls.apply(pattern));
        assert!(!controls.apply(&settings("(P1010)").unwrap()[0]));
        assert!(controls.apply(&settings("(P1010(M3))").unwrap()[0]));
        let shading = &settings("(S1)").unwrap()[0];
        assert!(controls.apply(shading) && controls.apply(shading));
    }
}
