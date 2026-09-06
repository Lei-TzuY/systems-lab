//go:build linux

// internal/network/port_linux.go
//
// Port Forwarding / Port Mapping (`-p hostPort:containerPort`)
// ─────────────────────────────────────────────────────────────
// Port mapping forwards traffic arriving at a port on the host to a target
// port inside the container's network namespace (at container IP 172.20.0.2).
//
// Mechanism: iptables DNAT (Destination NAT)
// ───────────────────────────────────────────
//   1. Append a DNAT rule to the PREROUTING chain in the nat table.
//   2. Append an OUTPUT rule for host-local traffic.
//
// Setup is transactional: if the OUTPUT rule cannot be installed after the
// PREROUTING rule succeeds, the first rule is removed before returning an
// error. A caller therefore never observes success for a half-configured port
// mapping.

package network

import (
	"errors"
	"fmt"
	"strconv"
)

type iptablesCommand func(args ...string) ([]byte, error)

func runIPTables(args ...string) ([]byte, error) {
	return runTrustedHostTool("iptables", args...)
}

// SetupPortForwarding configures iptables DNAT rules for port mapping.
func SetupPortForwarding(hostPort, containerPort int, containerIP, protocol string, debug bool) error {
	return setupPortForwardingWith(hostPort, containerPort, containerIP, protocol, debug, runIPTables)
}

func setupPortForwardingWith(hostPort, containerPort int, containerIP, protocol string, debug bool, run iptablesCommand) error {
	if run == nil {
		return fmt.Errorf("iptables command runner is nil")
	}
	if protocol == "" {
		protocol = "tcp"
	}
	target := fmt.Sprintf("%s:%d", containerIP, containerPort)
	portStr := strconv.Itoa(hostPort)

	preroutingAdd := []string{"-t", "nat", "-A", "PREROUTING", "-p", protocol,
		"--dport", portStr, "-j", "DNAT", "--to-destination", target}
	outputAdd := []string{"-t", "nat", "-A", "OUTPUT", "-p", protocol,
		"-m", "addrtype", "--dst-type", "LOCAL",
		"--dport", portStr, "-j", "DNAT", "--to-destination", target}
	preroutingDelete := []string{"-t", "nat", "-D", "PREROUTING", "-p", protocol,
		"--dport", portStr, "-j", "DNAT", "--to-destination", target}

	if out, err := run(preroutingAdd...); err != nil {
		return fmt.Errorf("iptables PREROUTING DNAT: %w\n%s", err, out)
	}
	if out, err := run(outputAdd...); err != nil {
		setupErr := fmt.Errorf("iptables OUTPUT DNAT: %w\n%s", err, out)
		if rollbackOut, rollbackErr := run(preroutingDelete...); rollbackErr != nil {
			return errors.Join(
				setupErr,
				fmt.Errorf("rollback PREROUTING DNAT: %w\n%s", rollbackErr, rollbackOut),
			)
		}
		return setupErr
	}

	if debug {
		fmt.Printf("[parent] port mapping: host %s/%d → container %s\n", protocol, hostPort, target)
	}
	return nil
}

// RemovePortForwarding deletes both DNAT rules and reports every failed delete.
// Both commands are attempted so one stale rule cannot prevent cleanup of the
// other rule.
func RemovePortForwarding(hostPort, containerPort int, containerIP, protocol string, debug bool) error {
	return removePortForwardingWith(hostPort, containerPort, containerIP, protocol, debug, runIPTables)
}

func removePortForwardingWith(hostPort, containerPort int, containerIP, protocol string, debug bool, run iptablesCommand) error {
	if run == nil {
		return fmt.Errorf("iptables command runner is nil")
	}
	if protocol == "" {
		protocol = "tcp"
	}
	target := fmt.Sprintf("%s:%d", containerIP, containerPort)
	portStr := strconv.Itoa(hostPort)

	preroutingDelete := []string{"-t", "nat", "-D", "PREROUTING", "-p", protocol,
		"--dport", portStr, "-j", "DNAT", "--to-destination", target}
	outputDelete := []string{"-t", "nat", "-D", "OUTPUT", "-p", protocol,
		"-m", "addrtype", "--dst-type", "LOCAL",
		"--dport", portStr, "-j", "DNAT", "--to-destination", target}

	var cleanupErrs []error
	if out, err := run(preroutingDelete...); err != nil {
		cleanupErrs = append(cleanupErrs, fmt.Errorf("iptables delete PREROUTING DNAT: %w\n%s", err, out))
	}
	if out, err := run(outputDelete...); err != nil {
		cleanupErrs = append(cleanupErrs, fmt.Errorf("iptables delete OUTPUT DNAT: %w\n%s", err, out))
	}
	if err := errors.Join(cleanupErrs...); err != nil {
		return err
	}

	if debug {
		fmt.Printf("[parent] cleaned up port mapping: host %s/%d\n", protocol, hostPort)
	}
	return nil
}
