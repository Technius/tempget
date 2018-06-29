use reqwest;
use reqwest::Response;
use template::Template;
use std::collections::HashMap;

pub fn fetch_template(templ: &Template) -> reqwest::Result<HashMap<String, Response>> {
    let client = reqwest::Client::new();
    let mut data = HashMap::new();
    for (file_name, url) in &templ.retrieve {
        let url = url.clone().into_inner();
        let text = client.get(url.clone()).send()?;
        data.insert(file_name.clone(), text);
    }
    Ok(data)
}
