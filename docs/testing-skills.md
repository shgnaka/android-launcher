# テスト設計に採用したスキルと鉄則

2026-11-16。skill-installerで公式curated一覧を確認し、このRust/NativeActivity構成に直接適合する専用テストスキルがないため、配布元の資料を追加調査した。常用環境への追加は行わず、調査用ディレクトリへ取得して読んだ。参照した版は [取得元の記録](testing-skill-sources.json)。

| 採用元 | 使う部分 | このプロジェクトでの適用 |
| --- | --- | --- |
| [obra/superpowers: test-driven-development](https://github.com/obra/superpowers/tree/main/skills/test-driven-development) | テスト先行、失敗理由の確認、最小の製品修正、実装への追従を避ける | 新規契約の振る舞いを先に固定。既存動作の正しいテストは最初から成功してよい、というユーザーの指定を優先する。既存製品を削除して書き直す指示は適用しない |
| [同: writing-good-tests](https://github.com/obra/superpowers/blob/main/skills/test-driven-development/writing-good-tests.md) | 何の不具合を検出するか、独立した期待値、モック・ソース文字列テストの乱用を避ける | テスト名と要件IDを結び、手で決めた正解を使う。ファイル実入出力と実eguiの描画・入力を使う |
| [同: verification-before-completion](https://github.com/obra/superpowers/tree/main/skills/verification-before-completion) | 最新の実行証拠を確認してから合否を述べる | 件数・失敗・環境不足を別々に報告。通った一部だけで全体充足と言わない |
| [Google android/skills: testing-setup](https://github.com/android/skills/tree/main/testing/testing-setup) | ロジック／UI／実Androidを分ける、制御された外部依存、復元・画面構成・実機連携を検査 | Kotlin/Compose/Gradle/Hiltの導入手順はRustというユーザー制約に適合しないため使わない。既存のcargo test・egui・JNI・adb・OCR構成を使う |

## 今回固定する鉄則

1. 正解は凍結済み要件と手で確認した入力から決める。現在の実装出力を正解として保存しない。
2. テストを書く前に「どの誤りを入れたら失敗するか」を説明する。1つの振る舞い／境界を1つの識別可能なテストにする。
3. 入力→実際の製品処理→見える結果／保存内容／送信先を検査する。テスト内で作った偽物の正しさを製品の証拠にしない。
4. 外部依存だけを制御する。ファイル保存・検索・描画・画面遷移は本物を使う。Android固有の成立は実Androidで別に検査する。
5. 実行できる状態で失敗を観測する。構文エラー・未定義API・環境なしを、要件違反のRedとして数えない。
6. 執筆・レビューを終えたテスト本文だけでなく、期待値・入力・ヘルパー・判定器・実行対象をハッシュで凍結する。凍結後に実装を変更する。
7. 成功済みの機能は要件に合う成功として残す。失敗させるために期待値を変えない。必要に応じて隔離した変異検査で検出力を確認する。
8. 不合格は不合格のまま報告する。skip、ignore、空のassert、欠けた環境での自動成功、特定ケースだけの実行除外をしない。
9. UIは実操作と描画を検査する。画像保存だけでは合格にせず、文字・矩形・クリップ・操作先を固定基準で観測する。
10. モックの成功をAndroidの成功に数えず、エミュレーター時間をOPPO実機のFPSに置き換えない。
11. 試験は独立した一時保存先を使い、終了時に整理する。故障注入・時計変更はユーザー実機に行わない。
12. 完了報告は最新の全対象の実行結果、未実行の理由、凍結チェック、要件別の範囲を添える。テスト件数を要件充足率に換算しない。

既存の仕様・契約・34件のテストとE2Eは保持する。今回のテスト追加と、未実装製品をすべて合格へ直す工程は記録上分ける。
