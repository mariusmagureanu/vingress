use log::debug;
use log::{error, info};
use std::process;
use std::process::Stdio;

use std::process::ExitStatus;
use tokio::process::Command;
use tokio::signal::unix::{SignalKind, signal};

pub struct Varnish<'a> {
    pub cmd: &'a str,
    pub port: &'a str,
    pub vcl: &'a str,
    pub work_dir: &'a str,
    pub params: &'a str,
    pub default_ttl: &'a str,
    pub storage: &'a str,
}

pub async fn start(v: &Varnish<'_>) {
    let varnish_addrr = format!("0.0.0.0:{}", v.port);

    let mut args: Vec<&str> = vec![
        "-a",
        &varnish_addrr,
        "-f",
        v.vcl,
        "-n",
        v.work_dir,
        "-t",
        v.default_ttl,
    ];

    v.params.split_whitespace().for_each(|p| {
        args.push("-p");
        args.push(p);
    });

    if !v.storage.is_empty() {
        args.push("-s");
        args.push(v.storage);
    }

    info!("Starting Varnish with the following args: {:?}", args);

    let mut child = Command::new(v.cmd)
        .args(args)
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to start Varnish");

    let child_handle = tokio::spawn(async move {
        match child.wait().await {
            Ok(status) => {
                handle_exit_status(Ok(status));
            }
            Err(e) => {
                handle_exit_status(Err(e));
            }
        }
    });

    let mut sigchld = signal(SignalKind::child()).expect("Failed to create SIGCHLD listener");

    tokio::select! {
        _ = sigchld.recv() => {
            debug!("Received SIGCHLD signal");
        }

        _ = child_handle => {
            info!("Varnish process finished");
        }
    }
}

fn handle_exit_status(exit_status: Result<ExitStatus, std::io::Error>) {
    match exit_status {
        Ok(status) => {
            if status.success() {
                info!("Varnish process completed successfully.");
            } else {
                error!("Varnish process crashed with status: {}", status);
                process::exit(1);
            }
        }
        Err(e) => {
            error!("Failed to wait on child process: {}", e);
            process::exit(1);
        }
    }
}
