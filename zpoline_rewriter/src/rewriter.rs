use crate::maps::MemoryRegion;
use iced_x86::{Decoder, DecoderOptions, Mnemonic};
use nix::sys::mman::{mprotect, ProtFlags};
use std::collections::HashSet;
use std::path::PathBuf;
use std::ptr::NonNull;
use std::slice;

/// 書き換えエラー
#[derive(Debug)]
pub enum RewriteError {
    /// メモリ保護の変更に失敗
    ProtectError(nix::Error),
    /// デコードエラー
    DecodeError(String),
    /// その他のエラー
    Other(String),
}

impl std::fmt::Display for RewriteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RewriteError::ProtectError(e) => write!(f, "Memory protection error: {}", e),
            RewriteError::DecodeError(s) => write!(f, "Decode error: {}", s),
            RewriteError::Other(s) => write!(f, "Error: {}", s),
        }
    }
}

impl std::error::Error for RewriteError {}

/// 書き換え設定
#[derive(Debug, Clone)]
pub struct RewriteConfig {
    /// 除外するパスのリスト（このパスを含む領域は書き換えない）
    pub exclude_paths: HashSet<PathBuf>,
    /// 除外するアドレス範囲のリスト
    pub exclude_ranges: Vec<(usize, usize)>,
    /// ドライランモード（実際には書き換えない）
    pub dry_run: bool,
}

impl Default for RewriteConfig {
    fn default() -> Self {
        Self {
            exclude_paths: HashSet::new(),
            exclude_ranges: Vec::new(),
            dry_run: false,
        }
    }
}

impl RewriteConfig {
    /// 新しい設定を作成
    pub fn new() -> Self {
        Self::default()
    }

    /// 除外パスを追加
    pub fn exclude_path(mut self, path: PathBuf) -> Self {
        self.exclude_paths.insert(path);
        self
    }

    /// 除外範囲を追加
    pub fn exclude_range(mut self, start: usize, end: usize) -> Self {
        self.exclude_ranges.push((start, end));
        self
    }

    /// ドライランモードを設定
    pub fn dry_run(mut self, enabled: bool) -> Self {
        self.dry_run = enabled;
        self
    }

    /// 指定された領域が除外対象かチェック
    pub fn is_excluded(&self, region: &MemoryRegion) -> bool {
        // パスによる除外チェック
        if let Some(ref path) = region.pathname {
            for exclude_path in &self.exclude_paths {
                if path.starts_with(exclude_path) || path == exclude_path {
                    return true;
                }
            }
        }

        // アドレス範囲による除外チェック
        for (exclude_start, exclude_end) in &self.exclude_ranges {
            if region.start < *exclude_end && region.end > *exclude_start {
                return true;
            }
        }

        false
    }
}

/// 書き換え統計情報
#[derive(Debug, Default, Clone)]
pub struct RewriteStats {
    /// 検査した領域の数
    pub regions_scanned: usize,
    /// 書き換えた領域の数
    pub regions_rewritten: usize,
    /// 置換したsyscall命令の数
    pub syscalls_replaced: usize,
    /// 置換したsysenter命令の数
    pub sysenters_replaced: usize,
    /// スキップした領域の数
    pub regions_skipped: usize,
}

/// システムコール書き換え器
pub struct Rewriter {
    config: RewriteConfig,
    stats: RewriteStats,
}

impl Rewriter {
    /// 新しい書き換え器を作成
    pub fn new(config: RewriteConfig) -> Self {
        Self {
            config,
            stats: RewriteStats::default(),
        }
    }

