use std::collections::HashMap;

use egui::{ColorImage, Context, Image, TextureHandle, TextureOptions, Vec2};

use crate::ui::state::{LeftToolCategory, ToolId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ToolIcon {
    Open,
    Paste,
    Save,
    SaveAs,
    Undo,
    Redo,
    Fit,
    ActualSize,
    Export,
    File,
    Browse,
    Drawing,
    Edit,
    Layers,
    Gif,
    Batch,
    C2pa,
    Move,
    Lasso,
    Crop,
    Brush,
    Line,
    Eraser,
    Stamp,
    Text,
    Shape,
    Eyedropper,
    Zoom,
}

impl ToolIcon {
    fn asset_name(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Paste => "paste",
            Self::Save => "save",
            Self::SaveAs => "save_as",
            Self::Undo => "undo",
            Self::Redo => "redo",
            Self::Fit => "fit",
            Self::ActualSize => "actual_size",
            Self::Export => "export",
            Self::File => "file",
            Self::Browse => "browse",
            Self::Drawing => "drawing",
            Self::Edit => "edit",
            Self::Layers => "layers",
            Self::Gif => "gif",
            Self::Batch => "batch",
            Self::C2pa => "c2pa",
            Self::Move => "move",
            Self::Lasso => "lasso",
            Self::Crop => "crop",
            Self::Brush => "brush",
            Self::Line => "line",
            Self::Eraser => "eraser",
            Self::Stamp => "stamp",
            Self::Text => "text",
            Self::Shape => "shape",
            Self::Eyedropper => "eyedropper",
            Self::Zoom => "zoom",
        }
    }

    fn png_bytes(self) -> &'static [u8] {
        match self {
            Self::Open => include_bytes!("../../../../assets/icon/tools/open.png"),
            Self::Paste => include_bytes!("../../../../assets/icon/tools/paste.png"),
            Self::Save => include_bytes!("../../../../assets/icon/tools/save.png"),
            Self::SaveAs => include_bytes!("../../../../assets/icon/tools/save_as.png"),
            Self::Undo => include_bytes!("../../../../assets/icon/tools/undo.png"),
            Self::Redo => include_bytes!("../../../../assets/icon/tools/redo.png"),
            Self::Fit => include_bytes!("../../../../assets/icon/tools/fit.png"),
            Self::ActualSize => include_bytes!("../../../../assets/icon/tools/actual_size.png"),
            Self::Export => include_bytes!("../../../../assets/icon/tools/export.png"),
            Self::File => include_bytes!("../../../../assets/icon/tools/file.png"),
            Self::Browse => include_bytes!("../../../../assets/icon/tools/browse.png"),
            Self::Drawing => include_bytes!("../../../../assets/icon/tools/drawing.png"),
            Self::Edit => include_bytes!("../../../../assets/icon/tools/edit.png"),
            Self::Layers => include_bytes!("../../../../assets/icon/tools/layers.png"),
            Self::Gif => include_bytes!("../../../../assets/icon/tools/gif.png"),
            Self::Batch => include_bytes!("../../../../assets/icon/tools/batch.png"),
            Self::C2pa => include_bytes!("../../../../assets/icon/tools/c2pa.png"),
            Self::Move => include_bytes!("../../../../assets/icon/tools/move.png"),
            Self::Lasso => include_bytes!("../../../../assets/icon/tools/lasso.png"),
            Self::Crop => include_bytes!("../../../../assets/icon/tools/crop.png"),
            Self::Brush => include_bytes!("../../../../assets/icon/tools/brush.png"),
            Self::Line => include_bytes!("../../../../assets/icon/tools/line.png"),
            Self::Eraser => include_bytes!("../../../../assets/icon/tools/eraser.png"),
            Self::Stamp => include_bytes!("../../../../assets/icon/tools/stamp.png"),
            Self::Text => include_bytes!("../../../../assets/icon/tools/text.png"),
            Self::Shape => include_bytes!("../../../../assets/icon/tools/shape.png"),
            Self::Eyedropper => include_bytes!("../../../../assets/icon/tools/eyedropper.png"),
            Self::Zoom => include_bytes!("../../../../assets/icon/tools/zoom.png"),
        }
    }
}

#[derive(Default)]
pub struct ToolIconTextures {
    textures: HashMap<ToolIcon, TextureHandle>,
}

impl ToolIconTextures {
    pub fn image(&mut self, ctx: &Context, icon: ToolIcon, size: f32) -> Image<'_> {
        Image::from_texture(self.texture(ctx, icon)).fit_to_exact_size(Vec2::splat(size))
    }

    fn texture(&mut self, ctx: &Context, icon: ToolIcon) -> &TextureHandle {
        self.textures.entry(icon).or_insert_with(|| {
            let rgba = image::load_from_memory(icon.png_bytes())
                .unwrap_or_else(|error| {
                    panic!("tool icon {} failed to decode: {error}", icon.asset_name())
                })
                .to_rgba8();
            let size = [rgba.width() as usize, rgba.height() as usize];
            let color_image = ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
            ctx.load_texture(
                format!("tool-icon-{}", icon.asset_name()),
                color_image,
                TextureOptions::LINEAR,
            )
        })
    }
}

impl From<ToolId> for ToolIcon {
    fn from(tool: ToolId) -> Self {
        match tool {
            ToolId::Move => Self::Move,
            ToolId::Lasso => Self::Lasso,
            ToolId::Crop => Self::Crop,
            ToolId::Brush => Self::Brush,
            ToolId::Line => Self::Line,
            ToolId::Eraser => Self::Eraser,
            ToolId::Stamp => Self::Stamp,
            ToolId::Text => Self::Text,
            ToolId::Shape => Self::Shape,
            ToolId::Eyedropper => Self::Eyedropper,
            ToolId::Zoom => Self::Zoom,
        }
    }
}

impl From<LeftToolCategory> for ToolIcon {
    fn from(category: LeftToolCategory) -> Self {
        match category {
            LeftToolCategory::File => Self::File,
            LeftToolCategory::Browse => Self::Browse,
            LeftToolCategory::Drawing => Self::Drawing,
            LeftToolCategory::Edit => Self::Edit,
            LeftToolCategory::Layers => Self::Layers,
            LeftToolCategory::Export => Self::Export,
            LeftToolCategory::Gif => Self::Gif,
            LeftToolCategory::Batch => Self::Batch,
            LeftToolCategory::C2pa => Self::C2pa,
        }
    }
}
