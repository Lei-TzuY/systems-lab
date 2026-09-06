//go:build linux

// internal/container/run.go
//
// Container Runtime — Process Creation and Namespace Isolation

package container

import (
	"errors"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strconv"
	"syscall"
	"time"

	"minicontainer/internal/cgroups"
	"minicontainer/internal/dns"
	"minicontainer/internal/network"
	"minicontainer/internal/ns"
	"minicontainer/internal/rootfs"
	"minicontainer/internal/state"
)

const (
	sentinelEnvKey = "MINICONTAINER_INIT"
	sentinelEnv    = sentinelEnvKey + "=1"
)

type runtimeStateError struct {
	err error
}

func (e *runtimeStateError) Error() string { return e.err.Error() }
func (e *runtimeStateError) Unwrap() error { return e.err }

// Run launches a new container, optionally handles restart policies.
func Run(cfg Config) (resultErr error) {
	lifecycleStore, err := openLifecycleStore(cfg)
	if err != nil {
		return err
	}
	if lifecycleStore != nil {
		defer lifecycleStore.Close()
		defer func() {
			resultErr = finalizeCreatedRunFailure(lifecycleStore, cfg.ContainerID, resultErr, time.Now())
		}()
	}

	maxAttempts := 1
	if cfg.Restart == "always" || cfg.Restart == "on-failure" {
		maxAttempts = 5
	}

	attempt := 0
	for {
		attempt++
		rollbackAdmission, admissionErr := beginNetworkAttemptAdmission(cfg, lifecycleStore)
		if admissionErr != nil {
			return markPreGenerationRunFailure(admissionErr)
		}

		err := runOnce(cfg, lifecycleStore)
		if rollbackAdmission != nil {
			if cleanupErr := rollbackAdmission(); cleanupErr != nil {
				err = errors.Join(err, &runtimeSetupError{err: fmt.Errorf("rollback network attempt admission: %w", cleanupErr)})
			}
		}

		if isRuntimeControlError(err) {
			return err
		}

		if err == nil {
			if cfg.Restart == "always" && attempt < maxAttempts {
				if cfg.Debug {
					fmt.Printf("[parent] restart policy %q: restarting container (attempt %d)\n", cfg.Restart, attempt+1)
				}
				time.Sleep(1 * time.Second)
				continue
			}
			return nil
		}

		if (cfg.Restart == "always" || cfg.Restart == "on-failure") && attempt < maxAttempts {
			if cfg.Debug {
				fmt.Printf("[parent] container failed (%v); restart policy %q: retrying (attempt %d)\n", err, cfg.Restart, attempt+1)
			}
			time.Sleep(1 * time.Second)
			continue
		}
		return err
	}
}

func openLifecycleStore(cfg Config) (*state.Store, error) {
	if cfg.ContainerID == "" {
		return nil, nil
	}
	dir := cfg.StateDir
	if dir == "" {
		dir = state.DefaultDir()
	}
	st, err := state.Open(dir)
	if err != nil {
		return nil, &runtimeStateError{err: fmt.Errorf("open lifecycle state store: %w", err)}
	}
	return st, nil
}