    /// 指定されたメモリ領域を書き換え
    pub fn rewrite_region(&mut self, region: &MemoryRegion) -> Result<usize, RewriteError> {
        self.stats.regions_scanned += 1;

        // 実行可能でない領域はスキップ
        if !region.is_executable() {
            return Ok(0);
        }

        // 除外対象の領域はスキップ
        if self.config.is_excluded(region) {
            self.stats.regions_skipped += 1;
            return Ok(0);
        }

        let size = region.size();
        let ptr = region.start as *mut u8;

        // 安全性: メモリ領域が有効であることを前提とする
        // /proc/self/mapsから取得した領域なので通常は安全
        let code_slice = unsafe { slice::from_raw_parts(ptr, size) };

        // 書き換え対象の位置を収集
        let replacements = self.find_syscalls(region.start, code_slice)?;

        if replacements.is_empty() {
            return Ok(0);
        }

        // ドライランモードでは実際の書き換えをスキップ
        if self.config.dry_run {
            self.stats.syscalls_replaced += replacements.len();
            return Ok(replacements.len());
        }

        // メモリ保護を一時的に変更（RWX）
        let original_prot = if region.is_writable() {
            ProtFlags::PROT_READ | ProtFlags::PROT_WRITE | ProtFlags::PROT_EXEC
        } else {
            ProtFlags::PROT_READ | ProtFlags::PROT_EXEC
        };

        let page_size = page_size::get();
        let aligned_start = (region.start / page_size) * page_size;
        let aligned_size = ((region.end - aligned_start + page_size - 1) / page_size) * page_size;

        // 書き込み可能にする
        unsafe {
            let ptr = NonNull::new(aligned_start as *mut std::ffi::c_void)
                .ok_or_else(|| RewriteError::Other("Invalid pointer".to_string()))?;
            mprotect(
                ptr,
                aligned_size,
                ProtFlags::PROT_READ | ProtFlags::PROT_WRITE | ProtFlags::PROT_EXEC,
            )
            .map_err(RewriteError::ProtectError)?;
        }

        // 書き換え実行
        let mut replaced_count = 0;
        let code_slice_mut = unsafe { slice::from_raw_parts_mut(ptr, size) };

        for (offset, replacement_type) in replacements {
            // syscall (0x0f 0x05) または sysenter (0x0f 0x34) を
            // callq *%rax (0xff 0xd0) に置換
            if code_slice_mut[offset] == 0x0f
                && (code_slice_mut[offset + 1] == 0x05 || code_slice_mut[offset + 1] == 0x34)
            {
                code_slice_mut[offset] = 0xff;
                code_slice_mut[offset + 1] = 0xd0;
                replaced_count += 1;

                match replacement_type {
                    SyscallType::Syscall => self.stats.syscalls_replaced += 1,
                    SyscallType::Sysenter => self.stats.sysenters_replaced += 1,
                }
            }
        }

        // メモリ保護を元に戻す
        unsafe {
            let ptr = NonNull::new(aligned_start as *mut std::ffi::c_void)
                .ok_or_else(|| RewriteError::Other("Invalid pointer".to_string()))?;
            mprotect(ptr, aligned_size, original_prot)
                .map_err(RewriteError::ProtectError)?;
        }

        if replaced_count > 0 {
            self.stats.regions_rewritten += 1;
        }

        Ok(replaced_count)
    }

    /// コード内のsyscall/sysenter命令を検出
    fn find_syscalls(
        &self,
        base_addr: usize,
        code: &[u8],
    ) -> Result<Vec<(usize, SyscallType)>, RewriteError> {
        let mut decoder = Decoder::with_ip(64, code, base_addr as u64, DecoderOptions::NONE);
        let mut replacements = Vec::new();

        while decoder.can_decode() {
            let instr = decoder.decode();

            match instr.mnemonic() {
                Mnemonic::Syscall => {
                    let offset = (instr.ip() as usize) - base_addr;
                    replacements.push((offset, SyscallType::Syscall));
                }
                Mnemonic::Sysenter => {
                    let offset = (instr.ip() as usize) - base_addr;
                    replacements.push((offset, SyscallType::Sysenter));
                }
                _ => {}
            }
        }

        Ok(replacements)
    }

    /// 統計情報を取得
    pub fn stats(&self) -> &RewriteStats {
        &self.stats
    }

    /// 統計情報をリセット
    pub fn reset_stats(&mut self) {
        self.stats = RewriteStats::default();
    }
}

/// システムコール命令の種類
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SyscallType {
    Syscall,
    Sysenter,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_builder() {
        let config = RewriteConfig::new()
            .exclude_path(PathBuf::from("/lib/libc.so"))
            .exclude_range(0x1000, 0x2000)
            .dry_run(true);

        assert!(config.exclude_paths.contains(&PathBuf::from("/lib/libc.so")));
        assert_eq!(config.exclude_ranges.len(), 1);
        assert!(config.dry_run);
    }

    #[test]
    fn test_find_syscall_in_code() {
        // syscall命令を含むコード: 0x0f 0x05
        let code = vec![
            0x48, 0xc7, 0xc0, 0x01, 0x00, 0x00, 0x00, // mov rax, 1
            0x0f, 0x05, // syscall
            0xc3, // ret
        ];

        let config = RewriteConfig::new();
        let rewriter = Rewriter::new(config);
        let replacements = rewriter.find_syscalls(0x1000, &code).unwrap();

        assert_eq!(replacements.len(), 1);
        assert_eq!(replacements[0].0, 7); // syscallのオフセット
        assert_eq!(replacements[0].1, SyscallType::Syscall);
    }
}
