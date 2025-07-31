#!/bin/bash

echo "=== 快速诊断和修复PR冲突 ==="

cd /workspace

echo "1. 当前分支状态："
git branch --show-current 2>/dev/null || echo "无法检测当前分支"

echo ""
echo "2. Git状态："
git status --porcelain 2>/dev/null || echo "Git状态检查失败"

echo ""
echo "3. 是否有正在进行的rebase/merge："
if [ -d ".git/rebase-merge" ] || [ -d ".git/rebase-apply" ]; then
    echo "❌ 有正在进行的rebase，需要处理"
    git rebase --abort 2>/dev/null || echo "无法中止rebase"
elif [ -f ".git/MERGE_HEAD" ]; then
    echo "❌ 有正在进行的merge，需要处理"  
    git merge --abort 2>/dev/null || echo "无法中止merge"
else
    echo "✅ 没有正在进行的rebase/merge"
fi

echo ""
echo "4. 尝试强制重置到干净状态："
git reset --hard HEAD 2>/dev/null || echo "重置失败"

echo ""
echo "5. 检查远程分支："
git fetch origin 2>/dev/null || echo "Fetch失败"

echo ""
echo "6. 列出所有分支："
git branch -a 2>/dev/null || echo "无法列出分支"

echo ""
echo "=== 诊断完成 ==="