package main

import (
	"context"
	"encoding/json"
	"fmt"
	"net"
	"net/http"
	osExec "os/exec"
	"strconv"
	"strings"
	"time"
)

type Server struct {
	core     *Core
	db       *DB
	engine   *Engine
	privKey  string
	settings *Settings
	mu       chan struct{} // settings reload gate
}

func writeJSON(w http.ResponseWriter, code int, v any) {
	w.Header().Set("content-type", "application/json")
	w.WriteHeader(code)
	json.NewEncoder(w).Encode(v)
}

func writeErr(w http.ResponseWriter, code int, err error) {
	writeJSON(w, code, map[string]string{"error": err.Error()})
}

type sseWriter struct {
	w  http.ResponseWriter
	fl http.Flusher
}

func newSSE(w http.ResponseWriter) (*sseWriter, error) {
	fl, ok := w.(http.Flusher)
	if !ok {
		return nil, fmt.Errorf("streaming unsupported")
	}
	w.Header().Set("content-type", "text/event-stream")
	w.Header().Set("cache-control", "no-cache")
	w.Header().Set("connection", "keep-alive")
	return &sseWriter{w, fl}, nil
}

func (s *sseWriter) send(kind string, data any) {
	b, _ := json.Marshal(data)
	fmt.Fprintf(s.w, "event: %s\ndata: %s\n\n", kind, b)
	s.fl.Flush()
}

func (srv *Server) routes() *http.ServeMux {
	mux := http.NewServeMux()
	mux.HandleFunc("GET /api/capabilities", srv.hCapabilities)
	mux.HandleFunc("GET /api/workspaces", srv.hListWorkspaces)
	mux.HandleFunc("POST /api/workspaces", srv.hCreateWorkspace)
	mux.HandleFunc("GET /api/workspaces/{name}", srv.hWorkspaceStatus)
	mux.HandleFunc("POST /api/workspaces/{name}/start", srv.hStartWorkspace)
	mux.HandleFunc("POST /api/workspaces/{name}/stop", srv.hStopWorkspace)
	mux.HandleFunc("DELETE /api/workspaces/{name}", srv.hDeleteWorkspace)
	mux.HandleFunc("GET /api/remotes", srv.hListRemotes)
	mux.HandleFunc("POST /api/remotes", srv.hCreateRemote)
	mux.HandleFunc("POST /api/remotes/{id}/test", srv.hTestRemote)
	mux.HandleFunc("POST /api/remotes/{id}/open", srv.hOpenRemote)
	mux.HandleFunc("POST /api/chat", srv.hChat)
	mux.HandleFunc("GET /api/tasks", srv.hListTasks)
	mux.HandleFunc("POST /api/tasks", srv.hCreateTask)
	mux.HandleFunc("GET /api/tasks/{id}", srv.hGetTask)
	mux.HandleFunc("GET /api/tasks/{id}/events", srv.hTaskEvents)
	mux.HandleFunc("GET /api/settings", srv.hGetSettings)
	mux.HandleFunc("PUT /api/settings", srv.hPutSettings)
	mux.Handle("/", staticHandler())
	return mux
}

func (srv *Server) hCapabilities(w http.ResponseWriter, r *http.Request) {
	caps, err := srv.core.Capabilities()
	if err != nil {
		writeErr(w, 500, err)
		return
	}
	writeJSON(w, 200, caps)
}

func (srv *Server) hListWorkspaces(w http.ResponseWriter, r *http.Request) {
	ws, err := srv.core.List()
	if err != nil {
		writeErr(w, 500, err)
		return
	}
	remotes, err := srv.db.listRemotes()
	if err != nil {
		writeErr(w, 500, err)
		return
	}
	for i := range remotes {
		ws = append(ws, remoteWorkspace(&remotes[i]))
	}
	writeJSON(w, 200, ws)
}

