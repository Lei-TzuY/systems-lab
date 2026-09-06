//go:build linux

package events

import (
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"path/filepath"

	"golang.org/x/sys/unix"
)

const maxEventLogBytes int64 = 16 << 20
const retainedEventLogGenerations = 2

type lockedEventLogFile struct {
	*os.File
	lockFile *os.File
}

func (f *lockedEventLogFile) Close() error {
	if f == nil {
		return nil
	}
	var fileErr error
	if f.File != nil {
		fileErr = f.File.Close()
		f.File = nil
	}
	if f.lockFile == nil {
		return fileErr
	}
	unlockErr := unix.Flock(int(f.lockFile.Fd()), unix.LOCK_UN)
	if unlockErr != nil {
		unlockErr = fmt.Errorf("unlock event log writer: %w", unlockErr)
	}
	lockCloseErr := f.lockFile.Close()
	f.lockFile = nil
	return errors.Join(fileErr, unlockErr, lockCloseErr)
}

func verifyHeldEventPath(file *os.File, path, kind string) error {
	held, err := file.Stat()
	if err != nil {
		return fmt.Errorf("inspect held %s: %w", kind, err)
	}
	current, err := os.Lstat(path)
	if err != nil {
		return fmt.Errorf("verify %s path identity: %w", kind, err)
	}
	if current.Mode()&os.ModeSymlink != 0 || !current.Mode().IsRegular() || !os.SameFile(held, current) {
		return fmt.Errorf("%s path changed while held", kind)
	}
	return nil
}

func verifyLockedEventPath(file *os.File, path string) error {
	return verifyHeldEventPath(file, path, "event lock")
}

func (f *lockedEventLogFile) verifyDataPath() error {
	if f == nil || f.File == nil {
		return fmt.Errorf("event log writer is closed")
	}
	return verifyHeldEventPath(f.File, f.File.Name(), "event log")
}

func (f *lockedEventLogFile) Write(p []byte) (int, error) {
	if err := f.verifyDataPath(); err != nil {
		return 0, fmt.Errorf("verify event log before write: %w", err)
	}
	n, err := f.File.Write(p)
	if err != nil {
		return n, err
	}
	if err := f.verifyDataPath(); err != nil {
		return n, fmt.Errorf("verify event log after write: %w", err)
	}
	return n, nil
}

func (f *lockedEventLogFile) Sync() error {
	if f == nil || f.File == nil {
		return fmt.Errorf("event log writer is closed")
	}
	if err := f.File.Sync(); err != nil {
		return err
	}
	if err := f.verifyDataPath(); err != nil {
		return fmt.Errorf("verify event log after sync: %w", err)
	}
	return nil
}

func syncEventLogDirectory(path string) error {
	dirPath := filepath.Dir(path)
	dfd, err := unix.Open(dirPath, unix.O_RDONLY|unix.O_DIRECTORY|unix.O_CLOEXEC|unix.O_NOFOLLOW, 0)
	if err != nil {
		return fmt.Errorf("open event log directory for sync: %w", err)
	}
	if err := unix.Fsync(dfd); err != nil {
		_ = unix.Close(dfd)
		return fmt.Errorf("sync event log directory: %w", err)
	}
	if err := unix.Close(dfd); err != nil {
		return fmt.Errorf("close event log directory after sync: %w", err)
	}
	return nil
}

// repairEventLogTail runs while the cross-process writer lock is held. A crash
// can leave the active generation ending in a partial JSON record. Appending the
// next event after that fragment would permanently merge two records into one
// corrupt line, so repair the EOF before retention/rotation decisions are made.
// A complete, valid event that only lost its terminating newline is salvaged;
// complete JSON that violates the event schema fails closed instead of erasing
// evidence that may represent tampering rather than a torn write.
func repairEventLogTail(file *os.File, path string) error {
	if err := verifyHeldEventPath(file, path, "event log tail repair"); err != nil {
		return err
	}
	info, err := file.Stat()
	if err != nil {
		return fmt.Errorf("stat event log for tail repair: %w", err)
	}
	if info.Size() == 0 {
		return nil
	}

	var last [1]byte
	if _, err := file.ReadAt(last[:], info.Size()-1); err != nil {
		return fmt.Errorf("read event log tail marker: %w", err)
	}
	if last[0] == '\n' {
		return nil
	}

	start := info.Size() - int64(maxEventRecordBytes+1)
	if start < 0 {
		start = 0
	}
	buf := make([]byte, info.Size()-start)
	if _, err := file.ReadAt(buf, start); err != nil {
		return fmt.Errorf("read event log tail: %w", err)
	}

	lastNewline := bytes.LastIndexByte(buf, '\n')
	if lastNewline < 0 && start > 0 {
		return fmt.Errorf("unterminated event record exceeds maximum size of %d bytes", maxEventRecordBytes)
	}
	tailStart := start
	if lastNewline >= 0 {
		tailStart = start + int64(lastNewline) + 1
		buf = buf[lastNewline+1:]
	}

	if json.Valid(buf) {
		if _, err := decodeEventRecord(buf); err != nil {
			return fmt.Errorf("validate unterminated event record: %w", err)
		}
		if _, err := file.WriteAt([]byte{'\n'}, info.Size()); err != nil {
			return fmt.Errorf("terminate complete event record: %w", err)
		}
	} else {
		if err := file.Truncate(tailStart); err != nil {
			return fmt.Errorf("truncate torn event record: %w", err)
		}
	}
	if err := file.Sync(); err != nil {
		return fmt.Errorf("sync repaired event log tail: %w", err)
	}
	if err := verifyHeldEventPath(file, path, "repaired event log"); err != nil {
		return err
	}
	return nil
}

