use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const AGENT_BROWSER_ROOT: &str = "../../agent-browser";

fn main() {
    ensure_dashboard_dir();
    generate_main_body();
    generate_cdp_types();
}

fn ensure_dashboard_dir() {
    let dashboard_out = Path::new(AGENT_BROWSER_ROOT).join("packages/dashboard/out");
    println!(
        "cargo:rerun-if-changed={}",
        dashboard_out.as_path().display()
    );
    if !dashboard_out.join("index.html").exists() {
        let _ = fs::create_dir_all(&dashboard_out);
        let _ = fs::write(
            dashboard_out.join("index.html"),
            "<!DOCTYPE html><html><body><p>Dashboard not built. Run: cd packages/dashboard &amp;&amp; pnpm build</p></body></html>\n",
        );
    }
}

fn generate_main_body() {
    let main_path = Path::new(AGENT_BROWSER_ROOT).join("cli/src/main.rs");
    println!("cargo:rerun-if-changed={}", main_path.display());

    let content = fs::read_to_string(&main_path).expect("failed to read agent-browser main.rs");
    let normalized = content.replace("\r\n", "\n");
    let body = normalized
        .split_once("\n\n")
        .map(|(_, body)| body)
        .unwrap_or(normalized.as_str());

    let out_path = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR")).join("main_body.rs");
    fs::write(out_path, body).expect("failed to write generated main_body.rs");
}

fn generate_cdp_types() {
    let protocol_dir = Path::new(AGENT_BROWSER_ROOT).join("cli/cdp-protocol");
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR");
    let out_path = Path::new(&out_dir).join("cdp_generated.rs");

    let browser_path = protocol_dir.join("browser_protocol.json");
    let js_path = protocol_dir.join("js_protocol.json");

    if !browser_path.exists() && !js_path.exists() {
        fs::write(
            &out_path,
            "// No protocol JSON files found in cdp-protocol/\n",
        )
        .expect("failed to write empty cdp_generated.rs");
        return;
    }

    let mut all_domains: Vec<Domain> = Vec::new();

    for path in [&browser_path, &js_path] {
        if !path.exists() {
            continue;
        }
        println!("cargo:rerun-if-changed={}", path.display());
        let content = fs::read_to_string(path).expect("failed to read CDP protocol JSON");
        let protocol: ProtocolSpec = match serde_json::from_str(&content) {
            Ok(protocol) => protocol,
            Err(error) => {
                eprintln!(
                    "cargo:warning=Failed to parse {}: {}",
                    path.display(),
                    error
                );
                continue;
            }
        };
        all_domains.extend(protocol.domains);
    }

    let mut domain_types: std::collections::HashMap<String, HashSet<String>> =
        std::collections::HashMap::new();
    for domain in &all_domains {
        let mut types = HashSet::new();
        for type_def in &domain.types {
            types.insert(type_def.id.clone());
        }
        domain_types.insert(domain.domain.clone(), types);
    }

    let recursive_fields: HashSet<(&str, &str, &str)> = [
        ("DOM", "Node", "contentDocument"),
        ("DOM", "Node", "templateContent"),
        ("DOM", "Node", "importedDocument"),
        ("Accessibility", "AXNode", "sources"),
        ("Runtime", "StackTrace", "parent"),
    ]
    .into_iter()
    .collect();

    let mut output = String::new();
    output.push_str("use serde::{Deserialize, Serialize};\n\n");

    for domain in &all_domains {
        generate_domain(domain, &domain_types, &recursive_fields, &mut output);
    }

    fs::write(&out_path, output).expect("failed to write generated CDP types");
}

#[allow(dead_code)]
#[derive(serde::Deserialize)]
struct ProtocolSpec {
    domains: Vec<Domain>,
}

#[allow(dead_code)]
#[derive(serde::Deserialize, Clone)]
struct Domain {
    domain: String,
    #[serde(default)]
    types: Vec<TypeDef>,
    #[serde(default)]
    commands: Vec<Command>,
    #[serde(default)]
    events: Vec<Event>,
}

