package events

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"net/http"
	"time"
)

// DispatchWebhook sends an event payload to a webhook URL.
func DispatchWebhook(ctx context.Context, webhookURL string, evt Event) error {
	if webhookURL == "" {
		return nil
	}

	payload, err := json.Marshal(map[string]interface{}{
		"event":        evt.Type,
		"container_id": evt.ContainerID,
		"image":        evt.Image,
		"message":      evt.Message,
		"timestamp":    evt.Timestamp.Format(time.RFC3339),
	})

	if err != nil {
		return fmt.Errorf("marshal webhook event: %w", err)
	}

	req, err := http.NewRequestWithContext(ctx, http.MethodPost, webhookURL, bytes.NewBuffer(payload))
	if err != nil {
		return fmt.Errorf("create webhook request: %w", err)
	}
	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("User-Agent", "minictl-webhook/1.2")

	client := &http.Client{Timeout: 5 * time.Second}
	resp, err := client.Do(req)
	if err != nil {
		return fmt.Errorf("dispatch webhook POST: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode < 200 || resp.StatusCode >= 300 {
		return fmt.Errorf("webhook responded with status: %d", resp.StatusCode)
	}
	return nil
}
