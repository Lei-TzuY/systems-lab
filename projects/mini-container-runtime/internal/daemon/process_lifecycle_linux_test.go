//go:build linux

package daemon

import (
	"encoding/json"
	"io"
	"net/http"
	"net/http/httptest"
	"os/exec"
	"testing"
	"time"

	"minicontainer/internal/cgroups"
	"minicontainer/internal/container"
	"minicontainer/internal/state"
)

func saveRunningProcess(t *testing.T, st *state.Store, id string, cmd *exec.Cmd, start uint64) {
	t.Helper()
	if err := st.Save(&state.Container{
		ID:           id,
		PID:          cmd.Process.Pid,
		PIDStartTime: start,
		Status:       state.StatusRunning,
		CreatedAt:    time.Now(),
	}); err != nil {
		t.Fatalf("save running state: %v", err)
	}
}

func saveStoppedOwnedGeneration(t *testing.T, st *state.Store, id string, pid int, start uint64) {
	t.Helper()
	if err := st.Save(&state.Container{
		ID:           id,
		PID:          pid,
		PIDStartTime: start,
		Status:       state.StatusRunning,
		CreatedAt:    time.Now(),
	}); err != nil {
		t.Fatal(err)
	}
	name, err := cgroups.NameForContainerProcess(id, pid, start)
	if err != nil {
		t.Fatal(err)
	}
	if err := st.MarkCgroupOwnedIfIdentity(id, pid, start, name); err != nil {
		t.Fatal(err)
	}
	if _, err := st.MarkStoppedIfIdentity(id, pid, start, -1, time.Now()); err != nil {
		t.Fatal(err)
	}
}

func TestStopContainerUsesVerifiedPidfdAndEscalates(t *testing.T) {
	cmd := exec.Command("sh", "-c", "trap '' TERM; printf R; while :; do :; done")
	readyPipe, err := cmd.StdoutPipe()
	if err != nil {
		t.Fatalf("create readiness pipe: %v", err)
	}
	if err := cmd.Start(); err != nil {
		t.Fatalf("start child: %v", err)
	}
	defer func() {
		if container.IsRunning(cmd.Process.Pid) {
			_ = cmd.Process.Kill()
		}
		_ = cmd.Wait()
	}()

	// cmd.Start only proves that the shell process exists. Wait until the child
	// explicitly reports that its SIGTERM-ignore trap has been installed before
	// exercising timeout escalation; otherwise SIGTERM can win the setup race.
	ready := make([]byte, 1)
	if _, err := io.ReadFull(readyPipe, ready); err != nil {
		t.Fatalf("wait for child signal readiness: %v", err)
	}
	if ready[0] != 'R' {
		t.Fatalf("unexpected child readiness byte %#x", ready[0])
	}

	start, err := container.ProcessStartTime(cmd.Process.Pid)
	if err != nil {
		t.Fatalf("process starttime: %v", err)
	}
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	saveRunningProcess(t, st, "ctr-escalate", cmd, start)
	srv := &Server{store: st}

	req := httptest.NewRequest(http.MethodPost, "/v1/containers/ctr-escalate/stop?timeout=25ms", nil)
	rec := httptest.NewRecorder()
	srv.handleStopContainer(rec, req, "ctr-escalate")

	if rec.Code != http.StatusOK {
		t.Fatalf("status=%d body=%s", rec.Code, rec.Body.String())
	}
	var body map[string]interface{}
	if err := json.Unmarshal(rec.Body.Bytes(), &body); err != nil {
		t.Fatal(err)
	}
	if body["escalated"] != true {
		t.Fatalf("expected SIGKILL escalation, body=%v", body)
	}
	if container.IsRunning(cmd.Process.Pid) {
		t.Fatal("process still running after stop")
	}
	current, err := st.Get("ctr-escalate")
	if err != nil {
		t.Fatal(err)
	}
	if current.Status != state.StatusStopped {
		t.Fatalf("state=%s, want stopped", current.Status)
	}
}

