# Trait-based Syscall Hooks Usage Guide

## 概要

zpoline-rsは、Rustのtrait systemを使用した型安全なシステムコールフック機構を提供します。この機能により、カスタムフックライブラリを簡単に作成できます。

## アーキテクチャ

trait-based hooksは、zpolineの`dlmopen`アーキテクチャを活用しています：

1. **zpoline_loader** - LD_PRELOAD経由でロードされ、トランポリン生成とコード書き換えを実行
2. **フックライブラリ** - `dlmopen`で別ネームスペースにロードされるcdylib
3. **zpoline_hook_api** - 両者で共有されるAPI（各ネームスペースに独立したコピー）

### 重要な設計ポイント

- フックライブラリは**cdylib**として作成する必要があります
- `dlmopen`により別ネームスペースにロードされるため、静的変数は分離されます
- 各ネームスペースが独自の`zpoline_hook_api`コピーを持ちます
- この分離により、フックライブラリ内でのtraitオブジェクト管理が安全に行えます

## 使用方法

### 1. フックライブラリの作成

#### Cargo.toml

```toml
[package]
name = "my_custom_hooks"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
zpoline_hook_api = { path = "../zpoline_hook_api" }
ctor = "0.2"
libc = "0.2"
```

#### src/lib.rs

```rust
use zpoline_hook_api::{
    register_syscall_hooks,
    syscall_hooks::*,
    SyscallHooks,
    get_trait_dispatch_hook
};
use ctor::ctor;
use std::sync::atomic::{AtomicUsize, Ordering};

/// カスタムフック実装
struct MyHooks;

// 統計カウンタ（例）
static WRITE_COUNT: AtomicUsize = AtomicUsize::new(0);
static READ_COUNT: AtomicUsize = AtomicUsize::new(0);

impl SyscallHooks for MyHooks {
    /// writeシステムコールをフック
    fn hook_write(&mut self, fd: i32, buf: *const std::ffi::c_void, count: usize) -> isize {
        WRITE_COUNT.fetch_add(1, Ordering::Relaxed);

        // カスタム処理をここに記述

        // デフォルト実装を呼び出して実際のシステムコールを実行
        default_write(fd, buf, count)
    }

    /// readシステムコールをフック
    fn hook_read(&mut self, fd: i32, buf: *mut std::ffi::c_void, count: usize) -> isize {
        READ_COUNT.fetch_add(1, Ordering::Relaxed);
        default_read(fd, buf, count)
    }

    // 他のsyscallメソッドもオーバーライド可能
    // オーバーライドしないメソッドはデフォルト実装が使用される
}

/// ライブラリロード時に自動実行される初期化関数
#[ctor]
fn init_hooks() {
    eprintln!("[my_custom_hooks] Initializing hooks...");
    register_syscall_hooks(MyHooks);
    eprintln!("[my_custom_hooks] Hooks registered");
}

/// zpoline_loaderから呼ばれる初期化関数
/// trait-based dispatcherへのポインタを返す
#[no_mangle]
pub extern "C" fn zpoline_hook_init() -> *const () {
    get_trait_dispatch_hook()
}
```

### 2. ビルド

```bash
cargo build --release
```

ビルド成果物は `target/release/libmy_custom_hooks.so` に生成されます。

### 3. 実行

```bash
# VA=0の設定（初回のみ、要sudo）
sudo sysctl -w vm.mmap_min_addr=0

# ZPOLINE_HOOK環境変数でフックライブラリを指定
ZPOLINE_HOOK=./target/release/libmy_custom_hooks.so \
LD_PRELOAD=./target/release/libzpoline_loader.so \
./your_program
```

## 利用可能なSyscallHooks メソッド

`SyscallHooks` traitは以下のsyscallメソッドを提供します（一部抜粋）：

### ファイルI/O
- `hook_read(fd, buf, count) -> isize`
- `hook_write(fd, buf, count) -> isize`
- `hook_open(pathname, flags, mode) -> i32`
- `hook_close(fd) -> i32`
- `hook_lseek(fd, offset, whence) -> off_t`
- `hook_openat(dirfd, pathname, flags, mode) -> i32`

### メモリ管理
- `hook_mmap(addr, length, prot, flags, fd, offset) -> *mut c_void`
- `hook_munmap(addr, length) -> i32`
- `hook_mprotect(addr, len, prot) -> i32`
- `hook_brk(addr) -> i32`

### プロセス管理
- `hook_getpid() -> pid_t`
- `hook_gettid() -> pid_t`
- `hook_fork() -> pid_t`
- `hook_execve(pathname, argv, envp) -> i32`
- `hook_exit(status) -> !`
- `hook_exit_group(status) -> !`

### ネットワーク
- `hook_socket(domain, ty, protocol) -> i32`
- `hook_connect(sockfd, addr, addrlen) -> i32`
- `hook_accept(sockfd, addr, addrlen) -> i32`
- `hook_bind(sockfd, addr, addrlen) -> i32`
- `hook_listen(sockfd, backlog) -> i32`

