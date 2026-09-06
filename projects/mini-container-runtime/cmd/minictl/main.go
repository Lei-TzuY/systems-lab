package main

import (
	"encoding/json"
	"flag"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"text/tabwriter"
	"time"

	"minicontainer/internal/attach"
	"minicontainer/internal/bench"
	"minicontainer/internal/builder"
	"minicontainer/internal/compose"
	"minicontainer/internal/container"
	"minicontainer/internal/daemon"
	"minicontainer/internal/diff"
	"minicontainer/internal/events"
	"minicontainer/internal/health"
	"minicontainer/internal/image"
	"minicontainer/internal/imagestore"
	"minicontainer/internal/logs"
	"minicontainer/internal/network"
	"minicontainer/internal/plugin"
	"minicontainer/internal/pty"
	"minicontainer/internal/registry"
	"minicontainer/internal/security"
	"minicontainer/internal/state"
	"minicontainer/internal/system"
	"minicontainer/internal/volume"
)

const defaultHostname = "minicontainer"

const usage = `
minictl — a minimal Linux container runtime (educational)

Usage:
  minictl run     [flags] <rootfs-dir> [command [args...]]  Launch a new container
  minictl build   -t <tag> [-f Dockerfile] <context-dir>       Build image from Dockerfile
  minictl exec    <id> <command> [args...]                    Run command in a container
  minictl top     <id>                                        List processes inside a container
  minictl inspect <id>                                        Inspect container details (JSON)
  minictl export  <id> [output.tar.gz]                        Export container rootfs to a tarball
  minictl commit  <id> <image-name>                           Create a new image from a container
  minictl update  [flags] <id>                                Dynamically update container resource limits
  minictl cp      <src> <dst>                                 Copy files between container and host
  minictl pull    <image> [dest-dir]                          Pull image from Docker Hub
  minictl compose up [-f file.json]                           Orchestrate multi-container app
  minictl events  [-f|--follow] [--json] [--container prefix] [--type type]  Query lifecycle events
  minictl ps      [--all]                                     List containers
  minictl logs    [-f] [--tail n] <id>                        View container logs
  minictl stats   [id]                                        View live resource usage
  minictl pause   <id>                                        Pause a container (cgroup freeze)
  minictl unpause <id>                                        Unpause container
  minictl stop    [-t timeout] <id>                           Gracefully stop container
  minictl kill    <id>                                        Kill a running container
  minictl rm      <id>                                        Remove a stopped container
  minictl prune                                               Remove all stopped containers
  minictl volume  <create|ls|inspect|rm|prune> [args...]      Manage persistent named volumes
  minictl network <create|ls|rm> [args...]                    Manage custom bridge networks
  minictl images                                              List loaded images
  minictl tag     <source-image> <target-tag>                 Tag a local image
  minictl rmi     <image-name-or-id>                          Remove a local image
  minictl load    <docker-save.tar> <dest-dir>                Load a docker-save archive
  minictl unpack  <tar-file>        <dest-dir>                Unpack a plain rootfs tar
  minictl help                                                Show this help

Run flags:
  --overlay               Use OverlayFS Copy-on-Write storage layer.
  --read-only             Mount container rootfs as read-only.
  --restart <policy>      Restart policy: "no", "always", or "on-failure".
  --cap-drop <cap>        Drop Linux capability from bounding set (e.g. CAP_SYS_ADMIN).
  --cpus <number>         Hard fractional CPU quota (e.g. 0.5 = 50% CPU, 2.0 = 2 CPUs).
  --hostname <name>       Hostname visible inside the container.
  -w, --workdir <dir>     Working directory inside the container (e.g. /app).
  -e, --env <key=val>     Environment variable to inject (repeatable).
  -p, --publish <spec>    Port mapping: hostPort:containerPort[/tcp|udp] (requires --bridge).
  --memory <size>         Memory limit, e.g. 67108864, 64m, 1g. 0 disables it.
  --cpu-weight <n>        Cgroup v2 CPU weight, 1..10000. 0 uses the default.
  --pids-limit <n>        Maximum process/thread count. 0 disables it.
  --no-user-ns            Disable user namespace; requires root / sudo.
  --bridge                Enable veth pair networking.
  --seccomp               Install BPF syscall block-list.
  -v, --volume <spec>     Bind mount: host:container[:ro] or volume_name:container[:ro].

Update flags:
  --memory <size>         Update memory limit (e.g. 128m)
  --cpus <number>         Update hard CPU quota (e.g. 1.5)
  --cpu-weight <n>        Update Cgroup v2 CPU weight (1..10000)
  --pids-limit <n>        Update process limit

Examples:
  # Build an image from Dockerfile context:
  minictl build -t my-app:v1 .

  # Dynamically update running container memory to 128MB and CPU quota to 1.5 CPUs:
  minictl update --memory 128m --cpus 1.5 <container-id>
`

func main() {
	if os.Getenv("MINICONTAINER_INIT") == "1" {
		containerInitMain()
		return
	}
	if os.Getenv("MINICONTAINER_EXEC") == "1" {
		containerExecMain()
		return
	}

	if len(os.Args) < 2 {
		fmt.Print(usage)
		os.Exit(1)
	}

	switch os.Args[1] {
	case "run":
		cmdRun(os.Args[2:])
	case "build":
		cmdBuild(os.Args[2:])
	case "exec":
		cmdExec(os.Args[2:])
	case "top":
		cmdTop(os.Args[2:])
	case "inspect":
		cmdInspect(os.Args[2:])
	case "export":
		cmdExport(os.Args[2:])
	case "commit":
		cmdCommit(os.Args[2:])
	case "update":
		cmdUpdate(os.Args[2:])
	case "cp":
		cmdCp(os.Args[2:])
	case "pull":
		cmdPull(os.Args[2:])
	case "compose":
		cmdCompose(os.Args[2:])
	case "events":
		cmdEvents(os.Args[2:])
	case "ps":
		cmdPs(os.Args[2:])
	case "logs":
		cmdLogs(os.Args[2:])
	case "stats":
		cmdStats(os.Args[2:])
	case "pause":
		cmdPause(os.Args[2:])
	case "unpause":
		cmdUnpause(os.Args[2:])
	case "stop":
		cmdStop(os.Args[2:])
	case "kill":
		cmdKill(os.Args[2:])
	case "rm":
		cmdRm(os.Args[2:])
	case "prune":
		cmdPrune()
	case "volume":
		cmdVolume(os.Args[2:])
	case "daemon":
		cmdDaemon(os.Args[2:])
	case "network":
		cmdNetwork(os.Args[2:])
	case "images":
		cmdImages()
	case "tag":
		cmdTag(os.Args[2:])
	case "rmi":
		cmdRmi(os.Args[2:])
	case "load":
		cmdLoad(os.Args[2:])
	case "unpack":
		cmdUnpack(os.Args[2:])
	case "diff":
		cmdDiff(os.Args[2:])
	case "push":
		cmdPush(os.Args[2:])
	case "attach":
		cmdAttach(os.Args[2:])
	case "system":
		cmdSystem(os.Args[2:])
	case "history":
		cmdHistory(os.Args[2:])
	case "version":
		cmdVersion()
	case "check":
		cmdCheck()
	case "dump":
		cmdDump(os.Args[2:])
	case "plugin":
		cmdPlugin(os.Args[2:])
	case "scan":
		cmdScan(os.Args[2:])
	case "bench":
		cmdBench(os.Args[2:])
	case "snapshot":
		cmdSnapshot(os.Args[2:])
	case "info":
		cmdInfo(os.Args[2:])
	case "rename":
		cmdRename(os.Args[2:])
	case "import":
		cmdImport(os.Args[2:])
	case "wait":
		cmdWait(os.Args[2:])
	case "help", "--help", "-h":
		fmt.Print(usage)
	default:
		fmt.Fprintf(os.Stderr, "unknown command: %s\n", os.Args[1])
		fmt.Print(usage)
		os.Exit(1)
	}
}

