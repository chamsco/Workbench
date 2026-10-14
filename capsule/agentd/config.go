package main

import (
	"encoding/json"
	"os"
	"path/filepath"
	"sync"
)

type Settings struct {
	Provider  string `json:"provider"`  // anthropic | openai-compatible
	APIKey    string `json:"api_key"`   // empty for ollama
	BaseURL   string `json:"base_url"`  // empty = provider default; http://localhost:11434/v1 for ollama
	Model     string `json:"model"`
	MaxTokens int    `json:"max_tokens"`
	// VM defaults
	DefaultProvider string `json:"default_provider"` // wsl | hyperv
	DefaultOS       string `json:"default_os"`
	DefaultCPUs     uint32 `json:"default_cpus"`
	DefaultMemory   uint32 `json:"default_memory"` // MiB
	DefaultDisk     uint32 `json:"default_disk"`   // GiB
}

func defaultSettings() *Settings {
	return &Settings{
		Provider:        "anthropic",
		Model:           "claude-sonnet-4-5",
		MaxTokens:       8192,
		DefaultProvider: "wsl",
		DefaultOS:       "ubuntu-24.04",
		DefaultCPUs:     2,
		DefaultMemory:   4096,
		DefaultDisk:     20,
	}
}

type Store struct {
	mu   sync.Mutex
	path string
	db   *DB
}

func dataDir() string {
	if v := os.Getenv("VISOR_HOME"); v != "" {
		return v
	}
	if v := os.Getenv("LOCALAPPDATA"); v != "" {
		return filepath.Join(v, "visor")
	}
	return ".visor"
}

func loadSettings() (*Settings, error) {
	s := defaultSettings()
	b, err := os.ReadFile(filepath.Join(dataDir(), "agentd.json"))
	if err != nil {
		if os.IsNotExist(err) {
			return s, nil
		}
		return nil, err
	}
	if err := json.Unmarshal(b, s); err != nil {
		return nil, err
	}
	return s, nil
}

func saveSettings(s *Settings) error {
	b, err := json.MarshalIndent(s, "", "  ")
	if err != nil {
		return err
	}
	if err := os.MkdirAll(dataDir(), 0o755); err != nil {
		return err
	}
	return os.WriteFile(filepath.Join(dataDir(), "agentd.json"), b, 0o600)
}
