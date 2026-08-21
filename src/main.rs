#![allow(clippy::too_many_arguments)]
#![allow(clippy::vec_init_then_push)]
#![allow(clippy::same_item_push)]
#![allow(clippy::unnecessary_sort_by)]
#![allow(clippy::if_same_then_else)]
#![allow(clippy::int_plus_one)]
#![allow(clippy::bool_assert_comparison)]
#![allow(clippy::type_complexity)]

use std::env;
use std::fs::File;
use std::process;
use toy_tcpip::arp::ArpPacket;
use toy_tcpip::ethernet::{EtherType, EthernetFrame, MacAddress};
use toy_tcpip::icmp::IcmpPacket;
use toy_tcpip::ipv4::{IpProtocol, Ipv4Address, Ipv4Packet};
use toy_tcpip::pcap::{LINKTYPE_ETHERNET, PcapPacket, PcapReader, PcapWriter};
use toy_tcpip::shell::NetworkShell;
use toy_tcpip::stack::{NetStack, NetStackConfig};
use toy_tcpip::tcp::TcpSegment;
use toy_tcpip::udp::UdpDatagram;

fn print_hex_ascii(data: &[u8], max_bytes: usize) {
    let to_show = data.len().min(max_bytes);
    let slice = &data[0..to_show];
    let ascii: String = slice
        .iter()
        .map(|&b| {
            if (32..=126).contains(&b) {
                b as char
            } else {
                '.'
            }
        })
        .collect();
    print!("    Hex: ");
    for b in slice {
        print!("{:02x} ", b);
    }
    if data.len() > max_bytes {
        print!("... ({} bytes total)", data.len());
    }
    println!("\n    Text: \"{}\"", ascii);
}

