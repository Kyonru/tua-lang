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
        "type Player = { hp: number, name: string }\ntype Players = Player[]\nenum Direction\n  \"North\"\n  \"South\"\nend\nlocal hp: number = 10\nlocal players: Player[] = {}\nfunction damage(player: Player, amount: number): number\n  player.hp -= amount\n  return player.hp\nend\n"
    );
}

#[test]
fn formats_generic_class_headers_and_applied_parent_without_erasure() {
    let source =
        "open class Base<T> {\n  value: T\n}\nfinal class Child<U> extends Base<Box<U[]>> {\n}\n";
    let output = format(source);

    assert!(output.contains("open class Base<T> {"), "{output}");
    assert!(
        output.contains("final class Child<U> extends Base<Box<U[]>> {"),
        "{output}"
    );
    assert_eq!(format(&output), output);
}

#[test]
fn generic_instance_alias_does_not_indent_the_following_top_level_statement() {
    let source =
        "final class Box<T> {\n}\nexport type BoxInstance<T> = Box<T>\nreturn { Box = Box }\n";
    let output = format(source);

    assert!(output.contains("\nreturn { Box = Box }\n"), "{output}");
    assert_eq!(format(&output), output);
    crate::check_tua_source("src/main.tua", output)
        .expect("formatted generic instance aliases should remain valid");
}

#[test]
fn formats_generic_function_headers_without_changing_their_syntax() {
    let source = r#"protocol Updatable { update(dt: number): nil }
local function _updateOne<T: Updatable>(item: T, dt: number): T
  item:update(dt)
  return item
end
"#;

    let output = format(source);
    assert!(
        output.contains("local function _updateOne<T: Updatable>(item: T, dt: number): T"),
        "{output}"
    );
    assert_eq!(format(&output), output);
    crate::check_tua_source("src/main.tua", output)
        .expect("formatted generic function headers should remain semantically valid");
}

#[test]
fn formats_protocol_comments_without_changing_method_signatures() {
    let source = r#"protocol Updatable {
update(dt: number): nil -- advance one simulation step
}
final class Actor implements Updatable {
update(dt:number):nil {}
}
local _actor:Updatable=Actor.new()
"#;
    let output = format(source);

    assert!(output.contains("update(dt: number): nil -- advance one simulation step"));
    assert_eq!(format(&output), output);
    crate::check_tua_source("src/main.tua", output)
        .expect("formatted protocol signatures should remain semantically valid");
}

#[test]
fn formats_multiline_protocol_signatures_idempotently_and_preserves_comments() {
    let source = r#"protocol Applies {
apply(
input:number, -- measurement
mode:string
):nil -- finished
}
final class Worker implements Applies {
apply(input:number,mode:string):nil {}
}
"#;
    let output = format(source);

    assert!(output.contains("-- measurement"), "{output}");
    assert!(output.contains("-- finished"), "{output}");
    assert_eq!(format(&output), output);
    crate::check_tua_source("src/main.tua", output)
        .expect("formatted multiline signatures should still satisfy protocol checking");
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
        "local function makePlayer(name: string): Player\n  return { name = name, hp = 100 }\nend\nlocal inline = function()\n  return 1\nend\nlocal player = makePlayer(\"Ada\")\n"
    );
}

