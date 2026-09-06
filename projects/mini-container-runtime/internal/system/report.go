package system

import (
	"fmt"
	"runtime"

	"minicontainer/internal/state"
)

type EngineReport struct {
	EngineVersion string      `json:"engine_version"`
	GoVersion     string      `json:"go_version"`
	HostOS        string      `json:"host_os"`
	HostArch      string      `json:"host_arch"`
	Containers    int         `json:"containers"`
	Images        int         `json:"images"`
	Volumes       int         `json:"volumes"`
	KernelChecks  CheckResult `json:"kernel_checks"`
}

// GenerateEngineReport builds a full system telemetry diagnostic report.
func GenerateEngineReport(st *state.Store) (*EngineReport, error) {
	if st == nil {
		return nil, fmt.Errorf("state store is nil")
	}

	df, err := CalculateDF(st)
	if err != nil {
		return nil, err
	}

	kChecks := CheckKernelFeatures()

	report := &EngineReport{
		EngineVersion: "1.6.0",
		GoVersion:     runtime.Version(),
		HostOS:        runtime.GOOS,
		HostArch:      runtime.GOARCH,
		Containers:    df.ContainersCount,
		Images:        df.ImagesCount,
		Volumes:       df.VolumesCount,
		KernelChecks:  kChecks,
	}

	return report, nil
}