func containerInitMain() {
	cfg, err := parseRunConfig(os.Args[2:])
	if err != nil {
		fmt.Fprintf(os.Stderr, "internal error: %v\n", err)
		os.Exit(1)
	}

	if err := container.ContainerInit(cfg); err != nil {
		fmt.Fprintf(os.Stderr, "container init: %v\n", err)
		os.Exit(1)
	}
}

func containerExecMain() {
	args := os.Args[2:]
	if len(args) < 3 {
		fmt.Fprintln(os.Stderr, "internal exec usage: exec <pid> <rootfs> <command...>")
		os.Exit(1)
	}
	pid, err := strconv.Atoi(args[0])
	if err != nil {
		fmt.Fprintf(os.Stderr, "internal exec: invalid pid %q: %v\n", args[0], err)
		os.Exit(1)
	}
	debug := os.Getenv("MINICONTAINER_DEBUG") == "1"
	if err := container.ExecInit(pid, args[1], args[2:], debug); err != nil {
		fmt.Fprintf(os.Stderr, "exec: %v\n", err)
		os.Exit(1)
	}
}

func cmdRun(args []string) {
	cfg, err := parseRunConfig(args)
	if err != nil {
		fmt.Fprintf(os.Stderr, "error: %v\n", err)
		fmt.Fprintln(os.Stderr, "Usage: minictl run [flags] <rootfs-dir> [command [args...]]")
		os.Exit(1)
	}

	if _, err := os.Stat(filepath.Join(cfg.RootFS, "bin")); os.IsNotExist(err) {
		fmt.Fprintf(os.Stderr,
			"rootfs %q does not look like a valid rootfs (missing bin/)\n"+
				"Tip: run  minictl unpack <alpine.tar.gz> %s  or  minictl pull alpine %s  first\n",
			cfg.RootFS, cfg.RootFS, cfg.RootFS)
		os.Exit(1)
	}

	store, rec, err := prepareManagedRunState(&cfg)
	if err != nil {
		fmt.Fprintf(os.Stderr, "error: prepare container state: %v\n", err)
		os.Exit(1)
	}
	containerID := rec.ID
	fmt.Printf("Container ID: %s\n", containerID[:8])

	if containerID != "" {
		_ = events.Publish(events.EventCreate, containerID, cfg.RootFS, "created container")
		logFile, err := logs.CreateLogFile(containerID)
		if err == nil {
			defer logFile.Close()
		}

		_ = pty.NewSession(os.Stdin, os.Stdout, os.Stderr, true)
		if store != nil {
			sup := health.NewSupervisor(containerID, health.Config{}, nil, store)
			_ = sup
		}
	}

	runErr := container.Run(cfg)

	if store != nil && rec != nil {
		_, settleErr := settleRunCommandState(store, containerID, runErr, time.Now())
		runErr = joinRunCommandErrors(runErr, settleErr)
	}

	if runErr != nil {
		fmt.Fprintf(os.Stderr, "error: %v\n", runErr)
		os.Exit(runCommandExitCode(runErr))
	}
}

func cmdExec(args []string) {
	if len(args) < 2 {
		fmt.Fprintln(os.Stderr, "Usage: minictl exec <id> <command> [args...]")
		os.Exit(1)
	}

	idOrPrefix := args[0]
	command := args[1:]
	debug := os.Getenv("MINICONTAINER_DEBUG") == "1"

	store, err := openStore()
	if err != nil {
		fmt.Fprintf(os.Stderr, "error: state store: %v\n", err)
		os.Exit(1)
	}

	rec, err := store.Resolve(idOrPrefix)
	if err != nil {
		fmt.Fprintf(os.Stderr, "error: %v\n", err)
		os.Exit(1)
	}
	rec, err = container.ReconcileContainerState(store, rec)
	if err != nil {
		fmt.Fprintf(os.Stderr, "error: reconcile container: %v\n", err)
		os.Exit(1)
	}

	if rec.Status != state.StatusRunning {
		fmt.Fprintf(os.Stderr, "error: container %s is %s (must be running)\n",
			rec.ID[:8], rec.Status)
		os.Exit(1)
	}

	_ = events.Publish(events.EventExec, rec.ID, rec.RootFS, fmt.Sprintf("exec %v", command))

	if err := container.Exec(container.ExecConfig{
		ContainerPID: rec.PID,
		RootFS:       rec.RootFS,
		Command:      command,
		Debug:        debug,
	}); err != nil {
		fmt.Fprintf(os.Stderr, "exec error: %v\n", err)
		os.Exit(1)
	}
}

func cmdUpdate(args []string) {
	cmdUpdateSafe(args)
}

func cmdPull(args []string) {
	if len(args) < 1 {
		fmt.Fprintln(os.Stderr, "Usage: minictl pull <image> [dest-dir]")
		os.Exit(1)
	}

	imageRef := args[0]
	destDir := "./rootfs"
	if len(args) >= 2 {
		destDir = args[1]
	}

	if err := registry.PullImage(imageRef, destDir); err != nil {
		fmt.Fprintf(os.Stderr, "pull failed: %v\n", err)
		os.Exit(1)
	}

	store, _ := openStore()
	if store != nil {
		_ = store.SaveImage(&state.Image{
			Name:     imageRef,
			RootFS:   destDir,
			LoadedAt: time.Now(),
		})
	}
}

func cmdCp(args []string) {
	if len(args) != 2 {
		fmt.Fprintln(os.Stderr, "Usage: minictl cp <src> <dst>")
		os.Exit(1)
	}

	store, err := openStore()
	if err != nil {
		fmt.Fprintf(os.Stderr, "error: %v\n", err)
		os.Exit(1)
	}

	srcID, srcPath := image.ParseCopyTarget(args[0])
	dstID, dstPath := image.ParseCopyTarget(args[1])

	var realSrc, realDst string

	if srcID != "" {
		rec, err := store.Resolve(srcID)
		if err != nil {
			fmt.Fprintf(os.Stderr, "error src: %v\n", err)
			os.Exit(1)
		}
		realSrc = filepath.Join(rec.RootFS, strings.TrimPrefix(srcPath, "/"))
	} else {
		realSrc = srcPath
	}

	if dstID != "" {
		rec, err := store.Resolve(dstID)
		if err != nil {
			fmt.Fprintf(os.Stderr, "error dst: %v\n", err)
			os.Exit(1)
		}
		realDst = filepath.Join(rec.RootFS, strings.TrimPrefix(dstPath, "/"))
	} else {
		realDst = dstPath
	}

	if err := image.CopyPath(realSrc, realDst); err != nil {
		fmt.Fprintf(os.Stderr, "cp error: %v\n", err)
		os.Exit(1)
	}
	fmt.Printf("Copied %s → %s\n", args[0], args[1])
}

