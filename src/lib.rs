//! A library for creating and managing commands.
//! 
//! Designed to be used in a single-threaded environment and to be very lightweight.
//! 
//! # Examples
//! 
//! ## Creating a command manager and scheduling a command
//! ```
//! use frclib_commands::{CommandBuilder, CommandManager};
//! use std::time::Duration;
//! 
//! fn main() {
//!    let mut manager = CommandManager::new();
//!
//!    CommandBuilder::new()
//!       .init(|| println!("Init"))
//!       .periodic(|_period| {
//!          println!("Periodic");
//!       })
//!       .build()
//!       .timeouts(Duration::from_secs(1))
//!       .schedule();
//! 
//!   for _ in 0..10 {
//!     manager.run();
//!     std::thread::sleep(Duration::from_millis(95));
//!   }
//! }
//! ```




#[macro_use]
pub mod manager;
pub mod commands;
pub mod conditions;
#[cfg(test)]
mod test;

pub use commands::*;
pub use manager::*;



#[derive(Debug, Clone, Copy)]
pub struct WrongThreadError(&'static str);
impl std::fmt::Display for WrongThreadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl std::error::Error for WrongThreadError {}



/// A macro that makes it more ergonomic to "move" Clonable's into closures.
///
/// Allows for any number of local clonable variables to be defined before the closure,
/// they will be cloned BEFORE the closure is created and moved into the closure.
///
/// # Examples
/// ```
/// use command::CommandBuilder;
///
/// let variable: Vec<f64> = Vec::new();
/// let variable2: Option<u8> = None;
///
/// let cmd = CommandBuilder::new()
///     .periodic(clone_mv!(variable >> |_period| {
///         println!("Periodic: {variable:?}");
///     }))
///     .init(clone_mv!(variable, variable2 >> || println!("Init: {variable:?} {variable2:?}")))
///     .build();
/// ```
#[macro_export]
macro_rules! clone_mv {
    // capture an infinite number of idents then move then a closure
    ($($name:ident),* >> |$($arg:ident),*| $body:expr) => {
        {
            // clone the idents
            $(let $name = $name.clone();)*
            // return a closure that takes the same arguments as the original closure
            move |$($arg),*| {
                // call the original closure
                $body
            }
        }
    };
    ($($name:ident),* >> || $body:expr) => {
        {
            // clone the idents
            $(let $name = $name.clone();)*
            // return a closure that takes the same arguments as the original closure
            move || {
                // call the original closure
                $body
            }
        }
    };
    ($($name:ident),* >> |_| $body:expr) => {
        {
            // clone the idents
            $(let $name = $name.clone();)*
            // return a closure that takes the same arguments as the original closure
            move || {
                // call the original closure
                $body
            }
        }
    };
    (|$($arg:ident),*| $body:expr) => {
        // return a closure that takes the same arguments as the original closure
        move |$($arg),*| {
            // call the original closure
            $body
        }
    };
}
