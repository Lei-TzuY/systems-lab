package cgroups

import (
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"testing"
)

func TestParsePSI_PreservesSomeAndFull(t *testing.T) {
	fixture := []byte("some avg10=1.25 avg60=2.50 avg300=3.75 total=123456 extra=ignored\nfull avg10=0.10 avg60=0.20 avg300=0.30 total=42\n")
	psi, err := parsePSI(fixture)
	if err != nil {
		t.Fatalf("parsePSI error: %v", err)
	}
	if psi.Some.Avg10 != 1.25 || psi.Some.Avg60 != 2.50 || psi.Some.Avg300 != 3.75 || psi.Some.Total != 123456 {
		t.Fatalf("unexpected some values: %+v", psi.Some)
	}
	if psi.Full == nil {
		t.Fatal("expected full PSI values")
	}
	if psi.Full.Avg10 != 0.10 || psi.Full.Avg60 != 0.20 || psi.Full.Avg300 != 0.30 || psi.Full.Total != 42 {
		t.Fatalf("unexpected full values: %+v", *psi.Full)
	}
}

func TestParsePSI_AllowsMissingFull(t *testing.T) {
	fixture := []byte("some avg10=0.50 avg60=0.10 avg300=0.02 total=456789\n")
	psi, err := parsePSI(fixture)
	if err != nil {
		t.Fatalf("parsePSI error: %v", err)
	}
	if psi.Full != nil {
		t.Fatalf("expected nil full values, got %+v", *psi.Full)
	}
	if psi.Some.Total != 456789 {
		t.Fatalf("expected total 456789, got %d", psi.Some.Total)
	}
}

func TestParsePSI_RejectsMalformedKnownFields(t *testing.T) {
	tests := []struct {
		name    string
		fixture string
		wantErr string
	}{
		{
			name:    "invalid average",
			fixture: "some avg10=nope avg60=0.00 avg300=0.00 total=0\n",
			wantErr: "avg10",
		},
		{
			name:    "average out of range",
			fixture: "some avg10=101 avg60=0.00 avg300=0.00 total=0\n",
			wantErr: "out of range",
		},
		{
			name:    "invalid total",
			fixture: "some avg10=0.00 avg60=0.00 avg300=0.00 total=-1\n",
			wantErr: "total",
		},
		{
			name:    "missing field",
			fixture: "some avg10=0.00 avg60=0.00 total=0\n",
			wantErr: "missing avg300",
		},
		{
			name:    "duplicate field",
			fixture: "some avg10=0.00 avg10=1.00 avg60=0.00 avg300=0.00 total=0\n",
			wantErr: "duplicate",
		},
		{
			name:    "duplicate scope",
			fixture: "some avg10=0.00 avg60=0.00 avg300=0.00 total=0\nsome avg10=0.00 avg60=0.00 avg300=0.00 total=0\n",
			wantErr: "duplicate PSI some",
		},
		{
			name:    "missing some scope",
			fixture: "full avg10=0.00 avg60=0.00 avg300=0.00 total=0\n",
			wantErr: "missing some",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			_, err := parsePSI([]byte(tt.fixture))
			if err == nil {
				t.Fatal("expected error")
			}
			if !strings.Contains(err.Error(), tt.wantErr) {
				t.Fatalf("error %q does not contain %q", err, tt.wantErr)
			}
		})
	}
}

func TestReadPSI_RejectsUnsupportedResource(t *testing.T) {
	if _, err := ReadPSI(t.TempDir(), "../memory"); err == nil {
		t.Fatal("expected unsupported PSI resource error")
	}
}

func TestReadPSI_ReturnsSomeScope(t *testing.T) {
	if runtime.GOOS != "linux" {
		t.Skip("cgroup PSI file reading requires Linux")
	}

	tmpDir := t.TempDir()
	fixture := "some avg10=1.00 avg60=2.00 avg300=3.00 total=99\nfull avg10=4.00 avg60=5.00 avg300=6.00 total=100\n"
	if err := os.WriteFile(filepath.Join(tmpDir, "memory.pressure"), []byte(fixture), 0o644); err != nil {
		t.Fatalf("write memory.pressure fixture: %v", err)
	}

	psi, err := ReadPSI(tmpDir, "memory")
	if err != nil {
		t.Fatalf("ReadPSI error: %v", err)
	}
	if psi.Avg10 != 1.00 || psi.Avg60 != 2.00 || psi.Avg300 != 3.00 || psi.Total != 99 {
		t.Fatalf("ReadPSI should return some scope, got %+v", *psi)
	}

	all, err := ReadPSIStats(tmpDir, "memory")
	if err != nil {
		t.Fatalf("ReadPSIStats error: %v", err)
	}
	if all.Full == nil || all.Full.Total != 100 {
		t.Fatalf("ReadPSIStats should preserve full scope, got %+v", all.Full)
	}
}
