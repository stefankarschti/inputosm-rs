use crate::{inputosm::require, *};
use std::{fs::File, io::BufReader, path::Path};
use xml::reader::{ParserConfig, XmlEvent};

#[derive(Default)]
struct Entity {
    kind: u8,
    id: i64,
    lat: i64,
    lon: i64,
    metadata: Metadata,
    tags: Vec<(String, String)>,
    refs: Vec<i64>,
    members: Vec<(MemberType, i64, String)>,
}
fn qualified_name(name: &xml::name::OwnedName) -> Vec<u8> {
    match &name.prefix {
        Some(prefix) => format!("{prefix}:{}", name.local_name).into_bytes(),
        None => name.local_name.as_bytes().to_vec(),
    }
}
fn attributes(values: Vec<xml::attribute::OwnedAttribute>) -> Result<Vec<(Vec<u8>, String)>> {
    let mut result: Vec<(Vec<u8>, String)> = Vec::with_capacity(values.len());
    for value in values {
        let key = qualified_name(&value.name);
        require(
            !result.iter().any(|(k, _)| k == &key),
            "Duplicate XML attribute",
        )?;
        result.push((key, value.value));
    }
    Ok(result)
}
fn attr<'a>(attrs: &'a [(Vec<u8>, String)], key: &[u8]) -> Option<&'a str> {
    attrs
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
}
fn integer(text: &str) -> Result<i64> {
    text.parse()
        .map_err(|_| Error::Invalid("Invalid XML integer"))
}
fn required<'a>(attrs: &'a [(Vec<u8>, String)], key: &[u8]) -> Result<&'a str> {
    attr(attrs, key).ok_or(Error::Invalid("Missing XML attribute"))
}
fn coordinate(text: &str, limit: i64) -> Result<i64> {
    let (negative, text) = if let Some(s) = text.strip_prefix('-') {
        (true, s)
    } else {
        (false, text.strip_prefix('+').unwrap_or(text))
    };
    let (whole, fraction) = text.split_once('.').unwrap_or((text, ""));
    require(
        !whole.is_empty()
            && whole.bytes().all(|c| c.is_ascii_digit())
            && fraction.bytes().all(|c| c.is_ascii_digit()),
        "Invalid XML coordinate",
    )?;
    let whole = integer(whole)?;
    require(
        whole <= limit && (whole < limit || fraction.bytes().all(|c| c == b'0')),
        "XML coordinate out of range",
    )?;
    let mut value = 0;
    for i in 0..7 {
        value = value * 10
            + fraction
                .as_bytes()
                .get(i)
                .map_or(0, |c| i64::from(c - b'0'));
    }
    value += whole * 10_000_000;
    Ok(if negative { -value } else { value })
}
impl Entity {
    fn start(
        kind: u8,
        attrs: &[(Vec<u8>, String)],
        options: InputOptions,
        mode: Mode,
    ) -> Result<Self> {
        let mut entity = Self {
            kind,
            id: integer(required(attrs, b"id")?)?,
            ..Self::default()
        };
        if kind == 1 {
            if let Some(lat) = attr(attrs, b"lat") {
                entity.lat = coordinate(lat, 90)?;
            } else {
                require(
                    mode == Mode::Delete || attr(attrs, b"visible") == Some("false"),
                    "Missing node latitude",
                )?;
            }
            if let Some(lon) = attr(attrs, b"lon") {
                entity.lon = coordinate(lon, 180)?;
            } else {
                require(
                    mode == Mode::Delete || attr(attrs, b"visible") == Some("false"),
                    "Missing node longitude",
                )?;
            }
        }
        if options.decode_metadata {
            if let Some(v) = attr(attrs, b"version") {
                entity.metadata.version = i32::try_from(integer(v)?)
                    .map_err(|_| Error::Invalid("XML version overflow"))?;
                entity.metadata.present |= Metadata::VERSION;
            }
            if let Some(v) = attr(attrs, b"changeset") {
                entity.metadata.changeset = integer(v)?;
                entity.metadata.present |= Metadata::CHANGESET;
            }
            if let Some(v) = attr(attrs, b"timestamp") {
                entity.metadata.raw_timestamp = crate::timeutil::parse_timestamp(v)?;
                entity.metadata.present |= Metadata::TIMESTAMP;
            }
            if let Some(v) = attr(attrs, b"uid") {
                entity.metadata.uid = i32::try_from(integer(v)?)
                    .map_err(|_| Error::Invalid("XML user ID overflow"))?;
                entity.metadata.present |= Metadata::UID;
            }
            if let Some(v) = attr(attrs, b"visible") {
                require(v == "true" || v == "false", "Invalid XML visibility")?;
                entity.metadata.visible = v == "true";
                entity.metadata.present |= Metadata::VISIBLE;
            }
        }
        Ok(entity)
    }
    fn emit(&self, context: Context, handler: &impl EntityHandler) -> Result<()> {
        let tags: Vec<_> = self
            .tags
            .iter()
            .map(|(k, v)| Tag {
                key: k.as_bytes(),
                value: v.as_bytes(),
            })
            .collect();
        match self.kind {
            1 => handler.nodes(
                context,
                &[Node {
                    id: self.id,
                    raw_latitude: self.lat,
                    raw_longitude: self.lon,
                    tags: &tags,
                    metadata: self.metadata,
                }],
            ),
            2 => handler.ways(
                context,
                &[Way {
                    id: self.id,
                    node_refs: &self.refs,
                    tags: &tags,
                    metadata: self.metadata,
                }],
            ),
            3 => {
                let members: Vec<_> = self
                    .members
                    .iter()
                    .map(|(member_type, id, role)| RelationMember {
                        member_type: *member_type,
                        id: *id,
                        role: role.as_bytes(),
                    })
                    .collect();
                handler.relations(
                    context,
                    &[Relation {
                        id: self.id,
                        members: &members,
                        tags: &tags,
                        metadata: self.metadata,
                    }],
                )
            }
            _ => unreachable!(),
        }
    }
}
pub(crate) fn input_xml(
    path: &Path,
    options: InputOptions,
    handler: &impl EntityHandler,
) -> Result<()> {
    let mut reader = ParserConfig::new()
        .max_attribute_length(32 * 1024 * 1024)
        .max_data_length(32 * 1024 * 1024)
        .max_attributes(1024)
        .max_name_length(1024)
        .max_entity_expansion_length(1024)
        .max_entity_expansion_depth(0)
        .create_reader(BufReader::new(File::open(path)?));
    let mut stack: Vec<Vec<u8>> = Vec::new();
    let mut root_seen = false;
    let mut mode = Mode::Bulk;
    let mut entity = None;
    loop {
        let event = reader
            .next()
            .map_err(|e| Error::Message(format!("XML: {e}")))?;
        match event {
            XmlEvent::StartElement {
                name,
                attributes: attrs,
                ..
            } => {
                let name = qualified_name(&name);
                let name = name.as_slice();
                require(stack.len() < 64, "XML nesting limit exceeded")?;
                let attrs = attributes(attrs)?;
                if stack.is_empty() {
                    require(
                        !root_seen && (name == b"osm" || name == b"osmChange"),
                        "Invalid XML root",
                    )?;
                    root_seen = true;
                } else {
                    match name {
                        b"create" | b"modify" | b"delete" => {
                            require(
                                stack.len() == 1 && stack[0] == b"osmChange",
                                "Invalid OSC operation location",
                            )?;
                            mode = match name {
                                b"create" => Mode::Create,
                                b"modify" => Mode::Modify,
                                _ => Mode::Delete,
                            };
                        }
                        b"node" | b"way" | b"relation" => {
                            require(
                                entity.is_none()
                                    && ((stack.len() == 1 && stack[0] == b"osm")
                                        || (stack.len() == 2 && mode != Mode::Bulk)),
                                "Invalid XML entity location",
                            )?;
                            entity = Some(Entity::start(
                                match name {
                                    b"node" => 1,
                                    b"way" => 2,
                                    _ => 3,
                                },
                                &attrs,
                                options,
                                mode,
                            )?);
                        }
                        b"tag" | b"nd" | b"member" => {
                            let parent = stack.last().unwrap().as_slice();
                            require(
                                matches!(parent, b"node" | b"way" | b"relation"),
                                "Invalid XML child location",
                            )?;
                            let e = entity
                                .as_mut()
                                .ok_or(Error::Invalid("XML child without entity"))?;
                            match name {
                                b"tag" => e.tags.push((
                                    required(&attrs, b"k")?.to_owned(),
                                    required(&attrs, b"v")?.to_owned(),
                                )),
                                b"nd" => {
                                    require(e.kind == 2, "Node reference outside way")?;
                                    e.refs.push(integer(required(&attrs, b"ref")?)?);
                                }
                                _ => {
                                    require(e.kind == 3, "Member outside relation")?;
                                    let kind = match required(&attrs, b"type")? {
                                        "node" => MemberType::Node,
                                        "way" => MemberType::Way,
                                        "relation" => MemberType::Relation,
                                        _ => return Err(Error::Invalid("Invalid XML member type")),
                                    };
                                    e.members.push((
                                        kind,
                                        integer(required(&attrs, b"ref")?)?,
                                        attr(&attrs, b"role").unwrap_or("").to_owned(),
                                    ));
                                }
                            }
                        }
                        _ => {}
                    }
                }
                stack.push(name.to_vec());
            }
            XmlEvent::EndElement { name } => {
                let name = qualified_name(&name);
                let name = name.as_slice();
                require(
                    stack.pop().is_some_and(|s| s == name),
                    "Mismatched XML end tag",
                )?;
                match name {
                    b"node" | b"way" | b"relation" => {
                        entity
                            .take()
                            .ok_or(Error::Invalid("Missing XML entity"))?
                            .emit(
                                Context {
                                    file_type: FileType::Xml,
                                    mode,
                                    block_index: 0,
                                    thread_index: 0,
                                },
                                handler,
                            )?;
                    }
                    b"create" | b"modify" | b"delete" => mode = Mode::Bulk,
                    _ => {}
                }
            }
            XmlEvent::Doctype { .. } => {
                return Err(Error::Invalid("XML document types are unsupported"));
            }
            XmlEvent::Characters(t) | XmlEvent::CData(t) => require(
                t.bytes().all(|b| b.is_ascii_whitespace()),
                "Unexpected XML text",
            )?,
            XmlEvent::EndDocument => {
                require(
                    root_seen && stack.is_empty() && entity.is_none(),
                    "Incomplete XML document",
                )?;
                return Ok(());
            }
            _ => {}
        }
    }
}