func TestStopContainerReconcilesReusedPIDIdentity(t *testing.T) {
	cmd := exec.Command("sleep", "30")
	if err := cmd.Start(); err != nil {
		t.Fatal(err)
	}
	defer func() {
		_ = cmd.Process.Kill()
		_ = cmd.Wait()
	}()
	start, err := container.ProcessStartTime(cmd.Process.Pid)
	if err != nil {
		t.Fatal(err)
	}

	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	saveRunningProcess(t, st, "ctr-reused", cmd, start+1)
	srv := &Server{store: st}

	req := httptest.NewRequest(http.MethodPost, "/v1/containers/ctr-reused/stop?timeout=0s", nil)
	rec := httptest.NewRecorder()
	srv.handleStopContainer(rec, req, "ctr-reused")
	if rec.Code != http.StatusOK {
		t.Fatalf("status=%d want 200; body=%s", rec.Code, rec.Body.String())
	}
	ok, err := container.ProcessIdentityMatches(cmd.Process.Pid, start)
	if err != nil || !ok {
		t.Fatalf("unrelated process was affected: match=%v err=%v", ok, err)
	}
	current, err := st.Get("ctr-reused")
	if err != nil {
		t.Fatal(err)
	}
	if current.Status != state.StatusStopped {
		t.Fatalf("state=%s, want stopped after stale-generation reconciliation", current.Status)
	}
}

func TestStopAlreadyStoppedRetriesPendingCgroupCleanup(t *testing.T) {
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	const id = "ctr-stopped-owned-stop"
	saveStoppedOwnedGeneration(t, st, id, 1<<29, 12345)
	srv := &Server{store: st}

	req := httptest.NewRequest(http.MethodPost, "/v1/containers/"+id+"/stop", nil)
	rec := httptest.NewRecorder()
	srv.handleStopContainer(rec, req, id)
	if rec.Code != http.StatusOK {
		t.Fatalf("status=%d want 200; body=%s", rec.Code, rec.Body.String())
	}
	if _, ok, err := st.GetCgroupOwnership(id); err != nil || ok {
		t.Fatalf("already-stopped handler left cgroup ownership: ok=%v err=%v", ok, err)
	}
	current, err := st.Get(id)
	if err != nil {
		t.Fatal(err)
	}
	if current.Status != state.StatusStopped {
		t.Fatalf("state=%s, want stopped", current.Status)
	}
}

func TestDeleteRefusesLiveRunningContainer(t *testing.T) {
	cmd := exec.Command("sleep", "30")
	if err := cmd.Start(); err != nil {
		t.Fatal(err)
	}
	defer func() {
		_ = cmd.Process.Kill()
		_ = cmd.Wait()
	}()
	start, err := container.ProcessStartTime(cmd.Process.Pid)
	if err != nil {
		t.Fatal(err)
	}

	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	saveRunningProcess(t, st, "ctr-live", cmd, start)
	srv := &Server{store: st}

	rec := httptest.NewRecorder()
	srv.handleDeleteContainer(rec, "ctr-live")
	if rec.Code != http.StatusConflict {
		t.Fatalf("status=%d want 409; body=%s", rec.Code, rec.Body.String())
	}
	if _, err := st.Get("ctr-live"); err != nil {
		t.Fatalf("running state was deleted: %v", err)
	}
}

