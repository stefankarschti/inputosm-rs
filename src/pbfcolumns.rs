use crate::{inputosm::require, inputosmpbf::wire::*, *};

#[derive(Default)]
pub(crate) struct Columns {
    pub ids: Vec<i64>,
    pub latitudes: Vec<i64>,
    pub longitudes: Vec<i64>,
    pub refs: Vec<i64>,
    pub member_ids: Vec<i64>,
    pub tag_offsets: Vec<u32>,
    pub ref_offsets: Vec<u32>,
    pub location_offsets: Vec<u32>,
    pub member_offsets: Vec<u32>,
    pub roles: Vec<u32>,
    pub tags: Vec<TagIds>,
    pub locations: Vec<Location>,
    pub locations_present: Vec<bool>,
    pub types: Vec<MemberType>,
    pub info: Vec<Metadata>,
    pub count: usize,
}
impl Columns {
    pub fn clear(&mut self) {
        self.ids.clear();
        self.latitudes.clear();
        self.longitudes.clear();
        self.refs.clear();
        self.member_ids.clear();
        self.tag_offsets.clear();
        self.ref_offsets.clear();
        self.location_offsets.clear();
        self.member_offsets.clear();
        self.roles.clear();
        self.tags.clear();
        self.locations.clear();
        self.locations_present.clear();
        self.types.clear();
        self.info.clear();
        self.count = 0;
    }
    fn end_list(offsets: &mut Vec<u32>, size: usize) -> Result<()> {
        if offsets.is_empty() {
            offsets.push(0);
        }
        offsets.push(u32::try_from(size).map_err(|_| Error::Invalid("PBF list exceeds 32 bits"))?);
        Ok(())
    }
    fn read_info(bytes: &[u8], result: &mut Metadata) -> Result<()> {
        let mut r = Wire::new(bytes);
        while let Some(f) = r.next()? {
            match f.number {
                1 => result.version = narrow(f.integer()? as i64)?,
                2 => result.raw_timestamp = f.integer()? as i64,
                3 => result.changeset = f.integer()? as i64,
                4 => result.uid = narrow(f.integer()? as i64)?,
                5 => result.user_sid = sid(f.integer()?)?,
                6 => result.visible = f.integer()? != 0,
                _ => continue,
            }
            result.present |= 1 << (f.number - 1);
        }
        Ok(())
    }
    pub fn ordinary(
        &mut self,
        bytes: &[u8],
        kind: u32,
        options: EntityOptions,
        locations_allowed: bool,
    ) -> Result<()> {
        let nodes = kind == 1;
        let ways = kind == 3;
        let n = options.nodes.unwrap_or_default();
        let w = options.ways.unwrap_or_default();
        let rel = options.relations.unwrap_or_default();
        let want_tags = if nodes {
            n.tags
        } else if ways {
            w.tags
        } else {
            rel.tags
        };
        let want_info = if nodes {
            n.metadata
        } else if ways {
            w.metadata
        } else {
            rel.metadata
        };
        let want_members =
            !nodes && !ways && (rel.member_ids || rel.member_types || rel.member_roles);
        let tag_start = self.tags.len();
        let location_start = self.locations.len();
        let (mut keys, mut values, mut refs, mut lats, mut lons, mut roles, mut ids, mut types) =
            (0, 0, 0, 0, 0, 0, 0, 0);
        let (mut id, mut lat, mut lon, mut reference, mut member) = (0, 0, 0, 0, 0);
        let (mut required, mut location_fields) = (0, 0);
        let mut meta = Metadata::default();
        let mut r = Wire::new(bytes);
        while let Some(f) = r.next()? {
            match f.number {
                1 => {
                    id = if nodes {
                        zigzag(f.integer()?)
                    } else {
                        f.integer()? as i64
                    };
                    required |= 1;
                }
                2 | 3 if want_tags => f.repeated(|v| {
                    let i = tag_start
                        + if f.number == 2 {
                            let i = keys;
                            keys += 1;
                            i
                        } else {
                            let i = values;
                            values += 1;
                            i
                        };
                    if i == self.tags.len() {
                        self.tags.push(TagIds::default());
                    }
                    if f.number == 2 {
                        self.tags[i].key = sid(v)?;
                    } else {
                        self.tags[i].value = sid(v)?;
                    }
                    Ok(())
                })?,
                4 if want_info => Self::read_info(f.message()?, &mut meta)?,
                8 if nodes => {
                    if n.latitude {
                        lat = zigzag(f.integer()?);
                    }
                    required |= 2;
                }
                8 if ways => {
                    if w.node_refs {
                        f.repeated(|v| {
                            reference = delta(reference, zigzag(v))?;
                            self.refs.push(reference);
                            refs += 1;
                            Ok(())
                        })?;
                    } else if w.node_locations {
                        refs += f.count()?;
                    }
                }
                8 if want_members => {
                    if rel.member_roles {
                        f.repeated(|v| {
                            self.roles.push(sid(v)?);
                            roles += 1;
                            Ok(())
                        })?;
                    } else {
                        roles += f.count()?;
                    }
                }
                9 if nodes => {
                    if n.longitude {
                        lon = zigzag(f.integer()?);
                    }
                    required |= 4;
                }
                9 | 10 if ways && w.node_locations => {
                    location_fields |= if f.number == 9 { 1 } else { 2 };
                    f.repeated(|v| {
                        let i = location_start
                            + if f.number == 9 {
                                let i = lats;
                                lats += 1;
                                i
                            } else {
                                let i = lons;
                                lons += 1;
                                i
                            };
                        if i == self.locations.len() {
                            self.locations.push(Location::default());
                        }
                        if f.number == 9 {
                            lat = delta(lat, zigzag(v))?;
                            self.locations[i].raw_latitude = lat;
                        } else {
                            lon = delta(lon, zigzag(v))?;
                            self.locations[i].raw_longitude = lon;
                        }
                        Ok(())
                    })?;
                }
                9 if want_members => {
                    if rel.member_ids {
                        f.repeated(|v| {
                            member = delta(member, zigzag(v))?;
                            self.member_ids.push(member);
                            ids += 1;
                            Ok(())
                        })?;
                    } else {
                        ids += f.count()?;
                    }
                }
                10 if want_members => {
                    if rel.member_types {
                        f.repeated(|v| {
                            self.types.push(match v {
                                0 => MemberType::Node,
                                1 => MemberType::Way,
                                2 => MemberType::Relation,
                                _ => return Err(Error::Invalid("Invalid member type")),
                            });
                            types += 1;
                            Ok(())
                        })?;
                    } else {
                        types += f.count()?;
                    }
                }
                _ => {}
            }
        }
        require(
            required == if nodes { 7 } else { 1 },
            "Missing required entity field",
        )?;
        if !nodes || n.id {
            self.ids.push(id);
        }
        if nodes && n.latitude {
            self.latitudes.push(lat);
        }
        if nodes && n.longitude {
            self.longitudes.push(lon);
        }
        if want_tags {
            require(keys == values, "Unequal tag arrays")?;
            Self::end_list(&mut self.tag_offsets, self.tags.len())?;
        }
        if want_info {
            self.info.push(meta);
        }
        if ways && w.node_refs {
            Self::end_list(&mut self.ref_offsets, self.refs.len())?;
        }
        if ways && w.node_locations {
            require(
                location_fields == 0
                    || (location_fields == 3 && locations_allowed && lats == refs && lons == refs),
                "Invalid way locations",
            )?;
            Self::end_list(&mut self.location_offsets, self.locations.len())?;
            self.locations_present.push(location_fields != 0);
        }
        if want_members {
            require(roles == ids && ids == types, "Unequal member arrays")?;
            let previous = self.member_offsets.last().copied().unwrap_or(0) as usize;
            Self::end_list(&mut self.member_offsets, previous + ids)?;
        }
        self.count += 1;
        Ok(())
    }
    fn packed_info(&mut self, fields: &[Vec<Field<'_>>; 10]) -> Result<bool> {
        let mut encoded: [&[u8]; 6] = [&[]; 6];
        let mut present = 0;
        for i in 0..6 {
            if fields[i + 4].is_empty() {
                continue;
            }
            if fields[i + 4].len() != 1 || fields[i + 4][0].wire != 2 {
                return Ok(false);
            }
            encoded[i] = fields[i + 4][0].bytes;
            if !encoded[i].is_empty() {
                present |= 1 << i;
            }
        }
        if present == 0 {
            self.info.resize(self.count, Metadata::default());
            return Ok(true);
        }
        let [
            mut version,
            mut timestamp,
            mut changeset,
            mut uid,
            mut user,
            mut visible,
        ] = encoded.map(Wire::new);
        let (mut time, mut change, mut user_id, mut user_name) = (0, 0, 0, 0);
        self.info.reserve(self.count);
        for _ in 0..self.count {
            let mut item = Metadata {
                present,
                ..Metadata::default()
            };
            if present & 1 != 0 {
                item.version = narrow(version.varint()? as i64)?;
            }
            if present & 2 != 0 {
                time = delta(time, zigzag(timestamp.varint()?))?;
                item.raw_timestamp = time;
            }
            if present & 4 != 0 {
                change = delta(change, zigzag(changeset.varint()?))?;
                item.changeset = change;
            }
            if present & 8 != 0 {
                user_id = delta(user_id, zigzag(uid.varint()?))?;
                item.uid = narrow(user_id)?;
            }
            if present & 16 != 0 {
                user_name = delta(user_name, zigzag(user.varint()?))?;
                item.user_sid = sid(user_name as u64)?;
            }
            if present & 32 != 0 {
                item.visible = visible.varint()? != 0;
            }
            self.info.push(item);
        }
        require(
            [version, timestamp, changeset, uid, user, visible]
                .iter()
                .all(|r| r.bytes.is_empty()),
            "Extra dense metadata value",
        )?;
        Ok(true)
    }
    pub fn dense(&mut self, fields: &[Vec<Field<'_>>; 10], options: NodeOptions) -> Result<()> {
        fn deltas(fields: &[Field<'_>], out: &mut Vec<i64>) -> Result<()> {
            let mut value = 0;
            for f in fields {
                f.repeated(|v| {
                    value = delta(value, zigzag(v))?;
                    out.push(value);
                    Ok(())
                })?;
            }
            Ok(())
        }
        if options.id {
            deltas(&fields[0], &mut self.ids)?;
            self.count = self.ids.len();
        } else {
            for f in &fields[0] {
                self.count += f.count()?;
            }
        }
        if options.latitude {
            deltas(&fields[1], &mut self.latitudes)?;
            require(
                self.latitudes.len() == self.count,
                "Invalid dense latitude count",
            )?;
        }
        if options.longitude {
            deltas(&fields[2], &mut self.longitudes)?;
            require(
                self.longitudes.len() == self.count,
                "Invalid dense longitude count",
            )?;
        }
        if options.metadata && !self.packed_info(fields)? {
            self.info.resize(self.count, Metadata::default());
            for (column, list) in fields.iter().enumerate().skip(4) {
                let mut i = 0;
                let mut acc = 0;
                for f in list {
                    f.repeated(|v| {
                        require(i < self.count, "Extra dense metadata value")?;
                        let item = &mut self.info[i];
                        i += 1;
                        item.present |= 1 << (column - 4);
                        match column {
                            4 => item.version = narrow(v as i64)?,
                            5 => {
                                acc = delta(acc, zigzag(v))?;
                                item.raw_timestamp = acc;
                            }
                            6 => {
                                acc = delta(acc, zigzag(v))?;
                                item.changeset = acc;
                            }
                            7 => {
                                acc = delta(acc, zigzag(v))?;
                                item.uid = narrow(acc)?;
                            }
                            8 => {
                                acc = delta(acc, zigzag(v))?;
                                item.user_sid = sid(acc as u64)?;
                            }
                            9 => item.visible = v != 0,
                            _ => unreachable!(),
                        }
                        Ok(())
                    })?;
                }
                require(i == 0 || i == self.count, "Missing dense metadata values")?;
            }
        }
        if options.tags {
            self.tag_offsets.push(0);
            let mut key = None;
            let mut any = false;
            for f in &fields[3] {
                f.repeated(|v| {
                    any = true;
                    require(self.tag_offsets.len() <= self.count, "Extra dense tags")?;
                    if let Some(k) = key.take() {
                        self.tags.push(TagIds {
                            key: k,
                            value: sid(v)?,
                        });
                    } else if v == 0 {
                        self.tag_offsets.push(self.tags.len() as u32);
                    } else {
                        key = Some(sid(v)?);
                    }
                    Ok(())
                })?;
            }
            require(key.is_none(), "Missing dense tag value")?;
            if !any {
                self.tag_offsets.resize(self.count + 1, 0);
            }
            require(
                self.tag_offsets.len() == self.count + 1,
                "Missing dense tag delimiters",
            )?;
        }
        Ok(())
    }
    pub fn batch(
        &self,
        context: BatchContext,
        kind: u32,
        options: EntityOptions,
    ) -> GroupBatch<'_> {
        let tags = ListView {
            offsets: &self.tag_offsets,
            values: &self.tags,
        };
        match kind {
            1 | 2 if options.nodes.is_some() => GroupBatch::Nodes(NodeBatch {
                context,
                fields: options.nodes.unwrap(),
                ids: &self.ids,
                raw_latitudes: &self.latitudes,
                raw_longitudes: &self.longitudes,
                tags,
                metadata: &self.info,
            }),
            3 if options.ways.is_some() => GroupBatch::Ways(WayBatch {
                context,
                fields: options.ways.unwrap(),
                ids: &self.ids,
                tags,
                node_refs: ListView {
                    offsets: &self.ref_offsets,
                    values: &self.refs,
                },
                node_locations: ListView {
                    offsets: &self.location_offsets,
                    values: &self.locations,
                },
                locations_present: &self.locations_present,
                metadata: &self.info,
            }),
            4 if options.relations.is_some() => GroupBatch::Relations(RelationBatch {
                context,
                fields: options.relations.unwrap(),
                ids: &self.ids,
                tags,
                member_offsets: &self.member_offsets,
                member_ids: &self.member_ids,
                member_types: &self.types,
                member_roles: &self.roles,
                metadata: &self.info,
            }),
            _ => GroupBatch::Empty(context),
        }
    }
}

pub(crate) fn collect_dense<'a>(
    bytes: &'a [u8],
    fields: &mut [Vec<Field<'a>>; 10],
    options: NodeOptions,
) -> Result<()> {
    let mut r = Wire::new(bytes);
    while let Some(f) = r.next()? {
        match f.number {
            1 => fields[0].push(f),
            8 if options.latitude => fields[1].push(f),
            9 if options.longitude => fields[2].push(f),
            10 if options.tags => fields[3].push(f),
            5 if options.metadata => {
                let mut meta = Wire::new(f.message()?);
                while let Some(m) = meta.next()? {
                    if (1..=6).contains(&m.number) {
                        fields[m.number as usize + 3].push(m);
                    }
                }
            }
            _ => {}
        }
    }
    Ok(())
}
