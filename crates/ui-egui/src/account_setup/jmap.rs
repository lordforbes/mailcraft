// SPDX-FileCopyrightText: MailCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0
//! The JMAP server page of Add Account (M6): an account kept in step over JMAP (RFC 8620, RFC 8621)
//! rather than IMAP and SMTP, such as one on Mailflare (a mail service on Cloudflare Workers) or
//! Stalwart. (Fastmail serves downloads from another host than its session, which is refused: the
//! secret goes to the session's own server and nowhere else.)
//!
//! The owner gives the server (its name, or the address of its session resource), the token or
//! password, and whether it is a Mailflare server. A name alone becomes the session's address:
//! `https://<name>/jmap/session` for Mailflare, `https://<name>/.well-known/jmap` otherwise (RFC
//! 8620 §2.2). A Mailflare server signs in with an API key that has the `jmap` scope (Mailflare's
//! Settings › App passwords makes one), sent as a bearer token, and sends mail from the draft
//! ([`mailcraft_model::JmapServer::sends_from_draft`]).
//!
//! Nothing is sent from here: the account is added by `account.add` with `kind: "jmap"`, the
//! secret is kept in memory for the account (never in the mailbox, never on disk), and its first
//! Send/Receive starts at once, which says what came of it.

use mailcraft_model::JmapServer;

use super::*;

/// The most characters the server's address takes.
const SESSION: usize = JmapServer::MAX_SESSION;

/// What the JMAP server page keeps.
#[derive(Default)]
pub(super) struct Jmap {
    /// The server's name, or its session address, as typed.
    pub(super) server: String,
    /// The server is a Mailflare one: a token, the session at `/jmap/session`, and sending from the draft.
    pub(super) mailflare: bool,
    /// The secret is the account's password (HTTP Basic, with the address as the user name), not a token.
    pub(super) password: bool,
}

/// What the Examples on the page show: what it is, what goes in Server, and how to sign in.
const EXAMPLES: [(&str, &str, &str); 4] = [
    (
        tl_later!("Mailflare on Cloudflare's workers.dev address"),
        "mailflare.your-subdomain.workers.dev",
        tl_later!(
            "Tick \"This is a Mailflare server\" and paste a Mailflare app password. Your subdomain is under Workers & Pages in Cloudflare's dashboard."
        ),
    ),
    (
        tl_later!("Mailflare on a domain of your own"),
        "mail.example.org",
        tl_later!(
            "The domain you open Mailflare's web inbox at. Tick \"This is a Mailflare server\". If Cloudflare Access protects it, let /jmap/* and /.well-known/jmap through: a mail program can't sign in to Access."
        ),
    ),
    (
        tl_later!("Stalwart, or another server that publishes JMAP"),
        "mail.example.org",
        tl_later!(
            "Its name alone: MailCraft asks https://mail.example.org/.well-known/jmap. Tick \"Sign in with my password\" unless the server gave you a token."
        ),
    ),
    (
        tl_later!("The session's address, when you know it"),
        "https://mail.example.org/jmap/session",
        tl_later!("Used as it is. It must begin with https://."),
    ),
];

/// The session address for what was typed: an address as it is, a server's name alone as the place
/// its session is (`mailflare`: `/jmap/session`, else the well-known `/.well-known/jmap`).
pub(super) fn session_of(typed: &str, mailflare: bool) -> String {
    let typed = typed.trim();
    if typed.contains("://") {
        return typed.to_string();
    }
    let name = typed.trim_end_matches('/');
    let path = if mailflare { "/jmap/session" } else { "/.well-known/jmap" };
    format!("https://{name}{path}")
}

impl AccountSetup {
    /// The JMAP server page, from the Choose page's tile.
    pub(super) fn jmap_page_from(&mut self) {
        self.picked = true;
        self.go(Page::Jmap);
        // (The examples are what a first visit needs most: shown until the owner folds them away.)
        self.settings_open = true;
    }

    /// What the JMAP page's actions do; `None` for an action of another page.
    pub(super) fn act_jmap(&mut self, app: &mut MailApp, action: Action) -> Option<bool> {
        match (self.page, action) {
            (Page::Jmap, Action::Go) => self.jmap_connect(app),
            (Page::Jmap, Action::Back) => {
                self.forget_password();
                self.go(self.origin());
            }
            _ => return None,
        }
        Some(self.done)
    }

