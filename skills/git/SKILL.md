---
name: git
version: 1.0.0
description: Git operations for checking repository status, viewing commit history, and seeing changes. Use when user asks about git state, commits, branches, or diffs.
author: gibb.eri.sh
modes: [Dev, Global]
read_only: true
timeout: 15
---

### git_status

Show the current git status with branch info and short format.

#### Parameters

None.

#### Command

```bash
git status --short --branch
```

---

### git_log

Show recent commit history in a compact format.

#### Parameters

| Name | Type | Required | Description |
|------|------|----------|-------------|
| count | integer | no | Number of commits to show (default: 10) |

#### Command

```bash
git log --oneline --graph -n {{count:10}}
```

---

### git_diff

Show uncommitted changes in the working directory.

#### Parameters

| Name | Type | Required | Description |
|------|------|----------|-------------|
| staged | boolean | no | Show only staged changes |
| file | string | no | Specific file to diff |

#### Command

```bash
git diff {{staged:--staged}} {{file}}
```

---

### git_branch

List all branches, highlighting the current one.

#### Parameters

| Name | Type | Required | Description |
|------|------|----------|-------------|
| all | boolean | no | Include remote branches |

#### Command

```bash
git branch {{all:--all}} -v
```

---

### git_stash_list

Show all stashed changes.

#### Parameters

None.

#### Command

```bash
git stash list
```
