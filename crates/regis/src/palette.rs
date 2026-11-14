//! The output map: sixteen colours the pixel values select.

mod hls;

pub(crate) use hls::hls_to_rgb;

/// A colour as red, green and blue, each 0 to 255.
pub type Rgb = [u8; 3];

/// How many locations the VT340's output map has (manual chapter 2).
pub(crate) const COLOURS: usize = 16;

/// Colour components are percentages (manual chapter 2, tables 2-2 and 2-3).
pub(crate) const FULL: i32 = 100;

/// A colour as red, green and blue percentages.
pub(crate) type Percent = [u8; 3];

/// The VT340's default colour map (manual chapter 2, table 2-3, RGB columns).
const DEFAULT: [Percent; COLOURS] = [
    [0, 0, 0],
    [20, 20, 80],
    [80, 13, 13],
    [20, 80, 20],
    [80, 20, 80],
    [20, 80, 80],
    [80, 80, 20],
    [53, 53, 53],
    [26, 26, 26],
    [33, 33, 60],
    [60, 26, 26],
    [33, 60, 33],
    [60, 33, 60],
    [33, 60, 60],
    [60, 60, 33],
    [80, 80, 80],
];

/// The output map: the colour each of the sixteen pixel values shows as.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Palette {
    entries: [Percent; COLOURS],
}

impl Default for Palette {
    fn default() -> Self {
        Self { entries: DEFAULT }
    }
}

impl Palette {
    /// All sixteen colours, by colour-map address.
    #[must_use]
    pub fn colours(&self) -> [Rgb; COLOURS] {
        self.entries.map(|entry| entry.map(to_byte))
    }

    /// One colour; addresses above 15 use their low four bits.
    #[must_use]
    pub fn colour(&self, address: u8) -> Rgb {
        let index = usize::from(address).checked_rem(COLOURS).unwrap_or(0);
        self.entries.get(index).copied().unwrap_or_default().map(to_byte)
    }

    pub(crate) fn set(&mut self, address: usize, colour: Percent) {
        if let Some(entry) = self.entries.get_mut(address) {
            *entry = colour;
        }
    }

    /// The address whose colour is closest to `colour` (manual chapter 3,
    /// "Foreground Intensity": "uses the closest match").
    pub(crate) fn nearest(&self, colour: Percent) -> u8 {
        let distance = |entry: &Percent| -> i32 {
            entry
                .iter()
                .zip(colour)
                .map(|(have, want)| {
                    let difference = i32::from(*have).saturating_sub(i32::from(want));
                    difference.saturating_mul(difference)
                })
                .fold(0, i32::saturating_add)
        };
        let best = self.entries.iter().enumerate().min_by_key(|(_, entry)| distance(entry));
        best.and_then(|(index, _)| u8::try_from(index).ok()).unwrap_or(0)
    }
}

/// A percentage as a byte, rounded: 100 percent is 255.
fn to_byte(percent: u8) -> u8 {
    const BYTE_FULL: u32 = 255;
    const PERCENT_FULL: u32 = 100;
    const HALF: u32 = 50;
    let scaled = u32::from(percent.min(100)).saturating_mul(BYTE_FULL).saturating_add(HALF);
    u8::try_from(scaled.checked_div(PERCENT_FULL).unwrap_or(0)).unwrap_or(u8::MAX)
}

/// The colour a single letter names (manual chapter 2, "RGB System").
pub(crate) fn named(letter: u8) -> Option<Percent> {
    let colour = match letter {
        b'D' => [0, 0, 0],
        b'R' => [100, 0, 0],
        b'G' => [0, 100, 0],
        b'B' => [0, 0, 100],
        b'C' => [0, 100, 100],
        b'Y' => [100, 100, 0],
        b'M' => [100, 0, 100],
        b'W' => [100, 100, 100],
        _ => return None,
    };
    Some(colour)
}
