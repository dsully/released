use async_trait::async_trait;

pub type Result<T, E = crate::errors::CommandError> = std::result::Result<T, E>;

#[async_trait]
pub trait RunCommand {
    async fn run(self) -> Result<()>;
}
