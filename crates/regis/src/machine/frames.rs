//! Where in the nested parentheses the interpreter stands.

/// The commands, by key letter (manual chapter 1, table 1-1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Command {
    Screen,
    Write,
    Position,
    Vector,
    Curve,
    Text,
    Load,
    Report,
    Fill,
}

impl Command {
    pub(crate) const fn of(letter: u8) -> Option<Self> {
        Some(match letter {
            b'S' => Self::Screen,
            b'W' => Self::Write,
            b'P' => Self::Position,
            b'V' => Self::Vector,
            b'C' => Self::Curve,
            b'T' => Self::Text,
            b'L' => Self::Load,
            b'R' => Self::Report,
            b'F' => Self::Fill,
            _ => return None,
        })
    }

    /// The commands a polygon fill may hold (manual chapter 11).
    pub(crate) const fn allowed_in_fill(self) -> bool {
        matches!(self, Self::Vector | Self::Curve | Self::Position | Self::Write)
    }
}

/// Whose colour a colour specifier sets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ColourTarget {
    /// `S(I(...))`
    Background,
    /// `W(I(...))`, for the lasting or the temporary write controls.
    Foreground { temporary: bool },
    /// `S(M n(...))`
    Map,
}

/// What an opening parenthesis began.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Context {
    /// The options of a command.
    Options(Command),
    /// Write controls used as an option of another command.
    TemporaryWrite,
    /// The body of a polygon fill, which holds commands.
    Fill,
    /// A colour: letters and HLS numbers.
    Colour(ColourTarget),
    /// `W(P n(M n))`
    PatternMultiplier { temporary: bool },
    /// `W(S(X))`
    ShadeAxis,
    /// `T(A0(L"..." R"..."))`
    Designators,
    /// `R(M(...))`
    ReportMacrograph,
    /// `R(P(I))`
    ReportPosition,
    /// Something not understood: skipped to its closing parenthesis.
    Skip,
}

/// One level of parentheses.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Frame {
    pub(crate) context: Context,
    /// The option letter that numbers, extents and strings now belong to.
    pub(crate) option: u8,
}
