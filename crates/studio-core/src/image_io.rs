use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use image::GenericImageView;

use crate::{PhotoError, Result, SupportedFormat};

#[derive(Debug, Clone)]
pub struct ImageInfo {
    pub path: PathBuf,
    pub format: SupportedFormat,
    pub width: u32,
    pub height: u32,
    pub color_type: String,
    pub file_size: u64,
}

pub fn inspect_image(path: impl AsRef<Path>) -> Result<ImageInfo> {
    let path = path.as_ref();
    let metadata = std::fs::metadata(path)?;
    let (image, format) = load_dynamic_image_with_format(path)?;
    let (width, height) = image.dimensions();

    Ok(ImageInfo {
        path: path.to_path_buf(),
        format,
        width,
        height,
        color_type: format!("{:?}", image.color()),
        file_size: metadata.len(),
    })
}

pub fn load_dynamic_image(path: impl AsRef<Path>) -> Result<image::DynamicImage> {
    let (image, _) = load_dynamic_image_with_format(path)?;
    Ok(image)
}

/// Load an image using extension lookup first (preserving `.jfif` and `.tga` identity),
/// falling back to magic-byte format detection when extension lookup is missing or mismatched.
pub fn load_dynamic_image_with_format(
    path: impl AsRef<Path>,
) -> Result<(image::DynamicImage, SupportedFormat)> {
    let path = path.as_ref();
    if let Some(ext_format) = SupportedFormat::from_path(path) {
        let mut reader = image::ImageReader::new(BufReader::new(File::open(path)?));
        reader.set_format(ext_format.image_format());
        match reader.decode() {
            Ok(image) => return Ok((image, ext_format)),
            Err(primary_err) => {
                let guessed = image::ImageReader::new(BufReader::new(File::open(path)?))
                    .with_guessed_format()?;
                let Some(actual_format) = guessed
                    .format()
                    .and_then(SupportedFormat::from_image_format)
                else {
                    return Err(PhotoError::UnsupportedFormat(path.to_path_buf()));
                };
                if actual_format.image_format() != ext_format.image_format() {
                    return Ok((guessed.decode()?, actual_format));
                }
                return Err(primary_err.into());
            }
        }
    }

    let guessed =
        image::ImageReader::new(BufReader::new(File::open(path)?)).with_guessed_format()?;
    let Some(format) = guessed
        .format()
        .and_then(SupportedFormat::from_image_format)
    else {
        return Err(PhotoError::UnsupportedFormat(path.to_path_buf()));
    };
    Ok((guessed.decode()?, format))
}

#[cfg(test)]
mod tests {
    use image::{DynamicImage, Rgba, RgbaImage};

    use super::{inspect_image, load_dynamic_image, load_dynamic_image_with_format};
    use crate::{PhotoError, SupportedFormat};

    #[test]
    fn decodes_misnamed_and_extensionless_images_via_magic_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let img = DynamicImage::ImageRgba8(RgbaImage::from_pixel(4, 3, Rgba([12, 34, 56, 255])));

        // PNG bytes saved with .jpg extension.
        let misnamed = dir.path().join("actually_png.jpg");
        img.save_with_format(&misnamed, image::ImageFormat::Png)
            .unwrap();
        let (decoded, detected) = load_dynamic_image_with_format(&misnamed).unwrap();
        assert_eq!(detected, SupportedFormat::Png);
        assert_eq!((decoded.width(), decoded.height()), (4, 3));
        assert_eq!(
            inspect_image(&misnamed).unwrap().format,
            SupportedFormat::Png
        );

        // JPEG bytes saved without an extension.
        let no_ext = dir.path().join("extensionless");
        DynamicImage::ImageRgb8(img.to_rgb8())
            .save_with_format(&no_ext, image::ImageFormat::Jpeg)
            .unwrap();
        let info = inspect_image(&no_ext).unwrap();
        assert_eq!(info.format, SupportedFormat::Jpeg);
        assert_eq!((info.width, info.height), (4, 3));
        assert_eq!(load_dynamic_image(&no_ext).unwrap().width(), 4);
    }

    #[test]
    fn preserves_jfif_identity_and_rejects_non_images() {
        let dir = tempfile::tempdir().unwrap();
        let img = DynamicImage::ImageRgb8(
            DynamicImage::ImageRgba8(RgbaImage::from_pixel(5, 4, Rgba([1, 2, 3, 255]))).to_rgb8(),
        );
        let jfif = dir.path().join("sample.jfif");
        img.save_with_format(&jfif, image::ImageFormat::Jpeg)
            .unwrap();
        assert_eq!(inspect_image(&jfif).unwrap().format, SupportedFormat::Jfif);

        let non_image = dir.path().join("garbage.png");
        std::fs::write(&non_image, b"definitely not an image").unwrap();
        assert!(matches!(
            load_dynamic_image(&non_image),
            Err(PhotoError::UnsupportedFormat(_))
        ));

        let unknown_ext = dir.path().join("garbage.xyz");
        std::fs::write(&unknown_ext, b"plain text").unwrap();
        assert!(matches!(
            inspect_image(&unknown_ext),
            Err(PhotoError::UnsupportedFormat(_))
        ));
    }
}
