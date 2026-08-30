use std::net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::Duration;

use crate::database::Database;
use crate::node::NodeRef;
use crate::protocol::{read_command, write_response, Command, Response};
use crate::structure::Structure;

/// Executes one command directly against a database without using TCP.
pub fn execute(db: &mut Database, command: Command) -> Response {
    match command {
        Command::Ping => Response::Pong,

        Command::AddStructure { name, mode } => {
            if name.is_empty() {
                return Response::Error("Structure name cannot be empty".to_string());
            }
            if !matches!(mode.as_str(), "semi-strict" | "un-strict") {
                return Response::Error(format!("Unknown structure mode '{mode}'"));
            }
            if db.get_structure(&name).is_some() {
                return Response::Error(format!("Structure '{name}' already exists"));
            }
            match db.add_structure(name, Structure::new(None, mode)) {
                Ok(()) => Response::Ok,
                Err(error) => Response::Error(error.to_string()),
            }
        }

        Command::AddNode {
            structure,
            key,
            value,
        } => {
            if key.is_empty() {
                return Response::Error("Node key cannot be empty".to_string());
            }
            match db.get_structure_mut(&structure) {
                Some(s) => match s.add_node(NodeRef::new(key, value)) {
                    Ok(_) => Response::Ok,
                    Err(error) => Response::Error(error.to_string()),
                },
                None => Response::Error(format!("Structure '{}' not found", structure)),
            }
        }

        Command::AddNodeWithEdges {
            structure,
            key,
            value,
            parent_keys,
            child_keys,
        } => {
            if key.is_empty() {
                return Response::Error("Node key cannot be empty".to_string());
            }
            match db.get_structure_mut(&structure) {
                Some(s) => {
                    match s.add_node_with_edges(NodeRef::new(key, value), &parent_keys, &child_keys)
                    {
                        Ok(_) => Response::Ok,
                        Err(error) => Response::Error(error.to_string()),
                    }
                }
                None => Response::Error(format!("Structure '{}' not found", structure)),
            }
        }

        Command::AddEdge {
            structure,
            parent_key,
            child_key,
        } => match db.get_structure_mut(&structure) {
            Some(s) => match s.add_edge_by_key(&parent_key, &child_key) {
                Ok(()) => Response::Ok,
                Err(error) => Response::Error(error.to_string()),
            },
            None => Response::Error(format!("Structure '{}' not found", structure)),
        },

        Command::GetNode { structure, key } => match db.get_structure(&structure) {
            Some(s) => match s.find_node_by_key(&key) {
                Some(n) => Response::Value(n.value()),
                None => Response::Error(format!("Node '{}' not found", key)),
            },
            None => Response::Error(format!("Structure '{}' not found", structure)),
        },

        Command::GetStructure { name } => match db.get_structure(&name) {
            Some(s) => {
                let nodes = s
                    .iter_nodes()
                    .map(|(k, n)| {
                        let children = n.children().iter().map(|c| c.key()).collect();
                        (k.clone(), n.value(), children)
                    })
                    .collect();
                Response::NodeList(nodes)
            }
            None => Response::Error(format!("Structure '{}' not found", name)),
        },

        Command::DeleteNode { structure, key } => match db.get_structure_mut(&structure) {
            Some(s) => {
                if s.delete_node_by_key(&key) {
                    Response::Ok
                } else {
                    Response::Error(format!("Could not delete '{}'", key))
                }
            }
            None => Response::Error(format!("Structure '{}' not found", structure)),
        },

        Command::Save { path } => match db.save(&path) {
            Ok(_) => Response::Ok,
            Err(e) => Response::Error(e.to_string()),
        },

        Command::Shutdown => Response::Ok,
    }
}

fn handle_connection(
    mut stream: TcpStream,
    cmd_tx: mpsc::Sender<(Command, mpsc::Sender<Response>)>,
) {
    loop {
        let command = match read_command(&mut stream) {
            Ok(command) => command,
            Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(error) => {
                eprintln!("Connection read error: {error}");
                break;
            }
        };
        let shutdown = matches!(command, Command::Shutdown);

        let (resp_tx, resp_rx) = mpsc::channel();
        if cmd_tx.send((command, resp_tx)).is_err() {
            break;
        }

        let response = match resp_rx.recv() {
            Ok(r) => r,
            Err(_) => break,
        };

        if write_response(&mut stream, &response).is_err() {
            break;
        }
        if shutdown {
            break;
        }
    }
}

