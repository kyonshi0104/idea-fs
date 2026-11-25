use std::{
    fs,
    io::Read,
    path::Path,
};

use base64::{Engine, engine::general_purpose::STANDARD as BASE64};

const MAX_ICON_BYTES: u64 = 5 * 1024 * 1024;

pub fn read_icon_data_url(path: String) -> Result<String, String> {
    let path = std::path::PathBuf::from(path);
    if !path.is_absolute() || !path.is_file() {
        return Err("Icon path must be an existing absolute file path".to_owned());
    }
    let mime_type = image_mime_type(&path)
        .ok_or_else(|| "Icon must be a PNG, JPEG, GIF, WebP, AVIF, or SVG image".to_owned())?;
    let file = fs::File::open(&path)
        .map_err(|error| format!("Failed to read icon {}: {error}", path.display()))?;
    let mut bytes = Vec::new();
    file.take(MAX_ICON_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("Failed to read icon {}: {error}", path.display()))?;
    if bytes.len() as u64 > MAX_ICON_BYTES {
        return Err("Icon image exceeds the 5 MiB limit".to_owned());
    }
    Ok(format!("data:{mime_type};base64,{}", BASE64.encode(bytes)))
}

pub(crate) fn image_mime_type(path: &Path) -> Option<&'static str> {
    match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        "avif" => Some("image/avif"),
        "svg" => Some("image/svg+xml"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_mime_type_is_limited_to_supported_image_formats() {
        assert_eq!(image_mime_type(Path::new("cover.PNG")), Some("image/png"));
        assert_eq!(image_mime_type(Path::new("cover.jpeg")), Some("image/jpeg"));
        assert_eq!(
            image_mime_type(Path::new("cover.svg")),
            Some("image/svg+xml")
        );
        assert_eq!(image_mime_type(Path::new("notes.md")), None);
    }
}