use maprootdb::{Database, DatabaseValue, NodeRef, Structure};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut graph = Structure::try_new(None, "semi-strict".to_string())?;
    graph.add_node(NodeRef::new(
        "idea".to_string(),
        DatabaseValue::Text("MapRootDb".to_string()),
    ))?;
    graph.add_node_with_edges(
        NodeRef::new(
            "prototype".to_string(),
            DatabaseValue::Text("working graph".to_string()),
        ),
        &["idea".to_string()],
        &[],
    )?;
    graph.add_node_with_edges(
        NodeRef::new(
            "release".to_string(),
            DatabaseValue::Text("public alpha".to_string()),
        ),
        &["prototype".to_string()],
        &[],
    )?;

    let mut database = Database::new();
    database.add_structure("roadmap".to_string(), graph)?;

    let path = std::env::temp_dir().join("maprootdb-three-node-example.bin");
    let path_text = path.to_string_lossy();
    database.save(&path_text)?;
    let restored = Database::load(&path_text)?;
    std::fs::remove_file(path)?;

    let roadmap = restored
        .get_structure("roadmap")
        .ok_or("restored roadmap is missing")?;
    let release = roadmap
        .find_node_by_key("release")
        .ok_or("restored release node is missing")?;
    let prototype = release
        .get_parent_by_key("prototype")
        .ok_or("release has no prototype parent")?;

    assert_eq!(
        release.value(),
        DatabaseValue::Text("public alpha".to_string())
    );
    assert!(prototype.has_child_by_key("release"));
    println!(
        "restored {} nodes in {} mode; prototype -> release is queryable",
        roadmap.len(),
        roadmap.mode()
    );
    Ok(())
}
