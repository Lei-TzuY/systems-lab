package container

import (
	"fmt"
	"strconv"
	"strings"
)

type RestartPolicyType string

const (
	RestartNo        RestartPolicyType = "no"
	RestartAlways    RestartPolicyType = "always"
	RestartOnFailure RestartPolicyType = "on-failure"
)

type RestartPolicy struct {
	Type       RestartPolicyType
	MaxRetries int
}

// ParseRestartPolicy parses policy strings like "no", "always", "on-failure", "on-failure:5".
func ParseRestartPolicy(spec string) (RestartPolicy, error) {
	spec = strings.TrimSpace(strings.ToLower(spec))
	if spec == "" || spec == "no" {
		return RestartPolicy{Type: RestartNo}, nil
	}
	if spec == "always" {
		return RestartPolicy{Type: RestartAlways}, nil
	}
	if strings.HasPrefix(spec, "on-failure") {
		parts := strings.Split(spec, ":")
		maxRetries := 0
		if len(parts) == 2 {
			n, err := strconv.Atoi(parts[1])
			if err != nil || n < 0 {
				return RestartPolicy{}, fmt.Errorf("invalid max retries in restart policy %q", spec)
			}
			maxRetries = n
		}
		return RestartPolicy{Type: RestartOnFailure, MaxRetries: maxRetries}, nil
	}
	return RestartPolicy{}, fmt.Errorf("unknown restart policy %q", spec)
}

// ShouldRestart evaluates whether a container should be restarted based on policy and exit code.
func ShouldRestart(policy RestartPolicy, exitCode int, currentRetries int) bool {
	switch policy.Type {
	case RestartAlways:
		return true
	case RestartOnFailure:
		if exitCode != 0 {
			if policy.MaxRetries == 0 || currentRetries < policy.MaxRetries {
				return true
			}
		}
		return false
	default:
		return false
	}
}
