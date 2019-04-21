use config;
use std::fs;
use std::path::Path;
use std::io::Read;
use std::collections::HashMap;
use serde_derive::Deserialize;
use url_serde; // For deriving Deserialize for Url

use crate::errors;

#[derive(Debug, Clone, Deserialize)]
/// Represents a template file.
pub struct Template {
    /// The files to download from the given URLs.
    pub retrieve: HashMap<String, url_serde::SerdeUrl>,
    #[serde(default)]
    /// The file archives that should be extracted.
    pub extract: HashMap<String, ExtractInfo>
}

impl Template {
    pub fn from_file(file_path: &Path) -> errors::Result<Self> {
        let mut cfg = config::Config::new();
        let mut file = fs::File::open(file_path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        let cfg_file = config::File::from_str(&contents, config::FileFormat::Toml);
        cfg.merge(cfg_file)?;
        let res = cfg.try_into()?;
        Ok(res)
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
/// Indicates how the files in an archive should be extracted.
pub enum ExtractInfo {
    /// All files in the archive should be extracted to the given directory.
    Directory(String),
    /// The files specified in the mapping should be extracted to the specified
    /// locations.
    Mapping(HashMap<String, String>)
}
