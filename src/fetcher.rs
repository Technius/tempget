use reqwest;
use reqwest::Response;
use template::Template;
use std::collections::HashMap;

pub fn fetch_template(templ: &Template) -> reqwest::Result<HashMap<String, Response>> {
    let client = reqwest::Client::new();
    let mut data = HashMap::new();
    for f in &templ.files {
        let text = client.get(f.url.clone()).send()?;
        data.insert(f.file_name.clone(), text);
    }
    Ok(data)
}
