# Quiet Launcher

Rust で実装する、テキスト中心の Android ホームアプリです。アプリの独自 Java / Kotlin ソースはありません。`egui` / `eframe` と Android の `NativeActivity` で描画し、Android API は Rust の JNI から呼び出します。Android 標準のクラスや各依存ライブラリは使用します。

全体の仕様は [全体要件v1.0](docs/requirements.md) を正本とします。既存の54要件IDを維持し、[受け入れケース52群](docs/acceptance-plan.md) と [固定入力・計測方法](docs/test-fixtures-v1.md) を定めました。外部ウィジェットの最低E2E合格ラインも3点でユーザー確認済みです。仕様の確定は現行APKの全件合格を意味しません。

0.4.0 では、非表示アプリ、80〜160%の文字サイズ、アプリ一覧の永続キャッシュ、かな・ローマ字と一部ブランド名の読み検索、今日／直近24時間の画面時間切替、設定のバックアップと明示的な復旧を追加しました。ホームは1画面とし、収まらない追加・文字サイズ変更はエラーで知らせ、登録内容を保持します。[追加契約](docs/contracts-2026-11-16.md) と [検証の範囲](docs/nov16-validation.md) を参照してください。メディアセッション操作など、全体要件のうち未実装の項目もあります。

0.3.0 では Fokus を参考に、上スワイプで下から開く独立したアプリ一覧と、その場で入力する検索欄に変更しました。「アプリ ↑」やホーム・アプリのヘッダーはありません。設定はホームの空白部分の長押し、または一覧右上の三点メニューから開きます。

0.2.0 ではタッチ終了時の慣性停止、全件の毎フレーム描画、長い文字列による横はみ出し、透明な背景による文字の読みにくさを修正しました。詳細は [修正の記録](docs/implementation-notes.md)、開発支援の候補は [スキル調査](docs/skills.md) を参照してください。

0.2.1 は Android 15 エミュレーターでインストール・操作・標準ホームへの復帰を検証しました。端末上で見つかったシステムバーとの重なり、検索結果が隠れる問題、Activity 再作成時の描画停止を修正しています。[検証結果と再実行手順](docs/android-testing.md) を参照してください。

0.2.2 では手入力のテキストウィジェットを削除しました。他アプリのウィジェットは提供元の名前を先に取得・保存し、許可待ちや内容の読み取りに失敗した場合もその名前を表示します。既存のお気に入りや外部ウィジェットの登録は引き継ぎます。

## 使い方

1. Android に APK をインストールして Quiet を開く。
2. ホームの空白部分を長押しし、「標準のホームアプリに設定」で Quiet を選ぶ。
3. ホームで指を上にスワイプすると、アプリ一覧が下から開く。「＋」でホームに追加、「−」で外す。
4. アプリ名をタップして起動する。ホームの表示順は設定で変更できる。
5. 一覧上部の検索欄にアプリ名を入力すると、その場で完全一致で絞り込む。空欄なら全件表示する。

一覧の検索欄は上部に残ります。一覧のアプリ名を長押しすると、ホームへの追加・解除、非表示、検索の読み設定を選べます。非表示はホーム・一覧・検索に反映し、設定の非表示アプリから解除できます。通常の並べ替え設定には表示しません。ホームのアプリ名には長押しメニューを出しません。一覧の先頭から下へ引くか、戻る操作でホームへ戻り、検索語を消します。Android の入力ダイアログ等を含む全経路の確認は未完了です。権限の許可や天気の地域指定は設定画面で行います。

Android の検索入力には画面内の標準 EditText を使い、日本語 IME に対応します。検索にダイアログや確定ボタンはありません。天気の緯度・経度の編集は従来どおり入力ダイアログを使います。

## 実装している機能

