package program

import docattestos "os"

// DocattestStartLine is what a block's program writes to its standard error as the
// block's own code is about to run: the adapter counts the block's time limit from it, so
// that the system's start-up of the program is not counted.
const DocattestStartLine = "docattest: the block starts\n"

// docattestSayStarted writes the start line. A block's program calls it to initialise a
// variable of its own, which comes before any of the block's.
func docattestSayStarted() bool {
	_, err := docattestos.Stderr.WriteString(DocattestStartLine)
	return err == nil
}
