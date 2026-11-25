use std::{fs, path::Path};

use crate::models::MetaData;

const DEFAULT_CATEGORIES: [(&str, &str); 4] = [
    ("01_Inbox", "inbox"),
    ("02_Backlog", "backlog"),
    ("03_WIP", "wip"),
    ("04_Graveyard", "graveyard"),
];

pub(crate) fn category_status(directory: &str) -> String {
    match category_prefix(directory) {
        Some(1) => "inbox".to_owned(),
        Some(2) => "backlog".to_owned(),
        Some(3) => "wip".to_owned(),
        Some(4) => "graveyard".to_owned(),
        _ => directory.to_owned(),
    }
}

fn is_default_category(directory: &str) -> bool {
    let status = category_status(directory);
    DEFAULT_CATEGORIES
        .iter()
        .any(|(_, default_status)| *default_status == status)
}

pub(crate) fn category_prefix(directory: &str) -> Option<u64> {
    let (prefix, _) = directory.split_once('_')?;
    prefix.parse().ok()
}

pub(crate) fn category_display_name(directory: &str) -> &str {
    directory
        .split_once('_')
        .map(|(_, name)| name)
        .unwrap_or(directory)
}

pub(crate) fn sort_categories(categories: &mut [String]) {
    categories.sort_by(|left, right| {
        match (category_prefix(left), category_prefix(right)) {
            (Some(left), Some(right)) => left.cmp(&right),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        }
        .then_with(|| left.to_lowercase().cmp(&right.to_lowercase()))
    });
}

pub(crate) fn ensure_default_categories(root: &Path) -> Result<(), String> {
    let existing = category_directories(root)?;
    for (directory, status) in DEFAULT_CATEGORIES {
        if existing
            .iter()
            .any(|category| category_status(category) == status)
        {
            continue;
        }
        let path = root.join(directory);
        fs::create_dir_all(&path)
            .map_err(|error| format!("Failed to create {}: {error}", path.display()))?;
    }
    Ok(())
}

pub(crate) fn category_directories(root: &Path) -> Result<Vec<String>, String> {
    let canonical_root = root
        .canonicalize()
        .map_err(|error| format!("Failed to access idea data directory: {error}"))?;
    let entries = fs::read_dir(root)
        .map_err(|error| format!("Failed to list {}: {error}", root.display()))?;
    let mut categories = Vec::new();
    for entry in entries {
        let entry = entry
            .map_err(|error| format!("Failed to read an entry in {}: {error}", root.display()))?;
        if !entry
            .file_type()
            .map_err(|error| format!("Failed to inspect {}: {error}", entry.path().display()))?
            .is_dir()
        {
            continue;
        }

        let name = entry.file_name().into_string().map_err(|_| {
            format!(
                "Category name is not valid Unicode: {}",
                entry.path().display()
            )
        })?;
        let canonical_category = entry
            .path()
            .canonicalize()
            .map_err(|error| format!("Failed to access {}: {error}", entry.path().display()))?;
        if !canonical_category.starts_with(&canonical_root) {
            return Err(format!(
                "Category directory is outside the data directory: {}",
                entry.path().display()
            ));
        }
        categories.push(name);
    }
    sort_categories(&mut categories);
    Ok(categories)
}

pub(crate) fn sanitize_category_name(name: &str) -> Result<String, String> {
    let sanitized = name
        .trim()
        .chars()
        .map(|character| {
            if character.is_control() || r#"\/:*?"<>|"#.contains(character) {
                '_'
            } else {
                character
            }
        })
        .collect::<String>();
    let sanitized = sanitized.trim_matches([' ', '.']);
    if sanitized.is_empty() {
        return Err("Category name cannot be empty".to_owned());
    }
    Ok(sanitized.to_owned())
}

pub(crate) fn create_category_directory(root: &Path, name: &str) -> Result<String, String> {
    let name = sanitize_category_name(name)?;
    let categories = category_directories(root)?;
    let next_prefix = categories
        .iter()
        .filter_map(|directory| category_prefix(directory))
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or_else(|| "No category prefix numbers are available".to_owned())?;
    let directory = format!("{next_prefix:02}_{name}");
    let path = root.join(&directory);
    fs::create_dir(&path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::AlreadyExists {
            format!("Category already exists: {}", path.display())
        } else {
            format!("Failed to create {}: {error}", path.display())
        }
    })?;
    Ok(directory)
}

