package logs

import (
	"strings"
	"testing"
)

func TestLogTemplateMiner_ExtractTemplate(t *testing.T) {
	miner := NewLogTemplateMiner()

	tests := []struct {
		name string
		line string
		want string
	}{
		{
			name: "parameterizes ip and port",
			line: "Connection accepted from 192.168.1.50 port 8080",
			want: "Connection accepted from <*> port <*>",
		},
		{
			name: "parameterizes uuid",
			line: "Task c0a80101-1234-5678-90ab-cdef12345678 completed in 42 ms",
			want: "Task <*> completed in <*> ms",
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			got := miner.ExtractTemplate(tc.line)
			if got != tc.want {
				t.Errorf("ExtractTemplate(%q) = %q, want %q", tc.line, got, tc.want)
			}
		})
	}
}

func TestLogTemplateMiner_MineClusters(t *testing.T) {
	miner := NewLogTemplateMiner()
	lines := []string{
		"User 100 logged in from 10.0.0.1",
		"User 200 logged in from 10.0.0.2",
		"User 300 logged in from 10.0.0.3",
		"Disk check completed in 50 ms",
	}

	clusters := miner.MineClusters(lines)
	if len(clusters) != 2 {
		t.Fatalf("expected 2 clusters, got %d", len(clusters))
	}

	// First cluster should be the user login cluster with count 3
	if clusters[0].Count != 3 {
		t.Errorf("clusters[0].Count = %d, want 3", clusters[0].Count)
	}

	summary := FormatTemplateClusters(clusters, 2)
	if !strings.Contains(summary, "Count: 3") {
		t.Errorf("expected 'Count: 3' in summary, got %q", summary)
	}
}
