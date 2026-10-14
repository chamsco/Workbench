package main

import (
	"context"
	"crypto/rand"
	"encoding/hex"
	"fmt"
	"time"
)

func shortID() string {
	b := make([]byte, 4)
	rand.Read(b)
	return hex.EncodeToString(b)
}

type Engine struct {
	core     *Core
	db       *DB
	privKey  string
	settings func() *Settings
}

// RunTask executes a headless task end-to-end: provision → agent → collect → teardown.
// Progress is written to the events table as it happens.
func (e *Engine) RunTask(ctx context.Context, id string) {
	t, err := e.db.getTask(id)
	if err != nil {
		return
	}
	sink := func(kind string, data any) { e.db.addEvent(id, kind, data) }
	setStatus := func(s string) {
		e.db.updateTask(id, s, t.Workspace, "", "", false)
		sink("status", s)
	}
	fail := func(err error) {
		e.db.updateTask(id, "error", t.Workspace, "", err.Error(), true)
		sink("error", err.Error())
		sink("done", "error")
	}

	setStatus("provisioning")

	s := e.settings()
	osName := t.OS
	if osName == "" {
		osName = s.DefaultOS
	}
	provider := t.Provider
	if provider == "" {
		provider = s.DefaultProvider
	}
	if provider == "hyperv" {
		if caps, err := e.core.Capabilities(); err != nil || !caps.HyperV || !caps.Elevated {
			sink("status", "hyperv unavailable, falling back to wsl")
			provider = "wsl"
		}
	}

	name := "task-" + id
	ws, err := e.core.Create(CreateOpts{
		Name: name, Provider: provider, OS: osName,
		CPUs: s.DefaultCPUs, Memory: s.DefaultMemory, Disk: s.DefaultDisk,
		SSHKey: e.privKey + ".pub",
	})
	if err != nil {
		fail(fmt.Errorf("provision: %w", err))
		return
	}
	t.Workspace = name
	defer func() {
		// events only — don't overwrite the final task status (done/error)
		sink("status", "tearing-down")
		if t.Policy == "keep" {
			e.core.Stop(name, true)
		} else {
			e.core.Delete(name)
		}
		sink("status", "teardown-complete")
	}()

	ws, err = e.core.Start(name)
	if err != nil {
		fail(fmt.Errorf("start: %w", err))
		return
	}

	// wait for the backend to become usable (cloud-init / ssh may lag)
	var backend ExecBackend
	deadline := time.Now().Add(3 * time.Minute)
	for time.Now().Before(deadline) {
		if ws.Provider == "hyperv" {
			if cur, err := e.core.Status(name); err == nil {
				ws = cur
			}
		}
		backend, err = backendFor(e.core, ws, e.privKey)
		if err == nil {
			if _, err = backend.Exec("true"); err == nil {
				break
			}
			backend.Close()
		}
		time.Sleep(3 * time.Second)
	}
	if err != nil {
		fail(fmt.Errorf("workspace not reachable: %w", err))
		return
	}
	defer backend.Close()

	setStatus("running")
	summary, _, err := runAgentLoop(ctx, provider_llm(e.settings()), backend, systemPrompt(ws),
		[]ChatMsg{{Role: "user", Text: t.Prompt}}, sink)
	if err != nil {
		fail(err)
		return
	}

	setStatus("collecting")
	e.db.updateTask(id, "done", name, summary, "", true)
	sink("done", "done")
}

// helper so tests can swap the llm; reads current settings each time
func provider_llm(s *Settings) LLMProvider {
	p, err := providerFromSettings(s)
	if err != nil {
		// return a provider that fails at Complete-time with the real error
		return &failingLLM{err: err}
	}
	return p
}

type failingLLM struct{ err error }

func (f *failingLLM) Name() string { return "none" }
func (f *failingLLM) Complete(ctx context.Context, _ string, _ []ChatMsg, _ []ToolDef) (*ChatMsg, error) {
	return nil, f.err
}
