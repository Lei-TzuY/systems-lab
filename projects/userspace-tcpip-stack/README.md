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
| **Fast Liveness** | **BFD (RFC 5880 / RFC 5881)** | Bidirectional Forwarding Detection over UDP 3784 (Control) and UDP 3785 (Echo), 24-byte control packet, 3-way handshake (`Down` $\rightarrow$ `Init` $\rightarrow$ `Up`), authentication headers (Simple Password, Keyed MD5, Keyed SHA1), sub-second link failure detection. |
| **Network (L3)** | **IPv4 (RFC 791)** | Header parser/builder (IHL, TTL, Identification, DF/MF flags, Protocol demux), header checksum verification & recalculation. |
| **Network (L3)** | **IPv6 (RFC 8200, 5952)** | 128-bit `Ipv6Address` with canonical zero-compression string formatting, 40-byte fixed header, Next Header dispatching, IPv6 pseudo-header checksum. |
| **IPv6 Extensions** | **IPv6 Ext Headers & Flow Label (RFC 8200 / 6437)** | Chained Extension Headers (Hop-by-Hop, Routing, Fragment, Destination Options, AH, No Next Header), TLV options (Pad1, PadN, Router Alert RFC 2711, Jumbo Payload RFC 2675), 20-bit 5-tuple ECMP Flow Label hashing, a bounded chain length, and the RFC 8200 section 4.1 rule that Hop-by-Hop must immediately follow the IPv6 header. |
| **Segment Routing** | **SRv6 (RFC 8754 / RFC 8986)** | Segment Routing over IPv6 Extension Header (SRH Type 4), 128-bit SID list, Segments Left pointer advancement, destination address mutation. |
| **SR Data Plane** | **SR-MPLS & TI-LFA (RFC 8660 / 8667 / 8402)** | Segment Routing over MPLS with SRGB / SRLB label indexing, Node-SID, Prefix-SID, Adj-SID, Binding-SID expansion, and TI-LFA sub-50ms backup repair label stack generation. |
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
| **Multicast Mgmt** | **IGMPv2 (RFC 2236) & IGMPv3 SSM (RFC 3376 / 4607)** | Group-and-Source-Specific Queries (GSSQ), Membership Reports with Group Records (`MODE_IS_INCLUDE`, `ALLOW_NEW_SOURCES`, `BLOCK_OLD_SOURCES`), host filter mode state machine, RFC 1112 multicast IP to MAC mapping. |
| **Multicast Routing**| **PIM-SM (RFC 7761)** | Protocol Independent Multicast - Sparse Mode over IP Protocol 103 / `224.0.0.13`, Hello, Rendezvous Point (RP) Shared Tree $(*, G)$, Join/Prune signaling. |
| **Link-State Routing** | **OSPFv2 (RFC 2328)** | Link-State Interior Gateway Protocol over IP 89 / `224.0.0.5`, 24-byte OSPF headers, Hello packets, LSDB graph, Dijkstra SPF calculation. |
| **Advanced Routing** | **Cisco EIGRP & DUAL (RFC 7868)** | IP Protocol 88 over Multicast `224.0.0.10`, 20-byte EIGRP header, composite metric formula ($256 \times (10^7/\text{BW} + \text{Delay})$), Feasibility Condition ($RD < FD$), Successor & Feasible Successor loop-free backup path. |
| **Dynamic Routing** | **RIPv2 (RFC 2453)** | Distance-Vector dynamic routing protocol over UDP 520, Bellman-Ford algorithm with Split Horizon & Poison Reverse, metric calculations (1..16). |
| **Inter-Domain Route**| **BGP-4 (RFC 4271)** | Packet-driven BGP speaker running over this stack's own TCP sockets on port 179: full Idle/Connect/Active/OpenSent/OpenConfirm/Established FSM, OPEN negotiation, ConnectRetry / Hold / Keepalive timers, stream reassembly, Adj-RIB-In / Loc-RIB / Adj-RIB-Out, best-path selection, and installation into the real IPv4 forwarding table. |
| **BGP EVPN Fabric** | **MP-BGP EVPN (RFC 4760 / 5492 / 6793 / 7432 / 8365)** | Packet-driven MP-BGP EVPN on the same TCP session as IPv4 unicast: Route Type 1 (Auto-Discovery), Type 2 (MAC/IP), Type 3 (Inclusive Multicast), Type 4 (Ethernet Segment & Modulo DF Election), Type 5 (IP Prefix), MAC mobility, and split-horizon BUM filtering. |
| **Route Reflection** | **BGP Route Reflection (RFC 4456)** | Configured client / non-client peer roles, cluster identity, `ORIGINATOR_ID` and `CLUSTER_LIST` with strict parsing and loop detection, and one reflection engine shared by IPv4 unicast and EVPN. An EVPN route reflector needs no VNI, no `EvpnInstance` and no tenant Route Target, yet retains and reflects the fabric; dual-reflector redundancy, RFC 4271 section 6.8 connection collision resolution, and the RFC 4456 section 9 shortest-`CLUSTER_LIST` tie-break that keeps a reflector pair from oscillating. |
| **Fragmentation (L3)** | **IP Fragmenter & Reassembler** | Splits $> \text{MTU}$ packets into 8-byte aligned slices with `MF` flags; reassembles out-of-order fragment streams. |
| **Control (L3.5)** | **ICMP (RFC 792)** | Type 8 (Echo Request / Ping) and Type 0 (Echo Reply), identifier & sequence number tracking, payload preservation. |
| **Control (L3.5)** | **ICMPv6 & NDP (RFC 4443, 4861, 8106)** | ICMPv6 Echo Request/Reply (`ping6`), Neighbor Solicitation (NS) / Neighbor Advertisement (NA), Router Solicitation / Router Advertisement, RDNSS (`RFC 8106`) recursive DNS servers, DNSSL search lists, MTU option, dynamic in-memory `NdpTable`. |
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
| **Transport (L4)** | **TCP (RFC 793, 9293, 2018, 7323)** | TCP Options (MSS, Window Scale, SACK-Permitted, SACK blocks, Timestamps), out-of-order segment reassembly queue, selective retransmission, full finite-state machine (`LISTEN`, `SYN_SENT`, `SYN_RECEIVED`, `ESTABLISHED`, `CLOSE_WAIT`, `LAST_ACK`), 3-way handshake, sequence & ACK tracking, stream payload buffering, and connection teardown. |
| **Timestamps & PAWS** | **TCP Timestamps (RFC 7323)** | SYN-only option negotiation (section 3.2), the `TS.Recent` update rule gated on `Last.ACK.sent` (section 4.3), and PAWS (section 5.3) discarding old duplicates whose sequence numbers have wrapped back into the receive window, with RFC 1982 serial arithmetic across the 32-bit timestamp wrap. |
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
| **Application (L7)** | **DNS & Modern Extensions (RFC 1035, 3596, 2782, 6891, 2308)** | Multi-record DNS query & response framing (A, AAAA, PTR, CNAME, MX, TXT, SRV, SOA, OPT), in-memory `DnsCache` with positive/negative caching and TTL expiration. |
| **Application (L7)** | **DHCP (RFC 2131)** | DHCP 4-way DORA handshake (Discover $\rightarrow$ Offer $\rightarrow$ Request $\rightarrow$ ACK) with TLV Options over UDP 67/68. |
| **Application (L7)** | **DHCPv6 & Prefix Delegation (RFC 8415 / RFC 3633)** | Dynamic Host Configuration Protocol for IPv6 over UDP 546/547, DUID architecture (DUID-LL, DUID-LLT, DUID-EN, DUID-UUID), 4-message exchange, Rapid Commit 2-message exchange, Prefix Delegation (`IA_PD` / `IAPREFIX`), Client & Server state machines. |
| **BGP Multi-Path** | **BGP ADD-PATH (RFC 7911 / RFC 8277)** | Capability Code 69, 4-byte Path-ID NLRI encoding/decoding, multi-path RIB retention, BGP Prefix Independent Convergence (BGP PIC Edge/Core) primary/backup fast failover. |
| **EVPN All-Active Multicast** | **EVPN IGMP/MLD Synch (RFC 9251)** | Route Type 7 (Join Synch) & Route Type 8 (Leave Synch) NLRIs, Ethernet Segment ID (ESI) multihomed state synchronization, dual-homed PE leave timer reconciliation. |
| **Deterministic IP/MPLS** | **DetNet PREF Data Plane (RFC 8655 / 8938 / 8939 / 8964)** | DetNet Control Word (d-CW), UDP port 3636 encapsulation, Packet Replication and Elimination Functions (PREF), sliding sequence history filter with wraparound handling for zero packet loss. |
| **5G Core / Carrier AAA** | **Diameter Gy / Ro Online Charging (RFC 4006 / 3GPP TS 32.299)** | Credit-Control-Request (CCR) / Credit-Control-Answer (CCA) (Command Code 272, App ID 4), Multiple-Services-Credit-Control (MSCC) grouped AVPs, Rating Groups, quota allocation, volume metering, and subscriber balance depletion. |
| **Multicast RP & SSM** | **PIM-BSR & PIM-SSM (RFC 5059 / RFC 4607 / RFC 7761)** | Dynamic Candidate-RP & BSR election, hash-based deterministic Group-to-RP mapping formula, and SSM (232.0.0.0/8) RP-bypass source-tree routing. |
| **5G / IMS Policy Control** | **Diameter Rx Policy & Charging Control (3GPP TS 29.214)** | Diameter Application ID 16777236, AA-Request (AAR) / AA-Answer (AAA) / Session-Termination (STR), Media-Component-Description grouped AVPs, QCI 1/2 QoS bearer authorization, and PCRF bandwidth admission control. |
| **Datacenter Broadcast Suppression** | **EVPN Proxy ARP/ND & Anycast Gateway (RFC 7432 Section 10 / RFC 9135)** | Proxy ARP cache auto-populated from EVPN Route Type 2 NLRIs, local ARP request snooping, synthesized unicast ARP replies to suppress broadcast flooding across VXLAN fabrics, and Anycast Gateway MAC (`00:00:5E:00:01:XX`) resolution. |
| **Dynamic Service Chaining** | **NSH MD Type 2 Dynamic Variable TLVs (RFC 8300 Section 3.5.2)** | Network Service Header Metadata Type 2 with variable-length Context TLVs (Tenant-ID, Flow-Hash, In-Band Path Trace, Security-Group-Tag), and SFF Service Index decrement forwarding & chain termination. |
| **Multicast MPLS Core** | **Multipoint LDP (mLDP - RFC 6388 / RFC 6513)** | Point-to-Multipoint (P2MP) and Multipoint-to-Multipoint (MP2MP) FEC Elements (Types 6, 7, 8), Root Node Address, Opaque Value TLVs (Generic LSP ID), and zero-head-end multicast branch replication across the MPLS core. |
| **5G / 4G Policy Control** | **Diameter Gx PCC Interface (3GPP TS 29.212)** | Command Code 272 over Application ID 16777238, `Charging-Rule-Install` / `Charging-Rule-Remove` grouped AVPs, dynamic PCC rules, QoS-Information (QCI, ARP, APN-AMBR, Max-Requested-Bandwidth-UL/DL), and PCEF policy gate enforcement. |
| **Datacenter Fast Convergence** | **EVPN Route Type 1 per-ES Mass Withdrawal (RFC 7432 Section 8.2 & 8.4)** | Route Type 1 Ethernet Auto-Discovery per-ES route, instantaneous link-failure mass withdrawal triggering $O(1)$ fast failover to surviving multi-homed PEs without individual Route Type 2 churn. |
| **Segment Routing OAM** | **SR-MPLS OAM: SR LSP Ping & Traceroute (RFC 8287 / RFC 8402)** | Target FEC Stack Sub-TLVs (Type 27 IPv4 Prefix SID, Type 28 IPv6 Prefix SID, Type 29 IPv4 Adjacency SID), Downstream Detailed Mapping (DDMAP) TLV, and SR-MPLS label stack path consistency verification. |
| **5G Fronthaul Timing** | **SyncE ESMC (ITU-T G.8264 / IEEE 802.3 Clause 57)** | Slow Protocols EtherType `0x8809`, Subtype `0x0A`, Quality Level (QL-PRC, QL-SSU-A, QL-SSU-B, QL-SEC, QL-DNU), SSM generation and ITU-T G.781 clock selection arbitration. |
| **5G / 4G Core Mobility**| **Diameter S6a HSS Mobility Interface (3GPP TS 29.272)** | Diameter Application ID `16777251`, AIR/AIA (Command 318), ULR/ULA (Command 316), EPS Authentication Vector generation (`RAND`, `XRES`, `AUTN`, `KASME`), and subscriber profile directory. |
| **Datacenter Cloud Security**| **EVPN E-Tree Service Architecture (RFC 8317)** | BGP EVPN E-Tree Extended Community (Type `0x06`, Sub-type `0x05`), Leaf Indication bit (`L-bit`), Leaf-label encoding, and Split-Horizon Root/Leaf forwarding policy enforcement. |
| **Segment Routing 5G Slicing**| **SRv6 Network Slicing & VTN Data Plane (RFC 9350 / RFC 9543)** | SRv6 Slice/VTN Identifier data plane, binding Flex-Algo 128 (URLLC Low-Latency) & Flex-Algo 129 (eMBB High-Throughput) to dedicated SRv6 segment lists, bandwidth SLA metering, and deterministic slice isolation. |
| **EVPN Multihoming DF** | **EVPN Preference-Based DF Election (RFC 8584)** | DF Election Extended Community (Type `0x06`, Sub-type `0x06`), Algorithm `0x02` (Preference-based), Don't Preempt bit (`DP`), Sticky bit (`S`), and deterministic highest-IP tie-breaking. |
| **In-Band Flow Analytics**| **IFA 2.0 In-Situ Flow Telemetry (RFC 9197 / IETF IFA 2.0)** | In-situ Flow Analytics hop-by-hop telemetry vector (Node-ID, Ingress/Egress ports, Queue Depth in bytes, Hop Latency in ns), ingress probe encapsulation, transit insertion, and egress telemetry extraction. |
| **5G / 4G Device Security**| **Diameter S13 / S13' EIR Interface (3GPP TS 29.272 Section 6)** | Diameter Application ID `16777252`, ME-Identity-Check (ECR/ECA - Command 324), Equipment-Status (AVP 1445: Whitelisted/Blacklisted/Greylisted), and stolen mobile equipment barring. |
| **5G Telecom Boundary Clock**| **PTP Telecom Boundary Clock (T-BC) (ITU-T G.8275.1 / G.8273.2)** | Alternate BMCA state machine, `localPriority` (1..255), Steps-Removed override, multi-port Boundary Clock state transitions (`Master`, `Slave`, `Passive`), and phase error damping filter. |
| **5G Fronthaul Timing Compliance**| **PTP Telecom Time Error (cTE / dTE) Modeling (ITU-T G.8273.2 / G.8271.1)** | Time Error $TE(t)$ real-time sampling, Constant Time Error ($cTE$) window moving average, Dynamic Time Error ($dTE$) peak-to-peak amplitude, and Class A, B, C, D 5G fronthaul mask verification. |
| **5G / 4G Policy Roaming**| **Diameter S9 PCRF Roaming Interface (3GPP TS 29.215)** | Diameter Application ID `16777267`, CCR/CCA Credit-Control over S9, `Subsession-Enforcement-Info` (AVP 2201), `Subsession-Decision-Info` (AVP 2200), and H-PCRF/V-PCRF roaming QoS provisioning. |
| **Datacenter EVPN Multicast**| **EVPN IGMP Snooping & Multicast Pruning (RFC 9251 Section 5 & 6)** | Access bridge port IGMP snooping table per $(VNI, Group)$, BGP EVPN Route Type 7/8 synchronization triggers, and edge multicast forwarding tree pruning to eliminate BUM flooding. |
| **DDoS Scrubbing & Mitigation**| **BGP Flowspec Redirect-to-VRF & DSCP Remarking (RFC 5575 / RFC 8955)** | BGP Flowspec Extended Community Type `0x80` Subtype `0x08` (Redirect to VRF / Route Target) and Subtype `0x09` (DSCP Remarking), automated DDoS mitigation, and quarantine traffic steering. |
| **5G User-Plane Telemetry**| **5G GTP-U PDU Session Container & In-Band Delay Telemetry (3GPP TS 38.415 / 29.281)** | GTP-U Extension Header Type `0x85`, 6-bit QoS Flow Identifier (QFI), Reflective QoS (RQI), Paging Policy (PPI), and RAN-to-UPF microsecond in-band transport delay reporting. |
| **5G P2P Transparent Clock**| **PTP Telecom Peer-to-Peer Transparent Clock (T-TC) (ITU-T G.8275.2 / IEEE 1588)** | P2P Peer Delay calculation via Pdelay_Req/Resp ($t_1..t_4$), ingress-to-egress residence time accumulation, and sub-nanosecond correctionField updating. |
| **5G / IMS Application Server**| **Diameter Sh AS-to-HSS Interface (3GPP TS 29.328 / TS 29.329)** | Diameter Application ID `16777217`, User-Data (UDR/UDA - Command 306), Subscribe-Notifications (SNR/SNA - Command 308), and transparent IMS service repository management. |
| **Datacenter Shared Services**| **EVPN Layer 3 Multi-VRF Route Leaking (RFC 9136 / RFC 4364 Section 10)** | Cross-VRF Route Target import/export intersection matching, tenant segmentation with shared Internet/DNS gateway leaking, and per-VRF LPM lookup. |
| **TSN Deterministic Scheduling**| **IEEE 802.1Qbv Time-Aware Shaper (TAS) GCL Engine (IEEE 802.1Qbv)** | Cyclic Gate Control List (GCL) scheduling for 8 Traffic Classes (TC 0..7), sub-microsecond epoch alignment, guard-band calculation, and protected scheduled traffic windows. |
| **5G / 4G Emergency Location**| **Diameter SLh LCS Location Services Interface (3GPP TS 29.173 / TS 29.171)** | Diameter Application ID `16777291`, LCS-Routing-Info (RIR/RIA - Command 8388620), `Serving-Node` (AVP 2401 Grouped), and GMLC-to-HSS subscriber serving MME/AMF resolution for E911. |
| **Datacenter Storm Control**| **EVPN Layer 2 Unknown Unicast (UU) Flood Suppression (RFC 7432 Section 13.2 / RFC 8317)** | Access bridge port Unknown Unicast suppression policy, gating against EVPN Type 2 MAC/IP RIB, and unknown frame drop protection against core broadcast storms. |
| **Overlay In-Band Telemetry**| **Geneve In-Band Network Telemetry (INT) Option Header (RFC 8926 Section 4.4)** | Geneve Variable Length Option TLV (Class `0x0103`, Type `0x01`), hop metadata insertion (Switch ID, Ingress/Egress ports, nanosecond latency, queue occupancy in bytes), and overlay telemetry extraction. |
| **TSN Zero-Loss Recovery**| **IEEE 802.1CB Frame Replication & Elimination (FRER) Sequence Recovery Function (SRF)** | Vector Recovery Algorithm (VRA / IEEE 802.1CB Section 7.4), sliding bit-vector history window, wrap-around serial number arithmetic, and per-stream duplicate elimination. |
| **5G / IMS Core Registration**| **Diameter Cx/Dx IMS Registration & Auth Interface (3GPP TS 29.228 / TS 29.229)** | Diameter Application ID `16777216`, User-Authorization (UAR/UAA - Command 300), Multimedia-Auth (MAR/MAA - Command 303), Server-Assignment (SAR/SAA - Command 301), and HSS authentication vector delivery. |
| **Datacenter EVPN Mobility**| **EVPN MAC Mobility Sequence Tracking & Sticky MAC Suppression (RFC 7432 Section 15)** | MAC Mobility Extended Community Type `0x06` Subtype `0x00`, monotonic sequence number progression, sticky/static MAC enforcement, and flapping duplicate-detection suppression. |
| **5G / 4G Session Control**| **3GPP GTPv2-C Session Management & Create Session Handshake (3GPP TS 29.274)** | GTPv2-C control plane header/IE codec, Create Session Request/Response (Msg 32/33), IMSI TBCD encoding, F-TEID, APN, EPS Bearer ID (EBI), and SGW session state management. |
| **TSN Peristaltic Shaping**| **IEEE 802.1Qch Multi-Queue Cyclic Queuing & Forwarding (CQF)** | 3-Queue rotating cyclic buffer ($Q_0, Q_1, Q_2$), zero-jitter bounded latency $D_{hop} \in [T_{cycle}, 2 \cdot T_{cycle}]$, phase offset synchronization, and buffer overflow protection. |
| **5G Non-3GPP Access** | **Diameter S6b Untrusted WLAN / ePDG AAA Interface (3GPP TS 29.273)** | Diameter Application ID `16777272`, AA-Request (AAR) / AA-Answer (AAA - Command 265), STR/STA (Command 275), MIP6-Agent-Info allocation, and ANID authorization. |
| **Datacenter Fast Reroute**| **EVPN Fast Reroute (FRR) & Secondary Nexthop Protection (RFC 7432 Section 16)** | Pre-computed backup nexthop and repair encapsulation, sub-millisecond local link fault detection, instantaneous hitless failover steering, and automatic recovery. |
| **5G SRv6 Direct Routing** | **SRv6 Mobile User Plane (MUP) End.M.GTP6.D/E Interworking (draft-ietf-dmm-srv6-mobile-uplane)** | Stateless translation between 5G GTP-U user-plane encapsulation and pure IPv6 Segment Routing underlay, `End.M.GTP6.D` stripping & `End.M.GTP6.E` restoration. |
| **Datacenter MAC Purge** | **EVPN Layer 2 MAC Flush on Link/Port Down (RFC 7432 Section 15 / RFC 8317)** | Rapid MAC table purging on local attachment circuit / LAG failure without waiting for aging timers, granular flush scopes (`AllOnEsi`, `VniOnEsi`, `SpecificMac`). |
| **5G Path Liveness** | **GTP-U Path Management Echo Request/Response & Heartbeat (3GPP TS 29.281 Section 7.2)** | Path reachability monitoring over UDP 2152, `Recovery` IE restart counter tracking, $N3\text{-REQUESTS}$ retry state machine, and path failure alarm notifications. |
| **TSN Stream Policing** | **IEEE 802.1Qci Per-Stream Filtering & Policing (PSFP) trTCM Multi-Stage Filter** | 3-stage cascaded inspection (SFI stream identification & Max SDU, SGI gate schedule & violation traps, FMI RFC 2698 trTCM CIR/CBS/PIR/PBS color marker). |
| **5G Untrusted Wi-Fi AAA** | **Diameter SWm / SWx Untrusted WLAN / ePDG AAA Interface (3GPP TS 29.273)** | Diameter Application ID `16777264` (SWm) / `16777265` (SWx), DER/DEA Command 268, EAP-AKA' authentication, and 64-byte Master Session Key (MSK) key derivation. |
| **TSN Buffer Isolation** | **IEEE 802.1Qcz Congestion Isolation & Head-of-Line (HoL) Blocking Mitigation** | Congestion Point (CP) dual-queue architecture (UQ/CIQ), offending flow detection, IEEE 802.1Qcz CNM generation, and line-rate forwarding for uncongested traffic. |
| **5G Direct EIR Check** | **Diameter S13' Direct EIR Interface & Terminal-Information IMEI-SV (3GPP TS 29.272)** | Diameter Application ID `16777252`, ECR/ECA Command 324, `Terminal-Information` (AVP 1401 Grouped) IMEI-SV tracking, rogue software version detection, and EIR status query. |
| **Datacenter Multicast Prune** | **EVPN Selective Ingress Replication (IR) & Leaf Pruning (RFC 7432 Section 11 / RFC 9251)** | Dynamic $(VNI, S, G)$ Ingress Replication lists built from Route Type 6 (SMET), selective replication to interested leaf VTEPs only, and non-interested PE pruning. |
| **5G Packet Reordering** | **3GPP GTP-U Sequence Number Out-of-Order Reordering & Jitter Buffer (3GPP TS 29.281)** | 16-bit GTP-U sequence number sliding window, RFC 1982 wrap-around arithmetic, out-of-order jitter buffering, and in-order contiguous upper-layer delivery. |
| **TSN Asynchronous Shaping** | **IEEE 802.1Qcr Asynchronous Traffic Shaping (ATS) Multi-Hop Cascaded Pipeline** | Urgency-Based Scheduler (UBS), per-flow Interleaved Regulator (IR) Eligibility Time calculation, multi-hop burstiness absorption, and bounded deterministic latency. |
| **5G SMS Core Delivery** | **Diameter SGd / T4 SMS Core Relay Interface (3GPP TS 29.338)** | Diameter Application ID `16777313`, MO/MT Forward Short Message (OFR/OFA, TFR/TFA Command 8388645/8388646), and `SM-RP-UI` SMS TPDU relay. |
| **Datacenter Anycast IRB** | **EVPN Layer 3 Anycast Gateway & Symmetric/Asymmetric IRB Dual-Mode (RFC 9135 / RFC 9136)** | Distributed Anycast Gateway MAC (`00:00:5E:00:01:01`) active on all leafs, Symmetric IRB with Transit L3VNI, and Asymmetric IRB direct L2VNI routing/bridging. |
| **5G UPF Anchor Handover** | **3GPP GTP-U UPF Anchor Relocation, Indirect Forwarding & End Marker (3GPP TS 23.501)** | S-UPF to T-UPF indirect GTP-U tunnel forwarding during handovers, End Marker (Msg Type 254) packet injection, and hitless user-plane cut-over. |
| **TSN GCL Dynamic Reconfig** | **IEEE 802.1Qbv TAS Dynamic GCL Reconfiguration & Hitless Admin-to-Oper Swap** | AdminGcl vs OperGcl dual-schedule state machine, `AdminBaseTime` atomic cycle epoch transition, and hitless schedule update without frame truncation. |
| **5G GBA Bootstrapping** | **Diameter Zh / GAA / GBA Bootstrapping Interface (3GPP TS 29.109)** | Diameter Application ID `16777221`, MAR/MAA Command 303, GUSS XML configuration delivery, and application-specific `Ks_NAF` key derivation. |
| **Datacenter BUM Storm Defense** | **EVPN Layer 2 BUM Traffic Storm Policer & Microburst Rate Limiter (RFC 7432)** | Independent Broadcast, Unknown Unicast, and Multicast token buckets per VNI, storm drop threshold tracking, and rogue MAC quarantine defense. |
| **5G QoS & AMBR Enforcement** | **3GPP TS 38.415 / TS 23.501 5G GTP-U QoS Flow Identifier & Session-AMBR Token Bucket** | 6-bit QFI packet classification, 5QI profile mapping (GBR/Non-GBR), Session-AMBR token bucket rate enforcement, and dynamic QFI remapping. |
| **TSN Credit-Based Shaper** | **IEEE 802.1Qav Credit-Based Shaper (CBS) Multi-Class Audio/Video Bridging Engine** | Class A and Class B bandwidth slopes (`idleSlope`, `sendSlope`), credit ceiling/floor tracking, and starvation prevention for best-effort traffic. |
| **5G SMS Routing Lookup** | **Diameter S6c SMS-GMSC/SMS-IWMSC to HSS Routing Query Interface (3GPP TS 29.338)** | Diameter Application ID `16777312`, SRR/SRA Command 8388647, RDR/RDA Command 8388648, and `Serving-Node` MME/SGSN/SMSF resolution. |
| **Datacenter Core Isolation** | **EVPN Layer 2 Core Isolation Defense & Split-Horizon Group Filtering (RFC 7432)** | Underlay spine uplink link-state tracking with automated client port shutdown, and ESI split-horizon label loop suppression on multi-homed PEs. |
| **5G Fast Path Failover** | **3GPP TS 23.501 5G GTP-U Path Loss Detection & Sub-Millisecond Fast Failover** | Primary/Secondary N3/N9 user plane path pre-provisioning, consecutive loss detection, autonomous sub-millisecond rerouting, and auto-reversion. |
| **TSN Frame Preemption** | **IEEE 802.1Qbu / 802.3br Frame Preemption & Qbv Dynamic Guard Band Engine** | Express vs Preemptable classification, dynamic guard band shrinkage down to 64B fragment boundary, and MAC Merge Hold/Release state machine. |
| **5G RAN Congestion** | **Diameter Np RAN User Plane Congestion Awareness Interface (3GPP TS 29.217)** | Diameter Application ID `16777342`, Non-Aggregated RUCI Report (NCR/NCA Command 8388725), and eNodeB/Cell congestion-level telemetry. |
| **Datacenter Flap Damping** | **EVPN Layer 2 Port Flap Damping & Route Dampening Exponential Decay (RFC 7432)** | Exponential half-life penalty decay, configurable Suppress/Reuse thresholds, and automated AC suppression to prevent BGP control-plane churn. |
| **5G MA-PDU ATSSS** | **3GPP TS 23.501 5G Multi-Access PDU (MA-PDU) Session & ATSSS Steering Engine** | Dual 3GPP/Non-3GPP access legs, Active-Standby, Smallest-Delay, and Weighted Load-Balancing steering policies with RTT telemetry. |
| **TSN CQF Time Dispatch** | **IEEE 802.1Qch CQF Cyclic Time-Based Frame Dispatch & Ping-Pong Queue Ingestion** | Microsecond cycle clock tick driving deterministic ping-pong queue swapping, and zero jitter across multi-hop topologies. |
| **5G CIoT SCEF Config** | **Diameter S6t SCEF to HSS Cellular IoT NIDD & Monitoring Event Interface (3GPP TS 29.336)** | Diameter Application ID `16777345`, CIR/CIA Command 8388728, and `Monitoring-Event-Configuration` for reachability/location tracking. |
| **Datacenter Private VLAN** | **EVPN Layer 2 Private VLAN (PVLAN) & Port Isolation Micro-Segmentation (RFC 7432 / RFC 5517)** | Promiscuous, Isolated, and Community port type classification, and inter-port micro-segmentation forwarding matrix. |
| **5G URLLC Redundant Paths** | **3GPP TS 23.501 5G GTP-U Redundant User Plane Transmission & Egress Deduplication** | Dual-tunnel packet replication at UPF/gNodeB, unified GTP-U sequence numbering, and zero-latency egress deduplication window. |
| **TSN CQF trTCM Meter** | **IEEE 802.1Qch Multi-Class CQF with trTCM Traffic Metering Integration (RFC 2698)** | PIR/PBS and CIR/CBS Two-Rate Three-Color Marker metering at cyclic queue ingress, and color-aware frame admission/dropping. |
| **TSN CQF Phase Offset** | **IEEE 802.1Qch CQF Multi-Hop Phase Offset Alignment Engine** | Propagation delay and switch internal processing compensation $\Delta \phi = (t_{\text{prop}} + t_{\text{proc}}) \pmod{T_{\text{cycle}}}$ with multi-hop end-to-end bounded delay guarantee. |
| **TSN CQF Deadline & Buffer** | **IEEE 802.1Qch CQF Deadline Expiry & Buffer Overrun Protection Engine** | Ingress safe admission window $[0, T_{\text{deadline}}]$, cycle capacity limits, and deterministic cycle-deferral vs deadline miss dropping. |
| **TSN CQF Dynamic Scale** | **IEEE 802.1Qch CQF Dynamic Cycle Duration Scaling Engine** | Hitless runtime cycle period reconfiguration, Admin/Oper state transition, and frame drain completion verification. |
| **TSN CQF Gate Coord** | **IEEE 802.1Qch CQF Multi-Priority Gate State Coordinated Dispatch Engine** | Priority bitmask gate state coordination, 8-PCP class filtering, and cycle boundary leak prevention. |
| **TSN CQF Jitter Bound** | **IEEE 802.1Qch CQF Multi-Hop Jitter Accumulation & Bounded Delay Predictor** | Multi-hop propagation delay and bridge forwarding evaluation $\text{Delay}_{\min}/\text{Delay}_{\max}$, jitter bounds, and stream SLA validation. |
| **TSN CQF Deficit Meter** | **IEEE 802.1Qch CQF Ingress Deficit Metering & Buffer Overrun Protection Engine** | Per-stream cycle quantum allocation, bounded credit carryover, and deterministic ingress policing against micro-bursts. |
| **TSN CQF Prio Promote** | **IEEE 802.1Qch CQF Stream Priority Promotion & Preemption Fallback Engine** | Dynamic frame PCP elevation based on residency age near deadline, drop protection, and preemption fallback demotion. |
| **TSN CQF Jitter Analyzer** | **IEEE 802.1Qch CQF Cyclic Frame Timestamping & End-to-End Jitter Analyzer** | Nanosecond ingress/egress transit timestamp stamping, real-time packet delay variation (jitter), and SLA breach flagging. |
| **TSN CQF Prio Inherit** | **IEEE 802.1Qch CQF Stream Priority Inheritance & Inversion Prevention Engine** | Dynamic effective PCP elevation for blocking frames, dependency resolution, and head-of-line priority inversion prevention. |
| **TSN CQF Gate Preempt** | **IEEE 802.1Qch CQF Gate Preemption Interlocking Engine (802.1Qbu / 802.3br)** | Preemptible frame fragmentation into mPackets, minimized 64-byte guard band, and bandwidth recovery before cycle gates. |
| **5G MAP-to-Diameter** | **Diameter S6m / S6n MAP-to-Diameter HSS Interworking Interface (3GPP TS 29.336)** | Diameter Application ID `16777310`, SIR/SIA Command 8388641, and subscriber authorization query for mobile-originated SMS routing. |
| **5G Dynamic Profile Update** | **Diameter S6a / S6d Insert-Subscriber-Data (IDR/IDA) Interface (3GPP TS 29.272)** | Application ID `16777251`, Command 319, HSS-initiated dynamic subscription & QoS AMBR profile updates pushed directly to MME/SGSN. |
| **5G Purge-UE Release** | **Diameter S6a / S6d Purge-UE (PUR / PUA) Context Reclamation Interface (3GPP TS 29.272)** | Application ID `16777251`, Command 321, MME-to-HSS subscriber detachment & TMSI freeze signaling (`PUA-Flags`). |
| **5G HSS Reset Resync** | **Diameter S6a / S6d Reset (RSR / RSA) Interface (3GPP TS 29.272)** | Application ID `16777251`, Command 322, HSS restart notification to MMEs for global/targeted subscriber state resynchronization. |
| **5G User Authorization** | **Diameter S6a / S6d User-Authorization (UAR / UAA) Interface (3GPP TS 29.272)** | Application ID `16777251`, Command 316, Visited-PLMN roaming authorization validation, emergency attach override, and HSS discovery. |
| **5G Cancel-Location Release** | **Diameter S6a / S6d Cancel-Location (CLR / CLA) Interface (3GPP TS 29.272)** | Application ID `16777251`, Command 317, HSS-to-MME context deletion with cancellation reasons (MME update, subscription revoke). |
| **5G Notify Synchronization** | **Diameter S6a / S6d Notify (NOR / NOA) Event Signaling Interface (3GPP TS 29.272)** | Application ID `16777251`, Command 323, MME-to-HSS dynamic state notifications (SRVCC support, IMEI changes, Ready-for-SM). |
| **5G Local EIR Cache** | **3GPP TS 29.272 Diameter S13 / S13' Dynamic Local EIR Cache & Expiry Engine** | Local MME/AMF IMEI classification caching (White/Black/Gray), dynamic TTL invalidation, and batch feed synchronization. |
| **5G Bulk EIR Push** | **3GPP TS 29.272 Diameter S13 / S13' Bulk IMEI Blacklist Push (BBP) Interface** | Application ID `16777252`, Command 326, high-throughput bulk IMEI blacklist push/removal with batch sequence versioning. |
| **5G GSMA IMEI-DB Query** | **3GPP TS 29.272 Diameter S13 / S13' Global GSMA IMEI-DB Federated Query Interface** | Application ID `16777252`, Command 327, global GSMA device fraud & stolen equipment federation queries across roaming partners. |
| **5G EIR Graylist Check** | **Diameter S13 Equipment Identity Check with Graylist Throttling (3GPP TS 29.272)** | Application ID `16777252`, ECR/ECA Command 324, White/Black/Gray IMEI evaluation, and dynamic bandwidth throttling for observed devices. |
| **Datacenter Unknown Multicast** | **EVPN Layer 2 Unknown Multicast Tree (UMT) & Ingress Replication Optimization (RFC 7432 / RFC 9251)** | Dynamic Selective Multicast (SMET) vs Inclusive Multicast (IMET) tree resolution and non-participating remote leaf pruning. |
| **Datacenter Port Security** | **EVPN Layer 2 Dynamic Port Security & Sticky MAC Aging (RFC 7432 Section 15)** | Configurable max MAC limits per port, Protect/Restrict/Shutdown violation policies, and sticky MAC learning with inactivity aging. |
| **Datacenter UU Rate Limiting** | **EVPN Layer 2 Unknown Unicast Storm Suppression Rate-Limiter (RFC 7432 Section 16)** | Token bucket rate-policing on flooded Unknown Unicast frames per VNI/EVI with burst tolerance and storm exhaustion protection. |
| **Datacenter UU Egress Prune** | **EVPN Layer 2 Unknown Unicast Egress Horizon & ESI Pruning Engine (RFC 7432)** | Multi-homed source ESI frame suppression, split-horizon egress domain isolation, and per-VNI active membership filtering. |
| **Datacenter MAC Mobility Freeze** | **EVPN Layer 2 MAC Address Mobility Freeze & Move Flap Damping Engine (RFC 7432)** | Sliding-window move threshold detection, automatic MAC quarantine/freeze, and sequence progression protection. |
| **Datacenter IP Anti-Spoofing** | **EVPN Layer 2 ARP / ND Snooping & Distributed IP Anti-Spoofing Policy Engine (RFC 7432 / RFC 9136)** | IP Source Guard binding verification $(VNI, Port, MAC, IP)$, port trust modes, and fine-grained drop classifications. |
| **Datacenter DHCP Snooping** | **EVPN Layer 2 DHCPv4 / DHCPv6 Snooping & Dynamic Option 82 Relay Engine (RFC 7432 / RFC 3046)** | Transparent Option 82 Circuit-ID/Remote-ID injection, rogue DHCP server filtering, and dynamic lease binding registration. |
| **Datacenter Dynamic ARP Inspect** | **EVPN Layer 2 Dynamic ARP Inspection (DAI) & Per-Port Rate-Limiter Engine (RFC 7432)** | Intercepts ARP frames against snooped IP-MAC binding tables with token-bucket storm mitigation on untrusted edge access ports. |
| **Datacenter UMRT Replication** | **EVPN Layer 2 Unknown Multicast Replication Tree (UMRT) & Pruning Engine (RFC 7432 / RFC 9251)** | Ingress replication tree generation for unknown multicast, selective access port pruning, and overlay split-horizon isolation. |
| **Datacenter Host Tracking** | **EVPN Layer 2 Dynamic Host Tracking (DHT) & Silent Host Probing Engine (RFC 7432)** | Inactivity tracking for silent edge hosts, targeted unicast ARP keep-alive probes, and accelerated Type-2 route withdrawal. |
| **Datacenter SSM Underlay** | **EVPN Layer 2 Selective Multicast Underlay Provider P-Tree / PMSI Mapping Engine (RFC 6514 / RFC 9251)** | Ingress Replication (IR) to S-PMSI core multicast encapsulation mapping with dynamic leaf threshold switching. |
| **Datacenter SSM DR Election** | **EVPN Layer 2 SSM Designated Router (DR) Election & Querier Sync Engine (RFC 8584 / RFC 9251)** | Multi-homed ESI multicast DR election with priority/IP tie-breaking and IGMP querier keepalive failover. |
| **5G Geofencing & Fraud** | **3GPP TS 29.272 Diameter S13 / S13' Geofencing & Cell-ID Anomaly Detection Engine** | ECGI / TAC tracking, restricted security zone enforcement, and supersonic impossible travel velocity detection. |
| **5G QoS Marking & DSCP** | **3GPP TS 29.281 / TS 38.415 5G GTP-U Outer IP DSCP & 802.1p PCP Dynamic QoS Mapping Engine** | QFI to 5QI mapping, DiffServ ToS byte generation, and 802.1Q PCP priority tagging for high-speed transport. |
| **TSN CQF Slot Reservation** | **IEEE 802.1Qch CQF Time-Slot Dynamic Bandwidth Reservation & Admission Engine** | Micro-slot slice reservation ($S_0..S_{k-1}$), bandwidth quota admission, and runtime timing conformance verification. |
| **TSN CQF Frame Replication** | **IEEE 802.1CB / 802.1Qch Cyclic Frame Replication & FRER Elimination Interworking Engine** | Dual-path R-TAG sequence replication across alternating CQF cycles and vector recovery sliding window elimination. |
| **5G IMEI-SV Tamper Validation** | **3GPP TS 29.272 Diameter S13 Hardware IMEI-SV Tamper & Luhn Validation Engine** | Luhn Mod-10 check digit verification, 16-digit IMEI-SV manufacturer profiling, and clone pattern detection. |
| **EVPN SSM Source Active** | **EVPN Layer 2 SSM Source Active (SA) Route Synchronization Engine (RFC 9251)** | Automatic Source Active route generation upon $(S, G)$ multicast ingress detection, remote PE learning, and inactivity aging. |
| **5G-to-4G Bearer Translation** | **3GPP TS 29.281 / TS 23.501 5G-to-4G Bearer ID (EBI) to QoS Flow (QFI) Translation Engine** | Bidirectional EBI 5..15 $\leftrightarrow$ QFI 1..64 mapping, multi-QFI aggregation into EPS bearers, and fallback handling. |
| **TSN CQF Burst Absorption** | **IEEE 802.1Qch CQF Cyclic Burst Absorption & Leaky Bucket Shaper Engine** | Dual-token bucket rate shaping (CIR/CBS/PBS), micro-burst buffer absorption, and bounded cycle delayed drain. |
| **5G Roaming TAC Mismatch** | **3GPP TS 29.272 Diameter S13 Roaming TAC Country Code Mismatch Detection Engine** | Home/serving country validation, MCC mismatch risk scoring, authorized roaming verification, and sanction blacklisting. |
| **EVPN Explicit Tracking** | **EVPN Layer 2 Multicast IGMPv3/MLDv2 Explicit Tracking & Fast Leave Engine (RFC 9251 / RFC 3376)** | Per-host $(S, G)$ subscriber tracking, immediate Fast Leave port pruning on last subscriber exit, and SMET sync. |
| **5G Multi-Tenancy DNN Demux** | **3GPP TS 29.281 / TS 23.501 5G GTP-U Network Instance & Multi-Tenancy DNN Demux Engine** | TEID to DNN/Tenant VRF demuxing, per-tenant bandwidth rate limiting, and cross-tenant injection validation. |
| **5G Path Jitter Telemetry** | **3GPP TS 38.415 / RFC 3550 5G GTP-U Path Jitter & Microsecond Delay Measurement Telemetry** | Microsecond timestamping in PDU Session Containers, RFC 3550 exponential moving average jitter, and min/max/avg latency tracking. |
| **5G ATSSS Active Probing** | **3GPP TS 24.193 / TS 23.501 5G Multi-Access Latency-Aware Active Probing Engine** | PMF synthetic echo probe dispatching across 3GPP and Non-3GPP legs, EWMA smoothed RTT calculation, and dynamic optimal leg election. |
| **5G Adaptive Echo Heartbeat** | **3GPP TS 29.281 5G GTP-U Adaptive Heartbeat & Loss-Triggered Fast Probing Engine** | Dual-mode heartbeat interval with loss-triggered sub-second fast probing acceleration and automatic healthy interval restoration. |
| **5G RTT Variance & RTO** | **3GPP TS 29.281 / RFC 6298 5G GTP-U Path RTT Variance & Adaptive RTO Predictor** | Integer fixed-point SRTT/RTTVAR computation, exponential back-off timeout adaptation, and forward/reverse delay asymmetry detection. |
| **5G Packet Loss Telemetry** | **3GPP TS 38.415 / ITU-T Y.1731 5G GTP-U In-Band Packet Loss Measurement (LMM/LMR) Engine** | Dual-ended frame count exchange ($T_{xFCf}, R_{xFCf}, T_{xFCb}, R_{xFCb}$), Far-End/Near-End loss, and basis-point loss ratio calculation. |
| **5G ATSSS Packet Split** | **3GPP TS 23.501 / TS 24.193 5G ATSSS Dynamic Packet Splitting & Aggregation Engine** | Weighted Round-Robin (WRR) multi-access packet dispatching, monotonic sequence stamping, and automatic failover on leg degradation. |
| **5G Dual-EMA RTT Smoothing** | **3GPP TS 29.281 5G GTP-U Path RTT Dual-EMA Smoothing & Spike Anomaly Filter** | Dual-time-constant EMA filter ($\alpha_{\text{fast}}$ vs $\alpha_{\text{slow}}$), micro-burst surge detection, and route optimization recognition. |
| **5G Multi-Link Aggregation** | **3GPP TS 29.281 / RFC 8684 5G GTP-U Multi-Link Flow Distribution & Aggregation Engine** | Deterministic 5-tuple micro-flow pinning, link health monitoring (Active/Degraded/Down), and resilient dynamic failover re-hashing. |
| **5G Adaptive Jitter Buffer** | **3GPP TS 29.281 / TS 23.501 5G GTP-U Path RTT-Adaptive Jitter Buffer Engine** | Real-time SRTT/RTTVAR adaptive delay scaling, in-order packet release, and automatic packet loss gap skipping upon hold timeout. |
| **5G Sequence Gap Retransmit** | **3GPP TS 29.281 / TS 38.415 5G GTP-U Sequence Gap Detection & Fast Retransmit Trigger** | Missing sequence hole tracking across multi-access legs, out-of-order packet threshold detection, and targeted NACK trigger. |
| **5G RTT-Adaptive Duplication** | **3GPP TS 29.281 / TS 23.501 5G GTP-U RTT-Adaptive Packet Duplication Engine** | Dynamic URLLC packet duplication across dual access legs triggered by SRTT/RTTVAR degradation with hysteresis recovery. |
| **TSN CQF Dual-Plane** | **IEEE 802.1Qch CQF Dual-Plane Redundancy & Redundant Path Engine** | Active/Passive and Dual-Active cycle dispatching, dynamic plane telemetry monitoring, and hitless failover. |
| **5G Equipment Notification** | **3GPP TS 29.272 Diameter S13 Equipment Status Change Notification (ESCN) & Audit** | Real-time IMEI status change notifications, MME subscription registry, ACK tracking, and reconciliation audits. |
| **EVPN IGMP Join Suppress** | **EVPN Layer 2 Multicast IGMPv3 Join Suppression & Proxy Reporting Engine (RFC 9251)** | Host join/leave aggregation, proxy IGMP report generation, and SMET Route Type 6 synchronization suppression. |
| **5G Reorder & Early Flush** | **3GPP TS 29.281 5G GTP-U Sequence Reordering & Early Timeout Flush Engine** | Sliding window buffer, dead sequence advancement, and age-based early flush preventing head-of-line blocking. |
| **TSN CQF Path Splicing** | **IEEE 802.1Qch CQF Dynamic Path Splicing & Hitless Switchover Engine** | Cycle-aligned live stream path transition, multi-hop phase delay compensation, and lossless switchover. |
| **5G Emergency Exemption** | **3GPP TS 29.272 Diameter S13 Emergency Call & eCall IMEI Exemption Engine** | Regulatory emergency override for blacklisted/unknown IMEIs, temporary session tracking, and restricted APN enforcement. |
| **EVPN Querier Election** | **EVPN Layer 2 IGMP Snooping Querier Election & Keepalive Failover (RFC 9251 / RFC 2236)** | Multi-homed lowest-IP querier election, other-querier present timer tracking, and non-disruptive failover. |
| **5G Hole & Proactive NACK** | **3GPP TS 29.281 / TS 38.415 5G GTP-U Sequence Hole Filling & Proactive NACK Engine** | Ingest gap detection, sliding window missing list, periodic proactive NACK retransmissions, and in-place recovery. |
| **TSN CQF Fragment Reassembly** | **IEEE 802.1Qch / 802.1Qbu Cyclic Frame Preemption Fragment Reassembly Engine** | Deterministic reassembly of preempted frame fragments (mPackets) across cyclic gate transitions with CRC verification and timeout eviction. |
| **5G TAC Range Matching** | **3GPP TS 29.272 Diameter S13 TAC / IMEI-SV Range Matching Engine** | High-throughput range categorization and wildcard TAC (Type Allocation Code) lookup for regulatory blocklists and manufacturer profiling. |
| **EVPN Snooping Boundary Filter** | **EVPN Layer 2 Multicast IGMP/MLD Snooping Group Boundary Filter & CAC Engine (RFC 9251)** | Per-port / per-VNI multicast group access control lists (ACLs), Channel Admission Control (CAC) quotas, and rogue group suppression. |
| **5G Sliding Window ACK & SACK** | **3GPP TS 29.281 / TS 38.415 5G GTP-U Reliable Transport Sliding Window ACK / SACK Engine** | Cumulative acknowledgment tracking, selective acknowledgment (SACK) block generation, sliding window buffering, and recovery signaling. |
| **TSN CQF Max-SDU Enforcer** | **IEEE 802.1Qch / 802.1Qci Maximum SDU Size Enforcement & Cyclic Truncation Engine** | Per-stream Max-SDU length validation, babbling-frame protection, configurable drop/truncate/alert policies, and cyclic byte forwarding tracking. |
| **5G TAC Lease Expiry** | **3GPP TS 29.272 Diameter S13 Temporary Whitelist & Lease Expiry Engine** | Time-bounded TAC whitelist leases, configurable grace-period access, lease renewals/revocations, and deterministic fallback enforcement. |
| **EVPN Multicast Rate Policer** | **EVPN Layer 2 Multicast IGMP/MLD Control Message Rate Limiter & Storm Policer (RFC 9251)** | Token-bucket rate limiting on IGMP/MLD membership reports and queries, burst tolerance, consecutive-drop tracking, and port quarantine penalty box. |
| **5G Flow Label Entropy** | **3GPP TS 29.281 / RFC 6437 5G GTP-U Outer IPv6 Flow Label Entropy & ECMP Hashing Engine** | Inner 5-tuple and 5G session ID (TEID, QFI) entropy extraction, 20-bit IPv6 Flow Label generation (FNV-1a, CRC32, Jenkins), and ECMP load balancing. |
| **BGP Flowspec IPv6** | **BGP Flow Specification for IPv6 (RFC 8956 / RFC 8955)** | AFI 2 / SAFI 133 IPv6 Flowspec NLRI codec, Destination/Source IPv6 prefix matching, Flow Label (RFC 6437), Next Header, Port, and DDoS mitigation drop / rate-limit / redirect actions. |
| **QUIC Datagrams** | **QUIC DATAGRAM Extension & WebTransport (RFC 9221 / RFC 9297)** | Frame Type `0x30` / `0x31` unreliable datagram transport, `max_datagram_frame_size` parameter negotiation, WebTransport Quarter-Stream multiplexing, and drop-oldest / drop-newest congestion control. |
| **Dynamic Routing (IPv6)**| **OSPFv3 for IPv6 (RFC 5340 / RFC 5838)** | Protocol 89 dynamic link-state routing over IPv6 (`ff02::5` / `ff02::6`), Hello packets, Link-LSA (`0x0008`), Intra-Area-Prefix-LSA (`0x2009`), Router-LSA (`0x2001`), and Dijkstra SPF computation. |
| **Zero-Trust Overlays** | **Geneve Micro-segmentation & Group-Based Policy (GBP / SGT) (RFC 8926)** | Geneve Option Class `0x0108` SGT encoding, Source/Destination group classification, and zero-trust micro-segmentation matrix policy engine. |
| **Generic LISP Encap** | **LISP-GPE Multi-Protocol Encapsulation (RFC 9245 / RFC 6830)** | Generic Protocol Extension for LISP over UDP 4341, P/I/V-bit flags, 24-bit Instance ID (VNI), Next Protocol multiplexing (IPv4 `0x01`, IPv6 `0x02`, Ethernet `0x03`, NSH `0x04`), and multi-tenant overlay routing. |
| **EVPN IPv6 Inter-Subnet**| **EVPN Route Type 5 for IPv6 (RFC 9136)** | AFI 2 / SAFI 70 EVPN IPv6 prefix advertisements, 128-bit CIDR matching, Gateway IPv6 overlay indices, 24-bit VNI / MPLS label mapping, and IPv6 VRF prefix RIB. |
| **Carrier Transport OAM** | **MPLS-TP OAM & Generic Associated Channel (G-ACh) (RFC 5860 / RFC 6374 / RFC 5586)** | 4-byte G-ACh header (`0x1` nibble), Loss Measurement (LM) PDU frame counters & loss ratio calculation, Two-Way Delay Measurement (DM) nanosecond timestamping, and BFD direct OAM channel. |
| **5G User Plane Routing** | **SRv6 Mobile User Plane (MUP) Type 1 / Type 2 Routing Architecture (draft-ietf-dmm-srv6-mobile-uplane)** | BGP MUP SAFI 85 NLRI encoding, MUP Type 1 Interwork Segment (MUP-IS) GTP-to-SRv6 SID mapping, MUP Type 2 Direct Segment (MUP-DS) UE prefix routing, and distributed anchor bypass. |
| **SRv6 Dual-Stack Multi-VRF** | **SRv6 End.DT46 Multi-VRF Dual-Stack Routing (RFC 8986 Section 4.15)** | Dual-stack endpoint behavior decapsulating outer IPv6/SRH, inner IP version inspection (IPv4 vs IPv6), and concurrent multi-tenant VRF FIB LPM forwarding. |
| **Service Chaining Overlays** | **Geneve Network Service Header (NSH) SFC Option Co-existence (RFC 8926 / RFC 8300)** | Geneve Option Class `0x0104`, NSH MD Type 1 16-byte Context Header (C1..C4), Service Path ID / Service Index decrement forwarding, and chain egress. |
| **Carrier QoS / Remarking** | **BGP Flowspec IPv6 Action Extended Communities & Remarking Engine (RFC 8956 / RFC 8955)** | Type `0x80` Subtypes `0x06` (Traffic-Rate token bucket), `0x07` (Terminal/Sample action), `0x08` (Redirect to VRF RT), and `0x09` (IPv6 Traffic Class / DSCP rewrite). |
| **Deterministic TSN Interworking** | **Deterministic IP DetNet-to-TSN Sub-Network Mapping & Stream Interworking (RFC 9024 / RFC 9025 / IEEE 802.1CB)** | DetNet IP 5-tuple flow classification to IEEE 802.1 TSN Stream ID, 802.1Q VLAN PCP queue steering, 802.1CB R-TAG generation, and FRER duplicate elimination. |
| **5G MUP Downlink & Session** | **SRv6 Mobile User Plane (MUP) Type 3 / Type 4 Route Extensions (draft-ietf-dmm-srv6-mobile-uplane)** | BGP MUP SAFI 85 Type 3 Downlink Data Plane Prefix Routes (TEID, QFI to SID) and Type 4 Session Notification Routes (PDU Session ID, TAC to SID). |
| **Overlay Congestion Propagation** | **Geneve Explicit Congestion Notification (ECN) & DiffServ Tunneling (RFC 8926 Section 4.5 / RFC 6040)** | RFC 6040 ECN combining matrix on egress, Not-ECT CE-drop safety enforcement, and RFC 2983 Uniform / Pipe DiffServ DSCP tunneling. |
| **Color-Aware SR-TE Steering** | **BGP Color-Aware Extended Community for SR-TE Steering (RFC 9012 / RFC 9256)** | Color Extended Community (`0x030B`), CO-Bits fallback modes (BestEffort, IgpColor, StrictDrop), and (Color, Endpoint) matching into candidate SR Policy paths. |
| **Deterministic MPLS Transport** | **Deterministic IP DetNet MPLS PREOF & Control Word Sub-Layer (RFC 8964 / RFC 8938)** | 4-byte DetNet Control Word (d-CW with `0x0` nibble), S-Label flow demux, F-Label path replication (PRF), and sequence-based elimination (PEF). |
| **High-Accuracy Sub-ns Sync** | **IEEE 1588-2019 / IEEE 802.1AS High-Accuracy PTP Profile & Delay Asymmetry Correction** | Sub-nanosecond time synchronization calculations, Delay Asymmetry TLV (`0x2001`), scaled nanosecond / picosecond time intervals, and physical port calibration. |
| **SRv6 IPv6-Only Multi-VRF** | **SRv6 End.DT6 Multi-VRF IPv6-Only Routing (RFC 8986 Section 4.14)** | Dedicated IPv6-only VRF decapsulation endpoint behavior enforcing inner IPv6 header verification, multi-tenant VRF isolation, and LPM FIB forwarding. |
| **EVPN L3 Fast Mass-Withdrawal** | **EVPN Layer 3 ESI Fast Mass-Withdrawal for Type 5 IP Prefix Routes (RFC 9136 / RFC 7432)** | Sub-millisecond failover for Type 5 IP Prefix routes triggered by Type 1 (Auto-Discovery per-ES) withdrawal, instantaneous atomic table update. |
| **Geneve Path MTU Discovery** | **Geneve Path MTU Discovery & Active Flow Probe Option (RFC 8926 Section 4.4 / RFC 1191)** | In-band active Path MTU probing and bottleneck discovery through Geneve Option Class `0x0109`, probe sequence tracking, and reply reflection. |
| **Deterministic IP-over-MPLS** | **Deterministic IP DetNet IP-to-MPLS Sub-Layer Mapping & TC Marking (RFC 8964 / RFC 9024)** | Ingress 5-tuple IP flow classification to dual-tier DetNet MPLS (S-Label + d-CW + F-Label), DSCP-to-TC marking, PREOF replication, and egress deduplication. |
| **5G SRv6 MUP Handover** | **SRv6 Mobile User Plane (MUP) Session Handover & State Machine (draft-ietf-dmm-srv6-mobile-uplane)** | 5G PDU session mobility state machine (`Active`, `Preparing`, `Executing`, `Completed`), buffered in-flight forwarding, and atomic SID re-binding. |
| **Datacenter L2 Flowspec** | **BGP Flowspec Layer 2 Matching Component Attributes (RFC 8955 / draft-ietf-idr-flowspec-l2vpn)** | Layer 2 Ethernet frame classification (Source/Destination MAC, EtherType, VLAN ID, 802.1p PCP, QinQ Inner VLAN), priority rules, and policy actions. |
| **PTP Fiber Delay Compensation** | **PTP Optical Fiber Delay Dispersion & Thermal Drift Compensation (IEEE 1588-2019 / ITU-T G.8275.1)** | Temperature-dependent optical fiber propagation delay drift (TCD) and chromatic dispersion asymmetry modeling (G.652/G.655) with picosecond precision. |
| **PTP Telecom Grandmaster Quality** | **PTP Telecom Grandmaster (T-GM) Clock Class & GNSS Holdover Aging (ITU-T G.8275.1 / IEEE 1588-2019)** | Grandmaster GNSS lock tracking, oscillator holdover aging (Rubidium/OCXO/TCXO), and automated Clock Class step-down (Class 6 $\to$ 7 $\to$ 14 $\to$ 15 $\to$ 165 $\to$ 248). |
| **SRv6 L2 VLAN Normalization** | **SRv6 End.DX2 & End.DX2V Endpoint with VLAN Manipulation & Normalization (RFC 8986 §4.11 / §4.12)** | Layer 2 cross-connect decapsulation, customer AC egress forwarding, and flexible VLAN rewrite actions (Raw, Pop, Push, Swap, and standardized QinQ Normalization). |
| **EVPN E-Tree Access Filtering** | **EVPN Layer 2 E-Tree Ingress/Egress Filtering & BUM Split-Horizon Replication (RFC 8317 Section 5 & 6)** | Root/Leaf tenant isolation enforcement, Leaf-to-Leaf Known Unicast blocking, and BUM split-horizon selective replication across access ACs and overlay VTEPs. |
| **Geneve EVC Multiplexing** | **Geneve Layer-2 Ethernet Virtual Circuit (EVC) Multiplexing & Service Mapping (RFC 8926 / MEF 6.2)** | Multi-tenant Carrier Ethernet E-Line / E-LAN service mapping over Geneve tunnels (UDP 6081) with CE-VLAN translation and EVC Metadata option headers. |
| **EVPN Point-to-Point VPWS** | **EVPN Flexible Cross-Connect (FXC) Point-to-Point VPWS Mode (RFC 8214)** | EVPN VPWS Attachment Circuit (AC) point-to-point cross-connect over MPLS/SR, Layer 2 Attributes Extended Community (C-bit / P-bit / B-bit), and MTU validation. |
| **DetNet Delay & Jitter Budget** | **Deterministic IP DetNet Bounded Jitter & End-to-End Latency Budget Calculator (RFC 8939 / RFC 9024)** | Deterministic latency bounds ($D_{\min}, D_{\max}$), worst-case end-to-end jitter ($J_{\text{e2e}}$), PREOF differential path skew, and PEF de-jitter elimination buffer sizing. |
| **PTP PHY Asymmetry Compensation** | **PTP IEEE 1588 Physical Layer (PHY) Asymmetric Delay Compensation (IEEE 1588-2019 Clause 9.5.4)** | Sub-nanosecond PHY Tx/Rx pipeline serialization delay calibration, static fiber asymmetry adjustment, and calibrated correctionField computation. |
| **SRv6 MUP 5QI QoS Flow Mapping** | **SRv6 Mobile User Plane (MUP) 5QI-to-DSCP QoS Flow Mapping (3GPP TS 23.501 / draft-ietf-dmm-srv6-mobile-uplane)** | 5G Standardized 5QIs (1..9, 65..86) mapping to DSCP, IPv6 Traffic Class bytes, and SRv6 color attributes for network slice SLA enforcement. |
| **EVPN S-PMSI Selective Multicast** | **EVPN Selective Multicast (S-PMSI) Trees & Leaf A-D Tracking (RFC 9572 / RFC 6514 §4.2)** | Route Type 6 (S-PMSI A-D) and Route Type 7 (Leaf A-D) route distribution, P-Tunnel Attribute (PTA), dynamic traffic rate threshold promotion, and selective tree replication. |
| **DetNet Schedulability Analysis** | **Deterministic IP DetNet Schedulability & Over-Provisioning Analysis Engine (RFC 9024 §4 / IEEE 802.1Qbv)** | Multi-hop deterministic schedulability, over-provisioning factor ($\alpha_{\text{over}}$), worst-case queue backlog bounds, and zero packet loss ($P_{\text{loss}} = 0$) SLA admission control. |
| **SRv6 End.DT2U L2 Unicast** | **SRv6 End.DT2U Layer-2 EVPN Unicast Lookup & Forwarding (RFC 8986 §4.13)** | SRv6 Endpoint Behavior `End.DT2U` decapsulation, tenant MAC-VRF FIB lookup, Attachment Circuit (AC) egress forwarding, and configurable unknown unicast flooding policies. |
| **PTP PDV Floor Filter** | **PTP Packet Delay Variation (PDV) Floor Filter & Min-Delay Estimator (ITU-T G.8275.2 / IEEE 1588 Annex C)** | Sliding-window min-delay floor selection, queuing jitter rejection, outlier filtering, and stable phase offset estimation across packet networks with partial timing support. |
| **Routing** | **Routing Table (LPM)** | Longest Prefix Match (LPM) route lookup engine, CIDR netmask matching, default gateway and on-link subnet resolution. |
| **Simulation** | **Virtual Network Bus** | Multi-node switched LAN simulator connecting multiple `NetStack` hosts over virtual Ethernet links. |
| **Interface** | **Interactive Network Shell** | Interactive CLI REPL with 218 commands including `tsn-cqf-max-sdu`, `diameter-s13-lease`, `evpn-mcast-policer`, `gtpu-flow-entropy`, `tsn-cqf-reassembly`, `diameter-s13-range`, `evpn-snoop-filter`, `gtpu-sliding-ack`, `tsn-cqf-splice`, `diameter-s13-exempt`, `evpn-querier-elect`, `gtpu-hole-nack`, `tsn-cqf-plane`, `diameter-s13-escn`, `evpn-join-suppress`, `gtpu-reorder-flush`, `tsn-cqf-burst`, `diameter-s13-roam`, `evpn-explicit-track`, `gtpu-dnn-demux`, `tsn-cqf-frer`, `diameter-s13-tamper`, `evpn-ssm-sa`, `gtpu-bearer-map`, `diameter-s13-geofence`, `evpn-ssm-dr`, `gtpu-qos-marking`, `tsn-cqf-slot`, `diameter-s13-ocp`, `evpn-ssm-underlay`, `gtpu-flow-reanchor`, `tsn-cqf-ring-align`, `diameter-s13-imeidb`, `evpn-ssm`, `gtpu-rtt-dup`, `tsn-cqf-preempt`, `diameter-s13-bulk`, `evpn-dht`, `gtpu-gap-retransmit`, `tsn-cqf-inherit`, `diameter-s13-cache`, `evpn-umrt`, `gtpu-jitter-buf`, `tsn-cqf-analyzer`, `tsn-cqf-promote`, `diameter-s6a-nor`, `evpn-dai`, `gtpu-link-agg`, `tsn-cqf-deficit`, `diameter-s6a-clr`, `evpn-dhcp-snoop`, `gtpu-rtt-smooth`, `tsn-cqf-jitter`, `diameter-s6a-uar`, `evpn-anti-spoof`, `gtpu-atsss-split`, `diameter-s6a-rsr`, `evpn-mac-freeze`, `gtpu-loss`, `tsn-cqf-gate`, `diameter-s6a-pur`, `evpn-uu-filter`, `gtpu-rtt-var`, `tsn-cqf-scale`, `diameter-s6a-idr`, `evpn-uu-rate`, `gtpu-dynamic-echo`, `tsn-cqf-deadline`, `diameter-s13-gray`, `evpn-port-sec`, `gtpu-probe`, `tsn-cqf-offset`, `diameter-s6m`, `evpn-umt`, `gtpu-jitter`, `tsn-cqf-meter`, `diameter-s6t`, `evpn-pvlan`, `gtpu-redundant`, `tsn-cqf-time`, `diameter-np`, `evpn-damp`, `gtpu-ma`, `tsn-preempt`, `diameter-s6c`, `evpn-core-iso`, `gtpu-failover`, `tsn-qav`, `diameter-zh`, `evpn-bum`, `gtpu-qos`, `tsn-qbv-reconfig`, `diameter-sgd`, `evpn-irb`, `gtpu-reloc`, `tsn-ats-multi`, `diameter-s13p`, `evpn-mcast-ir`, `gtpu-reorder`, `tsn-qcz`, `evpn-flush`, `gtpu-heartbeat`, `tsn-psfp`, `diameter-swm`, `tsn-cqf`, `diameter-s6b`, `evpn-frr`, `srv6-mup-direct`, `frer-srf`, `diameter-cx`, `evpn-mobility`, `gtpc-v2`, `tsn-qbv`, `diameter-slh`, `evpn-uu`, `geneve-telemetry`, `gtpu-telemetry`, `ptp-ttc`, `diameter-sh`, `evpn-vrf-leak`, `ptp-te`, `diameter-s9`, `evpn-snooping`, `flowspec-vrf`, `evpn-pref-df`, `ifa`, `diameter-s13`, `ptp-bc`, `synce`, `diameter-s6a`, `evpn-etree`, `srv6-slicing`, `mldp`, `diameter-gx`, `evpn-mass-withdraw`, `sr-oam`, `pim-bsr`, `diameter-rx`, `evpn-proxy-arp`, `nsh-md2`, `add-path`, `evpn-synch`, `detnet`, `diameter-charging`, `flowspec`, `otlp`, `gre6`, `ioam`, `netconf`, `lisp`, `wireguard`, `gptp`, `pcep`, `rsvp`, `openflow`, `diameter`, `nsh`, `sflow`, `6in4`, `4in6`, `roce`, `pfc`, `gue`, `evpn`, `dhcpv6`, `vxlan-gpe`, `vtp`, `ldp`, `glbp`, `tacacs`, `turn`, `gtp`, `hsrp`, `cdp`, `srv6`, `stun`, `rtp`, `ptp`, `erspan`, `mqtt`, `coap`, `sctp`, `ldap`, `netflow`, `sip`, `bfd`, `geneve`, `isis`, `syslog`, `l2tp`, `pim`, `radius`, `pppoe`, `eigrp`, `ping`, `ping6`, `traceroute`, `ntp`, `tftp`, `snmp`, `ospf`, `ipsec`, `http3`, `lacp`, `stp`, `vxlan`, `mpls`, `bgp`, `lldp`, `quic`, `vrrp`, `ndp`, `rip`, `tunnel`, `igmp`, `tls`, `http2`, `ws`, `dns`, `curl`, `udp send`, `arp`, `route`, `netstat`, `iptables`, `nat`, `tcp-stats`, and live PCAP recording. |

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
│   ├── tsn_cqf_max_sdu_enforcer.rs # TSN / Real-Time: IEEE 802.1Qch/802.1Qci Max-SDU Enforcement & Cyclic Truncation
│   ├── diameter_s13_tac_whitelist_expiry.rs # Carrier: 3GPP TS 29.272 S13 TAC Whitelist & Lease Expiry Engine
│   ├── evpn_igmp_rate_limit_policer.rs # Datacenter: EVPN Layer 2 Multicast Rate Limiter & Storm Policer (RFC 9251)
│   ├── gtpu_flow_label_entropy.rs # 5G Core: 3GPP TS 29.281 / RFC 6437 GTP-U IPv6 Flow Label Entropy & ECMP
│   ├── tsn_cqf_path_splice.rs # TSN / Real-Time: IEEE 802.1Qch CQF Dynamic Path Splicing & Hitless Switchover
│   ├── diameter_s13_emergency_exemption.rs # Carrier: 3GPP TS 29.272 S13 Emergency Call & eCall IMEI Exemption
│   ├── evpn_igmp_querier_election.rs # Datacenter: EVPN IGMP Snooping Querier Election (RFC 9251 / RFC 2236)
│   ├── gtpu_hole_nack.rs      # 5G Core: 3GPP TS 29.281 / TS 38.415 GTP-U Sequence Hole & Proactive NACK
│   ├── tsn_cqf_frame_reassembly.rs # TSN / Real-Time: IEEE 802.1Qch/802.1Qbu Fragment Reassembly & Cyclic Dispatch
│   ├── diameter_s13_imei_range.rs  # Carrier: 3GPP TS 29.272 S13 TAC / IMEI-SV Range Matching Engine
│   ├── evpn_igmp_mld_snooping_filter.rs # Datacenter: EVPN Layer 2 Multicast Snooping Group Boundary Filter & CAC
│   ├── gtpu_sliding_window_ack.rs  # 5G Core: 3GPP TS 29.281 / TS 38.415 GTP-U Sliding Window ACK & SACK Engine
│   ├── tsn_cqf_dual_plane.rs  # TSN / Real-Time: IEEE 802.1Qch CQF Dual-Plane Redundancy & Failover
│   ├── diameter_s13_escn.rs   # Carrier: 3GPP TS 29.272 S13 Equipment Status Change Notification (ESCN)
│   ├── evpn_igmp_join_suppress.rs # Datacenter: EVPN IGMPv3 Join Suppression & Proxy Reporting (RFC 9251)
│   ├── gtpu_reorder_flush.rs  # 5G Core: 3GPP TS 29.281 GTP-U Sequence Reordering & Early Flush
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
│   ├── bgp_router.rs      # Layer 3: BGP-4 speaker - FSM, timers, sockets on port 179, FIB installation,
│   │                      #          RFC 4456 route reflection & RFC 4271 6.8 collision resolution
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
│   ├── tsn_qbv_gcl.rs     # TSN Scheduling: IEEE 802.1Qbv Time-Aware Shaper (TAS) Gate Control List Engine
│   ├── tsn_qbv_reconfig.rs # TSN Scheduling: IEEE 802.1Qbv Dynamic GCL Reconfiguration & Hitless Admin/Oper Swap
│   ├── tsn_cqf_multicycle.rs # TSN Queuing: IEEE 802.1Qch Multi-Queue Cyclic Queuing & Forwarding (CQF)
│   ├── tsn_cqf_time_dispatch.rs # TSN Queuing: IEEE 802.1Qch CQF Time-Synchronized Dispatch Engine
│   ├── tsn_cqf_trtcm.rs   # TSN Metering: IEEE 802.1Qch CQF with trTCM Traffic Metering (RFC 2698)
│   ├── tsn_cqf_offset.rs  # TSN Alignment: IEEE 802.1Qch CQF Multi-Hop Phase Offset Alignment Engine
│   ├── tsn_cqf_deadline.rs # TSN Protection: IEEE 802.1Qch CQF Deadline & Buffer Overrun Engine
│   ├── tsn_cqf_cycle_scale.rs # TSN Scaling: IEEE 802.1Qch CQF Dynamic Cycle Duration Scaling Engine
│   ├── tsn_cqf_gate_coord.rs # TSN Gate Coord: IEEE 802.1Qch CQF Multi-Priority Gate Coordination Engine
│   ├── tsn_cqf_jitter_bound.rs # TSN Jitter Bound: IEEE 802.1Qch CQF Multi-Hop Delay & Jitter Predictor
│   ├── tsn_cqf_deficit_meter.rs # TSN Deficit: IEEE 802.1Qch CQF Ingress Deficit Metering Engine
│   ├── tsn_cqf_prio_promote.rs # TSN Promotion: IEEE 802.1Qch CQF Priority Promotion & Preemption Fallback
│   ├── tsn_cqf_prio_inherit.rs # TSN Inheritance: IEEE 802.1Qch CQF Priority Inheritance & Inversion Defense
│   ├── tsn_cqf_gate_preempt.rs # TSN Preemption: IEEE 802.1Qch CQF Gate Preemption Interlocking Engine
│   ├── tsn_cqf_ring_align.rs # TSN Alignment: IEEE 802.1Qch CQF Stream Redundancy & Dual-Ring Cyclic Alignment
│   ├── tsn_cqf_slot_reservation.rs # TSN Reservation: IEEE 802.1Qch CQF Time-Slot Dynamic Bandwidth Reservation Engine
│   ├── tsn_cqf_frame_replication.rs # TSN Replication: IEEE 802.1Qch CQF Frame Replication & FRER Elimination Engine
│   ├── tsn_cqf_burst_absorb.rs # TSN Burst: IEEE 802.1Qch CQF Cyclic Burst Absorption & Multi-Cycle Leaky-Bucket
│   ├── tsn_cqf_dual_plane.rs   # TSN Redundancy: IEEE 802.1Qch CQF Dual-Plane Redundancy & Active-Passive Gate Coordination
│   ├── tsn_cqf_timestamp_jitter.rs # TSN Jitter Analyzer: IEEE 802.1Qch CQF Cyclic Frame Timestamping
│   ├── tsn_ats_multihop.rs # TSN Shaping: IEEE 802.1Qcr Asynchronous Traffic Shaping (ATS) Multi-Hop Pipeline
│   ├── tsn_qav_cbs.rs     # TSN Shaping: IEEE 802.1Qav Credit-Based Shaper (CBS) Multi-Class AVB Engine
│   ├── tsn_guard_band.rs  # TSN Preemption: IEEE 802.1Qbu / 802.3br Frame Preemption & Qbv Dynamic Guard Band
│   ├── tsn_psfp_stream_filter.rs # TSN Policing: IEEE 802.1Qci PSFP trTCM Multi-Stage Stream Filter Engine
│   ├── tsn_qcz_congestion.rs # TSN Isolation: IEEE 802.1Qcz Congestion Isolation & HoL Blocking Mitigation Engine
│   ├── frer_srf.rs        # TSN Zero-Loss: IEEE 802.1CB Sequence Recovery Function (SRF) Vector Algorithm
│   ├── diameter_slh.rs    # 5G / Location: Diameter SLh LCS Location Services GMLC-HSS Interface (3GPP TS 29.173)
│   ├── diameter_cx.rs     # 5G / IMS Core: Diameter Cx/Dx IMS I/S-CSCF to HSS Registration Interface (3GPP TS 29.228)
│   ├── diameter_s6a_idr.rs # 5G / HSS-to-MME: Diameter S6a Insert-Subscriber-Data (IDR/IDA) Interface (3GPP TS 29.272)
│   ├── diameter_s6a_nor.rs # 5G / HSS-to-MME: Diameter S6a Notify (NOR/NOA) Interface (3GPP TS 29.272)
│   ├── diameter_s6a_pur.rs # 5G / HSS-to-MME: Diameter S6a Purge-UE (PUR/PUA) Interface (3GPP TS 29.272)
│   ├── diameter_s6a_rsr.rs # 5G / HSS-to-MME: Diameter S6a Reset (RSR/RSA) Interface (3GPP TS 29.272)
│   ├── diameter_s6a_uar.rs # 5G / HSS-to-MME: Diameter S6a User-Authorization (UAR/UAA) Interface (3GPP TS 29.272)
│   ├── diameter_s6a_clr.rs # 5G / HSS-to-MME: Diameter S6a Cancel-Location (CLR/CLA) Interface (3GPP TS 29.272)
│   ├── diameter_s6b.rs    # 5G / WLAN Access: Diameter S6b Untrusted WLAN / ePDG AAA Interface (3GPP TS 29.273)
│   ├── diameter_swm.rs    # 5G / WLAN AAA: Diameter SWm / SWx Untrusted WLAN ePDG EAP-AKA' AAA Interface (3GPP TS 29.273)
│   ├── diameter_s13_prime.rs # 5G / EIR Check: Diameter S13' Direct EIR Interface & IMEI-SV Verification (3GPP TS 29.272)
│   ├── diameter_s13_cache.rs # 5G / EIR Cache: Diameter S13 Local EIR Cache & TTL Expiry (3GPP TS 29.272)
│   ├── diameter_s13_bulk.rs # 5G / EIR Bulk: Diameter S13 Bulk IMEI Blacklist Push (3GPP TS 29.272)
│   ├── diameter_s13_imeidb.rs # 5G / GSMA IMEIDB: Diameter S13 Global GSMA IMEI-DB Query Interface (3GPP TS 29.272)
│   ├── diameter_s13_ocp.rs # 5G / EIR Overload: Diameter S13 Overload Control & Throttling (RFC 7683 DOIC)
│   ├── diameter_s13_geo_fence.rs # 5G / Geofencing: Diameter S13 Geofencing & Cell-ID Anomaly Detection (3GPP TS 29.272)
│   ├── diameter_s13_imei_tamper.rs # 5G / IMEI Tamper: Diameter S13 Hardware IMEI-SV Luhn & Tamper Validation (3GPP TS 29.272)
│   ├── diameter_s13_roam_mismatch.rs # 5G / EIR Roam: Diameter S13 Roaming TAC Country Mismatch & Risk Scoring (3GPP TS 29.272)
│   ├── diameter_s13_escn.rs   # 5G / EIR ESCN: Diameter S13 Equipment Status Change Notification & Edge Sync (3GPP TS 29.272)
│   ├── diameter_s13_graylist.rs # 5G / EIR Graylist: Diameter S13 Equipment Check & Throttling (3GPP TS 29.272)
│   ├── diameter_sgd.rs    # 5G / SMS Core: Diameter SGd / T4 SMS Core Relay Interface (3GPP TS 29.338)
│   ├── diameter_s6c.rs    # 5G / SMS Routing: Diameter S6c SMS Routing & Delivery Status Interface (3GPP TS 29.338)
│   ├── diameter_np.rs     # 5G / RCAF: Diameter Np RAN User Plane Congestion Reporting Interface (3GPP TS 29.217)
│   ├── diameter_s6t.rs    # 5G / SCEF: Diameter S6t SCEF to HSS Cellular IoT Interface (3GPP TS 29.336)
│   ├── diameter_s6m.rs    # 5G / SMS-IWMSC: Diameter S6m / S6n MAP-to-Diameter HSS Interface (3GPP TS 29.336)
│   ├── diameter_zh.rs     # 5G / GAA: Diameter Zh BSF-to-HSS Bootstrapping Interface & NAF Keys (3GPP TS 29.109)
│   ├── evpn_uu_suppression.rs # Datacenter EVPN: Layer 2 Unknown Unicast Flood Suppression & Storm Control (RFC 7432)
│   ├── evpn_uu_ratelimit.rs # Datacenter EVPN: Layer 2 Unknown Unicast Storm Suppression Rate-Limiter (RFC 7432 Section 16)
│   ├── evpn_uu_egress_filter.rs # Datacenter EVPN: Layer 2 Unknown Unicast Egress Horizon & ESI Pruning (RFC 7432)
│   ├── evpn_mac_mobility.rs # Datacenter EVPN: MAC Mobility Sequence Tracking & Sticky MAC Suppression (RFC 7432)
│   ├── evpn_mac_freeze.rs # Datacenter EVPN: Layer 2 MAC Address Mobility Freeze & Move Damping (RFC 7432)
│   ├── evpn_ip_anti_spoof.rs # Datacenter EVPN: Layer 2 ARP/ND Snooping & IP Anti-Spoofing (RFC 7432/9136)
│   ├── evpn_dhcp_snooping.rs # Datacenter EVPN: Layer 2 DHCP Snooping & Option 82 Relay Engine (RFC 7432/3046)
│   ├── evpn_dai_inspection.rs # Datacenter EVPN: Layer 2 Dynamic ARP Inspection & Rate-Limiter (RFC 7432)
│   ├── evpn_dht_probe.rs  # Datacenter EVPN: Dynamic Host Tracking & Silent Host Probing (RFC 7432)
│   ├── evpn_ssm_snooping.rs # Datacenter EVPN: Source-Specific Multicast (SSM) Snooping & SMET (RFC 7432/9251)
│   ├── evpn_ssm_dr_election.rs # Datacenter EVPN: SSM Designated Router (DR) Election & Querier Sync (RFC 8584/9251)
│   ├── evpn_ssm_source_active.rs # Datacenter EVPN: SSM Source Active (SA) Route Synchronization (RFC 9251)
│   ├── evpn_ssm_underlay.rs # Datacenter EVPN: Selective Multicast Underlay Provider P-Tree Mapping (RFC 6514/9251)
│   ├── evpn_igmp_explicit_tracking.rs # Datacenter EVPN: IGMPv3 Explicit Host Tracking & O(1) Fast Leave Engine (RFC 7432/9251)
│   ├── evpn_igmp_join_suppress.rs # Datacenter EVPN: Layer 2 IGMPv3 Join Suppression & Proxy Reporting Engine (RFC 7432/9251)
│   ├── evpn_umrt_prune.rs # Datacenter EVPN: Unknown Multicast Replication Tree (UMRT) & Pruning (RFC 9251)
│   ├── evpn_frr_protection.rs # Datacenter EVPN: Fast Reroute (FRR) & Secondary Nexthop Path Protection (RFC 7432)
│   ├── evpn_mac_flush.rs  # Datacenter EVPN: Layer 2 MAC Flush on Link/Port Down (RFC 7432 / RFC 8317)
│   ├── evpn_multicast_ir.rs # Datacenter EVPN: Selective Multicast Ingress Replication & Leaf Pruning (RFC 9251)
│   ├── evpn_irb_anycast.rs # Datacenter EVPN: Layer 3 Anycast Gateway & Symmetric/Asymmetric IRB Engine (RFC 9135/9136)
│   ├── evpn_bum_policer.rs # Datacenter EVPN: Layer 2 BUM Traffic Storm Policer & Quarantine (RFC 7432)
│   ├── evpn_core_isolation.rs # Datacenter EVPN: Layer 2 Core Isolation Defense & Split-Horizon Group (RFC 7432)
│   ├── evpn_flap_damping.rs # Datacenter EVPN: Layer 2 Port Flap Damping & Route Dampening (RFC 7432 Section 16)
│   ├── evpn_pvlan.rs      # Datacenter EVPN: Layer 2 Private VLAN (PVLAN) & Port Isolation (RFC 5517)
│   ├── evpn_umt_ir.rs     # Datacenter EVPN: Unknown Multicast Tree (UMT) & Ingress Replication Optimization (RFC 9251)
│   ├── evpn_port_security.rs # Datacenter EVPN: Layer 2 Dynamic Port Security & Sticky MAC Aging (RFC 7432 Section 15)
│   ├── geneve_telemetry_opt.rs # Datacenter Overlay: Geneve In-Band Network Telemetry INT Option (RFC 8926)
│   ├── gtpc_v2.rs         # 5G / 4G Core: 3GPP GTPv2-C Control Plane Session Management & SGW Engine (3GPP TS 29.274)
│   ├── gtpu_atsss_split.rs # 5G User Plane: 3GPP ATSSS Dynamic Packet Splitting Engine (TS 23.501)
│   ├── gtpu_bearer_qos_flow_map.rs # 5G User Plane: 3GPP 5G-to-4G Bearer ID (EBI) to QoS Flow (QFI) Map (TS 29.281/23.501)
│   ├── gtpu_network_instance_demux.rs # 5G User Plane: 3GPP GTP-U Network Instance & Multi-Tenancy DNN Demux Engine (TS 29.281)
│   ├── gtpu_reorder_flush.rs # 5G User Plane: 3GPP GTP-U Sequence Reordering Buffer & Early Flush Engine (TS 29.281)
│   ├── gtpu_heartbeat.rs  # 5G User Plane: 3GPP GTP-U Path Management Echo Heartbeat & Failure Detection (TS 29.281)
│   ├── gtpu_dynamic_echo.rs # 5G User Plane: 3GPP GTP-U Adaptive Heartbeat & Fast Probing Engine (TS 29.281)
│   ├── gtpu_reordering.rs # 5G User Plane: 3GPP GTP-U Sequence Out-of-Order Reordering & Jitter Buffer (TS 29.281)
│   ├── gtpu_upf_relocation.rs # 5G User Plane: 3GPP GTP-U UPF Relocation, Indirect Forwarding & End Marker (TS 23.501)
│   ├── gtpu_qos_enforcer.rs # 5G User Plane: 3GPP GTP-U QoS Flow Identifier & Session-AMBR Rate Limiter (TS 38.415)
│   ├── gtpu_qos_marking.rs # 5G User Plane: 3GPP GTP-U Outer IP DSCP & 802.1p PCP Dynamic QoS Marking (TS 38.415)
│   ├── gtpu_fast_failover.rs # 5G User Plane: 3GPP GTP-U Path Loss Detection & Fast Failover Engine (TS 23.501)
│   ├── gtpu_gap_retransmit.rs # 5G User Plane: 3GPP GTP-U Gap Detection & Fast Retransmit (TS 29.281/38.415)
│   ├── gtpu_link_agg.rs   # 5G User Plane: 3GPP GTP-U Multi-Link Flow Distribution & Aggregation Engine (TS 29.281)
│   ├── gtpu_jitter_buf.rs # 5G User Plane: 3GPP GTP-U Path RTT-Adaptive Jitter Buffer Engine (TS 29.281)
│   ├── gtpu_rtt_dup.rs    # 5G User Plane: 3GPP GTP-U RTT-Adaptive Packet Duplication Engine (TS 29.281/23.501)
│   ├── gtpu_flow_reanchor.rs # 5G User Plane: 3GPP GTP-U Flow Re-Anchoring & Migration Engine (TS 29.281/23.501)
│   ├── gtpu_ma_pdu.rs     # 5G User Plane: 3GPP GTP-U MA-PDU Session & ATSSS Traffic Steering Engine (TS 23.501)
│   ├── gtpu_redundant_paths.rs # 5G User Plane: 3GPP GTP-U Redundant Transmission & Deduplication (TS 23.501)
│   ├── gtpu_jitter_telemetry.rs # 5G User Plane: 3GPP GTP-U Path Jitter & OWD Telemetry Engine (TS 38.415)
│   ├── gtpu_loss_telemetry.rs # 5G User Plane: 3GPP GTP-U In-Band Packet Loss Telemetry Engine (TS 38.415)
│   ├── gtpu_rtt_probing.rs # 5G User Plane: 3GPP Multi-Access Active Latency Probing Engine (TS 24.193)
│   ├── gtpu_rtt_smooth.rs # 5G User Plane: 3GPP GTP-U Path RTT Dual-EMA Smoothing Engine (TS 29.281)
│   ├── gtpu_rtt_variance.rs # 5G User Plane: 3GPP GTP-U RTT Variance & Adaptive RTO Engine (TS 29.281)
│   ├── srv6_mup_interworking.rs # 5G / SRv6: Mobile User Plane (MUP) End.M.GTP6.D/E Interworking Engine
│   ├── gtpu_telemetry.rs  # 5G User Plane: GTP-U PDU Session Container Extension & In-Band Delay Telemetry (3GPP TS 38.415)
│   ├── ptp_telecom_tc.rs  # Timing / 5G: PTP Telecom Peer-to-Peer Transparent Clock (T-TC) Engine (ITU-T G.8275.2)
│   ├── diameter_sh.rs     # 5G / IMS Core: Diameter Sh IMS Application Server to HSS Interface (3GPP TS 29.328)
│   ├── evpn_vrf_leaking.rs # Datacenter EVPN: Layer 3 Multi-VRF Route Leaking & Shared Services (RFC 9136)
│   ├── ptp_time_error.rs  # Timing / 5G: PTP Telecom Time Error cTE/dTE Measurement & Mask Verification (ITU-T G.8273.2)
│   ├── diameter_s9.rs     # 5G / 4G Core: Diameter S9 PCRF Roaming Policy Coordination Interface (3GPP TS 29.215)
│   ├── evpn_igmp_snooping.rs # Datacenter EVPN: Layer 2 IGMP Snooping & Multicast Tree Pruning (RFC 9251)
│   ├── flowspec_redirect_vrf.rs # BGP / DDoS: BGP Flowspec Redirect-to-VRF & DSCP Traffic Marking (RFC 8955)
│   ├── evpn_pref_df.rs    # Datacenter EVPN: Preference-Based DF Election & Non-Preempt/Sticky (RFC 8584)
│   ├── ifa_telemetry.rs   # In-Band Telemetry: In-situ Flow Analytics IFA 2.0 Hop-by-Hop Telemetry (RFC 9197)
│   ├── diameter_s13.rs    # 5G / 4G Core: Diameter S13 / S13' EIR Equipment Identity Register (3GPP TS 29.272)
│   ├── ptp_telecom_bc.rs  # Timing / 5G: PTP Telecom Boundary Clock (T-BC) Alternate BMCA Engine (ITU-T G.8275.1)
│   ├── synce_esmc.rs      # 5G Fronthaul / Timing: SyncE ESMC Quality Level SSM & Physical Clock Selection (ITU-T G.8264)
│   ├── diameter_s6a.rs    # 5G / 4G Core: Diameter S6a Interface & HSS EPS Authentication Vectors (3GPP TS 29.272)
│   ├── evpn_etree.rs      # Datacenter EVPN: E-Tree Root/Leaf Service & Split-Horizon Forwarding (RFC 8317)
│   ├── srv6_slicing.rs    # Segment Routing / 5G: SRv6 Network Slicing & Flex-Algo VTN SLA Paths (RFC 9350 / RFC 9543)
│   ├── mldp.rs            # Multicast MPLS: Multipoint LDP P2MP/MP2MP Multicast Tree Replication (RFC 6388 / RFC 6513)
│   ├── diameter_gx.rs     # 5G / EPC Policy: Diameter Gx Policy & Charging Control PCEF Engine (3GPP TS 29.212)
│   ├── evpn_mass_withdraw.rs # Datacenter EVPN: Route Type 1 per-ES Mass Withdrawal Fast Convergence (RFC 7432)
│   ├── sr_mpls_oam.rs     # Carrier OAM: Segment Routing MPLS LSP Ping & Target FEC Stack Sub-TLVs (RFC 8287)
│   ├── pim_bsr.rs         # Multicast: PIM-BSR Dynamic RP Election & PIM-SSM 232.0.0.0/8 (RFC 5059 / RFC 4607)
│   ├── diameter_rx.rs     # 5G / IMS Policy: Diameter Rx Policy and Charging Control (3GPP TS 29.214 / PCRF)
│   ├── evpn_proxy_arp.rs  # Datacenter EVPN: Proxy ARP/ND Broadcast Suppression & Anycast Gateway (RFC 7432 / RFC 9135)
│   ├── nsh_md2.rs         # SFC / Overlays: NSH MD Type 2 Dynamic Variable-Length Context TLVs (RFC 8300 Section 3.5.2)
│   ├── bgp_add_path.rs    # Carrier / BGP: BGP ADD-PATH Capability 69 & Prefix Independent Convergence (RFC 7911/8277)
│   ├── evpn_synch.rs      # Carrier / EVPN: Route Type 7/8 Multicast Join & Leave Synchronization (RFC 9251)
│   ├── detnet.rs          # TSN / Deterministic: DetNet Control Word & PREF Elimination Filter (RFC 8655/8938/8939/8964)
│   ├── diameter_charging.rs # 5G Core / AAA: Diameter Gy/Ro Credit-Control & OCS Quota Engine (RFC 4006 / 3GPP TS 32.299)
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
│   ├── test_tsn_cqf_path_splice.rs # IEEE 802.1Qch CQF Dynamic Path Splicing & Switchover tests
│   ├── test_diameter_s13_emergency_exemption.rs # Diameter S13 Emergency Call Exemption tests
│   ├── test_evpn_igmp_querier_election.rs # EVPN IGMP Snooping Querier Election tests
│   ├── test_gtpu_hole_nack.rs # 5G GTP-U Sequence Hole & Proactive NACK tests
│   ├── test_tsn_cqf_frame_reassembly.rs # IEEE 802.1Qch/802.1Qbu Fragment Reassembly tests
│   ├── test_diameter_s13_imei_range.rs  # 3GPP TS 29.272 S13 TAC Range Matching tests
│   ├── test_evpn_igmp_mld_snooping_filter.rs # EVPN IGMP/MLD Snooping Group Boundary Filter tests
│   ├── test_gtpu_sliding_window_ack.rs  # 5G GTP-U Sliding Window ACK & SACK tests
│   ├── test_tsn_cqf_dual_plane.rs # IEEE 802.1Qch CQF Dual-Plane Redundancy tests
│   ├── test_diameter_s13_escn.rs # Diameter S13 Equipment Status Change Notification tests
│   ├── test_evpn_igmp_join_suppress.rs # EVPN IGMPv3 Join Suppression & Proxy Reporting tests
│   ├── test_gtpu_reorder_flush.rs # 5G GTP-U Sequence Reordering & Early Flush tests
│   ├── test_tsn_cqf_ring_align.rs # IEEE 802.1Qch CQF Dual-Ring Alignment tests
│   ├── test_diameter_s13_ocp.rs # Diameter S13 EIR Overload Control & Throttling tests
│   ├── test_evpn_ssm_underlay.rs # EVPN Selective Multicast Underlay P-Tree tests
│   ├── test_gtpu_flow_reanchor.rs # 5G GTP-U Flow Re-Anchoring & Migration tests
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
│   ├── test_bgp_route_reflector.rs  # RFC 4456 client/non-client rules, metadata, collisions
│   ├── test_evpn_route_reflector.rs # EVPN through a reflector with no VNI, plus the PCAP proof
│   ├── test_evpn_rr_failover.rs     # dual-RR redundancy, loop prevention, mobility, scale
│   ├── test_rr_malformed.rs         # hostile ORIGINATOR_ID and CLUSTER_LIST
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

