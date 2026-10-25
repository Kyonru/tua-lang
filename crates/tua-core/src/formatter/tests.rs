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
  if hp > 0 then
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
type Players=Player[]
enum Direction
  "North"
  "South"
end
local hp:number=10
local players:Player[]={}
function damage(player:Player, amount:number):number
player.hp-=amount
return player.hp
end"#;

    assert_eq!(
        format(source),
        r#"type Player = {hp:number, name:string}
type Players = Player[]
enum Direction
  "North"
  "South"
end
local hp:number = 10
local players:Player[] = {}
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
  return duration * strength
end
"#
    );
}

#[test]
fn spaces_binary_operators_without_changing_unary_or_lexical_tokens() {
    let source = r#"local arithmetic=a+b*c-d/e%f^g
local comparisons=a==b and c~=d or e<=f and g>=h and i<j and k>m
local unary=-a+-b
local nested=a*-b+p- -v
local joined=left..right
local scientific=1e-3+2e+4
local callback: (number)->string = tostring
local function collect(...)
local values={...}
return values
end
"#;

    assert_eq!(
        format(source),
        r#"local arithmetic = a + b * c - d / e % f ^ g
local comparisons = a == b and c ~= d or e <= f and g >= h and i < j and k > m
local unary = -a + -b
local nested = a * -b + p - -v
local joined = left .. right
local scientific = 1e-3 + 2e+4
local callback: (number) -> string = tostring
local function collect(...)
  local values = {...}
  return values
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

    let expected = r#"local values = {
  alpha = 1,
  beta = 2,
  gamma = 3,
  delta = 4,
  epsilon = 5,
  zeta = 6,
}
"#;
    assert_eq!(output, expected);
    assert_eq!(
        format_tua_source("src/main.tua", &output, &options).expect("output should reformat"),
        output
    );
}

#[test]
fn wraps_table_arguments_without_inserting_a_call_gap() {
    let source = "consume({ alpha = 1, beta = 2, gamma = 3, delta = 4, epsilon = 5 })\n";
    let options = FormatOptions {
        line_width: 45,
        ..FormatOptions::default()
    };

    let output = format_tua_source("src/main.tua", source, &options).expect("source should format");

    assert_eq!(
        output,
        r#"consume({
  alpha = 1,
  beta = 2,
  gamma = 3,
  delta = 4,
  epsilon = 5,
})
"#
    );
    assert_eq!(
        format_tua_source("src/main.tua", &output, &options).expect("output should reformat"),
        output
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
    parse_tua_source("src/main.tua", &output).expect("formatted call should parse");
    assert_eq!(
        format_tua_source("src/main.tua", &output, &options).expect("output should reformat"),
        output
    );
}

#[test]
fn line_width_counts_unicode_characters_instead_of_utf8_bytes() {
    let source = "local label = describe(\"日本語\", first, second)\n";
    let options = FormatOptions {
        line_width: source.trim_end().chars().count(),
        ..FormatOptions::default()
    };

    let output = format_tua_source("src/main.tua", source, &options).expect("source should format");

    assert_eq!(output, source);
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
fn indents_split_control_flow_headers() {
    let source = "if -- split header\r\n-- explain condition\r\nleft or\r\nright\r\nthen\r\nprint(\"if\")\r\nelseif\r\nfallback\r\nthen\r\nprint(\"elseif\")\r\nend\r\n\r\nwhile\r\nrunning\r\ndo\r\nprint(\"while\")\r\nend\r\n\r\nrepeat\r\nprint(\"repeat\")\r\nuntil\r\nfinished or\r\ncancelled\r\n";

    let output = format(source);

    assert_eq!(
        output,
        r#"if -- split header
  -- explain condition
  left or
  right
then
  print("if")
elseif
  fallback
then
  print("elseif")
end

while
  running
do
  print("while")
end

repeat
  print("repeat")
until
  finished or
  cancelled
"#
    );

    let parsed = parse_tua_source("src/main.tua", output).expect("formatted source should parse");
    let environment = crate::types::build_type_environment(&parsed)
        .expect("formatted source should build a type environment");
    let diagnostics = crate::linter::lint_parsed_source(
        &parsed,
        &crate::DiagnosticsConfig::default(),
        &crate::config::JitConfig::default(),
        &environment,
    );

    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code
                != Some(crate::DiagnosticCode::LINT_MULTI_LINE_STATEMENT)),
        "formatter output must satisfy its continuation-layout lint: {diagnostics:#?}"
    );
}

