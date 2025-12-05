#!/usr/bin/env bash
# release.sh - Prepare and create a release
# Usage: ./scripts/release.sh [major|minor|patch|X.Y.Z]

set -euo pipefail

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; exit 1; }

# Check we're on main branch
BRANCH=$(git branch --show-current)
if [[ "$BRANCH" != "main" && "$BRANCH" != "master" ]]; then
    log_error "Must be on main/master branch (currently on: $BRANCH)"
fi

# Check for uncommitted changes
if ! git diff-index --quiet HEAD --; then
    log_error "Uncommitted changes detected. Commit or stash first."
fi

# Get current version from Cargo.toml
CURRENT_VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/')
log_info "Current version: $CURRENT_VERSION"

# Parse current version
IFS='.' read -r MAJOR MINOR PATCH <<< "${CURRENT_VERSION%-*}"
PATCH="${PATCH%-*}"  # Remove any prerelease suffix

# Determine new version
BUMP_TYPE="${1:-patch}"
case "$BUMP_TYPE" in
    major)
        NEW_VERSION="$((MAJOR + 1)).0.0"
        ;;
    minor)
        NEW_VERSION="$MAJOR.$((MINOR + 1)).0"
        ;;
    patch)
        NEW_VERSION="$MAJOR.$MINOR.$((PATCH + 1))"
        ;;
    *.*.*) 
        NEW_VERSION="$BUMP_TYPE"
        ;;
    *)
        log_error "Usage: $0 [major|minor|patch|X.Y.Z]"
        ;;
esac

log_info "New version: $NEW_VERSION"
echo ""

# Confirm
read -p "Create release v$NEW_VERSION? [y/N] " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    log_warn "Aborted"
    exit 0
fi

# Update Cargo.toml
log_info "Updating Cargo.toml..."
sed -i.bak "s/^version = \"$CURRENT_VERSION\"/version = \"$NEW_VERSION\"/" Cargo.toml
rm -f Cargo.toml.bak

# Update Cargo.lock
log_info "Updating Cargo.lock..."
cargo update --workspace

# Run tests
log_info "Running tests..."
cargo test --locked

# Commit version bump
log_info "Committing version bump..."
git add Cargo.toml Cargo.lock
git commit -m "chore(release): bump version to $NEW_VERSION"

# Create annotated tag
log_info "Creating tag v$NEW_VERSION..."
git tag -a "v$NEW_VERSION" -m "Release $NEW_VERSION"

# Push
log_info "Pushing to origin..."
git push origin main
git push origin "v$NEW_VERSION"

echo ""
log_info "✅ Release v$NEW_VERSION created and pushed!"
log_info "   GitHub Actions will now build and publish the release."
log_info "   Watch progress: https://github.com/$(git remote get-url origin | sed 's/.*github.com[:/]\(.*\)\.git/\1/')/actions"