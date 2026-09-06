//go:build linux

// internal/network/veth_linux.go
//
// Virtual Ethernet Pair (veth) Setup
// ────────────────────────────────────
// A veth pair is two virtual network interfaces joined by a kernel pipe:
// every packet sent into one end comes out the other.  Container runtimes
// use them to bridge the host and container network namespaces:
//
//   host netns                     container netns
//   ┌──────────────────┐           ┌──────────────────┐
//   │ veth-hPID        │ ◄───────► │ eth0             │
//   │ 172.20.0.1/24    │           │ 172.20.0.2/24    │
//   └──────────────────┘           └──────────────────┘
//
// Two-phase setup
// ───────────────
// Phase 1 — parent (after fork, before releasing sync pipe):
//   1. RTM_NEWLINK  create veth-hPID ↔ veth-peer in host netns
//   2. RTM_NEWADDR  assign 172.20.0.1/24 to veth-hPID
//   3. RTM_NEWLINK  set veth-hPID UP
//   4. RTM_NEWLINK  move veth-peer to container's netns (by PID)
//
// Phase 2 — child (inside container netns, before pivot_root):
//   5. RTM_NEWLINK  rename veth-peer → eth0
//   6. RTM_NEWADDR  assign 172.20.0.2/24 to eth0
//   7. RTM_NEWLINK  set eth0 UP
//   8. RTM_NEWROUTE add default route via 172.20.0.1
//
// All operations use Linux's NETLINK_ROUTE socket — the same protocol
// used by the `ip` command from iproute2.
//
// Netlink message anatomy
// ───────────────────────
// Every request is:
//   NlMsghdr (16 B)   total length, message type, flags, seq, pid
//   type-specific struct  e.g. ifinfomsg for RTM_NEWLINK
//   netlink attributes    TLV (2-B len | 2-B type | data), padded to 4 B
//
// Attributes can be nested: the outer attr's value is itself a run of attrs.
// The veth-pair creation message uses three levels of nesting:
//   IFLA_LINKINFO → IFLA_INFO_DATA → VETH_INFO_PEER → ifinfomsg + IFLA_IFNAME

package network

import (
	"encoding/binary"
	"fmt"
	"net"
	"os"
	"syscall"
	"unsafe"
)

// ─── Constants not exported by the syscall package ───────────────────────────

const (
	iflaLinkinfo = 18 // IFLA_LINKINFO  nested attr: link-type metadata
	iflaInfoKind = 1  // IFLA_INFO_KIND string: link type, e.g. "veth"
	iflaInfoData = 2  // IFLA_INFO_DATA nested: link-type-specific attributes
	vethInfoPeer = 1  // VETH_INFO_PEER nested: peer ifinfomsg + attributes

	iflaNetNsPid = 19 // IFLA_NET_NS_PID u32: move iface into this PID's netns

	ifaAddress = 1 // IFA_ADDRESS: interface address (broadcast-capable ifaces)
	ifaLocal   = 2 // IFA_LOCAL:   local unicast address

	rtaGateway = 5 // RTA_GATEWAY: next-hop IP
	rtaOif     = 4 // RTA_OIF u32: output interface index
)

// nativeEndian is the byte order used for netlink structs.
// Netlink headers are always native-endian on the local machine.
var nativeEndian = func() binary.ByteOrder {
	var x uint32 = 1
	if *(*byte)(unsafe.Pointer(&x)) == 1 {
		return binary.LittleEndian
	}
	return binary.BigEndian
}()

// ─── Minimal NETLINK_ROUTE socket ────────────────────────────────────────────

type nlSock struct{ fd int }

func openNL() (*nlSock, error) {
	fd, err := syscall.Socket(
		syscall.AF_NETLINK,
		syscall.SOCK_RAW|syscall.SOCK_CLOEXEC,
		syscall.NETLINK_ROUTE,
	)
	if err != nil {
		return nil, fmt.Errorf("netlink socket: %w", err)
	}
	if err := syscall.Bind(fd, &syscall.SockaddrNetlink{Family: syscall.AF_NETLINK}); err != nil {
		syscall.Close(fd)
		return nil, fmt.Errorf("bind netlink: %w", err)
	}
	return &nlSock{fd: fd}, nil
}

func (s *nlSock) close() { syscall.Close(s.fd) }

