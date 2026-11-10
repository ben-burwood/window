//! YAML / TOML / JSON / XML parsed into one format-neutral tree, and written back out.
//!
//! Source text is the source of truth - tree is re-parsed from it on every edit.
//! Editing through the tree, or converting between formats, regenerates the text from the tree with [`serialize`].
//!
//! XML maps onto the tree with the usual "xmltodict" convention: an element is a table whose
//! `@name` entries are its attributes, `#text` its text, and other entries its child elements
//! (repeated children become an array). A text-only element with no attributes is a plain string.

use serde::Deserialize;
use std::collections::HashSet;
use std::path::Path;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Format {
    Yaml,
    Toml,
    Json,
    Xml,
}

impl Format {
    pub const ALL: [Format; 4] = [Format::Yaml, Format::Toml, Format::Json, Format::Xml];

    /// Every extension we open, for the file dialog filter.
    pub const EXTENSIONS: &[&str] = &["yaml", "yml", "toml", "json", "xml"];

    /// The format implied by the file extension, if it is one we know.
    pub fn from_path(path: &Path) -> Option<Self> {
        match window_core::extension_lower(path).as_deref() {
            Some("yaml" | "yml") => Some(Format::Yaml),
            Some("toml") => Some(Format::Toml),
            Some("json") => Some(Format::Json),
            Some("xml") => Some(Format::Xml),
            _ => None,
        }
    }

    /// Best guess for text with no telling extension.
    pub fn sniff(text: &str) -> Self {
        let trimmed = text.trim_start();
        if trimmed.is_empty() {
            Format::Yaml
        } else if trimmed.starts_with('<') {
            // Even if it doesn't parse yet: nothing else starts with a tag.
            Format::Xml
        } else if parse(text, Format::Json).is_ok() {
            // JSON is also valid YAML, so it has to be tried before falling back.
            Format::Json
        } else if parse(text, Format::Toml).is_ok() {
            Format::Toml
        } else {
            Format::Yaml
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Format::Yaml => "YAML",
            Format::Toml => "TOML",
            Format::Json => "JSON",
            Format::Xml => "XML",
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Format::Yaml => "yaml",
            Format::Toml => "toml",
            Format::Json => "json",
            Format::Xml => "xml",
        }
    }
}

/// One value in the document tree.
#[derive(Clone, Debug, PartialEq)]
pub enum Node {
    Null,
    Bool(bool),
    /// Integers and floats, kept as their textual form so big / special values round-trip exactly.
    Number(String),
    String(String),
    Datetime(String),
    /// A YAML `!tag value`.
    Tagged(String, Box<Node>),
    Seq(Vec<Node>),
    /// Entries in document order. YAML allows non-string keys; they are rendered to text.
    Map(Vec<(String, Node)>),
}

/// The kinds a node can be switched to from the tree's context menu.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    String,
    Number,
    Bool,
    Null,
    Array,
    Table,
}

impl Kind {
    pub const ALL: [Kind; 6] = [
        Kind::String,
        Kind::Number,
        Kind::Bool,
        Kind::Null,
        Kind::Array,
        Kind::Table,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Kind::String => "string",
            Kind::Number => "number",
            Kind::Bool => "bool",
            Kind::Null => "null",
            Kind::Array => "array",
            Kind::Table => "table",
        }
    }
}

impl Node {
    pub fn kind(&self) -> Kind {
        match self {
            Node::Null => Kind::Null,
            Node::Bool(_) => Kind::Bool,
            Node::Number(_) => Kind::Number,
            Node::String(_) | Node::Datetime(_) => Kind::String,
            Node::Tagged(_, inner) => inner.kind(),
            Node::Seq(_) => Kind::Array,
            Node::Map(_) => Kind::Table,
        }
    }

