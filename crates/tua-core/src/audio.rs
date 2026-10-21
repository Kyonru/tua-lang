use std::collections::{BTreeMap, BTreeSet};

use full_moon::ast::{
    Call, Expression, Field, FunctionArgs, FunctionCall, Suffix, TableConstructor,
};
use full_moon::node::Node;

use crate::ast_utils::{field_chain_before_first_call, token_text};
use crate::types::{FunctionParameter, FunctionType, TableField, Type, TypeEnvironment};
use crate::{Diagnostic, DiagnosticCode, ParsedSource, SourceSpan, TypedFunction, TypedParameter};

pub(crate) const TYPE_NAME: &str = "AudioSystem";
pub(crate) const OPTIONS_TYPE: &str = "AudioSystemOptions";
pub(crate) const VOICE_TYPE: &str = "AudioVoice";
pub(crate) const SNAPSHOT_TYPE: &str = "AudioSnapshot";
pub(crate) const ANALYSIS_JOB_TYPE: &str = "AudioAnalysisJob";
pub(crate) const EVENT_PAYLOAD_TYPE: &str = "AudioEventPayload";
pub(crate) const CONTROL_STATE_TYPE: &str = "AudioControlState";
pub(crate) const CONSTRUCTOR_NAME: &str = "audioSystem";
pub(crate) const RUNTIME_MODULE: &str = "tuaaudio";
pub(crate) const EMITTED_CONSTRUCTOR_LOCAL: &str = "__tua_audio_system";

const BUILTIN_TYPE_NAMES: &[&str] = &[
    TYPE_NAME,
    OPTIONS_TYPE,
    VOICE_TYPE,
    SNAPSHOT_TYPE,
    ANALYSIS_JOB_TYPE,
    EVENT_PAYLOAD_TYPE,
    CONTROL_STATE_TYPE,
];

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct AudioSpec {
    pub(crate) buses: Vec<String>,
    pub(crate) cues: Vec<String>,
    pub(crate) effects: Vec<String>,
    pub(crate) music_states: Vec<String>,
    pub(crate) layers: Vec<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AudioGraph {
    pub systems: Vec<AudioGraphSystem>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AudioGraphSystem {
    pub id: String,
    pub path: std::path::PathBuf,
    pub span: SourceSpan,
    pub buses: Vec<String>,
    pub cues: Vec<String>,
    pub effects: Vec<String>,
    pub music_states: Vec<String>,
    pub layers: Vec<String>,
}

pub(crate) fn audio_graph<'a>(parsed: impl IntoIterator<Item = &'a ParsedSource>) -> AudioGraph {
    let mut graph = AudioGraph::default();
    for source in parsed {
        let mut visitor = AudioGraphVisitor {
            source,
            graph: &mut graph,
        };
        full_moon::visitors::Visitor::visit_ast(&mut visitor, &source.ast);
    }
    graph
}

pub(crate) fn builtin_type_names() -> &'static [&'static str] {
    BUILTIN_TYPE_NAMES
}

pub(crate) fn builtin_aliases() -> Vec<(String, Type)> {
    vec![
        (
            TYPE_NAME.to_owned(),
            system_type(&AudioSpec::default(), false),
        ),
        (OPTIONS_TYPE.to_owned(), options_type()),
        (VOICE_TYPE.to_owned(), voice_type()),
        (SNAPSHOT_TYPE.to_owned(), snapshot_type()),
        (ANALYSIS_JOB_TYPE.to_owned(), analysis_job_type()),
        (EVENT_PAYLOAD_TYPE.to_owned(), event_payload_type()),
        (CONTROL_STATE_TYPE.to_owned(), control_state_type()),
    ]
}

pub(crate) fn is_constructor(name: &str) -> bool {
    name == CONSTRUCTOR_NAME
}

pub(crate) fn constructor_typed_function() -> TypedFunction {
    typed_function(
        CONSTRUCTOR_NAME,
        [parameter("options", Type::alias(OPTIONS_TYPE))],
        [Type::alias(TYPE_NAME)],
        documentation(),
    )
}

pub(crate) const fn detail() -> &'static str {
    "function audioSystem(options: AudioSystemOptions): AudioSystem"
}

