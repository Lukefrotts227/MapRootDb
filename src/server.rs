use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;
use std::thread;

use crate::database::Database;
use crate::node::NodeRef;
use crate::structure::Structure;
use crate::protocol::{Command, Response, read_command, write_response};

pub fn execute(db: &mut Database, command: Command) -> Response {
    match command {
        Command::Ping => Response::Pong,

        Command::AddStructure { name, mode } => {
            db.add_structure(name, Structure::new(None, mode));
            Response::Ok
        }

        Command::AddNode { structure, key, value } => {
            match db.get_structure_mut(&structure) {
                Some(s) => match s.add_node(NodeRef::new(key, value)) {
                    Ok(_)  => Response::Ok,
                    Err(_) => Response::Error("Add failed: mode violation".to_string()),
                },
                None => Response::Error(format!("Structure '{}' not found", structure)),
            }
        }

        Command::AddEdge { structure, parent_key, child_key } => {
            match db.get_structure_mut(&structure) {
                Some(s) => {
                    let parent = s.find_node_by_key(&parent_key);
                    let child  = s.find_node_by_key(&child_key);
                    match (parent, child) {
                        (Some(mut p), Some(c)) => { p.add_child(c); Response::Ok }
                        _ => Response::Error("Parent or child not found".to_string()),
                    }
                }
                None => Response::Error(format!("Structure '{}' not found", structure)),
            }
        }

        Command::GetNode { structure, key } => {
            match db.get_structure(&structure) {
                Some(s) => match s.find_node_by_key(&key) {
                    Some(n) => Response::Value(n.value()),
                    None    => Response::Error(format!("Node '{}' not found", key)),
                },
                None => Response::Error(format!("Structure '{}' not found", structure)),
            }
        }

        Command::GetStructure { name } => {
            match db.get_structure(&name) {
                Some(s) => {
                    let nodes = s.nodes.iter().map(|(k, n)| {
                        let children = n.children().iter().map(|c| c.key()).collect();
                        (k.clone(), n.value(), children)
                    }).collect();
                    Response::NodeList(nodes)
                }
                None => Response::Error(format!("Structure '{}' not found", name)),
            }
        }

        Command::DeleteNode { structure, key } => {
            match db.get_structure_mut(&structure) {
                Some(s) => {
                    if s.delete_node_by_key(&key) { Response::Ok }
                    else { Response::Error(format!("Could not delete '{}'", key)) }
                }
                None => Response::Error(format!("Structure '{}' not found", structure)),
            }
        }

        Command::Save { path } => {
            match db.save(&path) {
                Ok(_)  => Response::Ok,
                Err(e) => Response::Error(e.to_string()),
            }
        }

        Command::Shutdown => Response::Ok,
    }
}

fn handle_connection(
    mut stream: TcpStream,
    cmd_tx: mpsc::Sender<(Command, mpsc::Sender<Response>)>,
) {
    loop {
        let command = match read_command(&mut stream) {
            Ok(cmd) => cmd,
            Err(_)  => break,
        };

        let shutdown = matches!(command, Command::Shutdown);

        let (resp_tx, resp_rx) = mpsc::channel();
        if cmd_tx.send((command, resp_tx)).is_err() { break; }

        let response = match resp_rx.recv() {
            Ok(r)  => r,
            Err(_) => break,
        };

        if write_response(&mut stream, &response).is_err() { break; }
        if shutdown { break; }
    }
}

// Starts the server. Blocks the calling thread — that thread owns and drives the Database.
// Other threads communicate via the internal command channel.
pub fn start(mut db: Database, addr: &str) {
    let (cmd_tx, cmd_rx) = mpsc::channel::<(Command, mpsc::Sender<Response>)>();

    let addr_string = addr.to_string();
    let cmd_tx_for_listener = cmd_tx.clone();
    thread::spawn(move || {
        let listener = TcpListener::bind(&addr_string).expect("Failed to bind");
        println!("Listening on {addr_string}");
        for stream in listener.incoming() {
            match stream {
                Ok(s) => {
                    let tx = cmd_tx_for_listener.clone();
                    thread::spawn(move || handle_connection(s, tx));
                }
                Err(e) => eprintln!("Accept error: {e}"),
            }
        }
    });

    // DB loop — lives on this thread, never touches Rc across threads
    for (command, resp_tx) in &cmd_rx {
        let shutdown = matches!(command, Command::Shutdown);
        let response = execute(&mut db, command);
        let _ = resp_tx.send(response);
        if shutdown { break; }
    }
}