    /// Short type label shown in tooltips.
    pub fn kind_name(&self) -> &'static str {
        match self {
            Node::Datetime(_) => "datetime",
            other => other.kind().name(),
        }
    }

    /// Looks through a YAML tag to the value it wraps.
    pub fn untagged(&self) -> &Node {
        match self {
            Node::Tagged(_, inner) => inner.untagged(),
            other => other,
        }
    }

    fn untagged_mut(&mut self) -> &mut Node {
        match self {
            Node::Tagged(_, inner) => inner.untagged_mut(),
            other => other,
        }
    }

    pub fn is_container(&self) -> bool {
        matches!(self.untagged(), Node::Seq(_) | Node::Map(_))
    }

    /// Number of direct children of a container (0 for scalars).
    pub fn len(&self) -> usize {
        match self.untagged() {
            Node::Seq(items) => items.len(),
            Node::Map(entries) => entries.len(),
            _ => 0,
        }
    }

    /// The scalar rendered as it would appear in a config file; containers summarise themselves.
    pub fn display(&self) -> String {
        match self {
            Node::Null => "null".into(),
            Node::Bool(b) => b.to_string(),
            Node::Number(n) | Node::Datetime(n) => n.clone(),
            Node::String(s) => format!("{s:?}"),
            Node::Tagged(tag, inner) => format!("{tag} {}", inner.display()),
            Node::Seq(items) => format!("[{}]", items.len()),
            Node::Map(entries) => format!("{{{}}}", entries.len()),
        }
    }

    /// Plain text of a scalar for editing and copying (strings unquoted).
    pub fn text(&self) -> String {
        match self {
            Node::String(s) => s.clone(),
            Node::Tagged(_, inner) => inner.text(),
            other => other.display(),
        }
    }

    /// A new scalar of the same kind as `self` holding `text`. Strings stay strings; anything
    /// else is re-inferred, so typing `42` into a null makes a number.
    pub fn with_text(&self, text: &str) -> Node {
        match self {
            Node::String(_) => Node::String(text.to_owned()),
            Node::Datetime(_) => Node::Datetime(text.to_owned()),
            Node::Tagged(tag, inner) => Node::Tagged(tag.clone(), Box::new(inner.with_text(text))),
            _ => infer_scalar(text),
        }
    }

    /// This node converted to `kind`, keeping what it can (scalar text, container children).
    pub fn converted(&self, kind: Kind) -> Node {
        let node = self.untagged();
        match kind {
            Kind::String => Node::String(if node.is_container() {
                String::new()
            } else {
                node.text()
            }),
            Kind::Number => match node {
                Node::Number(_) => node.clone(),
                Node::Bool(b) => Node::Number(u8::from(*b).to_string()),
                _ if is_number(node.text().trim()) => Node::Number(node.text().trim().to_owned()),
                _ => Node::Number("0".into()),
            },
            Kind::Bool => Node::Bool(matches!(node.text().trim(), "true" | "1")),
            Kind::Null => Node::Null,
            Kind::Array => match node {
                Node::Seq(_) => node.clone(),
                Node::Map(entries) => Node::Seq(entries.iter().map(|(_, v)| v.clone()).collect()),
                Node::Null => Node::Seq(Vec::new()),
                scalar => Node::Seq(vec![scalar.clone()]),
            },
            Kind::Table => match node {
                Node::Map(_) => node.clone(),
                Node::Seq(items) => Node::Map(
                    items
                        .iter()
                        .enumerate()
                        .map(|(i, v)| (i.to_string(), v.clone()))
                        .collect(),
                ),
                _ => Node::Map(Vec::new()),
            },
        }
    }

    /// The node at `path` (child indices) below this one.
    pub fn at_mut(&mut self, path: &[usize]) -> Option<&mut Node> {
        let Some((&first, rest)) = path.split_first() else {
            return Some(self);
        };
        let child = match self.untagged_mut() {
            Node::Seq(items) => items.get_mut(first)?,
            Node::Map(entries) => &mut entries.get_mut(first)?.1,
            _ => return None,
        };
        child.at_mut(rest)
    }

    /// Rename the table entry at `index`. Refuses a key a sibling already has: every format
    /// would keep only one of the two, silently dropping the other's value.
    pub fn rename(&mut self, index: usize, key: String) -> Result<(), String> {
        let Node::Map(entries) = self.untagged_mut() else {
            return Ok(());
        };
        if entries
            .iter()
            .enumerate()
            .any(|(i, (k, _))| i != index && *k == key)
        {
            return Err(format!("There is already a key named {key:?}"));
        }
        if let Some((k, _)) = entries.get_mut(index) {
            *k = key;
        }
        Ok(())
    }

    pub fn remove(&mut self, index: usize) {
        match self.untagged_mut() {
            Node::Seq(items) if index < items.len() => {
                items.remove(index);
            }
            Node::Map(entries) if index < entries.len() => {
                entries.remove(index);
            }
            _ => {}
        }
    }

    /// Append an empty child: an empty string to an array, or `new_key = ""` (made unique) to a
    /// table. Returns the new child's index.
    pub fn push_child(&mut self) -> Option<usize> {
        match self.untagged_mut() {
            Node::Seq(items) => {
                let blank = items.last().map_or(Node::String(String::new()), blank_like);
                items.push(blank);
                Some(items.len() - 1)
            }
            Node::Map(entries) => {
                let mut key = "new_key".to_owned();
                let mut n = 2;
                while entries.iter().any(|(k, _)| *k == key) {
                    key = format!("new_key_{n}");
                    n += 1;
                }
                entries.push((key, Node::String(String::new())));
                Some(entries.len() - 1)
            }
            _ => None,
        }
    }
}