pub(crate) const fn documentation() -> &'static str {
    "Create a typed LÖVE audio system with buses, pooled voices, priorities, fades, music scenes, analysis, effects, and runtime insight snapshots."
}

pub(crate) const fn workflow_snippet() -> &'static str {
    "local ${1:audio} = audioSystem({\n  buses = {\n    master = { volume = 1 },\n    music = { parent = \"master\" },\n    sfx = { parent = \"master\" },\n  },\n  cues = {\n    ${2:jump} = { source = sound(\"${3:assets/audio/jump.ogg}\"), bus = \"sfx\" },\n  },\n  music = {},\n})\n\nfunction love.update(dt: number)\n  ${1:audio}:update(dt)\nend\n${0}"
}

pub(crate) fn runtime_source() -> &'static str {
    include_str!("../../../vendor/runtime/tuaaudio.lua")
}

pub(crate) fn audio_type_from_call(call: &FunctionCall) -> Option<Type> {
    let config = config_from_call(call)?;
    let spec = spec_from_config(config);
    Some(system_type(&spec, true))
}

pub(crate) fn typed_member(
    environment: &TypeEnvironment,
    receiver_type: &Type,
    member: &str,
) -> Option<TypedFunction> {
    let table = audio_table(environment, receiver_type, 0)?;
    let Type::Function(function) = &table.semantic_field(member)?.ty else {
        return None;
    };
    let mut function = function.clone();
    let specialized = match member {
        "play" => table.field("__tuaAudioCue"),
        "setBusVolume" | "setBusMuted" | "pauseBus" | "resumeBus" | "stopBus" => {
            table.field("__tuaAudioBus")
        }
        "setMusicState" => table.field("__tuaAudioState"),
        "setMusicLayer" => table.field("__tuaAudioLayer"),
        _ => None,
    };
    if let (Some(field), Some(parameter)) = (specialized, function.parameters.get_mut(1)) {
        parameter.ty = field.ty.clone();
    }
    let mut typed = typed_function_from_type(
        &format!("{TYPE_NAME}:{member}"),
        function,
        method_documentation(member),
    );
    let names: &[&str] = match member {
        "update" => &["self", "dt"],
        "play" => &["self", "cue", "options"],
        "setBusVolume" => &["self", "bus", "volume", "fade"],
        "setBusMuted" => &["self", "bus", "muted"],
        "pauseBus" | "resumeBus" | "stopBus" => &["self", "bus", "fade"],
        "setMusicState" => &["self", "state", "transition"],
        "setMusicLayer" => &["self", "layer", "volume", "fade"],
        "on" | "once" => &["self", "event", "listener"],
        "off" => &["self", "unsubscribe"],
        "subscribe" => &["self", "listener"],
        "requestAnalysis" => &["self", "path", "options"],
        _ => &[],
    };
    for (parameter, name) in typed.parameters.iter_mut().zip(names) {
        parameter.name = (*name).to_owned();
    }
    Some(typed)
}

pub(crate) fn config_diagnostics(parsed: &ParsedSource) -> Vec<Diagnostic> {
    let mut visitor = AudioDiagnosticVisitor {
        path: parsed.path.clone(),
        diagnostics: Vec::new(),
    };
    full_moon::visitors::Visitor::visit_ast(&mut visitor, &parsed.ast);
    visitor.diagnostics
}

fn options_type() -> Type {
    Type::table([
        ("maxVoices", TableField::optional(Type::number())),
        ("overflow", TableField::optional(overflow_type())),
        (
            "buses",
            TableField::optional(Type::dictionary(Type::string(), Type::Any)),
        ),
        (
            "cues",
            TableField::optional(Type::dictionary(Type::string(), Type::Any)),
        ),
        (
            "effects",
            TableField::optional(Type::dictionary(Type::string(), Type::Any)),
        ),
        (
            "music",
            TableField::optional(Type::dictionary(Type::string(), Type::Any)),
        ),
        ("analysis", TableField::optional(Type::Any)),
        ("driftTolerance", TableField::optional(Type::number())),
        ("analysisConfidence", TableField::optional(Type::number())),
    ])
}