| 機能 | 動作 |
| --- | --- |
| ホーム | 時計・年月日・曜日・電池残量、お気に入りアプリ、明暗切り替え |
| アプリ一覧 | 起動可能な Activity を列挙。コンポーネント単位で保存し、同名の別アプリも保持 |
| 検索 | かな・カナ・ローマ字の読みをそろえた完全一致。LINE／らいん等の小さい組み込み辞書とユーザー追加読みを利用。ネットワーク・大型モデル不要 |
| 天気 | 手入力した緯度・経度で Open-Meteo から取得。約15分ごとに更新。取得日時と通信エラーを表示 |
| スクリーンタイム | Usage Access 許可後、ローカル午前0時〜現在、または直近24時間の画面インタラクティブ時間を概算。ロック除外時間やアプリ別時間ではない |
| メディア | セッション別取得・記号操作・横スワイプの要件を定義。Android連携は未実装で、現在ホームには操作欄を出さない |
| 他アプリのウィジェット | AppWidgetHost でバインドし、提供元の名前と View 内の文字・説明文を表示。追加・許可・初回設定・削除に対応 |
| 保存 | 設定を検証して同期・置換、前回正常値のバックアップと明示的な復旧。破損した元データは保持 |
| 一覧キャッシュ | 起動先・名前・世代のみを保存。初回描画から再利用し、世代が変わった場合に再列挙。メモリ内の一覧は共有 |

## 制限と実機確認

- Android 9（API 28）以降、まず arm64 を対象にしています。ワークプロファイルや別ユーザーのアプリ、ショートカットは対象外です。
- メディアセッションの列挙・個別制御と通知アクセス連携は未実装です。従来の全体メディアキー送信は新しい要件を満たすものとして扱いません。
- ウィジェットの画像・Canvas・WebView・一部の一覧型 RemoteViews・表示中でなければ更新しない内容は読み取れません。抽出したテキストから元のボタン操作はできません。画面に取り付けない View の内容を読むため、全ウィジェットとの互換性はありません。独自描画の情報が必要な場合は、そのアプリの公開 API を使う個別の情報プロバイダーを追加します。
- ウィジェットの許可・設定は Android / 提供アプリの画面で行います。許可をキャンセルした項目は設定から削除してください。設定をキャンセルして内容が出ない場合は削除して追加し直します。追加直後の強制終了を含む Activity 結果の厳密な追跡は今後の改善項目です。
- 画面点灯履歴は端末実装や履歴の欠落に依存します。Digital Wellbeing のアプリ別使用時間とは定義が異なります。表示は概算です。
- 日本語フォントは端末の Noto CJK を利用します。独自フォント構成の端末では `src/ui.rs` の候補を追加するか、ライセンスを確認したフォントを同梱してください。
- 実機の権限画面、ホーム復帰、IME、メディアアプリ、各ウィジェットの互換性は、下記のチェック項目で確認してください。

## 開発環境

Rust stable、Java 17、Android SDK Platform 35、Build Tools 35.0.0、Android NDK r28c 以降を用意してください。NDK r28 は 16 KB ページに対応する設定を標準で使用します。

```sh
rustup target add aarch64-linux-android
cargo install cargo-apk --version 0.10.0 --locked

export JAVA_HOME=/path/to/jdk-17
export ANDROID_HOME=/path/to/android-sdk
export ANDROID_NDK_HOME="$ANDROID_HOME/ndk/28.2.13676358"
export PATH="$JAVA_HOME/bin:$ANDROID_HOME/platform-tools:$PATH"

cargo apk build --lib
adb install -r target/debug/apk/quiet-launcher.apk
adb shell am start -n dev.quiet.launcher/android.app.NativeActivity
```

開発 APK はデバッグ鍵で署名されます。配布用は `cargo-apk` の release 署名設定と固有の鍵を用意してください。既存環境の `CARGO_TARGET_DIR` によって APK の出力先が変わる場合があります。正確な出力先はビルドログを確認してください。

今回の開発版と同じ鍵で更新 APK を作る場合は、Git 管理外の `.local/android/debug.keystore` を使用します。この鍵は最初の APK と同じものです。

