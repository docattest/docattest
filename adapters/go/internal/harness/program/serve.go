package program

import (
	"bytes"
	"encoding/json"
	"fmt"
	"os"
	"strconv"
	"time"
)

// docattestJob is what the adapter tells a generated program: os.Args[1] names its file.
type docattestJob struct {
	TimeoutMs float64 `json:"timeoutMs"`
	Snapshots struct {
		Dir  string         `json:"dir"`
		Data map[string]any `json:"data"`
	} `json:"snapshots"`
	Manifests map[string]map[string]any    `json:"manifests"`
	Texts     map[string]string            `json:"texts"`
	Matches   map[string]map[string]string `json:"matches"`
	Fields    map[string]map[string]any    `json:"manifestFields"`
	Cases     docattestTable               `json:"cases"`
}

// docattestLine is one line of a program's results: an item's value, or that it
// panicked or ran out of time; or, before any item, why none can be served.
type docattestLine struct {
	Item    int    `json:"item"`
	Value   any    `json:"value,omitempty"`
	Panic   string `json:"panic,omitempty"`
	Timeout bool   `json:"timeout,omitempty"`
	Failed  string `json:"failed,omitempty"`
}

var (
	docattestTold    docattestJob
	docattestResults *os.File
)

// docattestStart reads the job and opens the results, which os.Args[2] names: a file,
// so that nothing the program prints can be taken for a result. It returns the item to
// begin at (os.Args[3]): the adapter starts a program again after an item that ended it.
// Facts, cases and observables cannot be tagged `check network`, so they always run offline.
func docattestStart() int {
	if len(os.Args) != 4 {
		docattestStop("usage: <job.json> <results> <first item>")
	}
	err := docattestReadJob(os.Args[1])
	if err != nil {
		docattestStop(err.Error())
	}
	docattestResults, err = os.OpenFile(os.Args[2], os.O_APPEND|os.O_CREATE|os.O_WRONLY, 0o600)
	if err != nil {
		docattestStop(err.Error())
	}
	docattestRefuseNetwork()
	first, _ := strconv.Atoi(os.Args[3])
	return first
}

// docattestReadJob reads the job, the numbers of its snapshots, manifests and fields as
// DocattestNumbers reads them.
func docattestReadJob(path string) error {
	text, err := os.ReadFile(path)
	if err != nil {
		return err
	}
	decoder := json.NewDecoder(bytes.NewReader(text))
	decoder.UseNumber()
	if err := decoder.Decode(&docattestTold); err != nil {
		return err
	}
	for name, data := range docattestTold.Snapshots.Data {
		if docattestTold.Snapshots.Data[name], err = DocattestNumbers(data); err != nil {
			return fmt.Errorf("snapshot %q: %w", name, err)
		}
	}
	for name, data := range docattestTold.Manifests {
		if _, err := DocattestNumbers(data); err != nil {
			return fmt.Errorf("manifest %q: %w", name, err)
		}
	}
	for name, fields := range docattestTold.Fields {
		if _, err := DocattestNumbers(fields); err != nil {
			return fmt.Errorf("field of %q: %w", name, err)
		}
	}
	return nil
}

func docattestStop(why string) {
	fmt.Fprintln(os.Stderr, "docattest:", why)
	os.Exit(2)
}

func docattestWrite(line docattestLine) {
	text, err := json.Marshal(line)
	if err != nil {
		text, _ = json.Marshal(docattestLine{Item: line.Item, Panic: "a result that is not JSON: " + err.Error()})
	}
	if _, err := docattestResults.Write(append(text, '\n')); err != nil {
		docattestStop(err.Error())
	}
}

// docattestServe runs items `first` to `count`, each with the job's time limit, and
// writes a line for each. An item that panics is reported and the next one runs; one
// that runs out of time cannot be stopped, so the program reports it and ends.
func docattestServe(first, count int, item func(k int) any) {
	limit := time.Duration(docattestTold.TimeoutMs * float64(time.Millisecond))
	for k := first; k < count; k++ {
		done := make(chan docattestLine, 1)
		go func() { done <- docattestGuarded(k, item) }()
		select {
		case line := <-done:
			docattestWrite(line)
		case <-time.After(limit):
			docattestWrite(docattestLine{Item: k, Timeout: true})
			os.Exit(3)
		}
	}
}

func docattestGuarded(k int, item func(k int) any) (line docattestLine) {
	defer func() {
		if caught := recover(); caught != nil {
			line = docattestLine{Item: k, Panic: fmt.Sprint(caught)}
		}
	}()
	return docattestLine{Item: k, Value: item(k)}
}

// snapshot is the data of a snapshot of the project, as encoding/json reads JSON into
// an `any` but for its numbers, which are as DocattestNumbers reads them. A fact
// expression calls it by this name.
func snapshot(name string) any {
	data, found := docattestTold.Snapshots.Data[name]
	if !found {
		where := docattestTold.Snapshots.Dir
		if where == "" {
			where = "(no snapshots dir configured)"
		}
		panic(fmt.Sprintf("snapshot %q not found in %s", name, where))
	}
	return data
}

// manifest is the data of a manifest of the project, as encoding/json reads a JSON object
// into a map, so that a fact indexes it at once: manifest("go.mod")["go"]; its numbers
// are as DocattestNumbers reads them. A fact expression calls it by this name. The core
// reads a manifest only when a fact writes its path out; one built at run time was never
// read.
func manifest(path string) map[string]any {
	data, found := docattestTold.Manifests[path]
	if !found {
		panic(fmt.Sprintf("manifest %q was not read: name it in the fact itself, as manifest(%q)", path, path))
	}
	return data
}

// file is the text of a file of the project, so that a fact reads it with the strings and
// regexp packages: regexp.MustCompile(`USER (\d+)`).FindStringSubmatch(file("Dockerfile"))[1].
// A fact expression calls it by this name. The core reads a file only when a fact writes
// its path out, as it does a manifest.
func file(path string) string {
	text, found := docattestTold.Texts[path]
	if !found {
		panic(fmt.Sprintf("file %q was not read: name it in the fact itself, as file(%q)", path, path))
	}
	return text
}

// matched is what a regex matches in a file of the project, as the core matched it: the
// text of the regex's first group in its first match, or of the whole match, so that a
// fact reads it at once: matched("Dockerfile", `^USER (\d+)$`). A fact expression calls it
// by this name. The core matches a regex only when a fact writes both arguments out.
func matched(path, pattern string) string {
	text, found := docattestTold.Matches[path][pattern]
	if !found {
		panic(docattestNotRead("matched", path, pattern))
	}
	return text
}

// manifestField is the value at a path of keys in a manifest of the project, as the core
// looked it up, held as manifest holds the manifest's data, so that a fact reads a deep
// field without a type assertion:
// manifestField(".gitlab-ci.yml", "deploy.variables.VERSION"). A fact expression calls it
// by this name. The core looks a field up only when a fact writes both arguments out.
func manifestField(path, keys string) any {
	value, found := docattestTold.Fields[path][keys]
	if !found {
		panic(docattestNotRead("manifestField", path, keys))
	}
	return value
}

func docattestNotRead(name, path, argument string) string {
	return fmt.Sprintf("%s(%q, %q) was not read: write both arguments out in the fact itself, as string literals", name, path, argument)
}
