use std::{
    cell::{RefCell, UnsafeCell},
    collections::{HashMap, HashSet},
    hash::{Hash, Hasher},
    ops::{Deref, DerefMut},
    time::{Duration, Instant},
};

use super::{commands::CommandTrait, conditions::ConditionalScheduler, Command, WrongThreadError};

pub type SubsystemSUID = u64;

struct ManagerQueue {
    cmd_queue: Vec<Command>,
    cond_queue: Vec<ConditionalScheduler>,
}
thread_local! {
    static MANAGER_QUEUE: RefCell<Option<ManagerQueue>> = RefCell::new(None);
}

/// Puts a command in the queue to be scheduled next time the scheduler runs
///
/// # Errors
/// - [`WrongThreadError`] if the current thread does not have a command manager
pub fn schedule(command: Command) -> Result<(), WrongThreadError> {
    MANAGER_QUEUE.with(|queue| {
        if let Some(queue) = &mut *queue.borrow_mut() {
            queue.cmd_queue.push(command);
            Ok(())
        } else {
            Err(WrongThreadError(
                "Can only schedule commands on a thread that has a command manager",
            ))
        }
    })
}

/// Puts a conditional scheduler in the queue to be added next time the scheduler runs
///
/// # Errors
/// - [`WrongThreadError`] if the current thread does not have a command manager
pub(crate) fn add_cond_scheduler(scheduler: ConditionalScheduler) -> Result<(), WrongThreadError> {
    MANAGER_QUEUE.with(|queue| {
        if let Some(queue) = &mut *queue.borrow_mut() {
            queue.cond_queue.push(scheduler);
            Ok(())
        } else {
            Err(WrongThreadError(
                "Can only schedule commands on a thread that has a command manager",
            ))
        }
    })
}

pub trait Subsystem {
    /// The name of the subsystem, mainly used for logging
    /// but also has to be unique for the subsystem to be registered.
    fn name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }

    /// Constructs the subsystem, called when the subsystem is registered.
    fn construct() -> Self;

    /// Called every cycle by the command manager.
    fn periodic(&self, _: Duration) {}

    /// The default command for the subsystem, if any.
    /// The default command is scheduled whenever no other command is scheduled for the subsystem.
    fn default_command(&mut self) -> Option<Command> {
        None
    }

    /// Called every cycle by the command manager, after [`periodic`](Subsystem::periodic).
    /// Helps with cluttering the [`periodic`](Subsystem::periodic) function.
    fn log(&self) {}

    /// A unique identifier for the subsystem. Only used internally.
    fn suid(&self) -> SubsystemSUID {
        let mut hasher = fxhash::FxHasher::default();
        self.name().hash(&mut hasher);
        hasher.finish()
    }
}

pub trait SubsystemRequirement {
    fn suid(&self) -> SubsystemSUID;
}
impl<T: Subsystem> SubsystemRequirement for SubsystemCell<T> {
    fn suid(&self) -> SubsystemSUID {
        self.get().suid()
    }
}

/// This type completely disregards mutable safety as it cannot be transferred between threads.
/// This type allows the user to handle subsystems similar to java with many mutable references
/// to shared data.
/// This type is also not really an RC, it will never fully drop the subsystem to prevent
/// null pointer errors in the manager.
#[derive(Debug)]
pub struct SubsystemCell<T: Subsystem + 'static>(&'static UnsafeCell<T>);

impl<T: Subsystem + 'static> Clone for SubsystemCell<T> {
    fn clone(&self) -> Self {
        Self(self.0)
    }
}
impl<T: Subsystem + 'static> Copy for SubsystemCell<T> {}

impl<T: Subsystem> Deref for SubsystemCell<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.0.get() }
    }
}

impl<T: Subsystem> DerefMut for SubsystemCell<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.0.get() }
    }
}

impl<T: Subsystem + 'static> SubsystemCell<T> {
    /// Creates a new subsystem cell, immortalizes the subsystem and registers it with the command manager.
    ///
    /// # Panics
    /// If the subsystem is already registered with the command manager.
    #[must_use]
    pub fn generate(manager: &mut CommandManager) -> Self {
        let slf = Self(Box::leak(Box::new(UnsafeCell::new(T::construct()))));
        tracing::debug!("Constructed subsystem: {}", slf.name());
        manager
            .register_subsystem(&slf, slf.get_mut().default_command())
            .expect("Subsystem already registered");
        slf
    }
}
impl<T: Subsystem + 'static> SubsystemCell<T> {
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn get(&self) -> &T {
        unsafe { &*self.0.get() }
    }
    #[must_use]
    #[allow(clippy::mut_from_ref)]
    /// Allows for interior mutability of the subsystem.
    ///
    /// # Safety
    /// This breaks rust's mutability rules, you can have multiple mutable references to the subsystem.
    /// Multiple mutable references to the subsystem can cause undefined behavior if not handled correctly.
    /// Subsystems are not Send or Sync so this should not be a problem.
    pub fn get_mut(&self) -> &mut T {
        unsafe { &mut *self.0.get() }
    }
    #[must_use]
    #[doc(hidden)]
    #[allow(clippy::missing_const_for_fn)]
    pub(crate) unsafe fn immortal_mut(&self) -> *mut T {
        self.0.get()
    }
}

