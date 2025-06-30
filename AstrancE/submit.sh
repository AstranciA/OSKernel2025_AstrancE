#!/bin/bash

# ===============================================================
# 项目构建与提交自动化脚本 (V13: 修复顶层 local 关键字使用错误)
# ===============================================================

set -e # 任何命令失败都立即退出

echo "--------------------------------------------------------"
echo "开始执行项目构建与提交脚本 (版本 V13)..."
echo "此脚本将检查现有仓库并进行更新，如果不存在则克隆。"
echo "--------------------------------------------------------"

# 获取脚本开始时的当前工作目录的绝对路径
SCRIPT_ORIGIN_DIR=$(pwd)
echo ">> 脚本运行的起始目录: ${SCRIPT_ORIGIN_DIR}"

# 辅助函数：更新或克隆Git仓库及其子模块
# 参数1: 仓库URL
# 参数2: 目标目录名 (相对于 SCRIPT_ORIGIN_DIR)
update_or_clone_repo() {
    local repo_url=$1
    local repo_name=$2
    local repo_dir="${SCRIPT_ORIGIN_DIR}/${repo_name}" # 构建绝对路径

    echo ">> 处理仓库: ${repo_name} (路径: ${repo_dir})"
    if [ -d "$repo_dir" ]; then
        echo ">> 目录 ${repo_dir} 已存在，正在更新..."
        (
            cd "$repo_dir" || { echo "错误：无法进入 ${repo_dir} 目录进行更新"; exit 1; }
            git reset --hard HEAD # 确保工作区是干净的，避免 pull 冲突
            # 尝试非递归拉取，如果需要子模块，再单独更新
            git pull || { echo "警告：更新 ${repo_dir} 失败，尝试继续..."; }
            echo ">> 更新子模块..."
            git submodule update --init --recursive || { echo "警告：更新 ${repo_dir} 子模块失败，尝试继续..."; }
        ) || exit 1 # 如果子shell失败，则退出整个脚本
    else
        echo ">> 目录 ${repo_dir} 不存在，正在克隆..."
        (
            cd "$SCRIPT_ORIGIN_DIR" || { echo "错误：无法返回起始目录 ${SCRIPT_ORIGIN_DIR}"; exit 1; }
            git clone --recurse-submodules "$repo_url" "$repo_name" || { echo "错误：克隆 ${repo_name} 失败"; exit 1; }
        ) || exit 1 # 如果子shell失败，则退出整个脚本
    fi
    echo ">> 仓库 ${repo_name} 处理完成。"
}

