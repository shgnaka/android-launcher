# 2026-11-17 失敗切り分けと改訂

`refactor/implementation` 分支で `source .local/test-tools/env.sh && cargo test --no-fail-fast` を実行した結果、279成功・6失敗。製品コードは変更せず、観測・期待値の誤りが判明した試験のみを改訂した。旧期待値と旧結果は本記録に残す。

## 判定と対応

| 失敗 | 分類 | 根拠 | 対応 |
| --- | --- | --- | --- |
| `src/ui.rs` の `drawer_has_inline_search_and_no_navigation_headings` (`電話` が y<200 にない) | 試験の期待値が陳腐化 | 実描画は Browser 97.5, カメラ 153.5, カレンダー 209.5, メッセージ 265.5, 設定 321.5, 電話 377.5。`replace_shared_apps` の NFKC・小文字キー順が正しく、`app_list_has_a_stable_normalized_order` と整合する | 固定モジュールは保持。`tests/drawer_order_revision_v1.rs` を別改訂として追加し、インライン検索・行の可視性・見出しなしを正しい順序で確認する |
| `a11_one_back_from_the_real_ime_returns_home_and_clears_search` | 観測器の誤り | `docs/testing-observation-review.md` のとおり、Camera Tools は IME 下にあり、行の可視性は検索語消去の代用にならない | 承認済みの別観測 (`tests/android_review_v1_1.rs`) と同じ方法を本試験へ移植。Back 後の実EditTextが空であることを uiautomator で確認し、新しい検索と起動で契約を検証する |
| `a19_hidden_package_is_managed_only_in_the_dedicated_section` | 観測器の誤り | 同記録のとおり、OCR が `Camera` と `再表示` を同一行とし、余白 x≈157 をタップ。ボタン文字は x=179〜270 | `tests/support/ocr_v1_revision.rs` の単語矩形タップへ変更し、解除の保存と起動復帰まで確認する |
| `a04_download_archive_contains_exactly_the_installable_apk` | 生成物の古さ | ZIP 内の APK は `2daf2f…` (2026-11-16 の `dist/quiet-launcher-0.4.0.apk`)、比較先 `target/release/apk/quiet-launcher.apk` は `e6942f…` (2026-11-17 13:32 再ビルド) | `dist/quiet-launcher-download.zip` を現行リリース APK から作り直した。`.gitignore` 対象のためソース変更なし |
| `a36_a47_reference_phone_measurements_are_present_and_satisfy_the_frozen_budget` | 環境証跡の欠落 | `target/v1-reference-phone/evidence.json` が存在しない。実 OPPO Reno7 A の計測が必要 | 証跡を捏造せず、実機計測まで失敗のままとする |
| `media::permission_revocation_hides_previously_valid_session_data` および `media::enumeration_order_does_not_change_the_selected_session` | 並列実行時のフレーク | 単独実行・`--test-threads=1`・`requirements_v1` 単体の並列実行では 148/148 成功。失敗ログでは初期のメディア領域が表示されておらず、非同期取得の完了前に固定フレーム数の待機が終了していた。旧記録の文字シェイプが原因という推測は誤り | 後続修正として、初期セッションの表示を期限付きで待つ |

## 旧期待値と旧結果

- A11: `lab.wait_text("Camera Tools")` (実機で IME 下の行を検出しようとして失敗)。
- A19: `lab.tap_text("再表示")` の行中心タップ後に `hidden_packages` が空でないため `unhide must persist` で失敗。
- いずれも 2026-11-17 の初回実行 (`/tmp/opencode/cargo-test-all.log`) に記録済み。製品側の不具合としては扱わない。

## 変更したもの

- `tests/android_requirements_v1.rs`: A11 は実EditTextの空文字と新規検索による起動、A19 は単語矩形タップと保存・起動復帰を検査するよう改訂。単一の実機で 19件すべて成功。
- `tests/frozen-v1.sha256.json`: 上記ファイルのハッシュを更新。
- `tests/drawer_order_revision_v1.rs` を追加（固定済み試験と旧結果は保持）。
- `tests/frozen-v1-infrastructure.sha256.json` に新しい改訂ファイルのハッシュを追加。
- `dist/quiet-launcher-download.zip` を現行リリース APK から再生成（git 管理外）。
- 本記録を追加。

変更していないもの: `src/` の製品コード、`tests/v1/` 以下の固定済み期待値、実機性能の証跡。

## 後続修正

ユーザーの許可に基づき、旧ドロワーテストの `電話` が y<200 という条件を、正規化順の先頭行 `Browser` が y<200 という条件に改訂した。検索欄、見出しの不在、検索結果なしの検査は維持し、旧テストモジュールのハッシュを更新した。改訂前の期待値と失敗は上表に残す。

メディア試験の初期化は固定30フレームの後に非同期ワーカーが実行済みと仮定していた。上記の2失敗はいずれも初期の `PLAYER ONE` がまだ表示されていない段階で発生している。初期表示を最大5秒待ち、到着しなければ失敗するように改訂した。製品側の性能判定とは別の試験セットアップの待機であり、旧期待値と後続の検査は維持する。
