//go:build linux

package network

import (
	"errors"
	"fmt"
	"net"
	"os"
	"path/filepath"
	"strings"
	"syscall"
)

type vethInterfaceLister func() ([]net.Interface, error)
type vethLinkDeleter func(name string, index int) error
type vethAliasReader func(name string) (string, error)

// RemoveVethHost deletes the host side of a container veth pair. Deleting one
// end of a veth pair removes its peer as well, including when the peer has
// already been moved into the container network namespace.
//
// Missing interfaces are treated as already-cleaned. Interface enumeration
// failures are not equivalent to absence and are reported so teardown cannot
// silently succeed without checking whether the owned link still exists.
func RemoveVethHost(containerPID int, debug bool) error {
	return removeVethHostWith(containerPID, debug, net.Interfaces, deleteVethLink)
}

func removeVethHostWith(containerPID int, debug bool, list vethInterfaceLister, deleteLink vethLinkDeleter) error {
	if list == nil || deleteLink == nil {
		return fmt.Errorf("veth cleanup operation is nil")
	}

	name := VethHostIface(containerPID)
	iface, err := findVethInterface(name, list)
	if err != nil {
		return err
	}
	if iface == nil {
		if debug {
			fmt.Printf("[parent] veth cleanup: %s already absent\n", name)
		}
		return nil
	}

	if err := deleteLink(name, iface.Index); err != nil {
		return fmt.Errorf("delete host veth %s: %w", name, err)
	}
	if debug {
		fmt.Printf("[parent] veth cleanup: removed %s\n", name)
	}
	return nil
}

// RemoveVethHostOwned removes only a host veth whose generated name and kernel
// ifalias both prove the exact generation owner. A pre-persisted cleanup intent
// may describe a link that was never created; absence and a same-name interface
// carrying another alias are therefore safe no-ops rather than reasons to
// delete by name alone.
func RemoveVethHostOwned(name, owner string, debug bool) error {
	return removeVethHostOwnedWith(name, owner, debug, net.Interfaces, readVethAlias, deleteVethLink)
}

func removeVethHostOwnedWith(
	name, owner string,
	debug bool,
	list vethInterfaceLister,
	readAlias vethAliasReader,
	deleteLink vethLinkDeleter,
) error {
	if list == nil || readAlias == nil || deleteLink == nil {
		return fmt.Errorf("owned veth cleanup operation is nil")
	}
	if err := validateOwnedVethName(name); err != nil {
		return err
	}
	if err := validateGenerationNetworkOwner(owner); err != nil {
		return fmt.Errorf("validate veth owner: %w", err)
	}
	if expected := VethHostIfaceOwned(owner); name != expected {
		return fmt.Errorf("owned veth name %q does not match generation owner (want %q)", name, expected)
	}

	iface, err := findVethInterface(name, list)
	if err != nil {
		return err
	}
	if iface == nil {
		if debug {
			fmt.Printf("[parent] owned veth cleanup: %s already absent\n", name)
		}
		return nil
	}

	alias, err := readAlias(name)
	if err != nil {
		if errors.Is(err, os.ErrNotExist) {
			// The interface disappeared after enumeration. Deletion by another
			// lifecycle actor is idempotent success.
			return nil
		}
		return fmt.Errorf("read owner alias for host veth %s: %w", name, err)
	}
	if alias != owner {
		if debug {
			fmt.Printf("[parent] owned veth cleanup: %s belongs to another owner; leaving it intact\n", name)
		}
		return nil
	}

	// Re-enumerate after the pathname-based alias read. Do not use the stale
	// index if the named link disappeared or was replaced between observations.
	current, err := findVethInterface(name, list)
	if err != nil {
		return err
	}
	if current == nil {
		return nil
	}
	if current.Index != iface.Index {
		return fmt.Errorf("host veth %s changed interface identity during cleanup (%d -> %d)", name, iface.Index, current.Index)
	}

	// The interface can retain the same name/index while its ownership alias is
	// changed between our first proof and deletion. Re-check immediately before
	// the destructive netlink operation so ownership is validated at the final
	// observable boundary rather than only against an earlier snapshot.
	alias, err = readAlias(name)
	if err != nil {
		if errors.Is(err, os.ErrNotExist) {
			return nil
		}
		return fmt.Errorf("re-read owner alias for host veth %s: %w", name, err)
	}
	if alias != owner {
		return fmt.Errorf("host veth %s ownership changed during cleanup", name)
	}

	if err := deleteLink(name, current.Index); err != nil {
		return fmt.Errorf("delete owned host veth %s: %w", name, err)
	}
	if debug {
		fmt.Printf("[parent] owned veth cleanup: removed %s (%s)\n", name, owner)
	}
	return nil
}

func findVethInterface(name string, list vethInterfaceLister) (*net.Interface, error) {
	ifaces, err := list()
	if err != nil {
		return nil, fmt.Errorf("list interfaces for veth cleanup %s: %w", name, err)
	}
	for i := range ifaces {
		if ifaces[i].Name == name {
			iface := ifaces[i]
			return &iface, nil
		}
	}
	return nil, nil
}

func readVethAlias(name string) (string, error) {
	if err := validateOwnedVethName(name); err != nil {
		return "", err
	}
	data, err := os.ReadFile(filepath.Join("/sys/class/net", name, "ifalias"))
	if err != nil {
		return "", err
	}
	return strings.TrimSpace(string(data)), nil
}

func deleteVethLink(name string, index int) error {
	body := mkIfInfomsg(syscall.AF_UNSPEC, int32(index), 0, 0)
	msg := nlMsg(syscall.RTM_DELLINK, syscall.NLM_F_REQUEST|syscall.NLM_F_ACK, body)
	s, err := openNL()
	if err != nil {
		return fmt.Errorf("open netlink: %w", err)
	}
	defer s.close()
	if err := s.do(msg); err != nil {
		return err
	}
	return nil
}
