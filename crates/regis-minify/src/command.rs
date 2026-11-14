//! Grouping tokens into commands.

use crate::token::Token;

/// A command letter and everything that belongs to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Command {
    pub(crate) letter: u8,
    pub(crate) arguments: Vec<Token>,
}

/// One piece of a command string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Item {
    Command(Command),
    /// A token outside any command: a semicolon, a macrograph operation, a
    /// load command, or something stray.
    Loose(Token),
}

impl Item {
    pub(crate) fn write(&self, out: &mut Vec<u8>) {
        match self {
            Self::Command(command) => {
                out.push(command.letter);
                for token in &command.arguments {
                    token.write(out);
                }
            }
            Self::Loose(token) => token.write(out),
        }
    }
}

/// Groups tokens: a letter outside parentheses starts a command, which
/// runs until the next such letter, a semicolon or a raw token.
pub(crate) fn group(tokens: impl Iterator<Item = Token>) -> Vec<Item> {
    let mut items = Vec::new();
    let mut current: Option<Command> = None;
    let mut depth = 0_u32;
    for token in tokens {
        let ends_command = depth == 0 && matches!(token, Token::Letter(_) | Token::Semicolon | Token::Raw(_));
        if ends_command && let Some(command) = current.take() {
            items.push(Item::Command(command));
        }
        match (&token, &mut current) {
            (Token::Letter(letter), None) if depth == 0 => {
                current = Some(Command { letter: *letter, arguments: Vec::new() });
            }
            (_, Some(command)) => {
                match token {
                    Token::Open => depth = depth.saturating_add(1),
                    Token::Close => depth = depth.saturating_sub(1),
                    _ => {}
                }
                command.arguments.push(token);
            }
            (_, None) => items.push(Item::Loose(token)),
        }
    }
    items.extend(current.map(Item::Command));
    items
}

#[cfg(test)]
mod tests {
    use super::{Item, group};
    use crate::token::Lexer;

    fn letters(input: &str) -> Vec<Option<u8>> {
        group(Lexer::new(input.as_bytes()))
            .iter()
            .map(|item| match item {
                Item::Command(command) => Some(command.letter),
                Item::Loose(_) => None,
            })
            .collect()
    }

    #[test]
    fn letters_inside_parentheses_are_options_not_commands() {
        assert_eq!(letters("W(I3,P1)P[1,2]V[3]"), [Some(b'W'), Some(b'P'), Some(b'V')]);
        assert_eq!(letters("F(W(R)V[1][2])C[+3]"), [Some(b'F'), Some(b'C')]);
    }

    #[test]
    fn semicolons_and_macrographs_stand_alone() {
        assert_eq!(letters("P[1];@AV[2]"), [Some(b'P'), None, None, Some(b'V')]);
    }
}
