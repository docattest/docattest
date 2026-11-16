//go:build !unix

package toolchain

import "os/exec"

// ownGroup has nothing to add where there are no process groups: a timeout kills the
// program itself, which is what exec.CommandContext does.
func ownGroup(*exec.Cmd) {}