func (srv *Server) hCreateWorkspace(w http.ResponseWriter, r *http.Request) {
	var in struct {
		Name     string `json:"name"`
		Provider string `json:"provider"`
		OS       string `json:"os"`
		CPUs     uint32 `json:"cpus"`
		Memory   uint32 `json:"memory"`
		Disk     uint32 `json:"disk"`
	}
	if err := json.NewDecoder(r.Body).Decode(&in); err != nil {
		writeErr(w, 400, err)
		return
	}
	s := srv.settings
	if in.Provider == "" {
		in.Provider = s.DefaultProvider
	}
	if in.OS == "" {
		in.OS = s.DefaultOS
	}
	if in.CPUs == 0 {
		in.CPUs = s.DefaultCPUs
	}
	if in.Memory == 0 {
		in.Memory = s.DefaultMemory
	}
	if in.Disk == 0 {
		in.Disk = s.DefaultDisk
	}
	ws, err := srv.core.Create(CreateOpts{
		Name: in.Name, Provider: in.Provider, OS: in.OS,
		CPUs: in.CPUs, Memory: in.Memory, Disk: in.Disk, SSHKey: srv.privKey + ".pub",
	})
	if err != nil {
		writeErr(w, 500, err)
		return
	}
	writeJSON(w, 201, ws)
}

func (srv *Server) hWorkspaceStatus(w http.ResponseWriter, r *http.Request) {
	ws, err := srv.workspace(r.PathValue("name"))
	if err != nil {
		writeErr(w, 404, err)
		return
	}
	writeJSON(w, 200, ws)
}

func (srv *Server) hStartWorkspace(w http.ResponseWriter, r *http.Request) {
	if remote, err := srv.db.getRemote(r.PathValue("name")); err == nil {
		state, testErr := srv.testRemote(remote)
		if testErr != nil {
			writeErr(w, 502, testErr)
			return
		}
		ws := remoteWorkspace(remote)
		ws.State = state
		writeJSON(w, 200, ws)
		return
	}
	ws, err := srv.core.Start(r.PathValue("name"))
	if err != nil {
		writeErr(w, 500, err)
		return
	}
	writeJSON(w, 200, ws)
}

func (srv *Server) hStopWorkspace(w http.ResponseWriter, r *http.Request) {
	if _, err := srv.db.getRemote(r.PathValue("name")); err == nil {
		writeJSON(w, 200, map[string]string{"status": "remote connection remains available"})
		return
	}
	if err := srv.core.Stop(r.PathValue("name"), true); err != nil {
		writeErr(w, 500, err)
		return
	}
	writeJSON(w, 200, map[string]string{"status": "stopped"})
}

func (srv *Server) hDeleteWorkspace(w http.ResponseWriter, r *http.Request) {
	if _, err := srv.db.getRemote(r.PathValue("name")); err == nil {
		if err := srv.db.deleteRemote(r.PathValue("name")); err != nil {
			writeErr(w, 500, err)
			return
		}
		w.WriteHeader(204)
		return
	}
	if err := srv.core.Delete(r.PathValue("name")); err != nil {
		writeErr(w, 500, err)
		return
	}
	w.WriteHeader(204)
}

func (srv *Server) workspace(name string) (*Workspace, error) {
	if remote, err := srv.db.getRemote(name); err == nil {
		ws := remoteWorkspace(remote)
		return &ws, nil
	}
	return srv.core.Status(name)
}

func (srv *Server) hListRemotes(w http.ResponseWriter, r *http.Request) {
	remotes, err := srv.db.listRemotes()
	if err != nil {
		writeErr(w, 500, err)
		return
	}
	writeJSON(w, 200, remotes)
}

func (srv *Server) hCreateRemote(w http.ResponseWriter, r *http.Request) {
	var in struct {
		Name     string `json:"name"`
		Kind     string `json:"kind"`
		Host     string `json:"host"`
		Port     uint16 `json:"port"`
		Username string `json:"username"`
		KeyPath  string `json:"key_path"`
		OS       string `json:"os"`
	}
	if err := json.NewDecoder(r.Body).Decode(&in); err != nil {
		writeErr(w, 400, err)
		return
	}
	if in.Name == "" || in.Host == "" {
		writeErr(w, 400, fmt.Errorf("name and host are required"))
		return
	}
	if in.Kind != "ssh" && in.Kind != "rdp" {
		writeErr(w, 400, fmt.Errorf("kind must be ssh or rdp"))
		return
	}
	if in.Port == 0 {
		if in.Kind == "ssh" {
			in.Port = 22
		} else {
			in.Port = 3389
		}
	}
	if in.OS == "" {
		in.OS = "linux"
	}
	remote := &Remote{ID: "remote-" + shortID(), Name: in.Name, Kind: in.Kind, Host: in.Host, Port: in.Port, Username: in.Username, KeyPath: in.KeyPath, OS: in.OS}
	if err := srv.db.createRemote(remote); err != nil {
		writeErr(w, 409, fmt.Errorf("save remote: %w", err))
		return
	}
	writeJSON(w, 201, remoteWorkspace(remote))
}

