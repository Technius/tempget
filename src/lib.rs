#[macro_use]
extern crate failure;

pub mod template;
pub mod fetcher;
pub mod cli;

pub mod errors {
    pub type Result<T> = ::std::result::Result<T, failure::Error>;
    pub type Error = ::failure::Error;

    #[derive(Fail, Debug)]
    #[fail(display = "download timed out since no data received")]
    pub struct Timeout;

    #[derive(Fail, Debug)]
    #[fail(display = "non-successful HTTP response status code: {}", code)]
    pub struct StatusCode {
        pub code: ::reqwest::StatusCode
    }

    /// Constructs a `Timeout` error
    pub fn timeout() -> Error {
        (Timeout {}).into()
    }

    /// Constructs a `StatusCode` error
    pub fn status_code(code: ::reqwest::StatusCode) -> Error {
        (StatusCode { code }).into()
    }
}
