# Mini Docker & Container Runtime (`minictl`)

A minimal, educational Linux container runtime written in Go from scratch. `minictl` directly invokes Linux kernel syscalls (`clone`, `pivot_root`, `setns`, `mount`, `prctl`) to implement process isolation, namespaces, cgroups v2 resource controls, OverlayFS copy-on-write storage, veth bridge networking, port mapping via iptables DNAT, Linux capabilities management, HTTP OCI registry image pulling, multi-container orchestration (`minictl compose`), dynamic resource updates (`minictl update`), real-time event streaming (`minictl events`), bidirectional container file transfer (`minictl cp`), health checking, and BPF seccomp syscall filtering without depending on Docker, containerd, or runc internally.

---

## 🏛️ Architecture & Syscall Mapping

```
                       ┌──────────────────────────────┐
                       │          minictl CLI         │
                       └──────────────┬───────────────┘
                                      │
              ┌───────────────────────┴───────────────────────┐
              ▼                                               ▼
   Stage 1: Parent Process                          Stage 2: Container Init
   • clone(CLONE_NEWPID|NEWUTS|NEWNS|...)           • Wait on sync pipe from parent
   • Apply cgroups v2 limits & cpus quota           • sethostname(2) in UTS netns
   • Create veth-h<pid> ↔ eth0 pair                 • Mount OverlayFS (--overlay)
   • Configure iptables DNAT (-p)                   • Mount /proc, /sys, /dev/pts
   • Move veth-peer to child netns                  • Bind mount volumes (-v)
   • Unblock child via sync pipe                    • pivot_root(2) into rootfs
   • Wait for child exit & cleanup NAT              • Remount root read-only (--read-only)
                                                    • chdir(2) to workdir (-w)
                                                    • Drop capabilities (PR_CAPBSET_DROP)
                                                    • Install seccomp BPF filter
                                                    • Inject env vars (-e) & execve(2)
```

### Key Subsystems & Syscalls Used

