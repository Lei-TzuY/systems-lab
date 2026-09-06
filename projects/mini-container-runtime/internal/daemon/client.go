package daemon

import (
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net"
	"net/http"
	"strings"
	"time"

	"minicontainer/internal/state"
)

// Client handles REST API communications with minictld daemon.
type Client struct {
	endpoint string
	hc       *http.Client
}

// NewClient initializes a client targeting a minictld daemon endpoint.
func NewClient(endpoint string) *Client {
	if endpoint == "" {
		endpoint = "unix:///tmp/minictl.sock"
	}

	transport := &http.Transport{}
	if strings.HasPrefix(endpoint, "unix://") {
		socketPath := strings.TrimPrefix(endpoint, "unix://")
		transport.DialContext = func(ctx context.Context, _, _ string) (net.Conn, error) {
			return net.Dial("unix", socketPath)
		}
	}

	return &Client{
		endpoint: endpoint,
		hc: &http.Client{
			Transport: transport,
			Timeout:   10 * time.Second,
		},
	}
}

func (c *Client) getURL(path string) string {
	if strings.HasPrefix(c.endpoint, "unix://") {
		return "http://unix" + path
	}
	return strings.TrimSuffix(c.endpoint, "/") + path
}

func (c *Client) SystemInfo() (map[string]interface{}, error) {
	resp, err := c.hc.Get(c.getURL("/v1/system/info"))
	if err != nil {
		return nil, err
	}
	defer resp.Body.Close()

	var res map[string]interface{}
	if err := json.NewDecoder(resp.Body).Decode(&res); err != nil {
		return nil, err
	}
	return res, nil
}

func (c *Client) ListContainers() ([]*state.Container, error) {
	resp, err := c.hc.Get(c.getURL("/v1/containers/json"))
	if err != nil {
		return nil, err
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		body, _ := io.ReadAll(resp.Body)
		return nil, fmt.Errorf("status %d: %s", resp.StatusCode, string(body))
	}

	var ctrs []*state.Container
	if err := json.NewDecoder(resp.Body).Decode(&ctrs); err != nil {
		return nil, err
	}
	return ctrs, nil
}

func (c *Client) ListImages() ([]*state.Image, error) {
	resp, err := c.hc.Get(c.getURL("/v1/images/json"))
	if err != nil {
		return nil, err
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		body, _ := io.ReadAll(resp.Body)
		return nil, fmt.Errorf("status %d: %s", resp.StatusCode, string(body))
	}

	var imgs []*state.Image
	if err := json.NewDecoder(resp.Body).Decode(&imgs); err != nil {
		return nil, err
	}
	return imgs, nil
}

func (c *Client) StopContainer(id string) error {
	req, err := http.NewRequest(http.MethodPost, c.getURL("/v1/containers/"+id+"/stop"), nil)
	if err != nil {
		return err
	}
	resp, err := c.hc.Do(req)
	if err != nil {
		return err
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		body, _ := io.ReadAll(resp.Body)
		return fmt.Errorf("status %d: %s", resp.StatusCode, string(body))
	}
	return nil
}
