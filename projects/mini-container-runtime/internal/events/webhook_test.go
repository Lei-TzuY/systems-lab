package events

import (
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"testing"
	"time"
)

func TestDispatchWebhook(t *testing.T) {
	received := false
	var receivedPayload map[string]interface{}

	ts := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		received = true
		_ = json.NewDecoder(r.Body).Decode(&receivedPayload)
		w.WriteHeader(http.StatusOK)
	}))
	defer ts.Close()

	evt := Event{
		Type:        EventStart,
		ContainerID: "ctr-wh-123",
		Message:     "container started",
		Timestamp:   time.Now(),
	}

	err := DispatchWebhook(context.Background(), ts.URL, evt)
	if err != nil {
		t.Fatalf("DispatchWebhook error: %v", err)
	}

	if !received {
		t.Fatalf("Webhook server did not receive request")
	}

	if receivedPayload["container_id"] != "ctr-wh-123" {
		t.Fatalf("Webhook payload container_id = %v, want ctr-wh-123", receivedPayload["container_id"])
	}
}
