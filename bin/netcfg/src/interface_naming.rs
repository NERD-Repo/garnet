// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.
use {crate::{Config, Interface},
     failure::{self, format_err, ResultExt},
     log::error,
     std::{fs, mem, path::Path}};

const MAC_ADDR_LEN: usize = 6;

const CUR_CFG_FILE: &str = "net.cfg.json";
const TMP_CFG_FILE: &str = "net.cfg.json.tmp";

// Stable Interface Naming
const PREFIX_WIRED_PCI_IFACE: &str = "ethp";
const PREFIX_WLAN_PCI_IFACE: &str = "wlanp";
const PREFIX_WIRED_OTHER_IFACE: &str = "ethx";
const PREFIX_WLAN_OTHER_IFACE: &str = "wlanx";

pub fn get_cur_config(
    default_config: &Config, cur_config_path: &Path,
) -> Result<Config, failure::Error> {
    let cur_config_file = cur_config_path.join(CUR_CFG_FILE);
    match fs::File::open(&cur_config_file) {
        Err(e) => {
            format!(
                "could not open current config file {}:{}",
                &cur_config_file.display(),
                e
            );
            Ok((*default_config).clone()) // use default_config instead
        }
        Ok(file) => match serde_json::from_reader(file) {
            Ok(config) => Ok(config),
            Err(e) => {
                error!(
                    "Failed to parse the netcfg from JSONin {}: {}. Starting with an \
                     default_config.",
                    cur_config_file.display(),
                    e
                );
                fs::remove_file(&cur_config_file).map_err(|e| {
                    format_err!("Failed to delete {}: {}", cur_config_file.display(), e)
                })?;
                Ok((*default_config).clone()) // use default_config instead
            }
        },
    }
}

fn lookup_eth_name(interfaces: &Vec<Interface>, topo: &String, mac_str: &String) -> Option<String> {
    match topo.split("/").find(|x| *x == "usb") {
        Some(_iter) => match interfaces.iter().find(|x| x.mac == *mac_str) {
            Some(iter) => Some(iter.name.clone()),
            None => None,
        },
        None => match interfaces.iter().find(|x| x.topo == *topo) {
            Some(iter) => Some(iter.name.clone()),
            None => None,
        },
    }
}

fn construct_eth_name_from_mac(
    interfaces: &Vec<Interface>, mac: &[u8; 6], is_wlan: bool,
) -> String {
    // use the last two digits mac address as suffix
    //check if the name is taken, if yes, increment suffix value
    let mut start_suffix: u8 = mac[MAC_ADDR_LEN - 1];
    let mut potential_name;
    loop {
        match is_wlan {
            true => potential_name = format!("{}{:X}", PREFIX_WLAN_OTHER_IFACE, start_suffix),
            false => potential_name = format!("{}{:X}", PREFIX_WIRED_OTHER_IFACE, start_suffix),
        };
        match interfaces.iter().find(|x| x.name == potential_name) {
            None => break,
            Some(_) => start_suffix = (start_suffix + 1) % 255,
        }
    }
    potential_name.clone()
}

fn construct_eth_name_from_topo(topo: &String, is_wlan: bool) -> String {
    // pci interface
    //let mut bdf: String = "".to_string();
    let mut bdf = topo
        .split('/')
        .skip_while(|s| !(*s == "pci"))
        .skip(1)
        .next()
        .unwrap()
        .to_string();
    let mut new_bdf = String::from("");
    //generate new_bdf from bdf, ex 03:00.0 will be converted to 03 as the tailing 0s are
    // dropped
    loop {
        let c = bdf.pop();
        if c == None {
            break;
        }
        if c.unwrap().is_digit(16) {
            if (c.unwrap() == '0' && new_bdf.len() != 0) || c.unwrap() != '0' {
                new_bdf = format!("{}{}", c.unwrap(), new_bdf)
            }
        }
    }
    match is_wlan {
        true => format!("{}{}", PREFIX_WLAN_PCI_IFACE, new_bdf),
        false => format!("{}{}", PREFIX_WIRED_PCI_IFACE, new_bdf),
    }
}

fn validity_topo(v: &Vec<&str>) -> bool {
    match v.iter().find(|x| **x == "pci") {
        Some(_iter) => true,
        None => false,
    }
}
fn construct_eth_name(
    interfaces: &Vec<Interface>, topo: &String, mac: &[u8; 6], is_wlan: bool,
) -> String {
    // check if it's a on board interface per topo, for topo pattern "pci" is followed by BDF(Bus
    // Device Function) ex. of PCI network interface topo path
    // "/dev/sys/pci/02:00.0/intel-ethernet/ethernet" ex. of USB network interface topo path
    // "/dev/sys/pci/00:14.
    // 0/xhci/usb/007/ifc-000/ralink-wlanphy/ralink-wlanmac/wlan/wlan-ethernet/ethernet"
    let v: Vec<&str> = topo.split('/').collect();
    if validity_topo(&v) {
        match v.iter().find(|x| **x == "usb") {
            Some(_iter) => construct_eth_name_from_mac(interfaces, mac, is_wlan),
            None => construct_eth_name_from_topo(topo, is_wlan),
        }
    } else {
        construct_eth_name_from_mac(interfaces, &mac, is_wlan)
    }
}

fn store(
    config: &mut Config, topo: &String, mac_str: &String, name: &String, config_file_path: &Path,
) -> Result<(), failure::Error> {
    config.eth_config.interfaces.push(Interface {
        name: name.to_string(),
        topo: topo.to_string(),
        mac: mac_str.to_string(),
    });
    let temp_file_path = config_file_path.join(TMP_CFG_FILE);
    let config_file_path = config_file_path.join(CUR_CFG_FILE);
    let temp_file = fs::File::create(&temp_file_path).with_context(|_| {
        format_err!(
            "could not create the temp file {}",
            temp_file_path.display()
        )
    })?;

    serde_json::to_writer_pretty(&temp_file, config).map_err(|e| {
        format_err!(
            "Failed to serialize JSON into {}: {}",
            temp_file_path.display(),
            e
        )
    })?;

    mem::drop(&temp_file);
    fs::rename(&temp_file_path, config_file_path)?;
    mem::forget(&temp_file_path);
    Ok(())
}

pub fn get_stable_interface_name(
    mut cur_config: &mut Config, topo: &mut String, mac: &[u8; 6], is_wlan: bool,
    path_to_store: &Path,
) -> String {
    if topo.starts_with("@") {
        topo.remove(0);
    }
    let mac_str = format!(
        "{:X}:{:X}:{:X}:{:X}:{:X}:{:X}",
        mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]
    );
    let name = match lookup_eth_name(&mut cur_config.eth_config.interfaces, &topo, &mac_str) {
        Some(name) => name,
        None => {
            let name = construct_eth_name(&cur_config.eth_config.interfaces, &topo, &mac, is_wlan);

            match store(&mut cur_config, &topo, &mac_str, &name, path_to_store) {
                Err(e) => error!(
                    "Failed to store the interface to the configuration file in {} as {}.",
                    path_to_store.display(),
                    e
                ),
                Ok(_) => (),
            }

            name
        }
    };
    name.clone()
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
