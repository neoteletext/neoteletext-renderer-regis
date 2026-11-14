//! Points and the eight pixel-vector directions.

/// A position, in user coordinates or in pixels as its holder says.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub(crate) struct Point {
    pub(crate) x: i32,
    pub(crate) y: i32,
}

impl Point {
    pub(crate) const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    /// This point moved by `steps` of `direction`.
    pub(crate) fn stepped(self, direction: (i32, i32), steps: i32) -> Self {
        Self {
            x: self.x.saturating_add(direction.0.saturating_mul(steps)),
            y: self.y.saturating_add(direction.1.saturating_mul(steps)),
        }
    }
}

/// How many pixel-vector directions there are (manual chapter 1, figure 1-2).
pub(crate) const DIRECTIONS: i32 = 8;

/// The screen step of each pixel-vector direction: 0 is right, and each
/// number turns 45 degrees counterclockwise; y grows downward.
const STEPS: [(i32, i32); 8] = [(1, 0), (1, -1), (0, -1), (-1, -1), (-1, 0), (-1, 1), (0, 1), (1, 1)];

/// The unit step of a direction; any integer is taken modulo eight.
pub(crate) fn direction(number: i32) -> (i32, i32) {
    let index = usize::try_from(number.rem_euclid(DIRECTIONS)).unwrap_or(0);
    STEPS.get(index).copied().unwrap_or((1, 0))
}

/// The direction nearest an angle in degrees (45 degrees to a direction).
pub(crate) fn direction_of_angle(degrees: i32) -> i32 {
    const DEGREES_PER_DIRECTION: i32 = 45;
    const HALF: i32 = 22;
    degrees.rem_euclid(360).saturating_add(HALF).div_euclid(DEGREES_PER_DIRECTION).rem_euclid(DIRECTIONS)
}
