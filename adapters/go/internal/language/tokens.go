package language

import (
	"go/scanner"
	"go/token"
)

// lexeme is one token of a piece of Go text.
type lexeme struct {
	Kind token.Token
	// Text is the token as written: an identifier's name, a literal, an operator.
	Text string
}

// scan scans a piece of Go text; ok is false when it is not Go's tokens.
func scan(text string) (tokens []lexeme, ok bool) {
	fset := token.NewFileSet()
	file := fset.AddFile("", fset.Base(), len(text))
	var s scanner.Scanner
	ok = true
	s.Init(file, []byte(text), func(token.Position, string) { ok = false }, 0)
	for {
		_, kind, literal := s.Scan()
		if kind == token.EOF {
			return tokens, ok
		}
		if kind == token.SEMICOLON && literal == "\n" {
			continue
		}
		if literal == "" {
			literal = kind.String()
		}
		tokens = append(tokens, lexeme{kind, literal})
	}
}

// Names are the identifiers of a piece of Go text that stand by themselves, in order:
// every identifier that does not follow a dot.
func Names(text string) []string {
	tokens, _ := scan(text)
	names := []string{}
	for k, t := range tokens {
		if t.Kind == token.IDENT && (k == 0 || tokens[k-1].Kind != token.PERIOD) {
			names = append(names, t.Text)
		}
	}
	return names
}

// Qualifiers are the identifiers of a piece of Go text that something is selected from
// (`strings` in `strings.ToUpper`), in order.
func Qualifiers(text string) []string {
	tokens, _ := scan(text)
	names := []string{}
	for k, t := range tokens {
		free := k == 0 || tokens[k-1].Kind != token.PERIOD
		if t.Kind == token.IDENT && free && k+1 < len(tokens) && tokens[k+1].Kind == token.PERIOD {
			names = append(names, t.Text)
		}
	}
	return names
}

// Spellings are the tokens of a piece of Go text as written, in order; what does not
// scan is spelled as far as it scans.
func Spellings(text string) []string {
	tokens, _ := scan(text)
	out := make([]string, len(tokens))
	for k, t := range tokens {
		out[k] = t.Text
	}
	return out
}