### その他
- `hook_ioctl(fd, request, arg) -> i32`
- `hook_access(pathname, mode) -> i32`
- `hook_pipe(pipefd) -> i32`
- `hook_dup(oldfd) -> i32`
- `hook_dup2(oldfd, newfd) -> i32`

完全なリストは `zpoline_hook_api/src/syscall_hooks.rs` を参照してください。

## デフォルト実装関数

各syscallには対応する`default_*`関数が提供されています：

- `default_read(fd, buf, count) -> isize`
- `default_write(fd, buf, count) -> isize`
- `default_open(pathname, flags, mode) -> i32`
- など

これらの関数を呼び出すことで、元のシステムコールを実行できます。

## 実装例

### 例1: システムコール統計収集

```rust
impl SyscallHooks for StatsHook {
    fn hook_write(&mut self, fd: i32, buf: *const std::ffi::c_void, count: usize) -> isize {
        // 統計を更新
        WRITE_COUNT.fetch_add(1, Ordering::Relaxed);
        TOTAL_BYTES_WRITTEN.fetch_add(count, Ordering::Relaxed);

        // 実際のシステムコールを実行
        default_write(fd, buf, count)
    }
}
```

### 例2: 特定のファイルへのアクセス監視

```rust
impl SyscallHooks for SecurityHook {
    fn hook_open(&mut self, pathname: *const i8, flags: i32, mode: u32) -> i32 {
        // パス名を取得
        let path_str = unsafe {
            if !pathname.is_null() {
                std::ffi::CStr::from_ptr(pathname)
                    .to_string_lossy()
                    .to_string()
            } else {
                "<null>".to_string()
            }
        };

        // 特定のパスをログ出力
        if path_str.contains("/etc/") {
            eprintln!("[SECURITY] Accessing: {}", path_str);
        }

        default_open(pathname, flags, mode)
    }
}
```

### 例3: システムコールの戻り値変更

```rust
impl SyscallHooks for MockHook {
    fn hook_read(&mut self, fd: i32, buf: *mut std::ffi::c_void, count: usize) -> isize {
        // 特定のfdに対してモックデータを返す
        if fd == 42 {
            // モックデータをバッファに書き込む
            unsafe {
                let mock_data = b"mock data";
                let len = std::cmp::min(mock_data.len(), count);
                std::ptr::copy_nonoverlapping(
                    mock_data.as_ptr(),
                    buf as *mut u8,
                    len
                );
                return len as isize;
            }
        }

        // それ以外は通常のシステムコールを実行
        default_read(fd, buf, count)
    }
}
```

## 注意事項

### 再入について

フック関数内で`eprintln!`や`println!`を使用すると、それ自体が`write`システムコールを発行するため再入が発生します。zpoline_hook_apiは再入ガード機構を持っていますが、パフォーマンスに影響する可能性があります。

本番環境では：
- `AtomicUsize`などの syscallを使わない方法でカウントする
- ログ出力を最小限にする
- バッファリングを活用する

### メモリ安全性

フック関数はCのABIで呼ばれるため、ポインタの扱いに注意が必要です：
- null ポインタチェックを行う
- バッファサイズを検証する
- 安全でない操作は`unsafe`ブロック内で行う

### パフォーマンス

フック関数は全てのシステムコールで呼ばれるため、軽量に保つ必要があります：
- 重い処理は避ける
- ロックの競合を最小化する
- 必要な場合のみ処理を行う

## トラブルシューティング

### フックが呼ばれない

1. `ZPOLINE_HOOK`環境変数が正しく設定されているか確認
2. フックライブラリが`cdylib`としてビルドされているか確認
3. `#[ctor]`で`register_syscall_hooks()`が呼ばれているか確認
4. `zpoline_hook_init()`が正しく実装されているか確認

### セグメンテーションフォルト

1. ポインタのnullチェックを追加
2. バッファサイズの検証を行う
3. 再入による問題の可能性を確認

### パフォーマンス問題

1. フック関数内の処理を最適化
2. 不要なログ出力を削減
3. 統計収集にアトミック操作を使用

## 実装済みサンプル

`zpoline_hook_trait_example` ディレクトリに完全な実装例があります：

```bash
# サンプルをビルド
cargo build --release -p zpoline_hook_trait_example

# サンプルを実行
ZPOLINE_HOOK=./target/release/libzpoline_hook_trait_example.so \
LD_PRELOAD=./target/release/libzpoline_loader.so \
./your_program
```

## まとめ

trait-based hooksにより、型安全でRustらしい方法でシステムコールをフックできます。`dlmopen`による分離により、安全かつ柔軟なフック機構が実現されています。

カスタムフックライブラリを作成することで：
- システムコールの監視
- セキュリティチェック
- パフォーマンス測定
- モックやテスト支援
- ユーザー空間実装への差し替え

などが可能になります。
