use std::{collections::HashSet, fmt::Debug, time::Duration};

use crate::{SubsystemRequirement, SubsystemSUID};
pub type Requirement<'a> = &'a dyn SubsystemRequirement;
pub type Requirements<'a, 'b> = &'a [Requirement<'b>];

pub trait CommandTrait {
    /// Called when the command is first scheduled.
    fn init(&mut self) {}

    /// Called every cycle.
    fn periodic(&mut self, _: Duration) {}

    /// Called when the command is interrupted or finished.
    fn end(&mut self, _interrupted: bool) {}

    /// Called every cycle to check if the command is finished.
    fn is_finished(&mut self) -> bool {
        false
    }

    // Eventually we should move this over to Box<[SubsystemSUID]>
    /// Returns the requirements of this command.
    fn get_requirements(&self) -> Vec<SubsystemSUID> {
        Vec::new()
    }

    /// Returns true if this command should run when disabled.
    fn run_when_disabled(&self) -> bool {
        false
    }

    /// Returns true if this command should cancel incoming commands.
    fn cancel_incoming(&self) -> bool {
        false
    }

    /// Returns the name of this command.
    fn get_name(&self) -> String {
        String::from("Unnamed Command")
    }
}

/// A builder for creating commands.
/// Allows for chaining of functions to create a command.
///
/// # Examples
/// ```
/// use frclib_commands::{CommandBuilder, CommandManager};
///
/// fn main() {
///     let mut manager = CommandManager::new();
///
///     CommandBuilder::new()
///         .init(|| println!("Init"))
///         .periodic(|period| {
///             println!("Periodic: {:?}", period);
///         })
///        .end(|interrupted| println!("End: {}", interrupted))
///        .is_finished(|| true)
///        .build()
///        .schedule();
///
///     manager.run();
/// }
pub struct CommandBuilder {
    init: Option<Box<dyn FnMut()>>,
    periodic: Option<Box<dyn FnMut(Duration)>>,
    end: Option<Box<dyn FnMut(bool)>>,
    is_finished: Option<Box<dyn FnMut() -> bool>>,
    requirements: Vec<SubsystemSUID>,
}
impl Debug for CommandBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        f.debug_struct("CommandBuilder")
            .field("init", &self.init.is_some())
            .field("periodic", &self.periodic.is_some())
            .field("end", &self.end.is_some())
            .field("is_finished", &self.is_finished.is_some())
            .field("requirements", &self.requirements)
            .finish()
    }
}

impl CommandBuilder {
    /// Creates a new command builder with no requirements and no functions.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            init: None,
            periodic: None,
            end: None,
            is_finished: None,
            requirements: Vec::new(),
        }
    }

    /// Defines the `init` function for this command.
    /// This is a chainable function for ease of use.
    #[must_use]
    pub fn init(mut self, init: impl FnMut() + 'static) -> Self {
        self.init = Some(Box::new(init));
        self
    }

    /// Defines the `periodic` function for this command.
    /// This is a chainable function for ease of use.
    #[must_use]
    pub fn periodic(mut self, periodic: impl FnMut(Duration) + 'static) -> Self {
        self.periodic = Some(Box::new(periodic));
        self
    }

    /// Defines the `end` function for this command.
    /// This is a chainable function for ease of use.
    #[must_use]
    pub fn end(mut self, end: impl FnMut(bool) + 'static) -> Self {
        self.end = Some(Box::new(end));
        self
    }

    /// Defines the `is_finished` function for this command.
    /// This is a chainable function for ease of use.
    #[must_use]
    pub fn is_finished(mut self, is_finished: impl FnMut() -> bool + 'static) -> Self {
        self.is_finished = Some(Box::new(is_finished));
        self
    }

    #[must_use]
    pub fn with_subsystem(mut self, subsystem: Requirement) -> Self {
        self.requirements.push(subsystem.suid());
        self
    }

    #[must_use]
    pub fn with_subsystems(mut self, subsystems: Requirements) -> Self {
        self.requirements
            .extend(subsystems.iter().map(|s| s.suid()));
        self
    }

    /// Builds the command.
    /// This is not chainable and consumes the builder.
    pub fn build(self) -> Command {
        Command::Simple(SimpleCommand {
            init: self.init,
            periodic: self.periodic,
            end: self.end,
            is_finished: self.is_finished,
            requirements: self.requirements,
        })
    }
}

