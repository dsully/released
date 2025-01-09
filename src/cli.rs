pub type Result<T, E = crate::errors::CommandError> = std::result::Result<T, E>;

pub trait RunCommand {
    async fn run(self) -> Result<()>;
}
