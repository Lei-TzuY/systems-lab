//go:build linux

package network

import "testing"

func TestIFLANetNsPIDMatchesLinuxUAPI(t *testing.T) {
	// linux/uapi/linux/if_link.h assigns IFLA_NET_NS_PID = 19.
	// Value 28 is IFLA_NET_NS_FD and would make the kernel interpret a PID as
	// a namespace file descriptor.
	const wantType = 19
	if iflaNetNsPid != wantType {
		t.Fatalf("IFLA_NET_NS_PID=%d, want Linux UAPI value %d", iflaNetNsPid, wantType)
	}

	const pid = 4242
	attr := nlAttrU32(iflaNetNsPid, pid)
	if got := nativeEndian.Uint16(attr[2:4]); got != wantType {
		t.Fatalf("serialized netns attribute type=%d, want %d", got, wantType)
	}
	if got := nativeEndian.Uint32(attr[4:8]); got != pid {
		t.Fatalf("serialized netns PID=%d, want %d", got, pid)
	}
}
