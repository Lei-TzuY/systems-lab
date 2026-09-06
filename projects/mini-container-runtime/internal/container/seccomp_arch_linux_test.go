//go:build linux && (amd64 || arm64)

package container

import (
	"os"
	"os/exec"
	"syscall"
	"testing"
)

func TestSeccompFilterStartsWithNativeArchitectureGate(t *testing.T) {
	prog := buildFilter()
	if len(prog) < 5 {
		t.Fatalf("filter too short: %d instructions", len(prog))
	}
	if got := prog[0]; got.code != bpfLdWAbs || got.k != seccompDataArch {
		t.Fatalf("first instruction = %#v, want load seccomp_data.arch", got)
	}
	if got := prog[1]; got.code != bpfJmpJeqK || got.k != auditArch || got.jt != 1 || got.jf != 0 {
		t.Fatalf("architecture check = %#v, want native AUDIT_ARCH gate", got)
	}
	if got := prog[2]; got.code != bpfRetK || got.k != seccompRetKill {
		t.Fatalf("architecture mismatch action = %#v, want KILL", got)
	}
	if got := prog[3]; got.code != bpfLdWAbs || got.k != seccompDataNR {
		t.Fatalf("syscall dispatch = %#v, want load seccomp_data.nr", got)
	}
}

func TestSeccompLiveProcessAllowsNativeABIAndKillsBlockedSyscall(t *testing.T) {
	if os.Getenv("MINICONTAINER_SECCOMP_ARCH_HELPER") == "1" {
		if err := applySeccomp(false); err != nil {
			os.Exit(90)
		}
		// A native-ABI harmless syscall must pass the architecture gate.
		_, _, errno := syscall.RawSyscall(syscall.SYS_GETPID, 0, 0, 0)
		if errno != 0 {
			os.Exit(91)
		}
		// unshare is in every supported architecture's blocked table.
		syscall.RawSyscall(syscall.SYS_UNSHARE, uintptr(syscall.CLONE_NEWNS), 0, 0)
		os.Exit(92)
	}

	cmd := exec.Command(os.Args[0], "-test.run=^TestSeccompLiveProcessAllowsNativeABIAndKillsBlockedSyscall$")
	cmd.Env = append(os.Environ(), "MINICONTAINER_SECCOMP_ARCH_HELPER=1")
	err := cmd.Run()
	exitErr, ok := err.(*exec.ExitError)
	if !ok {
		t.Fatalf("helper error = %v, want seccomp signal termination", err)
	}
	status, ok := exitErr.Sys().(syscall.WaitStatus)
	if !ok || !status.Signaled() || status.Signal() != syscall.SIGSYS {
		t.Fatalf("helper status = %v, want SIGSYS from blocked syscall", exitErr.Sys())
	}
}
