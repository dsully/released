use async_trait::async_trait;
use clap::Args;
use console::style;
use indicatif::HumanDuration;
use pluralizer::pluralize;
use std::time::Instant;

use crate::{
    cli::{Result, RunCommand},
    config::Config,
    errors::CommandError,
    install,
    spinner::spinner,
    system::System,
};

#[derive(Debug, Clone, Args)]
pub struct Update {
    /// Which package to update, when omitted all packages will be updated.
    only: Option<String>,
}

#[async_trait]
impl RunCommand for Update {
    async fn run(self) -> Result<()> {
        let mut config = Config::load()?;
        let system = System::default();

        let started = Instant::now();
        let mut count = 0;

        println!("Checking for package updates ...\n");

        for (name, package) in config.packages.clone().iter() {
            //
            if self.only.as_ref().is_some_and(|o| name != o) {
                continue;
            }

            count += 1;

            let s = spinner();

            s.set_message(format!("⊙ Checking {} ...", name));

            match install::install_release(&mut config, package, &system, None, false).await {
                Ok(_) => s.finish_with_message(format!("{} {} updated", style("󰄴").green(), &name)),
                Err(e) if e.to_string() == CommandError::NoUpdateNeeded.to_string() => {
                    s.finish_with_message(format!("{} {} is already up to date!", style("󰐾").blue(), &name));
                }
                Err(e) => s.finish_with_message(format!("{} {:?}", style("").red(), e.to_string())),
            }
        }

        println!("\n\nChecked for {} in {}", pluralize("update", count, true), HumanDuration(started.elapsed()));

        Ok(())
    }
}
