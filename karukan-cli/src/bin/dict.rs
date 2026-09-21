use anyhow::Result;
use axum::{
    Json, Router,
    extract::{Query, State},
    response::Html,
    routing::get,
};
use clap::{Parser, Subcommand};
use karukan_engine::dict::{
    DictSource, Dictionary, ReadingMap, detect_source, layer_readings, merge_readings, read_source,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;

/// karukan dictionary tool — build and view dictionaries.
#[derive(Parser, Debug)]
#[command(name = "karukan-dict")]
#[command(about = "karukan dictionary tool — build and view dictionaries")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Build a binary dictionary from one or more source files.
    ///
    /// Formats (auto-detected per file, or forced for all with --format):
    /// - `json`: Array of {reading, candidates: [{surface, score}]} (.json)
    /// - `sudachi`: SudachiDict CSV (.csv)
    /// - `mozc-system`: Mozc system dictionary TSV (reading\tlid\trid\tcost\tsurface)
    /// - `mozc`: Mozc/Google IME user dictionary TSV (reading\tword\tPOS\tcomment)
    ///
    /// The inputs are layers, in order: a (reading, surface) pair takes the
    /// score of the first layer that has it, and a pair first seen in the
    /// i-th layer is scored `score + 100000 * i`, so for the same reading an
    /// earlier layer's words always sort before a later layer's. Adjacent
    /// files of the same format form one layer (one dictionary shipped as
    /// several files), a pair in two of them keeping the lower score.
    Build {
        /// Input dictionary files, highest-priority first
        #[arg(required = true)]
        inputs: Vec<PathBuf>,

        /// Output binary dictionary file
        #[arg(short, long, default_value = "dict.bin")]
        output: PathBuf,

        /// Input format for every file: json, sudachi, mozc-system or mozc
        /// (auto-detected per file if omitted)
        #[arg(short, long)]
        format: Option<String>,
    },

    /// Dictionary viewer and inspector (web UI + CLI search).
    ///
    /// Without CLI search options, launches a local web server with a search
    /// interface. With --query or --all, prints results to stdout (CLI mode).
    View {
        /// Dictionary files to load (KRKN binary or Mozc TSV, auto-detected)
        #[arg(required = true)]
        dicts: Vec<PathBuf>,

        /// Port to listen on (web mode only)
        #[arg(long, default_value = "8080")]
        port: u16,

        /// Host to bind to (web mode only)
        #[arg(long, default_value = "127.0.0.1")]
        host: String,

        /// Search query (CLI mode: print results and exit)
        #[arg(short, long)]
        query: Option<String>,

        /// Search by surface form instead of reading
        #[arg(short, long)]
        surface: bool,

        /// Use prefix search instead of exact match (reading search only)
        #[arg(short, long)]
        prefix: bool,

        /// Show all entries (dump entire dictionary)
        #[arg(short, long)]
        all: bool,
    },
}

// --- build subcommand ---

/// Score step between layers: above every score any source produces
/// (Sudachi's unknown-word fallback is 99999), so layer order is strict.
const LAYER_STEP: f32 = 100_000.0;

fn run_build(inputs: Vec<PathBuf>, output: PathBuf, format: Option<String>) -> Result<()> {
    let forced = match format.as_deref() {
        Some(name) => Some(DictSource::from_name(name).ok_or_else(|| {
            anyhow::anyhow!(
                "Unknown format: {name}. Use 'json', 'sudachi', 'mozc-system' or 'mozc'."
            )
        })?),
        None => None,
    };

    // Adjacent files of one format are one dictionary split across files:
    // fold them into one layer before laying it under the earlier ones.
    let mut merged = ReadingMap::new();
    let mut layer_index = 0usize;
    let mut layer: Option<(DictSource, ReadingMap)> = None;
    let mut flush = |layer: Option<(DictSource, ReadingMap)>, merged: &mut ReadingMap| {
        if let Some((source, readings)) = layer {
            let pairs: usize = readings.values().map(|s| s.len()).sum();
            let added = layer_readings(merged, readings, LAYER_STEP * layer_index as f32);
            eprintln!(
                "Layer {layer_index} ({}): {pairs} pairs, {added} new",
                source.name()
            );
            layer_index += 1;
        }
    };
    for input in &inputs {
        let source = match forced {
            Some(source) => source,
            None => detect_source(input)?,
        };
        eprintln!("Reading {:?} (format: {})...", input, source.name());
        let readings = read_source(input, source)?;
        match &mut layer {
            Some((current, acc)) if *current == source => merge_readings(acc, readings),
            _ => {
                flush(layer.take(), &mut merged);
                layer = Some((source, readings));
            }
        }
    }
    flush(layer.take(), &mut merged);

    let readings = merged.len();
    let pairs: usize = merged.values().map(|s| s.len()).sum();
    eprintln!("Building trie: {pairs} pairs over {readings} readings...");
    let dict = Dictionary::from_readings(merged)?;

    eprintln!("Saving to {:?}...", output);
    dict.save(&output)?;

    eprintln!("Done.");
    Ok(())
}

// --- view subcommand ---

