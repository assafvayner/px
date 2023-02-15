pub mod messages;

use std::{
    cmp::Ordering,
    collections::{hash_map::Entry, HashMap},
    time::Duration,
};

use async_recursion::async_recursion;

use crate::{
    me,
    messages::{Message, MessageContent},
    send,
    timer::timer,
};

#[cfg(all(feature = "pingpong", not(feature = "paxos")))]
use crate::APP;

use self::messages::{Ping, Pong};

use super::{App, AppError, AppResult};

static PING_DELAY: u64 = 2000;
static CHECK_DELAY: u64 = 2500;

pub struct PingPongApp {
    pinging: HashMap<String, u64>,
}

impl PingPongApp {
    pub fn new() -> Self {
        PingPongApp {
            pinging: HashMap::new(),
        }
    }

    pub async fn init_pinger(&mut self, to: &String) {
        if self.pinging.contains_key(to) {
            return;
        }
        self.pinging.insert(to.clone(), 1);

        let ping = Ping {
            seqnum: 1,
            message: String::from(format!("{} init ping 1", me())),
        };
        let message = Message::new(me().clone(), to.clone(), MessageContent::Ping(ping));

        // send message if can
        send(&message).await;
        timer(
            check_ping_replied(message),
            Duration::from_millis(CHECK_DELAY),
        );
    }

    fn process_ping(&mut self, from: &String, ping: &Ping) -> AppResult {
        #[cfg(debug_assertions)]
        // debug_println(format!("proc {:?}", ping));
        let Ping { seqnum, .. } = ping;
        let pong = Pong::new(
            *seqnum,
            String::from(format!("pong seq: {} from: {} to: {}", *seqnum, me(), from)),
        );
        let message = Message::new(me().clone(), from.clone(), MessageContent::Pong(pong));

        // respond with pong immediately
        Ok(Some((message, None)))
    }

    fn process_pong(&mut self, from: &String, pong: &Pong) -> AppResult {
        #[cfg(debug_assertions)]
        // debug_println(format!("proc {:?}", pong));
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
                let stored_seqnum = o.get_mut();
                match (*stored_seqnum).cmp(seqnum) {
                    Ordering::Less => {
                        return Err(AppError {
                            error_message: format!(
                                "ping pong app; seqnum mismatch, expected: <= {}, received: {}.",
                                *stored_seqnum, seqnum
                            ),
                        });
                    }
                    Ordering::Equal => {
                        // this pong has been acknowledged
                        *stored_seqnum += 1;
                        /* continue processing */
                    }
                    Ordering::Greater => {
                        // pong for old seqnum, no response
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

        // check that ping was responded to,
        // ping sent after PING_DELAY, check CHECK_DELAY after that
        timer(
            check_ping_replied(message.clone()),
            Duration::from_millis(PING_DELAY + CHECK_DELAY),
        );

        // respond with new Ping after PING_DELAY
        let delay = Duration::from_millis(PING_DELAY);
        Ok(Some((message, Some(delay))))
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
    #[cfg(feature = "pingpong")]
    let ping_pong_app = APP.lock().await;

    #[cfg(feature = "pingpong")]
    let current_seqnum = ping_pong_app.pinging.get(&message.to).unwrap();

    #[cfg(not(feature = "pingpong"))]
    let current_seqnum: &u64 = &u64::MIN;

    if *current_seqnum > ping.seqnum {
        // good case already logged pong
        return;
    }

    #[cfg(debug_assertions)]
    // debug_println(format!(
    //     "failed to receive pong for seq: {} from {}",
    //     ping.seqnum, message.to
    // ));
    send(&message).await;
    timer(
        check_ping_replied(message),
        Duration::from_millis(CHECK_DELAY),
    );
}

impl App for PingPongApp {
    fn handles(message_content: &MessageContent) -> bool {
        match message_content {
            MessageContent::Ping(_) | MessageContent::Pong(_) => true,
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
