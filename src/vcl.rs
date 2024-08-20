use handlebars::{to_json, Handlebars};
use log::error;
use log::info;
use serde::Serialize;
use serde_json::value::Map;
use std::{fs::File, io::Write, process::Command};

const RELOAD_COMMAND: &str = "varnishreload";

const TEMPLATE_KEY: &str = "vcl";
const BACKEND_KEY: &str = "backend";
const SNIPPET_KEY: &str = "snippet";

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
pub struct Vcl {
    pub template: String,
    pub file: String,
    pub work_folder: String,
    pub snippet: String,
    pub backends: Vec<Backend>,
}

impl Vcl {
    pub fn new(file: String, template: String, work_folder: String, snippet: String) -> Self {
        Vcl {
            template,
            file,
            work_folder,
            snippet,
            backends: vec![],
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
/// Update the specified VCL file with the provided
/// list of Backend objects and VCL snippet.
///
pub fn update(vcl: &Vcl) -> Result<(), UpdateError> {
    let mut handlebars = Handlebars::new();

    // Register the template file with Handlebars
    handlebars
        .register_template_file(TEMPLATE_KEY, &vcl.template)
        .map_err(|e| {
            error!("Failed to register template file: {}", e);
            UpdateError(e.to_string())
        })?;

    // Prepare data for template rendering
    let mut template_data = Map::new();
    template_data.insert(BACKEND_KEY.to_string(), to_json(&vcl.backends));
    template_data.insert(SNIPPET_KEY.to_string(), to_json(&vcl.snippet));

    // Render the template with the provided data
    let rendered_content = handlebars
        .render(TEMPLATE_KEY, &template_data)
        .map_err(|e| {
            error!("Template render error: {}", e);
            UpdateError(format!("Template render error: {}", e))
        })?;

    // Write the rendered content to the specified file
    File::create(&vcl.file)
        .and_then(|mut file| file.write_all(rendered_content.as_bytes()))
        .map_err(|e| {
            error!("Failed to write to VCL file [{}]: {}", vcl.file, e);
            UpdateError(format!("VCL file write error: {}", e))
        })?;

    info!("VCL file [{}] has been successfully updated", vcl.file);
    Ok(())
}

/// Triggers Varnish to reload its VCL configuration.
///
/// Example:
///
/// ```bash
/// $ varnishreload -n /etc/varnish/work
/// ```
///
/// Check the Dockerfile to see which working folder is being provided to Varnish.
pub fn reload(vcl: &Vcl) -> Result<(), UpdateError> {
    let output = Command::new(RELOAD_COMMAND)
        .arg("-n")
        .arg(&vcl.work_folder)
        .output()
        .map_err(|e| {
            UpdateError(format!(
                "Failed to execute reload command for VCL [{}]: {}",
                vcl.file, e
            ))
        })?;

    if output.status.success() {
        info!("VCL [{}] reloaded successfully.", vcl.file);
        Ok(())
    } else {
        let stderr_output = String::from_utf8_lossy(&output.stderr).to_string();
        Err(UpdateError(format!(
            "Failed to reload VCL [{}]: {}",
            vcl.file, stderr_output
        )))
    }
}