use thiserror::Error;
#[derive(Debug, Clone, Copy, Error)]
pub enum CommandManagerError {
    #[error("Subsystem already registered")]
    SubsystemAlreadyRegistered,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommandIndex {
    DefaultCommand(usize),
    Command(usize),
    PreservedCommand(usize),
}

use fxhash::{FxHashMap, FxHashSet};

pub struct CommandManager {
    periodic_callbacks: Vec<(Box<dyn FnMut(Duration)>, Option<Instant>)>,
    commands: Vec<Option<Command>>,
    default_commands: Vec<Option<Command>>,
    preserved_commands: Vec<Option<Command>>,
    interrupt_state: FxHashMap<CommandIndex, bool>,
    subsystem_to_default: FxHashMap<SubsystemSUID, CommandIndex>,
    requirements: FxHashMap<SubsystemSUID, CommandIndex>,
    initialized_commands: FxHashSet<CommandIndex>,
    orphaned_commands: FxHashSet<CommandIndex>,
    cond_schedulers: Vec<ConditionalScheduler>,
}
impl CommandManager {
    #[must_use]
    pub fn new() -> Self {
        MANAGER_QUEUE.with(|queue| {
            *queue.borrow_mut() = Some(ManagerQueue {
                cmd_queue: Vec::new(),
                cond_queue: Vec::new(),
            });
        });
        Self {
            periodic_callbacks: Vec::new(),
            commands: Vec::new(),
            default_commands: Vec::new(),
            preserved_commands: Vec::new(),
            interrupt_state: HashMap::with_hasher(fxhash::FxBuildHasher::default()),
            subsystem_to_default: HashMap::with_hasher(fxhash::FxBuildHasher::default()),
            requirements: HashMap::with_hasher(fxhash::FxBuildHasher::default()),
            initialized_commands: HashSet::with_hasher(fxhash::FxBuildHasher::default()),
            orphaned_commands: HashSet::with_hasher(fxhash::FxBuildHasher::default()),
            cond_schedulers: Vec::new(),
        }
    }

    /// Registers a subsystem with the command manager. The subsystem will be polled every scheduler run.
    ///
    /// # Errors
    /// - [`CommandManagerError::SubsystemAlreadyRegistered`] if the subsystem has already been registered.
    pub(crate) fn register_subsystem<T: Subsystem + 'static>(
        &mut self,
        subsystem: &SubsystemCell<T>,
        default_command: Option<Command>,
    ) -> Result<(), CommandManagerError> {
        if self.subsystem_to_default.contains_key(&subsystem.suid()) {
            return Err(CommandManagerError::SubsystemAlreadyRegistered);
        }
        let immortal_mut = unsafe { subsystem.immortal_mut() };
        self.periodic_callbacks.push((
            Box::new(move |dt| unsafe {
                (&mut *immortal_mut).periodic(dt);
            }),
            None,
        ));
        self.default_commands.push(default_command);
        let idx = self.default_commands.len() - 1;
        self.subsystem_to_default
            .insert(subsystem.suid(), CommandIndex::DefaultCommand(idx));
        self.interrupt_state
            .insert(CommandIndex::DefaultCommand(idx), false);
        tracing::debug!("Registered subsystem: {}", subsystem.name());
        Ok(())
    }

    fn add_command(&mut self, command: Command) -> CommandIndex {
        if let Some(index) = self.commands.iter().position(Option::is_none) {
            self.commands[index] = Some(command);
            let cmd_idx = CommandIndex::Command(index);
            self.interrupt_state.insert(cmd_idx, false);
            cmd_idx
        } else {
            self.commands.push(Some(command));
            let cmd_idx = CommandIndex::Command(self.commands.len() - 1);
            self.interrupt_state.insert(cmd_idx, false);
            cmd_idx
        }
    }

    fn get_command(&mut self, index: CommandIndex) -> Option<&mut Command> {
        match index {
            CommandIndex::Command(idx) => self.commands.get_mut(idx).and_then(Option::as_mut),
            CommandIndex::DefaultCommand(idx) => {
                self.default_commands.get_mut(idx).and_then(Option::as_mut)
            }
            CommandIndex::PreservedCommand(idx) => self
                .preserved_commands
                .get_mut(idx)
                .and_then(Option::as_mut),
        }
    }

    pub fn schedule(&mut self, command: Command) {
        let index = self.add_command(command);
        self.inner_schedule(index);
    }

