use crate::{raw_syscall, SyscallRegs};
use libc::{c_char, c_int, c_uint, c_ulong, c_void, off_t, pid_t, size_t, ssize_t};

/// システムコールフックのためのtrait
///
/// このtraitを実装することで、特定のシステムコールに対するカスタム処理を
/// 型安全に記述できます。オーバーライドしないメソッドはデフォルト実装が
/// 使用され、元のシステムコールがそのまま実行されます。
///
/// # 使用例
///
/// ```rust
/// use zpoline_hook_api::{SyscallHooks, register_syscall_hooks};
///
/// struct MyHooks;
///
/// impl SyscallHooks for MyHooks {
///     fn hook_write(&mut self, fd: i32, buf: *const u8, count: usize) -> isize {
///         eprintln!("[CUSTOM] write called: fd={}, count={}", fd, count);
///         // デフォルトの実装を呼ぶ
///         default_write(fd, buf as *const _, count)
///     }
///
///     fn hook_open(&mut self, pathname: *const i8, flags: i32, mode: u32) -> i32 {
///         eprintln!("[CUSTOM] open called: flags={:#x}", flags);
///         default_open(pathname, flags, mode)
///     }
/// }
/// ```
pub trait SyscallHooks: Send + Sync + 'static {
    // ========================================================================
    // ファイルI/O関連
    // ========================================================================

    /// read(2) - ファイルディスクリプタから読み込み
    fn hook_read(&mut self, fd: c_int, buf: *mut c_void, count: size_t) -> ssize_t {
        default_read(fd, buf, count)
    }

    /// write(2) - ファイルディスクリプタへ書き込み
    fn hook_write(&mut self, fd: c_int, buf: *const c_void, count: size_t) -> ssize_t {
        default_write(fd, buf, count)
    }

    /// open(2) - ファイルを開く
    fn hook_open(&mut self, pathname: *const c_char, flags: c_int, mode: c_uint) -> c_int {
        default_open(pathname, flags, mode)
    }

    /// close(3) - ファイルディスクリプタを閉じる
    fn hook_close(&mut self, fd: c_int) -> c_int {
        default_close(fd)
    }

    /// lseek(2) - ファイルのオフセット位置を変更
    fn hook_lseek(&mut self, fd: c_int, offset: off_t, whence: c_int) -> off_t {
        default_lseek(fd, offset, whence)
    }

    /// openat(2) - ディレクトリファイルディスクリプタに相対的にファイルを開く
    fn hook_openat(
        &mut self,
        dirfd: c_int,
        pathname: *const c_char,
        flags: c_int,
        mode: c_uint,
    ) -> c_int {
        default_openat(dirfd, pathname, flags, mode)
    }

    /// dup(2) - ファイルディスクリプタを複製
    fn hook_dup(&mut self, oldfd: c_int) -> c_int {
        default_dup(oldfd)
    }

    /// dup2(2) - ファイルディスクリプタを複製（番号指定）
    fn hook_dup2(&mut self, oldfd: c_int, newfd: c_int) -> c_int {
        default_dup2(oldfd, newfd)
    }

    /// pipe(2) - パイプを作成
    fn hook_pipe(&mut self, pipefd: *mut c_int) -> c_int {
        default_pipe(pipefd)
    }

    // ========================================================================
    // メモリ管理関連
    // ========================================================================

    /// mmap(2) - メモリをマッピング
    fn hook_mmap(
        &mut self,
        addr: *mut c_void,
        length: size_t,
        prot: c_int,
        flags: c_int,
        fd: c_int,
        offset: off_t,
    ) -> *mut c_void {
        default_mmap(addr, length, prot, flags, fd, offset)
    }

    /// munmap(2) - メモリマッピングを解除
    fn hook_munmap(&mut self, addr: *mut c_void, length: size_t) -> c_int {
        default_munmap(addr, length)
    }

    /// mprotect(2) - メモリ保護を変更
    fn hook_mprotect(&mut self, addr: *mut c_void, len: size_t, prot: c_int) -> c_int {
        default_mprotect(addr, len, prot)
    }

    /// brk(2) - データセグメント終端を変更
    fn hook_brk(&mut self, addr: *mut c_void) -> c_int {
        default_brk(addr)
    }

    // ========================================================================
    // プロセス管理関連
    // ========================================================================

    /// getpid(2) - プロセスIDを取得
    fn hook_getpid(&mut self) -> pid_t {
        default_getpid()
    }

    /// gettid(2) - スレッドIDを取得
    fn hook_gettid(&mut self) -> pid_t {
        default_gettid()
    }

    /// fork(2) - 子プロセスを作成
    fn hook_fork(&mut self) -> pid_t {
        default_fork()
    }

    /// execve(2) - プログラムを実行
    fn hook_execve(
        &mut self,
        pathname: *const c_char,
        argv: *const *const c_char,
        envp: *const *const c_char,
    ) -> c_int {
        default_execve(pathname, argv, envp)
    }

    /// exit(2) - プロセスを終了
    fn hook_exit(&mut self, status: c_int) -> ! {
        default_exit(status)
    }

    /// exit_group(2) - すべてのスレッドを終了
    fn hook_exit_group(&mut self, status: c_int) -> ! {
        default_exit_group(status)
    }

    /// wait4(2) - プロセスの状態変化を待つ
    fn hook_wait4(
        &mut self,
        pid: pid_t,
        wstatus: *mut c_int,
        options: c_int,
        rusage: *mut c_void,
    ) -> pid_t {
        default_wait4(pid, wstatus, options, rusage)
    }

    /// kill(2) - シグナルを送信
    fn hook_kill(&mut self, pid: pid_t, sig: c_int) -> c_int {
        default_kill(pid, sig)
    }

    // ========================================================================
    // ネットワーク関連
    // ========================================================================

    /// socket(2) - ソケットを作成
    fn hook_socket(&mut self, domain: c_int, ty: c_int, protocol: c_int) -> c_int {
        default_socket(domain, ty, protocol)
    }

    /// connect(2) - ソケットを接続
    fn hook_connect(&mut self, sockfd: c_int, addr: *const c_void, addrlen: u32) -> c_int {
        default_connect(sockfd, addr, addrlen)
    }

    /// accept(2) - 接続を受け入れ
    fn hook_accept(&mut self, sockfd: c_int, addr: *mut c_void, addrlen: *mut u32) -> c_int {
        default_accept(sockfd, addr, addrlen)
    }

    /// bind(2) - ソケットにアドレスをバインド
    fn hook_bind(&mut self, sockfd: c_int, addr: *const c_void, addrlen: u32) -> c_int {
        default_bind(sockfd, addr, addrlen)
    }

    /// listen(2) - ソケットで接続を待つ
    fn hook_listen(&mut self, sockfd: c_int, backlog: c_int) -> c_int {
        default_listen(sockfd, backlog)
    }

    // ========================================================================
    // その他
    // ========================================================================

    /// ioctl(2) - デバイス制御
    fn hook_ioctl(&mut self, fd: c_int, request: c_ulong, arg: *mut c_void) -> c_int {
        default_ioctl(fd, request, arg)
    }

    /// access(2) - ファイルアクセス権限をチェック
    fn hook_access(&mut self, pathname: *const c_char, mode: c_int) -> c_int {
        default_access(pathname, mode)
    }
}