#[test]
fn formats_default_parameters() {
    let source = r#"function shake(duration=0.3, strength:number=1)
return duration*strength
end"#;

    assert_eq!(
        format(source),
        "function shake(duration = 0.3, strength: number = 1)\n  return duration * strength\nend\n"
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
  local values = { ... }
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
        "function shake(duration: number, easing?: string)\n  return duration\nend\n"
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
fn line_width_preserves_unicode_values_when_wrapping() {
    let source = "local label = describe(\"日本語\", first, second)\n";
    let options = FormatOptions {
        line_width: source.trim_end().chars().count(),
        ..FormatOptions::default()
    };

    let output = format_tua_source("src/main.tua", source, &options).expect("source should format");

    assert_eq!(
        output,
        "local label =\n  describe(\"日本語\", first, second)\n"
    );
    corpus::same_program(source, &output).unwrap();
}

#[test]
fn malformed_non_ascii_code_tokens_are_formatter_idempotent() {
    let source = "#!===�";
    let once = format(source);
    assert_eq!(format(&once), once);
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
        "function draw()\n  love.graphics.draw(sprite, 8 + (i - 1), 40 / sprite:getWidth())\nend\n"
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
        "function update()\n  local hit = utils.AABB_collision(\n    player.x,\n    player.y,\n    player.width,\n    player.height,\n    wall.x,\n    wall.y,\n    wall.width,\n    wall.height\n  )\nend\n"
    );
    parse_tua_source("src/main.tua", output).expect("formatted call should parse");
}

#[test]
fn indents_leading_concat_continuation() {
    let source = r#"local label =
"alpha"
.. "beta""#;

    assert_eq!(format(source), "local label = \"alpha\" .. \"beta\"\n");
}

#[test]
fn indents_split_control_flow_headers() {
    let source = "if -- split header\r\n-- explain condition\r\nleft or\r\nright\r\nthen\r\nprint(\"if\")\r\nelseif\r\nfallback\r\nthen\r\nprint(\"elseif\")\r\nend\r\n\r\nwhile\r\nrunning\r\ndo\r\nprint(\"while\")\r\nend\r\n\r\nrepeat\r\nprint(\"repeat\")\r\nuntil\r\nfinished or\r\ncancelled\r\n";

    let output = format(source);

    assert_eq!(
        output,
        "if -- split header\n  -- explain condition\n  left or right\nthen\n  print(\"if\")\nelseif fallback then\n  print(\"elseif\")\nend\n\nwhile running do\n  print(\"while\")\nend\n\nrepeat\n  print(\"repeat\")\nuntil finished or cancelled\n"
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
    let expected = "if player.x > 0 and player.y > 0 and player.x + player.width < arena.width then\n  print(\"inside\")\nend\n\nif player.x > 0 and player.y > 0 and player.x + player.width < arena.width then\n  print(\"inside\")\nend\n\nif mode == \"then\" and ready then\n  print(\"ready\")\nend\n";

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
    let expected = "local function collides(x: number, y: number, width: number, height: number): boolean\n  return x > 0 and y > 0\nend\n";

    let output = format(source);

    assert_eq!(output, expected);
    assert_eq!(format(&output), output);
    parse_tua_source("src/main.tua", output).expect("formatted signature should parse");
}

#[test]
fn indents_calls_after_multiline_function_signatures() {
    let cases = [
        "local function fireProjectile(screenX: number, screenY: number, weapon: any)\n  local velocityX: any = nil\n  local velocityY: any = nil\n  local lifetime = 10\n  if weapon.kind == \"ballistic\" then\n    velocityX, velocityY, lifetime = ballisticLaunch(screenX, screenY, weapon)\n    lifetime += 0.8\n  else\n    local length = math.sqrt(screenX * screenX + screenY * screenY)\n    if length > 0.01 then\n      lifetime = math.max(2.5, length + 0.25)\n    end\n  end\nend\n",
        "game:watch(\"score\", function(score: number, previous: number)\n  local value = math.max(score, previous)\n  local function nested(x: number)\n    return math.max(x, value)\n  end\nend)\n",
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

    assert_eq!(format(source), "local stopScore = game:watch(\"score\", function(score: number, previous: number, state: GameState)\n  print(\"score\", previous, score, state.playerName)\nend)\n");
    corpus::same_program(source, &format(source)).unwrap();
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

    assert_eq!(format(source), "audio:on(\"beat\", function(event)\n  beat = event.beat\nend)\n\nfunction love.draw()\n  love.graphics.print(\"Active voices\", 32, 118)\nend\n");
    corpus::same_program(source, &format(source)).unwrap();
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
        "local greeting = \"Olá, 世界 👋\"\n",
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
    let expected = "local function unproject(x: number, y: number, depth: number)\n  local values = {\n    x,\n    y,\n  }\n  local first, second = x, y\n  return x + depth, y + depth, first + second\nend\n";

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
    let expected = "local function compare(a: string, b: string)\n  local joined = a .. b\n  local same = a == b\n  local different = a ~= b\n  local before = a < b\n  local after = a > b\n  local atMost = a <= b\n  local atLeast = a >= b\n  local negated = not same\n  local commented = a -- tail\n    .. b\n  repeat\n    joined = joined .. \"!\"\n  until #joined == 4\n  return joined .. b, same, different, before, after, atMost, atLeast, negated, commented\nend\n\nlocal function count(...)\n  return ...\nend\n";

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
    let expected = "type Mode = | \"idle\" | \"run\"\n\nlocal function measure(a: number, b: number, items: any)\n  local sum = a + b - 1\n  local scaled = a * b / 2 % 3 ^ 2\n  local valid = a > 0 and b > 0 or false\n  local same = a == b\n  local different = a ~= b\n  local before = a < b\n  local after = a >= b\n  local total = sum + scaled\n  local count = items:filter():count()\n  local first = items.first\n  local joined = tostring(a) .. tostring(b)\n  return sum, scaled, valid, same, different, before, after, total, count, first, joined\nend\n";

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
    let expected = "local function visit(items: any, last: number)\n  for index = 1, last do\n    print(index)\n  end\n  for key, value in pairs(items), nil do\n    print(key, value)\n  end\n  for index = 1, last do\n    print(index)\n  end\nend\n";

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
    let expected = "local function make()\n  local x = --[[ default ]]\n    1\n  local y = 2 --[[ trailing ]]\n  local z = --[==[ ]] still a comment ]==]\n    3\n  return x + y + z\nend\nlocal w = --[[ opens\n]]\n  4\n";

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
    let expected = r#"each({ 1, 2, 3 }, function(value)
  print(value)
end)
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
fn expands_long_inline_blocks_before_wrapping_groups() {
    let source = r#"local handler = function(first_param, second_param) return first_param end
local obj = { run = function(self, first, second) return first end }
if ready then start(first_param, second_param, third_param) end
run(function(first, second) return first end, first_param, second_param)
local after = 1
"#;
    let expected = r#"local handler = function(
  first_param,
  second_param
)
  return first_param
end
local obj = {
  run = function(self, first, second)
    return first
  end,
}
if ready then
  start(first_param, second_param, third_param)
end
run(function(first, second)
  return first
end, first_param, second_param)
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
    let expected = "local rows = {\n  lookup(1, 2),\n  handler = function(value)\n    return lookup(value, 3)\n  end,\n}\nconsume(function() end, {\n  alpha = 1,\n})\n";

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
    let expected = "print(\n  string.format(\n    \"%s %s %s\",\n    first_value,\n    second_value\n  ),\n  third_value\n)\nlocal rows = {\n  lookup(\n    alpha_value,\n    beta_value,\n    gamma_value\n  ),\n  other,\n}\nconsume(first_argument, {\n  alpha = 1,\n  beta = 2,\n  gamma = 3,\n  delta = 4,\n})\n";
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
    let expected = "local n = -1\nlocal m = -n * 2\nlocal negated = -(-n)\nlocal size = #items + count(-n)\nlocal small = 1e-5 - -2\nreturn -n, #items\n";

    let output = format(source);

    assert_eq!(output, expected);
    assert_eq!(format(&output), output);
}

#[test]
fn separates_trailing_comments_from_code() {
    let source = r#"print(1)-- touching
local x = 1--[[ block ]]
local y = (x)--note
-- own line
"#;
    let expected =
        "print(1) -- touching\nlocal x = 1--[[ block ]]\nlocal y = x --note\n-- own line\n";

    let output = format(source);

    assert_eq!(output, expected);
    assert_eq!(format(&output), output);
}

#[test]
fn pads_one_line_tables_and_keeps_empty_tables_tight() {
    let source = r#"local list = {1, 2,3}
local empty = { }
local nested = {{x=1},{ y = 2 }}
local keyed = {["a b"]=1}
local args = {...}
local built = f{1}
local open = {
  1,
}
type Point = {x: number, y: number}
"#;
    let expected = "local list = { 1, 2, 3 }\nlocal empty = {}\nlocal nested = { { x = 1 }, { y = 2 } }\nlocal keyed = { [\"a b\"] = 1 }\nlocal args = { ... }\nlocal built = f { 1 }\nlocal open = {\n  1,\n}\ntype Point = { x: number, y: number }\n";

    let output = format(source);

    assert_eq!(output, expected);
    assert_eq!(format(&output), output);
}

#[test]
fn indents_parentheses_opened_on_one_line_once() {
    let source = r#"print(lookup(
target,
1
))
consume(first, wrap(
second
), third)
local value = outer(inner(
deep(
1
)
))
"#;
    let expected = "print(lookup(target, 1))\nconsume(first, wrap(second), third)\nlocal value = outer(inner(deep(1)))\n";

    let output = format(source);

    assert_eq!(output, expected);
    assert_eq!(format(&output), output);
    assert_no_multiline_layout_lint(&output);
}

