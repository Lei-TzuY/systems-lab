# Toy TCP/IP Stack

A complete educational dual-stack IPv4/IPv6 network protocol stack built from scratch in safe Rust with zero third-party networking dependencies.

---

## 🌟 Supported Protocols & Subsystems

| Layer | Protocol / System | Implementation Highlights |
|---|---|---|
| **Capture / Offline** | **PCAP Reader & Writer** | Parses 24-byte Libpcap global headers, auto-detects endianness, reads & writes 16-byte packet record headers. |
| **Link (L2)** | **Ethernet II** | MAC addresses (`MacAddress`), unicast/multicast/broadcast detection, EtherType dispatching (`0x0800 IPv4`, `0x0806 ARP`, `0x86DD IPv6`). |
| **VLAN (L2)** | **IEEE 802.1Q VLAN** | 4-byte 802.1Q tag parser/serializer (TPID `0x8100`, 12-bit VID 1..4094, 3-bit Priority Code Point), frame tagging and stripping. |
| **Flow Control** | **IEEE 802.1Qbb PFC** | Priority-based Flow Control pause frame over EtherType `0x8808` / Opcode `0x0101` with 8 per-priority class pause quantums for lossless Ethernet fabrics. |
| **Loop Prevention** | **Spanning Tree (STP / IEEE 802.1D)** | BPDU framing, Root Bridge election, Root Path Cost, Port Roles (Root, Designated, Blocked), Port States (`Blocking`, `Listening`, `Learning`, `Forwarding`). |
| **Link Aggregation**| **LACP (IEEE 802.1AX / 802.3ad)** | EtherType `0x8809`, Actor & Partner TLV negotiation, Bond device aggregation, FNV 5-tuple hash egress distribution. |
| **Carrier WAN Access**| **PPPoE (RFC 2516)** | Discovery Stage (PADI, PADO, PADR, PADS, PADT) over EtherType `0x8863` and Session Stage over EtherType `0x8864` with PPP encapsulation (`0x0021` IPv4). |
| **Pseudowire (L2)** | **L2TPv3 (RFC 3931)** | Layer 2 Tunneling Protocol v3 over IP Protocol 115, 32-bit Session ID, optional 64-bit Cookie, transparent Ethernet pseudowire encapsulation. |
| **Link Discovery** | **LLDP (IEEE 802.1AB)** | Layer 2 discovery over EtherType `0x88CC` and Multicast MAC `01:80:C2:00:00:0E`, TLV engine (Chassis ID, Port ID, TTL, System Name), neighbor cache. |
| **Cisco Discovery** | **CDPv2 (Cisco Discovery Protocol)** | Layer 2 discovery over SNAP/LLC framing (`0xAAAA03`, OUI `0x00000C`, PID `0x2000`) & Multicast MAC `01:00:0C:CC:CC:CC`, TLV parser/builder, neighbor cache table. |
| **Cisco VLAN Trunk**| **VTP (VLAN Trunking Protocol)** | Multi-switch VLAN database sync over Cisco SNAP (`0xAAAA03`, OUI `0x00000C`, PID `0x2003`), Summary/Subset advertisements, Configuration Revisions. |
| **Carrier Routing** | **IS-IS (ISO/IEC 10589 / RFC 1195)**| Direct Layer 2 link-state routing over EtherType `0x8870`, NLPID Discriminator `0x83`, L1/L2 LAN IIH framing, Area & NLPID TLVs (`0xCC` IPv4, `0x8E` IPv6). |
| **Resolution (L2.5)** | **ARP (RFC 826)** | 28-byte ARP header parser/builder, Opcode 1 (Request) & 2 (Reply), dynamic in-memory ARP Cache table. |
| **Label Switching** | **MPLS (RFC 3031 / RFC 3032)** | 32-bit (4-byte) MPLS shim header, 20-bit Label, 3-bit TC, Bottom of Stack (S), TTL, EtherType `0x8847`, Ingress Push, Core Swap, Egress Pop LFIB table. |
| **Label Signaling** | **MPLS LDP (RFC 5036)** | Label Distribution Protocol over UDP/TCP port 646, Hello Discovery, Label Mapping FEC TLV & Generic Label TLVs, dynamic LFIB prefix injection. |
| **Traffic Engineering**| **MPLS-TE / RSVP-TE (RFC 3209)** | Resource Reservation Protocol with TE extensions over IP Protocol 46, Explicit Route Object (ERO), SENDER_TSPEC bandwidth reservation, downstream label signaling. |
| **Path Computation**| **PCEP & SR-MPLS (RFC 5440 / 8664)**| Path Computation Element Communication Protocol over TCP port 4189, Segment Routing SR-ERO label stack computation (Node SID, Adj SID). |
| **Time-Sensitive** | **IEEE 802.1AS gPTP / TSN** | Generalized Precision Time Protocol for AVB & TSN over EtherType `0x88F7`, Pdelay peer delay mechanism, zero-jitter sub-nanosecond clock sync. |
| **In-Band Telemetry**| **IOAM (RFC 9197 / RFC 9326)** | In-situ Operations, Administration, and Maintenance recording per-hop Node ID, Ingress/Egress Interface IDs, Timestamps, and Transit Queue Delay within packet headers. |
| **Network Config** | **NETCONF XML-RPC (RFC 6241)** | Network Configuration Protocol over TCP 830, XML-RPC framing (`]]>]]>`), datastores (`running`, `candidate`), `<get-config>`, `<edit-config>`, `<commit>`. |
| **Overlay / Mobility**| **LISP (RFC 9300 / RFC 9301)** | Locator/ID Separation Protocol decoupling Endpoint Identifiers (EID) from Routing Locators (RLOC) over UDP 4341 (Data) and UDP 4342 (Map-Request/Reply). |
| **DDoS Mitigation**| **BGP Flowspec (RFC 5575 / RFC 8955)**| Dynamic Dissemination of Flow Specification Rules (AFI 1 / SAFI 133), Destination/Source Prefixes, IP Protocol, Port ranges, TCP Flags matchers, Drop/Rate-Limit/Redirect actions. |
| **Cloud Observability**| **OpenTelemetry OTLP (Ports 4317/4318)**| Cloud-native metrics (Counters, Gauges, Histograms) and distributed trace spans export over gRPC (TCP 4317) & HTTP (TCP 4318). |
| **Native IPv6 Tunnel**| **GRE-over-IPv6 (RFC 7676 / RFC 2473)**| Multi-protocol tunneling (IPv4, IPv6, MPLS, Ethernet) over native IPv6 networks using Next Header 47 (GRE). |
| **Modern VPN (L3)** | **WireGuard VPN Protocol** | Fast, secure Noise IK point-to-point UDP tunnel encapsulation over UDP port 51820, 1-RTT handshake, Type 4 authenticated data packets. |
| **SDN Datapath** | **OpenFlow v1.3 (ONF TS-025)** | SDN switch control plane over TCP port 6653, OXM Match engine, Instruction & Action pipelines (`Output`, `SetVlan`, `Drop`), prioritized `OfpFlowTable`. |
| **Carrier AAA Core**| **Diameter Base Protocol (RFC 6733)** | 4G/5G mobile core & IMS AAA protocol over TCP/SCTP port 3868, 20-byte header, AVP encoding (`Origin-Host`, `Origin-Realm`, `Vendor-Id`), CER/CEA handshake. |
| **Port Mirroring** | **ERSPAN & NVGRE (RFC 7637)**| Cisco ERSPAN Type II/III remote packet mirroring over GRE (Protocol 47, EtherType `0x88BE`), NVGRE 24-bit VSID overlay. |
| **Precision Time** | **IEEE 1588v2 PTP** | Sub-microsecond time synchronization over UDP 319/320 and EtherType `0x88F7`, Sync, Follow_Up, Delay_Resp, nanosecond master-to-slave offset & delay calculation. |
| **Redundancy** | **VRRPv3 (RFC 5798)** | Virtual Router Redundancy Protocol over IP 112 / `224.0.0.18`, Master/Backup election, virtual gateway MAC (`00:00:5E:00:01:XX`). |
| **Cisco Redundancy**| **HSRPv1 (RFC 2281)** | Hot Standby Router Protocol over UDP 1985 / `224.0.0.2`, Virtual MAC `00:00:0C:07:AC:XX`, Hello/Coup/Resign state machine, Active/Standby router election with preemption. |
| **Cisco Load Balancer**| **GLBP (Cisco GLBP)** | Gateway Load Balancing Protocol over UDP 3222 / `224.0.0.102`, Active Virtual Gateway (AVG) & Active Virtual Forwarders (AVF), Virtual MAC `00:07:B4:00:GG:FF`, Round-Robin load balancing. |
| **Fast Liveness** | **BFD (RFC 5880 / RFC 5881)** | Bidirectional Forwarding Detection over UDP 3784, 24-byte control packet, 3-way handshake (`Down` $\rightarrow$ `Init` $\rightarrow$ `Up`), sub-second link failure detection. |
| **Network (L3)** | **IPv4 (RFC 791)** | Header parser/builder (IHL, TTL, Identification, DF/MF flags, Protocol demux), header checksum verification & recalculation. |
| **Network (L3)** | **IPv6 (RFC 8200, 5952)** | 128-bit `Ipv6Address` with canonical zero-compression string formatting, 40-byte fixed header, Next Header dispatching, IPv6 pseudo-header checksum. |
| **Segment Routing** | **SRv6 (RFC 8754 / RFC 8986)** | Segment Routing over IPv6 Extension Header (SRH Type 4), 128-bit SID list, Segments Left pointer advancement, destination address mutation. |
| **Service Chaining**| **NSH (RFC 8300)** | Network Service Header for Service Function Chaining (SFC), 4-byte Base Header, 4-byte SPI/SI Service Path Header, 16-byte MD Type 1 Context Headers. |
| **IPv6 Transition** | **6in4 (RFC 4213) & 4in6 (RFC 2473)**| Dual-stack transition tunnels: IPv6 over IPv4 (Protocol 41) & IPv4 over IPv6 (Next Header 4). |
| **Diagnostics / PMTUD** | **Traceroute & PMTUD (RFC 1191)** | ICMP Time Exceeded (Type 11 Code 0) on TTL expiration, ICMP Frag-Needed (Type 3 Code 4) carrying Next-Hop MTU for Path MTU Discovery. |
| **VPN & Security (L3)**| **IPsec ESP (RFC 4303)** | IP Protocol 50, SPI, Sequence Number, Padding, Next Header, 16-byte ICV Authentication Tag, Security Association Database (SAD) & Anti-Replay window. |
| **Cloud Overlay** | **Geneve (RFC 8926)** | Generic Network Virtualization Encapsulation over UDP 6081 with 24-bit VNI, variable Option TLVs, and transparent Ethernet frame transport (`0x6558`). |
| **Overlay / Tunneling**| **VXLAN (RFC 7348)** | Layer 2 overlay encapsulation over UDP port 4789 with 24-bit VNI (16 million subnets) and inner Ethernet frame transport. |
| **Multi-Protocol Overlay**| **VXLAN-GPE (UDP 4790)** | VXLAN Generic Protocol Extension for direct IPv4 (`0x01`), IPv6 (`0x02`), Ethernet (`0x03`), and MPLS (`0x05`) multi-protocol overlay tunneling. |
| **Generic UDP Encap**| **GUE (RFC 7763) & FOU (RFC 8086)** | Generic UDP Encapsulation (UDP 6080) with 4-byte header and Foo-over-UDP (UDP 5555) direct packet transport for cloud underlay RSS. |
| **4G/5G Cellular** | **GTP-U (3GPP TS 29.281)** | GPRS Tunnelling Protocol User Plane over UDP 2152, G-PDU payload encapsulation, 32-bit TEID session routing, Echo Request/Response. |
| **Network Tunneling** | **GRE (RFC 2784) & IP-in-IP (RFC 2003)** | Generic Routing Encapsulation (IP protocol 47) with Checksum, Key, Sequence options; direct IP-in-IP (Protocol 4) site-to-site overlay encapsulation. |
| **Multicast Mgmt** | **IGMPv2 (RFC 2236) & Multicast MAC** | Membership Query, V2 Membership Report, Leave Group, dynamic subscription manager, RFC 1112 multicast IP to MAC mapping. |
| **Multicast Routing**| **PIM-SM (RFC 7761)** | Protocol Independent Multicast - Sparse Mode over IP Protocol 103 / `224.0.0.13`, Hello, Rendezvous Point (RP) Shared Tree $(*, G)$, Join/Prune signaling. |
| **Link-State Routing** | **OSPFv2 (RFC 2328)** | Link-State Interior Gateway Protocol over IP 89 / `224.0.0.5`, 24-byte OSPF headers, Hello packets, LSDB graph, Dijkstra SPF calculation. |
| **Advanced Routing** | **Cisco EIGRP & DUAL (RFC 7868)** | IP Protocol 88 over Multicast `224.0.0.10`, 20-byte EIGRP header, composite metric formula ($256 \times (10^7/\text{BW} + \text{Delay})$), Feasibility Condition ($RD < FD$), Successor & Feasible Successor loop-free backup path. |
| **Dynamic Routing** | **RIPv2 (RFC 2453)** | Distance-Vector dynamic routing protocol over UDP 520, Bellman-Ford algorithm with Split Horizon & Poison Reverse, metric calculations (1..16). |
| **Inter-Domain Route**| **BGP-4 (RFC 4271)** | Packet-driven BGP speaker running over this stack's own TCP sockets on port 179: full Idle/Connect/Active/OpenSent/OpenConfirm/Established FSM, OPEN negotiation, ConnectRetry / Hold / Keepalive timers, stream reassembly, Adj-RIB-In / Loc-RIB / Adj-RIB-Out, best-path selection, and installation into the real IPv4 forwarding table. |
| **BGP EVPN Fabric** | **MP-BGP EVPN (RFC 4760 / 5492 / 6793 / 7432 / 8365)** | Packet-driven MP-BGP EVPN on the same TCP session as IPv4 unicast: OPEN capability negotiation (Multiprotocol, Four-Octet AS), 32-bit ASNs with `AS_TRANS` / `AS4_PATH`, `MP_REACH_NLRI` / `MP_UNREACH_NLRI` carrying Route Type 2 and Type 3 NLRI, Route Target import, EVPN Adj-RIB-In / Loc-RIB / Adj-RIB-Out, MAC mobility, and a VTEP whose VXLAN forwarding is programmed only from the EVPN Loc-RIB. |
| **Fragmentation (L3)** | **IP Fragmenter & Reassembler** | Splits $> \text{MTU}$ packets into 8-byte aligned slices with `MF` flags; reassembles out-of-order fragment streams. |
| **Control (L3.5)** | **ICMP (RFC 792)** | Type 8 (Echo Request / Ping) and Type 0 (Echo Reply), identifier & sequence number tracking, payload preservation. |
| **Control (L3.5)** | **ICMPv6 & NDP (RFC 4443, 4861)** | ICMPv6 Echo Request/Reply (`ping6`), Neighbor Solicitation (NS) / Neighbor Advertisement (NA), dynamic in-memory `NdpTable` (Neighbor Cache). |
| **Time Sync** | **NTPv4 (RFC 5905)** | 48-byte NTP packet, 64-bit timestamps, clock offset $\theta$ and round-trip delay $\delta$ calculation over UDP 123. |
| **Flow Telemetry** | **NetFlow v9 & IPFIX (RFC 3954 / RFC 7011)**| Flow accounting exporter and cache table over UDP 2055, Template & Data FlowSets, 5-tuple flow metrics aggregation. |
| **Sampled Telemetry**| **sFlow v5 (RFC 3176)** | High-speed hardware packet sampling and interface counter exporter over UDP 6343. |
| **VoIP Signaling** | **SIP & SDP (RFC 3261 / RFC 4566)** | Session Initiation Protocol over UDP 5060, INVITE, 200 OK, ACK, BYE, Session Description Protocol audio stream negotiation. |
| **Real-time Media**| **RTP & RTCP (RFC 3550)** | Real-time Transport Protocol over UDP, 12-byte fixed header (PT, Seq, Timestamp, SSRC, CSRC), RTCP Sender Report (SR) telemetry. |
| **AI/ML RDMA (L4)** | **RoCEv2 (IBTA Specification)** | RDMA over Converged Ethernet v2 over UDP 4791, 12-byte InfiniBand BTH (`RC_SEND_ONLY`, `RC_RDMA_WRITE`, `RC_ACK`), RETH DMA header, Invariant CRC (ICRC). |
| **NAT Traversal** | **STUN (RFC 8489 / RFC 5389)** | Session Traversal Utilities for NAT over UDP 3478, Magic Cookie `0x2112A442`, Binding Request/Response, XOR-MAPPED-ADDRESS reflexive IP resolution. |
| **NAT Relay** | **TURN (RFC 5766 / RFC 8656)** | Traversal Using Relays around NAT over UDP 3478, Allocate Request/Response, XOR-RELAYED-ADDRESS, Send/Data Indications. |
| **Directory Service** | **LDAP (RFC 4511)** | Lightweight Directory Access Protocol over TCP 389, ASN.1 / BER encoding, BindRequest, SearchRequest, SearchResultEntry. |
| **Administrative AAA**| **TACACS+ (RFC 8907)** | Terminal Access Controller Access-Control System Plus over TCP 49, 12-byte header, Authentication START/REPLY flows with privilege levels. |
| **IoT Pub/Sub** | **MQTT (ISO/IEC 20922)** | Message Queuing Telemetry Transport over TCP 1883, variable length integer codec, CONNECT, PUBLISH, SUBSCRIBE, virtual broker. |
| **Constrained REST** | **CoAP (RFC 7252)** | Constrained Application Protocol over UDP 5683, 4-byte binary header, GET/POST, Uri-Path option, 2.05 Content response. |
| **Multi-Streaming (L4)**| **SCTP (RFC 4960)** | Stream Control Transmission Protocol over IP Protocol 132, 12-byte header, Verification Tag, Adler32 checksum, INIT/DATA chunks. |
| **Logging & Telemetry**| **Syslog (RFC 5424 / RFC 3164)** | UDP Port 514 event logging, Facility/Severity PRI calculation, RFC 5424 structured framing, in-memory event collector. |
| **Integrity** | **RFC 1071 Checksum** | 16-bit one's complement sum algorithm, IPv4 header checksums, ICMP checksums, IPv6 transport pseudo-headers, and TCP/UDP checksums. |
| **Transport (L4)** | **UDP (RFC 768)** | Datagram parser/builder, IPv4/IPv6 pseudo-header checksum, dynamic `UdpSocketTable` port multiplexer. |
| **Transport (L4)** | **TCP (RFC 793, 9293)** | TCP Options (MSS, Window Scale), out-of-order segment reassembly queue, full finite-state machine (`LISTEN`, `SYN_SENT`, `SYN_RECEIVED`, `ESTABLISHED`, `CLOSE_WAIT`, `LAST_ACK`), 3-way handshake, sequence & ACK tracking, stream payload buffering, and connection teardown. |
| **Next-Gen Transport** | **QUIC (RFC 9000)** | 2-bit prefix Variable-Length Integer (VINT) codec, Long Headers (Initial, Handshake, 0-RTT, Retry), Short Headers (1-RTT) with Connection IDs. |
| **Flow & Congestion** | **TCP Congestion Control (RFC 5681)** | Slow Start, Congestion Avoidance, Fast Retransmit / Fast Recovery on 3 dup ACKs, sliding window flow control. |
| **RTT Estimation** | **Jacobson's Algorithm (RFC 6298)** | Smooth RTT (`SRTT`), Round-Trip Variation (`RTTVAR`), and dynamic Retransmission Timeout (`RTO`). |
| **Security Layer** | **TLS 1.3 Framing (RFC 8446)** | 5-byte TLS record framing (Handshake, Alert, ApplicationData), `ClientHello` with SNI, `ServerHello` cipher negotiation (`TLS_AES_128_GCM_SHA256`). |
| **Network Translation** | **NAT / NAPT / Masquerade** | SNAT (dynamic port address translation for private LANs), DNAT (port forwarding), connection tracking (`conntrack`), header/checksum rewrites. |
| **Security & Filter** | **Stateful Firewall (iptables style)** | Rule chains (`INPUT`, `OUTPUT`, `FORWARD`), CIDR subnet matching, protocol matching, port ranges, `ACCEPT` / `DROP` / `REJECT` actions. |
| **Quality of Service** | **QoS & Traffic Shaper (RFC 2697)** | Token Bucket algorithm for egress rate limiting / traffic shaping and Strict Priority Queueing (SPQ) scheduler. |
| **AAA Authentication** | **RADIUS (RFC 2865 / 2866)** | UDP Port 1812 Authentication, Access-Request, Access-Accept, Access-Reject, 16-byte Authenticator, User-Password obfuscation, Framed-IP-Address AVPs. |
| **Device Drivers** | **`NetDevice` Abstraction** | Generic trait powering `LoopbackDevice`, `PcapDevice`, and `VirtualTapDevice`. |
| **Application (L7)** | **HTTP/1.1, HTTP/2 & HTTP/3** | HTTP/1.1 GET client, HTTP/2 (RFC 7540) binary framing, and HTTP/3 over QUIC (RFC 9114 / QPACK RFC 9204) binary framing (`DATA`, `HEADERS`, `SETTINGS`). |
| **Application (L7)** | **WebSocket Protocol (RFC 6455)** | Binary framing, FIN bit, Opcodes (Text, Binary, Ping, Pong, Close), 4-byte XOR payload masking / unmasking. |
| **Application (L7)** | **TFTP File Transfer (RFC 1350)** | UDP Port 69 lock-step reliable file transfer with 512-byte blocks, RRQ/WRQ/DATA/ACK/ERROR states. |
| **Application (L7)** | **SNMPv2c Network Telemetry (RFC 1901/3416)** | ASN.1 / BER TLV codec, `INTEGER`, `OCTET STRING`, `OID`, `SEQUENCE`, GetRequest/Response framing, MIB-II instrumentation. |
| **Application (L7)** | **DNS (RFC 1035)** | DNS query encoding and decoding, Question/Answer sections, pointer compression, and Type-A IPv4 resolution over UDP 53. |
| **Application (L7)** | **DHCP (RFC 2131)** | DHCP 4-way DORA handshake (Discover $\rightarrow$ Offer $\rightarrow$ Request $\rightarrow$ ACK) with TLV Options over UDP 67/68. |
| **Application (L7)** | **DHCPv6 (RFC 8415)** | IPv6 Solicit $\rightarrow$ Advertise $\rightarrow$ Request $\rightarrow$ Reply handshake, DUID-LLT, IA_NA, IAADDR, DNS options over UDP 546/547. |
| **Routing** | **Routing Table (LPM)** | Longest Prefix Match (LPM) route lookup engine, CIDR netmask matching, default gateway and on-link subnet resolution. |
| **Simulation** | **Virtual Network Bus** | Multi-node switched LAN simulator connecting multiple `NetStack` hosts over virtual Ethernet links. |
| **Interface** | **Interactive Network Shell** | Interactive CLI REPL with 74 commands including `flowspec`, `otlp`, `gre6`, `ioam`, `netconf`, `lisp`, `wireguard`, `gptp`, `pcep`, `rsvp`, `openflow`, `diameter`, `nsh`, `sflow`, `6in4`, `4in6`, `roce`, `pfc`, `gue`, `evpn`, `dhcpv6`, `vxlan-gpe`, `vtp`, `ldp`, `glbp`, `tacacs`, `turn`, `gtp`, `hsrp`, `cdp`, `srv6`, `stun`, `rtp`, `ptp`, `erspan`, `mqtt`, `coap`, `sctp`, `ldap`, `netflow`, `sip`, `bfd`, `geneve`, `isis`, `syslog`, `l2tp`, `pim`, `radius`, `pppoe`, `eigrp`, `ping`, `ping6`, `traceroute`, `ntp`, `tftp`, `snmp`, `ospf`, `ipsec`, `http3`, `lacp`, `stp`, `vxlan`, `mpls`, `bgp`, `lldp`, `quic`, `vrrp`, `ndp`, `rip`, `tunnel`, `igmp`, `tls`, `http2`, `ws`, `dns`, `curl`, `udp send`, `arp`, `route`, `netstat`, `iptables`, `nat`, `tcp-stats`, and live PCAP recording. |

