use futures::{Future, Stream};
use reqwest::r#async as req;
use std::fs;
use std::io;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, SyncSender};
use std::time::{Duration, Instant};
use structopt::StructOpt;
use tokio::prelude::FutureExt;
use tokio::prelude::StreamExt;

use tempget::template;
use tempget::errors;
use tempget::cli::*;
use tempget::template::ExtractInfo;

/// Application entry point.
fn main() {
    let options = CliOptions::from_args();
    let res = run(&options);
    match res {
        Ok(()) => {},
        Err(err) => {
            println!("Error: {}", err);
            std::process::exit(1);
        }
    }
}

/// Run the program with the given options.
fn run(options: &CliOptions) -> errors::Result<()> {
    let templ = template::Template::from_file(&options.template_file)?;

    let final_state = do_fetch(options, &templ)?;
    // Exit with exit code 1 if any of the downloads failed
    let failed = final_state.failed();
    if failed.len() > 0 {
        let msgs = failed.iter()
            .map(|(id, err)| {
                let path = final_state.get_path(id).unwrap();
                format!("\t{}: {}", path.to_string_lossy(), err)
            })
            .collect::<Vec<String>>();
        println!("The following downloads failed:\n{}", msgs.join("\n"));
        std::process::exit(1);
    } else if !options.no_extract {
        do_extract(templ)?;
    }
    Ok(())
}

/// Download the files specified in the `retrieve` section of the template and
/// display the progress. Returns the final `ProgressState` containing all file
/// download progress information.
fn do_fetch(options: &CliOptions, templ: &template::Template) -> errors::Result<ProgressState> {
    let timeout_dur = Duration::from_secs(options.timeout);
    let mut runtime = tokio::runtime::Builder::new().build()?;
    let client = req::Client::builder()
        .connect_timeout(timeout_dur)
        .build()?;
    let mut requests = Vec::<(usize, PathBuf, req::Request)>::new();
    let mut idx: usize = 0;
    for (path_str, request) in tempget::fetcher::get_template_requests(&templ) {
        let path = Path::new(&path_str);
        if path.exists() {
            println!("{} exists, skipping", path_str);
            continue;
        }
        requests.push((idx, path.to_owned(), request));
        idx += 1;
    }

    let file_info: HashMap<usize, _> = requests.iter()
        .map(|(idx, p, req)| (idx.clone(), (p.clone(), req.url().clone())))
        .collect();
    // `sync_channel` instead of `channel` since status message order is
    // important
    let (prog_tx, prog_rx) = std::sync::mpsc::sync_channel::<DownloadStatus>(1000);
    // The futures might complete before progress tracking even begins, which
    // causes the Receiver to fail since all senders will be dropped. This keeps
    // the Receiver open until progress is reported.
    let keep_alive = prog_tx.clone();

    // TODO: refactor into separate function (Vec<Requests> -> Stream<Vec<()>>)
    let tasks = futures::stream::iter_ok(requests)
        .map(move |(idx, path, request)| {
            let prog_tx = prog_tx.clone();
            prog_tx.send(DownloadStatus::Init(idx)).unwrap();
            let idx_err = idx.clone();
            let err_tx = prog_tx.clone();
            let timeout_secs = timeout_dur.as_secs();
            client
                .execute(request)
                .timeout(timeout_dur)
                .map_err(move |timer_err| {
                    let err_res: errors::Error =
                        if let Some(e) = timer_err.into_inner() {
                            e.into()
                        } else {
                            errors::timeout(timeout_secs)
                        };
                    err_res
                })
                .and_then(|response| {
                    let status = response.status();
                    if !status.is_success() {
                        Err(errors::status_code(status))
                    } else {
                        Ok(response)
                    }
                })
                .and_then(move |response| {
                    let size_opt = response.headers()
                        .get(reqwest::header::CONTENT_LENGTH)
                        .and_then(|ct_len| ct_len.to_str().ok())
                        .and_then(|ct_len| ct_len.parse().ok());

                    prog_tx.send(DownloadStatus::Start(idx, size_opt)).unwrap();

                    write_file(&path, response, idx, prog_tx, timeout_dur.clone())
                })
                .then(move |res| match res {
                    Ok(_) => Ok(()),
                    Err(err) => {
                        // We cannot let the stream actually have an error, since
                        // that would terminate all downloads. Instead, handle the
                        // error gracefully here.
                        err_tx.send(DownloadStatus::Failed(idx_err, err)).unwrap();
                        Ok(())
                    }
                })
        })
        .buffer_unordered(options.parallelism);

    let f = tasks.collect().map(|_| ());
    runtime.spawn(f);

    let final_state = block_progress(file_info, prog_rx)?;
    drop(keep_alive);
    runtime.shutdown_on_idle().wait().expect("Could not shutdown tokio runtime");
    Ok(final_state)
}

