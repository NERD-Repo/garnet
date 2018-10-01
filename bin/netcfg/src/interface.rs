// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::fmt;
use std::fs;
use std::path;

use failure::{self, ResultExt};
use serde_derive::{Deserialize, Serialize};

// https://serde.rs/remote-derive.html
#[derive(Serialize, Deserialize)]
#[serde(remote = "fidl_fuchsia_netstack::InterfaceConfig")]
struct InterfaceConfigDef {
    #[allow(dead_code)]
    name: String,
}

#[derive(Serialize, Deserialize)]
pub struct InterfaceConfig {
    #[serde(with = "InterfaceConfigDef")]
    inner: fidl_fuchsia_netstack::InterfaceConfig,
}

impl InterfaceConfig {
    pub fn new(inner: fidl_fuchsia_netstack::InterfaceConfig) -> Self {
        Self { inner }
    }

    pub fn into_mut(&mut self) -> &mut fidl_fuchsia_netstack::InterfaceConfig {
        let Self { ref mut inner } = self;
        inner
    }
}

// https://serde.rs/remote-derive.html
#[derive(Serialize, Deserialize)]
#[serde(remote = "fidl_zircon_ethernet::MacAddress")]
struct MacAddressDef {
    #[allow(dead_code)]
    octets: [u8; 6],
}

#[derive(Serialize, Deserialize)]
pub struct MacAddress {
    #[serde(with = "MacAddressDef")]
    inner: fidl_zircon_ethernet::MacAddress,
}

impl PartialEq for MacAddress {
    fn eq(&self, other: &Self) -> bool {
        let Self {
            inner:
                fidl_zircon_ethernet::MacAddress {
                    octets: self_octets,
                },
        } = self;
        let Self {
            inner:
                fidl_zircon_ethernet::MacAddress {
                    octets: other_octets,
                },
        } = other;
        self_octets == other_octets
    }
}

impl Eq for MacAddress {}

impl MacAddress {
    pub fn new(inner: fidl_zircon_ethernet::MacAddress) -> Self {
        Self { inner }
    }
}

impl fmt::Display for MacAddress {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let Self {
            inner: fidl_zircon_ethernet::MacAddress { octets },
        } = self;
        for (i, byte) in octets.iter().enumerate() {
            if i > 0 {
                write!(f, ":")?;
            }
            write!(f, "{:x}", byte)?;
        }
        Ok(())
    }
}

pub struct Identifier {
    topological_path: String,
    mac_address: MacAddress,
    wlan: bool,
}

impl Identifier {
    pub fn new(topological_path: String, mac_address: MacAddress, wlan: bool) -> Self {
        Self {
            topological_path,
            mac_address,
            wlan,
        }
    }
}

#[derive(PartialEq, Eq, Serialize, Deserialize)]
enum PersistentIdentifier {
    MacAddress(MacAddress),
    TopologicalPath(String),
}

#[derive(Serialize, Deserialize)]
struct Config {
    names: Vec<(PersistentIdentifier, InterfaceConfig)>,
}

impl Config {
    fn load<P: AsRef<path::Path>>(path: P) -> Result<Self, failure::Error> {
        let path = path.as_ref();
        let file = fs::File::open(path)
            .with_context(|_| format!("could not open config file {}", path.display()))?;
        let config = serde_json::from_reader(file)
            .with_context(|_| format!("could not deserialize config file {}", path.display()))?;
        Ok(config)
    }
}

pub struct FileBackedConfig<'a> {
    path: &'a path::Path,
    config: Config,
}

impl<'a> FileBackedConfig<'a> {
    pub fn load<P: AsRef<path::Path>>(path: &'a P) -> Result<Self, failure::Error> {
        let path = path.as_ref();
        let config = Config::load(path)?;
        Ok(Self { path, config })
    }

