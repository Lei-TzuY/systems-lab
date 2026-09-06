package state

import "errors"

// ErrStoreClosed is returned when an operation requiring Store-owned resources
// is attempted after Close has released them.
var ErrStoreClosed = errors.New("state store is closed")

// ErrContainerRunning marks the benign lifecycle race where a destructive
// operation observed that a container has entered a running generation and
// therefore refused deletion. Callers such as garbage collection may treat
// this specific condition as a skip while still surfacing all other deletion
// failures.
var ErrContainerRunning = errors.New("container is running")
