use std::io;
use std::net::TcpStream;

use maprootdb::{read_response, write_command, Command, DatabaseValue, Response};

fn send(stream: &mut TcpStream, command: Command) -> io::Result<Response> {
    write_command(stream, &command)?;
    read_response(stream)
}

fn require_ok(response: Response, operation: &str) -> io::Result<()> {
    match response {
        Response::Ok => Ok(()),
        other => Err(io::Error::other(format!("{operation} returned {other:?}"))),
    }
}

fn main() -> io::Result<()> {
    let address = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:7878".to_string());
    let snapshot = std::env::args()
        .nth(2)
        .unwrap_or_else(|| "tcp-client-demo.bin".to_string());

    let mut stream = TcpStream::connect(&address)?;
    stream.set_nodelay(true)?;

    match send(&mut stream, Command::Ping)? {
        Response::Pong => println!("server is ready at {address}"),
        other => return Err(io::Error::other(format!("Ping returned {other:?}"))),
    }

    require_ok(
        send(
            &mut stream,
            Command::AddStructure {
                name: "client-demo".to_string(),
                mode: "semi-strict".to_string(),
            },
        )?,
        "AddStructure",
    )?;
    require_ok(
        send(
            &mut stream,
            Command::AddNode {
                structure: "client-demo".to_string(),
                key: "root".to_string(),
                value: DatabaseValue::Text("root value".to_string()),
            },
        )?,
        "AddNode root",
    )?;
    require_ok(
        send(
            &mut stream,
            Command::AddNodeWithEdges {
                structure: "client-demo".to_string(),
                key: "child".to_string(),
                value: DatabaseValue::Int(2),
                parent_keys: vec!["root".to_string()],
                child_keys: vec![],
            },
        )?,
        "AddNodeWithEdges child",
    )?;

    match send(
        &mut stream,
        Command::GetStructure {
            name: "client-demo".to_string(),
        },
    )? {
        Response::NodeList(nodes) => println!("queried {} nodes", nodes.len()),
        other => return Err(io::Error::other(format!("GetStructure returned {other:?}"))),
    }

    require_ok(
        send(
            &mut stream,
            Command::Save {
                path: snapshot.clone(),
            },
        )?,
        "Save",
    )?;
    require_ok(send(&mut stream, Command::Shutdown)?, "Shutdown")?;
    println!("saved {snapshot} and stopped the server cleanly");
    Ok(())
}
