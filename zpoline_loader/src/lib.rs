mod init;
mod trampoline;

use ctor::ctor;
use std::sync::Once;

static INIT_ONCE: Once = Once::new();

/// LD_PRELOADによってロードされた際に自動的に呼ばれる初期化関数
#[ctor]
fn zpoline_init() {
    INIT_ONCE.call_once(|| {
        eprintln!("[zpoline] Initializing zpoline-rs...");

        // VA=0トランポリンの生成
        match trampoline::setup_trampoline() {
            Ok(()) => {
                eprintln!("[zpoline] Trampoline setup successful at address 0x0");
            }
            Err(e) => {
                eprintln!("[zpoline] ERROR: Failed to setup trampoline: {}", e);
                eprintln!("[zpoline] Make sure /proc/sys/vm/mmap_min_addr is set to 0");
                eprintln!("[zpoline] Run: sudo sysctl -w vm.mmap_min_addr=0");
                std::process::exit(1);
            }
        }

        // コード書き換えの実行
        match init::rewrite_syscalls() {
            Ok(stats) => {
                eprintln!("[zpoline] Code rewriting completed:");
                eprintln!("[zpoline]   Regions scanned: {}", stats.regions_scanned);
                eprintln!("[zpoline]   Regions rewritten: {}", stats.regions_rewritten);
                eprintln!("[zpoline]   Syscalls replaced: {}", stats.syscalls_replaced);
                eprintln!("[zpoline]   Sysenters replaced: {}", stats.sysenters_replaced);
                eprintln!("[zpoline]   Regions skipped: {}", stats.regions_skipped);
            }
            Err(e) => {
                eprintln!("[zpoline] ERROR: Failed to rewrite syscalls: {}", e);
                std::process::exit(1);
            }
        }

        eprintln!("[zpoline] Initialization complete!");
    });
}

// cdylibとしてエクスポートする必要がある関数
// これによりライブラリがロード時に初期化される
#[no_mangle]
pub extern "C" fn zpoline_get_version() -> *const u8 {
    b"zpoline-rs 0.1.0\0".as_ptr()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        let version = unsafe {
            let ptr = zpoline_get_version();
            std::ffi::CStr::from_ptr(ptr as *const i8)
        };
        assert!(version.to_str().unwrap().starts_with("zpoline-rs"));
    }
}
