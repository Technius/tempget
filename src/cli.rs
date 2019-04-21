use console::Term;
use reqwest::Url;
use std::io;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use number_prefix::NumberPrefix;
use structopt::StructOpt;

use crate::errors;

#[derive(StructOpt, Debug, Clone)]
#[structopt(name = "tempget", about = "Downloads files based on a template")]
pub struct CliOptions {
    #[structopt(parse(from_os_str))]
    /// The template file to use.
    pub template_file: PathBuf,
    #[structopt(long = "no-extract")]
    /// When this flag is present, files are not extracted from the given zip
    /// files.
    pub no_extract: bool,
    #[structopt(short = "p", long = "parallelism", default_value = "4")]
    /// The maximum number of files that should be downloaded simultaneously.
    pub parallelism: usize,
    /// The maximum amount of time (in seconds) to wait to connect or receive
    /// data before failing the download.
    #[structopt(long, default_value = "10")]
    pub timeout: u64
}

/// A message indicating the progress made by a file with the given id.
pub enum DownloadStatus {
    /// Initializing connection
    Init(usize),
    /// Download started
    Start(usize, Option<u64>),
    /// Download in progress, with the amount of bytes last downloaded and the timestamp
    Progress(usize, usize, Instant),
    /// Download finished
    Finish(usize),
    /// Download failed
    Failed(usize, errors::Error)
}