fn inspect_packet(idx: usize, pkt: &PcapPacket) {
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!(
        " 📦 Frame #{:<3} | Capture: {:>4} bytes | Time: {}.{:06}s",
        idx, pkt.incl_len, pkt.ts_sec, pkt.ts_usec
    );
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    let eth = match EthernetFrame::parse(&pkt.data) {
        Ok(e) => e,
        Err(err) => {
            println!("  ❌ Failed to parse Ethernet frame: {}", err);
            return;
        }
    };

    println!("  ┌── [ Layer 2: Ethernet II ]");
    println!("  │  Source MAC : {}", eth.src_mac);
    println!("  │  Dest MAC   : {}", eth.dst_mac);
    println!("  │  EtherType  : {}", eth.ethertype);

    match eth.ethertype {
        EtherType::Arp => match ArpPacket::parse(eth.payload) {
            Ok(arp) => {
                println!("  ├── [ Layer 2.5: ARP ]");
                println!("  │  Opcode     : {}", arp.opcode);
                println!(
                    "  │  Sender     : MAC {} | IP {}.{}.{}.{}",
                    arp.sender_mac,
                    arp.sender_ip[0],
                    arp.sender_ip[1],
                    arp.sender_ip[2],
                    arp.sender_ip[3]
                );
                println!(
                    "  │  Target     : MAC {} | IP {}.{}.{}.{}",
                    arp.target_mac,
                    arp.target_ip[0],
                    arp.target_ip[1],
                    arp.target_ip[2],
                    arp.target_ip[3]
                );
            }
            Err(e) => println!("  │  ❌ Invalid ARP packet: {}", e),
        },

        EtherType::IPv4 => match Ipv4Packet::parse(eth.payload, false) {
            Ok(ip) => {
                println!("  ├── [ Layer 3: IPv4 ]");
                println!(
                    "  │  Version/IHL: v{} / {} bytes (IHL={})",
                    ip.header.version,
                    ip.header.header_len_bytes(),
                    ip.header.ihl
                );
                println!("  │  Source IP  : {}", ip.header.src_ip);
                println!("  │  Dest IP    : {}", ip.header.dst_ip);
                println!(
                    "  │  TTL / Proto: TTL={} | Protocol={}",
                    ip.header.ttl, ip.header.protocol
                );
                println!(
                    "  │  Length / ID: Total {} bytes | ID=0x{:04x}",
                    ip.header.total_length, ip.header.identification
                );
                println!("  │  Checksum   : 0x{:04x}", ip.header.checksum);

                match ip.header.protocol {
                    IpProtocol::Icmp => match IcmpPacket::parse(ip.payload, false) {
                        Ok(icmp) => {
                            println!("  ├── [ Layer 3.5: ICMP ]");
                            println!("  │  Type / Code: {} | Code={}", icmp.icmp_type, icmp.code);
                            println!(
                                "  │  ID / SeqNum: ID=0x{:04x} ({}) | Seq={}",
                                icmp.identifier, icmp.identifier, icmp.sequence_number
                            );
                            println!("  │  Checksum   : 0x{:04x}", icmp.checksum);
                            if !icmp.payload.is_empty() {
                                println!("  │  Payload ({} bytes):", icmp.payload.len());
                                print_hex_ascii(icmp.payload, 32);
                            }
                        }
                        Err(e) => println!("  │  ❌ Invalid ICMP message: {}", e),
                    },

                    IpProtocol::Udp => match UdpDatagram::parse(
                        ip.header.src_ip,
                        ip.header.dst_ip,
                        ip.payload,
                        false,
                    ) {
                        Ok(udp) => {
                            println!("  ├── [ Layer 4: UDP ]");
                            println!("  │  Src Port   : {}", udp.src_port);
                            println!("  │  Dst Port   : {}", udp.dst_port);
                            println!("  │  Length     : {} bytes", udp.length);
                            println!("  │  Checksum   : 0x{:04x}", udp.checksum);
                            if !udp.payload.is_empty() {
                                println!("  │  Payload ({} bytes):", udp.payload.len());
                                print_hex_ascii(udp.payload, 32);
                            }
                        }
                        Err(e) => println!("  │  ❌ Invalid UDP datagram: {}", e),
                    },

                    IpProtocol::Tcp => match TcpSegment::parse(
                        ip.header.src_ip,
                        ip.header.dst_ip,
                        ip.payload,
                        false,
                    ) {
                        Ok(tcp) => {
                            println!("  ├── [ Layer 4: TCP ]");
                            println!("  │  Src Port   : {}", tcp.src_port);
                            println!("  │  Dst Port   : {}", tcp.dst_port);
                            println!("  │  Seq Number : {}", tcp.seq_num);
                            println!("  │  Ack Number : {}", tcp.ack_num);
                            println!("  │  Flags      : {}", tcp.flags);
                            println!("  │  Window Size: {}", tcp.window_size);
                            println!("  │  Checksum   : 0x{:04x}", tcp.checksum);
                            if !tcp.payload.is_empty() {
                                println!("  │  Payload ({} bytes):", tcp.payload.len());
                                print_hex_ascii(tcp.payload, 48);
                            }
                        }
                        Err(e) => println!("  │  ❌ Invalid TCP segment: {}", e),
                    },

                    _ => {
                        println!("  │  (Unparsed Layer 4 Protocol)");
                    }
                }
            }
            Err(e) => println!("  │  ❌ Invalid IPv4 packet: {}", e),
        },

        _ => {}
    }
    println!("  └─────────────────────────────────────────────────");
}

fn run_inspect(path: &str) {
    let file = match File::open(path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Error opening PCAP file '{}': {}", path, e);
            process::exit(1);
        }
    };

    let mut reader = match PcapReader::new(file) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error reading PCAP header: {}", e);
            process::exit(1);
        }
    };

    println!("Opened PCAP: '{}'", path);
    println!(
        "  Version: {}.{} | Snaplen: {} | LinkType: {}",
        reader.header.version_major,
        reader.header.version_minor,
        reader.header.snaplen,
        reader.header.network
    );

    let mut count = 0;
    while let Ok(Some(pkt)) = reader.next_packet() {
        count += 1;
        inspect_packet(count, &pkt);
    }

    println!("\n✅ Inspection complete. Total packets: {}", count);
}