// do sends msg and blocks until the kernel replies with an ACK or error.
// NLM_F_ACK must be set in the message for the kernel to send a response.
func (s *nlSock) do(msg []byte) error {
	if err := syscall.Sendto(s.fd, msg, 0,
		&syscall.SockaddrNetlink{Family: syscall.AF_NETLINK}); err != nil {
		return fmt.Errorf("netlink send: %w", err)
	}

	buf := make([]byte, 4096)
	for {
		n, _, err := syscall.Recvfrom(s.fd, buf, 0)
		if err != nil {
			return fmt.Errorf("netlink recv: %w", err)
		}
		msgs, err := syscall.ParseNetlinkMessage(buf[:n])
		if err != nil {
			return err
		}
		for _, m := range msgs {
			switch m.Header.Type {
			case syscall.NLMSG_DONE:
				return nil
			case syscall.NLMSG_ERROR:
				if len(m.Data) < 4 {
					return fmt.Errorf("netlink: truncated error response")
				}
				// NlMsgerr.Error is a negative errno value.
				if e := int32(nativeEndian.Uint32(m.Data[:4])); e != 0 {
					return syscall.Errno(-e)
				}
				return nil // errno == 0 → success ACK
			}
		}
	}
}

// ─── Message builders ─────────────────────────────────────────────────────────

var nlSeq uint32

// nlMsg builds a full netlink message:  NlMsghdr | body.
func nlMsg(typ, flags uint16, body []byte) []byte {
	nlSeq++
	total := 16 + len(body)
	msg := make([]byte, total)
	nativeEndian.PutUint32(msg[0:], uint32(total)) // nlmsg_len
	nativeEndian.PutUint16(msg[4:], typ)            // nlmsg_type
	nativeEndian.PutUint16(msg[6:], flags)          // nlmsg_flags
	nativeEndian.PutUint32(msg[8:], nlSeq)          // nlmsg_seq
	// nlmsg_pid = 0 → kernel fills it
	copy(msg[16:], body)
	return msg
}

// ifInfomsg builds a 16-byte struct ifinfomsg.
func mkIfInfomsg(family byte, index int32, flags, change uint32) []byte {
	b := make([]byte, 16)
	b[0] = family
	// b[1] __ifi_pad, b[2:4] ifi_type — leave as zero
	nativeEndian.PutUint32(b[4:], uint32(index))
	nativeEndian.PutUint32(b[8:], flags)
	nativeEndian.PutUint32(b[12:], change)
	return b
}

// mkIfAddrmsg builds an 8-byte struct ifaddrmsg.
func mkIfAddrmsg(family, prefixlen byte, index uint32) []byte {
	b := make([]byte, 8)
	b[0] = family
	b[1] = prefixlen
	// b[2] ifa_flags, b[3] ifa_scope — leave as zero
	nativeEndian.PutUint32(b[4:], index)
	return b
}

// mkRtmsg builds a 12-byte struct rtmsg for a default unicast route.
func mkRtmsg(family byte) []byte {
	b := make([]byte, 12)
	b[0] = family
	// rtm_dst_len=0 (default route), rtm_src_len=0, rtm_tos=0
	b[4] = syscall.RT_TABLE_MAIN
	b[5] = syscall.RTPROT_STATIC
	b[6] = syscall.RT_SCOPE_UNIVERSE
	b[7] = syscall.RTN_UNICAST
	// rtm_flags = 0
	return b
}

// nlAttr returns a netlink attribute: 4-byte TLV header + data, padded to 4 B.
func nlAttr(typ uint16, data []byte) []byte {
	sz := 4 + len(data)
	pad := (4 - sz&3) & 3
	b := make([]byte, sz+pad)
	nativeEndian.PutUint16(b[0:], uint16(sz))
	nativeEndian.PutUint16(b[2:], typ)
	copy(b[4:], data)
	return b
}

func nlAttrStr(typ uint16, s string) []byte {
	return nlAttr(typ, append([]byte(s), 0)) // kernel expects NUL-terminated
}
func nlAttrU32(typ uint16, v uint32) []byte {
	var b [4]byte
	nativeEndian.PutUint32(b[:], v)
	return nlAttr(typ, b[:])
}
func nlAttrIP(typ uint16, ip net.IP) []byte { return nlAttr(typ, ip.To4()) }

func cat(parts ...[]byte) (out []byte) {
	for _, p := range parts {
		out = append(out, p...)
	}
	return
}

// ─── Netlink operations ───────────────────────────────────────────────────────

