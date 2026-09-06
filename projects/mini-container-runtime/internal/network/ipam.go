package network

import (
	"encoding/json"
	"fmt"
	"net"
	"os"
	"path/filepath"
	"strings"
	"sync"

	"minicontainer/internal/state"
)

// IPAM manages persistent IP address allocation for container bridge networks.
// The in-process mutex protects a single manager from local races; every pool
// mutation is additionally serialized by a per-network kernel lock so
// independent minictl processes cannot allocate the same address concurrently.
type IPAM struct {
	mu      sync.Mutex
	dir     string
	initErr error
}

type SubnetPool struct {
	Subnet    string            `json:"subnet"`
	Allocated map[string]string `json:"allocated"` // IP -> ContainerID
}

func DefaultIPAMDir() string {
	return filepath.Join(state.DefaultDir(), "ipam")
}

// OpenIPAM opens an IPAM state directory with private permissions. A symlinked
// or non-directory state path is rejected so pool and lock files cannot be
// redirected outside the runtime-owned directory.
func OpenIPAM(dir string) (*IPAM, error) {
	if strings.TrimSpace(dir) == "" {
		return nil, fmt.Errorf("IPAM directory cannot be empty")
	}
	if err := os.MkdirAll(dir, 0700); err != nil {
		return nil, fmt.Errorf("create IPAM directory %q: %w", dir, err)
	}
	info, err := os.Lstat(dir)
	if err != nil {
		return nil, fmt.Errorf("inspect IPAM directory %q: %w", dir, err)
	}
	if info.Mode()&os.ModeSymlink != 0 || !info.IsDir() {
		return nil, fmt.Errorf("IPAM path %q must be a real directory", dir)
	}
	if err := os.Chmod(dir, 0700); err != nil {
		return nil, fmt.Errorf("chmod IPAM directory %q: %w", dir, err)
	}
	return &IPAM{dir: dir}, nil
}

// NewIPAM preserves the historical no-error constructor. Initialization errors
// are retained and surfaced by AllocateIP/ReleaseIP instead of being silently
// ignored.
func NewIPAM() *IPAM {
	dir := DefaultIPAMDir()
	ipam, err := OpenIPAM(dir)
	if err != nil {
		return &IPAM{dir: dir, initErr: err}
	}
	return ipam
}

func (ipam *IPAM) ready() error {
	if ipam == nil {
		return fmt.Errorf("IPAM manager is nil")
	}
	if ipam.initErr != nil {
		return ipam.initErr
	}
	if ipam.dir == "" {
		return fmt.Errorf("IPAM directory is not configured")
	}
	return nil
}

func validateNetworkName(name string) error {
	if name == "" {
		return fmt.Errorf("network name cannot be empty")
	}
	if name == "." || name == ".." || strings.ContainsAny(name, "/\\:") {
		return fmt.Errorf("invalid network name %q: path separators and relative components not allowed", name)
	}
	return nil
}

func validateContainerID(containerID string) error {
	if strings.TrimSpace(containerID) == "" {
		return fmt.Errorf("container ID cannot be empty")
	}
	return nil
}

func canonicalSubnet(cidr string) (*net.IPNet, string, error) {
	if cidr == "" {
		cidr = "172.20.0.0/24"
	}
	_, network, err := net.ParseCIDR(cidr)
	if err != nil {
		return nil, "", fmt.Errorf("invalid CIDR %q: %w", cidr, err)
	}
	return network, network.String(), nil
}

func subnetBroadcast(network *net.IPNet) net.IP {
	ipv4 := network.IP.To4()
	if ipv4 == nil {
		return nil
	}
	ones, bits := network.Mask.Size()
	if bits != 32 || ones > 30 {
		return nil
	}
	broadcast := make(net.IP, 4)
	for i := 0; i < 4; i++ {
		broadcast[i] = ipv4[i] | ^network.Mask[i]
	}
	return broadcast
}

