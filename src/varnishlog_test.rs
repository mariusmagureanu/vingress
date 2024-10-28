#[cfg(test)]
mod test {
    use crate::varnishlog::{get_regex_patterns, parse_log_line, RequestState};

    #[tokio::test]
    async fn test_parse_request() {
        let input_lines = r#"*   << Request  >> 24168
            -   Begin          req 24167 rxreq
            -   Timestamp      Start: 1728643157.173031 0.000000 0.000000
            -   Timestamp      Req: 1728643157.173031 0.000000 0.000000
            -   VCL_use        reload_20241006_164229_237
            -   ReqStart       10.125.174.2 61486 a0
            -   ReqMethod      GET
            -   ReqURL         /foo
            -   ReqProtocol    HTTP/1.1
            -   ReqHeader      Host: foo.bar.com
            -   ReqHeader      User-Agent: curl/8.10.1
            -   ReqHeader      Accept: */*
            -   ReqHeader      X-Forwarded-For: 10.125.174.2
            -   RespProtocol   HTTP/1.1
            -   RespStatus     200
            -   RespReason     OK
            -   RespHeader     Server: nginx/1.27.1
            -   RespHeader     Date: Fri, 11 Oct 2024 10:39:17 GMT
            -   RespHeader     Content-Type: text/plain
            -   RespHeader     Content-Length: 162
            -   RespHeader     Connection: keep-alive
            -   ReqAcct        78 0 78 327 162 489
            -   End            "#;

        let re_patterns = get_regex_patterns();
        let mut state = RequestState::default();

        for line in input_lines.lines() {
            let line = line.trim();
            parse_log_line(&line, &re_patterns, &mut state).await;
        }

        let expected_request = RequestState {
            method: "GET".to_string(),
            url: "/foo".to_string(),
            protocol: "HTTP/1.1".to_string(),
            req_headers: vec![
                ("Host".to_string(), "foo.bar.com".to_string()),
                ("User-Agent".to_string(), "curl/8.10.1".to_string()),
                ("Accept".to_string(), "*/*".to_string()),
                ("X-Forwarded-For".to_string(), "10.125.174.2".to_string()),
            ],
            resp_reason: "OK".to_string(),
            resp_status: "200".to_string(),
            resp_headers: vec![
                ("Server".to_string(), "nginx/1.27.1".to_string()),
                (
                    "Date".to_string(),
                    "Fri, 11 Oct 2024 10:39:17 GMT".to_string(),
                ),
                ("Content-Type".to_string(), "text/plain".to_string()),
                ("Content-Length".to_string(), "162".to_string()),
                ("Connection".to_string(), "keep-alive".to_string()),
            ],
            beresp_headers: vec![],
            beresp_status: String::from(""),
            beresp_reason: String::from(""),
        };

        assert_eq!(state.method, expected_request.method);
        assert_eq!(state.url, expected_request.url);
        assert_eq!(state.protocol, expected_request.protocol);
        assert_eq!(state.req_headers, expected_request.req_headers);
        assert_eq!(state.resp_reason, expected_request.resp_reason);
        assert_eq!(state.resp_status, expected_request.resp_status);
        assert_eq!(state.resp_headers, expected_request.resp_headers);
    }

    #[tokio::test]
    async fn test_parse_backend_request() {
        let input_lines = r#"**  << BeReq    >> 24169
            --  Begin          bereq 24168 fetch
            --  VCL_use        reload_20241006_164229_237
            --  Timestamp      Start: 1728643157.173163 0.000000 0.000000
            --  BereqMethod    GET
            --  BereqURL       /foo
            --  BereqProtocol  HTTP/1.1
            --  BereqHeader    Host: foo.bar.com
            --  BereqHeader    User-Agent: curl/8.10.1
            --  BereqHeader    Accept: */*
            --  BereqHeader    X-Forwarded-For: 10.125.174.2
            --  BereqHeader    Via: 1.1 varnish-ingress-controller-d465bf5c5-7pd9l (Varnish/7.6)
            --  BereqHeader    Accept-Encoding: gzip
            --  BereqHeader    X-Varnish: 24169
            --  VCL_call       BACKEND_FETCH
            --  VCL_return     fetch
            --  Timestamp      Fetch: 1728643157.173180 0.000017 0.000017
            --  Timestamp      Connected: 1728643157.173341 0.000178 0.000160
            --  BackendOpen    22 demo-media-media-v1-svc 172.20.163.200 80 10.125.161.62 57622 connect
            --  Timestamp      Bereq: 1728643157.173377 0.000214 0.000036
            --  BerespProtocol HTTP/1.1
            --  BerespStatus   200
            --  BerespReason   OK
            --  BerespHeader   Server: nginx/1.27.1
            --  BerespHeader   Date: Fri, 11 Oct 2024 10:39:17 GMT
            --  BerespHeader   Content-Type: text/plain
            --  BerespHeader   Content-Length: 162
            --  BerespHeader   Connection: keep-alive
            --  BerespHeader   Expires: Fri, 11 Oct 2024 10:39:16 GMT
            --  BerespHeader   Cache-Control: no-cache
            --  Timestamp      Beresp: 1728643157.173478 0.000315 0.000101
            --  TTL            RFC 0 10 0 1728643157 1728643157 1728643157 1728643156 0 cacheable
            --  VCL_call       BACKEND_RESPONSE
            --  BerespUnset    Cache-Control: no-cache
            --  VCL_return     deliver
            --  Timestamp      Process: 1728643157.173570 0.000406 0.000091
            --  Storage        malloc s0
            --  Fetch_Body     3 length stream
            --  BackendClose   22 demo-media-media-v1-svc recycle
            --  Timestamp      BerespBody: 1728643157.173638 0.000475 0.000068
            --  Length         162
            --  BereqAcct      217 0 217 214 162 376
            --  End            "#;

        let re_patterns = get_regex_patterns();
        let mut state = RequestState::default();

        for line in input_lines.lines() {
            let line = line.trim();
            parse_log_line(&line, &re_patterns, &mut state).await;
        }

        let expected_request = RequestState {
            method: "".to_string(),
            protocol: "".to_string(),
            resp_status: "".to_string(),
            resp_reason: "".to_string(),
            resp_headers: vec![],
            req_headers: vec![],
            url: "/foo".to_string(),
            beresp_status: "200".to_string(),
            beresp_reason: "OK".to_string(),
            beresp_headers: vec![
                ("Server".to_string(), "nginx/1.27.1".to_string()),
                (
                    "Date".to_string(),
                    "Fri, 11 Oct 2024 10:39:17 GMT".to_string(),
                ),
                ("Content-Type".to_string(), "text/plain".to_string()),
                ("Content-Length".to_string(), "162".to_string()),
                ("Connection".to_string(), "keep-alive".to_string()),
                (
                    "Expires".to_string(),
                    "Fri, 11 Oct 2024 10:39:16 GMT".to_string(),
                ),
                ("Cache-Control".to_string(), "no-cache".to_string()),
            ],
        };

        assert_eq!(state.beresp_reason, expected_request.beresp_reason);
        assert_eq!(state.beresp_status, expected_request.beresp_status);
        assert_eq!(state.beresp_headers, expected_request.beresp_headers);
    }
}
