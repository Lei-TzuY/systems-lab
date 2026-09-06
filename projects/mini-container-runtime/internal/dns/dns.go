package dns

import (
	"fmt"
	"net"
	"os"
	"path/filepath"
	"regexp"
	"strings"
	"sync"

	"minicontainer/internal/state"
)

const maxDNSHostnameBytes = 253

var (
	dnsMu                 sync.Mutex
	validNetworkNameRegex = regexp.MustCompile(`^[a-zA-Z0-9][a-zA-Z0-9_.-]*$`)
	validHostnameRegex    = regexp.MustCompile(`^[a-zA-Z0-9]([a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?(\.[a-zA-Z0-9]([a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?)*$`)
)

type HostEntry struct {
	ContainerID         string `json:"container_id"`
	Hostname            string `json:"hostname"`
	IP                  string `json:"ip"`
	OwnerPID            int    `json:"owner_pid,omitempty"`
	OwnerStartTime      uint64 `json:"owner_start_time,omitempty"`
	GenerationAware     bool   `json:"generation_aware,omitempty"`
	GenerationPID       int    `json:"generation_pid,omitempty"`
	GenerationStartTime uint64 `json:"generation_start_time,omitempty"`
	AdmissionPending    bool   `json:"admission_pending,omitempty"`
}

type NetworkDNS struct {
	mu  sync.Mutex
	dir string
}

func DefaultDNSDir() string {
	return filepath.Join(state.DefaultDir(), "dns")
}

func validateNetworkName(name string) error {
	if name == "" {
		return fmt.Errorf("network name cannot be empty")
	}
	if name == "." || name == ".." || strings.ContainsAny(name, "/\\:") {
		return fmt.Errorf("invalid network name %q: path separators and relative components not allowed", name)
	}
	if !validNetworkNameRegex.MatchString(name) {
		return fmt.Errorf("invalid network name %q: must start with alphanumeric character and contain only [a-zA-Z0-9_.-]", name)
	}
	return nil
}

func validateHostAndIP(hostname, ipAddr string) error {
	if hostname == "" {
		return fmt.Errorf("hostname cannot be empty")
	}
	if len(hostname) > maxDNSHostnameBytes {
		return fmt.Errorf("invalid hostname %q: exceeds %d-byte DNS name limit", hostname, maxDNSHostnameBytes)
	}
	if strings.ContainsAny(hostname, " \t\r\n\x00") || !validHostnameRegex.MatchString(hostname) {
		return fmt.Errorf("invalid hostname %q: must be a valid DNS name without whitespace or control characters", hostname)
	}
	if ipAddr == "" {
		return fmt.Errorf("IP address cannot be empty")
	}
	if net.ParseIP(ipAddr) == nil {
		return fmt.Errorf("invalid IP address %q", ipAddr)
	}
	return nil
}

func validateHostEntryOwner(entry HostEntry) error {
	legacy := entry.OwnerPID == 0 && entry.OwnerStartTime == 0
	if !legacy && (entry.OwnerPID <= 0 || entry.OwnerStartTime == 0) {
		return fmt.Errorf("incomplete registrar process identity %d/%d", entry.OwnerPID, entry.OwnerStartTime)
	}
	generationUnset := entry.GenerationPID == 0 && entry.GenerationStartTime == 0
	if !generationUnset && (entry.GenerationPID <= 0 || entry.GenerationStartTime == 0) {
		return fmt.Errorf("incomplete child process identity %d/%d", entry.GenerationPID, entry.GenerationStartTime)
	}
	if entry.GenerationAware && legacy {
		return fmt.Errorf("generation-aware registration requires registrar ownership")
	}
	if !generationUnset && !entry.GenerationAware {
		return fmt.Errorf("child process identity requires generation-aware registration")
	}
	if entry.AdmissionPending && !entry.GenerationAware {
		return fmt.Errorf("pending admission requires generation-aware registration")
	}
	return nil
}

