use super::*;

fn format(source: &str) -> String {
    format_tua_source("src/main.tua", source, &FormatOptions::default())
        .expect("source should format")
}

#[test]
fn formats_blocks_assignments_calls_and_comments() {
    let source = r#"function love.update(dt)
local hp=10
if hp>0 then
damage(player,15, "fast")   -- keep me
else
heal(player,5)
end
end"#;

    assert_eq!(
        format(source),
        r#"function love.update(dt)
  local hp = 10
  if hp>0 then
    damage(player, 15, "fast") -- keep me
  else
    heal(player, 5)
  end
end
"#
    );
}

#[test]
fn preserves_tua_specific_syntax() {
    let source = r#"type Player={hp:number,name:string}
enum Direction
  "North"
  "South"
end
local hp:number=10
function damage(player:Player, amount:number):number
player.hp-=amount
return player.hp
end"#;

    assert_eq!(
        format(source),
        r#"type Player = {hp:number, name:string}
enum Direction
  "North"
  "South"
end
local hp:number = 10
function damage(player:Player, amount:number):number
  player.hp -= amount
  return player.hp
end
"#
    );
}

#[test]
fn formats_local_function_blocks() {
    let source = r#"local function makePlayer(name:string):Player
return {name=name,hp=100}
end
local inline = function() return 1 end
local player=makePlayer("Ada")"#;

    assert_eq!(
        format(source),
        r#"local function makePlayer(name:string):Player
  return {name = name, hp = 100}
end
local inline = function() return 1 end
local player = makePlayer("Ada")
"#
    );
}

#[test]
fn formats_default_parameters() {
    let source = r#"function shake(duration=0.3, strength:number=1)
return duration*strength
end"#;

    assert_eq!(
        format(source),
        r#"function shake(duration = 0.3, strength:number = 1)
  return duration*strength
end
"#
    );
}

#[test]
fn formats_optional_parameters() {
    let source = r#"function shake(duration:number, easing?:string)
return duration
end"#;

    assert_eq!(
        format(source),
        r#"function shake(duration:number, easing?:string)
  return duration
end
"#
    );
}

#[test]
fn formats_multiline_type_and_table_bodies() {
    let source = r#"type Vec2 = {
x: number,
y: number,
}

type Player = {
name: string,
pos: Vec2,
hp: number,
alive: boolean,
}

local function makePlayer(name: string): Player
  return {
  name = name,
  pos = { x = 0, y = 0 },
  hp = 100,
  alive = true,
  }
end

return {
makePlayer = makePlayer,
sample = player,
}"#;

    assert_eq!(
        format(source),
        r#"type Vec2 = {
  x: number,
  y: number,
}

type Player = {
  name: string,
  pos: Vec2,
  hp: number,
  alive: boolean,
}

local function makePlayer(name: string): Player
  return {
    name = name,
    pos = { x = 0, y = 0 },
    hp = 100,
    alive = true,
  }
end

return {
  makePlayer = makePlayer,
  sample = player,
}
"#
    );
}

#[test]
fn breaks_long_table_groups() {
    let source =
        r#"local values = { alpha = 1, beta = 2, gamma = 3, delta = 4, epsilon = 5, zeta = 6 }"#;
    let options = FormatOptions {
        line_width: 60,
        ..FormatOptions::default()
    };

    let output = format_tua_source("src/main.tua", source, &options).expect("source should format");

    assert_eq!(
        output,
        r#"local values = {
  alpha = 1,
  beta = 2,
  gamma = 3,
  delta = 4,
  epsilon = 5,
  zeta = 6,
}
"#
    );
}

#[test]
fn breaks_long_call_groups_without_trailing_argument_comma() {
    let source = r#"local hit = intersects(player.x, player.y, player.width, player.height, wall.x, wall.y, wall.width, wall.height)"#;
    let options = FormatOptions {
        line_width: 60,
        ..FormatOptions::default()
    };

    let output = format_tua_source("src/main.tua", source, &options).expect("source should format");

    assert_eq!(
        output,
        r#"local hit = intersects(
  player.x,
  player.y,
  player.width,
  player.height,
  wall.x,
  wall.y,
  wall.width,
  wall.height
)
"#
    );
    parse_tua_source("src/main.tua", output).expect("formatted call should parse");
}

#[test]
fn preserves_multiline_call_indentation_and_grouping_spacing() {
    let source = r#"function draw()
love.graphics.draw(
sprite,
8 + (i - 1),
40 / sprite:getWidth()
)
end"#;

    assert_eq!(
        format(source),
        r#"function draw()
  love.graphics.draw(
    sprite,
    8 + (i - 1),
    40 / sprite:getWidth()
  )
end
"#
    );
}

#[test]
fn indents_continued_assignment_calls() {
    let source = r#"function update()
local hit =
utils.AABB_collision(player.x, player.y, player.width, player.height, wall.x, wall.y, wall.width, wall.height)
end"#;
    let options = FormatOptions {
        line_width: 60,
        ..FormatOptions::default()
    };

    let output = format_tua_source("src/main.tua", source, &options).expect("source should format");

    assert_eq!(
        output,
        r#"function update()
  local hit =
    utils.AABB_collision(
      player.x,
      player.y,
      player.width,
      player.height,
      wall.x,
      wall.y,
      wall.width,
      wall.height
    )
end
"#
    );
    parse_tua_source("src/main.tua", output).expect("formatted call should parse");
}

