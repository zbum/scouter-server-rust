use thiserror::Error;

#[derive(Error, Debug)]
pub enum ScouterError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Protocol error: {0}")]
    Protocol(String),

    #[error("Unknown pack type: {0}")]
    UnknownPackType(u8),

    #[error("Unknown value type: {0}")]
    UnknownValueType(u8),

    #[error("UTF-8 decode error: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),

    #[error("Config error: {0}")]
    Config(String),

    #[error("Channel send error")]
    ChannelSend,
}

pub type Result<T> = std::result::Result<T, ScouterError>;
