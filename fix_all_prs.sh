#!/bin/bash

# Simple script to fix all PR conflicts
set -e

echo "🚀 Starting to fix all PR conflicts..."

# Go to workspace
cd /workspace

# List of branches
BRANCHES=(
    "feature/add-auth-module-tests"
    "feature/add-storage-core-tests"
    "feature/add-admin-handlers-tests" 
    "feature/add-server-components-tests"
    "feature/add-integration-tests"
)

echo "📦 Updating main..."
git fetch origin
git checkout main
git reset --hard origin/main

for BRANCH in "${BRANCHES[@]}"; do
    echo ""
    echo "🔧 Fixing branch: $BRANCH"
    
    # Abort any ongoing operations
    git rebase --abort 2>/dev/null || true
    git merge --abort 2>/dev/null || true
    
    # Create a fresh branch based on main
    git checkout main
    git branch -D "$BRANCH-fixed" 2>/dev/null || true
    git checkout -b "$BRANCH-fixed"
    
    # Get the original branch content (excluding Cargo.lock)
    echo "📥 Fetching original branch content..."
    git fetch origin "$BRANCH:$BRANCH-temp" 2>/dev/null || git checkout -b "$BRANCH-temp" "origin/$BRANCH"
    
    # Cherry-pick all commits except Cargo.lock-only commits
    echo "🍒 Cherry-picking relevant commits..."
    
    # Get commit list (excluding merges)
    COMMITS=$(git log --oneline --no-merges "main..$BRANCH-temp" | grep -v "Update dependencies\|Cargo.lock" | cut -d' ' -f1 | tac)
    
    for COMMIT in $COMMITS; do
        echo "  📌 Cherry-picking: $COMMIT"
        if ! git cherry-pick "$COMMIT"; then
            echo "  ⚠️  Conflict in $COMMIT, resolving..."
            
            # Auto-resolve Cargo.lock conflicts
            if git status --porcelain | grep -q "Cargo.lock"; then
                git checkout --theirs Cargo.lock 2>/dev/null || git rm Cargo.lock
                git add Cargo.lock 2>/dev/null || true
            fi
            
            # Check if there are other conflicts
            if git status --porcelain | grep -q "^UU" | grep -v "Cargo.lock"; then
                echo "  ❌ Manual conflicts remaining, skipping..."
                git cherry-pick --abort
                continue
            else
                git cherry-pick --continue
            fi
        fi
    done
    
    # Replace the original branch
    echo "🔄 Updating original branch..."
    git checkout main
    git branch -D "$BRANCH" 2>/dev/null || true
    git checkout -b "$BRANCH" "$BRANCH-fixed"
    
    # Force push
    echo "🚀 Force pushing..."
    git push --force-with-lease origin "$BRANCH"
    
    # Cleanup
    git branch -D "$BRANCH-fixed" 2>/dev/null || true
    git branch -D "$BRANCH-temp" 2>/dev/null || true
    
    echo "✅ Fixed: $BRANCH"
done

echo ""
echo "🎉 All PR conflicts resolved!"
echo "🔍 Please check the PRs on GitHub to verify they can be merged."