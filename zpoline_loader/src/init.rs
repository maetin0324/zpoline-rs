use zpoline_rewriter::{parse_proc_maps, RewriteConfig, RewriteError, RewriteStats, Rewriter};
use std::path::PathBuf;

/// システムコール命令の書き換えを実行
pub fn rewrite_syscalls() -> Result<RewriteStats, RewriteError> {
    // /proc/self/mapsから実行可能な領域を取得
    let regions = parse_proc_maps().map_err(|e| {
        RewriteError::Other(format!("Failed to parse /proc/self/maps: {}", e))
    })?;

    // 除外リストの設定
    let config = build_rewrite_config();

    // 書き換え器を作成
    let mut rewriter = Rewriter::new(config);

    // 各実行可能領域を書き換え
    for region in regions {
        if !region.is_executable() {
            continue;
        }

        // vdsoとvsyscallは書き換えない
        if let Some(ref path) = region.pathname {
            let path_str = path.to_string_lossy();
            if path_str.contains("[vdso]") || path_str.contains("[vsyscall]") {
                eprintln!(
                    "[zpoline]   Skipping {} (special kernel region)",
                    path_str
                );
                continue;
            }
        }

        match rewriter.rewrite_region(&region) {
            Ok(count) => {
                if count > 0 {
                    eprintln!(
                        "[zpoline]   Rewritten {} syscalls in {:?} (0x{:x}-0x{:x})",
                        count,
                        region.pathname.as_ref().unwrap_or(&PathBuf::from("[anonymous]")),
                        region.start,
                        region.end
                    );
                }
            }
            Err(e) => {
                // エラーがあっても続行（一部の領域で失敗しても他は成功する可能性がある）
                eprintln!(
                    "[zpoline]   Warning: Failed to rewrite region {:?}: {}",
                    region.pathname.as_ref().unwrap_or(&PathBuf::from("[anonymous]")),
                    e
                );
            }
        }
    }

    Ok(rewriter.stats().clone())
}

/// 書き換え設定を構築
fn build_rewrite_config() -> RewriteConfig {
    let mut config = RewriteConfig::new();

    // 自分自身（zpoline_loader）を除外
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(parent) = exe_path.parent() {
            // zpoline_loaderライブラリを除外
            let loader_path = parent.join("libzpoline_loader.so");
            config = config.exclude_path(loader_path);
        }
    }

    // zpoline_hook_apiの関数が含まれる領域を除外
    // これらの関数にはraw_syscallが含まれているため、書き換えてはいけない
    let hook_api_start = zpoline_hook_api::raw_syscall as usize;
    let hook_api_end = hook_api_start + 4096; // 関数サイズの概算
    config = config.exclude_range(hook_api_start, hook_api_end);

    // VA=0のトランポリン領域を除外
    config = config.exclude_range(0, 65536);

    // vDSO領域を除外（通常は[vdso]という名前）
    // これは/proc/self/mapsのパース時に判定する

    // 環境変数からの追加除外パス
    if let Ok(exclude_paths) = std::env::var("ZPOLINE_EXCLUDE") {
        for path in exclude_paths.split(':') {
            if !path.is_empty() {
                config = config.exclude_path(PathBuf::from(path));
            }
        }
    }

    config
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_config() {
        let config = build_rewrite_config();
        // 少なくとも何らかの除外設定がされているはず
        assert!(config.exclude_ranges.len() > 0);
    }
}