func cmdCompose(args []string) {
	if len(args) < 1 || args[0] != "up" {
		fmt.Fprintln(os.Stderr, "Usage: minictl compose up [-f compose.json]")
		os.Exit(1)
	}

	fs := flag.NewFlagSet("compose up", flag.ExitOnError)
	file := fs.String("f", "compose.json", "path to compose JSON file")
	_ = fs.Parse(args[1:])

	cfg, err := compose.ParseConfigFile(*file)
	if err != nil {
		fmt.Fprintf(os.Stderr, "compose error: %v\n", err)
		os.Exit(1)
	}

	fmt.Printf("Orchestrating %d service(s) from %s …\n", len(cfg.Services), *file)
	for name, service := range cfg.Services {
		fmt.Printf("Starting service %q (%s) …\n", name, service.Image)
		cCfg := service.BuildContainerConfig(name)
		if err := container.Run(cCfg); err != nil {
			fmt.Fprintf(os.Stderr, "service %q failed: %v\n", name, err)
		}
	}
}

func cmdEvents(args []string) {
	opts, err := parseEventsCLIOptions(args, os.Stderr)
	if err != nil {
		fmt.Fprintf(os.Stderr, "events error: %v\n", err)
		os.Exit(1)
	}

	if err := events.StreamEventsWithOptions(opts, os.Stdout); err != nil {
		fmt.Fprintf(os.Stderr, "events error: %v\n", err)
		os.Exit(1)
	}
}

func cmdTop(args []string) {
	if len(args) < 1 {
		fmt.Fprintln(os.Stderr, "Usage: minictl top <id>")
		os.Exit(1)
	}

	store, err := openStore()
	if err != nil {
		fmt.Fprintf(os.Stderr, "error: %v\n", err)
		os.Exit(1)
	}

	rec, err := store.Resolve(args[0])
	if err != nil {
		fmt.Fprintf(os.Stderr, "error: %v\n", err)
		os.Exit(1)
	}

	if rec.Status != state.StatusRunning {
		fmt.Fprintf(os.Stderr, "container %s is %s (must be running)\n", rec.ID[:8], rec.Status)
		os.Exit(1)
	}

	procs, err := container.GetContainerProcesses(rec.PID)
	if err != nil {
		fmt.Fprintf(os.Stderr, "top error: %v\n", err)
		os.Exit(1)
	}

	w := tabwriter.NewWriter(os.Stdout, 0, 0, 2, ' ', 0)
	fmt.Fprintln(w, "PID\tPPID\tSTATE\tNAME")
	for _, p := range procs {
		fmt.Fprintf(w, "%d\t%d\t%s\t%s\n", p.PID, p.PPID, p.State, p.Name)
	}
	_ = w.Flush()
}

func cmdExport(args []string) {
	if len(args) < 1 {
		fmt.Fprintln(os.Stderr, "Usage: minictl export <id> [output.tar.gz]")
		os.Exit(1)
	}

	idOrPrefix := args[0]
	outPath := "container-export.tar.gz"
	if len(args) >= 2 {
		outPath = args[1]
	}

	store, err := openStore()
	if err != nil {
		fmt.Fprintf(os.Stderr, "error: %v\n", err)
		os.Exit(1)
	}

	rec, err := store.Resolve(idOrPrefix)
	if err != nil {
		fmt.Fprintf(os.Stderr, "error: %v\n", err)
		os.Exit(1)
	}

	fmt.Printf("Exporting container %s rootfs (%s) → %s …\n", rec.ID[:8], rec.RootFS, outPath)
	if err := image.ExportDir(rec.RootFS, outPath); err != nil {
		fmt.Fprintf(os.Stderr, "export error: %v\n", err)
		os.Exit(1)
	}
	fmt.Println("Export complete.")
}

func cmdCommit(args []string) {
	if len(args) < 2 {
		fmt.Fprintln(os.Stderr, "Usage: minictl commit <id> <image-name>")
		os.Exit(1)
	}

	idOrPrefix := args[0]
	imageName := args[1]

	store, err := openStore()
	if err != nil {
		fmt.Fprintf(os.Stderr, "error: %v\n", err)
		os.Exit(1)
	}

	img, err := commitContainerImage(store, idOrPrefix, imageName)
	if err != nil {
		fmt.Fprintf(os.Stderr, "commit error: %v\n", err)
		os.Exit(1)
	}
	shortImageID := img.ID
	if len(shortImageID) > 12 {
		shortImageID = shortImageID[:12]
	}
	fmt.Printf("Committed container %s as image %s (%s)\n", idOrPrefix[:min(8, len(idOrPrefix))], imageName, shortImageID)
}

func cmdInspect(args []string) {
	if len(args) < 1 {
		fmt.Fprintln(os.Stderr, "Usage: minictl inspect <id>")
		os.Exit(1)
	}

	store, err := openStore()
	if err != nil {
		fmt.Fprintf(os.Stderr, "error: %v\n", err)
		os.Exit(1)
	}

	rec, err := store.Resolve(args[0])
	if err != nil {
		fmt.Fprintf(os.Stderr, "error: %v\n", err)
		os.Exit(1)
	}

	if len(rec.Env) > 0 {
		rec.Env = container.MaskEnvVars(rec.Env)
	}

	raw, err := json.MarshalIndent(rec, "", "  ")
	if err != nil {
		fmt.Fprintf(os.Stderr, "json error: %v\n", err)
		os.Exit(1)
	}
	fmt.Println(string(raw))
}

func cmdLogs(args []string) {
	fs := flag.NewFlagSet("logs", flag.ExitOnError)
	follow := fs.Bool("f", false, "follow log output")
	_ = fs.Bool("follow", false, "follow log output")
	tail := fs.Int("tail", 0, "lines to show from end of logs")
	fs.SetOutput(os.Stderr)
	_ = fs.Parse(args)

	rest := fs.Args()
	if len(rest) < 1 {
		fmt.Fprintln(os.Stderr, "Usage: minictl logs [-f] [--tail n] <id>")
		os.Exit(1)
	}

	store, err := openStore()
	if err != nil {
		fmt.Fprintf(os.Stderr, "error: %v\n", err)
		os.Exit(1)
	}

	rec, err := store.Resolve(rest[0])
	if err != nil {
		fmt.Fprintf(os.Stderr, "error: %v\n", err)
		os.Exit(1)
	}

	if err := logs.PrintLogs(rec.ID, *tail, *follow, os.Stdout); err != nil {
		fmt.Fprintf(os.Stderr, "logs error: %v\n", err)
		os.Exit(1)
	}
}

func cmdStats(args []string) {
	cmdStatsSafe(args)
}

func cmdPause(args []string) {
	cmdPauseSafe(args)
}

func cmdUnpause(args []string) {
	cmdUnpauseSafe(args)
}

func cmdStop(args []string) {
	cmdStopSafe(args)
}

