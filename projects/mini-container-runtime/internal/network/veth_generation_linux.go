//go:build linux

package network

import (
	"crypto/sha256"
	"encoding/base32"
	"fmt"
	"strings"
	"syscall"
)

const (
	// linux/uapi/linux/if_link.h. IFALIAS is stored atomically with RTM_NEWLINK
	// so a pre-persisted cleanup intent can distinguish our veth from a foreign
	// interface that happened to occupy the same name before creation.
	iflaIfAlias = 20

	linuxInterfaceNameMax = 15 // IFNAMSIZ includes the trailing NUL byte.
	ownedVethPrefix       = "vh"
)

func validateGenerationNetworkOwner(owner string) error {
	if !strings.HasPrefix(owner, portForwardingOwnerPrefix) {
		return fmt.Errorf("invalid generation network owner %q", owner)
	}
	if len(owner) <= len(portForwardingOwnerPrefix) || len(owner) > 128 {
		return fmt.Errorf("invalid generation network owner length %d", len(owner))
	}
	for _, r := range owner[len(portForwardingOwnerPrefix):] {
		if !((r >= 'a' && r <= 'z') || (r >= 'A' && r <= 'Z') || (r >= '0' && r <= '9') || r == '-' || r == '_' || r == '.') {
			return fmt.Errorf("invalid generation network owner character %q", r)
		}
	}
	return nil
}

// VethHostIfaceOwned derives a generation-scoped host interface name from the
// durable network owner. The 64-bit digest fragment fits Linux IFNAMSIZ while
// making cross-generation name reuse independent of host PID reuse.
func VethHostIfaceOwned(owner string) string {
	sum := sha256.Sum256([]byte(owner))
	encoded := base32.StdEncoding.WithPadding(base32.NoPadding).EncodeToString(sum[:8])
	return ownedVethPrefix + strings.ToLower(encoded)
}

func validateOwnedVethName(name string) error {
	if len(name) != linuxInterfaceNameMax || !strings.HasPrefix(name, ownedVethPrefix) {
		return fmt.Errorf("invalid owned veth name %q", name)
	}
	for _, r := range name[len(ownedVethPrefix):] {
		if !((r >= 'a' && r <= 'z') || (r >= '2' && r <= '7')) {
			return fmt.Errorf("invalid owned veth name %q", name)
		}
	}
	return nil
}

func ownedVethPairBody(name, peer, owner string) ([]byte, error) {
	if err := validateGenerationNetworkOwner(owner); err != nil {
		return nil, fmt.Errorf("validate veth owner: %w", err)
	}
	if err := validateOwnedVethName(name); err != nil {
		return nil, err
	}
	if expected := VethHostIfaceOwned(owner); name != expected {
		return nil, fmt.Errorf("owned veth name %q does not match generation owner (want %q)", name, expected)
	}
	peerBlock := cat(
		mkIfInfomsg(syscall.AF_UNSPEC, 0, 0, 0),
		nlAttrStr(syscall.IFLA_IFNAME, peer),
	)
	return cat(
		mkIfInfomsg(syscall.AF_UNSPEC, 0, 0, 0),
		nlAttrStr(syscall.IFLA_IFNAME, name),
		nlAttrStr(iflaIfAlias, owner),
		nlAttr(iflaLinkinfo, cat(
			nlAttrStr(iflaInfoKind, "veth"),
			nlAttr(iflaInfoData, nlAttr(vethInfoPeer, peerBlock)),
		)),
	), nil
}

// createVethPairOwned creates a host veth whose kernel ifalias carries the exact
// generation owner in the same atomic link-creation request.
func createVethPairOwned(name, peer, owner string) error {
	body, err := ownedVethPairBody(name, peer, owner)
	if err != nil {
		return err
	}
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