---

## 🏗️ Project Architecture

```
TCP-IP Stack/
├── Cargo.toml
├── src/
│   ├── main.rs            # Multi-mode CLI tool (demo, shell, inspect, replay)
│   ├── lib.rs             # Protocol library root
│   ├── pcap.rs            # Raw Libpcap file reader and writer
│   ├── checksum.rs        # RFC 1071 16-bit one's complement checksum engine
│   ├── ethernet.rs        # Layer 2: Ethernet II frame & MAC address
│   ├── vlan.rs            # Layer 2: IEEE 802.1Q VLAN Tagging & Untagging
│   ├── stp.rs             # Layer 2: IEEE 802.1D Spanning Tree Protocol (STP)
│   ├── lacp.rs            # Layer 2: IEEE 802.1AX Link Aggregation Control Protocol
│   ├── isis.rs            # Layer 2: IS-IS Dynamic Routing Protocol (ISO 10589 / RFC 1195)
│   ├── pppoe.rs           # Layer 2: Point-to-Point Protocol over Ethernet (RFC 2516)
│   ├── l2tp.rs            # Layer 2: L2TPv3 Layer 2 Pseudowire Tunneling (RFC 3931)
│   ├── lldp.rs            # Layer 2: IEEE 802.1AB LLDP Neighbor Discovery
│   ├── cdp.rs             # Layer 2: Cisco Discovery Protocol v2 (CDPv2)
│   ├── vtp.rs             # Layer 2: Cisco VLAN Trunking Protocol (VTP)
│   ├── gptp.rs            # Precision Time: IEEE 802.1AS Generalized PTP / TSN (EtherType 0x88F7)
│   ├── openflow.rs        # SDN Datapath: OpenFlow v1.3 Switch Protocol (ONF TS-025)
│   ├── roce.rs            # AI/ML Transport: RoCEv2 (UDP 4791) & IEEE 802.1Qbb PFC
│   ├── erspan.rs          # Layer 2/3: Cisco ERSPAN Port Mirroring & NVGRE (RFC 7637)
│   ├── ptp.rs             # Precision Time: IEEE 1588v2 Nanosecond Clock Sync
│   ├── ioam.rs            # In-Band Telemetry: In-situ OAM Data Plane Telemetry (RFC 9197)
│   ├── flowspec.rs        # Carrier DDoS: BGP Flowspec Traffic Filtering (RFC 5575 / 8955)
│   ├── otlp.rs            # Observability: OpenTelemetry OTLP Exporter (Ports 4317/4318)
│   ├── gre_v6.rs          # Layer 3: GRE-over-IPv6 Multi-Protocol Tunneling (RFC 7676)
│   ├── vxlan.rs           # Layer 2/4: VXLAN 24-bit VNI Overlay Encapsulation (RFC 7348)
│   ├── vxlan_gpe.rs       # Layer 2/3/4: VXLAN Generic Protocol Extension (UDP 4790)
│   ├── gue.rs             # Cloud Encap: Generic UDP Encapsulation (RFC 7763) & FOU (RFC 8086)
│   ├── geneve.rs          # Layer 2/4: Geneve Cloud Virtualization Overlay (RFC 8926)
│   ├── gtp.rs             # 4G/5G Cellular: GTP-U User Plane Encapsulation (3GPP TS 29.281)
│   ├── mpls.rs            # Layer 2.5: MPLS Shim Header & LFIB Switching (RFC 3031)
│   ├── ldp.rs             # Layer 2.5: MPLS Label Distribution Protocol (RFC 5036)
│   ├── rsvp.rs            # Layer 3: MPLS-TE RSVP-TE Traffic Engineering (RFC 3209)
│   ├── pcep.rs            # Layer 4/7: Path Computation Element Protocol & SR-MPLS (RFC 5440/8664)
│   ├── wireguard.rs       # Layer 3/4: WireGuard VPN Protocol & Noise IK (UDP 51820)
│   ├── lisp.rs            # Layer 3/4: Locator/ID Separation Protocol Overlay (RFC 9300/9301)
│   ├── arp.rs             # Layer 2.5: Address Resolution Protocol & ARP Table
│   ├── vrrp.rs            # Redundancy: VRRPv3 Virtual Router Redundancy (RFC 5798)
│   ├── hsrp.rs            # Cisco Redundancy: HSRPv1 Hot Standby Router Protocol (RFC 2281)
│   ├── glbp.rs            # Cisco Load Balancing: Gateway Load Balancing Protocol (GLBP)
│   ├── bfd.rs             # Liveness: BFD Fast Path Failure Detection (RFC 5880)
│   ├── ipv4.rs            # Layer 3: IPv4 packet parser, serializer, and IP types
│   ├── ipv6.rs            # Layer 3: IPv6 packet parser, RFC 5952 formatter, pseudo-header
│   ├── srv6.rs            # Layer 3: Segment Routing over IPv6 (RFC 8754 SRH)
│   ├── nsh.rs             # Layer 3: Network Service Header & Service Function Chaining (RFC 8300)
│   ├── transition.rs      # Layer 3: IPv6 Transition Tunnels (6in4 RFC 4213 & 4in6 RFC 2473)
│   ├── ipsec.rs           # Layer 3: IPsec ESP Protocol 50 & Security Association (RFC 4303)
│   ├── diagnostics.rs     # Layer 3: Traceroute & PMTUD (ICMP Type 11 & Type 3 Code 4)
│   ├── pim.rs             # Layer 3: PIM-SM Sparse Mode Multicast Dynamic Routing (RFC 7761)
│   ├── ospf.rs            # Layer 3: OSPFv2 Link-State Dynamic Routing & SPF (RFC 2328)
│   ├── eigrp.rs           # Layer 3: Cisco EIGRP & DUAL Routing Engine (RFC 7868)
│   ├── rip.rs             # Layer 3: Routing Information Protocol v2 (RFC 2453)
│   ├── bgp.rs             # Layer 3/4: BGP-4 wire format, path attributes & TCP stream framer (RFC 4271)
│   ├── bgp_rib.rs         # Layer 3: BGP Adj-RIB-In / Loc-RIB / Adj-RIB-Out, decision process & route policy
│   ├── bgp_router.rs      # Layer 3: BGP-4 speaker - FSM, timers, sockets on port 179, FIB installation
│   ├── bgp_caps.rs        # Layer 3: BGP OPEN capability framework & AFI/SAFI negotiation (RFC 5492)
│   ├── bgp_mp.rs          # Layer 3: MP_REACH_NLRI / MP_UNREACH_NLRI multiprotocol attributes (RFC 4760)
│   ├── bgp_evpn.rs        # Datacenter Fabric: Route Targets, EVPN RIBs & EVPN decision process (RFC 7432)
│   ├── evpn_vtep.rs       # Datacenter Fabric: VTEP, EVPN instances & VXLAN forwarding state (RFC 8365)
│   ├── evpn.rs            # Datacenter Fabric: BGP EVPN Type 2/3 Control Plane (RFC 7432)
│   ├── tunnel.rs          # Layer 3: GRE (RFC 2784) & IP-in-IP (RFC 2003) Tunneling
│   ├── igmp.rs            # Layer 3.5: IGMPv2 (RFC 2236) & Multicast MAC (RFC 1112)
│   ├── fragment.rs        # Layer 3: IPv4 Fragmentation & Reassembly engine
│   ├── icmp.rs            # Layer 3.5: ICMP Echo request / reply handler
│   ├── icmpv6.rs          # Layer 3.5: ICMPv6 Ping6 & NDP Neighbor Discovery
│   ├── udp.rs             # Layer 4: UDP datagram & socket table
│   ├── bgp_prefix_sid.rs  # Carrier / BGP: BGP Prefix-SID Attribute for SR-MPLS & SRv6 (RFC 8669 Path Attr 40)
│   ├── cqf_enhanced.rs    # TSN / Deterministic: IEEE 802.1Qch CQF Ping-Pong Dual Buffer Zero-Jitter Scheduling
│   ├── nrf_oauth.rs       # 5G Core / Security: NRF OAuth 2.0 Access Token Authorization Service (3GPP TS 29.510)
│   ├── evpn_smet.rs       # Carrier / EVPN: Route Type 6 Selective Multicast Ethernet Tag (SMET / RFC 9251)
│   ├── congestion_isolation.rs # TSN / Datacenter: IEEE 802.1Qcz Congestion Isolation & RoCEv2 PFC Victim Mitigation
│   ├── nef_traffic_influence.rs # 5G Core / Edge MEC: Nnef_TrafficInfluence UPF Steering (3GPP TS 29.522)
│   ├── bgp_ls_srv6.rs     # Carrier / BGP-LS: SRv6 BGP-LS Extensions (RFC 9514 Locators & End SIDs)
│   ├── cbs.rs             # TSN / Shaping: IEEE 802.1Qav Credit-Based Shaper (AVB Class A/B Reservation)
│   ├── sba_events.rs      # 5G Core / SBA: Event Exposure Service Namf_EventExposure (3GPP TS 29.518)
│   ├── ats.rs             # TSN / Shaping: IEEE 802.1Qcr Asynchronous Traffic Shaping (ATS / Urgency-Based Scheduler)
│   ├── bgp_epe.rs         # Carrier / BGP: Segment Routing Egress Peer Engineering (RFC 9086/9087 Peer SIDs)
│   ├── gtp_ext.rs         # 5G Core / User Plane: GTP-U PDU Session Container & QoS Flow Identifier (TS 38.415)
│   ├── evpn_type3.rs      # Carrier / EVPN: Route Type 3 Inclusive Multicast Ethernet Tag & BUM Flooding (RFC 7432)
│   ├── ptp_tc.rs          # Timing / TSN: PTP Transparent Clock Residence Time & Peer Delay Correction (IEEE 1588)
│   ├── pfcp_5g.rs         # 5G Core / UPF: PFCP N4 Control Interface & PDR/FAR Forwarding (3GPP TS 29.244 / UDP 8805)
│   ├── tsn_cnc.rs         # TSN / CNC: IEEE 802.1Qcc Centralized Network Configuration & TSpec Reservation
│   ├── ptp_telecom.rs     # Timing / Cellular: PTP Telecom Profile ITU-T G.8275.1/G.8275.2 (T-GM/T-BC/T-TSC BMCA)
│   ├── ngap_5g.rs         # 5G RAN / Core: NGAP N2 Signalling Interface (3GPP TS 38.413 / SCTP 38412)
│   ├── tas.rs             # TSN / Scheduling: IEEE 802.1Qbv Time-Aware Shaper & GCL (Scheduled Traffic)
│   ├── sba_5g.rs          # 5G Core: Service-Based Architecture (SBA) HTTP/2 REST Dispatcher (3GPP TS 29.500)
│   ├── evpn_type5.rs      # Carrier / EVPN: Route Type 5 IP Prefix Route with Overlay Index (RFC 9136)
│   ├── preemption.rs      # TSN / L2: IEEE 802.1Qbu Frame Preemption & Express Interleaving (eMAC/pMAC)
│   ├── bgp_ext_comm.rs    # Carrier / BGP: Extended Communities, Color Steering & Tunnel Encap (RFC 4360/9012)
│   ├── sai.rs             # SDN / SONiC: OpenCompute Switch Abstraction Interface Hardware Tables
│   ├── psfp.rs            # TSN / Policing: IEEE 802.1Qci Per-Stream Filtering & Policing (GCL / Token Meter)
│   ├── p4runtime.rs       # SDN / P4: P4Runtime Match-Action Table Programming & Packet-IO (Port 9559)
│   ├── evpn_type1.rs      # Carrier / EVPN: Route Type 1 Ethernet A-D Aliasing & Mass Withdrawal (RFC 7432)
│   ├── cqf.rs             # TSN / Queuing: IEEE 802.1Qch Cyclic Queuing & Forwarding (Deterministic Latency)
│   ├── gribi.rs           # SDN / FIB: gRPC Routing Information Base Interface AFT Injection (Port 9340)
│   ├── evpn_multihoming.rs# Carrier / EVPN: Type 4 Ethernet Segment Route & DF Election (RFC 7432)
│   ├── frer.rs            # TSN / Reliability: IEEE 802.1CB Frame Replication & Elimination (R-TAG 0xF1C1)
│   ├── gnoi.rs            # Operations: gRPC Network Operations Interface (gNOI Port 9339)
│   ├── evpn_l3irb.rs      # Carrier / Overlays: EVPN Layer 3 VXLAN Symmetric IRB (RFC 9135 / Type 5)
│   ├── etag.rs            # Layer 2: IEEE 802.1BR Bridge Port Extension & E-TAG (EtherType 0x893F)
│   ├── gnmi.rs            # Telemetry: gRPC Network Management Interface (gNMI / OpenConfig Port 9339)
│   ├── sr_policy.rs       # Carrier: Segment Routing Policy Traffic Steering (RFC 9256 / RFC 9012)
│   ├── cfm.rs             # Carrier: Ethernet OAM IEEE 802.1ag / ITU-T Y.1731 (EtherType 0x8902)
│   ├── sbfd.rs            # Liveness: Seamless BFD Stateless Reflector & Initiator (RFC 7880 / UDP 7784)
│   ├── optical_dom.rs     # Telemetry: Digital Optical Monitoring SFF-8472 Transceiver Diagnostics
│   ├── flex_algo.rs       # Carrier: Segment Routing Flexible Algorithms (SR-Flex-Algo - RFC 9350)
│   ├── geneve_int.rs      # Telemetry: Geneve In-Band Network Telemetry (INT - RFC 8926 / P4)
│   ├── vpls.rs            # Layer 2.5: Virtual Private LAN Service & EoMPLS Pseudowire (RFC 4762)
│   ├── srv6_usid.rs       # IPv6: SRv6 Micro-SID (uSID) Shift-and-Forward Compression (IETF)
│   ├── netflow_v5.rs      # Telemetry: Cisco NetFlow v5 Datacenter Flow Exporter (UDP 2055)
│   ├── ti_lfa.rs          # Carrier: Topology-Independent Loop-Free Alternate & SR-FRR (RFC 4090)
│   ├── mld.rs             # Multicast: Multicast Listener Discovery v2 (MLDv2 - RFC 3810 / RFC 3569 SSM)
│   ├── bfd_v6.rs          # Liveness: Multi-Hop & IPv6 Bidirectional Forwarding Detection (RFC 5881/5883)
│   ├── geneve_sfc.rs      # Overlays: Geneve Service Function Chaining In-Band Metadata (RFC 8926 / RFC 8300)
│   ├── bgp_ls.rs          # Carrier / SDN: BGP Link-State Topology & TE Distribution (RFC 7752 / RFC 9552)
│   ├── ipfix.rs           # Telemetry: IP Flow Information Export / NetFlow v10 (RFC 7011 / RFC 7012)
│   ├── srv6_mup.rs        # 5G Core: SRv6 Mobile User Plane & UPF Interworking (End.M.GTP4.E/D)
│   ├── mpls_oam.rs        # OAM: MPLS LSP Ping & Traceroute Protocol (RFC 4379 / RFC 8029)
│   ├── srv6_ops.rs        # IPv6: SRv6 Network Programming & Endpoint Functions (RFC 8986)
│   ├── gre_udp.rs         # Overlays: GRE-in-UDP Multipath ECMP Tunneling (RFC 8086)
│   ├── twamp.rs           # OAM: Two-Way Active Measurement Protocol (RFC 5357 / RFC 4656)
│   ├── geneve_opts.rs     # Overlays: Geneve Extended Metadata & Dynamic In-Band TLVs (RFC 8926)
│   ├── gre_demux.rs       # Overlays: GRE RFC 2890 Key/Sequence Demuxer & Anti-Replay
│   ├── gre_v6.rs          # Overlays: GRE-over-IPv6 Multi-Protocol Tunneling (RFC 7676)
│   ├── flowspec.rs        # Carrier: BGP Flowspec Automated DDoS Mitigation (RFC 5575 / RFC 8955)
│   ├── otlp.rs            # Telemetry: OpenTelemetry OTLP Metrics & Spans Exporter (Ports 4317/4318)
│   ├── ioam.rs            # Telemetry: In-situ OAM In-Band Telemetry Recording (RFC 9197)
│   ├── netconf.rs         # Layer 7: NETCONF XML-RPC Network Configuration (RFC 6241)
│   ├── lisp.rs            # Overlays: Locator/ID Separation Protocol (RFC 9300 / RFC 9301)
│   ├── wireguard.rs       # Security: WireGuard Modern VPN Protocol (Noise IK Handshake)
│   ├── gptp.rs            # Timing: IEEE 802.1AS Generalized PTP / TSN (EtherType 0x88F7)
│   ├── pcep.rs            # Carrier: Path Computation Element Protocol / SR-MPLS (RFC 5440)
│   ├── rsvp.rs            # Carrier: MPLS-TE RSVP-TE Explicit Path Signaling (RFC 3209)
│   ├── openflow.rs        # SDN: OpenFlow v1.3 Switch Controller & Datapath (ONF TS-025)
│   ├── diameter.rs        # Carrier: 4G/5G Diameter Base AAA Protocol (RFC 6733)
│   ├── nsh.rs             # Overlays: Network Service Header & Service Function Chaining (RFC 8300)
│   ├── sflow.rs           # Telemetry: sflow v5 Sampled Network Telemetry (RFC 3176)
│   ├── transition.rs      # IPv6: Dual-Stack 6in4 (RFC 4213) & 4in6 (RFC 2473) Tunnels
│   ├── roce.rs            # RDMA: RoCEv2 InfiniBand & IEEE 802.1Qbb Lossless PFC
│   ├── gue.rs             # Overlays: Generic UDP Encapsulation (RFC 7763) & FOU
│   ├── evpn.rs            # Carrier: BGP EVPN MAC/IP Control Plane (RFC 7432 / RFC 8365)
│   ├── vxlan_gpe.rs       # Overlays: VXLAN Generic Protocol Extension (UDP 4790)
│   ├── vtp.rs             # Layer 2: Cisco VLAN Trunking Protocol (VTP)
│   ├── ldp.rs             # Layer 2.5: MPLS Label Distribution Protocol (RFC 5036)
│   ├── glbp.rs            # Redundancy: Cisco Gateway Load Balancing Protocol (GLBP)
│   ├── tacacs.rs          # Security: TACACS+ Administrative AAA Access Control (RFC 8907)
│   ├── cdp.rs             # Layer 2: Cisco Discovery Protocol (CDPv2 SNAP)
│   ├── srv6.rs            # IPv6: Segment Routing over IPv6 (RFC 8754 SRH)
│   ├── stun.rs            # Layer 7: STUN NAT Traversal Protocol (RFC 8489)
│   ├── turn.rs            # Layer 7: TURN NAT Relaying Protocol (RFC 5766)
│   ├── gtp.rs             # Cellular: 3GPP GTP-U 4G/5G Mobile User Plane (UDP 2152)
│   ├── hsrp.rs            # Redundancy: Cisco Hot Standby Router Protocol (RFC 2281)
│   ├── rtp.rs             # Layer 4/7: Real-time Transport Protocol & RTCP (RFC 3550)
│   ├── ptp.rs             # Timing: IEEE 1588v2 Precision Time Protocol
│   ├── erspan.rs          # Mirroring: Cisco ERSPAN Type II/III & NVGRE
│   ├── mqtt.rs            # Layer 7: MQTT IoT Publish/Subscribe Protocol (ISO 20922)
│   ├── coap.rs            # Layer 7: CoAP Constrained RESTful Protocol (RFC 7252)
│   ├── sctp.rs            # Layer 4: Stream Control Transmission Protocol (RFC 4960)
│   ├── ldap.rs            # Layer 7: Lightweight Directory Access Protocol (RFC 4511)
│   ├── netflow.rs         # Layer 7: NetFlow v9 & IPFIX Flow Telemetry (RFC 3954)
│   ├── sip.rs             # Layer 7: Session Initiation Protocol & SDP (RFC 3261)
│   ├── bfd.rs             # Layer 3: Bidirectional Forwarding Detection (RFC 5880)
│   ├── geneve.rs          # Overlays: RFC 8926 Geneve Cloud Virtualization Overlay
│   ├── isis.rs            # Layer 2: IS-IS Dynamic Link-State Routing (RFC 1195)
│   ├── syslog.rs          # Layer 7: Syslog Protocol & Event Telemetry (RFC 5424)
│   ├── l2tp.rs            # Layer 2: L2TPv3 Ethernet Pseudowire (RFC 3931)
│   ├── pim.rs             # Layer 3: PIM-SM Multicast Routing (RFC 7761)
│   ├── radius.rs          # Layer 7: RADIUS AAA Protocol (RFC 2865)
│   ├── pppoe.rs           # Layer 2: PPPoE Discovery & Session (RFC 2516)
│   ├── eigrp.rs           # Layer 3: Cisco EIGRP Dynamic Routing (RFC 7868)
│   ├── ospf.rs            # Layer 3: OSPFv2 Dynamic Link-State Routing (RFC 2328)
│   ├── ipsec.rs           # Layer 3: IPsec ESP Tunnel Mode (RFC 4303)
│   ├── http3.rs           # Layer 7: HTTP/3 Binary Framing & QPACK (RFC 9114)
│   ├── lacp.rs            # Layer 2: IEEE 802.1AX Link Aggregation Control Protocol
│   ├── mpls.rs            # Layer 2.5: Multi-Protocol Label Switching (RFC 3031)
│   ├── bgp.rs             # Layer 3: Border Gateway Protocol 4 (RFC 4271)
│   ├── lldp.rs            # Layer 2: IEEE 802.1AB LLDP Discovery
│   ├── stp.rs             # Layer 2: IEEE 802.1D Spanning Tree Protocol
│   ├── vxlan.rs           # Overlays: RFC 7348 VXLAN Cloud Overlay
│   ├── ntp.rs             # Layer 7: Network Time Protocol v4 (RFC 5905)
│   ├── tftp.rs            # Layer 7: Trivial File Transfer Protocol (RFC 1350)
│   ├── snmp.rs            # Layer 7: SNMPv2c BER TLV & MIB Instrumentation
│   ├── quic.rs            # Layer 4: QUIC Binary Framing & VINT (RFC 9000)
│   ├── vrrp.rs            # Layer 3: Virtual Router Redundancy Protocol (RFC 5798)
│   ├── tunnel.rs          # Layer 3: GRE (RFC 2784) & IP-in-IP (RFC 2003)
│   ├── igmp.rs            # Layer 3: IGMPv2 Multicast Group Management (RFC 2236)
│   ├── rip.rs             # Layer 3: RIPv2 Distance-Vector Routing (RFC 2453)
│   ├── socket.rs          # Layer 4/Application: Socket Runtime, UDP Sockets, TCP Listeners & Streams
│   ├── tcp.rs             # Layer 4: TCP Full State Machine, Retransmission & Congestion Control
│   ├── tcp_seq.rs         # Layer 4: RFC 1982 Serial Number Arithmetic & Wraparound Comparisons
│   ├── udp.rs             # Layer 4: UDP Datagram Sockets & Multi-Port Dispatch
│   ├── icmp.rs            # Layer 3: ICMP Echo Request & Reply (Ping)
│   ├── icmpv6.rs          # Layer 3: ICMPv6 & NDP Neighbor Discovery
│   ├── ipv4.rs            # Layer 3: IPv4 Packet Framing & RFC 1071 Checksum
│   ├── ipv6.rs            # Layer 3: IPv6 Packet Framing & RFC 5952 Formatting
│   ├── arp.rs             # Layer 2.5: Address Resolution Protocol & Cache Table
│   ├── vlan.rs            # Layer 2: IEEE 802.1Q VLAN Tagging & Sub-Interfaces
│   ├── ethernet.rs        # Layer 2: Ethernet II Frame & MAC Addressing
│   ├── checksum.rs        # RFC 1071 16-bit One's Complement Checksum Engine
│   └── pcap.rs            # PCAP File Format Global Header & Packet Records
├── tests/
│   ├── common/mod.rs          # Shared socket-API test harness (no hand-built packets)
│   ├── test_socket_runtime.rs # Socket Runtime, UDP ephemeral ports, TCP listen & accept queues
│   ├── test_tcp_reliability.rs# MSS segmentation, Fast Retransmit, Flow Control & Wraparound
│   ├── test_tcp_loss.rs       # Lossy Network Torture (Lost SYN/SYN-ACK/Data/FIN), HTTP/1.1 & PCAP
│   ├── test_bgp_prefix_sid.rs # BGP Prefix-SID Attribute (RFC 8669) tests
│   ├── test_cqf_enhanced.rs   # IEEE 802.1Qch CQF Ping-Pong Dual Buffer tests
│   ├── test_nrf_oauth.rs      # 5G Core NRF OAuth 2.0 Access Token Authorization tests
│   ├── test_evpn_smet.rs      # EVPN Route Type 6 SMET & Selective Multicast tests
│   ├── test_congestion_isolation.rs # IEEE 802.1Qcz Congestion Isolation tests
│   ├── test_nef_traffic_influence.rs# 5G Core NEF Traffic Influence & Edge MEC tests
│   ├── test_bgp_ls_srv6.rs# BGP-LS Extensions for SRv6 (RFC 9514) tests
│   ├── test_cbs.rs        # IEEE 802.1Qav Credit-Based Shaper (CBS) tests
│   ├── test_sba_events.rs # 5G SBA Event Exposure Service tests
│   ├── test_ats.rs        # IEEE 802.1Qcr Asynchronous Traffic Shaping (ATS) tests
│   ├── test_bgp_epe.rs    # BGP Segment Routing Egress Peer Engineering (EPE) tests
│   ├── test_gtp_ext.rs    # 5G GTP-U Extension Headers & PDU Session Container tests
│   ├── test_evpn_type3.rs # EVPN Route Type 3 Inclusive Multicast (IMET) tests
│   ├── test_ptp_tc.rs     # PTP Transparent Clock (TC) Residence Time tests
│   ├── test_pfcp_5g.rs    # 5G N4 PFCP Protocol & PDR/FAR Forwarding tests
│   ├── test_tsn_cnc.rs    # IEEE 802.1Qcc Centralized Network Configuration (CNC) tests
│   ├── test_ptp_telecom.rs# PTP Telecom Profile ITU-T G.8275.1/G.8275.2 BMCA tests
│   ├── test_ngap_5g.rs    # 5G N2 / NGAP Signalling Protocol tests
│   ├── test_tas.rs        # IEEE 802.1Qbv Time-Aware Shaper (TAS) tests
│   ├── test_sba_5g.rs     # 5G Core Service Based Architecture (SBA) tests
│   ├── test_evpn_type5.rs # EVPN Route Type 5 IP Prefix tests
│   ├── test_preemption.rs # IEEE 802.1Qbu Frame Preemption (FPE) tests
│   ├── test_bgp_ext_comm.rs # BGP Extended Communities & Color tests
│   ├── test_sai.rs        # OpenCompute SAI Hardware Abstraction tests
│   ├── test_psfp.rs       # IEEE 802.1Qci PSFP Policing & Gate Filtering tests
│   ├── test_p4runtime.rs  # P4Runtime Match-Action & Packet-IO tests
│   ├── test_evpn_type1.rs # EVPN Type 1 Ethernet A-D Aliasing & Mass Withdrawal tests
│   ├── test_cqf.rs        # IEEE 802.1Qch Cyclic Queuing & Forwarding (CQF) tests
│   ├── test_gribi.rs      # gRPC Routing Information Base (gRIBI) tests
│   ├── test_evpn_multihoming.rs # EVPN Type 4 Multi-Homing & DF Election tests
│   ├── test_frer.rs       # IEEE 802.1CB Frame Replication & Elimination (FRER) tests
│   ├── test_gnoi.rs       # gRPC Network Operations Interface (gNOI) tests
│   ├── test_evpn_l3irb.rs # EVPN Symmetric L3 IRB (RFC 9135) tests
│   ├── test_etag.rs       # IEEE 802.1BR Bridge Port Extension (E-TAG) tests
│   ├── test_gnmi.rs       # OpenConfig gNMI Streaming Telemetry tests
│   ├── test_sr_policy.rs  # Segment Routing Policy (RFC 9256) tests
│   ├── test_cfm.rs        # Carrier Ethernet CFM (IEEE 802.1ag / Y.1731) tests
│   ├── test_sbfd.rs       # Seamless BFD (RFC 7880 / 7881) tests
│   ├── test_optical_dom.rs# Digital Optical Monitoring SFF-8472 tests
│   ├── test_flex_algo.rs  # SR-Flex-Algo Constrained SPF tests
│   ├── test_geneve_int.rs # Geneve In-Band Telemetry (INT) tests
│   ├── test_vpls.rs       # VPLS & EoMPLS Pseudowire tests
│   ├── test_srv6_usid.rs  # SRv6 Micro-SID (uSID) & Compression tests
│   ├── test_netflow_v5.rs # Cisco NetFlow v5 Datacenter Flow tests
│   ├── test_ti_lfa.rs     # TI-LFA Fast Reroute & Backup Segment tests
│   ├── test_mld.rs        # MLDv2 & SSM Channel Subscription tests
│   ├── test_bfd_v6.rs     # Multi-Hop & IPv6 BFD (RFC 5881/5883) tests
│   ├── test_geneve_sfc.rs # Geneve Service Function Chaining tests
│   ├── test_bgp_ls.rs     # BGP-LS Link-State & TE Distribution tests
│   ├── test_ipfix.rs      # IPFIX / NetFlow v10 Flow Export tests
│   ├── test_srv6_mup.rs   # SRv6 Mobile User Plane 5G UPF tests
│   ├── test_mpls_oam.rs   # MPLS LSP Ping & Traceroute (RFC 4379/8029) tests
│   ├── test_srv6_ops.rs   # SRv6 Endpoint Behaviors (RFC 8986) tests
│   ├── test_gre_udp.rs    # GRE-in-UDP Encapsulation (RFC 8086) tests
│   ├── test_twamp.rs      # TWAMP Active Delay Measurement tests
│   ├── test_geneve_opts.rs# Geneve Dynamic TLV Options tests
│   ├── test_gre_demux.rs  # GRE RFC 2890 Key Demuxer & Anti-Replay tests
│   ├── test_pcap.rs       # PCAP reader/writer tests
│   ├── test_ethernet.rs   # Ethernet II frame tests
│   ├── test_vlan.rs       # IEEE 802.1Q VLAN tests
│   ├── test_stp.rs        # IEEE 802.1D Spanning Tree Protocol tests
│   ├── test_lacp.rs       # IEEE 802.1AX LACP bonding tests
│   ├── test_isis.rs       # IS-IS dynamic routing tests
│   ├── test_pppoe.rs      # PPPoE Discovery & Session tests
│   ├── test_l2tp.rs       # L2TPv3 Layer 2 Pseudowire tests
│   ├── test_lldp.rs       # IEEE 802.1AB LLDP tests
│   ├── test_cdp.rs        # Cisco Discovery Protocol (CDP) tests
│   ├── test_vtp.rs        # Cisco VLAN Trunking Protocol (VTP) tests
│   ├── test_gptp.rs       # IEEE 802.1AS gPTP / TSN tests
│   ├── test_wireguard.rs  # WireGuard VPN Protocol tests
│   ├── test_pcep.rs       # PCEP and SR-MPLS Path Computation tests
│   ├── test_ioam.rs       # IOAM In-Band Telemetry tests
│   ├── test_flowspec.rs   # BGP Flowspec DDoS Mitigation tests
│   ├── test_otlp.rs       # OpenTelemetry OTLP Exporter tests
│   ├── test_gre_v6.rs     # GRE-over-IPv6 Tunnel tests
│   ├── test_netconf.rs    # NETCONF XML-RPC Network Configuration tests
│   ├── test_lisp.rs       # LISP Overlay Routing & Mapping tests
│   ├── test_openflow.rs   # OpenFlow 1.3 SDN switch tests
│   ├── test_diameter.rs   # Diameter Base AAA Protocol tests
│   ├── test_roce.rs       # RoCEv2 & IEEE 802.1Qbb PFC tests
│   ├── test_gue.rs        # Generic UDP Encapsulation & FOU tests
│   ├── test_evpn.rs       # BGP EVPN control-plane tests
│   ├── test_nsh.rs        # Network Service Header (NSH) tests
│   ├── test_sflow.rs      # sFlow v5 network telemetry tests
│   ├── test_transition.rs # 6in4 & 4in6 transition tunnel tests
│   ├── test_vxlan.rs      # RFC 7348 VXLAN overlay tests
│   ├── test_vxlan_gpe.rs  # VXLAN Generic Protocol Extension tests
│   ├── test_geneve.rs     # RFC 8926 Geneve cloud overlay tests
│   ├── test_gtp.rs        # 3GPP GTP-U 4G/5G mobile core tests
│   ├── test_mpls.rs       # MPLS shim header & LFIB tests
│   ├── test_ldp.rs        # MPLS LDP label distribution tests
│   ├── test_arp.rs        # ARP request/reply & cache tests
│   ├── test_vrrp.rs       # VRRPv3 router redundancy tests
│   ├── test_hsrp.rs       # Cisco HSRP redundancy tests
│   ├── test_glbp.rs       # Cisco GLBP load balancing tests
│   ├── test_bfd.rs        # BFD fast failure detection tests
│   ├── test_ipv4.rs       # IPv4 parser & checksum tests
│   ├── test_ipv6.rs       # IPv6 parser & RFC 5952 formatting tests
│   ├── test_srv6.rs       # Segment Routing over IPv6 (SRv6) tests
│   ├── test_ipsec.rs      # IPsec ESP & SAD table tests
│   ├── test_diagnostics.rs# Traceroute & PMTUD tests
│   ├── test_pim.rs        # PIM-SM multicast routing tests
│   ├── test_ospf.rs       # OSPFv2 link-state & SPF tests
│   ├── test_eigrp.rs      # EIGRP DUAL metric & topology tests
│   ├── test_rip.rs        # RIPv2 distance-vector routing tests
│   ├── test_bgp.rs        # BGP-4 message codec tests
│   ├── test_bgp_session.rs # BGP-4 FSM, OPEN negotiation, timers & TCP stream framing tests
│   ├── test_bgp_control_plane.rs # BGP-4 UPDATE, RIBs, AS_PATH, withdrawal, FIB & data-plane tests
│   ├── test_bgp_failover.rs # BGP-4 best path, MED, iBGP, route policy & failover tests
│   ├── test_bgp_malformed.rs # BGP-4 hostile and malformed input tests
│   ├── test_bgp_capabilities.rs # OPEN capabilities, AFI/SAFI negotiation & 4-octet ASN tests
│   ├── test_evpn_vxlan.rs   # MP-BGP EVPN → VXLAN acceptance chain, end to end, plus PCAP proof
│   ├── test_evpn_failover.rs # EVPN withdrawal, session loss & MAC mobility tests
│   ├── test_evpn_isolation.rs # Two tenants on one session: Route Target isolation tests
│   ├── test_evpn_malformed.rs # Hostile MP-BGP and EVPN input tests
│   ├── test_tunnel.rs     # GRE and IP-in-IP tunneling tests
│   ├── test_igmp.rs       # IGMPv2 and Multicast MAC mapping tests
│   ├── test_fragment.rs   # IPv4 fragmentation & reassembly tests
│   ├── test_icmp.rs       # ICMP Ping echo tests
│   ├── test_icmpv6.rs     # ICMPv6 Ping6 and NDP tests
│   ├── test_udp.rs        # UDP datagram & pseudo-header checksum tests
│   ├── test_tcp.rs        # TCP 3-way handshake & state machine tests
│   ├── test_sctp.rs       # SCTP association & chunk tests
│   ├── test_rtp.rs        # RTP streaming & RTCP telemetry tests
│   ├── test_stun.rs       # STUN NAT traversal tests
│   ├── test_turn.rs       # TURN NAT relaying tests
│   ├── test_quic.rs       # QUIC binary framing & VINT tests
│   ├── test_congestion.rs # TCP Congestion Control & RTT estimator tests
│   ├── test_tls.rs        # TLS 1.3 Record & Handshake framing tests
│   ├── test_nat.rs        # NAT SNAT/DNAT translation tests
│   ├── test_firewall.rs   # Firewall CIDR and Port filtering tests
│   ├── test_qos.rs        # QoS Token Bucket and Priority Scheduler tests
│   ├── test_device.rs     # NetDevice drivers tests
│   ├── test_mqtt.rs       # MQTT IoT Pub/Sub tests
│   ├── test_coap.rs       # CoAP Constrained REST tests
│   ├── test_ldap.rs       # LDAP ASN.1/BER Directory Service tests
│   ├── test_tacacs.rs     # TACACS+ Administrative AAA Security tests
│   ├── test_diameter.rs   # Diameter Base AAA Protocol tests
│   ├── test_netflow.rs    # NetFlow v9 / IPFIX Flow Telemetry tests
│   ├── test_sip.rs        # SIP VoIP signaling & SDP tests
│   ├── test_syslog.rs     # Syslog Protocol & PRI tests
│   ├── test_radius.rs     # RADIUS Authentication & AVP tests
│   ├── test_http2.rs      # HTTP/2 binary framing & multiplexing tests
│   ├── test_http3.rs      # HTTP/3 binary framing & QPACK tests
│   ├── test_websocket.rs  # WebSocket binary framing & masking tests
│   ├── test_ntp.rs        # NTPv4 protocol & timestamp tests
│   ├── test_tftp.rs       # TFTP file transfer tests
│   ├── test_snmp.rs       # SNMPv2c BER encoding & MIB tests
│   ├── test_dns.rs        # DNS query and response tests
│   ├── test_dhcp.rs       # DHCP DORA handshake tests
│   ├── test_bus.rs        # Virtual network bus multi-node tests
│   ├── test_stack.rs      # End-to-end PCAP pipeline integration tests
│   ├── test_lab_e2e.rs    # Integrated Virtual Network Lab end-to-end data plane tests
│   ├── test_lab_advanced.rs # Advanced Virtual Lab: DHCP DORA, NAT, RIPv2 & TCP OOO reassembly
│   └── test_lab_fabric.rs   # Fabric & Transport: VXLAN Leaf-Spine-Leaf, OSPF Dijkstra SPF, Firewall & MPLS 3-Node LSP
```