/// An empty value shaped like `node`, so appending to `[1, 2]` gives a number.
fn blank_like(node: &Node) -> Node {
    match node.untagged() {
        Node::Number(_) => Node::Number("0".into()),
        Node::Bool(_) => Node::Bool(false),
        Node::Seq(_) => Node::Seq(Vec::new()),
        Node::Map(_) => Node::Map(Vec::new()),
        _ => Node::String(String::new()),
    }
}

fn is_number(s: &str) -> bool {
    !s.is_empty()
        && (s.parse::<i64>().is_ok() || s.parse::<u64>().is_ok() || s.parse::<f64>().is_ok())
}

/// The scalar a bare word means: `null`, a bool, a number, or otherwise a string.
fn infer_scalar(text: &str) -> Node {
    let t = text.trim();
    match t {
        "null" | "~" => Node::Null,
        "true" => Node::Bool(true),
        "false" => Node::Bool(false),
        _ if is_number(t) && !t.eq_ignore_ascii_case("inf") && !t.eq_ignore_ascii_case("nan") => {
            Node::Number(t.to_owned())
        }
        _ => Node::String(text.to_owned()),
    }
}

/// Human-readable path to a node, e.g. `server.ports[0]` or `["dotted.key"]`.
pub fn path_string(docs: &[Node], path: &[usize]) -> String {
    let Some((&doc, rest)) = path.split_first() else {
        return String::new();
    };
    let mut out = if docs.len() > 1 {
        format!("${doc}")
    } else {
        String::new()
    };
    let mut node = &docs[doc];
    for &i in rest {
        match node.untagged() {
            Node::Seq(items) => {
                out.push_str(&format!("[{i}]"));
                node = &items[i];
            }
            Node::Map(entries) => {
                let (key, child) = &entries[i];
                let plain = !key.is_empty()
                    && key
                        .chars()
                        .all(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '@' | '#'));
                if !plain {
                    out.push_str(&format!("[{key:?}]"));
                } else if out.is_empty() {
                    out.push_str(key);
                } else {
                    out.push_str(&format!(".{key}"));
                }
                node = child;
            }
            _ => break,
        }
    }
    out
}

/// Paths of every node the tree filter should show for `needle` (already lowercased): those
/// whose key or scalar value contains it, plus all their ancestors. Each document's own path
/// (`[doc]`) is included when anything in it matches.
pub fn matching_paths(docs: &[Node], needle: &str) -> HashSet<Vec<usize>> {
    fn walk(
        node: &Node,
        key: Option<&str>,
        needle: &str,
        path: &mut Vec<usize>,
        out: &mut HashSet<Vec<usize>>,
    ) -> bool {
        let mut hit = key.is_some_and(|k| k.to_lowercase().contains(needle));
        match node.untagged() {
            Node::Seq(items) => {
                for (i, child) in items.iter().enumerate() {
                    path.push(i);
                    hit |= walk(child, None, needle, path, out);
                    path.pop();
                }
            }
            Node::Map(entries) => {
                for (i, (k, child)) in entries.iter().enumerate() {
                    path.push(i);
                    hit |= walk(child, Some(k), needle, path, out);
                    path.pop();
                }
            }
            scalar => hit |= scalar.text().to_lowercase().contains(needle),
        }
        if let Node::Tagged(tag, _) = node {
            hit |= tag.to_lowercase().contains(needle);
        }
        if hit {
            out.insert(path.clone());
        }
        hit
    }

    let mut out = HashSet::new();
    for (i, doc) in docs.iter().enumerate() {
        walk(doc, None, needle, &mut vec![i], &mut out);
    }
    out
}

// --- Parsing ---------------------------------------------------------------------------------

