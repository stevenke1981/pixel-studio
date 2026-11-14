//! Metadata handling on export. Pixel Studio re-encodes from pixels, so exports carry
//! no EXIF/XMP by default; `copy_metadata` and `apply_metadata` optionally restore EXIF/ICC.

use std::path::Path;

use img_parts::{Bytes, DynImage, ImageEXIF, ImageICC};

/// In-memory snapshot of EXIF and ICC profile bytes extracted from a loaded image.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SourceMetadata {
    pub exif: Option<Vec<u8>>,
    pub icc_profile: Option<Vec<u8>>,
}

impl SourceMetadata {
    pub fn has_exif(&self) -> bool {
        self.exif.is_some()
    }
}

/// Read EXIF and ICC profile from raw encoded image bytes without keeping a file handle.
pub fn read_metadata_bytes(raw: &[u8]) -> Option<SourceMetadata> {
    let img = DynImage::from_bytes(Bytes::from(raw.to_vec())).ok()??;
    Some(SourceMetadata {
        exif: img.exif().map(|b| b.to_vec()),
        icc_profile: img.icc_profile().map(|b| b.to_vec()),
    })
}

/// Read EXIF and ICC profile from `path` into memory so later exports succeed even if the
/// source file on disk is moved, renamed, or deleted.
pub fn read_metadata(path: &Path) -> Option<SourceMetadata> {
    let bytes = std::fs::read(path).ok()?;
    read_metadata_bytes(&bytes)
}

/// Apply previously captured `SourceMetadata` into the already-written `dst` (JPEG/PNG/WebP).
/// Returns `Ok(false)` when `dst` container is unsupported.
pub fn apply_metadata(meta: &SourceMetadata, dst: &Path) -> anyhow::Result<bool> {
    let dst_bytes = Bytes::from(std::fs::read(dst)?);
    let Some(mut d) = DynImage::from_bytes(dst_bytes)? else {
        return Ok(false);
    };
    d.set_exif(meta.exif.as_ref().map(|b| Bytes::from(b.clone())));
    d.set_icc_profile(meta.icc_profile.as_ref().map(|b| Bytes::from(b.clone())));
    let mut out = Vec::new();
    d.encoder().write_to(&mut out)?;
    std::fs::write(dst, out)?;
    Ok(true)
}

/// Copy EXIF and ICC profile from `src` into the already-written `dst` (JPEG/PNG/WebP).
/// Returns Ok(false) when either file's container is unsupported.
pub fn copy_metadata(src: &Path, dst: &Path) -> anyhow::Result<bool> {
    let src_bytes = Bytes::from(std::fs::read(src)?);
    let Some(s) = DynImage::from_bytes(src_bytes)? else {
        return Ok(false);
    };
    let meta = SourceMetadata {
        exif: s.exif().map(|b| b.to_vec()),
        icc_profile: s.icc_profile().map(|b| b.to_vec()),
    };
    apply_metadata(&meta, dst)
}

/// True if the file contains an EXIF block.
pub fn has_exif(path: &Path) -> bool {
    std::fs::read(path)
        .ok()
        .and_then(|b| DynImage::from_bytes(Bytes::from(b)).ok().flatten())
        .is_some_and(|i| i.exif().is_some())
}
