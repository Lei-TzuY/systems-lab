package health

import (
	"context"
	"fmt"
	"sync"
	"time"

	"minicontainer/internal/state"
)

// CheckFunc is a function type that executes health command inside container and returns exit code.
type CheckFunc func(ctx context.Context) (int, error)

// Supervisor runs periodic background health checks for a container.
type Supervisor struct {
	containerID string
	config      Config
	checkFn     CheckFunc
	store       *state.Store
	mu          sync.Mutex
	failures    int
}

// NewSupervisor creates a supervisor instance.
func NewSupervisor(containerID string, cfg Config, checkFn CheckFunc, store *state.Store) *Supervisor {
	if cfg.Interval <= 0 {
		cfg.Interval = 5 * time.Second
	}
	if cfg.Timeout <= 0 {
		cfg.Timeout = 3 * time.Second
	}
	if cfg.Retries <= 0 {
		cfg.Retries = 3
	}
	return &Supervisor{
		containerID: containerID,
		config:      cfg,
		checkFn:     checkFn,
		store:       store,
	}
}

// Start runs background healthcheck loop until context is cancelled.
func (s *Supervisor) Start(ctx context.Context) {
	ticker := time.NewTicker(s.config.Interval)
	defer ticker.Stop()

	for {
		select {
		case <-ctx.Done():
			return
		case <-ticker.C:
			if ctx.Err() != nil {
				return
			}
			s.runOnce(ctx)
		}
	}
}

func (s *Supervisor) runOnce(parentCtx context.Context) {
	if parentCtx.Err() != nil {
		return
	}
	ctx, cancel := context.WithTimeout(parentCtx, s.config.Timeout)
	defer cancel()

	exitCode := 1
	if s.checkFn != nil {
		code, err := s.checkFn(ctx)
		if err == nil {
			exitCode = code
		}
	}

	if parentCtx.Err() != nil {
		return
	}

	s.mu.Lock()
	if exitCode == 0 {
		s.failures = 0
	} else {
		s.failures++
	}
	failures := s.failures
	s.mu.Unlock()

	status := Evaluate(failures, s.config.Retries)
	if s.store != nil && parentCtx.Err() == nil {
		if c, err := s.store.Get(s.containerID); err == nil {
			c.Health = status
			_ = s.store.Save(c)
		}
	}
	fmt.Printf("[HealthCheck %s] Check result: exitCode=%d, status=%s\n", s.containerID[:min(8, len(s.containerID))], exitCode, status)
}

func min(a, b int) int {
	if a < b {
		return a
	}
	return b
}