/// Parse `text` into one tree per document (YAML may have several, separated by `---`; the
/// others always have exactly one). The error includes the line / column the parser reports.
pub fn parse(text: &str, format: Format) -> Result<Vec<Node>, String> {
    // A blank buffer is "nothing yet", not a parse error / null document.
    if text.trim().is_empty() {
        return Ok(Vec::new());
    }
    match format {
        Format::Toml => {
            let table: toml::Table = toml::from_str(text).map_err(|e| e.to_string())?;
            Ok(vec![from_toml(toml::Value::Table(table))])
        }
        Format::Yaml => {
            let mut docs = Vec::new();
            for doc in serde_yaml_ng::Deserializer::from_str(text) {
                let value = serde_yaml_ng::Value::deserialize(doc).map_err(|e| e.to_string())?;
                docs.push(from_yaml(value));
            }
            Ok(docs)
        }
        Format::Json => {
            let value: serde_json::Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
            Ok(vec![from_json(value)])
        }
        Format::Xml => {
            let options = roxmltree::ParsingOptions {
                allow_dtd: true,
                ..Default::default()
            };
            let doc = roxmltree::Document::parse_with_options(text, options)
                .map_err(|e| e.to_string())?;
            let root = doc.root_element();
            Ok(vec![Node::Map(vec![(xml_name(root), from_xml(root))])])
        }
    }
}

fn from_toml(value: toml::Value) -> Node {
    match value {
        toml::Value::String(s) => Node::String(s),
        toml::Value::Integer(i) => Node::Number(i.to_string()),
        toml::Value::Float(f) => Node::Number(f.to_string()),
        toml::Value::Boolean(b) => Node::Bool(b),
        toml::Value::Datetime(d) => Node::Datetime(d.to_string()),
        toml::Value::Array(items) => Node::Seq(items.into_iter().map(from_toml).collect()),
        toml::Value::Table(table) => {
            Node::Map(table.into_iter().map(|(k, v)| (k, from_toml(v))).collect())
        }
    }
}

fn from_yaml(value: serde_yaml_ng::Value) -> Node {
    use serde_yaml_ng::Value;
    match value {
        Value::Null => Node::Null,
        Value::Bool(b) => Node::Bool(b),
        Value::Number(n) => Node::Number(n.to_string()),
        Value::String(s) => Node::String(s),
        Value::Sequence(items) => Node::Seq(items.into_iter().map(from_yaml).collect()),
        Value::Mapping(map) => Node::Map(
            map.into_iter()
                .map(|(k, v)| (yaml_key(k), from_yaml(v)))
                .collect(),
        ),
        Value::Tagged(tagged) => {
            let tag = tagged.tag.to_string();
            // The non-specific tag (`! 123`) means "a string, don't resolve this scalar", but
            // serde_yaml_ng resolves it anyway — undo that rather than keep an empty tag.
            if tag.trim_start_matches('!').is_empty() {
                match from_yaml(tagged.value) {
                    scalar @ (Node::Number(_) | Node::Bool(_)) => Node::String(scalar.text()),
                    other => other,
                }
            } else {
                Node::Tagged(tag, Box::new(from_yaml(tagged.value)))
            }
        }
    }
}

/// Render a YAML mapping key as text: strings bare, anything else in its scalar / flow form.
fn yaml_key(key: serde_yaml_ng::Value) -> String {
    match from_yaml(key) {
        Node::String(s) => s,
        other => other.display(),
    }
}

fn from_json(value: serde_json::Value) -> Node {
    use serde_json::Value;
    match value {
        Value::Null => Node::Null,
        Value::Bool(b) => Node::Bool(b),
        Value::Number(n) => Node::Number(n.to_string()),
        Value::String(s) => Node::String(s),
        Value::Array(items) => Node::Seq(items.into_iter().map(from_json).collect()),
        Value::Object(map) => Node::Map(map.into_iter().map(|(k, v)| (k, from_json(v))).collect()),
    }
}

/// An element or attribute name with its namespace prefix, as written in the source.
fn xml_qualified(node: roxmltree::Node, namespace: Option<&str>, local: &str) -> String {
    match namespace.and_then(|ns| node.lookup_prefix(ns)) {
        Some(prefix) if !prefix.is_empty() => format!("{prefix}:{local}"),
        _ => local.to_owned(),
    }
}

fn xml_name(el: roxmltree::Node) -> String {
    let tag = el.tag_name();
    xml_qualified(el, tag.namespace(), tag.name())
}

