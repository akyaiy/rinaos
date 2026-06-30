extern crate alloc;

mod context;
mod task;

use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use x86_64::instructions::interrupts;

use crate::sched::context::switch_context;
use crate::sched::task::{Task, TaskEntry, TaskId, TaskState};
use crate::sync::spinlock::IrqSpinLock;
use crate::{klog_debug, klog_error};

const PIC_TICKS_PER_SWITCH: u64 = 10;

static SCHEDULER: IrqSpinLock<Scheduler> = IrqSpinLock::new(Scheduler::new());
static STARTED: AtomicBool = AtomicBool::new(false);
static PIC_TICKS: AtomicU64 = AtomicU64::new(0);

struct Scheduler {
    tasks: Vec<Task>,
    current: usize,
    next_id: u64,
}

impl Scheduler {
    const fn new() -> Self {
        Self {
            tasks: Vec::new(),
            current: 0,
            next_id: 1,
        }
    }

    fn init(&mut self) {
        if !self.tasks.is_empty() {
            return;
        }

        self.tasks.push(Task::bootstrap(0));
    }

    fn spawn_kernel(&mut self, entry: TaskEntry) -> TaskId {
        let id = self.next_id;
        self.next_id += 1;
        self.tasks.push(Task::kernel(id, entry));
        id
    }

    fn next_ready(&self) -> Option<usize> {
        if self.tasks.len() <= 1 {
            return None;
        }

        for offset in 1..=self.tasks.len() {
            let index = (self.current + offset) % self.tasks.len();
            if self.tasks[index].state == TaskState::Ready {
                return Some(index);
            }
        }

        None
    }
}

pub fn init() {
    SCHEDULER.lock().init();
    STARTED.store(true, Ordering::Release);
    klog_debug!("scheduler initialized");
}

pub fn spawn_kernel(entry: fn() -> !) -> TaskId {
    SCHEDULER.lock().spawn_kernel(entry)
}

pub fn tick_pic() {
    if PIC_TICKS.fetch_add(1, Ordering::Relaxed) % PIC_TICKS_PER_SWITCH == PIC_TICKS_PER_SWITCH - 1
    {
        schedule();
    }
}

pub fn tick_local() {
    schedule();
}

pub fn yield_now() {
    schedule();
}

fn schedule() {
    if !STARTED.load(Ordering::Acquire) {
        return;
    }

    let restore_interrupts = interrupts::are_enabled();
    interrupts::disable();

    let (old_context, new_context) = {
        let mut scheduler = SCHEDULER.lock();

        let Some(next) = scheduler.next_ready() else {
            if restore_interrupts {
                interrupts::enable();
            }
            return;
        };

        let current = scheduler.current;
        if current == next {
            if restore_interrupts {
                interrupts::enable();
            }
            return;
        }

        if scheduler.tasks[current].state == TaskState::Running {
            scheduler.tasks[current].state = TaskState::Ready;
        }
        scheduler.tasks[next].state = TaskState::Running;
        scheduler.current = next;

        (
            &mut scheduler.tasks[current].context as *mut _,
            &scheduler.tasks[next].context as *const _,
        )
    };

    unsafe {
        switch_context(old_context, new_context);
    }

    if restore_interrupts {
        interrupts::enable();
    }
}

#[no_mangle]
pub extern "C" fn task_trampoline() -> ! {
    interrupts::disable();

    let (id, entry) = {
        let scheduler = SCHEDULER.lock();
        let task = &scheduler.tasks[scheduler.current];
        (task.id, task.entry)
    };

    match entry {
        Some(entry) => {
            crate::serial_println!("scheduler: starting task {}", id);
            interrupts::enable();
            entry()
        }
        None => {
            klog_error!("scheduler: task without entry");
            exit_current();
        }
    }
}

pub fn exit_current() -> ! {
    {
        let mut scheduler = SCHEDULER.lock();
        let current = scheduler.current;
        scheduler.tasks[current].state = TaskState::Exited;
    }

    loop {
        schedule();
        x86_64::instructions::hlt();
    }
}
