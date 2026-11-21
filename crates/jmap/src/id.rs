// SPDX-FileCopyrightText: MailCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0
//! JMAP ids (RFC 8620 §1.2).
//!
//! An id is 1 to [`MAX_ID_LEN`] octets of the URL-safe base64 alphabet of RFC 4648 §5 without padding:
//! `A-Z a-z 0-9 - _`, and, beyond what RFC 8620 §1.2 allows, the `~ . @ + =` some servers put in theirs
//! (Mailflare: `<mailbox>~<role>`, `msg~<id>`, identities ending in an address), never `.` or `..` alone
//! ([`mailcraft_model::is_jmap_id`] is the one rule, for what the model keeps too). None of them is a
//! space, a quote, a slash, a backslash, a `%` or a control character. Every id received from a server goes
//! through [`Id::new`] before it is used as a JSON key, in an address (percent-encoded) or in a later
//! request; one that names a property in a patch is escaped as a JSON Pointer token ([`pointer_token`]).
//! The maximum is the specification's own: RFC 8620 §1.2 allows 255 octets and nothing in a mail client
//! needs to refuse a conforming server's ids.

use std::fmt;
use std::str::FromStr;

/// The longest id accepted, in octets (RFC 8620 §1.2).
pub const MAX_ID_LEN: usize = 255;

/// Why a text is not a valid id.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdError {
    /// The id is empty.
    Empty,
    /// The id is longer than [`MAX_ID_LEN`] octets.
    TooLong,
    /// The id holds a character outside `A-Z a-z 0-9 - _ ~ . @ + =`, or is `.` or `..`.
    InvalidCharacter,
}

impl fmt::Display for IdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Empty => "the id is empty, but a JMAP id has 1 to 255 characters",
            Self::TooLong => "the id is longer than 255 octets, the most a JMAP id may have",
            Self::InvalidCharacter => {
                "the id contains a character other than letters, digits and - _ ~ . @ + =, the only ones a JMAP id may hold here"
            }
        })
    }
}

impl std::error::Error for IdError {}

/// A checked JMAP id.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Id(String);

impl Id {
    /// Checks `text` and wraps it.
    pub fn new(text: &str) -> Result<Self, IdError> {
        if text.is_empty() {
            return Err(IdError::Empty);
        }
        if text.len() > MAX_ID_LEN {
            return Err(IdError::TooLong);
        }
        if !mailcraft_model::is_jmap_id(text) {
            return Err(IdError::InvalidCharacter);
        }
        Ok(Self(text.to_owned()))
    }

    /// The id as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// `token` as one reference token of a JSON Pointer (RFC 6901 §3), as a patch names a property of an
/// object by it (RFC 8620 §5.3, `mailboxIds/<id>`, `keywords/<keyword>`): `~` written `~0` and `/` written
/// `~1`, everything else as it is.
pub fn pointer_token(token: &str) -> String {
    if !token.contains(['~', '/']) {
        return token.to_owned();
    }
    token.replace('~', "~0").replace('/', "~1")
}

impl FromStr for Id {
    type Err = IdError;

    fn from_str(text: &str) -> Result<Self, IdError> {
        Self::new(text)
    }
}

impl TryFrom<&str> for Id {
    type Error = IdError;

    fn try_from(text: &str) -> Result<Self, IdError> {
        Self::new(text)
    }
}

impl AsRef<str> for Id {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Debug for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Id({})", self.0)
    }
}

#[cfg(test)]
mod tests;
