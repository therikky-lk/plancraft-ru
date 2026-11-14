# Upstream integration audit

調査日: 2026-11-15

## 対象と作業場所

- 作業ブランチ: `codex/merge-upstream-features`
- ローカル開始点: `51da785` (`main`)
- 取り込み元: https://github.com/rallegretti/PlanCraft
- Git remote: `upstream`（元リポジトリの今後の更新取得用）。`origin`は変更しない。
- 取り込み対象: `ada13f4332bb6aa96d904a86f09714d742027233` (upstream `main`, v0.4.1)
- 参照: `refs/remotes/upstream/main`。fetch時点の対象を固定して比較する。
- 開始時の作業ツリーはクリーン。

## 統合方法

`git merge-base HEAD upstream/main` に共通祖先はない。現在のリポジトリは
`02c7913` / `b4cacfd` から始まる別履歴であり、通常の三方向マージの基点を持たない。
`--allow-unrelated-histories` で全面的な add/add 競合を作るのではなく、現在の実装を
土台として機能単位で移植し、出典コミットと互換性を追跡する。

機能移植を実装済み。画面・互換性・異常入力・性能の検証結果は末尾に記録する。
Git上の共通祖先の代わりに、変更を比較する技術的な参照点として upstream の `d3cee31`
を利用した。これは両リポジトリの共通祖先を意味しない。

## 機能の対応

| 項目 | 現在の実装・相違点 | 統合時の対応 |
| --- | --- | --- |
| Multiple Baselines Gantt | `extras.rs` に番号付きベースライン・中間計画がある | 比較ビューを追加し、既存保存値を維持する |
| Network Diagram | upstream に `network.rs` と専用ビューがある | レイアウト・操作・MCP・印刷を組み込む |
| Task Board | upstream に boardモデル・ビュー・MCPがある | 列と状態の保存、操作、印刷を組み込む |
| Print / Export PDF | 既存は `views/print_gantt/` のWindowsネイティブ印刷。upstreamは共通印刷モデルと自前PDF出力 | 標準印刷を保持。新プレビューのページも同じWindows印刷境界へ渡す |
| Split / Rejoin | 現在は `Task.resume` による未完了作業の延期。upstreamは `splits` / `gaps` を持つ | 複数分割・再結合を追加し、旧データとスケジュールを検証する |
| Custom fields / columns | 現在の列表示に独自拡張がある。upstreamに型付きカスタム値・列定義・保存がある | 既存列と表示操作を保ち、追加定義を統合する |
| AI assistants | 現在すでにClaude Code・Codex・GitHub Copilotの案内がある | 案内を維持し、新機能向けMCPツールを追加する |
| Recurring tasks | 双方に実装があるが型と編集方式が異なる | 既存データの読み込みと編集を守りながら統合する |
| Status date / Update Project | 現在も `extras.rs` に存在する | upstreamの追跡処理と比較し、重複実装を整理する |
| Microsoft Project XML (MSPDI) | 現在は `interchange.rs`、upstreamは `mspdi/` | 対応範囲・警告・往復互換性を比較して統合する |
| Fonts / RTL | 現在は多言語UIとフォント設定。upstreamはPDFのフォールバック・RTLも追加 | 既存10言語UIを維持し、追加ビュー・印刷へ適用する |
| Packaging | upstreamにLinux配布・Mac署名関連の変更がある | 自動適用せず、既存CI・所有者環境の配布方針と比較する |

## 保持する独自機能

- 10言語カタログ、初回言語検出、言語・フォント・通貨設定。
- Office風リボン、幅に応じた段階的な縮小、日付ピッカー。
- グリーンのブランド、既存テーマとロゴ。
- カスタムレポート、添付ファイル、既存インポート・エクスポート形式。
- 現在のMCP接続案内、認証・セッション保護。
- 既存CI、所有者のマシン上でのビルド・配布方針。

## 推奨する実装順と検証

1. 現在のテスト結果を記録し、既存モデルの保存・読み込みを固定する。
2. カスタムフィールド、ベースライン、分割、繰り返し、追跡のモデルと操作を統合する。
   既存形式からの読み込み、保存往復、Undo/Redo、異常入力を検証する。
3. Network Diagram / Task Board / Multiple Baselines Ganttを追加し、
   現在のリボン・テーマ・翻訳に接続する。
4. 印刷とPDFを統合し、日本語・改ページ・各ビューの出力を確認する。
5. MSPDIとMCPを統合し、既存機能・セキュリティの回帰を確認する。
6. `cargo fmt --all`、必要なテスト・画面確認、`cargo xtask ci` を実施する。
   コミットは `cargo xtask ci` 完了後に行う。ビルド成功と手動UI確認は分けて記録する。

## 利用者への確認

- Windows印刷: 利用者が「現在のWindows印刷を残してPDF出力などを追加」を選択済み。
  新プレビューからもWindows標準ダイアログを開き、PDFビューアー経由へ置き換えない。
- 金額表示: 統合前から表示の切り上げとテストの四捨五入期待値が食い違っていた。
  利用者が切り上げ維持を選択したため、表示処理を維持してテストを修正した。
  既存の小数値、負数、通貨の小数桁0についても期待値を検証する。
