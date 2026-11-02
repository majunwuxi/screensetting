//! 图像处理管线：裁剪 / 缩放 / 平铺 / 氛围模糊，以及 JPEG 编码。

use std::fs;
use std::io::Write;
use std::path::Path;

use base64::Engine;
use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::{DynamicImage, ImageEncoder, Rgba, RgbaImage};

const CANVAS_BG: Rgba<u8> = Rgba([10, 12, 18, 255]);
const MAX_INPUT_BYTES: u64 = 160 * 1024 * 1024;

pub fn load(path: &Path) -> Result<DynamicImage, String> {
    let meta = fs::metadata(path).map_err(|e| format!("无法读取文件：{e}"))?;
    if meta.len() > MAX_INPUT_BYTES {
        return Err("图片文件过大（超过 160 MB），请先压缩后再试。".to_string());
    }
    let reader = image::ImageReader::open(path)
        .map_err(|e| format!("无法打开图片：{e}"))?
        .with_guessed_format()
        .map_err(|e| format!("无法识别图片格式：{e}"))?;
    reader
        .decode()
        .map_err(|e| format!("无法解析图片：{e}（支持 JPG / PNG / WEBP / BMP / GIF / TIFF）"))
}

fn base_canvas(width: u32, height: u32) -> RgbaImage {
    RgbaImage::from_pixel(width.max(1), height.max(1), CANVAS_BG)
}

fn flatten(img: RgbaImage) -> RgbaImage {
    if img.pixels().all(|p| p[3] == 255) {
        return img;
    }
    let mut canvas = base_canvas(img.width(), img.height());
    image::imageops::overlay(&mut canvas, &img, 0, 0);
    canvas
}

/// 等比缩放后的尺寸（不超过目标框）。
fn fit_within(width: u32, height: u32, max_edge: u32) -> (u32, u32) {
    if width == 0 || height == 0 {
        return (max_edge, max_edge);
    }
    let w = width as f64;
    let h = height as f64;
    let scale = (max_edge as f64 / w).min(max_edge as f64 / h).min(1.0);
    (
        (w * scale).round().max(1.0) as u32,
        (h * scale).round().max(1.0) as u32,
    )
}

fn cover(img: &RgbaImage, width: u32, height: u32) -> RgbaImage {
    let scale = (width as f64 / img.width() as f64)
        .max(height as f64 / img.height() as f64);
    let nw = ((img.width() as f64 * scale).round() as u32).max(width);
    let nh = ((img.height() as f64 * scale).round() as u32).max(height);
    let resized = image::imageops::resize(img, nw, nh, FilterType::Lanczos3);
    let x = (nw - width) / 2;
    let y = (nh - height) / 2;
    image::imageops::crop_imm(&resized, x, y, width, height).to_image()
}

fn contain(img: &RgbaImage, width: u32, height: u32) -> RgbaImage {
    let scale = (width as f64 / img.width() as f64).min(height as f64 / img.height() as f64);
    let (w, h) = (
        ((img.width() as f64 * scale).round() as u32).max(1),
        ((img.height() as f64 * scale).round() as u32).max(1),
    );
    let resized = image::imageops::resize(img, w, h, FilterType::Lanczos3);
    let mut canvas = base_canvas(width, height);
    image::imageops::overlay(
        &mut canvas,
        &resized,
        (width - w) as i64 / 2,
        (height - h) as i64 / 2,
    );
    canvas
}

fn stretch(img: &RgbaImage, width: u32, height: u32) -> RgbaImage {
    image::imageops::resize(img, width, height, FilterType::Lanczos3)
}

fn centered(img: &RgbaImage, width: u32, height: u32) -> RgbaImage {
    // 原图大于画布时先等比缩小，避免被粗暴裁切。
    let tile = if img.width() > width || img.height() > height {
        let scale = (width as f64 / img.width() as f64).min(height as f64 / img.height() as f64);
        image::imageops::resize(
            img,
            ((img.width() as f64 * scale).round() as u32).max(1),
            ((img.height() as f64 * scale).round() as u32).max(1),
            FilterType::Lanczos3,
        )
    } else {
        img.clone()
    };
    let mut canvas = base_canvas(width, height);
    image::imageops::overlay(
        &mut canvas,
        &tile,
        (width as i64 - tile.width() as i64) / 2,
        (height as i64 - tile.height() as i64) / 2,
    );
    canvas
}

