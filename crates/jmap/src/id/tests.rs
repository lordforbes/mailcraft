// SPDX-FileCopyrightText: MailCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0
use super::*;

// RFC 8620 §1.2: 1 to 255 octets of A-Z a-z 0-9 - _.
#[test]
fn accepts_valid_ids() {
    for text in ["a", "A13824", "msg-1023_x", "0", "-", "_", &"x".repeat(255)] {
        assert_eq!(Id::new(text).unwrap().as_str(), text);
    }
}

// Beyond RFC 8620 §1.2, the `~ . @ + =` some servers use (Mailflare's mailbox, blob and identity ids).
#[test]
fn accepts_the_ids_some_servers_hand_out() {
    for text in ["mbx1~inbox", "mbx1~f~fld9", "msg~abc", "mbx1~ada@example.org", "abc=", "a+b", "a.b"] {
        assert_eq!(Id::new(text).unwrap().as_str(), text);
    }
}

// Separators, spaces, quotes, `%`, controls and non-ASCII are refused, and so are `.` and `..`.
#[test]
fn refuses_invalid_ids() {
    assert_eq!(Id::new(""), Err(IdError::Empty));
    assert_eq!(Id::new(&"x".repeat(256)), Err(IdError::TooLong));
    for text in [".", "..", "a/b", "a b", "a%2F", "a\\b", "a\u{0}", "é", "a#b", "../x", "\"", "a\nb", "a?b", "a&b"] {
        assert_eq!(Id::new(text), Err(IdError::InvalidCharacter), "{text:?}");
    }
}

// RFC 8620 §1.2: ids compare as exact strings (case matters).
#[test]
fn case_sensitive() {
    assert_ne!(Id::new("A").unwrap(), Id::new("a").unwrap());
    assert_eq!("M1".parse::<Id>().unwrap(), Id::try_from("M1").unwrap());
}

// RFC 6901 §3: `~` is `~0` and `/` is `~1` in a JSON Pointer's token, in that order.
#[test]
fn pointer_tokens_are_escaped() {
    assert_eq!(pointer_token("M1"), "M1");
    assert_eq!(pointer_token("$seen"), "$seen");
    assert_eq!(pointer_token("mbx1~inbox"), "mbx1~0inbox");
    assert_eq!(pointer_token("a/b~c"), "a~1b~0c");
    assert_eq!(pointer_token("~1"), "~01");
}