/// CommandBuilder methods for creating commands with different combinations of functions.
impl CommandBuilder {
    pub fn init_only(init: impl FnMut() + 'static, subsystems: Requirements) -> Command {
        Self::new().init(init).with_subsystems(subsystems).build()
    }

    pub fn periodic_only(
        periodic: impl FnMut(Duration) + 'static,
        subsystems: Requirements,
    ) -> Command {
        Self::new()
            .periodic(periodic)
            .with_subsystems(subsystems)
            .build()
    }

    pub fn end_only(end: impl FnMut(bool) + 'static, subsystems: Requirements) -> Command {
        Self::new().end(end).with_subsystems(subsystems).build()
    }

    pub fn init_periodic(
        init: impl FnMut() + 'static,
        periodic: impl FnMut(Duration) + 'static,
        subsystems: Requirements,
    ) -> Command {
        Self::new()
            .init(init)
            .periodic(periodic)
            .with_subsystems(subsystems)
            .build()
    }

    pub fn periodic_end(
        periodic: impl FnMut(Duration) + 'static,
        end: impl FnMut(bool) + 'static,
        subsystems: Requirements,
    ) -> Command {
        Self::new()
            .periodic(periodic)
            .end(end)
            .with_subsystems(subsystems)
            .build()
    }

    pub fn init_end(
        init: impl FnMut() + 'static,
        end: impl FnMut(bool) + 'static,
        subsystems: Requirements,
    ) -> Command {
        Self::new()
            .init(init)
            .end(end)
            .with_subsystems(subsystems)
            .build()
    }

    pub fn init_periodic_end(
        init: impl FnMut() + 'static,
        periodic: impl FnMut(Duration) + 'static,
        end: impl FnMut(bool) + 'static,
        subsystems: Requirements,
    ) -> Command {
        Self::new()
            .init(init)
            .periodic(periodic)
            .end(end)
            .with_subsystems(subsystems)
            .build()
    }

    pub fn run_until(
        periodic: impl FnMut(Duration) + 'static,
        is_finished: impl FnMut() -> bool + 'static,
        subsystems: Requirements,
    ) -> Command {
        Self::new()
            .is_finished(is_finished)
            .periodic(periodic)
            .with_subsystems(subsystems)
            .build()
    }

    pub fn periodic_end_until(
        is_finished: impl FnMut() -> bool + 'static,
        periodic: impl FnMut(Duration) + 'static,
        end: impl FnMut(bool) + 'static,
        subsystems: Requirements,
    ) -> Command {
        Self::new()
            .is_finished(is_finished)
            .periodic(periodic)
            .end(end)
            .with_subsystems(subsystems)
            .build()
    }

    pub fn init_periodic_until(
        init: impl FnMut() + 'static,
        is_finished: impl FnMut() -> bool + 'static,
        subsystems: Requirements,
    ) -> Command {
        Self::new()
            .init(init)
            .is_finished(is_finished)
            .with_subsystems(subsystems)
            .build()
    }

    pub fn full(
        init: impl FnMut() + 'static,
        periodic: impl FnMut(Duration) + 'static,
        end: impl FnMut(bool) + 'static,
        is_finished: impl FnMut() -> bool + 'static,
        subsystems: Requirements,
    ) -> Command {
        Self::new()
            .init(init)
            .periodic(periodic)
            .end(end)
            .is_finished(is_finished)
            .with_subsystems(subsystems)
            .build()
    }
}

pub struct SimpleCommand {
    init: Option<Box<dyn FnMut()>>,
    periodic: Option<Box<dyn FnMut(Duration)>>,
    end: Option<Box<dyn FnMut(bool)>>,
    is_finished: Option<Box<dyn FnMut() -> bool>>,
    requirements: Vec<SubsystemSUID>,
}
impl CommandTrait for SimpleCommand {
    fn init(&mut self) {
        if let Some(init) = self.init.as_mut() {
            init();
        }
    }

