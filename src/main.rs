use std::io;
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use maprootdb::{
    bind_loopback, read_response, start_on_listener, write_command, Command, Database,
    DatabaseValue, Response,
};

const DEFAULT_ADDRESS: &str = "127.0.0.1:7878";

#[derive(Debug, PartialEq, Eq)]
enum CliAction {
    Help,
    Serve {
        address: String,
        database: Option<PathBuf>,
    },
    Demo {
        address: String,
        output: PathBuf,
    },
}

fn usage() -> &'static str {
    "MapRootDb - an experimental local graph database\n\n\
Usage:\n\
  maprootdb serve [--addr ADDRESS] [--database PATH]\n\
  maprootdb demo  [--addr ADDRESS] [--output PATH]\n\
  maprootdb --help\n\n\
Commands:\n\
  serve  Start the loopback-only database server.\n\
  demo   Start a temporary server, run a client exercise, save a snapshot, and stop.\n\n\
Options:\n\
  --addr ADDRESS   Loopback listening address (default: 127.0.0.1:7878)\n\
  --database PATH  Existing snapshot to load when serving\n\
  --output PATH    Demo snapshot path (default: maprootdb-demo-<pid>.bin)\n\
  -h, --help       Show this help"
}

fn next_value(
    arguments: &mut impl Iterator<Item = String>,
    option: &str,
) -> Result<String, String> {
    arguments
        .next()
        .ok_or_else(|| format!("{option} requires a value"))
}

fn parse_cli(arguments: impl IntoIterator<Item = String>) -> Result<CliAction, String> {
    let mut arguments = arguments.into_iter();
    let Some(command) = arguments.next() else {
        return Ok(CliAction::Help);
    };

    if matches!(command.as_str(), "help" | "--help" | "-h") {
        return Ok(CliAction::Help);
    }

    let legacy_demo = command == "--run-client-tests";
    match command.as_str() {
        "serve" => {
            let mut address = DEFAULT_ADDRESS.to_string();
            let mut database = None;
            while let Some(argument) = arguments.next() {
                match argument.as_str() {
                    "--addr" => address = next_value(&mut arguments, "--addr")?,
                    "--database" => {
                        database = Some(PathBuf::from(next_value(&mut arguments, "--database")?))
                    }
                    "--help" | "-h" => return Ok(CliAction::Help),
                    _ => return Err(format!("unknown serve option '{argument}'")),
                }
            }
            Ok(CliAction::Serve { address, database })
        }
        "demo" | "--run-client-tests" => {
            let mut address = DEFAULT_ADDRESS.to_string();
            let mut output = PathBuf::from(format!("maprootdb-demo-{}.bin", std::process::id()));
            while let Some(argument) = arguments.next() {
                match argument.as_str() {
                    "--addr" => address = next_value(&mut arguments, "--addr")?,
                    "--output" => output = PathBuf::from(next_value(&mut arguments, "--output")?),
                    "--help" | "-h" => return Ok(CliAction::Help),
                    _ => return Err(format!("unknown demo option '{argument}'")),
                }
            }
            if legacy_demo {
                eprintln!("warning: --run-client-tests is deprecated; use 'demo'");
            }
            Ok(CliAction::Demo { address, output })
        }
        _ => Err(format!("unknown command '{command}'")),
    }
}

fn send(stream: &mut TcpStream, command: Command) -> io::Result<Response> {
    write_command(stream, &command)?;
    read_response(stream)
}

fn expect_response(
    stream: &mut TcpStream,
    command: Command,
    expected: impl FnOnce(&Response) -> bool,
    operation: &str,
) -> io::Result<Response> {
    let response = send(stream, command)?;
    if expected(&response) {
        Ok(response)
    } else {
        Err(io::Error::other(format!(
            "{operation} returned unexpected response: {response:?}"
        )))
    }
}

