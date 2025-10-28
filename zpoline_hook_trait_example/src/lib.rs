use zpoline_hook_api::{register_syscall_hooks, syscall_hooks::*, SyscallHooks, get_trait_dispatch_hook};
use ctor::ctor;
use std::sync::atomic::{AtomicUsize, Ordering};

/// システムコール統計を収集するフック
struct StatsHook;

// 統計カウンタ
static WRITE_COUNT: AtomicUsize = AtomicUsize::new(0);
static READ_COUNT: AtomicUsize = AtomicUsize::new(0);
static OPEN_COUNT: AtomicUsize = AtomicUsize::new(0);
static GETPID_COUNT: AtomicUsize = AtomicUsize::new(0);

impl SyscallHooks for StatsHook {
    /// write システムコールをフック
    fn hook_write(&mut self, fd: i32, buf: *const std::ffi::c_void, count: usize) -> isize {
        let count_val = WRITE_COUNT.fetch_add(1, Ordering::Relaxed);
        // 最初の3回だけログ出力
        if count_val < 3 {
            eprintln!("[TRAIT HOOK] write(fd={}, count={}) - call #{}", fd, count, count_val + 1);
        }
        default_write(fd, buf, count)
    }

    /// read システムコールをフック
    fn hook_read(&mut self, fd: i32, buf: *mut std::ffi::c_void, count: usize) -> isize {
        let count_val = READ_COUNT.fetch_add(1, Ordering::Relaxed);
        // 最初の3回だけログ出力
        if count_val < 3 {
            eprintln!("[TRAIT HOOK] read(fd={}, count={}) - call #{}", fd, count, count_val + 1);
        }
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

/// ライブラリがロードされたときにフックを登録
#[ctor]
fn init_hooks() {
    eprintln!("[zpoline_hook_trait_example] Initializing trait-based hooks...");
    register_syscall_hooks(StatsHook);
    eprintln!("[zpoline_hook_trait_example] Hooks registered successfully");
}

/// zpoline_loaderから呼ばれる初期化関数
/// trait-based dispatcherへのポインタを返す
#[no_mangle]
pub extern "C" fn zpoline_hook_init() -> *const () {
    eprintln!("[zpoline_hook_trait_example] zpoline_hook_init called");
    get_trait_dispatch_hook()
}

/// 統計情報を取得するためのエクスポート関数（オプション）
#[no_mangle]
pub extern "C" fn get_stats() {
    eprintln!("=== Syscall Statistics (from hook library) ===");
    eprintln!("write() calls: {}", WRITE_COUNT.load(Ordering::Relaxed));
    eprintln!("read() calls: {}", READ_COUNT.load(Ordering::Relaxed));
    eprintln!("open() calls: {}", OPEN_COUNT.load(Ordering::Relaxed));
    eprintln!("getpid() calls: {}", GETPID_COUNT.load(Ordering::Relaxed));
}
