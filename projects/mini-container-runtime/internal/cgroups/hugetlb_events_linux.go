//go:build linux

package cgroups

import (
	"bufio"
	"fmt"
	"os"
	"path/filepath"
	"strconv"
	"strings"
)

// ReadHugeTLBEventsMaxCount reads the `max N` fail counter from hugetlb.<pageSize>.events.
func ReadHugeTLBEventsMaxCount(cgroupPath, pageSize string) (uint64, error) {
	if pageSize == "" {
		pageSize = "2MB"
	}
	eventsFile := filepath.Join(cgroupPath, fmt.Sprintf("hugetlb.%s.events", pageSize))
	file, err := os.Open(eventsFile)
	if err != nil {
		if os.IsNotExist(err) {
			return 0, nil
		}
		return 0, err
	}
	defer file.Close()

	scanner := bufio.NewScanner(file)
	for scanner.Scan() {
		fields := strings.Fields(scanner.Text())
		if len(fields) == 2 && fields[0] == "max" {
			return strconv.ParseUint(fields[1], 10, 64)
		}
	}

	return 0, nil
}

// ReadHugeTLBCurrentBytes reads current hugepage bytes in use from hugetlb.<pageSize>.current.
func ReadHugeTLBCurrentBytes(cgroupPath, pageSize string) (uint64, error) {
	if pageSize == "" {
		pageSize = "2MB"
	}
	curFile := filepath.Join(cgroupPath, fmt.Sprintf("hugetlb.%s.current", pageSize))
	data, err := os.ReadFile(curFile)
	if err != nil {
		if os.IsNotExist(err) {
			return 0, nil
		}
		return 0, err
	}
	val := strings.TrimSpace(string(data))
	if val == "" {
		return 0, nil
	}
	return strconv.ParseUint(val, 10, 64)
}