```sh
CARGO_APK_RELEASE_KEYSTORE="$PWD/.local/android/debug.keystore" \
CARGO_APK_RELEASE_KEYSTORE_PASSWORD=android cargo apk build --release --lib
```

デスクトップで画面と検索を確認する場合:

```sh
cargo run --bin quiet-preview
```

プレビューのアプリ一覧はサンプルです。Android 固有の機能やデバイス数値は模擬実装で置き換えず、未取得または利用不可と表示します。Linux では日本語表示のため Noto CJK フォントを入れてください。プレビューの設定は標準では一時ディレクトリ内の `quiet-preview/settings.json` に保存します。`QUIET_SETTINGS` で保存先を変更できます。

## 検証

[全体の要件](docs/requirements.md) と [受け入れテスト計画](docs/acceptance-plan.md) に加え、追加契約から17件のテストを先行作成・凍結し、実装を変更しました。既存17件と合わせた34件の成功は全体要件の充足率を表しません。外部ウィジェットの実表示・再起動復元・削除の3点は最低合格ラインとして確認済みです。過去の実験記録は当時の状態を保持し、[確認記録](docs/widget-acceptance-confirmation.md) を別に残しています。

```sh
cargo test --features android-check
cargo run --no-default-features --example check_contracts
cargo check --all-targets --features android-check
cargo clippy --lib --bins --features android-check -- -D warnings
cargo fmt --package quiet-launcher --check
cargo apk build --lib
```

`android-check` は JNI ブリッジもデスクトップ上でコンパイル対象にします。Android API の実行テストではありません。

実機で確認する項目:

- 初回起動・ホームアプリ指定・ホームボタンからの復帰、縦横回転、画面消灯からの復帰。
- ホームの1画面制約・収容不可のエラー、文字サイズ、追加/削除/並べ替え・非表示・再起動後の保存、アンインストール後の表示。
- 日本語 IME での検索、全角/半角・英大小文字、同名アプリ、部分一致でヒットしないこと。
- Usage Access の許可前後・取り消し後・午前0時またぎ、未取得をゼロと誤表示しないこと。
- 地域設定・変更・解除、機内モード中のエラーと古い取得時刻。
- メディア連携実装後に、複数の実アプリで選択対象への操作、対象がない場合、消失、権限取消を検証。
- テキストを含むウィジェット、画像のみのウィジェット、初回設定が必要なウィジェット、許可キャンセル、削除、プロセス再起動。

## 拡張箇所

- `src/search.rs`: `AppSearch` を実装し、`Launcher::set_search_engine` で差し替える。将来の別名・前方一致・あいまい検索・ランキングをここに追加する。
- `src/platform/mod.rs`: 端末連携の `Platform`。Android 実装は `src/platform/android.rs` に隔離。
- `src/model.rs`: UI に渡すアプリ・端末・外部ウィジェットのデータ。
- `src/weather.rs`: `WeatherProvider`。データ取得先を変更してもホームの表示を維持できる。
- `src/usage.rs`: Android API に依存しない時間集計。
- `src/config.rs`: 設定。壊れたファイルは自動で上書きせず、エラーを表示する。
- `src/ui.rs`: ホーム、一覧、設定画面。

## データと参考資料

設定は端末内に保存します。天気を設定した場合のみ、指定した座標を Open-Meteo に送信します。アプリ一覧やスクリーンタイム、ウィジェットの内容を外部送信する機能はありません。天気データの利用条件・商用利用は Open-Meteo の現行の条件を確認してください。

- [eframe の Android 実装例](https://github.com/emilk/egui/tree/0.31.1/examples/hello_android)
- [android-activity](https://docs.rs/android-activity/0.6.1/android_activity/)
- [cargo-apk](https://github.com/rust-mobile/cargo-apk/blob/main/cargo-apk/README.md)
- [Android AppWidgetHost](https://developer.android.com/develop/ui/views/appwidgets/host)
- [Android UsageEvents](https://developer.android.com/reference/android/app/usage/UsageEvents.Event)
- [Open-Meteo API](https://open-meteo.com/en/docs)
