# JMAP accounts

> **Last reviewed:** 2026-10-28 · **Last updated:** 2026-11-14 · **Change:** minor

A JMAP account is one whose server speaks JMAP (RFC 8620 and RFC 8621) over HTTPS. MailCraft reads
such an account in the desktop app, on the command line (`mailcraft-cli sync`) and in the browser
build, takes what you change in its mail up to its server, and sends mail through it. A Mailflare
server (a mail service on Cloudflare Workers) is one: see [Mailflare](#mailflare).

## What works

- **Send/Receive** lists the server's mailboxes and reads the newest messages of each, as many as
  Messages to download per folder says (500 at the most): headers, marks and a preview. It runs when
  you ask for it and every few minutes by itself (then the Inbox and the folder on screen only),
  takes its turn in a Send/Receive of all accounts, and shows in Send/Receive Progress; Cancel stops
  it at once, the request under way with it.
- **A message comes whole** when you open it, before Reply, Reply All, Forward, saving it or one of
  its attachments, and in the background for the Inbox's forty newest (one larger than 10 MB waits
  until it is opened).
- **What the server changes comes down**: new mail, read marks, flags, answered and forwarded marks,
  and mail gone from a mailbox small enough to be read whole in a run (no more messages than
  Messages to download per folder, nor than the server takes in one request).

- **What you change goes up** (#1074): a read mark, a flag, the answered and forwarded marks, a
  move between two of the server's mailboxes (Delete is a move into its Deleted Items) and a
  deletion for good. They go with the next Send/Receive, before it reads, and one starts by itself
  a few seconds after the change once the password or token is at hand. Only the marks you changed
  are touched; the server's other keywords stay. A change the server refuses is put back to how
  the server has it and said once, in the status bar and in Send/Receive Progress, and so is a
  change to a message the server no longer has.
- **A flag is the server's `$flagged` and nothing else.** A JMAP server keeps no due date and no
  "complete": a follow-up's due date stays on this computer, and one marked complete is not flagged
  there. So whether a flag was changed here is told by that one bit. A flag you didn't change since
  the last Send/Receive follows the server: cleared on another device, it is cleared here with its
  due date; flagged again there after you marked it complete, it is flagged here. One you changed
  here goes up, whatever the server said meanwhile. Changed on both sides between two runs, the two
  can only have gone the same way, and the due date or the "complete" kept here stays.
- **A move takes the message out of the mailbox it was in and puts it into the other**, and says
  nothing of any other mailbox (`mailboxIds/<from>` and `mailboxIds/<to>`, never the whole of
  `mailboxIds`). That way it doesn't undo what another program did to the message meanwhile, and
  never takes it out of a mailbox this computer didn't know it was in (a server may keep a message
  in several). The other side of it: a message another program moved on in the meantime, or one
  that lay in a second mailbox already, ends up in both. The next Send/Receive that reads those
  mailboxes lists it where the server has it, and it is shown there (in one of them, when the
  server keeps it in two).
