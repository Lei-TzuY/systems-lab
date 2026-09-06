package builder

import (
	"bufio"
	"errors"
	"fmt"
	"os"
	"path"
	"path/filepath"
	"strings"
	"time"

	"minicontainer/internal/imagestore"
	"minicontainer/internal/state"
)

// BuildOptions options for minictl build.
type BuildOptions struct {
	ContextDir string
	Dockerfile string
	Tag        string
	OutputDir  string
	Store      *state.Store
}

// BuildResult holds outcome metadata of Dockerfile build.
type BuildResult struct {
	Image *state.Image
	Logs  []string
}

// BuildDockerfile parses and executes Dockerfile directives to create a container rootfs image.
func BuildDockerfile(opts BuildOptions) (result *BuildResult, retErr error) {
	if opts.ContextDir == "" {
		return nil, fmt.Errorf("context directory is required")
	}
	dockerfilePath := opts.Dockerfile
	if dockerfilePath == "" {
		dockerfilePath = filepath.Join(opts.ContextDir, "Dockerfile")
	}

	file, err := os.Open(dockerfilePath)
	if err != nil {
		return nil, fmt.Errorf("open Dockerfile %q: %w", dockerfilePath, err)
	}
	defer file.Close()

	imgID := imagestore.GenerateImageID()
	var managedOutput *managedBuildOutput
	cleanupManagedOutput := false
	cleanupOwnedOutput := false
	metadataRootFS := ""

	if opts.Store != nil && opts.OutputDir == "" {
		managedOutput, err = prepareManagedBuildOutput(opts.Store, imgID)
		if err != nil {
			return nil, err
		}
		opts.OutputDir = managedOutput.workRootFS
		metadataRootFS = managedOutput.durableRoot
		cleanupManagedOutput = true
		// Register Close first so owned-path cleanup runs while the lease is still
		// open and its /proc/self/fd mutation path remains stable.
		defer func() {
			if err := managedOutput.close(); err != nil {
				retErr = errors.Join(retErr, fmt.Errorf("close managed build image storage lease: %w", err))
			}
		}()
		defer func() {
			if !cleanupManagedOutput {
				return
			}
			if err := managedOutput.cleanupOwned(); err != nil {
				result = nil
				retErr = errors.Join(retErr, err)
			}
		}()
	} else {
		output, err := prepareBuildOutput(opts, imgID)
		if err != nil {
			return nil, err
		}
		opts.OutputDir = output.path
		metadataRootFS = opts.OutputDir
		cleanupOwnedOutput = output.owned
		defer func() {
			if !cleanupOwnedOutput {
				return
			}
			if err := os.RemoveAll(opts.OutputDir); err != nil {
				result = nil
				retErr = errors.Join(retErr, fmt.Errorf("rollback owned build output %q: %w", opts.OutputDir, err))
			}
		}()
	}

	repo, tag := imagestore.ParseRepositoryTag(opts.Tag)
	img := &state.Image{
		ID:         imgID,
		Repository: repo,
		Tag:        tag,
		Name:       opts.Tag,
		RootFS:     metadataRootFS,
		LoadedAt:   time.Now(),
		WorkDir:    "/",
	}

	var logs []string
	log := func(msg string) {
		logs = append(logs, msg)
		fmt.Println(msg)
	}

	log(fmt.Sprintf("Building image %s (ID: %s)...", opts.Tag, imgID))

	scanner := bufio.NewScanner(file)
	workDir := "/"

	for scanner.Scan() {
		line := strings.TrimSpace(scanner.Text())
		if line == "" || strings.HasPrefix(line, "#") {
			continue
		}

		parts := strings.Fields(line)
		cmd := strings.ToUpper(parts[0])
		args := strings.TrimSpace(line[len(parts[0]):])

		switch cmd {
		case "FROM":
			log(fmt.Sprintf("Step: FROM %s", args))
			if opts.Store != nil {
				if baseImg, err := opts.Store.GetImage(args); err == nil && baseImg.RootFS != "" {
					if err := copyTree(baseImg.RootFS, opts.OutputDir, "/", true); err != nil {
						return nil, fmt.Errorf("copy base image rootfs: %w", err)
					}
					log(fmt.Sprintf("  Loaded base rootfs from image %s", args))
					continue
				}
			}
			// If base image is directory path.
			if info, err := os.Stat(args); err == nil && info.IsDir() {
				if err := copyTree(args, opts.OutputDir, "/", true); err != nil {
					return nil, fmt.Errorf("copy base directory %s to %s failed: %w", args, opts.OutputDir, err)
				}
				log(fmt.Sprintf("  Loaded base rootfs from directory %s", args))
			} else {
				log(fmt.Sprintf("  Warning: Base image/directory %q not found locally. Initializing empty rootfs base.", args))
			}

		case "WORKDIR":
			log(fmt.Sprintf("Step: WORKDIR %s", args))
			logical, err := normalizeContainerPath(workDir, args)
			if err != nil {
				return nil, fmt.Errorf("WORKDIR %q: %w", args, err)
			}
			workDir = logical
			img.WorkDir = logical
			if err := mkdirRootFSPath(opts.OutputDir, logical, 0755); err != nil {
				return nil, fmt.Errorf("create WORKDIR %q: %w", logical, err)
			}

		case "ENV":
			log(fmt.Sprintf("Step: ENV %s", args))
			img.Env = append(img.Env, args)

		case "EXPOSE":
			log(fmt.Sprintf("Step: EXPOSE %s", args))
			img.ExposedPorts = append(img.ExposedPorts, args)

		case "CMD":
			log(fmt.Sprintf("Step: CMD %s", args))
			img.Cmd = parseArrayOrString(args)

		case "ENTRYPOINT":
			log(fmt.Sprintf("Step: ENTRYPOINT %s", args))
			img.Cmd = parseArrayOrString(args)

		case "COPY":
			log(fmt.Sprintf("Step: COPY %s", args))
			copyParts := strings.Fields(args)
			if len(copyParts) < 2 {
				return nil, fmt.Errorf("COPY requires source and destination args")
			}
			src := copyParts[0]
			dst := copyParts[1]

			srcPath, err := resolveBuildContextSource(opts.ContextDir, src)
			if err != nil {
				return nil, err
			}
			srcInfo, err := os.Lstat(srcPath)
			if err != nil {
				return nil, fmt.Errorf("inspect COPY source %q: %w", src, err)
			}
			dstLogical, err := normalizeContainerPath(workDir, dst)
			if err != nil {
				return nil, fmt.Errorf("COPY destination %q: %w", dst, err)
			}
			dstIsDir, err := destinationIsDirectory(opts.OutputDir, dstLogical)
			if err != nil {
				return nil, fmt.Errorf("inspect COPY destination %q: %w", dst, err)
			}
			if dstIsDir || dst == "." || strings.HasSuffix(dst, "/") || strings.HasSuffix(dst, "\\") {
				if err := mkdirRootFSPath(opts.OutputDir, dstLogical, 0755); err != nil {
					return nil, fmt.Errorf("create COPY destination %q: %w", dst, err)
				}
				dstLogical = path.Join(dstLogical, filepath.Base(srcPath))
			}

			if srcInfo.IsDir() {
				if err := copyTree(srcPath, opts.OutputDir, dstLogical, false); err != nil {
					return nil, fmt.Errorf("COPY dir %s to %s failed: %w", src, dst, err)
				}
			} else {
				if err := copyRegularFile(srcPath, opts.OutputDir, dstLogical, srcInfo.Mode()); err != nil {
					return nil, fmt.Errorf("COPY file %s to %s failed: %w", src, dst, err)
				}
			}
			log(fmt.Sprintf("  Copied %s -> %s", src, dst))

		case "RUN":
			log(fmt.Sprintf("Step: RUN %s", args))
			// Execute simple shell inline script if file creation / echo.
			if strings.HasPrefix(args, "echo ") && strings.Contains(args, ">") {
				echoParts := strings.SplitN(args, ">", 2)
				val := strings.TrimSpace(strings.TrimPrefix(echoParts[0], "echo"))
				val = strings.Trim(val, "\"'")
				outFile := strings.TrimSpace(echoParts[1])
				targetLogical, err := normalizeContainerPath(workDir, outFile)
				if err != nil {
					return nil, fmt.Errorf("RUN output %q: %w", outFile, err)
				}
				if err := mkdirRootFSPath(opts.OutputDir, path.Dir(targetLogical), 0755); err != nil {
					return nil, fmt.Errorf("create RUN output parent: %w", err)
				}
				targetFile, err := resolveRootFSPath(opts.OutputDir, targetLogical)
				if err != nil {
					return nil, fmt.Errorf("resolve RUN output: %w", err)
				}
				if err := os.WriteFile(targetFile, []byte(val+"\n"), 0644); err != nil {
					return nil, fmt.Errorf("write RUN output: %w", err)
				}
			}
		}
	}

	if err := scanner.Err(); err != nil {
		return nil, fmt.Errorf("scan Dockerfile: %w", err)
	}

	sz, err := imagestore.CalculateDirSize(opts.OutputDir)
	if err != nil {
		return nil, fmt.Errorf("calculate built image size: %w", err)
	}
	img.Size = sz

	if managedOutput != nil {
		if err := managedOutput.publish(); err != nil {
			return nil, err
		}
	}

	if opts.Store != nil {
		ownedExternal := cleanupOwnedOutput
		// Publication may commit metadata and then report a later maintenance
		// error. Disable generic external rollback and inspect durable state first.
		cleanupOwnedOutput = false
		if err := opts.Store.PublishImage(img); err != nil {
			saveErr := error(fmt.Errorf("save image state: %w", err))
			if managedOutput != nil {
				referenced, proofErr := buildPayloadHasCommittedReference(opts.Store, img.ID, img.RootFS)
				switch {
				case proofErr != nil:
					cleanupManagedOutput = false
					saveErr = errors.Join(saveErr, fmt.Errorf("preserve managed build output because metadata absence is unproven: %w", proofErr))
				case referenced:
					cleanupManagedOutput = false
				default:
					if cleanupErr := managedOutput.cleanupOwned(); cleanupErr != nil {
						saveErr = errors.Join(saveErr, cleanupErr)
					} else {
						cleanupManagedOutput = false
					}
				}
			} else if ownedExternal {
				referenced, proofErr := buildPayloadHasCommittedReference(opts.Store, img.ID, img.RootFS)
				switch {
				case proofErr != nil:
					saveErr = errors.Join(saveErr, fmt.Errorf("preserve newly built output because metadata absence is unproven: %w", proofErr))
				case referenced:
					// Durable metadata owns the output; deleting it would create a
					// dangling image even though publication returned an error.
				default:
					if cleanupErr := os.RemoveAll(opts.OutputDir); cleanupErr != nil {
						saveErr = errors.Join(saveErr, fmt.Errorf("rollback unpublished build output %q: %w", opts.OutputDir, cleanupErr))
					}
				}
			}
			return nil, saveErr
		}
	}

	cleanupManagedOutput = false
	cleanupOwnedOutput = false
	log(fmt.Sprintf("Successfully built image %s (Size: %d bytes)", opts.Tag, sz))
	return &BuildResult{Image: img, Logs: logs}, nil
}

func parseArrayOrString(val string) []string {
	val = strings.TrimSpace(val)
	if strings.HasPrefix(val, "[") && strings.HasSuffix(val, "]") {
		content := val[1 : len(val)-1]
		rawParts := strings.Split(content, ",")
		var out []string
		for _, p := range rawParts {
			cleaned := strings.TrimSpace(p)
			cleaned = strings.Trim(cleaned, "\"'")
			if cleaned != "" {
				out = append(out, cleaned)
			}
		}
		return out
	}
	return strings.Fields(val)
}
