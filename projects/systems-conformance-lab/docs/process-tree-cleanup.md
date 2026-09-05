# Post-exit descendant pipe cleanup

`run_process` treats target code as untrusted. A root target may spawn descendants that inherit stdin, stdout, or stderr and then exit before those descendants. Waiting indefinitely for the pipe reader/writer threads in that state would let a target escape the runner timeout after its root process has already terminated.

After the root exits, the runner therefore allows only a short bounded drain interval. If any stdio worker is still alive, the run is classified as an infrastructure failure with `ProcessTreeLeak`, the process-tree cleanup path is invoked again even though the root has exited, and cleanup joins are bounded as well. On POSIX the target is launched in its own session and the process group is killed; on Windows the existing `taskkill /T /F` tree cleanup is attempted. The runner never uses shell interpolation.

This classification is intentionally distinct from a product mismatch: a descendant retaining harness-owned stdio is an execution-environment failure, not target output that an oracle should compare.

The focused regression launches a real Python target that spawns a descendant inheriting its pipes, exits the root immediately, and verifies that `run_process` returns promptly with `ProcessTreeLeak` instead of blocking until the descendant exits.
