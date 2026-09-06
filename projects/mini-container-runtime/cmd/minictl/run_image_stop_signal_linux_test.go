//go:build linux

package main

import (
	"fmt"
	"os"
	"os/exec"
	"os/signal"
	"path/filepath"
	"syscall"
	"testing"
	"time"

	"minicontainer/internal/container"
	"minicontainer/internal/state"
)

func TestImageStopSignalHelperProcess(t *testing.T) {
	if os.Getenv("MINICONTAINER_IMAGE_STOP_SIGNAL_HELPER") != "1" {
		return
	}
	signal.Ignore(syscall.SIGTERM)
	ch := make(chan os.Signal, 1)
	signal.Notify(ch, syscall.SIGUSR1)
	defer signal.Stop(ch)
	fmt.Println("ready")
	select {
	case <-ch:
		marker := os.Getenv("MINICONTAINER_IMAGE_STOP_SIGNAL_MARKER")
		if err := os.WriteFile(marker, []byte("SIGUSR1"), 0o600); err != nil {
			os.Exit(3)
		}
		os.Exit(0)
	case <-time.After(10 * time.Second):
		os.Exit(4)
	}
}

func TestImageStopSignalFlowsThroughRunAdmissionToStop(t *testing.T) {
	stateDir := t.TempDir()
	rootfs := t.TempDir()
	st, err := state.Open(stateDir)
	if err != nil {
		t.Fatalf("open state: %v", err)
	}
	defer st.Close()
	if err := st.SaveImage(&state.Image{Name: "example:v1", RootFS: rootfs, LoadedAt: time.Now()}); err != nil {
		t.Fatalf("save image: %v", err)
	}
	if err := st.SaveImageStopSignal("example:v1", "SIGUSR1"); err != nil {
		t.Fatalf("save image stop signal: %v", err)
	}

	cfg := &container.Config{RootFS: rootfs, Command: []string{"helper"}, Hostname: "test"}
	admittedStore, rec, err := prepareManagedRunStateWith(cfg, runAdmissionDeps{
		openStore: func() (*state.Store, error) { return state.Open(stateDir) },
		newID:     func() (string, error) { return "ctr-image-stop-signal", nil },
		now:       time.Now,
	})
	if err != nil {
		t.Fatalf("prepareManagedRunStateWith: %v", err)
	}
	defer admittedStore.Close()
	gotSignal, err := admittedStore.ContainerStopSignal(rec.ID)
	if err != nil || gotSignal != "SIGUSR1" {
		t.Fatalf("admitted stop signal=%q err=%v, want SIGUSR1", gotSignal, err)
	}

	marker := filepath.Join(t.TempDir(), "signal-marker")
	cmd := exec.Command(os.Args[0], "-test.run=^TestImageStopSignalHelperProcess$")
	cmd.Env = append(os.Environ(),
		"MINICONTAINER_IMAGE_STOP_SIGNAL_HELPER=1",
		"MINICONTAINER_IMAGE_STOP_SIGNAL_MARKER="+marker,
	)
	stdout, err := cmd.StdoutPipe()
	if err != nil {
		t.Fatalf("helper stdout pipe: %v", err)
	}
	if err := cmd.Start(); err != nil {
		t.Fatalf("start helper: %v", err)
	}
	defer func() {
		if cmd.Process != nil && container.IsRunning(cmd.Process.Pid) {
			_ = cmd.Process.Kill()
		}
		_ = cmd.Wait()
	}()
	var ready string
	if _, err := fmt.Fscan(stdout, &ready); err != nil || ready != "ready" {
		t.Fatalf("helper readiness=%q err=%v", ready, err)
	}
	start, err := container.ProcessStartTime(cmd.Process.Pid)
	if err != nil {
		t.Fatalf("ProcessStartTime: %v", err)
	}
	rec.PID = cmd.Process.Pid
	rec.PIDStartTime = start
	rec.Status = state.StatusRunning
	if err := admittedStore.Save(rec); err != nil {
		t.Fatalf("save running state: %v", err)
	}
	if _, err := container.StopContainer(admittedStore, rec.ID, time.Second); err != nil {
		t.Fatalf("StopContainer: %v", err)
	}
	got, err := os.ReadFile(marker)
	if err != nil || string(got) != "SIGUSR1" {
		t.Fatalf("signal marker=%q err=%v, want SIGUSR1", got, err)
	}
}