#[test]
fn formats_multiline_conditions_without_losing_block_structure() {
    let source = r#"if(
player.x > 0 and
player.y > 0 and
player.x + player.width < arena.width
) then
print("inside")
end

if player.x > 0
and player.y > 0
and player.x + player.width < arena.width then
print("inside")
end

if mode == "then"
and ready then
print("ready")
end
"#;
    let expected = r#"if (
  player.x > 0 and
  player.y > 0 and
  player.x + player.width < arena.width
) then
  print("inside")
end

if player.x > 0
  and player.y > 0
  and player.x + player.width < arena.width then
  print("inside")
end

if mode == "then"
  and ready then
  print("ready")
end
"#;

    let output = format(source);

    assert_eq!(output, expected);
    assert_eq!(format(&output), output);
    let parsed = parse_tua_source("src/main.tua", output).expect("formatted source should parse");
    let environment = crate::types::build_type_environment(&parsed)
        .expect("formatted source should build a type environment");
    let diagnostics = crate::linter::lint_parsed_source(
        &parsed,
        &crate::DiagnosticsConfig::default(),
        &crate::config::JitConfig::default(),
        &environment,
    );
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code
                != Some(crate::DiagnosticCode::LINT_MULTI_LINE_STATEMENT)),
        "formatter output must satisfy its continuation-layout lint: {diagnostics:#?}"
    );
}

#[test]
fn aligns_multiline_function_signature_closers_with_the_declaration() {
    let source = r#"local function collides(
x: number,
y: number,
width: number,
height: number
): boolean
return x > 0 and y > 0
end
"#;
    let expected = r#"local function collides(
  x: number,
  y: number,
  width: number,
  height: number
): boolean
  return x > 0 and y > 0
end
"#;

    let output = format(source);

    assert_eq!(output, expected);
    assert_eq!(format(&output), output);
    parse_tua_source("src/main.tua", output).expect("formatted signature should parse");
}

#[test]
fn indents_calls_after_multiline_function_signatures() {
    let cases = [
        r#"local function fireProjectile(
  screenX: number,
  screenY: number,
  weapon: any
)
  local velocityX: any = nil
  local velocityY: any = nil
  local lifetime = 10
  if weapon.kind == "ballistic" then
    velocityX, velocityY, lifetime = ballisticLaunch(
      screenX,
      screenY,
      weapon
    )
    lifetime += 0.8
  else
    local length = math.sqrt(
      screenX * screenX + screenY * screenY
    )
    if length > 0.01 then
      lifetime = math.max(
        2.5,
        length + 0.25
      )
    end
  end
end
"#,
        r#"game:watch(
  "score",
  function(
    score: number,
    previous: number
  )
    local value = math.max(
      score,
      previous
    )
    local function nested(
      x: number
    )
      return math.max(
        x,
        value
      )
    end
  end
)
"#,
    ];
    for expected in cases {
        let source = expected
            .lines()
            .map(str::trim)
            .collect::<Vec<_>>()
            .join("\n");
        let output = format(&source);
        assert_eq!(output, expected);
        assert_eq!(format(&output), output);
        let parsed =
            parse_tua_source("src/main.tua", output).expect("formatted source should parse");
        let environment = crate::types::build_type_environment(&parsed)
            .expect("formatted source should build a type environment");
        let diagnostics = crate::linter::lint_parsed_source(
            &parsed,
            &crate::DiagnosticsConfig::default(),
            &crate::config::JitConfig::default(),
            &environment,
        );
        assert!(
            diagnostics.iter().all(|diagnostic| diagnostic.code
                != Some(crate::DiagnosticCode::LINT_MULTI_LINE_STATEMENT)),
            "formatted calls must satisfy TL4037: {diagnostics:#?}"
        );
    }
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
fn inline_function_arguments_do_not_dedent_later_multiline_calls() {
    let source = r#"audio:on("beat", function(event)
  beat = event.beat
end)

function love.draw()
  love.graphics.print(
    "Active voices",
    32,
    118
  )
end
"#;

    assert_eq!(format(source), source);
}

