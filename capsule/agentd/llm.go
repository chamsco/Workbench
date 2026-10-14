package main

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"time"
)

// ---- neutral chat model ----

type ToolDef struct {
	Name        string          `json:"name"`
	Description string          `json:"description"`
	Schema      json.RawMessage `json:"schema"`
}

type ToolCall struct {
	ID    string          `json:"id"`
	Name  string          `json:"name"`
	Input json.RawMessage `json:"input"`
}

type ChatMsg struct {
	Role       string     `json:"role"` // user | assistant | tool
	Text       string     `json:"text,omitempty"`
	ToolCalls  []ToolCall `json:"tool_calls,omitempty"`
	ToolCallID string     `json:"tool_call_id,omitempty"` // role=tool
}

type LLMProvider interface {
	Complete(ctx context.Context, system string, msgs []ChatMsg, tools []ToolDef) (*ChatMsg, error)
	Name() string
}

func providerFromSettings(s *Settings) (LLMProvider, error) {
	switch s.Provider {
	case "anthropic":
		if s.APIKey == "" {
			return nil, fmt.Errorf("anthropic api_key not set (PUT /api/settings)")
		}
		return &anthropicProvider{key: s.APIKey, model: s.Model, maxTokens: s.MaxTokens}, nil
	case "openai-compatible":
		base := s.BaseURL
		if base == "" {
			base = "https://api.openai.com/v1"
		}
		return &openAIProvider{key: s.APIKey, base: base, model: s.Model, maxTokens: s.MaxTokens}, nil
	default:
		return nil, fmt.Errorf("unknown llm provider %q", s.Provider)
	}
}

var httpClient = &http.Client{Timeout: 5 * time.Minute}

func doJSON(ctx context.Context, req *http.Request, out any) error {
	resp, err := httpClient.Do(req)
	if err != nil {
		return err
	}
	defer resp.Body.Close()
	body, _ := io.ReadAll(io.LimitReader(resp.Body, 32<<20))
	if resp.StatusCode >= 300 {
		return fmt.Errorf("%s %s: %d: %s", req.Method, req.URL, resp.StatusCode, truncate(string(body), 800))
	}
	return json.Unmarshal(body, out)
}

func truncate(s string, n int) string {
	if len(s) > n {
		return s[:n] + "…"
	}
	return s
}

// ---- Anthropic ----

type anthropicProvider struct {
	key, model string
	maxTokens  int
}

func (p *anthropicProvider) Name() string { return "anthropic/" + p.model }

func (p *anthropicProvider) Complete(ctx context.Context, system string, msgs []ChatMsg, tools []ToolDef) (*ChatMsg, error) {
	type content struct {
		Type  string          `json:"type"`
		Text  string          `json:"text,omitempty"`
		ID    string          `json:"id,omitempty"`
		Name  string          `json:"name,omitempty"`
		Input json.RawMessage `json:"input,omitempty"`
		// tool_result
		ToolUseID string `json:"tool_use_id,omitempty"`
		Content   string `json:"content,omitempty"`
	}
	type message struct {
		Role    string    `json:"role"`
		Content []content `json:"content"`
	}
	type tool struct {
		Name        string          `json:"name"`
		Description string          `json:"description"`
		InputSchema json.RawMessage `json:"input_schema"`
	}

	var amsgs []message
	for _, m := range msgs {
		switch m.Role {
		case "user":
			amsgs = append(amsgs, message{Role: "user", Content: []content{{Type: "text", Text: m.Text}}})
		case "assistant":
			var cs []content
			if m.Text != "" {
				cs = append(cs, content{Type: "text", Text: m.Text})
			}
			for _, tc := range m.ToolCalls {
				cs = append(cs, content{Type: "tool_use", ID: tc.ID, Name: tc.Name, Input: tc.Input})
			}
			amsgs = append(amsgs, message{Role: "assistant", Content: cs})
		case "tool":
			amsgs = append(amsgs, message{Role: "user", Content: []content{
				{Type: "tool_result", ToolUseID: m.ToolCallID, Content: m.Text},
			}})
		}
	}

	var atools []tool
	for _, t := range tools {
		atools = append(atools, tool{Name: t.Name, Description: t.Description, InputSchema: t.Schema})
	}

	body := map[string]any{
		"model":      p.model,
		"max_tokens": p.maxTokens,
		"system":     system,
		"messages":   amsgs,
	}
	if len(atools) > 0 {
		body["tools"] = atools
	}
	b, _ := json.Marshal(body)
	req, _ := http.NewRequestWithContext(ctx, "POST", "https://api.anthropic.com/v1/messages", bytes.NewReader(b))
	req.Header.Set("content-type", "application/json")
	req.Header.Set("x-api-key", p.key)
	req.Header.Set("anthropic-version", "2023-06-01")

	var out struct {
		Content []struct {
			Type  string          `json:"type"`
			Text  string          `json:"text"`
			ID    string          `json:"id"`
			Name  string          `json:"name"`
			Input json.RawMessage `json:"input"`
		} `json:"content"`
	}
	if err := doJSON(ctx, req, &out); err != nil {
		return nil, err
	}
	res := &ChatMsg{Role: "assistant"}
	for _, c := range out.Content {
		switch c.Type {
		case "text":
			res.Text += c.Text
		case "tool_use":
			res.ToolCalls = append(res.ToolCalls, ToolCall{ID: c.ID, Name: c.Name, Input: c.Input})
		}
	}
	return res, nil
}