    pub fn store(&self) -> Result<(), failure::Error> {
        let Self { path, config } = self;
        let temp_file_path = match path.file_name() {
            None => Err(failure::format_err!(
                "unexpected non-file path {}",
                path.display()
            )),
            Some(file_name) => {
                let mut file_name = file_name.to_os_string();
                file_name.push(".tmp");
                Ok(path.with_file_name(file_name))
            }
        }?;
        {
            let temp_file = fs::File::create(&temp_file_path).with_context(|_| {
                format!(
                    "could not create temporary file {}",
                    temp_file_path.display()
                )
            })?;
            serde_json::to_writer_pretty(temp_file, &config).with_context(|_| {
                format!(
                    "could not serialize config into temporary file {}",
                    temp_file_path.display()
                )
            })?;
        }

        fs::rename(&temp_file_path, path).with_context(|_| {
            format!(
                "could not rename temporary file {} to {}",
                temp_file_path.display(),
                path.display()
            )
        })?;
        Ok(())
    }

    pub fn get(&mut self, id: Identifier) -> Result<&mut InterfaceConfig, failure::Error> {
        let Identifier {
            mut topological_path,
            mac_address,
            wlan,
        } = id;
        // TODO(chunyingw): why do we strip this character? cite something.
        if topological_path.starts_with('@') {
            topological_path.remove(0);
        };

        // We use MAC addresses to identify USB devices; USB devices are those devices whose
        // topological path contains "/usb/". We use topological paths to identify on-board
        // devices; on-board devices are those devices whose topological path does not
        // contain "/usb". Topological paths of
        // both device types are expected to
        // contain "/pci"; devices whose topological path does not contain "/pci/" are
        // identified by their MAC address.
        //
        // At the time of writing, typical topological paths appear similar to:
        //
        // on-board:
        // "/dev/sys/pci/02:00.0/intel-ethernet/ethernet"
        //
        // USB:
        // "/dev/sys/pci/00:14.0/xhci/usb/007/ifc-000/<snip>/wlan/wlan-ethernet/ethernet"
        let persistent_id = match topological_path.contains("/usb/") {
            true => PersistentIdentifier::MacAddress(mac_address),
            false => PersistentIdentifier::TopologicalPath(topological_path),
        };
        let index =
            if let Some(index) = self.config.names.iter().enumerate().find_map(|(i, (key, _value))| {
                if key == &persistent_id {
                    Some(i)
                } else {
                    None
                }
            }) {
                index
            } else {
                let name = match persistent_id {
                    PersistentIdentifier::MacAddress(MacAddress {
                        inner: fidl_zircon_ethernet::MacAddress { octets },
                    }) => {
                        let prefix = match wlan {
                            true => "wlanx",
                            false => "ethx",
                        };
                        let last_byte = octets[octets.len() - 1];
                        match (0u8..255u8).find_map(|i| {
                            let candidate = last_byte + i;
                            match self.config.names.iter().any(
                                |(
                                    _key,
                                    InterfaceConfig {
                                        inner: fidl_fuchsia_netstack::InterfaceConfig { name },
                                    },
                                )| {
                                    name.starts_with(prefix)
                                        && u8::from_str_radix(&name[prefix.len()..], 16)
                                            == Ok(candidate)
                                },
                            ) {
                                true => None,
                                false => Some(format!("{}{:X}", prefix, candidate)),
                            }
                        }) {
                            None => Err(failure::format_err!(
                                "could not find unique name for mac={}, wlan={}",
                                MacAddress::new(fidl_zircon_ethernet::MacAddress { octets }),
                                wlan
                            )),
                            Some(name) => Ok(name),
                        }
                    }
                    PersistentIdentifier::TopologicalPath(ref topological_path) => {
                        let prefix = match wlan {
                            true => "wlanp",
                            false => "ethp",
                        };
                        let pat = "/pci/";
                        match topological_path.find(pat) {
                            None => Err(failure::format_err!(
                                "unexpected non-PCI topological path {}",
                                topological_path
                            )),
                            Some(index) => {
                                let topological_path = &topological_path[index + pat.len()..];
                                match topological_path.find('/') {
                                    None => Err(failure::format_err!(
                                        "unexpected PCI topological path suffix {}",
                                        topological_path
                                    )),
                                    Some(index) => {
                                        let mut name = String::from(prefix);
                                        for digit in topological_path[..index]
                                            .trim_right_matches(|c: char| {
                                                !c.is_digit(16) || c == '0'
                                            }).chars()
                                            .filter(|c| c.is_digit(16))
                                        {
                                            name.push(digit);
                                        }
                                        Ok(name)
                                    }
                                }
                            }
                        }
                    }
                }?;
                self.config.names.push((
                    persistent_id,
                    InterfaceConfig::new(fidl_fuchsia_netstack::InterfaceConfig { name }),
                ));
                self.store()?;
                self.config.names.len() - 1
            };
        let (ref _key, ref mut value) = self.config.names[index];
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use {crate::{DnsConfig, EthConfig},
         std::io::Write,
         tempdir};
    #[test]
    fn test_lookup_and_construct_name() {
        // create an empty default_config
        let default_config = Config {
            device_name: None,
            dns_config: DnsConfig { servers: vec![] },
            eth_config: EthConfig { interfaces: vec![] },
        };
        // test cases for usb interfaces
        let topo_usb =
            String::from("@/dev/sys/pci/00:14.0/xhci/usb/004/004/ifc-000/ax88179/ethernet");
        let mac1 = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06];

        // expect empty cur_config
        let temp_dir = tempdir::TempDir::new("netcfg_test1").expect("failed to create temp dir");
        let cur_config = get_cur_config(&default_config, &temp_dir.path()).unwrap();
        let mac1_str = format!(
            "{:X}:{:X}:{:X}:{:X}:{:X}:{:X}",
            mac1[0], mac1[1], mac1[2], mac1[3], mac1[4], mac1[5]
        );
        assert_eq!(
            None,
            lookup_eth_name(&cur_config.eth_config.interfaces, &topo_usb, &mac1_str)
        ); // No match
        assert_eq!(
            "wlanx6",
            construct_eth_name(&cur_config.eth_config.interfaces, &topo_usb, &mac1, true)
        ); // wlan usb interface, use mac to contruct a name
        assert_eq!(
            "ethx6",
            construct_eth_name(&cur_config.eth_config.interfaces, &topo_usb, &mac1, false)
        );

        //test case for PCI interfaces
        let topo_pci = String::from("@/dev/sys/pci/00:14.0/ethernet");
        let mac2 = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06];
        let mac2_str = format!(
            "{:X}:{:X}:{:X}:{:X}:{:X}:{:X}",
            mac2[0], mac2[1], mac2[2], mac2[3], mac2[4], mac2[5]
        );
        assert_eq!(
            None,
            lookup_eth_name(&cur_config.eth_config.interfaces, &topo_pci, &mac2_str)
        ); // No match
        assert_eq!(
            "wlanp0014",
            construct_eth_name(&cur_config.eth_config.interfaces, &topo_pci, &mac2, true)
        ); // wlan pci interface, use mac to contruct a name

