use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Parse error: {0}")]
    ParseError(String),
    #[error(".ZWO file parse error: {0}")]
    ZWOFileParseError(String),
    #[error("FTMS packet parse error: {0}")]
    FTMSPacketParseError(String),
    #[error("HRS packet parse error: {0}")]
    HRSParseError(String),
    #[error("BLE scan error: {0}")]
    BLEScanError(String),
    #[error("Device not found: {0}")]
    DeviceNotFound(String),
    #[error("BLE connection failed: {0}")]
    BLEConnectError(String),
    #[error("BLE command failed: {0}")]
    BLECommandError(String),
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
