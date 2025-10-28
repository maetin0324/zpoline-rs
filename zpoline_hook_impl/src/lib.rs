use zpoline_hook_api::SyscallRegs;

/// デフォルトのフック実装
/// このライブラリはdlmopenで別ネームスペースにロードされる
///
/// システムコールをトレースして標準エラー出力に表示する

/// フック関数のエントリポイント
/// dlmopenでロードされたときに呼ばれる
#[no_mangle]
pub extern "C" fn zpoline_hook_function(regs: &mut SyscallRegs) -> i64 {
    // システムコール番号に応じて名前を表示
    let syscall_name = match regs.rax {
        0 => "read",
        1 => "write",
        2 => "open",
        3 => "close",
        4 => "stat",
        5 => "fstat",
        6 => "lstat",
        7 => "poll",
        8 => "lseek",
        9 => "mmap",
        10 => "mprotect",
        11 => "munmap",
        12 => "brk",
        13 => "rt_sigaction",
        14 => "rt_sigprocmask",
        15 => "rt_sigreturn",
        16 => "ioctl",
        17 => "pread64",
        18 => "pwrite64",
        19 => "readv",
        20 => "writev",
        21 => "access",
        22 => "pipe",
        23 => "select",
        24 => "sched_yield",
        25 => "mremap",
        26 => "msync",
        27 => "mincore",
        28 => "madvise",
        29 => "shmget",
        30 => "shmat",
        31 => "shmctl",
        32 => "dup",
        33 => "dup2",
        34 => "pause",
        35 => "nanosleep",
        36 => "getitimer",
        37 => "alarm",
        38 => "setitimer",
        39 => "getpid",
        40 => "sendfile",
        41 => "socket",
        42 => "connect",
        43 => "accept",
        44 => "sendto",
        45 => "recvfrom",
        46 => "sendmsg",
        47 => "recvmsg",
        48 => "shutdown",
        49 => "bind",
        50 => "listen",
        51 => "getsockname",
        52 => "getpeername",
        53 => "socketpair",
        54 => "setsockopt",
        55 => "getsockopt",
        56 => "clone",
        57 => "fork",
        58 => "vfork",
        59 => "execve",
        60 => "exit",
        61 => "wait4",
        62 => "kill",
        63 => "uname",
        186 => "gettid",
        231 => "exit_group",
        257 => "openat",
        262 => "newfstatat",
        _ => "<unknown>",
    };

    // トレース出力（stderrを使用してstdoutと混ざらないように）
    // 注意: eprintln!はmallocを使う可能性があるため、
    // 再入の可能性があります。本番環境では注意が必要です。
    eprintln!(
        "[HOOK] {} (nr={}, args=[{:#x}, {:#x}, {:#x}, {:#x}, {:#x}, {:#x}])",
        syscall_name, regs.rax, regs.rdi, regs.rsi, regs.rdx, regs.r10, regs.r8, regs.r9
    );

    // 元のシステムコールを実行
    unsafe { zpoline_hook_api::raw_syscall(regs) }
}

/// ライブラリの初期化関数
/// dlmopenでロードされた後、この関数がzpoline_loaderから呼ばれる
#[no_mangle]
pub extern "C" fn zpoline_hook_init() -> *const () {
    eprintln!("[zpoline_hook_impl] Hook library loaded in separate namespace");
    zpoline_hook_function as *const ()
}
