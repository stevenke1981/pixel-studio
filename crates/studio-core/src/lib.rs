pub mod batch;
pub mod c2pa_metadata;
pub mod compose;
pub mod convert;
pub mod error;
pub mod filters;
pub mod formats;
pub mod gif;
pub mod gpano_xmp;
pub mod image_io;
pub mod panorama360;
pub mod resize;
pub mod transform;

pub use batch::{
    BatchOptions, BatchResult, BatchSummary, C2paAddOptions, C2paRemovalOptions,
    collect_supported_files, process_batch, process_batch_with_progress, process_c2pa_add_batch,
    process_c2pa_add_batch_with_progress, process_c2pa_removal_batch,
    process_c2pa_removal_batch_with_progress,
};
pub use c2pa_metadata::{
    C2paInfo, C2paManifestDraft, inspect_c2pa, remove_c2pa_manifest, write_c2pa_manifest,
};
pub use compose::{
    ComposeDocument, ComposeLayer, ImageLayer, LayerBounds, ShapeKind, ShapeLayer, TextLayer,
    compose_layer_bounds, render_composition, shape_layer_bounds, text_layer_bounds,
};
pub use convert::{
    ConvertOptions, ConvertResult, convert_image, flatten_alpha, save_dynamic_image,
};
pub use error::{PhotoError, Result};
pub use filters::{
    adjust_hue, adjust_temperature, adjust_tint, auto_contrast, posterize, rotate_180, sepia,
    vignette,
};
pub use formats::SupportedFormat;
pub use gif::{GifOptions, GifResult, write_animated_gif};
pub use image_io::{ImageInfo, inspect_image, load_dynamic_image};
pub use panorama360::{
    PanoramaMode, PanoramaOptions, PanoramaResult, make_equirectangular,
    write_panorama_dynamic_jpeg, write_panorama_jpeg,
};
pub use transform::{blur_region, crop_region};
