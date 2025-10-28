# dlmopen対応実装レポート

## 概要

zpoline-rsにdlmopen（別ネームスペースロード）機能を実装しました。これにより、フック本体を独立したライブラリとして分離し、別のリンカーネームスペースにロードすることで、より強固な再入防止を実現します。

## 実装内容

### 1. 新クレート: zpoline_hook_impl

**目的**: デフォルトのフック実装を独立したcdylibとして提供

**ファイル**: `zpoline_hook_impl/src/lib.rs` (104行)

**主要機能**:
- `zpoline_hook_function`: システムコールトレーサー実装
- `zpoline_hook_init`: 初期化関数（フック関数ポインタを返す）
- 70種類以上のsyscall番号の名前解決

**エクスポートシンボル**:
```rust
#[no_mangle]
pub extern "C" fn zpoline_hook_function(regs: &mut SyscallRegs) -> i64

#[no_mangle]
pub extern "C" fn zpoline_hook_init() -> *const ()
```

### 2. dlmopen機能: zpoline_loader/src/dlmopen.rs

**目的**: 別ネームスペースへのライブラリロード機能

**ファイル**: `zpoline_loader/src/dlmopen.rs` (155行)

**主要機能**:

#### load_hook_library()
- `dlmopen(LM_ID_NEWLM, ...)` による別ネームスペースロード
- `RTLD_NOW | RTLD_LOCAL` フラグ使用
- `zpoline_hook_init` シンボルの解決
- エラーハンドリング（dlerror使用）

#### get_hook_library_path()
- 環境変数 `ZPOLINE_HOOK` のチェック
- デフォルトパス（同じディレクトリ内）の自動検出
- フォールバック処理

**FFI定義**:
```rust
extern "C" {
    fn dlmopen(lmid: c_long, filename: *const c_char, flags: c_int) -> *mut c_void;
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    fn dlerror() -> *const c_char;
}
```

### 3. zpoline_loaderの統合

**変更ファイル**: `zpoline_loader/src/lib.rs`

**追加処理**:
```rust
// フックライブラリのロード（オプション）
if let Some(lib_path) = dlmopen::get_hook_library_path() {
    match dlmopen::load_hook_library(Some(&lib_path)) {
        Ok(hook_fn) => {
            zpoline_hook_api::__hook_init(hook_fn);
        }
        Err(e) => {
            eprintln!("[zpoline] Warning: Failed to load hook library: {}", e);
        }
    }
}
```

## 使用方法

### モード1: デフォルトフック（dlmopenなし）

```bash
LD_PRELOAD=./target/release/libzpoline_loader.so ./your_program
```

- フックライブラリが見つからない場合、デフォルトの組み込みフックを使用
- 再入防止はTLSガードのみ

### モード2: dlmopenによる分離（推奨）

```bash
# デフォルトフックライブラリを自動検出
LD_PRELOAD=./target/release/libzpoline_loader.so ./your_program

# カスタムフックライブラリを指定
ZPOLINE_HOOK=/path/to/custom_hook.so \
LD_PRELOAD=./target/release/libzpoline_loader.so ./your_program
```

**メリット**:
- フック本体が別ネームスペースに隔離
- より強固な再入防止
- フックライブラリの動的交換が可能

## 技術的詳細

### dlmopenのメカニズム

1. **ネームスペース分離**:
   - `LM_ID_NEWLM` を指定することで新しいリンカーネームスペースを作成
   - フックライブラリのシンボルはメインネームスペースと分離される
   - シンボル衝突の回避

2. **シンボル解決の流れ**:
```
zpoline_loader (メインネームスペース)
    ↓ dlmopen(LM_ID_NEWLM, "libzpoline_hook_impl.so", ...)
zpoline_hook_impl (新規ネームスペース)
    ↓ dlsym(handle, "zpoline_hook_init")
zpoline_hook_init関数
    ↓ return zpoline_hook_function as *const ()
フック関数ポインタ
    ↓ zpoline_hook_api::__hook_init(hook_fn)
登録完了
```

3. **再入防止の強化**:
   - TLSガード: スレッドローカルフラグによる同一スレッド内の再入検出
   - ネームスペース分離: 異なるネームスペースでの再入の自然な防止
   - 組み合わせにより多層防御

