use reqwest::Method;
use reqwest::r#async::Request;
use crate::template::Template;
use std::collections::HashMap;

/// Generates a mapping of file to HTTP requests
pub fn get_template_requests(templ: &Template) -> HashMap<String, Request> {
    let mut data = HashMap::new();
    for (file_name, url) in &templ.retrieve {
        let url = url.clone().into_inner();
        let req = Request::new(Method::GET, url.clone());
        data.insert(file_name.clone(), req);
    }
    data
}
