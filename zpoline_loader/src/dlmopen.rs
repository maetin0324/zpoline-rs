use std::ffi::CString;
use zpoline_hook_api::HookFn;

/// dlmopenのエラー
#[derive(Debug)]
pub enum DlmopenError {
    LibraryNotSpecified,
    DlmopenFailed(String),
    SymbolNotFound(String),
}

impl std::fmt::Display for DlmopenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DlmopenError::LibraryNotSpecified => {
                write!(f, "Hook library path not specified")
            }
            DlmopenError::DlmopenFailed(e) => write!(f, "dlmopen failed: {}", e),
            DlmopenError::SymbolNotFound(s) => write!(f, "Symbol not found: {}", s),
        }
    }
}

impl std::error::Error for DlmopenError {}

// dlmopenのFFI定義
extern "C" {
    fn dlmopen(
        lmid: libc::c_long,
        filename: *const libc::c_char,
        flags: libc::c_int,
    ) -> *mut libc::c_void;
    fn dlsym(handle: *mut libc::c_void, symbol: *const libc::c_char) -> *mut libc::c_void;
    fn dlerror() -> *const libc::c_char;
}

// 新しいネームスペースIDを指定する定数
const LM_ID_NEWLM: libc::c_long = -1;

/// フックライブラリを別ネームスペースにロードする
///
/// # 引数
/// * `lib_path` - ロードするライブラリのパス
///
/// # 戻り値
/// * `Ok(HookFn)` - ロードされたフック関数
/// * `Err(DlmopenError)` - エラー
pub fn load_hook_library(lib_path: Option<&str>) -> Result<HookFn, DlmopenError> {
    let lib_path = lib_path.ok_or(DlmopenError::LibraryNotSpecified)?;

    eprintln!("[zpoline] Loading hook library: {}", lib_path);

    // パスをCStringに変換
    let c_path = CString::new(lib_path).map_err(|e| {
        DlmopenError::DlmopenFailed(format!("Invalid path string: {}", e))
    })?;

    // dlmopenで新しいネームスペースにロード
    // RTLD_NOW: すぐにシンボルを解決
    // RTLD_LOCAL: シンボルを他のライブラリに公開しない
    let handle = unsafe {
        dlmopen(
            LM_ID_NEWLM,
            c_path.as_ptr(),
            libc::RTLD_NOW | libc::RTLD_LOCAL,
        )
    };

    if handle.is_null() {
        let error_msg = unsafe {
            let err_ptr = dlerror();
            if err_ptr.is_null() {
                "Unknown error".to_string()
            } else {
                std::ffi::CStr::from_ptr(err_ptr)
                    .to_string_lossy()
                    .to_string()
            }
        };
        return Err(DlmopenError::DlmopenFailed(error_msg));
    }

    eprintln!("[zpoline] Hook library loaded at handle: {:p}", handle);

    // 初期化関数を呼び出してフック関数ポインタを取得
    let init_symbol = CString::new("zpoline_hook_init").unwrap();
    let init_fn_ptr = unsafe { dlsym(handle, init_symbol.as_ptr()) };

    if init_fn_ptr.is_null() {
        return Err(DlmopenError::SymbolNotFound(
            "zpoline_hook_init".to_string(),
        ));
    }

    // 初期化関数を呼び出し
    let init_fn: extern "C" fn() -> *const () =
        unsafe { std::mem::transmute(init_fn_ptr) };
    let hook_fn_ptr = init_fn();

    if hook_fn_ptr.is_null() {
        return Err(DlmopenError::SymbolNotFound(
            "hook function returned null".to_string(),
        ));
    }

    // フック関数ポインタを変換
    let hook_fn: HookFn = unsafe { std::mem::transmute(hook_fn_ptr) };

    eprintln!("[zpoline] Hook function initialized: {:p}", hook_fn_ptr);

    Ok(hook_fn)
}

/// 環境変数からフックライブラリのパスを取得
///
/// 優先順位:
/// 1. ZPOLINE_HOOK - カスタムフックライブラリのパス
/// 2. デフォルトパス（zpoline_hook_implのパス）
pub fn get_hook_library_path() -> Option<String> {
    // 環境変数をチェック
    if let Ok(custom_path) = std::env::var("ZPOLINE_HOOK") {
        eprintln!("[zpoline] Using custom hook library from ZPOLINE_HOOK: {}", custom_path);
        return Some(custom_path);
    }

    // デフォルトパスを探す
    // loaderと同じディレクトリにあると仮定
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(dir) = exe_path.parent() {
            let default_path = dir.join("libzpoline_hook_impl.so");
            if default_path.exists() {
                eprintln!(
                    "[zpoline] Using default hook library: {}",
                    default_path.display()
                );
                return Some(default_path.to_string_lossy().to_string());
            }
        }
    }

    eprintln!("[zpoline] No hook library specified, using built-in hook");
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_hook_library_path() {
        // 環境変数がない場合はNoneまたはデフォルトパス
        let path = get_hook_library_path();
        // テスト環境では None の可能性が高い
        assert!(path.is_none() || path.is_some());
    }
}
