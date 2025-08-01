#!/usr/bin/env python3

import subprocess
import sys
import os

def run_cmd(cmd):
    """运行命令并返回结果"""
    try:
        result = subprocess.run(cmd, shell=True, capture_output=True, text=True, timeout=30)
        print(f"执行: {cmd}")
        if result.stdout:
            print(f"输出: {result.stdout.strip()}")
        if result.stderr:
            print(f"错误: {result.stderr.strip()}")
        return result.returncode == 0
    except subprocess.TimeoutExpired:
        print(f"命令超时: {cmd}")
        return False

def main():
    """修复剩下的PR分支"""
    print("🚀 修复剩下的PR分支...")
    
    # 剩下需要修复的分支
    remaining_branches = [
        ("feature/add-server-components-tests", "7c50378"),  # 第二个commit，避免Cargo.lock
        ("feature/add-integration-tests", "69f2d0a")  # 假设的主要commit
    ]
    
    os.chdir("/workspace")
    
    # 确保在main分支
    run_cmd("git checkout main")
    
    for branch_name, commit_hash in remaining_branches:
        print(f"\n🔧 修复分支: {branch_name}")
        
        # 创建干净分支
        clean_branch = f"{branch_name}-clean"
        run_cmd(f"git branch -D {clean_branch} 2>/dev/null || true")
        run_cmd(f"git checkout -b {clean_branch}")
        
        # 查找主要的测试提交
        print("📥 查找主要提交...")
        result = subprocess.run(f"git log --oneline origin/{branch_name} | head -10", 
                              shell=True, capture_output=True, text=True)
        
        if result.returncode == 0:
            commits = result.stdout.strip().split('\n')
            print("可用的提交:")
            for commit in commits:
                print(f"  {commit}")
            
            # 寻找包含测试的主要提交（不是Cargo.lock相关的）
            test_commit = None
            for commit in commits:
                if any(keyword in commit.lower() for keyword in ['test', 'feat:', 'add']):
                    if 'cargo.lock' not in commit.lower() and 'format' not in commit.lower():
                        test_commit = commit.split()[0]
                        break
            
            if test_commit:
                print(f"📌 选择提交: {test_commit}")
                if run_cmd(f"git cherry-pick {test_commit}"):
                    print("✅ Cherry-pick成功")
                else:
                    print("⚠️ Cherry-pick有冲突，尝试解决...")
                    # 解决Cargo.lock冲突
                    run_cmd("git checkout --theirs Cargo.lock || true")
                    run_cmd("git add Cargo.lock || true")
                    if run_cmd("git cherry-pick --continue"):
                        print("✅ 冲突解决成功")
                    else:
                        print("❌ 无法解决冲突，跳过")
                        run_cmd("git cherry-pick --abort")
                        continue
            else:
                print("❌ 未找到合适的测试提交")
                continue
        
        # 替换原分支
        print("🔄 替换原分支...")
        run_cmd(f"git checkout {branch_name}")
        run_cmd(f"git reset --hard {clean_branch}")
        
        # 推送
        print("🚀 推送分支...")
        if run_cmd(f"git push --force origin {branch_name}"):
            print(f"✅ 成功修复 {branch_name}")
        else:
            print(f"❌ 推送失败 {branch_name}")
        
        # 清理
        run_cmd(f"git branch -D {clean_branch}")
        run_cmd("git checkout main")
    
    print("\n🎉 完成剩余PR修复！")

if __name__ == "__main__":
    main()