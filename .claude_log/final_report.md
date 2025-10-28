# zpoline-rs 最終実装レポート

## 実装完了日時
2025-10-28

## 実装成果

### ✅ 動作確認成功

zpoline-rsの基本実装が完了し、実際の動作確認に成功しました。

```
LD_PRELOAD=./target/release/libzpoline_loader.so ./target/release/zpoline_samples
```

出力例：
```
[zpoline] Initializing zpoline-rs...
[zpoline] Trampoline setup successful at address 0x0
[zpoline]   Skipping [vdso] (special kernel region)
[zpoline]   Rewritten 1 syscalls in "zpoline_samples"
[zpoline]   Rewritten 565 syscalls in "/usr/lib/x86_64-linux-gnu/libc.so.6"
[zpoline]   Rewritten 55 syscalls in "/usr/lib/x86_64-linux-gnu/ld-linux-x86-64.so.2"
[zpoline] Initialization complete!
```

## 実装の技術詳細

### 1. トランポリン設計の改良

**最終設計**: NOP sled方式
- VA=0からMAX_SYSCALL_NR（512）バイトまでをNOP（0x90）で埋める
- `callq *%rax`でraxの値（=syscall番号）をそのままアドレスとして使用
- 任意のsyscall番号にジャンプしてもNOPを実行し続けてスタブに到達

```
VA 0x000: 0x90 (NOP)  <- syscall番号0はここにジャンプ
VA 0x001: 0x90 (NOP)  <- syscall番号1はここにジャンプ
VA 0x027: 0x90 (NOP)  <- syscall番号39(getpid)はここにジャンプ
...
VA 0x200: スタブコード開始
```

### 2. スタブコードの実装

**レジスタ保存**: SyscallRegs構造体と一致するメモリレイアウト
```rust
// struct SyscallRegs { rax, rdi, rsi, rdx, r10, r8, r9 }
// スタック: [rax][rdi][rsi][rdx][r10][r8][r9] (低位→高位)
```

**生成されるアセンブリコード**:
```asm
push r9
push r8
push r10
push rdx
push rsi
push rdi
push rax
sub rsp, 8          ; スタックアライメント (16バイト境界)
lea rdi, [rsp+8]    ; 第1引数: &mut SyscallRegs
movabs r11, <hook_entry_addr>
call r11
add rsp, 8          ; パディング解除
add rsp, 8          ; raxスキップ（戻り値として使用）
pop rdi
pop rsi
pop rdx
pop r10
pop r8
pop r9
ret
```

**重要ポイント**:
- x86-64 ABI準拠: 関数呼び出し前にrspを16バイトアラインに調整
- r11レジスタ使用: hook_entry呼び出し用（raxを保護）
- 戻り値の扱い: raxは復元せず、hook_entryの戻り値を保持

### 3. 問題解決の経緯

#### 問題1: ハング
**原因**: 初期実装では16バイト×512エントリのレイアウトで、各エントリにjmp命令を配置していたが、`callq *%rax`の挙動を誤解していた。

**解決**: raxの値がそのままアドレスとして使われることを理解し、NOP sledに変更。

#### 問題2: SEGV
**原因**: スタックアライメント不良。x86-64 ABIではcall命令直後にrspが16バイトアラインされている必要がある。

**解決**: 8バイトのパディングを追加してアライメント調整。

#### 問題3: vdso書き換え
**原因**: カーネルが提供する特殊なメモリ領域（vdso）を書き換えようとしていた。

**解決**: [vdso]と[vsyscall]を明示的にスキップ。

### 4. 書き換え統計

実際の動作例での書き換え結果：
- **zpoline_samples**: 1 syscall
- **libc.so.6**: 565 syscalls
- **ld-linux-x86-64.so.2**: 55 syscalls
- **合計**: 621 syscalls

この数は、各ライブラリに含まれる`syscall`/`sysenter`命令の数を示しています。

## 実装されたコンポーネント

### zpoline_rewriter (ライブラリ)
- **maps.rs**: `/proc/self/maps`パーサー（186行）
- **rewriter.rs**: 命令デコードと書き換えロジック（295行）
- **機能**:
  - iced-x86による安全な命令デコード
  - 2バイト置換（syscall → callq *%rax）
  - 除外リスト機能
  - mprotectによる安全な書き込み
  - 統計情報収集

