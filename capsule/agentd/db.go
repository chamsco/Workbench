package main

import (
	"database/sql"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"time"

	_ "modernc.org/sqlite"
)

type DB struct{ *sql.DB }

func openDB() (*DB, error) {
	if err := os.MkdirAll(dataDir(), 0o755); err != nil {
		return nil, err
	}
	db, err := sql.Open("sqlite", filepath.Join(dataDir(), "visor.db"))
	if err != nil {
		return nil, err
	}
	db.SetMaxOpenConns(1) // sqlite: serialize access, avoid SQLITE_BUSY
	if _, err := db.Exec(`PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;`); err != nil {
		return nil, err
	}
	_, err = db.Exec(`
CREATE TABLE IF NOT EXISTS sessions (
  id TEXT PRIMARY KEY,
  workspace TEXT NOT NULL,
  created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS messages (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  session_id TEXT NOT NULL,
  role TEXT NOT NULL,
  text TEXT,
  tool_calls TEXT,
  created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS tasks (
  id TEXT PRIMARY KEY,
  prompt TEXT NOT NULL,
  os TEXT,
  provider TEXT,
  policy TEXT NOT NULL DEFAULT 'destroy',
  status TEXT NOT NULL DEFAULT 'queued',
  workspace TEXT,
  result TEXT,
  error TEXT,
  created_at TEXT NOT NULL,
  finished_at TEXT
);
CREATE TABLE IF NOT EXISTS events (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  task_id TEXT NOT NULL,
  kind TEXT NOT NULL,
  data TEXT,
  created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS remotes (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL UNIQUE,
  kind TEXT NOT NULL,
  host TEXT NOT NULL,
  port INTEGER NOT NULL,
  username TEXT,
  key_path TEXT,
  os TEXT NOT NULL DEFAULT 'linux',
  created_at TEXT NOT NULL
);`)
	if err != nil {
		return nil, err
	}
	return &DB{db}, nil
}

func now() string { return time.Now().UTC().Format(time.RFC3339) }

type Remote struct {
	ID        string `json:"id"`
	Name      string `json:"name"`
	Kind      string `json:"kind"` // ssh | rdp
	Host      string `json:"host"`
	Port      uint16 `json:"port"`
	Username  string `json:"username,omitempty"`
	KeyPath   string `json:"key_path,omitempty"`
	OS        string `json:"os"`
	CreatedAt string `json:"created_at"`
}

func (d *DB) createRemote(remote *Remote) error {
	remote.CreatedAt = now()
	_, err := d.Exec(`INSERT INTO remotes (id, name, kind, host, port, username, key_path, os, created_at) VALUES (?,?,?,?,?,?,?,?,?)`,
		remote.ID, remote.Name, remote.Kind, remote.Host, remote.Port, remote.Username, remote.KeyPath, remote.OS, remote.CreatedAt)
	return err
}

func remoteWorkspace(remote *Remote) Workspace {
	return Workspace{
		Name: remote.Name, Provider: "remote-" + remote.Kind, Kind: "remote", OS: remote.OS,
		State: "remote", Host: remote.Host, Port: remote.Port, Username: remote.Username,
		KeyPath: remote.KeyPath, RemoteID: remote.ID,
	}
}

func (d *DB) listRemotes() ([]Remote, error) {
	rows, err := d.Query(`SELECT id, name, kind, host, port, COALESCE(username,''), COALESCE(key_path,''), os, created_at FROM remotes ORDER BY name`)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var out []Remote
	for rows.Next() {
		var remote Remote
		if err := rows.Scan(&remote.ID, &remote.Name, &remote.Kind, &remote.Host, &remote.Port, &remote.Username, &remote.KeyPath, &remote.OS, &remote.CreatedAt); err != nil {
			return nil, err
		}
		out = append(out, remote)
	}
	return out, rows.Err()
}

func (d *DB) getRemote(idOrName string) (*Remote, error) {
	var remote Remote
	err := d.QueryRow(`SELECT id, name, kind, host, port, COALESCE(username,''), COALESCE(key_path,''), os, created_at FROM remotes WHERE id=? OR name=?`, idOrName, idOrName).
		Scan(&remote.ID, &remote.Name, &remote.Kind, &remote.Host, &remote.Port, &remote.Username, &remote.KeyPath, &remote.OS, &remote.CreatedAt)
	if err != nil {
		return nil, err
	}
	return &remote, nil
}

func (d *DB) deleteRemote(idOrName string) error {
	_, err := d.Exec(`DELETE FROM remotes WHERE id=? OR name=?`, idOrName, idOrName)
	return err
}

func (d *DB) createSession(id, workspace string) error {
	_, err := d.Exec(`INSERT INTO sessions (id, workspace, created_at) VALUES (?,?,?)`, id, workspace, now())
	return err
}

