//go:build linux

package cgroups

import (
	"bufio"
	"os"
	"path/filepath"
	"strconv"
	"strings"
)

// MiscResourceUsage represents a single hardware/misc resource allocation (e.g. sev, sev_es, tdx).
type MiscResourceUsage struct {
	ResourceName string
	Usage        uint64
}

// ReadMiscCurrent parses misc.current in the cgroup directory.
func ReadMiscCurrent(cgroupPath string) ([]MiscResourceUsage, error) {
	file, err := os.Open(filepath.Join(cgroupPath, "misc.current"))
	if err != nil {
		if os.IsNotExist(err) {
			return nil, nil
		}
		return nil, err
	}
	defer file.Close()

	var list []MiscResourceUsage
	scanner := bufio.NewScanner(file)
	for scanner.Scan() {
		fields := strings.Fields(scanner.Text())
		if len(fields) != 2 {
			continue
		}
		val, _ := strconv.ParseUint(fields[1], 10, 64)
		list = append(list, MiscResourceUsage{
			ResourceName: fields[0],
			Usage:        val,
		})
	}

	return list, nil
}