func validateEntries(networkName string, entries []HostEntry) error {
	seenContainers := make(map[string]struct{}, len(entries))
	seenHostnames := make(map[string]struct{}, len(entries))
	for i, entry := range entries {
		if strings.TrimSpace(entry.ContainerID) == "" {
			return fmt.Errorf("DNS registry %q entry %d has empty container ID", networkName, i)
		}
		if err := validateHostAndIP(entry.Hostname, entry.IP); err != nil {
			return fmt.Errorf("DNS registry %q entry %d is invalid: %w", networkName, i, err)
		}
		canonicalIP, err := canonicalIPAddress(entry.IP)
		if err != nil {
			return fmt.Errorf("DNS registry %q entry %d is invalid: %w", networkName, i, err)
		}
		if entry.IP != canonicalIP {
			return fmt.Errorf("DNS registry %q entry %d has non-canonical IP address %q; canonical form is %q", networkName, i, entry.IP, canonicalIP)
		}
		if err := validateHostEntryOwner(entry); err != nil {
			return fmt.Errorf("DNS registry %q entry %d has invalid ownership: %w", networkName, i, err)
		}
		if _, ok := seenContainers[entry.ContainerID]; ok {
			return fmt.Errorf("DNS registry %q has duplicate container ID %q", networkName, entry.ContainerID)
		}
		hostnameKey := strings.ToLower(entry.Hostname)
		if _, ok := seenHostnames[hostnameKey]; ok {
			return fmt.Errorf("DNS registry %q has duplicate hostname %q under case-insensitive DNS matching", networkName, entry.Hostname)
		}
		seenContainers[entry.ContainerID] = struct{}{}
		seenHostnames[hostnameKey] = struct{}{}
	}
	return nil
}

func pruneStaleOwnedEntries(entries []HostEntry) ([]HostEntry, bool, error) {
	if len(entries) == 0 {
		return entries, false, nil
	}
	kept := make([]HostEntry, 0, len(entries))
	changed := false
	for _, entry := range entries {
		active, err := hostEntryOwnerActive(entry)
		if err != nil {
			return nil, false, fmt.Errorf("resolve DNS ownership for container %s: %w", entry.ContainerID, err)
		}
		if active {
			kept = append(kept, entry)
			continue
		}
		changed = true
	}
	return kept, changed, nil
}

func ensureDNSDir() (string, error) {
	dir := DefaultDNSDir()
	if err := os.MkdirAll(dir, 0o700); err != nil {
		return "", fmt.Errorf("create DNS registry directory %q: %w", dir, err)
	}
	info, err := os.Lstat(dir)
	if err != nil {
		return "", fmt.Errorf("inspect DNS registry directory %q: %w", dir, err)
	}
	if info.Mode()&os.ModeSymlink != 0 || !info.IsDir() {
		return "", fmt.Errorf("DNS registry path %q must be a real directory", dir)
	}
	if err := os.Chmod(dir, 0o700); err != nil {
		return "", fmt.Errorf("chmod DNS registry directory %q: %w", dir, err)
	}
	return dir, nil
}

func loadEntriesChecked(path, networkName string) ([]HostEntry, bool, error) {
	data, exists, err := readDNSRegistryFile(path, networkName)
	if err != nil || !exists {
		return nil, exists, err
	}
	entries, err := decodeDNSRegistry(data, networkName)
	if err != nil {
		return nil, false, fmt.Errorf("parse DNS registry %q: %w", networkName, err)
	}
	if err := validateEntries(networkName, entries); err != nil {
		return nil, false, err
	}
	return entries, true, nil
}

func loadEntriesCheckedAt(dirFD int, name, networkName string) ([]HostEntry, bool, error) {
	data, exists, err := readDNSRegistryFileAt(dirFD, name, networkName)
	if err != nil || !exists {
		return nil, exists, err
	}
	entries, err := decodeDNSRegistry(data, networkName)
	if err != nil {
		return nil, false, fmt.Errorf("parse DNS registry %q: %w", networkName, err)
	}
	if err := validateEntries(networkName, entries); err != nil {
		return nil, false, err
	}
	return entries, true, nil
}

