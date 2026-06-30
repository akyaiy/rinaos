use core::arch::global_asm;

#[repr(C)]
#[derive(Default)]
pub struct Context {
    rsp: u64,
    rbp: u64,
    rbx: u64,
    r12: u64,
    r13: u64,
    r14: u64,
    r15: u64,
}

extern "C" {
    pub fn switch_context(old: *mut Context, new: *const Context);
}

global_asm!(
    r#"
    .global switch_context
    .type switch_context, @function
switch_context:
    mov [rdi + 0x00], rsp
    mov [rdi + 0x08], rbp
    mov [rdi + 0x10], rbx
    mov [rdi + 0x18], r12
    mov [rdi + 0x20], r13
    mov [rdi + 0x28], r14
    mov [rdi + 0x30], r15

    mov rsp, [rsi + 0x00]
    mov rbp, [rsi + 0x08]
    mov rbx, [rsi + 0x10]
    mov r12, [rsi + 0x18]
    mov r13, [rsi + 0x20]
    mov r14, [rsi + 0x28]
    mov r15, [rsi + 0x30]
    ret
    .size switch_context, . - switch_context
    "#
);

impl Context {
    pub const fn empty() -> Self {
        Self {
            rsp: 0,
            rbp: 0,
            rbx: 0,
            r12: 0,
            r13: 0,
            r14: 0,
            r15: 0,
        }
    }

    pub fn set_stack_entry(&mut self, stack_top: usize, entry: extern "C" fn() -> !) {
        let mut rsp = stack_top & !0xf;
        rsp -= core::mem::size_of::<usize>() * 2;

        unsafe {
            (rsp as *mut usize).write(entry as usize);
        }

        self.rsp = rsp as u64;
    }
}
