# 要件から作るテスト

採用したスキルと鉄則は [testing-skills.md](testing-skills.md)、固定した入力は [test-fixtures-v1.md](test-fixtures-v1.md)。[対応範囲](testing-v1-coverage.md) は52ケース群ごとに自動化済みと残件を分け、[追加した操作境界](testing-v1-additions.md) に後続8件を記録する。この表に対応があることだけでは、全要件のテスト化・合格を意味しない。

## 実行

Rust、Android SDK/NDK、Java17、adb、英語・日本語対応Tesseractが必要。ローカル復旧用の環境変数は `.local/test-tools/env.sh`。ツールは `.local` に保持し、固定済みヘルパーが使う `/tmp` のパスをそこへのシンボリックリンクにしている。エミュレーターはAPI28/33/35。破損・強制終了試験は実機へ実行しない。

```sh
source .local/test-tools/env.sh
cargo run --offline --no-default-features --example check_v1_contracts
cargo run --offline --example run_acceptance_v1
```

後者はホスト試験を全件実行した後、起動中のAPI28/33/35へAndroid試験を実行する。欠けたAPIや実機証拠を成功扱いにせず、終了コードを非0にする。`target/v1-run/summary.json` と各対象のログを確認する。Androidの画像・OCR・UI階層はAPI別に `.local/acceptance-runs/api*/target/v1-android` へ分ける。同じ画像出力先を複数エミュレーターで共有しない。

APKとフィクスチャは事前ビルドが必要。Androidヘルパーはビルド入力のSHAを照合し、古いAPKを現ソースの試験として扱わない。

```sh
cargo run --offline --no-default-features --example build_acceptance_fixtures
cargo run --offline --no-default-features --example build_widget_fixture
cargo apk build --release --lib --target x86_64-linux-android --target-dir target/emulator
```

`target/v1-sut-build-inputs.sha256` は、そのAPKを作った `src/**/*.rs`・Androidリソース・Cargo.toml/Cargo.lockのハッシュをビルド時に記録する。フィクスチャの記録はビルダーが生成する。ハッシュを更新しただけで再ビルドを代用しない。

個別の切り分け:

```sh
cargo test --offline --features android-check --test requirements_v1
cargo test --offline --test boundaries_v1
cargo test --offline --test navigation_edges_v1
cargo test --offline --test storage_faults_v1
cargo test --offline --test apk_requirements_v1
cargo test --offline --test memory_v1 -- --nocapture
cargo test --offline --test performance_gate_v1 --test performance_matrix_v1
QUIET_SERIAL=emulator-5554 cargo test --offline --test android_requirements_v1
```

個別の成功で全件実行を置き換えない。並行実行するAndroid試験は必ずAPI別の隔離作業ディレクトリを使う。

## 判定の意味

- `requirements_v1` は148件。検索、非表示、保存、キャッシュ、集計、実egui入力・描画、制御時計・天気、メディアの製品側処理を通す。外部Platformだけを制御し、Android連携成立の代用にしない。
- `boundaries_v1` は22件。画面寸法・フォント・明暗・縦画面IMEの矩形、収容超過解除、長い語、メニュー取消、名前と受信更新。横／分割画面の280dp IMEはこの矩形試験の対象外で、未検証として対応表に残す。
- `navigation_edges_v1` は8件。2本指・9dp・縦横比・途中から先頭・慣性・未送信編集と遅い通知を検査する。
- `storage_faults_v1` は20件。Rust製の別プロセスが本物のSettings::saveを呼ぶ。Linux/glibcの書込・fsync・renameにEIOまたはSIGKILLを注入し、注入点へ実際に達したことも必須とする。旧／新の完全な設定、エラー返却、秘匿指定、再試行を観測する。物理的な電源断・Androidファイルシステムの証明ではない。
- `android_requirements_v1` は19件。Rust製の起動先・プレーヤーAPKと標準RemoteViewsを実インストールし、画面OCR・Activity・実セッション・保存内容を観測する。欠けたNotificationListener等は製品不合格、SDK・端末・ビルド証拠不足は環境エラー。
- Android失敗のうち観測方法の問題は [別レビュー](testing-observation-review.md) に記録する。元のテストと失敗を残し、別の単語座標判定器の6件と実画面による別検査を追加した。元19件の結果を黙って置き換えない。
- `apk_requirements_v1` は6件。実APKの署名、ABI、ホーム宣言、バックアップ、配布ZIP等。既存のarm64配布APKを検査し、ホスト試験対象ソースと同じ最新版とは扱わない。
- `memory_v1` はカタログと読み索引の部品割当を実測する1件。UIの補助索引やJVM・フォント・全PSSは含めない。
- 性能判定器の13件は計測器の試験。`reference_performance_v1` は実機証拠を要求する。OPPO実機の全試行、採取器、トレースからのサンプル抽出は未完了。架空のJSONやエミュレーターで実機性能を合格にしない。
- 旧34件と承認済みウィジェット3点E2Eは維持する。過去の実行ログを最新実行へ書き換えない。

OS障害注入の根拠は [Linux fsync](https://man7.org/linux/man-pages/man2/fsync.2.html)、[rename](https://man7.org/linux/man-pages/man2/rename.2.html)、[dlsym/RTLD_NEXT](https://man7.org/linux/man-pages/man3/dlsym.3.html)。ファイル同期とディレクトリ同期を別に検査する。

## 固定と実装修正

`tests/frozen-nov16.sha256.json`・`tests/legacy-tests.sha256.json` が旧契約、`tests/frozen-v1.sha256.json` が新規本体・観測ヘルパー・Androidフィクスチャ、`tests/frozen-v1-infrastructure.sha256.json` が追加境界・保存障害・実行器・対応範囲を保護する。正本5文書は `docs/requirements-v1.sha256` で固定する。

テストを追加する場合は、新しいファイル・期待値・入力・ヘルパーをレビューしてハッシュ対象へ追加する。既存の期待値・対象・閾値を変更しない。固定後は製品コードを直して再実行する。試験自体の誤りが判明した場合は、要件との不整合と旧結果を残した別改訂にする。

今回、制御時計と天気を渡す最小の製品インターフェースを用意した。標準起動では実時計・実天気を使う。コンパイル可能な接続を作る変更と、未合格機能の振る舞いを直す変更を混同しない。メディア取得・天気の詳細方針などはテストで不合格を検出する段階であり、全機能を修正済みではない。
