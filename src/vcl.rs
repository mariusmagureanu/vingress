use handlebars::{to_json, Handlebars};
use serde::Serialize;
use serde_json::value::Map;
use std::{fs::File, io::Write};

const BACKEND: &str = "backend";
const VCL: &str = "vcl";

#[derive(Debug, PartialEq)]
pub struct UpdateError(String);

impl std::fmt::Display for UpdateError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct Backend {
    pub name: String,
    pub host: String,
    pub path: String,
    pub port: u16,
}

#[derive(Serialize)]
pub struct Vcl<'a> {
    pub template: &'a str,
    pub file: &'a str,
    pub content: String,
}

impl UpdateError {
    pub fn new(err_text: String) -> Self {
        UpdateError(err_text)
    }
}

impl<'a> Vcl<'a> {
    pub fn new(file: &'a str, template: &'a str) -> Self {
        Vcl {
            template,
            file,
            content: String::new(),
        }
    }
}

impl Backend {
    pub fn new(name: String, host: String, path: String, port: u16) -> Self {
        Backend {
            name,
            host,
            path,
            port,
        }
    }
}

pub fn update(vcl: &mut Vcl, backends: Vec<Backend>) -> Option<UpdateError> {
    if backends.is_empty() {
        return Some(UpdateError("Backends cannot be empty".to_string()));
    }

    let mut hb = Handlebars::new();

    if let Err(e) = hb.register_template_file(VCL, vcl.template) {
        return Some(UpdateError(e.to_string()));
    }

    let mut vcl_data = Map::new();

    vcl_data.insert(BACKEND.to_string(), to_json(backends));

    match hb.render(VCL, &vcl_data) {
        Ok(c) => vcl.content = c,
        Err(e) => return Some(UpdateError::new(e.to_string())),
    };

    match File::create(vcl.file) {
        Ok(mut f) => {
            let _ = f.write_all(vcl.content.as_bytes());
        }
        Err(e) => return Some(UpdateError(e.to_string())),
    };

    None
}
