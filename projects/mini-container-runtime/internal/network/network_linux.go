//go:build linux

// internal/network/network_linux.go
//
// Network Namespace Setup
// ────────────────────────
// When a container is created with CLONE_NEWNET it gets a brand-new network
// stack that contains:
//
//   • A loopback device (lo)  — created by the kernel, but initially DOWN.
//   • No other interfaces.
//
// Without additional setup, nothing inside the container can open a network
// connection — not even 127.0.0.1, because lo is down.
//
// SetupLoopback brings lo up using the SIOCSIFFLAGS ioctl, mirroring what
// "ip link set lo up" does at the library level.
//
// For a production runtime you'd also:
//   1. Create a veth pair on the host (ip link add veth0 type veth peer vethN).
//   2. Move one end into the container namespace (ip link set vethN netns <pid>).
//   3. Assign IP addresses and set up NAT/masquerade on the host side.
// Those steps are intentionally out of scope here to keep the code readable.
//
// Relevant kernel interface
// ─────────────────────────
//   SIOCGIFFLAGS (0x8913) – read interface flags into struct ifreq
//   SIOCSIFFLAGS (0x8914) – write interface flags from struct ifreq
//   IFF_UP       (0x0001) – the "interface up" flag bit

package network

import (
	"fmt"
	"syscall"
	"unsafe"
)

// ifreq is the kernel's struct ifreq layout for flag operations.
// Total size must be exactly 40 bytes to match the C struct layout.
//
//	struct ifreq {
//	    char   ifr_name[IFNAMSIZ]; // 16 bytes — interface name, NUL-terminated
//	    union {
//	        short ifr_flags;       //  2 bytes — interface flags
//	        ...
//	    };                         // 24 bytes of union padding
//	};
type ifreq struct {
	name  [16]byte
	flags uint16
	_     [22]byte // padding to reach 40 bytes
}

// SetupLoopback brings the loopback interface (lo) to the UP state inside the
// current network namespace.  It uses raw ioctls so it works without any
// external binaries (no dependency on /sbin/ip or iproute2).
func SetupLoopback(debug bool) error {
	// Open a temporary UDP socket — we need a file descriptor to pass to
	// ioctl(2).  The socket is not used for any I/O; it's just a handle.
	// SOCK_CLOEXEC prevents the fd from leaking into child processes.
	fd, err := syscall.Socket(
		syscall.AF_INET,
		syscall.SOCK_DGRAM|syscall.SOCK_CLOEXEC,
		0,
	)
	if err != nil {
		return fmt.Errorf("socket for ioctl: %w", err)
	}
	defer syscall.Close(fd)

	var ifr ifreq
	copy(ifr.name[:], "lo") // target interface name

	// SIOCGIFFLAGS — read lo's current flags into ifr.flags.
	if _, _, errno := syscall.Syscall(
		syscall.SYS_IOCTL,
		uintptr(fd),
		syscall.SIOCGIFFLAGS,
		uintptr(unsafe.Pointer(&ifr)),
	); errno != 0 {
		return fmt.Errorf("SIOCGIFFLAGS lo: %w", errno)
	}

	// Set the IFF_UP bit.  Bitwise-OR ensures we don't clobber other flags
	// (e.g. IFF_LOOPBACK is already set by the kernel).
	ifr.flags |= syscall.IFF_UP

	// SIOCSIFFLAGS — write the updated flags back.
	if _, _, errno := syscall.Syscall(
		syscall.SYS_IOCTL,
		uintptr(fd),
		syscall.SIOCSIFFLAGS,
		uintptr(unsafe.Pointer(&ifr)),
	); errno != 0 {
		return fmt.Errorf("SIOCSIFFLAGS lo: %w", errno)
	}

	if debug {
		fmt.Println("[net] loopback (lo) interface brought UP")
	}
	return nil
}