// Starts the server. Blocks the calling thread — that thread owns and drives the Database.
// Other threads communicate via the internal command channel.
fn reject_non_loopback(address: SocketAddr) -> std::io::Result<SocketAddr> {
    if address.ip().is_loopback() {
        Ok(address)
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            format!(
                "refusing non-loopback address {address}; MapRootDb has no authentication or encryption"
            ),
        ))
    }
}

/// Resolves and binds an address only when every result is a loopback address.
///
/// MapRootDb's protocol has no authentication or encryption, and clients can request
/// snapshot writes on the server host. Non-loopback listeners are therefore rejected.
pub fn bind_loopback(addr: &str) -> std::io::Result<TcpListener> {
    let addresses: Vec<_> = addr.to_socket_addrs()?.collect();
    if addresses.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("address '{addr}' did not resolve"),
        ));
    }
    for address in &addresses {
        reject_non_loopback(*address)?;
    }
    TcpListener::bind(addresses.as_slice())
}

/// Binds a loopback-only `addr` and serves commands until a client sends
/// [`Command::Shutdown`].
pub fn start(db: Database, addr: &str) -> std::io::Result<()> {
    let listener = bind_loopback(addr)?;
    start_on_listener(db, listener)
}

/// Serves commands on an already-bound listener until a client requests shutdown.
///
/// This is useful for callers that need to select an ephemeral port or configure the
/// listener before handing ownership to MapRootDb.
pub fn start_on_listener(mut db: Database, listener: TcpListener) -> std::io::Result<()> {
    let address = reject_non_loopback(listener.local_addr()?)?;
    listener.set_nonblocking(true)?;
    println!("Listening on {address}");

    let (cmd_tx, cmd_rx) = mpsc::channel::<(Command, mpsc::Sender<Response>)>();
    let stopping = Arc::new(AtomicBool::new(false));
    let listener_stopping = Arc::clone(&stopping);
    let cmd_tx_for_listener = cmd_tx.clone();
    let listener_thread = thread::spawn(move || {
        while !listener_stopping.load(Ordering::Acquire) {
            match listener.accept() {
                Ok((s, _)) => {
                    let timeout = Some(Duration::from_secs(30));
                    if let Err(error) = s.set_nonblocking(false) {
                        eprintln!("Connection blocking-mode error: {error}");
                        continue;
                    }
                    if let Err(error) = s.set_nodelay(true) {
                        eprintln!("Connection nodelay error: {error}");
                        continue;
                    }
                    if let Err(error) = s.set_read_timeout(timeout) {
                        eprintln!("Connection read-timeout error: {error}");
                        continue;
                    }
                    if let Err(error) = s.set_write_timeout(timeout) {
                        eprintln!("Connection write-timeout error: {error}");
                        continue;
                    }
                    let tx = cmd_tx_for_listener.clone();
                    thread::spawn(move || handle_connection(s, tx));
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(10));
                }
                Err(error) => {
                    eprintln!("Accept error: {error}");
                    break;
                }
            }
        }
    });

    // DB loop — lives on this thread, never touches Rc across threads
    for (command, resp_tx) in &cmd_rx {
        let shutdown = matches!(command, Command::Shutdown);
        let response = execute(&mut db, command);
        let _ = resp_tx.send(response);
        if shutdown {
            break;
        }
    }

    stopping.store(true, Ordering::Release);
    listener_thread
        .join()
        .map_err(|_| std::io::Error::other("listener thread panicked"))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::DatabaseValue;
    use crate::protocol::{read_response, write_command};

    #[test]
    fn self_edge_returns_error_instead_of_panicking() {
        let mut db = Database::new();
        assert!(matches!(
            execute(
                &mut db,
                Command::AddStructure {
                    name: "graph".to_string(),
                    mode: "un-strict".to_string(),
                },
            ),
            Response::Ok
        ));
        assert!(matches!(
            execute(
                &mut db,
                Command::AddNode {
                    structure: "graph".to_string(),
                    key: "node".to_string(),
                    value: DatabaseValue::Null,
                },
            ),
            Response::Ok
        ));

        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            execute(
                &mut db,
                Command::AddEdge {
                    structure: "graph".to_string(),
                    parent_key: "node".to_string(),
                    child_key: "node".to_string(),
                },
            )
        }));

        assert!(outcome.is_ok(), "self-edge command panicked");
        assert!(matches!(outcome.unwrap(), Response::Error(_)));
    }

    #[test]
    fn invalid_structure_mode_is_rejected() {
        let mut db = Database::new();
        let response = execute(
            &mut db,
            Command::AddStructure {
                name: "graph".to_string(),
                mode: "typo".to_string(),
            },
        );

        assert!(matches!(response, Response::Error(_)));
        assert!(db.get_structure("graph").is_none());
    }

    #[test]
    fn duplicate_structure_and_node_names_are_rejected() {
        let mut db = Database::new();
        let add_structure = || Command::AddStructure {
            name: "graph".to_string(),
            mode: "un-strict".to_string(),
        };
        assert!(matches!(execute(&mut db, add_structure()), Response::Ok));
        assert!(matches!(
            execute(&mut db, add_structure()),
            Response::Error(_)
        ));

        let add_node = |value| Command::AddNode {
            structure: "graph".to_string(),
            key: "node".to_string(),
            value,
        };
        assert!(matches!(
            execute(&mut db, add_node(DatabaseValue::Int(1))),
            Response::Ok
        ));
        assert!(matches!(
            execute(&mut db, add_node(DatabaseValue::Int(2))),
            Response::Error(_)
        ));
        assert_eq!(
            db.get_structure("graph")
                .unwrap()
                .find_node_by_key("node")
                .unwrap()
                .value(),
            DatabaseValue::Int(1)
        );
    }

    #[test]
    fn bind_failure_is_returned_to_caller() {
        let occupied = TcpListener::bind("127.0.0.1:0").expect("fixture bind failed");
        let address = occupied.local_addr().expect("fixture address missing");

        let result = start(Database::new(), &address.to_string());

        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::AddrInUse);
    }

    #[test]
    fn non_loopback_addresses_are_rejected() {
        let result = start(Database::new(), "0.0.0.0:0");
        assert_eq!(
            result.unwrap_err().kind(),
            std::io::ErrorKind::PermissionDenied
        );

        let listener = TcpListener::bind("0.0.0.0:0").expect("fixture bind failed");
        let result = start_on_listener(Database::new(), listener);
        assert_eq!(
            result.unwrap_err().kind(),
            std::io::ErrorKind::PermissionDenied
        );
    }

    #[test]
    fn semi_strict_nodes_can_be_added_through_public_commands() {
        let mut db = Database::new();
        assert!(matches!(
            execute(
                &mut db,
                Command::AddStructure {
                    name: "graph".to_string(),
                    mode: "semi-strict".to_string(),
                },
            ),
            Response::Ok
        ));
        assert!(matches!(
            execute(
                &mut db,
                Command::AddNode {
                    structure: "graph".to_string(),
                    key: "root".to_string(),
                    value: DatabaseValue::Int(1),
                },
            ),
            Response::Ok
        ));
        assert!(matches!(
            execute(
                &mut db,
                Command::AddNodeWithEdges {
                    structure: "graph".to_string(),
                    key: "child".to_string(),
                    value: DatabaseValue::Int(2),
                    parent_keys: vec!["root".to_string()],
                    child_keys: vec![],
                },
            ),
            Response::Ok
        ));

        let graph = db.get_structure("graph").expect("graph missing");
        assert!(graph
            .find_node_by_key("root")
            .unwrap()
            .has_child_by_key("child"));
    }

    #[test]
    fn persistent_tcp_connection_handles_multiple_commands() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("fixture bind failed");
        let address = listener.local_addr().expect("fixture address missing");
        let server = thread::spawn(move || start_on_listener(Database::new(), listener));

        let mut stream = TcpStream::connect(address).expect("client connect failed");
        stream
            .set_nodelay(true)
            .expect("client nodelay setup failed");
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .expect("client timeout setup failed");

        for command in [Command::Ping, Command::Ping, Command::Shutdown] {
            write_command(&mut stream, &command).expect("command write failed");
            let response = read_response(&mut stream).expect("response read failed");
            assert!(matches!(response, Response::Pong | Response::Ok));
        }

        server
            .join()
            .expect("server thread panicked")
            .expect("server failed");
    }
}