| Feature | Mechanism / Syscall | Purpose |
| :--- | :--- | :--- |
| **PID Namespace** | `clone(CLONE_NEWPID)` | Isolated PID number space; container payload becomes PID 1 inside. |
| **UTS Namespace** | `clone(CLONE_NEWUTS)` + `sethostname(2)` | Isolated hostname & NIS domain name. |
| **Mount Namespace** | `clone(CLONE_NEWNS)` + `mount(2)` | Isolated mount table; host mounts remain hidden. |
| **Network Namespace** | `clone(CLONE_NEWNET)` + Netlink sockets | Isolated network stack (lo interface + veth pair). |
| **IPC Namespace** | `clone(CLONE_NEWIPC)` | Isolated System V IPC & POSIX message queues. |
| **User Namespace** | `clone(CLONE_NEWUSER)` + `uid_map` / `gid_map` | Maps host UID to container root (0/0); enables unprivileged containers. |
| **RootFS Isolation** | `pivot_root(2)` (with `chroot(2)` fallback) | Swaps `/` to new rootfs and unmounts host root via `MNT_DETACH`. |
| **OverlayFS (CoW)** | `mount -t overlay ...` | Merges read-only image layers with a per-container writable `upper` directory. |
| **Resource Limits** | Cgroups v2 (`/sys/fs/cgroup/...`) | Hard memory limits, fractional CPU quotas (`cpu.max`), CPU weights, and PIDs limit. |
| **Dynamic Updates** | Cgroups v2 `/sys/fs/cgroup` | Dynamically modifies memory (`memory.max`) and CPU (`cpu.max`) quotas on the fly. |
| **Process Freezer** | Cgroup v2 `cgroup.freeze` | Atomic pause/unpause of container processes without signal leakage. |
| **Container Exec** | `setns(2)` + `chroot(2)` | Attaches to an existing container's namespaces by host PID. |
| **Capabilities Control**| `prctl(PR_CAPBSET_DROP)` | Drops specific privileges (`CAP_SYS_ADMIN`, `CAP_NET_RAW`) from bounding set. |
| **OCI Image Pulling** | HTTP REST API v2 | Downloads image manifests and layer blobs directly from Docker Hub (`minictl pull`). |
| **Mini Compose** | Declarative JSON Runner | Orchestrates multi-container applications (`minictl compose up -f compose.json`). |
| **Event Audit Stream**| Real-time Event Logger | Streams container lifecycle events (`minictl events -f`). |
| **Container File Copy** | Bidirectional File Transfer | Copies files/directories between host and container (`minictl cp`). |
| **Health Checking** | State & Exec Evaluator | Evaluates container health status (`starting` -> `healthy` / `unhealthy`). |
| **Custom Networks** | `ip link add type bridge` | Creates and manages custom Layer 2 Linux bridge networks. |
| **Port Mapping** | `iptables` DNAT | Forwards host ports to container private IP (172.20.0.2). |
| **Seccomp Security** | `prctl(PR_SET_SECCOMP)` + cBPF | Restricts dangerous syscalls (`ptrace`, `kexec_load`, `init_module`, etc.). |
| **Log Run-to-Run Diff Comparator**| Execution Divergence Analyzer| Compares log streams across container runs/replicas with similarity ratio (`minictl logs --diff`). |
| **StopSignal Graceful Auditor**| Graceful Shutdown Auditor| Resolves `config.StopSignal` (SIGTERM/SIGINT/SIGQUIT/SIGKILL) and recommends shutdown timeouts. |
| **PIDs Peak Concurrent Tracker** | Cgroup v2 `pids.peak` | Reads the highest recorded concurrent process/thread count when the kernel exposes this read-only telemetry. |
| **DNS UseVC Attempts+Timeout+Ndots**| `/etc/resolv.conf` Driver| Quad-option decorator: `use-vc`, `attempts:N`, `timeout:M`, and `ndots:K` in one formatted line. |
| **Log Time-Window Grouper**| Fixed-Window Metrics Bucketer| Aggregates logs into time slices with error/warn counters and ASCII histogram (`minictl logs --window 1m`). |
| **User Namespace UID/GID Validator**| Rootless ID Mapper| Evaluates image User settings and validates compatibility with host subuid/subgid ranges. |
| **CPU Max Burst Quota Budget**| Cgroup v2 `cpu.max.burst`| Configures accumulated CFS burst quota in microseconds for peak processing (Linux 5.14+). |
| **DNS TrustAD Attempts+Timeout+Ndots**| `/etc/resolv.conf` Driver| Quad-option decorator: `trust-ad`, `attempts:N`, `timeout:M`, and `ndots:K` in one formatted line. |
| **Log Batch & Reservoir Sampler**| Stream Statistical Reducer| Samples 1-in-N, fractional rates, or fixed-k reservoirs to reduce logging telemetry volume (`minictl logs --sample`). |
| **Manifest Layer Size Diff**| Image Upgrade Diff Engine| Compares two manifest versions, measuring shared layer reuse, additions, deletions, and net byte delta. |
| **Proactive Memory Reclaim Options**| Cgroup v2 `memory.reclaim`| Compacts pagecache/swap with configurable swappiness and NUMA target node parameters (Linux 6.8+). |
| **DNS EDNS0 Attempts+Timeout+Ndots**| `/etc/resolv.conf` Driver| Quad-option decorator: `edns0`, `attempts:N`, `timeout:M`, and `ndots:K` in one formatted line. |
| **Log ANSI Colorizer & Stripper**| Severity Terminal Highlighter| Colorizes container logs by severity and strips ANSI escape sequences (`minictl logs --color`). |
| **Exposed Port Conflict Detector**| Multi-Image Port Auditor| Detects overlapping port bindings across multiple images before container scheduling. |
| **Swap Usage & High Limit**| Cgroup v2 `memory.swap.high`| Reads swap usage and configures throttle watermark limits to prevent host disk thrashing. |
| **DNS Rotate Attempts+Timeout**| `/etc/resolv.conf` Driver| Triple-option decorator: `rotate`, `attempts:N`, and `timeout:M` in one formatted line. |
| **Log Head/Tail Line Selector**| Stream Head & Tail Slicer| Extracts first N, last N, or combined head+tail lines with skip separator (`minictl logs --head --tail`). |
| **Label Policy Compliance Checker**| OCI Label Auditor| Validates required OCI recommended labels (title, version, vendor, source, licenses) with 0-100 score. |
| **CPU Stat Throttle Metrics**| Cgroup v2 `cpu.stat` Periods| Reads `nr_periods`, `nr_throttled`, `throttled_usec` counters and computes throttle percentage. |
| **DNS IP6Dotint Attempts+Timeout**| `/etc/resolv.conf` Driver| Triple-option decorator: `ip6-dotint`, `attempts:N`, and `timeout:M` in one formatted line. |
| **Log JSON-to-Logfmt Converter**| Structured Log Flattener| Converts JSON container logs to flat `key=value` logfmt format for grep and shell pipelines. |
| **Dockerfile Reconstructor**| History Reverse-Engineer| Reconstructs best-effort Dockerfile from OCI Image Config `history[]` layer commands. |
| **CPU Weight Nice Controller**| Cgroup v2 `cpu.weight.nice`| Reads and writes traditional Unix nice values (-20 to 19) for container CPU scheduling priority. |
| **DNS IP6Bytestring Attempts+Timeout**| `/etc/resolv.conf` Driver| Triple-option decorator: `ip6-bytestring`, `attempts:N`, and `timeout:M` in one formatted line. |
| **Log Syslog RFC 5424 Formatter**| Syslog Protocol Encoder| Formats logs into standard RFC 5424 Syslog records for centralized log daemon forwarding. |
| **Env Variable Interpolator**| Runtime POSIX Expander | Expands nested `${VAR}`, `${VAR:-default}`, and `$VAR` substitutions in OCI Image Config. |
| **Misc Events Max Counter**| Cgroup v2 `misc.events`| Reads hardware security extension capacity failure counters (SEV, TDX limits). |
| **DNS Debug Attempts+Timeout**| `/etc/resolv.conf` Driver| Triple-option decorator: `debug`, `attempts:N`, and `timeout:M` in one formatted line. |
| **Log Delta Time Annotator**| Inter-Line Latency Profiler| Computes elapsed time offsets between logs (`[+15.2ms]`, `[+1.50s]`) to locate slow steps (`minictl logs --delta-time`). |
| **Security Risk Auditor**| Privilege & Vulnerability Checker| Audits root execution, exposed privileged ports (<1024), and hardcoded environment credentials. |
| **Misc Resources Reader**| Cgroup v2 `misc.current`| Tracks hardware security extension ASID allocations (AMD SEV, SEV-ES, Intel TDX). |
| **DNS NoCheck Attempts+Timeout**| `/etc/resolv.conf` Driver| Triple-option decorator: `no-check-names`, `attempts:N`, and `timeout:M` in one formatted line. |
| **Log Alert Engine**    | Trigger-Based Alert Engine| Real-time scanning for panic, fatal, OOM killer, and segfault alert triggers (`minictl logs --alert-on`). |
| **Reproducible Build Auditor**| Deterministic Build Auditor| Checks for epoch zero/fixed timestamps, sorted environment variables, and build reproducibility. |
| **RDMA Resources Reader**| Cgroup v2 `rdma.current`| Tracks HCA device handle and object allocations for high-performance AI/GPU containers. |
| **DNS SingleReq Attempts+Timeout**| `/etc/resolv.conf` Driver| Triple-option decorator: `single-request`, `attempts:N`, and `timeout:M` in one formatted line. |
| **Log Sliding Window Rate Limiter**| Log Surge Flood Limiter| Suppresses logging spikes and crash loops using in-memory sliding window rate limits (`minictl logs --rate-limit`). |
| **Image Size Footprint Estimator**| Download & Disk Estimator| Projects total image download volume and expanded container rootfs disk footprints. |
| **HugeTLB Events & Allocation**| Cgroup v2 `hugetlb.*`  | Reads current allocated hugepage memory bytes and max allocation failure counters. |
| **DNS NoTLD Attempts+Timeout**| `/etc/resolv.conf` Driver| Triple-option decorator: `no-tld-query`, `attempts:N`, and `timeout:M` in one formatted line. |
| **Log Template Miner**  | Drain Template Clusterer| Clusters unstructured logs and extracts recurring templates with `<*>` parameterization (`minictl logs --templates`). |
| **Layer Uncompressed & DiffID Auditor**| Layer Chain Correlator| Maps compressed manifest layer digests to uncompressed rootfs diff_ids and calculates volume savings. |
| **Local Zswap Max Events**| Cgroup v2 `memory.events.local`| Reads non-hierarchical local container zswap max allocation rejections (Linux 6.8+). |
| **DNS NoReload Attempts+Timeout**| `/etc/resolv.conf` Driver| Triple-option decorator: `no-reload`, `attempts:N`, and `timeout:M` in one formatted line. |
| **Log Fuzzy Search Matcher**| Levenshtein Distance Matcher| Finds approximate query matches in log streams for typos and corrupt text (`minictl logs --fuzzy`). |
| **Multi-Platform Index Matcher**| OCI Manifest List Resolver| Selects matching image manifest descriptor for host target `os/arch/variant` from index. |
| **Local Zswap Writeback Events**| Cgroup v2 `memory.events.local`| Reads non-hierarchical local container zswap writeback evictions (Linux 6.8+). |
| **DNS EDNS0 Attempts+Timeout**| `/etc/resolv.conf` Driver| Triple-option decorator: `edns0`, `attempts:N`, and `timeout:M` in one formatted line. |
| **Log URL Sanitizer**   | URL Credentials Stripper| Strips Basic-Auth passwords & masks sensitive query parameters in log URLs (`minictl logs --sanitize-urls`). |
| **Layer MediaTypes Auditor**| Image Compression Auditor| Detects gzip, zstd, and tar compression formats across OCI/Docker layer descriptors. |
| **Memory Breakdown & Slab Ratio**| Cgroup v2 `memory.stat`| Computes Kernel vs User memory breakdown, and slab reclaimable efficiency ratios. |
| **DNS Inet6 Attempts+Timeout**| `/etc/resolv.conf` Driver| Triple-option decorator: `inet6`, `attempts:N`, and `timeout:M` in one formatted line. |
| **Log IP Address Masker**| PII & IP Anonymizer    | Anonymizes IPv4 and IPv6 addresses into `192.168.1.xxx` or `[IP_MASKED]` (`minictl logs --mask-ip`). |
| **Artifact Type Auditor**| OCI 1.1 Artifact Inspector| Inspects `artifactType` and `subject` descriptors for Cosign signatures and SBOM attestations. |
| **Zswap Max Events Counter**| Cgroup v2 `memory.events`| Reads `zswap_max N` rejected page compression counts from memory events (Linux 6.8+). |
| **DNS Use-VC Attempts+Timeout**| `/etc/resolv.conf` Driver| Triple-option decorator: `use-vc`, `attempts:N`, and `timeout:M` in one formatted line. |
| **Log Secret Redactor** | Credentials & PII Masker| Masks Bearer tokens, passwords, JWTs, emails, and custom secret patterns (`minictl logs --redact-secrets`). |
| **Manifest Annotations Auditor**| OCI Annotations Inspector| Extracts `org.opencontainers.image.*` standard metadata (title, version, vendor, license). |
| **I/O Stat Bandwidth Reader**| Cgroup v2 `io.stat`| Aggregates per-device read/write throughput bytes, IOPS, and discard statistics. |
| **DNS Trust-AD Attempts+Timeout**| `/etc/resolv.conf` Driver| Triple-option decorator: `trust-ad`, `attempts:N`, and `timeout:M` in one formatted line. |
| **Log Context Extractor**| Context Lines Grep (-A -B -C)| Extracts matches with configurable preceding/following context lines & separators (`--`). |
| **Manifest Schema Auditor**| OCI/Docker Manifest Auditor| Inspects `schemaVersion` & `mediaType` format (OCI v1 vs Docker Manifest v2/List). |
| **CPU Burst/Throttle Metrics**| Cgroup v2 `cpu.stat`| Calculates CPU throttling ratio and burst utilization ratio for autoscaling signals. |
| **DNS SingleReq-Recheck Timeout**| `/etc/resolv.conf` Driver| Combines `options single-request-recheck` and `options timeout:N` into formatted lines. |
| **Log Line Number Annotator**| Sequential Line Numbering| Prepends padded sequential line numbers for debugging (`minictl logs --line-numbers`). |
| **Author & Maintainer Auditor**| Image Metadata Inspector| Extracts `author` field and `maintainer` label from Image Config for provenance tracking. |
| **I/O PSI Pressure**    | Cgroup v2 `io.pressure`| Reads total I/O pressure stall time (µs) from PSI "some" line for disk bottleneck detection. |
| **DNS Inet6 Attempts**  | `/etc/resolv.conf` Driver| Combines `options inet6` (prefer IPv6 AAAA) and `options attempts:N` into formatted lines. |
| **Log Truncate Filter** | Line Length Limiter   | Caps individual log lines to configurable max bytes with `...[truncated]` suffix. |
| **RootFS Layers Auditor**| Layer Diff ID Inspector| Lists rootfs `diff_ids` layer digests, counts, and types from Image Config. |
| **CPU PSI Pressure**    | Cgroup v2 `cpu.pressure`| Reads total CPU pressure stall time (µs) from PSI "some" line for scheduling analysis. |
| **DNS SingleReq-Recheck Attempts**| `/etc/resolv.conf` Driver| Combines `options single-request-recheck` and `options attempts:N` into formatted lines. |
| **Log Dedup Filter**    | Line Deduplication Filter| Suppresses consecutive identical lines and emits `[repeated N times]` summaries. |
| **Build History Auditor**| Dockerfile Layer Inspector| Parses OCI image `history[]` entries (Dockerfile commands) with LAYER/META markers. |
| **Memory PSI Pressure** | Cgroup v2 `memory.pressure`| Reads total memory pressure stall time (µs) from PSI "some" line for throttle detection. |
| **DNS Rotate+Attempts+NDots**| `/etc/resolv.conf` Driver| Triple-option decorator: `rotate`, `attempts:N`, and `ndots:M` in one formatted line. |
| **Log CSV/TSV Exporter**| RFC 4180 Table Exporter| Exports container logs as structured CSV/TSV records with header rows (`minictl logs --csv`). |
| **Image Created-At Auditor**| Creation Timestamp Auditor| Inspects `created` timestamp and computes human-readable relative age (e.g. "3 days ago"). |
| **Memory OOM Group Controller**| Cgroup v2 `memory.oom.group`| Reads/sets group OOM kill policy: kill all cgroup processes on OOM instead of individual. |
| **DNS Use-VC Attempts**| `/etc/resolv.conf` Driver| Combines `options use-vc` (force TCP) and `options attempts:N` into formatted lines. |
| **Log Entropy Filter**  | Shannon Entropy Analyzer| Detects high-entropy encrypted blobs, tokens, and secret leakage (`minictl logs --entropy-min`). |
| **OS Compatibility Auditor**| Kernel Features Auditor| Inspects `os`, `os.version`, and `os.features` platform compatibility requirements. |
| **PIDs Events Max Counter**| Cgroup v2 `pids.events`| Reads fork/clone limit failure counts (`max N`) for container fork-bomb monitoring. |
| **DNS Trust-AD Attempts**| `/etc/resolv.conf` Driver| Combines `options trust-ad` and `options attempts:N` into formatted lines. |
| **Log Multi-line Regex Splitter**| Stream Record Assembler| Groups fragmented lines and stack traces by message start pattern (`minictl logs --multiline-regex`). |
| **Architecture Variant Auditor**| CPU Variant Auditor   | Inspects `architecture` and `variant` (e.g. `arm64/v8`, `arm/v7`) from Image Config. |
| **Memory NUMA Stat Reader**| Cgroup v2 `memory.numa_stat`| Reads per-NUMA-node memory distribution across anon, file, and kernel pages. |
| **DNS NoTLD Attempts**  | `/etc/resolv.conf` Driver| Combines `options no-tld-query` and `options attempts:N` into formatted lines. |
| **Log Time Range Filter**| Timestamp Window Filter| Filters container logs within `Since` and `Until` time boundaries (`minictl logs --since/--until`). |
| **Healthcheck StartInterval**| Probe Interval Auditor | Inspects `Healthcheck.StartInterval` custom probe interval during container startup. |
| **Zswap Writeback Events**| Cgroup v2 `memory.events`| Reads exact count of compressed zswap pages evicted to disk swap (`zswap_writeback N`). |
| **DNS NoReload Attempts**| `/etc/resolv.conf` Driver| Combines `options no-reload` and `options attempts:N` into formatted lines. |
| **Log Summary Stats Aggregator**| Stream Stats Calculator| Computes total lines, bytes, average length, and severity breakdown (`minictl logs --stats`). |
| **ExposedPorts Auditor**| Port & Protocol Auditor| Inspects and categorizes `config.ExposedPorts` into sorted TCP/UDP port listings. |
| **Zswap Writeback Controller**| Cgroup v2 `memory.zswap.writeback`| Enables or disables writing compressed zswap pool pages back to swap disk (0 or 1). |
| **DNS NoCheck Attempts**| `/etc/resolv.conf` Driver| Combines `options no-check-names` and `options attempts:N` into formatted lines. |
| **Log Column Extractor**| Delimited Field Extractor| Extracts positional columns from delimited log streams (`minictl logs --columns 1,3 --delimiter ","`). |
| **Healthcheck Timing Auditor**| Interval & Timeout Auditor| Inspects `Healthcheck.Interval`, `Timeout`, `StartPeriod`, and `Retries` from Image Config. |
| **Zswap Max Limit Controller**| Cgroup v2 `memory.zswap.max`| Reads and enforces maximum compressed zswap memory byte limits for the container. |
| **DNS EDNS0 Attempts**  | `/etc/resolv.conf` Driver| Combines `options edns0` and `options attempts:N` into formatted lines. |
| **Log Downsampling Filter**| Stream Rate Sampler   | Downsamples high-throughput container logs via interval (1-in-N) or probability (`minictl logs --sample`). |
| **Healthcheck Test Auditor**| Health Command Auditor| Inspects and normalizes `Healthcheck.Test` command slice types (`CMD`, `CMD-SHELL`, `NONE`). |
| **Zswap Usage Reader**   | Cgroup v2 `memory.zswap.current`| Reads exact compressed RAM bytes used by container zswap memory pages. |
| **DNS SingleReq Attempts**| `/etc/resolv.conf` Driver| Combines `options single-request` and `options attempts:N` into formatted lines. |
| **Log Color Highlighter**| Terminal Stream Colorizer| Decorates log stream severity levels and custom keywords with ANSI colors (`minictl logs --color`). |
| **Image Network Auditor**| Network Config Auditor | Inspects `config.NetworkDisabled` and `config.MacAddress` flags from Image Config. |
| **CPU Idle Controller**  | Cgroup v2 `cpu.idle`   | Manages SCHED_IDLE background priority class for low-priority batch containers (0 or 1). |
| **DNS Rotate Attempts**  | `/etc/resolv.conf` Driver| Combines `options rotate` and `options attempts:N` into formatted lines. |
| **Log Severity Filter** | Log Stream Level Filter| Filters container logs based on minimum severity rank (`minictl logs --level warn`). |
| **Image TTY Auditor**   | Terminal Config Auditor| Inspects `config.Tty`, `config.OpenStdin`, and `config.StdinOnce` flags from Image Config. |
| **Local Swap Failcnt Counter**| Cgroup v2 `memory.swap.events.local`| Reads count of local non-hierarchical swap allocation failures (`failcnt N`). |
| **DNS Use-VC Timeout**  | `/etc/resolv.conf` Driver| Combines `options use-vc` and `options timeout:N` into formatted lines. |
| **JSON Log Key Filter** | Structured Log Projector| Filters and projects specific JSON keys from structured log streams (`minictl logs --json-keys`). |
| **Image Attach Auditor**| Stdio Attachment Auditor| Inspects `config.AttachStdin`, `config.AttachStdout`, and `config.AttachStderr` flags from Image Config. |
| **Local Swap Max Counter**| Cgroup v2 `memory.swap.events.local`| Reads count of local non-hierarchical swap hard limit hits (`max N`). |
| **DNS Trust-AD Timeout**| `/etc/resolv.conf` Driver| Combines `options trust-ad` and `options timeout:N` into formatted lines. |
| **Log Regex Replacer**  | Stream Pattern Masker | Performs regex substitution and redaction over log output lines (`minictl logs --regex-replace`). |
| **Empty Layer Auditor** | History Layer Auditor | Differentiates metadata-only (`empty_layer: true`) vs data layers in OCI Image Config. |
| **Local Swap High Counter**| Cgroup v2 `memory.swap.events.local`| Reads count of local non-hierarchical swap soft limit throttle events (`high N`). |
| **DNS NoTLD Timeout**   | `/etc/resolv.conf` Driver| Combines `options no-tld-query` and `options timeout:N` into formatted lines. |
| **Log Custom Prefix**  | Log Line Tag Prepend  | Prepends custom identifier prefix string to each log line (`minictl logs --prefix`). |
| **Image Volumes Extractor**| Declared Volumes Auditor| Inspects `config.Volumes` mount point declarations from Image Config JSON. |
| **Swap Failcnt Counter**| Cgroup v2 `memory.swap.events`| Reads exact count of swap memory exhaustion encounters (`failcnt N`). |
| **DNS NoReload Timeout**| `/etc/resolv.conf` Driver| Combines `options no-reload` and `options timeout:N` into formatted lines. |
| **Log Field Extractor** | Structured Log Parser | Extracts specific named `key=value` fields from structured log lines (`minictl logs --fields`). |
| **Image Domainname Inspector**| Network Domain Auditor| Inspects `config.Domainname` declared network domain from Image Config JSON. |
| **CPU Weight Reader**   | Cgroup v2 `cpu.weight` | Reads container CPU scheduling weight share (1–10000, default 100). |
| **DNS Ndots Timeout**   | `/etc/resolv.conf` Driver| Combines `options ndots:N` and `options timeout:T` into formatted lines. |
| **Log Max Bytes**      | Byte Payload Limiter  | Truncates log payload at maximum byte threshold with indicator (`minictl logs --max-bytes`). |
| **Image Shell Auditor**| Default Shell Auditor | Inspects default execution shell (`config.Shell`) from Image Config JSON. |
| **Swap Max Counter**   | Cgroup v2 `memory.swap.events`| Reads exact count of swap hard limit enforcement events (`max N`). |
| **DNS NoCheck Timeout**| `/etc/resolv.conf` Driver| Combines `options no-check-names` and `options timeout:N` into formatted lines. |
| **Log Invert Grep**    | Inverted Pattern Filter| Filters and displays log lines that do NOT match specified pattern (`minictl logs --invert-grep`). |
| **Image Labels Auditor**| Metadata Labels Auditor| Inspects and filters container runtime label metadata from Image Config JSON. |
| **Swap High Counter**  | Cgroup v2 `memory.swap.events`| Reads exact count of swap soft limit throttle events (`high N`). |
| **DNS EDNS0 Timeout**  | `/etc/resolv.conf` Driver| Combines `options edns0` and `options timeout:N` into formatted lines. |
| **Log Grep Counter**   | Match Counter         | Counts regex pattern match occurrences across log lines (`minictl logs --grep-count`). |
| **Config Digest Calculator**| Image ID Calculator  | Computes SHA256 canonical digest hash of Image Config JSON. |
| **Local Memory Min Counter**| Cgroup v2 `memory.events.local`| Reads exact count of local non-hierarchical cgroup hard eviction protections (`min N`). |
| **DNS Inet6 Timeout**  | `/etc/resolv.conf` Driver| Combines `options inet6` and `options timeout:N` into formatted lines. |
| **Log Index Slice Extractor**| Log Range Filter    | Returns a slice range of log lines from index A to index B (`minictl logs --slice`). |
| **ContainerConfig Auditor**| Build Container Auditor| Inspects `container_config` metadata struct from Image Config JSON. |
| **Local Memory Low Counter**| Cgroup v2 `memory.events.local`| Reads exact count of local non-hierarchical cgroup reclaim protections (`low N`). |
| **DNS SingleReq Timeout**| `/etc/resolv.conf` Driver| Combines `options single-request-reopen` and `options timeout:N` flags. |
| **Log Head Truncator** | Log Head Filter       | Outputs only the first N lines of container stdio logs (`minictl logs --head`). |
| **RootFS DiffIDs Extractor**| Layer DiffID Auditor | Extracts all layer diffIDs from `rootfs.diff_ids` array in Image Config JSON. |
| **Local Memory High Counter**| Cgroup v2 `memory.events.local`| Reads exact count of local non-hierarchical cgroup soft limit throttles (`high N`). |
| **DNS Rotate Timeout** | `/etc/resolv.conf` Driver| Combines both `options rotate` and `options timeout:N` into formatted lines. |
| **Log Deduplicator**   | Line Repeat Summarizer| Merges consecutive identical log output lines (`minictl logs --dedup`). |
| **OnBuild Inspector**  | Base Image Trigger    | Inspects `OnBuild` trigger instructions from Image Config JSON. |
| **Local OOM Counter**  | Cgroup v2 `memory.events.local`| Reads exact count of local non-hierarchical cgroup OOM encounters (`oom N`). |
| **DNS Attempt Timeout**| `/etc/resolv.conf` Driver| Combines both `options attempts:N` and `options timeout:N` into formatted lines. |
| **Log Rate Limiter**   | Emission Throttler    | Limits maximum log lines output per second (`minictl logs --rate-limit`). |
| **Healthcheck Inspector**| Healthcheck Auditor | Inspects embedded Dockerfile `HEALTHCHECK` parameters from Image Config JSON. |
| **Local OOM Kill Counter**| Cgroup v2 `memory.events.local`| Reads exact count of non-hierarchical cgroup OOM kills (`oom_kill N`). |
| **DNS Debug Option**   | `/etc/resolv.conf` Driver| Injects `options debug` resolver flags into container `/etc/resolv.conf`. |
| **Log Multiline Aggregator**| Stack Trace Aggregator| Merges indented Java/Python exception stack traces into single events (`minictl logs --multiline`). |
| **Stop Signal Inspector**| Graceful Shutdown Auditor| Inspects `stopSignal` (`SIGTERM`, `SIGQUIT`) from Image Config JSON. |
| **Memory Min Counter** | Cgroup v2 `memory.events`| Reads exact count of hard memory page eviction protection events (`min N`). |
| **DNS ndots Option**    | `/etc/resolv.conf` Driver| Injects `options ndots:N` search threshold flags into container. |
| **Log Mask Filter**    | Credential Redactor   | Masks sensitive tokens, API keys, and passwords with `[REDACTED]` (`minictl logs --mask`). |
| **Image User Inspector**| User ID Auditor       | Inspects default execution user UID/GID (`root`, `1000:1000`) from image config. |
| **Memory Low Counter** | Cgroup v2 `memory.events`| Reads exact count of soft memory reclaim protection events (`low N`). |
| **DNS EDNS0 Payload**  | `/etc/resolv.conf` Driver| Injects `options edns0-payload:N` custom UDP payload size limits. |
| **Log ANSI Cleaner**   | Terminal ANSI Stripper | Strips ANSI escape color code sequences from log output (`minictl logs --ansi-clean`). |
| **OS Features Inspector**| OS Feature Auditor   | Inspects `os.features` array from Image Config JSON for feature flags. |
| **Memory High Counter**| Cgroup v2 `memory.events`| Reads exact count of soft memory limit throttle events (`high N`). |
| **DNS UseVC Fallback**  | `/etc/resolv.conf` Driver| Injects `options use-vc` TCP Virtual Circuit fallback flags. |
| **Syslog RFC5424 Formatter**| Syslog Standard Logger| Formats log lines into Syslog RFC5424 compliant string format (`minictl logs --syslog-fmt`). |
| **OS Version Inspector**| Kernel Version Auditor| Inspects `os.version` string from Image Config JSON for OS sub-version validation. |
| **Memory OOM Counter** | Cgroup v2 `memory.events`| Reads exact count of OOM condition encounters (`oom N`). |
| **DNS NoReload Option** | `/etc/resolv.conf` Driver| Injects `options no-reload` dynamic resolv.conf dynamic file watching disable flags. |
| **Log JSON Formatter**  | Structured JSON Logger| Encapsulates log lines into structured JSON objects (`minictl logs --json`). |
| **Variant Inspector**  | CPU Variant Auditor   | Inspects ARM CPU architecture variant strings (`v7`/`v8`) from config JSON. |
| **OOM Kill Counter**   | Cgroup v2 `memory.events`| Reads exact count of process OOM kill occurrences (`oom_kill N`). |
| **DNS NoTLD Option**    | `/etc/resolv.conf` Driver| Injects `options no-tld-query` top-level domain DNS query blocking options. |
| **Log Layout Renderer** | Log Template Renderer | Formats log entries using custom placeholder templates (`minictl logs --layout`). |
| **History Cmd Cleaner** | Dockerfile Cmd Parser  | Parses and cleans raw `created_by` strings into clean Dockerfile instructions. |
| **Memory Peak Alert**  | Cgroup v2 `memory.peak`| Audits if memory peak usage exceeds target percentage of max limit. |
| **DNS EDNS0 Size**     | `/etc/resolv.conf` Driver| Injects `options edns0-size:N` custom UDP buffer size tuning options. |
| **Log UTC Formatter**   | UTC ISO-8601 Formatter | Converts log timestamps to UTC ISO-8601 format (`minictl logs --utc`). |
| **DiffIDs Verifier**   | OCI Config DiffID Hasher| Verifies uncompressed layer diffIDs against image config descriptors. |
| **Memory Peak Reset**  | Cgroup v2 `memory.reclaim`| Resets Cgroup v2 memory peak usage watermark counters. |
| **DNS UseVC Option**   | `/etc/resolv.conf` Driver| Injects `options use-vc` TCP Virtual Circuit DNS query flags. |
| **Log Expired Pruner**  | Retention Log Cleaner | Deletes rotated log files older than max retention age (`minictl logs --prune`). |
| **ArtifactType Inspector**| OCI Artifact Inspector| Inspects custom artifactType fields (Helm charts, SBOMs, WASM). |
| **Swap High Alert**    | Cgroup v2 `memory.swap.high`| Audits if current swap usage exceeds soft limit threshold. |
| **DNS NoCheck Option**  | `/etc/resolv.conf` Driver| Injects `options no-check-names` name validation disable flags. |
| **Log Gzip Compression**| Gzip Log Archiver     | Automatically compresses rotated old log files into `.gz` archives (`minictl logs --compress`). |
| **Subject Inspector**  | OCI Subject Auditor   | Inspects optional subject descriptor field in OCI manifests for artifact linking. |
| **Swap Peak Reader**   | Cgroup v2 `memory.swap.peak`| Reads highest swap memory usage watermark for container. |
| **DNS inet6 Option**   | `/etc/resolv.conf` Driver| Injects `options inet6` IPv6 resolver flags into container `/etc/resolv.conf`. |
| **Log Multi-File Archive**| Multi-File Log Archiver| Rotates & manages multi-file log archives (`container.log.1`, `container.log.2`). |
| **RootFS Tree Verifier**| Layer Unpack Auditor  | Validates uncompressed rootfs directory structure and permissions. |
| **Swap Events Reader** | Cgroup v2 `memory.swap.events`| Reads swap error event counters (`high`, `max`, `fail`). |
| **DNS Attempts Option**| `/etc/resolv.conf` Driver| Injects `options attempts:N` retry count flags into container. |
| **Log Size Rotation**  | Log Size Truncator    | Rotates/truncates stdio log files exceeding byte size limits (`minictl logs --max-size`). |
| **Manifest Hash Calculator**| SHA-256 Digest Hasher| Computes sha256:hash over raw OCI manifest bytes. |
| **Atomic OOM Group**   | Cgroup v2 `memory.oom.group`| Enforces atomic container process group OOM termination. |
| **DNS Timeout Option** | `/etc/resolv.conf` Driver| Injects `options timeout:N` custom resolution timeout into container. |
| **Log Details Attacher**| Extra Metadata Attacher| Formats log lines with attached environment & container ID details (`minictl logs --details`). |
| **Accept Header Builder**| Manifest Accept Builder| Constructs HTTP Accept header supporting OCI Index, Manifest, and Docker v2 schemas. |
| **Memory Low Protection**| Cgroup v2 `memory.low` | Enforces soft memory protection watermarks before general RAM page reclamation. |
| **DNS Reload Option**  | `/etc/resolv.conf` Driver| Injects `options reload` dynamic resolv.conf re-parsing flag into container. |
| **Log Follow Streamer**| Live Log Stream Tailing| Streams container stdout/stderr output in real-time (`minictl logs -f`). |
| **Descriptor Auditor** | Manifest Size Calculator| Calculates total bytes across image config and layer descriptors. |
| **Memory Min Guarantee**| Cgroup v2 `memory.min` | Enforces hard memory page protection guarantees against RAM eviction. |
| **DNS EDNS0 Option**   | `/etc/resolv.conf` Driver| Injects `options edns0` Extension Mechanisms for DNS into container. |
| **Log Timestamps**     | RFC3339 Timestamp Injector| Formats/prepends RFC3339 nano timestamps to stdio log output lines (`minictl logs -t`). |
| **Platform Filter**    | Target Platform Matcher| Validates image manifest compatibility with host OS & CPU (`linux/amd64`). |
| **Local Memory Events**| Cgroup v2 `memory.events.local`| Reads non-hierarchical cgroup memory event counters. |
| **DNS Trust-AD Flag**  | `/etc/resolv.conf` Driver| Injects `options trust-ad` DNSSEC Authenticated Data flag into container. |
| **Log Grep Matcher**   | Regex Log Matcher      | Filters stdio log lines by regex or keyword (`minictl logs --grep ERROR`). |
| **Annotations Inspector**| OCI Metadata Auditor | Extracts & inspects image manifest annotations (`org.opencontainers...`). |
| **Memory Events Reader**| Cgroup v2 `memory.events`| Reads memory event counters (`low`, `high`, `max`, `oom`, `oom_kill`). |
| **DNS Single-Request** | `/etc/resolv.conf` Driver| Injects `options single-request-reopen` glibc DNS fix into container. |
| **Log Until Filter**   | Upperbound Log Auditor | Filters log entries emitted prior to cutoff time (`minictl logs --until 5m`). |
| **Media-Type Inspector**| OCI Manifest Inspector | Validates OCI manifest media types (`application/vnd.oci...`). |
| **IO Cost QoS Control**| Cgroup v2 `io.cost.qos` | Enforces linear disk I/O cost QoS model rules. |
| **DNS Rotate Option**  | `/etc/resolv.conf` Driver| Injects `options rotate` load balancing directive into container. |
| **Log Since Filter**   | Time-Window Log Auditor| Filters log entries emitted within duration window (`minictl logs --since 10m`). |
| **Compression Detector**| Magic Byte Inspector   | Detects image layer compression format (`gzip`, `zstd`, `raw-tar`). |
| **IO Latency Protection**| Cgroup v2 `io.latency` | Enforces disk I/O target latency protection to prevent I/O starvation. |
| **DNS Order Preference**| `/etc/resolv.conf` Driver| Sorts & orders search domain suffixes in container `/etc/resolv.conf`. |
| **Log Tail Filter**    | Stdio Log Extractor    | Extracts last N lines of log text from container (`minictl logs --tail N`). |
| **OCI Index Resolver** | Multi-Arch Manifest Resolver| Resolves target architecture manifests (`amd64`/`arm64`) from OCI Index JSON. |
| **IO Stat Auditor**    | Cgroup v2 `io.stat`    | Audits per-device disk I/O metrics (`rbytes`, `wbytes`, `rios`, `wios`). |
| **DNS Port Config**    | `/etc/resolv.conf` Driver| Injects custom nameserver entries with port numbers (`10.0.0.1:5353`). |
| **Top Threads Inspector**| `/proc/<pid>/task` Auditor| Inspects thread-level tasks & execution details (`minictl top --threads`). |
| **Schema v2 Inspector**| OCI Manifest Auditor   | Validates OCI / Docker Image Manifest Schema v2 JSON structures. |
| **CPU Stat Auditor**  | Cgroup v2 `cpu.stat`   | Audits CPU usage and throttling counters (`usage_usec`, `nr_throttled`). |
| **DNS Loopback Config**| `/etc/resolv.conf` Driver| Injects `nameserver 127.0.0.53` systemd-resolved entry into container. |
| **Stats Snapshot**    | Telemetry Collector    | Collects single-shot resource stats snapshot (`minictl stats --no-stream`). |
| **Layer Digest Matcher**| SHA-256 Digest Searcher | Finds image records containing matching layer blob digests (`image search --digest`). |
| **Memory PSI Inspector**| Cgroup v2 `memory.pressure`| Audits memory stall pressure metrics (`some`/`full`) for container. |
| **DNS Fallback Config**| Upstream Nameserver Injector| Formats fallback upstream nameserver entries (`8.8.8.8`) into `/etc/resolv.conf`. |
| **Signal Broadcaster**| OS Signal Emitter       | Sends custom OS signals (`SIGHUP`, `SIGUSR1`) to container PID 1 (`minictl kill -s`). |
| **Image Tag Alias**   | Image Tag Linker        | Creates new tag aliases for existing image records (`minictl image tag`). |
| **Swap High Limit**   | Cgroup v2 `memory.swap.high`| Enforces soft swap limit to trigger background page compacting. |
| **DNS Options Config**| `/etc/resolv.conf` Driver| Injects custom resolver options (`options timeout:2...`) into container. |
| **Container Wait**    | Exit Code Auditor      | Blocks until container terminates and returns exit code (`minictl wait`). |
| **Image Layer Size**   | Disk Usage Calculator  | Recursively calculates total uncompressed rootfs disk usage (`minictl image size`). |
| **Swap Max Limit**    | Cgroup v2 `memory.swap.max`| Enforces hard swap limit to prevent swap abuse. |
| **DNS Domain Directive**| `/etc/resolv.conf` Driver| Injects custom `domain` directive into container `/etc/resolv.conf`. |
| **Detached Exec**     | Namespace Background Exec| Runs background sub-processes inside container namespaces (`minictl exec -d`). |
| **Image Layer History**| Build History Auditor  | Audits layer build steps, sizes, and timestamps (`minictl image history`). |
| **I/O Weight Control**| Cgroup v2 `io.weight`  | Enforces block I/O fair share priority (1-10000) for containers. |
| **DNS Search Suffix** | `/etc/resolv.conf` Driver| Injects custom search domain suffixes (`search default.svc...`) into container. |
| **Atomic Freezer**    | Cgroup v2 `cgroup.freeze`| Freezes/thaws container execution without signal leakage (`minictl pause/unpause`). |
| **Image Layer Diff**  | Tree Diff Auditor      | Compares file modifications between two image tags (`minictl image diff`). |
| **Memory High Limit** | Cgroup v2 `memory.high`| Enforces soft memory limit throttling prior to hard OOM. |
| **Network MAC Gen**   | Hardware Address Binder| Generates deterministic MAC hardware addresses (`02:42:...`) for veth interfaces. |
| **Container Commit**   | Upperdir Layer Packager| Packages container rootfs changes into a new tagged image (`minictl commit`). |
| **Orphan Layer Pruner**| Image Store GC Engine  | Scans and deletes unreferenced image layers (`minictl image prune`). |
| **Memory Reclaim**    | Cgroup v2 `memory.reclaim`| Triggers forced memory page compacting for idle containers. |
| **Network MTU Config**| Link MTU Configurator  | Adjusts MTU byte size for container veth and bridge interfaces. |
| **Live Resource Mutator**| Cgroup v2 Limit Modifier| Dynamically updates memory limits & CPU quotas for running containers (`minictl update`). |
| **Layer Digest Verifier**| SHA-256 Digest Auditor | Computes & verifies layer checksum integrity against OCI digest hashes (`image verify`). |
| **CPU Bandwidth Control**| Cgroup v2 `cpu.weight` | Enforces Cgroup v2 `cpu.weight` fair share priority & `cpu.max` period/quota. |
| **Dual-Stack IPv6 Net**| IPv6 Subnet Allocator  | Allocates & configures dual-stack IPv6 addresses (`2001:db8::/64`) for container veths. |
| **Container Renamer**  | Alias & DNS Synchronizer | Safely updates container name alias & internal DNS host registry (`minictl rename`). |
| **Tarball Importer**   | Raw RootFS Tar Unpacker  | Imports standalone raw tarballs into imagestore with named tags (`minictl import`). |
| **Memory Alert Handler**| Cgroup v2 `memory.high` | Evaluates soft memory limit overload alerts for container processes. |
| **DNS Resolver Config**| `/etc/resolv.conf` Driver| Injects custom nameserver IPs and search domain suffixes into container. |
| **Process Inspector** | `/proc/<pid>/task`     | Lists active processes and threads inside running container (`minictl top`). |
| **Container Exporter** | RootFS Tar Packager   | Streams container rootfs into Docker-compatible tarball (`minictl export`). |
| **PIDs Limit Controller**| Cgroup v2 `pids.max` | Enforces max process count limit to prevent fork bomb attacks. |
| **Network Traffic Meter**| VETH Netlink Counter  | Measures RX/TX bytes and packet throughput for veth interfaces. |
| **Time-Based GC**     | Windowed Purge Engine  | Reclaims stopped containers created prior to duration cutoff (`system prune --until`). |
| **AES-256 Layer Crypto**| Symmetric Cryptography| Encrypts and decrypts rootfs image layers via AES-256-GCM (`image encrypt/decrypt`). |
| **Misc Device Limit**  | Cgroup v2 `misc.max`   | Controls misc hardware device allocations in Cgroup v2. |
| **Readiness Prober**  | Active Network Socket | Conducts active TCP and HTTP readiness probes against container ports. |
| **FS Checkpoint/Restore**| Instant Snapshot Engine| Saves & restores container OverlayFS upperdir snapshots (`minictl snapshot`). |
| **Layer Deduplication**| SHA-256 Hardlink Linker| Deduplicates identical image layers across images via hardlinks. |
| **HugePages Controller**| Cgroup v2 `hugetlb`    | Controls 2MB & 1GB HugePages memory quotas (`--hugetlb-limit`). |
| **JSON Diagnostic Info**| Engine Telemetry Audit | Generates engine telemetry report in JSON format (`minictl info --json`). |
| **Local Registry Mirror**| Layer Proxy Cacher   | Runs HTTP proxy server caching image layer blobs locally (`minictl mirror`). |
| **Syslog Driver**     | RFC5424 Log Streamer  | Formats container stdio logs & events into RFC5424 syslog streams. |
| **OOM Event Monitor** | Cgroup v2 `memory.events`| Monitors and records `oom` and `oom_kill` counters in container inspect state. |
| **Engine Benchmarker**| Performance Tester    | Measures container startup latency (ms) & state read/write throughput (`minictl bench`). |
| **Plugin Architecture**| Extension Manifest Driver | Discovers & loads volume, log, and network driver extensions (`minictl plugin`). |
| **RootFS Security Scan**| Static Audit Inspector| Scans rootfs for SUID binaries, world-writable directories, and SSH keys (`minictl scan`). |
| **PSI Monitor**       | Cgroup v2 Pressure Monitor| Reads `/sys/fs/cgroup/.../memory.pressure` & `cpu.pressure` contention metrics. |
| **VXLAN Overlay Net** | Multi-Node Tunnel Driver| Configures Linux VXLAN interfaces (UDP port 4789) for inter-host container networking. |
| **Memory Dump**      | Process State Snapshot| Dumps container process memory maps (`proc/maps`) & state to `.dump` file (`minictl dump`). |
| **HMAC Image Sign**  | Provenance Cryptography| Signs & verifies rootfs layers with HMAC-SHA256 signatures (`minictl image sign/verify`). |
| **Live Telemetry**   | Performance Dashboard | Streams live container resource consumption statistics (`minictl stats`). |
| **SubUID/SubGID Map**| User Namespace Remap  | Allocates unprivileged UID/GID mapping ranges for rootless execution (`--userns-remap`). |
| **Signal Propagation**| Unix Signal Dispatcher | Sends specific Unix signals (`SIGKILL`, `SIGTERM`, `SIGHUP`, etc.) (`minictl kill -s`). |
| **Secret Env Masking**| Credential Obscuration | Obscures sensitive environment variables (`PASS`, `KEY`, `TOKEN`) in inspect output. |
| **CPU/NUMA Pinning**  | Cgroup v2 `cpuset`     | Pins container execution to specific CPU cores & NUMA memory nodes (`--cpuset-cpus`). |
| **Kernel Diagnostics**| Capability Self-Checker| Performs self-checks of Linux kernel prerequisites & namespaces (`minictl check`). |
| **Image History**   | Layer Build Inspector | Audits image build steps, creation time, size, and commands (`minictl history`). |
| **Event Webhooks**  | HTTP Event Dispatcher | Posts JSON container lifecycle events to HTTP webhook endpoints (`--webhook`). |
| **Swap Memory Control**| Cgroup v2 `memory.swap`| Controls soft memory (`memory.high`) & swap thresholds (`--memory-swap`). |
| **Multi-Arch OCI Index**| OCI Index Builder | Generates multi-architecture OCI Image Index manifests (`amd64`, `arm64`). |
| **Auto-Restart Supervisor**| Exit State Monitor | Monitors container crashes & automatically respawns init process (`--restart`). |
| **Disk I/O Throttling**| Cgroup v2 `io.max` | Throttles device read/write throughput (BPS) & IOPS (`--device-read-bps`). |
| **Stream Attacher**   | Terminal Stream Piping | Attaches stdio stream to running container console (`minictl attach`). |
| **System Diagnostics**| Engine Disk & Prune Engine | Computes disk space usage breakdown (`system df`) & one-stop reclamation (`system prune -a`). |
| **Dynamic IPAM**     | Subnet Lease Pool Allocator| Dynamically allocates & recycles container IP addresses across bridge networks. |
| **Filesystem Diff** | OverlayFS Upper Inspector | Audits filesystem mutations (`A` added, `C` changed, `D` deleted) (`minictl diff`). |
| **Prometheus Exporter**| Metrics Endpoint Exporter| Exposes `/v1/metrics` REST endpoint formatted for Prometheus metrics scrapers. |
| **OCI Image Push**   | Tarball & Manifest Packager| Packages rootfs layers and OCI manifest for image registry upload (`minictl push`). |
| **REST API Daemon**  | HTTP over Unix/TCP Socket | Runs background engine server listening on `/tmp/minictl.sock` or `:2375` (`minictl daemon`). |
| **Interactive PTY**   | Master/Slave PTY Allocator | Provides raw terminal mode pass-through & stream piping for interactive sessions (`-it`). |
| **Health Supervisor**| Periodic Background Evaluator | Background worker evaluating container health command status (`healthy`/`unhealthy`). |
| **Dockerfile Builder**| AST Parser & Layer Executor| Builds container images step-by-step from Dockerfile (`minictl build`). |
| **Image Tag & Store**| Local Manifest Index | Manages local image repositories, tags, IDs, and disk sizes (`minictl images/tag/rmi`). |
| **Named Volumes**   | Persistent Data Driver | Manages named data volumes and automatic volume binding (`minictl volume`). |
| **Container DNS**    | Network Hosts Injector | Auto-registers container hostnames & IPs for service discovery inside bridge networks. |
| **Container Export** | `tar.Writer` | Packages container rootfs into `.tar` / `.tar.gz` (`minictl export`). |
| **Container Commit** | Metadata snapshot | Commits container filesystem as new local image (`minictl commit`). |
| **Container Top** | `/proc/<pid>/task` parsing | Displays tasks/threads running inside the container (`minictl top`). |

