use std::cell::Cell;
use std::sync::atomic::{AtomicPtr, Ordering};
use std::sync::Mutex;

pub mod syscall_hooks;

pub use syscall_hooks::SyscallHooks;

/// システムコールのレジスタ状態
/// x86-64のシステムコール呼び出し規約に従う
/// syscall番号: rax
/// 引数: rdi, rsi, rdx, r10, r8, r9
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SyscallRegs {
    pub rax: u64, // syscall番号
    pub rdi: u64, // 第1引数
    pub rsi: u64, // 第2引数
    pub rdx: u64, // 第3引数
    pub r10: u64, // 第4引数
    pub r8: u64,  // 第5引数
    pub r9: u64,  // 第6引数
}

impl SyscallRegs {
    /// 新しいレジスタ状態を作成
    pub fn new(rax: u64, rdi: u64, rsi: u64, rdx: u64, r10: u64, r8: u64, r9: u64) -> Self {
        Self {
            rax,
            rdi,
            rsi,
            rdx,
            r10,
            r8,
            r9,
        }
    }

    /// ゼロで初期化されたレジスタ状態
    pub fn zero() -> Self {
        Self {
            rax: 0,
            rdi: 0,
            rsi: 0,
            rdx: 0,
            r10: 0,
            r8: 0,
            r9: 0,
        }
    }
}

/// フック関数の型
/// 引数: レジスタ状態
/// 戻り値: システムコールの戻り値（負の値はエラー）
pub type HookFn = extern "C" fn(&mut SyscallRegs) -> i64;

/// デフォルトのフック関数（何もせずに元のsyscallにフォールバック）
#[no_mangle]
pub extern "C" fn default_hook(regs: &mut SyscallRegs) -> i64 {
    unsafe { raw_syscall(regs) }
}

/// グローバルなフック関数ポインタ
static HOOK_FUNCTION: AtomicPtr<()> = AtomicPtr::new(default_hook as *mut ());

/// SyscallHooksを実装した型のグローバルな保持
/// Mutexで保護されたBox<dyn SyscallHooks>
static HOOK_TRAIT_OBJECT: Mutex<Option<Box<dyn SyscallHooks>>> = Mutex::new(None);

/// フック関数を設定
#[no_mangle]
pub extern "C" fn __hook_init(hook_fn: HookFn) {
    HOOK_FUNCTION.store(hook_fn as *mut (), Ordering::SeqCst);
}

/// フック関数を取得
pub fn get_hook_fn() -> HookFn {
    let ptr = HOOK_FUNCTION.load(Ordering::SeqCst);
    unsafe { std::mem::transmute(ptr) }
}

// TLSによる再入ガード
thread_local! {
    static IN_HOOK: Cell<bool> = const { Cell::new(false) };
}

static HOOK_ENTRY_CALL_COUNT: AtomicUsize = AtomicUsize::new(0);

/// フックエントリポイント
/// これはVA=0トランポリンから呼ばれる
#[no_mangle]
pub extern "C" fn hook_entry(regs: &mut SyscallRegs) -> i64 {
    // デバッグ用: hook_entryが呼ばれたことを記録
    HOOK_ENTRY_CALL_COUNT.fetch_add(1, Ordering::Relaxed);

    // 再入チェック
    if IN_HOOK.with(|in_hook| {
        if in_hook.get() {
            true
        } else {
            in_hook.set(true);
            false
        }
    }) {
        // 再入検出 - 元のsyscallを直接実行
        let result = unsafe { raw_syscall(regs) };
        return result;
    }

    // フック関数を呼び出し
    let hook_fn = get_hook_fn();
    let result = hook_fn(regs);

    // フラグをリセット
    IN_HOOK.with(|in_hook| in_hook.set(false));

    result
}

/// デバッグ用: hook_entryが呼ばれた回数を取得
#[no_mangle]
pub extern "C" fn get_hook_entry_call_count() -> usize {
    HOOK_ENTRY_CALL_COUNT.load(Ordering::Relaxed)
}

/// raw syscallの実装
/// この関数は書き換え対象外のページに配置する必要がある
///
/// 注意: この実装は簡易版です。実際には専用のページに配置した
/// syscall命令を使用するか、asmマクロで実装する必要があります。
#[no_mangle]
pub unsafe extern "C" fn raw_syscall(regs: &SyscallRegs) -> i64 {
    raw_syscall_impl(
        regs.rax, regs.rdi, regs.rsi, regs.rdx, regs.r10, regs.r8, regs.r9,
    )
}

/// 実際のsyscall命令を含む関数
/// この関数は別途書き換え除外リストに追加される必要がある
#[inline(never)]
unsafe fn raw_syscall_impl(
    nr: u64,
    arg1: u64,
    arg2: u64,
    arg3: u64,
    arg4: u64,
    arg5: u64,
    arg6: u64,
) -> i64 {
    let ret: i64;
    core::arch::asm!(
        "syscall",
        inlateout("rax") nr => ret,
        in("rdi") arg1,
        in("rsi") arg2,
        in("rdx") arg3,
        in("r10") arg4,
        in("r8") arg5,
        in("r9") arg6,
        lateout("rcx") _,  // syscallはrcxを破壊する
        lateout("r11") _,  // syscallはr11を破壊する
        options(nostack)
    );
    ret
}

