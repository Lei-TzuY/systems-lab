//go:build !linux

package cgroups

func ReadMemoryPSI(cgroupPath string) (string, error) {
	return "some avg10=0.00 avg60=0.00 avg300=0.00 total=0\nfull avg10=0.00 avg60=0.00 avg300=0.00 total=0\n", nil
}
