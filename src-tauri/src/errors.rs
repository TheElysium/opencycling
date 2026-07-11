use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Parse error: {0}")]
    ParseError(String),
    #[error(".ZWO file parse error: {0}")]
    ZWOFileParseError(String),
    #[error(".ZWO file read error: {0}")]
    ZWOFileReadError(String),
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
    /// Wraps a rusqlite error directly, preserving the source for logging.
    #[error("Database error: {0}")]
    DbError(#[from] rusqlite::Error),
    #[error("Session already active")]
    SessionAlreadyActive,
    /// Wraps a std::io::Error directly, preserving the source for logging.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// Wraps a reqwest::Error directly for pure HTTP transport failures.
    /// Use StravaAuth / StravaUpload for errors with constructed context messages.
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Strava auth error: {0}")]
    StravaAuth(String),
    #[error("Strava API error: {0}")]
    StravaApi(String),
    #[error("Strava upload error: {0}")]
    StravaUpload(String),
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

// AppError serializes to a bare string at the Tauri boundary (see the Serialize impl
// above), so its specta type is just `String`. This makes command error types resolve
// to `string` in the generated bindings, matching what the frontend actually receives.
impl specta::Type for AppError {
    fn definition(types: &mut specta::Types) -> specta::datatype::DataType {
        <String as specta::Type>::definition(types)
    }
}
