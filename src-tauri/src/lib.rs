mod app;
mod commands;
mod config;
mod paths;
mod shortcuts;

pub use app::run;
pub use ideafs_core::{
    CliIdea, MetaData, cli_add_idea, cli_delete_idea, cli_list_ideas, data_root,
};