## 🪞 BGP Route Reflection & EVPN Route Reflectors

A full iBGP mesh needs a session between every pair of speakers, because RFC 4271 forbids
passing a route learned from one internal peer to another. RFC 4456 lifts that restriction
for one configured role: a **route reflector** may pass such a route on, and carries two
extra attributes so the loop the mesh rule used to prevent is caught explicitly instead.

The interesting half of this in an EVPN fabric is what a reflector must *not* need:

```text
Leaf1 local MAC
    ↓  EVPN Type 2 origination                   (src/evpn_vtep.rs)
MP_REACH_NLRI over the real TCP session          (src/bgp_router.rs → src/socket.rs)
    ↓
RR: no VNI, no EvpnInstance, no import Route Target, no VTEP
    ↓  RFC 4456 reflection + ORIGINATOR_ID + CLUSTER_LIST
Leaf2
    ↓  Route Target import                       (src/bgp_evpn.rs)
EVPN Loc-RIB → (VNI, MAC) → remote VTEP          (src/evpn_vtep.rs)
    ↓
VXLAN on UDP 4789 → tenant traffic
```

The reflector is a control-plane device. It never becomes a tenant forwarding endpoint, it
is in nobody's flood list, and no tenant MAC points at it.

### Peer roles and cluster identity

