use zpoline_hook_api::{register_syscall_hooks, syscall_hooks::*, SyscallHooks};
use std::sync::atomic::{AtomicUsize, Ordering};

/// 統計情報を収集するカスタムフック
struct StatisticsHook {
    write_count: AtomicUsize,
    read_count: AtomicUsize,
    open_count: AtomicUsize,
    total_bytes_written: AtomicUsize,
    total_bytes_read: AtomicUsize,
}

impl StatisticsHook {
    fn new() -> Self {
        Self {
            write_count: AtomicUsize::new(0),
            read_count: AtomicUsize::new(0),
            open_count: AtomicUsize::new(0),
            total_bytes_written: AtomicUsize::new(0),
            total_bytes_read: AtomicUsize::new(0),
        }
    }

    fn print_statistics(&self) {
        eprintln!("\n========== Syscall Statistics ==========");
        eprintln!("write() calls: {}", self.write_count.load(Ordering::Relaxed));
        eprintln!("  Total bytes written: {}", self.total_bytes_written.load(Ordering::Relaxed));
        eprintln!("read() calls: {}", self.read_count.load(Ordering::Relaxed));
        eprintln!("  Total bytes read: {}", self.total_bytes_read.load(Ordering::Relaxed));
        eprintln!("open() calls: {}", self.open_count.load(Ordering::Relaxed));
        eprintln!("========================================\n");
    }
}

// 統計を保存するためのグローバル変数（デモ用）
static mut STATS: Option<StatisticsHook> = None;

impl SyscallHooks for StatisticsHook {
    /// write システムコールをフック
    fn hook_write(&mut self, fd: i32, buf: *const std::ffi::c_void, count: usize) -> isize {
        self.write_count.fetch_add(1, Ordering::Relaxed);

        // 実際のシステムコールを実行
        let result = default_write(fd, buf, count);

        // 成功した場合、書き込まれたバイト数を記録
        if result > 0 {
            self.total_bytes_written.fetch_add(result as usize, Ordering::Relaxed);
        }

        result
    }

    /// read システムコールをフック
    fn hook_read(&mut self, fd: i32, buf: *mut std::ffi::c_void, count: usize) -> isize {
        self.read_count.fetch_add(1, Ordering::Relaxed);

        // 実際のシステムコールを実行
        let result = default_read(fd, buf, count);

        // 成功した場合、読み込まれたバイト数を記録
        if result > 0 {
            self.total_bytes_read.fetch_add(result as usize, Ordering::Relaxed);
        }

        result
    }

    /// open システムコールをフック
    fn hook_open(&mut self, pathname: *const i8, flags: i32, mode: u32) -> i32 {
        self.open_count.fetch_add(1, Ordering::Relaxed);

        // パス名を取得してログ出力
        let path_str = unsafe {
            if !pathname.is_null() {
                std::ffi::CStr::from_ptr(pathname)
                    .to_string_lossy()
                    .to_string()
            } else {
                "<null>".to_string()
            }
        };

        eprintln!("[HOOK] open(\"{}\", flags={:#x}, mode={:#o})", path_str, flags, mode);

        // 実際のシステムコールを実行
        default_open(pathname, flags, mode)
    }

    /// getpid システムコールをフック
    fn hook_getpid(&mut self) -> i32 {
        let pid = default_getpid();
        eprintln!("[HOOK] getpid() = {}", pid);
        pid
    }
}

fn main() {
    eprintln!("zpoline-rs trait-based hooks demo");
    eprintln!("==================================\n");

    // 統計情報を保存するインスタンスを作成
    unsafe {
        STATS = Some(StatisticsHook::new());
    }

    // フックを登録
    register_syscall_hooks(StatisticsHook::new());

    eprintln!("[INFO] Custom hooks registered using SyscallHooks trait\n");

    // 様々なシステムコールを実行してテスト
    perform_syscall_tests();

    // 統計を表示
    unsafe {
        if let Some(ref stats) = STATS {
            stats.print_statistics();
        }
    }

    eprintln!("[INFO] Demo completed successfully!");
}

fn perform_syscall_tests() {
    use std::fs::File;
    use std::io::{Read, Write};
    use std::process;

    eprintln!("=== Test 1: Write to stdout ===");
    println!("Hello from zpoline-rs!");
    println!("This message goes through our custom write() hook.");

    eprintln!("\n=== Test 2: Get process ID ===");
    let pid = process::id();
    println!("Process ID: {}", pid);

    eprintln!("\n=== Test 3: File operations ===");
    // ファイルを作成して書き込み
    let test_file = "/tmp/zpoline_test.txt";
    match File::create(test_file) {
        Ok(mut file) => {
            let data = b"Test data from zpoline-rs\n";
            match file.write_all(data) {
                Ok(_) => println!("Successfully wrote to {}", test_file),
                Err(e) => eprintln!("Failed to write: {}", e),
            }
        }
        Err(e) => eprintln!("Failed to create file: {}", e),
    }

    // ファイルを読み込み
    match File::open(test_file) {
        Ok(mut file) => {
            let mut buffer = String::new();
            match file.read_to_string(&mut buffer) {
                Ok(bytes) => {
                    println!("Read {} bytes from {}", bytes, test_file);
                    println!("Content: {}", buffer.trim());
                }
                Err(e) => eprintln!("Failed to read: {}", e),
            }
        }
        Err(e) => eprintln!("Failed to open file: {}", e),
    }

    // ファイルを削除
    match std::fs::remove_file(test_file) {
        Ok(_) => println!("Cleaned up {}", test_file),
        Err(e) => eprintln!("Failed to remove file: {}", e),
    }

    eprintln!("\n=== Test 4: Multiple writes ===");
    for i in 1..=5 {
        println!("Message #{}", i);
    }
}
