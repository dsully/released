use async_trait::async_trait;
use clap::Args;
use serde::Serialize;
use tabled::{
    settings::{
        object::Columns,
        style::{HorizontalLine, Style},
        Color, Modify,
    },
    Table, Tabled,
};

use crate::{
    cli::{Result, RunCommand},
    config::Config,
    errors::CommandError,
};

#[derive(Debug, Clone, Args)]
pub struct List {}

#[derive(Tabled, Serialize, PartialEq, PartialOrd, Eq, Ord)]
struct Installed<'a> {
    #[tabled(rename = "Alias")]
    alias: &'a str,
    #[tabled(rename = "Version")]
    version: &'a str,
    #[tabled(rename = "Path")]
    path: &'a str,
    #[tabled(rename = "Repository")]
    repository: String,
}

#[async_trait]
impl RunCommand for List {
    async fn run(self) -> Result<()> {
        let config = Config::load()?;

        if config.installed.is_empty() {
            return Err(CommandError::EmptyPackages);
        }

        let mut lines = Vec::with_capacity(config.installed.len());

        for (name, installed) in config.installed.iter() {
            let package = config.packages.get(&installed.name).unwrap();

            lines.push(Installed {
                repository: format!("https://github.com/{}", &package.name),
                alias: name,
                version: &installed.version,
                path: installed.path.to_str().unwrap(),
            });
        }

        lines.sort();

        println!("Installed packages:");
        println!("\n{}", create_table(&lines));

        Ok(())
    }
}

fn create_table(data: &[Installed]) -> Table {
    let theme = Style::modern()
        .remove_top()
        .remove_bottom()
        .remove_horizontal()
        // NB: order matters, must be after .remove_horizontal.
        .horizontals([HorizontalLine::new(1, Style::modern().get_horizontal())])
        .remove_left()
        .remove_right();

    let mut table = Table::builder(data).build();

    table
        .with(theme)
        .with(Modify::new(Columns::single(0)).with(Color::FG_WHITE))
        .with(Modify::new(Columns::single(1)).with(Color::FG_GREEN))
        .with(Modify::new(Columns::single(2)).with(Color::FG_CYAN))
        .with(Modify::new(Columns::single(3)).with(Color::FG_BLUE));

    table
}