#[test]
fn closes_call_and_table_indentation_after_multiline_literal_suffixes() {
    for source in [
        "local shader = love.graphics.newShader([[\nvec4 effect() { return vec4(1); }\n]])\nprint(shader)\n",
        "local data = {\n  text = [[\nkeep this unindented\n]]}\nprint(data)\n",
        "if ready then\n  print([=[\nif then function ( {\n]=])\n  print(1)\nend\nprint(2)\n",
        "print(\"first\\\nsecond\")\nprint(2)\n",
        "if ready then\n  --[[\ncomment\n]] end\nprint(2)\n",
    ] {
        let output = format(source);
        assert_eq!(output, source.replace("]]}", "]],\n}").replace("]] end", "]]\nend"));
        assert_eq!(format(&output), output);
    }
}

#[test]
fn closes_table_argument_indentation_when_call_ends_mid_line() {
    let source = "local low = AudioDesign.render({ waveform = \"sine\", frequency = 220, duration = 0.3,\n  gain = 0.25, envelope = { attack = 0.005, decay = 0.2, sustain = 0, release = 0.05 } })\nlocal high = AudioDesign.render({ waveform = \"sine\", frequency = 660, duration = 0.2,\n  gain = 0.25, envelope = { attack = 0.005, decay = 0.13, sustain = 0, release = 0.04 } })\nlocal nextValue = 3\n";

    let output = format(source);
    let expected = "local low = AudioDesign.render({\n  waveform = \"sine\",\n  frequency = 220,\n  duration = 0.3,\n  gain = 0.25,\n  envelope = { attack = 0.005, decay = 0.2, sustain = 0, release = 0.05 },\n})\nlocal high = AudioDesign.render({\n  waveform = \"sine\",\n  frequency = 660,\n  duration = 0.2,\n  gain = 0.25,\n  envelope = { attack = 0.005, decay = 0.13, sustain = 0, release = 0.04 },\n})\nlocal nextValue = 3\n";
    assert_eq!(output, expected);
    assert_eq!(format(&output), output);
}

#[test]
fn inline_middle_branches_close_their_own_conditional_indentation() {
    for branch in [
        "elseif side == 2 then x = 2 end",
        "else x = 2 end",
        "elseif side == 2 then if ready then x = 2 end end",
        "else local callback = function() return 2 end; x = callback() end",
    ] {
        let expected = format!("local function choose(side: number): number\n  local x = 0\n  if side == 1 then x = 1\n  {branch}\n  return x\nend\nprint(choose(2))\n");
        let source = expected
            .lines()
            .map(str::trim_start)
            .collect::<Vec<_>>()
            .join("\n");
        let actual = format(&source);
        assert!(
            actual.contains("\n  return x\nend\nprint(choose(2))\n"),
            "{actual}"
        );
        corpus::same_program(&source, &actual).unwrap();
        assert_eq!(format(&actual), actual, "idempotence: {branch}");
    }
}