#[derive(Clone)]
struct AppState {
    dict: Arc<Dictionary>,
}

#[derive(Deserialize)]
struct SearchParams {
    q: Option<String>,
    #[serde(default)]
    mode: SearchMode,
}

#[derive(Deserialize, Default, Clone, Copy)]
#[serde(rename_all = "snake_case")]
enum SearchMode {
    #[default]
    Reading,
    Prefix,
    Surface,
}

#[derive(Serialize)]
struct SearchResult {
    reading: String,
    surface: String,
    score: f32,
}

#[derive(Serialize)]
struct SearchResponse {
    query: String,
    mode: String,
    results: Vec<SearchResult>,
    count: usize,
}

async fn index() -> Html<&'static str> {
    Html(INDEX_HTML)
}

async fn api_search(
    State(state): State<AppState>,
    Query(params): Query<SearchParams>,
) -> Json<SearchResponse> {
    let query = params.q.unwrap_or_default();
    let mode = params.mode;

    if query.is_empty() {
        return Json(SearchResponse {
            query,
            mode: mode_name(mode).to_string(),
            results: Vec::new(),
            count: 0,
        });
    }

    let results: Vec<SearchResult> = match mode {
        SearchMode::Reading => match state.dict.exact_match_search(&query) {
            Some(result) => result
                .candidates
                .iter()
                .map(|c| SearchResult {
                    reading: result.reading.to_string(),
                    surface: c.surface.clone(),
                    score: c.score,
                })
                .collect(),
            None => Vec::new(),
        },
        SearchMode::Prefix => state
            .dict
            .common_prefix_search(&query)
            .into_iter()
            .flat_map(|result| {
                result.candidates.iter().map(move |c| SearchResult {
                    reading: result.reading.to_string(),
                    surface: c.surface.clone(),
                    score: c.score,
                })
            })
            .collect(),
        SearchMode::Surface => state
            .dict
            .search_by_surface(&query)
            .into_iter()
            .map(|(reading, surface, score)| SearchResult {
                reading,
                surface,
                score,
            })
            .collect(),
    };

    let count = results.len();
    Json(SearchResponse {
        query,
        mode: mode_name(mode).to_string(),
        results,
        count,
    })
}

fn mode_name(mode: SearchMode) -> &'static str {
    match mode {
        SearchMode::Reading => "reading",
        SearchMode::Prefix => "prefix",
        SearchMode::Surface => "surface",
    }
}

async fn run_view(
    dicts: Vec<PathBuf>,
    port: u16,
    host: String,
    query: Option<String>,
    surface: bool,
    prefix: bool,
    all: bool,
) -> Result<()> {
    eprintln!("Loading dictionaries...");
    let mut loaded = Vec::new();
    for path in &dicts {
        eprintln!("  Loading {:?}...", path);
        let dict = Dictionary::load_auto(path)?;
        loaded.push(dict);
    }

    let dict = if loaded.len() == 1 {
        // Safety: len() == 1 guarantees next() returns Some
        loaded.into_iter().next().expect("single dictionary loaded")
    } else {
        Dictionary::merge(loaded)?.expect("at least one dictionary loaded")
    };

    eprintln!("Dictionary loaded.");

    // CLI mode: --all
    if all {
        let mut stdout = std::io::stdout().lock();
        let count = dict.dump_all(&mut stdout)?;
        eprintln!("({} entries total)", count);
        return Ok(());
    }

    // CLI mode: --query
    if let Some(query) = &query {
        if surface {
            let results = dict.search_by_surface(query);
            if results.is_empty() {
                eprintln!("No entries found with surface containing \"{}\"", query);
            } else {
                for (reading, surface, score) in &results {
                    println!("{}\t{}\t{}", reading, surface, score);
                }
                eprintln!("({} results)", results.len());
            }
        } else if prefix {
            let results = dict.common_prefix_search(query);
            if results.is_empty() {
                eprintln!("No entries found with reading prefix \"{}\"", query);
            } else {
                for result in &results {
                    for cand in result.candidates {
                        println!("{}\t{}\t{}", result.reading, cand.surface, cand.score);
                    }
                }
                eprintln!("({} readings matched)", results.len());
            }
        } else {
            match dict.exact_match_search(query) {
                Some(result) => {
                    for cand in result.candidates {
                        println!("{}\t{}\t{}", result.reading, cand.surface, cand.score);
                    }
                    eprintln!("({} candidates)", result.candidates.len());
                }
                None => {
                    eprintln!("No entry found for reading \"{}\"", query);
                }
            }
        }
        return Ok(());
    }

    // Web server mode
    let state = AppState {
        dict: Arc::new(dict),
    };

    let app = Router::new()
        .route("/", get(index))
        .route("/api/search", get(api_search))
        .with_state(state);

    let addr = format!("{}:{}", host, port);
    eprintln!("Starting dict-viewer at http://{}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

// --- main ---

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Build {
            inputs,
            output,
            format,
        } => run_build(inputs, output, format),
        Commands::View {
            dicts,
            port,
            host,
            query,
            surface,
            prefix,
            all,
        } => run_view(dicts, port, host, query, surface, prefix, all).await,
    }
}