    fn periodic(&mut self, period: Duration) {
        if let Some(periodic) = self.periodic.as_mut() {
            periodic(period);
        }
    }

    fn end(&mut self, interrupted: bool) {
        if let Some(end) = self.end.as_mut() {
            end(interrupted);
        }
    }

    fn is_finished(&mut self) -> bool {
        self.is_finished
            .as_mut()
            .map_or(false, |is_finished| is_finished())
    }

    fn get_requirements(&self) -> Vec<SubsystemSUID> {
        self.requirements.clone()
    }
}
impl Debug for SimpleCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        f.debug_struct("SimpleBuiltCommand")
            .field("init", &self.init.is_some())
            .field("periodic", &self.periodic.is_some())
            .field("end", &self.end.is_some())
            .field("is_finished", &self.is_finished.is_some())
            .field("requirements", &self.requirements)
            .finish()
    }
}

#[derive(Clone, Copy)]
pub struct ConstCommand {
    init: Option<fn()>,
    periodic: Option<fn(Duration)>,
    end: Option<fn(bool)>,
    is_finished: Option<fn() -> bool>,
    requirements: &'static [SubsystemSUID],
}
impl CommandTrait for ConstCommand {
    fn init(&mut self) {
        if let Some(init) = self.init.as_mut() {
            init();
        }
    }

    fn periodic(&mut self, period: Duration) {
        if let Some(periodic) = self.periodic.as_mut() {
            periodic(period);
        }
    }

    fn end(&mut self, interrupted: bool) {
        if let Some(end) = self.end.as_mut() {
            end(interrupted);
        }
    }

    fn is_finished(&mut self) -> bool {
        self.is_finished
            .as_mut()
            .map_or(false, |is_finished| is_finished())
    }

    fn get_requirements(&self) -> Vec<SubsystemSUID> {
        self.requirements.to_vec()
    }
}
impl Debug for ConstCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        f.debug_struct("SimpleBuiltCommand")
            .field("init", &self.init.is_some())
            .field("periodic", &self.periodic.is_some())
            .field("end", &self.end.is_some())
            .field("is_finished", &self.is_finished.is_some())
            .field("requirements", &self.requirements)
            .finish()
    }
}

#[derive(Debug)]
pub struct ParallelCommand {
    commands: Vec<Command>,
    finished: Vec<bool>,
    requirements: HashSet<SubsystemSUID>,
    race: bool,
}
impl CommandTrait for ParallelCommand {
    fn init(&mut self) {
        for command in &mut self.commands {
            command.init();
        }
    }

    fn periodic(&mut self, period: Duration) {
        for (i, command) in self.commands.iter_mut().enumerate() {
            if !self.finished[i] {
                command.periodic(period);
                if command.is_finished() {
                    command.end(false);
                    self.finished[i] = true;
                }
            }
        }
    }

    fn end(&mut self, interrupted: bool) {
        if interrupted {
            for (i, command) in self.commands.iter_mut().enumerate() {
                if !self.finished[i] {
                    command.end(true);
                    self.finished[i] = true;
                }
            }
        }
    }

    fn is_finished(&mut self) -> bool {
        if self.race {
            self.finished.iter().any(|&finished| finished)
        } else {
            self.finished.iter().all(|&finished| finished)
        }
    }

    fn get_requirements(&self) -> Vec<SubsystemSUID> {
        self.requirements.clone().into_iter().collect()
    }

    fn get_name(&self) -> String {
        self.commands
            .iter()
            .map(CommandTrait::get_name)
            .collect::<Vec<_>>()
            .join(",")
    }
}

#[derive(Debug)]
pub struct SequentialCommand {
    commands: Vec<Command>,
    current: usize,
    requirements: HashSet<SubsystemSUID>,
}
impl SequentialCommand {
    fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }
}
impl CommandTrait for SequentialCommand {
    fn init(&mut self) {
        if self.is_empty() {
            return;
        }
        self.current = 0;
        self.commands[0].init();
    }

