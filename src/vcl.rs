use handlebars::Handlebars;

#[derive(Debug)]
pub struct Backend {
    pub name: String,
    pub host: String,
    pub port: u16,
}

pub fn load_template(vcl_template: &str) {
    let mut hb = Handlebars::new();

    let _ = hb.register_template_file("vcl", vcl_template);
}