fn system_type(spec: &AudioSpec, exact: bool) -> Type {
    let bus = names_type(&spec.buses, exact);
    let cue = names_type(&spec.cues, exact);
    let state = names_type(&spec.music_states, exact);
    let layer = names_type(&spec.layers, exact);
    let event = Type::union(
        [
            "voiceStarted",
            "voiceEnded",
            "voiceStolen",
            "voiceRejected",
            "fadeComplete",
            "busChanged",
            "musicTransitionScheduled",
            "musicTransitionStarted",
            "musicTransitionCompleted",
            "beat",
            "bar",
            "analysisReady",
            "analysisFailed",
            "effectUnsupported",
        ]
        .into_iter()
        .map(Type::string_literal),
    );
    let unsubscribe = Type::Function(FunctionType::new([], Type::nil()));
    let event_listener = Type::Function(FunctionType::new(
        [Type::alias(EVENT_PAYLOAD_TYPE)],
        Type::nil(),
    ));
    // The control store passes the current and previous control state.
    let control_listener = Type::Function(FunctionType::new(
        [
            Type::alias(CONTROL_STATE_TYPE),
            Type::alias(CONTROL_STATE_TYPE),
        ],
        Type::nil(),
    ));
    let transition_status = if exact {
        Type::union(
            ["started", "scheduled", "analysis-pending", "unknown-state"]
                .into_iter()
                .map(Type::string_literal),
        )
    } else {
        Type::Any
    };
    let play_status = if exact {
        Type::union(
            ["started", "stolen", "rejected"]
                .into_iter()
                .map(Type::string_literal),
        )
    } else {
        Type::Any
    };
    let play_returns = [
        Type::union([Type::alias(VOICE_TYPE), Type::nil()]),
        play_status,
    ];

    let mut system = Type::table([
        ("update", method([Type::number()], [Type::nil()])),
        ("play", method_optional([cue, Type::Any], 1, play_returns)),
        (
            "setBusVolume",
            method_optional(
                [bus.clone(), Type::number(), fade_type()],
                2,
                [Type::boolean()],
            ),
        ),
        (
            "setBusMuted",
            method([bus.clone(), Type::boolean()], [Type::boolean()]),
        ),
        (
            "pauseBus",
            method_optional([bus.clone(), fade_type()], 1, [Type::boolean()]),
        ),
        (
            "resumeBus",
            method_optional([bus.clone(), fade_type()], 1, [Type::boolean()]),
        ),
        (
            "stopBus",
            method_optional([bus, fade_type()], 1, [Type::number()]),
        ),
        (
            "setMusicState",
            method_optional([state, Type::Any], 1, [transition_status]),
        ),
        (
            "setMusicLayer",
            method_optional([layer, Type::number(), fade_type()], 2, [Type::boolean()]),
        ),
        (
            "setListenerPosition",
            method(
                [Type::number(), Type::number(), Type::number()],
                [Type::nil()],
            ),
        ),
        (
            "setListenerVelocity",
            method(
                [Type::number(), Type::number(), Type::number()],
                [Type::nil()],
            ),
        ),
        (
            "setListenerOrientation",
            method(
                [
                    Type::number(),
                    Type::number(),
                    Type::number(),
                    Type::number(),
                    Type::number(),
                    Type::number(),
                ],
                [Type::nil()],
            ),
        ),
        (
            "on",
            method(
                [event.clone(), event_listener.clone()],
                [unsubscribe.clone()],
            ),
        ),
        (
            "once",
            method([event, event_listener], [unsubscribe.clone()]),
        ),
        ("off", method([Type::Any], [Type::boolean()])),
        ("subscribe", method([control_listener], [unsubscribe])),
        ("snapshot", method([], [Type::alias(SNAPSHOT_TYPE)])),
        (
            "requestAnalysis",
            method_optional(
                [Type::string(), Type::Any],
                1,
                [Type::alias(ANALYSIS_JOB_TYPE)],
            ),
        ),
    ]);
    if let Type::Table(table) = &mut system {
        table.insert(
            "__tuaAudioBus",
            TableField::optional(names_type(&spec.buses, exact)),
        );
        table.insert(
            "__tuaAudioCue",
            TableField::optional(names_type(&spec.cues, exact)),
        );
        table.insert(
            "__tuaAudioState",
            TableField::optional(names_type(&spec.music_states, exact)),
        );
        table.insert(
            "__tuaAudioLayer",
            TableField::optional(names_type(&spec.layers, exact)),
        );
    }
    system
}

