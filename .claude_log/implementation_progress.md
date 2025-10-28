# zpoline-rs 実装進捗報告

実装日時: 2025-10-28

## 実装完了項目

### ✅ フェーズ1: プロジェクト構造とクレート設定
- Cargoワークスペースの設定
- 4つのクレートの作成と設定
  - `zpoline_loader` (cdylib): LD_PRELOADローダー
  - `zpoline_hook_api` (lib): フックAPI
  - `zpoline_rewriter` (lib): 命令書き換え
  - `zpoline_samples` (bin): サンプルプログラム

### ✅ フェーズ2: zpoline_rewriter - 命令デコーダと書換え基盤
実装ファイル:
- `zpoline_rewriter/src/lib.rs` - モジュール定義とエクスポート
- `zpoline_rewriter/src/maps.rs` - `/proc/self/maps`パーサー
- `zpoline_rewriter/src/rewriter.rs` - 命令書き換えロジック

主な機能:
- `/proc/self/maps`のパースと実行可能領域の抽出
- iced-x86による命令デコード
- syscall/sysenter命令の検出
- 2バイト置換（0x0f 0x05/0x0f 0x34 → 0xff 0xd0）
- 除外リスト機能（パスベース、アドレス範囲ベース）
- mprotectによる一時的な書き込み許可
- 書き換え統計情報の収集

### ✅ フェーズ3: zpoline_hook_api - フックABIとraw syscall
実装ファイル:
- `zpoline_hook_api/src/lib.rs` - フックAPI実装

主な機能:
- `SyscallRegs` 構造体（x86-64システムコール規約に準拠）
- `HookFn` 型定義
- `hook_entry` - メインのフックエントリポイント
- `raw_syscall` - 書き換え対象外のsyscall実装（inline asmを使用）
- `__hook_init` - フック関数の登録
- TLSベースの再入ガード
- AtomicPtrによるスレッドセーフなフック関数管理

### ✅ フェーズ4: zpoline_loader - ローダーとトランポリン生成
実装ファイル:
- `zpoline_loader/src/lib.rs` - 初期化処理
- `zpoline_loader/src/trampoline.rs` - VA=0トランポリン生成
- `zpoline_loader/src/init.rs` - 書き換え統合処理

主な機能:
- `#[ctor]`による自動初期化
- VA=0へのトランポリンマッピング（libc::mmap使用）
- NOPスレッド生成（512個のsyscall番号対応）
- フックスタブコード生成（アセンブリ）
- 書き換え設定の構築
- 除外リスト管理（zpoline自身、raw_syscall領域、VA=0領域）
- 環境変数（ZPOLINE_EXCLUDE）対応

### ✅ フェーズ5: 基本動作の統合とテスト
実装ファイル:
- `zpoline_samples/src/main.rs` - システムコールトレーサーサンプル

主な機能:
- カスタムフック関数の実装例
- 主要なsyscall番号の名前解決
- トレース出力機能
- 複数のシステムコール実行デモ（write, getpid, open/read/close, stat）

ドキュメント:
- `USAGE.md` - 詳細な使用方法ドキュメント
  - 必須要件とシステム設定
  - ビルド方法
  - 実行方法
  - カスタムフック実装例
  - トラブルシューティング
  - 制約事項とセキュリティ注意事項

## 技術的な実装詳細

### 2バイト置換の仕組み
```
syscall (0x0f 0x05)     →  callq *%rax (0xff 0xd0)
sysenter (0x0f 0x34)    →  callq *%rax (0xff 0xd0)
```

- RAXにはsyscall番号が格納されている
- `callq *%rax`でVA=0 + syscall番号*16の位置にジャンプ
- そこにはNOPスレッドがあり、最終的にフックスタブへ到達

### トランポリン構造
```
VA=0x0000: NOP sled (syscall番号0用)
VA=0x0010: NOP sled (syscall番号1用)
VA=0x0020: NOP sled (syscall番号2用)
...
VA=0x2000: フックスタブ（hook_entryを呼び出す）
```

### レジスタ保存/復元
フックスタブでは以下のレジスタを保存:
- rax (syscall番号)
- rdi, rsi, rdx, r10, r8, r9 (syscall引数)

### 再入防止
- TLS（Thread Local Storage）フラグによる再入検出
- 再入時は`raw_syscall`で直接システムコールを実行

## ビルド結果

```
✓ zpoline_rewriter - コンパイル成功
✓ zpoline_hook_api - コンパイル成功
✓ zpoline_loader - コンパイル成功（cdylib）
✓ zpoline_samples - コンパイル成功
```

## 既知の制約・今後の課題

### 現在の制約
1. **簡易的なトランポリンスタブ**
   - 現在のスタブは最小限のレジスタ保存のみ
   - より完全なレジスタ保存/復元が必要
   - calling conventionの厳密な遵守

