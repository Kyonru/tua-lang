use super::*;

#[test]
fn ecs_graph_returns_components_system_access_queries_and_insights() {
    let temp = TempProject::new("ecs-graph-inspection");

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write(
        "src/main.tua",
        r#"local Position = ecs.component("Position", { x = "number", y = "number" })
local Velocity = ecs.component("Velocity", { x = "number", y = "number" })
local Sprite = ecs.component("Sprite", { image = "Image" })
local Health = ecs.component("Health", { value = "number" })
local world = ecs.world()
local movement = ecs.system(world, { Position, Velocity }, function(_, position, velocity, dt)
  position.x += velocity.x * dt
  local scratch = { x = position.x }
end)
local crowded = world:query({ Position, Velocity, Sprite, Health })
"#,
    );

    let output = run_messages(vec![
        initialize_with_root(1, temp.uri()),
        ecs_graph_request(2),
    ]);
    let result = &response_with_id(&output, 2)["result"];
    let graph = &result["graph"];
    let components = graph["components"]
        .as_array()
        .expect("components should be an array");
    let systems = graph["systems"]
        .as_array()
        .expect("systems should be an array");
    let queries = graph["queries"]
        .as_array()
        .expect("queries should be an array");
    let insights = graph["insights"]
        .as_array()
        .expect("insights should be an array");

    assert!(result["analysisRevision"].as_u64().is_some());
    assert_eq!(components.len(), 4);
    assert_eq!(systems.len(), 1);
    assert_eq!(queries.len(), 2);
    assert_eq!(systems[0]["name"], "movement");
    assert_eq!(systems[0]["components"][0]["access"], "readWrite");
    assert_eq!(systems[0]["components"][1]["access"], "read");
    assert!(systems[0]["components"][0]["componentId"].is_string());
    assert!(queries.iter().any(|query| query["kind"] == "standalone"));
    assert!(queries.iter().any(|query| query["kind"] == "system"));
    assert!(insights
        .iter()
        .any(|insight| insight["kind"] == "callbackAllocation"));
    assert!(insights
        .iter()
        .any(|insight| insight["kind"] == "wideQueryAllocation"));
}

#[test]
fn ecs_graph_resolves_module_qualified_components_through_compiler_definitions() {
    let temp = TempProject::new("ecs-graph-module-component");

    temp.write("tua.toml", "source = \"src\"\n");
    temp.write(
        "src/components.tua",
        r#"local Position = ecs.component("Position", { x = "number" })
return { Position = Position }
"#,
    );
    temp.write(
        "src/main.tua",
        r#"local Components = require("components")
local world = ecs.world()
local positioned = world:query({ Components.Position })
"#,
    );

    let output = run_messages(vec![
        initialize_with_root(1, temp.uri()),
        ecs_graph_request(2),
    ]);
    let graph = &response_with_id(&output, 2)["result"]["graph"];
    let component_id = graph["components"][0]["id"]
        .as_str()
        .expect("component should have an ID");
    let query = graph["queries"]
        .as_array()
        .expect("queries should be an array")
        .iter()
        .find(|query| query["name"] == "positioned")
        .expect("qualified query should be indexed");

    assert_eq!(query["components"][0]["reference"], "Components.Position");
    assert_eq!(query["components"][0]["componentId"], component_id);
}