#[allow(dead_code)]
#[derive(serde::Deserialize, Clone)]
struct TypeDef {
    id: String,
    #[serde(rename = "type", default)]
    type_kind: String,
    #[serde(default)]
    properties: Vec<Property>,
    #[serde(rename = "enum", default)]
    enum_values: Vec<String>,
    #[serde(default)]
    description: Option<String>,
}

#[allow(dead_code)]
#[derive(serde::Deserialize, Clone)]
struct Command {
    name: String,
    #[serde(default)]
    parameters: Vec<Property>,
    #[serde(default)]
    returns: Vec<Property>,
    #[serde(default)]
    description: Option<String>,
}

#[allow(dead_code)]
#[derive(serde::Deserialize, Clone)]
struct Event {
    name: String,
    #[serde(default)]
    parameters: Vec<Property>,
    #[serde(default)]
    description: Option<String>,
}

#[allow(dead_code)]
#[derive(serde::Deserialize, Clone)]
struct Property {
    name: String,
    #[serde(rename = "type", default)]
    type_kind: Option<String>,
    #[serde(rename = "$ref", default)]
    ref_type: Option<String>,
    #[serde(default)]
    optional: bool,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    items: Option<Box<ItemType>>,
    #[serde(rename = "enum", default)]
    enum_values: Vec<String>,
}

#[allow(dead_code)]
#[derive(serde::Deserialize, Clone)]
struct ItemType {
    #[serde(rename = "type", default)]
    type_kind: Option<String>,
    #[serde(rename = "$ref", default)]
    ref_type: Option<String>,
}

fn to_pascal_case(value: &str) -> String {
    let mut result = String::new();
    let mut capitalize = true;
    for ch in value.chars() {
        if ch == '_' || ch == '-' || ch == '.' {
            capitalize = true;
        } else if capitalize {
            result.push(ch.to_ascii_uppercase());
            capitalize = false;
        } else {
            result.push(ch);
        }
    }
    result
}

fn to_snake_case(value: &str) -> String {
    let mut result = String::new();
    let chars: Vec<char> = value.chars().collect();
    for (index, ch) in chars.iter().enumerate() {
        if ch.is_uppercase() && index > 0 {
            let prev_upper = chars[index - 1].is_uppercase();
            let next_lower = chars.get(index + 1).is_some_and(|next| next.is_lowercase());
            if !prev_upper || next_lower {
                result.push('_');
            }
        }
        result.push(ch.to_ascii_lowercase());
    }
    result
}

fn resolve_ref(
    reference: &str,
    current_domain: &str,
    domain_types: &std::collections::HashMap<String, HashSet<String>>,
) -> String {
    let parts: Vec<&str> = reference.split('.').collect();
    if parts.len() == 2 {
        let ref_domain = parts[0];
        let ref_type = parts[1];
        if ref_domain == current_domain {
            to_pascal_case(ref_type)
        } else if domain_types
            .get(ref_domain)
            .is_some_and(|types| types.contains(ref_type))
        {
            format!(
                "super::cdp_{}::{}",
                to_snake_case(ref_domain),
                to_pascal_case(ref_type)
            )
        } else {
            "serde_json::Value".to_string()
        }
    } else {
        to_pascal_case(reference)
    }
}

