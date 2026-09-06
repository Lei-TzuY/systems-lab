package imagestore

import (
	"testing"
	"time"
)

func TestExtractCreatedAt(t *testing.T) {
	tests := []struct {
		name        string
		json        string
		wantHasTime bool
		wantYear    int
		wantErr     bool
	}{
		{
			name:        "rfc3339 timestamp",
			json:        `{"created":"2026-08-20T12:00:00Z"}`,
			wantHasTime: true,
			wantYear:    2026,
			wantErr:     false,
		},
		{
			name:        "rfc3339nano timestamp",
			json:        `{"created":"2026-01-15T08:30:00.123456789Z"}`,
			wantHasTime: true,
			wantYear:    2026,
			wantErr:     false,
		},
		{
			name:        "missing created field",
			json:        `{"config":{}}`,
			wantHasTime: false,
			wantErr:     false,
		},
		{
			name:    "invalid timestamp format",
			json:    `{"created":"not-a-timestamp"}`,
			wantErr: true,
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			info, err := ExtractCreatedAt([]byte(tc.json))
			if tc.wantErr {
				if err == nil {
					t.Fatal("expected error, got nil")
				}
				return
			}
			if err != nil {
				t.Fatalf("unexpected error: %v", err)
			}
			if info.HasTime != tc.wantHasTime {
				t.Errorf("HasTime = %t, want %t", info.HasTime, tc.wantHasTime)
			}
			if tc.wantHasTime && info.Timestamp.Year() != tc.wantYear {
				t.Errorf("Year = %d, want %d", info.Timestamp.Year(), tc.wantYear)
			}
		})
	}
}

func TestFormatRelativeAge(t *testing.T) {
	now := time.Date(2026, 8, 20, 12, 0, 0, 0, time.UTC)

	t.Run("seconds ago", func(t *testing.T) {
		created := now.Add(-30 * time.Second)
		if got := FormatRelativeAge(created, now); got != "30 seconds ago" {
			t.Errorf("got %q", got)
		}
	})

	t.Run("hours ago", func(t *testing.T) {
		created := now.Add(-5 * time.Hour)
		if got := FormatRelativeAge(created, now); got != "5 hours ago" {
			t.Errorf("got %q", got)
		}
	})

	t.Run("days ago", func(t *testing.T) {
		created := now.Add(-3 * 24 * time.Hour)
		if got := FormatRelativeAge(created, now); got != "3 days ago" {
			t.Errorf("got %q", got)
		}
	})

	t.Run("months ago", func(t *testing.T) {
		created := now.Add(-90 * 24 * time.Hour)
		if got := FormatRelativeAge(created, now); got != "3 months ago" {
			t.Errorf("got %q", got)
		}
	})
}