#[test]
fn indents_multiline_calls_nested_in_callback_blocks() {
    let source = r#"local function update(dt: number)
shared.player:update(function(state: any)
if screen_width / 2 > state.pointing.x then
targetX = 0
else
targetX = math.remap(
state.pointing.x,
screen_width / 2,
screen_width,
screen_width * 0.75,
screen_width
)
end
end)
end
"#;
    let expected = r#"local function update(dt: number)
  shared.player:update(function(state: any)
    if screen_width / 2 > state.pointing.x then
      targetX = 0
    else
      targetX = math.remap(
        state.pointing.x,
        screen_width / 2,
        screen_width,
        screen_width * 0.75,
        screen_width
      )
    end
  end)
end
"#;

    let output = format(source);

    assert_eq!(output, expected);
    assert_eq!(format(&output), output);
    let parsed = parse_tua_source("src/main.tua", output).expect("formatted source should parse");
    let environment = crate::types::build_type_environment(&parsed)
        .expect("formatted source should build a type environment");
    let diagnostics = crate::linter::lint_parsed_source(
        &parsed,
        &crate::DiagnosticsConfig::default(),
        &crate::config::JitConfig::default(),
        &environment,
    );
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code
                != Some(crate::DiagnosticCode::LINT_MULTI_LINE_STATEMENT)),
        "formatter output must satisfy its continuation-layout lint: {diagnostics:#?}"
    );
}

#[test]
fn removes_unnecessary_empty_lines() {
    let source = "\n\nlocal first=1\n\n\n\nlocal second=2\n\n\n";

    assert_eq!(
        format(source),
        r#"local first = 1

local second = 2
"#
    );
}

#[test]
fn preserves_empty_lines_inside_long_strings_and_block_comments() {
    let source = concat!(
        "local text = [[\n",
        "first\n",
        "  \n",
        "\n",
        "third\n",
        "]]\n",
        "\n",
        "--[[\n",
        "first\n",
        "\n",
        "\n",
        "third\n",
        "]]\n",
    );

    assert_eq!(format(source), source);
}

#[test]
fn preserves_code_like_text_inside_multiline_literals_and_comments() {
    let source = concat!(
        "local script = [[\n",
        "  local score=10\n",
        "if score>0 then\n",
        "    print( score )\n",
        "]]\n",
        "--[[\n",
        "  local lives=3\n",
        "if lives>0 then\n",
        "]]\n",
        "local escaped = \"first\\\n",
        "  second=unchanged\"\n",
    );

    let formatted = format(source);

    assert_eq!(formatted, source);
    assert_eq!(format(&formatted), formatted);
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
fn preserves_utf8_in_strings_comments_and_long_brackets() {
    let source = concat!(
        "local directions=\"← → move\" -- 日本語のコメント\n",
        "local greeting='Olá, 世界 👋'\n",
        "local description=[[\n",
        "Zażółć gęślą jaźń\n",
        "]]\n",
        "--[[ Ελληνικό σχόλιο ]]\n",
    );
    let expected = concat!(
        "local directions = \"← → move\" -- 日本語のコメント\n",
        "local greeting = 'Olá, 世界 👋'\n",
        "local description = [[\n",
        "Zażółć gęślą jaźń\n",
        "]]\n",
        "--[[ Ελληνικό σχόλιο ]]\n",
    );

    let formatted = format(source);

    assert_eq!(formatted, expected);
    assert_eq!(format(&formatted), formatted);
}

#[test]
fn formats_short_string_quotes_without_changing_values() {
    let source = r#"local plain = 'hello'
local quoted = 'say "hello"'
local apostrophe = 'it\'s ready'
local escaped = '\\ and \"'
local long = [[keep 'both' "styles"]]
-- keep 'comment quotes'
"#;
    let double = FormatOptions {
        quote_style: QuoteStyle::Double,
        ..FormatOptions::default()
    };
    let single = FormatOptions {
        quote_style: QuoteStyle::Single,
        ..FormatOptions::default()
    };

    let double_output =
        format_tua_source("src/main.tua", source, &double).expect("source should format");
    assert_eq!(
        double_output,
        r#"local plain = "hello"
local quoted = "say \"hello\""
local apostrophe = "it's ready"
local escaped = "\\ and \""
local long = [[keep 'both' "styles"]]
-- keep 'comment quotes'
"#
    );

    let single_output = format_tua_source("src/main.tua", double_output, &single)
        .expect("double-quoted source should format");
    assert_eq!(
        single_output,
        r#"local plain = 'hello'
local quoted = 'say "hello"'
local apostrophe = 'it\'s ready'
local escaped = '\\ and "'
local long = [[keep 'both' "styles"]]
-- keep 'comment quotes'
"#
    );
    assert_eq!(
        format_tua_source("src/main.tua", &single_output, &single)
            .expect("formatted source should remain valid"),
        single_output
    );
}

#[test]
fn supports_tab_indentation_for_blocks_and_wrapped_groups() {
    let source = r#"function draw()
local values = { alpha = 1, beta = 2, gamma = 3, delta = 4 }
end
"#;
    let options = FormatOptions {
        indent_style: IndentStyle::Tabs,
        line_width: 45,
        ..FormatOptions::default()
    };

    let output = format_tua_source("src/main.tua", source, &options).expect("source should format");

    assert_eq!(
        output,
        "function draw()\n\tlocal values = {\n\t\talpha = 1,\n\t\tbeta = 2,\n\t\tgamma = 3,\n\t\tdelta = 4,\n\t}\nend\n"
    );
    assert_eq!(
        format_tua_source("src/main.tua", &output, &options).expect("output should reformat"),
        output
    );
}

#[test]
fn supports_lf_crlf_and_source_preserving_line_endings() {
    let source = "function draw()\r\nlocal value=1\r\nend\r\n";
    let crlf = FormatOptions {
        line_ending: LineEnding::Crlf,
        ..FormatOptions::default()
    };
    let preserve = FormatOptions {
        line_ending: LineEnding::Preserve,
        ..FormatOptions::default()
    };

    let crlf_output =
        format_tua_source("src/main.tua", source, &crlf).expect("source should format");
    assert_eq!(
        crlf_output,
        "function draw()\r\n  local value = 1\r\nend\r\n"
    );
    assert_eq!(
        format_tua_source("src/main.tua", source, &preserve).expect("source should format"),
        crlf_output
    );
    assert_eq!(
        format_tua_source("src/main.tua", source, &FormatOptions::default())
            .expect("source should format"),
        "function draw()\n  local value = 1\nend\n"
    );
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

#[test]
fn indents_multiline_expression_lists() {
    let source = r#"local function unproject(x: number, y: number, depth: number)
local values = {
x,
y,
}
local first,
second = x, y
return x + depth,
y + depth,
first + second
end
"#;
    let expected = r#"local function unproject(x: number, y: number, depth: number)
  local values = {
    x,
    y,
  }
  local first,
    second = x, y
  return x + depth,
    y + depth,
    first + second
end
"#;

    let output = format(source);

    assert_eq!(output, expected);
    assert_eq!(format(&output), output);
    let parsed = parse_tua_source("src/main.tua", output).expect("formatted source should parse");
    let environment = crate::types::build_type_environment(&parsed)
        .expect("formatted source should build a type environment");
    let diagnostics = crate::linter::lint_parsed_source(
        &parsed,
        &crate::DiagnosticsConfig::default(),
        &crate::config::JitConfig::default(),
        &environment,
    );
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code
                != Some(crate::DiagnosticCode::LINT_MULTI_LINE_STATEMENT)),
        "formatter output must satisfy its continuation-layout lint: {diagnostics:#?}"
    );
}

