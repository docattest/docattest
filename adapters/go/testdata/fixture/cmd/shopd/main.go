// Command shopd serves the shop. Its settings are constants of package main, which
// nothing can import: the pages read them from source.
package main

import (
	"fmt"
	"strings"
	"time"

	"example.com/shop"
)

// listenPort is the port the server listens on.
const listenPort = 8080

// requestTimeout is how long a request may take before it is cancelled.
const requestTimeout = 30 * time.Second

// region is the tax region of a basket that names none.
const region = "nl"

// verbosity is how much the server logs.
type verbosity int

func (v verbosity) String() string { return [...]string{"quiet", "normal", "chatty"}[v] }

// logLevel is the verbosity the server starts with.
const logLevel verbosity = 2

// started is when the server started: a variable, which no fact can read.
var started = time.Now()

func main() {
	sample, _ := shop.Total([]shop.Money{shop.EUR(1000)}, strings.ToUpper(region))
	fmt.Printf("listening on :%d since %v, logging %v, timing out after %v; a sample basket costs %v\n",
		listenPort, started, logLevel, requestTimeout, sample)
}
