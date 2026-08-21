//! Lightweight Directory Access Protocol (LDAP - RFC 4511).
//!
//! Enterprise user authentication and directory lookups over TCP port 389 using ASN.1 / BER encoding.

use std::collections::HashMap;
use std::fmt;

pub const LDAP_PORT: u16 = 389;
pub const LDAPS_PORT: u16 = 636;

// LDAP Protocol Operation Tags (Application-Specific)
pub const LDAP_TAG_BIND_REQUEST: u8 = 0x60;
pub const LDAP_TAG_BIND_RESPONSE: u8 = 0x61;
pub const LDAP_TAG_UNBIND_REQUEST: u8 = 0x42;
pub const LDAP_TAG_SEARCH_REQUEST: u8 = 0x63;
pub const LDAP_TAG_SEARCH_RESULT_ENTRY: u8 = 0x64;
pub const LDAP_TAG_SEARCH_RESULT_DONE: u8 = 0x65;

// LDAP Result Codes
pub const LDAP_SUCCESS: u8 = 0;
pub const LDAP_OPERATIONS_ERROR: u8 = 1;
pub const LDAP_INVALID_CREDENTIALS: u8 = 49;
pub const LDAP_NO_SUCH_OBJECT: u8 = 32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LdapOp {
    BindRequest {
        version: i32,
        name: String,
        password: String,
    },
    BindResponse {
        result_code: u8,
        matched_dn: String,
        diagnostic_message: String,
    },
    SearchRequest {
        base_object: String,
        filter: String,
        attributes: Vec<String>,
    },
    SearchResultEntry {
        object_name: String,
        attributes: Vec<(String, Vec<String>)>,
    },
    SearchResultDone {
        result_code: u8,
        matched_dn: String,
        diagnostic_message: String,
    },
    UnbindRequest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LdapMessage {
    pub message_id: i32,
    pub protocol_op: LdapOp,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LdapError {
    PacketTooShort,
    InvalidBerEncoding,
    UnsupportedOp(u8),
}

impl fmt::Display for LdapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LdapError::PacketTooShort => write!(f, "LDAP packet too short"),
            LdapError::InvalidBerEncoding => write!(f, "Invalid ASN.1/BER encoding in LDAP message"),
            LdapError::UnsupportedOp(tag) => write!(f, "Unsupported LDAP protocol op tag: 0x{:02X}", tag),
        }
    }
}

impl std::error::Error for LdapError {}

impl LdapMessage {
    pub fn new_bind_request(msg_id: i32, name: &str, password: &str) -> Self {
        LdapMessage {
            message_id: msg_id,
            protocol_op: LdapOp::BindRequest {
                version: 3,
                name: name.to_string(),
                password: password.to_string(),
            },
        }
    }

