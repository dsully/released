use tracing::error;

use crate::{config::Config, errors::CommandError, install, system::System};

pub async fn update(config: &mut Config, system: &'_ System, only: Option<String>) -> super::Result<()> {
    for (name, package) in config.packages.clone().iter() {
        //
        if only.as_ref().is_some_and(|o| name != o) {
            continue;
        }

        println!("Updating {} ...", name);

        match install::install_release(config, package, system, None, false).await {
            Ok(_) => println!("Installed {} successfully!", &name),
            Err(e) if e.to_string() == CommandError::NoUpdateNeeded.to_string() => println!("{} is already up to date!", &package.name),
            Err(e) => error!("{:?}", e.to_string()),
        }
    }

    Ok(())
}