#[test]
fn inline_nested_end_does_not_close_its_middle_branch() {
    let expected = "if ready then\n  print(1)\nelseif fallback then\n  local callback = function()\n    return 2\n  end\n  print(callback())\nelse\n  if other then\n    print(3)\n  end\n  print(4)\nend\nprint(5)\n";
    let source = expected
        .lines()
        .map(str::trim_start)
        .collect::<Vec<_>>()
        .join("\n");
    let actual = format(&source);
    assert_eq!(actual, expected);
    assert_eq!(format(&actual), actual);
}

#[test]
fn inline_middle_branch_closes_inside_multiline_callback() {
    let expected = "consume(function()\n  if ready then\n    print(1)\n  else\n    print(2)\n  end\n  print(3)\nend)\nprint(4)\n";
    let source = expected
        .lines()
        .map(str::trim_start)
        .collect::<Vec<_>>()
        .join("\n");
    let actual = format(&source);
    assert_eq!(actual, expected);
    assert_eq!(format(&actual), actual);
}

#[test]
fn preserves_literal_whitespace_on_multiline_opening_lines() {
    for ending in ["\n", "\r\n"] {
        for opener in ["[[a  ", "[=[a\t "] {
            let closer = if opener.starts_with("[=[") {
                "]=]"
            } else {
                "]]"
            };
            let source =
                format!("local text = {opener}{ending}b{closer}{ending}print(#text){ending}");
            let options = FormatOptions {
                line_ending: LineEnding::Preserve,
                ..FormatOptions::default()
            };
            let formatted = format_tua_source("src/main.tua", &source, &options).unwrap();
            assert_eq!(formatted, source);
            assert_eq!(
                format_tua_source("src/main.tua", &formatted, &options).unwrap(),
                source
            );
        }
    }
}

#[test]
fn final_parameter_closes_header_state_before_later_wrapped_calls() {
    let source = r#"local function empty(first: number,
  second: number)
end
local function render()
  for _, row in ipairs(rows) do
    for _, cell in ipairs(row.cells) do
      output = output .. string.format("%s %s %s", string.sub(cell.description, 1, 80), cell.label, cell.value)
    end
  end
end
"#;
    let output = format(source);
    assert!(
        output.contains("      output = output\n        .. string.format(\"%s %s %s\", string.sub(cell.description, 1, 80), cell.label, cell.value)\n    end"),
        "{output}"
    );
    assert_eq!(format(&output), output);
}

#[test]
fn expands_crowded_statements_and_their_inline_block() {
    let source = "local a=1; local b=2\nif ready then a=3; b=4; return a+b end\n";
    let expected =
        "local a = 1\nlocal b = 2\nif ready then\n  a = 3\n  b = 4\n  return a + b\nend\n";
    assert_eq!(format(source), expected);
    assert_eq!(format(expected), expected);
}

#[test]
fn statement_layout_preserves_literals_and_ambiguous_call_separators() {
    let source = "local text = [=[a;b]=]; local quote = \";\"\ncall(); (factory())()\n";
    let output = format(source);
    assert!(output.contains("[=[a;b]=]\nlocal quote"), "{output}");
    assert!(output.contains("call();\n(factory())()"), "{output}");
    assert_eq!(format(&output), output);
}

#[test]
fn multiline_tables_have_one_field_per_line_and_keep_small_children_compact() {
    let source = "local config = { first = 1, second = 2,\n  pair = { 3, 4 } }\n";
    let expected = "local config = { first = 1, second = 2, pair = { 3, 4 } }\n";
    assert_eq!(format(source), expected);
    assert_eq!(format(expected), expected);
}

#[test]
fn expanding_callback_statements_expands_their_enclosing_table_in_one_pass() {
    let source = "local callbacks = { run = function() a(); b() end, count = 2 }\n";
    let output = format(source);
    assert!(output.contains("local callbacks = {\n"), "{output}");
    assert!(
        output.contains("function()\n    a()\n    b()\n  end,"),
        "{output}"
    );
    assert_eq!(format(&output), output);
}

#[test]
fn long_inline_branch_calls_expand_before_argument_wrapping() {
    let source = "if kind == 1 then first()\nelseif kind == 2 then world:add(entity, Component, { first = 1, second = 2, third = 3, fourth_parameter = 4 })\nend\n";
    let output = format(source);
    assert!(
        output.contains("elseif kind == 2 then\n  world:add"),
        "{output}"
    );
    assert_eq!(format(&output), output);
}

#[test]
fn table_in_for_header_uses_one_continuation_indent() {
    let source =
        "for _, value in ipairs({\n  \"first\",\n  \"second\",\n}) do\n  consume(value)\nend\n";
    assert_eq!(format(source), source);
    assert_eq!(format(&format(source)), source);
}

#[test]
fn inline_expansion_measures_canonical_spacing() {
    let source = "if ready then world:add(entity,Component,{first=1,second=2,third=3,fourth=4,fifth=5}) end\n";
    let output = format(source);
    assert!(output.starts_with("if ready then\n"), "{output}");
    assert_eq!(format(&output), output);
}

