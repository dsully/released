use async_trait::async_trait;
use clap::Args;
use tracing::debug;

use crate::{
    cli::{Result, RunCommand},
    config::Config,
    errors::CommandError,
};

#[derive(Debug, Clone, Args)]
pub struct Remove {
    /// Name of the package to remove.
    name: String,
}

#[async_trait]
impl RunCommand for Remove {
    async fn run(self) -> Result<()> {
        let mut config = Config::load()?;

        //
        match config.installed.get(&self.name) {
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
                config.installed.remove(&self.name);
                config.save()?;

                println!("Removed '{}'", &self.name);

                Ok(())
            }
            _ => Err(CommandError::PackageNotFound { name: self.name.to_string() }),
        }
    }
}