#[test]
fn indents_continuations_after_trailing_operators() {
    let source = r#"local function compare(a: string, b: string)
local joined = a ..
b
local same = a ==
b
local different = a ~=
b
local before = a <
b
local after = a >
b
local atMost = a <=
b
local atLeast = a >=
b
local negated = not
same
local commented = a .. -- tail
b
repeat
joined = joined .. "!"
until #joined ==
4
return joined ..
b, same, different, before, after, atMost, atLeast, negated, commented
end

local function count(...)
return ...
end
"#;
    let expected = r#"local function compare(a: string, b: string)
  local joined = a ..
    b
  local same = a ==
    b
  local different = a ~=
    b
  local before = a <
    b
  local after = a >
    b
  local atMost = a <=
    b
  local atLeast = a >=
    b
  local negated = not
    same
  local commented = a .. -- tail
    b
  repeat
    joined = joined .. "!"
  until #joined ==
    4
  return joined ..
    b, same, different, before, after, atMost, atLeast, negated, commented
end

local function count(...)
  return ...
end
"#;

    let output = format(source);

    assert_eq!(output, expected);
    assert_eq!(format(&output), output);
    assert_no_multiline_layout_lint(&output);
}

fn assert_no_multiline_layout_lint(output: &str) {
    let parsed =
        parse_tua_source("src/main.tua", output.to_owned()).expect("formatted source should parse");
    let environment = crate::types::build_type_environment(&parsed)
        .expect("formatted source should build a type environment");
    let diagnostics = crate::linter::lint_parsed_source(
        &parsed,
        &crate::DiagnosticsConfig::default(),
        &crate::config::JitConfig::default(),
        &environment,
    );
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code
                != Some(crate::DiagnosticCode::LINT_MULTI_LINE_STATEMENT)),
        "formatter output must satisfy its continuation-layout lint: {diagnostics:#?}"
    );
}

