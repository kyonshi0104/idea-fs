mod category;
mod icon;
mod idea;
mod mac_cli;
mod models;
mod paths;
mod url;

pub use category::{
    create_category_directory, delete_category_directory, ensure_default_categories,
    rename_category_directory,
};
pub use icon::read_icon_data_url;
pub use idea::{
    cli_add_idea, cli_delete_idea, cli_list_ideas, collect_idea_tree, create_idea_in_directory,
    idea_path, parse_file, resolve_status_directory,
};
pub use mac_cli::{install_mac_cli, is_mac_cli_installed};
pub use models::{CliIdea, FileData, MetaData, TreeData};
pub use paths::data_root;
pub use url::validate_http_url;

pub use category::{category_directories, category_status};

#[cfg(test)]
mod tests {
    use crate::{data_root, models::MetaData};

    #[test]
    fn cli_data_root_uses_documents_idea_explorer_directory() {
        assert_eq!(
            data_root().expect("Documents directory should be available"),
            dirs::document_dir()
                .expect("Documents directory should be available")
                .join("IdeaFS")
        );
    }

    #[test]
    fn legacy_frontmatter_defaults_new_fields_to_none() {
        let metadata: MetaData = serde_yaml::from_str(
            "title: Old idea\ntags: []\nstatus: inbox\ncreated_at: 2026-01-01T00:00:00Z\n",
        )
        .expect("legacy metadata should remain readable");

        assert_eq!(metadata.icon, None);
        assert_eq!(metadata.resource_path, None);
        assert_eq!(metadata.app_type, None);
        assert_eq!(metadata.url, None);
    }

    #[test]
    fn extended_frontmatter_reads_generic_resource_fields() {
        let metadata: MetaData = serde_yaml::from_str(
            "title: Music idea\ntags: [music]\nstatus: wip\ncreated_at: 2026-01-01T00:00:00Z\nicon: 🎹\nresource_path: C:/Music/Track.als\napp_type: ableton\nurl: https://example.com\n",
        )
        .expect("extended metadata should be readable");

        assert_eq!(metadata.icon.as_deref(), Some("🎹"));
        assert_eq!(
            metadata.resource_path.as_deref(),
            Some("C:/Music/Track.als")
        );
        assert_eq!(metadata.app_type.as_deref(), Some("ableton"));
        assert_eq!(metadata.url.as_deref(), Some("https://example.com"));
    }
}