const INDEX_HTML: &str = r##"<!DOCTYPE html>
<html lang="ja">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>karukan dict-viewer</title>
<style>
  * { box-sizing: border-box; margin: 0; padding: 0; }
  body {
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", "Noto Sans JP", sans-serif;
    background: #f5f5f5; color: #333; padding: 2rem;
    max-width: 900px; margin: 0 auto;
  }
  h1 { font-size: 1.5rem; margin-bottom: 1rem; }
  h1 span { color: #888; font-weight: normal; font-size: 0.9rem; }
  .search-box {
    display: flex; gap: 0.5rem; margin-bottom: 1rem;
    flex-wrap: wrap;
  }
  input[type="text"] {
    flex: 1; min-width: 200px; padding: 0.6rem 1rem;
    border: 2px solid #ddd; border-radius: 8px; font-size: 1rem;
    outline: none; transition: border-color 0.2s;
  }
  input[type="text"]:focus { border-color: #4a90d9; }
  .mode-btns { display: flex; gap: 0.25rem; }
  .mode-btns button {
    padding: 0.6rem 1rem; border: 2px solid #ddd; background: #fff;
    border-radius: 8px; cursor: pointer; font-size: 0.85rem;
    transition: all 0.2s;
  }
  .mode-btns button.active {
    background: #4a90d9; color: #fff; border-color: #4a90d9;
  }
  .mode-btns button:hover:not(.active) { border-color: #999; }
  .status {
    font-size: 0.85rem; color: #888; margin-bottom: 0.75rem;
  }
  table {
    width: 100%; border-collapse: collapse; background: #fff;
    border-radius: 8px; overflow: hidden;
    box-shadow: 0 1px 3px rgba(0,0,0,0.1);
  }
  th {
    background: #4a90d9; color: #fff; padding: 0.6rem 1rem;
    text-align: left; font-size: 0.85rem;
  }
  td {
    padding: 0.5rem 1rem; border-bottom: 1px solid #eee;
    font-size: 0.95rem;
  }
  tr:hover td { background: #f0f6ff; }
  .score { color: #888; font-size: 0.85rem; }
  .empty {
    text-align: center; padding: 3rem; color: #999; font-size: 0.95rem;
  }
  .reading-col { color: #4a90d9; font-weight: 500; }
  kbd {
    background: #eee; border: 1px solid #ddd; border-radius: 3px;
    padding: 0.1rem 0.3rem; font-size: 0.8rem;
  }
</style>
</head>
<body>

<h1>karukan dict-viewer <span>辞書ビューア</span></h1>

<div class="search-box">
  <input type="text" id="query" placeholder="検索... (ヨミ or 表層形)" autofocus>
  <div class="mode-btns">
    <button class="active" data-mode="reading">ヨミ (完全一致)</button>
    <button data-mode="prefix">ヨミ (前方一致)</button>
    <button data-mode="surface">表層形</button>
  </div>
</div>

<div class="status" id="status">検索クエリを入力してください</div>

<div id="results"></div>

<script>
const query = document.getElementById('query');
const status = document.getElementById('status');
const results = document.getElementById('results');
const modeButtons = document.querySelectorAll('.mode-btns button');
let currentMode = 'reading';
let debounceTimer = null;

modeButtons.forEach(btn => {
  btn.addEventListener('click', () => {
    modeButtons.forEach(b => b.classList.remove('active'));
    btn.classList.add('active');
    currentMode = btn.dataset.mode;
    doSearch();
  });
});

query.addEventListener('input', () => {
  clearTimeout(debounceTimer);
  debounceTimer = setTimeout(doSearch, 200);
});

query.addEventListener('keydown', (e) => {
  if (e.key === 'Enter') {
    clearTimeout(debounceTimer);
    doSearch();
  }
});

async function doSearch() {
  const q = query.value.trim();
  if (!q) {
    status.textContent = '検索クエリを入力してください';
    results.innerHTML = '';
    return;
  }

  status.textContent = '検索中...';

  try {
    const res = await fetch(`/api/search?q=${encodeURIComponent(q)}&mode=${currentMode}`);
    const data = await res.json();

    if (data.count === 0) {
      status.textContent = `「${data.query}」: 結果なし`;
      results.innerHTML = '<div class="empty">一致するエントリが見つかりませんでした</div>';
      return;
    }

    status.textContent = `「${data.query}」: ${data.count} 件`;

    let html = '<table><thead><tr><th>ヨミ</th><th>表層形</th><th>スコア</th></tr></thead><tbody>';
    for (const r of data.results) {
      html += `<tr>
        <td class="reading-col">${esc(r.reading)}</td>
        <td>${esc(r.surface)}</td>
        <td class="score">${r.score}</td>
      </tr>`;
    }
    html += '</tbody></table>';
    results.innerHTML = html;
  } catch (e) {
    status.textContent = 'エラー: ' + e.message;
  }
}

function esc(s) {
  const d = document.createElement('div');
  d.textContent = s;
  return d.innerHTML;
}
</script>
</body>
</html>
"##;
