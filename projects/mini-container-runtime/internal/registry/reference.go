package registry

import (
	"fmt"
	"net/url"
	"regexp"
	"strings"
)

const maxRepositoryNameLength = 255

var (
	repositoryComponentPattern = regexp.MustCompile(`^[a-z0-9]+(?:(?:[._]|__|-+)[a-z0-9]+)*$`)
	tagPattern                 = regexp.MustCompile(`^[A-Za-z0-9_][A-Za-z0-9_.-]{0,127}$`)
)

func validateRepositoryName(name string) error {
	if name == "" {
		return fmt.Errorf("repository name cannot be empty")
	}
	if len(name) > maxRepositoryNameLength {
		return fmt.Errorf("repository name exceeds %d characters", maxRepositoryNameLength)
	}
	parts := strings.Split(name, "/")
	for _, part := range parts {
		if !repositoryComponentPattern.MatchString(part) {
			return fmt.Errorf("invalid repository component %q", part)
		}
	}
	return nil
}

func validateImageReference(name, tag string) error {
	if err := validateRepositoryName(name); err != nil {
		return err
	}
	if !tagPattern.MatchString(tag) {
		return fmt.Errorf("invalid tag %q", tag)
	}
	return nil
}

func authTokenURL(imageName string) (string, error) {
	if err := validateRepositoryName(imageName); err != nil {
		return "", err
	}
	u := url.URL{Scheme: "https", Host: defaultAuthHost, Path: "/token"}
	query := u.Query()
	query.Set("service", "registry.docker.io")
	query.Set("scope", "repository:"+imageName+":pull")
	u.RawQuery = query.Encode()
	return u.String(), nil
}

func manifestURL(imageName, tag string) (string, error) {
	if err := validateImageReference(imageName, tag); err != nil {
		return "", err
	}
	u := url.URL{
		Scheme: "https",
		Host:   defaultRegistryHost,
		Path:   "/v2/" + imageName + "/manifests/" + tag,
	}
	return u.String(), nil
}

func blobURL(imageName, digest string) (string, error) {
	if err := validateRepositoryName(imageName); err != nil {
		return "", err
	}
	if _, err := parseSHA256Digest(digest); err != nil {
		return "", err
	}
	u := url.URL{
		Scheme: "https",
		Host:   defaultRegistryHost,
		Path:   "/v2/" + imageName + "/blobs/" + digest,
	}
	return u.String(), nil
}
