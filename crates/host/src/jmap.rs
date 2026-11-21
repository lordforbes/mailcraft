// SPDX-FileCopyrightText: MailCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0
//! A JMAP account's requests over this computer's sockets (M6): `mailcraft-jmap` runs them over
//! [`Transport`], one HTTPS exchange of `mailcraft_net::https` at a time, on the calling thread.
//! The desktop app runs a job on a thread of its own (`apps/mailcraft/src/jmap_runner.rs`), and the
//! command line on its own thread (`mailcraft-cli sync`); the browser build does the same over
//! `fetch` instead (`apps/mailcraft-web/src/jmap.rs`). A JMAP server such as Mailflare, Fastmail or
//! Stalwart is reached this way.
//!
//! **What the transport does**, as `mailcraft_jmap::http` asks of a program: the request goes as
//! built (its method, address, header fields and body); no redirect is followed (`mailcraft-jmap`
//! reads a `Location` and follows it itself, on the session's own origin only); no more of an
//! answer is read than the request may be answered with; and of the answer, its status, its body
//! and the fields the client reads (`Content-Type`, `Retry-After`, `Location`). TLS always, or plain
//! `http` only to this computer (for tests), as every connection of `mailcraft-net`; the system's
//! trust for the certificate, with no exception.
//!
//! **The credential** is the password or the token the caller hands over (the owner typed it, or
//! the command line's environment holds it). It lives in memory for the length of the job, is not
//! written anywhere, and is in no log line; `mailcraft-jmap` takes it out of anything a server says
//! before that is shown.
//!
//! **Stopping.** The run looks at its job's flag before each request; a request under way ends when
//! it is answered, or after the network's own patience (thirty seconds to connect, and as long for
//! each read).

use std::future::{Future, ready};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::{Context, Poll, Wake, Waker};
use std::thread::Thread;

use mailcraft_jmap::fetch::{failed_fetch, fetch_messages};
use mailcraft_jmap::{Connection, Credentials, Error, HttpExchange, HttpRequest, HttpResponse, TransportError, failed_run, sync_mail};
use mailcraft_model::sync::{GraphFetchResult, JmapFetchRequest, JmapMailRequest, JmapMailResult};
use mailcraft_net::{NetError, https};

/// One HTTPS exchange at a time for the JMAP client of one job: `cancel` is the job's.
pub struct Transport {
    cancel: Arc<AtomicBool>,
}

impl Transport {
    pub fn new(cancel: Arc<AtomicBool>) -> Transport {
        Transport { cancel }
    }

    fn send(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError> {
        if self.cancel.load(Ordering::Relaxed) {
            return Err(TransportError::Aborted);
        }
        let fields: Vec<(&str, &str)> = request.fields().collect();
        let asked = https::Request {
            method: request.method().as_str(),
            url: request.url(),
            fields: &fields,
            body: request.body(),
            max_response: request.max_response_bytes(),
            trusted: "",
        };
        let answer = https::send(&asked).map_err(|error| judged(&error))?;
        Ok(response(answer))
    }
}

impl HttpExchange for Transport {
    fn exchange(&self, request: &HttpRequest) -> impl Future<Output = Result<HttpResponse, TransportError>> {
        // (The call blocks, on the job's own thread.)
        ready(self.send(request))
    }
}

/// The answer as the JMAP client reads it: status, body, and the three fields it looks at.
pub fn response(answer: https::Response) -> HttpResponse {
    let https::Response { status, body, location, retry_after, content_type, .. } = answer;
    let mut out = HttpResponse::new(status, body);
    for (name, value) in [("Content-Type", content_type), ("Retry-After", retry_after), ("Location", location)] {
        if let Some(value) = value {
            out.set_field(name, &value);
        }
    }
    out
}

/// What a failed exchange is to the client, by its kind: no text of the network's is passed on, so
/// that nothing private can be.
pub fn judged(error: &NetError) -> TransportError {
    match error {
        NetError::Certificate(_) | NetError::Tls { .. } => TransportError::Tls,
        NetError::Connect { .. } => TransportError::Unreachable,
        error if https::is_too_long(error) => TransportError::BodyTooLarge,
        error if https::is_too_slow(error) => TransportError::TimedOut,
        error if https::is_closed(error) => TransportError::Unreachable,
        _ => TransportError::Other,
    }
}

/// The credentials a job goes with: the secret as a bearer token, or as the password of the
/// account's address. `None` when it can't be sent (empty, or more than a header field takes).
pub fn credentials(email: &str, bearer: bool, secret: &str) -> Option<Credentials> {
    if secret.is_empty() {
        return None;
    }
    if bearer { Credentials::bearer(secret).ok() } else { Credentials::basic(email, secret).ok() }
}

/// One Send/Receive of `request` on this thread: every piece through `emit`, the last one last.
pub fn run_mail_with(request: &JmapMailRequest, secret: &str, cancel: &Arc<AtomicBool>, emit: &mut dyn FnMut(JmapMailResult)) {
    let Some(credentials) = credentials(&request.email, request.bearer, secret) else {
        return emit(failed_run(request, &Error::CredentialsRefused));
    };
    block_on(async {
        match Connection::connect(Transport::new(cancel.clone()), &request.session, credentials).await {
            Ok(connection) => sync_mail(&connection, request, cancel, emit).await,
            Err(error) => emit(failed_run(request, &error)),
        }
    });
}

/// Messages of `request` fetched whole on this thread.
pub fn fetch_with(request: &JmapFetchRequest, secret: &str, cancel: &Arc<AtomicBool>) -> GraphFetchResult {
    let Some(credentials) = credentials(&request.email, request.bearer, secret) else {
        return failed_fetch(request, &Error::CredentialsRefused);
    };
    block_on(async {
        match Connection::connect(Transport::new(cancel.clone()), &request.session, credentials).await {
            Ok(connection) => fetch_messages(&connection, request, cancel).await,
            Err(error) => failed_fetch(request, &error),
        }
    })
}

/// Wakes by unparking the thread that waits.
struct Unpark(Thread);

impl Wake for Unpark {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
    fn wake_by_ref(self: &Arc<Self>) {
        self.0.unpark();
    }
}

/// Runs `future` to completion on this thread (the transport's exchanges block, so it is seldom parked).
fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = std::pin::pin!(future);
    let waker = Waker::from(Arc::new(Unpark(std::thread::current())));
    let mut cx = Context::from_waker(&waker);
    loop {
        match future.as_mut().poll(&mut cx) {
            Poll::Ready(out) => return out,
            Poll::Pending => std::thread::park(),
        }
    }
}

#[cfg(test)]
#[path = "jmap_tests.rs"]
mod jmap_tests;
