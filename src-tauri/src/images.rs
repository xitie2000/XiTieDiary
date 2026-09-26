use crate::error::AppError;
use image::imageops::FilterType;

pub fn compress_to_jpeg(
    bytes: &[u8],
    max_edge: u32,
    quality: u8,
) -> Result<(Vec<u8>, u32, u32), AppError> {
    let img = image::load_from_memory(bytes)
        .map_err(|e| AppError::Io(std::io::Error::other(format!("图片解码失败: {e}"))))?;
    let (w, h) = (img.width(), img.height());
    let longest = w.max(h);
    let img = if longest > max_edge {
        let scale = max_edge as f64 / longest as f64;
        let nw = ((w as f64 * scale).round() as u32).max(1);
        let nh = ((h as f64 * scale).round() as u32).max(1);
        img.resize_exact(nw, nh, FilterType::Triangle)
    } else {
        img
    };
    let (nw, nh) = (img.width(), img.height());
    let rgb = image::DynamicImage::ImageRgb8(img.to_rgb8());
    let mut out = Vec::new();
    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, quality);
    rgb.write_with_encoder(encoder)
        .map_err(|e| AppError::Io(std::io::Error::other(format!("JPEG 编码失败: {e}"))))?;
    Ok((out, nw, nh))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gradient_png(width: u32, height: u32) -> Vec<u8> {
        let img = image::RgbImage::from_fn(width, height, |x, y| {
            image::Rgb([
                (x % 251) as u8,
                (y % 241) as u8,
                ((x + y) % 233) as u8,
            ])
        });
        let mut buf = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(img)
            .write_to(&mut buf, image::ImageFormat::Png)
            .unwrap();
        buf.into_inner()
    }

    #[test]
    fn compress_resizes_large_image() {
        let input = gradient_png(4000, 2500);
        let (out, w, h) = compress_to_jpeg(&input, 1920, 80).unwrap();
        assert_eq!(w, 1920);
        assert_eq!(h, 1200);
        assert!(out.len() < input.len() / 2, "{} vs {}", out.len(), input.len());
        let format = image::guess_format(&out).unwrap();
        assert_eq!(format, image::ImageFormat::Jpeg);
    }

    #[test]
    fn compress_keeps_small_image_dims() {
        let input = gradient_png(800, 600);
        let (out, w, h) = compress_to_jpeg(&input, 1920, 80).unwrap();
        assert_eq!((w, h), (800, 600));
        assert_eq!(image::guess_format(&out).unwrap(), image::ImageFormat::Jpeg);
    }
}