    fn inner_schedule(&mut self, index: CommandIndex) {
        let req = &self
            .get_command(index)
            .expect("Internal State Error: Command not found")
            .get_requirements()[..];
        if req.is_empty() {
            self.orphaned_commands.insert(index);
        } else {
            let mut can_cancel = true;
            let mut to_cancel = HashSet::with_capacity(req.len());
            for requirement in req {
                if let Some(index) = self.requirements.get_mut(requirement) {
                    let index = *index;
                    if let Some(cmd) = self.get_command(index) {
                        if can_cancel && cmd.cancel_incoming() {
                            can_cancel = false;
                            break;
                        }
                        to_cancel.insert(index);
                    }
                }
            }
            if can_cancel {
                for index in to_cancel {
                    self.interrupt_state.insert(index, true);
                }
                for requirement in req {
                    self.requirements.insert(*requirement, index);
                }
                self.interrupt_state.insert(index, false);
            }
        }
    }

    pub(crate) fn remove_command(&mut self, command_idx: CommandIndex) {
        self.initialized_commands.remove(&command_idx);
        self.interrupt_state.remove(&command_idx);

        let command = match self.get_command(command_idx) {
            Some(command) => command,
            None => return,
        };
        let requirements = command.get_requirements();
        if let CommandIndex::Command(idx) = command_idx {
            self.commands[idx] = None;
        }
        self.orphaned_commands.remove(&command_idx);
        requirements.iter().for_each(|req| {
            self.requirements.remove(req);
        });
    }

    pub(crate) fn add_cond_scheduler(&mut self, mut scheduler: ConditionalScheduler) {
        if let Some(idx) = self.commands.iter().position(Option::is_none) {
            let index = CommandIndex::PreservedCommand(idx);
            self.interrupt_state.insert(index, false);
            let cmd = scheduler.exchange(index);
            self.preserved_commands[idx] = Some(cmd);
        } else {
            let index = CommandIndex::PreservedCommand(self.preserved_commands.len());
            self.interrupt_state.insert(index, false);
            let cmd = scheduler.exchange(index);
            self.preserved_commands.push(Some(cmd));
        }
        self.cond_schedulers.push(scheduler);
    }

    pub fn clear_conditional_schedulers(&mut self) {
        self.cond_schedulers.clear();
    }
}

/// Action methods
impl CommandManager {
    /// Will run all periodic callbacks, run all conditional schedulers, init all un-initialized commands, and run all commands
    /// in that order.
    pub fn run(&mut self) {
        self.update();
        self.run_subsystems();
        self.run_cond_schedulers();
        self.run_commands();
        tracing::trace!("Ran command scheduler");
    }

    fn update(&mut self) {
        MANAGER_QUEUE.with(|queue| {
            if let Some(queue) = &mut *queue.borrow_mut() {
                queue.cmd_queue.drain(..).for_each(|command| {
                    let index = self.add_command(command);
                    self.inner_schedule(index);
                });
                queue.cond_queue.drain(..).for_each(|scheduler| {
                    self.add_cond_scheduler(scheduler);
                });
            }
        });
    }

    fn run_subsystems(&mut self) {
        for callback in &mut self.periodic_callbacks {
            if let Some(last_run) = callback.1 {
                let dt = last_run.elapsed();
                callback.0(dt);
            } else {
                callback.0(Duration::from_secs(0));
            }
            callback.1 = Some(Instant::now());
        }
        for (suid, cmd_idx) in &self.subsystem_to_default {
            if !self.requirements.contains_key(suid) {
                self.requirements.insert(*suid, *cmd_idx);
            }
        }
    }

    fn run_cond_schedulers(&mut self) {
        let to_schedule = self
            .cond_schedulers
            .iter_mut()
            .filter_map(ConditionalScheduler::poll)
            .collect::<Vec<_>>();
        for index in to_schedule {
            self.inner_schedule(index);
        }
    }

    fn run_commands(&mut self) {
        let mut to_remove: Vec<CommandIndex> = Vec::new();
        let mut cmds = self.requirements.values().collect::<Vec<&CommandIndex>>();
        cmds.extend(self.orphaned_commands.iter());

        for index in cmds {
            if let Some(command) = match index {
                CommandIndex::Command(cmd) => &mut self.commands[*cmd],
                CommandIndex::DefaultCommand(cmd) => &mut self.default_commands[*cmd],
                CommandIndex::PreservedCommand(cmd) => &mut self.preserved_commands[*cmd],
            } {
                if self.interrupt_state[index] {
                    command.end(true);
                    to_remove.push(*index);
                    continue;
                }
                if !self.initialized_commands.contains(index) {
                    command.init();
                    self.initialized_commands.insert(*index);
                }
                //TODO: Add dt to periodic
                command.periodic(Duration::from_secs(0));
                if command.is_finished() {
                    command.end(false);
                    to_remove.push(*index);
                }
            }
        }
        for index in to_remove {
            self.remove_command(index);
        }
    }
}












impl Default for CommandManager {
    fn default() -> Self {
        Self::new()
    }
}
impl Drop for CommandManager {
    fn drop(&mut self) {
        tracing::debug!("Dropping command manager");
        MANAGER_QUEUE.with(|queue| {
            *queue.borrow_mut() = None;
        });
    }
}