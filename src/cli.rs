use std::io;
use std::path::PathBuf;
use console::Term;
use std::string::ToString;

#[derive(StructOpt, Debug, Clone)]
#[structopt(name = "tempget", about = "Downloads files based on a template")]
pub struct CliOptions {
    #[structopt(default_value = "template.toml", parse(from_os_str))]
    /// The template file to use. By default, this is set to "template.toml".
    pub template_file: PathBuf,
    #[structopt(long = "no-extract")]
    /// When this flag is present, files are not extracted from the given zip
    /// files.
    pub no_extract: bool
}

pub enum DownloadStatus {
    Start(usize, Option<u64>),
    Progress(usize, usize),
    Finish(usize)
}

impl DownloadStatus {
    pub fn get_index(&self) -> &usize {
        match self {
            DownloadStatus::Start(idx, _) => idx,
            DownloadStatus::Progress(idx, _) => idx,
            DownloadStatus::Finish(idx) => idx
        }
    }
}

/// Contains information about the status of the download
pub struct FileDownloadProgress {
    /// The max size of the file, in bytes.
    pub max_size: Option<u64>,
    /// The current number of bytes downloaded.
    pub down_size: u64
}

impl FileDownloadProgress {
    pub fn new(max_size: Option<u64>) -> Self {
        FileDownloadProgress {
            max_size,
            down_size: 0
        }
    }

    /// Adds the given amount of progress to the current download size.
    pub fn inc(&mut self, b: u64) {
        self.down_size += b;
    }
}

/// A utility for rendering progress text.
///
/// Do _not_ interleave standard printing with `ProgressRender`, or else there
/// will be strange clearing behavior. Instead, call `ProgressRender.message`.
pub struct ProgressRender {
    lines: usize,
    term: Term
}

impl ProgressRender {
    pub fn with_term(term: Term) -> Self {
        ProgressRender {
            lines: 0,
            term: term
        }
    }

    pub fn stderr() -> Self {
        Self::with_term(Term::buffered_stderr())
    }

    /// Prints a line to the terminal buffer. This line will be cleared when
    /// `clear` is called.
    pub fn println<S: ToString>(&mut self, s: S) -> io::Result<()> {
        self.lines += 1;
        self.term.write_line(&s.to_string())
    }

    /// Flushes the buffered lines to the terminal.
    pub fn flush(&self) -> io::Result<()> {
        self.term.flush()
    }

    /// Deletes the lines printed with `println`.
    pub fn clear(&mut self) -> io::Result<()> {
        let res = self.term.clear_last_lines(self.lines);
        self.lines = 0;
        res
    }

    /// Prints the line to the terminal buffer. This line will _not_ be cleared
    /// when `clear` is called. Be careful to clear the terminal before using
    /// `message`.
    pub fn message<S: ToString>(&mut self, s: S) -> io::Result<()> {
        self.term.write_line(&s.to_string())
    }
}