Roles are configured, never inferred from the shape of the topology — a speaker that
guessed "this looks like a hub" would start reflecting between peers an operator had
deliberately kept apart.

```rust
bgp.set_route_reflector_client(peer_addr, true);   // BgpPeerRole::RouteReflectorClient
bgp.set_cluster_id(Ipv4Address::new(10, 0, 0, 254));
bgp.is_route_reflector();                          // true once any peer is a client
```

`cluster_id()` defaults to the BGP identifier, which RFC 4456 section 7 allows for a
cluster served by one reflector. Two reflectors serving the same clients may either share a
cluster ID — in which case each refuses the other's reflections as its own cluster coming
back — or keep distinct ones, which is what gives a client two live paths.

### Propagation rules

The plain RFC 4271 rule stays the default; reflection is an exception carved out of it, and
only for the pairings the RFC names:

| Path learned from | to a client | to a non-client | to an eBGP peer |
|---|---|---|---|
| a route reflector client | reflected | reflected | advertised |
| a non-client iBGP peer | reflected | **withheld** | advertised |
| locally originated / eBGP | advertised | advertised | advertised |

An external session never reflects. The two attributes are non-transitive and describe one
autonomous system, so an eBGP neighbour is advertised to exactly as it was before.

The engine is one code path shared by both families: `BgpRouter::propagation` answers
"may this go, and is sending it reflection?", and `compute_adj_rib_out` (IPv4 unicast) and
`compute_evpn_adj_rib_out` (AFI 25 / SAFI 70) both consult it. There is no separate
EVPN-only reflection engine.

