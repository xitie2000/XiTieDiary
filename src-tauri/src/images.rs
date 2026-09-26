use crate::error::AppError;
use image::imageops::FilterType;
use image::ImageDecoder;

pub fn compress_to_jpeg(
    bytes: &[u8],
    max_edge: u32,
    quality: u8,
) -> Result<(Vec<u8>, u32, u32), AppError> {
    let reader = image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| AppError::Io(std::io::Error::other(format!("图片读取失败: {e}"))))?;
    let mut decoder = reader
        .into_decoder()
        .map_err(|e| AppError::Io(std::io::Error::other(format!("图片解码失败: {e}"))))?;
    let orientation = decoder
        .orientation()
        .map_err(|e| AppError::Io(std::io::Error::other(format!("EXIF 读取失败: {e}"))))?;
    let mut img = image::DynamicImage::from_decoder(decoder)
        .map_err(|e| AppError::Io(std::io::Error::other(format!("图片解码失败: {e}"))))?;
    img.apply_orientation(orientation);
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

    #[test]
    fn compress_applies_exif_orientation() {
        // 400x200 横向图 + EXIF Orientation=6（需顺时针旋转 90°）→ 输出应为 200x400
        let base = gradient_png(400, 200);
        let mut jpeg = Vec::new();
        image::DynamicImage::ImageRgb8(image::RgbImage::from_fn(400, 200, |x, y| {
            image::Rgb([(x % 251) as u8, (y % 241) as u8, 0x80])
        }))
        .write_to(&mut std::io::Cursor::new(&mut jpeg), image::ImageFormat::Jpeg)
        .unwrap();
        let _ = base;

        let with_exif = inject_exif_orientation(&jpeg, 6);
        let (out, w, h) = compress_to_jpeg(&with_exif, 1920, 80).unwrap();
        assert_eq!((w, h), (200, 400), "竖拍照片（Orientation=6）应转正");
        assert_eq!(image::guess_format(&out).unwrap(), image::ImageFormat::Jpeg);
    }

    /// 在 SOI 后注入一个仅含 Orientation 短标签的 EXIF APP1 段（小端 TIFF）
    fn inject_exif_orientation(jpeg: &[u8], orientation: u16) -> Vec<u8> {
        let mut tiff = Vec::new();
        tiff.extend_from_slice(&[b'I', b'I', 0x2A, 0x00, 8, 0, 0, 0]); // TIFF 头，IFD0 偏移 8
        tiff.extend_from_slice(&1u16.to_le_bytes()); // 目录项数
        tiff.extend_from_slice(&0x0112u16.to_le_bytes()); // Orientation 标签
        tiff.extend_from_slice(&3u16.to_le_bytes()); // 类型 SHORT
        tiff.extend_from_slice(&1u32.to_le_bytes()); // 数量 1
        tiff.extend_from_slice(&orientation.to_le_bytes());
        tiff.extend_from_slice(&[0u8, 0]); // 值域剩余两字节
        tiff.extend_from_slice(&0u32.to_le_bytes()); // 下一 IFD 为 0

        let payload_len = 6 + tiff.len();
        let mut app1 = vec![0xFF, 0xE1];
        app1.extend_from_slice(&(payload_len as u16).to_be_bytes());
        app1.extend_from_slice(b"Exif\0\0");
        app1.extend_from_slice(&tiff);

        let mut out = Vec::with_capacity(jpeg.len() + app1.len());
        out.extend_from_slice(&jpeg[..2]);
        out.extend_from_slice(&app1);
        out.extend_from_slice(&jpeg[2..]);
        out
    }
}
