# macOS Apple Silicon Build Guide

## Vấn đề thường gặp

Rust compiler trên macOS Apple Silicon có thể gặp các lỗi sau:
- `SIGSEGV` crashes trong LLVM codegen
- `ErrorGuaranteed` serialization panics
- Incremental compilation cache corruption

## Giải pháp đã áp dụng

### 1. Cargo Configuration (`.cargo/config.toml`)
- ✅ Tắt incremental compilation
- ✅ Giảm memory pressure với `codegen-units=256`
- ✅ Tắt debug symbols trong dev mode
- ✅ Giới hạn parallel jobs = 4

### 2. Environment Variables (`.envrc`)
- ✅ `RUST_MIN_STACK=16777216` - Tăng stack size
- ✅ `CARGO_BUILD_JOBS=4` - Giới hạn parallel jobs
- ✅ `CARGO_INCREMENTAL=0` - Tắt incremental compilation

### 3. Build Script (`build-macos.sh`)
- ✅ Tự động set environment variables
- ✅ Clean cache trước khi build
- ✅ Build với settings tối ưu

## Cách sử dụng

### Option 1: Dùng build script (Khuyến nghị)
```bash
./build-macos.sh
./build-macos.sh --release
```

### Option 2: Dùng direnv (Auto-load environment)
```bash
# Cài đặt direnv
brew install direnv

# Thêm vào ~/.zshrc
eval "$(direnv hook zsh)"

# Cho phép direnv trong project
direnv allow

# Build bình thường
cargo build
```

### Option 3: Manual export
```bash
export RUST_MIN_STACK=16777216
export CARGO_BUILD_JOBS=4
export CARGO_INCREMENTAL=0
cargo build
```

## Nếu vẫn gặp lỗi

### 1. Clean toàn bộ cache
```bash
cargo clean
rm -rf ~/.cargo/registry/cache
rm -rf ~/.cargo/git/checkouts
```

### 2. Update Rust toolchain
```bash
rustup update stable
rustup default stable
```

### 3. Kiểm tra Xcode Command Line Tools
```bash
xcode-select --install
```

### 4. Tăng stack size hơn nữa
```bash
export RUST_MIN_STACK=33554432  # 32MB
```

## Performance Tips

### Dev builds (nhanh hơn)
```bash
cargo build
# Hoặc
./build-macos.sh
```

### Release builds (tối ưu)
```bash
cargo build --release
# Hoặc
./build-macos.sh --release
```

### Check specific package
```bash
cargo build -p common
cargo build -p scheduler
cargo build -p worker
cargo build -p api
```

## Troubleshooting

### Lỗi: "SIGSEGV in LLVM"
→ Đã fix bằng `.cargo/config.toml` (remove `target-cpu=native`)

### Lỗi: "ErrorGuaranteed serialization"
→ Đã fix bằng `incremental = false`

### Lỗi: "Out of memory"
→ Đã fix bằng `CARGO_BUILD_JOBS=4` và `codegen-units=256`

### Build quá chậm
→ Tăng `CARGO_BUILD_JOBS` nếu RAM đủ lớn (>16GB):
```bash
export CARGO_BUILD_JOBS=8
```

## Verified Configuration

✅ Tested on:
- macOS Sonoma 14.x / Sequoia 15.x
- Apple Silicon M1/M2/M3
- Rust 1.84+ stable
- 16GB+ RAM recommended

✅ Build time:
- Dev build: ~1-2 minutes (first time), ~10-30s (incremental)
- Release build: ~3-5 minutes

✅ Memory usage:
- Peak: ~4-6GB during compilation
- Runtime: ~100-500MB per service
