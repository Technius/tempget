extern crate tempget;
extern crate reqwest;
extern crate zip;
extern crate structopt;
extern crate tokio;
extern crate tokio_codec;
extern crate futures;
extern crate console;

use futures::{Future, Stream};
use reqwest::async as req;
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

fn main() {
    let options = CliOptions::from_args();
    let res = run(&options);
    match res {
        Ok(()) => {},
        Err(err) => {
            println!("Error: {}", err)
        }
    }
}

fn run(options: &CliOptions) -> errors::Result<()> {
    let templ = template::Template::from_file(&options.template_file)?;
    do_fetch(options, &templ)?;
    if !options.no_extract {
        do_extract(templ)?;
    }
    Ok(())
}

fn do_fetch(options: &CliOptions, templ: &template::Template) -> errors::Result<()> {
    const REQUEST_TIMEOUT : u64 = 10; // seconds
    let mut runtime = tokio::runtime::Builder::new().build()?;
    let client = req::Client::builder()
        .connect_timeout(Duration::from_secs(REQUEST_TIMEOUT))
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
    let err_tx = prog_tx.clone();
    // The futures might complete before progress tracking even begins, which
    // causes the Receiver to fail since all senders will be dropped. This keeps
    // the Receiver open until progress is reported.
    let keep_alive = prog_tx.clone();

    let tasks = futures::stream::iter_ok(requests)
        .map(move |(idx, path, request)| {
            let prog_tx = prog_tx.clone();
            prog_tx.send(DownloadStatus::Init(idx)).unwrap();
            client
                .execute(request)
                .from_err::<errors::Error>()
                .timeout(std::time::Duration::from_secs(REQUEST_TIMEOUT))
                .map_err(|timer_err| timer_err.into_inner().unwrap_or(
                    errors::ErrorKind::Timeout.into()))
                .map(|r| (path, r))
                .and_then(|(path, response)| {
                    let status = response.status();
                    if !status.is_success() {
                        Err(errors::ErrorKind::StatusCode(status).into())
                    } else {
                        Ok((path, response))
                    }
                })
                .and_then(move |(path, response)| {
                    let size_opt = response.headers()
                        .get(reqwest::header::CONTENT_LENGTH)
                        .and_then(|ct_len| ct_len.to_str().ok())
                        .and_then(|ct_len| ct_len.parse().ok());

                    prog_tx.send(DownloadStatus::Start(idx, size_opt)).unwrap();

                    write_file(&path, response, idx, prog_tx)
                })
                .map_err(move |e| (idx, e))
        })
        .buffer_unordered(options.parallelism);

    let f = futures::future::ok(())
        .and_then(move |_| tasks.collect().map(|_| ()))
        .map_err(move |(idx, err)| {
            err_tx.send(DownloadStatus::Failed(idx, err)).unwrap();
        });
    runtime.spawn(f);

    block_progress(file_info, prog_rx)?;
    drop(keep_alive);
    runtime.shutdown_on_idle().wait().expect("Could not shutdown tokio runtime");
    Ok(())
}

/// Blocks and renders download progress until all files have been downloaded.
fn block_progress(file_info: HashMap<usize, (PathBuf, reqwest::Url)>,  rx: Receiver<DownloadStatus>)
                  -> io::Result<()> {
    let mut state = ProgressState::new(file_info);
    // Throttle rendering so we don't spend so much time reporting progress
    let mut last_render = std::time::Instant::now();
    let mut renderer = ProgressRender::stderr();
    while !state.is_done() {
        use DownloadStatus::*;
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
    Ok(())
}

fn do_extract(templ: template::Template) -> errors::Result<()> {
    for (archive, info) in &templ.extract {
        let mut file = fs::File::open(Path::new(archive))?;
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
fn write_file(file_path: &Path, response: req::Response, idx: usize, prog_tx: SyncSender<DownloadStatus>) -> impl Future<Item = (usize, SyncSender<DownloadStatus>), Error = errors::Error> {
    let file_path = file_path.to_owned();

    futures::future::result(create_parent_dirs(&file_path).map(|_| file_path.clone()))
        .from_err::<errors::Error>()
        .and_then(|path| tokio::fs::File::create(path).from_err::<_>())
        .and_then(move |file| {
            let codec = tokio_codec::BytesCodec::new();
            let file_sink = tokio_codec::FramedWrite::new(file, codec);
            let prog_tx_prog = prog_tx.clone();
            const READ_TIMEOUT: u64 = 10;
            response.into_body()
                .from_err::<_>()
                .inspect(move |chunk| {
                    prog_tx_prog.send(DownloadStatus::Progress(
                        idx, chunk.len(), Instant::now())).unwrap();
                })
                .map(|chunk| (&*chunk).into())
                .timeout(std::time::Duration::from_secs(READ_TIMEOUT))
                .map_err(|timer_err| timer_err.into_inner().unwrap_or(
                    errors::ErrorKind::Timeout.into()))
                .forward(file_sink)
                .map(move |_| {
                    prog_tx.send(DownloadStatus::Finish(idx)).unwrap();
                    (idx, prog_tx)
                })
        })
}

fn create_parent_dirs(file_path: &Path) -> io::Result<()> {
    if let Some(parent) = file_path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)?;
        }
    }
    Ok(())
}
