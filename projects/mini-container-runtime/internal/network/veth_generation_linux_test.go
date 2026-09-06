//go:build linux

package network

import (
	"bytes"
	"testing"
)

func TestVethHostIfaceOwnedIsStableGenerationScopedName(t *testing.T) {
	ownerA := "minicontainer:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
	ownerB := "minicontainer:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"

	nameA := VethHostIfaceOwned(ownerA)
	if nameA != VethHostIfaceOwned(ownerA) {
		t.Fatalf("owned veth name is not deterministic: %q", nameA)
	}
	if len(nameA) != linuxInterfaceNameMax {
		t.Fatalf("owned veth name length=%d, want %d: %q", len(nameA), linuxInterfaceNameMax, nameA)
	}
	if err := validateOwnedVethName(nameA); err != nil {
		t.Fatalf("generated owned veth name rejected: %v", err)
	}
	if nameA == VethHostIfaceOwned(ownerB) {
		t.Fatalf("different generation owners produced same veth name %q", nameA)
	}
}

func TestIFLAIfAliasMatchesLinuxUAPI(t *testing.T) {
	const wantType = 20
	if iflaIfAlias != wantType {
		t.Fatalf("IFLA_IFALIAS=%d, want Linux UAPI value %d", iflaIfAlias, wantType)
	}
}

func TestOwnedVethPairBodyCarriesExactGenerationAlias(t *testing.T) {
	owner := "minicontainer:0123456789abcdef0123456789abcdef"
	name := VethHostIfaceOwned(owner)
	body, err := ownedVethPairBody(name, vethPeerName, owner)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Contains(body, nlAttrStr(syscallIFLAIFNAMEForTest, name)) {
		t.Fatalf("owned veth body does not contain host name %q", name)
	}
	if !bytes.Contains(body, nlAttrStr(iflaIfAlias, owner)) {
		t.Fatalf("owned veth body does not contain exact owner alias %q", owner)
	}
}

func TestOwnedVethPairBodyRejectsInvalidIdentity(t *testing.T) {
	owner := "minicontainer:0123456789abcdef0123456789abcdef"
	if _, err := ownedVethPairBody("veth-h42", vethPeerName, owner); err == nil {
		t.Fatal("invalid generation veth name accepted")
	}
	if _, err := ownedVethPairBody(VethHostIfaceOwned(owner), vethPeerName, "bad-owner"); err == nil {
		t.Fatal("invalid generation owner accepted")
	}
}

// syscall.IFLA_IFNAME is 3 on Linux. Keeping the assertion local avoids adding
// another import only for a stable UAPI constant used by the message builder.
const syscallIFLAIFNAMEForTest = 3