---

## 🌐 Integrated Virtual Network Lab & Data Plane Simulation

The project includes an in-process **Deterministic Virtual Network Lab** (`src/lab.rs`) allowing realistic multi-node, multi-subnet networking topologies without external kernel privileges, root access, or physical hardware:

* **VXLAN Leaf-Spine-Leaf Overlay Fabric**: L2 Ethernet frame encapsulation over multi-hop underlay IP fabrics with 24-bit VNI segmentation, transparent tenant host bridging, and outer UDP 4789 encapsulation.
* **OSPFv2 Link-State Topology & Dijkstra SPF Engine**: Multi-router link-state graph building and Dijkstra Shortest Path First (SPF) algorithm to dynamically compute optimal loop-free cost paths across multi-node topologies.
* **Stateful Firewall & Security Policies**: Router data-plane filter evaluating `Input` and `Forward` chains against CIDR ranges, transport protocols (TCP/UDP/ICMP), and port ranges with line-rate `ACCEPT` / `DROP` / `REJECT` actions.
* **MPLS 3-Node Label Switched Path (LSP)**: End-to-end label switching data plane featuring Ingress LER Label PUSH (`0x8847`), Core LSR Label SWAP, and Egress LER Penultimate Hop Popping (PHP) delivering transparent customer transport.
* **Dynamic Network Auto-Configuration**: Full DHCPv4 DORA (Discover $\rightarrow$ Offer $\rightarrow$ Request $\rightarrow$ ACK) engine with dynamic IP pool allocation, lease management, and client stack auto-reconfiguration (`dhcp_discover`, `apply_dhcp_ack`).
* **Network Address Translation (NAPT)**: Multi-interface gateway router SNAT masquerading for outbound LAN traffic and bidirectional DNAT connection tracking (`NatTable`).
* **Dynamic Routing Convergence**: Multi-router distance-vector routing (`RIPv2`) over multicast `224.0.0.9:520` with split horizon, poison reverse, and automatic forwarding information base (FIB) synchronization.
* **Multi-AS BGP-4 Routing**: Routers run a real BGP speaker on TCP port 179 over this stack, establish sessions through a genuine three-way handshake and OPEN negotiation, propagate prefixes across autonomous systems with correct `AS_PATH` prepending, and install the selected paths into the forwarding table that actually carries host traffic.
* **Reliable TCP Under Loss**: Application-driven socket streams over lossy, reordering links — MSS segmentation, RTO and fast retransmit, out-of-order reassembly, congestion and flow control, and the full RFC 9293 lifecycle including `CLOSE_WAIT` and `TIME_WAIT`.
* **Deterministic Fault Injection & Simulated Time**: Per-link packet drops, hold-and-release reordering, MTU limits, and bit-level corruption, advanced by a logical clock (`advance_time`, `run_until`, `pump`) so every scenario is byte-for-byte reproducible.
* **Hardware-like Forwarding Plane**: LPM route table lookup, TTL decrementing & header checksum recalculation, cold ARP resolution queuing, and ICMP Time Exceeded (Type 11 Code 0) generation.
* **Fault Injection Engine**: Configurable link MTU limits, deterministic drop sequences, and bit-level payload corruption to verify strict checksum rejection.
* **Integrated PCAP Tap**: Continuous packet capture on every virtual link, exportable directly to Wireshark-compatible `.pcap` trace files.

