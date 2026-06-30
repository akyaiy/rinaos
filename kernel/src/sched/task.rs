use alloc::vec;
use alloc::vec::Vec;

use crate::sched::context::Context;

pub type TaskId = u64;
pub type TaskEntry = fn() -> !;

pub const KERNEL_STACK_SIZE: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TaskState {
    Ready,
    Running,
    Exited,
}

pub struct Task {
    pub id: TaskId,
    pub context: Context,
    pub entry: Option<TaskEntry>,
    pub state: TaskState,
    _stack: Option<Vec<u8>>,
}

impl Task {
    pub const fn bootstrap(id: TaskId) -> Self {
        Self {
            id,
            context: Context::empty(),
            entry: None,
            state: TaskState::Running,
            _stack: None,
        }
    }

    pub fn kernel(id: TaskId, entry: TaskEntry) -> Self {
        let mut stack = vec![0; KERNEL_STACK_SIZE];
        let stack_top = stack.as_mut_ptr() as usize + KERNEL_STACK_SIZE;
        let mut context = Context::empty();

        context.set_stack_entry(stack_top, crate::sched::task_trampoline);

        Self {
            id,
            context,
            entry: Some(entry),
            state: TaskState::Ready,
            _stack: Some(stack),
        }
    }
}