### ORIGINATOR_ID and CLUSTER_LIST

| | ORIGINATOR_ID (type 9) | CLUSTER_LIST (type 10) |
|---|---|---|
| flags | optional, **non-transitive** | optional, **non-transitive** |
| length | exactly 4 | non-zero multiple of 4, at most `MAX_CLUSTER_LIST_LEN` |
| set by | the first reflector, to the advertising speaker's BGP identifier | every reflector, prepending its own cluster ID |
| on later hops | unchanged | the previous list is preserved beneath the new entry |

A wrong length, wrong flags, or a second copy of either attribute is an UPDATE error and
resets the session. A route whose ORIGINATOR_ID is our own identifier, or whose
CLUSTER_LIST already contains our cluster, is a *loop* rather than a protocol violation: the
sender did nothing wrong and the topology simply brought the route back, so the route is
dropped, a counter moves, and the session is left alone. Treating that as an error would
tear a redundant fabric down every time redundancy did its job.

Both checks apply to IPv4 unicast and to EVPN. They have to: inside one AS the AS_PATH never
changes, so it can say nothing at all about whether a route has been round already.

### Retaining a tenant a reflector does not own

An ordinary leaf filters on import: a route whose Extended Communities carry no Route Target
it asked for is dropped at the edge of the Adj-RIB-In and can never program anything. That
is exactly what makes two VNIs on the same pair of leaves genuinely separate, and it is
unchanged.