impl DownloadStatus {
    /// Returns the id of the file that this status represents.
    pub fn get_index(&self) -> &usize {
        match self {
            DownloadStatus::Init(idx) => idx,
            DownloadStatus::Start(idx, _) => idx,
            DownloadStatus::Progress(idx, _, _) => idx,
            DownloadStatus::Finish(idx) => idx,
            DownloadStatus::Failed(idx, _) => idx,
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
    /// The number of bytes downloaded during the last update.
    pub(self) last_update_size: u64,
    /// The rate of download (in bytes, rounded) during the last update.
    pub(self) last_update_rate: u64,
}

impl FileDownloadProgress {
    pub fn new(max_size: Option<u64>) -> Self {
        FileDownloadProgress {
            max_size,
            down_size: 0,
            last_update_time: Instant::now(),
            last_update_size: 0,
            last_update_rate: 0
        }
    }

    /// Adds the given amount of progress to the current download size.
    pub fn inc(&mut self, b: u64, timestamp: &Instant) {
        self.down_size += b;

        // The write stream may not send its updates in order, so don't update
        // the rate if we get a timestamp older than the last update.
        if timestamp < &self.last_update_time {
           return;
        }

        let passed = timestamp.duration_since(self.last_update_time);
        // Only update if time has passed
        // Use an update threshold to avoid rounding errors from small time deltas.
        const UPDATE_THRESHOLD: Duration = Duration::from_millis(200);
        if passed >= UPDATE_THRESHOLD {
            let delta = self.down_size - self.last_update_size;
            self.last_update_rate = (1000.0 * delta as f64 / (passed.as_millis() as f64)) as u64;
            self.last_update_time = timestamp.clone();
            self.last_update_size = self.down_size;
        }
    }
}

/// A utility for rendering progress text.
///
/// Do _not_ interleave standard printing with `ProgressRender`, or else there
/// will be strange clearing behavior. Instead, call `ProgressRender.message`.
pub struct ProgressRender {
    /// The number of lines that are being used to render progress.
    lines: usize,
    /// The terminal used to render text.
    term: Term
}

impl ProgressRender {
    pub fn with_term(term: Term) -> Self {
        ProgressRender {
            lines: 0,
            term: term
        }
    }

    /// Creates a `ProgressRender` that renders progress on stderr.
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

/// Keeps track of the download progress for each file being downloaded. The
/// progress of each file is treated as a state machine, where the states
/// consist of `DownloadState`s. The states can be updated by calling the
/// appropriate methods, such as `mark_current` or `inc_progress`.
pub struct ProgressState {
    /// Maps file id to location on disk and URL
    pub file_info: HashMap<usize, (PathBuf, Url)>,
    /// Tracks file download state information
    states: HashMap<usize, DownloadState>,
}

/// Download progress state for one file.
enum DownloadState {
    /// The file is currently queued for download.
    Queued,
    /// Currently attempting to connect to the URL where the file is located.
    Connecting,
    /// The download is in progress.
    InProgress(FileDownloadProgress),
    /// The download is completed.
    Finished,
    /// The download failed due to some error.
    Failed(errors::Error)
}

impl ProgressState {
    pub fn new(file_info: HashMap<usize, (PathBuf, Url)>) -> Self {
        let init_state = file_info.iter()
            .map(|(idx, _)| (idx.clone(), DownloadState::Queued))
            .collect();
        ProgressState {
            states: init_state,
            file_info: file_info,
        }
    }

    /// Returns true when each file is downloaded or has failed to download.
    pub fn is_done(&self) -> bool {
        self.file_info.len() == self.ended().len()
    }

    /// Returns the indexes of all of the finished downloads.
    pub fn finished(&self) -> HashSet<usize> {
        self.states.iter()
            .filter_map(|(idx, st)| {
                if let DownloadState::Finished = st { Some(idx.clone()) } else { None }
            })
            .collect()
    }

    /// Returns the indexes of all of the failed downloads.
    pub fn failed(&self) -> HashMap<usize, &errors::Error> {
        self.states.iter()
            .filter_map(|(idx, st)| {
                if let DownloadState::Failed(err) = st { Some((idx.clone(), err)) } else { None }
            })
            .collect()
    }

    /// Returns the indexes of all finished or failed downloads.
    pub fn ended(&self) -> HashSet<usize> {
        self.states.iter()
            .filter_map(|(idx, st)| {
                match st {
                    DownloadState::Failed(_) => Some(idx.clone()),
                    DownloadState::Finished => Some(idx.clone()),
                    _ => None
                }
            })
            .collect()
    }

    /// Returns the indexes of all files being processed (not queued, finished, or failed)
    fn processing(&self) -> HashSet<usize> {
        self.states.iter()
            .filter_map(|(idx, st)| match st {
                DownloadState::Connecting => Some(idx.clone()),
                DownloadState::InProgress(_) => Some(idx.clone()),
                _ => None
            })
            .collect()
    }

    /// Marks the file with the given id as currently connecting if the file
    /// download is being queued.
    pub fn mark_connect(&mut self, id: &usize) {
        self.states.entry(id.clone()).and_modify(|st| {
            if let DownloadState::Queued = st {
                *st = DownloadState::Connecting;
            }
        });
    }

    /// Marks the file with the given id as being downloaded if the file
    /// download has not started yet.
    pub fn mark_current(&mut self, id: &usize, size_opt: Option<u64>) {
        self.states.entry(id.clone()).and_modify(|st| {
            if let DownloadState::Connecting = st {
                *st = DownloadState::InProgress(FileDownloadProgress::new(size_opt));
            }
        });
    }

    /// Marks the file with the given id as finished downloading. Does nothing
    /// if the file is not downloading.
    pub fn mark_finished(&mut self, id: &usize) {
        self.states.entry(id.clone()).and_modify(|st| {
            if let DownloadState::InProgress(_) = st {
                *st = DownloadState::Finished;
            }
        });
    }

    /// Marks the file with the given id as failed. Does nothing if the file is
    /// already marked as such.
    pub fn mark_failed(&mut self, id: &usize, err: errors::Error) {
        self.states.entry(id.clone()).and_modify(|st| {
            if let DownloadState::Failed(_) = st {} else {
                *st = DownloadState::Failed(err);
            }
        });
    }

    /// Increases the progress of the file with the given id if it is being
    /// downloaded, or does nothing otherwise.
    pub fn inc_progress(&mut self, id: usize, amount: u64, timestamp: &Instant)  {
        self.states.entry(id.clone()).and_modify(|st| {
            if let DownloadState::InProgress(prog) = st {
                prog.inc(amount, timestamp);
            }
        });
    }

    #[inline]
    /// Returns the total number of files tracked by this `ProgressState`.
    pub fn total(&self) -> usize {
        return self.file_info.len()
    }

    /// Returns the URL of the file with the given id, or `None` if there is no
    /// such file.
    pub fn get_url(&self, id: &usize) -> Option<&reqwest::Url> {
        return self.file_info.get(id).map(|(_, u)| u)
    }

    /// Returns the path to which the file with the given id will be downloaded
    /// to, or `None` if there is no such file.
    pub fn get_path(&self, id: &usize) -> Option<&Path> {
        return self.file_info.get(id).map(|(p, _)| p.as_path())
    }

    /// Returns the error that caused the file with the given id to fail, or
    /// `None` if there is no such file or the file has not failed to download.
    pub fn get_failure_error(&self, id: &usize) -> Option<&errors::Error> {
        self.states.get(id).and_then(|st| {
            if let DownloadState::Failed(err) = st { Some(err) } else { None }
        })
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
        let considered = self.processing().into_iter().collect::<Vec<usize>>();
        if !considered.is_empty() {
            lines.push(format!(
                "Downloading: ({}/{})", self.ended().len(), self.total()));
            for id in considered {
                let path = self.get_path(&id).unwrap();
                let path_str = path.to_string_lossy();
                match self.states.get(&id).unwrap() {
                    DownloadState::Connecting => {
                        lines.push(format!("{}\tconnecting", path_str));
                    },
                    DownloadState::InProgress(progress) => {
                        let down_bytes = Self::display_bytes(progress.down_size);
                        let rate_bytes = Self::display_bytes(progress.last_update_rate);
                        if let Some(max_size) = &progress.max_size {
                            let total_bytes = Self::display_bytes(*max_size);
                            let percent = 100.0 * (progress.down_size as f64)
                                / (*max_size as f64);
                            lines.push(format!("{}\t{} / {} ({:.2}%), {}/s",
                                               path_str, down_bytes, total_bytes, percent, rate_bytes));
                        } else {
                            lines.push(format!("{}\t{}, {}/s", path_str, down_bytes, rate_bytes));
                        }
                    },
                    _ => ()
                };
            }
        }
        lines
    }
}
