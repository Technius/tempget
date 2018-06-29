#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate error_chain;

extern crate serde;
extern crate config;
extern crate reqwest;
extern crate url_serde;
extern crate zip;

pub mod template;
pub mod fetcher;

pub mod errors {
    use std;
    use reqwest;
    use config;
    use zip;
    error_chain! {
        foreign_links {
            Io(std::io::Error);
            Reqwest(reqwest::Error);
            Config(config::ConfigError);
            Zip(zip::result::ZipError);
        }
    }
}