A reflector cannot do that, because it owns no tenant and would have nothing left to
reflect. So three things that are easy to conflate are kept apart:

| | holds it | can use it | can pass it on |
|---|---|---|---|
| `evpn_adj_rib_in` | everything received | — | — |
| `evpn_loc_rib` | — | only what the local Route Targets import; the **only** thing the VTEP is programmed from | — |
| `evpn_advertise_rib` | — | — | the best path per route, whatever its Route Targets |

`retains_all_route_targets()` is implied by being a reflector, and can also be set
explicitly for a transit speaker with no local instances. Each stored path carries an
`importable` flag recording whether this speaker's own Route Targets matched, so "stored"
and "usable here" are separate questions with separately recorded answers. The per-peer
`MAX_EVPN_ROUTES` ceiling still applies, so a retaining speaker is still bounded.

### Redundancy, and the oscillation it would otherwise cause

```text
           rr1  10.0.0.254                 rr1 and rr2 peer with each other
         /     \                            as ordinary non-clients, so a route
     leaf1     leaf2                        from leaf1 reaches leaf2 twice over
         \     /                            and reaches each reflector both
           rr2  10.0.0.253                  directly and through the other
```

Two reflected paths for one MAC produce two entries in the Adj-RIB-In and exactly one in
the Loc-RIB, because the Loc-RIB is keyed by route and not by peer. The winner is
deterministic, and the answer — MAC → VTEP — is the same whichever path wins, because a
reflector must not rewrite the next hop (RFC 4456 section 10): it names the VTEP that owns
the MAC, not the router that carried the route.

