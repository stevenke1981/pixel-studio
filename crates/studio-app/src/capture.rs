//! Platform screenshot capture for the desktop application.
//!
//! The UI should depend on [`CaptureProvider`] rather than on Windows APIs. A
//! capture request is converted to the same [`image::DynamicImage`] type used
//! by the existing open/paste paths, so callers can reuse their normal canvas
//! initialisation and clipboard code.

use std::time::Duration;

use anyhow::{Result, bail};
use image::DynamicImage;

/// A desktop region in physical screen coordinates (pixels).
///
/// Regions must fit within one display.  The origin is the top-left corner of
/// the virtual desktop, so negative coordinates are valid on a display placed
/// to the left or above the primary display. Regions must fit within one
/// display; the region's origin is relative to the virtual desktop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CaptureRegion {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl CaptureRegion {
    /// Create a region.  Width and height are checked when a request is
    /// captured so this constructor remains convenient for UI state.
    #[must_use]
    pub const fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.width > 0 && self.height > 0
    }
}

/// The source area for a screenshot request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureMode {
    /// Capture the primary display at its native pixel resolution.
    Fullscreen,
    /// Capture a region on the display containing the region's top-left point.
    Region(CaptureRegion),
}

impl CaptureMode {
    #[must_use]
    pub const fn region(region: CaptureRegion) -> Self {
        Self::Region(region)
    }
}

/// A screenshot operation, including an optional countdown delay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CaptureOptions {
    pub mode: CaptureMode,
    pub delay: Duration,
}

impl CaptureOptions {
    #[must_use]
    pub const fn fullscreen() -> Self {
        Self {
            mode: CaptureMode::Fullscreen,
            delay: Duration::ZERO,
        }
    }

    #[must_use]
    pub const fn region(region: CaptureRegion) -> Self {
        Self {
            mode: CaptureMode::Region(region),
            delay: Duration::ZERO,
        }
    }

    #[must_use]
    pub const fn with_delay(self, delay: Duration) -> Self {
        Self { delay, ..self }
    }
}

/// Capture backend used by the UI.
pub trait CaptureProvider {
    /// Capture according to `options`, returning an image ready for the
    /// existing Pixel Studio image-loading pipeline.
    fn capture(&self, options: CaptureOptions) -> Result<DynamicImage>;
}

/// Native desktop capture implementation.
#[derive(Debug, Default, Clone, Copy)]
pub struct PlatformCaptureProvider;

impl CaptureProvider for PlatformCaptureProvider {
    fn capture(&self, options: CaptureOptions) -> Result<DynamicImage> {
        if let CaptureMode::Region(region) = options.mode
            && !region.is_valid()
        {
            bail!("截圖區域的寬度與高度必須大於 0");
        }

        if !options.delay.is_zero() {
            std::thread::sleep(options.delay);
        }

        platform::capture(options.mode)
    }
}

/// Capture the primary display without constructing a provider manually.
pub fn capture_fullscreen() -> Result<DynamicImage> {
    PlatformCaptureProvider.capture(CaptureOptions::fullscreen())
}

/// Capture a single-display region without constructing a provider manually.
pub fn capture_region(region: CaptureRegion) -> Result<DynamicImage> {
    PlatformCaptureProvider.capture(CaptureOptions::region(region))
}

#[cfg(windows)]
mod platform {
    use super::{CaptureMode, Result};
    use anyhow::{Context, anyhow};
    use image::DynamicImage;
    use xcap::Monitor;

    pub(super) fn capture(mode: CaptureMode) -> Result<DynamicImage> {
        let monitor = match mode {
            CaptureMode::Fullscreen => primary_screen()?,
            CaptureMode::Region(region) => {
                let monitor = Monitor::from_point(region.x, region.y)
                    .context("找不到截圖區域所在的顯示器")?;
                let monitor_x = monitor.x().context("讀取顯示器 X 座標失敗")?;
                let monitor_y = monitor.y().context("讀取顯示器 Y 座標失敗")?;
                let local_x = region
                    .x
                    .checked_sub(monitor_x)
                    .ok_or_else(|| anyhow!("截圖區域座標超出顯示器範圍"))?;
                let local_y = region
                    .y
                    .checked_sub(monitor_y)
                    .ok_or_else(|| anyhow!("截圖區域座標超出顯示器範圍"))?;
                let local_x =
                    u32::try_from(local_x).map_err(|_| anyhow!("截圖區域 X 座標必須為非負數"))?;
                let local_y =
                    u32::try_from(local_y).map_err(|_| anyhow!("截圖區域 Y 座標必須為非負數"))?;
                let image = monitor
                    .capture_region(local_x, local_y, region.width, region.height)
                    .context("Windows 區域截圖失敗")?;
                return rgba_image(image);
            }
        };

        let image = monitor.capture_image().context("Windows 全螢幕截圖失敗")?;
        rgba_image(image)
    }

    fn primary_screen() -> Result<Monitor> {
        let mut monitors = Monitor::all().context("無法列舉 Windows 顯示器")?;
        monitors
            .iter()
            .find(|monitor| monitor.is_primary().unwrap_or(false))
            .cloned()
            .or_else(|| monitors.pop())
            .ok_or_else(|| anyhow!("找不到可用的 Windows 顯示器"))
    }

    fn rgba_image(source: xcap::image::RgbaImage) -> Result<DynamicImage> {
        let (width, height) = source.dimensions();
        let pixels = source.into_raw();
        let image = image::RgbaImage::from_raw(width, height, pixels)
            .ok_or_else(|| anyhow!("截圖像素資料大小不正確"))?;
        Ok(DynamicImage::ImageRgba8(image))
    }
}

#[cfg(not(windows))]
mod platform {
    use super::{CaptureMode, Result};
    use anyhow::anyhow;
    use image::DynamicImage;

    pub(super) fn capture(_mode: CaptureMode) -> Result<DynamicImage> {
        Err(anyhow!(
            "目前只支援 Windows 截圖；請改用開啟圖片或剪貼簿貼上"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn region_validation_rejects_empty_dimensions() {
        assert!(!CaptureRegion::new(0, 0, 0, 100).is_valid());
        assert!(!CaptureRegion::new(0, 0, 100, 0).is_valid());
        assert!(CaptureRegion::new(-200, 10, 640, 480).is_valid());
    }

    #[test]
    fn options_preserve_mode_when_delay_changes() {
        let region = CaptureRegion::new(10, 20, 100, 80);
        let options = CaptureOptions::region(region).with_delay(Duration::from_secs(3));
        assert_eq!(options.mode, CaptureMode::Region(region));
        assert_eq!(options.delay, Duration::from_secs(3));
    }

    #[cfg(not(windows))]
    #[test]
    fn unsupported_platform_returns_actionable_error() {
        let error = capture_fullscreen().expect_err("non-Windows capture should be unsupported");
        assert!(error.to_string().contains("只支援 Windows"));
    }
}
