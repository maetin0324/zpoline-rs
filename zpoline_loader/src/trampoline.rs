/// トランポリンのエラー
#[derive(Debug)]
pub enum TrampolineError {
    MmapFailed(nix::Error),
}

impl std::fmt::Display for TrampolineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TrampolineError::MmapFailed(e) => write!(f, "mmap failed: {}", e),
        }
    }
}

impl std::error::Error for TrampolineError {}

/// 最大システムコール番号（Linux x86-64では約450程度）
/// 安全のため、もう少し大きめに確保
const MAX_SYSCALL_NR: usize = 512;

/// トランポリン全体のサイズ
/// callq *%raxはraxの値をそのままアドレスとして使うため、
/// syscall番号0-511の各バイトアドレスをカバーする必要がある
const TRAMPOLINE_SIZE: usize = MAX_SYSCALL_NR + 4096; // syscall番号分 + スタブ用

/// VA=0にトランポリンを生成
///
/// 構造:
/// - 0x0000 - 0xNNNN: NOP sled (各syscall番号に対応)
/// - 末尾: jmp命令でhook_entryへ
pub fn setup_trampoline() -> Result<(), TrampolineError> {
    // VA=0にメモリをマップ
    // MAP_FIXEDを使用して強制的に0番地に配置
    // nix 0.29では、mmap_anonymousの第一引数はOption<NonZeroUsize>
    // 0番地を指定するため、NonZeroUsizeは使えないので、
    // 代わりにlibc::mmapを直接呼び出す

    let addr = 0 as *mut std::ffi::c_void;
    let result = unsafe {
        libc::mmap(
            addr,
            TRAMPOLINE_SIZE,
            libc::PROT_READ | libc::PROT_WRITE | libc::PROT_EXEC,
            libc::MAP_FIXED | libc::MAP_ANONYMOUS | libc::MAP_PRIVATE,
            -1,
            0,
        )
    };

    if result == libc::MAP_FAILED {
        return Err(TrampolineError::MmapFailed(nix::Error::last()));
    }

    let mapped_ptr = result as *mut u8;

    // マップされたメモリをバイトスライスとして取得
    let trampoline_mem = unsafe { std::slice::from_raw_parts_mut(mapped_ptr, TRAMPOLINE_SIZE) };

    // トランポリンコードを生成
    generate_trampoline_code(trampoline_mem)?;

    Ok(())
}

/// トランポリンコードを生成
///
/// callq *%raxで飛んでくる際、raxの値（syscall番号）がそのままアドレスとして使われる。
/// 例: syscall番号1 → アドレス0x1, syscall番号39 → アドレス0x27
///
/// 戦略: 0-511の全バイトをNOPで埋める。任意のアドレスにジャンプしても、
/// NOP命令を実行し続けて最終的にスタブに到達する。
fn generate_trampoline_code(mem: &mut [u8]) -> Result<(), TrampolineError> {
    // スタブの位置（syscall番号の最大値の次）
    let stub_offset = MAX_SYSCALL_NR;

    // 0からstub_offset-1までをNOPで埋める
    // これにより、任意のsyscall番号（=アドレス）からスタートしても
    // NOP命令を実行し続けてスタブに到達できる
    for i in 0..stub_offset {
        mem[i] = 0x90; // NOP (1バイト命令)
    }

    // スタブコードを生成
    generate_hook_stub(&mut mem[stub_offset..])?;

    Ok(())
}

