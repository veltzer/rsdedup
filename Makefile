.PHONY: all
all:
	@cargo build
	@cargo build --release
	cargo nextest run
	cargo nextest run --release

.PHONY: test
test:
	cargo nextest run
	cargo nextest run --release

.PHONY: clean
clean:
	@rm -rf release

.PHONY: artifacts
artifacts:
	@gh release view --json assets --jq '.assets[] | "\(.name)\t\(.size)\t\(.downloadCount)"' | column -t -N NAME,SIZE,DOWNLOADS

# Release: bump patch version, commit, tag, and push to trigger CI release
# Usage:
#   make release           - bump patch (0.1.0 -> 0.1.1)
#   make release PART=minor - bump minor (0.1.0 -> 0.2.0)
#   make release PART=major - bump major (0.1.0 -> 1.0.0)
PART ?= patch

.PHONY: release
release:
	@if [ -n "$$(git status --porcelain)" ]; then echo "Error: working tree is dirty, commit or stash changes first"; exit 1; fi
	@CURRENT=$$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/'); \
	MAJOR=$$(echo "$$CURRENT" | cut -d. -f1); \
	MINOR=$$(echo "$$CURRENT" | cut -d. -f2); \
	PATCH=$$(echo "$$CURRENT" | cut -d. -f3); \
	case "$(PART)" in \
		patch) PATCH=$$((PATCH + 1));; \
		minor) MINOR=$$((MINOR + 1)); PATCH=0;; \
		major) MAJOR=$$((MAJOR + 1)); MINOR=0; PATCH=0;; \
		*) echo "Error: PART must be patch, minor, or major"; exit 1;; \
	esac; \
	NEW="$$MAJOR.$$MINOR.$$PATCH"; \
	echo "Bumping version: $$CURRENT -> $$NEW"; \
	sed -i "s/^version = \"$$CURRENT\"/version = \"$$NEW\"/" Cargo.toml; \
	cargo check --quiet; \
	git add Cargo.toml Cargo.lock; \
	git commit -m "release v$$NEW"; \
	git tag -a "v$$NEW" -m "release v$$NEW"; \
	git push; \
	git push origin "v$$NEW"; \
	echo "Released v$$NEW"
