package health

import (
	"net"
	"net/http"
	"time"
)

// ProbeTCP checks if a TCP port is open and listening.
func ProbeTCP(address string, timeout time.Duration) bool {
	conn, err := net.DialTimeout("tcp", address, timeout)
	if err != nil {
		return false
	}
	_ = conn.Close()
	return true
}

// ProbeHTTP checks if an HTTP URL responds with a 2xx or 3xx status code.
func ProbeHTTP(urlStr string, timeout time.Duration) bool {
	client := &http.Client{Timeout: timeout}
	resp, err := client.Get(urlStr)
	if err != nil {
		return false
	}
	defer resp.Body.Close()
	return resp.StatusCode >= 200 && resp.StatusCode < 400
}