fn tiled(img: &RgbaImage, width: u32, height: u32) -> RgbaImage {
    let tile = if img.width() > width || img.height() > height {
        let scale = (width as f64 / img.width() as f64).min(1.0);
        image::imageops::resize(
            img,
            ((img.width() as f64 * scale).round() as u32).max(1),
            ((img.height() as f64 * scale).round() as u32).max(1),
            FilterType::Lanczos3,
        )
    } else {
        img.clone()
    };
    let mut canvas = base_canvas(width, height);
    let mut y = 0i64;
    while y < height as i64 {
        let mut x = 0i64;
        while x < width as i64 {
            image::imageops::overlay(&mut canvas, &tile, x, y);
            x += tile.width() as i64;
        }
        y += tile.height() as i64;
    }
    canvas
}

fn blur_background(img: &RgbaImage, width: u32, height: u32) -> RgbaImage {
    let small_w = (width / 16).max(24);
    let small_h = (height / 16).max(24);
    let tiny = cover(img, small_w, small_h);
    let mut canvas = image::imageops::resize(&tiny, width, height, FilterType::Triangle);
    // 压暗背景，让主体更突出。
    for pixel in canvas.pixels_mut() {
        pixel[0] = (pixel[0] as f32 * 0.42) as u8;
        pixel[1] = (pixel[1] as f32 * 0.42) as u8;
        pixel[2] = (pixel[2] as f32 * 0.46) as u8;
    }
    canvas
}

fn blurred(img: &RgbaImage, width: u32, height: u32) -> RgbaImage {
    let mut canvas = blur_background(img, width, height);
    let scale = (width as f64 / img.width() as f64).min(height as f64 / img.height() as f64) * 0.92;
    let (w, h) = (
        ((img.width() as f64 * scale).round() as u32).max(1),
        ((img.height() as f64 * scale).round() as u32).max(1),
    );
    let sharp = image::imageops::resize(img, w.min(width), h.min(height), FilterType::Lanczos3);
    image::imageops::overlay(
        &mut canvas,
        &sharp,
        (width as i64 - sharp.width() as i64) / 2,
        (height as i64 - sharp.height() as i64) / 2,
    );
    canvas
}

pub fn encode_jpeg(rgb: &image::RgbImage, out: &Path, quality: u8) -> Result<(), String> {
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建目录失败：{e}"))?;
    }
    let file = fs::File::create(out).map_err(|e| format!("创建文件失败：{e}"))?;
    let mut writer = std::io::BufWriter::new(file);
    let encoder = JpegEncoder::new_with_quality(&mut writer, quality.clamp(60, 100));
    encoder
        .write_image(
            rgb.as_raw(),
            rgb.width(),
            rgb.height(),
            image::ExtendedColorType::Rgb8,
        )
        .map_err(|e| format!("编码 JPEG 失败：{e}"))?;
    writer.flush().map_err(|e| format!("写入文件失败：{e}"))?;
    Ok(())
}

/// 把任意图片渲染成刚好覆盖屏幕的壁纸（保证满屏、无黑边）。
pub fn render_to_screen(
    src: &Path,
    out: &Path,
    width: u32,
    height: u32,
    fit: &str,
    quality: u8,
) -> Result<(), String> {
    let image = load(src)?;
    let rgba = flatten(image.to_rgba8());
    let canvas = match fit {
        "contain" => contain(&rgba, width, height),
        "stretch" => stretch(&rgba, width, height),
        "center" => centered(&rgba, width, height),
        "tile" => tiled(&rgba, width, height),
        "blur" => blurred(&rgba, width, height),
        _ => cover(&rgba, width, height),
    };
    let rgb = DynamicImage::ImageRgba8(canvas).to_rgb8();
    encode_jpeg(&rgb, out, quality)
}