#[test]
fn indents_continuations_that_start_with_operators() {
    let source = r#"type Mode =
| "idle"
| "run"

local function measure(a: number, b: number, items: any)
local sum = a
+ b
- 1
local scaled = a
* b
/ 2
% 3
^ 2
local valid = a > 0
and b > 0
or false
local same = a
== b
local different = a
~= b
local before = a
< b
local after = a
>= b
local total
= sum + scaled
local count = items
:filter()
:count()
local first = items
.first
local joined = tostring(a)
.. tostring(b)
return sum, scaled, valid, same, different, before, after, total, count, first, joined
end
"#;
    let expected = r#"type Mode =
  | "idle"
  | "run"

local function measure(a: number, b: number, items: any)
  local sum = a
    + b
    - 1
  local scaled = a
    * b
    / 2
    % 3
    ^ 2
  local valid = a > 0
    and b > 0
    or false
  local same = a
    == b
  local different = a
    ~= b
  local before = a
    < b
  local after = a
    >= b
  local total
    = sum + scaled
  local count = items
    :filter()
    :count()
  local first = items
    .first
  local joined = tostring(a)
    .. tostring(b)
  return sum, scaled, valid, same, different, before, after, total, count, first, joined
end
"#;

    let output = format(source);

    assert_eq!(output, expected);
    assert_eq!(format(&output), output);
    assert_no_multiline_layout_lint(&output);
}

#[test]
fn indents_bodies_of_split_for_headers() {
    let source = r#"local function visit(items: any, last: number)
for index = 1,
last do
print(index)
end
for key, value in pairs(items),
nil do
print(key, value)
end
for index = 1, last
do
print(index)
end
end
"#;
    let expected = r#"local function visit(items: any, last: number)
  for index = 1,
    last do
    print(index)
  end
  for key, value in pairs(items),
    nil do
    print(key, value)
  end
  for index = 1, last
  do
    print(index)
  end
end
"#;

    let output = format(source);

    assert_eq!(output, expected);
    assert_eq!(format(&output), output);
    assert_no_multiline_layout_lint(&output);
}

#[test]
fn keeps_the_space_before_bracketed_table_keys() {
    let source = r#"local names = { ["end"] = 1,[2] = "two" }
local nested = { [1] = { ["a b"] = true } }
consume({ [1] = 2 })
local value = names ["end"] .. lookup [ 1 ]
"#;
    let expected = r#"local names = { ["end"] = 1, [2] = "two" }
local nested = { [1] = { ["a b"] = true } }
consume({ [1] = 2 })
local value = names["end"] .. lookup[1]
"#;

    let output = format(source);

    assert_eq!(output, expected);
    assert_eq!(format(&output), output);
}

#[test]
fn reads_code_after_inline_block_comments() {
    let source = r#"local function make()
local x = --[[ default ]] 1
local y = 2 --[[ trailing ]]
local z = --[==[ ]] still a comment ]==] 3
return x + y + z
end
local w = --[[ opens
]] 4
"#;
    let expected = r#"local function make()
  local x = --[[ default ]] 1
  local y = 2 --[[ trailing ]]
  local z = --[==[ ]] still a comment ]==] 3
  return x + y + z
end
local w = --[[ opens
]] 4
"#;

    let output = format(source);

    assert_eq!(output, expected);
    assert_eq!(format(&output), output);
    assert_no_multiline_layout_lint(&output);
}

#[test]
fn keeps_the_space_after_wrapped_groups() {
    let source = r#"local total = compute(alpha_value, beta_value, gamma_value) + offset
report(alpha_value, beta_value, gamma_value) -- note
local config = { alpha = 1, beta = 2, gamma = 3, delta = 4 } -- defaults
"#;
    let expected = r#"local total = compute(
  alpha_value,
  beta_value,
  gamma_value
) + offset
report(
  alpha_value,
  beta_value,
  gamma_value
) -- note
local config = {
  alpha = 1,
  beta = 2,
  gamma = 3,
  delta = 4,
} -- defaults
"#;
    let options = FormatOptions {
        line_width: 40,
        ..FormatOptions::default()
    };

    let output = format_tua_source("src/main.tua", source, &options).expect("source should format");

    assert_eq!(output, expected);
    assert_eq!(
        format_tua_source("src/main.tua", &output, &options).expect("output should reformat"),
        output
    );
    assert_no_multiline_layout_lint(&output);
}

