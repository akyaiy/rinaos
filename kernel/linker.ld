ENTRY(_start)

PHDRS
{
    text PT_LOAD FLAGS(5);
    rodata PT_LOAD FLAGS(4);
    data PT_LOAD FLAGS(6);
}

SECTIONS
{
    . = 0xffffffff80000000;

    .text ALIGN(4K) : {
        *(.text .text.*)
    } :text

    . = ALIGN(4K);

    .rodata ALIGN(4K) : {
        *(.rodata .rodata.*)
    } :rodata

    . = ALIGN(4K);

    .data ALIGN(4K) : {
        *(.data .data.*)
    } :data

    .bss : {
        *(COMMON)
        *(.bss .bss.*)
    } :data

    /DISCARD/ : {
        *(.eh_frame .eh_frame.*)
        *(.eh_frame_hdr)
        *(.note .note.*)
        *(.comment)
    }
}
