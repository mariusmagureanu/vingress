use handlebars::{to_json, Handlebars};
use log::info;
use serde::Serialize;
use serde_json::value::Map;
use std::{fs::File, io::Write, process::Command};

const BACKEND: &str = "backend";
const SNIPPET: &str = "snippet";
const VCL: &str = "vcl";
const RELOAD_COMMAND: &str = "varnishreload";

#[derive(Debug, PartialEq)]
pub struct UpdateError(String);

impl std::fmt::Display for UpdateError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

///
/// Backend is a type that translates an Ingress backend
/// into a Varnish backend.
///
/// E.g. an Ingress with 3 backends will
/// yield 4 Varnish backends in the vcl.
///
/// https://kubernetes.io/docs/concepts/services-networking/ingress/#the-ingress-resource
///
/// See vcl.hbs template file in this repository.
#[derive(Debug, Serialize, Clone)]
pub struct Backend {
    /// The namespace where the Ingress
    /// object is located.
    pub namespace: String,

    /// The name of the backend which
    /// will then be used as a backend
    /// hint in the vcl.
    pub name: String,

    /// Host as defined in the Ingress
    /// rules.
    pub host: String,

    /// Path as defined in the Ingress
    /// rules.
    ///
    /// https://kubernetes.io/docs/concepts/services-networking/ingress/#path-types
    ///
    /// Note: path-types are not yet accounted
    /// for at the time of writing this.
    pub path: String,

    /// Kubernetes service name used
    /// as <host> in the Varnish backend definition.
    ///
    /// The service name is sufixed as follows:
    ///
    /// <service-name>.<namespace>.svc.cluster.local
    ///
    /// just so Varnish can resolve its IP.
    pub service: String,

    pub path_type: String,

    /// Kubernetes service port used
    /// as <port> in the Varnish backend definition.
    pub port: u16,
}

#[derive(Serialize)]
pub struct Vcl<'a> {
    pub template: &'a str,
    pub file: &'a str,
    pub work_folder: &'a str,
    pub snippet: &'a str,
    pub content: String,
}

impl<'a> Vcl<'a> {
    pub fn new(file: &'a str, template: &'a str, work_folder: &'a str, snippet: &'a str) -> Self {
        Vcl {
            template,
            file,
            work_folder,
            snippet,
            content: String::new(),
        }
    }
}

impl Backend {
    pub fn new(
        namespace: String,
        name: String,
        host: String,
        path: String,
        service: String,
        path_type: String,
        port: u16,
    ) -> Self {
        Backend {
            namespace,
            name,
            host,
            path,
            service,
            port,
            path_type,
        }
    }
}

///
/// Update the specified vcl file with the provided
/// list of Backend objects.
///
pub fn update(vcl: &mut Vcl, backends: Vec<Backend>) -> Option<UpdateError> {
    let mut hb = Handlebars::new();

    if let Err(e) = hb.register_template_file(VCL, vcl.template) {
        return Some(UpdateError(e.to_string()));
    }

    let mut vcl_data = Map::new();

    vcl_data.insert(BACKEND.to_string(), to_json(backends));
    vcl_data.insert(SNIPPET.to_string(), to_json(vcl.snippet));

    let rendered_content = match hb.render(VCL, &vcl_data) {
        Ok(content) => content,
        Err(e) => {
            return Some(UpdateError(format!("Template render error: {}", e)));
        }
    };

    vcl.content = rendered_content;

    if let Err(e) = File::create(vcl.file).and_then(|mut f| f.write_all(vcl.content.as_bytes())) {
        return Some(UpdateError(format!(
            "Vcl [{}] file write error: {}",
            vcl.file, e
        )));
    }

    info!("Vcl file [{}] has been updated", vcl.file);
    None
}

///
/// Triggers Varnish to reload its vcl configuration.
///
/// E.g:
///
/// $ varnishreload -n /etc/varnish/work
///
/// See the Dockerfile and check what working folder
/// is being provided to Varnish
pub fn reload(vcl: &Vcl) -> Option<UpdateError> {
    let output = match Command::new(RELOAD_COMMAND)
        .arg("-n")
        .arg(vcl.work_folder)
        .output()
    {
        Ok(output) => output,
        Err(e) => {
            return Some(UpdateError(format!(
                "Vcl [{}] reload command error: {}",
                vcl.file, e
            )));
        }
    };

    if output.status.success() {
        info!("Vcl [{}] reloaded successfully", vcl.file);
        None
    } else {
        Some(UpdateError(format!(
            "Vcl [{}] reload error: {}",
            vcl.file,
            String::from_utf8_lossy(&output.stdout)
        )))
    }
}
