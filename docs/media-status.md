# メディアの要件・テスト・実装

2026-11-17。正本は [requirements.md 第10節](requirements.md#10-メディア)、受け入れは [A27/A41](acceptance-plan.md)、固定データは [test-fixtures-v1.md 第5節](test-fixtures-v1.md#5-メディアの提供元と正解)。正本と固定テストを変更せず、Rust側を整理した。Androidの実セッション接続の完成とは区別する。

## 確定要件

- 操作可能な公開MediaSessionだけを対象にする。通知本文を利用・保存・送信しない。通知アクセスは設定からユーザーが許可する。
- 対象なし・権限なし・取得失敗ではホーム欄を隠し、設定で理由と権限導線を示す。
- 初回は再生中→バッファ／接続中→一時停止の優先。同順位はパッケージと識別子。選択は識別子で維持し、消失時だけ再選択。
- 左右48dp以上、横が縦の1.5倍以上で1対象ずつ切替。端で停止し、切替で操作を送らない。
- 前曲・再生／一時停止・次曲は描画記号と読み上げ名。操作領域48dp、全体84dp。曲名欠落を捏造しない。
- 操作は選択した対象だけへ送る。消失・権限取消・未対応操作で別対象へ転送しない。送信成功だけで再生状態を変えない。
- 復帰2秒以内、取得可能な変更1秒以内。トークン・曲名・選択を永続化しない。

## 今回の責務分離

| 場所 | 責務 |
| --- | --- |
| `src/media.rs` | 専用ワーカーで取得・操作。状態の選別と優先順、選択維持、端で止まる切替、操作前の対象・対応操作の再確認。状態は実取得結果だけから更新 |
| `src/media_ui.rs` | 最大84dpの表示、文字の省略、ベクトル記号、48dp操作領域、アクセシビリティ名、横スワイプ判定 |
| `src/ui.rs` | 表示設定・前面状態・時計を渡す。ホームと設定詳細へ接続し、対象取得完了で再描画 |
| `src/platform/mod.rs` | OS境界。セッション列挙、識別子を指定した操作、通知アクセス設定を開くAPI |
| `src/platform/android.rs` | 通知アクセス設定のIntent。従来の全体メディアキー送信はエラーにして、誤った再生先への配送を防止 |

UIスレッドでセッションの取得・操作完了を待たない。メディア取得を天気・アプリ再列挙から分離した。通常は前面で1秒ごとに取得し、復帰・表示有効化では直ちに要求する。完了時はUIを起こす。表示オフで表示対象・選択を破棄する。旧`MediaCommand`と`music_active`は固定済み呼出側との互換用に型を残すが、UIの対象選択・再生状態に利用しない。

## 固定テストと範囲

`tests/v1/media.rs` の12件は製品のUIと新コントローラーを通す。外部セッションだけをPlatform境界で制御する。

1. 再生中を初回選択し提供元・実曲名を表示
2. 横スワイプで次の対象へ、コマンド送信なし
3. 列挙順逆転でも選択維持
4. 選択消失時の再選択
5. 0件で欄を消す
6. 終了状態を操作対象から除く（現在の入力はSTOPPEDのみ）
7. 権限取消で既取得データを隠す
8. 文字の操作ボタンを出さず、操作名でフォーカス可能
9. 次曲を選択先だけへ送り、実更新を表示
10. 一時停止を選択先だけへ送り、実状態を反映
11. フォーカス対象消失時、別対象へ送らない
12. 操作を無視する提供元の状態を捏造しない

関連する `screens` の2件は、対象なしで全体操作ボタンを出さないこと、設定の通知アクセス導線を検査する。固定済みAndroid試験はA27（実2セッションから初回選択）、A41（実横切替でコマンドなし）の2件。過去のAPI33/35ではNotificationListener未登録で失敗している。

修正前の12件は2成功10失敗（`target/media-refactor-before.txt`）。接続後の12件は12成功（`target/media-refactor-after.txt`）。最新版の結果は `target/media-refactor-final-media.txt`。全148件は `target/media-refactor-full-host.txt` に112成功36失敗を記録し、メディア12件は成功。残りの失敗をメディアの合格へ含めない。

最終確認: メディア12件成功、旧34件成功（`media-refactor-final-legacy.txt`）。`android-check`付き製品Clippyの警告ゼロ、書式確認成功、正本・固定テスト・ヘルパーのハッシュ一致（`media-refactor-final-clippy.txt`、`media-refactor-format.txt`、`media-refactor-final-freeze.txt`）。テスト本文・期待値・判定器は変更していない。旧試験の過去の失敗ログも残す。

## 未完了と試験の不足

Android側のNotificationListenerService登録、許可範囲のMediaController列挙、トークンと識別子の対応、TransportControlsへの配送は未実装。通知アクセス設定を開けても、それだけで他アプリの再生を取得・制御できるわけではない。Android試験は現ソースで再実行していない。既存試験APKは今回のソース変更で古くなるので、ビルド入力ハッシュだけ更新せず再ビルドする必要がある。

ホスト12件だけでは、前曲・再生の全経路、BUFFERING/CONNECTING/NONE/ERRORの全状態、同順位の初回選択、スワイプの厳密な48dp/比率/端、未対応操作、全画面寸法・倍率、実TalkBack、通知イベントからの期限、無応答提供元を証明できない。ワーカーはUIを止めないが、OS呼出自体の中断・タイムアウトと、停止中要求の世代破棄は追加設計が必要。

Android連携は [MediaSessionManager公式API](https://developer.android.com/reference/android/media/session/MediaSessionManager#getActiveSessions(android.content.ComponentName)) と [通知アクセス設定](https://developer.android.com/reference/android/provider/Settings#ACTION_NOTIFICATION_LISTENER_SETTINGS) を使う次工程。Rust以外の製品ロジックを追加せず、必要なプラットフォームクラス接続を別途実装・検証する。

独立レビューの出力は [agyレビュー](reviews/agy-test-review-2026-11-17.md)。メディアのサービス欠落という指摘は現在も残件。観測器の誤りと製品の不足を混ぜない。