fn run_demo_client(address: &str, output: &Path) -> io::Result<()> {
    let mut stream = TcpStream::connect(address)?;
    stream.set_nodelay(true)?;
    let result = (|| {
        expect_response(
            &mut stream,
            Command::Ping,
            |response| matches!(response, Response::Pong),
            "Ping",
        )?;
        println!("[ok] Ping");

        expect_response(
            &mut stream,
            Command::AddStructure {
                name: "people".into(),
                mode: "semi-strict".into(),
            },
            |response| matches!(response, Response::Ok),
            "AddStructure",
        )?;
        println!("[ok] AddStructure 'people' (semi-strict)");

        expect_response(
            &mut stream,
            Command::AddNode {
                structure: "people".into(),
                key: "alice".into(),
                value: DatabaseValue::Text("Alice".into()),
            },
            |response| matches!(response, Response::Ok),
            "AddNode alice",
        )?;
        expect_response(
            &mut stream,
            Command::AddNodeWithEdges {
                structure: "people".into(),
                key: "bob".into(),
                value: DatabaseValue::Int(30),
                parent_keys: vec!["alice".into()],
                child_keys: vec![],
            },
            |response| matches!(response, Response::Ok),
            "AddNodeWithEdges bob",
        )?;
        println!("[ok] AddNode alice, bob with edge alice -> bob");

        expect_response(
            &mut stream,
            Command::GetNode {
                structure: "people".into(),
                key: "alice".into(),
            },
            |response| matches!(response, Response::Value(DatabaseValue::Text(_))),
            "GetNode alice",
        )?;
        println!("[ok] GetNode alice");

        let response = expect_response(
            &mut stream,
            Command::GetStructure {
                name: "people".into(),
            },
            |response| matches!(response, Response::NodeList(_)),
            "GetStructure people",
        )?;
        let Response::NodeList(nodes) = response else {
            unreachable!("response was validated as NodeList");
        };
        let alice = nodes
            .iter()
            .find(|(key, _, _)| key == "alice")
            .ok_or_else(|| io::Error::other("alice missing from structure response"))?;
        if !alice.2.contains(&"bob".to_string()) {
            return Err(io::Error::other("alice -> bob edge missing"));
        }
        println!("[ok] GetStructure: {} nodes, edges correct", nodes.len());

        expect_response(
            &mut stream,
            Command::Save {
                path: output.to_string_lossy().into_owned(),
            },
            |response| matches!(response, Response::Ok),
            "Save",
        )?;
        println!("[ok] Save {}", output.display());

        expect_response(
            &mut stream,
            Command::GetNode {
                structure: "missing".into(),
                key: "x".into(),
            },
            |response| matches!(response, Response::Error(_)),
            "missing-structure error",
        )?;
        println!("[ok] Error on missing structure");
        Ok(())
    })();

    let shutdown = send(&mut stream, Command::Shutdown).map(|_| ());
    result.and(shutdown)
}

fn run_demo(address: &str, output: &Path) -> io::Result<()> {
    let listener = bind_loopback(address)?;
    let bound_address = listener.local_addr()?.to_string();
    let output = output.to_path_buf();
    let client = thread::spawn(move || run_demo_client(&bound_address, &output));

    let server_result = start_on_listener(Database::new(), listener);
    let client_result = client
        .join()
        .map_err(|_| io::Error::other("demo client thread panicked"))?;
    server_result?;
    client_result?;
    println!("\nDemo completed successfully.");
    Ok(())
}

fn load_database(database_path: Option<&Path>) -> io::Result<Database> {
    match database_path {
        Some(path) => Database::load(path.to_string_lossy().as_ref()).map_err(|error| {
            io::Error::new(
                error.kind(),
                format!("could not load snapshot {}: {error}", path.display()),
            )
        }),
        None => Ok(Database::new()),
    }
}

fn request_shutdown(address: std::net::SocketAddr) -> io::Result<()> {
    let mut stream = TcpStream::connect(address)?;
    stream.set_nodelay(true)?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    match send(&mut stream, Command::Shutdown)? {
        Response::Ok => Ok(()),
        response => Err(io::Error::other(format!("Shutdown returned {response:?}"))),
    }
}