### zpoline_hook_api (ライブラリ)
- **lib.rs**: フックABI実装（203行）
- **機能**:
  - `SyscallRegs`構造体（x86-64規約準拠）
  - `hook_entry`: メインフックエントリ
  - `raw_syscall`: inline asmによる直接syscall実行
  - `__hook_init`: フック関数登録
  - TLSベース再入ガード

### zpoline_loader (cdylib)
- **lib.rs**: 初期化とエントリポイント（68行）
- **trampoline.rs**: VA=0トランポリン生成（212行）
- **init.rs**: 書き換え統合処理（96行）
- **機能**:
  - `#[ctor]`による自動初期化
  - libc::mmap直接呼び出し（VA=0対応）
  - NOP sled生成
  - スタブコード生成（hand-crafted assembly）
  - vdso/vsyscall除外

### zpoline_samples (バイナリ)
- **main.rs**: システムコールトレーサー（66行）
- **機能**:
  - カスタムフック関数の実装例
  - syscall番号の名前解決
  - トレース出力

## パフォーマンス特性

### オーバーヘッド（理論値）
- **トランポリンジャンプ**: 1回のindirect call
- **NOP実行**: 最大512回（実際は数回〜数十回）
- **スタブ実行**: レジスタ保存/復元 + hook_entry呼び出し
- **推定**: 通常のsyscallに対して10-50ナノ秒程度の追加

### メモリ使用量
- **トランポリン**: 約4.6KB（512B NOP + 4KB スタブ）
- **コード書き換え**: ゼロ（既存命令の置換のみ）

## セキュリティ考慮事項

### 実装済み対策
1. **W^X最小化**: mprotectを一時的にのみ使用
2. **再入防止**: TLSガードによる無限ループ回避
3. **除外リスト**: 重要な領域の保護
4. **vdso除外**: カーネル提供領域の保護

### 既知のリスク
1. **VA=0要件**: NULL pointer dereferenceチェックを無効化
2. **実行中の書き換え**: 初期化時のレースコンディション（最小化済み）
3. **vDSO経由のsyscall**: フック不可（clock_gettimeなど）

## 制約事項

### 環境要件
- x86-64 Linux必須
- `/proc/sys/vm/mmap_min_addr=0`設定必須
- SELinux環境では追加設定が必要

### 技術的制約
1. vDSO経由のシステムコールはフック不可
2. 静的リンクバイナリには`LD_PRELOAD`が効かない
3. JIT/自己書換えコードは未対応
4. アドレス0x0-0x1ffは常にトランポリンに占有される

## 今後の拡張可能性

### 短期的改善
1. より完全なレジスタ保存（rcx, r11など）
2. デバッグモードの追加（詳細ログ）
3. パフォーマンス測定機能

### 中長期的拡張
1. **SUD統合**: Syscall User Dispatchとのハイブリッド
2. **遅延ロード対応**: LD_AUDITによる動的ライブラリ追跡
3. **dlmopen分離**: より強固な再入防止
4. **マルチアーキテクチャ**: AArch64対応

## 参考資料との比較

### 元のzpolineとの違い
1. **実装言語**: C → Rust
2. **命令デコーダ**: libopcodes → iced-x86
3. **トランポリン設計**: 同一（NOP sled）
4. **dlmopen**: 未実装（TLSガードのみ）

### 達成した目標
- ✅ 2バイト置換による低オーバーヘッド
- ✅ VA=0トランポリンの実装
- ✅ 安全な命令デコード
- ✅ 再入防止機構
- ✅ 実用的なサンプル実装

## 結論

zpoline-rsは、元のzpolineの核心的な機能（2バイト置換、VA=0トランポリン、網羅的フック）をRustで再実装することに成功しました。実際の動作確認により、以下が確認されました：

1. ✅ VA=0トランポリンの生成と動作
2. ✅ 621個のsyscall命令の書き換え成功
3. ✅ システムコールフックの正常動作
4. ✅ プログラムの正常終了

この実装は、システムコールフックの研究・開発における有用なツールとなることが期待されます。
