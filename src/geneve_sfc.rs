//! Geneve Service Function Chaining (Geneve-SFC - RFC 8926 / RFC 8300).
//!
//! Integrates Network Service Header (NSH) metadata and In-Band SFC Path IDs directly
//! into Geneve Cloud Overlay tunnels for dynamic NFV / Middlebox service forwarding.

use crate::geneve::{GeneveOption, GenevePacket};
use crate::geneve_opts::GeneveOptionTlv;

pub const GENEVE_OPT_CLASS_SFC: u16 = 0x0104;
pub const GENEVE_OPT_TYPE_SFC_PATH: u8 = 0x01;
pub const GENEVE_OPT_TYPE_SFC_CONTEXT: u8 = 0x02;
pub const ETHERTYPE_NSH: u16 = 0x894F;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneveSfcHop {
    pub vni: u32,
    pub service_path_id: u32, // 24-bit SPI
    pub service_index: u8,    // 8-bit SI
    pub tenant_id: u32,
    pub security_group: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneveSfcPacket {
    pub vni: u32,
    pub protocol_type: u16,
    pub sfc_metadata: GeneveSfcHop,
    pub payload: Vec<u8>,
}

impl GeneveSfcPacket {
    pub fn build(
        vni: u32,
        protocol_type: u16,
        sfc_metadata: GeneveSfcHop,
        payload: &[u8],
    ) -> Self {
        GeneveSfcPacket {
            vni,
            protocol_type,
            sfc_metadata,
            payload: payload.to_vec(),
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut options = Vec::new();

        // 1. SFC Path Option (Class 0x0104, Type 0x01, Len 4B: 24-bit SPI + 8-bit SI)
        let mut path_data = Vec::with_capacity(4);
        let spi_bytes = self.sfc_metadata.service_path_id.to_be_bytes();
        path_data.push(spi_bytes[1]);
        path_data.push(spi_bytes[2]);
        path_data.push(spi_bytes[3]);
        path_data.push(self.sfc_metadata.service_index);

        let path_opt = GeneveOptionTlv::new(
            GENEVE_OPT_CLASS_SFC,
            GENEVE_OPT_TYPE_SFC_PATH,
            true, // Critical
            &path_data,
        );
        options.push(GeneveOption {
            class: path_opt.class,
            opt_type: path_opt.type_code,
            critical: path_opt.critical,
            data: path_opt.data,
        });

        // 2. SFC Context Option (Class 0x0104, Type 0x02, Len 8B: Tenant ID + Security Group)
        let mut ctx_data = Vec::with_capacity(8);
        ctx_data.extend_from_slice(&self.sfc_metadata.tenant_id.to_be_bytes());
        ctx_data.extend_from_slice(&self.sfc_metadata.security_group.to_be_bytes());

        let ctx_opt = GeneveOptionTlv::new(
            GENEVE_OPT_CLASS_SFC,
            GENEVE_OPT_TYPE_SFC_CONTEXT,
            false,
            &ctx_data,
        );
        options.push(GeneveOption {
            class: ctx_opt.class,
            opt_type: ctx_opt.type_code,
            critical: ctx_opt.critical,
            data: ctx_opt.data,
        });

        let geneve_pkt = GenevePacket {
            version: 0,
            oam: false,
            critical: true,
            protocol_type: self.protocol_type,
            vni: self.vni,
            options,
            payload: self.payload.clone(),
        };

        geneve_pkt.serialize()
    }

    pub fn parse(data: &[u8]) -> Option<Self> {
        let geneve = GenevePacket::parse(data).ok()?;

        let mut spi = 0;
        let mut si = 255;
        let mut tenant_id = 0;
        let mut security_group = 0;

        for opt in &geneve.options {
            if opt.class == GENEVE_OPT_CLASS_SFC {
                if opt.opt_type == GENEVE_OPT_TYPE_SFC_PATH && opt.data.len() >= 4 {
                    spi = u32::from_be_bytes([0, opt.data[0], opt.data[1], opt.data[2]]);
                    si = opt.data[3];
                } else if opt.opt_type == GENEVE_OPT_TYPE_SFC_CONTEXT && opt.data.len() >= 8 {
                    tenant_id = u32::from_be_bytes([opt.data[0], opt.data[1], opt.data[2], opt.data[3]]);
                    security_group = u32::from_be_bytes([opt.data[4], opt.data[5], opt.data[6], opt.data[7]]);
                }
            }
        }

        Some(GeneveSfcPacket {
            vni: geneve.vni,
            protocol_type: geneve.protocol_type,
            sfc_metadata: GeneveSfcHop {
                vni: geneve.vni,
                service_path_id: spi,
                service_index: si,
                tenant_id,
                security_group,
            },
            payload: geneve.payload,
        })
    }

    /// Advances the Service Index (SI) as the packet traverses a Service Function hop
    pub fn advance_service_hop(&mut self) -> bool {
        if self.sfc_metadata.service_index > 0 {
            self.sfc_metadata.service_index -= 1;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_geneve_sfc_framing_and_hop_progression() {
        let hop = GeneveSfcHop {
            vni: 7001,
            service_path_id: 0x001234,
            service_index: 3,
            tenant_id: 100,
            security_group: 50,
        };

        let payload = b"Enterprise Web Traffic through Firewall/DPI/WAF Chain";
        let mut sfc_pkt = GeneveSfcPacket::build(7001, 0x0800, hop, payload);
        let raw = sfc_pkt.serialize();

        let parsed = GeneveSfcPacket::parse(&raw).unwrap();
        assert_eq!(parsed.vni, 7001);
        assert_eq!(parsed.sfc_metadata.service_path_id, 0x001234);
        assert_eq!(parsed.sfc_metadata.service_index, 3);
        assert_eq!(parsed.sfc_metadata.tenant_id, 100);
        assert_eq!(parsed.sfc_metadata.security_group, 50);
        assert_eq!(parsed.payload, payload);

        // Advance hop: Firewall -> DPI
        assert!(sfc_pkt.advance_service_hop());
        assert_eq!(sfc_pkt.sfc_metadata.service_index, 2);

        // Advance hop: DPI -> WAF
        assert!(sfc_pkt.advance_service_hop());
        assert_eq!(sfc_pkt.sfc_metadata.service_index, 1);
    }
}
