use std::time::Duration;

use crate::messages::{Message, MessageContent};

pub mod paxos;
pub mod pingpong;

pub type Delay = Option<Duration>;
pub type AppResult = Result<Option<(Message, Delay)>, AppError>;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AppType {
    PINGPONG,
}

pub trait App {
    fn handles(message: &MessageContent) -> bool;
    fn handle(&mut self, message: &Message) -> AppResult;
}

#[derive(Debug, PartialEq, Eq)]
pub struct AppError {
    error_message: String,
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "AppError({})", self.error_message)
    }
}
