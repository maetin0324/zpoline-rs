use zpoline_hook_api::{register_syscall_hooks, syscall_hooks::*, SyscallHooks};
use std::sync::atomic::{AtomicUsize, Ordering};

/// シンプルなログ出力フック
struct LoggingHook;

// 統計カウンタ（システムコールを使わずに記録）
static WRITE_COUNT: AtomicUsize = AtomicUsize::new(0);
static READ_COUNT: AtomicUsize = AtomicUsize::new(0);
static OPEN_COUNT: AtomicUsize = AtomicUsize::new(0);
static GETPID_COUNT: AtomicUsize = AtomicUsize::new(0);

impl SyscallHooks for LoggingHook {
    /// write システムコールをフック
    fn hook_write(&mut self, fd: i32, buf: *const std::ffi::c_void, count: usize) -> isize {
        // eprintln!は使わない（再入の原因）
        WRITE_COUNT.fetch_add(1, Ordering::Relaxed);
        default_write(fd, buf, count)
    }

    /// read システムコールをフック
    fn hook_read(&mut self, fd: i32, buf: *mut std::ffi::c_void, count: usize) -> isize {
        READ_COUNT.fetch_add(1, Ordering::Relaxed);
        default_read(fd, buf, count)
    }

    /// open システムコールをフック
    fn hook_open(&mut self, pathname: *const i8, flags: i32, mode: u32) -> i32 {
        OPEN_COUNT.fetch_add(1, Ordering::Relaxed);
        default_open(pathname, flags, mode)
    }

    /// getpid システムコールをフック
    fn hook_getpid(&mut self) -> i32 {
        GETPID_COUNT.fetch_add(1, Ordering::Relaxed);
        default_getpid()
    }
}

fn main() {
    // フックを登録（main()の最初で実行）
    register_syscall_hooks(LoggingHook);

    // ここで統計を表示すると、writeシステムコールが発生するが、
    // フックが機能していることを確認できる

    println!("=== zpoline-rs Simple Hooks Example ===");
    println!();
    println!("Hooks are registered. Performing operations...");
    println!();

    // 直接writeシステムコールを呼び出してテスト
    unsafe {
        let msg = b"Direct syscall test\n";
        libc::write(1, msg.as_ptr() as *const libc::c_void, msg.len());
    }

    // いくつかの操作を実行
    println!("1. Hello, World!");
    println!("2. This is a test message.");

    let pid = std::process::id();
    println!("3. My process ID is: {}", pid);

    // ファイル操作
    if let Ok(contents) = std::fs::read_to_string("/etc/hostname") {
        println!("4. Hostname: {}", contents.trim());
    }

    // 統計を表示
    println!();
    println!("=== Syscall Statistics ===");
    println!("write() calls: {}", WRITE_COUNT.load(Ordering::Relaxed));
    println!("read() calls: {}", READ_COUNT.load(Ordering::Relaxed));
    println!("open() calls: {}", OPEN_COUNT.load(Ordering::Relaxed));
    println!("getpid() calls: {}", GETPID_COUNT.load(Ordering::Relaxed));

    // デバッグ: フック関数が呼ばれた回数を確認
    println!();
    println!("=== Debug Info ===");
    let hook_entry_count = zpoline_hook_api::get_hook_entry_call_count();
    let trait_hook_count = zpoline_hook_api::get_trait_hook_call_count();
    println!("hook_entry() calls: {}", hook_entry_count);
    println!("trait_based_hook() calls: {}", trait_hook_count);

    println!();
    println!("Example completed successfully!");
    println!("Note: The counts above include syscalls made by println! itself.");
}
