//! SQL Server Resolution Protocol (UDP 1434) — resolve a named instance to its TCP port.

use std::net::UdpSocket;
use std::time::Duration;

const BROWSER_PORT: u16 = 1434;
const CLNT_UCAST_INST: u8 = 0x04;

/// Build a `CLNT_UCAST_INST` request for `instance` (UTF-16LE, null-terminated).
fn ucast_instance_request(instance: &str) -> Vec<u8> {
    let mut req = vec![CLNT_UCAST_INST];
    for unit in instance.encode_utf16() {
        req.extend_from_slice(&unit.to_le_bytes());
    }
    req.extend_from_slice(&[0x00, 0x00]);
    req
}

/// Parse a SQL Browser `SVR_RESP` payload and return the `tcp` port, if present.
pub(crate) fn parse_browser_tcp_port(payload: &[u8]) -> Option<u16> {
    if payload.is_empty() {
        return None;
    }
    let body = if payload.first() == Some(&0x05) && payload.len() >= 3 {
        let header_len = usize::from(u16::from_le_bytes([payload[1], payload[2]]));
        if header_len >= 3 && header_len <= payload.len() {
            &payload[3..header_len]
        } else {
            &payload[3..]
        }
    } else {
        payload
    };
    let text = browser_response_text(body);
    let mut parts = text.split(';').filter(|p| !p.is_empty());
    while let Some(tok) = parts.next() {
        if tok.eq_ignore_ascii_case("tcp") {
            return parts.next()?.parse().ok();
        }
    }
    None
}

fn browser_response_text(body: &[u8]) -> String {
    if let Ok(s) = std::str::from_utf8(body) {
        if s.contains(';') {
            return s.to_string();
        }
    }
    if body.len() >= 2 {
        let u16_len = body.len() / 2;
        let mut u16s = Vec::with_capacity(u16_len);
        for chunk in body.chunks_exact(2) {
            u16s.push(u16::from_le_bytes([chunk[0], chunk[1]]));
        }
        let as_utf16 = String::from_utf16_lossy(&u16s);
        if as_utf16.contains(';') {
            return as_utf16;
        }
    }
    String::from_utf8_lossy(body).into_owned()
}

/// Ask the SQL Browser on `host` which TCP port serves `instance`.
pub(crate) fn resolve_tcp_port(host: &str, instance: &str) -> Result<u16, String> {
    let instance = instance.trim();
    if instance.is_empty() {
        return Err("instance query parameter must not be empty".into());
    }
    let socket =
        UdpSocket::bind("0.0.0.0:0").map_err(|e| format!("UDP bind for SQL Browser: {e}"))?;
    socket
        .set_read_timeout(Some(Duration::from_secs(3)))
        .map_err(|e| format!("UDP timeout for SQL Browser: {e}"))?;
    let req = ucast_instance_request(instance);
    let sent = socket
        .send_to(&req, (host.as_str(), BROWSER_PORT))
        .map_err(|e| format!("SQL Browser UDP send to {host}:{BROWSER_PORT}: {e}"))?;
    let _ = sent;
    let mut buf = [0u8; 4096];
    let len = socket
        .recv(&mut buf)
        .map_err(|e| format!("SQL Browser UDP recv from {host}:{BROWSER_PORT}: {e}"))?;
    parse_browser_tcp_port(&buf[..len]).ok_or_else(|| {
        format!(
            "SQL Browser did not return a TCP port for instance {instance:?} on {host:?} \
             (UDP {BROWSER_PORT}). Use an explicit port in the URL (without ?instance=), e.g. \
             mssql://user:pass@{host}:PORT/dbname"
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tcp_token_from_utf8_response() {
        let payload = b"\x05\x00\x00;ServerName;HOST;InstanceName;SQLEXPRESS;tcp;49152;np;pipe;";
        assert_eq!(parse_browser_tcp_port(payload), Some(49152));
    }

    #[test]
    fn parses_without_leading_header() {
        let payload = b"ServerName;X;InstanceName;MSSQLSERVER;tcp;1433;";
        assert_eq!(parse_browser_tcp_port(payload), Some(1433));
    }

    #[test]
    fn parses_utf16le_response_body() {
        let mut body: Vec<u8> = Vec::new();
        for ch in "ServerName;X;InstanceName;SQLEXPRESS;tcp;49152;".encode_utf16() {
            body.extend_from_slice(&ch.to_le_bytes());
        }
        let mut payload = vec![0x05u8, 0x00, 0x00];
        payload.extend(body);
        assert_eq!(parse_browser_tcp_port(&payload), Some(49152));
    }

    #[test]
    fn request_starts_with_ucast_opcode() {
        let req = ucast_instance_request("SQLEXPRESS");
        assert_eq!(req[0], CLNT_UCAST_INST);
        assert!(req.ends_with(&[0, 0]));
    }
}