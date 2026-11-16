package protocol

import (
	"bufio"
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"sync"

	"github.com/docattest/docattest/adapters/go/internal/protocoldata"
)

// Handler answers one method: the result, or an error (a UserError is the project's
// mistake; any other is the adapter's).
type Handler func(params json.RawMessage) (any, error)

// Session is the one project `open` creates.
type Session interface {
	// Methods are the session's methods by name.
	Methods() map[string]Handler
	// Close releases what the session holds.
	Close()
}

// Adapter is what a language gives the server to serve.
type Adapter struct {
	// Hello is the greeting, the same in every process.
	Hello Hello
	// Language are the questions about text, which need no session.
	Language map[string]Handler
	// Open creates the session.
	Open func(params json.RawMessage) (Session, error)
}

// UserError is a mistake in the project; the core prints its message as it is.
type UserError string

func (e UserError) Error() string { return string(e) }

// Decode reads a method's params into T; params that are left out read as `{}`.
func Decode[T any](raw json.RawMessage) (T, error) {
	var params T
	if len(bytes.TrimSpace(raw)) == 0 {
		raw = json.RawMessage("{}")
	}
	if err := json.Unmarshal(raw, &params); err != nil {
		return params, fmt.Errorf("params of the wrong shape: %w", err)
	}
	return params, nil
}

type request struct {
	ID     json.RawMessage `json:"id"`
	Method string          `json:"method"`
	Params json.RawMessage `json:"params"`
}

type server struct {
	adapter Adapter
	out     io.Writer
	writing sync.Mutex
	greeted bool
	phase   phase
	pending sync.WaitGroup
}

// phase is where a process is in the one session it serves: none opened yet, one open,
// or closed. Only an open session has methods.
type phase interface {
	// method is a session method's handler, or why there is none.
	method(name string) (Handler, error)
}

type noSession struct{}

type openSession struct {
	session Session
	methods map[string]Handler
}

type closedSession struct{}

func (noSession) method(name string) (Handler, error) {
	return nil, fmt.Errorf("%q needs a session, and none is open", name)
}

func (open openSession) method(name string) (Handler, error) {
	if handler, ok := open.methods[name]; ok {
		return handler, nil
	}
	return nil, fmt.Errorf("unknown method %q", name)
}

func (closedSession) method(name string) (Handler, error) {
	return nil, fmt.Errorf("%q after close: the session is closed", name)
}

// Serve answers the requests on `in`, one JSON value per line, until the input ends; it
// then writes the answers still in flight and closes the session.
func Serve(in io.Reader, out io.Writer, adapter Adapter) {
	s := &server{adapter: adapter, out: out, phase: noSession{}}
	reader := bufio.NewReaderSize(in, 1<<16)
	for {
		line, err := reader.ReadBytes('\n')
		if len(bytes.TrimSpace(line)) > 0 {
			s.receive(line)
		}
		if err != nil {
			break
		}
	}
	s.pending.Wait()
	s.closeSession()
}

// receive reads one line as a request and routes it; a line that is no request, or has no
// usable id, is answered with a null id.
func (s *server) receive(line []byte) {
	var req request
	if err := json.Unmarshal(line, &req); err != nil {
		s.fail(nil, fmt.Errorf("a line that is not a request: %w", err))
		return
	}
	if !usableID(req.ID) {
		s.fail(nil, fmt.Errorf("a request without a positive integer id"))
		return
	}
	s.route(req)
}

func usableID(raw json.RawMessage) bool {
	var id float64
	if len(raw) == 0 || json.Unmarshal(raw, &id) != nil {
		return false
	}
	return id > 0 && id == float64(int64(id))
}

func (s *server) route(req request) {
	switch {
	case req.Method == "hello":
		s.respond(req.ID, s.hello, req.Params)
	case !s.greeted:
		s.fail(req.ID, fmt.Errorf("%q before hello: hello comes first", req.Method))
	case req.Method == "open":
		s.respond(req.ID, s.open, req.Params)
	case req.Method == "close":
		s.respond(req.ID, s.close, req.Params)
	default:
		s.later(req)
	}
}

// later answers a request on its own goroutine, so that a slow one holds up no other.
func (s *server) later(req request) {
	handler, err := s.handler(req.Method)
	if err != nil {
		s.fail(req.ID, err)
		return
	}
	s.pending.Add(1)
	go func() {
		defer s.pending.Done()
		s.respond(req.ID, handler, req.Params)
	}()
}

func (s *server) handler(method string) (Handler, error) {
	if handler, ok := s.adapter.Language[method]; ok {
		return handler, nil
	}
	return s.phase.method(method)
}

func (s *server) hello(raw json.RawMessage) (any, error) {
	params, err := Decode[struct {
		Protocol float64 `json:"protocol"`
	}](raw)
	if err != nil {
		return nil, err
	}
	if spoken := protocoldata.Core().Protocol; params.Protocol != float64(spoken) {
		message := "docattest: the %s adapter speaks protocol %d, and the core asked for %v"
		return nil, UserError(fmt.Sprintf(message, s.adapter.Hello.Adapter.Name, spoken, params.Protocol))
	}
	s.greeted = true
	return s.adapter.Hello, nil
}

func (s *server) open(raw json.RawMessage) (any, error) {
	if _, fresh := s.phase.(noSession); !fresh {
		return nil, fmt.Errorf("a second open: one process serves one session")
	}
	session, err := s.adapter.Open(raw)
	if err != nil {
		return nil, err
	}
	s.phase = openSession{session: session, methods: session.Methods()}
	return struct{}{}, nil
}

// close waits for every request sent before it, then releases the session.
func (s *server) close(json.RawMessage) (any, error) {
	if _, isOpen := s.phase.(openSession); !isOpen {
		return nil, fmt.Errorf("close needs a session, and none is open")
	}
	s.pending.Wait()
	s.closeSession()
	return struct{}{}, nil
}

func (s *server) closeSession() {
	if open, isOpen := s.phase.(openSession); isOpen {
		open.session.Close()
		s.phase = closedSession{}
	}
}

// respond runs a handler and writes its answer; a handler that panics is the adapter's bug.
func (s *server) respond(id json.RawMessage, handler Handler, params json.RawMessage) {
	result, err := guarded(handler, params)
	if err != nil {
		s.fail(id, err)
		return
	}
	s.write(struct {
		ID     json.RawMessage `json:"id"`
		Result any             `json:"result"`
	}{id, result})
}

func guarded(handler Handler, params json.RawMessage) (result any, err error) {
	defer func() {
		if caught := recover(); caught != nil {
			err = fmt.Errorf("the adapter panicked: %v", caught)
		}
	}()
	return handler(params)
}

func (s *server) fail(id json.RawMessage, err error) {
	kind := "internal"
	if _, isUser := err.(UserError); isUser {
		kind = "user"
	}
	if id == nil {
		id = json.RawMessage("null")
	}
	s.write(struct {
		ID    json.RawMessage `json:"id"`
		Error any             `json:"error"`
	}{id, map[string]string{"kind": kind, "message": err.Error()}})
}

func (s *server) write(response any) {
	var line bytes.Buffer
	encoder := json.NewEncoder(&line)
	encoder.SetEscapeHTML(false)
	if err := encoder.Encode(response); err != nil {
		line.Reset()
		fmt.Fprintf(&line, `{"id":null,"error":{"kind":"internal","message":%q}}`+"\n", err.Error())
	}
	s.writing.Lock()
	defer s.writing.Unlock()
	_, _ = s.out.Write(line.Bytes())
}