fn voice_type() -> Type {
    Type::table([
        (
            "pause",
            method_optional_on(VOICE_TYPE, [fade_type()], 0, [Type::boolean()]),
        ),
        (
            "resume",
            method_optional_on(VOICE_TYPE, [fade_type()], 0, [Type::boolean()]),
        ),
        (
            "stop",
            method_optional_on(VOICE_TYPE, [fade_type()], 0, [Type::boolean()]),
        ),
        (
            "fadeTo",
            method_on(VOICE_TYPE, [Type::number(), fade_type()], [Type::boolean()]),
        ),
        (
            "setVolume",
            method_on(VOICE_TYPE, [Type::number()], [Type::boolean()]),
        ),
        (
            "setPitch",
            method_on(VOICE_TYPE, [Type::number()], [Type::boolean()]),
        ),
        (
            "setPosition",
            method_on(
                VOICE_TYPE,
                [Type::number(), Type::number(), Type::number()],
                [Type::boolean()],
            ),
        ),
        ("isPlaying", method_on(VOICE_TYPE, [], [Type::boolean()])),
        ("status", method_on(VOICE_TYPE, [], [Type::string()])),
    ])
}

fn snapshot_type() -> Type {
    Type::table([
        ("activeVoices", TableField::required(Type::number())),
        ("pooledVoices", TableField::required(Type::number())),
        ("started", TableField::required(Type::number())),
        ("stolen", TableField::required(Type::number())),
        ("rejected", TableField::required(Type::number())),
        ("buses", TableField::required(Type::Any)),
        ("music", TableField::required(Type::Any)),
        ("analysis", TableField::required(Type::Any)),
    ])
}

fn analysis_job_type() -> Type {
    Type::table([
        ("status", method_on(ANALYSIS_JOB_TYPE, [], [Type::string()])),
        (
            "cancel",
            method_on(ANALYSIS_JOB_TYPE, [], [Type::boolean()]),
        ),
        (
            "onComplete",
            method_on(ANALYSIS_JOB_TYPE, [Type::Any], [Type::Any]),
        ),
    ])
}

fn event_payload_type() -> Type {
    Type::table([
        ("cue", TableField::optional(Type::string())),
        ("stolenCue", TableField::optional(Type::string())),
        ("bus", TableField::optional(Type::string())),
        ("state", TableField::optional(Type::string())),
        ("effect", TableField::optional(Type::string())),
        ("path", TableField::optional(Type::string())),
        ("reason", TableField::optional(Type::string())),
        ("result", TableField::optional(Type::string())),
        ("quantize", TableField::optional(Type::string())),
        ("priority", TableField::optional(Type::number())),
        ("stolenPriority", TableField::optional(Type::number())),
        ("volume", TableField::optional(Type::number())),
        ("beat", TableField::optional(Type::number())),
        ("bar", TableField::optional(Type::number())),
        ("position", TableField::optional(Type::number())),
        ("muted", TableField::optional(Type::boolean())),
        ("cached", TableField::optional(Type::boolean())),
        ("voice", TableField::optional(Type::alias(VOICE_TYPE))),
        ("analysis", TableField::optional(Type::Any)),
    ])
}

fn control_state_type() -> Type {
    Type::table([
        ("activeVoices", TableField::required(Type::number())),
        (
            "musicState",
            TableField::required(Type::union([Type::string(), Type::nil()])),
        ),
    ])
}

fn fade_type() -> Type {
    Type::union([
        Type::number(),
        Type::table([
            ("duration", TableField::required(Type::number())),
            (
                "curve",
                TableField::optional(Type::union(
                    ["linear", "smoothstep", "exponential"]
                        .into_iter()
                        .map(Type::string_literal),
                )),
            ),
        ]),
    ])
}

fn overflow_type() -> Type {
    Type::union(["steal", "reject"].into_iter().map(Type::string_literal))
}