---

## 🚀 Requirements & Environment Setup

Linux kernel features (namespaces, cgroups, pivot_root) require a Linux kernel environment.

### Option A: WSL 2 (Windows 10 / 11) — Recommended
1. Open PowerShell as Administrator:
   ```powershell
   wsl --install
   ```
2. Restart your computer and open Ubuntu from the Start menu.
3. Install Go:
   ```bash
   sudo apt update && sudo apt install -y golang-go
   ```

### Option B: Native Linux / Cloud VM / VirtualBox
Install Go (v1.21+):
```bash
sudo apt update && sudo apt install -y golang-go make wget
```

---

## 🛠️ Build & Usage

### 1. Build `minictl`
```bash
make build
# or manually:
go build -o build/minictl ./cmd/minictl
```

### 2. Prepare RootFS Image
Pull an Alpine Linux image directly from Docker Hub:
```bash
./build/minictl pull alpine:3.19 ./rootfs
```

Or download and unpack an Alpine Linux minirootfs manually:
```bash
make rootfs
# or manually:
./build/minictl unpack alpine-minirootfs-3.19.0-x86_64.tar.gz ./rootfs
```

---

## 💻 CLI Command Reference

### `minictl update` — Dynamic Resource Updates
```bash
# Dynamically change memory limit to 128MB and CPU quota to 1.5 CPUs on running container
sudo ./build/minictl update --memory 128m --cpus 1.5 <container-id>
```

