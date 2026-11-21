// SPDX-FileCopyrightText: MailCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0
//! An account on a server that behaves as Mailflare (a mail service on Cloudflare Workers) does: ids with a
//! `~` in them, which a JSON Pointer must write `~0`; new mail imported into Drafts only; and a message sent
//! to who its own fields name, Bcc included, whatever the envelope says. Over the stand-in's Mailflare
//! profile; every message and address is invented, and nothing leaves the process.

use mailcraft_model::sync::{JmapChange, JmapOutcome, SendOutcome, SendUp};
use mailcraft_model::{Address, Body, Message};

use super::changes_tests::{ME, asks, changes_of, last, read_on, run_stand_in, sent_of};
use super::*;
use crate::standin::{Profile, SESSION, StandIn};

struct Ids {
    inbox: String,
    archive: String,
    drafts: String,
}

/// A Mailflare-like stand-in with an Inbox, an Archive, a Sent and a Drafts, that sends as the account.
fn mailflare() -> (StandIn, Ids) {
    let stand_in = StandIn::new();
    let ids = stand_in.with(|d| {
        d.profile = Profile::mailflare();
        d.identity(ME);
        // Mailflare's own shapes: `<mailbox>~<role>`, and `<mailbox>~f~<folder>` for a folder of the owner's,
        // whose `~1` a JSON Pointer that isn't escaped would read as a `/`.
        let mut place = |id: &str, name: &str, role: Option<&str>| {
            d.mailboxes.push(crate::standin::Mailbox { id: id.into(), name: name.into(), parent: None, role: role.map(str::to_string) });
            id.to_string()
        };
        let ids = Ids {
            inbox: place("mf1~inbox", "Inbox", Some("inbox")),
            archive: place("mf1~f~10", "Projects", None),
            drafts: place("mf1~drafts", "Drafts", Some("drafts")),
        };
        place("mf1~sent", "Sent", Some("sent"));
        ids
    });
    (stand_in, ids)
}

fn deliver(stand_in: &StandIn, mailbox: &str, subject: &str) -> String {
    let raw = format!("From: Zoe <zoe@example.org>\r\nTo: Ada <{ME}>\r\nSubject: {subject}\r\nMessage-ID: <{subject}@example.org>\r\n\r\nHi.\r\n");
    stand_in.with(|d| d.deliver(mailbox, raw.as_bytes(), &[], "2026-10-15T06:00:00Z"))
}

fn letter(bcc: &[&str]) -> SendUp {
    let message = Message {
        id: 41,
        from: Address::new("Ada", ME),
        to: vec![Address::new("", "bob@example.org")],
        bcc: bcc.iter().map(|email| Address::new("", *email)).collect(),
        subject: "Lunch".into(),
        message_id: "lunch-1@example.org".into(),
        date: 1_791_993_600,
        body: Body { text: "Shall we?".into(), html: None },
        ..Default::default()
    };
    SendUp { message: 41, draft: None, mime: Some(Box::new(message)) }
}

// Its mailboxes and mail are read although their ids are outside RFC 8620's alphabet.
#[test]
fn ids_with_a_tilde_are_read() {
    let (stand_in, ids) = mailflare();
    assert!(ids.inbox.contains('~'));
    let email = deliver(&stand_in, &ids.inbox, "Hello");
    let pieces = run_stand_in(&stand_in, &asks(SESSION));
    assert!(last(&pieces).errors.is_empty(), "{:?}", last(&pieces).errors);
    let listed: Vec<String> = pieces.iter().filter_map(|p| p.listing.as_ref()).flat_map(|l| l.folders.iter().map(|f| f.id.clone())).collect();
    assert!(listed.contains(&ids.inbox) && listed.contains(&ids.drafts), "{listed:?}");
    assert!(read_on(&pieces));
    let read: Vec<String> = pieces
        .iter()
        .flat_map(|p| &p.pages)
        .filter(|page| page.folder == ids.inbox)
        .flat_map(|page| page.messages.iter().map(|m| m.server.id.clone()))
        .collect();
    assert_eq!(read, vec![email]);
}