func saveEntriesAtomic(dir, path, networkName string, entries []HostEntry) error {
	if err := validateEntries(networkName, entries); err != nil {
		return err
	}
	data, err := encodeDNSRegistry(networkName, entries)
	if err != nil {
		return fmt.Errorf("marshal DNS registry %q: %w", networkName, err)
	}

	tmp, err := os.CreateTemp(dir, "."+networkName+".json.tmp-*")
	if err != nil {
		return fmt.Errorf("create DNS registry temp file %q: %w", networkName, err)
	}
	tmpName := tmp.Name()
	closed := false
	defer func() {
		if !closed {
			_ = tmp.Close()
		}
		_ = os.Remove(tmpName)
	}()

	if err := tmp.Chmod(0o600); err != nil {
		return fmt.Errorf("chmod DNS registry temp file %q: %w", networkName, err)
	}
	n, err := tmp.Write(data)
	if err != nil {
		return fmt.Errorf("write DNS registry temp file %q: %w", networkName, err)
	}
	if n != len(data) {
		return fmt.Errorf("write DNS registry temp file %q: short write %d/%d", networkName, n, len(data))
	}
	if err := tmp.Sync(); err != nil {
		return fmt.Errorf("sync DNS registry temp file %q: %w", networkName, err)
	}
	if err := tmp.Close(); err != nil {
		return fmt.Errorf("close DNS registry temp file %q: %w", networkName, err)
	}
	closed = true
	if err := os.Rename(tmpName, path); err != nil {
		return fmt.Errorf("publish DNS registry %q: %w", networkName, err)
	}

	dirFile, err := os.Open(dir)
	if err != nil {
		return fmt.Errorf("open DNS registry directory for sync: %w", err)
	}
	defer dirFile.Close()
	if err := dirFile.Sync(); err != nil {
		return fmt.Errorf("sync DNS registry directory: %w", err)
	}
	return nil
}

func saveEntriesAtomicAt(dirFD int, name, networkName string, entries []HostEntry) error {
	if err := validateEntries(networkName, entries); err != nil {
		return err
	}
	data, err := encodeDNSRegistry(networkName, entries)
	if err != nil {
		return fmt.Errorf("marshal DNS registry %q: %w", networkName, err)
	}
	return saveDNSRegistryFileAtomicAt(dirFD, name, networkName, data)
}

func entriesWithRegistration(entries []HostEntry, owner registrarIdentity, containerID, hostname, ipAddr string, admissionPending bool) ([]HostEntry, bool, error) {
	canonicalIP, err := canonicalIPAddress(ipAddr)
	if err != nil {
		return nil, false, err
	}
	ipAddr = canonicalIP
	for i, entry := range entries {
		hostnameMatches := strings.EqualFold(entry.Hostname, hostname)
		if entry.ContainerID != containerID && !hostnameMatches {
			continue
		}
		if entry.ContainerID == containerID && hostnameMatches && entry.IP == ipAddr && entry.OwnerPID == owner.PID && entry.OwnerStartTime == owner.StartTime {
			if entry.GenerationAware && entry.GenerationPID == 0 && entry.GenerationStartTime == 0 && entry.AdmissionPending == admissionPending {
				return entries, false, nil
			}
			updated := append([]HostEntry(nil), entries...)
			updated[i].GenerationAware = true
			updated[i].GenerationPID = 0
			updated[i].GenerationStartTime = 0
			updated[i].AdmissionPending = admissionPending
			return updated, true, nil
		}
		return nil, false, fmt.Errorf("live DNS registration conflict: container %q/hostname %q is owned by registrar %d/%d", entry.ContainerID, entry.Hostname, entry.OwnerPID, entry.OwnerStartTime)
	}
	updated := append([]HostEntry(nil), entries...)
	updated = append(updated, HostEntry{ContainerID: containerID, Hostname: hostname, IP: ipAddr, OwnerPID: owner.PID, OwnerStartTime: owner.StartTime, GenerationAware: true, AdmissionPending: admissionPending})
	return updated, true, nil
}

