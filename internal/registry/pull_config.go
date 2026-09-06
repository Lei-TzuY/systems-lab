package registry

import (
	"encoding/json"
	"fmt"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"time"

	"minicontainer/internal/container"
	"minicontainer/internal/state"
)

type imageConfigDocument struct {
	Config struct {
		StopSignal string   `json:"StopSignal"`
		Env        []string `json:"Env,omitempty"`
		WorkingDir string   `json:"WorkingDir,omitempty"`
		Entrypoint []string `json:"Entrypoint,omitempty"`
		Cmd        []string `json:"Cmd,omitempty"`
	} `json:"config"`
}

type imageRuntimeConfig struct {
	StopSignal string
	Env        []string
	WorkingDir string
	Entrypoint []string
	Cmd        []string
}

func pullImageRuntimeConfig(client *http.Client, imageName, token, tmpDir string, desc Descriptor) (imageRuntimeConfig, error) {
	if client == nil {
		return imageRuntimeConfig{}, fmt.Errorf("image config HTTP client is nil")
	}
	if err := validateLayerDescriptor(desc); err != nil {
		return imageRuntimeConfig{}, fmt.Errorf("invalid config descriptor: %w", err)
	}

	configPath := filepath.Join(tmpDir, "config.json")
	if err := downloadBlob(client, imageName, desc.Digest, token, configPath, desc.Size); err != nil {
		return imageRuntimeConfig{}, fmt.Errorf("download config blob: %w", err)
	}
	data, err := os.ReadFile(configPath)
	if err != nil {
		return imageRuntimeConfig{}, fmt.Errorf("read verified config blob: %w", err)
	}
	return parseImageRuntimeConfig(data)
}

func pullImageStopSignal(client *http.Client, imageName, token, tmpDir string, desc Descriptor) (string, error) {
	cfg, err := pullImageRuntimeConfig(client, imageName, token, tmpDir, desc)
	if err != nil {
		return "", err
	}
	return cfg.StopSignal, nil
}

func validateImageArgvField(name string, values []string) ([]string, error) {
	out := append([]string(nil), values...)
	for _, value := range out {
		if strings.IndexByte(value, 0) >= 0 {
			return nil, fmt.Errorf("invalid image %s entry %q: contains NUL", name, value)
		}
	}
	return out, nil
}

func parseImageRuntimeConfig(data []byte) (imageRuntimeConfig, error) {
	var cfg imageConfigDocument
	if err := json.Unmarshal(data, &cfg); err != nil {
		return imageRuntimeConfig{}, fmt.Errorf("decode image config: %w", err)
	}
	signal := strings.TrimSpace(cfg.Config.StopSignal)
	if signal == "" {
		signal = "SIGTERM"
	}
	if _, err := container.ParseSignal(signal); err != nil {
		return imageRuntimeConfig{}, fmt.Errorf("invalid image StopSignal %q: %w", signal, err)
	}
	env := make([]string, 0, len(cfg.Config.Env))
	for _, entry := range cfg.Config.Env {
		key, _, ok := strings.Cut(entry, "=")
		if !ok || key == "" || strings.IndexByte(key, 0) >= 0 || strings.IndexByte(entry, 0) >= 0 {
			return imageRuntimeConfig{}, fmt.Errorf("invalid image environment entry %q", entry)
		}
		env = append(env, entry)
	}
	workingDir := strings.TrimSpace(cfg.Config.WorkingDir)
	if strings.IndexByte(workingDir, 0) >= 0 {
		return imageRuntimeConfig{}, fmt.Errorf("invalid image WorkingDir %q: contains NUL", workingDir)
	}
	if workingDir != "" {
		if !filepath.IsAbs(workingDir) {
			return imageRuntimeConfig{}, fmt.Errorf("invalid image WorkingDir %q: must be absolute", workingDir)
		}
		workingDir = filepath.Clean(workingDir)
	}
	entrypoint, err := validateImageArgvField("Entrypoint", cfg.Config.Entrypoint)
	if err != nil {
		return imageRuntimeConfig{}, err
	}
	cmd, err := validateImageArgvField("Cmd", cfg.Config.Cmd)
	if err != nil {
		return imageRuntimeConfig{}, err
	}
	return imageRuntimeConfig{
		StopSignal: signal,
		Env:        env,
		WorkingDir: workingDir,
		Entrypoint: entrypoint,
		Cmd:        cmd,
	}, nil
}

func parseImageConfigStopSignal(data []byte) (string, error) {
	cfg, err := parseImageRuntimeConfig(data)
	if err != nil {
		return "", err
	}
	return cfg.StopSignal, nil
}

func persistPulledImageRuntimeMetadata(imageRef, destDir string, cfg imageRuntimeConfig) error {
	st, err := state.Open(state.DefaultDir())
	if err != nil {
		return fmt.Errorf("open image state store: %w", err)
	}
	defer st.Close()

	if err := st.SaveImage(&state.Image{
		Name:     imageRef,
		RootFS:   destDir,
		LoadedAt: time.Now(),
		WorkDir:  cfg.WorkingDir,
		Env:      append([]string(nil), cfg.Env...),
		Cmd:      append([]string(nil), cfg.Cmd...),
	}); err != nil {
		return fmt.Errorf("save pulled image: %w", err)
	}
	if err := st.SaveImageEnvironment(imageRef, cfg.Env); err != nil {
		return fmt.Errorf("save pulled image environment: %w", err)
	}
	if err := st.SaveImageWorkingDir(imageRef, cfg.WorkingDir); err != nil {
		return fmt.Errorf("save pulled image WorkingDir: %w", err)
	}
	if err := st.SaveImageCommand(imageRef, state.ImageCommand{
		Entrypoint: cfg.Entrypoint,
		Cmd:        cfg.Cmd,
	}); err != nil {
		return fmt.Errorf("save pulled image command: %w", err)
	}
	if err := st.SaveImageStopSignal(imageRef, cfg.StopSignal); err != nil {
		return fmt.Errorf("save pulled image StopSignal: %w", err)
	}
	return nil
}

func persistPulledImageMetadata(imageRef, destDir, stopSignal string) error {
	return persistPulledImageRuntimeMetadata(imageRef, destDir, imageRuntimeConfig{StopSignal: stopSignal})
}
