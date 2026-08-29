# 寄せ書きアプリ

みんなで1枚の共有キャンバスに書き込んでいく、Web版の寄せ書きサービスです。
テーマ（例：「体育祭2年5組」など）を作成し、参加者がペンで描いたり文字を置いたりして書き足していきます。

## 主な機能

- テーマの作成・選択
- 1テーマ＝1枚の共有キャンバス（参加者が同じ絵の上に直接書き足す方式）
- ペンツール（マウス・指・タッチペン対応、iPadなどのタッチ操作にも対応）
- 文字を置くツール
- 書き加えて保存
- 保存前の未保存分だけを取り消す「やり直す」機能
- 広いキャンバス（スクロールして移動しながら書ける）
- 寄せ書きのダウンロード（書き込みがある範囲だけを自動でトリミング）
- 投稿数・画像サイズの上限（無料利用枠を超えないための制限）
- Basic認証によるアクセス制限

## 技術スタック

- 言語: [Rust](https://www.rust-lang.org/)
- フロントエンド: [Leptos](https://leptos.dev/)（WebAssemblyにビルド）
- 実行環境: [Cloudflare Workers](https://developers.cloudflare.com/workers/)
- データベース: [Cloudflare D1](https://developers.cloudflare.com/d1/)
- 画像ストレージ: [Cloudflare R2](https://developers.cloudflare.com/r2/)

## セットアップ

以下のツールが必要です。

```sh
rustup target add wasm32-unknown-unknown
cargo install --locked cargo-leptos
cargo install worker-build
```

Cloudflareアカウントへのログインも必要です。

```sh
npx wrangler login
```

### 環境変数（Basic認証）

ローカル開発時は、リポジトリ直下に `.dev.vars` ファイル（Git管理対象外）を作成し、以下を設定してください。

```
BASIC_AUTH_USER=（任意のユーザー名）
BASIC_AUTH_PASS=（任意のパスワード）
```

本番環境では、以下のコマンドでシークレットとして設定します（値はコマンド実行時に入力します）。

```sh
npx wrangler secret put BASIC_AUTH_USER
npx wrangler secret put BASIC_AUTH_PASS
```

### D1データベースのマイグレーション

```sh
# ローカル
npx wrangler d1 migrations apply yosegaki-db --local

# 本番
npx wrangler d1 migrations apply yosegaki-db --remote
```

## ローカルで起動

```sh
npx wrangler dev
```

## デプロイ

```sh
npx wrangler deploy
```
