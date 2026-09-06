//go:build linux

package container

import (
	"fmt"
	"syscall"
	"unsafe"
)

const (
	prSetNoNewPrivs   = 38
	seccompModeFilter = 2
	seccompRetAllow   = 0x7fff0000
	seccompRetKill    = 0x80000000

	bpfLdWAbs  = 0x20
	bpfJmpJeqK = 0x15
	bpfRetK    = 0x06

	seccompDataNR   = 0
	seccompDataArch = 4
)

type bpfInsn struct {
	code uint16
	jt   uint8
	jf   uint8
	k    uint32
}

type bpfProg struct {
	len    uint16
	_      [6]byte
	filter *bpfInsn
}

// buildFilter returns a native-architecture cBPF syscall filter. The audit
// architecture check is deliberately first: syscall numbers are ABI-specific,
// so interpreting a foreign ABI against the native table must fail closed.
func buildFilter() []bpfInsn {
	n := len(blockedSyscalls)
	prog := make([]bpfInsn, 0, n*2+5)

	prog = append(prog,
		bpfInsn{code: bpfLdWAbs, k: seccompDataArch},
		bpfInsn{code: bpfJmpJeqK, jt: 1, jf: 0, k: auditArch},
		bpfInsn{code: bpfRetK, k: seccompRetKill},
		bpfInsn{code: bpfLdWAbs, k: seccompDataNR},
	)

	for _, nr := range blockedSyscalls {
		prog = append(prog,
			bpfInsn{code: bpfJmpJeqK, jt: 0, jf: 1, k: nr},
			bpfInsn{code: bpfRetK, k: seccompRetKill},
		)
	}
	prog = append(prog, bpfInsn{code: bpfRetK, k: seccompRetAllow})
	return prog
}

func applySeccomp(debug bool) error {
	if auditArch == 0 {
		return fmt.Errorf("seccomp is unsupported on this Linux architecture")
	}
	if _, _, errno := syscall.RawSyscall(syscall.SYS_PRCTL,
		prSetNoNewPrivs, 1, 0); errno != 0 {
		return fmt.Errorf("prctl(PR_SET_NO_NEW_PRIVS): %w", errno)
	}

	insns := buildFilter()
	prog := bpfProg{len: uint16(len(insns)), filter: &insns[0]}
	if _, _, errno := syscall.RawSyscall(syscall.SYS_PRCTL,
		syscall.PR_SET_SECCOMP,
		seccompModeFilter,
		uintptr(unsafe.Pointer(&prog))); errno != 0 {
		return fmt.Errorf("prctl(PR_SET_SECCOMP): %w", errno)
	}

	if debug {
		fmt.Printf("[init] seccomp: installed native-arch BPF filter (%d instructions, %d blocked syscalls)\n",
			len(insns), len(blockedSyscalls))
	}
	return nil
}
