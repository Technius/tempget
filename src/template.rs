use config;
use std::fs;
use std::io::Read;
use std::collections::HashMap;
use url_serde; // For deriving Deserialize for Url

use errors;

#[derive(Debug, Clone, Deserialize)]
pub struct Template {
    pub retrieve: HashMap<String, url_serde::SerdeUrl>
}

impl Template {
    pub fn from_file(fname: &str) -> errors::Result<Self> {
        let mut cfg = config::Config::new();
        let mut file = fs::File::open(fname)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        let cfg_file = config::File::from_str(&contents, config::FileFormat::Toml);
        cfg.merge(cfg_file)?;
        let res = cfg.try_into()?;
        Ok(res)
    }
}
