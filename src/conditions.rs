use std::{cell::Cell, fmt::Debug, rc::Rc};

use super::{Command, CommandIndex};

pub trait BooleanSupplier {
    fn get_as_boolean(&self) -> bool;
}
impl<F: Fn() -> bool> BooleanSupplier for F {
    fn get_as_boolean(&self) -> bool {
        self()
    }
}

#[derive(Debug)]
pub(crate) struct ConditionalScheduler {
    condition: Condition,
    command_slot: Option<Command>,
    idx_slot: Option<CommandIndex>,
}
impl ConditionalScheduler {
    #[must_use]
    pub const fn new(condition: Condition, command: Command) -> Self {
        Self {
            condition,
            command_slot: Some(command),
            idx_slot: None,
        }
    }
    pub fn exchange(&mut self, idx: CommandIndex) -> Command {
        self.idx_slot = Some(idx);
        self.command_slot
            .take()
            .expect("ConditionalScheduler::exchange called twice")
    }
    pub fn poll(&mut self) -> Option<CommandIndex> {
        if self.condition.get_as_boolean() {
            self.idx_slot
        } else {
            None
        }
    }
}

#[derive(Clone)]
#[allow(missing_debug_implementations)]
pub struct Condition {
    cond: Rc<dyn BooleanSupplier>,
}
impl Debug for Condition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Condition")
            .field("cond", &"dyn BooleanSupplier")
            .finish()
    }
}
impl BooleanSupplier for Condition {
    fn get_as_boolean(&self) -> bool {
        self.cond.get_as_boolean()
    }
}
impl Condition {
    pub fn new<F: Fn() -> bool + 'static>(cond: F) -> Self {
        Self {
            cond: Rc::new(cond),
        }
    }
    #[must_use]
    pub fn and<F: Fn() -> bool + 'static>(&self, cond: F) -> Self {
        let slf_cond = self.cond.clone();
        Self {
            cond: Rc::new(move || slf_cond.get_as_boolean() && cond()),
        }
    }
    #[must_use]
    pub fn or<F: Fn() -> bool + 'static>(&self, cond: F) -> Self {
        let slf_cond = self.cond.clone();
        Self {
            cond: Rc::new(move || slf_cond.get_as_boolean() | cond()),
        }
    }
    #[must_use]
    pub fn negate(&self) -> Self {
        let slf_cond = self.cond.clone();
        Self {
            cond: Rc::new(move || !slf_cond.get_as_boolean()),
        }
    }

    /// Creates a conditional scheduler that will run the given command on the rising edge of the condition.
    /// The command will only run once per rising edge.
    ///
    /// # Panics
    /// Panics if the conditional scheduler cannot be added to the scheduler manager
    /// due to being on a different thread.
    #[allow(clippy::return_self_not_must_use, clippy::must_use_candidate)]
    pub fn on_true(&self, command: Command) -> Self {
        //create a condition is true if last poll was false and current poll is true
        let last_poll = Cell::new(false);
        let slf_cond = self.cond.clone();
        let condition = Self::new(move || {
            let poll = slf_cond.get_as_boolean();
            let last_poll_val = last_poll.replace(poll);
            !last_poll_val && poll
        });
        let cond_sched = ConditionalScheduler::new(condition, command);
        super::manager::add_cond_scheduler(cond_sched)
            .expect("Failed to add conditional scheduler");

        self.clone()
    }

    /// Creates a conditional scheduler that will run the given command on the falling edge of the condition.
    /// The command will only run once per falling edge.
    ///
    /// # Panics
    /// Panics if the conditional scheduler cannot be added to the scheduler manager
    /// due to being on a different thread.
    #[allow(clippy::return_self_not_must_use, clippy::must_use_candidate)]
    pub fn on_false(&self, command: Command) -> Self {
        //create a condition is true if last poll was false and current poll is true
        let last_poll = Cell::new(false);
        let slf_cond = self.cond.clone();
        let condition = Self::new(move || {
            let poll = slf_cond.get_as_boolean();
            let last_poll_val = last_poll.replace(poll);
            last_poll_val && !poll
        });
        let cond_sched = ConditionalScheduler::new(condition, command);
        super::manager::add_cond_scheduler(cond_sched)
            .expect("Failed to add conditional scheduler");

        self.clone()
    }
}
