//! Declarative host-audio metadata; never loads assets or executes callbacks.
use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DialogueSpeech {
    Character,
    Off,
    Profile(serde_json::Value),
}

fn id(value: &serde_json::Value, key: &str, limit: usize) -> bool {
    value[key].as_str().is_some_and(|s| {
        !s.is_empty() && s.len() <= limit && !s.chars().any(|c| c.is_ascii_control())
    })
}
fn number(value: &serde_json::Value, key: &str, low: f64, high: f64) -> bool {
    value[key]
        .as_f64()
        .is_some_and(|n| n.is_finite() && n >= low && n <= high)
}
fn valid(value: &serde_json::Value, voice: bool) -> bool {
    let Some(fields) = value.as_object() else {
        return false;
    };
    if voice {
        fields.keys().all(|k| {
            matches!(
                k.as_str(),
                "cue" | "sourceLocale" | "fallback" | "mixSpeech"
            )
        }) && id(value, "cue", 256)
            && id(value, "sourceLocale", 64)
            && matches!(value["fallback"].as_str(), Some("silent" | "source"))
            && fields.get("mixSpeech").is_none_or(|v| v.is_boolean())
    } else {
        fields.keys().all(|k| {
            matches!(
                k.as_str(),
                "sampleSet"
                    | "pitchMin"
                    | "pitchMax"
                    | "variation"
                    | "gain"
                    | "minCueInterval"
                    | "cadence"
            )
        }) && id(value, "sampleSet", 256)
            && number(value, "pitchMin", 0.25, 4.0)
            && number(
                value,
                "pitchMax",
                value["pitchMin"].as_f64().unwrap_or(4.0),
                4.0,
            )
            && number(value, "variation", 0.0, 1.0)
            && number(value, "gain", 0.0, 1.0)
            && number(value, "minCueInterval", 0.0, 60.0)
            && matches!(value["cadence"].as_str(), Some("character" | "word"))
    }
}

impl Parser<'_> {
    pub(super) fn media_line(&mut self, raw: &str, at: SourceSpan) -> bool {
        let text = raw.trim();
        let (voice, rest) = if let Some(rest) = text.strip_prefix("@voice ") {
            (true, rest)
        } else if let Some(rest) = text.strip_prefix("@speech ") {
            (false, rest)
        } else {
            return false;
        };
        let speech = if !voice && rest == "off" {
            Some(DialogueSpeech::Off)
        } else if !voice && rest == "character" {
            Some(DialogueSpeech::Character)
        } else {
            None
        };
        let value = if speech.is_none() {
            match events::payload(rest) {
                Ok(value) if valid(&value, voice) => Some(value),
                _ => {
                    self.error(
                        at,
                        "invalid voice/speech metadata; use the documented bounded JSON record",
                    );
                    return true;
                }
            }
        } else {
            None
        };
        let imported = self
            .document
            .sections
            .last()
            .and_then(|s| s.lines.last())
            .and_then(|line| line.speaker.as_ref())
            .is_some_and(|speaker| {
                speaker.value.split_once('.').is_some_and(|(alias, _)| {
                    self.document.imports.iter().any(|i| i.alias.value == alias)
                }) && !self
                    .document
                    .speakers
                    .iter()
                    .any(|s| s.id.value == speaker.value)
            });
        let Some(line) = self.metadata_line(at) else {
            return true;
        };
        if voice {
            if line.voice.is_some() {
                self.error(at, "duplicate voice metadata");
            } else {
                line.voice = value;
            }
        } else if line.speech.is_some() {
            self.error(at, "duplicate speech metadata");
        } else {
            let speech = speech.unwrap_or_else(|| DialogueSpeech::Profile(value.unwrap()));
            if speech == DialogueSpeech::Character && !imported {
                self.error(at, "@speech character requires an imported character");
            } else {
                line.speech = Some(speech);
            }
        }
        true
    }
}
