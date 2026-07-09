use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, thiserror::Error)]
pub enum FormError {
    #[error("unsupported form code: {0}")]
    UnsupportedForm(String),
    #[error("invalid bundled form definition: {0}")]
    InvalidDefinition(String),
    #[error("missing required value for placeholder `{placeholder}` from path `{path}`")]
    MissingValue { placeholder: String, path: String },
    #[error("unresolved template placeholder remains: {0}")]
    UnresolvedPlaceholder(String),
}

#[derive(Debug, Clone, Deserialize)]
pub struct FormMetadata {
    pub code: String,
    pub version: String,
    pub display_name: String,
    pub category: String,
    pub frequency: String,
    pub remote_directory: String,
    pub filename_pattern: String,
    #[serde(default)]
    pub requires_employees: bool,
    #[serde(default)]
    pub requires_expanded_withholding_agent: bool,
    #[serde(default)]
    pub requires_vat_registered: bool,
}

#[derive(Debug, Clone)]
pub struct FormDefinition {
    pub metadata: FormMetadata,
    pub template: &'static str,
    pub mapping: BTreeMap<String, String>,
}

impl FormDefinition {
    pub fn builtin(form_code: &str) -> Result<Self, FormError> {
        match form_code.to_ascii_uppercase().as_str() {
            "1601C" => Self::from_static(
                include_str!("../forms/1601C/form.toml"),
                include_str!("../forms/1601C/mapping.toml"),
                include_str!("../forms/1601C/template.xml"),
            ),
            other => Err(FormError::UnsupportedForm(other.to_string())),
        }
    }

    fn from_static(
        metadata_toml: &'static str,
        mapping_toml: &'static str,
        template: &'static str,
    ) -> Result<Self, FormError> {
        let metadata: FormMetadata = toml::from_str(metadata_toml)
            .map_err(|err| FormError::InvalidDefinition(err.to_string()))?;
        let mapping: BTreeMap<String, String> = toml::from_str(mapping_toml)
            .map_err(|err| FormError::InvalidDefinition(err.to_string()))?;
        Ok(Self {
            metadata,
            template,
            mapping,
        })
    }
}

pub fn render_form(form_code: &str, input: &Value) -> Result<String, FormError> {
    let definition = FormDefinition::builtin(form_code)?;
    render_definition(&definition, input)
}

pub fn render_definition(definition: &FormDefinition, input: &Value) -> Result<String, FormError> {
    let mut rendered = definition.template.to_string();

    for (placeholder, path) in &definition.mapping {
        let value = lookup_string(input, path).ok_or_else(|| FormError::MissingValue {
            placeholder: placeholder.clone(),
            path: path.clone(),
        })?;
        rendered = rendered.replace(&format!("{{{{{placeholder}}}}}"), &value);
    }

    if let Some(unresolved) = first_placeholder(&rendered) {
        return Err(FormError::UnresolvedPlaceholder(unresolved));
    }

    Ok(rendered)
}

fn lookup_string(input: &Value, dotted_path: &str) -> Option<String> {
    let mut current = input;
    for part in dotted_path.split('.') {
        current = current.get(part)?;
    }

    match current {
        Value::String(s) => Some(s.clone()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Number(n) => Some(n.to_string()),
        Value::Null | Value::Array(_) | Value::Object(_) => None,
    }
}

fn first_placeholder(s: &str) -> Option<String> {
    let start = s.find("{{")?;
    let rest = &s[start + 2..];
    let end = rest.find("}}")?;
    Some(rest[..end].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn fixture_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/1601C")
    }

    #[test]
    fn renders_1601c_public_fixture_byte_stable() {
        let input: Value = serde_json::from_slice(
            &fs::read(fixture_dir().join("input.json")).expect("read public input"),
        )
        .expect("parse public input");
        let expected = fs::read_to_string(fixture_dir().join("official_plaintext.xml"))
            .expect("read expected plaintext");

        let rendered = render_form("1601C", &input).expect("render 1601C");
        assert_eq!(rendered, expected);
    }

    #[test]
    fn missing_required_fields_are_validation_errors() {
        let input = serde_json::json!({ "fields": { "txtMonth": "06" } });
        let err = render_form("1601C", &input).expect_err("missing fields should fail");
        assert!(matches!(err, FormError::MissingValue { .. }));
    }
}
