use indicatif::{ProgressBar, ProgressStyle};
use std::time::Duration;

pub fn spinner() -> ProgressBar {
    let pb = ProgressBar::new_spinner();

    pb.enable_steady_tick(Duration::from_millis(50));

    pb.set_style(
        ProgressStyle::with_template("{spinner:.cyan.bold} {msg}")
            .unwrap()
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏ "),
    );

    pb
}