// ========================================================================
// デフォルト実装を提供する独立した関数
// これらの関数はユーザーのカスタムフック実装内で呼び出すことができます
// ========================================================================

pub fn default_read(fd: c_int, buf: *mut c_void, count: size_t) -> ssize_t {
    unsafe {
        let mut regs = SyscallRegs {
            rax: 0, // SYS_read
            rdi: fd as u64,
            rsi: buf as u64,
            rdx: count as u64,
            r10: 0,
            r8: 0,
            r9: 0,
        };
        raw_syscall(&mut regs) as ssize_t
    }
}

pub fn default_write(fd: c_int, buf: *const c_void, count: size_t) -> ssize_t {
    unsafe {
        let mut regs = SyscallRegs {
            rax: 1, // SYS_write
            rdi: fd as u64,
            rsi: buf as u64,
            rdx: count as u64,
            r10: 0,
            r8: 0,
            r9: 0,
        };
        raw_syscall(&mut regs) as ssize_t
    }
}

pub fn default_open(pathname: *const c_char, flags: c_int, mode: c_uint) -> c_int {
    unsafe {
        let mut regs = SyscallRegs {
            rax: 2, // SYS_open
            rdi: pathname as u64,
            rsi: flags as u64,
            rdx: mode as u64,
            r10: 0,
            r8: 0,
            r9: 0,
        };
        raw_syscall(&mut regs) as c_int
    }
}

pub fn default_close(fd: c_int) -> c_int {
    unsafe {
        let mut regs = SyscallRegs {
            rax: 3, // SYS_close
            rdi: fd as u64,
            rsi: 0,
            rdx: 0,
            r10: 0,
            r8: 0,
            r9: 0,
        };
        raw_syscall(&mut regs) as c_int
    }
}