/// フックスタブを生成
///
/// このスタブはhook_entryを呼び出し、結果を返す。
/// 簡易実装のため、完全なレジスタ保存/復元は省略。
fn generate_hook_stub(mem: &mut [u8]) -> Result<(), TrampolineError> {
    // hook_entry関数のアドレスを取得
    let hook_entry_addr = zpoline_hook_api::hook_entry as usize;

    // 以下のコードを生成:
    // レジスタをSyscallRegs構造体のメモリレイアウトに合わせてスタックに積む
    // struct SyscallRegs { rax, rdi, rsi, rdx, r10, r8, r9 }
    // スタックは高位→低位に成長するため、逆順にpush: r9, r8, r10, rdx, rsi, rdi, rax
    //
    // スタックアライメント:
    // callq *%raxでリターンアドレス(8B)がpushされている
    // 7個のレジスタ(56B)をpushする
    // 合計64B → 16バイトアラインのため8Bのパディングが必要

    let mut offset = 0;

    // レジスタを逆順にpush（r9から始めてraxで終わる）
    // これによりスタック上で rsp+0:rax, rsp+8:rdi, ... となる

    // push r9
    mem[offset] = 0x41;
    mem[offset + 1] = 0x51;
    offset += 2;

    // push r8
    mem[offset] = 0x41;
    mem[offset + 1] = 0x50;
    offset += 2;

    // push r10
    mem[offset] = 0x41;
    mem[offset + 1] = 0x52;
    offset += 2;

    // push rdx
    mem[offset] = 0x52;
    offset += 1;

    // push rsi
    mem[offset] = 0x56;
    offset += 1;

    // push rdi
    mem[offset] = 0x57;
    offset += 1;

    // push rax
    mem[offset] = 0x50;
    offset += 1;

    // スタックアライメント調整: 8バイト減算（パディング）
    // sub rsp, 8
    mem[offset] = 0x48;
    mem[offset + 1] = 0x83;
    mem[offset + 2] = 0xec;
    mem[offset + 3] = 0x08;
    offset += 4;

    // mov rdi, rsp (第一引数としてスタックポインタ = &mut SyscallRegsを渡す)
    // ただし、パディングの8バイト分を加算
    // lea rdi, [rsp + 8]
    mem[offset] = 0x48;
    mem[offset + 1] = 0x8d;
    mem[offset + 2] = 0x7c;
    mem[offset + 3] = 0x24;
    mem[offset + 4] = 0x08;
    offset += 5;

    // movabs r11, hook_entry_addr (r11を使用してraxを保護)
    mem[offset] = 0x49;
    mem[offset + 1] = 0xbb;
    let addr_bytes = hook_entry_addr.to_le_bytes();
    mem[offset + 2..offset + 10].copy_from_slice(&addr_bytes);
    offset += 10;

    // call r11
    mem[offset] = 0x41;
    mem[offset + 1] = 0xff;
    mem[offset + 2] = 0xd3;
    offset += 3;

    // パディングを解除
    // add rsp, 8
    mem[offset] = 0x48;
    mem[offset + 1] = 0x83;
    mem[offset + 2] = 0xc4;
    mem[offset + 3] = 0x08;
    offset += 4;

    // hook_entryの戻り値（rax）はシステムコールの戻り値
    // スタックからレジスタを復元（raxは復元しない - 戻り値として使う）

    // add rsp, 8 (raxをスキップ)
    mem[offset] = 0x48;
    mem[offset + 1] = 0x83;
    mem[offset + 2] = 0xc4;
    mem[offset + 3] = 0x08;
    offset += 4;

    // pop rdi
    mem[offset] = 0x5f;
    offset += 1;

    // pop rsi
    mem[offset] = 0x5e;
    offset += 1;

    // pop rdx
    mem[offset] = 0x5a;
    offset += 1;

    // pop r10
    mem[offset] = 0x41;
    mem[offset + 1] = 0x5a;
    offset += 2;

    // pop r8
    mem[offset] = 0x41;
    mem[offset + 1] = 0x58;
    offset += 2;

    // pop r9
    mem[offset] = 0x41;
    mem[offset + 1] = 0x59;
    offset += 2;

    // ret (callq *%raxで積まれたリターンアドレスに戻る)
    mem[offset] = 0xc3;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trampoline_constants() {
        assert!(MAX_SYSCALL_NR > 400);
        assert!(TRAMPOLINE_SIZE > MAX_SYSCALL_NR);
    }
}
