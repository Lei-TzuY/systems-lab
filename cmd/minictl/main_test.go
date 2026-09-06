package main

import (
	"os"
	"strings"
	"testing"
)

func TestParseByteSize(t *testing.T) {
	tests := []struct {
		name string
		in   string
		want int64
	}{
		{name: "empty", in: "", want: 0},
		{name: "bytes", in: "4096", want: 4096},
		{name: "explicit bytes", in: "42b", want: 42},
		{name: "kilobytes", in: "2k", want: 2 * 1024},
		{name: "megabytes", in: "64m", want: 64 * 1024 * 1024},
		{name: "gigabytes", in: "1g", want: 1024 * 1024 * 1024},
		{name: "mixed case", in: "8MiB", want: 8 * 1024 * 1024},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got, err := parseByteSize(tt.in)
			if err != nil {
				t.Fatalf("parseByteSize(%q) returned error: %v", tt.in, err)
			}
			if got != tt.want {
				t.Fatalf("parseByteSize(%q) = %d, want %d", tt.in, got, tt.want)
			}
		})
	}
}

func TestParseByteSizeRejectsInvalidInput(t *testing.T) {
	for _, in := range []string{"m", "-1", "abc", "1.5g"} {
		t.Run(in, func(t *testing.T) {
			if _, err := parseByteSize(in); err == nil {
				t.Fatalf("parseByteSize(%q) succeeded, want error", in)
			}
		})
	}
}

func TestParsePortSpec(t *testing.T) {
	tests := []struct {
		spec      string
		wantHost  int
		wantCont  int
		wantProto string
	}{
		{"8080:80", 8080, 80, "tcp"},
		{"53:53/udp", 53, 53, "udp"},
		{"3000:3000/TCP", 3000, 3000, "tcp"},
	}

	for _, tt := range tests {
		t.Run(tt.spec, func(t *testing.T) {
			pm, err := parsePortSpec(tt.spec)
			if err != nil {
				t.Fatalf("parsePortSpec(%q) error: %v", tt.spec, err)
			}
			if pm.HostPort != tt.wantHost || pm.ContainerPort != tt.wantCont || pm.Protocol != tt.wantProto {
				t.Fatalf("parsePortSpec(%q) = %#v, want %d:%d/%s",
					tt.spec, pm, tt.wantHost, tt.wantCont, tt.wantProto)
			}
		})
	}

	for _, bad := range []string{"invalid", "80", "abc:80", "8080:abc", "0:80", "70000:80"} {
		t.Run("bad_"+bad, func(t *testing.T) {
			if _, err := parsePortSpec(bad); err == nil {
				t.Fatalf("parsePortSpec(%q) succeeded, want error", bad)
			}
		})
	}
}

func TestParseRunConfig(t *testing.T) {
	cfg, err := parseRunConfig([]string{
		"--overlay",
		"--read-only",
		"--restart", "on-failure",
		"--cap-drop", "CAP_SYS_ADMIN",
		"--cap-drop", "CAP_NET_RAW",
		"--cpus", "0.5",
		"--hostname", "demo",
		"-w", "/app",
		"-e", "MODE=test",
		"-e", "PORT=8080",
		"-p", "8080:80",
		"--memory", "64m",
		"--cpu-weight", "200",
		"--pids-limit", "32",
		"--no-user-ns",
		"./rootfs",
		"/bin/echo",
		"hello",
	})
	if err != nil {
		t.Fatalf("parseRunConfig returned error: %v", err)
	}

	if !cfg.Overlay {
		t.Fatalf("Overlay = false, want true")
	}
	if !cfg.ReadOnly {
		t.Fatalf("ReadOnly = false, want true")
	}
	if cfg.Restart != "on-failure" {
		t.Fatalf("Restart = %q, want on-failure", cfg.Restart)
	}
	if len(cfg.CapDrop) != 2 || cfg.CapDrop[0] != "CAP_SYS_ADMIN" || cfg.CapDrop[1] != "CAP_NET_RAW" {
		t.Fatalf("CapDrop = %#v", cfg.CapDrop)
	}
	if cfg.CPUs != 0.5 {
		t.Fatalf("CPUs = %f, want 0.5", cfg.CPUs)
	}
	if cfg.RootFS != "./rootfs" {
		t.Fatalf("RootFS = %q", cfg.RootFS)
	}
	if len(cfg.Command) != 2 || cfg.Command[0] != "/bin/echo" || cfg.Command[1] != "hello" {
		t.Fatalf("Command = %#v", cfg.Command)
	}
	if cfg.Hostname != "demo" {
		t.Fatalf("Hostname = %q", cfg.Hostname)
	}
	if cfg.WorkDir != "/app" {
		t.Fatalf("WorkDir = %q, want /app", cfg.WorkDir)
	}
	if len(cfg.Env) != 2 || cfg.Env[0] != "MODE=test" || cfg.Env[1] != "PORT=8080" {
		t.Fatalf("Env = %#v", cfg.Env)
	}
	if len(cfg.PortMappings) != 1 || cfg.PortMappings[0].HostPort != 8080 || cfg.PortMappings[0].ContainerPort != 80 {
		t.Fatalf("PortMappings = %#v", cfg.PortMappings)
	}
	if cfg.Memory != 64*1024*1024 {
		t.Fatalf("Memory = %d", cfg.Memory)
	}
	if cfg.CPUWeight != 200 {
		t.Fatalf("CPUWeight = %d", cfg.CPUWeight)
	}
	if cfg.PidsLimit != 32 {
		t.Fatalf("PidsLimit = %d", cfg.PidsLimit)
	}
	if cfg.UserNS {
		t.Fatalf("UserNS = true, want false")
	}
}

func TestCmdRunLeavesGenerationLifecycleToRuntime(t *testing.T) {
	source, err := os.ReadFile("main.go")
	if err != nil {
		t.Fatalf("read main.go: %v", err)
	}
	text := string(source)
	start := strings.Index(text, "func cmdRun(args []string)")
	end := strings.Index(text, "func cmdExec(args []string)")
	if start < 0 || end <= start {
		t.Fatalf("could not isolate cmdRun source")
	}
	cmdRunSource := text[start:end]

	for _, forbidden := range []string{
		"events.EventStart",
		"events.EventDie",
		"dns.RegisterHost",
		"dns.InjectHostsIntoRootFS",
		"dns.UnregisterHost",
	} {
		if strings.Contains(cmdRunSource, forbidden) {
			t.Fatalf("cmdRun regained duplicate runtime lifecycle authority via %q", forbidden)
		}
	}
	if !strings.Contains(cmdRunSource, "events.EventCreate") {
		t.Fatalf("cmdRun no longer publishes the CLI create event")
	}
}

func TestParseRunConfigAllowsRootFSOnlyForImageDefaults(t *testing.T) {
	cfg, err := parseRunConfig([]string{"./rootfs"})
	if err != nil {
		t.Fatalf("parseRunConfig rootfs-only returned error: %v", err)
	}
	if cfg.RootFS != "./rootfs" {
		t.Fatalf("RootFS = %q, want ./rootfs", cfg.RootFS)
	}
	if len(cfg.Command) != 0 {
		t.Fatalf("Command = %#v, want empty so image defaults can resolve it", cfg.Command)
	}
}

func TestParseRunConfigStillRequiresRootFS(t *testing.T) {
	_, err := parseRunConfig(nil)
	if err == nil {
		t.Fatal("parseRunConfig(nil) succeeded, want missing-rootfs error")
	}
	if !strings.Contains(err.Error(), "missing rootfs") {
		t.Fatalf("parseRunConfig(nil) error = %q, want missing rootfs", err)
	}
}
