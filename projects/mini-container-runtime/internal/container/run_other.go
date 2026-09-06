//go:build !linux

package container

import (
	"fmt"
	"runtime"
)

func Run(Config) error {
	return fmt.Errorf("container runtime requires Linux namespaces and cgroups; current OS is %s", runtime.GOOS)
}

func ContainerInit(Config) error {
	return fmt.Errorf("container init requires Linux namespaces and cgroups; current OS is %s", runtime.GOOS)
}