---

## 🛣️ BGP-4 Control Plane

BGP is a routing process, not a codec. It runs on top of this repository's own reliable
TCP runtime, and the routes it selects are installed into the same `RoutingTable` the IPv4
forwarding path consults:

```text
BGP Routing Process        (src/bgp_router.rs — FSM, peers, timers, policy)
    ↓  tcp_listen_any(179) · tcp_connect · tcp_write · tcp_read
Socket Runtime             (src/socket.rs)
    ↓
Reliable TCP               (src/tcp.rs — RTO, congestion & flow control)
    ↓
IPv4  →  ARP  →  Ethernet
    ↓
Virtual Network Lab        (src/lab.rs)
    ↓
Remote BGP peer
```

and the route pipeline inside a speaker:

```text
BGP UPDATE → Adj-RIB-In → best-path selection → Loc-RIB → RoutingTable → IPv4 forwarding
```

### Modules

| File | Role |
|---|---|
| `src/bgp.rs` | Wire format: 19-byte framing, OPEN, UPDATE, KEEPALIVE, NOTIFICATION, path attributes, `AsPath`, `Ipv4Prefix`, and `BgpFramer` (TCP stream reassembly). |
| `src/bgp_rib.rs` | `AdjRibIn`, `LocRib`, `AdjRibOut`, `BgpPath`, the decision process, and the prefix policy engine. |
| `src/bgp_router.rs` | `BgpRouter` / `BgpPeer`: the finite state machine, timers, socket I/O, import and export, and FIB synchronisation. |
| `src/router.rs` | `RouteSource` and administrative distance, so BGP routes can be installed and withdrawn without disturbing connected or static entries. |

