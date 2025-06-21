#!/bin/bash

# ===============================================================
# 项目构建与提交自动化脚本 (V39: 使用临时沙盒目录进行操作，并精确导入历史)
# ===============================================================

set -e

# 定义 AstrancE 和 App_oscomp 远程仓库 URL
ASTRANCE_SOURCE_URL="https://github.com/AstranciA/AstrancE.git"
APP_OSCOMP_SOURCE_URL="https://github.com/AstranciA/App_oscomp.git"

# 在脚本开始时获取当前工作目录的绝对路径，所有相对路径都基于此
SCRIPT_ORIGIN_DIR=$(pwd)
echo ">> 脚本运行的起始目录 (最终提交仓库根目录): ${SCRIPT_ORIGIN_DIR}"

# --- 定义临时沙盒目录 ---
# 所有中间操作都在此目录中进行，最后将其清除。
# 命名为随机字符串以避免冲突，并以 . 开头以便默认隐藏
SANDBOX_DIR="${SCRIPT_ORIGIN_DIR}/.tmpdir_$(head /dev/urandom | tr -dc A-Za-z0-9 | head -c 8)"

# 在沙盒内的项目目录路径
SANDBOX_APP_OSCOMP_DIR="${SANDBOX_DIR}/App_oscomp"
SANDBOX_ASTRANCE_DIR="${SANDBOX_DIR}/AstrancE"

# 最终在主仓库中的目标子目录名 (filter-repo --path-rename 用)
APP_OSCOMP_FINAL_SUBDIR_NAME="App_oscomp"
ASTRANCE_FINAL_SUBDIR_NAME="AstrancE"

# *************************************************************************
# 错误处理和清理函数
# *************************************************************************
# 调试模式，不清理目录
KEEP_DEBUG_DIRS=false
# 清空项目目录模式
CLEAN_PROJECT_DIRS=false # 这个参数现在用于清除宿主目录下的App_oscomp和AstrancE

for arg in "$@"; do
    case "$arg" in
        -d|--debug)
            KEEP_DEBUG_DIRS=true
            echo ">> (Debug Mode) 调试模式已开启：脚本退出时将保留沙盒目录和所有中间文件。"
            ;;
        --clean)
            CLEAN_PROJECT_DIRS=true
            echo ">> (Clean Mode) 检测到 --clean 参数，将清空宿主目录下已存在的 ${APP_OSCOMP_FINAL_SUBDIR_NAME} 和 ${ASTRANCE_FINAL_SUBDIR_NAME}。"
            ;;
        *)
            # 兼容其他未知参数
            ;;
    esac
done

cleanup_on_exit() {
    local exit_code=$? # 获取脚本的退出码

    echo ">> (trap) 脚本退出。退出码: $exit_code"

    if [ "$KEEP_DEBUG_DIRS" = true ]; then
        echo ">> (trap) 调试模式已开启，沙盒目录 ${SANDBOX_DIR} 将保留以供调试。"
    else
        if [ -d "$SANDBOX_DIR" ]; then
            echo ">> (trap) 正在清理临时沙盒目录: $SANDBOX_DIR"
            sudo rm -rf "$SANDBOX_DIR" || echo "警告：清理沙盒目录失败。请手动检查并清理: $SANDBOX_DIR"
        fi
        # git filter-repo 可能会在 /tmp 下留下 state 文件
        rm -f "/tmp/git-filter-repo-*.state" > /dev/null 2>&1 || true
        echo ">> (trap) 临时目录清理流程完成。"
    fi
    # 确保 master 分支为检出状态，防止 HEAD 分离导致后续操作问题
    git -C "$SCRIPT_ORIGIN_DIR" checkout master > /dev/null 2>&1 || true
    # 清理 filter-repo 可能留下的临时引用
    git -C "$SCRIPT_ORIGIN_DIR" reflog expire --all --expire=now > /dev/null 2>&1 || true
    git -C "$SCRIPT_ORIGIN_DIR" gc --prune=now --aggressive > /dev/null 2>&1 || true
    echo ">> (trap) Git 仓库清理完成。"
}

# 设置 trap，无论脚本是正常退出还是因错误退出，都会触发 cleanup_on_exit
#trap cleanup_on_exit EXIT

# *************************************************************************
# 脚本主逻辑开始
# *************************************************************************

echo "--------------------------------------------------------"
echo "开始执行项目构建与提交脚本 (版本 V39: 使用临时沙盒目录进行操作，并精确导入历史)."
echo "--------------------------------------------------------"

# 辅助函数：检查命令是否存在
check_command_exists() {
    local cmd=$1
    if ! command -v "$cmd" &> /dev/null; then
        echo "错误：所需命令 '$cmd' 未找到。请安装此工具。"
        return 1
    fi
    return 0
}

