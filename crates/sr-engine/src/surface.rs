//! The indexed 8-bit canvas the demo drew into.

/// Largest surface the engine will allocate, in pixels.
pub const MAX_WIDTH: usize = 640;
/// Largest surface the engine will allocate, in pixels.
pub const MAX_HEIGHT: usize = 400;

/// An indexed 8-bit image buffer, stored as a flat row-major run of pixels.
pub struct Surface {
    width: usize,
    height: usize,
    pixels: Vec<u8>,
}

impl Surface {
    /// Allocates a `width` × `height` surface, zeroed.
    pub fn new(width: usize, height: usize) -> Surface {
        Surface {
            width,
            height,
            pixels: vec![0; width * height],
        }
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    pub fn pixels_mut(&mut self) -> &mut [u8] {
        &mut self.pixels
    }

    /// `None` when out of bounds.
    pub fn get(&self, x: usize, y: usize) -> Option<u8> {
        if x < self.width && y < self.height {
            Some(self.pixels[y * self.width + x])
        } else {
            None
        }
    }

    /// Out-of-bounds writes are ignored. Never panics, never wraps to another row.
    pub fn set(&mut self, x: usize, y: usize, v: u8) {
        if x < self.width && y < self.height {
            self.pixels[y * self.width + x] = v;
        }
    }

    pub fn clear(&mut self, v: u8) {
        self.pixels.fill(v);
    }

    pub fn row(&self, y: usize) -> &[u8] {
        let start = y * self.width;
        &self.pixels[start..start + self.width]
    }

    pub fn row_mut(&mut self, y: usize) -> &mut [u8] {
        let start = y * self.width;
        &mut self.pixels[start..start + self.width]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_and_get_round_trip() {
        let mut s = Surface::new(4, 2);
        s.set(2, 1, 42);
        assert_eq!(s.get(2, 1), Some(42));
        assert_eq!(s.get(0, 0), Some(0));
    }

    #[test]
    fn set_beyond_width_is_ignored_and_does_not_wrap() {
        let mut s = Surface::new(4, 2);
        s.set(4, 0, 42);
        s.set(999, 0, 43);
        assert_eq!(s.row(1), &[0, 0, 0, 0]);
        assert_eq!(s.get(4, 0), None);
    }

    #[test]
    fn set_beyond_height_is_ignored() {
        let mut s = Surface::new(4, 2);
        s.set(0, 2, 42);
        s.set(0, 999, 43);
        assert_eq!(s.row(1), &[0, 0, 0, 0]);
    }

    #[test]
    fn clear_fills_every_pixel() {
        let mut s = Surface::new(3, 2);
        s.clear(7);
        assert_eq!(s.pixels(), &[7; 6]);
    }

    #[test]
    fn row_mut_does_not_touch_neighbours() {
        let mut s = Surface::new(3, 2);
        s.row_mut(0).fill(9);
        assert_eq!(s.row(0), &[9, 9, 9]);
        assert_eq!(s.row(1), &[0, 0, 0]);
    }

    #[test]
    fn new_is_zeroed() {
        assert_eq!(Surface::new(MAX_WIDTH, MAX_HEIGHT).pixels().len(), 256_000);
    }
}