func validatePoolAllocations(networkName string, pool *SubnetPool, network *net.IPNet) error {
	gateway := append(net.IP(nil), network.IP...)
	incIP(gateway)
	broadcast := subnetBroadcast(network)
	owners := make(map[string]string, len(pool.Allocated))

	for ipStr, owner := range pool.Allocated {
		ip := net.ParseIP(ipStr)
		if ip == nil || ip.String() != ipStr {
			return fmt.Errorf("IPAM pool %q has invalid allocation address %q", networkName, ipStr)
		}
		if !network.Contains(ip) {
			return fmt.Errorf("IPAM pool %q allocation %s is outside subnet %s", networkName, ipStr, network.String())
		}
		if ip.Equal(network.IP) || ip.Equal(gateway) || (broadcast != nil && ip.Equal(broadcast)) {
			return fmt.Errorf("IPAM pool %q allocation %s uses a reserved subnet address", networkName, ipStr)
		}
		if err := validateContainerID(owner); err != nil {
			return fmt.Errorf("IPAM pool %q allocation %s has invalid owner: %w", networkName, ipStr, err)
		}
		if previousIP, exists := owners[owner]; exists && previousIP != ipStr {
			return fmt.Errorf("IPAM pool %q assigns container %q more than one address (%s, %s)", networkName, owner, previousIP, ipStr)
		}
		owners[owner] = ipStr
	}
	return nil
}

// AllocateIP allocates a free IP address in subnet CIDR (e.g. 172.20.0.0/24)
// for containerID. The read-modify-write sequence is performed while holding a
// cross-process per-network lock.
func (ipam *IPAM) AllocateIP(networkName, cidr, containerID string) (string, error) {
	if err := ipam.ready(); err != nil {
		return "", err
	}
	if err := validateNetworkName(networkName); err != nil {
		return "", err
	}
	if err := validateContainerID(containerID); err != nil {
		return "", err
	}
	netObj, canonicalCIDR, err := canonicalSubnet(cidr)
	if err != nil {
		return "", err
	}

	ipam.mu.Lock()
	defer ipam.mu.Unlock()

	var allocated string
	err = withIPAMNetworkLock(ipam.dir, networkName, func() error {
		pool, err := ipam.loadPool(networkName, canonicalCIDR)
		if err != nil {
			return err
		}

		for allocatedIP, cID := range pool.Allocated {
			if cID == containerID {
				allocated = allocatedIP
				return nil
			}
		}

		broadcastIP := subnetBroadcast(netObj)
		currIP := append(net.IP(nil), netObj.IP...)
		incIP(currIP) // skip network address
		incIP(currIP) // skip gateway address
		for netObj.Contains(currIP) {
			if broadcastIP != nil && currIP.Equal(broadcastIP) {
				break
			}
			ipStr := currIP.String()
			if _, taken := pool.Allocated[ipStr]; !taken {
				pool.Allocated[ipStr] = containerID
				if err := ipam.savePool(networkName, pool); err != nil {
					return fmt.Errorf("save IPAM pool for %q: %w", networkName, err)
				}
				allocated = ipStr
				return nil
			}
			incIP(currIP)
		}
		return fmt.Errorf("subnet %s exhausted", canonicalCIDR)
	})
	if err != nil {
		return "", err
	}
	return allocated, nil
}

// ReleaseIP frees an allocated IP address under the same cross-process lock
// used by allocation, preventing lost updates between independent runtimes.
func (ipam *IPAM) ReleaseIP(networkName, containerID string) error {
	if err := ipam.ready(); err != nil {
		return err
	}
	if err := validateNetworkName(networkName); err != nil {
		return err
	}
	if err := validateContainerID(containerID); err != nil {
		return err
	}

	ipam.mu.Lock()
	defer ipam.mu.Unlock()

	return withIPAMNetworkLock(ipam.dir, networkName, func() error {
		pool, exists, err := ipam.loadExistingPool(networkName)
		if err != nil || !exists {
			return err
		}
		for ipStr, cID := range pool.Allocated {
			if cID == containerID {
				delete(pool.Allocated, ipStr)
				if err := ipam.savePool(networkName, pool); err != nil {
					return fmt.Errorf("save IPAM pool for %q: %w", networkName, err)
				}
				return nil
			}
		}
		return nil
	})
}

