package main

import (
	"flag"
	"fmt"
	"log"
	"net/http"
)

func main() {
	addr := flag.String("addr", "127.0.0.1:7700", "listen address")
	flag.Parse()

	core := NewCore()
	privKey, _, err := ensureKeypair()
	if err != nil {
		log.Fatalf("ssh keypair: %v", err)
	}
	db, err := openDB()
	if err != nil {
		log.Fatalf("db: %v", err)
	}
	settings, err := loadSettings()
	if err != nil {
		log.Fatalf("settings: %v", err)
	}

	srv := &Server{core: core, db: db, privKey: privKey, settings: settings}
	srv.engine = &Engine{core: core, db: db, privKey: privKey, settings: func() *Settings { return srv.settings }}

	if caps, err := core.Capabilities(); err == nil {
		fmt.Printf("capabilities: hyperv=%v wsl=%v elevated=%v\n", caps.HyperV, caps.WSL, caps.Elevated)
		if caps.HyperV && !caps.Elevated {
			fmt.Println("warning: Hyper-V present but agentd is not elevated — hyperv workspaces will fail")
		}
	}
	fmt.Printf("colony agentd listening on http://%s\n", *addr)
	log.Fatal(http.ListenAndServe(*addr, srv.routes()))
}