func (srv *Server) testRemote(remote *Remote) (string, error) {
	address := net.JoinHostPort(remote.Host, strconv.Itoa(int(remote.Port)))
	conn, err := net.DialTimeout("tcp", address, 4*time.Second)
	if err != nil {
		return "unreachable", fmt.Errorf("connect to %s: %w", address, err)
	}
	conn.Close()
	if remote.Kind == "ssh" {
		ws := remoteWorkspace(remote)
		backend, err := DialSSH(&ws, remote.KeyPath, remote.OS, 8*time.Second)
		if err != nil {
			return "reachable", fmt.Errorf("ssh authentication failed: %w", err)
		}
		defer backend.Close()
		if _, err := backend.Exec("true"); err != nil {
			return "reachable", err
		}
	}
	return "online", nil
}

func (srv *Server) hTestRemote(w http.ResponseWriter, r *http.Request) {
	remote, err := srv.db.getRemote(r.PathValue("id"))
	if err != nil {
		writeErr(w, 404, err)
		return
	}
	state, err := srv.testRemote(remote)
	if err != nil {
		writeErr(w, 502, err)
		return
	}
	ws := remoteWorkspace(remote)
	ws.State = state
	writeJSON(w, 200, ws)
}

func (srv *Server) hOpenRemote(w http.ResponseWriter, r *http.Request) {
	remote, err := srv.db.getRemote(r.PathValue("id"))
	if err != nil {
		writeErr(w, 404, err)
		return
	}
	if remote.Kind != "rdp" {
		writeErr(w, 400, fmt.Errorf("open remote desktop is only available for RDP connections"))
		return
	}
	address := net.JoinHostPort(remote.Host, strconv.Itoa(int(remote.Port)))
	if err := osExec.Command("mstsc.exe", "/v:"+address).Start(); err != nil {
		writeErr(w, 500, fmt.Errorf("launch Remote Desktop: %w", err))
		return
	}
	writeJSON(w, 200, map[string]string{"status": "remote desktop launched", "address": address})
}

// POST /api/chat {session_id?, workspace, message} — SSE stream of agent events.
func (srv *Server) hChat(w http.ResponseWriter, r *http.Request) {
	var in struct {
		SessionID string `json:"session_id"`
		Workspace string `json:"workspace"`
		Message   string `json:"message"`
	}
	if err := json.NewDecoder(r.Body).Decode(&in); err != nil || in.Message == "" || in.Workspace == "" {
		writeErr(w, 400, fmt.Errorf("need workspace and message"))
		return
	}
	sse, err := newSSE(w)
	if err != nil {
		writeErr(w, 500, err)
		return
	}

	sessionID := in.SessionID
	if sessionID == "" {
		sessionID = shortID() + shortID()
		srv.db.createSession(sessionID, in.Workspace)
	} else if wsName, err := srv.db.sessionWorkspace(sessionID); err == nil && wsName != in.Workspace {
		sse.send("error", "session belongs to workspace "+wsName)
		return
	}
	sse.send("session", map[string]string{"session_id": sessionID})

	ws, err := srv.workspace(in.Workspace)
	if err != nil {
		sse.send("error", err.Error())
		return
	}
	if ws.Provider == "remote-rdp" {
		sse.send("error", "RDP connections do not provide terminal access; use open remote desktop")
		return
	}
	if ws.State != "running" && ws.Provider != "wsl" && ws.Provider != "remote-ssh" {
		sse.send("error", "workspace not running")
		return
	}
	backend, err := backendFor(srv.core, ws, srv.privKey)
	if err != nil {
		sse.send("error", err.Error())
		return
	}
	defer backend.Close()

	history, _ := srv.db.loadMessages(sessionID)
	userMsg := ChatMsg{Role: "user", Text: in.Message}
	srv.db.saveMessage(sessionID, userMsg)
	history = append(history, userMsg)

	llm, err := providerFromSettings(srv.settings)
	if err != nil {
		sse.send("error", err.Error())
		return
	}

	sink := func(kind string, data any) { sse.send(kind, data) }
	_, msgs, err := runAgentLoop(r.Context(), llm, backend, systemPrompt(ws), history, sink)
	if err != nil {
		sse.send("error", err.Error())
	}
	// persist everything after the user message
	for _, m := range msgs[len(history):] {
		srv.db.saveMessage(sessionID, m)
	}
	sse.send("done", map[string]string{"session_id": sessionID})
}

