mod add;
mod list;
mod remove;
mod update;

pub use add::{add, Patterns};
pub use list::list;
pub use remove::remove;
pub use update::update;

pub type Result<T, E = crate::errors::CommandError> = std::result::Result<T, E>;
