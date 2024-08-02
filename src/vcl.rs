use handlebars::{to_json, Handlebars};
use log::info;
use serde::Serialize;
use serde_json::value::Map;
use std::{fs::File, io::{Read, Write}, process::Command};

const BACKEND: &str = "backend";
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

    /// Kubernetes service port used 
    /// as <port> in the Varnish backend definition.
    pub port: u16,
}

#[derive(Serialize)]
pub struct Vcl<'a> {
    pub template: &'a str,
    pub file: &'a str,
    pub work_folder: &'a str,
    pub content: String,
}

impl UpdateError {
    pub fn new(err_text: String) -> Self {
        UpdateError(err_text)
    }
}

impl<'a> Vcl<'a> {
    pub fn new(file: &'a str, template: &'a str, work_folder: &'a str) -> Self {
        Vcl {
            template,
            file,
            work_folder,
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
        port: u16,
    ) -> Self {
        Backend {
            namespace,
            name,
            host,
            path,
            service,
            port,
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

    match hb.render(VCL, &vcl_data) {
        Ok(c) => vcl.content = c,
        Err(e) => return Some(UpdateError::new(format!("render: {}", e.to_string()))),
    };

    match File::create(vcl.file) {
        Ok(mut f) => {
            let _ = f.write_all(vcl.content.as_bytes());
            info!("vcl file [{}] has been updated", vcl.file);
        }
        Err(e) => {
            return Some(UpdateError(format!(
                "vcl [{}] write error: {}",
                vcl.file,
                e.to_string()
            )))
        }
    };

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
    match Command::new(RELOAD_COMMAND)
        .arg("-n")
        .arg(vcl.work_folder)
        .output()
    {
        Ok(cs) => {
            if cs.status.success() {
                info!("vcl [{}] reloaded succesfully", vcl.file)
            } else {
                return Some(UpdateError(format!(
                    "vcl [{}] reload error: {:?}",
                    vcl.file, cs.stdout.bytes()
                )));
            }
        }
        Err(e) => {
            return Some(UpdateError(format!(
                "vcl [{}] reload error: {}",
                vcl.file,
                e.to_string()
            )))
        }
    }

    None
}
