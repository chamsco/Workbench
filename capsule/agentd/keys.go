package main

import (
	"crypto/ed25519"
	"crypto/rand"
	"crypto/x509"
	"encoding/pem"
	"fmt"
	"os"
	"path/filepath"

	"golang.org/x/crypto/ssh"
)

// ensureKeypair returns (privateKeyPath, publicKeyLine), generating an
// ed25519 pair in the data dir on first run.
func ensureKeypair() (string, string, error) {
	dir := filepath.Join(dataDir(), "keys")
	priv := filepath.Join(dir, "id_ed25519")
	pub := priv + ".pub"
	if _, err := os.Stat(priv); err == nil {
		b, err := os.ReadFile(pub)
		return priv, string(b), err
	}
	if err := os.MkdirAll(dir, 0o700); err != nil {
		return "", "", err
	}
	pubKey, privKey, err := ed25519.GenerateKey(rand.Reader)
	if err != nil {
		return "", "", err
	}
	sshPub, err := ssh.NewPublicKey(pubKey)
	if err != nil {
		return "", "", err
	}
	if err := os.WriteFile(pub, ssh.MarshalAuthorizedKey(sshPub), 0o644); err != nil {
		return "", "", err
	}
	der, err := x509.MarshalPKCS8PrivateKey(privKey)
	if err != nil {
		return "", "", err
	}
	pemBytes := pem.EncodeToMemory(&pem.Block{Type: "PRIVATE KEY", Bytes: der})
	if err := os.WriteFile(priv, pemBytes, 0o600); err != nil {
		return "", "", err
	}
	return priv, string(ssh.MarshalAuthorizedKey(sshPub)), nil
}

func loadSigner(path string) (ssh.Signer, error) {
	b, err := os.ReadFile(path)
	if err != nil {
		return nil, fmt.Errorf("read ssh key: %w", err)
	}
	return ssh.ParsePrivateKey(b)
}
