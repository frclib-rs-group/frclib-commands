use std::{
    cell::{UnsafeCell, RefCell},
    collections::{HashMap, HashSet},
    ops::{Deref, DerefMut},
    time::Duration,
};

use super::{commands::CommandTrait, Command, WrongThreadError, conditions::ConditionalScheduler};

pub type SubsystemSUID = u8;

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
            Err(WrongThreadError("Can only schedule commands on a thread that has a command manager"))
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
            Err(WrongThreadError("Can only schedule commands on a thread that has a command manager"))
        }
    })
}

pub trait SubsystemBase {
    fn periodic(&self, _: Duration) {}
}

pub trait Subsystem: SubsystemBase {
    const SUID: SubsystemSUID;

    fn name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }

    fn construct() -> Self;

    fn default_command(&mut self) -> Option<Command> {
        None
    }

    fn log(&self);
}

/// This type completely disregards mutable safety as it cannot be transferred between threads.
/// This type allows the user to handle subsystems similar to java with many mutable references
/// to shared data.
/// This type is also not really an RC, it will never fully drop the subsystem to prevent
/// null pointer errors in the manager.
#[derive(Debug)]
pub struct SubsystemCell<T: SubsystemBase + 'static>(&'static UnsafeCell<T>);

impl<T: SubsystemBase + 'static> Clone for SubsystemCell<T> {
    fn clone(&self) -> Self {
        Self(self.0)
    }
}
impl<T: SubsystemBase + 'static> Copy for SubsystemCell<T> {}


impl<T: SubsystemBase> Deref for SubsystemCell<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.0.get() }
    }
}

impl<T: SubsystemBase> DerefMut for SubsystemCell<T> {
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
        manager.register_subsystem(&slf, slf.get_mut().default_command())
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
    pub fn get_mut(&self) -> &mut T {
        unsafe { &mut *self.0.get() }
    }
    #[must_use]
    #[doc(hidden)]
    #[allow(clippy::missing_const_for_fn)]
    pub unsafe fn immortal_mut(&self) -> *mut T {
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

#[derive(Debug)]
pub struct CommandManager {
    periodic_callbacks: Vec<*mut dyn SubsystemBase>,
    commands: Vec<Option<Command>>,
    default_commands: Vec<Option<Command>>,
    preserved_commands: Vec<Option<Command>>,
    interrupt_state: HashMap<CommandIndex, bool>,
    subsystem_to_default: HashMap<SubsystemSUID, CommandIndex>,
    requirements: HashMap<SubsystemSUID, CommandIndex>,
    initialized_commands: HashSet<CommandIndex>,
    orphaned_commands: HashSet<CommandIndex>,
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
            interrupt_state: HashMap::new(),
            subsystem_to_default: HashMap::new(),
            requirements: HashMap::new(),
            initialized_commands: HashSet::new(),
            orphaned_commands: HashSet::new(),
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
        if self.subsystem_to_default.contains_key(&T::SUID) {
            return Err(CommandManagerError::SubsystemAlreadyRegistered);
        }
        self.periodic_callbacks
            .push(unsafe { subsystem.immortal_mut() });
        self.default_commands.push(default_command);
        let idx = self.default_commands.len() - 1;
        self.subsystem_to_default
            .insert(T::SUID, CommandIndex::DefaultCommand(idx));
        self.interrupt_state
            .insert(CommandIndex::DefaultCommand(idx), false);
        Ok(())
    }

    /// Will run all periodic callbacks, run all conditional schedulers, init all un-initialized commands, and run all commands
    /// in that order.
    pub fn run(&mut self) {
        self.update();
        self.run_subsystems();
        self.run_cond_schedulers();
        self.run_commands();
    }

    fn run_subsystems(&mut self) {
        for callback in &self.periodic_callbacks {
            unsafe {
                callback.as_ref()
                    .expect("Internal State Error: Subsystem not found")
                    .periodic(Duration::from_secs(0));
            }
        }
        for (suid, cmd_idx) in &self.subsystem_to_default {
            if !self.requirements.contains_key(suid) {
                self.requirements.insert(*suid, *cmd_idx);
            }
        }
    }

    fn run_cond_schedulers(&mut self) {
        let to_schedule = self.cond_schedulers
            .iter_mut()
            .filter_map(ConditionalScheduler::poll)
            .collect::<Vec<_>>();
        for index in to_schedule {
            self.inner_schedule(index);
        }
    }

    fn run_commands(&mut self) {
        let mut to_remove: Vec<usize> = Vec::new();
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
                    if let CommandIndex::Command(idx) = *index { to_remove.push(idx) }
                    continue;
                }
                if !self.initialized_commands.contains(index) {
                    command.init();
                    self.initialized_commands.insert(*index);
                }
                command.periodic(Duration::from_secs(0));
                if command.is_finished() {
                    command.end(false);
                    if let CommandIndex::Command(idx) = *index { to_remove.push(idx) }
                }
            }
        }
        for index in to_remove {
            self.initialized_commands
                .remove(&CommandIndex::Command(index));
            if let Some(cmd) = self.commands.remove(index) {
                let requirements = cmd.get_requirements();
                if requirements.is_empty() {
                    self.orphaned_commands.remove(&CommandIndex::Command(index));
                } else {
                    for req in cmd.get_requirements() {
                        self.requirements.remove(&req);
                        let idx = self.subsystem_to_default[&req];
                        self.requirements.insert(req, idx);
                    }
                }
            }
        }
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
            CommandIndex::DefaultCommand(idx) => self.default_commands.get_mut(idx).and_then(Option::as_mut),
            CommandIndex::PreservedCommand(idx) => self.preserved_commands.get_mut(idx).and_then(Option::as_mut),
        }
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

    pub fn schedule(&mut self, command: Command) {
        let index = self.add_command(command);
        self.inner_schedule(index);
    }

    fn inner_schedule(&mut self, index: CommandIndex) {
        let req = &self.get_command(index)
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

    pub fn cancel_all(&mut self) {
        for maybe_command in &mut self.commands {
            if let Some(command) = maybe_command.as_mut() {
                command.end(true);
            }
        }
        self.commands.clear();
        self.requirements.clear();
        self.initialized_commands.clear();
        self.orphaned_commands.clear();
    }

    pub(crate) fn add_cond_scheduler(&mut self, mut scheduler: ConditionalScheduler) {
        if let Some(idx) = self.commands.iter().position(Option::is_none) {
            let cmd = scheduler.exchange(CommandIndex::PreservedCommand(idx));
            self.preserved_commands[idx] = Some(cmd);
        } else {
            let cmd = scheduler.exchange(CommandIndex::Command(self.commands.len()));
            self.commands.push(Some(cmd));
        }
        self.cond_schedulers.push(scheduler);
    }

    pub fn clear_cond_schedulers(&mut self) {
        self.cond_schedulers.clear();
    }
}
impl Default for CommandManager {
    fn default() -> Self {
        Self::new()
    }
}