func runOnce(cfg Config, lifecycleStore *state.Store) (resultErr error) {
	processStarted := false
	defer func() {
		if resultErr != nil && !processStarted {
			resultErr = markPreGenerationRunFailure(resultErr)
		}
	}()

	if cfg.Debug {
		fmt.Println("[parent] spawning child with new namespaces")
	}

	self, err := os.Executable()
	if err != nil {
		return fmt.Errorf("could not resolve executable path: %w", err)
	}

	childArgs := []string{"run"}
	if !cfg.UserNS {
		childArgs = append(childArgs, "--no-user-ns")
	}
	if cfg.Hostname != "" {
		childArgs = append(childArgs, "--hostname", cfg.Hostname)
	}
	if cfg.WorkDir != "" {
		childArgs = append(childArgs, "--workdir", cfg.WorkDir)
	}
	if cfg.Overlay {
		childArgs = append(childArgs, "--overlay")
	}
	if cfg.ReadOnly {
		childArgs = append(childArgs, "--read-only")
	}
	for _, c := range cfg.CapDrop {
		childArgs = append(childArgs, "--cap-drop", c)
	}
	for _, env := range cfg.Env {
		childArgs = append(childArgs, "--env", env)
	}
	if cfg.Memory > 0 {
		childArgs = append(childArgs, "--memory", strconv.FormatInt(cfg.Memory, 10))
	}
	if cfg.CPUWeight > 0 {
		childArgs = append(childArgs, "--cpu-weight", strconv.FormatInt(cfg.CPUWeight, 10))
	}
	if cfg.CPUs > 0 {
		childArgs = append(childArgs, "--cpus", strconv.FormatFloat(cfg.CPUs, 'f', -1, 64))
	}
	if cfg.PidsLimit > 0 {
		childArgs = append(childArgs, "--pids-limit", strconv.FormatInt(cfg.PidsLimit, 10))
	}
	if cfg.BridgeNetwork {
		childArgs = append(childArgs, "--bridge")
	}
	if cfg.Seccomp {
		childArgs = append(childArgs, "--seccomp")
	}
	for _, p := range cfg.PortMappings {
		spec := fmt.Sprintf("%d:%d", p.HostPort, p.ContainerPort)
		if p.Protocol != "" && p.Protocol != "tcp" {
			spec += "/" + p.Protocol
		}
		childArgs = append(childArgs, "--publish", spec)
	}
	for _, v := range cfg.Volumes {
		spec := v.HostPath + ":" + v.ContainerPath
		if v.ReadOnly {
			spec += ":ro"
		}
		childArgs = append(childArgs, "--volume", spec)
	}
	childArgs = append(childArgs, cfg.RootFS)
	childArgs = append(childArgs, cfg.Command...)
	cmd := exec.Command(self, childArgs...)

	cmd.Stdin = os.Stdin
	cmd.Stdout = os.Stdout
	cmd.Stderr = os.Stderr
	cmd.Env = append(os.Environ(), sentinelEnv)

	overlayWorkDir, err := createParentOverlayWorkDir(cfg.Overlay, os.MkdirTemp)
	if err != nil {
		return err
	}
	if overlayWorkDir != "" {
		cmd.Env = appendOverlayWorkDirEnv(cmd.Env, overlayWorkDir)
		defer func() {
			resultErr = finishOverlayWorkDir(resultErr, overlayWorkDir, os.RemoveAll)
			if cfg.Debug && resultErr == nil {
				fmt.Println("[parent] container exited cleanly")
			}
		}()
	}

	runtimeHostsFile, err := createRuntimeHostsFile(cfg.BridgeNetwork)
	if err != nil {
		return err
	}
	defer func() {
		if runtimeHostsFile != nil {
			_ = runtimeHostsFile.Close()
		}
	}()

	cmd.SysProcAttr = ns.BuildCloneFlags(ns.Options{
		UserNS:  cfg.UserNS,
		HostUID: os.Getuid(),
		HostGID: os.Getgid(),
	})

	readPipe, writePipe, err := os.Pipe()
	if err != nil {
		return fmt.Errorf("creating sync pipe: %w", err)
	}
	initStatusReadPipe, initStatusWritePipe, err := os.Pipe()
	if err != nil {
		_ = readPipe.Close()
		_ = writePipe.Close()
		return fmt.Errorf("creating runtime init status pipe: %w", err)
	}
	defer initStatusReadPipe.Close()
	cmd.ExtraFiles = []*os.File{readPipe, initStatusWritePipe}
	if runtimeHostsFile != nil {
		cmd.ExtraFiles = append(cmd.ExtraFiles, runtimeHostsFile)
	}

	if err := startContainerProcess(cfg, cmd); err != nil {
		_ = readPipe.Close()
		_ = writePipe.Close()
		_ = initStatusReadPipe.Close()
		_ = initStatusWritePipe.Close()
		return fmt.Errorf("starting container process: %w", err)
	}
	processStarted = true
	_ = readPipe.Close()
	_ = initStatusWritePipe.Close()
	if runtimeHostsFile != nil {
		_ = runtimeHostsFile.Close()
		runtimeHostsFile = nil
	}

	childPID := cmd.Process.Pid
	if cfg.Debug {
		fmt.Printf("[parent] child started, PID=%d\n", childPID)
	}

	cgName := fmt.Sprintf("minicontainer-%d", childPID)
	var childStartTime uint64
	if lifecycleStore != nil {
		childStartTime, err = ProcessStartTime(childPID)
		if err != nil {
			return abortPreRunningChildFailure(cmd, writePipe, &runtimeStateError{err: fmt.Errorf("capture process identity for container %s: %w", cfg.ContainerID, err)})
		}
		cgName, err = cgroups.NameForContainerProcess(cfg.ContainerID, childPID, childStartTime)
		if err != nil {
			return abortPreRunningChildFailure(cmd, writePipe, &runtimeStateError{err: fmt.Errorf("derive cgroup identity for container %s: %w", cfg.ContainerID, err)})
		}
		startedAt := time.Now()
		if err := lifecycleStore.MarkRunning(cfg.ContainerID, childPID, childStartTime, startedAt); err != nil {
			return abortPreRunningChildFailure(cmd, writePipe, &runtimeStateError{err: fmt.Errorf("persist running state for container %s: %w", cfg.ContainerID, err)})
		}
	}

	cgCfg := cgroups.Config{
		Name:      cgName,
		MemoryMax: cfg.Memory,
		CPUWeight: cfg.CPUWeight,
		CPUs:      cfg.CPUs,
		PidsMax:   cfg.PidsLimit,
	}
	cgroupApplied, cgroupErr := applyCgroupWithDurableOwnership(
		lifecycleStore,
		cfg.ContainerID,
		childPID,
		childStartTime,
		cgCfg,
		cfg.Debug,
		cgroups.Apply,
	)
	if cgroupErr != nil {
		if isRuntimeControlError(cgroupErr) || resourceLimitsRequested(cfg) {
			return abortRuntimeSetupFailure(cmd, writePipe, lifecycleStore, cfg.ContainerID, childPID, childStartTime, fmt.Errorf("prepare required cgroup controls: %w", cgroupErr))
		}
		fmt.Fprintf(os.Stderr, "[parent] warning: cgroup setup failed: %v\n", cgroupErr)
	}

	const (
		hostCIDR    = "172.20.0.1/24"
		containerIP = "172.20.0.2"
	)

	var bridgeCleanup func() error
	if cfg.BridgeNetwork {
		networkOwner, err := network.NewPortForwardingOwner()
		if err != nil {
			return abortRuntimeSetupFailure(cmd, writePipe, lifecycleStore, cfg.ContainerID, childPID, childStartTime, fmt.Errorf("create bridge ownership marker: %w", err))
		}
		if lifecycleStore == nil {
			return abortRuntimeSetupFailure(cmd, writePipe, lifecycleStore, cfg.ContainerID, childPID, childStartTime, &runtimeStateError{err: fmt.Errorf("bridge networking requires managed lifecycle state")})
		}

		persistedNetworkOwnership := networkOwnershipForGeneration(networkOwner, childPID, childStartTime, containerIP, cfg.PortMappings)
		if err := lifecycleStore.MarkNetworkOwnedIfIdentity(cfg.ContainerID, persistedNetworkOwnership); err != nil {
			return abortRuntimeSetupFailure(cmd, writePipe, lifecycleStore, cfg.ContainerID, childPID, childStartTime, &runtimeStateError{err: fmt.Errorf("persist network ownership for container %s: %w", cfg.ContainerID, err)})
		}

		bridgeCleanup, err = setupBridgeHostOwned(childPID, hostCIDR, containerIP, cfg.PortMappings, networkOwner, cfg.Debug)
		if err != nil {
			setupErr := error(fmt.Errorf("configure required bridge network: %w", err))
			if cleanupErr := cleanupNetworkOwnership(lifecycleStore, cfg.ContainerID, persistedNetworkOwnership, cfg.Debug); cleanupErr != nil {
				setupErr = errors.Join(setupErr, fmt.Errorf("cleanup persisted network resources after bridge setup failure: %w", cleanupErr))
			}
			return abortRuntimeSetupFailure(cmd, writePipe, lifecycleStore, cfg.ContainerID, childPID, childStartTime, setupErr)
		}
		if err := dns.BindHostRegistrationGeneration(defaultBridgeDNSNetwork, cfg.ContainerID, childPID, childStartTime); err != nil {
			return abortRuntimeSetupFailure(
				cmd,
				writePipe,
				lifecycleStore,
				cfg.ContainerID,
				childPID,
				childStartTime,
				fmt.Errorf("bind bridge DNS registration to child generation: %w", err),
			)
		}

		baseBridgeCleanup := bridgeCleanup
		bridgeCleanup = func() error {
			var cleanupErr error
			if baseBridgeCleanup != nil {
				cleanupErr = baseBridgeCleanup()
			}
			if err := cleanupNetworkOwnership(lifecycleStore, cfg.ContainerID, persistedNetworkOwnership, cfg.Debug); err != nil {
				cleanupErr = errors.Join(cleanupErr, fmt.Errorf("reconcile persisted network cleanup: %w", err))
			}
			return cleanupErr
		}
	}

	if err := releaseBlockedChild(writePipe); err != nil {
		var setupErr error = fmt.Errorf("release container initialization: %w", err)
		if bridgeCleanup != nil {
			if cleanupErr := bridgeCleanup(); cleanupErr != nil {
				setupErr = errors.Join(setupErr, fmt.Errorf("cleanup bridge network after release failure: %w", cleanupErr))
			}
			bridgeCleanup = nil
		}
		return abortRuntimeSetupFailure(cmd, writePipe, lifecycleStore, cfg.ContainerID, childPID, childStartTime, setupErr)
	}

	initStatusErr := awaitPayloadExec(initStatusReadPipe)
	_ = initStatusReadPipe.Close()

	waitErr := cmd.Wait()

	bridgeCleanupErr := cleanupBridgeAfterNormalExit(lifecycleStore, bridgeCleanup)

	var finalizationErr error
	if lifecycleStore != nil {
		snapshot := &state.Container{ID: cfg.ContainerID, PID: childPID, PIDStartTime: childStartTime}
		finalizationErr = finalizeManagedParentExit(lifecycleStore, snapshot, exitCodeFromWaitError(waitErr), time.Now(), cgroupApplied, FinalizeStoppedGeneration)
	} else if cgroupApplied {
		if err := cgroups.RemoveChecked(cgCfg.Name, cfg.Debug); err != nil {
			finalizationErr = fmt.Errorf("cleanup cgroup %s: %w", cgCfg.Name, err)
		}
	}

	resultErr = parentExitResult(waitErr, finalizationErr, bridgeCleanupErr)
	resultErr = joinRuntimeInitFailure(resultErr, initStatusErr)
	if resultErr != nil {
		return resultErr
	}

	if cfg.Debug && overlayWorkDir == "" {
		fmt.Println("[parent] container exited cleanly")
	}
	return nil
}

