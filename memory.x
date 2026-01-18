/* memory.x — used by cortex-m-rt's built-in link.x */

MEMORY
{
  /* QEMU LM3S6965: 256 KiB Flash @ 0x0000_0000, 64 KiB RAM @ 0x2000_0000 */
  FLASH : ORIGIN = 0x00000000, LENGTH = 256K
  RAM   : ORIGIN = 0x20000000, LENGTH = 64K
}

/* ─────────────────────────────────────────────
   Stack layout (top of RAM, grows down)
   ───────────────────────────────────────────── */

_stack_start = ORIGIN(RAM) + LENGTH(RAM); /* 0x2000_0000 + 64K = 0x2001_0000 */

_stack_size  = 0x00001000;                /* 4 KiB stack */
_stack_end   = _stack_start - _stack_size;/* 0x2000_F000 */



/* ─────────────────────────────────────────────
   Heap layout (fixed size, grows up)
   ───────────────────────────────────────────── */

/* Example: heap from 0x2000_0000 to 0x2000_0FFF → 4 KiB */
_heap_start = ORIGIN(RAM) + 0x00001000;   /* 0x2000_0000 */
_heap_size  = 0x00001000;                 /* 4 KiB */
_heap_end   = _heap_start + _heap_size;   /* 0x2000_1000 */
