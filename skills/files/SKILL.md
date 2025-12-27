---
name: files
version: 1.0.0
description: File operations for searching, listing, and reading files. Use when user asks to find files, list directory contents, or read file contents.
author: gibb.eri.sh
modes: [Dev, Global]
read_only: true
timeout: 30
---

### find_files

Search for files by name using macOS Spotlight index.

#### Parameters

| Name | Type | Required | Description |
|------|------|----------|-------------|
| query | string | yes | Search query (filename or content) |
| folder | string | no | Limit search to specific folder |

#### Command

```bash
mdfind -name {{query}} {{folder:-onlyin}}
```

---

### list_directory

List contents of a directory with details.

#### Parameters

| Name | Type | Required | Description |
|------|------|----------|-------------|
| path | string | no | Directory path (default: current) |
| all | boolean | no | Include hidden files |

#### Command

```bash
ls -la {{all:-a}} {{path:.}}
```

---

### read_file

Read the contents of a text file.

#### Parameters

| Name | Type | Required | Description |
|------|------|----------|-------------|
| path | string | yes | Path to the file |
| lines | integer | no | Number of lines to read (default: all) |

#### Command

```bash
head -n {{lines:9999}} {{path}}
```

---

### file_info

Show detailed information about a file.

#### Parameters

| Name | Type | Required | Description |
|------|------|----------|-------------|
| path | string | yes | Path to the file |

#### Command

```bash
file {{path}}
```

---

### word_count

Count lines, words, and characters in a file.

#### Parameters

| Name | Type | Required | Description |
|------|------|----------|-------------|
| path | string | yes | Path to the file |

#### Command

```bash
wc {{path}}
```