2. **テストの不足**
   - 実際のVA=0での動作テスト未実施
   - 複雑なプログラムでの動作確認が必要
   - ユニットテストの拡充

3. **エラーハンドリング**
   - 一部のエラーケースで即座にexitしている
   - より細かいエラー報告と回復処理

### 今後の拡張機能

1. **SUD (Syscall User Dispatch) 統合**
   - SIGSYSによる捕捉
   - 初回はSUD、以降は書き換えというハイブリッド

2. **遅延ロード対応**
   - LD_AUDIT機構の利用
   - dlopenされたライブラリの動的書き換え

3. **dlmopen による分離**
   - フック本体を別ネームスペースにロード
   - より強固な再入防止

4. **パフォーマンス最適化**
   - トランポリンコードの最適化
   - キャッシュの有効活用

5. **vDSO対応**
   - vDSO経由のシステムコールへの対応
   - LD_PRELOADによるバイパス

## セキュリティ考慮事項

### 実装済み
- W^X最小化（mprotectの短時間使用）
- 再入ガード（TLS）
- 除外リスト機能

### 要検討
- より厳密な権限チェック
- 監査ログの出力
- セキュアモードでの動作

## パフォーマンス特性

理論的な特性（実測は今後の課題）:
- **オーバーヘッド**: 1回のシステムコールあたり数十ナノ秒程度の追加
- **置換コスト**: 初期化時のみ（実行時はゼロ）
- **メモリ使用量**: トランポリン用に約12KB + 各ライブラリのコードサイズ

## 動作確認結果（2025-10-28 更新）

### ✅ 実機での動作確認成功

VA=0設定環境での実行に成功しました。

**実行コマンド**:
```bash
sudo sysctl -w vm.mmap_min_addr=0
LD_PRELOAD=./target/release/libzpoline_loader.so ./target/release/zpoline_samples
```

**実行結果**:
```
[zpoline] Initializing zpoline-rs...
[zpoline] Trampoline setup successful at address 0x0
[zpoline]   Skipping [vdso] (special kernel region)
[zpoline]   Rewritten 1 syscalls in zpoline_samples
[zpoline]   Rewritten 565 syscalls in libc.so.6
[zpoline]   Rewritten 55 syscalls in ld-linux-x86-64.so.2
[zpoline] Initialization complete!
```

**書き換え統計**:
- 合計621個のsyscall命令を書き換え
- vdso領域は適切にスキップ
- システムコールトレーサーが正常動作

### 実装中に解決した技術的課題

1. **トランポリン設計の改良**
   - 問題: `callq *%rax`の挙動を誤解（16バイト間隔のエントリ配置）
   - 解決: NOP sled方式に変更（raxの値=アドレス）

2. **スタックアライメント**
   - 問題: SEGV発生
   - 解決: x86-64 ABI準拠の16バイトアライメント実装

3. **vdso書き換え**
   - 問題: カーネル領域を書き換えてSEGV
   - 解決: [vdso]と[vsyscall]を明示的に除外

## まとめ

zpoline-rsの基本実装が完了し、実際の動作確認に成功しました：

1. ✅ Rust製のzpoline再実装の完成
2. ✅ iced-x86による安全な命令デコード
3. ✅ VA=0トランポリンの生成と動作確認
4. ✅ 621個のsyscall命令の書き換え成功
5. ✅ システムコールフックの正常動作
6. ✅ サンプルプログラムとドキュメント完備

詳細は `.claude_log/final_report.md` を参照してください。

### ✅ フェーズ6: dlmopen対応（2025-10-28 完了）

実装ファイル:
- `zpoline_hook_impl/src/lib.rs` - デフォルトフック実装（cdylib）
- `zpoline_loader/src/dlmopen.rs` - dlmopen機能実装
- `zpoline_loader/src/lib.rs` - dlmopen統合

主な機能:
- デフォルトフックライブラリ（zpoline_hook_impl）の独立化
- dlmopenによる別ネームスペースロード（LM_ID_NEWLM使用）
- ZPOLINE_HOOK環境変数によるカスタムフックライブラリ指定
- 自動フォールバック機能（ライブラリ未検出時は組み込みフック使用）
- 70種類以上のsyscall番号の名前解決

**動作確認結果**:
- ✅ デフォルトフックライブラリの自動検出が成功
- ✅ ZPOLINE_HOOK環境変数によるカスタムパス指定が動作
- ✅ 別ネームスペースへのロードを確認
- ✅ ライブラリ未検出時のフォールバックが正常動作
- ✅ 621個のsyscallフックが正常動作

詳細は `.claude_log/dlmopen_implementation.md` および `.claude_log/dlmopen_verification.md` を参照してください。

### 今後の拡張可能性
- SUD (Syscall User Dispatch) 統合
- 遅延ロード対応（LD_AUDIT）
- パフォーマンス測定とベンチマーク
- マルチネームスペース対応（複数フックライブラリの同時ロード）
