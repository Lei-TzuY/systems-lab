//! O-RAN WG4 Open Fronthaul M-Plane Security Management & Certificate / TLS Lifecycle Engine
//!
//! Conforms to:
//! - O-RAN.WG4.MP.0 Section 6: Security
//! - `o-ran-usermgmt.yang`: User Management and RBAC
//! - `o-ran-certificates.yang`: X.509 Certificate and Trust Anchor Management
//! - RFC 8341: Network Configuration Access Control Model (NACM)
//! - RFC 4210 / RFC 6712: Certificate Management Protocol (CMPv2)
//!
//! Pure standard Rust (`std`/`core` only), zero external dependencies.

use std::collections::HashMap;
use std::fmt;

/// Simple FNV-1a 64-bit hashing with salt for credential security in pure Rust.
pub fn hash_password(password: &str, salt: u32) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325 ^ (salt as u64);
    for b in password.as_bytes() {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

/// User Role defined for O-RU M-Plane Role-Based Access Control (o-ran-usermgmt.yang).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UserRole {
    /// Super-User (e.g. `sudo`): full administrative access, certs, and system reboots.
    SuperUser,
    /// Operator: configuration read/write, status polling, cannot alter credentials.
    Operator,
    /// Field Installer: initial commissioning, self-tests, and limited configuration.
    Installer,
    /// Security Auditor: read-only access to audit logs, alarms, and PM data.
    Auditor,
}

impl fmt::Display for UserRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UserRole::SuperUser => write!(f, "super-user"),
            UserRole::Operator => write!(f, "operator"),
            UserRole::Installer => write!(f, "installer"),
            UserRole::Auditor => write!(f, "auditor"),
        }
    }
}

/// Access permission type for NACM (RFC 8341).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessPermission {
    Read,
    ReadWrite,
    Execute,
}

/// Managed user account profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserAccount {
    pub username: String,
    pub password_hash: u64,
    pub salt: u32,
    pub role: UserRole,
    pub failed_login_attempts: u32,
    pub locked_until_epoch: Option<u64>,
}

/// Type of X.509 Certificate managed in O-RU (o-ran-certificates.yang).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CertificateType {
    /// O-RU manufacturer/operator identity certificate.
    DeviceIdentity,
    /// Trusted Certification Authority (Root or Intermediate CA).
    TrustAnchor,
    /// TLS Server certificate for NETCONF over TLS (RFC 7589).
    TlsServer,
    /// TLS Client certificate for mutual TLS authentication with O-DU.
    TlsClient,
}

/// Managed X.509 Certificate Record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct X509CertRecord {
    pub cert_id: String,
    pub subject: String,
    pub issuer: String,
    pub cert_type: CertificateType,
    pub serial_number: u64,
    pub not_before_epoch: u64,
    pub not_after_epoch: u64,
    pub is_revoked: bool,
    pub fingerprint_sha256: [u8; 32],
}

impl X509CertRecord {
    /// Check whether the certificate is within its valid date range.
    pub fn is_valid_at(&self, current_epoch: u64) -> bool {
        !self.is_revoked
            && current_epoch >= self.not_before_epoch
            && current_epoch <= self.not_after_epoch
    }

    /// Check if certificate is expired.
    pub fn is_expired(&self, current_epoch: u64) -> bool {
        current_epoch > self.not_after_epoch
    }

    /// Calculate remaining days until expiration.
    pub fn days_until_expiry(&self, current_epoch: u64) -> i64 {
        let diff = self.not_after_epoch as i64 - current_epoch as i64;
        diff / 86400
    }
}

/// CMPv2 Message Types (RFC 4210).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cmpv2MessageType {
    InitializationRequest = 0,
    InitializationResponse = 1,
    CertificationRequest = 2,
    CertificationResponse = 3,
    KeyUpdateRequest = 7,
    KeyUpdateResponse = 8,
    RevocationRequest = 11,
    RevocationResponse = 12,
    Confirmation = 19,
}

