//! NeoTeletext's provided character sets as alphabets 100 to 154, and the
//! joined letters of the plain Arabic-script sets (`docs/regis-v2.md`,
//! "Provided character sets"; renderer-parity.md PAR-12 and PAR-13;
//! REQ-6764).

mod common;

use regis::limits::{FIRST_PROVIDED_ALPHABET, PROVIDED_SETS};
use regis::{Error, JoinedForm, JoiningType, ProvidedSet, Screen, Terminal};

/// The engine's glyph cell, which the provided sets are made in.
const GLYPH_WIDTH: u8 = 12;
const GLYPH_HEIGHT: u8 = 20;
/// The default text cell: display 9 by 20, unit 8 by 20 (manual table 1-2).
const DISPLAY_WIDTH: u32 = 9;
const UNIT_WIDTH: u32 = 8;
const UNIT_HEIGHT: u32 = 20;
/// The characters drawn on one line of the test: 48 × 9 = 432 pixels.
const PER_LINE: usize = 48;
const LINE_STEP: u32 = 40;
const TWELVE_BITS: u16 = 0x0FFF;

/// A glyph unlike every other: rows from a hash of the set, code and form.
fn glyph(set: u8, code: u8, form: u8) -> Vec<u16> {
    (0..GLYPH_HEIGHT)
        .map(|row| {
            let mut hash = u32::from(set) << 24 | u32::from(code) << 16 | u32::from(form) << 8 | u32::from(row);
            hash = hash.wrapping_mul(0x9E37_79B1);
            hash ^= hash >> 15;
            u16::try_from(hash & u32::from(TWELVE_BITS)).unwrap()
        })
        .collect()
}

fn provided_set(number: u8) -> ProvidedSet {
    let mut set = ProvidedSet::new(GLYPH_WIDTH, GLYPH_HEIGHT).unwrap();
    for code in 0x20..=0x7F {
        set.set_glyph(code, &glyph(number, code, 0)).unwrap();
    }
    set
}

/// The pixel (`u`, `v`) of a glyph scaled into the default unit cell: the
/// built-in strikes' rule, each unit pixel taking the glyph pixel its share
/// of the cell starts in.
fn lit(rows: &[u16], u: u32, v: u32) -> bool {
    let column = u * u32::from(GLYPH_WIDTH) / UNIT_WIDTH;
    let row = v * u32::from(GLYPH_HEIGHT) / UNIT_HEIGHT;
    rows[usize::try_from(row).unwrap()] >> (u32::from(GLYPH_WIDTH) - 1 - column) & 1 == 1
}

/// Whether the cell at `(x, y)` shows exactly `rows`, scaled.
fn shows(screen: &Screen, x: u32, y: u32, rows: &[u16]) -> bool {
    (0..UNIT_HEIGHT)
        .all(|v| (0..UNIT_WIDTH).all(|u| (screen.displayed().pixel(x + u, y + v).unwrap() != 0) == lit(rows, u, v)))
}

fn blank(screen: &Screen, x: u32, y: u32) -> bool {
    (0..UNIT_HEIGHT).all(|v| (0..DISPLAY_WIDTH).all(|u| screen.displayed().pixel(x + u, y + v).unwrap() == 0))
}

/// A string of codes quoted for ReGIS, a quote mark written twice.
fn quoted(codes: &[u8]) -> Vec<u8> {
    let mut out = vec![b'\''];
    for &code in codes {
        out.push(code);
        if code == b'\'' {
            out.push(code);
        }
    }
    out.push(b'\'');
    out
}

fn draw(terminal: &mut Terminal, commands: &[u8]) -> Screen {
    let mut screen = Screen::new();
    let _ = terminal.feed_regis(&mut screen, commands);
    let _ = terminal.finish(&mut screen);
    screen
}

#[test]
fn every_code_of_every_provided_alphabet_draws_its_glyph_scaled_into_the_default_cell() {
    let mut terminal = common::terminal();
    for number in 0..PROVIDED_SETS {
        terminal.set_provided_set(number, provided_set(number)).unwrap();
    }
    let codes: Vec<u8> = (0x20..=0x7E).collect();
    for number in 0..PROVIDED_SETS {
        let mut commands = format!("S(E)T(A{})", u32::from(FIRST_PROVIDED_ALPHABET) + u32::from(number)).into_bytes();
        for (line, chunk) in codes.chunks(PER_LINE).enumerate() {
            commands.extend(format!("P[0,{}]T", u32::try_from(line).unwrap() * LINE_STEP).bytes());
            commands.extend(quoted(chunk));
        }
        let mut screen = Screen::new();
        let _ = terminal.feed_regis(&mut screen, &commands);
        let _ = terminal.finish(&mut screen);
        for (index, &code) in codes.iter().enumerate() {
            let (line, column) = (index / PER_LINE, index % PER_LINE);
            let (x, y) = (u32::try_from(column).unwrap() * DISPLAY_WIDTH, u32::try_from(line).unwrap() * LINE_STEP);
            assert!(shows(&screen, x, y, &glyph(number, code, 0)), "set {number}, code {code:#04x}");
        }
    }
}