    fn periodic(&mut self, period: Duration) {
        if self.is_empty() {
            return;
        }
        self.commands[self.current].periodic(period);
        if self.commands[self.current].is_finished() {
            self.commands[self.current].end(false);
            self.current += 1;
            if self.current < self.commands.len() {
                self.commands[self.current].init();
            }
        }
    }

    fn end(&mut self, interrupted: bool) {
        if interrupted {
            if let Some(command) = self.commands.get_mut(self.current) {
                command.end(true);
            }
        }
    }

    fn is_finished(&mut self) -> bool {
        self.current >= self.commands.len()
    }

    fn get_requirements(&self) -> Vec<SubsystemSUID> {
        self.requirements.clone().into_iter().collect()
    }

    fn get_name(&self) -> String {
        self.commands
            .iter()
            .map(CommandTrait::get_name)
            .collect::<Vec<_>>()
            .join("->")
    }
}

pub struct ProxyCommand {
    command_supplier: Box<dyn FnMut() -> Command>,
    command: Option<Box<Command>>,
    requirements: HashSet<SubsystemSUID>,
}
impl ProxyCommand {
    fn get_command(&mut self) -> &mut Command {
        if self.command.is_none() {
            self.command = Some(Box::new((self.command_supplier)()));
        }
        self.command.as_mut().expect("Command Empty")
    }
}
impl CommandTrait for ProxyCommand {
    fn init(&mut self) {
        self.command = None;
        self.get_command().init();
    }

    fn periodic(&mut self, period: Duration) {
        self.get_command().periodic(period);
    }

    fn end(&mut self, interrupted: bool) {
        self.get_command().end(interrupted);
    }

    fn is_finished(&mut self) -> bool {
        self.get_command().is_finished()
    }

    fn get_requirements(&self) -> Vec<SubsystemSUID> {
        self.requirements.iter().copied().collect()
    }

    fn get_name(&self) -> String {
        self.command.as_ref().map_or_else(
            || String::from("ProxyCommand(?)"),
            |c| format!("ProxyCommand({})", c.get_name()),
        )
    }
}
impl Debug for ProxyCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        let mut dbg_struct = f.debug_struct("ProxyCommand");
        if let Some(command) = &self.command {
            dbg_struct
                .field("command", command)
                .finish_non_exhaustive()?;
        } else {
            dbg_struct
                .field("command", &"None")
                .finish_non_exhaustive()?;
        };
        Ok(())
    }
}
#[allow(missing_copy_implementations)]
#[derive(Debug)]
pub struct WaitCommand {
    start_instant: Option<std::time::Instant>,
    duration: std::time::Duration,
}
impl CommandTrait for WaitCommand {
    fn init(&mut self) {
        self.start_instant = Some(std::time::Instant::now());
    }

    fn periodic(&mut self, _: Duration) {}

    fn end(&mut self, _interrupted: bool) {}

    fn is_finished(&mut self) -> bool {
        self.start_instant.expect("Command Empty").elapsed() >= self.duration
    }

    fn get_requirements(&self) -> Vec<SubsystemSUID> {
        vec![]
    }

    fn get_name(&self) -> String {
        format!("TimedCommand({:?})", self.duration)
    }
}

#[derive(Debug)]
pub struct NamedCommand {
    name: String,
    command: Box<Command>,
}
impl CommandTrait for NamedCommand {
    fn init(&mut self) {
        self.command.init();
    }

    fn periodic(&mut self, period: Duration) {
        self.command.periodic(period);
    }

    fn end(&mut self, interrupted: bool) {
        self.command.end(interrupted);
    }

    fn is_finished(&mut self) -> bool {
        self.command.is_finished()
    }

    fn get_requirements(&self) -> Vec<SubsystemSUID> {
        self.command.get_requirements()
    }

    fn get_name(&self) -> String {
        self.name.clone()
    }
}

#[derive(Debug)]
pub struct ExtraRequirementsCommand {
    command: Box<Command>,
    requirements: Vec<SubsystemSUID>,
}
impl CommandTrait for ExtraRequirementsCommand {
    fn init(&mut self) {
        self.command.init();
    }

    fn periodic(&mut self, period: Duration) {
        self.command.periodic(period);
    }