fn map_type_in_domain(
    property: &Property,
    current_domain: &str,
    domain_types: &std::collections::HashMap<String, HashSet<String>>,
) -> String {
    if let Some(reference) = &property.ref_type {
        let type_name = resolve_ref(reference, current_domain, domain_types);
        if property.optional {
            format!("Option<{}>", type_name)
        } else {
            type_name
        }
    } else if let Some(type_kind) = &property.type_kind {
        let base = match type_kind.as_str() {
            "string" => "String".to_string(),
            "integer" => "i64".to_string(),
            "number" => "f64".to_string(),
            "boolean" => "bool".to_string(),
            "object" | "any" => "serde_json::Value".to_string(),
            "array" => {
                if let Some(items) = &property.items {
                    let inner = if let Some(reference) = &items.ref_type {
                        resolve_ref(reference, current_domain, domain_types)
                    } else {
                        match items.type_kind.as_deref().unwrap_or("any") {
                            "string" => "String".to_string(),
                            "integer" => "i64".to_string(),
                            "number" => "f64".to_string(),
                            "boolean" => "bool".to_string(),
                            _ => "serde_json::Value".to_string(),
                        }
                    };
                    format!("Vec<{}>", inner)
                } else {
                    "Vec<serde_json::Value>".to_string()
                }
            }
            _ => "serde_json::Value".to_string(),
        };
        if property.optional {
            format!("Option<{}>", base)
        } else {
            base
        }
    } else if property.optional {
        "Option<serde_json::Value>".to_string()
    } else {
        "serde_json::Value".to_string()
    }
}

fn is_rust_keyword(value: &str) -> bool {
    matches!(
        value,
        "type"
            | "self"
            | "Self"
            | "super"
            | "move"
            | "ref"
            | "fn"
            | "mod"
            | "use"
            | "pub"
            | "let"
            | "mut"
            | "const"
            | "static"
            | "if"
            | "else"
            | "for"
            | "while"
            | "loop"
            | "match"
            | "return"
            | "break"
            | "continue"
            | "as"
            | "in"
            | "impl"
            | "trait"
            | "struct"
            | "enum"
            | "where"
            | "async"
            | "await"
            | "dyn"
            | "box"
            | "yield"
            | "override"
            | "crate"
            | "extern"
    )
}

