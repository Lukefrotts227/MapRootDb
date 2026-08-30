use maprootdb::{DatabaseValue, NodeRef, Structure};

#[test]
fn external_consumer_can_build_a_semi_strict_graph() {
    let mut graph = Structure::try_new(None, "semi-strict".to_string())
        .expect("semi-strict structure should be valid");

    graph
        .add_node(NodeRef::new(
            "root".to_string(),
            DatabaseValue::Text("project".to_string()),
        ))
        .expect("first node should be accepted");
    graph
        .add_node_with_edges(
            NodeRef::new(
                "prototype".to_string(),
                DatabaseValue::Text("experiment".to_string()),
            ),
            &["root".to_string()],
            &[],
        )
        .expect("connected node should be accepted");

    assert!(graph
        .find_node_by_key("root")
        .expect("root missing")
        .has_child_by_key("prototype"));
}
