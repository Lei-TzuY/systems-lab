package builder

import (
	"fmt"
	"os"
	"path/filepath"

	"minicontainer/internal/state"
)

type managedBuildOutput struct {
	lease       *state.ImageStorageLease
	stageDir    string
	workRootFS  string
	finalDir    string
	durableRoot string
	published   bool
}

func prepareManagedBuildOutput(st *state.Store, imageID string) (*managedBuildOutput, error) {
	lease, err := st.AcquireImageStorage()
	if err != nil {
		return nil, fmt.Errorf("acquire image storage for build: %w", err)
	}
	fail := func(cause error) (*managedBuildOutput, error) {
		_ = lease.Close()
		return nil, cause
	}

	imagesDir := lease.Path()
	finalDir := filepath.Join(imagesDir, imageID)
	if _, err := os.Lstat(finalDir); err == nil {
		return fail(fmt.Errorf("managed build image directory %q already exists", imageID))
	} else if !os.IsNotExist(err) {
		return fail(fmt.Errorf("inspect managed build image directory %q: %w", imageID, err))
	}

	stageDir, err := os.MkdirTemp(imagesDir, ".build-"+imageID+"-")
	if err != nil {
		return fail(fmt.Errorf("create managed build staging directory: %w", err))
	}
	workRootFS := filepath.Join(stageDir, "rootfs")
	if err := os.Mkdir(workRootFS, 0o755); err != nil {
		_ = os.RemoveAll(stageDir)
		return fail(fmt.Errorf("create managed build rootfs staging directory: %w", err))
	}

	return &managedBuildOutput{
		lease:       lease,
		stageDir:    stageDir,
		workRootFS:  workRootFS,
		finalDir:    finalDir,
		durableRoot: filepath.Join(lease.DurablePath(), imageID, "rootfs"),
	}, nil
}

func (m *managedBuildOutput) publish() error {
	if m == nil || m.lease == nil {
		return fmt.Errorf("managed build output is closed")
	}
	if m.published {
		return nil
	}
	if err := m.lease.ValidateConfiguredGeneration(); err != nil {
		return fmt.Errorf("validate image storage generation before build publication: %w", err)
	}
	if err := os.Rename(m.stageDir, m.finalDir); err != nil {
		return fmt.Errorf("publish managed build image directory: %w", err)
	}
	m.published = true
	return nil
}

func (m *managedBuildOutput) cleanupOwned() error {
	if m == nil {
		return nil
	}
	if m.published {
		if err := os.RemoveAll(m.finalDir); err != nil {
			return fmt.Errorf("remove published managed build output %q: %w", m.finalDir, err)
		}
		return nil
	}
	if m.stageDir != "" {
		if err := os.RemoveAll(m.stageDir); err != nil {
			return fmt.Errorf("remove managed build staging directory %q: %w", m.stageDir, err)
		}
	}
	return nil
}

func (m *managedBuildOutput) close() error {
	if m == nil || m.lease == nil {
		return nil
	}
	err := m.lease.Close()
	m.lease = nil
	return err
}
