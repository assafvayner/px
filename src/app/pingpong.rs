use std::{
    cell::RefCell,
    cmp::Ordering,
    collections::{hash_map::Entry, HashMap},
    sync::Arc,
    time::Duration,
};

use async_recursion::async_recursion;

use serde::{Deserialize, Serialize};

use crate::{
    connection_manager::ConnectionManager,
    me,
    messages::{Message, MessageContent},
    println_safe, send,
    timer::timer,
    APP,
};

use super::{App, AppError, AppResult};

static PING_DELAY: u64 = 2000;
static CHECK_DELAY: u64 = 2500;

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
    pub cm: RefCell<Option<Arc<ConnectionManager>>>,
}

impl PingPongApp {
    pub async fn init_pinger(&mut self, to: &String) -> Result<(), String> {
        match self.pinging.entry(to.clone()) {
            Entry::Occupied(o) => return Err(o.key().clone()),
            Entry::Vacant(v) => {
                v.insert(1);
            }
        }
        // send ping from here?
        let ping = Ping {
            seqnum: 1,
            message: String::from(format!("{} init ping 1", me())),
        };
        let message = Message::new(me().clone(), to.clone(), MessageContent::Ping(ping));

        // send message if can
        if let Some(cm) = self.cm.get_mut() {
            send(cm.clone(), &message).await;
        } else {
            println_safe(format!("failed to send {:?}", &message));
        }

        timer(
            check_ping_replied(message.clone()),
            Duration::from_millis(CHECK_DELAY),
        );
        Ok(())
    }

    fn process_ping(&mut self, from: &String, ping: &Ping) -> AppResult {
        let Ping { seqnum, .. } = ping;
        println_safe(format!("proc {:?}", ping));
        let pong = Pong::new(
            *seqnum,
            String::from(format!("pong seq: {} from: {} to: {}", *seqnum, me(), from)),
        );
        let message = Message {
            from: me().clone(),
            to: from.clone(),
            content: MessageContent::Pong(pong),
        };

        Ok(Some((message, None)))
    }

    fn process_pong(&mut self, from: &String, pong: &Pong) -> AppResult {
        println_safe(format!("proc {:?}", pong));
        let Pong { seqnum, .. } = pong;
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

        let seqnum = seqnum + 1;

        let next_ping = Ping::new(
            seqnum,
            String::from(format!("ping seq: {} from: {} to: {}", seqnum, me(), from)),
        );

        let content = MessageContent::Ping(next_ping);
        let message = Message {
            from: me().clone(),
            to: from.clone(),
            content,
        };

        timer(
            check_ping_replied(message.clone()),
            Duration::from_millis(CHECK_DELAY),
        );

        let delay = Some(Duration::from_millis(PING_DELAY));
        Ok(Some((message, delay)))
    }
}

#[async_recursion]
async fn check_ping_replied(message: Message) {
    let ping = match &message.content {
        MessageContent::Ping(ping) => ping,
        _ => {
            return;
        }
    };
    let mut ping_pong_app = APP.lock().await;
    let current_seqnum = ping_pong_app.pinging.get(&message.to).unwrap();
    if *current_seqnum > ping.seqnum {
        // good case already logged pong
        return;
    }

    println_safe(format!(
        "failed to receive pong for seq: {} from {}",
        ping.seqnum, message.to
    ));
    if let Some(cm) = ping_pong_app.cm.get_mut() {
        send(cm.clone(), &message).await;
    } else {
        println_safe(format!("failed to send {:?}", &message));
    }
    timer(
        check_ping_replied(message),
        Duration::from_millis(CHECK_DELAY),
    );
}

impl App for PingPongApp {
    fn new() -> Self {
        PingPongApp {
            pinging: HashMap::new(),
            cm: RefCell::new(None),
        }
    }

    fn initialize(&mut self, cm: &Arc<ConnectionManager>) {
        self.cm.replace(Some(cm.clone()));
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