### `minictl events` — Real-time Lifecycle Audit Stream
```bash
# Stream real-time container events (create, start, exec, pause, stop, die, destroy)
./build/minictl events -f
```

### `minictl cp` — Bidirectional Container File Transfer
```bash
# Copy file from host into container
./build/minictl cp /tmp/config.json <container-id>:/etc/config.json

# Copy file from container out to host
./build/minictl cp <container-id>:/var/log/app.log /tmp/app.log
```

### `minictl compose up` — Multi-container Orchestration
```bash
sudo ./build/minictl compose up -f compose.json
```

### `minictl pull` — Download Images from Docker Hub
```bash
./build/minictl pull alpine:3.19 ./rootfs-alpine
./build/minictl pull ubuntu:22.04 ./rootfs-ubuntu
```

### `minictl run` — Launch a Container
```bash
# Basic shell with OverlayFS layer isolation, environment variables & working directory
sudo ./build/minictl run --overlay -w /app -e MODE=production ./rootfs /bin/sh

# Resource limits (Memory, CPU quota 50%, PIDs limit) & custom hostname
sudo ./build/minictl run \
  --hostname demo-box \
  --memory 64m \
  --cpus 0.5 \
  --pids-limit 32 \
  ./rootfs /bin/sh

# Security: Drop CAP_SYS_ADMIN & CAP_NET_RAW capabilities + Seccomp BPF filter
sudo ./build/minictl run \
  --cap-drop CAP_SYS_ADMIN \
  --cap-drop CAP_NET_RAW \
  --seccomp \
  ./rootfs /bin/sh

# Bind volumes (-v host:container[:ro])
sudo ./build/minictl run \
  -v /tmp/data:/data \
  -v /etc/hosts:/etc/hosts:ro \
  ./rootfs /bin/sh

# Bridge networking & Port mapping (-p hostPort:containerPort)
sudo ./build/minictl run --bridge -p 8080:80 ./rootfs /bin/nc -l -p 80
```

