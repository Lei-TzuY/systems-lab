package network

import (
	"testing"
)

func TestOrderDNSSearchDomains(t *testing.T) {
	domains := []string{"svc.local", "default.svc.cluster.local"}
	ordered := OrderDNSSearchDomains(domains)
	if len(ordered) != 2 || ordered[0] != "default.svc.cluster.local" {
		t.Fatalf("OrderDNSSearchDomains = %v, want longer domain first", ordered)
	}
}