#[test]
fn compact_middle_branch_does_not_wrap_under_added_block_indentation() {
    let source = "local handler = function() if first then run()\nelseif second then world:add(entity, Component, { size = 1, image = 1, age = 2 })\nend end\n";
    let options = FormatOptions {
        line_width: 80,
        ..FormatOptions::default()
    };
    let output = format_tua_source("src/main.tua", source, &options).unwrap();
    assert_eq!(
        format_tua_source("src/main.tua", &output, &options).unwrap(),
        output
    );
}

#[test]
fn continued_conditions_inside_callbacks_do_not_promote_closed_parentheses() {
    let source = "visit(function(id, depth)\n  if depth > 0 and (best == 0 or depth < best\n    or (depth == best and id < 7)) then\n    best = depth\n  end\nend)\n";
    corpus::same_program(source, &format(source)).unwrap();
    assert_eq!(format(&format(source)), format(source));
    let nested = "visit(function()\n  while ready and (first\n    or second) do\n    visit(function()\n      if first and (second\n        or third) then\n        act()\n      end\n    end)\n  end\nend)\n";
    corpus::same_program(nested, &format(nested)).unwrap();
    assert_eq!(format(&format(nested)), format(nested));
}

#[test]
fn long_compound_conditions_wrap_before_short_calls_in_one_pass() {
    for header in ["if", "elseif", "while", "until"] {
        let condition = "ready and not hasHit(projectile, entity) and habitat.canHit(entity) and habitat.isFlying(entity) == flags[index]";
        let source = match header {
            "elseif" => format!("if first then\n  act()\nelseif {condition} then\n  act()\nend\n"),
            "while" => format!("while {condition} do\n  act()\nend\n"),
            "until" => format!("repeat\n  act()\nuntil {condition}\n"),
            _ => format!("visit(function()\n  if {condition}\n    and lookup(entity) == position then\n    act()\n  end\nend)\n"),
        };
        let output = format(&source);
        assert!(
            output.contains("not hasHit(projectile, entity)"),
            "{output}"
        );
        assert_eq!(format(&output), output, "{header}");
        assert_eq!(lua_tokens(&source), lua_tokens(&output));
    }
}

fn lua_tokens(source: &str) -> Vec<String> {
    full_moon::tokenizer::Lexer::new(source, full_moon::LuaVersion::lua51())
        .collect()
        .unwrap()
        .into_iter()
        .filter(|token| {
            !matches!(
                token.token_type(),
                full_moon::tokenizer::TokenType::Whitespace { .. }
                    | full_moon::tokenizer::TokenType::SingleLineComment { .. }
                    | full_moon::tokenizer::TokenType::MultiLineComment { .. }
            )
        })
        .map(|token| token.to_string())
        .collect()
}

#[test]
fn condition_wrapping_preserves_grouping_strings_comments_and_continuations() {
    let source = "if predicate(\"and or\", [=[or]=]) and (left or right) and extremelyLongConditionNameThatExceedsThePreferredWidth then -- and keep\n  act()\nend\n";
    let output = format(source);
    assert!(output.contains("(left or right)"));
    assert!(output.contains("-- and keep"));
    assert_eq!(lua_tokens(source), lua_tokens(&output));
    assert_eq!(format(&output), output);
}

#[test]
fn indented_inline_condition_expands_body_before_wrapping() {
    let source = "local function select()\n  for _, entry in ipairs(entries) do\n    if entry.category == \"candidate\" and isReady and availableCount > 0 then return entry end\n  end\nend\n";
    let options = FormatOptions {
        line_width: 90,
        ..FormatOptions::default()
    };
    let output = format_tua_source("src/main.tua", source, &options).unwrap();
    assert!(output.contains("then\n      return entry\n"), "{output}");
    assert_eq!(
        format_tua_source("src/main.tua", &output, &options).unwrap(),
        output
    );
    assert_eq!(lua_tokens(source), lua_tokens(&output));
}

#[test]
fn split_compact_guards_do_not_indent_following_statements() {
    for operator in ["or", "and"] {
        for comment in ["", " -- explain"] {
            let source = format!("local function allowed(a: boolean, b: boolean): boolean\n  if not a{comment}\n    {operator} not b then return false end\n  return true\nend\n\nlocal after = 1\n");
            corpus::same_program(&source, &format(&source)).unwrap();
            assert_eq!(format(&format(&source)), format(&source));
        }
    }
    for source in [
        "while ready\n  and active do break end\nlocal after = 1\n",
        "for i = 1,\n  3 do act(i) end\nlocal after = 1\n",
        "for _, value in\n  ipairs(values) do act(value) end\nlocal after = 1\n",
        "visit(function()\n  if ready\n    and active then act() end\n  finish()\nend)\nlocal after = 1\n",
        "if ready then\n  act()\nelseif active\n  and fallback then act() end\nlocal after = 1\n",
        "if ready\n  and active then visit(function() end)\n  finish()\nend\nlocal after = 1\n",
    ] {
        corpus::same_program(source, &format(source)).unwrap();
        assert_eq!(format(&format(source)), format(source));
        assert_eq!(lua_tokens(source), lua_tokens(&format(source)));
    }
}

