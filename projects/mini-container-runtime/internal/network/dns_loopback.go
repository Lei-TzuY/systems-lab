package network

// GenerateDNSLoopbackConfig formats systemd-resolved loopback DNS entries.
func GenerateDNSLoopbackConfig() string {
	return "nameserver 127.0.0.53\n"
}