func abortBlockedChild(cmd *exec.Cmd, writePipe *os.File) {
	if cmd != nil && cmd.Process != nil {
		_ = cmd.Process.Kill()
	}
	if writePipe != nil {
		_ = writePipe.Close()
	}
	if cmd != nil {
		_ = cmd.Wait()
	}
}

func exitCodeFromWaitError(err error) int {
	if err == nil {
		return 0
	}
	var exitErr *exec.ExitError
	if errors.As(err, &exitErr) {
		return exitErr.ExitCode()
	}
	return 1
}

// ContainerInit is called when the re-executed child detects sentinelEnv.
func ContainerInit(cfg Config) (resultErr error) {
	initStatus, err := openRuntimeInitStatusWriter()
	if err != nil {
		return fmt.Errorf("open runtime init status: %w", err)
	}
	defer func() { initStatus.finish(resultErr) }()

	runtimeHostsFile, err := runtimeHostsFileFromFD(cfg.BridgeNetwork)
	if err != nil {
		return fmt.Errorf("open runtime hosts file: %w", err)
	}
	defer func() {
		if runtimeHostsFile != nil {
			_ = runtimeHostsFile.Close()
		}
	}()

	if cfg.Debug {
		fmt.Println("[init] running inside new namespaces")
	}

	syncPipe := os.NewFile(3, "sync-pipe")
	if err := awaitParentReady(syncPipe); err != nil {
		return fmt.Errorf("runtime parent readiness: %w", err)
	}

	if cfg.Debug {
		fmt.Println("[init] received runtime ready signal from parent")
	}

	if err := syscall.Mount("", "/", "", syscall.MS_REC|syscall.MS_PRIVATE, ""); err != nil {
		return fmt.Errorf("make mount namespace private: %w", err)
	}
	if cfg.Debug {
		fmt.Println("[init] mount namespace propagation set to private")
	}

	overlayTmp, err := consumeOverlayWorkDir(cfg.Overlay)
	if err != nil {
		return fmt.Errorf("runtime overlay workdir: %w", err)
	}

	targetRootFS := cfg.RootFS
	if cfg.Overlay {
		overlayDirs, err := rootfs.PrepareOverlay(cfg.RootFS, overlayTmp)
		if err != nil {
			return fmt.Errorf("prepare overlay: %w", err)
		}
		targetRootFS = overlayDirs.Merged
		if cfg.Debug {
			fmt.Printf("[init] overlayfs mounted (%s -> %s)\n", cfg.RootFS, targetRootFS)
		}
	}

	if err := mountRuntimeHostsFile(runtimeHostsFile, targetRootFS, cfg.Debug); err != nil {
		return fmt.Errorf("runtime hosts: %w", err)
	}
	if runtimeHostsFile != nil {
		_ = runtimeHostsFile.Close()
		runtimeHostsFile = nil
	}

	hostname := cfg.Hostname
	if hostname == "" {
		hostname = "minicontainer"
	}
	if err := syscall.Sethostname([]byte(hostname)); err != nil {
		return fmt.Errorf("sethostname: %w", err)
	}
	if cfg.Debug {
		fmt.Printf("[init] hostname set to %q\n", hostname)
	}

	procPath := filepath.Join(targetRootFS, "proc")
	if err := os.MkdirAll(procPath, 0755); err != nil {
		return fmt.Errorf("mkdir proc: %w", err)
	}
	if err := syscall.Mount("proc", procPath, "proc", 0, ""); err != nil {
		return fmt.Errorf("mount proc: %w", err)
	}
	if cfg.Debug {
		fmt.Println("[init] /proc mounted")
	}

	sysPath := filepath.Join(targetRootFS, "sys")
	if err := os.MkdirAll(sysPath, 0755); err != nil {
		return fmt.Errorf("mkdir sys: %w", err)
	}
	if err := syscall.Mount("sysfs", sysPath, "sysfs", syscall.MS_RDONLY|syscall.MS_NOSUID|syscall.MS_NOEXEC|syscall.MS_NODEV, ""); err != nil {
		if cfg.Debug {
			fmt.Printf("[init] mount sysfs: %v (ignored)\n", err)
		}
	}

	devPath := filepath.Join(targetRootFS, "dev")
	if err := os.MkdirAll(devPath, 0755); err != nil {
		return fmt.Errorf("mkdir dev: %w", err)
	}
	if err := syscall.Mount("/dev", devPath, "", syscall.MS_BIND|syscall.MS_REC, ""); err != nil {
		if cfg.Debug {
			fmt.Printf("[init] bind-mount /dev: %v (ignored)\n", err)
		}
	}

	if err := network.SetupLoopback(cfg.Debug); err != nil {
		if cfg.Debug {
			fmt.Printf("[init] loopback setup: %v (ignored)\n", err)
		}
	}

	const (
		containerCIDR = "172.20.0.2/24"
		gateway       = "172.20.0.1"
	)
	if err := setupBridgeContainer(cfg.BridgeNetwork, containerCIDR, gateway, cfg.Debug); err != nil {
		return err
	}

	for _, v := range cfg.Volumes {
		if err := mountVolume(v, targetRootFS, cfg.Debug); err != nil {
			return fmt.Errorf("volume %s:%s: %w", v.HostPath, v.ContainerPath, err)
		}
	}

	if err := rootfs.Isolate(targetRootFS, cfg.Debug); err != nil {
		return fmt.Errorf("rootfs isolation: %w", err)
	}

	if err := enforceReadOnlyRoot(cfg.ReadOnly, cfg.Debug); err != nil {
		return err
	}

	if err := enterWorkDir(cfg.WorkDir); err != nil {
		return err
	}

	if len(cfg.CapDrop) > 0 {
		if err := DropCapabilities(cfg.CapDrop, cfg.Debug); err != nil {
			return fmt.Errorf("drop capabilities: %w", err)
		}
	}

	if cfg.Seccomp {
		if err := applySeccomp(cfg.Debug); err != nil {
			return fmt.Errorf("seccomp: %w", err)
		}
	}

	binary, err := exec.LookPath(cfg.Command[0])
	if err != nil {
		binary = cfg.Command[0]
	}

	if cfg.Debug {
		fmt.Printf("[init] exec: %s %v\n", binary, cfg.Command[1:])
	}

	if err := os.Unsetenv(sentinelEnvKey); err != nil {
		return fmt.Errorf("clear runtime init environment: %w", err)
	}
	env := os.Environ()
	if len(cfg.Env) > 0 {
		env = append(env, cfg.Env...)
	}

	if err := initStatus.readyForExec(); err != nil {
		return err
	}
	if err := syscall.Exec(binary, cfg.Command, env); err != nil {
		return fmt.Errorf("exec %s: %w", binary, err)
	}

	return nil
}

