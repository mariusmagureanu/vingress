use handlebars::{to_json, Handlebars};
use serde::Serialize;
use serde_json::value::Map;

#[derive(Debug, PartialEq)]
pub struct UpdateError(String);

impl std::fmt::Display for UpdateError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Serialize)]
pub struct Backend {
    pub name: String,
    pub host: String,
    pub port: u16,
}

#[derive(Serialize)]
pub struct Vcl {
    pub template: String,
    pub file: String,
    pub content: String,
}

impl UpdateError {
    pub fn new(err_text: String) -> Self {
        UpdateError(err_text)
    }
}

impl Vcl {
    pub fn new(file: String, template: String) -> Self {
        Vcl {
            template,
            file,
            content: String::new(),
        }
    }
}

impl Backend {
    pub fn new(name: String, host: String, port: u16) -> Self {
        Backend { name, host, port }
    }
}

pub fn update(vcl: &mut Vcl, backends: Vec<Backend>) -> Option<UpdateError> {
    if backends.is_empty() {
        return Some(UpdateError("Backends cannot be empty".to_string()));
    }

    let mut hb = Handlebars::new();

    if let Err(e) = hb.register_template_file("vcl", vcl.template.clone()) {
        return Some(UpdateError(e.to_string()));
    }

    let mut vcl_data = Map::new();

    vcl_data.insert("backend".to_string(), to_json(backends));

    match hb.render("vcl", &vcl_data) {
        Ok(c) => vcl.content = c,
        Err(e) => return Some(UpdateError::new(e.to_string())),
    }

    None
}