    /// Connect: the account added, with its secret in memory, and its first Send/Receive started.
    fn jmap_connect(&mut self, app: &mut MailApp) {
        let Some((_, _)) = address(&self.email) else {
            self.error = self.not_an_address();
            return;
        };
        let session = session_of(&self.jmap.server, self.jmap.mailflare);
        if let Err(why) = JmapServer::check_session(&session) {
            self.error = fmt(tl!("The server's address can't be used: it {why}."), &[("why", why)]);
            return;
        }
        let bearer = self.jmap.mailflare || !self.jmap.password;
        let server = json!({"session": session, "bearer": bearer, "sendsFromDraft": self.jmap.mailflare});
        let added = app.execute("account.add", json!({"email": self.email.trim(), "displayName": self.name.trim(), "kind": "jmap", "jmap": server}));
        match added.map(|v| v.get("id").and_then(Value::as_u64)) {
            Ok(Some(id)) => {
                let secret = self.password.take();
                if !secret.is_empty() {
                    app.adopt_password(id, secret);
                }
                self.finish(app, id);
            }
            Ok(None) => self.error = tl!("The account couldn't be added").to_string(),
            Err(e) => self.error = e,
        }
    }

    /// The page: the server, Mailflare or not, how it signs in, and the secret.
    pub(super) fn jmap_form(&mut self, app: &MailApp, ui: &mut Ui, pass: &mut Pass) -> Option<Action> {
        let t = pass.t;
        page(ui, &t, |ui| {
            heading(ui, tl!("JMAP server account"), &t);
            note(ui, &shown(self.email.trim(), 120), t.text_dim);
            note(
                ui,
                tl!(
                    "For a mail server that speaks JMAP over HTTPS (port 443), such as Mailflare or Stalwart. There are no IMAP or SMTP ports to set. Nothing is sent until you press Connect, and then only to this server."
                ),
                t.text,
            );
            if field(ui, pass, "jmapServer", tl!("Server"), &mut self.jmap.server, "mailflare.example.workers.dev", SESSION).changed() {
                self.error.clear();
            }
            note(ui, tl!("The server's name, or the address of its JMAP session (https://…)."), t.text_dim);
            ui.checkbox(&mut self.jmap.mailflare, tl!("This is a Mailflare server"));
            if self.jmap.mailflare {
                note(
                    ui,
                    tl!(
                        "Make an app password in Mailflare (Settings › App passwords) and paste it below. The address above must be one of the mailboxes it is allowed to send from. Mail is sent from Mailflare's Drafts, as Mailflare does it."
                    ),
                    t.text_dim,
                );
            } else {
                ui.checkbox(&mut self.jmap.password, tl!("Sign in with my password instead of a token"));
            }
            let hint = if self.jmap.mailflare || !self.jmap.password { tl!("Token or app password") } else { "" };
            self.password_field(ui, pass, app.secret_watch(), hint);
            note(ui, tl!("MailCraft keeps it in memory until you close the app. It is never written to disk."), t.text_dim);
            ui.add_space(2.0);
            disclosure(ui, tl!("Examples"), &mut self.settings_open, &t);
            if self.settings_open {
                indented(ui, |ui| {
                    for (what, server, how) in EXAMPLES {
                        caption(ui, tl!(what), &t);
                        ui.label(RichText::new(server).color(t.text).monospace());
                        note(ui, tl!(how), t.text_dim);
                    }
                });
            }
        });
        problem(ui, &self.error);
        let ready = !self.password.0.is_empty() && !self.jmap.server.trim().is_empty();
        let (mut go, mut back) = (false, false);
        strip(ui, self.page, |ui| {
            go = button(ui, tl!("Connect"), true, ready).clicked();
            back = button(ui, tl!("Back"), false, true).clicked();
        });
        if back { Some(Action::Back) } else { (ready && (go || pass.submit)).then_some(Action::Go) }
    }
}

#[cfg(test)]
#[path = "jmap_tests.rs"]
mod jmap_tests;
