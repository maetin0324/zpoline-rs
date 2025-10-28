# Rust版 zpoline 再実装

> 要旨：`syscall/sysenter` を **`callq *%rax`**（2バイト）に置換し、**VA=0**（仮想アドレス0）に配置したトランポリンへ制御を飛ばす——このzpolineの骨子をRustで実装しました。`LD_PRELOAD` で初期化→**トランポリン生成**→**コード書き換え**→**フック実行**という流れを `cdylib`＋FFIと命令デコーダ（`iced-x86`）で構成しています。

## 実装状況

**✅ 基本実装完了（2025-10-28）**

- ✅ zpoline_loader（cdylib, LD_PRELOAD対応）
- ✅ zpoline_rewriter（iced-x86による命令デコードと書き換え）
- ✅ zpoline_hook_api（フックABI、raw syscall、TLS再入ガード）
- ✅ サンプルプログラム（システムコールトレーサー）
- ✅ ドキュメント（USAGE.md、実装進捗レポート）

詳細は [USAGE.md](USAGE.md) および [.claude_log/implementation_progress.md](.claude_log/implementation_progress.md) を参照してください。

## クイックスタート

```bash
# ビルド
cargo build --release

# VA=0設定（初回のみ、要sudo）
sudo sysctl -w vm.mmap_min_addr=0

# サンプル実行
LD_PRELOAD=./target/release/libzpoline_loader.so ./target/release/zpoline_samples
```

---

## 1. 目的・非機能要件

* 目的

  * **低オーバーヘッド**かつ**網羅的（exhaustive）**にシステムコールをフックし、任意のユーザー空間実装（例：ユーザー空間ファイルシステム／ネットワーク）へ透過的に差し替える。([GitHub][1])
* 成果物

  * Rust製ライブラリ群（`zpoline_loader_rs`・`zpoline_hook_api`・`zpoline_rewriter_rs`）と最小サンプル。
* 非機能要件

  * **x86-64 Linux**、glibc/musl両対応（優先度：glibc→musl）。
  * root不要だが、**mmap_min_addr=0** 設定が必要（SELinuxの例外設定を含む運用注意）。([GitHub][1])
  * 再入防止・クラッシュ耐性（TLSガード＋別ネームスペースロード）。([GitHub][1])

---

## 2. アーキテクチャ概要

```
┌──────────────────────────────────────────────┐
│ zpoline_loader_rs  (cdylib; LD_PRELOAD注入) │
│  ├─ init(): 0番地トランポリン生成            │
│  ├─ rewrite(): 実行ページ走査+2B置換         │
│  └─ load_hook(): dlmopenでフック本体分離     │
└────────────┬─────────────────────────────────┘
             │ (関数ポインタ/FFI)
┌────────────▼────────────────┐
│ zpoline_hook_api            │  ← フック実装(利用者提供/差し替え可)
│  ├─ hook_entry(rax, args.. )│
│  └─ raw_syscall_bypass(..)  │  ← 再入防止時の“元のsyscall”実行
└─────────────────────────────┘
┌─────────────────────────────┐
│ zpoline_rewriter_rs         │  ← 命令デコード/書換え(iced-x86等)
└─────────────────────────────┘
```

* 置換方針：`syscall`/`sysenter`（2B, `0x0f 0x05` / `0x0f 0x34`）→`callq *%rax`（2B, `0xff 0xd0`）。**2バイト→2バイト**なので命令境界を壊さない。**RAX＝syscall番号**を利用し、**VA=0**のNOP領域に「落として」末尾スタブでフックへ跳ぶ。
* 初期化～運用の流れ：**LD_PRELOADで起動前初期化**→**トランポリン生成**→**コード書換え**→**フック本体をdlmopenでロード**。本家の流儀をRustで踏襲。([GitHub][1])

---

## 3. コンポーネント詳細設計

### 3.1 zpoline_loader_rs（cdylib, `LD_PRELOAD`）

* `#[ctor]` or `.init_array` で `init()` 実行。
* **VA=0トランポリン生成**

  * `mmap(addr=0, size=1〜2ページ, PROT_EXEC|PROT_READ, MAP_FIXED|ANON|PRIVATE, -1, 0)`
  * 0〜最大syscall番号N（~500前後）まで `NOP` 埋め。**最後のNOP直後に`jmp hook_entry`相当**の薄いスタブを配置。
  * 前提設定：`/proc/sys/vm/mmap_min_addr=0`、SELinuxは例外設定が必要（本家READMEの手順に準拠）。([GitHub][1])
