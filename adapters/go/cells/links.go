package cells

import "regexp"

// linkTail is what makes a bracketed text a link: `(destination "title")`, either part
// optional, as CommonMark has it. Its backticks are the link's, not a code span's.
var linkTail = regexp.MustCompile(`^\(` + space + `*(?:` + destination + `)?` +
	`(?:` + space + `+(?:` + title + `))?` + space + `*\)`)

const (
	escaped     = `\\[[:punct:]]|\\`
	destination = `<(?:[^<>\n\\]|` + escaped + `)*>|` +
		`(?:[^\x00-\x20()\\]|` + escaped + `|\((?:[^\x00-\x20()\\]|` + escaped + `)*\))+`
	title = `"(?:[^"\\]|` + escaped + `)*"|'(?:[^'\\]|` + escaped + `)*'|\((?:[^()\\]|` + escaped + `)*\)`
)

// opener is a `[` or `![` that a later `]` may close into a link or an image.
type opener struct {
	image  bool
	active bool
}

// brackets are the openers not yet closed, innermost last.
type brackets []opener

// open records the opener of a link (`[`) or an image (`![`).
func (b *brackets) open(image bool) {
	*b = append(*b, opener{image: image, active: true})
}

// close closes the innermost opener with the `]` that `rest` follows, and returns the
// length of the link's tail when the two make a link, else 0. A link cannot hold a link,
// so the link openers around one that closes into a link become text.
func (b *brackets) close(rest string) int {
	if len(*b) == 0 {
		return 0
	}
	innermost := (*b)[len(*b)-1]
	*b = (*b)[:len(*b)-1]
	tail := len(linkTail.FindString(rest))
	if !innermost.active || tail == 0 {
		return 0
	}
	if innermost.image {
		return tail
	}
	for i := range *b {
		if !(*b)[i].image {
			(*b)[i].active = false
		}
	}
	return tail
}