/// raw syscallのバイパス（C ABIバージョン）
/// 引数を個別に受け取る
#[no_mangle]
pub unsafe extern "C" fn raw_syscall_bypass(
    nr: u64,
    arg1: u64,
    arg2: u64,
    arg3: u64,
    arg4: u64,
    arg5: u64,
    arg6: u64,
) -> i64 {
    raw_syscall_impl(nr, arg1, arg2, arg3, arg4, arg5, arg6)
}

/// 便利な関数: 再入ガードの状態を取得
pub fn is_in_hook() -> bool {
    IN_HOOK.with(|in_hook| in_hook.get())
}

/// SyscallHooksトレイトを実装した型を登録する
///
/// この関数は、traitベースのフック機構を有効にします。
/// 一度登録されると、システムコールはSyscallHooksのメソッドにディスパッチされます。
///
/// # 使用例
///
/// ```no_run
/// use zpoline_hook_api::{SyscallHooks, register_syscall_hooks};
///
/// struct MyHooks;
///
/// impl SyscallHooks for MyHooks {
///     fn hook_write(&mut self, fd: i32, buf: *const u8, count: usize) -> isize {
///         eprintln!("[CUSTOM] write called");
///         Self::default_write(fd, buf as *const _, count)
///     }
/// }
///
/// fn main() {
///     register_syscall_hooks(MyHooks);
///     // これ以降、システムコールはMyHooksにディスパッチされる
/// }
/// ```
pub fn register_syscall_hooks<T: SyscallHooks>(hooks: T) {
    // SyscallHooksをグローバルに保存
    let mut guard = HOOK_TRAIT_OBJECT.lock().unwrap();
    *guard = Some(Box::new(hooks));
    drop(guard); // 明示的にロックを解放

    // フック関数として trait_based_hook を設定
    __hook_init(trait_based_hook);
}

use std::sync::atomic::AtomicUsize;
static TRAIT_HOOK_CALL_COUNT: AtomicUsize = AtomicUsize::new(0);

/// traitベースのフック関数
/// SyscallHooksトレイトのメソッドにディスパッチする
extern "C" fn trait_based_hook(regs: &mut SyscallRegs) -> i64 {
    // デバッグ用: この関数が呼ばれたことを記録
    TRAIT_HOOK_CALL_COUNT.fetch_add(1, Ordering::Relaxed);

    // HOOK_TRAIT_OBJECTからフックオブジェクトを取得
    let mut guard = match HOOK_TRAIT_OBJECT.lock() {
        Ok(g) => g,
        Err(_e) => {
            // ロック失敗時はデフォルトのsyscallを実行
            return unsafe { raw_syscall(regs) };
        }
    };

    if let Some(ref mut hooks) = *guard {
        syscall_hooks::dispatch_syscall_hooks(hooks.as_mut(), regs)
    } else {
        // フックが登録されていない場合はデフォルトのsyscallを実行
        unsafe { raw_syscall(regs) }
    }
}

/// デバッグ用: trait_based_hookが呼ばれた回数を取得
#[no_mangle]
pub extern "C" fn get_trait_hook_call_count() -> usize {
    TRAIT_HOOK_CALL_COUNT.load(Ordering::Relaxed)
}

/// 登録されたSyscallHooksをディスパッチするフック関数
///
/// この関数は、カスタムフックライブラリの`zpoline_hook_init()`から
/// 返すことができます。事前に`register_syscall_hooks()`を呼び出して
/// フックを登録しておく必要があります。
///
/// # 使用例
///
/// ```no_run
/// use zpoline_hook_api::{SyscallHooks, register_syscall_hooks, get_trait_dispatch_hook};
/// use ctor::ctor;
///
/// struct MyHooks;
/// impl SyscallHooks for MyHooks { /* ... */ }
///
/// #[ctor]
/// fn init() {
///     register_syscall_hooks(MyHooks);
/// }
///
/// #[no_mangle]
/// pub extern "C" fn zpoline_hook_init() -> *const () {
///     get_trait_dispatch_hook()
/// }
/// ```
#[no_mangle]
pub extern "C" fn get_trait_dispatch_hook() -> *const () {
    trait_based_hook as *const ()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_syscall_regs() {
        let regs = SyscallRegs::new(1, 2, 3, 4, 5, 6, 7);
        assert_eq!(regs.rax, 1);
        assert_eq!(regs.rdi, 2);
        assert_eq!(regs.rsi, 3);
    }

    #[test]
    fn test_hook_function_ptr() {
        extern "C" fn test_hook(_regs: &mut SyscallRegs) -> i64 {
            42
        }

        __hook_init(test_hook);
        let hook_fn = get_hook_fn();

        let mut regs = SyscallRegs::zero();
        let result = hook_fn(&mut regs);
        assert_eq!(result, 42);
    }

    #[test]
    fn test_reentry_guard() {
        assert!(!is_in_hook());
        IN_HOOK.with(|in_hook| in_hook.set(true));
        assert!(is_in_hook());
        IN_HOOK.with(|in_hook| in_hook.set(false));
        assert!(!is_in_hook());
    }
}