fn names_type(names: &[String], exact: bool) -> Type {
    if !exact {
        Type::Any
    } else if names.is_empty() {
        Type::string()
    } else {
        Type::union(names.iter().cloned().map(Type::string_literal))
    }
}

fn method(
    arguments: impl IntoIterator<Item = Type>,
    returns: impl IntoIterator<Item = Type>,
) -> TableField {
    method_on(TYPE_NAME, arguments, returns)
}

fn method_optional(
    arguments: impl IntoIterator<Item = Type>,
    required: usize,
    returns: impl IntoIterator<Item = Type>,
) -> TableField {
    method_optional_on(TYPE_NAME, arguments, required, returns)
}

/// A method whose `self` is `owner`, such as a voice handle's methods, which
/// are called on the `AudioVoice` returned by `play`, not on the system.
fn method_on(
    owner: &str,
    arguments: impl IntoIterator<Item = Type>,
    returns: impl IntoIterator<Item = Type>,
) -> TableField {
    let arguments = arguments.into_iter().collect::<Vec<_>>();
    method_optional_on(owner, arguments.clone(), arguments.len(), returns)
}

fn method_optional_on(
    owner: &str,
    arguments: impl IntoIterator<Item = Type>,
    required: usize,
    returns: impl IntoIterator<Item = Type>,
) -> TableField {
    let mut parameters = vec![FunctionParameter::named("self", Type::alias(owner))];
    parameters.extend(
        arguments
            .into_iter()
            .enumerate()
            .map(|(index, ty)| FunctionParameter::named(format!("arg{}", index + 1), ty)),
    );
    TableField::required(Type::Function(FunctionType::new_signature(
        parameters,
        required + 1,
        None,
        returns,
    )))
}

fn spec_from_config(config: &TableConstructor) -> AudioSpec {
    let buses = named_field(config, "buses")
        .and_then(table_expression)
        .map(named_keys)
        .unwrap_or_default();
    let cues = named_field(config, "cues")
        .and_then(table_expression)
        .map(named_keys)
        .unwrap_or_default();
    let effects = named_field(config, "effects")
        .and_then(table_expression)
        .map(named_keys)
        .unwrap_or_default();
    let music_table = named_field(config, "music").and_then(table_expression);
    let music_states = music_table.map(named_keys).unwrap_or_default();
    let mut layers = BTreeSet::new();
    if let Some(music) = music_table {
        for (_, scene) in named_table_fields(music) {
            if let Some(scene) = table_expression(scene) {
                if let Some(scene_layers) = named_field(scene, "layers").and_then(table_expression)
                {
                    layers.extend(named_keys(scene_layers));
                }
            }
        }
    }
    AudioSpec {
        buses,
        cues,
        effects,
        music_states,
        layers: layers.into_iter().collect(),
    }
}

fn config_from_call(call: &FunctionCall) -> Option<&TableConstructor> {
    let parts = field_chain_before_first_call(call)?;
    if parts.as_slice() != [CONSTRUCTOR_NAME] {
        return None;
    }
    let Expression::TableConstructor(config) = first_parenthesized_argument(call)? else {
        return None;
    };
    Some(config)
}

fn first_parenthesized_argument(call: &FunctionCall) -> Option<&Expression> {
    call.suffixes().find_map(|suffix| match suffix {
        Suffix::Call(Call::AnonymousCall(FunctionArgs::Parentheses { arguments, .. })) => {
            arguments.iter().next()
        }
        _ => None,
    })
}

fn named_field<'a>(table: &'a TableConstructor, name: &str) -> Option<&'a Expression> {
    named_table_fields(table).find_map(|(candidate, value)| (candidate == name).then_some(value))
}

fn named_table_fields(table: &TableConstructor) -> impl Iterator<Item = (String, &Expression)> {
    table.fields().iter().filter_map(|field| match field {
        Field::NameKey { key, value, .. } => Some((token_text(key), value)),
        Field::ExpressionKey { key, value, .. } => string_expression(key).map(|key| (key, value)),
        _ => None,
    })
}

