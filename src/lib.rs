#[macro_use]
extern crate error_chain;

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
