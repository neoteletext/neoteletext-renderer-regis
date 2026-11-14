use alloc::vec::Vec;

use super::Lexer;
use crate::lexer::{DigitMode, Token};

/// Lexes a string with one digit mode throughout.
fn lex(input: &[u8], mode: DigitMode) -> Vec<Token> {
    let mut lexer = Lexer::default();
    let mut tokens = Vec::new();
    for &byte in input {
        loop {
            let step = lexer.push(byte, mode);
            tokens.extend(step.token);
            if step.consumed {
                break;
            }
        }
    }
    tokens.extend(lexer.finish());
    tokens
}

fn number_values(tokens: &[Token]) -> Vec<i32> {
    tokens.iter().filter_map(|token| if let Token::Number(n) = token { Some(n.value()) } else { None }).collect()
}

#[test]
fn letters_fold_to_upper_case_and_punctuation_is_tokenised() {
    let tokens = lex(b"s(e);", DigitMode::Number);
    assert_eq!(tokens, [Token::Letter(b'S'), Token::Open, Token::Letter(b'E'), Token::Close, Token::Resync]);
}

#[test]
fn bare_digits_are_separate_pixel_vectors() {
    assert_eq!(lex(b"642", DigitMode::Bare), [Token::Digit(6), Token::Digit(4), Token::Digit(2)]);
}

#[test]
fn a_number_ends_at_the_next_byte_which_is_then_lexed() {
    let tokens = lex(b"I0,F3)", DigitMode::Number);
    assert_eq!(number_values(&tokens), [0, 3]);
    assert_eq!(tokens.last(), Some(&Token::Close));
}

#[test]
fn an_e_with_no_digit_after_it_is_a_letter_not_an_exponent() {
    let tokens = lex(b"3E)", DigitMode::Number);
    assert_eq!(number_values(&tokens), [3]);
    assert_eq!(tokens.get(1..), Some(&[Token::Letter(b'E'), Token::Close][..]));
    // A sign with no digits after it is no number and yields nothing.
    let tokens = lex(b"3E+)", DigitMode::Number);
    assert_eq!(tokens.get(1..), Some(&[Token::Letter(b'E'), Token::Close][..]));
}

#[test]
fn doubled_quotes_are_literal_and_either_quote_mark_works() {
    assert_eq!(lex(b"'don''t'", DigitMode::Bare), [Token::Text(b"don't".to_vec())]);
    assert_eq!(lex(b"\"ditto (\"\")\"", DigitMode::Bare), [Token::Text(b"ditto (\")".to_vec())]);
    assert_eq!(lex(b"\"a;@b\"", DigitMode::Bare), [Token::Text(b"a;@b".to_vec())]);
}

#[test]
fn hex_mode_reads_letters_a_to_f_as_cell_data() {
    let tokens = lex(b"3E,ff G", DigitMode::Hex);
    let expected = [
        Token::Hex(3),
        Token::Hex(14),
        Token::Comma,
        Token::Hex(15),
        Token::Hex(15),
        Token::Comma,
        Token::Letter(b'G'),
    ];
    assert_eq!(tokens, expected);
}

#[test]
fn a_semicolon_inside_brackets_resynchronises() {
    assert_eq!(lex(b"[10,2;P", DigitMode::Bare), [Token::Resync, Token::Letter(b'P')]);
}

#[test]
fn strings_and_extents_are_bounded() {
    let mut input = Vec::from(*b"'");
    input.resize(5000, b'x');
    input.push(b'\'');
    let tokens = lex(&input, DigitMode::Bare);
    assert!(matches!(tokens.first(), Some(Token::Text(text)) if text.len() == crate::limits::MAX_STRING_BYTES));
}
