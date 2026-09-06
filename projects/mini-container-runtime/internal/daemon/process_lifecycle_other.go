//go:build !linux

package daemon

import (
	"net/http"
)

func (s *Server) handleDeleteContainer(w http.ResponseWriter, id string) {
	writeJSON(w, http.StatusNotImplemented, map[string]string{"error": "safe container lifecycle control requires Linux pidfds"})
}

func (s *Server) handleStopContainer(w http.ResponseWriter, r *http.Request, id string) {
	writeJSON(w, http.StatusNotImplemented, map[string]string{"error": "safe container lifecycle control requires Linux pidfds"})
}
