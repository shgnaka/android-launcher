# Android エミュレーターでの検証

2026-11-14、Android 15（API 35）/ x86_64 / 720×1280 / 320dpi の AOSP エミュレーターで Quiet 0.2.1 を実行した。KVM と SwiftShader を使用した。

## 0.3.0 の追加検証

同じエミュレーターで更新インストールして確認した。

- ホームの常設ヘッダーと「アプリ ↑」がない。時計・日時・外部ウィジェット・お気に入りを表示する。
- ホームからの1回の上スワイプで独立した一覧が開き、枠や「検索」の見出しのない入力欄が現れる。IME の自動表示と入力欄の文字を確認。
- ダイアログを介さず `Camera` を入力すると結果が1件になる。`Cam` に短縮すると一致なし。キーボードを閉じても検索語と結果を保持し、入力欄のタップで IME を再表示できる。
- 戻る操作で IME を閉じ、もう一度戻るとホームに戻る。一覧先頭から下へ引く操作でもホームに戻れる。
- ホームの空白部分の長押し、一覧右上の三点メニューのどちらからも設定画面が開く。
- インストール・描画済みウィンドウ・スワイプ時の誤起動なし・別アプリからの HOME 復帰3回の自動検査が成功。
- テスト17件、Clippy、rustfmt が成功。スクロール中の検索欄の固定、入力変更後の結果位置、10,000件の描画行数制限、タッチ慣性に加え、スワイプによる画面切り替えと、一覧途中の下スクロールでは閉じないことを検査する。
- 配布用 arm64 APK のバージョンは0.3.0、versionCodeは16777984。従来版と同じ署名を検証した。

[ホーム](screenshots/android-home-0.3.0.png)、[画面内の検索と結果](screenshots/android-search-0.3.0.png)、[IME を再表示](screenshots/android-ime-0.3.0.png)、[長押しで開いた設定](screenshots/android-settings-0.3.0.png)。日本語 IME の変換や Android 9/10 の挙動は、この英語 AOSP / Android 15 エミュレーターでは実行検証していない。

## 0.2.2 の追加検証

同じエミュレーターで更新 APK をインストールした。

- 設定画面の「ホームの並び順」から「他のアプリのウィジェット」へ続き、手入力のテキストウィジェットの追加・編集欄がないことを確認。
- Digital clock を選び、許可待ちのホームにも提供元の名前が表示されることを確認。プロセスを終了して再起動しても名前が残る。
- Android のウィジェット許可画面から CREATE を選び、`dumpsys appwidget` の Quiet のホストに DigitalAppWidgetProvider がバインドされることを確認。ホームに「Digital clock」とウィジェットの日付が表示される。
- 更新インストール・スワイプ・別アプリからの HOME 復帰3回の検査を再実行して成功。最後に画面上の「ホーム」で先頭に戻り、連携済みウィジェットの名前・日付が残ることを確認。
- 提供元名を内容の読み取りより前に解決し、読み取りエラーでも名前を保持する。未連携時は選択時に保存した名前を使う。どちらからも名前を取得できない古い未連携 ID だけは「未連携 (ID)」と表示する。
- 既存設定のメモを無視してお気に入り・外部ウィジェット ID を保持する移行と、名前の保存・復元を含むテスト15件、Clippy、rustfmt が成功。