fn generate_domain(
    domain: &Domain,
    domain_types: &std::collections::HashMap<String, HashSet<String>>,
    recursive_fields: &HashSet<(&str, &str, &str)>,
    output: &mut String,
) {
    let mod_name = to_snake_case(&domain.domain);
    output.push_str(&format!(
        "#[allow(dead_code, non_snake_case, non_camel_case_types, clippy::enum_variant_names)]\npub mod cdp_{} {{\n",
        mod_name
    ));
    output.push_str("    use super::*;\n\n");

    for type_def in &domain.types {
        if !type_def.enum_values.is_empty() {
            let mut seen_variants = HashSet::new();
            output.push_str("    #[derive(Debug, Clone, Serialize, Deserialize)]\n");
            output.push_str(&format!("    pub enum {} {{\n", type_def.id));
            for value in &type_def.enum_values {
                let mut variant = to_pascal_case(value);
                if variant == "Self" {
                    variant = "SelfValue".to_string();
                }
                if variant.chars().next().is_some_and(|ch| ch.is_ascii_digit()) {
                    variant = format!("V{}", variant);
                }
                if seen_variants.insert(variant.clone()) {
                    output.push_str(&format!(
                        "        #[serde(rename = \"{}\")]\n        {},\n",
                        value, variant
                    ));
                }
            }
            output.push_str("    }\n\n");
        } else if type_def.type_kind == "object" && !type_def.properties.is_empty() {
            output.push_str(
                "    #[derive(Debug, Clone, Serialize, Deserialize)]\n    #[serde(rename_all = \"camelCase\")]\n",
            );
            output.push_str(&format!("    pub struct {} {{\n", type_def.id));
            for property in &type_def.properties {
                let field_name = to_snake_case(&property.name);
                let field_name = if is_rust_keyword(&field_name) {
                    format!("r#{}", field_name)
                } else {
                    field_name
                };
                let mut rust_type = map_type_in_domain(property, &domain.domain, domain_types);

                if recursive_fields.contains(&(
                    domain.domain.as_str(),
                    type_def.id.as_str(),
                    property.name.as_str(),
                )) {
                    if rust_type.starts_with("Option<") {
                        let inner = &rust_type[7..rust_type.len() - 1];
                        rust_type = format!("Option<Box<{}>>", inner);
                    } else {
                        rust_type = format!("Box<{}>", rust_type);
                    }
                }

                if property.optional {
                    output
                        .push_str("        #[serde(skip_serializing_if = \"Option::is_none\")]\n");
                }
                output.push_str(&format!("        pub {}: {},\n", field_name, rust_type));
            }
            output.push_str("    }\n\n");
        } else if type_def.type_kind == "object" && type_def.properties.is_empty() {
            output.push_str(&format!(
                "    pub type {} = serde_json::Value;\n\n",
                type_def.id
            ));
        } else if type_def.type_kind == "array" {
            output.push_str(&format!(
                "    pub type {} = Vec<serde_json::Value>;\n\n",
                type_def.id
            ));
        } else if type_def.type_kind == "string" && type_def.enum_values.is_empty() {
            output.push_str(&format!("    pub type {} = String;\n\n", type_def.id));
        } else if type_def.type_kind == "integer" {
            output.push_str(&format!("    pub type {} = i64;\n\n", type_def.id));
        } else if type_def.type_kind == "number" {
            output.push_str(&format!("    pub type {} = f64;\n\n", type_def.id));
        }
    }

    for command in &domain.commands {
        let pascal_name = to_pascal_case(&command.name);

        if !command.parameters.is_empty() {
            output.push_str(
                "    #[derive(Debug, Clone, Serialize, Deserialize)]\n    #[serde(rename_all = \"camelCase\")]\n",
            );
            output.push_str(&format!("    pub struct {}Params {{\n", pascal_name));
            for parameter in &command.parameters {
                let field_name = to_snake_case(&parameter.name);
                let field_name = if is_rust_keyword(&field_name) {
                    format!("r#{}", field_name)
                } else {
                    field_name
                };
                let rust_type = map_type_in_domain(parameter, &domain.domain, domain_types);
                if parameter.optional {
                    output
                        .push_str("        #[serde(skip_serializing_if = \"Option::is_none\")]\n");
                }
                output.push_str(&format!("        pub {}: {},\n", field_name, rust_type));
            }
            output.push_str("    }\n\n");
        }

        if !command.returns.is_empty() {
            output.push_str(
                "    #[derive(Debug, Clone, Serialize, Deserialize)]\n    #[serde(rename_all = \"camelCase\")]\n",
            );
            output.push_str(&format!("    pub struct {}Result {{\n", pascal_name));
            for value in &command.returns {
                let field_name = to_snake_case(&value.name);
                let field_name = if is_rust_keyword(&field_name) {
                    format!("r#{}", field_name)
                } else {
                    field_name
                };
                let rust_type = map_type_in_domain(value, &domain.domain, domain_types);
                if value.optional {
                    output
                        .push_str("        #[serde(skip_serializing_if = \"Option::is_none\")]\n");
                }
                output.push_str(&format!("        pub {}: {},\n", field_name, rust_type));
            }
            output.push_str("    }\n\n");
        }
    }

    for event in &domain.events {
        if !event.parameters.is_empty() {
            let pascal_name = to_pascal_case(&event.name);
            output.push_str(
                "    #[derive(Debug, Clone, Serialize, Deserialize)]\n    #[serde(rename_all = \"camelCase\")]\n",
            );
            output.push_str(&format!("    pub struct {}Event {{\n", pascal_name));
            for parameter in &event.parameters {
                let field_name = to_snake_case(&parameter.name);
                let field_name = if is_rust_keyword(&field_name) {
                    format!("r#{}", field_name)
                } else {
                    field_name
                };
                let rust_type = map_type_in_domain(parameter, &domain.domain, domain_types);
                if parameter.optional {
                    output
                        .push_str("        #[serde(skip_serializing_if = \"Option::is_none\")]\n");
                }
                output.push_str(&format!("        pub {}: {},\n", field_name, rust_type));
            }
            output.push_str("    }\n\n");
        }
    }

    output.push_str("}\n\n");
}