func cmdPs(args []string) {
	fs := flag.NewFlagSet("ps", flag.ExitOnError)
	all := fs.Bool("all", false, "show all containers (default: only running)")
	_ = fs.Bool("a", false, "alias for --all")
	fs.SetOutput(os.Stderr)
	_ = fs.Parse(args)

	showAll := *all
	fs.Visit(func(f *flag.Flag) {
		if f.Name == "a" {
			showAll = true
		}
	})

	store, err := openStore()
	if err != nil {
		fmt.Fprintf(os.Stderr, "error: %v\n", err)
		os.Exit(1)
	}

	containers, err := store.List()
	if err != nil {
		fmt.Fprintf(os.Stderr, "error listing containers: %v\n", err)
		os.Exit(1)
	}

	for i, c := range containers {
		if c.Status != state.StatusRunning {
			continue
		}
		reconciled, reconcileErr := container.ReconcileContainerState(store, c)
		if reconciled != nil {
			containers[i] = reconciled
		}
		if reconcileErr != nil {
			shortID := c.ID
			if len(shortID) > 8 {
				shortID = shortID[:8]
			}
			fmt.Fprintf(os.Stderr, "warning: reconcile container %s: %v\n", shortID, reconcileErr)
		}
	}

	w := tabwriter.NewWriter(os.Stdout, 0, 0, 2, ' ', 0)
	fmt.Fprintln(w, "CONTAINER ID\tSTATUS\tCOMMAND\tHOSTNAME\tCREATED")
	for _, c := range containers {
		if !showAll && c.Status != state.StatusRunning {
			continue
		}
		shortID := c.ID
		if len(shortID) > 12 {
			shortID = shortID[:12]
		}
		statusStr := string(c.Status)
		if c.Health != "" {
			statusStr += " (" + c.Health + ")"
		}
		cmd := strings.Join(c.Command, " ")
		if len(cmd) > 30 {
			cmd = cmd[:27] + "..."
		}
		age := time.Since(c.CreatedAt).Round(time.Second)
		fmt.Fprintf(w, "%s\t%s\t%s\t%s\t%s ago\n", shortID, statusStr, cmd, c.Hostname, age)
	}
	_ = w.Flush()
}

func cmdKill(args []string) {
	cmdKillSafe(args)
}

func cmdRm(args []string) {
	if len(args) < 1 {
		fmt.Fprintln(os.Stderr, "Usage: minictl rm <id>")
		os.Exit(1)
	}

	store, err := openStore()
	if err != nil {
		fmt.Fprintf(os.Stderr, "error: %v\n", err)
		os.Exit(1)
	}

	rec, err := store.Resolve(args[0])
	if err != nil {
		fmt.Fprintf(os.Stderr, "error: %v\n", err)
		os.Exit(1)
	}
	rec, err = container.ReconcileContainerState(store, rec)
	if err != nil {
		fmt.Fprintf(os.Stderr, "rm %s: reconcile: %v\n", rec.ID[:8], err)
		os.Exit(1)
	}

	if rec.Status == state.StatusRunning {
		fmt.Fprintf(os.Stderr, "container %s is running — stop it first with 'minictl kill %s'\n", rec.ID[:8], rec.ID[:8])
		os.Exit(1)
	}

	if err := store.DeleteIfNotRunning(rec.ID); err != nil {
		fmt.Fprintf(os.Stderr, "rm %s: %v\n", rec.ID[:8], err)
		os.Exit(1)
	}
	_ = events.Publish(events.EventRemove, rec.ID, rec.RootFS, "removed container")
	fmt.Printf("%s\n", rec.ID[:8])
}

func cmdPrune() {
	store, err := openStore()
	if err != nil {
		fmt.Fprintf(os.Stderr, "error: %v\n", err)
		os.Exit(1)
	}

	all, err := store.List()
	if err != nil {
		fmt.Fprintf(os.Stderr, "error listing containers: %v\n", err)
		os.Exit(1)
	}

	var removed int
	for _, c := range all {
		current, reconcileErr := container.ReconcileContainerState(store, c)
		if reconcileErr != nil {
			shortID := c.ID
			if len(shortID) > 8 {
				shortID = shortID[:8]
			}
			fmt.Fprintf(os.Stderr, "warning: prune %s: reconcile: %v\n", shortID, reconcileErr)
			continue
		}
		if current.Status == state.StatusRunning {
			continue
		}
		if err := store.DeleteIfNotRunning(current.ID); err != nil {
			shortID := current.ID
			if len(shortID) > 8 {
				shortID = shortID[:8]
			}
			fmt.Fprintf(os.Stderr, "warning: prune %s: %v\n", shortID, err)
			continue
		}
		removed++
	}
	fmt.Printf("Pruned %d stopped container(s)\n", removed)
}

func cmdNetwork(args []string) {
	if len(args) < 1 {
		fmt.Fprintln(os.Stderr, "Usage: minictl network <create|ls|rm> [args...]")
		os.Exit(1)
	}

	debug := os.Getenv("MINICONTAINER_DEBUG") == "1"

	switch args[0] {
	case "create":
		if len(args) < 2 {
			fmt.Fprintln(os.Stderr, "Usage: minictl network create <name> [cidr]")
			os.Exit(1)
		}
		name := args[1]
		cidr := "172.28.0.1/24"
		if len(args) >= 3 {
			cidr = args[2]
		}
		if err := network.CreateBridge(name, cidr, debug); err != nil {
			fmt.Fprintf(os.Stderr, "network create error: %v\n", err)
			os.Exit(1)
		}
		fmt.Printf("Created bridge network %q (%s)\n", name, cidr)

	case "ls", "list":
		nets, err := network.ListBridges()
		if err != nil {
			fmt.Fprintf(os.Stderr, "network ls error: %v\n", err)
			os.Exit(1)
		}
		w := tabwriter.NewWriter(os.Stdout, 0, 0, 2, ' ', 0)
		fmt.Fprintln(w, "NETWORK NAME\tBRIDGE IFACE\tSTATUS")
		for _, n := range nets {
			fmt.Fprintf(w, "%s\t%s\t%s\n", n.Name, n.Bridge, n.Status)
		}
		_ = w.Flush()

	case "rm", "delete":
		if len(args) < 2 {
			fmt.Fprintln(os.Stderr, "Usage: minictl network rm <name>")
			os.Exit(1)
		}
		name := args[1]
		if err := network.DeleteBridge(name, debug); err != nil {
			fmt.Fprintf(os.Stderr, "network rm error: %v\n", err)
			os.Exit(1)
		}
		fmt.Printf("Deleted bridge network %q\n", name)

	default:
		fmt.Fprintf(os.Stderr, "unknown network subcommand: %s\n", args[0])
		os.Exit(1)
	}
}

func cmdBuild(args []string) {
	fs := flag.NewFlagSet("build", flag.ExitOnError)
	tag := fs.String("t", "latest", "Name and optionally a tag in the 'name:tag' format")
	dockerfile := fs.String("f", "", "Name of the Dockerfile (Default is 'PATH/Dockerfile')")
	_ = fs.Parse(args)

	rest := fs.Args()
	if len(rest) < 1 {
		fmt.Fprintln(os.Stderr, "Usage: minictl build -t <tag> [-f Dockerfile] <context-dir>")
		os.Exit(1)
	}

	contextDir := rest[0]
	st, _ := openStore()

	res, err := builder.BuildDockerfile(builder.BuildOptions{
		ContextDir: contextDir,
		Dockerfile: *dockerfile,
		Tag:        *tag,
		Store:      st,
	})

	if err != nil {
		fmt.Fprintf(os.Stderr, "build failed: %v\n", err)
		os.Exit(1)
	}

	fmt.Printf("Successfully built %s (Image ID: %s)\n", res.Image.Name, res.Image.ID)
}