pub fn default_lseek(fd: c_int, offset: off_t, whence: c_int) -> off_t {
    unsafe {
        let mut regs = SyscallRegs {
            rax: 8, // SYS_lseek
            rdi: fd as u64,
            rsi: offset as u64,
            rdx: whence as u64,
            r10: 0,
            r8: 0,
            r9: 0,
        };
        raw_syscall(&mut regs) as off_t
    }
}

pub fn default_openat(
    dirfd: c_int,
    pathname: *const c_char,
    flags: c_int,
    mode: c_uint,
) -> c_int {
    unsafe {
        let mut regs = SyscallRegs {
            rax: 257, // SYS_openat
            rdi: dirfd as u64,
            rsi: pathname as u64,
            rdx: flags as u64,
            r10: mode as u64,
            r8: 0,
            r9: 0,
        };
        raw_syscall(&mut regs) as c_int
    }
}

pub fn default_dup(oldfd: c_int) -> c_int {
    unsafe {
        let mut regs = SyscallRegs {
            rax: 32, // SYS_dup
            rdi: oldfd as u64,
            rsi: 0,
            rdx: 0,
            r10: 0,
            r8: 0,
            r9: 0,
        };
        raw_syscall(&mut regs) as c_int
    }
}

pub fn default_dup2(oldfd: c_int, newfd: c_int) -> c_int {
    unsafe {
        let mut regs = SyscallRegs {
            rax: 33, // SYS_dup2
            rdi: oldfd as u64,
            rsi: newfd as u64,
            rdx: 0,
            r10: 0,
            r8: 0,
            r9: 0,
        };
        raw_syscall(&mut regs) as c_int
    }
}

pub fn default_pipe(pipefd: *mut c_int) -> c_int {
    unsafe {
        let mut regs = SyscallRegs {
            rax: 22, // SYS_pipe
            rdi: pipefd as u64,
            rsi: 0,
            rdx: 0,
            r10: 0,
            r8: 0,
            r9: 0,
        };
        raw_syscall(&mut regs) as c_int
    }
}

pub fn default_mmap(
    addr: *mut c_void,
    length: size_t,
    prot: c_int,
    flags: c_int,
    fd: c_int,
    offset: off_t,
) -> *mut c_void {
    unsafe {
        let mut regs = SyscallRegs {
            rax: 9, // SYS_mmap
            rdi: addr as u64,
            rsi: length as u64,
            rdx: prot as u64,
            r10: flags as u64,
            r8: fd as u64,
            r9: offset as u64,
        };
        raw_syscall(&mut regs) as *mut c_void
    }
}

pub fn default_munmap(addr: *mut c_void, length: size_t) -> c_int {
    unsafe {
        let mut regs = SyscallRegs {
            rax: 11, // SYS_munmap
            rdi: addr as u64,
            rsi: length as u64,
            rdx: 0,
            r10: 0,
            r8: 0,
            r9: 0,
        };
        raw_syscall(&mut regs) as c_int
    }
}

pub fn default_mprotect(addr: *mut c_void, len: size_t, prot: c_int) -> c_int {
    unsafe {
        let mut regs = SyscallRegs {
            rax: 10, // SYS_mprotect
            rdi: addr as u64,
            rsi: len as u64,
            rdx: prot as u64,
            r10: 0,
            r8: 0,
            r9: 0,
        };
        raw_syscall(&mut regs) as c_int
    }
}

pub fn default_brk(addr: *mut c_void) -> c_int {
    unsafe {
        let mut regs = SyscallRegs {
            rax: 12, // SYS_brk
            rdi: addr as u64,
            rsi: 0,
            rdx: 0,
            r10: 0,
            r8: 0,
            r9: 0,
        };
        raw_syscall(&mut regs) as c_int
    }
}

pub fn default_getpid() -> pid_t {
    unsafe {
        let mut regs = SyscallRegs {
            rax: 39, // SYS_getpid
            rdi: 0,
            rsi: 0,
            rdx: 0,
            r10: 0,
            r8: 0,
            r9: 0,
        };
        raw_syscall(&mut regs) as pid_t
    }
}

pub fn default_gettid() -> pid_t {
    unsafe {
        let mut regs = SyscallRegs {
            rax: 186, // SYS_gettid
            rdi: 0,
            rsi: 0,
            rdx: 0,
            r10: 0,
            r8: 0,
            r9: 0,
        };
        raw_syscall(&mut regs) as pid_t
    }
}

pub fn default_fork() -> pid_t {
    unsafe {
        let mut regs = SyscallRegs {
            rax: 57, // SYS_fork
            rdi: 0,
            rsi: 0,
            rdx: 0,
            r10: 0,
            r8: 0,
            r9: 0,
        };
        raw_syscall(&mut regs) as pid_t
    }
}