# 检查所需工具
echo ">> 检查所需工具..."
check_command_exists "git" || exit 1
check_command_exists "cargo" || exit 1
check_command_exists "make" || exit 1
check_command_exists "riscv64-linux-musl-gcc" || exit 1
check_command_exists "loongarch64-linux-musl-gcc" || exit 1
if ! command -v git-filter-repo &> /dev/null; then
    echo "错误：git-filter-repo 未安装。"
    echo "请运行 'pip install git-filter-repo' 安装此工具。"
    exit 1
fi
echo ">> 所有工具已找到。"

# 在脚本开始时请求一次 sudo 权限，避免后续多次提示
echo ">> 脚本可能需要 sudo 权限来执行 make all 或其他系统操作。"
echo ">> 请在提示时输入您的 sudo 密码："
sudo -v || { echo "错误：无法获取 sudo 权限。请确保当前用户有 sudo 权限。"; exit 1; }
echo ">> 已成功获取 sudo 权限。"


# cargo vendor 及其 config.toml 更新
handle_vendor() {
    local project_name=$1
    local relative_config_path=$2
    local project_abs_path=$3
    local abs_config_path="${project_abs_path}/${relative_config_path}"
    local config_dir=$(dirname "$abs_config_path")

    echo ">> (Vendor) 准备处理 ${project_name} 的 cargo vendor (配置路径: ${abs_config_path})..."

    if [ ! -f "${project_abs_path}/Cargo.toml" ]; then
        echo "错误：(Vendor) 在 ${project_abs_path} 中找不到 Cargo.toml 文件，无法执行 cargo vendor。"
        return 1
    fi

    mkdir -p "$config_dir" || { echo "错误：(Vendor) 无法创建目录 ${config_dir}"; return 1; }

    local original_config_content=""
    if [ -f "$abs_config_path" ]; then
        original_config_content=$(cat "$abs_config_path")
        echo ">> (Vendor) 已读取并保存现有 config.toml 内容。"
    else
        echo ">> (Vendor) config.toml 文件不存在，将创建新文件。"
    fi
 
    local temp_config_for_update=$(mktemp)
    {
        echo "[net]"
        echo "offline = false"  # 临时设置为 false 允许联网
        echo "git-fetch-with-cli = true"
        echo ""
        # 写入原始内容，排除旧的 vendor 和 net 块
        echo "$original_config_content" | awk '
            BEGIN {
                in_net_section = 0;
                in_vendor_block = 0;
            }

            /^# === BEGIN CARGO VENDOR CONFIG === #/{ in_vendor_block=1; next; }
            /^# === END CARGO VENDOR CONFIG === #/{ in_vendor_block=0; next; }
            in_vendor_block { next; }

            /^[[:space:]]*\[net\]/{ in_net_section = 1; next; }
            /^[[:space:]]*\[[^]]+\]/{
                if (in_net_section) { in_net_section = 0; }
                print; next;
            }
            in_net_section { next; }
            { print; }
        ' | awk '{gsub(/^[ \t]+|[ \t]+$/, ""); print}' | awk 'NF > 0' | sed -e '/^[[:space:]]*$/d'
    } > "$temp_config_for_update"

    echo ">> (Vendor) 创建临时 config.toml (${temp_config_for_update}) 用于 cargo update..."

    cp "$temp_config_for_update" "$abs_config_path" || { echo "错误：(Vendor) 无法写入临时 config.toml 到 ${abs_config_path} for update phase"; rm -f "$temp_config_for_update"; return 1; }
    rm -f "$temp_config_for_update"

    echo ">> (Vendor) 切换到项目目录并执行 cargo update 以生成/更新 Cargo.lock..."
    if ! (cd "$project_abs_path" && cargo update --workspace); then
        echo "错误：(Vendor) ${project_name} 的 cargo update 失败。这通常意味着网络问题或 Cargo.toml 配置问题。"
        # 恢复原始config.toml内容
        if [ -n "$original_config_content" ]; then
            echo -e "$original_config_content" > "$abs_config_path"
        else
            rm -f "$abs_config_path" # 如果原本不存在，则删除
        fi
        return 1
    fi
    echo ">> (Vendor) Cargo.lock 已生成/更新。"

    echo ">> (Vendor) 执行 cargo vendor (${project_name} at ${project_abs_path})..."
    local cargo_vendor_full_output=""
    if ! cargo_vendor_full_output=$( (cd "$project_abs_path" && cargo vendor 2>&1 ) ); then
        echo "错误：(Vendor) ${project_name} 的 cargo vendor 失败。输出：\n$cargo_vendor_full_output"
        # 恢复原始config.toml内容
        if [ -n "$original_config_content" ]; then
            echo -e "$original_config_content" > "$abs_config_path"
        else
            rm -f "$abs_config_path" # 如果原本不存在，则删除
        fi
        return 1
    fi
    echo ">> (Vendor) cargo vendor 执行完毕。"

    echo ">> (Vendor) 从 cargo vendor 输出中提取 vendored-sources 配置块..."
    local extracted_vendor_config=""
    extracted_vendor_config=$(echo "$cargo_vendor_full_output" | \
                              awk '/^\[source\.vendored-sources\]/{print; p=1; next} p && !/^\[/{print} /^(targets|source)(\.|$)/{p=0}' | \
                              awk '!seen[$0]++' | \
                              sed 's/^[[:space:]]*//; s/[[:space:]]*$//' | \
                              awk 'NF > 0 {print}')

    if [ -z "$extracted_vendor_config" ]; then
        echo "警告：(Vendor) 从 cargo vendor 输出中未能提取到 vendored sources 配置。"
        echo "DEBUG: (Vendor) cargo vendor 完整输出内容: \n$cargo_vendor_full_output"
    fi

    local final_config_content=""
    final_config_content+="[net]\n"
    final_config_content+="offline = true\n" # 设为离线模式
    final_config_content+="git-fetch-with-cli = true\n"

    local cleaned_original_config_content_for_final=$(echo "$original_config_content" | awk '
        BEGIN {
            in_net_section = 0;
            in_vendor_block = 0;
        }

        /^# === BEGIN CARGO VENDOR CONFIG === #/{ in_vendor_block=1; next; }
        /^# === END CARGO VENDOR CONFIG === #/{ in_vendor_block=0; next; }
        in_vendor_block { next; }

        /^[[:space:]]*\[net\]/{ in_net_section = 1; next; }
        /^[[:space:]]*\[[^]]+\]/{
            if (in_net_section) { in_net_section = 0; }
            print; next;
        }
        in_net_section { next; }
        { print; }
    ' | awk '{gsub(/^[ \t]+|[ \t]+$/, ""); print}' | awk 'NF > 0' | sed -e '/^[[:space:]]*$/d')

    if [ -n "$cleaned_original_config_content_for_final" ]; then
        final_config_content+="\n"
        final_config_content+="$cleaned_original_config_content_for_final\n"
    fi

    if [ -n "$extracted_vendor_config" ]; then
        final_config_content+="\n# === BEGIN CARGO VENDOR CONFIG === #\n"
        final_config_content+="$extracted_vendor_config\n"
        final_config_content+="# === END CARGO VENDOR CONFIG === #\n"
    fi

    echo -e "$final_config_content" > "$abs_config_path" || { echo "错误：(Vendor) 无法写入最终 config.toml 到 ${abs_config_path}"; return 1; }
    echo ">> (Vendor) ${abs_config_path} 已更新并设为 offline = true，并包含了 vendored sources 配置。"

    return 0
}


