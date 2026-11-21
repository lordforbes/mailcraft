// SPDX-FileCopyrightText: MailCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0
//! The JMAP server page of Add Account (M6): an account kept in step over JMAP (RFC 8620, RFC 8621)
//! rather than IMAP and SMTP, such as one on Mailflare (a mail service on Cloudflare Workers),
//! Fastmail or Stalwart.
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
                    "For a mail server that speaks JMAP, such as Mailflare, Fastmail or Stalwart. Nothing is sent until you press Connect, and then only to this server."
                ),
                t.text,
            );
            if field(ui, pass, "jmapServer", tl!("Server"), &mut self.jmap.server, "mail.example.org", SESSION).changed() {
                self.error.clear();
            }
            note(ui, tl!("The server's name, or the address of its JMAP session (https://…)."), t.text_dim);
            ui.checkbox(&mut self.jmap.mailflare, tl!("This is a Mailflare server"));
            if self.jmap.mailflare {
                note(
                    ui,
                    tl!(
                        "Make an app password in Mailflare (Settings › App passwords) and paste it below. Mail is sent from Mailflare's Drafts, as Mailflare does it."
                    ),
                    t.text_dim,
                );
            } else {
                ui.checkbox(&mut self.jmap.password, tl!("Sign in with my password instead of a token"));
            }
            let hint = if self.jmap.mailflare || !self.jmap.password { tl!("Token or app password") } else { "" };
            self.password_field(ui, pass, app.secret_watch(), hint);
            note(ui, tl!("MailCraft keeps it in memory until you close the app. It is never written to disk."), t.text_dim);
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