        assert_eq!(
            "ethp0014",
            construct_eth_name(&cur_config.eth_config.interfaces, &topo_pci, &mac2, false)
        ); //wired pci interface, use BDF to contruct a name

        //Bad topo path
        let topo_bad_1 = String::from("@/dev/sys/00:14.0/xhci/004/004/ifc-000/ax88179/ethernet"); // without "usb" or "pci"
        let mac3 = [0x01, 0x01, 0x01, 0x01, 0x01, 0x01];
        let mac3_str = format!(
            "{:X}:{:X}:{:X}:{:X}:{:X}:{:X}",
            mac3[0], mac3[1], mac3[2], mac3[3], mac3[4], mac3[5]
        );
        assert_eq!(
            None,
            lookup_eth_name(&cur_config.eth_config.interfaces, &topo_bad_1, &mac3_str)
        ); // No match
        assert_eq!(
            "wlanx1",
            construct_eth_name(&cur_config.eth_config.interfaces, &topo_bad_1, &mac3, true)
        ); // wlan pci interface, use mac to contruct a name
    }
    #[test]
    fn test_get_stable_interface_name() {
        // create an empty default_config
        let default_config = Config {
            device_name: None,
            dns_config: DnsConfig { servers: vec![] },
            eth_config: EthConfig { interfaces: vec![] },
        };
        // test cases for usb interfaces
        let mut topo_usb =
            String::from("@/dev/sys/pci/00:14.0/xhci/usb/004/004/ifc-000/ax88179/ethernet");
        let topo_usb_updated =
            String::from("/dev/sys/pci/00:14.0/xhci/usb/004/004/ifc-000/ax88179/ethernet");
        let mac1 = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06];
        let mac2 = [0x02, 0x02, 0x03, 0x04, 0x05, 0x06]; // use the same topo as mac1 and the last u8 of mac is the same as mac1
        let temp_dir = tempdir::TempDir::new("netcfg_test2").expect("failed to create temp dir");
        // expect empty cur_config
        let mut cur_config = get_cur_config(&default_config, &temp_dir.path()).unwrap();

        let name = get_stable_interface_name(
            &mut cur_config,
            &mut topo_usb,
            &mac1,
            false,
            &temp_dir.path(),
        ); // stored into file
        assert_eq!(topo_usb, topo_usb_updated); // "@" at the beginning of the topo is trimmed off
        assert_eq!("ethx6".to_string(), name);
        let mac1_str = format!(
            "{:X}:{:X}:{:X}:{:X}:{:X}:{:X}",
            mac1[0], mac1[1], mac1[2], mac1[3], mac1[4], mac1[5]
        );
        assert_eq!(
            Some("ethx6".to_string()),
            lookup_eth_name(&cur_config.eth_config.interfaces, &topo_usb, &mac1_str)
        ); // for topo_usb, mac1, found
        let mac2_str = format!(
            "{:X}:{:X}:{:X}:{:X}:{:X}:{:X}",
            mac2[0], mac2[1], mac2[2], mac2[3], mac2[4], mac2[5]
        );
        assert_eq!(
            None,
            lookup_eth_name(&cur_config.eth_config.interfaces, &topo_usb, &mac2_str)
        ); // No match, for usb interface match mac
        let name = get_stable_interface_name(
            &mut cur_config,
            &mut topo_usb,
            &mac1,
            false,
            &temp_dir.path(),
        );
        assert_eq!("ethx6".to_string(), name);
        let name = get_stable_interface_name(
            &mut cur_config,
            &mut topo_usb,
            &mac2,
            false,
            &temp_dir.path(),
        );
        assert_eq!("ethx7".to_string(), name);
    }
    #[test]
    fn test_get_cur_config() {
        let default_config = Config {
            device_name: None,
            dns_config: DnsConfig { servers: vec![] },
            eth_config: EthConfig { interfaces: vec![] },
        };
        // if the cur_config_file fails to open, use the default_config
        let cur_config = get_cur_config(&default_config, &Path::new("/dev/null")).unwrap();
        assert_eq!(
            default_config.eth_config.interfaces.len(),
            cur_config.eth_config.interfaces.len()
        );
        // if the cur_config_file has invalid format
        let temp_dir = tempdir::TempDir::new("netcfg_test3").expect("failed to create temp dir");
        let path = temp_dir.path().join(CUR_CFG_FILE);
        let mut file = fs::File::create(&path).expect("failed to open file for writing");
        // Write invalid JSON and close the file
        file.write(b"{")
            .expect("failed to write broken json into file");
        mem::drop(file);
        assert!(path.exists());
        // succeed in getting cur_config from default_config
        let mut cur_config = get_cur_config(&default_config, &temp_dir.path()).unwrap();
        assert!(!path.exists());
        assert_eq!(
            default_config.eth_config.interfaces.len(),
            cur_config.eth_config.interfaces.len()
        );
        // Writing an entry should create the file
        let mut topo_usb =
            String::from("@/dev/sys/pci/00:14.0/xhci/usb/004/004/ifc-000/ax88179/ethernet");
        let mac1 = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06];
        get_stable_interface_name(
            &mut cur_config,
            &mut topo_usb,
            &mac1,
            false,
            &temp_dir.path(),
        );
        assert!(path.exists())
    }
    #[test]
    fn test_store_with_invalid_path() {
        // failed to store in the configuration file, the program will continue instead of bailing
        let mut cur_config = Config {
            device_name: None,
            dns_config: DnsConfig { servers: vec![] },
            eth_config: EthConfig { interfaces: vec![] },
        };
        let mut topo_usb =
            String::from("@/dev/sys/pci/00:14.0/xhci/usb/004/004/ifc-000/ax88179/ethernet");

        let mac1 = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06];

        let name = get_stable_interface_name(
            &mut cur_config,
            &mut topo_usb,
            &mac1,
            false,
            &Path::new("/dev/null"),
        );
        assert_eq!(name, "ethx6");
    }
}
