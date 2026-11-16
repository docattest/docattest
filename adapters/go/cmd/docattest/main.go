// Command docattest is docattest for a Go project: a launcher that finds the docattest
// core, tells it how to start the Go adapter, and becomes it.
package main

import (
	"fmt"
	"os"
	"runtime"

	"github.com/docattest/docattest/adapters/go/internal/bundled"
	"github.com/docattest/docattest/adapters/go/internal/launcher"
	"github.com/docattest/docattest/adapters/go/internal/protocoldata"
)

func main() {
	executable, err := os.Executable()
	if err != nil {
		executable = os.Args[0]
	}
	home, _ := os.UserHomeDir()
	machine := launcher.Machine{
		Env: os.Getenv, OS: runtime.GOOS, Arch: runtime.GOARCH,
		Home: home, Executable: executable, Ask: launcher.AskCore, Carried: bundled.Core(),
	}
	core, err := launcher.FindCore(machine, protocoldata.Core())
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
	// With a trace asked for, say which core runs.
	if launcher.Traced(os.Getenv) {
		fmt.Fprintf(os.Stderr, "[trace launcher] core %s\n", core)
	}
	args := os.Args[1:]
	err = launcher.Become(core, args, launcher.Environment(machine, args, os.Environ()))
	fmt.Fprintf(os.Stderr, "docattest: cannot run the core (%s): %v\n", core, err)
	os.Exit(1)
}
