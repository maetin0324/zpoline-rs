use zpoline_hook_api::{SyscallRegs, __hook_init};

/// システムコールをトレースするカスタムフック
extern "C" fn trace_hook(regs: &mut SyscallRegs) -> i64 {
    // システムコール番号に応じて名前を表示
    let syscall_name = match regs.rax {
        0 => "read",
        1 => "write",
        2 => "open",
        3 => "close",
        4 => "stat",
        5 => "fstat",
        9 => "mmap",
        10 => "mprotect",
        11 => "munmap",
        39 => "getpid",
        60 => "exit",
        158 => "arch_prctl",
        186 => "gettid",
        231 => "exit_group",
        _ => "<unknown>",
    };

    eprintln!(
        "[TRACE] syscall: {} (nr={}, args=[{:#x}, {:#x}, {:#x}, {:#x}, {:#x}, {:#x}])",
        syscall_name, regs.rax, regs.rdi, regs.rsi, regs.rdx, regs.r10, regs.r8, regs.r9
    );

    // 元のシステムコールを実行
    unsafe { zpoline_hook_api::raw_syscall(regs) }
}

fn main() {
    // フック関数を登録
    __hook_init(trace_hook);

    println!("zpoline-rs sample program");
    println!("This program traces system calls made by the process.");
    println!();

    // いくつかのシステムコールを実行してトレースを確認
    println!("Executing some system calls...");

    // write syscall (println!経由)
    println!("1. Hello from zpoline-rs!");

    // getpid syscall
    let pid = std::process::id();
    println!("2. Process ID: {}", pid);

    // open/read/close syscalls (ファイル読み込み)
    match std::fs::read_to_string("/proc/self/comm") {
        Ok(comm) => println!("3. Process name: {}", comm.trim()),
        Err(e) => eprintln!("3. Failed to read process name: {}", e),
    }

    // stat syscall
    match std::fs::metadata("/tmp") {
        Ok(meta) => println!("4. /tmp is a directory: {}", meta.is_dir()),
        Err(e) => eprintln!("4. Failed to stat /tmp: {}", e),
    }

    println!();
    println!("Sample completed. Check stderr for syscall trace output.");
}