fn named_keys(table: &TableConstructor) -> Vec<String> {
    named_table_fields(table).map(|(name, _)| name).collect()
}

fn table_expression(expression: &Expression) -> Option<&TableConstructor> {
    match expression {
        Expression::TableConstructor(table) => Some(table),
        _ => None,
    }
}

fn string_expression(expression: &Expression) -> Option<String> {
    match expression {
        Expression::String(token) => Some(token_text(token).trim_matches(['\'', '"']).to_owned()),
        _ => None,
    }
}

fn number_expression(expression: &Expression) -> Option<f64> {
    match expression {
        Expression::Number(token) => token_text(token).parse().ok(),
        _ => None,
    }
}

fn audio_table<'a>(
    environment: &'a TypeEnvironment,
    candidate: &'a Type,
    depth: usize,
) -> Option<&'a crate::types::TableShape> {
    if depth > 8 {
        return None;
    }
    match candidate {
        Type::Table(table)
            if table.field("play").is_some() && table.field("setMusicState").is_some() =>
        {
            Some(table)
        }
        Type::Alias(name) => environment
            .alias(name)
            .and_then(|ty| audio_table(environment, ty, depth + 1)),
        _ => None,
    }
}

fn typed_function(
    name: &str,
    parameters: impl IntoIterator<Item = TypedParameter>,
    returns: impl IntoIterator<Item = Type>,
    documentation: &str,
) -> TypedFunction {
    let parameters = parameters.into_iter().collect::<Vec<_>>();
    let returns = returns.into_iter().collect::<Vec<_>>();
    TypedFunction {
        name: Some(name.to_owned()),
        parameter_count: parameters.len(),
        required_parameter_count: parameters.len(),
        parameters,
        has_varargs: false,
        vararg_type: None,
        return_type: returns.first().cloned(),
        return_types: returns,
        documentation: Some(documentation.to_owned()),
        overloads: Vec::new(),
    }
}

fn typed_function_from_type(
    name: &str,
    function: FunctionType,
    documentation: &str,
) -> TypedFunction {
    let parameters = function
        .parameters
        .iter()
        .map(|parameter| TypedParameter {
            name: parameter.name.clone().unwrap_or_else(|| "value".to_owned()),
            ty: parameter.ty.clone(),
            documentation: None,
        })
        .collect::<Vec<_>>();
    TypedFunction {
        name: Some(name.to_owned()),
        parameter_count: parameters.len(),
        required_parameter_count: function.required_parameter_count,
        parameters,
        has_varargs: function.vararg_type.is_some(),
        vararg_type: function.vararg_type.clone().map(|ty| *ty),
        return_type: function.return_types_or_any().first().cloned(),
        return_types: function.return_types_or_any().to_vec(),
        documentation: Some(documentation.to_owned()),
        overloads: Vec::new(),
    }
}

fn parameter(name: &str, ty: Type) -> TypedParameter {
    TypedParameter {
        name: name.to_owned(),
        ty,
        documentation: None,
    }
}

fn method_documentation(member: &str) -> &'static str {
    match member {
        "play" => "Play a configured cue subject to its bus, voice limit, and priority policy.",
        "setMusicState" => "Start or schedule a transition to a configured music scene.",
        "snapshot" => "Return a detached runtime insight snapshot.",
        "requestAnalysis" => "Explicitly analyze a dynamic audio file on a LÖVE worker thread.",
        _ => "Control or inspect this audio system.",
    }
}

struct AudioDiagnosticVisitor {
    path: std::path::PathBuf,
    diagnostics: Vec<Diagnostic>,
}