fn rewrite_frontmatter_status(raw: &str, status: &str) -> Result<String, String> {
    let frontmatter = raw
        .strip_prefix("---\n")
        .or_else(|| raw.strip_prefix("---\r\n"))
        .ok_or_else(|| "Idea has no YAML frontmatter".to_owned())?;
    let (yaml, body_suffix) = frontmatter
        .split_once("\n---")
        .ok_or_else(|| "Idea has an unterminated YAML frontmatter".to_owned())?;
    let mut metadata: MetaData =
        serde_yaml::from_str(yaml).map_err(|error| format!("Invalid YAML frontmatter: {error}"))?;
    metadata.status = status.to_owned();
    let yaml = serde_yaml::to_string(&metadata)
        .map_err(|error| format!("Failed to serialize idea metadata: {error}"))?;
    Ok(format!("---\n{yaml}---{body_suffix}"))
}

pub(crate) fn rename_category_directory(
    root: &Path,
    current_directory: &str,
    name: &str,
) -> Result<String, String> {
    let categories = category_directories(root)?;
    if !categories
        .iter()
        .any(|category| category == current_directory)
    {
        return Err("Category does not exist".to_owned());
    }

    let name = sanitize_category_name(name)?;
    let prefix = current_directory
        .split_once('_')
        .map(|(prefix, _)| format!("{prefix}_"))
        .unwrap_or_default();
    let new_directory = format!("{prefix}{name}");
    if new_directory == current_directory {
        return Ok(new_directory);
    }

    let source = root.join(current_directory);
    let target = root.join(&new_directory);
    if target.exists() {
        return Err(format!("Category already exists: {}", target.display()));
    }

    let mut updates = Vec::new();
    if !is_default_category(current_directory) {
        let entries = fs::read_dir(&source)
            .map_err(|error| format!("Failed to list {}: {error}", source.display()))?;
        for entry in entries {
            let entry =
                entry.map_err(|error| format!("Failed to read {}: {error}", source.display()))?;
            let path = entry.path();
            if !path.is_file()
                || path.extension().and_then(|extension| extension.to_str()) != Some("md")
            {
                continue;
            }

            let original = fs::read_to_string(&path)
                .map_err(|error| format!("Failed to read {}: {error}", path.display()))?;
            let updated = rewrite_frontmatter_status(&original, &new_directory)?;
            let filename = path
                .file_name()
                .ok_or_else(|| "Invalid category filename".to_owned())?
                .to_owned();
            updates.push((filename, original, updated));
        }
    }

    fs::rename(&source, &target).map_err(|error| {
        format!(
            "Failed to rename category {} to {}: {error}",
            source.display(),
            target.display()
        )
    })?;

    for (filename, _, updated) in &updates {
        if let Err(error) = fs::write(target.join(filename), updated) {
            let mut rollback_errors = Vec::new();
            for (original_filename, original, _) in &updates {
                if let Err(rollback_error) = fs::write(target.join(original_filename), original) {
                    rollback_errors.push(format!(
                        "Failed to restore {}: {rollback_error}",
                        target.join(original_filename).display()
                    ));
                }
            }
            if let Err(rollback_error) = fs::rename(&target, &source) {
                rollback_errors.push(format!("Failed to restore category name: {rollback_error}"));
            }
            let rollback = if rollback_errors.is_empty() {
                String::new()
            } else {
                format!("; rollback errors: {}", rollback_errors.join("; "))
            };
            return Err(format!(
                "Failed to update renamed category metadata: {error}{rollback}"
            ));
        }
    }

    Ok(new_directory)
}

pub(crate) fn delete_category_directory(root: &Path, directory: &str) -> Result<(), String> {
    if !category_directories(root)?
        .iter()
        .any(|category| category == directory)
    {
        return Err("Category does not exist".to_owned());
    }
    if is_default_category(directory) {
        return Err("Built-in categories cannot be deleted".to_owned());
    }

    let path = root.join(directory);
    let mut entries = fs::read_dir(&path)
        .map_err(|error| format!("Failed to list {}: {error}", path.display()))?;
    match entries.next() {
        Some(Ok(_)) => return Err("Only empty categories can be deleted".to_owned()),
        Some(Err(error)) => {
            return Err(format!("Failed to read {}: {error}", path.display()));
        }
        None => {}
    }
    fs::remove_dir(&path).map_err(|error| format!("Failed to delete {}: {error}", path.display()))
}