# --- 辅助函数：确保沙盒中的项目存在并更新 ---
ensure_sandbox_project_ready() {
    local project_dir=$1
    local source_url=$2
    local project_name=$3

    git config --global core.sparseCheckout false # 确保不使用稀疏检出
    
    if [ ! -d "$project_dir/.git" ]; then
        echo ">> 正在克隆 ${project_name} 到沙盒 ${project_dir}..."
        (cd "$(dirname "$project_dir")" && git clone --recurse-submodules "${source_url}" "$(basename "$project_dir")") || { echo "错误：克隆 ${project_name} 到沙盒失败"; return 1; }
        echo ">> ${project_name} 克隆到沙盒完成。"
    else
        echo ">> 沙盒中 ${project_name} 已存在，正在更新..."
        (
            cd "$project_dir" || exit 1
            git pull --recurse-submodules --ff-only || { echo "警告：更新沙盒中 ${project_name} 失败，可能网络问题或冲突。跳过本地更新。"; }
        )
        echo ">> 沙盒中 ${project_name} 更新完成。"
    fi
    return 0
}

# *************************************************************************
# 流程 0: 初始化沙盒目录和宿主仓库
# *************************************************************************
echo "--------------------------------------------------------"
echo "流程 0: 初始化沙盒目录和宿主仓库..."
echo "--------------------------------------------------------"

# 创建沙盒目录
echo ">> 创建临时沙盒目录: $SANDBOX_DIR"
mkdir -p "$SANDBOX_DIR" || { echo "错误：无法创建沙盒目录 ${SANDBOX_DIR}"; exit 1; }

