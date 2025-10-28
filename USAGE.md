# zpoline-rs 使用方法

## 概要

zpoline-rsは、システムコール（`syscall`/`sysenter`命令）を2バイト命令 `callq *%rax` に置換し、VA=0に配置したトランポリンを通じて、低オーバーヘッドかつ網羅的にシステムコールをフックする仕組みです。

## 必須要件

### システム要件
- **x86-64 Linux** システム
- Rust 1.70以降
- `sudo`権限（初回設定のみ）

### 重要な前提条件

zpoline-rsを使用するには、**VA=0（仮想アドレス0）へのマッピングを許可する必要があります**。

#### 1. mmap_min_addrの設定

```bash
# 現在の設定を確認
cat /proc/sys/vm/mmap_min_addr

# 0に設定（一時的）
sudo sysctl -w vm.mmap_min_addr=0

# 永続的に設定する場合
echo "vm.mmap_min_addr = 0" | sudo tee -a /etc/sysctl.conf
sudo sysctl -p
```

#### 2. SELinuxの設定（SELinuxを使用している場合）

SELinuxが有効な場合、追加の設定が必要です：

```bash
# SELinuxのステータス確認
getenforce

# 一時的に無効化（テスト用）
sudo setenforce 0

# または、適切なポリシーを設定
# 詳細は元のzpolineのドキュメントを参照
# https://github.com/yasukata/zpoline
```

**注意**: 本番環境では、セキュリティへの影響を十分に理解した上で設定してください。

## ビルド方法

```bash
# ワークスペース全体をビルド
cargo build --release

# 特定のクレートのみビルド
cargo build --release -p zpoline_loader
cargo build --release -p zpoline_samples
```

ビルド成果物：
- `target/release/libzpoline_loader.so` - LD_PRELOADで使用するローダーライブラリ
- `target/release/zpoline_samples` - サンプルプログラム

## 使用方法

### 基本的な使用

zpoline-rsは `LD_PRELOAD` 環境変数を使用してプログラムに注入します：

```bash
# デフォルトフックライブラリを自動検出（dlmopen使用）
LD_PRELOAD=./target/release/libzpoline_loader.so ./target/release/zpoline_samples

# フックライブラリなし（組み込みフック使用）
# libzpoline_hook_impl.soが見つからない場合、自動的にこのモードになります
```

### dlmopen（別ネームスペース）対応

zpoline-rsは、フック本体を別のリンカーネームスペースにロードすることで、より強固な再入防止を実現します。

**デフォルトフックライブラリの使用**:
```bash
# libzpoline_hook_impl.soが自動的に検出されてロードされます
LD_PRELOAD=./target/release/libzpoline_loader.so ./your_program
```

**カスタムフックライブラリの指定**:
```bash
# ZPOLINE_HOOK環境変数でカスタムフックを指定
ZPOLINE_HOOK=/path/to/custom_hook.so \
LD_PRELOAD=./target/release/libzpoline_loader.so ./your_program
```

### サンプルプログラムの実行

```bash
# サンプルをビルド
cargo build --release -p zpoline_samples

# VA=0の設定を確認
cat /proc/sys/vm/mmap_min_addr  # 0であることを確認

# サンプルを実行
LD_PRELOAD=./target/release/libzpoline_loader.so ./target/release/zpoline_samples
```

サンプルプログラムは、システムコールをトレースして標準エラー出力に表示します。

### 既存のプログラムへの適用

任意のプログラムに対してシステムコールフックを適用できます：

```bash
# lsコマンドのシステムコールをフック
LD_PRELOAD=./target/release/libzpoline_loader.so ls -la

# 自作プログラムに適用
LD_PRELOAD=./target/release/libzpoline_loader.so ./my_program
```

### 環境変数

#### ZPOLINE_HOOK

カスタムフックライブラリのパスを指定します：

```bash
export ZPOLINE_HOOK="/path/to/custom_hook.so"
LD_PRELOAD=./target/release/libzpoline_loader.so ./my_program
```

**動作**:
1. `ZPOLINE_HOOK`が設定されている場合、そのパスのライブラリをdlmopenでロード
2. 未設定の場合、`libzpoline_hook_impl.so`を同じディレクトリから自動検出
3. どちらも見つからない場合、組み込みフックを使用（dlmopenなし）

#### ZPOLINE_EXCLUDE

書き換えから除外するライブラリパスを指定できます：

```bash
export ZPOLINE_EXCLUDE="/lib/libfoo.so:/usr/lib/libbar.so"
LD_PRELOAD=./target/release/libzpoline_loader.so ./my_program
```

## カスタムフックの実装

### 方法1: アプリケーション内でのフック登録

