use std::{
    cmp::Ordering,
    collections::{hash_map::Entry, HashMap},
    time::Duration,
};

use serde::{Deserialize, Serialize};

use crate::{
    me,
    messages::{Message, MessageContent},
};

use super::{App, AppError, AppResult};

#[derive(Debug, Hash, Serialize, Deserialize, Eq, PartialEq, Clone)]
pub struct Ping {
    pub seqnum: u64,
    pub message: String,
}

impl Ping {
    pub fn new(seqnum: u64, message: String) -> Ping {
        Ping { seqnum, message }
    }
}

#[derive(Debug, Hash, Serialize, Deserialize, Eq, PartialEq, Clone)]
pub struct Pong {
    pub seqnum: u64,
    pub message: String,
}

impl Pong {
    pub fn new(seqnum: u64, msg: String) -> Pong {
        Pong {
            seqnum,
            message: msg,
        }
    }
}

pub struct PingPongApp {
    pinging: HashMap<String, u64>,
}

impl PingPongApp {
    pub fn init_pinger(&mut self, to: &String) -> Result<(), String> {
        match self.pinging.entry(to.clone()) {
            Entry::Occupied(o) => Err(o.key().clone()),
            Entry::Vacant(v) => {
                v.insert(1);
                Ok(())
            }
        }
    }

    fn process_ping(&mut self, from: &String, ping: &Ping) -> AppResult {
        let Ping { seqnum, .. } = ping;
        println!("{} proc ping ({:?}), seq: {}", me(), from, seqnum);
        let message = Message {
            from: me().clone(),
            to: from.clone(),
            content: MessageContent::Pong(Pong {
                seqnum: *seqnum,
                message: String::from(""),
            }),
        };
        let delay = None;
        println!("{} sending out {:?}", me(), message);
        Ok(Some((message, delay)))
    }

    fn process_pong(&mut self, from: &String, pong: &Pong) -> AppResult {
        let Pong { seqnum, .. } = pong;
        println!("{} proc pong ({:?}), seq: {}", me(), from, seqnum);
        let entry = self.pinging.entry(from.clone());
        match entry {
            Entry::Vacant(_) => {
                return Err(AppError {
                    error_message: format!(
                        "ping pong app; never pinged but received pong: {:?}",
                        pong
                    ),
                });
            }
            Entry::Occupied(mut o) => {
                let val = o.get_mut();
                match (*val).cmp(seqnum) {
                    Ordering::Less => {
                        return Err(AppError {
                            error_message: format!(
                                "ping pong app; seqnum mismatch, expected: <= {}, received: {}.",
                                *val, seqnum
                            ),
                        });
                    }
                    Ordering::Equal => {
                        // this pong has been acknowledged
                        *val += 1;
                        /* continue processing */
                    }
                    Ordering::Greater => {
                        // pong for old seqnum
                        return Ok(None);
                    }
                }
            }
        }

        let next_ping = Ping {
            seqnum: seqnum + 1,
            message: String::from(""),
        };

        let content = MessageContent::Ping(next_ping);
        let message = Message {
            from: me().clone(),
            to: from.clone(),
            content,
        };

        println!("{} sending out {:?}", me(), message);
        let delay = Some(Duration::from_secs(10));
        Ok(Some((message, delay)))
    }
}

impl App for PingPongApp {
    fn new() -> Self {
        PingPongApp {
            pinging: HashMap::new(),
        }
    }

    fn handles(message_content: &MessageContent) -> bool {
        match message_content {
            MessageContent::Ping(_) => true,
            MessageContent::Pong(_) => true,
            _ => false,
        }
    }

    fn handle(&mut self, message: &Message) -> AppResult {
        let Message { from, content, .. } = message;
        match content {
            MessageContent::Ping(ping) => self.process_ping(from, &ping),
            MessageContent::Pong(pong) => self.process_pong(from, &pong),
            m => Err(AppError {
                error_message: format!(
                    "ping pong app does not handle {} messages",
                    m.message_type()
                ),
            }),
        }
    }
}