### Management Commands
```bash
# List containers
./build/minictl ps [-a]

# Inspect JSON metadata
./build/minictl inspect <id>

# View process list inside container
./build/minictl top <id>

# Manage custom bridge networks
sudo ./build/minictl network create demo-net 172.28.0.1/24
./build/minictl network ls
sudo ./build/minictl network rm demo-net

# Execute command inside running container
sudo ./build/minictl exec <id> /bin/sh

# Export container rootfs to a tarball archive
./build/minictl export <id> container-backup.tar.gz

# Commit container to new local image
./build/minictl commit <id> my-custom-app:v1

# View live resource metrics (cgroup v2)
./build/minictl stats [id]

# View container logs
./build/minictl logs [-f] [--tail n] <id>

# Pause / Unpause container processes (cgroup freezer)
sudo ./build/minictl pause <id>
sudo ./build/minictl unpause <id>

# Gracefully stop container (SIGTERM -> SIGKILL)
sudo ./build/minictl stop -t 5 <id>

# Force kill container (SIGKILL)
./build/minictl kill <id>

# Remove container or prune stopped containers
./build/minictl rm <id>
./build/minictl prune
```

---

## 🧪 Testing

Run all unit tests across state store, image unpacking, compose parsing, health check evaluation, container cp transfer, events stream, OCI image ref parsing, export round-tripping, safe-path traversal protection, port parsing, and platform stubs:
```bash
make test
# or manually:
go test -v ./...
```

---

## 🔬 Learning Highlights & Debugging

To observe raw syscall execution and namespace creation in real-time, set `MINICONTAINER_DEBUG=1`:
```bash
sudo MINICONTAINER_DEBUG=1 ./build/minictl run --overlay --cap-drop CAP_SYS_ADMIN ./rootfs /bin/sh
```
Output log trace:
```text
[parent] spawning child with new namespaces
[parent] child started, PID=14205
[cgroup] using cgroup v2 (unified hierarchy)
[cgroup v2] cpu.max = 50000 100000
[parent] veth: host side veth-h14205 ready (172.20.0.1/24)
[init] running inside new namespaces
[init] received sync signal from parent
[init] mount namespace propagation set to private
[init] overlayfs mounted (./rootfs -> /tmp/minicontainer-overlay-1234/merged)
[init] hostname set to "minicontainer"
[init] /proc mounted
[init] dropped capability CAP_SYS_ADMIN (21)
[init] pivot_root complete
[init] exec: /bin/sh []
```
