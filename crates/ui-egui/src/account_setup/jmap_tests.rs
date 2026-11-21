// SPDX-FileCopyrightText: MailCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0
//! Add Account for a JMAP server account (M6), Mailflare's among them: the session address made of
//! what is typed, the account added with how it signs in and sends, and its secret kept in memory
//! only. Nothing here leaves the machine; every name and address is invented.

use mailcraft_engine::Session;
use mailcraft_model::{AccountKind, Mailbox};

use super::*;
use crate::{MailApp, Services};

/// Wednesday 2026-10-14 16:00 UTC.
const NOW: i64 = 1_791_993_600;
const ME: &str = "ada@example.org";
const TOKEN: &str = "mf_not-a-real-key";

fn app() -> MailApp {
    let mut app = MailApp::new(Session::at(Mailbox::with_local_account("", ""), NOW, 0), Services::default());
    app.autosave = false;
    app
}

/// The JMAP page, filled in with `server`, the token, and Mailflare or not.
fn filled(server: &str, mailflare: bool) -> AccountSetup {
    let mut setup = AccountSetup::blank(Page::Jmap);
    setup.email = ME.into();
    setup.jmap = Jmap { server: server.into(), mailflare, password: false };
    setup.password = Secret(TOKEN.into());
    setup
}

#[test]
fn a_name_alone_is_the_place_of_its_session() {
    assert_eq!(session_of("mail.example.org", true), "https://mail.example.org/jmap/session");
    assert_eq!(session_of(" mail.example.org/ ", false), "https://mail.example.org/.well-known/jmap");
    assert_eq!(session_of("https://mail.example.org/jmap/session", false), "https://mail.example.org/jmap/session");
    // (An address that may not be used is still the owner's to see refused, not rewritten.)
    assert_eq!(session_of("http://mail.example.org/jmap/session", true), "http://mail.example.org/jmap/session");
}

#[test]
fn a_mailflare_account_is_added_with_a_token_that_stays_in_memory() {
    let mut app = app();
    let mut setup = filled("mail.example.org", true);
    setup.act(&mut app, Action::Go);
    let account = app.session.mailbox.accounts.iter().find(|a| a.email == ME).cloned().expect("the account is added");
    assert_eq!(account.kind, AccountKind::Jmap);
    let server = account.jmap.clone().unwrap();
    assert_eq!(server.session, "https://mail.example.org/jmap/session");
    assert!(server.bearer && server.sends_from_draft);
    assert_eq!(app.passwords.get(&account.id).map(String::as_str), Some(TOKEN));
    // Never in the mailbox file.
    let written = mailcraft_engine::io::write_mailbox(&app.session.mailbox).unwrap();
    assert!(!String::from_utf8_lossy(&written).contains(TOKEN));
    assert_eq!(setup.page, Page::Done);
}

#[test]
fn another_jmap_server_signs_in_as_the_owner_says() {
    let mut app = app();
    let mut setup = filled("https://jmap.example.org/.well-known/jmap", false);
    setup.jmap.password = true;
    setup.act(&mut app, Action::Go);
    let server = app.session.mailbox.accounts.iter().find(|a| a.email == ME).and_then(|a| a.jmap.clone()).unwrap();
    assert!(!server.bearer && !server.sends_from_draft);
}

#[test]
fn an_address_that_is_not_https_is_refused_and_nothing_is_added() {
    let mut app = app();
    let before = app.session.mailbox.accounts.len();
    let mut setup = filled("http://mail.example.org/jmap/session", true);
    setup.act(&mut app, Action::Go);
    assert_eq!(app.session.mailbox.accounts.len(), before);
    assert_eq!(setup.page, Page::Jmap);
    assert!(setup.error.contains("https"), "{}", setup.error);
}

#[test]
fn the_choose_page_offers_a_jmap_server() {
    assert!(Kind::tiles().contains(&Kind::Jmap));
    let mut app = app();
    let mut setup = AccountSetup::blank(Page::Choose);
    setup.email = ME.into();
    setup.act(&mut app, Action::Pick(Kind::Jmap));
    assert_eq!(setup.page, Page::Jmap);
    // Back returns to the Choose page it came from.
    setup.act(&mut app, Action::Back);
    assert_eq!(setup.page, Page::Choose);
}

// The examples start shown, and every one of them is a server the page takes as it is written there.
#[test]
fn the_examples_are_shown_and_each_is_an_address_that_may_be_used() {
    let mut app = app();
    let mut setup = AccountSetup::blank(Page::Choose);
    setup.email = ME.into();
    setup.act(&mut app, Action::Pick(Kind::Jmap));
    assert!(setup.settings_open);
    for (_, server, _) in EXAMPLES {
        for mailflare in [true, false] {
            assert_eq!(JmapServer::check_session(&session_of(server, mailflare)), Ok(()), "{server}");
        }
    }
}
