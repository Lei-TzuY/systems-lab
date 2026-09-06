# Makefile — Mini Container Runtime
#
# All targets are meant to be run inside a Linux environment (WSL 2, Linux VM,
# or a Linux cloud VM).  See cmd/minictl/main.go for setup instructions.
#
# Quick start:
#   make build          # compile the binary
#   make rootfs         # download Alpine Linux and unpack it
#   make run-sh         # open a shell inside the container
#   make demo           # run a one-shot echo inside the container

.PHONY: build rootfs run-sh run-echo demo clean vet test help

BINARY  := minictl
OUTDIR  := build
CMD     := ./cmd/minictl

ALPINE_VERSION := 3.19.0
ALPINE_ARCH    := x86_64
ALPINE_TAR     := alpine-minirootfs-$(ALPINE_VERSION)-$(ALPINE_ARCH).tar.gz
ALPINE_URL     := https://dl-cdn.alpinelinux.org/alpine/v3.19/releases/$(ALPINE_ARCH)/$(ALPINE_TAR)
ROOTFS_DIR     := rootfs

# ── Build ──────────────────────────────────────────────────────────────────────

build:
	@mkdir -p $(OUTDIR)
	GOOS=linux GOARCH=amd64 go build -o $(OUTDIR)/$(BINARY) $(CMD)
	@echo "Built: $(OUTDIR)/$(BINARY)"

# ── Rootfs preparation ─────────────────────────────────────────────────────────

# Download the Alpine Linux minirootfs tarball (if not already present) and
# unpack it with our own minictl binary.  Alpine is used because it is tiny
# (~5 MB) and has a working /bin/sh (busybox ash).
rootfs: build
	@if [ ! -f $(ALPINE_TAR) ]; then \
		echo "Downloading Alpine Linux $(ALPINE_VERSION) minirootfs…"; \
		wget -q --show-progress "$(ALPINE_URL)" -O $(ALPINE_TAR) \
		  || curl -L --progress-bar "$(ALPINE_URL)" -o $(ALPINE_TAR); \
	fi
	@rm -rf $(ROOTFS_DIR)
	$(OUTDIR)/$(BINARY) unpack $(ALPINE_TAR) $(ROOTFS_DIR)
	@echo "Rootfs ready at ./$(ROOTFS_DIR)"

# ── Run targets (require root / sudo) ─────────────────────────────────────────

# Open an interactive BusyBox shell inside the container.
run-sh: build
	@test -d $(ROOTFS_DIR)/bin || { echo "Run 'make rootfs' first"; exit 1; }
	sudo $(OUTDIR)/$(BINARY) run ./$(ROOTFS_DIR) /bin/sh

# Run a single command and exit.
run-echo: build
	@test -d $(ROOTFS_DIR)/bin || { echo "Run 'make rootfs' first"; exit 1; }
	sudo $(OUTDIR)/$(BINARY) run ./$(ROOTFS_DIR) /bin/echo "hello from minicontainer"

# Demonstration sequence: show hostname, PID tree, and mounts.
demo: build
	@test -d $(ROOTFS_DIR)/bin || { echo "Run 'make rootfs' first"; exit 1; }
	@echo "=== hostname ==="
	sudo $(OUTDIR)/$(BINARY) run ./$(ROOTFS_DIR) /bin/hostname
	@echo "=== ps aux ==="
	sudo $(OUTDIR)/$(BINARY) run ./$(ROOTFS_DIR) /bin/ps aux
	@echo "=== mount ==="
	sudo $(OUTDIR)/$(BINARY) run ./$(ROOTFS_DIR) /bin/mount

# Run with debug tracing enabled (shows every significant syscall).
debug-sh: build
	@test -d $(ROOTFS_DIR)/bin || { echo "Run 'make rootfs' first"; exit 1; }
	sudo MINICONTAINER_DEBUG=1 $(OUTDIR)/$(BINARY) run ./$(ROOTFS_DIR) /bin/sh

# ── Development ────────────────────────────────────────────────────────────────

vet:
	go vet ./...

test:
	go test -v ./...

# ── Cleanup ────────────────────────────────────────────────────────────────────

clean:
	rm -rf $(OUTDIR) $(ROOTFS_DIR) $(ALPINE_TAR)

# ── Help ───────────────────────────────────────────────────────────────────────

help:
	@echo ""
	@echo "Mini Container Runtime — Makefile targets"
	@echo ""
	@echo "  make build        Compile minictl (output: ./build/minictl)"
	@echo "  make rootfs       Download Alpine Linux and prepare rootfs"
	@echo "  make run-sh       Open /bin/sh inside the container  [requires sudo]"
	@echo "  make run-echo     Run a one-shot echo command         [requires sudo]"
	@echo "  make demo         Show hostname, ps, and mount inside container [requires sudo]"
	@echo "  make debug-sh     Same as run-sh but with MINICONTAINER_DEBUG=1"
	@echo "  make vet          Run go vet"
	@echo "  make test         Run go test"
	@echo "  make clean        Remove build artifacts and rootfs"
	@echo ""
	@echo "  CLI subcommands:"
	@echo "    minictl run, exec, ps, logs, stats, pause, unpause, kill, rm, images"
	@echo ""
