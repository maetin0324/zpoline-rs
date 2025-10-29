# zpoline-rs

Rust implementation of [zpoline](https://github.com/yasukata/zpoline) - a binary rewriting system call hook for Linux.

Replaces `syscall` instructions (0x0f 0x05) with `callq *%rax` (0xff 0xd0) to redirect execution to a VA=0 trampoline, enabling low-overhead syscall interception.

## Features

- Binary rewriting via iced-x86 instruction decoder
- VA=0 trampoline for syscall hooks
- Hook libraries loaded via dlmopen in separate namespace
- Trait-based hook API for type-safe syscall interception
- TLS-based re-entry guard
- Verified: 620+ syscall instructions rewritten in libc

## Components

- `zpoline_loader` - LD_PRELOAD library, handles trampoline setup and code rewriting
- `zpoline_rewriter` - Instruction decoder and syscall replacement
- `zpoline_hook_api` - Hook ABI and trait-based syscall hooks
- `zpoline_hook_impl` - Default hook library (syscall tracer)
- `zpoline_hook_trait_example` - Example trait-based hook library

## Quick Start

```bash
# Build
cargo build --release

# Set VA=0 (requires sudo, one-time)
sudo sysctl -w vm.mmap_min_addr=0

# Run with default tracer hook
LD_PRELOAD=./target/release/libzpoline_loader.so ./your_program

# Run with custom hook library
ZPOLINE_HOOK=./target/release/libzpoline_hook_trait_example.so \
LD_PRELOAD=./target/release/libzpoline_loader.so \
./your_program
```

## Trait-based Hooks

Create a hook library by implementing the `SyscallHooks` trait:

```rust
use zpoline_hook_api::{SyscallHooks, register_syscall_hooks, get_trait_dispatch_hook, syscall_hooks::*};
use ctor::ctor;

struct MyHooks;

impl SyscallHooks for MyHooks {
    fn hook_write(&mut self, fd: i32, buf: *const std::ffi::c_void, count: usize) -> isize {
        // Custom logic here
        default_write(fd, buf, count)
    }
}

#[ctor]
fn init() {
    register_syscall_hooks(MyHooks);
}

#[no_mangle]
pub extern "C" fn zpoline_hook_init() -> *const () {
    get_trait_dispatch_hook()
}
```

Build as cdylib and load via `ZPOLINE_HOOK` environment variable. See [TRAIT_HOOKS_USAGE.md](TRAIT_HOOKS_USAGE.md) for details.

## Requirements

- Linux x86-64
- `vm.mmap_min_addr=0` (sudo sysctl -w vm.mmap_min_addr=0)
- Rust 2021 edition

## Documentation

- [TRAIT_HOOKS_USAGE.md](TRAIT_HOOKS_USAGE.md) - Trait-based hooks guide
- [USAGE.md](USAGE.md) - Usage and troubleshooting

## Architecture

```
┌─────────────────────────┐
│  zpoline_loader         │  (LD_PRELOAD)
│  - VA=0 trampoline      │
│  - Code rewriting       │
│  - dlmopen hook loading │
└───────────┬─────────────┘
            │
┌───────────▼─────────────┐
│  zpoline_hook_api       │
│  - Hook ABI             │
│  - SyscallHooks trait   │
│  - raw_syscall          │
└─────────────────────────┘
```

Hook libraries are loaded via dlmopen into separate namespace, call `zpoline_hook_init()` to return hook function pointer.

## References

- [zpoline](https://github.com/yasukata/zpoline) - Original implementation
- [Syscall User Dispatch](https://docs.kernel.org/admin-guide/syscall-user-dispatch.html) - Kernel syscall interception
- [lazypoline](https://github.com/lazypoline/lazypoline) - Lazy binary rewriting with SUD

## License

MIT OR Apache-2.0
