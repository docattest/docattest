// Command docattest-adapter-go is the Go adapter of docattest: the program the
// docattest core starts to answer what needs Go. It speaks version 1 of the adapter
// protocol on its standard input and output. Nobody runs it by hand.
package main

import (
	"fmt"
	"os"
	"runtime/debug"

	"github.com/docattest/docattest/adapters/go/internal/language"
	"github.com/docattest/docattest/adapters/go/internal/protocol"
	"github.com/docattest/docattest/adapters/go/internal/session"
	"github.com/docattest/docattest/adapters/go/internal/toolchain"
)

// collectAt is the garbage collector's target, as GOGC sets it: the heap grows this many
// percent over what is live before a collection. The adapter lives for one invocation and
// type-checks whole import graphs from source, which allocate much and keep most, so
// collecting less often is worth more than the memory it costs.
const collectAt = 400

func main() {
	if os.Getenv("GOGC") == "" {
		debug.SetGCPercent(collectAt)
	}
	if err := toolchain.PutChosenGoFirst(); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
	// Responses go to the descriptor the core reads; from here on, anything this process
	// or a library it uses prints through os.Stdout goes to stderr instead.
	responses := os.Stdout
	os.Stdout = os.Stderr
	protocol.Serve(os.Stdin, responses, protocol.Adapter{
		Hello:    language.Hello(),
		Language: language.Methods(),
		Open:     session.Open,
	})
}