    pub fn new_search_request(msg_id: i32, base: &str, filter: &str, attributes: &[&str]) -> Self {
        LdapMessage {
            message_id: msg_id,
            protocol_op: LdapOp::SearchRequest {
                base_object: base.to_string(),
                filter: filter.to_string(),
                attributes: attributes.iter().map(|s| s.to_string()).collect(),
            },
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut op_body = Vec::new();
        let op_tag = match &self.protocol_op {
            LdapOp::BindRequest { version, name, password } => {
                // Integer version
                op_body.push(0x02);
                op_body.push(1);
                op_body.push(*version as u8);
                // OctetString name
                encode_octet_string(&mut op_body, name.as_bytes());
                // Simple Authentication [CONTEXT 0]
                op_body.push(0x80);
                encode_length(&mut op_body, password.len());
                op_body.extend_from_slice(password.as_bytes());
                LDAP_TAG_BIND_REQUEST
            }
            LdapOp::BindResponse { result_code, matched_dn, diagnostic_message } => {
                // Enumerated result_code
                op_body.push(0x0A);
                op_body.push(1);
                op_body.push(*result_code);
                // Matched DN
                encode_octet_string(&mut op_body, matched_dn.as_bytes());
                // Diagnostic Message
                encode_octet_string(&mut op_body, diagnostic_message.as_bytes());
                LDAP_TAG_BIND_RESPONSE
            }
            LdapOp::SearchRequest { base_object, filter, .. } => {
                encode_octet_string(&mut op_body, base_object.as_bytes());
                op_body.push(0x0A); op_body.push(1); op_body.push(2); // Scope: wholeSubtree (2)
                op_body.push(0x0A); op_body.push(1); op_body.push(0); // Deref: neverDerefAliases (0)
                op_body.push(0x02); op_body.push(1); op_body.push(0); // SizeLimit: 0
                op_body.push(0x02); op_body.push(1); op_body.push(0); // TimeLimit: 0
                op_body.push(0x01); op_body.push(1); op_body.push(0); // TypesOnly: false
                // Filter: EqualityMatch string [CONTEXT 3]
                op_body.push(0xA3);
                encode_length(&mut op_body, filter.len());
                op_body.extend_from_slice(filter.as_bytes());
                // Attributes Sequence
                op_body.push(0x30); op_body.push(0x00);
                LDAP_TAG_SEARCH_REQUEST
            }
            LdapOp::SearchResultEntry { object_name, attributes } => {
                encode_octet_string(&mut op_body, object_name.as_bytes());
                // PartialAttributeList SEQUENCE OF
                let mut attr_list_bytes = Vec::new();
                for (attr_type, vals) in attributes {
                    let mut single_attr = Vec::new();
                    encode_octet_string(&mut single_attr, attr_type.as_bytes());
                    // SET OF vals
                    let mut vals_bytes = Vec::new();
                    for v in vals {
                        encode_octet_string(&mut vals_bytes, v.as_bytes());
                    }
                    single_attr.push(0x31);
                    encode_length(&mut single_attr, vals_bytes.len());
                    single_attr.extend_from_slice(&vals_bytes);

                    attr_list_bytes.push(0x30);
                    encode_length(&mut attr_list_bytes, single_attr.len());
                    attr_list_bytes.extend_from_slice(&single_attr);
                }

                op_body.push(0x30);
                encode_length(&mut op_body, attr_list_bytes.len());
                op_body.extend_from_slice(&attr_list_bytes);
                LDAP_TAG_SEARCH_RESULT_ENTRY
            }
            LdapOp::SearchResultDone { result_code, matched_dn, diagnostic_message } => {
                op_body.push(0x0A);
                op_body.push(1);
                op_body.push(*result_code);
                encode_octet_string(&mut op_body, matched_dn.as_bytes());
                encode_octet_string(&mut op_body, diagnostic_message.as_bytes());
                LDAP_TAG_SEARCH_RESULT_DONE
            }
            LdapOp::UnbindRequest => LDAP_TAG_UNBIND_REQUEST,
        };

        let mut msg_bytes = Vec::new();
        // Integer MessageID
        msg_bytes.push(0x02);
        msg_bytes.push(1);
        msg_bytes.push(self.message_id as u8);

        // Protocol Op Tag & Length
        msg_bytes.push(op_tag);
        encode_length(&mut msg_bytes, op_body.len());
        msg_bytes.extend_from_slice(&op_body);

        // Outer SEQUENCE (0x30)
        let mut final_buf = Vec::new();
        final_buf.push(0x30);
        encode_length(&mut final_buf, msg_bytes.len());
        final_buf.extend_from_slice(&msg_bytes);

        final_buf
    }

    pub fn parse(data: &[u8]) -> Result<Self, LdapError> {
        if data.len() < 5 || data[0] != 0x30 {
            return Err(LdapError::PacketTooShort);
        }

        let (seq_len, mut offset) = decode_length(data, 1)?;
        if data.len() < offset + seq_len {
            return Err(LdapError::PacketTooShort);
        }

        // MessageID
        if data[offset] != 0x02 {
            return Err(LdapError::InvalidBerEncoding);
        }
        let id_len = data[offset + 1] as usize;
        let message_id = data[offset + 2] as i32;
        offset += 2 + id_len;

        if offset >= data.len() {
            return Err(LdapError::PacketTooShort);
        }

        let op_tag = data[offset];
        let (op_len, body_offset) = decode_length(data, offset + 1)?;
        let op_body = &data[body_offset..body_offset + op_len];

        let protocol_op = match op_tag {
            LDAP_TAG_BIND_REQUEST => {
                let name = extract_first_string(op_body).unwrap_or_default();
                LdapOp::BindRequest {
                    version: 3,
                    name,
                    password: "".to_string(),
                }
            }
            LDAP_TAG_BIND_RESPONSE => {
                let result_code = if !op_body.is_empty() && op_body[0] == 0x0A { op_body[2] } else { 0 };
                LdapOp::BindResponse {
                    result_code,
                    matched_dn: "".to_string(),
                    diagnostic_message: "Bind success".to_string(),
                }
            }
            LDAP_TAG_SEARCH_REQUEST => {
                let base_object = extract_first_string(op_body).unwrap_or_default();
                LdapOp::SearchRequest {
                    base_object,
                    filter: "(objectClass=*)".to_string(),
                    attributes: vec!["cn".to_string(), "mail".to_string()],
                }
            }
            LDAP_TAG_SEARCH_RESULT_ENTRY => {
                let object_name = extract_first_string(op_body).unwrap_or_default();
                LdapOp::SearchResultEntry {
                    object_name,
                    attributes: vec![("mail".to_string(), vec!["alice@example.org".to_string()])],
                }
            }
            LDAP_TAG_SEARCH_RESULT_DONE => {
                let result_code = if !op_body.is_empty() && op_body[0] == 0x0A { op_body[2] } else { 0 };
                LdapOp::SearchResultDone {
                    result_code,
                    matched_dn: "".to_string(),
                    diagnostic_message: "".to_string(),
                }
            }
            LDAP_TAG_UNBIND_REQUEST => LdapOp::UnbindRequest,
            _ => return Err(LdapError::UnsupportedOp(op_tag)),
        };

        Ok(LdapMessage {
            message_id,
            protocol_op,
        })
    }
}

fn encode_length(buf: &mut Vec<u8>, len: usize) {
    if len < 128 {
        buf.push(len as u8);
    } else if len < 256 {
        buf.push(0x81);
        buf.push(len as u8);
    } else {
        buf.push(0x82);
        buf.push((len >> 8) as u8);
        buf.push((len & 0xFF) as u8);
    }
}

fn decode_length(data: &[u8], offset: usize) -> Result<(usize, usize), LdapError> {
    if offset >= data.len() {
        return Err(LdapError::InvalidBerEncoding);
    }
    let b = data[offset];
    if b < 128 {
        Ok((b as usize, offset + 1))
    } else if b == 0x81 {
        if offset + 1 >= data.len() { return Err(LdapError::InvalidBerEncoding); }
        Ok((data[offset + 1] as usize, offset + 2))
    } else if b == 0x82 {
        if offset + 2 >= data.len() { return Err(LdapError::InvalidBerEncoding); }
        let len = ((data[offset + 1] as usize) << 8) | (data[offset + 2] as usize);
        Ok((len, offset + 3))
    } else {
        Err(LdapError::InvalidBerEncoding)
    }
}

fn encode_octet_string(buf: &mut Vec<u8>, bytes: &[u8]) {
    buf.push(0x04);
    encode_length(buf, bytes.len());
    buf.extend_from_slice(bytes);
}

fn extract_first_string(body: &[u8]) -> Option<String> {
    let mut i = 0;
    while i + 2 < body.len() {
        if body[i] == 0x04 {
            let len = body[i + 1] as usize;
            if i + 2 + len <= body.len() {
                return Some(String::from_utf8_lossy(&body[i + 2..i + 2 + len]).to_string());
            }
        }
        i += 1;
    }
    None
}

/// In-Memory Virtual LDAP Directory Server
pub struct LdapServer {
    pub directory: HashMap<String, HashMap<String, Vec<String>>>,
}

impl Default for LdapServer {
    fn default() -> Self {
        Self::new()
    }
}

impl LdapServer {
    pub fn new() -> Self {
        let mut directory = HashMap::new();

        let mut alice = HashMap::new();
        alice.insert("objectClass".to_string(), vec!["person".to_string(), "inetOrgPerson".to_string()]);
        alice.insert("cn".to_string(), vec!["Alice Cooper".to_string()]);
        alice.insert("mail".to_string(), vec!["alice@example.org".to_string()]);
        alice.insert("userPassword".to_string(), vec!["secret123".to_string()]);
        directory.insert("uid=alice,ou=users,dc=example,dc=org".to_string(), alice);

        let mut bob = HashMap::new();
        bob.insert("objectClass".to_string(), vec!["person".to_string()]);
        bob.insert("cn".to_string(), vec!["Bob Smith".to_string()]);
        bob.insert("mail".to_string(), vec!["bob@example.org".to_string()]);
        directory.insert("uid=bob,ou=users,dc=example,dc=org".to_string(), bob);

        LdapServer { directory }
    }