func cmdTag(args []string) {
	if len(args) != 2 {
		fmt.Fprintln(os.Stderr, "Usage: minictl tag <source-image> <target-tag>")
		os.Exit(1)
	}

	st, err := openStore()
	if err != nil {
		fmt.Fprintf(os.Stderr, "store error: %v\n", err)
		os.Exit(1)
	}

	tagged, err := imagestore.TagImage(st, args[0], args[1])
	if err != nil {
		fmt.Fprintf(os.Stderr, "tag failed: %v\n", err)
		os.Exit(1)
	}

	fmt.Printf("Tagged %s -> %s\n", args[0], tagged.Name)
}

func cmdRmi(args []string) {
	if len(args) < 1 {
		fmt.Fprintln(os.Stderr, "Usage: minictl rmi <image-name-or-id>")
		os.Exit(1)
	}

	st, err := openStore()
	if err != nil {
		fmt.Fprintf(os.Stderr, "store error: %v\n", err)
		os.Exit(1)
	}

	for _, name := range args {
		removed, err := imagestore.RemoveImage(st, name, true)
		if err != nil {
			fmt.Fprintf(os.Stderr, "failed to remove image %s: %v\n", name, err)
		} else {
			fmt.Printf("Untagged/Removed: %s\n", removed.Name)
		}
	}
}

func cmdVolume(args []string) {
	if len(args) < 1 {
		fmt.Fprintln(os.Stderr, "Usage: minictl volume <create|ls|inspect|rm|prune> [args...]")
		os.Exit(1)
	}

	switch args[0] {
	case "create":
		if len(args) < 2 {
			fmt.Fprintln(os.Stderr, "Usage: minictl volume create <volume-name>")
			os.Exit(1)
		}
		vol, err := volume.CreateVolume(args[1])
		if err != nil {
			fmt.Fprintf(os.Stderr, "create volume failed: %v\n", err)
			os.Exit(1)
		}
		fmt.Printf("%s\n", vol.Name)

	case "ls", "list":
		vols, err := volume.ListVolumes()
		if err != nil {
			fmt.Fprintf(os.Stderr, "list volumes failed: %v\n", err)
			os.Exit(1)
		}
		w := tabwriter.NewWriter(os.Stdout, 0, 0, 2, ' ', 0)
		fmt.Fprintln(w, "VOLUME NAME\tMOUNT PATH\tSIZE")
		for _, v := range vols {
			fmt.Fprintf(w, "%s\t%s\t%d B\n", v.Name, v.MountPath, v.Size)
		}
		_ = w.Flush()

	case "inspect":
		if len(args) < 2 {
			fmt.Fprintln(os.Stderr, "Usage: minictl volume inspect <volume-name>")
			os.Exit(1)
		}
		vol, err := volume.GetVolume(args[1])
		if err != nil {
			fmt.Fprintf(os.Stderr, "inspect volume failed: %v\n", err)
			os.Exit(1)
		}
		data, _ := json.MarshalIndent(vol, "", "  ")
		fmt.Println(string(data))

	case "rm", "remove":
		if len(args) < 2 {
			fmt.Fprintln(os.Stderr, "Usage: minictl volume rm <volume-name>")
			os.Exit(1)
		}
		if err := volume.RemoveVolume(args[1]); err != nil {
			fmt.Fprintf(os.Stderr, "remove volume failed: %v\n", err)
			os.Exit(1)
		}
		fmt.Printf("%s\n", args[1])

	case "prune":
		n, err := volume.PruneVolumes()
		if err != nil {
			fmt.Fprintf(os.Stderr, "prune volumes failed: %v\n", err)
			os.Exit(1)
		}
		fmt.Printf("Total reclaimed volumes: %d\n", n)

	default:
		fmt.Fprintf(os.Stderr, "unknown volume subcommand: %s\n", args[0])
		os.Exit(1)
	}
}

func cmdDaemon(args []string) {
	fs := flag.NewFlagSet("daemon", flag.ExitOnError)
	listen := fs.String("listen", "unix:///tmp/minictl.sock", "Daemon listen address (unix:///path/to/socket or tcp://host:port)")
	_ = fs.Parse(args)

	fmt.Printf("Starting minictl REST API Daemon on %s...\n", *listen)
	srv, err := daemon.NewServer(daemon.Config{ListenAddr: *listen})
	if err != nil {
		fmt.Fprintf(os.Stderr, "failed to initialize daemon server: %v\n", err)
		os.Exit(1)
	}

	if err := srv.Start(); err != nil {
		fmt.Fprintf(os.Stderr, "daemon server exited with error: %v\n", err)
		os.Exit(1)
	}
}

func cmdDiff(args []string) {
	if len(args) < 1 {
		fmt.Fprintln(os.Stderr, "Usage: minictl diff <container-id>")
		os.Exit(1)
	}
	st, err := openStore()
	if err != nil {
		fmt.Fprintf(os.Stderr, "store error: %v\n", err)
		os.Exit(1)
	}
	c, err := st.Resolve(args[0])
	if err != nil {
		fmt.Fprintf(os.Stderr, "resolve container failed: %v\n", err)
		os.Exit(1)
	}

	upperDir := filepath.Join(c.RootFS, "upper")
	if _, err := os.Stat(upperDir); err == nil {
		changes, err := diff.DiffUpper(upperDir)
		if err != nil {
			fmt.Fprintf(os.Stderr, "diff upper error: %v\n", err)
			os.Exit(1)
		}
		fmt.Print(diff.FormatDiff(changes))
	} else {
		fmt.Printf("Container %s does not use OverlayFS upperdir.\n", c.ID)
	}
}

func cmdPush(args []string) {
	if len(args) < 1 {
		fmt.Fprintln(os.Stderr, "Usage: minictl push <image-tag> [output.tar.gz]")
		os.Exit(1)
	}
	st, err := openStore()
	if err != nil {
		fmt.Fprintf(os.Stderr, "store error: %v\n", err)
		os.Exit(1)
	}
	tag := args[0]
	outArchive := "image-layer.tar.gz"
	if len(args) >= 2 {
		outArchive = args[1]
	}

	fmt.Printf("Packaging & Pushing OCI image %s -> %s...\n", tag, outArchive)
	if err := registry.PushImage(st, tag, outArchive); err != nil {
		fmt.Fprintf(os.Stderr, "push failed: %v\n", err)
		os.Exit(1)
	}
	fmt.Printf("Successfully exported OCI layer archive and manifest (%s.manifest.json)\n", outArchive)
}

func cmdAttach(args []string) {
	if len(args) < 1 {
		fmt.Fprintln(os.Stderr, "Usage: minictl attach <container-id>")
		os.Exit(1)
	}
	st, err := openStore()
	if err != nil {
		fmt.Fprintf(os.Stderr, "store error: %v\n", err)
		os.Exit(1)
	}
	if err := attach.AttachContainer(st, args[0], os.Stdin, os.Stdout); err != nil {
		fmt.Fprintf(os.Stderr, "attach failed: %v\n", err)
		os.Exit(1)
	}
}