    fn end(&mut self, interrupted: bool) {
        self.command.end(interrupted);
    }

    fn is_finished(&mut self) -> bool {
        self.command.is_finished()
    }

    fn get_requirements(&self) -> Vec<SubsystemSUID> {
        self.command
            .get_requirements()
            .into_iter()
            .chain(self.requirements.iter().copied())
            .collect()
    }

    fn get_name(&self) -> String {
        self.command.get_name()
    }
}

#[must_use]
pub enum Command {
    Parallel(ParallelCommand),
    Sequential(SequentialCommand),
    Simple(SimpleCommand),
    Const(ConstCommand),
    Custom(Box<dyn CommandTrait>),
    Named(NamedCommand),
    Wait(WaitCommand),
    Proxy(ProxyCommand),
    ExtraRequirments(ExtraRequirementsCommand),
}
impl Debug for Command {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        match self {
            Self::Parallel(command) => f
                .debug_struct("Parallel")
                .field("command", command)
                .finish(),
            Self::Sequential(command) => f
                .debug_struct("Sequential")
                .field("command", command)
                .finish(),
            Self::Simple(command) => f.debug_struct("Simple").field("command", command).finish(),
            Self::Const(command) => f.debug_struct("Const").field("command", command).finish(),
            Self::Custom(_) => f.debug_struct("Custom").finish(),
            Self::Named(command) => f.debug_struct("Named").field("command", command).finish(),
            Self::Wait(command) => f.debug_struct("Wait").field("command", command).finish(),
            Self::Proxy(command) => f.debug_struct("Proxy").field("command", command).finish(),
            Self::ExtraRequirments(command) => f
                .debug_struct("ExtraRequirments")
                .field("command", command)
                .finish(),
        }
    }
}
impl CommandTrait for Command {
    fn init(&mut self) {
        match self {
            Self::Parallel(command) => command.init(),
            Self::Sequential(command) => command.init(),
            Self::Simple(command) => command.init(),
            Self::Const(command) => command.init(),
            Self::Custom(command) => command.init(),
            Self::Named(command) => command.init(),
            Self::Wait(command) => command.init(),
            Self::Proxy(command) => command.init(),
            Self::ExtraRequirments(command) => command.init(),
        }
    }

    fn periodic(&mut self, period: Duration) {
        match self {
            Self::Parallel(command) => command.periodic(period),
            Self::Sequential(command) => command.periodic(period),
            Self::Simple(command) => command.periodic(period),
            Self::Const(command) => command.periodic(period),
            Self::Custom(command) => command.periodic(period),
            Self::Named(command) => command.periodic(period),
            Self::Wait(command) => command.periodic(period),
            Self::Proxy(command) => command.periodic(period),
            Self::ExtraRequirments(command) => command.periodic(period),
        }
    }

    fn end(&mut self, interrupted: bool) {
        match self {
            Self::Parallel(command) => command.end(interrupted),
            Self::Sequential(command) => command.end(interrupted),
            Self::Simple(command) => command.end(interrupted),
            Self::Const(command) => command.end(interrupted),
            Self::Custom(command) => command.end(interrupted),
            Self::Named(command) => command.end(interrupted),
            Self::Wait(command) => command.end(interrupted),
            Self::Proxy(command) => command.end(interrupted),
            Self::ExtraRequirments(command) => command.end(interrupted),
        }
    }

    fn is_finished(&mut self) -> bool {
        match self {
            Self::Parallel(command) => command.is_finished(),
            Self::Sequential(command) => command.is_finished(),
            Self::Simple(command) => command.is_finished(),
            Self::Const(command) => command.is_finished(),
            Self::Custom(command) => command.is_finished(),
            Self::Named(command) => command.is_finished(),
            Self::Wait(command) => command.is_finished(),
            Self::Proxy(command) => command.is_finished(),
            Self::ExtraRequirments(command) => command.is_finished(),
        }
    }