pub fn default_execve(
    pathname: *const c_char,
    argv: *const *const c_char,
    envp: *const *const c_char,
) -> c_int {
    unsafe {
        let mut regs = SyscallRegs {
            rax: 59, // SYS_execve
            rdi: pathname as u64,
            rsi: argv as u64,
            rdx: envp as u64,
            r10: 0,
            r8: 0,
            r9: 0,
        };
        raw_syscall(&mut regs) as c_int
    }
}

pub fn default_exit(status: c_int) -> ! {
    unsafe {
        let mut regs = SyscallRegs {
            rax: 60, // SYS_exit
            rdi: status as u64,
            rsi: 0,
            rdx: 0,
            r10: 0,
            r8: 0,
            r9: 0,
        };
        raw_syscall(&mut regs);
        core::hint::unreachable_unchecked()
    }
}

pub fn default_exit_group(status: c_int) -> ! {
    unsafe {
        let mut regs = SyscallRegs {
            rax: 231, // SYS_exit_group
            rdi: status as u64,
            rsi: 0,
            rdx: 0,
            r10: 0,
            r8: 0,
            r9: 0,
        };
        raw_syscall(&mut regs);
        core::hint::unreachable_unchecked()
    }
}

pub fn default_wait4(
    pid: pid_t,
    wstatus: *mut c_int,
    options: c_int,
    rusage: *mut c_void,
) -> pid_t {
    unsafe {
        let mut regs = SyscallRegs {
            rax: 61, // SYS_wait4
            rdi: pid as u64,
            rsi: wstatus as u64,
            rdx: options as u64,
            r10: rusage as u64,
            r8: 0,
            r9: 0,
        };
        raw_syscall(&mut regs) as pid_t
    }
}

pub fn default_kill(pid: pid_t, sig: c_int) -> c_int {
    unsafe {
        let mut regs = SyscallRegs {
            rax: 62, // SYS_kill
            rdi: pid as u64,
            rsi: sig as u64,
            rdx: 0,
            r10: 0,
            r8: 0,
            r9: 0,
        };
        raw_syscall(&mut regs) as c_int
    }
}

pub fn default_socket(domain: c_int, ty: c_int, protocol: c_int) -> c_int {
    unsafe {
        let mut regs = SyscallRegs {
            rax: 41, // SYS_socket
            rdi: domain as u64,
            rsi: ty as u64,
            rdx: protocol as u64,
            r10: 0,
            r8: 0,
            r9: 0,
        };
        raw_syscall(&mut regs) as c_int
    }
}

pub fn default_connect(sockfd: c_int, addr: *const c_void, addrlen: u32) -> c_int {
    unsafe {
        let mut regs = SyscallRegs {
            rax: 42, // SYS_connect
            rdi: sockfd as u64,
            rsi: addr as u64,
            rdx: addrlen as u64,
            r10: 0,
            r8: 0,
            r9: 0,
        };
        raw_syscall(&mut regs) as c_int
    }
}

pub fn default_accept(sockfd: c_int, addr: *mut c_void, addrlen: *mut u32) -> c_int {
    unsafe {
        let mut regs = SyscallRegs {
            rax: 43, // SYS_accept
            rdi: sockfd as u64,
            rsi: addr as u64,
            rdx: addrlen as u64,
            r10: 0,
            r8: 0,
            r9: 0,
        };
        raw_syscall(&mut regs) as c_int
    }
}

pub fn default_bind(sockfd: c_int, addr: *const c_void, addrlen: u32) -> c_int {
    unsafe {
        let mut regs = SyscallRegs {
            rax: 49, // SYS_bind
            rdi: sockfd as u64,
            rsi: addr as u64,
            rdx: addrlen as u64,
            r10: 0,
            r8: 0,
            r9: 0,
        };
        raw_syscall(&mut regs) as c_int
    }
}

pub fn default_listen(sockfd: c_int, backlog: c_int) -> c_int {
    unsafe {
        let mut regs = SyscallRegs {
            rax: 50, // SYS_listen
            rdi: sockfd as u64,
            rsi: backlog as u64,
            rdx: 0,
            r10: 0,
            r8: 0,
            r9: 0,
        };
        raw_syscall(&mut regs) as c_int
    }
}

pub fn default_ioctl(fd: c_int, request: c_ulong, arg: *mut c_void) -> c_int {
    unsafe {
        let mut regs = SyscallRegs {
            rax: 16, // SYS_ioctl
            rdi: fd as u64,
            rsi: request as u64,
            rdx: arg as u64,
            r10: 0,
            r8: 0,
            r9: 0,
        };
        raw_syscall(&mut regs) as c_int
    }
}

