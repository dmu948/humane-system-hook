const LUNA_MODEL: &str = "openai/gpt-5.6-luna";
pub const TERRA_MODEL: &str = "openai/gpt-5.6-terra";
const SOL_MODEL: &str = "openai/gpt-5.6-sol";

pub fn routed_models() -> [&'static str; 3] {
    [LUNA_MODEL, TERRA_MODEL, SOL_MODEL]
}

pub fn select_model(utterance: &str) -> &'static str {
    let text = utterance.to_ascii_lowercase();

    for (name, model) in [
        ("luna", LUNA_MODEL),
        ("terra", TERRA_MODEL),
        ("sol", SOL_MODEL),
    ] {
        if text.contains(&format!("use {name}"))
            || text.contains(&format!("using {name}"))
            || text.contains(&format!("with {name}"))
        {
            return model;
        }
    }

    // Weather commonly needs a tool call followed by a second completion. Luna can
    // finish the tool call but is not reliably fast enough to finish that second turn
    // inside the Pin's fixed 25-second RPC deadline.
    if is_weather_request(&text) {
        return TERRA_MODEL;
    }

    if [
        "research",
        "compare",
        "investigate",
        "diagnose",
        "analyze",
        "why does",
    ]
    .iter()
    .any(|term| text.contains(term))
    {
        return SOL_MODEL;
    }

    if is_short_greeting(&text)
        || ["battery", "device status", "wifi status", "network status"]
            .iter()
            .any(|term| text.contains(term))
    {
        return LUNA_MODEL;
    }

    TERRA_MODEL
}

pub fn is_weather_request(text: &str) -> bool {
    let text = text.to_ascii_lowercase();
    [
        "weather",
        "temperature outside",
        "temperature in ",
        "is it raining",
        "will it rain",
        "humidity",
        "forecast",
        "need a jacket",
        "wind outside",
    ]
    .iter()
    .any(|term| text.contains(term))
}

fn is_short_greeting(text: &str) -> bool {
    let normalized = text.trim().trim_matches(|c: char| !c.is_alphanumeric());
    matches!(
        normalized,
        "hi" | "hello" | "hey" | "good morning" | "good evening"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinary_weather_always_routes_to_terra() {
        for utterance in [
            "What's the weather in Fairfax?",
            "Will it rain tomorrow?",
            "Do I need a jacket outside?",
            "What's the humidity in New York?",
        ] {
            assert_eq!(select_model(utterance), TERRA_MODEL, "{utterance}");
        }
    }

    #[test]
    fn explicit_model_override_wins_for_weather() {
        assert_eq!(
            select_model("Use Luna for the weather in Fairfax"),
            LUNA_MODEL
        );
        assert_eq!(select_model("Check the weather with Sol"), SOL_MODEL);
    }

    #[test]
    fn non_weather_routes_remain_deterministic() {
        assert_eq!(select_model("hello"), LUNA_MODEL);
        assert_eq!(select_model("compare these two approaches"), SOL_MODEL);
        assert_eq!(select_model("tell me a story"), TERRA_MODEL);
    }
}
