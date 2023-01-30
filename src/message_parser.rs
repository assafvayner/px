use std::{collections::VecDeque, ops::Deref};

use bytes::{Buf, Bytes, BytesMut};

use crate::{
    messages::{Message, MessageContent},
    DELIMITER,
};

static INITIAL_CAPACITY: usize = 1000;

/// A struct in charge of parsing messages from chunks of data appended via
/// append_data and queried through
pub(crate) struct MessageParser {
    buf: BytesMut,
    message_queue: VecDeque<Message>,
}

impl MessageParser {
    /// creates a new MessageParser instance
    pub(crate) fn new() -> MessageParser {
        MessageParser {
            buf: BytesMut::with_capacity(INITIAL_CAPACITY),
            message_queue: VecDeque::new(),
        }
    }

    pub(crate) fn append_data(&mut self, data: &Bytes) {
        if !self.buf.is_empty() {
            // some data already needs to be processed before this data
            self.append_buf(data);
            return;
        }

        let idx_option = index_first_byte_equals(data, DELIMITER);
        if let None = idx_option {
            // not a full message
            self.append_buf(data);
            return;
        }
        let idx = idx_option.unwrap();

        if idx != data.len() - 1 {
            // there is more than 1 message worth of bytes in data
            self.append_buf(data);
            return;
        }
        if let Ok(message) = parse_single_message_delimited(data) {
            self.message_queue.push_back(message);
        } else {
            self.append_buf(data);
        }
    }

    fn append_buf(&mut self, data: &Bytes) {
        self.buf.extend(data);

        loop {
            while self.buf.len() > 0 && self.buf[0] == DELIMITER {
                self.buf.advance(1);
            }

            if self.buf.is_empty() {
                return;
            }

            let idx_option = index_first_byte_equals(&self.buf, DELIMITER);
            if let None = idx_option {
                break;
            }

            let delim_index = idx_option.unwrap();
            let single_message_delimited = self.buf.split_to(delim_index + 1);
            if let Ok(message) = parse_single_message_delimited(&single_message_delimited) {
                self.message_queue.push_back(message);
            }
            // if returned error, we should not insert message into message queue
            // however we should keep trying to read messages
        }
    }
}

impl Iterator for MessageParser {
    type Item = Message;

    fn next(&mut self) -> Option<Self::Item> {
        self.message_queue.pop_front()
    }
}

fn parse_single_message_delimited<'a, T>(single_message_delimited: &'a T) -> Result<Message, ()>
where
    T: Deref<Target = [u8]>,
{
    let single_message_slice_option = single_message_delimited.strip_suffix(&[DELIMITER]);
    if let None = single_message_slice_option {
        return Err(());
    }
    let single_message_slice = single_message_slice_option.unwrap();
    let message = parse(single_message_slice);
    if let MessageContent::Invalid = message.content {
        // invalid message, ignoring
        return Err(());
    }
    return Ok(message);
}

fn index_first_byte_equals<'a, T>(bytes: T, value: u8) -> Option<usize>
where
    T: IntoIterator<Item = &'a u8>, // Bytes/BytesMut
{
    let mut i: usize = 0;
    for byte in bytes {
        if *byte == value {
            return Some(i);
        }
        i += 1;
    }
    return None;
}

fn parse(data: &[u8]) -> Message {
    let json_parse_result: Result<Message, serde_json::Error> = serde_json::from_slice(data);
    if let Err(x) = &json_parse_result {
        eprintln!("{:?}, {:?}", x, data);
    }
    if let Ok(msg) = json_parse_result {
        return msg;
    }
    // failed to parse message, returns message with invalid
    eprintln!("******************");
    eprintln!("INVALID: {:?}", data);
    eprintln!("******************");
    Message {
        content: MessageContent::Invalid,
        from: String::new(),
        to: String::new(),
    }
}
