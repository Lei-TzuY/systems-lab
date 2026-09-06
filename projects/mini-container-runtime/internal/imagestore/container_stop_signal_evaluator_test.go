package imagestore

import (
	"strings"
	"testing"
)

func TestEvaluateStopSignal(t *testing.T) {
	tests := []struct {
		name          string
		json          string
		wantCanonical string
		wantNum       int
		wantGraceful  bool
		wantTimeout   int
		wantErr       bool
	}{
		{
			name:          "default unset signal",
			json:          `{"config":{}}`,
			wantCanonical: "SIGTERM",
			wantNum:       15,
			wantGraceful:  true,
			wantTimeout:   10,
		},
		{
			name:          "custom SIGQUIT",
			json:          `{"config":{"StopSignal":"SIGQUIT"}}`,
			wantCanonical: "SIGQUIT",
			wantNum:       3,
			wantGraceful:  true,
			wantTimeout:   10,
		},
		{
			name:          "common signal SIGABRT",
			json:          `{"config":{"StopSignal":"SIGABRT"}}`,
			wantCanonical: "SIGABRT",
			wantNum:       6,
			wantGraceful:  true,
			wantTimeout:   10,
		},
		{
			name:          "numeric 9 (SIGKILL)",
			json:          `{"config":{"StopSignal":"9"}}`,
			wantCanonical: "SIGKILL",
			wantNum:       9,
			wantGraceful:  false,
			wantTimeout:   0,
		},
		{
			name:          "SIGSTOP cannot be handled gracefully",
			json:          `{"config":{"StopSignal":"SIGSTOP"}}`,
			wantCanonical: "SIGSTOP",
			wantNum:       19,
			wantGraceful:  false,
			wantTimeout:   0,
		},
		{
			name:          "without SIG prefix 'INT'",
			json:          `{"config":{"StopSignal":"INT"}}`,
			wantCanonical: "SIGINT",
			wantNum:       2,
			wantGraceful:  true,
			wantTimeout:   10,
		},
		{
			name:          "numeric realtime signal",
			json:          `{"config":{"StopSignal":"34"}}`,
			wantCanonical: "SIG_34",
			wantNum:       34,
			wantGraceful:  true,
			wantTimeout:   10,
		},
		{
			name:          "OCI realtime signal",
			json:          `{"config":{"StopSignal":"SIGRTMIN+3"}}`,
			wantCanonical: "SIGRTMIN+3",
			wantNum:       37,
			wantGraceful:  true,
			wantTimeout:   10,
		},
		{
			name:          "realtime signal without SIG prefix",
			json:          `{"config":{"StopSignal":"RTMIN+3"}}`,
			wantCanonical: "SIGRTMIN+3",
			wantNum:       37,
			wantGraceful:  true,
			wantTimeout:   10,
		},
		{
			name:          "realtime signal from max",
			json:          `{"config":{"StopSignal":"SIGRTMAX-3"}}`,
			wantCanonical: "SIGRTMAX-3",
			wantNum:       61,
			wantGraceful:  true,
			wantTimeout:   10,
		},
		{
			name:    "zero is invalid",
			json:    `{"config":{"StopSignal":"0"}}`,
			wantErr: true,
		},
		{
			name:    "negative is invalid",
			json:    `{"config":{"StopSignal":"-1"}}`,
			wantErr: true,
		},
		{
			name:    "out of range is invalid",
			json:    `{"config":{"StopSignal":"65"}}`,
			wantErr: true,
		},
		{
			name:    "realtime offset out of range",
			json:    `{"config":{"StopSignal":"SIGRTMIN+31"}}`,
			wantErr: true,
		},
		{
			name:    "malformed realtime signal",
			json:    `{"config":{"StopSignal":"SIGRTMIN+X"}}`,
			wantErr: true,
		},
		{
			name:    "unknown named signal is invalid",
			json:    `{"config":{"StopSignal":"SIGBANANA"}}`,
			wantErr: true,
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			res, err := EvaluateStopSignal([]byte(tc.json))
			if tc.wantErr {
				if err == nil {
					t.Fatalf("expected error, got result %+v", res)
				}
				return
			}
			if err != nil {
				t.Fatalf("unexpected error: %v", err)
			}
			if res.CanonicalSignal != tc.wantCanonical {
				t.Errorf("CanonicalSignal = %q, want %q", res.CanonicalSignal, tc.wantCanonical)
			}
			if res.SignalNumber != tc.wantNum {
				t.Errorf("SignalNumber = %d, want %d", res.SignalNumber, tc.wantNum)
			}
			if res.IsGraceful != tc.wantGraceful {
				t.Errorf("IsGraceful = %t, want %t", res.IsGraceful, tc.wantGraceful)
			}
			if res.DefaultTimeoutSec != tc.wantTimeout {
				t.Errorf("DefaultTimeoutSec = %d, want %d", res.DefaultTimeoutSec, tc.wantTimeout)
			}
		})
	}
}

func TestEvaluateStopSignal_AliasCanonicalization(t *testing.T) {
	res, err := EvaluateStopSignal([]byte(`{"config":{"StopSignal":"SIGIOT"}}`))
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if res.CanonicalSignal != "SIGABRT" || res.SignalNumber != 6 {
		t.Fatalf("alias resolved to %+v, want SIGABRT/6", res)
	}
}

func TestFormatStopSignal(t *testing.T) {
	got := FormatStopSignal([]byte(`{"config":{"StopSignal":"SIGTERM"}}`))
	if !strings.Contains(got, "Stop Signal: SIGTERM (num: 15, graceful: true") {
		t.Errorf("expected SIGTERM summary in %q", got)
	}
}

func TestFormatStopSignal_InvalidSignal(t *testing.T) {
	got := FormatStopSignal([]byte(`{"config":{"StopSignal":"SIGBANANA"}}`))
	if !strings.Contains(got, "error: unknown or unsupported stop signal") {
		t.Errorf("expected validation error in %q", got)
	}
}
