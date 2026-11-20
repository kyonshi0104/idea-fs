#[cfg(test)]
use clap::CommandFactory;
use clap::{Parser, Subcommand, error::ErrorKind};
use std::{
    io::{self, Write},
    process::{Command, ExitCode},
};

#[derive(Parser)]
#[command(name = "idfs", about = "IdeaFS CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Add {
        title: String,
        #[arg(short, long)]
        category: Option<String>,
        #[arg(short, long, value_delimiter = ',')]
        tags: Vec<String>,
        #[arg(short, long)]
        resource: Option<String>,
        #[arg(short, long)]
        url: Option<String>,
    },
    List {
        #[arg(short, long)]
        category: Option<String>,
    },
    #[command(alias = "remove")]
    Rm {
        query: String,
        #[arg(short, long)]
        force: bool,
    },
    Open {
        query: Option<String>,
    },
}

fn main() -> ExitCode {
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    if args.is_empty() || (args.len() == 1 && args[0] == "open") {
        return match launch_gui() {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("Error: {error}");
                ExitCode::FAILURE
            }
        };
    }

    match Cli::try_parse_from(
        std::iter::once(String::from("idfs")).chain(
            args.iter()
                .map(|argument| argument.to_string_lossy().into_owned()),
        ),
    ) {
        Ok(cli) => match run_cli(cli) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("Error: {error}");
                ExitCode::FAILURE
            }
        },
        Err(error) => {
            let code = if matches!(
                error.kind(),
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
            ) {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            };
            let _ = error.print();
            code
        }
    }
}

fn launch_gui() -> Result<(), String> {
    let executable = std::env::current_exe()
        .map_err(|error| format!("Failed to locate idfs executable: {error}"))?;
    let directory = executable
        .parent()
        .ok_or_else(|| "idfs executable has no parent directory".to_owned())?;
    let gui_executable = directory.join(gui_binary_name());
    if !gui_executable.is_file() {
        return Err(format!(
            "IdeaFS GUI executable was not found: {}",
            gui_executable.display()
        ));
    }

    Command::new(&gui_executable)
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("Failed to launch {}: {error}", gui_executable.display()))
}

#[cfg(windows)]
fn gui_binary_name() -> &'static str {
    "ideafs.exe"
}

#[cfg(not(windows))]
fn gui_binary_name() -> &'static str {
    "ideafs"
}

fn run_cli(cli: Cli) -> Result<(), String> {
    match cli.command {
        Commands::Add {
            title,
            category,
            tags,
            resource,
            url,
        } => {
            let filepath =
                idea_explorer_lib::cli_add_idea(&title, category.as_deref(), tags, resource, url)?;
            println!("Saved: {filepath}");
            Ok(())
        }
        Commands::List { category } => list_ideas(category.as_deref()),
        Commands::Rm { query, force } => remove_idea(&query, force),
        Commands::Open { query: Some(query) } => open_idea(&query),
        Commands::Open { query: None } => launch_gui(),
    }
}

fn list_ideas(category: Option<&str>) -> Result<(), String> {
    let ideas = idea_explorer_lib::cli_list_ideas(category)?;
    if ideas.is_empty() {
        println!("No ideas found.");
        return Ok(());
    }

    let mut previous_category = String::new();
    for idea in ideas {
        let category = idea.filepath.split('/').next().unwrap_or_default();
        if category != previous_category {
            previous_category = category.to_owned();
            println!("\n{}", display_category(category));
        }
        let tags = if idea.metadata.tags.is_empty() {
            String::new()
        } else {
            format!("  [{}]", idea.metadata.tags.join(", "))
        };
        println!("  {}{tags}\n    {}", idea.metadata.title, idea.filepath);
    }
    Ok(())
}

fn remove_idea(query: &str, force: bool) -> Result<(), String> {
    let matches = find_ideas(query)?;
    match matches.as_slice() {
        [] => Err(format!("No ideas match \"{query}\"")),
        [idea] => {
            if !force {
                print!("Delete \"{}\"? [y/N] ", idea.metadata.title);
                io::stdout()
                    .flush()
                    .map_err(|error| format!("Failed to write confirmation prompt: {error}"))?;
                let mut answer = String::new();
                io::stdin()
                    .read_line(&mut answer)
                    .map_err(|error| format!("Failed to read confirmation: {error}"))?;
                if !matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes") {
                    println!("Cancelled.");
                    return Ok(());
                }
            }
            idea_explorer_lib::cli_delete_idea(&idea.filepath)?;
            println!("Deleted: {}", idea.filepath);
            Ok(())
        }
        many => {
            println!(
                "Query \"{query}\" matches {} ideas; no files were deleted:",
                many.len()
            );
            for idea in many {
                println!("  {}  ({})", idea.metadata.title, idea.filepath);
            }
            Ok(())
        }
    }
}

fn open_idea(query: &str) -> Result<(), String> {
    let matches = find_ideas(query)?;
    let idea = match matches.as_slice() {
        [idea] => idea,
        [] => return Err(format!("No ideas match \"{query}\"")),
        many => {
            let candidates = many
                .iter()
                .map(|idea| format!("{} ({})", idea.metadata.title, idea.filepath))
                .collect::<Vec<_>>()
                .join("\n");
            return Err(format!(
                "Query \"{query}\" matches multiple ideas:\n{candidates}"
            ));
        }
    };

    let target = idea
        .metadata
        .resource_path
        .as_deref()
        .or(idea.metadata.url.as_deref())
        .ok_or_else(|| format!("{} has no resource path or URL", idea.metadata.title))?;
    open::that(target).map_err(|error| format!("Failed to open {target}: {error}"))
}

fn find_ideas(query: &str) -> Result<Vec<idea_explorer_lib::CliIdea>, String> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return Err("Search query cannot be empty".to_owned());
    }
    let ideas = idea_explorer_lib::cli_list_ideas(None)?;
    Ok(ideas
        .into_iter()
        .filter(|idea| {
            idea.metadata.title.to_lowercase().contains(&query)
                || idea.filepath.to_lowercase().contains(&query)
        })
        .collect())
}

fn display_category(directory: &str) -> &str {
    directory
        .split_once('_')
        .map(|(_, name)| name)
        .unwrap_or(directory)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_exposes_requested_subcommands() {
        let help = Cli::command().render_long_help().to_string();
        assert!(help.contains("add"));
        assert!(help.contains("list"));
        assert!(help.contains("rm"));
        assert!(help.contains("open"));
    }

    #[test]
    fn category_display_hides_numeric_prefix() {
        assert_eq!(display_category("03_WIP"), "WIP");
        assert_eq!(display_category("Ideas"), "Ideas");
    }

    #[test]
    fn gui_binary_name_matches_platform_executable() {
        #[cfg(windows)]
        assert_eq!(gui_binary_name(), "ideafs.exe");
        #[cfg(not(windows))]
        assert_eq!(gui_binary_name(), "ideafs");
    }
}
