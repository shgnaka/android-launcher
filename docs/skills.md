# Android / Rust UI 開発に使えるスキルの調査

2026-11-14 に配布元の SKILL.md を確認した。確認用に `/tmp/quiet-reviewed-skills` に取得し、常用スキルとしてのグローバルインストールはしていない。

| スキル | 配布元 | このプロジェクトでの適用 |
| --- | --- | --- |
| mobile-android-design | [wshobson/agents](https://github.com/wshobson/agents/tree/main/plugins/ui-design/skills/mobile-android-design) | 今回使用。48dp 以上のタッチ領域、端末幅に合わせた配置、長い一覧の遅延描画、検索欄の固定という方針を Rust / egui に適用した。Compose のサンプルはそのまま導入していない。 |
| edge-to-edge | [android/skills](https://github.com/android/skills/tree/main/system/edge-to-edge) | Google 提供。システムバー・IME・画面端の扱いに関する候補。ただし SKILL.md が Compose を必須条件としているため、Rust の実装手順として直接は使わない。NativeActivity の余白取得と実機テストで確認する。 |
| anr-audit-and-fix | [android/skills](https://github.com/android/skills/tree/main/performance/anr-audit-and-fix) | Google 提供。ANR のスタックやメインスレッドのブロックを調査するときの候補。一般的なスクロールのコマ落ち・FPS 調査向けではないと明記されているため、その用途では適用しない。 |
| testing-setup | [android/skills](https://github.com/android/skills/tree/main/testing/testing-setup) | Google 提供。画面サイズ・テーマ・操作の検証計画に参考になる。ただしセットアップ手順は Gradle / Kotlin / Compose / Views を前提にしている。Rust のプロジェクトへ Hilt や JUnit を追加する目的では使わない。 |

Rust + egui + NativeActivity では、スキルのサンプルコードよりも使用中のクレートのソース確認が必要なことがある。今回の慣性停止は、その版の入力変換と座標履歴を追い、実際のタッチ終了イベントを使う回帰テストで確認した。

スキル探索・確認には [OpenAI skill-installer](https://github.com/openai/skills/tree/main/skills/.system/skill-installer) を使った。公式 curated 一覧では、この Rust 製 Android ランチャーの UI を直接扱う専用スキルは選べなかったため、Android の配布元とモバイル UI の配布元も調べた。
