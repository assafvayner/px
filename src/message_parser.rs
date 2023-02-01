use std::{collections::VecDeque, ops::Deref};

use bytes::{Buf, Bytes, BytesMut};

use crate::{
    debug_println,
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

    pub(crate) fn is_empty(&self) -> bool {
        self.message_queue.is_empty()
    }

    /// function to append data to be parsed into messages, no limit on the data's
    /// structure, could contain 1, less than 1 or more data than 1 message
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

    // append the data to the internal buffer then attempt to parse as many messages as possible
    fn append_buf(&mut self, data: &Bytes) {
        self.buf.extend(data);

        loop {
            // clean start of buffer
            self.advance_while_delimiter();

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

    // advance interior buf while the start of it is delimiter, i.e. end of message/invalid start
    fn advance_while_delimiter(&mut self) {
        let mut first_non_delimiter = 0;
        for byte in self.buf.iter() {
            if *byte != DELIMITER {
                break;
            }
            first_non_delimiter += 1;
        }
        if first_non_delimiter == 0 {
            return;
        }
        self.buf.advance(first_non_delimiter);
    }
}

/// message parser is an iterator, not with trait IntoIterator,
/// it still holds on to its message queue
impl Iterator for MessageParser {
    type Item = Message;

    /// returns Some(message) if there is a next message
    /// or None otherwise
    fn next(&mut self) -> Option<Self::Item> {
        self.message_queue.pop_front()
    }
}

// util to parse a single function
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

/// util to find the index of the first byte in a buffer that equals to a value
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

/// util to parse a json message to a Message struct
fn parse(data: &[u8]) -> Message {
    let json_parse_result: Result<Message, serde_json::Error> = serde_json::from_slice(data);
    match json_parse_result {
        Ok(message) => message,
        Err(e) => {
            #[cfg(debug_assertions)]
            debug_println(format!("INVALID MESSAGE, err: {:?} data: {:?}", e, data));
            Message::new(String::new(), String::new(), MessageContent::Invalid)
        }
    }
}
