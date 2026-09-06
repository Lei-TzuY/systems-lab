//go:build linux

package daemon

import (
	"errors"
	"fmt"
	"net/http"
	"syscall"
	"time"

	"minicontainer/internal/container"
	"minicontainer/internal/state"
)

const (
	defaultContainerStopTimeout = 5 * time.Second
	maxContainerStopTimeout     = 7 * time.Second
	parentStateSettleTimeout    = 500 * time.Millisecond
	postKillWaitTimeout         = 2 * time.Second
)

func (s *Server) handleDeleteContainer(w http.ResponseWriter, id string) {
	c, err := s.store.Resolve(id)
	if err != nil {
		writeJSON(w, http.StatusNotFound, map[string]string{"error": err.Error()})
		return
	}

	c, err = container.ReconcileContainerState(s.store, c)
	if err != nil {
		if errors.Is(err, container.ErrProcessIdentityUnavailable) {
			writeJSON(w, http.StatusConflict, map[string]string{"error": "running container lacks a verified process identity; refusing deletion"})
			return
		}
		writeJSON(w, http.StatusInternalServerError, map[string]string{"error": err.Error()})
		return
	}
	if c.Status == state.StatusRunning {
		writeJSON(w, http.StatusConflict, map[string]string{"error": "container is still running; stop it before deletion"})
		return
	}

	// Recheck the current on-disk status while holding the state lock. This
	// closes the final reconciliation->delete window if another actor restarts
	// the container after the exact-generation probe above.
	if err := s.store.DeleteIfNotRunning(c.ID); err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]string{"error": err.Error()})
		return
	}
	writeJSON(w, http.StatusOK, map[string]string{"status": "deleted", "id": c.ID})
}

func (s *Server) handleStopContainer(w http.ResponseWriter, r *http.Request, id string) {
	c, err := s.store.Resolve(id)
	if err != nil {
		writeJSON(w, http.StatusNotFound, map[string]string{"error": err.Error()})
		return
	}
	c, err = container.ReconcileContainerState(s.store, c)
	if err != nil {
		if errors.Is(err, container.ErrProcessIdentityUnavailable) {
			writeJSON(w, http.StatusConflict, map[string]string{"error": "running container lacks PID starttime identity; refusing to signal by PID alone"})
			return
		}
		writeJSON(w, http.StatusInternalServerError, map[string]string{"error": err.Error()})
		return
	}
	if c.Status != state.StatusRunning {
		if c.Status == state.StatusStopped {
			if err := container.CleanupStoppedRuntimeResources(s.store, c); err != nil {
				writeJSON(w, http.StatusInternalServerError, map[string]string{"error": err.Error()})
				return
			}
		}
		writeJSON(w, http.StatusOK, map[string]interface{}{"status": "stopped", "id": c.ID, "already_stopped": true})
		return
	}

	timeout, err := parseContainerStopTimeout(r)
	if err != nil {
		writeJSON(w, http.StatusBadRequest, map[string]string{"error": err.Error()})
		return
	}

	handle, err := container.OpenProcessHandle(c.PID, c.PIDStartTime)
	if err != nil {
		switch {
		case errors.Is(err, container.ErrProcessIdentityUnavailable):
			writeJSON(w, http.StatusConflict, map[string]string{"error": "running container lacks PID starttime identity; refusing to signal by PID alone"})
			return
		case errors.Is(err, container.ErrProcessNotFound), errors.Is(err, container.ErrProcessIdentityMismatch):
			// The exact generation may have exited or its numeric PID may now
			// belong to an unrelated process after the initial reconciliation.
			// Reconcile by PID/start-time identity instead of ever signaling the
			// replacement process.
			latest, reconcileErr := container.ReconcileContainerState(s.store, c)
			if reconcileErr != nil {
				writeJSON(w, http.StatusInternalServerError, map[string]string{"error": reconcileErr.Error()})
				return
			}
			if latest.Status != state.StatusRunning {
				writeJSON(w, http.StatusOK, map[string]interface{}{"status": "stopped", "id": latest.ID, "already_exited": true})
				return
			}
			writeJSON(w, http.StatusConflict, map[string]string{"error": "container process generation changed while stop was opening its process handle; retry"})
			return
		default:
			writeJSON(w, http.StatusInternalServerError, map[string]string{"error": err.Error()})
			return
		}
	}
	defer handle.Close()

	if err := handle.Signal(syscall.SIGTERM); err != nil && !errors.Is(err, container.ErrProcessNotFound) {
		writeJSON(w, http.StatusInternalServerError, map[string]string{"error": fmt.Sprintf("send SIGTERM: %v", err)})
		return
	}

	exited, err := handle.WaitExit(timeout)
	if err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]string{"error": err.Error()})
		return
	}
	escalated := false
	if !exited {
		escalated = true
		if err := handle.Signal(syscall.SIGKILL); err != nil && !errors.Is(err, container.ErrProcessNotFound) {
			writeJSON(w, http.StatusInternalServerError, map[string]string{"error": fmt.Sprintf("send SIGKILL: %v", err)})
			return
		}
		exited, err = handle.WaitExit(postKillWaitTimeout)
		if err != nil {
			writeJSON(w, http.StatusInternalServerError, map[string]string{"error": err.Error()})
			return
		}
		if !exited {
			writeJSON(w, http.StatusInternalServerError, map[string]string{"error": "container process did not exit after SIGKILL"})
			return
		}
	}

	// The CLI parent owns wait(2) and therefore knows the real exit status. Give
	// it a short window to publish the authoritative stopped state; only fall
	// back to an unknown exit code if that parent is gone or unresponsive.
	if err := waitForParentStoppedState(s.store, c.ID, parentStateSettleTimeout); err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]string{"error": err.Error()})
		return
	}
	current, err := s.store.Get(c.ID)
	if err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]string{"error": err.Error()})
		return
	}

	exitCode := -1
	if current.Status == state.StatusStopped {
		exitCode = current.ExitCode
	}
	// Finalize the exact generation captured before signaling. If the parent
	// already stopped it or a restart has installed a new PID/start-time pair,
	// the CAS is a no-op. Durable cleanup additionally requires exact ownership
	// proof for that generation.
	if _, err := container.FinalizeStoppedGeneration(s.store, c, exitCode, time.Now()); err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]string{"error": err.Error()})
		return
	}

	writeJSON(w, http.StatusOK, map[string]interface{}{
		"status":    "stopped",
		"id":        c.ID,
		"escalated": escalated,
	})
}

func parseContainerStopTimeout(r *http.Request) (time.Duration, error) {
	raw := r.URL.Query().Get("timeout")
	if raw == "" {
		return defaultContainerStopTimeout, nil
	}
	d, err := time.ParseDuration(raw)
	if err != nil {
		return 0, fmt.Errorf("invalid stop timeout: %w", err)
	}
	if d < 0 || d > maxContainerStopTimeout {
		return 0, fmt.Errorf("stop timeout must be between 0 and %s", maxContainerStopTimeout)
	}
	return d, nil
}

func waitForParentStoppedState(st *state.Store, id string, timeout time.Duration) error {
	deadline := time.Now().Add(timeout)
	for time.Now().Before(deadline) {
		c, err := st.Get(id)
		if err != nil {
			return fmt.Errorf("read container state while settling stop: %w", err)
		}
		if c.Status != state.StatusRunning {
			return nil
		}
		time.Sleep(25 * time.Millisecond)
	}
	return nil
}
