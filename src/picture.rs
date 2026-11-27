//! Pictures put in relationship diagram nodes: read from a file, made small enough to keep in
//! the project (at most `MAX_SIDE` pixels on the longer side), and kept as base64 PNG text in
//! `Shape::image`, so a project file carries its pictures.

use std::io::Cursor;
use std::path::Path;

/// The longest side a picture is kept at (pixels).
pub const MAX_SIDE: u32 = 320;

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Standard base64 with padding.
pub fn encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = (b[0] as u32) << 16 | (b[1] as u32) << 8 | b[2] as u32;
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(ALPHABET[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// Base64 back to bytes (whitespace is skipped); None when it isn't base64.
pub fn decode(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(text.len() / 4 * 3);
    let (mut acc, mut bits) = (0u32, 0u32);
    for c in text.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' => break,
            b' ' | b'\n' | b'\r' | b'\t' => continue,
            _ => return None,
        };
        acc = acc << 6 | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits & 0xFF) as u8);
        }
    }
    Some(out)
}

/// A picture as kept in a project: shrunk to `MAX_SIDE`, PNG, base64.
pub fn from_image(img: image::DynamicImage) -> Result<String, String> {
    let img = if img.width() > MAX_SIDE || img.height() > MAX_SIDE { img.thumbnail(MAX_SIDE, MAX_SIDE) } else { img };
    let mut png = Vec::new();
    image::DynamicImage::ImageRgba8(img.to_rgba8()).write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png).map_err(|e| e.to_string())?;
    Ok(encode(&png))
}

/// Read a picture file (PNG, JPEG or WebP) for a node.
pub fn from_file(path: &Path) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let img = image::load_from_memory(&bytes).map_err(|e| e.to_string())?;
    from_image(img)
}

/// A kept picture's pixels (RGBA) and size, or None when it can't be read.
pub fn pixels(data: &str) -> Option<([usize; 2], Vec<u8>)> {
    let img = image::load_from_memory(&decode(data)?).ok()?.to_rgba8();
    Some(([img.width() as usize, img.height() as usize], img.into_raw()))
}

/// A simple drawn portrait (a head and shoulders on a coloured ground), for the example project.
pub fn portrait(ground: [u8; 3], hair: [u8; 3], shirt: [u8; 3]) -> String {
    let n = 96u32;
    let skin = [240u8, 205, 175];
    let img = image::RgbaImage::from_fn(n, n, |x, y| {
        let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
        let head = ((fx - 48.0) / 17.0).powi(2) + ((fy - 40.0) / 20.0).powi(2) <= 1.0;
        let hair_cap = ((fx - 48.0) / 19.0).powi(2) + ((fy - 34.0) / 18.0).powi(2) <= 1.0 && fy < 34.0;
        let body = ((fx - 48.0) / 36.0).powi(2) + ((fy - 100.0) / 34.0).powi(2) <= 1.0;
        let c = if hair_cap {
            hair
        } else if head {
            skin
        } else if body {
            shirt
        } else {
            ground
        };
        image::Rgba([c[0], c[1], c[2], 255])
    });
    from_image(image::DynamicImage::ImageRgba8(img)).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_round_trip() {
        for s in ["", "f", "fo", "foo", "foob", "fooba", "foobar"] {
            assert_eq!(decode(&encode(s.as_bytes())).unwrap(), s.as_bytes());
        }
        assert_eq!(encode(b"foobar"), "Zm9vYmFy");
        assert_eq!(encode(b"fo"), "Zm8=");
        let p = portrait([200, 220, 240], [60, 40, 30], [40, 90, 160]);
        assert_eq!(pixels(&p).map(|(s, _)| s), Some([96, 96]));
    }

    #[test]
    fn big_picture_files_are_shrunk() {
        let img = image::RgbImage::from_pixel(800, 400, image::Rgb([10, 120, 200]));
        let path = std::env::temp_dir().join(format!("napkin-picture-test-{}.png", std::process::id()));
        img.save(&path).unwrap();
        let data = from_file(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!(pixels(&data).map(|(s, _)| s), Some([MAX_SIDE as usize, MAX_SIDE as usize / 2]));
    }
}
