package cells

import "regexp"

// raw is what CommonMark reads instead of a code span when it starts first: an autolink, or
// an HTML tag, comment, processing instruction, declaration or CDATA section. The patterns
// are the spec's own.
var raw = regexp.MustCompile(`^(?:` + autolink + `|` + email + `|` + openTag + `|` + closingTag +
	`|<!-->|<!--->|<!--(?s:.)*?-->|<\?(?s:.)*?\?>|<![A-Za-z][^>]*>|<!\[CDATA\[(?s:.)*?\]\]>)`)

const (
	autolink       = `<[A-Za-z][A-Za-z0-9+.-]{1,31}:[^<>\x00-\x20]*>`
	email          = `<[A-Za-z0-9.!#$%&'*+/=?^_` + "`" + `{|}~-]+@` + domainLabel + `(?:\.` + domainLabel + `)*>`
	domainLabel    = `[A-Za-z0-9](?:[A-Za-z0-9-]{0,61}[A-Za-z0-9])?`
	tagName        = `[A-Za-z][A-Za-z0-9-]*`
	space          = `[ \t\n]`
	attributeValue = `(?:[^"'=<>` + "`" + `\x00-\x20]+|'[^']*'|"[^"]*")`
	attribute      = space + `+[A-Za-z_:][A-Za-z0-9_.:-]*(?:` + space + `*=` + space + `*` + attributeValue + `)?`
	openTag        = `<` + tagName + `(?:` + attribute + `)*` + space + `*/?>`
	closingTag     = `</` + tagName + space + `*>`
)

// rawLength is the length of the autolink or the HTML that starts `text`, or 0.
func rawLength(text string) int {
	return len(raw.FindString(text))
}
