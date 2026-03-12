use mlua::{MetaMethod, UserData, UserDataMethods, Value};
use rmpc_mpd::commands::metadata_tag::MetadataTag as MpdMetadataTag;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MetadataTag {
    One(String),
    Many(Vec<String>),
}

impl From<MpdMetadataTag> for MetadataTag {
    fn from(value: MpdMetadataTag) -> Self {
        match value {
            MpdMetadataTag::Single(s) => MetadataTag::One(s),
            MpdMetadataTag::Multiple(v) => MetadataTag::Many(v),
        }
    }
}

impl From<&MpdMetadataTag> for MetadataTag {
    fn from(value: &MpdMetadataTag) -> Self {
        match value {
            MpdMetadataTag::Single(s) => MetadataTag::One(s.clone()),
            MpdMetadataTag::Multiple(v) => MetadataTag::Many(v.clone()),
        }
    }
}

pub trait MetadataTagExt {
    fn to_metadata_tag(&self) -> Option<MetadataTag>;
}

impl MetadataTagExt for Option<MpdMetadataTag> {
    fn to_metadata_tag(&self) -> Option<MetadataTag> {
        self.as_ref().map(MetadataTag::from)
    }
}

impl MetadataTagExt for Option<&MpdMetadataTag> {
    fn to_metadata_tag(&self) -> Option<MetadataTag> {
        self.as_ref().map(|tag| MetadataTag::from(*tag))
    }
}

impl MetadataTag {
    pub fn first(&self) -> &str {
        match self {
            MetadataTag::One(s) => s,
            MetadataTag::Many(v) => &v[0],
        }
    }

    pub fn last(&self) -> &str {
        match self {
            MetadataTag::One(s) => s,
            MetadataTag::Many(v) => v.last().expect("non-empty"),
        }
    }

    pub fn join(&self, sep: &str) -> String {
        match self {
            MetadataTag::One(s) => s.clone(),
            MetadataTag::Many(v) => v.join(sep),
        }
    }

    pub fn values(&self) -> &[String] {
        match self {
            MetadataTag::One(s) => std::slice::from_ref(s),
            MetadataTag::Many(v) => v.as_slice(),
        }
    }
}

impl UserData for MetadataTag {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("first", |_, this, ()| Ok(this.first().to_string()));
        methods.add_method("last", |_, this, ()| Ok(this.last().to_string()));
        methods.add_method("join", |_, this, sep: Option<String>| {
            Ok(this.join(sep.as_deref().unwrap_or("")))
        });
        methods.add_method("values", |lua, this, ()| {
            let table = lua.create_table()?;
            for (i, value) in this.values().iter().enumerate() {
                table.set(i + 1, value.clone())?;
            }
            Ok(table)
        });

        methods.add_meta_method(MetaMethod::ToString, |_, this, ()| Ok(this.first().to_string()));
        methods.add_meta_function(MetaMethod::Concat, |lua, (lhs, rhs): (Value, Value)| {
            fn as_string(lua: &mlua::Lua, v: Value) -> mlua::Result<String> {
                match v {
                    Value::UserData(ud) => {
                        if let Ok(tag) = ud.borrow::<MetadataTag>() {
                            return Ok(tag.first().to_string());
                        }

                        lua.coerce_string(Value::UserData(ud))?
                            .map_or_else(|| Ok(String::new()), |s| Ok(s.to_str()?.to_owned()))
                    }
                    other => Ok(other.to_string()?),
                }
            }

            let l = as_string(lua, lhs)?;
            let r = as_string(lua, rhs)?;
            Ok(format!("{l}{r}"))
        });
    }
}
