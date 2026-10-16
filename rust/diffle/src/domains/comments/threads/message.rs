//! One message of a thread, and the non-empty list a thread keeps.

use std::iter;

use super::body::{Body, MessageText};
use super::ids::MessageId;
use crate::domains::comments::comment_error::NotFound;
use crate::kernel::epoch_millis::EpochMillis;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    id: MessageId,
    text: MessageText,
    created_at: EpochMillis,
    updated_at: EpochMillis,
}

impl Message {
    /// A message written now.
    #[must_use]
    pub fn written(body: Body, now: EpochMillis) -> Self {
        Self {
            id: MessageId::generate(),
            text: body.into(),
            created_at: now,
            updated_at: now,
        }
    }

    /// A message as stored.
    #[must_use]
    pub fn restored(
        id: MessageId,
        text: MessageText,
        created_at: EpochMillis,
        updated_at: EpochMillis,
    ) -> Self {
        Self {
            id,
            text,
            created_at,
            updated_at,
        }
    }

    #[must_use]
    pub fn id(&self) -> &MessageId {
        &self.id
    }

    #[must_use]
    pub fn text(&self) -> &MessageText {
        &self.text
    }

    #[must_use]
    pub fn created_at(&self) -> EpochMillis {
        self.created_at
    }

    #[must_use]
    pub fn updated_at(&self) -> EpochMillis {
        self.updated_at
    }
}

/// The messages of a thread: the one that opened it, then the replies. Never empty.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Messages {
    first: Message,
    replies: Vec<Message>,
}

/// What is left of a thread's messages once one is removed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessageRemoval {
    Kept(Messages),
    /// The last message went.
    Emptied,
}

impl Messages {
    #[must_use]
    pub fn opened_by(first: Message) -> Self {
        Self {
            first,
            replies: Vec::new(),
        }
    }

    /// `None` for an empty list: a thread has at least the message that opened it.
    #[must_use]
    pub fn from_vec(all: Vec<Message>) -> Option<Self> {
        let mut all = all.into_iter();
        let first = all.next()?;
        Some(Self {
            first,
            replies: all.collect(),
        })
    }

    #[must_use]
    pub fn first(&self) -> &Message {
        &self.first
    }

    pub fn iter(&self) -> impl Iterator<Item = &Message> {
        iter::once(&self.first).chain(&self.replies)
    }

    pub fn push(&mut self, reply: Message) {
        self.replies.push(reply);
    }

    /// Replaces the text of the first message with that id.
    pub fn edit(
        &mut self,
        id: &MessageId,
        text: MessageText,
        now: EpochMillis,
    ) -> Result<(), NotFound> {
        let message = iter::once(&mut self.first)
            .chain(&mut self.replies)
            .find(|message| message.id == *id)
            .ok_or_else(|| NotFound::Message(id.clone()))?;
        message.text = text;
        message.updated_at = now;
        Ok(())
    }

    /// Drops every message with that id.
    pub fn without(&self, id: &MessageId) -> Result<MessageRemoval, NotFound> {
        let all: Vec<Message> = self.iter().cloned().collect();
        let before = all.len();
        let kept: Vec<Message> = all
            .into_iter()
            .filter(|message| message.id != *id)
            .collect();
        if kept.len() == before {
            return Err(NotFound::Message(id.clone()));
        }
        Ok(Self::from_vec(kept).map_or(MessageRemoval::Emptied, MessageRemoval::Kept))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message(id: &str, text: &str) -> Message {
        Message::restored(
            MessageId::from(id.to_owned()),
            MessageText::new(text),
            EpochMillis::new(1),
            EpochMillis::new(1),
        )
    }

    fn texts(messages: &Messages) -> Vec<&str> {
        messages.iter().map(|m| m.text().as_str()).collect()
    }

    #[test]
    fn a_written_message_is_stamped_and_given_an_id() {
        let now = EpochMillis::new(9);
        let written = Message::written(Body::parse("hi").unwrap(), now);
        assert_eq!(written.text().as_str(), "hi");
        assert_eq!((written.created_at(), written.updated_at()), (now, now));
        assert!(!written.id().as_str().is_empty());
    }

    #[test]
    fn messages_are_never_empty() {
        assert!(Messages::from_vec(Vec::new()).is_none());
        let mut messages =
            Messages::from_vec(vec![message("a", "one"), message("b", "two")]).unwrap();
        messages.push(message("c", "three"));
        assert_eq!(texts(&messages), ["one", "two", "three"]);
        assert_eq!(messages.first().id().as_str(), "a");
        assert_eq!(texts(&Messages::opened_by(message("a", "x"))), ["x"]);
    }

    #[test]
    fn an_edit_replaces_the_text_and_stamps_the_message() {
        let mut messages =
            Messages::from_vec(vec![message("a", "one"), message("b", "two")]).unwrap();
        let id = MessageId::from("b".to_owned());
        messages
            .edit(&id, MessageText::new("2"), EpochMillis::new(5))
            .unwrap();
        let edited = messages.iter().nth(1).unwrap();
        assert_eq!(edited.text().as_str(), "2");
        assert_eq!(
            (edited.created_at(), edited.updated_at()),
            (EpochMillis::new(1), EpochMillis::new(5))
        );
        let missing = MessageId::from("zz".to_owned());
        assert_eq!(
            messages.edit(&missing, MessageText::new("x"), EpochMillis::new(6)),
            Err(NotFound::Message(missing))
        );
    }

    #[test]
    fn removing_the_last_message_empties_the_list() {
        let messages = Messages::from_vec(vec![message("a", "one"), message("b", "two")]).unwrap();
        let a = MessageId::from("a".to_owned());
        let b = MessageId::from("b".to_owned());
        let MessageRemoval::Kept(rest) = messages.without(&a).unwrap() else {
            panic!("a reply remains");
        };
        assert_eq!(texts(&rest), ["two"]);
        assert_eq!(rest.without(&b).unwrap(), MessageRemoval::Emptied);
        assert_eq!(rest.without(&a), Err(NotFound::Message(a)));
    }
}
