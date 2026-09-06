//! Session Initiation Protocol (SIP - RFC 3261) & SDP (RFC 4566).
//!
//! Voice-over-IP (VoIP) and multimedia session signaling over UDP port 5060.

use std::collections::HashMap;
use std::fmt;

pub const SIP_PORT: u16 = 5060;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SipMethod {
    Invite,
    Ack,
    Bye,
    Cancel,
    Register,
    Options,
}

impl fmt::Display for SipMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SipMethod::Invite => write!(f, "INVITE"),
            SipMethod::Ack => write!(f, "ACK"),
            SipMethod::Bye => write!(f, "BYE"),
            SipMethod::Cancel => write!(f, "CANCEL"),
            SipMethod::Register => write!(f, "REGISTER"),
            SipMethod::Options => write!(f, "OPTIONS"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SipMessage {
    pub is_response: bool,
    pub status_code: u16,
    pub reason_phrase: String,
    pub method: Option<SipMethod>,
    pub request_uri: String,
    pub headers: HashMap<String, String>,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SipError {
    EmptyMessage,
    InvalidStartLine,
}

impl fmt::Display for SipError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SipError::EmptyMessage => write!(f, "Empty SIP message"),
            SipError::InvalidStartLine => write!(f, "Invalid SIP request/response start line"),
        }
    }
}

impl std::error::Error for SipError {}

impl SipMessage {
    pub fn build_invite(from: &str, to: &str, call_id: &str, sdp: &str) -> Self {
        let mut headers = HashMap::new();
        headers.insert(
            "Via".to_string(),
            "SIP/2.0/UDP 192.168.1.100:5060;branch=z9hG4bK776asdhds".to_string(),
        );
        headers.insert("From".to_string(), format!("<sip:{}>;tag=1928301774", from));
        headers.insert("To".to_string(), format!("<sip:{}>", to));
        headers.insert("Call-ID".to_string(), call_id.to_string());
        headers.insert("CSeq".to_string(), "1 INVITE".to_string());
        headers.insert(
            "Contact".to_string(),
            format!("<sip:{}@192.168.1.100:5060>", from),
        );
        headers.insert("Content-Type".to_string(), "application/sdp".to_string());
        headers.insert("Content-Length".to_string(), sdp.len().to_string());

        SipMessage {
            is_response: false,
            status_code: 0,
            reason_phrase: "".to_string(),
            method: Some(SipMethod::Invite),
            request_uri: format!("sip:{}", to),
            headers,
            body: sdp.to_string(),
        }
    }

    pub fn build_200_ok(invite_req: &SipMessage, local_sdp: &str) -> Self {
        let mut headers = HashMap::new();
        if let Some(via) = invite_req.headers.get("Via") {
            headers.insert("Via".to_string(), via.clone());
        }
        if let Some(from) = invite_req.headers.get("From") {
            headers.insert("From".to_string(), from.clone());
        }
        if let Some(to) = invite_req.headers.get("To") {
            headers.insert("To".to_string(), format!("{};tag=a6c85cf", to));
        }
        if let Some(cid) = invite_req.headers.get("Call-ID") {
            headers.insert("Call-ID".to_string(), cid.clone());
        }
        if let Some(cseq) = invite_req.headers.get("CSeq") {
            headers.insert("CSeq".to_string(), cseq.clone());
        }
        headers.insert("Content-Type".to_string(), "application/sdp".to_string());
        headers.insert("Content-Length".to_string(), local_sdp.len().to_string());

        SipMessage {
            is_response: true,
            status_code: 200,
            reason_phrase: "OK".to_string(),
            method: None,
            request_uri: "".to_string(),
            headers,
            body: local_sdp.to_string(),
        }
    }

    pub fn serialize(&self) -> String {
        let mut out = String::new();
        if self.is_response {
            out.push_str(&format!(
                "SIP/2.0 {} {}\r\n",
                self.status_code, self.reason_phrase
            ));
        } else {
            let m_str = self
                .method
                .as_ref()
                .map(|m| m.to_string())
                .unwrap_or_else(|| "INVITE".to_string());
            out.push_str(&format!("{} {} SIP/2.0\r\n", m_str, self.request_uri));
        }

        for (k, v) in &self.headers {
            out.push_str(&format!("{}: {}\r\n", k, v));
        }
        out.push_str("\r\n");
        out.push_str(&self.body);

        out
    }

    pub fn parse(text: &str) -> Result<Self, SipError> {
        let mut lines = text.lines();
        let start_line = lines.next().ok_or(SipError::EmptyMessage)?;

        let mut is_response = false;
        let mut status_code = 0;
        let mut reason_phrase = String::new();
        let mut method = None;
        let mut request_uri = String::new();

        if start_line.starts_with("SIP/2.0 ") {
            is_response = true;
            let parts: Vec<&str> = start_line.split_whitespace().collect();
            if parts.len() >= 3 {
                status_code = parts[1].parse::<u16>().unwrap_or(200);
                reason_phrase = parts[2..].join(" ");
            }
        } else {
            let parts: Vec<&str> = start_line.split_whitespace().collect();
            if parts.len() >= 2 {
                method = match parts[0] {
                    "INVITE" => Some(SipMethod::Invite),
                    "ACK" => Some(SipMethod::Ack),
                    "BYE" => Some(SipMethod::Bye),
                    "CANCEL" => Some(SipMethod::Cancel),
                    "REGISTER" => Some(SipMethod::Register),
                    _ => Some(SipMethod::Options),
                };
                request_uri = parts[1].to_string();
            }
        }

        let mut headers = HashMap::new();
        let mut in_body = false;
        let mut body_lines = Vec::new();

        for line in lines {
            if in_body {
                body_lines.push(line);
            } else if line.is_empty() || line == "\r" {
                in_body = true;
            } else if let Some(idx) = line.find(':') {
                let key = line[..idx].trim().to_string();
                let val = line[idx + 1..].trim().to_string();
                headers.insert(key, val);
            }
        }

        Ok(SipMessage {
            is_response,
            status_code,
            reason_phrase,
            method,
            request_uri,
            headers,
            body: body_lines.join("\r\n"),
        })
    }
}

/// Simple Session Description Protocol (SDP - RFC 4566) helper
pub fn build_simple_sdp(username: &str, ip: &str, rtp_port: u16) -> String {
    format!(
        "v=0\r\no={} 2890844526 2890844526 IN IP4 {}\r\ns=ToyStack Audio Call\r\nc=IN IP4 {}\r\nt=0 0\r\nm=audio {} RTP/AVP 0 101\r\na=rtpmap:0 PCMU/8000\r\n",
        username, ip, ip, rtp_port
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sip_invite_and_200_ok_flow() {
        let sdp = build_simple_sdp("alice", "192.168.1.100", 4000);
        let invite =
            SipMessage::build_invite("alice@example.com", "bob@example.com", "call-12345", &sdp);
        let raw = invite.serialize();

        let parsed = SipMessage::parse(&raw).unwrap();
        assert!(!parsed.is_response);
        assert_eq!(parsed.method, Some(SipMethod::Invite));
        assert_eq!(parsed.headers.get("Call-ID").unwrap(), "call-12345");
        assert!(parsed.body.contains("m=audio 4000 RTP/AVP 0"));

        let ok_sdp = build_simple_sdp("bob", "192.168.1.10", 5000);
        let ok_resp = SipMessage::build_200_ok(&parsed, &ok_sdp);
        assert_eq!(ok_resp.status_code, 200);
        assert_eq!(ok_resp.reason_phrase, "OK");
    }
}
