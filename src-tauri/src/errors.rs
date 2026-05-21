use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Parse error: {0}")]
    ParseError(String),
    #[error("Device not found")]
    DeviceNotFound,
    #[error("Characteristic not found: {0}")]
    CharacteristicNotFound(String),
    #[error("Actor channel closed")]
    ChannelClosed,
    #[error("{0}")]
    Other(String),
}

impl serde::Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}