func (srv *Server) hCreateTask(w http.ResponseWriter, r *http.Request) {
	var in struct {
		Prompt   string `json:"prompt"`
		OS       string `json:"os"`
		Provider string `json:"provider"`
		Policy   string `json:"policy"`
	}
	if err := json.NewDecoder(r.Body).Decode(&in); err != nil || in.Prompt == "" {
		writeErr(w, 400, fmt.Errorf("need prompt"))
		return
	}
	if in.Policy == "" {
		in.Policy = "destroy"
	}
	t := &Task{ID: shortID(), Prompt: in.Prompt, OS: in.OS, Provider: in.Provider, Policy: in.Policy, Status: "queued"}
	if err := srv.db.createTask(t); err != nil {
		writeErr(w, 500, err)
		return
	}
	srv.db.addEvent(t.ID, "status", "queued")
	go srv.engine.RunTask(context.Background(), t.ID)
	writeJSON(w, 202, t)
}

func (srv *Server) hListTasks(w http.ResponseWriter, r *http.Request) {
	tasks, err := srv.db.listTasks()
	if err != nil {
		writeErr(w, 500, err)
		return
	}
	writeJSON(w, 200, tasks)
}

func (srv *Server) hGetTask(w http.ResponseWriter, r *http.Request) {
	t, err := srv.db.getTask(r.PathValue("id"))
	if err != nil {
		writeErr(w, 404, err)
		return
	}
	writeJSON(w, 200, t)
}

// GET /api/tasks/{id}/events?after=N — SSE replay + live tail until done.
func (srv *Server) hTaskEvents(w http.ResponseWriter, r *http.Request) {
	sse, err := newSSE(w)
	if err != nil {
		writeErr(w, 500, err)
		return
	}
	id := r.PathValue("id")
	var after int64
	fmt.Sscan(r.URL.Query().Get("after"), &after)
	for {
		events, err := srv.db.eventsSince(id, after)
		if err != nil {
			sse.send("error", err.Error())
			return
		}
		done := false
		for _, e := range events {
			sse.send(e.Kind, json.RawMessage(quoteIfNeeded(e.Data)))
			after = e.ID
			if e.Kind == "done" {
				done = true
			}
		}
		if done {
			return
		}
		select {
		case <-r.Context().Done():
			return
		case <-time.After(700 * time.Millisecond):
		}
	}
}

// events store plain strings; SSE sends JSON — wrap bare strings in quotes.
func quoteIfNeeded(s string) string {
	if strings.HasPrefix(s, "{") || strings.HasPrefix(s, "[") || strings.HasPrefix(s, "\"") {
		return s
	}
	b, _ := json.Marshal(s)
	return string(b)
}

func (srv *Server) hGetSettings(w http.ResponseWriter, r *http.Request) {
	masked := *srv.settings
	if masked.APIKey != "" {
		masked.APIKey = masked.APIKey[:4] + "…"
	}
	writeJSON(w, 200, &masked)
}

func (srv *Server) hPutSettings(w http.ResponseWriter, r *http.Request) {
	var in Settings
	if err := json.NewDecoder(r.Body).Decode(&in); err != nil {
		writeErr(w, 400, err)
		return
	}
	if strings.HasSuffix(in.APIKey, "…") || in.APIKey == "" {
		in.APIKey = srv.settings.APIKey // keep existing
	}
	if err := saveSettings(&in); err != nil {
		writeErr(w, 500, err)
		return
	}
	srv.settings = &in
	writeJSON(w, 200, map[string]string{"status": "saved"})
}
