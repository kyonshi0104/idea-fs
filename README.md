<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="https://github.com/user-attachments/assets/b9f8a1b5-ef56-471a-a221-4de7fca88ffb" />
    <source media="(prefers-color-scheme: light)" srcset="https://github.com/user-attachments/assets/3b5382b6-56e6-4143-9fb3-3df6cc903eb4" />
    <img src="https://github.com/user-attachments/assets/3b5382b6-56e6-4143-9fb3-3df6cc903eb4" alt="Idea FS" width="300" />
  </picture>
</p>

<p align="center">
  TauriによるデスクトップUIとRust製CLIを備えた、アイデア管理ツールです。<br />
  アイデアはYAMLフロントマター付きのMarkdownファイルとして保存され、データベースは使用しません。
</p>
<table>
  <tr>
    <td><img src="https://github.com/user-attachments/assets/74a2870f-316c-41f0-98a7-64e766bb0aa4" alt="Screenshot 1" width="100%" /></td>
    <td><img src="https://github.com/user-attachments/assets/0534c0ac-2cdb-4e80-a37b-9e03211d135c" alt="Screenshot 2" width="100%" /></td>
  </tr>
  <tr>
    <td><img src="https://github.com/user-attachments/assets/78097cd8-6aba-4022-b287-124118d0f770" alt="Screenshot 3" width="100%" /></td>
    <td><img src="https://github.com/user-attachments/assets/8ec13d6c-368d-4dd2-88a2-e3a3026ac98f" alt="Screenshot 4" width="100%" /></td>
  </tr>
</table>

## Features

- **GUI / CLI 構成**: デスクトップUI（`ideafs`）とCLI（`idfs`）の2バイナリ構成
- **Quick Add**: グローバルショートカット（初期値: `Ctrl+Alt+I` / `Cmd+Alt+I`）から記録
- **省メモリ常駐**: ウィンドウを閉じるとWebViewを破棄し、システムトレイで待機
- **自動アップデート**: GitHub Releases による更新に対応

## Installation

### Windows

1. [Releases](https://github.com/kyonshi0104/idea-fs/releases) から `IdeaFS_<version>_x64-setup.exe` をダウンロードして実行
2. インストール時にユーザー環境変数 `PATH` へ自動登録
3. 新しいターミナルを開き、`idfs --help` で動作確認

## CLI Usage (`idfs`)

### 追加 (`add`)

```bash
idfs add "新しいプロジェクト案"
idfs add "CLIツールの試作" -c Inbox -t rust,cli -u https://example.com
```

| オプション | 短縮 | 内容 |
| :--- | :--- | :--- |
| `--category <name>` | `-c` | 保存先カテゴリ（部分一致。省略時は先頭カテゴリ） |
| `--tags <tags>` | `-t` | カンマ区切りのタグ |
| `--resource <path>` | `-r` | 関連パス（`resource_path`） |
| `--url <url>` | `-u` | 関連URL（`url`） |

### 一覧 (`list`)

```bash
idfs list
idfs list -c Inbox
```

| オプション | 短縮 | 内容 |
| :--- | :--- | :--- |
| `--category <name>` | `-c` | 指定カテゴリのみ表示 |

### 削除 (`rm` / `remove`)

```bash
idfs rm "テストアイデア"
idfs rm "テストアイデア" -f
```

| オプション | 短縮 | 内容 |
| :--- | :--- | :--- |
| `--force` | `-f` | 確認プロンプトをスキップ |

### 開く (`open`)

```bash
# GUIを開く
idfs open

# 一致したアイデアの resource_path（なければ url）を開く
idfs open "CLIツールの試作"
```

## Data Format

保存先: `~/Documents/IdeaFS/`

```markdown
---
title: サンプルアイデア
tags:
  - rust
  - tauri
status: inbox
created_at: 2026-11-19T12:00:00+09:00
resource_path: C:\path\to\project
url: [https://example.com](https://example.com)
---

本文（Markdown）
```

## License

[MIT](./LICENSE)