#[test]
fn a_doubled_quote_draws_one_quote_mark() {
    let mut terminal = common::terminal();
    terminal.set_provided_set(0, provided_set(0)).unwrap();
    let screen = draw(&mut terminal, b"T(A100)P[0,0]T\"\"\"\"P[0,100]T'A'");
    assert!(shows(&screen, 0, 0, &glyph(0, b'"', 0)));
    assert!(blank(&screen, DISPLAY_WIDTH, 0), "one character, not two");
    assert!(shows(&screen, 0, 100, &glyph(0, b'A', 0)));
}

#[test]
fn a_byte_a_set_does_not_reach_draws_an_empty_cell_and_advances() {
    let mut terminal = common::terminal();
    terminal.set_provided_set(0, provided_set(0)).unwrap();
    // An eight-bit character is kept in a string and draws the empty cell.
    // DEL and the controls other than the four text controls never reach
    // a string: the string leaves them out (manual chapter 7, "Control
    // Characters in Text Strings"), so they take no cell.
    let screen = draw(&mut terminal, b"T(A100)P[0,0]T'\xC1\x7F\x01\xFFA'");
    assert!(blank(&screen, 0, 0) && blank(&screen, DISPLAY_WIDTH, 0));
    assert!(shows(&screen, 2 * DISPLAY_WIDTH, 0, &glyph(0, b'A', 0)));
}

#[test]
fn a_provided_alphabet_never_given_draws_empty_cells() {
    let mut terminal = common::terminal();
    let screen = draw(&mut terminal, b"T(A117)P[0,0]T'AB'");
    assert!(blank(&screen, 0, 0) && blank(&screen, DISPLAY_WIDTH, 0));
}

#[test]
fn alphabets_outside_the_provided_ones_stay_ignored() {
    let mut terminal = common::terminal();
    terminal.set_provided_set(0, provided_set(0)).unwrap();
    // 155 and 99 are ignored: alphabet 100 stays in force.
    let screen = draw(&mut terminal, b"T(A100)T(A155)T(A99)T(A4)P[0,0]T'A'");
    assert!(shows(&screen, 0, 0, &glyph(0, b'A', 0)));
    // And the built-in font comes back with alphabet 0.
    let built_in = draw(&mut common::terminal(), b"P[0,0]T'A'");
    let screen = draw(&mut terminal, b"T(A100)T(A0)P[0,0]T'A'");
    assert_eq!(screen.displayed().pixels(), built_in.displayed().pixels());
}

#[test]
fn the_load_command_refuses_a_provided_alphabet() {
    let mut terminal = common::terminal();
    terminal.set_provided_set(0, provided_set(0)).unwrap();
    // The load is refused and alphabet 1 stays the one being loaded, so
    // its cell is loaded there and not into the provided set.
    let screen = draw(&mut terminal, b"L(A100)'A'FF,FF,FF,FF,FF,FF,FF,FF,FF,FF;T(A100)P[0,0]T'A'");
    assert!(shows(&screen, 0, 0, &glyph(0, b'A', 0)));
    let loaded = draw(&mut terminal, b"T(A1)P[0,0]T'A'");
    assert_ne!(loaded.displayed().pixel(0, 0), Some(0), "the cell went to alphabet 1");
}

#[test]
fn a_set_number_past_the_provided_sets_is_refused() {
    let mut terminal = Terminal::new();
    assert_eq!(terminal.set_provided_set(PROVIDED_SETS, provided_set(0)), Err(Error::ProvidedSetNumber));
}

/// Set 44, `G0-Persian`, as the engine's code table has it.
const PERSIAN: u8 = 44;
const ALEF: u8 = 0x46;
const SEEN: u8 = 0x54;
const LAM: u8 = 0x60;
const MEEM: u8 = 0x61;
/// The form numbers [`glyph`] is given: 0 the isolated glyph.
const FORMS: [(JoinedForm, u8); 3] = [(JoinedForm::Final, 1), (JoinedForm::Initial, 2), (JoinedForm::Medial, 3)];

