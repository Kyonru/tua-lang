use super::*;

#[test]
fn object_pool_graph_returns_lifecycle_operations_and_insights() {
    let temp = TempProject::new("object-pool-graph-inspection");

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write(
        "src/main.tua",
        r#"type Bullet = { active: boolean }

local bullets = objectPool({
  create = function(): Bullet
    return { active = false }
  end,
})

local leaked = bullets:acquire()
local released = bullets:acquire()
bullets:release(released)
"#,
    );

    let output = run_messages(vec![
        initialize_with_root(1, temp.uri()),
        object_pool_graph_request(2),
    ]);
    let result = &response_with_id(&output, 2)["result"];
    let graph = &result["graph"];
    let pools = graph["pools"].as_array().expect("pools should be an array");
    let operations = graph["operations"]
        .as_array()
        .expect("operations should be an array");
    let insights = graph["insights"]
        .as_array()
        .expect("insights should be an array");

    assert!(result["analysisRevision"].as_u64().is_some());
    assert_eq!(pools.len(), 1);
    assert_eq!(pools[0]["name"], "bullets");
    assert_eq!(pools[0]["itemType"], "Bullet");
    assert_eq!(pools[0]["uri"], temp.uri_for("src/main.tua"));
    assert!(pools[0]["range"]["start"]["line"].as_u64().is_some());

    assert_eq!(operations.len(), 3);
    assert_eq!(operations[0]["kind"], "acquire");
    assert_eq!(operations[0]["itemName"], "leaked");
    assert_eq!(operations[1]["kind"], "acquire");
    assert_eq!(operations[1]["itemName"], "released");
    assert_eq!(operations[2]["kind"], "release");
    assert_eq!(operations[2]["itemName"], "released");
    assert_eq!(operations[0]["poolId"], pools[0]["id"]);

    assert_eq!(insights.len(), 1);
    assert_eq!(insights[0]["kind"], "unreleasedAcquire");
    assert_eq!(insights[0]["itemName"], "leaked");
    assert_eq!(insights[0]["poolId"], pools[0]["id"]);
    assert_eq!(insights[0]["acquireOperationId"], operations[0]["id"]);
}
