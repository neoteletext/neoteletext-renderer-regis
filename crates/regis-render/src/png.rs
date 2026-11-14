//! PNG output: an indexed-colour picture with a palette.
//!
//! The pixel data is stored, not compressed (RFC 1951 section 3.2.4), so no
//! compression library is needed; a page is 384,000 bytes.

use regis::Rgb;

use crate::checksum::{adler32, crc32};
use crate::picture::Picture;

/// PNG specification section 5.2.
const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1A, b'\n'];
/// IHDR fields (PNG specification section 11.2.2): eight bits a pixel,
/// colour type 3 (palette), and no interlace.
const BIT_DEPTH: u8 = 8;
const COLOUR_TYPE_PALETTE: u8 = 3;
/// A stored deflate block holds at most 65,535 bytes.
const STORED_BLOCK: usize = 65_535;
/// zlib header for deflate with a 32K window and no preset dictionary
/// (RFC 1950 section 2.2); the pair is a multiple of 31 as required.
const ZLIB_HEADER: [u8; 2] = [0x78, 0x01];
/// Filter type 0, "None", begins each scanline (PNG specification section 9.2).
const FILTER_NONE: u8 = 0;

fn chunk(out: &mut Vec<u8>, kind: [u8; 4], data: &[u8]) {
    out.extend_from_slice(&u32::try_from(data.len()).unwrap_or(u32::MAX).to_be_bytes());
    out.extend_from_slice(&kind);
    out.extend_from_slice(data);
    out.extend_from_slice(&crc32(&[&kind, data]).to_be_bytes());
}

/// The scanlines wrapped as a zlib stream of stored blocks.
fn zlib_stored(raw: &[u8]) -> Vec<u8> {
    let mut out = Vec::from(ZLIB_HEADER);
    let mut blocks = raw.chunks(STORED_BLOCK).peekable();
    while let Some(block) = blocks.next() {
        let length = u16::try_from(block.len()).unwrap_or(u16::MAX);
        out.push(u8::from(blocks.peek().is_none()));
        out.extend_from_slice(&length.to_le_bytes());
        out.extend_from_slice(&(!length).to_le_bytes());
        out.extend_from_slice(block);
    }
    out.extend_from_slice(&adler32(raw).to_be_bytes());
    out
}

/// Encodes a picture as a PNG file.
pub(crate) fn encode(picture: &Picture<'_>, colours: &[Rgb]) -> Vec<u8> {
    let mut header = Vec::new();
    header.extend_from_slice(&picture.width.to_be_bytes());
    header.extend_from_slice(&picture.height.to_be_bytes());
    header.extend_from_slice(&[BIT_DEPTH, COLOUR_TYPE_PALETTE, 0, 0, 0]);

    let mut raw = Vec::with_capacity(picture.pixels.len().saturating_add(picture.pixels.len() / 64));
    for row in picture.rows() {
        raw.push(FILTER_NONE);
        raw.extend_from_slice(row);
    }

    let mut out = Vec::from(SIGNATURE);
    chunk(&mut out, *b"IHDR", &header);
    chunk(&mut out, *b"PLTE", colours.as_flattened());
    chunk(&mut out, *b"IDAT", &zlib_stored(&raw));
    chunk(&mut out, *b"IEND", &[]);
    out
}

#[cfg(test)]
mod tests {
    use super::{SIGNATURE, encode};
    use crate::picture::Picture;

    #[test]
    fn a_small_picture_has_the_four_chunks_in_order() {
        let picture = Picture { pixels: &[0, 1, 1, 0], width: 2, height: 2 };
        let bytes = encode(&picture, &[[0, 0, 0], [255, 255, 255]]);
        assert!(bytes.starts_with(&SIGNATURE));
        let position = |kind: &[u8]| bytes.windows(4).position(|window| window == kind);
        let order = [position(b"IHDR"), position(b"PLTE"), position(b"IDAT"), position(b"IEND")];
        assert!(order.iter().all(Option::is_some));
        assert!(order.is_sorted());
        // IHDR: width 2, height 2, depth 8, colour type 3.
        assert_eq!(&bytes[16..26], &[0, 0, 0, 2, 0, 0, 0, 2, 8, 3]);
    }
}