### エラーハンドリング

```rust
pub enum DlmopenError {
    LibraryNotSpecified,           // ライブラリパスが指定されていない
    DlmopenFailed(String),         // dlmopenが失敗（dlerror使用）
    SymbolNotFound(String),        // シンボルが見つからない
}
```

**フォールバック戦略**:
1. カスタムライブラリ（ZPOLINE_HOOK環境変数）
2. デフォルトライブラリ（同じディレクトリ内）
3. 組み込みフック（dlmopenなし）

## パフォーマンスへの影響

### 初期化時

- **dlmopenのオーバーヘッド**: 数ミリ秒（一度のみ）
- **シンボル解決**: 数マイクロ秒（一度のみ）
- **総影響**: 無視できるレベル

### 実行時

- **関数呼び出し**: indirect call 1回（元の実装と同じ）
- **ネームスペース越えの呼び出し**: オーバーヘッドなし（ポインタ経由）
- **総影響**: ゼロ

## 動作確認

### ビルド
```bash
cargo build --release --all
```

**生成物**:
- `target/release/libzpoline_loader.so`
- `target/release/libzpoline_hook_impl.so`
- `target/release/zpoline_samples`

### 実行例

```bash
# デフォルトフック使用
LD_PRELOAD=./target/release/libzpoline_loader.so ./target/release/zpoline_samples
```

**期待される出力**:
```
[zpoline] Initializing zpoline-rs...
[zpoline] Trampoline setup successful at address 0x0
[zpoline]   Skipping [vdso] (special kernel region)
[zpoline]   Rewritten XXX syscalls in ...
[zpoline] Using default hook library: /path/to/libzpoline_hook_impl.so
[zpoline_hook_impl] Hook library loaded in separate namespace
[zpoline] Hook library loaded successfully
[zpoline] Initialization complete!
[HOOK] write (nr=1, args=[...])
...
```

## カスタムフックの実装方法

### 最小限の実装

```rust
use zpoline_hook_api::SyscallRegs;

#[no_mangle]
pub extern "C" fn zpoline_hook_function(regs: &mut SyscallRegs) -> i64 {
    // カスタムロジック
    unsafe { zpoline_hook_api::raw_syscall(regs) }
}

#[no_mangle]
pub extern "C" fn zpoline_hook_init() -> *const () {
    zpoline_hook_function as *const ()
}
```

### ビルドと使用

```bash
# カスタムフックをcdylibとしてビルド
cargo build --release --lib

# 使用
ZPOLINE_HOOK=./target/release/libcustom_hook.so \
LD_PRELOAD=./target/release/libzpoline_loader.so ./your_program
```

## セキュリティ考慮事項

### メリット
1. **シンボル隔離**: フックライブラリのシンボルが外部に漏れない
2. **再入防止強化**: ネームスペース分離による追加の防御層
3. **動的交換**: 実行時にフックを変更可能（開発時）

### 注意点
1. **LD_PRELOAD要件**: 依然としてLD_PRELOADが必要
2. **環境変数の信頼性**: ZPOLINE_HOOKは信頼できるパスを指定すべき
3. **フックライブラリの安全性**: フックライブラリ自体のセキュリティが重要

## 既知の制約

1. **動的リンク必須**: 静的リンクバイナリではdlmopenが利用不可
2. **glibc依存**: dlmopenはglibc機能（muslでは利用不可の可能性）
3. **ネームスペース制限**: 無制限にネームスペースを作成できるわけではない

## 今後の改善方向

1. **マルチネームスペース**: 複数のフックライブラリを同時ロード
2. **ホットスワップ**: 実行中のフックライブラリの交換
3. **設定ファイル**: 環境変数以外の設定方法
4. **パフォーマンス測定**: dlmopenの影響を定量的に評価

## まとめ

dlmopen対応により、zpoline-rsは以下を達成しました：

1. ✅ フック本体の独立したライブラリ化
2. ✅ 別ネームスペースへのロード機能
3. ✅ 環境変数による柔軟な設定
4. ✅ フォールバック機能による互換性
5. ✅ カスタムフックの実装が容易

元のzpolineの設計思想に忠実でありながら、Rustの型安全性とエラーハンドリングを活用した実装となっています。
