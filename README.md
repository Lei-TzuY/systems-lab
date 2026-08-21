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
| **Inter-Domain Route**| **BGP-4 (RFC 4271)** | Border Gateway Protocol over TCP port 179, 19-byte framing (Marker `0xFF`*16), OPEN, UPDATE with AS_PATH & NEXT_HOP attributes, BGP RIB table. |
| **BGP EVPN Fabric** | **BGP EVPN (RFC 7432 / RFC 8365)** | BGP L2VPN EVPN (AFI 25, SAFI 70), Route Type 2 (MAC/IP Advertisement) & Route Type 3 (Inclusive Multicast), `EvpnMacTable` control-plane forwarding. |
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
│   ├── bgp.rs             # Layer 3/4: Border Gateway Protocol 4 (RFC 4271)
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
│   ├── tcp.rs             # Layer 4: TCP Full State Machine & Congestion Control
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
│   ├── test_bgp.rs        # BGP-4 inter-domain routing tests
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
│   ├── test_dhcpv6.rs     # DHCPv6 Solicit/Advertise handshake tests
│   ├── test_bus.rs        # Virtual network bus multi-node tests
│   └── test_stack.rs      # End-to-end PCAP pipeline integration tests
```

---

## 🚀 Quickstart & Commands

### 1. Run All Tests (181 Unit & Integration Tests)
```bash
cargo test
```

### 2. Launch the Dual-Stack Interactive Shell (REPL)
```bash
cargo run -- shell
```
Inside the interactive shell:
```text
netstack > status
netstack > flowspec rules
netstack > flowspec drop 192.168.1.100 53
netstack > otlp export
netstack > gre6 encap "Multi-Protocol Overlay Packet over IPv6"
netstack > ioam record "Spine-Leaf IOAM Telemetry Flow"
netstack > netconf get
netstack > lisp lookup 10.1.1.50
netstack > wireguard handshake
netstack > gptp pdelay
netstack > pcep req 10.0.0.4
netstack > rsvp path 192.168.1.10 100
netstack > ofp tables
netstack > diameter cer
netstack > nsh encap 100 255 "Service Chained Flow"
netstack > sflow export
netstack > roce send 202 "GPU Tensor Buffer"
netstack > pfc pause 3
netstack > ping 192.168.1.10
netstack > ping6 2001:db8::10
netstack > exit
```