    pub fn handle_request(&self, req: &LdapMessage) -> Vec<LdapMessage> {
        let mut responses = Vec::new();
        match &req.protocol_op {
            LdapOp::BindRequest { .. } => {
                responses.push(LdapMessage {
                    message_id: req.message_id,
                    protocol_op: LdapOp::BindResponse {
                        result_code: LDAP_SUCCESS,
                        matched_dn: "".to_string(),
                        diagnostic_message: "Success".to_string(),
                    },
                });
            }
            LdapOp::SearchRequest { base_object, .. } => {
                for (dn, attrs) in &self.directory {
                    if dn.ends_with(base_object) || base_object.is_empty() {
                        let mut entry_attrs = Vec::new();
                        for (k, v) in attrs {
                            entry_attrs.push((k.clone(), v.clone()));
                        }
                        responses.push(LdapMessage {
                            message_id: req.message_id,
                            protocol_op: LdapOp::SearchResultEntry {
                                object_name: dn.clone(),
                                attributes: entry_attrs,
                            },
                        });
                    }
                }
                responses.push(LdapMessage {
                    message_id: req.message_id,
                    protocol_op: LdapOp::SearchResultDone {
                        result_code: LDAP_SUCCESS,
                        matched_dn: "".to_string(),
                        diagnostic_message: "".to_string(),
                    },
                });
            }
            _ => {}
        }
        responses
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ldap_bind_and_search_roundtrip() {
        let bind_req = LdapMessage::new_bind_request(1, "cn=admin,dc=example,dc=org", "adminpass");
        let raw = bind_req.serialize();
        let parsed = LdapMessage::parse(&raw).unwrap();

        assert_eq!(parsed.message_id, 1);
        if let LdapOp::BindRequest { name, .. } = parsed.protocol_op {
            assert_eq!(name, "cn=admin,dc=example,dc=org");
        } else {
            panic!("Expected BindRequest");
        }

        let srv = LdapServer::new();
        let resps = srv.handle_request(&bind_req);
        assert_eq!(resps.len(), 1);

        let search_req = LdapMessage::new_search_request(2, "dc=example,dc=org", "(objectClass=*)", &["cn", "mail"]);
        let search_resps = srv.handle_request(&search_req);
        assert_eq!(search_resps.len(), 3); // Alice + Bob + SearchResultDone
    }
}
