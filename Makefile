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