/// CMPv2 PKIStatus (RFC 4210 §5.2.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cmpv2Status {
    Accepted = 0,
    GrantedWithMods = 1,
    Rejection = 2,
    Waiting = 3,
}

/// Lightweight CMPv2 wire message container.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cmpv2Message {
    pub transaction_id: u32,
    pub msg_type: Cmpv2MessageType,
    pub status: Cmpv2Status,
    pub sender_nonce: u32,
    pub recipient_nonce: u32,
    pub cert_data: Option<Vec<u8>>,
}

impl Cmpv2Message {
    /// Serialize message into wire bytes.
    /// Wire format:
    /// [TxID (4B)][MsgType (1B)][Status (1B)][SenderNonce (4B)][RecipientNonce (4B)][DataLen (2B)][Data (N B)]
    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&self.transaction_id.to_be_bytes());
        buf.push(self.msg_type as u8);
        buf.push(self.status as u8);
        buf.extend_from_slice(&self.sender_nonce.to_be_bytes());
        buf.extend_from_slice(&self.recipient_nonce.to_be_bytes());

        if let Some(ref data) = self.cert_data {
            let len = data.len() as u16;
            buf.extend_from_slice(&len.to_be_bytes());
            buf.extend_from_slice(data);
        } else {
            buf.extend_from_slice(&0u16.to_be_bytes());
        }
        buf
    }

    /// Parse a CMPv2 message from wire bytes.
    pub fn parse(bytes: &[u8]) -> Result<Self, &'static str> {
        if bytes.len() < 16 {
            return Err("CMPv2 message buffer too short");
        }

        let transaction_id = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let msg_type = match bytes[4] {
            0 => Cmpv2MessageType::InitializationRequest,
            1 => Cmpv2MessageType::InitializationResponse,
            2 => Cmpv2MessageType::CertificationRequest,
            3 => Cmpv2MessageType::CertificationResponse,
            7 => Cmpv2MessageType::KeyUpdateRequest,
            8 => Cmpv2MessageType::KeyUpdateResponse,
            11 => Cmpv2MessageType::RevocationRequest,
            12 => Cmpv2MessageType::RevocationResponse,
            19 => Cmpv2MessageType::Confirmation,
            _ => return Err("Unknown CMPv2 message type"),
        };

        let status = match bytes[5] {
            0 => Cmpv2Status::Accepted,
            1 => Cmpv2Status::GrantedWithMods,
            2 => Cmpv2Status::Rejection,
            3 => Cmpv2Status::Waiting,
            _ => return Err("Invalid CMPv2 status code"),
        };

        let sender_nonce = u32::from_be_bytes([bytes[6], bytes[7], bytes[8], bytes[9]]);
        let recipient_nonce = u32::from_be_bytes([bytes[10], bytes[11], bytes[12], bytes[13]]);

        let data_len = u16::from_be_bytes([bytes[14], bytes[15]]) as usize;
        let cert_data = if data_len > 0 {
            if bytes.len() < 16 + data_len {
                return Err("Truncated CMPv2 payload data");
            }
            Some(bytes[16..16 + data_len].to_vec())
        } else {
            None
        };

        Ok(Self {
            transaction_id,
            msg_type,
            status,
            sender_nonce,
            recipient_nonce,
            cert_data,
        })
    }
}

/// Security event severity levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecurityEventSeverity {
    Informational,
    Warning,
    Minor,
    Major,
    Critical,
}

/// Security audit log entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecurityAuditRecord {
    pub timestamp_epoch: u64,
    pub severity: SecurityEventSeverity,
    pub source_ip: String,
    pub username: Option<String>,
    pub description: String,
}

