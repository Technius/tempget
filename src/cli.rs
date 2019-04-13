use console::Term;
use reqwest::Url;
use std::io;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Instant;
use number_prefix::NumberPrefix;

#[derive(StructOpt, Debug, Clone)]
#[structopt(name = "tempget", about = "Downloads files based on a template")]
pub struct CliOptions {
    #[structopt(default_value = "template.toml", parse(from_os_str))]
    /// The template file to use. By default, this is set to "template.toml".
    pub template_file: PathBuf,
    #[structopt(long = "no-extract")]
    /// When this flag is present, files are not extracted from the given zip
    /// files.
    pub no_extract: bool,
    #[structopt(short = "p", long = "parallelism")]
    /// The maximum number of files that should be downloaded simultaneously.
    pub parallelism: usize
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
    pub down_size: u64,
    /// The last time this progress was updated.
    pub(self) last_update_time: Instant,
    /// The rate of download (in bytes, rounded) during the last update.
    pub(self) last_update_rate: u64
}

impl FileDownloadProgress {
    pub fn new(max_size: Option<u64>) -> Self {
        FileDownloadProgress {
            max_size,
            down_size: 0,
            last_update_time: Instant::now(),
            last_update_rate: 0
        }
    }

    /// Adds the given amount of progress to the current download size.
    pub fn inc(&mut self, b: u64) {
        self.down_size += b;
        let now = Instant::now();
        let passed = now.duration_since(self.last_update_time).as_secs();
        // Only update if time has passed
        if passed > 0 {
            self.last_update_rate = b / passed;
            self.last_update_time = now;
        }
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
    pub fn println<S: AsRef<str>>(&mut self, s: S) -> io::Result<()> {
        self.lines += 1;
        self.term.write_line(s.as_ref())
    }

    /// Prints multiple lines to the terminal buffer. These lines will be
    /// cleared when `clear` is called.
    pub fn println_multi<S: AsRef<str>>(&mut self, lines: &[S]) -> io::Result<()> {
        self.lines += lines.len();
        for s in lines {
            self.term.write_line(s.as_ref())?;
        }
        Ok(())
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
    pub fn message<S: AsRef<str>>(&mut self, s: S) -> io::Result<()> {
        self.term.write_line(s.as_ref())
    }
}

/// Contains information about the download progress.
pub struct ProgressState {
    /// Maps file id to location on disk and URL
    pub file_info: HashMap<usize, (PathBuf, Url)>,
    /// Maps id to download progress
    pub current: HashMap<usize, FileDownloadProgress>,
    /// Tracks ids of files done downloading
    pub finished: HashSet<usize>
}

impl ProgressState {
    pub fn new(file_info: HashMap<usize, (PathBuf, Url)>) -> Self {
        let size = file_info.len();
        ProgressState {
            current: HashMap::with_capacity(size),
            file_info: file_info,
            finished: HashSet::with_capacity(size)
        }
    }

    pub fn is_done(&self) -> bool {
        self.file_info.len() == self.finished.len()
    }

    /// Marks the file with the given id as being downloaded if the file
    /// download has not started yet.
    pub fn mark_current(&mut self, id: &usize, size_opt: Option<u64>) {
        self.current.entry(id.clone()).or_insert(FileDownloadProgress::new(size_opt));
    }

    /// Marks the file with the given id as finished downloading. Does nothing
    /// if the file is already marked as such.
    pub fn mark_finished(&mut self, id: &usize) {
        if let Some(_) = self.current.remove(id) {
            self.finished.insert(id.clone());
        }
    }

    /// Increases the progress of the file with the given id if it is being
    /// downloaded, or does nothing otherwise.
    pub fn inc_progress(&mut self, id: usize, amount: u64)  {
        self.current.entry(id).and_modify(|prog| prog.inc(amount));
    }

    #[inline]
    pub fn total(&self) -> usize {
        return self.file_info.len()
    }

    pub fn get_url(&self, id: &usize) -> Option<&reqwest::Url> {
        return self.file_info.get(id).map(|(_, u)| u)
    }

    pub fn get_path(&self, id: &usize) -> Option<&Path> {
        return self.file_info.get(id).map(|(p, _)| p.as_path())
    }

    /// Displays the size number, along with its units.
    fn display_bytes(size: u64) -> String {
        match NumberPrefix::decimal(size as f64) {
            NumberPrefix::Standalone(_) => format!("{} bytes", size),
            NumberPrefix::Prefixed(units, n) => format!("{:.2} {}B", n, units)
        }
    }

    /// Renders the download progress to a `Vec<String>`
    pub fn render(&self) -> Vec<String> {
        let mut lines = Vec::new();
        if !self.current.is_empty() {
            lines.push(format!(
                "Downloading: ({}/{})", self.finished.len(), self.total()));
            for (id, progress) in &self.current {
                let path = self.get_path(id).unwrap();
                let path_str = path.to_string_lossy();
                if let Some(max_size) = &progress.max_size {
                    let down_bytes = Self::display_bytes(progress.down_size);
                    let total_bytes = Self::display_bytes(*max_size);
                    let rate_bytes = Self::display_bytes(progress.last_update_rate);
                    let percent = 100.0 * (progress.down_size as f64)
                        / (*max_size as f64);
                    lines.push(format!("{}\t{} / {} ({:.2}%), {}/s",
                                       path_str, down_bytes, total_bytes, percent, rate_bytes));
                } else {
                    lines.push(format!("{}", path_str));
                }
            }
        }
        lines
    }
}
