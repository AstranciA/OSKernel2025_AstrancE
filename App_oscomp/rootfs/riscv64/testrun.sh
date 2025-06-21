./lua_testcode.sh
./basic_testcode.sh
./libcbench_testcode.sh
/test_busybox.sh
./busybox echo "#### OS COMP TEST GROUP START libctest-musl ####"
./run-static.sh
./run-dynamic.sh
./busybox echo "#### OS COMP TEST GROUP END libctest-musl ####"