# 清理宿主目录中可能存在的旧项目文件 (如果使用 --clean 参数)
if [ "$CLEAN_PROJECT_DIRS" = true ]; then
    echo ">> (Clean) 清理宿主目录下旧的 ${APP_OSCOMP_FINAL_SUBDIR_NAME} 和 ${ASTRANCE_FINAL_SUBDIR_NAME}..."
    sudo rm -rf "${SCRIPT_ORIGIN_DIR}/${APP_OSCOMP_FINAL_SUBDIR_NAME}" || true
    sudo rm -rf "${SCRIPT_ORIGIN_DIR}/${ASTRANCE_FINAL_SUBDIR_NAME}" || true
    # 移除可能存在的 .git，重新初始化 (如果 `--clean` 的语义是完全重新开始)
    if [ -d "${SCRIPT_ORIGIN_DIR}/.git" ]; then
        echo ">> 移除宿主目录下的 .git 仓库进行完全清理..."
        sudo rm -rf "${SCRIPT_ORIGIN_DIR}/.git"
    fi
fi


# *************************************************************************
# 流程 1: 在沙盒中克隆并配置 App_oscomp
# *************************************************************************
echo "--------------------------------------------------------"
echo "流程 1: 在沙盒中克隆并配置 App_oscomp..."
echo "--------------------------------------------------------"
ensure_sandbox_project_ready "$SANDBOX_APP_OSCOMP_DIR" "$APP_OSCOMP_SOURCE_URL" "App_oscomp" || exit 1

(
    # 进入沙盒内的 App_oscomp 目录进行配置
    echo ">> 进入沙盒 ${SANDBOX_APP_OSCOMP_DIR}"
    cd "$SANDBOX_APP_OSCOMP_DIR" || exit 1

    echo ">> (App_oscomp-Sandbox) 安装 axconfig-gen..."
    cargo install --root . axconfig-gen || { echo "错误：(App_oscomp-Sandbox) 安装 axconfig-gen 失败。"; exit 1; }
    export PATH="$(pwd)/bin:$PATH" # 将其bin目录添加到当前PATH

    echo ">> (App_oscomp-Sandbox) App_oscomp 软链接创建和 vendor 操作将推迟到 AstrancE 准备完成后。"
)


# *************************************************************************
# 流程 2: 在沙盒中克隆并配置 AstrancE
# *************************************************************************
echo "--------------------------------------------------------"
echo "流程 2: 在沙盒中克隆并配置 AstrancE..."
echo "--------------------------------------------------------"
ensure_sandbox_project_ready "$SANDBOX_ASTRANCE_DIR" "$ASTRANCE_SOURCE_URL" "AstrancE" || exit 1

