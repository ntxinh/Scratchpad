# Thin aliases over mise tasks — single source of truth is mise.toml.
MISE := mise run

.PHONY: build run test lint install
build:   ; $(MISE) build
run:     ; $(MISE) run
test:    ; $(MISE) test
lint:    ; $(MISE) lint
install: ; $(MISE) install
