package dns

import (
	"crypto/rand"
	"encoding/hex"
	"fmt"
	"sync"
)

type attemptOwnershipKey struct {
	networkName string
	containerID string
}

var (
	attemptOwnershipMu sync.Mutex
	attemptOwners      = make(map[attemptOwnershipKey]string)
)

func newAttemptToken() (string, error) {
	var raw [16]byte
	if _, err := rand.Read(raw[:]); err != nil {
		return "", fmt.Errorf("generate DNS attempt ownership token: %w", err)
	}
	return hex.EncodeToString(raw[:]), nil
}

// BeginHostRegistrationAttempt reserves service discovery for one runtime
// attempt and returns an exact-attempt rollback. The persistent entry remains
// admission-pending (and therefore hidden from peer snapshots) until bridge
// setup binds the exact child generation.
func BeginHostRegistrationAttempt(networkName, containerID, hostname, ipAddr string) (func() error, error) {
	token, err := newAttemptToken()
	if err != nil {
		return nil, err
	}
	key := attemptOwnershipKey{networkName: networkName, containerID: containerID}
	return beginHostRegistrationAttemptWith(
		key,
		token,
		func() error { return registerHostAdmission(networkName, containerID, hostname, ipAddr) },
		func() error { return UnregisterHostOwned(networkName, containerID) },
	)
}

func beginHostRegistrationAttemptWith(key attemptOwnershipKey, token string, register, unregister func() error) (func() error, error) {
	if token == "" {
		return nil, fmt.Errorf("DNS attempt ownership token cannot be empty")
	}
	if register == nil || unregister == nil {
		return nil, fmt.Errorf("DNS attempt ownership callbacks are incomplete")
	}

	attemptOwnershipMu.Lock()
	defer attemptOwnershipMu.Unlock()
	if err := register(); err != nil {
		return nil, err
	}
	attemptOwners[key] = token

	rollback := func() error {
		attemptOwnershipMu.Lock()
		defer attemptOwnershipMu.Unlock()
		if attemptOwners[key] != token {
			return nil
		}
		if err := unregister(); err != nil {
			return err
		}
		delete(attemptOwners, key)
		return nil
	}
	return rollback, nil
}
