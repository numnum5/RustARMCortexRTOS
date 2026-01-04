@"C:\Program Files\qemu\qemu-system-arm.exe" ^
    -cpu cortex-m3 ^
    -machine lm3s6965evb ^
    -nographic ^
    -semihosting-config enable=on,target=native ^
    -kernel %*