.PHONY: release-macos test-release sync-release

release-macos:
	./scripts/build-release-macos.sh

test-release:
	cargo test --release -p agent-brain

sync-release:
	./scripts/sync-local-release.sh
