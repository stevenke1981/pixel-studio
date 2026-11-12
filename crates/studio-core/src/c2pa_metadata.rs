use std::path::Path;

use c2pa::{Builder, EphemeralSigner, Reader};
use serde_json::{Value, json};

use crate::{PhotoError, Result};

const PHOTO_TOOL_ASSERTION: &str = "org.photo_tool.c2pa.edit";

#[derive(Debug, Clone, Default)]
pub struct C2paInfo {
    pub present: bool,
    pub title: Option<String>,
    pub creator: Option<String>,
    pub action: Option<String>,
    pub validation_state: Option<String>,
    pub manifest_count: usize,
    pub raw_json: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct C2paManifestDraft {
    pub title: String,
    pub creator: String,
    pub action: String,
}

fn is_jfif(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("jfif"))
}

fn c2pa_format_for_path(path: &Path) -> Option<String> {
    if is_jfif(path) {
        Some("image/jpeg".to_owned())
    } else {
        c2pa::format_from_path(path)
    }
}

pub fn inspect_c2pa(path: impl AsRef<Path>) -> Result<C2paInfo> {
    let path = path.as_ref();
    let reader_res = if is_jfif(path) {
        let mut file = std::fs::File::open(path)?;
        Reader::default().with_stream("image/jpeg", &mut file)
    } else {
        Reader::default().with_file(path)
    };
    let reader = match reader_res {
        Ok(reader) => reader,
        Err(c2pa::Error::JumbfNotFound) => {
            return Ok(C2paInfo::default());
        }
        Err(error) => return Err(c2pa_error(error)),
    };

    let raw_json = reader.json();
    let parsed = serde_json::from_str::<Value>(&raw_json).ok();
    let custom = parsed.as_ref().and_then(find_photo_tool_assertion);
    let active_manifest = reader.active_manifest();

    Ok(C2paInfo {
        present: true,
        title: active_manifest
            .and_then(|manifest| manifest.title())
            .map(ToOwned::to_owned),
        creator: custom
            .and_then(|value| value.get("creator"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        action: custom
            .and_then(|value| value.get("action"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        validation_state: Some(format!("{:?}", reader.validation_state())),
        manifest_count: reader.manifests().len(),
        raw_json: Some(raw_json),
    })
}

pub fn write_c2pa_manifest(
    source: impl AsRef<Path>,
    dest: impl AsRef<Path>,
    draft: &C2paManifestDraft,
) -> Result<()> {
    let source = source.as_ref();
    let dest = dest.as_ref();
    if dest.exists() {
        return Err(PhotoError::InvalidInput(format!(
            "C2PA output already exists: {}",
            dest.display()
        )));
    }

    let format = c2pa_format_for_path(dest)
        .ok_or_else(|| PhotoError::UnsupportedFormat(dest.to_path_buf()))?;
    let signer = EphemeralSigner::new("studio-app.local").map_err(c2pa_error)?;
    let mut builder = Builder::default()
        .with_definition(json!({
            "title": draft.title.trim(),
            "format": format,
            "claim_generator_info": [
                {
                    "name": "studio-app",
                    "version": env!("CARGO_PKG_VERSION")
                }
            ]
        }))
        .map_err(c2pa_error)?;

    builder
        .add_action(json!({
            "action": "c2pa.edited",
            "softwareAgent": {
                "name": "studio-app",
                "version": env!("CARGO_PKG_VERSION")
            },
            "parameters": {
                "description": draft.action.trim()
            }
        }))
        .map_err(c2pa_error)?;
    builder
        .add_assertion_json(
            PHOTO_TOOL_ASSERTION,
            &json!({
                "creator": draft.creator.trim(),
                "action": draft.action.trim()
            }),
        )
        .map_err(c2pa_error)?;

    if is_jfif(source) || is_jfif(dest) {
        let src_format = c2pa_format_for_path(source)
            .ok_or_else(|| PhotoError::UnsupportedFormat(source.to_path_buf()))?;
        if src_format != format {
            return Err(PhotoError::InvalidInput(
                "Source and destination file formats must match".to_owned(),
            ));
        }
        if let Some(output_dir) = dest.parent()
            && !output_dir.as_os_str().is_empty()
        {
            std::fs::create_dir_all(output_dir)?;
        }
        let mut src_file = std::fs::File::open(source)?;
        let mut dst_file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(dest)?;
        if let Err(err) = builder.sign(&signer, &format, &mut src_file, &mut dst_file) {
            drop(dst_file);
            let _ = std::fs::remove_file(dest);
            return Err(c2pa_error(err));
        }
        return Ok(());
    }

    builder
        .sign_file(&signer, source, dest)
        .map(|_| ())
        .map_err(c2pa_error)
}

pub fn remove_c2pa_manifest(source: impl AsRef<Path>, dest: impl AsRef<Path>) -> Result<()> {
    let source = source.as_ref();
    let dest = dest.as_ref();
    if dest.exists() {
        return Err(PhotoError::InvalidInput(format!(
            "C2PA output already exists: {}",
            dest.display()
        )));
    }

    if is_jfif(dest) {
        let temp_jpg = dest.with_extension("c2pa_strip.jpg");
        let _ = std::fs::remove_file(&temp_jpg);
        std::fs::copy(source, &temp_jpg)?;
        let res = match c2pa::jumbf_io::remove_jumbf_from_file(&temp_jpg) {
            Ok(()) | Err(c2pa::Error::JumbfNotFound) => Ok(()),
            Err(error) => Err(c2pa_error(error)),
        };
        if let Err(err) = res {
            let _ = std::fs::remove_file(&temp_jpg);
            return Err(err);
        }
        std::fs::rename(&temp_jpg, dest)?;
        return Ok(());
    }

    std::fs::copy(source, dest)?;
    match c2pa::jumbf_io::remove_jumbf_from_file(dest) {
        Ok(()) => Ok(()),
        Err(c2pa::Error::JumbfNotFound) => Ok(()),
        Err(error) => Err(c2pa_error(error)),
    }
}

fn find_photo_tool_assertion(value: &Value) -> Option<&Value> {
    match value {
        Value::Object(map) => {
            if map.get("label").and_then(Value::as_str) == Some(PHOTO_TOOL_ASSERTION) {
                return map.get("data");
            }
            map.values().find_map(find_photo_tool_assertion)
        }
        Value::Array(values) => values.iter().find_map(find_photo_tool_assertion),
        _ => None,
    }
}

fn c2pa_error(error: c2pa::Error) -> PhotoError {
    PhotoError::C2pa(error.to_string())
}