#[test]
fn indents_leading_concat_continuation() {
    let source = r#"local label =
"alpha"
.. "beta""#;

    assert_eq!(
        format(source),
        r#"local label =
  "alpha"
  .. "beta"
"#
    );
}

#[test]
fn avoids_extra_paren_indent_inside_call_table_and_function_blocks() {
    let source = r#"function update()
table.insert(list, {
x = 1,
y = 2,
})
player.damage(1, function()
print("hit")
end)
end"#;

    assert_eq!(
        format(source),
        r#"function update()
  table.insert(list, {
    x = 1,
    y = 2,
  })
  player.damage(1, function()
    print("hit")
  end)
end
"#
    );
}

#[test]
fn keeps_multiline_function_argument_bodies_relative_to_the_call() {
    let source = r#"local stopScore = game:watch(
  "score",
  function(score: number, previous: number, state: GameState)
    print("score", previous, score, state.playerName)
  end
)
"#;

    assert_eq!(format(source), source);
}

#[test]
fn is_idempotent() {
    let source = r#"function draw()
  local values = {
    alpha = 1,
    beta = 2,
  }
end
"#;

    assert_eq!(format(&format(source)), format(source));
}

#[test]
fn returns_parse_diagnostics_without_formatting_invalid_source() {
    let diagnostics = format_tua_source(
        "src/main.tua",
        "function broken(\n",
        &FormatOptions::default(),
    )
    .expect_err("invalid source should not format");

    assert!(!diagnostics.is_empty());
}

#[test]
fn range_formatting_returns_minimal_line_edits_inside_selection() {
    let source = r#"local before=1
function draw()
local value=call(1,2)
local after=3
end
"#;
    let edits = format_tua_source_range(
        "src/main.tua",
        source,
        &FormatOptions::default(),
        SourceSpan {
            start: SourcePosition { line: 3, column: 1 },
            end: SourcePosition { line: 4, column: 1 },
        },
    )
    .expect("range should format");

    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0].span.start.line, 3);
    assert_eq!(edits[0].span.end.line, 3);
    assert_eq!(edits[0].new_text, "  local value = call(1, ");
}

#[test]
fn range_formatting_can_replace_selected_wrapped_line() {
    let source =
        "local values = { alpha = 1, beta = 2, gamma = 3, delta = 4, epsilon = 5, zeta = 6 }\n";
    let options = FormatOptions {
        line_width: 60,
        ..FormatOptions::default()
    };
    let edits = format_tua_source_range(
        "src/main.tua",
        source,
        &options,
        SourceSpan {
            start: SourcePosition { line: 1, column: 1 },
            end: SourcePosition { line: 2, column: 1 },
        },
    )
    .expect("range should format");

    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0].span.start, SourcePosition { line: 1, column: 1 });
    assert_eq!(edits[0].span.end, SourcePosition { line: 2, column: 1 });
    assert_eq!(
        edits[0].new_text,
        r#"local values = {
  alpha = 1,
  beta = 2,
  gamma = 3,
  delta = 4,
  epsilon = 5,
  zeta = 6,
}
"#
    );
}

#[test]
fn on_type_formatting_rewrites_only_trigger_line() {
    let source = r#"local before=1
function draw()
local value=call(1,2)
local after=3
end
"#;
    let edits = format_tua_source_on_type(
        "src/main.tua",
        source,
        &FormatOptions::default(),
        SourcePosition {
            line: 3,
            column: 22,
        },
        ")",
    )
    .expect("on-type should format");

    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0].span.start.line, 3);
    assert_eq!(edits[0].span.end.line, 3);
    assert_eq!(edits[0].new_text, "  local value = call(1, ");
}

#[test]
fn on_type_formatting_ignores_unstable_triggers() {
    let edits = format_tua_source_on_type(
        "src/main.tua",
        "local value=1\n",
        &FormatOptions::default(),
        SourcePosition { line: 1, column: 6 },
        "x",
    )
    .expect("unknown trigger should be safe");

    assert!(edits.is_empty());
}

#[test]
fn range_and_on_type_formatting_avoid_invalid_sources() {
    let range_error = format_tua_source_range(
        "src/main.tua",
        "function broken(\n",
        &FormatOptions::default(),
        SourceSpan {
            start: SourcePosition { line: 1, column: 1 },
            end: SourcePosition { line: 2, column: 1 },
        },
    )
    .expect_err("invalid source should not range format");
    let on_type_error = format_tua_source_on_type(
        "src/main.tua",
        "function broken(\n",
        &FormatOptions::default(),
        SourcePosition {
            line: 1,
            column: 17,
        },
        "\n",
    )
    .expect_err("invalid source should not on-type format");

    assert!(!range_error.is_empty());
    assert!(!on_type_error.is_empty());
}