func (d *DB) saveMessage(sessionID string, m ChatMsg) error {
	tc, _ := json.Marshal(m.ToolCalls)
	_, err := d.Exec(`INSERT INTO messages (session_id, role, text, tool_calls, created_at) VALUES (?,?,?,?,?)`,
		sessionID, m.Role, m.Text, string(tc), now())
	return err
}

func (d *DB) loadMessages(sessionID string) ([]ChatMsg, error) {
	rows, err := d.Query(`SELECT role, text, tool_calls FROM messages WHERE session_id=? ORDER BY id`, sessionID)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var out []ChatMsg
	for rows.Next() {
		var m ChatMsg
		var tc string
		if err := rows.Scan(&m.Role, &m.Text, &tc); err != nil {
			return nil, err
		}
		json.Unmarshal([]byte(tc), &m.ToolCalls)
		out = append(out, m)
	}
	return out, rows.Err()
}

func (d *DB) sessionWorkspace(sessionID string) (string, error) {
	var ws string
	err := d.QueryRow(`SELECT workspace FROM sessions WHERE id=?`, sessionID).Scan(&ws)
	return ws, err
}

// ---- tasks ----

type Task struct {
	ID        string `json:"id"`
	Prompt    string `json:"prompt"`
	OS        string `json:"os"`
	Provider  string `json:"provider"`
	Policy    string `json:"policy"` // destroy | keep
	Status    string `json:"status"` // queued|provisioning|running|collecting|done|error
	Workspace string `json:"workspace"`
	Result    string `json:"result"`
	Error     string `json:"error"`
	CreatedAt string `json:"created_at"`
}

func (d *DB) createTask(t *Task) error {
	t.CreatedAt = now()
	_, err := d.Exec(`INSERT INTO tasks (id, prompt, os, provider, policy, status, created_at) VALUES (?,?,?,?,?,?,?)`,
		t.ID, t.Prompt, t.OS, t.Provider, t.Policy, t.Status, t.CreatedAt)
	return err
}

func (d *DB) updateTask(id, status, workspace, result, errStr string, finished bool) error {
	if finished {
		_, err := d.Exec(`UPDATE tasks SET status=?, workspace=?, result=?, error=?, finished_at=? WHERE id=?`,
			status, workspace, result, errStr, now(), id)
		return err
	}
	_, err := d.Exec(`UPDATE tasks SET status=?, workspace=?, result=?, error=? WHERE id=?`,
		status, workspace, result, errStr, id)
	return err
}

func (d *DB) listTasks() ([]Task, error) {
	rows, err := d.Query(`SELECT id, prompt, COALESCE(os,''), COALESCE(provider,''), policy, status, COALESCE(workspace,''), COALESCE(result,''), COALESCE(error,''), created_at FROM tasks ORDER BY created_at DESC LIMIT 100`)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var out []Task
	for rows.Next() {
		var t Task
		if err := rows.Scan(&t.ID, &t.Prompt, &t.OS, &t.Provider, &t.Policy, &t.Status, &t.Workspace, &t.Result, &t.Error, &t.CreatedAt); err != nil {
			return nil, err
		}
		out = append(out, t)
	}
	return out, rows.Err()
}

func (d *DB) getTask(id string) (*Task, error) {
	var t Task
	err := d.QueryRow(`SELECT id, prompt, COALESCE(os,''), COALESCE(provider,''), policy, status, COALESCE(workspace,''), COALESCE(result,''), COALESCE(error,''), created_at FROM tasks WHERE id=?`, id).
		Scan(&t.ID, &t.Prompt, &t.OS, &t.Provider, &t.Policy, &t.Status, &t.Workspace, &t.Result, &t.Error, &t.CreatedAt)
	if err != nil {
		return nil, err
	}
	return &t, nil
}

type Event struct {
	ID        int64  `json:"id"`
	Kind      string `json:"kind"` // status|text|tool_call|tool_result|error|done
	Data      string `json:"data"`
	CreatedAt string `json:"created_at"`
}

func (d *DB) addEvent(taskID, kind string, data any) error {
	var s string
	switch v := data.(type) {
	case string:
		s = v
	default:
		b, _ := json.Marshal(v)
		s = string(b)
	}
	_, err := d.Exec(`INSERT INTO events (task_id, kind, data, created_at) VALUES (?,?,?,?)`, taskID, kind, s, now())
	return err
}

func (d *DB) eventsSince(taskID string, afterID int64) ([]Event, error) {
	rows, err := d.Query(`SELECT id, kind, data, created_at FROM events WHERE task_id=? AND id>? ORDER BY id`, taskID, afterID)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var out []Event
	for rows.Next() {
		var e Event
		if err := rows.Scan(&e.ID, &e.Kind, &e.Data, &e.CreatedAt); err != nil {
			return nil, fmt.Errorf("scan event: %w", err)
		}
		out = append(out, e)
	}
	return out, rows.Err()
}