func rotateRetainedEventLog(path string) error {
	retained := path + ".1"
	older := path + ".2"
	file, err := openEventLogForRead(retained)
	if err != nil {
		if errors.Is(err, os.ErrNotExist) {
			return nil
		}
		return fmt.Errorf("open retained event log for rotation: %w", err)
	}
	if err := verifyHeldEventPath(file, retained, "retained event log rotation source"); err != nil {
		_ = file.Close()
		return err
	}
	if err := os.Rename(retained, older); err != nil {
		_ = file.Close()
		return fmt.Errorf("rotate retained event log: %w", err)
	}
	if err := verifyHeldEventPath(file, older, "older retained event log"); err != nil {
		_ = file.Close()
		return err
	}
	if err := file.Close(); err != nil {
		return fmt.Errorf("close older retained event log: %w", err)
	}
	// Persist the retained-generation shift before touching the active pathname.
	// A crash between `.1 -> .2` and `active -> .1` must leave a recoverable
	// intermediate state (active + durable .2), rather than relying on the later
	// directory fsync that may never execute.
	if err := syncEventLogDirectory(path); err != nil {
		return fmt.Errorf("sync event log directory after retained rotation: %w", err)
	}
	return nil
}

func rotateEventLogIfNeeded(path string) error {
	// Use the writable validation path so an append keeps its established
	// contract of repairing loose permissions to 0600 instead of regressing to
	// the stricter read-only policy. O_RDWR is required to inspect and recover a
	// crash-torn final record before it can be rotated into retained history.
	current, err := openEventLog(path, unix.O_RDWR, 0)
	if err != nil {
		if errors.Is(err, os.ErrNotExist) {
			return nil
		}
		return fmt.Errorf("inspect event log for rotation: %w", err)
	}
	if err := repairEventLogTail(current, path); err != nil {
		_ = current.Close()
		return fmt.Errorf("repair event log tail: %w", err)
	}
	info, statErr := current.Stat()
	if statErr != nil {
		_ = current.Close()
		return fmt.Errorf("stat event log for rotation: %w", statErr)
	}
	if info.Size() < maxEventLogBytes {
		return current.Close()
	}
	if err := verifyHeldEventPath(current, path, "event log rotation source"); err != nil {
		_ = current.Close()
		return err
	}

	// Shift the previous retained generation first. os.Rename replaces an
	// existing .2 atomically, keeping storage bounded at active + two retained
	// generations while preserving one additional handoff window for followers.
	if err := rotateRetainedEventLog(path); err != nil {
		_ = current.Close()
		return err
	}

	rotated := path + ".1"
	if err := os.Rename(path, rotated); err != nil {
		_ = current.Close()
		return fmt.Errorf("rotate event log: %w", err)
	}
	// Revalidate the destination against the fd we held across rename. A
	// same-user pathname swap racing the rename must never be reported as a
	// successful rotation of the intended generation.
	if err := verifyHeldEventPath(current, rotated, "rotated event log"); err != nil {
		_ = current.Close()
		return err
	}
	if err := current.Close(); err != nil {
		return fmt.Errorf("close rotated event log: %w", err)
	}

	if err := syncEventLogDirectory(path); err != nil {
		return fmt.Errorf("sync event log directory after rotation: %w", err)
	}
	return nil
}