### Session establishment

One end of each session is configured `Active` and the other `Passive`, which is standard
operational practice and removes connection-collision ambiguity:

```text
TCP SYN → SYN-ACK → ACK  →  BGP OPEN ⇄ BGP OPEN  →  KEEPALIVE ⇄ KEEPALIVE  →  Established
```

An OPEN is validated before it is acted on — version, ASN, BGP identifier, and hold time —
and a structurally invalid one is answered with the NOTIFICATION RFC 4271 prescribes rather
than silently accepted.

### Timers

`ConnectRetryTimer`, `HoldTimer`, and `KeepaliveTimer` all run off the lab's simulated
logical clock. There are no wall-clock sleeps, no threads, and no async runtime, so a
hold-timer expiry is reproducible to the millisecond. The negotiated hold time is the lower
of the two proposals, and the keepalive interval is a third of it.

### Message framing over a byte stream

TCP has no message boundaries. `BgpFramer` buffers whatever arrives and hands back one
complete message at a time, so a message split across several reads, several messages
delivered in one read, and a peer that disappears mid-message are all handled. The buffer is
hard-capped: the marker, length, and type fields are validated as soon as 19 bytes are
present, so a peer cannot make it grow without bound.

### Best-path selection

Deterministic, in this order:

1. locally originated paths
2. highest `LOCAL_PREF`
3. shortest `AS_PATH` (an `AS_SET` counts as one hop)
4. lowest `ORIGIN`
5. lowest `MULTI_EXIT_DISC`, compared **only** between paths from the same neighbouring AS
6. eBGP over iBGP
7. lowest peer BGP identifier, then lowest peer address

