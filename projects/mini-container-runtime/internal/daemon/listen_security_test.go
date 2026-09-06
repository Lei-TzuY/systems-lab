package daemon

import (
	"strings"
	"testing"
)

func TestValidateUnauthenticatedTCPAddressAllowsNumericLoopback(t *testing.T) {
	for _, address := range []string{
		"127.0.0.1:2375",
		"127.123.45.67:0",
		"[::1]:2375",
	} {
		t.Run(address, func(t *testing.T) {
			if err := validateUnauthenticatedTCPAddress(address); err != nil {
				t.Fatalf("loopback address rejected: %v", err)
			}
		})
	}
}

func TestValidateUnauthenticatedTCPAddressRejectsRemoteOrAmbiguousBind(t *testing.T) {
	tests := []struct {
		address string
		want    string
	}{
		{"0.0.0.0:2375", "not loopback"},
		{"[::]:2375", "not loopback"},
		{":2375", "empty host"},
		{"192.168.1.10:2375", "not loopback"},
		{"10.0.0.1:2375", "not loopback"},
		{"localhost:2375", "numeric loopback"},
		{"example.com:2375", "numeric loopback"},
		{"127.0.0.1", "invalid TCP listen address"},
	}
	for _, tt := range tests {
		t.Run(tt.address, func(t *testing.T) {
			err := validateUnauthenticatedTCPAddress(tt.address)
			if err == nil || !strings.Contains(err.Error(), tt.want) {
				t.Fatalf("address=%q error=%v, want substring %q", tt.address, err, tt.want)
			}
		})
	}
}

func TestListenRejectsWildcardTCPBeforeBind(t *testing.T) {
	listener, err := listen("tcp", "0.0.0.0:0")
	if listener != nil {
		_ = listener.Close()
		t.Fatal("wildcard TCP unexpectedly returned a listener")
	}
	if err == nil || !strings.Contains(err.Error(), "not loopback") {
		t.Fatalf("wildcard listen error=%v", err)
	}
}