/// Blocks the current thread and renders download progress until all files have
/// been downloaded.
fn block_progress(file_info: HashMap<usize, (PathBuf, reqwest::Url)>,  rx: Receiver<DownloadStatus>)
                  -> io::Result<ProgressState> {
    let mut state = ProgressState::new(file_info);
    // Throttle rendering so we don't spend so much time reporting progress
    let mut last_render = std::time::Instant::now();
    let mut renderer = ProgressRender::stderr();
    while !state.is_done() {
        use crate::DownloadStatus::*;
        match rx.recv() {
            Ok(Init(idx)) => {
                state.mark_connect(&idx);
                renderer.clear()?;
                renderer.println_multi(&state.render())?;
                renderer.flush()?;
            },
            Ok(Start(idx, size_opt)) => {
                state.mark_current(&idx, size_opt);
                renderer.clear()?;
                let url = state.get_url(&idx).unwrap();
                let path = state.get_path(&idx).unwrap().display();
                renderer.message(format!("Downloading {} to {:#?}", url, path))?;
                renderer.println_multi(&state.render())?;
                renderer.flush()?;
            },
            Ok(Progress(idx, down_size, timestamp)) => {
                state.inc_progress(idx, down_size as u64, &timestamp);
                let now = std::time::Instant::now();
                if now - last_render > std::time::Duration::from_millis(200) {
                    last_render = now;
                    renderer.clear()?;
                    renderer.println_multi(&state.render())?;
                    renderer.flush()?;
                }
            },
            Ok(Finish(idx)) => {
                state.mark_finished(&idx);
                renderer.clear()?;
                let download_path = state.get_path(&idx).unwrap().display();
                renderer.message(format!("Finished downloading {}", download_path))?;
                renderer.println_multi(&state.render())?;
                renderer.flush()?;
            },
            Ok(Failed(idx, err)) => {
                state.mark_failed(&idx, err);
                renderer.clear()?;
                let download_path = state.get_path(&idx).unwrap().display();
                let err = state.get_failure_error(&idx).unwrap();
                renderer.message(format!("Failed to download {}: {}", download_path, err))?;
                renderer.println_multi(&state.render())?;
                renderer.flush()?;
            },
            Err(_) => break
        }
    };
    renderer.clear()?;
    Ok(state)
}

/// Extract all files specified in the template file. Note that extraction is
/// currently synchronous.
fn do_extract(templ: template::Template) -> errors::Result<()> {
    for (archive, info) in &templ.extract {
        let file = fs::File::open(Path::new(archive))?;
        let mut zip_archive = zip::read::ZipArchive::new(file)?;
        let mut extract_files = Vec::<(usize, PathBuf)>::new();
        match info {
            ExtractInfo::Directory(d) => {
                let dest_dir = Path::new(d);
                for i in 0..zip_archive.len() {
                    let f = zip_archive.by_index(i)?;
                    if !f.name().ends_with("/") {
                        // Don't add directories
                        extract_files.push((i, dest_dir.join(Path::new(f.name()))));
                    }
                }
            },
            ExtractInfo::Mapping(files) => {
                for i in 0..zip_archive.len() {
                    let f = zip_archive.by_index(i)?;
                    if let Some(dest) = files.get(f.name()) {
                        extract_files.push((i, Path::new(dest).to_owned()));
                    }
                }
            }
        }

        for (index, dest_path) in extract_files {
            let mut f = zip_archive.by_index(index)?;
            if dest_path.exists() {
                println!("{} already exists, skipping", dest_path.to_string_lossy());
                continue;
            }
            create_parent_dirs(&dest_path)?;
            println!("Extracting {} to {}", f.name(), dest_path.to_string_lossy());
            let mut dest_file = fs::File::create(&dest_path)?;
            io::copy(&mut f, &mut dest_file)?;
        }
    }
    Ok(())
}

/// Returns a `Future` that represents asynchronously writing the contents of
/// the `Response` to the given file path. The resulting value is the given file
/// path.
fn write_file(file_path: &Path,
              response: req::Response,
              idx: usize,
              prog_tx: SyncSender<DownloadStatus>,
              timeout: Duration)
              -> impl Future<Item = (usize, SyncSender<DownloadStatus>), Error = errors::Error> {
    let file_path = file_path.to_owned();

    futures::future::result(create_parent_dirs(&file_path).map(|_| file_path.clone()))
        .from_err::<errors::Error>()
        .and_then(|path| tokio::fs::File::create(path).from_err::<_>())
        .and_then(move |file| {
            let codec = tokio::codec::BytesCodec::new();
            let file_sink = tokio::codec::FramedWrite::new(file, codec);
            let prog_tx_prog = prog_tx.clone();
            response.into_body()
                .from_err::<_>()
                .inspect(move |chunk| {
                    prog_tx_prog.send(DownloadStatus::Progress(
                        idx, chunk.len(), Instant::now())).unwrap();
                })
                .map(|chunk| (&*chunk).into())
                .timeout(timeout)
                .map_err(move |timer_err| timer_err.into_inner().unwrap_or(
                    errors::timeout(timeout.as_secs())))
                .forward(file_sink)
                .map(move |_| {
                    prog_tx.send(DownloadStatus::Finish(idx)).unwrap();
                    (idx, prog_tx)
                })
        })
}

/// Create all parent directories of the given path.
fn create_parent_dirs(file_path: &Path) -> io::Result<()> {
    if let Some(parent) = file_path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)?;
        }
    }
    Ok(())
}