The chain ends in comparisons that are unique per peer, so no two paths can tie and the
winner never depends on arrival order.

### Loop prevention and propagation

* Advertising over eBGP prepends the local ASN; advertising over iBGP does not.
* An UPDATE whose `AS_PATH` already contains the local ASN is discarded.
* An UPDATE from an **external** peer must carry a non-empty `AS_PATH` that leads with
  that neighbour's own ASN, or the session is reset with a Malformed AS_PATH
  NOTIFICATION (RFC 4271 sections 6.3 and 9.1.2). The leading-AS half is the check
  vendors call *enforce-first-as* and can be relaxed per peer with
  `set_enforce_first_as`; the non-empty half cannot, because a zero-length `AS_PATH`
  wins the shortest-path step against every legitimate route. Neither rule applies to
  an internal peer, which legitimately relays paths it did not originate.
* A route is never advertised back to the peer it was learned from, nor into an AS already
  on its path, nor from one iBGP peer to another.
* eBGP advertisements rewrite `NEXT_HOP` to the session's own address; iBGP sessions can
  opt into the same behaviour with `set_next_hop_self`.

### Withdrawal and session loss

Withdrawn NLRI is removed from the Adj-RIB-In, the decision process reruns, and the FIB
entry goes with it. Losing a session — hold timer, TCP failure, NOTIFICATION, or an
administrative shutdown — purges everything learned from that peer, reruns best-path
selection, installs an alternate path if one exists, and withdraws downstream. Nothing
stale is left in any RIB or in the forwarding table.

