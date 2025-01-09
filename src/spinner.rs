use std::time::Duration;

use indicatif::{ProgressBar, ProgressStyle};

pub fn spinner() -> ProgressBar {
    let pb = ProgressBar::new_spinner();

    pb.enable_steady_tick(Duration::from_millis(50));

    pb.set_style(
        ProgressStyle::with_template("{msg} {spinner:.cyan.bold}")
            .expect("Failed to set progress style.")
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏ "),
    );

    pb
}
