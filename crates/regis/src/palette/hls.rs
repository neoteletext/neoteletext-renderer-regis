//! Hue, lightness and saturation to red, green and blue, in integers.
#![allow(
    clippy::arithmetic_side_effects,
    reason = "inputs are clamped to 0..=360 and 0..=100 first, so every product is below 2^20"
)]

use super::{FULL, Percent};

/// DEC puts blue at hue 0, red at 120 and green at 240 (manual chapter 2,
/// "HLS System"); the usual colour wheel puts red at 0, so DEC hues are
/// turned by this much to reach it.
const DEC_HUE_OFFSET: i32 = 240;
const CIRCLE: i32 = 360;
const SECTOR: i32 = 60;

/// The colour of a hue in degrees and a lightness and saturation in percent.
pub(crate) fn hls_to_rgb(hue: i32, lightness: i32, saturation: i32) -> Percent {
    let hue = (hue.rem_euclid(CIRCLE) + DEC_HUE_OFFSET) % CIRCLE;
    let lightness = lightness.clamp(0, FULL);
    let saturation = saturation.clamp(0, FULL);

    let chroma = (FULL - (2 * lightness - FULL).abs()) * saturation;
    let second = chroma * (SECTOR - (hue % (2 * SECTOR) - SECTOR).abs()) / SECTOR;
    let floor = lightness * FULL - chroma / 2;
    let (red, green, blue) = match hue / SECTOR {
        0 => (chroma, second, 0),
        1 => (second, chroma, 0),
        2 => (0, chroma, second),
        3 => (0, second, chroma),
        4 => (second, 0, chroma),
        _ => (chroma, 0, second),
    };
    [red, green, blue].map(|part| {
        let percent = (part + floor + FULL / 2) / FULL;
        u8::try_from(percent.clamp(0, FULL)).unwrap_or(0)
    })
}

#[cfg(test)]
mod tests {
    use super::hls_to_rgb;

    #[test]
    fn the_primary_hues_are_blue_red_and_green() {
        assert_eq!(hls_to_rgb(0, 50, 100), [0, 0, 100]);
        assert_eq!(hls_to_rgb(120, 50, 100), [100, 0, 0]);
        assert_eq!(hls_to_rgb(240, 50, 100), [0, 100, 0]);
    }

    #[test]
    fn the_default_map_values_convert_as_the_manual_tabulates() {
        // Manual table 2-3: location 1 is H0 L50 S60 = RGB 20, 20, 80.
        assert_eq!(hls_to_rgb(0, 50, 60), [20, 20, 80]);
        assert_eq!(hls_to_rgb(180, 50, 60), [80, 80, 20]);
        assert_eq!(hls_to_rgb(0, 53, 0), [53, 53, 53]);
    }

    #[test]
    fn out_of_range_values_are_clamped_not_wrapped() {
        assert_eq!(hls_to_rgb(0, 500, 0), [100, 100, 100]);
        assert_eq!(hls_to_rgb(-240, 50, 100), hls_to_rgb(120, 50, 100));
    }
}