### Route policy

A small ordered prefix policy per peer, in each direction: `permit`, `deny`, `set
local-pref`, and `set med`, matched by exact prefix, prefix-or-longer, or any. First match
wins; unmatched prefixes fall through to the default action.

### Hardening

Network input is never trusted. Length fields are bounds-checked before use, attribute
flags and lengths are validated, unknown well-known attributes are an error while unknown
optional ones are ignored, duplicate attributes are rejected, an `AS_PATH` segment that
overruns its attribute is rejected, and each peer has a prefix limit whose breach closes
the session with a Cease NOTIFICATION.

The send path is held to the same standard. A BGP message is only written when the
transport can take all of it, so one message is never split across two writes and a
retry can never put the same header on the wire twice; an `AS_PATH` segment longer than
the 255 its count octet can express is emitted as several segments rather than being
silently truncated into an unparseable stream. On the receive side, everything a peer
delivered before closing is decoded before end-of-stream is acted on, so a final
NOTIFICATION is reported as the reason a session ended rather than being lost behind
the FIN.

### Shell diagnostics

`bgp <subcommand>` in the interactive shell builds and converges a real three-AS fabric,
then reports what that running control plane actually holds:

```text
netstack > bgp summary      # per-neighbor state, uptime, hold time, prefix counts
netstack > bgp peers        # timers, message counters, discard reasons, last error
netstack > bgp routes       # the Loc-RIB, with FIB status per prefix
netstack > bgp rib          # the Adj-RIB-In, best paths marked
netstack > bgp advertised   # the Adj-RIB-Out, per neighbor
netstack > bgp route        # each router's real IPv4 forwarding table
netstack > bgp events       # the control-plane event log
```

---

## 🧬 MP-BGP EVPN / VXLAN Overlay

The BGP speaker also carries `AFI 25 / SAFI 70`, and what it learns there programs the
VXLAN data plane. A remote MAC becomes forwardable because an EVPN route said so — never
because a tunnel destination was configured, and never because a frame was flooded and the
source address remembered:

```text
local host sends a frame
    ↓  access port learning                 (src/evpn_vtep.rs)
EVPN Type 2 route originated
    ↓  MP_REACH_NLRI, AFI 25 / SAFI 70      (src/bgp_mp.rs)
MP-BGP UPDATE
    ↓  the same TCP session on port 179     (src/bgp_router.rs → src/socket.rs)
remote EVPN Adj-RIB-In
    ↓  Route Target import                  (src/bgp_evpn.rs)
EVPN Loc-RIB
    ↓  (VNI, MAC) → remote VTEP             (src/evpn_vtep.rs)
VXLAN encapsulation on UDP 4789             (src/vxlan.rs)
    ↓
IPv4 underlay → remote VTEP → decapsulation → tenant host
```

### Modules

| File | Role |
|---|---|
| `src/bgp_caps.rs` | RFC 5492 capability framework: `AfiSafi`, Multiprotocol and Four-Octet AS capabilities, and the negotiation that decides what a session may carry. |
| `src/bgp_mp.rs` | RFC 4760 `MP_REACH_NLRI` and `MP_UNREACH_NLRI`. The NLRI payload stays opaque here; what the bytes mean belongs to the family that owns them. |
| `src/bgp_evpn.rs` | `RouteTarget`, `EvpnRouteKey`, `EvpnRoute`, the EVPN RIBs, the EVPN decision process, and the NLRI list codec. |
| `src/evpn_vtep.rs` | `Vtep` and `EvpnInstance`: local MAC learning, origination, and the overlay forwarding table the data plane reads. |
| `src/evpn.rs` | The EVPN NLRI wire format (Route Types 2 and 3) and the Route Distinguisher. |
| `src/vxlan.rs` | VXLAN header and encapsulation on UDP 4789. |

### Capability negotiation

Both OPENs are intersected into a negotiated family set, and that set gates everything
else. A speaker that sent no Multiprotocol capability is a legacy RFC 4271 speaker and
negotiates IPv4 unicast alone, so an ordinary session is unaffected by any of this:

```text
netstack > bgp capabilities
  advertised: Multiprotocol IPv4 Unicast (AFI 1/SAFI 1), Multiprotocol L2VPN EVPN (AFI 25/SAFI 70), Four-Octet AS 65001
  neighbor 10.0.0.2 (Established)
    peer offered : Multiprotocol IPv4 Unicast, Multiprotocol L2VPN EVPN, Four-Octet AS 65002
    negotiated   : IPv4 Unicast (AFI 1/SAFI 1), L2VPN EVPN (AFI 25/SAFI 70)
```

A peer that did not negotiate EVPN is never sent an EVPN route, and one that sends EVPN
NLRI anyway is answered with a NOTIFICATION rather than having its routes quietly stored.

### 4-octet autonomous system numbers

`AsPath` holds `u32`, and the encoding width comes from the negotiation rather than being
fixed. Where a two-octet field cannot hold the value it carries `AS_TRANS` and the true
value travels in the Four-Octet AS capability or `AS4_PATH` (RFC 6793) — never a truncation
to a different real AS. The width is passed into the parser rather than guessed, because
the bytes genuinely do not say: a two-ASN wide segment and a four-ASN narrow one are the
same length and both decode.

### Route Targets and tenant isolation

Import is a filter *on the way in*. A route whose Extended Communities carry no Route
Target this speaker imports never reaches the Loc-RIB, so it cannot program anything:

```text
VNI 5001   RD 10.0.0.1:5001   import RT 65001:5001   export RT 65001:5001
VNI 5002   RD 10.0.0.2:5002   import RT 65001:5002   export RT 65001:5002
```

An instance accepts a route only when an import RT matches **and** the route's VNI is that
instance's VNI. The two conditions are independent — the RT says which tenant asked for the
route, the VNI says which broadcast domain the sender put it in — and requiring both stops
a neighbour using a valid Route Target to inject a MAC into a tenant it has no business
reaching.

