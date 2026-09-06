//go:build linux

// internal/ns/ns_linux.go
//
// Namespace Configuration
// ────────────────────────
// Linux namespaces are the fundamental building block of container isolation.
// Each namespace wraps one aspect of the global OS state and gives a process
// its own private view of that resource.
//
// Namespaces we create (passed as clone(2) flags):
//
//   CLONE_NEWUSER — User namespace  (optional, enabled by default)
//     The most important security namespace.  It gives the container its own
//     UID/GID number space and grants a full capability set *within* that
//     namespace, so a process that is unprivileged on the host can act as
//     "root" inside the container without any real root privilege.
//
//     uid_map / gid_map protocol
//     ──────────────────────────
//     After clone(2), the parent must write a UID mapping into
//     /proc/<child-pid>/uid_map (and the GID mapping into gid_map) before
//     the child's capabilities take effect.  The file format is:
//
//       <container-id>  <host-id>  <count>
//
//     e.g. "0  1000  1" ≡ container UID 0 (root) maps to host UID 1000.
//
//     setgroups restriction
//     ─────────────────────
//     An unprivileged parent must write "deny" to /proc/<pid>/setgroups
//     BEFORE writing gid_map.  Omitting this causes EPERM on the gid_map
//     write.  Go's runtime handles both writes automatically when
//     SysProcAttr.UidMappings is set and GidMappingsEnableSetgroups is
//     false (the default).
//
//   CLONE_NEWPID  — PID namespace
//     The first process in the namespace becomes PID 1.  Host processes are
//     invisible.  If PID 1 exits, every other process in the namespace is
//     killed.
//
//   CLONE_NEWUTS  — UTS namespace (hostname + NIS domain)
//     "UTS" comes from the Unix Time-sharing System struct in the kernel.
//     Gives each container its own hostname; sethostname(2) inside the
//     container won't affect the host or other containers.
//
//   CLONE_NEWNS   — Mount namespace
//     The first namespace ever added to Linux (2002).  Each container has
//     its own mount table so mounts don't propagate to the host.
//     Combined with pivot_root/chroot this achieves full FS isolation.
//
//   CLONE_NEWNET  — Network namespace
//     Own network stack: interfaces, routing table, iptables rules, socket
//     table, port numbers.  A fresh namespace contains only a loopback
//     device (lo) in the DOWN state.
//
//   CLONE_NEWIPC  — IPC namespace
//     Isolates System V IPC objects (semaphores, shared memory, message
//     queues) and POSIX message queues.  Containers can't signal each other
//     via IPC primitives.
//
// Namespaces NOT created here (out of scope for this runtime):
//   CLONE_NEWTIME   – per-container CLOCK_MONOTONIC offset (kernel ≥ 5.6)
//   CLONE_NEWCGROUP – hides host cgroup hierarchy (kernel ≥ 4.6)

package ns

import "syscall"

// Options controls which namespaces are created for the container.
type Options struct {
	// UserNS adds CLONE_NEWUSER and maps HostUID/HostGID to UID/GID 0 inside
	// the container.  This lets unprivileged users create all other namespaces
	// without root and is the primary UID-isolation boundary.
	UserNS  bool
	HostUID int // host UID that maps to container root (UID 0)
	HostGID int // host GID that maps to container root (GID 0)
}

// BuildCloneFlags returns the SysProcAttr that will make exec.Cmd create
// new namespaces when it forks the child process via clone(2).
//
// The Cloneflags field is passed verbatim to clone(2); the kernel creates a
// new namespace for each flag and attaches it before the child runs its first
// instruction.  When UserNS is true the runtime also writes uid_map, gid_map,
// and setgroups on behalf of the caller.
func BuildCloneFlags(opts Options) *syscall.SysProcAttr {
	attr := &syscall.SysProcAttr{
		Cloneflags: syscall.CLONE_NEWPID | // own PID space → container sees PID 1
			syscall.CLONE_NEWUTS | // own hostname
			syscall.CLONE_NEWNS | // own mount table
			syscall.CLONE_NEWNET | // own network stack
			syscall.CLONE_NEWIPC, // own SysV IPC / POSIX MQ

		// Pdeathsig ensures the child is killed if the parent dies unexpectedly
		// (e.g. the user presses Ctrl-C before the child has exec'd its payload).
		Pdeathsig: syscall.SIGKILL,
	}

	if opts.UserNS {
		// CLONE_NEWUSER must be combined with the other namespace flags in the
		// same clone(2) call.  The kernel creates the user namespace first,
		// granting the new process a full capability set within it, which is
		// what allows it to create the other namespaces without real root.
		attr.Cloneflags |= syscall.CLONE_NEWUSER

		// Map the invoking host UID/GID to root (0/0) inside the container.
		// Size: 1 means exactly one ID is mapped (only root is valid inside).
		attr.UidMappings = []syscall.SysProcIDMap{
			{ContainerID: 0, HostID: opts.HostUID, Size: 1},
		}
		attr.GidMappings = []syscall.SysProcIDMap{
			{ContainerID: 0, HostID: opts.HostGID, Size: 1},
		}
		// GidMappingsEnableSetgroups defaults to false → Go writes "deny" to
		// /proc/<pid>/setgroups before gid_map, satisfying the kernel's
		// unprivileged-parent restriction.
	}

	return attr
}
