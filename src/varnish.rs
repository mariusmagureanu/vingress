use log::debug;
use log::{error, info};
use std::process::Stdio;
use thiserror::Error;
use tokio::process::Command;

#[derive(Debug, Error)]
pub enum VarnishError {
    #[error("Failed to start Varnish: {0}")]
    SpawnFailed(#[from] std::io::Error),

    #[error("Varnish process crashed with status: {0}")]
    ProcessCrashed(std::process::ExitStatus),

    #[error("Failed to wait on Varnish process: {0}")]
    WaitFailed(std::io::Error),
}

pub struct Varnish {
    pub cmd: String,
    pub port: String,
    pub vcl: String,
    pub work_dir: String,
    pub params: String,
    pub default_ttl: String,
    pub storage: String,
}

pub async fn start(v: &Varnish) -> Result<(), VarnishError> {
    let varnish_addr = format!("0.0.0.0:{}", v.port);

    let mut args: Vec<String> = vec![
        "-a".to_string(),
        varnish_addr,
        "-f".to_string(),
        v.vcl.clone(),
        "-n".to_string(),
        v.work_dir.clone(),
        "-t".to_string(),
        v.default_ttl.clone(),
    ];

    for p in v.params.split_whitespace() {
        args.push("-p".to_string());
        args.push(p.to_string());
    }

    if !v.storage.is_empty() {
        args.push("-s".to_string());
        args.push(v.storage.clone());
    }

    info!("Starting Varnish with the following args: {args:?}");

    let mut child = Command::new(&v.cmd)
        .args(&args)
        .stdout(Stdio::piped())
        .spawn()?;

    let status = child.wait().await.map_err(VarnishError::WaitFailed)?;

    if status.success() {
        debug!("Varnish process completed successfully.");
        Ok(())
    } else {
        error!("Varnish process crashed with status: {status}");
        Err(VarnishError::ProcessCrashed(status))
    }
}
