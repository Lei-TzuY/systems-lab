// internal/container/config.go
//
// Container configuration — the specification of what to run and how.

package container

import "os"

// Volume describes one host directory bind-mounted into the container.
type Volume struct {
	HostPath      string // absolute path on the host
	ContainerPath string // absolute path inside the container (relative to rootfs)
	ReadOnly      bool
}

// PortMapping describes a host-to-container port forwarding rule.
type PortMapping struct {
	HostPort      int
	ContainerPort int
	Protocol      string // "tcp" or "udp"
}

// Config holds everything our runtime needs to launch one container.
type Config struct {
	// ContainerID is the unique ID assigned to the container.
	ContainerID string

	// StateDir optionally overrides the persistent runtime state directory used
	// for lifecycle transitions. Empty uses state.DefaultDir(). It is parent-side
	// runtime metadata and is not propagated into the container process.
	StateDir string

	// RootFS is the path to the directory that will become the container's root.
	RootFS string

	// RootFSIdentity is the parent-side filesystem object observed when a managed
	// run was admitted. It is never propagated to the child; each runtime attempt
	// uses it to fail closed if RootFS was replaced before process creation.
	RootFSIdentity os.FileInfo

	// Overlay enables OverlayFS CoW layer isolation.
	Overlay bool

	// ReadOnly mounts the container rootfs as read-only.
	ReadOnly bool

	// Restart specifies restart policy: "no", "always", or "on-failure".
	Restart string

	// CapDrop is a list of Linux Capabilities to drop from the bounding set (e.g. CAP_SYS_ADMIN).
	CapDrop []string

	// Command is the executable and its arguments to run inside the container.
	Command []string

	// Hostname is the UTS name seen inside the container.
	Hostname string

	// WorkDir is the working directory inside the container after pivot_root.
	WorkDir string

	// Env is a list of environment variables (e.g. "KEY=VALUE") to inject.
	Env []string

	// Memory is the memory limit in bytes enforced via cgroups.
	Memory int64

	// CPUWeight is the cgroup v2 CPU weight in the range 1..10000.
	CPUWeight int64

	// CPUs is the hard fractional CPU limit (e.g. 0.5 = 50% CPU, 2.0 = 2 CPUs).
	CPUs float64

	// PidsLimit is the maximum number of processes inside the container.
	PidsLimit int64

	// Seccomp enables the BPF syscall block-list filter.
	Seccomp bool

	// BridgeNetwork enables veth pair networking.
	BridgeNetwork bool

	// PortMappings is the list of published host-to-container ports.
	PortMappings []PortMapping

	// Volumes is the list of host directories to bind-mount into the container.
	Volumes []Volume

	// UserNS enables user namespace isolation (CLONE_NEWUSER).
	UserNS bool

	// Debug enables verbose logging of every significant syscall.
	Debug bool
}
