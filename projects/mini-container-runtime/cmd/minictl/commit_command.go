package main

import (
	"minicontainer/internal/container"
	"minicontainer/internal/state"
)

func commitContainerImage(st *state.Store, containerID, imageName string) (*state.Image, error) {
	return container.CommitContainer(st, containerID, imageName)
}
