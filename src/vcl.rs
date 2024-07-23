use handlebars::{to_json, Handlebars};
use serde::Serialize;
use serde_json::value::Map;

#[derive(Debug, Serialize)]
pub struct Backend {
    pub name: String,
    pub host: String,
    pub port: u16,
}

pub fn update(vcl_template: &str) {
    let mut hb = Handlebars::new();

    hb.register_template_file("vcl", vcl_template).unwrap();

    let b = Backend {
        name: "foobar".to_string(),
        host: "foobar.com".to_string(),
        port: 6081,
    };

    let mut ba: Vec<Backend> = vec![];
    ba.push(b);
    let mut data = Map::new();
    data.insert("backend".to_string(), to_json(ba));
    println!("{}", hb.render("vcl", &data).unwrap());
}
