//go:build !linux

package cgroups

import "fmt"

func RemoveChecked(name string, debug bool) error {
	return fmt.Errorf("cgroups cleanup requires Linux")
}