fn from_xml(el: roxmltree::Node) -> Node {
    let mut entries: Vec<(String, Node)> = Vec::new();

    // Namespace declarations made on this element (not inherited ones) round-trip as attributes.
    let inherited: Vec<_> = el
        .parent_element()
        .map(|p| p.namespaces().map(|ns| (ns.name(), ns.uri())).collect())
        .unwrap_or_default();
    for ns in el.namespaces() {
        if ns.name() == Some("xml") || inherited.contains(&(ns.name(), ns.uri())) {
            continue;
        }
        let key = match ns.name() {
            Some(prefix) => format!("@xmlns:{prefix}"),
            None => "@xmlns".to_owned(),
        };
        entries.push((key, Node::String(ns.uri().to_owned())));
    }
    for attr in el.attributes() {
        let name = xml_qualified(el, attr.namespace(), attr.name());
        entries.push((format!("@{name}"), Node::String(attr.value().to_owned())));
    }

    let mut text = String::new();
    for child in el.children() {
        if child.is_element() {
            let name = xml_name(child);
            let value = from_xml(child);
            match entries.iter_mut().find(|(k, _)| *k == name) {
                Some((_, Node::Seq(items))) => items.push(value),
                Some((_, existing)) => {
                    let first = std::mem::replace(existing, Node::Null);
                    *existing = Node::Seq(vec![first, value]);
                }
                None => entries.push((name, value)),
            }
        } else if child.is_text() {
            let t = child.text().unwrap_or_default().trim();
            if !t.is_empty() {
                if !text.is_empty() {
                    text.push(' ');
                }
                text.push_str(t);
            }
        }
    }

    match (entries.is_empty(), text.is_empty()) {
        (true, true) => Node::Null,
        (true, false) => Node::String(text),
        (false, empty) => {
            if !empty {
                entries.push(("#text".into(), Node::String(text)));
            }
            Node::Map(entries)
        }
    }
}

// --- Serialising -----------------------------------------------------------------------------

/// Write the tree back out as `format`. Fails when the tree uses something the format can't
/// hold (e.g. `null` in TOML, or several documents in anything but YAML).
pub fn serialize(docs: &[Node], format: Format) -> Result<String, String> {
    if format != Format::Yaml && docs.len() > 1 {
        return Err(format!(
            "{} holds a single document; this file has {}",
            format.name(),
            docs.len()
        ));
    }
    let empty = Node::Map(Vec::new());
    let first = docs.first().unwrap_or(&empty);
    match format {
        Format::Yaml => {
            let mut out = String::new();
            for (i, doc) in docs.iter().enumerate() {
                if i > 0 {
                    out.push_str("---\n");
                }
                out.push_str(&serde_yaml_ng::to_string(&to_yaml(doc)).map_err(|e| e.to_string())?);
            }
            Ok(out)
        }
        Format::Toml => {
            let toml::Value::Table(table) = to_toml(first, "")? else {
                return Err("A TOML document must be a table at the top level".into());
            };
            toml::to_string_pretty(&table).map_err(|e| e.to_string())
        }
        Format::Json => {
            let mut out =
                serde_json::to_string_pretty(&to_json(first)?).map_err(|e| e.to_string())?;
            out.push('\n');
            Ok(out)
        }
        Format::Xml => {
            let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
            match first.untagged() {
                // A single top-level key is the root element.
                Node::Map(entries) if entries.len() == 1 && !entries[0].1.untagged().is_seq() => {
                    write_xml(&mut out, &entries[0].0, &entries[0].1, 0);
                }
                other => write_xml(&mut out, "root", other, 0),
            }
            Ok(out)
        }
    }
}

impl Node {
    fn is_seq(&self) -> bool {
        matches!(self, Node::Seq(_))
    }
}

fn to_yaml(node: &Node) -> serde_yaml_ng::Value {
    use serde_yaml_ng::Value;
    match node {
        Node::Null => Value::Null,
        Node::Bool(b) => Value::Bool(*b),
        Node::Number(n) => serde_yaml_ng::from_str::<Value>(n)
            .ok()
            .filter(Value::is_number)
            .unwrap_or_else(|| Value::String(n.clone())),
        Node::String(s) | Node::Datetime(s) => Value::String(s.clone()),
        // `Tag::new` panics on an empty tag, and takes the leading `!` as-is (stripping it would
        // turn `!!foo` into `!foo`).
        Node::Tagged(tag, inner) if tag.trim_start_matches('!').is_empty() => to_yaml(inner),
        Node::Tagged(tag, inner) => Value::Tagged(Box::new(serde_yaml_ng::value::TaggedValue {
            tag: serde_yaml_ng::value::Tag::new(tag.clone()),
            value: to_yaml(inner),
        })),
        Node::Seq(items) => Value::Sequence(items.iter().map(to_yaml).collect()),
        Node::Map(entries) => Value::Mapping(
            entries
                .iter()
                .map(|(k, v)| (Value::String(k.clone()), to_yaml(v)))
                .collect(),
        ),
    }
}

