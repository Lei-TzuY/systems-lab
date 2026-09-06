package main

import (
	"flag"
	"fmt"
	"io"
	"os"
	"time"

	"minicontainer/internal/container"
	"minicontainer/internal/events"
)

type stopCommandOptions struct {
	containerID string
	timeout     time.Duration
	signal      string
}

func parseStopCommandArgs(args []string) (stopCommandOptions, error) {
	fs := flag.NewFlagSet("stop", flag.ContinueOnError)
	fs.SetOutput(io.Discard)

	var timeoutSec int
	var signal string
	fs.IntVar(&timeoutSec, "t", 10, "seconds to wait for stop before killing")
	fs.IntVar(&timeoutSec, "timeout", 10, "seconds to wait for stop before killing")
	fs.StringVar(&signal, "s", "SIGTERM", "graceful signal to send before timeout")
	fs.StringVar(&signal, "signal", "SIGTERM", "graceful signal to send before timeout")
	if err := fs.Parse(args); err != nil {
		return stopCommandOptions{}, err
	}
	if timeoutSec < 0 {
		return stopCommandOptions{}, fmt.Errorf("stop timeout must not be negative")
	}
	maxSeconds := int64((time.Duration(1<<63 - 1)) / time.Second)
	if int64(timeoutSec) > maxSeconds {
		return stopCommandOptions{}, fmt.Errorf("stop timeout is too large")
	}
	if _, err := container.ParseSignal(signal); err != nil {
		return stopCommandOptions{}, err
	}
	rest := fs.Args()
	if len(rest) != 1 {
		return stopCommandOptions{}, fmt.Errorf("expected exactly one container id")
	}
	return stopCommandOptions{
		containerID: rest[0],
		timeout:     time.Duration(timeoutSec) * time.Second,
		signal:      signal,
	}, nil
}

type killCommandOptions struct {
	containerID string
	signal      string
}

func parseKillCommandArgs(args []string) (killCommandOptions, error) {
	fs := flag.NewFlagSet("kill", flag.ContinueOnError)
	fs.SetOutput(io.Discard)

	var signal string
	fs.StringVar(&signal, "s", "SIGKILL", "signal to send to container")
	fs.StringVar(&signal, "signal", "SIGKILL", "signal to send to container")
	if err := fs.Parse(args); err != nil {
		return killCommandOptions{}, err
	}
	rest := fs.Args()
	if len(rest) != 1 {
		return killCommandOptions{}, fmt.Errorf("expected exactly one container id")
	}
	if _, err := container.ParseSignal(signal); err != nil {
		return killCommandOptions{}, err
	}
	return killCommandOptions{containerID: rest[0], signal: signal}, nil
}

func cmdStopSafe(args []string) {
	opts, err := parseStopCommandArgs(args)
	if err != nil {
		fmt.Fprintf(os.Stderr, "stop: %v\n", err)
		fmt.Fprintln(os.Stderr, "Usage: minictl stop [-t timeout] [-s SIGNAL] <id>")
		os.Exit(1)
	}

	store, err := openStore()
	if err != nil {
		fmt.Fprintf(os.Stderr, "error: %v\n", err)
		os.Exit(1)
	}
	rec, err := container.StopContainerWithSignal(store, opts.containerID, opts.signal, opts.timeout)
	if err != nil {
		fmt.Fprintf(os.Stderr, "stop error: %v\n", err)
		os.Exit(1)
	}
	_ = events.Publish(events.EventStop, rec.ID, rec.RootFS, fmt.Sprintf("stopped container process identity with %s", opts.signal))
	fmt.Printf("%s\n", shortContainerID(rec.ID))
}

func cmdKillSafe(args []string) {
	opts, err := parseKillCommandArgs(args)
	if err != nil {
		fmt.Fprintf(os.Stderr, "kill: %v\n", err)
		fmt.Fprintln(os.Stderr, "Usage: minictl kill [-s SIGNAL] <id>")
		os.Exit(1)
	}

	store, err := openStore()
	if err != nil {
		fmt.Fprintf(os.Stderr, "error: %v\n", err)
		os.Exit(1)
	}
	rec, err := container.SendSignalResolved(store, opts.containerID, opts.signal)
	if err != nil {
		fmt.Fprintf(os.Stderr, "kill error: %v\n", err)
		os.Exit(1)
	}

	_ = events.Publish(events.EventSignal, rec.ID, rec.RootFS, fmt.Sprintf("sent signal %s", opts.signal))
	fmt.Printf("%s\n", shortContainerID(rec.ID))
}

func shortContainerID(id string) string {
	if len(id) > 8 {
		return id[:8]
	}
	return id
}
