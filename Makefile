.PHONY: build release test clippy fmt check clean docker run help

BINARY   := vingress
IMAGE    := mariusm/vingress
VERSION  := $(shell grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/')

help: ## Show this help
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | \
		awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-15s\033[0m %s\n", $$1, $$2}'

build: ## Build in debug mode
	cargo build

release: ## Build in release mode (optimized, stripped, LTO)
	cargo build --release

test: ## Run all tests
	cargo test

clippy: ## Run clippy lints
	cargo clippy -- -D warnings

fmt: ## Format code
	cargo fmt

check: fmt clippy test ## Format, lint, and test (CI-ready)

clean: ## Remove build artifacts
	cargo clean

docker: ## Build the Docker image
	docker build -t $(IMAGE):$(VERSION) -t $(IMAGE):latest .

docker-push: ## Push Docker image to registry
	docker push $(IMAGE):$(VERSION)
	docker push $(IMAGE):latest

run: build ## Run locally (requires varnishd in PATH)
	cargo run

audit: ## Run cargo-audit for known vulnerabilities
	cargo audit