fn run_replay(in_path: &str, out_path: &str) {
    let in_file = match File::open(in_path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Error opening input PCAP file '{}': {}", in_path, e);
            process::exit(1);
        }
    };

    let mut reader = match PcapReader::new(in_file) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error reading input PCAP header: {}", e);
            process::exit(1);
        }
    };

    let out_file = match File::create(out_path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Error creating output PCAP file '{}': {}", out_path, e);
            process::exit(1);
        }
    };

    let mut writer = match PcapWriter::new(out_file, 65535, LINKTYPE_ETHERNET) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("Error initializing PCAP writer: {}", e);
            process::exit(1);
        }
    };

    // Configure our virtual NetStack host: 192.168.1.10 (02:00:00:00:00:10)
    let stack_mac = MacAddress([0x02, 0x00, 0x00, 0x00, 0x00, 0x10]);
    let stack_ip = Ipv4Address::new(192, 168, 1, 10);
    let mut stack = NetStack::new(NetStackConfig {
        mac: stack_mac,
        ip: stack_ip,
        ipv6: None,
        subnet_mask: 24,
        gateway: Some(Ipv4Address::new(192, 168, 1, 1)),
    });

    // Register UDP echo on port 7 and DNS mock on port 53
    stack
        .udp_sockets
        .bind(7, |_src_ip, _src_port, payload| Some(payload.to_vec()));
    stack.udp_sockets.bind(53, |_src_ip, _src_port, _payload| {
        Some(b"DNS: toy-tcpip.local -> 192.168.1.10".to_vec())
    });

    // Listen on TCP HTTP port 80 and Echo port 7777
    stack.tcp_manager.listen(80);
    stack.tcp_manager.listen(7777);

    println!(
        "Replaying packets from '{}' into NetStack (IP: {}, MAC: {})...",
        in_path, stack_ip, stack_mac
    );

    let mut in_count = 0;
    let mut out_count = 0;

    while let Ok(Some(pkt)) = reader.next_packet() {
        in_count += 1;
        let responses = stack.process_frame(&pkt.data);
        for resp in responses {
            out_count += 1;
            writer
                .write_packet(pkt.ts_sec, pkt.ts_usec + out_count as u32 * 10, &resp)
                .expect("Failed to write response packet");
        }
    }

    println!(
        "✅ Replay complete! Processed {} incoming frames, generated {} response frames -> written to '{}'",
        in_count, out_count, out_path
    );
}

