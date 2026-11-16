// Package platform translates between Go's names for a platform (GOOS and GOARCH) and
// docattest's (`linux-x64`, the names protocol/platforms.json lists), and says what a
// program's file is called on one.
package platform

import "strings"

// docattestArch are the processors docattest names otherwise than Go does, by Go's name.
var docattestArch = map[string]string{"amd64": "x64"}

// Name is docattest's name for the platform Go calls goos/goarch: darwin-arm64, linux-x64.
func Name(goos, goarch string) string {
	if renamed, ok := docattestArch[goarch]; ok {
		goarch = renamed
	}
	return goos + "-" + goarch
}

// GoNames are Go's names for a platform docattest names.
func GoNames(name string) (goos, goarch string) {
	goos, arch, _ := strings.Cut(name, "-")
	for goName, docattestName := range docattestArch {
		if arch == docattestName {
			return goos, goName
		}
	}
	return goos, arch
}

// Executable is the file name of a program called `base` on a system Go calls goos.
func Executable(goos, base string) string {
	if goos == "windows" {
		return base + ".exe"
	}
	return base
}