pub fn default_access(pathname: *const c_char, mode: c_int) -> c_int {
    unsafe {
        let mut regs = SyscallRegs {
            rax: 21, // SYS_access
            rdi: pathname as u64,
            rsi: mode as u64,
            rdx: 0,
            r10: 0,
            r8: 0,
            r9: 0,
        };
        raw_syscall(&mut regs) as c_int
    }
}

/// SyscallHooksを実装した型を登録するための内部処理
///
/// これはzpoline内部で使用されます。通常、ユーザーは`register_syscall_hooks`を
/// 使用してフックを登録します。
pub(crate) fn dispatch_syscall_hooks(
    hooks: &mut dyn SyscallHooks,
    regs: &mut SyscallRegs,
) -> i64 {
    match regs.rax {
        0 => hooks.hook_read(
            regs.rdi as c_int,
            regs.rsi as *mut c_void,
            regs.rdx as size_t,
        ) as i64,
        1 => hooks.hook_write(
            regs.rdi as c_int,
            regs.rsi as *const c_void,
            regs.rdx as size_t,
        ) as i64,
        2 => hooks.hook_open(
            regs.rdi as *const c_char,
            regs.rsi as c_int,
            regs.rdx as c_uint,
        ) as i64,
        3 => hooks.hook_close(regs.rdi as c_int) as i64,
        8 => hooks.hook_lseek(
            regs.rdi as c_int,
            regs.rsi as off_t,
            regs.rdx as c_int,
        ) as i64,
        9 => hooks.hook_mmap(
            regs.rdi as *mut c_void,
            regs.rsi as size_t,
            regs.rdx as c_int,
            regs.r10 as c_int,
            regs.r8 as c_int,
            regs.r9 as off_t,
        ) as i64,
        10 => hooks.hook_mprotect(
            regs.rdi as *mut c_void,
            regs.rsi as size_t,
            regs.rdx as c_int,
        ) as i64,
        11 => hooks.hook_munmap(regs.rdi as *mut c_void, regs.rsi as size_t) as i64,
        12 => hooks.hook_brk(regs.rdi as *mut c_void) as i64,
        16 => hooks.hook_ioctl(
            regs.rdi as c_int,
            regs.rsi as c_ulong,
            regs.rdx as *mut c_void,
        ) as i64,
        21 => hooks.hook_access(regs.rdi as *const c_char, regs.rsi as c_int) as i64,
        22 => hooks.hook_pipe(regs.rdi as *mut c_int) as i64,
        32 => hooks.hook_dup(regs.rdi as c_int) as i64,
        33 => hooks.hook_dup2(regs.rdi as c_int, regs.rsi as c_int) as i64,
        39 => hooks.hook_getpid() as i64,
        41 => hooks.hook_socket(
            regs.rdi as c_int,
            regs.rsi as c_int,
            regs.rdx as c_int,
        ) as i64,
        42 => hooks.hook_connect(
            regs.rdi as c_int,
            regs.rsi as *const c_void,
            regs.rdx as u32,
        ) as i64,
        43 => hooks.hook_accept(
            regs.rdi as c_int,
            regs.rsi as *mut c_void,
            regs.rdx as *mut u32,
        ) as i64,
        49 => hooks.hook_bind(
            regs.rdi as c_int,
            regs.rsi as *const c_void,
            regs.rdx as u32,
        ) as i64,
        50 => hooks.hook_listen(regs.rdi as c_int, regs.rsi as c_int) as i64,
        57 => hooks.hook_fork() as i64,
        59 => hooks.hook_execve(
            regs.rdi as *const c_char,
            regs.rsi as *const *const c_char,
            regs.rdx as *const *const c_char,
        ) as i64,
        60 => hooks.hook_exit(regs.rdi as c_int),
        61 => hooks.hook_wait4(
            regs.rdi as pid_t,
            regs.rsi as *mut c_int,
            regs.rdx as c_int,
            regs.r10 as *mut c_void,
        ) as i64,
        62 => hooks.hook_kill(regs.rdi as pid_t, regs.rsi as c_int) as i64,
        186 => hooks.hook_gettid() as i64,
        231 => hooks.hook_exit_group(regs.rdi as c_int),
        257 => hooks.hook_openat(
            regs.rdi as c_int,
            regs.rsi as *const c_char,
            regs.rdx as c_int,
            regs.r10 as c_uint,
        ) as i64,
        // 未知のsyscallはデフォルトで実行
        _ => unsafe { raw_syscall(regs) },
    }
}
