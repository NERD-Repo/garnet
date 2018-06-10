// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#![allow(non_snake_case)]

use fidl_bluetooth_bonder::{BondingData, Key, LeConnectionParameters, LeData, Ltk};
use std::collections::HashMap;

#[derive(Serialize, Deserialize)]
#[serde(remote = "LeConnectionParameters")]
pub struct LeConnectionParametersDef {
    pub connection_interval: u16,
    pub connection_latency: u16,
    pub supervision_timeout: u16,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "Key")]
pub struct KeyDef {
    pub authenticated: bool,
    pub value: [u8; 16],
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "Ltk")]
pub struct LtkDef {
    #[serde(with = "KeyDef")]
    pub key: Key,
    pub key_size: u8,
    pub ediv: u16,
    pub rand: u64,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "LeData")]
pub struct LeDataDef {
    pub address: String,
    pub resolvable: bool,
    pub master: bool,
    #[serde(with = "LeConnectionParametersWrapper")]
    pub connection_parameters: Option<Box<LeConnectionParameters>>,
    pub services: Vec<String>,
    #[serde(with = "LtkWrapper")]
    pub ltk: Option<Box<Ltk>>,
    #[serde(with = "KeyWrapper")]
    pub irk: Option<Box<Key>>,
    #[serde(with = "KeyWrapper")]
    pub csrk: Option<Box<Key>>,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "BondingData")]
pub struct BondingDataDef {
    pub name: String,
    #[serde(with = "LeDataWrapper")]
    pub le: Option<Box<LeData>>,
}

/// Wrap the Ser/De types to work with Option<Box<T>>
macro_rules! optboxify {
    ($mod:ident, $b:ident, $c:ident, $d:expr) => {
        mod $mod {
            use super::{$b, $c};
            use serde::{Deserialize, Deserializer};
            use serde::{Serialize, Serializer};

            pub fn serialize<S>(value: &Option<Box<$b>>, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                #[derive(Serialize)]
                struct Wrapper<'a>(#[serde(with = $d)] &'a Box<$b>);
                value.as_ref().map(Wrapper).serialize(serializer)
            }

            pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Box<$b>>, D::Error>
            where
                D: Deserializer<'de>,
            {
                #[derive(Deserialize)]
                struct Wrapper(#[serde(with = $d)] $b);

                let helper = Option::deserialize(deserializer)?;
                Ok(helper.map(|Wrapper(external)| Box::new(external)))
            }
        }
    };
}

optboxify!(LeDataWrapper, LeData, LeDataDef, "LeDataDef");
optboxify!(KeyWrapper, Key, KeyDef, "KeyDef");
optboxify!(LtkWrapper, Ltk, LtkDef, "LtkDef");
optboxify!(
    LeConnectionParametersWrapper,
    LeConnectionParameters,
    LeConnectionParametersDef,
    "LeConnectionParametersDef"
);

#[derive(Serialize, Deserialize)]
pub struct BondMap(HashMap<String, VecBondingData>);

impl BondMap {
    pub fn inner(&self) -> &HashMap<String, VecBondingData> {
        &self.0
    }

    pub fn inner_mut(&mut self) -> &mut HashMap<String, VecBondingData> {
        &mut self.0
    }
}

#[derive(Serialize, Deserialize)]
pub struct VecBondingData {
    #[serde(with = "VecBondData")]
    pub inner: Vec<BondingData>,
}

//impl VecBondingData {
//    pub fn inner(&self) -> VecBondingData {
//        &self.inner
//    }
//}

// TODO
struct VecBondIter {}
//
//impl Iterator for VecBondIter {
//    type Item = BondingData;
//
//    fn next(&mut self) -> Option<Self::Item> {
//        self.inner.iter().next()
//    }
//}
//impl IntoIterator for VecBondingData {
//    type Item = BondingData;
//    //type IntoIterator = VecBondIter;
//
//    //fn into_iter(self) -> Self::IntoIterator {
//
//    //}
//}

mod VecBondData {
    use super::{BondingData, BondingDataDef};
    use serde::Serializer;
    use serde::{Deserialize, Deserializer};

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<BondingData>, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Wrapper(#[serde(with = "BondingDataDef")] BondingData);

        let v = Vec::deserialize(deserializer)?;
        Ok(v.into_iter().map(|Wrapper(a)| a).collect())
    }

    pub fn serialize<S>(value: &Vec<BondingData>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        #[derive(Serialize)]
        struct Wrapper<'a>(#[serde(with = "BondingDataDef")] &'a BondingData);

        serializer.collect_seq(value.iter().map(Wrapper))
    }
}