// A move names each mailbox by a JSON Pointer token: `mf1~f~10` goes as `mf1~0f~010`, which the server
// reads back as `mf1~f~10` (unescaped, its `~1` would be a `/`, and the mailbox none at all).
#[test]
fn a_move_between_mailboxes_with_a_tilde_goes_up() {
    let (stand_in, ids) = mailflare();
    let email = deliver(&stand_in, &ids.inbox, "Move me");
    let change = JmapChange::Move { message: 12, id: email.clone(), from: ids.inbox.clone(), to: ids.archive.clone() };
    let pieces = run_stand_in(&stand_in, &JmapMailRequest { changes: vec![change], ..asks(SESSION) });
    let done = changes_of(&pieces);
    assert!(matches!(done.as_slice(), [one] if matches!(one.outcome, JmapOutcome::Done { .. })), "{done:?}");
    let lies = stand_in.with(|d| d.email(&email).map(|e| e.mailboxes.iter().cloned().collect::<Vec<_>>()));
    assert_eq!(lies, Some(vec![ids.archive.clone()]));
}

// Sent the RFC 8621 way, the message is imported into Sent, which such a server refuses: it is held with
// the server's reason and not sent.
#[test]
fn without_the_setting_the_server_refuses_the_copy_in_sent() {
    let (stand_in, _) = mailflare();
    let pieces = run_stand_in(&stand_in, &JmapMailRequest { sends: vec![letter(&[])], ..asks(SESSION) });
    let sent = sent_of(&pieces);
    assert!(matches!(&sent[..], [done] if matches!(&done.outcome, SendOutcome::Held { lost: false, why } if why.contains("Drafts"))), "{sent:?}");
    assert!(stand_in.with(|d| d.submitted.is_empty()));
}

// With the setting, the message is imported into Drafts with its Bcc field, the server sends it to the
// blind copies too, and lets the draft go: nothing is left over, and the message is sent.
#[test]
fn sending_from_the_draft_reaches_the_blind_copies() {
    let (stand_in, ids) = mailflare();
    let request = JmapMailRequest { sends: vec![letter(&["carol@example.org"])], sends_from_draft: true, ..asks(SESSION) };
    let pieces = run_stand_in(&stand_in, &request);
    let sent = sent_of(&pieces);
    assert!(matches!(&sent[..], [done] if done.outcome == SendOutcome::Sent), "{sent:?}");
    let (to, raw, left) = stand_in.with(|d| {
        let one = d.submitted.first().cloned().unwrap();
        let left = d.emails.iter().filter(|e| e.mailboxes.contains(&ids.drafts)).count();
        (one.rcpt_to, one.raw, left)
    });
    assert_eq!(to, vec!["bob@example.org".to_string(), "carol@example.org".to_string()]);
    assert!(String::from_utf8_lossy(&raw).to_ascii_lowercase().contains("bcc: carol@example.org"));
    assert_eq!(left, 0, "the draft goes once it is sent");
    assert!(last(&pieces).errors.is_empty(), "{:?}", last(&pieces).errors);
}

// Without blind copies the setting changes nothing of who gets the message, and no `Bcc` field is written.
#[test]
fn sending_from_the_draft_without_blind_copies_writes_no_bcc() {
    let (stand_in, _) = mailflare();
    let request = JmapMailRequest { sends: vec![letter(&[])], sends_from_draft: true, ..asks(SESSION) };
    let sent = sent_of(&run_stand_in(&stand_in, &request));
    assert!(matches!(&sent[..], [done] if done.outcome == SendOutcome::Sent), "{sent:?}");
    let raw = stand_in.with(|d| d.submitted.first().map(|s| s.raw.clone()).unwrap_or_default());
    assert!(!String::from_utf8_lossy(&raw).to_ascii_lowercase().contains("\nbcc:"));
}
