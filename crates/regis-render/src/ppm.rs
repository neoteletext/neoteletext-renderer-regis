//! Binary Portable Pixmap ("P6") output.

use regis::Rgb;

use crate::picture::Picture;

/// The largest sample value: one byte to a sample.
const MAX_SAMPLE: u8 = 255;

/// Encodes a picture as a PPM file.
pub(crate) fn encode(picture: &Picture<'_>, colours: &[Rgb]) -> Vec<u8> {
    let mut out = format!("P6\n{} {}\n{MAX_SAMPLE}\n", picture.width, picture.height).into_bytes();
    for &pixel in picture.pixels {
        out.extend_from_slice(&colours.get(usize::from(pixel)).copied().unwrap_or_default());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::encode;
    use crate::picture::Picture;

    #[test]
    fn the_header_and_one_triple_per_pixel_are_written() {
        let picture = Picture { pixels: &[0, 1], width: 2, height: 1 };
        let bytes = encode(&picture, &[[1, 2, 3], [4, 5, 6]]);
        assert_eq!(bytes, b"P6\n2 1\n255\n\x01\x02\x03\x04\x05\x06");
    }
}
