use log::info;
use std::io;
use std::process::{Command, Stdio};

pub struct Varnish<'a> {
    pub cmd: &'a str,
    pub port: &'a str,
    pub vcl: &'a str,
    pub work_dir: &'a str,
    pub params: &'a str,
    pub default_ttl: &'a str,
    pub storage: &'a str,
}

pub fn start(v: &Varnish) -> Result<u32, io::Error> {
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

    let _ = v.params.split_whitespace().for_each(|p| {
        args.push("-p");
        args.push(p);
    });

    if !v.storage.is_empty() {
        args.push("-s");
        args.push(v.storage);
    }

    info!("Starting Varnish with the following args: {:?}", args);

    Command::new(v.cmd)
        .args(args)
        .stdout(Stdio::piped())
        .spawn()
        .map(|c| c.id())
}