Losing one reflector purges only its paths; the other reflector's copy keeps the overlay
working and nothing is withdrawn that is still reachable. Restoring it reconverges without
duplicate forwarding state and without an update storm.

The decision process gained the RFC 4456 section 9 tie-break — **prefer the shorter
CLUSTER_LIST** — and that is not decoration. Without it, a reflector pair prefers each
other's reflected copy of a client's route over the copy the client advertised directly,
whenever the client's BGP identifier is the higher one, since that is what the next
tie-break compares. Each reflector then sees its best path as coming from the other, stops
advertising to it under split horizon, immediately loses that path again, and
re-advertises — for ever. `build_evpn_rr_oscillation_fabric` is that topology, and
`test_reflectors_do_not_oscillate_when_the_leaves_have_high_identifiers` is the regression:
before the tie-break it produced roughly 8.7 million UPDATEs in fifteen seconds of
simulated time and never settled; after it, twelve, and then silence.

### Connection collision resolution

A reflector topology makes simultaneous opens likely, and the speaker now resolves them
per RFC 4271 section 6.8 rather than refusing the second connection outright.

An inbound connection arriving for a peer that already has one is *held* rather than
aborted, because the rule needs the peer's BGP identifier and that does not arrive until
its OPEN does. The held connection's OPEN is read without being consumed; then the speaker
with the lower identifier keeps the connection its peer initiated and drops the one it
initiated itself, and the other end does the reverse. Both ends therefore choose the same
connection. If the held connection wins it is promoted with its framer intact, so the OPEN
already sitting in it is processed by the ordinary FSM exactly as on any accepted
connection. The loser is aborted, never abandoned, so no orphan TCP stream is left behind.

