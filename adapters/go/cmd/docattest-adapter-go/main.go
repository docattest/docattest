// Command docattest-adapter-go is the Go adapter of docattest: the program the
// docattest core starts to answer what needs Go. It speaks version 1 of the adapter
// protocol on its standard input and output. Nobody runs it by hand.
package main

import (
	"os"

	"github.com/docattest/docattest/adapters/go/internal/language"
	"github.com/docattest/docattest/adapters/go/internal/protocol"
	"github.com/docattest/docattest/adapters/go/internal/session"
)

func main() {
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
