use tracing::debug;

use crate::{config::Config, errors::CommandError};

pub async fn remove(config: &mut Config, name: &'_ str) -> super::Result<()> {
    //
    match config.installed.get(name) {
        Some(installed) => {
            if installed.path.exists() {
                debug!("Removing {:?}", &installed.path);

                if let Err(_) = std::fs::remove_file(&installed.path) {
                    return Err(CommandError::FileDelete {
                        file_name: installed.path.clone(),
                    });
                };
            }

            config.packages.remove(&installed.name);
            config.installed.remove(name);
            config.save()?;

            println!("Removed '{}'", &name);

            Ok(())
        }
        _ => Err(CommandError::PackageNotFound { name: name.to_string() }),
    }
}
