arm-none-eabi-objcopy -O binary ./target/test.elf ./target/test_objcopy.bin
cargo run -- deploy -f nrf52840 ./target/test.elf
cmp -l ./target/test_objcopy.bin ./target/test.bin
diff --report-identical-files ./target/test_objcopy.bin ./target/test.bin
