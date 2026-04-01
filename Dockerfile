# 定义构建参数
ARG TARGETARCH
ARG BUILD_PREVIEW=false
ARG BUILD_COMPAT=false

# ==================== 构建阶段 ====================
FROM --platform=linux/${TARGETARCH} rustlang/rust:nightly-trixie-slim AS builder

ARG TARGETARCH
ARG BUILD_PREVIEW
ARG BUILD_COMPAT

WORKDIR /build

# 安装构建依赖及 Rust musl 工具链
RUN apt-get update && \
    apt-get install -y --no-install-recommends gcc nodejs npm lld musl-tools && \
    rm -rf /var/lib/apt/lists/* && \
    case "$TARGETARCH" in \
        amd64) rustup target add x86_64-unknown-linux-musl ;; \
        arm64) rustup target add aarch64-unknown-linux-musl ;; \
        *) echo "Unsupported architecture for rustup: $TARGETARCH" && exit 1 ;; \
    esac

COPY . .

# 根据构建选项，设置编译参数并构建项目
RUN \
    # 根据架构设置编译目标和优化的 CPU 型号
    case "$TARGETARCH" in \
        amd64) \
            TARGET_TRIPLE="x86_64-unknown-linux-musl"; \
            TARGET_CPU="x86-64-v3" ;; \
        arm64) \
            TARGET_TRIPLE="aarch64-unknown-linux-musl"; \
            TARGET_CPU="neoverse-n1" ;; \
        *) echo "Unsupported architecture: $TARGETARCH" && exit 1 ;; \
    esac && \
    \
    # 组合 cargo features
    FEATURES="" && \
    if [ "$BUILD_PREVIEW" = "true" ]; then FEATURES="$FEATURES __preview_locked"; fi && \
    if [ "$BUILD_COMPAT" != "true" ]; then FEATURES="$FEATURES __perf"; fi && \
    FEATURES=$(echo "$FEATURES" | xargs) && \
    \
    # 准备 RUSTFLAGS，兼容模式下移除特定 CPU 优化以获得更好的兼容性
    RUSTFLAGS_BASE="-C link-arg=-s -C link-arg=-fuse-ld=lld -C target-feature=+crt-static -A unused" && \
    if [ "$BUILD_COMPAT" = "true" ]; then \
        export RUSTFLAGS="$RUSTFLAGS_BASE"; \
    else \
        export RUSTFLAGS="$RUSTFLAGS_BASE -C target-cpu=$TARGET_CPU"; \
    fi && \
    \
    # 执行构建
    # -C link-arg=-s: 移除符号表以减小体积
    # -C target-feature=+crt-static: 静态链接 C 运行时
    # -C target-cpu: 针对特定 CPU 优化
    # -A unused: 允许未使用的代码
    if [ -n "$FEATURES" ]; then \
        cargo build --bin cursor-api --release --target=$TARGET_TRIPLE --features "$FEATURES"; \
    else \
        cargo build --bin cursor-api --release --target=$TARGET_TRIPLE; \
    fi && \
    \
    mkdir -p /app && \
    cp target/$TARGET_TRIPLE/release/cursor-api /app/

# ==================== 运行阶段 ====================
FROM scratch

# 从构建阶段复制二进制文件，并设置为非 root 用户所有
COPY --chown=1001:1001 --chmod=0700 --from=builder /app /app

WORKDIR /app

ENV PORT=3000
EXPOSE ${PORT}

# 使用非 root 用户运行，增强安全性
USER 1001

ENTRYPOINT ["/app/cursor-api"]