#[test]
fn expanded_blocks_include_closing_index_tokens() {
    for expression in [
        "values[i]",
        "values[keys[i]]",
        "values[i].field",
        "values[i](key)",
    ] {
        let source = format!("function roll()\n  for i = 1, count do prepare(); total = total + {expression} end\n  return total\nend\nlocal after = true\n");
        let expected = format!("function roll()\n  for i = 1, count do\n    prepare()\n    total = total + {expression}\n  end\n  return total\nend\nlocal after = true\n");
        let output = format(&source);
        assert_eq!(output, expected);
        assert_eq!(format(&output), output);
    }
}

#[test]
fn procedural_runtime_layout_settles_in_one_pass() {
    let source = include_str!("../../../../vendor/runtime/tuarot.lua");
    let output = format(source);
    assert_eq!(format(&output), output);
    assert!(output.contains("\nRot.Dice = Dice\n"));
    assert!(output.contains("\nRot.StringGenerator = StringGenerator\n"));
    // Layout removes optional semicolons and inserts trailing table commas.
    let significant = |text: &str| {
        lua_tokens(text)
            .into_iter()
            .filter(|token| token != ";" && token != ",")
            .collect::<Vec<_>>()
    };
    assert_eq!(significant(source), significant(&output));
}

#[test]
fn concatenation_wrapping_preserves_return_expression_lists() {
    let source = "local function labels()\n  return formatLabel(5, shortcuts) .. \"  \" .. formatLabel(6, shortcuts) .. \"  \" .. formatLabel(7, shortcuts), tail()\nend\n";
    let output = format(source);
    assert!(output.contains("formatLabel(5, shortcuts)"), "{output}");
    assert!(output.contains("\n    tail()"), "{output}");
    corpus::same_program(source, &output).unwrap();
    assert_eq!(lua_tokens(source), lua_tokens(&output));
    assert_eq!(format(&output), output);
}

#[test]
fn concatenation_wrapping_preserves_grouping_comments_and_return_tails() {
    let options = FormatOptions {
        line_width: 60,
        ..FormatOptions::default()
    };
    for source in [
        "local message = prefix .. formatLabel(5, true) .. \" and .. or \" .. suffix -- keep ..\n",
        "return prefix .. (middle .. suffix) .. [=[literal .. text]=], tail()\n",
        "local message = \"é\" .. firstLongOperand .. secondLongOperand .. thirdLongOperand\n",
        "return prefix .. formatLabel(5, true) .. suffix, prefix .. suffix, ...\n",
    ] {
        let output = format_tua_source("src/main.tua", source, &options).unwrap();
        assert_eq!(lua_tokens(source), lua_tokens(&output));
        assert_eq!(
            format_tua_source("src/main.tua", &output, &options).unwrap(),
            output
        );
    }
}

#[test]
fn concatenation_in_call_arguments_and_open_calls_settles_in_one_pass() {
    let options = FormatOptions {
        line_width: 60,
        ..FormatOptions::default()
    };
    for source in [
        "draw(\n  firstLabel .. tostring(counter) .. secondLabel .. tostring(capacity),\n  20,\n  30\n)\n",
        "draw(firstLabel .. tostring(counter) .. secondLabel .. tostring(capacity), 20, 30)\n",
        "local label = label .. formatMessage(\"Long text with several arguments\",\n  firstArgument, secondArgument, thirdArgument)\n",
    ] {
        let output = format_tua_source("src/main.tua", source, &options).unwrap();
        assert_eq!(lua_tokens(source), lua_tokens(&output));
        assert_eq!(format_tua_source("src/main.tua", &output, &options).unwrap(), output);
    }
}

#[test]
fn wrapped_function_type_alias_settles_in_one_pass() {
    let options = FormatOptions {
        line_width: 60,
        ..FormatOptions::default()
    };
    let source = "type Resolver = (key: string, source: string, arguments: string, locale: string) -> string\n";
    let output = format_tua_source("contract.tua", source, &options).unwrap();
    assert!(output.starts_with("type Resolver = (\n"), "{output}");
    assert_eq!(
        format_tua_source("contract.tua", &output, &options).unwrap(),
        output
    );
}

#[test]
fn stylua_authored_types_imports_and_runtime_tests_are_stable() {
    for source in [
        "import type {Player,Room as Level} from './model'\nlocal p:Player\n",
        "import type Models from './model'\nexport type {Player as Actor} from './model'\n",
        "class Actor {\n hp:number=1\n}\nlocal actor=Actor.new()\nif actor is Actor and actor.hp>0 then print(actor.hp) end\n",
        "class Factory {\n create:(number)->number=function(value:number):number return value+1 end\n}\nlocal factory=Factory.new()\nprint(factory.create(2))\n",
        "class Factory {\n values: number[] = {1,2,3}\n init(\n -- input\n value: number\n ) {\n print(value)\n }\n}\n",
    ] {
        let output = format(source);
        assert_eq!(format(&output), output, "{source}");
        assert!(parse_tua_source("test.tua", &output).is_ok(), "{output}");
    }
}