// createVethPair creates a veth pair name↔peer in the current netns.
//
// The message nests three levels deep to describe the peer:
//
//	RTM_NEWLINK body:
//	  ifinfomsg
//	  IFLA_IFNAME = name
//	  IFLA_LINKINFO:
//	    IFLA_INFO_KIND = "veth"
//	    IFLA_INFO_DATA:
//	      VETH_INFO_PEER:
//	        ifinfomsg   (for the peer — can be zeroed)
//	        IFLA_IFNAME = peer
func createVethPair(name, peer string) error {
	peerBlock := cat(
		mkIfInfomsg(syscall.AF_UNSPEC, 0, 0, 0),
		nlAttrStr(syscall.IFLA_IFNAME, peer),
	)
	body := cat(
		mkIfInfomsg(syscall.AF_UNSPEC, 0, 0, 0),
		nlAttrStr(syscall.IFLA_IFNAME, name),
		nlAttr(iflaLinkinfo, cat(
			nlAttrStr(iflaInfoKind, "veth"),
			nlAttr(iflaInfoData, nlAttr(vethInfoPeer, peerBlock)),
		)),
	)
	msg := nlMsg(
		syscall.RTM_NEWLINK,
		syscall.NLM_F_REQUEST|syscall.NLM_F_ACK|syscall.NLM_F_CREATE|syscall.NLM_F_EXCL,
		body,
	)
	s, err := openNL()
	if err != nil {
		return err
	}
	defer s.close()
	return s.do(msg)
}

// moveToNetns moves the named interface into the network namespace of pid.
// After this call the interface is no longer visible in the current netns.
func moveToNetns(name string, pid int) error {
	iface, err := net.InterfaceByName(name)
	if err != nil {
		return err
	}
	body := cat(
		mkIfInfomsg(syscall.AF_UNSPEC, int32(iface.Index), 0, 0),
		nlAttrU32(iflaNetNsPid, uint32(pid)),
	)
	msg := nlMsg(syscall.RTM_NEWLINK, syscall.NLM_F_REQUEST|syscall.NLM_F_ACK, body)
	s, err := openNL()
	if err != nil {
		return err
	}
	defer s.close()
	return s.do(msg)
}

// renameIface changes an interface's name.
// The interface must be DOWN (kernel rejects rename on UP interfaces).
func renameIface(oldName, newName string) error {
	iface, err := net.InterfaceByName(oldName)
	if err != nil {
		return err
	}
	body := cat(
		mkIfInfomsg(syscall.AF_UNSPEC, int32(iface.Index), 0, 0),
		nlAttrStr(syscall.IFLA_IFNAME, newName),
	)
	msg := nlMsg(syscall.RTM_NEWLINK, syscall.NLM_F_REQUEST|syscall.NLM_F_ACK, body)
	s, err := openNL()
	if err != nil {
		return err
	}
	defer s.close()
	return s.do(msg)
}

// setLinkUp brings an interface to the UP state.
func setLinkUp(name string) error {
	iface, err := net.InterfaceByName(name)
	if err != nil {
		return err
	}
	// Setting ifi_flags=IFF_UP and ifi_change=IFF_UP tells the kernel to
	// enable that bit without touching any other flag.
	body := mkIfInfomsg(syscall.AF_UNSPEC, int32(iface.Index),
		syscall.IFF_UP, syscall.IFF_UP)
	msg := nlMsg(syscall.RTM_NEWLINK, syscall.NLM_F_REQUEST|syscall.NLM_F_ACK, body)
	s, err := openNL()
	if err != nil {
		return err
	}
	defer s.close()
	return s.do(msg)
}

// addAddr assigns cidr (e.g. "172.20.0.1/24") to the named interface.
func addAddr(name, cidr string) error {
	ip, ipnet, err := net.ParseCIDR(cidr)
	if err != nil {
		return err
	}
	prefixLen, _ := ipnet.Mask.Size()
	iface, err := net.InterfaceByName(name)
	if err != nil {
		return err
	}

	body := cat(
		mkIfAddrmsg(syscall.AF_INET, uint8(prefixLen), uint32(iface.Index)),
		// IFA_LOCAL  = unicast address on this end
		// IFA_ADDRESS = broadcast address (same as local for /30 or smaller,
		//               but always set both for compatibility)
		nlAttrIP(ifaLocal, ip),
		nlAttrIP(ifaAddress, ip),
	)
	msg := nlMsg(syscall.RTM_NEWADDR,
		syscall.NLM_F_REQUEST|syscall.NLM_F_ACK|syscall.NLM_F_CREATE,
		body)
	s, err := openNL()
	if err != nil {
		return err
	}
	defer s.close()
	return s.do(msg)
}

// addDefaultRoute adds "default via gateway dev iface" (0.0.0.0/0).
func addDefaultRoute(iface, gateway string) error {
	gw := net.ParseIP(gateway).To4()
	if gw == nil {
		return fmt.Errorf("invalid gateway IP: %s", gateway)
	}
	ifaceObj, err := net.InterfaceByName(iface)
	if err != nil {
		return err
	}
	body := cat(
		mkRtmsg(syscall.AF_INET),
		nlAttrIP(rtaGateway, gw),
		nlAttrU32(rtaOif, uint32(ifaceObj.Index)),
	)
	msg := nlMsg(syscall.RTM_NEWROUTE,
		syscall.NLM_F_REQUEST|syscall.NLM_F_ACK|syscall.NLM_F_CREATE,
		body)
	s, err := openNL()
	if err != nil {
		return err
	}
	defer s.close()
	return s.do(msg)
}

