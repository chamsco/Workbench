package main

import (
	"encoding/base64"
	"fmt"
	"time"

	"golang.org/x/crypto/ssh"
)

// ExecBackend runs commands inside a workspace.
type ExecBackend interface {
	// Exec runs a shell command (sh -c on linux, cmd /c on windows guests).
	Exec(command string) (*ExecResult, error)
	ReadFile(path string) (string, error)
	WriteFile(path, content string) error
	Close() error
}

// ---- WSL backend: shell out to visor-core exec ----

type WSLBackend struct {
	core *Core
	name string
}

func (b *WSLBackend) Exec(command string) (*ExecResult, error) {
	return b.core.ExecWSL(b.name, "sh", "-c", command)
}

func (b *WSLBackend) ReadFile(path string) (string, error) {
	res, err := b.Exec("cat " + shellQuote(path))
	if err != nil {
		return "", err
	}
	if res.ExitCode != 0 {
		return "", fmt.Errorf("read %s: %s", path, res.Stderr)
	}
	return res.Stdout, nil
}

func (b *WSLBackend) WriteFile(path, content string) error {
	enc := base64.StdEncoding.EncodeToString([]byte(content))
	res, err := b.Exec(fmt.Sprintf("echo %s | base64 -d > %s", shellQuote(enc), shellQuote(path)))
	if err != nil {
		return err
	}
	if res.ExitCode != 0 {
		return fmt.Errorf("write %s: %s", path, res.Stderr)
	}
	return nil
}

func (b *WSLBackend) Close() error { return nil }

// ---- SSH backend: for Hyper-V guests ----

type SSHBackend struct {
	client *ssh.Client
	osKind string // "linux" | "windows"
}

func DialSSH(ws *Workspace, keyPath string, osKind string, timeout time.Duration) (*SSHBackend, error) {
	if ws.KeyPath != "" {
		keyPath = ws.KeyPath
	}
	key, err := loadSigner(keyPath)
	if err != nil {
		return nil, err
	}
	user := ws.Username
	if user == "" {
		user = ws.SSHUser
	}
	cfg := &ssh.ClientConfig{
		User:            user,
		Auth:            []ssh.AuthMethod{ssh.PublicKeys(key)},
		HostKeyCallback: ssh.InsecureIgnoreHostKey(), // ephemeral local VMs
		Timeout:         timeout,
	}
	port := ws.Port
	if port == 0 {
		port = ws.SSHPort
	}
	if port == 0 {
		port = 22
	}
	client, err := ssh.Dial("tcp", fmt.Sprintf("%s:%d", ws.IP, port), cfg)
	if err != nil {
		return nil, err
	}
	return &SSHBackend{client: client, osKind: osKind}, nil
}

func (b *SSHBackend) Exec(command string) (*ExecResult, error) {
	sess, err := b.client.NewSession()
	if err != nil {
		return nil, err
	}
	defer sess.Close()
	var so, se stringBuf
	sess.Stdout, sess.Stderr = &so, &se
	shell := "sh -c " + shellQuote(command)
	if b.osKind == "windows" {
		shell = "cmd /c " + command
	}
	runErr := sess.Run(shell)
	code := 0
	if exitErr, ok := runErr.(*ssh.ExitError); ok {
		code = exitErr.ExitStatus()
	} else if runErr != nil {
		return nil, runErr
	}
	return &ExecResult{ExitCode: code, Stdout: so.String(), Stderr: se.String()}, nil
}

func (b *SSHBackend) ReadFile(path string) (string, error) {
	cmd := "cat " + shellQuote(path)
	if b.osKind == "windows" {
		cmd = "type " + path
	}
	res, err := b.Exec(cmd)
	if err != nil {
		return "", err
	}
	if res.ExitCode != 0 {
		return "", fmt.Errorf("read %s: %s", path, res.Stderr)
	}
	return res.Stdout, nil
}

func (b *SSHBackend) WriteFile(path, content string) error {
	enc := base64.StdEncoding.EncodeToString([]byte(content))
	cmd := fmt.Sprintf("echo %s | base64 -d > %s", shellQuote(enc), shellQuote(path))
	if b.osKind == "windows" {
		// PowerShell is available on any modern Windows guest
		cmd = fmt.Sprintf(`powershell -NoProfile -Command "[IO.File]::WriteAllBytes('%s', [Convert]::FromBase64String('%s'))"`, path, enc)
	}
	res, err := b.Exec(cmd)
	if err != nil {
		return err
	}
	if res.ExitCode != 0 {
		return fmt.Errorf("write %s: %s", path, res.Stderr)
	}
	return nil
}

func (b *SSHBackend) Close() error { return b.client.Close() }

// ---- helpers ----

type stringBuf struct{ b []byte }

func (s *stringBuf) Write(p []byte) (int, error) { s.b = append(s.b, p...); return len(p), nil }
func (s *stringBuf) String() string              { return string(s.b) }

func shellQuote(s string) string {
	out := "'"
	for _, c := range s {
		if c == '\'' {
			out += `'\''`
		} else {
			out += string(c)
		}
	}
	return out + "'"
}

// backendFor returns an exec backend for a workspace.
func backendFor(core *Core, ws *Workspace, keyPath string) (ExecBackend, error) {
	switch ws.Provider {
	case "wsl":
		return &WSLBackend{core: core, name: ws.Name}, nil
	case "hyperv", "remote-ssh":
		if ws.IP == "" {
			if ws.Host == "" {
				return nil, fmt.Errorf("workspace %s has no host yet", ws.Name)
			}
			ws.IP = ws.Host
		}
		osKind := "linux"
		if len(ws.OS) >= 7 && ws.OS[:7] == "windows" {
			osKind = "windows"
		}
		return DialSSH(ws, keyPath, osKind, 10*time.Second)
	default:
		return nil, fmt.Errorf("unknown provider %q", ws.Provider)
	}
}
