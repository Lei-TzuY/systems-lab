//go:build linux

package network

import (
	"crypto/rand"
	"encoding/hex"
	"errors"
	"fmt"
	"os/exec"
	"strconv"
	"strings"
)

const portForwardingOwnerPrefix = "minicontainer:"

// NewPortForwardingOwner returns a generation-scoped marker for iptables rules.
// Cleanup includes this marker in the rule specification so an identical rule
// installed by another runtime instance cannot be mistaken for ours.
func NewPortForwardingOwner() (string, error) {
	var nonce [16]byte
	if _, err := rand.Read(nonce[:]); err != nil {
		return "", fmt.Errorf("generate port-forwarding owner: %w", err)
	}
	return portForwardingOwnerPrefix + hex.EncodeToString(nonce[:]), nil
}

func validatePortForwardingOwner(owner string) error {
	if owner == "" {
		return fmt.Errorf("port-forwarding owner is empty")
	}
	if len(owner) > 128 {
		return fmt.Errorf("port-forwarding owner is too long")
	}
	if strings.ContainsRune(owner, '\x00') {
		return fmt.Errorf("port-forwarding owner contains NUL")
	}
	return nil
}

// SetupPortForwardingOwned installs generation-tagged DNAT rules.
func SetupPortForwardingOwned(owner string, hostPort, containerPort int, containerIP, protocol string, debug bool) error {
	return setupPortForwardingOwnedWith(owner, hostPort, containerPort, containerIP, protocol, debug, runIPTables)
}

func setupPortForwardingOwnedWith(owner string, hostPort, containerPort int, containerIP, protocol string, debug bool, run iptablesCommand) error {
	if run == nil {
		return fmt.Errorf("iptables command runner is nil")
	}
	if err := validatePortForwardingOwner(owner); err != nil {
		return err
	}
	if protocol == "" {
		protocol = "tcp"
	}

	target := fmt.Sprintf("%s:%d", containerIP, containerPort)
	portStr := strconv.Itoa(hostPort)
	ownerArgs := []string{"-m", "comment", "--comment", owner}

	preroutingAdd := append([]string{"-t", "nat", "-A", "PREROUTING", "-p", protocol, "--dport", portStr}, ownerArgs...)
	preroutingAdd = append(preroutingAdd, "-j", "DNAT", "--to-destination", target)
	outputAdd := append([]string{"-t", "nat", "-A", "OUTPUT", "-p", protocol, "-m", "addrtype", "--dst-type", "LOCAL", "--dport", portStr}, ownerArgs...)
	outputAdd = append(outputAdd, "-j", "DNAT", "--to-destination", target)
	preroutingDelete := append([]string{"-t", "nat", "-D", "PREROUTING", "-p", protocol, "--dport", portStr}, ownerArgs...)
	preroutingDelete = append(preroutingDelete, "-j", "DNAT", "--to-destination", target)

	if out, err := run(preroutingAdd...); err != nil {
		return fmt.Errorf("iptables PREROUTING DNAT: %w\n%s", err, out)
	}
	if out, err := run(outputAdd...); err != nil {
		setupErr := fmt.Errorf("iptables OUTPUT DNAT: %w\n%s", err, out)
		if rollbackOut, rollbackErr := run(preroutingDelete...); rollbackErr != nil {
			return errors.Join(
				setupErr,
				fmt.Errorf("rollback owned PREROUTING DNAT: %w\n%s", rollbackErr, rollbackOut),
			)
		}
		return setupErr
	}

	if debug {
		fmt.Printf("[parent] port mapping: host %s/%d → container %s (%s)\n", protocol, hostPort, target, owner)
	}
	return nil
}

func iptablesRuleAbsent(err error) bool {
	var exitErr *exec.ExitError
	return errors.As(err, &exitErr) && exitErr.ExitCode() == 1
}

func removeOwnedRuleIfPresent(label string, checkArgs, deleteArgs []string, run iptablesCommand) error {
	out, err := run(checkArgs...)
	if err != nil {
		if iptablesRuleAbsent(err) {
			return nil
		}
		return fmt.Errorf("iptables check owned %s: %w\n%s", label, err, out)
	}
	if out, err := run(deleteArgs...); err != nil {
		return fmt.Errorf("iptables delete owned %s: %w\n%s", label, err, out)
	}
	return nil
}

// RemovePortForwardingOwned deletes only the DNAT rules carrying owner. Missing
// rules are idempotent success after an explicit iptables -C check. This lets a
// persisted ownership intent safely describe rules that may only have been
// partially installed when a runtime process crashed.
func RemovePortForwardingOwned(owner string, hostPort, containerPort int, containerIP, protocol string, debug bool) error {
	return removePortForwardingOwnedWith(owner, hostPort, containerPort, containerIP, protocol, debug, runIPTables)
}

func removePortForwardingOwnedWith(owner string, hostPort, containerPort int, containerIP, protocol string, debug bool, run iptablesCommand) error {
	if run == nil {
		return fmt.Errorf("iptables command runner is nil")
	}
	if err := validatePortForwardingOwner(owner); err != nil {
		return err
	}
	if protocol == "" {
		protocol = "tcp"
	}

	target := fmt.Sprintf("%s:%d", containerIP, containerPort)
	portStr := strconv.Itoa(hostPort)
	ownerArgs := []string{"-m", "comment", "--comment", owner}

	preroutingCheck := append([]string{"-t", "nat", "-C", "PREROUTING", "-p", protocol, "--dport", portStr}, ownerArgs...)
	preroutingCheck = append(preroutingCheck, "-j", "DNAT", "--to-destination", target)
	preroutingDelete := append([]string{"-t", "nat", "-D", "PREROUTING", "-p", protocol, "--dport", portStr}, ownerArgs...)
	preroutingDelete = append(preroutingDelete, "-j", "DNAT", "--to-destination", target)
	outputCheck := append([]string{"-t", "nat", "-C", "OUTPUT", "-p", protocol, "-m", "addrtype", "--dst-type", "LOCAL", "--dport", portStr}, ownerArgs...)
	outputCheck = append(outputCheck, "-j", "DNAT", "--to-destination", target)
	outputDelete := append([]string{"-t", "nat", "-D", "OUTPUT", "-p", protocol, "-m", "addrtype", "--dst-type", "LOCAL", "--dport", portStr}, ownerArgs...)
	outputDelete = append(outputDelete, "-j", "DNAT", "--to-destination", target)

	var cleanupErrs []error
	if err := removeOwnedRuleIfPresent("PREROUTING DNAT", preroutingCheck, preroutingDelete, run); err != nil {
		cleanupErrs = append(cleanupErrs, err)
	}
	if err := removeOwnedRuleIfPresent("OUTPUT DNAT", outputCheck, outputDelete, run); err != nil {
		cleanupErrs = append(cleanupErrs, err)
	}
	if err := errors.Join(cleanupErrs...); err != nil {
		return err
	}

	if debug {
		fmt.Printf("[parent] cleaned up owned port mapping: host %s/%d (%s)\n", protocol, hostPort, owner)
	}
	return nil
}