pub(crate) fn resolve_category_query<'a>(
    categories: &'a [String],
    query: &str,
) -> Result<&'a String, String> {
    let matches = categories
        .iter()
        .filter(|category| {
            category_display_name(category)
                .to_lowercase()
                .contains(&query.trim().to_lowercase())
        })
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [category] => Ok(category),
        [] => Err(format!("No category matches \"{query}\"")),
        _ => Err(format!(
            "Category query \"{query}\" is ambiguous: {}",
            matches
                .iter()
                .map(|category| category_display_name(category).to_owned())
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    use super::*;
    use crate::idea::parse_file;

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            static NEXT_ID: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "ideafs-category-test-{}-{}",
                std::process::id(),
                NEXT_ID.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).expect("temporary test directory should be created");
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn category_prefixes_sort_before_unprefixed_categories() {
        let mut categories = vec![
            "Notes".to_owned(),
            "10_Archive".to_owned(),
            "02_Backlog".to_owned(),
            "01_Inbox".to_owned(),
        ];

        sort_categories(&mut categories);

        assert_eq!(
            categories,
            ["01_Inbox", "02_Backlog", "10_Archive", "Notes"]
        );
        assert_eq!(category_prefix("10_Archive"), Some(10));
        assert_eq!(category_prefix("Notes"), None);
    }

    #[test]
    fn category_names_are_sanitized_and_nonempty() {
        assert_eq!(
            sanitize_category_name("  Art/Ideas. ").as_deref(),
            Ok("Art_Ideas")
        );
        assert!(sanitize_category_name(" .. ").is_err());
    }

    #[test]
    fn new_category_uses_next_number_after_highest_prefix() {
        let root = TestDirectory::new();
        fs::create_dir(root.0.join("01_Inbox")).expect("default category should be created");
        fs::create_dir(root.0.join("07_Archive")).expect("numbered category should be created");
        fs::create_dir(root.0.join("Notes")).expect("unprefixed category should be created");

        let name =
            create_category_directory(&root.0, "Art/Ideas").expect("category should be created");

        assert_eq!(name, "08_Art_Ideas");
        assert!(root.0.join(name).is_dir());
    }

    #[test]
    fn category_rename_updates_custom_statuses_and_preserves_files() {
        let root = TestDirectory::new();
        let old_directory = root.0.join("05_Research");
        fs::create_dir(&old_directory).expect("category should be created");
        fs::write(
            old_directory.join("idea.md"),
            "---\ntitle: Research\ntags: []\nstatus: 05_Research\ncreated_at: 2026-01-01T00:00:00Z\n---\n\nNotes\n",
        )
        .expect("idea should be created");

        let new_name = rename_category_directory(&root.0, "05_Research", "Reading")
            .expect("category should be renamed");
        let renamed_file = root.0.join(&new_name).join("idea.md");
        let renamed = parse_file(format!("{new_name}/idea.md"), &renamed_file)
            .expect("renamed idea should be readable");

        assert_eq!(new_name, "05_Reading");
        assert_eq!(renamed.metadata.status, "05_Reading");
        assert_eq!(renamed.content, "\nNotes\n");
        assert!(!old_directory.exists());
    }

    #[test]
    fn renaming_a_default_category_keeps_its_status_and_prevents_reseeding() {
        let root = TestDirectory::new();
        fs::create_dir(root.0.join("01_Inbox")).expect("default category should be created");

        let renamed = rename_category_directory(&root.0, "01_Inbox", "Capture")
            .expect("default category should be renamed");
        ensure_default_categories(&root.0).expect("missing defaults should be seeded");

        assert_eq!(renamed, "01_Capture");
        assert_eq!(category_status(&renamed), "inbox");
        assert!(!root.0.join("01_Inbox").exists());
    }

    #[test]
    fn category_delete_rejects_builtins_and_nonempty_categories() {
        let root = TestDirectory::new();
        fs::create_dir(root.0.join("01_Inbox")).expect("default category should be created");
        let custom = root.0.join("05_Notes");
        fs::create_dir(&custom).expect("custom category should be created");
        fs::write(custom.join("idea.md"), "idea").expect("file should be created");

        assert!(delete_category_directory(&root.0, "01_Inbox").is_err());
        assert!(
            delete_category_directory(&root.0, "05_Notes")
                .expect_err("nonempty category should not be deleted")
                .contains("Only empty categories")
        );

        fs::remove_file(custom.join("idea.md")).expect("file should be removed");
        delete_category_directory(&root.0, "05_Notes").expect("empty category should be deleted");
        assert!(!custom.exists());
    }
}