- **Mail is sent** (#1074): a message written from the account waits in its Outbox and goes with
  the next Send/Receive, which starts by itself a few seconds after Send; Send asks for the
  password or token when it isn't at hand. The server itself sends it (JMAP submission, RFC 8621):
  the message is uploaded, filed in the server's Sent mailbox (in Drafts where it has none) and
  submitted, as the identity the server has for the account's address. Who is named in Bcc is told
  to the server as recipients only; the message names none of them. A message the server refuses
  is held in the Outbox with the reason, and so is one whose answer was lost, which may have gone:
  neither is sent again by itself. The copy is filed in Sent (or Drafts) marked as a draft before
  it is submitted, and the mark comes off once it is sent: for one whose answer was lost, a copy no
  longer marked as a draft went, and one still marked may have gone too (the mark stays when the
  server sends it and then fails to change the copy), and the message held says so. A server that
  offers no submission, or has no identity for the
  account's address, sends nothing: the message is held and says so. Only Send sends: a message
  moved into the account's Outbox (a draft, or mail only this computer has) is held there until
  Send Again, and a copy doesn't go into it. A message that was never dated is dated before its
  first try and keeps that date, and what is read or flagged here is not in the message, so that a
  try after one that ended early uploads the same message; a copy the server has from that earlier
  try is sent instead, and filed in Sent as well wherever else it lies (moved into Sent on a server
  that keeps an email in one mailbox).
  The identity is the one the server has for the account's own address, or failing that one for
  its whole domain (`*@domain`); an alias the server keeps an identity of its own for is not used,
  so a message written from that alias is refused by the server (`forbiddenFrom`) and held. The
  copy in Sent names no Bcc recipients either, and once the server has the message this
  computer's own copy is let go of, so after sending nothing here records who got a blind copy
  (an IMAP account's copy in Sent keeps them).

Not yet: what you change in an account's folders doesn't go up.
Renaming, moving or deleting one of the server's mailboxes, deleting a folder that holds the
server's mail, and moving the server's mail into a folder the server hasn't got (one made here,
another account's) are refused with the reason, since the next Send/Receive would bring them back.
What you write in a draft the server has stays on this computer.

## The password, or the access token

The first Send/Receive asks for the account's password, or for its access token when the account
says the server takes a bearer token (the desktop app's Add Account takes it on its JMAP page, and
`mailcraft-cli sync` from the environment). MailCraft keeps it in memory until you close the page
or the app, the
server turns it down, the account's server address changes, the account is removed or another
mailbox is opened; it is never written to the browser's storage, the mailbox or a file. A server
that turns it down is not asked again by itself: its messages say so when you open them, and the
next Send/Receive you start asks for it again.

## Adding an account

**In the desktop app**, File › Add Account: type the address, press Connect, and on the page that
asks which kind of account it is, choose **JMAP server**. Give the server's name (or the address of
its session, `https://…`), tick **This is a Mailflare server** for one, and paste the token (or,
for a server that signs in with the account's password, tick that and type the password). Nothing
is sent until Connect; then the account's first Send/Receive starts. A server's name alone stands for
`https://<name>/.well-known/jmap` (RFC 8620 §2.2), or `https://<name>/jmap/session` for Mailflare.
JMAP is HTTPS: there are no IMAP or SMTP ports to set, only port 443, and mail is sent over the same
connection.

Examples (the page shows them too, under Examples):

| What | Server | How it signs in |
|---|---|---|
| Mailflare on Cloudflare's own address | `mailflare.<your-subdomain>.workers.dev` | Tick **This is a Mailflare server**; paste a Mailflare app password. The subdomain is under Workers & Pages in Cloudflare's dashboard. |
| Mailflare on a domain of your own | `mail.example.org` (the address of Mailflare's web inbox) | As above. |
| Stalwart, or another server that publishes `/.well-known/jmap` | `mail.example.org` | Tick **Sign in with my password** unless the server gave you a token. |
| A session address you know | `https://mail.example.org/jmap/session` | Used as it is; `https://` only. |

The account's address must be one the token or password may send as: the server's identity for
that address is the one mail goes out as.

**Not yet:** Fastmail. Its session names a download address on another host
(`www.fastmailusercontent.com`) than the session's (`api.fastmail.com`), and MailCraft sends the
credential to the session's own origin and nowhere else, so the connection is refused with that
reason. Fastmail works over IMAP and SMTP in the meantime.

**On the command line**, add it with `account.add` (below) and run `mailcraft-cli sync <mailbox>
--account <address>` with the secret in the environment: `MAILCRAFT_JMAP_TOKEN` for a token,
`MAILCRAFT_PASSWORD` for a password (never on the command line, which other users can list).

**In the browser build** there is no JMAP page in Add Account yet. An account comes in one of two ways:

- **The page names it.** A site that serves MailCraft can name the accounts of its users before
  MailCraft starts. Those it holds the credential for are never asked for one (see
  [For a site that serves MailCraft](#for-a-site-that-serves-mailcraft)).
- **A mailbox file brings it.** Make one with the command line, then open it with File › Open
  (it takes the place of the mailbox kept in the browser) and confirm its servers:

  ```text
  mailcraft-cli run --cmd 'account.add={"email": "you@example.org", "kind": "jmap", "jmap": {"session": "https://mail.example.org/jmap/session"}}' --save jmap.mailcraft
  ```

  `"bearer": true` beside `"session"` when the server takes an access token rather than the
  password, and `"sendsFromDraft": true` for a server that sends from the draft (Mailflare).

The session address is the address of the server's session resource, such as
`https://<server>/jmap/session`. It must be `https://` (plain `http://` only to this computer, for
trying things), and it must be the address itself: the browser build follows no redirect, so
`https://<server>/.well-known/jmap` will do only on a server that answers with the session there
rather than redirecting to it. Where the server redirects, the account's Send/Receive says to give
the session's own address.

## What the server has to allow (browser build)

The browser asks the server from the page's own address, so the server has to let that page in
(CORS):

- answer the preflight (`OPTIONS`) of `GET` and `POST` with the fields `Authorization`,
  `Content-Type` and `Accept` allowed, for the page's origin;
- send `Access-Control-Allow-Origin` with every answer, the session's, the API's, a download's and
  an upload's (a message that is sent is uploaded first, a `POST` with `Content-Type: message/rfc822`);
- send `Access-Control-Expose-Headers: Retry-After`, so that what a busy server asks is read: the
  status line then says how long it asked to wait, and the background download of whole messages
  waits at least that long.

MailCraft sends no cookies (`credentials: "omit"`), keeps nothing in the browser's HTTP cache and
sends no `Referer`. Every address the session names (the API, downloads, uploads) must be on the
same origin as the session address (scheme, name and port): MailCraft sends the credential nowhere
else.

## For a site that serves MailCraft

A page that serves the browser build can name JMAP accounts, and hold their credential itself, with
an object it sets before MailCraft starts:

```js
window.mailcraftJmap = {
  accounts: [
    { email: "ada@example.org", session: "https://mail.example.org/jmap/session", bearer: true, name: "Work" },
  ],
  // Optional: the credential of one of those accounts, a string or a promise of one, within
  // thirty seconds.
  secret(email, session) { return currentToken(email); },
};
```

- The accounts are added to the mailbox when it has none at that address. With accounts named, a
  first visit starts from an empty mailbox rather than the sample.
- With `secret`, MailCraft never asks the owner for the credential of one of those accounts, at the
  server the page named (an account the owner points at another server is the owner's to sign in
  to); each Send/Receive and each download asks the page for it, and a run the server turns it
  down for asks once more before it ends, so that the page can hand over a renewed one. MailCraft
  keeps none of it beyond the job. When the server still turns it down, no Send/Receive starts by
  itself for that account until the owner asks for one.
- Without `secret`, the owner types the credential as for any JMAP account.

## Mailflare

[Mailflare](https://github.com/hieunc229/mailflare) serves JMAP at `/jmap/session` (and
`/.well-known/jmap`) of the address it runs at. MailCraft reads it, takes what you change up to it and
sends through it, with what sets it apart from the RFC taken into account:

- **The token.** Mailflare signs a mail program in with an API key that has the `jmap` scope: make one
  in Mailflare under Settings › App passwords, and give it as the account's token (sent as a bearer
  token). One key is one Mailflare user, with the mailboxes the key may reach.
- **Its ids** have a `~` in them (`<mailbox>~inbox`, `<mailbox>~f~<folder>`, `msg~<id>`, and an
  identity's id ends in its address), which RFC 8620 §1.2 doesn't allow. MailCraft takes an id of
  letters, digits and `- _ ~ . @ + =` (never `.` or `..` alone), and writes each one in a patch as a
  JSON Pointer token (`~` as `~0`, `/` as `~1`, RFC 6901), as every server should read it.
- **Sending from the draft.** Mailflare takes new mail into a Drafts mailbox only, and sends a message
  to whom its draft names, passing over the submission's envelope. A Mailflare account is set to
  *send from the draft* (`sendsFromDraft`): its message is imported into Drafts, never Sent, with its
  `Bcc` field, which is where Mailflare finds who gets a blind copy (it hands them to its sending
  service as blind copies, and they are not in the message that goes); Mailflare then lets the draft
  go and files its own copy in Sent, which the next Send/Receive brings down. Without the setting, a
  message to such a server is held with its reason ("New messages can only be created in a Drafts
  mailbox"), and nothing is sent.
- **The address you add the account with** must be one of the Mailflare mailboxes the key may send
  from: MailCraft sends as the identity the server has for that address.
- **Its folders.** Mailflare lists each of its mailboxes as a folder with no role, with that mailbox's
  Inbox, Sent, Drafts, Archive, Spam, Trash and the owner's folders inside it; that folder itself
  stands for all the mail of the mailbox. When the key reaches one mailbox, MailCraft leaves that
  folder out and shows the Inbox and the rest at the top, as for any other account, and reads each
  message once. When it reaches two or more, each mailbox stays a folder of its own, with its
  folders inside it.
- **Cloudflare Access.** A mail program can't sign in to Cloudflare Access. If Access protects
  Mailflare, let `/jmap/*` and `/.well-known/jmap` through (the app password still protects them),
  or MailCraft is answered with Access's sign-in page instead of JMAP.

## Trying it

The command line's tests have the JMAP server they test against, in the process, served over HTTP on
this computer for a browser:

```text
cargo test -p mailcraft-cli --test jmap_loopback -- --ignored --nocapture
```

It serves `http://127.0.0.1:8780/jmap/session` with a few mailboxes and messages, lets in the bearer
token `standin-token`, and answers a page from any origin. Name the account for the browser build
in a script at the top of the `<head>` of `apps/mailcraft-web/index.html` (for the try only):

```html
<script>
  window.mailcraftJmap = { accounts: [{ email: "ada@example.org", session: "http://127.0.0.1:8780/jmap/session", bearer: true }] };
</script>
```

then serve it (`trunk serve` in `apps/mailcraft-web`) and open it: Send/Receive asks for the token.

## How it is built

- `crates/jmap` (`mailcraft-jmap`): the JMAP client, with no sockets and no browser APIs; it builds
  each request and judges each answer, and holds every answer to sizes. The in-process server
  (`standin`) is behind the `stand-in` feature, for tests.
- The engine works out what a run reads (`sync.jmapRequest`) and takes in what it brought
  (`sync.applyJmap`), and the same for messages fetched whole, only for the account and from the
  server it was asked for; it never reaches a server ([control-protocol.md](control-protocol.md)).
- The window hands a run to its host (`Services::jmap`, `Services::jmap_fetch`) with the
  credential, and takes the pieces in as they come.
- The browser build is that host (`apps/mailcraft-web/src/jmap.rs`), over `fetch`
  (`apps/mailcraft-web/src/fetch.rs`).
- The desktop app is one too (`apps/mailcraft/src/jmap_runner.rs`), each job on a thread of its own,
  over this computer's sockets (`mailcraft_host::jmap`: one HTTPS exchange of `mailcraft_net::https`
  at a time, no redirect followed, the system's trust for the certificate), and so is
  `mailcraft-cli sync`.

## Revision history

| Date | Change | Summary |
|---|---|---|
| 2026-11-14 | patch | #1074: which identity a message is sent as (not an alias with an identity of its own), and that who got a blind copy isn't kept after sending |
| 2026-11-13 | patch | #1074 part 2: a copy still marked as a draft may have gone too, and it may be in Drafts; what is read or flagged here is not in the message sent; a copy is moved into Sent on a server that keeps an email in one mailbox |
| 2026-11-13 | patch | #1074 part 2: a message whose answer was lost says to look in Sent for a copy no longer marked as a draft, since a copy is there either way |
| 2026-11-13 | patch | #1074 part 2: a message never dated keeps the date of its first try; a copy left by an earlier try is filed in Sent as well |
| 2026-11-13 | patch | #1074 part 2: a message moved into a JMAP account's Outbox is held there until Send Again; a copy doesn't go into it |
| 2026-11-12 | patch | #1074 part 1: a flag is the server's `$flagged` alone (a due date and "complete" stay here, and a flag not changed here follows the server); why a move names two mailboxes and not all of them, and what follows from it |
| 2026-11-11 | minor | #1074 part 2: mail is sent from a JMAP account through its server; what the server has to allow for an upload |
| 2026-11-11 | minor | #1074 part 1: what you change in a JMAP account's mail goes up (marks, moves between the server's mailboxes, deletions for good); what is still refused |
| 2026-10-28 | major | First version: reading a JMAP account in the browser build (M6, #226) |
