#!/bin/bash
set -e

cd /workspace

# 清理状态
git rebase --abort 2>/dev/null || true
git checkout main
git fetch origin
git reset --hard origin/main

# 修复第一个分支
git checkout feature/add-auth-module-tests
git reset --hard origin/feature/add-auth-module-tests
git rebase main || {
    git checkout --theirs Cargo.lock
    git add Cargo.lock
    git rebase --continue
}
git push --force-with-lease origin feature/add-auth-module-tests

# 修复其他分支
for branch in feature/add-storage-core-tests feature/add-admin-handlers-tests feature/add-server-components-tests feature/add-integration-tests; do
    git checkout main
    git checkout $branch
    git reset --hard origin/$branch
    git rebase main || {
        git checkout --theirs Cargo.lock
        git add Cargo.lock
        git rebase --continue || git rebase --skip
    }
    git push --force-with-lease origin $branch
done

echo "所有分支已修复!"