### Scale

`build_evpn_rr_scale_fabric` builds two reflectors and N leaves on one underlay subnet with
several tenants each. `tests/test_evpn_rr_failover.rs` runs it at eight leaves, four VNIs
and eight hosts per tenant — 288 EVPN routes — and asserts exact counts rather than
approximate health: every leaf's Loc-RIB holds the whole fabric and nothing more, every
leaf holds exactly one copy per reflector, each reflector can advertise all 288 and imports
none of them, no MAC appears in the wrong tenant or points at the wrong VTEP, and after
convergence five further minutes of simulated time produce no UPDATE at all while
KEEPALIVEs keep flowing.

### Shell diagnostics

Everything below is read out of a live two-reflector fabric, built and converged on first
use. Neither reflector in it has a VTEP, a VNI, or an import Route Target.

```text
netstack > bgp rr             # role, cluster ID, received / imported / retained / advertisable,
                              # reflected and withheld counts, loop rejections, collisions
netstack > bgp rr clients     # who is a client of whom, and what that permits
netstack > bgp rr routes      # every EVPN path held: from whom, client or not, imported or retained-only
netstack > bgp rr advertised  # the Adj-RIB-Out, marking what was reflected and with which cluster list
netstack > bgp capabilities   # now also shows the reflection role and local cluster ID per session
```

