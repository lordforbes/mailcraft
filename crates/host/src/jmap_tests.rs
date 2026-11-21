// SPDX-FileCopyrightText: MailCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0
//! The desktop's JMAP transport: the answer as the client reads it, failures by their kind, and a
//! whole run against the in-process JMAP server behind a loopback HTTP server, so that the bytes go
//! through `mailcraft_net::https` as they do to a real server. Nothing leaves this computer; every
//! address is invented.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use mailcraft_jmap::standin::{Profile, SESSION, StandIn, TOKEN};
use mailcraft_jmap::{HttpResponse, TransportError};
use mailcraft_model::sync::{JmapMailRequest, JmapScope, SendOutcome, SendUp};
use mailcraft_model::{Address, Body, Message};
use mailcraft_net::https;

use super::*;

const ME: &str = "ada@example.org";

#[test]
fn the_answer_keeps_what_the_client_reads() {
    let answer = https::Response {
        status: 503,
        body: b"{}".to_vec(),
        location: Some("https://jmap.example/jmap/session".into()),
        retry_after: Some("30".into()),
        content_type: Some("application/json; charset=utf-8".into()),
        ..Default::default()
    };
    let read = response(answer);
    assert_eq!(read.status(), 503);
    assert_eq!(read.content_type(), Some("application/json; charset=utf-8"));
    assert_eq!(read.retry_after(), Some("30"));
    assert_eq!(read.location(), Some("https://jmap.example/jmap/session"));
    assert_eq!(read.body(), b"{}");
    assert_eq!(response(https::Response::default()), HttpResponse::new(0, Vec::new()));
}

#[test]
fn failures_are_told_by_their_kind() {
    let connect = NetError::Connect { host: "jmap.example".into(), reason: "refused".into() };
    let tls = NetError::Tls { host: "jmap.example".into(), reason: "refused".into() };
    assert_eq!(judged(&connect), TransportError::Unreachable);
    assert_eq!(judged(&tls), TransportError::Tls);
    assert_eq!(judged(&NetError::SignInCancelled), TransportError::Other);
}

#[test]
fn no_secret_is_no_sign_in() {
    assert!(credentials(ME, true, "").is_none());
    assert!(credentials(ME, true, "tok").is_some() && credentials(ME, false, "pw").is_some());
    assert!(credentials(ME, true, "a\r\nb").is_none(), "a token that would end the header field early");
}

#[test]
fn a_stopped_job_asks_nothing() {
    let transport = Transport::new(Arc::new(AtomicBool::new(true)));
    let request = JmapMailRequest { email: ME.into(), session: "http://127.0.0.1:9/jmap/session".into(), bearer: true, ..Default::default() };
    let mut pieces = Vec::new();
    run_mail_with(&request, "tok", &transport.cancel, &mut |piece| pieces.push(piece));
    assert!(matches!(pieces.as_slice(), [one] if !one.partial && !one.errors.is_empty()), "{pieces:?}");
}

/// One HTTP/1.1 exchange read off `stream` and answered by the stand-in: request line, header fields,
/// a body by its length.
fn serve_one(stand_in: &StandIn, stream: std::net::TcpStream) -> Option<()> {
    let mut reader = BufReader::new(stream.try_clone().ok()?);
    let mut line = String::new();
    reader.read_line(&mut line).ok()?;
    let mut words = line.split_whitespace();
    let (method, path) = (words.next()?.to_string(), words.next()?.to_string());
    let mut fields: Vec<(String, String)> = Vec::new();
    loop {
        let mut field = String::new();
        reader.read_line(&mut field).ok()?;
        let field = field.trim_end();
        if field.is_empty() {
            break;
        }
        let (name, value) = field.split_once(':')?;
        fields.push((name.trim().to_string(), value.trim().to_string()));
    }
    let length = fields.iter().find(|(name, _)| name.eq_ignore_ascii_case("content-length")).and_then(|(_, v)| v.parse().ok()).unwrap_or(0);
    let mut body = vec![0; length];
    reader.read_exact(&mut body).ok()?;
    // (The stand-in's addresses are relative to its own origin, which the loopback stands for.)
    let url = format!("{}{path}", SESSION.trim_end_matches("/jmap/session"));
    let named: Vec<(&str, &str)> = fields.iter().map(|(n, v)| (n.as_str(), v.as_str())).collect();
    let answer = stand_in.answer(&method, &url, &named, &body).ok()?;
    let mut out = stream;
    let kind = answer.content_type().unwrap_or("application/octet-stream").to_string();
    write!(out, "HTTP/1.1 {} X\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", answer.status(), answer.body().len())
        .ok()?;
    out.write_all(answer.body()).ok()?;
    Some(())
}

/// A loopback HTTP server in front of `stand_in`, for as long as the test runs: its session address.
fn loopback(stand_in: &StandIn) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let stand_in = stand_in.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let _ = serve_one(&stand_in, stream);
        }
    });
    format!("http://127.0.0.1:{port}/jmap/session")
}

// The whole way, through the sockets: a Mailflare-like server's mail read and a message sent to a blind copy.
#[test]
fn a_mailflare_account_is_read_and_sends_through_the_desktop_transport() {
    let stand_in = StandIn::new();
    stand_in.with(|d| {
        d.profile = Profile::mailflare();
        d.identity(ME);
        let inbox = d.mailbox("Inbox", None, Some("inbox"));
        d.mailbox("Drafts", None, Some("drafts"));
        d.mailbox("Sent", None, Some("sent"));
        let raw = format!("From: Zoe <zoe@example.org>\r\nTo: Ada <{ME}>\r\nSubject: Hi\r\nMessage-ID: <hi@example.org>\r\n\r\nHi.\r\n");
        d.deliver(&inbox, raw.as_bytes(), &[], "2026-10-15T06:00:00Z");
    });
    let session = loopback(&stand_in);
    let message = Message {
        id: 41,
        from: Address::new("Ada", ME),
        to: vec![Address::new("", "bob@example.org")],
        bcc: vec![Address::new("", "carol@example.org")],
        subject: "Lunch".into(),
        message_id: "lunch-1@example.org".into(),
        date: 1_791_993_600,
        body: Body { text: "Shall we?".into(), html: None },
        ..Default::default()
    };
    let request = JmapMailRequest {
        account: 3,
        email: ME.into(),
        session,
        bearer: true,
        sends_from_draft: true,
        window: 50,
        list_folders: true,
        scope: JmapScope::All,
        sends: vec![SendUp { message: 41, draft: None, mime: Some(Box::new(message)) }],
        ..Default::default()
    };
    let mut pieces = Vec::new();
    run_mail_with(&request, TOKEN, &Arc::new(AtomicBool::new(false)), &mut |piece| pieces.push(piece));
    let last = pieces.last().unwrap();
    assert!(!last.partial && last.errors.is_empty(), "{:?}", last.errors);
    assert!(pieces.iter().flat_map(|p| &p.sent).any(|done| done.outcome == SendOutcome::Sent));
    assert!(pieces.iter().flat_map(|p| &p.pages).any(|page| !page.messages.is_empty()));
    let to = stand_in.with(|d| d.submitted.first().map(|s| s.rcpt_to.clone()).unwrap_or_default());
    assert_eq!(to, vec!["bob@example.org".to_string(), "carol@example.org".to_string()]);
}