/// 生成缩略图文件（用于历史记录）。
pub fn write_thumbnail(
    src: &Path,
    out: &Path,
    max_edge: u32,
    quality: u8,
) -> Result<(), String> {
    let image = load(src)?;
    let rgba = flatten(image.to_rgba8());
    let (w, h) = fit_within(rgba.width(), rgba.height(), max_edge);
    let resized = if w >= rgba.width() && h >= rgba.height() {
        rgba
    } else {
        image::imageops::resize(&rgba, w, h, FilterType::Lanczos3)
    };
    let rgb = DynamicImage::ImageRgba8(resized).to_rgb8();
    encode_jpeg(&rgb, out, quality)
}

/// 读取本地图片并以 data URL 返回，供 WebView 预览。
pub fn preview_data_url(
    path: &Path,
    max_edge: u32,
    quality: u8,
) -> Result<(String, u32, u32), String> {
    let image = load(path)?;
    let (src_w, src_h) = (image.width(), image.height());
    let rgba = flatten(image.to_rgba8());
    let (w, h) = fit_within(rgba.width(), rgba.height(), max_edge);
    let resized = if w >= rgba.width() && h >= rgba.height() {
        rgba
    } else {
        image::imageops::resize(&rgba, w, h, FilterType::Triangle)
    };
    let rgb = DynamicImage::ImageRgba8(resized).to_rgb8();

    let mut buffer: Vec<u8> = Vec::new();
    JpegEncoder::new_with_quality(&mut buffer, quality.clamp(60, 100))
        .write_image(
            rgb.as_raw(),
            rgb.width(),
            rgb.height(),
            image::ExtendedColorType::Rgb8,
        )
        .map_err(|e| format!("生成预览失败：{e}"))?;

    let encoded = base64::engine::general_purpose::STANDARD.encode(&buffer);
    Ok((
        format!("data:image/jpeg;base64,{encoded}"),
        src_w,
        src_h,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn sample(width: u32, height: u32) -> RgbaImage {
        RgbaImage::from_fn(width, height, |x, y| {
            Rgba([
                (x * 255 / width.max(1)) as u8,
                (y * 255 / height.max(1)) as u8,
                160,
                255,
            ])
        })
    }

    fn temp_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("wallpaper-studio-test-{}-{name}", std::process::id()))
    }

    #[test]
    fn renders_every_fit_mode_at_screen_size() {
        let src = temp_path("src.png");
        sample(900, 600).save(&src).expect("write sample");

        for fit in ["cover", "contain", "stretch", "center", "tile", "blur"] {
            let out = temp_path(&format!("out-{fit}.jpg"));
            render_to_screen(&src, &out, 640, 480, fit, 90).expect("render");
            let image = image::open(&out).expect("open output");
            assert_eq!((image.width(), image.height()), (640, 480), "fit={fit}");
            let _ = std::fs::remove_file(out);
        }

        let _ = std::fs::remove_file(src);
    }

    #[test]
    fn preview_is_a_jpeg_data_url() {
        let src = temp_path("preview-src.png");
        sample(500, 500).save(&src).expect("write sample");
        let (url, width, height) = preview_data_url(&src, 240, 80).expect("preview");
        assert!(url.starts_with("data:image/jpeg;base64,"));
        assert_eq!((width, height), (500, 500));
        let _ = std::fs::remove_file(src);
    }

    #[test]
    fn thumbnail_is_bounded_by_max_edge() {
        let src = temp_path("thumb-src.png");
        let out = temp_path("thumb-out.jpg");
        sample(1200, 800).save(&src).expect("write sample");
        write_thumbnail(&src, &out, 400, 80).expect("thumbnail");
        let image = image::open(&out).expect("open thumb");
        assert!(image.width() <= 400 && image.height() <= 400);
        let _ = std::fs::remove_file(src);
        let _ = std::fs::remove_file(out);
    }
}
