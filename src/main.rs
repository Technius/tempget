extern crate tempget;
extern crate reqwest;
extern crate pbr;
extern crate zip;
extern crate structopt;
extern crate tokio;
extern crate tokio_codec;
extern crate futures;

use futures::{Future, Stream};
use tempget::template;
use tempget::errors;
use tempget::cli::CliOptions;
use tempget::template::ExtractInfo;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use structopt::StructOpt;

use reqwest::unstable::async as req;

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
    do_fetch(&templ)?;
    if !options.no_extract {
        do_extract(templ)?;
    }
    Ok(())
}

fn do_fetch(templ: &template::Template) -> errors::Result<()> {
    const NUM_CONNECTIONS: usize = 5;
    let pool_builder = tokio::executor::thread_pool::Builder::new();
    let mut runtime = tokio::runtime::Builder::new()
        .threadpool_builder(pool_builder)
        .build()?;
    let client = req::Client::new();
    let mut requests = Vec::<(PathBuf, req::Request)>::new();
    for (path_str, request) in tempget::fetcher::get_template_requests(&templ) {
        let path = Path::new(&path_str);
        if path.exists() {
            println!("{} exists, skipping", path_str);
            continue;
        }
        requests.push((path.to_owned(), request));
    }

    let tasks = futures::stream::iter_ok(requests)
        .map(move |(path, request)| {
            println!("Downloading {} to {:#?}", request.url(), path);
            client.execute(request).map(|r| (path, r))
        })
        .buffer_unordered(NUM_CONNECTIONS)
        .from_err::<errors::Error>()
        .for_each(move |(path, response)| {
            println!("Beginning download of {:#?}", path);
            let max_size_opt = response.headers()
                .get(reqwest::header::CONTENT_LENGTH)
                .and_then(|ct_len| ct_len.to_str().ok())
                .and_then(|ct_len| ct_len.parse().ok());
            write_file(&path, response, &max_size_opt)
        });

    runtime.block_on(tasks)?;
    runtime.shutdown_on_idle().wait().expect("Could not shutdown tokio runtime");
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

fn write_file(file_path: &Path, response: req::Response, max_size_opt: &Option<u64>) -> impl Future<Item = (), Error = errors::Error> {

    if let Some(_max_size) = max_size_opt {
        // TODO: progress bar stuff
    } else {
        // io::copy(&mut input, &mut file)?;
    }

    let file_path = file_path.to_owned();

    futures::future::result(create_parent_dirs(&file_path).map(|_| file_path.clone()))
        .from_err::<errors::Error>()
        .and_then(|path| tokio::fs::File::create(path).from_err::<errors::Error>())
        .and_then(move |file| {
            println!("Starting file i/o for {:#?}", file_path);
            let file_sink = tokio_codec::FramedWrite::new(file, tokio_codec::BytesCodec::new());
            response.into_body()
                .from_err::<errors::Error>()
                .map(|chunk| (&*chunk).into())
                .forward(file_sink)
                .map(move |_| {
                    println!("Done downloading {:#?}", file_path);
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
