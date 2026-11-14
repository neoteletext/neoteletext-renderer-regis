//! Cutting a ReGIS command string into tokens.

/// One piece of a ReGIS command string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Token {
    /// A command or option letter, in upper case.
    Letter(u8),
    Open,
    Close,
    /// What stood between `[` and `]`, less blanks.
    Bracket(Vec<u8>),
    /// A quoted string with its quotation marks, exactly as written.
    Quoted(Vec<u8>),
    /// Digits with any sign and decimal point.
    Number(Vec<u8>),
    Comma,
    Semicolon,
    /// Bytes copied as they are: a macrograph operation, a load command,
    /// or a character with no meaning here.
    Raw(Vec<u8>),
}

impl Token {
    pub(crate) fn write(&self, out: &mut Vec<u8>) {
        match self {
            Self::Letter(letter) => out.push(*letter),
            Self::Open => out.push(b'('),
            Self::Close => out.push(b')'),
            Self::Bracket(inside) => {
                out.push(b'[');
                out.extend_from_slice(inside);
                out.push(b']');
            }
            Self::Quoted(bytes) | Self::Number(bytes) | Self::Raw(bytes) => out.extend_from_slice(bytes),
            Self::Comma => out.push(b','),
            Self::Semicolon => out.push(b';'),
        }
    }
}

/// Bytes a ReGIS interpreter passes over between tokens: blanks and
/// control characters.
const fn is_blank(byte: u8) -> bool {
    byte <= b' ' || byte == 0x7F
}

const fn is_number_byte(byte: u8) -> bool {
    byte.is_ascii_digit() || matches!(byte, b'+' | b'-' | b'.')
}

/// Reads tokens from a command string.
pub(crate) struct Lexer<'a> {
    rest: &'a [u8],
    /// Nesting of parentheses, so a letter is known as command or option.
    depth: u32,
}

impl<'a> Lexer<'a> {
    pub(crate) const fn new(input: &'a [u8]) -> Self {
        Self { rest: input, depth: 0 }
    }

    /// Takes bytes from the front while `keep` holds.
    fn take_while(&mut self, keep: impl Fn(u8) -> bool) -> &'a [u8] {
        let length = self.rest.iter().position(|byte| !keep(*byte)).unwrap_or(self.rest.len());
        let (taken, rest) = self.rest.split_at(length);
        self.rest = rest;
        taken
    }

    fn take(&mut self, length: usize) -> &'a [u8] {
        let (taken, rest) = self.rest.split_at(length.min(self.rest.len()));
        self.rest = rest;
        taken
    }

    /// The length of the quoted string at the front, its marks included.
    /// A doubled mark inside the string stands for one mark.
    fn quoted_length(&self) -> usize {
        let Some(mark) = self.rest.first().copied() else { return 0 };
        let mut index = 1_usize;
        loop {
            match (self.rest.get(index), self.rest.get(index.saturating_add(1))) {
                (None, _) => return self.rest.len(),
                (Some(a), Some(b)) if *a == mark && *b == mark => index = index.saturating_add(2),
                (Some(a), _) if *a == mark => return index.saturating_add(1),
                _ => index = index.saturating_add(1),
            }
        }
    }

    /// The length up to and including the first `end` outside quotation
    /// marks, or everything if there is none.
    fn length_through(&self, end: &[u8]) -> usize {
        let mut index = 0_usize;
        while let Some(tail) = self.rest.get(index..) {
            if tail.is_empty() {
                break;
            }
            if tail.starts_with(end) {
                return index.saturating_add(end.len());
            }
            let step = match tail.first() {
                Some(b'"' | b'\'') => Lexer { rest: tail, depth: 0 }.quoted_length().max(1),
                _ => 1,
            };
            index = index.saturating_add(step);
        }
        self.rest.len()
    }

    /// A macrograph operation: a definition is kept whole, because its
    /// body is replayed later wherever it is invoked.
    fn macrograph(&mut self) -> Token {
        let length = match self.rest.get(1) {
            Some(b':') => self.length_through(b"@;"),
            Some(_) => 2,
            None => 1,
        };
        Token::Raw(self.take(length).to_vec())
    }
}

impl Iterator for Lexer<'_> {
    type Item = Token;

    fn next(&mut self) -> Option<Token> {
        self.take_while(is_blank);
        let first = self.rest.first().copied()?;
        Some(match first {
            b'(' => {
                self.take(1);
                self.depth = self.depth.saturating_add(1);
                Token::Open
            }
            b')' => {
                self.take(1);
                self.depth = self.depth.saturating_sub(1);
                Token::Close
            }
            b',' => {
                self.take(1);
                Token::Comma
            }
            b';' => {
                self.take(1);
                Token::Semicolon
            }
            b'[' => {
                self.take(1);
                let inside: Vec<u8> =
                    self.take_while(|byte| byte != b']').iter().copied().filter(|byte| !is_blank(*byte)).collect();
                self.take(1);
                Token::Bracket(inside)
            }
            b'"' | b'\'' => {
                let length = self.quoted_length();
                Token::Quoted(self.take(length).to_vec())
            }
            b'@' => self.macrograph(),
            // A load command's character cells are hexadecimal, which would
            // read as letters: the command is kept whole up to its end.
            b'L' | b'l' if self.depth == 0 => {
                let length = self.length_through(b";");
                Token::Raw(self.take(length).to_vec())
            }
            letter if letter.is_ascii_alphabetic() => {
                self.take(1);
                Token::Letter(letter.to_ascii_uppercase())
            }
            byte if is_number_byte(byte) => Token::Number(self.take_while(is_number_byte).to_vec()),
            _ => Token::Raw(self.take(1).to_vec()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{Lexer, Token};

    fn tokens(input: &str) -> Vec<Token> {
        Lexer::new(input.as_bytes()).collect()
    }

    #[test]
    fn blanks_go_and_letters_are_upper_case() {
        assert_eq!(
            tokens(" p [ 1 , 2 ]\n v[+3]"),
            [Token::Letter(b'P'), Token::Bracket(b"1,2".to_vec()), Token::Letter(b'V'), Token::Bracket(b"+3".to_vec())]
        );
    }

    #[test]
    fn a_quoted_string_keeps_its_blanks_and_doubled_marks() {
        assert_eq!(
            tokens("T'a ''b'' c'P"),
            [Token::Letter(b'T'), Token::Quoted(b"'a ''b'' c'".to_vec()), Token::Letter(b'P')]
        );
    }

    #[test]
    fn a_macrograph_definition_and_a_load_command_are_kept_whole() {
        assert_eq!(tokens("@:A v[1] @;@A"), [Token::Raw(b"@:A v[1] @;".to_vec()), Token::Raw(b"@A".to_vec())]);
        assert_eq!(tokens("L(A1)\"B\"AA,55;P"), [Token::Raw(b"L(A1)\"B\"AA,55;".to_vec()), Token::Letter(b'P')]);
    }

    #[test]
    fn a_letter_inside_parentheses_is_not_a_load_command() {
        assert_eq!(tokens("S(L)").len(), 4);
    }
}