追加時に許可画面が即座に閉じる問題も再現した。通常の `startActivity` では呼び出し元が取得できず、Android 設定の [AllowBindAppWidgetActivity](https://github.com/aosp-mirror/platform_packages_apps_settings/blob/android15-release/src/com/android/settings/AllowBindAppWidgetActivity.java) が終了していた。ウィジェットの許可・設定に `startActivityForResult` を使い、許可画面の表示とバインド成功をエミュレーターで確認した。バインド状態は `getAppWidgetInfo` で観測する。

[名前と外部ウィジェットの内容を表示したホーム](screenshots/android-widget-0.2.2.png)、[手入力ウィジェットを削除した設定画面](screenshots/android-settings-0.2.2.png)。この確認は AOSP の時計ウィジェットが対象で、すべての外部ウィジェットの互換性を保証するものではない。

## 結果

| 操作 | 結果 |
| --- | --- |
| 起動完了待ち | `sys.boot_completed=1` と `pm path android` の両方を確認 |
| APK インストール、起動 | 成功。NativeActivity の可視ウィンドウが `HAS_DRAWN` になることも確認 |
| ホームからのスワイプ | 一覧が表示され、スワイプで別アプリが誤起動しない |
| ネイティブ検索入力 | Android の EditText に `Camera` を入力し、完了操作後に Camera の1件を表示 |
| 完全一致 | `Cam` では一致なし、`Settings` では Settings の1件を表示 |
| お気に入り | Settings を追加。プロセスを終了・再起動してもホームに残る |
| アプリ起動 | ホームの Settings から Android 設定アプリが起動する |
| 標準ホームへの復帰 | Quiet を標準ホームに設定し、別アプリ→HOME を3回繰り返す。毎回 Quiet のウィンドウが可視・描画済みになる |
| 明暗切り替え | 暗い背景に切り替え、ホームとシステムバーの表示を確認。明るい背景へ戻せる |
| Usage Access | エミュレーターで許可した後、画面時間の数値表示に切り替わる。集計精度の検証とは別 |
| ソース検証 | 回帰テスト13件、Clippy、rustfmt が成功 |

画像による確認と、コマンドのアサーションは区別している。`examples/emulator_smoke.rs` はインストール・起動・スワイプ・標準ホーム復帰を検査する。検索とお気に入りは adb で操作し、ネイティブ入力欄の XML とキャプチャを確認した。

エミュレーターは x86_64 版であり、配布 APK は同じソースからビルドした arm64 版。実機の FPS 改善率、全メーカーの端末、他アプリの全ウィジェットとの互換性はこの検証では確認していない。

## 見つかった不具合と修正

1. Android 15 のシステムバーを NativeActivity の content rectangle だけでは避けられないケースがあった。`WindowInsets` を JNI から取得し、システムバー・画面の切り欠きの余白を反映した。明暗に合わせてステータスバーとナビゲーションバーのアイコン色を変更する。
2. スクロール後に検索で一覧が短くなると、結果が固定検索欄の裏に隠れた。検索条件を変えたら結果の先頭へ戻す。端末で再現し、ホスト上の回帰テストでも修正前の失敗・修正後の成功を確認した。
3. アプリアイコンからの起動と HOME タスクへの移行で、Activity が作り直される。winit 0.30.13 のプロセス全体のイベントループ作成制限と、Android Destroy 通知の未処理が、繰り返す終了・Java メインスレッドの停止を引き起こした。Android だけ作成制限を Activity のスレッド単位にし、Destroy 時にはイベントループを終了させた。変更した依存コードは [パッチの説明](../vendor/winit/QUIET-PATCH.md) を参照。

「前面の Activity に Quiet と表示される」だけでは、古い別アプリの画面が残っている場合があった。そのためテストは WindowManager の `Surface: shown=true` と `mDrawState=HAS_DRAWN` も検査する。

## 起動と再実行

この作業環境の SDK は `/tmp/android-launcher-sdk`、AVD は `/tmp/quiet-avd` にある。一時ディレクトリなので、消去された場合は SDK・NDK・システムイメージ・AVD を再作成する。

まず仮想化支援を確認する。

```sh
"$ANDROID_HOME/emulator/emulator" -accel-check
```

今回の初回失敗は `/dev/kvm` のアクセス権限不足だった。ユーザーがローカル端末で次のコマンドを実行した後、KVM 起動を確認した。

```sh
sudo setfacl -m u:shgnaka:rw /dev/kvm
```

この環境に作成した AVD を起動する例:

```sh
ANDROID_HOME=/tmp/android-launcher-sdk \
ANDROID_AVD_HOME=/tmp/quiet-avd \
ANDROID_USER_HOME=/tmp/android-launcher-android-user \
/tmp/android-launcher-sdk/emulator/emulator \
  -avd quiet -no-window -no-audio -no-boot-anim -no-snapshot \
  -accel on -gpu swiftshader -feature -Vulkan -cores 2 -memory 2560
```

エミュレーター用 APK は専用の出力先に作成し、スマートフォン用 arm64 APK を上書きしない。

```sh
rustup target add x86_64-linux-android
CARGO_APK_RELEASE_KEYSTORE="$PWD/.local/android/debug.keystore" \
CARGO_APK_RELEASE_KEYSTORE_PASSWORD=android \
cargo apk build --release --lib --target x86_64-linux-android --target-dir target/emulator

cargo run --no-default-features --example emulator_smoke -- \
  --adb "$ANDROID_HOME/platform-tools/adb" \
  --apk target/emulator/release/apk/quiet-launcher.apk \
  --out target/android-verified
```

テストは `emulator-` で始まる serial だけを受け付け、接続した実機には実行しない。エミュレーターに APK をインストールし、標準ホームを Quiet に変更する。失敗時には元のホームの復元を試みる。出力先には判定 JSON、起動ログ、画面キャプチャ、Activity/WindowManager の情報が残る。

## キャプチャ

- [標準ホームへ復帰した画面](screenshots/android-home-0.2.1.png)
- [検索結果](screenshots/android-search-0.2.1.png)
- [アプリ一覧](screenshots/android-drawer-0.2.1.png)
