package main

import (
	"bytes"
	"encoding/json"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
)

// ---- visor-core CLI client ----

type Workspace struct {
	Name      string `json:"name"`
	Provider  string `json:"provider"`
	Kind      string `json:"kind,omitempty"` // local | remote
	OS        string `json:"os"`
	State     string `json:"state"`
	IP        string `json:"ip,omitempty"`
	SSHUser   string `json:"ssh_user,omitempty"`
	SSHPort   uint16 `json:"ssh_port,omitempty"`
	Host      string `json:"host,omitempty"`
	Port      uint16 `json:"port,omitempty"`
	Username  string `json:"username,omitempty"`
	KeyPath   string `json:"key_path,omitempty"`
	RemoteID  string `json:"remote_id,omitempty"`
	CPUs      uint32 `json:"cpus"`
	MemoryMiB uint32 `json:"memory_mib"`
	DiskGiB   uint32 `json:"disk_gib"`
}

type Capabilities struct {
	HyperV   bool `json:"hyperv"`
	WSL      bool `json:"wsl"`
	Elevated bool `json:"elevated"`
}

type Core struct {
	Bin string
}

func NewCore() *Core {
	bin := os.Getenv("VISOR_CORE")
	if bin == "" {
		// next to agentd.exe first, then PATH
		if exe, err := os.Executable(); err == nil {
			cand := filepath.Join(filepath.Dir(exe), "visor-core.exe")
			if _, err := os.Stat(cand); err == nil {
				bin = cand
			}
		}
		if bin == "" {
			if p, err := exec.LookPath("visor-core.exe"); err == nil {
				bin = p
			} else {
				bin = "visor-core.exe"
			}
		}
	}
	return &Core{Bin: bin}
}

func (c *Core) run(args ...string) (json.RawMessage, error) {
	cmd := exec.Command(c.Bin, args...)
	var stdout, stderr bytes.Buffer
	cmd.Stdout, cmd.Stderr = &stdout, &stderr
	if err := cmd.Run(); err != nil {
		return nil, fmt.Errorf("visor-core %v: %v: %s", args[:1], err, stderr.String())
	}
	return json.RawMessage(stdout.Bytes()), nil
}

func (c *Core) Capabilities() (*Capabilities, error) {
	raw, err := c.run("capabilities")
	if err != nil {
		return nil, err
	}
	var caps Capabilities
	return &caps, json.Unmarshal(raw, &caps)
}

func (c *Core) List() ([]Workspace, error) {
	raw, err := c.run("list")
	if err != nil {
		return nil, err
	}
	var ws []Workspace
	return ws, json.Unmarshal(raw, &ws)
}

type CreateOpts struct {
	Name     string
	Provider string
	OS       string
	CPUs     uint32
	Memory   uint32
	Disk     uint32
	SSHKey   string
}

func (c *Core) Create(o CreateOpts) (*Workspace, error) {
	args := []string{"create", "--provider", o.Provider, "--name", o.Name, "--os", o.OS,
		"--cpus", fmt.Sprint(o.CPUs), "--memory", fmt.Sprint(o.Memory), "--disk", fmt.Sprint(o.Disk)}
	if o.SSHKey != "" {
		args = append(args, "--ssh-key", o.SSHKey)
	}
	raw, err := c.run(args...)
	if err != nil {
		return nil, err
	}
	var ws Workspace
	return &ws, json.Unmarshal(raw, &ws)
}

func (c *Core) Start(name string) (*Workspace, error) {
	raw, err := c.run("start", name)
	if err != nil {
		return nil, err
	}
	var ws Workspace
	return &ws, json.Unmarshal(raw, &ws)
}

func (c *Core) Stop(name string, force bool) error {
	args := []string{"stop", name}
	if force {
		args = append(args, "--force")
	}
	_, err := c.run(args...)
	return err
}

func (c *Core) Delete(name string) error {
	_, err := c.run("delete", name)
	return err
}

func (c *Core) Status(name string) (*Workspace, error) {
	raw, err := c.run("status", name)
	if err != nil {
		return nil, err
	}
	var ws Workspace
	return &ws, json.Unmarshal(raw, &ws)
}

type ExecResult struct {
	ExitCode int    `json:"exit_code"`
	Stdout   string `json:"stdout"`
	Stderr   string `json:"stderr"`
}

func (c *Core) ExecWSL(name string, argv ...string) (*ExecResult, error) {
	args := append([]string{"exec", name}, argv...)
	raw, err := c.run(args...)
	if err != nil {
		return nil, err
	}
	var res ExecResult
	return &res, json.Unmarshal(raw, &res)
}
