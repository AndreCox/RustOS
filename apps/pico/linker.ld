ENTRY(_start)

SECTIONS {
    /* We don't care about the absolute address since it has no globals/rodata accesses 
       but we set it to 0 just in case */
    . = 0x0;
    
    .text : {
        /* Ensure _start is at the very beginning */
        *(.text.start)
        *(.text .text.*)
    }
    
    .rodata : { *(.rodata .rodata.*) }
    .data : { *(.data .data.*) }
    .bss : { *(.bss .bss.*) }

    /DISCARD/ : {
        *(.eh_frame)
        *(.note .note.*)
    }
}
