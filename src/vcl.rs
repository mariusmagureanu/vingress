use handlebars::Handlebars;

fn load_template(vcl_template: &str) {
    let mut hb = Handlebars::new();

    hb.register_template_file("vcl", vcl_template); 
}