/// Summary report of O-RU security health.
#[derive(Debug, Clone, PartialEq)]
pub struct SecurityAuditSummary {
    pub total_users: usize,
    pub locked_users: usize,
    pub total_certificates: usize,
    pub valid_certificates: usize,
    pub expiring_certificates: usize,
    pub revoked_certificates: usize,
    pub total_audit_events: usize,
    pub critical_events_count: usize,
}

/// Primary O-RAN WG4 M-Plane Security and Certificate Lifecycle Manager.
#[derive(Debug, Clone)]
pub struct OranSecurityManager {
    /// Managed user database (username -> UserAccount).
    users: HashMap<String, UserAccount>,
    /// Managed X.509 certificate store (cert_id -> X509CertRecord).
    certificates: HashMap<String, X509CertRecord>,
    /// Security audit trail.
    audit_log: Vec<SecurityAuditRecord>,
    /// Threshold in days to raise certificate expiration warnings (default: 30 days).
    pub cert_expiry_warning_days: i64,
    /// Maximum failed login attempts before locking account (default: 5).
    pub max_failed_login_attempts: u32,
    /// Account lockout duration in seconds (default: 900s = 15 minutes).
    pub lockout_duration_sec: u64,
}

impl Default for OranSecurityManager {
    fn default() -> Self {
        Self::new()
    }
}

impl OranSecurityManager {
    /// Construct a new Security Manager with default root policies.
    pub fn new() -> Self {
        let mut mgr = Self {
            users: HashMap::new(),
            certificates: HashMap::new(),
            audit_log: Vec::new(),
            cert_expiry_warning_days: 30,
            max_failed_login_attempts: 5,
            lockout_duration_sec: 900,
        };

        // Create default super-user account
        mgr.add_user("admin", "Admin@12345", UserRole::SuperUser, 1001)
            .expect("Failed to add default admin");
        mgr
    }

    /// Add a new user account.
    pub fn add_user(
        &mut self,
        username: &str,
        password: &str,
        role: UserRole,
        salt: u32,
    ) -> Result<(), &'static str> {
        if self.users.contains_key(username) {
            return Err("User account already exists");
        }
        if username.len() < 3 {
            return Err("Username too short");
        }
        if password.len() < 6 {
            return Err("Password too short; minimum 6 characters required");
        }