    fn get_requirements(&self) -> Vec<SubsystemSUID> {
        match self {
            Self::Parallel(command) => command.get_requirements(),
            Self::Sequential(command) => command.get_requirements(),
            Self::Simple(command) => command.get_requirements(),
            Self::Const(command) => command.get_requirements(),
            Self::Custom(command) => command.get_requirements(),
            Self::Named(command) => command.get_requirements(),
            Self::Wait(command) => command.get_requirements(),
            Self::Proxy(command) => command.get_requirements(),
            Self::ExtraRequirments(command) => command.get_requirements(),
        }
    }

    fn get_name(&self) -> String {
        match self {
            Self::Parallel(command) => command.get_name(),
            Self::Sequential(command) => command.get_name(),
            Self::Simple(command) => command.get_name(),
            Self::Const(command) => command.get_name(),
            Self::Custom(command) => command.get_name(),
            Self::Named(command) => command.get_name(),
            Self::Wait(command) => command.get_name(),
            Self::Proxy(command) => command.get_name(),
            Self::ExtraRequirments(command) => command.get_name(),
        }
    }
}

impl Command {
    /// Constructs a Parallel Command of self and other
    pub fn along_with(self, other: Self) -> Self {
        Self::Parallel(ParallelCommand {
            requirements: self
                .get_requirements()
                .into_iter()
                .chain(other.get_requirements())
                .collect(),
            commands: vec![self, other],
            finished: vec![false, false],
            race: false,
        })
    }

    /// Constructs a Parallel Command of self and others
    pub fn along_with_many(self, others: Vec<Self>) -> Self {
        let mut commands = vec![self];
        commands.extend(others);
        Self::Parallel(ParallelCommand {
            finished: vec![false; commands.len()],
            requirements: commands
                .iter()
                .flat_map(CommandTrait::get_requirements)
                .collect(),
            commands,
            race: false,
        })
    }

    /// Constructs a Parallel Command of self and other that will finish when one of them finishes
    pub fn race_with(self, other: Self) -> Self {
        Self::Parallel(ParallelCommand {
            requirements: self
                .get_requirements()
                .into_iter()
                .chain(other.get_requirements())
                .collect(),
            commands: vec![self, other],
            finished: vec![false, false],
            race: true,
        })
    }

    /// Constructs a Parallel Command of self and others that will finish when one of them finishes
    pub fn race_with_many(self, others: Vec<Self>) -> Self {
        let mut commands = vec![self];
        commands.extend(others);
        Self::Parallel(ParallelCommand {
            finished: vec![false; commands.len()],
            requirements: commands
                .iter()
                .flat_map(CommandTrait::get_requirements)
                .collect(),
            commands,
            race: true,
        })
    }

    pub fn timeout(self, duration: Duration) -> Self {
        self.race_with(Command::wait_for(duration))
    }

    /// Constructs a Sequential Command of self and other,
    /// self will run first then other will run
    pub fn before(self, other: Self) -> Self {
        Self::Sequential(SequentialCommand {
            requirements: self
                .get_requirements()
                .into_iter()
                .chain(other.get_requirements())
                .collect(),
            commands: vec![self, other],
            current: 0,
        })
    }

    /// Constructs a Sequential Command of self and other,
    /// other will run first then self will run
    pub fn after(self, other: Self) -> Self {
        Self::Sequential(SequentialCommand {
            requirements: self
                .get_requirements()
                .into_iter()
                .chain(other.get_requirements())
                .collect(),
            commands: vec![other, self],
            current: 0,
        })
    }

    /// Constructs a Sequential Command of self and others,
    /// self will run first then others will run
    pub fn and_then_many(self, others: Vec<Self>) -> Self {
        let mut commands = vec![self];
        commands.extend(others);
        Self::Sequential(SequentialCommand {
            requirements: commands
                .iter()
                .flat_map(CommandTrait::get_requirements)
                .collect(),
            commands,
            current: 0,
        })
    }

    /// Constructs a Named Command of self with the given name
    pub fn with_name(self, name: &impl ToString) -> Self {
        Self::Named(NamedCommand {
            name: name.to_string(),
            command: Box::new(self),
        })
    }

    /// Constructs a ExtraRequirements Command of self with the given requirements
    pub fn with_extra_requirements(self, subsystems: Requirements) -> Self {
        Self::ExtraRequirments(ExtraRequirementsCommand {
            command: Box::new(self),
            requirements: subsystems.iter().map(|s| s.suid()).collect(),
        })
    }

