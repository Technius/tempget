#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate error_chain;
#[macro_use]
extern crate structopt;

extern crate serde;
extern crate config;
extern crate reqwest;
extern crate url_serde;
extern crate zip;
extern crate console;
extern crate number_prefix;
extern crate tokio;

pub mod template;
pub mod fetcher;
pub mod cli;

#[allow(deprecated)] // See https://github.com/rust-lang-nursery/error-chain/issues/254
pub mod errors {
    error_chain! {
        foreign_links {
            Io(::std::io::Error);
            ReqwestFail(::reqwest::Error);
            Config(::config::ConfigError);
            Zip(::zip::result::ZipError);
        }

        errors {
            Timeout {
                description("download timed out since no data received"),
                display("download timed out since no data received"),
            }

            StatusCode(code: ::reqwest::StatusCode) {
                description("HTTP response status code was not a success"),
                display("non-successful HTTP response status code: {}", code),
            }
        }
    }
}
