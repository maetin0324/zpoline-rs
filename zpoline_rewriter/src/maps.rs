use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

/// メモリ領域の情報
#[derive(Debug, Clone, PartialEq)]
pub struct MemoryRegion {
    /// 開始アドレス
    pub start: usize,
    /// 終了アドレス
    pub end: usize,
    /// 読み込み可能
    pub readable: bool,
    /// 書き込み可能
    pub writable: bool,
    /// 実行可能
    pub executable: bool,
    /// プライベートマッピング
    pub private: bool,
    /// オフセット
    pub offset: u64,
    /// デバイス
    pub device: String,
    /// inode
    pub inode: u64,
    /// パス名
    pub pathname: Option<PathBuf>,
}

impl MemoryRegion {
    /// この領域が実行可能かどうか
    pub fn is_executable(&self) -> bool {
        self.executable
    }

    /// この領域が書き込み可能かどうか
    pub fn is_writable(&self) -> bool {
        self.writable
    }

    /// 領域のサイズ
    pub fn size(&self) -> usize {
        self.end - self.start
    }
}

/// /proc/self/maps をパースして実行可能な領域を返す
pub fn parse_proc_maps() -> std::io::Result<Vec<MemoryRegion>> {
    parse_maps_file("/proc/self/maps")
}

/// 指定されたmapsファイルをパース
pub fn parse_maps_file(path: &str) -> std::io::Result<Vec<MemoryRegion>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut regions = Vec::new();

    for line in reader.lines() {
        let line = line?;
        if let Some(region) = parse_maps_line(&line) {
            regions.push(region);
        }
    }

    Ok(regions)
}

/// maps の1行をパース
/// フォーマット: address perms offset dev inode pathname
/// 例: 7f8b4c000000-7f8b4c021000 r-xp 00000000 08:01 1234 /lib/x86_64-linux-gnu/libc.so.6
fn parse_maps_line(line: &str) -> Option<MemoryRegion> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.is_empty() {
        return None;
    }

    // アドレス範囲のパース
    let addr_parts: Vec<&str> = parts[0].split('-').collect();
    if addr_parts.len() != 2 {
        return None;
    }

    let start = usize::from_str_radix(addr_parts[0], 16).ok()?;
    let end = usize::from_str_radix(addr_parts[1], 16).ok()?;

    if parts.len() < 5 {
        return None;
    }

    // パーミッションのパース
    let perms = parts[1];
    if perms.len() != 4 {
        return None;
    }

    let readable = perms.chars().nth(0)? == 'r';
    let writable = perms.chars().nth(1)? == 'w';
    let executable = perms.chars().nth(2)? == 'x';
    let private = perms.chars().nth(3)? == 'p';

    // オフセット
    let offset = u64::from_str_radix(parts[2], 16).ok()?;

    // デバイス
    let device = parts[3].to_string();

    // inode
    let inode = parts[4].parse().ok()?;

    // パス名（オプション）
    let pathname = if parts.len() > 5 {
        Some(PathBuf::from(parts[5..].join(" ")))
    } else {
        None
    };

    Some(MemoryRegion {
        start,
        end,
        readable,
        writable,
        executable,
        private,
        offset,
        device,
        inode,
        pathname,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_maps_line() {
        let line = "7f8b4c000000-7f8b4c021000 r-xp 00000000 08:01 1234 /lib/x86_64-linux-gnu/libc.so.6";
        let region = parse_maps_line(line).unwrap();

        assert_eq!(region.start, 0x7f8b4c000000);
        assert_eq!(region.end, 0x7f8b4c021000);
        assert_eq!(region.readable, true);
        assert_eq!(region.writable, false);
        assert_eq!(region.executable, true);
        assert_eq!(region.private, true);
        assert_eq!(region.offset, 0);
        assert_eq!(region.device, "08:01");
        assert_eq!(region.inode, 1234);
        assert_eq!(
            region.pathname,
            Some(PathBuf::from("/lib/x86_64-linux-gnu/libc.so.6"))
        );
    }

    #[test]
    fn test_parse_anonymous_mapping() {
        let line = "7ffd1234000-7ffd1235000 rw-p 00000000 00:00 0";
        let region = parse_maps_line(line).unwrap();

        assert_eq!(region.start, 0x7ffd1234000);
        assert_eq!(region.end, 0x7ffd1235000);
        assert_eq!(region.readable, true);
        assert_eq!(region.writable, true);
        assert_eq!(region.executable, false);
        assert_eq!(region.pathname, None);
    }
}
