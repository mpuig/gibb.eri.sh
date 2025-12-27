---
name: summarize
version: 1.0.0
description: Summarize webpages, YouTube videos, podcasts, PDFs, or local files. Use when user wants a summary, TL;DR, or overview of any content.
author: gibb.eri.sh
modes: [Global]
read_only: true
network: true
timeout: 120
---

### summarize_url

Summarize a webpage, YouTube video, podcast, or any URL.

#### Parameters

| Name | Type | Required | Description |
|------|------|----------|-------------|
| url | string | yes | The URL to summarize (webpage, YouTube, podcast, PDF) |
| length | string | no | Output length: short, medium, long (default: medium) |

#### Command

```bash
npx -y @steipete/summarize {{url}} --length {{length:medium}}
```

---

### summarize_file

Summarize a local file (text, PDF, or document).

#### Parameters

| Name | Type | Required | Description |
|------|------|----------|-------------|
| path | string | yes | Path to the file to summarize |
| length | string | no | Output length: short, medium, long (default: medium) |

#### Command

```bash
npx -y @steipete/summarize {{path}} --length {{length:medium}}
```

---

### summarize_youtube

Summarize a YouTube video by its URL or ID.

#### Parameters

| Name | Type | Required | Description |
|------|------|----------|-------------|
| video | string | yes | YouTube URL or video ID |
| length | string | no | Output length: short, medium, long (default: medium) |

#### Command

```bash
npx -y @steipete/summarize {{video}} --youtube auto --length {{length:medium}}
```

---

### extract_content

Extract and display raw content from a URL without summarizing.

#### Parameters

| Name | Type | Required | Description |
|------|------|----------|-------------|
| url | string | yes | The URL to extract content from |

#### Command

```bash
npx -y @steipete/summarize {{url}} --extract
```
