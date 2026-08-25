mod node;
mod structure;
mod database;
mod protocol;
mod server;

use std::net::TcpStream;
use std::thread;
use std::time::Duration;

use database::{Database, DatabaseValue};
use protocol::{Command, Response, write_command, read_response};

fn send(stream: &mut TcpStream, cmd: Command) -> std::io::Result<Response> {
    write_command(stream, &cmd)?;
    read_response(stream)
}

fn run_client_tests() -> std::io::Result<()> {
    thread::sleep(Duration::from_millis(100));
    let mut s = TcpStream::connect("127.0.0.1:7878").unwrap();

    // ping
    assert!(matches!(send(&mut s, Command::Ping)?, Response::Pong), "Ping failed");
    println!("[ok] Ping");

    // add structure
    assert!(matches!(
        send(&mut s, Command::AddStructure { name: "people".into(), mode: "un-strict".into() })?,
        Response::Ok
    ), "AddStructure failed");
    println!("[ok] AddStructure 'people'");

    // add nodes
    assert!(matches!(
        send(&mut s, Command::AddNode {
            structure: "people".into(), key: "alice".into(),
            value: DatabaseValue::Text("Alice".into()),
        })?,
        Response::Ok
    ), "AddNode alice failed");

    assert!(matches!(
        send(&mut s, Command::AddNode {
            structure: "people".into(), key: "bob".into(),
            value: DatabaseValue::Int(30),
        })?,
        Response::Ok
    ), "AddNode bob failed");
    println!("[ok] AddNode alice, bob");

    // add edge
    assert!(matches!(
        send(&mut s, Command::AddEdge {
            structure: "people".into(),
            parent_key: "alice".into(),
            child_key: "bob".into(),
        })?,
        Response::Ok
    ), "AddEdge failed");
    println!("[ok] AddEdge alice -> bob");

    // get node
    let resp = send(&mut s, Command::GetNode { structure: "people".into(), key: "alice".into() })?;
    assert!(matches!(resp, Response::Value(DatabaseValue::Text(_))), "GetNode wrong value");
    println!("[ok] GetNode alice");

    // get structure
    let resp = send(&mut s, Command::GetStructure { name: "people".into() })?;
    if let Response::NodeList(nodes) = resp {
        assert_eq!(nodes.len(), 2);
        let bob_entry = nodes.iter().find(|(k, _, _)| k == "bob").expect("bob missing");
        // bob has no children
        assert!(bob_entry.2.is_empty());
        let alice_entry = nodes.iter().find(|(k, _, _)| k == "alice").expect("alice missing");
        // alice -> bob
        assert!(alice_entry.2.contains(&"bob".to_string()));
        println!("[ok] GetStructure: {} nodes, edges correct", nodes.len());
    } else {
        panic!("GetStructure returned unexpected response");
    }

    // save
    assert!(matches!(
        send(&mut s, Command::Save { path: "mydb.bin".into() })?,
        Response::Ok
    ), "Save failed");
    println!("[ok] Save mydb.bin");

    // missing structure
    let resp = send(&mut s, Command::GetNode { structure: "nope".into(), key: "x".into() })?;
    assert!(matches!(resp, Response::Error(_)), "Expected error for missing structure");
    println!("[ok] Error on missing structure");

    println!("\nAll tests passed.");
    send(&mut s, Command::Shutdown)?;
    Ok(())
}

fn main() {
    let db = Database::new();

    // The ad-hoc client integration script only runs when explicitly requested,
    // either via `cargo run -- --run-client-tests` or the RUN_CLIENT_TESTS env var.
    // This keeps `cargo run` starting a clean server by default (see PROGRESS.md T10).
    let run_tests = std::env::args().any(|a| a == "--run-client-tests")
        || std::env::var("RUN_CLIENT_TESTS").is_ok();

    if run_tests {
        thread::spawn(|| {
            if let Err(e) = run_client_tests() {
                eprintln!("run_client_tests failed: {}", e);
            }
        });
    }

    server::start(db, "127.0.0.1:7878");
    println!("Server shut down.");
}
