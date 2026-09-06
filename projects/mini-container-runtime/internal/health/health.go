// internal/health/health.go
//
// Container Health Checking (`--health-cmd`, `--health-interval`)
// ─────────────────────────────────────────────────────────────
// Periodically executes a health check command inside the container.
// Updates container health status: "starting" -> "healthy" or "unhealthy".

package health

import (
	"fmt"
	"time"
)

const (
	StatusStarting  = "starting"
	StatusHealthy   = "healthy"
	StatusUnhealthy = "unhealthy"
)

// Config describes health check parameters.
type Config struct {
	Command  []string      // Command to run inside container (e.g. ["/bin/cat", "/tmp/ready"])
	Interval time.Duration // Time between checks (default: 5s)
	Timeout  time.Duration // Timeout for check (default: 3s)
	Retries  int           // Consecutive failures needed to report unhealthy (default: 3)
}

// Result holds the outcome of one health check execution.
type Result struct {
	Timestamp time.Time
	ExitCode  int
	Output    string
	Status    string
}

// Evaluate determines the health status given consecutive failures and retries limit.
func Evaluate(consecutiveFailures, maxRetries int) string {
	if consecutiveFailures == 0 {
		return StatusHealthy
	}
	if consecutiveFailures >= maxRetries {
		return StatusUnhealthy
	}
	return StatusStarting
}

func (c *Config) String() string {
	return fmt.Sprintf("cmd=%v, interval=%s, retries=%d", c.Command, c.Interval, c.Retries)
}
