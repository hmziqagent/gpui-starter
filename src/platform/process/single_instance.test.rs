use std::{
    io::{BufRead, Write},
    sync::mpsc,
    time::Duration,
};

use tempfile::tempdir;

use super::{
    MAX_LINE_BYTES, SCHEME, append_forwarded_link, drain_forwarded_links, read_bounded_line,
    resolve_ipc_name, send_forwarded_link_via_ipc,
};
use interprocess::local_socket::{GenericNamespaced, ListenerOptions, prelude::*};

#[test]
fn forwarded_links_roundtrip_in_order() {
    let dir = tempdir().expect("tempdir");
    let queue = dir.path().join("forward.queue");

    append_forwarded_link(&queue, "gpui-starter://settings");
    append_forwarded_link(&queue, "gpui-starter://notifications");

    let links = drain_forwarded_links(&queue);
    assert_eq!(
        links,
        vec![
            "gpui-starter://settings".to_string(),
            "gpui-starter://notifications".to_string()
        ]
    );
    assert!(drain_forwarded_links(&queue).is_empty());
}

#[test]
fn forwards_link_over_ipc() {
    if !GenericNamespaced::is_supported() {
        return;
    }
    let unique_name = format!("com.gpui-starter.tests.{}", uuid::Uuid::new_v4());
    let name = resolve_ipc_name(&unique_name).expect("resolve name");
    let listener = ListenerOptions::new()
        .name(name)
        .create_sync()
        .expect("create listener");
    let (tx, rx) = mpsc::channel::<String>();
    std::thread::spawn(move || {
        if let Some(Ok(conn)) = listener.incoming().next() {
            let mut reader = std::io::BufReader::new(conn);
            let mut line = String::new();
            let _ = reader.read_line(&mut line);
            let _ = tx.send(line.trim().to_string());
        }
    });

    let sent = format!("{SCHEME}settings");
    send_forwarded_link_via_ipc(&unique_name, &sent).expect("send via ipc");
    let received = rx
        .recv_timeout(Duration::from_secs(3))
        .expect("receive forwarded link");
    assert_eq!(received, sent);
}

/// Listener plus a connected client on a throwaway name; the reader runs on
/// a thread because `read_bounded_line` blocks until a frame terminates.
fn bounded_line_harness(payload: usize) -> Option<String> {
    let unique_name = format!("com.gpui-starter.tests.{}", uuid::Uuid::new_v4());
    let name = resolve_ipc_name(&unique_name).expect("resolve name");
    let listener = ListenerOptions::new()
        .name(name)
        .create_sync()
        .expect("create listener");
    let (tx, rx) = mpsc::channel::<Option<String>>();
    std::thread::spawn(move || {
        let conn = listener.incoming().next().and_then(|c| c.ok());
        let _ = tx.send(conn.as_ref().and_then(read_bounded_line));
    });

    let mut client = interprocess::local_socket::Stream::connect(
        resolve_ipc_name(&unique_name).expect("resolve client name"),
    )
    .expect("connect client");
    writeln!(client, "{}", "a".repeat(payload)).expect("write frame");
    rx.recv_timeout(Duration::from_secs(3))
        .expect("reader finished")
}

#[test]
fn ipc_frame_at_bound_is_accepted() {
    if !GenericNamespaced::is_supported() {
        return;
    }
    // Payload of exactly MAX_LINE_BYTES plus newline: the reader's
    // take(MAX + 1) window still sees the terminator.
    let line = bounded_line_harness(MAX_LINE_BYTES).expect("frame at the cap must be accepted");
    assert_eq!(line.len(), MAX_LINE_BYTES);
}

#[test]
fn oversized_ipc_frame_is_dropped() {
    if !GenericNamespaced::is_supported() {
        return;
    }
    let line = bounded_line_harness(MAX_LINE_BYTES + 1);
    assert_eq!(line, None, "frame past the 16 KiB cap must be dropped");
}
