mod app;
mod category;
mod commands;
mod config;
mod icon;
mod idea;
mod mac_cli;
mod models;
mod paths;
mod shortcuts;
mod url;

pub use app::run;
pub use idea::{cli_add_idea, cli_delete_idea, cli_list_ideas};
pub use models::{CliIdea, MetaData};
pub use paths::data_root;

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