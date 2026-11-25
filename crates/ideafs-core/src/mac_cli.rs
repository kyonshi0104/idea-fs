#[cfg(target_os = "macos")]
use std::{fs, path::Path};

#[cfg(target_os = "macos")]
pub(crate) fn mac_cli_executable() -> Result<std::path::PathBuf, String> {
    let gui_executable = std::env::current_exe()
        .map_err(|error| format!("Failed to locate the application executable: {error}"))?;
    let directory = gui_executable
        .parent()
        .ok_or_else(|| "Application executable has no parent directory".to_owned())?;
    let cli_executable = directory.join("idfs");
    if !cli_executable.is_file() {
        return Err(format!(
            "IdeaFS CLI executable was not found: {}",
            cli_executable.display()
        ));
    }
    Ok(cli_executable)
}

pub fn is_mac_cli_installed() -> Result<bool, String> {
    #[cfg(target_os = "macos")]
    {
        let link = Path::new("/usr/local/bin/idfs");
        match fs::read_link(link) {
            Ok(target) => {
                let executable = mac_cli_executable()?;
                Ok(target == executable)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(format!("Failed to inspect {}: {error}", link.display())),
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        Ok(false)
    }
}

pub fn install_mac_cli() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        use std::os::unix::fs::symlink;

        let executable = mac_cli_executable()?;
        let link = Path::new("/usr/local/bin/idfs");
        if let Ok(existing_target) = fs::read_link(link) {
            if existing_target == executable {
                return Ok(());
            }
        }
        match fs::symlink_metadata(link) {
            Ok(_) => {
                return Err(format!(
                    "{} already exists and does not point to this application",
                    link.display()
                ));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!("Failed to inspect {}: {error}", link.display()));
            }
        }
        fs::create_dir_all("/usr/local/bin")
            .map_err(|error| format!("Failed to create /usr/local/bin: {error}"))?;
        symlink(&executable, link).map_err(|error| {
            format!(
                "Failed to install {}: {error}. You may need permission to write /usr/local/bin.",
                link.display()
            )
        })
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("The CLI PATH installer is only available on macOS".to_owned())
    }
}
