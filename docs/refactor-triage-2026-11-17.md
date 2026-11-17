# 2026-11-17 失敗切り分けと別改訂

`refactor/implementation` 分支で `source .local/test-tools/env.sh && cargo test --no-fail-fast` を実行した結果、279成功・6失敗。固定済み期待値・対象・閾値は変更せず、誤りが判明した試験は別ファイルで改訂した。

## 判定

| 失敗 | 分類 | 根拠 | 対応 |
| --- | --- | --- | --- |
| `src/ui.rs` の `drawer_has_inline_search_and_no_navigation_headings` (`電話` が y<200 にない) | 試験の期待値が陳腐化 | 実描画は Browser 97.5, カメラ 153.5, カレンダー 209.5, メッセージ 265.5, 設定 321.5, 電話 377.5。`replace_shared_apps` の NFKC・小文字キー順が正しく、`app_list_has_a_stable_normalized_order` と整合する | 固定モジュールは保持。`tests/drawer_order_revision_v1.rs` を別改訂として追加し、インライン検索・行の可視性・見出しなしを正しい順序で確認する |
| `a11_one_back_from_the_real_ime_returns_home_and_clears_search` | 観測器の誤り | `docs/testing-observation-review.md` のとおり、Camera Tools は IME 下にあり、行の可視性は検索語消去の代用にならない | 製品・固定試験は変更しない。既存の `examples/review_android_observations_v1.rs` を別観測として保持 |
| `a19_hidden_package_is_managed_only_in_the_dedicated_section` | 観測器の誤り | 同記録のとおり、OCR が `Camera` と `再表示` を同一行とし、余白 x≈157 をタップ。ボタン文字は x=179〜270 | 製品・固定試験は変更しない。単語矩形を使う既存の別観測を保持 |
| `a04_download_archive_contains_exactly_the_installable_apk` | 生成物の古さ | ZIP 内の APK は `2daf2f…` (2026-11-16 の `dist/quiet-launcher-0.4.0.apk`)、比較先 `target/release/apk/quiet-launcher.apk` は `e6942f…` (2026-11-17 13:32 再ビルド) | `dist/quiet-launcher-download.zip` を現行リリース APK から作り直した。`.gitignore` 対象のためソース変更なし |
| `a36_a47_reference_phone_measurements_are_present_and_satisfy_the_frozen_budget` | 環境証跡の欠落 | `target/v1-reference-phone/evidence.json` が存在しない。実 OPPO Reno7 A の計測が必要 | 証跡を捏造せず、実機計測まで失敗のままとする |
| `media::permission_revocation_hides_previously_valid_session_data` | 並列実行時のフレーク | 単独実行・`--test-threads=1`・`requirements_v1` 単体の並列実行では 148/148 成功。全テストバイナリ同時実行時のみ、 denial 前の初期表示で失敗 | 製品・固定試験は変更しない。フル並列時の負荷要因として記録する |

## 変更したもの

- `tests/drawer_order_revision_v1.rs` を追加（固定済み試験と旧結果は保持）。
- `tests/frozen-v1-infrastructure.sha256.json` に新しい改訂ファイルのハッシュを追加。
- `dist/quiet-launcher-download.zip` を現行リリース APK から再生成（git 管理外）。
- 本記録を追加。

変更していないもの: `src/` の製品コード、`tests/` の固定済み期待値、Android 観測の元試験、実機性能の証跡。
