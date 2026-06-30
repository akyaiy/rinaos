mod acpi;
mod gdt;
mod idt;
mod interrupts;

pub fn init(boot_info: &crate::boot::BootInfo) {
    gdt::init();
    idt::init();
    interrupts::init(boot_info);
}
