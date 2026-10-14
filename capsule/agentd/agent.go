package main

import (
	"context"
	"encoding/json"
	"fmt"
)

var agentTools = []ToolDef{
	{
		Name:        "exec",
		Description: "Run a shell command inside the workspace VM (sh -c on Linux, cmd /c on Windows). Returns exit code, stdout, stderr.",
		Schema:      json.RawMessage(`{"type":"object","properties":{"command":{"type":"string","description":"the full command line"}},"required":["command"]}`),
	},
	{
		Name:        "read_file",
		Description: "Read a text file from the workspace.",
		Schema:      json.RawMessage(`{"type":"object","properties":{"path":{"type":"string"}},"required":["path"]}`),
	},
	{
		Name:        "write_file",
		Description: "Write a text file in the workspace (overwrites).",
		Schema:      json.RawMessage(`{"type":"object","properties":{"path":{"type":"string"},"content":{"type":"string"}},"required":["path","content"]}`),
	},
}

const maxAgentIterations = 25
const maxToolOutput = 100_000

// EventSink receives agent progress events: kind = text|tool_call|tool_result|error
type EventSink func(kind string, data any)

func runAgentLoop(ctx context.Context, llm LLMProvider, backend ExecBackend, system string, history []ChatMsg, sink EventSink) (string, []ChatMsg, error) {
	msgs := append([]ChatMsg(nil), history...)
	for i := 0; i < maxAgentIterations; i++ {
		if err := ctx.Err(); err != nil {
			return "", msgs, err
		}
		resp, err := llm.Complete(ctx, system, msgs, agentTools)
		if err != nil {
			return "", msgs, err
		}
		msgs = append(msgs, *resp)
		if resp.Text != "" {
			sink("text", resp.Text)
		}
		if len(resp.ToolCalls) == 0 {
			return resp.Text, msgs, nil
		}
		for _, tc := range resp.ToolCalls {
			sink("tool_call", map[string]any{"name": tc.Name, "input": json.RawMessage(tc.Input)})
			out := execTool(backend, tc)
			sink("tool_result", map[string]any{"name": tc.Name, "output": truncate(out, 2000)})
			msgs = append(msgs, ChatMsg{Role: "tool", ToolCallID: tc.ID, Text: out})
		}
	}
	return "", msgs, fmt.Errorf("agent did not finish within %d iterations", maxAgentIterations)
}

func execTool(backend ExecBackend, tc ToolCall) string {
	switch tc.Name {
	case "exec":
		var in struct {
			Command string `json:"command"`
		}
		if err := json.Unmarshal(tc.Input, &in); err != nil || in.Command == "" {
			return "error: invalid input for exec"
		}
		res, err := backend.Exec(in.Command)
		if err != nil {
			return "error: " + err.Error()
		}
		s := fmt.Sprintf("exit_code: %d\nstdout:\n%s\nstderr:\n%s", res.ExitCode, res.Stdout, res.Stderr)
		return truncate(s, maxToolOutput)
	case "read_file":
		var in struct {
			Path string `json:"path"`
		}
		if err := json.Unmarshal(tc.Input, &in); err != nil || in.Path == "" {
			return "error: invalid input for read_file"
		}
		content, err := backend.ReadFile(in.Path)
		if err != nil {
			return "error: " + err.Error()
		}
		return truncate(content, maxToolOutput)
	case "write_file":
		var in struct {
			Path    string `json:"path"`
			Content string `json:"content"`
		}
		if err := json.Unmarshal(tc.Input, &in); err != nil || in.Path == "" {
			return "error: invalid input for write_file"
		}
		if err := backend.WriteFile(in.Path, in.Content); err != nil {
			return "error: " + err.Error()
		}
		return "ok"
	default:
		return "error: unknown tool " + tc.Name
	}
}

func systemPrompt(ws *Workspace) string {
	osDesc := ws.OS
	if ws.Provider == "wsl" {
		osDesc += " (running as a WSL2 distribution; you are root)"
	} else {
		osDesc += " (Hyper-V VM; you connect over SSH)"
	}
	return fmt.Sprintf(`You are colony's agent, operating autonomously inside an isolated VM workspace.

Workspace: %s — %s

You have three tools: exec (run shell commands), read_file, write_file.
Work methodically: check the state of the machine before changing it, verify
your actions afterwards, and install whatever you need (apt is available on
Ubuntu workspaces). When the task is done, reply with a concise summary of
what you did and any results or artifacts the user asked for. Do not ask
questions — make reasonable assumptions and state them.`, ws.Name, osDesc)
}
