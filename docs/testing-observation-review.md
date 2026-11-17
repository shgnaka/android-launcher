# Android観測のレビュー記録

2026-11-16。固定済みの `tests/android_requirements_v1.rs` と `tests/support/android.rs` は一切変更していない。元19件の10成功・9失敗も取り消さない。以下は失敗を無条件に製品不具合へ分類しないための別検査であり、元テストの除外・自動成功・閾値緩和ではない。

## 分かったこと

| 元ケース | 元の失敗条件 | 独立して観測した事実 | 扱い |
| --- | --- | --- | --- |
| A11 | Back後に再度開いた一覧の初期表示にCamera Toolsが見つからない | Back後はメモを表示。再度開いた実EditTextの `text` は空。Calendarと3つのCameraが先に並び、Camera ToolsはIMEより下にある。新たにCamera Toolsを検索すると正しい別Activityが起動する | 行の可視性を検索語消去の代用にした観測条件の誤り。製品を誤った初期スクロールへ変更してこの試験を通さない |
| A19 | 「再表示」をタップした直後も非表示指定が残る | OCRが `Camera` と `再表示` を同じ行にまとめ、行の中心x=157をタップ。実ボタン文字の範囲はx=179〜270で、元のタップは間の余白だった。単語の矩形を選ぶと解除・正しい起動・保存を確認できる | タップ対象の観測器の誤り。製品のボタンを隣のラベルまで広げて試験へ追従しない |
| A16 | 入力送信直後のファイルに非表示指定がない | 同じ手順を別観測すると、最初の読取時点（約40ms）で非表示指定を確認できた。adb inputの終了は描画や保存の完了通知ではない | 完了待ちの競合の可能性。元の1回の失敗を「保存できない」と断定しない。新しい観測でも一覧維持側の不一致があり、解決済みとはしない |

レビュー用の実行可能な別ファイルは `examples/review_android_observations_v1.rs`。A11は実EditTextの空語と次の実検索・Activity起動を検査する。A19は対象文字だけの単語矩形でタップし、解除した起動先と保存内容を照合する。A16は非表示確定後の空検索結果を観測の障壁にしてから保存を検査する。今回A11/A19は成功、A16は一覧の空結果を待つ間にホームとなり失敗した。この追加の不一致もログに残す。

```sh
source .local/test-tools/env.sh
cargo test --offline --no-default-features --test ocr_observer_v1
cargo run --offline --no-default-features --example review_android_observations_v1
```

`tests/support/ocr_v1_revision.rs` は別の観測器。対象語を構成する単語だけの矩形を合成する。別行、複数候補、対象と無関係な文字が同じ単語に混ざる場合は推測してタップしない。`tests/ocr_observer_v1.rs` の6件で、隣のラベルを含めない、候補不明・複数をエラーにする等を検査する。この6成功は製品機能の成功件数には数えない。

単語矩形の取得は [Tesseract公式のTSV出力](https://github.com/tesseract-ocr/tesseract/blob/main/doc/tesseract.1.asc) に従う。

証拠は `target/v1-{hide-commit,unhide-commit,ime-clear}-diagnostic.txt`、`target/v1-observation-review.txt`、`target/v1-observation-review/results.json`、`target/v1-android/diagnostic-*` と `review-*` のPNG・XML・TSV・選択座標。元結果は `target/v1-run/android-api35.txt`、API別元画像は `.local/acceptance-runs/api35/target/v1-android`。

今後この観測器を正式な試験版へ採用する場合も、旧版とその結果を残し、変更理由・対象ケース・期待する製品動作が同じであることを別改訂として明示する。元版を黙って書き換えて成功したと報告しない。
