//! A picture to encode: colour-map addresses and its size.

/// One page's pixels.
pub(crate) struct Picture<'a> {
    /// One byte a pixel, row by row.
    pub(crate) pixels: &'a [u8],
    pub(crate) width: u32,
    pub(crate) height: u32,
}

impl Picture<'_> {
    /// The rows of the picture, top first.
    pub(crate) fn rows(&self) -> impl Iterator<Item = &[u8]> {
        let width = usize::try_from(self.width).unwrap_or(1).max(1);
        self.pixels.chunks_exact(width)
    }
}
