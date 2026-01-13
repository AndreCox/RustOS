use super::task::Task;
use crate::alloc::collections::VecDeque;
use lazy_static::lazy_static;
use spin::Mutex;

pub struct Scheduler {
    tasks: VecDeque<Task>,
    pub current_task: Option<Task>,
}
impl Scheduler {
    pub fn new() -> Self {
        Scheduler {
            tasks: VecDeque::new(),
            current_task: None,
        }
    }

    pub fn add_task(&mut self, task: Task) {
        x86_64::instructions::interrupts::without_interrupts(|| {
            self.tasks.push_back(task);
        });
    }

    pub fn get_next_task(&mut self) -> Option<Task> {
        self.tasks.pop_front()
    }

    pub fn schedule(&mut self, stack_pointer: u64) -> u64 {
        // if task was running save its state
        if let Some(mut task) = self.current_task.take() {
            task.stack_pointer = stack_pointer;
            self.tasks.push_back(task);
        }

        // get next task to run
        if let Some(next_task) = self.get_next_task() {
            let next_stack_pointer = next_task.stack_pointer;
            self.current_task = Some(next_task);
            next_stack_pointer
        } else {
            stack_pointer // No task to switch to, return the same stack pointer
        }
    }
}

// this lazy_static delays the initialization of the SCHEDULER until it's first accessed
// since we use the heap we need to wait until after the allocator is set up
lazy_static! {
    pub static ref SCHEDULER: Mutex<Scheduler> = Mutex::new(Scheduler {
        tasks: VecDeque::new(),
        current_task: None,
    });
}
