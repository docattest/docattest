// Package protocol is the docattest adapter protocol as this adapter speaks it, in the
// version protocoldata pins: the data types of the schemas in protocol/schema and the loop
// that reads requests and writes responses.
package protocol

import "encoding/json"

// Kind is what an attestation, or the result of one, is.
type Kind string

// The kinds of attestation the Go adapter reads or answers.
const (
	KindExample Kind = "example"
	KindFact    Kind = "fact"
	KindCases   Kind = "cases"
	KindTest    Kind = "test"
)

// Status is how a result ended.
type Status string

// The statuses of a result.
const (
	StatusPass    Status = "pass"
	StatusFail    Status = "fail"
	StatusError   Status = "error"
	StatusSkipped Status = "skipped"
)

// Example is an ExampleAttestation: a fenced block, or an example in a source file.
type Example struct {
	ID      string   `json:"id"`
	Page    string   `json:"page"`
	Line    int      `json:"line"`
	Claim   string   `json:"claim"`
	Kind    Kind     `json:"kind"`
	Lang    string   `json:"lang"`
	Mode    string   `json:"mode"`
	Network bool     `json:"network"`
	Code    string   `json:"code"`
	Weak    bool     `json:"weak"`
	Owner   string   `json:"owner,omitempty"`
	Tags    []string `json:"tags,omitempty"`
	Error   string   `json:"error,omitempty"`
}

// Fact is a FactAttestation: an expression over a surface. Its target is the core's.
type Fact struct {
	ID         string `json:"id"`
	Page       string `json:"page"`
	Line       int    `json:"line"`
	Expression string `json:"expression"`
	Surface    string `json:"surface"`
}

// CasesRow is one row of a cases table.
type CasesRow struct {
	Line   int               `json:"line"`
	Name   string            `json:"name"`
	Values map[string]string `json:"values"`
}

// Cases is a CasesAttestation: a table run row by row by a runner of the setup package.
type Cases struct {
	ID     string     `json:"id"`
	Page   string     `json:"page"`
	Line   int        `json:"line"`
	Kind   Kind       `json:"kind"`
	Runner string     `json:"runner"`
	Header []string   `json:"header"`
	Rows   []CasesRow `json:"rows"`
}

// Attestation is any of the four kinds, with the fields `symbols` reads of each.
type Attestation struct {
	ID         string   `json:"id"`
	Page       string   `json:"page"`
	Line       int      `json:"line"`
	Kind       Kind     `json:"kind"`
	Code       string   `json:"code"`
	Expression string   `json:"expression"`
	Surface    string   `json:"surface"`
	Runner     string   `json:"runner"`
	File       string   `json:"file"`
	Title      string   `json:"title"`
	Describes  []string `json:"describes"`
}

// Diagnostic is a reason an example does not compile.
type Diagnostic struct {
	ID      string `json:"id"`
	Message string `json:"message"`
	Line    int    `json:"line,omitempty"`
}

// Observation is a value as the runtime observed it.
type Observation struct {
	Text        string   `json:"text"`
	Items       []string `json:"items,omitempty"`
	Description string   `json:"description"`
	Kind        string   `json:"kind,omitempty"`
}

// Evaluation is the outcome of evaluating one fact expression.
type Evaluation struct {
	OK      bool         `json:"ok"`
	Value   *Observation `json:"value,omitempty"`
	Message string       `json:"message,omitempty"`
}

// Failed is the Evaluation of a fact that has no value.
func Failed(message string) Evaluation {
	return Evaluation{Message: message}
}

// SymbolRef is a surface symbol an attestation exercises.
type SymbolRef struct {
	Name    string `json:"name"`
	Surface string `json:"surface,omitempty"`
}

// SymbolMutants are the edits that change the literals of one symbol's declaration, each
// alone; Why says why there are none, exactly when there are none.
type SymbolMutants struct {
	Edits []MutantEdit `json:"edits"`
	Why   MutantsWhy   `json:"why,omitempty"`
}

// MutantsWhy is why a symbol has no edits.
type MutantsWhy string

const (
	// WhyFunction is a function, a method or a type: what a proof reads of it is
	// computed when it runs.
	WhyFunction MutantsWhy = "function"
	// WhyNoLiteral is a value whose declaration holds no literal.
	WhyNoLiteral MutantsWhy = "noLiteral"
	// WhyNotFound is a name no file of the project's own declares.
	WhyNotFound MutantsWhy = "notFound"
)

// MutantEdit is one change to one file: the bytes from Start to End (UTF-8 offsets, End
// excluded) replaced by Replacement.
type MutantEdit struct {
	Path        string `json:"path"`
	Start       int    `json:"start"`
	End         int    `json:"end"`
	Replacement string `json:"replacement"`
	// Description says what changes, as Go writes it: `8080 → 8081`.
	Description string `json:"description"`
}

