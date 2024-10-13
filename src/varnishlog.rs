use log::{error, info};
use regex::Regex;
use serde::Serialize;
use std::fmt;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

pub async fn start(work_dir: &str) {
    let args: Vec<&str> = vec!["-n", work_dir, "-g", "request"];

    info!("Starting VarnishLog with the following args: {:?}", args);

    let mut child = Command::new("varnishlog")
        .args(args)
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to start VarnishLog");

    let re_patterns = get_regex_patterns();

    if let Some(stdout) = child.stdout.take() {
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();

        let mut state = RequestState::default();

        while let Some(line) = lines.next_line().await.unwrap() {
            parse_log_line(&line, &re_patterns, &mut state).await;
        }
    }
}

pub fn get_regex_patterns() -> RegexPatterns {
    RegexPatterns {
        re_req_method: Regex::new(r"^-   ReqMethod\s+(\w+)").unwrap(),
        re_req_url: Regex::new(r"^-   ReqURL\s+(.+)").unwrap(),
        re_req_protocol: Regex::new(r"^-   ReqProtocol\s+(.+)").unwrap(),
        re_req_header: Regex::new(r"^-   ReqHeader\s+(.+):\s+(.+)").unwrap(),
        re_resp_status: Regex::new(r"^-   RespStatus\s+(\d+)").unwrap(),
        re_resp_reason: Regex::new(r"^-   RespReason\s+(.+)").unwrap(),
        re_resp_header: Regex::new(r"^-   RespHeader\s+(.+):\s+(.+)").unwrap(),
        re_beresp_status: Regex::new(r"^--  BerespStatus\s+(\d+)").unwrap(),
        re_beresp_reason: Regex::new(r"^--  BerespReason\s+(.+)").unwrap(),
        re_beresp_header: Regex::new(r"^--  BerespHeader\s+(.+):\s+(.+)").unwrap(),
    }
}

pub struct RegexPatterns {
    re_req_method: Regex,
    re_req_url: Regex,
    re_req_protocol: Regex,
    re_req_header: Regex,
    re_resp_status: Regex,
    re_resp_reason: Regex,
    re_resp_header: Regex,
    re_beresp_status: Regex,
    re_beresp_reason: Regex,
    re_beresp_header: Regex,
}

// Struct to hold the state of the current request and backend response being parsed
#[derive(Default, Debug, PartialEq, Serialize)]
pub struct RequestState {
    pub method: String,
    pub url: String,
    pub protocol: String,
    pub req_headers: Vec<(String, String)>,
    pub resp_status: String,
    pub resp_reason: String,
    pub resp_headers: Vec<(String, String)>,
    pub beresp_status: String,
    pub beresp_reason: String,
    pub beresp_headers: Vec<(String, String)>,
}

pub async fn parse_log_line(line: &str, re_patterns: &RegexPatterns, state: &mut RequestState) {
    if line.trim().is_empty() {
        info!(
            "{} {} {} | {} {}",
            state.method, state.protocol, state.url, state.resp_status, state.resp_reason
        );
        match serde_json::to_string(state) {
            Ok(json) => info!("{}", json),
            Err(err) => error!("Failed to serialize request state: {}", err),
        }
        state.clear();
    }

    match () {
        _ if re_patterns.re_req_method.is_match(line) => {
            let caps = re_patterns.re_req_method.captures(line).unwrap();
            state.method = caps[1].to_string();
        }
        _ if re_patterns.re_req_url.is_match(line) => {
            let caps = re_patterns.re_req_url.captures(line).unwrap();
            state.url = caps[1].to_string();
        }
        _ if re_patterns.re_req_protocol.is_match(line) => {
            let caps = re_patterns.re_req_protocol.captures(line).unwrap();
            state.protocol = caps[1].to_string();
        }
        _ if re_patterns.re_req_header.is_match(line) => {
            let caps = re_patterns.re_req_header.captures(line).unwrap();
            state
                .req_headers
                .push((caps[1].to_string(), caps[2].to_string()));
        }
        _ if re_patterns.re_resp_status.is_match(line) => {
            let caps = re_patterns.re_resp_status.captures(line).unwrap();
            state.resp_status = caps[1].to_string();
        }
        _ if re_patterns.re_resp_reason.is_match(line) => {
            let caps = re_patterns.re_resp_reason.captures(line).unwrap();
            state.resp_reason = caps[1].to_string();
        }
        _ if re_patterns.re_resp_header.is_match(line) => {
            let caps = re_patterns.re_resp_header.captures(line).unwrap();
            state
                .resp_headers
                .push((caps[1].to_string(), caps[2].to_string()));
        }
        _ if re_patterns.re_beresp_status.is_match(line) => {
            let caps = re_patterns.re_beresp_status.captures(line).unwrap();
            state.beresp_status = caps[1].to_string();
        }
        _ if re_patterns.re_beresp_reason.is_match(line) => {
            let caps = re_patterns.re_beresp_reason.captures(line).unwrap();
            state.beresp_reason = caps[1].to_string();
        }
        _ if re_patterns.re_beresp_header.is_match(line) => {
            let caps = re_patterns.re_beresp_header.captures(line).unwrap();
            state
                .beresp_headers
                .push((caps[1].to_string(), caps[2].to_string()));
        }
        _ => {}
    }
}

impl RequestState {
    fn clear(&mut self) {
        self.method.clear();
        self.url.clear();
        self.protocol.clear();
        self.req_headers.clear();
        self.resp_status.clear();
        self.resp_reason.clear();
        self.resp_headers.clear();
        self.beresp_status.clear();
        self.beresp_reason.clear();
        self.beresp_headers.clear();
    }
}

impl fmt::Display for RequestState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "{} {} {} | {} {}",
            self.method, self.protocol, self.url, self.resp_status, self.resp_reason
        )?;

        for (key, value) in &self.req_headers {
            writeln!(f, ">> {}: {}", key, value)?;
        }

        for (key, value) in &self.resp_headers {
            writeln!(f, "  << {}: {}", key, value)?;
        }

        if !self.beresp_status.is_empty() && !self.beresp_reason.is_empty() {
            writeln!(f, "    <<< {} {}", self.beresp_status, self.beresp_reason)?;
            for (key, value) in &self.beresp_headers {
                writeln!(f, "    <<< {}: {}", key, value)?;
            }
        }

        Ok(())
    }
}