(
    # 进入沙盒内的 AstrancE 目录进行配置
    echo ">> 进入沙盒 ${SANDBOX_ASTRANCE_DIR}"
    cd "$SANDBOX_ASTRANCE_DIR" || exit 1

    echo ">> (AstrancE-Sandbox) 预编译 AstrancE/crates/lwext4_rust..."
    lwext4_rust_dir="${SANDBOX_ASTRANCE_DIR}/crates/lwext4_rust"
    if [ ! -f "${lwext4_rust_dir}/Cargo.toml" ]; then
        echo "错误：(AstrancE-Sandbox) 在 ${lwext4_rust_dir} 中找不到 Cargo.toml 文件，可能是克隆不完整或路径错误。"
        return 1
    fi
    
    cd "$lwext4_rust_dir" || { echo "错误：无法进入 ${lwext4_rust_dir} 目录"; exit 1; }

    # 编译 riscv
    riscv_output="${SANDBOX_ASTRANCE_DIR}/target/riscv64gc-unknown-none-elf/debug/liblwext4_rust.a"
    echo ">> (AstrancE-Sandbox) 编译 lwext4_rust (针对 riscv)... (预期输出: $riscv_output)"
    if [ -f "$riscv_output" ]; then
        echo ">> (AstrancE-Sandbox) lwext4_rust (riscv) 已编译，跳过。"
    elif [ -d "$(dirname "$riscv_output")" ]; then
        export CC="riscv64-linux-musl-gcc"
        PATH="$PATH" cargo build -vv --target riscv64gc-unknown-none-elf || { echo "错误：(AstrancE-Sandbox) lwext4_rust (riscv) 编译失败"; exit 1; }
        echo ">> (AstrancE-Sandbox) lwext4_rust (riscv) 编译完成。"
    else
        echo "警告：(AstrancE-Sandbox) 目标输出目录 ${riscv_output%/*} 不存在或不可写，跳过riscv编译。"
    fi

    # 编译 loongarch
    loongarch_output="${SANDBOX_ASTRANCE_DIR}/target/loongarch64-unknown-none/debug/liblwext4_rust.a"
    echo ">> (AstrancE-Sandbox) 编译 lwext4_rust (针对 loongarch)... (预期输出: $loongarch_output)"
    if [ -f "$loongarch_output" ]; then
        echo ">> (AstrancE-Sandbox) lwext4_rust (loongarch) 已编译，跳过。"
    elif [ -d "$(dirname "$loongarch_output")" ]; then
        export CC="loongarch64-linux-musl-gcc"
        PATH="$PATH" cargo build -vv --target loongarch64-unknown-none || { echo "错误：(AstrancE-Sandbox) lwext4_rust (loongarch) 编译失败"; exit 1; }
        echo ">> (AstrancE-Sandbox) lwext4_rust (loongarch) 编译完成。"
    else
        echo "警告：(AstrancE-Sandbox) 目标输出目录 ${loongarch_output%/*} 不存在或不可写，跳过loongarch编译。"
    fi

    echo ">> (AstrancE-Sandbox) 返回 AstrancE 根目录..."
    cd "$SANDBOX_ASTRANCE_DIR" || exit 1 # 返回 AstrancE 根目录

    echo ">> (AstrancE-Sandbox) 对 AstrancE 执行 cargo vendor..."
    handle_vendor "AstrancE" ".cargo/config.toml" "$SANDBOX_ASTRANCE_DIR" || { echo "错误：AstrancE 的 handle_vendor 失败"; exit 1; }

    echo ">> (AstrancE-Sandbox) 修改 AstrancE/scripts/make/cargo.mk 第 13 行..."
    cargo_mk_path="${SANDBOX_ASTRANCE_DIR}/scripts/make/cargo.mk"
    if grep -q "build_args := \\\n  --offline \\\\" "$cargo_mk_path" >/dev/null; then
        echo ">> (AstrancE-Sandbox) AstrancE/scripts/make/cargo.mk 已包含 --offline，跳过修改。"
    else
        if sed -i.bak '13a\  --offline \\' "$cargo_mk_path"; then
            echo ">> (AstrancE-Sandbox) AstrancE/scripts/make/cargo.mk 修改完成。"
            rm "$cargo_mk_path.bak"
        else
            echo "错误：(AstrancE-Sandbox) 修改 AstrancE/scripts/make/cargo.mk 失败。"
            echo "请检查 ${cargo_mk_path} 的内容和第 13 行附近。"
            exit 1
        fi
    fi
)
echo ">> AstrancE (沙盒) 仓库配置完成。"


# *************************************************************************
# 流程 3: 完成 App_oscomp 配置 (依赖沙盒中已准备好的 AstrancE)
# *************************************************************************
echo "--------------------------------------------------------"
echo "流程 3: 完成 App_oscomp 配置 (依赖沙盒中已准备好的 AstrancE)..."
echo "--------------------------------------------------------"

(
    echo ">> 进入沙盒 ${SANDBOX_APP_OSCOMP_DIR}"
    cd "$SANDBOX_APP_OSCOMP_DIR" || exit 1

    echo ">> (App_oscomp-Sandbox) 创建 App_oscomp/.AstrancE 软链接..."
    # 软链接目标是与 App_oscomp 同级的 AstrancE 目录
    # 每次运行脚本时清理并重新创建，确保链接正确
    if [ -L "./.AstrancE" ] || [ -d "./.AstrancE" ]; then
        rm -rf "./.AstrancE" || { echo "错误：无法删除旧的 ./.AstrancE"; exit 1; }
    fi
    ln -s "../${ASTRANCE_FINAL_SUBDIR_NAME}" "./.AstrancE" || { echo "错误：创建软链接 App_oscomp/.AstrancE 失败"; exit 1; }
    echo ">> (App_oscomp-Sandbox) App_oscomp/.AstrancE 软链接创建完成。"

    echo ">> (App_oscomp-Sandbox) 对 App_oscomp 执行 cargo vendor..."
    handle_vendor "App_oscomp" "scripts/config.toml" "$SANDBOX_APP_OSCOMP_DIR" || { echo "错误：App_oscomp 的 handle_vendor 失败"; exit 1; }

    echo ">> (App_oscomp-Sandbox) 修改 App_oscomp/Makefile..."
    makefile_path="${SANDBOX_APP_OSCOMP_DIR}/Makefile"
    if grep -q "RUSTUP_TOOLCHAIN=nightly-2025-01-18" "$makefile_path"; then
        echo ">> (App_oscomp-Sandbox) App_oscomp/Makefile 已包含 RUSTUP_TOOLCHAIN，跳过修改。"
    else
        # 匹配 'A=$(PWD)' 捕获它，然后在其后插入 RUSTUP_TOOLCHAIN
        # 在 RUSTUP_TOOLCHAIN-value 和 '$1' 之间添加一个空格
        if sed -i.bak 's/\(A=\$\(PWD\)\)\([[:space:]]*EXTRA_CONFIG\)/RUSTUP_TOOLCHAIN=nightly-2025-01-18 \1\3/' "$makefile_path"; then
            echo ">> (App_oscomp-Sandbox) App_oscomp/Makefile 修改完成。"
            rm "$makefile_path.bak"
        else
            echo "错误：(App_oscomp-Sandbox) 修改 App_oscomp/Makefile 失败。"
            echo "请手动检查 ${makefile_path} 的内容。"
            exit 1
        fi
    fi
)
echo ">> App_oscomp (沙盒) 运行时配置完成。"