func TestDeleteReconcilesReusedPIDIdentity(t *testing.T) {
	cmd := exec.Command("sleep", "30")
	if err := cmd.Start(); err != nil {
		t.Fatal(err)
	}
	defer func() {
		_ = cmd.Process.Kill()
		_ = cmd.Wait()
	}()
	start, err := container.ProcessStartTime(cmd.Process.Pid)
	if err != nil {
		t.Fatal(err)
	}

	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	saveRunningProcess(t, st, "ctr-delete-reused", cmd, start+1)
	srv := &Server{store: st}

	rec := httptest.NewRecorder()
	srv.handleDeleteContainer(rec, "ctr-delete-reused")
	if rec.Code != http.StatusOK {
		t.Fatalf("status=%d want 200; body=%s", rec.Code, rec.Body.String())
	}
	ok, err := container.ProcessIdentityMatches(cmd.Process.Pid, start)
	if err != nil || !ok {
		t.Fatalf("unrelated process was affected: match=%v err=%v", ok, err)
	}
	if _, err := st.Get("ctr-delete-reused"); err == nil {
		t.Fatal("reconciled stale container state was not deleted")
	}
}

func TestDeleteRetriesPendingStoppedCgroupBeforeStateRemoval(t *testing.T) {
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	const id = "ctr-stopped-owned-delete"
	saveStoppedOwnedGeneration(t, st, id, 1<<29, 54321)
	srv := &Server{store: st}

	rec := httptest.NewRecorder()
	srv.handleDeleteContainer(rec, id)
	if rec.Code != http.StatusOK {
		t.Fatalf("status=%d want 200; body=%s", rec.Code, rec.Body.String())
	}
	if _, err := st.Get(id); err == nil {
		t.Fatal("stopped container state remained after successful cleanup/delete")
	}
	if _, ok, err := st.GetCgroupOwnership(id); err != nil || ok {
		t.Fatalf("delete retained ownership token: ok=%v err=%v", ok, err)
	}
}

func TestDeleteAllowsStaleRunningStateWhenPIDIsAbsent(t *testing.T) {
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	if err := st.Save(&state.Container{
		ID:           "ctr-dead",
		PID:          1 << 30,
		PIDStartTime: 123,
		Status:       state.StatusRunning,
		CreatedAt:    time.Now(),
	}); err != nil {
		t.Fatal(err)
	}
	srv := &Server{store: st}

	rec := httptest.NewRecorder()
	srv.handleDeleteContainer(rec, "ctr-dead")
	if rec.Code != http.StatusOK {
		t.Fatalf("status=%d want 200; body=%s", rec.Code, rec.Body.String())
	}
	if _, err := st.Get("ctr-dead"); err == nil {
		t.Fatal("stale dead container state was not deleted")
	}
}

func TestLifecycleHandlersRejectMissingIdentity(t *testing.T) {
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	if err := st.Save(&state.Container{ID: "ctr-legacy", PID: 1234, Status: state.StatusRunning, CreatedAt: time.Now()}); err != nil {
		t.Fatal(err)
	}
	srv := &Server{store: st}

	stopRec := httptest.NewRecorder()
	srv.handleStopContainer(stopRec, httptest.NewRequest(http.MethodPost, "/v1/containers/ctr-legacy/stop", nil), "ctr-legacy")
	if stopRec.Code != http.StatusConflict {
		t.Fatalf("stop status=%d want 409", stopRec.Code)
	}

	deleteRec := httptest.NewRecorder()
	srv.handleDeleteContainer(deleteRec, "ctr-legacy")
	if deleteRec.Code != http.StatusConflict {
		t.Fatalf("delete status=%d want 409", deleteRec.Code)
	}
}

func TestParseContainerStopTimeout(t *testing.T) {
	for _, tc := range []struct {
		raw     string
		wantErr bool
	}{
		{"", false},
		{"0s", false},
		{"25ms", false},
		{"7s", false},
		{"-1s", true},
		{"8s", true},
		{"10s", true},
		{"nope", true},
	} {
		req := httptest.NewRequest(http.MethodPost, "/stop?timeout="+tc.raw, nil)
		_, err := parseContainerStopTimeout(req)
		if (err != nil) != tc.wantErr {
			t.Fatalf("timeout %q err=%v wantErr=%v", tc.raw, err, tc.wantErr)
		}
	}
}