func (ipam *IPAM) loadPool(networkName, expectedCIDR string) (*SubnetPool, error) {
	pool, exists, err := ipam.loadExistingPool(networkName)
	if err != nil {
		return nil, err
	}
	if !exists {
		return &SubnetPool{Subnet: expectedCIDR, Allocated: make(map[string]string)}, nil
	}
	if pool.Subnet != expectedCIDR {
		return nil, fmt.Errorf("IPAM pool %q subnet mismatch: stored %s, requested %s", networkName, pool.Subnet, expectedCIDR)
	}
	return pool, nil
}

func (ipam *IPAM) loadExistingPool(networkName string) (*SubnetPool, bool, error) {
	poolFile := filepath.Join(ipam.dir, networkName+".json")
	info, err := os.Lstat(poolFile)
	if err != nil {
		if os.IsNotExist(err) {
			return nil, false, nil
		}
		return nil, false, fmt.Errorf("inspect IPAM pool %q: %w", networkName, err)
	}
	if info.Mode()&os.ModeSymlink != 0 || !info.Mode().IsRegular() {
		return nil, false, fmt.Errorf("IPAM pool %q must be a regular file", networkName)
	}
	if err := os.Chmod(poolFile, 0600); err != nil {
		return nil, false, fmt.Errorf("chmod IPAM pool %q: %w", networkName, err)
	}
	data, err := os.ReadFile(poolFile)
	if err != nil {
		return nil, false, fmt.Errorf("read IPAM pool %q: %w", networkName, err)
	}
	var pool SubnetPool
	if err := json.Unmarshal(data, &pool); err != nil {
		return nil, false, fmt.Errorf("parse IPAM pool %q: %w", networkName, err)
	}
	if pool.Allocated == nil {
		pool.Allocated = make(map[string]string)
	}
	network, canonicalCIDR, err := canonicalSubnet(pool.Subnet)
	if err != nil {
		return nil, false, fmt.Errorf("IPAM pool %q has invalid stored subnet %q: %w", networkName, pool.Subnet, err)
	}
	pool.Subnet = canonicalCIDR
	if err := validatePoolAllocations(networkName, &pool, network); err != nil {
		return nil, false, err
	}
	return &pool, true, nil
}

func (ipam *IPAM) savePool(networkName string, pool *SubnetPool) error {
	if pool == nil {
		return fmt.Errorf("IPAM pool cannot be nil")
	}
	data, err := json.MarshalIndent(pool, "", "  ")
	if err != nil {
		return fmt.Errorf("marshal pool: %w", err)
	}

	tmp, err := os.CreateTemp(ipam.dir, "."+networkName+".json.tmp-*")
	if err != nil {
		return fmt.Errorf("create temporary pool file: %w", err)
	}
	tmpName := tmp.Name()
	committed := false
	defer func() {
		_ = tmp.Close()
		if !committed {
			_ = os.Remove(tmpName)
		}
	}()

	if err := tmp.Chmod(0600); err != nil {
		return fmt.Errorf("chmod temporary pool file: %w", err)
	}
	if _, err := tmp.Write(data); err != nil {
		return fmt.Errorf("write temporary pool file: %w", err)
	}
	if err := tmp.Sync(); err != nil {
		return fmt.Errorf("fsync temporary pool file: %w", err)
	}
	if err := tmp.Close(); err != nil {
		return fmt.Errorf("close temporary pool file: %w", err)
	}

	poolFile := filepath.Join(ipam.dir, networkName+".json")
	if err := os.Rename(tmpName, poolFile); err != nil {
		return fmt.Errorf("atomically replace pool file: %w", err)
	}
	committed = true

	dir, err := os.Open(ipam.dir)
	if err != nil {
		return fmt.Errorf("open IPAM directory for fsync: %w", err)
	}
	defer dir.Close()
	if err := dir.Sync(); err != nil {
		return fmt.Errorf("fsync IPAM directory: %w", err)
	}
	return nil
}

func incIP(ip net.IP) {
	for j := len(ip) - 1; j >= 0; j-- {
		ip[j]++
		if ip[j] > 0 {
			break
		}
	}
}
