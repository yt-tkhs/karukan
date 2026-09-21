# karukan-cli

karukan-engineを利用したCLIツール群。かな漢字変換サーバー、辞書ビルド、辞書ビューア、ベンチマーク評価ツールを提供します。

## Binaries

| バイナリ | 概要 |
|---------|------|
| `karukan-dict` | 辞書のビルド（JSON/Mozc TSV → バイナリ）とビューア（Web UI + CLI検索） |
| `sudachi-dict` | Sudachi CSVからJSON辞書を生成 |
| `karukan-server` | かな漢字変換HTTPサーバー（Web UI付き） |
| `ajimee-bench` | AJIMEE-Bench評価ツール |

## Build

```bash
# リポジトリルートから実行
cargo build -p karukan-cli --release
```

## karukan-dict

辞書のビルドと検索を行うツール。`build` と `view` の2つのサブコマンドがあります。

### build — 辞書ビルド

1 つ以上の入力ファイルからバイナリ辞書を生成します。入力は指定した順にレイヤーとして重なります。

```bash
# 1 ファイル（形式は自動判定）
cargo run --release --bin karukan-dict -- build input.json -o dict.bin

# 複数ファイルを重ねる: Mozc 辞書 → SudachiDict → Google IME 形式の辞書
cargo run --release --bin karukan-dict -- build \
  dictionary0?.txt small_lex.csv core_lex.csv notcore_lex.csv extra.txt -o dict.bin

# フォーマットを明示指定（全入力に適用）
cargo run --release --bin karukan-dict -- build input.txt --format mozc -o dict.bin
```

| オプション | デフォルト | 説明 |
|-----------|----------|------|
| `inputs` (必須) | — | 入力ファイル（1 つ以上、優先度の高い順） |
| `-o, --output` | `dict.bin` | 出力バイナリ辞書ファイル |
| `-f, --format` | 自動判定 | 全入力の形式: `json` / `sudachi` / `mozc-system` / `mozc` |

**入力形式（ファイルごとに自動判定）:**

| 形式 | 判定 | 内容 |
|------|------|------|
| `json` | 拡張子 `.json` | `[{reading, candidates: [{surface, score}]}]` の配列 |
| `sudachi` | 拡張子 `.csv` | SudachiDict の CSV（読みはカタカナ。ひらがなに正規化する） |
| `mozc-system` | タブ区切り 5 列で 2〜4 列目が整数 | Mozc のシステム辞書（`読み\t左ID\t右ID\tコスト\t表記`） |
| `mozc` | それ以外のタブ区切り | Mozc/Google IME のユーザー辞書（`読み\t表記\t品詞\tコメント`）。コストが無いのでファイル内の順で採点する |

**レイヤーの規則:** 隣り合う同じ形式のファイルは 1 つのレイヤー（1 つの辞書が複数ファイルに分かれたもの。Mozc の `dictionary00〜09.txt` や SudachiDict の small / core / notcore）として扱い、両方にある（読み, 表記）は低い方のコストを取ります。ある（読み, 表記）のスコアは、それを最初に持っていたレイヤーのものになります。i 番目（0 始まり）のレイヤーで初めて現れた語は `スコア + 100000 × i` になるので、同じ読みの中では前のレイヤーの語が後のレイヤーの語より必ず先に並び、各レイヤーの中では元のコスト順が保たれます。前方一致（予測）の並びでも同じ差が効きます。

配布している `dict.bin` はこの規則で Mozc 辞書・SudachiDict・dic-nico-intersection-pixiv を重ねたものです。`scripts/build-dict.sh` がデータの取得からビルドまでを行います（[docs/README.md](docs/README.md) にデータ源とライセンス）。

```bash
# リポジトリのルートで。build/dict/ にダウンロードして dict.tgz を作る
scripts/build-dict.sh
# ニコニコ大百科・ピクシブ百科事典由来の辞書を外す
scripts/build-dict.sh --without-nico
# Wikipedia の見出し語（mozcdic-ut-jawiki、CC BY-SA）を足す。手元用向け
scripts/build-dict.sh --with-jawiki
```

Mozc の顔文字データ（emoticon.tsv）は常に最後の層に入ります（「にこ」→ (^^) など）。

### view — 辞書ビューア

辞書の内容を検索・閲覧します。CLIモードとWebモードの2つの動作モードがあります。

```bash
# Webモード（ブラウザで辞書を検索）
cargo run --release --bin karukan-dict -- view dict.bin
# → http://127.0.0.1:8080

# CLI検索（完全一致）
cargo run --release --bin karukan-dict -- view dict.bin --query きょう

# CLI検索（前方一致）
cargo run --release --bin karukan-dict -- view dict.bin --query きょう --prefix

# CLI検索（表層形で検索）
cargo run --release --bin karukan-dict -- view dict.bin --query 今日 --surface

# 全エントリのダンプ
cargo run --release --bin karukan-dict -- view dict.bin --all
```