fn run_demo() {
    println!("╔════════════════════════════════════════════════════════════════════════════╗");
    println!("║                 🚀 Educational Toy TCP/IP Stack Demo                      ║");
    println!("╚════════════════════════════════════════════════════════════════════════════╝\n");

    let stack_mac = MacAddress([0x02, 0x00, 0x00, 0x00, 0x00, 0x10]);
    let stack_ip = Ipv4Address::new(192, 168, 1, 10);
    let client_mac = MacAddress([0x52, 0x54, 0x00, 0x12, 0x34, 0x56]);
    let client_ip = Ipv4Address::new(192, 168, 1, 100);

    let mut stack = NetStack::new(NetStackConfig {
        mac: stack_mac,
        ip: stack_ip,
        ipv6: None,
        subnet_mask: 24,
        gateway: Some(Ipv4Address::new(192, 168, 1, 1)),
    });

    // Setup services
    stack
        .udp_sockets
        .bind(7, |_src_ip, _src_port, data| Some(data.to_vec()));
    stack.tcp_manager.listen(80);

    println!("🖥️  Initialized NetStack:");
    println!("   - Host IP      : {}", stack_ip);
    println!("   - Host MAC     : {}", stack_mac);
    println!("   - Subnet       : 192.168.1.0/24");
    println!("   - Gateway      : 192.168.1.1");
    println!("   - UDP Port 7   : Echo service enabled");
    println!("   - TCP Port 80  : HTTP/Web service listening");

    println!("\n────────────────────────────────────────────────────────────────────────────");
    println!(" 🔹 SCENARIO 1: Address Resolution Protocol (ARP Request -> Reply)");
    println!("────────────────────────────────────────────────────────────────────────────");

    let arp_req = ArpPacket::build_request(client_mac, client_ip.0, stack_ip.0);
    let arp_frame = EthernetFrame::serialize(
        MacAddress::BROADCAST,
        client_mac,
        toy_tcpip::ethernet::ETHERTYPE_ARP,
        &arp_req.serialize(),
    );

    println!("Client (192.168.1.100) broadcasts: \"Who has 192.168.1.10? Tell 192.168.1.100\"");
    let responses = stack.process_frame(&arp_frame);
    assert_eq!(responses.len(), 1);

    let resp_eth = EthernetFrame::parse(&responses[0]).unwrap();
    let resp_arp = ArpPacket::parse(resp_eth.payload).unwrap();
    println!("⚡ NetStack replied with ARP Reply:");
    println!("   - Destination MAC: {}", resp_eth.dst_mac);
    println!("   - Sender MAC     : {}", resp_arp.sender_mac);
    println!(
        "   - Sender IP      : {}.{}.{}.{}",
        resp_arp.sender_ip[0], resp_arp.sender_ip[1], resp_arp.sender_ip[2], resp_arp.sender_ip[3]
    );
    println!(
        "   - ARP Cache Updated: 192.168.1.100 -> {}",
        stack.arp_table.lookup(&client_ip.0).unwrap()
    );

    println!("\n────────────────────────────────────────────────────────────────────────────");
    println!(" 🔹 SCENARIO 2: ICMP Ping (Echo Request -> Echo Reply)");
    println!("────────────────────────────────────────────────────────────────────────────");

    let ping_data = b"Hello NetStack ICMP Echo!";
    let icmp_req = IcmpPacket::build_echo_request(0x1337, 1, ping_data);
    let ip_req = Ipv4Packet::serialize(
        client_ip,
        stack_ip,
        toy_tcpip::ipv4::IP_PROTO_ICMP,
        101,
        64,
        &icmp_req,
    );
    let eth_req = EthernetFrame::serialize(
        stack_mac,
        client_mac,
        toy_tcpip::ethernet::ETHERTYPE_IPV4,
        &ip_req,
    );

    println!(
        "Client sends ICMP Echo Request (Ping, ID=0x1337, Seq=1, \"Hello NetStack ICMP Echo!\")"
    );
    let ping_resps = stack.process_frame(&eth_req);
    assert_eq!(ping_resps.len(), 1);

    let ping_eth = EthernetFrame::parse(&ping_resps[0]).unwrap();
    let ping_ip = Ipv4Packet::parse(ping_eth.payload, true).unwrap();
    let ping_icmp = IcmpPacket::parse(ping_ip.payload, true).unwrap();
    println!("⚡ NetStack replied with ICMP Echo Reply:");
    println!(
        "   - Type / Code    : {} / {}",
        ping_icmp.icmp_type, ping_icmp.code
    );
    println!("   - Identifier     : 0x{:04x}", ping_icmp.identifier);
    println!("   - Sequence Number: {}", ping_icmp.sequence_number);
    println!(
        "   - Payload Match  : \"{}\"",
        String::from_utf8_lossy(ping_icmp.payload)
    );

    println!("\n────────────────────────────────────────────────────────────────────────────");
    println!(" 🔹 SCENARIO 3: UDP Datagram (Echo Service on Port 7)");
    println!("────────────────────────────────────────────────────────────────────────────");

    let udp_data = b"Data packet sent to UDP Echo";
    let udp_req = UdpDatagram::serialize(client_ip, stack_ip, 49152, 7, udp_data);
    let ip_udp_req = Ipv4Packet::serialize(
        client_ip,
        stack_ip,
        toy_tcpip::ipv4::IP_PROTO_UDP,
        102,
        64,
        &udp_req,
    );
    let eth_udp_req = EthernetFrame::serialize(
        stack_mac,
        client_mac,
        toy_tcpip::ethernet::ETHERTYPE_IPV4,
        &ip_udp_req,
    );

    println!(
        "Client sends UDP to port 7 (length {} bytes)",
        udp_data.len()
    );
    let udp_resps = stack.process_frame(&eth_udp_req);
    assert_eq!(udp_resps.len(), 1);

    let udp_eth = EthernetFrame::parse(&udp_resps[0]).unwrap();
    let udp_ip = Ipv4Packet::parse(udp_eth.payload, true).unwrap();
    let udp_resp = UdpDatagram::parse(
        udp_ip.header.src_ip,
        udp_ip.header.dst_ip,
        udp_ip.payload,
        true,
    )
    .unwrap();
    println!("⚡ NetStack replied with UDP Echo response:");
    println!(
        "   - Src Port / Dst Port: {} -> {}",
        udp_resp.src_port, udp_resp.dst_port
    );
    println!(
        "   - Echoed Payload     : \"{}\"",
        String::from_utf8_lossy(udp_resp.payload)
    );

    println!("\n────────────────────────────────────────────────────────────────────────────");
    println!(" 🔹 SCENARIO 4: TCP 3-Way Handshake & Data Transfer (Port 80)");
    println!("────────────────────────────────────────────────────────────────────────────");

    let client_port = 50000;
    let client_isn = 10000;

    // Step 1: SYN
    println!("1️⃣  [Client -> Server] TCP SYN (Seq={})", client_isn);
    let syn_seg = TcpSegment::serialize(
        client_ip,
        stack_ip,
        client_port,
        80,
        client_isn,
        0,
        toy_tcpip::tcp::TcpFlags::syn(),
        65535,
        &[],
    );
    let ip_syn = Ipv4Packet::serialize(
        client_ip,
        stack_ip,
        toy_tcpip::ipv4::IP_PROTO_TCP,
        103,
        64,
        &syn_seg,
    );
    let eth_syn = EthernetFrame::serialize(
        stack_mac,
        client_mac,
        toy_tcpip::ethernet::ETHERTYPE_IPV4,
        &ip_syn,
    );

    let syn_resps = stack.process_frame(&eth_syn);
    assert_eq!(syn_resps.len(), 1);

    let syn_ack_eth = EthernetFrame::parse(&syn_resps[0]).unwrap();
    let syn_ack_ip = Ipv4Packet::parse(syn_ack_eth.payload, true).unwrap();
    let syn_ack_tcp = TcpSegment::parse(
        syn_ack_ip.header.src_ip,
        syn_ack_ip.header.dst_ip,
        syn_ack_ip.payload,
        true,
    )
    .unwrap();
    println!(
        "2️⃣  [Server -> Client] TCP SYN+ACK (Flags={}, Seq={}, Ack={})",
        syn_ack_tcp.flags, syn_ack_tcp.seq_num, syn_ack_tcp.ack_num
    );

    // Step 2: ACK (completing 3-way handshake)
    let server_isn = syn_ack_tcp.seq_num;
    println!(
        "3️⃣  [Client -> Server] TCP ACK (Seq={}, Ack={})",
        syn_ack_tcp.ack_num,
        server_isn + 1
    );
    let ack_seg = TcpSegment::serialize(
        client_ip,
        stack_ip,
        client_port,
        80,
        syn_ack_tcp.ack_num,
        server_isn + 1,
        toy_tcpip::tcp::TcpFlags::ack(),
        65535,
        &[],
    );
    let ip_ack = Ipv4Packet::serialize(
        client_ip,
        stack_ip,
        toy_tcpip::ipv4::IP_PROTO_TCP,
        104,
        64,
        &ack_seg,
    );
    let eth_ack = EthernetFrame::serialize(
        stack_mac,
        client_mac,
        toy_tcpip::ethernet::ETHERTYPE_IPV4,
        &ip_ack,
    );
    let _ = stack.process_frame(&eth_ack);

    let key = toy_tcpip::tcp::TcpConnectionKey {
        local: toy_tcpip::tcp::SocketAddrV4 {
            ip: stack_ip,
            port: 80,
        },
        remote: toy_tcpip::tcp::SocketAddrV4 {
            ip: client_ip,
            port: client_port,
        },
    };
    let conn = stack.tcp_manager.connections.get(&key).unwrap();
    println!("🎉 TCP Connection State: {}", conn.state);

    // Step 3: Client sends HTTP GET
    let http_req = b"GET /index.html HTTP/1.1\r\nHost: 192.168.1.10\r\n\r\n";
    println!(
        "4️⃣  [Client -> Server] TCP PUSH DATA ({} bytes: \"GET /index.html ...\")",
        http_req.len()
    );
    let data_seg = TcpSegment::serialize(
        client_ip,
        stack_ip,
        client_port,
        80,
        syn_ack_tcp.ack_num,
        server_isn + 1,
        toy_tcpip::tcp::TcpFlags {
            psh: true,
            ack: true,
            ..Default::default()
        },
        65535,
        http_req,
    );
    let ip_data = Ipv4Packet::serialize(
        client_ip,
        stack_ip,
        toy_tcpip::ipv4::IP_PROTO_TCP,
        105,
        64,
        &data_seg,
    );
    let eth_data = EthernetFrame::serialize(
        stack_mac,
        client_mac,
        toy_tcpip::ethernet::ETHERTYPE_IPV4,
        &ip_data,
    );

    let data_resps = stack.process_frame(&eth_data);
    assert_eq!(data_resps.len(), 1);
    let ack_resp_eth = EthernetFrame::parse(&data_resps[0]).unwrap();
    let ack_resp_ip = Ipv4Packet::parse(ack_resp_eth.payload, true).unwrap();
    let ack_resp_tcp = TcpSegment::parse(
        ack_resp_ip.header.src_ip,
        ack_resp_ip.header.dst_ip,
        ack_resp_ip.payload,
        true,
    )
    .unwrap();
    println!(
        "5️⃣  [Server -> Client] TCP ACK for Data (Ack={})",
        ack_resp_tcp.ack_num
    );

    let conn_after = stack.tcp_manager.connections.get(&key).unwrap();
    println!(
        "📥 NetStack received stream payload: \"{}\"",
        String::from_utf8_lossy(&conn_after.rx_buffer)
    );

    println!(
        "\n✨ Demo successfully completed all layers: Ethernet -> ARP -> IPv4 -> ICMP -> UDP -> TCP! ✨\n"
    );
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        println!("Toy TCP/IP Stack - Educational Network Protocol Suite\n");
        println!("Usage:");
        println!("  toy_tcpip demo");
        println!("  toy_tcpip shell");
        println!("  toy_tcpip inspect <file.pcap>");
        println!("  toy_tcpip replay <input.pcap> <output.pcap>");
        println!("\nRunning built-in interactive demo by default...\n");
        run_demo();
        return;
    }

    match args[1].as_str() {
        "demo" => run_demo(),
        "shell" => {
            let mut shell = NetworkShell::new();
            shell.run_repl();
        }
        "inspect" => {
            if args.len() < 3 {
                eprintln!(
                    "Error: 'inspect' requires a PCAP file path: toy_tcpip inspect <file.pcap>"
                );
                process::exit(1);
            }
            run_inspect(&args[2]);
        }
        "replay" => {
            if args.len() < 4 {
                eprintln!(
                    "Error: 'replay' requires input and output PCAP paths: toy_tcpip replay <input.pcap> <output.pcap>"
                );
                process::exit(1);
            }
            run_replay(&args[2], &args[3]);
        }
        path if path.ends_with(".pcap") => {
            // Direct file inspect shortcut
            run_inspect(path);
        }
        other => {
            eprintln!("Unknown command: '{}'", other);
            eprintln!(
                "Available commands: demo, shell, inspect <file.pcap>, replay <in.pcap> <out.pcap>"
            );
            process::exit(1);
        }
    }
}