# *************************************************************************
# 流程 4: 在沙盒中创建外部 Makefile (临时验证用)
# *************************************************************************
echo "--------------------------------------------------------"
echo "流程 4: 在沙盒中创建外部 Makefile (临时验证用)..."
echo "--------------------------------------------------------"
# 这个 Makefile 仅用于沙盒内的本地编译验证，最终提交的 Makefile 是在宿主目录的
SANDBOX_MAJOR_MAKEFILE_CONTENT=$(cat <<'EOF'
all:
	cd ./App_oscomp && \
	export PATH="$$PATH:$$HOME/.cargo/bin" && \
	export PATH="$$PATH:$(CURDIR)/App_oscomp/bin" && \
	#export MAKEFLAGS="-j$(shell nproc)" && \
	make all TOOLCHAIN_DIR=~/.rustup/toolchains/nightly-2025-01-18-x86_64-unknown-linux-gnu && \
	cd .. && \
	mv ./App_oscomp/disk-rv.img . || true && \
	mv ./App_oscomp/disk-la.img . || true && \
	mv ./App_oscomp/kernel-rv.bin ./kernel-rv || true && \
	mv ./App_oscomp/kernel-la.elf ./kernel-la || true

.PHONY: all

clean:
	rm -f ./disk-la.img || true
	rm -f ./disk-rv.img || true
	rm -f ./kernel-rv || true
	rm -f ./kernel-la || true
	cd ./App_oscomp && \
	make clean || true
EOF
)
echo "$SANDBOX_MAJOR_MAKEFILE_CONTENT" > "${SANDBOX_DIR}/Makefile" || { echo "错误：在沙盒中创建 Makefile 失败"; exit 1; }
echo ">> 沙盒中 Makefile 创建完成。"


# *************************************************************************
# 流程 5: 运行 make all 验证编译 (在沙盒中 App_oscomp 目录)
# *************************************************************************
echo "--------------------------------------------------------"
echo "流程 5: 运行 make all 验证编译 (在沙盒中 ${SANDBOX_APP_OSCOMP_DIR})..."
echo "--------------------------------------------------------"
(
    cd "$SANDBOX_DIR" || exit 1 # 进入沙盒根目录，运行沙盒内的 Makefile

    echo ">> 在沙盒 ${SANDBOX_DIR} 中执行 make all..."
    # 确保 axconfig-gen 在 PATH 中 (虽然 makefile 会处理，但此处额外确保)
    export PATH="${SANDBOX_APP_OSCOMP_DIR}/bin:$PATH" 
    make all || {
        echo "--------------------------------------------------------"
        echo "错误：在沙盒 ${SANDBOX_DIR} 中 make all 失败！"
        echo "--------------------------------------------------------"
        exit 1
    }
    echo ">> 在沙盒 ${SANDBOX_DIR} 中 make all 成功完成。"

    echo ">> 在沙盒 ${SANDBOX_DIR} 中执行 make clean (清理临时编译文件)..."
    make clean || { echo "警告：在沙盒 ${SANDBOX_DIR} 中 make clean 失败"; }
    echo ">> 沙盒 ${SANDBOX_DIR} 已清理。"
)


# *************************************************************************
# 流程 6: 使用 git filter-repo 导入沙盒内项目的历史到主仓库
# *************************************************************************
echo "--------------------------------------------------------"
echo "流程 6: 使用 git filter-repo 导入沙盒内项目的历史到主仓库..."
echo "--------------------------------------------------------"

# 确保宿主目录是一个 Git 仓库并有一个初始提交
cd "$SCRIPT_ORIGIN_DIR" || exit 1
if [ ! -d ".git" ]; then
    git init || { echo "错误：初始化宿主 Git 仓库失败。"; exit 1; }
    echo ">> 宿主 Git 仓库已初始化。"
fi
echo ">> 宿主 Git 仓库准备完毕。"