* **コード書換え `rewrite()`**

  * `/proc/self/maps` から `r-xp`（実行）領域を抽出。
  * `mprotect` で一時的に `PROT_READ|PROT_WRITE|PROT_EXEC`（W^Xは最小時間）→命令列を**デコード**し、**“実際の” `syscall/sysenter` 命令**のみを `0xff 0xd0` に置換。
  * デコーダ：`iced-x86`（安全） or `capstone`。本家は `libopcodes` を使用。([GitHub][1])
  * **除外リスト**：自ライブラリ領域、`raw_syscall` サンクチュアリ、任意に指定されたDSO。
* **フック本体ロード `load_hook()`**

  * `dlmopen(LM_ID_NEWLM, LIBZPHOOK, RTLD_LOCAL)` で**別ネームスペース**にロードし、再入を軽減。**LIBZPHOOK**（環境変数）経由は本家互換。([GitHub][1])

### 3.2 zpoline_hook_api（フックABI）

* `extern "C" fn __hook_init(orig: *mut HookFn)`：起動時に差し替え可能な**エントリ関数ポインタ**を受け取る（本家流儀）。([GitHub][1])
* `extern "C" fn hook_entry(sysno: u64, regs: &mut RegState) -> i64`：

  * VA=0スタブから呼ばれるRust側の中心。`sysno` は RAX。レジスタ・引数は規約に合わせて取得/復元。
  * **TLSフラグ**で再入ガード。必要に応じて `raw_syscall_bypass` を呼んで本来のシステムコールへフォールバック。
* `raw_syscall_bypass(...)`：

  * **書換え対象外**の別マッピングに配置した“生の `syscall` 命令”か、**SUD**（後述）でSIGSYS→内側で直叩き。
  * loaderのリライタは**このページを常に除外**しておく。

### 3.3 zpoline_rewriter_rs（命令走査・安全書換え）

* ELF/メモリマップ走査（`dl_iterate_phdr` / `/proc/self/maps`）。
* x86-64命令デコード→`syscall/sysenter` を特定→**2B置換**。**命令境界・分岐先**を壊さない。
* 同期化

  * 初期化は `main` 前なので競合は最小。
  * **遅延ロード（dlopen等）**への追随は任意機能：`LD_AUDIT` でDSOロード時に対象領域だけ再スキャン→書換え。

---

## 4. 重要な仕様と運用前提

1. **VA=0の確保**

* 事前に `echo 0 | sudo tee /proc/sys/vm/mmap_min_addr`。SELinuxで弾かれる場合は適切なポリシー調整（本家README参照）。([GitHub][1])

2. **vDSOの扱い**

* vDSO実装（例：`clock_gettime`）は**syscall命令を使わない**場合があり、フック対象外。必要なら `LD_PRELOAD` でvDSOをバイパスする関数実装を用意。

3. **再入と“元のsyscall”**

* `dlmopen` でフック本体を分離しつつ、TLSで**ガード区間**を設けて再帰を遮断。必要に応じ**SUD（Syscall User Dispatch）**を補助的に併用して“内側”のみ直叩き経路を切る。([docs.kernel.org][2])

4. **安全性**

* 置換は**2バイト命令→2バイト命令**の同サイズ変換のみを実施。**RWX時間の最小化**、例外時ロールバック、監査ログ出力を必須。

---

## 5. 代替/補助メカニズム（任意機能）

* **SUD（Syscall User Dispatch）統合**

  * SIGSYSで確実に捕捉→初回は遅いが**「初回で位置を学習→以後は自前の書換えに切替」**というハイブリッドが可能。Linux公式ドキュメントのとおり**領域ベースの分岐**や**flip-switch**で対象/非対象を切替えられる。([docs.kernel.org][2])
* **lazypoline方式の“怠惰な書換え”**

  * 研究実装では**SUD＋遅延バイナリ書換え**で「**網羅性＋高速性**」を両取りしている。Rustからの設計流用・参考に好適。([GitHub][3])

---

## 6. エラーハンドリングとフォールバック

