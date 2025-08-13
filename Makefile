# 定义项目根目录，这是当前Makefile所在的目录
PROJECT_ROOT := $(dir $(abspath $(lastword $(MAKEFILE_LIST))))

# 定义App_oscomp子目录的完整路径
APP_OSCOMP_DIR := $(PROJECT_ROOT)App_oscomp
AstrancE_DIR := $(PROJECT_ROOT)AstrancE

# 定义Rust工具链目录变量
NIGHTLY_TOOLCHAIN_DIR ?= ~/.rustup/toolchains/nightly-2025-01-18-x86_64-unknown-linux-gnu

# LOG 变量，可以从命令行传递，例如 `make all LOG=1`
LOG ?= off

.PHONY: all clean help kernel-la kernel-rv rootfs vendor


# $1: 要重试的命令
# $2: 目标名称（用于错误信息）
# $3: 最大重试次数
# $4: 每次重试的延迟 (秒)
define RETRY_COMMAND
	@echo "--- Building $2 in App_oscomp (attempt 1/$(shell echo $(word 3,$(subst -, ,$(1))))) ---"
	@i=1; \
	MAX_RETRIES=$3; \
	RETRY_DELAY=$4; \
	while true; do \
	    if $(1) ; then \
	        break; \
	    fi; \
	    if [ $$i -ge $$MAX_RETRIES ]; then \
	        echo "Error: Building $2 failed after $$i attempts." >&2; \
	        exit 1; \
	    fi; \
	    i=$$(expr $$i + 1); \
	    echo "--- Building $2 failed, retrying (attempt $$i/$$MAX_RETRIES) ---"; \
	    sleep $$RETRY_DELAY; \
	done; \
	echo "--- Building $2 succeeded! ---"
endef

# 配置重试参数
DEFAULT_RETRIES := 3
DEFAULT_RETRY_DELAY := 5 # 秒

# 主构建目标
# 当 make all 时，它会依次构建 kernel-la, kernel-rv, rootfs
# 各自目标将完成自己的构建和文件移动
all: vendor kernel-la kernel-rv rootfs
	@echo "--- All components built and moved to project root ---"

vendor:
	cat AstrancE/vendor.tar.gz.* > AstrancE/vendor.tar.gz
	tar -xzf AstrancE/vendor.tar.gz -C AstrancE/
	cat App_oscomp/vendor.tar.gz.* > App_oscomp/vendor.tar.gz
	tar -xzf App_oscomp/vendor.tar.gz -C App_oscomp/


kernel-la:
	@echo "--- Building kernel-la in App_oscomp ---"
	$(call RETRY_COMMAND, \
	    ( cd "$(APP_OSCOMP_DIR)" && \
	      export PATH="$$PATH:$$HOME/.cargo/bin:./bin" && \
		  export PATH=$$PATH:$(CURDIR)/App_oscomp/bin && \
	      $(MAKE) LOG=$(LOG) kernel-la TOOLCHAIN_DIR="$(NIGHTLY_TOOLCHAIN_DIR)" \
	    ), \
	    kernel-la, \
	    $(DEFAULT_RETRIES), \
	    $(DEFAULT_RETRY_DELAY) \
	)
	@echo "--- Moving kernel-la artifacts to project root ---"
	mv -f "$(APP_OSCOMP_DIR)/kernel-la.elf" "$(PROJECT_ROOT)kernel-la" || true

# --- 特定目标: kernel-rv ---
kernel-rv:
	@echo "--- Building kernel-rv in App_oscomp ---"
	$(call RETRY_COMMAND, \
	    ( cd "$(APP_OSCOMP_DIR)" && \
	      export PATH="$$PATH:$$HOME/.cargo/bin:./bin" && \
		  export PATH=$$PATH:$(CURDIR)/App_oscomp/bin && \
	      $(MAKE) LOG=$(LOG) kernel-rv TOOLCHAIN_DIR="$(NIGHTLY_TOOLCHAIN_DIR)" \
	    ), \
	    kernel-rv, \
	    $(DEFAULT_RETRIES), \
	    $(DEFAULT_RETRY_DELAY) \
	)
	@echo "--- Moving kernel-rv artifacts to project root ---"
	mv -f "$(APP_OSCOMP_DIR)/kernel-rv.bin" "$(PROJECT_ROOT)kernel-rv" || true

# --- 特定目标: rootfs ---
# 假设 rootfs 目标在 App_oscomp 中会生成 disk-rv.img 和 disk-la.img
rootfs:
	@echo "--- Building rootfs in App_oscomp ---"
	( \
	    cd "$(APP_OSCOMP_DIR)" && \
	    export PATH="$$PATH:$$HOME/.cargo/bin:./bin" && \
		export PATH=$$PATH:$(CURDIR)/App_oscomp/bin && \
	    $(MAKE) LOG=$(LOG) rootfs TOOLCHAIN_DIR="$(NIGHTLY_TOOLCHAIN_DIR)" \
	)
	@echo "--- Moving rootfs-related artifacts to project root ---"
	cp -f "$(APP_OSCOMP_DIR)/disk-rv.img" "$(PROJECT_ROOT)disk.img" || true
	mv -f "$(APP_OSCOMP_DIR)/disk-rv.img" "$(PROJECT_ROOT)disk-rv.img" || true
	mv -f "$(APP_OSCOMP_DIR)/disk-la.img" "$(PROJECT_ROOT)disk-la.img" || true

clean:
	@echo "--- Cleaning main project artifacts ---"
	# 直接列出所有可能生成的文件，进行清理
	rm -f "$(PROJECT_ROOT)disk-la.img" \
	      "$(PROJECT_ROOT)disk-rv.img" \
	      "$(PROJECT_ROOT)kernel-rv" \
	      "$(PROJECT_ROOT)kernel-la" || true

	@echo "--- Initiating clean for App_oscomp ---"
	( \
		cd "$(APP_OSCOMP_DIR)" && \
		rm -rf vendor vendor.tar.gz && \
		$(MAKE) clean\
	) || true
	@echo "--- Initiating clean for AstrancE ---"
	( \
		cd "$(AstrancE_DIR)" && \
		rm -rf vendor vendor.tar.gz && \
		$(MAKE) clean \
	) || true

help:
	@echo "Usage:"
	@echo "  make all                     Builds all artifacts."
	@echo "  make kernel-la               Builds kernel-la only."
	@echo "  make kernel-rv               Builds kernel-rv only."
	@echo "  make rootfs                  Builds rootfs only."
	@echo "  make clean                   Cleans the project."
	@echo "  make all LOG=1               Builds with verbose logging."
	@echo "  make all NIGHTLY_TOOLCHAIN_DIR=/path/to/your/toolchain  Build with a specific nightly toolchain."
	@echo ""
	@echo "Configurable variables:"
	@echo "  NIGHTLY_TOOLCHAIN_DIR = $(NIGHTLY_TOOLCHAIN_DIR) (Default toolchain directory)"
	@echo "  LOG = $(LOG) (0 or 1 for verbose logging)"



