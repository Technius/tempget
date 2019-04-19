use config;
use std::fs;
use std::path::Path;
use std::io::Read;
use std::collections::HashMap;
use serde_derive::Deserialize;
use url_serde; // For deriving Deserialize for Url

use crate::errors;

#[derive(Debug, Clone, Deserialize)]
pub struct Template {
    pub retrieve: HashMap<String, url_serde::SerdeUrl>,
    #[serde(default)]
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
pub enum ExtractInfo {
    Directory(String),
    Mapping(HashMap<String, String>)
}
