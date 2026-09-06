package network

import (
	"fmt"
	"testing"
)

func TestIPAMAllocation(t *testing.T) {
	tmpHome := t.TempDir()
	t.Setenv("HOME", tmpHome)
	t.Setenv("USERPROFILE", tmpHome)

	ipam := NewIPAM()

	ip1, err := ipam.AllocateIP("demo-net", "172.28.0.0/24", "ctr1")
	if err != nil || ip1 != "172.28.0.2" {
		t.Fatalf("AllocateIP ctr1 = %s, err: %v (want 172.28.0.2)", ip1, err)
	}

	ip2, err := ipam.AllocateIP("demo-net", "172.28.0.0/24", "ctr2")
	if err != nil || ip2 != "172.28.0.3" {
		t.Fatalf("AllocateIP ctr2 = %s, err: %v (want 172.28.0.3)", ip2, err)
	}

	// Idempotent allocation for same container
	ip1Again, err := ipam.AllocateIP("demo-net", "172.28.0.0/24", "ctr1")
	if err != nil || ip1Again != ip1 {
		t.Fatalf("Re-AllocateIP ctr1 = %s, want %s", ip1Again, ip1)
	}

	// Release ctr1 IP
	if err := ipam.ReleaseIP("demo-net", "ctr1"); err != nil {
		t.Fatalf("ReleaseIP ctr1 error: %v", err)
	}

	// Allocate ctr3 should reuse 172.28.0.2
	ip3, err := ipam.AllocateIP("demo-net", "172.28.0.0/24", "ctr3")
	if err != nil || ip3 != "172.28.0.2" {
		t.Fatalf("AllocateIP ctr3 after release = %s, want 172.28.0.2", ip3)
	}
}

func TestIPAMBroadcastExclusion(t *testing.T) {
	tmpHome := t.TempDir()
	t.Setenv("HOME", tmpHome)
	t.Setenv("USERPROFILE", tmpHome)

	ipam := NewIPAM()

	// In a /30 subnet (10.0.0.0/30):
	// .0 = network, .1 = gateway, .2 = host, .3 = broadcast
	// Exactly 1 host IP (.2) is allocatable.
	ip1, err := ipam.AllocateIP("tiny-net", "10.0.0.0/30", "ctr1")
	if err != nil || ip1 != "10.0.0.2" {
		t.Fatalf("AllocateIP ctr1 in /30 = %s, err: %v (want 10.0.0.2)", ip1, err)
	}

	// 2nd allocation must fail because .3 is broadcast and cannot be allocated
	ip2, err := ipam.AllocateIP("tiny-net", "10.0.0.0/30", "ctr2")
	if err == nil {
		t.Fatalf("AllocateIP ctr2 in /30 succeeded with %s, expected subnet exhausted error", ip2)
	}

	// In a /29 subnet (192.168.1.0/29):
	// .0 = network, .1 = gateway, .2-.6 = hosts (5 hosts), .7 = broadcast
	ipam2 := NewIPAM()
	for i := 1; i <= 5; i++ {
		ctr := fmt.Sprintf("ctr%d", i)
		ip, err := ipam2.AllocateIP("sub29-net", "192.168.1.0/29", ctr)
		if err != nil {
			t.Fatalf("allocation %d failed: %v", i, err)
		}
		expectedIP := fmt.Sprintf("192.168.1.%d", i+1)
		if ip != expectedIP {
			t.Errorf("allocation %d = %s, want %s", i, ip, expectedIP)
		}
	}

	// 6th allocation must fail (not allocate 192.168.1.7 broadcast)
	if ip6, err := ipam2.AllocateIP("sub29-net", "192.168.1.0/29", "ctr6"); err == nil {
		t.Fatalf("6th allocation in /29 unexpectedly succeeded with %s (should reject broadcast)", ip6)
	}
}

func TestIPAMNetworkNameValidation(t *testing.T) {
	tmpHome := t.TempDir()
	t.Setenv("HOME", tmpHome)
	t.Setenv("USERPROFILE", tmpHome)

	ipam := NewIPAM()

	invalidNames := []string{
		"",
		".",
		"..",
		"../escape",
		"../../evil",
		"foo/bar",
		"foo\\bar",
		"colon:net",
	}

	for _, name := range invalidNames {
		if _, err := ipam.AllocateIP(name, "172.20.0.0/24", "ctr1"); err == nil {
			t.Errorf("AllocateIP(%q) expected error, got nil", name)
		}
		if err := ipam.ReleaseIP(name, "ctr1"); err == nil {
			t.Errorf("ReleaseIP(%q) expected error, got nil", name)
		}
	}
}
