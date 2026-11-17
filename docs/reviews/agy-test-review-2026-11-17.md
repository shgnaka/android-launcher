Rust製Androidランチャー「Quiet Launcher」のテスト体系に対する**読み取り専用の独立レビュー**を完了しました。
ファイル変更、期待値変更、テストのskip/ignore、凍結ハッシュ更新は一切行っていません。

詳細な分析および全件の検証記録は [test_review_report.md](file:///home/shgnaka/.gemini/antigravity-cli/brain/037357f7-6ef1-447c-bd63-ece9d09da894/test_review_report.md) にまとめています。以下に重要事項を要約してご報告します。

---

### 1. 契約ハッシュ検証結果
[check_v1_contracts.rs](file:///home/shgnaka/program/android-launcher/examples/check_v1_contracts.rs) による検証を実施し、正本5文書（[requirements-v1.sha256](file:///home/shgnaka/program/android-launcher/docs/requirements-v1.sha256)）および全固定契約ファイル（`tests/frozen-*.json`, `tests/legacy-tests.sha256.json`）のハッシュが**完全に一致**していることを確認しました。

---

### 2. 重要度順の指摘事項の要約

#### 【致命的 (Critical)】実装を誤った期待値へ追従させる危険
1. **`overflow_notice.rs` の初回非同期前提の不備（新規発見）**
   - **区分**: **テスト修正**
   - **対象**: [tests/overflow_notice.rs:62-69](file:///home/shgnaka/program/android-launcher/tests/overflow_notice.rs#L62-L69)
   - **内容**: 事前キャッシュ（`apps.json`）を配置せず、初回描画（1フレーム目）で同期的にアプリ名が表示されることを期待して失敗している。
   - **危険性**: これを製品側で解決しようとして、起動時や1フレーム目に `Platform::apps()` をメインスレッドで**同期ブロック呼び出し**させると、実機での IPC 遅延により**ANR（アプリ応答なし）を引き起こす致命的な破壊的変更**となります。製品の非同期読み込みは維持し、テスト側に事前キャッシュ配置等の待機設計を行う必要があります。

2. **A11 行可視性を検索語消去の代用にした誤判定（既知事項）**
   - **区分**: **テスト修正（観測器改訂）**
   - **対象**: [tests/android_requirements_v1.rs:36-38](file:///home/shgnaka/program/android-launcher/tests/android_requirements_v1.rs#L36-L38)
   - **内容**: Back後の再オープン時、IME下の行に「Camera Tools」が隠れていることを失格と判定。製品の初期スクロール位置を不当に変更して追従させてはなりません（NAV-03違反）。

3. **A19 OCR行統合による余白タップ（既知事項）**
   - **区分**: **テスト修正（単語矩形観測器への移行）**
   - **対象**: [tests/support/android.rs:372-380](file:///home/shgnaka/program/android-launcher/tests/support/android.rs#L372-L380)
   - **内容**: ボタン文字とラベルが1行にまとまり、その中心（余白）をタップしていた。製品のボタン判定をラベル側へ不自然に拡大してはなりません。

4. **A16 adb input 後の保存完了待ち競合（既知事項）**
   - **区分**: **テスト修正（観測障壁の設置）**
   - **対象**: [tests/android_requirements_v1.rs:97-105](file:///home/shgnaka/program/android-launcher/tests/android_requirements_v1.rs#L97-L105)
   - **内容**: 入力直後の保存競合。UIスレッドを保存完了まで同期停止させてはなりません（NAV-04 / DATA-03違反）。

---

#### 【高 (High)】正当なテスト失敗（製品側の未実装・修正が必要な事項）
5. **A05 / screens: 同名アプリ存在時のパッケージ識別表示欠落（新規確認）**
   - **区分**: **製品修正**
   - **対象**: [src/mobile.rs:98-153](file:///home/shgnaka/program/android-launcher/src/mobile.rs#L98-L153), [src/ui.rs:1070-1095](file:///home/shgnaka/program/android-launcher/src/ui.rs#L1070-L1095)
   - **根拠**: HOME-02, LIST-01。同名アプリ存在時にパッケージ情報が表示されず、Android連携・ホスト画面双方で失敗。

6. **A27 / A41: メディア連携用 `NotificationListenerService` 欠落（新規確認）**
   - **区分**: **製品修正**
   - **対象**: `AndroidManifest.xml`
   - **根拠**: INFO-04。Manifest に Service 宣言がなく、Android 側で接続不可。

7. **A30: 外部ウィジェットの contentDescription 未抽出・画像エラー文言欠落（新規確認）**
   - **区分**: **製品修正**
   - **対象**: [src/model.rs](file:///home/shgnaka/program/android-launcher/src/model.rs)
   - **根拠**: WIDGET-02。contentDescription を無視して一律「テキストなし」と表示しており、仕様規定の文言（"内容を表示できません"）と乖離。

8. **A29: 外部設定画面から Back 復帰時のホーム遷移欠落（新規確認）**
   - **区分**: **製品修正**
   - **対象**: [src/ui.rs](file:///home/shgnaka/program/android-launcher/src/ui.rs)
   - **根拠**: NAV-03 / 画面状態遷移表。外部 Activity 復帰時はピッカー画面を残さずホームへ直接復帰する規約。

9. **A52: バイナリ Manifest での自動バックアップ無効化欠落（新規確認）**
   - **区分**: **製品修正**
   - **対象**: `AndroidManifest.xml`
   - **根拠**: DATA-04, A52。`android:allowBackup="false"` の宣言が欠落。

10. **A12 / screens: タッチ入力による 500ms 長押しステートマシンの欠落（新規確認）**
    - **区分**: **製品修正**
    - **対象**: [src/mobile.rs:140-153](file:///home/shgnaka/program/android-launcher/src/mobile.rs#L140-L153)
    - **根拠**: NAV-04, HIDE-03。マウスのセカンダリクリック（右クリック）のみ判定しており、実タッチの 500ms 保持判定が存在しない。

---

#### 【中 (Medium)】テスト独立性・観測精度の問題
11. **旧テストにおける一時ディレクトリのクリーンアップ漏れ（新規確認）**
    - **区分**: **テスト改善**
    - **対象**: [tests/nov16_contract.rs](file:///home/shgnaka/program/android-launcher/tests/nov16_contract.rs#L13-L22), [tests/nov16_ui.rs](file:///home/shgnaka/program/android-launcher/tests/nov16_ui.rs#L61-L68)
    - **内容**: RAII型（`support::Directory`）を用いず手動パス生成しているため、テスト終了後も `/tmp` にゴミが残る。

12. **`screen.rs` における画面外・部分可視行の誤判定リスク（新規確認）**
    - **区分**: **テスト観測器改善**
    - **対象**: [tests/support/screen.rs:367-382](file:///home/shgnaka/program/android-launcher/tests/support/screen.rs#L367-L382)
    - **内容**: 画面境界に1pxでもかすったテキストを可視と判定し、クリップされた中心座標をクリックするためタップ位置がずれるリスクがある。

---

### 3. 偽の合格判定への警告
- **判定器の自己テスト成功 ≠ 性能合格**:
  `performance_gate_v1`（10件）および `performance_matrix_v1`（3件）のパスは、判定関数のテストが通っただけであり、**実機（OPPO Reno7 A）の 60/90Hz フレーム維持率や入力遅延 P95 が合格したわけではありません**。
- **ヒープ部品測定 ≠ 全体PSS予算合格**:
  `memory_v1`（1件）のパスは、Rust製カタログ＋読み索引のヒープ測定（8MiB/16MiB）であり、**実機上の JVM、egui テクスチャ、フォントグリフ等を含めた全体 PSS 予算の合格ではありません**。

---

### 4. レビュー範囲と未確認事項

- **確認済み範囲**:
  正本要件書（[requirements.md](file:///home/shgnaka/program/android-launcher/docs/requirements.md) 等全5文書）、固定ハッシュ、テストコード全20ファイル（`tests/v1/*.rs`, `tests/support/*.rs`, `boundaries_v1.rs`, `storage_faults_v1.rs`, `apk_requirements_v1.rs` 等）、`summary.json` および実行ログ。
- **未確認・未実行の試験（不作為の証明）**:
  1. **実機性能試験（OPPO Reno7 A）**: 実機エビデンス（`evidence.json`, `trace.pftrace`）が存在しないため未実行。
  2. **Android API 28 / API 35 エミュレータ試験**: エミュレータ未起動のため未実行（`summary.json` でも environment_error として記録）。
  3. **承認済みウィジェット3点E2E（`widget_e2e.rs`）**: API 35 エミュレータ未起動のため未実行。
  4. **UID別パケットキャプチャ試験**: ネットワーク検証環境未構築のため未実行。

詳細な分析・コード対比は [test_review_report.md](file:///home/shgnaka/.gemini/antigravity-cli/brain/037357f7-6ef1-447c-bd63-ece9d09da894/test_review_report.md) をご参照ください。