// ─── Package API ──────────────────────────────────────────────────────────────

// VethHostIface returns the host-side veth interface name for the given
// container PID, e.g. "veth-h1234".
func VethHostIface(pid int) string { return fmt.Sprintf("veth-h%d", pid) }

// vethPeerName is the fixed name of the peer inside every container's netns.
// It is always renamed to "eth0" by the child init before use.
const vethPeerName = "veth-peer"

// SetupVethHost is called by the parent after the child process starts.
// It creates the veth pair, assigns hostCIDR to the host end, and moves the
// peer into the container's network namespace (identified by containerPID).
//
// Must be called while still holding the sync pipe open (before the child
// proceeds past its wait).
func SetupVethHost(containerPID int, hostCIDR string, debug bool) error {
	host := VethHostIface(containerPID)

	if debug {
		fmt.Printf("[parent] veth: creating pair %s ↔ %s\n", host, vethPeerName)
	}
	if err := createVethPair(host, vethPeerName); err != nil {
		return fmt.Errorf("create veth pair: %w", err)
	}
	if err := addAddr(host, hostCIDR); err != nil {
		return fmt.Errorf("host addr: %w", err)
	}
	if err := setLinkUp(host); err != nil {
		return fmt.Errorf("host link up: %w", err)
	}
	if err := moveToNetns(vethPeerName, containerPID); err != nil {
		return fmt.Errorf("move peer to container netns: %w", err)
	}
	if debug {
		fmt.Printf("[parent] veth: host side %s ready (%s)\n", host, hostCIDR)
	}
	return nil
}

// SetupVethContainer is called inside ContainerInit, before pivot_root.
// It renames the peer to eth0, assigns containerCIDR, and adds a default
// route via gateway so the container can reach the host and beyond.
func SetupVethContainer(containerCIDR, gateway string, debug bool) error {
	if debug {
		fmt.Printf("[init] veth: configuring %s → eth0 (%s, gw %s)\n",
			vethPeerName, containerCIDR, gateway)
	}

	// The peer arrives DOWN with the name vethPeerName.
	// Rename it first (kernel rejects renaming UP interfaces).
	if err := renameIface(vethPeerName, "eth0"); err != nil {
		return fmt.Errorf("rename to eth0: %w", err)
	}
	if err := addAddr("eth0", containerCIDR); err != nil {
		return fmt.Errorf("container addr: %w", err)
	}
	if err := setLinkUp("eth0"); err != nil {
		return fmt.Errorf("eth0 up: %w", err)
	}
	if err := addDefaultRoute("eth0", gateway); err != nil {
		return fmt.Errorf("default route: %w", err)
	}
	if debug {
		fmt.Println("[init] veth: eth0 up, default route via", gateway)
	}
	return nil
}

// SetupNAT enables internet access for bridge-networked containers by:
//  1. Writing "1" to /proc/sys/net/ipv4/ip_forward so the kernel routes
//     packets between the host's physical interface and the veth bridge.
//  2. Installing an iptables MASQUERADE rule on the POSTROUTING chain so
//     outbound container packets are source-NATted to the host's IP before
//     leaving.  Without this the return packets would have no route back to
//     the container's private 172.20.0.x address.
//
// The iptables rule is idempotent: we check with -C before adding with -A.
// The ip_forward sysctl persists only until reboot; iptables rules persist
// until the next `iptables -F` or reboot (depending on distro config).
func SetupNAT(subnet string, debug bool) error {
	if err := os.WriteFile("/proc/sys/net/ipv4/ip_forward", []byte("1\n"), 0644); err != nil {
		return fmt.Errorf("enable ip_forward: %w", err)
	}

	rule := []string{"-t", "nat", "-A", "POSTROUTING",
		"-s", subnet, "!", "-d", subnet, "-j", "MASQUERADE"}
	check := []string{"-t", "nat", "-C", "POSTROUTING",
		"-s", subnet, "!", "-d", subnet, "-j", "MASQUERADE"}

	if _, err := runTrustedHostTool("iptables", check...); err != nil {
		// Rule absent — add it.
		if out, err := runTrustedHostTool("iptables", rule...); err != nil {
			return fmt.Errorf("iptables MASQUERADE: %w\n%s", err, out)
		}
	}

	if debug {
		fmt.Printf("[parent] NAT: ip_forward=1, MASQUERADE rule for %s\n", subnet)
	}
	return nil
}
