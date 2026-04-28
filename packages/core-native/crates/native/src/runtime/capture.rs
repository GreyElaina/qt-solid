use vello::peniko::color::PremulRgba8;

use super::types::{WidgetError, WidgetResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WidgetCaptureFormat {
    Argb32Premultiplied,
    Rgba8Premultiplied,
}

impl WidgetCaptureFormat {
    pub const fn bytes_per_pixel(self) -> usize {
        match self {
            Self::Argb32Premultiplied => 4,
            Self::Rgba8Premultiplied => 4,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct WidgetCapture {
    format: WidgetCaptureFormat,
    width_px: u32,
    height_px: u32,
    stride: usize,
    scale_factor: f64,
    bytes: Vec<u8>,
}

impl WidgetCapture {
    fn validate_layout(
        format: WidgetCaptureFormat,
        width_px: u32,
        height_px: u32,
        stride: usize,
    ) -> WidgetResult<usize> {
        let bytes_per_pixel = format.bytes_per_pixel();
        if stride % bytes_per_pixel != 0 {
            return Err(WidgetError::new(format!(
                "widget capture stride {stride} is not aligned to pixel size {bytes_per_pixel}"
            )));
        }

        let min_stride = width_px as usize * bytes_per_pixel;
        if width_px > 0 && stride < min_stride {
            return Err(WidgetError::new(format!(
                "widget capture stride {stride} is smaller than minimum {min_stride}"
            )));
        }

        stride
            .checked_mul(height_px as usize)
            .ok_or_else(|| WidgetError::new("widget capture buffer length overflow"))
    }

    pub fn new_zeroed(
        format: WidgetCaptureFormat,
        width_px: u32,
        height_px: u32,
        stride: usize,
        scale_factor: f64,
    ) -> WidgetResult<Self> {
        let byte_len = Self::validate_layout(format, width_px, height_px, stride)?;

        Ok(Self {
            format,
            width_px,
            height_px,
            stride,
            scale_factor,
            bytes: vec![0; byte_len],
        })
    }

    pub fn from_premul_rgba_pixels(
        width_px: u32,
        height_px: u32,
        stride: usize,
        scale_factor: f64,
        pixels: Vec<PremulRgba8>,
    ) -> WidgetResult<Self> {
        let format = WidgetCaptureFormat::Rgba8Premultiplied;
        let byte_len = Self::validate_layout(format, width_px, height_px, stride)?;
        let mut bytes = Vec::with_capacity(pixels.len() * 4);
        for pixel in pixels {
            bytes.extend_from_slice(&[pixel.r, pixel.g, pixel.b, pixel.a]);
        }
        if bytes.len() != byte_len {
            return Err(WidgetError::new(format!(
                "widget capture byte length {} does not match expected {byte_len}",
                bytes.len()
            )));
        }

        Ok(Self {
            format,
            width_px,
            height_px,
            stride,
            scale_factor,
            bytes,
        })
    }

    pub fn format(&self) -> WidgetCaptureFormat {
        self.format
    }

    pub fn width_px(&self) -> u32 {
        self.width_px
    }

    pub fn height_px(&self) -> u32 {
        self.height_px
    }

    pub fn stride(&self) -> usize {
        self.stride
    }

    pub fn scale_factor(&self) -> f64 {
        self.scale_factor
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn bytes_mut(&mut self) -> &mut [u8] {
        &mut self.bytes
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    /// Convert to an `image::RgbaImage`, un-premultiplying alpha.
    pub fn to_rgba_image(&self) -> image::RgbaImage {
        let mut img = image::RgbaImage::new(self.width_px, self.height_px);
        for y in 0..self.height_px {
            for x in 0..self.width_px {
                let off = y as usize * self.stride + x as usize * 4;
                if off + 4 > self.bytes.len() {
                    continue;
                }
                let (r, g, b, a) = match self.format {
                    WidgetCaptureFormat::Argb32Premultiplied => {
                        (self.bytes[off + 1], self.bytes[off + 2], self.bytes[off + 3], self.bytes[off])
                    }
                    WidgetCaptureFormat::Rgba8Premultiplied => {
                        (self.bytes[off], self.bytes[off + 1], self.bytes[off + 2], self.bytes[off + 3])
                    }
                };
                let pixel = if a == 0 {
                    [0, 0, 0, 0]
                } else if a == 255 {
                    [r, g, b, 255]
                } else {
                    let inv = 255.0 / a as f64;
                    [
                        (r as f64 * inv).round().min(255.0) as u8,
                        (g as f64 * inv).round().min(255.0) as u8,
                        (b as f64 * inv).round().min(255.0) as u8,
                        a,
                    ]
                };
                img.put_pixel(x, y, image::Rgba(pixel));
            }
        }
        img
    }

    /// Convert to PNG bytes (un-premultiplied, straight alpha).
    pub fn to_png_bytes(&self) -> Option<Vec<u8>> {
        if self.width_px == 0 || self.height_px == 0 || self.bytes.is_empty() {
            return None;
        }
        let img = self.to_rgba_image();
        let mut buf = Vec::new();
        let mut cursor = std::io::Cursor::new(&mut buf);
        img.write_to(&mut cursor, image::ImageFormat::Png).ok()?;
        Some(buf)
    }
}
