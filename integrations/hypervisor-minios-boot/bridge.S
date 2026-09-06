.section .text
.code16
.globl _start
.type _start, @function

.set MULTIBOOT_BOOTLOADER_MAGIC, 0x2BADB002
.set MULTIBOOT_INFO_ADDR, 0x5000
.set KERNEL_ENTRY_ADDR, 0x6000

_start:
    cli
    xorw %ax, %ax
    movw %ax, %ds
    movw %ax, %es
    movw %ax, %ss

    lgdt gdt_descriptor

    movl %cr0, %eax
    orl $0x1, %eax
    movl %eax, %cr0
    ljmpl $0x08, $protected_entry

.code32
protected_entry:
    movw $0x10, %ax
    movw %ax, %ds
    movw %ax, %es
    movw %ax, %fs
    movw %ax, %gs
    movw %ax, %ss

    xorl %ebp, %ebp
    movl $MULTIBOOT_BOOTLOADER_MAGIC, %eax
    movl $MULTIBOOT_INFO_ADDR, %ebx
    movl KERNEL_ENTRY_ADDR, %ecx
    jmp *%ecx

.align 8
gdt_start:
    .quad 0x0000000000000000
    .quad 0x00cf9a000000ffff
    .quad 0x00cf92000000ffff
gdt_end:

gdt_descriptor:
    .word gdt_end - gdt_start - 1
    .long gdt_start

.size _start, . - _start
.section .note.GNU-stack,"",@progbits
