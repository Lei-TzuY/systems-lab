package container

import "errors"

var (
	ErrProcessNotFound            = errors.New("process not found")
	ErrProcessIdentityMismatch    = errors.New("process identity mismatch")
	ErrProcessIdentityUnavailable = errors.New("process identity unavailable")
	ErrProcessControlUnsupported  = errors.New("safe process control unsupported")
)
