extern crate tempget;
extern crate reqwest;
extern crate pbr;
extern crate zip;
extern crate structopt;

use tempget::template;
use tempget::errors;
use tempget::cli::CliOptions;
use tempget::template::ExtractInfo;
use std::fs;
use std::io;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use structopt::StructOpt;

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
    let client = reqwest::Client::new();
    let requests = tempget::fetcher::get_template_requests(&templ);
    for (path_str, request) in requests {
        let path = Path::new(&path_str);
        if path.exists() {
            println!("{} exists, skipping", path_str);
            continue;
        }
        println!("Downloading {} to {}", request.url(), path_str);
        create_parent_dirs(&path)?;
        let response = client.execute(request)?;
        let max_size_opt = response.headers()
            .get::<reqwest::header::ContentLength>()
            .map(|cl| **cl);
        write_file(&path, response, &max_size_opt)?;
    }
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

fn write_file<R: Read>(file_path: &Path, mut input: R, max_size_opt: &Option<u64>) -> io::Result<()> {
    let mut file = fs::File::create(file_path)?;

    if let Some(max_size) = max_size_opt {
        // Need to manually copy because Write::broadcast is still unstable
        let mut buf = [0; 1024 * 1024]; // 1 MB buffer
        let mut progress = pbr::ProgressBar::new(*max_size);
        progress.set_units(pbr::Units::Bytes);
        loop {
            let len = input.read(&mut buf)?;
            if len == 0 {
                break;
            } else {
                file.write_all(&buf[..len])?;
            }
            progress.add(len as u64);
        }
        progress.finish_print("\n");
    } else {
        io::copy(&mut input, &mut file)?;
    }
    Ok(())
}

fn create_parent_dirs(file_path: &Path) -> io::Result<()> {
    if let Some(parent) = file_path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)?;
        }
    }
    Ok(())
}