fn to_toml(node: &Node, path: &str) -> Result<toml::Value, String> {
    Ok(match node {
        Node::Null => {
            let at = if path.is_empty() { "the root" } else { path };
            return Err(format!("TOML has no null value (at {at})"));
        }
        Node::Bool(b) => toml::Value::Boolean(*b),
        Node::Number(n) => match (n.parse::<i64>(), n.parse::<f64>()) {
            (Ok(i), _) => toml::Value::Integer(i),
            // Falling through to a float would silently round it.
            _ if n.parse::<i128>().is_ok() => {
                return Err(format!(
                    "{n} is too large for a TOML integer (at {})",
                    if path.is_empty() { "the root" } else { path }
                ));
            }
            (_, Ok(f)) => toml::Value::Float(f),
            _ => toml::Value::String(n.clone()),
        },
        Node::String(s) => toml::Value::String(s.clone()),
        Node::Datetime(d) => d
            .parse()
            .map(toml::Value::Datetime)
            .unwrap_or_else(|_| toml::Value::String(d.clone())),
        Node::Tagged(_, inner) => to_toml(inner, path)?,
        Node::Seq(items) => toml::Value::Array(
            items
                .iter()
                .enumerate()
                .map(|(i, v)| to_toml(v, &format!("{path}[{i}]")))
                .collect::<Result<_, _>>()?,
        ),
        Node::Map(entries) => {
            let mut table = toml::Table::new();
            for (k, v) in entries {
                let child = if path.is_empty() {
                    k.clone()
                } else {
                    format!("{path}.{k}")
                };
                table.insert(k.clone(), to_toml(v, &child)?);
            }
            toml::Value::Table(table)
        }
    })
}

fn to_json(node: &Node) -> Result<serde_json::Value, String> {
    use serde_json::Value;
    Ok(match node {
        Node::Null => Value::Null,
        Node::Bool(b) => Value::Bool(*b),
        Node::Number(n) => serde_json::from_str::<Value>(n)
            .ok()
            .filter(Value::is_number)
            .or_else(|| {
                n.parse::<f64>()
                    .ok()
                    .and_then(serde_json::Number::from_f64)
                    .map(Value::Number)
            })
            .unwrap_or_else(|| Value::String(n.clone())),
        Node::String(s) | Node::Datetime(s) => Value::String(s.clone()),
        Node::Tagged(_, inner) => to_json(inner)?,
        Node::Seq(items) => Value::Array(items.iter().map(to_json).collect::<Result<_, _>>()?),
        Node::Map(entries) => Value::Object(
            entries
                .iter()
                .map(|(k, v)| Ok((k.clone(), to_json(v)?)))
                .collect::<Result<_, String>>()?,
        ),
    })
}

/// Make `name` a legal XML name by replacing anything that isn't.
fn xml_safe_name(name: &str) -> String {
    let mut out: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, '_' | '-' | '.' | ':') {
                c
            } else {
                '_'
            }
        })
        .collect();
    if !out.starts_with(|c: char| c.is_alphabetic() || c == '_') {
        out.insert(0, '_');
    }
    out
}

fn xml_escape(s: &str, attr: bool) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' if attr => out.push_str("&quot;"),
            c => out.push(c),
        }
    }
    out
}