        let password_hash = hash_password(password, salt);
        self.users.insert(
            username.to_string(),
            UserAccount {
                username: username.to_string(),
                password_hash,
                salt,
                role,
                failed_login_attempts: 0,
                locked_until_epoch: None,
            },
        );
        Ok(())
    }

    /// Authenticate a user via credentials, with lockout protection and audit logging.
    pub fn authenticate_user(
        &mut self,
        username: &str,
        password: &str,
        source_ip: &str,
        current_epoch: u64,
    ) -> Result<UserRole, &'static str> {
        let auth_status = {
            let user = self
                .users
                .get_mut(username)
                .ok_or("User account does not exist")?;

            // 1. Check if account is currently locked
            if let Some(locked_until) = user.locked_until_epoch {
                if current_epoch < locked_until {
                    Err("Account is locked due to excessive failed attempts")
                } else {
                    user.locked_until_epoch = None;
                    user.failed_login_attempts = 0;
                    let attempt_hash = hash_password(password, user.salt);
                    if attempt_hash == user.password_hash {
                        user.failed_login_attempts = 0;
                        Ok(user.role)
                    } else {
                        user.failed_login_attempts += 1;
                        Err("Password incorrect")
                    }
                }
            } else {
                let attempt_hash = hash_password(password, user.salt);
                if attempt_hash == user.password_hash {
                    user.failed_login_attempts = 0;
                    Ok(user.role)
                } else {
                    user.failed_login_attempts += 1;
                    if user.failed_login_attempts >= self.max_failed_login_attempts {
                        user.locked_until_epoch = Some(current_epoch + self.lockout_duration_sec);
                        Err("Password incorrect; account locked out")
                    } else {
                        Err("Password incorrect")
                    }
                }
            }
        };

        match auth_status {
            Ok(role) => {
                self.log_event(
                    current_epoch,
                    SecurityEventSeverity::Informational,
                    source_ip,
                    Some(username),
                    "User successfully authenticated",
                );
                Ok(role)
            }
            Err("Account is locked due to excessive failed attempts") => {
                self.log_event(
                    current_epoch,
                    SecurityEventSeverity::Major,
                    source_ip,
                    Some(username),
                    "Login attempt on locked user account rejected",
                );
                Err("Account is locked due to excessive failed attempts")
            }
            Err("Password incorrect; account locked out") => {
                self.log_event(
                    current_epoch,
                    SecurityEventSeverity::Critical,
                    source_ip,
                    Some(username),
                    "Account locked out due to multiple consecutive failed logins",
                );
                Err("Password incorrect; account locked out")
            }
            Err(e) => {
                self.log_event(
                    current_epoch,
                    SecurityEventSeverity::Warning,
                    source_ip,
                    Some(username),
                    "Failed login attempt: invalid password",
                );
                Err(e)
            }
        }
    }

    /// Evaluate NACM (RFC 8341) access control for a given user role and operation.
    pub fn check_nacm_access(
        &self,
        role: UserRole,
        module_path: &str,
        permission: AccessPermission,
    ) -> bool {
        match role {
            UserRole::SuperUser => true, // Full administrative rights
            UserRole::Operator => {
                // Operators can configure carriers, delay, and ALDs, but cannot alter users or security
                if module_path.starts_with("/o-ran-usermgmt")
                    || module_path.starts_with("/o-ran-certificates")
                {
                    permission == AccessPermission::Read
                } else {
                    true // ReadWrite permitted on radio & transmission modules
                }
            }
            UserRole::Installer => {
                // Installers have read access and can only execute self-test / diagnostics
                match permission {
                    AccessPermission::Read => true,
                    AccessPermission::Execute => {
                        module_path.contains("self-test") || module_path.contains("ping")
                    }
                    AccessPermission::ReadWrite => false,
                }
            }
            UserRole::Auditor => {
                // Auditor has strict read-only access to all modules
                permission == AccessPermission::Read
            }
        }
    }

    /// Install or update an X.509 certificate in the store.
    pub fn install_certificate(&mut self, cert: X509CertRecord) {
        self.certificates.insert(cert.cert_id.clone(), cert);
    }

    /// Revoke a certificate.
    pub fn revoke_certificate(
        &mut self,
        cert_id: &str,
        source_ip: &str,
        current_epoch: u64,
    ) -> Result<(), &'static str> {
        let cert = self
            .certificates
            .get_mut(cert_id)
            .ok_or("Certificate not found")?;
        cert.is_revoked = true;

        self.log_event(
            current_epoch,
            SecurityEventSeverity::Major,
            source_ip,
            None,
            &format!("Certificate '{}' has been revoked", cert_id),
        );
        Ok(())
    }

    /// Validate a certificate's status and expiration at the current epoch.
    pub fn validate_certificate(
        &self,
        cert_id: &str,
        current_epoch: u64,
    ) -> Result<(), &'static str> {
        let cert = self
            .certificates
            .get(cert_id)
            .ok_or("Certificate not found")?;

        if cert.is_revoked {
            return Err("Certificate is revoked");
        }
        if current_epoch < cert.not_before_epoch {
            return Err("Certificate is not yet valid");
        }
        if current_epoch > cert.not_after_epoch {
            return Err("Certificate has expired");
        }
        Ok(())
    }

    /// Process a CMPv2 Certificate Management Protocol request (RFC 4210).
    pub fn process_cmpv2_request(
        &mut self,
        request: &Cmpv2Message,
        current_epoch: u64,
    ) -> Result<Cmpv2Message, &'static str> {
        match request.msg_type {
            Cmpv2MessageType::InitializationRequest | Cmpv2MessageType::CertificationRequest => {
                // Issue / enroll certificate
                let response = Cmpv2Message {
                    transaction_id: request.transaction_id,
                    msg_type: Cmpv2MessageType::CertificationResponse,
                    status: Cmpv2Status::Accepted,
                    sender_nonce: request.recipient_nonce + 1,
                    recipient_nonce: request.sender_nonce,
                    cert_data: Some(vec![0x30, 0x82, 0x01, 0x00]), // Simulated X.509 DER
                };
                self.log_event(
                    current_epoch,
                    SecurityEventSeverity::Informational,
                    "CMPv2-Server",
                    None,
                    "Processed CMPv2 Certificate Enrollment Request",
                );
                Ok(response)
            }
            Cmpv2MessageType::KeyUpdateRequest => {
                // Key update renewal
                let response = Cmpv2Message {
                    transaction_id: request.transaction_id,
                    msg_type: Cmpv2MessageType::KeyUpdateResponse,
                    status: Cmpv2Status::Accepted,
                    sender_nonce: request.recipient_nonce + 1,
                    recipient_nonce: request.sender_nonce,
                    cert_data: Some(vec![0x30, 0x82, 0x01, 0x01]),
                };
                self.log_event(
                    current_epoch,
                    SecurityEventSeverity::Informational,
                    "CMPv2-Server",
                    None,
                    "Processed CMPv2 Key Update / Certificate Renewal Request",
                );
                Ok(response)
            }
            Cmpv2MessageType::RevocationRequest => {
                let response = Cmpv2Message {
                    transaction_id: request.transaction_id,
                    msg_type: Cmpv2MessageType::RevocationResponse,
                    status: Cmpv2Status::Accepted,
                    sender_nonce: request.recipient_nonce + 1,
                    recipient_nonce: request.sender_nonce,
                    cert_data: None,
                };
                Ok(response)
            }
            _ => Err("Unsupported CMPv2 request message type"),
        }
    }

    /// Log a security audit record.
    pub fn log_event(
        &mut self,
        timestamp_epoch: u64,
        severity: SecurityEventSeverity,
        source_ip: &str,
        username: Option<&str>,
        description: &str,
    ) {
        self.audit_log.push(SecurityAuditRecord {
            timestamp_epoch,
            severity,
            source_ip: source_ip.to_string(),
            username: username.map(|s| s.to_string()),
            description: description.to_string(),
        });
    }

    /// Retrieve reference to all audit records.
    pub fn audit_log(&self) -> &[SecurityAuditRecord] {
        &self.audit_log
    }

    /// Generate an overall security status audit summary.
    pub fn audit_summary(&self, current_epoch: u64) -> SecurityAuditSummary {
        let total_users = self.users.len();
        let locked_users = self
            .users
            .values()
            .filter(|u| {
                u.locked_until_epoch
                    .map(|t| current_epoch < t)
                    .unwrap_or(false)
            })
            .count();

        let total_certificates = self.certificates.len();
        let mut valid_certificates = 0;
        let mut expiring_certificates = 0;
        let mut revoked_certificates = 0;

        for c in self.certificates.values() {
            if c.is_revoked {
                revoked_certificates += 1;
            } else if c.is_valid_at(current_epoch) {
                valid_certificates += 1;
                let days = c.days_until_expiry(current_epoch);
                if days <= self.cert_expiry_warning_days && days >= 0 {
                    expiring_certificates += 1;
                }
            }
        }

        let total_audit_events = self.audit_log.len();
        let critical_events_count = self
            .audit_log
            .iter()
            .filter(|e| e.severity == SecurityEventSeverity::Critical)
            .count();

        SecurityAuditSummary {
            total_users,
            locked_users,
            total_certificates,
            valid_certificates,
            expiring_certificates,
            revoked_certificates,
            total_audit_events,
            critical_events_count,
        }
    }
}