    /// Constructs a Wait Command that will wait for the given seconds
    pub fn wait_for(duration: Duration) -> Self {
        Self::Wait(WaitCommand {
            duration,
            start_instant: None,
        })
    }

    /// Creates a wrapper around a custom defined command
    pub fn custom(command: Box<dyn CommandTrait>) -> Self {
        Self::Custom(command)
    }

    /// Creates an empty command with no requirements
    pub fn empty() -> Self {
        CommandBuilder::init_only(|| {}, &[])
    }

    /// Creates a command that will run the given commands in "parallel" every cycle.
    /// This command will adopt all requirements of the given commands.
    ///
    /// This command will finish when all of the given commands finish.
    ///
    /// The commands do not actually run in parallel,
    /// they run sequentially in the order they are given but they are all run every cycle
    /// unlike a sequential command where only one command is run every cycle.
    pub fn parallel(commands: Vec<Command>) -> Command {
        Command::Parallel(ParallelCommand {
            finished: vec![false; commands.len()],
            requirements: commands
                .iter()
                .flat_map(CommandTrait::get_requirements)
                .collect(),
            commands,
            race: false,
        })
    }

    pub fn race(commands: Vec<Command>) -> Command {
        Command::Parallel(ParallelCommand {
            finished: vec![false; commands.len()],
            requirements: commands
                .iter()
                .flat_map(CommandTrait::get_requirements)
                .collect(),
            commands,
            race: true,
        })
    }

    pub fn sequential(commands: Vec<Command>) -> Command {
        Command::Sequential(SequentialCommand {
            requirements: commands
                .iter()
                .flat_map(CommandTrait::get_requirements)
                .collect(),
            commands,
            current: 0,
        })
    }

    /// Schedule this command to run
    ///
    /// # Panics
    /// If this is called in a thread that does not have a command manager.
    /// If you want to handle this error use [`Command::try_schedule`]
    pub fn schedule(self) {
        super::manager::schedule(self)
            .expect("Failed to schedule command, command requirements not met");
    }

    /// Schedule this command to run
    ///
    /// # Errors
    /// - [`super::manager::WrongThreadError`] if this is called in a thread that does not have a command manager.
    pub fn try_schedule(self) -> Result<(), super::WrongThreadError> {
        super::manager::schedule(self)
    }
}
impl Default for Command {
    fn default() -> Self {
        Self::empty()
    }
}
impl From<SimpleCommand> for Command {
    fn from(command: SimpleCommand) -> Self {
        Self::Simple(command)
    }
}
impl From<ParallelCommand> for Command {
    fn from(command: ParallelCommand) -> Self {
        Self::Parallel(command)
    }
}
impl From<SequentialCommand> for Command {
    fn from(command: SequentialCommand) -> Self {
        Self::Sequential(command)
    }
}
impl From<WaitCommand> for Command {
    fn from(command: WaitCommand) -> Self {
        Self::Wait(command)
    }
}
impl From<ProxyCommand> for Command {
    fn from(command: ProxyCommand) -> Self {
        Self::Proxy(command)
    }
}
impl From<Box<dyn CommandTrait>> for Command {
    fn from(command: Box<dyn CommandTrait>) -> Self {
        Self::Custom(command)
    }
}
impl From<NamedCommand> for Command {
    fn from(command: NamedCommand) -> Self {
        Self::Named(command)
    }
}
impl From<CommandBuilder> for Command {
    fn from(command: CommandBuilder) -> Self {
        command.build()
    }
}
impl From<Command> for Box<dyn CommandTrait> {
    fn from(command: Command) -> Self {
        match command {
            Command::Parallel(command) => Box::new(command),
            Command::Sequential(command) => Box::new(command),
            Command::Simple(command) => Box::new(command),
            Command::Const(command) => Box::new(command),
            Command::Custom(command) => command,
            Command::Named(command) => Box::new(command),
            Command::Wait(command) => Box::new(command),
            Command::Proxy(command) => Box::new(command),
            Command::ExtraRequirments(command) => Box::new(command),
        }
    }
}