#[test]
fn stylua_default_quotes_minimize_escapes_and_preserve_is_explicit() {
    assert_eq!(format("local x='plain'\n"), "local x = \"plain\"\n");
    assert_eq!(
        format("local x='say \"hello\"'\n"),
        "local x = 'say \"hello\"'\n"
    );
    let options = FormatOptions {
        quote_style: QuoteStyle::Preserve,
        ..FormatOptions::default()
    };
    assert_eq!(
        format_tua_source("test.tua", "local x='plain'\n", &options).unwrap(),
        "local x = 'plain'\n"
    );
}

#[test]
fn range_formatting_class_fields_and_methods_preserves_surrounding_source() {
    let source = "local before=1\nclass Counter {\ncount:number=0\n  add(value:number):number {\nself.count+=value\nreturn self.count\n  }\n}\nlocal after=2\n";
    for (line, next) in [(3, 4), (5, 6)] {
        let edits = format_tua_source_range(
            "counter.tua",
            source,
            &FormatOptions::default(),
            SourceSpan {
                start: SourcePosition { line, column: 1 },
                end: SourcePosition {
                    line: next,
                    column: 1,
                },
            },
        )
        .unwrap();
        assert!(!edits.is_empty(), "selected class member should format");
        let mut output = source.to_owned();
        for edit in edits.iter().rev() {
            assert!(edit.span.start.line >= line && edit.span.end.line <= next);
            let start = source_byte_offset(source, edit.span.start);
            let end = source_byte_offset(source, edit.span.end);
            output.replace_range(start..end, &edit.new_text);
        }
        assert!(output.starts_with("local before=1\nclass Counter {\n"));
        assert!(output.ends_with("}\nlocal after=2\n"));
        corpus::same_program(source, &output).unwrap();
    }
}

#[test]
fn wrapped_class_and_protocol_record_types_remain_parseable() {
    let source = "protocol Values {\n  read(): { firstLongPropertyName: number, secondLongPropertyName: string, thirdLongPropertyName: boolean }\n}\nclass Holder {\n  value: { firstLongPropertyName: number, secondLongPropertyName: string, thirdLongPropertyName: boolean }\n}\n";
    let options = FormatOptions {
        line_width: 45,
        ..FormatOptions::default()
    };
    let output = format_tua_source("records.tua", source, &options).unwrap();
    assert!(output.contains("firstLongPropertyName: number,\n"));
    assert_eq!(
        format_tua_source("records.tua", &output, &options).unwrap(),
        output
    );
    corpus::same_program(source, &output).unwrap();
}

#[test]
fn type_import_formatting_preserves_comments_and_stabilizes_without_final_newline() {
    for source in [
        "import type { Actor, Player as Hero } from './actors'",
        "import type {\n Actor -- exported actor\n , Player as Hero,\n} from './actors'\n",
        "export type {\n Actor as PublicActor,\n} -- public types\nfrom './actors'\n",
    ] {
        let output = format(source);
        assert_eq!(format(&output), output);
        let before = parse_tua_source("types.tua", source).unwrap();
        let after = parse_tua_source("types.tua", &output).unwrap();
        assert_eq!(before.type_imports[0].module, after.type_imports[0].module);
        assert_eq!(
            before.type_imports[0].bindings.len(),
            after.type_imports[0].bindings.len()
        );
    }
}

fn assert_nested_table_stability(source: &str) {
    for line_width in [80, 100, 120] {
        for quote_style in [QuoteStyle::Preserve, QuoteStyle::AutoDouble] {
            for line_ending in [LineEnding::Lf, LineEnding::Crlf] {
                let options = FormatOptions {
                    line_width,
                    quote_style,
                    line_ending,
                    ..FormatOptions::default()
                };
                let first = format_tua_source("nested.tua", source, &options).unwrap();
                let second = format_tua_source("nested.tua", &first, &options).unwrap();
                assert_eq!(first, second, "{source} with {options:?}");
                corpus::same_program(source, &first).unwrap();
            }
        }
    }
}

#[test]
fn nested_table_assignment_expands_with_its_callback_in_one_pass() {
    assert_nested_table_stability(
        r#"package.loaded['demo.state'] = { state = { update = function(_, callback) callback(value) end } }"#,
    );
}

#[test]
fn nested_table_width_excludes_leading_indentation() {
    assert_nested_table_stability(
        r#"return { entries = { entries = { { bpm = 60.000000000, score = 0.459995278, relativeScore = 1.000000000, relation = "primary" }, { bpm = 119.650659850, score = 0.447463229, relativeScore = 0.972755000, relation = "other" } } } }"#,
    );
}

#[test]
fn table_width_accounts_for_inserted_spacing_in_one_pass() {
    assert_nested_table_stability(
        "return {field_0 = {field_0 = 12.345678900,field_1 = true,field_2 = 1,field_3 = true},field_1 = {field_0 = true} ,}\n",
    );
}

#[test]
fn on_type_formatting_does_not_drop_a_statement_moved_to_another_line() {
    let source = "local first=1; local second=2\n\n";
    let edits = format_tua_source_on_type(
        "input.tua",
        source,
        &FormatOptions::default(),
        SourcePosition { line: 2, column: 1 },
        "\n",
    )
    .unwrap();
    let mut output = source.to_owned();
    for edit in edits.iter().rev() {
        let start = source_byte_offset(source, edit.span.start);
        let end = source_byte_offset(source, edit.span.end);
        output.replace_range(start..end, &edit.new_text);
    }
    corpus::same_program(source, &output).unwrap();
}