func mountVolume(v Volume, rootfs string, debug bool) error {
	if v.HostPath == "" || !filepath.IsAbs(v.HostPath) {
		return fmt.Errorf("host path %q must be absolute", v.HostPath)
	}

	source, sourceFile, err := resolveVolumeMountSource(v.HostPath)
	if err != nil {
		return fmt.Errorf("secure mount source: %w", err)
	}
	if sourceFile != nil {
		defer sourceFile.Close()
	}

	targetFD, err := openVolumeTarget(rootfs, v.ContainerPath)
	if err != nil {
		return fmt.Errorf("secure mount target: %w", err)
	}
	defer syscall.Close(targetFD)
	target := volumeTargetFDPath(targetFD)

	if err := syscall.Mount(source, target, "", syscall.MS_BIND|syscall.MS_REC, ""); err != nil {
		return fmt.Errorf("bind mount: %w", err)
	}

	if v.ReadOnly {
		if err := syscall.Mount("", target, "", syscall.MS_BIND|syscall.MS_REMOUNT|syscall.MS_RDONLY, ""); err != nil {
			return fmt.Errorf("remount read-only: %w", err)
		}
	}

	if debug {
		mode := "rw"
		if v.ReadOnly {
			mode = "ro"
		}
		fmt.Printf("[init] volume: %s → %s (%s)\n", v.HostPath, v.ContainerPath, mode)
	}
	return nil
}
