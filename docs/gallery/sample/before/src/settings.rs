//! Import settings.

/// How many orders one import reads at most, unless the settings say otherwise.
pub const DEFAULT_BATCH_SIZE: usize = 500;

/// Settings read from the import file.
pub struct Settings {
    /// The most orders one import reads.
    pub batch_size: usize,
}

/// Load settings from `path`, falling back to the defaults for anything missing.
pub fn load(path: &str) -> Result<Settings, String> {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let mut settings = Settings { batch_size: DEFAULT_BATCH_SIZE };
    for line in text.lines() {
        if let Some(value) = line.strip_prefix("batch_size = ") {
            settings.batch_size = value.parse().map_err(|_| "batch_size is not a number".to_string())?;
        }
    }
    Ok(settings)
}
