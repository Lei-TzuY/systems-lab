package dns

import (
	"strings"
	"testing"
)

func TestGenerateResolvConf(t *testing.T) {
	conf := GenerateResolvConf([]string{"9.9.9.9"}, []string{"internal.domain"})
	if !strings.Contains(conf, "search internal.domain") {
		t.Fatalf("resolv.conf missing search domain: %s", conf)
	}
	if !strings.Contains(conf, "nameserver 9.9.9.9") {
		t.Fatalf("resolv.conf missing nameserver: %s", conf)
	}
}

func TestGenerateResolvConfInjectionDefense(t *testing.T) {
	// Attack payload with newline injection
	conf := GenerateResolvConf([]string{"8.8.8.8\noptions rotate\nnameserver 1.2.3.4", "1.1.1.1"}, []string{"valid.domain", "evil.domain\nnameserver 6.6.6.6"})

	if strings.Contains(conf, "options rotate") {
		t.Fatalf("resolv.conf failed to reject nameserver newline injection:\n%s", conf)
	}
	if strings.Contains(conf, "6.6.6.6") {
		t.Fatalf("resolv.conf failed to reject search domain newline injection:\n%s", conf)
	}
	if !strings.Contains(conf, "nameserver 1.1.1.1") {
		t.Fatalf("resolv.conf missing valid nameserver 1.1.1.1:\n%s", conf)
	}
	if !strings.Contains(conf, "search valid.domain") {
		t.Fatalf("resolv.conf missing valid search domain:\n%s", conf)
	}
}
