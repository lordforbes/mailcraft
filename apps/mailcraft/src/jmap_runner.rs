// SPDX-FileCopyrightText: MailCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0
//! The desktop's side of a JMAP account (M6), Mailflare's among them: each job on a thread of its
//! own, over the sockets of [`mailcraft_host::jmap`], the pieces of a run reaching the window through
//! a channel as they come. What the transport does, and what becomes of the credential, is said
//! there.

use std::sync::atomic::Ordering;
use std::sync::mpsc::{Receiver, sync_channel};

use mailcraft_host::jmap::{fetch_with, run_mail_with};
use mailcraft_model::sync::{GraphFetchResult, JmapMailResult};
use mailcraft_ui_egui::jmap_mail::{JmapFetchJob, JmapJob};

/// Runs a JMAP Send/Receive on a thread of its own, handing the pieces to the window as they come.
pub(crate) fn run_mail(job: JmapJob, wake: &egui::Context) -> Receiver<JmapMailResult> {
    // Through a channel short enough that a busy window holds the run back instead of piling pieces up.
    let (tx, rx) = sync_channel(8);
    let wake = wake.clone();
    let spawned = std::thread::Builder::new().name("jmap send/receive".into()).spawn(move || {
        let JmapJob { request, secret, cancel } = job;
        run_mail_with(&request, &secret, &cancel, &mut |piece| {
            // Nobody is listening any more (the window closed): stop.
            if tx.send(piece).is_err() {
                cancel.store(true, Ordering::Relaxed);
            }
            wake.request_repaint();
        });
    });
    // A dropped sender reads as "stopped unexpectedly" in the app.
    if let Err(e) = spawned {
        log::error!("couldn't start Send/Receive for a JMAP account: {e}");
    }
    rx
}

/// Fetches messages of a JMAP account whole on a thread of its own.
pub(crate) fn run_fetch(job: JmapFetchJob, wake: &egui::Context) -> Receiver<GraphFetchResult> {
    let (tx, rx) = sync_channel(1);
    let wake = wake.clone();
    let spawned = std::thread::Builder::new().name("jmap download".into()).spawn(move || {
        let JmapFetchJob { request, secret, cancel } = job;
        let _ = tx.send(fetch_with(&request, &secret, &cancel));
        wake.request_repaint();
    });
    if let Err(e) = spawned {
        log::error!("couldn't start a download for a JMAP account: {e}");
    }
    rx
}
