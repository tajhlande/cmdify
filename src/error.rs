use thiserror::Error;

#[derive(Debug, Error)]
#[allow(clippy::enum_variant_names)]
pub enum Error {
    #[error("config error: {0}")]
    ConfigError(String),
    #[error("provider error: {0}")]
    ProviderError(String),
    #[error("response error: unexpected format: {0}")]
    ResponseError(String),
    #[error("http error: {0}")]
    HttpError(#[from] reqwest::Error),
    #[error("io error: {0}")]
    IoError(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
