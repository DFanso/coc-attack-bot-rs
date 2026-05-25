use anyhow::Result;
use base64::Engine;
use image::ImageReader;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Duration;

const BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash-lite-preview-06-17:generateContent";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Loot {
    #[serde(default)]
    pub gold: u64,
    #[serde(default)]
    pub elixir: u64,
    #[serde(default)]
    pub dark_elixir: u64,
}

impl Default for Loot {
    fn default() -> Self {
        Self { gold: 0, elixir: 0, dark_elixir: 0 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Analysis {
    #[serde(default)]
    pub loot: Loot,
    #[serde(default)]
    pub townhall_level: u32,
    #[serde(default)]
    pub difficulty: String,
    pub recommendation: String,
    #[serde(default)]
    pub reasoning: String,
    #[serde(default)]
    pub error: bool,
}

pub struct AiAnalyzer {
    pub api_key: String,
    client: reqwest::blocking::Client,
}

impl AiAnalyzer {
    pub fn new(api_key: String) -> Result<Self> {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()?;
        Ok(Self { api_key, client })
    }

    pub fn analyze_base(
        &self,
        screenshot_path: &Path,
        min_gold: u64,
        min_elixir: u64,
        min_dark: u64,
    ) -> Analysis {
        tracing::info!("🤖 Analyzing base with AI: {}", screenshot_path.display());

        let image_data = match encode_image(screenshot_path) {
            Ok(d) => d,
            Err(e) => {
                tracing::error!("Image encoding error: {e}");
                return error_response("Failed to encode image");
            }
        };

        let prompt = build_prompt(min_gold, min_elixir, min_dark);
        match self.send_request(&image_data, &prompt) {
            Ok(Some(a)) => {
                tracing::info!("✅ AI Analysis: {} — {}", a.recommendation, a.reasoning);
                a
            }
            Ok(None) => error_response("Failed to get AI response"),
            Err(e) => {
                tracing::error!("AI request error: {e}");
                error_response(&format!("Analysis error: {e}"))
            }
        }
    }

    pub fn test_connection(&self) -> bool {
        let url = format!("{BASE_URL}?key={}", self.api_key);
        let body = serde_json::json!({
            "contents": [{"parts": [{"text": "Hello, respond with 'OK'"}]}],
            "generationConfig": {"maxOutputTokens": 10}
        });
        match self.client.post(&url).json(&body).send() {
            Ok(resp) if resp.status().is_success() => {
                tracing::info!("✅ Gemini API connection successful");
                true
            }
            Ok(resp) => {
                tracing::error!("❌ Gemini API test failed: {}", resp.status());
                false
            }
            Err(e) => {
                tracing::error!("❌ Gemini API test error: {e}");
                false
            }
        }
    }

    fn send_request(&self, image_b64: &str, prompt: &str) -> Result<Option<Analysis>> {
        let url = format!("{BASE_URL}?key={}", self.api_key);
        let body = serde_json::json!({
            "contents": [{
                "parts": [
                    { "text": prompt },
                    {
                        "inline_data": {
                            "mime_type": "image/png",
                            "data": image_b64,
                        }
                    }
                ]
            }],
            "generationConfig": {
                "temperature": 0.1,
                "topK": 1,
                "topP": 1,
                "maxOutputTokens": 1024
            }
        });

        tracing::info!("🌐 Sending request to Gemini API...");
        let resp = self.client.post(&url).json(&body).send()?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().unwrap_or_default();
            tracing::error!("Gemini API error: {status} — {text}");
            return Ok(None);
        }

        let val: serde_json::Value = resp.json()?;
        let Some(text) = val
            .get("candidates")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("content"))
            .and_then(|c| c.get("parts"))
            .and_then(|p| p.get(0))
            .and_then(|p| p.get("text"))
            .and_then(|t| t.as_str())
        else {
            tracing::error!("No candidates in Gemini response");
            return Ok(None);
        };

        let cleaned = strip_markdown_fence(text);
        match serde_json::from_str::<Analysis>(&cleaned) {
            Ok(a) => Ok(Some(a)),
            Err(e) => {
                tracing::error!("Failed to parse AI response as JSON: {e}");
                tracing::error!("Raw response: {cleaned}");
                Ok(None)
            }
        }
    }
}

fn encode_image(path: &Path) -> Result<String> {
    // Open + decode, resize if wider than 1024, re-encode as PNG, base64.
    let img = ImageReader::open(path)?.decode()?;
    let resized = if img.width() > 1024 {
        let ratio = 1024.0 / img.width() as f32;
        let new_h = (img.height() as f32 * ratio) as u32;
        img.resize_exact(1024, new_h, image::imageops::FilterType::Lanczos3)
    } else {
        img
    };
    let mut buf = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut buf);
    resized.write_to(&mut cursor, image::ImageFormat::Png)?;
    Ok(base64::engine::general_purpose::STANDARD.encode(&buf))
}

fn strip_markdown_fence(s: &str) -> String {
    let s = s.trim();
    let s = s
        .strip_prefix("```json")
        .or_else(|| s.strip_prefix("```"))
        .unwrap_or(s);
    let s = s.strip_suffix("```").unwrap_or(s);
    s.trim().to_string()
}

fn build_prompt(min_gold: u64, min_elixir: u64, min_dark: u64) -> String {
    format!(
        r#"
You are an expert Clash of Clans player analyzing enemy bases for attack decisions.

CRITICAL: You must read the EXACT loot numbers displayed in the top-left area of the screen.

Current loot requirements:
- Minimum Gold: {min_gold:}
- Minimum Elixir: {min_elixir:}
- Minimum Dark Elixir: {min_dark:}

INSTRUCTIONS:
1. Look at the "Available Loot:" section in the top-left corner of the screenshot
2. Read the EXACT numbers next to the gold coin (yellow), elixir drop (pink), and dark elixir drop (black) icons
3. Identify the Town Hall level by looking at the Town Hall building
4. Compare loot numbers to minimum requirements above
5. Make recommendation based on loot AND Town Hall level

LOOT READING RULES:
- Gold is shown next to a yellow coin icon
- Elixir is shown next to a pink/purple drop icon
- Dark Elixir is shown next to a black drop icon
- Numbers may have spaces (e.g. "123 456" = 123456)
- Be extremely careful reading the digits

TOWN HALL RULES:
- Town Hall 13, 14, 15, 16+ are TOO STRONG - always SKIP these
- Only attack Town Hall 12 and below
- Look at the Town Hall building design to identify the level

DECISION CRITERIA:
- ATTACK only if: ALL loot types meet requirements AND Town Hall is level 12 or below
- SKIP if: ANY loot type is below requirements OR Town Hall is level 13+
- Do NOT consider base difficulty - focus ONLY on loot amounts and Town Hall level

Respond in this exact JSON format:
{{
    "loot": {{
        "gold": actual_gold_amount_you_read,
        "elixir": actual_elixir_amount_you_read,
        "dark_elixir": actual_dark_elixir_amount_you_read
    }},
    "townhall_level": town_hall_level_number,
    "difficulty": "Easy/Medium/Hard",
    "recommendation": "ATTACK/SKIP",
    "reasoning": "Specific reason: Gold X vs required Y, Elixir A vs required B, Dark C vs required D, TH level E"
}}
"#
    )
}

fn error_response(msg: &str) -> Analysis {
    Analysis {
        loot: Loot::default(),
        townhall_level: 0,
        difficulty: "Unknown".into(),
        recommendation: "SKIP".into(),
        reasoning: format!("Error: {msg}"),
        error: true,
    }
}
