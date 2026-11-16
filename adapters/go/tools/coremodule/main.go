// Command coremodule writes what lets `go install` bring the docattest core.
//
//	go run ./tools/coremodule -base <module path> -out <dir> -link <dir> <platform>=<core>…
//
// For each platform it writes the module that carries that platform's core into
// <out>/core-<platform>, and the file that imports it into <link>, which is the
// launcher's internal/bundled. <module path> is where the modules will be fetched from:
// each is <module path>/core-<platform>. The launcher's go.mod must then require them
// (`go mod tidy`), and every module must be tagged with the release's version.
package main

import (
	"flag"
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/docattest/docattest/adapters/go/internal/coremodule"
)

func main() {
	base := flag.String("base", "", "the module path the core modules are fetched below")
	out := flag.String("out", "", "the directory the core modules are written into")
	link := flag.String("link", "", "the launcher's internal/bundled directory")
	flag.Parse()
	if *base == "" || *out == "" || *link == "" || flag.NArg() == 0 {
		flag.Usage()
		os.Exit(2)
	}
	for _, arg := range flag.Args() {
		if err := write(*base, *out, *link, arg); err != nil {
			fmt.Fprintln(os.Stderr, "coremodule:", err)
			os.Exit(1)
		}
	}
}

// write writes one platform's module and its link file, from `<platform>=<core>`.
func write(base, out, link, arg string) error {
	name, path, found := strings.Cut(arg, "=")
	if !found {
		return fmt.Errorf("%q is not <platform>=<core>", arg)
	}
	core, err := os.ReadFile(path)
	if err != nil {
		return err
	}
	platform := coremodule.Platform(name)
	if err := coremodule.WriteModule(filepath.Join(out, "core-"+name), base, platform, core); err != nil {
		return err
	}
	file, text := coremodule.LinkFile(base, platform)
	return os.WriteFile(filepath.Join(link, file), []byte(text), 0o644)
}
