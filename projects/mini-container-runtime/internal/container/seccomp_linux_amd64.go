//go:build linux && amd64

package container

// AUDIT_ARCH_X86_64 from linux/audit.h.
const auditArch uint32 = 0xc000003e

// blockedSyscalls for x86-64 (amd64).
// Numbers sourced from <asm/unistd_64.h> in the Linux kernel.
var blockedSyscalls = []uint32{
	246, // kexec_load
	320, // kexec_file_load
	101, // ptrace
	169, // reboot
	103, // syslog
	175, // init_module
	313, // finit_module
	176, // delete_module
	174, // create_module
	172, // iopl
	173, // ioperm
	164, // settimeofday
	227, // clock_settime
	404, // clock_settime64
	165, // mount
	166, // umount2
	155, // pivot_root
	167, // swapon
	168, // swapoff
	163, // acct
	248, // add_key
	249, // request_key
	250, // keyctl
	321, // bpf
	298, // perf_event_open
	310, // process_vm_readv
	311, // process_vm_writev
	304, // open_by_handle_at
	300, // fanotify_init
	323, // userfaultfd
	272, // unshare
}
