//! Metadata handling on export. Pixel Studio re-encodes from pixels, so exports carry
//! no EXIF/XMP by default; `copy_metadata` optionally restores the source's EXIF/ICC.

use std::path::Path;

use img_parts::{Bytes, DynImage, ImageEXIF, ImageICC};

/// Copy EXIF and ICC profile from `src` into the already-written `dst` (JPEG/PNG/WebP).
/// Returns Ok(false) when either file's container is unsupported.
pub fn copy_metadata(src: &Path, dst: &Path) -> anyhow::Result<bool> {
    let src_bytes = Bytes::from(std::fs::read(src)?);
    let dst_bytes = Bytes::from(std::fs::read(dst)?);
    let (Some(s), Some(mut d)) = (DynImage::from_bytes(src_bytes)?, DynImage::from_bytes(dst_bytes)?) else {
        return Ok(false);
    };
    d.set_exif(s.exif());
    d.set_icc_profile(s.icc_profile());
    let mut out = Vec::new();
    d.encoder().write_to(&mut out)?;
    std::fs::write(dst, out)?;
    Ok(true)
}

/// True if the file contains an EXIF block.
pub fn has_exif(path: &Path) -> bool {
    std::fs::read(path)
        .ok()
        .and_then(|b| DynImage::from_bytes(Bytes::from(b)).ok().flatten())
        .is_some_and(|i| i.exif().is_some())
}
