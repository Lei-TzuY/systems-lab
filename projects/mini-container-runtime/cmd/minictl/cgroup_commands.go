package main

import (
	"flag"
	"fmt"
	"io"
	"os"
	"text/tabwriter"

	"minicontainer/internal/cgroups"
	"minicontainer/internal/container"
	"minicontainer/internal/events"
	"minicontainer/internal/state"
	"minicontainer/internal/stats"
)

type updateCommandOptions struct {
	containerID string
	config      cgroups.UpdateConfig
}

func parseUpdateCommandArgs(args []string) (updateCommandOptions, error) {
	fs := flag.NewFlagSet("update", flag.ContinueOnError)
	fs.SetOutput(io.Discard)

	memory := fs.String("memory", "", "memory limit in bytes, k, m, or g")
	cpus := fs.Float64("cpus", 0.0, "hard fractional CPU limit")
	cpuWeight := fs.Int64("cpu-weight", 0, "cgroup v2 CPU weight")
	pidsLimit := fs.Int64("pids-limit", 0, "maximum process count")
	if err := fs.Parse(args); err != nil {
		return updateCommandOptions{}, err
	}
	if len(fs.Args()) != 1 {
		return updateCommandOptions{}, fmt.Errorf("expected exactly one container id")
	}
	memoryBytes, err := parseByteSize(*memory)
	if err != nil {
		return updateCommandOptions{}, fmt.Errorf("--memory: %w", err)
	}

	return updateCommandOptions{
		containerID: fs.Args()[0],
		config: cgroups.UpdateConfig{
			MemoryMax: memoryBytes,
			CPUs:      *cpus,
			CPUWeight: *cpuWeight,
			PidsMax:   *pidsLimit,
		},
	}, nil
}

func cmdUpdateSafe(args []string) {
	opts, err := parseUpdateCommandArgs(args)
	if err != nil {
		fmt.Fprintf(os.Stderr, "update: %v\n", err)
		fmt.Fprintln(os.Stderr, "Usage: minictl update [--memory size] [--cpus float] [--cpu-weight n] [--pids-limit n] <id>")
		os.Exit(1)
	}

	store, err := openStore()
	if err != nil {
		fmt.Fprintf(os.Stderr, "error: %v\n", err)
		os.Exit(1)
	}
	rec, err := container.UpdateContainerResourcesResolved(store, opts.containerID, opts.config, os.Getenv("MINICONTAINER_DEBUG") == "1")
	if err != nil {
		fmt.Fprintf(os.Stderr, "update error: %v\n", err)
		os.Exit(1)
	}
	fmt.Printf("%s\n", shortContainerID(rec.ID))
}

func cmdPauseSafe(args []string) {
	if len(args) != 1 {
		fmt.Fprintln(os.Stderr, "Usage: minictl pause <id>")
		os.Exit(1)
	}
	store, err := openStore()
	if err != nil {
		fmt.Fprintf(os.Stderr, "error: %v\n", err)
		os.Exit(1)
	}
	rec, err := container.FreezeContainerResolved(store, args[0])
	if err != nil {
		fmt.Fprintf(os.Stderr, "pause error: %v\n", err)
		os.Exit(1)
	}
	_ = events.Publish(events.EventPause, rec.ID, rec.RootFS, "paused container process generation")
	fmt.Printf("%s\n", shortContainerID(rec.ID))
}

func cmdUnpauseSafe(args []string) {
	if len(args) != 1 {
		fmt.Fprintln(os.Stderr, "Usage: minictl unpause <id>")
		os.Exit(1)
	}
	store, err := openStore()
	if err != nil {
		fmt.Fprintf(os.Stderr, "error: %v\n", err)
		os.Exit(1)
	}
	rec, err := container.ThawContainerResolved(store, args[0])
	if err != nil {
		fmt.Fprintf(os.Stderr, "unpause error: %v\n", err)
		os.Exit(1)
	}
	_ = events.Publish(events.EventUnpause, rec.ID, rec.RootFS, "unpaused container process generation")
	fmt.Printf("%s\n", shortContainerID(rec.ID))
}

func cmdStatsSafe(args []string) {
	if len(args) > 1 {
		fmt.Fprintln(os.Stderr, "Usage: minictl stats [id]")
		os.Exit(1)
	}
	store, err := openStore()
	if err != nil {
		fmt.Fprintf(os.Stderr, "error: %v\n", err)
		os.Exit(1)
	}

	collected, err := stats.CollectStats(store)
	if err != nil {
		fmt.Fprintf(os.Stderr, "stats error: %v\n", err)
		os.Exit(1)
	}
	if len(args) == 1 {
		rec, err := store.Resolve(args[0])
		if err != nil {
			fmt.Fprintf(os.Stderr, "error: %v\n", err)
			os.Exit(1)
		}
		if rec.Status != state.StatusRunning {
			fmt.Fprintf(os.Stderr, "container %s is not running\n", shortContainerID(rec.ID))
			os.Exit(1)
		}
		filtered := collected[:0]
		for _, item := range collected {
			if item.ContainerID == rec.ID {
				filtered = append(filtered, item)
				break
			}
		}
		collected = filtered
	}

	w := tabwriter.NewWriter(os.Stdout, 0, 0, 2, ' ', 0)
	fmt.Fprintln(w, "CONTAINER ID\tPID\tMEM USAGE / LIMIT\tPIDS\tCPU USEC\tSTATUS")
	for _, item := range collected {
		if !item.Available {
			reason := item.UnavailableReason
			if reason == "" {
				reason = "unavailable"
			}
			fmt.Fprintf(w, "%s\t%d\tN/A\tN/A\tN/A\t%s\n",
				shortContainerID(item.ContainerID), item.PID, reason)
			continue
		}
		limit := "unlimited"
		if item.MemLimitBytes > 0 {
			limit = fmt.Sprintf("%.2f MiB", float64(item.MemLimitBytes)/(1024*1024))
		}
		usage := fmt.Sprintf("%.2f MiB", float64(item.MemBytes)/(1024*1024))
		fmt.Fprintf(w, "%s\t%d\t%s / %s\t%d\t%d us\tok\n",
			shortContainerID(item.ContainerID), item.PID, usage, limit, item.PIDs, item.CPUUsageUsec)
	}
	_ = w.Flush()
}
