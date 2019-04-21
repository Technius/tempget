#[macro_use]
extern crate failure;

pub mod template;
pub mod fetcher;
pub mod cli;

pub mod errors {
    pub type Result<T> = ::std::result::Result<T, failure::Error>;
    pub type Error = ::failure::Error;

    #[derive(Fail, Debug)]
    #[fail(display = "download timed out: no data received in a {} second interval", _0)]
    /// Download timed out. Annotated with the length of the timeout (in seconds)
    pub struct Timeout(u64);

    #[derive(Fail, Debug)]
    #[fail(display = "non-successful HTTP response status code: {}", code)]
    /// The request returned a non-2XX status code.
    pub struct StatusCode {
        pub code: ::reqwest::StatusCode
    }

    /// Constructs a `Timeout` error
    pub fn timeout(seconds: u64) -> Error {
        Timeout(seconds).into()
    }

    /// Constructs a `StatusCode` error
    pub fn status_code(code: ::reqwest::StatusCode) -> Error {
        (StatusCode { code }).into()
    }
}