#[test]
fn wraps_the_outermost_group_first() {
    let source = r#"each({ 1, 2, 3 }, function(value) print(value) end)
consume({ alpha = 1, beta = 2, gamma = 3, delta = 4 })
local rows = { lookup(alpha, beta), lookup(gamma, delta) }
"#;
    let expected = r#"each(
  { 1, 2, 3 },
  function(value) print(value) end
)
consume({
  alpha = 1,
  beta = 2,
  gamma = 3,
  delta = 4,
})
local rows = {
  lookup(alpha, beta),
  lookup(gamma, delta),
}
"#;
    let options = FormatOptions {
        line_width: 40,
        ..FormatOptions::default()
    };

    let output = format_tua_source("src/main.tua", source, &options).expect("source should format");

    assert_eq!(output, expected);
    assert_eq!(
        format_tua_source("src/main.tua", &output, &options).expect("output should reformat"),
        output
    );
    assert_no_multiline_layout_lint(&output);
}

#[test]
fn does_not_wrap_inside_one_line_blocks() {
    let source = r#"local handler = function(first_param, second_param) return first_param end
local obj = { run = function(self, first, second) return first end }
if ready then start(first_param, second_param, third_param) end
run(function(first, second) return first end, first_param, second_param)
local after = 1
"#;
    let expected = r#"local handler = function(first_param, second_param) return first_param end
local obj = { run = function(self, first, second) return first end }
if ready then start(first_param, second_param, third_param) end
run(
  function(first, second) return first end,
  first_param,
  second_param
)
local after = 1
"#;
    let options = FormatOptions {
        line_width: 50,
        ..FormatOptions::default()
    };

    let output = format_tua_source("src/main.tua", source, &options).expect("source should format");

    assert_eq!(output, expected);
    assert_eq!(
        format_tua_source("src/main.tua", &output, &options).expect("output should reformat"),
        output
    );
    assert_no_multiline_layout_lint(&output);
}

#[test]
fn indents_multiline_calls_inside_tables() {
    let source = r#"local rows = {
lookup(
1,
2
),
handler = function(value)
return lookup(
value,
3
)
end,
}
consume(function() end, {
alpha = 1,
})
"#;
    let expected = r#"local rows = {
  lookup(
    1,
    2
  ),
  handler = function(value)
    return lookup(
      value,
      3
    )
  end,
}
consume(function() end, {
  alpha = 1,
})
"#;

    let output = format(source);

    assert_eq!(output, expected);
    assert_eq!(format(&output), output);
    assert_no_multiline_layout_lint(&output);
}

#[test]
fn wraps_nested_groups_in_one_pass() {
    let source = r#"print(string.format("%s %s %s", first_value, second_value), third_value)
local rows = { lookup(alpha_value, beta_value, gamma_value), other }
consume(first_argument, { alpha = 1, beta = 2, gamma = 3, delta = 4 })
"#;
    let expected = r#"print(
  string.format(
    "%s %s %s",
    first_value,
    second_value
  ),
  third_value
)
local rows = {
  lookup(
    alpha_value,
    beta_value,
    gamma_value
  ),
  other,
}
consume(
  first_argument,
  {
    alpha = 1,
    beta = 2,
    gamma = 3,
    delta = 4,
  }
)
"#;
    let options = FormatOptions {
        line_width: 40,
        ..FormatOptions::default()
    };

    let output = format_tua_source("src/main.tua", source, &options).expect("source should format");

    assert_eq!(output, expected);
    assert_eq!(
        format_tua_source("src/main.tua", &output, &options).expect("output should reformat"),
        output
    );
    assert_no_multiline_layout_lint(&output);
}

#[test]
fn drops_a_leading_byte_order_mark() {
    assert_eq!(
        format("\u{feff}local x=1\nprint(x)\n"),
        "local x = 1\nprint(x)\n"
    );
}

#[test]
fn compacts_unary_minus_and_length_operators() {
    let source = r#"local n = - 1
local m = - n * 2
local negated = - - n
local size = # items + count(- n)
local small = 1e-5 - - 2
return - n, # items
"#;
    let expected = r#"local n = -1
local m = -n * 2
local negated = - -n
local size = #items + count(-n)
local small = 1e-5 - -2
return -n, #items
"#;

    let output = format(source);

    assert_eq!(output, expected);
    assert_eq!(format(&output), output);
}