func cmdSystem(args []string) {
	if len(args) < 1 {
		fmt.Fprintln(os.Stderr, "Usage: minictl system <df|prune>")
		os.Exit(1)
	}
	st, err := openStore()
	if err != nil {
		fmt.Fprintf(os.Stderr, "store error: %v\n", err)
		os.Exit(1)
	}

	switch args[0] {
	case "df":
		df, err := system.CalculateDF(st)
		if err != nil {
			fmt.Fprintf(os.Stderr, "system df error: %v\n", err)
			os.Exit(1)
		}
		w := tabwriter.NewWriter(os.Stdout, 0, 0, 2, ' ', 0)
		fmt.Fprintln(w, "TYPE\tTOTAL\tSIZE")
		fmt.Fprintf(w, "Containers\t%d\t%d B\n", df.ContainersCount, df.ContainersSize)
		fmt.Fprintf(w, "Images\t%d\t%d B\n", df.ImagesCount, df.ImagesSize)
		fmt.Fprintf(w, "Local Volumes\t%d\t%d B\n", df.VolumesCount, df.VolumesSize)
		_ = w.Flush()

	case "prune":
		pruneAll := false
		untilStr := ""
		for i := 1; i < len(args); i++ {
			if args[i] == "-a" || args[i] == "--all" {
				pruneAll = true
			} else if strings.HasPrefix(args[i], "--until=") {
				untilStr = strings.TrimPrefix(args[i], "--until=")
			} else if args[i] == "--until" && i+1 < len(args) {
				untilStr = args[i+1]
			}
		}

		if untilStr != "" {
			dur, err := system.ParseUntilDuration(untilStr)
			if err != nil {
				fmt.Fprintf(os.Stderr, "invalid --until duration: %v\n", err)
				os.Exit(1)
			}
			cutoff := time.Now().Add(-dur)
			res, err := system.PruneUntil(st, cutoff)
			if err != nil {
				fmt.Fprintf(os.Stderr, "system prune --until error: %v\n", err)
				os.Exit(1)
			}
			fmt.Printf("Deleted Containers (older than %s): %d\n", untilStr, res.ContainersReclaimed)
			return
		}

		res, err := system.SystemPrune(st, pruneAll)
		if err != nil {
			fmt.Fprintf(os.Stderr, "system prune error: %v\n", err)
			os.Exit(1)
		}
		fmt.Printf("Deleted Containers: %d\n", res.ContainersReclaimed)
		fmt.Printf("Deleted Images: %d\n", res.ImagesReclaimed)
		fmt.Printf("Deleted Volumes: %d\n", res.VolumesReclaimed)

	default:
		fmt.Fprintf(os.Stderr, "unknown system subcommand: %s\n", args[0])
		os.Exit(1)
	}
}

func cmdHistory(args []string) {
	if len(args) < 1 {
		fmt.Fprintln(os.Stderr, "Usage: minictl history <image-name-or-id>")
		os.Exit(1)
	}
	st, err := openStore()
	if err != nil {
		fmt.Fprintf(os.Stderr, "store error: %v\n", err)
		os.Exit(1)
	}
	layers, err := imagestore.GetImageHistory(st, args[0])
	if err != nil {
		fmt.Fprintf(os.Stderr, "history failed: %v\n", err)
		os.Exit(1)
	}

	w := tabwriter.NewWriter(os.Stdout, 0, 0, 2, ' ', 0)
	fmt.Fprintln(w, "CREATED BY\tSIZE\tCREATED")
	for _, l := range layers {
		age := time.Since(l.CreatedAt).Round(time.Second)
		fmt.Fprintf(w, "%s\t%d B\t%s ago\n", l.CreatedBy, l.Size, age)
	}
	_ = w.Flush()
}

func cmdVersion() {
	res := system.CheckKernelFeatures()
	fmt.Printf("minictl Engine Version: v1.6.0\n")
	fmt.Printf("Go Version:           %s\n", res.GoVersion)
	fmt.Printf("OS/Arch:              %s/%s\n", res.OS, res.Arch)
}

func cmdCheck() {
	res := system.CheckKernelFeatures()
	w := tabwriter.NewWriter(os.Stdout, 0, 0, 2, ' ', 0)
	fmt.Fprintln(w, "FEATURE\tSUPPORTED")
	fmt.Fprintf(w, "Linux Namespaces\t%v\n", res.NamespacesSupported)
	fmt.Fprintf(w, "Cgroups v2\t%v\n", res.CgroupsV2Supported)
	fmt.Fprintf(w, "OverlayFS\t%v\n", res.OverlayFSSupported)
	fmt.Fprintf(w, "Seccomp BPF\t%v\n", res.SeccompSupported)
	fmt.Fprintf(w, "PivotRoot\t%v\n", res.PivotRootSupported)
	_ = w.Flush()
}

func cmdDump(args []string) {
	if len(args) < 1 {
		fmt.Fprintln(os.Stderr, "Usage: minictl dump <container-id> [out.dump]")
		os.Exit(1)
	}
	outPath := ""
	if len(args) >= 2 {
		outPath = args[1]
	}
	st, err := openStore()
	if err != nil {
		fmt.Fprintf(os.Stderr, "store error: %v\n", err)
		os.Exit(1)
	}
	info, err := container.DumpContainerMemory(st, args[0], outPath)
	if err != nil {
		fmt.Fprintf(os.Stderr, "dump error: %v\n", err)
		os.Exit(1)
	}
	if outPath != "" {
		fmt.Printf("Dumped container %s state -> %s\n", info.ContainerID[:8], outPath)
	} else {
		raw, _ := json.MarshalIndent(info, "", "  ")
		fmt.Println(string(raw))
	}
}

func cmdPlugin(args []string) {
	plugins, err := plugin.ListPlugins()
	if err != nil {
		fmt.Fprintf(os.Stderr, "plugin error: %v\n", err)
		os.Exit(1)
	}
	w := tabwriter.NewWriter(os.Stdout, 0, 0, 2, ' ', 0)
	fmt.Fprintln(w, "NAME\tTYPE\tVERSION\tENABLED")
	for _, p := range plugins {
		fmt.Fprintf(w, "%s\t%s\t%s\t%v\n", p.Name, p.Type, p.Version, p.Enabled)
	}
	_ = w.Flush()
}

func cmdScan(args []string) {
	if len(args) < 1 {
		fmt.Fprintln(os.Stderr, "Usage: minictl scan <rootfs-path-or-id>")
		os.Exit(1)
	}
	target := args[0]
	st, err := openStore()
	if err == nil {
		rec, resolveErr := st.Resolve(target)
		if resolveErr == nil && rec.RootFS != "" {
			target = rec.RootFS
		}
	}

	report, err := security.ScanRootFS(target)
	if err != nil {
		fmt.Fprintf(os.Stderr, "scan error: %v\n", err)
		os.Exit(1)
	}

	raw, _ := json.MarshalIndent(report, "", "  ")
	fmt.Println(string(raw))
}

func cmdBench(args []string) {
	st, err := openStore()
	if err != nil {
		fmt.Fprintf(os.Stderr, "store error: %v\n", err)
		os.Exit(1)
	}

	fmt.Println("Running minictl engine benchmark (50 iterations)...")
	res, err := bench.RunBenchmark(st, 50)
	if err != nil {
		fmt.Fprintf(os.Stderr, "benchmark error: %v\n", err)
		os.Exit(1)
	}

	fmt.Printf("State Write Latency:   %.4f ms/op\n", res.StateWriteMs)
	fmt.Printf("State Read Latency:    %.4f ms/op\n", res.StateReadMs)
	fmt.Printf("Init Startup Overhead: %.4f ms/op\n", res.StartupLatencyMs)
}

