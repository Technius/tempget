extern crate tempget;
extern crate reqwest;

use tempget::template;
use tempget::errors;
use std::fs;
use std::io;
use std::path::Path;

fn main() {
    // TODO: use error-chain
    let res = template::Template::from_file("hello.toml");
    match res {
        Ok(templ) => {
            let res = do_fetch(templ);
            println!("{:?}", res);
        },
        Err(err) => {
            println!("Error: {}", err)
        }
    }
}

fn do_fetch(templ: template::Template) -> errors::Result<()> {
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
        download_file(&path, response)?;
    }
    Ok(())
}

fn download_file(file_path: &Path, mut resp: reqwest::Response) -> io::Result<()> {
    let mut file = fs::File::create(file_path)?;
    io::copy(&mut resp, &mut file)?;
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
