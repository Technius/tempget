use config;
use reqwest::{Url, UrlError};
use std::fs;
use std::io::Read;
use url_serde; // For deriving Deserialize for Url

use errors;

#[derive(Debug, Clone, Deserialize)]
pub struct Template {
    pub files: Vec<ResourceInfo>
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

#[derive(Debug, Clone, Deserialize)]
pub struct ResourceInfo {
    #[serde(with = "url_serde")]
    pub url: Url,
    pub file_name: String
}

impl ResourceInfo {
    pub fn new(url: Url, file_name: String) -> Self {
        ResourceInfo {
            url, file_name
        }
    }
    pub fn from_url_str<'a, S : Into<&'a str>>(url_str: S, file_name: String) ->
          Result<Self, UrlError> {
        Url::parse(url_str.into()).map(move |u| ResourceInfo::new(u, file_name))
    }
}