func cmdSnapshot(args []string) {
	if len(args) < 2 {
		fmt.Fprintln(os.Stderr, "Usage: minictl snapshot <create|restore> <container-id> [snapshot-name]")
		os.Exit(1)
	}
	subCmd := args[0]
	containerID := args[1]
	snapName := "snap-default"
	if len(args) >= 3 {
		snapName = args[2]
	}

	st, err := openStore()
	if err != nil {
		fmt.Fprintf(os.Stderr, "store error: %v\n", err)
		os.Exit(1)
	}

	switch subCmd {
	case "create":
		snap, err := container.CreateSnapshot(st, containerID, snapName)
		if err != nil {
			fmt.Fprintf(os.Stderr, "snapshot create failed: %v\n", err)
			os.Exit(1)
		}
		fmt.Printf("Created snapshot %q for container %s -> %s\n", snap.Name, snap.ContainerID[:8], snap.Path)
	case "restore":
		if err := container.RestoreSnapshot(st, containerID, snapName); err != nil {
			fmt.Fprintf(os.Stderr, "snapshot restore failed: %v\n", err)
			os.Exit(1)
		}
		fmt.Printf("Restored container %s from snapshot %q\n", containerID[:8], snapName)
	default:
		fmt.Fprintf(os.Stderr, "unknown snapshot command: %s\n", subCmd)
		os.Exit(1)
	}
}

func cmdInfo(args []string) {
	st, err := openStore()
	if err != nil {
		fmt.Fprintf(os.Stderr, "store error: %v\n", err)
		os.Exit(1)
	}
	report, err := system.GenerateEngineReport(st)
	if err != nil {
		fmt.Fprintf(os.Stderr, "report error: %v\n", err)
		os.Exit(1)
	}
	raw, _ := json.MarshalIndent(report, "", "  ")
	fmt.Println(string(raw))
}

func cmdRename(args []string) {
	if len(args) < 2 {
		fmt.Fprintln(os.Stderr, "Usage: minictl rename <container-id> <new-name>")
		os.Exit(1)
	}
	st, err := openStore()
	if err != nil {
		fmt.Fprintf(os.Stderr, "store error: %v\n", err)
		os.Exit(1)
	}
	if err := container.RenameContainer(st, args[0], args[1]); err != nil {
		fmt.Fprintf(os.Stderr, "rename error: %v\n", err)
		os.Exit(1)
	}
	fmt.Printf("Renamed container %s -> %s\n", args[0][:min(8, len(args[0]))], args[1])
}

func cmdImport(args []string) {
	if len(args) < 2 {
		fmt.Fprintln(os.Stderr, "Usage: minictl import <tarball> <image-tag>")
		os.Exit(1)
	}
	st, err := openStore()
	if err != nil {
		fmt.Fprintf(os.Stderr, "store error: %v\n", err)
		os.Exit(1)
	}
	rec, err := imagestore.ImportRawRootFS(st, args[0], args[1])
	if err != nil {
		fmt.Fprintf(os.Stderr, "import error: %v\n", err)
		os.Exit(1)
	}
	fmt.Printf("Imported image %s [%s]\n", rec.ID[:8], rec.Tag)
}

func cmdWait(args []string) {
	if len(args) < 1 {
		fmt.Fprintln(os.Stderr, "Usage: minictl wait <container-id>")
		os.Exit(1)
	}
	st, err := openStore()
	if err != nil {
		fmt.Fprintf(os.Stderr, "store error: %v\n", err)
		os.Exit(1)
	}
	exitCode, err := container.WaitContainer(st, args[0])
	if err != nil {
		fmt.Fprintf(os.Stderr, "wait error: %v\n", err)
		os.Exit(1)
	}
	fmt.Printf("%d\n", exitCode)
}

func cmdImages() {
	store, err := openStore()
	if err != nil {
		fmt.Fprintf(os.Stderr, "error: %v\n", err)
		os.Exit(1)
	}

	imgs, err := store.ListImages()
	if err != nil {
		fmt.Fprintf(os.Stderr, "error listing images: %v\n", err)
		os.Exit(1)
	}

	w := tabwriter.NewWriter(os.Stdout, 0, 0, 2, ' ', 0)
	fmt.Fprintln(w, "REPOSITORY\tTAG\tIMAGE ID\tSIZE\tCREATED")
	for _, img := range imgs {
		repo := img.Repository
		if repo == "" {
			repo = img.Name
		}
		tag := img.Tag
		if tag == "" {
			tag = "latest"
		}
		id := img.ID
		if id == "" {
			id = "N/A"
		}
		if len(id) > 12 {
			id = id[:12]
		}
		age := time.Since(img.LoadedAt).Round(time.Second)
		fmt.Fprintf(w, "%s\t%s\t%s\t%d B\t%s ago\n", repo, tag, id, img.Size, age)
	}
	_ = w.Flush()
}

func cmdLoad(args []string) {
	if len(args) != 2 {
		fmt.Fprintln(os.Stderr, "Usage: minictl load <docker-save.tar> <dest-dir>")
		os.Exit(1)
	}
	tarFile := args[0]
	destDir := args[1]

	if err := image.LoadDockerSave(tarFile, destDir); err != nil {
		fmt.Fprintf(os.Stderr, "load failed: %v\n", err)
		os.Exit(1)
	}

	store, _ := openStore()
	if store != nil {
		imgName := filepath.Base(tarFile)
		imgName = strings.TrimSuffix(imgName, filepath.Ext(imgName))
		_ = store.SaveImage(&state.Image{
			Name:     imgName,
			RootFS:   destDir,
			LoadedAt: time.Now(),
		})
	}
}

func cmdUnpack(args []string) {
	if len(args) != 2 {
		fmt.Fprintln(os.Stderr, "Usage: minictl unpack <tar-file> <dest-dir>")
		os.Exit(1)
	}
	tarFile := args[0]
	destDir := args[1]

	fmt.Printf("Unpacking %s -> %s\n", tarFile, destDir)
	if err := image.Unpack(tarFile, destDir); err != nil {
		fmt.Fprintf(os.Stderr, "unpack failed: %v\n", err)
		os.Exit(1)
	}
	fmt.Printf("Done. Rootfs ready at %s\n", destDir)

	store, _ := openStore()
	if store != nil {
		imgName := filepath.Base(tarFile)
		imgName = strings.TrimSuffix(imgName, ".tar.gz")
		imgName = strings.TrimSuffix(imgName, ".tgz")
		imgName = strings.TrimSuffix(imgName, ".tar")
		_ = store.SaveImage(&state.Image{
			Name:     imgName,
			RootFS:   destDir,
			LoadedAt: time.Now(),
		})
	}
}

func openStore() (*state.Store, error) {
	return state.Open(state.DefaultDir())
}

