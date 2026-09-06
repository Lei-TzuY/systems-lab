//go:build !linux

package container

// RunPayloadExitCode never reports a payload exit on unsupported platforms.
// The non-Linux runtime stub fails before a container payload can execute.
func RunPayloadExitCode(error) (int, bool) {
	return 0, false
}