echo ">> 将 ${SANDBOX_ASTRANCE_DIR} 分支更名为master"

cd "$SANDBOX_ASTRANCE_DIR" && git branch -m "master" || exit 1

cd "$SCRIPT_ORIGIN_DIR" || exit 1 # 回到宿主 Git 仓库根目录

#echo ">> 将 ${SANDBOX_APP_OSCOMP_DIR} 的历史导入宿主仓库的 ${APP_OSCOMP_FINAL_SUBDIR_NAME}/..."
#git filter-repo --source "${SANDBOX_APP_OSCOMP_DIR}" --refs master --target . --path-rename ":${APP_OSCOMP_FINAL_SUBDIR_NAME}/" --force || { echo "错误：导入 App_oscomp 历史失败。"; exit 1; }
#echo ">> App_oscomp 历史导入完成。"

echo ">> 将 ${SANDBOX_ASTRANCE_DIR} 的历史导入宿主仓库的 ${ASTRANCE_FINAL_SUBDIR_NAME}/..."
git filter-repo --source "${SANDBOX_ASTRANCE_DIR}" --refs master --target . --path-rename ":${ASTRANCE_FINAL_SUBDIR_NAME}/" --force || { echo "错误：导入 AstrancE 历史失败。"; exit 1; }
cp -r $SANDBOX_ASTRANCE_DIR/crates $APP_OSCOMP_FINAL_SUBDIR_NAME/crates
echo ">> AstrancE 历史导入完成。"

cp -r $SANDBOX_APP_OSCOMP_DIR $APP_OSCOMP_FINAL_SUBDIR_NAME
echo ">> 复制App_oscomp 完成"

# 解决软链接问题：因为现在 App_oscomp 和 AstrancE 都已在主仓库根目录下，软链接需要重新指向
# 在主仓库层面创建 App_oscomp/.AstrancE 软链接
echo ">> 在最终主仓库目录中创建 App_oscomp/.AstrancE 软链接..."
(
    cd "${SCRIPT_ORIGIN_DIR}/${APP_OSCOMP_FINAL_SUBDIR_NAME}" || exit 1
    if [ -L "./.AstrancE" ] || [ -d "./.AstrancE" ]; then
        rm -rf "./.AstrancE" || { echo "错误：无法删除旧的 ./.AstrancE"; exit 1; }
    fi
    ln -s "../${ASTRANCE_FINAL_SUBDIR_NAME}" "./.AstrancE" || { echo "错误：创建 ./App_oscomp/.AstrancE 软链接失败"; exit 1; }
)
echo ">> 最终软链接 App_oscomp/.AstrancE 创建完成。"


# *************************************************************************
# 流程 7: 创建并验证宿主 Makefile
# *************************************************************************
echo "--------------------------------------------------------"
echo "流程 7: 创建并验证宿主 Makefile..."
echo "--------------------------------------------------------"
MAJOR_MAKEFILE_CONTENT=$(cat <<'EOF'
all:
	cd ./App_oscomp && \
	export PATH="$$PATH:$$HOME/.cargo/bin" && \
	export PATH="$$PATH:$(CURDIR)/App_oscomp/bin" && \
	#export MAKEFLAGS="-j$(shell nproc)" && \
	make all TOOLCHAIN_DIR=~/.rustup/toolchains/nightly-2025-01-18-x86_64-unknown-linux-gnu && \
	cd .. && \
	mv ./App_oscomp/disk-rv.img . || true && \
	mv ./App_oscomp/disk-la.img . || true && \
	mv ./App_oscomp/kernel-rv.bin ./kernel-rv || true && \
	mv ./App_oscomp/kernel-la.elf ./kernel-la || true

.PHONY: all

clean:
	rm -f ./disk-la.img || true
	rm -f ./disk-rv.img || true
	rm -f ./kernel-rv || true
	rm -f ./kernel-la || true
	cd ./App_oscomp && \
	make clean || true
EOF
)
makefile_path_at_origin_dir="${SCRIPT_ORIGIN_DIR}/Makefile"
if [ -f "$makefile_path_at_origin_dir" ]; then
    echo ">> 宿主 Makefile 已存在，内容校验并更新..."
    if ! diff -q <(echo "$MAJOR_MAKEFILE_CONTENT") "$makefile_path_at_origin_dir" >/dev/null; then
        echo ">> 宿主 Makefile 内容不一致，正在更新..."
        echo "$MAJOR_MAKEFILE_CONTENT" > "$makefile_path_at_origin_dir" || { echo "错误：创建宿主 Makefile 失败"; exit 1; }
        echo ">> 宿主 Makefile 更新完成。"
    else
        echo ">> 宿主 Makefile 内容验证通过，无需修改。"
    fi
