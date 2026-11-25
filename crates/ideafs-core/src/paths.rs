use std::path::PathBuf;

pub fn data_root() -> Result<PathBuf, String> {
    dirs::document_dir()
        .map(|directory| directory.join("IdeaFS"))
        .ok_or_else(|| "Could not resolve the user's Documents directory".to_owned())
}