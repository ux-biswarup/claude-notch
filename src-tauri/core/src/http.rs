//! Minimal HTTP/1.1 request parsing for the loopback hook listener.
//!
//! The listener only ever talks to `curl` on 127.0.0.1, so a full HTTP stack
//! would be dead weight. This handles exactly what we need: a request line,
//! headers, and a `Content-Length` body.

pub const MAX_REQUEST_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRequest {
    pub method: String,
    pub path: String,
    pub body: Vec<u8>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ParseStatus {
    /// Need more bytes.
    Incomplete,
    /// Not something we are willing to serve.
    Invalid,
    Complete(HttpRequest),
}

/// Try to parse a complete request from the bytes received so far.
pub fn parse_request(buf: &[u8]) -> ParseStatus {
    let Some(head_end) = find(buf, b"\r\n\r\n") else {
        return if buf.len() > MAX_REQUEST_BYTES {
            ParseStatus::Invalid
        } else {
            ParseStatus::Incomplete
        };
    };

    let Ok(head) = std::str::from_utf8(&buf[..head_end]) else {
        return ParseStatus::Invalid;
    };
    let mut lines = head.split("\r\n");
    let request_line = lines.next().unwrap_or_default();
    let mut parts = request_line.split_whitespace();
    let (Some(method), Some(path)) = (parts.next(), parts.next()) else {
        return ParseStatus::Invalid;
    };

    let mut content_length = 0usize;
    for line in lines {
        if let Some((name, value)) = line.split_once(':') {
            if name.trim().eq_ignore_ascii_case("content-length") {
                match value.trim().parse::<usize>() {
                    Ok(n) => content_length = n,
                    Err(_) => return ParseStatus::Invalid,
                }
            }
        }
    }
    if content_length > MAX_REQUEST_BYTES {
        return ParseStatus::Invalid;
    }

    let body_start = head_end + 4;
    if buf.len() < body_start + content_length {
        return ParseStatus::Incomplete;
    }

    ParseStatus::Complete(HttpRequest {
        method: method.to_string(),
        path: path.to_string(),
        body: buf[body_start..body_start + content_length].to_vec(),
    })
}

/// Build a plain-text response. `204` responses carry no body headers.
pub fn response(status: u16, reason: &str, body: &str) -> Vec<u8> {
    if status == 204 {
        return format!("HTTP/1.1 204 {reason}\r\nConnection: close\r\n\r\n").into_bytes();
    }
    format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .into_bytes()
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_curl_style_post() {
        let raw = b"POST /hook HTTP/1.1\r\nHost: 127.0.0.1:47831\r\nUser-Agent: curl/8.21.0\r\nAccept: */*\r\nContent-Type: application/json\r\nContent-Length: 13\r\n\r\n{\"a\":\"hello\"}";
        match parse_request(raw) {
            ParseStatus::Complete(req) => {
                assert_eq!(req.method, "POST");
                assert_eq!(req.path, "/hook");
                assert_eq!(req.body, b"{\"a\":\"hello\"}");
            }
            other => panic!("expected complete request, got {other:?}"),
        }
    }

    #[test]
    fn waits_for_headers_and_body() {
        assert_eq!(
            parse_request(b"POST /hook HTTP/1.1\r\nContent-Len"),
            ParseStatus::Incomplete
        );
        assert_eq!(
            parse_request(b"POST /hook HTTP/1.1\r\nContent-Length: 5\r\n\r\nab"),
            ParseStatus::Incomplete
        );
    }

    #[test]
    fn get_without_body_is_complete() {
        match parse_request(b"GET /health HTTP/1.1\r\nHost: x\r\n\r\n") {
            ParseStatus::Complete(req) => {
                assert_eq!(req.method, "GET");
                assert_eq!(req.path, "/health");
                assert!(req.body.is_empty());
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn rejects_garbage() {
        assert_eq!(parse_request(b"\r\n\r\n"), ParseStatus::Invalid);
        assert_eq!(
            parse_request(b"POST /hook HTTP/1.1\r\nContent-Length: nope\r\n\r\n"),
            ParseStatus::Invalid
        );
        assert_eq!(
            parse_request(b"POST /hook HTTP/1.1\r\nContent-Length: 99999999\r\n\r\n"),
            ParseStatus::Invalid
        );
    }

    #[test]
    fn builds_responses() {
        let ok = String::from_utf8(response(200, "OK", "ok")).unwrap();
        assert!(ok.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(ok.contains("Content-Length: 2\r\n"));
        assert!(ok.ends_with("\r\n\r\nok"));

        let empty = String::from_utf8(response(204, "No Content", "")).unwrap();
        assert!(!empty.contains("Content-Length"));
        assert!(empty.ends_with("\r\n\r\n"));
    }
}