- このほかデータ・操作の仕様選択が必要になった場合も、具体的な差異を示して確認する。

## 変更前の検証結果

`cargo test --workspace --locked` を実行。コンパイル完了後、appのテストで停止した。
app: 36成功、2失敗、1無視。後続のcore/MCP等の実行結果はこのコマンドからは得られていない。

- `format::tests::units_and_money`: 金額の丸め。実際は `$1,234,567.90`、期待値は `$1,234,567.89`。
- `theme::tests::ui_text_avoids_glyphs_missing_from_bundled_fonts`: 日付ピッカーの `▦` が
  バンドルフォントに存在しない。
- Windows PDFドライバーの印刷テストは明示的にignored。

上記2件は機能移植前の現行コードで再現。統合で発生した回帰と区別する。

## 実装と互換性

- Network Diagram、Task Board、Multiple Baselines Gantt、分割・再結合、型付きカスタム
  フィールド、列の挿入・非表示・移動、並べ替え・グループ化を現在のリボンへ接続した。
- 番号付き基準計画、中間計画、選択した基準計画によるアーンドバリューに対応。
  既存のアーンドバリュー表・カスタムレポートも同じ基準計画を参照する。
- 新しい繰り返しタスクダイアログ・進捗更新・MCPツール・MSPDI入出力を取り込んだ。
  既存のテンプレート、CSV/Excelインポート、WBS・ハイパーリンク・コスト計上方法を扱う
  XMLエクスポートも残した。新MSPDIと拡張フィールド付きXMLはFileのExportから選べる。
- 旧 `recurrence` 定義と新 `recurrence_pattern` は別々に保存して旧定義を失わない。
  upstreamの `recurrence` キーに新定義が入っているファイルも読み込み時に識別する。
  旧 `resume` は再計算時に新しい `splits` へ変換し、Stop/Resume・作業配分・印刷を共有する。
- 表の保存名は翻訳表示名から分離した安定キーにした。言語切り替えによって保存列が
  見えなくなることを防ぐ。既存10言語カタログ・フォント選択・通貨設定は保持した。
- 印刷: Ctrl+Pは共通プレビュー、Ctrl+Shift+Pは従来ガントプレビュー。
  Windowsではどちらも標準印刷ダイアログを使用。共通プレビューはPDFを直接保存できる。
  PDFはcoreのフォント埋め込み付き出力、標準プリンターへの出力は既存境界の300dpi画像。
- 新しい画面の日本語文言を追加した。他言語の既存訳は保持し、追加文言の未翻訳分は
  英語を表示する。未翻訳を翻訳完了とは扱わない（`docs/LOCALIZATION.md`参照）。
- CI・配布設定、署名、ブランディング、所有者のビルド環境は置き換えていない。

## 検証記録

- `cargo clippy --workspace --all-targets --locked -- -D warnings`: 通過。
- `cargo test --workspace --locked`: 金額テストを含め407件成功、失敗0、11件無視。
  無視されたものには別途実行済みのリリース性能テストとWindowsドライバーテストも含む。
- `cargo xtask ci`: 利用者の回答に従って金額テストを修正後、全チェック通過。
  fmt、Clippy（警告をエラー扱い）、通常テスト407件、リリース性能テスト10件、
  cargo-denyを実行し、合計417件成功・失敗0・フィルターによる除外0。
- `cargo test --release --locked -p plancraft-core --test speed -- --nocapture`:
  10件通過。200タスク再計算は10msの既存上限内。
  平準化の試行日計算を増分化し、1,000タスクを約1.67秒で処理（上限2秒は変更していない）。
  短い稼働日・休日・分割を含め、元の絶対日計算と一致する回帰テストを追加した。
- `cargo deny check`: advisories / bans / licenses / sourcesすべて通過。
  未導入だったcargo-denyを導入。検査で見つかった既存依存関係を
  calamine 0.38.0、async-executor 1.14.1、clipboard-win 5.4.3へ更新した。
- `cargo tree -p plancraft-app -i cc -e build`: 該当なし。
- Windowsのignoredドライバーテストを明示実行して通過。
  `target/print-proof-native.pdf` を生成。物理プリンターには出力していない。
- 新旧形式の保存・読み込み、添付・メモ・基準計画・WBS・通貨設定の保持、
  新機能のUndo/Redoを `tests/upstream_integration.rs` で検証する。
- 日本語の実アプリ画面をスクリーンショットで確認。ネットワーク図、ボード、印刷プレビュー。
  書式リボンのはみ出しを修正。新プレビューからWindowsダイアログへの手動操作や
  実機の紙への印刷、Linux/macOSでの実行は未確認。
- 最新のdebugアプリをビルド済み: `target/debug/plancraft.exe`。
  `target/upstream-baselines-ja.png`、`target/upstream-network-ja.png`、
  `target/upstream-board-ja.png`、`target/upstream-print-ja.png` が画面確認資料。
- `cargo run -p plancraft-core --example check_locales --locked`: 全10言語で不足キー0。
  翻訳の完了判定ではなく、カタログのキー整合性の検査である。