fn install_ctrlc_shutdown(address: std::net::SocketAddr) -> io::Result<()> {
    ctrlc::set_handler(move || {
        let _ = request_shutdown(address);
    })
    .map_err(|error| io::Error::other(format!("could not install Ctrl+C handler: {error}")))
}

fn run_server(address: &str, database_path: Option<&Path>) -> io::Result<()> {
    let database = load_database(database_path)?;
    if let Some(path) = database_path {
        println!("Loaded database from {}", path.display());
    }

    let listener = bind_loopback(address)?;
    let bound_address = listener.local_addr()?;
    install_ctrlc_shutdown(bound_address)?;
    println!("Press Ctrl+C or send Command::Shutdown to stop cleanly.");
    start_on_listener(database, listener)
}

fn main() -> io::Result<()> {
    let action = parse_cli(std::env::args().skip(1))
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;

    match action {
        CliAction::Help => {
            println!("{}", usage());
            Ok(())
        }
        CliAction::Serve { address, database } => {
            run_server(&address, database.as_deref())?;
            println!("Server shut down.");
            Ok(())
        }
        CliAction::Demo { address, output } => run_demo(&address, &output),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_is_the_default_and_explicit_help_is_supported() {
        assert_eq!(parse_cli(Vec::<String>::new()).unwrap(), CliAction::Help);
        assert_eq!(parse_cli(["--help".to_string()]).unwrap(), CliAction::Help);
    }

    #[test]
    fn serve_options_are_parsed() {
        assert_eq!(
            parse_cli([
                "serve".to_string(),
                "--addr".to_string(),
                "127.0.0.1:9000".to_string(),
                "--database".to_string(),
                "data.bin".to_string(),
            ])
            .unwrap(),
            CliAction::Serve {
                address: "127.0.0.1:9000".to_string(),
                database: Some(PathBuf::from("data.bin")),
            }
        );
    }

    #[test]
    fn demo_options_are_parsed() {
        assert_eq!(
            parse_cli([
                "demo".to_string(),
                "--addr".to_string(),
                "127.0.0.1:0".to_string(),
                "--output".to_string(),
                "demo.bin".to_string(),
            ])
            .unwrap(),
            CliAction::Demo {
                address: "127.0.0.1:0".to_string(),
                output: PathBuf::from("demo.bin"),
            }
        );
    }

    #[test]
    fn unknown_arguments_are_rejected() {
        assert!(parse_cli(["--typo".to_string()]).is_err());
        assert!(parse_cli(["serve".to_string(), "--typo".to_string()]).is_err());
    }

    #[test]
    fn demo_default_output_is_process_specific() {
        let CliAction::Demo { output, .. } = parse_cli(["demo".to_string()]).unwrap() else {
            panic!("demo command was not parsed as a demo");
        };
        assert_eq!(
            output,
            PathBuf::from(format!("maprootdb-demo-{}.bin", std::process::id()))
        );
    }

    #[test]
    fn explicitly_missing_snapshot_is_an_error() {
        let path = std::env::temp_dir().join(format!(
            "maprootdb-definitely-missing-{}.bin",
            std::process::id()
        ));
        let error = match load_database(Some(&path)) {
            Ok(_) => panic!("missing snapshot was accepted"),
            Err(error) => error,
        };
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
        assert!(error.to_string().contains("could not load snapshot"));
    }

    #[test]
    fn shutdown_request_stops_a_running_server() {
        let listener = bind_loopback("127.0.0.1:0").expect("fixture bind failed");
        let address = listener.local_addr().expect("fixture address missing");
        let server = thread::spawn(move || start_on_listener(Database::new(), listener));

        request_shutdown(address).expect("shutdown request failed");
        server
            .join()
            .expect("server thread panicked")
            .expect("server failed");
    }
}