impl full_moon::visitors::Visitor for AudioDiagnosticVisitor {
    fn visit_function_call(&mut self, call: &FunctionCall) {
        let Some(config) = config_from_call(call) else {
            return;
        };
        let buses_table = named_field(config, "buses").and_then(table_expression);
        if named_field(config, "maxVoices")
            .and_then(number_expression)
            .is_some_and(|limit| limit < 1.0 || limit.fract() != 0.0)
        {
            self.diagnostics.push(audio_error(
                &self.path,
                "audio system `maxVoices` must be a positive integer".to_owned(),
            ));
        }
        let bus_names = buses_table
            .map(named_keys)
            .unwrap_or_default()
            .into_iter()
            .collect::<BTreeSet<_>>();
        let mut parents = BTreeMap::new();
        if let Some(buses) = buses_table {
            for (name, value) in named_table_fields(buses) {
                let Some(bus) = table_expression(value) else {
                    continue;
                };
                if let Some(parent) = named_field(bus, "parent").and_then(string_expression) {
                    if !bus_names.contains(&parent) {
                        self.diagnostics.push(audio_error(
                            &self.path,
                            format!("audio bus `{name}` references unknown parent `{parent}`"),
                        ));
                    }
                    parents.insert(name.clone(), parent);
                }
                if named_field(bus, "maxVoices")
                    .and_then(number_expression)
                    .is_some_and(|limit| limit < 1.0 || limit.fract() != 0.0)
                {
                    self.diagnostics.push(audio_error(
                        &self.path,
                        format!("audio bus `{name}` has an invalid voice limit"),
                    ));
                }
            }
        }
        for start in parents.keys() {
            let mut seen = BTreeSet::new();
            let mut current = start.as_str();
            while let Some(parent) = parents.get(current) {
                if !seen.insert(current.to_owned()) {
                    self.diagnostics.push(audio_error(
                        &self.path,
                        format!("audio bus hierarchy contains a cycle through `{start}`"),
                    ));
                    break;
                }
                current = parent;
            }
        }
        if let Some(cues) = named_field(config, "cues").and_then(table_expression) {
            for (name, value) in named_table_fields(cues) {
                let Some(cue) = table_expression(value) else {
                    continue;
                };
                if let Some(bus) = named_field(cue, "bus").and_then(string_expression) {
                    if !bus_names.contains(&bus) {
                        self.diagnostics.push(audio_error(
                            &self.path,
                            format!("audio cue `{name}` references unknown bus `{bus}`"),
                        ));
                    }
                }
                if named_field(cue, "maxVoices")
                    .and_then(number_expression)
                    .is_some_and(|limit| limit < 1.0 || limit.fract() != 0.0)
                {
                    self.diagnostics.push(audio_error(
                        &self.path,
                        format!("audio cue `{name}` has an invalid voice limit"),
                    ));
                }
                if named_field(cue, "overflow")
                    .and_then(string_expression)
                    .is_some_and(|overflow| overflow != "steal" && overflow != "reject")
                {
                    self.diagnostics.push(audio_error(
                        &self.path,
                        format!("audio cue `{name}` overflow must be `steal` or `reject`"),
                    ));
                }
            }
        }
        if let Some(effects) = named_field(config, "effects").and_then(table_expression) {
            for (name, value) in named_table_fields(effects) {
                let Some(effect) = table_expression(value) else {
                    continue;
                };
                if named_field(effect, "type")
                    .and_then(string_expression)
                    .is_none()
                {
                    self.diagnostics.push(audio_error(
                        &self.path,
                        format!("audio effect `{name}` requires a string `type`"),
                    ));
                }
            }
        }
    }
}

fn audio_error(path: &std::path::Path, message: String) -> Diagnostic {
    Diagnostic::error(path.to_path_buf(), 1, 1, message)
        .with_code(DiagnosticCode::DOMAIN_AUDIO_CONFIG)
}

struct AudioGraphVisitor<'a, 'b> {
    source: &'a ParsedSource,
    graph: &'b mut AudioGraph,
}

impl full_moon::visitors::Visitor for AudioGraphVisitor<'_, '_> {
    fn visit_function_call(&mut self, call: &FunctionCall) {
        let Some(config) = config_from_call(call) else {
            return;
        };
        let Some((start, end)) = call.range() else {
            return;
        };
        let span = SourceSpan::from_range(start, end);
        let spec = spec_from_config(config);
        self.graph.systems.push(AudioGraphSystem {
            id: format!(
                "{}:{}:{}",
                self.source.path.display(),
                span.start.line,
                span.start.column
            ),
            path: self.source.path.clone(),
            span,
            buses: spec.buses,
            cues: spec.cues,
            effects: spec.effects,
            music_states: spec.music_states,
            layers: spec.layers,
        });
    }
}
