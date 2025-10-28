# zpoline-rs 実装計画

## 概要
本プロジェクトは、zpolineのRust再実装です。zpolineは、システムコール（`syscall`/`sysenter`命令）を2バイト命令 `callq *%rax` に置換し、VA=0に配置したトランポリンに制御を渡すことで、低オーバーヘッドかつ網羅的にシステムコールをフックする仕組みです。

## 実装段階

### フェーズ1: プロジェクト構造とクレート設定
- [x] 基本的なワークスペース構造の作成
- [ ] 3つのクレートの作成
  - `zpoline_loader` (cdylib): LD_PRELOADで注入されるローダー
  - `zpoline_hook_api` (lib): フックABIの定義
  - `zpoline_rewriter` (lib): 命令デコードと書換え
  - `zpoline_samples` (bin): サンプルプログラム

### フェーズ2: zpoline_rewriter - 命令デコーダと書換え基盤
- [ ] 依存クレートの追加（iced-x86, nix, libc）
- [ ] `/proc/self/maps` のパーサー実装
- [ ] x86-64命令デコーダの統合（iced-x86使用）
- [ ] `syscall`/`sysenter` 命令検出ロジック
- [ ] 2バイト置換（0x0f 0x05 → 0xff 0xd0）の実装
- [ ] 除外リスト機能（特定ページの書換えをスキップ）

### フェーズ3: zpoline_hook_api - フックABIとraw syscall
- [ ] `extern "C"` FFI ABIの定義
- [ ] `hook_entry` 関数のシグネチャとレジスタ状態の構造体
- [ ] `raw_syscall_bypass` の実装（書換え対象外のsyscall命令）
- [ ] TLSベースの再入ガード機構
- [ ] `__hook_init` の実装（フック関数ポインタ受け渡し）

### フェーズ4: zpoline_loader - ローダーとトランポリン生成
- [ ] cdylib設定とLD_PRELOAD対応
- [ ] `#[ctor]` を使った初期化処理（ctor crate使用）
- [ ] VA=0トランポリンの生成
  - `mmap(addr=0, PROT_EXEC|PROT_READ, MAP_FIXED|ANON|PRIVATE)`
  - NOPスレッド（0～最大syscall番号）の配置
  - 末尾のjmp stub実装
- [ ] コード書換え統合（rewriterクレートの呼び出し）
- [ ] 除外リストの管理（自ライブラリ、raw_syscallページ）

### フェーズ5: 基本動作の統合とテスト
- [ ] 最小サンプルプログラムの作成（write(1, "test", 4)など）
- [ ] LD_PRELOADでの動作確認
- [ ] システムコールフックの動作確認
- [ ] レジスタ引数の正確な復元確認

### フェーズ6: 再入対策と安全性
- [ ] TLSガードの動作確認と改善
- [ ] `dlmopen` による別ネームスペースロード（任意機能）
- [ ] エラーハンドリングの実装
  - VA=0確保失敗時の処理
  - 書換え失敗時のフォールバック
- [ ] W^X最小化（mprotectの適切な使用）

### フェーズ7: 追加機能（任意）
- [ ] 遅延ロード対応（LD_AUDIT）
- [ ] SUD（Syscall User Dispatch）統合
- [ ] vDSOバイパス機構

### フェーズ8: 評価とドキュメント
- [ ] 機能テストスイート
- [ ] 網羅性テスト（フック命中率測定）
- [ ] 性能ベンチマーク
- [ ] 使用方法ドキュメントの作成

## 技術スタック

### 依存クレート
- `libc`: システムコールとCライブラリバインディング
- `nix`: Unix系API（mmap, mprotect等）
- `iced-x86`: x86-64命令デコーダ
- `object` or `goblin`: ELFパーサー
- `ctor`: コンストラクタ属性（#[ctor]）
- `lazy_static` or `once_cell`: グローバル初期化

### 実装上の重要ポイント

1. **2バイト置換の安全性**
   - `syscall` (0x0f 0x05) → `callq *%rax` (0xff 0xd0)
   - 同じ2バイトなので命令境界を壊さない

2. **VA=0の要件**
   - 事前に `/proc/sys/vm/mmap_min_addr` を0に設定が必要
   - SELinuxの場合は追加のポリシー調整が必要

3. **再入防止**
   - TLSフラグで再入を検出
   - `dlmopen` で別ネームスペースにフック本体をロード
   - `raw_syscall_bypass` で元のシステムコールを実行

4. **除外領域**
   - 自ライブラリ（zpoline_loader自体）
   - raw_syscall実装ページ
   - ユーザー指定のDSO

## 進捗管理
各フェーズの完了後にこのドキュメントを更新し、実装中の問題点や設計変更を記録します。

## リスク管理

### 既知の制約
- VA=0が確保できない環境では動作不可
- vDSOはフック対象外
- JIT/自己書換えコードは未対応

### 対策
- VA=0確保失敗時のエラー表示と中止
- vDSO関数のLD_PRELOADバイパス（将来的に）
- SUDモードへのフォールバック（将来的に）