func parseRunConfig(args []string) (container.Config, error) {
	fs := flag.NewFlagSet("run", flag.ContinueOnError)
	fs.SetOutput(io.Discard)

	overlay := fs.Bool("overlay", false, "use OverlayFS Copy-on-Write storage layer")
	readOnly := fs.Bool("read-only", false, "mount container rootfs as read-only")
	restart := fs.String("restart", "no", "restart policy: no, always, or on-failure")
	cpus := fs.Float64("cpus", 0.0, "hard fractional CPU limit (e.g. 0.5 = 50% CPU, 2.0 = 2 CPUs)")
	noUserNS := fs.Bool("no-user-ns", false, "disable user namespace; requires root")
	bridge := fs.Bool("bridge", false, "enable veth pair networking")
	seccomp := fs.Bool("seccomp", false, "install seccomp BPF syscall block-list filter")
	hostname := fs.String("hostname", defaultHostname, "container hostname")
	var workDir string
	fs.StringVar(&workDir, "workdir", "/", "container working directory")
	fs.StringVar(&workDir, "w", "/", "alias for --workdir")
	memory := fs.String("memory", "", "memory limit in bytes, k, m, or g")
	cpuWeight := fs.Int64("cpu-weight", 0, "cgroup v2 CPU weight, 1..10000")
	pidsLimit := fs.Int64("pids-limit", 0, "maximum process/thread count")

	var rawCapDrops stringList
	fs.Var(&rawCapDrops, "cap-drop", "Linux capability to drop from bounding set (repeatable)")

	var rawEnvs stringList
	fs.Var(&rawEnvs, "env", "environment variable: KEY=VAL (repeatable)")
	fs.Var(&rawEnvs, "e", "alias for --env")

	var rawPorts stringList
	fs.Var(&rawPorts, "publish", "port mapping: hostPort:containerPort[/tcp|udp] (requires --bridge)")
	fs.Var(&rawPorts, "p", "alias for --publish")

	var rawVolumes stringList
	fs.Var(&rawVolumes, "volume", "bind mount: host:container[:ro] (repeatable)")
	fs.Var(&rawVolumes, "v", "alias for --volume")

	if err := fs.Parse(args); err != nil {
		return container.Config{}, err
	}

	rest := fs.Args()
	if len(rest) < 1 {
		return container.Config{}, fmt.Errorf("missing rootfs")
	}

	memoryBytes, err := parseByteSize(*memory)
	if err != nil {
		return container.Config{}, fmt.Errorf("--memory: %w", err)
	}
	if *cpuWeight < 0 || *cpuWeight > 10000 {
		return container.Config{}, fmt.Errorf("--cpu-weight must be 0 or between 1 and 10000")
	}
	if *cpus < 0 {
		return container.Config{}, fmt.Errorf("--cpus must be >= 0")
	}
	if *pidsLimit < 0 {
		return container.Config{}, fmt.Errorf("--pids-limit must be >= 0")
	}

	name := strings.TrimSpace(*hostname)
	if name == "" {
		name = defaultHostname
	}

	ports := make([]container.PortMapping, 0, len(rawPorts))
	for _, spec := range rawPorts {
		pm, err := parsePortSpec(spec)
		if err != nil {
			return container.Config{}, fmt.Errorf("-p %q: %w", spec, err)
		}
		ports = append(ports, pm)
	}

	volumes := make([]container.Volume, 0, len(rawVolumes))
	for _, spec := range rawVolumes {
		v, err := parseVolumeSpec(spec)
		if err != nil {
			return container.Config{}, fmt.Errorf("-v %q: %w", spec, err)
		}
		volumes = append(volumes, v)
	}

	return container.Config{
		RootFS:        rest[0],
		Overlay:       *overlay,
		ReadOnly:      *readOnly,
		Restart:       *restart,
		CapDrop:       rawCapDrops,
		Command:       rest[1:],
		Hostname:      name,
		WorkDir:       workDir,
		Env:           rawEnvs,
		Memory:        memoryBytes,
		CPUWeight:     *cpuWeight,
		CPUs:          *cpus,
		PidsLimit:     *pidsLimit,
		Volumes:       volumes,
		PortMappings:  ports,
		BridgeNetwork: *bridge,
		Seccomp:       *seccomp,
		UserNS:        !*noUserNS,
		Debug:         os.Getenv("MINICONTAINER_DEBUG") == "1",
	}, nil
}

type stringList []string

func (sl *stringList) String() string { return strings.Join(*sl, ", ") }
func (sl *stringList) Set(s string) error {
	*sl = append(*sl, s)
	return nil
}

func parsePortSpec(spec string) (container.PortMapping, error) {
	protocol := "tcp"
	if idx := strings.Index(spec, "/"); idx != -1 {
		protocol = strings.ToLower(spec[idx+1:])
		spec = spec[:idx]
	}
	parts := strings.Split(spec, ":")
	if len(parts) != 2 {
		return container.PortMapping{}, fmt.Errorf("expected hostPort:containerPort")
	}
	hPort, err := strconv.Atoi(parts[0])
	if err != nil || hPort <= 0 || hPort > 65535 {
		return container.PortMapping{}, fmt.Errorf("invalid host port %q", parts[0])
	}
	cPort, err := strconv.Atoi(parts[1])
	if err != nil || cPort <= 0 || cPort > 65535 {
		return container.PortMapping{}, fmt.Errorf("invalid container port %q", parts[1])
	}
	return container.PortMapping{
		HostPort:      hPort,
		ContainerPort: cPort,
		Protocol:      protocol,
	}, nil
}

func parseVolumeSpec(spec string) (container.Volume, error) {
	parts := strings.SplitN(spec, ":", 3)
	if len(parts) < 2 {
		return container.Volume{}, fmt.Errorf("expected host:container[:ro]")
	}

	hostSpec := parts[0]
	resolvedHost := volume.ResolveVolumePath(hostSpec)

	v := container.Volume{
		HostPath:      filepath.Clean(resolvedHost),
		ContainerPath: parts[1],
	}

	if len(parts) == 3 {
		if parts[2] != "ro" {
			return container.Volume{}, fmt.Errorf("unknown modifier %q (only :ro is supported)", parts[2])
		}
		v.ReadOnly = true
	}

	if !filepath.IsAbs(v.HostPath) {
		return container.Volume{}, fmt.Errorf("host path %q must be absolute or a valid named volume", hostSpec)
	}
	if !strings.HasPrefix(v.ContainerPath, "/") {
		return container.Volume{}, fmt.Errorf("container path %q must be absolute", v.ContainerPath)
	}

	return v, nil
}

func parseByteSize(raw string) (int64, error) {
	value := strings.TrimSpace(raw)
	if value == "" {
		return 0, nil
	}

	lower := strings.ToLower(value)
	multiplier := int64(1)
	for _, suffix := range []struct {
		text       string
		multiplier int64
	}{
		{"kib", 1024},
		{"kb", 1024},
		{"k", 1024},
		{"mib", 1024 * 1024},
		{"mb", 1024 * 1024},
		{"m", 1024 * 1024},
		{"gib", 1024 * 1024 * 1024},
		{"gb", 1024 * 1024 * 1024},
		{"g", 1024 * 1024 * 1024},
		{"b", 1},
	} {
		if strings.HasSuffix(lower, suffix.text) {
			value = strings.TrimSpace(value[:len(value)-len(suffix.text)])
			multiplier = suffix.multiplier
			break
		}
	}

	if value == "" {
		return 0, fmt.Errorf("missing number")
	}

	n, err := strconv.ParseInt(value, 10, 64)
	if err != nil {
		return 0, fmt.Errorf("invalid size %q", raw)
	}
	if n < 0 {
		return 0, fmt.Errorf("must be >= 0")
	}
	if n > (1<<63-1)/multiplier {
		return 0, fmt.Errorf("size overflows int64")
	}
	return n * multiplier, nil
}