* VA=0確保失敗 → 実行を中止（明示）／SUD専用モードで継続。([GitHub][1])
* 書換え失敗（不明命令/圧縮コード等） → その箇所だけSUD捕捉へフォールバック。
* フック本体の例外 → TLSガード下で**raw_syscall**に切替えて復旧。

---

## 7. テスト計画

1. **機能試験**

* 最小プログラム（`write(1,"x",1)`）で番号一致と引数復元を確認。
* glibc/musl・静的/動的リンク・`dlopen` 後ロードの追随を確認。

2. **網羅性試験**

* 代表アプリ（`/bin/ls`, `cp`, `redis-benchmark`, `fio`など）で**フック命中率=100%**をログ集計（vDSOを除外して評価）。

3. **性能試験**

* 本家論文の**マイクロベンチ**に準拠（空syscall反復）。ptraceやSUD単独、lazypolineと比較（参考）。

4. **回帰・耐障害**

* SELinux有無、`mmap_min_addr` 非0、ASLR各設定、`fork/exec`、スレッドレース。

---

## 8. 実装ロードマップ（目安の順序）

1. **骨格**：`cdylib` で `init()` → VA=0トランポリン生成 → 2B置換のPoC。
2. **命令デコーダ統合**：iced-x86で安全な`syscall/sysenter`特定。
3. **フックABI**：`__hook_init`/`hook_entry`/`raw_syscall_bypass`を確定。
4. **再入対策**：TLSガード＋`dlmopen`分離、除外マップの実装。([GitHub][1])
5. **遅延ロード追随**：`LD_AUDIT`ベースの増分書換え（任意）。
6. **SUD統合**（任意）：SIGSYSで初回捕捉→以後は書換え。([docs.kernel.org][2])
7. **評価**：機能・網羅性・性能の3系統を自動化。

---

## 9. Rust実装の要点（技術スタック）

* crate

  * `zpoline_loader_rs`（cdylib, `libc`, `nix`, `iced-x86`, `object`/`goblin`）
  * `zpoline_hook_api`（`extern "C"` ABI, TLSガード, 最小asm）
  * `zpoline_samples`（動作確認）
* 低レベル

  * `core::arch::asm!` で `syscall` サンクチュアリ（置換除外ページ）を実装。
  * `/proc/self/maps` パース、`mprotect` とページ整列、W^X最小化。
  * `dlmopen` は `libloading` 経由の FFI。

---

## 10. 既知の制約・リスク

* **VA=0禁止環境**（mmap_min_addrやSELinux）では本方式は利用不可→SUDモードへ切替。([GitHub][1])
* **vDSOはフック外**（必要ならユーザー関数で明示的にバイパス）。
* 書換え中の**実行レース**：初期化を`main`前に完了し、遅延ロードは`LD_AUDIT`等で逐次対応。
* JIT/自己書換えコードは想定外。

---

## 11. 参考実装と一次情報

* **zpoline（公式）**：**2B→2B置換**と**VA=0トランポリン**の設計・性能。READMEに**LD_PRELOAD→setup_trampoline/rewrite_code/load_hook**の流れや**mmap_min_addr=0**・SELinux注意が明記。
* **Syscall User Dispatch（SUD）**：ユーザー空間でsyscallを捕捉/分離する公式機構。ハイブリッドの土台に適する。([docs.kernel.org][2])
* **lazypoline**：SUD＋遅延書換えで**網羅性と高速性**を両立した研究実装・論文。設計の参考に有用。([GitHub][3])

---

### 付記：互換実績の例

* zpolineは、ユーザー空間ネットワークスタックの透過適用（例：lwIPに差し替える `poem-lwip`）等の応用例が公開されており、実用的な統合手法の参考になります。([GitHub][4])

---

[1]: https://github.com/yasukata/zpoline "GitHub - yasukata/zpoline: system call hook for Linux"
[2]: https://docs.kernel.org/admin-guide/syscall-user-dispatch.html?utm_source=chatgpt.com "Syscall User Dispatch"
[3]: https://github.com/lazypoline/lazypoline?utm_source=chatgpt.com "The lazypoline syscall interposer"
[4]: https://github.com/yasukata/poem-lwip?utm_source=chatgpt.com "yasukata/poem-lwip: Using lwIP over the socket API"