// ---- OpenAI-compatible (OpenAI, Ollama, LM Studio, …) ----

type openAIProvider struct {
	key, base, model string
	maxTokens        int
}

func (p *openAIProvider) Name() string { return "openai-compatible/" + p.model }

func (p *openAIProvider) Complete(ctx context.Context, system string, msgs []ChatMsg, tools []ToolDef) (*ChatMsg, error) {
	type oaToolCall struct {
		ID       string `json:"id"`
		Type     string `json:"type"`
		Function struct {
			Name      string `json:"name"`
			Arguments string `json:"arguments"`
		} `json:"function"`
	}
	type oaMsg struct {
		Role       string       `json:"role"`
		Content    string       `json:"content,omitempty"`
		ToolCalls  []oaToolCall `json:"tool_calls,omitempty"`
		ToolCallID string       `json:"tool_call_id,omitempty"`
	}

	omsgs := []oaMsg{{Role: "system", Content: system}}
	for _, m := range msgs {
		switch m.Role {
		case "user":
			omsgs = append(omsgs, oaMsg{Role: "user", Content: m.Text})
		case "assistant":
			om := oaMsg{Role: "assistant", Content: m.Text}
			for _, tc := range m.ToolCalls {
				var c oaToolCall
				c.ID, c.Type = tc.ID, "function"
				c.Function.Name = tc.Name
				c.Function.Arguments = string(tc.Input)
				om.ToolCalls = append(om.ToolCalls, c)
			}
			omsgs = append(omsgs, om)
		case "tool":
			omsgs = append(omsgs, oaMsg{Role: "tool", Content: m.Text, ToolCallID: m.ToolCallID})
		}
	}

	var otools []map[string]any
	for _, t := range tools {
		var schema any
		json.Unmarshal(t.Schema, &schema)
		otools = append(otools, map[string]any{
			"type": "function",
			"function": map[string]any{
				"name":        t.Name,
				"description": t.Description,
				"parameters":  schema,
			},
		})
	}

	body := map[string]any{"model": p.model, "messages": omsgs}
	if len(otools) > 0 {
		body["tools"] = otools
	}
	if p.maxTokens > 0 {
		body["max_tokens"] = p.maxTokens
	}
	b, _ := json.Marshal(body)
	req, _ := http.NewRequestWithContext(ctx, "POST", p.base+"/chat/completions", bytes.NewReader(b))
	req.Header.Set("content-type", "application/json")
	if p.key != "" {
		req.Header.Set("authorization", "Bearer "+p.key)
	}

	var out struct {
		Choices []struct {
			Message struct {
				Content   string `json:"content"`
				ToolCalls []struct {
					ID       string `json:"id"`
					Function struct {
						Name      string `json:"name"`
						Arguments string `json:"arguments"`
					} `json:"function"`
				} `json:"tool_calls"`
			} `json:"message"`
		} `json:"choices"`
	}
	if err := doJSON(ctx, req, &out); err != nil {
		return nil, err
	}
	if len(out.Choices) == 0 {
		return nil, fmt.Errorf("no choices in response")
	}
	res := &ChatMsg{Role: "assistant", Text: out.Choices[0].Message.Content}
	for _, tc := range out.Choices[0].Message.ToolCalls {
		res.ToolCalls = append(res.ToolCalls, ToolCall{
			ID: tc.ID, Name: tc.Function.Name, Input: json.RawMessage(tc.Function.Arguments),
		})
	}
	return res, nil
}
