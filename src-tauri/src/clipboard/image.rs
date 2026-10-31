//! Image helpers for clipboard history: PNG encode/decode and thumbnails.

use base64::Engine;

/// An 8-bit RGBA image.
#[derive(Debug, Clone, PartialEq)]
pub struct Rgba {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

/// Largest image (in pixels) that is kept in history.
pub const MAX_PIXELS: u64 = 40_000_000;
const THUMB_W: u32 = 240;
const THUMB_H: u32 = 160;

impl Rgba {
    pub fn new(width: u32, height: u32, pixels: Vec<u8>) -> Option<Self> {
        let expected = width as u64 * height as u64 * 4;
        (width > 0 && height > 0 && pixels.len() as u64 == expected && expected / 4 <= MAX_PIXELS)
            .then_some(Self {
                width,
                height,
                pixels,
            })
    }

    pub fn to_png(&self) -> Option<Vec<u8>> {
        let mut out = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut out, self.width, self.height);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder.set_compression(png::Compression::Fast);
            let mut writer = encoder.write_header().ok()?;
            writer.write_image_data(&self.pixels).ok()?;
        }
        Some(out)
    }

    pub fn from_png(bytes: &[u8]) -> Option<Self> {
        let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
        decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
        let mut reader = decoder.read_info().ok()?;
        let (w, h) = (reader.info().width, reader.info().height);
        if w as u64 * h as u64 > MAX_PIXELS {
            return None;
        }
        let mut buf = vec![0; reader.output_buffer_size()?];
        let frame = reader.next_frame(&mut buf).ok()?;
        buf.truncate(frame.buffer_size());
        let pixels = match frame.color_type {
            png::ColorType::Rgba => buf,
            png::ColorType::Rgb => buf
                .chunks_exact(3)
                .flat_map(|p| [p[0], p[1], p[2], 255])
                .collect(),
            png::ColorType::GrayscaleAlpha => buf
                .chunks_exact(2)
                .flat_map(|p| [p[0], p[0], p[0], p[1]])
                .collect(),
            png::ColorType::Grayscale => buf.iter().flat_map(|&g| [g, g, g, 255]).collect(),
            png::ColorType::Indexed => return None,
        };
        Self::new(frame.width, frame.height, pixels)
    }

    /// Box-filtered thumbnail that fits inside `max_w` × `max_h`.
    pub fn thumbnail(&self, max_w: u32, max_h: u32) -> Rgba {
        let scale = (max_w as f64 / self.width as f64)
            .min(max_h as f64 / self.height as f64)
            .min(1.0);
        let tw = ((self.width as f64 * scale).round() as u32).max(1);
        let th = ((self.height as f64 * scale).round() as u32).max(1);
        if tw == self.width && th == self.height {
            return self.clone();
        }
        let mut out = vec![0u8; (tw * th * 4) as usize];
        for ty in 0..th {
            let y0 = (ty as u64 * self.height as u64 / th as u64) as u32;
            let y1 = (((ty + 1) as u64 * self.height as u64 / th as u64) as u32).max(y0 + 1);
            for tx in 0..tw {
                let x0 = (tx as u64 * self.width as u64 / tw as u64) as u32;
                let x1 = (((tx + 1) as u64 * self.width as u64 / tw as u64) as u32).max(x0 + 1);
                let mut acc = [0u64; 4];
                let mut n = 0u64;
                // Sample at most an 8×8 grid per output pixel to bound the cost on huge images.
                let sy = ((y1 - y0) / 8).max(1);
                let sx = ((x1 - x0) / 8).max(1);
                let mut y = y0;
                while y < y1 {
                    let mut x = x0;
                    while x < x1 {
                        let i = ((y * self.width + x) * 4) as usize;
                        for (a, &p) in acc.iter_mut().zip(&self.pixels[i..i + 4]) {
                            *a += p as u64;
                        }
                        n += 1;
                        x += sx;
                    }
                    y += sy;
                }
                let o = ((ty * tw + tx) * 4) as usize;
                for (dst, a) in out[o..o + 4].iter_mut().zip(acc) {
                    *dst = (a / n) as u8;
                }
            }
        }
        Rgba {
            width: tw,
            height: th,
            pixels: out,
        }
    }

    /// Small PNG preview as a `data:` URL.
    pub fn thumbnail_data_url(&self) -> Option<String> {
        let png = self.thumbnail(THUMB_W, THUMB_H).to_png()?;
        Some(format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(png)
        ))
    }
}

/// Stable 64-bit FNV-1a hash, used to recognise repeated copies across restarts.
pub fn fnv1a(parts: &[&[u8]]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for part in parts {
        for &b in *part {
            h ^= b as u64;
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
        // Separator so ("ab","c") and ("a","bc") differ.
        h ^= 0xff;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{h:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checker(w: u32, h: u32) -> Rgba {
        let pixels = (0..w * h)
            .flat_map(|i| {
                if (i % w + i / w) % 2 == 0 {
                    [255, 0, 0, 255]
                } else {
                    [0, 0, 255, 255]
                }
            })
            .collect();
        Rgba::new(w, h, pixels).unwrap()
    }

    #[test]
    fn png_round_trip() {
        let img = checker(7, 5);
        let back = Rgba::from_png(&img.to_png().unwrap()).unwrap();
        assert_eq!(back, img);
    }

    #[test]
    fn thumbnail_fits_and_keeps_aspect() {
        let t = checker(1000, 500).thumbnail(240, 160);
        assert_eq!((t.width, t.height), (240, 120));
        let small = checker(10, 10).thumbnail(240, 160);
        assert_eq!((small.width, small.height), (10, 10));
    }

    #[test]
    fn rejects_bad_buffers() {
        assert!(Rgba::new(2, 2, vec![0; 15]).is_none());
        assert!(Rgba::new(0, 2, vec![]).is_none());
        assert!(Rgba::from_png(b"not a png").is_none());
    }

    #[test]
    fn hash_is_stable_and_separates_parts() {
        assert_eq!(fnv1a(&[b"hello"]), fnv1a(&[b"hello"]));
        assert_ne!(fnv1a(&[b"ab", b"c"]), fnv1a(&[b"a", b"bc"]));
        assert_eq!(fnv1a(&[b""]).len(), 16);
    }
}