| オプション | デフォルト | 説明 |
|-----------|----------|------|
| `dicts` (必須) | — | 辞書ファイル（複数指定可、KRKN or Mozc TSV） |
| `--port` | `8080` | Webモードのポート |
| `--host` | `127.0.0.1` | Webモードのバインドアドレス |
| `-q, --query` | — | CLI検索クエリ |
| `-s, --surface` | off | 表層形で検索 |
| `-p, --prefix` | off | 前方一致検索 |
| `-a, --all` | off | 全エントリをダンプ |

## sudachi-dict

Sudachi辞書CSVファイルからJSON辞書を生成します。デフォルトではSudachiの正規コストをそのまま使用し、`--model-scores` を指定するとjinenモデルのNLLスコアリングで候補を順序付けします。

入力となるSudachi辞書CSVは[SudachiDict](http://sudachi.s3-website-ap-northeast-1.amazonaws.com/sudachidict-raw/)からダウンロードできます。

```bash
# 基本的な使い方（Sudachiコストを使用）
cargo run --release --bin sudachi-dict -- input.csv -o scored.json

# モデルスコアリングを使用
cargo run --release --bin sudachi-dict -- input.csv --model-scores -o scored.json

# モデルとスレッド数を指定
cargo run --release --bin sudachi-dict -- input.csv --model-scores --model jinen-v2-small-q5 --threads 8
```

| オプション | デフォルト | 説明 |
|-----------|----------|------|
| `csv_files` (必須) | — | 入力Sudachi CSVファイル（複数指定可） |
| `-o, --output` | `scored.json` | 出力JSONファイル |
| `--model-scores` | off | モデルNLLスコアリングを使用（デフォルトはSudachiコスト） |
| `--model` | `jinen-v2-xsmall-q5` | モデルバリアントIDまたはGGUFファイルパス |
| `--tokenizer-json` | — | tokenizer.jsonパス（`--model` がGGUFパス時に必要） |
| `--threads` | CPUコア数 / 2 | 並列スコアリングスレッド数 |
| `--n-ctx` | `256` | モデルのコンテキストウィンドウサイズ |

出力JSONは `karukan-dict build` の入力として使用できます。

## karukan-server

ニューラルかな漢字変換を提供するHTTPサーバー。起動時にHuggingFaceからGGUFモデルを自動ダウンロードします。

### 起動

```bash
cargo run --release --bin karukan-server

# オプション
cargo run --release --bin karukan-server -- --port 8080 --host 0.0.0.0 --verbose --debug
```

| オプション | デフォルト | 説明 |
|-----------|----------|------|
| `-p, --port` | `3000` | 待ち受けポート |
| `--host` | `127.0.0.1` | バインドアドレス |
| `-v, --verbose` | off | デバッグレベルのログ出力 |
| `--debug` | off | `/api/tokenize` エンドポイントを有効化 |

### API エンドポイント

| メソッド | パス | 説明 |
|---------|------|------|
| POST | `/api/convert` | ローマ字→ひらがな変換 |
| POST | `/api/reset` | ローマ字変換器をリセット |
| POST | `/api/kanji/convert` | かな漢字変換（ビームサーチ対応） |
| GET | `/api/models` | 利用可能なモデル一覧 |
| GET | `/health` | ヘルスチェック |
| POST | `/api/tokenize` | トークナイズ（`--debug` 時のみ） |

`static/` ディレクトリからWeb UIを配信します。

## ajimee-bench

[AJIMEE-Bench](https://github.com/Ajimee-Bench/AJIMEE-Bench)によるかな漢字変換の精度評価ツール。Exact Match Rate と Character Error Rate (CER) を計算します。

```bash
# 基本的な使い方
cargo run --release --bin ajimee-bench -- evaluation_items.json

# モデルを指定して実行
cargo run --release --bin ajimee-bench -- evaluation_items.json --model jinen-v2-small-q5

# 結果をJSONに保存（サマリーのみ表示）
cargo run --release --bin ajimee-bench -- evaluation_items.json --output results.json --quiet
```

| オプション | デフォルト | 説明 |
|-----------|----------|------|
| `bench_path` (必須) | — | evaluation_items.json のパス |
| `--model` | `jinen-v2-xsmall-q5` | モデルバリアントID |
| `--gguf` | — | GGUFファイルパス（`--model` を上書き） |
| `--tokenizer-json` | — | tokenizer.jsonパス（`--gguf` 使用時に必要） |
| `--output` | — | 詳細結果の出力先JSONファイル |
| `--no-context` | off | 左コンテキストを使用しない |
| `--quiet` | off | サマリーのみ表示 |
| `--n-ctx` | `512` | コンテキストウィンドウサイズ |

## License

MIT OR Apache-2.0
