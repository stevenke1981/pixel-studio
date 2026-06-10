use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum PhotoError {
    #[error("unsupported image format for path: {0}")]
    UnsupportedFormat(PathBuf),

    #[error("invalid JPEG data")]
    InvalidJpeg,

    #[error("XMP segment is too large for a JPEG APP1 marker")]
    XmpTooLarge,

    #[error("invalid input: {0}")]
    InvalidInput(String),

    #[error("C2PA error: {0}")]
    C2pa(String),

    #[error("image error: {0}")]
    Image(#[from] image::ImageError),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, PhotoError>;