# 辅助函数：处理 cargo vendor 及其 config.toml 更新
# 这是核心修改部分，旨在彻底解决 vendored-sources 问题
# 参数1: 项目名 (str) - 用于打印日志信息
# 参数2: config.toml 相对路径 (str) - 相对于当前函数执行时的CWD
# 参数3: 项目的根绝对路径 (str) - 用于 cargo vendor
handle_vendor() {
    local project_name=$1
    local relative_config_path=$2
    local project_abs_path=$3 # 项目的根绝对路径
    local abs_config_path="${project_abs_path}/${relative_config_path}"
    local config_dir=$(dirname "$abs_config_path")
    local vendor_source_section_marker="To use vendored sources, add this to your .cargo/config.toml for this project:"

    echo ">> 准备处理 ${project_name} 的 cargo vendor (配置路径: ${abs_config_path})..."

    # 确保 config.toml 的目录存在
    mkdir -p "$config_dir" || { echo "错误：无法创建目录 ${config_dir}"; return 1; }

    # 1. 备份原始 config.toml (如果存在)
    local original_config_content=""
    if [ -f "$abs_config_path" ]; then
        original_config_content=$(cat "$abs_config_path")
        echo ">> 备份原始 config.toml 内容..."
    fi

    # 2. 构建临时的 config.toml 用于 cargo vendor (只包含 [net] offline = false)
    #    这个临时的 config.toml 确保 cargo vendor 在线执行，不受其他配置干扰
    #    写入一个临时文件，然后复制到目标位置，确保目标文件内容是干净的。
    local temp_config_for_vendor=$(mktemp)
    echo -e "[net]\noffline = false" > "$temp_config_for_vendor"
    echo ">> 创建临时 config.toml (${temp_config_for_vendor}) 用于 cargo vendor..."
    
    # 将临时 config.toml 复制到实际 config_path，以便 cargo vendor 使用
    # 这一步可能会短暂覆盖掉实际的 config.toml，但在 vendor 后会重新生成。
    cp "$temp_config_for_vendor" "$abs_config_path" || { echo "错误：无法写入临时 config.toml 到 ${abs_config_path}"; return 1; }


    # 3. 执行 cargo vendor
    echo ">> 执行 cargo vendor (${project_name} at ${project_abs_path})..."
    local vendor_output_file=$(mktemp)
    
    # 切换到项目根目录执行 cargo vendor
    (
        cd "$project_abs_path" || { echo "错误：无法进入 ${project_abs_path} 目录进行 cargo vendor"; exit 1; }
        if ! cargo vendor 2>&1 | tee "$vendor_output_file"; then
            echo "WARN：${project_name} 的 cargo vendor 失败。"
            exit 1 # 退出子shell
        fi
    ) || {
        # 如果子shell失败，则回到脚本原始目录，并处理清理和恢复
        echo "WARN：${project_name} 的 cargo vendor 失败。将尝试恢复 config.toml。"
        if [ -n "$original_config_content" ]; then
            echo -e "$original_config_content" > "$abs_config_path"
            echo ">> config.toml 已恢复到原始内容。"
        else
            rm -f "$abs_config_path" # 如果原本没有，则删除
            echo ">> config.toml 已删除 (因 vendor 失败且无原始内容)。"
        fi
        rm -f "$vendor_output_file" "$temp_config_for_vendor" 2>/dev/null || true # 清理临时文件
        return 1 # 返回非零值表示失败
    }

    # 4. 从 cargo vendor 输出中提取 vendored sources 配置
    echo ">> 从 cargo vendor 输出中提取 vendored-sources 配置..."
    local extracted_vendor_config=$(awk -v marker="$vendor_source_section_marker" '
        $0 ~ marker { found_marker=1; next; } # 找到标记行，跳过它
        found_marker { print; } # 打印标记行之后的所有内容 (包括空行)
    ' "$vendor_output_file")
    
    # 移除 cargo vendor 额外输出的空行，并trim首尾空白
    extracted_vendor_config=$(echo "$extracted_vendor_config" | awk 'NF > 0 {print}' | sed -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//')

    rm -f "$vendor_output_file" # 清理 cargo vendor 的临时输出文件

    # 5. 构建最终的 config.toml 内容
    local final_config_content=""
    
    # A. 提取原始 config.toml 中非 [net] 和非 [source.*] 的部分
    local existing_non_vendor_non_net_content=""
    if [ -n "$original_config_content" ]; then
        existing_non_vendor_non_net_content=$(echo "$original_config_content" | awk '
            BEGIN { in_relevant_section=0; }
            /^\[net\]/ { in_relevant_section=1; next; }
            /^\[source\./ { in_relevant_section=1; next; }
            
            # 如果在相关 section 中遇到新的 Section，则结束当前 section
            in_relevant_section==1 && /^\s*\[[a-zA-Z0-9_\.-]+\]/ {
                in_relevant_section=0;
            }
            # 如果不在相关 section，且不是空行，则打印
            in_relevant_section==0 && length($0) > 0 { print; }
        ')
        # 移除任何可能因awk处理产生的多余空行，并trim首尾空白
        existing_non_vendor_non_net_content=$(echo "$existing_non_vendor_non_net_content" | awk 'NF > 0 {print}' | sed -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//')
    fi

    # B. 组合最终内容
    # 1. 首先添加 [net] 部分，强制 offline = true
    final_config_content+="[net]\noffline = true"

    # 2. 添加原有内容（如果存在且非空）
    if [ -n "$existing_non_vendor_non_net_content" ]; then
        # 确保与 [net] 之间有一个空行
        final_config_content+="\n\n$existing_non_vendor_non_net_content"
    fi

    # 3. 添加 cargo vendor 的部分
    if [ -n "$extracted_vendor_config" ]; then
        # 确保与前一部分有一个空行
        final_config_content+="\n\n# === BEGIN CARGO VENDOR CONFIG === #\n"
        final_config_content+="$extracted_vendor_config\n"
        final_config_content+="# === END CARGO VENDOR CONFIG === #"
    else
        echo "警告：在 cargo vendor 输出中未找到 vendored sources 配置。请检查 vendor 是否成功。"
    fi
    
    # 写入最终的 config.toml
    echo -e "$final_config_content" > "$abs_config_path" || { echo "错误：无法写入最终 config.toml 到 ${abs_config_path}"; return 1; }
    echo ">> ${abs_config_path} 已更新并设为 offline = true，并包含了 vendored sources 配置。"

    # 清理所有临时文件
    rm -f "$temp_config_for_vendor" 2>/dev/null || true
    return 0 # 成功
}


# 1. 克隆或更新仓库
update_or_clone_repo "https://github.com/AstranciA/AstrancE.git" "AstrancE"
update_or_clone_repo "https://github.com/AstranciA/App_oscomp.git" "App_oscomp"
echo ">> 仓库处理完成。"

# 定义项目根目录的绝对路径，以便后续使用
APP_OSCOMP_DIR="${SCRIPT_ORIGIN_DIR}/App_oscomp"
ASTRANCE_DIR="${SCRIPT_ORIGIN_DIR}/AstrancE"


# 2. 处理 App_oscomp 目录
echo ">> 处理 App_oscomp 目录..."
(
    cd "$APP_OSCOMP_DIR" || { echo "错误：无法进入 ${APP_OSCOMP_DIR} 目录"; exit 1; }

    echo ">> 安装 axconfig-gen..."
    # 检查全局或本地是否已安装
    # 优先使用项目内的 target_bins/bin 路径，因为它是由 cargo install --root 安装的
    if [ -f "${APP_OSCOMP_DIR}/target_bins/bin/axconfig-gen" ]; then
        export PATH="${APP_OSCOMP_DIR}/target_bins/bin:$PATH"
        echo ">> axconfig-gen 已在项目本地 bin 目录，并已添加到 PATH。"
    elif ! command -v axconfig-gen &> /dev/null; then
        echo ">> axconfig-gen 不在 PATH 中，正在安装..."
        cargo install --root "${APP_OSCOMP_DIR}/target_bins" axconfig-gen || { echo "错误：安装 axconfig-gen 失败"; exit 1; }
        export PATH="${APP_OSCOMP_DIR}/target_bins/bin:$PATH"
        echo ">> axconfig-gen 已安装并添加到 PATH。"
    else
        echo ">> axconfig-gen 已在 PATH 中 (可能为全局安装或其他位置)。"
    fi

    echo ">> 创建软链接 ./.AstrancE..."
    # 软链接 ./.AstrancE 应该指向 App_oscomp 的同级目录下的 AstrancE
    # 所以当我们在 App_oscomp 内部时，它应该指向 ../AstrancE (尽管我们现在用绝对路径创建)
    link_target="${ASTRANCE_DIR}"
    link_name="${APP_OSCOMP_DIR}/.AstrancE" # 这里不是 local，因为在子shell内，会被导出
    if [ -L "$link_name" ] && [ -e "$link_name" ]; then
        # 检查软链接是否指向正确的目标
        current_target=$(readlink "$link_name")
        if [ "$current_target" == "$link_target" ]; then
            echo ">> 软链接 ${link_name} 已存在且有效，跳过创建。"
        else
            echo ">> 软链接 ${link_name} 存在但指向错误 (${current_target})，正在修正。"
            rm "$link_name"
            if [ -d "$link_target" ]; then
                ln -s "$link_target" "$link_name" || { echo "错误：创建软链接 ${link_name} -> ${link_target} 失败"; exit 1; }
                echo ">> 软链接 ${link_name} 修正完成。"
            else
                echo "错误：无法找到目标目录 ${link_target}，软链接修正失败。"
                exit 1 # 退出子shell
            fi
        fi
    else
        # 删除旧的无效软链接以便重新创建
        [ -L "$link_name" ] && rm "$link_name"
        if [ -d "$link_target" ]; then
            ln -s "$link_target" "$link_name" || { echo "错误：创建软链接 ${link_name} -> ${link_target} 失败"; exit 1; }
            echo ">> 软链接 ${link_name} 创建完成。"
        else
            echo "错误：无法找到目标目录 ${link_target}，软链接创建失败。"
            exit 1 # 退出子shell
        fi
    fi

    # 调用 handle_vendor 函数处理 App_oscomp 的 vendoring
    # 相对于 App_oscomp 目录的 config.toml 路径是 "scripts/config.toml"
    handle_vendor "App_oscomp" "scripts/config.toml" "$APP_OSCOMP_DIR" || { echo "错误：App_oscomp 的 handle_vendor 失败"; exit 1; }


    # 新增：执行 make ax_root 以设置 AX_ROOT
    echo ">> 执行 make ax_root 以设置 AX_ROOT..."
    make ax_root || { echo "错误：make ax_root 失败"; exit 1; } # 确保在 App_oscomp 目录内部执行

    echo ">> 修改 App_oscomp/Makefile..."
    makefile_path="${APP_OSCOMP_DIR}/Makefile"
    if grep -q "RUSTUP_TOOLCHAIN=nightly-2025-01-18 -C \$(AX_ROOT)" "$makefile_path"; then
        echo ">> App_oscomp/Makefile 已包含 RUSTUP_TOOLCHAIN=nightly-2025-01-18 -C \$(AX_ROOT)，跳过修改。"
    else
        # sed -i 默认会在修改前创建文件副本，因此 .bak 后缀会创建一个 .bak 文件
        if sed -i.bak 's/^\(@make\s*\)\(-C\s*\$\(AX_ROOT\)\)\(\s*.*$\)/\1RUSTUP_TOOLCHAIN=nightly-2025-01-18 \2\4/' "$makefile_path"; then
            echo ">> App_oscomp/Makefile 修改完成。"
        else
            echo "错误：修改 App_oscomp/Makefile 失败。请检查文件内容或行号。"
            exit 1 # 退出子shell
        fi
    fi
) # 退出 App_oscomp 的子shell
echo ">> App_oscomp 目录处理完成。"


# 3. 处理 AstrancE 目录
echo ">> 处理 AstrancE 目录..."
(
    cd "$ASTRANCE_DIR" || { echo "错误：无法进入 ${ASTRANCE_DIR} 目录"; exit 1; }

    # 首先处理 AstrancE 的 vendoring，确保编译时依赖能找到
    handle_vendor "AstrancE" ".cargo/config.toml" "$ASTRANCE_DIR" || { echo "错误：AstrancE 的 handle_vendor 失败"; exit 1; }

    echo ">> 进入 crates/lwext4_rust/"
    lwext4_rust_dir="${ASTRANCE_DIR}/crates/lwext4_rust"
    cd "$lwext4_rust_dir" || { echo "错误：无法进入 ${lwext4_rust_dir} 目录"; exit 1; }

    echo ">> 编译 lwext4_rust (针对 riscv)..."
    # 假设的输出文件路径, 使用绝对路径
    riscv_output="${ASTRANCE_DIR}/target/riscv64gc-unknown-none-elf/debug/lwext4_rust_static.a"
    if [ -f "$riscv_output" ]; then
        echo ">> lwext4_rust (riscv) 已编译，跳过。"
    else
        # 在 lwext4_rust 目录中执行 cargo build
        cargo build -vv --target riscv64gc-unknown-none-elf || { echo "错误：lwext4_rust (riscv) 编译失败"; exit 1; }
        echo ">> lwext4_rust (riscv) 编译完成。"
    fi

    echo ">> 编译 lwext4_rust (针对 loongarch)..."
    # 假设的输出文件路径, 使用绝对路径
    loongarch_output="${ASTRANCE_DIR}/target/loongarch64-unknown-none/debug/lwext4_rust_static.a"
    if [ -f "$loongarch_output" ]; then
        echo ">> lwext4_rust (loongarch) 已编译，跳过。"
    else
        # 在 lwext4_rust 目录中执行 cargo build
        cargo build -vv --target loongarch64-unknown-none || { echo "错误：lwext4_rust (loongarch) 编译失败"; exit 1; }
        echo ">> lwext4_rust (loongarch) 编译完成。"
    fi

    echo ">> 返回 AstrancE 根目录..."
    cd "$ASTRANCE_DIR" || { echo "错误：无法返回 ${ASTRANCE_DIR} 根目录"; exit 1; }


    echo ">> 修改 AstrancE/scripts/make/cargo.mk 第 13 行..."
    cargo_mk_path="${ASTRANCE_DIR}/scripts/make/cargo.mk"
    if grep -q "build_args := \\\n  --offline \\\\" "$cargo_mk_path"; then
        echo ">> AstrancE/scripts/make/cargo.mk 已包含 --offline，跳过修改。"
    else
        if sed -i.bak 's/^\(build_args\s*:=\s*\\\)\(\s*\n\)/\1\2  --offline \\\n/' "$cargo_mk_path"; then
            echo ">> AstrancE/scripts/make/cargo.mk 修改完成。"
        else
            echo "错误：修改 AstrancE/scripts/make/cargo.mk 失败。请检查文件内容或行号。"
            exit 1 # 退出子shell
        fi
    fi
) # 退出 AstrancE 的子shell
echo ">> AstrancE 目录处理完成。"


# 4. 创建外部 Makefile
echo ">> 创建外部 Makefile..."
MAJOR_MAKEFILE_CONTENT=$(cat <<'EOF'
all:
	# 进入 App_oscomp 目录并执行 make all
	# 注意：这里的 makeall 命令会在 App_oscomp 自己的 Makefile 中寻找定义
	# 确保 App_oscomp 的 Makefile 能够找到 axconfig-gen 和正确的工具链
	cd ./App_oscomp && \
	export PATH=$$PATH:$(CURDIR)/App_oscomp/target_bins/bin && \
	make all TOOLCHAIN_DIR = ~/.rustup/toolchains/nightly-2025-01-18-x86_64-unknown-linux-gnu && \
	cd .. && \
	mv ./App_oscomp/disk.img . || true && \
	mv ./App_oscomp/kernel-rv.bin ./kernel-rv || true && \
	mv ./App_oscomp/kernel-la.elf ./kernel-la || true

.PHONY: all

clean:
	rm -f ./disk.img || true
	rm -f ./kernel-rv || true
	rm -f ./kernel-la || true
	cd ./App_oscomp && \
	make clean || true
EOF
)
# 修正：移除顶层的 local 关键字
makefile_path_at_origin_dir="${SCRIPT_ORIGIN_DIR}/Makefile"
if [ -f "$makefile_path_at_origin_dir" ]; then
    echo ">> 外部 Makefile 已存在，内容校验并更新..."
    if ! diff -q <(echo "$MAJOR_MAKEFILE_CONTENT") "$makefile_path_at_origin_dir" >/dev/null; then
        echo ">> 外部 Makefile 内容不一致，正在更新..."
        echo "$MAJOR_MAKEFILE_CONTENT" > "$makefile_path_at_origin_dir" || { echo "错误：创建外部 Makefile 失败"; exit 1; }
        echo ">> 外部 Makefile 更新完成。"
    else
        echo ">> 外部 Makefile 内容验证通过，无需修改。"
    fi
else
    echo "$MAJOR_MAKEFILE_CONTENT" > "$makefile_path_at_origin_dir" || { echo "错误：创建外部 Makefile 失败"; exit 1; }
    echo ">> 外部 Makefile 创建完成。"
fi


# 5. 运行 make all
echo ">> 运行 make all..."
echo "注意: 编译过程可能需要较长时间，请耐心等待。"
# 将 make all 也在一个子shell中，确保 PATH 变量的临时性
(
    # Ensure axconfig-gen is in PATH for the main make call, in case it's not global
    export PATH="${APP_OSCOMP_DIR}/target_bins/bin:$PATH"
    make all || {
        echo "--------------------------------------------------------"
        echo "错误：make all 失败！"
        echo "      常见的失败原因包括："
        echo "      - `axconfig-gen` 没有正确安装或路径不对。请确认 `cargo install --root "${APP_OSCOMP_DIR}/target_bins"` 命令成功。"
        echo "      - `App_oscomp/Makefile` 的修改不正确。"
        echo "      - `cargo vendor` 后 `config.toml` 配置不生效，导致网络访问。请检查 `handle_vendor` 函数是否成功执行且 `$project_abs_path/.cargo/config.toml` 内容正确。"
        echo "      - 某个 `crate` 有多个不同版本在 `vendor` 中，需要手动排查修改依赖。"
        echo "      - 工具链错误，请检查 Makefile 中 TOOLCHAIN_DIR 路径是否正确，是否安装了 `riscv` 和 `loongarch` 的 `rustup target`。"
        echo "      - Rust target 未安装: `rustup target add riscv64gc-unknown-none-elf loongarch64-unknown-none`"
        echo "      请手动解决上述问题后，重新运行此脚本."
        echo "--------------------------------------------------------"
        exit 1
    }
) || { echo "错误：make all 命令执行失败。"; exit 1; } # 捕获子shell的退出码
echo ">> make all 成功完成。"

echo ">> 执行 make clean..."
make clean || { echo "错误：make clean 失败"; exit 1; }
echo ">> make clean 完成。"

# Return to SCRIPT_ORIGIN_DIR before git operations
cd "$SCRIPT_ORIGIN_DIR" || { echo "错误：无法返回起始目录 ${SCRIPT_ORIGIN_DIR}"; exit 1; }

# 6. 配置 .gitignore
echo ">> 配置 .gitignore 文件..."
gitignore_path="${SCRIPT_ORIGIN_DIR}/.gitignore" # 修正：移除 local
if [ -f "$gitignore_path" ]; then
    cp "$gitignore_path" "$gitignore_path.bak"
    # 过滤掉 App_oscomp/target/，build/，target/，以及各种二进制文件和图片文件
    grep -v -E "^\/App_oscomp\/target\/.*|^build\/|^target\/|\*.\(img\|elf\|bin\)$" "$gitignore_path.bak" > "$gitignore_path"
    rm "$gitignore_path.bak"
fi

cat << EOF >> "$gitignore_path"
/App_oscomp/target/
build/
target/
*.img
*.elf
*.bin
# 添加 App_oscomp/target_bins 目录
/App_oscomp/target_bins/
EOF
echo ">> .gitignore 配置完成。"

# 7. 清理 .git 目录并初始化新仓库
echo ">> 清理 App_oscomp 和 AstrancE 的 .git 目录..."
echo "注意: 此操作将删除 App_oscomp 和 AstrancE 的原始 Git 历史记录。"
clean_git_dirs() {
    local dir=$1
    if [ -d "$dir/.git" ]; then
        rm -rf "$dir/.git" || { echo "错误：删除 $dir/.git 失败"; exit 1; }
        echo ">> 已删除 ${dir}/.git"
    fi
    if [ -f "$dir/.gitmodules" ]; then
        # 尝试删除，如果失败则输出警告并继续
        rm -f "$dir/.gitmodules" && echo ">> 已删除 ${dir}/.gitmodules" || echo "警告：删除 ${dir}/.gitmodules 失败或文件不存在，可能没有副作用。"
    fi
}
clean_git_dirs "$APP_OSCOMP_DIR"
clean_git_dirs "$ASTRANCE_DIR"
echo ">> .git 目录清理完成。"

echo ">> 初始化新的 Git 仓库..."
if [ -d "${SCRIPT_ORIGIN_DIR}/.git" ]; then
    echo ">> 当前目录已是 Git 仓库。"
    git add . || { echo "错误：git add 失败"; exit 1; }
    if ! git diff-index --quiet HEAD --; then
        git commit -m "Update after build process" || { echo "错误：git commit failed. Did you run this script in an empty directory initially?" ; exit 1; }
        echo ">> 新的更改已提交。"
    else
        echo ">> 没有新的更改需要提交。"
    fi
else
    git init || { echo "错误：git init failed. Did you run this script in an empty directory initially?"; exit 1; }
    git add . || { echo "错误：git add failed. Did you run this script in an empty directory initially?"; exit 1; }
    git commit -m "Initial commit after build process" || { echo "错误：git commit failed. Did you run this script in an empty directory initially?"; exit 1; }
    echo ">> 新的 Git 仓库初始化并提交完成。"
fi


# 8. 推送代码
GIT_REMOTE_URL="https://gitlab.eduxiji.net/T202518123995667/submit_test.git" # 将此替换为你的实际远程仓库地址

echo ">> 准备添加远程仓库并推送代码..."
if ! git remote get-url origin &> /dev/null; then
    git remote add origin "$GIT_REMOTE_URL" || {
        echo "警告：添加远程仓库失败，可能URL不正确或网络问题。"
    }
else
    current_remote_url=$(git config --get remote.origin.url)
    if [ "$current_remote_url" != "$GIT_REMOTE_URL" ]; then
        echo ">> 远程 origin URL 不匹配，更新为新的 URL。"
        git remote set-url origin "$GIT_REMOTE_URL" || {
            echo "警告：更新远程 origin URL 失败。"
        }
        git remote set-url --push origin "$GIT_REMOTE_URL" # 确保推送URL也更新
    else
        echo ">> 远程 origin URL 已正确配置。"
    fi
fi

if git branch -M main; then
    echo ">> 分支已重命名为 main。"
else
    echo "警告：重命名分支为 main 失败。可能已经存在或 Git 版本问题。"
fi

echo ">> 正在进行 Git 推送..."
echo "预计总大小在 20-40MB 之间。如果超过，请检查 .gitignore。"
git push -uf origin main || {
    echo "--------------------------------------------------------"
    echo "错误：git push 失败！"
    echo "      原因包括但不限于："
    echo "      - 网络连接问题。"
    echo "      - Git 远程仓库的访问权限问题 (例如 SSH Key 未设置或 HTTP 密码错误)。"
    echo "      - 推送的分支冲突，或者远程仓库是空的需要先 `push -u`。"
    echo "      - 推送内容过大，请检查 `.gitignore` 是否正确过滤了大型文件 (如 `target/`, `node_modules/` 等)。"
    echo "      解决方法："
    echo "      1. 检查网络连接。"
    echo "      2. 确认 GitLab 账户权限和 SSH key/密码是否正确。"
    echo "      3. 如果是首次推送，可以使用 `git push -u origin main`。"
    echo "      4. 检查 `.gitignore` 文件，确保不必要的大文件未被跟踪。"
    echo "--------------------------------------------------------"
    exit 1
}
echo ">> 代码推送成功！"

echo "--------------------------------------------------------"
echo "所有操作已成功完成！"
echo "--------------------------------------------------------"


