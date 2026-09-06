//go:build linux

package cgroups

import (
	"errors"
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"testing"
)

func TestApplyResourceUpdateTransactionRollsBackOnLaterWriteFailure(t *testing.T) {
	cgPath := "/fake/cgroup"
	values := map[string]string{
		"memory.max": "67108864",
		"cpu.max":    "max 100000",
		"cpu.weight": "100",
	}
	var writes []string
	cpuWriteCause := errors.New("cpu write failed")

	err := applyResourceUpdateTransaction(cgPath, UpdateConfig{
		MemoryMax: 134217728,
		CPUs:      1.5,
		CPUWeight: 200,
	}, false, updateFileOps{
		readFile: func(path string) ([]byte, error) {
			name := filepath.Base(path)
			return []byte(values[name] + "\n"), nil
		},
		writeFile: func(path string, data []byte, _ os.FileMode) error {
			name := filepath.Base(path)
			value := string(data)
			writes = append(writes, name+"="+value)
			if name == "cpu.max" && value == "150000 100000" {
				return cpuWriteCause
			}
			values[name] = value
			return nil
		},
	})
	if !errors.Is(err, cpuWriteCause) {
		t.Fatalf("error=%v, want cpu write failure", err)
	}
	if got, want := values["memory.max"], "67108864"; got != want {
		t.Fatalf("memory.max=%q after rollback, want %q", got, want)
	}
	if got, want := values["cpu.max"], "max 100000"; got != want {
		t.Fatalf("cpu.max=%q after rollback, want %q", got, want)
	}
	if got, want := values["cpu.weight"], "100"; got != want {
		t.Fatalf("cpu.weight=%q should remain untouched, want %q", got, want)
	}
	wantWrites := []string{
		"memory.max=134217728",
		"cpu.max=150000 100000",
		"cpu.max=max 100000",
		"memory.max=67108864",
	}
	if !reflect.DeepEqual(writes, wantWrites) {
		t.Fatalf("writes=%v, want %v", writes, wantWrites)
	}
}

func TestApplyResourceUpdateTransactionPreReadsBeforeAnyWrite(t *testing.T) {
	readCause := errors.New("cpu.max disappeared")
	writes := 0
	err := applyResourceUpdateTransaction("/fake/cgroup", UpdateConfig{
		MemoryMax: 134217728,
		CPUs:      2,
	}, false, updateFileOps{
		readFile: func(path string) ([]byte, error) {
			switch filepath.Base(path) {
			case "memory.max":
				return []byte("67108864\n"), nil
			case "cpu.max":
				return nil, readCause
			default:
				t.Fatalf("unexpected read %q", path)
				return nil, nil
			}
		},
		writeFile: func(string, []byte, os.FileMode) error {
			writes++
			return nil
		},
	})
	if !errors.Is(err, readCause) {
		t.Fatalf("error=%v, want read failure", err)
	}
	if writes != 0 {
		t.Fatalf("writes=%d after preflight read failure, want 0", writes)
	}
}

func TestApplyResourceUpdateTransactionJoinsRollbackFailure(t *testing.T) {
	primary := errors.New("cpu update denied")
	rollback := errors.New("memory rollback denied")
	values := map[string]string{
		"memory.max": "67108864",
		"cpu.max":    "max 100000",
	}

	err := applyResourceUpdateTransaction("/fake/cgroup", UpdateConfig{
		MemoryMax: 134217728,
		CPUs:      1,
	}, false, updateFileOps{
		readFile: func(path string) ([]byte, error) {
			return []byte(values[filepath.Base(path)]), nil
		},
		writeFile: func(path string, data []byte, _ os.FileMode) error {
			name := filepath.Base(path)
			value := string(data)
			if name == "cpu.max" && value == "100000 100000" {
				return primary
			}
			if name == "memory.max" && value == "67108864" {
				return rollback
			}
			values[name] = value
			return nil
		},
	})
	if !errors.Is(err, primary) {
		t.Fatalf("error=%v, missing primary update error", err)
	}
	if !errors.Is(err, rollback) {
		t.Fatalf("error=%v, missing rollback error", err)
	}
	if !strings.Contains(err.Error(), "rollback memory.max") {
		t.Fatalf("error=%v, missing rollback context", err)
	}
}

func TestApplyResourceUpdateTransactionSuccessAppliesAllValues(t *testing.T) {
	values := map[string]string{
		"memory.max": "67108864",
		"cpu.max":    "max 100000",
		"cpu.weight": "100",
		"pids.max":   "max",
	}
	err := applyResourceUpdateTransaction("/fake/cgroup", UpdateConfig{
		MemoryMax: 134217728,
		CPUs:      0.5,
		CPUWeight: 500,
		PidsMax:   256,
	}, false, updateFileOps{
		readFile: func(path string) ([]byte, error) {
			return []byte(values[filepath.Base(path)] + "\n"), nil
		},
		writeFile: func(path string, data []byte, mode os.FileMode) error {
			if mode != 0o644 {
				t.Fatalf("mode=%#o, want 0644", mode)
			}
			values[filepath.Base(path)] = string(data)
			return nil
		},
	})
	if err != nil {
		t.Fatalf("apply transaction: %v", err)
	}
	want := map[string]string{
		"memory.max": "134217728",
		"cpu.max":    "50000 100000",
		"cpu.weight": "500",
		"pids.max":   "256",
	}
	if !reflect.DeepEqual(values, want) {
		t.Fatalf("values=%v, want %v", values, want)
	}
}

func TestApplyResourceUpdateTransactionRejectsEmptyCurrentValue(t *testing.T) {
	writes := 0
	err := applyResourceUpdateTransaction("/fake/cgroup", UpdateConfig{MemoryMax: 1}, false, updateFileOps{
		readFile: func(string) ([]byte, error) { return []byte(" \n"), nil },
		writeFile: func(string, []byte, os.FileMode) error {
			writes++
			return nil
		},
	})
	if err == nil || !strings.Contains(err.Error(), "empty value") {
		t.Fatalf("error=%v, want empty-value failure", err)
	}
	if writes != 0 {
		t.Fatalf("writes=%d after empty preflight value, want 0", writes)
	}
}