```rust
use zpoline_hook_api::{SyscallRegs, __hook_init};

// カスタムフック関数
extern "C" fn my_custom_hook(regs: &mut SyscallRegs) -> i64 {
    // regs.rax: システムコール番号
    // regs.rdi, rsi, rdx, r10, r8, r9: 引数

    // 特定のシステムコールを書き換え
    if regs.rax == 1 {  // write syscall
        eprintln!("[HOOK] write syscall intercepted");
    }

    // 元のシステムコールを実行
    unsafe { zpoline_hook_api::raw_syscall(regs) }
}

fn main() {
    // フック関数を登録
    __hook_init(my_custom_hook);

    // プログラムのメイン処理
    println!("Hello, world!");
}
```

### 方法2: dlmopenでロードされる独立ライブラリ（推奨）

**Cargo.toml**:
```toml
[package]
name = "my_custom_hook"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
zpoline_hook_api = { path = "/path/to/zpoline_hook_api" }
```

**src/lib.rs**:
```rust
use zpoline_hook_api::SyscallRegs;

#[no_mangle]
pub extern "C" fn zpoline_hook_function(regs: &mut SyscallRegs) -> i64 {
    // カスタムロジックを実装
    if regs.rax == 1 {  // write syscall
        eprintln!("[CUSTOM] write intercepted");
    }

    unsafe { zpoline_hook_api::raw_syscall(regs) }
}

#[no_mangle]
pub extern "C" fn zpoline_hook_init() -> *const () {
    eprintln!("[CUSTOM] Custom hook library loaded");
    zpoline_hook_function as *const ()
}
```

**ビルドと使用**:
```bash
# ビルド
cargo build --release

# 使用
ZPOLINE_HOOK=./target/release/libmy_custom_hook.so \
LD_PRELOAD=/path/to/libzpoline_loader.so ./your_program
```

**メリット**:
- フック本体が別ネームスペースに隔離される
- より強固な再入防止
- アプリケーションコードを変更せずにフックを交換可能

## トラブルシューティング

### エラー: mmap failed

```
[zpoline] ERROR: Failed to setup trampoline: mmap failed
```

**原因**: VA=0へのマッピングが許可されていない

**解決方法**:
```bash
sudo sysctl -w vm.mmap_min_addr=0
```

### システムコールが置換されない

**確認事項**:
1. `libzpoline_loader.so`が正しくプリロードされているか
2. 実行ファイルが動的リンクされているか（`ldd`コマンドで確認）
3. セキュリティ機能（SELinux、AppArmorなど）が干渉していないか

### セグメンテーションフォルト

**考えられる原因**:
1. トランポリンコードの生成に問題がある
2. 書き換え対象外にすべき領域が書き換えられている
3. スタック破壊やレジスタ保存の問題

**デバッグ方法**:
```bash
# gdbで実行
gdb --args env LD_PRELOAD=./target/release/libzpoline_loader.so ./target/release/zpoline_samples

# 詳細なログを確認
export RUST_BACKTRACE=1
LD_PRELOAD=./target/release/libzpoline_loader.so ./target/release/zpoline_samples 2>&1 | less
```

## 制約事項

1. **VA=0要件**: システムが仮想アドレス0へのマッピングを許可する必要がある
2. **x86-64のみ**: 現在はx86-64 Linuxのみサポート
3. **vDSOは対象外**: vDSO経由のシステムコール（一部の`clock_gettime`など）はフック不可
4. **JIT非対応**: JITコンパイラや自己書換えコードには未対応
5. **静的リンクバイナリ**: 完全な静的リンクバイナリには `LD_PRELOAD` が効かない

## パフォーマンス

zpoline-rsは、元のzpoline同様、非常に低いオーバーヘッドでシステムコールをフックします：

- **ptrace**よりも約100倍高速
- **seccomp/BPF**よりも約10倍高速
- 通常のシステムコールに対して数%のオーバーヘッド

詳細なベンチマーク結果は、元のzpolineの論文を参照してください。

## セキュリティ上の注意

- **VA=0の使用**: セキュリティ機構（NULL pointer dereference protection）を無効化します
- **本番環境**: 本番環境での使用は、セキュリティリスクを十分に評価してください
- **権限**: 通常はroot権限は不要ですが、初回設定には`sudo`が必要です

## 参考資料

- [元のzpoline実装](https://github.com/yasukata/zpoline)
- [zpoline論文](https://www.usenix.org/conference/atc23/presentation/yasukata)
- [Syscall User Dispatch](https://docs.kernel.org/admin-guide/syscall-user-dispatch.html)
- [lazypoline](https://github.com/lazypoline/lazypoline)

## ライセンス

MIT OR Apache-2.0