```text
netstack > bgp rr
== rr1 == router-id 10.0.0.254  AS 65000  route-reflector enabled
  cluster-id 10.0.0.254  clients [10.0.0.1, 10.0.0.2]
  import route-targets []  retain-all-RTs yes
  EVPN routes: received 8  locally imported 0  retained-not-imported 8  advertisable 4  originated 0
  VXLAN tenant forwarding: none (control plane only)
  neighbor 10.0.0.1 role client     state Established up 60000ms
    AFI/SAFI [IPv4 Unicast, L2VPN EVPN]  4-octet ASN yes
    EVPN received 2  advertised 2  reflected 2  withheld-by-propagation-rules 0
```

### Scope

This phase is route reflection and EVPN control-plane scale and high availability. EVPN
multihoming — Route Types 1 and 4, designated-forwarder election, Ethernet Segment split
horizon, aliasing, and mass withdrawal by Ethernet Segment — is the next one and is
deliberately absent.

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
cargo test --test test_tcp_paws           # RFC 7323 timestamp negotiation, TS.Recent, PAWS
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

### 5b. Run the Route Reflection Suites
```bash
cargo test --test test_bgp_route_reflector  # client/non-client rules, metadata, collisions
cargo test --test test_evpn_route_reflector # EVPN through a reflector with no VNI, plus PCAP
cargo test --test test_evpn_rr_failover     # dual-RR HA, loop prevention, mobility, scale
cargo test --test test_rr_malformed         # hostile ORIGINATOR_ID and CLUSTER_LIST
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
