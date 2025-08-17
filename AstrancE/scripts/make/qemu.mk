# QEMU arguments

QEMU := qemu-system-$(ARCH)

ifeq ($(BUS), mmio)
  vdev-suffix := device
else ifeq ($(BUS), pci)
  vdev-suffix := pci
else
  $(error "BUS" must be one of "mmio" or "pci")
endif

ifeq ($(ARCH), x86_64)
  machine := q35
else ifeq ($(ARCH), riscv64)
  machine := virt
else ifeq ($(ARCH), aarch64)
  ifeq ($(PLAT_NAME), aarch64-raspi4)
    machine := raspi4b
    override MEM := 2G
  else
    machine := virt
  endif
else ifeq ($(ARCH), loongarch64)
  machine := virt
  override MEM := 1G
endif

qemu_args-x86_64 := \
  -machine $(machine) \
  -kernel $(OUT_ELF)

qemu_args-riscv64 := \
  -machine $(machine) \
  -bios default \
  -kernel $(FINAL_IMG)

qemu_args-aarch64 := \
  -cpu cortex-a72 \
  -machine $(machine) \
  -kernel $(FINAL_IMG)

qemu_args-loongarch64 := \
  -machine $(machine) \
  -kernel $(OUT_ELF)

qemu_args-loongarch64 := \
  -kernel $(OUT_ELF)

qemu_args-y := -m $(MEM) -smp $(SMP) $(qemu_args-$(ARCH))

#qemu_args-$(BLK) += \
  #-device virtio-blk-$(vdev-suffix),drive=disk0 \
  #-drive id=disk0,if=none,format=raw,file=$(DISK_IMG)
# 解析 DISK_IMG 为列表形式（用逗号分隔的字符串转为列表）
DISK_LIST := $(subst ,,$(shell echo $(DISK_IMG) | tr ',' ' '))

# 检查 DISK_LIST 长度是否大于 0 且 BLK 为 y 时才添加设备参数
ifneq ($(DISK_LIST),)
ifneq ($(BLK),n)
  # 定义一个计数器和挂载点，用于给每个磁盘分配唯一的 id 和设备
  DISK_COUNT := 0
  ROOTFS_ID := disk$(ROOTFS_DISK)

  # 使用 shell 循环处理每个磁盘镜像路径
  define add_disk
    qemu_args-$(BLK) += -device virtio-blk-$(vdev-suffix),drive=disk$(DISK_COUNT) \
                        -drive id=disk$(DISK_COUNT),if=none,format=raw,file=$(strip $(1))
    ifeq (disk$(DISK_COUNT),$(ROOTFS_ID))
      # 如果当前磁盘是 rootfs，则添加 root=/dev/vdX 和相关启动参数
      # 这里假设使用 virtio-blk 设备命名为 /dev/vda, /dev/vdb, ...
      DISK_LETTER := $(shell printf "\\x$(shell printf %x $$(($(DISK_COUNT) + 97)))" | tr '[:upper:]' '[:lower:]')
      qemu_args-$(BLK) += -append "root=/dev/vd$(DISK_LETTER) rw rootwait"
    endif
    DISK_COUNT := $(shell expr $(DISK_COUNT) + 1)
  endef

  # 调用 foreach 为每个磁盘镜像添加参数
  $(foreach disk,$(DISK_LIST),$(eval $(call add_disk,$(disk))))
endif
endif


qemu_args-$(NET) += \
  -device virtio-net-$(vdev-suffix),netdev=net0

ifeq ($(NET_DEV), user)
  qemu_args-$(NET) += -netdev user,id=net0,hostfwd=tcp::5555-:5555,hostfwd=udp::5555-:5555
else ifeq ($(NET_DEV), tap)
  qemu_args-$(NET) += -netdev tap,id=net0,script=scripts/net/qemu-ifup.sh,downscript=no,vhost=$(VHOST),vhostforce=$(VHOST)
  QEMU := sudo $(QEMU)
else ifeq ($(NET_DEV), bridge)
  qemu_args-$(NET) += -netdev bridge,id=net0,br=virbr0
  QEMU := sudo $(QEMU)
else
  $(error "NET_DEV" must be one of "user", "tap", or "bridge")
endif

ifneq ($(VFIO_PCI),)
  qemu_args-y += --device vfio-pci,host=$(VFIO_PCI)
  QEMU := sudo $(QEMU)
endif

ifeq ($(NET_DUMP), y)
  qemu_args-$(NET) += -object filter-dump,id=dump0,netdev=net0,file=netdump.pcap
endif

qemu_args-$(GRAPHIC) += \
  -device virtio-gpu-$(vdev-suffix) -vga none \
  -serial mon:stdio

ifeq ($(GRAPHIC), n)
  qemu_args-y += -nographic
endif

ifeq ($(QEMU_LOG), y)
  qemu_args-y += -D qemu.log -d in_asm,int,mmu,pcall,cpu_reset,guest_errors
endif

qemu_args-debug := $(qemu_args-y) -s -S

ifeq ($(ACCEL),)
  ifneq ($(findstring -microsoft, $(shell uname -r | tr '[:upper:]' '[:lower:]')),)
    ACCEL := n  # Don't enable kvm for WSL/WSL2
  else ifeq ($(ARCH), x86_64)
    ACCEL := $(if $(findstring x86_64, $(shell uname -m)),y,n)
  else ifeq ($(ARCH), aarch64)
    ACCEL := $(if $(findstring arm64, $(shell uname -m)),y,n)
  else
    ACCEL := n
  endif
endif

# Do not use KVM for debugging
ifeq ($(shell uname), Darwin)
  qemu_args-$(ACCEL) += -cpu host -accel hvf
else ifneq ($(wildcard /dev/kvm),)
  qemu_args-$(ACCEL) += -cpu host -accel kvm
endif

define run_qemu
  @printf "    $(CYAN_C)Running$(END_C) on qemu...\n"
  $(call run_cmd,$(QEMU),$(qemu_args-y))
endef

define run_qemu_debug
  @printf "    $(CYAN_C)Debugging$(END_C) on qemu...\n"
  $(call run_cmd,$(QEMU),$(qemu_args-debug))
endef
