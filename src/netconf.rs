//! Network Configuration Protocol (NETCONF - RFC 6241).
//!
//! Programmable XML-RPC network device management, datastores, and configuration sessions over TCP port 830.

pub const NETCONF_PORT: u16 = 830;
pub const NETCONF_EOM_1_0: &str = "]]>]]>";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetconfRpc {
    Hello { session_id: u32, capabilities: Vec<String> },
    GetConfig { message_id: String, source: String },
    EditConfig { message_id: String, target: String, config_xml: String },
    Commit { message_id: String },
    Lock { message_id: String, target: String },
    Unlock { message_id: String, target: String },
    CloseSession { message_id: String },
    Unknown { message_id: String, raw_xml: String },
}

impl NetconfRpc {
    pub fn build_hello_reply(session_id: u32) -> String {
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
             <hello xmlns=\"urn:ietf:params:xml:ns:netconf:base:1.0\">\n  \
               <capabilities>\n    \
                 <capability>urn:ietf:params:netconf:base:1.0</capability>\n    \
                 <capability>urn:ietf:params:netconf:base:1.1</capability>\n    \
                 <capability>urn:ietf:params:netconf:capability:candidate:1.0</capability>\n  \
               </capabilities>\n  \
               <session-id>{}</session-id>\n\
             </hello>{}",
            session_id, NETCONF_EOM_1_0
        )
    }

    pub fn build_rpc_reply(message_id: &str, inner_xml: &str) -> String {
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
             <rpc-reply message-id=\"{}\" xmlns=\"urn:ietf:params:xml:ns:netconf:base:1.0\">\n  \
               {}\n\
             </rpc-reply>{}",
            message_id, inner_xml, NETCONF_EOM_1_0
        )
    }

    pub fn build_ok(message_id: &str) -> String {
        Self::build_rpc_reply(message_id, "<ok/>")
    }

    pub fn parse_rpc(xml: &str) -> Self {
        let msg_id = if let Some(idx) = xml.find("message-id=\"") {
            let rest = &xml[idx + 12..];
            if let Some(end) = rest.find('"') {
                rest[..end].to_string()
            } else {
                "1".to_string()
            }
        } else {
            "1".to_string()
        };

        if xml.contains("<hello") {
            NetconfRpc::Hello {
                session_id: 1,
                capabilities: vec![
                    "urn:ietf:params:netconf:base:1.0".to_string(),
                    "urn:ietf:params:netconf:base:1.1".to_string(),
                ],
            }
        } else if xml.contains("<get-config>") {
            let source = if xml.contains("<candidate/>") { "candidate" } else { "running" };
            NetconfRpc::GetConfig {
                message_id: msg_id,
                source: source.to_string(),
            }
        } else if xml.contains("<edit-config>") {
            let target = if xml.contains("<candidate/>") { "candidate" } else { "running" };
            NetconfRpc::EditConfig {
                message_id: msg_id,
                target: target.to_string(),
                config_xml: xml.to_string(),
            }
        } else if xml.contains("<commit/>") {
            NetconfRpc::Commit { message_id: msg_id }
        } else if xml.contains("<lock>") {
            NetconfRpc::Lock { message_id: msg_id, target: "running".to_string() }
        } else if xml.contains("<unlock>") {
            NetconfRpc::Unlock { message_id: msg_id, target: "running".to_string() }
        } else if xml.contains("<close-session/>") {
            NetconfRpc::CloseSession { message_id: msg_id }
        } else {
            NetconfRpc::Unknown {
                message_id: msg_id,
                raw_xml: xml.to_string(),
            }
        }
    }
}

/// In-memory NETCONF Configuration Server
#[derive(Debug, Clone)]
pub struct NetconfServer {
    pub session_counter: u32,
    pub running_config: String,
    pub candidate_config: String,
    pub is_locked: bool,
}

impl Default for NetconfServer {
    fn default() -> Self {
        Self::new()
    }
}

impl NetconfServer {
    pub fn new() -> Self {
        let default_config = "<interfaces>\n  <interface>\n    <name>eth0</name>\n    <ipv4>192.168.1.100/24</ipv4>\n    <enabled>true</enabled>\n  </interface>\n</interfaces>".to_string();
        NetconfServer {
            session_counter: 101,
            running_config: default_config.clone(),
            candidate_config: default_config,
            is_locked: false,
        }
    }

    pub fn handle_request(&mut self, xml_input: &str) -> String {
        let clean_xml = xml_input.trim_end_matches(NETCONF_EOM_1_0).trim();
        let rpc = NetconfRpc::parse_rpc(clean_xml);

        match rpc {
            NetconfRpc::Hello { .. } => {
                let sid = self.session_counter;
                self.session_counter += 1;
                NetconfRpc::build_hello_reply(sid)
            }
            NetconfRpc::GetConfig { message_id, source } => {
                let cfg = if source == "candidate" {
                    &self.candidate_config
                } else {
                    &self.running_config
                };
                let data_xml = format!("<data>\n  {}\n</data>", cfg);
                NetconfRpc::build_rpc_reply(&message_id, &data_xml)
            }
            NetconfRpc::EditConfig { message_id, config_xml, .. } => {
                self.candidate_config = config_xml;
                NetconfRpc::build_ok(&message_id)
            }
            NetconfRpc::Commit { message_id } => {
                self.running_config = self.candidate_config.clone();
                NetconfRpc::build_ok(&message_id)
            }
            NetconfRpc::Lock { message_id, .. } => {
                self.is_locked = true;
                NetconfRpc::build_ok(&message_id)
            }
            NetconfRpc::Unlock { message_id, .. } => {
                self.is_locked = false;
                NetconfRpc::build_ok(&message_id)
            }
            NetconfRpc::CloseSession { message_id } => NetconfRpc::build_ok(&message_id),
            NetconfRpc::Unknown { message_id, .. } => NetconfRpc::build_ok(&message_id),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_netconf_hello_get_config_and_commit() {
        let mut server = NetconfServer::new();

        // 1. Client Hello
        let client_hello = "<hello xmlns=\"urn:ietf:params:xml:ns:netconf:base:1.0\"><capabilities><capability>urn:ietf:params:netconf:base:1.0</capability></capabilities></hello>]]>]]>";
        let resp_hello = server.handle_request(client_hello);
        assert_eq!(resp_hello.contains("<session-id>101</session-id>"), true);
        assert_eq!(resp_hello.ends_with(NETCONF_EOM_1_0), true);

        // 2. Get Config
        let get_cfg = "<rpc message-id=\"102\" xmlns=\"urn:ietf:params:xml:ns:netconf:base:1.0\"><get-config><source><running/></source></get-config></rpc>]]>]]>";
        let resp_get = server.handle_request(get_cfg);
        assert_eq!(resp_get.contains("<rpc-reply message-id=\"102\""), true);
        assert_eq!(resp_get.contains("<name>eth0</name>"), true);

        // 3. Commit
        let commit = "<rpc message-id=\"103\" xmlns=\"urn:ietf:params:xml:ns:netconf:base:1.0\"><commit/></rpc>]]>]]>";
        let resp_commit = server.handle_request(commit);
        assert_eq!(resp_commit.contains("<ok/>"), true);

        assert_eq!(NETCONF_PORT, 830);
    }
}