/// Set 44 with the joining types of the engine's table for the letters
/// used here, and every form a glyph of its own.
fn persian() -> ProvidedSet {
    let mut set = provided_set(PERSIAN);
    for (code, kind) in
        [(ALEF, JoiningType::Right), (SEEN, JoiningType::Dual), (LAM, JoiningType::Dual), (MEEM, JoiningType::Dual)]
    {
        set.set_joining(code, kind).unwrap();
        for (form, number) in FORMS {
            set.set_form(code, form, &glyph(PERSIAN, code, number)).unwrap();
        }
    }
    set
}

fn form_number(form: JoinedForm) -> u8 {
    FORMS.iter().find(|(each, _)| *each == form).unwrap().1
}

#[test]
fn a_persian_word_draws_the_forms_the_teletext_renderer_draws() {
    let mut terminal = common::terminal();
    terminal.set_provided_set(PERSIAN, persian()).unwrap();
    // سلام in visual order, as the engine's test `a_word_joins_right_to_left`
    // lays it out: meem, alef, lam, seen. The engine gives them isolated,
    // final, medial and initial.
    let mut commands = b"T(A144)P[0,0]T".to_vec();
    commands.extend(quoted(&[MEEM, ALEF, LAM, SEEN]));
    let screen = draw(&mut terminal, &commands);
    let expected = [
        (MEEM, 0),
        (ALEF, form_number(JoinedForm::Final)),
        (LAM, form_number(JoinedForm::Medial)),
        (SEEN, form_number(JoinedForm::Initial)),
    ];
    for (index, (code, form)) in expected.into_iter().enumerate() {
        let x = u32::try_from(index).unwrap() * DISPLAY_WIDTH;
        assert!(shows(&screen, x, 0, &glyph(PERSIAN, code, form)), "letter {index}");
    }
}

#[test]
fn letters_do_not_join_across_a_change_of_alphabet() {
    let mut terminal = common::terminal();
    terminal.set_provided_set(PERSIAN, persian()).unwrap();
    terminal.set_provided_set(0, provided_set(0)).unwrap();
    // Lam and seen would join; a change of alphabet between them, even back
    // to the same one, ends the string and the joining.
    let screen = draw(&mut terminal, b"T(A144)P[0,0]T'`'T(A100)T(A144)T'T'");
    assert!(shows(&screen, 0, 0, &glyph(PERSIAN, LAM, 0)));
    assert!(shows(&screen, DISPLAY_WIDTH, 0, &glyph(PERSIAN, SEEN, 0)));
    // In one string they join: seen initial on the right, lam final.
    let screen = draw(&mut terminal, b"T(A144)P[0,0]T'`T'");
    assert!(shows(&screen, 0, 0, &glyph(PERSIAN, LAM, form_number(JoinedForm::Final))));
    assert!(shows(&screen, DISPLAY_WIDTH, 0, &glyph(PERSIAN, SEEN, form_number(JoinedForm::Initial))));
}

#[test]
fn a_set_not_marked_as_joined_draws_every_letter_isolated() {
    let mut terminal = common::terminal();
    terminal.set_provided_set(PERSIAN, provided_set(PERSIAN)).unwrap();
    let screen = draw(&mut terminal, b"T(A144)P[0,0]T'`T'");
    assert!(shows(&screen, 0, 0, &glyph(PERSIAN, LAM, 0)));
    assert!(shows(&screen, DISPLAY_WIDTH, 0, &glyph(PERSIAN, SEEN, 0)));
}

#[test]
fn numbered_drawings_draw_provided_alphabets_too() {
    let mut terminal = common::terminal();
    terminal.set_provided_set(PERSIAN, persian()).unwrap();
    let mut base = Screen::new();
    let _ = terminal.feed_regis(&mut base, b"D(B1)T(A144)P[0,0]T'`T'D(E)D(S1(X0,Y0),C)");
    let _ = terminal.finish(&mut base);
    let mut picture = regis::Picture::new();
    terminal.compose(&base, &mut picture);
    let canvas = picture.canvas();
    let lit_at = |x: u32, y: u32| canvas.pixel(x, y).unwrap() != 0;
    let rows = glyph(PERSIAN, LAM, form_number(JoinedForm::Final));
    assert!((0..UNIT_HEIGHT).all(|v| (0..UNIT_WIDTH).all(|u| lit_at(u, v) == lit(&rows, u, v))));
}
