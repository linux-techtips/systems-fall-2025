use core::arch::asm;

fn main() {
    unsafe { asm!("mov rax, 60", "svc", options(noreturn)) }
}