#[test]
fn multiline_nested_table_candidate_expands_its_parent_in_one_pass() {
    assert_nested_table_stability(
        "return {field_0 = {field_0 = {field_0 = {field_0 = 12.345678900, field_1 = true}} ,},field_1 = {field_0 = {field_0 = {field_0 = true ,} ,} ,} ,}\n",
    );
}

#[test]
fn deeply_nested_multiline_values_do_not_repeatedly_explore_both_table_layouts() {
    let source = format!(
        "return {}[[hello\nworld]]{}\n",
        "{ child = ".repeat(24),
        " }".repeat(24)
    );
    let options = FormatOptions {
        line_width: 1_000_000,
        ..FormatOptions::default()
    };
    let first = format_tua_source("nested.tua", &source, &options).unwrap();
    let second = format_tua_source("nested.tua", &first, &options).unwrap();
    assert_eq!(first, second);
    corpus::same_program(&source, &first).unwrap();
}

#[test]
fn table_call_arguments_use_canonical_width_in_layout_trials() {
    assert_nested_table_stability(
        "draw(1, {field_0 = {field_0 = {field_0 = true ,},field_1 = {field_0 = true} ,}}, 2)\n",
    );
}

#[test]
fn named_table_key_comments_survive_formatting() {
    for comment in ["-- key note\n", "--[[ key note ]] "] {
        let source = format!("local t={{ key {comment}= 1 }}\n");
        let output = format_tua_source("input.tua", &source, &FormatOptions::default()).unwrap();
        assert!(output.contains(comment.trim()));
        assert_eq!(
            format_tua_source("input.tua", &output, &FormatOptions::default()).unwrap(),
            output
        );
        corpus::same_program(&source, &output).unwrap();
    }
}

#[test]
fn table_key_comments_keep_their_order_relative_to_strings() {
    for key in ["key", "[\"key\"]", "[lookup(\"key\")]"] {
        for comment in ["-- key note\n", "--[[ key note ]] ", "---@type string\n"] {
            for field in [
                format!("{key} {comment}= \"value\""),
                format!("{key} = {comment}\"value\""),
            ] {
                let source = format!("local t={{ {field} }}\n");
                let output =
                    format_tua_source("input.tua", &source, &FormatOptions::default()).unwrap();
                assert!(output.contains(comment.trim()));
                assert!(output.find(key).unwrap() < output.find(comment.trim()).unwrap());
                assert!(output.find(comment.trim()).unwrap() < output.find("\"value\"").unwrap());
                assert_eq!(
                    format_tua_source("input.tua", &output, &FormatOptions::default()).unwrap(),
                    output
                );
                corpus::same_program(&source, &output).unwrap();
            }
        }
    }
}

#[test]
fn lua_formatting_accepts_luajit_syntax_without_expanding_tua() {
    use full_moon::node::Node;
    for source in [
        "local value=1LL\nreturn value\n",
        "local value=1ULL\nreturn value\n",
        "local value=1i\nreturn value\n",
        "local value=0x1.fp3\nreturn value\n",
        "goto done\n::done::\nreturn 1\n",
    ] {
        let output = format_source(
            "input.lua",
            source,
            &FormatOptions::default(),
            ParsedLanguage::Lua,
        )
        .unwrap();
        let original = parse_lua_source("input.lua", source).unwrap();
        let formatted = parse_lua_source("input.lua", &output).unwrap();
        assert!(original.ast.nodes().similar(formatted.ast.nodes()));
        assert_eq!(
            format_source(
                "input.lua",
                &output,
                &FormatOptions::default(),
                ParsedLanguage::Lua
            )
            .unwrap(),
            output
        );
        assert!(format_tua_source("input.tua", source, &FormatOptions::default()).is_err());
    }
    let typed = "local value: number=1\n";
    assert!(format_source(
        "input.lua",
        typed,
        &FormatOptions::default(),
        ParsedLanguage::Lua
    )
    .is_err());
    assert!(format_tua_source("input.tua", typed, &FormatOptions::default()).is_ok());
}

#[test]
fn partial_lua_formatting_validates_with_the_luajit_dialect() {
    use full_moon::node::Node;
    let source = "local value=tonumber(1LL)\nreturn value\n";
    let options = FormatOptions::default();
    let range = format_source_range(
        "input.lua",
        source,
        &options,
        SourceSpan {
            start: SourcePosition { line: 1, column: 1 },
            end: SourcePosition { line: 2, column: 1 },
        },
        ParsedLanguage::Lua,
    )
    .unwrap();
    let on_type = format_source_on_type(
        "input.lua",
        source,
        &options,
        SourcePosition {
            line: 1,
            column: source.lines().next().unwrap().len() + 1,
        },
        ")",
        ParsedLanguage::Lua,
    )
    .unwrap();
    for edits in [range, on_type] {
        assert!(!edits.is_empty());
        let mut output = source.to_owned();
        for edit in edits.iter().rev() {
            let start = source_byte_offset(source, edit.span.start);
            let end = source_byte_offset(source, edit.span.end);
            output.replace_range(start..end, &edit.new_text);
        }
        assert!(parse_lua_source("input.lua", source)
            .unwrap()
            .ast
            .nodes()
            .similar(parse_lua_source("input.lua", output).unwrap().ast.nodes()));
    }
}
