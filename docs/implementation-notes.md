# モバイル UI の修正（0.2.0）

## 0.3.0 の画面構成

Fokus の [NavGraph.kt](https://github.com/luantak/FokusLauncher/blob/master/app/src/main/java/com/lu4p/fokuslauncher/ui/navigation/NavGraph.kt)、[AppDrawerScreen.kt](https://github.com/luantak/FokusLauncher/blob/master/app/src/main/java/com/lu4p/fokuslauncher/ui/drawer/AppDrawerScreen.kt)、[SearchBar.kt](https://github.com/luantak/FokusLauncher/blob/master/app/src/main/java/com/lu4p/fokuslauncher/ui/components/SearchBar.kt) を再確認した。以下は Kotlin を取り込まず Rust で実装している。

- ホームとアプリ一覧を独立した画面に変更。上スワイプで一覧を下から開く。移動は220msの減速アニメーション。
- 一覧先頭で下方向に引くとホームへ戻る。途中の一覧を先頭へスクロールする操作では閉じない。
- 「アプリ ↑」、ホーム・設定の常設ナビゲーション、一覧の「アプリ」「検索」見出しを削除。
- 検索は枠・背景・プレースホルダーのない入力欄を上部に固定。入力と同時に検索し、変更時は結果を先頭へ戻す。完全一致と `AppSearch` の差し替え口は維持。
- Android は標準 EditText と PopupWindow を Rust/JNI から作成する。NativeActivity のウィンドウ表面はネイティブ描画が所有するため、同じ View 階層へ追加するだけでは入力文字が表面に描画されない。小さな標準の別表面に検索欄を重ね、ネイティブ IME・カーソル・範囲選択・ハードウェアキーを使えるようにした。入力欄を隠しても query が遅れて復活しないよう、ホームでは検索結果の入力通知を無視する。
- 一覧をスクロールすると IME を閉じ、検索欄のタップで再入力。キーボードの領域を除いて一覧をレイアウトする。API28/29 は可視フレームから、API30以降は IME の WindowInsets から領域を求める。
- 設定はホームの空白部分の長押し、または一覧右上の三点メニューから開く。Android Back で設定・一覧を閉じるためのキー変換は [winit のパッチ](../vendor/winit/QUIET-PATCH.md) に記載。

以下の記録は0.2.0の旧画面構成について。

FokusLauncher のホーム・アプリ一覧と、egui 0.31.1 の入力実装を確認して修正した。Kotlin のコードは取り込まず、画面構成や処理の分離を Rust で実装した。

## 参照した実装

- [Fokus / HomeScreen.kt](https://github.com/luantak/FokusLauncher/blob/master/app/src/main/java/com/lu4p/fokuslauncher/ui/home/HomeScreen.kt): 時計・日時・お気に入りの情報を中心にしたホーム、幅に合わせた配置、システムバーの余白。
- [Fokus / AppDrawerScreen.kt](https://github.com/luantak/FokusLauncher/blob/master/app/src/main/java/com/lu4p/fokuslauncher/ui/drawer/AppDrawerScreen.kt): `LazyColumn` による一覧の遅延描画、一覧と検索 UI の分離、56dp を基準にした行。
- [egui 0.31.1 / ScrollArea](https://docs.rs/egui/0.31.1/egui/containers/scroll_area/struct.ScrollArea.html): ドラッグ・慣性・表示領域の扱い。
- [egui 0.31.1 の入力処理](https://github.com/emilk/egui/blob/0.31.1/crates/egui/src/input_state/mod.rs) と [egui-winit のタッチ変換](https://github.com/emilk/egui/blob/0.31.1/crates/egui-winit/src/lib.rs): タッチ終了と `PointerGone` が同じ入力フレームに入ると、スクロールが速度を読む前に座標履歴が消える。

Fokus の独自機能（カテゴリ、プロファイル、ロック操作等）は今回の移植対象に含めない。検索は既存の完全一致を維持する。

## 修正

- タッチを離すイベントは即時処理し、ポインター消失だけを次のフレームに送る。慣性に必要な移動履歴を残し、その後のホバー表示も消す。依存クレートは変更せず `App::raw_input_hook` を使用する。
- ラベルの文字選択を無効化して、縦ドラッグを一覧のスクロールに渡す。操作中・慣性移動中は次フレームを要求する。
- アプリ一覧は固定高の行として領域だけ確保し、画面に入る行と前後1行だけを描画する。
- 検索はアプリ一覧・入力・検索エンジンが変わったときだけ実行する。毎フレームの Unicode 正規化・全件複製を避ける。
- 文字列からボタン幅を決めず、画面幅内のタッチ領域を先に確保する。長いアプリ名・検索語は1行で省略する。詳細情報は折り返す。
- 一覧の検索欄を上部に固定する。スクロール後は上部にホームへ戻る操作を表示する。
- NativeActivity の content rectangle からシステム領域の余白を得る。固定のステータスバー高を使わない。
- Android のウィジェット処理が保持する Mutex を描画側で待たない。表示用のスナップショットを使い、入力中と慣性移動中は定期更新を延期する。
- 権限・天気の設定案内をホームから除き、設定画面にまとめる。詳細エラーとウィジェットの制限説明は折りたたむ。
- 背景を明示的に塗る。`Frame::NONE` の透明なパネルが eframe の暗いクリア色を露出させ、明るいテーマの文字を読みにくくする問題を修正する。

## 検証

`cargo test --features android-check` に、タッチイベントの開始・移動・終了を入力し、指を離した後も移動すること、ドラッグでアプリを誤起動しないこと、検索が見えることを確認するテストを含めた。

10,000件のアプリで描画行数を制限し、複数フレームの描画で検索が繰り返されないことも確認する。幅 240 / 320 / 360dp と横長画面で、ホーム・一覧・設定のテキストが画面幅内に収まることを検証する。

デスクトップで実際の描画をキャプチャする場合:

```sh
EFRAME_SCREENSHOT_TO=/tmp/quiet-home.png cargo run --features eframe/__screenshot --bin quiet-preview
```

これは Android の実行検証とは別のチェックである。0.2.0 の初回確認では KVM のアクセス権限がなく、Android の起動完了前のインストールに失敗した。その後、権限付与を受けて 0.2.1 を KVM 上の Android 15 で実行した。[実行検証の結果](android-testing.md) を参照。実機での FPS 改善率は測定していない。

最終確認ではテスト12件、Clippy（警告をエラーとして扱う）、rustfmt、arm64 release APK のビルドが成功した。APK の v3 署名を検証し、0.1.0 と同じ証明書・同じパッケージ名で、versionCode が増えていることを確認した。ホームの実描画は [キャプチャ](screenshots/quiet-home-0.2.0.png) でも確認した。