fn write_xml(out: &mut String, name: &str, node: &Node, depth: usize) {
    let indent = "  ".repeat(depth);
    let name = xml_safe_name(name);
    match node.untagged() {
        // Repeated elements.
        Node::Seq(items) => {
            for item in items {
                write_xml(out, &name, item, depth);
            }
        }
        Node::Map(entries) => {
            out.push_str(&format!("{indent}<{name}"));
            let mut text = None;
            let mut children = Vec::new();
            for (k, v) in entries {
                if let Some(attr) = k.strip_prefix('@').filter(|_| !v.is_container()) {
                    let value = xml_escape(&v.text(), true);
                    out.push_str(&format!(" {}=\"{value}\"", xml_safe_name(attr)));
                } else if k == "#text" && !v.is_container() {
                    text = Some(xml_escape(&v.text(), false));
                } else {
                    children.push((k, v));
                }
            }
            match (children.is_empty(), text) {
                (true, None) => out.push_str("/>\n"),
                (true, Some(text)) => out.push_str(&format!(">{text}</{name}>\n")),
                (false, text) => {
                    out.push_str(">\n");
                    if let Some(text) = text {
                        out.push_str(&format!("{indent}  {text}\n"));
                    }
                    for (k, v) in children {
                        write_xml(out, k, v, depth + 1);
                    }
                    out.push_str(&format!("{indent}</{name}>\n"));
                }
            }
        }
        Node::Null => out.push_str(&format!("{indent}<{name}/>\n")),
        scalar => {
            let text = xml_escape(&scalar.text(), false);
            out.push_str(&format!("{indent}<{name}>{text}</{name}>\n"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(docs: &[Node]) -> &[(String, Node)] {
        let Node::Map(entries) = &docs[0] else {
            panic!("not a map: {docs:?}")
        };
        entries
    }

    #[test]
    fn toml_keeps_document_order() {
        let docs = parse(
            "zeta = 1\nalpha = \"a\"\n[table]\nkey = true\n",
            Format::Toml,
        )
        .unwrap();
        let keys: Vec<_> = map(&docs).iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(keys, ["zeta", "alpha", "table"]);
        assert_eq!(map(&docs)[0].1, Node::Number("1".into()));
    }

    #[test]
    fn toml_datetime_round_trips() {
        let text = "when = 1979-05-27T07:32:00Z\n";
        let docs = parse(text, Format::Toml).unwrap();
        assert_eq!(
            map(&docs)[0].1,
            Node::Datetime("1979-05-27T07:32:00Z".into())
        );
        assert_eq!(serialize(&docs, Format::Toml).unwrap(), text);
    }

    #[test]
    fn yaml_multi_document_and_tags() {
        let docs = parse("a: 1\n---\n- !env HOME\n- null\n", Format::Yaml).unwrap();
        assert_eq!(docs.len(), 2);
        assert_eq!(
            docs[1],
            Node::Seq(vec![
                Node::Tagged("!env".into(), Box::new(Node::String("HOME".into()))),
                Node::Null,
            ])
        );
        let again = parse(&serialize(&docs, Format::Yaml).unwrap(), Format::Yaml).unwrap();
        assert_eq!(again, docs);
    }

    #[test]
    fn yaml_non_string_keys() {
        let docs = parse("1: one\ntrue: yes\n", Format::Yaml).unwrap();
        assert_eq!(map(&docs)[0].0, "1");
        assert_eq!(map(&docs)[1].0, "true");
    }

    #[test]
    fn json_keeps_order_and_numbers() {
        let docs = parse(r#"{"b": 1.5, "a": [1, null, "x"]}"#, Format::Json).unwrap();
        assert_eq!(map(&docs)[0], ("b".into(), Node::Number("1.5".into())));
        let out = serialize(&docs, Format::Json).unwrap();
        assert!(out.starts_with("{\n  \"b\": 1.5,"), "{out}");
    }

    #[test]
    fn xml_attributes_text_and_repeats() {
        let text = r#"<cfg version="2"><server>a</server><server>b</server><note lang="en">hi</note><empty/></cfg>"#;
        let docs = parse(text, Format::Xml).unwrap();
        let Node::Map(cfg) = &map(&docs)[0].1 else {
            panic!()
        };
        assert_eq!(cfg[0], ("@version".into(), Node::String("2".into())));
        assert_eq!(
            cfg[1],
            (
                "server".into(),
                Node::Seq(vec![Node::String("a".into()), Node::String("b".into())])
            )
        );
        assert_eq!(
            cfg[2].1,
            Node::Map(vec![
                ("@lang".into(), Node::String("en".into())),
                ("#text".into(), Node::String("hi".into())),
            ])
        );
        assert_eq!(cfg[3].1, Node::Null);

        let again = parse(&serialize(&docs, Format::Xml).unwrap(), Format::Xml).unwrap();
        assert_eq!(again, docs);
    }

    #[test]
    fn xml_namespaces_keep_prefixes() {
        let text = r#"<a:root xmlns:a="urn:a"><a:item a:id="1"/></a:root>"#;
        let docs = parse(text, Format::Xml).unwrap();
        assert_eq!(map(&docs)[0].0, "a:root");
        let again = parse(&serialize(&docs, Format::Xml).unwrap(), Format::Xml).unwrap();
        assert_eq!(again, docs);
    }

    #[test]
    fn convert_yaml_to_toml_and_back() {
        let docs = parse(
            "name: app\nports: [80, 443]\ndb:\n  host: x\n",
            Format::Yaml,
        )
        .unwrap();
        let toml = serialize(&docs, Format::Toml).unwrap();
        assert_eq!(parse(&toml, Format::Toml).unwrap(), docs);
    }

    #[test]
    fn toml_rejects_null_with_path() {
        let docs = parse("a:\n  b: null\n", Format::Yaml).unwrap();
        let err = serialize(&docs, Format::Toml).unwrap_err();
        assert!(err.contains("a.b"), "{err}");
    }

    #[test]
    fn errors_report_location() {
        let err = parse("a = \n", Format::Toml).unwrap_err();
        assert!(err.contains("line 1"), "{err}");
        let err = parse("a: [1, 2\n", Format::Yaml).unwrap_err();
        assert!(err.contains("line"), "{err}");
        let err = parse("{\"a\": }", Format::Json).unwrap_err();
        assert!(err.contains("line 1"), "{err}");
        assert!(parse("<a><b></a>", Format::Xml).is_err());
    }

    #[test]
    fn filter_matches_keys_values_and_ancestors() {
        let docs = parse("server:\n  host: Example.COM\n  port: 80\n", Format::Yaml).unwrap();
        let hits = matching_paths(&docs, "example");
        let expected: HashSet<Vec<usize>> = [vec![0], vec![0, 0], vec![0, 0, 0]].into();
        assert_eq!(hits, expected);
        assert!(matching_paths(&docs, "port").contains(&vec![0, 0, 1]));
        assert!(matching_paths(&docs, "nope").is_empty());
    }

    #[test]
    fn bare_yaml_tag_does_not_panic() {
        let docs = parse("a: ! 123\n", Format::Yaml).unwrap();
        assert_eq!(map(&docs)[0].1, Node::String("123".into()));
        assert_eq!(serialize(&docs, Format::Yaml).unwrap(), "a: '123'\n");
        // And should a tree ever hold one, writing it out still mustn't panic.
        let odd = vec![Node::Tagged("!".into(), Box::new(Node::Null))];
        assert_eq!(serialize(&odd, Format::Yaml).unwrap(), "null\n");
    }

    #[test]
    fn rename_refuses_duplicate_keys() {
        let mut docs = parse("a: 1\nb: 2\n", Format::Yaml).unwrap();
        assert!(docs[0].rename(0, "b".into()).is_err());
        assert!(docs[0].rename(0, "a".into()).is_ok());
        assert_eq!(map(&docs).len(), 2);
    }

    #[test]
    fn toml_rejects_integers_it_cannot_hold() {
        let docs = parse("{\"big\": 18446744073709551615}", Format::Json).unwrap();
        let err = serialize(&docs, Format::Toml).unwrap_err();
        assert!(err.contains("too large") && err.contains("big"), "{err}");
    }

    #[test]
    fn tree_edits() {
        let mut docs = parse("a: [1, 2]\nb: x\n", Format::Yaml).unwrap();
        let root = &mut docs[0];
        assert_eq!(root.at_mut(&[0]).unwrap().push_child(), Some(2));
        assert_eq!(*root.at_mut(&[0, 2]).unwrap(), Node::Number("0".into()));
        root.rename(1, "c".into()).unwrap();
        let b = root.at_mut(&[1]).unwrap();
        *b = b.with_text("42");
        assert_eq!(*b, Node::String("42".into()));
        root.remove(0);
        assert_eq!(serialize(&docs, Format::Yaml).unwrap(), "c: '42'\n");
    }

    #[test]
    fn path_strings() {
        let docs = parse("a:\n  - x\n  - {\"b.c\": 1}\n", Format::Yaml).unwrap();
        assert_eq!(path_string(&docs, &[0, 0, 1, 0]), "a[1][\"b.c\"]");
    }

    #[test]
    fn sniffing() {
        assert_eq!(Format::sniff("  <?xml version='1.0'?><a/>"), Format::Xml);
        assert_eq!(Format::sniff("{\"a\": 1}"), Format::Json);
        assert_eq!(Format::sniff("[1, 2]"), Format::Json);
        assert_eq!(Format::sniff("{a: 1}"), Format::Yaml);
        assert_eq!(Format::sniff("[package]\nname = \"x\"\n"), Format::Toml);
        assert_eq!(Format::sniff("a: 1\n"), Format::Yaml);
    }

    #[test]
    fn blank_text_has_no_documents() {
        for format in Format::ALL {
            assert_eq!(parse(" \n", format).unwrap(), vec![]);
        }
    }
}