else
    echo "$MAJOR_MAKEFILE_CONTENT" > "$makefile_path_at_origin_dir" || { echo "错误：创建宿主 Makefile 失败"; exit 1; }
    echo ">> 宿主 Makefile 创建完成。"
fi

echo ">> 运行宿主 make all 进行最终验证..."
cd "$SCRIPT_ORIGIN_DIR" || exit 1
(
    export PATH="${SCRIPT_ORIGIN_DIR}/${APP_OSCOMP_FINAL_SUBDIR_NAME}/bin:$PATH"
    make all || {
        echo "--------------------------------------------------------"
        echo "错误：宿主 make all 失败！"
        echo "--------------------------------------------------------"
        exit 1
    }
)
echo ">> 宿主 make all 成功完成。"

echo ">> 执行宿主 make clean (清理最终输出文件)..."
make clean || { echo "错误：make clean 失败"; exit 1; }
echo ">> 宿主 make clean 完成。"


# *************************************************************************
# 流程 8: 配置 .gitignore
# *************************************************************************
echo "--------------------------------------------------------"
echo "流程 8: 配置 .gitignore 文件..."
echo "--------------------------------------------------------"
gitignore_path="${SCRIPT_ORIGIN_DIR}/.gitignore"

NEW_GITIGNORE_CONTENT=$(cat << EOF
# Generated by automated build script (appended rules)

# Temp sandbox directory
/${SANDBOX_DIR##*/}/

# Project specific ignores within the final merged directories
/App_oscomp/target/
/App_oscomp/bin/ # axconfig-gen 安装的 bin 目录
/App_oscomp/vendor/
/App_oscomp/disk-la.img
/App_oscomp/disk-rv.img
/App_oscomp/kernel-la.elf
/App_oscomp/kernel-rv.bin

/AstrancE/target/
/AstrancE/vendor/

# Root level build outputs (moved from App_oscomp)
*.img
*.elf
*.bin

# Other common build artifacts
*.o
*.d
EOF
)

if [ -f "$gitignore_path" ]; then
    (
        cat "$gitignore_path"
        echo "$NEW_GITIGNORE_CONTENT"
    ) | awk '{ if (!seen[$0]++) print }' > "${gitignore_path}.tmp" && mv "${gitignore_path}.tmp" "$gitignore_path"
else
    echo "$NEW_GITIGNORE_CONTENT" > "$gitignore_path"
fi

echo ">> .gitignore 配置完成。"


# *************************************************************************
# 流程 9: 提交更改
# *************************************************************************
echo "--------------------------------------------------------"
echo "流程 9: 提交更改..."
echo "--------------------------------------------------------"
git add . || { echo "Error: git add failed"; exit 1; }

if ! git diff-index --quiet HEAD --; then
    git commit -m "Integrate App_oscomp and AstrancE with full historical context and build setup" || { echo "Error: git commit failed." ; exit 1; }
    echo ">> 新的更改已提交。"
else
    echo ">> 没有新的更改需要提交。"
fi


# *************************************************************************
# 流程 X: 推送代码
# *************************************************************************
echo "--------------------------------------------------------"
echo "流程 X: 推送代码..."
echo "--------------------------------------------------------"
GIT_REMOTE_URL="https://gitlab.eduxiji.net/T202518123995667/submit_test.git"

echo ">> 准备添加远程仓库并推送代码..."
if ! git remote get-url origin &> /dev/null; then
    git remote add origin "$GIT_REMOTE_URL" || {
        echo "Warning: Failed to add remote repository, URL may be incorrect or network issue."
    }
else
    current_remote_url=$(git config --get remote.origin.url)
    if [ "$current_remote_url" != "$GIT_REMOTE_URL" ]; then
        echo ">> Remote origin URL mismatch, updating to new URL."
        git remote set-url origin "$GIT_REMOTE_URL" || {
            echo "Warning: Failed to update remote origin URL."
        }
        git remote set-url --push origin "$GIT_REMOTE_URL"
    else
        echo ">> Remote origin URL is already correctly configured."
    fi
fi

if git branch -M master 2>/dev/null; then
    echo ">> Branch named 'master'."
else
    echo "Warning: Failed to ensure branch is 'master'. May already be 'master' or other issue."
fi

echo ">> Performing Git push..."
echo "Estimated total size is between 20-40MB. If it exceeds, please check .gitignore."
git push -uf origin master || {
    echo "--------------------------------------------------------"
    echo "Error: git push failed!"
    echo "--------------------------------------------------------"
    exit 1
}
echo ">> Code pushed successfully!"

echo "--------------------------------------------------------"
echo "所有操作均已完成！"
echo "--------------------------------------------------------"


