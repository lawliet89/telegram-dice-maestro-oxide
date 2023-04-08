use std::borrow::Cow;
use std::fmt::Display;
use std::fs::File;
use std::io::BufReader;
use std::str::FromStr;

use anyhow::Context;
use serde::{Deserialize, Serialize};
use serde_aux::field_attributes::deserialize_number_from_string;

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
#[serde(rename_all = "snake_case")]
#[serde(deny_unknown_fields)]
pub struct Character<'a> {
    name: Cow<'a, str>,

    attribute_modifiers: AttributeModifiers<i8>,
    saving_throw_modifiers: AttributeModifiers<i8>,

    skill_modifiers: SkillModifiers<i8>,

    #[serde(deserialize_with = "deserialize_number_from_string")]
    initiative_modifier: i8,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
#[serde(rename_all = "snake_case")]
#[serde(deny_unknown_fields)]
pub struct AttributeModifiers<T>
where
    T: FromStr + serde::de::DeserializeOwned,
    <T as FromStr>::Err: Display,
{
    #[serde(alias = "str")]
    #[serde(deserialize_with = "deserialize_number_from_string")]
    strength: T,
    #[serde(alias = "dex")]
    #[serde(deserialize_with = "deserialize_number_from_string")]
    dexterity: T,
    #[serde(alias = "con")]
    #[serde(deserialize_with = "deserialize_number_from_string")]
    constitution: T,
    #[serde(alias = "int")]
    #[serde(deserialize_with = "deserialize_number_from_string")]
    intelligence: T,
    #[serde(alias = "wis")]
    #[serde(deserialize_with = "deserialize_number_from_string")]
    wisdom: T,
    #[serde(alias = "cha")]
    #[serde(deserialize_with = "deserialize_number_from_string")]
    charisma: T,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
#[serde(rename_all = "PascalCase")]
#[serde(deny_unknown_fields)]
pub struct SkillModifiers<T>
where
    T: FromStr + serde::de::DeserializeOwned,
    <T as FromStr>::Err: Display,
{
    #[serde(deserialize_with = "deserialize_number_from_string")]
    acrobatics: T,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    #[serde(alias = "Animal Handling")]
    animal_handling: T,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    arcana: T,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    athletics: T,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    deception: T,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    history: T,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    insight: T,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    intimidation: T,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    investigation: T,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    medicine: T,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    nature: T,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    perception: T,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    performance: T,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    persuasion: T,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    religion: T,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    #[serde(alias = "Sleight of Hand")]
    sleight_of_hand: T,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    stealth: T,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    survival: T,
}

impl<'a> Character<'a> {
    pub fn from_json_file<P>(path: P) -> anyhow::Result<Self>
    where
        P: AsRef<std::path::Path> + std::fmt::Debug,
    {
        let file = File::open(&path).with_context(|| format!("error opening file {:?}", path))?;
        let reader = BufReader::new(file);
        serde_json::from_reader(reader)
            .with_context(|| format!("error deserializing JSON {:?}", path))
    }

    pub fn load_from_pattern<S: AsRef<str>>(
        pattern: S,
    ) -> anyhow::Result<(Vec<Self>, Vec<anyhow::Error>)> {
        let result = glob::glob(pattern.as_ref())
            .with_context(|| format!("error figuring out path {}", pattern.as_ref()))?
            .map(|entry| {
                entry
                    .with_context(|| "error handling file")
                    .and_then(Self::from_json_file)
            });

        let (ok, err): (Vec<_>, Vec<_>) = result.partition(Result::is_ok);

        let ok = ok.into_iter().map(|r| r.unwrap()).collect();
        let err = err.into_iter().map(|r| r.unwrap_err()).collect();

        Ok((ok, err))
    }
}