### Forwarding

| Destination | What happens |
|---|---|
| A MAC with a Type 2 route | VXLAN unicast to exactly that VTEP. |
| A MAC on another local access port | Bridged locally; nothing is encapsulated. |
| A MAC on the port it arrived from | Dropped: the segment already delivered it. |
| Broadcast, multicast, or an unknown MAC | Ingress replication to the VTEPs that advertised a Type 3 route, and to nobody else. |
| Anything, with no Type 3 route yet | Dropped. There is nowhere to send it. |

Decapsulation deliberately learns nothing from the inner frame. Data-plane learning would
reintroduce the flood-and-learn behaviour EVPN exists to replace, and would install
forwarding state that no withdrawal could remove.

### Withdrawal, session loss, and mobility

Remote forwarding state is a pure function of the EVPN Loc-RIB: the tables are rebuilt on
every programming pass rather than patched. A withdrawn route, a dead session, and a host
that moved are therefore the same operation — the input set changed — and none of them can
leave an entry behind.

MAC mobility is ordered by the sequence number, compared *ahead* of every ordinary BGP
attribute (RFC 7432 section 15). Running the normal tie-break chain first would let a
detail such as a lower peer address pin traffic to the location a host has left. A MAC that
moves more than `MAX_MAC_MOVES` times is declared duplicate and left alone, so two VTEPs
that both genuinely hold it stop bidding the sequence number up against each other.

### The leaf-spine-leaf fabric

`build_evpn_fabric` gives each leaf a loopback VTEP address and peers the leaves loopback
to loopback, so the TCP session carrying the EVPN routes is itself multihop traffic the
spine forwards:

```text
 host_a 192.168.10.11                       host_b 192.168.10.22
 02:00:00:00:0a:0a                          02:00:00:00:0b:0b
        │ tenant1                                   │ tenant2
      leaf1  VTEP 10.0.0.1                        leaf2  VTEP 10.0.0.2
        └── 10.1.0.1/30 ─── spine ─── 10.2.0.2/30 ──┘
                       (IP underlay only)
```

The spine runs no BGP and knows no VNI. Neither leaf is configured with a remote MAC, a
remote VTEP, or a tunnel destination; both tenant hosts sit in one `/24` with no gateway, so
every packet between them has to cross the overlay for that to be true.

### Hardening

MP-BGP nests three length-delimited structures — a path attribute, an MP attribute inside
it, and the EVPN NLRI inside that — so every length is checked against what actually
remains before it is used. `tests/test_evpn_malformed.rs` covers malformed capability
blocks, lying `MP_REACH` and `MP_UNREACH` lengths, EVPN NLRI truncated at every possible
byte, MAC and IP length fields that do not match the body, Extended Communities lists that
are not a multiple of eight bytes, duplicate attributes, unusable next hops, and EVPN NLRI
on a session that never negotiated the family. The EVPN Adj-RIB-In and the per-instance MAC
tables are both bounded.

Two defects in the pre-existing EVPN NLRI parser were fixed in the process: a MAC/IP route
claiming a 32-bit host IP inside a body too short to hold one indexed past the end, and the
MAC and IP length fields were read and then ignored, so a route with a non-48-bit MAC was
decoded as if it had said 48.

### Shell diagnostics

```text
netstack > evpn mac          # (VNI, MAC) → location, sequence number, and how it was learned
netstack > evpn routes       # the EVPN Loc-RIB: type, RD, MAC, host IP, VNI, VTEP, RTs
netstack > evpn adj-rib-in   # every EVPN route received, before best-path selection
netstack > evpn advertised   # the EVPN Adj-RIB-Out, per neighbor
netstack > evpn vni          # one row per instance: RD, import and export RTs, MAC counts
netstack > bgp capabilities  # what each speaker offers and what each session agreed
netstack > bgp evpn summary  # sessions, negotiated families, EVPN route counts
netstack > vxlan vtep        # VTEP source, underlay interface, instances, flood lists
netstack > vxlan vni         # the per-VNI table
```

---

## 🔌 Application Socket Runtime & Reliable TCP

The stack is usable by ordinary applications. An application talks to sockets; the runtime
owns everything below that line, including segmentation, retransmission, and encapsulation:

```text
Application  (HTTP client / HTTP server / any byte-stream app)
    ↓  tcp_listen · tcp_connect · tcp_write · tcp_read · tcp_close
Socket API   (src/socket.rs — socket tables, ports, accept queues)
    ↓
TCP / UDP Runtime  (src/tcp.rs, src/udp.rs — FSM, RTO, congestion & flow control)
    ↓
IPv4  →  ARP  →  Ethernet
    ↓
Deterministic Virtual Network Lab  (src/lab.rs — loss, reordering, MTU, PCAP tap)
```

No application ever builds a TCP segment, IPv4 packet, or Ethernet frame, and no
application touches `TcpConnection` directly.

### Socket API

```rust
// Server
let listener        = stack.tcp_listen(8080)?;
let (stream, peer)  = stack.tcp_accept(listener)?;   // WouldBlock until one arrives

// Client
let stream = stack.tcp_connect(SocketAddrV4 { ip, port: 8080 })?;

// Byte stream — segmentation, retransmission and ordering are the transport's job
stack.tcp_write(stream, &payload)?;   // may report a short write; the buffer is bounded
stack.tcp_read(stream, &mut buf)?;    // Ok(0) == end of stream
stack.tcp_close(stream)?;             // FIN follows any data still queued

// UDP
let sock = stack.udp_bind(0)?;                       // 0 allocates an ephemeral port
stack.udp_send_to(sock, b"hello", remote)?;
let (data, from) = stack.udp_recv_from(sock)?;
```

Several clients may share one listening port; connections are demultiplexed by the TCP
4-tuple. Ports come from the ephemeral range 49152–65535 and are released when a
connection is reclaimed.

### Reliability mechanisms

* **Deterministic timers.** Every timer is driven by a caller-supplied simulated clock
  (`lab.advance_time`, `lab.run_until`, `stack.step_timers`). Nothing consults the wall
  clock, sleeps, or spawns a thread, so every test is reproducible.
* **Real retransmission.** Every sequence-space-consuming transmission — SYN, data, FIN —
  is tracked until acknowledged and resent on RTO expiry, with exponential backoff and a
  retransmission cap that aborts instead of looping forever.
* **RFC 6298 RTO.** Initial RTO of 1 s, SRTT/RTTVAR smoothing (α = 1/8, β = 1/4),
  `RTO = SRTT + 4·RTTVAR` clamped to [200 ms, 60 s]. **Karn's algorithm** discards the
  ambiguous samples that retransmitted segments would otherwise contribute.
* **MSS segmentation.** A single large `tcp_write` is split to the negotiated MSS; the
  receiver reconstructs the original byte stream exactly.
* **Congestion control that actually gates transmission.** Slow start, congestion
  avoidance, and fast recovery decide what may be sent:
  `bytes_in_flight ≤ min(cwnd, rwnd)` is enforced by the send path, not merely recorded.
* **Fast retransmit.** Three duplicate ACKs resend the missing range without waiting for
  the RTO.
* **Flow control.** The advertised receive window shrinks as the receive buffer fills and a
  window update is emitted when it reopens; a persist timer probes a closed window so a
  lost update cannot deadlock the sender.
* **Wraparound-safe sequence arithmetic.** All comparisons go through RFC 1982 serial
  arithmetic (`src/tcp_seq.rs`), verified across `0xFFFF_FFFF` for handshake, data,
  acknowledgement, and out-of-order reassembly.
* **Bounded buffers.** The send buffer, receive buffer, out-of-order reassembly queue,
  listener backlog, and finished-connection history all have explicit caps.
* **Hostile input.** Malformed, truncated, out-of-window, and replayed segments are
  rejected without panicking; resets are validated against the receive window (RFC 5961)
  and ACKs of unsent data are refused.

### Diagnostics

`stack.tcp_diagnostics(stream)` returns a snapshot per connection — state, bytes and
segments sent/received, retransmissions, fast retransmits, timeouts, duplicate ACKs,
`cwnd`, `ssthresh`, `srtt`, `rttvar`, `rto`, bytes in flight, send and receive windows,
unacknowledged segments, and buffer occupancy. All of it is owned by the connection; there
is no global mutable state.

---

## 🚀 Quickstart & Commands

### 1. Run All Tests
```bash
cargo test --all-targets
```

### 2. Run Virtual Lab Integration Suites
```bash
cargo test --test test_lab_e2e
cargo test --test test_lab_advanced
cargo test --test test_lab_fabric
```

### 3. Run the Socket Runtime & TCP Reliability Suites
```bash
cargo test --test test_socket_runtime     # socket API, ports, accept queues, reaping
cargo test --test test_tcp_reliability    # MSS, congestion, fast retransmit, wraparound
cargo test --test test_tcp_loss           # loss/reorder torture, HTTP/1.1, 128 KiB + PCAP
```

### 4. Run the BGP-4 Control Plane Suites
```bash
cargo test --test test_bgp_session        # FSM, OPEN negotiation, timers, stream framing
cargo test --test test_bgp_control_plane  # UPDATE, RIBs, AS_PATH, withdrawal, FIB, data plane
cargo test --test test_bgp_failover       # best path, MED, iBGP rules, policy, failover
cargo test --test test_bgp_malformed      # hostile and malformed input
```

### 5. Run the MP-BGP EVPN / VXLAN Overlay Suites
```bash
cargo test --test test_bgp_capabilities   # OPEN capabilities, AFI/SAFI negotiation, 4-octet ASNs
cargo test --test test_evpn_vxlan         # the acceptance chain, end to end, plus the PCAP proof
cargo test --test test_evpn_failover      # withdrawal, session loss, MAC mobility
cargo test --test test_evpn_isolation     # two tenants, Route Target isolation
cargo test --test test_evpn_malformed     # hostile MP-BGP and EVPN input
```

### 6. Launch the Dual-Stack Interactive Shell (REPL)
```bash
cargo run -- shell
```
Inside the interactive shell:
```text
netstack > lab topology
netstack > lab dhcp
netstack > lab nat
netstack > lab rip
netstack > lab vxlan
netstack > lab ospf
netstack > lab firewall
netstack > lab mpls
netstack > lab ping4 192.168.1.20
netstack > lab ping6 2001:db8:1::20
netstack > lab route4 10.0.2.2 64
netstack > lab route4 10.0.2.2 1
netstack > lab udp-echo "Hello Virtual Network Lab"
netstack > lab tcp-demo
netstack > lab sockets
netstack > lab tcp-reliable
netstack > lab tcp-loss
netstack > lab tcp-reorder
netstack > lab http
netstack > lab tcp-stats
netstack > lab pcap lab_capture.pcap
netstack > bgp summary
netstack > bgp routes
netstack > bgp route
netstack > bgp capabilities
netstack > bgp evpn summary
netstack > evpn mac
netstack > evpn routes
netstack > evpn vni
netstack > vxlan vtep
netstack > vxlan vni
netstack > status
netstack > exit
```
