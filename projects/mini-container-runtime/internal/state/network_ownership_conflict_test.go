package state

import (
	"strings"
	"testing"
)

func TestValidateNetworkOwnershipRejectsCompetingHostIngressTargets(t *testing.T) {
	ownership := NetworkOwnership{
		Owner:        "minicontainer:ambiguous-ingress",
		PID:          101,
		PIDStartTime: 202,
		Mappings: []PortForwardingOwnership{
			{HostPort: 8080, ContainerPort: 80, ContainerIP: "172.20.0.2", Protocol: "tcp"},
			{HostPort: 8080, ContainerPort: 8080, ContainerIP: "172.20.0.3", Protocol: "tcp"},
		},
	}

	err := validateNetworkOwnership(ownership)
	if err == nil || !strings.Contains(err.Error(), "duplicate network host ingress 8080/tcp") {
		t.Fatalf("ambiguous host ingress error=%v", err)
	}
}

func TestValidateNetworkOwnershipAllowsSameHostPortAcrossProtocols(t *testing.T) {
	ownership := NetworkOwnership{
		Owner:        "minicontainer:protocol-split",
		PID:          303,
		PIDStartTime: 404,
		Mappings: []PortForwardingOwnership{
			{HostPort: 5353, ContainerPort: 53, ContainerIP: "172.20.0.2", Protocol: "tcp"},
			{HostPort: 5353, ContainerPort: 53, ContainerIP: "172.20.0.2", Protocol: "udp"},
		},
	}

	if err := validateNetworkOwnership(ownership); err != nil {
		t.Fatalf("protocol-distinct host ingress rejected: %v", err)
	}
}