// Result is the verdict on one example, or on one row of a cases table.
type Result struct {
	Page     string   `json:"page"`
	Line     int      `json:"line"`
	Kind     Kind     `json:"kind"`
	Status   Status   `json:"status"`
	Symbols  []string `json:"symbols"`
	Expected string   `json:"expected,omitempty"`
	Actual   string   `json:"actual,omitempty"`
	Message  string   `json:"message"`
}

// DeclaredTest is a test a test file declares; a nil title in Describes cannot be cited.
type DeclaredTest struct {
	Title     string    `json:"title"`
	Describes []*string `json:"describes"`
	Line      int       `json:"line"`
}

// DeclaredSuite is something a test file declares that may be cited by mistake for a test.
type DeclaredSuite struct {
	Title     string    `json:"title"`
	Describes []*string `json:"describes"`
	Line      int       `json:"line"`
	Kind      string    `json:"kind"`
}

// TestListing is what a test file declares, or why it cannot be read.
type TestListing struct {
	OK      bool             `json:"ok"`
	Tests   *[]DeclaredTest  `json:"tests,omitempty"`
	Suites  *[]DeclaredSuite `json:"suites,omitempty"`
	Message string           `json:"message,omitempty"`
}

// CitedTest is a test a citation names, as its file declares it: the names that enclose
// it, outermost first, and its own.
type CitedTest struct {
	File      string   `json:"file"`
	Describes []string `json:"describes"`
	Title     string   `json:"title"`
}

// TestRunStatus is how a cited test ran.
type TestRunStatus string

// The ways a cited test can have run.
const (
	TestPassed  TestRunStatus = "passed"
	TestFailed  TestRunStatus = "failed"
	TestSkipped TestRunStatus = "skipped"
	TestNotRun  TestRunStatus = "notRun"
)

// TestRun is how a cited test ran. Message, what the runner said of it, is empty exactly
// when the test passed.
type TestRun struct {
	Status  TestRunStatus `json:"status"`
	Message string        `json:"message,omitempty"`
}

// ConfiguredSurface is a surface as the config names it.
type ConfiguredSurface struct {
	Name string `json:"name"`
	// Entry is a SurfaceEntry: a string, or an import and a path. What it means is the
	// adapter's to say.
	Entry json.RawMessage `json:"entry"`
}

// SurfacePackage is the package a surface belongs to, as the language writes it.
type SurfacePackage struct {
	Name    string `json:"name"`
	Display string `json:"display"`
}

// SurfaceResolution says where a surface resolved.
type SurfaceResolution struct {
	Import  string          `json:"import"`
	Entry   string          `json:"entry"`
	Package *SurfacePackage `json:"package,omitempty"`
}

// SurfaceDescription is what the report says of one surface.
type SurfaceDescription struct {
	Resolution *SurfaceResolution `json:"resolution,omitempty"`
	Warnings   []string           `json:"warnings"`
}

// Problem is a failure that belongs to no attestation, at a line of a project file.
type Problem struct {
	Path    string `json:"path"`
	Line    int    `json:"line"`
	Message string `json:"message"`
}

// ProjectProblems answers `problems`: the failures outside every attestation, and the
// warnings about the whole project.
type ProjectProblems struct {
	Problems []Problem `json:"problems"`
	Warnings []string  `json:"warnings"`
}

// Observable is a value a surface offers, with the fact expression that reads it.
type Observable struct {
	Surface    string      `json:"surface"`
	Expression string      `json:"expression"`
	Path       []string    `json:"path,omitempty"`
	Value      Observation `json:"value"`
}

// SourceText is a file by path and text, as the questions about text receive it.
type SourceText struct {
	Page string `json:"page"`
	Text string `json:"text"`
}

// DocProse is one doc comment as prose, with its first and last line.
type DocProse struct {
	Text  string `json:"text"`
	Start int    `json:"start"`
	End   int    `json:"end"`
}

// ExpressionShape is a fact expression with its names taken out.
type ExpressionShape struct {
	Shape string   `json:"shape"`
	Names []string `json:"names"`
}

// SourceFiles are the globs for the language's source files.
type SourceFiles struct {
	Include []string `json:"include"`
	Exclude []string `json:"exclude"`
}

// AdapterName is how an adapter names itself in its greeting.
type AdapterName struct {
	Name    string `json:"name"`
	Version string `json:"version"`
}

// Hello is the greeting.
type Hello struct {
	Protocol       int               `json:"protocol"`
	Adapter        AdapterName       `json:"adapter"`
	BlockLanguages []string          `json:"blockLanguages"`
	SourceFiles    SourceFiles       `json:"sourceFiles"`
	IgnoredNames   []string          `json:"ignoredNames"`
	Capabilities   []string          `json:"capabilities"`
	Vocabulary     map[string]string `json:"vocabulary,omitempty"`
}
