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

    #[derive(Fail, Debug)]
    /// Files which have failed to download
    pub struct DownloadsFailed {
        pub files: Vec<(std::path::PathBuf, String)>
    }

    impl std::fmt::Display for DownloadsFailed {
        fn fmt(&self, ft: &mut std::fmt::Formatter) -> std::fmt::Result {
            let msgs = self.files.iter()
                .map(|(f, err)| format!("\t{}: {}", f.display(), err))
                .collect::<Vec<String>>();
            write!(ft, "the following downloads failed:\n{}", msgs.join("\n"))
        }
    }

    /// Constructs a `Timeout` error
    pub fn timeout(seconds: u64) -> Error {
        Timeout(seconds).into()
    }

    /// Constructs a `StatusCode` error
    pub fn status_code(code: ::reqwest::StatusCode) -> Error {
        (StatusCode { code }).into()
    }

    /// Constructs a `DownloadsFailed` error
    pub fn download_failed(files: Vec<(std::path::PathBuf, String)>) -> Error {
        DownloadsFailed { files }.into()
    }
}