func registerHost(networkName, containerID, hostname, ipAddr string, admissionPending bool) error {
	if err := validateNetworkName(networkName); err != nil {
		return err
	}
	if strings.TrimSpace(containerID) == "" {
		return fmt.Errorf("container ID cannot be empty")
	}
	if err := validateHostAndIP(hostname, ipAddr); err != nil {
		return err
	}
	owner, err := currentRegistrarIdentity()
	if err != nil {
		return err
	}

	dnsMu.Lock()
	defer dnsMu.Unlock()
	dir, err := ensureDNSDir()
	if err != nil {
		return err
	}
	return withDNSNetworkLock(dir, networkName, func(dirFD int) error {
		netName := networkName + ".json"
		entries, _, err := loadEntriesCheckedAt(dirFD, netName, networkName)
		if err != nil {
			return err
		}
		entries, pruned, err := pruneStaleOwnedEntries(entries)
		if err != nil {
			return err
		}
		updated, changed, err := entriesWithRegistration(entries, owner, containerID, hostname, ipAddr, admissionPending)
		if err != nil {
			return err
		}
		if !changed && !pruned {
			return nil
		}
		return saveEntriesAtomicAt(dirFD, netName, networkName, updated)
	})
}

func RegisterHost(networkName, containerID, hostname, ipAddr string) error {
	return registerHost(networkName, containerID, hostname, ipAddr, false)
}

func registerHostAdmission(networkName, containerID, hostname, ipAddr string) error {
	return registerHost(networkName, containerID, hostname, ipAddr, true)
}

func UnregisterHost(networkName, containerID string) error {
	return UnregisterHostOwned(networkName, containerID)
}

func GenerateHostsContentChecked(networkName string) (string, error) {
	if err := validateNetworkName(networkName); err != nil {
		return "", err
	}

	dnsMu.Lock()
	defer dnsMu.Unlock()
	dir, err := ensureDNSDir()
	if err != nil {
		return "", err
	}
	var entries []HostEntry
	if err := withDNSNetworkLock(dir, networkName, func(dirFD int) error {
		netName := networkName + ".json"
		var exists bool
		var loadErr error
		entries, exists, loadErr = loadEntriesCheckedAt(dirFD, netName, networkName)
		if loadErr != nil || !exists {
			return loadErr
		}
		var changed bool
		entries, changed, loadErr = pruneStaleOwnedEntries(entries)
		if loadErr != nil {
			return loadErr
		}
		if changed {
			return saveEntriesAtomicAt(dirFD, netName, networkName, entries)
		}
		return nil
	}); err != nil {
		return "", err
	}

	lines := []string{
		"127.0.0.1\tlocalhost",
		"::1\tlocalhost ip6-localhost ip6-loopback",
		"# Mini Docker Network Service Discovery (" + networkName + ")",
	}
	for _, entry := range entries {
		if entry.AdmissionPending {
			continue
		}
		lines = append(lines, fmt.Sprintf("%s\t%s", entry.IP, entry.Hostname))
	}
	return strings.Join(lines, "\n") + "\n", nil
}

func GenerateHostsContent(networkName string) string {
	content, err := GenerateHostsContentChecked(networkName)
	if err != nil {
		return ""
	}
	return content
}

func InjectHostsIntoRootFS(rootfsPath, networkName string) error {
	if rootfsPath == "" {
		return fmt.Errorf("rootfs path cannot be empty")
	}
	if err := validateNetworkName(networkName); err != nil {
		return err
	}
	rootfsAbs, err := filepath.Abs(rootfsPath)
	if err != nil {
		return fmt.Errorf("resolve rootfs path %q: %w", rootfsPath, err)
	}
	st, err := os.Stat(rootfsAbs)
	if err != nil {
		return fmt.Errorf("stat rootfs %q: %w", rootfsPath, err)
	}
	if !st.IsDir() {
		return fmt.Errorf("rootfs %q is not a directory", rootfsPath)
	}
	return nil
}

func isSubDir(base, target string) bool {
	baseAbs, err1 := filepath.Abs(base)
	targetAbs, err2 := filepath.Abs(target)
	if err1 != nil || err2 != nil {
		return false
	}
	rel, err := filepath.Rel(baseAbs, targetAbs)
	if err != nil || rel == ".." || strings.HasPrefix(rel, ".."+string(filepath.Separator)) {
		return false
	}
	return true
}