func openEventLogForAppend(path string) (*lockedEventLogFile, error) {
	lockPath := path + ".lock"
	lockFile, err := openEventLog(lockPath, unix.O_RDWR|unix.O_CREAT, 0o600)
	if err != nil {
		return nil, fmt.Errorf("open event log writer lock: %w", err)
	}
	if err := unix.Flock(int(lockFile.Fd()), unix.LOCK_EX); err != nil {
		_ = lockFile.Close()
		return nil, fmt.Errorf("lock event log writer: %w", err)
	}
	if err := verifyLockedEventPath(lockFile, lockPath); err != nil {
		_ = unix.Flock(int(lockFile.Fd()), unix.LOCK_UN)
		_ = lockFile.Close()
		return nil, err
	}
	if err := rotateEventLogIfNeeded(path); err != nil {
		_ = unix.Flock(int(lockFile.Fd()), unix.LOCK_UN)
		_ = lockFile.Close()
		return nil, err
	}

	file, err := openEventLog(path, unix.O_WRONLY|unix.O_CREAT|unix.O_APPEND, 0o600)
	if err != nil {
		_ = unix.Flock(int(lockFile.Fd()), unix.LOCK_UN)
		_ = lockFile.Close()
		return nil, err
	}
	if err := verifyHeldEventPath(file, path, "event log"); err != nil {
		_ = file.Close()
		_ = unix.Flock(int(lockFile.Fd()), unix.LOCK_UN)
		_ = lockFile.Close()
		return nil, err
	}
	// Persist the active pathname before any lifecycle record is accepted. This
	// is required both for the first events.log and for the new active generation
	// created after rotation: fsyncing the file contents alone does not make a
	// newly created directory entry crash-durable.
	if err := syncEventLogDirectory(path); err != nil {
		_ = file.Close()
		_ = unix.Flock(int(lockFile.Fd()), unix.LOCK_UN)
		_ = lockFile.Close()
		return nil, fmt.Errorf("persist event log pathname: %w", err)
	}
	// A same-user pathname replacement racing the directory fsync must not let
	// us report a durability barrier for a different inode.
	if err := verifyHeldEventPath(file, path, "event log"); err != nil {
		_ = file.Close()
		_ = unix.Flock(int(lockFile.Fd()), unix.LOCK_UN)
		_ = lockFile.Close()
		return nil, err
	}
	return &lockedEventLogFile{File: file, lockFile: lockFile}, nil
}

func openEventLogForRead(path string) (*os.File, error) {
	return openEventLog(path, unix.O_RDONLY, 0)
}

func openEventLog(path string, flags int, mode uint32) (*os.File, error) {
	if isManagedEventLogPath(path) {
		return openManagedEventLog(path, flags, mode)
	}

	dir := filepath.Dir(path)
	if flags&unix.O_CREAT != 0 {
		if err := os.MkdirAll(dir, 0o700); err != nil {
			return nil, fmt.Errorf("create event log directory: %w", err)
		}
	}
	info, err := os.Lstat(dir)
	if err != nil {
		return nil, fmt.Errorf("inspect event log directory: %w", err)
	}
	if info.Mode()&os.ModeSymlink != 0 || !info.IsDir() {
		return nil, fmt.Errorf("event log directory is not a real directory")
	}
	if err := os.Chmod(dir, 0o700); err != nil {
		return nil, fmt.Errorf("secure event log directory: %w", err)
	}

	dfd, err := unix.Open(dir, unix.O_RDONLY|unix.O_DIRECTORY|unix.O_CLOEXEC|unix.O_NOFOLLOW, 0)
	if err != nil {
		return nil, fmt.Errorf("open event log directory: %w", err)
	}
	defer unix.Close(dfd)
	return openEventAt(dfd, filepath.Base(path), path, flags, mode)
}

func openManagedEventLog(path string, flags int, mode uint32) (*os.File, error) {
	base := eventStateDir()
	if flags&unix.O_CREAT != 0 {
		if err := unix.Mkdir(base, 0o700); err != nil && err != unix.EEXIST {
			return nil, fmt.Errorf("create event state directory: %w", err)
		}
	}

	dfd, err := unix.Open(base, unix.O_RDONLY|unix.O_DIRECTORY|unix.O_CLOEXEC|unix.O_NOFOLLOW, 0)
	if err != nil {
		return nil, fmt.Errorf("open event state directory: %w", err)
	}
	defer unix.Close(dfd)
	if err := unix.Fchmod(dfd, 0o700); err != nil {
		return nil, fmt.Errorf("secure event state directory: %w", err)
	}
	return openEventAt(dfd, filepath.Base(path), path, flags, mode)
}

func openEventAt(dirFD int, name, displayPath string, flags int, mode uint32) (*os.File, error) {
	fd, err := unix.Openat(dirFD, name, flags|unix.O_CLOEXEC|unix.O_NOFOLLOW|unix.O_NONBLOCK, mode)
	if err != nil {
		return nil, fmt.Errorf("open event log: %w", err)
	}

	var st unix.Stat_t
	if err := unix.Fstat(fd, &st); err != nil {
		unix.Close(fd)
		return nil, fmt.Errorf("stat event log: %w", err)
	}
	if st.Mode&unix.S_IFMT != unix.S_IFREG {
		unix.Close(fd)
		return nil, fmt.Errorf("event log is not a regular file")
	}
	if st.Uid != uint32(unix.Geteuid()) {
		unix.Close(fd)
		return nil, fmt.Errorf("event log owner does not match runtime user")
	}
	if st.Nlink != 1 {
		unix.Close(fd)
		return nil, fmt.Errorf("event log has unexpected hard links")
	}

	writable := flags&(unix.O_WRONLY|unix.O_RDWR) != 0
	if writable {
		if err := unix.Fchmod(fd, 0o600); err != nil {
			unix.Close(fd)
			return nil, fmt.Errorf("secure event log permissions: %w", err)
		}
	} else if st.Mode&0o077 != 0 {
		unix.Close(fd)
		return nil, fmt.Errorf("event log permissions are not private")
	}

	file := os.NewFile(uintptr(fd), displayPath)
	if file == nil {
		unix.Close(fd)
		return nil, fmt.Errorf("wrap event log fd")
	}
	return file, nil
}
